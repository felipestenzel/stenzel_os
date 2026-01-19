//! Desktop module
//!
//! Provides desktop background, wallpaper, and desktop icons functionality.

use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;

use crate::drivers::framebuffer::Color;
use super::surface::{Surface, PixelFormat};

/// Wallpaper type
#[derive(Debug, Clone)]
pub enum Wallpaper {
    /// Solid color
    SolidColor(Color),
    /// Horizontal gradient (left to right)
    HorizontalGradient { start: Color, end: Color },
    /// Vertical gradient (top to bottom)
    VerticalGradient { start: Color, end: Color },
    /// Radial gradient (center to edges)
    RadialGradient { center: Color, edge: Color },
    /// Image (stored as surface)
    Image(WallpaperImage),
}

/// Wallpaper image with scaling options
#[derive(Debug, Clone)]
pub struct WallpaperImage {
    /// Image surface
    pub data: Vec<u8>,
    /// Image width
    pub width: usize,
    /// Image height
    pub height: usize,
    /// Scaling mode
    pub scale_mode: ScaleMode,
}

/// Image scaling mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleMode {
    /// Stretch to fill (may distort)
    Stretch,
    /// Fit within screen (preserve aspect ratio, may have borders)
    Fit,
    /// Fill screen (preserve aspect ratio, may crop)
    Fill,
    /// Center without scaling
    Center,
    /// Tile the image
    Tile,
}

/// Desktop icon
#[derive(Debug, Clone)]
pub struct DesktopIcon {
    /// Icon ID
    pub id: u64,
    /// Icon name/label
    pub name: String,
    /// X position (grid units)
    pub grid_x: usize,
    /// Y position (grid units)
    pub grid_y: usize,
    /// Icon image (32x32 pixels, RGBA)
    pub icon_data: Option<Vec<u8>>,
    /// Path to open when clicked
    pub target_path: String,
    /// Whether icon is selected
    pub selected: bool,
}

static NEXT_ICON_ID: Mutex<u64> = Mutex::new(1);

impl DesktopIcon {
    /// Create a new desktop icon
    pub fn new(name: &str, grid_x: usize, grid_y: usize, target_path: &str) -> Self {
        let mut next = NEXT_ICON_ID.lock();
        let id = *next;
        *next += 1;

        Self {
            id,
            name: String::from(name),
            grid_x,
            grid_y,
            icon_data: None,
            target_path: String::from(target_path),
            selected: false,
        }
    }

    /// Create with icon data
    pub fn with_icon(mut self, data: Vec<u8>) -> Self {
        self.icon_data = Some(data);
        self
    }
}

/// Desktop state
pub struct Desktop {
    /// Wallpaper
    wallpaper: Wallpaper,
    /// Rendered wallpaper surface
    wallpaper_surface: Surface,
    /// Desktop icons
    icons: Vec<DesktopIcon>,
    /// Icon grid cell size
    icon_cell_width: usize,
    icon_cell_height: usize,
    /// Icon size
    icon_size: usize,
    /// Screen dimensions
    screen_width: usize,
    screen_height: usize,
    /// Selected icon (if any)
    selected_icon: Option<u64>,
    /// Whether desktop needs redraw
    dirty: bool,
}

impl Desktop {
    /// Create a new desktop
    pub fn new(screen_width: usize, screen_height: usize) -> Self {
        let wallpaper = Wallpaper::VerticalGradient {
            start: Color::new(32, 64, 128),
            end: Color::new(16, 32, 64),
        };

        let mut desktop = Self {
            wallpaper,
            wallpaper_surface: Surface::new(screen_width, screen_height, PixelFormat::Rgba8888),
            icons: Vec::new(),
            icon_cell_width: 80,
            icon_cell_height: 90,
            icon_size: 32,
            screen_width,
            screen_height,
            selected_icon: None,
            dirty: true,
        };

        desktop.render_wallpaper();
        desktop
    }

    /// Set wallpaper
    pub fn set_wallpaper(&mut self, wallpaper: Wallpaper) {
        self.wallpaper = wallpaper;
        self.render_wallpaper();
        self.dirty = true;
    }

    /// Get wallpaper surface
    pub fn wallpaper_surface(&self) -> &Surface {
        &self.wallpaper_surface
    }

    /// Add a desktop icon
    pub fn add_icon(&mut self, icon: DesktopIcon) {
        self.icons.push(icon);
        self.dirty = true;
    }

    /// Remove an icon by ID
    pub fn remove_icon(&mut self, id: u64) -> bool {
        let len_before = self.icons.len();
        self.icons.retain(|i| i.id != id);
        if self.icons.len() != len_before {
            self.dirty = true;
            if self.selected_icon == Some(id) {
                self.selected_icon = None;
            }
            true
        } else {
            false
        }
    }

