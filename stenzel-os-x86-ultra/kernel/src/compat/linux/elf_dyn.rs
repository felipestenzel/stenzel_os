//! ELF Dynamic Section Parser
//!
//! Parses the .dynamic section of ELF files for dynamic linking support.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::mem;

/// ELF64 Dynamic Entry
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64Dyn {
    pub d_tag: i64,
    pub d_val: u64, // Can also be d_ptr
}

/// Dynamic entry tags
pub mod dt {
    pub const NULL: i64 = 0;
    pub const NEEDED: i64 = 1;
    pub const PLTRELSZ: i64 = 2;
    pub const PLTGOT: i64 = 3;
    pub const HASH: i64 = 4;
    pub const STRTAB: i64 = 5;
    pub const SYMTAB: i64 = 6;
    pub const RELA: i64 = 7;
    pub const RELASZ: i64 = 8;
    pub const RELAENT: i64 = 9;
    pub const STRSZ: i64 = 10;
    pub const SYMENT: i64 = 11;
    pub const INIT: i64 = 12;
    pub const FINI: i64 = 13;
    pub const SONAME: i64 = 14;
    pub const RPATH: i64 = 15;
    pub const SYMBOLIC: i64 = 16;
    pub const REL: i64 = 17;
    pub const RELSZ: i64 = 18;
    pub const RELENT: i64 = 19;
    pub const PLTREL: i64 = 20;
    pub const DEBUG: i64 = 21;
    pub const TEXTREL: i64 = 22;
    pub const JMPREL: i64 = 23;
    pub const BIND_NOW: i64 = 24;
    pub const INIT_ARRAY: i64 = 25;
    pub const FINI_ARRAY: i64 = 26;
    pub const INIT_ARRAYSZ: i64 = 27;
    pub const FINI_ARRAYSZ: i64 = 28;
    pub const RUNPATH: i64 = 29;
    pub const FLAGS: i64 = 30;
    pub const PREINIT_ARRAY: i64 = 32;
    pub const PREINIT_ARRAYSZ: i64 = 33;
    pub const GNU_HASH: i64 = 0x6ffffef5;
    pub const RELACOUNT: i64 = 0x6ffffff9;
    pub const RELCOUNT: i64 = 0x6ffffffa;
    pub const FLAGS_1: i64 = 0x6ffffffb;
    pub const VERSYM: i64 = 0x6ffffff0;
    pub const VERDEF: i64 = 0x6ffffffc;
    pub const VERDEFNUM: i64 = 0x6ffffffd;
    pub const VERNEED: i64 = 0x6ffffffe;
    pub const VERNEEDNUM: i64 = 0x6fffffff;
}

/// ELF64 Symbol Table Entry
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64Sym {
    pub st_name: u32,
    pub st_info: u8,
    pub st_other: u8,
    pub st_shndx: u16,
    pub st_value: u64,
    pub st_size: u64,
}

impl Elf64Sym {
    pub fn bind(&self) -> u8 {
        self.st_info >> 4
    }

    pub fn sym_type(&self) -> u8 {
        self.st_info & 0xf
    }

    pub fn visibility(&self) -> u8 {
        self.st_other & 0x3
    }
}

/// Symbol binding
pub mod stb {
    pub const LOCAL: u8 = 0;
    pub const GLOBAL: u8 = 1;
    pub const WEAK: u8 = 2;
}

/// Symbol types
pub mod stt {
    pub const NOTYPE: u8 = 0;
    pub const OBJECT: u8 = 1;
    pub const FUNC: u8 = 2;
    pub const SECTION: u8 = 3;
    pub const FILE: u8 = 4;
    pub const COMMON: u8 = 5;
    pub const TLS: u8 = 6;
}

/// ELF64 Relocation Entry with Addend
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64Rela {
    pub r_offset: u64,
    pub r_info: u64,
    pub r_addend: i64,
}

impl Elf64Rela {
    pub fn sym(&self) -> u32 {
        (self.r_info >> 32) as u32
    }

    pub fn rel_type(&self) -> u32 {
        (self.r_info & 0xffffffff) as u32
    }
}

/// x86_64 relocation types
pub mod r_x86_64 {
    pub const NONE: u32 = 0;
    pub const R64: u32 = 1;
    pub const PC32: u32 = 2;
    pub const GOT32: u32 = 3;
    pub const PLT32: u32 = 4;
    pub const COPY: u32 = 5;
    pub const GLOB_DAT: u32 = 6;
    pub const JUMP_SLOT: u32 = 7;
    pub const RELATIVE: u32 = 8;
    pub const GOTPCREL: u32 = 9;
    pub const R32: u32 = 10;
    pub const R32S: u32 = 11;
    pub const R16: u32 = 12;
    pub const PC16: u32 = 13;
    pub const R8: u32 = 14;
    pub const PC8: u32 = 15;
    pub const DTPMOD64: u32 = 16;
    pub const DTPOFF64: u32 = 17;
    pub const TPOFF64: u32 = 18;
    pub const TLSGD: u32 = 19;
    pub const TLSLD: u32 = 20;
    pub const DTPOFF32: u32 = 21;
    pub const GOTTPOFF: u32 = 22;
    pub const TPOFF32: u32 = 23;
    pub const PC64: u32 = 24;
    pub const GOTOFF64: u32 = 25;
    pub const GOTPC32: u32 = 26;
    pub const IRELATIVE: u32 = 37;
}

/// GNU Hash table header
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GnuHashHeader {
    pub nbuckets: u32,
    pub symoffset: u32,
    pub bloom_size: u32,
    pub bloom_shift: u32,
}

