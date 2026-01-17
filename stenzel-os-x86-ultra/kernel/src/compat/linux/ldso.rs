//! LD.so Dynamic Linker Emulation
//!
//! Provides dynamic linking support for ELF executables.
//! This is a simplified implementation that handles:
//! - Loading shared libraries
//! - Symbol resolution
//! - Relocations
//! - Running init/fini functions

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use spin::Mutex;

use super::elf_dyn::{
    DynamicInfo, Elf64Rela, Elf64Sym, r_x86_64, read_string,
    elf_hash, gnu_hash,
};

/// Global dynamic linker instance
pub static DYNAMIC_LINKER: Mutex<DynamicLinker> = Mutex::new(DynamicLinker::new());

/// Standard library search paths
pub const LIBRARY_PATHS: &[&str] = &[
    "/lib",
    "/lib64",
    "/usr/lib",
    "/usr/lib64",
    "/lib/x86_64-linux-gnu",
    "/usr/lib/x86_64-linux-gnu",
];

/// A loaded shared object
#[derive(Debug)]
pub struct SharedObject {
    /// Name/path of the shared object
    pub name: String,
    /// Base address where the object is loaded
    pub base: u64,
    /// End address of the loaded object
    pub end: u64,
    /// Dynamic section info
    pub dyn_info: DynamicInfo,
    /// Entry point (for executables)
    pub entry: u64,
    /// Reference count
    pub refcount: usize,
    /// Has been initialized (init functions called)
    pub initialized: bool,
    /// Symbol table (cached for quick lookup)
    pub symbols: Vec<Elf64Sym>,
    /// String table data
    pub strtab_data: Vec<u8>,
    /// Is this the main executable?
    pub is_main: bool,
}

impl SharedObject {
    pub fn new(name: &str, base: u64, end: u64) -> Self {
        SharedObject {
            name: name.to_string(),
            base,
            end,
            dyn_info: DynamicInfo::default(),
            entry: 0,
            refcount: 1,
            initialized: false,
            symbols: Vec::new(),
            strtab_data: Vec::new(),
            is_main: false,
        }
    }

    /// Get symbol name by index
    pub fn get_symbol_name(&self, sym: &Elf64Sym) -> Option<String> {
        if sym.st_name == 0 || self.strtab_data.is_empty() {
            return None;
        }
        read_string(&self.strtab_data, sym.st_name as usize)
    }

    /// Find a symbol by name
    pub fn find_symbol(&self, name: &str) -> Option<(u64, &Elf64Sym)> {
        for sym in &self.symbols {
            if let Some(sym_name) = self.get_symbol_name(sym) {
                if sym_name == name && sym.st_value != 0 {
                    return Some((self.base + sym.st_value, sym));
                }
            }
        }
        None
    }
}

/// The dynamic linker
pub struct DynamicLinker {
    /// Loaded shared objects
    objects: BTreeMap<String, SharedObject>,
    /// Handle counter for dlopen
    next_handle: u64,
    /// Handle to object mapping
    handles: BTreeMap<u64, String>,
    /// Last error message
    last_error: Option<String>,
    /// Global symbol table (for quick lookup)
    global_symbols: BTreeMap<String, (u64, String)>,  // name -> (address, soname)
}

impl DynamicLinker {
    pub const fn new() -> Self {
        DynamicLinker {
            objects: BTreeMap::new(),
            next_handle: 1,
            handles: BTreeMap::new(),
            last_error: None,
            global_symbols: BTreeMap::new(),
        }
    }

    /// Load a shared object
    pub fn load(&mut self, name: &str, base: u64, end: u64) -> Result<u64, String> {
        crate::kprintln!("ldso: loading {} at {:#x}-{:#x}", name, base, end);

        // Check if already loaded
        if let Some(obj) = self.objects.get_mut(name) {
            obj.refcount += 1;
            return Ok(base);
        }

        let obj = SharedObject::new(name, base, end);
        self.objects.insert(name.to_string(), obj);

        // Generate handle
        let handle = self.next_handle;
        self.next_handle += 1;
        self.handles.insert(handle, name.to_string());

        Ok(handle)
    }

    /// Set dynamic info for a loaded object
    pub fn set_dynamic_info(&mut self, name: &str, dyn_info: DynamicInfo) {
        if let Some(obj) = self.objects.get_mut(name) {
            obj.dyn_info = dyn_info;
        }
    }

