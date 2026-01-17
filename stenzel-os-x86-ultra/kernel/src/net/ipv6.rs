//! IPv6 (Internet Protocol version 6)
//!
//! Implements IPv6 support including:
//! - IPv6 address representation
//! - IPv6 header parsing and building
//! - ICMPv6 basics
//! - Neighbor Discovery Protocol (NDP)
//!
//! References:
//! - RFC 8200: IPv6 Specification
//! - RFC 4291: IPv6 Addressing Architecture
//! - RFC 4443: ICMPv6
//! - RFC 4861: Neighbor Discovery

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use crate::sync::IrqSafeMutex;

// ============================================================================
// IPv6 Address
// ============================================================================

/// IPv6 address (128 bits)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Ipv6Addr(pub [u8; 16]);

impl Ipv6Addr {
    /// Unspecified address (::)
    pub const UNSPECIFIED: Self = Self([0; 16]);

    /// Loopback address (::1)
    pub const LOOPBACK: Self = Self([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

    /// All nodes multicast (ff02::1)
    pub const ALL_NODES: Self = Self([0xff, 0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

    /// All routers multicast (ff02::2)
    pub const ALL_ROUTERS: Self = Self([0xff, 0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2]);

    /// Create from 8 16-bit segments
    pub const fn new(a: u16, b: u16, c: u16, d: u16, e: u16, f: u16, g: u16, h: u16) -> Self {
        Self([
            (a >> 8) as u8, a as u8,
            (b >> 8) as u8, b as u8,
            (c >> 8) as u8, c as u8,
            (d >> 8) as u8, d as u8,
            (e >> 8) as u8, e as u8,
            (f >> 8) as u8, f as u8,
            (g >> 8) as u8, g as u8,
            (h >> 8) as u8, h as u8,
        ])
    }

    /// Create from bytes
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 16 {
            return None;
        }
        let mut arr = [0u8; 16];
        arr.copy_from_slice(&bytes[..16]);
        Some(Self(arr))
    }

    /// Get as bytes
    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    /// Check if unspecified (::)
    pub fn is_unspecified(&self) -> bool {
        *self == Self::UNSPECIFIED
    }

    /// Check if loopback (::1)
    pub fn is_loopback(&self) -> bool {
        *self == Self::LOOPBACK
    }

    /// Check if multicast (ff00::/8)
    pub fn is_multicast(&self) -> bool {
        self.0[0] == 0xff
    }

    /// Check if link-local (fe80::/10)
    pub fn is_link_local(&self) -> bool {
        self.0[0] == 0xfe && (self.0[1] & 0xc0) == 0x80
    }

    /// Check if site-local (fec0::/10) - deprecated but still exists
    pub fn is_site_local(&self) -> bool {
        self.0[0] == 0xfe && (self.0[1] & 0xc0) == 0xc0
    }

    /// Check if unique local (fc00::/7)
    pub fn is_unique_local(&self) -> bool {
        (self.0[0] & 0xfe) == 0xfc
    }

    /// Check if global unicast
    pub fn is_global(&self) -> bool {
        !self.is_unspecified()
            && !self.is_loopback()
            && !self.is_multicast()
            && !self.is_link_local()
            && !self.is_unique_local()
    }

    /// Get the solicited-node multicast address for this unicast address
    /// ff02::1:ffXX:XXXX where XX:XXXX are the last 24 bits
    pub fn solicited_node_multicast(&self) -> Self {
        Self([
            0xff, 0x02, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 1, 0xff, self.0[13], self.0[14], self.0[15],
        ])
    }

    /// Create link-local address from MAC address (EUI-64)
    pub fn from_mac_link_local(mac: &[u8; 6]) -> Self {
        Self([
            0xfe, 0x80, 0, 0, 0, 0, 0, 0,
            mac[0] ^ 0x02, mac[1], mac[2], 0xff,
            0xfe, mac[3], mac[4], mac[5],
        ])
    }

    /// Get the 8 16-bit segments
    pub fn segments(&self) -> [u16; 8] {
        [
            u16::from_be_bytes([self.0[0], self.0[1]]),
            u16::from_be_bytes([self.0[2], self.0[3]]),
            u16::from_be_bytes([self.0[4], self.0[5]]),
            u16::from_be_bytes([self.0[6], self.0[7]]),
            u16::from_be_bytes([self.0[8], self.0[9]]),
            u16::from_be_bytes([self.0[10], self.0[11]]),
            u16::from_be_bytes([self.0[12], self.0[13]]),
            u16::from_be_bytes([self.0[14], self.0[15]]),
        ]
    }
}

impl fmt::Display for Ipv6Addr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let segs = self.segments();

        // Find the longest run of zeros for :: compression
        let mut best_start = 0;
        let mut best_len = 0;
        let mut cur_start = 0;
        let mut cur_len = 0;

        for (i, &seg) in segs.iter().enumerate() {
            if seg == 0 {
                if cur_len == 0 {
                    cur_start = i;
                }
                cur_len += 1;
            } else {
                if cur_len > best_len {
                    best_start = cur_start;
                    best_len = cur_len;
                }
                cur_len = 0;
            }
        }
        if cur_len > best_len {
            best_start = cur_start;
            best_len = cur_len;
        }

        // Only compress if at least 2 consecutive zeros
        if best_len < 2 {
            best_start = 8;
            best_len = 0;
        }

        let mut first = true;
        for i in 0..8 {
            if i == best_start {
                if first {
                    write!(f, ":")?;
                }
                write!(f, ":")?;
                first = false;
            } else if i >= best_start && i < best_start + best_len {
                // Skip
            } else {
                if !first {
                    write!(f, ":")?;
                }
                write!(f, "{:x}", segs[i])?;
                first = false;
            }
        }

        Ok(())
    }
}

// ============================================================================
// IPv6 Header
// ============================================================================

/// IPv6 Next Header / Protocol values
pub mod next_header {
    pub const HOP_BY_HOP: u8 = 0;
    pub const ICMPV6: u8 = 58;
    pub const TCP: u8 = 6;
    pub const UDP: u8 = 17;
    pub const ROUTING: u8 = 43;
    pub const FRAGMENT: u8 = 44;
    pub const ESP: u8 = 50;
    pub const AH: u8 = 51;
    pub const NO_NEXT_HEADER: u8 = 59;
    pub const DESTINATION: u8 = 60;
}

/// IPv6 header (40 bytes fixed)
#[derive(Debug, Clone, Copy)]
pub struct Ipv6Header {
    /// Version (always 6), Traffic Class, Flow Label
    pub version_tc_fl: u32,
    /// Payload length
    pub payload_length: u16,
    /// Next header (protocol)
    pub next_header: u8,
    /// Hop limit (TTL)
    pub hop_limit: u8,
    /// Source address
    pub src_addr: Ipv6Addr,
    /// Destination address
    pub dst_addr: Ipv6Addr,
}

impl Ipv6Header {
    /// Header size in bytes
    pub const SIZE: usize = 40;

