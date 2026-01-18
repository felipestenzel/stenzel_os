//! NVIDIA GPU Driver
//!
//! Basic driver for NVIDIA GeForce GPUs.
//! Provides:
//! - PCI device detection and identification
//! - MMIO register access
//! - Basic mode setting
//! - Framebuffer management
//!
//! Supported GPU architectures:
//! - Kepler (GeForce 600/700 series)
//! - Maxwell (GeForce 900 series)
//! - Pascal (GeForce 10 series)
//! - Turing (GeForce 16/RTX 20 series)
//! - Ampere (GeForce RTX 30 series)
//! - Ada Lovelace (GeForce RTX 40 series)

#![allow(dead_code)]

use alloc::string::String;
use core::ptr;
use spin::Mutex;

/// NVIDIA vendor ID
pub const NVIDIA_VENDOR_ID: u16 = 0x10DE;

/// NVIDIA GPU device IDs by family
pub mod device_ids {
    // =========================================================================
    // Kepler (GeForce 600/700 series)
    // =========================================================================
    pub const GK104_GTX680: u16 = 0x1180;
    pub const GK104_GTX670: u16 = 0x1189;
    pub const GK104_GTX660TI: u16 = 0x1183;
    pub const GK106_GTX660: u16 = 0x11C0;
    pub const GK107_GTX650: u16 = 0x0FC6;
    pub const GK107_GT740: u16 = 0x0FC8;
    pub const GK110_GTX780TI: u16 = 0x100A;
    pub const GK110_GTX780: u16 = 0x1004;
    pub const GK110_GTX_TITAN: u16 = 0x1005;
    pub const GK110_TITAN_BLACK: u16 = 0x100C;
    pub const GK110_GTX770: u16 = 0x1184;
    pub const GK208_GT730: u16 = 0x1287;
    pub const GK208_GT710: u16 = 0x128B;

    // =========================================================================
    // Maxwell (GeForce 900 series)
    // =========================================================================
    pub const GM107_GTX750TI: u16 = 0x1380;
    pub const GM107_GTX750: u16 = 0x1381;
    pub const GM107_GTX745: u16 = 0x1382;
    pub const GM204_GTX980: u16 = 0x13C0;
    pub const GM204_GTX970: u16 = 0x13C2;
    pub const GM204_GTX980TI: u16 = 0x17C8;
    pub const GM206_GTX960: u16 = 0x1401;
    pub const GM206_GTX950: u16 = 0x1402;
    pub const GM200_TITAN_X: u16 = 0x17C2;
    pub const GM200_GTX980TI_ALT: u16 = 0x17C8;

    // =========================================================================
    // Pascal (GeForce 10 series)
    // =========================================================================
    pub const GP100_TITAN_XP: u16 = 0x1B00;
    pub const GP102_TITAN_X: u16 = 0x1B02;
    pub const GP102_GTX1080TI: u16 = 0x1B06;
    pub const GP104_GTX1080: u16 = 0x1B80;
    pub const GP104_GTX1070: u16 = 0x1B81;
    pub const GP104_GTX1070TI: u16 = 0x1B82;
    pub const GP106_GTX1060_6GB: u16 = 0x1C03;
    pub const GP106_GTX1060_3GB: u16 = 0x1C02;
    pub const GP107_GTX1050TI: u16 = 0x1C82;
    pub const GP107_GTX1050: u16 = 0x1C81;
    pub const GP108_GT1030: u16 = 0x1D01;
    pub const GP108_MX150: u16 = 0x1D10;

    // =========================================================================
    // Turing (GeForce 16/RTX 20 series)
    // =========================================================================
    pub const TU102_RTX2080TI: u16 = 0x1E04;
    pub const TU102_RTX_TITAN: u16 = 0x1E02;
    pub const TU104_RTX2080: u16 = 0x1E82;
    pub const TU104_RTX2080_SUPER: u16 = 0x1E81;
    pub const TU104_RTX2070_SUPER: u16 = 0x1E84;
    pub const TU106_RTX2070: u16 = 0x1F02;
    pub const TU106_RTX2060: u16 = 0x1F08;
    pub const TU106_RTX2060_SUPER: u16 = 0x1F06;
    pub const TU116_GTX1660TI: u16 = 0x2182;
    pub const TU116_GTX1660_SUPER: u16 = 0x21C4;
    pub const TU116_GTX1660: u16 = 0x2184;
    pub const TU117_GTX1650: u16 = 0x1F82;
    pub const TU117_GTX1650_SUPER: u16 = 0x2187;

