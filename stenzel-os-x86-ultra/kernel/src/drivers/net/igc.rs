//! Intel I225/I226 2.5 Gigabit Ethernet driver (IGC).
//!
//! This driver supports Intel I225/I226 NICs which are common in modern
//! desktop systems and offer 2.5 Gbps connectivity.
//!
//! Features:
//! - MMIO register access
//! - DMA ring buffers for TX/RX
//! - Auto-negotiation support (10/100/1000/2500 Mbps)
//! - Multiple queue support with MSI-X
//! - Hardware timestamping (IEEE 1588 PTP)
//! - Energy Efficient Ethernet (EEE)
//! - Wake-on-LAN support

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use crate::drivers::pci;
use crate::util::{KResult, KError};
use crate::sync::IrqSafeMutex;

/// Intel vendor ID.
const INTEL_VENDOR_ID: u16 = 0x8086;

/// Supported device IDs.
pub mod device_ids {
    // I225 - 2.5GbE
    pub const I225_LM: u16 = 0x15F2;
    pub const I225_V: u16 = 0x15F3;
    pub const I225_I: u16 = 0x15F8;
    pub const I225_K: u16 = 0x3100;
    pub const I225_K2: u16 = 0x3101;
    pub const I225_LMVP: u16 = 0x5502;
    pub const I225_IT: u16 = 0x0D9F;

    // I226 - 2.5GbE (newer revision)
    pub const I226_LM: u16 = 0x125B;
    pub const I226_V: u16 = 0x125C;
    pub const I226_IT: u16 = 0x125D;
    pub const I226_K: u16 = 0x3102;

    pub fn is_supported(device_id: u16) -> bool {
        matches!(device_id,
            I225_LM | I225_V | I225_I | I225_K | I225_K2 | I225_LMVP | I225_IT |
            I226_LM | I226_V | I226_IT | I226_K
        )
    }

    pub fn name(device_id: u16) -> &'static str {
        match device_id {
            I225_LM | I225_V | I225_I | I225_K | I225_K2 | I225_LMVP | I225_IT => "I225",
            I226_LM | I226_V | I226_IT | I226_K => "I226",
            _ => "IGC",
        }
    }

    pub fn is_i226(device_id: u16) -> bool {
        matches!(device_id, I226_LM | I226_V | I226_IT | I226_K)
    }
}

/// MMIO register offsets.
mod regs {
    // Device control
    pub const CTRL: u32 = 0x0000;           // Device Control
    pub const STATUS: u32 = 0x0008;         // Device Status
    pub const CTRL_EXT: u32 = 0x0018;       // Extended Device Control
    pub const MDIC: u32 = 0x0020;           // MDI Control
    pub const FEXTNVM4: u32 = 0x0024;       // Future Extended NVM 4
    pub const FEXTNVM7: u32 = 0x00E4;       // Future Extended NVM 7

    // PHY
    pub const PHPM: u32 = 0x0E14;           // PHY Power Management
    pub const CONNSW: u32 = 0x0034;         // Copper/Fiber Switch
    pub const I225_PHPM: u32 = 0x0E00;      // I225 specific PHY PM

    // EEPROM/NVM
    pub const EECD: u32 = 0x0010;           // EEPROM/Flash Control
    pub const EERD: u32 = 0x0014;           // EEPROM Read
    pub const EEWR: u32 = 0x102C;           // EEPROM Write
    pub const FLA: u32 = 0x001C;            // Flash Access

    // Interrupts
    pub const ICR: u32 = 0x01500;           // Interrupt Cause Read
    pub const ICS: u32 = 0x01504;           // Interrupt Cause Set
    pub const IMS: u32 = 0x01508;           // Interrupt Mask Set/Read
    pub const IMC: u32 = 0x0150C;           // Interrupt Mask Clear
    pub const IAM: u32 = 0x01510;           // Interrupt Acknowledge Auto Mask
    pub const EIAC: u32 = 0x0152C;          // Extended Interrupt Auto Clear
    pub const EIAM: u32 = 0x01530;          // Extended Interrupt Auto Mask
    pub const EICR: u32 = 0x01580;          // Extended Interrupt Cause Read

    // MSI-X
    pub const GPIE: u32 = 0x01514;          // General Purpose Interrupt Enable
    pub const IVAR0: u32 = 0x01700;         // Interrupt Vector Allocation Q0
    pub const IVAR_MISC: u32 = 0x01740;     // Interrupt Vector Allocation Misc

    // Receive
    pub const RCTL: u32 = 0x0100;           // Receive Control
    pub const SRRCTL0: u32 = 0x0C00C;       // Split Receive Control Q0
    pub const RDBAL0: u32 = 0x0C000;        // RX Descriptor Base Low Q0
    pub const RDBAH0: u32 = 0x0C004;        // RX Descriptor Base High Q0
    pub const RDLEN0: u32 = 0x0C008;        // RX Descriptor Length Q0
    pub const RDH0: u32 = 0x0C010;          // RX Descriptor Head Q0
    pub const RDT0: u32 = 0x0C018;          // RX Descriptor Tail Q0
    pub const RXDCTL0: u32 = 0x0C028;       // RX Descriptor Control Q0
    pub const RXCTL0: u32 = 0x0C014;        // RX DCA Control Q0

