//! Network Profiles
//!
//! Manages saved network configurations including WiFi passwords, VPN settings,
//! proxy configurations, and per-network preferences.

use alloc::string::String;
use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use crate::util::{KResult, KError};
use crate::sync::IrqSafeMutex;

// ============================================================================
// Profile Types
// ============================================================================

/// Network profile type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileType {
    /// WiFi network profile
    Wifi,
    /// Ethernet profile
    Ethernet,
    /// VPN profile
    Vpn,
    /// Mobile broadband profile
    MobileBroadband,
    /// Bluetooth PAN profile
    BluetoothPan,
}

/// Unique profile identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProfileId(pub u64);

impl ProfileId {
    /// Generate a new unique ID
    pub fn new() -> Self {
        static COUNTER: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, core::sync::atomic::Ordering::Relaxed))
    }
}

impl Default for ProfileId {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// WiFi Profile
// ============================================================================

/// WiFi security type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WifiSecurityType {
    /// Open network
    None,
    /// WEP (deprecated)
    Wep,
    /// WPA-Personal
    WpaPsk,
    /// WPA2-Personal
    Wpa2Psk,
    /// WPA3-SAE
    Wpa3Sae,
    /// WPA2/WPA3 transition
    Wpa2Wpa3,
    /// WPA2-Enterprise
    Wpa2Enterprise,
    /// WPA3-Enterprise
    Wpa3Enterprise,
    /// OWE (Opportunistic Wireless Encryption)
    Owe,
}

impl WifiSecurityType {
    /// Check if password is required
    pub fn requires_password(&self) -> bool {
        matches!(
            self,
            Self::Wep | Self::WpaPsk | Self::Wpa2Psk | Self::Wpa3Sae | Self::Wpa2Wpa3
        )
    }

    /// Check if enterprise auth is used
    pub fn is_enterprise(&self) -> bool {
        matches!(self, Self::Wpa2Enterprise | Self::Wpa3Enterprise)
    }
}

/// WiFi profile credentials
#[derive(Debug, Clone)]
pub enum WifiCredentials {
    /// No credentials (open network or OWE)
    None,
    /// Pre-shared key (password)
    Psk {
        /// Password
        password: String,
    },
    /// Enterprise credentials
    Enterprise {
        /// EAP method
        eap_method: EapMethod,
        /// Identity/username
        identity: String,
        /// Anonymous identity (outer)
        anonymous_identity: Option<String>,
        /// Password (for PEAP/TTLS)
        password: Option<String>,
        /// Client certificate (PEM)
        client_cert: Option<String>,
        /// Client private key (PEM)
        client_key: Option<String>,
        /// CA certificate (PEM)
        ca_cert: Option<String>,
        /// Domain constraint
        domain_constraint: Option<String>,
    },
}

/// EAP method for enterprise auth
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EapMethod {
    /// EAP-TLS
    Tls,
    /// EAP-TTLS
    Ttls,
    /// PEAP
    Peap,
    /// EAP-FAST
    Fast,
    /// EAP-PWD
    Pwd,
}

/// WiFi profile
#[derive(Debug, Clone)]
pub struct WifiProfile {
    /// Profile ID
    pub id: ProfileId,
    /// Network name (SSID)
    pub ssid: String,
    /// Security type
    pub security: WifiSecurityType,
    /// Credentials
    pub credentials: WifiCredentials,
    /// Hidden network
    pub hidden: bool,
    /// Auto-connect when in range
    pub auto_connect: bool,
    /// Connect even when metered
    pub connect_when_metered: bool,
    /// Preferred band
    pub preferred_band: Option<WifiBand>,
    /// Connect to specific BSSID only
    pub bssid_lock: Option<[u8; 6]>,
    /// Priority (higher = more preferred)
    pub priority: i32,
    /// Randomize MAC address
    pub random_mac: MacRandomization,
    /// Enable IPv6
    pub ipv6_enabled: bool,
    /// Static IP configuration
    pub static_ip: Option<StaticIpConfig>,
    /// DNS servers (empty = DHCP)
    pub dns_servers: Vec<[u8; 4]>,
    /// Proxy settings
    pub proxy: ProxyConfig,
    /// Created timestamp
    pub created_at: u64,
    /// Last connected timestamp
    pub last_connected: Option<u64>,
    /// Connection count
    pub connection_count: u32,
}

