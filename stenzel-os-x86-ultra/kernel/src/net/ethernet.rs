//! Ethernet (Layer 2)

#![allow(dead_code)]

use super::{MacAddr, arp, ipv4};

/// EtherTypes
pub const ETHERTYPE_IPV4: u16 = 0x0800;
pub const ETHERTYPE_ARP: u16 = 0x0806;
pub const ETHERTYPE_IPV6: u16 = 0x86DD;

/// Frame Ethernet parseado
#[derive(Debug)]
pub struct EthernetFrame {
    pub dst: MacAddr,
    pub src: MacAddr,
    pub ethertype: u16,
    pub payload_offset: usize,
}

impl EthernetFrame {
    /// Parseia um frame Ethernet
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 14 {
            return None;
        }

        let dst = MacAddr::from_bytes(&data[0..6]);
        let src = MacAddr::from_bytes(&data[6..12]);
        let ethertype = u16::from_be_bytes([data[12], data[13]]);

        Some(Self {
            dst,
            src,
            ethertype,
            payload_offset: 14,
        })
    }
}

/// Processa um frame Ethernet recebido
pub fn handle_frame(frame: EthernetFrame, raw: &[u8]) {
    let payload = &raw[frame.payload_offset..];

    match frame.ethertype {
        ETHERTYPE_ARP => {
            arp::handle_packet(payload, &frame);
        }
        ETHERTYPE_IPV4 => {
            ipv4::handle_packet(payload, &frame);
        }
        _ => {
            // Ignora outros tipos
        }
    }
}
