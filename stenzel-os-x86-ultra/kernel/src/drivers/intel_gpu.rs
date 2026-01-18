//! Intel Integrated Graphics Driver
//!
//! Basic driver for Intel integrated GPUs (Gen4 through Xe).
//! Provides:
//! - PCI device detection and identification
//! - MMIO register access
//! - GTT (Graphics Translation Table) management
//! - Display pipe configuration
//! - Basic mode setting
//! - Framebuffer management
//!
//! Supported GPU generations:
//! - Gen4: 965GM, G45, etc.
//! - Gen5: Ironlake
//! - Gen6: Sandy Bridge
//! - Gen7: Ivy Bridge, Haswell
//! - Gen8: Broadwell
//! - Gen9: Skylake, Kaby Lake
//! - Gen11: Ice Lake
//! - Gen12/Xe: Tiger Lake, Alder Lake

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use core::ptr;
use spin::Mutex;

/// Intel GPU vendor ID
pub const INTEL_VENDOR_ID: u16 = 0x8086;

/// Intel GPU device IDs by generation
pub mod device_ids {
    // Gen4 (965)
    pub const G965_1: u16 = 0x29A2;
    pub const G965_2: u16 = 0x2992;
    pub const GM965_1: u16 = 0x2A02;
    pub const GM965_2: u16 = 0x2A12;
    pub const G45: u16 = 0x2E22;
    pub const GM45: u16 = 0x2A42;

    // Gen5 (Ironlake)
    pub const IRONLAKE_D: u16 = 0x0042;
    pub const IRONLAKE_M: u16 = 0x0046;

    // Gen6 (Sandy Bridge)
    pub const SNB_GT1_D: u16 = 0x0102;
    pub const SNB_GT2_D: u16 = 0x0112;
    pub const SNB_GT1_M: u16 = 0x0106;
    pub const SNB_GT2_M: u16 = 0x0116;
    pub const SNB_GT2_M_1: u16 = 0x0126;

    // Gen7 (Ivy Bridge)
    pub const IVB_GT1_D: u16 = 0x0152;
    pub const IVB_GT2_D: u16 = 0x0162;
    pub const IVB_GT1_M: u16 = 0x0156;
    pub const IVB_GT2_M: u16 = 0x0166;

    // Gen7.5 (Haswell)
    pub const HSW_GT1_D: u16 = 0x0402;
    pub const HSW_GT2_D: u16 = 0x0412;
    pub const HSW_GT3_D: u16 = 0x0422;
    pub const HSW_GT1_M: u16 = 0x0406;
    pub const HSW_GT2_M: u16 = 0x0416;
    pub const HSW_GT3_M: u16 = 0x0426;
    pub const HSW_ULT_GT1: u16 = 0x0A06;
    pub const HSW_ULT_GT2: u16 = 0x0A16;
    pub const HSW_ULT_GT3: u16 = 0x0A26;

    // Gen8 (Broadwell)
    pub const BDW_GT1_D: u16 = 0x1602;
    pub const BDW_GT2_D: u16 = 0x1612;
    pub const BDW_GT3_D: u16 = 0x1622;
    pub const BDW_GT1_M: u16 = 0x1606;
    pub const BDW_GT2_M: u16 = 0x1616;
    pub const BDW_GT3_M: u16 = 0x1626;

    // Gen9 (Skylake)
    pub const SKL_GT1_D: u16 = 0x1902;
    pub const SKL_GT2_D: u16 = 0x1912;
    pub const SKL_GT3_D: u16 = 0x1932;
    pub const SKL_GT1_M: u16 = 0x1906;
    pub const SKL_GT2_M: u16 = 0x1916;
    pub const SKL_GT3_M: u16 = 0x1926;
    pub const SKL_GT4: u16 = 0x193B;

    // Gen9.5 (Kaby Lake / Coffee Lake)
    pub const KBL_GT1_D: u16 = 0x5902;
    pub const KBL_GT2_D: u16 = 0x5912;
    pub const KBL_GT3_D: u16 = 0x5932;
    pub const KBL_GT1_M: u16 = 0x5906;
    pub const KBL_GT2_M: u16 = 0x5916;
    pub const KBL_GT3_M: u16 = 0x5926;
    pub const CFL_GT2: u16 = 0x3E92;
    pub const CFL_GT2_1: u16 = 0x3E91;