impl WifiProfile {
    /// Create a new open network profile
    pub fn open(ssid: &str) -> Self {
        Self {
            id: ProfileId::new(),
            ssid: String::from(ssid),
            security: WifiSecurityType::None,
            credentials: WifiCredentials::None,
            hidden: false,
            auto_connect: true,
            connect_when_metered: true,
            preferred_band: None,
            bssid_lock: None,
            priority: 0,
            random_mac: MacRandomization::PerNetwork,
            ipv6_enabled: true,
            static_ip: None,
            dns_servers: Vec::new(),
            proxy: ProxyConfig::None,
            created_at: crate::time::ticks(),
            last_connected: None,
            connection_count: 0,
        }
    }

    /// Create a new WPA2/WPA3 profile
    pub fn psk(ssid: &str, password: &str, security: WifiSecurityType) -> Self {
        Self {
            id: ProfileId::new(),
            ssid: String::from(ssid),
            security,
            credentials: WifiCredentials::Psk {
                password: String::from(password),
            },
            hidden: false,
            auto_connect: true,
            connect_when_metered: true,
            preferred_band: None,
            bssid_lock: None,
            priority: 0,
            random_mac: MacRandomization::PerNetwork,
            ipv6_enabled: true,
            static_ip: None,
            dns_servers: Vec::new(),
            proxy: ProxyConfig::None,
            created_at: crate::time::ticks(),
            last_connected: None,
            connection_count: 0,
        }
    }

    /// Create a new enterprise profile
    pub fn enterprise(
        ssid: &str,
        eap_method: EapMethod,
        identity: &str,
        security: WifiSecurityType,
    ) -> Self {
        Self {
            id: ProfileId::new(),
            ssid: String::from(ssid),
            security,
            credentials: WifiCredentials::Enterprise {
                eap_method,
                identity: String::from(identity),
                anonymous_identity: None,
                password: None,
                client_cert: None,
                client_key: None,
                ca_cert: None,
                domain_constraint: None,
            },
            hidden: false,
            auto_connect: true,
            connect_when_metered: true,
            preferred_band: None,
            bssid_lock: None,
            priority: 0,
            random_mac: MacRandomization::PerNetwork,
            ipv6_enabled: true,
            static_ip: None,
            dns_servers: Vec::new(),
            proxy: ProxyConfig::None,
            created_at: crate::time::ticks(),
            last_connected: None,
            connection_count: 0,
        }
    }

    /// Set password for enterprise profile
    pub fn with_password(mut self, password: &str) -> Self {
        if let WifiCredentials::Enterprise { password: ref mut pwd, .. } = self.credentials {
            *pwd = Some(String::from(password));
        }
        self
    }

    /// Set certificates for enterprise profile
    pub fn with_certs(
        mut self,
        client_cert: &str,
        client_key: &str,
        ca_cert: Option<&str>,
    ) -> Self {
        if let WifiCredentials::Enterprise {
            client_cert: ref mut cc,
            client_key: ref mut ck,
            ca_cert: ref mut ca,
            ..
        } = self.credentials
        {
            *cc = Some(String::from(client_cert));
            *ck = Some(String::from(client_key));
            *ca = ca_cert.map(String::from);
        }
        self
    }

    /// Set as hidden network
    pub fn as_hidden(mut self) -> Self {
        self.hidden = true;
        self
    }

    /// Disable auto-connect
    pub fn no_auto_connect(mut self) -> Self {
        self.auto_connect = false;
        self
    }

    /// Set priority
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Set static IP
    pub fn with_static_ip(mut self, config: StaticIpConfig) -> Self {
        self.static_ip = Some(config);
        self
    }

    /// Set DNS servers
    pub fn with_dns(mut self, servers: Vec<[u8; 4]>) -> Self {
        self.dns_servers = servers;
        self
    }

    /// Set proxy
    pub fn with_proxy(mut self, proxy: ProxyConfig) -> Self {
        self.proxy = proxy;
        self
    }

