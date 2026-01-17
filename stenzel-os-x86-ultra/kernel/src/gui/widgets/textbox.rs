//! TextBox widget
//!
//! A single-line text input field.

use alloc::string::String;
use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use super::{Widget, WidgetId, WidgetState, WidgetEvent, Bounds, MouseButton, theme};

/// TextBox change callback
pub type TextChangeCallback = fn(WidgetId, &str);

/// A single-line text input
pub struct TextBox {
    id: WidgetId,
    bounds: Bounds,
    text: String,
    placeholder: String,
    cursor_pos: usize,
    selection_start: Option<usize>,
    scroll_offset: usize,
    state: WidgetState,
    enabled: bool,
    visible: bool,
    password_mode: bool,
    max_length: Option<usize>,
    on_change: Option<TextChangeCallback>,
}

impl TextBox {
    /// Create a new textbox
    pub fn new(x: isize, y: isize, width: usize) -> Self {
        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(x, y, width, 24),
            text: String::new(),
            placeholder: String::new(),
            cursor_pos: 0,
            selection_start: None,
            scroll_offset: 0,
            state: WidgetState::Normal,
            enabled: true,
            visible: true,
            password_mode: false,
            max_length: None,
            on_change: None,
        }
    }

    /// Set text
    pub fn set_text(&mut self, text: &str) {
        self.text = String::from(text);
        self.cursor_pos = self.text.chars().count();
        self.selection_start = None;
        self.update_scroll();
    }

    /// Get text
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Set placeholder text
    pub fn set_placeholder(&mut self, placeholder: &str) {
        self.placeholder = String::from(placeholder);
    }

    /// Enable password mode (show dots instead of text)
    pub fn set_password_mode(&mut self, enabled: bool) {
        self.password_mode = enabled;
    }

    /// Set maximum length
    pub fn set_max_length(&mut self, max: usize) {
        self.max_length = Some(max);
    }

    /// Set change callback
    pub fn set_on_change(&mut self, callback: TextChangeCallback) {
        self.on_change = Some(callback);
    }

    /// Insert character at cursor
    fn insert_char(&mut self, c: char) {
        if let Some(max) = self.max_length {
            if self.text.chars().count() >= max {
                return;
            }
        }

        // Delete selection first if any
        self.delete_selection();

        let byte_pos = self.char_to_byte_pos(self.cursor_pos);
        self.text.insert(byte_pos, c);
        self.cursor_pos += 1;
        self.update_scroll();
        self.notify_change();
    }

    /// Delete character before cursor (backspace)
    fn delete_backward(&mut self) {
        if self.delete_selection() {
            return;
        }

        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            let byte_pos = self.char_to_byte_pos(self.cursor_pos);
            let char_len = self.text[byte_pos..].chars().next().map(|c| c.len_utf8()).unwrap_or(0);
            self.text.drain(byte_pos..byte_pos + char_len);
            self.update_scroll();
            self.notify_change();
        }
    }

    /// Delete character after cursor (delete)
    fn delete_forward(&mut self) {
        if self.delete_selection() {
            return;
        }

        let char_count = self.text.chars().count();
        if self.cursor_pos < char_count {
            let byte_pos = self.char_to_byte_pos(self.cursor_pos);
            let char_len = self.text[byte_pos..].chars().next().map(|c| c.len_utf8()).unwrap_or(0);
            self.text.drain(byte_pos..byte_pos + char_len);
            self.notify_change();
        }
    }

    /// Delete selected text, returns true if there was a selection
    fn delete_selection(&mut self) -> bool {
        if let Some(sel_start) = self.selection_start {
            let (start, end) = if sel_start < self.cursor_pos {
                (sel_start, self.cursor_pos)
            } else {
                (self.cursor_pos, sel_start)
            };

            let start_byte = self.char_to_byte_pos(start);
            let end_byte = self.char_to_byte_pos(end);
            self.text.drain(start_byte..end_byte);
            self.cursor_pos = start;
            self.selection_start = None;
            self.update_scroll();
            self.notify_change();
            true
        } else {
            false
        }
    }

    /// Move cursor left
    fn move_left(&mut self, select: bool) {
        if select && self.selection_start.is_none() {
            self.selection_start = Some(self.cursor_pos);
        } else if !select {
            self.selection_start = None;
        }

        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            self.update_scroll();
        }
    }

    /// Move cursor right
    fn move_right(&mut self, select: bool) {
        if select && self.selection_start.is_none() {
            self.selection_start = Some(self.cursor_pos);
        } else if !select {
            self.selection_start = None;
        }

        let char_count = self.text.chars().count();
        if self.cursor_pos < char_count {
            self.cursor_pos += 1;
            self.update_scroll();
        }
    }

    /// Move to start
    fn move_home(&mut self, select: bool) {
        if select && self.selection_start.is_none() {
            self.selection_start = Some(self.cursor_pos);
        } else if !select {
            self.selection_start = None;
        }
        self.cursor_pos = 0;
        self.update_scroll();
    }

    /// Move to end
    fn move_end(&mut self, select: bool) {
        if select && self.selection_start.is_none() {
            self.selection_start = Some(self.cursor_pos);
        } else if !select {
            self.selection_start = None;
        }
        self.cursor_pos = self.text.chars().count();
        self.update_scroll();
    }

    /// Select all text
    fn select_all(&mut self) {
        self.selection_start = Some(0);
        self.cursor_pos = self.text.chars().count();
    }

    /// Convert character position to byte position
    fn char_to_byte_pos(&self, char_pos: usize) -> usize {
        self.text.char_indices()
            .nth(char_pos)
            .map(|(i, _)| i)
            .unwrap_or(self.text.len())
    }

    /// Update scroll offset to keep cursor visible
    fn update_scroll(&mut self) {
        let char_width = 8;
        let visible_chars = (self.bounds.width - 8) / char_width;

        if self.cursor_pos < self.scroll_offset {
            self.scroll_offset = self.cursor_pos;
        } else if self.cursor_pos >= self.scroll_offset + visible_chars {
            self.scroll_offset = self.cursor_pos - visible_chars + 1;
        }
    }

    /// Notify change callback
    fn notify_change(&self) {
        if let Some(callback) = self.on_change {
            callback(self.id, &self.text);
        }
    }

    /// Get character index at pixel position
    fn char_at_x(&self, px: isize) -> usize {
        let char_width = 8;
        let rel_x = (px - self.bounds.x - 4).max(0) as usize;
        let char_index = self.scroll_offset + rel_x / char_width;
        char_index.min(self.text.chars().count())
    }
}

