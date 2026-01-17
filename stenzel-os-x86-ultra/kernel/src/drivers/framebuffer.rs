//! Framebuffer driver for GOP/UEFI graphics
//!
//! Provides access to the linear framebuffer passed from the bootloader.
//! Supports basic drawing primitives, text rendering, and mode setting.
//!
//! Mode Setting:
//! - Reports current display mode via sysfs
//! - Supports virtual resolution (viewport) smaller than physical
//! - Supports panning (viewport offset) within the framebuffer
//! - Provides Linux-compatible FBIO ioctls

use bootloader_api::info::{FrameBufferInfo, PixelFormat};
use spin::Mutex;

/// Global framebuffer instance
static FRAMEBUFFER: Mutex<Option<FrameBufferState>> = Mutex::new(None);

/// Display mode information
#[derive(Debug, Clone, Copy)]
pub struct DisplayMode {
    /// Physical width in pixels
    pub width: u32,
    /// Physical height in pixels
    pub height: u32,
    /// Bits per pixel
    pub bits_per_pixel: u32,
    /// Refresh rate in Hz (0 if unknown)
    pub refresh_rate: u32,
    /// Pixel format identifier
    pub pixel_format: u32,
}

impl DisplayMode {
    pub const PIXEL_FORMAT_RGB: u32 = 0;
    pub const PIXEL_FORMAT_BGR: u32 = 1;
    pub const PIXEL_FORMAT_GRAY: u32 = 2;
    pub const PIXEL_FORMAT_UNKNOWN: u32 = 255;
}

/// Virtual screen info (Linux FBIOGET_VSCREENINFO compatible)
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct VarScreenInfo {
    /// Visible resolution X
    pub xres: u32,
    /// Visible resolution Y
    pub yres: u32,
    /// Virtual resolution X
    pub xres_virtual: u32,
    /// Virtual resolution Y
    pub yres_virtual: u32,
    /// Offset from virtual to visible X
    pub xoffset: u32,
    /// Offset from virtual to visible Y
    pub yoffset: u32,
    /// Bits per pixel
    pub bits_per_pixel: u32,
    /// Grayscale if != 0
    pub grayscale: u32,
    /// Red bitfield
    pub red_offset: u32,
    pub red_length: u32,
    /// Green bitfield
    pub green_offset: u32,
    pub green_length: u32,
    /// Blue bitfield
    pub blue_offset: u32,
    pub blue_length: u32,
    /// Alpha bitfield (transparency)
    pub transp_offset: u32,
    pub transp_length: u32,
    /// Non-standard pixel format
    pub nonstd: u32,
    /// Activate changes
    pub activate: u32,
    /// Height of picture in mm
    pub height_mm: u32,
    /// Width of picture in mm
    pub width_mm: u32,
    /// Pixel clock in ps
    pub pixclock: u32,
    /// Left margin
    pub left_margin: u32,
    /// Right margin
    pub right_margin: u32,
    /// Upper margin
    pub upper_margin: u32,
    /// Lower margin
    pub lower_margin: u32,
    /// Horizontal sync length
    pub hsync_len: u32,
    /// Vertical sync length
    pub vsync_len: u32,
    /// Sync flags
    pub sync: u32,
    /// Video mode flags
    pub vmode: u32,
    /// Angle of rotation (0, 90, 180, 270)
    pub rotate: u32,
    /// Colorspace
    pub colorspace: u32,
    /// Reserved
    pub reserved: [u32; 4],
}

/// Fixed screen info (Linux FBIOGET_FSCREENINFO compatible)
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct FixScreenInfo {
    /// Identification string
    pub id: [u8; 16],
    /// Start of frame buffer memory (physical address)
    pub smem_start: u64,
    /// Length of frame buffer memory
    pub smem_len: u32,
    /// Type of frame buffer
    pub fb_type: u32,
    /// Type aux for interleaved planes
    pub type_aux: u32,
    /// Visual type
    pub visual: u32,
    /// X panning step
    pub xpanstep: u16,
    /// Y panning step
    pub ypanstep: u16,
    /// Y wrapping step
    pub ywrapstep: u16,
    /// Line length (bytes)
    pub line_length: u32,
    /// Start of MMIO
    pub mmio_start: u64,
    /// Length of MMIO
    pub mmio_len: u32,
    /// Acceleration capabilities
    pub accel: u32,
    /// Capabilities flags
    pub capabilities: u16,
    /// Reserved
    pub reserved: [u16; 2],
}

// ioctl numbers (Linux compatible)
pub const FBIOGET_VSCREENINFO: u32 = 0x4600;
pub const FBIOPUT_VSCREENINFO: u32 = 0x4601;
pub const FBIOGET_FSCREENINFO: u32 = 0x4602;
pub const FBIOPAN_DISPLAY: u32 = 0x4606;
pub const FBIO_WAITFORVSYNC: u32 = 0x4620;

// FB types
pub const FB_TYPE_PACKED_PIXELS: u32 = 0;
pub const FB_TYPE_PLANES: u32 = 1;
pub const FB_TYPE_INTERLEAVED_PLANES: u32 = 2;

// FB visual types
pub const FB_VISUAL_TRUECOLOR: u32 = 2;
pub const FB_VISUAL_DIRECTCOLOR: u32 = 4;

// Activation flags
pub const FB_ACTIVATE_NOW: u32 = 0;
pub const FB_ACTIVATE_NXTOPEN: u32 = 1;
pub const FB_ACTIVATE_TEST: u32 = 2;

/// Framebuffer state
pub struct FrameBufferState {
    /// Raw buffer pointer
    buffer: &'static mut [u8],
    /// Framebuffer info
    info: FrameBufferInfo,
    /// Virtual resolution (viewport size)
    virtual_width: usize,
    virtual_height: usize,
    /// Pan offset (viewport position)
    pan_x: usize,
    pan_y: usize,
    /// Current clipping rectangle (None = no clipping, full screen)
    clip_rect: Option<ClipRect>,
}

