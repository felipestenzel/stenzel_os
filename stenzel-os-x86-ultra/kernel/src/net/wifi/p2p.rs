//! WiFi Direct (Wi-Fi P2P) support.
//!
//! Provides:
//! - Device discovery and service discovery
//! - Group formation (GO negotiation)
//! - P2P connection management
//! - Persistent groups
//! - WPS for connection
//! - Miracast/Wi-Fi Display support

use alloc::vec::Vec;
use alloc::vec;
use alloc::string::String;
use alloc::format;
use core::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use crate::sync::IrqSafeMutex;
use crate::util::{KResult, KError};

/// Maximum device name length
const MAX_DEVICE_NAME: usize = 32;
/// Maximum discovered peers
const MAX_PEERS: usize = 32;
/// Maximum persistent groups
const MAX_PERSISTENT_GROUPS: usize = 8;
/// Default GO intent (0-15, 15 = always GO)
const DEFAULT_GO_INTENT: u8 = 7;

/// WiFi P2P state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum P2pState {
    #[default]
    Disabled,
    Idle,
    Discovering,
    Connecting,
    Connected,
    GroupOwner,
    Client,
    Error,
}

impl P2pState {
    pub fn as_str(&self) -> &'static str {
        match self {
            P2pState::Disabled => "Disabled",
            P2pState::Idle => "Idle",
            P2pState::Discovering => "Discovering",
            P2pState::Connecting => "Connecting",
            P2pState::Connected => "Connected",
            P2pState::GroupOwner => "Group Owner",
            P2pState::Client => "P2P Client",
            P2pState::Error => "Error",
        }
    }
}

/// P2P device address (same as MAC address)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct P2pAddress([u8; 6]);

impl P2pAddress {
    pub fn new(addr: [u8; 6]) -> Self {
        Self(addr)
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() >= 6 {
            let mut addr = [0u8; 6];
            addr.copy_from_slice(&bytes[..6]);
            Some(Self(addr))
        } else {
            None
        }
    }

    pub fn as_bytes(&self) -> &[u8; 6] {
        &self.0
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0; 6]
    }

    pub fn to_string(&self) -> String {
        format!("{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            self.0[0], self.0[1], self.0[2],
            self.0[3], self.0[4], self.0[5])
    }
}

/// P2P device type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum P2pDeviceType {
    #[default]
    Unknown,
    Computer,
    Phone,
    Tablet,
    Tv,
    Display,
    Camera,
    Printer,
    GameConsole,
    MediaPlayer,
    NetworkInfrastructure,
    InputDevice,
    Dock,
    MultimediaDevice,
    Gaming,
    Telephone,
    Audio,
}

impl P2pDeviceType {
    /// Get primary device type category (WSC)
    pub fn category(&self) -> u16 {
        match self {
            P2pDeviceType::Computer => 1,
            P2pDeviceType::InputDevice => 2,
            P2pDeviceType::Printer => 3,
            P2pDeviceType::Camera => 4,
            P2pDeviceType::NetworkInfrastructure => 5,
            P2pDeviceType::Display | P2pDeviceType::Tv => 6,
            P2pDeviceType::MediaPlayer | P2pDeviceType::MultimediaDevice => 7,
            P2pDeviceType::GameConsole | P2pDeviceType::Gaming => 8,
            P2pDeviceType::Phone => 9,
            P2pDeviceType::Audio => 10,
            P2pDeviceType::Dock => 11,
            _ => 0,
        }
    }

