//! Compositor module
//!
//! The compositor manages all windows and combines them into a single image
//! that is displayed on the framebuffer. It handles z-ordering, window focus,
//! dirty region tracking, and efficient partial updates.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use spin::Mutex;

use crate::drivers::framebuffer::{self, Color};
use super::surface::{Surface, PixelFormat, DirtyRect};
use super::window::{Window, WindowId};

/// Global compositor instance
static COMPOSITOR: Mutex<Option<Compositor>> = Mutex::new(None);

/// The compositor state
pub struct Compositor {
    /// All managed windows (by ID)
    windows: BTreeMap<WindowId, Window>,
    /// Window order (front to back, first = topmost)
    window_order: Vec<WindowId>,
    /// Currently focused window
    focused_window: Option<WindowId>,
    /// Back buffer for double buffering
    back_buffer: Surface,
    /// Screen width
    screen_width: usize,
    /// Screen height
    screen_height: usize,
    /// Desktop/background color
    background_color: Color,
    /// Whether the compositor needs a full redraw
    needs_full_redraw: bool,
    /// Dirty regions that need updating
    dirty_regions: Vec<DirtyRect>,
    /// Whether the compositor is enabled
    enabled: bool,
}

impl Compositor {
    /// Create a new compositor for the given screen dimensions
    pub fn new(screen_width: usize, screen_height: usize) -> Self {
        let back_buffer = Surface::new_with_color(
            screen_width,
            screen_height,
            PixelFormat::Rgba8888,
            Color::new(32, 32, 64), // Default dark blue background
        );

        Self {
            windows: BTreeMap::new(),
            window_order: Vec::new(),
            focused_window: None,
            back_buffer,
            screen_width,
            screen_height,
            background_color: Color::new(32, 32, 64),
            needs_full_redraw: true,
            dirty_regions: Vec::new(),
            enabled: true,
        }
    }

    /// Set the background color
    pub fn set_background_color(&mut self, color: Color) {
        self.background_color = color;
        self.needs_full_redraw = true;
    }

    /// Get screen dimensions
    pub fn screen_size(&self) -> (usize, usize) {
        (self.screen_width, self.screen_height)
    }

    /// Create a new window and add it to the compositor
    pub fn create_window(&mut self, title: &str, x: isize, y: isize, width: usize, height: usize) -> WindowId {
        let mut window = Window::new(title, x, y, width, height);
        let id = window.id();

        // Set initial z-order
        let z = self.window_order.len() as u32;
        window.set_z_order(z);

        // Add to collections
        self.windows.insert(id, window);
        self.window_order.insert(0, id); // Add to front

        // Focus the new window
        self.focus_window(id);

        // Mark dirty
        self.needs_full_redraw = true;

        id
    }

    /// Create an undecorated window
    pub fn create_window_undecorated(&mut self, x: isize, y: isize, width: usize, height: usize) -> WindowId {
        let mut window = Window::new_undecorated(x, y, width, height);
        let id = window.id();

        let z = self.window_order.len() as u32;
        window.set_z_order(z);

        self.windows.insert(id, window);
        self.window_order.insert(0, id);

        self.needs_full_redraw = true;

        id
    }

    /// Remove a window
    pub fn destroy_window(&mut self, id: WindowId) -> bool {
        if self.windows.remove(&id).is_some() {
            self.window_order.retain(|&wid| wid != id);

            if self.focused_window == Some(id) {
                self.focused_window = self.window_order.first().copied();
                if let Some(new_focus) = self.focused_window {
                    if let Some(window) = self.windows.get_mut(&new_focus) {
                        window.set_focused(true);
                    }
                }
            }

            self.needs_full_redraw = true;
            true
        } else {
            false
        }
    }

    /// Get a window by ID
    pub fn get_window(&self, id: WindowId) -> Option<&Window> {
        self.windows.get(&id)
    }

    /// Get a mutable window by ID
    pub fn get_window_mut(&mut self, id: WindowId) -> Option<&mut Window> {
        self.windows.get_mut(&id)
    }

    /// Focus a window (bring to front)
    pub fn focus_window(&mut self, id: WindowId) -> bool {
        if !self.windows.contains_key(&id) {
            return false;
        }

        // Unfocus previous window
        if let Some(prev_id) = self.focused_window {
            if let Some(prev) = self.windows.get_mut(&prev_id) {
                prev.set_focused(false);
            }
        }

        // Focus new window
        if let Some(window) = self.windows.get_mut(&id) {
            window.set_focused(true);
            window.flags_mut().minimized = false;
        }
        self.focused_window = Some(id);

        // Bring to front
        self.window_order.retain(|&wid| wid != id);
        self.window_order.insert(0, id);

        // Update z-orders
        for (i, &wid) in self.window_order.iter().enumerate() {
            if let Some(window) = self.windows.get_mut(&wid) {
                window.set_z_order((self.window_order.len() - i) as u32);
            }
        }

        self.needs_full_redraw = true;
        true
    }