/// Integer square root using Newton's method
fn isqrt(n: u32) -> u32 {
    if n == 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// Clipping rectangle
#[derive(Debug, Clone, Copy)]
pub struct ClipRect {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

impl ClipRect {
    pub const fn new(x: usize, y: usize, width: usize, height: usize) -> Self {
        Self { x, y, width, height }
    }

    /// Check if a point is inside the clip rect
    pub fn contains(&self, x: usize, y: usize) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }

    /// Intersect two clip rects
    pub fn intersect(&self, other: &ClipRect) -> Option<ClipRect> {
        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);
        let x2 = (self.x + self.width).min(other.x + other.width);
        let y2 = (self.y + self.height).min(other.y + other.height);

        if x1 < x2 && y1 < y2 {
            Some(ClipRect::new(x1, y1, x2 - x1, y2 - y1))
        } else {
            None
        }
    }
}

/// Color representation (RGBA)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn with_alpha(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Alias for with_alpha (more common naming)
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    // Common colors
    pub const BLACK: Color = Color::new(0, 0, 0);
    pub const WHITE: Color = Color::new(255, 255, 255);
    pub const RED: Color = Color::new(255, 0, 0);
    pub const GREEN: Color = Color::new(0, 255, 0);
    pub const BLUE: Color = Color::new(0, 0, 255);
    pub const YELLOW: Color = Color::new(255, 255, 0);
    pub const CYAN: Color = Color::new(0, 255, 255);
    pub const MAGENTA: Color = Color::new(255, 0, 255);
    pub const GRAY: Color = Color::new(128, 128, 128);
    pub const LIGHT_GRAY: Color = Color::new(192, 192, 192);
    pub const DARK_GRAY: Color = Color::new(64, 64, 64);
    pub const TRANSPARENT: Color = Color::with_alpha(0, 0, 0, 0);

    /// Blend this color over another using alpha compositing (Porter-Duff "over" operator)
    /// self is the foreground, bg is the background
    pub fn blend_over(self, bg: Color) -> Color {
        if self.a == 255 {
            return self;
        }
        if self.a == 0 {
            return bg;
        }

        let src_a = self.a as u32;
        let dst_a = bg.a as u32;
        let inv_src_a = 255 - src_a;

        // out_a = src_a + dst_a * (1 - src_a)
        let out_a = src_a + (dst_a * inv_src_a) / 255;

        if out_a == 0 {
            return Color::TRANSPARENT;
        }

        // out_rgb = (src_rgb * src_a + dst_rgb * dst_a * (1 - src_a)) / out_a
        let r = ((self.r as u32 * src_a + bg.r as u32 * dst_a * inv_src_a / 255) / out_a) as u8;
        let g = ((self.g as u32 * src_a + bg.g as u32 * dst_a * inv_src_a / 255) / out_a) as u8;
        let b = ((self.b as u32 * src_a + bg.b as u32 * dst_a * inv_src_a / 255) / out_a) as u8;

        Color::with_alpha(r, g, b, out_a as u8)
    }

    /// Simple alpha blend (assumes opaque background)
    pub fn blend_over_opaque(self, bg: Color) -> Color {
        if self.a == 255 {
            return self;
        }
        if self.a == 0 {
            return bg;
        }

        let a = self.a as u32;
        let inv_a = 255 - a;

        let r = ((self.r as u32 * a + bg.r as u32 * inv_a) / 255) as u8;
        let g = ((self.g as u32 * a + bg.g as u32 * inv_a) / 255) as u8;
        let b = ((self.b as u32 * a + bg.b as u32 * inv_a) / 255) as u8;

        Color::new(r, g, b)
    }

    /// Multiply colors (for lighting effects)
    pub fn multiply(self, other: Color) -> Color {
        Color::with_alpha(
            ((self.r as u32 * other.r as u32) / 255) as u8,
            ((self.g as u32 * other.g as u32) / 255) as u8,
            ((self.b as u32 * other.b as u32) / 255) as u8,
            ((self.a as u32 * other.a as u32) / 255) as u8,
        )
    }

    /// Lighten color
    pub fn lighten(self, amount: u8) -> Color {
        Color::with_alpha(
            self.r.saturating_add(amount),
            self.g.saturating_add(amount),
            self.b.saturating_add(amount),
            self.a,
        )
    }

    /// Darken color
    pub fn darken(self, amount: u8) -> Color {
        Color::with_alpha(
            self.r.saturating_sub(amount),
            self.g.saturating_sub(amount),
            self.b.saturating_sub(amount),
            self.a,
        )
    }

    /// Create color from HSV (hue: 0-360, saturation: 0-100, value: 0-100)
    /// Uses integer-only arithmetic suitable for no_std
    pub fn from_hsv(h: u16, s: u8, v: u8) -> Color {
        let s = s.min(100) as u32;
        let v = v.min(100) as u32;
        let h = (h % 360) as u32;

        // c = v * s / 100 (scaled to 0-100)
        let c = (v * s) / 100;

        // x = c * (1 - |h/60 mod 2 - 1|)
        // Calculate |h/60 mod 2 - 1| * 100 to keep integer
        let h_sector = h / 60;
        let h_mod = h % 60;
        let f = if h_sector % 2 == 0 {
            // Ascending in sector
            (h_mod * 100) / 60
        } else {
            // Descending in sector
            100 - (h_mod * 100) / 60
        };
        let x = (c * f) / 100;

        // m = v - c (scaled to 0-100)
        let m = v - c;

        // Assign RGB based on hue sector
        let (r, g, b) = match h_sector {
            0 => (c, x, 0),
            1 => (x, c, 0),
            2 => (0, c, x),
            3 => (0, x, c),
            4 => (x, 0, c),
            _ => (c, 0, x),
        };

        // Scale from 0-100 to 0-255
        Color::new(
            (((r + m) * 255) / 100) as u8,
            (((g + m) * 255) / 100) as u8,
            (((b + m) * 255) / 100) as u8,
        )
    }
}

impl FrameBufferState {
    /// Create a new framebuffer state from bootloader info
    pub fn new(buffer: &'static mut [u8], info: FrameBufferInfo) -> Self {
        Self {
            buffer,
            virtual_width: info.width,
            virtual_height: info.height,
            pan_x: 0,
            pan_y: 0,
            clip_rect: None,
            info,
        }
    }

