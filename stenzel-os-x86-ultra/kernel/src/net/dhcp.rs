//! DHCP (Dynamic Host Configuration Protocol) Client
//!
//! Implements DHCP client to automatically obtain network configuration.
//! Uses UDP ports 67 (server) and 68 (client).

#![allow(dead_code)]

use alloc::vec::Vec;

use super::Ipv4Addr;
use super::udp;
use super::dns;
use crate::util::{KResult, KError};
use crate::kprintln;

// DHCP ports
const DHCP_SERVER_PORT: u16 = 67;
const DHCP_CLIENT_PORT: u16 = 68;

// DHCP message types
const DHCP_DISCOVER: u8 = 1;
const DHCP_OFFER: u8 = 2;
const DHCP_REQUEST: u8 = 3;
const DHCP_DECLINE: u8 = 4;
const DHCP_ACK: u8 = 5;
const DHCP_NAK: u8 = 6;
const DHCP_RELEASE: u8 = 7;
const DHCP_INFORM: u8 = 8;

// DHCP option codes
const OPT_PAD: u8 = 0;
const OPT_SUBNET_MASK: u8 = 1;
const OPT_ROUTER: u8 = 3;
const OPT_DNS: u8 = 6;
const OPT_HOSTNAME: u8 = 12;
const OPT_REQUESTED_IP: u8 = 50;
const OPT_LEASE_TIME: u8 = 51;
const OPT_MESSAGE_TYPE: u8 = 53;
const OPT_SERVER_ID: u8 = 54;
const OPT_PARAM_REQUEST: u8 = 55;
const OPT_END: u8 = 255;

// BOOTP opcodes
const BOOTREQUEST: u8 = 1;
const BOOTREPLY: u8 = 2;

// DHCP magic cookie
const MAGIC_COOKIE: [u8; 4] = [99, 130, 83, 99];

/// DHCP configuration obtained from server
#[derive(Debug, Clone)]
pub struct DhcpConfig {
    pub ip: Ipv4Addr,
    pub netmask: Ipv4Addr,
    pub gateway: Ipv4Addr,
    pub dns_servers: Vec<Ipv4Addr>,
    pub lease_time: u32,
    pub server_id: Ipv4Addr,
}

/// DHCP packet builder/parser
struct DhcpPacket {
    op: u8,
    htype: u8,
    hlen: u8,
    hops: u8,
    xid: u32,
    secs: u16,
    flags: u16,
    ciaddr: Ipv4Addr,
    yiaddr: Ipv4Addr,
    siaddr: Ipv4Addr,
    giaddr: Ipv4Addr,
    chaddr: [u8; 16],
    options: Vec<u8>,
}

impl DhcpPacket {
    fn new_discover(mac: &[u8; 6], xid: u32) -> Self {
        let mut chaddr = [0u8; 16];
        chaddr[..6].copy_from_slice(mac);

        let mut pkt = Self {
            op: BOOTREQUEST,
            htype: 1,      // Ethernet
            hlen: 6,       // MAC address length
            hops: 0,
            xid,
            secs: 0,
            flags: 0x8000, // Broadcast flag
            ciaddr: Ipv4Addr::UNSPECIFIED,
            yiaddr: Ipv4Addr::UNSPECIFIED,
            siaddr: Ipv4Addr::UNSPECIFIED,
            giaddr: Ipv4Addr::UNSPECIFIED,
            chaddr,
            options: Vec::new(),
        };

        // Add DHCP options
        pkt.add_option_message_type(DHCP_DISCOVER);
        pkt.add_option_param_request();
        pkt.add_option_end();

        pkt
    }

    fn new_request(mac: &[u8; 6], xid: u32, offered_ip: Ipv4Addr, server_id: Ipv4Addr) -> Self {
        let mut chaddr = [0u8; 16];
        chaddr[..6].copy_from_slice(mac);

        let mut pkt = Self {
            op: BOOTREQUEST,
            htype: 1,
            hlen: 6,
            hops: 0,
            xid,
            secs: 0,
            flags: 0x8000,
            ciaddr: Ipv4Addr::UNSPECIFIED,
            yiaddr: Ipv4Addr::UNSPECIFIED,
            siaddr: Ipv4Addr::UNSPECIFIED,
            giaddr: Ipv4Addr::UNSPECIFIED,
            chaddr,
            options: Vec::new(),
        };

        pkt.add_option_message_type(DHCP_REQUEST);
        pkt.add_option_requested_ip(offered_ip);
        pkt.add_option_server_id(server_id);
        pkt.add_option_param_request();
        pkt.add_option_end();

        pkt
    }

