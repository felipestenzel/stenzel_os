//! Screen Recorder Application
//!
//! Records screen activity with audio support, region selection,
//! and various output format options.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;

use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use crate::gui::widgets::{Widget, WidgetId, WidgetEvent, Bounds, MouseButton, theme};

/// Recording state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordingState {
    Idle,
    Countdown,
    Recording,
    Paused,
    Stopping,
    Encoding,
}

impl RecordingState {
    pub fn is_active(&self) -> bool {
        matches!(self, RecordingState::Recording | RecordingState::Paused)
    }

    pub fn can_start(&self) -> bool {
        matches!(self, RecordingState::Idle)
    }
}

/// Recording region type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionType {
    FullScreen,
    Window,
    Selection,
    Monitor(usize),
}

impl RegionType {
    pub fn name(&self) -> &'static str {
        match self {
            RegionType::FullScreen => "Full Screen",
            RegionType::Window => "Window",
            RegionType::Selection => "Selection",
            RegionType::Monitor(n) => {
                match n {
                    0 => "Monitor 1",
                    1 => "Monitor 2",
                    _ => "Monitor",
                }
            }
        }
    }
}

/// Output video format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoFormat {
    Mp4,
    Webm,
    Mkv,
    Avi,
    Gif,
}

impl VideoFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            VideoFormat::Mp4 => "mp4",
            VideoFormat::Webm => "webm",
            VideoFormat::Mkv => "mkv",
            VideoFormat::Avi => "avi",
            VideoFormat::Gif => "gif",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            VideoFormat::Mp4 => "MP4 (H.264)",
            VideoFormat::Webm => "WebM (VP9)",
            VideoFormat::Mkv => "MKV (H.264)",
            VideoFormat::Avi => "AVI",
            VideoFormat::Gif => "GIF (Animated)",
        }
    }

    pub fn supports_audio(&self) -> bool {
        !matches!(self, VideoFormat::Gif)
    }
}

/// Video quality preset
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityPreset {
    Low,
    Medium,
    High,
    VeryHigh,
    Lossless,
}

impl QualityPreset {
    pub fn name(&self) -> &'static str {
        match self {
            QualityPreset::Low => "Low (720p)",
            QualityPreset::Medium => "Medium (1080p)",
            QualityPreset::High => "High (1080p HQ)",
            QualityPreset::VeryHigh => "Very High (4K)",
            QualityPreset::Lossless => "Lossless",
        }
    }

    pub fn bitrate(&self) -> u32 {
        match self {
            QualityPreset::Low => 2_000,
            QualityPreset::Medium => 5_000,
            QualityPreset::High => 10_000,
            QualityPreset::VeryHigh => 25_000,
            QualityPreset::Lossless => 50_000,
        }
    }
}

/// Frame rate option
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameRate {
    Fps15,
    Fps24,
    Fps30,
    Fps60,
    Fps120,
}

impl FrameRate {
    pub fn value(&self) -> u32 {
        match self {
            FrameRate::Fps15 => 15,
            FrameRate::Fps24 => 24,
            FrameRate::Fps30 => 30,
            FrameRate::Fps60 => 60,
            FrameRate::Fps120 => 120,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            FrameRate::Fps15 => "15 fps",
            FrameRate::Fps24 => "24 fps",
            FrameRate::Fps30 => "30 fps",
            FrameRate::Fps60 => "60 fps",
            FrameRate::Fps120 => "120 fps",
        }
    }
}

/// Audio source
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioSource {
    None,
    System,
    Microphone,
    Both,
}

impl AudioSource {
    pub fn name(&self) -> &'static str {
        match self {
            AudioSource::None => "No Audio",
            AudioSource::System => "System Audio",
            AudioSource::Microphone => "Microphone",
            AudioSource::Both => "System + Mic",
        }
    }
}

