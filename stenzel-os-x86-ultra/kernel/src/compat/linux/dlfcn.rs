//! Dynamic Linking Functions (dlfcn.h)
//!
//! Implements dlopen, dlsym, dlclose, dlerror for dynamic library loading.

extern crate alloc;

use alloc::string::{String, ToString};
use super::ldso::DYNAMIC_LINKER;

/// dlopen flags
pub mod flags {
    /// Lazy binding - resolve symbols only when needed
    pub const RTLD_LAZY: i32 = 0x00001;
    /// Immediate binding - resolve all symbols on load
    pub const RTLD_NOW: i32 = 0x00002;
    /// Don't load the shared library if not already loaded
    pub const RTLD_NOLOAD: i32 = 0x00004;
    /// Don't delete object when handle is closed
    pub const RTLD_NODELETE: i32 = 0x01000;
    /// Make symbols available for subsequently loaded objects
    pub const RTLD_GLOBAL: i32 = 0x00100;
    /// Opposite of RTLD_GLOBAL (default)
    pub const RTLD_LOCAL: i32 = 0x00000;
    /// Find only in the object specified (for dlsym)
    pub const RTLD_NEXT: i32 = -1;
    /// Use default search order (for dlsym)
    pub const RTLD_DEFAULT: i32 = 0;
}

/// Last error message (thread-local in real implementation)
static mut DLERROR: Option<String> = None;

/// Open a dynamic library
///
/// Returns a handle that can be used with dlsym and dlclose.
/// Returns NULL (0) on error; use dlerror() to get the error message.
pub fn dlopen(filename: Option<&str>, _flags: i32) -> u64 {
    let mut linker = DYNAMIC_LINKER.lock();

    // Clear previous error
    unsafe { DLERROR = None; }

    match filename {
        Some(name) => {
            crate::kprintln!("dlfcn: dlopen(\"{}\")", name);

            // Check if already loaded
            if let Some(handle) = linker.get_handle(name) {
                return handle;
            }

            // Find library path
            let path = match super::ldso::DynamicLinker::find_library(name) {
                Some(p) => p,
                None => {
                    let err = alloc::format!("{}: cannot open shared object file: No such file or directory", name);
                    unsafe { DLERROR = Some(err); }
                    linker.set_error(&alloc::format!("Cannot find library: {}", name));
                    return 0;
                }
            };

            // In a real implementation, we would:
            // 1. Read the file from the filesystem
            // 2. Parse the ELF headers
            // 3. Map the segments into memory
            // 4. Apply relocations
            // 5. Run init functions

            // For now, return a stub handle
            // The actual loading would happen through the VFS/filesystem layer
            crate::kprintln!("dlfcn: would load {} from {}", name, path);

            // Create a placeholder
            match linker.load(name, 0x7f000000, 0x7f100000) {
                Ok(handle) => handle,
                Err(e) => {
                    unsafe { DLERROR = Some(e.clone()); }
                    0
                }
            }
        }
        None => {
            // NULL filename returns handle to main program
            crate::kprintln!("dlfcn: dlopen(NULL) - returning main program handle");
            // Return a special handle for the main program
            u64::MAX
        }
    }
}

/// Look up a symbol in a dynamic library
///
/// Returns the address of the symbol, or NULL (0) on error.
pub fn dlsym(handle: u64, symbol: &str) -> u64 {
    let linker = DYNAMIC_LINKER.lock();

    // Clear previous error
    unsafe { DLERROR = None; }

    crate::kprintln!("dlfcn: dlsym({:#x}, \"{}\")", handle, symbol);

    if handle == u64::MAX {
        // Main program handle - search all loaded objects
        if let Some(addr) = linker.lookup_symbol(symbol) {
            return addr;
        }
    } else if handle == flags::RTLD_NEXT as u64 {
        // Search starting from the next object after the caller
        // This would need caller information
        if let Some(addr) = linker.lookup_symbol(symbol) {
            return addr;
        }
    } else if handle == flags::RTLD_DEFAULT as u64 {
        // Default search order
        if let Some(addr) = linker.lookup_symbol(symbol) {
            return addr;
        }
    } else {
        // Specific handle
        if let Some(name) = linker.get_name_from_handle(handle) {
            // Search in that specific object
            if let Some(obj) = linker.get_object(name.as_str()) {
                if let Some((addr, _sym)) = obj.find_symbol(symbol) {
                    return addr;
                }
            }
        }
    }

    // Symbol not found
    let err = alloc::format!("undefined symbol: {}", symbol);
    unsafe { DLERROR = Some(err); }
    0
}