    /// Set symbol table for a loaded object
    pub fn set_symbols(&mut self, name: &str, symbols: Vec<Elf64Sym>, strtab: Vec<u8>) {
        if let Some(obj) = self.objects.get_mut(name) {
            obj.symbols = symbols;
            obj.strtab_data = strtab;

            // Add to global symbol table
            for sym in &obj.symbols {
                if sym.st_value != 0 {
                    if let Some(sym_name) = obj.get_symbol_name(sym) {
                        let addr = obj.base + sym.st_value;
                        self.global_symbols.insert(sym_name, (addr, name.to_string()));
                    }
                }
            }
        }
    }

    /// Apply relocations for a loaded object
    pub fn apply_relocations(&mut self, name: &str, rela_data: &[u8], rela_count: usize) -> Result<(), String> {
        let base = self.objects.get(name)
            .map(|o| o.base)
            .ok_or_else(|| "Object not found".to_string())?;

        crate::kprintln!("ldso: applying {} relocations for {}", rela_count, name);

        for i in 0..rela_count {
            let offset = i * core::mem::size_of::<Elf64Rela>();
            if offset + core::mem::size_of::<Elf64Rela>() > rela_data.len() {
                break;
            }

            let rela = unsafe { &*(rela_data.as_ptr().add(offset) as *const Elf64Rela) };
            self.apply_single_relocation(name, rela, base)?;
        }

        Ok(())
    }

