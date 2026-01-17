//! Virtual Memory Area (VMA) management
//!
//! Gerencia regiões de memória virtual por processo para mmap/munmap.

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use x86_64::structures::paging::{Page, PageTableFlags, PhysFrame, Size4KiB};
use x86_64::VirtAddr;

use crate::sync::IrqSafeMutex;
use crate::util::{KError, KResult};

/// Proteção de memória
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Protection {
    pub read: bool,
    pub write: bool,
    pub exec: bool,
}

impl Protection {
    pub const NONE: Self = Self { read: false, write: false, exec: false };
    pub const READ: Self = Self { read: true, write: false, exec: false };
    pub const READ_WRITE: Self = Self { read: true, write: true, exec: false };
    pub const READ_EXEC: Self = Self { read: true, write: false, exec: true };
    pub const READ_WRITE_EXEC: Self = Self { read: true, write: true, exec: true };

    pub fn from_prot(prot: i32) -> Self {
        const PROT_READ: i32 = 0x1;
        const PROT_WRITE: i32 = 0x2;
        const PROT_EXEC: i32 = 0x4;

        Self {
            read: prot & PROT_READ != 0,
            write: prot & PROT_WRITE != 0,
            exec: prot & PROT_EXEC != 0,
        }
    }

    pub fn to_page_flags(&self, user: bool) -> PageTableFlags {
        let mut flags = PageTableFlags::PRESENT;
        if self.write {
            flags |= PageTableFlags::WRITABLE;
        }
        if !self.exec {
            flags |= PageTableFlags::NO_EXECUTE;
        }
        if user {
            flags |= PageTableFlags::USER_ACCESSIBLE;
        }
        flags
    }
}

/// Flags de mapeamento
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapFlags {
    pub shared: bool,
    pub private: bool,
    pub anonymous: bool,
    pub fixed: bool,
}

impl MapFlags {
    pub fn from_flags(flags: i32) -> Self {
        const MAP_SHARED: i32 = 0x01;
        const MAP_PRIVATE: i32 = 0x02;
        const MAP_FIXED: i32 = 0x10;
        const MAP_ANONYMOUS: i32 = 0x20;

        Self {
            shared: flags & MAP_SHARED != 0,
            private: flags & MAP_PRIVATE != 0,
            anonymous: flags & MAP_ANONYMOUS != 0,
            fixed: flags & MAP_FIXED != 0,
        }
    }
}

/// Uma região de memória virtual
#[derive(Debug, Clone)]
pub struct Vma {
    /// Endereço virtual inicial (page-aligned)
    pub start: u64,
    /// Tamanho em bytes
    pub size: usize,
    /// Proteção
    pub prot: Protection,
    /// Flags
    pub flags: MapFlags,
    /// Frames físicos alocados para esta VMA (None = not allocated yet, for demand paging)
    pub frames: Vec<Option<PhysFrame<Size4KiB>>>,
    /// Bitmap de páginas que são CoW (compartilhadas com outro processo)
    pub cow_pages: Vec<bool>,
}

impl Vma {
    pub fn end(&self) -> u64 {
        self.start + self.size as u64
    }

    pub fn contains(&self, addr: u64) -> bool {
        addr >= self.start && addr < self.end()
    }

    pub fn overlaps(&self, start: u64, size: usize) -> bool {
        let end = start + size as u64;
        !(end <= self.start || start >= self.end())
    }
}

/// Gerenciador de VMAs por processo
pub struct VmaManager {
    /// Mapa de VMAs ordenado por endereço inicial
    vmas: BTreeMap<u64, Vma>,
    /// Próximo endereço para alocação automática
    next_addr: u64,
}

impl VmaManager {
    /// Endereço base para mmap (região de userspace)
    const MMAP_BASE: u64 = 0x0000_7000_0000_0000;
    /// Limite superior
    const MMAP_LIMIT: u64 = 0x0000_7FFF_0000_0000;

    pub fn new() -> Self {
        Self {
            vmas: BTreeMap::new(),
            next_addr: Self::MMAP_BASE,
        }
    }

