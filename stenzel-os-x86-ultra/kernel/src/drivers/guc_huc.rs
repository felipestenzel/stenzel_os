//! Intel GuC/HuC Firmware Loader
//!
//! Loads and manages firmware for Intel GPU microcontrollers:
//! - GuC (Graphics Microcontroller): GPU scheduling, power management
//! - HuC (HEVC Microcontroller): Hardware video decode authentication

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};
use spin::Mutex;

use super::firmware;

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static GUC_HUC_STATE: Mutex<Option<GucHucState>> = Mutex::new(None);

/// GuC/HuC state
#[derive(Debug)]
pub struct GucHucState {
    /// GPU MMIO base
    pub mmio_base: u64,
    /// GPU generation
    pub gen: IntelGen,
    /// GuC firmware status
    pub guc_status: FirmwareStatus,
    /// HuC firmware status
    pub huc_status: FirmwareStatus,
    /// GuC firmware version
    pub guc_version: Option<FirmwareVersion>,
    /// HuC firmware version
    pub huc_version: Option<FirmwareVersion>,
    /// GuC submission enabled
    pub guc_submission: bool,
}

/// Intel GPU generation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntelGen {
    Gen9,   // Skylake
    Gen9p5, // Kaby Lake
    Gen10,  // Cannon Lake (never released)
    Gen11,  // Ice Lake
    Gen12,  // Tiger Lake
    Gen12p5, // Alder Lake
    Gen12p7, // DG2/Alchemist
    Unknown,
}

/// Firmware status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirmwareStatus {
    NotLoaded,
    Loading,
    Loaded,
    Running,
    Failed,
    Disabled,
}

/// Firmware version
#[derive(Debug, Clone)]
pub struct FirmwareVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

/// GuC firmware header (CSS header)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct CssHeader {
    module_type: u32,
    header_len: u32,
    header_version: u32,
    module_id: u32,
    module_vendor: u32,
    date: u32,
    size: u32,
    key_size: u32,
    modulus_size: u32,
    exponent_size: u32,
    time: u32,
    sw_version: u32,
    reserved: [u32; 12],
}

/// GuC UCode header
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct GucUcodeHeader {
    css: CssHeader,
    ucode_version: u32,
    ucode_size: u32,
    header_size: u32,
}

/// GuC/HuC register offsets
mod regs {
    pub const GUC_STATUS: u64 = 0xC000;
    pub const GUC_WOPCM_SIZE: u64 = 0xC050;
    pub const GUC_SHIM_CONTROL: u64 = 0xC064;
    pub const GUC_SEND_INTERRUPT: u64 = 0xC4C8;
    pub const GUC_HOST_COMMUNICATION: u64 = 0xC4E8;

    pub const HUC_STATUS: u64 = 0xD000;
    pub const HUC_LOADING_AGENT_GUC: u64 = 0xD048;

    pub const SOFT_SCRATCH_0: u64 = 0xC180;
    pub const SOFT_SCRATCH_15: u64 = 0xC1BC;

    pub const DMA_CTRL: u64 = 0xC300;
    pub const DMA_ADDR_0: u64 = 0xC304;
    pub const DMA_ADDR_1: u64 = 0xC308;
    pub const DMA_GUC_WOPCM_OFFSET: u64 = 0xC340;
    pub const DMA_START: u64 = 0xC344;
}

/// Initialize GuC/HuC subsystem
pub fn init(mmio_base: u64, gen: IntelGen) {
    if INITIALIZED.swap(true, Ordering::SeqCst) {
        return;
    }

    crate::kprintln!("guc_huc: Initializing Intel GuC/HuC firmware loader...");

    let state = GucHucState {
        mmio_base,
        gen,
        guc_status: FirmwareStatus::NotLoaded,
        huc_status: FirmwareStatus::NotLoaded,
        guc_version: None,
        huc_version: None,
        guc_submission: false,
    };

    *GUC_HUC_STATE.lock() = Some(state);

    // Try to load firmware
    if let Err(e) = load_guc_firmware() {
        crate::kprintln!("guc_huc: GuC firmware load failed: {:?}", e);
    }

    if let Err(e) = load_huc_firmware() {
        crate::kprintln!("guc_huc: HuC firmware load failed: {:?}", e);
    }

    crate::kprintln!("guc_huc: Initialization complete");
}

