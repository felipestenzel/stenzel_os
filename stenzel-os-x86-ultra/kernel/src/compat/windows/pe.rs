//! PE/COFF Executable Format Parser
//!
//! Implements parsing for Windows PE32 and PE32+ (64-bit) executables.
//! Based on Microsoft PE/COFF specification.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use core::mem::size_of;

/// DOS Header - MZ signature at the start of every PE file
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct DosHeader {
    /// Magic number ("MZ")
    pub e_magic: u16,
    /// Bytes on last page of file
    pub e_cblp: u16,
    /// Pages in file
    pub e_cp: u16,
    /// Relocations
    pub e_crlc: u16,
    /// Size of header in paragraphs
    pub e_cparhdr: u16,
    /// Minimum extra paragraphs needed
    pub e_minalloc: u16,
    /// Maximum extra paragraphs needed
    pub e_maxalloc: u16,
    /// Initial (relative) SS value
    pub e_ss: u16,
    /// Initial SP value
    pub e_sp: u16,
    /// Checksum
    pub e_csum: u16,
    /// Initial IP value
    pub e_ip: u16,
    /// Initial (relative) CS value
    pub e_cs: u16,
    /// File address of relocation table
    pub e_lfarlc: u16,
    /// Overlay number
    pub e_ovno: u16,
    /// Reserved words
    pub e_res: [u16; 4],
    /// OEM identifier
    pub e_oemid: u16,
    /// OEM information
    pub e_oeminfo: u16,
    /// Reserved words
    pub e_res2: [u16; 10],
    /// File address of new exe header (PE header offset)
    pub e_lfanew: i32,
}

impl DosHeader {
    pub const MAGIC: u16 = 0x5A4D; // "MZ"

    pub fn is_valid(&self) -> bool {
        self.e_magic == Self::MAGIC
    }

    pub fn pe_offset(&self) -> usize {
        self.e_lfanew as usize
    }
}

/// PE Signature - "PE\0\0"
pub const PE_SIGNATURE: u32 = 0x00004550;

/// COFF File Header (immediately after PE signature)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct CoffHeader {
    /// Target machine type
    pub machine: u16,
    /// Number of sections
    pub number_of_sections: u16,
    /// Timestamp (seconds since 1970)
    pub time_date_stamp: u32,
    /// Offset to symbol table (usually 0 for images)
    pub pointer_to_symbol_table: u32,
    /// Number of symbols
    pub number_of_symbols: u32,
    /// Size of optional header
    pub size_of_optional_header: u16,
    /// Characteristics flags
    pub characteristics: u16,
}

/// Machine types
pub mod machine {
    pub const UNKNOWN: u16 = 0x0;
    pub const I386: u16 = 0x14c;
    pub const AMD64: u16 = 0x8664;
    pub const ARM: u16 = 0x1c0;
    pub const ARM64: u16 = 0xaa64;
}

/// COFF characteristics flags
pub mod characteristics {
    /// Relocation info stripped from file
    pub const RELOCS_STRIPPED: u16 = 0x0001;
    /// File is executable (no unresolved external references)
    pub const EXECUTABLE_IMAGE: u16 = 0x0002;
    /// Line numbers stripped
    pub const LINE_NUMS_STRIPPED: u16 = 0x0004;
    /// Local symbols stripped
    pub const LOCAL_SYMS_STRIPPED: u16 = 0x0008;
    /// Aggressively trim working set (obsolete)
    pub const AGGRESSIVE_WS_TRIM: u16 = 0x0010;
    /// Can handle > 2GB addresses
    pub const LARGE_ADDRESS_AWARE: u16 = 0x0020;
    /// 32-bit word machine
    pub const MACHINE_32BIT: u16 = 0x0100;
    /// Debugging info stripped
    pub const DEBUG_STRIPPED: u16 = 0x0200;
    /// Copy to swap if on removable media
    pub const REMOVABLE_RUN_FROM_SWAP: u16 = 0x0400;
    /// Copy to swap if on network
    pub const NET_RUN_FROM_SWAP: u16 = 0x0800;
    /// System file
    pub const SYSTEM: u16 = 0x1000;
    /// DLL
    pub const DLL: u16 = 0x2000;
    /// Only run on uniprocessor
    pub const UP_SYSTEM_ONLY: u16 = 0x4000;
}

