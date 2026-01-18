//! WiFi Hotspot / Access Point Mode
//!
//! Implements SoftAP (Software Access Point) functionality allowing the device
//! to act as a WiFi access point for other devices to connect to.
//!
//! Features:
//! - WPA2/WPA3 Personal authentication
//! - Client management (connect/disconnect)
//! - DHCP server integration
//! - Bandwidth limiting
//! - Client isolation
//! - MAC filtering
//! - Band selection (2.4GHz / 5GHz / 6GHz)

use alloc::string::String;
use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use crate::util::{KResult, KError};
use crate::sync::IrqSafeMutex;
use super::mac::MacAddress;

// ============================================================================
// Hotspot Configuration
// ============================================================================

/// WiFi security mode for hotspot
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotspotSecurity {
    /// Open network (no password)
    Open,
    /// WPA2-Personal (PSK)
    Wpa2Personal,
    /// WPA3-Personal (SAE)
    Wpa3Personal,
    /// WPA2/WPA3 transition mode
    Wpa2Wpa3Transition,
    /// WPA2-Enterprise (802.1X)
    Wpa2Enterprise,
    /// WPA3-Enterprise
    Wpa3Enterprise,
}

impl HotspotSecurity {
    /// Get security name
    pub fn name(&self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::Wpa2Personal => "WPA2-Personal",
            Self::Wpa3Personal => "WPA3-Personal",
            Self::Wpa2Wpa3Transition => "WPA2/WPA3",
            Self::Wpa2Enterprise => "WPA2-Enterprise",
            Self::Wpa3Enterprise => "WPA3-Enterprise",
        }
    }

    /// Returns whether this security requires a password
    pub fn requires_password(&self) -> bool {
        !matches!(self, Self::Open)
    }

    /// Returns whether this security supports PMF (Protected Management Frames)
    pub fn supports_pmf(&self) -> bool {
        matches!(
            self,
            Self::Wpa3Personal | Self::Wpa2Wpa3Transition | Self::Wpa3Enterprise
        )
    }
}

impl Default for HotspotSecurity {
    fn default() -> Self {
        Self::Wpa2Wpa3Transition
    }
}

/// WiFi frequency band
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WifiBand {
    /// 2.4 GHz band (802.11b/g/n)
    Band2_4GHz,
    /// 5 GHz band (802.11a/n/ac)
    Band5GHz,
    /// 6 GHz band (802.11ax/WiFi 6E)
    Band6GHz,
    /// Dual band (2.4 + 5 GHz)
    DualBand,
    /// Tri-band (2.4 + 5 + 6 GHz)
    TriBand,
}

impl WifiBand {
    /// Get band name
    pub fn name(&self) -> &'static str {
        match self {
            Self::Band2_4GHz => "2.4 GHz",
            Self::Band5GHz => "5 GHz",
            Self::Band6GHz => "6 GHz",
            Self::DualBand => "Dual Band",
            Self::TriBand => "Tri-Band",
        }
    }

    /// Returns valid channel numbers for this band
    pub fn channels(&self) -> &'static [u8] {
        match self {
            Self::Band2_4GHz => &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
            Self::Band5GHz => &[36, 40, 44, 48, 52, 56, 60, 64, 100, 104, 108, 112, 116, 120, 124, 128, 132, 136, 140, 144, 149, 153, 157, 161, 165],
            Self::Band6GHz => &[1, 5, 9, 13, 17, 21, 25, 29, 33, 37, 41, 45, 49, 53, 57, 61, 65, 69, 73, 77, 81, 85, 89, 93],
            Self::DualBand => &[1, 6, 11, 36, 40, 44, 48, 149, 153, 157, 161, 165], // Common channels
            Self::TriBand => &[1, 6, 11, 36, 44, 149, 1, 5, 9], // Representative channels
        }
    }

    /// Default channel for this band
    pub fn default_channel(&self) -> u8 {
        match self {
            Self::Band2_4GHz => 6,
            Self::Band5GHz => 36,
            Self::Band6GHz => 1,
            Self::DualBand => 36,
            Self::TriBand => 36,
        }
    }
}

impl Default for WifiBand {
    fn default() -> Self {
        Self::Band2_4GHz
    }
}

