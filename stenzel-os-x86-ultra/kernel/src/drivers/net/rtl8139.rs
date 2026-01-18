//! Realtek RTL8139 Network Driver
//!
//! The RTL8139 is a popular 10/100 Mbps Ethernet controller.
//! It's commonly emulated in QEMU and other virtual machines.
//!
//! Features:
//! - 10/100 Mbps Fast Ethernet
//! - PCI Bus Master DMA
//! - Simple programming model
//!
//! Register access is through I/O ports (not MMIO).

#![allow(dead_code)]

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};
use spin::Mutex;
use x86_64::instructions::port::Port;

use crate::util::{KResult, KError};

/// RTL8139 Vendor ID
pub const RTL8139_VENDOR_ID: u16 = 0x10EC;

/// RTL8139 Device IDs
pub const RTL8139_DEVICE_ID: u16 = 0x8139;
pub const RTL8139_DEVICE_ID_ALT: u16 = 0x8138;

/// I/O Port Register Offsets
pub mod regs {
    pub const MAC0: u16 = 0x00;      // MAC address byte 0
    pub const MAC1: u16 = 0x01;      // MAC address byte 1
    pub const MAC2: u16 = 0x02;      // MAC address byte 2
    pub const MAC3: u16 = 0x03;      // MAC address byte 3
    pub const MAC4: u16 = 0x04;      // MAC address byte 4
    pub const MAC5: u16 = 0x05;      // MAC address byte 5
    pub const MAR0: u16 = 0x08;      // Multicast filter bytes 0-7
    pub const TSD0: u16 = 0x10;      // Transmit Status Descriptor 0
    pub const TSD1: u16 = 0x14;      // Transmit Status Descriptor 1
    pub const TSD2: u16 = 0x18;      // Transmit Status Descriptor 2
    pub const TSD3: u16 = 0x1C;      // Transmit Status Descriptor 3
    pub const TSAD0: u16 = 0x20;     // Transmit Start Address 0
    pub const TSAD1: u16 = 0x24;     // Transmit Start Address 1
    pub const TSAD2: u16 = 0x28;     // Transmit Start Address 2
    pub const TSAD3: u16 = 0x2C;     // Transmit Start Address 3
    pub const RBSTART: u16 = 0x30;   // Receive Buffer Start Address
    pub const ERBCR: u16 = 0x34;     // Early Receive Byte Count
    pub const ERSR: u16 = 0x36;      // Early Receive Status Register
    pub const CR: u16 = 0x37;        // Command Register
    pub const CAPR: u16 = 0x38;      // Current Address of Packet Read
    pub const CBR: u16 = 0x3A;       // Current Buffer Address
    pub const IMR: u16 = 0x3C;       // Interrupt Mask Register
    pub const ISR: u16 = 0x3E;       // Interrupt Status Register
    pub const TCR: u16 = 0x40;       // Transmit Configuration Register
    pub const RCR: u16 = 0x44;       // Receive Configuration Register
    pub const TCTR: u16 = 0x48;      // Timer Count Register
    pub const MPC: u16 = 0x4C;       // Missed Packet Counter
    pub const CR93C46: u16 = 0x50;   // 93C46 (EEPROM) Command Register
    pub const CONFIG0: u16 = 0x51;   // Configuration Register 0
    pub const CONFIG1: u16 = 0x52;   // Configuration Register 1
    pub const TIMERINT: u16 = 0x54;  // Timer Interrupt Register
    pub const MSR: u16 = 0x58;       // Media Status Register
    pub const CONFIG3: u16 = 0x59;   // Configuration Register 3
    pub const CONFIG4: u16 = 0x5A;   // Configuration Register 4
    pub const MULINT: u16 = 0x5C;    // Multiple Interrupt Select
    pub const RERID: u16 = 0x5E;     // PCI Revision ID
    pub const TSAD: u16 = 0x60;      // Transmit Status of All Descriptors
    pub const BMCR: u16 = 0x62;      // Basic Mode Control Register
    pub const BMSR: u16 = 0x64;      // Basic Mode Status Register
    pub const ANAR: u16 = 0x66;      // Auto-Negotiation Advertisement
    pub const ANLPAR: u16 = 0x68;    // Auto-Negotiation Link Partner
    pub const ANER: u16 = 0x6A;      // Auto-Negotiation Expansion
    pub const DIS: u16 = 0x6C;       // Disconnect Counter
    pub const FCSC: u16 = 0x6E;      // False Carrier Sense Counter
    pub const NWAYTR: u16 = 0x70;    // N-way Test Register
    pub const REC: u16 = 0x72;       // RX_ER Counter
    pub const CSCR: u16 = 0x74;      // CS Configuration Register
    pub const PHY1_PARM: u16 = 0x78; // PHY Parameter 1
    pub const TW_PARM: u16 = 0x7C;   // Twister Parameter
    pub const PHY2_PARM: u16 = 0x80; // PHY Parameter 2
}

