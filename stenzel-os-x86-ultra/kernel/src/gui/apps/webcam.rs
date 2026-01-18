//! Webcam Application
//!
//! Application for capturing photos and videos from webcams and other video devices.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;

use crate::gui::widgets::{Widget, WidgetId, WidgetEvent, Bounds, MouseButton};
use crate::gui::surface::Surface;
use crate::drivers::framebuffer::Color;

/// Video device capabilities
#[derive(Debug, Clone)]
pub struct DeviceCapabilities {
    pub name: String,
    pub resolutions: Vec<Resolution>,
    pub formats: Vec<PixelFormat>,
    pub has_autofocus: bool,
    pub has_zoom: bool,
    pub has_pan_tilt: bool,
    pub has_flash: bool,
}

impl DeviceCapabilities {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            resolutions: Vec::new(),
            formats: Vec::new(),
            has_autofocus: false,
            has_zoom: false,
            has_pan_tilt: false,
            has_flash: false,
        }
    }
}

/// Video resolution
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolution {
    pub width: usize,
    pub height: usize,
    pub fps: usize,
}

impl Resolution {
    pub fn new(width: usize, height: usize, fps: usize) -> Self {
        Self { width, height, fps }
    }

    pub fn format(&self) -> String {
        format!("{}x{}@{}fps", self.width, self.height, self.fps)
    }

    pub fn aspect_ratio(&self) -> String {
        let gcd = gcd(self.width, self.height);
        let w = self.width / gcd;
        let h = self.height / gcd;
        format!("{}:{}", w, h)
    }

    pub fn megapixels(&self) -> f32 {
        (self.width * self.height) as f32 / 1_000_000.0
    }

    /// Standard resolutions
    pub fn vga() -> Self { Self::new(640, 480, 30) }
    pub fn hd720() -> Self { Self::new(1280, 720, 30) }
    pub fn hd1080() -> Self { Self::new(1920, 1080, 30) }
    pub fn uhd4k() -> Self { Self::new(3840, 2160, 30) }
}

fn gcd(a: usize, b: usize) -> usize {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// Pixel format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Yuyv,
    Mjpeg,
    Rgb24,
    Rgb32,
    Nv12,
    I420,
}

impl PixelFormat {
    pub fn name(&self) -> &'static str {
        match self {
            PixelFormat::Yuyv => "YUYV",
            PixelFormat::Mjpeg => "MJPEG",
            PixelFormat::Rgb24 => "RGB24",
            PixelFormat::Rgb32 => "RGB32",
            PixelFormat::Nv12 => "NV12",
            PixelFormat::I420 => "I420",
        }
    }

    pub fn bytes_per_pixel(&self) -> usize {
        match self {
            PixelFormat::Yuyv => 2,
            PixelFormat::Mjpeg => 0, // Variable
            PixelFormat::Rgb24 => 3,
            PixelFormat::Rgb32 => 4,
            PixelFormat::Nv12 => 0, // Planar
            PixelFormat::I420 => 0, // Planar
        }
    }
}

/// Video device
#[derive(Debug, Clone)]
pub struct VideoDevice {
    pub id: u64,
    pub name: String,
    pub path: String,
    pub capabilities: DeviceCapabilities,
    pub is_connected: bool,
    pub is_open: bool,
}

impl VideoDevice {
    pub fn new(id: u64, name: &str, path: &str) -> Self {
        Self {
            id,
            name: name.to_string(),
            path: path.to_string(),
            capabilities: DeviceCapabilities::new(name),
            is_connected: true,
            is_open: false,
        }
    }
}

/// Capture mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureMode {
    Photo,
    Video,
    Timelapse,
    Burst,
}

impl CaptureMode {
    pub fn name(&self) -> &'static str {
        match self {
            CaptureMode::Photo => "Photo",
            CaptureMode::Video => "Video",
            CaptureMode::Timelapse => "Timelapse",
            CaptureMode::Burst => "Burst",
        }
    }

    pub fn icon(&self) -> char {
        match self {
            CaptureMode::Photo => '📷',
            CaptureMode::Video => '🎥',
            CaptureMode::Timelapse => '⏱',
            CaptureMode::Burst => '📸',
        }
    }
}

/// Photo quality settings
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhotoQuality {
    Low,
    Medium,
    High,
    Maximum,
}

impl PhotoQuality {
    pub fn name(&self) -> &'static str {
        match self {
            PhotoQuality::Low => "Low",
            PhotoQuality::Medium => "Medium",
            PhotoQuality::High => "High",
            PhotoQuality::Maximum => "Maximum",
        }
    }

    pub fn jpeg_quality(&self) -> u8 {
        match self {
            PhotoQuality::Low => 60,
            PhotoQuality::Medium => 80,
            PhotoQuality::High => 90,
            PhotoQuality::Maximum => 100,
        }
    }
}

/// Video quality preset
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoQuality {
    Low,      // 480p
    Medium,   // 720p
    High,     // 1080p
    UltraHd,  // 4K
}

