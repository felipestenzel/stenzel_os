//! Music Player Application
//!
//! Full-featured music player with playlist support, equalizer,
//! visualizations, and library management.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use alloc::collections::BTreeMap;

use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use crate::gui::widgets::{Widget, WidgetId, WidgetEvent, Bounds, MouseButton, theme};

/// Supported audio formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    Mp3,
    Flac,
    Wav,
    Ogg,
    Aac,
    M4a,
    Wma,
    Opus,
    Unknown,
}

impl AudioFormat {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "mp3" => AudioFormat::Mp3,
            "flac" => AudioFormat::Flac,
            "wav" => AudioFormat::Wav,
            "ogg" | "oga" => AudioFormat::Ogg,
            "aac" => AudioFormat::Aac,
            "m4a" => AudioFormat::M4a,
            "wma" => AudioFormat::Wma,
            "opus" => AudioFormat::Opus,
            _ => AudioFormat::Unknown,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            AudioFormat::Mp3 => "MP3",
            AudioFormat::Flac => "FLAC",
            AudioFormat::Wav => "WAV",
            AudioFormat::Ogg => "Ogg Vorbis",
            AudioFormat::Aac => "AAC",
            AudioFormat::M4a => "M4A",
            AudioFormat::Wma => "WMA",
            AudioFormat::Opus => "Opus",
            AudioFormat::Unknown => "Unknown",
        }
    }

    pub fn is_lossless(&self) -> bool {
        matches!(self, AudioFormat::Flac | AudioFormat::Wav)
    }
}

/// Playback state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerState {
    Stopped,
    Playing,
    Paused,
    Loading,
    Error,
}

/// Repeat mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatMode {
    Off,
    All,
    One,
}

impl RepeatMode {
    pub fn next(&self) -> RepeatMode {
        match self {
            RepeatMode::Off => RepeatMode::All,
            RepeatMode::All => RepeatMode::One,
            RepeatMode::One => RepeatMode::Off,
        }
    }

    pub fn icon(&self) -> char {
        match self {
            RepeatMode::Off => 'R',
            RepeatMode::All => 'A',
            RepeatMode::One => '1',
        }
    }
}

/// Shuffle mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShuffleMode {
    Off,
    On,
}

/// Audio track metadata
#[derive(Debug, Clone)]
pub struct TrackMetadata {
    /// Track title
    pub title: String,
    /// Artist name
    pub artist: String,
    /// Album name
    pub album: String,
    /// Album artist
    pub album_artist: String,
    /// Track number
    pub track_number: Option<u32>,
    /// Total tracks
    pub total_tracks: Option<u32>,
    /// Disc number
    pub disc_number: Option<u32>,
    /// Year
    pub year: Option<u32>,
    /// Genre
    pub genre: String,
    /// Duration in seconds
    pub duration: u32,
    /// Bitrate in kbps
    pub bitrate: u32,
    /// Sample rate in Hz
    pub sample_rate: u32,
    /// Channels
    pub channels: u8,
    /// Album art path
    pub album_art: Option<String>,
    /// Lyrics
    pub lyrics: Option<String>,
    /// Comment
    pub comment: String,
    /// Composer
    pub composer: String,
}

impl TrackMetadata {
    pub fn new() -> Self {
        TrackMetadata {
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            album_artist: String::new(),
            track_number: None,
            total_tracks: None,
            disc_number: None,
            year: None,
            genre: String::new(),
            duration: 0,
            bitrate: 0,
            sample_rate: 44100,
            channels: 2,
            album_art: None,
            lyrics: None,
            comment: String::new(),
            composer: String::new(),
        }
    }

    pub fn format_duration(&self) -> String {
        let minutes = self.duration / 60;
        let seconds = self.duration % 60;
        format!("{}:{:02}", minutes, seconds)
    }
}

impl Default for TrackMetadata {
    fn default() -> Self {
        Self::new()
    }
}

/// A single audio track
#[derive(Debug, Clone)]
pub struct Track {
    /// Unique track ID
    pub id: u64,
    /// File path
    pub path: String,
    /// File format
    pub format: AudioFormat,
    /// File size in bytes
    pub file_size: u64,
    /// Metadata
    pub metadata: TrackMetadata,
    /// Last played timestamp
    pub last_played: Option<u64>,
    /// Play count
    pub play_count: u32,
    /// Rating (0-5)
    pub rating: u8,
    /// Is favorite
    pub is_favorite: bool,
}

