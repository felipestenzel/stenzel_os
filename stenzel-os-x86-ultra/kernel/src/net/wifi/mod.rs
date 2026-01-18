//! IEEE 802.11 WiFi Stack
//!
//! Implementation of the 802.11 wireless networking protocol.
//! Supports infrastructure mode (connecting to access points).

pub mod frame;
pub mod mac;
pub mod mlme;
pub mod scan;
pub mod crypto;
pub mod driver;
pub mod wpa;
pub mod wpa3;
pub mod connection;
pub mod hotspot;
pub mod wifi6e;

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::sync::Arc;
use alloc::collections::BTreeMap;
use crate::sync::IrqSafeMutex;
use crate::util::{KResult, KError};

// Re-export common types
pub use mac::MacAddress;
pub use driver::WifiDriver;

/// WiFi interface state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WifiState {
    /// Interface is down
    Down,
    /// Interface is up but not connected
    Disconnected,
    /// Scanning for networks
    Scanning,
    /// Authenticating with AP
    Authenticating,
    /// Associating with AP
    Associating,
    /// Connected to AP
    Connected,
    /// Disconnecting
    Disconnecting,
}

/// WiFi interface capabilities
#[derive(Debug, Clone)]
pub struct WifiCapabilities {
    /// Supported frequency bands
    pub bands: Vec<WifiBand>,
    /// Maximum scan SSIDs
    pub max_scan_ssids: u8,
    /// Maximum scheduled scan SSIDs
    pub max_sched_scan_ssids: u8,
    /// Signal type (0 = unspecified, 1 = dBm, 2 = arbitrary units)
    pub signal_type: u8,
    /// Supports AP mode
    pub supports_ap: bool,
    /// Supports P2P mode
    pub supports_p2p: bool,
    /// Supports monitor mode
    pub supports_monitor: bool,
    /// Supports mesh mode
    pub supports_mesh: bool,
}

impl Default for WifiCapabilities {
    fn default() -> Self {
        Self {
            bands: vec![WifiBand::Band2_4GHz],
            max_scan_ssids: 4,
            max_sched_scan_ssids: 0,
            signal_type: 1,
            supports_ap: false,
            supports_p2p: false,
            supports_monitor: false,
            supports_mesh: false,
        }
    }
}

/// WiFi frequency band
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WifiBand {
    /// 2.4 GHz (802.11b/g/n)
    Band2_4GHz,
    /// 5 GHz (802.11a/n/ac)
    Band5GHz,
    /// 6 GHz (802.11ax)
    Band6GHz,
}

/// WiFi channel
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WifiChannel {
    /// Channel number
    pub number: u8,
    /// Center frequency in MHz
    pub frequency: u32,
    /// Channel width
    pub width: ChannelWidth,
}

impl WifiChannel {
    /// Create a 2.4 GHz channel
    pub fn channel_2_4ghz(number: u8) -> Self {
        let frequency = 2407 + (number as u32) * 5;
        Self {
            number,
            frequency,
            width: ChannelWidth::Mhz20,
        }
    }

    /// Create a 5 GHz channel
    pub fn channel_5ghz(number: u8) -> Self {
        let frequency = 5000 + (number as u32) * 5;
        Self {
            number,
            frequency,
            width: ChannelWidth::Mhz20,
        }
    }

    /// Get band from frequency
    pub fn band(&self) -> WifiBand {
        if self.frequency < 3000 {
            WifiBand::Band2_4GHz
        } else if self.frequency < 6000 {
            WifiBand::Band5GHz
        } else {
            WifiBand::Band6GHz
        }
    }

    /// Is DFS channel
    pub fn is_dfs(&self) -> bool {
        // 5 GHz DFS channels: 52-144
        self.band() == WifiBand::Band5GHz &&
            self.number >= 52 && self.number <= 144
    }

    /// Get all 2.4 GHz channels
    pub fn all_2_4ghz() -> Vec<Self> {
        (1..=14).map(Self::channel_2_4ghz).collect()
    }

    /// Get all 5 GHz channels
    pub fn all_5ghz() -> Vec<Self> {
        let channels = [36, 40, 44, 48, 52, 56, 60, 64, 100, 104, 108, 112,
                       116, 120, 124, 128, 132, 136, 140, 144, 149, 153, 157, 161, 165];
        channels.iter().map(|&n| Self::channel_5ghz(n)).collect()
    }
}

/// Channel width
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelWidth {
    Mhz20,
    Mhz40,
    Mhz80,
    Mhz160,
}