/// Channel width
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelWidth {
    /// 20 MHz (legacy)
    Mhz20,
    /// 40 MHz (802.11n)
    Mhz40,
    /// 80 MHz (802.11ac)
    Mhz80,
    /// 160 MHz (802.11ac Wave 2)
    Mhz160,
    /// 320 MHz (802.11be/WiFi 7)
    Mhz320,
}

impl ChannelWidth {
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
}

impl Default for ChannelWidth {
    fn default() -> Self {
        Self::Mhz20
    }
}

/// Hotspot configuration
#[derive(Debug, Clone)]
pub struct HotspotConfig {
    /// Network name (SSID)
    pub ssid: String,
    /// Password (for secured networks)
    pub password: Option<String>,
    /// Security mode
    pub security: HotspotSecurity,
    /// WiFi band
    pub band: WifiBand,
    /// Channel number (0 = auto)
    pub channel: u8,
    /// Channel width
    pub channel_width: ChannelWidth,
    /// Hide SSID (don't broadcast)
    pub hidden_ssid: bool,
    /// Maximum number of clients
    pub max_clients: u8,
    /// Client isolation (clients can't see each other)
    pub client_isolation: bool,
    /// Enable 802.11w (PMF - Protected Management Frames)
    pub pmf_required: bool,
    /// Beacon interval in TUs (Time Units, 1 TU = 1.024ms)
    pub beacon_interval: u16,
    /// DTIM period
    pub dtim_period: u8,
    /// Enable WMM (Wireless Multimedia)
    pub wmm_enabled: bool,
    /// Enable 802.11r (Fast BSS Transition)
    pub fast_bss_transition: bool,
    /// Country code (for regulatory)
    pub country_code: [u8; 2],
    /// Bandwidth limit per client (bytes/sec, 0 = unlimited)
    pub bandwidth_limit: u64,
    /// Inactivity timeout in seconds
    pub inactivity_timeout: u32,
    /// MAC filter mode
    pub mac_filter: MacFilterMode,
    /// MAC filter list
    pub mac_filter_list: Vec<MacAddress>,
}

impl Default for HotspotConfig {
    fn default() -> Self {
        Self {
            ssid: String::from("StenzelOS-AP"),
            password: None,
            security: HotspotSecurity::Wpa2Wpa3Transition,
            band: WifiBand::Band2_4GHz,
            channel: 0, // Auto
            channel_width: ChannelWidth::Mhz20,
            hidden_ssid: false,
            max_clients: 10,
            client_isolation: false,
            pmf_required: false,
            beacon_interval: 100, // ~102.4ms
            dtim_period: 2,
            wmm_enabled: true,
            fast_bss_transition: false,
            country_code: [b'U', b'S'],
            bandwidth_limit: 0,
            inactivity_timeout: 300,
            mac_filter: MacFilterMode::Disabled,
            mac_filter_list: Vec::new(),
        }
    }
}

impl HotspotConfig {
    /// Create a new open hotspot configuration
    pub fn open(ssid: &str) -> Self {
        Self {
            ssid: String::from(ssid),
            security: HotspotSecurity::Open,
            ..Default::default()
        }
    }

    /// Create a new WPA2 hotspot configuration
    pub fn wpa2(ssid: &str, password: &str) -> Self {
        Self {
            ssid: String::from(ssid),
            password: Some(String::from(password)),
            security: HotspotSecurity::Wpa2Personal,
            ..Default::default()
        }
    }

    /// Create a new WPA3 hotspot configuration
    pub fn wpa3(ssid: &str, password: &str) -> Self {
        Self {
            ssid: String::from(ssid),
            password: Some(String::from(password)),
            security: HotspotSecurity::Wpa3Personal,
            pmf_required: true,
            ..Default::default()
        }
    }

    /// Set band
    pub fn with_band(mut self, band: WifiBand) -> Self {
        self.band = band;
        self
    }

    /// Set channel
    pub fn with_channel(mut self, channel: u8) -> Self {
        self.channel = channel;
        self
    }

    /// Set max clients
    pub fn with_max_clients(mut self, max: u8) -> Self {
        self.max_clients = max;
        self
    }

    /// Enable client isolation
    pub fn with_client_isolation(mut self) -> Self {
        self.client_isolation = true;
        self
    }

