//! AMD/ATI GPU Driver
//!
//! Basic driver for AMD Radeon GPUs (GCN through RDNA architectures).
//! Provides:
//! - PCI device detection and identification
//! - MMIO register access
//! - Display controller engine (DCE/DCN) configuration
//! - Basic mode setting
//! - Framebuffer management
//!
//! Supported GPU architectures:
//! - GCN 1.0 (Southern Islands): HD 7000 series
//! - GCN 1.1 (Sea Islands): R7/R9 200 series
//! - GCN 1.2 (Volcanic Islands): R9 300/Fury series
//! - GCN 3.0 (Arctic Islands / Polaris): RX 400/500 series
//! - GCN 5.0 (Vega): RX Vega series
//! - RDNA 1 (Navi 10): RX 5000 series
//! - RDNA 2 (Navi 2x): RX 6000 series
//! - RDNA 3 (Navi 3x): RX 7000 series

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use core::ptr;
use spin::Mutex;

/// AMD vendor ID
pub const AMD_VENDOR_ID: u16 = 0x1002;

/// AMD GPU device IDs by family
pub mod device_ids {
    // =========================================================================
    // GCN 1.0 (Southern Islands) - HD 7000 series
    // =========================================================================
    pub const TAHITI_XT: u16 = 0x6798;  // HD 7970
    pub const TAHITI_PRO: u16 = 0x679A; // HD 7950
    pub const TAHITI_LE: u16 = 0x679E;  // HD 7870 XT
    pub const PITCAIRN_XT: u16 = 0x6810; // HD 7870
    pub const PITCAIRN_PRO: u16 = 0x6811; // HD 7850
    pub const VERDE_XT: u16 = 0x6820;   // HD 7870M
    pub const VERDE_PRO: u16 = 0x6821;  // HD 7850M
    pub const OLAND: u16 = 0x6600;      // HD 8570
    pub const HAINAN: u16 = 0x6660;     // HD 8500M

    // =========================================================================
    // GCN 1.1 (Sea Islands) - R7/R9 200 series
    // =========================================================================
    pub const BONAIRE_XT: u16 = 0x6640; // R7 260X
    pub const BONAIRE_PRO: u16 = 0x6641; // R7 260
    pub const HAWAII_XT: u16 = 0x67B0;  // R9 290X
    pub const HAWAII_PRO: u16 = 0x67B1; // R9 290
    pub const KAVERI_1: u16 = 0x1304;   // A10-7850K APU
    pub const KAVERI_2: u16 = 0x1305;   // A8-7600 APU
    pub const KABINI: u16 = 0x9830;     // E1/E2 APU
    pub const MULLINS: u16 = 0x9850;    // A4/A6 Micro APU

    // =========================================================================
    // GCN 1.2 (Volcanic Islands) - R9 300/Fury series
    // =========================================================================
    pub const TONGA_XT: u16 = 0x6920;   // R9 380X
    pub const TONGA_PRO: u16 = 0x6921;  // R9 380
    pub const FIJI_XT: u16 = 0x7300;    // R9 Fury X
    pub const FIJI_PRO: u16 = 0x7301;   // R9 Fury
    pub const FIJI_NANO: u16 = 0x7302;  // R9 Nano
    pub const CARRIZO: u16 = 0x9870;    // A10-8700P APU
    pub const STONEY: u16 = 0x98E4;     // A9-9400 APU

    // =========================================================================
    // GCN 3.0 (Arctic Islands / Polaris) - RX 400/500 series
    // =========================================================================
    pub const POLARIS10_XT: u16 = 0x67DF; // RX 480
    pub const POLARIS10_PRO: u16 = 0x67EF; // RX 470
    pub const POLARIS11_XT: u16 = 0x67FF; // RX 460
    pub const POLARIS11_PRO: u16 = 0x67E0; // RX 560
    pub const POLARIS12: u16 = 0x699F;  // RX 550

    // RX 500 series (Polaris refresh)
    pub const POLARIS10_XT2: u16 = 0x67DF; // RX 580 (same PCI ID)
    pub const POLARIS10_PRO2: u16 = 0x67EF; // RX 570 (same PCI ID)
    pub const POLARIS11_XT2: u16 = 0x67FF; // RX 560 (same PCI ID)

    // =========================================================================
    // GCN 5.0 (Vega) - RX Vega series
    // =========================================================================
    pub const VEGA10_XT: u16 = 0x687F;  // RX Vega 64
    pub const VEGA10_XL: u16 = 0x687E;  // RX Vega 56
    pub const VEGA10_XTX: u16 = 0x6863; // Vega Frontier
    pub const VEGA12: u16 = 0x69AF;     // Vega 12
    pub const VEGA20_XT: u16 = 0x66A0;  // Radeon VII
    pub const VEGA20_XL: u16 = 0x66A1;  // Radeon Pro VII
    pub const RAVEN: u16 = 0x15DD;      // Ryzen APU (Vega 8/11)
    pub const RAVEN2: u16 = 0x15D8;     // Ryzen APU (Vega 3/8)
    pub const RENOIR: u16 = 0x1636;     // Ryzen 4000 APU