impl VideoQuality {
    pub fn name(&self) -> &'static str {
        match self {
            VideoQuality::Low => "480p",
            VideoQuality::Medium => "720p",
            VideoQuality::High => "1080p",
            VideoQuality::UltraHd => "4K",
        }
    }

    pub fn resolution(&self) -> Resolution {
        match self {
            VideoQuality::Low => Resolution::new(640, 480, 30),
            VideoQuality::Medium => Resolution::hd720(),
            VideoQuality::High => Resolution::hd1080(),
            VideoQuality::UltraHd => Resolution::uhd4k(),
        }
    }

    pub fn bitrate_kbps(&self) -> u32 {
        match self {
            VideoQuality::Low => 1000,
            VideoQuality::Medium => 2500,
            VideoQuality::High => 5000,
            VideoQuality::UltraHd => 15000,
        }
    }
}

/// Video codec
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoCodec {
    H264,
    H265,
    Vp8,
    Vp9,
    Av1,
    Raw,
}

impl VideoCodec {
    pub fn name(&self) -> &'static str {
        match self {
            VideoCodec::H264 => "H.264",
            VideoCodec::H265 => "H.265",
            VideoCodec::Vp8 => "VP8",
            VideoCodec::Vp9 => "VP9",
            VideoCodec::Av1 => "AV1",
            VideoCodec::Raw => "Raw",
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            VideoCodec::H264 | VideoCodec::H265 => "mp4",
            VideoCodec::Vp8 | VideoCodec::Vp9 | VideoCodec::Av1 => "webm",
            VideoCodec::Raw => "avi",
        }
    }
}

/// Camera state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraState {
    Idle,
    Previewing,
    Capturing,
    Recording,
    Processing,
    Error,
}

impl CameraState {
    pub fn name(&self) -> &'static str {
        match self {
            CameraState::Idle => "Idle",
            CameraState::Previewing => "Preview",
            CameraState::Capturing => "Capturing",
            CameraState::Recording => "Recording",
            CameraState::Processing => "Processing",
            CameraState::Error => "Error",
        }
    }

    pub fn can_capture(&self) -> bool {
        matches!(self, CameraState::Previewing)
    }
}

/// Timer settings for self-timer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerSetting {
    Off,
    Seconds3,
    Seconds5,
    Seconds10,
}

impl TimerSetting {
    pub fn seconds(&self) -> u64 {
        match self {
            TimerSetting::Off => 0,
            TimerSetting::Seconds3 => 3,
            TimerSetting::Seconds5 => 5,
            TimerSetting::Seconds10 => 10,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            TimerSetting::Off => "Off",
            TimerSetting::Seconds3 => "3s",
            TimerSetting::Seconds5 => "5s",
            TimerSetting::Seconds10 => "10s",
        }
    }
}

/// Camera settings
#[derive(Debug, Clone)]
pub struct CameraSettings {
    pub resolution: Resolution,
    pub photo_quality: PhotoQuality,
    pub video_quality: VideoQuality,
    pub video_codec: VideoCodec,
    pub timer: TimerSetting,
    pub mirror_preview: bool,
    pub sound_enabled: bool,
    pub grid_enabled: bool,
    pub date_stamp: bool,
    pub location_stamp: bool,
    pub auto_brightness: bool,
    pub auto_focus: bool,
    pub flash_mode: FlashMode,
    pub white_balance: WhiteBalance,
    pub exposure: i32,
    pub zoom: u32,
    pub burst_count: usize,
    pub timelapse_interval: u64,
    pub output_directory: String,
}

impl Default for CameraSettings {
    fn default() -> Self {
        Self {
            resolution: Resolution::hd720(),
            photo_quality: PhotoQuality::High,
            video_quality: VideoQuality::High,
            video_codec: VideoCodec::H264,
            timer: TimerSetting::Off,
            mirror_preview: true,
            sound_enabled: true,
            grid_enabled: false,
            date_stamp: false,
            location_stamp: false,
            auto_brightness: true,
            auto_focus: true,
            flash_mode: FlashMode::Auto,
            white_balance: WhiteBalance::Auto,
            exposure: 0,
            zoom: 100,
            burst_count: 5,
            timelapse_interval: 5,
            output_directory: String::from("/home/user/Pictures"),
        }
    }
}

/// Flash mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlashMode {
    Off,
    On,
    Auto,
    RedEyeReduction,
}

impl FlashMode {
    pub fn name(&self) -> &'static str {
        match self {
            FlashMode::Off => "Off",
            FlashMode::On => "On",
            FlashMode::Auto => "Auto",
            FlashMode::RedEyeReduction => "Red Eye",
        }
    }
}

/// White balance
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhiteBalance {
    Auto,
    Daylight,
    Cloudy,
    Incandescent,
    Fluorescent,
    Flash,
    Custom,
}