/// Command Register bits
pub mod cmd {
    pub const RESET: u8 = 1 << 4;
    pub const RX_ENABLE: u8 = 1 << 3;
    pub const TX_ENABLE: u8 = 1 << 2;
    pub const BUFE: u8 = 1 << 0;     // Buffer Empty
}

/// Transmit Status bits
pub mod tsd {
    pub const OWN: u32 = 1 << 13;
    pub const TUN: u32 = 1 << 14;    // Transmit FIFO Underrun
    pub const TOK: u32 = 1 << 15;    // Transmit OK
    pub const OWC: u32 = 1 << 29;    // Out of Window Collision
    pub const TABT: u32 = 1 << 30;   // Transmit Abort
    pub const CRS: u32 = 1 << 31;    // Carrier Sense Lost
    pub const SIZE_SHIFT: u32 = 0;
    pub const SIZE_MASK: u32 = 0x1FFF;
    pub const EARLY_TX_THRESHOLD: u32 = 0x10 << 16; // Threshold = 256 bytes
}

/// Receive Configuration Register bits
pub mod rcr {
    pub const AAP: u32 = 1 << 0;     // Accept All Packets
    pub const APM: u32 = 1 << 1;     // Accept Physical Match
    pub const AM: u32 = 1 << 2;      // Accept Multicast
    pub const AB: u32 = 1 << 3;      // Accept Broadcast
    pub const AR: u32 = 1 << 4;      // Accept Runt
    pub const AER: u32 = 1 << 5;     // Accept Error Packet
    pub const WRAP: u32 = 1 << 7;    // Wrap bit for ring buffer
    pub const RBLEN_8K: u32 = 0 << 11;
    pub const RBLEN_16K: u32 = 1 << 11;
    pub const RBLEN_32K: u32 = 2 << 11;
    pub const RBLEN_64K: u32 = 3 << 11;
    pub const MXDMA_UNLIMITED: u32 = 7 << 8;
}

/// Transmit Configuration Register bits
pub mod tcr {
    pub const CLRABT: u32 = 1 << 0;
    pub const MXDMA_UNLIMITED: u32 = 7 << 8;
    pub const IFG_NORMAL: u32 = 3 << 24;
}

/// Interrupt Status/Mask bits
pub mod intr {
    pub const ROK: u16 = 1 << 0;     // Receive OK
    pub const RER: u16 = 1 << 1;     // Receive Error
    pub const TOK: u16 = 1 << 2;     // Transmit OK
    pub const TER: u16 = 1 << 3;     // Transmit Error
    pub const RXOVW: u16 = 1 << 4;   // Rx Buffer Overflow
    pub const LINK: u16 = 1 << 5;    // Link Change
    pub const FOVW: u16 = 1 << 6;    // Rx FIFO Overflow
    pub const TIMEOUT: u16 = 1 << 14;
    pub const SERR: u16 = 1 << 15;   // System Error
}

/// Receive packet header
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct RxPacketHeader {
    pub status: u16,
    pub length: u16,
}

impl RxPacketHeader {
    pub fn is_ok(&self) -> bool {
        (self.status & 0x01) != 0
    }

