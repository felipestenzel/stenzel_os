//! VESA BIOS Extensions (VBE) Support
//!
//! VBE (Video BIOS Extensions) provides standardized video mode support
//! through BIOS services. Since BIOS interrupts cannot be called from
//! long mode, this module provides:
//!
//! 1. VBE data structures for parsing bootloader-provided mode info
//! 2. Common VBE mode definitions
//! 3. VBE mode enumeration from multiboot/bootloader info
//! 4. Integration with the framebuffer driver
//!
//! For actual mode switching in x86_64:
//! - UEFI systems: Use GOP (handled by framebuffer.rs)
//! - BIOS systems: Bootloader sets VBE mode before entering long mode
//! - Hardware mode-setting: Requires GPU-specific drivers (Intel, AMD, etc.)

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;

/// VBE Controller Information (returned by VBE function 00h)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct VbeControllerInfo {
    /// VBE Signature ('VESA')
    pub signature: [u8; 4],
    /// VBE Version (high byte = major, low byte = minor)
    pub version: u16,
    /// Pointer to OEM String
    pub oem_string_ptr: u32,
    /// Capabilities of graphics controller
    pub capabilities: u32,
    /// Pointer to VideoModeList
    pub video_mode_ptr: u32,
    /// Total memory in 64KB blocks
    pub total_memory: u16,
    /// VBE implementation Software revision
    pub software_rev: u16,
    /// Vendor name string pointer
    pub vendor: u32,
    /// Product name string pointer
    pub product_name: u32,
    /// Product revision string pointer
    pub product_rev: u32,
    /// Reserved for VBE/AF software
    pub reserved: [u8; 222],
    /// OEM data
    pub oem_data: [u8; 256],
}

impl VbeControllerInfo {
    /// Check if the signature is valid ("VESA")
    pub fn is_valid(&self) -> bool {
        self.signature == *b"VESA"
    }

    /// Get VBE version as (major, minor)
    pub fn version(&self) -> (u8, u8) {
        ((self.version >> 8) as u8, (self.version & 0xFF) as u8)
    }

    /// Get total video memory in bytes
    pub fn total_memory_bytes(&self) -> usize {
        self.total_memory as usize * 64 * 1024
    }

    /// Check capability flags
    pub fn supports_dac_8bit(&self) -> bool {
        self.capabilities & 0x01 != 0
    }

    pub fn supports_vga_compatible(&self) -> bool {
        self.capabilities & 0x02 == 0
    }

    pub fn supports_af(&self) -> bool {
        self.capabilities & 0x08 != 0
    }
}

