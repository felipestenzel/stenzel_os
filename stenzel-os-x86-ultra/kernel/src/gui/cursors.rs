//! Cursor Theme System
//!
//! Provides cursor themes with animated cursors and HiDPI support.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use spin::Mutex;

// ============================================================================
// Math Helpers (no_std compatible)
// ============================================================================

/// Approximate sin for f32 using Taylor series
fn sin_f32(x: f32) -> f32 {
    // Normalize to -PI..PI
    let pi = core::f32::consts::PI;
    let mut x = x % (2.0 * pi);
    if x > pi { x -= 2.0 * pi; }
    if x < -pi { x += 2.0 * pi; }

    // Taylor series: sin(x) = x - x^3/3! + x^5/5! - x^7/7! + ...
    let x2 = x * x;
    let x3 = x2 * x;
    let x5 = x3 * x2;
    let x7 = x5 * x2;
    x - x3 / 6.0 + x5 / 120.0 - x7 / 5040.0
}

/// Approximate cos for f32 using Taylor series
fn cos_f32(x: f32) -> f32 {
    sin_f32(x + core::f32::consts::PI / 2.0)
}

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

/// Global cursor theme state
static CURSOR_STATE: Mutex<Option<CursorThemeState>> = Mutex::new(None);

/// Cursor theme state
pub struct CursorThemeState {
    /// Current theme
    pub current_theme: CursorTheme,
    /// Available themes
    pub themes: BTreeMap<String, CursorTheme>,
    /// Current cursor
    pub current_cursor: CursorType,
    /// Current cursor image
    pub current_image: Option<CursorImage>,
    /// Animation state
    pub animation: Option<AnimationState>,
    /// Default size
    pub default_size: u32,
    /// Current scale factor
    pub scale: u32,
}

/// Cursor theme definition
#[derive(Debug, Clone)]
pub struct CursorTheme {
    /// Theme name
    pub name: String,
    /// Theme display name
    pub display_name: String,
    /// Theme comment
    pub comment: String,
    /// Inherits from
    pub inherits: Option<String>,
    /// Cursors
    pub cursors: BTreeMap<CursorType, CursorDefinition>,
}

/// Cursor type
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CursorType {
    /// Default pointer
    Default,
    /// Text cursor (I-beam)
    Text,
    /// Wait/busy cursor
    Wait,
    /// Crosshair
    Crosshair,
    /// Help cursor
    Help,
    /// Pointer (hand)
    Pointer,
    /// Progress (busy but still usable)
    Progress,
    /// Not allowed
    NotAllowed,
    /// Resize (all directions)
    Move,
    /// Resize horizontal
    ResizeEw,
    /// Resize vertical
    ResizeNs,
    /// Resize diagonal (NE-SW)
    ResizeNesw,
    /// Resize diagonal (NW-SE)
    ResizeNwse,
    /// Resize north
    ResizeN,
    /// Resize south
    ResizeS,
    /// Resize east
    ResizeE,
    /// Resize west
    ResizeW,
    /// Resize north-east
    ResizeNe,
    /// Resize north-west
    ResizeNw,
    /// Resize south-east
    ResizeSe,
    /// Resize south-west
    ResizeSw,
    /// Grabbing
    Grabbing,
    /// Grab
    Grab,
    /// Zoom in
    ZoomIn,
    /// Zoom out
    ZoomOut,
    /// Cell selection
    Cell,
    /// Copy
    Copy,
    /// Alias
    Alias,
    /// Context menu
    ContextMenu,
    /// Vertical text
    VerticalText,
    /// No drop
    NoDrop,
    /// Row resize
    RowResize,
    /// Column resize
    ColResize,
}

