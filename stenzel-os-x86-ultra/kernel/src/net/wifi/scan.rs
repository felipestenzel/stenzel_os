//! WiFi Scanning
//!
//! Active and passive scanning for WiFi networks.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use crate::util::{KResult, KError};
use crate::sync::IrqSafeMutex;
use super::mac::MacAddress;
use super::frame::{ProbeRequestBody, InformationElement, ElementId};
use super::{WifiNetwork, WifiChannel, WifiBand, ChannelWidth};

/// Scan type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanType {
    /// Passive scan - only listen for beacons
    Passive,
    /// Active scan - send probe requests
    Active,
}

/// Scan configuration
#[derive(Debug, Clone)]
pub struct ScanConfig {
    /// Scan type
    pub scan_type: ScanType,
    /// Specific SSID to scan for (None = wildcard)
    pub ssid: Option<String>,
    /// Channels to scan (empty = all supported)
    pub channels: Vec<WifiChannel>,
    /// Dwell time per channel (milliseconds)
    pub dwell_time_ms: u64,
    /// Minimum dwell time for passive scan
    pub min_dwell_time_ms: u64,
    /// Maximum dwell time for passive scan
    pub max_dwell_time_ms: u64,
    /// Number of probe requests per channel (for active scan)
    pub probes_per_channel: u8,
    /// Flush old results before scan
    pub flush: bool,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            scan_type: ScanType::Active,
            ssid: None,
            channels: Vec::new(),
            dwell_time_ms: 100,
            min_dwell_time_ms: 60,
            max_dwell_time_ms: 200,
            probes_per_channel: 2,
            flush: true,
        }
    }
}

/// Scan state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanState {
    /// Not scanning
    Idle,
    /// Scan in progress
    Scanning,
    /// Scan completed
    Completed,
    /// Scan aborted
    Aborted,
}

/// Scan result entry with additional metadata
#[derive(Debug, Clone)]
pub struct ScanResult {
    /// Network information
    pub network: WifiNetwork,
    /// Timestamp when discovered (kernel ticks)
    pub timestamp: u64,
    /// Number of times beacon/probe response received
    pub seen_count: u32,
    /// Last seen timestamp
    pub last_seen: u64,
    /// Signal quality (0-100)
    pub quality: u8,
    /// Noise level (dBm)
    pub noise: i8,
}

impl ScanResult {
    /// Create from WifiNetwork
    pub fn from_network(network: WifiNetwork, timestamp: u64) -> Self {
        let quality = signal_to_quality(network.signal_strength);
        Self {
            network,
            timestamp,
            seen_count: 1,
            last_seen: timestamp,
            quality,
            noise: -95, // Default noise floor
        }
    }

    /// Update with new sighting
    pub fn update(&mut self, signal: i8, timestamp: u64) {
        self.network.signal_strength = signal;
        self.quality = signal_to_quality(signal);
        self.seen_count += 1;
        self.last_seen = timestamp;
    }

    /// Age of result in ticks
    pub fn age(&self, current_time: u64) -> u64 {
        current_time.saturating_sub(self.last_seen)
    }
}

/// Convert signal strength (dBm) to quality percentage
fn signal_to_quality(signal_dbm: i8) -> u8 {
    // Typical range: -30 dBm (excellent) to -90 dBm (poor)
    if signal_dbm >= -30 {
        100
    } else if signal_dbm <= -90 {
        0
    } else {
        // Linear interpolation
        let range = 60; // -30 to -90
        let offset = signal_dbm + 90;
        ((offset as u32 * 100) / range as u32) as u8
    }
}

/// Scanner manages WiFi scanning
pub struct Scanner {
    /// Current state
    state: ScanState,
    /// Configuration
    config: ScanConfig,
    /// Results
    results: Vec<ScanResult>,
    /// Current channel index
    channel_index: usize,
    /// Probes sent on current channel
    probes_sent: u8,
    /// Channel dwell start time
    dwell_start: u64,
    /// Home channel (to return to after scan)
    home_channel: Option<WifiChannel>,
}

impl Scanner {
    /// Create new scanner
    pub const fn new() -> Self {
        Self {
            state: ScanState::Idle,
            config: ScanConfig {
                scan_type: ScanType::Active,
                ssid: None,
                channels: Vec::new(),
                dwell_time_ms: 100,
                min_dwell_time_ms: 60,
                max_dwell_time_ms: 200,
                probes_per_channel: 2,
                flush: true,
            },
            results: Vec::new(),
            channel_index: 0,
            probes_sent: 0,
            dwell_start: 0,
            home_channel: None,
        }
    }