/// Recording region
#[derive(Debug, Clone)]
pub struct RecordingRegion {
    pub region_type: RegionType,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl RecordingRegion {
    pub fn full_screen(width: u32, height: u32) -> Self {
        RecordingRegion {
            region_type: RegionType::FullScreen,
            x: 0,
            y: 0,
            width,
            height,
        }
    }

    pub fn selection(x: i32, y: i32, width: u32, height: u32) -> Self {
        RecordingRegion {
            region_type: RegionType::Selection,
            x,
            y,
            width,
            height,
        }
    }
}

/// Recording settings
#[derive(Debug, Clone)]
pub struct RecordingSettings {
    /// Output format
    pub format: VideoFormat,
    /// Quality preset
    pub quality: QualityPreset,
    /// Frame rate
    pub frame_rate: FrameRate,
    /// Audio source
    pub audio: AudioSource,
    /// Recording region
    pub region: RecordingRegion,
    /// Show cursor
    pub show_cursor: bool,
    /// Highlight clicks
    pub highlight_clicks: bool,
    /// Countdown seconds before recording
    pub countdown: u8,
    /// Output directory
    pub output_dir: String,
    /// Custom filename prefix
    pub filename_prefix: String,
    /// Auto-stop after seconds (0 = disabled)
    pub auto_stop: u32,
    /// Enable hardware encoding
    pub hw_encoding: bool,
}

impl Default for RecordingSettings {
    fn default() -> Self {
        RecordingSettings {
            format: VideoFormat::Mp4,
            quality: QualityPreset::High,
            frame_rate: FrameRate::Fps30,
            audio: AudioSource::System,
            region: RecordingRegion::full_screen(1920, 1080),
            show_cursor: true,
            highlight_clicks: true,
            countdown: 3,
            output_dir: "/home/user/Videos/Recordings".to_string(),
            filename_prefix: "Recording".to_string(),
            auto_stop: 0,
            hw_encoding: true,
        }
    }
}

/// Recording statistics
#[derive(Debug, Clone, Default)]
pub struct RecordingStats {
    /// Duration in seconds
    pub duration: u32,
    /// Total frames captured
    pub frames_captured: u64,
    /// Dropped frames
    pub frames_dropped: u64,
    /// Current file size in bytes
    pub file_size: u64,
    /// Average bitrate
    pub avg_bitrate: u32,
    /// CPU usage percentage
    pub cpu_usage: f32,
    /// Memory usage in bytes
    pub memory_usage: u64,
}

impl RecordingStats {
    pub fn format_duration(&self) -> String {
        let hours = self.duration / 3600;
        let minutes = (self.duration % 3600) / 60;
        let seconds = self.duration % 60;
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    }

    pub fn format_file_size(&self) -> String {
        if self.file_size < 1024 {
            format!("{} B", self.file_size)
        } else if self.file_size < 1024 * 1024 {
            format!("{:.1} KB", self.file_size as f64 / 1024.0)
        } else if self.file_size < 1024 * 1024 * 1024 {
            format!("{:.1} MB", self.file_size as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.2} GB", self.file_size as f64 / (1024.0 * 1024.0 * 1024.0))
        }
    }

    pub fn drop_rate(&self) -> f32 {
        let total = self.frames_captured + self.frames_dropped;
        if total == 0 {
            0.0
        } else {
            self.frames_dropped as f32 / total as f32 * 100.0
        }
    }
}

/// Recent recording entry
#[derive(Debug, Clone)]
pub struct RecentRecording {
    pub id: u64,
    pub path: String,
    pub filename: String,
    pub format: VideoFormat,
    pub duration: u32,
    pub file_size: u64,
    pub timestamp: u64,
    pub width: u32,
    pub height: u32,
}

impl RecentRecording {
    pub fn format_duration(&self) -> String {
        let minutes = self.duration / 60;
        let seconds = self.duration % 60;
        format!("{}:{:02}", minutes, seconds)
    }
}

/// View mode for the recorder
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecorderView {
    Main,
    Settings,
    Recordings,
}

/// Screen Recorder Widget
pub struct ScreenRecorder {
    id: WidgetId,
    bounds: Bounds,
    enabled: bool,
    visible: bool,

