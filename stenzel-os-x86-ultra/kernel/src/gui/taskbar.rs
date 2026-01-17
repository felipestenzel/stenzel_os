//! Taskbar/Panel module
//!
//! Provides the taskbar (panel) at the bottom of the screen with:
//! - Start menu button
//! - Running application buttons
//! - System tray
//! - Clock

use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;

use crate::drivers::framebuffer::Color;
use super::surface::{Surface, PixelFormat};
use super::window::WindowId;

/// Taskbar height in pixels
pub const TASKBAR_HEIGHT: usize = 32;

/// Taskbar button
#[derive(Debug, Clone)]
pub struct TaskbarButton {
    /// Associated window ID
    pub window_id: WindowId,
    /// Window title
    pub title: String,
    /// X position
    pub x: usize,
    /// Button width
    pub width: usize,
    /// Is this button active (window focused)?
    pub active: bool,
    /// Is the window minimized?
    pub minimized: bool,
}

/// System tray item
#[derive(Debug, Clone)]
pub struct TrayItem {
    /// Item ID
    pub id: u64,
    /// Tooltip
    pub tooltip: String,
    /// Icon (16x16 RGBA)
    pub icon: Option<Vec<u8>>,
    /// X position
    pub x: usize,
}

/// Taskbar state
pub struct Taskbar {
    /// Surface for rendering
    surface: Surface,
    /// Screen width
    screen_width: usize,
    /// Task buttons
    buttons: Vec<TaskbarButton>,
    /// System tray items
    tray_items: Vec<TrayItem>,
    /// Is start menu open?
    start_menu_open: bool,
    /// Taskbar background color
    bg_color: Color,
    /// Button background color
    button_bg: Color,
    /// Active button background
    button_active_bg: Color,
    /// Text color
    text_color: Color,
    /// Whether taskbar needs redraw
    dirty: bool,
    /// Start button width
    start_button_width: usize,
    /// Task button width
    task_button_width: usize,
    /// Clock area width
    clock_width: usize,
    /// Tray area width
    tray_width: usize,
}

impl Taskbar {
    /// Create a new taskbar
    pub fn new(screen_width: usize) -> Self {
        let surface = Surface::new(screen_width, TASKBAR_HEIGHT, PixelFormat::Rgba8888);

        let mut taskbar = Self {
            surface,
            screen_width,
            buttons: Vec::new(),
            tray_items: Vec::new(),
            start_menu_open: false,
            bg_color: Color::new(32, 32, 40),
            button_bg: Color::new(48, 48, 56),
            button_active_bg: Color::new(80, 80, 100),
            text_color: Color::WHITE,
            dirty: true,
            start_button_width: 48,
            task_button_width: 120,
            clock_width: 60,
            tray_width: 80,
        };

        taskbar.render();
        taskbar
    }

    /// Get the taskbar surface
    pub fn surface(&self) -> &Surface {
        &self.surface
    }

    /// Get taskbar position (x, y, width, height)
    pub fn bounds(&self, screen_height: usize) -> (usize, usize, usize, usize) {
        (0, screen_height - TASKBAR_HEIGHT, self.screen_width, TASKBAR_HEIGHT)
    }

    /// Update task buttons from window list
    pub fn update_tasks(&mut self, windows: &[(WindowId, String, bool, bool)]) {
        self.buttons.clear();

        let task_area_start = self.start_button_width + 8;
        let task_area_end = self.screen_width - self.clock_width - self.tray_width;
        let task_area_width = task_area_end - task_area_start;

        let num_windows = windows.len();
        let button_width = if num_windows > 0 {
            (task_area_width / num_windows).min(self.task_button_width)
        } else {
            self.task_button_width
        };

        let mut x = task_area_start;
        for (id, title, focused, minimized) in windows {
            self.buttons.push(TaskbarButton {
                window_id: *id,
                title: title.clone(),
                x,
                width: button_width.saturating_sub(4),
                active: *focused,
                minimized: *minimized,
            });
            x += button_width;
        }

        self.dirty = true;
    }

    /// Add a system tray item
    pub fn add_tray_item(&mut self, item: TrayItem) {
        self.tray_items.push(item);
        self.dirty = true;
    }

    /// Remove a tray item
    pub fn remove_tray_item(&mut self, id: u64) {
        self.tray_items.retain(|i| i.id != id);
        self.dirty = true;
    }

    /// Toggle start menu
    pub fn toggle_start_menu(&mut self) {
        self.start_menu_open = !self.start_menu_open;
        self.dirty = true;
    }

    /// Check if start menu is open
    pub fn is_start_menu_open(&self) -> bool {
        self.start_menu_open
    }

    /// Get button at position
    pub fn button_at(&self, x: usize, y: usize) -> Option<&TaskbarButton> {
        if y < TASKBAR_HEIGHT {
            for button in &self.buttons {
                if x >= button.x && x < button.x + button.width {
                    return Some(button);
                }
            }
        }
        None
    }

    /// Check if position is on start button
    pub fn is_on_start_button(&self, x: usize, y: usize) -> bool {
        y < TASKBAR_HEIGHT && x < self.start_button_width
    }

    /// Check if position is on clock area
    pub fn is_on_clock(&self, x: usize, y: usize) -> bool {
        y < TASKBAR_HEIGHT && x >= self.screen_width - self.clock_width
    }

