//! PCI (legacy config space via 0xCF8/0xCFC).
//!
//! Este módulo faz varredura do barramento PCI e leitura/escrita do config space.
//! É suficiente para inicializar virtio-blk no QEMU e também serve como base para
//! AHCI/NVMe em máquinas reais.

#![allow(dead_code)]

use alloc::vec::Vec;
use x86_64::instructions::port::Port;

#[derive(Debug, Clone, Copy)]
pub struct PciAddress {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct PciId {
    pub vendor_id: u16,
    pub device_id: u16,
}

#[derive(Debug, Clone, Copy)]
pub struct PciClass {
    pub class_code: u8,
    pub subclass: u8,
    pub prog_if: u8,
    pub revision: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct PciDevice {
    pub addr: PciAddress,
    pub id: PciId,
    pub class: PciClass,
    pub header_type: u8,
}

const CONFIG_ADDRESS: u16 = 0xCF8;
const CONFIG_DATA: u16 = 0xCFC;

#[inline]
fn config_addr(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    let bus = bus as u32;
    let device = device as u32;
    let function = function as u32;
    let offset = (offset as u32) & 0xFC;
    (1u32 << 31) | (bus << 16) | (device << 11) | (function << 8) | offset
}

pub fn read_u32(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    unsafe {
        let mut addr = Port::<u32>::new(CONFIG_ADDRESS);
        let mut data = Port::<u32>::new(CONFIG_DATA);
        addr.write(config_addr(bus, device, function, offset));
        data.read()
    }
}

pub fn write_u32(bus: u8, device: u8, function: u8, offset: u8, value: u32) {
    unsafe {
        let mut addr = Port::<u32>::new(CONFIG_ADDRESS);
        let mut data = Port::<u32>::new(CONFIG_DATA);
        addr.write(config_addr(bus, device, function, offset));
        data.write(value);
    }
}

pub fn read_u16(bus: u8, device: u8, function: u8, offset: u8) -> u16 {
    let v = read_u32(bus, device, function, offset & 0xFC);
    let shift = ((offset & 2) * 8) as u32;
    ((v >> shift) & 0xFFFF) as u16
}

pub fn write_u16(bus: u8, device: u8, function: u8, offset: u8, value: u16) {
    let aligned = offset & 0xFC;
    let mut v = read_u32(bus, device, function, aligned);
    let shift = ((offset & 2) * 8) as u32;
    v &= !(0xFFFFu32 << shift);
    v |= (value as u32) << shift;
    write_u32(bus, device, function, aligned, v);
}

pub fn read_u8(bus: u8, device: u8, function: u8, offset: u8) -> u8 {
    let v = read_u32(bus, device, function, offset & 0xFC);
    let shift = ((offset & 3) * 8) as u32;
    ((v >> shift) & 0xFF) as u8
}

pub fn write_u8(bus: u8, device: u8, function: u8, offset: u8, value: u8) {
    let aligned = offset & 0xFC;
    let mut v = read_u32(bus, device, function, aligned);
    let shift = ((offset & 3) * 8) as u32;
    v &= !(0xFFu32 << shift);
    v |= (value as u32) << shift;
    write_u32(bus, device, function, aligned, v);
}

pub fn scan() -> Vec<PciDevice> {
    let mut out = Vec::new();
    for bus in 0u16..=255 {
        let bus = bus as u8;
        for device in 0u8..32 {
            let vendor = read_u16(bus, device, 0, 0x00);
            if vendor == 0xFFFF {
                continue;
            }
            let header_type = read_u8(bus, device, 0, 0x0E);
            let multi = (header_type & 0x80) != 0;
            let functions = if multi { 8 } else { 1 };

            for function in 0u8..functions {
                let vendor = read_u16(bus, device, function, 0x00);
                if vendor == 0xFFFF {
                    continue;
                }
                let device_id = read_u16(bus, device, function, 0x02);

                let revision = read_u8(bus, device, function, 0x08);
                let prog_if = read_u8(bus, device, function, 0x09);
                let subclass = read_u8(bus, device, function, 0x0A);
                let class_code = read_u8(bus, device, function, 0x0B);

                let header_type = read_u8(bus, device, function, 0x0E);

                out.push(PciDevice {
                    addr: PciAddress {
                        bus,
                        device,
                        function,
                    },
                    id: PciId {
                        vendor_id: vendor,
                        device_id,
                    },
                    class: PciClass {
                        class_code,
                        subclass,
                        prog_if,
                        revision,
                    },
                    header_type,
                });
            }
        }
    }
    out
}

/// Lê BAR n (0..5). Retorna (base, is_io).
pub fn read_bar(dev: &PciDevice, bar_index: u8) -> (u64, bool) {
    let offset = 0x10 + bar_index * 4;
    let raw = read_u32(dev.addr.bus, dev.addr.device, dev.addr.function, offset);
    if raw & 0x1 == 0x1 {
        // I/O
        ((raw & 0xFFFF_FFFC) as u64, true)
    } else {
        // MMIO (32-bit); para 64-bit, BAR ocupa dois regs
        ((raw & 0xFFFF_FFF0) as u64, false)
    }
}

pub fn enable_bus_mastering(dev: &PciDevice) {
    // PCI command register @ 0x04
    let cmd = read_u16(dev.addr.bus, dev.addr.device, dev.addr.function, 0x04);
    // bits: 0=IO, 1=MEM, 2=bus master
    let new = cmd | (1 << 0) | (1 << 1) | (1 << 2);
    write_u16(dev.addr.bus, dev.addr.device, dev.addr.function, 0x04, new);
}

// ============================================================================
// PCI Express Support (ECAM - Enhanced Configuration Access Mechanism)
// ============================================================================

use spin::Once;

/// ECAM base address from ACPI MCFG
static ECAM_BASE: Once<u64> = Once::new();

/// Initialize PCIe support by reading ECAM base from ACPI MCFG
pub fn init_pcie() {
    if let Some(mcfg) = crate::drivers::acpi::parse_mcfg() {
        if let Some(entry) = mcfg.entries.first() {
            // McfgEntry is packed, so we need to read fields carefully
            let base = unsafe { core::ptr::read_unaligned(core::ptr::addr_of!(entry.base_address)) };
            let segment = unsafe { core::ptr::read_unaligned(core::ptr::addr_of!(entry.segment_group)) };
            let start_bus = entry.start_bus;
            let end_bus = entry.end_bus;

            ECAM_BASE.call_once(|| base);
            crate::kprintln!(
                "pcie: ECAM base @ {:#x} (segment {}, bus {}-{})",
                base,
                segment,
                start_bus,
                end_bus
            );
        }
    } else {
        crate::kprintln!("pcie: MCFG not found, using legacy PCI config space");
    }
}

/// Check if PCIe ECAM is available
pub fn pcie_available() -> bool {
    ECAM_BASE.get().is_some()
}

/// Get ECAM virtual address for a given bus/device/function
fn ecam_addr(bus: u8, device: u8, function: u8, offset: u16) -> Option<u64> {
    let base = *ECAM_BASE.get()?;

    // ECAM addressing: base + (bus << 20) | (device << 15) | (function << 12) | offset
    // Each function gets 4KB of config space
    let addr = base
        + ((bus as u64) << 20)
        + ((device as u64) << 15)
        + ((function as u64) << 12)
        + (offset as u64);

    // Convert to virtual address
    let phys_offset = crate::mm::physical_memory_offset();
    Some((phys_offset + addr).as_u64())
}

/// Read u32 from PCIe config space using ECAM
pub fn pcie_read_u32(bus: u8, device: u8, function: u8, offset: u16) -> Option<u32> {
    let addr = ecam_addr(bus, device, function, offset & 0xFFC)?;
    unsafe { Some(*(addr as *const u32)) }
}

/// Write u32 to PCIe config space using ECAM
pub fn pcie_write_u32(bus: u8, device: u8, function: u8, offset: u16, value: u32) {
    if let Some(addr) = ecam_addr(bus, device, function, offset & 0xFFC) {
        unsafe {
            *(addr as *mut u32) = value;
        }
    }
}

/// Read u16 from PCIe config space using ECAM
pub fn pcie_read_u16(bus: u8, device: u8, function: u8, offset: u16) -> Option<u16> {
    let addr = ecam_addr(bus, device, function, offset & 0xFFE)?;
    unsafe { Some(*(addr as *const u16)) }
}

/// Write u16 to PCIe config space using ECAM
pub fn pcie_write_u16(bus: u8, device: u8, function: u8, offset: u16, value: u16) {
    if let Some(addr) = ecam_addr(bus, device, function, offset & 0xFFE) {
        unsafe {
            *(addr as *mut u16) = value;
        }
    }
}

/// Read u8 from PCIe config space using ECAM
pub fn pcie_read_u8(bus: u8, device: u8, function: u8, offset: u16) -> Option<u8> {
    let addr = ecam_addr(bus, device, function, offset)?;
    unsafe { Some(*(addr as *const u8)) }
}

/// Write u8 to PCIe config space using ECAM
pub fn pcie_write_u8(bus: u8, device: u8, function: u8, offset: u16, value: u8) {
    if let Some(addr) = ecam_addr(bus, device, function, offset) {
        unsafe {
            *(addr as *mut u8) = value;
        }
    }
}

/// Read u32 from extended config space (works with both PCIe and legacy PCI)
/// Offset can be 0-4095 for PCIe, 0-255 for legacy PCI
pub fn config_read_u32(bus: u8, device: u8, function: u8, offset: u16) -> u32 {
    if pcie_available() && offset <= 4092 {
        pcie_read_u32(bus, device, function, offset).unwrap_or(0xFFFFFFFF)
    } else if offset <= 252 {
        read_u32(bus, device, function, offset as u8)
    } else {
        0xFFFFFFFF
    }
}

/// Write u32 to extended config space
pub fn config_write_u32(bus: u8, device: u8, function: u8, offset: u16, value: u32) {
    if pcie_available() && offset <= 4092 {
        pcie_write_u32(bus, device, function, offset, value);
    } else if offset <= 252 {
        write_u32(bus, device, function, offset as u8, value);
    }
}

/// Read u16 from extended config space
pub fn config_read_u16(bus: u8, device: u8, function: u8, offset: u16) -> u16 {
    if pcie_available() && offset <= 4094 {
        pcie_read_u16(bus, device, function, offset).unwrap_or(0xFFFF)
    } else if offset <= 254 {
        read_u16(bus, device, function, offset as u8)
    } else {
        0xFFFF
    }
}

/// Write u16 to extended config space
pub fn config_write_u16(bus: u8, device: u8, function: u8, offset: u16, value: u16) {
    if pcie_available() && offset <= 4094 {
        pcie_write_u16(bus, device, function, offset, value);
    } else if offset <= 254 {
        write_u16(bus, device, function, offset as u8, value);
    }
}

/// Read u8 from extended config space
pub fn config_read_u8(bus: u8, device: u8, function: u8, offset: u16) -> u8 {
    if pcie_available() && offset <= 4095 {
        pcie_read_u8(bus, device, function, offset).unwrap_or(0xFF)
    } else if offset <= 255 {
        read_u8(bus, device, function, offset as u8)
    } else {
        0xFF
    }
}

/// Write u8 to extended config space
pub fn config_write_u8(bus: u8, device: u8, function: u8, offset: u16, value: u8) {
    if pcie_available() && offset <= 4095 {
        pcie_write_u8(bus, device, function, offset, value);
    } else if offset <= 255 {
        write_u8(bus, device, function, offset as u8, value);
    }
}

// ============================================================================
// PCIe Capabilities
// ============================================================================

/// PCIe capability ID
pub const PCI_CAP_ID_PCIE: u8 = 0x10;
pub const PCI_CAP_ID_MSI: u8 = 0x05;
pub const PCI_CAP_ID_MSIX: u8 = 0x11;
pub const PCI_CAP_ID_PM: u8 = 0x01;
pub const PCI_CAP_ID_VENDOR: u8 = 0x09;

/// Find a capability in the PCI capability list
pub fn find_capability(dev: &PciDevice, cap_id: u8) -> Option<u8> {
    // Check if capabilities are supported (bit 4 of status register)
    let status = read_u16(dev.addr.bus, dev.addr.device, dev.addr.function, 0x06);
    if status & (1 << 4) == 0 {
        return None;
    }

    // Capability pointer is at offset 0x34
    let mut ptr = read_u8(dev.addr.bus, dev.addr.device, dev.addr.function, 0x34);

    // Walk the capability list
    while ptr != 0 {
        let id = read_u8(dev.addr.bus, dev.addr.device, dev.addr.function, ptr);
        if id == cap_id {
            return Some(ptr);
        }
        // Next capability pointer
        ptr = read_u8(dev.addr.bus, dev.addr.device, dev.addr.function, ptr + 1);
    }

    None
}

/// Check if a device is a PCIe device
pub fn is_pcie_device(dev: &PciDevice) -> bool {
    find_capability(dev, PCI_CAP_ID_PCIE).is_some()
}

/// PCIe device type (from PCIe capability)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcieDeviceType {
    Endpoint,
    LegacyEndpoint,
    RootPort,
    UpstreamPort,
    DownstreamPort,
    PcieToPciBridge,
    PciToPcieBridge,
    RootComplexIntegratedEndpoint,
    RootComplexEventCollector,
    Unknown(u8),
}

/// Get PCIe device type
pub fn pcie_device_type(dev: &PciDevice) -> Option<PcieDeviceType> {
    let cap_offset = find_capability(dev, PCI_CAP_ID_PCIE)?;

    // PCIe capabilities register is at cap_offset + 2
    let caps = read_u16(dev.addr.bus, dev.addr.device, dev.addr.function, cap_offset + 2);
    let device_type = ((caps >> 4) & 0xF) as u8;

    Some(match device_type {
        0 => PcieDeviceType::Endpoint,
        1 => PcieDeviceType::LegacyEndpoint,
        4 => PcieDeviceType::RootPort,
        5 => PcieDeviceType::UpstreamPort,
        6 => PcieDeviceType::DownstreamPort,
        7 => PcieDeviceType::PcieToPciBridge,
        8 => PcieDeviceType::PciToPcieBridge,
        9 => PcieDeviceType::RootComplexIntegratedEndpoint,
        10 => PcieDeviceType::RootComplexEventCollector,
        t => PcieDeviceType::Unknown(t),
    })
}

/// PCIe link status
#[derive(Debug, Clone, Copy)]
pub struct PcieLinkStatus {
    pub speed: u8,       // 1=2.5GT/s, 2=5GT/s, 3=8GT/s, 4=16GT/s, 5=32GT/s
    pub width: u8,       // x1, x2, x4, x8, x16, x32
    pub training: bool,
    pub slot_clock: bool,
}

/// Get PCIe link status
pub fn pcie_link_status(dev: &PciDevice) -> Option<PcieLinkStatus> {
    let cap_offset = find_capability(dev, PCI_CAP_ID_PCIE)?;

    // Link Status register is at cap_offset + 0x12
    let status = read_u16(dev.addr.bus, dev.addr.device, dev.addr.function, cap_offset + 0x12);

    Some(PcieLinkStatus {
        speed: (status & 0xF) as u8,
        width: ((status >> 4) & 0x3F) as u8,
        training: (status & (1 << 11)) != 0,
        slot_clock: (status & (1 << 12)) != 0,
    })
}

/// Print PCIe device info
pub fn print_pcie_info(dev: &PciDevice) {
    if !is_pcie_device(dev) {
        crate::kprintln!("  PCI device (legacy)");
        return;
    }

    let dev_type = pcie_device_type(dev);
    let link = pcie_link_status(dev);

    crate::kprint!("  PCIe ");

    if let Some(t) = dev_type {
        crate::kprint!("{:?}", t);
    }

    if let Some(l) = link {
        let speed_str = match l.speed {
            1 => "2.5GT/s",
            2 => "5GT/s",
            3 => "8GT/s",
            4 => "16GT/s",
            5 => "32GT/s",
            _ => "?",
        };
        crate::kprint!(" (x{} @ {})", l.width, speed_str);
    }

    crate::kprintln!();
}

/// Scan PCIe bus and return extended device info
pub fn scan_pcie() -> Vec<PciDevice> {
    let mut devices = scan();

    // Initialize PCIe ECAM if available
    init_pcie();

    // Log PCIe info for each device
    for dev in &devices {
        crate::kprint!(
            "{:02x}:{:02x}.{} [{:04x}:{:04x}] class {:02x}{:02x}: ",
            dev.addr.bus,
            dev.addr.device,
            dev.addr.function,
            dev.id.vendor_id,
            dev.id.device_id,
            dev.class.class_code,
            dev.class.subclass
        );
        print_pcie_info(dev);
    }

    devices
}
