//! Realtek RTL8169/8168/8111 Gigabit Ethernet Driver
//!
//! The RTL8169 family includes several variants:
//! - RTL8169: Original Gigabit controller
//! - RTL8168: PCIe version
//! - RTL8111: Integrated PCIe version
//!
//! Features:
//! - 10/100/1000 Mbps Gigabit Ethernet
//! - PCI Express
//! - DMA ring buffers for TX and RX
//! - Hardware checksumming
//! - VLAN tagging
//!
//! Unlike RTL8139, this uses MMIO (not I/O ports) and ring descriptors.

#![allow(dead_code)]

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::ptr;
use core::sync::atomic::{AtomicBool, Ordering};
use spin::Mutex;

use crate::util::{KResult, KError};

/// Realtek Vendor ID
pub const RTL_VENDOR_ID: u16 = 0x10EC;

/// RTL8169 family Device IDs
pub mod device_ids {
    pub const RTL8169: u16 = 0x8169;
    pub const RTL8168_8111: u16 = 0x8168;
    pub const RTL8101E: u16 = 0x8136;
    pub const RTL8167: u16 = 0x8167;
}

/// MMIO Register Offsets
pub mod regs {
    pub const MAC0: u32 = 0x00;       // MAC address bytes 0-3
    pub const MAC4: u32 = 0x04;       // MAC address bytes 4-5
    pub const MAR0: u32 = 0x08;       // Multicast filter bytes 0-7
    pub const DTCCR: u32 = 0x10;      // Dump Tally Counter Command
    pub const TNPDS_LO: u32 = 0x20;   // Transmit Normal Priority Desc Start (low)
    pub const TNPDS_HI: u32 = 0x24;   // Transmit Normal Priority Desc Start (high)
    pub const THPDS_LO: u32 = 0x28;   // Transmit High Priority Desc Start (low)
    pub const THPDS_HI: u32 = 0x2C;   // Transmit High Priority Desc Start (high)
    pub const FLASH: u32 = 0x30;      // Flash memory read/write
    pub const ERBCR: u32 = 0x34;      // Early Receive Byte Count
    pub const ERSR: u32 = 0x36;       // Early Receive Status
    pub const CR: u32 = 0x37;         // Command Register
    pub const TPPOLL: u32 = 0x38;     // Transmit Priority Polling
    pub const IMR: u32 = 0x3C;        // Interrupt Mask Register
    pub const ISR: u32 = 0x3E;        // Interrupt Status Register
    pub const TCR: u32 = 0x40;        // Transmit Configuration
    pub const RCR: u32 = 0x44;        // Receive Configuration
    pub const TCTR: u32 = 0x48;       // Timer Count
    pub const MPC: u32 = 0x4C;        // Missed Packet Counter
    pub const EECMD: u32 = 0x50;      // EEPROM Command
    pub const CONFIG0: u32 = 0x51;    // Configuration Register 0
    pub const CONFIG1: u32 = 0x52;    // Configuration Register 1
    pub const CONFIG2: u32 = 0x53;    // Configuration Register 2
    pub const CONFIG3: u32 = 0x54;    // Configuration Register 3
    pub const CONFIG4: u32 = 0x55;    // Configuration Register 4
    pub const CONFIG5: u32 = 0x56;    // Configuration Register 5
    pub const TIMERINT: u32 = 0x58;   // Timer Interrupt
    pub const PHYAR: u32 = 0x60;      // PHY Access Register
    pub const TBICSR0: u32 = 0x64;    // TBI Control and Status
    pub const TBI_ANAR: u32 = 0x68;   // TBI Auto-Negotiation Adv
    pub const TBI_LPAR: u32 = 0x6A;   // TBI Auto-Negotiation Link Partner
    pub const PHYSTATUS: u32 = 0x6C;  // PHY Status
    pub const WAKEUP0: u32 = 0x84;    // Wake-up frame 0
    pub const WAKEUP1: u32 = 0x8C;    // Wake-up frame 1
    pub const WAKEUP2: u32 = 0xC4;    // Wake-up frame 2
    pub const WAKEUP3: u32 = 0xCC;    // Wake-up frame 3
    pub const WAKEUP4: u32 = 0xD4;    // Wake-up frame 4
    pub const CRC0: u32 = 0xDC;       // CRC of wake-up frame 0-4
    pub const RMS: u32 = 0xDA;        // Receive Max Size
    pub const CCR: u32 = 0xE0;        // C+ Command
    pub const RDSAR_LO: u32 = 0xE4;   // Receive Desc Start Address (low)
    pub const RDSAR_HI: u32 = 0xE8;   // Receive Desc Start Address (high)
    pub const MTPS: u32 = 0xEC;       // Max Transmit Packet Size
}

