//! Intel E1000/E1000e Network Driver
//!
//! Implements driver for Intel 82540EM Gigabit Ethernet (E1000) and variants.
//! This is commonly emulated by QEMU and VMware.

#![allow(dead_code)]

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::sync::atomic::{fence, Ordering};

use x86_64::VirtAddr;

use crate::drivers::pci::{self, PciDevice};
use crate::mm;
use crate::sync::IrqSafeMutex;
use crate::util::{KError, KResult};

// Intel vendor ID
const INTEL_VENDOR_ID: u16 = 0x8086;

// E1000 device IDs
const E1000_DEV_ID_82540EM: u16 = 0x100E;  // QEMU default
const E1000_DEV_ID_82545EM_A: u16 = 0x100F;
const E1000_DEV_ID_82574L: u16 = 0x10D3;   // E1000e
const E1000_DEV_ID_82579LM: u16 = 0x1502;  // E1000e
const E1000_DEV_ID_I217LM: u16 = 0x153A;   // E1000e

// E1000 Register offsets
const REG_CTRL: u32 = 0x0000;     // Device Control
const REG_STATUS: u32 = 0x0008;   // Device Status
const REG_EECD: u32 = 0x0010;     // EEPROM Control
const REG_EERD: u32 = 0x0014;     // EEPROM Read
const REG_ICR: u32 = 0x00C0;      // Interrupt Cause Read
const REG_IMS: u32 = 0x00D0;      // Interrupt Mask Set
const REG_IMC: u32 = 0x00D8;      // Interrupt Mask Clear
const REG_RCTL: u32 = 0x0100;     // Receive Control
const REG_TCTL: u32 = 0x0400;     // Transmit Control
const REG_RDBAL: u32 = 0x2800;    // RX Descriptor Base Low
const REG_RDBAH: u32 = 0x2804;    // RX Descriptor Base High
const REG_RDLEN: u32 = 0x2808;    // RX Descriptor Length
const REG_RDH: u32 = 0x2810;      // RX Descriptor Head
const REG_RDT: u32 = 0x2818;      // RX Descriptor Tail
const REG_TDBAL: u32 = 0x3800;    // TX Descriptor Base Low
const REG_TDBAH: u32 = 0x3804;    // TX Descriptor Base High
const REG_TDLEN: u32 = 0x3808;    // TX Descriptor Length
const REG_TDH: u32 = 0x3810;      // TX Descriptor Head
const REG_TDT: u32 = 0x3818;      // TX Descriptor Tail
const REG_RAL0: u32 = 0x5400;     // Receive Address Low 0
const REG_RAH0: u32 = 0x5404;     // Receive Address High 0
const REG_MTA: u32 = 0x5200;      // Multicast Table Array

// Control Register bits
const CTRL_FD: u32 = 1 << 0;      // Full Duplex
const CTRL_ASDE: u32 = 1 << 5;    // Auto-Speed Detection Enable
const CTRL_SLU: u32 = 1 << 6;     // Set Link Up
const CTRL_RST: u32 = 1 << 26;    // Device Reset

// Receive Control bits
const RCTL_EN: u32 = 1 << 1;      // Receiver Enable
const RCTL_SBP: u32 = 1 << 2;     // Store Bad Packets
const RCTL_UPE: u32 = 1 << 3;     // Unicast Promiscuous Enable
const RCTL_MPE: u32 = 1 << 4;     // Multicast Promiscuous Enable
const RCTL_LPE: u32 = 1 << 5;     // Long Packet Enable
const RCTL_BAM: u32 = 1 << 15;    // Broadcast Accept Mode
const RCTL_BSIZE_2048: u32 = 0 << 16; // Buffer size 2048
const RCTL_BSIZE_4096: u32 = 3 << 16; // Buffer size 4096
const RCTL_SECRC: u32 = 1 << 26;  // Strip Ethernet CRC

