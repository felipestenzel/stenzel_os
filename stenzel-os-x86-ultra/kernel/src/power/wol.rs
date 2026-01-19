//! Wake on LAN (WoL) support.
//!
//! Provides:
//! - WoL magic packet sending
//! - WoL configuration for network interfaces
//! - SecureOn password support
//! - Wake pattern configuration
//! - Wake event logging

use alloc::vec::Vec;
use alloc::vec;
use alloc::string::String;
use alloc::format;
use core::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use crate::sync::IrqSafeMutex;
use crate::util::{KResult, KError};

/// Magic packet sync stream
const MAGIC_SYNC: [u8; 6] = [0xFF; 6];
/// Magic packet repeat count
const MAC_REPEAT_COUNT: usize = 16;
/// Default WoL port (UDP)
const DEFAULT_WOL_PORT: u16 = 9;
/// SecureOn password length
const SECUREON_PASSWORD_LEN: usize = 6;

/// MAC address
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MacAddress([u8; 6]);

impl MacAddress {
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

    pub fn from_string(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split(|c| c == ':' || c == '-').collect();
        if parts.len() != 6 {
            return None;
        }

        let mut addr = [0u8; 6];
        for (i, part) in parts.iter().enumerate() {
            addr[i] = u8::from_str_radix(part, 16).ok()?;
        }

        Some(Self(addr))
    }

    pub fn as_bytes(&self) -> &[u8; 6] {
        &self.0
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0; 6]
    }

    pub fn is_broadcast(&self) -> bool {
        self.0 == [0xFF; 6]
    }

    pub fn to_string(&self) -> String {
        format!("{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            self.0[0], self.0[1], self.0[2],
            self.0[3], self.0[4], self.0[5])
    }
}

/// WoL mode flags
#[derive(Debug, Clone, Copy, Default)]
pub struct WolMode {
    /// PHY activity wake
    pub phy_activity: bool,
    /// Unicast wake
    pub unicast: bool,
    /// Multicast wake
    pub multicast: bool,
    /// Broadcast wake
    pub broadcast: bool,
    /// ARP wake
    pub arp: bool,
    /// Magic packet wake
    pub magic: bool,
    /// Magic packet with SecureOn password
    pub magic_secure: bool,
}

impl WolMode {
    /// No wake sources
    pub fn disabled() -> Self {
        Self::default()
    }

    /// Wake on magic packet only
    pub fn magic_packet() -> Self {
        Self {
            magic: true,
            ..Default::default()
        }
    }

    /// Wake on magic packet with SecureOn
    pub fn magic_secure() -> Self {
        Self {
            magic: true,
            magic_secure: true,
            ..Default::default()
        }
    }

    /// Wake on any activity
    pub fn any() -> Self {
        Self {
            phy_activity: true,
            unicast: true,
            multicast: true,
            broadcast: true,
            arp: true,
            magic: true,
            magic_secure: false,
        }
    }

    /// Convert to ethtool-style flags
    pub fn to_flags(&self) -> u32 {
        let mut flags = 0u32;
        if self.phy_activity { flags |= 0x01; }
        if self.unicast { flags |= 0x02; }
        if self.multicast { flags |= 0x04; }
        if self.broadcast { flags |= 0x08; }
        if self.arp { flags |= 0x10; }
        if self.magic { flags |= 0x20; }
        if self.magic_secure { flags |= 0x40; }
        flags
    }

    /// Create from ethtool-style flags
    pub fn from_flags(flags: u32) -> Self {
        Self {
            phy_activity: (flags & 0x01) != 0,
            unicast: (flags & 0x02) != 0,
            multicast: (flags & 0x04) != 0,
            broadcast: (flags & 0x08) != 0,
            arp: (flags & 0x10) != 0,
            magic: (flags & 0x20) != 0,
            magic_secure: (flags & 0x40) != 0,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.phy_activity || self.unicast || self.multicast ||
        self.broadcast || self.arp || self.magic || self.magic_secure
    }
}

/// Wake pattern type
#[derive(Debug, Clone)]
pub enum WakePattern {
    /// Exact byte match
    Exact(Vec<u8>),
    /// Pattern with mask (1 = care, 0 = don't care)
    Masked {
        pattern: Vec<u8>,
        mask: Vec<u8>,
    },
    /// IPv4 packet
    Ipv4,
    /// IPv6 packet
    Ipv6,
    /// TCP SYN packet
    TcpSyn {
        port: Option<u16>,
    },
}

impl WakePattern {
    pub fn tcp_syn_any() -> Self {
        WakePattern::TcpSyn { port: None }
    }

