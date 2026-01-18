//! WiFi 6E (6 GHz Band) Support
//!
//! Implements support for the 6 GHz frequency band introduced with 802.11ax (WiFi 6E).
//! The 6 GHz band provides:
//! - 1200 MHz of additional spectrum
//! - Less interference (new band, fewer legacy devices)
//! - 160 MHz and 320 MHz (WiFi 7) channels
//! - Lower latency due to mandatory WPA3 and OFDMA
//!
//! Regulatory Notes:
//! - FCC (US): 5925-7125 MHz (1200 MHz)
//! - ETSI (EU): 5925-6425 MHz (500 MHz, indoor-only for some)
//! - AFC (Automated Frequency Coordination) required for outdoor use in some regions

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use crate::util::{KResult, KError};
use crate::sync::IrqSafeMutex;

// ============================================================================
// 6 GHz Channel Definitions
// ============================================================================

/// 6 GHz channel information
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Channel6Ghz {
    /// Channel number (1-233)
    pub number: u8,
    /// Center frequency in MHz
    pub frequency: u32,
    /// Channel width
    pub width: ChannelWidth6Ghz,
    /// Operating class
    pub operating_class: u8,
    /// Is PSC (Preferred Scanning Channel)
    pub is_psc: bool,
}

impl Channel6Ghz {
    /// Create a 20 MHz channel
    pub const fn channel_20mhz(number: u8) -> Self {
        let frequency = 5950 + (number as u32) * 5;
        let is_psc = matches!(number, 5 | 21 | 37 | 53 | 69 | 85 | 101 | 117 | 133 | 149 | 165 | 181 | 197 | 213 | 229);
        Self {
            number,
            frequency,
            width: ChannelWidth6Ghz::Mhz20,
            operating_class: 131,
            is_psc,
        }
    }

    /// Create a 40 MHz channel
    pub const fn channel_40mhz(number: u8) -> Self {
        let frequency = 5950 + (number as u32) * 5;
        Self {
            number,
            frequency,
            width: ChannelWidth6Ghz::Mhz40,
            operating_class: 132,
            is_psc: false,
        }
    }

    /// Create an 80 MHz channel
    pub const fn channel_80mhz(number: u8) -> Self {
        let frequency = 5950 + (number as u32) * 5;
        Self {
            number,
            frequency,
            width: ChannelWidth6Ghz::Mhz80,
            operating_class: 133,
            is_psc: false,
        }
    }

    /// Create a 160 MHz channel
    pub const fn channel_160mhz(number: u8) -> Self {
        let frequency = 5950 + (number as u32) * 5;
        Self {
            number,
            frequency,
            width: ChannelWidth6Ghz::Mhz160,
            operating_class: 134,
            is_psc: false,
        }
    }

    /// Create a 320 MHz channel (WiFi 7)
    pub const fn channel_320mhz(number: u8) -> Self {
        let frequency = 5950 + (number as u32) * 5;
        Self {
            number,
            frequency,
            width: ChannelWidth6Ghz::Mhz320,
            operating_class: 137, // Draft value
            is_psc: false,
        }
    }

    /// Get all PSC (Preferred Scanning Channels) for 20 MHz
    pub fn psc_channels() -> Vec<Self> {
        PSC_CHANNELS_20MHZ.iter().map(|&n| Self::channel_20mhz(n)).collect()
    }

    /// Get all 20 MHz channels in UNII-5 (5925-6425 MHz)
    pub fn unii5_channels() -> Vec<Self> {
        (1u8..=93).step_by(4).map(Self::channel_20mhz).collect()
    }

    /// Get all 20 MHz channels in UNII-6 (6425-6525 MHz)
    pub fn unii6_channels() -> Vec<Self> {
        (97u8..=117).step_by(4).map(Self::channel_20mhz).collect()
    }

    /// Get all 20 MHz channels in UNII-7 (6525-6875 MHz)
    pub fn unii7_channels() -> Vec<Self> {
        (121u8..=189).step_by(4).map(Self::channel_20mhz).collect()
    }

    /// Get all 20 MHz channels in UNII-8 (6875-7125 MHz)
    pub fn unii8_channels() -> Vec<Self> {
        (193u8..=233).step_by(4).map(Self::channel_20mhz).collect()
    }

