//! Processos e ELF loader.
//!
//! Suporta:
//! - Estrutura de processos com PID
//! - ELF64 loader para binários x86_64
//! - Execução em ring3

#![allow(dead_code)]

pub mod elf;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use x86_64::registers::control::Cr3;
use x86_64::structures::paging::{OffsetPageTable, PageTable, PhysFrame, Size4KiB};

use crate::mm;
use crate::security::Cred;
use crate::util::KError;

pub use elf::ElfInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pid(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    Runnable,
    Running,
    Zombie,
}

pub struct Process {
    pub pid: Pid,
    pub name: String,
    pub state: ProcessState,
    pub cred: Cred,
    pub cr3: PhysFrame<Size4KiB>,
    pub entry: u64,
    pub stack_pointer: u64,
}

pub struct ProcessTable {
    next: AtomicU64,
    procs: BTreeMap<Pid, Arc<Process>>,
}

impl ProcessTable {
    pub fn new() -> Self {
        Self {
            next: AtomicU64::new(1),
            procs: BTreeMap::new(),
        }
    }

    pub fn alloc_pid(&self) -> Pid {
        Pid(self.next.fetch_add(1, Ordering::Relaxed))
    }
}

/// Resultado do carregamento de um ELF
#[derive(Debug)]
pub struct LoadedElf {
    pub cr3: PhysFrame<Size4KiB>,
    pub entry: u64,
    pub stack_pointer: u64,
    pub elf_info: ElfInfo,
}

/// Carrega um binário ELF em um novo address space
///
/// Retorna as informações necessárias para executar o processo
pub fn load_elf_into_new_space(
    elf_data: &[u8],
    argv: &[&str],
    envp: &[&str],
    exec_path: &str,
) -> Result<LoadedElf, KError> {
    // Valida ELF
    if !elf::is_elf(elf_data) {
        crate::kprintln!("load_elf: não é um ELF válido");
        return Err(KError::Invalid);
    }

    // Cria novo address space
    let (cr3, mut mapper) = create_user_address_space()?;

    // Obtém lock do frame allocator
    let mut fa = mm::frame_allocator_lock();

    // Carrega o ELF
    let elf_info = elf::load_elf(&mut mapper, &mut *fa, elf_data)?;

    crate::kprintln!(
        "load_elf: entry={:#x}, load_base={:#x}, load_end={:#x}",
        elf_info.entry,
        elf_info.load_base,
        elf_info.load_end
    );

    // Setup da stack do usuário
    let stack_info = elf::setup_user_stack(&mut mapper, &mut *fa, &elf_info, argv, envp, exec_path)?;

    Ok(LoadedElf {
        cr3,
        entry: elf_info.entry,
        stack_pointer: stack_info.sp,
        elf_info,
    })
}