    // =========================================================================
    // Ampere (GeForce RTX 30 series)
    // =========================================================================
    pub const GA102_RTX3090: u16 = 0x2204;
    pub const GA102_RTX3090TI: u16 = 0x2203;
    pub const GA102_RTX3080TI: u16 = 0x2208;
    pub const GA102_RTX3080: u16 = 0x2206;
    pub const GA102_RTX3080_12GB: u16 = 0x220A;
    pub const GA104_RTX3070TI: u16 = 0x2482;
    pub const GA104_RTX3070: u16 = 0x2484;
    pub const GA104_RTX3060TI: u16 = 0x2486;
    pub const GA106_RTX3060: u16 = 0x2503;
    pub const GA106_RTX3060_12GB: u16 = 0x2504;
    pub const GA107_RTX3050: u16 = 0x2507;

    // =========================================================================
    // Ada Lovelace (GeForce RTX 40 series)
    // =========================================================================
    pub const AD102_RTX4090: u16 = 0x2684;
    pub const AD102_RTX4090D: u16 = 0x2685;
    pub const AD103_RTX4080: u16 = 0x2704;
    pub const AD103_RTX4080_SUPER: u16 = 0x2702;
    pub const AD104_RTX4070TI: u16 = 0x2782;
    pub const AD104_RTX4070TI_SUPER: u16 = 0x2786;
    pub const AD104_RTX4070_SUPER: u16 = 0x2783;
    pub const AD104_RTX4070: u16 = 0x2786;
    pub const AD106_RTX4060TI: u16 = 0x2803;
    pub const AD107_RTX4060: u16 = 0x2882;
    pub const AD107_RTX4060_LAPTOP: u16 = 0x28A0;
}

/// GPU architecture/generation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuGeneration {
    /// Kepler (GK1xx) - GeForce 600/700
    Kepler,
    /// Maxwell (GM1xx/GM2xx) - GeForce 900
    Maxwell,
    /// Pascal (GP1xx) - GeForce 10
    Pascal,
    /// Turing (TU1xx) - GeForce 16/RTX 20
    Turing,
    /// Ampere (GA1xx) - RTX 30
    Ampere,
    /// Ada Lovelace (AD1xx) - RTX 40
    AdaLovelace,
    /// Unknown generation
    Unknown,
}

impl GpuGeneration {
    pub fn from_device_id(device_id: u16) -> Self {
        use device_ids::*;
        match device_id {
            // Kepler
            GK104_GTX680 | GK104_GTX670 | GK104_GTX660TI | GK106_GTX660
            | GK107_GTX650 | GK107_GT740 | GK110_GTX780TI | GK110_GTX780
            | GK110_GTX_TITAN | GK110_TITAN_BLACK | GK110_GTX770
            | GK208_GT730 | GK208_GT710 => GpuGeneration::Kepler,

            // Maxwell
            GM107_GTX750TI | GM107_GTX750 | GM107_GTX745 | GM204_GTX980 | GM204_GTX970
            | GM204_GTX980TI | GM206_GTX960 | GM206_GTX950 | GM200_TITAN_X
            | GM200_GTX980TI_ALT => GpuGeneration::Maxwell,

            // Pascal
            GP100_TITAN_XP | GP102_TITAN_X | GP102_GTX1080TI | GP104_GTX1080
            | GP104_GTX1070 | GP104_GTX1070TI | GP106_GTX1060_6GB | GP106_GTX1060_3GB
            | GP107_GTX1050TI | GP107_GTX1050 | GP108_GT1030 | GP108_MX150 => GpuGeneration::Pascal,

            // Turing
            TU102_RTX2080TI | TU102_RTX_TITAN | TU104_RTX2080 | TU104_RTX2080_SUPER
            | TU104_RTX2070_SUPER | TU106_RTX2070 | TU106_RTX2060 | TU106_RTX2060_SUPER
            | TU116_GTX1660TI | TU116_GTX1660_SUPER | TU116_GTX1660
            | TU117_GTX1650 | TU117_GTX1650_SUPER => GpuGeneration::Turing,

            // Ampere
            GA102_RTX3090 | GA102_RTX3090TI | GA102_RTX3080TI | GA102_RTX3080 | GA102_RTX3080_12GB
            | GA104_RTX3070TI | GA104_RTX3070 | GA104_RTX3060TI | GA106_RTX3060
            | GA106_RTX3060_12GB | GA107_RTX3050 => GpuGeneration::Ampere,

            // Ada Lovelace
            AD102_RTX4090 | AD102_RTX4090D | AD103_RTX4080 | AD103_RTX4080_SUPER
            | AD104_RTX4070TI | AD104_RTX4070TI_SUPER | AD104_RTX4070_SUPER | AD104_RTX4070
            | AD106_RTX4060TI | AD107_RTX4060 | AD107_RTX4060_LAPTOP => GpuGeneration::AdaLovelace,

            _ => {
                // Try to identify by device ID pattern
                let high_byte = (device_id >> 8) as u8;
                match high_byte {
                    0x0F..=0x12 => GpuGeneration::Kepler,
                    0x13..=0x17 => GpuGeneration::Maxwell,
                    0x1B..=0x1D => GpuGeneration::Pascal,
                    0x1E..=0x21 => GpuGeneration::Turing,
                    0x22..=0x25 => GpuGeneration::Ampere,
                    0x26..=0x2A => GpuGeneration::AdaLovelace,
                    _ => GpuGeneration::Unknown,
                }
            }
        }
    }