    pub fn tcp_syn_port(port: u16) -> Self {
        WakePattern::TcpSyn { port: Some(port) }
    }
}

/// WoL configuration for an interface
#[derive(Debug, Clone)]
pub struct WolConfig {
    /// Interface name
    pub interface: String,
    /// MAC address
    pub mac_address: MacAddress,
    /// Enabled modes
    pub mode: WolMode,
    /// SecureOn password (6 bytes)
    pub secure_on_password: Option<[u8; 6]>,
    /// Wake patterns
    pub patterns: Vec<WakePattern>,
    /// Wake on link change
    pub wake_on_link: bool,
}

impl WolConfig {
    pub fn new(interface: &str, mac: MacAddress) -> Self {
        Self {
            interface: String::from(interface),
            mac_address: mac,
            mode: WolMode::disabled(),
            secure_on_password: None,
            patterns: Vec::new(),
            wake_on_link: false,
        }
    }

    pub fn enable_magic_packet(&mut self) {
        self.mode.magic = true;
    }

    pub fn set_secure_on(&mut self, password: [u8; 6]) {
        self.mode.magic_secure = true;
        self.secure_on_password = Some(password);
    }

    pub fn add_pattern(&mut self, pattern: WakePattern) {
        self.patterns.push(pattern);
    }
}

/// Magic packet builder
pub struct MagicPacket {
    /// Target MAC address
    target_mac: MacAddress,
    /// SecureOn password
    secure_on: Option<[u8; 6]>,
}

impl MagicPacket {
    pub fn new(target_mac: MacAddress) -> Self {
        Self {
            target_mac,
            secure_on: None,
        }
    }

    pub fn with_secure_on(target_mac: MacAddress, password: [u8; 6]) -> Self {
        Self {
            target_mac,
            secure_on: Some(password),
        }
    }

    /// Build the magic packet payload
    pub fn build(&self) -> Vec<u8> {
        let base_len = 6 + 6 * MAC_REPEAT_COUNT;
        let total_len = if self.secure_on.is_some() {
            base_len + SECUREON_PASSWORD_LEN
        } else {
            base_len
        };

        let mut packet = Vec::with_capacity(total_len);

        // Sync stream (6 bytes of 0xFF)
        packet.extend_from_slice(&MAGIC_SYNC);

        // Target MAC repeated 16 times
        for _ in 0..MAC_REPEAT_COUNT {
            packet.extend_from_slice(self.target_mac.as_bytes());
        }

        // SecureOn password if present
        if let Some(ref password) = self.secure_on {
            packet.extend_from_slice(password);
        }

        packet
    }

    /// Build as UDP packet for sending
    pub fn build_udp(&self, src_port: u16) -> Vec<u8> {
        let payload = self.build();
        let total_len = 8 + payload.len(); // UDP header + payload

        let mut packet = Vec::with_capacity(total_len);

        // UDP header
        packet.push((src_port >> 8) as u8);
        packet.push(src_port as u8);
        packet.push((DEFAULT_WOL_PORT >> 8) as u8);
        packet.push(DEFAULT_WOL_PORT as u8);
        packet.push((total_len >> 8) as u8);
        packet.push(total_len as u8);
        packet.push(0); // Checksum (optional for IPv4)
        packet.push(0);

        packet.extend_from_slice(&payload);

        packet
    }
}

/// Wake event
#[derive(Debug, Clone)]
pub struct WakeEvent {
    /// Timestamp
    pub timestamp: u64,
    /// Interface that received wake
    pub interface: String,
    /// Wake reason
    pub reason: WakeReason,
    /// Source MAC (if available)
    pub source_mac: Option<MacAddress>,
    /// Source IP (if available)
    pub source_ip: Option<[u8; 4]>,
}

/// Wake reason
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WakeReason {
    Unknown,
    MagicPacket,
    MagicPacketSecure,
    UnicastPacket,
    MulticastPacket,
    BroadcastPacket,
    ArpPacket,
    Pattern,
    LinkChange,
    PhyActivity,
}

impl WakeReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            WakeReason::Unknown => "Unknown",
            WakeReason::MagicPacket => "Magic Packet",
            WakeReason::MagicPacketSecure => "Magic Packet (SecureOn)",
            WakeReason::UnicastPacket => "Unicast Packet",
            WakeReason::MulticastPacket => "Multicast Packet",
            WakeReason::BroadcastPacket => "Broadcast Packet",
            WakeReason::ArpPacket => "ARP Packet",
            WakeReason::Pattern => "Wake Pattern Match",
            WakeReason::LinkChange => "Link Change",
            WakeReason::PhyActivity => "PHY Activity",
        }
    }
}

