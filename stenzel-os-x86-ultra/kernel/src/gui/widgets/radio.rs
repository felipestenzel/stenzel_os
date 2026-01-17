//! Radio Button widget
//!
//! Radio buttons for selecting one option from a group.

use alloc::string::String;
use alloc::vec::Vec;
use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use super::{Widget, WidgetId, WidgetState, WidgetEvent, Bounds, MouseButton, theme};

/// Radio button change callback
pub type RadioCallback = fn(WidgetId, bool);

/// A single radio button
pub struct RadioButton {
    id: WidgetId,
    bounds: Bounds,
    label: String,
    selected: bool,
    group_id: Option<u64>,
    state: WidgetState,
    enabled: bool,
    visible: bool,
    on_change: Option<RadioCallback>,
}

impl RadioButton {
    const RADIO_SIZE: usize = 16;

    /// Create a new radio button
    pub fn new(x: isize, y: isize, label: &str) -> Self {
        let char_width = 8;
        let label_width = label.chars().count() * char_width;
        let width = Self::RADIO_SIZE + 8 + label_width;

        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(x, y, width, Self::RADIO_SIZE),
            label: String::from(label),
            selected: false,
            group_id: None,
            state: WidgetState::Normal,
            enabled: true,
            visible: true,
            on_change: None,
        }
    }

    /// Set selected state
    pub fn set_selected(&mut self, selected: bool) {
        if self.selected != selected {
            self.selected = selected;
            if let Some(callback) = self.on_change {
                callback(self.id, selected);
            }
        }
    }

    /// Get selected state
    pub fn is_selected(&self) -> bool {
        self.selected
    }

    /// Set group ID
    pub fn set_group(&mut self, group_id: u64) {
        self.group_id = Some(group_id);
    }

    /// Get group ID
    pub fn group_id(&self) -> Option<u64> {
        self.group_id
    }

    /// Set label
    pub fn set_label(&mut self, label: &str) {
        self.label = String::from(label);
        let char_width = 8;
        let label_width = label.chars().count() * char_width;
        self.bounds.width = Self::RADIO_SIZE + 8 + label_width;
    }

    /// Set change callback
    pub fn set_on_change(&mut self, callback: RadioCallback) {
        self.on_change = Some(callback);
    }

    /// Select this radio button
    pub fn select(&mut self) {
        if !self.selected {
            self.selected = true;
            if let Some(callback) = self.on_change {
                callback(self.id, true);
            }
        }
    }
}

impl Widget for RadioButton {
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
                    self.select();
                    self.state = WidgetState::Hovered;
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
            WidgetEvent::KeyDown { key: 0x39, .. } => {
                if self.state == WidgetState::Focused {
                    self.select();
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

        let (bg_color, border_color, fg_color) = match self.state {
            WidgetState::Disabled => (theme.bg_disabled, theme.border, theme.fg_disabled),
            WidgetState::Hovered => (theme.bg_hover, theme.border, theme.fg),
            WidgetState::Pressed => (theme.bg_pressed, theme.accent, theme.fg),
            WidgetState::Focused => (theme.bg, theme.border_focused, theme.fg),
            WidgetState::Normal => (theme.bg, theme.border, theme.fg),
        };

        // Draw radio circle
        let size = Self::RADIO_SIZE;
        let cx = x + size / 2;
        let cy = y + size / 2;
        let radius = size / 2 - 1;

        // Draw filled circle (background)
        for py in 0..size {
            for px in 0..size {
                let dx = px as isize - (size / 2) as isize;
                let dy = py as isize - (size / 2) as isize;
                let dist_sq = dx * dx + dy * dy;
                let r_sq = (radius as isize) * (radius as isize);

                if dist_sq <= r_sq {
                    if dist_sq >= (radius as isize - 1) * (radius as isize - 1) {
                        // Border
                        surface.set_pixel(x + px, y + py, border_color);
                    } else {
                        // Fill
                        surface.set_pixel(x + px, y + py, bg_color);
                    }
                }
            }
        }

        // Draw selected dot
        if self.selected {
            let dot_color = if self.state == WidgetState::Disabled {
                theme.fg_disabled
            } else {
                theme.accent
            };

            let dot_radius = radius / 2;
            for py in 0..size {
                for px in 0..size {
                    let dx = px as isize - (size / 2) as isize;
                    let dy = py as isize - (size / 2) as isize;
                    let dist_sq = dx * dx + dy * dy;
                    let r_sq = (dot_radius as isize) * (dot_radius as isize);

                    if dist_sq <= r_sq {
                        surface.set_pixel(x + px, y + py, dot_color);
                    }
                }
            }
        }

        // Draw label
        if !self.label.is_empty() {
            let label_x = x + size + 8;
            let label_y = y + (size.saturating_sub(16)) / 2;

            for (i, c) in self.label.chars().enumerate() {
                draw_char_simple(surface, label_x + i * 8, label_y, c, fg_color);
            }
        }
    }
}

/// A group of radio buttons (only one can be selected)
pub struct RadioGroup {
    id: u64,
    buttons: Vec<RadioButton>,
    selected_index: Option<usize>,
}

static NEXT_GROUP_ID: spin::Mutex<u64> = spin::Mutex::new(1);

impl RadioGroup {
    /// Create a new radio group
    pub fn new() -> Self {
        let mut next = NEXT_GROUP_ID.lock();
        let id = *next;
        *next += 1;

        Self {
            id,
            buttons: Vec::new(),
            selected_index: None,
        }
    }

    /// Add a radio button to the group
    pub fn add_button(&mut self, mut button: RadioButton) {
        button.set_group(self.id);
        self.buttons.push(button);
    }

    /// Create and add a button
    pub fn add(&mut self, x: isize, y: isize, label: &str) -> usize {
        let mut button = RadioButton::new(x, y, label);
        button.set_group(self.id);
        let index = self.buttons.len();
        self.buttons.push(button);
        index
    }

    /// Select button by index
    pub fn select(&mut self, index: usize) {
        if index >= self.buttons.len() {
            return;
        }

        // Deselect current
        if let Some(current) = self.selected_index {
            if current < self.buttons.len() {
                self.buttons[current].set_selected(false);
            }
        }

        // Select new
        self.buttons[index].set_selected(true);
        self.selected_index = Some(index);
    }

    /// Get selected index
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    /// Get button count
    pub fn len(&self) -> usize {
        self.buttons.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.buttons.is_empty()
    }

    /// Get button by index
    pub fn get(&self, index: usize) -> Option<&RadioButton> {
        self.buttons.get(index)
    }

    /// Get mutable button by index
    pub fn get_mut(&mut self, index: usize) -> Option<&mut RadioButton> {
        self.buttons.get_mut(index)
    }

    /// Handle event for all buttons
    pub fn handle_event(&mut self, event: &WidgetEvent, x: isize, y: isize) -> bool {
        for (i, button) in self.buttons.iter_mut().enumerate() {
            if button.contains(x, y) {
                if button.handle_event(event) {
                    // If this button was selected, deselect others
                    if button.is_selected() {
                        self.selected_index = Some(i);
                        for (j, other) in self.buttons.iter_mut().enumerate() {
                            if j != i {
                                other.set_selected(false);
                            }
                        }
                    }
                    return true;
                }
            }
        }
        false
    }

    /// Render all buttons
    pub fn render(&self, surface: &mut Surface) {
        for button in &self.buttons {
            button.render(surface);
        }
    }
}

impl Default for RadioGroup {
    fn default() -> Self {
        Self::new()
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