    pub fn has_error(&self) -> bool {
        (self.status & 0x1E) != 0
    }

    pub fn packet_length(&self) -> usize {
        self.length as usize
    }
}

/// Rx buffer size (8K + 16 bytes + 1500 bytes wrap space)
const RX_BUFFER_SIZE: usize = 8192 + 16 + 1500;

/// Tx buffer size (max MTU)
const TX_BUFFER_SIZE: usize = 1536;

/// Number of TX descriptors
const TX_DESCRIPTOR_COUNT: usize = 4;

/// RTL8139 driver state
pub struct Rtl8139 {
    /// I/O base port
    io_base: u16,
    /// MAC address
    mac: [u8; 6],
    /// Receive buffer (physically contiguous)
    rx_buffer: Box<[u8; RX_BUFFER_SIZE]>,
    /// Current position in RX buffer
    rx_offset: usize,
    /// Transmit buffers
    tx_buffers: [Box<[u8; TX_BUFFER_SIZE]>; TX_DESCRIPTOR_COUNT],
    /// Current TX descriptor index
    tx_index: usize,
    /// Initialized flag
    initialized: bool,
}

impl Rtl8139 {
    /// Create a new RTL8139 driver instance
    pub fn new(io_base: u16) -> Self {
        Self {
            io_base,
            mac: [0; 6],
            rx_buffer: Box::new([0u8; RX_BUFFER_SIZE]),
            rx_offset: 0,
            tx_buffers: [
                Box::new([0u8; TX_BUFFER_SIZE]),
                Box::new([0u8; TX_BUFFER_SIZE]),
                Box::new([0u8; TX_BUFFER_SIZE]),
                Box::new([0u8; TX_BUFFER_SIZE]),
            ],
            tx_index: 0,
            initialized: false,
        }
    }

    /// Read 8-bit register
    unsafe fn read8(&self, reg: u16) -> u8 {
        let mut port = Port::<u8>::new(self.io_base + reg);
        port.read()
    }

    /// Write 8-bit register
    unsafe fn write8(&self, reg: u16, value: u8) {
        let mut port = Port::<u8>::new(self.io_base + reg);
        port.write(value);
    }

    /// Read 16-bit register
    unsafe fn read16(&self, reg: u16) -> u16 {
        let mut port = Port::<u16>::new(self.io_base + reg);
        port.read()
    }

    /// Write 16-bit register
    unsafe fn write16(&self, reg: u16, value: u16) {
        let mut port = Port::<u16>::new(self.io_base + reg);
        port.write(value);
    }

    /// Read 32-bit register
    unsafe fn read32(&self, reg: u16) -> u32 {
        let mut port = Port::<u32>::new(self.io_base + reg);
        port.read()
    }

    /// Write 32-bit register
    unsafe fn write32(&self, reg: u16, value: u32) {
        let mut port = Port::<u32>::new(self.io_base + reg);
        port.write(value);
    }

