//! Screenshot Tool
//!
//! Screen capture utility with region selection, window capture, and delay options.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use spin::Mutex;
use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use crate::gui::widgets::{Widget, WidgetId, WidgetEvent, Bounds, MouseButton};
use crate::security::Cred;

/// Global screenshot state
static SCREENSHOT_STATE: Mutex<Option<ScreenshotState>> = Mutex::new(None);

/// Screenshot state
pub struct ScreenshotState {
    /// Save directory
    pub save_directory: String,
    /// Default filename format
    pub filename_format: String,
    /// Default image format
    pub default_format: ImageFormat,
    /// Include cursor in screenshots
    pub include_cursor: bool,
    /// Play sound on capture
    pub play_sound: bool,
    /// Copy to clipboard
    pub copy_to_clipboard: bool,
    /// Show notification
    pub show_notification: bool,
    /// Recent screenshots
    pub recent: Vec<ScreenshotInfo>,
    /// Current capture mode
    pub capture_mode: CaptureMode,
    /// Delay before capture (seconds)
    pub delay: u32,
    /// Selection state for region capture
    pub selection: Option<SelectionState>,
}

/// Image format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    Jpeg,
    Bmp,
    Webp,
}

impl ImageFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            ImageFormat::Png => "png",
            ImageFormat::Jpeg => "jpg",
            ImageFormat::Bmp => "bmp",
            ImageFormat::Webp => "webp",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            ImageFormat::Png => "PNG",
            ImageFormat::Jpeg => "JPEG",
            ImageFormat::Bmp => "BMP",
            ImageFormat::Webp => "WebP",
        }
    }

    pub fn mime_type(&self) -> &'static str {
        match self {
            ImageFormat::Png => "image/png",
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::Bmp => "image/bmp",
            ImageFormat::Webp => "image/webp",
        }
    }
}

/// Capture mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureMode {
    /// Capture entire screen
    FullScreen,
    /// Capture active window
    ActiveWindow,
    /// Capture selected region
    Region,
    /// Capture specific window
    Window,
    /// Capture all monitors
    AllMonitors,
}

impl CaptureMode {
    pub fn name(&self) -> &'static str {
        match self {
            CaptureMode::FullScreen => "Full Screen",
            CaptureMode::ActiveWindow => "Active Window",
            CaptureMode::Region => "Select Region",
            CaptureMode::Window => "Select Window",
            CaptureMode::AllMonitors => "All Monitors",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            CaptureMode::FullScreen => "Capture the entire screen",
            CaptureMode::ActiveWindow => "Capture the currently active window",
            CaptureMode::Region => "Select a rectangular region to capture",
            CaptureMode::Window => "Click on a window to capture it",
            CaptureMode::AllMonitors => "Capture all connected monitors",
        }
    }
}

/// Selection state for region capture
#[derive(Debug, Clone, Copy)]
pub struct SelectionState {
    /// Start X coordinate
    pub start_x: i32,
    /// Start Y coordinate
    pub start_y: i32,
    /// End X coordinate
    pub end_x: i32,
    /// End Y coordinate
    pub end_y: i32,
    /// Is selecting
    pub selecting: bool,
}

impl SelectionState {
    /// Get normalized bounds (top-left to bottom-right)
    pub fn bounds(&self) -> (i32, i32, i32, i32) {
        let x1 = self.start_x.min(self.end_x);
        let y1 = self.start_y.min(self.end_y);
        let x2 = self.start_x.max(self.end_x);
        let y2 = self.start_y.max(self.end_y);
        (x1, y1, x2 - x1, y2 - y1)
    }

    /// Get width
    pub fn width(&self) -> i32 {
        (self.end_x - self.start_x).abs()
    }

    /// Get height
    pub fn height(&self) -> i32 {
        (self.end_y - self.start_y).abs()
    }
}

/// Screenshot info
#[derive(Debug, Clone)]
pub struct ScreenshotInfo {
    /// File path
    pub path: String,
    /// Filename
    pub filename: String,
    /// Capture timestamp
    pub timestamp: u64,
    /// Image format
    pub format: ImageFormat,
    /// Width
    pub width: u32,
    /// Height
    pub height: u32,
    /// File size in bytes
    pub file_size: u64,
    /// Capture mode used
    pub capture_mode: CaptureMode,
}

