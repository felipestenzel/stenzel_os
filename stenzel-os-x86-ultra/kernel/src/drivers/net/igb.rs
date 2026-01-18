//! Intel I210/I211 Gigabit Ethernet driver (IGB).
//!
//! This driver supports Intel I210/I211/I217/I218/I219 NICs which are common
//! in modern desktop and laptop systems.
//!
//! Features:
//! - MMIO register access
//! - DMA ring buffers for TX/RX
//! - Auto-negotiation support
//! - Multiple queue support (I210)

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::ptr;
use crate::drivers::pci;
use crate::util::{KResult, KError};

/// Intel vendor ID.
const INTEL_VENDOR_ID: u16 = 0x8086;

/// Supported device IDs.
mod device_ids {
    // I210 (Server)
    pub const I210_COPPER: u16 = 0x1533;
    pub const I210_FIBER: u16 = 0x1536;
    pub const I210_SERDES: u16 = 0x1537;
    pub const I210_SGMII: u16 = 0x1538;
    pub const I210_COPPER_FLASHLESS: u16 = 0x157B;
    pub const I210_SERDES_FLASHLESS: u16 = 0x157C;

    // I211 (Desktop)
    pub const I211_COPPER: u16 = 0x1539;

    // I217 (LOM - LAN on Motherboard)
    pub const I217_LM: u16 = 0x153A;
    pub const I217_V: u16 = 0x153B;

    // I218 (LOM)
    pub const I218_LM: u16 = 0x155A;
    pub const I218_V: u16 = 0x1559;
    pub const I218_LM2: u16 = 0x15A0;
    pub const I218_V2: u16 = 0x15A1;
    pub const I218_LM3: u16 = 0x15A2;
    pub const I218_V3: u16 = 0x15A3;

    // I219 (LOM - common in modern laptops/desktops)
    pub const I219_LM: u16 = 0x156F;
    pub const I219_V: u16 = 0x1570;
    pub const I219_LM2: u16 = 0x15B7;
    pub const I219_V2: u16 = 0x15B8;
    pub const I219_LM3: u16 = 0x15B9;
    pub const I219_LM4: u16 = 0x15D7;
    pub const I219_V4: u16 = 0x15D8;
    pub const I219_LM5: u16 = 0x15E3;
    pub const I219_V5: u16 = 0x15D6;
    pub const I219_LM6: u16 = 0x15BD;
    pub const I219_V6: u16 = 0x15BE;
    pub const I219_LM7: u16 = 0x15BB;
    pub const I219_V7: u16 = 0x15BC;
    pub const I219_LM8: u16 = 0x15DF;
    pub const I219_V8: u16 = 0x15E0;
    pub const I219_LM9: u16 = 0x15E1;
    pub const I219_V9: u16 = 0x15E2;
    pub const I219_LM10: u16 = 0x0D4E;
    pub const I219_V10: u16 = 0x0D4F;
    pub const I219_LM11: u16 = 0x0D4C;
    pub const I219_V11: u16 = 0x0D4D;
    pub const I219_LM12: u16 = 0x0D53;
    pub const I219_V12: u16 = 0x0D55;

    pub fn is_supported(device_id: u16) -> bool {
        matches!(device_id,
            I210_COPPER | I210_FIBER | I210_SERDES | I210_SGMII |
            I210_COPPER_FLASHLESS | I210_SERDES_FLASHLESS |
            I211_COPPER |
            I217_LM | I217_V |
            I218_LM | I218_V | I218_LM2 | I218_V2 | I218_LM3 | I218_V3 |
            I219_LM | I219_V | I219_LM2 | I219_V2 | I219_LM3 |
            I219_LM4 | I219_V4 | I219_LM5 | I219_V5 | I219_LM6 | I219_V6 |
            I219_LM7 | I219_V7 | I219_LM8 | I219_V8 | I219_LM9 | I219_V9 |
            I219_LM10 | I219_V10 | I219_LM11 | I219_V11 | I219_LM12 | I219_V12
        )
    }