    /// Parse from bytes
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < Self::SIZE {
            return None;
        }

        let version_tc_fl = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);

        // Check version
        if (version_tc_fl >> 28) != 6 {
            return None;
        }

        let payload_length = u16::from_be_bytes([data[4], data[5]]);
        let next_header = data[6];
        let hop_limit = data[7];

        let src_addr = Ipv6Addr::from_bytes(&data[8..24])?;
        let dst_addr = Ipv6Addr::from_bytes(&data[24..40])?;

        Some(Self {
            version_tc_fl,
            payload_length,
            next_header,
            hop_limit,
            src_addr,
            dst_addr,
        })
    }

    /// Get version (always 6)
    pub fn version(&self) -> u8 {
        ((self.version_tc_fl >> 28) & 0xf) as u8
    }

    /// Get traffic class
    pub fn traffic_class(&self) -> u8 {
        ((self.version_tc_fl >> 20) & 0xff) as u8
    }

    /// Get flow label
    pub fn flow_label(&self) -> u32 {
        self.version_tc_fl & 0xfffff
    }

    /// Build header bytes
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut bytes = [0u8; Self::SIZE];

        bytes[0..4].copy_from_slice(&self.version_tc_fl.to_be_bytes());
        bytes[4..6].copy_from_slice(&self.payload_length.to_be_bytes());
        bytes[6] = self.next_header;
        bytes[7] = self.hop_limit;
        bytes[8..24].copy_from_slice(&self.src_addr.0);
        bytes[24..40].copy_from_slice(&self.dst_addr.0);

        bytes
    }

    /// Create a new IPv6 header
    pub fn new(src: Ipv6Addr, dst: Ipv6Addr, next_header: u8, payload_len: u16) -> Self {
        Self {
            version_tc_fl: 0x60000000, // Version 6, TC=0, FL=0
            payload_length: payload_len,
            next_header,
            hop_limit: 64,
            src_addr: src,
            dst_addr: dst,
        }
    }
}