/// Screenshot error
#[derive(Debug, Clone)]
pub enum ScreenshotError {
    NotInitialized,
    CaptureError(String),
    SaveError(String),
    InvalidRegion,
    NoWindowSelected,
    ClipboardError,
    EncodingError(String),
}

/// Initialize screenshot system
pub fn init() {
    let mut state = SCREENSHOT_STATE.lock();
    if state.is_some() {
        return;
    }

    *state = Some(ScreenshotState {
        save_directory: "/home/user/Pictures/Screenshots".to_string(),
        filename_format: "Screenshot_%Y-%m-%d_%H-%M-%S".to_string(),
        default_format: ImageFormat::Png,
        include_cursor: false,
        play_sound: true,
        copy_to_clipboard: true,
        show_notification: true,
        recent: Vec::new(),
        capture_mode: CaptureMode::FullScreen,
        delay: 0,
        selection: None,
    });

    crate::kprintln!("screenshot: initialized");
}

/// Set save directory
pub fn set_save_directory(path: &str) {
    let mut state = SCREENSHOT_STATE.lock();
    if let Some(ref mut s) = *state {
        s.save_directory = path.to_string();
    }
}

/// Get save directory
pub fn get_save_directory() -> String {
    let state = SCREENSHOT_STATE.lock();
    state.as_ref().map(|s| s.save_directory.clone()).unwrap_or_default()
}

/// Set capture mode
pub fn set_capture_mode(mode: CaptureMode) {
    let mut state = SCREENSHOT_STATE.lock();
    if let Some(ref mut s) = *state {
        s.capture_mode = mode;
    }
}

/// Get capture mode
pub fn get_capture_mode() -> CaptureMode {
    let state = SCREENSHOT_STATE.lock();
    state.as_ref().map(|s| s.capture_mode).unwrap_or(CaptureMode::FullScreen)
}

/// Set delay before capture
pub fn set_delay(seconds: u32) {
    let mut state = SCREENSHOT_STATE.lock();
    if let Some(ref mut s) = *state {
        s.delay = seconds;
    }
}

/// Set image format
pub fn set_format(format: ImageFormat) {
    let mut state = SCREENSHOT_STATE.lock();
    if let Some(ref mut s) = *state {
        s.default_format = format;
    }
}

/// Set include cursor
pub fn set_include_cursor(include: bool) {
    let mut state = SCREENSHOT_STATE.lock();
    if let Some(ref mut s) = *state {
        s.include_cursor = include;
    }
}

/// Set copy to clipboard
pub fn set_copy_to_clipboard(copy: bool) {
    let mut state = SCREENSHOT_STATE.lock();
    if let Some(ref mut s) = *state {
        s.copy_to_clipboard = copy;
    }
}

/// Set show notification
pub fn set_show_notification(show: bool) {
    let mut state = SCREENSHOT_STATE.lock();
    if let Some(ref mut s) = *state {
        s.show_notification = show;
    }
}

/// Take a screenshot with current settings
pub fn take_screenshot() -> Result<ScreenshotInfo, ScreenshotError> {
    let (mode, delay, format, include_cursor, copy_clipboard, show_notif, save_dir) = {
        let state = SCREENSHOT_STATE.lock();
        let s = state.as_ref().ok_or(ScreenshotError::NotInitialized)?;
        (s.capture_mode, s.delay, s.default_format, s.include_cursor,
         s.copy_to_clipboard, s.show_notification, s.save_directory.clone())
    };

    // Apply delay if set
    if delay > 0 {
        crate::kprintln!("screenshot: waiting {} seconds...", delay);
    }

    // Capture based on mode
    let (pixels, width, height) = capture_screen(mode, include_cursor)?;

    // Generate filename
    let timestamp = crate::time::uptime_secs();
    let filename = generate_filename(timestamp, format);
    let path = alloc::format!("{}/{}", save_dir, filename);

    // Encode and save
    let file_size = save_screenshot(&pixels, width, height, format, &path)?;

    // Copy to clipboard if enabled
    if copy_clipboard {
        let _ = copy_to_clipboard_impl(&pixels, width, height);
    }

    // Create screenshot info
    let info = ScreenshotInfo {
        path: path.clone(),
        filename,
        timestamp,
        format,
        width,
        height,
        file_size,
        capture_mode: mode,
    };

    // Add to recent list
    {
        let mut state = SCREENSHOT_STATE.lock();
        if let Some(ref mut s) = *state {
            s.recent.insert(0, info.clone());
            if s.recent.len() > 50 {
                s.recent.pop();
            }
        }
    }

    // Show notification if enabled
    if show_notif {
        crate::kprintln!("screenshot: captured {} ({}x{})", path, width, height);
    }

    Ok(info)
}