/// Cipher suite for encryption
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CipherSuite {
    /// No encryption (open network)
    None,
    /// WEP 40-bit (deprecated, insecure)
    Wep40,
    /// WEP 104-bit (deprecated, insecure)
    Wep104,
    /// TKIP (WPA)
    Tkip,
    /// CCMP/AES (WPA2)
    Ccmp,
    /// GCMP (WPA3)
    Gcmp,
}

/// Authentication and Key Management (AKM) suite
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AkmSuite {
    /// Open (no authentication)
    Open,
    /// Pre-shared key (PSK)
    Psk,
    /// 802.1X/EAP
    Eap,
    /// SAE (WPA3)
    Sae,
    /// OWE (WPA3)
    Owe,
}

/// Security type for a network
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityType {
    /// Open network
    Open,
    /// WEP (deprecated)
    Wep,
    /// WPA-PSK
    WpaPsk,
    /// WPA2-PSK
    Wpa2Psk,
    /// WPA3-SAE
    Wpa3Sae,
    /// WPA/WPA2 Enterprise (802.1X)
    Enterprise,
}

/// WiFi network (BSS) information
#[derive(Debug, Clone)]
pub struct WifiNetwork {
    /// BSSID (MAC address of AP)
    pub bssid: MacAddress,
    /// SSID (network name)
    pub ssid: String,
    /// Channel
    pub channel: Option<WifiChannel>,
    /// Signal strength (dBm)
    pub signal_strength: i8,
    /// Security type
    pub security: SecurityType,
    /// RSN (WPA2) information
    pub rsn_info: Option<RsnInfo>,
    /// WPA information
    pub wpa_info: Option<WpaInfo>,
}

impl WifiNetwork {
    /// Create a new network entry
    pub fn new(bssid: MacAddress, ssid: String, channel: Option<WifiChannel>) -> Self {
        Self {
            bssid,
            ssid,
            channel,
            signal_strength: -100,
            security: SecurityType::Open,
            rsn_info: None,
            wpa_info: None,
        }
    }

    /// Calculate signal quality (0-100%)
    pub fn quality(&self) -> u8 {
        // Convert dBm to percentage
        // -100 dBm = 0%, -50 dBm = 100%
        if self.signal_strength <= -100 {
            0
        } else if self.signal_strength >= -50 {
            100
        } else {
            ((self.signal_strength + 100) * 2) as u8
        }
    }

    /// Check if network uses encryption
    pub fn is_encrypted(&self) -> bool {
        self.security != SecurityType::Open
    }
}

/// RSN (Robust Security Network) information element
#[derive(Debug, Clone)]
pub struct RsnInfo {
    /// Version
    pub version: u16,
    /// Group cipher suite
    pub group_cipher: CipherSuite,
    /// Pairwise cipher suites
    pub pairwise_ciphers: Vec<CipherSuite>,
    /// AKM suites
    pub akm_suites: Vec<AkmSuite>,
    /// RSN capabilities
    pub capabilities: u16,
}

/// WPA information element
#[derive(Debug, Clone)]
pub struct WpaInfo {
    /// Version
    pub version: u16,
    /// Group cipher suite
    pub group_cipher: CipherSuite,
    /// Pairwise cipher suites
    pub pairwise_ciphers: Vec<CipherSuite>,
    /// AKM suites
    pub akm_suites: Vec<AkmSuite>,
}

/// WiFi interface
pub struct WifiInterface {
    /// Interface name
    pub name: String,
    /// MAC address
    pub mac: MacAddress,
    /// Current state
    pub state: WifiState,
    /// Capabilities
    pub capabilities: WifiCapabilities,
    /// Current channel
    pub channel: Option<WifiChannel>,
    /// Connected network
    pub connected_network: Option<WifiNetwork>,
    /// Scan results
    pub scan_results: Vec<WifiNetwork>,
    /// Driver
    driver: Option<Arc<dyn WifiDriver>>,
}

impl WifiInterface {
    /// Create a new WiFi interface
    pub fn new(name: &str, mac: MacAddress) -> Self {
        Self {
            name: String::from(name),
            mac,
            state: WifiState::Down,
            capabilities: WifiCapabilities::default(),
            channel: None,
            connected_network: None,
            scan_results: Vec::new(),
            driver: None,
        }
    }

    /// Set the driver
    pub fn set_driver(&mut self, driver: Arc<dyn WifiDriver>) {
        self.capabilities = driver.capabilities();
        self.mac = driver.mac_address();
        self.driver = Some(driver);
    }

