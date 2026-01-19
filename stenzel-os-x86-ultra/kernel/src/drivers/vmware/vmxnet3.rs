//! VMware VMXNET3 Network Driver
//!
//! High-performance paravirtualized network adapter.

#![allow(dead_code)]

use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::vec;
use alloc::string::String;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// VMXNET3 PCI device IDs
pub const VMXNET3_VENDOR_ID: u16 = 0x15AD;
pub const VMXNET3_DEVICE_ID: u16 = 0x07B0;

/// VMXNET3 register offsets
mod regs {
    pub const VRRS: u32 = 0x0;      // VMXNET3 Revision Report Selection
    pub const UVRS: u32 = 0x8;      // UPT Version Report Selection
    pub const DSL: u32 = 0x10;      // Driver Shared Low
    pub const DSH: u32 = 0x18;      // Driver Shared High
    pub const CMD: u32 = 0x20;      // Command
    pub const MACL: u32 = 0x28;     // MAC Address Low
    pub const MACH: u32 = 0x30;     // MAC Address High
    pub const ICR: u32 = 0x38;      // Interrupt Cause
    pub const IMR: u32 = 0x40;      // Interrupt Mask
    pub const TXPROD: u32 = 0x600;  // TX Producer Index
    pub const RXPROD: u32 = 0x800;  // RX Producer Index (1)
    pub const RXPROD2: u32 = 0xA00; // RX Producer Index (2)
}

/// VMXNET3 commands
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum Vmxnet3Cmd {
    FirstSet = 0xCAFE0000,
    Activate = 0xCAFE0001,
    QuiesceDevice = 0xCAFE0002,
    ResetDevice = 0xCAFE0003,
    UpdateRxMode = 0xCAFE0004,
    UpdateMacFilters = 0xCAFE0005,
    UpdateVlanFilters = 0xCAFE0006,
    UpdateFeature = 0xCAFE0007,
    GetStatus = 0xCAFE0008,
    GetMacHi = 0xCAFE0009,
    GetMacLo = 0xCAFE000A,
    GetQueueStatistics = 0xCAFE000B,
    GetLinkStatus = 0xCAFE000C,
    GetStatsSize = 0xCAFE000D,
    GetStatsData = 0xCAFE000E,
    GetConf = 0xCAFE000F,
    GetUptFeature = 0xCAFE0010,
}

/// TX descriptor
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Vmxnet3TxDesc {
    pub addr: u64,
    pub len: u32,
    pub gen: u8,
    pub reserved: u8,
    pub dtype: u8,
    pub ext1: u8,
    pub msscof: u16,
    pub hlen: u16,
    pub om: u8,
    pub eop: u8,
    pub cq: u8,
    pub ext2: u8,
}

/// TX completion descriptor
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Vmxnet3TxCompDesc {
    pub idx: u32,
    pub reserved: [u32; 2],
    pub gen: u8,
    pub rsvd: u8,
    pub type_flags: u8,
    pub eop: u8,
}

/// RX descriptor
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Vmxnet3RxDesc {
    pub addr: u64,
    pub len: u32,
    pub btype: u8,
    pub dtype: u8,
    pub gen: u8,
    pub reserved: u8,
}

/// RX completion descriptor
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Vmxnet3RxCompDesc {
    pub idx: u32,
    pub len: u32,
    pub reserved: u32,
    pub gen: u8,
    pub rsvd: u8,
    pub type_flags: u8,
    pub err: u8,
}

/// Link state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkState {
    Down,
    Up,
}

/// Driver statistics
#[derive(Debug, Default)]
pub struct Vmxnet3Stats {
    pub tx_packets: AtomicU64,
    pub rx_packets: AtomicU64,
    pub tx_bytes: AtomicU64,
    pub rx_bytes: AtomicU64,
    pub tx_errors: AtomicU64,
    pub rx_errors: AtomicU64,
}

/// TX queue
pub struct Vmxnet3TxQueue {
    pub desc_ring: Vec<Vmxnet3TxDesc>,
    pub comp_ring: Vec<Vmxnet3TxCompDesc>,
    pub producer_idx: u32,
    pub consumer_idx: u32,
    pub comp_consumer_idx: u32,
    pub gen: u8,
    pub comp_gen: u8,
}

impl Vmxnet3TxQueue {
    pub fn new(size: usize) -> Self {
        Self {
            desc_ring: vec![Vmxnet3TxDesc::default(); size],
            comp_ring: vec![Vmxnet3TxCompDesc::default(); size],
            producer_idx: 0,
            consumer_idx: 0,
            comp_consumer_idx: 0,
            gen: 1,
            comp_gen: 1,
        }
    }

    pub fn is_full(&self) -> bool {
        let next = (self.producer_idx + 1) % self.desc_ring.len() as u32;
        next == self.consumer_idx
    }
}