/// Command Register bits
pub mod cmd {
    pub const RESET: u8 = 1 << 4;
    pub const RX_ENABLE: u8 = 1 << 3;
    pub const TX_ENABLE: u8 = 1 << 2;
}

/// Transmit Configuration bits
pub mod tcr {
    pub const MXDMA_MASK: u32 = 0x700;
    pub const MXDMA_UNLIMITED: u32 = 0x700;
    pub const IFG_MASK: u32 = 0x03000000;
    pub const IFG_NORMAL: u32 = 0x03000000;
}

/// Receive Configuration bits
pub mod rcr {
    pub const AAP: u32 = 1 << 0;      // Accept All Packets
    pub const APM: u32 = 1 << 1;      // Accept Physical Match
    pub const AM: u32 = 1 << 2;       // Accept Multicast
    pub const AB: u32 = 1 << 3;       // Accept Broadcast
    pub const AR: u32 = 1 << 4;       // Accept Runt
    pub const AER: u32 = 1 << 5;      // Accept Error
    pub const RXFTH_MASK: u32 = 0xE000;
    pub const RXFTH_NONE: u32 = 0xE000;
    pub const MXDMA_MASK: u32 = 0x700;
    pub const MXDMA_UNLIMITED: u32 = 0x700;
}

/// C+ Command Register bits
pub mod ccr {
    pub const RXVLAN: u16 = 1 << 6;
    pub const RXCHKSUM: u16 = 1 << 5;
    pub const PCIDAC: u16 = 1 << 4;
    pub const PCIMULRW: u16 = 1 << 3;
    pub const CPCMD_RXEN: u16 = 1 << 1;
    pub const CPCMD_TXEN: u16 = 1 << 0;
}

/// Interrupt bits
pub mod intr {
    pub const ROK: u16 = 1 << 0;      // Receive OK
    pub const RER: u16 = 1 << 1;      // Receive Error
    pub const TOK: u16 = 1 << 2;      // Transmit OK
    pub const TER: u16 = 1 << 3;      // Transmit Error
    pub const RDU: u16 = 1 << 4;      // Rx Descriptor Unavailable
    pub const LINK_CHG: u16 = 1 << 5; // Link Change
    pub const FOVW: u16 = 1 << 6;     // Rx FIFO Overflow
    pub const TDU: u16 = 1 << 7;      // Tx Descriptor Unavailable
    pub const SW_INT: u16 = 1 << 8;   // Software Interrupt
    pub const TIMEOUT: u16 = 1 << 14; // Time Out
    pub const SERR: u16 = 1 << 15;    // System Error
}

/// PHY Status bits
pub mod phystatus {
    pub const LINK_STS: u8 = 1 << 1;   // Link Status
    pub const SPEED_10M: u8 = 1 << 2;
    pub const SPEED_100M: u8 = 1 << 3;
    pub const SPEED_1000M: u8 = 1 << 4;
    pub const FULL_DUP: u8 = 1 << 5;
}

/// TX Descriptor flags (first dword)
pub mod tx_desc {
    pub const OWN: u32 = 1 << 31;
    pub const EOR: u32 = 1 << 30;     // End of Ring
    pub const FS: u32 = 1 << 29;      // First Segment
    pub const LS: u32 = 1 << 28;      // Last Segment
    pub const LGSEN: u32 = 1 << 27;   // Large Send
    pub const IPCS: u32 = 1 << 18;    // IP Checksum
    pub const UDPCS: u32 = 1 << 17;   // UDP Checksum
    pub const TCPCS: u32 = 1 << 16;   // TCP Checksum
}

/// RX Descriptor flags (first dword)
pub mod rx_desc {
    pub const OWN: u32 = 1 << 31;
    pub const EOR: u32 = 1 << 30;     // End of Ring
    pub const FS: u32 = 1 << 29;      // First Segment
    pub const LS: u32 = 1 << 28;      // Last Segment
    pub const MAR: u32 = 1 << 26;     // Multicast
    pub const PAM: u32 = 1 << 25;     // Physical Address Match
    pub const BAR: u32 = 1 << 24;     // Broadcast
    pub const BOVF: u32 = 1 << 23;    // Buffer Overflow
    pub const RES: u32 = 1 << 21;     // Receive Error Summary
    pub const RWT: u32 = 1 << 22;     // Receive Watchdog Timer
    pub const FOVF: u32 = 1 << 20;    // FIFO Overflow
    pub const CRC: u32 = 1 << 19;     // CRC Error
    pub const RUNT: u32 = 1 << 18;    // Runt Packet
}