    /// Get icon at screen position
    pub fn icon_at(&self, x: isize, y: isize) -> Option<&DesktopIcon> {
        if x < 0 || y < 0 {
            return None;
        }
        let x = x as usize;
        let y = y as usize;

        for icon in &self.icons {
            let icon_x = icon.grid_x * self.icon_cell_width + (self.icon_cell_width - self.icon_size) / 2;
            let icon_y = icon.grid_y * self.icon_cell_height + 4;

            if x >= icon_x && x < icon_x + self.icon_size
                && y >= icon_y && y < icon_y + self.icon_size
            {
                return Some(icon);
            }

            // Check label area too
            let label_y = icon_y + self.icon_size + 4;
            let label_height = 16;
            if y >= label_y && y < label_y + label_height
                && x >= icon.grid_x * self.icon_cell_width
                && x < (icon.grid_x + 1) * self.icon_cell_width
            {
                return Some(icon);
            }
        }
        None
    }

    /// Select an icon
    pub fn select_icon(&mut self, id: Option<u64>) {
        // Deselect current
        for icon in &mut self.icons {
            icon.selected = false;
        }

        // Select new
        if let Some(id) = id {
            for icon in &mut self.icons {
                if icon.id == id {
                    icon.selected = true;
                    break;
                }
            }
        }

        self.selected_icon = id;
        self.dirty = true;
    }

    /// Get selected icon
    pub fn selected_icon(&self) -> Option<&DesktopIcon> {
        self.selected_icon.and_then(|id| {
            self.icons.iter().find(|i| i.id == id)
        })
    }

    /// Get all icons
    pub fn icons(&self) -> &[DesktopIcon] {
        &self.icons
    }

    /// Check if dirty
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Clear dirty flag
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    /// Render wallpaper to surface
    fn render_wallpaper(&mut self) {
        match &self.wallpaper {
            Wallpaper::SolidColor(color) => {
                self.wallpaper_surface.clear(*color);
            }
            Wallpaper::HorizontalGradient { start, end } => {
                for x in 0..self.screen_width {
                    let t = (x * 255 / self.screen_width.max(1)) as u8;
                    let color = Self::lerp_color(*start, *end, t);
                    for y in 0..self.screen_height {
                        self.wallpaper_surface.set_pixel(x, y, color);
                    }
                }
            }
            Wallpaper::VerticalGradient { start, end } => {
                for y in 0..self.screen_height {
                    let t = (y * 255 / self.screen_height.max(1)) as u8;
                    let color = Self::lerp_color(*start, *end, t);
                    for x in 0..self.screen_width {
                        self.wallpaper_surface.set_pixel(x, y, color);
                    }
                }
            }
            Wallpaper::RadialGradient { center, edge } => {
                let cx = self.screen_width / 2;
                let cy = self.screen_height / 2;
                let max_dist = isqrt((cx * cx + cy * cy) as u32) as usize;

                for y in 0..self.screen_height {
                    for x in 0..self.screen_width {
                        let dx = if x > cx { x - cx } else { cx - x };
                        let dy = if y > cy { y - cy } else { cy - y };
                        let dist = isqrt((dx * dx + dy * dy) as u32) as usize;
                        let t = ((dist * 255) / max_dist.max(1)).min(255) as u8;
                        let color = Self::lerp_color(*center, *edge, t);
                        self.wallpaper_surface.set_pixel(x, y, color);
                    }
                }
            }
            Wallpaper::Image(_img) => {
                // TODO: Implement image wallpaper
                self.wallpaper_surface.clear(Color::new(32, 32, 64));
            }
        }
    }

    /// Linear interpolation between two colors
    fn lerp_color(start: Color, end: Color, t: u8) -> Color {
        let t = t as u32;
        let inv_t = 255 - t;

        Color::new(
            ((start.r as u32 * inv_t + end.r as u32 * t) / 255) as u8,
            ((start.g as u32 * inv_t + end.g as u32 * t) / 255) as u8,
            ((start.b as u32 * inv_t + end.b as u32 * t) / 255) as u8,
        )
    }