impl Track {
    pub fn new(id: u64, path: &str) -> Self {
        let extension = path.rsplit('.').next().unwrap_or("");
        let format = AudioFormat::from_extension(extension);
        let filename = path.rsplit('/').next().unwrap_or(path);

        let mut metadata = TrackMetadata::new();
        metadata.title = filename.to_string();

        Track {
            id,
            path: path.to_string(),
            format,
            file_size: 0,
            metadata,
            last_played: None,
            play_count: 0,
            rating: 0,
            is_favorite: false,
        }
    }

    pub fn display_title(&self) -> &str {
        if self.metadata.title.is_empty() {
            self.path.rsplit('/').next().unwrap_or(&self.path)
        } else {
            &self.metadata.title
        }
    }

    pub fn display_artist(&self) -> &str {
        if self.metadata.artist.is_empty() {
            "Unknown Artist"
        } else {
            &self.metadata.artist
        }
    }
}

/// Playlist
#[derive(Debug, Clone)]
pub struct Playlist {
    /// Playlist ID
    pub id: u64,
    /// Playlist name
    pub name: String,
    /// Track IDs in order
    pub tracks: Vec<u64>,
    /// Creation time
    pub created: u64,
    /// Last modified time
    pub modified: u64,
    /// Is smart playlist
    pub is_smart: bool,
}

impl Playlist {
    pub fn new(id: u64, name: &str) -> Self {
        Playlist {
            id,
            name: name.to_string(),
            tracks: Vec::new(),
            created: 0,
            modified: 0,
            is_smart: false,
        }
    }
}

/// Equalizer preset
#[derive(Debug, Clone)]
pub struct EqualizerPreset {
    pub name: String,
    /// 10-band EQ values (-12 to +12 dB)
    pub bands: [i8; 10],
    /// Preamp (-12 to +12 dB)
    pub preamp: i8,
}

impl EqualizerPreset {
    pub fn flat() -> Self {
        EqualizerPreset {
            name: "Flat".to_string(),
            bands: [0; 10],
            preamp: 0,
        }
    }

    pub fn rock() -> Self {
        EqualizerPreset {
            name: "Rock".to_string(),
            bands: [4, 3, 2, 0, -1, 0, 2, 3, 4, 4],
            preamp: 0,
        }
    }

    pub fn pop() -> Self {
        EqualizerPreset {
            name: "Pop".to_string(),
            bands: [-1, 2, 4, 5, 4, 2, 0, 0, 0, 0],
            preamp: 0,
        }
    }

    pub fn jazz() -> Self {
        EqualizerPreset {
            name: "Jazz".to_string(),
            bands: [3, 2, 0, 1, 2, 3, 3, 3, 4, 4],
            preamp: 0,
        }
    }

    pub fn classical() -> Self {
        EqualizerPreset {
            name: "Classical".to_string(),
            bands: [4, 3, 2, 1, -1, -1, 0, 1, 2, 3],
            preamp: 0,
        }
    }

    pub fn bass_boost() -> Self {
        EqualizerPreset {
            name: "Bass Boost".to_string(),
            bands: [6, 5, 4, 3, 1, 0, 0, 0, 0, 0],
            preamp: -2,
        }
    }
}

/// Player view mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    NowPlaying,
    Library,
    Playlists,
    Queue,
    Equalizer,
}

/// Library view type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryView {
    Songs,
    Albums,
    Artists,
    Genres,
}

/// Music Player Widget
pub struct MusicPlayer {
    id: WidgetId,
    bounds: Bounds,
    enabled: bool,
    visible: bool,

    /// Player state
    state: PlayerState,

    /// Current track
    current_track: Option<Track>,

    /// Playback position in seconds
    position: u32,

    /// Volume (0-100)
    volume: u8,

    /// Muted
    muted: bool,

    /// Repeat mode
    repeat_mode: RepeatMode,

    /// Shuffle mode
    shuffle: ShuffleMode,

    /// Track library
    library: Vec<Track>,

    /// Playlists
    playlists: Vec<Playlist>,

    /// Play queue
    queue: Vec<u64>,

    /// Current queue index
    queue_index: usize,

    /// Current view mode
    view_mode: ViewMode,