    /// Recording state
    state: RecordingState,

    /// Recording settings
    settings: RecordingSettings,

    /// Current recording stats
    stats: RecordingStats,

    /// Countdown remaining
    countdown_remaining: u8,

    /// Recent recordings
    recordings: Vec<RecentRecording>,

    /// Current view
    view: RecorderView,

    /// Selected settings tab
    settings_tab: usize,

    /// Next recording ID
    next_recording_id: u64,

    /// Output file path (when recording)
    output_path: Option<String>,

    /// Simulated time
    current_time: u64,

    /// Scroll offset for recordings list
    scroll_offset: usize,

    /// Selected recording
    selected_recording: Option<usize>,
}

impl ScreenRecorder {
    pub fn new(id: WidgetId, x: isize, y: isize, width: usize, height: usize) -> Self {
        let mut recorder = ScreenRecorder {
            id,
            bounds: Bounds { x, y, width, height },
            enabled: true,
            visible: true,
            state: RecordingState::Idle,
            settings: RecordingSettings::default(),
            stats: RecordingStats::default(),
            countdown_remaining: 0,
            recordings: Vec::new(),
            view: RecorderView::Main,
            settings_tab: 0,
            next_recording_id: 1,
            output_path: None,
            current_time: 1737216000,
            scroll_offset: 0,
            selected_recording: None,
        };

        // Add sample recordings
        recorder.add_sample_recordings();
        recorder
    }

    fn add_sample_recordings(&mut self) {
        let samples = [
            ("Recording_2026-01-18_14-30-00.mp4", VideoFormat::Mp4, 125, 45_000_000, 1920, 1080),
            ("Tutorial_2026-01-17_10-15-00.mp4", VideoFormat::Mp4, 300, 120_000_000, 1920, 1080),
            ("Gameplay_2026-01-16_20-00-00.mkv", VideoFormat::Mkv, 1800, 2_500_000_000, 2560, 1440),
            ("Demo_2026-01-15_09-45-00.webm", VideoFormat::Webm, 60, 15_000_000, 1280, 720),
            ("Bug_report.gif", VideoFormat::Gif, 10, 5_000_000, 800, 600),
        ];

        for (filename, format, duration, size, w, h) in samples {
            let rec = RecentRecording {
                id: self.next_recording_id,
                path: format!("/home/user/Videos/Recordings/{}", filename),
                filename: filename.to_string(),
                format,
                duration,
                file_size: size,
                timestamp: self.current_time - (self.next_recording_id * 86400),
                width: w,
                height: h,
            };
            self.next_recording_id += 1;
            self.recordings.push(rec);
        }
    }

    pub fn start_recording(&mut self) {
        if !self.state.can_start() {
            return;
        }

        if self.settings.countdown > 0 {
            self.countdown_remaining = self.settings.countdown;
            self.state = RecordingState::Countdown;
        } else {
            self.begin_recording();
        }
    }

    fn begin_recording(&mut self) {
        let timestamp = self.current_time;
        let filename = format!(
            "{}_{}.{}",
            self.settings.filename_prefix,
            timestamp,
            self.settings.format.extension()
        );
        self.output_path = Some(format!("{}/{}", self.settings.output_dir, filename));
        self.state = RecordingState::Recording;
        self.stats = RecordingStats::default();
    }

    pub fn stop_recording(&mut self) {
        if !self.state.is_active() {
            return;
        }

        self.state = RecordingState::Stopping;

        // Simulate encoding completion
        self.state = RecordingState::Idle;

        // Create recording entry
        if let Some(path) = self.output_path.take() {
            let filename = path.rsplit('/').next().unwrap_or(&path).to_string();
            let recording = RecentRecording {
                id: self.next_recording_id,
                path: path.clone(),
                filename,
                format: self.settings.format,
                duration: self.stats.duration,
                file_size: self.stats.file_size,
                timestamp: self.current_time,
                width: self.settings.region.width,
                height: self.settings.region.height,
            };
            self.next_recording_id += 1;
            self.recordings.insert(0, recording);
        }
    }