    /// Get all 20 MHz channels (full band)
    pub fn all_20mhz_channels() -> Vec<Self> {
        let mut channels = Vec::new();
        channels.extend(Self::unii5_channels());
        channels.extend(Self::unii6_channels());
        channels.extend(Self::unii7_channels());
        channels.extend(Self::unii8_channels());
        channels
    }

    /// Get all 160 MHz channels
    pub fn all_160mhz_channels() -> Vec<Self> {
        // 160 MHz channels: 15, 47, 79, 111, 143, 175, 207
        vec![
            Self::channel_160mhz(15),
            Self::channel_160mhz(47),
            Self::channel_160mhz(79),
            Self::channel_160mhz(111),
            Self::channel_160mhz(143),
            Self::channel_160mhz(175),
            Self::channel_160mhz(207),
        ]
    }

    /// Get all 320 MHz channels (WiFi 7)
    pub fn all_320mhz_channels() -> Vec<Self> {
        // 320 MHz channels: 31, 95, 159, 191
        vec![
            Self::channel_320mhz(31),
            Self::channel_320mhz(95),
            Self::channel_320mhz(159),
            Self::channel_320mhz(191),
        ]
    }

    /// Get bandwidth in MHz
    pub fn bandwidth_mhz(&self) -> u16 {
        self.width.mhz()
    }

    /// Check if channel is in UNII-5
    pub fn is_unii5(&self) -> bool {
        self.frequency >= 5925 && self.frequency <= 6425
    }

    /// Check if channel is in UNII-6
    pub fn is_unii6(&self) -> bool {
        self.frequency > 6425 && self.frequency <= 6525
    }

    /// Check if channel is in UNII-7
    pub fn is_unii7(&self) -> bool {
        self.frequency > 6525 && self.frequency <= 6875
    }

    /// Check if channel is in UNII-8
    pub fn is_unii8(&self) -> bool {
        self.frequency > 6875 && self.frequency <= 7125
    }

    /// Get UNII band name
    pub fn unii_band(&self) -> &'static str {
        if self.is_unii5() { "UNII-5" }
        else if self.is_unii6() { "UNII-6" }
        else if self.is_unii7() { "UNII-7" }
        else if self.is_unii8() { "UNII-8" }
        else { "Unknown" }
    }
}

/// PSC (Preferred Scanning Channels) for 20 MHz in 6 GHz
pub const PSC_CHANNELS_20MHZ: [u8; 15] = [
    5, 21, 37, 53, 69, 85, 101, 117, 133, 149, 165, 181, 197, 213, 229
];

/// Channel width for 6 GHz
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelWidth6Ghz {
    /// 20 MHz
    Mhz20,
    /// 40 MHz
    Mhz40,
    /// 80 MHz
    Mhz80,
    /// 160 MHz
    Mhz160,
    /// 320 MHz (WiFi 7 / 802.11be)
    Mhz320,
}

impl ChannelWidth6Ghz {
    /// Get width in MHz
    pub fn mhz(&self) -> u16 {
        match self {
            Self::Mhz20 => 20,
            Self::Mhz40 => 40,
            Self::Mhz80 => 80,
            Self::Mhz160 => 160,
            Self::Mhz320 => 320,
        }
    }

    /// Get operating class
    pub fn operating_class(&self) -> u8 {
        match self {
            Self::Mhz20 => 131,
            Self::Mhz40 => 132,
            Self::Mhz80 => 133,
            Self::Mhz160 => 134,
            Self::Mhz320 => 137,
        }
    }
}

// ============================================================================
// Regulatory Domains
// ============================================================================

/// Regulatory domain for 6 GHz
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegulatoryDomain6Ghz {
    /// FCC (US) - 5925-7125 MHz
    Fcc,
    /// ETSI (EU) - 5925-6425 MHz (limited)
    Etsi,
    /// MIC (Japan) - varies
    Mic,
    /// IC (Canada) - similar to FCC
    Ic,
    /// ACMA (Australia)
    Acma,
    /// Other/Unknown
    Other,
}

