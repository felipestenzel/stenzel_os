//! DNS (Domain Name System) Resolver
//!
//! Simple DNS resolver for translating domain names to IP addresses.
//! Uses UDP port 53.

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::collections::BTreeMap;

use super::Ipv4Addr;
use super::udp;
use crate::sync::IrqSafeMutex;
use crate::util::{KResult, KError};

// DNS constants
const DNS_PORT: u16 = 53;
const DNS_HEADER_SIZE: usize = 12;

// DNS record types
const TYPE_A: u16 = 1;      // IPv4 address
const TYPE_AAAA: u16 = 28;  // IPv6 address
const TYPE_CNAME: u16 = 5;  // Canonical name

// DNS record classes
const CLASS_IN: u16 = 1;    // Internet

// DNS flags
const FLAG_QR: u16 = 0x8000;      // Query/Response
const FLAG_OPCODE: u16 = 0x7800;  // Opcode
const FLAG_AA: u16 = 0x0400;      // Authoritative Answer
const FLAG_TC: u16 = 0x0200;      // Truncated
const FLAG_RD: u16 = 0x0100;      // Recursion Desired
const FLAG_RA: u16 = 0x0080;      // Recursion Available
const FLAG_RCODE: u16 = 0x000F;   // Response code

// Response codes
const RCODE_OK: u16 = 0;
const RCODE_FORMAT_ERROR: u16 = 1;
const RCODE_SERVER_FAILURE: u16 = 2;
const RCODE_NAME_ERROR: u16 = 3;  // NXDOMAIN
const RCODE_NOT_IMPLEMENTED: u16 = 4;
const RCODE_REFUSED: u16 = 5;

/// DNS configuration
static DNS_CONFIG: IrqSafeMutex<DnsConfig> = IrqSafeMutex::new(DnsConfig {
    servers: Vec::new(),
    timeout_ms: 5000,
    retries: 3,
});

/// DNS cache (domain -> (IP, expiry_time))
static DNS_CACHE: IrqSafeMutex<BTreeMap<String, (Ipv4Addr, u64)>> = IrqSafeMutex::new(BTreeMap::new());

/// Next transaction ID
static NEXT_TX_ID: core::sync::atomic::AtomicU16 = core::sync::atomic::AtomicU16::new(1);

/// DNS configuration
struct DnsConfig {
    servers: Vec<Ipv4Addr>,
    timeout_ms: u32,
    retries: u32,
}

/// DNS header
#[derive(Debug, Clone)]
struct DnsHeader {
    id: u16,
    flags: u16,
    qdcount: u16,  // Number of questions
    ancount: u16,  // Number of answers
    nscount: u16,  // Number of authority records
    arcount: u16,  // Number of additional records
}

impl DnsHeader {
    fn new_query(id: u16) -> Self {
        Self {
            id,
            flags: FLAG_RD,  // Recursion desired
            qdcount: 1,
            ancount: 0,
            nscount: 0,
            arcount: 0,
        }
    }

    fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < DNS_HEADER_SIZE {
            return None;
        }
        Some(Self {
            id: u16::from_be_bytes([data[0], data[1]]),
            flags: u16::from_be_bytes([data[2], data[3]]),
            qdcount: u16::from_be_bytes([data[4], data[5]]),
            ancount: u16::from_be_bytes([data[6], data[7]]),
            nscount: u16::from_be_bytes([data[8], data[9]]),
            arcount: u16::from_be_bytes([data[10], data[11]]),
        })
    }

    fn serialize(&self) -> [u8; DNS_HEADER_SIZE] {
        let mut buf = [0u8; DNS_HEADER_SIZE];
        buf[0..2].copy_from_slice(&self.id.to_be_bytes());
        buf[2..4].copy_from_slice(&self.flags.to_be_bytes());
        buf[4..6].copy_from_slice(&self.qdcount.to_be_bytes());
        buf[6..8].copy_from_slice(&self.ancount.to_be_bytes());
        buf[8..10].copy_from_slice(&self.nscount.to_be_bytes());
        buf[10..12].copy_from_slice(&self.arcount.to_be_bytes());
        buf
    }

    fn is_response(&self) -> bool {
        (self.flags & FLAG_QR) != 0
    }

    fn rcode(&self) -> u16 {
        self.flags & FLAG_RCODE
    }
}