/// RX queue
pub struct Vmxnet3RxQueue {
    pub desc_ring: Vec<Vmxnet3RxDesc>,
    pub comp_ring: Vec<Vmxnet3RxCompDesc>,
    pub producer_idx: u32,
    pub consumer_idx: u32,
    pub comp_consumer_idx: u32,
    pub gen: u8,
    pub comp_gen: u8,
    pub buffers: Vec<Vec<u8>>,
}

impl Vmxnet3RxQueue {
    pub fn new(size: usize, buffer_size: usize) -> Self {
        let mut buffers = Vec::with_capacity(size);
        for _ in 0..size {
            buffers.push(vec![0u8; buffer_size]);
        }

        Self {
            desc_ring: vec![Vmxnet3RxDesc::default(); size],
            comp_ring: vec![Vmxnet3RxCompDesc::default(); size],
            producer_idx: 0,
            consumer_idx: 0,
            comp_consumer_idx: 0,
            gen: 1,
            comp_gen: 1,
            buffers,
        }
    }
}

/// VMXNET3 device
pub struct Vmxnet3Device {
    /// MMIO base address
    mmio_base: u64,
    /// MAC address
    mac: [u8; 6],
    /// Link state
    link_state: LinkState,
    /// MTU
    mtu: u16,
    /// TX queue
    tx_queue: Vmxnet3TxQueue,
    /// RX queue
    rx_queue: Vmxnet3RxQueue,
    /// Initialized
    initialized: AtomicBool,
    /// Statistics
    stats: Vmxnet3Stats,
}

impl Vmxnet3Device {
    /// Queue size
    const QUEUE_SIZE: usize = 256;
    /// RX buffer size
    const RX_BUFFER_SIZE: usize = 2048;

    /// Create new device
    pub fn new(mmio_base: u64) -> Self {
        Self {
            mmio_base,
            mac: [0; 6],
            link_state: LinkState::Down,
            mtu: 1500,
            tx_queue: Vmxnet3TxQueue::new(Self::QUEUE_SIZE),
            rx_queue: Vmxnet3RxQueue::new(Self::QUEUE_SIZE, Self::RX_BUFFER_SIZE),
            initialized: AtomicBool::new(false),
            stats: Vmxnet3Stats::default(),
        }
    }

    /// Read register
    fn read_reg(&self, offset: u32) -> u32 {
        unsafe {
            let ptr = (self.mmio_base + offset as u64) as *const u32;
            core::ptr::read_volatile(ptr)
        }
    }

    /// Write register
    fn write_reg(&self, offset: u32, value: u32) {
        unsafe {
            let ptr = (self.mmio_base + offset as u64) as *mut u32;
            core::ptr::write_volatile(ptr, value);
        }
    }

    /// Send command
    fn send_cmd(&self, cmd: Vmxnet3Cmd) {
        self.write_reg(regs::CMD, cmd as u32);
    }

    /// Initialize device
    pub fn init(&mut self) -> Result<(), &'static str> {
        // Reset device
        self.send_cmd(Vmxnet3Cmd::ResetDevice);

        // Get MAC address
        self.send_cmd(Vmxnet3Cmd::GetMacLo);
        let mac_lo = self.read_reg(regs::CMD);
        self.send_cmd(Vmxnet3Cmd::GetMacHi);
        let mac_hi = self.read_reg(regs::CMD);

        self.mac[0] = mac_lo as u8;
        self.mac[1] = (mac_lo >> 8) as u8;
        self.mac[2] = (mac_lo >> 16) as u8;
        self.mac[3] = (mac_lo >> 24) as u8;
        self.mac[4] = mac_hi as u8;
        self.mac[5] = (mac_hi >> 8) as u8;

        // Set up RX buffers
        for i in 0..self.rx_queue.desc_ring.len() {
            let desc = &mut self.rx_queue.desc_ring[i];
            desc.addr = self.rx_queue.buffers[i].as_ptr() as u64;
            desc.len = Self::RX_BUFFER_SIZE as u32;
            desc.gen = self.rx_queue.gen;
        }

        // Activate device
        self.send_cmd(Vmxnet3Cmd::Activate);

        // Check link status
        self.send_cmd(Vmxnet3Cmd::GetLinkStatus);
        let link = self.read_reg(regs::CMD);
        self.link_state = if link != 0 { LinkState::Up } else { LinkState::Down };

        self.initialized.store(true, Ordering::Release);
        crate::kprintln!("vmxnet3: Initialized, MAC={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.mac[0], self.mac[1], self.mac[2],
            self.mac[3], self.mac[4], self.mac[5]);