/// Close a dynamic library handle
///
/// Returns 0 on success, non-zero on error.
pub fn dlclose(handle: u64) -> i32 {
    let mut linker = DYNAMIC_LINKER.lock();

    // Clear previous error
    unsafe { DLERROR = None; }

    crate::kprintln!("dlfcn: dlclose({:#x})", handle);

    if handle == u64::MAX {
        // Can't close main program
        return 0;
    }

    if let Some(name) = linker.get_name_from_handle(handle).cloned() {
        if linker.unload(&name) {
            return 0;
        }
    }

    let err = "invalid handle".to_string();
    unsafe { DLERROR = Some(err); }
    -1
}

/// Get the last error message
///
/// Returns the error message, or NULL if no error occurred.
/// Note: In real glibc, this function clears the error after returning.
pub fn dlerror() -> Option<String> {
    unsafe { DLERROR.take() }
}

/// Get information about a symbol (dladdr)
#[derive(Debug, Default)]
pub struct DlInfo {
    /// Pathname of shared object containing address
    pub dli_fname: String,
    /// Base address at which shared object is loaded
    pub dli_fbase: u64,
    /// Name of symbol whose definition overlaps addr
    pub dli_sname: String,
    /// Exact address of symbol
    pub dli_saddr: u64,
}

/// Get information about a symbol from an address
pub fn dladdr(addr: u64) -> Option<DlInfo> {
    let linker = DYNAMIC_LINKER.lock();

    crate::kprintln!("dlfcn: dladdr({:#x})", addr);

    // Search through all loaded objects to find which one contains this address
    for (name, obj) in linker.iter_objects() {
        if addr >= obj.base && addr < obj.end {
            // Found the containing object
            let mut info = DlInfo {
                dli_fname: name.clone(),
                dli_fbase: obj.base,
                dli_sname: String::new(),
                dli_saddr: 0,
            };

            // Find the nearest symbol
            let offset = addr - obj.base;
            let mut best_sym: Option<(&Elf64Sym, u64)> = None;

            for sym in &obj.symbols {
                if sym.st_value != 0 && sym.st_value <= offset {
                    let distance = offset - sym.st_value;
                    if best_sym.is_none() || distance < best_sym.unwrap().1 {
                        best_sym = Some((sym, distance));
                    }
                }
            }

            if let Some((sym, _)) = best_sym {
                if let Some(sym_name) = obj.get_symbol_name(sym) {
                    info.dli_sname = sym_name;
                    info.dli_saddr = obj.base + sym.st_value;
                }
            }

            return Some(info);
        }
    }

    None
}

use super::elf_dyn::Elf64Sym;

/// dlinfo request codes
pub mod dlinfo_request {
    /// Get link map
    pub const RTLD_DI_LINKMAP: i32 = 2;
    /// Get origin
    pub const RTLD_DI_ORIGIN: i32 = 6;
    /// Get serial number
    pub const RTLD_DI_SERINFO: i32 = 4;
    /// Get TLS module ID
    pub const RTLD_DI_TLS_MODID: i32 = 9;
    /// Get TLS data address
    pub const RTLD_DI_TLS_DATA: i32 = 10;
}

/// Get information about a loaded object
pub fn dlinfo(handle: u64, request: i32, _info: *mut u8) -> i32 {
    crate::kprintln!("dlfcn: dlinfo({:#x}, {})", handle, request);

    // Stub implementation
    match request {
        dlinfo_request::RTLD_DI_LINKMAP => {
            // Would return the link_map structure
            -1
        }
        dlinfo_request::RTLD_DI_ORIGIN => {
            // Would return the origin directory
            -1
        }
        _ => -1
    }
}
