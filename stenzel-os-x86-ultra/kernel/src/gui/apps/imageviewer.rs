//! Image Viewer Application
//!
//! A simple image viewer for displaying BMP, and basic image formats.

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use crate::gui::widgets::{Widget, WidgetId, Bounds, WidgetEvent, MouseButton, theme};

/// Supported image formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Bmp,
    Ppm,
    Raw,
    Unknown,
}

/// Image data structure
#[derive(Debug, Clone)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub format: ImageFormat,
    pub pixels: Vec<Color>,
}

impl Image {
    /// Create an empty image
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            format: ImageFormat::Raw,
            pixels: vec![Color::BLACK; width * height],
        }
    }

    /// Get pixel at (x, y)
    pub fn get_pixel(&self, x: usize, y: usize) -> Option<Color> {
        if x < self.width && y < self.height {
            Some(self.pixels[y * self.width + x])
        } else {
            None
        }
    }

    /// Set pixel at (x, y)
    pub fn set_pixel(&mut self, x: usize, y: usize, color: Color) {
        if x < self.width && y < self.height {
            self.pixels[y * self.width + x] = color;
        }
    }

    /// Load BMP image from bytes
    pub fn from_bmp(data: &[u8]) -> Result<Self, &'static str> {
        if data.len() < 54 {
            return Err("BMP file too small");
        }

        // Check BMP signature
        if data[0] != b'B' || data[1] != b'M' {
            return Err("Not a valid BMP file");
        }

        // Read header
        let data_offset = u32::from_le_bytes([data[10], data[11], data[12], data[13]]) as usize;
        let width = i32::from_le_bytes([data[18], data[19], data[20], data[21]]) as usize;
        let height_raw = i32::from_le_bytes([data[22], data[23], data[24], data[25]]);
        let bits_per_pixel = u16::from_le_bytes([data[28], data[29]]) as usize;

        // Handle negative height (top-down bitmap)
        let (height, top_down) = if height_raw < 0 {
            ((-height_raw) as usize, true)
        } else {
            (height_raw as usize, false)
        };

        if width == 0 || height == 0 {
            return Err("Invalid image dimensions");
        }

        // We support 24-bit and 32-bit BMPs
        if bits_per_pixel != 24 && bits_per_pixel != 32 {
            return Err("Unsupported BMP bit depth (only 24/32-bit supported)");
        }

        let bytes_per_pixel = bits_per_pixel / 8;
        let row_size = ((bits_per_pixel * width + 31) / 32) * 4; // Row padding to 4-byte boundary

        let mut image = Image::new(width, height);
        image.format = ImageFormat::Bmp;

        for row in 0..height {
            let y = if top_down { row } else { height - 1 - row };
            let row_start = data_offset + row * row_size;

            for col in 0..width {
                let pixel_start = row_start + col * bytes_per_pixel;

                if pixel_start + bytes_per_pixel > data.len() {
                    continue;
                }

                // BMP stores as BGR(A)
                let b = data[pixel_start];
                let g = data[pixel_start + 1];
                let r = data[pixel_start + 2];
                let a = if bytes_per_pixel == 4 {
                    data[pixel_start + 3]
                } else {
                    255
                };

                image.set_pixel(col, y, Color::with_alpha(r, g, b, a));
            }
        }

        Ok(image)
    }

    /// Load PPM (P6) image from bytes
    pub fn from_ppm(data: &[u8]) -> Result<Self, &'static str> {
        if data.len() < 10 {
            return Err("PPM file too small");
        }

        // Check PPM signature (P6 = binary RGB)
        if data[0] != b'P' || data[1] != b'6' {
            return Err("Not a valid PPM P6 file");
        }

        // Parse header (simple parser, whitespace separated)
        let mut pos = 2;
        let mut values: Vec<usize> = Vec::new();

        // Skip whitespace and comments, read width, height, maxval
        while values.len() < 3 && pos < data.len() {
            // Skip whitespace
            while pos < data.len() && (data[pos] == b' ' || data[pos] == b'\n' || data[pos] == b'\r' || data[pos] == b'\t') {
                pos += 1;
            }

            // Skip comments
            if pos < data.len() && data[pos] == b'#' {
                while pos < data.len() && data[pos] != b'\n' {
                    pos += 1;
                }
                continue;
            }

            // Read number
            let mut num = 0usize;
            while pos < data.len() && data[pos] >= b'0' && data[pos] <= b'9' {
                num = num * 10 + (data[pos] - b'0') as usize;
                pos += 1;
            }
            values.push(num);
        }

        if values.len() < 3 {
            return Err("Invalid PPM header");
        }

        let width = values[0];
        let height = values[1];
        let _maxval = values[2];

        // Skip single whitespace after maxval
        pos += 1;

        if width == 0 || height == 0 {
            return Err("Invalid image dimensions");
        }

        let mut image = Image::new(width, height);
        image.format = ImageFormat::Ppm;

        // Read pixel data (RGB triplets)
        for y in 0..height {
            for x in 0..width {
                if pos + 2 < data.len() {
                    let r = data[pos];
                    let g = data[pos + 1];
                    let b = data[pos + 2];
                    image.set_pixel(x, y, Color::new(r, g, b));
                    pos += 3;
                }
            }
        }

        Ok(image)
    }

    /// Try to load image from bytes, detecting format
    pub fn from_bytes(data: &[u8]) -> Result<Self, &'static str> {
        if data.len() < 2 {
            return Err("Data too small");
        }

        // Try to detect format
        if data[0] == b'B' && data[1] == b'M' {
            return Self::from_bmp(data);
        } else if data[0] == b'P' && data[1] == b'6' {
            return Self::from_ppm(data);
        }

        Err("Unknown image format")
    }
}