    /// Get the currently focused window
    pub fn focused_window(&self) -> Option<WindowId> {
        self.focused_window
    }

    /// Find the window at a given screen position
    pub fn window_at(&self, x: isize, y: isize) -> Option<WindowId> {
        // Check front to back
        for &id in &self.window_order {
            if let Some(window) = self.windows.get(&id) {
                if window.is_visible() && window.contains_point(x, y) {
                    return Some(id);
                }
            }
        }
        None
    }

    /// Get all window IDs in z-order (front to back)
    pub fn window_order(&self) -> &[WindowId] {
        &self.window_order
    }

    /// Mark a region as dirty
    pub fn mark_dirty(&mut self, x: usize, y: usize, width: usize, height: usize) {
        self.dirty_regions.push(DirtyRect::new(x, y, width, height));
    }

    /// Request a full redraw
    pub fn request_full_redraw(&mut self) {
        self.needs_full_redraw = true;
    }

    /// Compose all windows to the back buffer
    pub fn compose(&mut self) {
        if !self.enabled {
            return;
        }

        // Check if any window needs redrawing
        let any_dirty = self.needs_full_redraw
            || !self.dirty_regions.is_empty()
            || self.windows.values().any(|w| w.needs_redraw());

        if !any_dirty {
            return;
        }

        // For now, always do full redraw (optimization: use dirty regions)
        // Clear background
        self.back_buffer.clear(self.background_color);

        // Draw windows back to front
        for &id in self.window_order.iter().rev() {
            if let Some(window) = self.windows.get(&id) {
                if window.is_visible() {
                    window.render(&mut self.back_buffer, None);
                }
            }
        }

        // Clear dirty flags
        for window in self.windows.values_mut() {
            window.clear_dirty();
        }
        self.needs_full_redraw = false;
        self.dirty_regions.clear();
    }

    /// Present the back buffer to the screen
    pub fn present(&self) {
        if !self.enabled {
            return;
        }

        framebuffer::with_framebuffer(|fb| {
            // Copy back buffer to framebuffer
            for y in 0..self.screen_height.min(fb.height()) {
                for x in 0..self.screen_width.min(fb.width()) {
                    if let Some(color) = self.back_buffer.get_pixel(x, y) {
                        fb.set_pixel(x, y, color);
                    }
                }
            }
        });
    }

    /// Compose and present in one call
    pub fn update(&mut self) {
        self.compose();
        self.present();
    }