    /// Initialize the NIC
    pub fn init(&mut self) -> KResult<()> {
        unsafe {
            // Power on
            self.write8(regs::CONFIG1, 0x00);

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
            for i in 0..6 {
                self.mac[i] = self.read8(regs::MAC0 + i as u16);
            }

            crate::kprintln!(
                "rtl8139: MAC address {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                self.mac[0], self.mac[1], self.mac[2],
                self.mac[3], self.mac[4], self.mac[5]
            );

            // Setup RX buffer
            let rx_phys = &*self.rx_buffer as *const [u8; RX_BUFFER_SIZE] as u64;
            // Note: In a real implementation, we'd need to convert this to a physical address
            // For now, assume identity mapping for the first 4GB
            self.write32(regs::RBSTART, rx_phys as u32);

            // Setup TX buffer addresses
            for i in 0..TX_DESCRIPTOR_COUNT {
                let tx_phys = &*self.tx_buffers[i] as *const [u8; TX_BUFFER_SIZE] as u64;
                let tsad_reg = match i {
                    0 => regs::TSAD0,
                    1 => regs::TSAD1,
                    2 => regs::TSAD2,
                    3 => regs::TSAD3,
                    _ => unreachable!(),
                };
                self.write32(tsad_reg, tx_phys as u32);
            }

            // Clear pending interrupts
            self.write16(regs::ISR, 0xFFFF);

            // Configure interrupts (enable RX and TX interrupts)
            self.write16(
                regs::IMR,
                intr::ROK | intr::TOK | intr::RER | intr::TER | intr::RXOVW,
            );

            // Configure RX:
            // - Accept broadcast, multicast, physical match
            // - 8KB buffer
            // - Wrap mode
            // - No threshold
            self.write32(
                regs::RCR,
                rcr::AB | rcr::AM | rcr::APM | rcr::WRAP | rcr::RBLEN_8K | rcr::MXDMA_UNLIMITED,
            );

            // Configure TX:
            // - Normal IFG
            // - Unlimited DMA burst
            self.write32(regs::TCR, tcr::IFG_NORMAL | tcr::MXDMA_UNLIMITED);

            // Enable receiver and transmitter
            self.write8(regs::CR, cmd::RX_ENABLE | cmd::TX_ENABLE);

            self.initialized = true;
            crate::kprintln!("rtl8139: initialized");
        }

        Ok(())
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

        if data.len() > TX_BUFFER_SIZE {
            return Err(KError::OutOfRange);
        }

        unsafe {
            // Get current descriptor
            let desc = self.tx_index;
            let tsd_reg = match desc {
                0 => regs::TSD0,
                1 => regs::TSD1,
                2 => regs::TSD2,
                3 => regs::TSD3,
                _ => unreachable!(),
            };

            // Wait for previous transmission to complete
            for _ in 0..10000 {
                let status = self.read32(tsd_reg);
                if status & tsd::OWN != 0 {
                    break;
                }
                if status & tsd::TOK != 0 {
                    break;
                }
                core::hint::spin_loop();
            }

            // Copy data to TX buffer
            self.tx_buffers[desc][..data.len()].copy_from_slice(data);

            // Set size and start transmission
            // Note: size must be at least 60 bytes (Ethernet minimum)
            let size = data.len().max(60) as u32;
            self.write32(tsd_reg, size | tsd::EARLY_TX_THRESHOLD);

            // Move to next descriptor
            self.tx_index = (self.tx_index + 1) % TX_DESCRIPTOR_COUNT;
        }

        Ok(())
    }

    /// Receive a packet
    pub fn recv(&mut self) -> Option<Vec<u8>> {
        if !self.initialized {
            return None;
        }

        unsafe {
            // Check if buffer is empty
            let cr = self.read8(regs::CR);
            if cr & cmd::BUFE != 0 {
                return None;
            }

            // Read packet header
            let header_ptr = &self.rx_buffer[self.rx_offset] as *const u8 as *const RxPacketHeader;
            let header = *header_ptr;

            // Check for valid packet
            if !header.is_ok() || header.has_error() {
                // Skip bad packet
                self.rx_offset = (self.rx_offset + 4) & (RX_BUFFER_SIZE - 1);
                return None;
            }

            let packet_len = header.packet_length();

            // Sanity check
            if packet_len < 4 || packet_len > 1518 {
                // Invalid length, skip
                self.rx_offset = (self.rx_offset + 4) & (RX_BUFFER_SIZE - 1);
                return None;
            }

            // Copy packet data (after header)
            let data_start = self.rx_offset + 4; // Skip header
            let data_len = packet_len - 4; // Remove CRC

            let mut packet = Vec::with_capacity(data_len);
            for i in 0..data_len {
                let idx = (data_start + i) % RX_BUFFER_SIZE;
                packet.push(self.rx_buffer[idx]);
            }

            // Update offset (align to dword boundary)
            self.rx_offset = (self.rx_offset + packet_len + 4 + 3) & !3;
            self.rx_offset %= RX_BUFFER_SIZE;

            // Update CAPR (Current Address of Packet Read)
            // CAPR is 16 bytes behind actual position
            let capr = ((self.rx_offset as u16).wrapping_sub(16)) & 0xFFFF;
            self.write16(regs::CAPR, capr);

            Some(packet)
        }
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
                crate::kprintln!("rtl8139: receive error");
            }