    pub fn pause_recording(&mut self) {
        if self.state == RecordingState::Recording {
            self.state = RecordingState::Paused;
        }
    }

    pub fn resume_recording(&mut self) {
        if self.state == RecordingState::Paused {
            self.state = RecordingState::Recording;
        }
    }

    pub fn toggle_pause(&mut self) {
        match self.state {
            RecordingState::Recording => self.pause_recording(),
            RecordingState::Paused => self.resume_recording(),
            _ => {}
        }
    }

    pub fn cancel_countdown(&mut self) {
        if self.state == RecordingState::Countdown {
            self.state = RecordingState::Idle;
            self.countdown_remaining = 0;
        }
    }

    pub fn update(&mut self, delta_time: u32) {
        match self.state {
            RecordingState::Countdown => {
                // Countdown logic would decrement every second
                if self.countdown_remaining > 0 {
                    self.countdown_remaining -= 1;
                    if self.countdown_remaining == 0 {
                        self.begin_recording();
                    }
                }
            }
            RecordingState::Recording => {
                // Update stats
                self.stats.duration += delta_time;
                self.stats.frames_captured += (self.settings.frame_rate.value() * delta_time) as u64;
                self.stats.file_size = (self.stats.duration as u64 * self.settings.quality.bitrate() as u64 / 8) * 1000;
                self.stats.avg_bitrate = self.settings.quality.bitrate();
                self.stats.cpu_usage = 15.0 + (self.stats.duration % 10) as f32;
                self.stats.memory_usage = 200 * 1024 * 1024 + (self.stats.frames_captured * 1000);

                // Check auto-stop
                if self.settings.auto_stop > 0 && self.stats.duration >= self.settings.auto_stop {
                    self.stop_recording();
                }
            }
            _ => {}
        }
    }

    fn get_visible_recordings(&self) -> usize {
        (self.bounds.height.saturating_sub(150)) / 45
    }

    pub fn get_recording(&self, index: usize) -> Option<&RecentRecording> {
        self.recordings.get(index)
    }

    pub fn delete_recording(&mut self, index: usize) {
        if index < self.recordings.len() {
            self.recordings.remove(index);
            if self.selected_recording == Some(index) {
                self.selected_recording = None;
            }
        }
    }
}

impl Widget for ScreenRecorder {
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
        if !self.enabled || !self.visible {
            return false;
        }

        match event {
            WidgetEvent::MouseDown { x, y, button } => {
                if *button != MouseButton::Left {
                    return false;
                }

                let px = *x as isize;
                let py = *y as isize;

                if px < self.bounds.x || px >= self.bounds.x + self.bounds.width as isize ||
                   py < self.bounds.y || py >= self.bounds.y + self.bounds.height as isize {
                    return false;
                }

                let local_x = (px - self.bounds.x) as usize;
                let local_y = (py - self.bounds.y) as usize;

                // Check view tabs (top)
                if local_y < 40 {
                    let tab_width = 100;
                    if local_x < tab_width {
                        self.view = RecorderView::Main;
                    } else if local_x < tab_width * 2 {
                        self.view = RecorderView::Settings;
                    } else if local_x < tab_width * 3 {
                        self.view = RecorderView::Recordings;
                    }
                    return true;
                }

                // Main view buttons
                if self.view == RecorderView::Main && local_y >= self.bounds.height - 80 {
                    let center_x = self.bounds.width / 2;

                    // Record/Stop button
                    if local_x >= center_x - 50 && local_x < center_x + 50 {
                        if self.state.is_active() || self.state == RecordingState::Countdown {
                            if self.state == RecordingState::Countdown {
                                self.cancel_countdown();
                            } else {
                                self.stop_recording();
                            }
                        } else {
                            self.start_recording();
                        }
                        return true;
                    }

                    // Pause button (when recording)
                    if self.state.is_active() {
                        if local_x >= center_x + 70 && local_x < center_x + 130 {
                            self.toggle_pause();
                            return true;
                        }
                    }
                }

                // Recordings list
                if self.view == RecorderView::Recordings {
                    let list_start_y = 80;
                    let item_height = 45;
                    if local_y >= list_start_y {
                        let clicked_idx = (local_y - list_start_y) / item_height + self.scroll_offset;
                        if clicked_idx < self.recordings.len() {
                            self.selected_recording = Some(clicked_idx);
                            return true;
                        }
                    }
                }

                false
            }

            WidgetEvent::Scroll { delta_y, .. } => {
                if self.view == RecorderView::Recordings {
                    let max_scroll = self.recordings.len().saturating_sub(self.get_visible_recordings());
                    if *delta_y < 0 && self.scroll_offset < max_scroll {
                        self.scroll_offset = (self.scroll_offset + 2).min(max_scroll);
                        return true;
                    } else if *delta_y > 0 && self.scroll_offset > 0 {
                        self.scroll_offset = self.scroll_offset.saturating_sub(2);
                        return true;
                    }
                }
                false
            }

            WidgetEvent::KeyDown { key, .. } => {
                match *key {
                    0x52 | 0x72 => { // R - start recording
                        if self.state.can_start() {
                            self.start_recording();
                        }
                        true
                    }
                    0x53 | 0x73 => { // S - stop
                        if self.state.is_active() {
                            self.stop_recording();
                        }
                        true
                    }
                    0x20 => { // Space - pause/resume
                        self.toggle_pause();
                        true
                    }
                    0x1B => { // Escape - cancel countdown
                        self.cancel_countdown();
                        true
                    }
                    _ => false,
                }
            }

            _ => false,
        }
    }