    /// Validate configuration
    pub fn validate(&self) -> KResult<()> {
        // Check SSID length (1-32 bytes)
        if self.ssid.is_empty() || self.ssid.len() > 32 {
            return Err(KError::Invalid);
        }

        // Check password if required
        if self.security.requires_password() {
            if let Some(ref pwd) = self.password {
                if pwd.len() < 8 || pwd.len() > 63 {
                    return Err(KError::Invalid);
                }
            } else {
                return Err(KError::Invalid);
            }
        }

        // Check channel validity
        if self.channel != 0 && !self.band.channels().contains(&self.channel) {
            return Err(KError::Invalid);
        }

        Ok(())
    }
}

/// MAC address filter mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MacFilterMode {
    /// Filtering disabled
    #[default]
    Disabled,
    /// Whitelist - only allow listed MACs
    Whitelist,
    /// Blacklist - block listed MACs
    Blacklist,
}

// ============================================================================
// Client Management
// ============================================================================

/// Client connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientState {
    /// Just connected, authenticating
    Authenticating,
    /// Association in progress
    Associating,
    /// EAPOL 4-way handshake
    Handshaking,
    /// Fully connected
    Connected,
    /// Disconnecting
    Disconnecting,
    /// Disconnected
    Disconnected,
    /// Blocked (MAC filter or other)
    Blocked,
}

/// Information about a connected client
#[derive(Debug, Clone)]
pub struct ClientInfo {
    /// Client MAC address
    pub mac: MacAddress,
    /// Current state
    pub state: ClientState,
    /// Assigned IP address (from DHCP)
    pub ip_address: Option<[u8; 4]>,
    /// Hostname (if known from DHCP)
    pub hostname: Option<String>,
    /// Connection timestamp (ticks since boot)
    pub connected_at: u64,
    /// Last activity timestamp
    pub last_activity: u64,
    /// Signal strength (dBm)
    pub signal_strength: i8,
    /// Transmit rate (Mbps)
    pub tx_rate: u16,
    /// Receive rate (Mbps)
    pub rx_rate: u16,
    /// Total bytes transmitted to client
    pub tx_bytes: u64,
    /// Total bytes received from client
    pub rx_bytes: u64,
    /// Total packets transmitted
    pub tx_packets: u64,
    /// Total packets received
    pub rx_packets: u64,
    /// Association ID
    pub aid: u16,
    /// Capabilities (HT, VHT, HE, etc.)
    pub capabilities: ClientCapabilities,
    /// PTK (for encryption)
    ptk: Option<[u8; 64]>,
}

impl ClientInfo {
    /// Create new client info
    pub fn new(mac: MacAddress, aid: u16) -> Self {
        Self {
            mac,
            state: ClientState::Authenticating,
            ip_address: None,
            hostname: None,
            connected_at: crate::time::ticks(),
            last_activity: crate::time::ticks(),
            signal_strength: -50,
            tx_rate: 0,
            rx_rate: 0,
            tx_bytes: 0,
            rx_bytes: 0,
            tx_packets: 0,
            rx_packets: 0,
            aid,
            capabilities: ClientCapabilities::default(),
            ptk: None,
        }
    }

    /// Get connection duration in seconds
    pub fn connection_duration(&self) -> u64 {
        let now = crate::time::ticks();
        let ticks_per_sec = crate::time::hz();
        if ticks_per_sec > 0 {
            (now - self.connected_at) / ticks_per_sec
        } else {
            0
        }
    }

    /// Get idle time in seconds
    pub fn idle_time(&self) -> u64 {
        let now = crate::time::ticks();
        let ticks_per_sec = crate::time::hz();
        if ticks_per_sec > 0 {
            (now - self.last_activity) / ticks_per_sec
        } else {
            0
        }
    }

    /// Update activity timestamp
    pub fn touch(&mut self) {
        self.last_activity = crate::time::ticks();
    }

    /// Record transmitted bytes
    pub fn add_tx(&mut self, bytes: u64, packets: u64) {
        self.tx_bytes += bytes;
        self.tx_packets += packets;
        self.touch();
    }

    /// Record received bytes
    pub fn add_rx(&mut self, bytes: u64, packets: u64) {
        self.rx_bytes += bytes;
        self.rx_packets += packets;
        self.touch();
    }
}