    /// Encontra um endereço livre para um mapeamento de `size` bytes
    fn find_free_region(&self, size: usize) -> Option<u64> {
        let aligned_size = (size + 0xFFF) & !0xFFF; // Page align
        let mut addr = self.next_addr;

        while addr + aligned_size as u64 <= Self::MMAP_LIMIT {
            // Verifica se há overlap com alguma VMA existente
            let overlaps = self.vmas.values().any(|vma| vma.overlaps(addr, aligned_size));
            if !overlaps {
                return Some(addr);
            }
            // Pula para depois da próxima VMA
            for vma in self.vmas.values() {
                if vma.start >= addr && vma.start < addr + aligned_size as u64 {
                    addr = (vma.end() + 0xFFF) & !0xFFF;
                    break;
                }
            }
        }

        None
    }

    /// Cria um novo mapeamento anônimo (demand paging - páginas NÃO são alocadas imediatamente)
    pub fn mmap(
        &mut self,
        addr_hint: u64,
        size: usize,
        prot: Protection,
        flags: MapFlags,
    ) -> KResult<u64> {
        if size == 0 {
            return Err(KError::Invalid);
        }

        let aligned_size = (size + 0xFFF) & !0xFFF;
        let num_pages = aligned_size / 4096;

        // Determina o endereço
        let addr = if flags.fixed {
            // MAP_FIXED: usa o endereço exato
            if addr_hint == 0 || addr_hint & 0xFFF != 0 {
                return Err(KError::Invalid);
            }
            // Remove mapeamentos existentes nessa região
            self.munmap(addr_hint, aligned_size)?;
            addr_hint
        } else if addr_hint != 0 && addr_hint & 0xFFF == 0 {
            // Tenta usar o hint, senão encontra outro
            let overlaps = self.vmas.values().any(|vma| vma.overlaps(addr_hint, aligned_size));
            if !overlaps {
                addr_hint
            } else {
                self.find_free_region(aligned_size).ok_or(KError::NoMemory)?
            }
        } else {
            self.find_free_region(aligned_size).ok_or(KError::NoMemory)?
        };

        // DEMAND PAGING: Não aloca frames ainda, só registra a VMA
        // Frames serão alocados sob demanda no page fault handler
        let frames = alloc::vec![None; num_pages];
        let cow_pages = alloc::vec![false; num_pages];

        // Registra a VMA (páginas não estão mapeadas ainda)
        let vma = Vma {
            start: addr,
            size: aligned_size,
            prot,
            flags,
            frames,
            cow_pages,
        };
        self.vmas.insert(addr, vma);

        // Atualiza next_addr se necessário
        if addr >= self.next_addr {
            self.next_addr = (addr + aligned_size as u64 + 0xFFF) & !0xFFF;
        }

        Ok(addr)
    }

    /// Aloca uma página on-demand (chamado pelo page fault handler)
    pub fn handle_page_fault(&mut self, fault_addr: u64) -> KResult<()> {
        let page_addr = fault_addr & !0xFFF;

        // Encontra a VMA que contém este endereço
        let vma = self.vmas.values_mut().find(|vma| vma.contains(fault_addr));
        let vma = vma.ok_or(KError::Invalid)?;

        // Calcula qual página dentro da VMA
        let page_index = ((page_addr - vma.start) / 4096) as usize;

        // Se já está alocada, não faz nada (pode ser erro de proteção)
        if vma.frames.get(page_index).map(|f| f.is_some()).unwrap_or(false) {
            // Página já alocada - provavelmente erro de proteção
            return Err(KError::PermissionDenied);
        }

        // Aloca um frame físico
        let mut fa = super::frame_allocator_lock();
        let frame = fa.allocate().ok_or(KError::NoMemory)?;
        drop(fa);

        // Zero a página se for anônima
        if vma.flags.anonymous {
            let virt = super::phys_to_virt(frame.start_address());
            unsafe {
                core::ptr::write_bytes(virt.as_mut_ptr::<u8>(), 0, 4096);
            }
        }

        // Mapeia a página
        let page_flags = vma.prot.to_page_flags(true);
        {
            let mut mapper = super::mapper_lock();
            let mut fa = super::frame_allocator_lock();
            let page: Page<Size4KiB> = Page::containing_address(VirtAddr::new(page_addr));
            mapper.map_page(page, frame, page_flags, &mut *fa)?;
        }

        // Registra o frame na VMA
        if page_index < vma.frames.len() {
            vma.frames[page_index] = Some(frame);
        }

        Ok(())
    }