    pub fn name(device_id: u16) -> &'static str {
        match device_id {
            I210_COPPER | I210_FIBER | I210_SERDES | I210_SGMII |
            I210_COPPER_FLASHLESS | I210_SERDES_FLASHLESS => "I210",
            I211_COPPER => "I211",
            I217_LM | I217_V => "I217",
            I218_LM | I218_V | I218_LM2 | I218_V2 | I218_LM3 | I218_V3 => "I218",
            _ => "I219",
        }
    }
}

/// MMIO register offsets.
mod regs {
    // Device control
    pub const CTRL: u32 = 0x0000;       // Device Control
    pub const STATUS: u32 = 0x0008;     // Device Status
    pub const CTRL_EXT: u32 = 0x0018;   // Extended Device Control

    // EEPROM
    pub const EERD: u32 = 0x0014;       // EEPROM Read
    pub const EEWR: u32 = 0x102C;       // EEPROM Write

    // Interrupts
    pub const ICR: u32 = 0x00C0;        // Interrupt Cause Read
    pub const ICS: u32 = 0x00C8;        // Interrupt Cause Set
    pub const IMS: u32 = 0x00D0;        // Interrupt Mask Set
    pub const IMC: u32 = 0x00D8;        // Interrupt Mask Clear
    pub const IAM: u32 = 0x00E0;        // Interrupt Acknowledge Auto Mask

    // Receive
    pub const RCTL: u32 = 0x0100;       // Receive Control
    pub const RDBAL: u32 = 0xC000;      // RX Descriptor Base Low (Queue 0)
    pub const RDBAH: u32 = 0xC004;      // RX Descriptor Base High
    pub const RDLEN: u32 = 0xC008;      // RX Descriptor Length
    pub const RDH: u32 = 0xC010;        // RX Descriptor Head
    pub const RDT: u32 = 0xC018;        // RX Descriptor Tail
    pub const RXDCTL: u32 = 0xC028;     // RX Descriptor Control

    // Transmit
    pub const TCTL: u32 = 0x0400;       // Transmit Control
    pub const TDBAL: u32 = 0xE000;      // TX Descriptor Base Low (Queue 0)
    pub const TDBAH: u32 = 0xE004;      // TX Descriptor Base High
    pub const TDLEN: u32 = 0xE008;      // TX Descriptor Length
    pub const TDH: u32 = 0xE010;        // TX Descriptor Head
    pub const TDT: u32 = 0xE018;        // TX Descriptor Tail
    pub const TXDCTL: u32 = 0xE028;     // TX Descriptor Control

    // MAC Address
    pub const RAL: u32 = 0x5400;        // Receive Address Low
    pub const RAH: u32 = 0x5404;        // Receive Address High

    // Statistics
    pub const MPC: u32 = 0x4010;        // Missed Packets Count
    pub const GPRC: u32 = 0x4074;       // Good Packets Received Count
    pub const GPTC: u32 = 0x4080;       // Good Packets Transmitted Count
}

/// Control register bits.
mod ctrl {
    pub const FD: u32 = 1 << 0;         // Full Duplex
    pub const GIO_MASTER_DISABLE: u32 = 1 << 2;
    pub const LRST: u32 = 1 << 3;       // Link Reset
    pub const ASDE: u32 = 1 << 5;       // Auto-Speed Detection Enable
    pub const SLU: u32 = 1 << 6;        // Set Link Up
    pub const ILOS: u32 = 1 << 7;       // Invert Loss-of-Signal
    pub const SPEED_MASK: u32 = 3 << 8;
    pub const SPEED_10: u32 = 0 << 8;
    pub const SPEED_100: u32 = 1 << 8;
    pub const SPEED_1000: u32 = 2 << 8;
    pub const FRCSPD: u32 = 1 << 11;    // Force Speed
    pub const FRCDPX: u32 = 1 << 12;    // Force Duplex
    pub const RST: u32 = 1 << 26;       // Device Reset
    pub const VME: u32 = 1 << 30;       // VLAN Mode Enable
    pub const PHY_RST: u32 = 1 << 31;   // PHY Reset
}

/// Status register bits.
mod status {
    pub const FD: u32 = 1 << 0;         // Full Duplex
    pub const LU: u32 = 1 << 1;         // Link Up
    pub const SPEED_MASK: u32 = 3 << 6;
    pub const SPEED_10: u32 = 0 << 6;
    pub const SPEED_100: u32 = 1 << 6;
    pub const SPEED_1000: u32 = 2 << 6;
}