/// Ring descriptor (16 bytes for 64-bit)
#[repr(C, align(256))]
#[derive(Clone, Copy)]
pub struct Descriptor {
    pub opts1: u32,      // Flags and length
    pub opts2: u32,      // VLAN tag
    pub addr_lo: u32,    // Buffer address (low 32 bits)
    pub addr_hi: u32,    // Buffer address (high 32 bits)
}

impl Descriptor {
    pub const fn new() -> Self {
        Self {
            opts1: 0,
            opts2: 0,
            addr_lo: 0,
            addr_hi: 0,
        }
    }

    pub fn set_buffer(&mut self, addr: u64, len: u16) {
        self.addr_lo = addr as u32;
        self.addr_hi = (addr >> 32) as u32;
        self.opts1 = (self.opts1 & !0x3FFF) | (len as u32 & 0x3FFF);
    }

    pub fn length(&self) -> u16 {
        (self.opts1 & 0x3FFF) as u16
    }

    pub fn is_owned(&self) -> bool {
        (self.opts1 & rx_desc::OWN) != 0
    }
}

/// Number of TX descriptors
const TX_RING_SIZE: usize = 64;

/// Number of RX descriptors
const RX_RING_SIZE: usize = 64;

/// Buffer size
const BUFFER_SIZE: usize = 2048;

/// RTL8169 driver state
pub struct Rtl8169 {
    /// MMIO base address
    mmio_base: u64,
    /// MAC address
    mac: [u8; 6],
    /// TX descriptor ring
    tx_ring: Box<[Descriptor; TX_RING_SIZE]>,
    /// RX descriptor ring
    rx_ring: Box<[Descriptor; RX_RING_SIZE]>,
    /// TX buffers
    tx_buffers: Box<[[u8; BUFFER_SIZE]; TX_RING_SIZE]>,
    /// RX buffers
    rx_buffers: Box<[[u8; BUFFER_SIZE]; RX_RING_SIZE]>,
    /// Current TX index
    tx_index: usize,
    /// TX tail (last completed)
    tx_tail: usize,
    /// Current RX index
    rx_index: usize,
    /// Initialized flag
    initialized: bool,
    /// Chip version
    chip_version: u8,
}

impl Rtl8169 {
    /// Create a new RTL8169 driver instance
    pub fn new(mmio_base: u64) -> Self {
        Self {
            mmio_base,
            mac: [0; 6],
            tx_ring: Box::new([Descriptor::new(); TX_RING_SIZE]),
            rx_ring: Box::new([Descriptor::new(); RX_RING_SIZE]),
            tx_buffers: Box::new([[0u8; BUFFER_SIZE]; TX_RING_SIZE]),
            rx_buffers: Box::new([[0u8; BUFFER_SIZE]; RX_RING_SIZE]),
            tx_index: 0,
            tx_tail: 0,
            rx_index: 0,
            initialized: false,
            chip_version: 0,
        }
    }

    /// Read 8-bit MMIO register
    unsafe fn read8(&self, reg: u32) -> u8 {
        let addr = (self.mmio_base + reg as u64) as *const u8;
        ptr::read_volatile(addr)
    }

    /// Write 8-bit MMIO register
    unsafe fn write8(&self, reg: u32, value: u8) {
        let addr = (self.mmio_base + reg as u64) as *mut u8;
        ptr::write_volatile(addr, value);
    }

    /// Read 16-bit MMIO register
    unsafe fn read16(&self, reg: u32) -> u16 {
        let addr = (self.mmio_base + reg as u64) as *const u16;
        ptr::read_volatile(addr)
    }

    /// Write 16-bit MMIO register
    unsafe fn write16(&self, reg: u32, value: u16) {
        let addr = (self.mmio_base + reg as u64) as *mut u16;
        ptr::write_volatile(addr, value);
    }

    /// Read 32-bit MMIO register
    unsafe fn read32(&self, reg: u32) -> u32 {
        let addr = (self.mmio_base + reg as u64) as *const u32;
        ptr::read_volatile(addr)
    }

