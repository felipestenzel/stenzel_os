//! Lock Screen
//!
//! Full-screen lock interface for system security.

use alloc::string::String;
use alloc::vec::Vec;
use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use crate::gui::widgets::{Widget, WidgetId, WidgetEvent, Bounds, MouseButton};

/// Lock screen state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockState {
    Locked,
    Unlocking,
    Unlocked,
}

/// Unlock callback - returns true if password is correct
pub type UnlockCallback = fn(&str) -> bool;

/// Lock Screen widget
pub struct LockScreen {
    id: WidgetId,
    bounds: Bounds,

    // State
    state: LockState,
    visible: bool,
    focused: bool,

    // User info
    username: String,
    display_name: String,

    // Time display
    time_str: String,
    date_str: String,

    // Password input
    password: String,
    password_visible: bool,
    input_focused: bool,
    error_message: Option<String>,
    shake_animation: u8, // For shake effect on wrong password

    // Callbacks
    on_unlock: Option<UnlockCallback>,
}

impl LockScreen {
    const CHAR_WIDTH: usize = 8;
    const CHAR_HEIGHT: usize = 16;
    const INPUT_WIDTH: usize = 280;
    const INPUT_HEIGHT: usize = 40;
    const AVATAR_SIZE: usize = 96;

    /// Create a new lock screen
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(0, 0, width, height),
            state: LockState::Locked,
            visible: false,
            focused: false,
            username: String::from("user"),
            display_name: String::from("User"),
            time_str: String::from("00:00"),
            date_str: String::from("Monday, January 1"),
            password: String::new(),
            password_visible: false,
            input_focused: true,
            error_message: None,
            shake_animation: 0,
            on_unlock: None,
        }
    }

    /// Set unlock callback
    pub fn set_on_unlock(&mut self, callback: UnlockCallback) {
        self.on_unlock = Some(callback);
    }

    /// Set user information
    pub fn set_user(&mut self, username: &str, display_name: &str) {
        self.username = String::from(username);
        self.display_name = String::from(display_name);
    }

    /// Update time display
    pub fn set_time(&mut self, hours: u8, minutes: u8) {
        self.time_str = format_time(hours, minutes);
    }

    /// Update date display
    pub fn set_date(&mut self, weekday: &str, month: &str, day: u8) {
        self.date_str = format_date(weekday, month, day);
    }

    /// Lock the screen
    pub fn lock(&mut self) {
        self.state = LockState::Locked;
        self.visible = true;
        self.password.clear();
        self.error_message = None;
        self.input_focused = true;
    }

    /// Attempt to unlock
    fn attempt_unlock(&mut self) {
        if self.password.is_empty() {
            return;
        }

        self.state = LockState::Unlocking;

        let success = if let Some(callback) = self.on_unlock {
            callback(&self.password)
        } else {
            // No callback set - allow any password (for testing)
            true
        };

        if success {
            self.state = LockState::Unlocked;
            self.visible = false;
            self.password.clear();
            self.error_message = None;
        } else {
            self.state = LockState::Locked;
            self.password.clear();
            self.error_message = Some(String::from("Incorrect password"));
            self.shake_animation = 10; // Start shake effect
        }
    }

    /// Check if locked
    pub fn is_locked(&self) -> bool {
        self.state != LockState::Unlocked
    }

    /// Handle key input
    fn handle_key(&mut self, scancode: u8) -> bool {
        match scancode {
            0x1C => { // Enter
                self.attempt_unlock();
                true
            }
            0x0E => { // Backspace
                self.password.pop();
                self.error_message = None;
                true
            }
            0x01 => { // Escape
                self.password.clear();
                self.error_message = None;
                true
            }
            _ => false,
        }
    }
}

impl Widget for LockScreen {
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
        if !self.visible {
            return false;
        }