impl CursorType {
    /// Get X11/Xcursor name
    pub fn x11_name(&self) -> &'static str {
        match self {
            CursorType::Default => "default",
            CursorType::Text => "text",
            CursorType::Wait => "wait",
            CursorType::Crosshair => "crosshair",
            CursorType::Help => "help",
            CursorType::Pointer => "pointer",
            CursorType::Progress => "progress",
            CursorType::NotAllowed => "not-allowed",
            CursorType::Move => "move",
            CursorType::ResizeEw => "ew-resize",
            CursorType::ResizeNs => "ns-resize",
            CursorType::ResizeNesw => "nesw-resize",
            CursorType::ResizeNwse => "nwse-resize",
            CursorType::ResizeN => "n-resize",
            CursorType::ResizeS => "s-resize",
            CursorType::ResizeE => "e-resize",
            CursorType::ResizeW => "w-resize",
            CursorType::ResizeNe => "ne-resize",
            CursorType::ResizeNw => "nw-resize",
            CursorType::ResizeSe => "se-resize",
            CursorType::ResizeSw => "sw-resize",
            CursorType::Grabbing => "grabbing",
            CursorType::Grab => "grab",
            CursorType::ZoomIn => "zoom-in",
            CursorType::ZoomOut => "zoom-out",
            CursorType::Cell => "cell",
            CursorType::Copy => "copy",
            CursorType::Alias => "alias",
            CursorType::ContextMenu => "context-menu",
            CursorType::VerticalText => "vertical-text",
            CursorType::NoDrop => "no-drop",
            CursorType::RowResize => "row-resize",
            CursorType::ColResize => "col-resize",
        }
    }

    /// Get all cursor types
    pub fn all() -> &'static [CursorType] {
        &[
            CursorType::Default,
            CursorType::Text,
            CursorType::Wait,
            CursorType::Crosshair,
            CursorType::Help,
            CursorType::Pointer,
            CursorType::Progress,
            CursorType::NotAllowed,
            CursorType::Move,
            CursorType::ResizeEw,
            CursorType::ResizeNs,
            CursorType::ResizeNesw,
            CursorType::ResizeNwse,
            CursorType::Grabbing,
            CursorType::Grab,
            CursorType::ZoomIn,
            CursorType::ZoomOut,
        ]
    }
}

/// Cursor definition
#[derive(Debug, Clone)]
pub struct CursorDefinition {
    /// Frames (for animated cursors)
    pub frames: Vec<CursorFrame>,
    /// Animation delay between frames (ms)
    pub delay_ms: u32,
}

/// Single cursor frame
#[derive(Debug, Clone)]
pub struct CursorFrame {
    /// Image data (RGBA)
    pub image: Vec<u8>,
    /// Width
    pub width: u32,
    /// Height
    pub height: u32,
    /// Hotspot X
    pub hotspot_x: u32,
    /// Hotspot Y
    pub hotspot_y: u32,
}

/// Current cursor image
#[derive(Debug, Clone)]
pub struct CursorImage {
    /// Image data (RGBA)
    pub data: Vec<u8>,
    /// Width
    pub width: u32,
    /// Height
    pub height: u32,
    /// Hotspot X
    pub hotspot_x: u32,
    /// Hotspot Y
    pub hotspot_y: u32,
}

/// Animation state
#[derive(Debug, Clone)]
pub struct AnimationState {
    /// Current frame index
    pub frame_index: usize,
    /// Total frames
    pub total_frames: usize,
    /// Delay between frames (ms)
    pub delay_ms: u32,
    /// Last frame change time
    pub last_change_ms: u64,
}

/// Initialize cursor theme system
pub fn init() {
    let mut state = CURSOR_STATE.lock();
    if state.is_some() {
        return;
    }

    let default_theme = create_default_cursor_theme();

    let mut themes = BTreeMap::new();
    themes.insert("Stenzel".to_string(), default_theme.clone());

    *state = Some(CursorThemeState {
        current_theme: default_theme,
        themes,
        current_cursor: CursorType::Default,
        current_image: None,
        animation: None,
        default_size: 24,
        scale: 1,
    });

    // Set initial cursor
    drop(state);
    set_cursor(CursorType::Default);

    crate::kprintln!("cursors: initialized with Stenzel theme");
}