    /// Bring interface up
    pub fn up(&mut self) -> KResult<()> {
        if let Some(driver) = &self.driver {
            driver.power_on()?;
            self.state = WifiState::Disconnected;
            Ok(())
        } else {
            Err(KError::NotSupported)
        }
    }

    /// Bring interface down
    pub fn down(&mut self) -> KResult<()> {
        if let Some(driver) = &self.driver {
            driver.power_off()?;
            self.state = WifiState::Down;
            self.connected_network = None;
            Ok(())
        } else {
            Err(KError::NotSupported)
        }
    }

    /// Start scanning for networks
    pub fn scan(&mut self) -> KResult<()> {
        if self.state == WifiState::Down {
            return Err(KError::Invalid);
        }

        if let Some(driver) = &self.driver {
            self.state = WifiState::Scanning;
            self.scan_results.clear();
            driver.start_scan()?;
            Ok(())
        } else {
            Err(KError::NotSupported)
        }
    }

    /// Get scan results
    pub fn get_scan_results(&self) -> &[WifiNetwork] {
        &self.scan_results
    }

    /// Connect to a network
    pub fn connect(&mut self, ssid: &str, password: Option<&str>) -> KResult<()> {
        if self.state == WifiState::Down {
            return Err(KError::Invalid);
        }

        // Find network in scan results
        let network = self.scan_results.iter()
            .find(|n| n.ssid == ssid)
            .cloned()
            .ok_or(KError::NotFound)?;

        // Check if password is required
        if network.is_encrypted() && password.is_none() {
            return Err(KError::Invalid);
        }

        if let Some(driver) = &self.driver {
            self.state = WifiState::Authenticating;

            // Set channel
            if let Some(ref channel) = network.channel {
                driver.set_channel(channel)?;
                self.channel = Some(channel.clone());
            }

            // Start connection
            driver.connect(&network, password)?;

            self.connected_network = Some(network);
            self.state = WifiState::Connected;
            Ok(())
        } else {
            Err(KError::NotSupported)
        }
    }

    /// Disconnect from current network
    pub fn disconnect(&mut self) -> KResult<()> {
        if self.state != WifiState::Connected {
            return Err(KError::Invalid);
        }

        if let Some(driver) = &self.driver {
            self.state = WifiState::Disconnecting;
            driver.disconnect()?;
            self.connected_network = None;
            self.state = WifiState::Disconnected;
            Ok(())
        } else {
            Err(KError::NotSupported)
        }
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        self.state == WifiState::Connected
    }

    /// Get connected SSID
    pub fn connected_ssid(&self) -> Option<&str> {
        self.connected_network.as_ref().map(|n| n.ssid.as_str())
    }

    /// Get signal strength of current connection
    pub fn signal_strength(&self) -> Option<i8> {
        if let Some(driver) = &self.driver {
            driver.get_rssi()
        } else {
            self.connected_network.as_ref().map(|n| n.signal_strength)
        }
    }
}

/// Global WiFi manager
struct WifiManager {
    /// Registered interfaces
    interfaces: BTreeMap<String, WifiInterface>,
    /// Default interface name
    default_interface: Option<String>,
}

impl WifiManager {
    const fn new() -> Self {
        Self {
            interfaces: BTreeMap::new(),
            default_interface: None,
        }
    }

    /// Register a WiFi interface
    fn register(&mut self, iface: WifiInterface) {
        let name = iface.name.clone();
        if self.default_interface.is_none() {
            self.default_interface = Some(name.clone());
        }
        self.interfaces.insert(name, iface);
    }

    /// Unregister a WiFi interface
    fn unregister(&mut self, name: &str) {
        self.interfaces.remove(name);
        if self.default_interface.as_ref().map(|n| n == name).unwrap_or(false) {
            self.default_interface = self.interfaces.keys().next().cloned();
        }
    }

    /// Get interface by name
    fn get(&self, name: &str) -> Option<&WifiInterface> {
        self.interfaces.get(name)
    }

    /// Get mutable interface by name
    fn get_mut(&mut self, name: &str) -> Option<&mut WifiInterface> {
        self.interfaces.get_mut(name)
    }

    /// Get default interface
    fn default(&self) -> Option<&WifiInterface> {
        self.default_interface.as_ref().and_then(|n| self.interfaces.get(n))
    }

    /// Get mutable default interface
    fn default_mut(&mut self) -> Option<&mut WifiInterface> {
        if let Some(name) = &self.default_interface {
            self.interfaces.get_mut(name)
        } else {
            None
        }
    }

    /// List all interfaces
    fn list(&self) -> Vec<&str> {
        self.interfaces.keys().map(|s| s.as_str()).collect()
    }
}

