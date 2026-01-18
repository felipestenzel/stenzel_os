//! Firmware Loader
//!
//! Loads firmware blobs from /lib/firmware for device drivers.
//! Supports:
//! - Binary firmware files
//! - Compressed firmware (.xz, .zst)
//! - Firmware fallback paths
//! - Firmware caching

#![allow(dead_code)]

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use spin::Mutex;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::fs;

static FIRMWARE_CACHE: Mutex<BTreeMap<String, Vec<u8>>> = Mutex::new(BTreeMap::new());
static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Firmware search paths (in order of preference)
const FIRMWARE_PATHS: &[&str] = &[
    "/lib/firmware",
    "/lib/firmware/updates",
    "/usr/lib/firmware",
    "/usr/share/firmware",
];

/// Firmware load result
#[derive(Debug)]
pub enum FirmwareError {
    NotFound,
    IoError,
    DecompressError,
    InvalidFormat,
}

/// Firmware metadata
#[derive(Debug, Clone)]
pub struct FirmwareInfo {
    pub name: String,
    pub path: String,
    pub size: usize,
    pub compressed: bool,
}

/// Initialize firmware loader
pub fn init() {
    if INITIALIZED.swap(true, Ordering::SeqCst) {
        return;
    }
    crate::kprintln!("firmware: Firmware loader initialized");
}

/// Request firmware by name
pub fn request_firmware(name: &str) -> Result<Vec<u8>, FirmwareError> {
    // Check cache first
    {
        let cache = FIRMWARE_CACHE.lock();
        if let Some(data) = cache.get(name) {
            crate::kprintln!("firmware: {} (cached, {} bytes)", name, data.len());
            return Ok(data.clone());
        }
    }

    // Try to load from filesystem
    for base_path in FIRMWARE_PATHS {
        // Try uncompressed
        let path = alloc::format!("{}/{}", base_path, name);
        if let Ok(data) = load_firmware_file(&path) {
            cache_firmware(name, data.clone());
            crate::kprintln!("firmware: {} loaded ({} bytes)", name, data.len());
            return Ok(data);
        }

        // Try .xz compressed
        let xz_path = alloc::format!("{}.xz", path);
        if let Ok(data) = load_firmware_file(&xz_path) {
            // Decompress XZ
            if let Ok(decompressed) = decompress_xz(&data) {
                cache_firmware(name, decompressed.clone());
                crate::kprintln!("firmware: {} loaded (xz, {} bytes)", name, decompressed.len());
                return Ok(decompressed);
            }
        }

        // Try .zst compressed
        let zst_path = alloc::format!("{}.zst", path);
        if let Ok(data) = load_firmware_file(&zst_path) {
            // Decompress Zstd
            if let Ok(decompressed) = decompress_zstd(&data) {
                cache_firmware(name, decompressed.clone());
                crate::kprintln!("firmware: {} loaded (zstd, {} bytes)", name, decompressed.len());
                return Ok(decompressed);
            }
        }
    }

    crate::kprintln!("firmware: {} not found", name);
    Err(FirmwareError::NotFound)
}

/// Load firmware file from path
fn load_firmware_file(path: &str) -> Result<Vec<u8>, FirmwareError> {
    let cred = match crate::security::user_db().login("root") {
        Ok(c) => c,
        Err(_) => return Err(FirmwareError::IoError),
    };

    let mut vfs = fs::vfs_lock();
    match vfs.read_file(path, &cred) {
        Ok(data) => Ok(data),
        Err(_) => Err(FirmwareError::IoError),
    }
}

/// Cache firmware in memory
fn cache_firmware(name: &str, data: Vec<u8>) {
    let mut cache = FIRMWARE_CACHE.lock();
    // Limit cache size (16MB total)
    const MAX_CACHE_SIZE: usize = 16 * 1024 * 1024;
    let current_size: usize = cache.values().map(|v| v.len()).sum();

    if current_size + data.len() < MAX_CACHE_SIZE {
        cache.insert(name.to_string(), data);
    }
}

/// Decompress XZ data
fn decompress_xz(data: &[u8]) -> Result<Vec<u8>, FirmwareError> {
    // XZ magic: 0xFD 0x37 0x7A 0x58 0x5A 0x00
    if data.len() < 6 || &data[0..6] != b"\xFD7zXZ\x00" {
        return Err(FirmwareError::InvalidFormat);
    }

    // TODO: Implement XZ decompression
    // For now, return error
    Err(FirmwareError::DecompressError)
}

/// Decompress Zstd data
fn decompress_zstd(data: &[u8]) -> Result<Vec<u8>, FirmwareError> {
    // Zstd magic: 0x28 0xB5 0x2F 0xFD
    if data.len() < 4 || &data[0..4] != b"\x28\xB5\x2F\xFD" {
        return Err(FirmwareError::InvalidFormat);
    }

    // TODO: Implement Zstd decompression
    // For now, return error
    Err(FirmwareError::DecompressError)
}