/// Take screenshot of full screen
pub fn capture_fullscreen() -> Result<ScreenshotInfo, ScreenshotError> {
    set_capture_mode(CaptureMode::FullScreen);
    take_screenshot()
}

/// Take screenshot of active window
pub fn capture_active_window() -> Result<ScreenshotInfo, ScreenshotError> {
    set_capture_mode(CaptureMode::ActiveWindow);
    take_screenshot()
}

/// Start region selection
pub fn start_region_selection() {
    let mut state = SCREENSHOT_STATE.lock();
    if let Some(ref mut s) = *state {
        s.capture_mode = CaptureMode::Region;
        s.selection = Some(SelectionState {
            start_x: 0,
            start_y: 0,
            end_x: 0,
            end_y: 0,
            selecting: false,
        });
    }
}

/// Update region selection
pub fn update_selection(x: i32, y: i32, is_start: bool) {
    let mut state = SCREENSHOT_STATE.lock();
    if let Some(ref mut s) = *state {
        if let Some(ref mut sel) = s.selection {
            if is_start {
                sel.start_x = x;
                sel.start_y = y;
                sel.end_x = x;
                sel.end_y = y;
                sel.selecting = true;
            } else {
                sel.end_x = x;
                sel.end_y = y;
            }
        }
    }
}

/// Complete region selection and capture
pub fn capture_region() -> Result<ScreenshotInfo, ScreenshotError> {
    let selection = {
        let mut state = SCREENSHOT_STATE.lock();
        let s = state.as_mut().ok_or(ScreenshotError::NotInitialized)?;
        let sel = s.selection.take().ok_or(ScreenshotError::InvalidRegion)?;
        sel
    };

    if selection.width() < 1 || selection.height() < 1 {
        return Err(ScreenshotError::InvalidRegion);
    }

    // Capture the selected region
    let (x, y, w, h) = selection.bounds();
    capture_region_internal(x, y, w, h)
}

/// Capture a specific region
fn capture_region_internal(x: i32, y: i32, width: i32, height: i32) -> Result<ScreenshotInfo, ScreenshotError> {
    let (format, save_dir) = {
        let state = SCREENSHOT_STATE.lock();
        let s = state.as_ref().ok_or(ScreenshotError::NotInitialized)?;
        (s.default_format, s.save_directory.clone())
    };

    // Get framebuffer data for region
    let (pixels, w, h) = capture_framebuffer_region(x, y, width as u32, height as u32)?;

    // Generate filename
    let timestamp = crate::time::uptime_secs();
    let filename = generate_filename(timestamp, format);
    let path = alloc::format!("{}/{}", save_dir, filename);

    // Save
    let file_size = save_screenshot(&pixels, w, h, format, &path)?;

    let info = ScreenshotInfo {
        path: path.clone(),
        filename,
        timestamp,
        format,
        width: w,
        height: h,
        file_size,
        capture_mode: CaptureMode::Region,
    };

    // Add to recent
    {
        let mut state = SCREENSHOT_STATE.lock();
        if let Some(ref mut s) = *state {
            s.recent.insert(0, info.clone());
            if s.recent.len() > 50 {
                s.recent.pop();
            }
        }
    }

    Ok(info)
}

/// Internal capture implementation
fn capture_screen(mode: CaptureMode, _include_cursor: bool) -> Result<(Vec<u8>, u32, u32), ScreenshotError> {
    match mode {
        CaptureMode::FullScreen | CaptureMode::AllMonitors => {
            capture_framebuffer()
        }
        CaptureMode::ActiveWindow => {
            // Get active window bounds from window manager
            if let Some(bounds) = get_active_window_bounds() {
                capture_framebuffer_region(bounds.0 as i32, bounds.1 as i32, bounds.2, bounds.3)
            } else {
                // Fall back to full screen
                capture_framebuffer()
            }
        }
        CaptureMode::Window => {
            Err(ScreenshotError::NoWindowSelected)
        }
        CaptureMode::Region => {
            Err(ScreenshotError::InvalidRegion)
        }
    }
}

