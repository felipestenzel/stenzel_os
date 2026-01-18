//! Video Player Application
//!
//! Media player supporting common video formats with playback controls.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use spin::Mutex;
use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use crate::gui::widgets::{Widget, WidgetId, WidgetEvent, Bounds, MouseButton};

/// Global video player state
static VIDEO_PLAYER_STATE: Mutex<Option<VideoPlayerState>> = Mutex::new(None);

/// Video player state
pub struct VideoPlayerState {
    /// Currently playing media
    pub current_media: Option<MediaInfo>,
    /// Playback state
    pub playback_state: PlaybackState,
    /// Volume (0-100)
    pub volume: u32,
    /// Muted
    pub muted: bool,
    /// Playback speed (1.0 = normal)
    pub speed: f32,
    /// Loop mode
    pub loop_mode: LoopMode,
    /// Playlist
    pub playlist: Vec<MediaInfo>,
    /// Current playlist index
    pub playlist_index: usize,
    /// Shuffle enabled
    pub shuffle: bool,
    /// Recent files
    pub recent: Vec<String>,
    /// Subtitle track
    pub subtitle_track: Option<SubtitleTrack>,
    /// Audio track index
    pub audio_track: usize,
}

/// Media information
#[derive(Debug, Clone)]
pub struct MediaInfo {
    /// File path
    pub path: String,
    /// Title
    pub title: String,
    /// Duration in seconds
    pub duration: f64,
    /// Current position in seconds
    pub position: f64,
    /// Video codec
    pub video_codec: Option<String>,
    /// Audio codec
    pub audio_codec: Option<String>,
    /// Width
    pub width: u32,
    /// Height
    pub height: u32,
    /// Frame rate
    pub fps: f32,
    /// Bitrate (kbps)
    pub bitrate: u32,
    /// Container format
    pub container: ContainerFormat,
    /// Has video stream
    pub has_video: bool,
    /// Has audio stream
    pub has_audio: bool,
    /// Audio tracks
    pub audio_tracks: Vec<AudioTrack>,
    /// Subtitle tracks
    pub subtitle_tracks: Vec<SubtitleTrack>,
}

/// Container format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerFormat {
    Mp4,
    Mkv,
    Avi,
    Webm,
    Mov,
    Flv,
    Wmv,
    Ogg,
    Unknown,
}

impl ContainerFormat {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "mp4" | "m4v" => ContainerFormat::Mp4,
            "mkv" => ContainerFormat::Mkv,
            "avi" => ContainerFormat::Avi,
            "webm" => ContainerFormat::Webm,
            "mov" => ContainerFormat::Mov,
            "flv" => ContainerFormat::Flv,
            "wmv" => ContainerFormat::Wmv,
            "ogg" | "ogv" => ContainerFormat::Ogg,
            _ => ContainerFormat::Unknown,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            ContainerFormat::Mp4 => "MP4",
            ContainerFormat::Mkv => "MKV",
            ContainerFormat::Avi => "AVI",
            ContainerFormat::Webm => "WebM",
            ContainerFormat::Mov => "QuickTime",
            ContainerFormat::Flv => "Flash Video",
            ContainerFormat::Wmv => "Windows Media",
            ContainerFormat::Ogg => "Ogg",
            ContainerFormat::Unknown => "Unknown",
        }
    }
}

/// Audio track
#[derive(Debug, Clone)]
pub struct AudioTrack {
    /// Track index
    pub index: usize,
    /// Language code
    pub language: Option<String>,
    /// Codec name
    pub codec: String,
    /// Channels
    pub channels: u8,
    /// Sample rate
    pub sample_rate: u32,
    /// Is default track
    pub default: bool,
}

/// Subtitle track
#[derive(Debug, Clone)]
pub struct SubtitleTrack {
    /// Track index
    pub index: usize,
    /// Language code
    pub language: Option<String>,
    /// Format (SRT, ASS, etc.)
    pub format: SubtitleFormat,
    /// Is default track
    pub default: bool,
    /// Is forced track
    pub forced: bool,
}

/// Subtitle format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubtitleFormat {
    Srt,
    Ass,
    Ssa,
    VobSub,
    PgsSub,
    Unknown,
}

/// Playback state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
    Buffering,
    Error,
}

/// Loop mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopMode {
    Off,
    Single,
    Playlist,
}