    fn add_option_message_type(&mut self, msg_type: u8) {
        self.options.push(OPT_MESSAGE_TYPE);
        self.options.push(1);
        self.options.push(msg_type);
    }

    fn add_option_requested_ip(&mut self, ip: Ipv4Addr) {
        self.options.push(OPT_REQUESTED_IP);
        self.options.push(4);
        self.options.extend_from_slice(&ip.0);
    }

    fn add_option_server_id(&mut self, ip: Ipv4Addr) {
        self.options.push(OPT_SERVER_ID);
        self.options.push(4);
        self.options.extend_from_slice(&ip.0);
    }

    fn add_option_param_request(&mut self) {
        self.options.push(OPT_PARAM_REQUEST);
        self.options.push(4);
        self.options.push(OPT_SUBNET_MASK);
        self.options.push(OPT_ROUTER);
        self.options.push(OPT_DNS);
        self.options.push(OPT_LEASE_TIME);
    }

    fn add_option_end(&mut self) {
        self.options.push(OPT_END);
    }

    fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(300);

        // Fixed header
        buf.push(self.op);
        buf.push(self.htype);
        buf.push(self.hlen);
        buf.push(self.hops);
        buf.extend_from_slice(&self.xid.to_be_bytes());
        buf.extend_from_slice(&self.secs.to_be_bytes());
        buf.extend_from_slice(&self.flags.to_be_bytes());
        buf.extend_from_slice(&self.ciaddr.0);
        buf.extend_from_slice(&self.yiaddr.0);
        buf.extend_from_slice(&self.siaddr.0);
        buf.extend_from_slice(&self.giaddr.0);
        buf.extend_from_slice(&self.chaddr);

        // Server host name (64 bytes) + boot file (128 bytes)
        buf.extend_from_slice(&[0u8; 192]);

        // Magic cookie
        buf.extend_from_slice(&MAGIC_COOKIE);

        // Options
        buf.extend_from_slice(&self.options);

        // Padding to minimum size
        while buf.len() < 300 {
            buf.push(0);
        }