/// Cria um novo address space para um processo de usuário
///
/// Retorna o frame do P4 e um mapper para o novo address space
fn create_user_address_space() -> Result<(PhysFrame<Size4KiB>, OffsetPageTable<'static>), KError> {
    let phys_off = mm::physical_memory_offset();
    let mut fa = mm::frame_allocator_lock();

    // Aloca frame para o P4
    let p4_frame = fa.allocate().ok_or(KError::NoMemory)?;

    // Ponteiro virtual do P4
    let p4_virt = mm::phys_to_virt(p4_frame.start_address());
    let p4_ptr = p4_virt.as_mut_ptr::<PageTable>();

    // Zera o P4
    unsafe { (*p4_ptr).zero(); }

    // Copia as entradas do kernel (high-half) do P4 ativo
    let (active_p4_frame, _) = Cr3::read();
    let active_p4_virt = mm::phys_to_virt(active_p4_frame.start_address());
    let active_p4_ptr = active_p4_virt.as_ptr::<PageTable>();

    unsafe {
        let p4_ref = &mut *p4_ptr;
        let active_p4_ref = &*active_p4_ptr;
        use x86_64::structures::paging::PageTableFlags;

        // The bootloader uses LOW-HALF addresses for kernel (not high-half).
        // P4[0] contains both kernel code (~0x200000) and user code (0x400000).
        // We need to deep-copy P4[0] but only keep kernel (non-USER_ACCESSIBLE) entries.
        // Other P4 entries (except P4[255] which is user stack) can be shallow-copied.

        // Handle P4[0] specially - it contains both kernel and user code
        if active_p4_ref[0].flags().contains(PageTableFlags::PRESENT) {
            // Allocate new P3 for P4[0]
            let new_p3_frame = fa.allocate().ok_or(KError::NoMemory)?;
            let new_p3_virt = mm::phys_to_virt(new_p3_frame.start_address());
            let new_p3_ptr = new_p3_virt.as_mut_ptr::<PageTable>();
            (*new_p3_ptr).zero();

            // Get old P3
            let old_p3_phys = active_p4_ref[0].addr();
            let old_p3_virt = mm::phys_to_virt(old_p3_phys);
            let old_p3_ref = &*(old_p3_virt.as_ptr::<PageTable>());
            let new_p3_ref = &mut *new_p3_ptr;

            // Copy P3 entries, but for P3[0] we need to deep-copy only kernel P2 entries
            for j in 0..512 {
                if !old_p3_ref[j].flags().contains(PageTableFlags::PRESENT) {
                    continue;
                }

                if j == 0 {
                    // P3[0] contains both kernel (P2[1] at 0x200000) and user (P2[2+] at 0x400000+)
                    // Deep-copy only kernel portions
                    let new_p2_frame = fa.allocate().ok_or(KError::NoMemory)?;
                    let new_p2_virt = mm::phys_to_virt(new_p2_frame.start_address());
                    let new_p2_ptr = new_p2_virt.as_mut_ptr::<PageTable>();
                    (*new_p2_ptr).zero();

                    let old_p2_phys = old_p3_ref[0].addr();
                    let old_p2_virt = mm::phys_to_virt(old_p2_phys);
                    let old_p2_ref = &*(old_p2_virt.as_ptr::<PageTable>());
                    let new_p2_ref = &mut *new_p2_ptr;

                    // Copy only non-USER_ACCESSIBLE P2 entries (kernel entries)
                    for k in 0..512 {
                        let p2_flags = old_p2_ref[k].flags();
                        if p2_flags.contains(PageTableFlags::PRESENT)
                           && !p2_flags.contains(PageTableFlags::USER_ACCESSIBLE) {
                            new_p2_ref[k] = old_p2_ref[k].clone();
                        }
                    }

                    new_p3_ref[0].set_addr(
                        new_p2_frame.start_address(),
                        old_p3_ref[0].flags() & !PageTableFlags::USER_ACCESSIBLE,
                    );
                } else if !old_p3_ref[j].flags().contains(PageTableFlags::USER_ACCESSIBLE) {
                    // Non-user P3 entries can be shallow-copied
                    new_p3_ref[j] = old_p3_ref[j].clone();
                }
                // Skip USER_ACCESSIBLE P3 entries (user code/data)
            }

            // Set P4[0] to point to new P3
            p4_ref[0].set_addr(
                new_p3_frame.start_address(),
                active_p4_ref[0].flags() & !PageTableFlags::USER_ACCESSIBLE,
            );
        }

        // Copy other kernel P4 entries (skip P4[0] already handled, skip P4[255] user stack)
        for i in 1..512 {
            if i == 255 {
                continue; // Skip user stack region
            }
            let flags = active_p4_ref[i].flags();
            if flags.contains(PageTableFlags::PRESENT) && !flags.contains(PageTableFlags::USER_ACCESSIBLE) {
                p4_ref[i] = active_p4_ref[i].clone();
            }
        }
    }

    // Cria mapper para o novo address space
    let mapper = unsafe { OffsetPageTable::new(&mut *p4_ptr, phys_off) };

    Ok((p4_frame, mapper))
}

/// Carrega um ELF de um arquivo no filesystem
pub fn load_elf_from_path(
    path: &str,
    argv: &[&str],
    envp: &[&str],
) -> Result<LoadedElf, KError> {
    use crate::fs;
    use crate::security::Cred;

    // Abre o arquivo
    let vfs = fs::vfs_lock();
    let cred = Cred::root(); // TODO: usar credenciais do processo atual
    let inode = vfs.resolve(path, &cred)?;

    // Lê tamanho do arquivo
    let size = inode.0.size()?;

    if size == 0 {
        return Err(KError::Invalid);
    }

    // Aloca buffer e lê o arquivo
    let mut buf = Vec::with_capacity(size);
    buf.resize(size, 0u8);

    let read = inode.0.read_at(0, &mut buf)?;

    if read != size {
        crate::kprintln!("load_elf: leitura parcial ({} de {} bytes)", read, size);
    }

    drop(vfs);

    // Carrega o ELF
    load_elf_into_new_space(&buf, argv, envp, path)
}