    // =========================================================================
    // RDNA 1 (Navi 10) - RX 5000 series
    // =========================================================================
    pub const NAVI10_XT: u16 = 0x731F;  // RX 5700 XT
    pub const NAVI10_XL: u16 = 0x7340;  // RX 5700
    pub const NAVI10_XLE: u16 = 0x7341; // RX 5600 XT
    pub const NAVI14_XT: u16 = 0x7360;  // RX 5500 XT
    pub const NAVI14_XTM: u16 = 0x7362; // RX 5500M
    pub const NAVI14_XLM: u16 = 0x7363; // RX 5300M

    // =========================================================================
    // RDNA 2 (Navi 2x) - RX 6000 series
    // =========================================================================
    pub const NAVI21_XT: u16 = 0x73BF;  // RX 6900 XT
    pub const NAVI21_XTX: u16 = 0x73A5; // RX 6950 XT
    pub const NAVI21_XL: u16 = 0x73A2;  // RX 6800 XT
    pub const NAVI21_LLXL: u16 = 0x73A3; // RX 6800
    pub const NAVI22_XT: u16 = 0x73DF;  // RX 6700 XT
    pub const NAVI22_XL: u16 = 0x73E0;  // RX 6700
    pub const NAVI23_XT: u16 = 0x73EF;  // RX 6600 XT
    pub const NAVI23_XL: u16 = 0x73E3;  // RX 6600
    pub const NAVI24_XT: u16 = 0x743F;  // RX 6500 XT
    pub const NAVI24_XL: u16 = 0x7422;  // RX 6400
    pub const VAN_GOGH: u16 = 0x163F;   // Steam Deck APU

    // =========================================================================
    // RDNA 3 (Navi 3x) - RX 7000 series
    // =========================================================================
    pub const NAVI31_XT: u16 = 0x744C;  // RX 7900 XTX
    pub const NAVI31_XL: u16 = 0x744E;  // RX 7900 XT
    pub const NAVI32_XT: u16 = 0x7470;  // RX 7800 XT
    pub const NAVI32_XL: u16 = 0x7471;  // RX 7700 XT
    pub const NAVI33_XT: u16 = 0x7480;  // RX 7600
    pub const PHOENIX: u16 = 0x15BF;    // Ryzen 7040 APU
}

/// GPU architecture/family
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuFamily {
    /// GCN 1.0 - Southern Islands (HD 7000)
    SouthernIslands,
    /// GCN 1.1 - Sea Islands (R7/R9 200)
    SeaIslands,
    /// GCN 1.2 - Volcanic Islands (R9 300/Fury)
    VolcanicIslands,
    /// GCN 3.0 - Polaris (RX 400/500)
    Polaris,
    /// GCN 5.0 - Vega (RX Vega)
    Vega,
    /// RDNA 1 - Navi 10 (RX 5000)
    Navi10,
    /// RDNA 2 - Navi 2x (RX 6000)
    Navi2x,
    /// RDNA 3 - Navi 3x (RX 7000)
    Navi3x,
    /// Unknown family
    Unknown,
}

impl GpuFamily {
    pub fn from_device_id(device_id: u16) -> Self {
        use device_ids::*;
        match device_id {
            // Southern Islands
            TAHITI_XT | TAHITI_PRO | TAHITI_LE | PITCAIRN_XT | PITCAIRN_PRO
            | VERDE_XT | VERDE_PRO | OLAND | HAINAN => GpuFamily::SouthernIslands,

            // Sea Islands
            BONAIRE_XT | BONAIRE_PRO | HAWAII_XT | HAWAII_PRO
            | KAVERI_1 | KAVERI_2 | KABINI | MULLINS => GpuFamily::SeaIslands,

            // Volcanic Islands
            TONGA_XT | TONGA_PRO | FIJI_XT | FIJI_PRO | FIJI_NANO
            | CARRIZO | STONEY => GpuFamily::VolcanicIslands,

            // Polaris
            POLARIS10_XT | POLARIS10_PRO | POLARIS11_XT | POLARIS11_PRO
            | POLARIS12 => GpuFamily::Polaris,

            // Vega
            VEGA10_XT | VEGA10_XL | VEGA10_XTX | VEGA12 | VEGA20_XT | VEGA20_XL
            | RAVEN | RAVEN2 | RENOIR => GpuFamily::Vega,

            // Navi 10 (RDNA 1)
            NAVI10_XT | NAVI10_XL | NAVI10_XLE | NAVI14_XT | NAVI14_XTM
            | NAVI14_XLM => GpuFamily::Navi10,

            // Navi 2x (RDNA 2)
            NAVI21_XT | NAVI21_XTX | NAVI21_XL | NAVI21_LLXL | NAVI22_XT | NAVI22_XL
            | NAVI23_XT | NAVI23_XL | NAVI24_XT | NAVI24_XL | VAN_GOGH => GpuFamily::Navi2x,

            // Navi 3x (RDNA 3)
            NAVI31_XT | NAVI31_XL | NAVI32_XT | NAVI32_XL | NAVI33_XT
            | PHOENIX => GpuFamily::Navi3x,

            _ => GpuFamily::Unknown,
        }
    }