/// VBE Mode Information (returned by VBE function 01h)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct VbeModeInfo {
    // Mandatory information for all VBE revisions
    /// Mode attributes
    pub mode_attributes: u16,
    /// Window A attributes
    pub win_a_attributes: u8,
    /// Window B attributes
    pub win_b_attributes: u8,
    /// Window granularity in KB
    pub win_granularity: u16,
    /// Window size in KB
    pub win_size: u16,
    /// Window A start segment
    pub win_a_segment: u16,
    /// Window B start segment
    pub win_b_segment: u16,
    /// Pointer to window function
    pub win_func_ptr: u32,
    /// Bytes per scan line
    pub bytes_per_scanline: u16,

    // Mandatory information for VBE 1.2 and above
    /// Horizontal resolution in pixels
    pub x_resolution: u16,
    /// Vertical resolution in pixels
    pub y_resolution: u16,
    /// Character cell width in pixels
    pub x_char_size: u8,
    /// Character cell height in pixels
    pub y_char_size: u8,
    /// Number of memory planes
    pub number_of_planes: u8,
    /// Bits per pixel
    pub bits_per_pixel: u8,
    /// Number of banks
    pub number_of_banks: u8,
    /// Memory model type
    pub memory_model: u8,
    /// Bank size in KB
    pub bank_size: u8,
    /// Number of images
    pub number_of_image_pages: u8,
    /// Reserved for page function
    pub reserved1: u8,

    // Direct Color fields
    /// Size of direct color red mask in bits
    pub red_mask_size: u8,
    /// Bit position of LSB of red mask
    pub red_field_position: u8,
    /// Size of direct color green mask in bits
    pub green_mask_size: u8,
    /// Bit position of LSB of green mask
    pub green_field_position: u8,
    /// Size of direct color blue mask in bits
    pub blue_mask_size: u8,
    /// Bit position of LSB of blue mask
    pub blue_field_position: u8,
    /// Size of direct color reserved mask in bits
    pub reserved_mask_size: u8,
    /// Bit position of LSB of reserved mask
    pub reserved_field_position: u8,
    /// Direct color mode info
    pub direct_color_mode_info: u8,

    // Mandatory information for VBE 2.0 and above
    /// Physical address of linear framebuffer
    pub phys_base_ptr: u32,
    /// Reserved
    pub reserved2: u32,
    /// Reserved
    pub reserved3: u16,

    // Mandatory information for VBE 3.0 and above
    /// Bytes per scanline for linear modes
    pub lin_bytes_per_scanline: u16,
    /// Number of image pages for banked modes
    pub bnk_number_of_image_pages: u8,
    /// Number of image pages for linear modes
    pub lin_number_of_image_pages: u8,
    /// Linear mode: red mask size
    pub lin_red_mask_size: u8,
    /// Linear mode: red field position
    pub lin_red_field_position: u8,
    /// Linear mode: green mask size
    pub lin_green_mask_size: u8,
    /// Linear mode: green field position
    pub lin_green_field_position: u8,
    /// Linear mode: blue mask size
    pub lin_blue_mask_size: u8,
    /// Linear mode: blue field position
    pub lin_blue_field_position: u8,
    /// Linear mode: reserved mask size
    pub lin_reserved_mask_size: u8,
    /// Linear mode: reserved field position
    pub lin_reserved_field_position: u8,
    /// Maximum pixel clock (in Hz) for graphics mode
    pub max_pixel_clock: u32,

    /// Reserved
    pub reserved4: [u8; 189],
}

impl VbeModeInfo {
    /// Check if mode is supported
    pub fn is_supported(&self) -> bool {
        self.mode_attributes & 0x01 != 0
    }

    /// Check if TTY output is supported
    pub fn has_tty_support(&self) -> bool {
        self.mode_attributes & 0x04 != 0
    }

    /// Check if color mode
    pub fn is_color(&self) -> bool {
        self.mode_attributes & 0x08 != 0
    }

    /// Check if graphics mode
    pub fn is_graphics(&self) -> bool {
        self.mode_attributes & 0x10 != 0
    }

    /// Check if VGA compatible
    pub fn is_vga_compatible(&self) -> bool {
        self.mode_attributes & 0x20 == 0
    }

    /// Check if windowed mode supported
    pub fn supports_windowed(&self) -> bool {
        self.mode_attributes & 0x40 == 0
    }

    /// Check if linear framebuffer available
    pub fn supports_linear_framebuffer(&self) -> bool {
        self.mode_attributes & 0x80 != 0
    }

    /// Check if double scan available
    pub fn supports_double_scan(&self) -> bool {
        self.mode_attributes & 0x100 != 0
    }

    /// Check if interlace available
    pub fn supports_interlace(&self) -> bool {
        self.mode_attributes & 0x200 != 0
    }

    /// Check if hardware triple buffering supported
    pub fn supports_triple_buffer(&self) -> bool {
        self.mode_attributes & 0x400 != 0
    }

    /// Check if hardware stereoscopic display supported
    pub fn supports_stereo(&self) -> bool {
        self.mode_attributes & 0x800 != 0
    }

    /// Check if dual display start address supported
    pub fn supports_dual_display(&self) -> bool {
        self.mode_attributes & 0x1000 != 0
    }

    /// Get memory model type
    pub fn memory_model_type(&self) -> VbeMemoryModel {
        VbeMemoryModel::from_u8(self.memory_model)
    }

    /// Get linear framebuffer address
    pub fn framebuffer_address(&self) -> u64 {
        self.phys_base_ptr as u64
    }

    /// Get stride (bytes per scanline) for linear modes
    pub fn stride(&self) -> usize {
        if self.lin_bytes_per_scanline != 0 {
            self.lin_bytes_per_scanline as usize
        } else {
            self.bytes_per_scanline as usize
        }
    }