    /// Remove um mapeamento
    pub fn munmap(&mut self, addr: u64, size: usize) -> KResult<()> {
        if addr & 0xFFF != 0 || size == 0 {
            return Err(KError::Invalid);
        }

        let aligned_size = (size + 0xFFF) & !0xFFF;
        let end = addr + aligned_size as u64;

        // Encontra VMAs que intersectam com a região
        let overlapping: Vec<u64> = self.vmas
            .iter()
            .filter(|(_, vma)| vma.overlaps(addr, aligned_size))
            .map(|(&k, _)| k)
            .collect();

        for vma_start in overlapping {
            if let Some(vma) = self.vmas.remove(&vma_start) {
                // Caso 1: VMA completamente contida na região - remove tudo
                if vma.start >= addr && vma.end() <= end {
                    // Desmapeia todas as páginas e libera os frames
                    let mut mapper = super::mapper_lock();
                    let mut fa = super::frame_allocator_lock();
                    for (i, frame) in vma.frames.iter().enumerate() {
                        if let Some(frame) = frame {
                            let page_addr = VirtAddr::new(vma.start + (i * 4096) as u64);
                            let page: Page<Size4KiB> = Page::containing_address(page_addr);
                            let _ = mapper.unmap_page(page);
                            // Libera o frame físico
                            fa.deallocate(*frame);
                        }
                    }
                }
                // Caso 2: VMA parcialmente contida - precisa split
                else if vma.start < addr && vma.end() > end {
                    // Split: cria duas novas VMAs (antes e depois da região removida)
                    let mut mapper = super::mapper_lock();
                    let mut fa = super::frame_allocator_lock();

                    // Calcula índices das páginas a remover
                    let start_page = ((addr - vma.start) / 4096) as usize;
                    let end_page = ((end - vma.start) / 4096) as usize;

                    // Remove as páginas no meio
                    for i in start_page..end_page.min(vma.frames.len()) {
                        if let Some(frame) = vma.frames[i] {
                            let page_addr = VirtAddr::new(vma.start + (i * 4096) as u64);
                            let page: Page<Size4KiB> = Page::containing_address(page_addr);
                            let _ = mapper.unmap_page(page);
                            fa.deallocate(frame);
                        }
                    }

                    drop(mapper);
                    drop(fa);

                    // Cria VMA antes da região removida
                    if start_page > 0 {
                        let before_vma = Vma {
                            start: vma.start,
                            size: start_page * 4096,
                            prot: vma.prot,
                            flags: vma.flags,
                            frames: vma.frames[..start_page].to_vec(),
                            cow_pages: vma.cow_pages[..start_page].to_vec(),
                        };
                        self.vmas.insert(before_vma.start, before_vma);
                    }

                    // Cria VMA depois da região removida
                    if end_page < vma.frames.len() {
                        let after_vma = Vma {
                            start: end,
                            size: (vma.frames.len() - end_page) * 4096,
                            prot: vma.prot,
                            flags: vma.flags,
                            frames: vma.frames[end_page..].to_vec(),
                            cow_pages: vma.cow_pages[end_page..].to_vec(),
                        };
                        self.vmas.insert(after_vma.start, after_vma);
                    }
                }
                // Caso 3: VMA parcialmente sobreposta no início
                else if vma.start < addr {
                    let mut mapper = super::mapper_lock();
                    let mut fa = super::frame_allocator_lock();

                    let keep_pages = ((addr - vma.start) / 4096) as usize;

                    // Remove páginas do final
                    for i in keep_pages..vma.frames.len() {
                        if let Some(frame) = vma.frames[i] {
                            let page_addr = VirtAddr::new(vma.start + (i * 4096) as u64);
                            let page: Page<Size4KiB> = Page::containing_address(page_addr);
                            let _ = mapper.unmap_page(page);
                            fa.deallocate(frame);
                        }
                    }

                    drop(mapper);
                    drop(fa);

                    // Mantém a parte inicial
                    if keep_pages > 0 {
                        let trimmed_vma = Vma {
                            start: vma.start,
                            size: keep_pages * 4096,
                            prot: vma.prot,
                            flags: vma.flags,
                            frames: vma.frames[..keep_pages].to_vec(),
                            cow_pages: vma.cow_pages[..keep_pages].to_vec(),
                        };
                        self.vmas.insert(trimmed_vma.start, trimmed_vma);
                    }
                }
                // Caso 4: VMA parcialmente sobreposta no fim
                else {
                    let mut mapper = super::mapper_lock();
                    let mut fa = super::frame_allocator_lock();

                    let remove_pages = ((vma.end().min(end) - vma.start) / 4096) as usize;

                    // Remove páginas do início
                    for i in 0..remove_pages.min(vma.frames.len()) {
                        if let Some(frame) = vma.frames[i] {
                            let page_addr = VirtAddr::new(vma.start + (i * 4096) as u64);
                            let page: Page<Size4KiB> = Page::containing_address(page_addr);
                            let _ = mapper.unmap_page(page);
                            fa.deallocate(frame);
                        }
                    }

                    drop(mapper);
                    drop(fa);

                    // Mantém a parte final
                    if remove_pages < vma.frames.len() {
                        let trimmed_vma = Vma {
                            start: vma.start + (remove_pages * 4096) as u64,
                            size: (vma.frames.len() - remove_pages) * 4096,
                            prot: vma.prot,
                            flags: vma.flags,
                            frames: vma.frames[remove_pages..].to_vec(),
                            cow_pages: vma.cow_pages[remove_pages..].to_vec(),
                        };
                        self.vmas.insert(trimmed_vma.start, trimmed_vma);
                    }
                }
            }
        }

        Ok(())
    }

