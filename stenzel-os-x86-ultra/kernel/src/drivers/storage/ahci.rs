//! AHCI (Advanced Host Controller Interface) driver for SATA devices.
//!
//! AHCI é a interface padrão para drives SATA em PCs.

#![allow(dead_code)]
//!
//! Arquitetura:
//! - ABAR (BAR5) contém os registradores do HBA (MMIO)
//! - Cada porta tem Command List, Received FIS, e Command Tables
//! - Comandos são enviados via Command Slots
//!
//! Referências:
//! - AHCI 1.3.1 Specification
//! - https://wiki.osdev.org/AHCI

extern crate alloc;

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::ptr::{read_volatile, write_volatile};

use spin::Mutex;
use x86_64::VirtAddr;

use crate::drivers::pci::{self, PciDevice};
use crate::mm;
use crate::storage::{BlockDevice, BlockDeviceId};
use crate::util::{KError, KResult};

// ============================================================================
// Constantes AHCI
// ============================================================================

const AHCI_CLASS: u8 = 0x01;
const AHCI_SUBCLASS: u8 = 0x06;

// HBA Memory Registers offsets
const HBA_CAP: usize = 0x00;       // Host Capabilities
const HBA_GHC: usize = 0x04;       // Global Host Control
const HBA_IS: usize = 0x08;        // Interrupt Status
const HBA_PI: usize = 0x0C;        // Ports Implemented
const HBA_VS: usize = 0x10;        // Version
const HBA_PORT_BASE: usize = 0x100;
const HBA_PORT_SIZE: usize = 0x80;

// Port registers offsets (relative to port base)
const PORT_CLB: usize = 0x00;      // Command List Base Address
const PORT_CLBU: usize = 0x04;     // Command List Base Address Upper
const PORT_FB: usize = 0x08;       // FIS Base Address
const PORT_FBU: usize = 0x0C;      // FIS Base Address Upper
const PORT_IS: usize = 0x10;       // Interrupt Status
const PORT_IE: usize = 0x14;       // Interrupt Enable
const PORT_CMD: usize = 0x18;      // Command and Status
const PORT_TFD: usize = 0x20;      // Task File Data
const PORT_SIG: usize = 0x24;      // Signature
const PORT_SSTS: usize = 0x28;     // SATA Status
const PORT_SCTL: usize = 0x2C;     // SATA Control
const PORT_SERR: usize = 0x30;     // SATA Error
const PORT_SACT: usize = 0x34;     // SATA Active
const PORT_CI: usize = 0x38;       // Command Issue

// GHC bits
const GHC_AE: u32 = 1 << 31;       // AHCI Enable
const GHC_HR: u32 = 1 << 0;        // HBA Reset

// Port CMD bits
const PORT_CMD_ST: u32 = 1 << 0;   // Start
const PORT_CMD_FRE: u32 = 1 << 4;  // FIS Receive Enable
const PORT_CMD_FR: u32 = 1 << 14;  // FIS Receive Running
const PORT_CMD_CR: u32 = 1 << 15;  // Command List Running

// Port SSTS bits
const SSTS_DET_MASK: u32 = 0xF;
const SSTS_DET_PRESENT: u32 = 3;

// Device signatures
const SATA_SIG_ATA: u32 = 0x00000101;
const SATA_SIG_ATAPI: u32 = 0xEB140101;

// FIS types
const FIS_TYPE_REG_H2D: u8 = 0x27;

// ATA commands
const ATA_CMD_READ_DMA_EXT: u8 = 0x25;
const ATA_CMD_WRITE_DMA_EXT: u8 = 0x35;
const ATA_CMD_IDENTIFY: u8 = 0xEC;

// ============================================================================
// Estruturas AHCI
// ============================================================================

#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
struct HbaCommandHeader {
    // DW0
    flags: u16,         // CFL, A, W, P, R, B, C, PMPort
    prdtl: u16,         // PRDT Length
    // DW1
    prdbc: u32,         // PRD Byte Count
    // DW2-3
    ctba: u64,          // Command Table Base Address
    // DW4-7
    _reserved: [u32; 4],
}

#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
struct HbaPrdtEntry {
    dba: u64,           // Data Base Address
    _reserved: u32,
    dbc: u32,           // Data Byte Count (bit 31 = interrupt on completion)
}

#[repr(C, packed)]
struct HbaCommandTable {
    cfis: [u8; 64],     // Command FIS
    acmd: [u8; 16],     // ATAPI Command
    _reserved: [u8; 48],
    prdt: [HbaPrdtEntry; 8], // PRD Table entries
}