    /// Start a new scan
    pub fn start(&mut self, config: ScanConfig, current_time: u64) -> KResult<()> {
        if self.state == ScanState::Scanning {
            return Err(KError::Busy);
        }

        // Determine channels to scan
        let channels = if config.channels.is_empty() {
            default_scan_channels()
        } else {
            config.channels.clone()
        };

        if channels.is_empty() {
            return Err(KError::Invalid);
        }

        if config.flush {
            self.results.clear();
        }

        self.config = ScanConfig { channels, ..config };
        self.state = ScanState::Scanning;
        self.channel_index = 0;
        self.probes_sent = 0;
        self.dwell_start = current_time;

        Ok(())
    }

    /// Stop scanning
    pub fn stop(&mut self) {
        if self.state == ScanState::Scanning {
            self.state = ScanState::Aborted;
        }
    }

    /// Get current state
    pub fn state(&self) -> ScanState {
        self.state
    }

    /// Get current channel to scan
    pub fn current_channel(&self) -> Option<&WifiChannel> {
        if self.state == ScanState::Scanning {
            self.config.channels.get(self.channel_index)
        } else {
            None
        }
    }

    /// Check if should send probe request
    pub fn should_probe(&self) -> bool {
        self.state == ScanState::Scanning &&
        self.config.scan_type == ScanType::Active &&
        self.probes_sent < self.config.probes_per_channel
    }

    /// Mark probe as sent
    pub fn probe_sent(&mut self) {
        self.probes_sent += 1;
    }

    /// Create probe request frame body
    pub fn create_probe_request(&self) -> ProbeRequestBody {
        let rates = vec![
            0x82, 0x84, 0x8b, 0x96, // Basic rates (1, 2, 5.5, 11 Mbps)
            0x0c, 0x12, 0x18, 0x24, // Extended rates (6, 9, 12, 18 Mbps)
        ];

        let mut elements = Vec::new();

        // SSID
        elements.push(InformationElement {
            id: ElementId::Ssid as u8,
            data: self.config.ssid.as_ref()
                .map(|s| s.as_bytes().to_vec())
                .unwrap_or_default(),
        });

        // Supported rates
        elements.push(InformationElement {
            id: ElementId::SupportedRates as u8,
            data: rates[..8.min(rates.len())].to_vec(),
        });

        // Extended supported rates
        if rates.len() > 8 {
            elements.push(InformationElement {
                id: ElementId::ExtendedSupportedRates as u8,
                data: rates[8..].to_vec(),
            });
        }

        ProbeRequestBody { elements }
    }

    /// Check if should move to next channel
    pub fn should_advance(&self, current_time: u64) -> bool {
        if self.state != ScanState::Scanning {
            return false;
        }

        let elapsed = current_time.saturating_sub(self.dwell_start);
        let dwell_complete = elapsed >= self.config.dwell_time_ms;

        // For active scan, wait for probes to be sent
        let probes_done = self.config.scan_type == ScanType::Passive ||
            self.probes_sent >= self.config.probes_per_channel;

        dwell_complete && probes_done
    }

    /// Advance to next channel
    pub fn advance(&mut self, current_time: u64) -> bool {
        self.channel_index += 1;
        self.probes_sent = 0;
        self.dwell_start = current_time;

        if self.channel_index >= self.config.channels.len() {
            self.state = ScanState::Completed;
            false
        } else {
            true
        }
    }

    /// Process a discovered network
    pub fn process_network(&mut self, network: WifiNetwork, current_time: u64) {
        // Check if already in results
        if let Some(existing) = self.results.iter_mut()
            .find(|r| r.network.bssid == network.bssid)
        {
            existing.update(network.signal_strength, current_time);
        } else {
            self.results.push(ScanResult::from_network(network, current_time));
        }
    }

    /// Get scan results
    pub fn results(&self) -> &[ScanResult] {
        &self.results
    }

    /// Get results sorted by signal strength
    pub fn results_by_signal(&self) -> Vec<&ScanResult> {
        let mut sorted: Vec<_> = self.results.iter().collect();
        sorted.sort_by(|a, b| b.network.signal_strength.cmp(&a.network.signal_strength));
        sorted
    }

    /// Filter results by SSID
    pub fn find_by_ssid(&self, ssid: &str) -> Option<&ScanResult> {
        self.results.iter()
            .filter(|r| r.network.ssid == ssid)
            .max_by_key(|r| r.network.signal_strength)
    }

    /// Get best result (strongest signal)
    pub fn best_result(&self) -> Option<&ScanResult> {
        self.results.iter()
            .max_by_key(|r| r.network.signal_strength)
    }

    /// Remove old results
    pub fn prune_old(&mut self, max_age: u64, current_time: u64) {
        self.results.retain(|r| r.age(current_time) < max_age);
    }

    /// Set home channel
    pub fn set_home_channel(&mut self, channel: WifiChannel) {
        self.home_channel = Some(channel);
    }

    /// Get home channel
    pub fn home_channel(&self) -> Option<&WifiChannel> {
        self.home_channel.as_ref()
    }
}

impl Default for Scanner {
    fn default() -> Self {
        Self::new()
    }
}