    /// Render an icon to a surface
    pub fn render_icon(&self, icon: &DesktopIcon, target: &mut Surface) {
        let icon_x = icon.grid_x * self.icon_cell_width + (self.icon_cell_width - self.icon_size) / 2;
        let icon_y = icon.grid_y * self.icon_cell_height + 4;

        // Draw icon background (if selected)
        if icon.selected {
            let bg_x = icon.grid_x * self.icon_cell_width;
            let bg_w = self.icon_cell_width;
            let bg_h = self.icon_cell_height;
            let bg_y = icon.grid_y * self.icon_cell_height;

            for py in bg_y..bg_y + bg_h {
                for px in bg_x..bg_x + bg_w {
                    target.set_pixel_blended(px, py, Color::with_alpha(100, 150, 255, 80));
                }
            }
        }

        // Draw icon image or placeholder
        if let Some(ref _data) = icon.icon_data {
            // TODO: Draw actual icon image
            // For now, draw a placeholder
            Self::draw_placeholder_icon(target, icon_x, icon_y, self.icon_size, icon.selected);
        } else {
            Self::draw_placeholder_icon(target, icon_x, icon_y, self.icon_size, icon.selected);
        }

        // Draw icon label
        let label_y = icon_y + self.icon_size + 4;
        let label_x = icon.grid_x * self.icon_cell_width + 4;
        let _label_width = self.icon_cell_width - 8;

        // Simple label rendering (just the first few chars that fit)
        let text_color = if icon.selected { Color::WHITE } else { Color::WHITE };
        let _shadow_color = Color::new(0, 0, 0);

        // TODO: Use actual font rendering
        // For now, just draw a placeholder text area
        let label_bg = if icon.selected {
            Color::with_alpha(0, 100, 200, 180)
        } else {
            Color::with_alpha(0, 0, 0, 100)
        };

        // Draw label background
        let char_width = 8;
        let label_chars = icon.name.chars().count().min(self.icon_cell_width / char_width);
        let label_pixel_width = label_chars * char_width;
        let centered_x = icon.grid_x * self.icon_cell_width + (self.icon_cell_width - label_pixel_width) / 2;

        for py in label_y..label_y + 16 {
            for px in centered_x.saturating_sub(2)..centered_x + label_pixel_width + 2 {
                target.set_pixel_blended(px, py, label_bg);
            }
        }

        // We'll use the framebuffer font rendering for actual text in the compositor
        let _ = text_color;
    }

    /// Draw a placeholder icon
    fn draw_placeholder_icon(target: &mut Surface, x: usize, y: usize, size: usize, selected: bool) {
        // Draw folder-like shape
        let border_color = if selected { Color::WHITE } else { Color::new(200, 200, 200) };
        let fill_color = if selected {
            Color::new(120, 180, 255)
        } else {
            Color::new(255, 220, 100)
        };

        // Fill
        for py in y + 4..y + size - 2 {
            for px in x + 2..x + size - 2 {
                target.set_pixel(px, py, fill_color);
            }
        }

        // Folder tab
        for py in y..y + 4 {
            for px in x + 2..x + size / 3 {
                target.set_pixel(px, py, fill_color);
            }
        }

        // Border
        target.draw_rect(x + 2, y, size / 3 - 2, 4, border_color);
        target.draw_rect(x + 2, y + 4, size - 4, size - 6, border_color);
    }

    /// Resize desktop
    pub fn resize(&mut self, width: usize, height: usize) {
        self.screen_width = width;
        self.screen_height = height;
        self.wallpaper_surface.resize(width, height);
        self.render_wallpaper();
        self.dirty = true;
    }
}

/// Integer square root
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

/// Global desktop instance
static DESKTOP: Mutex<Option<Desktop>> = Mutex::new(None);

/// Initialize the desktop
pub fn init(screen_width: usize, screen_height: usize) {
    let desktop = Desktop::new(screen_width, screen_height);
    *DESKTOP.lock() = Some(desktop);
    crate::kprintln!("desktop: initialized {}x{}", screen_width, screen_height);
}

/// Set wallpaper
pub fn set_wallpaper(wallpaper: Wallpaper) {
    let mut desktop = DESKTOP.lock();
    if let Some(ref mut d) = *desktop {
        d.set_wallpaper(wallpaper);
    }
}

/// Add a desktop icon
pub fn add_icon(icon: DesktopIcon) {
    let mut desktop = DESKTOP.lock();
    if let Some(ref mut d) = *desktop {
        d.add_icon(icon);
    }
}

/// Remove a desktop icon
pub fn remove_icon(id: u64) -> bool {
    let mut desktop = DESKTOP.lock();
    desktop.as_mut().map(|d| d.remove_icon(id)).unwrap_or(false)
}

/// Get icon at position
pub fn icon_at(x: isize, y: isize) -> Option<u64> {
    let desktop = DESKTOP.lock();
    desktop.as_ref().and_then(|d| d.icon_at(x, y).map(|i| i.id))
}

/// Select an icon
pub fn select_icon(id: Option<u64>) {
    let mut desktop = DESKTOP.lock();
    if let Some(ref mut d) = *desktop {
        d.select_icon(id);
    }
}

/// Execute with desktop access
pub fn with_desktop<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut Desktop) -> R,
{
    let mut desktop = DESKTOP.lock();
    desktop.as_mut().map(f)
}

/// Check if desktop is available
pub fn is_available() -> bool {
    DESKTOP.lock().is_some()
}

/// Render the desktop (wallpaper + icons) to a target surface
pub fn render_to(target: &mut super::surface::Surface) {
    let desktop = DESKTOP.lock();
    if let Some(ref d) = *desktop {
        // Copy wallpaper
        let wp = d.wallpaper_surface();
        for y in 0..wp.height().min(target.height()) {
            for x in 0..wp.width().min(target.width()) {
                if let Some(color) = wp.get_pixel(x, y) {
                    target.set_pixel(x, y, color);
                }
            }
        }
        // Render icons on top
        for icon in d.icons() {
            d.render_icon(icon, target);
        }
    }
}