    /// Get architecture name
    pub fn name(&self) -> &'static str {
        match self {
            GpuFamily::SouthernIslands => "GCN 1.0 (Southern Islands)",
            GpuFamily::SeaIslands => "GCN 1.1 (Sea Islands)",
            GpuFamily::VolcanicIslands => "GCN 1.2 (Volcanic Islands)",
            GpuFamily::Polaris => "GCN 3.0 (Polaris)",
            GpuFamily::Vega => "GCN 5.0 (Vega)",
            GpuFamily::Navi10 => "RDNA 1 (Navi)",
            GpuFamily::Navi2x => "RDNA 2 (Navi 2x)",
            GpuFamily::Navi3x => "RDNA 3 (Navi 3x)",
            GpuFamily::Unknown => "Unknown",
        }
    }

    /// Check if this is an APU (integrated graphics)
    pub fn is_apu(&self) -> bool {
        // APUs are detected by specific device IDs
        false
    }

    /// Get minimum VRAM requirement
    pub fn min_vram(&self) -> usize {
        match self {
            GpuFamily::SouthernIslands => 1024 * 1024 * 1024,  // 1 GB
            GpuFamily::SeaIslands => 1024 * 1024 * 1024,       // 1 GB
            GpuFamily::VolcanicIslands => 2048 * 1024 * 1024,  // 2 GB
            GpuFamily::Polaris => 2048 * 1024 * 1024,          // 2 GB
            GpuFamily::Vega => 4096 * 1024 * 1024,             // 4 GB (HBM)
            GpuFamily::Navi10 => 4096 * 1024 * 1024,           // 4 GB
            GpuFamily::Navi2x => 4096 * 1024 * 1024,           // 4 GB
            GpuFamily::Navi3x => 8192 * 1024 * 1024,           // 8 GB
            GpuFamily::Unknown => 256 * 1024 * 1024,           // 256 MB
        }
    }

    /// Uses DCN (Display Core Next) instead of DCE
    pub fn uses_dcn(&self) -> bool {
        matches!(self,
            GpuFamily::Vega | GpuFamily::Navi10 | GpuFamily::Navi2x | GpuFamily::Navi3x)
    }
}

// =============================================================================
// Register Offsets
// =============================================================================

/// MMIO register offsets
pub mod regs {
    // =========================================================================
    // General Purpose Registers
    // =========================================================================
    pub const MM_INDEX: u32 = 0x0000;
    pub const MM_DATA: u32 = 0x0004;

    // GPU identification
    pub const CONFIG_MEMSIZE: u32 = 0x5428;
    pub const CONFIG_APER_SIZE: u32 = 0x5430;

    // =========================================================================
    // Display Controller Engine (DCE) - GCN
    // =========================================================================
    pub const CRTC_H_TOTAL: u32 = 0x6880;
    pub const CRTC_H_BLANK_START_END: u32 = 0x6884;
    pub const CRTC_H_SYNC_A: u32 = 0x6888;
    pub const CRTC_V_TOTAL: u32 = 0x688C;
    pub const CRTC_V_BLANK_START_END: u32 = 0x6890;
    pub const CRTC_V_SYNC_A: u32 = 0x6894;
    pub const CRTC_CONTROL: u32 = 0x6880;
    pub const CRTC_STATUS: u32 = 0x6898;

    // Primary surface
    pub const GRPH_ENABLE: u32 = 0x6900;
    pub const GRPH_CONTROL: u32 = 0x6904;
    pub const GRPH_SWAP_CONTROL: u32 = 0x690C;
    pub const GRPH_PRIMARY_SURFACE_ADDRESS: u32 = 0x6910;
    pub const GRPH_PRIMARY_SURFACE_ADDRESS_HIGH: u32 = 0x6914;
    pub const GRPH_SECONDARY_SURFACE_ADDRESS: u32 = 0x6918;
    pub const GRPH_SECONDARY_SURFACE_ADDRESS_HIGH: u32 = 0x691C;
    pub const GRPH_PITCH: u32 = 0x6920;
    pub const GRPH_SURFACE_OFFSET_X: u32 = 0x6924;
    pub const GRPH_SURFACE_OFFSET_Y: u32 = 0x6928;
    pub const GRPH_X_START: u32 = 0x692C;
    pub const GRPH_Y_START: u32 = 0x6930;
    pub const GRPH_X_END: u32 = 0x6934;
    pub const GRPH_Y_END: u32 = 0x6938;
    pub const GRPH_UPDATE: u32 = 0x6940;

    // Cursor
    pub const CUR_CONTROL: u32 = 0x6998;
    pub const CUR_SURFACE_ADDRESS: u32 = 0x699C;
    pub const CUR_SIZE: u32 = 0x69A4;
    pub const CUR_POSITION: u32 = 0x69A8;
    pub const CUR_HOT_SPOT: u32 = 0x69AC;

    // Output connectors
    pub const DAC_ENABLE: u32 = 0x7800;
    pub const DAC_SOURCE_SELECT: u32 = 0x7804;
    pub const DAC_CONTROL: u32 = 0x7808;
    pub const DAC_COMPARATOR_ENABLE: u32 = 0x7810;
    pub const DAC_COMPARATOR_OUTPUT: u32 = 0x7814;

    pub const LVDS_DATA_ENABLE: u32 = 0x7A10;
    pub const LVDS_CONTROL: u32 = 0x7A00;

