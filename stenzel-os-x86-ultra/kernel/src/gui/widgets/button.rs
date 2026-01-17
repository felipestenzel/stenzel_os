//! Button widget
//!
//! A clickable button with text label.

use alloc::string::String;
use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use super::{Widget, WidgetId, WidgetState, WidgetEvent, Bounds, MouseButton, theme};

/// Button click callback type
pub type ButtonCallback = fn(WidgetId);

/// A clickable button
pub struct Button {
    id: WidgetId,
    bounds: Bounds,
    text: String,
    state: WidgetState,
    enabled: bool,
    visible: bool,
    on_click: Option<ButtonCallback>,
}

impl Button {
    /// Create a new button
    pub fn new(x: isize, y: isize, width: usize, height: usize, text: &str) -> Self {
        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(x, y, width, height),
            text: String::from(text),
            state: WidgetState::Normal,
            enabled: true,
            visible: true,
            on_click: None,
        }
    }

    /// Set button text
    pub fn set_text(&mut self, text: &str) {
        self.text = String::from(text);
    }

    /// Get button text
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Set click callback
    pub fn set_on_click(&mut self, callback: ButtonCallback) {
        self.on_click = Some(callback);
    }

    /// Get current state
    pub fn state(&self) -> WidgetState {
        self.state
    }

    /// Trigger click programmatically
    pub fn click(&mut self) {
        if self.enabled {
            if let Some(callback) = self.on_click {
                callback(self.id);
            }
        }
    }
}

impl Widget for Button {
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
        if !enabled {
            self.state = WidgetState::Disabled;
        } else if self.state == WidgetState::Disabled {
            self.state = WidgetState::Normal;
        }
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
            WidgetEvent::MouseEnter => {
                self.state = WidgetState::Hovered;
                true
            }
            WidgetEvent::MouseLeave => {
                self.state = WidgetState::Normal;
                true
            }
            WidgetEvent::MouseDown { button: MouseButton::Left, .. } => {
                self.state = WidgetState::Pressed;
                true
            }
            WidgetEvent::MouseUp { button: MouseButton::Left, .. } => {
                if self.state == WidgetState::Pressed {
                    self.state = WidgetState::Hovered;
                    self.click();
                }
                true
            }
            WidgetEvent::Focus => {
                self.state = WidgetState::Focused;
                true
            }
            WidgetEvent::Blur => {
                self.state = WidgetState::Normal;
                true
            }
            WidgetEvent::KeyDown { key: 0x1C, .. } | // Enter
            WidgetEvent::KeyDown { key: 0x39, .. } => { // Space
                if self.state == WidgetState::Focused {
                    self.state = WidgetState::Pressed;
                    return true;
                }
                false
            }
            WidgetEvent::KeyUp { key: 0x1C, .. } |
            WidgetEvent::KeyUp { key: 0x39, .. } => {
                if self.state == WidgetState::Pressed {
                    self.state = WidgetState::Focused;
                    self.click();
                    return true;
                }
                false
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

        // Determine colors based on state
        let (bg_color, fg_color, border_color) = match self.state {
            WidgetState::Normal => (theme.bg, theme.fg, theme.border),
            WidgetState::Hovered => (theme.bg_hover, theme.fg, theme.border),
            WidgetState::Pressed => (theme.bg_pressed, theme.fg, theme.accent),
            WidgetState::Focused => (theme.bg, theme.fg, theme.border_focused),
            WidgetState::Disabled => (theme.bg_disabled, theme.fg_disabled, theme.border),
        };

        // Draw background
        for py in 1..h.saturating_sub(1) {
            for px in 1..w.saturating_sub(1) {
                surface.set_pixel(x + px, y + py, bg_color);
            }
        }

        // Draw border
        for px in 0..w {
            surface.set_pixel(x + px, y, border_color);
            surface.set_pixel(x + px, y + h.saturating_sub(1), border_color);
        }
        for py in 0..h {
            surface.set_pixel(x, y + py, border_color);
            surface.set_pixel(x + w.saturating_sub(1), y + py, border_color);
        }

        // Draw pressed effect (inset)
        if self.state == WidgetState::Pressed {
            for px in 1..w.saturating_sub(1) {
                surface.set_pixel(x + px, y + 1, Color::new(30, 30, 38));
            }
            for py in 1..h.saturating_sub(1) {
                surface.set_pixel(x + 1, y + py, Color::new(30, 30, 38));
            }
        }

        // Draw text (centered)
        // Calculate text position
        let char_width = 8;
        let char_height = 16;
        let text_width = self.text.chars().count() * char_width;
        let text_x = x + (w.saturating_sub(text_width)) / 2;
        let text_y = y + (h.saturating_sub(char_height)) / 2;

        // Offset text when pressed
        let (text_x, text_y) = if self.state == WidgetState::Pressed {
            (text_x + 1, text_y + 1)
        } else {
            (text_x, text_y)
        };

        // Draw each character using simple bitmap rendering
        let mut cx = text_x;
        for c in self.text.chars() {
            draw_char_simple(surface, cx, text_y, c, fg_color);
            cx += char_width;
        }
    }
}

/// Simple character drawing (uses built-in font via framebuffer)
fn draw_char_simple(surface: &mut Surface, x: usize, y: usize, c: char, color: Color) {
    // Use the default font data
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