    /// Get bytes per pixel
    pub fn bytes_per_pixel(&self) -> usize {
        (self.bits_per_pixel as usize + 7) / 8
    }

    /// Get framebuffer size in bytes
    pub fn framebuffer_size(&self) -> usize {
        self.stride() * self.y_resolution as usize
    }

    /// Convert to a display mode descriptor
    pub fn to_display_mode(&self, mode_number: u16) -> VbeDisplayMode {
        VbeDisplayMode {
            mode_number,
            width: self.x_resolution,
            height: self.y_resolution,
            bpp: self.bits_per_pixel,
            stride: self.stride() as u32,
            framebuffer: self.phys_base_ptr as u64,
            red_mask: self.red_mask_size,
            red_pos: self.red_field_position,
            green_mask: self.green_mask_size,
            green_pos: self.green_field_position,
            blue_mask: self.blue_mask_size,
            blue_pos: self.blue_field_position,
            memory_model: self.memory_model,
            linear_supported: self.supports_linear_framebuffer(),
        }
    }
}

/// VBE Memory Model types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VbeMemoryModel {
    /// Text mode
    Text,
    /// CGA graphics
    Cga,
    /// Hercules graphics
    Hercules,
    /// Planar (EGA/VGA 16 color)
    Planar,
    /// Packed pixel (256 color)
    PackedPixel,
    /// Non-chain 4, 256 color
    NonChain4,
    /// Direct Color (high/true color)
    DirectColor,
    /// YUV (video)
    Yuv,
    /// Unknown
    Unknown(u8),
}

impl VbeMemoryModel {
    pub fn from_u8(val: u8) -> Self {
        match val {
            0 => VbeMemoryModel::Text,
            1 => VbeMemoryModel::Cga,
            2 => VbeMemoryModel::Hercules,
            3 => VbeMemoryModel::Planar,
            4 => VbeMemoryModel::PackedPixel,
            5 => VbeMemoryModel::NonChain4,
            6 => VbeMemoryModel::DirectColor,
            7 => VbeMemoryModel::Yuv,
            n => VbeMemoryModel::Unknown(n),
        }
    }
}

/// Simplified display mode info
#[derive(Debug, Clone, Copy)]
pub struct VbeDisplayMode {
    /// VBE mode number
    pub mode_number: u16,
    /// Horizontal resolution
    pub width: u16,
    /// Vertical resolution
    pub height: u16,
    /// Bits per pixel
    pub bpp: u8,
    /// Bytes per scanline
    pub stride: u32,
    /// Physical framebuffer address
    pub framebuffer: u64,
    /// Red mask size
    pub red_mask: u8,
    /// Red field position
    pub red_pos: u8,
    /// Green mask size
    pub green_mask: u8,
    /// Green field position
    pub green_pos: u8,
    /// Blue mask size
    pub blue_mask: u8,
    /// Blue field position
    pub blue_pos: u8,
    /// Memory model
    pub memory_model: u8,
    /// Linear framebuffer supported
    pub linear_supported: bool,
}

impl VbeDisplayMode {
    /// Check if this is a graphics mode
    pub fn is_graphics(&self) -> bool {
        self.memory_model >= 4
    }

    /// Check if this is a true color mode
    pub fn is_true_color(&self) -> bool {
        self.memory_model == 6 && self.bpp >= 15
    }

    /// Get mode as a readable string
    pub fn to_string(&self) -> String {
        alloc::format!(
            "{}x{}x{} (mode {:04X})",
            self.width, self.height, self.bpp, self.mode_number
        )
    }
}

/// Standard VBE mode numbers
pub mod standard_modes {
    // Text modes
    pub const MODE_TEXT_80X25: u16 = 0x003;
    pub const MODE_TEXT_132X25: u16 = 0x109;
    pub const MODE_TEXT_132X43: u16 = 0x10A;
    pub const MODE_TEXT_132X50: u16 = 0x10B;
    pub const MODE_TEXT_132X60: u16 = 0x10C;

    // 4-bit (16 color) modes
    pub const MODE_640X480X4: u16 = 0x012;
    pub const MODE_800X600X4: u16 = 0x102;
    pub const MODE_1024X768X4: u16 = 0x104;
    pub const MODE_1280X1024X4: u16 = 0x106;