/// Create default cursor theme
fn create_default_cursor_theme() -> CursorTheme {
    let mut cursors = BTreeMap::new();

    // Create builtin cursors
    for cursor_type in CursorType::all() {
        let frames = vec![render_builtin_cursor(*cursor_type, 24)];
        cursors.insert(*cursor_type, CursorDefinition {
            frames,
            delay_ms: 0,
        });
    }

    // Create animated wait cursor
    let wait_frames = create_animated_wait_cursor(24);
    cursors.insert(CursorType::Wait, CursorDefinition {
        frames: wait_frames,
        delay_ms: 100,
    });

    // Create animated progress cursor
    let progress_frames = create_animated_progress_cursor(24);
    cursors.insert(CursorType::Progress, CursorDefinition {
        frames: progress_frames,
        delay_ms: 100,
    });

    CursorTheme {
        name: "Stenzel".to_string(),
        display_name: "Stenzel".to_string(),
        comment: "Default Stenzel OS cursor theme".to_string(),
        inherits: None,
        cursors,
    }
}

/// Render a builtin cursor
fn render_builtin_cursor(cursor_type: CursorType, size: u32) -> CursorFrame {
    let mut data = vec![0u8; (size * size * 4) as usize];
    let (hotspot_x, hotspot_y) = match cursor_type {
        CursorType::Default | CursorType::Pointer => (0, 0),
        CursorType::Text => (size / 2, size / 2),
        CursorType::Crosshair => (size / 2, size / 2),
        CursorType::Move => (size / 2, size / 2),
        CursorType::ResizeEw | CursorType::ColResize => (size / 2, size / 2),
        CursorType::ResizeNs | CursorType::RowResize => (size / 2, size / 2),
        CursorType::ResizeNesw | CursorType::ResizeNwse => (size / 2, size / 2),
        _ => (size / 2, size / 2),
    };

    match cursor_type {
        CursorType::Default => render_arrow_cursor(&mut data, size),
        CursorType::Text => render_ibeam_cursor(&mut data, size),
        CursorType::Wait => render_wait_cursor(&mut data, size, 0),
        CursorType::Crosshair => render_crosshair_cursor(&mut data, size),
        CursorType::Help => render_help_cursor(&mut data, size),
        CursorType::Pointer => render_hand_cursor(&mut data, size),
        CursorType::Progress => render_progress_cursor(&mut data, size, 0),
        CursorType::NotAllowed => render_not_allowed_cursor(&mut data, size),
        CursorType::Move => render_move_cursor(&mut data, size),
        CursorType::ResizeEw | CursorType::ColResize => render_resize_ew_cursor(&mut data, size),
        CursorType::ResizeNs | CursorType::RowResize => render_resize_ns_cursor(&mut data, size),
        CursorType::ResizeNesw => render_resize_nesw_cursor(&mut data, size),
        CursorType::ResizeNwse => render_resize_nwse_cursor(&mut data, size),
        CursorType::Grabbing => render_grabbing_cursor(&mut data, size),
        CursorType::Grab => render_grab_cursor(&mut data, size),
        _ => render_arrow_cursor(&mut data, size),
    }

    CursorFrame {
        image: data,
        width: size,
        height: size,
        hotspot_x,
        hotspot_y,
    }
}

/// Render arrow cursor
fn render_arrow_cursor(data: &mut [u8], size: u32) {
    let outline_color = (0, 0, 0, 255);
    let fill_color = (255, 255, 255, 255);

    // Arrow shape
    for y in 0..size {
        let max_x = (y as f32 * 0.7) as u32;
        for x in 0..max_x.min(size) {
            // Check if on edge
            let is_edge = x == 0 || x == max_x - 1 || y == size - 1 ||
                (y > size / 2 && x == (y as f32 * 0.3) as u32);

            let (r, g, b, a) = if is_edge { outline_color } else { fill_color };
            set_pixel(data, size, x, y, r, g, b, a);
        }
    }
}

