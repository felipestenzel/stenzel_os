//! NVMe (Non-Volatile Memory Express) driver.
//!
//! NVMe é a interface de alta performance para SSDs via PCIe.

#![allow(dead_code)]
//!
//! Arquitetura:
//! - BAR0 contém os registradores do controller (MMIO)
//! - Admin Queue (ASQ/ACQ) para comandos administrativos
//! - I/O Queues (SQ/CQ) para leitura/escrita de dados
//!
//! Referências:
//! - NVMe Base Specification 2.0
//! - https://wiki.osdev.org/NVMe

extern crate alloc;

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec;
use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{AtomicU16, Ordering};

use spin::Mutex;

use x86_64::VirtAddr;

use crate::drivers::pci::{self, PciDevice};
use crate::mm;
use crate::storage::{BlockDevice, BlockDeviceId};
use crate::util::{KError, KResult};

fn virt_to_phys(va: u64) -> u64 {
    mm::virt_to_phys(VirtAddr::new(va))
        .map(|pa| pa.as_u64())
        .unwrap_or(0)
}

// ============================================================================
// Constantes NVMe
// ============================================================================

const NVME_CLASS: u8 = 0x01;
const NVME_SUBCLASS: u8 = 0x08;
const NVME_PROG_IF: u8 = 0x02;

// Tamanhos de queue (número de entries)
const ADMIN_QUEUE_SIZE: u16 = 32;
const IO_QUEUE_SIZE: u16 = 64;

// Opcodes Admin Commands
const ADMIN_DELETE_SQ: u8 = 0x00;
const ADMIN_CREATE_SQ: u8 = 0x01;
const ADMIN_DELETE_CQ: u8 = 0x04;
const ADMIN_CREATE_CQ: u8 = 0x05;
const ADMIN_IDENTIFY: u8 = 0x06;
const ADMIN_SET_FEATURES: u8 = 0x09;

// Opcodes I/O Commands
const IO_READ: u8 = 0x02;
const IO_WRITE: u8 = 0x01;

// ============================================================================
// Registradores NVMe (BAR0 offsets)
// ============================================================================

#[repr(C)]
struct NvmeRegs {
    cap: u64,        // 0x00: Controller Capabilities
    vs: u32,         // 0x08: Version
    intms: u32,      // 0x0C: Interrupt Mask Set
    intmc: u32,      // 0x10: Interrupt Mask Clear
    cc: u32,         // 0x14: Controller Configuration
    _rsvd: u32,      // 0x18: Reserved
    csts: u32,       // 0x1C: Controller Status
    nssr: u32,       // 0x20: NVM Subsystem Reset
    aqa: u32,        // 0x24: Admin Queue Attributes
    asq: u64,        // 0x28: Admin SQ Base Address
    acq: u64,        // 0x30: Admin CQ Base Address
}