/// Client WiFi capabilities
#[derive(Debug, Clone, Copy, Default)]
pub struct ClientCapabilities {
    /// 802.11n (HT) support
    pub ht: bool,
    /// 802.11ac (VHT) support
    pub vht: bool,
    /// 802.11ax (HE) support
    pub he: bool,
    /// 802.11be (EHT) support
    pub eht: bool,
    /// Short GI (guard interval) for 20MHz
    pub short_gi_20: bool,
    /// Short GI for 40MHz
    pub short_gi_40: bool,
    /// Short GI for 80MHz
    pub short_gi_80: bool,
    /// LDPC coding
    pub ldpc: bool,
    /// TX STBC
    pub tx_stbc: bool,
    /// RX STBC streams
    pub rx_stbc: u8,
    /// Number of spatial streams
    pub spatial_streams: u8,
    /// Max A-MPDU length exponent
    pub max_ampdu_exp: u8,
    /// Supports 40MHz in 2.4GHz
    pub width_40_2ghz: bool,
    /// Supports 80MHz
    pub width_80: bool,
    /// Supports 160MHz
    pub width_160: bool,
    /// Supports 320MHz
    pub width_320: bool,
    /// Power management
    pub power_save: bool,
    /// WMM/QoS support
    pub wmm: bool,
    /// MFP capable
    pub mfp_capable: bool,
    /// MFP required
    pub mfp_required: bool,
}

// ============================================================================
// Hotspot State Machine
// ============================================================================

/// Hotspot state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotspotState {
    /// Hotspot is disabled
    Disabled,
    /// Starting up
    Starting,
    /// Running
    Running,
    /// Stopping
    Stopping,
    /// Error state
    Error,
}

/// Hotspot events
#[derive(Debug, Clone)]
pub enum HotspotEvent {
    /// Hotspot started
    Started,
    /// Hotspot stopped
    Stopped,
    /// Client connected
    ClientConnected(MacAddress),
    /// Client disconnected
    ClientDisconnected(MacAddress),
    /// Client authenticated
    ClientAuthenticated(MacAddress),
    /// Client failed authentication
    ClientAuthFailed(MacAddress),
    /// Channel changed
    ChannelChanged(u8),
    /// Error occurred
    Error(HotspotError),
}

/// Hotspot errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotspotError {
    /// Invalid configuration
    InvalidConfig,
    /// Hardware not supported
    NotSupported,
    /// Already running
    AlreadyRunning,
    /// Not running
    NotRunning,
    /// Channel not available
    ChannelNotAvailable,
    /// Max clients reached
    MaxClientsReached,
    /// Client not found
    ClientNotFound,
    /// Authentication failed
    AuthFailed,
    /// Hardware error
    HardwareError,
    /// Regulatory error
    RegulatoryError,
}

// ============================================================================
// Hotspot Manager
// ============================================================================

/// WiFi Hotspot/Access Point manager
pub struct HotspotManager {
    /// Current configuration
    config: Option<HotspotConfig>,
    /// Current state
    state: HotspotState,
    /// Connected clients
    clients: BTreeMap<[u8; 6], ClientInfo>,
    /// Next association ID
    next_aid: u16,
    /// Our MAC address
    our_mac: MacAddress,
    /// Current channel
    current_channel: u8,
    /// Event queue
    events: Vec<HotspotEvent>,
    /// Statistics
    stats: HotspotStats,
    /// DHCP lease pool (IP -> MAC)
    dhcp_leases: BTreeMap<[u8; 4], MacAddress>,
    /// DHCP IP pool start
    dhcp_start_ip: [u8; 4],
    /// DHCP IP pool end
    dhcp_end_ip: [u8; 4],
    /// Initialized
    initialized: bool,
}

/// Hotspot statistics
#[derive(Debug, Clone, Default)]
pub struct HotspotStats {
    /// Total clients ever connected
    pub total_clients: u64,
    /// Currently connected clients
    pub current_clients: u32,
    /// Total bytes transmitted
    pub total_tx_bytes: u64,
    /// Total bytes received
    pub total_rx_bytes: u64,
    /// Total packets transmitted
    pub total_tx_packets: u64,
    /// Total packets received
    pub total_rx_packets: u64,
    /// Beacons sent
    pub beacons_sent: u64,
    /// Authentication failures
    pub auth_failures: u64,
    /// Deauthentications sent
    pub deauths_sent: u64,
    /// Uptime in seconds
    pub uptime: u64,
}

