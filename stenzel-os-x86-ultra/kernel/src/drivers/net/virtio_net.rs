//! virtio-net driver (PCI, modo legacy I/O)
//!
//! Implementa driver de rede virtio para QEMU.
//! Similar ao virtio-blk mas para pacotes de rede.

#![allow(dead_code)]

use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::vec;
use alloc::vec::Vec;
use core::mem::size_of;
use core::sync::atomic::{fence, Ordering};

use x86_64::instructions::port::Port;
use x86_64::VirtAddr;

use crate::drivers::pci::{self, PciDevice};
use crate::mm;
use crate::sync::IrqSafeMutex;
use crate::util::{KError, KResult};

// Virtio vendor (QEMU)
const VIRTIO_PCI_VENDOR: u16 = 0x1AF4;
// Device IDs: 0x1000 = network (transitional)
const VIRTIO_NET_DEVICE_ID: u16 = 0x1000;

// Status bits (legacy)
const STATUS_ACKNOWLEDGE: u8 = 1;
const STATUS_DRIVER: u8 = 2;
const STATUS_DRIVER_OK: u8 = 4;
const STATUS_FEATURES_OK: u8 = 8;

// Virtio PCI legacy register offsets
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

// Feature bits
const VIRTIO_NET_F_MAC: u32 = 1 << 5;
const VIRTIO_NET_F_STATUS: u32 = 1 << 16;

// Virtio net header (simplificado, sem GSO)
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VirtioNetHdr {
    pub flags: u8,
    pub gso_type: u8,
    pub hdr_len: u16,
    pub gso_size: u16,
    pub csum_start: u16,
    pub csum_offset: u16,
    pub num_buffers: u16,
}

impl VirtioNetHdr {
    pub const SIZE: usize = 12; // 10 bytes padrão + 2 para num_buffers em mergeable