impl LoopMode {
    pub fn name(&self) -> &'static str {
        match self {
            LoopMode::Off => "Off",
            LoopMode::Single => "Repeat One",
            LoopMode::Playlist => "Repeat All",
        }
    }
}

/// Video player error
#[derive(Debug, Clone)]
pub enum VideoPlayerError {
    NotInitialized,
    FileNotFound(String),
    UnsupportedFormat(String),
    DecoderError(String),
    AudioError(String),
    NoMediaLoaded,
}

/// Initialize video player
pub fn init() {
    let mut state = VIDEO_PLAYER_STATE.lock();
    if state.is_some() {
        return;
    }

    *state = Some(VideoPlayerState {
        current_media: None,
        playback_state: PlaybackState::Stopped,
        volume: 100,
        muted: false,
        speed: 1.0,
        loop_mode: LoopMode::Off,
        playlist: Vec::new(),
        playlist_index: 0,
        shuffle: false,
        recent: Vec::new(),
        subtitle_track: None,
        audio_track: 0,
    });

    crate::kprintln!("videoplayer: initialized");
}

/// Open a media file
pub fn open(path: &str) -> Result<MediaInfo, VideoPlayerError> {
    let mut state = VIDEO_PLAYER_STATE.lock();
    let s = state.as_mut().ok_or(VideoPlayerError::NotInitialized)?;

    // Get file extension
    let ext = path.rsplit('.').next().unwrap_or("");
    let container = ContainerFormat::from_extension(ext);

    // Parse media file (simplified - would use actual demuxer)
    let media = MediaInfo {
        path: path.to_string(),
        title: path.rsplit('/').next().unwrap_or(path).to_string(),
        duration: 0.0, // Would be read from file
        position: 0.0,
        video_codec: Some("H.264".to_string()),
        audio_codec: Some("AAC".to_string()),
        width: 1920,
        height: 1080,
        fps: 24.0,
        bitrate: 5000,
        container,
        has_video: true,
        has_audio: true,
        audio_tracks: vec![AudioTrack {
            index: 0,
            language: Some("eng".to_string()),
            codec: "AAC".to_string(),
            channels: 2,
            sample_rate: 48000,
            default: true,
        }],
        subtitle_tracks: Vec::new(),
    };

    // Add to recent
    if !s.recent.contains(&path.to_string()) {
        s.recent.insert(0, path.to_string());
        if s.recent.len() > 20 {
            s.recent.pop();
        }
    }

    s.current_media = Some(media.clone());
    s.playback_state = PlaybackState::Paused;

    Ok(media)
}

/// Play current media
pub fn play() -> Result<(), VideoPlayerError> {
    let mut state = VIDEO_PLAYER_STATE.lock();
    let s = state.as_mut().ok_or(VideoPlayerError::NotInitialized)?;

    if s.current_media.is_none() {
        return Err(VideoPlayerError::NoMediaLoaded);
    }

    s.playback_state = PlaybackState::Playing;
    Ok(())
}

/// Pause playback
pub fn pause() {
    let mut state = VIDEO_PLAYER_STATE.lock();
    if let Some(ref mut s) = *state {
        if s.playback_state == PlaybackState::Playing {
            s.playback_state = PlaybackState::Paused;
        }
    }
}

/// Stop playback
pub fn stop() {
    let mut state = VIDEO_PLAYER_STATE.lock();
    if let Some(ref mut s) = *state {
        s.playback_state = PlaybackState::Stopped;
        if let Some(ref mut media) = s.current_media {
            media.position = 0.0;
        }
    }
}

/// Toggle play/pause
pub fn toggle_play_pause() {
    let mut state = VIDEO_PLAYER_STATE.lock();
    if let Some(ref mut s) = *state {
        match s.playback_state {
            PlaybackState::Playing => s.playback_state = PlaybackState::Paused,
            PlaybackState::Paused => s.playback_state = PlaybackState::Playing,
            PlaybackState::Stopped => {
                if s.current_media.is_some() {
                    s.playback_state = PlaybackState::Playing;
                }
            }
            _ => {}
        }
    }
}

/// Seek to position (seconds)
pub fn seek(position: f64) {
    let mut state = VIDEO_PLAYER_STATE.lock();
    if let Some(ref mut s) = *state {
        if let Some(ref mut media) = s.current_media {
            media.position = position.max(0.0).min(media.duration);
        }
    }
}