    /// Library view type
    library_view: LibraryView,

    /// Equalizer preset
    equalizer: EqualizerPreset,

    /// Available presets
    presets: Vec<EqualizerPreset>,

    /// Visualization data (simulated)
    spectrum: [u8; 32],

    /// Selected item in list
    selected_index: Option<usize>,

    /// Scroll offset
    scroll_offset: usize,

    /// Next track ID
    next_track_id: u64,

    /// Next playlist ID
    next_playlist_id: u64,

    /// Simulated time
    current_time: u64,
}

impl MusicPlayer {
    pub fn new(id: WidgetId, x: isize, y: isize, width: usize, height: usize) -> Self {
        let mut player = MusicPlayer {
            id,
            bounds: Bounds { x, y, width, height },
            enabled: true,
            visible: true,
            state: PlayerState::Stopped,
            current_track: None,
            position: 0,
            volume: 80,
            muted: false,
            repeat_mode: RepeatMode::Off,
            shuffle: ShuffleMode::Off,
            library: Vec::new(),
            playlists: Vec::new(),
            queue: Vec::new(),
            queue_index: 0,
            view_mode: ViewMode::NowPlaying,
            library_view: LibraryView::Songs,
            equalizer: EqualizerPreset::flat(),
            presets: vec![
                EqualizerPreset::flat(),
                EqualizerPreset::rock(),
                EqualizerPreset::pop(),
                EqualizerPreset::jazz(),
                EqualizerPreset::classical(),
                EqualizerPreset::bass_boost(),
            ],
            spectrum: [0; 32],
            selected_index: None,
            scroll_offset: 0,
            next_track_id: 1,
            next_playlist_id: 1,
            current_time: 1737216000,
        };

        // Add sample tracks
        player.add_sample_library();
        player
    }

    fn add_sample_library(&mut self) {
        let tracks = [
            ("Bohemian Rhapsody", "Queen", "A Night at the Opera", 354, 320),
            ("Stairway to Heaven", "Led Zeppelin", "Led Zeppelin IV", 482, 320),
            ("Hotel California", "Eagles", "Hotel California", 391, 320),
            ("Comfortably Numb", "Pink Floyd", "The Wall", 382, 320),
            ("Sweet Child O' Mine", "Guns N' Roses", "Appetite for Destruction", 356, 320),
            ("Smells Like Teen Spirit", "Nirvana", "Nevermind", 301, 320),
            ("Nothing Else Matters", "Metallica", "Metallica", 388, 320),
            ("Imagine", "John Lennon", "Imagine", 183, 320),
            ("Billie Jean", "Michael Jackson", "Thriller", 294, 320),
            ("Like a Rolling Stone", "Bob Dylan", "Highway 61 Revisited", 369, 320),
        ];

        for (title, artist, album, duration, bitrate) in tracks {
            let path = format!("/music/{}/{}.mp3", artist, title);
            let mut track = Track::new(self.next_track_id, &path);
            track.metadata.title = title.to_string();
            track.metadata.artist = artist.to_string();
            track.metadata.album = album.to_string();
            track.metadata.duration = duration;
            track.metadata.bitrate = bitrate;
            track.metadata.sample_rate = 44100;
            track.metadata.channels = 2;
            self.next_track_id += 1;
            self.library.push(track);
        }

        // Create default playlist
        let mut favorites = Playlist::new(self.next_playlist_id, "Favorites");
        favorites.tracks.push(1);
        favorites.tracks.push(3);
        favorites.tracks.push(5);
        self.next_playlist_id += 1;
        self.playlists.push(favorites);
    }

    pub fn play(&mut self) {
        if self.current_track.is_some() {
            self.state = PlayerState::Playing;
        } else if !self.queue.is_empty() {
            self.play_from_queue(0);
        } else if !self.library.is_empty() {
            // Add all tracks to queue and play
            self.queue = self.library.iter().map(|t| t.id).collect();
            self.play_from_queue(0);
        }
    }

    pub fn pause(&mut self) {
        if self.state == PlayerState::Playing {
            self.state = PlayerState::Paused;
        }
    }

    pub fn toggle_play_pause(&mut self) {
        match self.state {
            PlayerState::Playing => self.pause(),
            PlayerState::Paused | PlayerState::Stopped => self.play(),
            _ => {}
        }
    }