    // 8-bit (256 color) modes
    pub const MODE_320X200X8: u16 = 0x013;
    pub const MODE_640X400X8: u16 = 0x100;
    pub const MODE_640X480X8: u16 = 0x101;
    pub const MODE_800X600X8: u16 = 0x103;
    pub const MODE_1024X768X8: u16 = 0x105;
    pub const MODE_1280X1024X8: u16 = 0x107;

    // 15-bit (32K color, 5-5-5) modes
    pub const MODE_320X200X15: u16 = 0x10D;
    pub const MODE_640X480X15: u16 = 0x110;
    pub const MODE_800X600X15: u16 = 0x113;
    pub const MODE_1024X768X15: u16 = 0x116;
    pub const MODE_1280X1024X15: u16 = 0x119;

    // 16-bit (64K color, 5-6-5) modes
    pub const MODE_320X200X16: u16 = 0x10E;
    pub const MODE_640X480X16: u16 = 0x111;
    pub const MODE_800X600X16: u16 = 0x114;
    pub const MODE_1024X768X16: u16 = 0x117;
    pub const MODE_1280X1024X16: u16 = 0x11A;

    // 24-bit (16M color) modes
    pub const MODE_320X200X24: u16 = 0x10F;
    pub const MODE_640X480X24: u16 = 0x112;
    pub const MODE_800X600X24: u16 = 0x115;
    pub const MODE_1024X768X24: u16 = 0x118;
    pub const MODE_1280X1024X24: u16 = 0x11B;

    // Linear framebuffer bit (add to mode number)
    pub const LINEAR_FRAMEBUFFER: u16 = 0x4000;

    // Don't clear display memory bit
    pub const NO_CLEAR_MEMORY: u16 = 0x8000;
}

/// Common VBE mode presets
#[derive(Debug, Clone, Copy)]
pub struct VbeModePreset {
    pub name: &'static str,
    pub width: u16,
    pub height: u16,
    pub bpp: u8,
    pub standard_mode: Option<u16>,
}

pub const MODE_PRESETS: &[VbeModePreset] = &[
    VbeModePreset { name: "VGA", width: 640, height: 480, bpp: 32, standard_mode: None },
    VbeModePreset { name: "SVGA", width: 800, height: 600, bpp: 32, standard_mode: None },
    VbeModePreset { name: "XGA", width: 1024, height: 768, bpp: 32, standard_mode: None },
    VbeModePreset { name: "SXGA", width: 1280, height: 1024, bpp: 32, standard_mode: None },
    VbeModePreset { name: "WXGA", width: 1280, height: 800, bpp: 32, standard_mode: None },
    VbeModePreset { name: "720p", width: 1280, height: 720, bpp: 32, standard_mode: None },
    VbeModePreset { name: "WSXGA+", width: 1680, height: 1050, bpp: 32, standard_mode: None },
    VbeModePreset { name: "1080p", width: 1920, height: 1080, bpp: 32, standard_mode: None },
    VbeModePreset { name: "WUXGA", width: 1920, height: 1200, bpp: 32, standard_mode: None },
    VbeModePreset { name: "QHD", width: 2560, height: 1440, bpp: 32, standard_mode: None },
    VbeModePreset { name: "4K UHD", width: 3840, height: 2160, bpp: 32, standard_mode: None },
];

/// Current VBE state
#[derive(Debug)]
pub struct VbeState {
    /// Available modes
    pub modes: Vec<VbeDisplayMode>,
    /// Current mode
    pub current_mode: Option<VbeDisplayMode>,
    /// Controller info available
    pub controller_info: Option<VbeControllerInfo>,
    /// VBE version
    pub version: Option<(u8, u8)>,
    /// Total video memory
    pub total_memory: usize,
    /// OEM string
    pub oem_string: Option<String>,
}

impl VbeState {
    pub const fn new() -> Self {
        Self {
            modes: Vec::new(),
            current_mode: None,
            controller_info: None,
            version: None,
            total_memory: 0,
            oem_string: None,
        }
    }