impl CoffHeader {
    pub fn is_dll(&self) -> bool {
        (self.characteristics & characteristics::DLL) != 0
    }

    pub fn is_executable(&self) -> bool {
        (self.characteristics & characteristics::EXECUTABLE_IMAGE) != 0
    }

    pub fn is_64bit(&self) -> bool {
        self.machine == machine::AMD64 || self.machine == machine::ARM64
    }
}

/// Optional Header Magic values
pub const PE32_MAGIC: u16 = 0x10b;
pub const PE32PLUS_MAGIC: u16 = 0x20b;

/// Optional Header (PE32 - 32-bit)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct OptionalHeader32 {
    // Standard fields
    /// Magic number (0x10b for PE32)
    pub magic: u16,
    /// Linker major version
    pub major_linker_version: u8,
    /// Linker minor version
    pub minor_linker_version: u8,
    /// Size of code section
    pub size_of_code: u32,
    /// Size of initialized data
    pub size_of_initialized_data: u32,
    /// Size of uninitialized data
    pub size_of_uninitialized_data: u32,
    /// Entry point RVA
    pub address_of_entry_point: u32,
    /// Base of code section RVA
    pub base_of_code: u32,
    /// Base of data section RVA (PE32 only)
    pub base_of_data: u32,

    // Windows-specific fields
    /// Preferred load address
    pub image_base: u32,
    /// Section alignment in memory
    pub section_alignment: u32,
    /// File alignment on disk
    pub file_alignment: u32,
    /// Required OS major version
    pub major_operating_system_version: u16,
    /// Required OS minor version
    pub minor_operating_system_version: u16,
    /// Image major version
    pub major_image_version: u16,
    /// Image minor version
    pub minor_image_version: u16,
    /// Subsystem major version
    pub major_subsystem_version: u16,
    /// Subsystem minor version
    pub minor_subsystem_version: u16,
    /// Reserved
    pub win32_version_value: u32,
    /// Size of image in memory
    pub size_of_image: u32,
    /// Size of all headers
    pub size_of_headers: u32,
    /// Checksum
    pub checksum: u32,
    /// Subsystem type
    pub subsystem: u16,
    /// DLL characteristics
    pub dll_characteristics: u16,
    /// Stack reserve size
    pub size_of_stack_reserve: u32,
    /// Stack commit size
    pub size_of_stack_commit: u32,
    /// Heap reserve size
    pub size_of_heap_reserve: u32,
    /// Heap commit size
    pub size_of_heap_commit: u32,
    /// Loader flags (reserved)
    pub loader_flags: u32,
    /// Number of data directories
    pub number_of_rva_and_sizes: u32,
}

