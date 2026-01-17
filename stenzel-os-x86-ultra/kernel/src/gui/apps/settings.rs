//! Settings Application
//!
//! System configuration application for Stenzel OS.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use crate::gui::widgets::{Widget, WidgetId, WidgetEvent, Bounds, MouseButton, theme};

/// Settings category
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsCategory {
    Display,
    Network,
    DateTime,
    Keyboard,
    Sound,
    Users,
    About,
}

impl SettingsCategory {
    pub fn label(&self) -> &'static str {
        match self {
            SettingsCategory::Display => "Display",
            SettingsCategory::Network => "Network",
            SettingsCategory::DateTime => "Date & Time",
            SettingsCategory::Keyboard => "Keyboard",
            SettingsCategory::Sound => "Sound",
            SettingsCategory::Users => "Users",
            SettingsCategory::About => "About",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            SettingsCategory::Display => "[#]",
            SettingsCategory::Network => "[~]",
            SettingsCategory::DateTime => "[@]",
            SettingsCategory::Keyboard => "[K]",
            SettingsCategory::Sound => "[S]",
            SettingsCategory::Users => "[U]",
            SettingsCategory::About => "[?]",
        }
    }

    pub fn all() -> &'static [SettingsCategory] {
        &[
            SettingsCategory::Display,
            SettingsCategory::Network,
            SettingsCategory::DateTime,
            SettingsCategory::Keyboard,
            SettingsCategory::Sound,
            SettingsCategory::Users,
            SettingsCategory::About,
        ]
    }
}

/// Display settings
#[derive(Debug, Clone)]
pub struct DisplaySettings {
    pub resolution_index: usize,
    pub available_resolutions: Vec<(usize, usize)>,
    pub theme: ThemeSetting,
    pub wallpaper: WallpaperSetting,
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            resolution_index: 0,
            available_resolutions: vec![
                (1920, 1080),
                (1680, 1050),
                (1440, 900),
                (1366, 768),
                (1280, 1024),
                (1280, 720),
                (1024, 768),
            ],
            theme: ThemeSetting::Dark,
            wallpaper: WallpaperSetting::SolidColor(Color::new(32, 32, 64)),
        }
    }
}

/// Theme setting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeSetting {
    Light,
    Dark,
}

/// Wallpaper setting
#[derive(Debug, Clone)]
pub enum WallpaperSetting {
    SolidColor(Color),
    Gradient { start: Color, end: Color },
    Image(String),
}

/// Network settings
#[derive(Debug, Clone)]
pub struct NetworkSettings {
    pub hostname: String,
    pub interfaces: Vec<NetworkInterface>,
    pub dns_servers: Vec<String>,
}

impl Default for NetworkSettings {
    fn default() -> Self {
        Self {
            hostname: String::from("stenzel"),
            interfaces: Vec::new(),
            dns_servers: vec![String::from("8.8.8.8"), String::from("1.1.1.1")],
        }
    }
}

/// Network interface
#[derive(Debug, Clone)]
pub struct NetworkInterface {
    pub name: String,
    pub mac: String,
    pub ip: Option<String>,
    pub netmask: Option<String>,
    pub gateway: Option<String>,
    pub dhcp: bool,
    pub connected: bool,
}

/// Date/Time settings
#[derive(Debug, Clone)]
pub struct DateTimeSettings {
    pub timezone: String,
    pub use_24h: bool,
    pub auto_sync: bool,
    pub ntp_server: String,
}

impl Default for DateTimeSettings {
    fn default() -> Self {
        Self {
            timezone: String::from("UTC"),
            use_24h: true,
            auto_sync: true,
            ntp_server: String::from("pool.ntp.org"),
        }
    }
}

/// Keyboard settings
#[derive(Debug, Clone)]
pub struct KeyboardSettings {
    pub layout: String,
    pub available_layouts: Vec<String>,
    pub repeat_delay: u32, // ms
    pub repeat_rate: u32,  // chars/sec
}