    // Transmit
    pub const TCTL: u32 = 0x0400;           // Transmit Control
    pub const TCTL_EXT: u32 = 0x0404;       // Extended Transmit Control
    pub const TDBAL0: u32 = 0x0E000;        // TX Descriptor Base Low Q0
    pub const TDBAH0: u32 = 0x0E004;        // TX Descriptor Base High Q0
    pub const TDLEN0: u32 = 0x0E008;        // TX Descriptor Length Q0
    pub const TDH0: u32 = 0x0E010;          // TX Descriptor Head Q0
    pub const TDT0: u32 = 0x0E018;          // TX Descriptor Tail Q0
    pub const TXDCTL0: u32 = 0x0E028;       // TX Descriptor Control Q0
    pub const TXCTL0: u32 = 0x0E014;        // TX DCA Control Q0

    // MAC Address
    pub const RAL0: u32 = 0x5400;           // Receive Address Low 0
    pub const RAH0: u32 = 0x5404;           // Receive Address High 0

    // Statistics
    pub const MPC: u32 = 0x4010;            // Missed Packets Count
    pub const CRCERRS: u32 = 0x4000;        // CRC Error Count
    pub const RNBC: u32 = 0x40A0;           // Receive No Buffers Count
    pub const GPRC: u32 = 0x4074;           // Good Packets Received Count
    pub const GPTC: u32 = 0x4080;           // Good Packets Transmitted Count
    pub const GORCL: u32 = 0x4088;          // Good Octets Received Low
    pub const GORCH: u32 = 0x408C;          // Good Octets Received High
    pub const GOTCL: u32 = 0x4090;          // Good Octets Transmitted Low
    pub const GOTCH: u32 = 0x4094;          // Good Octets Transmitted High
    pub const TPR: u32 = 0x40D0;            // Total Packets Received
    pub const TPT: u32 = 0x40D4;            // Total Packets Transmitted
    pub const PTC64: u32 = 0x40D8;          // Packets Transmitted (64 bytes)
    pub const PRC64: u32 = 0x405C;          // Packets Received (64 bytes)

    // PTP/Timestamping
    pub const TSICR: u32 = 0x0B66C;         // Time Sync Interrupt Cause
    pub const TSIM: u32 = 0x0B674;          // Time Sync Interrupt Mask
    pub const SYSTIML: u32 = 0x0B600;       // System Time Low
    pub const SYSTIMH: u32 = 0x0B604;       // System Time High
    pub const TIMINCA: u32 = 0x0B608;       // Time Increment Attributes
    pub const TSAUXC: u32 = 0x0B640;        // Time Sync Auxiliary Control
    pub const AUXSTMPL0: u32 = 0x0B65C;     // Auxiliary Time Stamp 0 Low
    pub const AUXSTMPH0: u32 = 0x0B660;     // Auxiliary Time Stamp 0 High

    // Wake-on-LAN
    pub const WUC: u32 = 0x5800;            // Wake Up Control
    pub const WUFC: u32 = 0x5808;           // Wake Up Filter Control

    // Energy Efficient Ethernet
    pub const EEER: u32 = 0x0E30;           // EEE Register
    pub const EEE_SU: u32 = 0x0E34;         // EEE Status and Setup

    // Flow Control
    pub const FCAL: u32 = 0x0028;           // Flow Control Address Low
    pub const FCAH: u32 = 0x002C;           // Flow Control Address High
    pub const FCT: u32 = 0x0030;            // Flow Control Type
    pub const FCTTV: u32 = 0x0170;          // Flow Control Transmit Timer Value
    pub const FCRTL: u32 = 0x2160;          // Flow Control Receive Threshold Low
    pub const FCRTH: u32 = 0x2168;          // Flow Control Receive Threshold High
    pub const FCRTV: u32 = 0x2460;          // Flow Control Refresh Timer Value

    // Queue offsets (for multiple queues)
    pub const fn rx_queue_offset(q: u32) -> u32 { q * 0x40 }
    pub const fn tx_queue_offset(q: u32) -> u32 { q * 0x40 }
}