    // Gen11 (Ice Lake)
    pub const ICL_GT0_5: u16 = 0x8A50;
    pub const ICL_GT1: u16 = 0x8A51;
    pub const ICL_GT1_5: u16 = 0x8A52;
    pub const ICL_GT2: u16 = 0x8A53;

    // Gen12 / Xe (Tiger Lake)
    pub const TGL_GT1: u16 = 0x9A49;
    pub const TGL_GT2: u16 = 0x9A40;
    pub const TGL_GT2_1: u16 = 0x9A60;

    // Gen12 / Xe (Alder Lake)
    pub const ADL_GT1: u16 = 0x4680;
    pub const ADL_GT2: u16 = 0x4682;
    pub const ADL_N: u16 = 0x46D0;
}

/// GPU generation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuGeneration {
    Gen4,
    Gen5,
    Gen6,
    Gen7,
    Gen7_5,
    Gen8,
    Gen9,
    Gen9_5,
    Gen11,
    Gen12,
    Unknown,
}

impl GpuGeneration {
    pub fn from_device_id(device_id: u16) -> Self {
        use device_ids::*;
        match device_id {
            G965_1 | G965_2 | GM965_1 | GM965_2 | G45 | GM45 => GpuGeneration::Gen4,
            IRONLAKE_D | IRONLAKE_M => GpuGeneration::Gen5,
            SNB_GT1_D | SNB_GT2_D | SNB_GT1_M | SNB_GT2_M | SNB_GT2_M_1 => GpuGeneration::Gen6,
            IVB_GT1_D | IVB_GT2_D | IVB_GT1_M | IVB_GT2_M => GpuGeneration::Gen7,
            HSW_GT1_D | HSW_GT2_D | HSW_GT3_D | HSW_GT1_M | HSW_GT2_M | HSW_GT3_M
            | HSW_ULT_GT1 | HSW_ULT_GT2 | HSW_ULT_GT3 => GpuGeneration::Gen7_5,
            BDW_GT1_D | BDW_GT2_D | BDW_GT3_D | BDW_GT1_M | BDW_GT2_M | BDW_GT3_M => {
                GpuGeneration::Gen8
            }
            SKL_GT1_D | SKL_GT2_D | SKL_GT3_D | SKL_GT1_M | SKL_GT2_M | SKL_GT3_M | SKL_GT4 => {
                GpuGeneration::Gen9
            }
            KBL_GT1_D | KBL_GT2_D | KBL_GT3_D | KBL_GT1_M | KBL_GT2_M | KBL_GT3_M
            | CFL_GT2 | CFL_GT2_1 => GpuGeneration::Gen9_5,
            ICL_GT0_5 | ICL_GT1 | ICL_GT1_5 | ICL_GT2 => GpuGeneration::Gen11,
            TGL_GT1 | TGL_GT2 | TGL_GT2_1 | ADL_GT1 | ADL_GT2 | ADL_N => GpuGeneration::Gen12,
            _ => GpuGeneration::Unknown,
        }
    }

    /// Get minimum graphics memory size for generation
    pub fn min_graphics_memory(&self) -> usize {
        match self {
            GpuGeneration::Gen4 => 256 * 1024 * 1024,  // 256 MB
            GpuGeneration::Gen5 => 256 * 1024 * 1024,
            GpuGeneration::Gen6 => 512 * 1024 * 1024,  // 512 MB
            GpuGeneration::Gen7 => 512 * 1024 * 1024,
            GpuGeneration::Gen7_5 => 1024 * 1024 * 1024, // 1 GB
            GpuGeneration::Gen8 => 1024 * 1024 * 1024,
            GpuGeneration::Gen9 | GpuGeneration::Gen9_5 => 2048 * 1024 * 1024, // 2 GB
            GpuGeneration::Gen11 | GpuGeneration::Gen12 => 2048 * 1024 * 1024,
            GpuGeneration::Unknown => 256 * 1024 * 1024,
        }
    }
}

/// Register offsets for Intel GPU MMIO
pub mod regs {
    // Graphics Memory Interface
    pub const GMCH_CTRL: u32 = 0x50;
    pub const BSM: u32 = 0x5C; // Base of Stolen Memory