impl Default for KeyboardSettings {
    fn default() -> Self {
        Self {
            layout: String::from("US"),
            available_layouts: vec![
                String::from("US"),
                String::from("ABNT2"),
                String::from("UK"),
                String::from("DE"),
                String::from("FR"),
            ],
            repeat_delay: 500,
            repeat_rate: 30,
        }
    }
}

/// Sound settings
#[derive(Debug, Clone)]
pub struct SoundSettings {
    pub master_volume: u8, // 0-100
    pub muted: bool,
    pub system_sounds: bool,
}

impl Default for SoundSettings {
    fn default() -> Self {
        Self {
            master_volume: 80,
            muted: false,
            system_sounds: true,
        }
    }
}

/// System information
#[derive(Debug, Clone)]
pub struct SystemInfo {
    pub os_name: String,
    pub os_version: String,
    pub kernel_version: String,
    pub cpu_name: String,
    pub cpu_cores: usize,
    pub total_memory: u64,
    pub hostname: String,
}

impl Default for SystemInfo {
    fn default() -> Self {
        Self {
            os_name: String::from("Stenzel OS"),
            os_version: String::from("0.1.0"),
            kernel_version: String::from("0.1.0"),
            cpu_name: String::from("Unknown CPU"),
            cpu_cores: 1,
            total_memory: 0,
            hostname: String::from("stenzel"),
        }
    }
}

/// Settings change callback
pub type SettingsCallback = fn(SettingsCategory);

/// Settings application widget
pub struct Settings {
    id: WidgetId,
    bounds: Bounds,

    // Current category
    current_category: SettingsCategory,

    // Settings data
    display: DisplaySettings,
    network: NetworkSettings,
    datetime: DateTimeSettings,
    keyboard: KeyboardSettings,
    sound: SoundSettings,
    system_info: SystemInfo,

    // UI state
    visible: bool,
    focused: bool,
    sidebar_hover: Option<usize>,
    scroll_y: usize,

    // Editing state
    editing_field: Option<String>,
    edit_value: String,

    // Callback
    on_change: Option<SettingsCallback>,
}

impl Settings {
    const CHAR_WIDTH: usize = 8;
    const CHAR_HEIGHT: usize = 16;
    const SIDEBAR_WIDTH: usize = 200;
    const HEADER_HEIGHT: usize = 50;
    const ROW_HEIGHT: usize = 36;
    const PADDING: usize = 16;