/// Load GuC firmware
fn load_guc_firmware() -> Result<(), GucError> {
    let mut state = GUC_HUC_STATE.lock();
    let state = state.as_mut().ok_or(GucError::NotInitialized)?;

    state.guc_status = FirmwareStatus::Loading;

    // Get firmware path based on GPU generation
    let fw_path = get_guc_firmware_path(state.gen);
    crate::kprintln!("guc_huc: Loading GuC firmware: {}", fw_path);

    // Request firmware from filesystem
    let fw_data = firmware::request_firmware(&fw_path)
        .map_err(|_| GucError::FirmwareNotFound)?;

    // Validate firmware header
    if fw_data.len() < core::mem::size_of::<GucUcodeHeader>() {
        state.guc_status = FirmwareStatus::Failed;
        return Err(GucError::InvalidFirmware);
    }

    let header = unsafe {
        core::ptr::read_unaligned(fw_data.as_ptr() as *const GucUcodeHeader)
    };

    // Copy values from packed struct
    let ucode_version = { header.ucode_version };
    let ucode_size = { header.ucode_size };

    let major = (ucode_version >> 24) & 0xFF;
    let minor = (ucode_version >> 16) & 0xFF;
    let patch = ucode_version & 0xFFFF;

    state.guc_version = Some(FirmwareVersion {
        major,
        minor,
        patch,
    });

    crate::kprintln!("guc_huc: GuC firmware version: {}.{}.{}", major, minor, patch);

    // Upload firmware to WOPCM (Write Once Protected Content Memory)
    upload_guc_firmware(state.mmio_base, &fw_data)?;

    // Start GuC
    start_guc(state.mmio_base)?;

    // Verify GuC is running
    if verify_guc_running(state.mmio_base) {
        state.guc_status = FirmwareStatus::Running;
        crate::kprintln!("guc_huc: GuC firmware running");
        Ok(())
    } else {
        state.guc_status = FirmwareStatus::Failed;
        Err(GucError::StartFailed)
    }
}

/// Load HuC firmware
fn load_huc_firmware() -> Result<(), GucError> {
    let mut state = GUC_HUC_STATE.lock();
    let state = state.as_mut().ok_or(GucError::NotInitialized)?;

    // HuC requires GuC to be running for Gen11+
    if state.gen as u32 >= IntelGen::Gen11 as u32 && state.guc_status != FirmwareStatus::Running {
        state.huc_status = FirmwareStatus::Disabled;
        return Ok(());
    }

    state.huc_status = FirmwareStatus::Loading;

    let fw_path = get_huc_firmware_path(state.gen);
    crate::kprintln!("guc_huc: Loading HuC firmware: {}", fw_path);

    let fw_data = firmware::request_firmware(&fw_path)
        .map_err(|_| GucError::FirmwareNotFound)?;

    // Validate firmware
    if fw_data.len() < core::mem::size_of::<CssHeader>() {
        state.huc_status = FirmwareStatus::Failed;
        return Err(GucError::InvalidFirmware);
    }

    let header = unsafe {
        core::ptr::read_unaligned(fw_data.as_ptr() as *const CssHeader)
    };

    let sw_version = { header.sw_version };
    let major = (sw_version >> 24) & 0xFF;
    let minor = (sw_version >> 16) & 0xFF;
    let patch = sw_version & 0xFFFF;

    state.huc_version = Some(FirmwareVersion {
        major,
        minor,
        patch,
    });

    crate::kprintln!("guc_huc: HuC firmware version: {}.{}.{}", major, minor, patch);

    // Upload HuC firmware via GuC
    upload_huc_firmware(state.mmio_base, &fw_data)?;

    // Verify HuC is authenticated
    if verify_huc_authenticated(state.mmio_base) {
        state.huc_status = FirmwareStatus::Running;
        crate::kprintln!("guc_huc: HuC firmware authenticated");
        Ok(())
    } else {
        state.huc_status = FirmwareStatus::Failed;
        Err(GucError::AuthenticationFailed)
    }
}