        buf
    }

    fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 240 {
            return None;
        }

        let op = data[0];
        let htype = data[1];
        let hlen = data[2];
        let hops = data[3];
        let xid = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let secs = u16::from_be_bytes([data[8], data[9]]);
        let flags = u16::from_be_bytes([data[10], data[11]]);
        let ciaddr = Ipv4Addr::from_bytes(&data[12..16]);
        let yiaddr = Ipv4Addr::from_bytes(&data[16..20]);
        let siaddr = Ipv4Addr::from_bytes(&data[20..24]);
        let giaddr = Ipv4Addr::from_bytes(&data[24..28]);

        let mut chaddr = [0u8; 16];
        chaddr.copy_from_slice(&data[28..44]);

        // Skip sname (64) + file (128)
        // Check magic cookie at offset 236
        if &data[236..240] != &MAGIC_COOKIE {
            return None;
        }

        // Options start at offset 240
        let options = data[240..].to_vec();

        Some(Self {
            op,
            htype,
            hlen,
            hops,
            xid,
            secs,
            flags,
            ciaddr,
            yiaddr,
            siaddr,
            giaddr,
            chaddr,
            options,
        })
    }

    fn get_message_type(&self) -> Option<u8> {
        let mut i = 0;
        while i < self.options.len() {
            let opt = self.options[i];
            if opt == OPT_END {
                break;
            }
            if opt == OPT_PAD {
                i += 1;
                continue;
            }
            if i + 1 >= self.options.len() {
                break;
            }
            let len = self.options[i + 1] as usize;
            if opt == OPT_MESSAGE_TYPE && len == 1 && i + 2 < self.options.len() {
                return Some(self.options[i + 2]);
            }
            i += 2 + len;
        }
        None
    }

    fn get_option_ip(&self, code: u8) -> Option<Ipv4Addr> {
        let mut i = 0;
        while i < self.options.len() {
            let opt = self.options[i];
            if opt == OPT_END {
                break;
            }
            if opt == OPT_PAD {
                i += 1;
                continue;
            }
            if i + 1 >= self.options.len() {
                break;
            }
            let len = self.options[i + 1] as usize;
            if opt == code && len >= 4 && i + 6 <= self.options.len() {
                return Some(Ipv4Addr::from_bytes(&self.options[i + 2..i + 6]));
            }
            i += 2 + len;
        }
        None
    }

    fn get_option_ips(&self, code: u8) -> Vec<Ipv4Addr> {
        let mut ips = Vec::new();
        let mut i = 0;
        while i < self.options.len() {
            let opt = self.options[i];
            if opt == OPT_END {
                break;
            }
            if opt == OPT_PAD {
                i += 1;
                continue;
            }
            if i + 1 >= self.options.len() {
                break;
            }
            let len = self.options[i + 1] as usize;
            if opt == code && len >= 4 {
                let count = len / 4;
                for j in 0..count {
                    let offset = i + 2 + j * 4;
                    if offset + 4 <= self.options.len() {
                        ips.push(Ipv4Addr::from_bytes(&self.options[offset..offset + 4]));
                    }
                }
            }
            i += 2 + len;
        }
        ips
    }

    fn get_option_u32(&self, code: u8) -> Option<u32> {
        let mut i = 0;
        while i < self.options.len() {
            let opt = self.options[i];
            if opt == OPT_END {
                break;
            }
            if opt == OPT_PAD {
                i += 1;
                continue;
            }
            if i + 1 >= self.options.len() {
                break;
            }
            let len = self.options[i + 1] as usize;
            if opt == code && len == 4 && i + 6 <= self.options.len() {
                return Some(u32::from_be_bytes([
                    self.options[i + 2],
                    self.options[i + 3],
                    self.options[i + 4],
                    self.options[i + 5],
                ]));
            }
            i += 2 + len;
        }
        None
    }
}

/// Generate a transaction ID
fn generate_xid() -> u32 {
    static XID: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0x12345678);
    XID.fetch_add(1, core::sync::atomic::Ordering::Relaxed)
}