/// Zoom mode for display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomMode {
    /// Fit entire image in view
    FitToWindow,
    /// Show at actual size (100%)
    ActualSize,
    /// Custom zoom percentage
    Custom(u32),
}

/// Image viewer state
pub struct ImageViewer {
    id: WidgetId,
    bounds: Bounds,
    image: Option<Image>,
    file_path: Option<String>,
    zoom_mode: ZoomMode,
    zoom_percent: u32,
    pan_x: isize,
    pan_y: isize,
    dragging: bool,
    drag_start_x: isize,
    drag_start_y: isize,
    drag_pan_x: isize,
    drag_pan_y: isize,
    enabled: bool,
    visible: bool,
    show_info: bool,
    background_color: Color,
}

impl ImageViewer {
    pub fn new(x: isize, y: isize, width: usize, height: usize) -> Self {
        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(x, y, width, height),
            image: None,
            file_path: None,
            zoom_mode: ZoomMode::FitToWindow,
            zoom_percent: 100,
            pan_x: 0,
            pan_y: 0,
            dragging: false,
            drag_start_x: 0,
            drag_start_y: 0,
            drag_pan_x: 0,
            drag_pan_y: 0,
            enabled: true,
            visible: true,
            show_info: true,
            background_color: Color::new(30, 30, 35),
        }
    }

    /// Load image from file bytes
    pub fn load_image(&mut self, data: &[u8], path: Option<&str>) -> Result<(), &'static str> {
        let image = Image::from_bytes(data)?;
        self.image = Some(image);
        self.file_path = path.map(String::from);
        self.reset_view();
        Ok(())
    }

    /// Load image directly
    pub fn set_image(&mut self, image: Image) {
        self.image = Some(image);
        self.reset_view();
    }

    /// Clear image
    pub fn clear(&mut self) {
        self.image = None;
        self.file_path = None;
        self.reset_view();
    }

    /// Reset view to default
    pub fn reset_view(&mut self) {
        self.pan_x = 0;
        self.pan_y = 0;
        self.update_zoom();
    }

    /// Get current image
    pub fn image(&self) -> Option<&Image> {
        self.image.as_ref()
    }

    /// Set zoom mode
    pub fn set_zoom_mode(&mut self, mode: ZoomMode) {
        self.zoom_mode = mode;
        self.update_zoom();
    }

    /// Set zoom percentage directly
    pub fn set_zoom(&mut self, percent: u32) {
        self.zoom_mode = ZoomMode::Custom(percent);
        self.zoom_percent = percent.clamp(10, 1000);
    }

    /// Zoom in
    pub fn zoom_in(&mut self) {
        let new_zoom = (self.zoom_percent + 25).min(1000);
        self.set_zoom(new_zoom);
    }

    /// Zoom out
    pub fn zoom_out(&mut self) {
        let new_zoom = self.zoom_percent.saturating_sub(25).max(10);
        self.set_zoom(new_zoom);
    }

    /// Toggle fit to window
    pub fn toggle_fit_to_window(&mut self) {
        if self.zoom_mode == ZoomMode::FitToWindow {
            self.zoom_mode = ZoomMode::ActualSize;
        } else {
            self.zoom_mode = ZoomMode::FitToWindow;
        }
        self.update_zoom();
    }

    /// Show/hide info overlay
    pub fn set_show_info(&mut self, show: bool) {
        self.show_info = show;
    }

    /// Set background color
    pub fn set_background_color(&mut self, color: Color) {
        self.background_color = color;
    }

    /// Update zoom based on mode
    fn update_zoom(&mut self) {
        match self.zoom_mode {
            ZoomMode::FitToWindow => {
                if let Some(ref img) = self.image {
                    let zoom_x = (self.bounds.width * 100) / img.width.max(1);
                    let zoom_y = (self.bounds.height * 100) / img.height.max(1);
                    self.zoom_percent = zoom_x.min(zoom_y).max(1) as u32;
                }
            }
            ZoomMode::ActualSize => {
                self.zoom_percent = 100;
            }
            ZoomMode::Custom(pct) => {
                self.zoom_percent = pct;
            }
        }
    }

    /// Get displayed image dimensions
    fn displayed_size(&self) -> (usize, usize) {
        if let Some(ref img) = self.image {
            let w = (img.width * self.zoom_percent as usize) / 100;
            let h = (img.height * self.zoom_percent as usize) / 100;
            (w.max(1), h.max(1))
        } else {
            (0, 0)
        }
    }
}

