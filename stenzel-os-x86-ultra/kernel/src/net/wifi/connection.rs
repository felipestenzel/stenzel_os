//! WiFi Connection Manager
//!
//! Handles the full WiFi connection lifecycle including:
//! - Connecting to networks with WPA/WPA2 authentication
//! - Connection state machine
//! - Reconnection handling
//! - Saved network profiles
//! - DHCP integration for IP configuration

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::sync::Arc;
use alloc::collections::BTreeMap;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::sync::IrqSafeMutex;
use crate::util::{KResult, KError};
use crate::time::ticks as get_ticks;

use super::{WifiNetwork, SecurityType, MacAddress};
use super::wpa::{WpaSupplicant, WpaConfig, WpaEvent};
use super::driver::WifiDriver;

/// Connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Not connected
    Disconnected,
    /// Switching to target channel
    SwitchingChannel,
    /// Sending authentication request
    Authenticating,
    /// WPA/WPA2 4-way handshake
    FourWayHandshake,
    /// Sending association request
    Associating,
    /// Waiting for DHCP
    ObtainingIp,
    /// Fully connected
    Connected,
    /// Disconnecting
    Disconnecting,
    /// Connection failed
    Failed,
}

/// Connection failure reason
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionFailure {
    /// Timeout waiting for response
    Timeout,
    /// Authentication failed (wrong password)
    AuthFailed,
    /// Association rejected
    AssocRejected,
    /// Network not found
    NetworkNotFound,
    /// DHCP failed
    DhcpFailed,
    /// Driver error
    DriverError,
    /// User cancelled
    Cancelled,
    /// Deauthenticated by AP
    Deauthenticated,
    /// Invalid security configuration
    SecurityMismatch,
}

/// Connection event for callbacks
#[derive(Debug, Clone)]
pub enum ConnectionEvent {
    /// State changed
    StateChanged(ConnectionState),
    /// Authentication progress
    AuthProgress(u8),
    /// IP address obtained
    IpObtained {
        ip: [u8; 4],
        netmask: [u8; 4],
        gateway: [u8; 4],
    },
    /// Disconnected
    Disconnected(ConnectionFailure),
    /// Signal strength changed
    SignalChanged(i8),
}

/// Network profile for saved networks
#[derive(Debug, Clone)]
pub struct NetworkProfile {
    /// SSID
    pub ssid: String,
    /// Password (encrypted in a real system)
    pub password: Option<String>,
    /// Expected security type
    pub security: SecurityType,
    /// Priority (higher = prefer)
    pub priority: u8,
    /// Auto-connect when in range
    pub auto_connect: bool,
    /// Hidden network (requires active scan)
    pub hidden: bool,
    /// BSSID lock (connect only to specific AP)
    pub bssid_lock: Option<MacAddress>,
    /// Last successful connection timestamp
    pub last_connected: u64,
}

impl NetworkProfile {
    /// Create a new network profile
    pub fn new(ssid: String, password: Option<String>, security: SecurityType) -> Self {
        Self {
            ssid,
            password,
            security,
            priority: 0,
            auto_connect: true,
            hidden: false,
            bssid_lock: None,
            last_connected: 0,
        }
    }
}

/// Connection configuration
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    /// Connection timeout in milliseconds
    pub timeout_ms: u64,
    /// Authentication timeout
    pub auth_timeout_ms: u64,
    /// Association timeout
    pub assoc_timeout_ms: u64,
    /// DHCP timeout
    pub dhcp_timeout_ms: u64,
    /// Number of retries
    pub max_retries: u8,
    /// Enable background scanning while connected
    pub background_scan: bool,
    /// Roaming threshold (dBm)
    pub roam_threshold: i8,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            timeout_ms: 30000,
            auth_timeout_ms: 5000,
            assoc_timeout_ms: 5000,
            dhcp_timeout_ms: 15000,
            max_retries: 3,
            background_scan: true,
            roam_threshold: -70,
        }
    }
}

