//! ELF64 Loader
//!
//! Carrega binários ELF64 em um address space de processo.

#![allow(dead_code)]

use alloc::vec::Vec;
use core::mem;

use x86_64::structures::paging::{
    Mapper, OffsetPageTable, Page, PageTableFlags, Size4KiB,
};
use x86_64::VirtAddr;

use crate::mm::{self, BitmapFrameAllocator};
use crate::util::KError;

/// ELF Magic: 0x7F 'E' 'L' 'F'
pub const ELF_MAGIC: [u8; 4] = [0x7F, b'E', b'L', b'F'];

/// ELF64 Header
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64Header {
    pub e_ident: [u8; 16],
    pub e_type: u16,
    pub e_machine: u16,
    pub e_version: u32,
    pub e_entry: u64,
    pub e_phoff: u64,
    pub e_shoff: u64,
    pub e_flags: u32,
    pub e_ehsize: u16,
    pub e_phentsize: u16,
    pub e_phnum: u16,
    pub e_shentsize: u16,
    pub e_shnum: u16,
    pub e_shstrndx: u16,
}

impl Elf64Header {
    pub fn from_bytes(data: &[u8]) -> Option<&Self> {
        if data.len() < mem::size_of::<Self>() {
            return None;
        }
        let hdr = unsafe { &*(data.as_ptr() as *const Self) };
        if &hdr.e_ident[0..4] != &ELF_MAGIC {
            return None;
        }
        Some(hdr)
    }

    /// Verifica se é ELF64 válido para x86_64
    pub fn is_valid_x86_64(&self) -> bool {
        // EI_CLASS = 2 (64-bit)
        self.e_ident[4] == 2 &&
        // EI_DATA = 1 (little-endian)
        self.e_ident[5] == 1 &&
        // e_machine = 0x3E (x86_64)
        self.e_machine == 0x3E &&
        // e_type = 2 (executable) or 3 (shared/PIE)
        (self.e_type == 2 || self.e_type == 3)
    }
}

/// ELF64 Program Header
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64Phdr {
    pub p_type: u32,
    pub p_flags: u32,
    pub p_offset: u64,
    pub p_vaddr: u64,
    pub p_paddr: u64,
    pub p_filesz: u64,
    pub p_memsz: u64,
    pub p_align: u64,
}

/// Program header types
pub const PT_NULL: u32 = 0;
pub const PT_LOAD: u32 = 1;
pub const PT_DYNAMIC: u32 = 2;
pub const PT_INTERP: u32 = 3;
pub const PT_NOTE: u32 = 4;
pub const PT_SHLIB: u32 = 5;
pub const PT_PHDR: u32 = 6;
pub const PT_TLS: u32 = 7;
pub const PT_GNU_EH_FRAME: u32 = 0x6474e550;
pub const PT_GNU_STACK: u32 = 0x6474e551;
pub const PT_GNU_RELRO: u32 = 0x6474e552;

/// Program header flags
pub const PF_X: u32 = 1; // Executable
pub const PF_W: u32 = 2; // Writable
pub const PF_R: u32 = 4; // Readable

/// Auxiliary vector entry types (para stack setup)
#[repr(u64)]
#[derive(Debug, Clone, Copy)]
pub enum AuxvType {
    Null = 0,
    Ignore = 1,
    ExecFd = 2,
    Phdr = 3,
    Phent = 4,
    Phnum = 5,
    Pagesz = 6,
    Base = 7,
    Flags = 8,
    Entry = 9,
    NotElf = 10,
    Uid = 11,
    Euid = 12,
    Gid = 13,
    Egid = 14,
    Platform = 15,
    Hwcap = 16,
    Clktck = 17,
    Secure = 23,
    BasePlatform = 24,
    Random = 25,
    Hwcap2 = 26,
    Execfn = 31,
}

/// Informações extraídas do ELF
#[derive(Debug)]
pub struct ElfInfo {
    pub entry: u64,
    pub phdr_addr: u64,
    pub phdr_num: u16,
    pub phdr_size: u16,
    pub stack_executable: bool,
    pub load_base: u64,  // Base onde o ELF foi carregado (para PIE)
    pub load_end: u64,   // Fim do último segmento carregado
}