    /// Create new settings app
    pub fn new(x: isize, y: isize, width: usize, height: usize) -> Self {
        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(x, y, width, height),
            current_category: SettingsCategory::Display,
            display: DisplaySettings::default(),
            network: NetworkSettings::default(),
            datetime: DateTimeSettings::default(),
            keyboard: KeyboardSettings::default(),
            sound: SoundSettings::default(),
            system_info: SystemInfo::default(),
            visible: true,
            focused: false,
            sidebar_hover: None,
            scroll_y: 0,
            editing_field: None,
            edit_value: String::new(),
            on_change: None,
        }
    }

    /// Set change callback
    pub fn set_on_change(&mut self, callback: SettingsCallback) {
        self.on_change = Some(callback);
    }

    /// Get display settings
    pub fn display_settings(&self) -> &DisplaySettings {
        &self.display
    }

    /// Get network settings
    pub fn network_settings(&self) -> &NetworkSettings {
        &self.network
    }

    /// Get datetime settings
    pub fn datetime_settings(&self) -> &DateTimeSettings {
        &self.datetime
    }

    /// Get keyboard settings
    pub fn keyboard_settings(&self) -> &KeyboardSettings {
        &self.keyboard
    }

    /// Get sound settings
    pub fn sound_settings(&self) -> &SoundSettings {
        &self.sound
    }

    /// Set system info
    pub fn set_system_info(&mut self, info: SystemInfo) {
        self.system_info = info;
    }

    /// Set network interfaces
    pub fn set_network_interfaces(&mut self, interfaces: Vec<NetworkInterface>) {
        self.network.interfaces = interfaces;
    }

    /// Get category at point (sidebar)
    fn category_at_point(&self, x: isize, y: isize) -> Option<SettingsCategory> {
        let local_x = (x - self.bounds.x) as usize;
        let local_y = (y - self.bounds.y) as usize;

        if local_x >= Self::SIDEBAR_WIDTH {
            return None;
        }

        if local_y < Self::HEADER_HEIGHT {
            return None;
        }

        let row = (local_y - Self::HEADER_HEIGHT) / Self::ROW_HEIGHT;
        let categories = SettingsCategory::all();
        categories.get(row).copied()
    }

    /// Get content row at point
    fn content_row_at_point(&self, x: isize, y: isize) -> Option<usize> {
        let local_x = (x - self.bounds.x) as usize;
        let local_y = (y - self.bounds.y) as usize;

        if local_x < Self::SIDEBAR_WIDTH + Self::PADDING {
            return None;
        }

        if local_y < Self::HEADER_HEIGHT + Self::PADDING {
            return None;
        }

        let row = (local_y - Self::HEADER_HEIGHT - Self::PADDING) / Self::ROW_HEIGHT;
        Some(row + self.scroll_y)
    }

    /// Trigger change callback
    fn notify_change(&self) {
        if let Some(callback) = self.on_change {
            callback(self.current_category);
        }
    }

    /// Handle click in display settings
    fn handle_display_click(&mut self, row: usize, x: usize) {
        match row {
            0 => {
                // Resolution - cycle through options
                self.display.resolution_index = (self.display.resolution_index + 1)
                    % self.display.available_resolutions.len();
                self.notify_change();
            }
            1 => {
                // Theme toggle
                self.display.theme = match self.display.theme {
                    ThemeSetting::Light => ThemeSetting::Dark,
                    ThemeSetting::Dark => ThemeSetting::Light,
                };
                self.notify_change();
            }
            _ => {}
        }
    }

    /// Handle click in sound settings
    fn handle_sound_click(&mut self, row: usize, x: usize) {
        let content_x = Self::SIDEBAR_WIDTH + Self::PADDING;
        let slider_x = content_x + 120;
        let slider_w = 200;

        match row {
            0 => {
                // Volume slider
                if x >= slider_x && x < slider_x + slider_w {
                    let vol = ((x - slider_x) as f32 / slider_w as f32 * 100.0) as u8;
                    self.sound.master_volume = vol.min(100);
                    self.notify_change();
                }
            }
            1 => {
                // Mute toggle
                self.sound.muted = !self.sound.muted;
                self.notify_change();
            }
            2 => {
                // System sounds toggle
                self.sound.system_sounds = !self.sound.system_sounds;
                self.notify_change();
            }
            _ => {}
        }
    }

    /// Handle click in keyboard settings
    fn handle_keyboard_click(&mut self, row: usize) {
        match row {
            0 => {
                // Cycle keyboard layout
                let idx = self.keyboard.available_layouts.iter()
                    .position(|l| l == &self.keyboard.layout)
                    .unwrap_or(0);
                let next_idx = (idx + 1) % self.keyboard.available_layouts.len();
                self.keyboard.layout = self.keyboard.available_layouts[next_idx].clone();
                self.notify_change();
            }
            _ => {}
        }
    }

    /// Handle click in datetime settings
    fn handle_datetime_click(&mut self, row: usize) {
        match row {
            1 => {
                // Toggle 24h
                self.datetime.use_24h = !self.datetime.use_24h;
                self.notify_change();
            }
            2 => {
                // Toggle auto sync
                self.datetime.auto_sync = !self.datetime.auto_sync;
                self.notify_change();
            }
            _ => {}
        }
    }
}