impl WhiteBalance {
    pub fn name(&self) -> &'static str {
        match self {
            WhiteBalance::Auto => "Auto",
            WhiteBalance::Daylight => "Daylight",
            WhiteBalance::Cloudy => "Cloudy",
            WhiteBalance::Incandescent => "Incandescent",
            WhiteBalance::Fluorescent => "Fluorescent",
            WhiteBalance::Flash => "Flash",
            WhiteBalance::Custom => "Custom",
        }
    }
}

/// Captured media item
#[derive(Debug, Clone)]
pub struct MediaItem {
    pub id: u64,
    pub path: String,
    pub filename: String,
    pub is_video: bool,
    pub timestamp: u64,
    pub size: u64,
    pub duration_ms: Option<u64>,
    pub resolution: Resolution,
    pub thumbnail: Option<Vec<u8>>,
}

impl MediaItem {
    pub fn format_size(&self) -> String {
        if self.size < 1024 {
            format!("{} B", self.size)
        } else if self.size < 1024 * 1024 {
            format!("{} KB", self.size / 1024)
        } else {
            format!("{:.1} MB", self.size as f32 / (1024.0 * 1024.0))
        }
    }

    pub fn format_duration(&self) -> Option<String> {
        self.duration_ms.map(|ms| {
            let secs = ms / 1000;
            let mins = secs / 60;
            let secs = secs % 60;
            format!("{}:{:02}", mins, secs)
        })
    }
}

/// Recording statistics
#[derive(Debug, Clone, Default)]
pub struct RecordingStats {
    pub duration_seconds: u64,
    pub frames_recorded: u64,
    pub frames_dropped: u64,
    pub bytes_written: u64,
    pub current_fps: f32,
    pub avg_fps: f32,
    pub bitrate_kbps: u32,
}

impl RecordingStats {
    pub fn format_duration(&self) -> String {
        let mins = self.duration_seconds / 60;
        let secs = self.duration_seconds % 60;
        format!("{}:{:02}", mins, secs)
    }

    pub fn format_size(&self) -> String {
        if self.bytes_written < 1024 * 1024 {
            format!("{} KB", self.bytes_written / 1024)
        } else {
            format!("{:.1} MB", self.bytes_written as f32 / (1024.0 * 1024.0))
        }
    }
}

// Helper functions for rendering
fn draw_char_at(surface: &mut Surface, x: usize, y: usize, c: char, color: Color) {
    use crate::drivers::font::DEFAULT_FONT;
    if let Some(glyph) = DEFAULT_FONT.get_glyph(c) {
        for row in 0..DEFAULT_FONT.height {
            let byte = glyph[row];
            for col in 0..DEFAULT_FONT.width {
                if (byte >> (7 - col)) & 1 != 0 {
                    surface.set_pixel(x + col, y + row, color);
                }
            }
        }
    }
}

fn draw_char(surface: &mut Surface, x: isize, y: isize, c: char, color: Color) {
    if x >= 0 && y >= 0 {
        draw_char_at(surface, x as usize, y as usize, c, color);
    }
}

fn draw_string(surface: &mut Surface, x: isize, y: isize, s: &str, color: Color) {
    if x < 0 || y < 0 { return; }
    let mut px = x as usize;
    for c in s.chars() {
        draw_char_at(surface, px, y as usize, c, color);
        px += 8;
    }
}

/// Webcam application widget
pub struct WebcamApp {
    id: WidgetId,
    bounds: Bounds,
    enabled: bool,
    visible: bool,

    // Devices
    devices: Vec<VideoDevice>,
    selected_device_id: Option<u64>,
    next_device_id: u64,

    // State
    state: CameraState,
    capture_mode: CaptureMode,
    settings: CameraSettings,
    recording_stats: RecordingStats,
    error_message: Option<String>,

    // Captured media
    gallery: Vec<MediaItem>,
    next_media_id: u64,
    selected_media_id: Option<u64>,

    // UI state
    show_settings: bool,
    show_gallery: bool,
    timer_countdown: Option<u64>,
    preview_frame: Option<Vec<u8>>,
    hovered_button: Option<usize>,
}

impl WebcamApp {
    pub fn new(id: WidgetId) -> Self {
        let mut app = Self {
            id,
            bounds: Bounds { x: 0, y: 0, width: 800, height: 600 },
            enabled: true,
            visible: true,
            devices: Vec::new(),
            selected_device_id: None,
            next_device_id: 1,
            state: CameraState::Idle,
            capture_mode: CaptureMode::Photo,
            settings: CameraSettings::default(),
            recording_stats: RecordingStats::default(),
            error_message: None,
            gallery: Vec::new(),
            next_media_id: 1,
            selected_media_id: None,
            show_settings: false,
            show_gallery: false,
            timer_countdown: None,
            preview_frame: None,
            hovered_button: None,
        };

        app.detect_devices();
        app
    }