    /// Check if taskbar needs redraw
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Clear dirty flag
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
        self.surface.clear_dirty();
    }

    /// Mark as needing redraw
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Render the taskbar
    pub fn render(&mut self) {
        // Clear with background color
        self.surface.clear(self.bg_color);

        // Draw start button
        self.draw_start_button();

        // Draw task buttons
        for i in 0..self.buttons.len() {
            let button = &self.buttons[i];
            self.draw_task_button(button.x, button.width, button.active, button.minimized);
        }

        // Draw tray area
        self.draw_tray_area();

        // Draw clock area
        self.draw_clock_area();

        // Draw top border
        for x in 0..self.screen_width {
            self.surface.set_pixel(x, 0, Color::new(64, 64, 72));
        }

        self.dirty = false;
    }

    /// Draw start button
    fn draw_start_button(&mut self) {
        let color = if self.start_menu_open {
            self.button_active_bg
        } else {
            self.button_bg
        };

        // Button background
        for y in 2..TASKBAR_HEIGHT - 2 {
            for x in 2..self.start_button_width - 2 {
                self.surface.set_pixel(x, y, color);
            }
        }

        // Draw a simple "S" or logo placeholder
        let logo_x = self.start_button_width / 2 - 4;
        let logo_y = TASKBAR_HEIGHT / 2 - 4;

        // Draw a simple square as logo placeholder
        for py in 0..8 {
            for px in 0..8 {
                if py == 0 || py == 7 || px == 0 || px == 7 || (py == 3 || py == 4) {
                    self.surface.set_pixel(logo_x + px, logo_y + py, Color::new(100, 200, 255));
                }
            }
        }
    }

    /// Draw a task button
    fn draw_task_button(&mut self, x: usize, width: usize, active: bool, minimized: bool) {
        let color = if active {
            self.button_active_bg
        } else if minimized {
            Color::new(40, 40, 48)
        } else {
            self.button_bg
        };

        // Button background
        for y in 2..TASKBAR_HEIGHT - 2 {
            for px in 0..width {
                self.surface.set_pixel(x + px, y, color);
            }
        }

        // Left border highlight
        for y in 2..TASKBAR_HEIGHT - 2 {
            self.surface.set_pixel(x, y, Color::new(80, 80, 88));
        }

        // Bottom highlight for active
        if active {
            for px in 0..width {
                self.surface.set_pixel(x + px, TASKBAR_HEIGHT - 3, Color::new(100, 180, 255));
                self.surface.set_pixel(x + px, TASKBAR_HEIGHT - 2, Color::new(100, 180, 255));
            }
        }
    }

    /// Draw system tray area
    fn draw_tray_area(&mut self) {
        let tray_x = self.screen_width - self.clock_width - self.tray_width;

        // Separator
        for y in 4..TASKBAR_HEIGHT - 4 {
            self.surface.set_pixel(tray_x, y, Color::new(64, 64, 72));
        }

        // Tray item placeholders (16x16 icons)
        let mut item_x = tray_x + 8;
        for _item in &self.tray_items {
            // Draw placeholder icon
            for py in 8..24 {
                for px in 0..16 {
                    self.surface.set_pixel(item_x + px, py, Color::new(100, 100, 108));
                }
            }
            item_x += 20;
        }
    }

    /// Draw clock area
    fn draw_clock_area(&mut self) {
        let clock_x = self.screen_width - self.clock_width;

        // Separator
        for y in 4..TASKBAR_HEIGHT - 4 {
            self.surface.set_pixel(clock_x, y, Color::new(64, 64, 72));
        }

        // Clock placeholder (actual time would need RTC integration)
        // Draw "00:00" as placeholder
        // In real implementation, we'd use font rendering here
    }

    /// Resize taskbar
    pub fn resize(&mut self, width: usize) {
        self.screen_width = width;
        self.surface.resize(width, TASKBAR_HEIGHT);
        self.dirty = true;
    }
}

/// Global taskbar instance
static TASKBAR: Mutex<Option<Taskbar>> = Mutex::new(None);

/// Initialize the taskbar
pub fn init(screen_width: usize) {
    let taskbar = Taskbar::new(screen_width);
    *TASKBAR.lock() = Some(taskbar);
    crate::kprintln!("taskbar: initialized (width={})", screen_width);
}

/// Update task buttons from window list
pub fn update_tasks(windows: &[(WindowId, String, bool, bool)]) {
    let mut taskbar = TASKBAR.lock();
    if let Some(ref mut t) = *taskbar {
        t.update_tasks(windows);
    }
}

/// Toggle start menu
pub fn toggle_start_menu() {
    let mut taskbar = TASKBAR.lock();
    if let Some(ref mut t) = *taskbar {
        t.toggle_start_menu();
    }
}

/// Check if start menu is open
pub fn is_start_menu_open() -> bool {
    let taskbar = TASKBAR.lock();
    taskbar.as_ref().map(|t| t.is_start_menu_open()).unwrap_or(false)
}

/// Get button at position
pub fn button_at(x: usize, y: usize) -> Option<WindowId> {
    let taskbar = TASKBAR.lock();
    taskbar.as_ref().and_then(|t| t.button_at(x, y).map(|b| b.window_id))
}

/// Check if position is on start button
pub fn is_on_start_button(x: usize, y: usize) -> bool {
    let taskbar = TASKBAR.lock();
    taskbar.as_ref().map(|t| t.is_on_start_button(x, y)).unwrap_or(false)
}

/// Execute with taskbar access
pub fn with_taskbar<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut Taskbar) -> R,
{
    let mut taskbar = TASKBAR.lock();
    taskbar.as_mut().map(f)
}

/// Check if taskbar is available
pub fn is_available() -> bool {
    TASKBAR.lock().is_some()
}

/// Render the taskbar
pub fn render() {
    let mut taskbar = TASKBAR.lock();
    if let Some(ref mut t) = *taskbar {
        t.render();
    }
}

/// Get taskbar height
pub fn height() -> usize {
    TASKBAR_HEIGHT
}