    /// Enable/disable the compositor
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if enabled {
            self.needs_full_redraw = true;
        }
    }

    /// Check if compositor is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get number of windows
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    /// Iterate over all windows
    pub fn windows(&self) -> impl Iterator<Item = &Window> {
        self.windows.values()
    }

    /// Move a window
    pub fn move_window(&mut self, id: WindowId, x: isize, y: isize) {
        if let Some(window) = self.windows.get_mut(&id) {
            window.set_position(x, y);
            self.needs_full_redraw = true;
        }
    }

    /// Resize a window
    pub fn resize_window(&mut self, id: WindowId, width: usize, height: usize) {
        if let Some(window) = self.windows.get_mut(&id) {
            window.set_size(width, height);
            self.needs_full_redraw = true;
        }
    }

    /// Minimize a window
    pub fn minimize_window(&mut self, id: WindowId) {
        if let Some(window) = self.windows.get_mut(&id) {
            window.minimize();
            self.needs_full_redraw = true;

            // Focus next visible window
            if self.focused_window == Some(id) {
                for &other_id in &self.window_order {
                    if other_id != id {
                        if let Some(other) = self.windows.get(&other_id) {
                            if other.is_visible() {
                                self.focus_window(other_id);
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Maximize a window
    pub fn maximize_window(&mut self, id: WindowId) {
        if let Some(window) = self.windows.get_mut(&id) {
            window.maximize(self.screen_width, self.screen_height);
            self.needs_full_redraw = true;
        }
    }

    /// Restore a window
    pub fn restore_window(&mut self, id: WindowId) {
        if let Some(window) = self.windows.get_mut(&id) {
            window.restore();
            self.needs_full_redraw = true;
        }
    }

    /// Close a window (alias for destroy)
    pub fn close_window(&mut self, id: WindowId) -> bool {
        self.destroy_window(id)
    }
}

// ============================================================================
// Global API
// ============================================================================

/// Initialize the compositor
pub fn init() {
    if let Some((width, height)) = framebuffer::dimensions() {
        let compositor = Compositor::new(width, height);
        *COMPOSITOR.lock() = Some(compositor);
        crate::kprintln!("compositor: initialized {}x{}", width, height);
    } else {
        crate::kprintln!("compositor: no framebuffer available, skipping init");
    }
}

/// Create a new window
pub fn create_window(title: &str, x: isize, y: isize, width: usize, height: usize) -> Option<WindowId> {
    let mut comp = COMPOSITOR.lock();
    comp.as_mut().map(|c| c.create_window(title, x, y, width, height))
}

/// Create an undecorated window
pub fn create_window_undecorated(x: isize, y: isize, width: usize, height: usize) -> Option<WindowId> {
    let mut comp = COMPOSITOR.lock();
    comp.as_mut().map(|c| c.create_window_undecorated(x, y, width, height))
}

/// Destroy a window
pub fn destroy_window(id: WindowId) -> bool {
    let mut comp = COMPOSITOR.lock();
    comp.as_mut().map(|c| c.destroy_window(id)).unwrap_or(false)
}

/// Focus a window
pub fn focus_window(id: WindowId) -> bool {
    let mut comp = COMPOSITOR.lock();
    comp.as_mut().map(|c| c.focus_window(id)).unwrap_or(false)
}

/// Get the focused window
pub fn focused_window() -> Option<WindowId> {
    let comp = COMPOSITOR.lock();
    comp.as_ref().and_then(|c| c.focused_window())
}

/// Find window at screen position
pub fn window_at(x: isize, y: isize) -> Option<WindowId> {
    let comp = COMPOSITOR.lock();
    comp.as_ref().and_then(|c| c.window_at(x, y))
}

/// Update the compositor (compose + present)
pub fn update() {
    let mut comp = COMPOSITOR.lock();
    if let Some(ref mut c) = *comp {
        c.update();
    }
}

/// Compose windows (without presenting)
pub fn compose() {
    let mut comp = COMPOSITOR.lock();
    if let Some(ref mut c) = *comp {
        c.compose();
    }
}

/// Present to screen (without composing)
pub fn present() {
    let comp = COMPOSITOR.lock();
    if let Some(ref c) = *comp {
        c.present();
    }
}

/// Execute a function with access to the compositor
pub fn with_compositor<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut Compositor) -> R,
{
    let mut comp = COMPOSITOR.lock();
    comp.as_mut().map(f)
}

/// Execute a function with access to a window
pub fn with_window<F, R>(id: WindowId, f: F) -> Option<R>
where
    F: FnOnce(&mut Window) -> R,
{
    let mut comp = COMPOSITOR.lock();
    comp.as_mut().and_then(|c| c.get_window_mut(id).map(f))
}

/// Move a window
pub fn move_window(id: WindowId, x: isize, y: isize) {
    let mut comp = COMPOSITOR.lock();
    if let Some(ref mut c) = *comp {
        c.move_window(id, x, y);
    }
}

/// Resize a window
pub fn resize_window(id: WindowId, width: usize, height: usize) {
    let mut comp = COMPOSITOR.lock();
    if let Some(ref mut c) = *comp {
        c.resize_window(id, width, height);
    }
}

/// Minimize a window
pub fn minimize_window(id: WindowId) {
    let mut comp = COMPOSITOR.lock();
    if let Some(ref mut c) = *comp {
        c.minimize_window(id);
    }
}

/// Maximize a window
pub fn maximize_window(id: WindowId) {
    let mut comp = COMPOSITOR.lock();
    if let Some(ref mut c) = *comp {
        c.maximize_window(id);
    }
}

/// Close a window
pub fn close_window(id: WindowId) -> bool {
    let mut comp = COMPOSITOR.lock();
    comp.as_mut().map(|c| c.close_window(id)).unwrap_or(false)
}

/// Set background color
pub fn set_background_color(color: Color) {
    let mut comp = COMPOSITOR.lock();
    if let Some(ref mut c) = *comp {
        c.set_background_color(color);
    }
}

/// Get screen size
pub fn screen_size() -> Option<(usize, usize)> {
    let comp = COMPOSITOR.lock();
    comp.as_ref().map(|c| c.screen_size())
}

/// Request a full redraw
pub fn request_redraw() {
    let mut comp = COMPOSITOR.lock();
    if let Some(ref mut c) = *comp {
        c.request_full_redraw();
    }
}

/// Enable/disable the compositor
pub fn set_enabled(enabled: bool) {
    let mut comp = COMPOSITOR.lock();
    if let Some(ref mut c) = *comp {
        c.set_enabled(enabled);
    }
}

/// Check if compositor is available
pub fn is_available() -> bool {
    COMPOSITOR.lock().is_some()
}