/// Connection statistics
#[derive(Debug, Clone, Default)]
pub struct ConnectionStats {
    /// Total connection attempts
    pub attempts: u64,
    /// Successful connections
    pub successes: u64,
    /// Failed connections
    pub failures: u64,
    /// Total bytes sent
    pub tx_bytes: u64,
    /// Total bytes received
    pub rx_bytes: u64,
    /// Total packets sent
    pub tx_packets: u64,
    /// Total packets received
    pub rx_packets: u64,
    /// Current connection uptime (ticks)
    pub uptime: u64,
    /// Average signal strength
    pub avg_signal: i8,
}

/// Connection manager
pub struct ConnectionManager {
    /// Current state
    state: ConnectionState,
    /// Current target network
    target_network: Option<WifiNetwork>,
    /// Current password
    password: Option<String>,
    /// Configuration
    config: ConnectionConfig,
    /// WPA supplicant
    wpa: Option<WpaSupplicant>,
    /// Connection start time
    start_time: u64,
    /// Current retry count
    retry_count: u8,
    /// Last failure reason
    last_failure: Option<ConnectionFailure>,
    /// Statistics
    stats: ConnectionStats,
    /// IP configuration
    ip_config: Option<IpConfig>,
    /// Connection time (when entered Connected state)
    connected_time: u64,
}

/// IP configuration
#[derive(Debug, Clone)]
pub struct IpConfig {
    pub ip: [u8; 4],
    pub netmask: [u8; 4],
    pub gateway: [u8; 4],
    pub dns: [[u8; 4]; 2],
}

impl ConnectionManager {
    /// Create a new connection manager
    pub fn new() -> Self {
        Self {
            state: ConnectionState::Disconnected,
            target_network: None,
            password: None,
            config: ConnectionConfig::default(),
            wpa: None,
            start_time: 0,
            retry_count: 0,
            last_failure: None,
            stats: ConnectionStats::default(),
            ip_config: None,
            connected_time: 0,
        }
    }

    /// Get current state
    pub fn state(&self) -> ConnectionState {
        self.state
    }

    /// Get target network
    pub fn target_network(&self) -> Option<&WifiNetwork> {
        self.target_network.as_ref()
    }

    /// Get IP configuration
    pub fn ip_config(&self) -> Option<&IpConfig> {
        self.ip_config.as_ref()
    }

    /// Get statistics
    pub fn stats(&self) -> &ConnectionStats {
        &self.stats
    }

    /// Get last failure reason
    pub fn last_failure(&self) -> Option<ConnectionFailure> {
        self.last_failure
    }

    /// Start connection to a network
    pub fn connect(
        &mut self,
        network: WifiNetwork,
        password: Option<String>,
        driver: &Arc<dyn WifiDriver>,
    ) -> KResult<()> {
        if self.state != ConnectionState::Disconnected &&
           self.state != ConnectionState::Failed {
            return Err(KError::Busy);
        }

        // Validate security configuration
        if network.is_encrypted() && password.is_none() {
            return Err(KError::Invalid);
        }

        self.stats.attempts += 1;
        self.start_time = get_ticks();
        self.retry_count = 0;
        self.last_failure = None;

        // Set up WPA if needed
        if matches!(network.security, SecurityType::WpaPsk | SecurityType::Wpa2Psk) {
            if let Some(ref pass) = password {
                let wpa_config = WpaConfig {
                    ssid: network.ssid.clone(),
                    passphrase: pass.clone(),
                    ..Default::default()
                };
                let mut wpa = WpaSupplicant::new();
                wpa.configure(wpa_config);
                self.wpa = Some(wpa);
            }
        }

        self.target_network = Some(network.clone());
        self.password = password;

        // Switch to target channel
        if let Some(ref channel) = network.channel {
            driver.set_channel(channel)?;
        }

        self.state = ConnectionState::SwitchingChannel;

        // Move to authentication
        self.start_authentication(driver)?;

        Ok(())
    }

