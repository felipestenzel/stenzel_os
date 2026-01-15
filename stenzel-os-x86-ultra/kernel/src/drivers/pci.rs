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