/// Optional Header (PE32+ - 64-bit)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct OptionalHeader64 {
    // Standard fields
    /// Magic number (0x20b for PE32+)
    pub magic: u16,
    /// Linker major version
    pub major_linker_version: u8,
    /// Linker minor version
    pub minor_linker_version: u8,
    /// Size of code section
    pub size_of_code: u32,
    /// Size of initialized data
    pub size_of_initialized_data: u32,
    /// Size of uninitialized data
    pub size_of_uninitialized_data: u32,
    /// Entry point RVA
    pub address_of_entry_point: u32,
    /// Base of code section RVA
    pub base_of_code: u32,

    // Windows-specific fields (note: no base_of_data, image_base is 64-bit)
    /// Preferred load address (64-bit)
    pub image_base: u64,
    /// Section alignment in memory
    pub section_alignment: u32,
    /// File alignment on disk
    pub file_alignment: u32,
    /// Required OS major version
    pub major_operating_system_version: u16,
    /// Required OS minor version
    pub minor_operating_system_version: u16,
    /// Image major version
    pub major_image_version: u16,
    /// Image minor version
    pub minor_image_version: u16,
    /// Subsystem major version
    pub major_subsystem_version: u16,
    /// Subsystem minor version
    pub minor_subsystem_version: u16,
    /// Reserved
    pub win32_version_value: u32,
    /// Size of image in memory
    pub size_of_image: u32,
    /// Size of all headers
    pub size_of_headers: u32,
    /// Checksum
    pub checksum: u32,
    /// Subsystem type
    pub subsystem: u16,
    /// DLL characteristics
    pub dll_characteristics: u16,
    /// Stack reserve size (64-bit)
    pub size_of_stack_reserve: u64,
    /// Stack commit size (64-bit)
    pub size_of_stack_commit: u64,
    /// Heap reserve size (64-bit)
    pub size_of_heap_reserve: u64,
    /// Heap commit size (64-bit)
    pub size_of_heap_commit: u64,
    /// Loader flags (reserved)
    pub loader_flags: u32,
    /// Number of data directories
    pub number_of_rva_and_sizes: u32,
}

/// DLL Characteristics flags
pub mod dll_characteristics {
    /// Image can handle a high entropy 64-bit virtual address space
    pub const HIGH_ENTROPY_VA: u16 = 0x0020;
    /// DLL can be relocated at load time
    pub const DYNAMIC_BASE: u16 = 0x0040;
    /// Code Integrity checks enforced
    pub const FORCE_INTEGRITY: u16 = 0x0080;
    /// Image is NX compatible
    pub const NX_COMPAT: u16 = 0x0100;
    /// No isolation
    pub const NO_ISOLATION: u16 = 0x0200;
    /// No SEH
    pub const NO_SEH: u16 = 0x0400;
    /// Do not bind
    pub const NO_BIND: u16 = 0x0800;
    /// Image should execute in AppContainer
    pub const APPCONTAINER: u16 = 0x1000;
    /// WDM driver
    pub const WDM_DRIVER: u16 = 0x2000;
    /// Image supports Control Flow Guard
    pub const GUARD_CF: u16 = 0x4000;
    /// Terminal Server aware
    pub const TERMINAL_SERVER_AWARE: u16 = 0x8000;
}

/// Data Directory indices
pub mod data_directory {
    pub const EXPORT: usize = 0;
    pub const IMPORT: usize = 1;
    pub const RESOURCE: usize = 2;
    pub const EXCEPTION: usize = 3;
    pub const SECURITY: usize = 4;
    pub const BASERELOC: usize = 5;
    pub const DEBUG: usize = 6;
    pub const ARCHITECTURE: usize = 7;
    pub const GLOBALPTR: usize = 8;
    pub const TLS: usize = 9;
    pub const LOAD_CONFIG: usize = 10;
    pub const BOUND_IMPORT: usize = 11;
    pub const IAT: usize = 12;
    pub const DELAY_IMPORT: usize = 13;
    pub const CLR_RUNTIME: usize = 14;
    pub const RESERVED: usize = 15;
    pub const COUNT: usize = 16;
}

/// Data Directory Entry
#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Default)]
pub struct DataDirectory {
    /// RVA of the data
    pub virtual_address: u32,
    /// Size of the data
    pub size: u32,
}

/// Section Header
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct SectionHeader {
    /// Section name (8 bytes, null-padded)
    pub name: [u8; 8],
    /// Virtual size (or physical address for objects)
    pub virtual_size: u32,
    /// RVA of section start
    pub virtual_address: u32,
    /// Size of raw data on disk
    pub size_of_raw_data: u32,
    /// File offset to raw data
    pub pointer_to_raw_data: u32,
    /// File offset to relocations
    pub pointer_to_relocations: u32,
    /// File offset to line numbers
    pub pointer_to_linenumbers: u32,
    /// Number of relocations
    pub number_of_relocations: u16,
    /// Number of line numbers
    pub number_of_linenumbers: u16,
    /// Section characteristics flags
    pub characteristics: u32,
}