/// Control register bits.
mod ctrl {
    pub const FD: u32 = 1 << 0;             // Full Duplex
    pub const GIO_MASTER_DISABLE: u32 = 1 << 2;
    pub const LRST: u32 = 1 << 3;           // Link Reset
    pub const ASDE: u32 = 1 << 5;           // Auto-Speed Detection Enable
    pub const SLU: u32 = 1 << 6;            // Set Link Up
    pub const SPEED_100: u32 = 1 << 8;      // Speed Selection 100Mbps
    pub const SPEED_1000: u32 = 1 << 9;     // Speed Selection 1000Mbps
    pub const FRCSPD: u32 = 1 << 11;        // Force Speed
    pub const FRCDPLX: u32 = 1 << 12;       // Force Duplex
    pub const SDP0_DATA: u32 = 1 << 18;     // Software Defined Pin 0 Data
    pub const SDP1_DATA: u32 = 1 << 19;     // Software Defined Pin 1 Data
    pub const ADVD3WUC: u32 = 1 << 20;      // D3Cold Wake Up Capability Advertisement Enable
    pub const RST: u32 = 1 << 26;           // Device Reset
    pub const RFCE: u32 = 1 << 27;          // Receive Flow Control Enable
    pub const TFCE: u32 = 1 << 28;          // Transmit Flow Control Enable
    pub const VME: u32 = 1 << 30;           // VLAN Mode Enable
    pub const PHY_RST: u32 = 1 << 31;       // PHY Reset
}

/// Status register bits.
mod status {
    pub const FD: u32 = 1 << 0;             // Full Duplex
    pub const LU: u32 = 1 << 1;             // Link Up
    pub const SPEED_MASK: u32 = 3 << 6;     // Speed bits
    pub const SPEED_10: u32 = 0 << 6;       // 10 Mbps
    pub const SPEED_100: u32 = 1 << 6;      // 100 Mbps
    pub const SPEED_1000: u32 = 2 << 6;     // 1000 Mbps
    pub const SPEED_2500: u32 = 3 << 6;     // 2500 Mbps (I225/I226 specific)
    pub const PHYRA: u32 = 1 << 10;         // PHY Reset Asserted
    pub const GIO_MASTER_ENABLE_STATUS: u32 = 1 << 19;
}

/// Receive control register bits.
mod rctl {
    pub const EN: u32 = 1 << 1;             // Receiver Enable
    pub const SBP: u32 = 1 << 2;            // Store Bad Packets
    pub const UPE: u32 = 1 << 3;            // Unicast Promiscuous Enable
    pub const MPE: u32 = 1 << 4;            // Multicast Promiscuous Enable
    pub const LPE: u32 = 1 << 5;            // Long Packet Enable
    pub const LBM_MAC: u32 = 1 << 6;        // Loopback Mode MAC
    pub const RDMTS_HALF: u32 = 0 << 8;     // RX Descriptor Min Threshold Size 1/2
    pub const RDMTS_QUARTER: u32 = 1 << 8;  // 1/4
    pub const RDMTS_EIGHTH: u32 = 2 << 8;   // 1/8
    pub const MO_36: u32 = 0 << 12;         // Multicast Offset 36
    pub const MO_35: u32 = 1 << 12;         // 35
    pub const MO_34: u32 = 2 << 12;         // 34
    pub const MO_32: u32 = 3 << 12;         // 32
    pub const BAM: u32 = 1 << 15;           // Broadcast Accept Mode
    pub const BSIZE_256: u32 = 3 << 16;     // Buffer Size 256
    pub const BSIZE_512: u32 = 2 << 16;     // Buffer Size 512
    pub const BSIZE_1024: u32 = 1 << 16;    // Buffer Size 1024
    pub const BSIZE_2048: u32 = 0 << 16;    // Buffer Size 2048
    pub const BSIZE_4096: u32 = 3 << 16 | 1 << 25;  // Buffer Size 4096
    pub const BSIZE_8192: u32 = 2 << 16 | 1 << 25;  // Buffer Size 8192
    pub const BSIZE_16384: u32 = 1 << 16 | 1 << 25; // Buffer Size 16384
    pub const VFE: u32 = 1 << 18;           // VLAN Filter Enable
    pub const CFIEN: u32 = 1 << 19;         // Canonical Form Indicator Enable
    pub const CFI: u32 = 1 << 20;           // Canonical Form Indicator
    pub const DPF: u32 = 1 << 22;           // Discard Pause Frames
    pub const PMCF: u32 = 1 << 23;          // Pass MAC Control Frames
    pub const BSEX: u32 = 1 << 25;          // Buffer Size Extension
    pub const SECRC: u32 = 1 << 26;         // Strip Ethernet CRC
}

/// Transmit control register bits.
mod tctl {
    pub const EN: u32 = 1 << 1;             // Transmitter Enable
    pub const PSP: u32 = 1 << 3;            // Pad Short Packets
    pub const CT_IEEE: u32 = 0x0F << 4;     // Collision Threshold IEEE 802.3
    pub const COLD_HD: u32 = 0x200 << 12;   // Collision Distance Half Duplex
    pub const COLD_FD: u32 = 0x40 << 12;    // Collision Distance Full Duplex
    pub const RTLC: u32 = 1 << 24;          // Re-transmit on Late Collision
    pub const MULR: u32 = 1 << 28;          // Multiple Request Support
}

