//! Network Settings
//!
//! Wi-Fi, Ethernet, VPN, and network configuration.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use spin::Mutex;

/// Global network settings state
static NETWORK_SETTINGS: Mutex<Option<NetworkSettings>> = Mutex::new(None);

/// Network settings state
pub struct NetworkSettings {
    /// Wi-Fi enabled
    pub wifi_enabled: bool,
    /// Airplane mode
    pub airplane_mode: bool,
    /// Connected network
    pub connected_network: Option<String>,
    /// Available Wi-Fi networks
    pub wifi_networks: Vec<WifiNetwork>,
    /// Ethernet connections
    pub ethernet_connections: Vec<EthernetConnection>,
    /// VPN connections
    pub vpn_connections: Vec<VpnConnection>,
    /// Saved networks
    pub saved_networks: Vec<SavedNetwork>,
    /// Proxy settings
    pub proxy: ProxySettings,
    /// Hotspot settings
    pub hotspot: HotspotSettings,
}

/// Wi-Fi network
#[derive(Debug, Clone)]
pub struct WifiNetwork {
    /// SSID
    pub ssid: String,
    /// BSSID
    pub bssid: String,
    /// Signal strength (0-100)
    pub signal_strength: u32,
    /// Security type
    pub security: WifiSecurity,
    /// Frequency (MHz)
    pub frequency: u32,
    /// Is connected
    pub connected: bool,
    /// Is saved
    pub saved: bool,
}

/// Wi-Fi security type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WifiSecurity {
    Open,
    Wep,
    WpaPsk,
    Wpa2Psk,
    Wpa3Sae,
    Wpa2Enterprise,
    Wpa3Enterprise,
}

impl WifiSecurity {
    pub fn name(&self) -> &'static str {
        match self {
            WifiSecurity::Open => "Open",
            WifiSecurity::Wep => "WEP",
            WifiSecurity::WpaPsk => "WPA",
            WifiSecurity::Wpa2Psk => "WPA2",
            WifiSecurity::Wpa3Sae => "WPA3",
            WifiSecurity::Wpa2Enterprise => "WPA2 Enterprise",
            WifiSecurity::Wpa3Enterprise => "WPA3 Enterprise",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            WifiSecurity::Open => "network-wireless-no-encryption",
            _ => "network-wireless-encrypted",
        }
    }
}

/// Ethernet connection
#[derive(Debug, Clone)]
pub struct EthernetConnection {
    /// Interface name
    pub interface: String,
    /// Display name
    pub name: String,
    /// Is connected
    pub connected: bool,
    /// Speed (Mbps)
    pub speed: u32,
    /// IP address
    pub ip_address: Option<String>,
    /// MAC address
    pub mac_address: String,
    /// DHCP enabled
    pub dhcp: bool,
}

/// VPN connection
#[derive(Debug, Clone)]
pub struct VpnConnection {
    /// Connection name
    pub name: String,
    /// VPN type
    pub vpn_type: VpnType,
    /// Server address
    pub server: String,
    /// Is connected
    pub connected: bool,
    /// Is saved
    pub saved: bool,
}

/// VPN type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VpnType {
    WireGuard,
    OpenVpn,
    IpsecIkev2,
    L2tp,
    Pptp,
}

impl VpnType {
    pub fn name(&self) -> &'static str {
        match self {
            VpnType::WireGuard => "WireGuard",
            VpnType::OpenVpn => "OpenVPN",
            VpnType::IpsecIkev2 => "IPsec IKEv2",
            VpnType::L2tp => "L2TP",
            VpnType::Pptp => "PPTP",
        }
    }
}

/// Saved network
#[derive(Debug, Clone)]
pub struct SavedNetwork {
    /// SSID
    pub ssid: String,
    /// Security type
    pub security: WifiSecurity,
    /// Auto connect
    pub auto_connect: bool,
    /// Last connected timestamp
    pub last_connected: Option<u64>,
}

/// Proxy settings
#[derive(Debug, Clone)]
pub struct ProxySettings {
    /// Proxy mode
    pub mode: ProxyMode,
    /// HTTP proxy
    pub http_proxy: Option<ProxyConfig>,
    /// HTTPS proxy
    pub https_proxy: Option<ProxyConfig>,
    /// SOCKS proxy
    pub socks_proxy: Option<ProxyConfig>,
    /// Bypass list
    pub bypass: Vec<String>,
}

/// Proxy mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyMode {
    None,
    Manual,
    Auto,
}

/// Proxy config
#[derive(Debug, Clone)]
pub struct ProxyConfig {
    /// Host
    pub host: String,
    /// Port
    pub port: u16,
    /// Requires auth
    pub auth: bool,
    /// Username
    pub username: Option<String>,
}

/// Hotspot settings
#[derive(Debug, Clone)]
pub struct HotspotSettings {
    /// Enabled
    pub enabled: bool,
    /// SSID
    pub ssid: String,
    /// Password
    pub password: Option<String>,
    /// Band
    pub band: WifiBand,
    /// Connected clients
    pub clients: Vec<HotspotClient>,
}

/// Wi-Fi band
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WifiBand {
    Band2_4GHz,
    Band5GHz,
    Band6GHz,
    Auto,
}

/// Hotspot client
#[derive(Debug, Clone)]
pub struct HotspotClient {
    /// Device name
    pub name: String,
    /// MAC address
    pub mac: String,
    /// IP address
    pub ip: String,
}