impl Widget for Settings {
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
        true
    }

    fn set_enabled(&mut self, _enabled: bool) {}

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn handle_event(&mut self, event: &WidgetEvent) -> bool {
        match event {
            WidgetEvent::Focus => {
                self.focused = true;
                true
            }
            WidgetEvent::Blur => {
                self.focused = false;
                true
            }
            WidgetEvent::MouseDown { button: MouseButton::Left, x, y } => {
                // Check sidebar
                if let Some(cat) = self.category_at_point(*x, *y) {
                    self.current_category = cat;
                    self.scroll_y = 0;
                    return true;
                }

                // Check content area
                if let Some(row) = self.content_row_at_point(*x, *y) {
                    let local_x = (*x - self.bounds.x) as usize;
                    match self.current_category {
                        SettingsCategory::Display => self.handle_display_click(row, local_x),
                        SettingsCategory::Sound => self.handle_sound_click(row, local_x),
                        SettingsCategory::Keyboard => self.handle_keyboard_click(row),
                        SettingsCategory::DateTime => self.handle_datetime_click(row),
                        _ => {}
                    }
                    return true;
                }

                false
            }
            WidgetEvent::MouseMove { x, y } => {
                // Update sidebar hover
                self.sidebar_hover = self.category_at_point(*x, *y)
                    .and_then(|cat| {
                        SettingsCategory::all().iter().position(|c| *c == cat)
                    });
                true
            }
            WidgetEvent::Scroll { delta_y, .. } => {
                if *delta_y < 0 {
                    self.scroll_y = self.scroll_y.saturating_add(1);
                } else {
                    self.scroll_y = self.scroll_y.saturating_sub(1);
                }
                true
            }
            _ => false,
        }
    }

    fn render(&self, surface: &mut Surface) {
        if !self.visible {
            return;
        }

        let theme = theme();
        let x = self.bounds.x.max(0) as usize;
        let y = self.bounds.y.max(0) as usize;
        let w = self.bounds.width;
        let h = self.bounds.height;

        // Background
        let bg = Color::new(30, 30, 30);
        for py in 0..h {
            for px in 0..w {
                surface.set_pixel(x + px, y + py, bg);
            }
        }

        // Sidebar background
        let sidebar_bg = Color::new(37, 37, 38);
        for py in 0..h {
            for px in 0..Self::SIDEBAR_WIDTH {
                surface.set_pixel(x + px, y + py, sidebar_bg);
            }
        }

        // Sidebar header
        let header_bg = Color::new(45, 45, 48);
        for py in 0..Self::HEADER_HEIGHT {
            for px in 0..Self::SIDEBAR_WIDTH {
                surface.set_pixel(x + px, y + py, header_bg);
            }
        }

        // "Settings" title
        draw_string(surface, x + 16, y + 16, "Settings", theme.fg);

        // Sidebar categories
        for (i, cat) in SettingsCategory::all().iter().enumerate() {
            let row_y = y + Self::HEADER_HEIGHT + i * Self::ROW_HEIGHT;

            // Highlight current/hovered
            let is_current = *cat == self.current_category;
            let is_hovered = self.sidebar_hover == Some(i);

            if is_current {
                let highlight = Color::new(0, 122, 204);
                for py in 0..Self::ROW_HEIGHT {
                    for px in 0..Self::SIDEBAR_WIDTH {
                        surface.set_pixel(x + px, row_y + py, highlight);
                    }
                }
            } else if is_hovered {
                let hover = Color::new(50, 50, 53);
                for py in 0..Self::ROW_HEIGHT {
                    for px in 0..Self::SIDEBAR_WIDTH {
                        surface.set_pixel(x + px, row_y + py, hover);
                    }
                }
            }

            // Icon
            draw_string(surface, x + 16, row_y + 10, cat.icon(), theme.fg);

            // Label
            draw_string(surface, x + 56, row_y + 10, cat.label(), theme.fg);
        }

        // Content header
        let content_x = x + Self::SIDEBAR_WIDTH;
        let content_w = w - Self::SIDEBAR_WIDTH;

        for py in 0..Self::HEADER_HEIGHT {
            for px in 0..content_w {
                surface.set_pixel(content_x + px, y + py, header_bg);
            }
        }

        // Category title
        let title = self.current_category.label();
        draw_string(surface, content_x + Self::PADDING, y + 16, title, theme.fg);

        // Content area
        let content_y = y + Self::HEADER_HEIGHT + Self::PADDING;

        match self.current_category {
            SettingsCategory::Display => self.render_display(surface, content_x + Self::PADDING, content_y),
            SettingsCategory::Network => self.render_network(surface, content_x + Self::PADDING, content_y),
            SettingsCategory::DateTime => self.render_datetime(surface, content_x + Self::PADDING, content_y),
            SettingsCategory::Keyboard => self.render_keyboard(surface, content_x + Self::PADDING, content_y),
            SettingsCategory::Sound => self.render_sound(surface, content_x + Self::PADDING, content_y),
            SettingsCategory::Users => self.render_users(surface, content_x + Self::PADDING, content_y),
            SettingsCategory::About => self.render_about(surface, content_x + Self::PADDING, content_y),
        }
    }
}

