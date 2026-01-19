//! Realtek RTL8125 2.5 Gigabit Ethernet Driver
//!
//! The RTL8125 is Realtek's 2.5 Gigabit Ethernet controller family.
//! Common variants include:
//! - RTL8125A: Original 2.5GbE version
//! - RTL8125B: Improved version with better performance
//! - RTL8125BG: Gaming branded version
//!
//! Features:
//! - 10/100/1000/2500 Mbps Ethernet
//! - PCI Express
//! - DMA ring buffers for TX and RX
//! - Hardware checksumming (TCP/UDP/IP)
//! - Large Send Offload (LSO)
//! - VLAN tagging
//! - Wake-on-LAN
//! - Energy Efficient Ethernet (EEE)
//!
//! Register interface is similar to RTL8168/8111 but with extensions
//! for 2.5GbE support.

#![allow(dead_code)]

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use crate::sync::IrqSafeMutex;

use crate::util::{KResult, KError};
use crate::drivers::pci;

/// Realtek Vendor ID
pub const RTL_VENDOR_ID: u16 = 0x10EC;

/// RTL8125 Device IDs
pub mod device_ids {
    pub const RTL8125A: u16 = 0x8125;       // RTL8125 2.5GbE
    pub const RTL8125B: u16 = 0x8126;       // RTL8125B improved
    pub const RTL8125BG: u16 = 0x3000;      // RTL8125BG gaming

    pub fn is_supported(device_id: u16) -> bool {
        matches!(device_id, RTL8125A | RTL8125B | RTL8125BG)
    }

    pub fn name(device_id: u16) -> &'static str {
        match device_id {
            RTL8125A => "RTL8125A",
            RTL8125B => "RTL8125B",
            RTL8125BG => "RTL8125BG",
            _ => "RTL8125",
        }
    }
}

/// MMIO Register Offsets (RTL8125 specific)
mod regs {
    // Basic registers (compatible with RTL8168)
    pub const MAC0: u32 = 0x00;             // MAC address bytes 0-3
    pub const MAC4: u32 = 0x04;             // MAC address bytes 4-5
    pub const MAR0: u32 = 0x08;             // Multicast filter bytes 0-7
    pub const DTCCR: u32 = 0x10;            // Dump Tally Counter Command

    // Descriptor addresses
    pub const TNPDS_LO: u32 = 0x20;         // TX Normal Priority Desc Start (low)
    pub const TNPDS_HI: u32 = 0x24;         // TX Normal Priority Desc Start (high)
    pub const THPDS_LO: u32 = 0x28;         // TX High Priority Desc Start (low)
    pub const THPDS_HI: u32 = 0x2C;         // TX High Priority Desc Start (high)

    // Control registers
    pub const CR: u32 = 0x37;               // Command Register
    pub const TPPOLL: u32 = 0x38;           // Transmit Priority Polling
    pub const IMR: u32 = 0x3C;              // Interrupt Mask Register (16-bit)
    pub const ISR: u32 = 0x3E;              // Interrupt Status Register (16-bit)

    // Configuration
    pub const TCR: u32 = 0x40;              // Transmit Configuration
    pub const RCR: u32 = 0x44;              // Receive Configuration
    pub const TCTR: u32 = 0x48;             // Timer Count
    pub const MPC: u32 = 0x4C;              // Missed Packet Counter

    // EEPROM and config
    pub const EECMD: u32 = 0x50;            // EEPROM Command
    pub const CONFIG0: u32 = 0x51;          // Configuration Register 0
    pub const CONFIG1: u32 = 0x52;          // Configuration Register 1
    pub const CONFIG2: u32 = 0x53;          // Configuration Register 2
    pub const CONFIG3: u32 = 0x54;          // Configuration Register 3
    pub const CONFIG4: u32 = 0x55;          // Configuration Register 4
    pub const CONFIG5: u32 = 0x56;          // Configuration Register 5
    pub const TIMERINT: u32 = 0x58;         // Timer Interrupt

    // PHY access
    pub const PHYAR: u32 = 0x60;            // PHY Access Register
    pub const PHYSTATUS: u32 = 0x6C;        // PHY Status