    /// Set the clipping rectangle
    pub fn set_clip(&mut self, clip: Option<ClipRect>) {
        self.clip_rect = clip;
    }

    /// Get the current clipping rectangle
    pub fn get_clip(&self) -> Option<ClipRect> {
        self.clip_rect
    }

    /// Set clipping to the full screen
    pub fn reset_clip(&mut self) {
        self.clip_rect = None;
    }

    /// Get the effective clip rect (either set clip or full screen)
    pub fn effective_clip(&self) -> ClipRect {
        self.clip_rect.unwrap_or(ClipRect::new(0, 0, self.info.width, self.info.height))
    }

    /// Check if a point is within the current clip region
    pub fn is_clipped(&self, x: usize, y: usize) -> bool {
        if x >= self.info.width || y >= self.info.height {
            return true;
        }
        if let Some(ref clip) = self.clip_rect {
            !clip.contains(x, y)
        } else {
            false
        }
    }

    /// Get physical framebuffer width in pixels
    pub fn width(&self) -> usize {
        self.info.width
    }

    /// Get physical framebuffer height in pixels
    pub fn height(&self) -> usize {
        self.info.height
    }

    /// Get virtual (viewport) width
    pub fn virtual_width(&self) -> usize {
        self.virtual_width
    }

    /// Get virtual (viewport) height
    pub fn virtual_height(&self) -> usize {
        self.virtual_height
    }

    /// Get pan X offset
    pub fn pan_x(&self) -> usize {
        self.pan_x
    }

    /// Get pan Y offset
    pub fn pan_y(&self) -> usize {
        self.pan_y
    }

    /// Set virtual resolution (viewport size)
    /// Returns true if successful, false if invalid
    pub fn set_virtual_size(&mut self, width: usize, height: usize) -> bool {
        if width == 0 || height == 0 || width > self.info.width || height > self.info.height {
            return false;
        }
        // Adjust pan if needed
        if self.pan_x + width > self.info.width {
            self.pan_x = self.info.width - width;
        }
        if self.pan_y + height > self.info.height {
            self.pan_y = self.info.height - height;
        }
        self.virtual_width = width;
        self.virtual_height = height;
        true
    }

    /// Set pan offset (viewport position)
    /// Returns true if successful, false if invalid
    pub fn set_pan(&mut self, x: usize, y: usize) -> bool {
        if x + self.virtual_width > self.info.width || y + self.virtual_height > self.info.height {
            return false;
        }
        self.pan_x = x;
        self.pan_y = y;
        true
    }

    /// Get bytes per pixel
    pub fn bytes_per_pixel(&self) -> usize {
        self.info.bytes_per_pixel
    }

    /// Get stride (pixels per row, including padding)
    pub fn stride(&self) -> usize {
        self.info.stride
    }

    /// Get pixel format
    pub fn pixel_format(&self) -> PixelFormat {
        self.info.pixel_format
    }

    /// Get current display mode
    pub fn current_mode(&self) -> DisplayMode {
        let pixel_format = match self.info.pixel_format {
            PixelFormat::Rgb => DisplayMode::PIXEL_FORMAT_RGB,
            PixelFormat::Bgr => DisplayMode::PIXEL_FORMAT_BGR,
            PixelFormat::U8 => DisplayMode::PIXEL_FORMAT_GRAY,
            _ => DisplayMode::PIXEL_FORMAT_UNKNOWN,
        };
        DisplayMode {
            width: self.info.width as u32,
            height: self.info.height as u32,
            bits_per_pixel: (self.info.bytes_per_pixel * 8) as u32,
            refresh_rate: 60, // Assume 60Hz (GOP doesn't provide this info)
            pixel_format,
        }
    }

    /// Get variable screen info (for ioctl)
    pub fn get_var_screen_info(&self) -> VarScreenInfo {
        let (red_offset, green_offset, blue_offset) = match self.info.pixel_format {
            PixelFormat::Rgb => (0, 8, 16),
            PixelFormat::Bgr => (16, 8, 0),
            PixelFormat::Unknown { red_position, green_position, blue_position } => {
                (red_position as u32, green_position as u32, blue_position as u32)
            }
            _ => (0, 8, 16),
        };

        VarScreenInfo {
            xres: self.info.width as u32,
            yres: self.info.height as u32,
            xres_virtual: self.virtual_width as u32,
            yres_virtual: self.virtual_height as u32,
            xoffset: self.pan_x as u32,
            yoffset: self.pan_y as u32,
            bits_per_pixel: (self.info.bytes_per_pixel * 8) as u32,
            grayscale: if matches!(self.info.pixel_format, PixelFormat::U8) { 1 } else { 0 },
            red_offset,
            red_length: 8,
            green_offset,
            green_length: 8,
            blue_offset,
            blue_length: 8,
            transp_offset: 24,
            transp_length: if self.info.bytes_per_pixel > 3 { 8 } else { 0 },
            nonstd: 0,
            activate: FB_ACTIVATE_NOW,
            height_mm: 0,  // Unknown
            width_mm: 0,   // Unknown
            pixclock: 0,   // Unknown
            left_margin: 0,
            right_margin: 0,
            upper_margin: 0,
            lower_margin: 0,
            hsync_len: 0,
            vsync_len: 0,
            sync: 0,
            vmode: 0,
            rotate: 0,
            colorspace: 0,
            reserved: [0; 4],
        }
    }

    /// Set variable screen info (for ioctl)
    /// Only virtual size and pan are modifiable
    pub fn set_var_screen_info(&mut self, info: &VarScreenInfo) -> bool {
        // Can't change physical resolution
        if info.xres != self.info.width as u32 || info.yres != self.info.height as u32 {
            return false;
        }
        // Can't change bits per pixel
        if info.bits_per_pixel != (self.info.bytes_per_pixel * 8) as u32 {
            return false;
        }
        // Set virtual size and pan
        if !self.set_virtual_size(info.xres_virtual as usize, info.yres_virtual as usize) {
            return false;
        }
        if !self.set_pan(info.xoffset as usize, info.yoffset as usize) {
            return false;
        }
        true
    }

