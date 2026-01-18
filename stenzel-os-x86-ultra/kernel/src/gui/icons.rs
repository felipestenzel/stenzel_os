//! Icon Theme System
//!
//! Provides a freedesktop.org-compatible icon theme system with SVG and PNG support.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use spin::Mutex;

// ============================================================================
// Math Helpers (no_std compatible)
// ============================================================================

/// Approximate sqrt for f32 using Newton's method
fn sqrt_f32(x: f32) -> f32 {
    if x <= 0.0 { return 0.0; }
    let mut guess = x / 2.0;
    if guess == 0.0 { guess = 1.0; }
    for _ in 0..10 {
        let new_guess = (guess + x / guess) / 2.0;
        if (new_guess - guess).abs() < 1e-7 { break; }
        guess = new_guess;
    }
    guess
}

/// Global icon theme state
static ICON_STATE: Mutex<Option<IconThemeState>> = Mutex::new(None);

/// Icon theme state
pub struct IconThemeState {
    /// Current theme
    pub current_theme: IconTheme,
    /// Available themes
    pub themes: BTreeMap<String, IconTheme>,
    /// Icon cache
    pub cache: BTreeMap<IconCacheKey, CachedIcon>,
    /// Fallback theme
    pub fallback_theme: String,
    /// Default sizes
    pub default_sizes: Vec<u32>,
}

/// Icon theme definition
#[derive(Debug, Clone)]
pub struct IconTheme {
    /// Theme name
    pub name: String,
    /// Theme display name
    pub display_name: String,
    /// Theme comment/description
    pub comment: String,
    /// Inherits from (fallback themes)
    pub inherits: Vec<String>,
    /// Theme directories
    pub directories: Vec<IconDirectory>,
    /// Is hidden from theme picker
    pub hidden: bool,
    /// Example icon name
    pub example: Option<String>,
}

/// Icon directory definition
#[derive(Debug, Clone)]
pub struct IconDirectory {
    /// Directory path relative to theme root
    pub path: String,
    /// Size of icons in this directory
    pub size: u32,
    /// Scale factor (for HiDPI)
    pub scale: u32,
    /// Context (Actions, Applications, etc.)
    pub context: IconContext,
    /// Type of directory
    pub dir_type: IconDirectoryType,
    /// Minimum size (for scalable)
    pub min_size: Option<u32>,
    /// Maximum size (for scalable)
    pub max_size: Option<u32>,
    /// Threshold (for threshold type)
    pub threshold: Option<u32>,
}

/// Icon context
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconContext {
    Actions,
    Animations,
    Applications,
    Categories,
    Devices,
    Emblems,
    Emotes,
    FilesTypes,
    International,
    MimeTypes,
    Places,
    Status,
    Stock,
}

impl IconContext {
    /// Get context name
    pub fn name(&self) -> &'static str {
        match self {
            IconContext::Actions => "Actions",
            IconContext::Animations => "Animations",
            IconContext::Applications => "Applications",
            IconContext::Categories => "Categories",
            IconContext::Devices => "Devices",
            IconContext::Emblems => "Emblems",
            IconContext::Emotes => "Emotes",
            IconContext::FilesTypes => "FilesTypes",
            IconContext::International => "International",
            IconContext::MimeTypes => "MimeTypes",
            IconContext::Places => "Places",
            IconContext::Status => "Status",
            IconContext::Stock => "Stock",
        }
    }
}

/// Icon directory type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconDirectoryType {
    /// Fixed size icons
    Fixed,
    /// Scalable icons
    Scalable,
    /// Threshold (match within threshold)
    Threshold,
}

/// Icon cache key
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct IconCacheKey {
    /// Theme name
    pub theme: String,
    /// Icon name
    pub name: String,
    /// Requested size
    pub size: u32,
    /// Scale factor
    pub scale: u32,
}

/// Cached icon
#[derive(Debug, Clone)]
pub struct CachedIcon {
    /// Icon data
    pub data: IconData,
    /// Actual size
    pub actual_size: u32,
    /// Source path
    pub source_path: String,
}

/// Icon data
#[derive(Debug, Clone)]
pub enum IconData {
    /// Rasterized RGBA pixel data
    Raster {
        width: u32,
        height: u32,
        data: Vec<u8>,
    },
    /// SVG source
    Svg(String),
    /// Reference to built-in icon
    Builtin(BuiltinIcon),
}