/// Section characteristics flags
pub mod section_characteristics {
    /// Contains executable code
    pub const CODE: u32 = 0x00000020;
    /// Contains initialized data
    pub const INITIALIZED_DATA: u32 = 0x00000040;
    /// Contains uninitialized data
    pub const UNINITIALIZED_DATA: u32 = 0x00000080;
    /// Section can be discarded
    pub const DISCARDABLE: u32 = 0x02000000;
    /// Section cannot be cached
    pub const NOT_CACHED: u32 = 0x04000000;
    /// Section is not pageable
    pub const NOT_PAGED: u32 = 0x08000000;
    /// Section can be shared
    pub const SHARED: u32 = 0x10000000;
    /// Section is executable
    pub const EXECUTE: u32 = 0x20000000;
    /// Section is readable
    pub const READ: u32 = 0x40000000;
    /// Section is writable
    pub const WRITE: u32 = 0x80000000;
}

impl SectionHeader {
    pub fn name_str(&self) -> String {
        let mut name = String::new();
        for &b in &self.name {
            if b == 0 {
                break;
            }
            name.push(b as char);
        }
        name
    }

    pub fn is_code(&self) -> bool {
        (self.characteristics & section_characteristics::CODE) != 0
    }

    pub fn is_readable(&self) -> bool {
        (self.characteristics & section_characteristics::READ) != 0
    }

    pub fn is_writable(&self) -> bool {
        (self.characteristics & section_characteristics::WRITE) != 0
    }

    pub fn is_executable(&self) -> bool {
        (self.characteristics & section_characteristics::EXECUTE) != 0
    }
}

/// Import Directory Entry
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ImportDescriptor {
    /// RVA to Import Lookup Table (or Import Name Table)
    pub original_first_thunk: u32,
    /// Time/date stamp (0 if not bound)
    pub time_date_stamp: u32,
    /// Forwarder chain index (-1 if no forwarders)
    pub forwarder_chain: u32,
    /// RVA to DLL name string
    pub name: u32,
    /// RVA to Import Address Table (IAT)
    pub first_thunk: u32,
}

impl ImportDescriptor {
    pub fn is_null(&self) -> bool {
        self.original_first_thunk == 0 && self.name == 0 && self.first_thunk == 0
    }
}

/// Import by Name entry (after hint)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ImportByName {
    /// Index into export name pointer table (hint)
    pub hint: u16,
    // Followed by null-terminated name string
}

/// Export Directory
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ExportDirectory {
    /// Reserved (0)
    pub characteristics: u32,
    /// Time/date stamp
    pub time_date_stamp: u32,
    /// Major version
    pub major_version: u16,
    /// Minor version
    pub minor_version: u16,
    /// RVA to DLL name
    pub name: u32,
    /// Starting ordinal number
    pub base: u32,
    /// Number of exported functions
    pub number_of_functions: u32,
    /// Number of exported names
    pub number_of_names: u32,
    /// RVA to Export Address Table
    pub address_of_functions: u32,
    /// RVA to Export Name Pointer Table
    pub address_of_names: u32,
    /// RVA to Ordinal Table
    pub address_of_name_ordinals: u32,
}

/// Base Relocation Block
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct BaseRelocation {
    /// RVA of page
    pub virtual_address: u32,
    /// Size of block including header
    pub size_of_block: u32,
}

/// Base relocation types
pub mod relocation_type {
    pub const ABSOLUTE: u16 = 0;
    pub const HIGH: u16 = 1;
    pub const LOW: u16 = 2;
    pub const HIGHLOW: u16 = 3;
    pub const HIGHADJ: u16 = 4;
    pub const DIR64: u16 = 10;
}

/// TLS Directory (32-bit)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct TlsDirectory32 {
    pub start_address_of_raw_data: u32,
    pub end_address_of_raw_data: u32,
    pub address_of_index: u32,
    pub address_of_callbacks: u32,
    pub size_of_zero_fill: u32,
    pub characteristics: u32,
}