    /// Get fixed screen info (for ioctl)
    pub fn get_fix_screen_info(&self) -> FixScreenInfo {
        let mut id = [0u8; 16];
        let name = b"StenzelOS FB";
        id[..name.len()].copy_from_slice(name);

        FixScreenInfo {
            id,
            smem_start: self.buffer.as_ptr() as u64,
            smem_len: self.buffer.len() as u32,
            fb_type: FB_TYPE_PACKED_PIXELS,
            type_aux: 0,
            visual: FB_VISUAL_TRUECOLOR,
            xpanstep: 1,
            ypanstep: 1,
            ywrapstep: 0,
            line_length: (self.info.stride * self.info.bytes_per_pixel) as u32,
            mmio_start: 0,
            mmio_len: 0,
            accel: 0, // No hardware acceleration
            capabilities: 0,
            reserved: [0; 2],
        }
    }

    /// Calculate byte offset for a pixel at (x, y)
    fn pixel_offset(&self, x: usize, y: usize) -> usize {
        (y * self.info.stride + x) * self.info.bytes_per_pixel
    }

    /// Set a pixel at (x, y) with the given color (respects clipping)
    pub fn set_pixel(&mut self, x: usize, y: usize, color: Color) {
        if self.is_clipped(x, y) {
            return;
        }

        let offset = self.pixel_offset(x, y);
        let bpp = self.info.bytes_per_pixel;

        if offset + bpp > self.buffer.len() {
            return;
        }

        match self.info.pixel_format {
            PixelFormat::Rgb => {
                self.buffer[offset] = color.r;
                self.buffer[offset + 1] = color.g;
                self.buffer[offset + 2] = color.b;
                if bpp > 3 {
                    self.buffer[offset + 3] = color.a;
                }
            }
            PixelFormat::Bgr => {
                self.buffer[offset] = color.b;
                self.buffer[offset + 1] = color.g;
                self.buffer[offset + 2] = color.r;
                if bpp > 3 {
                    self.buffer[offset + 3] = color.a;
                }
            }
            PixelFormat::U8 => {
                // Grayscale - use luminance formula
                let gray = ((color.r as u16 * 77 + color.g as u16 * 150 + color.b as u16 * 29) / 256) as u8;
                self.buffer[offset] = gray;
            }
            PixelFormat::Unknown { red_position, green_position, blue_position } => {
                // Assuming 32-bit pixels
                let mut pixel: u32 = 0;
                pixel |= (color.r as u32) << red_position;
                pixel |= (color.g as u32) << green_position;
                pixel |= (color.b as u32) << blue_position;
                let bytes = pixel.to_ne_bytes();
                for i in 0..bpp.min(4) {
                    self.buffer[offset + i] = bytes[i];
                }
            }
            _ => {
                // Future pixel formats - default to RGB
                self.buffer[offset] = color.r;
                self.buffer[offset + 1] = color.g;
                self.buffer[offset + 2] = color.b;
            }
        }
    }

    /// Set a pixel with alpha blending (blends over existing pixel)
    pub fn set_pixel_alpha(&mut self, x: usize, y: usize, color: Color) {
        if color.a == 255 {
            self.set_pixel(x, y, color);
            return;
        }
        if color.a == 0 {
            return; // Fully transparent, nothing to draw
        }

        // Get background color
        if let Some(bg) = self.get_pixel(x, y) {
            let blended = color.blend_over_opaque(bg);
            self.set_pixel(x, y, blended);
        }
    }

    /// Get pixel color at (x, y)
    pub fn get_pixel(&self, x: usize, y: usize) -> Option<Color> {
        if x >= self.info.width || y >= self.info.height {
            return None;
        }

        let offset = self.pixel_offset(x, y);
        let bpp = self.info.bytes_per_pixel;

        if offset + bpp > self.buffer.len() {
            return None;
        }

        let color = match self.info.pixel_format {
            PixelFormat::Rgb => Color::new(
                self.buffer[offset],
                self.buffer[offset + 1],
                self.buffer[offset + 2],
            ),
            PixelFormat::Bgr => Color::new(
                self.buffer[offset + 2],
                self.buffer[offset + 1],
                self.buffer[offset],
            ),
            PixelFormat::U8 => {
                let gray = self.buffer[offset];
                Color::new(gray, gray, gray)
            }
            _ => Color::BLACK,
        };

        Some(color)
    }

    /// Fill the entire screen with a color
    pub fn clear(&mut self, color: Color) {
        // Optimize for common case: BGR with 4 bytes per pixel
        if matches!(self.info.pixel_format, PixelFormat::Bgr) && self.info.bytes_per_pixel == 4 {
            let pixel = [color.b, color.g, color.r, color.a];
            for chunk in self.buffer.chunks_exact_mut(4) {
                chunk.copy_from_slice(&pixel);
            }
        } else {
            for y in 0..self.info.height {
                for x in 0..self.info.width {
                    self.set_pixel(x, y, color);
                }
            }
        }
    }

    /// Draw a filled rectangle
    pub fn fill_rect(&mut self, x: usize, y: usize, width: usize, height: usize, color: Color) {
        let x_end = (x + width).min(self.info.width);
        let y_end = (y + height).min(self.info.height);

        for py in y..y_end {
            for px in x..x_end {
                self.set_pixel(px, py, color);
            }
        }
    }

    /// Draw a horizontal line
    pub fn hline(&mut self, x: usize, y: usize, width: usize, color: Color) {
        let x_end = (x + width).min(self.info.width);
        for px in x..x_end {
            self.set_pixel(px, y, color);
        }
    }

    /// Draw a vertical line
    pub fn vline(&mut self, x: usize, y: usize, height: usize, color: Color) {
        let y_end = (y + height).min(self.info.height);
        for py in y..y_end {
            self.set_pixel(x, py, color);
        }
    }

    /// Draw a rectangle outline
    pub fn draw_rect(&mut self, x: usize, y: usize, width: usize, height: usize, color: Color) {
        self.hline(x, y, width, color);
        self.hline(x, y + height.saturating_sub(1), width, color);
        self.vline(x, y, height, color);
        self.vline(x + width.saturating_sub(1), y, height, color);
    }