/// Render I-beam cursor
fn render_ibeam_cursor(data: &mut [u8], size: u32) {
    let color = (0, 0, 0, 255);
    let center_x = size / 2;
    let thickness = (size / 12).max(1);

    // Vertical bar
    for y in size / 6..size - size / 6 {
        for t in 0..thickness {
            set_pixel(data, size, center_x + t - thickness / 2, y, color.0, color.1, color.2, color.3);
        }
    }

    // Top serif
    for x in center_x - size / 4..center_x + size / 4 {
        for t in 0..thickness {
            set_pixel(data, size, x, size / 6 + t, color.0, color.1, color.2, color.3);
        }
    }

    // Bottom serif
    for x in center_x - size / 4..center_x + size / 4 {
        for t in 0..thickness {
            set_pixel(data, size, x, size - size / 6 - t, color.0, color.1, color.2, color.3);
        }
    }
}

/// Render wait cursor (spinner)
fn render_wait_cursor(data: &mut [u8], size: u32, frame: usize) {
    let center = size as f32 / 2.0;
    let outer_radius = size as f32 / 2.0 - 2.0;
    let inner_radius = outer_radius * 0.5;
    let segments = 8;
    let active_segment = frame % segments;

    for segment in 0..segments {
        let angle = (segment as f32 / segments as f32) * 2.0 * core::f32::consts::PI - core::f32::consts::PI / 2.0;
        let intensity = if segment == active_segment {
            255
        } else {
            ((segment as i32 - active_segment as i32).abs() as f32 / segments as f32 * 200.0) as u8 + 55
        };

        // Draw segment
        for r in (inner_radius as u32)..(outer_radius as u32) {
            let r_f = r as f32;
            let x = center + cos_f32(angle) * r_f;
            let y = center + sin_f32(angle) * r_f;
            if x >= 0.0 && x < size as f32 && y >= 0.0 && y < size as f32 {
                set_pixel(data, size, x as u32, y as u32, 0, 0, 0, intensity);
            }
        }
    }
}

/// Render crosshair cursor
fn render_crosshair_cursor(data: &mut [u8], size: u32) {
    let color = (0, 0, 0, 255);
    let center = size / 2;
    let thickness = (size / 16).max(1);
    let gap = size / 8;

    // Horizontal
    for x in 0..size {
        if (x as i32 - center as i32).unsigned_abs() > gap {
            for t in 0..thickness {
                set_pixel(data, size, x, center + t - thickness / 2, color.0, color.1, color.2, color.3);
            }
        }
    }

    // Vertical
    for y in 0..size {
        if (y as i32 - center as i32).unsigned_abs() > gap {
            for t in 0..thickness {
                set_pixel(data, size, center + t - thickness / 2, y, color.0, color.1, color.2, color.3);
            }
        }
    }
}

/// Render help cursor
fn render_help_cursor(data: &mut [u8], size: u32) {
    // Arrow base
    render_arrow_cursor(data, size);

    // Question mark
    let qm_size = size / 3;
    let qm_x = size - qm_size - 2;
    let qm_y = size - qm_size - 2;
    let color = (0, 0, 255, 255);

    // Draw question mark circle
    let center_x = qm_x + qm_size / 2;
    let center_y = qm_y + qm_size / 3;
    let radius = qm_size / 3;

    for angle in 0..180 {
        let rad = (angle as f32) * core::f32::consts::PI / 180.0;
        let x = center_x as f32 + cos_f32(rad) * radius as f32;
        let y = center_y as f32 - sin_f32(rad) * radius as f32;
        if x >= 0.0 && x < size as f32 && y >= 0.0 && y < size as f32 {
            set_pixel(data, size, x as u32, y as u32, color.0, color.1, color.2, color.3);
        }
    }

    // Stem
    for dy in 0..qm_size / 4 {
        set_pixel(data, size, center_x, qm_y + qm_size / 2 + dy, color.0, color.1, color.2, color.3);
    }

    // Dot
    for dx in 0..2 {
        for dy in 0..2 {
            set_pixel(data, size, center_x + dx, qm_y + qm_size - 2 + dy, color.0, color.1, color.2, color.3);
        }
    }
}