/// WoL statistics
#[derive(Debug, Default)]
pub struct WolStats {
    /// Magic packets sent
    pub packets_sent: AtomicU64,
    /// Wake events received
    pub wake_events: AtomicU64,
    /// Configurations applied
    pub configs_applied: AtomicU64,
    /// Errors
    pub errors: AtomicU64,
}

impl WolStats {
    pub const fn new() -> Self {
        Self {
            packets_sent: AtomicU64::new(0),
            wake_events: AtomicU64::new(0),
            configs_applied: AtomicU64::new(0),
            errors: AtomicU64::new(0),
        }
    }
}

/// WoL manager
pub struct WolManager {
    /// Interface configurations
    configs: Vec<WolConfig>,
    /// Wake event log
    events: Vec<WakeEvent>,
    /// Maximum events to keep
    max_events: usize,
    /// Statistics
    stats: WolStats,
    /// Initialized
    initialized: AtomicBool,
}

impl WolManager {
    pub const fn new() -> Self {
        Self {
            configs: Vec::new(),
            events: Vec::new(),
            max_events: 100,
            stats: WolStats::new(),
            initialized: AtomicBool::new(false),
        }
    }

    /// Initialize WoL manager
    pub fn init(&mut self) {
        self.initialized.store(true, Ordering::SeqCst);
    }

    /// Configure WoL for an interface
    pub fn configure(&mut self, config: WolConfig) -> KResult<()> {
        // Find existing config or add new
        if let Some(existing) = self.configs.iter_mut()
            .find(|c| c.interface == config.interface)
        {
            *existing = config;
        } else {
            self.configs.push(config);
        }

        self.stats.configs_applied.fetch_add(1, Ordering::Relaxed);

        // Would apply to hardware here
        Ok(())
    }

    /// Get configuration for interface
    pub fn get_config(&self, interface: &str) -> Option<&WolConfig> {
        self.configs.iter().find(|c| c.interface == interface)
    }

    /// Enable WoL with magic packet on interface
    pub fn enable_magic(&mut self, interface: &str, mac: MacAddress) -> KResult<()> {
        let mut config = WolConfig::new(interface, mac);
        config.enable_magic_packet();
        self.configure(config)
    }

    /// Disable WoL on interface
    pub fn disable(&mut self, interface: &str) -> KResult<()> {
        if let Some(config) = self.configs.iter_mut()
            .find(|c| c.interface == interface)
        {
            config.mode = WolMode::disabled();
            config.patterns.clear();
            Ok(())
        } else {
            Err(KError::NotFound)
        }
    }

    /// Send magic packet to wake a machine
    pub fn send_wake_packet(&mut self, target_mac: MacAddress, secure_on: Option<[u8; 6]>) -> KResult<()> {
        let packet = if let Some(password) = secure_on {
            MagicPacket::with_secure_on(target_mac, password)
        } else {
            MagicPacket::new(target_mac)
        };

        let payload = packet.build();

        // Would send via broadcast
        // For now, increment stats
        let _ = payload; // Use the payload
        self.stats.packets_sent.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }

    /// Send magic packet to subnet broadcast
    pub fn send_wake_broadcast(&mut self, target_mac: MacAddress, broadcast_ip: [u8; 4]) -> KResult<()> {
        let packet = MagicPacket::new(target_mac);
        let _udp_payload = packet.build_udp(40000);

        // Would send via UDP to broadcast_ip:9
        let _ = broadcast_ip;
        self.stats.packets_sent.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }

    /// Record wake event
    pub fn record_wake_event(&mut self, interface: &str, reason: WakeReason) {
        self.stats.wake_events.fetch_add(1, Ordering::Relaxed);

        let event = WakeEvent {
            timestamp: 0, // Would get from timer
            interface: String::from(interface),
            reason,
            source_mac: None,
            source_ip: None,
        };

        self.events.push(event);

        // Trim if too many events
        while self.events.len() > self.max_events {
            self.events.remove(0);
        }
    }

    /// Get recent wake events
    pub fn events(&self) -> &[WakeEvent] {
        &self.events
    }

    /// Get last wake event
    pub fn last_wake_event(&self) -> Option<&WakeEvent> {
        self.events.last()
    }

    /// Clear wake events
    pub fn clear_events(&mut self) {
        self.events.clear();
    }

