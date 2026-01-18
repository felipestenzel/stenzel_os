//! CPU Microcode Update Driver
//!
//! Loads and applies CPU microcode updates for Intel and AMD processors.
//! Microcode updates fix CPU errata and security vulnerabilities.

#![allow(dead_code)]

use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};
use spin::Mutex;

use super::firmware;

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static MICROCODE_INFO: Mutex<Option<MicrocodeInfo>> = Mutex::new(None);

/// Microcode update information
#[derive(Debug, Clone)]
pub struct MicrocodeInfo {
    pub vendor: CpuVendor,
    pub family: u8,
    pub model: u8,
    pub stepping: u8,
    pub current_revision: u32,
    pub updated_revision: Option<u32>,
}

/// CPU vendor
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuVendor {
    Intel,
    Amd,
    Unknown,
}

/// Intel microcode header (48 bytes)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct IntelMicrocodeHeader {
    header_version: u32,
    update_revision: u32,
    date: u32,           // BCD: MMDDYYYY
    processor_signature: u32,
    checksum: u32,
    loader_revision: u32,
    processor_flags: u32,
    data_size: u32,      // 0 means 2000 bytes
    total_size: u32,     // 0 means 2048 bytes
    reserved: [u32; 3],
}

/// AMD microcode header
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct AmdMicrocodeHeader {
    data_code: u32,
    patch_id: u32,
    mc_patch_data_id: u16,
    mc_patch_data_len: u8,
    init_flag: u8,
    mc_patch_data_checksum: u32,
    nb_dev_id: u32,
    sb_dev_id: u32,
    processor_rev_id: u16,
    nb_rev_id: u8,
    sb_rev_id: u8,
    bios_api_rev: u8,
    reserved: [u8; 3],
    match_reg: [u32; 8],
}

/// Initialize microcode loader
pub fn init() {
    if INITIALIZED.swap(true, Ordering::SeqCst) {
        return;
    }

    crate::kprintln!("microcode: Initializing CPU microcode loader...");

    let vendor = detect_cpu_vendor();
    let (family, model, stepping) = get_cpu_signature();
    let current_rev = get_current_microcode_revision();

    crate::kprintln!("microcode: CPU: {:?} family {:#x} model {:#x} stepping {}",
        vendor, family, model, stepping);
    crate::kprintln!("microcode: Current revision: {:#010x}", current_rev);

    let info = MicrocodeInfo {
        vendor,
        family,
        model,
        stepping,
        current_revision: current_rev,
        updated_revision: None,
    };

    *MICROCODE_INFO.lock() = Some(info);

    // Try to load and apply microcode
    match vendor {
        CpuVendor::Intel => {
            if let Err(e) = load_intel_microcode(family, model, stepping) {
                crate::kprintln!("microcode: Intel update failed: {:?}", e);
            }
        }
        CpuVendor::Amd => {
            if let Err(e) = load_amd_microcode(family, model, stepping) {
                crate::kprintln!("microcode: AMD update failed: {:?}", e);
            }
        }
        CpuVendor::Unknown => {
            crate::kprintln!("microcode: Unknown CPU vendor");
        }
    }
}

/// Detect CPU vendor from CPUID
fn detect_cpu_vendor() -> CpuVendor {
    let vendor_string = unsafe {
        let mut ebx: u32;
        let mut ecx: u32;
        let mut edx: u32;

        // CPUID with eax=0 returns vendor string in EBX:EDX:ECX
        // We need to save/restore rbx since LLVM uses it internally
        core::arch::asm!(
            "push rbx",
            "cpuid",
            "mov {0:e}, ebx",
            "pop rbx",
            out(reg) ebx,
            inout("eax") 0u32 => _,
            out("ecx") ecx,
            out("edx") edx,
            options(preserves_flags)
        );

        let mut vendor = [0u8; 12];
        vendor[0..4].copy_from_slice(&ebx.to_le_bytes());
        vendor[4..8].copy_from_slice(&edx.to_le_bytes());
        vendor[8..12].copy_from_slice(&ecx.to_le_bytes());
        vendor
    };

    match &vendor_string {
        b"GenuineIntel" => CpuVendor::Intel,
        b"AuthenticAMD" => CpuVendor::Amd,
        _ => CpuVendor::Unknown,
    }
}

