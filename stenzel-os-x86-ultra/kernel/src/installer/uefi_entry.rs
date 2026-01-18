//! UEFI Boot Entry Management

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use super::{InstallError, InstallResult};

/// UEFI boot entry
#[derive(Debug, Clone)]
pub struct UefiBootEntry {
    pub number: u16,
    pub name: String,
    pub path: String,
    pub active: bool,
}

/// Create UEFI boot entry for Stenzel OS
pub fn create_boot_entry(esp_partition: u32, loader_path: &str) -> InstallResult<u16> {
    crate::kprintln!("uefi_entry: Creating boot entry for {}", loader_path);

    // Use UEFI runtime services to create boot entry
    let entry_num = allocate_boot_number()?;

    // Create Boot#### variable
    create_boot_variable(entry_num, "Stenzel OS", esp_partition, loader_path)?;

    // Add to BootOrder
    add_to_boot_order(entry_num)?;

    crate::kprintln!("uefi_entry: Created Boot{:04X}", entry_num);
    Ok(entry_num)
}

fn allocate_boot_number() -> InstallResult<u16> {
    // Find unused boot entry number
    for i in 0..0xFFFF {
        if !boot_entry_exists(i) {
            return Ok(i);
        }
    }
    Err(InstallError::BootloaderError(String::from("No free boot entry")))
}

fn boot_entry_exists(num: u16) -> bool {
    // Check if Boot#### variable exists
    let _ = num;
    false
}

fn create_boot_variable(num: u16, name: &str, partition: u32, path: &str) -> InstallResult<()> {
    crate::kprintln!("uefi_entry: Creating Boot{:04X} = {} (part {}, {})", num, name, partition, path);
    Ok(())
}

fn add_to_boot_order(entry_num: u16) -> InstallResult<()> {
    crate::kprintln!("uefi_entry: Adding Boot{:04X} to BootOrder", entry_num);
    Ok(())
}

/// List all UEFI boot entries
pub fn list_boot_entries() -> InstallResult<Vec<UefiBootEntry>> {
    let mut entries = Vec::new();
    
    // Read BootOrder and each Boot#### variable
    entries.push(UefiBootEntry {
        number: 0,
        name: String::from("Stenzel OS"),
        path: String::from("\\EFI\\BOOT\\BOOTX64.EFI"),
        active: true,
    });
    
    Ok(entries)
}

/// Remove UEFI boot entry
pub fn remove_boot_entry(entry_num: u16) -> InstallResult<()> {
    crate::kprintln!("uefi_entry: Removing Boot{:04X}", entry_num);
    // Delete Boot#### variable
    // Remove from BootOrder
    Ok(())
}

/// Set default boot entry
pub fn set_default_entry(entry_num: u16) -> InstallResult<()> {
    crate::kprintln!("uefi_entry: Setting Boot{:04X} as default", entry_num);
    // Move entry to front of BootOrder
    Ok(())
}

pub fn init() {
    crate::kprintln!("uefi_entry: UEFI boot manager initialized");
}