// ============================================================================
// ICMPv6
// ============================================================================

/// ICMPv6 message types
pub mod icmpv6_type {
    // Error messages
    pub const DESTINATION_UNREACHABLE: u8 = 1;
    pub const PACKET_TOO_BIG: u8 = 2;
    pub const TIME_EXCEEDED: u8 = 3;
    pub const PARAMETER_PROBLEM: u8 = 4;

    // Informational messages
    pub const ECHO_REQUEST: u8 = 128;
    pub const ECHO_REPLY: u8 = 129;

    // Neighbor Discovery
    pub const ROUTER_SOLICITATION: u8 = 133;
    pub const ROUTER_ADVERTISEMENT: u8 = 134;
    pub const NEIGHBOR_SOLICITATION: u8 = 135;
    pub const NEIGHBOR_ADVERTISEMENT: u8 = 136;
    pub const REDIRECT: u8 = 137;
}

/// ICMPv6 header
#[derive(Debug, Clone, Copy)]
pub struct Icmpv6Header {
    pub msg_type: u8,
    pub code: u8,
    pub checksum: u16,
}

impl Icmpv6Header {
    pub const SIZE: usize = 4;

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < Self::SIZE {
            return None;
        }

        Some(Self {
            msg_type: data[0],
            code: data[1],
            checksum: u16::from_be_bytes([data[2], data[3]]),
        })
    }

    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        [
            self.msg_type,
            self.code,
            (self.checksum >> 8) as u8,
            self.checksum as u8,
        ]
    }
}

/// Compute ICMPv6 checksum (includes pseudo-header)
pub fn compute_icmpv6_checksum(src: &Ipv6Addr, dst: &Ipv6Addr, icmp_data: &[u8]) -> u16 {
    let mut sum: u32 = 0;

    // Pseudo-header
    for chunk in src.0.chunks(2) {
        sum += u16::from_be_bytes([chunk[0], chunk[1]]) as u32;
    }
    for chunk in dst.0.chunks(2) {
        sum += u16::from_be_bytes([chunk[0], chunk[1]]) as u32;
    }
    sum += icmp_data.len() as u32;
    sum += next_header::ICMPV6 as u32;

    // ICMPv6 data
    for i in (0..icmp_data.len()).step_by(2) {
        let word = if i + 1 < icmp_data.len() {
            u16::from_be_bytes([icmp_data[i], icmp_data[i + 1]])
        } else {
            u16::from_be_bytes([icmp_data[i], 0])
        };
        sum += word as u32;
    }

    // Fold to 16 bits
    while sum > 0xffff {
        sum = (sum & 0xffff) + (sum >> 16);
    }

    !sum as u16
}

// ============================================================================
// Neighbor Discovery Protocol (NDP)
// ============================================================================