    /// Altera proteção de uma região
    pub fn mprotect(&mut self, addr: u64, size: usize, prot: Protection) -> KResult<()> {
        if addr & 0xFFF != 0 || size == 0 {
            return Err(KError::Invalid);
        }

        let aligned_size = (size + 0xFFF) & !0xFFF;
        let new_flags = prot.to_page_flags(true);

        // Encontra a VMA que contém este endereço
        for vma in self.vmas.values_mut() {
            if vma.contains(addr) && addr + aligned_size as u64 <= vma.end() {
                vma.prot = prot;

                // Atualiza as flags de cada página na região
                let mut mapper = super::mapper_lock();
                let start_offset = (addr - vma.start) as usize;
                let start_page = start_offset / 4096;
                let num_pages = aligned_size / 4096;

                for i in start_page..(start_page + num_pages).min(vma.frames.len()) {
                    let page_addr = VirtAddr::new(vma.start + (i * 4096) as u64);
                    let page: Page<Size4KiB> = Page::containing_address(page_addr);
                    // Atualiza as flags da página
                    mapper.update_page_flags(page, new_flags);
                }

                return Ok(());
            }
        }

        Err(KError::Invalid)
    }

    /// Verifica se um endereço está mapeado
    pub fn is_mapped(&self, addr: u64) -> bool {
        self.vmas.values().any(|vma| vma.contains(addr))
    }

    /// Obtém informações sobre uma VMA
    pub fn get_vma(&self, addr: u64) -> Option<&Vma> {
        self.vmas.values().find(|vma| vma.contains(addr))
    }

    /// Remove uma VMA do tracking sem liberar frames físicos
    /// Usado para shared memory detach onde os frames são gerenciados externamente
    pub fn remove_vma(&mut self, addr: u64) -> KResult<()> {
        // Simply remove the VMA entry without deallocating frames
        // The frames are managed by the shared memory subsystem
        if self.vmas.remove(&addr).is_some() {
            Ok(())
        } else {
            // Try to find and remove VMA by address contained
            let key_to_remove = self.vmas.iter()
                .find(|(_, vma)| vma.start == addr)
                .map(|(k, _)| *k);

            if let Some(key) = key_to_remove {
                self.vmas.remove(&key);
                Ok(())
            } else {
                Err(KError::Invalid)
            }
        }
    }
}

/// VMA manager global (simplificado para single process)
static VMA_MANAGER: IrqSafeMutex<Option<VmaManager>> = IrqSafeMutex::new(None);

pub fn init() {
    *VMA_MANAGER.lock() = Some(VmaManager::new());
}

