//! Copy-on-Write (CoW) Memory Management
//!
//! Este módulo implementa o suporte a Copy-on-Write para fork() eficiente.
//! Em vez de copiar todas as páginas imediatamente, fork() marca as páginas
//! como somente-leitura e compartilhadas. Quando um processo tenta escrever,
//! ocorre um page fault e a página é copiada nesse momento.

use alloc::collections::BTreeMap;
use core::sync::atomic::{AtomicUsize, Ordering};
use spin::Mutex;
use x86_64::structures::paging::PhysFrame;
use x86_64::PhysAddr;

/// Informações sobre um frame físico compartilhado via CoW
#[derive(Debug)]
pub struct CowFrameInfo {
    /// Contagem de referências (quantos processos compartilham este frame)
    ref_count: AtomicUsize,
}

impl CowFrameInfo {
    pub fn new() -> Self {
        Self {
            ref_count: AtomicUsize::new(1),
        }
    }

    pub fn ref_count(&self) -> usize {
        self.ref_count.load(Ordering::Acquire)
    }

    pub fn increment(&self) {
        self.ref_count.fetch_add(1, Ordering::AcqRel);
    }

    /// Decrementa e retorna true se o frame deve ser liberado (ref_count chegou a 0)
    pub fn decrement(&self) -> bool {
        let prev = self.ref_count.fetch_sub(1, Ordering::AcqRel);
        prev == 1 // Se era 1, agora é 0
    }
}

/// Gerenciador global de frames CoW
pub struct CowManager {
    /// Mapa de endereço físico para informações de CoW
    frames: BTreeMap<u64, CowFrameInfo>,
}

impl CowManager {
    pub const fn new() -> Self {
        Self {
            frames: BTreeMap::new(),
        }
    }

    /// Registra um frame como compartilhado via CoW (primeira vez)
    pub fn register_frame(&mut self, phys_addr: PhysAddr) {
        let addr = phys_addr.as_u64();
        if !self.frames.contains_key(&addr) {
            self.frames.insert(addr, CowFrameInfo::new());
        }
    }

    /// Incrementa a contagem de referências de um frame
    pub fn increment_ref(&mut self, phys_addr: PhysAddr) {
        let addr = phys_addr.as_u64();
        if let Some(info) = self.frames.get(&addr) {
            info.increment();
        } else {
            // Frame não registrado, registra com ref_count = 1 e incrementa para 2
            let info = CowFrameInfo::new();
            info.increment(); // Agora ref_count = 2 (pai + filho)
            self.frames.insert(addr, info);
        }
    }

    /// Decrementa a contagem de referências e retorna true se deve liberar o frame
    pub fn decrement_ref(&mut self, phys_addr: PhysAddr) -> bool {
        let addr = phys_addr.as_u64();
        if let Some(info) = self.frames.get(&addr) {
            let should_free = info.decrement();
            if should_free {
                self.frames.remove(&addr);
            }
            should_free
        } else {
            // Frame não estava no CoW manager, pode ser liberado diretamente
            true
        }
    }

    /// Retorna a contagem de referências de um frame (0 se não registrado)
    pub fn ref_count(&self, phys_addr: PhysAddr) -> usize {
        let addr = phys_addr.as_u64();
        self.frames.get(&addr).map(|i| i.ref_count()).unwrap_or(0)
    }

    /// Verifica se um frame é compartilhado (ref_count > 1)
    pub fn is_shared(&self, phys_addr: PhysAddr) -> bool {
        self.ref_count(phys_addr) > 1
    }

    /// Remove um frame do gerenciamento CoW (quando não é mais CoW)
    pub fn unregister_frame(&mut self, phys_addr: PhysAddr) {
        self.frames.remove(&phys_addr.as_u64());
    }
}

/// Gerenciador global de CoW
static COW_MANAGER: Mutex<CowManager> = Mutex::new(CowManager::new());

/// Registra um frame como CoW (primeira vez, ref_count = 1)
pub fn register_frame(phys_addr: PhysAddr) {
    COW_MANAGER.lock().register_frame(phys_addr);
}

/// Incrementa a referência de um frame CoW
pub fn increment_ref(phys_addr: PhysAddr) {
    COW_MANAGER.lock().increment_ref(phys_addr);
}

/// Decrementa a referência e retorna true se deve liberar
pub fn decrement_ref(phys_addr: PhysAddr) -> bool {
    COW_MANAGER.lock().decrement_ref(phys_addr)
}

/// Verifica se um frame é compartilhado
pub fn is_shared(phys_addr: PhysAddr) -> bool {
    COW_MANAGER.lock().is_shared(phys_addr)
}

/// Obtém a contagem de referências
pub fn ref_count(phys_addr: PhysAddr) -> usize {
    COW_MANAGER.lock().ref_count(phys_addr)
}

/// Remove um frame do gerenciamento CoW
pub fn unregister_frame(phys_addr: PhysAddr) {
    COW_MANAGER.lock().unregister_frame(phys_addr);
}

/// Processa um page fault de CoW
///
/// Chamado quando ocorre um write fault em uma página presente mas read-only
/// que faz parte de um mapeamento CoW.
///
/// Retorna Ok(new_frame) se a cópia foi feita, ou Err se não era CoW
pub fn handle_cow_fault(
    _fault_addr: u64,
    old_frame_phys: PhysAddr,
) -> Result<PhysFrame, ()> {
    let mut manager = COW_MANAGER.lock();

    // Verifica se é realmente um frame CoW compartilhado
    let ref_count = manager.ref_count(old_frame_phys);

    if ref_count == 0 {
        // Não é um frame CoW gerenciado
        return Err(());
    }

    if ref_count == 1 {
        // Único dono - pode simplesmente tornar writable sem copiar
        manager.unregister_frame(old_frame_phys);
        // Retorna o mesmo frame (caller deve apenas atualizar flags)
        return Ok(PhysFrame::containing_address(old_frame_phys));
    }

    // ref_count > 1: precisa fazer a cópia

    // Aloca novo frame
    let mut fa = super::frame_allocator_lock();
    let new_frame = fa.allocate().ok_or(())?;
    drop(fa);

    // Copia o conteúdo
    let old_virt = super::phys_to_virt(old_frame_phys);
    let new_virt = super::phys_to_virt(new_frame.start_address());
    unsafe {
        core::ptr::copy_nonoverlapping(
            old_virt.as_ptr::<u8>(),
            new_virt.as_mut_ptr::<u8>(),
            4096,
        );
    }

    // Decrementa ref do frame antigo
    manager.decrement_ref(old_frame_phys);

    Ok(new_frame)
}