/// TLS Directory (64-bit)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct TlsDirectory64 {
    pub start_address_of_raw_data: u64,
    pub end_address_of_raw_data: u64,
    pub address_of_index: u64,
    pub address_of_callbacks: u64,
    pub size_of_zero_fill: u32,
    pub characteristics: u32,
}

/// PE parsing errors
#[derive(Debug, Clone)]
pub enum PeError {
    InvalidDosSignature,
    InvalidPeSignature,
    InvalidOptionalHeaderMagic,
    UnsupportedMachine(u16),
    InvalidSectionCount,
    BufferTooSmall,
    InvalidRva(u32),
    InvalidImport(String),
    InvalidExport(String),
}

/// Parsed PE file representation
#[derive(Debug)]
pub struct PeFile {
    pub is_64bit: bool,
    pub machine: u16,
    pub subsystem: u16,
    pub entry_point: u64,
    pub image_base: u64,
    pub size_of_image: u32,
    pub size_of_headers: u32,
    pub section_alignment: u32,
    pub file_alignment: u32,
    pub is_dll: bool,
    pub characteristics: u16,
    pub dll_characteristics: u16,
    pub stack_reserve: u64,
    pub stack_commit: u64,
    pub heap_reserve: u64,
    pub heap_commit: u64,
    pub data_directories: [DataDirectory; data_directory::COUNT],
    pub sections: Vec<SectionHeader>,
}

