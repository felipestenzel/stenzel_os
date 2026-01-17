//! Windows PE Executable Loader
//!
//! Loads PE32/PE32+ executables into memory, performs relocations,
//! and resolves imports.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;

use super::pe::{
    self, PeFile, PeError, DataDirectory, BaseRelocation,
    data_directory, relocation_type,
};

/// Loaded module information
#[derive(Debug)]
pub struct LoadedModule {
    /// Module name (e.g., "kernel32.dll")
    pub name: String,
    /// Base address where loaded
    pub base_address: u64,
    /// Size of loaded image
    pub size: u32,
    /// Entry point address
    pub entry_point: u64,
    /// Whether this is a DLL
    pub is_dll: bool,
    /// Exported functions: name -> RVA
    pub exports: BTreeMap<String, u64>,
    /// Loaded section information
    pub sections: Vec<LoadedSection>,
}

/// Loaded section info
#[derive(Debug, Clone)]
pub struct LoadedSection {
    pub name: String,
    pub virtual_address: u64,
    pub size: u32,
    pub readable: bool,
    pub writable: bool,
    pub executable: bool,
}

/// Loader errors
#[derive(Debug, Clone)]
pub enum LoaderError {
    PeError(PeError),
    AllocationFailed,
    ImportNotFound(String, String),
    ModuleNotFound(String),
    RelocationsRequired,
    UnsupportedRelocationType(u16),
}

impl From<PeError> for LoaderError {
    fn from(e: PeError) -> Self {
        LoaderError::PeError(e)
    }
}

/// Module loader context
pub struct PeLoader {
    /// Loaded modules by name (lowercase)
    modules: BTreeMap<String, LoadedModule>,
    /// Next available base address for loading
    next_base: u64,
}

impl PeLoader {
    /// Create new loader
    pub fn new() -> Self {
        Self {
            modules: BTreeMap::new(),
            // Start loading modules at 0x10000000 (256MB)
            // This leaves room for the main executable at lower addresses
            next_base: 0x1000_0000,
        }
    }

    /// Get a loaded module by name
    pub fn get_module(&self, name: &str) -> Option<&LoadedModule> {
        self.modules.get(&name.to_lowercase())
    }

    /// Check if module is loaded
    pub fn is_loaded(&self, name: &str) -> bool {
        self.modules.contains_key(&name.to_lowercase())
    }

    /// Register a built-in module (for emulated DLLs like kernel32)
    pub fn register_builtin(&mut self, name: &str, exports: BTreeMap<String, u64>) {
        let module = LoadedModule {
            name: name.to_string(),
            base_address: 0, // Built-in modules don't have a real base
            size: 0,
            entry_point: 0,
            is_dll: true,
            exports,
            sections: vec![],
        };
        self.modules.insert(name.to_lowercase(), module);
    }