/// Built-in icons (hardcoded for essential system icons)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinIcon {
    // Window controls
    WindowClose,
    WindowMinimize,
    WindowMaximize,
    WindowRestore,

    // System
    SystemShutdown,
    SystemRestart,
    SystemLogout,
    SystemLock,
    SystemSettings,
    SystemSearch,

    // Files
    FolderOpen,
    FolderClosed,
    FileGeneric,
    FileText,
    FileImage,
    FileAudio,
    FileVideo,
    FileArchive,
    FileCode,

    // Actions
    ActionAdd,
    ActionRemove,
    ActionEdit,
    ActionDelete,
    ActionRefresh,
    ActionUndo,
    ActionRedo,
    ActionCopy,
    ActionPaste,
    ActionCut,
    ActionSave,
    ActionOpen,

    // Navigation
    NavBack,
    NavForward,
    NavUp,
    NavDown,
    NavHome,

    // Status
    StatusInfo,
    StatusWarning,
    StatusError,
    StatusSuccess,
    StatusQuestion,

    // Devices
    DeviceComputer,
    DeviceDrive,
    DeviceUSB,
    DeviceNetwork,
    DevicePrinter,
    DeviceCamera,
    DevicePhone,

    // Apps
    AppTerminal,
    AppBrowser,
    AppFiles,
    AppSettings,
    AppCalculator,
    AppCalendar,
    AppContacts,
    AppMail,
    AppMusic,
    AppPhotos,
    AppVideos,
    AppStore,

    // Network
    NetworkWifi0,
    NetworkWifi1,
    NetworkWifi2,
    NetworkWifi3,
    NetworkWired,
    NetworkDisconnected,
    NetworkVpn,

    // Battery
    BatteryEmpty,
    BatteryLow,
    BatteryMedium,
    BatteryHigh,
    BatteryFull,
    BatteryCharging,

    // Volume
    VolumeHigh,
    VolumeMedium,
    VolumeLow,
    VolumeMuted,

    // Misc
    Notification,
    Bluetooth,
    BluetoothDisabled,
    Brightness,
    Night,
    Airplane,
    Location,
    User,
    Users,
    Clock,
    Calendar,
    Weather,
}