/// DNS question
#[derive(Debug, Clone)]
struct DnsQuestion {
    name: String,
    qtype: u16,
    qclass: u16,
}

impl DnsQuestion {
    fn new(name: &str, qtype: u16) -> Self {
        Self {
            name: String::from(name),
            qtype,
            qclass: CLASS_IN,
        }
    }

    fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        // Encode name as labels
        for label in self.name.split('.') {
            if label.is_empty() {
                continue;
            }
            buf.push(label.len() as u8);
            buf.extend_from_slice(label.as_bytes());
        }
        buf.push(0); // Root label
        buf.extend_from_slice(&self.qtype.to_be_bytes());
        buf.extend_from_slice(&self.qclass.to_be_bytes());
        buf
    }
}

/// DNS resource record (answer)
#[derive(Debug, Clone)]
struct DnsRecord {
    name: String,
    rtype: u16,
    rclass: u16,
    ttl: u32,
    rdata: Vec<u8>,
}

impl DnsRecord {
    fn as_ipv4(&self) -> Option<Ipv4Addr> {
        if self.rtype == TYPE_A && self.rdata.len() == 4 {
            Some(Ipv4Addr::from_bytes(&self.rdata))
        } else {
            None
        }
    }
}

/// Build a DNS query packet
fn build_query(name: &str, qtype: u16) -> (u16, Vec<u8>) {
    use core::sync::atomic::Ordering;

    let tx_id = NEXT_TX_ID.fetch_add(1, Ordering::Relaxed);
    let header = DnsHeader::new_query(tx_id);
    let question = DnsQuestion::new(name, qtype);

    let mut packet = Vec::new();
    packet.extend_from_slice(&header.serialize());
    packet.extend_from_slice(&question.serialize());

    (tx_id, packet)
}

/// Parse a domain name from DNS response (handles compression)
fn parse_name(data: &[u8], start: usize) -> Option<(String, usize)> {
    let mut name = String::new();
    let mut pos = start;
    let mut jumped = false;
    let mut original_end = 0;

    loop {
        if pos >= data.len() {
            return None;
        }

        let len = data[pos] as usize;

        if len == 0 {
            // End of name
            if !jumped {
                original_end = pos + 1;
            }
            break;
        }

        if (len & 0xC0) == 0xC0 {
            // Compression pointer
            if pos + 1 >= data.len() {
                return None;
            }
            let offset = (((len & 0x3F) as usize) << 8) | (data[pos + 1] as usize);
            if !jumped {
                original_end = pos + 2;
            }
            jumped = true;
            pos = offset;
            continue;
        }

        // Regular label
        pos += 1;
        if pos + len > data.len() {
            return None;
        }

        if !name.is_empty() {
            name.push('.');
        }

        if let Ok(s) = core::str::from_utf8(&data[pos..pos + len]) {
            name.push_str(s);
        } else {
            return None;
        }

        pos += len;
    }

    let end = if jumped { original_end } else { pos + 1 };
    Some((name, end))
}

/// Parse DNS response and extract answers
fn parse_response(data: &[u8], expected_id: u16) -> Option<Vec<DnsRecord>> {
    let header = DnsHeader::parse(data)?;

    // Verify response
    if !header.is_response() || header.id != expected_id {
        return None;
    }

    // Check for errors
    let rcode = header.rcode();
    if rcode != RCODE_OK {
        return None;
    }

    let mut pos = DNS_HEADER_SIZE;

    // Skip questions
    for _ in 0..header.qdcount {
        let (_, end) = parse_name(data, pos)?;
        pos = end + 4; // Skip qtype and qclass
        if pos > data.len() {
            return None;
        }
    }

    // Parse answers
    let mut records = Vec::new();
    for _ in 0..header.ancount {
        let (name, end) = parse_name(data, pos)?;
        pos = end;

        if pos + 10 > data.len() {
            return None;
        }

        let rtype = u16::from_be_bytes([data[pos], data[pos + 1]]);
        let rclass = u16::from_be_bytes([data[pos + 2], data[pos + 3]]);
        let ttl = u32::from_be_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]]);
        let rdlength = u16::from_be_bytes([data[pos + 8], data[pos + 9]]) as usize;

        pos += 10;

        if pos + rdlength > data.len() {
            return None;
        }

        let rdata = data[pos..pos + rdlength].to_vec();
        pos += rdlength;

        records.push(DnsRecord {
            name,
            rtype,
            rclass,
            ttl,
            rdata,
        });
    }

    Some(records)
}