/// Get GuC firmware path for GPU generation
fn get_guc_firmware_path(gen: IntelGen) -> String {
    match gen {
        IntelGen::Gen9 => String::from("i915/skl_guc_70.1.1.bin"),
        IntelGen::Gen9p5 => String::from("i915/kbl_guc_70.1.1.bin"),
        IntelGen::Gen11 => String::from("i915/icl_guc_70.1.1.bin"),
        IntelGen::Gen12 => String::from("i915/tgl_guc_70.1.1.bin"),
        IntelGen::Gen12p5 => String::from("i915/adlp_guc_70.1.1.bin"),
        IntelGen::Gen12p7 => String::from("i915/dg2_guc_70.1.1.bin"),
        _ => String::from("i915/guc_70.1.1.bin"),
    }
}

/// Get HuC firmware path for GPU generation
fn get_huc_firmware_path(gen: IntelGen) -> String {
    match gen {
        IntelGen::Gen9 => String::from("i915/skl_huc_2.0.0.bin"),
        IntelGen::Gen9p5 => String::from("i915/kbl_huc_4.0.0.bin"),
        IntelGen::Gen11 => String::from("i915/icl_huc_9.0.0.bin"),
        IntelGen::Gen12 => String::from("i915/tgl_huc_7.9.3.bin"),
        IntelGen::Gen12p5 => String::from("i915/adlp_huc_9.3.0.bin"),
        IntelGen::Gen12p7 => String::from("i915/dg2_huc_7.10.3.bin"),
        _ => String::from("i915/huc_7.0.0.bin"),
    }
}

/// Upload GuC firmware to WOPCM
fn upload_guc_firmware(mmio_base: u64, fw_data: &[u8]) -> Result<(), GucError> {
    unsafe {
        // Configure WOPCM size
        let wopcm_size = 0x400000; // 4MB
        core::ptr::write_volatile(
            (mmio_base + regs::GUC_WOPCM_SIZE) as *mut u32,
            wopcm_size,
        );

        // Allocate DMA buffer for firmware
        let num_pages = (fw_data.len() + 4095) / 4096;
        let dma_frame = crate::mm::alloc_frames(num_pages)
            .ok_or(GucError::AllocationFailed)?;
        let dma_buffer = dma_frame.start_address().as_u64();

        // Copy firmware to DMA buffer - need to use virtual address
        let virt_addr = crate::mm::phys_to_virt(dma_frame.start_address());
        core::ptr::copy_nonoverlapping(
            fw_data.as_ptr(),
            virt_addr.as_mut_ptr::<u8>(),
            fw_data.len(),
        );

        // Setup DMA transfer
        core::ptr::write_volatile(
            (mmio_base + regs::DMA_ADDR_0) as *mut u32,
            (dma_buffer as u32) & 0xFFFFFFFF,
        );
        core::ptr::write_volatile(
            (mmio_base + regs::DMA_ADDR_1) as *mut u32,
            ((dma_buffer >> 32) as u32) & 0xFFFF,
        );

        // Set WOPCM offset
        core::ptr::write_volatile(
            (mmio_base + regs::DMA_GUC_WOPCM_OFFSET) as *mut u32,
            0,
        );

        // Start DMA
        core::ptr::write_volatile(
            (mmio_base + regs::DMA_START) as *mut u32,
            fw_data.len() as u32,
        );

        // Wait for DMA completion
        let mut timeout = 1000000;
        while timeout > 0 {
            let status = core::ptr::read_volatile(
                (mmio_base + regs::DMA_CTRL) as *const u32
            );
            if status & 0x01 == 0 {
                break;
            }
            timeout -= 1;
            core::hint::spin_loop();
        }

        // Free DMA buffer
        crate::mm::free_frames(dma_frame, num_pages);

        if timeout == 0 {
            return Err(GucError::DmaTimeout);
        }
    }

    Ok(())
}

/// Start GuC execution
fn start_guc(mmio_base: u64) -> Result<(), GucError> {
    unsafe {
        // Configure GuC shim
        let shim_control = core::ptr::read_volatile(
            (mmio_base + regs::GUC_SHIM_CONTROL) as *const u32
        );
        core::ptr::write_volatile(
            (mmio_base + regs::GUC_SHIM_CONTROL) as *mut u32,
            shim_control | 0x01, // Enable GuC
        );

        // Wait for GuC to start
        let mut timeout = 1000000;
        while timeout > 0 {
            let status = core::ptr::read_volatile(
                (mmio_base + regs::GUC_STATUS) as *const u32
            );
            if status & 0x01 != 0 {
                return Ok(());
            }
            timeout -= 1;
            core::hint::spin_loop();
        }
    }

    Err(GucError::StartFailed)
}