impl BuiltinIcon {
    /// Get icon name
    pub fn name(&self) -> &'static str {
        match self {
            BuiltinIcon::WindowClose => "window-close",
            BuiltinIcon::WindowMinimize => "window-minimize",
            BuiltinIcon::WindowMaximize => "window-maximize",
            BuiltinIcon::WindowRestore => "window-restore",
            BuiltinIcon::SystemShutdown => "system-shutdown",
            BuiltinIcon::SystemRestart => "system-restart",
            BuiltinIcon::SystemLogout => "system-log-out",
            BuiltinIcon::SystemLock => "system-lock-screen",
            BuiltinIcon::SystemSettings => "preferences-system",
            BuiltinIcon::SystemSearch => "system-search",
            BuiltinIcon::FolderOpen => "folder-open",
            BuiltinIcon::FolderClosed => "folder",
            BuiltinIcon::FileGeneric => "text-x-generic",
            BuiltinIcon::FileText => "text-x-generic",
            BuiltinIcon::FileImage => "image-x-generic",
            BuiltinIcon::FileAudio => "audio-x-generic",
            BuiltinIcon::FileVideo => "video-x-generic",
            BuiltinIcon::FileArchive => "package-x-generic",
            BuiltinIcon::FileCode => "text-x-script",
            BuiltinIcon::ActionAdd => "list-add",
            BuiltinIcon::ActionRemove => "list-remove",
            BuiltinIcon::ActionEdit => "document-edit",
            BuiltinIcon::ActionDelete => "edit-delete",
            BuiltinIcon::ActionRefresh => "view-refresh",
            BuiltinIcon::ActionUndo => "edit-undo",
            BuiltinIcon::ActionRedo => "edit-redo",
            BuiltinIcon::ActionCopy => "edit-copy",
            BuiltinIcon::ActionPaste => "edit-paste",
            BuiltinIcon::ActionCut => "edit-cut",
            BuiltinIcon::ActionSave => "document-save",
            BuiltinIcon::ActionOpen => "document-open",
            BuiltinIcon::NavBack => "go-previous",
            BuiltinIcon::NavForward => "go-next",
            BuiltinIcon::NavUp => "go-up",
            BuiltinIcon::NavDown => "go-down",
            BuiltinIcon::NavHome => "go-home",
            BuiltinIcon::StatusInfo => "dialog-information",
            BuiltinIcon::StatusWarning => "dialog-warning",
            BuiltinIcon::StatusError => "dialog-error",
            BuiltinIcon::StatusSuccess => "emblem-ok-symbolic",
            BuiltinIcon::StatusQuestion => "dialog-question",
            BuiltinIcon::DeviceComputer => "computer",
            BuiltinIcon::DeviceDrive => "drive-harddisk",
            BuiltinIcon::DeviceUSB => "drive-removable-media-usb",
            BuiltinIcon::DeviceNetwork => "network-server",
            BuiltinIcon::DevicePrinter => "printer",
            BuiltinIcon::DeviceCamera => "camera-photo",
            BuiltinIcon::DevicePhone => "phone",
            BuiltinIcon::AppTerminal => "utilities-terminal",
            BuiltinIcon::AppBrowser => "web-browser",
            BuiltinIcon::AppFiles => "system-file-manager",
            BuiltinIcon::AppSettings => "preferences-system",
            BuiltinIcon::AppCalculator => "accessories-calculator",
            BuiltinIcon::AppCalendar => "x-office-calendar",
            BuiltinIcon::AppContacts => "x-office-address-book",
            BuiltinIcon::AppMail => "mail-read",
            BuiltinIcon::AppMusic => "applications-multimedia",
            BuiltinIcon::AppPhotos => "applications-graphics",
            BuiltinIcon::AppVideos => "applications-multimedia",
            BuiltinIcon::AppStore => "system-software-install",
            BuiltinIcon::NetworkWifi0 => "network-wireless-signal-none",
            BuiltinIcon::NetworkWifi1 => "network-wireless-signal-weak",
            BuiltinIcon::NetworkWifi2 => "network-wireless-signal-ok",
            BuiltinIcon::NetworkWifi3 => "network-wireless-signal-good",
            BuiltinIcon::NetworkWired => "network-wired",
            BuiltinIcon::NetworkDisconnected => "network-offline",
            BuiltinIcon::NetworkVpn => "network-vpn",
            BuiltinIcon::BatteryEmpty => "battery-empty",
            BuiltinIcon::BatteryLow => "battery-low",
            BuiltinIcon::BatteryMedium => "battery-medium",
            BuiltinIcon::BatteryHigh => "battery-good",
            BuiltinIcon::BatteryFull => "battery-full",
            BuiltinIcon::BatteryCharging => "battery-full-charging",
            BuiltinIcon::VolumeHigh => "audio-volume-high",
            BuiltinIcon::VolumeMedium => "audio-volume-medium",
            BuiltinIcon::VolumeLow => "audio-volume-low",
            BuiltinIcon::VolumeMuted => "audio-volume-muted",
            BuiltinIcon::Notification => "notification-symbolic",
            BuiltinIcon::Bluetooth => "bluetooth-active",
            BuiltinIcon::BluetoothDisabled => "bluetooth-disabled",
            BuiltinIcon::Brightness => "display-brightness-symbolic",
            BuiltinIcon::Night => "night-light-symbolic",
            BuiltinIcon::Airplane => "airplane-mode-symbolic",
            BuiltinIcon::Location => "find-location-symbolic",
            BuiltinIcon::User => "avatar-default",
            BuiltinIcon::Users => "system-users",
            BuiltinIcon::Clock => "appointment-soon",
            BuiltinIcon::Calendar => "x-office-calendar",
            BuiltinIcon::Weather => "weather-clear",
        }
    }

    /// Find builtin icon by name
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "window-close" => Some(BuiltinIcon::WindowClose),
            "window-minimize" => Some(BuiltinIcon::WindowMinimize),
            "window-maximize" => Some(BuiltinIcon::WindowMaximize),
            "window-restore" => Some(BuiltinIcon::WindowRestore),
            "system-shutdown" => Some(BuiltinIcon::SystemShutdown),
            "system-restart" => Some(BuiltinIcon::SystemRestart),
            "system-log-out" => Some(BuiltinIcon::SystemLogout),
            "system-lock-screen" => Some(BuiltinIcon::SystemLock),
            "preferences-system" => Some(BuiltinIcon::SystemSettings),
            "system-search" => Some(BuiltinIcon::SystemSearch),
            "folder-open" => Some(BuiltinIcon::FolderOpen),
            "folder" => Some(BuiltinIcon::FolderClosed),
            "text-x-generic" => Some(BuiltinIcon::FileGeneric),
            "image-x-generic" => Some(BuiltinIcon::FileImage),
            "audio-x-generic" => Some(BuiltinIcon::FileAudio),
            "video-x-generic" => Some(BuiltinIcon::FileVideo),
            "package-x-generic" => Some(BuiltinIcon::FileArchive),
            "text-x-script" => Some(BuiltinIcon::FileCode),
            "list-add" => Some(BuiltinIcon::ActionAdd),
            "list-remove" => Some(BuiltinIcon::ActionRemove),
            "document-edit" => Some(BuiltinIcon::ActionEdit),
            "edit-delete" => Some(BuiltinIcon::ActionDelete),
            "view-refresh" => Some(BuiltinIcon::ActionRefresh),
            "edit-undo" => Some(BuiltinIcon::ActionUndo),
            "edit-redo" => Some(BuiltinIcon::ActionRedo),
            "edit-copy" => Some(BuiltinIcon::ActionCopy),
            "edit-paste" => Some(BuiltinIcon::ActionPaste),
            "edit-cut" => Some(BuiltinIcon::ActionCut),
            "document-save" => Some(BuiltinIcon::ActionSave),
            "document-open" => Some(BuiltinIcon::ActionOpen),
            "go-previous" => Some(BuiltinIcon::NavBack),
            "go-next" => Some(BuiltinIcon::NavForward),
            "go-up" => Some(BuiltinIcon::NavUp),
            "go-down" => Some(BuiltinIcon::NavDown),
            "go-home" => Some(BuiltinIcon::NavHome),
            "dialog-information" => Some(BuiltinIcon::StatusInfo),
            "dialog-warning" => Some(BuiltinIcon::StatusWarning),
            "dialog-error" => Some(BuiltinIcon::StatusError),
            "dialog-question" => Some(BuiltinIcon::StatusQuestion),
            "computer" => Some(BuiltinIcon::DeviceComputer),
            "drive-harddisk" => Some(BuiltinIcon::DeviceDrive),
            "drive-removable-media-usb" => Some(BuiltinIcon::DeviceUSB),
            "network-server" => Some(BuiltinIcon::DeviceNetwork),
            "printer" => Some(BuiltinIcon::DevicePrinter),
            "camera-photo" => Some(BuiltinIcon::DeviceCamera),
            "phone" => Some(BuiltinIcon::DevicePhone),
            "utilities-terminal" => Some(BuiltinIcon::AppTerminal),
            "web-browser" => Some(BuiltinIcon::AppBrowser),
            "system-file-manager" => Some(BuiltinIcon::AppFiles),
            "accessories-calculator" => Some(BuiltinIcon::AppCalculator),
            _ => None,
        }
    }

    /// Render builtin icon to RGBA data
    pub fn render(&self, size: u32, color: u32) -> Vec<u8> {
        // Generate simple geometric icons
        let mut data = vec![0u8; (size * size * 4) as usize];

        let r = ((color >> 16) & 0xFF) as u8;
        let g = ((color >> 8) & 0xFF) as u8;
        let b = (color & 0xFF) as u8;

        match self {
            BuiltinIcon::WindowClose => {
                // X shape
                render_x_shape(&mut data, size, r, g, b);
            }
            BuiltinIcon::WindowMinimize => {
                // Horizontal line
                render_horizontal_line(&mut data, size, r, g, b);
            }
            BuiltinIcon::WindowMaximize => {
                // Square outline
                render_square_outline(&mut data, size, r, g, b);
            }
            BuiltinIcon::FolderClosed | BuiltinIcon::FolderOpen => {
                // Folder shape
                render_folder(&mut data, size, r, g, b);
            }
            BuiltinIcon::FileGeneric | BuiltinIcon::FileText => {
                // File shape
                render_file(&mut data, size, r, g, b);
            }
            BuiltinIcon::ActionAdd => {
                // Plus sign
                render_plus(&mut data, size, r, g, b);
            }
            BuiltinIcon::ActionRemove => {
                // Minus sign
                render_horizontal_line(&mut data, size, r, g, b);
            }
            BuiltinIcon::NavBack => {
                // Left arrow
                render_arrow_left(&mut data, size, r, g, b);
            }
            BuiltinIcon::NavForward => {
                // Right arrow
                render_arrow_right(&mut data, size, r, g, b);
            }
            BuiltinIcon::NavUp => {
                // Up arrow
                render_arrow_up(&mut data, size, r, g, b);
            }
            BuiltinIcon::NavDown => {
                // Down arrow
                render_arrow_down(&mut data, size, r, g, b);
            }
            BuiltinIcon::StatusInfo | BuiltinIcon::StatusQuestion => {
                // Circle with i or ?
                render_circle(&mut data, size, r, g, b);
            }
            BuiltinIcon::StatusWarning => {
                // Triangle
                render_triangle(&mut data, size, r, g, b);
            }
            BuiltinIcon::StatusError => {
                // Circle with X
                render_circle_x(&mut data, size, r, g, b);
            }
            BuiltinIcon::StatusSuccess => {
                // Checkmark
                render_checkmark(&mut data, size, r, g, b);
            }
            _ => {
                // Default: filled circle
                render_circle_filled(&mut data, size, r, g, b);
            }
        }

        data
    }
}

