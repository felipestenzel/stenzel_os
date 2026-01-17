//! Dropdown/Combobox widget
//!
//! A dropdown menu for selecting from a list of options.

use alloc::string::String;
use alloc::vec::Vec;
use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use super::{Widget, WidgetId, WidgetState, WidgetEvent, Bounds, MouseButton, theme};

/// Dropdown selection callback
pub type DropdownCallback = fn(WidgetId, usize, &str);

/// A dropdown item
#[derive(Debug, Clone)]
pub struct DropdownItem {
    pub label: String,
    pub value: String,
    pub enabled: bool,
}

impl DropdownItem {
    pub fn new(label: &str, value: &str) -> Self {
        Self {
            label: String::from(label),
            value: String::from(value),
            enabled: true,
        }
    }

    pub fn simple(label: &str) -> Self {
        Self::new(label, label)
    }
}

/// A dropdown menu widget
pub struct Dropdown {
    id: WidgetId,
    bounds: Bounds,
    items: Vec<DropdownItem>,
    selected_index: Option<usize>,
    hover_index: Option<usize>,
    expanded: bool,
    state: WidgetState,
    enabled: bool,
    visible: bool,
    placeholder: String,
    max_visible_items: usize,
    scroll_offset: usize,
    on_select: Option<DropdownCallback>,
}

impl Dropdown {
    const ITEM_HEIGHT: usize = 24;
    const ARROW_WIDTH: usize = 20;

    /// Create a new dropdown
    pub fn new(x: isize, y: isize, width: usize) -> Self {
        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(x, y, width, 24),
            items: Vec::new(),
            selected_index: None,
            hover_index: None,
            expanded: false,
            state: WidgetState::Normal,
            enabled: true,
            visible: true,
            placeholder: String::from("Select..."),
            max_visible_items: 5,
            scroll_offset: 0,
            on_select: None,
        }
    }

    /// Add an item
    pub fn add_item(&mut self, item: DropdownItem) {
        self.items.push(item);
    }

    /// Add a simple item (label = value)
    pub fn add(&mut self, label: &str) {
        self.items.push(DropdownItem::simple(label));
    }

    /// Clear all items
    pub fn clear(&mut self) {
        self.items.clear();
        self.selected_index = None;
        self.hover_index = None;
        self.scroll_offset = 0;
    }

    /// Set selected index
    pub fn set_selected(&mut self, index: usize) {
        if index < self.items.len() {
            self.selected_index = Some(index);
            self.notify_selection();
        }
    }

    /// Get selected index
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    /// Get selected item
    pub fn selected_item(&self) -> Option<&DropdownItem> {
        self.selected_index.and_then(|i| self.items.get(i))
    }

    /// Get selected value
    pub fn selected_value(&self) -> Option<&str> {
        self.selected_item().map(|i| i.value.as_str())
    }

    /// Set placeholder text
    pub fn set_placeholder(&mut self, text: &str) {
        self.placeholder = String::from(text);
    }

    /// Set max visible items in dropdown
    pub fn set_max_visible(&mut self, max: usize) {
        self.max_visible_items = max.max(1);
    }

    /// Set selection callback
    pub fn set_on_select(&mut self, callback: DropdownCallback) {
        self.on_select = Some(callback);
    }

    /// Toggle expanded state
    pub fn toggle(&mut self) {
        self.expanded = !self.expanded;
        if self.expanded {
            // Scroll to show selected item
            if let Some(index) = self.selected_index {
                if index < self.scroll_offset {
                    self.scroll_offset = index;
                } else if index >= self.scroll_offset + self.max_visible_items {
                    self.scroll_offset = index - self.max_visible_items + 1;
                }
            }
        }
    }

    /// Close the dropdown
    pub fn close(&mut self) {
        self.expanded = false;
        self.hover_index = None;
    }

    /// Is the dropdown expanded?
    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    /// Get item count
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Get expanded bounds (includes dropdown list)
    pub fn expanded_bounds(&self) -> Bounds {
        if self.expanded {
            let visible_items = self.items.len().min(self.max_visible_items);
            let dropdown_height = visible_items * Self::ITEM_HEIGHT;
            Bounds::new(
                self.bounds.x,
                self.bounds.y,
                self.bounds.width,
                self.bounds.height + dropdown_height,
            )
        } else {
            self.bounds
        }
    }

    fn notify_selection(&self) {
        if let Some(callback) = self.on_select {
            if let Some(index) = self.selected_index {
                if let Some(item) = self.items.get(index) {
                    callback(self.id, index, &item.value);
                }
            }
        }
    }

    fn item_at_y(&self, y: isize) -> Option<usize> {
        let list_y = self.bounds.y + self.bounds.height as isize;
        if y < list_y {
            return None;
        }

        let rel_y = (y - list_y) as usize;
        let item_index = self.scroll_offset + rel_y / Self::ITEM_HEIGHT;

        if item_index < self.items.len() {
            Some(item_index)
        } else {
            None
        }
    }
}