pub fn manager_lock() -> crate::sync::IrqSafeGuard<'static, Option<VmaManager>> {
    VMA_MANAGER.lock()
}

/// Syscall mmap
pub fn sys_mmap(addr: u64, length: usize, prot: i32, flags: i32, _fd: i32, _offset: i64) -> KResult<u64> {
    let prot = Protection::from_prot(prot);
    let map_flags = MapFlags::from_flags(flags);

    // Por enquanto só suportamos mapeamentos anônimos
    if !map_flags.anonymous {
        // TODO: suportar mapeamento de arquivos
        return Err(KError::NotSupported);
    }

    let mut guard = VMA_MANAGER.lock();
    let manager = guard.as_mut().ok_or(KError::NotSupported)?;
    manager.mmap(addr, length, prot, map_flags)
}

/// Syscall munmap
pub fn sys_munmap(addr: u64, length: usize) -> KResult<()> {
    let mut guard = VMA_MANAGER.lock();
    let manager = guard.as_mut().ok_or(KError::NotSupported)?;
    manager.munmap(addr, length)
}

/// Syscall mprotect
pub fn sys_mprotect(addr: u64, length: usize, prot: i32) -> KResult<()> {
    let prot = Protection::from_prot(prot);
    let mut guard = VMA_MANAGER.lock();
    let manager = guard.as_mut().ok_or(KError::NotSupported)?;
    manager.mprotect(addr, length, prot)
}

/// Handle page fault for demand paging (called from interrupt handler)
pub fn handle_page_fault(fault_addr: u64) -> KResult<()> {
    let mut guard = VMA_MANAGER.lock();
    let manager = guard.as_mut().ok_or(KError::Invalid)?;
    manager.handle_page_fault(fault_addr)
}

/// Handle CoW page fault (write to a page that was CoW-shared)
/// Called when a write fault occurs on a present page that's marked as CoW
pub fn handle_cow_page_fault(fault_addr: u64, old_phys: x86_64::PhysAddr) -> KResult<()> {
    let mut guard = VMA_MANAGER.lock();
    let manager = guard.as_mut().ok_or(KError::Invalid)?;

    let page_addr = fault_addr & !0xFFF;

    // Encontra a VMA que contém este endereço
    let vma = manager.vmas.values_mut().find(|vma| vma.contains(fault_addr));
    let vma = vma.ok_or(KError::Invalid)?;

    // Verifica se a VMA permite escrita
    if !vma.prot.write {
        return Err(KError::PermissionDenied);
    }

    // Calcula qual página dentro da VMA
    let page_index = ((page_addr - vma.start) / 4096) as usize;

    // Verifica se esta página está marcada como CoW
    if page_index >= vma.cow_pages.len() || !vma.cow_pages[page_index] {
        return Err(KError::Invalid);
    }

    // Aloca novo frame
    let mut fa = super::frame_allocator_lock();
    let new_frame = fa.allocate().ok_or(KError::NoMemory)?;
    drop(fa);

    // Copia o conteúdo do frame antigo para o novo
    let old_virt = super::phys_to_virt(old_phys);
    let new_virt = super::phys_to_virt(new_frame.start_address());
    unsafe {
        core::ptr::copy_nonoverlapping(
            old_virt.as_ptr::<u8>(),
            new_virt.as_mut_ptr::<u8>(),
            4096,
        );
    }

    // Atualiza o mapeamento para apontar para o novo frame com permissão de escrita
    let page: Page<Size4KiB> = Page::containing_address(VirtAddr::new(page_addr));
    let page_flags = vma.prot.to_page_flags(true);

    {
        let mut mapper = super::mapper_lock();
        let mut fa = super::frame_allocator_lock();

        // Desmapeia a página antiga
        if let Ok((_old_frame, flush)) = mapper.unmap_page(page) {
            flush();
        }

        // Mapeia o novo frame
        mapper.map_page(page, new_frame, page_flags, &mut *fa)?;
    }

    // Decrementa referência do frame antigo no CoW manager
    super::cow::decrement_ref(old_phys);

    // Atualiza a VMA
    if page_index < vma.frames.len() {
        vma.frames[page_index] = Some(new_frame);
    }
    vma.cow_pages[page_index] = false;

    Ok(())
}
