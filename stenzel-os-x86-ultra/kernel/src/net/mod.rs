//! Network stack do kernel.
//!
//! Implementa:
//! - Ethernet (Layer 2)
//! - ARP (Address Resolution)
//! - IPv4 (Layer 3)
//! - ICMP (ping)
//! - UDP (Layer 4)
//! - TCP (Layer 4)
//! - DNS (Domain Name Resolution)
//! - DHCP (Dynamic Host Configuration)

#![allow(dead_code)]

extern crate alloc;

pub mod ethernet;
pub mod arp;
pub mod ipv4;
pub mod ipv6;
pub mod icmp;
pub mod udp;
pub mod tcp;
pub mod socket;
pub mod dns;
pub mod dhcp;
pub mod http;
pub mod tls;
pub mod ssh;
pub mod wifi;
pub mod ntp;
pub mod ftp;

use alloc::vec::Vec;
use crate::sync::IrqSafeMutex;

/// Configuração de rede
#[derive(Debug, Clone, Copy)]
pub struct NetConfig {
    pub mac: [u8; 6],
    pub ip: Ipv4Addr,
    pub netmask: Ipv4Addr,
    pub gateway: Ipv4Addr,
}

impl Default for NetConfig {
    fn default() -> Self {
        Self {
            mac: [0; 6],
            ip: Ipv4Addr::new(10, 0, 2, 15),      // QEMU user mode default
            netmask: Ipv4Addr::new(255, 255, 255, 0),
            gateway: Ipv4Addr::new(10, 0, 2, 2),   // QEMU gateway
        }
    }
}

/// Endereço IPv4
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Ipv4Addr(pub [u8; 4]);

impl Ipv4Addr {
    pub const BROADCAST: Self = Self([255, 255, 255, 255]);
    pub const UNSPECIFIED: Self = Self([0, 0, 0, 0]);

    pub const fn new(a: u8, b: u8, c: u8, d: u8) -> Self {
        Self([a, b, c, d])
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut arr = [0u8; 4];
        arr.copy_from_slice(&bytes[..4]);
        Self(arr)
    }

    pub fn as_bytes(&self) -> &[u8; 4] {
        &self.0
    }

    pub fn to_u32(&self) -> u32 {
        u32::from_be_bytes(self.0)
    }

    pub fn from_u32(val: u32) -> Self {
        Self(val.to_be_bytes())
    }

    pub fn is_broadcast(&self) -> bool {
        *self == Self::BROADCAST
    }

    pub fn is_multicast(&self) -> bool {
        self.0[0] >= 224 && self.0[0] <= 239
    }
}

impl core::fmt::Display for Ipv4Addr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}.{}.{}.{}", self.0[0], self.0[1], self.0[2], self.0[3])
    }
}

/// MAC address
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct MacAddr(pub [u8; 6]);

impl MacAddr {
    pub const BROADCAST: Self = Self([0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
    pub const ZERO: Self = Self([0; 6]);

    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut arr = [0u8; 6];
        arr.copy_from_slice(&bytes[..6]);
        Self(arr)
    }

    pub fn as_bytes(&self) -> &[u8; 6] {
        &self.0
    }

    pub fn is_broadcast(&self) -> bool {
        *self == Self::BROADCAST
    }
}

impl core::fmt::Display for MacAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5])
    }
}

/// Configuração global de rede
static NET_CONFIG: IrqSafeMutex<Option<NetConfig>> = IrqSafeMutex::new(None);

/// Inicializa o network stack
pub fn init() {
    // Inicializa o driver de rede
    crate::drivers::net::init();

    // Obtém o MAC do driver
    if let Some(mac) = crate::drivers::net::get_mac() {
        let config = NetConfig {
            mac,
            ..Default::default()
        };

        crate::kprintln!("net: MAC = {}", MacAddr(mac));
        crate::kprintln!("net: IP = {}", config.ip);
        crate::kprintln!("net: Gateway = {}", config.gateway);

        *NET_CONFIG.lock() = Some(config);

        // Inicializa subsistemas
        arp::init();
        socket::init();
        dns::init();
        http::init();
        tls::init();

        crate::kprintln!("net: stack inicializado");
    } else {
        crate::kprintln!("net: nenhuma interface de rede disponível");
    }
}

/// Retorna a configuração de rede atual
pub fn config() -> Option<NetConfig> {
    *NET_CONFIG.lock()
}

/// Processa pacotes recebidos
pub fn poll() {
    while let Some(packet) = crate::drivers::net::recv() {
        if let Some(eth_frame) = ethernet::EthernetFrame::parse(&packet) {
            ethernet::handle_frame(eth_frame, &packet);
        }
    }
}

/// Envia um pacote Ethernet
pub fn send_ethernet(dst_mac: MacAddr, ethertype: u16, payload: &[u8]) -> crate::util::KResult<()> {
    let config = config().ok_or(crate::util::KError::NotSupported)?;

    let mut packet = Vec::with_capacity(14 + payload.len());

    // Ethernet header
    packet.extend_from_slice(&dst_mac.0);
    packet.extend_from_slice(&config.mac);
    packet.extend_from_slice(&ethertype.to_be_bytes());
    packet.extend_from_slice(payload);

    // Padding mínimo de 60 bytes
    while packet.len() < 60 {
        packet.push(0);
    }

    crate::drivers::net::send(&packet)
}