    // RX max size
    pub const RMS: u32 = 0xDA;              // Receive Max Size

    // C+ Command
    pub const CCR: u32 = 0xE0;              // C+ Command Register (16-bit)
    pub const RDSAR_LO: u32 = 0xE4;         // RX Desc Start Address (low)
    pub const RDSAR_HI: u32 = 0xE8;         // RX Desc Start Address (high)
    pub const MTPS: u32 = 0xEC;             // Max Transmit Packet Size

    // RTL8125-specific registers
    pub const MISC: u32 = 0xF0;             // Miscellaneous register
    pub const MISC2: u32 = 0xF2;            // Miscellaneous register 2
    pub const INT_CFG0_8125: u32 = 0x34;    // Interrupt config (8125)
    pub const INT_CFG1_8125: u32 = 0x7A;    // Interrupt config 1 (8125)
    pub const IMR0_8125: u32 = 0x38;        // Interrupt mask (8125)
    pub const ISR0_8125: u32 = 0x3C;        // Interrupt status (8125)

    // Extended registers (0x100+)
    pub const TX_DESC_ADDR_LO_0: u32 = 0x2100;   // TX desc base queue 0 low
    pub const TX_DESC_ADDR_HI_0: u32 = 0x2104;   // TX desc base queue 0 high
    pub const SW_TAIL_PTR_0: u32 = 0x2108;       // TX tail pointer queue 0
    pub const TX_DESC_ADDR_LO_1: u32 = 0x2140;   // TX desc base queue 1
    pub const TX_DESC_ADDR_HI_1: u32 = 0x2144;   // TX desc base queue 1 high
    pub const SW_TAIL_PTR_1: u32 = 0x2148;       // TX tail pointer queue 1

    pub const RX_DESC_ADDR_LO_0: u32 = 0x2300;   // RX desc base queue 0 low
    pub const RX_DESC_ADDR_HI_0: u32 = 0x2304;   // RX desc base queue 0 high
    pub const RX_TAIL_PTR_0: u32 = 0x2308;       // RX tail pointer queue 0
    pub const RX_DESC_ADDR_LO_1: u32 = 0x2340;   // RX desc base queue 1
    pub const RX_DESC_ADDR_HI_1: u32 = 0x2344;   // RX desc base queue 1 high
    pub const RX_TAIL_PTR_1: u32 = 0x2348;       // RX tail pointer queue 1

    // RSS (Receive Side Scaling)
    pub const RSS_CTRL_8125: u32 = 0x4500;
    pub const RSS_KEY_8125: u32 = 0x4510;
    pub const RSS_INDIRECTION_TBL: u32 = 0x4600;

    // EEE (Energy Efficient Ethernet)
    pub const EEE_LED: u32 = 0x1B0;
    pub const EEEAR: u32 = 0x1B4;
    pub const EEECR: u32 = 0x1B8;
    pub const EEE_TXIDLE_TIMER: u32 = 0x1BC;

    // PTP (IEEE 1588)
    pub const PTP_CTRL: u32 = 0x2700;
    pub const PTP_TX_STATUS: u32 = 0x2704;
    pub const PTP_TX_TIMESTAMP_LO: u32 = 0x2708;
    pub const PTP_TX_TIMESTAMP_HI: u32 = 0x270C;
}

/// Command Register bits
mod cmd {
    pub const RESET: u8 = 1 << 4;
    pub const RX_ENABLE: u8 = 1 << 3;
    pub const TX_ENABLE: u8 = 1 << 2;
}

/// Transmit Configuration bits
mod tcr {
    pub const MXDMA_MASK: u32 = 0x700;
    pub const MXDMA_UNLIMITED: u32 = 0x700;
    pub const IFG_MASK: u32 = 0x03000000;
    pub const IFG_NORMAL: u32 = 0x03000000;
}