impl Settings {
    fn render_display(&self, surface: &mut Surface, x: usize, y: usize) {
        let theme = theme();
        let label_color = Color::new(180, 180, 180);
        let value_color = theme.fg;

        // Resolution
        draw_string(surface, x, y, "Resolution:", label_color);
        let res = &self.display.available_resolutions[self.display.resolution_index];
        let res_str = format_resolution(res.0, res.1);
        draw_string(surface, x + 120, y, &res_str, value_color);
        draw_string(surface, x + 250, y, "[Click to change]", Color::new(100, 100, 100));

        // Theme
        draw_string(surface, x, y + Self::ROW_HEIGHT, "Theme:", label_color);
        let theme_str = match self.display.theme {
            ThemeSetting::Light => "Light",
            ThemeSetting::Dark => "Dark",
        };
        draw_string(surface, x + 120, y + Self::ROW_HEIGHT, theme_str, value_color);
        draw_string(surface, x + 250, y + Self::ROW_HEIGHT, "[Click to toggle]", Color::new(100, 100, 100));

        // Wallpaper
        draw_string(surface, x, y + Self::ROW_HEIGHT * 2, "Wallpaper:", label_color);
        let wp_str = match &self.display.wallpaper {
            WallpaperSetting::SolidColor(_) => "Solid Color",
            WallpaperSetting::Gradient { .. } => "Gradient",
            WallpaperSetting::Image(path) => path.as_str(),
        };
        draw_string(surface, x + 120, y + Self::ROW_HEIGHT * 2, wp_str, value_color);
    }

    fn render_network(&self, surface: &mut Surface, x: usize, y: usize) {
        let theme = theme();
        let label_color = Color::new(180, 180, 180);
        let value_color = theme.fg;

        // Hostname
        draw_string(surface, x, y, "Hostname:", label_color);
        draw_string(surface, x + 120, y, &self.network.hostname, value_color);

        // DNS Servers
        draw_string(surface, x, y + Self::ROW_HEIGHT, "DNS Servers:", label_color);
        let dns_str = self.network.dns_servers.join(", ");
        draw_string(surface, x + 120, y + Self::ROW_HEIGHT, &dns_str, value_color);

        // Interfaces
        draw_string(surface, x, y + Self::ROW_HEIGHT * 2, "Network Interfaces:", label_color);

        for (i, iface) in self.network.interfaces.iter().enumerate() {
            let iface_y = y + Self::ROW_HEIGHT * (3 + i);
            let status = if iface.connected { "[*]" } else { "[ ]" };
            draw_string(surface, x + 16, iface_y, status, value_color);
            draw_string(surface, x + 48, iface_y, &iface.name, value_color);

            if let Some(ip) = &iface.ip {
                draw_string(surface, x + 150, iface_y, ip, label_color);
            }
        }

        if self.network.interfaces.is_empty() {
            draw_string(surface, x + 16, y + Self::ROW_HEIGHT * 3, "No interfaces found", Color::new(100, 100, 100));
        }
    }