impl PeFile {
    /// Parse a PE file from a byte buffer
    pub fn parse(data: &[u8]) -> Result<Self, PeError> {
        if data.len() < size_of::<DosHeader>() {
            return Err(PeError::BufferTooSmall);
        }

        // Parse DOS header
        let dos_header = unsafe { &*(data.as_ptr() as *const DosHeader) };
        if !dos_header.is_valid() {
            return Err(PeError::InvalidDosSignature);
        }

        let pe_offset = dos_header.pe_offset();
        if pe_offset + 4 > data.len() {
            return Err(PeError::BufferTooSmall);
        }

        // Verify PE signature
        let pe_sig = unsafe { *(data.as_ptr().add(pe_offset) as *const u32) };
        if pe_sig != PE_SIGNATURE {
            return Err(PeError::InvalidPeSignature);
        }

        // Parse COFF header
        let coff_offset = pe_offset + 4;
        if coff_offset + size_of::<CoffHeader>() > data.len() {
            return Err(PeError::BufferTooSmall);
        }
        let coff_header = unsafe { &*(data.as_ptr().add(coff_offset) as *const CoffHeader) };

        // Verify machine type
        match coff_header.machine {
            machine::I386 | machine::AMD64 | machine::ARM | machine::ARM64 => {}
            m => return Err(PeError::UnsupportedMachine(m)),
        }

        // Parse Optional Header
        let optional_offset = coff_offset + size_of::<CoffHeader>();
        if optional_offset + 2 > data.len() {
            return Err(PeError::BufferTooSmall);
        }

        let magic = unsafe { *(data.as_ptr().add(optional_offset) as *const u16) };

        let (is_64bit, entry_point, image_base, size_of_image, size_of_headers,
             section_alignment, file_alignment, subsystem, dll_chars,
             stack_reserve, stack_commit, heap_reserve, heap_commit,
             num_data_dirs, data_dir_offset) = match magic {
            PE32_MAGIC => {
                if optional_offset + size_of::<OptionalHeader32>() > data.len() {
                    return Err(PeError::BufferTooSmall);
                }
                let opt = unsafe { &*(data.as_ptr().add(optional_offset) as *const OptionalHeader32) };
                (
                    false,
                    opt.address_of_entry_point as u64,
                    opt.image_base as u64,
                    opt.size_of_image,
                    opt.size_of_headers,
                    opt.section_alignment,
                    opt.file_alignment,
                    opt.subsystem,
                    opt.dll_characteristics,
                    opt.size_of_stack_reserve as u64,
                    opt.size_of_stack_commit as u64,
                    opt.size_of_heap_reserve as u64,
                    opt.size_of_heap_commit as u64,
                    opt.number_of_rva_and_sizes as usize,
                    optional_offset + size_of::<OptionalHeader32>(),
                )
            }
            PE32PLUS_MAGIC => {
                if optional_offset + size_of::<OptionalHeader64>() > data.len() {
                    return Err(PeError::BufferTooSmall);
                }
                let opt = unsafe { &*(data.as_ptr().add(optional_offset) as *const OptionalHeader64) };
                (
                    true,
                    opt.address_of_entry_point as u64,
                    opt.image_base,
                    opt.size_of_image,
                    opt.size_of_headers,
                    opt.section_alignment,
                    opt.file_alignment,
                    opt.subsystem,
                    opt.dll_characteristics,
                    opt.size_of_stack_reserve,
                    opt.size_of_stack_commit,
                    opt.size_of_heap_reserve,
                    opt.size_of_heap_commit,
                    opt.number_of_rva_and_sizes as usize,
                    optional_offset + size_of::<OptionalHeader64>(),
                )
            }
            _ => return Err(PeError::InvalidOptionalHeaderMagic),
        };

        // Parse data directories
        let mut data_directories = [DataDirectory::default(); data_directory::COUNT];
        let num_dirs = core::cmp::min(num_data_dirs, data_directory::COUNT);
        if data_dir_offset + num_dirs * size_of::<DataDirectory>() > data.len() {
            return Err(PeError::BufferTooSmall);
        }

        for i in 0..num_dirs {
            let dir_ptr = unsafe {
                data.as_ptr().add(data_dir_offset + i * size_of::<DataDirectory>())
            } as *const DataDirectory;
            data_directories[i] = unsafe { *dir_ptr };
        }

        // Parse section headers
        let section_table_offset = optional_offset + coff_header.size_of_optional_header as usize;
        let num_sections = coff_header.number_of_sections as usize;

        if num_sections > 96 {
            return Err(PeError::InvalidSectionCount);
        }

        if section_table_offset + num_sections * size_of::<SectionHeader>() > data.len() {
            return Err(PeError::BufferTooSmall);
        }

        let mut sections = Vec::with_capacity(num_sections);
        for i in 0..num_sections {
            let section_ptr = unsafe {
                data.as_ptr().add(section_table_offset + i * size_of::<SectionHeader>())
            } as *const SectionHeader;
            sections.push(unsafe { *section_ptr });
        }

        Ok(PeFile {
            is_64bit,
            machine: coff_header.machine,
            subsystem,
            entry_point,
            image_base,
            size_of_image,
            size_of_headers,
            section_alignment,
            file_alignment,
            is_dll: coff_header.is_dll(),
            characteristics: coff_header.characteristics,
            dll_characteristics: dll_chars,
            stack_reserve,
            stack_commit,
            heap_reserve,
            heap_commit,
            data_directories,
            sections,
        })
    }

    /// Convert RVA to file offset
    pub fn rva_to_offset(&self, rva: u32) -> Option<usize> {
        for section in &self.sections {
            let section_start = section.virtual_address;
            let section_end = section_start + section.virtual_size;
            if rva >= section_start && rva < section_end {
                let offset_in_section = rva - section_start;
                return Some((section.pointer_to_raw_data + offset_in_section) as usize);
            }
        }
        None
    }

    /// Get section containing RVA
    pub fn section_for_rva(&self, rva: u32) -> Option<&SectionHeader> {
        for section in &self.sections {
            let section_start = section.virtual_address;
            let section_end = section_start + section.virtual_size;
            if rva >= section_start && rva < section_end {
                return Some(section);
            }
        }
        None
    }