        match event {
            WidgetEvent::Focus => {
                self.focused = true;
                self.input_focused = true;
                true
            }
            WidgetEvent::Blur => {
                self.focused = false;
                true
            }
            WidgetEvent::KeyDown { key, .. } => {
                self.handle_key(*key)
            }
            WidgetEvent::Character { c } => {
                if self.input_focused && *c >= ' ' && *c != '\x7f' {
                    if self.password.len() < 64 {
                        self.password.push(*c);
                        self.error_message = None;
                    }
                    return true;
                }
                false
            }
            WidgetEvent::MouseDown { button: MouseButton::Left, .. } => {
                // Click anywhere to focus input
                self.input_focused = true;
                true
            }
            _ => false,
        }
    }

    fn render(&self, surface: &mut Surface) {
        if !self.visible {
            return;
        }

        let w = self.bounds.width;
        let h = self.bounds.height;

        // Dark overlay background
        let bg = Color::new(20, 20, 30);
        for py in 0..h {
            for px in 0..w {
                surface.set_pixel(px, py, bg);
            }
        }

        // Gradient at top
        for py in 0..100 {
            let alpha = (100 - py) as f32 / 100.0;
            let r = (20.0 + alpha * 10.0) as u8;
            let g = (20.0 + alpha * 20.0) as u8;
            let b = (30.0 + alpha * 40.0) as u8;
            for px in 0..w {
                surface.set_pixel(px, py, Color::new(r, g, b));
            }
        }

        // Center position for content
        let center_x = w / 2;
        let center_y = h / 2;

        // Time display (large, at top)
        let time_y = 60;
        let time_scale = 4; // Large text
        draw_string_scaled(surface, center_x.saturating_sub(self.time_str.len() * Self::CHAR_WIDTH * time_scale / 2), time_y, &self.time_str, Color::new(255, 255, 255), time_scale);

        // Date display
        let date_y = time_y + Self::CHAR_HEIGHT * time_scale + 16;
        draw_string_centered(surface, center_x, date_y, &self.date_str, Color::new(180, 180, 180));

        // Avatar placeholder (circle)
        let avatar_y = center_y - Self::AVATAR_SIZE / 2 - 40;
        let avatar_center_x = center_x;
        let avatar_center_y = avatar_y + Self::AVATAR_SIZE / 2;
        let radius = Self::AVATAR_SIZE / 2;

        // Draw circle
        let avatar_bg = Color::new(60, 60, 80);
        for py in 0..Self::AVATAR_SIZE {
            for px in 0..Self::AVATAR_SIZE {
                let dx = px as isize - radius as isize;
                let dy = py as isize - radius as isize;
                if dx * dx + dy * dy <= (radius as isize * radius as isize) {
                    surface.set_pixel(center_x - radius + px, avatar_y + py, avatar_bg);
                }
            }
        }

        // User icon (simple person shape)
        let icon_color = Color::new(150, 150, 170);
        // Head
        let head_radius = 12;
        for py in 0..head_radius * 2 {
            for px in 0..head_radius * 2 {
                let dx = px as isize - head_radius as isize;
                let dy = py as isize - head_radius as isize;
                if dx * dx + dy * dy <= (head_radius as isize * head_radius as isize) {
                    surface.set_pixel(avatar_center_x - head_radius + px, avatar_center_y - 20 + py, icon_color);
                }
            }
        }
        // Body (triangle-ish)
        for py in 0..30 {
            let width = 10 + py / 2;
            for px in 0..width {
                surface.set_pixel(avatar_center_x - width / 2 + px, avatar_center_y + 8 + py, icon_color);
            }
        }

        // Display name
        let name_y = avatar_y + Self::AVATAR_SIZE + 16;
        draw_string_centered(surface, center_x, name_y, &self.display_name, Color::new(255, 255, 255));

        // Password input box
        let input_y = name_y + 40;
        let input_x = center_x.saturating_sub(Self::INPUT_WIDTH / 2);

        // Apply shake animation
        let shake_offset = if self.shake_animation > 0 {
            ((self.shake_animation as isize % 4) - 2) * 3
        } else {
            0
        };
        let input_x_shake = (input_x as isize + shake_offset).max(0) as usize;

        // Input background
        let input_bg = if self.input_focused {
            Color::new(50, 50, 60)
        } else {
            Color::new(40, 40, 50)
        };
        for py in 0..Self::INPUT_HEIGHT {
            for px in 0..Self::INPUT_WIDTH {
                surface.set_pixel(input_x_shake + px, input_y + py, input_bg);
            }
        }

        // Input border
        let border_color = if self.input_focused {
            Color::new(0, 122, 204)
        } else {
            Color::new(80, 80, 100)
        };
        for px in 0..Self::INPUT_WIDTH {
            surface.set_pixel(input_x_shake + px, input_y, border_color);
            surface.set_pixel(input_x_shake + px, input_y + Self::INPUT_HEIGHT - 1, border_color);
        }
        for py in 0..Self::INPUT_HEIGHT {
            surface.set_pixel(input_x_shake, input_y + py, border_color);
            surface.set_pixel(input_x_shake + Self::INPUT_WIDTH - 1, input_y + py, border_color);
        }

        // Password dots or placeholder
        if self.password.is_empty() {
            draw_string(surface, input_x_shake + 16, input_y + 12, "Enter password", Color::new(100, 100, 120));
        } else {
            // Show dots for each character
            let dots: String = (0..self.password.len()).map(|_| '*').collect();
            draw_string(surface, input_x_shake + 16, input_y + 12, &dots, Color::new(255, 255, 255));
        }

        // Cursor
        if self.input_focused {
            let cursor_x = input_x_shake + 16 + self.password.len() * Self::CHAR_WIDTH;
            let cursor_y = input_y + 10;
            for py in 0..20 {
                surface.set_pixel(cursor_x, cursor_y + py, Color::new(255, 255, 255));
            }
        }

        // Error message
        if let Some(ref error) = self.error_message {
            let error_y = input_y + Self::INPUT_HEIGHT + 16;
            draw_string_centered(surface, center_x, error_y, error, Color::new(255, 100, 100));
        }

        // Hint
        let hint_y = input_y + Self::INPUT_HEIGHT + 50;
        draw_string_centered(surface, center_x, hint_y, "Press Enter to unlock", Color::new(100, 100, 120));

        // Bottom info
        let info_y = h - 40;
        draw_string_centered(surface, center_x, info_y, "Stenzel OS", Color::new(80, 80, 100));
    }
}