impl HotspotManager {
    /// Create a new hotspot manager
    pub fn new() -> Self {
        Self {
            config: None,
            state: HotspotState::Disabled,
            clients: BTreeMap::new(),
            next_aid: 1,
            our_mac: MacAddress([0; 6]),
            current_channel: 0,
            events: Vec::new(),
            stats: HotspotStats::default(),
            dhcp_leases: BTreeMap::new(),
            dhcp_start_ip: [192, 168, 43, 2],
            dhcp_end_ip: [192, 168, 43, 254],
            initialized: false,
        }
    }

    /// Initialize the hotspot manager
    pub fn init(&mut self, mac: MacAddress) {
        self.our_mac = mac;
        self.initialized = true;
    }

    /// Start hotspot with given configuration
    pub fn start(&mut self, config: HotspotConfig) -> KResult<()> {
        if self.state == HotspotState::Running {
            return Err(KError::Busy);
        }

        config.validate()?;

        // Select channel if auto
        let channel = if config.channel == 0 {
            self.auto_select_channel(&config)
        } else {
            config.channel
        };

        self.current_channel = channel;
        self.config = Some(config);
        self.state = HotspotState::Starting;

        // Configure hardware (simplified - real impl talks to driver)
        self.configure_hardware()?;

        self.state = HotspotState::Running;
        self.events.push(HotspotEvent::Started);

        crate::kprintln!("hotspot: started on channel {}", channel);

        Ok(())
    }

    /// Stop hotspot
    pub fn stop(&mut self) -> KResult<()> {
        if self.state != HotspotState::Running {
            return Err(KError::Invalid);
        }

        self.state = HotspotState::Stopping;

        // Disconnect all clients
        let macs: Vec<[u8; 6]> = self.clients.keys().copied().collect();
        for mac in macs {
            self.disconnect_client(&MacAddress(mac), 0)?;
        }

        self.config = None;
        self.state = HotspotState::Disabled;
        self.events.push(HotspotEvent::Stopped);

        crate::kprintln!("hotspot: stopped");

        Ok(())
    }

    /// Auto-select best channel
    fn auto_select_channel(&self, config: &HotspotConfig) -> u8 {
        // In a real implementation, we'd scan and pick the least busy channel
        config.band.default_channel()
    }

    /// Configure hardware for AP mode
    fn configure_hardware(&mut self) -> KResult<()> {
        // Real implementation would:
        // 1. Set interface to AP mode
        // 2. Configure channel
        // 3. Set up beacon
        // 4. Configure security
        // 5. Start beacon transmission
        Ok(())
    }

    /// Handle client authentication request
    pub fn handle_auth_request(&mut self, client_mac: MacAddress) -> KResult<()> {
        let config = self.config.as_ref().ok_or(KError::Invalid)?;

        // Check MAC filter
        if !self.check_mac_filter(&client_mac, config) {
            self.events.push(HotspotEvent::ClientAuthFailed(client_mac));
            self.stats.auth_failures += 1;
            return Err(KError::PermissionDenied);
        }

        // Check max clients
        if self.clients.len() >= config.max_clients as usize {
            self.events.push(HotspotEvent::ClientAuthFailed(client_mac));
            return Err(KError::NoMemory);
        }

        // Create client entry
        let aid = self.next_aid;
        self.next_aid = self.next_aid.wrapping_add(1).max(1);

        let client = ClientInfo::new(client_mac, aid);
        self.clients.insert(client_mac.0, client);

        crate::kprintln!("hotspot: auth request from {}", client_mac);

        Ok(())
    }

    /// Handle client association request
    pub fn handle_assoc_request(
        &mut self,
        client_mac: MacAddress,
        capabilities: ClientCapabilities,
    ) -> KResult<u16> {
        let client = self.clients
            .get_mut(&client_mac.0)
            .ok_or(KError::NotFound)?;

        client.state = ClientState::Associating;
        client.capabilities = capabilities;

        crate::kprintln!("hotspot: assoc request from {} (AID={})", client_mac, client.aid);

        Ok(client.aid)
    }

    /// Complete client authentication (after 4-way handshake)
    pub fn complete_auth(&mut self, client_mac: MacAddress, ptk: [u8; 64]) -> KResult<()> {
        let client = self.clients
            .get_mut(&client_mac.0)
            .ok_or(KError::NotFound)?;

        client.ptk = Some(ptk);
        client.state = ClientState::Connected;

        self.stats.total_clients += 1;
        self.stats.current_clients += 1;

        self.events.push(HotspotEvent::ClientConnected(client_mac));

        crate::kprintln!("hotspot: client {} connected", client_mac);

        Ok(())
    }