/// Carrega um ELF64 no address space
pub fn load_elf(
    mapper: &mut OffsetPageTable<'static>,
    fa: &mut BitmapFrameAllocator,
    elf_data: &[u8],
) -> Result<ElfInfo, KError> {
    // Parse header
    let hdr = Elf64Header::from_bytes(elf_data).ok_or(KError::Invalid)?;

    if !hdr.is_valid_x86_64() {
        crate::kprintln!("elf: formato inválido (não é ELF64 x86_64)");
        return Err(KError::Invalid);
    }

    crate::kprintln!(
        "elf: entry={:#x}, phoff={:#x}, phnum={}",
        hdr.e_entry,
        hdr.e_phoff,
        hdr.e_phnum
    );

    // Parse program headers
    let mut load_base = u64::MAX;
    let mut load_end = 0u64;
    let mut phdr_addr = 0u64;
    let mut stack_executable = false;

    for i in 0..hdr.e_phnum {
        let ph_offset = hdr.e_phoff as usize + (i as usize) * hdr.e_phentsize as usize;
        if ph_offset + mem::size_of::<Elf64Phdr>() > elf_data.len() {
            return Err(KError::Invalid);
        }

        let phdr = unsafe { &*(elf_data.as_ptr().add(ph_offset) as *const Elf64Phdr) };

        match phdr.p_type {
            PT_LOAD => {
                load_segment(mapper, fa, elf_data, phdr)?;

                if phdr.p_vaddr < load_base {
                    load_base = phdr.p_vaddr;
                }
                let seg_end = phdr.p_vaddr + phdr.p_memsz;
                if seg_end > load_end {
                    load_end = seg_end;
                }
            }
            PT_PHDR => {
                phdr_addr = phdr.p_vaddr;
            }
            PT_GNU_STACK => {
                stack_executable = (phdr.p_flags & PF_X) != 0;
            }
            PT_INTERP => {
                // Dynamic linking não suportado ainda
                crate::kprintln!("elf: aviso: PT_INTERP ignorado (dynamic linking não suportado)");
            }
            _ => {
                // Ignora outros tipos
            }
        }
    }

    if load_base == u64::MAX {
        crate::kprintln!("elf: nenhum segmento PT_LOAD encontrado");
        return Err(KError::Invalid);
    }

    // Se não encontrou PT_PHDR, calcula
    if phdr_addr == 0 && hdr.e_phoff > 0 {
        phdr_addr = load_base + hdr.e_phoff;
    }

    Ok(ElfInfo {
        entry: hdr.e_entry,
        phdr_addr,
        phdr_num: hdr.e_phnum,
        phdr_size: hdr.e_phentsize,
        stack_executable,
        load_base,
        load_end,
    })
}

/// Carrega um segmento PT_LOAD
fn load_segment(
    mapper: &mut OffsetPageTable<'static>,
    fa: &mut BitmapFrameAllocator,
    elf_data: &[u8],
    phdr: &Elf64Phdr,
) -> Result<(), KError> {
    use x86_64::structures::paging::Translate;

    if phdr.p_memsz == 0 {
        return Ok(());
    }

    let vaddr_start = phdr.p_vaddr & !0xFFF; // Page-align down
    let vaddr_end = (phdr.p_vaddr + phdr.p_memsz + 0xFFF) & !0xFFF; // Page-align up
    let num_pages = ((vaddr_end - vaddr_start) / 4096) as usize;

    // Constrói flags de página
    let mut flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
    if (phdr.p_flags & PF_W) != 0 {
        flags |= PageTableFlags::WRITABLE;
    }
    // NX bit seria: if (phdr.p_flags & PF_X) == 0 { flags |= PageTableFlags::NO_EXECUTE; }

    crate::kprintln!(
        "elf: load segment vaddr={:#x}-{:#x} ({} pages), offset={:#x}, filesz={:#x}, memsz={:#x}",
        vaddr_start,
        vaddr_end,
        num_pages,
        phdr.p_offset,
        phdr.p_filesz,
        phdr.p_memsz
    );

    for page_idx in 0..num_pages {
        let page_vaddr = vaddr_start + (page_idx as u64) * 4096;
        let page: Page<Size4KiB> = Page::containing_address(VirtAddr::new(page_vaddr));

        // Verifica se a página já está mapeada
        let already_mapped = mapper.translate_addr(VirtAddr::new(page_vaddr)).is_some();

        let frame_phys = if already_mapped {
            // Página já mapeada (por segmento anterior) - apenas obtém o frame existente
            let phys = mapper.translate_addr(VirtAddr::new(page_vaddr)).unwrap();
            // Arredonda para o início do frame
            x86_64::PhysAddr::new(phys.as_u64() & !0xFFF)
        } else {
            // Aloca novo frame
            let frame = fa.allocate().ok_or(KError::NoMemory)?;

            // Mapeia
            unsafe {
                mapper
                    .map_to(page, frame, flags, fa)
                    .map_err(|e| {
                        crate::kprintln!("elf: map_to failed for page {:#x}: {:?}", page_vaddr, e);
                        KError::NoMemory
                    })?
                    .flush();
            }

            // Zera a página nova
            let frame_virt = mm::phys_to_virt(frame.start_address());
            let dst = frame_virt.as_mut_ptr::<u8>();
            unsafe {
                core::ptr::write_bytes(dst, 0, 4096);
            }

            frame.start_address()
        };

        // Copia dados para o frame
        let frame_virt = mm::phys_to_virt(frame_phys);
        let dst = frame_virt.as_mut_ptr::<u8>();

        // Calcula o que copiar do arquivo
        let page_file_start = if page_vaddr >= phdr.p_vaddr {
            page_vaddr - phdr.p_vaddr
        } else {
            0
        };

        if page_file_start < phdr.p_filesz {
            let offset_in_page = if page_vaddr < phdr.p_vaddr {
                (phdr.p_vaddr - page_vaddr) as usize
            } else {
                0
            };

            let file_offset = phdr.p_offset + page_file_start;
            let remaining_file = phdr.p_filesz - page_file_start;
            let copy_len = core::cmp::min(remaining_file as usize, 4096 - offset_in_page);

            if file_offset as usize + copy_len <= elf_data.len() {
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        elf_data.as_ptr().add(file_offset as usize),
                        dst.add(offset_in_page),
                        copy_len,
                    );
                }
            }
        }
    }

    Ok(())
}