/// Receive Configuration bits
mod rcr {
    pub const AAP: u32 = 1 << 0;            // Accept All Packets (promiscuous)
    pub const APM: u32 = 1 << 1;            // Accept Physical Match (unicast)
    pub const AM: u32 = 1 << 2;             // Accept Multicast
    pub const AB: u32 = 1 << 3;             // Accept Broadcast
    pub const AR: u32 = 1 << 4;             // Accept Runt
    pub const AER: u32 = 1 << 5;            // Accept Error
    pub const RXFTH_MASK: u32 = 0xE000;
    pub const RXFTH_NONE: u32 = 0xE000;
    pub const MXDMA_MASK: u32 = 0x700;
    pub const MXDMA_UNLIMITED: u32 = 0x700;
    pub const RCR_VLAN: u32 = 1 << 22;      // VLAN de-tagging
}

/// C+ Command Register bits
mod ccr {
    pub const RXVLAN: u16 = 1 << 6;
    pub const RXCHKSUM: u16 = 1 << 5;
    pub const PCIDAC: u16 = 1 << 4;
    pub const PCIMULRW: u16 = 1 << 3;
    pub const CPCMD_RXEN: u16 = 1 << 1;
    pub const CPCMD_TXEN: u16 = 1 << 0;
}

/// Interrupt bits
mod intr {
    pub const ROK: u16 = 1 << 0;            // Receive OK
    pub const RER: u16 = 1 << 1;            // Receive Error
    pub const TOK: u16 = 1 << 2;            // Transmit OK
    pub const TER: u16 = 1 << 3;            // Transmit Error
    pub const RDU: u16 = 1 << 4;            // Rx Descriptor Unavailable
    pub const LINK_CHG: u16 = 1 << 5;       // Link Change
    pub const FOVW: u16 = 1 << 6;           // Rx FIFO Overflow
    pub const TDU: u16 = 1 << 7;            // Tx Descriptor Unavailable
    pub const SW_INT: u16 = 1 << 8;         // Software Interrupt
    pub const TIMEOUT: u16 = 1 << 14;       // Time Out
    pub const SERR: u16 = 1 << 15;          // System Error
}

/// PHY Status bits
mod phystatus {
    pub const LINK_STS: u8 = 1 << 1;
    pub const SPEED_10M: u8 = 1 << 2;
    pub const SPEED_100M: u8 = 1 << 3;
    pub const SPEED_1000M: u8 = 1 << 4;
    pub const SPEED_2500M: u8 = 1 << 5;     // 2.5GbE (RTL8125 specific)
    pub const FULL_DUP: u8 = 1 << 6;
}

/// TX Descriptor flags (first dword)
mod tx_desc {
    pub const OWN: u32 = 1 << 31;           // Owned by hardware
    pub const EOR: u32 = 1 << 30;           // End of Ring
    pub const FS: u32 = 1 << 29;            // First Segment
    pub const LS: u32 = 1 << 28;            // Last Segment
    pub const LGSEN: u32 = 1 << 27;         // Large Send Enable (TSO)
    pub const IPCS: u32 = 1 << 18;          // IP Checksum
    pub const UDPCS: u32 = 1 << 17;         // UDP Checksum
    pub const TCPCS: u32 = 1 << 16;         // TCP Checksum
    pub const LEN_MASK: u32 = 0xFFFF;       // Packet length
}

/// RX Descriptor flags (first dword)
mod rx_desc {
    pub const OWN: u32 = 1 << 31;           // Owned by hardware
    pub const EOR: u32 = 1 << 30;           // End of Ring
    pub const FS: u32 = 1 << 29;            // First Segment
    pub const LS: u32 = 1 << 28;            // Last Segment
    pub const MAR: u32 = 1 << 26;           // Multicast
    pub const PAM: u32 = 1 << 25;           // Physical Address Matched
    pub const BAR: u32 = 1 << 24;           // Broadcast
    pub const BOVF: u32 = 1 << 23;          // Buffer Overflow
    pub const FOVF: u32 = 1 << 22;          // FIFO Overflow
    pub const RWT: u32 = 1 << 21;           // Watchdog Timer Expired
    pub const RES: u32 = 1 << 20;           // Receive Error Summary
    pub const RUNT: u32 = 1 << 19;          // Runt Packet
    pub const CRC: u32 = 1 << 18;           // CRC Error
    pub const PID1: u32 = 1 << 17;          // Protocol ID bit 1
    pub const PID0: u32 = 1 << 16;          // Protocol ID bit 0
    pub const IPF: u32 = 1 << 15;           // IP Checksum Failed
    pub const UDPF: u32 = 1 << 14;          // UDP Checksum Failed
    pub const TCPF: u32 = 1 << 13;          // TCP Checksum Failed
    pub const LEN_MASK: u32 = 0x1FFF;       // Packet length (13 bits)
}