    // GTT (Graphics Translation Table)
    pub const PGTBL_CTL: u32 = 0x2020;
    pub const PGTBL_ER: u32 = 0x2024;

    // Display
    pub const DSPCNTR_A: u32 = 0x70180; // Display Control A
    pub const DSPADDR_A: u32 = 0x70184; // Display Base Address A
    pub const DSPSTRIDE_A: u32 = 0x70188; // Display Stride A
    pub const DSPPOS_A: u32 = 0x7018C;  // Display Position A
    pub const DSPSIZE_A: u32 = 0x70190; // Display Size A
    pub const DSPSURF_A: u32 = 0x7019C; // Display Surface A (Gen4+)
    pub const DSPTILEOFF_A: u32 = 0x701A4; // Display Tile Offset A

    pub const DSPCNTR_B: u32 = 0x71180; // Display Control B
    pub const DSPADDR_B: u32 = 0x71184;
    pub const DSPSTRIDE_B: u32 = 0x71188;
    pub const DSPPOS_B: u32 = 0x7118C;
    pub const DSPSIZE_B: u32 = 0x71190;
    pub const DSPSURF_B: u32 = 0x7119C;

    // Pipe configuration
    pub const PIPEA_CONF: u32 = 0x70008;
    pub const PIPEB_CONF: u32 = 0x71008;
    pub const PIPEAFRAMEHIGH: u32 = 0x70040;
    pub const PIPEAFRAMEPIXEL: u32 = 0x70044;
    pub const PIPEASTAT: u32 = 0x70024;

    // Horizontal timings
    pub const HTOTAL_A: u32 = 0x60000;
    pub const HBLANK_A: u32 = 0x60004;
    pub const HSYNC_A: u32 = 0x60008;
    pub const HTOTAL_B: u32 = 0x61000;
    pub const HBLANK_B: u32 = 0x61004;
    pub const HSYNC_B: u32 = 0x61008;

    // Vertical timings
    pub const VTOTAL_A: u32 = 0x6000C;
    pub const VBLANK_A: u32 = 0x60010;
    pub const VSYNC_A: u32 = 0x60014;
    pub const VTOTAL_B: u32 = 0x6100C;
    pub const VBLANK_B: u32 = 0x61010;
    pub const VSYNC_B: u32 = 0x61014;

    // Source image size
    pub const PIPEASRC: u32 = 0x6001C;
    pub const PIPEBSRC: u32 = 0x6101C;

    // Panel fitting
    pub const PFIT_CONTROL: u32 = 0x61230;
    pub const PFIT_PGM_RATIOS: u32 = 0x61234;

    // Display Port
    pub const DP_A: u32 = 0x64000;
    pub const DP_B: u32 = 0x64100;
    pub const DP_C: u32 = 0x64200;
    pub const DP_D: u32 = 0x64300;

    // HDMI/DVI
    pub const HDMI_B: u32 = 0x61140;
    pub const HDMI_C: u32 = 0x61160;
    pub const HDMI_D: u32 = 0x6116C;

    // VGA
    pub const ADPA: u32 = 0x61100;

    // LVDS (Laptop panel)
    pub const LVDS: u32 = 0x61180;

    // GPU power management
    pub const PWRCTX: u32 = 0x2088;
    pub const PWRCTXA: u32 = 0x208C;
    pub const PWRGATE: u32 = 0xA090;

    // Fence registers (for tiling)
    pub const FENCE_REG_BASE: u32 = 0x2000;
    pub const FENCE_COUNT: u32 = 16;

    // Hardware status
    pub const INSTPM: u32 = 0x20C0;
    pub const HWSTAM: u32 = 0x2098;
    pub const IER: u32 = 0x20A0;  // Interrupt Enable
    pub const IIR: u32 = 0x20A4;  // Interrupt Identity
    pub const IMR: u32 = 0x20A8;  // Interrupt Mask
    pub const ISR: u32 = 0x20AC;  // Interrupt Status

    // Clock gating
    pub const DSPCLK_GATE_D: u32 = 0x6200;