    /// Get subcategory
    pub fn subcategory(&self) -> u16 {
        match self {
            P2pDeviceType::Computer => 1, // PC
            P2pDeviceType::Phone => 1,    // Windows Mobile
            P2pDeviceType::Tablet => 2,   // Generic phone
            P2pDeviceType::Tv => 1,       // TV
            P2pDeviceType::Display => 2,  // Display
            P2pDeviceType::Printer => 1,  // Printer
            P2pDeviceType::Camera => 1,   // Digital still camera
            _ => 0,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            P2pDeviceType::Unknown => "Unknown",
            P2pDeviceType::Computer => "Computer",
            P2pDeviceType::Phone => "Phone",
            P2pDeviceType::Tablet => "Tablet",
            P2pDeviceType::Tv => "TV",
            P2pDeviceType::Display => "Display",
            P2pDeviceType::Camera => "Camera",
            P2pDeviceType::Printer => "Printer",
            P2pDeviceType::GameConsole => "Game Console",
            P2pDeviceType::MediaPlayer => "Media Player",
            P2pDeviceType::NetworkInfrastructure => "Network Infrastructure",
            P2pDeviceType::InputDevice => "Input Device",
            P2pDeviceType::Dock => "Dock",
            P2pDeviceType::MultimediaDevice => "Multimedia Device",
            P2pDeviceType::Gaming => "Gaming Device",
            P2pDeviceType::Telephone => "Telephone",
            P2pDeviceType::Audio => "Audio Device",
        }
    }
}

/// P2P device capability
#[derive(Debug, Clone, Copy, Default)]
pub struct P2pCapability {
    /// Service discovery
    pub service_discovery: bool,
    /// P2P client discoverability
    pub client_discoverability: bool,
    /// Concurrent operation
    pub concurrent_operation: bool,
    /// P2P infrastructure managed
    pub infrastructure_managed: bool,
    /// P2P device limit
    pub device_limit: bool,
    /// P2P invitation procedure
    pub invitation_procedure: bool,
}

impl P2pCapability {
    pub fn from_u8(val: u8) -> Self {
        Self {
            service_discovery: (val & 0x01) != 0,
            client_discoverability: (val & 0x02) != 0,
            concurrent_operation: (val & 0x04) != 0,
            infrastructure_managed: (val & 0x08) != 0,
            device_limit: (val & 0x10) != 0,
            invitation_procedure: (val & 0x20) != 0,
        }
    }

    pub fn to_u8(&self) -> u8 {
        let mut val = 0u8;
        if self.service_discovery { val |= 0x01; }
        if self.client_discoverability { val |= 0x02; }
        if self.concurrent_operation { val |= 0x04; }
        if self.infrastructure_managed { val |= 0x08; }
        if self.device_limit { val |= 0x10; }
        if self.invitation_procedure { val |= 0x20; }
        val
    }
}

/// P2P group capability
#[derive(Debug, Clone, Copy, Default)]
pub struct GroupCapability {
    /// P2P group owner
    pub group_owner: bool,
    /// Persistent P2P group
    pub persistent_group: bool,
    /// P2P group limit
    pub group_limit: bool,
    /// Intra-BSS distribution
    pub intra_bss_dist: bool,
    /// Cross connection
    pub cross_connection: bool,
    /// Persistent reconnect
    pub persistent_reconnect: bool,
    /// Group formation
    pub group_formation: bool,
    /// IP address allocation
    pub ip_allocation: bool,
}

impl GroupCapability {
    pub fn from_u8(val: u8) -> Self {
        Self {
            group_owner: (val & 0x01) != 0,
            persistent_group: (val & 0x02) != 0,
            group_limit: (val & 0x04) != 0,
            intra_bss_dist: (val & 0x08) != 0,
            cross_connection: (val & 0x10) != 0,
            persistent_reconnect: (val & 0x20) != 0,
            group_formation: (val & 0x40) != 0,
            ip_allocation: (val & 0x80) != 0,
        }
    }

    pub fn to_u8(&self) -> u8 {
        let mut val = 0u8;
        if self.group_owner { val |= 0x01; }
        if self.persistent_group { val |= 0x02; }
        if self.group_limit { val |= 0x04; }
        if self.intra_bss_dist { val |= 0x08; }
        if self.cross_connection { val |= 0x10; }
        if self.persistent_reconnect { val |= 0x20; }
        if self.group_formation { val |= 0x40; }
        if self.ip_allocation { val |= 0x80; }
        val
    }
}