    fn detect_devices(&mut self) {
        // Simulate detecting webcams
        let mut webcam = VideoDevice::new(self.next_device_id, "Integrated Webcam", "/dev/video0");
        webcam.capabilities.resolutions = vec![
            Resolution::new(640, 480, 30),
            Resolution::new(1280, 720, 30),
            Resolution::new(1920, 1080, 30),
            Resolution::new(1920, 1080, 60),
        ];
        webcam.capabilities.formats = vec![PixelFormat::Yuyv, PixelFormat::Mjpeg];
        webcam.capabilities.has_autofocus = true;
        self.devices.push(webcam);
        self.next_device_id += 1;

        // If there's a device, select it
        if let Some(device) = self.devices.first() {
            self.selected_device_id = Some(device.id);
        }

        // Add sample gallery items
        self.add_sample_gallery();
    }

    fn add_sample_gallery(&mut self) {
        // Sample captured photos/videos
        self.gallery.push(MediaItem {
            id: self.next_media_id,
            path: String::from("/home/user/Pictures/photo_001.jpg"),
            filename: String::from("photo_001.jpg"),
            is_video: false,
            timestamp: 1705600000,
            size: 2_500_000,
            duration_ms: None,
            resolution: Resolution::hd1080(),
            thumbnail: None,
        });
        self.next_media_id += 1;

        self.gallery.push(MediaItem {
            id: self.next_media_id,
            path: String::from("/home/user/Videos/video_001.mp4"),
            filename: String::from("video_001.mp4"),
            is_video: true,
            timestamp: 1705590000,
            size: 50_000_000,
            duration_ms: Some(30000),
            resolution: Resolution::hd1080(),
            thumbnail: None,
        });
        self.next_media_id += 1;
    }

    // Device management
    pub fn select_device(&mut self, device_id: u64) {
        if self.state == CameraState::Recording {
            return; // Can't change device while recording
        }

        if let Some(device) = self.devices.iter().find(|d| d.id == device_id) {
            self.selected_device_id = Some(device.id);
            self.state = CameraState::Idle;
        }
    }

    pub fn get_selected_device(&self) -> Option<&VideoDevice> {
        self.selected_device_id.and_then(|id| self.devices.iter().find(|d| d.id == id))
    }

    // Camera controls
    pub fn start_preview(&mut self) {
        if self.selected_device_id.is_some() && self.state == CameraState::Idle {
            self.state = CameraState::Previewing;
            self.error_message = None;
        }
    }

    pub fn stop_preview(&mut self) {
        if self.state == CameraState::Previewing {
            self.state = CameraState::Idle;
        }
    }

    pub fn capture_photo(&mut self) {
        if self.state != CameraState::Previewing {
            return;
        }

        if self.settings.timer.seconds() > 0 {
            self.timer_countdown = Some(self.settings.timer.seconds());
        } else {
            self.do_capture_photo();
        }
    }

    fn do_capture_photo(&mut self) {
        self.state = CameraState::Capturing;

        // Simulate capture
        let item = MediaItem {
            id: self.next_media_id,
            path: format!("{}/photo_{:03}.jpg", self.settings.output_directory, self.next_media_id),
            filename: format!("photo_{:03}.jpg", self.next_media_id),
            is_video: false,
            timestamp: 0, // Would be real timestamp
            size: 2_000_000 + (self.next_media_id * 100_000),
            duration_ms: None,
            resolution: self.settings.resolution,
            thumbnail: None,
        };
        self.gallery.insert(0, item);
        self.next_media_id += 1;

        self.state = CameraState::Previewing;
        self.timer_countdown = None;
    }

    pub fn start_recording(&mut self) {
        if self.state != CameraState::Previewing {
            return;
        }

        self.state = CameraState::Recording;
        self.recording_stats = RecordingStats::default();
    }

    pub fn stop_recording(&mut self) {
        if self.state != CameraState::Recording {
            return;
        }

        self.state = CameraState::Processing;

        // Simulate saving recording
        let item = MediaItem {
            id: self.next_media_id,
            path: format!("{}/video_{:03}.{}",
                self.settings.output_directory,
                self.next_media_id,
                self.settings.video_codec.extension()),
            filename: format!("video_{:03}.{}", self.next_media_id, self.settings.video_codec.extension()),
            is_video: true,
            timestamp: 0,
            size: self.recording_stats.bytes_written,
            duration_ms: Some(self.recording_stats.duration_seconds * 1000),
            resolution: self.settings.video_quality.resolution(),
            thumbnail: None,
        };
        self.gallery.insert(0, item);
        self.next_media_id += 1;

        self.state = CameraState::Previewing;
    }

    pub fn toggle_recording(&mut self) {
        match self.state {
            CameraState::Previewing => self.start_recording(),
            CameraState::Recording => self.stop_recording(),
            _ => {}
        }
    }

    // Settings
    pub fn set_capture_mode(&mut self, mode: CaptureMode) {
        if self.state != CameraState::Recording {
            self.capture_mode = mode;
        }
    }