    /// Write 32-bit MMIO register
    unsafe fn write32(&self, reg: u32, value: u32) {
        let addr = (self.mmio_base + reg as u64) as *mut u32;
        ptr::write_volatile(addr, value);
    }

    /// Initialize the NIC
    pub fn init(&mut self) -> KResult<()> {
        unsafe {
            // Software reset
            self.write8(regs::CR, cmd::RESET);

            // Wait for reset to complete
            for _ in 0..1000 {
                if self.read8(regs::CR) & cmd::RESET == 0 {
                    break;
                }
                core::hint::spin_loop();
            }

            // Read MAC address
            let mac_lo = self.read32(regs::MAC0);
            let mac_hi = self.read32(regs::MAC4);
            self.mac[0] = mac_lo as u8;
            self.mac[1] = (mac_lo >> 8) as u8;
            self.mac[2] = (mac_lo >> 16) as u8;
            self.mac[3] = (mac_lo >> 24) as u8;
            self.mac[4] = mac_hi as u8;
            self.mac[5] = (mac_hi >> 8) as u8;

            crate::kprintln!(
                "rtl8169: MAC address {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                self.mac[0], self.mac[1], self.mac[2],
                self.mac[3], self.mac[4], self.mac[5]
            );

            // Unlock configuration registers
            self.write8(regs::EECMD, 0xC0);

            // Enable C+ mode
            self.write16(
                regs::CCR,
                ccr::RXCHKSUM | ccr::PCIDAC | ccr::PCIMULRW,
            );

            // Set RX max size
            self.write16(regs::RMS, BUFFER_SIZE as u16);

            // Set TX max size
            self.write8(regs::MTPS, 0x3B); // ~16KB

            // Configure RX descriptors
            self.init_rx_ring();

            // Configure TX descriptors
            self.init_tx_ring();

            // Set descriptor base addresses
            let rx_phys = self.rx_ring.as_ptr() as u64;
            let tx_phys = self.tx_ring.as_ptr() as u64;

            self.write32(regs::RDSAR_LO, rx_phys as u32);
            self.write32(regs::RDSAR_HI, (rx_phys >> 32) as u32);
            self.write32(regs::TNPDS_LO, tx_phys as u32);
            self.write32(regs::TNPDS_HI, (tx_phys >> 32) as u32);

            // Configure RX
            self.write32(
                regs::RCR,
                rcr::AB | rcr::AM | rcr::APM | rcr::RXFTH_NONE | rcr::MXDMA_UNLIMITED,
            );

            // Configure TX
            self.write32(
                regs::TCR,
                tcr::IFG_NORMAL | tcr::MXDMA_UNLIMITED,
            );

            // Lock configuration registers
            self.write8(regs::EECMD, 0x00);

            // Clear pending interrupts
            self.write16(regs::ISR, 0xFFFF);

            // Enable interrupts
            self.write16(
                regs::IMR,
                intr::ROK | intr::TOK | intr::RER | intr::TER | intr::RDU | intr::LINK_CHG,
            );

            // Enable receiver and transmitter
            self.write8(regs::CR, cmd::RX_ENABLE | cmd::TX_ENABLE);

            self.initialized = true;
            crate::kprintln!("rtl8169: initialized");
        }

        Ok(())
    }

    /// Initialize RX descriptor ring
    fn init_rx_ring(&mut self) {
        for i in 0..RX_RING_SIZE {
            let buf_addr = self.rx_buffers[i].as_ptr() as u64;
            self.rx_ring[i].set_buffer(buf_addr, BUFFER_SIZE as u16);
            self.rx_ring[i].opts1 = rx_desc::OWN | (BUFFER_SIZE as u32 & 0x3FFF);

            // Mark end of ring
            if i == RX_RING_SIZE - 1 {
                self.rx_ring[i].opts1 |= rx_desc::EOR;
            }
        }
        self.rx_index = 0;
    }

    /// Initialize TX descriptor ring
    fn init_tx_ring(&mut self) {
        for i in 0..TX_RING_SIZE {
            let buf_addr = self.tx_buffers[i].as_ptr() as u64;
            self.tx_ring[i].set_buffer(buf_addr, 0);
            self.tx_ring[i].opts1 = 0;

            // Mark end of ring
            if i == TX_RING_SIZE - 1 {
                self.tx_ring[i].opts1 |= tx_desc::EOR;
            }
        }
        self.tx_index = 0;
        self.tx_tail = 0;
    }