/// Discovered P2P peer
#[derive(Debug, Clone)]
pub struct P2pPeer {
    /// Device address
    pub address: P2pAddress,
    /// Device name
    pub name: String,
    /// Device type
    pub device_type: P2pDeviceType,
    /// Device capability
    pub capability: P2pCapability,
    /// Group capability
    pub group_capability: GroupCapability,
    /// Configuration methods (WPS)
    pub config_methods: u16,
    /// GO intent (0-15)
    pub go_intent: u8,
    /// Signal strength (dBm)
    pub rssi: i8,
    /// Listen channel
    pub listen_channel: u8,
    /// Operating channel
    pub operating_channel: u8,
    /// Services advertised
    pub services: Vec<P2pService>,
    /// Last seen timestamp
    pub last_seen: u64,
    /// Persistent group network ID (if known)
    pub persistent_group_id: Option<u8>,
}

impl P2pPeer {
    pub fn new(address: P2pAddress, name: String) -> Self {
        Self {
            address,
            name,
            device_type: P2pDeviceType::Unknown,
            capability: P2pCapability::default(),
            group_capability: GroupCapability::default(),
            config_methods: 0,
            go_intent: 0,
            rssi: -100,
            listen_channel: 0,
            operating_channel: 0,
            services: Vec::new(),
            last_seen: 0,
            persistent_group_id: None,
        }
    }
}

/// P2P service (for service discovery)
#[derive(Debug, Clone)]
pub struct P2pService {
    /// Service type
    pub service_type: P2pServiceType,
    /// Service name
    pub name: String,
    /// Service info
    pub info: Vec<u8>,
    /// Status
    pub status: u8,
}

/// P2P service type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum P2pServiceType {
    /// All services (for query)
    All,
    /// Bonjour (DNS-SD)
    Bonjour,
    /// UPnP
    Upnp,
    /// WiFi Display (Miracast)
    WifiDisplay,
    /// Vendor specific
    Vendor,
}

impl P2pServiceType {
    pub fn id(&self) -> u8 {
        match self {
            P2pServiceType::All => 0,
            P2pServiceType::Bonjour => 1,
            P2pServiceType::Upnp => 2,
            P2pServiceType::WifiDisplay => 3,
            P2pServiceType::Vendor => 255,
        }
    }
}

/// P2P group information
#[derive(Debug, Clone)]
pub struct P2pGroup {
    /// Network ID (for persistent groups)
    pub network_id: u8,
    /// Group owner address
    pub go_address: P2pAddress,
    /// SSID
    pub ssid: String,
    /// Passphrase
    pub passphrase: String,
    /// Operating channel
    pub channel: u8,
    /// Is this a persistent group
    pub persistent: bool,
    /// GO intent used
    pub go_intent: u8,
    /// Clients in group
    pub clients: Vec<P2pAddress>,
    /// IP address (if allocated)
    pub ip_address: Option<[u8; 4]>,
}

impl P2pGroup {
    pub fn new(go_address: P2pAddress, ssid: String, passphrase: String) -> Self {
        Self {
            network_id: 0,
            go_address,
            ssid,
            passphrase,
            channel: 0,
            persistent: false,
            go_intent: 0,
            clients: Vec::new(),
            ip_address: None,
        }
    }
}

/// WPS configuration method
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WpsMethod {
    /// Push button
    Pbc,
    /// PIN display (we display)
    PinDisplay,
    /// PIN keypad (we enter)
    PinKeypad,
    /// P2P show passphrase
    P2ps,
}

impl WpsMethod {
    pub fn config_methods_bit(&self) -> u16 {
        match self {
            WpsMethod::Pbc => 0x0080,
            WpsMethod::PinDisplay => 0x0008,
            WpsMethod::PinKeypad => 0x0100,
            WpsMethod::P2ps => 0x1000,
        }
    }
}

/// GO negotiation result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoNegotiationResult {
    /// We become GO
    WeAreGo,
    /// Peer becomes GO
    PeerIsGo,
    /// Negotiation failed
    Failed,
}

/// P2P connection info
#[derive(Debug, Clone)]
pub struct P2pConnectionInfo {
    /// Peer address
    pub peer: P2pAddress,
    /// Group formed
    pub group: Option<P2pGroup>,
    /// We are GO
    pub is_go: bool,
    /// WPS method used
    pub wps_method: WpsMethod,
    /// WPS PIN (if applicable)
    pub wps_pin: Option<String>,
}