/// Capture entire framebuffer
fn capture_framebuffer() -> Result<(Vec<u8>, u32, u32), ScreenshotError> {
    // Get screen dimensions from compositor
    let (width, height) = crate::gui::compositor::screen_size()
        .ok_or_else(|| ScreenshotError::CaptureError("No framebuffer available".to_string()))?;

    // Read framebuffer pixels using with_framebuffer
    let pixels = crate::drivers::framebuffer::with_framebuffer(|fb| {
        fb.buffer().to_vec()
    }).ok_or_else(|| ScreenshotError::CaptureError("Cannot access framebuffer".to_string()))?;

    Ok((pixels, width as u32, height as u32))
}

/// Capture region of framebuffer
fn capture_framebuffer_region(x: i32, y: i32, width: u32, height: u32) -> Result<(Vec<u8>, u32, u32), ScreenshotError> {
    // Clamp to screen bounds
    let (screen_w, screen_h) = crate::gui::compositor::screen_size()
        .ok_or_else(|| ScreenshotError::CaptureError("No framebuffer available".to_string()))?;

    let x = x.max(0) as usize;
    let y = y.max(0) as usize;
    let width = (width as usize).min(screen_w.saturating_sub(x));
    let height = (height as usize).min(screen_h.saturating_sub(y));

    if width == 0 || height == 0 {
        return Err(ScreenshotError::InvalidRegion);
    }

    // Read framebuffer region using with_framebuffer
    let result = crate::drivers::framebuffer::with_framebuffer(|fb| {
        let fb_info = fb.info();
        let bpp = fb_info.bytes_per_pixel;
        let stride = fb_info.stride;
        let mut pixels = Vec::with_capacity(width * height * 4);
        let buffer = fb.buffer();

        for row in 0..height {
            let src_y = y + row;
            for col in 0..width {
                let src_x = x + col;
                let offset = src_y * stride + src_x * bpp;
                if offset + bpp <= buffer.len() {
                    // Read BGRA and convert to RGBA
                    pixels.push(buffer[offset + 2]); // R
                    pixels.push(buffer[offset + 1]); // G
                    pixels.push(buffer[offset]);     // B
                    pixels.push(if bpp >= 4 { buffer[offset + 3] } else { 255 }); // A
                }
            }
        }
        pixels
    }).ok_or_else(|| ScreenshotError::CaptureError("Cannot access framebuffer".to_string()))?;

    Ok((result, width as u32, height as u32))
}

/// Get active window bounds
fn get_active_window_bounds() -> Option<(u32, u32, u32, u32)> {
    // TODO: Get from window manager
    None
}

/// Generate filename from timestamp and format
fn generate_filename(timestamp: u64, format: ImageFormat) -> String {
    alloc::format!("Screenshot_{}.{}", timestamp, format.extension())
}

/// Save screenshot to file
fn save_screenshot(pixels: &[u8], width: u32, height: u32, format: ImageFormat, path: &str) -> Result<u64, ScreenshotError> {
    // Encode based on format
    let data = match format {
        ImageFormat::Png => encode_png(pixels, width, height)?,
        ImageFormat::Jpeg => encode_bmp(pixels, width, height)?, // Fall back to BMP
        ImageFormat::Bmp => encode_bmp(pixels, width, height)?,
        ImageFormat::Webp => encode_bmp(pixels, width, height)?, // Fall back to BMP
    };

    // Write to file using root credentials
    let cred = Cred::root();
    crate::fs::write_file(path, &cred, crate::fs::vfs::Mode::from_bits_truncate(0o644), &data)
        .map_err(|e| ScreenshotError::SaveError(alloc::format!("{:?}", e)))?;

    Ok(data.len() as u64)
}