    // Ring buffer (command submission)
    pub const RCS_RING_BUFFER_TAIL: u32 = 0x2030;
    pub const RCS_RING_BUFFER_HEAD: u32 = 0x2034;
    pub const RCS_RING_BUFFER_START: u32 = 0x2038;
    pub const RCS_RING_BUFFER_CTL: u32 = 0x203C;
}

/// Display control register bits
pub mod dspcntr {
    pub const ENABLE: u32 = 1 << 31;
    pub const GAMMA_ENABLE: u32 = 1 << 30;
    pub const FORMAT_MASK: u32 = 0xF << 26;
    pub const FORMAT_8BPP: u32 = 2 << 26;
    pub const FORMAT_RGB565: u32 = 5 << 26;
    pub const FORMAT_XRGB8888: u32 = 6 << 26;
    pub const FORMAT_XBGR8888: u32 = 0xE << 26;
    pub const FORMAT_ARGB8888: u32 = 7 << 26;
    pub const TILED: u32 = 1 << 10;
    pub const TILED_X: u32 = 1 << 10;
    pub const TILED_Y: u32 = 1 << 9 | 1 << 10;
    pub const ROTATE_180: u32 = 1 << 15;
}

/// Pipe configuration bits
pub mod pipe {
    pub const ENABLE: u32 = 1 << 31;
    pub const STATE_ENABLED: u32 = 1 << 30;
    pub const INTERLACE_MASK: u32 = 7 << 21;
    pub const INTERLACE_PROGRESSIVE: u32 = 0 << 21;
}

/// GTT entry format
pub mod gtt {
    pub const PRESENT: u64 = 1 << 0;
    pub const CACHE_LLC: u64 = 1 << 1;
    pub const CACHE_SNOOPED: u64 = 1 << 2;
    pub const ADDR_MASK: u64 = 0xFFFFF000;
}

/// Display mode information
#[derive(Debug, Clone, Copy)]
pub struct IntelDisplayMode {
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
    pub refresh_rate: u32, // in Hz
}

impl IntelDisplayMode {
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

    /// Create a standard 1024x768@60Hz mode
    pub fn mode_xga() -> Self {
        Self {
            width: 1024,
            height: 768,
            bpp: 32,
            pixel_clock: 65000,
            h_total: 1344,
            h_blank_start: 1024,
            h_blank_end: 1344,
            h_sync_start: 1048,
            h_sync_end: 1184,
            v_total: 806,
            v_blank_start: 768,
            v_blank_end: 806,
            v_sync_start: 771,
            v_sync_end: 777,
            refresh_rate: 60,
        }
    }

    /// Stride in bytes
    pub fn stride(&self) -> u32 {
        self.width * (self.bpp / 8)
    }

    /// Framebuffer size in bytes
    pub fn framebuffer_size(&self) -> usize {
        (self.stride() * self.height) as usize
    }
}

/// Intel GPU driver state
pub struct IntelGpu {
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
    /// GTT base address (BAR2 or mapped in BAR0)
    pub gtt_base: u64,
    /// GTT size in entries
    pub gtt_size: usize,
    /// Stolen memory base (for framebuffer)
    pub stolen_base: u64,
    /// Stolen memory size
    pub stolen_size: usize,
    /// Current display mode
    pub current_mode: Option<IntelDisplayMode>,
    /// Framebuffer address in GTT
    pub framebuffer_offset: u32,
    /// Is initialized
    pub initialized: bool,
    /// Display pipe in use (0 = A, 1 = B)
    pub active_pipe: u8,
}

