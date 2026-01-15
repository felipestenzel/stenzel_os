//! IPv4 (Internet Protocol version 4)

#![allow(dead_code)]

use alloc::vec::Vec;

use super::{Ipv4Addr, ethernet, arp};
use crate::net::icmp;
use crate::net::udp;
use crate::net::tcp;

/// Protocolo de transporte
pub const PROTO_ICMP: u8 = 1;
pub const PROTO_TCP: u8 = 6;
pub const PROTO_UDP: u8 = 17;

/// Contador de identificação de pacotes
static mut PACKET_ID: u16 = 0;

/// Pacote IPv4 parseado
#[derive(Debug)]
pub struct Ipv4Packet {
    pub version: u8,
    pub ihl: u8,
    pub dscp: u8,
    pub ecn: u8,
    pub total_length: u16,
    pub identification: u16,
    pub flags: u8,
    pub fragment_offset: u16,
    pub ttl: u8,
    pub protocol: u8,
    pub checksum: u16,
    pub src: Ipv4Addr,
    pub dst: Ipv4Addr,
    pub header_len: usize,
}

impl Ipv4Packet {
    /// Parseia um pacote IPv4
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 20 {
            return None;
        }

        let version = data[0] >> 4;
        let ihl = data[0] & 0x0F;

        if version != 4 || ihl < 5 {
            return None;
        }

        let header_len = (ihl as usize) * 4;
        if data.len() < header_len {
            return None;
        }

        let total_length = u16::from_be_bytes([data[2], data[3]]);
        let flags_frag = u16::from_be_bytes([data[6], data[7]]);

        Some(Self {
            version,
            ihl,
            dscp: data[1] >> 2,
            ecn: data[1] & 0x03,
            total_length,
            identification: u16::from_be_bytes([data[4], data[5]]),
            flags: (flags_frag >> 13) as u8,
            fragment_offset: flags_frag & 0x1FFF,
            ttl: data[8],
            protocol: data[9],
            checksum: u16::from_be_bytes([data[10], data[11]]),
            src: Ipv4Addr::from_bytes(&data[12..16]),
            dst: Ipv4Addr::from_bytes(&data[16..20]),
            header_len,
        })
    }
}

/// Calcula o checksum do header IPv4
pub fn compute_checksum(header: &[u8]) -> u16 {
    let mut sum: u32 = 0;

    // Soma todos os words de 16 bits
    for i in (0..header.len()).step_by(2) {
        let word = if i + 1 < header.len() {
            u16::from_be_bytes([header[i], header[i + 1]])
        } else {
            u16::from_be_bytes([header[i], 0])
        };
        sum += word as u32;
    }

    // Fold de 32 para 16 bits
    while sum > 0xFFFF {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }

    // Complemento de um
    !(sum as u16)
}

/// Processa um pacote IPv4 recebido
pub fn handle_packet(data: &[u8], _eth_frame: &ethernet::EthernetFrame) {
    let Some(ip) = Ipv4Packet::parse(data) else {
        return;
    };

    // Verifica se é para nós
    if let Some(config) = super::config() {
        if ip.dst != config.ip && !ip.dst.is_broadcast() && !ip.dst.is_multicast() {
            return;
        }
    }

    // Verifica checksum
    let stored_checksum = ip.checksum;
    let mut header = data[..ip.header_len].to_vec();
    header[10] = 0;
    header[11] = 0;
    let computed = compute_checksum(&header);
    if stored_checksum != computed {
        return; // Checksum inválido
    }

    let payload = &data[ip.header_len..];

    match ip.protocol {
        PROTO_ICMP => {
            icmp::handle_packet(payload, &ip);
        }
        PROTO_UDP => {
            udp::handle_packet(payload, &ip);
        }
        PROTO_TCP => {
            tcp::handle_packet(payload, &ip);
        }
        _ => {
            // Protocolo não suportado
        }
    }
}

/// Envia um pacote IPv4
pub fn send(dst: Ipv4Addr, protocol: u8, payload: &[u8]) -> crate::util::KResult<()> {
    let config = super::config().ok_or(crate::util::KError::NotSupported)?;

    // Resolve MAC do próximo hop
    let dst_mac = arp::resolve_next_hop(dst).ok_or(crate::util::KError::NotSupported)?;

    // Constrói header IPv4
    let total_len = 20 + payload.len();
    let mut packet = Vec::with_capacity(total_len);

    // Version (4) + IHL (5)
    packet.push(0x45);
    // DSCP + ECN
    packet.push(0);
    // Total length
    packet.extend_from_slice(&(total_len as u16).to_be_bytes());
    // Identification
    let id = unsafe {
        PACKET_ID = PACKET_ID.wrapping_add(1);
        PACKET_ID
    };
    packet.extend_from_slice(&id.to_be_bytes());
    // Flags + Fragment offset (Don't Fragment)
    packet.extend_from_slice(&0x4000u16.to_be_bytes());
    // TTL
    packet.push(64);
    // Protocol
    packet.push(protocol);
    // Checksum (placeholder)
    packet.push(0);
    packet.push(0);
    // Source IP
    packet.extend_from_slice(&config.ip.0);
    // Dest IP
    packet.extend_from_slice(&dst.0);

    // Calcula checksum
    let checksum = compute_checksum(&packet);
    packet[10] = (checksum >> 8) as u8;
    packet[11] = (checksum & 0xFF) as u8;

    // Payload
    packet.extend_from_slice(payload);

    // Envia via Ethernet
    super::send_ethernet(dst_mac, ethernet::ETHERTYPE_IPV4, &packet)
}