/// Get default channels to scan for 2.4 GHz band
pub fn default_2ghz_channels() -> Vec<WifiChannel> {
    // Channels 1, 6, 11 are non-overlapping
    // Include others for complete scan
    [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11].iter().map(|&ch| {
        WifiChannel {
            number: ch,
            frequency: 2407 + (ch as u32) * 5,
            width: ChannelWidth::Mhz20,
        }
    }).collect()
}

/// Get default channels to scan for 5 GHz band
pub fn default_5ghz_channels() -> Vec<WifiChannel> {
    // UNII-1 (36, 40, 44, 48)
    // UNII-2A (52, 56, 60, 64)
    // UNII-2C (100, 104, 108, 112, 116, 120, 124, 128, 132, 136, 140, 144)
    // UNII-3 (149, 153, 157, 161, 165)
    let channels = [
        36, 40, 44, 48,
        52, 56, 60, 64,
        100, 104, 108, 112, 116, 120, 124, 128, 132, 136, 140, 144,
        149, 153, 157, 161, 165,
    ];

    channels.iter().map(|&ch| {
        WifiChannel {
            number: ch,
            frequency: 5000 + (ch as u32) * 5,
            width: ChannelWidth::Mhz20,
        }
    }).collect()
}

/// Get all default scan channels
pub fn default_scan_channels() -> Vec<WifiChannel> {
    let mut channels = default_2ghz_channels();
    channels.extend(default_5ghz_channels());
    channels
}

/// Regulatory information for a region
#[derive(Debug, Clone)]
pub struct RegulatoryInfo {
    /// Country code (e.g., "US", "BR", "EU")
    pub country_code: [u8; 2],
    /// Allowed 2.4 GHz channels
    pub channels_2ghz: Vec<u8>,
    /// Allowed 5 GHz channels
    pub channels_5ghz: Vec<u8>,
    /// Maximum TX power 2.4 GHz (dBm)
    pub max_power_2ghz: u8,
    /// Maximum TX power 5 GHz (dBm)
    pub max_power_5ghz: u8,
    /// DFS required for UNII-2 channels
    pub dfs_required: bool,
}

impl RegulatoryInfo {
    /// US regulatory domain
    pub fn us() -> Self {
        Self {
            country_code: *b"US",
            channels_2ghz: (1..=11).collect(),
            channels_5ghz: vec![
                36, 40, 44, 48, 52, 56, 60, 64,
                100, 104, 108, 112, 116, 120, 124, 128, 132, 136, 140, 144,
                149, 153, 157, 161, 165,
            ],
            max_power_2ghz: 30,
            max_power_5ghz: 30,
            dfs_required: true,
        }
    }

    /// Brazil regulatory domain
    pub fn br() -> Self {
        Self {
            country_code: *b"BR",
            channels_2ghz: (1..=13).collect(),
            channels_5ghz: vec![
                36, 40, 44, 48, 52, 56, 60, 64,
                100, 104, 108, 112, 116, 120, 124, 128, 132, 136, 140, 144,
                149, 153, 157, 161, 165,
            ],
            max_power_2ghz: 20,
            max_power_5ghz: 23,
            dfs_required: true,
        }
    }

    /// World regulatory domain (most restrictive)
    pub fn world() -> Self {
        Self {
            country_code: *b"00",
            channels_2ghz: (1..=11).collect(),
            channels_5ghz: vec![36, 40, 44, 48],
            max_power_2ghz: 20,
            max_power_5ghz: 20,
            dfs_required: false,
        }
    }

    /// Check if channel is allowed
    pub fn is_channel_allowed(&self, channel: u8) -> bool {
        self.channels_2ghz.contains(&channel) ||
        self.channels_5ghz.contains(&channel)
    }
}

/// Global regulatory info (lazily initialized)
static REGULATORY: IrqSafeMutex<Option<RegulatoryInfo>> = IrqSafeMutex::new(None);

/// Set regulatory domain
pub fn set_regulatory(info: RegulatoryInfo) {
    *REGULATORY.lock() = Some(info);
}

/// Get current regulatory info
pub fn get_regulatory() -> RegulatoryInfo {
    let mut guard = REGULATORY.lock();
    if guard.is_none() {
        *guard = Some(RegulatoryInfo::world());
    }
    guard.clone().unwrap()
}

/// Get allowed channels for current regulatory domain
pub fn get_allowed_channels() -> Vec<WifiChannel> {
    let reg = get_regulatory();
    let mut channels = Vec::new();

    for &ch in &reg.channels_2ghz {
        channels.push(WifiChannel {
            number: ch,
            frequency: 2407 + (ch as u32) * 5,
            width: ChannelWidth::Mhz20,
        });
    }

    for &ch in &reg.channels_5ghz {
        channels.push(WifiChannel {
            number: ch,
            frequency: 5000 + (ch as u32) * 5,
            width: ChannelWidth::Mhz20,
        });
    }

    channels
}