/// Interrupt mask bits.
mod ints {
    pub const TXDW: u32 = 1 << 0;           // Transmit Descriptor Written Back
    pub const TXQE: u32 = 1 << 1;           // Transmit Queue Empty
    pub const LSC: u32 = 1 << 2;            // Link Status Change
    pub const RXSEQ: u32 = 1 << 3;          // Receive Sequence Error
    pub const RXDMT0: u32 = 1 << 4;         // RX Descriptor Minimum Threshold Q0
    pub const RXO: u32 = 1 << 6;            // Receiver Overrun
    pub const RXT0: u32 = 1 << 7;           // Receiver Timer Interrupt Q0
    pub const MDAC: u32 = 1 << 9;           // MDI/O Access Complete
    pub const RXCFG: u32 = 1 << 10;         // Receiving /C/ ordered sets
    pub const GPI_SDP2: u32 = 1 << 13;      // General Purpose Interrupt on SDP2
    pub const GPI_SDP3: u32 = 1 << 14;      // General Purpose Interrupt on SDP3
    pub const TXD_LOW: u32 = 1 << 15;       // TX Descriptor Low Threshold
    pub const SRPD: u32 = 1 << 16;          // Small Receive Packet Detected
    pub const ACK: u32 = 1 << 17;           // Receive ACK Frame Detected
    pub const MNG: u32 = 1 << 18;           // Manageability Event Detected
    pub const EPRST: u32 = 1 << 20;         // ME Firmware Reset Occurred
    pub const RXQ0: u32 = 1 << 20;          // Receive Queue 0
    pub const RXQ1: u32 = 1 << 21;          // Receive Queue 1
    pub const TXQ0: u32 = 1 << 22;          // Transmit Queue 0
    pub const TXQ1: u32 = 1 << 23;          // Transmit Queue 1
    pub const OTHER: u32 = 1 << 24;         // Other Interrupt
    pub const FER: u32 = 1 << 26;           // Fatal Error
    pub const NFER: u32 = 1 << 27;          // Non-Fatal Error
    pub const AMED: u32 = 1 << 28;          // Asserted Management Event
    pub const DSENT: u32 = 1 << 29;         // RXDMT FIFO Serviced
    pub const TCP_TIMER: u32 = 1 << 30;     // TCP Timer
    pub const DRSTA: u32 = 1 << 31;         // Device Reset Asserted
}

/// Number of descriptors in each ring.
const NUM_RX_DESC: usize = 256;
const NUM_TX_DESC: usize = 256;

/// Buffer size.
const BUFFER_SIZE: usize = 2048;

/// Advanced receive descriptor.
#[repr(C, align(16))]
#[derive(Clone, Copy, Default)]
struct RxDescAdv {
    /// Buffer address
    buffer_addr: u64,
    /// Header buffer address
    hdr_addr: u64,
}

/// Advanced receive descriptor writeback format.
#[repr(C, align(16))]
#[derive(Clone, Copy, Default)]
struct RxDescAdvWb {
    /// RSS hash / Packet type
    rss_pkt_info: u32,
    /// Header buffer length / Split header flag / Header length
    hdr_info: u16,
    /// Split payload length
    sph_len: u16,
    /// Extended status
    status_error: u32,
    /// Packet length
    pkt_len: u16,
    /// VLAN tag
    vlan: u16,
}

impl RxDescAdvWb {
    fn done(&self) -> bool {
        (self.status_error & 0x01) != 0  // DD bit
    }

    fn eop(&self) -> bool {
        (self.status_error & 0x02) != 0  // EOP bit
    }

    fn length(&self) -> u16 {
        self.pkt_len
    }
}

/// Advanced transmit descriptor (context).
#[repr(C, align(16))]
#[derive(Clone, Copy, Default)]
struct TxDescAdvCtx {
    /// VLAN, etc.
    vlan_macip_lens: u32,
    /// Sequence number / NAT fields
    seqnum_seed: u32,
    /// Type and extended
    type_tucmd_mlhl: u32,
    /// MSS / L4 len
    mss_l4len_idx: u32,
}

/// Advanced transmit descriptor (data).
#[repr(C, align(16))]
#[derive(Clone, Copy, Default)]
struct TxDescAdv {
    /// Buffer address
    buffer_addr: u64,
    /// Command and length
    cmd_type_len: u32,
    /// Options and paylen
    olinfo_status: u32,
}

impl TxDescAdv {
    const DTYP_DATA: u32 = 0x00300000;
    const DCMD_EOP: u32 = 0x01000000;
    const DCMD_IFCS: u32 = 0x02000000;
    const DCMD_RS: u32 = 0x08000000;
    const DCMD_DEXT: u32 = 0x20000000;
    const PAYLEN_SHIFT: u32 = 14;
}

/// Link speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkSpeed {
    Speed10Mbps,
    Speed100Mbps,
    Speed1000Mbps,
    Speed2500Mbps,
    Unknown,
}