/// WiFi Display (Miracast) info
#[derive(Debug, Clone, Default)]
pub struct WifiDisplayInfo {
    /// Device info bitmap
    pub device_info: u16,
    /// Control port (TCP)
    pub control_port: u16,
    /// Maximum throughput (Mbps)
    pub max_throughput: u16,
    /// Coupled sink status
    pub coupled_sink: Option<P2pAddress>,
}

impl WifiDisplayInfo {
    /// Source/sink supported
    pub fn is_source(&self) -> bool {
        (self.device_info & 0x03) == 0x00 || (self.device_info & 0x03) == 0x03
    }

    pub fn is_sink(&self) -> bool {
        (self.device_info & 0x03) == 0x01 || (self.device_info & 0x03) == 0x03
    }

    /// Session available
    pub fn session_available(&self) -> bool {
        ((self.device_info >> 4) & 0x03) > 0
    }

    /// WFD service discovery
    pub fn wfd_service_discovery(&self) -> bool {
        (self.device_info & 0x40) != 0
    }

    /// HDCP 2.x content protection
    pub fn hdcp2_support(&self) -> bool {
        (self.device_info & 0x100) != 0
    }
}

/// P2P statistics
#[derive(Debug, Default)]
pub struct P2pStats {
    /// Discovery sessions
    pub discovery_sessions: AtomicU64,
    /// Peers discovered
    pub peers_discovered: AtomicU64,
    /// Connections initiated
    pub connections_initiated: AtomicU64,
    /// Connections received
    pub connections_received: AtomicU64,
    /// GO negotiations
    pub go_negotiations: AtomicU64,
    /// Groups formed
    pub groups_formed: AtomicU64,
    /// Connection failures
    pub connection_failures: AtomicU64,
}

impl P2pStats {
    pub const fn new() -> Self {
        Self {
            discovery_sessions: AtomicU64::new(0),
            peers_discovered: AtomicU64::new(0),
            connections_initiated: AtomicU64::new(0),
            connections_received: AtomicU64::new(0),
            go_negotiations: AtomicU64::new(0),
            groups_formed: AtomicU64::new(0),
            connection_failures: AtomicU64::new(0),
        }
    }
}

/// WiFi P2P Manager
pub struct P2pManager {
    /// Current state
    state: P2pState,
    /// Our device address
    device_address: P2pAddress,
    /// Our device name
    device_name: String,
    /// Our device type
    device_type: P2pDeviceType,
    /// Our capability
    capability: P2pCapability,
    /// GO intent (0-15)
    go_intent: u8,
    /// Discovered peers
    peers: Vec<P2pPeer>,
    /// Current group (if any)
    current_group: Option<P2pGroup>,
    /// Persistent groups
    persistent_groups: Vec<P2pGroup>,
    /// WiFi Display info
    wfd_info: Option<WifiDisplayInfo>,
    /// Listen channel
    listen_channel: u8,
    /// Operating channel
    operating_channel: u8,
    /// Registered services
    services: Vec<P2pService>,
    /// Statistics
    stats: P2pStats,
    /// Initialized
    initialized: AtomicBool,
}

impl P2pManager {
    pub const fn new() -> Self {
        Self {
            state: P2pState::Disabled,
            device_address: P2pAddress([0; 6]),
            device_name: String::new(),
            device_type: P2pDeviceType::Computer,
            capability: P2pCapability {
                service_discovery: true,
                client_discoverability: true,
                concurrent_operation: false,
                infrastructure_managed: false,
                device_limit: false,
                invitation_procedure: true,
            },
            go_intent: DEFAULT_GO_INTENT,
            peers: Vec::new(),
            current_group: None,
            persistent_groups: Vec::new(),
            wfd_info: None,
            listen_channel: 6,
            operating_channel: 6,
            services: Vec::new(),
            stats: P2pStats::new(),
            initialized: AtomicBool::new(false),
        }
    }

    /// Initialize P2P manager
    pub fn init(&mut self, mac_address: [u8; 6]) {
        self.device_address = P2pAddress::new(mac_address);
        self.device_name = String::from("Stenzel-OS");
        self.state = P2pState::Idle;
        self.initialized.store(true, Ordering::SeqCst);
    }