    /// Disconnect a client
    pub fn disconnect_client(&mut self, client_mac: &MacAddress, reason: u16) -> KResult<()> {
        if let Some(mut client) = self.clients.remove(&client_mac.0) {
            client.state = ClientState::Disconnected;

            // Update stats
            self.stats.total_tx_bytes += client.tx_bytes;
            self.stats.total_rx_bytes += client.rx_bytes;
            self.stats.total_tx_packets += client.tx_packets;
            self.stats.total_rx_packets += client.rx_packets;

            if self.stats.current_clients > 0 {
                self.stats.current_clients -= 1;
            }

            // Release DHCP lease
            if let Some(ip) = client.ip_address {
                self.dhcp_leases.remove(&ip);
            }

            self.events.push(HotspotEvent::ClientDisconnected(*client_mac));
            self.stats.deauths_sent += 1;

            crate::kprintln!("hotspot: client {} disconnected (reason={})", client_mac, reason);
        }

        Ok(())
    }

    /// Check MAC filter
    fn check_mac_filter(&self, mac: &MacAddress, config: &HotspotConfig) -> bool {
        match config.mac_filter {
            MacFilterMode::Disabled => true,
            MacFilterMode::Whitelist => {
                config.mac_filter_list.iter().any(|m| m.0 == mac.0)
            }
            MacFilterMode::Blacklist => {
                !config.mac_filter_list.iter().any(|m| m.0 == mac.0)
            }
        }
    }

    /// Allocate DHCP IP for client
    pub fn allocate_dhcp_ip(&mut self, client_mac: MacAddress) -> Option<[u8; 4]> {
        // Check if client already has lease
        for (ip, mac) in &self.dhcp_leases {
            if mac.0 == client_mac.0 {
                return Some(*ip);
            }
        }

        // Find free IP
        let mut ip = self.dhcp_start_ip;
        while ip <= self.dhcp_end_ip {
            if !self.dhcp_leases.contains_key(&ip) {
                self.dhcp_leases.insert(ip, client_mac);

                // Update client record
                if let Some(client) = self.clients.get_mut(&client_mac.0) {
                    client.ip_address = Some(ip);
                }

                return Some(ip);
            }

            // Increment IP
            if ip[3] < 255 {
                ip[3] += 1;
            } else {
                break;
            }
        }

        None
    }

    /// Get client by MAC
    pub fn get_client(&self, mac: &MacAddress) -> Option<&ClientInfo> {
        self.clients.get(&mac.0)
    }

    /// Get all connected clients
    pub fn clients(&self) -> impl Iterator<Item = &ClientInfo> {
        self.clients.values()
    }

    /// Get number of connected clients
    pub fn client_count(&self) -> usize {
        self.clients.len()
    }

    /// Get current state
    pub fn state(&self) -> HotspotState {
        self.state
    }

    /// Is hotspot running?
    pub fn is_running(&self) -> bool {
        self.state == HotspotState::Running
    }

    /// Get current configuration
    pub fn config(&self) -> Option<&HotspotConfig> {
        self.config.as_ref()
    }

    /// Get statistics
    pub fn stats(&self) -> &HotspotStats {
        &self.stats
    }

    /// Get current channel
    pub fn current_channel(&self) -> u8 {
        self.current_channel
    }

    /// Get our MAC address
    pub fn our_mac(&self) -> MacAddress {
        self.our_mac
    }

    /// Take pending events
    pub fn take_events(&mut self) -> Vec<HotspotEvent> {
        core::mem::take(&mut self.events)
    }

    /// Process timeout (check for inactive clients)
    pub fn process_timeout(&mut self) {
        let config = match &self.config {
            Some(c) => c,
            None => return,
        };

        let timeout = config.inactivity_timeout as u64;
        if timeout == 0 {
            return;
        }

        let to_disconnect: Vec<MacAddress> = self.clients
            .values()
            .filter(|c| c.idle_time() > timeout && c.state == ClientState::Connected)
            .map(|c| c.mac)
            .collect();

        for mac in to_disconnect {
            let _ = self.disconnect_client(&mac, 4); // Reason: Inactivity
        }
    }

