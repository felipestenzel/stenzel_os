//! VirtualBox Video Driver
//!
//! Graphics driver for VirtualBox virtual display adapter.

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::vec;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// VBox Video PCI IDs
pub const VBOX_VIDEO_VENDOR_ID: u16 = 0x80EE;
pub const VBOX_VIDEO_DEVICE_ID: u16 = 0xBEEF;

/// VBVA (VirtualBox Video Acceleration) commands
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VbvaCmd {
    QueryConfig = 1,
    Flush = 2,
    InfoView = 3,
    InfoHeap = 4,
    InfoScreen = 5,
    Enable = 6,
    MousePointer = 7,
}

/// Display mode
#[derive(Debug, Clone, Copy, Default)]
pub struct DisplayMode {
    pub width: u32,
    pub height: u32,
    pub bpp: u32,
    pub flags: u32,
}

/// Screen info
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ScreenInfo {
    pub index: u32,
    pub flags: u32,
    pub origin_x: i32,
    pub origin_y: i32,
    pub width: u32,
    pub height: u32,
    pub bpp: u32,
}

/// VBVA buffer
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct VbvaBuffer {
    pub flags: u32,
    pub data_offset: u32,
    pub free_offset: u32,
    pub record_first_index: u32,
    pub record_free_index: u32,
    pub records_count: u32,
    pub partial_write_thresh: u32,
}

/// Video mode info
#[derive(Debug, Clone, Copy)]
pub struct VideoModeInfo {
    pub mode_number: u16,
    pub width: u32,
    pub height: u32,
    pub bpp: u32,
    pub bytes_per_line: u32,
    pub text_mode: bool,
}

/// VBox Video device
pub struct VboxVideoDevice {
    /// MMIO base (VRAM)
    vram_base: u64,
    /// VRAM size
    vram_size: u32,
    /// I/O port base
    io_port: u16,
    /// Current mode
    current_mode: DisplayMode,
    /// Available modes
    modes: Vec<VideoModeInfo>,
    /// Number of displays
    display_count: u32,
    /// Screen info per display
    screens: Vec<ScreenInfo>,
    /// VBVA buffer
    vbva_buffer: Option<VbvaBuffer>,
    /// VBVA enabled
    vbva_enabled: AtomicBool,
    /// Hardware cursor enabled
    hw_cursor_enabled: bool,
    /// Initialized flag
    initialized: AtomicBool,
    /// Cursor X position
    cursor_x: AtomicU32,
    /// Cursor Y position
    cursor_y: AtomicU32,
}

impl VboxVideoDevice {
    /// VBE dispi index port
    const VBE_DISPI_IOPORT_INDEX: u16 = 0x01CE;
    /// VBE dispi data port
    const VBE_DISPI_IOPORT_DATA: u16 = 0x01CF;

    /// VBE dispi registers
    const VBE_DISPI_INDEX_ID: u16 = 0;
    const VBE_DISPI_INDEX_XRES: u16 = 1;
    const VBE_DISPI_INDEX_YRES: u16 = 2;
    const VBE_DISPI_INDEX_BPP: u16 = 3;
    const VBE_DISPI_INDEX_ENABLE: u16 = 4;
    const VBE_DISPI_INDEX_BANK: u16 = 5;
    const VBE_DISPI_INDEX_VIRT_WIDTH: u16 = 6;
    const VBE_DISPI_INDEX_VIRT_HEIGHT: u16 = 7;
    const VBE_DISPI_INDEX_X_OFFSET: u16 = 8;
    const VBE_DISPI_INDEX_Y_OFFSET: u16 = 9;
    const VBE_DISPI_INDEX_VIDEO_MEMORY_64K: u16 = 10;

    /// VBE dispi flags
    const VBE_DISPI_DISABLED: u16 = 0x00;
    const VBE_DISPI_ENABLED: u16 = 0x01;
    const VBE_DISPI_LFB_ENABLED: u16 = 0x40;
    const VBE_DISPI_NOCLEARMEM: u16 = 0x80;

    /// VBE ID
    const VBE_DISPI_ID_VBOX_VIDEO: u16 = 0xBE00;