    /// Load a PE file from memory
    pub fn load(
        &mut self,
        name: &str,
        data: &[u8],
        preferred_base: Option<u64>,
    ) -> Result<&LoadedModule, LoaderError> {
        let pe = PeFile::parse(data)?;

        // Determine load address
        let load_base = if let Some(base) = preferred_base {
            base
        } else if pe.supports_aslr() {
            // Use next available address
            let base = self.next_base;
            self.next_base += self.align_up(pe.size_of_image as u64, 0x10000);
            base
        } else {
            // Try to use preferred base
            pe.image_base
        };

        // Calculate delta for relocations
        let delta = load_base as i64 - pe.image_base as i64;
        let needs_relocation = delta != 0;

        // Check if relocations are needed but not available
        if needs_relocation {
            let reloc_dir = pe.data_directories[data_directory::BASERELOC];
            if reloc_dir.virtual_address == 0 || reloc_dir.size == 0 {
                if !pe.supports_aslr() {
                    // Non-ASLR image without relocations can't be moved
                    return Err(LoaderError::RelocationsRequired);
                }
            }
        }

        // Allocate memory for the image
        // In a real implementation, this would use mm::alloc_pages()
        // For now, we simulate the allocation
        let image_size = pe.size_of_image as usize;

        // Copy headers
        let headers_size = pe.size_of_headers as usize;
        let mut image = vec![0u8; image_size];
        if headers_size <= data.len() {
            image[..headers_size].copy_from_slice(&data[..headers_size]);
        }

        // Load sections
        let mut loaded_sections = Vec::new();
        for section in &pe.sections {
            let dest_start = section.virtual_address as usize;
            let dest_end = dest_start + section.size_of_raw_data as usize;

            if dest_end > image_size {
                continue;
            }

            let src_start = section.pointer_to_raw_data as usize;
            let src_end = src_start + section.size_of_raw_data as usize;

            if src_end <= data.len() {
                image[dest_start..dest_end].copy_from_slice(&data[src_start..src_end]);
            }

            loaded_sections.push(LoadedSection {
                name: section.name_str(),
                virtual_address: load_base + section.virtual_address as u64,
                size: section.virtual_size,
                readable: section.is_readable(),
                writable: section.is_writable(),
                executable: section.is_executable(),
            });
        }

        // Apply relocations if needed
        if needs_relocation {
            self.apply_relocations(&pe, &mut image, delta, pe.is_64bit)?;
        }

        // Parse exports
        let exports_list = pe::parse_exports(&pe, data)?;
        let mut exports = BTreeMap::new();
        for (func_name, rva) in exports_list {
            exports.insert(func_name, load_base + rva as u64);
        }

        // Create module entry
        let module = LoadedModule {
            name: name.to_string(),
            base_address: load_base,
            size: pe.size_of_image,
            entry_point: load_base + pe.entry_point,
            is_dll: pe.is_dll,
            exports,
            sections: loaded_sections,
        };

        // Store module
        let key = name.to_lowercase();
        self.modules.insert(key.clone(), module);
        Ok(self.modules.get(&key).unwrap())
    }

    /// Apply base relocations
    fn apply_relocations(
        &self,
        pe: &PeFile,
        image: &mut [u8],
        delta: i64,
        is_64bit: bool,
    ) -> Result<(), LoaderError> {
        let reloc_dir = pe.data_directories[data_directory::BASERELOC];
        if reloc_dir.virtual_address == 0 {
            return Ok(());
        }

        let mut offset = reloc_dir.virtual_address as usize;
        let end_offset = offset + reloc_dir.size as usize;

        while offset < end_offset {
            if offset + 8 > image.len() {
                break;
            }

            // Read relocation block header
            let block_rva = u32::from_le_bytes([
                image[offset], image[offset + 1],
                image[offset + 2], image[offset + 3],
            ]);
            let block_size = u32::from_le_bytes([
                image[offset + 4], image[offset + 5],
                image[offset + 6], image[offset + 7],
            ]) as usize;

            if block_size < 8 || block_size > end_offset - offset {
                break;
            }

            // Process entries in this block
            let num_entries = (block_size - 8) / 2;
            for i in 0..num_entries {
                let entry_offset = offset + 8 + i * 2;
                if entry_offset + 2 > image.len() {
                    break;
                }

                let entry = u16::from_le_bytes([image[entry_offset], image[entry_offset + 1]]);
                let reloc_type = (entry >> 12) as u16;
                let reloc_offset = (entry & 0x0FFF) as u32;

                let patch_rva = (block_rva + reloc_offset) as usize;
                if patch_rva + 8 > image.len() {
                    continue;
                }

                match reloc_type {
                    relocation_type::ABSOLUTE => {
                        // No operation
                    }
                    relocation_type::HIGHLOW => {
                        // 32-bit relocation
                        let value = u32::from_le_bytes([
                            image[patch_rva], image[patch_rva + 1],
                            image[patch_rva + 2], image[patch_rva + 3],
                        ]);
                        let new_value = (value as i64 + delta) as u32;
                        image[patch_rva..patch_rva + 4].copy_from_slice(&new_value.to_le_bytes());
                    }
                    relocation_type::DIR64 => {
                        // 64-bit relocation
                        let value = u64::from_le_bytes([
                            image[patch_rva], image[patch_rva + 1],
                            image[patch_rva + 2], image[patch_rva + 3],
                            image[patch_rva + 4], image[patch_rva + 5],
                            image[patch_rva + 6], image[patch_rva + 7],
                        ]);
                        let new_value = (value as i64 + delta) as u64;
                        image[patch_rva..patch_rva + 8].copy_from_slice(&new_value.to_le_bytes());
                    }
                    relocation_type::HIGH => {
                        // High 16 bits
                        let value = u16::from_le_bytes([image[patch_rva], image[patch_rva + 1]]);
                        let full_value = ((value as u32) << 16) as i64 + delta;
                        let new_value = ((full_value >> 16) & 0xFFFF) as u16;
                        image[patch_rva..patch_rva + 2].copy_from_slice(&new_value.to_le_bytes());
                    }
                    relocation_type::LOW => {
                        // Low 16 bits
                        let value = u16::from_le_bytes([image[patch_rva], image[patch_rva + 1]]);
                        let new_value = ((value as i64 + delta) & 0xFFFF) as u16;
                        image[patch_rva..patch_rva + 2].copy_from_slice(&new_value.to_le_bytes());
                    }
                    _ => {
                        return Err(LoaderError::UnsupportedRelocationType(reloc_type));
                    }
                }
            }

            offset += block_size;
        }

        Ok(())
    }

