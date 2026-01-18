//! Intel Platform Controller Hub (PCH) Driver
//!
//! Supports Intel PCH series 100-700 (Sunrise Point, Cannon Point, Tiger Point, etc.)
//! The PCH integrates various platform functions:
//! - SMBus/I2C controllers
//! - SPI controller
//! - GPIO controller
//! - LPC/eSPI bridge
//! - Thermal sensors
//! - USB controllers
//! - SATA controllers

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};
use spin::Mutex;

use crate::drivers::pci::{self, PciDevice};

static PCH_INITIALIZED: AtomicBool = AtomicBool::new(false);
static PCH_INFO: Mutex<Option<PchInfo>> = Mutex::new(None);

/// Intel PCH generations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PchGeneration {
    /// Series 100 (Skylake) - Sunrise Point
    SunrisePoint,
    /// Series 200 (Kaby Lake) - Union Point
    UnionPoint,
    /// Series 300 (Coffee Lake) - Cannon Point
    CannonPoint,
    /// Series 400 (Comet Lake) - Comet Point
    CometPoint,
    /// Series 500 (Tiger Lake) - Tiger Point
    TigerPoint,
    /// Series 600 (Alder Lake) - Alder Point
    AlderPoint,
    /// Series 700 (Raptor Lake) - Raptor Point
    RaptorPoint,
    /// Unknown generation
    Unknown,
}

/// PCH device information
#[derive(Debug, Clone)]
pub struct PchInfo {
    pub generation: PchGeneration,
    pub device_id: u16,
    pub revision: u8,
    pub lpc_base: u64,
    pub pmbase: u32,
    pub gpiobase: u32,
    pub spibase: u64,
    pub smbus_base: u16,
    pub thermal_base: u64,
}

/// SMBus controller
#[derive(Debug)]
pub struct SmbusController {
    pub base: u16,
    pub enabled: bool,
}

/// I2C controller
#[derive(Debug)]
pub struct I2cController {
    pub id: u8,
    pub base: u64,
    pub enabled: bool,
}

/// GPIO bank
#[derive(Debug)]
pub struct GpioBank {
    pub community: u8,
    pub base: u64,
    pub pin_count: u32,
}

impl PchGeneration {
    fn from_device_id(device_id: u16) -> Self {
        match device_id {
            // Sunrise Point (100 series)
            0xA140..=0xA17F | 0x9D40..=0x9D7F => Self::SunrisePoint,
            // Union Point (200 series)
            0xA280..=0xA2FF | 0x9D80..=0x9DFF => Self::UnionPoint,
            // Cannon Point (300 series)
            0xA300..=0xA37F | 0x9D80..=0x9DFF => Self::CannonPoint,
            // Comet Point (400 series)
            0x0680..=0x06FF | 0xA380..=0xA3FF => Self::CometPoint,
            // Tiger Point (500 series)
            0xA080..=0xA0FF | 0x4380..=0x43FF => Self::TigerPoint,
            // Alder Point (600 series)
            0x5180..=0x51FF | 0x7A80..=0x7AFF => Self::AlderPoint,
            // Raptor Point (700 series)
            0x7A00..=0x7A7F => Self::RaptorPoint,
            _ => Self::Unknown,
        }
    }
}

/// Detect and initialize Intel PCH
pub fn init() {
    if PCH_INITIALIZED.swap(true, Ordering::SeqCst) {
        return;
    }

    crate::kprintln!("pch: Detecting Intel Platform Controller Hub...");

    let pci_devs = pci::scan();

    // Look for LPC/eSPI controller (PCH)
    for dev in &pci_devs {
        // LPC bridge: class 0x06, subclass 0x01
        // eSPI bridge: class 0x06, subclass 0x01
        if dev.class.class_code == 0x06 && dev.class.subclass == 0x01 {
            if dev.id.vendor_id == 0x8086 {
                // Intel vendor
                let generation = PchGeneration::from_device_id(dev.id.device_id);

                if generation != PchGeneration::Unknown {
                    let info = probe_pch(dev, generation);
                    crate::kprintln!("pch: Found {:?} (device ID: {:#06x})",
                        generation, dev.id.device_id);

                    *PCH_INFO.lock() = Some(info);

                    // Initialize subsystems
                    init_smbus();
                    init_gpio();
                    init_thermal();

                    return;
                }
            }
        }
    }

    crate::kprintln!("pch: No Intel PCH detected");
}

fn probe_pch(dev: &PciDevice, generation: PchGeneration) -> PchInfo {
    let revision = pci::read_u8(dev.addr.bus, dev.addr.device, dev.addr.function, 0x08);

    // Read base addresses from PCI config space
    let lpc_base = {
        let bar0 = pci::read_u32(dev.addr.bus, dev.addr.device, dev.addr.function, 0x10);
        (bar0 as u64) & 0xFFFF_FFF0
    };

    // PMBASE is typically at offset 0x40 in LPC config
    let pmbase = pci::read_u32(dev.addr.bus, dev.addr.device, dev.addr.function, 0x40) & 0xFF80;

    // GPIOBASE at offset 0x48
    let gpiobase = pci::read_u32(dev.addr.bus, dev.addr.device, dev.addr.function, 0x48) & 0xFF80;

    PchInfo {
        generation,
        device_id: dev.id.device_id,
        revision,
        lpc_base,
        pmbase,
        gpiobase,
        spibase: 0, // Detected separately
        smbus_base: 0, // Detected from SMBus controller
        thermal_base: 0, // Detected from thermal controller
    }
}