    fn render_datetime(&self, surface: &mut Surface, x: usize, y: usize) {
        let theme = theme();
        let label_color = Color::new(180, 180, 180);
        let value_color = theme.fg;

        // Timezone
        draw_string(surface, x, y, "Timezone:", label_color);
        draw_string(surface, x + 120, y, &self.datetime.timezone, value_color);

        // 24h format
        draw_string(surface, x, y + Self::ROW_HEIGHT, "24-hour format:", label_color);
        let fmt_str = if self.datetime.use_24h { "Yes" } else { "No" };
        draw_string(surface, x + 140, y + Self::ROW_HEIGHT, fmt_str, value_color);
        draw_string(surface, x + 200, y + Self::ROW_HEIGHT, "[Click to toggle]", Color::new(100, 100, 100));

        // Auto sync
        draw_string(surface, x, y + Self::ROW_HEIGHT * 2, "Auto-sync time:", label_color);
        let sync_str = if self.datetime.auto_sync { "Yes" } else { "No" };
        draw_string(surface, x + 140, y + Self::ROW_HEIGHT * 2, sync_str, value_color);
        draw_string(surface, x + 200, y + Self::ROW_HEIGHT * 2, "[Click to toggle]", Color::new(100, 100, 100));

        // NTP Server
        draw_string(surface, x, y + Self::ROW_HEIGHT * 3, "NTP Server:", label_color);
        draw_string(surface, x + 120, y + Self::ROW_HEIGHT * 3, &self.datetime.ntp_server, value_color);
    }

    fn render_keyboard(&self, surface: &mut Surface, x: usize, y: usize) {
        let theme = theme();
        let label_color = Color::new(180, 180, 180);
        let value_color = theme.fg;

        // Layout
        draw_string(surface, x, y, "Layout:", label_color);
        draw_string(surface, x + 120, y, &self.keyboard.layout, value_color);
        draw_string(surface, x + 200, y, "[Click to change]", Color::new(100, 100, 100));

        // Repeat delay
        draw_string(surface, x, y + Self::ROW_HEIGHT, "Repeat Delay:", label_color);
        let delay_str = format_num(self.keyboard.repeat_delay as u64, " ms");
        draw_string(surface, x + 140, y + Self::ROW_HEIGHT, &delay_str, value_color);

        // Repeat rate
        draw_string(surface, x, y + Self::ROW_HEIGHT * 2, "Repeat Rate:", label_color);
        let rate_str = format_num(self.keyboard.repeat_rate as u64, " chars/sec");
        draw_string(surface, x + 140, y + Self::ROW_HEIGHT * 2, &rate_str, value_color);

        // Available layouts
        draw_string(surface, x, y + Self::ROW_HEIGHT * 4, "Available Layouts:", label_color);
        let layouts_str = self.keyboard.available_layouts.join(", ");
        draw_string(surface, x + 16, y + Self::ROW_HEIGHT * 5, &layouts_str, Color::new(100, 100, 100));
    }

    fn render_sound(&self, surface: &mut Surface, x: usize, y: usize) {
        let theme = theme();
        let label_color = Color::new(180, 180, 180);
        let value_color = theme.fg;

        // Volume slider
        draw_string(surface, x, y, "Master Volume:", label_color);

        // Draw slider track
        let slider_x = x + 120;
        let slider_y = y + 4;
        let slider_w = 200;
        let slider_h = 8;
        let track_color = Color::new(60, 60, 63);
        let fill_color = Color::new(0, 122, 204);

        for py in 0..slider_h {
            for px in 0..slider_w {
                surface.set_pixel(slider_x + px, slider_y + py, track_color);
            }
        }

        // Filled portion
        let fill_w = (self.sound.master_volume as usize * slider_w) / 100;
        for py in 0..slider_h {
            for px in 0..fill_w {
                surface.set_pixel(slider_x + px, slider_y + py, fill_color);
            }
        }

        // Volume percentage
        let vol_str = format_num(self.sound.master_volume as u64, "%");
        draw_string(surface, slider_x + slider_w + 16, y, &vol_str, value_color);

        // Mute toggle
        draw_string(surface, x, y + Self::ROW_HEIGHT, "Muted:", label_color);
        let mute_str = if self.sound.muted { "Yes" } else { "No" };
        draw_string(surface, x + 120, y + Self::ROW_HEIGHT, mute_str, value_color);
        draw_string(surface, x + 180, y + Self::ROW_HEIGHT, "[Click to toggle]", Color::new(100, 100, 100));

        // System sounds
        draw_string(surface, x, y + Self::ROW_HEIGHT * 2, "System Sounds:", label_color);
        let sys_str = if self.sound.system_sounds { "Enabled" } else { "Disabled" };
        draw_string(surface, x + 140, y + Self::ROW_HEIGHT * 2, sys_str, value_color);
        draw_string(surface, x + 230, y + Self::ROW_HEIGHT * 2, "[Click to toggle]", Color::new(100, 100, 100));
    }