// Helper functions for rendering built-in icons
fn render_x_shape(data: &mut [u8], size: u32, r: u8, g: u8, b: u8) {
    let margin = size / 4;
    let thickness = (size / 8).max(1);

    for y in margin..size - margin {
        for x in margin..size - margin {
            let dx = x.abs_diff(y);
            let dx2 = x.abs_diff(size - 1 - y);
            if dx < thickness || dx2 < thickness {
                set_pixel(data, size, x, y, r, g, b, 255);
            }
        }
    }
}

fn render_horizontal_line(data: &mut [u8], size: u32, r: u8, g: u8, b: u8) {
    let margin = size / 4;
    let y_center = size / 2;
    let thickness = (size / 8).max(1);

    for y in y_center - thickness / 2..y_center + thickness / 2 + 1 {
        for x in margin..size - margin {
            set_pixel(data, size, x, y, r, g, b, 255);
        }
    }
}

fn render_square_outline(data: &mut [u8], size: u32, r: u8, g: u8, b: u8) {
    let margin = size / 4;
    let thickness = (size / 10).max(1);

    for y in margin..size - margin {
        for x in margin..size - margin {
            let is_border = x < margin + thickness
                || x >= size - margin - thickness
                || y < margin + thickness
                || y >= size - margin - thickness;
            if is_border {
                set_pixel(data, size, x, y, r, g, b, 255);
            }
        }
    }
}