/// Receive control bits.
mod rctl {
    pub const EN: u32 = 1 << 1;         // Receiver Enable
    pub const SBP: u32 = 1 << 2;        // Store Bad Packets
    pub const UPE: u32 = 1 << 3;        // Unicast Promiscuous Enable
    pub const MPE: u32 = 1 << 4;        // Multicast Promiscuous Enable
    pub const LPE: u32 = 1 << 5;        // Long Packet Reception Enable
    pub const LBM_MASK: u32 = 3 << 6;   // Loopback Mode
    pub const RDMTS_HALF: u32 = 0 << 8; // RX Desc Min Threshold Size
    pub const RDMTS_QUARTER: u32 = 1 << 8;
    pub const RDMTS_EIGHTH: u32 = 2 << 8;
    pub const MO_MASK: u32 = 3 << 12;   // Multicast Offset
    pub const BAM: u32 = 1 << 15;       // Broadcast Accept Mode
    pub const BSIZE_MASK: u32 = 3 << 16;
    pub const BSIZE_2048: u32 = 0 << 16;
    pub const BSIZE_1024: u32 = 1 << 16;
    pub const BSIZE_512: u32 = 2 << 16;
    pub const BSIZE_256: u32 = 3 << 16;
    pub const VFE: u32 = 1 << 18;       // VLAN Filter Enable
    pub const CFIEN: u32 = 1 << 19;     // CFI Enable
    pub const CFI: u32 = 1 << 20;       // CFI Value
    pub const DPF: u32 = 1 << 22;       // Discard Pause Frames
    pub const PMCF: u32 = 1 << 23;      // Pass MAC Control Frames
    pub const BSEX: u32 = 1 << 25;      // Buffer Size Extension
    pub const SECRC: u32 = 1 << 26;     // Strip Ethernet CRC
}

/// Transmit control bits.
mod tctl {
    pub const EN: u32 = 1 << 1;         // Transmitter Enable
    pub const PSP: u32 = 1 << 3;        // Pad Short Packets
    pub const CT_SHIFT: u32 = 4;        // Collision Threshold
    pub const COLD_SHIFT: u32 = 12;     // Collision Distance
    pub const SWXOFF: u32 = 1 << 22;    // Software XOFF Transmission
    pub const RTLC: u32 = 1 << 24;      // Re-transmit on Late Collision
    pub const RRTHRESH_MASK: u32 = 3 << 29;
}

/// Interrupt bits.
mod intr {
    pub const TXDW: u32 = 1 << 0;       // TX Descriptor Written Back
    pub const TXQE: u32 = 1 << 1;       // TX Queue Empty
    pub const LSC: u32 = 1 << 2;        // Link Status Change
    pub const RXDMT0: u32 = 1 << 4;     // RX Desc Min Threshold Reached
    pub const RXO: u32 = 1 << 6;        // Receiver Overrun
    pub const RXT0: u32 = 1 << 7;       // Receiver Timer Interrupt
}

/// TX Descriptor (Legacy format).
#[repr(C, align(16))]
#[derive(Clone, Copy)]
struct TxDesc {
    buffer_addr: u64,
    length: u16,
    cso: u8,
    cmd: u8,
    status: u8,
    css: u8,
    special: u16,
}

impl TxDesc {
    const fn new() -> Self {
        Self {
            buffer_addr: 0,
            length: 0,
            cso: 0,
            cmd: 0,
            status: 0,
            css: 0,
            special: 0,
        }
    }
}

/// TX command bits.
mod txcmd {
    pub const EOP: u8 = 1 << 0;         // End of Packet
    pub const IFCS: u8 = 1 << 1;        // Insert FCS
    pub const IC: u8 = 1 << 2;          // Insert Checksum
    pub const RS: u8 = 1 << 3;          // Report Status
    pub const DEXT: u8 = 1 << 5;        // Extension
    pub const VLE: u8 = 1 << 6;         // VLAN Enable
    pub const IDE: u8 = 1 << 7;         // Interrupt Delay Enable
}