    /// Create new device
    pub fn new(vram_base: u64, vram_size: u32, io_port: u16) -> Self {
        Self {
            vram_base,
            vram_size,
            io_port,
            current_mode: DisplayMode::default(),
            modes: Vec::new(),
            display_count: 1,
            screens: Vec::new(),
            vbva_buffer: None,
            vbva_enabled: AtomicBool::new(false),
            hw_cursor_enabled: false,
            initialized: AtomicBool::new(false),
            cursor_x: AtomicU32::new(0),
            cursor_y: AtomicU32::new(0),
        }
    }

    /// Read VBE dispi register
    fn read_dispi(&self, index: u16) -> u16 {
        unsafe {
            core::arch::asm!(
                "out dx, ax",
                in("dx") Self::VBE_DISPI_IOPORT_INDEX,
                in("ax") index,
                options(nostack, nomem)
            );

            let value: u16;
            core::arch::asm!(
                "in ax, dx",
                in("dx") Self::VBE_DISPI_IOPORT_DATA,
                out("ax") value,
                options(nostack, nomem)
            );
            value
        }
    }

    /// Write VBE dispi register
    fn write_dispi(&self, index: u16, value: u16) {
        unsafe {
            core::arch::asm!(
                "out dx, ax",
                in("dx") Self::VBE_DISPI_IOPORT_INDEX,
                in("ax") index,
                options(nostack, nomem)
            );

            core::arch::asm!(
                "out dx, ax",
                in("dx") Self::VBE_DISPI_IOPORT_DATA,
                in("ax") value,
                options(nostack, nomem)
            );
        }
    }

    /// Initialize device
    pub fn init(&mut self) -> Result<(), &'static str> {
        // Check VBE ID
        let id = self.read_dispi(Self::VBE_DISPI_INDEX_ID);
        if id < Self::VBE_DISPI_ID_VBOX_VIDEO {
            return Err("VirtualBox Video not detected");
        }

        // Get VRAM size
        let vram_64k = self.read_dispi(Self::VBE_DISPI_INDEX_VIDEO_MEMORY_64K);
        if vram_64k > 0 {
            self.vram_size = (vram_64k as u32) * 64 * 1024;
        }

        // Build mode list
        self.build_mode_list();

        // Set default mode (1024x768x32)
        self.set_mode(1024, 768, 32)?;

        self.initialized.store(true, Ordering::Release);
        crate::kprintln!("vboxvideo: Initialized, VRAM={}MB, modes={}",
            self.vram_size / (1024 * 1024), self.modes.len());