    /// Record a successful connection
    pub fn record_connection(&mut self) {
        self.last_connected = Some(crate::time::ticks());
        self.connection_count += 1;
    }
}

/// WiFi frequency band preference
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WifiBand {
    /// 2.4 GHz
    Band2_4GHz,
    /// 5 GHz
    Band5GHz,
    /// 6 GHz
    Band6GHz,
    /// Any band (prefer 5 GHz)
    Any,
}

/// MAC address randomization mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MacRandomization {
    /// Never randomize
    Never,
    /// Random MAC per connection
    PerConnection,
    /// Stable random MAC per network
    #[default]
    PerNetwork,
    /// Always use hardware MAC
    UseHardware,
}

// ============================================================================
// IP Configuration
// ============================================================================

/// Static IP configuration
#[derive(Debug, Clone)]
pub struct StaticIpConfig {
    /// IP address
    pub ip_address: [u8; 4],
    /// Subnet mask
    pub netmask: [u8; 4],
    /// Gateway
    pub gateway: [u8; 4],
}

impl StaticIpConfig {
    /// Create new static IP config
    pub fn new(ip: [u8; 4], netmask: [u8; 4], gateway: [u8; 4]) -> Self {
        Self {
            ip_address: ip,
            netmask,
            gateway,
        }
    }

    /// Common /24 subnet
    pub fn class_c(ip: [u8; 4], gateway: [u8; 4]) -> Self {
        Self::new(ip, [255, 255, 255, 0], gateway)
    }
}

/// Proxy configuration
#[derive(Debug, Clone, Default)]
pub enum ProxyConfig {
    /// No proxy
    #[default]
    None,
    /// System proxy settings
    System,
    /// Manual proxy
    Manual {
        /// HTTP proxy host
        http_host: Option<String>,
        /// HTTP proxy port
        http_port: Option<u16>,
        /// HTTPS proxy host
        https_host: Option<String>,
        /// HTTPS proxy port
        https_port: Option<u16>,
        /// Bypass list (comma-separated)
        bypass_list: String,
    },
    /// PAC (Proxy Auto-Config) URL
    Pac {
        /// PAC URL
        url: String,
    },
}

// ============================================================================
// VPN Profile
// ============================================================================

/// VPN protocol type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VpnProtocol {
    /// OpenVPN
    OpenVpn,
    /// WireGuard
    WireGuard,
    /// IPsec/IKEv2
    IpsecIkev2,
    /// L2TP/IPsec
    L2tpIpsec,
    /// PPTP (deprecated)
    Pptp,
    /// SSTP
    Sstp,
}

/// VPN profile
#[derive(Debug, Clone)]
pub struct VpnProfile {
    /// Profile ID
    pub id: ProfileId,
    /// Profile name
    pub name: String,
    /// VPN protocol
    pub protocol: VpnProtocol,
    /// Server address
    pub server: String,
    /// Server port
    pub port: u16,
    /// Username
    pub username: Option<String>,
    /// Password
    pub password: Option<String>,
    /// Pre-shared key
    pub psk: Option<String>,
    /// Certificate (PEM)
    pub certificate: Option<String>,
    /// Private key (PEM)
    pub private_key: Option<String>,
    /// CA certificate (PEM)
    pub ca_cert: Option<String>,
    /// Auto-connect on startup
    pub auto_connect: bool,
    /// Connect on demand (certain domains)
    pub on_demand_rules: Vec<OnDemandRule>,
    /// Kill switch (block traffic when disconnected)
    pub kill_switch: bool,
    /// Split tunneling (exclude local traffic)
    pub split_tunnel: bool,
    /// Excluded subnets from VPN
    pub excluded_subnets: Vec<String>,
    /// DNS servers to use when connected
    pub dns_servers: Vec<[u8; 4]>,
    /// OpenVPN config file content
    pub ovpn_config: Option<String>,
    /// WireGuard config
    pub wg_config: Option<WireGuardConfig>,
    /// Created timestamp
    pub created_at: u64,
    /// Last connected timestamp
    pub last_connected: Option<u64>,
}