    fn render(&self, surface: &mut Surface) {
        if !self.visible {
            return;
        }

        let _theme = theme();
        let x = self.bounds.x.max(0) as usize;
        let y = self.bounds.y.max(0) as usize;
        let w = self.bounds.width;
        let h = self.bounds.height;

        // Colors
        let bg_color = Color::new(30, 30, 35);
        let header_bg = Color::new(40, 40, 50);
        let text_color = Color::new(220, 220, 220);
        let dim_text = Color::new(150, 150, 150);
        let accent_color = Color::new(220, 50, 50); // Red for recording
        let active_tab = Color::new(60, 60, 70);
        let selected_bg = Color::new(50, 50, 60);

        // Draw background
        surface.fill_rect(x, y, w, h, bg_color);

        // Draw header/tabs
        surface.fill_rect(x, y, w, 40, header_bg);

        let tabs = [
            (RecorderView::Main, "Record"),
            (RecorderView::Settings, "Settings"),
            (RecorderView::Recordings, "Recordings"),
        ];

        let tab_width = 100;
        for (i, (view, name)) in tabs.iter().enumerate() {
            let tab_x = x + i * tab_width;
            if *view == self.view {
                surface.fill_rect(tab_x, y, tab_width, 40, active_tab);
            }
            draw_string(surface, tab_x + 20, y + 12, name, text_color);
        }

        // Recording indicator
        if self.state.is_active() {
            let indicator_x = x + w - 80;
            surface.fill_rect(indicator_x, y + 12, 16, 16, accent_color);
            draw_string(surface, indicator_x + 20, y + 12, "REC", accent_color);
        }

        // Content area
        let content_y = y + 50;

        match self.view {
            RecorderView::Main => {
                self.render_main_view(surface, x, content_y, w, h - 50, text_color, dim_text, accent_color);
            }
            RecorderView::Settings => {
                self.render_settings_view(surface, x, content_y, w, text_color, dim_text, accent_color);
            }
            RecorderView::Recordings => {
                self.render_recordings_view(surface, x, content_y, w, h - 50, text_color, dim_text, selected_bg);
            }
        }
    }
}

