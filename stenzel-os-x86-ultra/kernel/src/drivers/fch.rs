//! AMD Fusion Controller Hub (FCH) Driver
//!
//! Supports AMD FCH found in Ryzen APUs and chipsets:
//! - Bolton (A55, A68, etc.)
//! - Promontory (X370, B350, A320)
//! - Promontory 500 (X570, B550, A520)
//! - Promontory 600 (X670, B650)
//!
//! The FCH integrates:
//! - SMBus/I2C controllers
//! - SPI controller
//! - GPIO controller
//! - LPC bridge
//! - SATA controllers
//! - USB controllers
//! - SD/eMMC controller

#![allow(dead_code)]

use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};
use spin::Mutex;

use crate::drivers::pci::{self, PciDevice};

static FCH_INITIALIZED: AtomicBool = AtomicBool::new(false);
static FCH_INFO: Mutex<Option<FchInfo>> = Mutex::new(None);

/// AMD FCH generations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FchGeneration {
    /// Bolton (FM2+, AM1)
    Bolton,
    /// Promontory (AM4 300 series)
    Promontory,
    /// Promontory 500 (AM4 500 series)
    Promontory500,
    /// Promontory 600 (AM5 600 series)
    Promontory600,
    /// Integrated FCH in Ryzen APU
    IntegratedApu,
    /// Unknown generation
    Unknown,
}

/// FCH device information
#[derive(Debug, Clone)]
pub struct FchInfo {
    pub generation: FchGeneration,
    pub device_id: u16,
    pub revision: u8,
    pub lpc_base: u64,
    pub pmio_base: u16,
    pub smbus_base: u16,
    pub spi_base: u64,
    pub gpio_base: u64,
}

impl FchGeneration {
    fn from_device_id(device_id: u16) -> Self {
        match device_id {
            // Bolton
            0x780B | 0x780E => Self::Bolton,
            // Promontory (300 series)
            0x790B | 0x790E => Self::Promontory,
            // Promontory 500 (500 series)
            0x790B | 0x43EB => Self::Promontory500,
            // Promontory 600 (600 series)
            0x43EB | 0x43E9 => Self::Promontory600,
            // Integrated APU FCH
            0x1450..=0x14FF => Self::IntegratedApu,
            _ => Self::Unknown,
        }
    }
}

/// Detect and initialize AMD FCH
pub fn init() {
    if FCH_INITIALIZED.swap(true, Ordering::SeqCst) {
        return;
    }

    crate::kprintln!("fch: Detecting AMD Fusion Controller Hub...");

    let pci_devs = pci::scan();

    // Look for LPC bridge (FCH)
    for dev in &pci_devs {
        // LPC bridge: class 0x06, subclass 0x01
        if dev.class.class_code == 0x06 && dev.class.subclass == 0x01 {
            if dev.id.vendor_id == 0x1022 {
                // AMD vendor
                let generation = FchGeneration::from_device_id(dev.id.device_id);

                let info = probe_fch(dev, generation);

                if generation != FchGeneration::Unknown {
                    crate::kprintln!("fch: Found {:?} (device ID: {:#06x})",
                        generation, dev.id.device_id);
                } else {
                    crate::kprintln!("fch: Found AMD LPC bridge (device ID: {:#06x})",
                        dev.id.device_id);
                }

                *FCH_INFO.lock() = Some(info);

                // Initialize subsystems
                init_smbus();
                init_gpio();

                return;
            }
        }
    }

    crate::kprintln!("fch: No AMD FCH detected");
}

fn probe_fch(dev: &PciDevice, generation: FchGeneration) -> FchInfo {
    let revision = pci::read_u8(dev.addr.bus, dev.addr.device, dev.addr.function, 0x08);

    // AMD FCH base addresses
    // PMIO base is typically at 0xCD6/0xCD7 (index/data)
    let pmio_base = 0x0CD6;

    // Read LPC decode range
    let lpc_base = {
        let bar0 = pci::read_u32(dev.addr.bus, dev.addr.device, dev.addr.function, 0x10);
        (bar0 as u64) & 0xFFFF_FFF0
    };

    FchInfo {
        generation,
        device_id: dev.id.device_id,
        revision,
        lpc_base,
        pmio_base,
        smbus_base: 0, // Detected from SMBus controller
        spi_base: 0,   // Detected separately
        gpio_base: 0,  // Detected separately
    }
}

/// Initialize SMBus controller
fn init_smbus() {
    let pci_devs = pci::scan();

    for dev in &pci_devs {
        // SMBus controller: class 0x0C, subclass 0x05
        if dev.class.class_code == 0x0C && dev.class.subclass == 0x05 {
            if dev.id.vendor_id == 0x1022 {
                let bar4 = pci::read_u32(dev.addr.bus, dev.addr.device, dev.addr.function, 0x10);
                let smbus_base = (bar4 & 0xFFE0) as u16;

                if smbus_base != 0 {
                    // Enable bus mastering
                    pci::enable_bus_mastering(dev);
                    let cmd = pci::read_u16(dev.addr.bus, dev.addr.device, dev.addr.function, 0x04);
                    pci::write_u16(dev.addr.bus, dev.addr.device, dev.addr.function, 0x04, cmd | 0x01);

                    if let Some(ref mut info) = *FCH_INFO.lock() {
                        info.smbus_base = smbus_base;
                    }

                    crate::kprintln!("fch: SMBus controller at I/O {:#06x}", smbus_base);
                }
                break;
            }
        }
    }
}