impl VpnProfile {
    /// Create new OpenVPN profile
    pub fn openvpn(name: &str, server: &str) -> Self {
        Self {
            id: ProfileId::new(),
            name: String::from(name),
            protocol: VpnProtocol::OpenVpn,
            server: String::from(server),
            port: 1194,
            username: None,
            password: None,
            psk: None,
            certificate: None,
            private_key: None,
            ca_cert: None,
            auto_connect: false,
            on_demand_rules: Vec::new(),
            kill_switch: false,
            split_tunnel: false,
            excluded_subnets: Vec::new(),
            dns_servers: Vec::new(),
            ovpn_config: None,
            wg_config: None,
            created_at: crate::time::ticks(),
            last_connected: None,
        }
    }

    /// Create new WireGuard profile
    pub fn wireguard(name: &str, config: WireGuardConfig) -> Self {
        Self {
            id: ProfileId::new(),
            name: String::from(name),
            protocol: VpnProtocol::WireGuard,
            server: config.endpoint.clone(),
            port: config.endpoint_port,
            username: None,
            password: None,
            psk: config.preshared_key.clone(),
            certificate: None,
            private_key: None,
            ca_cert: None,
            auto_connect: false,
            on_demand_rules: Vec::new(),
            kill_switch: false,
            split_tunnel: false,
            excluded_subnets: Vec::new(),
            dns_servers: config.dns.clone(),
            ovpn_config: None,
            wg_config: Some(config),
            created_at: crate::time::ticks(),
            last_connected: None,
        }
    }

    /// Create new IPsec/IKEv2 profile
    pub fn ikev2(name: &str, server: &str) -> Self {
        Self {
            id: ProfileId::new(),
            name: String::from(name),
            protocol: VpnProtocol::IpsecIkev2,
            server: String::from(server),
            port: 500,
            username: None,
            password: None,
            psk: None,
            certificate: None,
            private_key: None,
            ca_cert: None,
            auto_connect: false,
            on_demand_rules: Vec::new(),
            kill_switch: false,
            split_tunnel: false,
            excluded_subnets: Vec::new(),
            dns_servers: Vec::new(),
            ovpn_config: None,
            wg_config: None,
            created_at: crate::time::ticks(),
            last_connected: None,
        }
    }

    /// Set credentials
    pub fn with_credentials(mut self, username: &str, password: &str) -> Self {
        self.username = Some(String::from(username));
        self.password = Some(String::from(password));
        self
    }

    /// Set certificates
    pub fn with_certs(mut self, cert: &str, key: &str, ca: Option<&str>) -> Self {
        self.certificate = Some(String::from(cert));
        self.private_key = Some(String::from(key));
        self.ca_cert = ca.map(String::from);
        self
    }

    /// Enable kill switch
    pub fn with_kill_switch(mut self) -> Self {
        self.kill_switch = true;
        self
    }

    /// Enable split tunneling
    pub fn with_split_tunnel(mut self, excluded: Vec<String>) -> Self {
        self.split_tunnel = true;
        self.excluded_subnets = excluded;
        self
    }

    /// Import from .ovpn file
    pub fn from_ovpn(name: &str, ovpn_content: &str) -> KResult<Self> {
        let mut profile = Self::openvpn(name, "");

        // Parse .ovpn file
        for line in ovpn_content.lines() {
            let line = line.trim();
            if line.starts_with("remote ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    profile.server = String::from(parts[1]);
                }
                if parts.len() >= 3 {
                    profile.port = parts[2].parse().unwrap_or(1194);
                }
            }
        }

        profile.ovpn_config = Some(String::from(ovpn_content));
        Ok(profile)
    }
}

/// WireGuard configuration
#[derive(Debug, Clone)]
pub struct WireGuardConfig {
    /// Private key (base64)
    pub private_key: String,
    /// Local address
    pub address: String,
    /// Peer public key (base64)
    pub peer_public_key: String,
    /// Pre-shared key (optional, base64)
    pub preshared_key: Option<String>,
    /// Endpoint server
    pub endpoint: String,
    /// Endpoint port
    pub endpoint_port: u16,
    /// Allowed IPs
    pub allowed_ips: Vec<String>,
    /// Persistent keepalive interval
    pub persistent_keepalive: Option<u16>,
    /// DNS servers
    pub dns: Vec<[u8; 4]>,
}