fn render_folder(data: &mut [u8], size: u32, r: u8, g: u8, b: u8) {
    let margin = size / 6;
    let tab_width = size / 3;
    let tab_height = size / 8;

    // Tab
    for y in margin..margin + tab_height {
        for x in margin..margin + tab_width {
            set_pixel(data, size, x, y, r, g, b, 255);
        }
    }

    // Body
    for y in margin + tab_height..size - margin {
        for x in margin..size - margin {
            set_pixel(data, size, x, y, r, g, b, 255);
        }
    }
}

fn render_file(data: &mut [u8], size: u32, r: u8, g: u8, b: u8) {
    let margin = size / 5;
    let fold = size / 4;

    for y in margin..size - margin {
        for x in margin..size - margin {
            // Cut corner for dog-ear effect
            if x > size - margin - fold && y < margin + fold {
                if x + y < size - margin + margin + fold {
                    set_pixel(data, size, x, y, r, g, b, 255);
                }
            } else {
                set_pixel(data, size, x, y, r, g, b, 255);
            }
        }
    }
}

fn render_plus(data: &mut [u8], size: u32, r: u8, g: u8, b: u8) {
    let margin = size / 4;
    let thickness = (size / 6).max(1);
    let center = size / 2;

    // Horizontal
    for y in center - thickness / 2..center + thickness / 2 + 1 {
        for x in margin..size - margin {
            set_pixel(data, size, x, y, r, g, b, 255);
        }
    }

    // Vertical
    for y in margin..size - margin {
        for x in center - thickness / 2..center + thickness / 2 + 1 {
            set_pixel(data, size, x, y, r, g, b, 255);
        }
    }
}

fn render_arrow_left(data: &mut [u8], size: u32, r: u8, g: u8, b: u8) {
    let center = size / 2;
    let arrow_size = size / 3;

    for y in 0..size {
        let dy = (y as i32 - center as i32).abs() as u32;
        if dy < arrow_size {
            let x_start = center - arrow_size + dy;
            let x_end = center;
            for x in x_start..x_end.min(size) {
                set_pixel(data, size, x, y, r, g, b, 255);
            }
        }
    }
}

fn render_arrow_right(data: &mut [u8], size: u32, r: u8, g: u8, b: u8) {
    let center = size / 2;
    let arrow_size = size / 3;

    for y in 0..size {
        let dy = (y as i32 - center as i32).abs() as u32;
        if dy < arrow_size {
            let x_start = center;
            let x_end = center + arrow_size - dy;
            for x in x_start..x_end.min(size) {
                set_pixel(data, size, x, y, r, g, b, 255);
            }
        }
    }
}