/// Seek relative (seconds)
pub fn seek_relative(delta: f64) {
    let mut state = VIDEO_PLAYER_STATE.lock();
    if let Some(ref mut s) = *state {
        if let Some(ref mut media) = s.current_media {
            let new_pos = (media.position + delta).max(0.0).min(media.duration);
            media.position = new_pos;
        }
    }
}

/// Set volume (0-100)
pub fn set_volume(volume: u32) {
    let mut state = VIDEO_PLAYER_STATE.lock();
    if let Some(ref mut s) = *state {
        s.volume = volume.min(100);
    }
}

/// Get volume
pub fn get_volume() -> u32 {
    let state = VIDEO_PLAYER_STATE.lock();
    state.as_ref().map(|s| s.volume).unwrap_or(100)
}

/// Toggle mute
pub fn toggle_mute() {
    let mut state = VIDEO_PLAYER_STATE.lock();
    if let Some(ref mut s) = *state {
        s.muted = !s.muted;
    }
}

/// Set playback speed
pub fn set_speed(speed: f32) {
    let mut state = VIDEO_PLAYER_STATE.lock();
    if let Some(ref mut s) = *state {
        s.speed = speed.max(0.25).min(4.0);
    }
}

/// Set loop mode
pub fn set_loop_mode(mode: LoopMode) {
    let mut state = VIDEO_PLAYER_STATE.lock();
    if let Some(ref mut s) = *state {
        s.loop_mode = mode;
    }
}

/// Toggle shuffle
pub fn toggle_shuffle() {
    let mut state = VIDEO_PLAYER_STATE.lock();
    if let Some(ref mut s) = *state {
        s.shuffle = !s.shuffle;
    }
}

/// Add to playlist
pub fn add_to_playlist(media: MediaInfo) {
    let mut state = VIDEO_PLAYER_STATE.lock();
    if let Some(ref mut s) = *state {
        s.playlist.push(media);
    }
}

/// Clear playlist
pub fn clear_playlist() {
    let mut state = VIDEO_PLAYER_STATE.lock();
    if let Some(ref mut s) = *state {
        s.playlist.clear();
        s.playlist_index = 0;
    }
}

/// Next in playlist
pub fn next() {
    let mut state = VIDEO_PLAYER_STATE.lock();
    if let Some(ref mut s) = *state {
        if s.playlist.is_empty() {
            return;
        }

        s.playlist_index = (s.playlist_index + 1) % s.playlist.len();
        if let Some(media) = s.playlist.get(s.playlist_index).cloned() {
            s.current_media = Some(media);
            s.playback_state = PlaybackState::Playing;
        }
    }
}

/// Previous in playlist
pub fn previous() {
    let mut state = VIDEO_PLAYER_STATE.lock();
    if let Some(ref mut s) = *state {
        if s.playlist.is_empty() {
            return;
        }

        if s.playlist_index == 0 {
            s.playlist_index = s.playlist.len() - 1;
        } else {
            s.playlist_index -= 1;
        }

        if let Some(media) = s.playlist.get(s.playlist_index).cloned() {
            s.current_media = Some(media);
            s.playback_state = PlaybackState::Playing;
        }
    }
}

/// Set subtitle track
pub fn set_subtitle_track(track: Option<SubtitleTrack>) {
    let mut state = VIDEO_PLAYER_STATE.lock();
    if let Some(ref mut s) = *state {
        s.subtitle_track = track;
    }
}

/// Set audio track
pub fn set_audio_track(index: usize) {
    let mut state = VIDEO_PLAYER_STATE.lock();
    if let Some(ref mut s) = *state {
        s.audio_track = index;
    }
}

/// Get current media info
pub fn get_current_media() -> Option<MediaInfo> {
    let state = VIDEO_PLAYER_STATE.lock();
    state.as_ref().and_then(|s| s.current_media.clone())
}

/// Get playback state
pub fn get_playback_state() -> PlaybackState {
    let state = VIDEO_PLAYER_STATE.lock();
    state.as_ref().map(|s| s.playback_state).unwrap_or(PlaybackState::Stopped)
}

/// Get recent files
pub fn get_recent() -> Vec<String> {
    let state = VIDEO_PLAYER_STATE.lock();
    state.as_ref().map(|s| s.recent.clone()).unwrap_or_default()
}

/// Format time (seconds) as HH:MM:SS
pub fn format_time(seconds: f64) -> String {
    let total_secs = seconds as u64;
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let secs = total_secs % 60;

    if hours > 0 {
        alloc::format!("{:02}:{:02}:{:02}", hours, minutes, secs)
    } else {
        alloc::format!("{:02}:{:02}", minutes, secs)
    }
}