    /// Find a mode by resolution and bpp
    pub fn find_mode(&self, width: u16, height: u16, bpp: u8) -> Option<&VbeDisplayMode> {
        self.modes.iter().find(|m| {
            m.width == width && m.height == height && m.bpp == bpp
        })
    }

    /// Find best matching mode
    pub fn find_best_mode(&self, target_width: u16, target_height: u16, prefer_32bpp: bool) -> Option<&VbeDisplayMode> {
        let mut best: Option<&VbeDisplayMode> = None;
        let mut best_score = i64::MIN;

        for mode in &self.modes {
            // Skip non-graphics modes
            if !mode.is_graphics() {
                continue;
            }

            // Calculate score
            let mut score: i64 = 0;

            // Prefer matching resolution
            if mode.width == target_width && mode.height == target_height {
                score += 10000;
            } else {
                // Penalize based on difference
                let w_diff = (mode.width as i64 - target_width as i64).abs();
                let h_diff = (mode.height as i64 - target_height as i64).abs();
                score -= w_diff + h_diff;
            }

            // Prefer higher bpp
            score += mode.bpp as i64 * 10;

            // Bonus for 32bpp if preferred
            if prefer_32bpp && mode.bpp == 32 {
                score += 500;
            }

            // Prefer linear framebuffer
            if mode.linear_supported {
                score += 100;
            }

            if score > best_score {
                best_score = score;
                best = Some(mode);
            }
        }

        best
    }

    /// Get all modes matching resolution
    pub fn modes_at_resolution(&self, width: u16, height: u16) -> Vec<&VbeDisplayMode> {
        self.modes.iter()
            .filter(|m| m.width == width && m.height == height)
            .collect()
    }

    /// Get all available resolutions (unique width x height)
    pub fn available_resolutions(&self) -> Vec<(u16, u16)> {
        let mut resolutions: Vec<(u16, u16)> = self.modes
            .iter()
            .filter(|m| m.is_graphics())
            .map(|m| (m.width, m.height))
            .collect();
        resolutions.sort();
        resolutions.dedup();
        resolutions
    }
}

/// Global VBE state
static VBE_STATE: Mutex<VbeState> = Mutex::new(VbeState::new());

/// Initialize VBE from bootloader-provided information
pub fn init_from_framebuffer(info: &super::framebuffer::FrameBufferState) {
    let mut state = VBE_STATE.lock();

    // Create a display mode from current framebuffer
    let mode = VbeDisplayMode {
        mode_number: 0xFFFF, // Custom mode
        width: info.width() as u16,
        height: info.height() as u16,
        bpp: (info.bytes_per_pixel() * 8) as u8,
        stride: (info.stride() * info.bytes_per_pixel()) as u32,
        framebuffer: info.buffer().as_ptr() as u64,
        red_mask: 8,
        red_pos: match info.pixel_format() {
            bootloader_api::info::PixelFormat::Rgb => 0,
            bootloader_api::info::PixelFormat::Bgr => 16,
            _ => 0,
        },
        green_mask: 8,
        green_pos: 8,
        blue_mask: 8,
        blue_pos: match info.pixel_format() {
            bootloader_api::info::PixelFormat::Rgb => 16,
            bootloader_api::info::PixelFormat::Bgr => 0,
            _ => 16,
        },
        memory_model: 6, // Direct color
        linear_supported: true,
    };

    state.current_mode = Some(mode);
    state.modes.push(mode);
    state.total_memory = info.buffer().len();

    crate::kprintln!(
        "vbe: initialized from framebuffer: {}x{}x{}",
        mode.width, mode.height, mode.bpp
    );
}

/// Initialize VBE from multiboot information
#[cfg(feature = "multiboot")]
pub fn init_from_multiboot(vbe_info: *const u8, mode_info: *const u8) {
    if vbe_info.is_null() || mode_info.is_null() {
        return;
    }

    let mut state = VBE_STATE.lock();

    unsafe {
        // Parse controller info
        let ctrl_info = &*(vbe_info as *const VbeControllerInfo);
        if ctrl_info.is_valid() {
            state.version = Some(ctrl_info.version());
            state.total_memory = ctrl_info.total_memory_bytes();
            state.controller_info = Some(*ctrl_info);
        }

        // Parse mode info
        let mode = &*(mode_info as *const VbeModeInfo);
        let display_mode = mode.to_display_mode(0xFFFF);
        state.current_mode = Some(display_mode);
        state.modes.push(display_mode);
    }
}