    pub const DIG_ENABLE: u32 = 0x79A4;  // Digital encoder (DP/HDMI)
    pub const DIG_SOURCE_SELECT: u32 = 0x79A8;

    // =========================================================================
    // Display Core Next (DCN) - Vega/RDNA
    // =========================================================================
    pub const HUBP0_DCSURF_SURFACE_CONFIG: u32 = 0x5E00;
    pub const HUBP0_DCSURF_ADDR_CONFIG: u32 = 0x5E04;
    pub const HUBP0_DCSURF_TILING_CONFIG: u32 = 0x5E08;
    pub const HUBP0_DCSURF_PRI_VIEWPORT_START: u32 = 0x5E0C;
    pub const HUBP0_DCSURF_PRI_VIEWPORT_DIMENSION: u32 = 0x5E10;
    pub const HUBP0_DCSURF_PRIMARY_SURFACE_ADDRESS: u32 = 0x5E14;
    pub const HUBP0_DCSURF_PRIMARY_SURFACE_ADDRESS_HIGH: u32 = 0x5E18;

    pub const OTG0_OTG_H_TOTAL: u32 = 0x1B00;
    pub const OTG0_OTG_H_BLANK_START_END: u32 = 0x1B04;
    pub const OTG0_OTG_H_SYNC_A: u32 = 0x1B08;
    pub const OTG0_OTG_V_TOTAL: u32 = 0x1B0C;
    pub const OTG0_OTG_V_BLANK_START_END: u32 = 0x1B10;
    pub const OTG0_OTG_V_SYNC_A: u32 = 0x1B14;
    pub const OTG0_OTG_CONTROL: u32 = 0x1B80;
    pub const OTG0_OTG_STATUS: u32 = 0x1B84;

    // =========================================================================
    // Memory Controller
    // =========================================================================
    pub const MC_VM_FB_LOCATION: u32 = 0x2000;
    pub const MC_VM_AGP_BASE: u32 = 0x2004;
    pub const MC_VM_AGP_BOT: u32 = 0x2008;
    pub const MC_VM_AGP_TOP: u32 = 0x200C;
    pub const MC_VM_SYSTEM_APERTURE_LOW_ADDR: u32 = 0x2034;
    pub const MC_VM_SYSTEM_APERTURE_HIGH_ADDR: u32 = 0x2038;

    // =========================================================================
    // GRBM (Graphics Register Bus Manager)
    // =========================================================================
    pub const GRBM_STATUS: u32 = 0x8010;
    pub const GRBM_STATUS2: u32 = 0x8014;
    pub const GRBM_SOFT_RESET: u32 = 0x8020;

    // =========================================================================
    // Interrupts
    // =========================================================================
    pub const IH_RB_CNTL: u32 = 0x3E00;
    pub const IH_RB_BASE: u32 = 0x3E04;
    pub const IH_RB_RPTR: u32 = 0x3E08;
    pub const IH_RB_WPTR: u32 = 0x3E0C;
    pub const IH_CNTL: u32 = 0x3E18;

    // =========================================================================
    // Power Management
    // =========================================================================
    pub const CG_SPLL_FUNC_CNTL: u32 = 0x600;
    pub const CG_SPLL_FUNC_CNTL_2: u32 = 0x604;
    pub const MPLL_CNTL_MODE: u32 = 0x620;
    pub const SMC_IND_INDEX: u32 = 0x80;
    pub const SMC_IND_DATA: u32 = 0x84;
}

/// Graphics format bits
pub mod grph_control {
    pub const GRPH_DEPTH_8BPP: u32 = 0;
    pub const GRPH_DEPTH_16BPP: u32 = 1;
    pub const GRPH_DEPTH_32BPP: u32 = 2;

    pub const GRPH_FORMAT_INDEXED: u32 = 0 << 8;
    pub const GRPH_FORMAT_ARGB1555: u32 = 1 << 8;
    pub const GRPH_FORMAT_ARGB565: u32 = 2 << 8;
    pub const GRPH_FORMAT_ARGB4444: u32 = 3 << 8;
    pub const GRPH_FORMAT_ARGB8888: u32 = 0 << 8;  // With 32bpp depth
    pub const GRPH_FORMAT_ARGB2101010: u32 = 1 << 8;
    pub const GRPH_FORMAT_FP16: u32 = 3 << 8;
}

/// CRTC control bits
pub mod crtc_control {
    pub const CRTC_MASTER_EN: u32 = 1 << 0;
    pub const CRTC_DISP_READ_REQUEST_DISABLE: u32 = 1 << 24;
}

// =============================================================================
// Display Mode
// =============================================================================

/// Display mode information
#[derive(Debug, Clone, Copy)]
pub struct AmdDisplayMode {
    pub width: u32,
    pub height: u32,
    pub bpp: u32,
    pub pixel_clock: u32,  // in kHz
    pub h_total: u32,
    pub h_blank_start: u32,
    pub h_blank_end: u32,
    pub h_sync_start: u32,
    pub h_sync_end: u32,
    pub v_total: u32,
    pub v_blank_start: u32,
    pub v_blank_end: u32,
    pub v_sync_start: u32,
    pub v_sync_end: u32,
    pub refresh_rate: u32,
}