    /// Get MAC address
    pub fn get_mac(&self) -> Option<[u8; 6]> {
        if self.initialized {
            Some(self.mac)
        } else {
            None
        }
    }

    /// Send a packet
    pub fn send(&mut self, data: &[u8]) -> KResult<()> {
        if !self.initialized {
            return Err(KError::NotSupported);
        }

        if data.len() > BUFFER_SIZE {
            return Err(KError::OutOfRange);
        }

        // Wait for descriptor to be available
        for _ in 0..10000 {
            if (self.tx_ring[self.tx_index].opts1 & tx_desc::OWN) == 0 {
                break;
            }
            core::hint::spin_loop();
        }

        if (self.tx_ring[self.tx_index].opts1 & tx_desc::OWN) != 0 {
            return Err(KError::Busy);
        }

        // Copy data to TX buffer
        self.tx_buffers[self.tx_index][..data.len()].copy_from_slice(data);

        // Setup descriptor
        let eor = if self.tx_index == TX_RING_SIZE - 1 {
            tx_desc::EOR
        } else {
            0
        };
        self.tx_ring[self.tx_index].opts1 =
            tx_desc::OWN | tx_desc::FS | tx_desc::LS | eor | (data.len() as u32 & 0x3FFF);

        // Trigger transmission
        unsafe {
            self.write8(regs::TPPOLL, 0x40); // Normal priority queue
        }

        // Move to next descriptor
        self.tx_index = (self.tx_index + 1) % TX_RING_SIZE;

        Ok(())
    }

    /// Receive a packet
    pub fn recv(&mut self) -> Option<Vec<u8>> {
        if !self.initialized {
            return None;
        }

        // Check if there's a packet available
        if (self.rx_ring[self.rx_index].opts1 & rx_desc::OWN) != 0 {
            return None;
        }

        let opts1 = self.rx_ring[self.rx_index].opts1;

        // Check for errors
        if (opts1 & rx_desc::RES) != 0 {
            // Error, reset descriptor and continue
            self.reset_rx_descriptor(self.rx_index);
            self.rx_index = (self.rx_index + 1) % RX_RING_SIZE;
            return None;
        }

        // Check for first and last segment
        if (opts1 & (rx_desc::FS | rx_desc::LS)) != (rx_desc::FS | rx_desc::LS) {
            // Fragmented packet, not supported yet
            self.reset_rx_descriptor(self.rx_index);
            self.rx_index = (self.rx_index + 1) % RX_RING_SIZE;
            return None;
        }

        // Get packet length (subtract 4 for CRC)
        let len = (opts1 & 0x3FFF) as usize;
        let packet_len = if len >= 4 { len - 4 } else { 0 };

        if packet_len == 0 || packet_len > BUFFER_SIZE {
            self.reset_rx_descriptor(self.rx_index);
            self.rx_index = (self.rx_index + 1) % RX_RING_SIZE;
            return None;
        }

        // Copy packet data
        let packet = self.rx_buffers[self.rx_index][..packet_len].to_vec();

        // Reset descriptor for reuse
        self.reset_rx_descriptor(self.rx_index);

        // Move to next descriptor
        self.rx_index = (self.rx_index + 1) % RX_RING_SIZE;

        Some(packet)
    }

    /// Reset RX descriptor for reuse
    fn reset_rx_descriptor(&mut self, index: usize) {
        let eor = if index == RX_RING_SIZE - 1 {
            rx_desc::EOR
        } else {
            0
        };
        self.rx_ring[index].opts1 = rx_desc::OWN | eor | (BUFFER_SIZE as u32 & 0x3FFF);
    }

    /// Handle interrupt
    pub fn handle_interrupt(&mut self) {
        unsafe {
            let status = self.read16(regs::ISR);

            // Acknowledge all interrupts
            self.write16(regs::ISR, status);

            if status & intr::ROK != 0 {
                // Packet received - will be handled by recv()
            }

            if status & intr::TOK != 0 {
                // Packet transmitted OK
            }

            if status & intr::RER != 0 {
                crate::kprintln!("rtl8169: receive error");
            }

            if status & intr::TER != 0 {
                crate::kprintln!("rtl8169: transmit error");
            }

            if status & intr::RDU != 0 {
                crate::kprintln!("rtl8169: RX descriptor unavailable");
            }

            if status & intr::LINK_CHG != 0 {
                let phy_status = self.read8(regs::PHYSTATUS);
                let link_up = (phy_status & phystatus::LINK_STS) != 0;
                crate::kprintln!("rtl8169: link {}", if link_up { "up" } else { "down" });
            }
        }
    }