#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
struct FisRegH2D {
    fis_type: u8,
    flags: u8,          // PM Port, C bit
    command: u8,
    feature_lo: u8,
    lba0: u8,
    lba1: u8,
    lba2: u8,
    device: u8,
    lba3: u8,
    lba4: u8,
    lba5: u8,
    feature_hi: u8,
    count_lo: u8,
    count_hi: u8,
    icc: u8,
    control: u8,
    _reserved: [u8; 4],
}

// ============================================================================
// Estruturas do driver
// ============================================================================

struct AhciPort {
    port_num: u8,
    hba_base: *mut u8,
    cmd_list: Box<[HbaCommandHeader; 32]>,
    cmd_list_phys: u64,
    fis_base: Box<[u8; 256]>,
    fis_phys: u64,
    cmd_tables: Box<[Box<HbaCommandTable>; 32]>,
    block_count: u64,
    is_atapi: bool,
}

unsafe impl Send for AhciPort {}

impl AhciPort {
    fn port_base(&self) -> *mut u8 {
        unsafe { self.hba_base.add(HBA_PORT_BASE + self.port_num as usize * HBA_PORT_SIZE) }
    }

    fn read_reg(&self, offset: usize) -> u32 {
        unsafe { read_volatile(self.port_base().add(offset) as *const u32) }
    }

    fn write_reg(&self, offset: usize, value: u32) {
        unsafe { write_volatile(self.port_base().add(offset) as *mut u32, value) }
    }

    fn stop(&self) {
        let cmd = self.read_reg(PORT_CMD);
        self.write_reg(PORT_CMD, cmd & !(PORT_CMD_ST | PORT_CMD_FRE));

        // Espera parar
        let mut timeout = 1_000_000;
        while timeout > 0 {
            let cmd = self.read_reg(PORT_CMD);
            if (cmd & (PORT_CMD_FR | PORT_CMD_CR)) == 0 {
                break;
            }
            core::hint::spin_loop();
            timeout -= 1;
        }
    }

    fn start(&self) {
        // Espera até que não esteja rodando
        let mut timeout = 1_000_000;
        while timeout > 0 {
            let cmd = self.read_reg(PORT_CMD);
            if (cmd & PORT_CMD_CR) == 0 {
                break;
            }
            core::hint::spin_loop();
            timeout -= 1;
        }

        let cmd = self.read_reg(PORT_CMD);
        self.write_reg(PORT_CMD, cmd | PORT_CMD_FRE | PORT_CMD_ST);
    }

    fn wait_slot_ready(&self, slot: usize) -> KResult<()> {
        let mut timeout = 10_000_000;
        while timeout > 0 {
            let ci = self.read_reg(PORT_CI);
            if (ci & (1 << slot)) == 0 {
                return Ok(());
            }

            // Verifica erro
            let tfd = self.read_reg(PORT_TFD);
            if (tfd & 0x01) != 0 { // ERR bit
                return Err(KError::IO);
            }

            core::hint::spin_loop();
            timeout -= 1;
        }
        Err(KError::Timeout)
    }

    fn issue_command(&mut self, slot: usize, fis: &FisRegH2D, prdt: &[(u64, u32)]) -> KResult<()> {
        // Configura Command Header
        let flags = (core::mem::size_of::<FisRegH2D>() / 4) as u16; // CFL
        let flags = flags | (if prdt.iter().any(|(_, _)| true) { 0 } else { 0 }); // W bit se write

        self.cmd_list[slot].flags = flags;
        self.cmd_list[slot].prdtl = prdt.len() as u16;
        self.cmd_list[slot].prdbc = 0;

        let table = &mut *self.cmd_tables[slot];
        let table_phys = virt_to_phys(table as *const _ as u64);
        self.cmd_list[slot].ctba = table_phys;

        // Copia FIS para Command Table
        unsafe {
            core::ptr::copy_nonoverlapping(
                fis as *const FisRegH2D as *const u8,
                table.cfis.as_mut_ptr(),
                core::mem::size_of::<FisRegH2D>(),
            );
        }

        // Configura PRDT
        for (i, (addr, size)) in prdt.iter().enumerate() {
            if i >= 8 {
                break;
            }
            table.prdt[i].dba = *addr;
            table.prdt[i].dbc = (*size - 1) | (1 << 31); // Byte count - 1, IOC bit
        }

        // Limpa interrupções pendentes
        self.write_reg(PORT_IS, u32::MAX);

        // Issue command
        self.write_reg(PORT_CI, 1 << slot);

        // Espera conclusão
        self.wait_slot_ready(slot)
    }