// Helper drawing functions
fn draw_string(surface: &mut Surface, x: isize, y: isize, text: &str, color: Color) {
    use crate::drivers::font::DEFAULT_FONT;

    if x < 0 || y < 0 {
        return;
    }

    let mut cx = x as usize;
    let cy = y as usize;

    for c in text.chars() {
        if let Some(glyph) = DEFAULT_FONT.get_glyph(c) {
            for row in 0..DEFAULT_FONT.height {
                let byte = glyph[row];
                for col in 0..DEFAULT_FONT.width {
                    if (byte >> (DEFAULT_FONT.width - 1 - col)) & 1 != 0 {
                        surface.set_pixel(cx + col, cy + row, color);
                    }
                }
            }
        }
        cx += DEFAULT_FONT.width;
    }
}

fn fill_rect_safe(surface: &mut Surface, x: isize, y: isize, width: usize, height: usize, color: Color) {
    if x < 0 || y < 0 || width == 0 || height == 0 {
        return;
    }
    surface.fill_rect(x as usize, y as usize, width, height, color);
}

impl Widget for ImageViewer {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn bounds(&self) -> Bounds {
        self.bounds
    }

    fn set_position(&mut self, x: isize, y: isize) {
        self.bounds.x = x;
        self.bounds.y = y;
    }