    /// Draw a line using Bresenham's algorithm
    /// Supports lines in any direction (all octants)
    pub fn draw_line(&mut self, x0: isize, y0: isize, x1: isize, y1: isize, color: Color) {
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx: isize = if x0 < x1 { 1 } else { -1 };
        let sy: isize = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        let mut x = x0;
        let mut y = y0;

        loop {
            // Only draw if within bounds
            if x >= 0 && y >= 0 {
                let ux = x as usize;
                let uy = y as usize;
                if ux < self.info.width && uy < self.info.height {
                    self.set_pixel(ux, uy, color);
                }
            }

            if x == x1 && y == y1 {
                break;
            }

            let e2 = 2 * err;
            if e2 >= dy {
                if x == x1 {
                    break;
                }
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                if y == y1 {
                    break;
                }
                err += dx;
                y += sy;
            }
        }
    }

    /// Draw a line with thickness (anti-aliased would require more work)
    pub fn draw_thick_line(&mut self, x0: isize, y0: isize, x1: isize, y1: isize, thickness: usize, color: Color) {
        if thickness <= 1 {
            self.draw_line(x0, y0, x1, y1, color);
            return;
        }

        // Calculate perpendicular offset for thickness
        let dx = x1 - x0;
        let dy = y1 - y0;
        let len_sq = (dx * dx + dy * dy) as u32;
        let len = isqrt(len_sq) as isize;

        if len == 0 {
            // Point, draw a filled circle instead
            self.fill_circle(x0, y0, thickness as isize / 2, color);
            return;
        }

        // Perpendicular direction (scaled by len to avoid division)
        let px = -dy;
        let py = dx;

        // Draw multiple parallel lines using integer arithmetic
        let half = thickness as isize / 2;
        for i in 0..thickness as isize {
            let offset = i - half;
            // Scale offset by perpendicular direction, divide by len
            let ox = (px * offset) / len;
            let oy = (py * offset) / len;
            self.draw_line(x0 + ox, y0 + oy, x1 + ox, y1 + oy, color);
        }
    }

    /// Draw a circle using Bresenham's midpoint circle algorithm
    pub fn draw_circle(&mut self, cx: isize, cy: isize, radius: isize, color: Color) {
        if radius <= 0 {
            if cx >= 0 && cy >= 0 {
                self.set_pixel(cx as usize, cy as usize, color);
            }
            return;
        }

        let mut x = radius;
        let mut y: isize = 0;
        let mut err = 1 - radius;

        while x >= y {
            // Draw 8 octants
            self.set_pixel_signed(cx + x, cy + y, color);
            self.set_pixel_signed(cx - x, cy + y, color);
            self.set_pixel_signed(cx + x, cy - y, color);
            self.set_pixel_signed(cx - x, cy - y, color);
            self.set_pixel_signed(cx + y, cy + x, color);
            self.set_pixel_signed(cx - y, cy + x, color);
            self.set_pixel_signed(cx + y, cy - x, color);
            self.set_pixel_signed(cx - y, cy - x, color);

            y += 1;
            if err < 0 {
                err += 2 * y + 1;
            } else {
                x -= 1;
                err += 2 * (y - x) + 1;
            }
        }
    }

    /// Draw a filled circle
    pub fn fill_circle(&mut self, cx: isize, cy: isize, radius: isize, color: Color) {
        if radius <= 0 {
            if cx >= 0 && cy >= 0 {
                self.set_pixel(cx as usize, cy as usize, color);
            }
            return;
        }

        let mut x = radius;
        let mut y: isize = 0;
        let mut err = 1 - radius;

        while x >= y {
            // Draw horizontal lines for filled circle
            self.hline_signed(cx - x, cy + y, (2 * x + 1) as usize, color);
            self.hline_signed(cx - x, cy - y, (2 * x + 1) as usize, color);
            self.hline_signed(cx - y, cy + x, (2 * y + 1) as usize, color);
            self.hline_signed(cx - y, cy - x, (2 * y + 1) as usize, color);

            y += 1;
            if err < 0 {
                err += 2 * y + 1;
            } else {
                x -= 1;
                err += 2 * (y - x) + 1;
            }
        }
    }

    /// Draw an ellipse using Bresenham's algorithm
    pub fn draw_ellipse(&mut self, cx: isize, cy: isize, rx: isize, ry: isize, color: Color) {
        if rx <= 0 || ry <= 0 {
            if cx >= 0 && cy >= 0 {
                self.set_pixel(cx as usize, cy as usize, color);
            }
            return;
        }

        let rx2 = rx * rx;
        let ry2 = ry * ry;
        let two_rx2 = 2 * rx2;
        let two_ry2 = 2 * ry2;

        let mut x: isize = 0;
        let mut y = ry;
        let mut px: isize = 0;
        let mut py = two_rx2 * y;

        // Region 1
        let mut p = ry2 - rx2 * ry + rx2 / 4;
        while px < py {
            self.set_pixel_signed(cx + x, cy + y, color);
            self.set_pixel_signed(cx - x, cy + y, color);
            self.set_pixel_signed(cx + x, cy - y, color);
            self.set_pixel_signed(cx - x, cy - y, color);

            x += 1;
            px += two_ry2;
            if p < 0 {
                p += ry2 + px;
            } else {
                y -= 1;
                py -= two_rx2;
                p += ry2 + px - py;
            }
        }

        // Region 2
        p = ry2 * (x * x + x) + rx2 * (y - 1) * (y - 1) - rx2 * ry2;
        while y >= 0 {
            self.set_pixel_signed(cx + x, cy + y, color);
            self.set_pixel_signed(cx - x, cy + y, color);
            self.set_pixel_signed(cx + x, cy - y, color);
            self.set_pixel_signed(cx - x, cy - y, color);

            y -= 1;
            py -= two_rx2;
            if p > 0 {
                p += rx2 - py;
            } else {
                x += 1;
                px += two_ry2;
                p += rx2 - py + px;
            }
        }
    }