/// Render hand/pointer cursor
fn render_hand_cursor(data: &mut [u8], size: u32) {
    let outline = (0, 0, 0, 255);
    let fill = (255, 255, 255, 255);

    // Simplified hand shape - pointing finger
    let finger_width = size / 4;
    let finger_height = size * 2 / 3;

    // Index finger
    for y in 0..finger_height {
        for x in size / 3..size / 3 + finger_width {
            let is_edge = x == size / 3 || x == size / 3 + finger_width - 1 || y == 0 || y == finger_height - 1;
            let (r, g, b, a) = if is_edge { outline } else { fill };
            set_pixel(data, size, x, y, r, g, b, a);
        }
    }

    // Palm
    for y in finger_height..size {
        for x in size / 6..size - size / 6 {
            let is_edge = x == size / 6 || x == size - size / 6 - 1 || y == size - 1;
            let (r, g, b, a) = if is_edge { outline } else { fill };
            set_pixel(data, size, x, y, r, g, b, a);
        }
    }
}

/// Render progress cursor (arrow with spinner)
fn render_progress_cursor(data: &mut [u8], size: u32, frame: usize) {
    // Arrow in upper left
    let arrow_size = size * 2 / 3;
    for y in 0..arrow_size {
        let max_x = (y as f32 * 0.7) as u32;
        for x in 0..max_x.min(arrow_size) {
            let is_edge = x == 0 || x == max_x - 1;
            let (r, g, b, a) = if is_edge { (0, 0, 0, 255) } else { (255, 255, 255, 255) };
            set_pixel(data, size, x, y, r, g, b, a);
        }
    }

    // Small spinner in lower right
    let spinner_center_x = size - size / 4;
    let spinner_center_y = size - size / 4;
    let spinner_radius = size / 6;
    let segments = 8;
    let active = frame % segments;

    for seg in 0..segments {
        let angle = (seg as f32 / segments as f32) * 2.0 * core::f32::consts::PI;
        let intensity = if seg == active { 255 } else { 100 };

        let x = spinner_center_x as f32 + cos_f32(angle) * spinner_radius as f32;
        let y = spinner_center_y as f32 + sin_f32(angle) * spinner_radius as f32;
        if x >= 0.0 && x < size as f32 && y >= 0.0 && y < size as f32 {
            for dx in 0..2 {
                for dy in 0..2 {
                    let px = x as u32 + dx;
                    let py = y as u32 + dy;
                    if px < size && py < size {
                        set_pixel(data, size, px, py, 0, 0, 0, intensity);
                    }
                }
            }
        }
    }
}

/// Render not allowed cursor
fn render_not_allowed_cursor(data: &mut [u8], size: u32) {
    let color = (255, 0, 0, 255);
    let center = size / 2;
    let radius = size / 3;
    let thickness = (size / 10).max(2);

    // Circle
    for angle in 0..360 {
        let rad = (angle as f32) * core::f32::consts::PI / 180.0;
        for t in 0..thickness {
            let r = radius as f32 - t as f32;
            let x = center as f32 + cos_f32(rad) * r;
            let y = center as f32 + sin_f32(rad) * r;
            if x >= 0.0 && x < size as f32 && y >= 0.0 && y < size as f32 {
                set_pixel(data, size, x as u32, y as u32, color.0, color.1, color.2, color.3);
            }
        }
    }

    // Diagonal line
    for i in 0..radius * 2 {
        let offset = i as i32 - radius as i32;
        for t in 0..thickness {
            let x = (center as i32 + offset) as u32;
            let y = (center as i32 - offset + t as i32) as u32;
            if x < size && y < size {
                set_pixel(data, size, x, y, color.0, color.1, color.2, color.3);
            }
        }
    }
}