/// Initialize network settings
pub fn init() {
    let mut state = NETWORK_SETTINGS.lock();
    if state.is_some() {
        return;
    }

    *state = Some(NetworkSettings {
        wifi_enabled: true,
        airplane_mode: false,
        connected_network: None,
        wifi_networks: Vec::new(),
        ethernet_connections: vec![
            EthernetConnection {
                interface: "eth0".to_string(),
                name: "Ethernet".to_string(),
                connected: false,
                speed: 1000,
                ip_address: None,
                mac_address: "00:00:00:00:00:00".to_string(),
                dhcp: true,
            },
        ],
        vpn_connections: Vec::new(),
        saved_networks: Vec::new(),
        proxy: ProxySettings {
            mode: ProxyMode::None,
            http_proxy: None,
            https_proxy: None,
            socks_proxy: None,
            bypass: Vec::new(),
        },
        hotspot: HotspotSettings {
            enabled: false,
            ssid: "Stenzel-Hotspot".to_string(),
            password: None,
            band: WifiBand::Auto,
            clients: Vec::new(),
        },
    });

    crate::kprintln!("network settings: initialized");
}

/// Set Wi-Fi enabled
pub fn set_wifi_enabled(enabled: bool) {
    let mut state = NETWORK_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.wifi_enabled = enabled;
        if !enabled {
            s.connected_network = None;
        }
    }
}

/// Is Wi-Fi enabled
pub fn is_wifi_enabled() -> bool {
    let state = NETWORK_SETTINGS.lock();
    state.as_ref().map(|s| s.wifi_enabled).unwrap_or(false)
}

/// Set airplane mode
pub fn set_airplane_mode(enabled: bool) {
    let mut state = NETWORK_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.airplane_mode = enabled;
        if enabled {
            s.wifi_enabled = false;
            s.connected_network = None;
        }
    }
}

/// Is airplane mode
pub fn is_airplane_mode() -> bool {
    let state = NETWORK_SETTINGS.lock();
    state.as_ref().map(|s| s.airplane_mode).unwrap_or(false)
}

/// Get available Wi-Fi networks
pub fn get_wifi_networks() -> Vec<WifiNetwork> {
    let state = NETWORK_SETTINGS.lock();
    state.as_ref().map(|s| s.wifi_networks.clone()).unwrap_or_default()
}

/// Connect to Wi-Fi network
pub fn connect_wifi(ssid: &str, password: Option<&str>) -> Result<(), NetworkError> {
    let mut state = NETWORK_SETTINGS.lock();
    let state = state.as_mut().ok_or(NetworkError::NotInitialized)?;

    if !state.wifi_enabled {
        return Err(NetworkError::WifiDisabled);
    }

    let network = state.wifi_networks.iter()
        .find(|n| n.ssid == ssid)
        .ok_or(NetworkError::NetworkNotFound)?;

    // Check if password required
    if network.security != WifiSecurity::Open && password.is_none() {
        return Err(NetworkError::PasswordRequired);
    }

    // TODO: Actually connect via wireless driver
    state.connected_network = Some(ssid.to_string());

    Ok(())
}

/// Disconnect Wi-Fi
pub fn disconnect_wifi() {
    let mut state = NETWORK_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.connected_network = None;
    }
}

/// Get connected network
pub fn get_connected_network() -> Option<String> {
    let state = NETWORK_SETTINGS.lock();
    state.as_ref().and_then(|s| s.connected_network.clone())
}

/// Get Ethernet connections
pub fn get_ethernet_connections() -> Vec<EthernetConnection> {
    let state = NETWORK_SETTINGS.lock();
    state.as_ref().map(|s| s.ethernet_connections.clone()).unwrap_or_default()
}

/// Get VPN connections
pub fn get_vpn_connections() -> Vec<VpnConnection> {
    let state = NETWORK_SETTINGS.lock();
    state.as_ref().map(|s| s.vpn_connections.clone()).unwrap_or_default()
}

/// Add VPN connection
pub fn add_vpn(name: &str, vpn_type: VpnType, server: &str) {
    let mut state = NETWORK_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.vpn_connections.push(VpnConnection {
            name: name.to_string(),
            vpn_type,
            server: server.to_string(),
            connected: false,
            saved: true,
        });
    }
}

/// Connect VPN
pub fn connect_vpn(name: &str) -> Result<(), NetworkError> {
    let mut state = NETWORK_SETTINGS.lock();
    let state = state.as_mut().ok_or(NetworkError::NotInitialized)?;

    let vpn = state.vpn_connections.iter_mut()
        .find(|v| v.name == name)
        .ok_or(NetworkError::VpnNotFound)?;

    // TODO: Actually connect
    vpn.connected = true;

    Ok(())
}

/// Disconnect VPN
pub fn disconnect_vpn(name: &str) -> Result<(), NetworkError> {
    let mut state = NETWORK_SETTINGS.lock();
    let state = state.as_mut().ok_or(NetworkError::NotInitialized)?;

    let vpn = state.vpn_connections.iter_mut()
        .find(|v| v.name == name)
        .ok_or(NetworkError::VpnNotFound)?;

    vpn.connected = false;

    Ok(())
}

/// Set hotspot enabled
pub fn set_hotspot_enabled(enabled: bool) {
    let mut state = NETWORK_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.hotspot.enabled = enabled;
    }
}

/// Configure hotspot
pub fn configure_hotspot(ssid: &str, password: Option<&str>, band: WifiBand) {
    let mut state = NETWORK_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.hotspot.ssid = ssid.to_string();
        s.hotspot.password = password.map(|p| p.to_string());
        s.hotspot.band = band;
    }
}

/// Scan for Wi-Fi networks
pub fn scan_wifi() {
    // TODO: Trigger actual Wi-Fi scan via driver
    crate::kprintln!("network: scanning for Wi-Fi networks...");
}

/// Network error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkError {
    NotInitialized,
    WifiDisabled,
    NetworkNotFound,
    PasswordRequired,
    AuthFailed,
    VpnNotFound,
    ConnectionFailed,
}