    /// Read null-terminated string at RVA
    pub fn read_string_at_rva(&self, data: &[u8], rva: u32) -> Option<String> {
        let offset = self.rva_to_offset(rva)?;
        let mut s = String::new();
        let mut i = offset;
        while i < data.len() {
            let b = data[i];
            if b == 0 {
                break;
            }
            s.push(b as char);
            i += 1;
        }
        Some(s)
    }

    /// Get subsystem as enum
    pub fn get_subsystem(&self) -> super::WindowsSubsystem {
        super::WindowsSubsystem::from_u16(self.subsystem)
    }

    /// Check if ASLR (dynamic base) is supported
    pub fn supports_aslr(&self) -> bool {
        (self.dll_characteristics & dll_characteristics::DYNAMIC_BASE) != 0
    }

    /// Check if NX (DEP) compatible
    pub fn is_nx_compatible(&self) -> bool {
        (self.dll_characteristics & dll_characteristics::NX_COMPAT) != 0
    }
}

/// Parse import table from PE file
pub fn parse_imports(pe: &PeFile, data: &[u8]) -> Result<Vec<(String, Vec<String>)>, PeError> {
    let import_dir = pe.data_directories[data_directory::IMPORT];
    if import_dir.virtual_address == 0 || import_dir.size == 0 {
        return Ok(vec![]);
    }

    let mut imports = Vec::new();
    let mut desc_offset = pe.rva_to_offset(import_dir.virtual_address)
        .ok_or(PeError::InvalidRva(import_dir.virtual_address))?;

    loop {
        if desc_offset + size_of::<ImportDescriptor>() > data.len() {
            break;
        }

        let desc = unsafe { &*(data.as_ptr().add(desc_offset) as *const ImportDescriptor) };
        if desc.is_null() {
            break;
        }

        let dll_name = pe.read_string_at_rva(data, desc.name)
            .ok_or_else(|| PeError::InvalidImport(String::from("Invalid DLL name RVA")))?;

        let mut functions = Vec::new();
        let ilt_rva = if desc.original_first_thunk != 0 {
            desc.original_first_thunk
        } else {
            desc.first_thunk
        };

        let mut thunk_offset = pe.rva_to_offset(ilt_rva)
            .ok_or(PeError::InvalidRva(ilt_rva))?;

        if pe.is_64bit {
            // 64-bit: thunks are 8 bytes
            loop {
                if thunk_offset + 8 > data.len() {
                    break;
                }
                let thunk = unsafe { *(data.as_ptr().add(thunk_offset) as *const u64) };
                if thunk == 0 {
                    break;
                }

                // Check if import by ordinal (bit 63 set)
                if (thunk & 0x8000_0000_0000_0000) != 0 {
                    let ordinal = (thunk & 0xFFFF) as u16;
                    functions.push(alloc::format!("#{}", ordinal));
                } else {
                    // Import by name
                    let hint_rva = (thunk & 0x7FFF_FFFF) as u32;
                    if let Some(name_offset) = pe.rva_to_offset(hint_rva) {
                        // Skip hint (2 bytes) and read name
                        if name_offset + 2 < data.len() {
                            let mut name = String::new();
                            let mut i = name_offset + 2;
                            while i < data.len() && data[i] != 0 {
                                name.push(data[i] as char);
                                i += 1;
                            }
                            functions.push(name);
                        }
                    }
                }
                thunk_offset += 8;
            }
        } else {
            // 32-bit: thunks are 4 bytes
            loop {
                if thunk_offset + 4 > data.len() {
                    break;
                }
                let thunk = unsafe { *(data.as_ptr().add(thunk_offset) as *const u32) };
                if thunk == 0 {
                    break;
                }

                // Check if import by ordinal (bit 31 set)
                if (thunk & 0x8000_0000) != 0 {
                    let ordinal = (thunk & 0xFFFF) as u16;
                    functions.push(alloc::format!("#{}", ordinal));
                } else {
                    // Import by name
                    if let Some(name_offset) = pe.rva_to_offset(thunk) {
                        // Skip hint (2 bytes) and read name
                        if name_offset + 2 < data.len() {
                            let mut name = String::new();
                            let mut i = name_offset + 2;
                            while i < data.len() && data[i] != 0 {
                                name.push(data[i] as char);
                                i += 1;
                            }
                            functions.push(name);
                        }
                    }
                }
                thunk_offset += 4;
            }
        }

        imports.push((dll_name, functions));
        desc_offset += size_of::<ImportDescriptor>();
    }

    Ok(imports)
}