/// Number of descriptors
const NUM_RX_DESC: usize = 256;
const NUM_TX_DESC: usize = 256;

/// Buffer size (MTU + headers)
const BUFFER_SIZE: usize = 2048;

/// Maximum receive size
const RX_MAX_SIZE: u16 = 9216;              // Jumbo frame support

/// TX Descriptor structure (16 bytes)
#[repr(C, align(256))]
#[derive(Clone, Copy)]
struct TxDescriptor {
    flags_len: u32,         // Flags and length
    vlan_tag: u32,          // VLAN tag
    buf_addr_lo: u32,       // Buffer address low
    buf_addr_hi: u32,       // Buffer address high
}

impl Default for TxDescriptor {
    fn default() -> Self {
        Self {
            flags_len: 0,
            vlan_tag: 0,
            buf_addr_lo: 0,
            buf_addr_hi: 0,
        }
    }
}

/// RX Descriptor structure (16 bytes)
#[repr(C, align(256))]
#[derive(Clone, Copy)]
struct RxDescriptor {
    flags_len: u32,         // Flags and length
    vlan_tag: u32,          // VLAN tag
    buf_addr_lo: u32,       // Buffer address low
    buf_addr_hi: u32,       // Buffer address high
}

impl Default for RxDescriptor {
    fn default() -> Self {
        Self {
            flags_len: rx_desc::OWN | (BUFFER_SIZE as u32),
            vlan_tag: 0,
            buf_addr_lo: 0,
            buf_addr_hi: 0,
        }
    }
}

/// Link speed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkSpeed {
    Speed10Mbps,
    Speed100Mbps,
    Speed1000Mbps,
    Speed2500Mbps,
    Unknown,
}

/// Driver statistics
#[derive(Debug, Default)]
pub struct Rtl8125Stats {
    pub tx_packets: AtomicU64,
    pub rx_packets: AtomicU64,
    pub tx_bytes: AtomicU64,
    pub rx_bytes: AtomicU64,
    pub tx_errors: AtomicU64,
    pub rx_errors: AtomicU64,
    pub rx_dropped: AtomicU64,
    pub rx_crc_errors: AtomicU64,
    pub rx_length_errors: AtomicU64,
}

/// Statistics snapshot
#[derive(Debug, Clone)]
pub struct Rtl8125StatsSnapshot {
    pub tx_packets: u64,
    pub rx_packets: u64,
    pub tx_bytes: u64,
    pub rx_bytes: u64,
    pub tx_errors: u64,
    pub rx_errors: u64,
    pub rx_dropped: u64,
}

/// RTL8125 Driver state
pub struct Rtl8125Driver {
    /// MMIO base address
    mmio_base: usize,
    /// MAC address
    mac: [u8; 6],
    /// TX descriptors
    tx_desc: Box<[TxDescriptor; NUM_TX_DESC]>,
    /// RX descriptors
    rx_desc: Box<[RxDescriptor; NUM_RX_DESC]>,
    /// TX buffers
    tx_buffers: Vec<Box<[u8; BUFFER_SIZE]>>,
    /// RX buffers
    rx_buffers: Vec<Box<[u8; BUFFER_SIZE]>>,
    /// Current TX descriptor index
    tx_cur: usize,
    /// Current RX descriptor index
    rx_cur: usize,
    /// Device ID
    device_id: u16,
    /// Link status
    link_up: AtomicBool,
    /// Link speed
    link_speed: LinkSpeed,
    /// Statistics
    stats: Rtl8125Stats,
}

/// Global driver instance
static DRIVER: IrqSafeMutex<Option<Rtl8125Driver>> = IrqSafeMutex::new(None);

impl Rtl8125Driver {
    /// Write to MMIO register (8-bit)
    fn write8(&self, reg: u32, value: u8) {
        unsafe {
            ptr::write_volatile((self.mmio_base + reg as usize) as *mut u8, value);
        }
    }