    pub fn stop(&mut self) {
        self.state = PlayerState::Stopped;
        self.position = 0;
    }

    pub fn next(&mut self) {
        if self.queue.is_empty() {
            return;
        }

        let next_index = if self.shuffle == ShuffleMode::On {
            // Simple pseudo-random for demo
            (self.queue_index + 3) % self.queue.len()
        } else {
            (self.queue_index + 1) % self.queue.len()
        };

        self.play_from_queue(next_index);
    }

    pub fn previous(&mut self) {
        if self.queue.is_empty() {
            return;
        }

        // If more than 3 seconds into track, restart it
        if self.position > 3 {
            self.position = 0;
            return;
        }

        let prev_index = if self.queue_index == 0 {
            self.queue.len() - 1
        } else {
            self.queue_index - 1
        };

        self.play_from_queue(prev_index);
    }

    fn play_from_queue(&mut self, index: usize) {
        if index >= self.queue.len() {
            return;
        }

        self.queue_index = index;
        let track_id = self.queue[index];

        if let Some(track) = self.library.iter().find(|t| t.id == track_id) {
            self.current_track = Some(track.clone());
            self.position = 0;
            self.state = PlayerState::Playing;
        }
    }

    pub fn seek(&mut self, position: u32) {
        if let Some(track) = &self.current_track {
            self.position = position.min(track.metadata.duration);
        }
    }

    pub fn set_volume(&mut self, volume: u8) {
        self.volume = volume.min(100);
    }

    pub fn toggle_mute(&mut self) {
        self.muted = !self.muted;
    }

    pub fn toggle_repeat(&mut self) {
        self.repeat_mode = self.repeat_mode.next();
    }

    pub fn toggle_shuffle(&mut self) {
        self.shuffle = match self.shuffle {
            ShuffleMode::Off => ShuffleMode::On,
            ShuffleMode::On => ShuffleMode::Off,
        };
    }

    pub fn add_to_queue(&mut self, track_id: u64) {
        if !self.queue.contains(&track_id) {
            self.queue.push(track_id);
        }
    }

    pub fn clear_queue(&mut self) {
        self.queue.clear();
        self.queue_index = 0;
    }

    pub fn get_track(&self, id: u64) -> Option<&Track> {
        self.library.iter().find(|t| t.id == id)
    }

    pub fn set_equalizer(&mut self, preset: EqualizerPreset) {
        self.equalizer = preset;
    }

    fn update_spectrum(&mut self) {
        // Simulate spectrum analyzer
        for i in 0..32 {
            if self.state == PlayerState::Playing {
                // Random-ish values for visualization
                self.spectrum[i] = ((self.position as usize * 17 + i * 31) % 100) as u8;
            } else {
                self.spectrum[i] = 0;
            }
        }
    }

    fn get_visible_items(&self) -> usize {
        (self.bounds.height.saturating_sub(200)) / 28
    }

    fn format_time(&self, seconds: u32) -> String {
        let m = seconds / 60;
        let s = seconds % 60;
        format!("{}:{:02}", m, s)
    }
}

impl Widget for MusicPlayer {
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

                // Check bounds
                if px < self.bounds.x || px >= self.bounds.x + self.bounds.width as isize ||
                   py < self.bounds.y || py >= self.bounds.y + self.bounds.height as isize {
                    return false;
                }

                let local_x = (px - self.bounds.x) as usize;
                let local_y = (py - self.bounds.y) as usize;

                // Check control buttons (bottom area)
                let controls_y = self.bounds.height - 80;
                if local_y >= controls_y && local_y < controls_y + 50 {
                    let center_x = self.bounds.width / 2;

                    // Previous button
                    if local_x >= center_x - 100 && local_x < center_x - 60 {
                        self.previous();
                        return true;
                    }

                    // Play/Pause button
                    if local_x >= center_x - 30 && local_x < center_x + 30 {
                        self.toggle_play_pause();
                        return true;
                    }

                    // Next button
                    if local_x >= center_x + 60 && local_x < center_x + 100 {
                        self.next();
                        return true;
                    }

                    // Repeat button
                    if local_x >= center_x + 120 && local_x < center_x + 150 {
                        self.toggle_repeat();
                        return true;
                    }

                    // Shuffle button
                    if local_x >= center_x + 160 && local_x < center_x + 190 {
                        self.toggle_shuffle();
                        return true;
                    }
                }