/// Check if VBE is available
pub fn is_available() -> bool {
    VBE_STATE.lock().current_mode.is_some()
}

/// Get current mode
pub fn current_mode() -> Option<VbeDisplayMode> {
    VBE_STATE.lock().current_mode
}

/// Get VBE version
pub fn version() -> Option<(u8, u8)> {
    VBE_STATE.lock().version
}

/// Get total video memory
pub fn total_memory() -> usize {
    VBE_STATE.lock().total_memory
}

/// Get all available modes
pub fn available_modes() -> Vec<VbeDisplayMode> {
    VBE_STATE.lock().modes.clone()
}

/// Get available resolutions
pub fn available_resolutions() -> Vec<(u16, u16)> {
    VBE_STATE.lock().available_resolutions()
}

/// Find mode by resolution and bpp
pub fn find_mode(width: u16, height: u16, bpp: u8) -> Option<VbeDisplayMode> {
    VBE_STATE.lock().find_mode(width, height, bpp).copied()
}

/// Add a mode to the available modes list
pub fn add_mode(mode: VbeDisplayMode) {
    let mut state = VBE_STATE.lock();
    // Check if mode already exists
    if state.modes.iter().any(|m| m.mode_number == mode.mode_number) {
        return;
    }
    state.modes.push(mode);
}

/// Set current mode (informational only, actual mode setting requires bootloader)
pub fn set_current_mode(mode: VbeDisplayMode) {
    let mut state = VBE_STATE.lock();
    state.current_mode = Some(mode);
}

// =============================================================================
// EDID Support (for monitor capabilities)
// =============================================================================

/// EDID block (128 bytes)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct EdidBlock {
    /// Header (always 00 FF FF FF FF FF FF 00)
    pub header: [u8; 8],
    /// Manufacturer ID
    pub manufacturer_id: [u8; 2],
    /// Product code
    pub product_code: u16,
    /// Serial number
    pub serial_number: u32,
    /// Week of manufacture
    pub manufacture_week: u8,
    /// Year of manufacture
    pub manufacture_year: u8,
    /// EDID version
    pub version: u8,
    /// EDID revision
    pub revision: u8,
    /// Video input definition
    pub video_input: u8,
    /// Max horizontal size in cm
    pub max_h_size: u8,
    /// Max vertical size in cm
    pub max_v_size: u8,
    /// Gamma
    pub gamma: u8,
    /// Feature support
    pub features: u8,
    /// Chromaticity coordinates
    pub chromaticity: [u8; 10],
    /// Established timings
    pub established_timings: [u8; 3],
    /// Standard timings
    pub standard_timings: [u8; 16],
    /// Detailed timing descriptors
    pub detailed_timings: [[u8; 18]; 4],
    /// Extension block count
    pub extension_count: u8,
    /// Checksum
    pub checksum: u8,
}

impl EdidBlock {
    /// Check if EDID header is valid
    pub fn is_valid(&self) -> bool {
        self.header == [0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00]
    }

    /// Get manufacturer ID as a string (3 characters)
    pub fn manufacturer(&self) -> [char; 3] {
        let raw = ((self.manufacturer_id[0] as u16) << 8) | (self.manufacturer_id[1] as u16);
        [
            (((raw >> 10) & 0x1F) as u8 + b'A' - 1) as char,
            (((raw >> 5) & 0x1F) as u8 + b'A' - 1) as char,
            ((raw & 0x1F) as u8 + b'A' - 1) as char,
        ]
    }

    /// Get year of manufacture
    pub fn year(&self) -> u16 {
        1990 + self.manufacture_year as u16
    }

    /// Check if digital display
    pub fn is_digital(&self) -> bool {
        self.video_input & 0x80 != 0
    }