    fn identify(&mut self) -> KResult<(u64, bool)> {
        let buf = vec![0u8; 512].into_boxed_slice();
        let buf_phys = virt_to_phys(buf.as_ptr() as u64);

        let fis = FisRegH2D {
            fis_type: FIS_TYPE_REG_H2D,
            flags: 0x80, // Command bit
            command: ATA_CMD_IDENTIFY,
            device: 0,
            ..Default::default()
        };

        self.issue_command(0, &fis, &[(buf_phys, 512)])?;

        // Parse identify data
        let sectors = u64::from_le_bytes([
            buf[200], buf[201], buf[202], buf[203],
            buf[204], buf[205], buf[206], buf[207],
        ]);

        // Model name (words 27-46)
        let model: Vec<u8> = buf[54..94]
            .chunks(2)
            .flat_map(|c| [c[1], c[0]])
            .take_while(|&c| c != 0)
            .collect();
        let model_str = core::str::from_utf8(&model).unwrap_or("Unknown").trim();

        crate::kprintln!("ahci: disk: {} ({} sectors)", model_str, sectors);

        Ok((sectors, false))
    }
}

pub struct AhciController {
    pci: PciDevice,
    hba_base: *mut u8,
    ports: Mutex<Vec<AhciPort>>,
    id: BlockDeviceId,
}

unsafe impl Send for AhciController {}
unsafe impl Sync for AhciController {}

fn virt_to_phys(va: u64) -> u64 {
    mm::virt_to_phys(VirtAddr::new(va))
        .map(|pa| pa.as_u64())
        .unwrap_or(0)
}

impl AhciController {
    fn read_hba(&self, offset: usize) -> u32 {
        unsafe { read_volatile(self.hba_base.add(offset) as *const u32) }
    }

    fn write_hba(&self, offset: usize, value: u32) {
        unsafe { write_volatile(self.hba_base.add(offset) as *mut u32, value) }
    }
}

impl BlockDevice for AhciController {
    fn id(&self) -> BlockDeviceId {
        self.id
    }

    fn block_size(&self) -> u32 {
        512
    }

    fn num_blocks(&self) -> u64 {
        let ports = self.ports.lock();
        ports.first().map(|p| p.block_count).unwrap_or(0)
    }

    fn read_blocks(&self, lba: u64, count: u32, out: &mut [u8]) -> KResult<()> {
        if count == 0 {
            return Ok(());
        }

        let mut ports = self.ports.lock();
        let port = ports.first_mut().ok_or(KError::NotFound)?;

        let buf_phys = virt_to_phys(out.as_ptr() as u64);

        let fis = FisRegH2D {
            fis_type: FIS_TYPE_REG_H2D,
            flags: 0x80,
            command: ATA_CMD_READ_DMA_EXT,
            device: 0x40, // LBA mode
            lba0: (lba & 0xFF) as u8,
            lba1: ((lba >> 8) & 0xFF) as u8,
            lba2: ((lba >> 16) & 0xFF) as u8,
            lba3: ((lba >> 24) & 0xFF) as u8,
            lba4: ((lba >> 32) & 0xFF) as u8,
            lba5: ((lba >> 40) & 0xFF) as u8,
            count_lo: (count & 0xFF) as u8,
            count_hi: ((count >> 8) & 0xFF) as u8,
            ..Default::default()
        };

        let size = count as u32 * 512;
        port.issue_command(0, &fis, &[(buf_phys, size)])
    }

    fn write_blocks(&self, lba: u64, count: u32, data: &[u8]) -> KResult<()> {
        if count == 0 {
            return Ok(());
        }

        let mut ports = self.ports.lock();
        let port = ports.first_mut().ok_or(KError::NotFound)?;

        let buf_phys = virt_to_phys(data.as_ptr() as u64);

        let fis = FisRegH2D {
            fis_type: FIS_TYPE_REG_H2D,
            flags: 0x80,
            command: ATA_CMD_WRITE_DMA_EXT,
            device: 0x40,
            lba0: (lba & 0xFF) as u8,
            lba1: ((lba >> 8) & 0xFF) as u8,
            lba2: ((lba >> 16) & 0xFF) as u8,
            lba3: ((lba >> 24) & 0xFF) as u8,
            lba4: ((lba >> 32) & 0xFF) as u8,
            lba5: ((lba >> 40) & 0xFF) as u8,
            count_lo: (count & 0xFF) as u8,
            count_hi: ((count >> 8) & 0xFF) as u8,
            ..Default::default()
        };

        // Marca como write no command header
        let size = count as u32 * 512;
        port.cmd_list[0].flags |= 0x40; // W bit
        port.issue_command(0, &fis, &[(buf_phys, size)])
    }
}

// ============================================================================
// Funções públicas
// ============================================================================