    /// Get generation name
    pub fn name(&self) -> &'static str {
        match self {
            GpuGeneration::Kepler => "Kepler",
            GpuGeneration::Maxwell => "Maxwell",
            GpuGeneration::Pascal => "Pascal",
            GpuGeneration::Turing => "Turing",
            GpuGeneration::Ampere => "Ampere",
            GpuGeneration::AdaLovelace => "Ada Lovelace",
            GpuGeneration::Unknown => "Unknown",
        }
    }

    /// Check if this generation supports ray tracing
    pub fn supports_raytracing(&self) -> bool {
        matches!(self,
            GpuGeneration::Turing | GpuGeneration::Ampere | GpuGeneration::AdaLovelace)
    }

    /// Check if this generation has tensor cores
    pub fn has_tensor_cores(&self) -> bool {
        matches!(self,
            GpuGeneration::Turing | GpuGeneration::Ampere | GpuGeneration::AdaLovelace)
    }

    /// Get minimum VRAM for this generation
    pub fn min_vram(&self) -> usize {
        match self {
            GpuGeneration::Kepler => 1024 * 1024 * 1024,      // 1 GB
            GpuGeneration::Maxwell => 2048 * 1024 * 1024,     // 2 GB
            GpuGeneration::Pascal => 2048 * 1024 * 1024,      // 2 GB
            GpuGeneration::Turing => 4096 * 1024 * 1024,      // 4 GB
            GpuGeneration::Ampere => 8192 * 1024 * 1024,      // 8 GB
            GpuGeneration::AdaLovelace => 8192 * 1024 * 1024, // 8 GB
            GpuGeneration::Unknown => 256 * 1024 * 1024,      // 256 MB
        }
    }
}

// =============================================================================
// Register Offsets
// =============================================================================

/// NVIDIA MMIO register offsets (NV_* registers from nouveau driver)
pub mod regs {
    // =========================================================================
    // PMC - Power Management Controller
    // =========================================================================
    pub const PMC_BOOT_0: u32 = 0x000000;     // Boot register - contains GPU ID
    pub const PMC_ENABLE: u32 = 0x000200;     // Engine enable bits
    pub const PMC_INTR: u32 = 0x000100;       // Interrupt status
    pub const PMC_INTR_EN: u32 = 0x000140;    // Interrupt enable

    // =========================================================================
    // PBUS - Bus Control
    // =========================================================================
    pub const PBUS_BAR0_WINDOW: u32 = 0x001700;  // BAR0 MMIO window
    pub const PBUS_BAR0_WINDOW_BASE: u32 = 0x001704;
    pub const PBUS_BAR1_BLOCK: u32 = 0x001714;   // BAR1 (VRAM aperture) config
    pub const PBUS_BAR2_BLOCK: u32 = 0x001718;   // BAR2/3 config