fn render_arrow_up(data: &mut [u8], size: u32, r: u8, g: u8, b: u8) {
    let center = size / 2;
    let arrow_size = size / 3;

    for x in 0..size {
        let dx = (x as i32 - center as i32).abs() as u32;
        if dx < arrow_size {
            let y_start = center - arrow_size + dx;
            let y_end = center;
            for y in y_start..y_end.min(size) {
                set_pixel(data, size, x, y, r, g, b, 255);
            }
        }
    }
}

fn render_arrow_down(data: &mut [u8], size: u32, r: u8, g: u8, b: u8) {
    let center = size / 2;
    let arrow_size = size / 3;

    for x in 0..size {
        let dx = (x as i32 - center as i32).abs() as u32;
        if dx < arrow_size {
            let y_start = center;
            let y_end = center + arrow_size - dx;
            for y in y_start..y_end.min(size) {
                set_pixel(data, size, x, y, r, g, b, 255);
            }
        }
    }
}

fn render_circle(data: &mut [u8], size: u32, r: u8, g: u8, b: u8) {
    let center = size / 2;
    let radius = size / 3;
    let thickness = (size / 10).max(1);

    for y in 0..size {
        for x in 0..size {
            let dx = (x as i32 - center as i32).abs();
            let dy = (y as i32 - center as i32).abs();
            let dist = sqrt_f32((dx * dx + dy * dy) as f32) as u32;
            if dist >= radius - thickness && dist <= radius + thickness {
                set_pixel(data, size, x, y, r, g, b, 255);
            }
        }
    }
}

fn render_circle_filled(data: &mut [u8], size: u32, r: u8, g: u8, b: u8) {
    let center = size / 2;
    let radius = size / 3;

    for y in 0..size {
        for x in 0..size {
            let dx = (x as i32 - center as i32).abs();
            let dy = (y as i32 - center as i32).abs();
            let dist = sqrt_f32((dx * dx + dy * dy) as f32) as u32;
            if dist <= radius {
                set_pixel(data, size, x, y, r, g, b, 255);
            }
        }
    }
}

fn render_triangle(data: &mut [u8], size: u32, r: u8, g: u8, b: u8) {
    let margin = size / 6;
    let thickness = (size / 10).max(1);

    for y in margin..size - margin {
        let progress = (y - margin) as f32 / (size - 2 * margin) as f32;
        let half_width = (progress * (size / 2 - margin) as f32) as u32;
        let center = size / 2;

        // Left edge
        for t in 0..thickness {
            if center >= half_width + t {
                set_pixel(data, size, center - half_width - t, y, r, g, b, 255);
            }
        }
        // Right edge
        for t in 0..thickness {
            if center + half_width + t < size {
                set_pixel(data, size, center + half_width + t, y, r, g, b, 255);
            }
        }
        // Bottom edge
        if y >= size - margin - thickness {
            for x in center - half_width..center + half_width + 1 {
                if x < size {
                    set_pixel(data, size, x, y, r, g, b, 255);
                }
            }
        }
    }
}

fn render_circle_x(data: &mut [u8], size: u32, r: u8, g: u8, b: u8) {
    render_circle(data, size, r, g, b);

    // X inside
    let center = size / 2;
    let x_size = size / 5;
    let thickness = (size / 12).max(1);

    for i in 0..x_size * 2 {
        let offset = i as i32 - x_size as i32;
        for t in 0..thickness as i32 {
            let x1 = (center as i32 + offset + t) as u32;
            let y1 = (center as i32 + offset) as u32;
            let x2 = (center as i32 + offset + t) as u32;
            let y2 = (center as i32 - offset) as u32;

            if x1 < size && y1 < size {
                set_pixel(data, size, x1, y1, r, g, b, 255);
            }
            if x2 < size && y2 < size {
                set_pixel(data, size, x2, y2, r, g, b, 255);
            }
        }
    }
}