/// TX status bits.
mod txsts {
    pub const DD: u8 = 1 << 0;          // Descriptor Done
    pub const EC: u8 = 1 << 1;          // Excess Collisions
    pub const LC: u8 = 1 << 2;          // Late Collision
}

/// RX Descriptor (Legacy format).
#[repr(C, align(16))]
#[derive(Clone, Copy)]
struct RxDesc {
    buffer_addr: u64,
    length: u16,
    checksum: u16,
    status: u8,
    errors: u8,
    special: u16,
}

impl RxDesc {
    const fn new() -> Self {
        Self {
            buffer_addr: 0,
            length: 0,
            checksum: 0,
            status: 0,
            errors: 0,
            special: 0,
        }
    }
}

/// RX status bits.
mod rxsts {
    pub const DD: u8 = 1 << 0;          // Descriptor Done
    pub const EOP: u8 = 1 << 1;         // End of Packet
    pub const IXSM: u8 = 1 << 2;        // Ignore Checksum Indication
    pub const VP: u8 = 1 << 3;          // VLAN Packet
    pub const TCPCS: u8 = 1 << 5;       // TCP Checksum Calculated
    pub const IPCS: u8 = 1 << 6;        // IP Checksum Calculated
    pub const PIF: u8 = 1 << 7;         // Passed In-exact Filter
}

/// RX error bits.
mod rxerr {
    pub const CE: u8 = 1 << 0;          // CRC Error or Alignment Error
    pub const SE: u8 = 1 << 1;          // Symbol Error
    pub const SEQ: u8 = 1 << 2;         // Sequence Error
    pub const CXE: u8 = 1 << 4;         // Carrier Extension Error
    pub const TCPE: u8 = 1 << 5;        // TCP/UDP Checksum Error
    pub const IPE: u8 = 1 << 6;         // IP Checksum Error
    pub const RXE: u8 = 1 << 7;         // RX Data Error
}

const TX_RING_SIZE: usize = 64;
const RX_RING_SIZE: usize = 64;
const BUFFER_SIZE: usize = 2048;

/// Intel I210/I211 NIC driver.
pub struct Igb {
    mmio_base: u64,
    mac: [u8; 6],
    device_id: u16,
    tx_ring: Box<[TxDesc; TX_RING_SIZE]>,
    rx_ring: Box<[RxDesc; RX_RING_SIZE]>,
    tx_buffers: Box<[[u8; BUFFER_SIZE]; TX_RING_SIZE]>,
    rx_buffers: Box<[[u8; BUFFER_SIZE]; RX_RING_SIZE]>,
    tx_head: usize,
    tx_tail: usize,
    rx_index: usize,
    initialized: bool,
}

static mut IGB_DRIVER: Option<Igb> = None;

impl Igb {
    /// Create a new IGB driver instance.
    fn new(mmio_base: u64, device_id: u16) -> Self {
        Self {
            mmio_base,
            mac: [0; 6],
            device_id,
            tx_ring: Box::new([TxDesc::new(); TX_RING_SIZE]),
            rx_ring: Box::new([RxDesc::new(); RX_RING_SIZE]),
            tx_buffers: Box::new([[0u8; BUFFER_SIZE]; TX_RING_SIZE]),
            rx_buffers: Box::new([[0u8; BUFFER_SIZE]; RX_RING_SIZE]),
            tx_head: 0,
            tx_tail: 0,
            rx_index: 0,
            initialized: false,
        }
    }

    /// Read 32-bit MMIO register.
    fn read32(&self, reg: u32) -> u32 {
        unsafe {
            let ptr = (self.mmio_base + reg as u64) as *const u32;
            ptr::read_volatile(ptr)
        }
    }

    /// Write 32-bit MMIO register.
    fn write32(&self, reg: u32, value: u32) {
        unsafe {
            let ptr = (self.mmio_base + reg as u64) as *mut u32;
            ptr::write_volatile(ptr, value);
        }
    }

    /// Read from EEPROM.
    fn eeprom_read(&self, addr: u16) -> u16 {
        // Start read
        self.write32(regs::EERD, ((addr as u32) << 2) | 0x01);

        // Wait for completion
        for _ in 0..1000 {
            let val = self.read32(regs::EERD);
            if val & 0x02 != 0 {
                return (val >> 16) as u16;
            }
            // Small delay
            for _ in 0..100 {
                core::hint::spin_loop();
            }
        }

        0xFFFF
    }