impl RegulatoryDomain6Ghz {
    /// Get country code
    pub fn country_code(&self) -> &'static str {
        match self {
            Self::Fcc => "US",
            Self::Etsi => "EU",
            Self::Mic => "JP",
            Self::Ic => "CA",
            Self::Acma => "AU",
            Self::Other => "WW",
        }
    }

    /// Get allowed frequency range
    pub fn frequency_range(&self) -> (u32, u32) {
        match self {
            Self::Fcc => (5925, 7125),
            Self::Etsi => (5925, 6425), // Limited, more with AFC
            Self::Mic => (5925, 6425),
            Self::Ic => (5925, 7125),
            Self::Acma => (5925, 6425),
            Self::Other => (5925, 6425), // Conservative
        }
    }

    /// Check if AFC (Automated Frequency Coordination) is required
    pub fn requires_afc(&self) -> bool {
        matches!(self, Self::Fcc | Self::Etsi | Self::Ic)
    }

    /// Get maximum EIRP in dBm for low power indoor (LPI)
    pub fn max_eirp_lpi(&self) -> i32 {
        match self {
            Self::Fcc => 30,  // 1W
            Self::Etsi => 23, // 200mW
            Self::Mic => 23,
            Self::Ic => 30,
            Self::Acma => 24,
            Self::Other => 20,
        }
    }

    /// Get maximum EIRP in dBm for standard power (SP) with AFC
    pub fn max_eirp_sp(&self) -> i32 {
        match self {
            Self::Fcc => 36,  // 4W with AFC
            Self::Etsi => 23, // Limited
            Self::Mic => 23,
            Self::Ic => 36,
            Self::Acma => 24,
            Self::Other => 20,
        }
    }
}

/// Power type for 6 GHz operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerType6Ghz {
    /// Low Power Indoor (LPI) - no AFC required
    LowPowerIndoor,
    /// Standard Power (SP) - requires AFC
    StandardPower,
    /// Very Low Power (VLP) - portable devices
    VeryLowPower,
}

impl PowerType6Ghz {
    /// Get regulatory name
    pub fn name(&self) -> &'static str {
        match self {
            Self::LowPowerIndoor => "LPI",
            Self::StandardPower => "SP",
            Self::VeryLowPower => "VLP",
        }
    }

    /// Requires AFC?
    pub fn requires_afc(&self) -> bool {
        matches!(self, Self::StandardPower)
    }
}

// ============================================================================
// AFC (Automated Frequency Coordination)
// ============================================================================

/// AFC request for spectrum availability
#[derive(Debug, Clone)]
pub struct AfcRequest {
    /// Device serial number
    pub serial_number: String,
    /// Device location (latitude)
    pub latitude: f64,
    /// Device location (longitude)
    pub longitude: f64,
    /// Altitude in meters
    pub altitude: Option<f64>,
    /// Uncertainty in meters
    pub uncertainty: u32,
    /// Requested channels
    pub requested_channels: Vec<Channel6Ghz>,
    /// Min desired EIRP in dBm
    pub min_eirp: i32,
    /// Operating class
    pub operating_class: u8,
}

impl AfcRequest {
    /// Create a new AFC request
    pub fn new(serial: &str, lat: f64, lon: f64) -> Self {
        Self {
            serial_number: String::from(serial),
            latitude: lat,
            longitude: lon,
            altitude: None,
            uncertainty: 50, // 50 meters default
            requested_channels: Channel6Ghz::all_20mhz_channels(),
            min_eirp: 20,
            operating_class: 131,
        }
    }

    /// Set altitude
    pub fn with_altitude(mut self, alt: f64) -> Self {
        self.altitude = Some(alt);
        self
    }

    /// Set uncertainty
    pub fn with_uncertainty(mut self, unc: u32) -> Self {
        self.uncertainty = unc;
        self
    }

    /// Request specific channels
    pub fn with_channels(mut self, channels: Vec<Channel6Ghz>) -> Self {
        if let Some(ch) = channels.first() {
            self.operating_class = ch.operating_class;
        }
        self.requested_channels = channels;
        self
    }
}

/// AFC response with available spectrum
#[derive(Debug, Clone)]
pub struct AfcResponse {
    /// Response time (Unix timestamp)
    pub response_time: u64,
    /// Expiry time (Unix timestamp)
    pub expiry_time: u64,
    /// Available channels with their max EIRP
    pub available_channels: Vec<AvailableChannel>,
    /// Response code
    pub response_code: AfcResponseCode,
}

/// Available channel from AFC
#[derive(Debug, Clone, Copy)]
pub struct AvailableChannel {
    /// Channel info
    pub channel: Channel6Ghz,
    /// Maximum allowed EIRP in dBm
    pub max_eirp: i32,
}