    // =========================================================================
    // PTIMER - Timer
    // =========================================================================
    pub const PTIMER_TIME_0: u32 = 0x009400;  // Time low
    pub const PTIMER_TIME_1: u32 = 0x009410;  // Time high
    pub const PTIMER_ALARM_0: u32 = 0x009420; // Alarm

    // =========================================================================
    // PFB - Frame Buffer Control
    // =========================================================================
    pub const PFB_CFG0: u32 = 0x100200;       // Framebuffer config
    pub const PFB_CFG1: u32 = 0x100204;
    pub const PFB_CSTATUS: u32 = 0x10020C;    // Framebuffer status

    // =========================================================================
    // PDISP - Display Controller
    // =========================================================================
    pub const PDISP_CAPS: u32 = 0x610000;     // Display capabilities
    pub const PDISP_HEAD0_CTRL: u32 = 0x610300; // Display head 0 control
    pub const PDISP_HEAD0_STATE: u32 = 0x610304;
    pub const PDISP_HEAD1_CTRL: u32 = 0x610380; // Display head 1 control

    // Core display registers (based on head offset)
    pub const PDISP_HEAD_OFFSET: u32 = 0x80;  // Offset between heads

    // Per-head registers (add HEAD_OFFSET * head_num)
    pub const HEAD_SYNC_START_TO_BLANK_END: u32 = 0x610AE4;
    pub const HEAD_BLANK_END_TO_HACTIVE: u32 = 0x610AE8;
    pub const HEAD_HACTIVE_TO_BLANK_START: u32 = 0x610AEC;
    pub const HEAD_BLANK_START_TO_SYNC_START: u32 = 0x610AF0;
    pub const HEAD_VSYNC_START_TO_BLANK_END: u32 = 0x610AF4;
    pub const HEAD_VBLANK_END_TO_VACTIVE: u32 = 0x610AF8;
    pub const HEAD_VACTIVE_TO_BLANK_START: u32 = 0x610AFC;
    pub const HEAD_BLANK_START_TO_VSYNC_START: u32 = 0x610B00;
    pub const HEAD_TOTAL: u32 = 0x610B08;
    pub const HEAD_SYNC_ACTIVE: u32 = 0x610B0C;
    pub const HEAD_INTERLACE: u32 = 0x610B10;
    pub const HEAD_CURS_POS: u32 = 0x610B20;

    // Surface (framebuffer) registers
    pub const HEAD_SURFACE_OFFSET: u32 = 0x610B60;
    pub const HEAD_SURFACE_SIZE: u32 = 0x610B64;
    pub const HEAD_SURFACE_PITCH: u32 = 0x610B68;
    pub const HEAD_SURFACE_FORMAT: u32 = 0x610B6C;

    // =========================================================================
    // PCRTC - CRT Controller (Legacy)
    // =========================================================================
    pub const PCRTC_INTR_0: u32 = 0x600100;
    pub const PCRTC_INTR_EN_0: u32 = 0x600140;
    pub const PCRTC_START: u32 = 0x600800;
    pub const PCRTC_CONFIG: u32 = 0x600804;

    // =========================================================================
    // PRAMDAC - RAMDAC
    // =========================================================================
    pub const PRAMDAC_PLL_COEFF_SELECT: u32 = 0x680500;
    pub const PRAMDAC_VPLL_COEFF: u32 = 0x680508;
    pub const PRAMDAC_MPLL_COEFF: u32 = 0x680504;
    pub const PRAMDAC_GENERAL_CTRL: u32 = 0x680600;

    // =========================================================================
    // GPU Info
    // =========================================================================
    pub const GPU_GPCCOUNT: u32 = 0x022430;   // Number of GPCs
    pub const GPU_TPCCOUNT: u32 = 0x022434;   // Number of TPCs
}

/// Surface format values
pub mod surface_format {
    pub const A8R8G8B8: u32 = 0xCF;   // 32-bit ARGB
    pub const X8R8G8B8: u32 = 0xE6;   // 32-bit XRGB
    pub const R5G6B5: u32 = 0xE8;     // 16-bit RGB565
    pub const A8: u32 = 0x1B;         // 8-bit alpha
}