    /// Reset the device.
    fn reset(&mut self) {
        // Disable interrupts
        self.write32(regs::IMC, 0xFFFFFFFF);

        // Global reset
        let ctrl = self.read32(regs::CTRL);
        self.write32(regs::CTRL, ctrl | ctrl::RST);

        // Wait for reset to complete
        for _ in 0..1000 {
            let ctrl = self.read32(regs::CTRL);
            if ctrl & ctrl::RST == 0 {
                break;
            }
            for _ in 0..1000 {
                core::hint::spin_loop();
            }
        }

        // Disable interrupts again after reset
        self.write32(regs::IMC, 0xFFFFFFFF);

        // Clear interrupt causes
        let _ = self.read32(regs::ICR);
    }

    /// Read MAC address.
    fn read_mac(&mut self) {
        // Try reading from RAL/RAH first
        let ral = self.read32(regs::RAL);
        let rah = self.read32(regs::RAH);

        if ral != 0 && ral != 0xFFFFFFFF {
            self.mac[0] = (ral & 0xFF) as u8;
            self.mac[1] = ((ral >> 8) & 0xFF) as u8;
            self.mac[2] = ((ral >> 16) & 0xFF) as u8;
            self.mac[3] = ((ral >> 24) & 0xFF) as u8;
            self.mac[4] = (rah & 0xFF) as u8;
            self.mac[5] = ((rah >> 8) & 0xFF) as u8;
            return;
        }

        // Try EEPROM
        let word0 = self.eeprom_read(0);
        let word1 = self.eeprom_read(1);
        let word2 = self.eeprom_read(2);

        if word0 != 0xFFFF {
            self.mac[0] = (word0 & 0xFF) as u8;
            self.mac[1] = (word0 >> 8) as u8;
            self.mac[2] = (word1 & 0xFF) as u8;
            self.mac[3] = (word1 >> 8) as u8;
            self.mac[4] = (word2 & 0xFF) as u8;
            self.mac[5] = (word2 >> 8) as u8;

            // Write to RAL/RAH
            self.write32(regs::RAL,
                (self.mac[0] as u32) |
                ((self.mac[1] as u32) << 8) |
                ((self.mac[2] as u32) << 16) |
                ((self.mac[3] as u32) << 24)
            );
            self.write32(regs::RAH,
                (self.mac[4] as u32) |
                ((self.mac[5] as u32) << 8) |
                (1 << 31)  // Address Valid
            );
        }
    }

    /// Initialize TX ring.
    fn init_tx(&mut self) {
        let tx_ring_phys = self.tx_ring.as_ptr() as u64;

        // Set TX descriptor ring address
        self.write32(regs::TDBAL, tx_ring_phys as u32);
        self.write32(regs::TDBAH, (tx_ring_phys >> 32) as u32);

        // Set ring length (in bytes)
        self.write32(regs::TDLEN, (TX_RING_SIZE * 16) as u32);

        // Set head and tail
        self.write32(regs::TDH, 0);
        self.write32(regs::TDT, 0);

        // Enable TX
        self.write32(regs::TCTL,
            tctl::EN |
            tctl::PSP |
            (0x0F << tctl::CT_SHIFT) |    // Collision Threshold
            (0x40 << tctl::COLD_SHIFT) |  // Collision Distance (Full Duplex)
            tctl::RTLC
        );

        // TX descriptor control
        self.write32(regs::TXDCTL,
            (1 << 25) |  // WTHRESH
            (1 << 16) |  // HTHRESH
            (1 << 0)     // PTHRESH
        );

        self.tx_head = 0;
        self.tx_tail = 0;
    }