    pub fn set_resolution(&mut self, resolution: Resolution) {
        if self.state != CameraState::Recording {
            self.settings.resolution = resolution;
        }
    }

    pub fn set_timer(&mut self, timer: TimerSetting) {
        self.settings.timer = timer;
    }

    pub fn toggle_mirror(&mut self) {
        self.settings.mirror_preview = !self.settings.mirror_preview;
    }

    pub fn toggle_grid(&mut self) {
        self.settings.grid_enabled = !self.settings.grid_enabled;
    }

    pub fn toggle_settings(&mut self) {
        self.show_settings = !self.show_settings;
        self.show_gallery = false;
    }

    pub fn toggle_gallery(&mut self) {
        self.show_gallery = !self.show_gallery;
        self.show_settings = false;
    }

    // Gallery management
    pub fn select_media(&mut self, media_id: u64) {
        self.selected_media_id = Some(media_id);
    }

    pub fn delete_selected_media(&mut self) {
        if let Some(id) = self.selected_media_id {
            self.gallery.retain(|m| m.id != id);
            self.selected_media_id = None;
        }
    }

    // Helpers
    fn format_countdown(&self) -> Option<String> {
        self.timer_countdown.map(|c| c.to_string())
    }
}

impl Widget for WebcamApp {
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

    fn handle_event(&mut self, event: &WidgetEvent) -> bool {
        match event {
            WidgetEvent::MouseDown { x, y, button } => {
                if *button != MouseButton::Left {
                    return false;
                }

                let rel_x = *x - self.bounds.x;
                let rel_y = *y - self.bounds.y;

                // Toolbar buttons (top)
                if rel_y >= 10 && rel_y < 50 {
                    // Settings button
                    if rel_x >= 10 && rel_x < 60 {
                        self.toggle_settings();
                        return true;
                    }

                    // Gallery button
                    if rel_x >= 70 && rel_x < 130 {
                        self.toggle_gallery();
                        return true;
                    }

                    // Device selector
                    if rel_x >= 140 && rel_x < 300 && self.devices.len() > 1 {
                        // Cycle through devices
                        if let Some(current_id) = self.selected_device_id {
                            let next_idx = self.devices.iter()
                                .position(|d| d.id == current_id)
                                .map(|i| (i + 1) % self.devices.len())
                                .unwrap_or(0);
                            if let Some(next_device) = self.devices.get(next_idx) {
                                self.select_device(next_device.id);
                            }
                        }
                        return true;
                    }
                }

                // Bottom controls
                let controls_y = self.bounds.height as isize - 80;
                if rel_y >= controls_y && rel_y < self.bounds.height as isize - 10 {
                    let center_x = self.bounds.width as isize / 2;

                    // Mode buttons (left side)
                    let mode_x = center_x - 150;
                    if rel_x >= mode_x && rel_x < mode_x + 40 {
                        self.set_capture_mode(CaptureMode::Photo);
                        return true;
                    }
                    if rel_x >= mode_x + 45 && rel_x < mode_x + 85 {
                        self.set_capture_mode(CaptureMode::Video);
                        return true;
                    }

                    // Capture button (center)
                    let capture_x = center_x - 30;
                    if rel_x >= capture_x && rel_x < capture_x + 60 {
                        if self.state == CameraState::Idle {
                            self.start_preview();
                        } else if self.state == CameraState::Previewing {
                            match self.capture_mode {
                                CaptureMode::Photo | CaptureMode::Burst => self.capture_photo(),
                                CaptureMode::Video | CaptureMode::Timelapse => self.start_recording(),
                            }
                        } else if self.state == CameraState::Recording {
                            self.stop_recording();
                        }
                        return true;
                    }

                    // Timer button (right side)
                    let timer_x = center_x + 80;
                    if rel_x >= timer_x && rel_x < timer_x + 50 {
                        let next_timer = match self.settings.timer {
                            TimerSetting::Off => TimerSetting::Seconds3,
                            TimerSetting::Seconds3 => TimerSetting::Seconds5,
                            TimerSetting::Seconds5 => TimerSetting::Seconds10,
                            TimerSetting::Seconds10 => TimerSetting::Off,
                        };
                        self.set_timer(next_timer);
                        return true;
                    }
                }

                false
            }

            WidgetEvent::KeyDown { key, .. } => {
                match *key {
                    0x39 => { // Space - capture
                        if self.state == CameraState::Previewing {
                            match self.capture_mode {
                                CaptureMode::Photo | CaptureMode::Burst => self.capture_photo(),
                                CaptureMode::Video | CaptureMode::Timelapse => self.toggle_recording(),
                            }
                        } else if self.state == CameraState::Recording {
                            self.stop_recording();
                        }
                        true
                    }
                    0x1B => { // Escape - close panels
                        if self.show_settings || self.show_gallery {
                            self.show_settings = false;
                            self.show_gallery = false;
                            true
                        } else if self.state == CameraState::Previewing {
                            self.stop_preview();
                            true
                        } else {
                            false
                        }
                    }
                    0x22 => { // G - gallery
                        self.toggle_gallery();
                        true
                    }
                    0x1F => { // S - settings
                        self.toggle_settings();
                        true
                    }
                    _ => false,
                }
            }

            _ => false,
        }
    }