impl ScreenRecorder {
    fn render_main_view(&self, surface: &mut Surface, x: usize, y: usize, w: usize, h: usize,
                        text_color: Color, dim_text: Color, accent_color: Color) {
        // Preview area
        let preview_height = 200;
        let preview_y = y + 20;
        surface.fill_rect(x + 20, preview_y, w - 40, preview_height, Color::new(20, 20, 25));
        surface.draw_rect(x + 20, preview_y, w - 40, preview_height, Color::new(60, 60, 70));

        // Preview label
        let preview_text = match self.state {
            RecordingState::Countdown => format!("Starting in {}...", self.countdown_remaining),
            RecordingState::Recording => "● Recording".to_string(),
            RecordingState::Paused => "⏸ Paused".to_string(),
            _ => "Preview".to_string(),
        };
        let text_x = x + 20 + (w - 40 - preview_text.len() * 8) / 2;
        let color = if self.state == RecordingState::Recording { accent_color } else { dim_text };
        draw_string(surface, text_x, preview_y + preview_height / 2 - 8, &preview_text, color);

        // Stats area (when recording)
        let stats_y = preview_y + preview_height + 20;
        if self.state.is_active() {
            // Duration
            draw_string(surface, x + 40, stats_y, "Duration:", dim_text);
            draw_string(surface, x + 120, stats_y, &self.stats.format_duration(), text_color);

            // File size
            draw_string(surface, x + 220, stats_y, "Size:", dim_text);
            draw_string(surface, x + 280, stats_y, &self.stats.format_file_size(), text_color);

            // Frames
            draw_string(surface, x + 40, stats_y + 25, "Frames:", dim_text);
            let frames_text = format!("{}", self.stats.frames_captured);
            draw_string(surface, x + 120, stats_y + 25, &frames_text, text_color);

            // Drop rate
            let drop_text = format!("{:.1}% dropped", self.stats.drop_rate());
            draw_string(surface, x + 220, stats_y + 25, &drop_text, dim_text);

            // CPU/Memory
            let cpu_text = format!("CPU: {:.0}%", self.stats.cpu_usage);
            draw_string(surface, x + 40, stats_y + 50, &cpu_text, dim_text);

            let mem_mb = self.stats.memory_usage / (1024 * 1024);
            let mem_text = format!("Memory: {} MB", mem_mb);
            draw_string(surface, x + 160, stats_y + 50, &mem_text, dim_text);
        } else {
            // Settings summary
            draw_string(surface, x + 40, stats_y, "Format:", dim_text);
            draw_string(surface, x + 120, stats_y, self.settings.format.name(), text_color);

            draw_string(surface, x + 40, stats_y + 25, "Quality:", dim_text);
            draw_string(surface, x + 120, stats_y + 25, self.settings.quality.name(), text_color);

            draw_string(surface, x + 40, stats_y + 50, "FPS:", dim_text);
            draw_string(surface, x + 120, stats_y + 50, self.settings.frame_rate.name(), text_color);

            draw_string(surface, x + 40, stats_y + 75, "Audio:", dim_text);
            draw_string(surface, x + 120, stats_y + 75, self.settings.audio.name(), text_color);

            draw_string(surface, x + 250, stats_y, "Region:", dim_text);
            draw_string(surface, x + 320, stats_y, self.settings.region.region_type.name(), text_color);

            let region_size = format!("{}x{}", self.settings.region.width, self.settings.region.height);
            draw_string(surface, x + 250, stats_y + 25, "Size:", dim_text);
            draw_string(surface, x + 320, stats_y + 25, &region_size, text_color);
        }

        // Control buttons
        let btn_y = y + h - 70;
        let center_x = x + w / 2;

        // Main record/stop button
        let btn_color = if self.state.is_active() || self.state == RecordingState::Countdown {
            Color::new(150, 50, 50)  // Stop color
        } else {
            accent_color  // Record color
        };

        surface.fill_rect(center_x - 40, btn_y, 80, 40, btn_color);
        let btn_text = if self.state.is_active() || self.state == RecordingState::Countdown {
            "Stop"
        } else {
            "Record"
        };
        let btn_text_x = center_x - btn_text.len() * 4;
        draw_string(surface, btn_text_x, btn_y + 12, btn_text, text_color);

        // Pause button (when recording)
        if self.state.is_active() {
            surface.fill_rect(center_x + 60, btn_y, 60, 40, Color::new(60, 60, 70));
            let pause_text = if self.state == RecordingState::Paused { "Resume" } else { "Pause" };
            draw_string(surface, center_x + 65, btn_y + 12, pause_text, text_color);
        }

        // Keyboard shortcuts hint
        let hint_y = btn_y + 50;
        draw_string(surface, x + 20, hint_y, "R: Record  S: Stop  Space: Pause  Esc: Cancel", dim_text);
    }