// =============================================================================
// Display Mode
// =============================================================================

/// Display mode information
#[derive(Debug, Clone, Copy)]
pub struct NvidiaDisplayMode {
    pub width: u32,
    pub height: u32,
    pub bpp: u32,
    pub pixel_clock: u32,  // in kHz
    pub h_total: u32,
    pub h_sync_start: u32,
    pub h_sync_end: u32,
    pub h_blank_start: u32,
    pub h_blank_end: u32,
    pub v_total: u32,
    pub v_sync_start: u32,
    pub v_sync_end: u32,
    pub v_blank_start: u32,
    pub v_blank_end: u32,
    pub refresh_rate: u32,
}

impl NvidiaDisplayMode {
    /// Create a standard 1920x1080@60Hz mode
    pub fn mode_1080p() -> Self {
        Self {
            width: 1920,
            height: 1080,
            bpp: 32,
            pixel_clock: 148500,
            h_total: 2200,
            h_sync_start: 2008,
            h_sync_end: 2052,
            h_blank_start: 1920,
            h_blank_end: 2200,
            v_total: 1125,
            v_sync_start: 1084,
            v_sync_end: 1089,
            v_blank_start: 1080,
            v_blank_end: 1125,
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
            h_sync_start: 1390,
            h_sync_end: 1430,
            h_blank_start: 1280,
            h_blank_end: 1650,
            v_total: 750,
            v_sync_start: 725,
            v_sync_end: 730,
            v_blank_start: 720,
            v_blank_end: 750,
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
            h_sync_start: 2608,
            h_sync_end: 2640,
            h_blank_start: 2560,
            h_blank_end: 2720,
            v_total: 1481,
            v_sync_start: 1443,
            v_sync_end: 1448,
            v_blank_start: 1440,
            v_blank_end: 1481,
            refresh_rate: 60,
        }
    }

    /// Create a 3840x2160@60Hz (4K) mode
    pub fn mode_4k() -> Self {
        Self {
            width: 3840,
            height: 2160,
            bpp: 32,
            pixel_clock: 594000,
            h_total: 4400,
            h_sync_start: 4016,
            h_sync_end: 4104,
            h_blank_start: 3840,
            h_blank_end: 4400,
            v_total: 2250,
            v_sync_start: 2168,
            v_sync_end: 2178,
            v_blank_start: 2160,
            v_blank_end: 2250,
            refresh_rate: 60,
        }
    }

    /// Stride in bytes (NVIDIA requires 256-byte alignment)
    pub fn stride(&self) -> u32 {
        let raw_stride = self.width * (self.bpp / 8);
        (raw_stride + 255) & !255
    }

    /// Framebuffer size in bytes
    pub fn framebuffer_size(&self) -> usize {
        (self.stride() * self.height) as usize
    }
}

// =============================================================================
// NVIDIA GPU Driver
// =============================================================================

/// NVIDIA GPU driver state
pub struct NvidiaGpu {
    /// PCI device info
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    /// Device ID
    pub device_id: u16,
    /// GPU generation
    pub generation: GpuGeneration,
    /// MMIO base address (BAR0)
    pub mmio_base: u64,
    /// MMIO size
    pub mmio_size: usize,
    /// VRAM aperture base (BAR1)
    pub vram_base: u64,
    /// VRAM aperture size
    pub vram_size: usize,
    /// BAR2/3 address (for newer GPUs)
    pub bar2_base: u64,
    /// Current display mode
    pub current_mode: Option<NvidiaDisplayMode>,
    /// Framebuffer offset in VRAM
    pub framebuffer_offset: u64,
    /// Is initialized
    pub initialized: bool,
    /// Active display head (0 or 1)
    pub active_head: u8,
    /// Detected VRAM size
    pub detected_vram: usize,
    /// Boot register value (contains GPU info)
    pub boot0: u32,
}