fn draw_string(surface: &mut Surface, x: usize, y: usize, s: &str, color: Color) {
    for (i, c) in s.chars().enumerate() {
        draw_char_simple(surface, x + i * 8, y, c, color);
    }
}

fn draw_string_centered(surface: &mut Surface, center_x: usize, y: usize, s: &str, color: Color) {
    let width = s.len() * 8;
    let x = center_x.saturating_sub(width / 2);
    draw_string(surface, x, y, s, color);
}

fn draw_string_scaled(surface: &mut Surface, x: usize, y: usize, s: &str, color: Color, scale: usize) {
    for (char_idx, c) in s.chars().enumerate() {
        draw_char_scaled(surface, x + char_idx * 8 * scale, y, c, color, scale);
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

fn draw_char_scaled(surface: &mut Surface, x: usize, y: usize, c: char, color: Color, scale: usize) {
    use crate::drivers::font::DEFAULT_FONT;

    if let Some(glyph) = DEFAULT_FONT.get_glyph(c) {
        for row in 0..DEFAULT_FONT.height {
            let byte = glyph[row];
            for col in 0..DEFAULT_FONT.width {
                if (byte >> (DEFAULT_FONT.width - 1 - col)) & 1 != 0 {
                    // Draw scaled pixel
                    for sy in 0..scale {
                        for sx in 0..scale {
                            surface.set_pixel(x + col * scale + sx, y + row * scale + sy, color);
                        }
                    }
                }
            }
        }
    }
}

fn format_time(hours: u8, minutes: u8) -> String {
    use alloc::string::ToString;
    let mut s = String::new();
    if hours < 10 {
        s.push('0');
    }
    s.push_str(&hours.to_string());
    s.push(':');
    if minutes < 10 {
        s.push('0');
    }
    s.push_str(&minutes.to_string());
    s
}

fn format_date(weekday: &str, month: &str, day: u8) -> String {
    use alloc::string::ToString;
    let mut s = String::from(weekday);
    s.push_str(", ");
    s.push_str(month);
    s.push(' ');
    s.push_str(&day.to_string());
    s
}