/// NDP Option types
pub mod ndp_option {
    pub const SOURCE_LINK_LAYER_ADDR: u8 = 1;
    pub const TARGET_LINK_LAYER_ADDR: u8 = 2;
    pub const PREFIX_INFORMATION: u8 = 3;
    pub const REDIRECTED_HEADER: u8 = 4;
    pub const MTU: u8 = 5;
}

/// Neighbor cache entry state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NeighborState {
    /// Entry created, sending solicitations
    Incomplete,
    /// Recently confirmed reachable
    Reachable,
    /// Reachable time expired, waiting for traffic
    Stale,
    /// Upper layer is sending, waiting for reachability confirmation
    Delay,
    /// Actively probing
    Probe,
}

/// Neighbor cache entry
#[derive(Debug, Clone)]
pub struct NeighborEntry {
    /// IPv6 address
    pub ip_addr: Ipv6Addr,
    /// Link-layer address (MAC)
    pub mac_addr: Option<[u8; 6]>,
    /// Entry state
    pub state: NeighborState,
    /// Is router flag
    pub is_router: bool,
    /// Last update time
    pub last_updated: u64,
    /// Number of solicitations sent
    pub solicits_sent: u8,
}

impl NeighborEntry {
    pub fn new(ip_addr: Ipv6Addr) -> Self {
        Self {
            ip_addr,
            mac_addr: None,
            state: NeighborState::Incomplete,
            is_router: false,
            last_updated: crate::time::ticks(),
            solicits_sent: 0,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.mac_addr.is_some()
            && matches!(
                self.state,
                NeighborState::Reachable | NeighborState::Stale | NeighborState::Delay | NeighborState::Probe
            )
    }
}

/// Neighbor cache
static NEIGHBOR_CACHE: IrqSafeMutex<BTreeMap<Ipv6Addr, NeighborEntry>> =
    IrqSafeMutex::new(BTreeMap::new());

/// Router list
static ROUTER_LIST: IrqSafeMutex<Vec<Ipv6Addr>> = IrqSafeMutex::new(Vec::new());

/// Lookup neighbor in cache
pub fn neighbor_lookup(addr: &Ipv6Addr) -> Option<NeighborEntry> {
    NEIGHBOR_CACHE.lock().get(addr).cloned()
}

/// Insert or update neighbor cache entry
pub fn neighbor_update(addr: Ipv6Addr, mac: [u8; 6], is_router: bool) {
    let mut cache = NEIGHBOR_CACHE.lock();

    if let Some(entry) = cache.get_mut(&addr) {
        entry.mac_addr = Some(mac);
        entry.state = NeighborState::Reachable;
        entry.is_router = is_router;
        entry.last_updated = crate::time::ticks();
    } else {
        let mut entry = NeighborEntry::new(addr);
        entry.mac_addr = Some(mac);
        entry.state = NeighborState::Reachable;
        entry.is_router = is_router;
        cache.insert(addr, entry);
    }

    // Update router list
    if is_router {
        let mut routers = ROUTER_LIST.lock();
        if !routers.contains(&addr) {
            routers.push(addr);
        }
    }
}

/// Remove neighbor from cache
pub fn neighbor_remove(addr: &Ipv6Addr) {
    NEIGHBOR_CACHE.lock().remove(addr);
}

// ============================================================================
// IPv6 Configuration
// ============================================================================

/// IPv6 interface configuration
#[derive(Debug, Clone)]
pub struct Ipv6Config {
    /// Link-local address
    pub link_local: Ipv6Addr,
    /// Global addresses
    pub global_addrs: Vec<Ipv6Addr>,
    /// Default router
    pub default_router: Option<Ipv6Addr>,
    /// MTU
    pub mtu: u32,
    /// Hop limit
    pub hop_limit: u8,
}

impl Ipv6Config {
    pub fn new(mac: &[u8; 6]) -> Self {
        Self {
            link_local: Ipv6Addr::from_mac_link_local(mac),
            global_addrs: Vec::new(),
            default_router: None,
            mtu: 1500,
            hop_limit: 64,
        }
    }
}

static IPV6_CONFIG: IrqSafeMutex<Option<Ipv6Config>> = IrqSafeMutex::new(None);

/// Initialize IPv6 stack
pub fn init() {
    if let Some(mac) = crate::drivers::net::get_mac() {
        let config = Ipv6Config::new(&mac);
        crate::kprintln!("ipv6: link-local address: {}", config.link_local);
        *IPV6_CONFIG.lock() = Some(config);
    }
    crate::kprintln!("ipv6: stack initialized");
}

/// Get current IPv6 configuration
pub fn config() -> Option<Ipv6Config> {
    IPV6_CONFIG.lock().clone()
}

/// Add a global address
pub fn add_global_address(addr: Ipv6Addr) {
    if let Some(ref mut config) = *IPV6_CONFIG.lock() {
        if !config.global_addrs.contains(&addr) {
            config.global_addrs.push(addr);
            crate::kprintln!("ipv6: added global address: {}", addr);
        }
    }
}

/// Set default router
pub fn set_default_router(router: Ipv6Addr) {
    if let Some(ref mut config) = *IPV6_CONFIG.lock() {
        config.default_router = Some(router);
        crate::kprintln!("ipv6: set default router: {}", router);
    }
}

// ============================================================================
// Packet handling
// ============================================================================

/// Handle incoming IPv6 packet
pub fn handle_packet(data: &[u8]) {
    let header = match Ipv6Header::parse(data) {
        Some(h) => h,
        None => return,
    };

    let payload = &data[Ipv6Header::SIZE..];

    // Check if packet is for us
    let config = match config() {
        Some(c) => c,
        None => return,
    };

    let is_for_us = header.dst_addr == config.link_local
        || config.global_addrs.contains(&header.dst_addr)
        || header.dst_addr.is_multicast();

    if !is_for_us {
        return;
    }

    match header.next_header {
        next_header::ICMPV6 => handle_icmpv6(&header, payload),
        next_header::TCP => {
            // Would forward to TCP handler
        }
        next_header::UDP => {
            // Would forward to UDP handler
        }
        _ => {}
    }
}

/// Handle ICMPv6 packet
fn handle_icmpv6(ip_header: &Ipv6Header, data: &[u8]) {
    let icmp_header = match Icmpv6Header::parse(data) {
        Some(h) => h,
        None => return,
    };

    match icmp_header.msg_type {
        icmpv6_type::ECHO_REQUEST => {
            handle_echo_request(ip_header, data);
        }
        icmpv6_type::NEIGHBOR_SOLICITATION => {
            handle_neighbor_solicitation(ip_header, data);
        }
        icmpv6_type::NEIGHBOR_ADVERTISEMENT => {
            handle_neighbor_advertisement(ip_header, data);
        }
        icmpv6_type::ROUTER_ADVERTISEMENT => {
            handle_router_advertisement(ip_header, data);
        }
        _ => {}
    }
}

/// Handle ICMPv6 Echo Request (ping6)
fn handle_echo_request(ip_header: &Ipv6Header, data: &[u8]) {
    if data.len() < 8 {
        return;
    }

    // Build echo reply
    let mut reply = Vec::with_capacity(data.len());
    reply.push(icmpv6_type::ECHO_REPLY);
    reply.push(0); // code
    reply.extend_from_slice(&[0, 0]); // checksum placeholder
    reply.extend_from_slice(&data[4..]); // identifier, seq, data

    // Calculate checksum
    let config = match config() {
        Some(c) => c,
        None => return,
    };

    let checksum = compute_icmpv6_checksum(&config.link_local, &ip_header.src_addr, &reply);
    reply[2] = (checksum >> 8) as u8;
    reply[3] = checksum as u8;

    // Send reply
    let _ = send_packet(&ip_header.src_addr, next_header::ICMPV6, &reply);
}

/// Handle Neighbor Solicitation
fn handle_neighbor_solicitation(ip_header: &Ipv6Header, data: &[u8]) {
    if data.len() < 24 {
        return;
    }

    // Target address is at offset 8
    let target = match Ipv6Addr::from_bytes(&data[8..24]) {
        Some(a) => a,
        None => return,
    };

    // Check if target is our address
    let config = match config() {
        Some(c) => c,
        None => return,
    };

    if target != config.link_local && !config.global_addrs.contains(&target) {
        return;
    }

    // Extract source link-layer address option if present
    let mut source_mac = None;
    let mut offset = 24;
    while offset + 2 <= data.len() {
        let opt_type = data[offset];
        let opt_len = data[offset + 1] as usize * 8;
        if opt_len == 0 {
            break;
        }

        if opt_type == ndp_option::SOURCE_LINK_LAYER_ADDR && opt_len >= 8 {
            let mut mac = [0u8; 6];
            mac.copy_from_slice(&data[offset + 2..offset + 8]);
            source_mac = Some(mac);
        }

        offset += opt_len;
    }

    // Update neighbor cache if we got source MAC
    if let Some(mac) = source_mac {
        if !ip_header.src_addr.is_unspecified() {
            neighbor_update(ip_header.src_addr, mac, false);
        }
    }

    // Send Neighbor Advertisement
    send_neighbor_advertisement(&ip_header.src_addr, &target, true);
}

/// Handle Neighbor Advertisement
fn handle_neighbor_advertisement(ip_header: &Ipv6Header, data: &[u8]) {
    if data.len() < 24 {
        return;
    }

    let flags = data[4];
    let _router = (flags & 0x80) != 0;
    let _solicited = (flags & 0x40) != 0;
    let _override_flag = (flags & 0x20) != 0;

    // Target address
    let target = match Ipv6Addr::from_bytes(&data[8..24]) {
        Some(a) => a,
        None => return,
    };

    // Extract target link-layer address option
    let mut target_mac = None;
    let mut offset = 24;
    while offset + 2 <= data.len() {
        let opt_type = data[offset];
        let opt_len = data[offset + 1] as usize * 8;
        if opt_len == 0 {
            break;
        }

        if opt_type == ndp_option::TARGET_LINK_LAYER_ADDR && opt_len >= 8 {
            let mut mac = [0u8; 6];
            mac.copy_from_slice(&data[offset + 2..offset + 8]);
            target_mac = Some(mac);
        }

        offset += opt_len;
    }

    // Update neighbor cache
    if let Some(mac) = target_mac {
        neighbor_update(target, mac, _router);
    }
}

/// Handle Router Advertisement
fn handle_router_advertisement(ip_header: &Ipv6Header, data: &[u8]) {
    if data.len() < 16 {
        return;
    }

    let _cur_hop_limit = data[4];
    let _flags = data[5];
    let _router_lifetime = u16::from_be_bytes([data[6], data[7]]);

    // Update router list
    if _router_lifetime > 0 {
        set_default_router(ip_header.src_addr);
    }

    // Parse options for prefix info, etc.
    let mut offset = 16;
    while offset + 2 <= data.len() {
        let opt_type = data[offset];
        let opt_len = data[offset + 1] as usize * 8;
        if opt_len == 0 {
            break;
        }

        if opt_type == ndp_option::SOURCE_LINK_LAYER_ADDR && opt_len >= 8 {
            let mut mac = [0u8; 6];
            mac.copy_from_slice(&data[offset + 2..offset + 8]);
            neighbor_update(ip_header.src_addr, mac, true);
        }

        offset += opt_len;
    }
}

/// Send Neighbor Advertisement
fn send_neighbor_advertisement(dst: &Ipv6Addr, target: &Ipv6Addr, solicited: bool) {
    let config = match config() {
        Some(c) => c,
        None => return,
    };

    let mac = match crate::drivers::net::get_mac() {
        Some(m) => m,
        None => return,
    };

    // Build NA
    let mut na = Vec::with_capacity(32);

    // ICMPv6 header
    na.push(icmpv6_type::NEIGHBOR_ADVERTISEMENT);
    na.push(0); // code
    na.extend_from_slice(&[0, 0]); // checksum placeholder

    // Flags: R=0, S=solicited, O=1
    let flags = if solicited { 0x60 } else { 0x20 };
    na.push(flags);
    na.extend_from_slice(&[0, 0, 0]); // reserved

    // Target address
    na.extend_from_slice(&target.0);

    // Target link-layer address option
    na.push(ndp_option::TARGET_LINK_LAYER_ADDR);
    na.push(1); // length = 8 bytes / 8 = 1
    na.extend_from_slice(&mac);

    // Checksum
    let checksum = compute_icmpv6_checksum(&config.link_local, dst, &na);
    na[2] = (checksum >> 8) as u8;
    na[3] = checksum as u8;

    let _ = send_packet(dst, next_header::ICMPV6, &na);
}

/// Send Neighbor Solicitation
pub fn send_neighbor_solicitation(target: &Ipv6Addr) {
    let config = match config() {
        Some(c) => c,
        None => return,
    };

    let mac = match crate::drivers::net::get_mac() {
        Some(m) => m,
        None => return,
    };

    let dst = target.solicited_node_multicast();

    // Build NS
    let mut ns = Vec::with_capacity(32);

    // ICMPv6 header
    ns.push(icmpv6_type::NEIGHBOR_SOLICITATION);
    ns.push(0); // code
    ns.extend_from_slice(&[0, 0]); // checksum placeholder
    ns.extend_from_slice(&[0, 0, 0, 0]); // reserved

    // Target address
    ns.extend_from_slice(&target.0);

    // Source link-layer address option
    ns.push(ndp_option::SOURCE_LINK_LAYER_ADDR);
    ns.push(1); // length
    ns.extend_from_slice(&mac);

    // Checksum
    let checksum = compute_icmpv6_checksum(&config.link_local, &dst, &ns);
    ns[2] = (checksum >> 8) as u8;
    ns[3] = checksum as u8;

    let _ = send_packet(&dst, next_header::ICMPV6, &ns);
}

/// Send IPv6 packet
pub fn send_packet(dst: &Ipv6Addr, next_header: u8, payload: &[u8]) -> crate::util::KResult<()> {
    let config = config().ok_or(crate::util::KError::NotSupported)?;

    // Determine source address
    let src = if dst.is_link_local() {
        config.link_local
    } else if let Some(global) = config.global_addrs.first() {
        *global
    } else {
        config.link_local
    };

    // Build IPv6 header
    let header = Ipv6Header::new(src, *dst, next_header, payload.len() as u16);
    let header_bytes = header.to_bytes();

    // Determine destination MAC
    let dst_mac = if dst.is_multicast() {
        // Multicast MAC: 33:33:xx:xx:xx:xx (last 32 bits of IPv6)
        [0x33, 0x33, dst.0[12], dst.0[13], dst.0[14], dst.0[15]]
    } else if let Some(entry) = neighbor_lookup(dst) {
        if let Some(mac) = entry.mac_addr {
            mac
        } else {
            // Need to do NDP
            send_neighbor_solicitation(dst);
            return Err(crate::util::KError::WouldBlock);
        }
    } else {
        // No cache entry, start NDP
        send_neighbor_solicitation(dst);
        return Err(crate::util::KError::WouldBlock);
    };

    // Build full packet
    let mut packet = Vec::with_capacity(header_bytes.len() + payload.len());
    packet.extend_from_slice(&header_bytes);
    packet.extend_from_slice(payload);

    // Send via Ethernet
    super::send_ethernet(super::MacAddr(dst_mac), 0x86DD, &packet)
}