    /// Read from MMIO register (8-bit)
    fn read8(&self, reg: u32) -> u8 {
        unsafe {
            ptr::read_volatile((self.mmio_base + reg as usize) as *const u8)
        }
    }

    /// Write to MMIO register (16-bit)
    fn write16(&self, reg: u32, value: u16) {
        unsafe {
            ptr::write_volatile((self.mmio_base + reg as usize) as *mut u16, value);
        }
    }

    /// Read from MMIO register (16-bit)
    fn read16(&self, reg: u32) -> u16 {
        unsafe {
            ptr::read_volatile((self.mmio_base + reg as usize) as *const u16)
        }
    }

    /// Write to MMIO register (32-bit)
    fn write32(&self, reg: u32, value: u32) {
        unsafe {
            ptr::write_volatile((self.mmio_base + reg as usize) as *mut u32, value);
        }
    }

    /// Read from MMIO register (32-bit)
    fn read32(&self, reg: u32) -> u32 {
        unsafe {
            ptr::read_volatile((self.mmio_base + reg as usize) as *const u32)
        }
    }

    /// Reset the device
    fn reset(&mut self) {
        // Disable interrupts
        self.write16(regs::IMR, 0);

        // Disable TX and RX
        self.write8(regs::CR, 0);

        // Issue reset
        self.write8(regs::CR, cmd::RESET);

        // Wait for reset to complete
        for _ in 0..1000 {
            if self.read8(regs::CR) & cmd::RESET == 0 {
                break;
            }
            for _ in 0..1000 {
                core::hint::spin_loop();
            }
        }

        // Wait a bit more
        for _ in 0..10000 {
            core::hint::spin_loop();
        }
    }

    /// Read MAC address
    fn read_mac(&mut self) {
        let mac_lo = self.read32(regs::MAC0);
        let mac_hi = self.read16(regs::MAC4);

        self.mac[0] = (mac_lo >> 0) as u8;
        self.mac[1] = (mac_lo >> 8) as u8;
        self.mac[2] = (mac_lo >> 16) as u8;
        self.mac[3] = (mac_lo >> 24) as u8;
        self.mac[4] = (mac_hi >> 0) as u8;
        self.mac[5] = (mac_hi >> 8) as u8;
    }

    /// Initialize RX
    fn init_rx(&mut self) {
        // Allocate RX buffers and set up descriptors
        for i in 0..NUM_RX_DESC {
            let buffer = Box::new([0u8; BUFFER_SIZE]);
            let buffer_addr = buffer.as_ptr() as u64;
            self.rx_buffers.push(buffer);

            let mut flags = rx_desc::OWN | (BUFFER_SIZE as u32);
            if i == NUM_RX_DESC - 1 {
                flags |= rx_desc::EOR;  // End of ring
            }

            self.rx_desc[i].flags_len = flags;
            self.rx_desc[i].buf_addr_lo = buffer_addr as u32;
            self.rx_desc[i].buf_addr_hi = (buffer_addr >> 32) as u32;
        }

        // Set RX descriptor base address
        let rx_desc_addr = self.rx_desc.as_ptr() as u64;
        self.write32(regs::RDSAR_LO, rx_desc_addr as u32);
        self.write32(regs::RDSAR_HI, (rx_desc_addr >> 32) as u32);

        // Set max receive size
        self.write16(regs::RMS, RX_MAX_SIZE);

        // Configure receive
        let rcr = rcr::APM         // Accept physical match
            | rcr::AB              // Accept broadcast
            | rcr::AM              // Accept multicast
            | rcr::MXDMA_UNLIMITED // Max DMA burst
            | rcr::RXFTH_NONE;     // No FIFO threshold
        self.write32(regs::RCR, rcr);
    }