/// Render move cursor (four arrows)
fn render_move_cursor(data: &mut [u8], size: u32) {
    let color = (0, 0, 0, 255);
    let center = size / 2;
    let arrow_len = size / 4;
    let thickness = (size / 12).max(1);

    // Cross
    for i in 0..size {
        for t in 0..thickness {
            // Horizontal
            set_pixel(data, size, i, center + t - thickness / 2, color.0, color.1, color.2, color.3);
            // Vertical
            set_pixel(data, size, center + t - thickness / 2, i, color.0, color.1, color.2, color.3);
        }
    }

    // Arrow heads
    for i in 0..arrow_len {
        // Up
        for t in 0..thickness {
            set_pixel(data, size, center - i + t, i, color.0, color.1, color.2, color.3);
            set_pixel(data, size, center + i + t, i, color.0, color.1, color.2, color.3);
        }
        // Down
        for t in 0..thickness {
            set_pixel(data, size, center - i + t, size - 1 - i, color.0, color.1, color.2, color.3);
            set_pixel(data, size, center + i + t, size - 1 - i, color.0, color.1, color.2, color.3);
        }
        // Left
        for t in 0..thickness {
            set_pixel(data, size, i, center - i + t, color.0, color.1, color.2, color.3);
            set_pixel(data, size, i, center + i + t, color.0, color.1, color.2, color.3);
        }
        // Right
        for t in 0..thickness {
            set_pixel(data, size, size - 1 - i, center - i + t, color.0, color.1, color.2, color.3);
            set_pixel(data, size, size - 1 - i, center + i + t, color.0, color.1, color.2, color.3);
        }
    }
}

/// Render horizontal resize cursor
fn render_resize_ew_cursor(data: &mut [u8], size: u32) {
    let color = (0, 0, 0, 255);
    let center_y = size / 2;
    let arrow_len = size / 4;
    let thickness = (size / 12).max(1);

    // Horizontal line
    for x in arrow_len..size - arrow_len {
        for t in 0..thickness {
            set_pixel(data, size, x, center_y + t - thickness / 2, color.0, color.1, color.2, color.3);
        }
    }

    // Left arrow
    for i in 0..arrow_len {
        for t in 0..thickness {
            set_pixel(data, size, i, center_y - (arrow_len - i) + t, color.0, color.1, color.2, color.3);
            set_pixel(data, size, i, center_y + (arrow_len - i) + t, color.0, color.1, color.2, color.3);
        }
    }

    // Right arrow
    for i in 0..arrow_len {
        for t in 0..thickness {
            set_pixel(data, size, size - 1 - i, center_y - (arrow_len - i) + t, color.0, color.1, color.2, color.3);
            set_pixel(data, size, size - 1 - i, center_y + (arrow_len - i) + t, color.0, color.1, color.2, color.3);
        }
    }
}

/// Render vertical resize cursor
fn render_resize_ns_cursor(data: &mut [u8], size: u32) {
    let color = (0, 0, 0, 255);
    let center_x = size / 2;
    let arrow_len = size / 4;
    let thickness = (size / 12).max(1);

    // Vertical line
    for y in arrow_len..size - arrow_len {
        for t in 0..thickness {
            set_pixel(data, size, center_x + t - thickness / 2, y, color.0, color.1, color.2, color.3);
        }
    }

    // Top arrow
    for i in 0..arrow_len {
        for t in 0..thickness {
            set_pixel(data, size, center_x - (arrow_len - i) + t, i, color.0, color.1, color.2, color.3);
            set_pixel(data, size, center_x + (arrow_len - i) + t, i, color.0, color.1, color.2, color.3);
        }
    }

    // Bottom arrow
    for i in 0..arrow_len {
        for t in 0..thickness {
            set_pixel(data, size, center_x - (arrow_len - i) + t, size - 1 - i, color.0, color.1, color.2, color.3);
            set_pixel(data, size, center_x + (arrow_len - i) + t, size - 1 - i, color.0, color.1, color.2, color.3);
        }
    }
}