        Ok(())
    }

    /// Build available mode list
    fn build_mode_list(&mut self) {
        self.modes.clear();

        // Common resolutions
        let resolutions: [(u32, u32); 12] = [
            (640, 480), (800, 600), (1024, 768), (1152, 864),
            (1280, 720), (1280, 800), (1280, 1024), (1366, 768),
            (1440, 900), (1600, 900), (1920, 1080), (2560, 1440),
        ];

        let bpps: [u32; 3] = [16, 24, 32];

        for (i, (w, h)) in resolutions.iter().enumerate() {
            for &bpp in &bpps {
                let bytes_per_pixel = bpp / 8;
                let bytes_per_line = w * bytes_per_pixel;
                let total_size = bytes_per_line * h;

                if total_size <= self.vram_size {
                    self.modes.push(VideoModeInfo {
                        mode_number: (i * 3 + bpp as usize / 8) as u16,
                        width: *w,
                        height: *h,
                        bpp,
                        bytes_per_line,
                        text_mode: false,
                    });
                }
            }
        }
    }

    /// Set display mode
    pub fn set_mode(&mut self, width: u32, height: u32, bpp: u32) -> Result<(), &'static str> {
        let bytes_per_pixel = bpp / 8;
        let total_size = width * height * bytes_per_pixel;

        if total_size > self.vram_size {
            return Err("Mode requires more VRAM than available");
        }

        // Disable display
        self.write_dispi(Self::VBE_DISPI_INDEX_ENABLE, Self::VBE_DISPI_DISABLED);

        // Set mode
        self.write_dispi(Self::VBE_DISPI_INDEX_XRES, width as u16);
        self.write_dispi(Self::VBE_DISPI_INDEX_YRES, height as u16);
        self.write_dispi(Self::VBE_DISPI_INDEX_BPP, bpp as u16);

        // Enable display with LFB
        self.write_dispi(Self::VBE_DISPI_INDEX_ENABLE,
            Self::VBE_DISPI_ENABLED | Self::VBE_DISPI_LFB_ENABLED);

        // Update current mode
        self.current_mode = DisplayMode {
            width,
            height,
            bpp,
            flags: 0,
        };

        Ok(())
    }

    /// Get framebuffer address
    pub fn framebuffer(&self) -> u64 {
        self.vram_base
    }

    /// Get framebuffer size
    pub fn framebuffer_size(&self) -> u32 {
        let bpp = self.current_mode.bpp / 8;
        self.current_mode.width * self.current_mode.height * bpp
    }

    /// Get pitch (bytes per line)
    pub fn pitch(&self) -> u32 {
        self.current_mode.width * (self.current_mode.bpp / 8)
    }

    /// Get current mode
    pub fn mode(&self) -> &DisplayMode {
        &self.current_mode
    }

    /// Get available modes
    pub fn modes(&self) -> &[VideoModeInfo] {
        &self.modes
    }

    /// Enable VBVA (video acceleration)
    pub fn enable_vbva(&mut self) -> Result<(), &'static str> {
        if !self.initialized.load(Ordering::Acquire) {
            return Err("Device not initialized");
        }

        // In real implementation, set up VBVA buffer and notify VMMDev
        self.vbva_enabled.store(true, Ordering::Release);
        Ok(())
    }

    /// Disable VBVA
    pub fn disable_vbva(&mut self) {
        self.vbva_enabled.store(false, Ordering::Release);
    }

    /// Is VBVA enabled?
    pub fn is_vbva_enabled(&self) -> bool {
        self.vbva_enabled.load(Ordering::Acquire)
    }

    /// Set cursor position
    pub fn set_cursor_pos(&self, x: u32, y: u32) {
        self.cursor_x.store(x, Ordering::Relaxed);
        self.cursor_y.store(y, Ordering::Relaxed);
    }

    /// Show/hide hardware cursor
    pub fn set_cursor_visible(&mut self, visible: bool) {
        self.hw_cursor_enabled = visible;
    }

    /// Define cursor shape
    pub fn define_cursor(&self, _hotspot_x: u32, _hotspot_y: u32, _width: u32, _height: u32, _data: &[u32]) {
        // In real implementation, send cursor shape to VMMDev
    }

    /// Update dirty region
    pub fn update(&self, x: u32, y: u32, width: u32, height: u32) {
        if !self.is_vbva_enabled() {
            return;
        }

        // In real implementation, add dirty rect to VBVA buffer
        let _ = (x, y, width, height);
    }

    /// Flush pending updates
    pub fn flush(&self) {
        if !self.is_vbva_enabled() {
            return;
        }

        // In real implementation, signal VMMDev to process VBVA buffer
    }

    /// Handle display change from host
    pub fn handle_display_change(&mut self, width: u32, height: u32, bpp: u32) {
        if width > 0 && height > 0 && bpp > 0 {
            let _ = self.set_mode(width, height, bpp);
        }
    }

    /// Format status
    pub fn format_status(&self) -> String {
        alloc::format!(
            "VBoxVideo: {}x{}x{} VRAM={}MB VBVA={}",
            self.current_mode.width,
            self.current_mode.height,
            self.current_mode.bpp,
            self.vram_size / (1024 * 1024),
            self.is_vbva_enabled()
        )
    }
}

impl Default for VboxVideoDevice {
    fn default() -> Self {
        Self::new(0, 0, 0)
    }
}

/// VBox Video manager
pub struct VboxVideoManager {
    devices: Vec<VboxVideoDevice>,
}

impl VboxVideoManager {
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
        }
    }

    pub fn add_device(&mut self, device: VboxVideoDevice) -> usize {
        let idx = self.devices.len();
        self.devices.push(device);
        idx
    }

    pub fn get_device(&mut self, idx: usize) -> Option<&mut VboxVideoDevice> {
        self.devices.get_mut(idx)
    }

    pub fn device_count(&self) -> usize {
        self.devices.len()
    }
}

impl Default for VboxVideoManager {
    fn default() -> Self {
        Self::new()
    }
}