impl Widget for TextBox {
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
                if self.state != WidgetState::Focused {
                    self.state = WidgetState::Hovered;
                }
                true
            }
            WidgetEvent::MouseLeave => {
                if self.state != WidgetState::Focused {
                    self.state = WidgetState::Normal;
                }
                true
            }
            WidgetEvent::MouseDown { button: MouseButton::Left, x, .. } => {
                self.state = WidgetState::Focused;
                self.cursor_pos = self.char_at_x(*x);
                self.selection_start = None;
                true
            }
            WidgetEvent::Focus => {
                self.state = WidgetState::Focused;
                true
            }
            WidgetEvent::Blur => {
                self.state = WidgetState::Normal;
                self.selection_start = None;
                true
            }
            WidgetEvent::Character { c } => {
                if self.state == WidgetState::Focused && !c.is_control() {
                    self.insert_char(*c);
                    return true;
                }
                false
            }
            WidgetEvent::KeyDown { key, modifiers } => {
                if self.state != WidgetState::Focused {
                    return false;
                }

                let shift = (*modifiers & super::modifiers::SHIFT) != 0;
                let ctrl = (*modifiers & super::modifiers::CTRL) != 0;

                match *key {
                    0x0E => { self.delete_backward(); true } // Backspace
                    0x53 => { self.delete_forward(); true }  // Delete
                    0x4B => { self.move_left(shift); true }  // Left
                    0x4D => { self.move_right(shift); true } // Right
                    0x47 => { self.move_home(shift); true }  // Home
                    0x4F => { self.move_end(shift); true }   // End
                    0x1E if ctrl => { self.select_all(); true } // Ctrl+A
                    _ => false
                }
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

        // Determine colors
        let (bg_color, border_color) = match self.state {
            WidgetState::Disabled => (theme.bg_disabled, theme.border),
            WidgetState::Focused => (Color::new(40, 40, 48), theme.border_focused),
            WidgetState::Hovered => (Color::new(50, 50, 58), theme.border),
            _ => (Color::new(45, 45, 53), theme.border),
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

        // Text rendering area
        let text_x = x + 4;
        let text_y = y + (h - 16) / 2;
        let char_width = 8;
        let visible_chars = (w - 8) / char_width;

        // Get display text
        let display_text: String = if self.password_mode {
            core::iter::repeat('*').take(self.text.chars().count()).collect()
        } else {
            self.text.clone()
        };

        let text_color = if self.state == WidgetState::Disabled {
            theme.fg_disabled
        } else {
            theme.fg
        };

        // Draw selection background
        if let Some(sel_start) = self.selection_start {
            let (start, end) = if sel_start < self.cursor_pos {
                (sel_start, self.cursor_pos)
            } else {
                (self.cursor_pos, sel_start)
            };

            let sel_start_x = text_x + (start.saturating_sub(self.scroll_offset)) * char_width;
            let sel_end_x = text_x + (end.saturating_sub(self.scroll_offset)) * char_width;

            for py in text_y..text_y + 16 {
                for px in sel_start_x..sel_end_x.min(x + w - 4) {
                    surface.set_pixel(px, py, theme.accent);
                }
            }
        }

        // Draw text or placeholder
        if display_text.is_empty() && !self.placeholder.is_empty() {
            // Draw placeholder
            let mut cx = text_x;
            for c in self.placeholder.chars().skip(self.scroll_offset).take(visible_chars) {
                draw_char_simple(surface, cx, text_y, c, theme.fg_disabled);
                cx += char_width;
            }
        } else {
            // Draw text
            let mut cx = text_x;
            for c in display_text.chars().skip(self.scroll_offset).take(visible_chars) {
                draw_char_simple(surface, cx, text_y, c, text_color);
                cx += char_width;
            }
        }

        // Draw cursor (blinking would require timer)
        if self.state == WidgetState::Focused {
            let cursor_x = text_x + (self.cursor_pos.saturating_sub(self.scroll_offset)) * char_width;
            if cursor_x < x + w - 4 {
                for py in text_y..text_y + 16 {
                    surface.set_pixel(cursor_x, py, theme.fg);
                }
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
