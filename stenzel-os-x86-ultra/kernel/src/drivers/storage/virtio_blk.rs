//! virtio-blk (PCI, modo *legacy I/O*)
//!
//! Objetivo:
//! - Funcionar no QEMU de forma previsível
//! - Ser um "dispositivo de bloco" real para validar GPT/FS

#![allow(dead_code)]
//!
//! Notas:
//! - Implementa o caminho síncrono (1 request por vez) usando virtqueue 0.
//! - Usa o layout legacy (registradores via BAR I/O). Para simplificar o bring-up,
//!   o runner (crate `os`) passa `disable-modern=on` no QEMU.
//!
//! Referências úteis:
//! - virtio 0.9/legacy header offsets (OSDev, Linux sources)

use alloc::vec::Vec;
use core::mem::size_of;
use core::sync::atomic::{fence, Ordering};

use x86_64::instructions::port::Port;
use x86_64::{PhysAddr, VirtAddr};

use crate::drivers::pci::{self, PciDevice};
use crate::mm;
use crate::storage::block::{check_io_args, BlockDevice, BlockDeviceId};
use crate::util::{KError, KResult};

// Vendor do virtio (QEMU)
const VIRTIO_PCI_VENDOR: u16 = 0x1AF4;

// Status bits (legacy)
const STATUS_ACKNOWLEDGE: u8 = 1;
const STATUS_DRIVER: u8 = 2;
const STATUS_DRIVER_OK: u8 = 4;
const STATUS_FEATURES_OK: u8 = 8;
const STATUS_FAILED: u8 = 0x80;

// Virtio PCI legacy register offsets (I/O base)
const REG_DEVICE_FEATURES: u16 = 0x00;
const REG_GUEST_FEATURES: u16 = 0x04;
const REG_QUEUE_ADDRESS: u16 = 0x08;
const REG_QUEUE_SIZE: u16 = 0x0C;
const REG_QUEUE_SELECT: u16 = 0x0E;
const REG_QUEUE_NOTIFY: u16 = 0x10;
const REG_DEVICE_STATUS: u16 = 0x12;
const REG_ISR_STATUS: u16 = 0x13;
const REG_DEVICE_CONFIG: u16 = 0x14;

// virtqueue flags
const VIRTQ_DESC_F_NEXT: u16 = 1;
const VIRTQ_DESC_F_WRITE: u16 = 2;

// virtio-blk request types
const VIRTIO_BLK_T_IN: u32 = 0; // read
const VIRTIO_BLK_T_OUT: u32 = 1; // write

#[derive(Debug, Clone, Copy)]
pub enum VirtioBlkError {
    NotVirtio,
    NoIoBar,
    QueueUnavailable,
    NoMemory,
    FeaturesRejected,
    Io,
}