                // Check view mode tabs (sidebar)
                if local_x < 150 {
                    let tab_height = 35;
                    let tab_y = (local_y.saturating_sub(50)) / tab_height;
                    self.view_mode = match tab_y {
                        0 => ViewMode::NowPlaying,
                        1 => ViewMode::Library,
                        2 => ViewMode::Playlists,
                        3 => ViewMode::Queue,
                        4 => ViewMode::Equalizer,
                        _ => self.view_mode,
                    };
                    return true;
                }

                // Check list items
                let list_start_y = 100;
                let item_height = 28;
                if local_y >= list_start_y && local_x >= 160 {
                    let clicked_idx = (local_y - list_start_y) / item_height + self.scroll_offset;
                    let item_count = match self.view_mode {
                        ViewMode::Library => self.library.len(),
                        ViewMode::Queue => self.queue.len(),
                        ViewMode::Playlists => self.playlists.len(),
                        _ => 0,
                    };

                    if clicked_idx < item_count {
                        self.selected_index = Some(clicked_idx);

                        // Double-click to play (simplified as single click for demo)
                        if self.view_mode == ViewMode::Library {
                            let track_id = self.library[clicked_idx].id;
                            self.queue = vec![track_id];
                            self.play_from_queue(0);
                        } else if self.view_mode == ViewMode::Queue {
                            self.play_from_queue(clicked_idx);
                        }
                        return true;
                    }
                }