    /// Start authentication
    fn start_authentication(&mut self, driver: &Arc<dyn WifiDriver>) -> KResult<()> {
        let network = self.target_network.as_ref().ok_or(KError::Invalid)?;

        self.state = ConnectionState::Authenticating;

        // For open networks, skip authentication
        if network.security == SecurityType::Open {
            self.start_association(driver)?;
            return Ok(());
        }

        // Send authentication frame (Open System for WPA)
        driver.authenticate(&network.bssid)?;

        Ok(())
    }

    /// Start WPA 4-way handshake
    fn start_four_way_handshake(&mut self) -> KResult<()> {
        self.state = ConnectionState::FourWayHandshake;

        if let Some(ref mut wpa) = self.wpa {
            wpa.start()?;
        }

        Ok(())
    }

    /// Start association
    fn start_association(&mut self, driver: &Arc<dyn WifiDriver>) -> KResult<()> {
        let network = self.target_network.as_ref().ok_or(KError::Invalid)?;

        self.state = ConnectionState::Associating;

        // Send association request
        driver.associate(&network.bssid, &network.ssid)?;

        Ok(())
    }

    /// Start DHCP
    fn start_dhcp(&mut self) -> KResult<()> {
        self.state = ConnectionState::ObtainingIp;

        // Request IP from DHCP
        // This integrates with the DHCP client in the network stack
        crate::net::dhcp::request()?;

        Ok(())
    }

    /// Complete connection
    fn complete_connection(&mut self) {
        self.state = ConnectionState::Connected;
        self.connected_time = get_ticks();
        self.stats.successes += 1;
    }

    /// Handle authentication response
    pub fn on_auth_response(&mut self, success: bool, driver: &Arc<dyn WifiDriver>) -> KResult<()> {
        if self.state != ConnectionState::Authenticating {
            return Ok(());
        }

        if success {
            let network = self.target_network.as_ref().ok_or(KError::Invalid)?;

            if matches!(network.security, SecurityType::WpaPsk | SecurityType::Wpa2Psk) {
                // Start 4-way handshake
                self.start_four_way_handshake()?;
            } else {
                // Move to association
                self.start_association(driver)?;
            }
        } else {
            self.fail(ConnectionFailure::AuthFailed);
        }

        Ok(())
    }