    /// Resolve imports for a loaded module
    pub fn resolve_imports(
        &self,
        pe: &PeFile,
        image: &mut [u8],
        load_base: u64,
    ) -> Result<(), LoaderError> {
        let imports = pe::parse_imports(pe, image)?;

        for (dll_name, functions) in imports {
            let dll_lower = dll_name.to_lowercase();

            // Find the module
            let module = self.modules.get(&dll_lower)
                .ok_or_else(|| LoaderError::ModuleNotFound(dll_name.clone()))?;

            // Resolve each import
            for func_name in functions {
                let func_addr = if func_name.starts_with('#') {
                    // Import by ordinal - not commonly used, skip for now
                    continue;
                } else {
                    module.exports.get(&func_name)
                        .ok_or_else(|| LoaderError::ImportNotFound(dll_name.clone(), func_name.clone()))?
                };

                // Update IAT entry
                // This would require finding the IAT location and patching it
                // For now, we'll handle this in the actual loading code
                crate::kprintln!("winload: resolved {}!{} -> {:#x}",
                    dll_name, func_name, func_addr);
            }
        }

        Ok(())
    }

    /// Align value up to alignment
    fn align_up(&self, value: u64, alignment: u64) -> u64 {
        (value + alignment - 1) & !(alignment - 1)
    }

    /// Get list of loaded modules
    pub fn list_modules(&self) -> Vec<&LoadedModule> {
        self.modules.values().collect()
    }

    /// Unload a module
    pub fn unload(&mut self, name: &str) -> bool {
        self.modules.remove(&name.to_lowercase()).is_some()
    }
}

impl Default for PeLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Global PE loader instance
static mut PE_LOADER: Option<PeLoader> = None;

/// Initialize the PE loader
pub fn init() {
    unsafe {
        PE_LOADER = Some(PeLoader::new());
    }
    crate::kprintln!("winload: PE loader initialized");
}

/// Get the global PE loader
pub fn loader() -> &'static mut PeLoader {
    unsafe {
        PE_LOADER.as_mut().expect("PE loader not initialized")
    }
}

/// Load a PE executable
pub fn load_executable(name: &str, data: &[u8]) -> Result<u64, LoaderError> {
    let loader = loader();

    // Load the main executable at a low address
    let module = loader.load(name, data, Some(0x0040_0000))?;
    Ok(module.entry_point)
}

/// Load a DLL
pub fn load_library(name: &str, data: &[u8]) -> Result<u64, LoaderError> {
    let loader = loader();

    // DLLs are loaded at higher addresses
    let module = loader.load(name, data, None)?;
    Ok(module.base_address)
}

/// Check if a file looks like a PE executable
pub fn is_pe_file(data: &[u8]) -> bool {
    if data.len() < 2 {
        return false;
    }
    // Check for "MZ" signature
    data[0] == b'M' && data[1] == b'Z'
}

/// Get PE file type
pub fn get_pe_type(data: &[u8]) -> Option<&'static str> {
    let pe = PeFile::parse(data).ok()?;

    Some(match pe.get_subsystem() {
        super::WindowsSubsystem::WindowsCui => "Windows Console Application",
        super::WindowsSubsystem::WindowsGui => "Windows GUI Application",
        super::WindowsSubsystem::Native => "Windows Native Application",
        super::WindowsSubsystem::EfiApplication => "EFI Application",
        _ => if pe.is_dll { "Windows DLL" } else { "Windows Executable" },
    })
}