            if status & intr::TER != 0 {
                crate::kprintln!("rtl8139: transmit error");
            }

            if status & intr::RXOVW != 0 {
                crate::kprintln!("rtl8139: RX buffer overflow");
                // Reset RX
                self.write8(regs::CR, cmd::TX_ENABLE);
                self.rx_offset = 0;
                let rx_phys = &*self.rx_buffer as *const [u8; RX_BUFFER_SIZE] as u64;
                self.write32(regs::RBSTART, rx_phys as u32);
                self.write8(regs::CR, cmd::RX_ENABLE | cmd::TX_ENABLE);
            }
        }
    }

    /// Check link status
    pub fn is_link_up(&self) -> bool {
        if !self.initialized {
            return false;
        }
        unsafe {
            let msr = self.read8(regs::MSR);
            // Bit 2 = Link Status (0 = link up, 1 = link down)
            (msr & (1 << 2)) == 0
        }
    }

    /// Get link speed (10 or 100 Mbps)
    pub fn get_speed(&self) -> u32 {
        if !self.initialized {
            return 0;
        }
        unsafe {
            let msr = self.read8(regs::MSR);
            // Bit 3 = Speed (0 = 100Mbps, 1 = 10Mbps)
            if (msr & (1 << 3)) != 0 {
                10
            } else {
                100
            }
        }
    }
}

/// Global RTL8139 instance
static RTL8139: Mutex<Option<Rtl8139>> = Mutex::new(None);

/// Initialized flag
static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Initialize RTL8139 driver
pub fn init() {
    use crate::drivers::pci::{scan, read_bar};

    for dev in scan() {
        if dev.id.vendor_id == RTL8139_VENDOR_ID
            && (dev.id.device_id == RTL8139_DEVICE_ID || dev.id.device_id == RTL8139_DEVICE_ID_ALT)
        {
            crate::kprintln!(
                "rtl8139: found at {:02X}:{:02X}.{:X}",
                dev.addr.bus,
                dev.addr.device,
                dev.addr.function
            );

            // Get I/O base from BAR0
            let (bar0, is_io) = read_bar(&dev, 0);
            if !is_io {
                crate::kprintln!("rtl8139: expected I/O BAR, got MMIO");
                continue;
            }

            let io_base = bar0 as u16;
            crate::kprintln!("rtl8139: I/O base {:#X}", io_base);

            // Enable bus mastering
            crate::drivers::pci::enable_bus_mastering(&dev);

            let mut rtl = Rtl8139::new(io_base);
            if rtl.init().is_ok() {
                *RTL8139.lock() = Some(rtl);
                INITIALIZED.store(true, Ordering::Release);
                return;
            }
        }
    }
}

/// Get MAC address
pub fn get_mac() -> Option<[u8; 6]> {
    RTL8139.lock().as_ref().and_then(|r| r.get_mac())
}

/// Send packet
pub fn send(data: &[u8]) -> KResult<()> {
    match RTL8139.lock().as_mut() {
        Some(r) => r.send(data),
        None => Err(KError::NotSupported),
    }
}

/// Receive packet
pub fn recv() -> Option<Vec<u8>> {
    RTL8139.lock().as_mut().and_then(|r| r.recv())
}

/// Handle interrupt
pub fn handle_interrupt() {
    if let Some(r) = RTL8139.lock().as_mut() {
        r.handle_interrupt();
    }
}

/// Check if driver is initialized
pub fn is_initialized() -> bool {
    INITIALIZED.load(Ordering::Acquire)
}

/// Check link status
pub fn is_link_up() -> bool {
    RTL8139.lock().as_ref().map_or(false, |r| r.is_link_up())
}

/// Get link speed
pub fn get_speed() -> u32 {
    RTL8139.lock().as_ref().map_or(0, |r| r.get_speed())
}