    /// Handle EAPOL frame (for WPA)
    pub fn on_eapol_frame(&mut self, data: &[u8], driver: &Arc<dyn WifiDriver>) -> KResult<()> {
        if self.state != ConnectionState::FourWayHandshake {
            return Ok(());
        }

        if let Some(ref mut wpa) = self.wpa {
            let event = wpa.process_eapol(data)?;

            match event {
                WpaEvent::Message2Ready => {
                    // Send Message 2
                    if let Some(msg) = wpa.get_outgoing_message() {
                        let network = self.target_network.as_ref().ok_or(KError::Invalid)?;
                        driver.send_eapol(&network.bssid, &msg)?;
                    }
                }
                WpaEvent::Message4Ready => {
                    // Send Message 4
                    if let Some(msg) = wpa.get_outgoing_message() {
                        let network = self.target_network.as_ref().ok_or(KError::Invalid)?;
                        driver.send_eapol(&network.bssid, &msg)?;
                    }
                }
                WpaEvent::Complete => {
                    // Handshake complete, install keys
                    if let (Some(ptk), Some(gtk)) = (wpa.get_ptk(), wpa.get_gtk()) {
                        driver.install_key(&ptk.tk, false)?; // Pairwise key
                        driver.install_key(gtk, true)?; // Group key
                    }

                    // Move to association (or directly to DHCP if already associated)
                    self.start_dhcp()?;
                }
                WpaEvent::Failed => {
                    self.fail(ConnectionFailure::AuthFailed);
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Handle association response
    pub fn on_assoc_response(&mut self, success: bool, aid: u16) -> KResult<()> {
        if self.state != ConnectionState::Associating {
            return Ok(());
        }

        if success {
            let network = self.target_network.as_ref().ok_or(KError::Invalid)?;

            if matches!(network.security, SecurityType::WpaPsk | SecurityType::Wpa2Psk) {
                // WPA needs 4-way handshake first (already done, so go to DHCP)
                self.start_dhcp()?;
            } else {
                // Open network, go directly to DHCP
                self.start_dhcp()?;
            }
        } else {
            self.fail(ConnectionFailure::AssocRejected);
        }

        Ok(())
    }

    /// Handle DHCP completion
    pub fn on_dhcp_complete(&mut self, config: IpConfig) {
        if self.state != ConnectionState::ObtainingIp {
            return;
        }

        self.ip_config = Some(config);
        self.complete_connection();
    }

    /// Handle DHCP failure
    pub fn on_dhcp_failed(&mut self) {
        if self.state != ConnectionState::ObtainingIp {
            return;
        }

        self.fail(ConnectionFailure::DhcpFailed);
    }

    /// Handle deauthentication
    pub fn on_deauth(&mut self, _reason: u16) {
        if self.state == ConnectionState::Connected {
            self.state = ConnectionState::Disconnected;
            self.last_failure = Some(ConnectionFailure::Deauthenticated);
        }
    }

    /// Handle disassociation
    pub fn on_disassoc(&mut self, _reason: u16) {
        if self.state == ConnectionState::Connected {
            self.state = ConnectionState::Disconnected;
            self.last_failure = Some(ConnectionFailure::Deauthenticated);
        }
    }

    /// Poll for timeouts
    pub fn poll(&mut self, driver: &Arc<dyn WifiDriver>) {
        let now = get_ticks();
        let elapsed = now.saturating_sub(self.start_time);

        match self.state {
            ConnectionState::Authenticating => {
                if elapsed > self.config.auth_timeout_ms {
                    self.handle_timeout(driver);
                }
            }
            ConnectionState::FourWayHandshake => {
                if elapsed > self.config.auth_timeout_ms {
                    self.handle_timeout(driver);
                }
            }
            ConnectionState::Associating => {
                if elapsed > self.config.assoc_timeout_ms {
                    self.handle_timeout(driver);
                }
            }
            ConnectionState::ObtainingIp => {
                if elapsed > self.config.dhcp_timeout_ms {
                    self.fail(ConnectionFailure::DhcpFailed);
                }
            }
            ConnectionState::Connected => {
                // Update uptime
                self.stats.uptime = now.saturating_sub(self.connected_time);
            }
            _ => {}
        }
    }

    /// Handle timeout
    fn handle_timeout(&mut self, driver: &Arc<dyn WifiDriver>) {
        self.retry_count += 1;

        if self.retry_count >= self.config.max_retries {
            self.fail(ConnectionFailure::Timeout);
        } else {
            // Retry
            self.start_time = get_ticks();
            match self.state {
                ConnectionState::Authenticating => {
                    let _ = self.start_authentication(driver);
                }
                ConnectionState::Associating => {
                    let _ = self.start_association(driver);
                }
                _ => {}
            }
        }
    }

    /// Mark connection as failed
    fn fail(&mut self, reason: ConnectionFailure) {
        self.state = ConnectionState::Failed;
        self.last_failure = Some(reason);
        self.stats.failures += 1;
        self.wpa = None;
    }

    /// Disconnect
    pub fn disconnect(&mut self, driver: &Arc<dyn WifiDriver>) -> KResult<()> {
        if self.state == ConnectionState::Disconnected {
            return Ok(());
        }

        self.state = ConnectionState::Disconnecting;

        // Send deauthentication frame
        if let Some(ref network) = self.target_network {
            let _ = driver.deauthenticate(&network.bssid);
        }

        self.state = ConnectionState::Disconnected;
        self.target_network = None;
        self.password = None;
        self.wpa = None;
        self.ip_config = None;

        Ok(())
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        self.state == ConnectionState::Connected
    }

    /// Get connection uptime
    pub fn uptime(&self) -> u64 {
        if self.state == ConnectionState::Connected {
            get_ticks().saturating_sub(self.connected_time)
        } else {
            0
        }
    }
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Saved network profile manager
pub struct ProfileManager {
    /// Saved profiles
    profiles: BTreeMap<String, NetworkProfile>,
    /// Maximum profiles to store
    max_profiles: usize,
}

impl ProfileManager {
    /// Create a new profile manager
    pub fn new() -> Self {
        Self {
            profiles: BTreeMap::new(),
            max_profiles: 32,
        }
    }

    /// Add or update a profile
    pub fn save(&mut self, profile: NetworkProfile) -> KResult<()> {
        if self.profiles.len() >= self.max_profiles && !self.profiles.contains_key(&profile.ssid) {
            // Remove lowest priority profile
            if let Some(ssid) = self.lowest_priority_ssid() {
                self.profiles.remove(&ssid);
            }
        }

        self.profiles.insert(profile.ssid.clone(), profile);
        Ok(())
    }

    /// Get a profile by SSID
    pub fn get(&self, ssid: &str) -> Option<&NetworkProfile> {
        self.profiles.get(ssid)
    }

    /// Remove a profile
    pub fn remove(&mut self, ssid: &str) -> Option<NetworkProfile> {
        self.profiles.remove(ssid)
    }

    /// Get all profiles
    pub fn all(&self) -> Vec<&NetworkProfile> {
        self.profiles.values().collect()
    }

    /// Get profiles sorted by priority
    pub fn by_priority(&self) -> Vec<&NetworkProfile> {
        let mut profiles: Vec<_> = self.profiles.values().collect();
        profiles.sort_by(|a, b| b.priority.cmp(&a.priority));
        profiles
    }

    /// Get auto-connect profiles
    pub fn auto_connect(&self) -> Vec<&NetworkProfile> {
        self.profiles.values()
            .filter(|p| p.auto_connect)
            .collect()
    }

    /// Find lowest priority SSID for eviction
    fn lowest_priority_ssid(&self) -> Option<String> {
        self.profiles.iter()
            .min_by_key(|(_, p)| (p.priority, p.last_connected))
            .map(|(s, _)| s.clone())
    }

    /// Update last connected time
    pub fn touch(&mut self, ssid: &str) {
        if let Some(profile) = self.profiles.get_mut(ssid) {
            profile.last_connected = get_ticks();
        }
    }

    /// Check if profile exists
    pub fn contains(&self, ssid: &str) -> bool {
        self.profiles.contains_key(ssid)
    }

    /// Get profile count
    pub fn count(&self) -> usize {
        self.profiles.len()
    }

    /// Clear all profiles
    pub fn clear(&mut self) {
        self.profiles.clear();
    }
}

impl Default for ProfileManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Auto-connect manager
pub struct AutoConnect {
    /// Profile manager reference
    profiles: ProfileManager,
    /// Whether auto-connect is enabled
    enabled: bool,
    /// Last scan time
    last_scan: u64,
    /// Scan interval (ms)
    scan_interval: u64,
    /// Connection in progress
    connecting: bool,
}

impl AutoConnect {
    /// Create new auto-connect manager
    pub fn new(profiles: ProfileManager) -> Self {
        Self {
            profiles,
            enabled: true,
            last_scan: 0,
            scan_interval: 60000, // 1 minute
            connecting: false,
        }
    }

    /// Enable auto-connect
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disable auto-connect
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Check if enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Set scan interval
    pub fn set_scan_interval(&mut self, interval_ms: u64) {
        self.scan_interval = interval_ms;
    }

    /// Process auto-connect logic
    pub fn process(
        &mut self,
        available_networks: &[WifiNetwork],
        connection_manager: &mut ConnectionManager,
        driver: &Arc<dyn WifiDriver>,
    ) -> KResult<()> {
        if !self.enabled || self.connecting {
            return Ok(());
        }

        // Already connected?
        if connection_manager.is_connected() {
            return Ok(());
        }

        // Find best matching profile
        let profiles = self.profiles.by_priority();

        for profile in profiles {
            if !profile.auto_connect {
                continue;
            }

            // Find network matching profile
            let matching = available_networks.iter()
                .filter(|n| n.ssid == profile.ssid)
                .filter(|n| {
                    // Check BSSID lock if set
                    profile.bssid_lock.map_or(true, |bssid| n.bssid == bssid)
                })
                .max_by_key(|n| n.signal_strength);

            if let Some(network) = matching {
                // Attempt connection
                self.connecting = true;
                let result = connection_manager.connect(
                    network.clone(),
                    profile.password.clone(),
                    driver,
                );

                if result.is_ok() {
                    return Ok(());
                }
                self.connecting = false;
            }
        }

        Ok(())
    }

    /// Notify connection complete
    pub fn on_connected(&mut self, ssid: &str) {
        self.connecting = false;
        self.profiles.touch(ssid);
    }

    /// Notify connection failed
    pub fn on_failed(&mut self) {
        self.connecting = false;
    }

    /// Get profiles
    pub fn profiles(&self) -> &ProfileManager {
        &self.profiles
    }

    /// Get mutable profiles
    pub fn profiles_mut(&mut self) -> &mut ProfileManager {
        &mut self.profiles
    }
}

/// Roaming manager for seamless AP switching
pub struct RoamingManager {
    /// Roaming threshold (dBm)
    threshold: i8,
    /// Hysteresis (dBm)
    hysteresis: i8,
    /// Last signal reading
    last_signal: i8,
    /// Roaming enabled
    enabled: bool,
    /// Minimum time between roams (ms)
    min_interval: u64,
    /// Last roam time
    last_roam: u64,
}

impl RoamingManager {
    /// Create new roaming manager
    pub fn new() -> Self {
        Self {
            threshold: -70,
            hysteresis: 5,
            last_signal: 0,
            enabled: true,
            min_interval: 10000,
            last_roam: 0,
        }
    }

    /// Set roaming threshold
    pub fn set_threshold(&mut self, threshold: i8) {
        self.threshold = threshold;
    }

    /// Enable roaming
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disable roaming
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Check if should roam
    pub fn should_roam<'a>(
        &self,
        current_signal: i8,
        current_bssid: &MacAddress,
        candidates: &'a [WifiNetwork],
    ) -> Option<&'a WifiNetwork> {
        if !self.enabled {
            return None;
        }

        // Check if current signal is below threshold
        if current_signal >= self.threshold {
            return None;
        }

        // Check minimum interval
        let now = get_ticks();
        if now.saturating_sub(self.last_roam) < self.min_interval {
            return None;
        }

        // Find better candidate
        candidates.iter()
            .filter(|n| n.bssid != *current_bssid)
            .filter(|n| n.signal_strength > current_signal + self.hysteresis)
            .max_by_key(|n| n.signal_strength)
    }

    /// Notify roam occurred
    pub fn on_roam(&mut self) {
        self.last_roam = get_ticks();
    }

    /// Update signal
    pub fn update_signal(&mut self, signal: i8) {
        self.last_signal = signal;
    }
}

impl Default for RoamingManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global connection manager (lazy initialized)
static CONNECTION_MANAGER: IrqSafeMutex<Option<ConnectionManager>> =
    IrqSafeMutex::new(None);

/// Global profile manager (lazy initialized)
static PROFILE_MANAGER: IrqSafeMutex<Option<ProfileManager>> =
    IrqSafeMutex::new(None);

/// Get or initialize connection manager
fn get_connection_manager() -> &'static IrqSafeMutex<Option<ConnectionManager>> {
    let mut guard = CONNECTION_MANAGER.lock();
    if guard.is_none() {
        *guard = Some(ConnectionManager::new());
    }
    drop(guard);
    &CONNECTION_MANAGER
}

/// Get or initialize profile manager
fn get_profile_manager() -> &'static IrqSafeMutex<Option<ProfileManager>> {
    let mut guard = PROFILE_MANAGER.lock();
    if guard.is_none() {
        *guard = Some(ProfileManager::new());
    }
    drop(guard);
    &PROFILE_MANAGER
}

/// Connect to a network
pub fn connect_to_network(
    network: WifiNetwork,
    password: Option<String>,
    driver: Arc<dyn WifiDriver>,
) -> KResult<()> {
    let mut guard = get_connection_manager().lock();
    guard.as_mut().ok_or(KError::NotSupported)?.connect(network, password, &driver)
}

/// Disconnect from current network
pub fn disconnect_from_network(driver: Arc<dyn WifiDriver>) -> KResult<()> {
    let mut guard = get_connection_manager().lock();
    guard.as_mut().ok_or(KError::NotSupported)?.disconnect(&driver)
}

/// Get current connection state
pub fn connection_state() -> ConnectionState {
    let guard = get_connection_manager().lock();
    guard.as_ref().map(|m| m.state()).unwrap_or(ConnectionState::Disconnected)
}

/// Check if connected
pub fn is_connected() -> bool {
    let guard = get_connection_manager().lock();
    guard.as_ref().map(|m| m.is_connected()).unwrap_or(false)
}

/// Get IP configuration
pub fn get_ip_config() -> Option<IpConfig> {
    let guard = get_connection_manager().lock();
    guard.as_ref().and_then(|m| m.ip_config().cloned())
}

/// Get connection statistics
pub fn get_connection_stats() -> ConnectionStats {
    let guard = get_connection_manager().lock();
    guard.as_ref().map(|m| m.stats().clone()).unwrap_or_default()
}

/// Save network profile
pub fn save_profile(profile: NetworkProfile) -> KResult<()> {
    let mut guard = get_profile_manager().lock();
    guard.as_mut().ok_or(KError::NotSupported)?.save(profile)
}

/// Get saved profile
pub fn get_profile(ssid: &str) -> Option<NetworkProfile> {
    let guard = get_profile_manager().lock();
    guard.as_ref().and_then(|m| m.get(ssid).cloned())
}

/// Remove saved profile
pub fn remove_profile(ssid: &str) -> Option<NetworkProfile> {
    let mut guard = get_profile_manager().lock();
    guard.as_mut().and_then(|m| m.remove(ssid))
}

/// List all saved profiles
pub fn list_profiles() -> Vec<String> {
    let guard = get_profile_manager().lock();
    guard.as_ref()
        .map(|m| m.all().iter().map(|p| p.ssid.clone()).collect())
        .unwrap_or_default()
}

/// Process incoming EAPOL frame
pub fn process_eapol(data: &[u8], driver: Arc<dyn WifiDriver>) -> KResult<()> {
    let mut guard = get_connection_manager().lock();
    guard.as_mut().ok_or(KError::NotSupported)?.on_eapol_frame(data, &driver)
}

/// Notify authentication response
pub fn on_auth_response(success: bool, driver: Arc<dyn WifiDriver>) -> KResult<()> {
    let mut guard = get_connection_manager().lock();
    guard.as_mut().ok_or(KError::NotSupported)?.on_auth_response(success, &driver)
}

/// Notify association response
pub fn on_assoc_response(success: bool, aid: u16) -> KResult<()> {
    let mut guard = get_connection_manager().lock();
    guard.as_mut().ok_or(KError::NotSupported)?.on_assoc_response(success, aid)
}

/// Notify DHCP complete
pub fn on_dhcp_complete(ip: [u8; 4], netmask: [u8; 4], gateway: [u8; 4], dns: [[u8; 4]; 2]) {
    let config = IpConfig { ip, netmask, gateway, dns };
    let mut guard = get_connection_manager().lock();
    if let Some(m) = guard.as_mut() {
        m.on_dhcp_complete(config);
    }
}

/// Notify DHCP failed
pub fn on_dhcp_failed() {
    let mut guard = get_connection_manager().lock();
    if let Some(m) = guard.as_mut() {
        m.on_dhcp_failed();
    }
}

/// Notify deauthentication
pub fn on_deauth(reason: u16) {
    let mut guard = get_connection_manager().lock();
    if let Some(m) = guard.as_mut() {
        m.on_deauth(reason);
    }
}

/// Notify disassociation
pub fn on_disassoc(reason: u16) {
    let mut guard = get_connection_manager().lock();
    if let Some(m) = guard.as_mut() {
        m.on_disassoc(reason);
    }
}

/// Poll connection manager
pub fn poll(driver: Arc<dyn WifiDriver>) {
    let mut guard = get_connection_manager().lock();
    if let Some(m) = guard.as_mut() {
        m.poll(&driver);
    }
}
