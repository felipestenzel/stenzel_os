//! UDP (User Datagram Protocol)

#![allow(dead_code)]

use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use alloc::collections::VecDeque;

use super::ipv4::{self, Ipv4Packet};
use super::Ipv4Addr;
use crate::sync::IrqSafeMutex;

/// Datagrama UDP parseado
#[derive(Debug)]
pub struct UdpDatagram {
    pub src_port: u16,
    pub dst_port: u16,
    pub length: u16,
    pub checksum: u16,
    pub data_offset: usize,
}

impl UdpDatagram {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 8 {
            return None;
        }

        Some(Self {
            src_port: u16::from_be_bytes([data[0], data[1]]),
            dst_port: u16::from_be_bytes([data[2], data[3]]),
            length: u16::from_be_bytes([data[4], data[5]]),
            checksum: u16::from_be_bytes([data[6], data[7]]),
            data_offset: 8,
        })
    }
}

/// Datagrama recebido
#[derive(Debug, Clone)]
pub struct ReceivedDatagram {
    pub src_addr: Ipv4Addr,
    pub src_port: u16,
    pub data: Vec<u8>,
}

/// Buffer de recepção para uma porta
struct UdpSocket {
    queue: VecDeque<ReceivedDatagram>,
    max_queue: usize,
}

impl UdpSocket {
    fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            max_queue: 32,
        }
    }
}

/// Portas UDP em listening
static UDP_SOCKETS: IrqSafeMutex<BTreeMap<u16, UdpSocket>> = IrqSafeMutex::new(BTreeMap::new());

/// Próxima porta efêmera (atômica para thread safety)
static NEXT_EPHEMERAL_PORT: core::sync::atomic::AtomicU16 = core::sync::atomic::AtomicU16::new(49152);

/// Calcula checksum UDP (com pseudo-header)
fn compute_checksum(src_ip: Ipv4Addr, dst_ip: Ipv4Addr, udp_data: &[u8]) -> u16 {
    let mut sum: u32 = 0;

    // Pseudo-header
    let src = src_ip.0;
    let dst = dst_ip.0;
    sum += u16::from_be_bytes([src[0], src[1]]) as u32;
    sum += u16::from_be_bytes([src[2], src[3]]) as u32;
    sum += u16::from_be_bytes([dst[0], dst[1]]) as u32;
    sum += u16::from_be_bytes([dst[2], dst[3]]) as u32;
    sum += 17u32; // Protocol UDP
    sum += udp_data.len() as u32;

    // UDP header + data
    for i in (0..udp_data.len()).step_by(2) {
        let word = if i + 1 < udp_data.len() {
            u16::from_be_bytes([udp_data[i], udp_data[i + 1]])
        } else {
            u16::from_be_bytes([udp_data[i], 0])
        };
        sum += word as u32;
    }

    while sum > 0xFFFF {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }

    let result = !(sum as u16);
    if result == 0 { 0xFFFF } else { result }
}

/// Processa um pacote UDP recebido
pub fn handle_packet(data: &[u8], ip: &Ipv4Packet) {
    let Some(udp) = UdpDatagram::parse(data) else {
        return;
    };

    // Verifica checksum se presente
    if udp.checksum != 0 {
        let mut check_data = data.to_vec();
        check_data[6] = 0;
        check_data[7] = 0;
        let computed = compute_checksum(ip.src, ip.dst, &check_data);
        if udp.checksum != computed {
            return;
        }
    }

    let payload_len = (udp.length as usize).saturating_sub(8);
    let payload = &data[udp.data_offset..udp.data_offset + payload_len.min(data.len() - udp.data_offset)];

    // Entrega ao socket
    let mut sockets = UDP_SOCKETS.lock();
    if let Some(socket) = sockets.get_mut(&udp.dst_port) {
        if socket.queue.len() < socket.max_queue {
            socket.queue.push_back(ReceivedDatagram {
                src_addr: ip.src,
                src_port: udp.src_port,
                data: payload.to_vec(),
            });
        }
    }
}

/// Bind em uma porta UDP
pub fn bind(port: u16) -> crate::util::KResult<()> {
    let mut sockets = UDP_SOCKETS.lock();
    if sockets.contains_key(&port) {
        return Err(crate::util::KError::AlreadyExists);
    }
    sockets.insert(port, UdpSocket::new());
    Ok(())
}

/// Unbind de uma porta UDP
pub fn unbind(port: u16) {
    let mut sockets = UDP_SOCKETS.lock();
    sockets.remove(&port);
}

/// Aloca uma porta efêmera
pub fn allocate_port() -> u16 {
    use core::sync::atomic::Ordering;
    let mut sockets = UDP_SOCKETS.lock();
    loop {
        let p = NEXT_EPHEMERAL_PORT.fetch_add(1, Ordering::Relaxed);
        let port = if p >= 65534 {
            NEXT_EPHEMERAL_PORT.store(49152, Ordering::Relaxed);
            49152
        } else {
            p
        };
        if !sockets.contains_key(&port) {
            sockets.insert(port, UdpSocket::new());
            return port;
        }
    }
}

/// Envia um datagrama UDP
pub fn send(src_port: u16, dst_addr: Ipv4Addr, dst_port: u16, payload: &[u8]) -> crate::util::KResult<()> {
    let config = super::config().ok_or(crate::util::KError::NotSupported)?;

    let udp_len = 8 + payload.len();
    let mut packet = Vec::with_capacity(udp_len);

    // Source port
    packet.extend_from_slice(&src_port.to_be_bytes());
    // Dest port
    packet.extend_from_slice(&dst_port.to_be_bytes());
    // Length
    packet.extend_from_slice(&(udp_len as u16).to_be_bytes());
    // Checksum (placeholder)
    packet.push(0);
    packet.push(0);
    // Data
    packet.extend_from_slice(payload);

    // Calcula checksum
    let checksum = compute_checksum(config.ip, dst_addr, &packet);
    packet[6] = (checksum >> 8) as u8;
    packet[7] = (checksum & 0xFF) as u8;

    ipv4::send(dst_addr, ipv4::PROTO_UDP, &packet)
}

/// Recebe um datagrama UDP (non-blocking)
pub fn recv(port: u16) -> Option<ReceivedDatagram> {
    let mut sockets = UDP_SOCKETS.lock();
    if let Some(socket) = sockets.get_mut(&port) {
        socket.queue.pop_front()
    } else {
        None
    }
}

/// Recebe um datagrama UDP (blocking com timeout)
pub fn recv_timeout(port: u16, timeout_ms: u32) -> Option<ReceivedDatagram> {
    let deadline = timeout_ms as u64 * 1000;
    let mut waited: u64 = 0;

    while waited < deadline {
        super::poll();

        if let Some(dgram) = recv(port) {
            return Some(dgram);
        }

        for _ in 0..10000 {
            core::hint::spin_loop();
        }
        waited += 1;
    }

    None
}

/// Verifica se há dados disponíveis para leitura (para poll/select)
pub fn has_data(port: u16) -> bool {
    let sockets = UDP_SOCKETS.lock();
    if let Some(socket) = sockets.get(&port) {
        !socket.queue.is_empty()
    } else {
        false
    }
}