// Transmit Control bits
const TCTL_EN: u32 = 1 << 1;      // Transmitter Enable
const TCTL_PSP: u32 = 1 << 3;     // Pad Short Packets
const TCTL_CT_SHIFT: u32 = 4;     // Collision Threshold
const TCTL_COLD_SHIFT: u32 = 12;  // Collision Distance

// TX Descriptor Command bits
const TDESC_CMD_EOP: u8 = 1 << 0;  // End of Packet
const TDESC_CMD_IFCS: u8 = 1 << 1; // Insert FCS
const TDESC_CMD_RS: u8 = 1 << 3;   // Report Status

// TX Descriptor Status bits
const TDESC_STA_DD: u8 = 1 << 0;   // Descriptor Done

// RX Descriptor Status bits
const RDESC_STA_DD: u8 = 1 << 0;   // Descriptor Done
const RDESC_STA_EOP: u8 = 1 << 1;  // End of Packet

// Number of descriptors
const NUM_RX_DESC: usize = 32;
const NUM_TX_DESC: usize = 32;
const BUFFER_SIZE: usize = 2048;

/// E1000 RX Descriptor (legacy format)
#[repr(C, align(16))]
#[derive(Clone, Copy, Default)]
struct RxDesc {
    buffer_addr: u64,
    length: u16,
    checksum: u16,
    status: u8,
    errors: u8,
    special: u16,
}

/// E1000 TX Descriptor (legacy format)
#[repr(C, align(16))]
#[derive(Clone, Copy, Default)]
struct TxDesc {
    buffer_addr: u64,
    length: u16,
    cso: u8,
    cmd: u8,
    sta: u8,
    css: u8,
    special: u16,
}

/// E1000 Network Driver
pub struct E1000 {
    /// MMIO base address
    mmio_base: u64,
    /// MAC address
    mac: [u8; 6],
    /// RX descriptor ring (physically contiguous)
    rx_descs: Box<[RxDesc; NUM_RX_DESC]>,
    /// TX descriptor ring (physically contiguous)
    tx_descs: Box<[TxDesc; NUM_TX_DESC]>,
    /// RX buffers
    rx_buffers: Box<[[u8; BUFFER_SIZE]; NUM_RX_DESC]>,
    /// TX buffers
    tx_buffers: Box<[[u8; BUFFER_SIZE]; NUM_TX_DESC]>,
    /// Current RX descriptor index
    rx_cur: usize,
    /// Current TX descriptor index
    tx_cur: usize,
}

impl E1000 {
    /// Try to initialize E1000 from a PCI device
    pub fn try_new(pci_dev: &PciDevice) -> KResult<Self> {
        // Get MMIO base address from BAR0
        let (bar0, is_io) = pci::read_bar(pci_dev, 0);
        if bar0 == 0 || is_io {
            return Err(KError::NotSupported);
        }

        // Enable bus mastering and MMIO
        pci::enable_bus_mastering(pci_dev);

        // Convert physical MMIO address to virtual address using physical memory offset
        let phys_offset = mm::physical_memory_offset();
        let mmio_base = (phys_offset + bar0).as_u64();

        let mut driver = Self {
            mmio_base,
            mac: [0; 6],
            rx_descs: Box::new([RxDesc::default(); NUM_RX_DESC]),
            tx_descs: Box::new([TxDesc::default(); NUM_TX_DESC]),
            rx_buffers: Box::new([[0u8; BUFFER_SIZE]; NUM_RX_DESC]),
            tx_buffers: Box::new([[0u8; BUFFER_SIZE]; NUM_TX_DESC]),
            rx_cur: 0,
            tx_cur: 0,
        };

        // Reset the device
        driver.reset()?;

        // Read MAC address
        driver.read_mac();

        // Initialize RX
        driver.init_rx();

        // Initialize TX
        driver.init_tx();

        // Enable interrupts (we'll use polling though)
        driver.write_reg(REG_IMS, 0x1F6DC);
        driver.write_reg(REG_IMS, 0xFF & !4);

        // Clear pending interrupts
        driver.read_reg(REG_ICR);

        crate::kprintln!("e1000: initialized at MMIO 0x{:x}", mmio_base);
        crate::kprintln!("e1000: MAC = {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            driver.mac[0], driver.mac[1], driver.mac[2],
            driver.mac[3], driver.mac[4], driver.mac[5]);

        Ok(driver)
    }