impl NvidiaGpu {
    /// Create a new NVIDIA GPU driver instance
    pub const fn new() -> Self {
        Self {
            bus: 0,
            device: 0,
            function: 0,
            device_id: 0,
            generation: GpuGeneration::Unknown,
            mmio_base: 0,
            mmio_size: 0,
            vram_base: 0,
            vram_size: 0,
            bar2_base: 0,
            current_mode: None,
            framebuffer_offset: 0,
            initialized: false,
            active_head: 0,
            detected_vram: 0,
            boot0: 0,
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

    /// Modify 32-bit register (read-modify-write)
    pub unsafe fn modify32(&self, offset: u32, clear: u32, set: u32) {
        let value = self.read32(offset);
        self.write32(offset, (value & !clear) | set);
    }

    /// Initialize GPU
    pub fn init(&mut self) -> Result<(), &'static str> {
        if self.mmio_base == 0 {
            return Err("MMIO not mapped");
        }

        unsafe {
            // Read boot register
            self.boot0 = self.read32(regs::PMC_BOOT_0);

            // Detect VRAM size
            self.detect_vram();

            // Initialize display
            self.init_display()?;
        }

        self.initialized = true;
        Ok(())
    }

    /// Detect VRAM size from framebuffer registers
    unsafe fn detect_vram(&mut self) {
        // Try to read from PFB config registers
        let cfg0 = self.read32(regs::PFB_CFG0);

        // VRAM size calculation depends on generation
        // This is a simplified detection
        let vram_mb = match self.generation {
            GpuGeneration::Kepler | GpuGeneration::Maxwell => {
                // Read from BAR1 size or PMC
                let size_code = (cfg0 >> 8) & 0xF;
                match size_code {
                    0 => 256,
                    1 => 512,
                    2 => 1024,
                    3 => 2048,
                    4 => 4096,
                    5 => 8192,
                    _ => self.generation.min_vram() / (1024 * 1024),
                }
            }
            _ => {
                // Newer GPUs - use BAR1 size as approximation
                if self.vram_size > 0 {
                    self.vram_size / (1024 * 1024)
                } else {
                    self.generation.min_vram() / (1024 * 1024)
                }
            }
        };

        self.detected_vram = vram_mb * 1024 * 1024;
        crate::kprintln!("nvidia_gpu: VRAM size: {} MB", vram_mb);
    }

    /// Initialize display controller
    unsafe fn init_display(&mut self) -> Result<(), &'static str> {
        self.active_head = 0;

        // Read display capabilities
        let caps = self.read32(regs::PDISP_CAPS);
        crate::kprintln!("nvidia_gpu: display caps: {:#X}", caps);

        // Check if display is already active
        let head_state = self.read32(regs::PDISP_HEAD0_STATE);
        if head_state != 0 {
            crate::kprintln!("nvidia_gpu: display head 0 already active");
        }

        Ok(())
    }

    /// Set display mode
    pub fn set_mode(&mut self, mode: &NvidiaDisplayMode) -> Result<(), &'static str> {
        if !self.initialized {
            return Err("GPU not initialized");
        }