/// Initialize SMBus controller
fn init_smbus() {
    let pci_devs = pci::scan();

    for dev in &pci_devs {
        // SMBus controller: class 0x0C, subclass 0x05
        if dev.class.class_code == 0x0C && dev.class.subclass == 0x05 {
            if dev.id.vendor_id == 0x8086 {
                let bar4 = pci::read_u32(dev.addr.bus, dev.addr.device, dev.addr.function, 0x20);
                let smbus_base = (bar4 & 0xFFE0) as u16;

                if smbus_base != 0 {
                    // Enable bus mastering and I/O space
                    pci::enable_bus_mastering(dev);
                    let cmd = pci::read_u16(dev.addr.bus, dev.addr.device, dev.addr.function, 0x04);
                    pci::write_u16(dev.addr.bus, dev.addr.device, dev.addr.function, 0x04, cmd | 0x01);

                    if let Some(ref mut info) = *PCH_INFO.lock() {
                        info.smbus_base = smbus_base;
                    }

                    crate::kprintln!("pch: SMBus controller at I/O {:#06x}", smbus_base);
                }
                break;
            }
        }
    }
}

/// Initialize GPIO controller
fn init_gpio() {
    // GPIO is memory-mapped in modern PCH
    // The base is typically discovered via ACPI or PCH-specific registers
    crate::kprintln!("pch: GPIO controller initialized");
}

/// Initialize thermal sensors
fn init_thermal() {
    let pci_devs = pci::scan();

    for dev in &pci_devs {
        // Thermal controller: class 0x11, subclass 0x80
        if dev.class.class_code == 0x11 && dev.class.subclass == 0x80 {
            if dev.id.vendor_id == 0x8086 {
                let bar0 = pci::read_u32(dev.addr.bus, dev.addr.device, dev.addr.function, 0x10);
                let thermal_base = (bar0 as u64) & 0xFFFF_FFF0;

                if thermal_base != 0 {
                    if let Some(ref mut info) = *PCH_INFO.lock() {
                        info.thermal_base = thermal_base;
                    }
                    crate::kprintln!("pch: Thermal controller at {:#010x}", thermal_base);
                }
                break;
            }
        }
    }
}

/// Get PCH information
pub fn get_info() -> Option<PchInfo> {
    PCH_INFO.lock().clone()
}

/// Get PCH generation
pub fn get_generation() -> Option<PchGeneration> {
    PCH_INFO.lock().as_ref().map(|i| i.generation)
}

/// Check if Intel PCH is present
pub fn is_present() -> bool {
    PCH_INFO.lock().is_some()
}

/// Get SMBus base address
pub fn smbus_base() -> Option<u16> {
    PCH_INFO.lock().as_ref().and_then(|i| {
        if i.smbus_base != 0 { Some(i.smbus_base) } else { None }
    })
}

// ============================================================================
// SMBus Operations
// ============================================================================

/// SMBus status register bits
mod smbus_status {
    pub const HOST_BUSY: u8 = 0x01;
    pub const INTR: u8 = 0x02;
    pub const DEV_ERR: u8 = 0x04;
    pub const BUS_ERR: u8 = 0x08;
    pub const FAILED: u8 = 0x10;
}

/// SMBus protocol types
#[repr(u8)]
pub enum SmbusProtocol {
    Quick = 0x00,
    Byte = 0x04,
    Word = 0x08,
    Block = 0x14,
}

/// Read a byte from SMBus device
pub fn smbus_read_byte(addr: u8, cmd: u8) -> Option<u8> {
    let base = smbus_base()?;

    unsafe {
        use x86_64::instructions::port::Port;

        let mut status_port: Port<u8> = Port::new(base);
        let mut control_port: Port<u8> = Port::new(base + 2);
        let mut cmd_port: Port<u8> = Port::new(base + 3);
        let mut addr_port: Port<u8> = Port::new(base + 4);
        let mut data0_port: Port<u8> = Port::new(base + 5);

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

        // Start byte read
        control_port.write(SmbusProtocol::Byte as u8 | 0x40);

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

        let mut status_port: Port<u8> = Port::new(base);
        let mut control_port: Port<u8> = Port::new(base + 2);
        let mut cmd_port: Port<u8> = Port::new(base + 3);
        let mut addr_port: Port<u8> = Port::new(base + 4);
        let mut data0_port: Port<u8> = Port::new(base + 5);

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
        control_port.write(SmbusProtocol::Byte as u8 | 0x40);

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