    /// Initialize TX
    fn init_tx(&mut self) {
        // Allocate TX buffers (descriptors start owned by software)
        for i in 0..NUM_TX_DESC {
            let buffer = Box::new([0u8; BUFFER_SIZE]);
            let buffer_addr = buffer.as_ptr() as u64;
            self.tx_buffers.push(buffer);

            let mut flags = 0u32;
            if i == NUM_TX_DESC - 1 {
                flags |= tx_desc::EOR;  // End of ring
            }

            self.tx_desc[i].flags_len = flags;
            self.tx_desc[i].buf_addr_lo = buffer_addr as u32;
            self.tx_desc[i].buf_addr_hi = (buffer_addr >> 32) as u32;
        }

        // Set TX descriptor base address
        let tx_desc_addr = self.tx_desc.as_ptr() as u64;
        self.write32(regs::TNPDS_LO, tx_desc_addr as u32);
        self.write32(regs::TNPDS_HI, (tx_desc_addr >> 32) as u32);

        // Set max transmit packet size
        self.write8(regs::MTPS, 0x3B);  // 9KB

        // Configure transmit
        let tcr = tcr::MXDMA_UNLIMITED | tcr::IFG_NORMAL;
        self.write32(regs::TCR, tcr);
    }

    /// Set up link
    fn setup_link(&mut self) {
        // Wait for link
        for _ in 0..100 {
            let phystatus = self.read8(regs::PHYSTATUS);

            if phystatus & phystatus::LINK_STS != 0 {
                self.link_up.store(true, Ordering::SeqCst);

                // Determine speed
                self.link_speed = if phystatus & phystatus::SPEED_2500M != 0 {
                    LinkSpeed::Speed2500Mbps
                } else if phystatus & phystatus::SPEED_1000M != 0 {
                    LinkSpeed::Speed1000Mbps
                } else if phystatus & phystatus::SPEED_100M != 0 {
                    LinkSpeed::Speed100Mbps
                } else if phystatus & phystatus::SPEED_10M != 0 {
                    LinkSpeed::Speed10Mbps
                } else {
                    LinkSpeed::Unknown
                };

                crate::kprintln!("rtl8125: link up at {:?}", self.link_speed);
                return;
            }

            for _ in 0..100000 {
                core::hint::spin_loop();
            }
        }

        crate::kprintln!("rtl8125: link down");
    }

    /// Enable TX and RX
    fn enable(&self) {
        // Enable C+ mode with checksumming
        let ccr = ccr::RXCHKSUM | ccr::PCIMULRW;
        self.write16(regs::CCR, ccr);

        // Enable TX and RX
        self.write8(regs::CR, cmd::TX_ENABLE | cmd::RX_ENABLE);

        // Enable interrupts
        let imr = intr::ROK | intr::RER | intr::TOK | intr::TER | intr::LINK_CHG;
        self.write16(regs::IMR, imr);

        // Clear pending interrupts
        self.write16(regs::ISR, 0xFFFF);
    }

    /// Send a packet
    fn send(&mut self, data: &[u8]) -> KResult<()> {
        if data.len() > BUFFER_SIZE - 4 {
            return Err(KError::Invalid);
        }

        let idx = self.tx_cur;

        // Check if descriptor is available (not owned by hardware)
        let flags = unsafe {
            ptr::read_volatile(&self.tx_desc[idx].flags_len)
        };
        if flags & tx_desc::OWN != 0 {
            return Err(KError::WouldBlock);
        }

        // Copy data to buffer
        self.tx_buffers[idx][..data.len()].copy_from_slice(data);

        // Update buffer address
        let buffer_addr = self.tx_buffers[idx].as_ptr() as u64;
        self.tx_desc[idx].buf_addr_lo = buffer_addr as u32;
        self.tx_desc[idx].buf_addr_hi = (buffer_addr >> 32) as u32;

        // Set up descriptor flags
        let mut new_flags = tx_desc::OWN      // Hardware owns it
            | tx_desc::FS                     // First segment
            | tx_desc::LS                     // Last segment
            | (data.len() as u32);            // Length

        if idx == NUM_TX_DESC - 1 {
            new_flags |= tx_desc::EOR;        // End of ring
        }

        // Memory barrier
        core::sync::atomic::fence(Ordering::SeqCst);

        // Write flags (this triggers the send)
        unsafe {
            ptr::write_volatile(&mut self.tx_desc[idx].flags_len, new_flags);
        }

        // Memory barrier
        core::sync::atomic::fence(Ordering::SeqCst);

        // Trigger TX polling
        self.write8(regs::TPPOLL, 0x40);  // NPQ bit

        // Move to next descriptor
        self.tx_cur = (idx + 1) % NUM_TX_DESC;

        // Update stats
        self.stats.tx_packets.fetch_add(1, Ordering::Relaxed);
        self.stats.tx_bytes.fetch_add(data.len() as u64, Ordering::Relaxed);

        Ok(())
    }

