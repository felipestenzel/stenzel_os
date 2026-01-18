//! Window module
//!
//! Windows are the user-visible containers that hold application content.
//! Each window has a surface for content and optional decorations (title bar, borders).

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;
use crate::drivers::framebuffer::Color;
use super::surface::{Surface, SurfaceId, PixelFormat};
use super::transparency::{WindowTransparency, Opacity, BlendMode};

/// A unique window identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WindowId(pub u64);

static NEXT_WINDOW_ID: Mutex<u64> = Mutex::new(1);

impl WindowId {
    /// Generate a new unique window ID
    pub fn new() -> Self {
        let mut next = NEXT_WINDOW_ID.lock();
        let id = *next;
        *next += 1;
        WindowId(id)
    }
}

impl Default for WindowId {
    fn default() -> Self {
        Self::new()
    }
}

/// Window state flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowFlags {
    /// Window is visible
    pub visible: bool,
    /// Window is minimized
    pub minimized: bool,
    /// Window is maximized
    pub maximized: bool,
    /// Window is focused (receives input)
    pub focused: bool,
    /// Window can be resized
    pub resizable: bool,
    /// Window has decorations (title bar, borders)
    pub decorated: bool,
    /// Window is always on top
    pub always_on_top: bool,
    /// Window is modal (blocks input to other windows)
    pub modal: bool,
}

impl Default for WindowFlags {
    fn default() -> Self {
        Self {
            visible: true,
            minimized: false,
            maximized: false,
            focused: false,
            resizable: true,
            decorated: true,
            always_on_top: false,
            modal: false,
        }
    }
}

/// Window decoration style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowStyle {
    /// Title bar height
    pub title_bar_height: usize,
    /// Border width
    pub border_width: usize,
    /// Title bar color
    pub title_bar_color: Color,
    /// Title bar color when focused
    pub title_bar_color_focused: Color,
    /// Border color
    pub border_color: Color,
    /// Title text color
    pub title_color: Color,
    /// Button size (close, minimize, maximize)
    pub button_size: usize,
}

impl Default for WindowStyle {
    fn default() -> Self {
        Self {
            title_bar_height: 24,
            border_width: 1,
            title_bar_color: Color::new(64, 64, 64),
            title_bar_color_focused: Color::new(0, 102, 204),
            border_color: Color::new(128, 128, 128),
            title_color: Color::WHITE,
            button_size: 16,
        }
    }
}

/// A window in the GUI system
pub struct Window {
    /// Unique identifier
    id: WindowId,
    /// Window title
    title: String,
    /// X position (screen coordinates)
    x: isize,
    /// Y position (screen coordinates)
    y: isize,
    /// Window width (including decorations)
    width: usize,
    /// Window height (including decorations)
    height: usize,
    /// Content surface
    content: Surface,
    /// Window flags
    flags: WindowFlags,
    /// Window style
    style: WindowStyle,
    /// Z-order (higher = on top)
    z_order: u32,
    /// Minimum width
    min_width: usize,
    /// Minimum height
    min_height: usize,
    /// Maximum width (0 = no limit)
    max_width: usize,
    /// Maximum height (0 = no limit)
    max_height: usize,
    /// Process ID that owns this window
    owner_pid: Option<u64>,
    /// Whether the window needs to be recomposited
    needs_redraw: bool,
    /// Saved position before maximize
    saved_rect: Option<(isize, isize, usize, usize)>,
    /// Transparency settings
    transparency: WindowTransparency,
}

impl Window {
    /// Create a new window
    pub fn new(title: &str, x: isize, y: isize, width: usize, height: usize) -> Self {
        let style = WindowStyle::default();
        let content_width = width.saturating_sub(style.border_width * 2);
        let content_height = height.saturating_sub(style.title_bar_height + style.border_width * 2);

        Self {
            id: WindowId::new(),
            title: String::from(title),
            x,
            y,
            width,
            height,
            content: Surface::new(content_width, content_height, PixelFormat::Rgba8888),
            flags: WindowFlags::default(),
            style,
            z_order: 0,
            min_width: 100,
            min_height: 50,
            max_width: 0,
            max_height: 0,
            owner_pid: None,
            needs_redraw: true,
            saved_rect: None,
            transparency: WindowTransparency::default(),
        }
    }

    /// Create a new undecorated window
    pub fn new_undecorated(x: isize, y: isize, width: usize, height: usize) -> Self {
        let mut window = Self::new("", x, y, width, height);
        window.flags.decorated = false;
        window.style.title_bar_height = 0;
        window.style.border_width = 0;
        window.content.resize(width, height);
        window
    }