impl WireGuardConfig {
    /// Parse from WireGuard config format
    pub fn parse(config: &str) -> KResult<Self> {
        let mut private_key = String::new();
        let mut address = String::new();
        let mut peer_public_key = String::new();
        let mut preshared_key = None;
        let mut endpoint = String::new();
        let mut endpoint_port = 51820u16;
        let mut allowed_ips = Vec::new();
        let mut persistent_keepalive = None;
        let mut dns = Vec::new();

        for line in config.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
                continue;
            }

            let mut parts = line.splitn(2, '=');
            let key = parts.next().unwrap_or("").trim();
            let value = parts.next().unwrap_or("").trim();

            match key {
                "PrivateKey" => private_key = String::from(value),
                "Address" => address = String::from(value),
                "PublicKey" => peer_public_key = String::from(value),
                "PresharedKey" => preshared_key = Some(String::from(value)),
                "Endpoint" => {
                    let ep_parts: Vec<&str> = value.rsplitn(2, ':').collect();
                    if ep_parts.len() == 2 {
                        endpoint_port = ep_parts[0].parse().unwrap_or(51820);
                        endpoint = String::from(ep_parts[1]);
                    } else {
                        endpoint = String::from(value);
                    }
                }
                "AllowedIPs" => {
                    allowed_ips = value.split(',').map(|s| String::from(s.trim())).collect();
                }
                "PersistentKeepalive" => {
                    persistent_keepalive = value.parse().ok();
                }
                "DNS" => {
                    for dns_str in value.split(',') {
                        let dns_str = dns_str.trim();
                        let parts: Vec<&str> = dns_str.split('.').collect();
                        if parts.len() == 4 {
                            if let (Ok(a), Ok(b), Ok(c), Ok(d)) = (
                                parts[0].parse::<u8>(),
                                parts[1].parse::<u8>(),
                                parts[2].parse::<u8>(),
                                parts[3].parse::<u8>(),
                            ) {
                                dns.push([a, b, c, d]);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        if private_key.is_empty() || peer_public_key.is_empty() {
            return Err(KError::Invalid);
        }

        Ok(Self {
            private_key,
            address,
            peer_public_key,
            preshared_key,
            endpoint,
            endpoint_port,
            allowed_ips,
            persistent_keepalive,
            dns,
        })
    }
}

/// On-demand VPN connection rule
#[derive(Debug, Clone)]
pub struct OnDemandRule {
    /// Domains that trigger connection
    pub domains: Vec<String>,
    /// Action when matching
    pub action: OnDemandAction,
}

/// On-demand action
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnDemandAction {
    /// Connect VPN
    Connect,
    /// Disconnect VPN
    Disconnect,
    /// Ignore (use current state)
    Ignore,
}

// ============================================================================
// Ethernet Profile
// ============================================================================

/// Ethernet profile
#[derive(Debug, Clone)]
pub struct EthernetProfile {
    /// Profile ID
    pub id: ProfileId,
    /// Profile name
    pub name: String,
    /// Match by MAC address
    pub interface_mac: Option<[u8; 6]>,
    /// 802.1X authentication
    pub auth_8021x: Option<Dot1xConfig>,
    /// Static IP configuration
    pub static_ip: Option<StaticIpConfig>,
    /// DNS servers
    pub dns_servers: Vec<[u8; 4]>,
    /// Proxy settings
    pub proxy: ProxyConfig,
    /// Created timestamp
    pub created_at: u64,
}

impl EthernetProfile {
    /// Create new DHCP profile
    pub fn dhcp(name: &str) -> Self {
        Self {
            id: ProfileId::new(),
            name: String::from(name),
            interface_mac: None,
            auth_8021x: None,
            static_ip: None,
            dns_servers: Vec::new(),
            proxy: ProxyConfig::None,
            created_at: crate::time::ticks(),
        }
    }

    /// Create new static IP profile
    pub fn static_config(name: &str, config: StaticIpConfig) -> Self {
        Self {
            id: ProfileId::new(),
            name: String::from(name),
            interface_mac: None,
            auth_8021x: None,
            static_ip: Some(config),
            dns_servers: Vec::new(),
            proxy: ProxyConfig::None,
            created_at: crate::time::ticks(),
        }
    }
}

/// 802.1X configuration
#[derive(Debug, Clone)]
pub struct Dot1xConfig {
    /// EAP method
    pub eap_method: EapMethod,
    /// Identity
    pub identity: String,
    /// Password
    pub password: Option<String>,
    /// Client certificate (PEM)
    pub client_cert: Option<String>,
    /// Client private key (PEM)
    pub client_key: Option<String>,
    /// CA certificate (PEM)
    pub ca_cert: Option<String>,
}

// ============================================================================
// Profile Manager
// ============================================================================

/// Network profile manager
pub struct ProfileManager {
    /// WiFi profiles by SSID
    wifi_profiles: BTreeMap<String, WifiProfile>,
    /// VPN profiles by name
    vpn_profiles: BTreeMap<String, VpnProfile>,
    /// Ethernet profiles by name
    ethernet_profiles: BTreeMap<String, EthernetProfile>,
    /// Active connections
    active_connections: Vec<ProfileId>,
    /// Default WiFi profile SSID
    default_wifi: Option<String>,
    /// Initialized
    initialized: bool,
}

impl ProfileManager {
    /// Create a new profile manager
    pub fn new() -> Self {
        Self {
            wifi_profiles: BTreeMap::new(),
            vpn_profiles: BTreeMap::new(),
            ethernet_profiles: BTreeMap::new(),
            active_connections: Vec::new(),
            default_wifi: None,
            initialized: false,
        }
    }

    /// Initialize the profile manager
    pub fn init(&mut self) {
        self.initialized = true;
        crate::kprintln!("profiles: network profile manager initialized");
    }

    // WiFi profiles

    /// Add a WiFi profile
    pub fn add_wifi(&mut self, profile: WifiProfile) {
        let ssid = profile.ssid.clone();
        self.wifi_profiles.insert(ssid, profile);
    }

    /// Get a WiFi profile by SSID
    pub fn get_wifi(&self, ssid: &str) -> Option<&WifiProfile> {
        self.wifi_profiles.get(ssid)
    }

    /// Get a mutable WiFi profile
    pub fn get_wifi_mut(&mut self, ssid: &str) -> Option<&mut WifiProfile> {
        self.wifi_profiles.get_mut(ssid)
    }

    /// Remove a WiFi profile
    pub fn remove_wifi(&mut self, ssid: &str) -> Option<WifiProfile> {
        self.wifi_profiles.remove(ssid)
    }

    /// List all WiFi profiles
    pub fn list_wifi(&self) -> Vec<&WifiProfile> {
        self.wifi_profiles.values().collect()
    }

    /// Find WiFi profiles that match a scanned network
    pub fn find_matching_wifi(&self, ssid: &str, bssid: Option<[u8; 6]>) -> Option<&WifiProfile> {
        self.wifi_profiles.get(ssid).filter(|p| {
            // If profile has BSSID lock, check it matches
            if let Some(lock_bssid) = p.bssid_lock {
                if let Some(scanned_bssid) = bssid {
                    lock_bssid == scanned_bssid
                } else {
                    false
                }
            } else {
                true
            }
        })
    }

    /// Get WiFi profiles sorted by priority and last connection
    pub fn wifi_by_priority(&self) -> Vec<&WifiProfile> {
        let mut profiles: Vec<_> = self.wifi_profiles.values().collect();
        profiles.sort_by(|a, b| {
            // Higher priority first, then most recently connected
            b.priority.cmp(&a.priority)
                .then_with(|| b.last_connected.cmp(&a.last_connected))
        });
        profiles
    }

    // VPN profiles

    /// Add a VPN profile
    pub fn add_vpn(&mut self, profile: VpnProfile) {
        let name = profile.name.clone();
        self.vpn_profiles.insert(name, profile);
    }

    /// Get a VPN profile by name
    pub fn get_vpn(&self, name: &str) -> Option<&VpnProfile> {
        self.vpn_profiles.get(name)
    }

    /// Get a mutable VPN profile
    pub fn get_vpn_mut(&mut self, name: &str) -> Option<&mut VpnProfile> {
        self.vpn_profiles.get_mut(name)
    }

    /// Remove a VPN profile
    pub fn remove_vpn(&mut self, name: &str) -> Option<VpnProfile> {
        self.vpn_profiles.remove(name)
    }

    /// List all VPN profiles
    pub fn list_vpn(&self) -> Vec<&VpnProfile> {
        self.vpn_profiles.values().collect()
    }

    // Ethernet profiles

    /// Add an Ethernet profile
    pub fn add_ethernet(&mut self, profile: EthernetProfile) {
        let name = profile.name.clone();
        self.ethernet_profiles.insert(name, profile);
    }

    /// Get an Ethernet profile by name
    pub fn get_ethernet(&self, name: &str) -> Option<&EthernetProfile> {
        self.ethernet_profiles.get(name)
    }

    /// Remove an Ethernet profile
    pub fn remove_ethernet(&mut self, name: &str) -> Option<EthernetProfile> {
        self.ethernet_profiles.remove(name)
    }

    /// List all Ethernet profiles
    pub fn list_ethernet(&self) -> Vec<&EthernetProfile> {
        self.ethernet_profiles.values().collect()
    }

    // Connection management

    /// Mark a profile as active
    pub fn mark_active(&mut self, id: ProfileId) {
        if !self.active_connections.contains(&id) {
            self.active_connections.push(id);
        }
    }

    /// Mark a profile as inactive
    pub fn mark_inactive(&mut self, id: ProfileId) {
        self.active_connections.retain(|&i| i != id);
    }

    /// Get active connection IDs
    pub fn active_connections(&self) -> &[ProfileId] {
        &self.active_connections
    }

    /// Check if a profile is active
    pub fn is_active(&self, id: ProfileId) -> bool {
        self.active_connections.contains(&id)
    }

    // Statistics

    /// Get total number of profiles
    pub fn total_profiles(&self) -> usize {
        self.wifi_profiles.len() + self.vpn_profiles.len() + self.ethernet_profiles.len()
    }

    /// Clear all profiles
    pub fn clear_all(&mut self) {
        self.wifi_profiles.clear();
        self.vpn_profiles.clear();
        self.ethernet_profiles.clear();
        self.active_connections.clear();
        self.default_wifi = None;
    }
}

impl Default for ProfileManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Global Manager
// ============================================================================

static PROFILE_MANAGER: IrqSafeMutex<Option<ProfileManager>> = IrqSafeMutex::new(None);

/// Initialize the profile manager
pub fn init() {
    let mut manager = ProfileManager::new();
    manager.init();
    *PROFILE_MANAGER.lock() = Some(manager);
}

/// Add a WiFi profile
pub fn add_wifi(profile: WifiProfile) {
    if let Some(ref mut manager) = *PROFILE_MANAGER.lock() {
        manager.add_wifi(profile);
    }
}

/// Get a WiFi profile by SSID
pub fn get_wifi(ssid: &str) -> Option<WifiProfile> {
    PROFILE_MANAGER.lock().as_ref()
        .and_then(|m| m.get_wifi(ssid).cloned())
}

/// Remove a WiFi profile
pub fn remove_wifi(ssid: &str) -> Option<WifiProfile> {
    if let Some(ref mut manager) = *PROFILE_MANAGER.lock() {
        manager.remove_wifi(ssid)
    } else {
        None
    }
}

/// Add a VPN profile
pub fn add_vpn(profile: VpnProfile) {
    if let Some(ref mut manager) = *PROFILE_MANAGER.lock() {
        manager.add_vpn(profile);
    }
}

/// Get a VPN profile by name
pub fn get_vpn(name: &str) -> Option<VpnProfile> {
    PROFILE_MANAGER.lock().as_ref()
        .and_then(|m| m.get_vpn(name).cloned())
}

/// Remove a VPN profile
pub fn remove_vpn(name: &str) -> Option<VpnProfile> {
    if let Some(ref mut manager) = *PROFILE_MANAGER.lock() {
        manager.remove_vpn(name)
    } else {
        None
    }
}

/// Get number of saved profiles
pub fn profile_count() -> usize {
    PROFILE_MANAGER.lock().as_ref()
        .map(|m| m.total_profiles())
        .unwrap_or(0)
}