/// Release firmware from cache
pub fn release_firmware(name: &str) {
    let mut cache = FIRMWARE_CACHE.lock();
    cache.remove(name);
}

/// Clear firmware cache
pub fn clear_cache() {
    let mut cache = FIRMWARE_CACHE.lock();
    cache.clear();
}

/// Get cache statistics
pub fn cache_stats() -> (usize, usize) {
    let cache = FIRMWARE_CACHE.lock();
    let count = cache.len();
    let size: usize = cache.values().map(|v| v.len()).sum();
    (count, size)
}

// ============================================================================
// Common Firmware Names
// ============================================================================

/// Intel WiFi firmware
pub mod intel_wifi {
    pub const IWL_7260: &str = "iwlwifi-7260-17.ucode";
    pub const IWL_8000: &str = "iwlwifi-8000C-36.ucode";
    pub const IWL_8265: &str = "iwlwifi-8265-36.ucode";
    pub const IWL_9000: &str = "iwlwifi-9000-pu-b0-jf-b0-46.ucode";
    pub const IWL_9260: &str = "iwlwifi-9260-th-b0-jf-b0-46.ucode";
    pub const IWL_AX200: &str = "iwlwifi-cc-a0-77.ucode";
    pub const IWL_AX201: &str = "iwlwifi-QuZ-a0-hr-b0-77.ucode";
    pub const IWL_AX210: &str = "iwlwifi-ty-a0-gf-a0-77.ucode";
    pub const IWL_AX211: &str = "iwlwifi-so-a0-gf-a0-77.ucode";
}

/// Intel GPU firmware
pub mod intel_gpu {
    pub const GUC_SKL: &str = "i915/skl_guc_70.1.1.bin";
    pub const HUC_SKL: &str = "i915/skl_huc_2.0.0.bin";
    pub const GUC_KBL: &str = "i915/kbl_guc_70.1.1.bin";
    pub const HUC_KBL: &str = "i915/kbl_huc_4.0.0.bin";
    pub const GUC_ICL: &str = "i915/icl_guc_70.1.1.bin";
    pub const HUC_ICL: &str = "i915/icl_huc_9.0.0.bin";
    pub const GUC_TGL: &str = "i915/tgl_guc_70.1.1.bin";
    pub const HUC_TGL: &str = "i915/tgl_huc_7.9.3.bin";
    pub const GUC_ADL: &str = "i915/adlp_guc_70.1.1.bin";
    pub const HUC_ADL: &str = "i915/adlp_huc_9.3.0.bin";
    pub const DMC_SKL: &str = "i915/skl_dmc_ver1_27.bin";
    pub const DMC_KBL: &str = "i915/kbl_dmc_ver1_04.bin";
    pub const DMC_ICL: &str = "i915/icl_dmc_ver1_09.bin";
    pub const DMC_TGL: &str = "i915/tgl_dmc_ver2_12.bin";
}

/// AMD GPU firmware
pub mod amd_gpu {
    pub const POLARIS_CE: &str = "amdgpu/polaris10_ce.bin";
    pub const POLARIS_ME: &str = "amdgpu/polaris10_me.bin";
    pub const POLARIS_MEC: &str = "amdgpu/polaris10_mec.bin";
    pub const POLARIS_PFP: &str = "amdgpu/polaris10_pfp.bin";
    pub const POLARIS_RLC: &str = "amdgpu/polaris10_rlc.bin";
    pub const POLARIS_SMC: &str = "amdgpu/polaris10_smc.bin";
    pub const VEGA_CE: &str = "amdgpu/vega10_ce.bin";
    pub const VEGA_ME: &str = "amdgpu/vega10_me.bin";
    pub const NAVI10_PFP: &str = "amdgpu/navi10_pfp.bin";
    pub const NAVI10_ME: &str = "amdgpu/navi10_me.bin";
    pub const NAVI10_CE: &str = "amdgpu/navi10_ce.bin";
    pub const NAVI10_MEC: &str = "amdgpu/navi10_mec.bin";
    pub const NAVI10_RLC: &str = "amdgpu/navi10_rlc.bin";
    pub const NAVI10_SMC: &str = "amdgpu/navi10_smc.bin";
}

/// Realtek WiFi firmware
pub mod realtek_wifi {
    pub const RTL8188E: &str = "rtlwifi/rtl8188efw.bin";
    pub const RTL8192E: &str = "rtlwifi/rtl8192eefw.bin";
    pub const RTL8821AE: &str = "rtlwifi/rtl8821aefw.bin";
    pub const RTL8822BE: &str = "rtlwifi/rtl8822befw.bin";
}