    /// Draw a filled ellipse
    pub fn fill_ellipse(&mut self, cx: isize, cy: isize, rx: isize, ry: isize, color: Color) {
        if rx <= 0 || ry <= 0 {
            if cx >= 0 && cy >= 0 {
                self.set_pixel(cx as usize, cy as usize, color);
            }
            return;
        }

        // Use the ellipse equation: (x/rx)^2 + (y/ry)^2 = 1
        // At each y, x = rx * sqrt(1 - (y/ry)^2) = rx * sqrt(ry^2 - y^2) / ry
        let rx2 = (rx * rx) as u32;
        let ry2 = (ry * ry) as u32;

        for y in -ry..=ry {
            let y2 = (y * y) as u32;
            // x_extent = rx * sqrt(ry^2 - y^2) / ry
            let inner = ry2.saturating_sub(y2);
            let x_extent = ((rx as u32 * isqrt(inner)) / (ry as u32)) as isize;
            self.hline_signed(cx - x_extent, cy + y, (2 * x_extent + 1) as usize, color);
        }
    }

    /// Helper: set pixel with signed coordinates (clips to bounds)
    fn set_pixel_signed(&mut self, x: isize, y: isize, color: Color) {
        if x >= 0 && y >= 0 {
            let ux = x as usize;
            let uy = y as usize;
            if ux < self.info.width && uy < self.info.height {
                self.set_pixel(ux, uy, color);
            }
        }
    }

    /// Helper: horizontal line with signed start coordinate
    fn hline_signed(&mut self, x: isize, y: isize, width: usize, color: Color) {
        if y < 0 || y >= self.info.height as isize {
            return;
        }
        let start_x = x.max(0) as usize;
        let end_x = ((x + width as isize) as usize).min(self.info.width);
        if start_x < end_x {
            for px in start_x..end_x {
                self.set_pixel(px, y as usize, color);
            }
        }
    }

    /// Copy a region of pixels
    pub fn blit(&mut self, src: &[u8], src_width: usize, dst_x: usize, dst_y: usize, width: usize, height: usize) {
        let bpp = self.info.bytes_per_pixel;
        for row in 0..height {
            let dy = dst_y + row;
            if dy >= self.info.height {
                break;
            }

            for col in 0..width {
                let dx = dst_x + col;
                if dx >= self.info.width {
                    break;
                }

                let src_offset = (row * src_width + col) * bpp;
                if src_offset + bpp <= src.len() {
                    let dst_offset = self.pixel_offset(dx, dy);
                    if dst_offset + bpp <= self.buffer.len() {
                        self.buffer[dst_offset..dst_offset + bpp]
                            .copy_from_slice(&src[src_offset..src_offset + bpp]);
                    }
                }
            }
        }
    }

    /// Get raw buffer slice (for direct access)
    pub fn buffer(&self) -> &[u8] {
        self.buffer
    }

    /// Get mutable raw buffer slice
    pub fn buffer_mut(&mut self) -> &mut [u8] {
        self.buffer
    }

    /// Get framebuffer info
    pub fn info(&self) -> FrameBufferInfo {
        self.info
    }

    // ========================================================================
    // Text Rendering Methods
    // ========================================================================

    /// Draw a single character using the default bitmap font
    /// Returns the width of the drawn character
    pub fn draw_char(&mut self, x: usize, y: usize, c: char, fg: Color, bg: Option<Color>) -> usize {
        use super::font::DEFAULT_FONT;

        let font = &DEFAULT_FONT;

        // Draw background if specified
        if let Some(bg_color) = bg {
            self.fill_rect(x, y, font.width, font.height, bg_color);
        }

        // Draw the glyph
        if let Some(glyph) = font.get_glyph(c) {
            for row in 0..font.height {
                let byte = glyph[row];
                for col in 0..font.width {
                    if (byte >> (font.width - 1 - col)) & 1 != 0 {
                        self.set_pixel(x + col, y + row, fg);
                    }
                }
            }
        } else {
            // Draw replacement character (filled box outline)
            for row in 0..font.height {
                for col in 0..font.width {
                    if row == 0 || row == font.height - 1 || col == 0 || col == font.width - 1 {
                        self.set_pixel(x + col, y + row, fg);
                    }
                }
            }
        }

        font.width
    }

    /// Draw a string using the default bitmap font
    /// Handles newlines by moving to the next line
    /// Returns the final (x, y) cursor position
    pub fn draw_string(&mut self, x: usize, y: usize, s: &str, fg: Color, bg: Option<Color>) -> (usize, usize) {
        use super::font::DEFAULT_FONT;

        let font = &DEFAULT_FONT;
        let mut cur_x = x;
        let mut cur_y = y;

        for c in s.chars() {
            if c == '\n' {
                cur_x = x;
                cur_y += font.height;
                continue;
            }
            if c == '\r' {
                cur_x = x;
                continue;
            }
            if c == '\t' {
                // Tab: move to next 8-character boundary
                let tab_width = 8 * font.width;
                cur_x = ((cur_x / tab_width) + 1) * tab_width;
                continue;
            }

            // Check if we need to wrap
            if cur_x + font.width > self.width() {
                cur_x = x;
                cur_y += font.height;
            }

            // Stop if we're off the bottom of the screen
            if cur_y + font.height > self.height() {
                break;
            }

            cur_x += self.draw_char(cur_x, cur_y, c, fg, bg);
        }

        (cur_x, cur_y)
    }

    /// Draw text with alignment within a bounding box
    pub fn draw_text_aligned(
        &mut self,
        x: usize,
        y: usize,
        width: usize,
        text: &str,
        align: super::font::TextAlign,
        fg: Color,
        bg: Option<Color>,
    ) {
        use super::font::{DEFAULT_FONT, TextAlign};

        let font = &DEFAULT_FONT;
        let text_width = font.measure_string(text);

        let start_x = match align {
            TextAlign::Left => x,
            TextAlign::Center => {
                if text_width < width {
                    x + (width - text_width) / 2
                } else {
                    x
                }
            }
            TextAlign::Right => {
                if text_width < width {
                    x + width - text_width
                } else {
                    x
                }
            }
        };

        self.draw_string(start_x, y, text, fg, bg);
    }