                false
            }

            WidgetEvent::Scroll { delta_y, .. } => {
                let max_items = match self.view_mode {
                    ViewMode::Library => self.library.len(),
                    ViewMode::Queue => self.queue.len(),
                    ViewMode::Playlists => self.playlists.len(),
                    _ => 0,
                };
                let visible = self.get_visible_items();
                let max_scroll = max_items.saturating_sub(visible);

                if *delta_y < 0 && self.scroll_offset < max_scroll {
                    self.scroll_offset = (self.scroll_offset + 3).min(max_scroll);
                    return true;
                } else if *delta_y > 0 && self.scroll_offset > 0 {
                    self.scroll_offset = self.scroll_offset.saturating_sub(3);
                    return true;
                }

                false
            }

            WidgetEvent::KeyDown { key, .. } => {
                match *key {
                    0x20 => { // Space - play/pause
                        self.toggle_play_pause();
                        true
                    }
                    0x4E | 0x6E => { // N - next
                        self.next();
                        true
                    }
                    0x50 | 0x70 => { // P - previous
                        self.previous();
                        true
                    }
                    0x4D | 0x6D => { // M - mute
                        self.toggle_mute();
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

        self.update_spectrum_for_render(surface);
    }
}

impl MusicPlayer {
    fn update_spectrum_for_render(&self, surface: &mut Surface) {
        let _theme = theme();
        let x = self.bounds.x.max(0) as usize;
        let y = self.bounds.y.max(0) as usize;
        let w = self.bounds.width;
        let h = self.bounds.height;

        // Colors
        let bg_color = Color::new(25, 25, 30);
        let sidebar_bg = Color::new(35, 35, 45);
        let text_color = Color::new(220, 220, 220);
        let accent_color = Color::new(29, 185, 84); // Spotify green
        let dim_text = Color::new(150, 150, 150);
        let selected_bg = Color::new(50, 50, 60);
        let playing_color = Color::new(29, 185, 84);

        // Draw background
        surface.fill_rect(x, y, w, h, bg_color);

        // Draw sidebar
        let sidebar_width = 150;
        surface.fill_rect(x, y, sidebar_width, h, sidebar_bg);

        // Draw sidebar title
        draw_string(surface, x + 10, y + 15, "Music Player", text_color);

        // Draw view mode tabs
        let tabs = [
            (ViewMode::NowPlaying, "Now Playing"),
            (ViewMode::Library, "Library"),
            (ViewMode::Playlists, "Playlists"),
            (ViewMode::Queue, "Queue"),
            (ViewMode::Equalizer, "Equalizer"),
        ];

        let tab_height = 35;
        for (i, (mode, name)) in tabs.iter().enumerate() {
            let tab_y = y + 50 + i * tab_height;
            let is_selected = *mode == self.view_mode;

            if is_selected {
                surface.fill_rect(x, tab_y, sidebar_width, tab_height, selected_bg);
                surface.fill_rect(x, tab_y, 3, tab_height, accent_color);
            }

            let text_c = if is_selected { text_color } else { dim_text };
            draw_string(surface, x + 15, tab_y + 10, name, text_c);
        }

        // Draw main content area
        let content_x = x + sidebar_width + 10;
        let content_y = y + 10;
        let content_w = w - sidebar_width - 20;

        // Draw based on view mode
        match self.view_mode {
            ViewMode::NowPlaying => {
                self.render_now_playing(surface, content_x, content_y, content_w);
            }
            ViewMode::Library => {
                self.render_library(surface, content_x, content_y, content_w, text_color, dim_text, selected_bg, playing_color);
            }
            ViewMode::Queue => {
                self.render_queue(surface, content_x, content_y, content_w, text_color, dim_text, selected_bg, playing_color);
            }
            ViewMode::Playlists => {
                self.render_playlists(surface, content_x, content_y, content_w, text_color, dim_text, selected_bg);
            }
            ViewMode::Equalizer => {
                self.render_equalizer(surface, content_x, content_y, content_w, text_color, accent_color, dim_text);
            }
        }

        // Draw playback controls at bottom
        self.render_controls(surface, x, y + h - 90, w, text_color, accent_color, dim_text);
    }

    fn render_now_playing(&self, surface: &mut Surface, x: usize, y: usize, w: usize) {
        let text_color = Color::new(220, 220, 220);
        let dim_text = Color::new(150, 150, 150);
        let accent_color = Color::new(29, 185, 84);

        // Title
        draw_string(surface, x, y, "Now Playing", text_color);

        if let Some(track) = &self.current_track {
            // Album art placeholder
            let art_size = 200;
            let art_x = x + (w - art_size) / 2;
            let art_y = y + 40;
            surface.fill_rect(art_x, art_y, art_size, art_size, Color::new(60, 60, 70));

            // Musical note icon
            draw_string(surface, art_x + art_size / 2 - 8, art_y + art_size / 2 - 8, "♪", accent_color);

            // Track info
            let info_y = art_y + art_size + 20;
            let title = track.display_title();
            let title_x = x + (w - title.len() * 8) / 2;
            draw_string(surface, title_x, info_y, title, text_color);

            let artist = track.display_artist();
            let artist_x = x + (w - artist.len() * 8) / 2;
            draw_string(surface, artist_x, info_y + 25, artist, dim_text);

            let album = &track.metadata.album;
            if !album.is_empty() {
                let album_x = x + (w - album.len() * 8) / 2;
                draw_string(surface, album_x, info_y + 45, album, dim_text);
            }

            // Spectrum visualization
            let spectrum_y = info_y + 80;
            let bar_width = 8;
            let bar_gap = 4;
            let spectrum_width = 32 * (bar_width + bar_gap);
            let spectrum_x = x + (w - spectrum_width) / 2;

            for i in 0..32 {
                let bar_height = (self.spectrum[i] as usize * 50 / 100).max(2);
                let bar_x = spectrum_x + i * (bar_width + bar_gap);
                let bar_y = spectrum_y + 50 - bar_height;

                // Gradient color for bars
                let green = (100 + bar_height * 3).min(255) as u8;
                surface.fill_rect(bar_x, bar_y, bar_width, bar_height, Color::new(29, green, 84));
            }
        } else {
            let msg = "No track playing";
            let msg_x = x + (w - msg.len() * 8) / 2;
            draw_string(surface, msg_x, y + 150, msg, dim_text);
        }
    }

    fn render_library(&self, surface: &mut Surface, x: usize, y: usize, w: usize,
                      text_color: Color, dim_text: Color, selected_bg: Color, playing_color: Color) {
        // Header
        draw_string(surface, x, y, "Library", text_color);
        let track_count = format!("{} tracks", self.library.len());
        draw_string(surface, x + w - track_count.len() * 8 - 10, y, &track_count, dim_text);

        // Column headers
        let header_y = y + 30;
        draw_string(surface, x + 30, header_y, "Title", dim_text);
        draw_string(surface, x + 250, header_y, "Artist", dim_text);
        draw_string(surface, x + 400, header_y, "Album", dim_text);
        draw_string(surface, x + 550, header_y, "Duration", dim_text);

        // Tracks
        let list_y = y + 55;
        let item_height = 28;
        let visible = self.get_visible_items();
        let current_track_id = self.current_track.as_ref().map(|t| t.id);

        for (i, track) in self.library.iter().skip(self.scroll_offset).take(visible).enumerate() {
            let item_y = list_y + i * item_height;
            let actual_idx = i + self.scroll_offset;

            // Background
            let is_selected = Some(actual_idx) == self.selected_index;
            let is_playing = Some(track.id) == current_track_id;

            if is_selected {
                surface.fill_rect(x, item_y, w, item_height - 2, selected_bg);
            }

            // Playing indicator
            let title_color = if is_playing { playing_color } else { text_color };
            if is_playing {
                draw_string(surface, x + 5, item_y + 6, "▶", playing_color);
            }

            // Track info
            let title = if track.metadata.title.len() > 25 {
                format!("{}...", &track.metadata.title[..22])
            } else {
                track.metadata.title.clone()
            };
            draw_string(surface, x + 30, item_y + 6, &title, title_color);

            let artist = if track.metadata.artist.len() > 18 {
                format!("{}...", &track.metadata.artist[..15])
            } else {
                track.metadata.artist.clone()
            };
            draw_string(surface, x + 250, item_y + 6, &artist, dim_text);

            let album = if track.metadata.album.len() > 18 {
                format!("{}...", &track.metadata.album[..15])
            } else {
                track.metadata.album.clone()
            };
            draw_string(surface, x + 400, item_y + 6, &album, dim_text);

            draw_string(surface, x + 550, item_y + 6, &track.metadata.format_duration(), dim_text);
        }
    }

    fn render_queue(&self, surface: &mut Surface, x: usize, y: usize, w: usize,
                    text_color: Color, dim_text: Color, selected_bg: Color, playing_color: Color) {
        draw_string(surface, x, y, "Queue", text_color);
        let queue_info = format!("{} tracks", self.queue.len());
        draw_string(surface, x + w - queue_info.len() * 8 - 10, y, &queue_info, dim_text);

        let list_y = y + 40;
        let item_height = 28;
        let visible = self.get_visible_items();

        for (i, track_id) in self.queue.iter().skip(self.scroll_offset).take(visible).enumerate() {
            let item_y = list_y + i * item_height;
            let actual_idx = i + self.scroll_offset;
            let is_current = actual_idx == self.queue_index;

            if is_current {
                surface.fill_rect(x, item_y, w, item_height - 2, selected_bg);
            }

            if let Some(track) = self.get_track(*track_id) {
                let color = if is_current { playing_color } else { text_color };
                let num = format!("{}.", actual_idx + 1);
                draw_string(surface, x + 5, item_y + 6, &num, dim_text);
                draw_string(surface, x + 35, item_y + 6, track.display_title(), color);
                draw_string(surface, x + 300, item_y + 6, track.display_artist(), dim_text);
            }
        }
    }

    fn render_playlists(&self, surface: &mut Surface, x: usize, y: usize, w: usize,
                        text_color: Color, dim_text: Color, selected_bg: Color) {
        draw_string(surface, x, y, "Playlists", text_color);

        let list_y = y + 40;
        let item_height = 35;

        for (i, playlist) in self.playlists.iter().enumerate() {
            let item_y = list_y + i * item_height;

            if Some(i) == self.selected_index {
                surface.fill_rect(x, item_y, w, item_height - 2, selected_bg);
            }

            // Playlist icon
            surface.fill_rect(x + 5, item_y + 5, 25, 25, Color::new(80, 80, 100));

            draw_string(surface, x + 40, item_y + 5, &playlist.name, text_color);
            let track_info = format!("{} tracks", playlist.tracks.len());
            draw_string(surface, x + 40, item_y + 20, &track_info, dim_text);
        }
    }

    fn render_equalizer(&self, surface: &mut Surface, x: usize, y: usize, w: usize,
                        text_color: Color, accent_color: Color, dim_text: Color) {
        draw_string(surface, x, y, "Equalizer", text_color);

        // Preset selector
        draw_string(surface, x, y + 30, "Preset:", dim_text);
        draw_string(surface, x + 70, y + 30, &self.equalizer.name, accent_color);

        // EQ bands
        let band_labels = ["32", "64", "125", "250", "500", "1K", "2K", "4K", "8K", "16K"];
        let band_width = 40;
        let band_height = 150;
        let bands_x = x + (w - 10 * band_width) / 2;
        let bands_y = y + 80;

        for (i, (value, label)) in self.equalizer.bands.iter().zip(band_labels.iter()).enumerate() {
            let band_x = bands_x + i * band_width;

            // Draw track
            surface.fill_rect(band_x + 15, bands_y, 10, band_height, Color::new(50, 50, 60));

            // Draw slider position (centered at middle)
            let center_y = bands_y + band_height / 2;
            let offset = (*value as isize * band_height as isize / 24) as isize;
            let slider_y = (center_y as isize - offset).max(bands_y as isize) as usize;

            surface.fill_rect(band_x + 12, slider_y, 16, 8, accent_color);

            // Draw label
            draw_string(surface, band_x + 12, bands_y + band_height + 10, label, dim_text);

            // Draw value
            let val_str = format!("{:+}", value);
            draw_string(surface, band_x + 12, bands_y - 15, &val_str, dim_text);
        }

        // Preamp
        draw_string(surface, x, bands_y + band_height + 40, "Preamp:", dim_text);
        let preamp_str = format!("{:+} dB", self.equalizer.preamp);
        draw_string(surface, x + 70, bands_y + band_height + 40, &preamp_str, text_color);
    }

    fn render_controls(&self, surface: &mut Surface, x: usize, y: usize, w: usize,
                       text_color: Color, accent_color: Color, dim_text: Color) {
        // Progress bar background
        let progress_y = y + 5;
        let progress_x = x + 160;
        let progress_w = w - 320;

        surface.fill_rect(progress_x, progress_y, progress_w, 6, Color::new(50, 50, 60));

        // Progress bar fill
        if let Some(track) = &self.current_track {
            if track.metadata.duration > 0 {
                let fill_w = (self.position as usize * progress_w / track.metadata.duration as usize).min(progress_w);
                surface.fill_rect(progress_x, progress_y, fill_w, 6, accent_color);

                // Time labels
                draw_string(surface, progress_x - 50, progress_y - 2, &self.format_time(self.position), dim_text);
                let duration_str = self.format_time(track.metadata.duration);
                draw_string(surface, progress_x + progress_w + 10, progress_y - 2, &duration_str, dim_text);
            }
        }

        // Control buttons
        let center_x = x + w / 2;
        let controls_y = y + 30;

        // Previous
        draw_string(surface, center_x - 90, controls_y, "⏮", text_color);

        // Play/Pause
        let play_icon = if self.state == PlayerState::Playing { "⏸" } else { "▶" };
        draw_string(surface, center_x - 10, controls_y, play_icon, accent_color);

        // Next
        draw_string(surface, center_x + 70, controls_y, "⏭", text_color);

        // Repeat
        let repeat_color = if self.repeat_mode != RepeatMode::Off { accent_color } else { dim_text };
        let repeat_str = format!("R{}", if self.repeat_mode == RepeatMode::One { "1" } else { "" });
        draw_string(surface, center_x + 130, controls_y, &repeat_str, repeat_color);

        // Shuffle
        let shuffle_color = if self.shuffle == ShuffleMode::On { accent_color } else { dim_text };
        draw_string(surface, center_x + 170, controls_y, "S", shuffle_color);

        // Volume
        let volume_x = x + w - 150;
        let vol_icon = if self.muted || self.volume == 0 { "🔇" } else { "🔊" };
        draw_string(surface, volume_x, controls_y, vol_icon, text_color);

        // Volume bar
        let vol_bar_x = volume_x + 30;
        let vol_bar_w = 80;
        surface.fill_rect(vol_bar_x, controls_y + 4, vol_bar_w, 8, Color::new(50, 50, 60));

        let vol_fill = if self.muted { 0 } else { self.volume as usize * vol_bar_w / 100 };
        surface.fill_rect(vol_bar_x, controls_y + 4, vol_fill, 8, accent_color);

        // Track info at bottom
        if let Some(track) = &self.current_track {
            let info_y = y + 60;
            draw_string(surface, x + 160, info_y, track.display_title(), text_color);
            draw_string(surface, x + 160, info_y + 15, track.display_artist(), dim_text);
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