    /// Get preferred timing from first detailed timing descriptor
    pub fn preferred_timing(&self) -> Option<(u16, u16, u16)> {
        let dtd = &self.detailed_timings[0];

        // Check if this is a detailed timing descriptor (not a display descriptor)
        if dtd[0] == 0 && dtd[1] == 0 {
            return None;
        }

        let h_active = ((dtd[4] as u16 & 0xF0) << 4) | (dtd[2] as u16);
        let v_active = ((dtd[7] as u16 & 0xF0) << 4) | (dtd[5] as u16);

        // Pixel clock in 10 kHz units
        let pixel_clock = (dtd[0] as u32) | ((dtd[1] as u32) << 8);

        // Calculate refresh rate
        let h_total = h_active + (((dtd[4] as u16 & 0x0F) << 8) | (dtd[3] as u16));
        let v_total = v_active + (((dtd[7] as u16 & 0x0F) << 8) | (dtd[6] as u16));

        let refresh = if h_total > 0 && v_total > 0 {
            (pixel_clock * 10000) / (h_total as u32 * v_total as u32)
        } else {
            60
        };

        Some((h_active, v_active, refresh as u16))
    }

    /// Verify checksum
    pub fn verify_checksum(&self) -> bool {
        let bytes = unsafe {
            core::slice::from_raw_parts(self as *const EdidBlock as *const u8, 128)
        };
        let sum: u8 = bytes.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
        sum == 0
    }
}

/// Global EDID storage
static EDID_DATA: Mutex<Option<EdidBlock>> = Mutex::new(None);

/// Set EDID data
pub fn set_edid(edid: EdidBlock) {
    if edid.is_valid() {
        *EDID_DATA.lock() = Some(edid);
        crate::kprintln!(
            "vbe: EDID from {}{}{}, year {}",
            edid.manufacturer()[0],
            edid.manufacturer()[1],
            edid.manufacturer()[2],
            edid.year()
        );
        if let Some((w, h, hz)) = edid.preferred_timing() {
            crate::kprintln!("vbe: preferred timing {}x{}@{}Hz", w, h, hz);
        }
    }
}

/// Get EDID data
pub fn get_edid() -> Option<EdidBlock> {
    *EDID_DATA.lock()
}

/// Get preferred resolution from EDID
pub fn preferred_resolution() -> Option<(u16, u16)> {
    let edid = EDID_DATA.lock();
    edid.as_ref().and_then(|e| e.preferred_timing()).map(|(w, h, _)| (w, h))
}

// =============================================================================
// VBE BIOS Call Structures (for reference/documentation)
// =============================================================================

/// VBE function numbers (called via INT 10h, AH=4Fh)
pub mod vbe_function {
    /// Return VBE Controller Information
    pub const GET_CONTROLLER_INFO: u16 = 0x4F00;
    /// Return VBE Mode Information
    pub const GET_MODE_INFO: u16 = 0x4F01;
    /// Set VBE Mode
    pub const SET_MODE: u16 = 0x4F02;
    /// Return Current VBE Mode
    pub const GET_CURRENT_MODE: u16 = 0x4F03;
    /// Save/Restore State
    pub const SAVE_RESTORE_STATE: u16 = 0x4F04;
    /// Display Window Control
    pub const WINDOW_CONTROL: u16 = 0x4F05;
    /// Set/Get Logical Scan Line Length
    pub const SCANLINE_LENGTH: u16 = 0x4F06;
    /// Set/Get Display Start
    pub const DISPLAY_START: u16 = 0x4F07;
    /// Set/Get DAC Palette Format
    pub const DAC_PALETTE_FORMAT: u16 = 0x4F08;
    /// Set/Get Palette Data
    pub const PALETTE_DATA: u16 = 0x4F09;
    /// Return VBE Protected Mode Interface
    pub const PROTECTED_MODE_INTERFACE: u16 = 0x4F0A;
    /// Get/Set Pixel Clock
    pub const PIXEL_CLOCK: u16 = 0x4F0B;
}

/// VBE return status codes
pub mod vbe_status {
    pub const SUCCESS: u16 = 0x004F;
    pub const FAILED: u16 = 0x014F;
    pub const NOT_SUPPORTED: u16 = 0x024F;
    pub const INVALID_IN_CURRENT_MODE: u16 = 0x034F;
}

/// Initialize VBE subsystem
pub fn init() {
    // VBE initialization is typically done via init_from_framebuffer
    // when the bootloader sets up graphics
    crate::kprintln!("vbe: module initialized");
}