/// Parse export table from PE file (for DLLs)
pub fn parse_exports(pe: &PeFile, data: &[u8]) -> Result<Vec<(String, u32)>, PeError> {
    let export_dir = pe.data_directories[data_directory::EXPORT];
    if export_dir.virtual_address == 0 || export_dir.size == 0 {
        return Ok(vec![]);
    }

    let dir_offset = pe.rva_to_offset(export_dir.virtual_address)
        .ok_or(PeError::InvalidRva(export_dir.virtual_address))?;

    if dir_offset + size_of::<ExportDirectory>() > data.len() {
        return Err(PeError::BufferTooSmall);
    }

    let export = unsafe { &*(data.as_ptr().add(dir_offset) as *const ExportDirectory) };

    let num_names = export.number_of_names as usize;
    let num_functions = export.number_of_functions as usize;

    // Get arrays
    let names_offset = pe.rva_to_offset(export.address_of_names)
        .ok_or(PeError::InvalidRva(export.address_of_names))?;
    let ordinals_offset = pe.rva_to_offset(export.address_of_name_ordinals)
        .ok_or(PeError::InvalidRva(export.address_of_name_ordinals))?;
    let funcs_offset = pe.rva_to_offset(export.address_of_functions)
        .ok_or(PeError::InvalidRva(export.address_of_functions))?;

    let mut exports = Vec::new();

    for i in 0..num_names {
        // Get name RVA
        let name_rva_offset = names_offset + i * 4;
        if name_rva_offset + 4 > data.len() {
            break;
        }
        let name_rva = unsafe { *(data.as_ptr().add(name_rva_offset) as *const u32) };

        // Get ordinal
        let ord_offset = ordinals_offset + i * 2;
        if ord_offset + 2 > data.len() {
            break;
        }
        let ordinal = unsafe { *(data.as_ptr().add(ord_offset) as *const u16) } as usize;

        // Get function RVA
        if ordinal >= num_functions {
            continue;
        }
        let func_rva_offset = funcs_offset + ordinal * 4;
        if func_rva_offset + 4 > data.len() {
            continue;
        }
        let func_rva = unsafe { *(data.as_ptr().add(func_rva_offset) as *const u32) };

        // Read name
        if let Some(name) = pe.read_string_at_rva(data, name_rva) {
            exports.push((name, func_rva));
        }
    }

    Ok(exports)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal valid PE header for testing
    fn create_minimal_pe() -> Vec<u8> {
        let mut data = vec![0u8; 512];

        // DOS Header
        data[0] = b'M';
        data[1] = b'Z';
        // e_lfanew at offset 0x3C = PE header at 0x80
        data[0x3C] = 0x80;

        // PE Signature at 0x80
        data[0x80] = b'P';
        data[0x81] = b'E';
        data[0x82] = 0;
        data[0x83] = 0;

        // COFF Header at 0x84
        // Machine: AMD64 (0x8664)
        data[0x84] = 0x64;
        data[0x85] = 0x86;
        // NumberOfSections: 1
        data[0x86] = 1;
        data[0x87] = 0;
        // SizeOfOptionalHeader: 240 (0xF0) for PE32+
        data[0x94] = 0xF0;
        data[0x95] = 0x00;
        // Characteristics: executable
        data[0x96] = 0x22;
        data[0x97] = 0x00;

        // Optional Header at 0x98
        // Magic: PE32+ (0x20B)
        data[0x98] = 0x0B;
        data[0x99] = 0x02;

        // Rest filled with zeros (default values)
        data
    }
}