impl IntelGpu {
    /// Create a new Intel GPU driver instance
    pub const fn new() -> Self {
        Self {
            bus: 0,
            device: 0,
            function: 0,
            device_id: 0,
            generation: GpuGeneration::Unknown,
            mmio_base: 0,
            mmio_size: 0,
            gtt_base: 0,
            gtt_size: 0,
            stolen_base: 0,
            stolen_size: 0,
            current_mode: None,
            framebuffer_offset: 0,
            initialized: false,
            active_pipe: 0,
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

    /// Read-modify-write a register
    pub unsafe fn modify32<F>(&self, offset: u32, f: F)
    where
        F: FnOnce(u32) -> u32,
    {
        let value = self.read32(offset);
        self.write32(offset, f(value));
    }

    /// Write GTT entry
    pub unsafe fn write_gtt_entry(&self, index: usize, phys_addr: u64, flags: u64) {
        let gtt_addr = (self.gtt_base + (index * 8) as u64) as *mut u64;
        let entry = (phys_addr & gtt::ADDR_MASK) | flags;
        ptr::write_volatile(gtt_addr, entry);
    }

    /// Initialize GPU
    pub fn init(&mut self) -> Result<(), &'static str> {
        if self.mmio_base == 0 {
            return Err("MMIO not mapped");
        }

        unsafe {
            // Disable interrupts
            self.write32(regs::IER, 0);
            self.write32(regs::IMR, 0xFFFFFFFF);

            // Read current state
            let _gmch_ctrl = self.read32(regs::GMCH_CTRL);

            // Initialize GTT if needed
            self.init_gtt()?;

            // Setup display
            self.init_display()?;
        }

        self.initialized = true;
        Ok(())
    }

    /// Initialize GTT
    unsafe fn init_gtt(&mut self) -> Result<(), &'static str> {
        // GTT is typically pre-configured by firmware
        // We just need to map our framebuffer into it

        // Calculate GTT size based on stolen memory
        let gtt_pages = self.stolen_size / 4096;
        self.gtt_size = gtt_pages;

        crate::kprintln!(
            "intel_gpu: GTT size: {} entries ({} MB addressable)",
            self.gtt_size,
            (self.gtt_size * 4096) / (1024 * 1024)
        );

        Ok(())
    }

    /// Initialize display
    unsafe fn init_display(&mut self) -> Result<(), &'static str> {
        // Default to pipe A
        self.active_pipe = 0;

        // Read current pipe state
        let pipe_conf = self.read32(regs::PIPEA_CONF);

        if pipe_conf & pipe::ENABLE != 0 {
            // Pipe is already enabled, read current mode from hardware
            let htotal = self.read32(regs::HTOTAL_A);
            let vtotal = self.read32(regs::VTOTAL_A);

            let h_active = (htotal & 0xFFF) + 1;
            let v_active = (vtotal & 0xFFF) + 1;

            crate::kprintln!(
                "intel_gpu: display already enabled: {}x{}",
                h_active,
                v_active
            );

            // TODO: Parse full mode from registers
        }