/// Global WiFi manager instance
static WIFI_MANAGER: IrqSafeMutex<WifiManager> = IrqSafeMutex::new(WifiManager::new());

/// Initialize WiFi subsystem
pub fn init() {
    crate::kprintln!("wifi: initializing 802.11 stack");

    // Initialize regulatory database
    scan::set_regulatory(scan::RegulatoryInfo::br());

    crate::kprintln!("wifi: stack ready");
}

/// Register a WiFi interface
pub fn register_interface(iface: WifiInterface) {
    let name = iface.name.clone();
    WIFI_MANAGER.lock().register(iface);
    crate::kprintln!("wifi: registered interface {}", name);
}

/// Unregister a WiFi interface
pub fn unregister_interface(name: &str) {
    WIFI_MANAGER.lock().unregister(name);
    crate::kprintln!("wifi: unregistered interface {}", name);
}

/// Get list of WiFi interfaces
pub fn list_interfaces() -> Vec<String> {
    WIFI_MANAGER.lock().list().iter().map(|s| String::from(*s)).collect()
}

/// Scan for networks on default interface
pub fn scan() -> KResult<()> {
    WIFI_MANAGER.lock().default_mut()
        .ok_or(KError::NotFound)?
        .scan()
}

/// Get scan results from default interface
pub fn scan_results() -> Vec<WifiNetwork> {
    WIFI_MANAGER.lock().default()
        .map(|i| i.scan_results.clone())
        .unwrap_or_default()
}

/// Connect to a network on default interface
pub fn connect(ssid: &str, password: Option<&str>) -> KResult<()> {
    WIFI_MANAGER.lock().default_mut()
        .ok_or(KError::NotFound)?
        .connect(ssid, password)
}

/// Disconnect from current network on default interface
pub fn disconnect() -> KResult<()> {
    WIFI_MANAGER.lock().default_mut()
        .ok_or(KError::NotFound)?
        .disconnect()
}

/// Get connection status
pub fn is_connected() -> bool {
    WIFI_MANAGER.lock().default()
        .map(|i| i.is_connected())
        .unwrap_or(false)
}

/// Get connected SSID
pub fn connected_ssid() -> Option<String> {
    WIFI_MANAGER.lock().default()
        .and_then(|i| i.connected_ssid().map(String::from))
}

/// Get current signal strength
pub fn signal_strength() -> Option<i8> {
    WIFI_MANAGER.lock().default()
        .and_then(|i| i.signal_strength())
}

/// Bring interface up
pub fn up(name: Option<&str>) -> KResult<()> {
    let mut manager = WIFI_MANAGER.lock();
    let iface = if let Some(n) = name {
        manager.get_mut(n)
    } else {
        manager.default_mut()
    };
    iface.ok_or(KError::NotFound)?.up()
}

/// Bring interface down
pub fn down(name: Option<&str>) -> KResult<()> {
    let mut manager = WIFI_MANAGER.lock();
    let iface = if let Some(n) = name {
        manager.get_mut(n)
    } else {
        manager.default_mut()
    };
    iface.ok_or(KError::NotFound)?.down()
}

/// Get interface state
pub fn get_state(name: Option<&str>) -> Option<WifiState> {
    let manager = WIFI_MANAGER.lock();
    let iface = if let Some(n) = name {
        manager.get(n)
    } else {
        manager.default()
    };
    iface.map(|i| i.state)
}

/// Process WiFi events (call periodically)
pub fn poll() {
    // Process received frames from drivers
    let drivers = driver::active_drivers();
    for drv in drivers {
        while let Some(frame_data) = drv.recv_frame() {
            // Process frame through MLME
            // In a full implementation, this would update scan results,
            // handle authentication responses, etc.
            let _ = frame_data;
        }
    }
}

/// Scan configuration options
#[derive(Debug, Clone)]
pub struct ScanOptions {
    /// Specific SSID to scan for (None = all networks)
    pub ssid: Option<String>,
    /// Scan only specific channels (empty = all channels)
    pub channels: Vec<WifiChannel>,
    /// Active scan (send probe requests) vs passive (listen only)
    pub active: bool,
    /// Dwell time per channel in milliseconds
    pub dwell_time_ms: u64,
    /// Maximum scan duration in milliseconds (0 = unlimited)
    pub max_duration_ms: u64,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            ssid: None,
            channels: Vec::new(),
            active: true,
            dwell_time_ms: 100,
            max_duration_ms: 10000,
        }
    }
}