// Theme colors
fn window_background() -> Color { Color::new(30, 30, 30) }
fn control_background() -> Color { Color::new(45, 45, 45) }
fn accent_color() -> Color { Color::new(0, 120, 215) }
fn text_color() -> Color { Color::new(240, 240, 240) }
fn _progress_bg() -> Color { Color::new(60, 60, 60) }

/// Video player widget
pub struct VideoPlayer {
    id: WidgetId,
    bounds: Bounds,
    enabled: bool,
    visible: bool,
    fullscreen: bool,
    controls_visible: bool,
    controls_timeout: u64,
    seeking: bool,
    seek_position: f64,
}

impl VideoPlayer {
    pub fn new(id: WidgetId, bounds: Bounds) -> Self {
        Self {
            id,
            bounds,
            enabled: true,
            visible: true,
            fullscreen: false,
            controls_visible: true,
            controls_timeout: 0,
            seeking: false,
            seek_position: 0.0,
        }
    }

    pub fn toggle_fullscreen(&mut self) {
        self.fullscreen = !self.fullscreen;
    }

    pub fn show_controls(&mut self) {
        self.controls_visible = true;
        self.controls_timeout = crate::time::uptime_secs() + 3;
    }

    pub fn update(&mut self) {
        // Hide controls after timeout
        if self.controls_visible && crate::time::uptime_secs() > self.controls_timeout {
            self.controls_visible = false;
        }
    }

    fn render_controls(&self, surface: &mut Surface, x: usize, y: usize, w: usize, h: usize) {
        let control_height = 60;
        let control_y = y + h - control_height;

        // Control bar background
        surface.fill_rect(x, control_y, w, control_height, control_background());

        // Get current state
        let state = VIDEO_PLAYER_STATE.lock();
        if let Some(ref s) = *state {
            // Progress bar
            let progress_y = control_y + 5;
            let progress_w = w - 40;
            surface.fill_rect(x + 20, progress_y, progress_w, 6, Color::new(60, 60, 60));

            if let Some(ref media) = s.current_media {
                if media.duration > 0.0 {
                    let pos = if self.seeking { self.seek_position } else { media.position };
                    let progress = (pos / media.duration * progress_w as f64) as usize;
                    surface.fill_rect(x + 20, progress_y, progress.min(progress_w), 6, accent_color());

                    // Progress handle
                    let handle_x = x + 20 + progress.min(progress_w);
                    surface.fill_rect(handle_x.saturating_sub(4), progress_y - 2, 8, 10, text_color());
                }
            }

            // Play/Pause button
            let btn_y = control_y + 25;
            let btn_size = 30;
            let play_x = x + w / 2 - btn_size / 2;

            if s.playback_state == PlaybackState::Playing {
                // Pause icon (two bars)
                surface.fill_rect(play_x + 8, btn_y, 5, btn_size - 5, text_color());
                surface.fill_rect(play_x + 17, btn_y, 5, btn_size - 5, text_color());
            } else {
                // Play icon (triangle)
                for i in 0..15 {
                    let bar_h = 25 - i * 2;
                    if bar_h > 0 {
                        surface.fill_rect(play_x + 8 + i, btn_y + i, 2, bar_h as usize, text_color());
                    }
                }
            }

            // Previous button
            let prev_x = play_x - 50;
            surface.fill_rect(prev_x, btn_y + 5, 5, 20, text_color());
            for i in 0..10 {
                surface.fill_rect(prev_x + 5 + i, btn_y + 5 + i, 2, (20 - i * 2).max(1), text_color());
            }

            // Next button
            let next_x = play_x + btn_size + 20;
            for i in 0..10 {
                surface.fill_rect(next_x + i, btn_y + 5 + i, 2, (20 - i * 2).max(1), text_color());
            }
            surface.fill_rect(next_x + 15, btn_y + 5, 5, 20, text_color());

            // Volume
            let vol_x = x + w - 120;
            let vol_w = 80;
            surface.fill_rect(vol_x, btn_y + 10, vol_w, 6, Color::new(60, 60, 60));
            let vol_filled = if s.muted { 0 } else { (s.volume as usize * vol_w) / 100 };
            surface.fill_rect(vol_x, btn_y + 10, vol_filled, 6, accent_color());

            // Fullscreen button
            let fs_x = x + w - 30;
            surface.draw_rect(fs_x, btn_y + 5, 20, 20, text_color());
        }
    }
}