    fn render(&self, surface: &mut Surface) {
        let bg = Color::new(20, 20, 25);
        let toolbar_bg = Color::new(30, 30, 35);
        let preview_bg = Color::new(10, 10, 15);
        let text_color = Color::new(230, 230, 230);
        let dim_text = Color::new(150, 150, 155);
        let accent_color = Color::new(255, 80, 80);
        let button_bg = Color::new(50, 50, 55);
        let button_hover = Color::new(70, 70, 75);
        let recording_color = Color::new(255, 50, 50);

        // Background
        for y in 0..self.bounds.height {
            for x in 0..self.bounds.width {
                surface.set_pixel(
                    (self.bounds.x as usize) + x,
                    (self.bounds.y as usize) + y,
                    bg
                );
            }
        }

        // Top toolbar
        for y in 0..60 {
            for x in 0..self.bounds.width {
                surface.set_pixel(
                    (self.bounds.x as usize) + x,
                    (self.bounds.y as usize) + y,
                    toolbar_bg
                );
            }
        }

        // Toolbar buttons
        let toolbar_y = self.bounds.y + 18;

        // Settings button
        draw_string(surface, self.bounds.x + 15, toolbar_y, "[Set]", text_color);

        // Gallery button
        draw_string(surface, self.bounds.x + 75, toolbar_y, "[Gal]", text_color);

        // Device name
        if let Some(device) = self.get_selected_device() {
            draw_string(surface, self.bounds.x + 150, toolbar_y, &device.name, dim_text);
        } else {
            draw_string(surface, self.bounds.x + 150, toolbar_y, "No camera detected", dim_text);
        }

        // State indicator
        let state_str = self.state.name();
        let state_color = match self.state {
            CameraState::Recording => recording_color,
            CameraState::Error => Color::new(255, 100, 100),
            _ => dim_text,
        };
        draw_string(surface, self.bounds.x + self.bounds.width as isize - 120, toolbar_y, state_str, state_color);

        // Preview area
        let preview_x = self.bounds.x + 20;
        let preview_y = self.bounds.y + 70;
        let preview_width = self.bounds.width - 40;
        let preview_height = self.bounds.height - 170;

        // Preview background
        for y in 0..preview_height {
            for x in 0..preview_width {
                surface.set_pixel(
                    (preview_x as usize) + x,
                    (preview_y as usize) + y,
                    preview_bg
                );
            }
        }

        // Preview content based on state
        let center_x = preview_x + (preview_width as isize / 2);
        let center_y = preview_y + (preview_height as isize / 2);

        match self.state {
            CameraState::Idle => {
                draw_string(surface, center_x - 80, center_y - 20, "Camera Off", dim_text);
                draw_string(surface, center_x - 100, center_y + 10, "Press capture to start", dim_text);
            }
            CameraState::Previewing | CameraState::Recording => {
                // Simulate camera preview with pattern
                for y in (0..preview_height).step_by(40) {
                    for x in (0..preview_width).step_by(40) {
                        let shade = ((x + y) % 80) as u8 + 30;
                        for py in 0..35 {
                            for px in 0..35 {
                                if (preview_x as usize) + x + px < (self.bounds.x as usize) + self.bounds.width - 20 &&
                                   (preview_y as usize) + y + py < (self.bounds.y as usize) + self.bounds.height - 100 {
                                    surface.set_pixel(
                                        (preview_x as usize) + x + px,
                                        (preview_y as usize) + y + py,
                                        Color::new(shade, shade + 10, shade + 20)
                                    );
                                }
                            }
                        }
                    }
                }

                // Grid overlay
                if self.settings.grid_enabled {
                    let grid_color = Color::new(100, 100, 100);
                    // Vertical lines (rule of thirds)
                    for i in 1..3 {
                        let x = preview_x as usize + (preview_width * i / 3);
                        for y in 0..preview_height {
                            surface.set_pixel(x, (preview_y as usize) + y, grid_color);
                        }
                    }
                    // Horizontal lines
                    for i in 1..3 {
                        let y = preview_y as usize + (preview_height * i / 3);
                        for x in 0..preview_width {
                            surface.set_pixel((preview_x as usize) + x, y, grid_color);
                        }
                    }
                }

                // Recording indicator
                if self.state == CameraState::Recording {
                    // Red recording dot
                    let dot_x = preview_x + 20;
                    let dot_y = preview_y + 20;
                    for dy in 0..12 {
                        for dx in 0..12 {
                            let dist = ((dx as i32 - 6).pow(2) + (dy as i32 - 6).pow(2)) as f32;
                            if dist < 25.0 { // sqrt optimization: dist < r^2
                                surface.set_pixel((dot_x + dx as isize) as usize, (dot_y + dy as isize) as usize, recording_color);
                            }
                        }
                    }
                    draw_string(surface, dot_x + 20, dot_y + 2, "REC", recording_color);

                    // Recording time
                    let time_str = self.recording_stats.format_duration();
                    draw_string(surface, dot_x + 60, dot_y + 2, &time_str, text_color);
                }

                // Timer countdown
                if let Some(countdown) = self.timer_countdown {
                    let countdown_str = countdown.to_string();
                    // Draw large countdown number
                    draw_string(surface, center_x - 8, center_y - 8, &countdown_str, accent_color);
                }
            }
            CameraState::Processing => {
                draw_string(surface, center_x - 50, center_y, "Processing...", dim_text);
            }
            CameraState::Error => {
                if let Some(ref msg) = self.error_message {
                    draw_string(surface, center_x - 80, center_y, msg, Color::new(255, 100, 100));
                } else {
                    draw_string(surface, center_x - 40, center_y, "Error", Color::new(255, 100, 100));
                }
            }
            CameraState::Capturing => {
                // Flash effect
                for y in 0..preview_height {
                    for x in 0..preview_width {
                        surface.set_pixel(
                            (preview_x as usize) + x,
                            (preview_y as usize) + y,
                            Color::new(255, 255, 255)
                        );
                    }
                }
            }
        }

        // Bottom controls bar
        let controls_y = self.bounds.y + self.bounds.height as isize - 80;
        for y in 0..80 {
            for x in 0..self.bounds.width {
                surface.set_pixel(
                    (self.bounds.x as usize) + x,
                    (controls_y as usize) + y,
                    toolbar_bg
                );
            }
        }

        let control_center_x = self.bounds.x + (self.bounds.width as isize / 2);

        // Mode buttons
        let mode_y = controls_y + 30;
        let photo_color = if self.capture_mode == CaptureMode::Photo { accent_color } else { dim_text };
        let video_color = if self.capture_mode == CaptureMode::Video { accent_color } else { dim_text };

        draw_string(surface, control_center_x - 150, mode_y, "Photo", photo_color);
        draw_string(surface, control_center_x - 95, mode_y, "Video", video_color);

        // Capture button
        let capture_btn_x = control_center_x - 25;
        let capture_btn_y = controls_y + 15;
        let capture_size = 50;

        // Draw circular capture button
        for dy in 0..capture_size {
            for dx in 0..capture_size {
                let dist = ((dx as i32 - capture_size as i32 / 2).pow(2) +
                           (dy as i32 - capture_size as i32 / 2).pow(2)) as f32;
                let radius = capture_size as f32 / 2.0;
                let radius_sq = radius * radius;
                if dist < radius_sq {
                    let btn_color = if self.state == CameraState::Recording {
                        recording_color
                    } else if self.capture_mode == CaptureMode::Video {
                        Color::new(200, 50, 50)
                    } else {
                        Color::new(240, 240, 240)
                    };
                    surface.set_pixel(
                        (capture_btn_x + dx as isize) as usize,
                        (capture_btn_y + dy as isize) as usize,
                        btn_color
                    );
                }
            }
        }

        // Inner circle for recording stop
        if self.state == CameraState::Recording {
            let inner_size = 20;
            let inner_x = capture_btn_x + (capture_size as isize - inner_size as isize) / 2;
            let inner_y = capture_btn_y + (capture_size as isize - inner_size as isize) / 2;
            for dy in 0..inner_size {
                for dx in 0..inner_size {
                    surface.set_pixel(
                        (inner_x + dx as isize) as usize,
                        (inner_y + dy as isize) as usize,
                        Color::new(255, 255, 255)
                    );
                }
            }
        }

        // Timer button
        let timer_x = control_center_x + 80;
        let timer_name = self.settings.timer.name();
        draw_string(surface, timer_x, mode_y, &format!("Timer: {}", timer_name), dim_text);

        // Resolution info
        let res_str = self.settings.resolution.format();
        draw_string(surface, self.bounds.x + 20, controls_y + 55, &res_str, dim_text);

        // Gallery preview
        if !self.gallery.is_empty() {
            let gallery_btn_x = self.bounds.x + self.bounds.width as isize - 70;
            let gallery_btn_y = controls_y + 15;

            // Thumbnail placeholder
            for y in 0..50 {
                for x in 0..50 {
                    surface.set_pixel(
                        (gallery_btn_x + x as isize) as usize,
                        (gallery_btn_y + y as isize) as usize,
                        button_bg
                    );
                }
            }

            let count_str = self.gallery.len().to_string();
            draw_string(surface, gallery_btn_x + 20, gallery_btn_y + 20, &count_str, text_color);
        }

        // Settings panel overlay
        if self.show_settings {
            let panel_x = self.bounds.x + 20;
            let panel_y = self.bounds.y + 70;
            let panel_width = 250;
            let panel_height = 300;

            // Panel background
            for y in 0..panel_height {
                for x in 0..panel_width {
                    surface.set_pixel(
                        (panel_x + x as isize) as usize,
                        (panel_y + y as isize) as usize,
                        toolbar_bg
                    );
                }
            }

            draw_string(surface, panel_x + 10, panel_y + 15, "Settings", accent_color);

            let mut setting_y = panel_y + 50;
            let line_height = 25isize;

            // Resolution
            draw_string(surface, panel_x + 10, setting_y, "Resolution:", text_color);
            draw_string(surface, panel_x + 120, setting_y, &self.settings.resolution.format(), dim_text);
            setting_y += line_height;

            // Quality
            let quality_name = match self.capture_mode {
                CaptureMode::Photo | CaptureMode::Burst => self.settings.photo_quality.name(),
                CaptureMode::Video | CaptureMode::Timelapse => self.settings.video_quality.name(),
            };
            draw_string(surface, panel_x + 10, setting_y, "Quality:", text_color);
            draw_string(surface, panel_x + 120, setting_y, quality_name, dim_text);
            setting_y += line_height;

            // Flash
            draw_string(surface, panel_x + 10, setting_y, "Flash:", text_color);
            draw_string(surface, panel_x + 120, setting_y, self.settings.flash_mode.name(), dim_text);
            setting_y += line_height;

            // White balance
            draw_string(surface, panel_x + 10, setting_y, "White Bal:", text_color);
            draw_string(surface, panel_x + 120, setting_y, self.settings.white_balance.name(), dim_text);
            setting_y += line_height;

            // Mirror
            draw_string(surface, panel_x + 10, setting_y, "Mirror:", text_color);
            draw_string(surface, panel_x + 120, setting_y,
                if self.settings.mirror_preview { "On" } else { "Off" }, dim_text);
            setting_y += line_height;

            // Grid
            draw_string(surface, panel_x + 10, setting_y, "Grid:", text_color);
            draw_string(surface, panel_x + 120, setting_y,
                if self.settings.grid_enabled { "On" } else { "Off" }, dim_text);
            setting_y += line_height;

            // Auto focus
            draw_string(surface, panel_x + 10, setting_y, "Auto Focus:", text_color);
            draw_string(surface, panel_x + 120, setting_y,
                if self.settings.auto_focus { "On" } else { "Off" }, dim_text);
        }

        // Gallery panel overlay
        if self.show_gallery {
            let panel_x = self.bounds.x + self.bounds.width as isize - 270;
            let panel_y = self.bounds.y + 70;
            let panel_width = 250;
            let panel_height = 350;

            // Panel background
            for y in 0..panel_height {
                for x in 0..panel_width {
                    surface.set_pixel(
                        (panel_x + x as isize) as usize,
                        (panel_y + y as isize) as usize,
                        toolbar_bg
                    );
                }
            }

            draw_string(surface, panel_x + 10, panel_y + 15, "Gallery", accent_color);

            let items_start_y = panel_y + 50;
            let item_height = 40isize;

            for (i, item) in self.gallery.iter().take(7).enumerate() {
                let item_y = items_start_y + (i as isize * item_height);
                let is_selected = self.selected_media_id == Some(item.id);

                if is_selected {
                    for y in 0..item_height as usize - 2 {
                        for x in 0..panel_width {
                            surface.set_pixel(
                                (panel_x + x as isize) as usize,
                                (item_y + y as isize) as usize,
                                button_hover
                            );
                        }
                    }
                }

                // Icon
                let icon = if item.is_video { '🎥' } else { '📷' };
                let icon_str: String = icon.to_string();
                draw_string(surface, panel_x + 10, item_y + 12, &icon_str, text_color);

                // Filename
                let name = if item.filename.len() > 20 {
                    let mut n: String = item.filename.chars().take(17).collect();
                    n.push_str("...");
                    n
                } else {
                    item.filename.clone()
                };
                draw_string(surface, panel_x + 30, item_y + 5, &name, text_color);

                // Size / Duration
                let info = if item.is_video {
                    item.format_duration().unwrap_or_else(|| item.format_size())
                } else {
                    item.format_size()
                };
                draw_string(surface, panel_x + 30, item_y + 20, &info, dim_text);
            }

            if self.gallery.is_empty() {
                draw_string(surface, panel_x + 70, panel_y + 150, "No media", dim_text);
            }
        }

        // Recording stats (when recording)
        if self.state == CameraState::Recording {
            let stats_y = self.bounds.y + self.bounds.height as isize - 25;
            let stats_str = format!("Frames: {} | Size: {} | FPS: {:.1}",
                self.recording_stats.frames_recorded,
                self.recording_stats.format_size(),
                self.recording_stats.current_fps);
            draw_string(surface, self.bounds.x + 20, stats_y, &stats_str, dim_text);
        }
    }
}

/// Initialize the webcam module
pub fn init() {
    crate::kprintln!("[Webcam] Webcam application initialized");
}