/// Encode as PNG (simplified)
fn encode_png(pixels: &[u8], width: u32, height: u32) -> Result<Vec<u8>, ScreenshotError> {
    let mut data = Vec::new();

    // PNG signature
    data.extend_from_slice(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);

    // IHDR chunk
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.push(8);  // bit depth
    ihdr.push(2);  // color type (RGB)
    ihdr.push(0);  // compression
    ihdr.push(0);  // filter
    ihdr.push(0);  // interlace

    write_png_chunk(&mut data, b"IHDR", &ihdr);

    // IDAT chunk (uncompressed for simplicity)
    let mut idat = Vec::new();
    for y in 0..height {
        idat.push(0); // filter byte
        for x in 0..width {
            let idx = ((y * width + x) * 4) as usize;
            if idx + 2 < pixels.len() {
                idat.push(pixels[idx]);     // R
                idat.push(pixels[idx + 1]); // G
                idat.push(pixels[idx + 2]); // B
            }
        }
    }

    write_png_chunk(&mut data, b"IDAT", &idat);

    // IEND chunk
    write_png_chunk(&mut data, b"IEND", &[]);

    Ok(data)
}

/// Write PNG chunk
fn write_png_chunk(data: &mut Vec<u8>, chunk_type: &[u8; 4], chunk_data: &[u8]) {
    let length = chunk_data.len() as u32;
    data.extend_from_slice(&length.to_be_bytes());
    data.extend_from_slice(chunk_type);
    data.extend_from_slice(chunk_data);

    // CRC32 placeholder
    let crc: u32 = 0;
    data.extend_from_slice(&crc.to_be_bytes());
}

/// Encode as BMP
fn encode_bmp(pixels: &[u8], width: u32, height: u32) -> Result<Vec<u8>, ScreenshotError> {
    let row_size = ((width * 3 + 3) / 4) * 4;
    let pixel_data_size = row_size * height;
    let file_size = 54 + pixel_data_size;

    let mut data = Vec::with_capacity(file_size as usize);

    // BMP header (14 bytes)
    data.extend_from_slice(b"BM");
    data.extend_from_slice(&file_size.to_le_bytes());
    data.extend_from_slice(&[0u8; 4]); // Reserved
    data.extend_from_slice(&54u32.to_le_bytes()); // Pixel data offset

    // DIB header (40 bytes)
    data.extend_from_slice(&40u32.to_le_bytes());
    data.extend_from_slice(&(width as i32).to_le_bytes());
    data.extend_from_slice(&(-(height as i32)).to_le_bytes()); // Negative for top-down
    data.extend_from_slice(&1u16.to_le_bytes()); // Planes
    data.extend_from_slice(&24u16.to_le_bytes()); // Bits per pixel
    data.extend_from_slice(&0u32.to_le_bytes()); // Compression
    data.extend_from_slice(&pixel_data_size.to_le_bytes());
    data.extend_from_slice(&2835u32.to_le_bytes()); // X pixels per meter
    data.extend_from_slice(&2835u32.to_le_bytes()); // Y pixels per meter
    data.extend_from_slice(&0u32.to_le_bytes()); // Colors in palette
    data.extend_from_slice(&0u32.to_le_bytes()); // Important colors

    // Pixel data (BGR format)
    for y in 0..height {
        for x in 0..width {
            let idx = ((y * width + x) * 4) as usize;
            if idx + 2 < pixels.len() {
                data.push(pixels[idx + 2]); // B
                data.push(pixels[idx + 1]); // G
                data.push(pixels[idx]);     // R
            } else {
                data.extend_from_slice(&[0, 0, 0]);
            }
        }
        // Padding to 4-byte boundary
        let padding = (row_size - width * 3) as usize;
        for _ in 0..padding {
            data.push(0);
        }
    }

    Ok(data)
}

/// Copy screenshot to clipboard
fn copy_to_clipboard_impl(_pixels: &[u8], _width: u32, _height: u32) -> Result<(), ScreenshotError> {
    // TODO: Implement clipboard support
    Ok(())
}

/// Get recent screenshots
pub fn get_recent_screenshots() -> Vec<ScreenshotInfo> {
    let state = SCREENSHOT_STATE.lock();
    state.as_ref().map(|s| s.recent.clone()).unwrap_or_default()
}

/// Clear recent screenshots list
pub fn clear_recent() {
    let mut state = SCREENSHOT_STATE.lock();
    if let Some(ref mut s) = *state {
        s.recent.clear();
    }
}

// Theme colors (simple defaults)
fn window_background() -> Color { Color::new(45, 45, 45) }
fn _text_color() -> Color { Color::new(240, 240, 240) }
fn accent_color() -> Color { Color::new(0, 120, 215) }
fn button_color() -> Color { Color::new(60, 60, 60) }