impl Widget for VideoPlayer {
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

        // Video area (black background)
        surface.fill_rect(x, y, w, h, window_background());

        // Would render video frame here
        // For now, show placeholder
        let state = VIDEO_PLAYER_STATE.lock();
        if state.as_ref().and_then(|s| s.current_media.as_ref()).is_none() {
            // No media - show placeholder
            let center_x = x + w / 2;
            let center_y = y + h / 2;

            // Play icon placeholder
            for i in 0..30usize {
                let bar_h = (50usize).saturating_sub(i * 2);
                if bar_h > 0 {
                    surface.fill_rect(center_x - 15 + i, center_y - 25 + i, 3, bar_h, Color::new(80, 80, 80));
                }
            }
        }
        drop(state);

        // Render controls if visible
        if self.controls_visible {
            self.render_controls(surface, x, y, w, h);
        }
    }

    fn handle_event(&mut self, event: &WidgetEvent) -> bool {
        match event {
            WidgetEvent::MouseDown { x, y, button: MouseButton::Left } => {
                let rel_x = (*x - self.bounds.x) as usize;
                let rel_y = (*y - self.bounds.y) as usize;
                let h = self.bounds.height;
                let w = self.bounds.width;

                // Show controls on click
                self.show_controls();

                // Check if clicking on controls
                let control_height = 60;
                if rel_y >= h - control_height {
                    // Progress bar click
                    if rel_y < h - control_height + 15 && rel_x >= 20 && rel_x < w - 20 {
                        let progress_w = w - 40;
                        let click_progress = (rel_x - 20) as f64 / progress_w as f64;
                        let state = VIDEO_PLAYER_STATE.lock();
                        if let Some(ref s) = *state {
                            if let Some(ref media) = s.current_media {
                                self.seeking = true;
                                self.seek_position = click_progress * media.duration;
                            }
                        }
                        return true;
                    }

                    // Play/Pause button
                    let btn_x = w / 2 - 15;
                    if rel_x >= btn_x && rel_x < btn_x + 30 && rel_y >= h - 40 {
                        toggle_play_pause();
                        return true;
                    }

                    // Previous button
                    if rel_x >= btn_x - 50 && rel_x < btn_x - 20 {
                        previous();
                        return true;
                    }

                    // Next button
                    if rel_x >= btn_x + 50 && rel_x < btn_x + 80 {
                        next();
                        return true;
                    }

                    // Fullscreen button
                    if rel_x >= w - 30 {
                        self.toggle_fullscreen();
                        return true;
                    }

                    return true;
                }

                // Double click on video area toggles fullscreen
                false
            }
            WidgetEvent::MouseUp { .. } => {
                if self.seeking {
                    seek(self.seek_position);
                    self.seeking = false;
                    return true;
                }
                false
            }
            WidgetEvent::MouseMove { x, .. } => {
                self.show_controls();

                if self.seeking {
                    let rel_x = (*x - self.bounds.x) as usize;
                    let w = self.bounds.width;
                    let progress_w = w - 40;
                    let click_progress = ((rel_x.saturating_sub(20)) as f64 / progress_w as f64).max(0.0).min(1.0);
                    let state = VIDEO_PLAYER_STATE.lock();
                    if let Some(ref s) = *state {
                        if let Some(ref media) = s.current_media {
                            self.seek_position = click_progress * media.duration;
                        }
                    }
                    return true;
                }
                false
            }
            WidgetEvent::KeyDown { key, .. } => {
                match *key {
                    0x39 => { // Space
                        toggle_play_pause();
                        return true;
                    }
                    0x4B => { // Left
                        seek_relative(-5.0);
                        return true;
                    }
                    0x4D => { // Right
                        seek_relative(5.0);
                        return true;
                    }
                    0x48 => { // Up
                        let vol = get_volume();
                        set_volume(vol.saturating_add(5));
                        return true;
                    }
                    0x50 => { // Down
                        let vol = get_volume();
                        set_volume(vol.saturating_sub(5));
                        return true;
                    }
                    0x3A => { // F
                        self.toggle_fullscreen();
                        return true;
                    }
                    0x32 => { // M
                        toggle_mute();
                        return true;
                    }
                    _ => {}
                }
                false
            }
            _ => false,
        }
    }
}