        Ok(())
    }

    /// Set display mode
    pub fn set_mode(&mut self, mode: &IntelDisplayMode) -> Result<(), &'static str> {
        if !self.initialized {
            return Err("GPU not initialized");
        }

        unsafe {
            // Disable display plane
            let dspcntr_reg = if self.active_pipe == 0 {
                regs::DSPCNTR_A
            } else {
                regs::DSPCNTR_B
            };
            let pipe_conf_reg = if self.active_pipe == 0 {
                regs::PIPEA_CONF
            } else {
                regs::PIPEB_CONF
            };

            // Disable pipe and plane
            self.write32(dspcntr_reg, 0);
            self.write32(pipe_conf_reg, 0);

            // Wait for pipe to disable
            for _ in 0..1000 {
                let conf = self.read32(pipe_conf_reg);
                if conf & pipe::STATE_ENABLED == 0 {
                    break;
                }
                core::hint::spin_loop();
            }

            // Set timings (for pipe A)
            if self.active_pipe == 0 {
                // Horizontal
                self.write32(
                    regs::HTOTAL_A,
                    ((mode.h_total - 1) << 16) | (mode.width - 1),
                );
                self.write32(
                    regs::HBLANK_A,
                    ((mode.h_blank_end - 1) << 16) | (mode.h_blank_start - 1),
                );
                self.write32(
                    regs::HSYNC_A,
                    ((mode.h_sync_end - 1) << 16) | (mode.h_sync_start - 1),
                );

                // Vertical
                self.write32(
                    regs::VTOTAL_A,
                    ((mode.v_total - 1) << 16) | (mode.height - 1),
                );
                self.write32(
                    regs::VBLANK_A,
                    ((mode.v_blank_end - 1) << 16) | (mode.v_blank_start - 1),
                );
                self.write32(
                    regs::VSYNC_A,
                    ((mode.v_sync_end - 1) << 16) | (mode.v_sync_start - 1),
                );

                // Source size
                self.write32(
                    regs::PIPEASRC,
                    ((mode.width - 1) << 16) | (mode.height - 1),
                );
            }

            // Setup display plane
            let format = match mode.bpp {
                8 => dspcntr::FORMAT_8BPP,
                16 => dspcntr::FORMAT_RGB565,
                24 | 32 => dspcntr::FORMAT_XRGB8888,
                _ => return Err("Unsupported BPP"),
            };

            // Set stride
            let stride_reg = if self.active_pipe == 0 {
                regs::DSPSTRIDE_A
            } else {
                regs::DSPSTRIDE_B
            };
            self.write32(stride_reg, mode.stride());

            // Set position (0, 0)
            let pos_reg = if self.active_pipe == 0 {
                regs::DSPPOS_A
            } else {
                regs::DSPPOS_B
            };
            self.write32(pos_reg, 0);

            // Set size
            let size_reg = if self.active_pipe == 0 {
                regs::DSPSIZE_A
            } else {
                regs::DSPSIZE_B
            };
            self.write32(size_reg, ((mode.height - 1) << 16) | (mode.width - 1));

            // Set surface base (using stolen memory)
            let surf_reg = if self.active_pipe == 0 {
                regs::DSPSURF_A
            } else {
                regs::DSPSURF_B
            };
            self.write32(surf_reg, self.framebuffer_offset);

            // Enable pipe
            self.write32(pipe_conf_reg, pipe::ENABLE);

            // Wait for pipe to enable
            for _ in 0..1000 {
                let conf = self.read32(pipe_conf_reg);
                if conf & pipe::STATE_ENABLED != 0 {
                    break;
                }
                core::hint::spin_loop();
            }

            // Enable display plane
            self.write32(dspcntr_reg, dspcntr::ENABLE | format);
        }

        self.current_mode = Some(*mode);

        crate::kprintln!(
            "intel_gpu: mode set to {}x{}@{}Hz",
            mode.width,
            mode.height,
            mode.refresh_rate
        );

        Ok(())
    }

    /// Get framebuffer address for CPU access
    pub fn framebuffer_address(&self) -> u64 {
        self.stolen_base + self.framebuffer_offset as u64
    }

    /// Get framebuffer size
    pub fn framebuffer_size(&self) -> usize {
        self.current_mode.map_or(0, |m| m.framebuffer_size())
    }

    /// Wait for vblank
    pub fn wait_vblank(&self) {
        if !self.initialized {
            return;
        }

        unsafe {
            let stat_reg = if self.active_pipe == 0 {
                regs::PIPEASTAT
            } else {
                regs::PIPEASTAT + 0x1000
            };

            // Clear vblank bit
            self.write32(stat_reg, 1 << 1);

            // Wait for vblank
            for _ in 0..100000 {
                let stat = self.read32(stat_reg);
                if stat & (1 << 1) != 0 {
                    break;
                }
                core::hint::spin_loop();
            }
        }
    }

    /// Disable GPU
    pub fn disable(&mut self) {
        if !self.initialized {
            return;
        }

        unsafe {
            // Disable display plane
            let dspcntr_reg = if self.active_pipe == 0 {
                regs::DSPCNTR_A
            } else {
                regs::DSPCNTR_B
            };
            self.write32(dspcntr_reg, 0);

            // Disable pipe
            let pipe_conf_reg = if self.active_pipe == 0 {
                regs::PIPEA_CONF
            } else {
                regs::PIPEB_CONF
            };
            self.write32(pipe_conf_reg, 0);
        }

        self.initialized = false;
    }
}

/// Global GPU instance
static INTEL_GPU: Mutex<Option<IntelGpu>> = Mutex::new(None);

/// Check if device ID is an Intel GPU
pub fn is_intel_gpu(vendor_id: u16, device_id: u16) -> bool {
    if vendor_id != INTEL_VENDOR_ID {
        return false;
    }

    // Check if it's a known GPU device class (VGA or display controller)
    GpuGeneration::from_device_id(device_id) != GpuGeneration::Unknown
}