impl AmdDisplayMode {
    /// Create a standard 1920x1080@60Hz mode
    pub fn mode_1080p() -> Self {
        Self {
            width: 1920,
            height: 1080,
            bpp: 32,
            pixel_clock: 148500,
            h_total: 2200,
            h_blank_start: 1920,
            h_blank_end: 2200,
            h_sync_start: 2008,
            h_sync_end: 2052,
            v_total: 1125,
            v_blank_start: 1080,
            v_blank_end: 1125,
            v_sync_start: 1084,
            v_sync_end: 1089,
            refresh_rate: 60,
        }
    }

    /// Create a standard 1280x720@60Hz mode
    pub fn mode_720p() -> Self {
        Self {
            width: 1280,
            height: 720,
            bpp: 32,
            pixel_clock: 74250,
            h_total: 1650,
            h_blank_start: 1280,
            h_blank_end: 1650,
            h_sync_start: 1390,
            h_sync_end: 1430,
            v_total: 750,
            v_blank_start: 720,
            v_blank_end: 750,
            v_sync_start: 725,
            v_sync_end: 730,
            refresh_rate: 60,
        }
    }

    /// Create a standard 2560x1440@60Hz mode
    pub fn mode_1440p() -> Self {
        Self {
            width: 2560,
            height: 1440,
            bpp: 32,
            pixel_clock: 241500,
            h_total: 2720,
            h_blank_start: 2560,
            h_blank_end: 2720,
            h_sync_start: 2608,
            h_sync_end: 2640,
            v_total: 1481,
            v_blank_start: 1440,
            v_blank_end: 1481,
            v_sync_start: 1443,
            v_sync_end: 1448,
            refresh_rate: 60,
        }
    }

    /// Create a standard 3840x2160@60Hz (4K) mode
    pub fn mode_4k() -> Self {
        Self {
            width: 3840,
            height: 2160,
            bpp: 32,
            pixel_clock: 594000,
            h_total: 4400,
            h_blank_start: 3840,
            h_blank_end: 4400,
            h_sync_start: 4016,
            h_sync_end: 4104,
            v_total: 2250,
            v_blank_start: 2160,
            v_blank_end: 2250,
            v_sync_start: 2168,
            v_sync_end: 2178,
            refresh_rate: 60,
        }
    }

    /// Stride in bytes
    pub fn stride(&self) -> u32 {
        // AMD GPUs require 256-byte aligned pitch
        let raw_stride = self.width * (self.bpp / 8);
        (raw_stride + 255) & !255
    }

    /// Framebuffer size in bytes
    pub fn framebuffer_size(&self) -> usize {
        (self.stride() * self.height) as usize
    }
}

// =============================================================================
// AMD GPU Driver
// =============================================================================

/// AMD GPU driver state
pub struct AmdGpu {
    /// PCI device info
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    /// Device ID
    pub device_id: u16,
    /// GPU family
    pub family: GpuFamily,
    /// MMIO base address (BAR0)
    pub mmio_base: u64,
    /// MMIO size
    pub mmio_size: usize,
    /// VRAM aperture base (BAR2)
    pub vram_base: u64,
    /// VRAM aperture size
    pub vram_size: usize,
    /// Doorbell aperture (BAR4) - for newer GPUs
    pub doorbell_base: u64,
    /// Doorbell size
    pub doorbell_size: usize,
    /// Current display mode
    pub current_mode: Option<AmdDisplayMode>,
    /// Framebuffer offset in VRAM
    pub framebuffer_offset: u64,
    /// Is initialized
    pub initialized: bool,
    /// Active CRTC (display controller)
    pub active_crtc: u8,
    /// Is an APU (integrated graphics)
    pub is_apu: bool,
    /// Detected VRAM size
    pub detected_vram: usize,
}

impl AmdGpu {
    /// Create a new AMD GPU driver instance
    pub const fn new() -> Self {
        Self {
            bus: 0,
            device: 0,
            function: 0,
            device_id: 0,
            family: GpuFamily::Unknown,
            mmio_base: 0,
            mmio_size: 0,
            vram_base: 0,
            vram_size: 0,
            doorbell_base: 0,
            doorbell_size: 0,
            current_mode: None,
            framebuffer_offset: 0,
            initialized: false,
            active_crtc: 0,
            is_apu: false,
            detected_vram: 0,
        }
    }

    /// Read 32-bit MMIO register
    pub unsafe fn read32(&self, offset: u32) -> u32 {
        let addr = (self.mmio_base + offset as u64) as *const u32;
        ptr::read_volatile(addr)
    }

    /// Write 32-bit MMIO register
    pub unsafe fn write32(&self, offset: u32, value: u32) {
        let addr = (self.mmio_base + offset as u64) as *mut u32;
        ptr::write_volatile(addr, value);
    }

    /// Read indirect register (through MM_INDEX/MM_DATA)
    pub unsafe fn read_indirect(&self, reg: u32) -> u32 {
        self.write32(regs::MM_INDEX, reg);
        self.read32(regs::MM_DATA)
    }

    /// Write indirect register
    pub unsafe fn write_indirect(&self, reg: u32, value: u32) {
        self.write32(regs::MM_INDEX, reg);
        self.write32(regs::MM_DATA, value);
    }