/// Verify GuC is running
fn verify_guc_running(mmio_base: u64) -> bool {
    unsafe {
        let status = core::ptr::read_volatile(
            (mmio_base + regs::GUC_STATUS) as *const u32
        );
        // Check MIA core running bit
        status & 0x01 != 0
    }
}

/// Upload HuC firmware via GuC
fn upload_huc_firmware(mmio_base: u64, fw_data: &[u8]) -> Result<(), GucError> {
    // HuC firmware is loaded via GuC DMA
    // Similar to GuC upload but to different memory region
    upload_guc_firmware(mmio_base, fw_data)
}

/// Verify HuC is authenticated
fn verify_huc_authenticated(mmio_base: u64) -> bool {
    unsafe {
        let status = core::ptr::read_volatile(
            (mmio_base + regs::HUC_STATUS) as *const u32
        );
        // Check authentication complete bit
        status & 0x01 != 0
    }
}

/// Send command to GuC
pub fn guc_send_action(action: &[u32]) -> Result<u32, GucError> {
    let state = GUC_HUC_STATE.lock();
    let state = state.as_ref().ok_or(GucError::NotInitialized)?;

    if state.guc_status != FirmwareStatus::Running {
        return Err(GucError::NotRunning);
    }

    unsafe {
        // Write action to scratch registers
        for (i, &word) in action.iter().enumerate() {
            if i >= 16 {
                break;
            }
            core::ptr::write_volatile(
                (state.mmio_base + regs::SOFT_SCRATCH_0 + (i as u64 * 4)) as *mut u32,
                word,
            );
        }

        // Trigger interrupt to GuC
        core::ptr::write_volatile(
            (state.mmio_base + regs::GUC_SEND_INTERRUPT) as *mut u32,
            1,
        );

        // Wait for response
        let mut timeout = 100000;
        while timeout > 0 {
            let response = core::ptr::read_volatile(
                (state.mmio_base + regs::SOFT_SCRATCH_0) as *const u32
            );
            if response != action[0] {
                return Ok(response);
            }
            timeout -= 1;
            core::hint::spin_loop();
        }
    }

    Err(GucError::Timeout)
}

/// Enable GuC submission (replace execlist-based submission)
pub fn enable_guc_submission() -> bool {
    let mut state = GUC_HUC_STATE.lock();
    let state = match state.as_mut() {
        Some(s) => s,
        None => return false,
    };

    if state.guc_status != FirmwareStatus::Running {
        return false;
    }

    // Enable GuC submission via action command
    let enable_action = [0x0100u32, 0x0001]; // INTEL_GUC_ACTION_ENABLE_SUBMISSION
    if guc_send_action(&enable_action).is_ok() {
        state.guc_submission = true;
        crate::kprintln!("guc_huc: GuC submission enabled");
        true
    } else {
        false
    }
}

/// Get GuC status
pub fn get_guc_status() -> FirmwareStatus {
    let state = GUC_HUC_STATE.lock();
    state.as_ref().map(|s| s.guc_status).unwrap_or(FirmwareStatus::NotLoaded)
}

/// Get HuC status
pub fn get_huc_status() -> FirmwareStatus {
    let state = GUC_HUC_STATE.lock();
    state.as_ref().map(|s| s.huc_status).unwrap_or(FirmwareStatus::NotLoaded)
}

/// Get GuC version
pub fn get_guc_version() -> Option<FirmwareVersion> {
    let state = GUC_HUC_STATE.lock();
    state.as_ref().and_then(|s| s.guc_version.clone())
}

/// Get HuC version
pub fn get_huc_version() -> Option<FirmwareVersion> {
    let state = GUC_HUC_STATE.lock();
    state.as_ref().and_then(|s| s.huc_version.clone())
}

/// GuC error types
#[derive(Debug)]
pub enum GucError {
    NotInitialized,
    FirmwareNotFound,
    InvalidFirmware,
    AllocationFailed,
    DmaTimeout,
    StartFailed,
    AuthenticationFailed,
    NotRunning,
    Timeout,
}
