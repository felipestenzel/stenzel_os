//! List View widget
//!
//! A scrollable list of selectable items.

use alloc::string::String;
use alloc::vec::Vec;
use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use super::{Widget, WidgetId, WidgetState, WidgetEvent, Bounds, MouseButton, theme};

/// List item selection callback
pub type ListSelectCallback = fn(WidgetId, usize, &str);

/// A list item
#[derive(Debug, Clone)]
pub struct ListItem {
    pub text: String,
    pub data: Option<String>,
    pub enabled: bool,
    pub icon: Option<usize>, // Icon index (for future use)
}

impl ListItem {
    pub fn new(text: &str) -> Self {
        Self {
            text: String::from(text),
            data: None,
            enabled: true,
            icon: None,
        }
    }

    pub fn with_data(text: &str, data: &str) -> Self {
        Self {
            text: String::from(text),
            data: Some(String::from(data)),
            enabled: true,
            icon: None,
        }
    }
}

/// Selection mode for list view
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMode {
    /// No selection allowed
    None,
    /// Single item selection
    Single,
    /// Multiple item selection
    Multiple,
}

/// A scrollable list view widget
pub struct ListView {
    id: WidgetId,
    bounds: Bounds,
    items: Vec<ListItem>,
    selected_indices: Vec<usize>,
    hover_index: Option<usize>,
    scroll_offset: usize,
    selection_mode: SelectionMode,
    state: WidgetState,
    enabled: bool,
    visible: bool,
    show_scrollbar: bool,
    on_select: Option<ListSelectCallback>,
    on_double_click: Option<ListSelectCallback>,
}

impl ListView {
    const ITEM_HEIGHT: usize = 24;
    const SCROLLBAR_WIDTH: usize = 16;

    /// Create a new list view
    pub fn new(x: isize, y: isize, width: usize, height: usize) -> Self {
        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(x, y, width, height),
            items: Vec::new(),
            selected_indices: Vec::new(),
            hover_index: None,
            scroll_offset: 0,
            selection_mode: SelectionMode::Single,
            state: WidgetState::Normal,
            enabled: true,
            visible: true,
            show_scrollbar: true,
            on_select: None,
            on_double_click: None,
        }
    }

    /// Add an item
    pub fn add_item(&mut self, item: ListItem) {
        self.items.push(item);
    }

    /// Add a simple text item
    pub fn add(&mut self, text: &str) {
        self.items.push(ListItem::new(text));
    }

    /// Clear all items
    pub fn clear(&mut self) {
        self.items.clear();
        self.selected_indices.clear();
        self.hover_index = None;
        self.scroll_offset = 0;
    }

    /// Get item count
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Get item by index
    pub fn get(&self, index: usize) -> Option<&ListItem> {
        self.items.get(index)
    }

    /// Get mutable item by index
    pub fn get_mut(&mut self, index: usize) -> Option<&mut ListItem> {
        self.items.get_mut(index)
    }

    /// Set selection mode
    pub fn set_selection_mode(&mut self, mode: SelectionMode) {
        self.selection_mode = mode;
        if mode == SelectionMode::None {
            self.selected_indices.clear();
        } else if mode == SelectionMode::Single && self.selected_indices.len() > 1 {
            let first = self.selected_indices[0];
            self.selected_indices.clear();
            self.selected_indices.push(first);
        }
    }

    /// Select item by index
    pub fn select(&mut self, index: usize) {
        if index >= self.items.len() || self.selection_mode == SelectionMode::None {
            return;
        }

        if !self.items[index].enabled {
            return;
        }

        match self.selection_mode {
            SelectionMode::Single => {
                self.selected_indices.clear();
                self.selected_indices.push(index);
            }
            SelectionMode::Multiple => {
                if !self.selected_indices.contains(&index) {
                    self.selected_indices.push(index);
                }
            }
            SelectionMode::None => {}
        }

        self.ensure_visible(index);
        self.notify_select(index);
    }

    /// Toggle selection (for multi-select)
    pub fn toggle_select(&mut self, index: usize) {
        if self.selection_mode != SelectionMode::Multiple {
            self.select(index);
            return;
        }

        if let Some(pos) = self.selected_indices.iter().position(|&i| i == index) {
            self.selected_indices.remove(pos);
        } else {
            self.select(index);
        }
    }

    /// Deselect item
    pub fn deselect(&mut self, index: usize) {
        if let Some(pos) = self.selected_indices.iter().position(|&i| i == index) {
            self.selected_indices.remove(pos);
        }
    }

    /// Deselect all
    pub fn deselect_all(&mut self) {
        self.selected_indices.clear();
    }

    /// Get selected index (first one for multi-select)
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_indices.first().copied()
    }

    /// Get all selected indices
    pub fn selected_indices(&self) -> &[usize] {
        &self.selected_indices
    }

    /// Check if index is selected
    pub fn is_selected(&self, index: usize) -> bool {
        self.selected_indices.contains(&index)
    }

    /// Get selected item (first one)
    pub fn selected_item(&self) -> Option<&ListItem> {
        self.selected_index().and_then(|i| self.items.get(i))
    }

    /// Set select callback
    pub fn set_on_select(&mut self, callback: ListSelectCallback) {
        self.on_select = Some(callback);
    }

    /// Set double-click callback
    pub fn set_on_double_click(&mut self, callback: ListSelectCallback) {
        self.on_double_click = Some(callback);
    }

    /// Enable/disable scrollbar
    pub fn set_show_scrollbar(&mut self, show: bool) {
        self.show_scrollbar = show;
    }

    /// Scroll to make item visible
    pub fn ensure_visible(&mut self, index: usize) {
        let visible_count = self.visible_items();
        if index < self.scroll_offset {
            self.scroll_offset = index;
        } else if index >= self.scroll_offset + visible_count {
            self.scroll_offset = index.saturating_sub(visible_count) + 1;
        }
    }

    /// Get number of visible items
    fn visible_items(&self) -> usize {
        (self.bounds.height - 2) / Self::ITEM_HEIGHT
    }

    /// Get item index at y position
    fn item_at_y(&self, y: isize) -> Option<usize> {
        if y < self.bounds.y + 1 {
            return None;
        }

        let rel_y = (y - self.bounds.y - 1) as usize;
        let index = self.scroll_offset + rel_y / Self::ITEM_HEIGHT;

        if index < self.items.len() {
            Some(index)
        } else {
            None
        }
    }

    /// Check if scrollbar needed
    fn needs_scrollbar(&self) -> bool {
        self.show_scrollbar && self.items.len() > self.visible_items()
    }

    fn notify_select(&self, index: usize) {
        if let Some(callback) = self.on_select {
            if let Some(item) = self.items.get(index) {
                callback(self.id, index, &item.text);
            }
        }
    }

    fn notify_double_click(&self, index: usize) {
        if let Some(callback) = self.on_double_click {
            if let Some(item) = self.items.get(index) {
                callback(self.id, index, &item.text);
            }
        }
    }
}

