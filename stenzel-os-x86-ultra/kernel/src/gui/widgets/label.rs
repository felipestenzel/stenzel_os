//! Label widget
//!
//! A text label for displaying static or dynamic text.

use alloc::string::String;
use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use super::{Widget, WidgetId, WidgetEvent, Bounds, theme};

/// Text alignment for labels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

/// A text label
pub struct Label {
    id: WidgetId,
    bounds: Bounds,
    text: String,
    color: Option<Color>,
    align: TextAlign,
    visible: bool,
    wrap: bool,
}

impl Label {
    /// Create a new label
    pub fn new(x: isize, y: isize, text: &str) -> Self {
        let char_width = 8;
        let char_height = 16;
        let width = text.chars().count() * char_width;

        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(x, y, width, char_height),
            text: String::from(text),
            color: None,
            align: TextAlign::Left,
            visible: true,
            wrap: false,
        }
    }

    /// Create with specific size
    pub fn new_sized(x: isize, y: isize, width: usize, height: usize, text: &str) -> Self {
        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(x, y, width, height),
            text: String::from(text),
            color: None,
            align: TextAlign::Left,
            visible: true,
            wrap: false,
        }
    }

    /// Set text
    pub fn set_text(&mut self, text: &str) {
        self.text = String::from(text);
        if !self.wrap {
            let char_width = 8;
            self.bounds.width = text.chars().count() * char_width;
        }
    }

    /// Get text
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Set custom color
    pub fn set_color(&mut self, color: Color) {
        self.color = Some(color);
    }

    /// Clear custom color (use theme color)
    pub fn clear_color(&mut self) {
        self.color = None;
    }

    /// Set alignment
    pub fn set_align(&mut self, align: TextAlign) {
        self.align = align;
    }

    /// Enable/disable word wrap
    pub fn set_wrap(&mut self, wrap: bool) {
        self.wrap = wrap;
    }
}

impl Widget for Label {
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
        true // Labels are always "enabled"
    }

    fn set_enabled(&mut self, _enabled: bool) {
        // Labels don't have enabled state
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn handle_event(&mut self, _event: &WidgetEvent) -> bool {
        false // Labels don't handle events
    }

    fn render(&self, surface: &mut Surface) {
        if !self.visible {
            return;
        }

        let theme = theme();
        let color = self.color.unwrap_or(theme.fg);

        let x = self.bounds.x.max(0) as usize;
        let y = self.bounds.y.max(0) as usize;
        let w = self.bounds.width;

        let char_width = 8;
        let char_height = 16;
        let text_width = self.text.chars().count() * char_width;

        // Calculate starting X based on alignment
        let text_x = match self.align {
            TextAlign::Left => x,
            TextAlign::Center => x + w.saturating_sub(text_width) / 2,
            TextAlign::Right => x + w.saturating_sub(text_width),
        };

        if self.wrap {
            // Word wrap rendering
            let mut line_y = y;
            let mut line_x = text_x;
            let max_x = x + w;

            for c in self.text.chars() {
                if c == '\n' {
                    line_x = text_x;
                    line_y += char_height;
                    continue;
                }

                if line_x + char_width > max_x {
                    line_x = text_x;
                    line_y += char_height;
                }

                if line_y + char_height > y + self.bounds.height {
                    break;
                }

                draw_char_simple(surface, line_x, line_y, c, color);
                line_x += char_width;
            }
        } else {
            // Single line rendering
            let mut cx = text_x;
            for c in self.text.chars() {
                if cx + char_width > x + w {
                    break;
                }
                draw_char_simple(surface, cx, y, c, color);
                cx += char_width;
            }
        }
    }
}

/// Simple character drawing
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