    /// Draw text with a shadow effect
    pub fn draw_text_shadowed(
        &mut self,
        x: usize,
        y: usize,
        text: &str,
        fg: Color,
        shadow: Color,
        offset: usize,
    ) {
        // Draw shadow first (offset down and right)
        self.draw_string(x + offset, y + offset, text, shadow, None);
        // Draw main text on top
        self.draw_string(x, y, text, fg, None);
    }

    /// Get font metrics (width, height)
    pub fn font_metrics(&self) -> (usize, usize) {
        use super::font::DEFAULT_FONT;
        (DEFAULT_FONT.width, DEFAULT_FONT.height)
    }

    /// Calculate how many characters fit in a given width
    pub fn chars_per_width(&self, width: usize) -> usize {
        use super::font::DEFAULT_FONT;
        width / DEFAULT_FONT.width
    }

    /// Calculate how many lines fit in a given height
    pub fn lines_per_height(&self, height: usize) -> usize {
        use super::font::DEFAULT_FONT;
        height / DEFAULT_FONT.height
    }

    /// Get the maximum characters per line on the screen
    pub fn chars_per_line(&self) -> usize {
        self.chars_per_width(self.width())
    }

    /// Get the maximum lines on the screen
    pub fn max_lines(&self) -> usize {
        self.lines_per_height(self.height())
    }
}

/// Initialize the framebuffer from bootloader info
pub fn init(framebuffer: bootloader_api::info::FrameBuffer) {
    let info = framebuffer.info();
    let buffer = framebuffer.into_buffer();

    let state = FrameBufferState::new(buffer, info);

    crate::kprintln!(
        "framebuffer: {}x{}, {}bpp, {:?}",
        info.width,
        info.height,
        info.bytes_per_pixel * 8,
        info.pixel_format
    );

    *FRAMEBUFFER.lock() = Some(state);
}

/// Check if framebuffer is available
pub fn is_available() -> bool {
    FRAMEBUFFER.lock().is_some()
}

/// Get framebuffer dimensions
pub fn dimensions() -> Option<(usize, usize)> {
    let fb = FRAMEBUFFER.lock();
    fb.as_ref().map(|s| (s.width(), s.height()))
}

/// Get framebuffer info
pub fn info() -> Option<FrameBufferInfo> {
    let fb = FRAMEBUFFER.lock();
    fb.as_ref().map(|s| s.info())
}

/// Clear the framebuffer with a color
pub fn clear(color: Color) {
    let mut fb = FRAMEBUFFER.lock();
    if let Some(ref mut state) = *fb {
        state.clear(color);
    }
}

/// Set a pixel
pub fn set_pixel(x: usize, y: usize, color: Color) {
    let mut fb = FRAMEBUFFER.lock();
    if let Some(ref mut state) = *fb {
        state.set_pixel(x, y, color);
    }
}

/// Fill a rectangle
pub fn fill_rect(x: usize, y: usize, width: usize, height: usize, color: Color) {
    let mut fb = FRAMEBUFFER.lock();
    if let Some(ref mut state) = *fb {
        state.fill_rect(x, y, width, height, color);
    }
}

/// Draw a rectangle outline
pub fn draw_rect(x: usize, y: usize, width: usize, height: usize, color: Color) {
    let mut fb = FRAMEBUFFER.lock();
    if let Some(ref mut state) = *fb {
        state.draw_rect(x, y, width, height, color);
    }
}

/// Execute a function with mutable access to the framebuffer state
pub fn with_framebuffer<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut FrameBufferState) -> R,
{
    let mut fb = FRAMEBUFFER.lock();
    fb.as_mut().map(f)
}

/// Draw a test pattern to verify framebuffer is working
pub fn draw_test_pattern() {
    let mut fb = FRAMEBUFFER.lock();
    if let Some(ref mut state) = *fb {
        let width = state.width();
        let height = state.height();

        // Clear to dark blue
        state.clear(Color::new(0, 0, 64));

        // Draw color bars
        let bar_height = height / 8;
        state.fill_rect(0, 0, width, bar_height, Color::RED);
        state.fill_rect(0, bar_height, width, bar_height, Color::GREEN);
        state.fill_rect(0, bar_height * 2, width, bar_height, Color::BLUE);
        state.fill_rect(0, bar_height * 3, width, bar_height, Color::YELLOW);
        state.fill_rect(0, bar_height * 4, width, bar_height, Color::CYAN);
        state.fill_rect(0, bar_height * 5, width, bar_height, Color::MAGENTA);
        state.fill_rect(0, bar_height * 6, width, bar_height, Color::WHITE);
        state.fill_rect(0, bar_height * 7, width, bar_height, Color::GRAY);

        // Draw a border
        state.draw_rect(10, 10, width - 20, height - 20, Color::WHITE);
    }
}

// ============================================================================
// Text Rendering API
// ============================================================================

/// Draw a character at the specified position
pub fn draw_char(x: usize, y: usize, c: char, fg: Color, bg: Option<Color>) -> usize {
    let mut fb = FRAMEBUFFER.lock();
    if let Some(ref mut state) = *fb {
        state.draw_char(x, y, c, fg, bg)
    } else {
        0
    }
}

/// Draw a string at the specified position
/// Returns the final cursor position (x, y)
pub fn draw_string(x: usize, y: usize, s: &str, fg: Color, bg: Option<Color>) -> (usize, usize) {
    let mut fb = FRAMEBUFFER.lock();
    if let Some(ref mut state) = *fb {
        state.draw_string(x, y, s, fg, bg)
    } else {
        (x, y)
    }
}

/// Draw text with alignment
pub fn draw_text_aligned(
    x: usize,
    y: usize,
    width: usize,
    text: &str,
    align: super::font::TextAlign,
    fg: Color,
    bg: Option<Color>,
) {
    let mut fb = FRAMEBUFFER.lock();
    if let Some(ref mut state) = *fb {
        state.draw_text_aligned(x, y, width, text, align, fg, bg);
    }
}