    fn read_reg(&self, reg: u32) -> u32 {
        unsafe {
            let ptr = (self.mmio_base + reg as u64) as *const u32;
            core::ptr::read_volatile(ptr)
        }
    }

    fn write_reg(&self, reg: u32, value: u32) {
        unsafe {
            let ptr = (self.mmio_base + reg as u64) as *mut u32;
            core::ptr::write_volatile(ptr, value);
        }
    }

    fn reset(&mut self) -> KResult<()> {
        // Set the reset bit
        self.write_reg(REG_CTRL, CTRL_RST);

        // Wait for reset to complete
        for _ in 0..10000 {
            if (self.read_reg(REG_CTRL) & CTRL_RST) == 0 {
                break;
            }
            core::hint::spin_loop();
        }

        // Wait a bit more
        for _ in 0..100000 {
            core::hint::spin_loop();
        }

        // Disable interrupts
        self.write_reg(REG_IMC, 0xFFFFFFFF);

        // Clear any pending interrupts
        self.read_reg(REG_ICR);

        // Set link up
        let ctrl = self.read_reg(REG_CTRL);
        self.write_reg(REG_CTRL, ctrl | CTRL_SLU | CTRL_ASDE);

        // Clear multicast table array
        for i in 0..128 {
            self.write_reg(REG_MTA + i * 4, 0);
        }

        Ok(())
    }

    fn read_mac(&mut self) {
        // Try to read from EEPROM first
        if self.read_mac_from_eeprom().is_err() {
            // Fall back to reading from RAL/RAH
            let ral = self.read_reg(REG_RAL0);
            let rah = self.read_reg(REG_RAH0);

            self.mac[0] = (ral >> 0) as u8;
            self.mac[1] = (ral >> 8) as u8;
            self.mac[2] = (ral >> 16) as u8;
            self.mac[3] = (ral >> 24) as u8;
            self.mac[4] = (rah >> 0) as u8;
            self.mac[5] = (rah >> 8) as u8;
        }

        // If MAC is still all zeros or all ones, generate a random one
        if self.mac == [0; 6] || self.mac == [0xFF; 6] {
            // Use a deterministic "random" MAC based on MMIO address
            self.mac[0] = 0x52;  // Locally administered
            self.mac[1] = 0x54;
            self.mac[2] = 0x00;
            self.mac[3] = ((self.mmio_base >> 16) & 0xFF) as u8;
            self.mac[4] = ((self.mmio_base >> 8) & 0xFF) as u8;
            self.mac[5] = (self.mmio_base & 0xFF) as u8;
        }

        // Write MAC to receive address register
        let ral = (self.mac[0] as u32) |
                  ((self.mac[1] as u32) << 8) |
                  ((self.mac[2] as u32) << 16) |
                  ((self.mac[3] as u32) << 24);
        let rah = (self.mac[4] as u32) |
                  ((self.mac[5] as u32) << 8) |
                  (1 << 31);  // Address Valid bit

        self.write_reg(REG_RAL0, ral);
        self.write_reg(REG_RAH0, rah);
    }

    fn read_mac_from_eeprom(&mut self) -> KResult<()> {
        // Try to read MAC from EEPROM
        for i in 0usize..3 {
            self.write_reg(REG_EERD, ((i as u32) << 8) | 1);

            // Wait for read to complete
            let mut value = 0u32;
            for _ in 0..10000 {
                value = self.read_reg(REG_EERD);
                if (value & (1 << 4)) != 0 {
                    break;
                }
                core::hint::spin_loop();
            }

            if (value & (1 << 4)) == 0 {
                return Err(KError::Timeout);
            }

            let word = (value >> 16) as u16;
            self.mac[i * 2] = word as u8;
            self.mac[i * 2 + 1] = (word >> 8) as u8;
        }

        Ok(())
    }