    /// Set device name
    pub fn set_device_name(&mut self, name: &str) {
        let truncated: String = name.chars().take(MAX_DEVICE_NAME).collect();
        self.device_name = truncated;
    }

    /// Set device type
    pub fn set_device_type(&mut self, device_type: P2pDeviceType) {
        self.device_type = device_type;
    }

    /// Set GO intent (0-15)
    pub fn set_go_intent(&mut self, intent: u8) {
        self.go_intent = intent.min(15);
    }

    /// Start discovery
    pub fn start_discovery(&mut self) -> KResult<()> {
        if self.state != P2pState::Idle {
            return Err(KError::Busy);
        }

        self.state = P2pState::Discovering;
        self.peers.clear();
        self.stats.discovery_sessions.fetch_add(1, Ordering::Relaxed);

        // Would send P2P find command to driver
        Ok(())
    }

    /// Stop discovery
    pub fn stop_discovery(&mut self) {
        if self.state == P2pState::Discovering {
            self.state = P2pState::Idle;
        }
    }

    /// Handle peer discovered
    pub fn peer_discovered(&mut self, peer: P2pPeer) {
        self.stats.peers_discovered.fetch_add(1, Ordering::Relaxed);

        // Update or add peer
        if let Some(existing) = self.peers.iter_mut().find(|p| p.address == peer.address) {
            *existing = peer;
        } else if self.peers.len() < MAX_PEERS {
            self.peers.push(peer);
        }
    }

    /// Get discovered peers
    pub fn peers(&self) -> &[P2pPeer] {
        &self.peers
    }

    /// Get peer by address
    pub fn get_peer(&self, addr: &P2pAddress) -> Option<&P2pPeer> {
        self.peers.iter().find(|p| &p.address == addr)
    }

    /// Connect to peer
    pub fn connect(&mut self, peer_addr: &P2pAddress, wps_method: WpsMethod, pin: Option<&str>) -> KResult<()> {
        if self.state != P2pState::Idle && self.state != P2pState::Discovering {
            return Err(KError::Busy);
        }

        let _peer = self.peers.iter().find(|p| &p.address == peer_addr)
            .ok_or(KError::NotFound)?;

        self.state = P2pState::Connecting;
        self.stats.connections_initiated.fetch_add(1, Ordering::Relaxed);

        // Would initiate P2P GO negotiation
        let _wps_pin = pin.map(String::from);
        let _method = wps_method;

        Ok(())
    }

    /// Cancel connection
    pub fn cancel_connect(&mut self) {
        if self.state == P2pState::Connecting {
            self.state = P2pState::Idle;
        }
    }