    fn apply_single_relocation(&self, name: &str, rela: &Elf64Rela, base: u64) -> Result<(), String> {
        let rel_type = rela.rel_type();
        let target = base + rela.r_offset;

        match rel_type {
            r_x86_64::NONE => {
                // No relocation needed
            }

            r_x86_64::RELATIVE => {
                // R_X86_64_RELATIVE: *target = base + addend
                let value = (base as i64 + rela.r_addend) as u64;
                unsafe {
                    let ptr = target as *mut u64;
                    *ptr = value;
                }
            }

            r_x86_64::GLOB_DAT | r_x86_64::JUMP_SLOT => {
                // R_X86_64_GLOB_DAT / R_X86_64_JUMP_SLOT: *target = symbol_address
                let sym_addr = self.resolve_symbol_reloc(name, rela, base);
                if let Some(addr) = sym_addr {
                    unsafe {
                        let ptr = target as *mut u64;
                        *ptr = addr;
                    }
                }
            }

            r_x86_64::R64 => {
                // R_X86_64_64: *target = symbol_address + addend
                if let Some(addr) = self.resolve_symbol_reloc(name, rela, base) {
                    let value = (addr as i64 + rela.r_addend) as u64;
                    unsafe {
                        let ptr = target as *mut u64;
                        *ptr = value;
                    }
                }
            }

            r_x86_64::PC32 => {
                // R_X86_64_PC32: *target = symbol_address + addend - target
                // 32-bit PC-relative relocation
                if let Some(addr) = self.resolve_symbol_reloc(name, rela, base) {
                    let value = ((addr as i64 + rela.r_addend) - target as i64) as i32;
                    unsafe {
                        let ptr = target as *mut i32;
                        *ptr = value;
                    }
                }
            }

            r_x86_64::PLT32 => {
                // R_X86_64_PLT32: *target = L + addend - target (L = PLT entry or symbol)
                // Treated same as PC32 when symbol is resolved
                if let Some(addr) = self.resolve_symbol_reloc(name, rela, base) {
                    let value = ((addr as i64 + rela.r_addend) - target as i64) as i32;
                    unsafe {
                        let ptr = target as *mut i32;
                        *ptr = value;
                    }
                }
            }

            r_x86_64::R32 | r_x86_64::R32S => {
                // R_X86_64_32/32S: *target = (symbol_address + addend) as 32-bit
                if let Some(addr) = self.resolve_symbol_reloc(name, rela, base) {
                    let value = (addr as i64 + rela.r_addend) as u32;
                    unsafe {
                        let ptr = target as *mut u32;
                        *ptr = value;
                    }
                }
            }

            r_x86_64::PC64 => {
                // R_X86_64_PC64: *target = symbol_address + addend - target
                // 64-bit PC-relative relocation
                if let Some(addr) = self.resolve_symbol_reloc(name, rela, base) {
                    let value = (addr as i64 + rela.r_addend - target as i64) as u64;
                    unsafe {
                        let ptr = target as *mut u64;
                        *ptr = value;
                    }
                }
            }

            r_x86_64::GOTPCREL => {
                // R_X86_64_GOTPCREL: *target = GOT[symbol] + addend - target
                // PC-relative GOT entry - for now, treat as direct symbol access
                if let Some(addr) = self.resolve_symbol_reloc(name, rela, base) {
                    // Simplified: directly use symbol address (assumes GOT entry holds symbol address)
                    let value = ((addr as i64 + rela.r_addend) - target as i64) as i32;
                    unsafe {
                        let ptr = target as *mut i32;
                        *ptr = value;
                    }
                }
            }

            r_x86_64::COPY => {
                // R_X86_64_COPY: Copy data from shared library to executable's BSS
                let sym_idx = rela.sym() as usize;
                if let Some(obj) = self.objects.get(name) {
                    if sym_idx > 0 && sym_idx < obj.symbols.len() {
                        let sym = &obj.symbols[sym_idx];
                        let size = sym.st_size as usize;
                        if let Some(sym_name) = obj.get_symbol_name(sym) {
                            // Find symbol in other loaded objects
                            if let Some(src_addr) = self.find_symbol_in_others(name, &sym_name) {
                                if size > 0 {
                                    unsafe {
                                        core::ptr::copy_nonoverlapping(
                                            src_addr as *const u8,
                                            target as *mut u8,
                                            size,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }

            r_x86_64::IRELATIVE => {
                // R_X86_64_IRELATIVE: *target = resolver()(base + addend)
                // GNU indirect function - call the resolver at runtime
                let resolver = (base as i64 + rela.r_addend) as u64;
                // Call resolver function to get the actual address
                let resolver_fn: extern "C" fn() -> u64 = unsafe { core::mem::transmute(resolver) };
                let resolved = resolver_fn();
                unsafe {
                    let ptr = target as *mut u64;
                    *ptr = resolved;
                }
            }

            // TLS relocations
            r_x86_64::DTPMOD64 => {
                // R_X86_64_DTPMOD64: *target = module ID (for GD/LD TLS model)
                // For now, assume single module (ID = 1)
                unsafe {
                    let ptr = target as *mut u64;
                    *ptr = 1;
                }
            }

            r_x86_64::DTPOFF64 => {
                // R_X86_64_DTPOFF64: *target = TLS offset (for GD/LD TLS model)
                if let Some(obj) = self.objects.get(name) {
                    let sym_idx = rela.sym() as usize;
                    if sym_idx > 0 && sym_idx < obj.symbols.len() {
                        let sym = &obj.symbols[sym_idx];
                        let offset = (sym.st_value as i64 + rela.r_addend) as u64;
                        unsafe {
                            let ptr = target as *mut u64;
                            *ptr = offset;
                        }
                    }
                }
            }

            r_x86_64::TPOFF64 => {
                // R_X86_64_TPOFF64: *target = TLS offset from thread pointer (IE model)
                // For static TLS: offset = symbol_offset - tls_size
                if let Some(obj) = self.objects.get(name) {
                    let sym_idx = rela.sym() as usize;
                    if sym_idx > 0 && sym_idx < obj.symbols.len() {
                        let sym = &obj.symbols[sym_idx];
                        // Negative offset from TP for x86_64 (variant 2)
                        let offset = sym.st_value as i64 + rela.r_addend;
                        unsafe {
                            let ptr = target as *mut i64;
                            *ptr = offset;
                        }
                    }
                }
            }

            r_x86_64::TPOFF32 => {
                // R_X86_64_TPOFF32: 32-bit TLS offset from thread pointer
                if let Some(obj) = self.objects.get(name) {
                    let sym_idx = rela.sym() as usize;
                    if sym_idx > 0 && sym_idx < obj.symbols.len() {
                        let sym = &obj.symbols[sym_idx];
                        let offset = (sym.st_value as i64 + rela.r_addend) as i32;
                        unsafe {
                            let ptr = target as *mut i32;
                            *ptr = offset;
                        }
                    }
                }
            }

            r_x86_64::GOTTPOFF => {
                // R_X86_64_GOTTPOFF: *target = GOT[TLS_offset] + addend - target
                // PC-relative GOT entry for TLS IE access
                if let Some(obj) = self.objects.get(name) {
                    let sym_idx = rela.sym() as usize;
                    if sym_idx > 0 && sym_idx < obj.symbols.len() {
                        let sym = &obj.symbols[sym_idx];
                        // Simplified: store TLS offset directly
                        let tls_offset = sym.st_value as i64 + rela.r_addend;
                        let value = (tls_offset - target as i64) as i32;
                        unsafe {
                            let ptr = target as *mut i32;
                            *ptr = value;
                        }
                    }
                }
            }

            _ => {
                // Log unsupported relocation types
                crate::kprintln!("ldso: unsupported relocation type {} at {:#x}", rel_type, target);
            }
        }

        Ok(())
    }

    /// Resolve symbol for relocation
    fn resolve_symbol_reloc(&self, name: &str, rela: &Elf64Rela, base: u64) -> Option<u64> {
        let sym_idx = rela.sym() as usize;
        if sym_idx == 0 {
            return Some(base); // Symbol index 0 means use base address
        }

        if let Some(obj) = self.objects.get(name) {
            if sym_idx < obj.symbols.len() {
                let sym = &obj.symbols[sym_idx];
                if let Some(sym_name) = obj.get_symbol_name(sym) {
                    // First try global symbols
                    if let Some((addr, _)) = self.global_symbols.get(&sym_name) {
                        return Some(*addr);
                    }
                    // Then try local symbol
                    if sym.st_value != 0 {
                        return Some(base + sym.st_value);
                    }
                    // Search other objects
                    if let Some(addr) = self.find_symbol_in_others(name, &sym_name) {
                        return Some(addr);
                    }
                }
            }
        }
        None
    }

    /// Find a symbol in objects other than the specified one
    fn find_symbol_in_others(&self, exclude_name: &str, sym_name: &str) -> Option<u64> {
        for (obj_name, obj) in &self.objects {
            if obj_name != exclude_name {
                if let Some((addr, _sym)) = obj.find_symbol(sym_name) {
                    return Some(addr);
                }
            }
        }
        None
    }

    /// Look up a symbol by name
    pub fn lookup_symbol(&self, name: &str) -> Option<u64> {
        if let Some((addr, _)) = self.global_symbols.get(name) {
            return Some(*addr);
        }

        // Search through all loaded objects
        for (_, obj) in &self.objects {
            if let Some((addr, _)) = obj.find_symbol(name) {
                return Some(addr);
            }
        }

        None
    }

    /// Get the list of needed libraries for an object
    pub fn get_needed(&self, name: &str) -> Vec<String> {
        if let Some(obj) = self.objects.get(name) {
            let mut needed = Vec::new();
            for offset in &obj.dyn_info.needed {
                if let Some(lib_name) = read_string(&obj.strtab_data, *offset as usize) {
                    needed.push(lib_name);
                }
            }
            return needed;
        }
        Vec::new()
    }

    /// Mark an object as initialized
    pub fn mark_initialized(&mut self, name: &str) {
        if let Some(obj) = self.objects.get_mut(name) {
            obj.initialized = true;
        }
    }

    /// Get init functions for an object
    pub fn get_init_functions(&self, name: &str) -> Vec<u64> {
        let mut funcs = Vec::new();

        if let Some(obj) = self.objects.get(name) {
            // DT_INIT
            if obj.dyn_info.init != 0 {
                funcs.push(obj.base + obj.dyn_info.init);
            }

            // DT_INIT_ARRAY
            if obj.dyn_info.init_array != 0 && obj.dyn_info.init_arraysz > 0 {
                let array_addr = obj.base + obj.dyn_info.init_array;
                let count = obj.dyn_info.init_arraysz / 8;
                for i in 0..count {
                    let ptr = (array_addr + i * 8) as *const u64;
                    let func = unsafe { *ptr };
                    if func != 0 && func != u64::MAX {
                        funcs.push(func);
                    }
                }
            }
        }

        funcs
    }

    /// Get fini functions for an object (in reverse order)
    pub fn get_fini_functions(&self, name: &str) -> Vec<u64> {
        let mut funcs = Vec::new();

        if let Some(obj) = self.objects.get(name) {
            // DT_FINI_ARRAY (in reverse)
            if obj.dyn_info.fini_array != 0 && obj.dyn_info.fini_arraysz > 0 {
                let array_addr = obj.base + obj.dyn_info.fini_array;
                let count = obj.dyn_info.fini_arraysz / 8;
                for i in (0..count).rev() {
                    let ptr = (array_addr + i * 8) as *const u64;
                    let func = unsafe { *ptr };
                    if func != 0 && func != u64::MAX {
                        funcs.push(func);
                    }
                }
            }

            // DT_FINI
            if obj.dyn_info.fini != 0 {
                funcs.push(obj.base + obj.dyn_info.fini);
            }
        }

        funcs
    }

    /// Unload a shared object (decrement refcount)
    pub fn unload(&mut self, name: &str) -> bool {
        if let Some(obj) = self.objects.get_mut(name) {
            if obj.refcount > 1 {
                obj.refcount -= 1;
                return true;
            }
            // Would need to run fini functions and unmap memory
        }
        self.objects.remove(name);
        true
    }

    /// Get last error message
    pub fn get_error(&mut self) -> Option<String> {
        self.last_error.take()
    }

    /// Set error message
    pub fn set_error(&mut self, msg: &str) {
        self.last_error = Some(msg.to_string());
    }

    /// Get handle for an object
    pub fn get_handle(&self, name: &str) -> Option<u64> {
        for (handle, obj_name) in &self.handles {
            if obj_name == name {
                return Some(*handle);
            }
        }
        None
    }

    /// Get object name from handle
    pub fn get_name_from_handle(&self, handle: u64) -> Option<&String> {
        self.handles.get(&handle)
    }

    /// Check if object is loaded
    pub fn is_loaded(&self, name: &str) -> bool {
        self.objects.contains_key(name)
    }

    /// Get loaded objects count
    pub fn loaded_count(&self) -> usize {
        self.objects.len()
    }

    /// Get object by name
    pub fn get_object(&self, name: &str) -> Option<&SharedObject> {
        self.objects.get(name)
    }

    /// Iterate over all loaded objects
    pub fn iter_objects(&self) -> impl Iterator<Item = (&String, &SharedObject)> {
        self.objects.iter()
    }

    /// Find library in search paths
    pub fn find_library(name: &str) -> Option<String> {
        // If absolute path, use as-is
        if name.starts_with('/') {
            return Some(name.to_string());
        }

        // Search in library paths
        for path in LIBRARY_PATHS {
            let full_path = alloc::format!("{}/{}", path, name);
            // In a real implementation, we'd check if the file exists
            // For now, just return the path
            return Some(full_path);
        }

        None
    }
}

/// Initialize the dynamic linker
pub fn init() {
    let mut linker = DYNAMIC_LINKER.lock();
    crate::kprintln!("ldso: dynamic linker initialized");
    // Could preload common libraries here
    let _ = &mut linker;  // Suppress unused warning
}

/// Load a shared library (called from ELF loader)
pub fn load_library(name: &str, base: u64, end: u64) -> Result<u64, String> {
    DYNAMIC_LINKER.lock().load(name, base, end)
}

/// Look up a symbol
pub fn lookup(name: &str) -> Option<u64> {
    DYNAMIC_LINKER.lock().lookup_symbol(name)
}

/// Get interpreter path from ELF
pub fn get_interpreter(elf_data: &[u8]) -> Option<String> {
    use crate::process::elf::{Elf64Header, Elf64Phdr, PT_INTERP};
    use core::mem;

    let hdr = Elf64Header::from_bytes(elf_data)?;

    for i in 0..hdr.e_phnum {
        let ph_offset = hdr.e_phoff as usize + (i as usize) * hdr.e_phentsize as usize;
        if ph_offset + mem::size_of::<Elf64Phdr>() > elf_data.len() {
            continue;
        }

        let phdr = unsafe { &*(elf_data.as_ptr().add(ph_offset) as *const Elf64Phdr) };

        if phdr.p_type == PT_INTERP {
            let offset = phdr.p_offset as usize;
            let size = phdr.p_filesz as usize;
            if offset + size <= elf_data.len() {
                // Read null-terminated string
                let mut end = offset;
                while end < offset + size && elf_data[end] != 0 {
                    end += 1;
                }
                return String::from_utf8(elf_data[offset..end].to_vec()).ok();
            }
        }
    }

    None
}