/// AFC response codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AfcResponseCode {
    /// Success
    Success,
    /// General failure
    GeneralFailure,
    /// Invalid request
    InvalidRequest,
    /// Unsupported version
    UnsupportedVersion,
    /// Device not allowed
    DeviceNotAllowed,
    /// Location outside coverage
    LocationOutsideCoverage,
}

// ============================================================================
// 6 GHz Discovery
// ============================================================================

/// 6 GHz discovery method
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoveryMethod {
    /// Passive scanning (listen for beacons)
    Passive,
    /// PSC scanning (scan only Preferred Scanning Channels)
    PscOnly,
    /// Out-of-band discovery (via 2.4/5 GHz RNR element)
    OutOfBand,
    /// FILS discovery (fast initial link setup)
    Fils,
    /// Unsolicited probe response (UPR)
    UnsolicitedProbe,
}

/// Reduced Neighbor Report (RNR) element for 6 GHz discovery
#[derive(Debug, Clone)]
pub struct ReducedNeighborReport {
    /// Reported neighbors
    pub neighbors: Vec<RnrNeighbor>,
}

/// RNR neighbor entry
#[derive(Debug, Clone)]
pub struct RnrNeighbor {
    /// Operating class
    pub operating_class: u8,
    /// Channel number
    pub channel: u8,
    /// BSSID
    pub bssid: [u8; 6],
    /// Short SSID (CRC-32 of SSID)
    pub short_ssid: u32,
    /// BSS parameters
    pub bss_params: u8,
    /// 20 MHz PSD (power spectral density)
    pub psd_20mhz: i8,
}

impl ReducedNeighborReport {
    /// Parse RNR from element data
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 5 {
            return None;
        }

        let mut neighbors = Vec::new();
        let mut pos = 0;

        while pos + 13 <= data.len() {
            let tbtt_header = data[pos];
            let _tbtt_length = (tbtt_header >> 4) & 0x0f;
            let _tbtt_count = tbtt_header & 0x0f;

            let operating_class = data[pos + 1];
            let channel = data[pos + 2];

            // Parse TBTT info (simplified)
            let bssid: [u8; 6] = data[pos + 3..pos + 9].try_into().ok()?;
            let short_ssid = u32::from_le_bytes(data[pos + 9..pos + 13].try_into().ok()?);

            neighbors.push(RnrNeighbor {
                operating_class,
                channel,
                bssid,
                short_ssid,
                bss_params: 0,
                psd_20mhz: 0,
            });

            pos += 13;
        }

        Some(Self { neighbors })
    }

    /// Get 6 GHz neighbors only
    pub fn sixghz_neighbors(&self) -> Vec<&RnrNeighbor> {
        self.neighbors.iter()
            .filter(|n| n.operating_class >= 131 && n.operating_class <= 137)
            .collect()
    }
}

// ============================================================================
// 802.11ax HE Capabilities for 6 GHz
// ============================================================================

/// HE (High Efficiency) 6 GHz capabilities
#[derive(Debug, Clone, Copy, Default)]
pub struct He6GhzCapabilities {
    /// Minimum MPDU start spacing
    pub min_mpdu_start_spacing: u8,
    /// Maximum A-MPDU length exponent
    pub max_ampdu_length_exp: u8,
    /// Maximum MPDU length
    pub max_mpdu_length: u16,
    /// SM power save
    pub sm_power_save: SmPowerSave,
    /// RD responder
    pub rd_responder: bool,
    /// Rx antenna pattern consistency
    pub rx_antenna_pattern: bool,
    /// Tx antenna pattern consistency
    pub tx_antenna_pattern: bool,
}

impl He6GhzCapabilities {
    /// Parse from 6 GHz Band Capabilities element
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 2 {
            return None;
        }

        let caps = u16::from_le_bytes([data[0], data[1]]);