fn render_checkmark(data: &mut [u8], size: u32, r: u8, g: u8, b: u8) {
    let thickness = (size / 6).max(1);
    let start_x = size / 5;
    let mid_x = size * 2 / 5;
    let end_x = size * 4 / 5;
    let start_y = size / 2;
    let mid_y = size * 3 / 4;
    let end_y = size / 4;

    // First line: start to mid (going down-right)
    for i in 0..=mid_x - start_x {
        let x = start_x + i;
        let y = start_y + (i as f32 * (mid_y - start_y) as f32 / (mid_x - start_x) as f32) as u32;
        for t in 0..thickness {
            if y + t < size {
                set_pixel(data, size, x, y + t, r, g, b, 255);
            }
        }
    }

    // Second line: mid to end (going up-right)
    for i in 0..=end_x - mid_x {
        let x = mid_x + i;
        let y = mid_y - (i as f32 * (mid_y - end_y) as f32 / (end_x - mid_x) as f32) as u32;
        for t in 0..thickness {
            if y + t < size {
                set_pixel(data, size, x, y + t, r, g, b, 255);
            }
        }
    }
}

fn set_pixel(data: &mut [u8], size: u32, x: u32, y: u32, r: u8, g: u8, b: u8, a: u8) {
    if x < size && y < size {
        let idx = ((y * size + x) * 4) as usize;
        if idx + 3 < data.len() {
            data[idx] = r;
            data[idx + 1] = g;
            data[idx + 2] = b;
            data[idx + 3] = a;
        }
    }
}

/// Initialize icon theme system
pub fn init() {
    let mut state = ICON_STATE.lock();
    if state.is_some() {
        return;
    }

    let default_theme = create_default_icon_theme();

    let mut themes = BTreeMap::new();
    themes.insert("Stenzel".to_string(), default_theme.clone());
    themes.insert("hicolor".to_string(), create_hicolor_theme());

    *state = Some(IconThemeState {
        current_theme: default_theme,
        themes,
        cache: BTreeMap::new(),
        fallback_theme: "hicolor".to_string(),
        default_sizes: vec![16, 22, 24, 32, 48, 64, 96, 128, 256],
    });

    crate::kprintln!("icons: initialized with Stenzel theme");
}

/// Create default Stenzel icon theme
fn create_default_icon_theme() -> IconTheme {
    IconTheme {
        name: "Stenzel".to_string(),
        display_name: "Stenzel".to_string(),
        comment: "Default Stenzel OS icon theme".to_string(),
        inherits: vec!["hicolor".to_string()],
        directories: vec![
            IconDirectory {
                path: "16x16/actions".to_string(),
                size: 16,
                scale: 1,
                context: IconContext::Actions,
                dir_type: IconDirectoryType::Fixed,
                min_size: None,
                max_size: None,
                threshold: None,
            },
            IconDirectory {
                path: "22x22/actions".to_string(),
                size: 22,
                scale: 1,
                context: IconContext::Actions,
                dir_type: IconDirectoryType::Fixed,
                min_size: None,
                max_size: None,
                threshold: None,
            },
            IconDirectory {
                path: "24x24/actions".to_string(),
                size: 24,
                scale: 1,
                context: IconContext::Actions,
                dir_type: IconDirectoryType::Fixed,
                min_size: None,
                max_size: None,
                threshold: None,
            },
            IconDirectory {
                path: "32x32/actions".to_string(),
                size: 32,
                scale: 1,
                context: IconContext::Actions,
                dir_type: IconDirectoryType::Fixed,
                min_size: None,
                max_size: None,
                threshold: None,
            },
            IconDirectory {
                path: "48x48/actions".to_string(),
                size: 48,
                scale: 1,
                context: IconContext::Actions,
                dir_type: IconDirectoryType::Fixed,
                min_size: None,
                max_size: None,
                threshold: None,
            },
            IconDirectory {
                path: "scalable/actions".to_string(),
                size: 16,
                scale: 1,
                context: IconContext::Actions,
                dir_type: IconDirectoryType::Scalable,
                min_size: Some(16),
                max_size: Some(256),
                threshold: None,
            },
        ],
        hidden: false,
        example: Some("folder".to_string()),
    }
}

/// Create hicolor fallback theme
fn create_hicolor_theme() -> IconTheme {
    IconTheme {
        name: "hicolor".to_string(),
        display_name: "Hicolor".to_string(),
        comment: "Fallback icon theme".to_string(),
        inherits: Vec::new(),
        directories: vec![
            IconDirectory {
                path: "16x16/actions".to_string(),
                size: 16,
                scale: 1,
                context: IconContext::Actions,
                dir_type: IconDirectoryType::Fixed,
                min_size: None,
                max_size: None,
                threshold: None,
            },
        ],
        hidden: true,
        example: None,
    }
}

