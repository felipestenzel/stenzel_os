//! VirtIO Network Device Driver
//!
//! Provides network access via VirtIO protocol.

#![allow(dead_code)]

use alloc::vec::Vec;
use alloc::vec;
use alloc::string::String;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use super::virtqueue::Virtqueue;
use super::{VirtioDevice, VirtioDeviceType, features};

/// Network device feature flags
pub mod net_features {
    pub const VIRTIO_NET_F_CSUM: u64 = 1 << 0;
    pub const VIRTIO_NET_F_GUEST_CSUM: u64 = 1 << 1;
    pub const VIRTIO_NET_F_CTRL_GUEST_OFFLOADS: u64 = 1 << 2;
    pub const VIRTIO_NET_F_MTU: u64 = 1 << 3;
    pub const VIRTIO_NET_F_MAC: u64 = 1 << 5;
    pub const VIRTIO_NET_F_GUEST_TSO4: u64 = 1 << 7;
    pub const VIRTIO_NET_F_GUEST_TSO6: u64 = 1 << 8;
    pub const VIRTIO_NET_F_GUEST_ECN: u64 = 1 << 9;
    pub const VIRTIO_NET_F_GUEST_UFO: u64 = 1 << 10;
    pub const VIRTIO_NET_F_HOST_TSO4: u64 = 1 << 11;
    pub const VIRTIO_NET_F_HOST_TSO6: u64 = 1 << 12;
    pub const VIRTIO_NET_F_HOST_ECN: u64 = 1 << 13;
    pub const VIRTIO_NET_F_HOST_UFO: u64 = 1 << 14;
    pub const VIRTIO_NET_F_MRG_RXBUF: u64 = 1 << 15;
    pub const VIRTIO_NET_F_STATUS: u64 = 1 << 16;
    pub const VIRTIO_NET_F_CTRL_VQ: u64 = 1 << 17;
    pub const VIRTIO_NET_F_CTRL_RX: u64 = 1 << 18;
    pub const VIRTIO_NET_F_CTRL_VLAN: u64 = 1 << 19;
    pub const VIRTIO_NET_F_GUEST_ANNOUNCE: u64 = 1 << 21;
    pub const VIRTIO_NET_F_MQ: u64 = 1 << 22;
    pub const VIRTIO_NET_F_CTRL_MAC_ADDR: u64 = 1 << 23;
    pub const VIRTIO_NET_F_SPEED_DUPLEX: u64 = 1 << 63;
}

/// Network packet header
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct VirtioNetHeader {
    pub flags: u8,
    pub gso_type: u8,
    pub hdr_len: u16,
    pub gso_size: u16,
    pub csum_start: u16,
    pub csum_offset: u16,
    pub num_buffers: u16,
}

/// Network header flags
pub mod header_flags {
    pub const VIRTIO_NET_HDR_F_NEEDS_CSUM: u8 = 1;
    pub const VIRTIO_NET_HDR_F_DATA_VALID: u8 = 2;
    pub const VIRTIO_NET_HDR_F_RSC_INFO: u8 = 4;
}

/// GSO types
pub mod gso_type {
    pub const VIRTIO_NET_HDR_GSO_NONE: u8 = 0;
    pub const VIRTIO_NET_HDR_GSO_TCPV4: u8 = 1;
    pub const VIRTIO_NET_HDR_GSO_UDP: u8 = 3;
    pub const VIRTIO_NET_HDR_GSO_TCPV6: u8 = 4;
    pub const VIRTIO_NET_HDR_GSO_ECN: u8 = 0x80;
}

/// Network device status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetStatus {
    Down,
    Up,
    LinkUp,
}

/// Network device configuration
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct VirtioNetConfig {
    pub mac: [u8; 6],
    pub status: u16,
    pub max_virtqueue_pairs: u16,
    pub mtu: u16,
    pub speed: u32,
    pub duplex: u8,
}

/// MAC address
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MacAddress(pub [u8; 6]);

impl MacAddress {
    pub fn new(bytes: [u8; 6]) -> Self {
        Self(bytes)
    }

    pub fn from_slice(slice: &[u8]) -> Option<Self> {
        if slice.len() >= 6 {
            let mut mac = [0u8; 6];
            mac.copy_from_slice(&slice[..6]);
            Some(Self(mac))
        } else {
            None
        }
    }

    pub fn is_broadcast(&self) -> bool {
        self.0 == [0xff, 0xff, 0xff, 0xff, 0xff, 0xff]
    }

    pub fn is_multicast(&self) -> bool {
        self.0[0] & 0x01 != 0
    }

    pub fn is_unicast(&self) -> bool {
        !self.is_multicast()
    }