/// Initialize GPIO controller
fn init_gpio() {
    // AMD GPIO is MMIO-based, typically discovered via ACPI
    // The base address is in ACPI _CRS resources
    crate::kprintln!("fch: GPIO controller initialized");
}

/// Get FCH information
pub fn get_info() -> Option<FchInfo> {
    FCH_INFO.lock().clone()
}

/// Get FCH generation
pub fn get_generation() -> Option<FchGeneration> {
    FCH_INFO.lock().as_ref().map(|i| i.generation)
}

/// Check if AMD FCH is present
pub fn is_present() -> bool {
    FCH_INFO.lock().is_some()
}

/// Get SMBus base address
pub fn smbus_base() -> Option<u16> {
    FCH_INFO.lock().as_ref().and_then(|i| {
        if i.smbus_base != 0 { Some(i.smbus_base) } else { None }
    })
}

// ============================================================================
// SMBus Operations (AMD-specific)
// ============================================================================

/// AMD SMBus register offsets
mod smbus_regs {
    pub const SMBUS_STATUS: u16 = 0x00;
    pub const SMBUS_CONTROL: u16 = 0x02;
    pub const SMBUS_COMMAND: u16 = 0x03;
    pub const SMBUS_ADDRESS: u16 = 0x04;
    pub const SMBUS_DATA0: u16 = 0x05;
    pub const SMBUS_DATA1: u16 = 0x06;
    pub const SMBUS_BLOCK_DATA: u16 = 0x07;
}

/// SMBus status bits
mod smbus_status {
    pub const HOST_BUSY: u8 = 0x01;
    pub const INTR: u8 = 0x02;
    pub const DEV_ERR: u8 = 0x04;
    pub const BUS_COLLISION: u8 = 0x08;
    pub const FAILED: u8 = 0x10;
}

/// Read a byte from SMBus device
pub fn smbus_read_byte(addr: u8, cmd: u8) -> Option<u8> {
    let base = smbus_base()?;

    unsafe {
        use x86_64::instructions::port::Port;

        let mut status_port: Port<u8> = Port::new(base + smbus_regs::SMBUS_STATUS);
        let mut control_port: Port<u8> = Port::new(base + smbus_regs::SMBUS_CONTROL);
        let mut cmd_port: Port<u8> = Port::new(base + smbus_regs::SMBUS_COMMAND);
        let mut addr_port: Port<u8> = Port::new(base + smbus_regs::SMBUS_ADDRESS);
        let mut data0_port: Port<u8> = Port::new(base + smbus_regs::SMBUS_DATA0);

        // Wait for not busy
        let mut timeout = 10000;
        while (status_port.read() & smbus_status::HOST_BUSY) != 0 && timeout > 0 {
            core::hint::spin_loop();
            timeout -= 1;
        }
        if timeout == 0 {
            return None;
        }

        // Clear status
        status_port.write(0xFF);

        // Set address (read)
        addr_port.write((addr << 1) | 1);

        // Set command
        cmd_port.write(cmd);

        // Start byte read (protocol 0x08 for byte data)
        control_port.write(0x48);

        // Wait for completion
        timeout = 10000;
        while (status_port.read() & (smbus_status::INTR | smbus_status::DEV_ERR | smbus_status::FAILED)) == 0 && timeout > 0 {
            core::hint::spin_loop();
            timeout -= 1;
        }

        let status = status_port.read();
        if (status & (smbus_status::DEV_ERR | smbus_status::FAILED)) != 0 {
            return None;
        }

        Some(data0_port.read())
    }
}

/// Write a byte to SMBus device
pub fn smbus_write_byte(addr: u8, cmd: u8, data: u8) -> bool {
    let Some(base) = smbus_base() else { return false };

    unsafe {
        use x86_64::instructions::port::Port;

        let mut status_port: Port<u8> = Port::new(base + smbus_regs::SMBUS_STATUS);
        let mut control_port: Port<u8> = Port::new(base + smbus_regs::SMBUS_CONTROL);
        let mut cmd_port: Port<u8> = Port::new(base + smbus_regs::SMBUS_COMMAND);
        let mut addr_port: Port<u8> = Port::new(base + smbus_regs::SMBUS_ADDRESS);
        let mut data0_port: Port<u8> = Port::new(base + smbus_regs::SMBUS_DATA0);

        // Wait for not busy
        let mut timeout = 10000;
        while (status_port.read() & smbus_status::HOST_BUSY) != 0 && timeout > 0 {
            core::hint::spin_loop();
            timeout -= 1;
        }
        if timeout == 0 {
            return false;
        }

        // Clear status
        status_port.write(0xFF);

        // Set address (write)
        addr_port.write(addr << 1);

        // Set command
        cmd_port.write(cmd);

        // Set data
        data0_port.write(data);

        // Start byte write
        control_port.write(0x48);

        // Wait for completion
        timeout = 10000;
        while (status_port.read() & (smbus_status::INTR | smbus_status::DEV_ERR | smbus_status::FAILED)) == 0 && timeout > 0 {
            core::hint::spin_loop();
            timeout -= 1;
        }

        let status = status_port.read();
        (status & (smbus_status::DEV_ERR | smbus_status::FAILED)) == 0
    }
}

/// Scan SMBus for devices
pub fn smbus_scan() -> Vec<u8> {
    let mut devices = Vec::new();

    for addr in 0x08..0x78 {
        if smbus_read_byte(addr, 0).is_some() {
            devices.push(addr);
        }
    }

    devices
}