    fn init_rx(&mut self) {
        // Set up RX descriptors
        for i in 0..NUM_RX_DESC {
            let buf_addr = mm::virt_to_phys(VirtAddr::from_ptr(self.rx_buffers[i].as_ptr()))
                .map(|p| p.as_u64())
                .unwrap_or(0);

            self.rx_descs[i].buffer_addr = buf_addr;
            self.rx_descs[i].status = 0;
        }

        // Get physical address of descriptor ring
        let descs_phys = mm::virt_to_phys(VirtAddr::from_ptr(self.rx_descs.as_ptr()))
            .map(|p| p.as_u64())
            .unwrap_or(0);

        // Set descriptor base address
        self.write_reg(REG_RDBAL, descs_phys as u32);
        self.write_reg(REG_RDBAH, (descs_phys >> 32) as u32);

        // Set descriptor ring length
        self.write_reg(REG_RDLEN, (NUM_RX_DESC * core::mem::size_of::<RxDesc>()) as u32);

        // Set head and tail
        self.write_reg(REG_RDH, 0);
        self.write_reg(REG_RDT, (NUM_RX_DESC - 1) as u32);

        // Enable receiver
        let rctl = RCTL_EN | RCTL_BAM | RCTL_BSIZE_2048 | RCTL_SECRC;
        self.write_reg(REG_RCTL, rctl);
    }

    fn init_tx(&mut self) {
        // Set up TX descriptors
        for i in 0..NUM_TX_DESC {
            let buf_addr = mm::virt_to_phys(VirtAddr::from_ptr(self.tx_buffers[i].as_ptr()))
                .map(|p| p.as_u64())
                .unwrap_or(0);

            self.tx_descs[i].buffer_addr = buf_addr;
            self.tx_descs[i].cmd = 0;
            self.tx_descs[i].sta = TDESC_STA_DD;  // Mark as done initially
        }

        // Get physical address of descriptor ring
        let descs_phys = mm::virt_to_phys(VirtAddr::from_ptr(self.tx_descs.as_ptr()))
            .map(|p| p.as_u64())
            .unwrap_or(0);

        // Set descriptor base address
        self.write_reg(REG_TDBAL, descs_phys as u32);
        self.write_reg(REG_TDBAH, (descs_phys >> 32) as u32);

        // Set descriptor ring length
        self.write_reg(REG_TDLEN, (NUM_TX_DESC * core::mem::size_of::<TxDesc>()) as u32);

        // Set head and tail
        self.write_reg(REG_TDH, 0);
        self.write_reg(REG_TDT, 0);

        // Enable transmitter
        let tctl = TCTL_EN | TCTL_PSP |
                   (15 << TCTL_CT_SHIFT) |      // Collision Threshold
                   (64 << TCTL_COLD_SHIFT);     // Collision Distance
        self.write_reg(REG_TCTL, tctl);
    }

    /// Get MAC address
    pub fn mac(&self) -> [u8; 6] {
        self.mac
    }