/// Perform DHCP discovery and obtain configuration
pub fn discover(mac: &[u8; 6]) -> KResult<DhcpConfig> {
    kprintln!("dhcp: starting discovery...");

    // Bind to DHCP client port
    if udp::bind(DHCP_CLIENT_PORT).is_err() {
        // Port might already be bound, try to use it anyway
    }

    let xid = generate_xid();

    // Send DISCOVER
    let discover = DhcpPacket::new_discover(mac, xid);
    let discover_data = discover.serialize();

    for attempt in 0..3 {
        kprintln!("dhcp: sending DISCOVER (attempt {})", attempt + 1);

        // Send to broadcast address
        if udp::send(DHCP_CLIENT_PORT, Ipv4Addr::BROADCAST, DHCP_SERVER_PORT, &discover_data).is_err() {
            continue;
        }

        // Wait for OFFER
        let timeout = 5000 + attempt * 2000;
        if let Some(response) = udp::recv_timeout(DHCP_CLIENT_PORT, timeout) {
            if let Some(offer) = DhcpPacket::parse(&response.data) {
                if offer.xid != xid || offer.op != BOOTREPLY {
                    continue;
                }

                if let Some(msg_type) = offer.get_message_type() {
                    if msg_type == DHCP_OFFER {
                        kprintln!("dhcp: received OFFER: {}", offer.yiaddr);

                        // Extract server ID
                        let server_id = offer.get_option_ip(OPT_SERVER_ID)
                            .unwrap_or(offer.siaddr);

                        // Send REQUEST
                        let request = DhcpPacket::new_request(mac, xid, offer.yiaddr, server_id);
                        let request_data = request.serialize();

                        kprintln!("dhcp: sending REQUEST for {}", offer.yiaddr);

                        for req_attempt in 0..3 {
                            if udp::send(DHCP_CLIENT_PORT, Ipv4Addr::BROADCAST, DHCP_SERVER_PORT, &request_data).is_err() {
                                continue;
                            }

                            // Wait for ACK
                            let req_timeout = 3000 + req_attempt * 1000;
                            if let Some(ack_response) = udp::recv_timeout(DHCP_CLIENT_PORT, req_timeout) {
                                if let Some(ack) = DhcpPacket::parse(&ack_response.data) {
                                    if ack.xid != xid || ack.op != BOOTREPLY {
                                        continue;
                                    }

                                    if let Some(ack_type) = ack.get_message_type() {
                                        if ack_type == DHCP_ACK {
                                            let config = DhcpConfig {
                                                ip: ack.yiaddr,
                                                netmask: ack.get_option_ip(OPT_SUBNET_MASK)
                                                    .unwrap_or(Ipv4Addr::new(255, 255, 255, 0)),
                                                gateway: ack.get_option_ip(OPT_ROUTER)
                                                    .unwrap_or(Ipv4Addr::UNSPECIFIED),
                                                dns_servers: ack.get_option_ips(OPT_DNS),
                                                lease_time: ack.get_option_u32(OPT_LEASE_TIME)
                                                    .unwrap_or(86400),
                                                server_id,
                                            };

                                            kprintln!("dhcp: ACK received!");
                                            kprintln!("dhcp: IP = {}", config.ip);
                                            kprintln!("dhcp: Netmask = {}", config.netmask);
                                            kprintln!("dhcp: Gateway = {}", config.gateway);
                                            kprintln!("dhcp: Lease = {} seconds", config.lease_time);

                                            if !config.dns_servers.is_empty() {
                                                kprintln!("dhcp: DNS = {}", config.dns_servers[0]);
                                            }

                                            udp::unbind(DHCP_CLIENT_PORT);
                                            return Ok(config);
                                        } else if ack_type == DHCP_NAK {
                                            kprintln!("dhcp: received NAK, retrying...");
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    udp::unbind(DHCP_CLIENT_PORT);
    kprintln!("dhcp: discovery failed");
    Err(KError::Timeout)
}

/// Apply DHCP configuration to the network stack
pub fn apply_config(config: &DhcpConfig) {
    use super::NET_CONFIG;

    let mut net_config = NET_CONFIG.lock();
    if let Some(ref mut cfg) = *net_config {
        cfg.ip = config.ip;
        cfg.netmask = config.netmask;
        cfg.gateway = config.gateway;

        kprintln!("dhcp: configuration applied");
    }

    // Update DNS servers
    if !config.dns_servers.is_empty() {
        dns::set_servers(&config.dns_servers);
        kprintln!("dhcp: DNS servers updated");
    }
}

/// Perform DHCP and apply configuration
pub fn auto_configure() -> KResult<DhcpConfig> {
    let mac = {
        let config = super::config().ok_or(KError::NotSupported)?;
        config.mac
    };

    let config = discover(&mac)?;
    apply_config(&config);
    Ok(config)
}

/// Request IP configuration (non-blocking start)
/// Used by the WiFi connection manager to start DHCP asynchronously
pub fn request() -> KResult<()> {
    // For now, just start auto_configure in the background
    // In a real implementation, this would be async
    let mac = {
        let config = super::config().ok_or(KError::NotSupported)?;
        config.mac
    };

    // Bind to DHCP client port
    let _ = udp::bind(DHCP_CLIENT_PORT);

    let xid = generate_xid();

    // Send DISCOVER
    let discover = DhcpPacket::new_discover(&mac, xid);
    let discover_data = discover.serialize();

    kprintln!("dhcp: sending DISCOVER (async request)");

    // Send to broadcast address
    udp::send(DHCP_CLIENT_PORT, Ipv4Addr::BROADCAST, DHCP_SERVER_PORT, &discover_data)?;

    Ok(())
}

/// Check if DHCP is in progress
pub fn is_pending() -> bool {
    // Check if we have a pending DHCP request
    // For now, return false
    false
}

/// Get current DHCP status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DhcpStatus {
    /// No DHCP in progress
    Idle,
    /// Waiting for offer
    Discovering,
    /// Waiting for ACK
    Requesting,
    /// Configuration obtained
    Configured,
    /// DHCP failed
    Failed,
}

/// Get DHCP status
pub fn status() -> DhcpStatus {
    // For now, return Idle
    DhcpStatus::Idle
}