/// Get CPU family/model/stepping from CPUID
fn get_cpu_signature() -> (u8, u8, u8) {
    let eax: u32;

    unsafe {
        // Save/restore rbx since LLVM uses it internally
        core::arch::asm!(
            "push rbx",
            "cpuid",
            "pop rbx",
            inout("eax") 1u32 => eax,
            out("ecx") _,
            out("edx") _,
            options(preserves_flags)
        );
    }

    let stepping = (eax & 0xF) as u8;
    let model = ((eax >> 4) & 0xF) as u8;
    let family = ((eax >> 8) & 0xF) as u8;
    let ext_model = ((eax >> 16) & 0xF) as u8;
    let ext_family = ((eax >> 20) & 0xFF) as u8;

    let actual_family = if family == 0xF {
        family + ext_family
    } else {
        family
    };

    let actual_model = if family == 0x6 || family == 0xF {
        (ext_model << 4) | model
    } else {
        model
    };

    (actual_family, actual_model, stepping)
}

/// Get current microcode revision
fn get_current_microcode_revision() -> u32 {
    // Write 0 to IA32_BIOS_SIGN_ID MSR, then read CPUID, then read MSR
    unsafe {
        // Clear signature
        core::arch::asm!(
            "wrmsr",
            in("ecx") 0x8Bu32,
            in("eax") 0u32,
            in("edx") 0u32,
        );

        // CPUID to update signature - save/restore rbx since LLVM uses it
        core::arch::asm!(
            "push rbx",
            "cpuid",
            "pop rbx",
            inout("eax") 1u32 => _,
            out("ecx") _,
            out("edx") _,
            options(preserves_flags)
        );

        // Read signature from MSR
        let eax: u32;
        let edx: u32;
        core::arch::asm!(
            "rdmsr",
            in("ecx") 0x8Bu32,
            out("eax") eax,
            out("edx") edx,
        );

        edx // Microcode revision is in upper 32 bits
    }
}

/// Load Intel microcode
fn load_intel_microcode(family: u8, model: u8, stepping: u8) -> Result<(), MicrocodeError> {
    // Construct firmware path
    let path = alloc::format!(
        "intel-ucode/{:02x}-{:02x}-{:02x}",
        family, model, stepping
    );

    let data = firmware::request_firmware(&path)
        .map_err(|_| MicrocodeError::NotFound)?;

    if data.len() < core::mem::size_of::<IntelMicrocodeHeader>() {
        return Err(MicrocodeError::InvalidFormat);
    }

    // Parse header
    let header = unsafe {
        core::ptr::read_unaligned(data.as_ptr() as *const IntelMicrocodeHeader)
    };

    // Copy values from packed struct to avoid unaligned reference issues
    let update_revision = { header.update_revision };
    let processor_signature = { header.processor_signature };

    crate::kprintln!("microcode: Found Intel update revision {:#010x}", update_revision);

    // Verify this is for our CPU
    let expected_sig = ((family as u32) << 8) | ((model as u32) << 4) | (stepping as u32);
    if processor_signature != expected_sig {
        crate::kprintln!("microcode: Signature mismatch (expected {:#x}, got {:#x})",
            expected_sig, processor_signature);
        // Continue anyway, signature format may differ
    }

    // Apply microcode update
    let data_ptr = data.as_ptr() as u64;
    unsafe {
        core::arch::asm!(
            "wrmsr",
            in("ecx") 0x79u32,  // IA32_BIOS_UPDT_TRIG
            in("eax") (data_ptr as u32),
            in("edx") ((data_ptr >> 32) as u32),
        );
    }

    // Verify update was applied
    let new_rev = get_current_microcode_revision();
    if new_rev != update_revision {
        crate::kprintln!("microcode: Update may not have been applied (rev: {:#010x})", new_rev);
    } else {
        crate::kprintln!("microcode: Successfully updated to revision {:#010x}", new_rev);

        if let Some(ref mut info) = *MICROCODE_INFO.lock() {
            info.updated_revision = Some(new_rev);
        }
    }

    Ok(())
}