/// Initialize DNS with default servers
pub fn init() {
    let mut config = DNS_CONFIG.lock();
    // Default to Google DNS
    config.servers = vec![
        Ipv4Addr::from_bytes(&[8, 8, 8, 8]),
        Ipv4Addr::from_bytes(&[8, 8, 4, 4]),
    ];
}

/// Set DNS servers
pub fn set_servers(servers: &[Ipv4Addr]) {
    let mut config = DNS_CONFIG.lock();
    config.servers = servers.to_vec();
}

/// Add a DNS server
pub fn add_server(server: Ipv4Addr) {
    let mut config = DNS_CONFIG.lock();
    if !config.servers.contains(&server) {
        config.servers.push(server);
    }
}

/// Get current DNS servers
pub fn get_servers() -> Vec<Ipv4Addr> {
    let config = DNS_CONFIG.lock();
    config.servers.clone()
}

/// Clear DNS cache
pub fn clear_cache() {
    let mut cache = DNS_CACHE.lock();
    cache.clear();
}

/// Look up an IP address from cache
fn cache_lookup(name: &str) -> Option<Ipv4Addr> {
    let cache = DNS_CACHE.lock();
    if let Some(&(ip, expiry)) = cache.get(name) {
        // Check if still valid (using uptime as time reference)
        let now = crate::time::uptime_secs();
        if now < expiry {
            return Some(ip);
        }
    }
    None
}

/// Store IP in cache
fn cache_store(name: &str, ip: Ipv4Addr, ttl: u32) {
    let mut cache = DNS_CACHE.lock();
    let now = crate::time::uptime_secs();
    let expiry = now + (ttl as u64).max(60).min(86400); // Min 60s, max 24h
    cache.insert(String::from(name), (ip, expiry));

    // Limit cache size
    while cache.len() > 256 {
        if let Some(key) = cache.keys().next().cloned() {
            cache.remove(&key);
        }
    }
}

/// Resolve a domain name to an IPv4 address
pub fn resolve(name: &str) -> KResult<Ipv4Addr> {
    // Check if it's already an IP address
    if let Some(ip) = parse_ipv4(name) {
        return Ok(ip);
    }

    // Check cache first
    if let Some(ip) = cache_lookup(name) {
        return Ok(ip);
    }

    // Get DNS servers
    let (servers, timeout_ms, retries) = {
        let config = DNS_CONFIG.lock();
        (config.servers.clone(), config.timeout_ms, config.retries)
    };

    if servers.is_empty() {
        return Err(KError::NotSupported);
    }

    // Allocate ephemeral port
    let src_port = udp::allocate_port();

    // Build query
    let (tx_id, query) = build_query(name, TYPE_A);

    // Try each server
    for server in &servers {
        for _ in 0..retries {
            // Send query
            if udp::send(src_port, *server, DNS_PORT, &query).is_err() {
                continue;
            }

            // Wait for response
            if let Some(response) = udp::recv_timeout(src_port, timeout_ms) {
                // Parse response
                if let Some(records) = parse_response(&response.data, tx_id) {
                    // Find A record
                    for record in records {
                        if record.rtype == TYPE_A {
                            if let Some(ip) = record.as_ipv4() {
                                // Store in cache
                                cache_store(name, ip, record.ttl);
                                // Clean up
                                udp::unbind(src_port);
                                return Ok(ip);
                            }
                        }
                    }
                }
            }
        }
    }

    // Clean up
    udp::unbind(src_port);
    Err(KError::NotFound)
}

/// Parse IPv4 address from string (e.g., "192.168.1.1")
fn parse_ipv4(s: &str) -> Option<Ipv4Addr> {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 4 {
        return None;
    }

    let mut bytes = [0u8; 4];
    for (i, part) in parts.iter().enumerate() {
        bytes[i] = part.parse().ok()?;
    }

    Some(Ipv4Addr::from_bytes(&bytes))
}

/// Resolve a hostname (convenience wrapper)
pub fn lookup_host(hostname: &str) -> KResult<Ipv4Addr> {
    resolve(hostname)
}