    fn render_users(&self, surface: &mut Surface, x: usize, y: usize) {
        let theme = theme();
        let label_color = Color::new(180, 180, 180);

        draw_string(surface, x, y, "User Management", theme.fg);
        draw_string(surface, x, y + Self::ROW_HEIGHT, "Current User: root", label_color);
        draw_string(surface, x, y + Self::ROW_HEIGHT * 3, "User management coming soon...", Color::new(100, 100, 100));
    }

    fn render_about(&self, surface: &mut Surface, x: usize, y: usize) {
        let theme = theme();
        let label_color = Color::new(180, 180, 180);
        let value_color = theme.fg;

        // OS Name
        draw_string(surface, x, y, "Operating System:", label_color);
        draw_string(surface, x + 160, y, &self.system_info.os_name, value_color);

        // OS Version
        draw_string(surface, x, y + Self::ROW_HEIGHT, "OS Version:", label_color);
        draw_string(surface, x + 160, y + Self::ROW_HEIGHT, &self.system_info.os_version, value_color);

        // Kernel Version
        draw_string(surface, x, y + Self::ROW_HEIGHT * 2, "Kernel Version:", label_color);
        draw_string(surface, x + 160, y + Self::ROW_HEIGHT * 2, &self.system_info.kernel_version, value_color);

        // CPU
        draw_string(surface, x, y + Self::ROW_HEIGHT * 3, "Processor:", label_color);
        draw_string(surface, x + 160, y + Self::ROW_HEIGHT * 3, &self.system_info.cpu_name, value_color);

        // CPU Cores
        draw_string(surface, x, y + Self::ROW_HEIGHT * 4, "CPU Cores:", label_color);
        let cores_str = format_num(self.system_info.cpu_cores as u64, "");
        draw_string(surface, x + 160, y + Self::ROW_HEIGHT * 4, &cores_str, value_color);

        // Memory
        draw_string(surface, x, y + Self::ROW_HEIGHT * 5, "Total Memory:", label_color);
        let mem_str = format_memory(self.system_info.total_memory);
        draw_string(surface, x + 160, y + Self::ROW_HEIGHT * 5, &mem_str, value_color);

        // Hostname
        draw_string(surface, x, y + Self::ROW_HEIGHT * 6, "Hostname:", label_color);
        draw_string(surface, x + 160, y + Self::ROW_HEIGHT * 6, &self.system_info.hostname, value_color);
    }
}

fn draw_string(surface: &mut Surface, x: usize, y: usize, s: &str, color: Color) {
    for (i, c) in s.chars().enumerate() {
        draw_char_simple(surface, x + i * 8, y, c, color);
    }
}

fn draw_char_simple(surface: &mut Surface, x: usize, y: usize, c: char, color: Color) {
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

fn format_num(n: u64, suffix: &str) -> String {
    use alloc::string::ToString;
    let mut s = n.to_string();
    s.push_str(suffix);
    s
}

fn format_resolution(w: usize, h: usize) -> String {
    use alloc::string::ToString;
    let mut s = w.to_string();
    s.push('x');
    s.push_str(&h.to_string());
    s
}

fn format_memory(bytes: u64) -> String {
    use alloc::string::ToString;
    if bytes < 1024 * 1024 {
        let kb = bytes / 1024;
        let mut s = kb.to_string();
        s.push_str(" KB");
        s
    } else if bytes < 1024 * 1024 * 1024 {
        let mb = bytes / (1024 * 1024);
        let mut s = mb.to_string();
        s.push_str(" MB");
        s
    } else {
        let gb = bytes / (1024 * 1024 * 1024);
        let mut s = gb.to_string();
        s.push_str(" GB");
        s
    }
}