    fn render_settings_view(&self, surface: &mut Surface, x: usize, y: usize, w: usize,
                            text_color: Color, dim_text: Color, accent_color: Color) {
        let setting_height = 35;
        let label_x = x + 30;
        let value_x = x + 180;

        // Format
        draw_string(surface, label_x, y + 10, "Video Format:", dim_text);
        draw_string(surface, value_x, y + 10, self.settings.format.name(), text_color);

        // Quality
        draw_string(surface, label_x, y + 10 + setting_height, "Quality:", dim_text);
        draw_string(surface, value_x, y + 10 + setting_height, self.settings.quality.name(), text_color);

        // Frame rate
        draw_string(surface, label_x, y + 10 + setting_height * 2, "Frame Rate:", dim_text);
        draw_string(surface, value_x, y + 10 + setting_height * 2, self.settings.frame_rate.name(), text_color);

        // Audio
        draw_string(surface, label_x, y + 10 + setting_height * 3, "Audio Source:", dim_text);
        draw_string(surface, value_x, y + 10 + setting_height * 3, self.settings.audio.name(), text_color);

        // Region
        draw_string(surface, label_x, y + 10 + setting_height * 4, "Record Region:", dim_text);
        draw_string(surface, value_x, y + 10 + setting_height * 4, self.settings.region.region_type.name(), text_color);

        // Cursor settings
        draw_string(surface, label_x, y + 10 + setting_height * 5, "Show Cursor:", dim_text);
        let cursor_text = if self.settings.show_cursor { "Yes" } else { "No" };
        draw_string(surface, value_x, y + 10 + setting_height * 5, cursor_text, text_color);

        draw_string(surface, label_x, y + 10 + setting_height * 6, "Highlight Clicks:", dim_text);
        let clicks_text = if self.settings.highlight_clicks { "Yes" } else { "No" };
        draw_string(surface, value_x, y + 10 + setting_height * 6, clicks_text, text_color);

        // Countdown
        draw_string(surface, label_x, y + 10 + setting_height * 7, "Countdown:", dim_text);
        let countdown_text = format!("{} seconds", self.settings.countdown);
        draw_string(surface, value_x, y + 10 + setting_height * 7, &countdown_text, text_color);

        // Output directory
        draw_string(surface, label_x, y + 10 + setting_height * 8, "Output Folder:", dim_text);
        let dir_display = if self.settings.output_dir.len() > 35 {
            format!("...{}", &self.settings.output_dir[self.settings.output_dir.len()-32..])
        } else {
            self.settings.output_dir.clone()
        };
        draw_string(surface, value_x, y + 10 + setting_height * 8, &dir_display, text_color);

        // Hardware encoding
        draw_string(surface, label_x, y + 10 + setting_height * 9, "HW Encoding:", dim_text);
        let hw_text = if self.settings.hw_encoding { "Enabled (GPU)" } else { "Disabled (CPU)" };
        let hw_color = if self.settings.hw_encoding { accent_color } else { dim_text };
        draw_string(surface, value_x, y + 10 + setting_height * 9, hw_text, hw_color);

        // Auto-stop
        draw_string(surface, label_x, y + 10 + setting_height * 10, "Auto-stop:", dim_text);
        let auto_text = if self.settings.auto_stop > 0 {
            format!("After {} seconds", self.settings.auto_stop)
        } else {
            "Disabled".to_string()
        };
        draw_string(surface, value_x, y + 10 + setting_height * 10, &auto_text, text_color);
    }