    /// Receive a packet
    fn recv(&mut self) -> Option<Vec<u8>> {
        let idx = self.rx_cur;

        // Check if we have a packet
        let flags = unsafe {
            ptr::read_volatile(&self.rx_desc[idx].flags_len)
        };

        if flags & rx_desc::OWN != 0 {
            // Still owned by hardware
            return None;
        }

        // Check for errors
        if flags & rx_desc::RES != 0 {
            self.stats.rx_errors.fetch_add(1, Ordering::Relaxed);
            self.reset_rx_desc(idx);
            self.rx_cur = (idx + 1) % NUM_RX_DESC;
            return None;
        }

        // Get packet length
        let length = (flags & rx_desc::LEN_MASK) as usize;

        if length == 0 || length > BUFFER_SIZE {
            self.stats.rx_length_errors.fetch_add(1, Ordering::Relaxed);
            self.reset_rx_desc(idx);
            self.rx_cur = (idx + 1) % NUM_RX_DESC;
            return None;
        }

        // Copy data
        let data = self.rx_buffers[idx][..length].to_vec();

        // Update stats
        self.stats.rx_packets.fetch_add(1, Ordering::Relaxed);
        self.stats.rx_bytes.fetch_add(length as u64, Ordering::Relaxed);

        // Reset descriptor
        self.reset_rx_desc(idx);

        // Move to next descriptor
        self.rx_cur = (idx + 1) % NUM_RX_DESC;

        Some(data)
    }

    /// Reset an RX descriptor
    fn reset_rx_desc(&mut self, idx: usize) {
        let buffer_addr = self.rx_buffers[idx].as_ptr() as u64;

        let mut flags = rx_desc::OWN | (BUFFER_SIZE as u32);
        if idx == NUM_RX_DESC - 1 {
            flags |= rx_desc::EOR;
        }

        self.rx_desc[idx].flags_len = flags;
        self.rx_desc[idx].buf_addr_lo = buffer_addr as u32;
        self.rx_desc[idx].buf_addr_hi = (buffer_addr >> 32) as u32;
    }

    /// Check link status
    fn check_link(&mut self) {
        let phystatus = self.read8(regs::PHYSTATUS);
        let link = phystatus & phystatus::LINK_STS != 0;

        if link != self.link_up.load(Ordering::Relaxed) {
            self.link_up.store(link, Ordering::SeqCst);

            if link {
                self.link_speed = if phystatus & phystatus::SPEED_2500M != 0 {
                    LinkSpeed::Speed2500Mbps
                } else if phystatus & phystatus::SPEED_1000M != 0 {
                    LinkSpeed::Speed1000Mbps
                } else if phystatus & phystatus::SPEED_100M != 0 {
                    LinkSpeed::Speed100Mbps
                } else if phystatus & phystatus::SPEED_10M != 0 {
                    LinkSpeed::Speed10Mbps
                } else {
                    LinkSpeed::Unknown
                };
                crate::kprintln!("rtl8125: link up at {:?}", self.link_speed);
            } else {
                self.link_speed = LinkSpeed::Unknown;
                crate::kprintln!("rtl8125: link down");
            }
        }
    }

    /// Get statistics
    pub fn get_stats(&self) -> Rtl8125StatsSnapshot {
        Rtl8125StatsSnapshot {
            tx_packets: self.stats.tx_packets.load(Ordering::Relaxed),
            rx_packets: self.stats.rx_packets.load(Ordering::Relaxed),
            tx_bytes: self.stats.tx_bytes.load(Ordering::Relaxed),
            rx_bytes: self.stats.rx_bytes.load(Ordering::Relaxed),
            tx_errors: self.stats.tx_errors.load(Ordering::Relaxed),
            rx_errors: self.stats.rx_errors.load(Ordering::Relaxed),
            rx_dropped: self.stats.rx_dropped.load(Ordering::Relaxed),
        }
    }
}