    /// Initialize GPU
    pub fn init(&mut self) -> Result<(), &'static str> {
        if self.mmio_base == 0 {
            return Err("MMIO not mapped");
        }

        unsafe {
            // Read VRAM size from MC registers
            self.detect_vram();

            // Disable interrupts initially
            self.disable_interrupts();

            // Initialize display controller
            self.init_display()?;
        }

        self.initialized = true;
        Ok(())
    }

    /// Detect VRAM size
    unsafe fn detect_vram(&mut self) {
        let config_memsize = self.read32(regs::CONFIG_MEMSIZE);
        self.detected_vram = (config_memsize as usize) * 1024 * 1024;

        // Fallback to minimum for family if detection fails
        if self.detected_vram == 0 {
            self.detected_vram = self.family.min_vram();
        }

        crate::kprintln!("amd_gpu: VRAM size: {} MB", self.detected_vram / (1024 * 1024));
    }

    /// Disable interrupts
    unsafe fn disable_interrupts(&self) {
        self.write32(regs::IH_CNTL, 0);
        self.write32(regs::IH_RB_CNTL, 0);
    }

    /// Initialize display controller
    unsafe fn init_display(&mut self) -> Result<(), &'static str> {
        self.active_crtc = 0;

        if self.family.uses_dcn() {
            self.init_dcn()?;
        } else {
            self.init_dce()?;
        }

        Ok(())
    }

    /// Initialize DCE (Display Controller Engine) for GCN GPUs
    unsafe fn init_dce(&self) -> Result<(), &'static str> {
        // Check if display is already enabled
        let crtc_status = self.read32(regs::CRTC_STATUS);

        if crtc_status != 0 {
            crate::kprintln!("amd_gpu: DCE CRTC already enabled");
        }

        Ok(())
    }

    /// Initialize DCN (Display Core Next) for Vega/RDNA GPUs
    unsafe fn init_dcn(&self) -> Result<(), &'static str> {
        // Check OTG (Output Timing Generator) status
        let otg_status = self.read32(regs::OTG0_OTG_STATUS);

        if otg_status != 0 {
            crate::kprintln!("amd_gpu: DCN OTG already active");
        }

        Ok(())
    }

    /// Set display mode
    pub fn set_mode(&mut self, mode: &AmdDisplayMode) -> Result<(), &'static str> {
        if !self.initialized {
            return Err("GPU not initialized");
        }

        unsafe {
            if self.family.uses_dcn() {
                self.set_mode_dcn(mode)?;
            } else {
                self.set_mode_dce(mode)?;
            }
        }

        self.current_mode = Some(*mode);

        crate::kprintln!(
            "amd_gpu: mode set to {}x{}@{}Hz",
            mode.width,
            mode.height,
            mode.refresh_rate
        );

        Ok(())
    }

    /// Set mode using DCE (GCN)
    unsafe fn set_mode_dce(&self, mode: &AmdDisplayMode) -> Result<(), &'static str> {
        // Disable CRTC
        self.write32(regs::CRTC_CONTROL, 0);

        // Wait for disable
        for _ in 0..1000 {
            let status = self.read32(regs::CRTC_STATUS);
            if status == 0 {
                break;
            }
            core::hint::spin_loop();
        }

        // Set timing registers
        self.write32(regs::CRTC_H_TOTAL, mode.h_total - 1);
        self.write32(
            regs::CRTC_H_BLANK_START_END,
            ((mode.h_blank_end - 1) << 16) | (mode.h_blank_start - 1),
        );
        self.write32(
            regs::CRTC_H_SYNC_A,
            ((mode.h_sync_end - 1) << 16) | (mode.h_sync_start - 1),
        );

        self.write32(regs::CRTC_V_TOTAL, mode.v_total - 1);
        self.write32(
            regs::CRTC_V_BLANK_START_END,
            ((mode.v_blank_end - 1) << 16) | (mode.v_blank_start - 1),
        );
        self.write32(
            regs::CRTC_V_SYNC_A,
            ((mode.v_sync_end - 1) << 16) | (mode.v_sync_start - 1),
        );

        // Configure graphics plane
        self.write32(regs::GRPH_ENABLE, 1);

        // Set pixel format (ARGB8888 with 32bpp)
        let grph_control = grph_control::GRPH_DEPTH_32BPP | grph_control::GRPH_FORMAT_ARGB8888;
        self.write32(regs::GRPH_CONTROL, grph_control);

        // Set pitch (stride in pixels, AMD uses 256-byte alignment)
        let pitch_pixels = mode.stride() / (mode.bpp / 8);
        self.write32(regs::GRPH_PITCH, pitch_pixels);

        // Set surface address (framebuffer in VRAM)
        self.write32(regs::GRPH_PRIMARY_SURFACE_ADDRESS, self.framebuffer_offset as u32);
        self.write32(
            regs::GRPH_PRIMARY_SURFACE_ADDRESS_HIGH,
            (self.framebuffer_offset >> 32) as u32,
        );

        // Set display size
        self.write32(regs::GRPH_X_START, 0);
        self.write32(regs::GRPH_Y_START, 0);
        self.write32(regs::GRPH_X_END, mode.width);
        self.write32(regs::GRPH_Y_END, mode.height);

        // Enable CRTC
        self.write32(regs::CRTC_CONTROL, crtc_control::CRTC_MASTER_EN);

        // Wait for CRTC to enable
        for _ in 0..1000 {
            let status = self.read32(regs::CRTC_STATUS);
            if status != 0 {
                break;
            }
            core::hint::spin_loop();
        }

        Ok(())
    }

    /// Set mode using DCN (Vega/RDNA)
    unsafe fn set_mode_dcn(&self, mode: &AmdDisplayMode) -> Result<(), &'static str> {
        // Disable OTG
        self.write32(regs::OTG0_OTG_CONTROL, 0);

        // Wait for disable
        for _ in 0..1000 {
            let status = self.read32(regs::OTG0_OTG_STATUS);
            if status == 0 {
                break;
            }
            core::hint::spin_loop();
        }

        // Set timing
        self.write32(regs::OTG0_OTG_H_TOTAL, mode.h_total - 1);
        self.write32(
            regs::OTG0_OTG_H_BLANK_START_END,
            ((mode.h_blank_end - 1) << 16) | (mode.h_blank_start - 1),
        );
        self.write32(
            regs::OTG0_OTG_H_SYNC_A,
            ((mode.h_sync_end - 1) << 16) | (mode.h_sync_start - 1),
        );

        self.write32(regs::OTG0_OTG_V_TOTAL, mode.v_total - 1);
        self.write32(
            regs::OTG0_OTG_V_BLANK_START_END,
            ((mode.v_blank_end - 1) << 16) | (mode.v_blank_start - 1),
        );
        self.write32(
            regs::OTG0_OTG_V_SYNC_A,
            ((mode.v_sync_end - 1) << 16) | (mode.v_sync_start - 1),
        );

        // Configure HUBP (Hub Pipe) for framebuffer
        self.write32(regs::HUBP0_DCSURF_PRIMARY_SURFACE_ADDRESS, self.framebuffer_offset as u32);
        self.write32(
            regs::HUBP0_DCSURF_PRIMARY_SURFACE_ADDRESS_HIGH,
            (self.framebuffer_offset >> 32) as u32,
        );

        // Set viewport
        self.write32(regs::HUBP0_DCSURF_PRI_VIEWPORT_START, 0);
        self.write32(
            regs::HUBP0_DCSURF_PRI_VIEWPORT_DIMENSION,
            (mode.height << 16) | mode.width,
        );

        // Enable OTG
        self.write32(regs::OTG0_OTG_CONTROL, 1);

        Ok(())
    }

    /// Get framebuffer address for CPU access
    pub fn framebuffer_address(&self) -> u64 {
        self.vram_base + self.framebuffer_offset
    }

    /// Get framebuffer size
    pub fn framebuffer_size(&self) -> usize {
        self.current_mode.map_or(0, |m| m.framebuffer_size())
    }

    /// Wait for vertical blank
    pub fn wait_vblank(&self) {
        if !self.initialized {
            return;
        }

        unsafe {
            let status_reg = if self.family.uses_dcn() {
                regs::OTG0_OTG_STATUS
            } else {
                regs::CRTC_STATUS
            };

            // Wait for vblank bit
            for _ in 0..100000 {
                let status = self.read32(status_reg);
                // Bit 0 typically indicates vblank
                if status & 1 != 0 {
                    break;
                }
                core::hint::spin_loop();
            }
        }
    }

    /// Set cursor position
    pub fn set_cursor_position(&self, x: u32, y: u32) {
        if !self.initialized {
            return;
        }

        unsafe {
            self.write32(regs::CUR_POSITION, (y << 16) | x);
        }
    }

    /// Enable/disable cursor
    pub fn set_cursor_visible(&self, visible: bool) {
        if !self.initialized {
            return;
        }

        unsafe {
            let ctrl = if visible { 1 } else { 0 };
            self.write32(regs::CUR_CONTROL, ctrl);
        }
    }

    /// Disable GPU
    pub fn disable(&mut self) {
        if !self.initialized {
            return;
        }

        unsafe {
            // Disable graphics plane
            self.write32(regs::GRPH_ENABLE, 0);

            // Disable CRTC/OTG
            if self.family.uses_dcn() {
                self.write32(regs::OTG0_OTG_CONTROL, 0);
            } else {
                self.write32(regs::CRTC_CONTROL, 0);
            }
        }

        self.initialized = false;
    }
}