/// Draw text with shadow effect
pub fn draw_text_shadowed(x: usize, y: usize, text: &str, fg: Color, shadow: Color, offset: usize) {
    let mut fb = FRAMEBUFFER.lock();
    if let Some(ref mut state) = *fb {
        state.draw_text_shadowed(x, y, text, fg, shadow, offset);
    }
}

/// Get font metrics (width, height)
pub fn font_metrics() -> (usize, usize) {
    let fb = FRAMEBUFFER.lock();
    if let Some(ref state) = *fb {
        state.font_metrics()
    } else {
        (8, 16) // Default
    }
}

/// Get the number of text columns on screen
pub fn text_columns() -> usize {
    let fb = FRAMEBUFFER.lock();
    if let Some(ref state) = *fb {
        state.chars_per_line()
    } else {
        80
    }
}

/// Get the number of text rows on screen
pub fn text_rows() -> usize {
    let fb = FRAMEBUFFER.lock();
    if let Some(ref state) = *fb {
        state.max_lines()
    } else {
        25
    }
}

// ============================================================================
// Mode Setting API
// ============================================================================

/// Get the current display mode
pub fn current_mode() -> Option<DisplayMode> {
    let fb = FRAMEBUFFER.lock();
    fb.as_ref().map(|s| s.current_mode())
}

/// Get virtual screen dimensions
pub fn virtual_dimensions() -> Option<(usize, usize)> {
    let fb = FRAMEBUFFER.lock();
    fb.as_ref().map(|s| (s.virtual_width(), s.virtual_height()))
}

/// Get pan offset
pub fn pan_offset() -> Option<(usize, usize)> {
    let fb = FRAMEBUFFER.lock();
    fb.as_ref().map(|s| (s.pan_x(), s.pan_y()))
}

/// Set virtual screen size (viewport)
pub fn set_virtual_size(width: usize, height: usize) -> bool {
    let mut fb = FRAMEBUFFER.lock();
    if let Some(ref mut state) = *fb {
        state.set_virtual_size(width, height)
    } else {
        false
    }
}

/// Set pan offset (viewport position)
pub fn set_pan(x: usize, y: usize) -> bool {
    let mut fb = FRAMEBUFFER.lock();
    if let Some(ref mut state) = *fb {
        state.set_pan(x, y)
    } else {
        false
    }
}

/// Get variable screen info (for ioctl compatibility)
pub fn get_var_screen_info() -> Option<VarScreenInfo> {
    let fb = FRAMEBUFFER.lock();
    fb.as_ref().map(|s| s.get_var_screen_info())
}

/// Set variable screen info (for ioctl compatibility)
pub fn set_var_screen_info(info: &VarScreenInfo) -> bool {
    let mut fb = FRAMEBUFFER.lock();
    if let Some(ref mut state) = *fb {
        state.set_var_screen_info(info)
    } else {
        false
    }
}

/// Get fixed screen info (for ioctl compatibility)
pub fn get_fix_screen_info() -> Option<FixScreenInfo> {
    let fb = FRAMEBUFFER.lock();
    fb.as_ref().map(|s| s.get_fix_screen_info())
}

/// Handle framebuffer ioctl
/// Returns (result, output_data) where result is 0 on success, -errno on error
pub fn ioctl(cmd: u32, arg: usize) -> (i32, Option<alloc::vec::Vec<u8>>) {
    match cmd {
        FBIOGET_VSCREENINFO => {
            if let Some(info) = get_var_screen_info() {
                let bytes = unsafe {
                    core::slice::from_raw_parts(
                        &info as *const VarScreenInfo as *const u8,
                        core::mem::size_of::<VarScreenInfo>()
                    )
                };
                (0, Some(bytes.to_vec()))
            } else {
                (-19, None) // ENODEV
            }
        }
        FBIOPUT_VSCREENINFO => {
            // arg should point to VarScreenInfo struct
            // In real implementation, copy from user space
            // For now, just acknowledge
            if is_available() {
                (0, None)
            } else {
                (-19, None) // ENODEV
            }
        }
        FBIOGET_FSCREENINFO => {
            if let Some(info) = get_fix_screen_info() {
                let bytes = unsafe {
                    core::slice::from_raw_parts(
                        &info as *const FixScreenInfo as *const u8,
                        core::mem::size_of::<FixScreenInfo>()
                    )
                };
                (0, Some(bytes.to_vec()))
            } else {
                (-19, None) // ENODEV
            }
        }
        FBIOPAN_DISPLAY => {
            // Pan is already handled via VarScreenInfo
            if is_available() {
                (0, None)
            } else {
                (-19, None)
            }
        }
        FBIO_WAITFORVSYNC => {
            // No vsync support, return immediately
            if is_available() {
                (0, None)
            } else {
                (-19, None)
            }
        }
        _ => (-22, None) // EINVAL - invalid ioctl
    }
}

/// Get mode string for sysfs (e.g., "U:1280x720p-60")
pub fn mode_string() -> Option<alloc::string::String> {
    let mode = current_mode()?;
    Some(alloc::format!(
        "U:{}x{}p-{}",
        mode.width,
        mode.height,
        mode.refresh_rate
    ))
}

/// Get bits per pixel string for sysfs
pub fn bits_per_pixel_string() -> Option<alloc::string::String> {
    let mode = current_mode()?;
    Some(alloc::format!("{}", mode.bits_per_pixel))
}

/// Get resolution string for sysfs (e.g., "1280,720")
pub fn resolution_string() -> Option<alloc::string::String> {
    let mode = current_mode()?;
    Some(alloc::format!("{},{}", mode.width, mode.height))
}

/// Get virtual size string for sysfs
pub fn virtual_size_string() -> Option<alloc::string::String> {
    let (w, h) = virtual_dimensions()?;
    Some(alloc::format!("{},{}", w, h))
}

/// Get pan string for sysfs
pub fn pan_string() -> Option<alloc::string::String> {
    let (x, y) = pan_offset()?;
    Some(alloc::format!("{},{}", x, y))
}

/// Get stride string for sysfs
pub fn stride_string() -> Option<alloc::string::String> {
    let fb = FRAMEBUFFER.lock();
    fb.as_ref().map(|s| alloc::format!("{}", s.stride() * s.bytes_per_pixel()))
}