/// Constantes para stack setup
pub const USER_STACK_TOP: u64 = 0x0000_7FFF_FFFF_0000; // Stack no topo do user space
pub const USER_STACK_SIZE: usize = 2 * 1024 * 1024; // 2 MB
pub const USER_STACK_PAGES: usize = USER_STACK_SIZE / 4096;

/// Informações do stack após setup
#[derive(Debug)]
pub struct StackInfo {
    pub sp: u64,           // Stack pointer inicial
    pub argc_ptr: u64,     // Ponteiro para argc na stack
}

/// Configura a stack do usuário com argc, argv, envp, auxv
///
/// Layout da stack (endereços crescendo para baixo):
/// ```
/// High address
/// +------------------+
/// | random bytes(16) |  <- para AT_RANDOM
/// +------------------+
/// | strings (argv)   |
/// | strings (envp)   |
/// | executable name  |
/// +------------------+
/// | padding (align)  |
/// +------------------+
/// | auxv[n] = {0,0}  |  <- terminator
/// | auxv[...]        |
/// | auxv[0]          |
/// +------------------+
/// | NULL             |  <- envp terminator
/// | envp[...]        |
/// | envp[0]          |
/// +------------------+
/// | NULL             |  <- argv terminator
/// | argv[...]        |
/// | argv[0]          |
/// +------------------+
/// | argc             |  <- SP aponta aqui na entrada
/// +------------------+
/// Low address
/// ```
pub fn setup_user_stack(
    mapper: &mut OffsetPageTable<'static>,
    fa: &mut BitmapFrameAllocator,
    elf_info: &ElfInfo,
    argv: &[&str],
    envp: &[&str],
    exec_path: &str,
) -> Result<StackInfo, KError> {
    // Mapeia páginas de stack
    let stack_base = USER_STACK_TOP - (USER_STACK_PAGES as u64 * 4096);
    let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE;

    for i in 0..USER_STACK_PAGES {
        let va = VirtAddr::new(stack_base + (i as u64) * 4096);
        let page: Page<Size4KiB> = Page::containing_address(va);
        let frame = match fa.allocate() {
            Some(f) => f,
            None => {
                crate::kprintln!("setup_user_stack: frame alloc failed at page {}", i);
                return Err(KError::NoMemory);
            }
        };
        unsafe {
            match mapper.map_to(page, frame, flags, fa) {
                Ok(flush) => flush.flush(),
                Err(e) => {
                    crate::kprintln!("setup_user_stack: map_to failed at page {} va={:#x}: {:?}", i, va.as_u64(), e);
                    return Err(KError::NoMemory);
                }
            }

            // Zera a página
            let dst = mm::phys_to_virt(frame.start_address()).as_mut_ptr::<u8>();
            core::ptr::write_bytes(dst, 0, 4096);
        }
    }

    // Agora construímos o conteúdo da stack
    // Trabalhamos em um buffer temporário e depois copiamos

    let mut sp = USER_STACK_TOP;

    // Helper para escrever na stack do usuário
    let write_to_stack = |mapper: &mut OffsetPageTable<'static>, addr: u64, data: &[u8]| -> Result<(), KError> {
        use x86_64::structures::paging::Translate;
        for (i, &byte) in data.iter().enumerate() {
            let va = VirtAddr::new(addr + i as u64);
            let phys = mapper.translate_addr(va).ok_or(KError::Invalid)?;
            let virt = mm::phys_to_virt(phys);
            unsafe { *(virt.as_mut_ptr::<u8>()) = byte; }
        }
        Ok(())
    };

    // 1. Random bytes (16 bytes para AT_RANDOM)
    sp -= 16;
    let random_addr = sp;
    // Usamos um "random" simples baseado no endereço
    let random_bytes: [u8; 16] = [
        0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0,
        0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88,
    ];
    write_to_stack(mapper, random_addr, &random_bytes)?;

    // 2. Executable name string
    sp -= exec_path.len() as u64 + 1;
    let execfn_addr = sp;
    write_to_stack(mapper, execfn_addr, exec_path.as_bytes())?;
    write_to_stack(mapper, execfn_addr + exec_path.len() as u64, &[0])?;

    // 3. Environment strings
    let mut envp_addrs = Vec::new();
    for env in envp.iter().rev() {
        sp -= env.len() as u64 + 1;
        write_to_stack(mapper, sp, env.as_bytes())?;
        write_to_stack(mapper, sp + env.len() as u64, &[0])?;
        envp_addrs.push(sp);
    }
    envp_addrs.reverse();

    // 4. Argument strings
    let mut argv_addrs = Vec::new();
    for arg in argv.iter().rev() {
        sp -= arg.len() as u64 + 1;
        write_to_stack(mapper, sp, arg.as_bytes())?;
        write_to_stack(mapper, sp + arg.len() as u64, &[0])?;
        argv_addrs.push(sp);
    }
    argv_addrs.reverse();

    // Align to 16 bytes
    sp &= !0xF;

    // 5. Auxiliary vector
    let auxv: [(u64, u64); 12] = [
        (AuxvType::Phdr as u64, elf_info.phdr_addr),
        (AuxvType::Phent as u64, elf_info.phdr_size as u64),
        (AuxvType::Phnum as u64, elf_info.phdr_num as u64),
        (AuxvType::Pagesz as u64, 4096),
        (AuxvType::Entry as u64, elf_info.entry),
        (AuxvType::Uid as u64, 1000),
        (AuxvType::Euid as u64, 1000),
        (AuxvType::Gid as u64, 1000),
        (AuxvType::Egid as u64, 1000),
        (AuxvType::Random as u64, random_addr),
        (AuxvType::Execfn as u64, execfn_addr),
        (AuxvType::Null as u64, 0), // Terminator
    ];

    // Write auxv (in reverse, bottom to top)
    for &(atype, aval) in auxv.iter().rev() {
        sp -= 8;
        write_to_stack(mapper, sp, &aval.to_le_bytes())?;
        sp -= 8;
        write_to_stack(mapper, sp, &atype.to_le_bytes())?;
    }

    // 6. NULL terminator for envp
    sp -= 8;
    write_to_stack(mapper, sp, &0u64.to_le_bytes())?;

    // 7. envp pointers
    for &addr in envp_addrs.iter().rev() {
        sp -= 8;
        write_to_stack(mapper, sp, &addr.to_le_bytes())?;
    }

    // 8. NULL terminator for argv
    sp -= 8;
    write_to_stack(mapper, sp, &0u64.to_le_bytes())?;

    // 9. argv pointers
    for &addr in argv_addrs.iter().rev() {
        sp -= 8;
        write_to_stack(mapper, sp, &addr.to_le_bytes())?;
    }

    // 10. argc
    sp -= 8;
    let argc = argv.len() as u64;
    write_to_stack(mapper, sp, &argc.to_le_bytes())?;

    let argc_ptr = sp;

    Ok(StackInfo {
        sp,
        argc_ptr,
    })
}

/// Verifica se os dados são um ELF válido
pub fn is_elf(data: &[u8]) -> bool {
    if data.len() < 4 {
        return false;
    }
    &data[0..4] == &ELF_MAGIC
}

/// Obtém o entry point de um ELF sem carregar
pub fn get_entry_point(data: &[u8]) -> Option<u64> {
    let hdr = Elf64Header::from_bytes(data)?;
    if hdr.is_valid_x86_64() {
        Some(hdr.e_entry)
    } else {
        None
    }
}