// =============================================================================
// Global State
// =============================================================================

/// Global GPU instance
static AMD_GPU: Mutex<Option<AmdGpu>> = Mutex::new(None);

/// Check if device ID is an AMD GPU
pub fn is_amd_gpu(vendor_id: u16, device_id: u16) -> bool {
    if vendor_id != AMD_VENDOR_ID {
        return false;
    }
    GpuFamily::from_device_id(device_id) != GpuFamily::Unknown
}

/// Initialize AMD GPU from PCI
pub fn init_from_pci(
    bus: u8,
    device: u8,
    function: u8,
    device_id: u16,
    bar0: u64,
    bar2: u64,
    bar4: u64,
) -> Result<(), &'static str> {
    let family = GpuFamily::from_device_id(device_id);

    if family == GpuFamily::Unknown {
        return Err("Unknown AMD GPU");
    }

    let mut gpu = AmdGpu::new();
    gpu.bus = bus;
    gpu.device = device;
    gpu.function = function;
    gpu.device_id = device_id;
    gpu.family = family;
    gpu.mmio_base = bar0;
    gpu.mmio_size = 256 * 1024; // 256 KB typical for MMIO
    gpu.vram_base = bar2;
    gpu.vram_size = family.min_vram();
    gpu.doorbell_base = bar4;
    gpu.doorbell_size = 4 * 1024; // 4 KB typical

    // Check if this is an APU
    gpu.is_apu = matches!(device_id,
        device_ids::KAVERI_1 | device_ids::KAVERI_2 | device_ids::KABINI | device_ids::MULLINS |
        device_ids::CARRIZO | device_ids::STONEY |
        device_ids::RAVEN | device_ids::RAVEN2 | device_ids::RENOIR |
        device_ids::VAN_GOGH | device_ids::PHOENIX
    );

    crate::kprintln!(
        "amd_gpu: detected {} GPU (device {:04X}){}",
        family.name(),
        device_id,
        if gpu.is_apu { " [APU]" } else { "" }
    );
    crate::kprintln!(
        "amd_gpu: MMIO at {:#X}, VRAM at {:#X}",
        gpu.mmio_base,
        gpu.vram_base
    );

    // Initialize
    gpu.init()?;

    *AMD_GPU.lock() = Some(gpu);

    Ok(())
}