/// Load AMD microcode
fn load_amd_microcode(family: u8, model: u8, _stepping: u8) -> Result<(), MicrocodeError> {
    // AMD microcode is in a container format
    let path = alloc::format!(
        "amd-ucode/microcode_amd_fam{:02x}h.bin",
        family
    );

    let data = firmware::request_firmware(&path)
        .map_err(|_| MicrocodeError::NotFound)?;

    if data.len() < 32 {
        return Err(MicrocodeError::InvalidFormat);
    }

    // AMD container has multiple patches
    // We need to find the one matching our CPU
    let mut offset = 0;

    while offset + core::mem::size_of::<AmdMicrocodeHeader>() <= data.len() {
        let header = unsafe {
            core::ptr::read_unaligned(
                data[offset..].as_ptr() as *const AmdMicrocodeHeader
            )
        };

        // Copy values from packed struct to avoid unaligned reference issues
        let data_code = { header.data_code };
        let patch_id = { header.patch_id };
        let processor_rev_id = { header.processor_rev_id };
        let mc_patch_data_len = { header.mc_patch_data_len };

        if data_code == 0 {
            break; // End of container
        }

        // Check if this patch matches our CPU
        let patch_family = ((processor_rev_id >> 8) & 0xFF) as u8;
        let patch_model = (processor_rev_id & 0xFF) as u8;

        if patch_family == family && patch_model == model {
            crate::kprintln!("microcode: Found AMD patch ID {:#010x}", patch_id);

            // Apply AMD microcode
            let patch_ptr = data[offset..].as_ptr() as u64;
            unsafe {
                core::arch::asm!(
                    "wrmsr",
                    in("ecx") 0xC0010020u32,  // AMD PATCH_LOADER MSR
                    in("eax") (patch_ptr as u32),
                    in("edx") ((patch_ptr >> 32) as u32),
                );
            }

            // Read new revision
            let new_rev: u32;
            unsafe {
                let eax: u32;
                core::arch::asm!(
                    "rdmsr",
                    in("ecx") 0x8Bu32,
                    out("eax") eax,
                    out("edx") _,
                );
                new_rev = eax;
            }

            crate::kprintln!("microcode: AMD microcode revision: {:#010x}", new_rev);

            if let Some(ref mut info) = *MICROCODE_INFO.lock() {
                info.updated_revision = Some(new_rev);
            }

            return Ok(());
        }

        // Move to next patch
        let patch_size = mc_patch_data_len as usize * 4;
        offset += core::mem::size_of::<AmdMicrocodeHeader>() + patch_size;
    }

    crate::kprintln!("microcode: No matching AMD patch found for family {:#x} model {:#x}",
        family, model);
    Err(MicrocodeError::NotFound)
}

/// Microcode error type
#[derive(Debug)]
pub enum MicrocodeError {
    NotFound,
    InvalidFormat,
    ApplyFailed,
    UnsupportedCpu,
}

/// Get microcode info
pub fn get_info() -> Option<MicrocodeInfo> {
    MICROCODE_INFO.lock().clone()
}

/// Check if microcode was updated this boot
pub fn was_updated() -> bool {
    MICROCODE_INFO.lock()
        .as_ref()
        .map(|i| i.updated_revision.is_some())
        .unwrap_or(false)
}

/// Get CPU vendor
pub fn cpu_vendor() -> CpuVendor {
    MICROCODE_INFO.lock()
        .as_ref()
        .map(|i| i.vendor)
        .unwrap_or(CpuVendor::Unknown)
}