/// Render diagonal resize cursor (NE-SW)
fn render_resize_nesw_cursor(data: &mut [u8], size: u32) {
    let color = (0, 0, 0, 255);
    let thickness = (size / 10).max(1);

    // Diagonal line
    for i in 0..size {
        for t in 0..thickness {
            let x = i;
            let y = size - 1 - i;
            if x + t < size && y < size {
                set_pixel(data, size, x + t, y, color.0, color.1, color.2, color.3);
            }
        }
    }
}

/// Render diagonal resize cursor (NW-SE)
fn render_resize_nwse_cursor(data: &mut [u8], size: u32) {
    let color = (0, 0, 0, 255);
    let thickness = (size / 10).max(1);

    // Diagonal line
    for i in 0..size {
        for t in 0..thickness {
            if i + t < size {
                set_pixel(data, size, i + t, i, color.0, color.1, color.2, color.3);
            }
        }
    }
}

/// Render grabbing cursor (closed hand)
fn render_grabbing_cursor(data: &mut [u8], size: u32) {
    let outline = (0, 0, 0, 255);
    let fill = (255, 255, 255, 255);

    // Simple closed fist
    let fist_width = size * 2 / 3;
    let fist_height = size / 2;
    let start_x = (size - fist_width) / 2;
    let start_y = size / 3;

    for y in start_y..start_y + fist_height {
        for x in start_x..start_x + fist_width {
            let is_edge = x == start_x || x == start_x + fist_width - 1 ||
                          y == start_y || y == start_y + fist_height - 1;
            let (r, g, b, a) = if is_edge { outline } else { fill };
            set_pixel(data, size, x, y, r, g, b, a);
        }
    }
}

/// Render grab cursor (open hand)
fn render_grab_cursor(data: &mut [u8], size: u32) {
    let outline = (0, 0, 0, 255);
    let fill = (255, 255, 255, 255);

    // Simple open hand with fingers
    let palm_width = size / 2;
    let palm_height = size / 3;
    let finger_width = size / 8;
    let finger_height = size / 3;

    let palm_x = (size - palm_width) / 2;
    let palm_y = size - palm_height - size / 6;

    // Palm
    for y in palm_y..palm_y + palm_height {
        for x in palm_x..palm_x + palm_width {
            let is_edge = x == palm_x || x == palm_x + palm_width - 1 ||
                          y == palm_y || y == palm_y + palm_height - 1;
            let (r, g, b, a) = if is_edge { outline } else { fill };
            set_pixel(data, size, x, y, r, g, b, a);
        }
    }

    // Fingers
    for finger in 0..4 {
        let fx = palm_x + finger * finger_width + finger_width / 2;
        let fy = palm_y - finger_height;

        for y in fy..palm_y {
            for dx in 0..finger_width {
                let x = fx + dx;
                if x < size {
                    let is_edge = dx == 0 || dx == finger_width - 1 || y == fy;
                    let (r, g, b, a) = if is_edge { outline } else { fill };
                    set_pixel(data, size, x, y, r, g, b, a);
                }
            }
        }
    }
}

/// Create animated wait cursor frames
fn create_animated_wait_cursor(size: u32) -> Vec<CursorFrame> {
    let mut frames = Vec::new();
    let num_frames = 8;

    for frame in 0..num_frames {
        let mut data = vec![0u8; (size * size * 4) as usize];
        render_wait_cursor(&mut data, size, frame);
        frames.push(CursorFrame {
            image: data,
            width: size,
            height: size,
            hotspot_x: size / 2,
            hotspot_y: size / 2,
        });
    }

    frames
}