/// Get GPU info
pub fn get_info() -> Option<(u16, GpuFamily, u32, u32)> {
    let gpu = AMD_GPU.lock();
    gpu.as_ref().map(|g| {
        let (w, h) = g
            .current_mode
            .map(|m| (m.width, m.height))
            .unwrap_or((0, 0));
        (g.device_id, g.family, w, h)
    })
}

/// Check if AMD GPU is present
pub fn is_present() -> bool {
    AMD_GPU.lock().is_some()
}

/// Set display mode
pub fn set_mode(mode: AmdDisplayMode) -> Result<(), &'static str> {
    let mut gpu = AMD_GPU.lock();
    match gpu.as_mut() {
        Some(g) => g.set_mode(&mode),
        None => Err("No AMD GPU"),
    }
}

/// Get framebuffer address
pub fn framebuffer_address() -> Option<u64> {
    let gpu = AMD_GPU.lock();
    gpu.as_ref().map(|g| g.framebuffer_address())
}

/// Wait for vblank
pub fn wait_vblank() {
    let gpu = AMD_GPU.lock();
    if let Some(g) = gpu.as_ref() {
        g.wait_vblank();
    }
}

/// Probe PCI for AMD GPU
pub fn probe_pci() {
    use crate::drivers::pci::{scan, read_bar};

    for dev in scan() {
        if dev.id.vendor_id == AMD_VENDOR_ID {
            // Check device class (VGA controller = 0x03, subclass 0x00)
            // Or display controller (0x03, subclass 0x80)
            if (dev.class.class_code == 0x03 && dev.class.subclass == 0x00)
                || (dev.class.class_code == 0x03 && dev.class.subclass == 0x80)
            {
                let family = GpuFamily::from_device_id(dev.id.device_id);
                if family != GpuFamily::Unknown {
                    crate::kprintln!(
                        "amd_gpu: found AMD {} at {:02X}:{:02X}.{:X}",
                        family.name(),
                        dev.addr.bus,
                        dev.addr.device,
                        dev.addr.function
                    );

                    // Get BARs
                    let (bar0_addr, _) = read_bar(&dev, 0); // MMIO
                    let (bar2_addr, _) = read_bar(&dev, 2); // VRAM aperture
                    let (bar4_addr, _) = read_bar(&dev, 4); // Doorbell (newer GPUs)

                    if let Err(e) = init_from_pci(
                        dev.addr.bus,
                        dev.addr.device,
                        dev.addr.function,
                        dev.id.device_id,
                        bar0_addr,
                        bar2_addr,
                        bar4_addr,
                    ) {
                        crate::kprintln!("amd_gpu: init failed: {}", e);
                    }
                    break;
                }
            }
        }
    }
}

/// Initialize AMD GPU subsystem
pub fn init() {
    crate::kprintln!("amd_gpu: probing for AMD Radeon graphics");
    probe_pci();
}

// =============================================================================
// Sysfs Interface
// =============================================================================

/// Get GPU info string for sysfs
pub fn get_info_string() -> Option<String> {
    let gpu = AMD_GPU.lock();
    gpu.as_ref().map(|g| {
        alloc::format!(
            "AMD {} (Device ID: {:04X}){}",
            g.family.name(),
            g.device_id,
            if g.is_apu { " [APU]" } else { "" }
        )
    })
}

/// Get VRAM info string
pub fn get_vram_string() -> Option<String> {
    let gpu = AMD_GPU.lock();
    gpu.as_ref().map(|g| {
        alloc::format!("{} MB", g.detected_vram / (1024 * 1024))
    })
}

/// Get current mode string
pub fn get_mode_string() -> Option<String> {
    let gpu = AMD_GPU.lock();
    gpu.as_ref().and_then(|g| {
        g.current_mode.map(|m| {
            alloc::format!(
                "{}x{}@{}Hz",
                m.width,
                m.height,
                m.refresh_rate
            )
        })
    })
}