    /// Handle GO negotiation result
    pub fn go_negotiation_complete(&mut self, result: GoNegotiationResult, peer: &P2pAddress) {
        self.stats.go_negotiations.fetch_add(1, Ordering::Relaxed);

        match result {
            GoNegotiationResult::WeAreGo => {
                self.state = P2pState::GroupOwner;
                // Create group
                let ssid = format!("DIRECT-{:02X}{:02X}-{}",
                    self.device_address.0[4],
                    self.device_address.0[5],
                    &self.device_name[..self.device_name.len().min(8)]);

                let passphrase = self.generate_passphrase();

                let mut group = P2pGroup::new(self.device_address, ssid, passphrase);
                group.channel = self.operating_channel;
                group.go_intent = self.go_intent;
                group.clients.push(*peer);

                self.current_group = Some(group);
                self.stats.groups_formed.fetch_add(1, Ordering::Relaxed);
            }
            GoNegotiationResult::PeerIsGo => {
                self.state = P2pState::Client;
                // Will receive group info from GO
            }
            GoNegotiationResult::Failed => {
                self.state = P2pState::Idle;
                self.stats.connection_failures.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Generate random passphrase
    fn generate_passphrase(&self) -> String {
        // Would use random number generator
        String::from("12345678")
    }

    /// Disconnect from group
    pub fn disconnect(&mut self) -> KResult<()> {
        if self.current_group.is_none() {
            return Err(KError::NotFound);
        }

        // Would send disconnect command
        self.current_group = None;
        self.state = P2pState::Idle;
        Ok(())
    }

    /// Get current group
    pub fn current_group(&self) -> Option<&P2pGroup> {
        self.current_group.as_ref()
    }

    /// Create group (autonomous GO)
    pub fn create_group(&mut self, persistent: bool) -> KResult<()> {
        if self.state != P2pState::Idle {
            return Err(KError::Busy);
        }

        let ssid = format!("DIRECT-{:02X}{:02X}-{}",
            self.device_address.0[4],
            self.device_address.0[5],
            &self.device_name[..self.device_name.len().min(8)]);

        let passphrase = self.generate_passphrase();

        let mut group = P2pGroup::new(self.device_address, ssid, passphrase);
        group.channel = self.operating_channel;
        group.go_intent = 15; // Autonomous GO
        group.persistent = persistent;

        if persistent {
            group.network_id = self.persistent_groups.len() as u8;
            if self.persistent_groups.len() < MAX_PERSISTENT_GROUPS {
                self.persistent_groups.push(group.clone());
            }
        }

        self.current_group = Some(group);
        self.state = P2pState::GroupOwner;
        self.stats.groups_formed.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }

    /// Remove group
    pub fn remove_group(&mut self) -> KResult<()> {
        if self.state != P2pState::GroupOwner && self.state != P2pState::Client {
            return Err(KError::NotFound);
        }

        self.current_group = None;
        self.state = P2pState::Idle;
        Ok(())
    }

    /// Invite peer to group
    pub fn invite(&mut self, peer_addr: &P2pAddress) -> KResult<()> {
        if self.state != P2pState::GroupOwner {
            return Err(KError::Invalid);
        }

        let _peer = self.peers.iter().find(|p| &p.address == peer_addr)
            .ok_or(KError::NotFound)?;

        // Would send P2P invitation
        Ok(())
    }

    /// Register service for advertisement
    pub fn register_service(&mut self, service: P2pService) {
        self.services.push(service);
    }

    /// Unregister service
    pub fn unregister_service(&mut self, name: &str) {
        self.services.retain(|s| s.name != name);
    }

    /// Request service discovery
    pub fn discover_services(&mut self, service_type: P2pServiceType) -> KResult<()> {
        if self.state != P2pState::Discovering {
            return Err(KError::Invalid);
        }

        // Would send service discovery request
        let _ = service_type;
        Ok(())
    }

    /// Enable WiFi Display
    pub fn enable_wifi_display(&mut self, is_source: bool, control_port: u16) {
        let device_info = if is_source {
            0x0000 // Source
        } else {
            0x0001 // Sink
        } | 0x0010; // Session available

        self.wfd_info = Some(WifiDisplayInfo {
            device_info,
            control_port,
            max_throughput: 300, // 300 Mbps
            coupled_sink: None,
        });
    }

    /// Disable WiFi Display
    pub fn disable_wifi_display(&mut self) {
        self.wfd_info = None;
    }

    /// Get WiFi Display info
    pub fn wifi_display_info(&self) -> Option<&WifiDisplayInfo> {
        self.wfd_info.as_ref()
    }

    /// Get persistent groups
    pub fn persistent_groups(&self) -> &[P2pGroup] {
        &self.persistent_groups
    }

    /// Remove persistent group
    pub fn remove_persistent_group(&mut self, network_id: u8) -> KResult<()> {
        let pos = self.persistent_groups.iter()
            .position(|g| g.network_id == network_id)
            .ok_or(KError::NotFound)?;

        self.persistent_groups.remove(pos);
        Ok(())
    }

    /// Get current state
    pub fn state(&self) -> P2pState {
        self.state
    }

    /// Get device address
    pub fn device_address(&self) -> P2pAddress {
        self.device_address
    }

    /// Get device name
    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// Get statistics
    pub fn stats(&self) -> P2pStatsSnapshot {
        P2pStatsSnapshot {
            discovery_sessions: self.stats.discovery_sessions.load(Ordering::Relaxed),
            peers_discovered: self.stats.peers_discovered.load(Ordering::Relaxed),
            connections_initiated: self.stats.connections_initiated.load(Ordering::Relaxed),
            connections_received: self.stats.connections_received.load(Ordering::Relaxed),
            go_negotiations: self.stats.go_negotiations.load(Ordering::Relaxed),
            groups_formed: self.stats.groups_formed.load(Ordering::Relaxed),
            connection_failures: self.stats.connection_failures.load(Ordering::Relaxed),
            current_peers: self.peers.len(),
            persistent_groups: self.persistent_groups.len(),
        }
    }

    /// Format status string
    pub fn format_status(&self) -> String {
        use core::fmt::Write;
        let mut s = String::new();

        let _ = writeln!(s, "WiFi Direct Status:");
        let _ = writeln!(s, "  State: {}", self.state.as_str());
        let _ = writeln!(s, "  Device: {} ({})", self.device_name, self.device_address.to_string());
        let _ = writeln!(s, "  Type: {}", self.device_type.as_str());
        let _ = writeln!(s, "  GO Intent: {}/15", self.go_intent);
        let _ = writeln!(s, "  Listen Channel: {}", self.listen_channel);
        let _ = writeln!(s, "  Peers: {}", self.peers.len());

        if let Some(ref group) = self.current_group {
            let _ = writeln!(s, "\nCurrent Group:");
            let _ = writeln!(s, "  SSID: {}", group.ssid);
            let _ = writeln!(s, "  Channel: {}", group.channel);
            let _ = writeln!(s, "  Clients: {}", group.clients.len());
        }

        if self.wfd_info.is_some() {
            let _ = writeln!(s, "\nWiFi Display: Enabled");
        }

        s
    }
}

/// Statistics snapshot
#[derive(Debug, Clone)]
pub struct P2pStatsSnapshot {
    pub discovery_sessions: u64,
    pub peers_discovered: u64,
    pub connections_initiated: u64,
    pub connections_received: u64,
    pub go_negotiations: u64,
    pub groups_formed: u64,
    pub connection_failures: u64,
    pub current_peers: usize,
    pub persistent_groups: usize,
}

/// Global P2P manager
static P2P_MANAGER: IrqSafeMutex<P2pManager> = IrqSafeMutex::new(P2pManager::new());

/// Initialize P2P subsystem
pub fn init(mac_address: [u8; 6]) {
    P2P_MANAGER.lock().init(mac_address);
}

/// Get state
pub fn state() -> P2pState {
    P2P_MANAGER.lock().state()
}

/// Set device name
pub fn set_device_name(name: &str) {
    P2P_MANAGER.lock().set_device_name(name);
}

/// Start discovery
pub fn start_discovery() -> KResult<()> {
    P2P_MANAGER.lock().start_discovery()
}

/// Stop discovery
pub fn stop_discovery() {
    P2P_MANAGER.lock().stop_discovery()
}

/// Get peers
pub fn peers() -> Vec<P2pPeer> {
    P2P_MANAGER.lock().peers().to_vec()
}

/// Connect to peer
pub fn connect(peer_addr: &P2pAddress, wps_method: WpsMethod, pin: Option<&str>) -> KResult<()> {
    P2P_MANAGER.lock().connect(peer_addr, wps_method, pin)
}

/// Disconnect
pub fn disconnect() -> KResult<()> {
    P2P_MANAGER.lock().disconnect()
}

/// Create group
pub fn create_group(persistent: bool) -> KResult<()> {
    P2P_MANAGER.lock().create_group(persistent)
}

/// Remove group
pub fn remove_group() -> KResult<()> {
    P2P_MANAGER.lock().remove_group()
}

/// Enable WiFi Display
pub fn enable_wifi_display(is_source: bool, control_port: u16) {
    P2P_MANAGER.lock().enable_wifi_display(is_source, control_port)
}

/// Get statistics
pub fn stats() -> P2pStatsSnapshot {
    P2P_MANAGER.lock().stats()
}

/// Format status
pub fn format_status() -> String {
    P2P_MANAGER.lock().format_status()
}