/// Create animated progress cursor frames
fn create_animated_progress_cursor(size: u32) -> Vec<CursorFrame> {
    let mut frames = Vec::new();
    let num_frames = 8;

    for frame in 0..num_frames {
        let mut data = vec![0u8; (size * size * 4) as usize];
        render_progress_cursor(&mut data, size, frame);
        frames.push(CursorFrame {
            image: data,
            width: size,
            height: size,
            hotspot_x: 0,
            hotspot_y: 0,
        });
    }

    frames
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

/// Set current cursor
pub fn set_cursor(cursor_type: CursorType) {
    let mut state = CURSOR_STATE.lock();
    if let Some(ref mut s) = *state {
        s.current_cursor = cursor_type;

        if let Some(def) = s.current_theme.cursors.get(&cursor_type) {
            if !def.frames.is_empty() {
                let frame = &def.frames[0];
                s.current_image = Some(CursorImage {
                    data: frame.image.clone(),
                    width: frame.width,
                    height: frame.height,
                    hotspot_x: frame.hotspot_x,
                    hotspot_y: frame.hotspot_y,
                });

                // Setup animation if multi-frame
                if def.frames.len() > 1 {
                    s.animation = Some(AnimationState {
                        frame_index: 0,
                        total_frames: def.frames.len(),
                        delay_ms: def.delay_ms,
                        last_change_ms: 0,
                    });
                } else {
                    s.animation = None;
                }
            }
        }
    }
}

/// Get current cursor image
pub fn get_cursor_image() -> Option<CursorImage> {
    let state = CURSOR_STATE.lock();
    state.as_ref().and_then(|s| s.current_image.clone())
}

/// Get current cursor type
pub fn get_cursor_type() -> CursorType {
    let state = CURSOR_STATE.lock();
    state.as_ref().map(|s| s.current_cursor).unwrap_or(CursorType::Default)
}

/// Update cursor animation
pub fn update_animation(current_time_ms: u64) {
    let mut state = CURSOR_STATE.lock();
    if let Some(ref mut s) = *state {
        if let Some(ref mut anim) = s.animation {
            if current_time_ms - anim.last_change_ms >= anim.delay_ms as u64 {
                anim.frame_index = (anim.frame_index + 1) % anim.total_frames;
                anim.last_change_ms = current_time_ms;

                // Update current image
                if let Some(def) = s.current_theme.cursors.get(&s.current_cursor) {
                    if anim.frame_index < def.frames.len() {
                        let frame = &def.frames[anim.frame_index];
                        s.current_image = Some(CursorImage {
                            data: frame.image.clone(),
                            width: frame.width,
                            height: frame.height,
                            hotspot_x: frame.hotspot_x,
                            hotspot_y: frame.hotspot_y,
                        });
                    }
                }
            }
        }
    }
}

/// Set cursor size
pub fn set_cursor_size(size: u32) {
    let mut state = CURSOR_STATE.lock();
    if let Some(ref mut s) = *state {
        s.default_size = size;
        // Regenerate cursors at new size
        for cursor_type in CursorType::all() {
            let frames = vec![render_builtin_cursor(*cursor_type, size)];
            if let Some(def) = s.current_theme.cursors.get_mut(cursor_type) {
                def.frames = frames;
            }
        }
        // Refresh current cursor
        let current = s.current_cursor;
        drop(state);
        set_cursor(current);
    }
}

/// Set cursor theme
pub fn set_theme(name: &str) -> Result<(), CursorError> {
    let mut state = CURSOR_STATE.lock();
    if let Some(ref mut s) = *state {
        if let Some(theme) = s.themes.get(name) {
            s.current_theme = theme.clone();
            let current = s.current_cursor;
            drop(state);
            set_cursor(current);
            Ok(())
        } else {
            Err(CursorError::ThemeNotFound)
        }
    } else {
        Err(CursorError::NotInitialized)
    }
}

/// Get current cursor theme name
pub fn get_current_theme() -> Option<String> {
    let state = CURSOR_STATE.lock();
    state.as_ref().map(|s| s.current_theme.name.clone())
}

/// List available cursor themes
pub fn list_themes() -> Vec<String> {
    let state = CURSOR_STATE.lock();
    state.as_ref()
        .map(|s| s.themes.keys().cloned().collect())
        .unwrap_or_default()
}

/// Cursor error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorError {
    NotInitialized,
    ThemeNotFound,
}