    /// Get window ID
    pub fn id(&self) -> WindowId {
        self.id
    }

    /// Get title
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Set title
    pub fn set_title(&mut self, title: &str) {
        self.title = String::from(title);
        self.needs_redraw = true;
    }

    /// Get position
    pub fn position(&self) -> (isize, isize) {
        (self.x, self.y)
    }

    /// Set position
    pub fn set_position(&mut self, x: isize, y: isize) {
        self.x = x;
        self.y = y;
    }

    /// Get size
    pub fn size(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    /// Set size
    pub fn set_size(&mut self, width: usize, height: usize) {
        let width = width.max(self.min_width);
        let height = height.max(self.min_height);
        let width = if self.max_width > 0 { width.min(self.max_width) } else { width };
        let height = if self.max_height > 0 { height.min(self.max_height) } else { height };

        self.width = width;
        self.height = height;

        // Resize content surface
        let content_width = if self.flags.decorated {
            width.saturating_sub(self.style.border_width * 2)
        } else {
            width
        };
        let content_height = if self.flags.decorated {
            height.saturating_sub(self.style.title_bar_height + self.style.border_width * 2)
        } else {
            height
        };
        self.content.resize(content_width, content_height);
        self.needs_redraw = true;
    }

    /// Get bounds (x, y, width, height)
    pub fn bounds(&self) -> (isize, isize, usize, usize) {
        (self.x, self.y, self.width, self.height)
    }

    /// Get content area (relative to window)
    pub fn content_rect(&self) -> (usize, usize, usize, usize) {
        if self.flags.decorated {
            (
                self.style.border_width,
                self.style.title_bar_height,
                self.content.width(),
                self.content.height(),
            )
        } else {
            (0, 0, self.content.width(), self.content.height())
        }
    }

    /// Get content surface
    pub fn content(&self) -> &Surface {
        &self.content
    }

    /// Get mutable content surface
    pub fn content_mut(&mut self) -> &mut Surface {
        self.needs_redraw = true;
        &mut self.content
    }

    /// Get flags
    pub fn flags(&self) -> &WindowFlags {
        &self.flags
    }

    /// Get mutable flags
    pub fn flags_mut(&mut self) -> &mut WindowFlags {
        &mut self.flags
    }

    /// Get style
    pub fn style(&self) -> &WindowStyle {
        &self.style
    }

    /// Set style
    pub fn set_style(&mut self, style: WindowStyle) {
        self.style = style;
        self.needs_redraw = true;
    }

    /// Get z-order
    pub fn z_order(&self) -> u32 {
        self.z_order
    }

    /// Set z-order
    pub fn set_z_order(&mut self, z: u32) {
        self.z_order = z;
    }

    /// Check if window is visible
    pub fn is_visible(&self) -> bool {
        self.flags.visible && !self.flags.minimized
    }

    /// Show the window
    pub fn show(&mut self) {
        self.flags.visible = true;
        self.flags.minimized = false;
        self.needs_redraw = true;
    }

    /// Hide the window
    pub fn hide(&mut self) {
        self.flags.visible = false;
    }

    /// Minimize the window
    pub fn minimize(&mut self) {
        self.flags.minimized = true;
    }

    /// Restore the window from minimized state
    pub fn restore(&mut self) {
        if self.flags.maximized {
            self.unmaximize();
        } else {
            self.flags.minimized = false;
            self.needs_redraw = true;
        }
    }

    /// Maximize the window
    pub fn maximize(&mut self, screen_width: usize, screen_height: usize) {
        if !self.flags.maximized {
            // Save current position
            self.saved_rect = Some((self.x, self.y, self.width, self.height));
            self.flags.maximized = true;
            self.x = 0;
            self.y = 0;
            self.set_size(screen_width, screen_height);
        }
    }

    /// Restore from maximized state
    pub fn unmaximize(&mut self) {
        if self.flags.maximized {
            self.flags.maximized = false;
            if let Some((x, y, w, h)) = self.saved_rect.take() {
                self.x = x;
                self.y = y;
                self.set_size(w, h);
            }
            self.needs_redraw = true;
        }
    }

    /// Set focus
    pub fn set_focused(&mut self, focused: bool) {
        if self.flags.focused != focused {
            self.flags.focused = focused;
            self.needs_redraw = true;
        }
    }

    /// Check if a point is inside the window
    pub fn contains_point(&self, px: isize, py: isize) -> bool {
        px >= self.x
            && px < self.x + self.width as isize
            && py >= self.y
            && py < self.y + self.height as isize
    }

    /// Check if a point is inside the title bar
    pub fn is_in_title_bar(&self, px: isize, py: isize) -> bool {
        if !self.flags.decorated {
            return false;
        }
        px >= self.x
            && px < self.x + self.width as isize
            && py >= self.y
            && py < self.y + self.style.title_bar_height as isize
    }

    /// Check if a point is on the close button
    pub fn is_on_close_button(&self, px: isize, py: isize) -> bool {
        if !self.flags.decorated {
            return false;
        }
        let btn_x = self.x + self.width as isize - self.style.button_size as isize - 4;
        let btn_y = self.y + 4;
        let btn_size = self.style.button_size as isize;
        px >= btn_x && px < btn_x + btn_size && py >= btn_y && py < btn_y + btn_size
    }

    /// Check if a point is on a resize edge
    /// Returns (horizontal: -1/0/1, vertical: -1/0/1)
    pub fn get_resize_edge(&self, px: isize, py: isize) -> Option<(i8, i8)> {
        if !self.flags.resizable {
            return None;
        }

        let edge_size = 8isize;
        let mut h: i8 = 0;
        let mut v: i8 = 0;

        // Check horizontal edges
        if px < self.x + edge_size {
            h = -1;
        } else if px >= self.x + self.width as isize - edge_size {
            h = 1;
        }

        // Check vertical edges
        if py < self.y + edge_size {
            v = -1;
        } else if py >= self.y + self.height as isize - edge_size {
            v = 1;
        }

        if h != 0 || v != 0 {
            Some((h, v))
        } else {
            None
        }
    }

    /// Check if window needs redraw
    pub fn needs_redraw(&self) -> bool {
        self.needs_redraw || self.content.is_dirty()
    }

    /// Mark as needing redraw
    pub fn mark_dirty(&mut self) {
        self.needs_redraw = true;
    }

    /// Clear redraw flag
    pub fn clear_dirty(&mut self) {
        self.needs_redraw = false;
        self.content.clear_dirty();
    }

    /// Set owner process ID
    pub fn set_owner(&mut self, pid: u64) {
        self.owner_pid = Some(pid);
    }

    /// Get owner process ID
    pub fn owner(&self) -> Option<u64> {
        self.owner_pid
    }

    /// Set size limits
    pub fn set_size_limits(
        &mut self,
        min_width: usize,
        min_height: usize,
        max_width: usize,
        max_height: usize,
    ) {
        self.min_width = min_width;
        self.min_height = min_height;
        self.max_width = max_width;
        self.max_height = max_height;
    }

    // ========================================================================
    // Transparency Methods
    // ========================================================================

    /// Get window transparency settings
    pub fn transparency(&self) -> &WindowTransparency {
        &self.transparency
    }

    /// Get mutable transparency settings
    pub fn transparency_mut(&mut self) -> &mut WindowTransparency {
        self.needs_redraw = true;
        &mut self.transparency
    }

    /// Set window opacity (0-255)
    pub fn set_opacity(&mut self, opacity: u8) {
        self.transparency.opacity = Opacity(opacity);
        self.needs_redraw = true;
    }

    /// Set window opacity from percentage (0-100)
    pub fn set_opacity_percent(&mut self, percent: u8) {
        self.transparency.opacity = Opacity::from_percent(percent);
        self.needs_redraw = true;
    }

    /// Get window opacity (0-255)
    pub fn opacity(&self) -> u8 {
        self.transparency.opacity.0
    }

    /// Get window opacity as percentage (0-100)
    pub fn opacity_percent(&self) -> u8 {
        self.transparency.opacity.as_percent()
    }

    /// Enable drop shadow
    pub fn enable_shadow(&mut self, enabled: bool) {
        self.transparency.shadow_enabled = enabled;
        self.needs_redraw = true;
    }

    /// Check if shadow is enabled
    pub fn has_shadow(&self) -> bool {
        self.transparency.shadow_enabled
    }

    /// Enable glass effect
    pub fn enable_glass(&mut self, enabled: bool) {
        self.transparency.glass_enabled = enabled;
        self.needs_redraw = true;
    }

    /// Check if glass effect is enabled
    pub fn has_glass(&self) -> bool {
        self.transparency.glass_enabled
    }

    /// Set blend mode
    pub fn set_blend_mode(&mut self, mode: BlendMode) {
        self.transparency.blend_mode = mode;
        self.needs_redraw = true;
    }

    /// Get blend mode
    pub fn blend_mode(&self) -> BlendMode {
        self.transparency.blend_mode
    }

    /// Check if window is fully opaque
    pub fn is_opaque(&self) -> bool {
        self.transparency.is_opaque()
    }

    /// Check if click should pass through at given point's alpha
    pub fn should_click_through(&self, alpha: u8) -> bool {
        self.transparency.should_click_through(alpha)
    }

    /// Render the window to a surface (decorations + content)
    pub fn render(&self, target: &mut Surface, clip: Option<(isize, isize, usize, usize)>) {
        if !self.is_visible() {
            return;
        }

        // Calculate visible region
        let (clip_x, clip_y, clip_w, clip_h) = clip.unwrap_or((
            0,
            0,
            target.width(),
            target.height(),
        ));

        if self.flags.decorated {
            // Draw title bar
            let title_color = if self.flags.focused {
                self.style.title_bar_color_focused
            } else {
                self.style.title_bar_color
            };

            // Title bar background
            for py in 0..self.style.title_bar_height {
                for px in 0..self.width {
                    let sx = self.x + px as isize;
                    let sy = self.y + py as isize;
                    if sx >= clip_x && sx < clip_x + clip_w as isize
                        && sy >= clip_y && sy < clip_y + clip_h as isize
                    {
                        if sx >= 0 && sy >= 0 {
                            target.set_pixel(sx as usize, sy as usize, title_color);
                        }
                    }
                }
            }

            // Draw border
            let border_color = self.style.border_color;
            // Top border (already covered by title bar)
            // Left border
            for py in self.style.title_bar_height..self.height {
                for bx in 0..self.style.border_width {
                    let sx = self.x + bx as isize;
                    let sy = self.y + py as isize;
                    if sx >= 0 && sy >= 0 {
                        target.set_pixel(sx as usize, sy as usize, border_color);
                    }
                }
            }
            // Right border
            for py in self.style.title_bar_height..self.height {
                for bx in 0..self.style.border_width {
                    let sx = self.x + (self.width - self.style.border_width + bx) as isize;
                    let sy = self.y + py as isize;
                    if sx >= 0 && sy >= 0 {
                        target.set_pixel(sx as usize, sy as usize, border_color);
                    }
                }
            }
            // Bottom border
            for py in 0..self.style.border_width {
                for px in 0..self.width {
                    let sx = self.x + px as isize;
                    let sy = self.y + (self.height - self.style.border_width + py) as isize;
                    if sx >= 0 && sy >= 0 {
                        target.set_pixel(sx as usize, sy as usize, border_color);
                    }
                }
            }

            // Draw close button (red square)
            let btn_x = self.width - self.style.button_size - 4;
            let btn_y = 4usize;
            for py in 0..self.style.button_size {
                for px in 0..self.style.button_size {
                    let sx = self.x + (btn_x + px) as isize;
                    let sy = self.y + (btn_y + py) as isize;
                    if sx >= 0 && sy >= 0 {
                        // Draw X pattern
                        let is_x = (px == py) || (px + py == self.style.button_size - 1);
                        let color = if is_x {
                            Color::WHITE
                        } else {
                            Color::new(200, 50, 50)
                        };
                        target.set_pixel(sx as usize, sy as usize, color);
                    }
                }
            }
        }

        // Draw content
        let (content_x, content_y, _, _) = self.content_rect();
        let dst_x = self.x + content_x as isize;
        let dst_y = self.y + content_y as isize;
        target.blit(&self.content, dst_x, dst_y);
    }

    // ======================== Tiling methods ========================

    /// Tile window to the left half of the screen
    pub fn tile_left(&mut self, screen_width: usize, screen_height: usize, taskbar_height: usize) {
        let available_height = screen_height.saturating_sub(taskbar_height);
        self.saved_rect = Some((self.x, self.y, self.width, self.height));
        self.x = 0;
        self.y = 0;
        self.set_size(screen_width / 2, available_height);
        self.flags.maximized = false;
    }

    /// Tile window to the right half of the screen
    pub fn tile_right(&mut self, screen_width: usize, screen_height: usize, taskbar_height: usize) {
        let available_height = screen_height.saturating_sub(taskbar_height);
        self.saved_rect = Some((self.x, self.y, self.width, self.height));
        self.x = (screen_width / 2) as isize;
        self.y = 0;
        self.set_size(screen_width / 2, available_height);
        self.flags.maximized = false;
    }

    /// Tile window to the top half of the screen
    pub fn tile_top(&mut self, screen_width: usize, screen_height: usize, taskbar_height: usize) {
        let available_height = screen_height.saturating_sub(taskbar_height);
        self.saved_rect = Some((self.x, self.y, self.width, self.height));
        self.x = 0;
        self.y = 0;
        self.set_size(screen_width, available_height / 2);
        self.flags.maximized = false;
    }

    /// Tile window to the bottom half of the screen
    pub fn tile_bottom(&mut self, screen_width: usize, screen_height: usize, taskbar_height: usize) {
        let available_height = screen_height.saturating_sub(taskbar_height);
        self.saved_rect = Some((self.x, self.y, self.width, self.height));
        self.x = 0;
        self.y = (available_height / 2) as isize;
        self.set_size(screen_width, available_height / 2);
        self.flags.maximized = false;
    }

    /// Tile window to top-left quarter
    pub fn tile_top_left(&mut self, screen_width: usize, screen_height: usize, taskbar_height: usize) {
        let available_height = screen_height.saturating_sub(taskbar_height);
        self.saved_rect = Some((self.x, self.y, self.width, self.height));
        self.x = 0;
        self.y = 0;
        self.set_size(screen_width / 2, available_height / 2);
        self.flags.maximized = false;
    }

    /// Tile window to top-right quarter
    pub fn tile_top_right(&mut self, screen_width: usize, screen_height: usize, taskbar_height: usize) {
        let available_height = screen_height.saturating_sub(taskbar_height);
        self.saved_rect = Some((self.x, self.y, self.width, self.height));
        self.x = (screen_width / 2) as isize;
        self.y = 0;
        self.set_size(screen_width / 2, available_height / 2);
        self.flags.maximized = false;
    }

    /// Tile window to bottom-left quarter
    pub fn tile_bottom_left(&mut self, screen_width: usize, screen_height: usize, taskbar_height: usize) {
        let available_height = screen_height.saturating_sub(taskbar_height);
        self.saved_rect = Some((self.x, self.y, self.width, self.height));
        self.x = 0;
        self.y = (available_height / 2) as isize;
        self.set_size(screen_width / 2, available_height / 2);
        self.flags.maximized = false;
    }

    /// Tile window to bottom-right quarter
    pub fn tile_bottom_right(&mut self, screen_width: usize, screen_height: usize, taskbar_height: usize) {
        let available_height = screen_height.saturating_sub(taskbar_height);
        self.saved_rect = Some((self.x, self.y, self.width, self.height));
        self.x = (screen_width / 2) as isize;
        self.y = (available_height / 2) as isize;
        self.set_size(screen_width / 2, available_height / 2);
        self.flags.maximized = false;
    }

    /// Restore window from tiled state to previous position
    pub fn restore_from_tile(&mut self) {
        if let Some((x, y, w, h)) = self.saved_rect.take() {
            self.x = x;
            self.y = y;
            self.set_size(w, h);
            self.flags.maximized = false;
        }
    }

    /// Check if window is tiled (snapped to edge)
    pub fn is_tiled(&self) -> bool {
        self.saved_rect.is_some() && !self.flags.maximized
    }
}

/// Tiling position for window snapping
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TilePosition {
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

// ======================== Virtual Desktops ========================

/// Virtual desktop identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DesktopId(pub u32);

impl Default for DesktopId {
    fn default() -> Self {
        DesktopId(0)
    }
}

/// Virtual desktop (workspace)
pub struct VirtualDesktop {
    /// Desktop identifier
    pub id: DesktopId,
    /// Desktop name
    pub name: String,
    /// Windows on this desktop (by window ID)
    pub windows: Vec<WindowId>,
    /// Whether this desktop is active
    pub active: bool,
}

impl VirtualDesktop {
    /// Create a new virtual desktop
    pub fn new(id: u32, name: &str) -> Self {
        Self {
            id: DesktopId(id),
            name: String::from(name),
            windows: Vec::new(),
            active: false,
        }
    }

    /// Add a window to this desktop
    pub fn add_window(&mut self, window_id: WindowId) {
        if !self.windows.contains(&window_id) {
            self.windows.push(window_id);
        }
    }

    /// Remove a window from this desktop
    pub fn remove_window(&mut self, window_id: WindowId) -> bool {
        if let Some(pos) = self.windows.iter().position(|&id| id == window_id) {
            self.windows.remove(pos);
            true
        } else {
            false
        }
    }

    /// Check if window is on this desktop
    pub fn has_window(&self, window_id: WindowId) -> bool {
        self.windows.contains(&window_id)
    }
}

/// Virtual desktop manager
pub struct VirtualDesktopManager {
    /// All virtual desktops
    desktops: Vec<VirtualDesktop>,
    /// Currently active desktop index
    active_index: usize,
    /// Maximum number of desktops
    max_desktops: usize,
}

impl VirtualDesktopManager {
    /// Create a new virtual desktop manager with default desktops
    pub fn new(num_desktops: usize) -> Self {
        let mut desktops = Vec::with_capacity(num_desktops);
        for i in 0..num_desktops {
            let name = alloc::format!("Desktop {}", i + 1);
            let mut desktop = VirtualDesktop::new(i as u32, &name);
            if i == 0 {
                desktop.active = true;
            }
            desktops.push(desktop);
        }

        Self {
            desktops,
            active_index: 0,
            max_desktops: 16,
        }
    }

    /// Get current active desktop
    pub fn active_desktop(&self) -> &VirtualDesktop {
        &self.desktops[self.active_index]
    }

    /// Get mutable reference to active desktop
    pub fn active_desktop_mut(&mut self) -> &mut VirtualDesktop {
        &mut self.desktops[self.active_index]
    }

    /// Get active desktop index (0-based)
    pub fn active_index(&self) -> usize {
        self.active_index
    }

    /// Get total number of desktops
    pub fn desktop_count(&self) -> usize {
        self.desktops.len()
    }

    /// Switch to a specific desktop by index
    pub fn switch_to(&mut self, index: usize) -> bool {
        if index < self.desktops.len() {
            self.desktops[self.active_index].active = false;
            self.active_index = index;
            self.desktops[self.active_index].active = true;
            true
        } else {
            false
        }
    }

    /// Switch to next desktop (wraps around)
    pub fn switch_next(&mut self) {
        let next = (self.active_index + 1) % self.desktops.len();
        self.switch_to(next);
    }

    /// Switch to previous desktop (wraps around)
    pub fn switch_prev(&mut self) {
        let prev = if self.active_index == 0 {
            self.desktops.len() - 1
        } else {
            self.active_index - 1
        };
        self.switch_to(prev);
    }

    /// Add a new desktop
    pub fn add_desktop(&mut self, name: &str) -> Option<DesktopId> {
        if self.desktops.len() >= self.max_desktops {
            return None;
        }
        let id = self.desktops.len() as u32;
        self.desktops.push(VirtualDesktop::new(id, name));
        Some(DesktopId(id))
    }

    /// Remove a desktop by index (cannot remove last desktop)
    pub fn remove_desktop(&mut self, index: usize) -> bool {
        if self.desktops.len() <= 1 || index >= self.desktops.len() {
            return false;
        }

        // Move windows to previous desktop
        let windows: Vec<WindowId> = self.desktops[index].windows.clone();
        let target = if index > 0 { index - 1 } else { 0 };

        for wid in windows {
            self.desktops[target].add_window(wid);
        }

        self.desktops.remove(index);

        // Adjust active index if needed
        if self.active_index >= self.desktops.len() {
            self.active_index = self.desktops.len() - 1;
        }

        true
    }

    /// Move a window to a specific desktop
    pub fn move_window_to_desktop(&mut self, window_id: WindowId, desktop_index: usize) -> bool {
        if desktop_index >= self.desktops.len() {
            return false;
        }

        // Remove from all desktops
        for desktop in &mut self.desktops {
            desktop.remove_window(window_id);
        }

        // Add to target desktop
        self.desktops[desktop_index].add_window(window_id);
        true
    }

    /// Get desktop containing a window
    pub fn find_window_desktop(&self, window_id: WindowId) -> Option<usize> {
        self.desktops.iter().position(|d| d.has_window(window_id))
    }

    /// Check if a window should be visible (on active desktop)
    pub fn is_window_visible(&self, window_id: WindowId) -> bool {
        self.desktops[self.active_index].has_window(window_id)
    }

    /// Get all desktop names
    pub fn desktop_names(&self) -> Vec<&str> {
        self.desktops.iter().map(|d| d.name.as_str()).collect()
    }

    /// Rename a desktop
    pub fn rename_desktop(&mut self, index: usize, name: &str) -> bool {
        if index < self.desktops.len() {
            self.desktops[index].name = String::from(name);
            true
        } else {
            false
        }
    }
}