/// Start a scan with options
pub fn scan_with_options(options: &ScanOptions) -> KResult<()> {
    let mut manager = WIFI_MANAGER.lock();
    let iface = manager.default_mut().ok_or(KError::NotFound)?;

    if iface.state == WifiState::Down {
        return Err(KError::Invalid);
    }

    if let Some(ref driver) = iface.driver {
        iface.state = WifiState::Scanning;
        iface.scan_results.clear();

        // Set up channels
        let channels = if options.channels.is_empty() {
            scan::get_allowed_channels()
        } else {
            options.channels.clone()
        };

        // Start scanning each channel
        for channel in channels {
            driver.set_channel(&channel)?;
            driver.start_scan()?;
        }

        Ok(())
    } else {
        Err(KError::NotSupported)
    }
}

/// Scan for a specific network by SSID
pub fn scan_for_ssid(ssid: &str) -> KResult<Vec<WifiNetwork>> {
    let options = ScanOptions {
        ssid: Some(String::from(ssid)),
        active: true,
        ..Default::default()
    };
    scan_with_options(&options)?;

    // Wait for results
    core::hint::spin_loop();

    // Return matching results
    Ok(scan_results()
        .into_iter()
        .filter(|n| n.ssid == ssid)
        .collect())
}

/// Get best network for an SSID (strongest signal)
pub fn find_best_network(ssid: &str) -> Option<WifiNetwork> {
    let results = scan_results();
    results
        .into_iter()
        .filter(|n| n.ssid == ssid)
        .max_by_key(|n| n.signal_strength)
}

/// Check if scanning is in progress
pub fn is_scanning() -> bool {
    WIFI_MANAGER.lock().default()
        .map(|i| i.state == WifiState::Scanning)
        .unwrap_or(false)
}

/// Stop ongoing scan
pub fn stop_scan() -> KResult<()> {
    let mut manager = WIFI_MANAGER.lock();
    let iface = manager.default_mut().ok_or(KError::NotFound)?;

    if iface.state == WifiState::Scanning {
        if let Some(ref driver) = iface.driver {
            driver.stop_scan()?;
        }
        iface.state = WifiState::Disconnected;
    }

    Ok(())
}

/// Get open (unencrypted) networks
pub fn get_open_networks() -> Vec<WifiNetwork> {
    scan_results()
        .into_iter()
        .filter(|n| n.security == SecurityType::Open)
        .collect()
}

/// Get encrypted networks only
pub fn get_secure_networks() -> Vec<WifiNetwork> {
    scan_results()
        .into_iter()
        .filter(|n| n.security != SecurityType::Open)
        .collect()
}

/// Get networks sorted by signal strength (strongest first)
pub fn get_networks_by_signal() -> Vec<WifiNetwork> {
    let mut results = scan_results();
    results.sort_by(|a, b| b.signal_strength.cmp(&a.signal_strength));
    results
}

/// Get hidden networks (empty SSID)
pub fn get_hidden_networks() -> Vec<WifiNetwork> {
    scan_results()
        .into_iter()
        .filter(|n| n.ssid.is_empty())
        .collect()
}

/// Get 2.4 GHz networks only
pub fn get_2ghz_networks() -> Vec<WifiNetwork> {
    scan_results()
        .into_iter()
        .filter(|n| {
            n.channel.as_ref()
                .map(|c| c.band() == WifiBand::Band2_4GHz)
                .unwrap_or(false)
        })
        .collect()
}

/// Get 5 GHz networks only
pub fn get_5ghz_networks() -> Vec<WifiNetwork> {
    scan_results()
        .into_iter()
        .filter(|n| {
            n.channel.as_ref()
                .map(|c| c.band() == WifiBand::Band5GHz)
                .unwrap_or(false)
        })
        .collect()
}

/// Process a beacon frame and update scan results
pub fn process_beacon(
    bssid: MacAddress,
    ssid: String,
    channel: Option<WifiChannel>,
    signal: i8,
    security: SecurityType,
    rsn_info: Option<RsnInfo>,
) {
    let network = WifiNetwork {
        bssid,
        ssid,
        channel,
        signal_strength: signal,
        security,
        rsn_info,
        wpa_info: None,
    };

    let mut manager = WIFI_MANAGER.lock();
    if let Some(iface) = manager.default_mut() {
        // Check if already in results
        if let Some(existing) = iface.scan_results.iter_mut()
            .find(|n| n.bssid == network.bssid)
        {
            // Update signal strength
            existing.signal_strength = signal;
        } else {
            // Add new network
            iface.scan_results.push(network);
        }
    }
}

/// Get number of networks found
pub fn network_count() -> usize {
    scan_results().len()
}