    fn render_recordings_view(&self, surface: &mut Surface, x: usize, y: usize, w: usize, h: usize,
                              text_color: Color, dim_text: Color, selected_bg: Color) {
        // Header
        let count_text = format!("{} recordings", self.recordings.len());
        draw_string(surface, x + 20, y + 10, "Recent Recordings", text_color);
        draw_string(surface, x + w - count_text.len() * 8 - 20, y + 10, &count_text, dim_text);

        // Column headers
        let header_y = y + 40;
        draw_string(surface, x + 20, header_y, "Name", dim_text);
        draw_string(surface, x + 300, header_y, "Duration", dim_text);
        draw_string(surface, x + 400, header_y, "Size", dim_text);
        draw_string(surface, x + 500, header_y, "Resolution", dim_text);

        // Recordings list
        let list_y = y + 65;
        let item_height = 45;
        let visible = self.get_visible_recordings();

        for (i, rec) in self.recordings.iter().skip(self.scroll_offset).take(visible).enumerate() {
            let item_y = list_y + i * item_height;
            let actual_idx = i + self.scroll_offset;

            // Selection
            if Some(actual_idx) == self.selected_recording {
                surface.fill_rect(x + 10, item_y, w - 20, item_height - 5, selected_bg);
            }

            // Format icon/indicator
            let format_color = match rec.format {
                VideoFormat::Mp4 => Color::new(100, 150, 255),
                VideoFormat::Webm => Color::new(100, 255, 150),
                VideoFormat::Mkv => Color::new(255, 150, 100),
                VideoFormat::Avi => Color::new(200, 200, 100),
                VideoFormat::Gif => Color::new(255, 100, 200),
            };
            surface.fill_rect(x + 20, item_y + 5, 4, item_height - 15, format_color);

            // Filename
            let name = if rec.filename.len() > 32 {
                format!("{}...", &rec.filename[..29])
            } else {
                rec.filename.clone()
            };
            draw_string(surface, x + 35, item_y + 8, &name, text_color);

            // Format label
            draw_string(surface, x + 35, item_y + 25, rec.format.extension().to_uppercase().as_str(), dim_text);

            // Duration
            draw_string(surface, x + 300, item_y + 15, &rec.format_duration(), text_color);

            // Size
            let size_text = if rec.file_size < 1024 * 1024 {
                format!("{:.1} KB", rec.file_size as f64 / 1024.0)
            } else if rec.file_size < 1024 * 1024 * 1024 {
                format!("{:.1} MB", rec.file_size as f64 / (1024.0 * 1024.0))
            } else {
                format!("{:.2} GB", rec.file_size as f64 / (1024.0 * 1024.0 * 1024.0))
            };
            draw_string(surface, x + 400, item_y + 15, &size_text, text_color);

            // Resolution
            let res_text = format!("{}x{}", rec.width, rec.height);
            draw_string(surface, x + 500, item_y + 15, &res_text, dim_text);
        }

        // Empty state
        if self.recordings.is_empty() {
            let msg = "No recordings yet";
            let msg_x = x + (w - msg.len() * 8) / 2;
            draw_string(surface, msg_x, y + h / 2, msg, dim_text);
        }
    }
}

// Helper functions for text rendering
fn draw_char(surface: &mut Surface, x: usize, y: usize, c: char, color: Color) {
    use crate::drivers::font::DEFAULT_FONT;
    if let Some(glyph) = DEFAULT_FONT.get_glyph(c) {
        for row in 0..DEFAULT_FONT.height {
            let byte = glyph[row];
            for col in 0..DEFAULT_FONT.width {
                if (byte >> (DEFAULT_FONT.width - 1 - col)) & 1 != 0 {
                    surface.set_pixel(x + col, y + row, color);
                }
            }
        }
    }
}

fn draw_string(surface: &mut Surface, x: usize, y: usize, s: &str, color: Color) {
    for (i, c) in s.chars().enumerate() {
        draw_char(surface, x + i * 8, y, c, color);
    }
}