impl Widget for ListView {
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
                self.hover_index = None;
                true
            }
            WidgetEvent::MouseMove { y, x, .. } => {
                // Check if in scrollbar area
                if self.needs_scrollbar() {
                    let scrollbar_x = self.bounds.x + self.bounds.width as isize - Self::SCROLLBAR_WIDTH as isize;
                    if *x >= scrollbar_x {
                        self.hover_index = None;
                        return true;
                    }
                }

                self.hover_index = self.item_at_y(*y);
                true
            }
            WidgetEvent::MouseDown { button: MouseButton::Left, x, y } => {
                // Handle scrollbar click
                if self.needs_scrollbar() {
                    let scrollbar_x = self.bounds.x + self.bounds.width as isize - Self::SCROLLBAR_WIDTH as isize;
                    if *x >= scrollbar_x {
                        // Calculate scroll position from click
                        let rel_y = (*y - self.bounds.y - 1).max(0) as usize;
                        let track_height = self.bounds.height - 2;
                        let total_items = self.items.len();
                        let new_offset = (rel_y * total_items) / track_height;
                        self.scroll_offset = new_offset.min(total_items.saturating_sub(self.visible_items()));
                        return true;
                    }
                }

                if let Some(index) = self.item_at_y(*y) {
                    if self.items[index].enabled {
                        self.select(index);
                    }
                }
                true
            }
            WidgetEvent::DoubleClick { x, y, .. } => {
                if let Some(index) = self.item_at_y(*y) {
                    // Check if not in scrollbar
                    if self.needs_scrollbar() {
                        let scrollbar_x = self.bounds.x + self.bounds.width as isize - Self::SCROLLBAR_WIDTH as isize;
                        if *x >= scrollbar_x {
                            return true;
                        }
                    }

                    if self.items[index].enabled {
                        self.notify_double_click(index);
                    }
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
            WidgetEvent::KeyDown { key, modifiers } => {
                let ctrl = (*modifiers & super::modifiers::CTRL) != 0;

                match *key {
                    0x48 => { // Up
                        if let Some(index) = self.selected_index() {
                            if index > 0 {
                                self.select(index - 1);
                            }
                        } else if !self.items.is_empty() {
                            self.select(0);
                        }
                        true
                    }
                    0x50 => { // Down
                        if let Some(index) = self.selected_index() {
                            if index + 1 < self.items.len() {
                                self.select(index + 1);
                            }
                        } else if !self.items.is_empty() {
                            self.select(0);
                        }
                        true
                    }
                    0x47 => { // Home
                        if !self.items.is_empty() {
                            self.select(0);
                        }
                        true
                    }
                    0x4F => { // End
                        if !self.items.is_empty() {
                            self.select(self.items.len() - 1);
                        }
                        true
                    }
                    0x49 => { // Page Up
                        if let Some(index) = self.selected_index() {
                            let new_index = index.saturating_sub(self.visible_items());
                            self.select(new_index);
                        }
                        true
                    }
                    0x51 => { // Page Down
                        if let Some(index) = self.selected_index() {
                            let new_index = (index + self.visible_items()).min(self.items.len() - 1);
                            self.select(new_index);
                        }
                        true
                    }
                    0x1E if ctrl => { // Ctrl+A - select all
                        if self.selection_mode == SelectionMode::Multiple {
                            self.selected_indices.clear();
                            for i in 0..self.items.len() {
                                if self.items[i].enabled {
                                    self.selected_indices.push(i);
                                }
                            }
                        }
                        true
                    }
                    _ => false,
                }
            }
            WidgetEvent::Scroll { delta_y, .. } => {
                let delta = *delta_y * 3;
                if delta < 0 {
                    self.scroll_offset = self.scroll_offset.saturating_sub((-delta) as usize);
                } else {
                    let max_scroll = self.items.len().saturating_sub(self.visible_items());
                    self.scroll_offset = (self.scroll_offset + delta as usize).min(max_scroll);
                }
                true
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

        let bg_color = Color::new(35, 35, 43);
        let border_color = if self.state == WidgetState::Focused {
            theme.border_focused
        } else {
            theme.border
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
            surface.set_pixel(x + px, y + h - 1, border_color);
        }
        for py in 0..h {
            surface.set_pixel(x, y + py, border_color);
            surface.set_pixel(x + w - 1, y + py, border_color);
        }

        // Calculate content area
        let content_width = if self.needs_scrollbar() {
            w - 2 - Self::SCROLLBAR_WIDTH
        } else {
            w - 2
        };

        // Draw items
        let visible_count = self.visible_items();
        for i in 0..visible_count {
            let item_index = self.scroll_offset + i;
            if item_index >= self.items.len() {
                break;
            }

            let item = &self.items[item_index];
            let item_y = y + 1 + i * Self::ITEM_HEIGHT;

            // Draw item background
            let is_selected = self.is_selected(item_index);
            let is_hover = self.hover_index == Some(item_index);

            let item_bg = if is_selected {
                theme.accent
            } else if is_hover {
                theme.bg_hover
            } else {
                bg_color
            };

            for py in 0..Self::ITEM_HEIGHT {
                for px in 0..content_width {
                    surface.set_pixel(x + 1 + px, item_y + py, item_bg);
                }
            }

            // Draw item text
            let text_color = if !item.enabled {
                theme.fg_disabled
            } else if is_selected {
                Color::WHITE
            } else {
                theme.fg
            };

            let text_y = item_y + (Self::ITEM_HEIGHT - 16) / 2;
            let max_chars = content_width / 8;

            for (j, c) in item.text.chars().take(max_chars).enumerate() {
                draw_char_simple(surface, x + 5 + j * 8, text_y, c, text_color);
            }
        }

        // Draw scrollbar if needed
        if self.needs_scrollbar() {
            let scrollbar_x = x + w - Self::SCROLLBAR_WIDTH - 1;
            let scrollbar_h = h - 2;

            // Scrollbar track
            for py in 0..scrollbar_h {
                for px in 0..Self::SCROLLBAR_WIDTH {
                    surface.set_pixel(scrollbar_x + px, y + 1 + py, Color::new(45, 45, 53));
                }
            }

            // Scrollbar thumb
            let total_items = self.items.len();
            let thumb_height = (visible_count * scrollbar_h / total_items).max(20);
            let thumb_y = if total_items > visible_count {
                self.scroll_offset * (scrollbar_h - thumb_height) / (total_items - visible_count)
            } else {
                0
            };

            for py in 0..thumb_height {
                for px in 2..Self::SCROLLBAR_WIDTH - 2 {
                    surface.set_pixel(scrollbar_x + px, y + 1 + thumb_y + py, Color::new(80, 80, 88));
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