/// Initialize Intel GPU from PCI
pub fn init_from_pci(
    bus: u8,
    device: u8,
    function: u8,
    device_id: u16,
    bar0: u64,
    bar2: u64,
) -> Result<(), &'static str> {
    let generation = GpuGeneration::from_device_id(device_id);

    if generation == GpuGeneration::Unknown {
        return Err("Unknown Intel GPU");
    }

    let mut gpu = IntelGpu::new();
    gpu.bus = bus;
    gpu.device = device;
    gpu.function = function;
    gpu.device_id = device_id;
    gpu.generation = generation;
    gpu.mmio_base = bar0;
    gpu.mmio_size = 2 * 1024 * 1024; // 2 MB typical

    // GTT is in BAR2 for most generations, or at end of BAR0 for older
    if bar2 != 0 {
        gpu.gtt_base = bar2;
    } else {
        gpu.gtt_base = bar0 + 2 * 1024 * 1024;
    }

    // Read stolen memory base from BSM register
    // (This requires PCI config access which we'd need to implement)
    // For now, use a reasonable default
    gpu.stolen_size = generation.min_graphics_memory();

    crate::kprintln!(
        "intel_gpu: detected {:?} GPU (device {:04X})",
        generation,
        device_id
    );
    crate::kprintln!(
        "intel_gpu: MMIO at {:#X}, GTT at {:#X}",
        gpu.mmio_base,
        gpu.gtt_base
    );

    // Initialize
    gpu.init()?;

    *INTEL_GPU.lock() = Some(gpu);

    Ok(())
}

/// Get GPU info
pub fn get_info() -> Option<(u16, GpuGeneration, u32, u32)> {
    let gpu = INTEL_GPU.lock();
    gpu.as_ref().map(|g| {
        let (w, h) = g
            .current_mode
            .map(|m| (m.width, m.height))
            .unwrap_or((0, 0));
        (g.device_id, g.generation, w, h)
    })
}

/// Check if Intel GPU is present
pub fn is_present() -> bool {
    INTEL_GPU.lock().is_some()
}

/// Set display mode
pub fn set_mode(mode: IntelDisplayMode) -> Result<(), &'static str> {
    let mut gpu = INTEL_GPU.lock();
    match gpu.as_mut() {
        Some(g) => g.set_mode(&mode),
        None => Err("No Intel GPU"),
    }
}

/// Get framebuffer address
pub fn framebuffer_address() -> Option<u64> {
    let gpu = INTEL_GPU.lock();
    gpu.as_ref().map(|g| g.framebuffer_address())
}

/// Wait for vblank
pub fn wait_vblank() {
    let gpu = INTEL_GPU.lock();
    if let Some(g) = gpu.as_ref() {
        g.wait_vblank();
    }
}

/// Probe PCI for Intel GPU
pub fn probe_pci() {
    use crate::drivers::pci::{scan, read_bar};

    // Use PCI module to find Intel GPU
    for dev in scan() {
        if dev.id.vendor_id == INTEL_VENDOR_ID {
            // Check device class (VGA controller = 0x03, subclass 0x00)
            if (dev.class.class_code == 0x03 && dev.class.subclass == 0x00)
                || (dev.class.class_code == 0x03 && dev.class.subclass == 0x80)
            {
                let generation = GpuGeneration::from_device_id(dev.id.device_id);
                if generation != GpuGeneration::Unknown {
                    crate::kprintln!(
                        "intel_gpu: found Intel {:?} at {:02X}:{:02X}.{:X}",
                        generation,
                        dev.addr.bus,
                        dev.addr.device,
                        dev.addr.function
                    );

                    // Get BARs (bar0 = MMIO, bar2 = GTT/aperture for some generations)
                    let (bar0_addr, _bar0_io) = read_bar(&dev, 0);
                    let (bar2_addr, _bar2_io) = read_bar(&dev, 2);

                    if let Err(e) = init_from_pci(
                        dev.addr.bus,
                        dev.addr.device,
                        dev.addr.function,
                        dev.id.device_id,
                        bar0_addr,
                        bar2_addr,
                    ) {
                        crate::kprintln!("intel_gpu: init failed: {}", e);
                    }
                    break;
                }
            }
        }
    }
}

/// Initialize Intel GPU subsystem
pub fn init() {
    crate::kprintln!("intel_gpu: probing for Intel integrated graphics");
    probe_pci();
}