impl From<VirtioBlkError> for KError {
    fn from(_e: VirtioBlkError) -> Self {
        KError::IO
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct VirtqDesc {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
}

#[repr(C)]
struct VirtqAvail {
    flags: u16,
    idx: u16,
    ring: [u16; 0],
    // u16 used_event (se VIRTIO_F_EVENT_IDX)
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct VirtqUsedElem {
    id: u32,
    len: u32,
}

#[repr(C)]
struct VirtqUsed {
    flags: u16,
    idx: u16,
    ring: [VirtqUsedElem; 0],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct VirtioBlkReq {
    type_: u32,
    reserved: u32,
    sector: u64,
}

pub struct VirtioBlk {
    id: BlockDeviceId,
    pci: PciDevice,
    io_base: u16,
    queue_size: u16,
    queue_paddr: PhysAddr,
    desc: *mut VirtqDesc,
    avail: *mut VirtqAvail,
    used: *mut VirtqUsed,
    used_ring_offset: usize,
    capacity_blocks: u64,
}

unsafe impl Send for VirtioBlk {}
unsafe impl Sync for VirtioBlk {}

/// Heurística de probe: vendor virtio + device_id dentro do range legacy/moderno.
pub fn probe(dev: &PciDevice) -> Option<PciDevice> {
    if dev.id.vendor_id != VIRTIO_PCI_VENDOR {
        return None;
    }
    Some(*dev)
}

pub fn init(dev: PciDevice) -> Result<VirtioBlk, VirtioBlkError> {
    // BAR0 precisa ser I/O para legacy.
    let (bar0, is_io) = pci::read_bar(&dev, 0);
    if !is_io {
        return Err(VirtioBlkError::NoIoBar);
    }
    let io_base = bar0 as u16;

    // Habilita bus mastering (DMA) + IO/MEM.
    pci::enable_bus_mastering(&dev);

    // Reset
    write_status(io_base, 0);

    // Acknowledge + Driver
    write_status(io_base, STATUS_ACKNOWLEDGE);
    write_status(io_base, STATUS_ACKNOWLEDGE | STATUS_DRIVER);

    // Features: para bring-up, aceitamos 0.
    // (Depois: negociar VIRTIO_BLK_F_RO, VIRTIO_BLK_F_FLUSH, VIRTIO_F_RING_EVENT_IDX, etc.)
    let _dev_features = read_u32(io_base, REG_DEVICE_FEATURES);
    write_u32(io_base, REG_GUEST_FEATURES, 0);
    write_status(io_base, STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK);

    // Rele e verifica FEATURES_OK.
    let st = read_u8(io_base, REG_DEVICE_STATUS);
    if (st & STATUS_FEATURES_OK) == 0 {
        write_status(io_base, st | STATUS_FAILED);
        return Err(VirtioBlkError::FeaturesRejected);
    }

    // Configura virtqueue 0
    write_u16(io_base, REG_QUEUE_SELECT, 0);
    let qsz = read_u16(io_base, REG_QUEUE_SIZE);
    if qsz == 0 {
        return Err(VirtioBlkError::QueueUnavailable);
    }

    // Usar o tamanho do device para que o layout da queue seja correto
    let queue_size = qsz;

    // Aloca memória contígua e alinhada para a virtqueue.
    // Layout legacy (virtio 0.9):
    // - Descriptors: 16 * queue_size bytes (começa em offset 0)
    // - Available ring: 4 + 2 * queue_size bytes (logo após descriptors)
    // - Padding para alinhar ao próximo page boundary
    // - Used ring: 4 + 8 * queue_size bytes (começa em page boundary)
    let desc_bytes = size_of::<VirtqDesc>() * queue_size as usize;
    let avail_bytes = 4 + 2 * queue_size as usize;
    let used_bytes = 4 + 8 * queue_size as usize;

    // Used ring deve começar em page boundary (spec virtio 0.9)
    let used_ring_offset = align_up(desc_bytes + avail_bytes, 4096);
    let total = used_ring_offset + used_bytes;
    let pages = align_up(total, 4096) / 4096;

    let (queue_frame, queue_virt) = mm::alloc_contiguous_pages(pages).ok_or(VirtioBlkError::NoMemory)?;
    let queue_paddr = queue_frame.start_address();

    // Zera a região
    unsafe {
        core::ptr::write_bytes(queue_virt.as_mut_ptr::<u8>(), 0, total);
    }

    let desc_ptr = queue_virt.as_u64() as *mut VirtqDesc;
    let avail_ptr = (queue_virt.as_u64() + desc_bytes as u64) as *mut VirtqAvail;
    let used_ptr = (queue_virt.as_u64() + used_ring_offset as u64) as *mut VirtqUsed;

    // Informa a queue ao device: valor é PFN (addr/4096)
    let pfn = (queue_paddr.as_u64() / 4096) as u32;
    write_u32(io_base, REG_QUEUE_ADDRESS, pfn);

    // Driver OK
    write_status(io_base, STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK | STATUS_DRIVER_OK);

    // Lê capacity (em setores 512) do config space
    let cap_lo = read_u32(io_base, REG_DEVICE_CONFIG + 0) as u64;
    let cap_hi = read_u32(io_base, REG_DEVICE_CONFIG + 4) as u64;
    let capacity_blocks = cap_lo | (cap_hi << 32);

    Ok(VirtioBlk {
        id: BlockDeviceId(1),
        pci: dev,
        io_base,
        queue_size,
        queue_paddr,
        desc: desc_ptr,
        avail: avail_ptr,
        used: used_ptr,
        used_ring_offset,
        capacity_blocks,
    })
}

impl VirtioBlk {
    fn notify(&self) {
        // Escreve o número da queue (0) no queue notify.
        write_u16(self.io_base, REG_QUEUE_NOTIFY, 0);
    }

    fn avail_ring_ptr(&self) -> *mut u16 {
        unsafe {
            // avail.ring começa logo após flags/idx (4 bytes)
            (self.avail as *mut u8).add(4) as *mut u16
        }
    }

    fn used_ring_ptr(&self) -> *mut VirtqUsedElem {
        unsafe {
            // used.ring começa logo após flags/idx (4 bytes)
            (self.used as *mut u8).add(4) as *mut VirtqUsedElem
        }
    }

    fn submit_request(&self, req: &VirtioBlkReq, data: &mut [u8], write: bool) -> Result<(), VirtioBlkError> {
        // Aloca header e status no heap para garantir DMA-acessível
        // (stack pode não ser mapeado com identidade)
        let req_box = alloc::boxed::Box::new(*req);
        let status_box = alloc::boxed::Box::new(0u8);

        // Monta scatter/gather por páginas.
        let mut segs: Vec<(u64, u32)> = Vec::new();
        let mut remaining = data.len();
        let mut cur = data.as_ptr() as u64;

        while remaining > 0 {
            let page_off = (cur as usize) & 0xFFF;
            let chunk = core::cmp::min(remaining, 4096 - page_off);
            let p = mm::virt_to_phys(VirtAddr::new(cur)).ok_or(VirtioBlkError::Io)?;
            segs.push((p.as_u64(), chunk as u32));
            cur += chunk as u64;
            remaining -= chunk;
        }

        // Precisamos: header + segs + status
        let needed = 1 + segs.len() + 1;
        if needed > self.queue_size as usize {
            return Err(VirtioBlkError::Io);
        }

        // Endereços físicos dos buffers
        let req_virt = VirtAddr::from_ptr(&*req_box as *const _);
        let req_p = mm::virt_to_phys(req_virt).ok_or(VirtioBlkError::Io)?;

        let status_virt = VirtAddr::from_ptr(&*status_box as *const _);
        let status_p = mm::virt_to_phys(status_virt).ok_or(VirtioBlkError::Io)?;

        // Descriptor 0: header (VOLATILE writes - device lê via DMA)
        unsafe {
            let d = self.desc.add(0);
            core::ptr::write_volatile(&mut (*d).addr, req_p.as_u64());
            core::ptr::write_volatile(&mut (*d).len, size_of::<VirtioBlkReq>() as u32);
            core::ptr::write_volatile(&mut (*d).flags, VIRTQ_DESC_F_NEXT);
            core::ptr::write_volatile(&mut (*d).next, 1);
        }

        // Data descriptors
        for (i, (addr, len)) in segs.iter().enumerate() {
            let idx = 1 + i;
            unsafe {
                let d = self.desc.add(idx);
                core::ptr::write_volatile(&mut (*d).addr, *addr);
                core::ptr::write_volatile(&mut (*d).len, *len);
                // Se for READ (device -> driver), marca WRITE.
                let mut flags = VIRTQ_DESC_F_NEXT;
                if !write {
                    flags |= VIRTQ_DESC_F_WRITE;
                }
                core::ptr::write_volatile(&mut (*d).flags, flags);
                core::ptr::write_volatile(&mut (*d).next, (idx + 1) as u16);
            }
        }

        // Last: status
        let status_idx = 1 + segs.len();
        unsafe {
            let d = self.desc.add(status_idx);
            core::ptr::write_volatile(&mut (*d).addr, status_p.as_u64());
            core::ptr::write_volatile(&mut (*d).len, 1);
            core::ptr::write_volatile(&mut (*d).flags, VIRTQ_DESC_F_WRITE);
            core::ptr::write_volatile(&mut (*d).next, 0);
        }

        // Memory barrier: garante que todos os descritores foram escritos
        fence(Ordering::SeqCst);

        // Captura used.idx ANTES de submeter (device pode processar muito rápido)
        let start_used = unsafe { core::ptr::read_volatile(&(*self.used).idx) };

        // Coloca head=0 na avail ring (VOLATILE - dispositivo lê via DMA)
        unsafe {
            let idx = core::ptr::read_volatile(&(*self.avail).idx);
            let ring = self.avail_ring_ptr();
            core::ptr::write_volatile(
                ring.add((idx as usize) % (self.queue_size as usize)),
                0
            );
            fence(Ordering::SeqCst);
            core::ptr::write_volatile(&mut (*self.avail).idx, idx.wrapping_add(1));
        }

        // Memory barrier antes de notificar
        fence(Ordering::SeqCst);

        // Notifica o device
        self.notify();

        // Espera resposta (polling)
        loop {
            fence(Ordering::SeqCst);
            let cur_used = unsafe { core::ptr::read_volatile(&(*self.used).idx) };
            if cur_used != start_used {
                break;
            }
            core::hint::spin_loop();
        }

        // Verifica status (0 = OK)
        fence(Ordering::SeqCst);
        let final_status = unsafe { core::ptr::read_volatile(&*status_box) };
        if final_status != 0 {
            return Err(VirtioBlkError::Io);
        }

        Ok(())
    }

    fn read_write(&self, lba: u64, _count: u32, buf: &mut [u8], write: bool) -> Result<(), VirtioBlkError> {
        let mut req = VirtioBlkReq::default();
        req.type_ = if write { VIRTIO_BLK_T_OUT } else { VIRTIO_BLK_T_IN };
        // virtio-blk usa setor de 512 bytes.
        // Nosso BlockDevice também usa block_size=512 aqui.
        req.sector = lba;

        self.submit_request(&req, buf, write)
    }
}

impl BlockDevice for VirtioBlk {
    fn id(&self) -> BlockDeviceId {
        self.id
    }

    fn block_size(&self) -> u32 {
        512
    }

    fn num_blocks(&self) -> u64 {
        self.capacity_blocks
    }

    fn read_blocks(&self, lba: u64, count: u32, out: &mut [u8]) -> KResult<()> {
        check_io_args(self.block_size(), count, out.len())?;
        // lba em setores; count em setores
        self.read_write(lba, count, out, false).map_err(|e| e.into())
    }

    fn write_blocks(&self, lba: u64, count: u32, data: &[u8]) -> KResult<()> {
        check_io_args(self.block_size(), count, data.len())?;
        // Precisamos de &mut [u8] para scatter/gather, então copiamos p/ um buffer temporário.
        // (Depois: suportar slice imutável criando descritores read-only diretamente.)
        let mut tmp = Vec::with_capacity(data.len());
        tmp.extend_from_slice(data);
        self.read_write(lba, count, &mut tmp, true).map_err(|e| e.into())
    }
}

#[inline]
fn align_up(x: usize, a: usize) -> usize {
    (x + (a - 1)) & !(a - 1)
}

#[inline]
fn read_u32(base: u16, off: u16) -> u32 {
    unsafe { Port::<u32>::new(base + off).read() }
}

#[inline]
fn write_u32(base: u16, off: u16, v: u32) {
    unsafe { Port::<u32>::new(base + off).write(v) }
}

#[inline]
fn read_u16(base: u16, off: u16) -> u16 {
    unsafe { Port::<u16>::new(base + off).read() }
}

#[inline]
fn write_u16(base: u16, off: u16, v: u16) {
    unsafe { Port::<u16>::new(base + off).write(v) }
}

#[inline]
fn read_u8(base: u16, off: u16) -> u8 {
    unsafe { Port::<u8>::new(base + off).read() }
}

#[inline]
fn write_status(base: u16, status: u8) {
    unsafe { Port::<u8>::new(base + REG_DEVICE_STATUS).write(status) }
}