/// Lookup an icon by name and size
pub fn lookup_icon(name: &str, size: u32, scale: u32) -> Option<IconData> {
    let mut state = ICON_STATE.lock();
    let state = state.as_mut()?;

    // Check cache first
    let cache_key = IconCacheKey {
        theme: state.current_theme.name.clone(),
        name: name.to_string(),
        size,
        scale,
    };

    if let Some(cached) = state.cache.get(&cache_key) {
        return Some(cached.data.clone());
    }

    // Try to find in current theme
    if let Some(icon) = find_icon_in_theme(&state.current_theme, name, size, scale) {
        state.cache.insert(cache_key, CachedIcon {
            data: icon.clone(),
            actual_size: size,
            source_path: String::new(),
        });
        return Some(icon);
    }

    // Try fallback theme
    if let Some(fallback) = state.themes.get(&state.fallback_theme) {
        if let Some(icon) = find_icon_in_theme(fallback, name, size, scale) {
            return Some(icon);
        }
    }

    // Try builtin icons
    if let Some(builtin) = BuiltinIcon::from_name(name) {
        let color = crate::gui::theme::get_current_colors()
            .map(|c| c.foreground.primary)
            .unwrap_or(0x000000);

        let data = builtin.render(size, color);
        let icon_data = IconData::Raster {
            width: size,
            height: size,
            data,
        };

        state.cache.insert(cache_key, CachedIcon {
            data: icon_data.clone(),
            actual_size: size,
            source_path: "builtin".to_string(),
        });

        return Some(icon_data);
    }

    None
}

/// Find icon in a specific theme
fn find_icon_in_theme(theme: &IconTheme, _name: &str, size: u32, _scale: u32) -> Option<IconData> {
    // In a real implementation, this would search the filesystem
    // For now, return None to fall back to builtin icons
    let _ = (theme, size);
    None
}

/// Set current icon theme
pub fn set_theme(name: &str) -> Result<(), IconError> {
    let mut state = ICON_STATE.lock();
    if let Some(ref mut s) = *state {
        if let Some(theme) = s.themes.get(name) {
            s.current_theme = theme.clone();
            s.cache.clear(); // Clear cache on theme change
            crate::kprintln!("icons: switched to theme '{}'", name);
            Ok(())
        } else {
            Err(IconError::ThemeNotFound)
        }
    } else {
        Err(IconError::NotInitialized)
    }
}

/// Get current icon theme name
pub fn get_current_theme() -> Option<String> {
    let state = ICON_STATE.lock();
    state.as_ref().map(|s| s.current_theme.name.clone())
}

/// List available icon themes
pub fn list_themes() -> Vec<(String, String)> {
    let state = ICON_STATE.lock();
    state.as_ref()
        .map(|s| {
            s.themes
                .values()
                .filter(|t| !t.hidden)
                .map(|t| (t.name.clone(), t.display_name.clone()))
                .collect()
        })
        .unwrap_or_default()
}

/// Register a new icon theme
pub fn register_theme(theme: IconTheme) {
    let mut state = ICON_STATE.lock();
    if let Some(ref mut s) = *state {
        s.themes.insert(theme.name.clone(), theme);
    }
}

/// Clear icon cache
pub fn clear_cache() {
    let mut state = ICON_STATE.lock();
    if let Some(ref mut s) = *state {
        s.cache.clear();
    }
}

/// Get icon for file type
pub fn get_file_icon(filename: &str, is_directory: bool, size: u32) -> Option<IconData> {
    if is_directory {
        return lookup_icon("folder", size, 1);
    }

    // Determine icon based on extension
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    let icon_name = match ext.as_str() {
        "txt" | "md" | "rst" => "text-x-generic",
        "rs" | "py" | "js" | "ts" | "c" | "cpp" | "h" | "java" | "go" => "text-x-script",
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "svg" | "webp" => "image-x-generic",
        "mp3" | "wav" | "ogg" | "flac" | "m4a" => "audio-x-generic",
        "mp4" | "mkv" | "avi" | "mov" | "webm" => "video-x-generic",
        "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" => "package-x-generic",
        "pdf" => "application-pdf",
        "doc" | "docx" | "odt" => "x-office-document",
        "xls" | "xlsx" | "ods" => "x-office-spreadsheet",
        "ppt" | "pptx" | "odp" => "x-office-presentation",
        _ => "text-x-generic",
    };

    lookup_icon(icon_name, size, 1)
}

/// Icon error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconError {
    NotInitialized,
    ThemeNotFound,
    IconNotFound,
    InvalidFormat,
}