    /// Initialize RX ring.
    fn init_rx(&mut self) {
        // Initialize RX descriptors with buffer addresses
        for i in 0..RX_RING_SIZE {
            let buffer_phys = self.rx_buffers[i].as_ptr() as u64;
            self.rx_ring[i].buffer_addr = buffer_phys;
        }

        let rx_ring_phys = self.rx_ring.as_ptr() as u64;

        // Set RX descriptor ring address
        self.write32(regs::RDBAL, rx_ring_phys as u32);
        self.write32(regs::RDBAH, (rx_ring_phys >> 32) as u32);

        // Set ring length (in bytes)
        self.write32(regs::RDLEN, (RX_RING_SIZE * 16) as u32);

        // Set head and tail
        self.write32(regs::RDH, 0);
        self.write32(regs::RDT, (RX_RING_SIZE - 1) as u32);

        // Enable RX
        self.write32(regs::RCTL,
            rctl::EN |
            rctl::BAM |           // Accept broadcast
            rctl::BSIZE_2048 |    // 2048 byte buffers
            rctl::SECRC           // Strip CRC
        );

        self.rx_index = 0;
    }

    /// Enable interrupts.
    fn enable_interrupts(&self) {
        self.write32(regs::IMS,
            intr::TXDW |   // TX Done
            intr::LSC |    // Link Status Change
            intr::RXT0 |   // RX Timer
            intr::RXO      // RX Overrun
        );
    }

    /// Initialize the device.
    pub fn init(&mut self) {
        // Reset
        self.reset();

        // Set link up
        let ctrl = self.read32(regs::CTRL);
        self.write32(regs::CTRL, ctrl | ctrl::SLU | ctrl::ASDE);

        // Read MAC address
        self.read_mac();

        // Initialize rings
        self.init_tx();
        self.init_rx();

        // Enable interrupts
        self.enable_interrupts();

        self.initialized = true;

        crate::kprintln!("IGB: {} initialized, MAC: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            device_ids::name(self.device_id),
            self.mac[0], self.mac[1], self.mac[2],
            self.mac[3], self.mac[4], self.mac[5]);
    }

    /// Send a packet.
    pub fn send(&mut self, data: &[u8]) -> KResult<()> {
        if !self.initialized {
            return Err(KError::NotSupported);
        }

        if data.len() > BUFFER_SIZE {
            return Err(KError::OutOfRange);
        }

        let tx_index = self.tx_tail;

        // Copy data to buffer
        self.tx_buffers[tx_index][..data.len()].copy_from_slice(data);

        // Setup descriptor
        let buffer_phys = self.tx_buffers[tx_index].as_ptr() as u64;
        self.tx_ring[tx_index].buffer_addr = buffer_phys;
        self.tx_ring[tx_index].length = data.len() as u16;
        self.tx_ring[tx_index].cmd = txcmd::EOP | txcmd::IFCS | txcmd::RS;
        self.tx_ring[tx_index].status = 0;

        // Update tail
        self.tx_tail = (self.tx_tail + 1) % TX_RING_SIZE;
        self.write32(regs::TDT, self.tx_tail as u32);

        // Wait for completion
        for _ in 0..10000 {
            if self.tx_ring[tx_index].status & txsts::DD != 0 {
                return Ok(());
            }
            core::hint::spin_loop();
        }

        Err(KError::Timeout)
    }

    /// Receive a packet.
    pub fn recv(&mut self) -> Option<Vec<u8>> {
        if !self.initialized {
            return None;
        }

        let rx_index = self.rx_index;
        let desc = &mut self.rx_ring[rx_index];

        // Check if packet is ready
        if desc.status & rxsts::DD == 0 {
            return None;
        }

        // Check for errors
        if desc.errors != 0 {
            // Reset descriptor and continue
            desc.status = 0;
            self.rx_index = (self.rx_index + 1) % RX_RING_SIZE;
            self.write32(regs::RDT, ((self.rx_index + RX_RING_SIZE - 1) % RX_RING_SIZE) as u32);
            return None;
        }

        // Check for complete packet
        if desc.status & rxsts::EOP == 0 {
            // Jumbo frame, not supported
            desc.status = 0;
            self.rx_index = (self.rx_index + 1) % RX_RING_SIZE;
            self.write32(regs::RDT, ((self.rx_index + RX_RING_SIZE - 1) % RX_RING_SIZE) as u32);
            return None;
        }

        let length = desc.length as usize;
        if length == 0 || length > BUFFER_SIZE {
            desc.status = 0;
            self.rx_index = (self.rx_index + 1) % RX_RING_SIZE;
            self.write32(regs::RDT, ((self.rx_index + RX_RING_SIZE - 1) % RX_RING_SIZE) as u32);
            return None;
        }

        // Copy data
        let mut packet = vec![0u8; length];
        packet.copy_from_slice(&self.rx_buffers[rx_index][..length]);

        // Reset descriptor
        desc.status = 0;

        // Update index and tail
        self.rx_index = (self.rx_index + 1) % RX_RING_SIZE;
        self.write32(regs::RDT, ((self.rx_index + RX_RING_SIZE - 1) % RX_RING_SIZE) as u32);

        Some(packet)
    }