    pub fn as_bytes(&self) -> &[u8; 6] {
        &self.0
    }
}

impl core::fmt::Display for MacAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
               self.0[0], self.0[1], self.0[2],
               self.0[3], self.0[4], self.0[5])
    }
}

/// Network statistics
#[derive(Debug, Default)]
pub struct NetStats {
    pub rx_packets: AtomicU64,
    pub tx_packets: AtomicU64,
    pub rx_bytes: AtomicU64,
    pub tx_bytes: AtomicU64,
    pub rx_errors: AtomicU64,
    pub tx_errors: AtomicU64,
    pub rx_dropped: AtomicU64,
    pub tx_dropped: AtomicU64,
}

/// Receive buffer descriptor
struct RxBuffer {
    descriptor_id: u16,
    buffer: Vec<u8>,
}

/// Transmit buffer descriptor
struct TxBuffer {
    descriptor_id: u16,
    buffer: Vec<u8>,
}

/// VirtIO network device
pub struct VirtioNetDevice {
    /// MAC address
    mac: MacAddress,
    /// Device configuration
    config: VirtioNetConfig,
    /// Receive queue
    rx_queue: Virtqueue,
    /// Transmit queue
    tx_queue: Virtqueue,
    /// Control queue (optional)
    ctrl_queue: Option<Virtqueue>,
    /// Negotiated features
    features: u64,
    /// Device status
    status: NetStatus,
    /// Initialized flag
    initialized: AtomicBool,
    /// Receive buffers
    rx_buffers: Vec<RxBuffer>,
    /// Statistics
    stats: NetStats,
    /// MTU
    mtu: u16,
    /// Promiscuous mode
    promiscuous: bool,
    /// All-multicast mode
    allmulti: bool,
}

impl VirtioNetDevice {
    /// Create new network device
    pub fn new(queue_size: u16) -> Self {
        Self {
            mac: MacAddress::default(),
            config: VirtioNetConfig::default(),
            rx_queue: Virtqueue::new(0, queue_size),
            tx_queue: Virtqueue::new(1, queue_size),
            ctrl_queue: None,
            features: 0,
            status: NetStatus::Down,
            initialized: AtomicBool::new(false),
            rx_buffers: Vec::new(),
            stats: NetStats::default(),
            mtu: 1500,
            promiscuous: false,
            allmulti: false,
        }
    }

    /// Get MAC address
    pub fn mac(&self) -> MacAddress {
        self.mac
    }

    /// Get MTU
    pub fn mtu(&self) -> u16 {
        self.mtu
    }

    /// Set MTU
    pub fn set_mtu(&mut self, mtu: u16) -> bool {
        if mtu >= 68 && mtu <= 65535 {
            self.mtu = mtu;
            true
        } else {
            false
        }
    }

    /// Get link status
    pub fn is_link_up(&self) -> bool {
        self.status == NetStatus::LinkUp
    }

    /// Transmit packet
    pub fn transmit(&mut self, packet: &[u8]) -> Result<(), &'static str> {
        if !self.initialized.load(Ordering::Acquire) {
            return Err("Device not initialized");
        }

        if self.status != NetStatus::LinkUp {
            return Err("Link is down");
        }

        if packet.len() > self.mtu as usize + 14 { // 14 bytes for Ethernet header
            return Err("Packet too large");
        }

        // Create header
        let header = VirtioNetHeader::default();

        // In real implementation:
        // 1. Allocate DMA buffer
        // 2. Copy header + packet data
        // 3. Add to TX queue
        // 4. Notify device

        self.stats.tx_packets.fetch_add(1, Ordering::Relaxed);
        self.stats.tx_bytes.fetch_add(packet.len() as u64, Ordering::Relaxed);