        Ok(())
    }

    /// Get MAC address
    pub fn mac(&self) -> &[u8; 6] {
        &self.mac
    }

    /// Get link state
    pub fn link_state(&self) -> LinkState {
        self.link_state
    }

    /// Get MTU
    pub fn mtu(&self) -> u16 {
        self.mtu
    }

    /// Transmit packet
    pub fn transmit(&mut self, data: &[u8]) -> Result<(), &'static str> {
        if !self.initialized.load(Ordering::Acquire) {
            return Err("Device not initialized");
        }

        if self.link_state != LinkState::Up {
            return Err("Link is down");
        }

        if self.tx_queue.is_full() {
            return Err("TX queue full");
        }

        if data.len() > self.mtu as usize + 14 {
            return Err("Packet too large");
        }

        let idx = self.tx_queue.producer_idx as usize;
        let desc = &mut self.tx_queue.desc_ring[idx];

        // Set up descriptor (in real implementation, need DMA buffer)
        desc.addr = data.as_ptr() as u64;
        desc.len = data.len() as u32;
        desc.gen = self.tx_queue.gen;
        desc.eop = 1;

        // Advance producer
        self.tx_queue.producer_idx = ((idx + 1) % self.tx_queue.desc_ring.len()) as u32;
        if self.tx_queue.producer_idx == 0 {
            self.tx_queue.gen = 1 - self.tx_queue.gen;
        }

        // Ring doorbell
        self.write_reg(regs::TXPROD, self.tx_queue.producer_idx);

        self.stats.tx_packets.fetch_add(1, Ordering::Relaxed);
        self.stats.tx_bytes.fetch_add(data.len() as u64, Ordering::Relaxed);

        Ok(())
    }

    /// Receive packet
    pub fn receive(&mut self) -> Option<Vec<u8>> {
        if !self.initialized.load(Ordering::Acquire) {
            return None;
        }

        let comp_idx = self.rx_queue.comp_consumer_idx as usize;
        let comp = &self.rx_queue.comp_ring[comp_idx];

        // Check if completion is valid
        if comp.gen != self.rx_queue.comp_gen {
            return None;
        }

        // Get data
        let desc_idx = comp.idx as usize;
        let len = comp.len as usize;

        if desc_idx < self.rx_queue.buffers.len() && len > 0 {
            let data = self.rx_queue.buffers[desc_idx][..len].to_vec();

            // Advance consumer
            self.rx_queue.comp_consumer_idx = ((comp_idx + 1) % self.rx_queue.comp_ring.len()) as u32;
            if self.rx_queue.comp_consumer_idx == 0 {
                self.rx_queue.comp_gen = 1 - self.rx_queue.comp_gen;
            }

            self.stats.rx_packets.fetch_add(1, Ordering::Relaxed);
            self.stats.rx_bytes.fetch_add(len as u64, Ordering::Relaxed);

            return Some(data);
        }

        None
    }

    /// Handle interrupt
    pub fn handle_interrupt(&mut self) {
        // Read interrupt cause
        let icr = self.read_reg(regs::ICR);

        // TX completion
        if icr & 0x1 != 0 {
            while let Some((_, _)) = self.process_tx_completion() {}
        }

        // RX
        if icr & 0x2 != 0 {
            // RX packets will be retrieved via receive()
        }

        // Link change
        if icr & 0x4 != 0 {
            self.send_cmd(Vmxnet3Cmd::GetLinkStatus);
            let link = self.read_reg(regs::CMD);
            self.link_state = if link != 0 { LinkState::Up } else { LinkState::Down };
        }
    }

    /// Process TX completion
    fn process_tx_completion(&mut self) -> Option<(u32, u8)> {
        let comp_idx = self.tx_queue.comp_consumer_idx as usize;
        let comp = &self.tx_queue.comp_ring[comp_idx];

        if comp.gen != self.tx_queue.comp_gen {
            return None;
        }

        let idx = comp.idx;
        let eop = comp.eop;

        self.tx_queue.comp_consumer_idx = ((comp_idx + 1) % self.tx_queue.comp_ring.len()) as u32;
        if self.tx_queue.comp_consumer_idx == 0 {
            self.tx_queue.comp_gen = 1 - self.tx_queue.comp_gen;
        }

        Some((idx, eop))
    }

    /// Get statistics
    pub fn stats(&self) -> &Vmxnet3Stats {
        &self.stats
    }

    /// Format status
    pub fn format_status(&self) -> String {
        alloc::format!(
            "VMXNET3: MAC={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} Link={:?}",
            self.mac[0], self.mac[1], self.mac[2],
            self.mac[3], self.mac[4], self.mac[5],
            self.link_state
        )
    }
}

/// Device manager
pub struct Vmxnet3Manager {
    devices: Vec<Vmxnet3Device>,
}

impl Vmxnet3Manager {
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
        }
    }

    pub fn add_device(&mut self, device: Vmxnet3Device) -> usize {
        let idx = self.devices.len();
        self.devices.push(device);
        idx
    }

    pub fn get_device(&mut self, idx: usize) -> Option<&mut Vmxnet3Device> {
        self.devices.get_mut(idx)
    }

    pub fn device_count(&self) -> usize {
        self.devices.len()
    }
}

impl Default for Vmxnet3Manager {
    fn default() -> Self {
        Self::new()
    }
}
