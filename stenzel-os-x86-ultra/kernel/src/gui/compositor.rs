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

/// Flag indicating cursor needs update (for deferred rendering outside interrupt context)
static CURSOR_DIRTY: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// Cursor dimensions (width x height)
const CURSOR_WIDTH: usize = 13;
const CURSOR_HEIGHT: usize = 19;

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
    /// Cursor X position
    cursor_x: isize,
    /// Cursor Y position
    cursor_y: isize,
    /// Whether cursor is visible
    cursor_visible: bool,
    /// Saved background under cursor (for cursor overlay optimization)
    cursor_background: Vec<Color>,
    /// Last cursor X position (where background was saved)
    cursor_last_x: isize,
    /// Last cursor Y position (where background was saved)
    cursor_last_y: isize,
    /// Whether cursor background is valid
    cursor_bg_valid: bool,
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
            cursor_x: (screen_width / 2) as isize,
            cursor_y: (screen_height / 2) as isize,
            cursor_visible: true,
            cursor_background: alloc::vec![Color::BLACK; CURSOR_WIDTH * CURSOR_HEIGHT],
            cursor_last_x: (screen_width / 2) as isize,
            cursor_last_y: (screen_height / 2) as isize,
            cursor_bg_valid: false,
        }
    }

    /// Move cursor by relative amount (does NOT trigger full redraw)
    pub fn move_cursor(&mut self, dx: i32, dy: i32) {
        self.cursor_x = (self.cursor_x + dx as isize).clamp(0, self.screen_width as isize - 1);
        self.cursor_y = (self.cursor_y + dy as isize).clamp(0, self.screen_height as isize - 1);
        // Note: cursor movement no longer triggers full redraw
        // Use update_cursor_only() for efficient cursor updates
    }

    /// Set cursor position (does NOT trigger full redraw)
    pub fn set_cursor_pos(&mut self, x: isize, y: isize) {
        self.cursor_x = x.clamp(0, self.screen_width as isize - 1);
        self.cursor_y = y.clamp(0, self.screen_height as isize - 1);
        // Note: cursor movement no longer triggers full redraw
        // Use update_cursor_only() for efficient cursor updates
    }

    /// Save the background under the cursor from the back buffer
    fn save_cursor_background(&mut self) {
        let cx = self.cursor_x as usize;
        let cy = self.cursor_y as usize;

        for row in 0..CURSOR_HEIGHT {
            for col in 0..CURSOR_WIDTH {
                let px = cx + col;
                let py = cy + row;
                let idx = row * CURSOR_WIDTH + col;
                if px < self.screen_width && py < self.screen_height {
                    self.cursor_background[idx] = self.back_buffer.get_pixel(px, py)
                        .unwrap_or(self.background_color);
                } else {
                    self.cursor_background[idx] = self.background_color;
                }
            }
        }

        self.cursor_last_x = self.cursor_x;
        self.cursor_last_y = self.cursor_y;
        self.cursor_bg_valid = true;
    }

    /// Restore the saved background to the back buffer
    fn restore_cursor_background(&mut self) {
        if !self.cursor_bg_valid {
            return;
        }

        let cx = self.cursor_last_x as usize;
        let cy = self.cursor_last_y as usize;

        for row in 0..CURSOR_HEIGHT {
            for col in 0..CURSOR_WIDTH {
                let px = cx + col;
                let py = cy + row;
                let idx = row * CURSOR_WIDTH + col;
                if px < self.screen_width && py < self.screen_height {
                    self.back_buffer.set_pixel(px, py, self.cursor_background[idx]);
                }
            }
        }
    }

    /// Update cursor position efficiently (restore old bg, save new bg, draw cursor)
    /// This modifies the back buffer but does NOT present to screen
    pub fn update_cursor_in_backbuffer(&mut self) {
        if !self.cursor_visible {
            return;
        }

        // 1. Restore old background (erase cursor from old position)
        self.restore_cursor_background();

        // 2. Save new background at new cursor position
        self.save_cursor_background();

        // 3. Draw cursor at new position
        let cx = self.cursor_x as usize;
        let cy = self.cursor_y as usize;
        let sw = self.screen_width;
        let sh = self.screen_height;
        Self::draw_cursor_at(&mut self.back_buffer, cx, cy, sw, sh);
    }

    /// Present only the cursor regions to the framebuffer (fast path)
    /// This copies only the old and new cursor areas, not the entire screen
    /// old_x/old_y are the coordinates where the cursor WAS (before move)
    fn present_cursor_only_at(&self, old_x: isize, old_y: isize) {
        if !self.enabled {
            return;
        }

        framebuffer::with_framebuffer(|fb| {
            // Present old cursor region (now restored background)
            let ox = old_x.max(0) as usize;
            let oy = old_y.max(0) as usize;
            Self::present_region(&self.back_buffer, fb, ox, oy, CURSOR_WIDTH, CURSOR_HEIGHT);

            // Present new cursor region (if different from old)
            let new_x = self.cursor_x.max(0) as usize;
            let new_y = self.cursor_y.max(0) as usize;
            if new_x != ox || new_y != oy {
                Self::present_region(&self.back_buffer, fb, new_x, new_y, CURSOR_WIDTH, CURSOR_HEIGHT);
            }
        });
    }

    /// Helper: copy a region from back buffer to framebuffer
    /// Optimized to copy row by row for better cache locality
    fn present_region(
        back_buffer: &Surface,
        fb: &mut framebuffer::FrameBufferState,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
    ) {
        let fb_width = fb.width();
        let fb_height = fb.height();
        let bb_width = back_buffer.width();
        let bb_height = back_buffer.height();

        // Clamp region to valid bounds
        let actual_width = width.min(fb_width.saturating_sub(x)).min(bb_width.saturating_sub(x));
        let actual_height = height.min(fb_height.saturating_sub(y)).min(bb_height.saturating_sub(y));

        if actual_width == 0 || actual_height == 0 {
            return;
        }

        // Fast path: copy pixel by pixel but without bounds checking per pixel
        for row in 0..actual_height {
            let py = y + row;
            for col in 0..actual_width {
                let px = x + col;
                // Both are guaranteed in bounds due to clamping above
                if let Some(color) = back_buffer.get_pixel(px, py) {
                    fb.set_pixel(px, py, color);
                }
            }
        }
    }

    /// Efficient cursor-only update: update back buffer and present cursor regions only
    pub fn update_cursor_only(&mut self) {
        if !self.enabled || !self.cursor_visible {
            return;
        }

        // CRITICAL: Save old position BEFORE update_cursor_in_backbuffer() changes cursor_last_x/y
        let old_x = self.cursor_last_x;
        let old_y = self.cursor_last_y;

        // This updates cursor_last_x/y to the new position
        self.update_cursor_in_backbuffer();

        // Present using the saved old position
        self.present_cursor_only_at(old_x, old_y);
    }

    /// Get cursor position
    pub fn cursor_pos(&self) -> (isize, isize) {
        (self.cursor_x, self.cursor_y)
    }

    /// Draw a simple arrow cursor at the given position (static helper)
    fn draw_cursor_at(surface: &mut Surface, x: usize, y: usize, screen_width: usize, screen_height: usize) {
        // Simple arrow cursor (12x19 pixels)
        const CURSOR: &[&[u8]] = &[
            b"X............",
            b"XX...........",
            b"XWX..........",
            b"XWWX.........",
            b"XWWWX........",
            b"XWWWWX.......",
            b"XWWWWWX......",
            b"XWWWWWWX.....",
            b"XWWWWWWWX....",
            b"XWWWWWWWWX...",
            b"XWWWWWWWWWX..",
            b"XWWWWWWXXXX..",
            b"XWWWXWWX.....",
            b"XWWX.XWX.....",
            b"XWX..XWX.....",
            b"XX....XWX....",
            b"X.....XWX....",
            b"......XWX....",
            b".......XX....",
        ];

        for (cy, row) in CURSOR.iter().enumerate() {
            for (cx, &pixel) in row.iter().enumerate() {
                let px = x + cx;
                let py = y + cy;
                if px < screen_width && py < screen_height {
                    let color = match pixel {
                        b'X' => Color::BLACK,
                        b'W' => Color::WHITE,
                        _ => continue,
                    };
                    surface.set_pixel(px, py, color);
                }
            }
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
        // Render desktop (wallpaper + icons) as background
        if super::desktop::is_available() {
            super::desktop::render_to(&mut self.back_buffer);
        } else {
            // Fallback: clear with solid color
            self.back_buffer.clear(self.background_color);
        }

        // Draw windows back to front
        for &id in self.window_order.iter().rev() {
            if let Some(window) = self.windows.get(&id) {
                if window.is_visible() {
                    window.render(&mut self.back_buffer, None);
                }
            }
        }

        // Draw cursor on top of everything using the overlay system
        if self.cursor_visible {
            // Save background at cursor position BEFORE drawing cursor
            // This initializes the cursor overlay for subsequent cursor-only updates
            self.save_cursor_background();

            let cx = self.cursor_x as usize;
            let cy = self.cursor_y as usize;
            let sw = self.screen_width;
            let sh = self.screen_height;
            Self::draw_cursor_at(&mut self.back_buffer, cx, cy, sw, sh);
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

/// Move the cursor by relative amount
pub fn move_cursor(dx: i32, dy: i32) {
    let mut comp = COMPOSITOR.lock();
    if let Some(ref mut c) = *comp {
        c.move_cursor(dx, dy);
    }
}

/// Set cursor position
pub fn set_cursor_pos(x: isize, y: isize) {
    let mut comp = COMPOSITOR.lock();
    if let Some(ref mut c) = *comp {
        c.set_cursor_pos(x, y);
    }
}

/// Get cursor position
pub fn cursor_pos() -> Option<(isize, isize)> {
    let comp = COMPOSITOR.lock();
    comp.as_ref().map(|c| c.cursor_pos())
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

/// Efficient cursor-only update (for mouse movement)
/// This only updates the cursor regions instead of doing a full compose+present
pub fn update_cursor_only() {
    let mut comp = COMPOSITOR.lock();
    if let Some(ref mut c) = *comp {
        c.update_cursor_only();
    }
}

/// Mark cursor as needing update (for deferred rendering)
/// Use this from interrupt context instead of update_cursor_only()
pub fn mark_cursor_dirty() {
    CURSOR_DIRTY.store(true, core::sync::atomic::Ordering::Release);
}

/// Process deferred cursor update if needed
/// Call this from the main loop or idle task (outside interrupt context)
pub fn process_deferred_cursor() {
    if CURSOR_DIRTY.swap(false, core::sync::atomic::Ordering::AcqRel) {
        update_cursor_only();
    }
}

/// Check if cursor needs update
pub fn is_cursor_dirty() -> bool {
    CURSOR_DIRTY.load(core::sync::atomic::Ordering::Acquire)
}