impl Widget for Dropdown {
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
            self.close();
        } else if self.state == WidgetState::Disabled {
            self.state = WidgetState::Normal;
        }
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
        if !visible {
            self.close();
        }
    }

    fn handle_event(&mut self, event: &WidgetEvent) -> bool {
        if !self.enabled || !self.visible {
            return false;
        }

        match event {
            WidgetEvent::MouseEnter => {
                if !self.expanded {
                    self.state = WidgetState::Hovered;
                }
                true
            }
            WidgetEvent::MouseLeave => {
                if !self.expanded {
                    self.state = WidgetState::Normal;
                }
                self.hover_index = None;
                true
            }
            WidgetEvent::MouseMove { y, .. } => {
                if self.expanded {
                    self.hover_index = self.item_at_y(*y);
                    return true;
                }
                false
            }
            WidgetEvent::MouseDown { button: MouseButton::Left, y, .. } => {
                let in_header = *y < self.bounds.y + self.bounds.height as isize;

                if in_header {
                    self.toggle();
                    return true;
                } else if self.expanded {
                    if let Some(index) = self.item_at_y(*y) {
                        if self.items.get(index).map(|i| i.enabled).unwrap_or(false) {
                            self.selected_index = Some(index);
                            self.close();
                            self.notify_selection();
                        }
                    }
                    return true;
                }
                false
            }
            WidgetEvent::Blur => {
                self.close();
                self.state = WidgetState::Normal;
                true
            }
            WidgetEvent::KeyDown { key, .. } => {
                if !self.expanded {
                    if *key == 0x39 || *key == 0x1C { // Space or Enter
                        self.toggle();
                        return true;
                    }
                } else {
                    match *key {
                        0x48 => { // Up
                            if let Some(hover) = self.hover_index {
                                if hover > 0 {
                                    self.hover_index = Some(hover - 1);
                                    if hover - 1 < self.scroll_offset {
                                        self.scroll_offset = hover - 1;
                                    }
                                }
                            } else {
                                self.hover_index = Some(self.items.len().saturating_sub(1));
                            }
                            return true;
                        }
                        0x50 => { // Down
                            if let Some(hover) = self.hover_index {
                                if hover + 1 < self.items.len() {
                                    self.hover_index = Some(hover + 1);
                                    if hover + 1 >= self.scroll_offset + self.max_visible_items {
                                        self.scroll_offset = hover + 2 - self.max_visible_items;
                                    }
                                }
                            } else {
                                self.hover_index = Some(0);
                            }
                            return true;
                        }
                        0x1C => { // Enter
                            if let Some(hover) = self.hover_index {
                                if self.items.get(hover).map(|i| i.enabled).unwrap_or(false) {
                                    self.selected_index = Some(hover);
                                    self.close();
                                    self.notify_selection();
                                }
                            }
                            return true;
                        }
                        0x01 => { // Escape
                            self.close();
                            return true;
                        }
                        _ => {}
                    }
                }
                false
            }
            WidgetEvent::Scroll { delta_y, .. } => {
                if self.expanded {
                    if *delta_y < 0 && self.scroll_offset > 0 {
                        self.scroll_offset -= 1;
                    } else if *delta_y > 0 && self.scroll_offset + self.max_visible_items < self.items.len() {
                        self.scroll_offset += 1;
                    }
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

        // Determine header colors
        let (bg_color, border_color, fg_color) = match self.state {
            WidgetState::Disabled => (theme.bg_disabled, theme.border, theme.fg_disabled),
            WidgetState::Hovered | WidgetState::Pressed => (theme.bg_hover, theme.border, theme.fg),
            WidgetState::Focused => (theme.bg, theme.border_focused, theme.fg),
            WidgetState::Normal => (theme.bg, theme.border, theme.fg),
        };

        let border_color = if self.expanded { theme.border_focused } else { border_color };

        // Draw header background
        for py in 1..h.saturating_sub(1) {
            for px in 1..w.saturating_sub(1) {
                surface.set_pixel(x + px, y + py, bg_color);
            }
        }

        // Draw header border
        for px in 0..w {
            surface.set_pixel(x + px, y, border_color);
            surface.set_pixel(x + px, y + h - 1, border_color);
        }
        for py in 0..h {
            surface.set_pixel(x, y + py, border_color);
            surface.set_pixel(x + w - 1, y + py, border_color);
        }

        // Draw selected text or placeholder
        let text = self.selected_item()
            .map(|i| i.label.as_str())
            .unwrap_or(&self.placeholder);

        let text_x = x + 8;
        let text_y = y + (h - 16) / 2;
        let max_chars = (w - Self::ARROW_WIDTH - 12) / 8;

        let display_color = if self.selected_index.is_some() { fg_color } else { theme.fg_disabled };

        for (i, c) in text.chars().take(max_chars).enumerate() {
            draw_char_simple(surface, text_x + i * 8, text_y, c, display_color);
        }

        // Draw dropdown arrow
        let arrow_x = x + w - Self::ARROW_WIDTH + 4;
        let arrow_y = y + h / 2;

        if self.expanded {
            // Up arrow
            for i in 0..5 {
                surface.set_pixel(arrow_x + 4 - i, arrow_y - 2 + i, fg_color);
                surface.set_pixel(arrow_x + 4 + i, arrow_y - 2 + i, fg_color);
            }
        } else {
            // Down arrow
            for i in 0..5 {
                surface.set_pixel(arrow_x + 4 - i, arrow_y + 2 - i, fg_color);
                surface.set_pixel(arrow_x + 4 + i, arrow_y + 2 - i, fg_color);
            }
        }

        // Draw dropdown list if expanded
        if self.expanded {
            let visible_items = self.items.len().min(self.max_visible_items);
            let list_y = y + h;
            let list_height = visible_items * Self::ITEM_HEIGHT;

            // List background
            for py in 0..list_height {
                for px in 1..w - 1 {
                    surface.set_pixel(x + px, list_y + py, Color::new(50, 50, 58));
                }
            }

            // List border
            for py in 0..list_height {
                surface.set_pixel(x, list_y + py, border_color);
                surface.set_pixel(x + w - 1, list_y + py, border_color);
            }
            for px in 0..w {
                surface.set_pixel(x + px, list_y + list_height - 1, border_color);
            }

            // Draw items
            for i in 0..visible_items {
                let item_index = self.scroll_offset + i;
                if item_index >= self.items.len() {
                    break;
                }

                let item = &self.items[item_index];
                let item_y = list_y + i * Self::ITEM_HEIGHT;

                // Highlight hover/selected
                let is_hover = self.hover_index == Some(item_index);
                let is_selected = self.selected_index == Some(item_index);

                if is_hover || is_selected {
                    let highlight = if is_hover { theme.bg_hover } else { theme.accent };
                    for py in 0..Self::ITEM_HEIGHT {
                        for px in 1..w - 1 {
                            surface.set_pixel(x + px, item_y + py, highlight);
                        }
                    }
                }

                // Draw item text
                let item_color = if !item.enabled {
                    theme.fg_disabled
                } else if is_selected {
                    Color::WHITE
                } else {
                    theme.fg
                };

                let item_text_y = item_y + (Self::ITEM_HEIGHT - 16) / 2;
                for (j, c) in item.label.chars().take(max_chars).enumerate() {
                    draw_char_simple(surface, text_x + j * 8, item_text_y, c, item_color);
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