    pub fn new() -> Self {
        Self::default()
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

/// Virtqueue para comunicação
struct Virtqueue {
    desc: *mut VirtqDesc,
    avail: *mut VirtqAvail,
    used: *mut VirtqUsed,
    queue_size: u16,
    free_head: u16,
    last_used_idx: u16,
    // Buffers pré-alocados
    buffers: Vec<Box<[u8]>>,
}

// Safety: Virtqueue is only accessed through the IrqSafeMutex-protected VirtioNet
unsafe impl Send for Virtqueue {}
unsafe impl Sync for Virtqueue {}

impl Virtqueue {
    fn new(queue_size: u16) -> KResult<Self> {
        // Aloca memória contígua para desc, avail, used
        let desc_size = size_of::<VirtqDesc>() * queue_size as usize;
        let avail_size = 4 + 2 * queue_size as usize;
        let used_size = 4 + 8 * queue_size as usize;

        let total_size = desc_size + avail_size + used_size;
        let pages = (total_size + 4095) / 4096;

        let mut fa = mm::frame_allocator_lock();
        let mut frames = Vec::new();
        for _ in 0..pages {
            let frame = fa.allocate().ok_or(KError::NoMemory)?;
            frames.push(frame);
        }

        let base_virt = mm::phys_to_virt(frames[0].start_address());

        // Zera a memória
        unsafe {
            core::ptr::write_bytes(base_virt.as_mut_ptr::<u8>(), 0, pages * 4096);
        }

        let desc = base_virt.as_mut_ptr::<VirtqDesc>();
        let avail = (base_virt + desc_size as u64).as_mut_ptr::<VirtqAvail>();
        let used = (base_virt + (desc_size + avail_size) as u64).as_mut_ptr::<VirtqUsed>();

        // Inicializa a lista de descritores livres
        for i in 0..queue_size {
            unsafe {
                (*desc.add(i as usize)).next = if i + 1 < queue_size { i + 1 } else { 0 };
            }
        }

        // Pré-aloca buffers para RX (2KB cada, suficiente para Ethernet)
        let mut buffers = Vec::with_capacity(queue_size as usize);
        for _ in 0..queue_size {
            let buf = vec![0u8; 2048].into_boxed_slice();
            buffers.push(buf);
        }

        Ok(Self {
            desc,
            avail,
            used,
            queue_size,
            free_head: 0,
            last_used_idx: 0,
            buffers,
        })
    }

    fn phys_addr(&self) -> u64 {
        let virt = VirtAddr::from_ptr(self.desc);
        mm::virt_to_phys(virt).map(|p| p.as_u64()).unwrap_or(0)
    }

    fn alloc_desc(&mut self) -> Option<u16> {
        let idx = self.free_head;
        if idx >= self.queue_size {
            return None;
        }
        unsafe {
            self.free_head = (*self.desc.add(idx as usize)).next;
        }
        Some(idx)
    }

    fn free_desc(&mut self, idx: u16) {
        unsafe {
            (*self.desc.add(idx as usize)).next = self.free_head;
        }
        self.free_head = idx;
    }
}

/// Driver virtio-net
pub struct VirtioNet {
    io_base: u16,
    mac: [u8; 6],
    rx_queue: Virtqueue,
    tx_queue: Virtqueue,
    // Pacotes recebidos pendentes
    rx_pending: VecDeque<Vec<u8>>,
}

impl VirtioNet {
    /// Tenta inicializar um dispositivo virtio-net a partir de um PCI device
    pub fn try_new(dev: &PciDevice) -> KResult<Self> {
        if dev.id.vendor_id != VIRTIO_PCI_VENDOR {
            return Err(KError::NotSupported);
        }

        // Device ID 0x1000 = network (transitional)
        if dev.id.device_id != VIRTIO_NET_DEVICE_ID {
            return Err(KError::NotSupported);
        }

        crate::kprintln!("virtio-net: encontrado em {:02x}:{:02x}.{}",
            dev.addr.bus, dev.addr.device, dev.addr.function);

        // Obtém o BAR0 (I/O port)
        let (bar0_addr, is_io) = pci::read_bar(dev, 0);
        if !is_io {
            crate::kprintln!("virtio-net: BAR0 não é I/O space");
            return Err(KError::NotSupported);
        }
        let io_base = bar0_addr as u16;

        crate::kprintln!("virtio-net: I/O base = {:#x}", io_base);

        // Reset do dispositivo
        Self::write_status(io_base, 0);

        // Acknowledge
        Self::write_status(io_base, STATUS_ACKNOWLEDGE);
        Self::write_status(io_base, STATUS_ACKNOWLEDGE | STATUS_DRIVER);

        // Lê features do dispositivo
        let device_features = Self::read_features(io_base);
        crate::kprintln!("virtio-net: device features = {:#x}", device_features);

        // Aceita features mínimas
        let guest_features = device_features & VIRTIO_NET_F_MAC;
        Self::write_features(io_base, guest_features);

        // Lê MAC address
        let mut mac = [0u8; 6];
        for i in 0..6 {
            unsafe {
                let mut port: Port<u8> = Port::new(io_base + REG_DEVICE_CONFIG + i as u16);
                mac[i] = port.read();
            }
        }
        crate::kprintln!("virtio-net: MAC = {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]);

        // Configura RX queue (queue 0)
        Self::select_queue(io_base, 0);
        let rx_size = Self::read_queue_size(io_base);
        crate::kprintln!("virtio-net: RX queue size = {}", rx_size);

        let rx_queue = Virtqueue::new(rx_size)?;
        let rx_phys = rx_queue.phys_addr();
        Self::write_queue_addr(io_base, (rx_phys >> 12) as u32);

        // Configura TX queue (queue 1)
        Self::select_queue(io_base, 1);
        let tx_size = Self::read_queue_size(io_base);
        crate::kprintln!("virtio-net: TX queue size = {}", tx_size);

        let tx_queue = Virtqueue::new(tx_size)?;
        let tx_phys = tx_queue.phys_addr();
        Self::write_queue_addr(io_base, (tx_phys >> 12) as u32);

        // Marca driver como OK
        Self::write_status(io_base, STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_DRIVER_OK);

        let mut net = Self {
            io_base,
            mac,
            rx_queue,
            tx_queue,
            rx_pending: VecDeque::new(),
        };

        // Popula a RX queue com buffers
        net.replenish_rx_buffers();

        crate::kprintln!("virtio-net: inicializado com sucesso");

        Ok(net)
    }

    fn write_status(io_base: u16, status: u8) {
        unsafe {
            let mut port: Port<u8> = Port::new(io_base + REG_DEVICE_STATUS);
            port.write(status);
        }
    }

    fn read_features(io_base: u16) -> u32 {
        unsafe {
            let mut port: Port<u32> = Port::new(io_base + REG_DEVICE_FEATURES);
            port.read()
        }
    }

    fn write_features(io_base: u16, features: u32) {
        unsafe {
            let mut port: Port<u32> = Port::new(io_base + REG_GUEST_FEATURES);
            port.write(features);
        }
    }

    fn select_queue(io_base: u16, queue: u16) {
        unsafe {
            let mut port: Port<u16> = Port::new(io_base + REG_QUEUE_SELECT);
            port.write(queue);
        }
    }

    fn read_queue_size(io_base: u16) -> u16 {
        unsafe {
            let mut port: Port<u16> = Port::new(io_base + REG_QUEUE_SIZE);
            port.read()
        }
    }

    fn write_queue_addr(io_base: u16, addr: u32) {
        unsafe {
            let mut port: Port<u32> = Port::new(io_base + REG_QUEUE_ADDRESS);
            port.write(addr);
        }
    }

    fn notify_queue(&self, queue: u16) {
        unsafe {
            let mut port: Port<u16> = Port::new(self.io_base + REG_QUEUE_NOTIFY);
            port.write(queue);
        }
    }

    /// Adiciona buffers vazios à RX queue
    fn replenish_rx_buffers(&mut self) {
        for i in 0..self.rx_queue.queue_size {
            let buf = &self.rx_queue.buffers[i as usize];
            let buf_phys = mm::virt_to_phys(VirtAddr::from_ptr(buf.as_ptr())).map(|p| p.as_u64()).unwrap_or(0);

            // Configura descritor
            unsafe {
                let desc = self.rx_queue.desc.add(i as usize);
                (*desc).addr = buf_phys;
                (*desc).len = buf.len() as u32;
                (*desc).flags = VIRTQ_DESC_F_WRITE;
                (*desc).next = 0;
            }

            // Adiciona ao available ring
            unsafe {
                let avail = self.rx_queue.avail;
                let idx = (*avail).idx;
                let ring_ptr = (avail as *mut u16).add(2);
                *ring_ptr.add((idx % self.rx_queue.queue_size) as usize) = i;
                fence(Ordering::SeqCst);
                (*avail).idx = idx.wrapping_add(1);
            }
        }

        fence(Ordering::SeqCst);
        self.notify_queue(0);
    }

    /// Retorna o MAC address
    pub fn mac(&self) -> [u8; 6] {
        self.mac
    }

    /// Envia um pacote
    pub fn send(&mut self, data: &[u8]) -> KResult<()> {
        if data.len() > 1514 {
            return Err(KError::Invalid);
        }

        // Aloca descritor para o header + dados
        let desc_idx = self.tx_queue.alloc_desc().ok_or(KError::WouldBlock)?;

        // Prepara buffer com header + dados
        let total_len = VirtioNetHdr::SIZE + data.len();
        let buf = &mut self.tx_queue.buffers[desc_idx as usize];

        // Limpa e preenche o header
        buf[..VirtioNetHdr::SIZE].fill(0);

        // Copia os dados
        buf[VirtioNetHdr::SIZE..total_len].copy_from_slice(data);

        let buf_phys = mm::virt_to_phys(VirtAddr::from_ptr(buf.as_ptr())).map(|p| p.as_u64()).unwrap_or(0);

        // Configura descritor
        unsafe {
            let desc = self.tx_queue.desc.add(desc_idx as usize);
            (*desc).addr = buf_phys;
            (*desc).len = total_len as u32;
            (*desc).flags = 0;
            (*desc).next = 0;
        }

        // Adiciona ao available ring
        unsafe {
            let avail = self.tx_queue.avail;
            let idx = (*avail).idx;
            let ring_ptr = (avail as *mut u16).add(2);
            *ring_ptr.add((idx % self.tx_queue.queue_size) as usize) = desc_idx;
            fence(Ordering::SeqCst);
            (*avail).idx = idx.wrapping_add(1);
        }

        fence(Ordering::SeqCst);
        self.notify_queue(1);

        // Espera completar (síncrono por simplicidade)
        self.wait_tx_complete(desc_idx)?;

        // Libera descritor
        self.tx_queue.free_desc(desc_idx);

        Ok(())
    }

    fn wait_tx_complete(&mut self, _desc_idx: u16) -> KResult<()> {
        // Polling simples
        for _ in 0..100000 {
            fence(Ordering::SeqCst);
            unsafe {
                let used = self.tx_queue.used;
                let used_idx = (*used).idx;
                if used_idx != self.tx_queue.last_used_idx {
                    self.tx_queue.last_used_idx = used_idx;
                    return Ok(());
                }
            }
            core::hint::spin_loop();
        }
        Err(KError::Timeout)
    }

    /// Verifica se há pacotes recebidos
    pub fn poll_rx(&mut self) {
        fence(Ordering::SeqCst);

        unsafe {
            let used = self.rx_queue.used;
            while self.rx_queue.last_used_idx != (*used).idx {
                let ring_idx = (self.rx_queue.last_used_idx % self.rx_queue.queue_size) as usize;
                let ring_ptr = (used as *const VirtqUsedElem).add(1);
                let elem = *ring_ptr.add(ring_idx);

                let desc_idx = elem.id as usize;
                let len = elem.len as usize;

                // Copia os dados (pula o header)
                if len > VirtioNetHdr::SIZE {
                    let buf = &self.rx_queue.buffers[desc_idx];
                    let data_len = len - VirtioNetHdr::SIZE;
                    let packet = buf[VirtioNetHdr::SIZE..VirtioNetHdr::SIZE + data_len].to_vec();
                    self.rx_pending.push_back(packet);
                }

                self.rx_queue.last_used_idx = self.rx_queue.last_used_idx.wrapping_add(1);
            }
        }

        // Re-adiciona buffers ao RX
        if !self.rx_pending.is_empty() {
            self.replenish_rx_buffers();
        }
    }

    /// Recebe um pacote (se disponível)
    pub fn recv(&mut self) -> Option<Vec<u8>> {
        self.poll_rx();
        self.rx_pending.pop_front()
    }
}

// Instância global do driver
static NET_DRIVER: IrqSafeMutex<Option<VirtioNet>> = IrqSafeMutex::new(None);

/// Inicializa o driver de rede
pub fn init() {
    let devices = pci::scan();

    for dev in devices {
        if dev.id.vendor_id == VIRTIO_PCI_VENDOR && dev.id.device_id == VIRTIO_NET_DEVICE_ID {
            match VirtioNet::try_new(&dev) {
                Ok(net) => {
                    let mut driver = NET_DRIVER.lock();
                    *driver = Some(net);
                    crate::kprintln!("virtio-net: driver inicializado");
                    return;
                }
                Err(e) => {
                    crate::kprintln!("virtio-net: falha ao inicializar: {:?}", e);
                }
            }
        }
    }

    crate::kprintln!("virtio-net: nenhum dispositivo encontrado");
}

/// Retorna o MAC address da interface
pub fn get_mac() -> Option<[u8; 6]> {
    NET_DRIVER.lock().as_ref().map(|n| n.mac())
}

/// Envia um pacote
pub fn send(data: &[u8]) -> KResult<()> {
    NET_DRIVER.lock().as_mut().ok_or(KError::NotFound)?.send(data)
}

/// Recebe um pacote
pub fn recv() -> Option<Vec<u8>> {
    NET_DRIVER.lock().as_mut()?.recv()
}