        let _ = header;
        Ok(())
    }

    /// Receive packet
    pub fn receive(&mut self) -> Option<Vec<u8>> {
        if !self.initialized.load(Ordering::Acquire) {
            return None;
        }

        // Check for completed RX buffers
        if let Some((desc_id, len)) = self.rx_queue.get_used() {
            // Find the buffer
            if let Some(pos) = self.rx_buffers.iter().position(|b| b.descriptor_id == desc_id) {
                let mut buffer = self.rx_buffers.remove(pos);

                // Skip header
                let header_size = core::mem::size_of::<VirtioNetHeader>();
                if len as usize > header_size {
                    buffer.buffer.truncate(len as usize);
                    let packet = buffer.buffer[header_size..].to_vec();

                    self.stats.rx_packets.fetch_add(1, Ordering::Relaxed);
                    self.stats.rx_bytes.fetch_add(packet.len() as u64, Ordering::Relaxed);

                    // Recycle buffer
                    self.replenish_rx_buffer();

                    return Some(packet);
                }
            }
        }

        None
    }

    /// Replenish receive buffers
    fn replenish_rx_buffer(&mut self) {
        // Allocate new buffer
        let buffer_size = self.mtu as usize + core::mem::size_of::<VirtioNetHeader>() + 14;
        let buffer = vec![0u8; buffer_size];

        // Add to queue (would need physical address in real impl)
        if let Some(desc_id) = self.rx_queue.add_buffer(0, buffer_size as u32, true) {
            self.rx_buffers.push(RxBuffer { descriptor_id: desc_id, buffer });
        }
    }

    /// Set promiscuous mode
    pub fn set_promiscuous(&mut self, enable: bool) {
        self.promiscuous = enable;
        // Would send control command in real implementation
    }

    /// Set all-multicast mode
    pub fn set_allmulti(&mut self, enable: bool) {
        self.allmulti = enable;
        // Would send control command in real implementation
    }

    /// Get statistics
    pub fn stats(&self) -> &NetStats {
        &self.stats
    }

    /// Format status
    pub fn format_status(&self) -> String {
        alloc::format!(
            "VirtIO Net: MAC={} MTU={} Status={:?}",
            self.mac, self.mtu, self.status
        )
    }
}

impl VirtioDevice for VirtioNetDevice {
    fn device_type(&self) -> VirtioDeviceType {
        VirtioDeviceType::Network
    }

    fn init(&mut self) -> Result<(), &'static str> {
        // Read MAC address from config
        // In real implementation, read from MMIO/PCI config space
        self.mac = MacAddress::new([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]);
        self.config.mtu = 1500;

        // Allocate RX buffers
        for _ in 0..16 {
            self.replenish_rx_buffer();
        }

        Ok(())
    }

    fn reset(&mut self) {
        self.initialized.store(false, Ordering::Release);
        self.status = NetStatus::Down;
        self.rx_buffers.clear();
        self.rx_queue = Virtqueue::new(0, self.rx_queue.size);
        self.tx_queue = Virtqueue::new(1, self.tx_queue.size);
    }

    fn negotiate_features(&mut self, offered: u64) -> u64 {
        let mut wanted = features::VIRTIO_F_VERSION_1;

        // Request network-specific features
        if offered & net_features::VIRTIO_NET_F_MAC != 0 {
            wanted |= net_features::VIRTIO_NET_F_MAC;
        }
        if offered & net_features::VIRTIO_NET_F_STATUS != 0 {
            wanted |= net_features::VIRTIO_NET_F_STATUS;
        }
        if offered & net_features::VIRTIO_NET_F_MTU != 0 {
            wanted |= net_features::VIRTIO_NET_F_MTU;
        }
        if offered & net_features::VIRTIO_NET_F_CSUM != 0 {
            wanted |= net_features::VIRTIO_NET_F_CSUM;
        }
        if offered & net_features::VIRTIO_NET_F_GUEST_CSUM != 0 {
            wanted |= net_features::VIRTIO_NET_F_GUEST_CSUM;
        }
        if offered & net_features::VIRTIO_NET_F_CTRL_VQ != 0 {
            wanted |= net_features::VIRTIO_NET_F_CTRL_VQ;
            // Create control queue
            self.ctrl_queue = Some(Virtqueue::new(2, 64));
        }
        if offered & net_features::VIRTIO_NET_F_MRG_RXBUF != 0 {
            wanted |= net_features::VIRTIO_NET_F_MRG_RXBUF;
        }

        self.features = wanted & offered;
        self.features
    }

    fn activate(&mut self) -> Result<(), &'static str> {
        self.initialized.store(true, Ordering::Release);
        self.status = NetStatus::LinkUp;
        crate::kprintln!("virtio-net: Activated, MAC={}", self.mac);
        Ok(())
    }

    fn handle_interrupt(&mut self) {
        // Process RX completions
        while let Some(_packet) = self.receive() {
            // Would pass to network stack
        }
    }
}

/// Network device manager
pub struct VirtioNetManager {
    devices: Vec<VirtioNetDevice>,
}

impl VirtioNetManager {
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
        }
    }

    pub fn add_device(&mut self, device: VirtioNetDevice) -> usize {
        let idx = self.devices.len();
        self.devices.push(device);
        idx
    }

    pub fn get_device(&mut self, idx: usize) -> Option<&mut VirtioNetDevice> {
        self.devices.get_mut(idx)
    }

    pub fn device_count(&self) -> usize {
        self.devices.len()
    }
}

impl Default for VirtioNetManager {
    fn default() -> Self {
        Self::new()
    }
}