    /// Get all configured interfaces
    pub fn configured_interfaces(&self) -> Vec<&str> {
        self.configs.iter().map(|c| c.interface.as_str()).collect()
    }

    /// Check if interface has WoL enabled
    pub fn is_enabled(&self, interface: &str) -> bool {
        self.configs.iter()
            .find(|c| c.interface == interface)
            .map(|c| c.mode.is_enabled())
            .unwrap_or(false)
    }

    /// Get statistics
    pub fn stats(&self) -> WolStatsSnapshot {
        WolStatsSnapshot {
            packets_sent: self.stats.packets_sent.load(Ordering::Relaxed),
            wake_events: self.stats.wake_events.load(Ordering::Relaxed),
            configs_applied: self.stats.configs_applied.load(Ordering::Relaxed),
            errors: self.stats.errors.load(Ordering::Relaxed),
            configured_interfaces: self.configs.len(),
            event_log_size: self.events.len(),
        }
    }

    /// Format status
    pub fn format_status(&self) -> String {
        use core::fmt::Write;
        let mut s = String::new();

        let _ = writeln!(s, "Wake on LAN Status:");
        let _ = writeln!(s, "  Configured interfaces: {}", self.configs.len());

        for config in &self.configs {
            let _ = writeln!(s, "\n  {} ({}):",
                config.interface,
                config.mac_address.to_string());
            let _ = writeln!(s, "    Magic packet: {}",
                if config.mode.magic { "enabled" } else { "disabled" });
            let _ = writeln!(s, "    SecureOn: {}",
                if config.mode.magic_secure { "enabled" } else { "disabled" });
            let _ = writeln!(s, "    Patterns: {}", config.patterns.len());
        }

        if let Some(event) = self.events.last() {
            let _ = writeln!(s, "\nLast wake event:");
            let _ = writeln!(s, "  Interface: {}", event.interface);
            let _ = writeln!(s, "  Reason: {}", event.reason.as_str());
        }

        s
    }
}

/// Statistics snapshot
#[derive(Debug, Clone)]
pub struct WolStatsSnapshot {
    pub packets_sent: u64,
    pub wake_events: u64,
    pub configs_applied: u64,
    pub errors: u64,
    pub configured_interfaces: usize,
    pub event_log_size: usize,
}

/// Global WoL manager
static WOL_MANAGER: IrqSafeMutex<WolManager> = IrqSafeMutex::new(WolManager::new());

/// Initialize WoL subsystem
pub fn init() {
    WOL_MANAGER.lock().init();
}

/// Configure WoL for interface
pub fn configure(config: WolConfig) -> KResult<()> {
    WOL_MANAGER.lock().configure(config)
}

/// Enable magic packet WoL on interface
pub fn enable_magic(interface: &str, mac: MacAddress) -> KResult<()> {
    WOL_MANAGER.lock().enable_magic(interface, mac)
}

/// Disable WoL on interface
pub fn disable(interface: &str) -> KResult<()> {
    WOL_MANAGER.lock().disable(interface)
}

/// Send wake packet
pub fn send_wake_packet(target_mac: MacAddress, secure_on: Option<[u8; 6]>) -> KResult<()> {
    WOL_MANAGER.lock().send_wake_packet(target_mac, secure_on)
}

/// Send wake broadcast
pub fn send_wake_broadcast(target_mac: MacAddress, broadcast_ip: [u8; 4]) -> KResult<()> {
    WOL_MANAGER.lock().send_wake_broadcast(target_mac, broadcast_ip)
}

/// Record wake event
pub fn record_wake_event(interface: &str, reason: WakeReason) {
    WOL_MANAGER.lock().record_wake_event(interface, reason)
}

/// Check if interface has WoL enabled
pub fn is_enabled(interface: &str) -> bool {
    WOL_MANAGER.lock().is_enabled(interface)
}

/// Get statistics
pub fn stats() -> WolStatsSnapshot {
    WOL_MANAGER.lock().stats()
}

/// Format status
pub fn format_status() -> String {
    WOL_MANAGER.lock().format_status()
}

/// Create magic packet for sending
pub fn create_magic_packet(target_mac: [u8; 6]) -> Vec<u8> {
    MagicPacket::new(MacAddress::new(target_mac)).build()
}

/// Create magic packet with SecureOn password
pub fn create_magic_packet_secure(target_mac: [u8; 6], password: [u8; 6]) -> Vec<u8> {
    MagicPacket::with_secure_on(MacAddress::new(target_mac), password).build()
}