    /// Send a packet
    pub fn send(&mut self, data: &[u8]) -> KResult<()> {
        if data.len() > BUFFER_SIZE {
            return Err(KError::Invalid);
        }

        let old_cur = self.tx_cur;

        // Wait for descriptor to be available
        for _ in 0..100000 {
            if (self.tx_descs[self.tx_cur].sta & TDESC_STA_DD) != 0 {
                break;
            }
            core::hint::spin_loop();
        }

        if (self.tx_descs[self.tx_cur].sta & TDESC_STA_DD) == 0 {
            return Err(KError::Busy);
        }

        // Copy data to buffer
        self.tx_buffers[self.tx_cur][..data.len()].copy_from_slice(data);

        // Set up descriptor
        self.tx_descs[self.tx_cur].length = data.len() as u16;
        self.tx_descs[self.tx_cur].cmd = TDESC_CMD_EOP | TDESC_CMD_IFCS | TDESC_CMD_RS;
        self.tx_descs[self.tx_cur].sta = 0;

        fence(Ordering::SeqCst);

        // Advance tail
        self.tx_cur = (self.tx_cur + 1) % NUM_TX_DESC;
        self.write_reg(REG_TDT, self.tx_cur as u32);

        // Wait for completion
        for _ in 0..100000 {
            fence(Ordering::SeqCst);
            if (self.tx_descs[old_cur].sta & TDESC_STA_DD) != 0 {
                return Ok(());
            }
            core::hint::spin_loop();
        }

        Err(KError::Timeout)
    }

    /// Receive a packet (if available)
    pub fn recv(&mut self) -> Option<Vec<u8>> {
        fence(Ordering::SeqCst);

        // Check if current descriptor has a packet
        if (self.rx_descs[self.rx_cur].status & RDESC_STA_DD) == 0 {
            return None;
        }

        // Check for end of packet
        if (self.rx_descs[self.rx_cur].status & RDESC_STA_EOP) == 0 {
            // Multi-descriptor packets not supported, skip
            self.rx_descs[self.rx_cur].status = 0;
            let old_tail = self.rx_cur;
            self.rx_cur = (self.rx_cur + 1) % NUM_RX_DESC;
            self.write_reg(REG_RDT, old_tail as u32);
            return None;
        }

        let length = self.rx_descs[self.rx_cur].length as usize;

        // Copy packet data
        let packet = self.rx_buffers[self.rx_cur][..length].to_vec();

        // Reset descriptor
        self.rx_descs[self.rx_cur].status = 0;

        // Advance tail
        let old_tail = self.rx_cur;
        self.rx_cur = (self.rx_cur + 1) % NUM_RX_DESC;
        self.write_reg(REG_RDT, old_tail as u32);

        Some(packet)
    }
}

// Global driver instance
static E1000_DRIVER: IrqSafeMutex<Option<E1000>> = IrqSafeMutex::new(None);

/// Check if a PCI device is an E1000
fn is_e1000_device(dev: &PciDevice) -> bool {
    if dev.id.vendor_id != INTEL_VENDOR_ID {
        return false;
    }

    matches!(dev.id.device_id,
        E1000_DEV_ID_82540EM |
        E1000_DEV_ID_82545EM_A |
        E1000_DEV_ID_82574L |
        E1000_DEV_ID_82579LM |
        E1000_DEV_ID_I217LM
    )
}

/// Initialize E1000 driver
pub fn init() {
    let devices = pci::scan();

    for dev in devices {
        if is_e1000_device(&dev) {
            crate::kprintln!("e1000: found device {:04x}:{:04x} @ {:02x}:{:02x}.{}",
                dev.id.vendor_id, dev.id.device_id,
                dev.addr.bus, dev.addr.device, dev.addr.function);

            match E1000::try_new(&dev) {
                Ok(e1000) => {
                    let mut driver = E1000_DRIVER.lock();
                    *driver = Some(e1000);
                    return;
                }
                Err(e) => {
                    crate::kprintln!("e1000: initialization failed: {:?}", e);
                }
            }
        }
    }

    crate::kprintln!("e1000: no device found");
}

/// Get MAC address
pub fn get_mac() -> Option<[u8; 6]> {
    E1000_DRIVER.lock().as_ref().map(|e| e.mac())
}

/// Send a packet
pub fn send(data: &[u8]) -> KResult<()> {
    E1000_DRIVER.lock().as_mut().ok_or(KError::NotFound)?.send(data)
}

/// Receive a packet
pub fn recv() -> Option<Vec<u8>> {
    E1000_DRIVER.lock().as_mut()?.recv()
}

/// Check if driver is available
pub fn is_available() -> bool {
    E1000_DRIVER.lock().is_some()
}