        unsafe {
            // Calculate head-specific register offset
            let head_offset = self.active_head as u32 * regs::PDISP_HEAD_OFFSET;

            // Set horizontal timing
            // Sync start to blank end
            let h_sync_to_blank = ((mode.h_blank_end - mode.h_sync_start) << 16)
                | (mode.h_sync_end - mode.h_sync_start);
            self.write32(regs::HEAD_SYNC_START_TO_BLANK_END + head_offset, h_sync_to_blank);

            // Blank end to active
            let h_blank_to_active = mode.h_blank_end - mode.h_sync_start;
            self.write32(regs::HEAD_BLANK_END_TO_HACTIVE + head_offset, h_blank_to_active);

            // Active to blank start
            self.write32(regs::HEAD_HACTIVE_TO_BLANK_START + head_offset, mode.width);

            // Blank start to sync start
            let h_blank_to_sync = mode.h_sync_start - mode.h_blank_start;
            self.write32(regs::HEAD_BLANK_START_TO_SYNC_START + head_offset, h_blank_to_sync);

            // Set vertical timing (similar structure)
            let v_sync_to_blank = ((mode.v_blank_end - mode.v_sync_start) << 16)
                | (mode.v_sync_end - mode.v_sync_start);
            self.write32(regs::HEAD_VSYNC_START_TO_BLANK_END + head_offset, v_sync_to_blank);

            let v_blank_to_active = mode.v_blank_end - mode.v_sync_start;
            self.write32(regs::HEAD_VBLANK_END_TO_VACTIVE + head_offset, v_blank_to_active);

            self.write32(regs::HEAD_VACTIVE_TO_BLANK_START + head_offset, mode.height);

            let v_blank_to_sync = mode.v_sync_start - mode.v_blank_start;
            self.write32(regs::HEAD_BLANK_START_TO_VSYNC_START + head_offset, v_blank_to_sync);

            // Set total size
            let total = (mode.v_total << 16) | mode.h_total;
            self.write32(regs::HEAD_TOTAL + head_offset, total);

            // Set sync active widths
            let sync_active = ((mode.v_sync_end - mode.v_sync_start) << 16)
                | (mode.h_sync_end - mode.h_sync_start);
            self.write32(regs::HEAD_SYNC_ACTIVE + head_offset, sync_active);

            // Set surface (framebuffer)
            self.write32(regs::HEAD_SURFACE_OFFSET + head_offset, self.framebuffer_offset as u32);

            let surface_size = (mode.height << 16) | mode.width;
            self.write32(regs::HEAD_SURFACE_SIZE + head_offset, surface_size);

            self.write32(regs::HEAD_SURFACE_PITCH + head_offset, mode.stride());

            // Set format (32-bit ARGB)
            self.write32(regs::HEAD_SURFACE_FORMAT + head_offset, surface_format::A8R8G8B8);

            // No interlace
            self.write32(regs::HEAD_INTERLACE + head_offset, 0);
        }

        self.current_mode = Some(*mode);

        crate::kprintln!(
            "nvidia_gpu: mode set to {}x{}@{}Hz",
            mode.width,
            mode.height,
            mode.refresh_rate
        );

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
            // Wait for VBLANK interrupt
            let intr_reg = regs::PCRTC_INTR_0;

            // Clear any pending interrupt
            self.write32(intr_reg, 1);

            // Wait for vblank
            for _ in 0..100000 {
                let status = self.read32(intr_reg);
                if status & 1 != 0 {
                    self.write32(intr_reg, 1); // Acknowledge
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
            let head_offset = self.active_head as u32 * regs::PDISP_HEAD_OFFSET;
            let pos = (y << 16) | x;
            self.write32(regs::HEAD_CURS_POS + head_offset, pos);
        }
    }

    /// Disable GPU
    pub fn disable(&mut self) {
        if !self.initialized {
            return;
        }

        // Disable display engines would go here
        self.initialized = false;
    }

    /// Get GPU information string
    pub fn get_info_string(&self) -> String {
        let mut info = alloc::format!(
            "NVIDIA {} GPU (Device ID: {:04X})",
            self.generation.name(),
            self.device_id
        );

        if self.generation.supports_raytracing() {
            info.push_str(" [RTX]");
        }

        if self.generation.has_tensor_cores() {
            info.push_str(" [Tensor]");
        }

        info
    }
}

// =============================================================================
// Global State
// =============================================================================

/// Global GPU instance
static NVIDIA_GPU: Mutex<Option<NvidiaGpu>> = Mutex::new(None);

/// Check if device ID is an NVIDIA GPU
pub fn is_nvidia_gpu(vendor_id: u16, device_id: u16) -> bool {
    if vendor_id != NVIDIA_VENDOR_ID {
        return false;
    }
    GpuGeneration::from_device_id(device_id) != GpuGeneration::Unknown
}

/// Initialize NVIDIA GPU from PCI
pub fn init_from_pci(
    bus: u8,
    device: u8,
    function: u8,
    device_id: u16,
    bar0: u64,
    bar1: u64,
    bar2: u64,
) -> Result<(), &'static str> {
    let generation = GpuGeneration::from_device_id(device_id);

    if generation == GpuGeneration::Unknown {
        return Err("Unknown NVIDIA GPU");
    }

    let mut gpu = NvidiaGpu::new();
    gpu.bus = bus;
    gpu.device = device;
    gpu.function = function;
    gpu.device_id = device_id;
    gpu.generation = generation;
    gpu.mmio_base = bar0;
    gpu.mmio_size = 16 * 1024 * 1024;  // 16 MB typical for MMIO
    gpu.vram_base = bar1;
    gpu.vram_size = generation.min_vram();
    gpu.bar2_base = bar2;

    crate::kprintln!(
        "nvidia_gpu: detected {} GPU (device {:04X})",
        generation.name(),
        device_id
    );
    crate::kprintln!(
        "nvidia_gpu: MMIO at {:#X}, VRAM at {:#X}",
        gpu.mmio_base,
        gpu.vram_base
    );

    if generation.supports_raytracing() {
        crate::kprintln!("nvidia_gpu: RTX features available");
    }

    // Initialize
    gpu.init()?;

    *NVIDIA_GPU.lock() = Some(gpu);

    Ok(())
}