        Some(Self {
            min_mpdu_start_spacing: (caps & 0x07) as u8,
            max_ampdu_length_exp: ((caps >> 3) & 0x07) as u8,
            max_mpdu_length: match (caps >> 6) & 0x03 {
                0 => 3895,
                1 => 7991,
                2 => 11454,
                _ => 11454,
            },
            sm_power_save: SmPowerSave::from_u8(((caps >> 9) & 0x03) as u8),
            rd_responder: (caps >> 11) & 0x01 != 0,
            rx_antenna_pattern: (caps >> 12) & 0x01 != 0,
            tx_antenna_pattern: (caps >> 13) & 0x01 != 0,
        })
    }

    /// Encode to bytes
    pub fn to_bytes(&self) -> [u8; 2] {
        let mut caps: u16 = 0;
        caps |= (self.min_mpdu_start_spacing as u16) & 0x07;
        caps |= ((self.max_ampdu_length_exp as u16) & 0x07) << 3;
        caps |= (match self.max_mpdu_length {
            3895 => 0,
            7991 => 1,
            11454 => 2,
            _ => 2,
        }) << 6;
        caps |= (self.sm_power_save.to_u8() as u16) << 9;
        if self.rd_responder { caps |= 1 << 11; }
        if self.rx_antenna_pattern { caps |= 1 << 12; }
        if self.tx_antenna_pattern { caps |= 1 << 13; }

        caps.to_le_bytes()
    }
}

/// SM (Spatial Multiplexing) power save mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SmPowerSave {
    /// Static SM power save
    Static,
    /// Dynamic SM power save
    Dynamic,
    /// SM power save disabled
    #[default]
    Disabled,
}

impl SmPowerSave {
    fn from_u8(val: u8) -> Self {
        match val {
            0 => Self::Static,
            1 => Self::Dynamic,
            _ => Self::Disabled,
        }
    }

    fn to_u8(&self) -> u8 {
        match self {
            Self::Static => 0,
            Self::Dynamic => 1,
            Self::Disabled => 3,
        }
    }
}

// ============================================================================
// 6 GHz Operation
// ============================================================================

/// 6 GHz band operation element
#[derive(Debug, Clone, Copy)]
pub struct Operation6Ghz {
    /// Primary channel
    pub primary_channel: u8,
    /// Channel width
    pub channel_width: ChannelWidth6Ghz,
    /// Duplicate beacon
    pub duplicate_beacon: bool,
    /// Regulatory info
    pub regulatory_info: u8,
    /// Center frequency segment 0
    pub center_freq_seg0: u8,
    /// Center frequency segment 1 (for 160/320 MHz)
    pub center_freq_seg1: u8,
    /// Minimum rate
    pub min_rate: u8,
}

impl Operation6Ghz {
    /// Parse from HE Operation 6 GHz Information field
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 5 {
            return None;
        }

        let primary = data[0];
        let control = data[1];
        let seg0 = data[2];
        let seg1 = data[3];
        let min_rate = data[4];

        let width = match (control >> 0) & 0x03 {
            0 => ChannelWidth6Ghz::Mhz20,
            1 => ChannelWidth6Ghz::Mhz40,
            2 => ChannelWidth6Ghz::Mhz80,
            3 => if seg1 != 0 { ChannelWidth6Ghz::Mhz160 } else { ChannelWidth6Ghz::Mhz80 },
            _ => ChannelWidth6Ghz::Mhz20,
        };

        Some(Self {
            primary_channel: primary,
            channel_width: width,
            duplicate_beacon: (control >> 2) & 0x01 != 0,
            regulatory_info: (control >> 3) & 0x07,
            center_freq_seg0: seg0,
            center_freq_seg1: seg1,
            min_rate,
        })
    }

    /// Get center frequency in MHz
    pub fn center_frequency(&self) -> u32 {
        5950 + (self.center_freq_seg0 as u32) * 5
    }
}

// ============================================================================
// 6 GHz Manager
// ============================================================================

/// WiFi 6 GHz band manager
pub struct Wifi6GhzManager {
    /// Regulatory domain
    regulatory: RegulatoryDomain6Ghz,
    /// Power type
    power_type: PowerType6Ghz,
    /// AFC response (if using SP)
    afc_response: Option<AfcResponse>,
    /// Discovered 6 GHz networks
    discovered_networks: Vec<Discovered6GhzNetwork>,
    /// Supported channels based on regulatory and AFC
    supported_channels: Vec<Channel6Ghz>,
    /// Initialized
    initialized: bool,
}

/// Discovered 6 GHz network
#[derive(Debug, Clone)]
pub struct Discovered6GhzNetwork {
    /// BSSID
    pub bssid: [u8; 6],
    /// SSID
    pub ssid: String,
    /// Channel
    pub channel: Channel6Ghz,
    /// Signal strength (dBm)
    pub rssi: i8,
    /// Discovery method
    pub discovery_method: DiscoveryMethod,
    /// HE capabilities
    pub he_caps: Option<He6GhzCapabilities>,
    /// Timestamp
    pub discovered_at: u64,
}