/// Screenshot tool widget
pub struct ScreenshotWidget {
    id: WidgetId,
    bounds: Bounds,
    selected_mode: CaptureMode,
    delay_seconds: u32,
    include_cursor: bool,
    enabled: bool,
    visible: bool,
}

impl ScreenshotWidget {
    pub fn new(id: WidgetId, bounds: Bounds) -> Self {
        Self {
            id,
            bounds,
            selected_mode: CaptureMode::FullScreen,
            delay_seconds: 0,
            include_cursor: false,
            enabled: true,
            visible: true,
        }
    }
}

impl Widget for ScreenshotWidget {
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

    fn render(&self, surface: &mut Surface) {
        let x = self.bounds.x as usize;
        let y = self.bounds.y as usize;
        let w = self.bounds.width;
        let h = self.bounds.height;

        // Background
        surface.fill_rect(x, y, w, h, window_background());

        // Mode selection buttons
        let modes = [
            CaptureMode::FullScreen,
            CaptureMode::ActiveWindow,
            CaptureMode::Region,
        ];

        for (i, &mode) in modes.iter().enumerate() {
            let btn_x = x + 20 + i * 130;
            let btn_y = y + 60;
            let is_selected = self.selected_mode == mode;
            let bg = if is_selected { accent_color() } else { button_color() };
            surface.fill_rect(btn_x, btn_y, 120, 60, bg);
        }

        // Delay buttons
        let delays = [0u32, 3, 5, 10];
        for (i, &delay) in delays.iter().enumerate() {
            let btn_x = x + 20 + i * 50;
            let btn_y = y + 140;
            let is_selected = self.delay_seconds == delay;
            let bg = if is_selected { accent_color() } else { button_color() };
            surface.fill_rect(btn_x, btn_y, 40, 25, bg);
        }

        // Include cursor checkbox
        let checkbox_x = x + 20;
        let checkbox_y = y + 180;
        let check_bg = if self.include_cursor { accent_color() } else { button_color() };
        surface.fill_rect(checkbox_x, checkbox_y, 20, 20, check_bg);

        // Capture button
        let btn_x = x + w / 2 - 60;
        let btn_y = y + h - 60;
        surface.fill_rect(btn_x, btn_y, 120, 40, accent_color());
    }

    fn handle_event(&mut self, event: &WidgetEvent) -> bool {
        match event {
            WidgetEvent::MouseDown { x, y, button: MouseButton::Left } => {
                let rel_x = (*x - self.bounds.x) as usize;
                let rel_y = (*y - self.bounds.y) as usize;

                // Check mode buttons
                if rel_y >= 60 && rel_y < 120 {
                    if rel_x >= 20 && rel_x < 140 {
                        self.selected_mode = CaptureMode::FullScreen;
                        return true;
                    } else if rel_x >= 150 && rel_x < 270 {
                        self.selected_mode = CaptureMode::ActiveWindow;
                        return true;
                    } else if rel_x >= 280 && rel_x < 400 {
                        self.selected_mode = CaptureMode::Region;
                        return true;
                    }
                }

                // Check delay buttons
                if rel_y >= 140 && rel_y < 165 {
                    let delays = [0u32, 3, 5, 10];
                    for (i, &delay) in delays.iter().enumerate() {
                        let btn_x = 20 + i * 50;
                        if rel_x >= btn_x && rel_x < btn_x + 40 {
                            self.delay_seconds = delay;
                            return true;
                        }
                    }
                }

                // Check include cursor checkbox
                if rel_y >= 180 && rel_y < 200 && rel_x >= 20 && rel_x < 40 {
                    self.include_cursor = !self.include_cursor;
                    return true;
                }

                // Check capture button
                let btn_x = self.bounds.width / 2 - 60;
                let btn_y = self.bounds.height - 60;
                if rel_x >= btn_x && rel_x < btn_x + 120 && rel_y >= btn_y && rel_y < btn_y + 40 {
                    set_capture_mode(self.selected_mode);
                    set_delay(self.delay_seconds);
                    set_include_cursor(self.include_cursor);

                    if self.selected_mode == CaptureMode::Region {
                        start_region_selection();
                    } else {
                        let _ = take_screenshot();
                    }
                    return true;
                }

                false
            }
            _ => false,
        }
    }
}
