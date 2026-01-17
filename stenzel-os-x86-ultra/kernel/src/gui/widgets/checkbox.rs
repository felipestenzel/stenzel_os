//! Checkbox widget
//!
//! A toggleable checkbox with optional label.

use alloc::string::String;
use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use super::{Widget, WidgetId, WidgetState, WidgetEvent, Bounds, MouseButton, theme};

/// Checkbox change callback
pub type CheckboxCallback = fn(WidgetId, bool);

/// A checkbox with optional label
pub struct Checkbox {
    id: WidgetId,
    bounds: Bounds,
    label: String,
    checked: bool,
    state: WidgetState,
    enabled: bool,
    visible: bool,
    on_change: Option<CheckboxCallback>,
}

impl Checkbox {
    /// Box size constant
    const BOX_SIZE: usize = 16;

    /// Create a new checkbox
    pub fn new(x: isize, y: isize, label: &str) -> Self {
        let char_width = 8;
        let label_width = label.chars().count() * char_width;
        let width = Self::BOX_SIZE + 8 + label_width;

        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(x, y, width, Self::BOX_SIZE),
            label: String::from(label),
            checked: false,
            state: WidgetState::Normal,
            enabled: true,
            visible: true,
            on_change: None,
        }
    }

    /// Set checked state
    pub fn set_checked(&mut self, checked: bool) {
        if self.checked != checked {
            self.checked = checked;
            self.notify_change();
        }
    }

    /// Get checked state
    pub fn is_checked(&self) -> bool {
        self.checked
    }

    /// Toggle checked state
    pub fn toggle(&mut self) {
        self.checked = !self.checked;
        self.notify_change();
    }

    /// Set label
    pub fn set_label(&mut self, label: &str) {
        self.label = String::from(label);
        let char_width = 8;
        let label_width = label.chars().count() * char_width;
        self.bounds.width = Self::BOX_SIZE + 8 + label_width;
    }

    /// Set change callback
    pub fn set_on_change(&mut self, callback: CheckboxCallback) {
        self.on_change = Some(callback);
    }

    fn notify_change(&self) {
        if let Some(callback) = self.on_change {
            callback(self.id, self.checked);
        }
    }
}

impl Widget for Checkbox {
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
                    self.toggle();
                    self.state = WidgetState::Hovered;
                }
                true
            }
            WidgetEvent::Click { button: MouseButton::Left } => {
                self.toggle();
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
            WidgetEvent::KeyDown { key: 0x39, .. } => { // Space
                if self.state == WidgetState::Focused {
                    self.toggle();
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

        // Determine colors
        let (box_bg, border_color, fg_color) = match self.state {
            WidgetState::Disabled => (theme.bg_disabled, theme.border, theme.fg_disabled),
            WidgetState::Hovered => (theme.bg_hover, theme.border, theme.fg),
            WidgetState::Pressed => (theme.bg_pressed, theme.accent, theme.fg),
            WidgetState::Focused => (theme.bg, theme.border_focused, theme.fg),
            WidgetState::Normal => (theme.bg, theme.border, theme.fg),
        };

        // Draw checkbox box
        let box_size = Self::BOX_SIZE;

        // Background
        for py in 1..box_size - 1 {
            for px in 1..box_size - 1 {
                surface.set_pixel(x + px, y + py, box_bg);
            }
        }

        // Border
        for px in 0..box_size {
            surface.set_pixel(x + px, y, border_color);
            surface.set_pixel(x + px, y + box_size - 1, border_color);
        }
        for py in 0..box_size {
            surface.set_pixel(x, y + py, border_color);
            surface.set_pixel(x + box_size - 1, y + py, border_color);
        }

        // Draw checkmark if checked
        if self.checked {
            let check_color = if self.state == WidgetState::Disabled {
                theme.fg_disabled
            } else {
                theme.accent
            };

            // Draw a simple checkmark
            // Line from (3,8) to (6,11)
            for i in 0..4 {
                surface.set_pixel(x + 3 + i, y + 8 + i, check_color);
                surface.set_pixel(x + 4 + i, y + 8 + i, check_color);
            }
            // Line from (6,11) to (12,5)
            for i in 0..7 {
                surface.set_pixel(x + 6 + i, y + 11 - i, check_color);
                surface.set_pixel(x + 7 + i, y + 11 - i, check_color);
            }
        }

        // Draw label
        if !self.label.is_empty() {
            let label_x = x + box_size + 8;
            let label_y = y + (box_size - 16) / 2;

            for (i, c) in self.label.chars().enumerate() {
                draw_char_simple(surface, label_x + i * 8, label_y, c, fg_color);
            }
        }
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
