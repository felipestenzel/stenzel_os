//! ARP (Address Resolution Protocol)

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use super::{MacAddr, Ipv4Addr, ethernet};
use crate::sync::IrqSafeMutex;

const ARP_HTYPE_ETHERNET: u16 = 1;
const ARP_PTYPE_IPV4: u16 = 0x0800;
const ARP_OP_REQUEST: u16 = 1;
const ARP_OP_REPLY: u16 = 2;

/// Cache ARP
static ARP_CACHE: IrqSafeMutex<BTreeMap<u32, MacAddr>> = IrqSafeMutex::new(BTreeMap::new());

/// Pacote ARP parseado
#[derive(Debug)]
pub struct ArpPacket {
    pub htype: u16,
    pub ptype: u16,
    pub hlen: u8,
    pub plen: u8,
    pub oper: u16,
    pub sha: MacAddr,
    pub spa: Ipv4Addr,
    pub tha: MacAddr,
    pub tpa: Ipv4Addr,
}

impl ArpPacket {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 28 {
            return None;
        }

        Some(Self {
            htype: u16::from_be_bytes([data[0], data[1]]),
            ptype: u16::from_be_bytes([data[2], data[3]]),
            hlen: data[4],
            plen: data[5],
            oper: u16::from_be_bytes([data[6], data[7]]),
            sha: MacAddr::from_bytes(&data[8..14]),
            spa: Ipv4Addr::from_bytes(&data[14..18]),
            tha: MacAddr::from_bytes(&data[18..24]),
            tpa: Ipv4Addr::from_bytes(&data[24..28]),
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(28);
        buf.extend_from_slice(&self.htype.to_be_bytes());
        buf.extend_from_slice(&self.ptype.to_be_bytes());
        buf.push(self.hlen);
        buf.push(self.plen);
        buf.extend_from_slice(&self.oper.to_be_bytes());
        buf.extend_from_slice(&self.sha.0);
        buf.extend_from_slice(&self.spa.0);
        buf.extend_from_slice(&self.tha.0);
        buf.extend_from_slice(&self.tpa.0);
        buf
    }
}

/// Inicializa o subsistema ARP
pub fn init() {
    // Cache começa vazio
}

/// Processa um pacote ARP recebido
pub fn handle_packet(data: &[u8], _eth_frame: &ethernet::EthernetFrame) {
    let Some(arp) = ArpPacket::parse(data) else {
        return;
    };

    // Só processamos Ethernet/IPv4
    if arp.htype != ARP_HTYPE_ETHERNET || arp.ptype != ARP_PTYPE_IPV4 {
        return;
    }

    // Atualiza cache com o sender
    {
        let mut cache = ARP_CACHE.lock();
        cache.insert(arp.spa.to_u32(), arp.sha);
    }

    // Se é um request para nosso IP, responde
    if arp.oper == ARP_OP_REQUEST {
        if let Some(config) = super::config() {
            if arp.tpa == config.ip {
                send_reply(&arp, &config);
            }
        }
    }
}

/// Envia uma resposta ARP
fn send_reply(request: &ArpPacket, config: &super::NetConfig) {
    let reply = ArpPacket {
        htype: ARP_HTYPE_ETHERNET,
        ptype: ARP_PTYPE_IPV4,
        hlen: 6,
        plen: 4,
        oper: ARP_OP_REPLY,
        sha: MacAddr(config.mac),
        spa: config.ip,
        tha: request.sha,
        tpa: request.spa,
    };

    let payload = reply.to_bytes();
    let _ = super::send_ethernet(request.sha, ethernet::ETHERTYPE_ARP, &payload);
}

/// Resolve um IP para MAC (blocking)
pub fn resolve(ip: Ipv4Addr) -> Option<MacAddr> {
    // Broadcast sempre vai para broadcast MAC
    if ip.is_broadcast() {
        return Some(MacAddr::BROADCAST);
    }

    // Verifica cache primeiro
    {
        let cache = ARP_CACHE.lock();
        if let Some(&mac) = cache.get(&ip.to_u32()) {
            return Some(mac);
        }
    }

    // Envia ARP request
    if let Some(config) = super::config() {
        let request = ArpPacket {
            htype: ARP_HTYPE_ETHERNET,
            ptype: ARP_PTYPE_IPV4,
            hlen: 6,
            plen: 4,
            oper: ARP_OP_REQUEST,
            sha: MacAddr(config.mac),
            spa: config.ip,
            tha: MacAddr::ZERO,
            tpa: ip,
        };

        let payload = request.to_bytes();
        let _ = super::send_ethernet(MacAddr::BROADCAST, ethernet::ETHERTYPE_ARP, &payload);

        // Espera resposta (polling simples)
        for _ in 0..1000 {
            super::poll();

            let cache = ARP_CACHE.lock();
            if let Some(&mac) = cache.get(&ip.to_u32()) {
                return Some(mac);
            }
            drop(cache);

            // Pequeno delay
            for _ in 0..10000 {
                core::hint::spin_loop();
            }
        }
    }

    None
}

/// Obtém o MAC do gateway para IPs fora da rede local
pub fn resolve_gateway() -> Option<MacAddr> {
    let config = super::config()?;
    resolve(config.gateway)
}

/// Verifica se um IP está na rede local
pub fn is_local(ip: Ipv4Addr) -> bool {
    if let Some(config) = super::config() {
        let local = config.ip.to_u32() & config.netmask.to_u32();
        let target = ip.to_u32() & config.netmask.to_u32();
        local == target
    } else {
        false
    }
}

/// Resolve o próximo hop para um IP de destino
pub fn resolve_next_hop(dst: Ipv4Addr) -> Option<MacAddr> {
    if is_local(dst) {
        resolve(dst)
    } else {
        resolve_gateway()
    }
}