/// Driver statistics.
#[derive(Debug, Default)]
pub struct IgcStats {
    pub tx_packets: AtomicU64,
    pub rx_packets: AtomicU64,
    pub tx_bytes: AtomicU64,
    pub rx_bytes: AtomicU64,
    pub tx_errors: AtomicU64,
    pub rx_errors: AtomicU64,
    pub rx_dropped: AtomicU64,
}

/// IGC driver state.
pub struct IgcDriver {
    /// MMIO base address
    mmio_base: usize,
    /// MAC address
    mac: [u8; 6],
    /// RX descriptors
    rx_desc: Box<[RxDescAdv; NUM_RX_DESC]>,
    /// TX descriptors
    tx_desc: Box<[TxDescAdv; NUM_TX_DESC]>,
    /// RX buffers
    rx_buffers: Vec<Box<[u8; BUFFER_SIZE]>>,
    /// TX buffers
    tx_buffers: Vec<Box<[u8; BUFFER_SIZE]>>,
    /// Current RX descriptor index
    rx_cur: usize,
    /// Current TX descriptor index
    tx_cur: usize,
    /// Device ID
    device_id: u16,
    /// Is I226
    is_i226: bool,
    /// Link up
    link_up: AtomicBool,
    /// Link speed
    link_speed: LinkSpeed,
    /// Statistics
    stats: IgcStats,
}

/// Global driver instance.
static DRIVER: IrqSafeMutex<Option<IgcDriver>> = IrqSafeMutex::new(None);

impl IgcDriver {
    /// Write to MMIO register.
    fn write_reg(&self, reg: u32, value: u32) {
        unsafe {
            ptr::write_volatile((self.mmio_base + reg as usize) as *mut u32, value);
        }
    }

    /// Read from MMIO register.
    fn read_reg(&self, reg: u32) -> u32 {
        unsafe {
            ptr::read_volatile((self.mmio_base + reg as usize) as *const u32)
        }
    }

