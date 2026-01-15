//! ICMP (Internet Control Message Protocol)

#![allow(dead_code)]

use alloc::vec::Vec;

use super::ipv4::{self, Ipv4Packet};
use super::Ipv4Addr;

/// Tipos ICMP
pub const ICMP_ECHO_REPLY: u8 = 0;
pub const ICMP_DEST_UNREACHABLE: u8 = 3;
pub const ICMP_ECHO_REQUEST: u8 = 8;
pub const ICMP_TIME_EXCEEDED: u8 = 11;

/// Mensagem ICMP parseada
#[derive(Debug)]
pub struct IcmpMessage {
    pub msg_type: u8,
    pub code: u8,
    pub checksum: u16,
    pub rest_of_header: [u8; 4],
    pub data_offset: usize,
}

impl IcmpMessage {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 8 {
            return None;
        }

        Some(Self {
            msg_type: data[0],
            code: data[1],
            checksum: u16::from_be_bytes([data[2], data[3]]),
            rest_of_header: [data[4], data[5], data[6], data[7]],
            data_offset: 8,
        })
    }

    /// Retorna identifier e sequence para Echo Request/Reply
    pub fn echo_id_seq(&self) -> (u16, u16) {
        let id = u16::from_be_bytes([self.rest_of_header[0], self.rest_of_header[1]]);
        let seq = u16::from_be_bytes([self.rest_of_header[2], self.rest_of_header[3]]);
        (id, seq)
    }
}

/// Calcula checksum ICMP (mesmo algoritmo do IPv4)
fn compute_checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;

    for i in (0..data.len()).step_by(2) {
        let word = if i + 1 < data.len() {
            u16::from_be_bytes([data[i], data[i + 1]])
        } else {
            u16::from_be_bytes([data[i], 0])
        };
        sum += word as u32;
    }

    while sum > 0xFFFF {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }

    !(sum as u16)
}

/// Processa um pacote ICMP recebido
pub fn handle_packet(data: &[u8], ip: &Ipv4Packet) {
    let Some(icmp) = IcmpMessage::parse(data) else {
        return;
    };

    // Verifica checksum
    let stored = icmp.checksum;
    let mut check_data = data.to_vec();
    check_data[2] = 0;
    check_data[3] = 0;
    if compute_checksum(&check_data) != stored {
        return;
    }

    match icmp.msg_type {
        ICMP_ECHO_REQUEST => {
            // Responde com Echo Reply
            let payload = &data[icmp.data_offset..];
            send_echo_reply(ip.src, icmp.echo_id_seq(), payload);
        }
        ICMP_ECHO_REPLY => {
            // Notifica waiters (para implementação de ping)
            let (id, seq) = icmp.echo_id_seq();
            notify_ping_reply(ip.src, id, seq);
        }
        _ => {
            // Ignora outros tipos por enquanto
        }
    }
}

/// Envia um Echo Reply
fn send_echo_reply(dst: Ipv4Addr, (id, seq): (u16, u16), payload: &[u8]) {
    let mut packet = Vec::with_capacity(8 + payload.len());

    // Type = Echo Reply
    packet.push(ICMP_ECHO_REPLY);
    // Code
    packet.push(0);
    // Checksum (placeholder)
    packet.push(0);
    packet.push(0);
    // Identifier
    packet.extend_from_slice(&id.to_be_bytes());
    // Sequence
    packet.extend_from_slice(&seq.to_be_bytes());
    // Data
    packet.extend_from_slice(payload);

    // Calcula checksum
    let checksum = compute_checksum(&packet);
    packet[2] = (checksum >> 8) as u8;
    packet[3] = (checksum & 0xFF) as u8;

    let _ = ipv4::send(dst, ipv4::PROTO_ICMP, &packet);
}

/// Envia um Echo Request (ping)
pub fn send_ping(dst: Ipv4Addr, id: u16, seq: u16, payload: &[u8]) -> crate::util::KResult<()> {
    let mut packet = Vec::with_capacity(8 + payload.len());

    // Type = Echo Request
    packet.push(ICMP_ECHO_REQUEST);
    // Code
    packet.push(0);
    // Checksum (placeholder)
    packet.push(0);
    packet.push(0);
    // Identifier
    packet.extend_from_slice(&id.to_be_bytes());
    // Sequence
    packet.extend_from_slice(&seq.to_be_bytes());
    // Data
    packet.extend_from_slice(payload);

    // Calcula checksum
    let checksum = compute_checksum(&packet);
    packet[2] = (checksum >> 8) as u8;
    packet[3] = (checksum & 0xFF) as u8;

    ipv4::send(dst, ipv4::PROTO_ICMP, &packet)
}

// Estrutura simples para tracking de ping replies
use crate::sync::IrqSafeMutex;
use alloc::collections::VecDeque;

struct PingReply {
    src: Ipv4Addr,
    id: u16,
    seq: u16,
}

static PING_REPLIES: IrqSafeMutex<VecDeque<PingReply>> = IrqSafeMutex::new(VecDeque::new());

fn notify_ping_reply(src: Ipv4Addr, id: u16, seq: u16) {
    let mut replies = PING_REPLIES.lock();
    if replies.len() < 64 {
        replies.push_back(PingReply { src, id, seq });
    }
}

/// Espera por um ping reply específico
pub fn wait_ping_reply(dst: Ipv4Addr, id: u16, seq: u16, timeout_ms: u32) -> bool {
    let deadline = timeout_ms as u64 * 1000; // Simplificado
    let mut waited: u64 = 0;

    while waited < deadline {
        super::poll();

        {
            let mut replies = PING_REPLIES.lock();
            if let Some(pos) = replies.iter().position(|r| r.src == dst && r.id == id && r.seq == seq) {
                replies.remove(pos);
                return true;
            }
        }

        // Delay
        for _ in 0..10000 {
            core::hint::spin_loop();
        }
        waited += 1;
    }

    false
}