    fn set_size(&mut self, width: usize, height: usize) {
        self.bounds.width = width;
        self.bounds.height = height;
        self.update_zoom();
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn handle_event(&mut self, event: &WidgetEvent) -> bool {
        if !self.enabled || !self.visible {
            return false;
        }

        match event {
            WidgetEvent::MouseDown { button: MouseButton::Left, x, y } => {
                if self.bounds.contains(*x, *y) {
                    self.dragging = true;
                    self.drag_start_x = *x;
                    self.drag_start_y = *y;
                    self.drag_pan_x = self.pan_x;
                    self.drag_pan_y = self.pan_y;
                    return true;
                }
            }
            WidgetEvent::MouseUp { button: MouseButton::Left, .. } => {
                if self.dragging {
                    self.dragging = false;
                    return true;
                }
            }
            WidgetEvent::MouseMove { x, y } => {
                if self.dragging {
                    self.pan_x = self.drag_pan_x + (*x - self.drag_start_x);
                    self.pan_y = self.drag_pan_y + (*y - self.drag_start_y);
                    return true;
                }
            }
            WidgetEvent::Scroll { delta_y, .. } => {
                if *delta_y > 0 {
                    self.zoom_in();
                } else if *delta_y < 0 {
                    self.zoom_out();
                }
                return true;
            }
            WidgetEvent::KeyDown { key, .. } => {
                match *key {
                    0x0D => { // + key
                        self.zoom_in();
                        return true;
                    }
                    0x0C => { // - key
                        self.zoom_out();
                        return true;
                    }
                    0x13 => { // 0 key - actual size
                        self.set_zoom_mode(ZoomMode::ActualSize);
                        return true;
                    }
                    0x21 => { // F key - fit to window
                        self.toggle_fit_to_window();
                        return true;
                    }
                    0x17 => { // I key - toggle info
                        self.show_info = !self.show_info;
                        return true;
                    }
                    0x48 => { // Up arrow - pan up
                        self.pan_y += 20;
                        return true;
                    }
                    0x50 => { // Down arrow - pan down
                        self.pan_y -= 20;
                        return true;
                    }
                    0x4B => { // Left arrow - pan left
                        self.pan_x += 20;
                        return true;
                    }
                    0x4D => { // Right arrow - pan right
                        self.pan_x -= 20;
                        return true;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        false
    }

    fn render(&self, surface: &mut Surface) {
        if !self.visible {
            return;
        }

        let theme = theme();

        // Draw background
        fill_rect_safe(surface, self.bounds.x, self.bounds.y,
                       self.bounds.width, self.bounds.height, self.background_color);

        // Draw image if loaded
        if let Some(ref img) = self.image {
            let (disp_w, disp_h) = self.displayed_size();

            // Calculate centered position with pan offset
            let center_x = self.bounds.x + (self.bounds.width as isize) / 2;
            let center_y = self.bounds.y + (self.bounds.height as isize) / 2;
            let img_x = center_x - (disp_w as isize) / 2 + self.pan_x;
            let img_y = center_y - (disp_h as isize) / 2 + self.pan_y;

            // Draw scaled image (simple nearest neighbor)
            let scale_x = img.width as f32 / disp_w as f32;
            let scale_y = img.height as f32 / disp_h as f32;

            for dy in 0..disp_h {
                let screen_y = img_y + dy as isize;
                if screen_y < self.bounds.y || screen_y >= self.bounds.y + (self.bounds.height as isize) {
                    continue;
                }

                let src_y = ((dy as f32) * scale_y) as usize;
                if src_y >= img.height {
                    continue;
                }

                for dx in 0..disp_w {
                    let screen_x = img_x + dx as isize;
                    if screen_x < self.bounds.x || screen_x >= self.bounds.x + (self.bounds.width as isize) {
                        continue;
                    }

                    let src_x = ((dx as f32) * scale_x) as usize;
                    if src_x >= img.width {
                        continue;
                    }

                    if let Some(color) = img.get_pixel(src_x, src_y) {
                        if screen_x >= 0 && screen_y >= 0 {
                            surface.set_pixel(screen_x as usize, screen_y as usize, color);
                        }
                    }
                }
            }

            // Draw info overlay
            if self.show_info {
                let info_bg = Color::with_alpha(0, 0, 0, 180);
                let info_y = self.bounds.y + self.bounds.height as isize - 50;
                fill_rect_safe(surface, self.bounds.x, info_y, self.bounds.width, 50, info_bg);

                // Image dimensions
                let dims = alloc::format!("{}x{}", img.width, img.height);
                draw_string(surface, self.bounds.x + 10, info_y + 8, &dims, theme.fg);

                // Zoom level
                let zoom_str = alloc::format!("Zoom: {}%", self.zoom_percent);
                draw_string(surface, self.bounds.x + 10, info_y + 26, &zoom_str, theme.fg);

                // Format
                let format_str = match img.format {
                    ImageFormat::Bmp => "BMP",
                    ImageFormat::Ppm => "PPM",
                    ImageFormat::Raw => "RAW",
                    ImageFormat::Unknown => "?",
                };
                draw_string(surface, self.bounds.x + 150, info_y + 8, format_str, Color::new(150, 150, 150));

                // File path if available
                if let Some(ref path) = self.file_path {
                    let display_path = if path.len() > 40 {
                        alloc::format!("...{}", &path[path.len() - 37..])
                    } else {
                        path.clone()
                    };
                    draw_string(surface, self.bounds.x + 150, info_y + 26, &display_path, Color::new(150, 150, 150));
                }
            }
        } else {
            // No image loaded - show placeholder
            let text = "No image loaded";
            let text_w = text.len() * 8;
            let text_x = self.bounds.x + (self.bounds.width as isize - text_w as isize) / 2;
            let text_y = self.bounds.y + (self.bounds.height as isize) / 2 - 8;
            draw_string(surface, text_x, text_y, text, Color::new(100, 100, 100));

            let hint = "Drag image here or press O to open";
            let hint_w = hint.len() * 8;
            let hint_x = self.bounds.x + (self.bounds.width as isize - hint_w as isize) / 2;
            draw_string(surface, hint_x, text_y + 20, hint, Color::new(80, 80, 80));
        }
    }
}

/// Create a test pattern image for testing
pub fn create_test_image(width: usize, height: usize) -> Image {
    let mut image = Image::new(width, height);

    for y in 0..height {
        for x in 0..width {
            // Gradient + checkerboard pattern
            let r = ((x * 255) / width.max(1)) as u8;
            let g = ((y * 255) / height.max(1)) as u8;
            let b = (((x + y) * 128) / (width + height).max(1)) as u8;

            // Add checkerboard
            let checker = ((x / 16) + (y / 16)) % 2 == 0;
            let color = if checker {
                Color::new(r, g, b)
            } else {
                Color::new(r / 2, g / 2, b / 2)
            };

            image.set_pixel(x, y, color);
        }
    }

    image
}