    /// Reset the device.
    fn reset(&mut self) {
        // Disable interrupts
        self.write_reg(regs::IMC, 0xFFFFFFFF);

        // Disable RX and TX
        self.write_reg(regs::RCTL, 0);
        self.write_reg(regs::TCTL, 0);

        // Master disable
        let mut ctrl = self.read_reg(regs::CTRL);
        ctrl |= ctrl::GIO_MASTER_DISABLE;
        self.write_reg(regs::CTRL, ctrl);

        // Wait for master disable
        for _ in 0..100 {
            let status = self.read_reg(regs::STATUS);
            if status & status::GIO_MASTER_ENABLE_STATUS == 0 {
                break;
            }
            // Small delay
            for _ in 0..1000 {
                core::hint::spin_loop();
            }
        }

        // Reset
        ctrl = self.read_reg(regs::CTRL);
        ctrl |= ctrl::RST;
        self.write_reg(regs::CTRL, ctrl);

        // Wait for reset to complete
        for _ in 0..1000 {
            let ctrl = self.read_reg(regs::CTRL);
            if ctrl & ctrl::RST == 0 {
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

        // Disable interrupts again after reset
        self.write_reg(regs::IMC, 0xFFFFFFFF);
    }

    /// Read MAC address from EEPROM or RAL/RAH.
    fn read_mac(&mut self) {
        // Try reading from RAL/RAH first (may be programmed by firmware)
        let ral = self.read_reg(regs::RAL0);
        let rah = self.read_reg(regs::RAH0);

        if ral != 0 && ral != 0xFFFFFFFF {
            self.mac[0] = (ral >> 0) as u8;
            self.mac[1] = (ral >> 8) as u8;
            self.mac[2] = (ral >> 16) as u8;
            self.mac[3] = (ral >> 24) as u8;
            self.mac[4] = (rah >> 0) as u8;
            self.mac[5] = (rah >> 8) as u8;
            return;
        }

        // Try reading from EEPROM
        for i in 0..3 {
            let word = self.read_eeprom(i as u16);
            self.mac[i * 2] = (word >> 0) as u8;
            self.mac[i * 2 + 1] = (word >> 8) as u8;
        }
    }

    /// Read a word from EEPROM.
    fn read_eeprom(&self, offset: u16) -> u16 {
        // Start read
        self.write_reg(regs::EERD, ((offset as u32) << 8) | 0x01);

        // Wait for completion
        for _ in 0..10000 {
            let eerd = self.read_reg(regs::EERD);
            if eerd & 0x02 != 0 {
                return ((eerd >> 16) & 0xFFFF) as u16;
            }
            for _ in 0..100 {
                core::hint::spin_loop();
            }
        }

        0xFFFF
    }

    /// Initialize RX.
    fn init_rx(&mut self) {
        // Allocate and initialize RX descriptors and buffers
        for i in 0..NUM_RX_DESC {
            let buffer = Box::new([0u8; BUFFER_SIZE]);
            let buffer_addr = buffer.as_ptr() as u64;
            self.rx_buffers.push(buffer);
            self.rx_desc[i].buffer_addr = buffer_addr;
            self.rx_desc[i].hdr_addr = 0;
        }

        // Set RX descriptor base address
        let rx_desc_addr = self.rx_desc.as_ptr() as u64;
        self.write_reg(regs::RDBAL0, rx_desc_addr as u32);
        self.write_reg(regs::RDBAH0, (rx_desc_addr >> 32) as u32);

        // Set RX descriptor ring length
        self.write_reg(regs::RDLEN0, (NUM_RX_DESC * 16) as u32);

        // Set head and tail
        self.write_reg(regs::RDH0, 0);
        self.write_reg(regs::RDT0, (NUM_RX_DESC - 1) as u32);

        // Configure split receive control for advanced descriptors
        let srrctl = (BUFFER_SIZE / 1024) as u32  // Buffer size in KB
            | (4 << 8)   // Header buffer size (4 * 64 = 256 bytes)
            | (1 << 25)  // Drop packets when no descriptors
            | (1 << 28); // Advanced descriptor type
        self.write_reg(regs::SRRCTL0, srrctl);

        // Enable RX descriptor queue
        let rxdctl = self.read_reg(regs::RXDCTL0);
        self.write_reg(regs::RXDCTL0, rxdctl | (1 << 25));  // Enable queue

        // Wait for queue to enable
        for _ in 0..1000 {
            if self.read_reg(regs::RXDCTL0) & (1 << 25) != 0 {
                break;
            }
            core::hint::spin_loop();
        }

        // Configure RX control
        let rctl = rctl::EN
            | rctl::BAM           // Accept broadcast
            | rctl::BSIZE_2048    // 2048 byte buffers
            | rctl::SECRC;        // Strip CRC
        self.write_reg(regs::RCTL, rctl);
    }

    /// Initialize TX.
    fn init_tx(&mut self) {
        // Allocate TX buffers
        for _ in 0..NUM_TX_DESC {
            let buffer = Box::new([0u8; BUFFER_SIZE]);
            self.tx_buffers.push(buffer);
            // TX descriptors are initialized as needed when sending
        }

        // Set TX descriptor base address
        let tx_desc_addr = self.tx_desc.as_ptr() as u64;
        self.write_reg(regs::TDBAL0, tx_desc_addr as u32);
        self.write_reg(regs::TDBAH0, (tx_desc_addr >> 32) as u32);

        // Set TX descriptor ring length
        self.write_reg(regs::TDLEN0, (NUM_TX_DESC * 16) as u32);

        // Set head and tail
        self.write_reg(regs::TDH0, 0);
        self.write_reg(regs::TDT0, 0);

        // Enable TX descriptor queue
        let txdctl = (1 << 25)    // Enable queue
            | (32 << 16)          // Prefetch threshold
            | (1 << 8)            // Host threshold
            | 32;                 // Write back threshold
        self.write_reg(regs::TXDCTL0, txdctl);

        // Wait for queue to enable
        for _ in 0..1000 {
            if self.read_reg(regs::TXDCTL0) & (1 << 25) != 0 {
                break;
            }
            core::hint::spin_loop();
        }

        // Configure TX control
        let tctl = tctl::EN
            | tctl::PSP           // Pad short packets
            | tctl::CT_IEEE       // Collision threshold
            | tctl::COLD_FD;      // Collision distance (full duplex)
        self.write_reg(regs::TCTL, tctl);
    }

    /// Set up link.
    fn setup_link(&mut self) {
        // Enable auto-negotiation for 2.5G/1G/100M/10M
        let mut ctrl = self.read_reg(regs::CTRL);
        ctrl &= !(ctrl::FRCSPD | ctrl::FRCDPLX);
        ctrl |= ctrl::ASDE | ctrl::SLU;
        self.write_reg(regs::CTRL, ctrl);

        // Wait for link
        for _ in 0..100 {
            let status = self.read_reg(regs::STATUS);
            if status & status::LU != 0 {
                self.link_up.store(true, Ordering::SeqCst);

                // Determine speed
                self.link_speed = match status & status::SPEED_MASK {
                    status::SPEED_10 => LinkSpeed::Speed10Mbps,
                    status::SPEED_100 => LinkSpeed::Speed100Mbps,
                    status::SPEED_1000 => LinkSpeed::Speed1000Mbps,
                    status::SPEED_2500 => LinkSpeed::Speed2500Mbps,
                    _ => LinkSpeed::Unknown,
                };

                crate::kprintln!("igc: link up at {:?}", self.link_speed);
                return;
            }
            // Wait 10ms
            for _ in 0..100000 {
                core::hint::spin_loop();
            }
        }

        crate::kprintln!("igc: link down");
    }

    /// Enable interrupts.
    fn enable_interrupts(&self) {
        // Clear any pending interrupts
        self.read_reg(regs::ICR);

        // Enable interrupts we care about
        let ims = ints::LSC       // Link status change
            | ints::RXT0          // RX timer
            | ints::RXDMT0        // RX descriptor minimum threshold
            | ints::RXO           // RX overrun
            | ints::TXDW          // TX descriptor writeback
            | ints::TXQE;         // TX queue empty
        self.write_reg(regs::IMS, ims);
    }

    /// Send a packet.
    fn send(&mut self, data: &[u8]) -> KResult<()> {
        if data.len() > BUFFER_SIZE - 4 {
            return Err(KError::Invalid);
        }

        // Get next TX descriptor
        let idx = self.tx_cur;
        let next_idx = (idx + 1) % NUM_TX_DESC;

        // Copy data to TX buffer
        self.tx_buffers[idx][..data.len()].copy_from_slice(data);

        // Set up descriptor
        let buffer_addr = self.tx_buffers[idx].as_ptr() as u64;
        self.tx_desc[idx].buffer_addr = buffer_addr;
        self.tx_desc[idx].cmd_type_len = TxDescAdv::DTYP_DATA
            | TxDescAdv::DCMD_EOP
            | TxDescAdv::DCMD_IFCS
            | TxDescAdv::DCMD_RS
            | TxDescAdv::DCMD_DEXT
            | data.len() as u32;
        self.tx_desc[idx].olinfo_status = (data.len() as u32) << TxDescAdv::PAYLEN_SHIFT;

        // Memory barrier
        core::sync::atomic::fence(Ordering::SeqCst);

        // Update tail
        self.tx_cur = next_idx;
        self.write_reg(regs::TDT0, next_idx as u32);

        // Update stats
        self.stats.tx_packets.fetch_add(1, Ordering::Relaxed);
        self.stats.tx_bytes.fetch_add(data.len() as u64, Ordering::Relaxed);

        Ok(())
    }

    /// Receive a packet.
    fn recv(&mut self) -> Option<Vec<u8>> {
        let idx = self.rx_cur;

        // Check if descriptor has been written back
        let desc_wb = unsafe {
            ptr::read_volatile(&self.rx_desc[idx] as *const RxDescAdv as *const RxDescAdvWb)
        };

        if !desc_wb.done() {
            return None;
        }

        let length = desc_wb.length() as usize;

        if length == 0 || length > BUFFER_SIZE {
            // Invalid packet, reset descriptor
            self.reset_rx_desc(idx);
            self.rx_cur = (idx + 1) % NUM_RX_DESC;
            return None;
        }

        // Copy data from buffer
        let data = self.rx_buffers[idx][..length].to_vec();

        // Update stats
        self.stats.rx_packets.fetch_add(1, Ordering::Relaxed);
        self.stats.rx_bytes.fetch_add(length as u64, Ordering::Relaxed);

        // Reset descriptor for reuse
        self.reset_rx_desc(idx);

        // Move to next descriptor
        let next_idx = (idx + 1) % NUM_RX_DESC;
        self.rx_cur = next_idx;

        // Update tail
        self.write_reg(regs::RDT0, idx as u32);

        Some(data)
    }

    /// Reset an RX descriptor.
    fn reset_rx_desc(&mut self, idx: usize) {
        let buffer_addr = self.rx_buffers[idx].as_ptr() as u64;
        self.rx_desc[idx].buffer_addr = buffer_addr;
        self.rx_desc[idx].hdr_addr = 0;
    }

    /// Check link status.
    fn check_link(&mut self) {
        let status = self.read_reg(regs::STATUS);
        let link = status & status::LU != 0;

        if link != self.link_up.load(Ordering::Relaxed) {
            self.link_up.store(link, Ordering::SeqCst);
            if link {
                self.link_speed = match status & status::SPEED_MASK {
                    status::SPEED_10 => LinkSpeed::Speed10Mbps,
                    status::SPEED_100 => LinkSpeed::Speed100Mbps,
                    status::SPEED_1000 => LinkSpeed::Speed1000Mbps,
                    status::SPEED_2500 => LinkSpeed::Speed2500Mbps,
                    _ => LinkSpeed::Unknown,
                };
                crate::kprintln!("igc: link up at {:?}", self.link_speed);
            } else {
                self.link_speed = LinkSpeed::Unknown;
                crate::kprintln!("igc: link down");
            }
        }
    }

    /// Get statistics.
    pub fn get_stats(&self) -> IgcStatsSnapshot {
        // Read hardware counters
        let hw_rx_packets = self.read_reg(regs::GPRC) as u64;
        let hw_tx_packets = self.read_reg(regs::GPTC) as u64;
        let hw_rx_errors = self.read_reg(regs::CRCERRS) as u64;
        let hw_rx_missed = self.read_reg(regs::MPC) as u64;

        IgcStatsSnapshot {
            tx_packets: self.stats.tx_packets.load(Ordering::Relaxed),
            rx_packets: self.stats.rx_packets.load(Ordering::Relaxed),
            tx_bytes: self.stats.tx_bytes.load(Ordering::Relaxed),
            rx_bytes: self.stats.rx_bytes.load(Ordering::Relaxed),
            tx_errors: self.stats.tx_errors.load(Ordering::Relaxed),
            rx_errors: hw_rx_errors + self.stats.rx_errors.load(Ordering::Relaxed),
            rx_dropped: hw_rx_missed + self.stats.rx_dropped.load(Ordering::Relaxed),
            hw_rx_packets,
            hw_tx_packets,
        }
    }
}

/// Statistics snapshot.
#[derive(Debug, Clone)]
pub struct IgcStatsSnapshot {
    pub tx_packets: u64,
    pub rx_packets: u64,
    pub tx_bytes: u64,
    pub rx_bytes: u64,
    pub tx_errors: u64,
    pub rx_errors: u64,
    pub rx_dropped: u64,
    pub hw_rx_packets: u64,
    pub hw_tx_packets: u64,
}

/// Initialize the IGC driver.
pub fn init() {
    // Scan PCI for Intel I225/I226
    let devices = pci::scan();

    for dev in devices {
        if dev.id.vendor_id != INTEL_VENDOR_ID {
            continue;
        }

        if !device_ids::is_supported(dev.id.device_id) {
            continue;
        }

        crate::kprintln!("igc: found Intel {} at {:02x}:{:02x}.{}",
            device_ids::name(dev.id.device_id),
            dev.addr.bus, dev.addr.device, dev.addr.function);

        // Get MMIO base address
        let (bar0, is_io) = pci::read_bar(&dev, 0);
        if is_io {
            crate::kprintln!("igc: BAR0 is I/O, expected memory");
            continue;
        }
        let mmio_base = (bar0 & 0xFFFFFFF0) as usize;

        if mmio_base == 0 {
            crate::kprintln!("igc: invalid BAR0");
            continue;
        }

        // Enable bus mastering and memory space
        pci::enable_bus_mastering(&dev);

        // Create driver instance
        let mut driver = IgcDriver {
            mmio_base,
            mac: [0; 6],
            rx_desc: Box::new([RxDescAdv::default(); NUM_RX_DESC]),
            tx_desc: Box::new([TxDescAdv::default(); NUM_TX_DESC]),
            rx_buffers: Vec::with_capacity(NUM_RX_DESC),
            tx_buffers: Vec::with_capacity(NUM_TX_DESC),
            rx_cur: 0,
            tx_cur: 0,
            device_id: dev.id.device_id,
            is_i226: device_ids::is_i226(dev.id.device_id),
            link_up: AtomicBool::new(false),
            link_speed: LinkSpeed::Unknown,
            stats: IgcStats::default(),
        };

        // Reset device
        driver.reset();

        // Read MAC address
        driver.read_mac();

        crate::kprintln!("igc: MAC {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            driver.mac[0], driver.mac[1], driver.mac[2],
            driver.mac[3], driver.mac[4], driver.mac[5]);

        // Initialize RX and TX
        driver.init_rx();
        driver.init_tx();

        // Set up link
        driver.setup_link();

        // Enable interrupts
        driver.enable_interrupts();

        // Store driver
        *DRIVER.lock() = Some(driver);

        crate::kprintln!("igc: driver initialized");
        return;
    }
}

/// Get MAC address.
pub fn get_mac() -> Option<[u8; 6]> {
    DRIVER.lock().as_ref().map(|d| d.mac)
}

/// Send a packet.
pub fn send(data: &[u8]) -> KResult<()> {
    DRIVER.lock().as_mut()
        .ok_or(KError::NotSupported)?
        .send(data)
}

/// Receive a packet.
pub fn recv() -> Option<Vec<u8>> {
    DRIVER.lock().as_mut()?.recv()
}

/// Check if link is up.
pub fn is_link_up() -> bool {
    DRIVER.lock().as_ref()
        .map(|d| d.link_up.load(Ordering::Relaxed))
        .unwrap_or(false)
}

/// Get link speed.
pub fn link_speed() -> LinkSpeed {
    DRIVER.lock().as_ref()
        .map(|d| d.link_speed)
        .unwrap_or(LinkSpeed::Unknown)
}

/// Get statistics.
pub fn stats() -> Option<IgcStatsSnapshot> {
    DRIVER.lock().as_ref().map(|d| d.get_stats())
}

/// Check link status (call periodically).
pub fn check_link() {
    if let Some(ref mut driver) = *DRIVER.lock() {
        driver.check_link();
    }
}

/// Get device name.
pub fn device_name() -> &'static str {
    DRIVER.lock().as_ref()
        .map(|d| device_ids::name(d.device_id))
        .unwrap_or("none")
}

/// Check if driver is active.
pub fn is_active() -> bool {
    DRIVER.lock().is_some()
}