    /// Handle interrupt.
    pub fn handle_interrupt(&mut self) {
        if !self.initialized {
            return;
        }

        let icr = self.read32(regs::ICR);

        if icr & intr::LSC != 0 {
            // Link status changed
            let status = self.read32(regs::STATUS);
            if status & status::LU != 0 {
                crate::kprintln!("IGB: Link up");
            } else {
                crate::kprintln!("IGB: Link down");
            }
        }

        if icr & intr::RXO != 0 {
            crate::kprintln!("IGB: RX overrun");
        }
    }

    /// Check if link is up.
    pub fn is_link_up(&self) -> bool {
        if !self.initialized {
            return false;
        }
        let status = self.read32(regs::STATUS);
        status & status::LU != 0
    }

    /// Get link speed in Mbps.
    pub fn get_speed(&self) -> u32 {
        if !self.initialized || !self.is_link_up() {
            return 0;
        }

        let status = self.read32(regs::STATUS);
        match status & status::SPEED_MASK {
            status::SPEED_10 => 10,
            status::SPEED_100 => 100,
            status::SPEED_1000 => 1000,
            _ => 0,
        }
    }
}

/// Probe PCI for Intel I210/I211/I219 devices.
pub fn probe_pci() -> Option<(u64, u16)> {
    let devices = pci::scan();

    for dev in devices {
        if dev.id.vendor_id == INTEL_VENDOR_ID && device_ids::is_supported(dev.id.device_id) {
            // Enable bus mastering
            pci::enable_bus_mastering(&dev);

            // Get BAR0 (MMIO)
            let (bar0, is_io) = pci::read_bar(&dev, 0);
            if !is_io && bar0 != 0 {
                // Memory-mapped
                let mmio_base = bar0 & !0xF;
                return Some((mmio_base, dev.id.device_id));
            }
        }
    }

    None
}

/// Initialize the IGB driver.
pub fn init() {
    if let Some((mmio_base, device_id)) = probe_pci() {
        unsafe {
            let mut driver = Igb::new(mmio_base, device_id);
            driver.init();
            IGB_DRIVER = Some(driver);
        }
    }
}

/// Get MAC address.
pub fn get_mac() -> Option<[u8; 6]> {
    unsafe {
        IGB_DRIVER.as_ref().map(|d| d.mac)
    }
}

/// Send a packet.
pub fn send(data: &[u8]) -> KResult<()> {
    unsafe {
        match IGB_DRIVER.as_mut() {
            Some(d) => d.send(data),
            None => Err(KError::NotSupported),
        }
    }
}

/// Receive a packet.
pub fn recv() -> Option<Vec<u8>> {
    unsafe {
        IGB_DRIVER.as_mut().and_then(|d| d.recv())
    }
}

/// Handle interrupt.
pub fn handle_interrupt() {
    unsafe {
        if let Some(d) = IGB_DRIVER.as_mut() {
            d.handle_interrupt();
        }
    }
}

/// Check if link is up.
pub fn is_link_up() -> bool {
    unsafe {
        IGB_DRIVER.as_ref().map_or(false, |d| d.is_link_up())
    }
}

/// Get link speed.
pub fn get_speed() -> u32 {
    unsafe {
        IGB_DRIVER.as_ref().map_or(0, |d| d.get_speed())
    }
}

/// Get device name.
pub fn device_name() -> &'static str {
    unsafe {
        IGB_DRIVER.as_ref().map_or("none", |d| device_ids::name(d.device_id))
    }
}