/// Initialize the RTL8125 driver
pub fn init() {
    let devices = pci::scan();

    for dev in devices {
        if dev.id.vendor_id != RTL_VENDOR_ID {
            continue;
        }

        if !device_ids::is_supported(dev.id.device_id) {
            continue;
        }

        crate::kprintln!("rtl8125: found {} at {:02x}:{:02x}.{}",
            device_ids::name(dev.id.device_id),
            dev.addr.bus, dev.addr.device, dev.addr.function);

        // Get MMIO base address
        let (bar0, is_io) = pci::read_bar(&dev, 0);
        if is_io {
            // RTL8125 also has I/O port mode, but we prefer MMIO
            let (bar2, is_io2) = pci::read_bar(&dev, 2);
            if is_io2 {
                crate::kprintln!("rtl8125: no MMIO BAR found");
                continue;
            }
            let mmio_base = (bar2 & 0xFFFFFFF0) as usize;
            if mmio_base == 0 {
                crate::kprintln!("rtl8125: invalid BAR2");
                continue;
            }
        }

        let mmio_base = (bar0 & 0xFFFFFFF0) as usize;
        if mmio_base == 0 {
            crate::kprintln!("rtl8125: invalid BAR0");
            continue;
        }

        // Enable bus mastering
        pci::enable_bus_mastering(&dev);

        // Create driver instance
        let mut driver = Rtl8125Driver {
            mmio_base,
            mac: [0; 6],
            tx_desc: Box::new([TxDescriptor::default(); NUM_TX_DESC]),
            rx_desc: Box::new([RxDescriptor::default(); NUM_RX_DESC]),
            tx_buffers: Vec::with_capacity(NUM_TX_DESC),
            rx_buffers: Vec::with_capacity(NUM_RX_DESC),
            tx_cur: 0,
            rx_cur: 0,
            device_id: dev.id.device_id,
            link_up: AtomicBool::new(false),
            link_speed: LinkSpeed::Unknown,
            stats: Rtl8125Stats::default(),
        };

        // Reset device
        driver.reset();

        // Read MAC address
        driver.read_mac();

        crate::kprintln!("rtl8125: MAC {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            driver.mac[0], driver.mac[1], driver.mac[2],
            driver.mac[3], driver.mac[4], driver.mac[5]);

        // Initialize RX and TX
        driver.init_rx();
        driver.init_tx();

        // Set up link
        driver.setup_link();

        // Enable the device
        driver.enable();

        // Store driver
        *DRIVER.lock() = Some(driver);

        crate::kprintln!("rtl8125: driver initialized");
        return;
    }
}

/// Get MAC address
pub fn get_mac() -> Option<[u8; 6]> {
    DRIVER.lock().as_ref().map(|d| d.mac)
}

/// Send a packet
pub fn send(data: &[u8]) -> KResult<()> {
    DRIVER.lock().as_mut()
        .ok_or(KError::NotSupported)?
        .send(data)
}

/// Receive a packet
pub fn recv() -> Option<Vec<u8>> {
    DRIVER.lock().as_mut()?.recv()
}

/// Check if link is up
pub fn is_link_up() -> bool {
    DRIVER.lock().as_ref()
        .map(|d| d.link_up.load(Ordering::Relaxed))
        .unwrap_or(false)
}

/// Get link speed
pub fn link_speed() -> LinkSpeed {
    DRIVER.lock().as_ref()
        .map(|d| d.link_speed)
        .unwrap_or(LinkSpeed::Unknown)
}

/// Get statistics
pub fn stats() -> Option<Rtl8125StatsSnapshot> {
    DRIVER.lock().as_ref().map(|d| d.get_stats())
}

/// Check link status (call periodically)
pub fn check_link() {
    if let Some(ref mut driver) = *DRIVER.lock() {
        driver.check_link();
    }
}

/// Get device name
pub fn device_name() -> &'static str {
    DRIVER.lock().as_ref()
        .map(|d| device_ids::name(d.device_id))
        .unwrap_or("none")
}

/// Check if driver is active
pub fn is_active() -> bool {
    DRIVER.lock().is_some()
}