/// Parsed dynamic information
#[derive(Debug, Default)]
pub struct DynamicInfo {
    pub strtab: u64,
    pub strsz: u64,
    pub symtab: u64,
    pub syment: u64,
    pub hash: u64,
    pub gnu_hash: u64,
    pub rela: u64,
    pub relasz: u64,
    pub relaent: u64,
    pub jmprel: u64,
    pub pltrelsz: u64,
    pub pltrel: u64,
    pub pltgot: u64,
    pub init: u64,
    pub fini: u64,
    pub init_array: u64,
    pub init_arraysz: u64,
    pub fini_array: u64,
    pub fini_arraysz: u64,
    pub needed: Vec<u64>,  // String table offsets
    pub soname: u64,
    pub rpath: u64,
    pub runpath: u64,
    pub flags: u64,
    pub flags_1: u64,
    pub relacount: u64,
}

impl DynamicInfo {
    /// Parse dynamic section from memory
    pub fn parse(data: &[u8], offset: usize) -> Self {
        let mut info = DynamicInfo::default();

        let mut pos = offset;
        while pos + mem::size_of::<Elf64Dyn>() <= data.len() {
            let dyn_entry = unsafe { &*(data.as_ptr().add(pos) as *const Elf64Dyn) };

            if dyn_entry.d_tag == dt::NULL {
                break;
            }

            match dyn_entry.d_tag {
                dt::STRTAB => info.strtab = dyn_entry.d_val,
                dt::STRSZ => info.strsz = dyn_entry.d_val,
                dt::SYMTAB => info.symtab = dyn_entry.d_val,
                dt::SYMENT => info.syment = dyn_entry.d_val,
                dt::HASH => info.hash = dyn_entry.d_val,
                dt::GNU_HASH => info.gnu_hash = dyn_entry.d_val,
                dt::RELA => info.rela = dyn_entry.d_val,
                dt::RELASZ => info.relasz = dyn_entry.d_val,
                dt::RELAENT => info.relaent = dyn_entry.d_val,
                dt::JMPREL => info.jmprel = dyn_entry.d_val,
                dt::PLTRELSZ => info.pltrelsz = dyn_entry.d_val,
                dt::PLTREL => info.pltrel = dyn_entry.d_val,
                dt::PLTGOT => info.pltgot = dyn_entry.d_val,
                dt::INIT => info.init = dyn_entry.d_val,
                dt::FINI => info.fini = dyn_entry.d_val,
                dt::INIT_ARRAY => info.init_array = dyn_entry.d_val,
                dt::INIT_ARRAYSZ => info.init_arraysz = dyn_entry.d_val,
                dt::FINI_ARRAY => info.fini_array = dyn_entry.d_val,
                dt::FINI_ARRAYSZ => info.fini_arraysz = dyn_entry.d_val,
                dt::NEEDED => info.needed.push(dyn_entry.d_val),
                dt::SONAME => info.soname = dyn_entry.d_val,
                dt::RPATH => info.rpath = dyn_entry.d_val,
                dt::RUNPATH => info.runpath = dyn_entry.d_val,
                dt::FLAGS => info.flags = dyn_entry.d_val,
                dt::FLAGS_1 => info.flags_1 = dyn_entry.d_val,
                dt::RELACOUNT => info.relacount = dyn_entry.d_val,
                _ => {}
            }

            pos += mem::size_of::<Elf64Dyn>();
        }

        info
    }
}

/// Read a null-terminated string from a string table
pub fn read_string(strtab: &[u8], offset: usize) -> Option<String> {
    if offset >= strtab.len() {
        return None;
    }

    let mut end = offset;
    while end < strtab.len() && strtab[end] != 0 {
        end += 1;
    }

    String::from_utf8(strtab[offset..end].to_vec()).ok()
}

/// ELF hash function (SYSV hash)
pub fn elf_hash(name: &str) -> u32 {
    let mut h: u32 = 0;
    for c in name.bytes() {
        h = (h << 4) + c as u32;
        let g = h & 0xf0000000;
        if g != 0 {
            h ^= g >> 24;
        }
        h &= !g;
    }
    h
}

/// GNU hash function
pub fn gnu_hash(name: &str) -> u32 {
    let mut h: u32 = 5381;
    for c in name.bytes() {
        h = h.wrapping_mul(33).wrapping_add(c as u32);
    }
    h
}

/// Section header entry
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64Shdr {
    pub sh_name: u32,
    pub sh_type: u32,
    pub sh_flags: u64,
    pub sh_addr: u64,
    pub sh_offset: u64,
    pub sh_size: u64,
    pub sh_link: u32,
    pub sh_info: u32,
    pub sh_addralign: u64,
    pub sh_entsize: u64,
}

/// Section types
pub mod sht {
    pub const NULL: u32 = 0;
    pub const PROGBITS: u32 = 1;
    pub const SYMTAB: u32 = 2;
    pub const STRTAB: u32 = 3;
    pub const RELA: u32 = 4;
    pub const HASH: u32 = 5;
    pub const DYNAMIC: u32 = 6;
    pub const NOTE: u32 = 7;
    pub const NOBITS: u32 = 8;
    pub const REL: u32 = 9;
    pub const SHLIB: u32 = 10;
    pub const DYNSYM: u32 = 11;
    pub const INIT_ARRAY: u32 = 14;
    pub const FINI_ARRAY: u32 = 15;
    pub const PREINIT_ARRAY: u32 = 16;
    pub const GROUP: u32 = 17;
    pub const SYMTAB_SHNDX: u32 = 18;
    pub const GNU_HASH: u32 = 0x6ffffff6;
    pub const GNU_VERDEF: u32 = 0x6ffffffd;
    pub const GNU_VERNEED: u32 = 0x6ffffffe;
    pub const GNU_VERSYM: u32 = 0x6fffffff;
}