/// Get GPU info
pub fn get_info() -> Option<(u16, GpuGeneration, u32, u32)> {
    let gpu = NVIDIA_GPU.lock();
    gpu.as_ref().map(|g| {
        let (w, h) = g
            .current_mode
            .map(|m| (m.width, m.height))
            .unwrap_or((0, 0));
        (g.device_id, g.generation, w, h)
    })
}

/// Check if NVIDIA GPU is present
pub fn is_present() -> bool {
    NVIDIA_GPU.lock().is_some()
}

/// Set display mode
pub fn set_mode(mode: NvidiaDisplayMode) -> Result<(), &'static str> {
    let mut gpu = NVIDIA_GPU.lock();
    match gpu.as_mut() {
        Some(g) => g.set_mode(&mode),
        None => Err("No NVIDIA GPU"),
    }
}

/// Get framebuffer address
pub fn framebuffer_address() -> Option<u64> {
    let gpu = NVIDIA_GPU.lock();
    gpu.as_ref().map(|g| g.framebuffer_address())
}

/// Wait for vblank
pub fn wait_vblank() {
    let gpu = NVIDIA_GPU.lock();
    if let Some(g) = gpu.as_ref() {
        g.wait_vblank();
    }
}

/// Probe PCI for NVIDIA GPU
pub fn probe_pci() {
    use crate::drivers::pci::{scan, read_bar};

    for dev in scan() {
        if dev.id.vendor_id == NVIDIA_VENDOR_ID {
            // Check device class (VGA controller = 0x03, subclass 0x00)
            // Or 3D controller (0x03, subclass 0x02)
            if (dev.class.class_code == 0x03 && dev.class.subclass == 0x00)
                || (dev.class.class_code == 0x03 && dev.class.subclass == 0x02)
            {
                let generation = GpuGeneration::from_device_id(dev.id.device_id);
                if generation != GpuGeneration::Unknown {
                    crate::kprintln!(
                        "nvidia_gpu: found NVIDIA {} at {:02X}:{:02X}.{:X}",
                        generation.name(),
                        dev.addr.bus,
                        dev.addr.device,
                        dev.addr.function
                    );

                    // Get BARs
                    let (bar0_addr, _) = read_bar(&dev, 0); // MMIO
                    let (bar1_addr, _) = read_bar(&dev, 1); // VRAM aperture
                    let (bar2_addr, _) = read_bar(&dev, 2); // Additional

                    if let Err(e) = init_from_pci(
                        dev.addr.bus,
                        dev.addr.device,
                        dev.addr.function,
                        dev.id.device_id,
                        bar0_addr,
                        bar1_addr,
                        bar2_addr,
                    ) {
                        crate::kprintln!("nvidia_gpu: init failed: {}", e);
                    }
                    break;
                }
            }
        }
    }
}

/// Initialize NVIDIA GPU subsystem
pub fn init() {
    crate::kprintln!("nvidia_gpu: probing for NVIDIA GeForce graphics");
    probe_pci();
}

// =============================================================================
// Sysfs Interface
// =============================================================================

/// Get GPU info string for sysfs
pub fn get_info_string() -> Option<String> {
    let gpu = NVIDIA_GPU.lock();
    gpu.as_ref().map(|g| g.get_info_string())
}

/// Get VRAM info string
pub fn get_vram_string() -> Option<String> {
    let gpu = NVIDIA_GPU.lock();
    gpu.as_ref().map(|g| {
        alloc::format!("{} MB", g.detected_vram / (1024 * 1024))
    })
}

/// Get current mode string
pub fn get_mode_string() -> Option<String> {
    let gpu = NVIDIA_GPU.lock();
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