    /// Check link status
    pub fn is_link_up(&self) -> bool {
        if !self.initialized {
            return false;
        }
        unsafe {
            let phy_status = self.read8(regs::PHYSTATUS);
            (phy_status & phystatus::LINK_STS) != 0
        }
    }

    /// Get link speed (10, 100, or 1000 Mbps)
    pub fn get_speed(&self) -> u32 {
        if !self.initialized {
            return 0;
        }
        unsafe {
            let phy_status = self.read8(regs::PHYSTATUS);
            if (phy_status & phystatus::SPEED_1000M) != 0 {
                1000
            } else if (phy_status & phystatus::SPEED_100M) != 0 {
                100
            } else if (phy_status & phystatus::SPEED_10M) != 0 {
                10
            } else {
                0
            }
        }
    }

    /// Check if full duplex
    pub fn is_full_duplex(&self) -> bool {
        if !self.initialized {
            return false;
        }
        unsafe {
            let phy_status = self.read8(regs::PHYSTATUS);
            (phy_status & phystatus::FULL_DUP) != 0
        }
    }
}

/// Global RTL8169 instance
static RTL8169: Mutex<Option<Rtl8169>> = Mutex::new(None);

/// Initialized flag
static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Initialize RTL8169 driver
pub fn init() {
    use crate::drivers::pci::{scan, read_bar, enable_bus_mastering};

    for dev in scan() {
        if dev.id.vendor_id == RTL_VENDOR_ID {
            let is_rtl8169 = matches!(
                dev.id.device_id,
                device_ids::RTL8169
                    | device_ids::RTL8168_8111
                    | device_ids::RTL8101E
                    | device_ids::RTL8167
            );

            if is_rtl8169 {
                crate::kprintln!(
                    "rtl8169: found device {:04X} at {:02X}:{:02X}.{:X}",
                    dev.id.device_id,
                    dev.addr.bus,
                    dev.addr.device,
                    dev.addr.function
                );

                // Get MMIO base from BAR2 (RTL8169 uses BAR2 for MMIO)
                let (bar2, is_io) = read_bar(&dev, 2);
                if is_io {
                    // Try BAR1
                    let (bar1, is_io) = read_bar(&dev, 1);
                    if is_io {
                        crate::kprintln!("rtl8169: could not find MMIO BAR");
                        continue;
                    }
                    let mmio_base = bar1 & !0xF;
                    crate::kprintln!("rtl8169: MMIO base {:#X}", mmio_base);

                    enable_bus_mastering(&dev);

                    let mut rtl = Rtl8169::new(mmio_base);
                    if rtl.init().is_ok() {
                        *RTL8169.lock() = Some(rtl);
                        INITIALIZED.store(true, Ordering::Release);
                        return;
                    }
                } else {
                    let mmio_base = bar2 & !0xF;
                    crate::kprintln!("rtl8169: MMIO base {:#X}", mmio_base);

                    enable_bus_mastering(&dev);

                    let mut rtl = Rtl8169::new(mmio_base);
                    if rtl.init().is_ok() {
                        *RTL8169.lock() = Some(rtl);
                        INITIALIZED.store(true, Ordering::Release);
                        return;
                    }
                }
            }
        }
    }
}

/// Get MAC address
pub fn get_mac() -> Option<[u8; 6]> {
    RTL8169.lock().as_ref().and_then(|r| r.get_mac())
}

/// Send packet
pub fn send(data: &[u8]) -> KResult<()> {
    match RTL8169.lock().as_mut() {
        Some(r) => r.send(data),
        None => Err(KError::NotSupported),
    }
}

/// Receive packet
pub fn recv() -> Option<Vec<u8>> {
    RTL8169.lock().as_mut().and_then(|r| r.recv())
}

/// Handle interrupt
pub fn handle_interrupt() {
    if let Some(r) = RTL8169.lock().as_mut() {
        r.handle_interrupt();
    }
}

/// Check if driver is initialized
pub fn is_initialized() -> bool {
    INITIALIZED.load(Ordering::Acquire)
}

/// Check link status
pub fn is_link_up() -> bool {
    RTL8169.lock().as_ref().map_or(false, |r| r.is_link_up())
}

/// Get link speed
pub fn get_speed() -> u32 {
    RTL8169.lock().as_ref().map_or(0, |r| r.get_speed())
}