    /// Build beacon frame
    pub fn build_beacon(&self) -> Option<Vec<u8>> {
        let config = self.config.as_ref()?;

        let mut beacon = Vec::with_capacity(256);

        // Frame control (beacon = 0x80)
        beacon.extend_from_slice(&[0x80, 0x00]);

        // Duration
        beacon.extend_from_slice(&[0x00, 0x00]);

        // Destination (broadcast)
        beacon.extend_from_slice(&[0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);

        // Source (our MAC)
        beacon.extend_from_slice(&self.our_mac.0);

        // BSSID (our MAC)
        beacon.extend_from_slice(&self.our_mac.0);

        // Sequence control
        beacon.extend_from_slice(&[0x00, 0x00]);

        // Timestamp (8 bytes)
        beacon.extend_from_slice(&[0x00; 8]);

        // Beacon interval
        beacon.extend_from_slice(&config.beacon_interval.to_le_bytes());

        // Capability info
        let mut caps: u16 = 0x0001; // ESS
        if config.security != HotspotSecurity::Open {
            caps |= 0x0010; // Privacy
        }
        if config.wmm_enabled {
            caps |= 0x0200; // QoS
        }
        beacon.extend_from_slice(&caps.to_le_bytes());

        // SSID element
        beacon.push(0); // Element ID
        beacon.push(config.ssid.len() as u8);
        if !config.hidden_ssid {
            beacon.extend_from_slice(config.ssid.as_bytes());
        }

        // Supported rates
        beacon.push(1); // Element ID
        beacon.push(8); // Length
        beacon.extend_from_slice(&[0x82, 0x84, 0x8b, 0x96, 0x0c, 0x12, 0x18, 0x24]);

        // DS Parameter Set (channel)
        beacon.push(3); // Element ID
        beacon.push(1); // Length
        beacon.push(self.current_channel);

        // TIM (Traffic Indication Map)
        beacon.push(5); // Element ID
        beacon.push(4); // Length
        beacon.push(config.dtim_period);
        beacon.push(1); // DTIM count
        beacon.push(0); // Bitmap control
        beacon.push(0); // Partial virtual bitmap

        // Country element
        beacon.push(7); // Element ID
        beacon.push(6); // Length
        beacon.extend_from_slice(&config.country_code);
        beacon.push(b' ');
        beacon.push(1); // First channel
        beacon.push(11); // Number of channels
        beacon.push(20); // Max TX power dBm

        // RSN Information Element (if secured)
        if config.security != HotspotSecurity::Open {
            let rsn_ie = self.build_rsn_ie(config);
            beacon.extend_from_slice(&rsn_ie);
        }

        Some(beacon)
    }

    /// Build RSN IE for beacon/probe response
    fn build_rsn_ie(&self, config: &HotspotConfig) -> Vec<u8> {
        let mut ie = Vec::with_capacity(32);

        ie.push(48); // RSN Element ID

        let length_pos = ie.len();
        ie.push(0); // Placeholder for length

        // Version
        ie.extend_from_slice(&1u16.to_le_bytes());

        // Group cipher suite (CCMP)
        ie.extend_from_slice(&[0x00, 0x0f, 0xac, 0x04]);

        // Pairwise cipher suite count
        ie.extend_from_slice(&1u16.to_le_bytes());
        // Pairwise cipher suite (CCMP)
        ie.extend_from_slice(&[0x00, 0x0f, 0xac, 0x04]);

        // AKM suite count
        ie.extend_from_slice(&1u16.to_le_bytes());
        // AKM suite
        let akm = match config.security {
            HotspotSecurity::Wpa2Personal => [0x00, 0x0f, 0xac, 0x02], // PSK
            HotspotSecurity::Wpa3Personal => [0x00, 0x0f, 0xac, 0x08], // SAE
            HotspotSecurity::Wpa2Wpa3Transition => [0x00, 0x0f, 0xac, 0x02], // PSK (SAE will be separate)
            HotspotSecurity::Wpa2Enterprise => [0x00, 0x0f, 0xac, 0x01], // 802.1X
            HotspotSecurity::Wpa3Enterprise => [0x00, 0x0f, 0xac, 0x05], // Suite B
            _ => [0x00, 0x0f, 0xac, 0x02],
        };
        ie.extend_from_slice(&akm);

        // RSN capabilities
        let mut caps: u16 = 0;
        if config.pmf_required {
            caps |= 0x0040; // MFPR
        }
        if config.security.supports_pmf() {
            caps |= 0x0080; // MFPC
        }
        ie.extend_from_slice(&caps.to_le_bytes());

        // Update length
        ie[length_pos] = (ie.len() - length_pos - 1) as u8;

        ie
    }

    /// Process received frame
    pub fn process_frame(&mut self, frame: &[u8], _rssi: i8) -> KResult<Option<Vec<u8>>> {
        if frame.len() < 24 {
            return Ok(None);
        }

        let frame_ctrl = u16::from_le_bytes([frame[0], frame[1]]);
        let frame_type = (frame_ctrl >> 2) & 0x03;
        let frame_subtype = (frame_ctrl >> 4) & 0x0f;

        // Only handle management frames
        if frame_type != 0 {
            return Ok(None);
        }

        let src_mac = MacAddress::from_slice(&frame[10..16]).ok_or(KError::Invalid)?;

        match frame_subtype {
            0x00 => {
                // Association Request
                self.handle_assoc_request(src_mac, ClientCapabilities::default())?;
                let response = self.build_assoc_response(&src_mac)?;
                Ok(Some(response))
            }
            0x04 => {
                // Probe Request
                let response = self.build_probe_response(&src_mac)?;
                Ok(Some(response))
            }
            0x0b => {
                // Authentication
                self.handle_auth_request(src_mac)?;
                let response = self.build_auth_response(&src_mac)?;
                Ok(Some(response))
            }
            0x0c => {
                // Deauthentication
                self.disconnect_client(&src_mac, 0)?;
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    /// Build association response
    fn build_assoc_response(&self, _dest: &MacAddress) -> KResult<Vec<u8>> {
        // Simplified - real implementation builds proper frame
        Ok(Vec::new())
    }

    /// Build probe response
    fn build_probe_response(&self, _dest: &MacAddress) -> KResult<Vec<u8>> {
        // Similar to beacon but unicast
        self.build_beacon().ok_or(KError::Invalid)
    }

    /// Build authentication response
    fn build_auth_response(&self, _dest: &MacAddress) -> KResult<Vec<u8>> {
        // Simplified - real implementation builds proper frame
        Ok(Vec::new())
    }
}

impl Default for HotspotManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Global Hotspot Manager
// ============================================================================

static HOTSPOT_MANAGER: IrqSafeMutex<Option<HotspotManager>> = IrqSafeMutex::new(None);

/// Initialize hotspot subsystem
pub fn init(mac: MacAddress) {
    let mut manager = HotspotManager::new();
    manager.init(mac);
    *HOTSPOT_MANAGER.lock() = Some(manager);
    crate::kprintln!("hotspot: initialized");
}

/// Start hotspot
pub fn start(config: HotspotConfig) -> KResult<()> {
    if let Some(ref mut manager) = *HOTSPOT_MANAGER.lock() {
        manager.start(config)
    } else {
        Err(KError::NotSupported)
    }
}

/// Stop hotspot
pub fn stop() -> KResult<()> {
    if let Some(ref mut manager) = *HOTSPOT_MANAGER.lock() {
        manager.stop()
    } else {
        Err(KError::NotSupported)
    }
}

/// Check if hotspot is running
pub fn is_running() -> bool {
    if let Some(ref manager) = *HOTSPOT_MANAGER.lock() {
        manager.is_running()
    } else {
        false
    }
}

/// Get number of connected clients
pub fn client_count() -> usize {
    if let Some(ref manager) = *HOTSPOT_MANAGER.lock() {
        manager.client_count()
    } else {
        0
    }
}

/// Get hotspot statistics
pub fn stats() -> Option<HotspotStats> {
    if let Some(ref manager) = *HOTSPOT_MANAGER.lock() {
        Some(manager.stats().clone())
    } else {
        None
    }
}

/// Disconnect a client
pub fn disconnect_client(mac: MacAddress) -> KResult<()> {
    if let Some(ref mut manager) = *HOTSPOT_MANAGER.lock() {
        manager.disconnect_client(&mac, 1)
    } else {
        Err(KError::NotSupported)
    }
}

/// Process timeout (call periodically)
pub fn process_timeout() {
    if let Some(ref mut manager) = *HOTSPOT_MANAGER.lock() {
        manager.process_timeout();
    }
}