impl Wifi6GhzManager {
    /// Create a new 6 GHz manager
    pub fn new(regulatory: RegulatoryDomain6Ghz) -> Self {
        let supported = Self::get_supported_channels(&regulatory, PowerType6Ghz::LowPowerIndoor, None);

        Self {
            regulatory,
            power_type: PowerType6Ghz::LowPowerIndoor,
            afc_response: None,
            discovered_networks: Vec::new(),
            supported_channels: supported,
            initialized: false,
        }
    }

    /// Initialize the manager
    pub fn init(&mut self) {
        self.initialized = true;
        crate::kprintln!("wifi6e: initialized for {} domain", self.regulatory.country_code());
    }

    /// Set power type
    pub fn set_power_type(&mut self, power_type: PowerType6Ghz) {
        self.power_type = power_type;
        self.update_supported_channels();
    }

    /// Process AFC response
    pub fn process_afc_response(&mut self, response: AfcResponse) {
        if response.response_code == AfcResponseCode::Success {
            self.afc_response = Some(response);
            self.update_supported_channels();
        }
    }

    /// Update supported channels based on regulatory and AFC
    fn update_supported_channels(&mut self) {
        self.supported_channels = Self::get_supported_channels(
            &self.regulatory,
            self.power_type,
            self.afc_response.as_ref(),
        );
    }

    /// Get supported channels
    fn get_supported_channels(
        regulatory: &RegulatoryDomain6Ghz,
        power_type: PowerType6Ghz,
        afc: Option<&AfcResponse>,
    ) -> Vec<Channel6Ghz> {
        let (min_freq, max_freq) = regulatory.frequency_range();

        // For standard power with AFC, use AFC response
        if power_type == PowerType6Ghz::StandardPower {
            if let Some(afc_resp) = afc {
                return afc_resp.available_channels.iter()
                    .map(|ac| ac.channel)
                    .collect();
            }
            // No AFC response, no SP channels
            return Vec::new();
        }

        // For LPI/VLP, use regulatory-defined channels
        Channel6Ghz::all_20mhz_channels()
            .into_iter()
            .filter(|ch| ch.frequency >= min_freq && ch.frequency <= max_freq)
            .collect()
    }

    /// Get PSC channels for scanning
    pub fn psc_channels(&self) -> Vec<Channel6Ghz> {
        self.supported_channels.iter()
            .filter(|ch| ch.is_psc)
            .copied()
            .collect()
    }

    /// Process RNR from 2.4/5 GHz beacon
    pub fn process_rnr(&mut self, rnr: &ReducedNeighborReport) {
        for neighbor in rnr.sixghz_neighbors() {
            let channel = Channel6Ghz::channel_20mhz(neighbor.channel);

            // Check if channel is supported
            if !self.supported_channels.iter().any(|c| c.number == channel.number) {
                continue;
            }

            // Add discovered network
            let network = Discovered6GhzNetwork {
                bssid: neighbor.bssid,
                ssid: String::new(), // Unknown from RNR alone
                channel,
                rssi: -100, // Unknown
                discovery_method: DiscoveryMethod::OutOfBand,
                he_caps: None,
                discovered_at: crate::time::ticks(),
            };

            self.discovered_networks.push(network);
        }
    }

    /// Add discovered network from beacon/probe response
    pub fn add_discovered(&mut self, network: Discovered6GhzNetwork) {
        // Update existing or add new
        if let Some(existing) = self.discovered_networks.iter_mut()
            .find(|n| n.bssid == network.bssid)
        {
            existing.rssi = network.rssi;
            existing.ssid = network.ssid.clone();
            existing.he_caps = network.he_caps;
            existing.discovered_at = network.discovered_at;
        } else {
            self.discovered_networks.push(network);
        }
    }

    /// Get discovered networks
    pub fn discovered_networks(&self) -> &[Discovered6GhzNetwork] {
        &self.discovered_networks
    }

    /// Clear discovered networks
    pub fn clear_discovered(&mut self) {
        self.discovered_networks.clear();
    }