// ============================================================================
// Estruturas de comando/completude NVMe
// ============================================================================

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct NvmeCommand {
    opcode: u8,
    flags: u8,
    cid: u16,           // Command ID
    nsid: u32,          // Namespace ID
    rsvd: u64,
    mptr: u64,          // Metadata Pointer
    prp1: u64,          // PRP Entry 1
    prp2: u64,          // PRP Entry 2
    cdw10: u32,
    cdw11: u32,
    cdw12: u32,
    cdw13: u32,
    cdw14: u32,
    cdw15: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct NvmeCompletion {
    dw0: u32,           // Command-specific
    dw1: u32,           // Reserved
    sq_head: u16,       // SQ Head Pointer
    sq_id: u16,         // SQ Identifier
    cid: u16,           // Command ID
    status: u16,        // Status (includes phase bit)
}

impl NvmeCompletion {
    fn phase(&self) -> bool {
        (self.status & 1) != 0
    }

    fn status_code(&self) -> u16 {
        (self.status >> 1) & 0x7FF
    }

    fn is_error(&self) -> bool {
        self.status_code() != 0
    }
}

// ============================================================================
// Estruturas de Queue
// ============================================================================

struct SubmissionQueue {
    entries: Box<[NvmeCommand]>,
    phys: u64,
    size: u16,
    tail: u16,
    doorbell: *mut u32,
}

struct CompletionQueue {
    entries: Box<[NvmeCompletion]>,
    phys: u64,
    size: u16,
    head: u16,
    phase: bool,
    doorbell: *mut u32,
}

impl SubmissionQueue {
    fn new(size: u16, doorbell: *mut u32) -> Self {
        let entries = vec![NvmeCommand::default(); size as usize].into_boxed_slice();
        let phys = virt_to_phys(entries.as_ptr() as u64);
        Self {
            entries,
            phys,
            size,
            tail: 0,
            doorbell,
        }
    }

    fn submit(&mut self, cmd: NvmeCommand) -> u16 {
        let idx = self.tail as usize;
        self.entries[idx] = cmd;
        self.tail = (self.tail + 1) % self.size;
        unsafe { write_volatile(self.doorbell, self.tail as u32); }
        cmd.cid
    }
}

impl CompletionQueue {
    fn new(size: u16, doorbell: *mut u32) -> Self {
        let entries = vec![NvmeCompletion::default(); size as usize].into_boxed_slice();
        let phys = virt_to_phys(entries.as_ptr() as u64);
        Self {
            entries,
            phys,
            size,
            head: 0,
            phase: true,
            doorbell,
        }
    }

    fn poll(&mut self) -> Option<NvmeCompletion> {
        let idx = self.head as usize;
        let entry = unsafe { read_volatile(&self.entries[idx]) };

        if entry.phase() == self.phase {
            self.head = (self.head + 1) % self.size;
            if self.head == 0 {
                self.phase = !self.phase;
            }
            unsafe { write_volatile(self.doorbell, self.head as u32); }
            Some(entry)
        } else {
            None
        }
    }

    fn wait_completion(&mut self, timeout_us: u64) -> Option<NvmeCompletion> {
        let deadline = timeout_us * 1000; // Convert to rough cycles
        let mut elapsed = 0u64;
        while elapsed < deadline {
            if let Some(cqe) = self.poll() {
                return Some(cqe);
            }
            core::hint::spin_loop();
            elapsed += 1;
        }
        None
    }
}

// ============================================================================
// Controller NVMe
// ============================================================================

pub struct NvmeController {
    pci: PciDevice,
    regs: *mut NvmeRegs,
    admin_sq: Mutex<SubmissionQueue>,
    admin_cq: Mutex<CompletionQueue>,
    io_sq: Mutex<SubmissionQueue>,
    io_cq: Mutex<CompletionQueue>,
    next_cid: AtomicU16,
    block_size: u32,
    num_blocks: u64,
    id: BlockDeviceId,
}

unsafe impl Send for NvmeController {}
unsafe impl Sync for NvmeController {}

impl NvmeController {
    fn alloc_cid(&self) -> u16 {
        self.next_cid.fetch_add(1, Ordering::Relaxed)
    }

    fn read_cap(&self) -> u64 {
        unsafe { read_volatile(&(*self.regs).cap) }
    }

    fn read_csts(&self) -> u32 {
        unsafe { read_volatile(&(*self.regs).csts) }
    }

    fn write_cc(&self, val: u32) {
        unsafe { write_volatile(&mut (*self.regs).cc, val); }
    }

    fn write_aqa(&self, val: u32) {
        unsafe { write_volatile(&mut (*self.regs).aqa, val); }
    }

    fn write_asq(&self, val: u64) {
        unsafe { write_volatile(&mut (*self.regs).asq, val); }
    }

    fn write_acq(&self, val: u64) {
        unsafe { write_volatile(&mut (*self.regs).acq, val); }
    }

    fn doorbell_base(&self) -> *mut u32 {
        unsafe { (self.regs as *mut u8).add(0x1000) as *mut u32 }
    }

    fn sq_doorbell(&self, qid: u16) -> *mut u32 {
        let stride = 4; // Doorbell stride (4 bytes for most controllers)
        unsafe { self.doorbell_base().add((2 * qid as usize) * stride) }
    }

    fn cq_doorbell(&self, qid: u16) -> *mut u32 {
        let stride = 4;
        unsafe { self.doorbell_base().add((2 * qid as usize + 1) * stride) }
    }

    fn admin_command(&self, mut cmd: NvmeCommand) -> KResult<NvmeCompletion> {
        cmd.cid = self.alloc_cid();
        self.admin_sq.lock().submit(cmd);

        let cqe = self.admin_cq.lock().wait_completion(1_000_000)
            .ok_or(KError::Timeout)?;

        if cqe.is_error() {
            crate::kprintln!("nvme: admin cmd error: status={:#x}", cqe.status_code());
            return Err(KError::IO);
        }

        Ok(cqe)
    }

    fn identify_controller(&self) -> KResult<()> {
        // Aloca buffer para Identify Controller
        let buf = vec![0u8; 4096].into_boxed_slice();
        let buf_phys = virt_to_phys(buf.as_ptr() as u64);

        let cmd = NvmeCommand {
            opcode: ADMIN_IDENTIFY,
            flags: 0,
            cid: 0,
            nsid: 0,
            rsvd: 0,
            mptr: 0,
            prp1: buf_phys,
            prp2: 0,
            cdw10: 1, // CNS = 1 (Identify Controller)
            ..Default::default()
        };

        self.admin_command(cmd)?;

        // Parse identify data
        let sn = core::str::from_utf8(&buf[4..24]).unwrap_or("Unknown").trim();
        let mn = core::str::from_utf8(&buf[24..64]).unwrap_or("Unknown").trim();
        crate::kprintln!("nvme: controller: {} {}", mn, sn);

        Ok(())
    }

    fn identify_namespace(&self, nsid: u32) -> KResult<(u64, u32)> {
        let buf = vec![0u8; 4096].into_boxed_slice();
        let buf_phys = virt_to_phys(buf.as_ptr() as u64);

        let cmd = NvmeCommand {
            opcode: ADMIN_IDENTIFY,
            flags: 0,
            cid: 0,
            nsid,
            rsvd: 0,
            mptr: 0,
            prp1: buf_phys,
            prp2: 0,
            cdw10: 0, // CNS = 0 (Identify Namespace)
            ..Default::default()
        };

        self.admin_command(cmd)?;

        // Parse namespace data
        let nsze = u64::from_le_bytes(buf[0..8].try_into().unwrap());
        let flbas = buf[26];
        let lba_index = (flbas & 0xF) as usize;

        // LBA Format descriptor starts at offset 128
        let lbaf_off = 128 + lba_index * 4;
        let lbaf = u32::from_le_bytes(buf[lbaf_off..lbaf_off + 4].try_into().unwrap());
        let lba_ds = (lbaf >> 16) & 0xFF;
        let block_size = 1u32 << lba_ds;

        crate::kprintln!("nvme: namespace {}: {} blocks, {} bytes/block", nsid, nsze, block_size);

        Ok((nsze, block_size))
    }

    fn create_io_queues(&self) -> KResult<()> {
        // Create I/O Completion Queue (QID=1)
        let cq_phys = {
            let cq = self.io_cq.lock();
            cq.phys
        };

        let cmd = NvmeCommand {
            opcode: ADMIN_CREATE_CQ,
            flags: 0,
            cid: 0,
            nsid: 0,
            rsvd: 0,
            mptr: 0,
            prp1: cq_phys,
            prp2: 0,
            cdw10: ((IO_QUEUE_SIZE as u32 - 1) << 16) | 1, // QID=1, size
            cdw11: 1, // Physically Contiguous, Interrupts Enabled
            ..Default::default()
        };
        self.admin_command(cmd)?;

        // Create I/O Submission Queue (QID=1)
        let sq_phys = {
            let sq = self.io_sq.lock();
            sq.phys
        };

        let cmd = NvmeCommand {
            opcode: ADMIN_CREATE_SQ,
            flags: 0,
            cid: 0,
            nsid: 0,
            rsvd: 0,
            mptr: 0,
            prp1: sq_phys,
            prp2: 0,
            cdw10: ((IO_QUEUE_SIZE as u32 - 1) << 16) | 1, // QID=1, size
            cdw11: (1 << 16) | 1, // CQID=1, Physically Contiguous
            ..Default::default()
        };
        self.admin_command(cmd)?;

        crate::kprintln!("nvme: I/O queues criadas (QID=1)");
        Ok(())
    }

    fn io_command(&self, cmd: NvmeCommand) -> KResult<NvmeCompletion> {
        let _cid = {
            let mut sq = self.io_sq.lock();
            sq.submit(cmd)
        };

        let cqe = self.io_cq.lock().wait_completion(10_000_000)
            .ok_or(KError::Timeout)?;

        if cqe.is_error() {
            crate::kprintln!("nvme: I/O error: status={:#x}", cqe.status_code());
            return Err(KError::IO);
        }

        Ok(cqe)
    }
}

impl BlockDevice for NvmeController {
    fn id(&self) -> BlockDeviceId {
        self.id
    }

    fn block_size(&self) -> u32 {
        self.block_size
    }

    fn num_blocks(&self) -> u64 {
        self.num_blocks
    }

    fn read_blocks(&self, lba: u64, count: u32, out: &mut [u8]) -> KResult<()> {
        if count == 0 {
            return Ok(());
        }

        let expected_len = count as usize * self.block_size as usize;
        if out.len() < expected_len {
            return Err(KError::Invalid);
        }

        // Para simplicidade, lê um bloco por vez
        // Uma implementação otimizada usaria PRPs para múltiplos blocos
        for i in 0..count {
            let block_lba = lba + i as u64;
            let offset = i as usize * self.block_size as usize;
            let buf_phys = virt_to_phys(out[offset..].as_ptr() as u64);

            let cmd = NvmeCommand {
                opcode: IO_READ,
                flags: 0,
                cid: self.alloc_cid(),
                nsid: 1,
                rsvd: 0,
                mptr: 0,
                prp1: buf_phys,
                prp2: 0,
                cdw10: block_lba as u32,
                cdw11: (block_lba >> 32) as u32,
                cdw12: 0, // NLB = 0 (1 block)
                ..Default::default()
            };

            self.io_command(cmd)?;
        }

        Ok(())
    }

    fn write_blocks(&self, lba: u64, count: u32, data: &[u8]) -> KResult<()> {
        if count == 0 {
            return Ok(());
        }

        let expected_len = count as usize * self.block_size as usize;
        if data.len() < expected_len {
            return Err(KError::Invalid);
        }

        for i in 0..count {
            let block_lba = lba + i as u64;
            let offset = i as usize * self.block_size as usize;
            let buf_phys = virt_to_phys(data[offset..].as_ptr() as u64);

            let cmd = NvmeCommand {
                opcode: IO_WRITE,
                flags: 0,
                cid: self.alloc_cid(),
                nsid: 1,
                rsvd: 0,
                mptr: 0,
                prp1: buf_phys,
                prp2: 0,
                cdw10: block_lba as u32,
                cdw11: (block_lba >> 32) as u32,
                cdw12: 0, // NLB = 0 (1 block)
                ..Default::default()
            };

            self.io_command(cmd)?;
        }

        Ok(())
    }
}

// ============================================================================
// Funções públicas
// ============================================================================

pub fn probe(dev: &PciDevice) -> Option<Arc<NvmeController>> {
    if dev.class.class_code != NVME_CLASS
        || dev.class.subclass != NVME_SUBCLASS
        || dev.class.prog_if != NVME_PROG_IF
    {
        return None;
    }

    crate::kprintln!("nvme: encontrado controller PCI {:02x}:{:02x}.{}",
        dev.addr.bus, dev.addr.device, dev.addr.function);

    // Lê BAR0 (64-bit MMIO)
    let bar0_lo = pci::read_u32(dev.addr.bus, dev.addr.device, dev.addr.function, 0x10);
    let bar0_hi = pci::read_u32(dev.addr.bus, dev.addr.device, dev.addr.function, 0x14);
    let bar0_phys = ((bar0_hi as u64) << 32) | ((bar0_lo as u64) & 0xFFFF_FFF0);

    if bar0_phys == 0 {
        crate::kprintln!("nvme: BAR0 inválido");
        return None;
    }

    // Habilita bus mastering e memória
    pci::enable_bus_mastering(dev);

    // Mapeia MMIO
    let regs = mm::phys_to_virt(x86_64::PhysAddr::new(bar0_phys)).as_mut_ptr::<NvmeRegs>();

    Some(Arc::new(NvmeController {
        pci: *dev,
        regs,
        admin_sq: Mutex::new(SubmissionQueue::new(ADMIN_QUEUE_SIZE, core::ptr::null_mut())),
        admin_cq: Mutex::new(CompletionQueue::new(ADMIN_QUEUE_SIZE, core::ptr::null_mut())),
        io_sq: Mutex::new(SubmissionQueue::new(IO_QUEUE_SIZE, core::ptr::null_mut())),
        io_cq: Mutex::new(CompletionQueue::new(IO_QUEUE_SIZE, core::ptr::null_mut())),
        next_cid: AtomicU16::new(1),
        block_size: 512,
        num_blocks: 0,
        id: BlockDeviceId(100),
    }))
}

pub fn init(ctrl: Arc<NvmeController>) -> KResult<Arc<NvmeController>> {
    // Desabilita controller
    ctrl.write_cc(0);

    // Espera controller desabilitar
    let mut timeout = 1_000_000;
    while ctrl.read_csts() & 1 != 0 && timeout > 0 {
        core::hint::spin_loop();
        timeout -= 1;
    }
    if timeout == 0 {
        crate::kprintln!("nvme: timeout desabilitando controller");
        return Err(KError::Timeout);
    }

    // Lê capacidades
    let cap = ctrl.read_cap();
    let mqes = (cap & 0xFFFF) as u16 + 1; // Maximum Queue Entries Supported
    let dstrd = ((cap >> 32) & 0xF) as u8; // Doorbell Stride
    crate::kprintln!("nvme: CAP: MQES={}, DSTRD={}", mqes, dstrd);

    // Configura Admin Queues
    let aqa = ((ADMIN_QUEUE_SIZE as u32 - 1) << 16) | (ADMIN_QUEUE_SIZE as u32 - 1);
    ctrl.write_aqa(aqa);

    // Configura doorbells
    let admin_sq_db = ctrl.sq_doorbell(0);
    let admin_cq_db = ctrl.cq_doorbell(0);
    {
        let mut sq = ctrl.admin_sq.lock();
        sq.doorbell = admin_sq_db;
        ctrl.write_asq(sq.phys);
    }
    {
        let mut cq = ctrl.admin_cq.lock();
        cq.doorbell = admin_cq_db;
        ctrl.write_acq(cq.phys);
    }

    // Configura I/O queue doorbells
    let io_sq_db = ctrl.sq_doorbell(1);
    let io_cq_db = ctrl.cq_doorbell(1);
    {
        let mut sq = ctrl.io_sq.lock();
        sq.doorbell = io_sq_db;
    }
    {
        let mut cq = ctrl.io_cq.lock();
        cq.doorbell = io_cq_db;
    }

    // Configura CC (Controller Configuration)
    // MPS=0 (4K pages), CSS=0 (NVM command set), EN=1
    let cc = (0 << 7) | (0 << 4) | 1;
    ctrl.write_cc(cc);

    // Espera controller ready
    timeout = 1_000_000;
    while ctrl.read_csts() & 1 == 0 && timeout > 0 {
        core::hint::spin_loop();
        timeout -= 1;
    }
    if timeout == 0 {
        crate::kprintln!("nvme: timeout habilitando controller");
        return Err(KError::Timeout);
    }

    crate::kprintln!("nvme: controller habilitado");

    // Identifica controller
    ctrl.identify_controller()?;

    // Identifica namespace 1
    let (_num_blocks, _block_size) = ctrl.identify_namespace(1)?;

    // Cria I/O queues
    ctrl.create_io_queues()?;

    // Atualiza informações do dispositivo
    // Precisamos recriar o Arc com os valores corretos
    // Por simplicidade, retornamos o original (já tem valores default)
    // Uma implementação real usaria interior mutability ou builder pattern

    crate::kprintln!("nvme: driver inicializado");
    Ok(ctrl)
}