pub fn probe(dev: &PciDevice) -> Option<Arc<AhciController>> {
    if dev.class.class_code != AHCI_CLASS || dev.class.subclass != AHCI_SUBCLASS {
        return None;
    }

    crate::kprintln!("ahci: encontrado controller PCI {:02x}:{:02x}.{}",
        dev.addr.bus, dev.addr.device, dev.addr.function);

    // Lê BAR5 (ABAR)
    let (bar5, is_io) = pci::read_bar(dev, 5);
    if is_io || bar5 == 0 {
        crate::kprintln!("ahci: BAR5 inválido");
        return None;
    }

    // Habilita bus mastering
    pci::enable_bus_mastering(dev);

    // Mapeia MMIO
    let hba_base = mm::phys_to_virt(x86_64::PhysAddr::new(bar5)).as_mut_ptr::<u8>();

    Some(Arc::new(AhciController {
        pci: *dev,
        hba_base,
        ports: Mutex::new(Vec::new()),
        id: BlockDeviceId(200),
    }))
}

pub fn init(ctrl: Arc<AhciController>) -> KResult<Arc<AhciController>> {
    // Habilita AHCI mode
    let ghc = ctrl.read_hba(HBA_GHC);
    ctrl.write_hba(HBA_GHC, ghc | GHC_AE);

    // Lê capabilities
    let cap = ctrl.read_hba(HBA_CAP);
    let num_ports = ((cap & 0x1F) + 1) as u8;
    let num_slots = (((cap >> 8) & 0x1F) + 1) as u8;
    crate::kprintln!("ahci: {} ports, {} command slots", num_ports, num_slots);

    // Verifica quais ports estão implementados
    let pi = ctrl.read_hba(HBA_PI);
    crate::kprintln!("ahci: ports implemented: {:#010x}", pi);

    let mut ports = Vec::new();

    for port_num in 0..32u8 {
        if (pi & (1 << port_num)) == 0 {
            continue;
        }

        let port_base = unsafe { ctrl.hba_base.add(HBA_PORT_BASE + port_num as usize * HBA_PORT_SIZE) };

        // Verifica se há dispositivo conectado
        let ssts = unsafe { read_volatile(port_base.add(PORT_SSTS) as *const u32) };
        let det = ssts & SSTS_DET_MASK;
        if det != SSTS_DET_PRESENT {
            continue;
        }

        // Verifica signature
        let sig = unsafe { read_volatile(port_base.add(PORT_SIG) as *const u32) };
        let is_atapi = sig == SATA_SIG_ATAPI;

        crate::kprintln!("ahci: port {} presente (sig={:#010x}{})",
            port_num, sig, if is_atapi { " ATAPI" } else { "" });

        // Aloca estruturas do port
        let cmd_list = Box::new([HbaCommandHeader::default(); 32]);
        let cmd_list_phys = virt_to_phys(cmd_list.as_ptr() as u64);

        let fis_base = Box::new([0u8; 256]);
        let fis_phys = virt_to_phys(fis_base.as_ptr() as u64);

        // Aloca Command Tables
        let cmd_tables: [Box<HbaCommandTable>; 32] = core::array::from_fn(|_| {
            Box::new(HbaCommandTable {
                cfis: [0; 64],
                acmd: [0; 16],
                _reserved: [0; 48],
                prdt: [HbaPrdtEntry::default(); 8],
            })
        });

        let mut port = AhciPort {
            port_num,
            hba_base: ctrl.hba_base,
            cmd_list,
            cmd_list_phys,
            fis_base,
            fis_phys,
            cmd_tables: Box::new(cmd_tables),
            block_count: 0,
            is_atapi,
        };

        // Para a porta
        port.stop();

        // Configura endereços
        port.write_reg(PORT_CLB, port.cmd_list_phys as u32);
        port.write_reg(PORT_CLBU, (port.cmd_list_phys >> 32) as u32);
        port.write_reg(PORT_FB, port.fis_phys as u32);
        port.write_reg(PORT_FBU, (port.fis_phys >> 32) as u32);

        // Limpa erros
        port.write_reg(PORT_SERR, u32::MAX);
        port.write_reg(PORT_IS, u32::MAX);

        // Inicia a porta
        port.start();

        // Identifica o dispositivo
        if !is_atapi {
            match port.identify() {
                Ok((sectors, _)) => {
                    port.block_count = sectors;
                    ports.push(port);
                }
                Err(e) => {
                    crate::kprintln!("ahci: port {} identify falhou: {:?}", port_num, e);
                }
            }
        }
    }

    if ports.is_empty() {
        crate::kprintln!("ahci: nenhum dispositivo SATA encontrado");
        return Err(KError::NotFound);
    }

    *ctrl.ports.lock() = ports;
    crate::kprintln!("ahci: driver inicializado");

    Ok(ctrl)
}