    /// Get supported channels
    pub fn supported_channels(&self) -> &[Channel6Ghz] {
        &self.supported_channels
    }

    /// Get current regulatory domain
    pub fn regulatory_domain(&self) -> RegulatoryDomain6Ghz {
        self.regulatory
    }

    /// Get current power type
    pub fn power_type(&self) -> PowerType6Ghz {
        self.power_type
    }

    /// Get maximum EIRP for current configuration
    pub fn max_eirp(&self) -> i32 {
        match self.power_type {
            PowerType6Ghz::LowPowerIndoor => self.regulatory.max_eirp_lpi(),
            PowerType6Ghz::StandardPower => {
                if let Some(ref afc) = self.afc_response {
                    afc.available_channels.iter()
                        .map(|c| c.max_eirp)
                        .max()
                        .unwrap_or(self.regulatory.max_eirp_sp())
                } else {
                    0 // No AFC, no SP allowed
                }
            }
            PowerType6Ghz::VeryLowPower => 14, // Typically 14 dBm for VLP
        }
    }

    /// Check if 6 GHz is enabled
    pub fn is_enabled(&self) -> bool {
        self.initialized && !self.supported_channels.is_empty()
    }

    /// Build HE 6 GHz Band Capabilities element
    pub fn build_6ghz_caps_element(&self) -> Vec<u8> {
        let caps = He6GhzCapabilities::default();
        let mut element = Vec::with_capacity(4);
        element.push(255); // Extension element ID
        element.push(59);  // HE 6 GHz Band Capabilities
        element.extend_from_slice(&caps.to_bytes());
        element
    }
}

impl Default for Wifi6GhzManager {
    fn default() -> Self {
        Self::new(RegulatoryDomain6Ghz::Other)
    }
}

// ============================================================================
// Global Manager
// ============================================================================

static WIFI6E_MANAGER: IrqSafeMutex<Option<Wifi6GhzManager>> = IrqSafeMutex::new(None);

/// Initialize WiFi 6E subsystem
pub fn init(country_code: &str) {
    let regulatory = match country_code {
        "US" => RegulatoryDomain6Ghz::Fcc,
        "CA" => RegulatoryDomain6Ghz::Ic,
        "EU" | "DE" | "FR" | "GB" | "IT" | "ES" => RegulatoryDomain6Ghz::Etsi,
        "JP" => RegulatoryDomain6Ghz::Mic,
        "AU" => RegulatoryDomain6Ghz::Acma,
        _ => RegulatoryDomain6Ghz::Other,
    };

    let mut manager = Wifi6GhzManager::new(regulatory);
    manager.init();
    *WIFI6E_MANAGER.lock() = Some(manager);
}

/// Check if 6 GHz is available
pub fn is_available() -> bool {
    WIFI6E_MANAGER.lock().as_ref()
        .map(|m| m.is_enabled())
        .unwrap_or(false)
}

/// Get PSC channels for scanning
pub fn psc_channels() -> Vec<Channel6Ghz> {
    WIFI6E_MANAGER.lock().as_ref()
        .map(|m| m.psc_channels())
        .unwrap_or_default()
}

/// Get all supported 6 GHz channels
pub fn supported_channels() -> Vec<Channel6Ghz> {
    WIFI6E_MANAGER.lock().as_ref()
        .map(|m| m.supported_channels().to_vec())
        .unwrap_or_default()
}

/// Process RNR element from 2.4/5 GHz beacon
pub fn process_rnr(data: &[u8]) {
    if let Some(rnr) = ReducedNeighborReport::parse(data) {
        if let Some(ref mut manager) = *WIFI6E_MANAGER.lock() {
            manager.process_rnr(&rnr);
        }
    }
}

/// Get discovered 6 GHz networks
pub fn discovered_networks() -> Vec<Discovered6GhzNetwork> {
    WIFI6E_MANAGER.lock().as_ref()
        .map(|m| m.discovered_networks().to_vec())
        .unwrap_or_default()
}

/// Set power type
pub fn set_power_type(power_type: PowerType6Ghz) {
    if let Some(ref mut manager) = *WIFI6E_MANAGER.lock() {
        manager.set_power_type(power_type);
    }
}

/// Get current max EIRP
pub fn max_eirp() -> i32 {
    WIFI6E_MANAGER.lock().as_ref()
        .map(|m| m.max_eirp())
        .unwrap_or(0)
}
