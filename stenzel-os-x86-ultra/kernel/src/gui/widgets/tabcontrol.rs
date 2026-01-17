//! Tab Control Widget
//!
//! A tabbed interface widget for organizing content into multiple panels.

use alloc::string::String;
use alloc::vec::Vec;
use super::{Widget, WidgetId, WidgetState, Bounds, WidgetEvent, MouseButton, theme};
use crate::gui::surface::Surface;
use crate::drivers::framebuffer::Color;

/// Unique identifier for tabs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TabId(pub u64);

static NEXT_TAB_ID: spin::Mutex<u64> = spin::Mutex::new(1);

impl TabId {
    pub fn new() -> Self {
        let mut next = NEXT_TAB_ID.lock();
        let id = *next;
        *next += 1;
        TabId(id)
    }
}

impl Default for TabId {
    fn default() -> Self {
        Self::new()
    }
}

/// Tab style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabStyle {
    /// Standard tabs with borders
    Standard,
    /// Pill-shaped tabs
    Pill,
    /// Underlined tabs
    Underline,
    /// Flat tabs (no borders)
    Flat,
}

/// Tab position
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabPosition {
    Top,
    Bottom,
}

/// A single tab
#[derive(Debug, Clone)]
pub struct Tab {
    pub id: TabId,
    pub label: String,
    pub icon: Option<char>,
    pub closable: bool,
    pub enabled: bool,
    pub user_data: u64,
}

impl Tab {
    pub fn new(label: &str) -> Self {
        Self {
            id: TabId::new(),
            label: String::from(label),
            icon: None,
            closable: false,
            enabled: true,
            user_data: 0,
        }
    }

    pub fn with_icon(label: &str, icon: char) -> Self {
        Self {
            id: TabId::new(),
            label: String::from(label),
            icon: Some(icon),
            closable: false,
            enabled: true,
            user_data: 0,
        }
    }

    pub fn set_closable(&mut self, closable: bool) {
        self.closable = closable;
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn set_user_data(&mut self, data: u64) {
        self.user_data = data;
    }
}

/// Tab Control widget
pub struct TabControl {
    id: WidgetId,
    bounds: Bounds,
    tabs: Vec<Tab>,
    active_index: usize,
    hovered_index: Option<usize>,
    hovered_close: Option<usize>,
    style: TabStyle,
    position: TabPosition,
    tab_height: usize,
    min_tab_width: usize,
    max_tab_width: usize,
    tab_padding: usize,
    scroll_offset: usize,
    enabled: bool,
    visible: bool,
    state: WidgetState,
    on_tab_change: Option<fn(TabId, usize)>,
    on_tab_close: Option<fn(TabId) -> bool>,
}

impl TabControl {
    pub fn new(x: isize, y: isize, width: usize, height: usize) -> Self {
        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(x, y, width, height),
            tabs: Vec::new(),
            active_index: 0,
            hovered_index: None,
            hovered_close: None,
            style: TabStyle::Standard,
            position: TabPosition::Top,
            tab_height: 32,
            min_tab_width: 80,
            max_tab_width: 200,
            tab_padding: 12,
            scroll_offset: 0,
            enabled: true,
            visible: true,
            state: WidgetState::Normal,
            on_tab_change: None,
            on_tab_close: None,
        }
    }

    pub fn set_style(&mut self, style: TabStyle) {
        self.style = style;
    }

    pub fn set_position(&mut self, position: TabPosition) {
        self.position = position;
    }

    pub fn set_tab_height(&mut self, height: usize) {
        self.tab_height = height;
    }

    pub fn set_tab_width_range(&mut self, min: usize, max: usize) {
        self.min_tab_width = min;
        self.max_tab_width = max;
    }

    pub fn set_tab_padding(&mut self, padding: usize) {
        self.tab_padding = padding;
    }

    /// Add a tab
    pub fn add_tab(&mut self, tab: Tab) -> TabId {
        let id = tab.id;
        self.tabs.push(tab);
        id
    }

    /// Insert a tab at a specific index
    pub fn insert_tab(&mut self, index: usize, tab: Tab) -> TabId {
        let id = tab.id;
        let idx = index.min(self.tabs.len());
        self.tabs.insert(idx, tab);
        // Adjust active index if needed
        if self.active_index >= idx && self.active_index > 0 {
            self.active_index += 1;
        }
        id
    }

    /// Remove a tab by ID
    pub fn remove_tab(&mut self, id: TabId) -> bool {
        if let Some(idx) = self.tabs.iter().position(|t| t.id == id) {
            self.tabs.remove(idx);
            // Adjust active index
            if self.active_index >= self.tabs.len() && self.active_index > 0 {
                self.active_index = self.tabs.len() - 1;
            } else if self.active_index > idx {
                self.active_index -= 1;
            }
            return true;
        }
        false
    }

    /// Remove a tab by index
    pub fn remove_tab_at(&mut self, index: usize) -> bool {
        if index < self.tabs.len() {
            self.tabs.remove(index);
            if self.active_index >= self.tabs.len() && self.active_index > 0 {
                self.active_index = self.tabs.len() - 1;
            } else if self.active_index > index {
                self.active_index -= 1;
            }
            return true;
        }
        false
    }

    /// Clear all tabs
    pub fn clear(&mut self) {
        self.tabs.clear();
        self.active_index = 0;
        self.scroll_offset = 0;
    }

    /// Get tab count
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Get active tab index
    pub fn active_index(&self) -> usize {
        self.active_index
    }

    /// Get active tab ID
    pub fn active_tab_id(&self) -> Option<TabId> {
        self.tabs.get(self.active_index).map(|t| t.id)
    }

    /// Set active tab by index
    pub fn set_active_index(&mut self, index: usize) {
        if index < self.tabs.len() && self.tabs[index].enabled {
            let old_index = self.active_index;
            self.active_index = index;
            if old_index != index {
                if let Some(callback) = self.on_tab_change {
                    callback(self.tabs[index].id, index);
                }
            }
            self.ensure_visible(index);
        }
    }

    /// Set active tab by ID
    pub fn set_active_tab(&mut self, id: TabId) {
        if let Some(idx) = self.tabs.iter().position(|t| t.id == id) {
            self.set_active_index(idx);
        }
    }

    /// Get tab by index
    pub fn get_tab(&self, index: usize) -> Option<&Tab> {
        self.tabs.get(index)
    }

    /// Get tab by index (mutable)
    pub fn get_tab_mut(&mut self, index: usize) -> Option<&mut Tab> {
        self.tabs.get_mut(index)
    }

    /// Get tab by ID
    pub fn find_tab(&self, id: TabId) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.id == id)
    }

    /// Get tab by ID (mutable)
    pub fn find_tab_mut(&mut self, id: TabId) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|t| t.id == id)
    }

    /// Set on_tab_change callback
    pub fn set_on_tab_change(&mut self, callback: fn(TabId, usize)) {
        self.on_tab_change = Some(callback);
    }

    /// Set on_tab_close callback (return true to allow close)
    pub fn set_on_tab_close(&mut self, callback: fn(TabId) -> bool) {
        self.on_tab_close = Some(callback);
    }

    /// Get content area bounds
    pub fn content_bounds(&self) -> Bounds {
        match self.position {
            TabPosition::Top => Bounds::new(
                self.bounds.x,
                self.bounds.y + self.tab_height as isize,
                self.bounds.width,
                self.bounds.height.saturating_sub(self.tab_height),
            ),
            TabPosition::Bottom => Bounds::new(
                self.bounds.x,
                self.bounds.y,
                self.bounds.width,
                self.bounds.height.saturating_sub(self.tab_height),
            ),
        }
    }

    /// Calculate tab width
    fn calculate_tab_width(&self, tab: &Tab) -> usize {
        let char_width = 8;
        let text_width = tab.label.len() * char_width;
        let icon_width = if tab.icon.is_some() { 16 } else { 0 };
        let close_width = if tab.closable { 18 } else { 0 };
        let total = text_width + icon_width + close_width + self.tab_padding * 2;
        total.clamp(self.min_tab_width, self.max_tab_width)
    }

    /// Get tab x position
    fn get_tab_x(&self, index: usize) -> isize {
        let mut x = self.bounds.x - self.scroll_offset as isize;
        for i in 0..index {
            if i < self.tabs.len() {
                x += self.calculate_tab_width(&self.tabs[i]) as isize;
            }
        }
        x
    }

    /// Get total tabs width
    fn total_tabs_width(&self) -> usize {
        self.tabs.iter().map(|t| self.calculate_tab_width(t)).sum()
    }

    /// Ensure tab at index is visible
    fn ensure_visible(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }

        let tab_x = self.get_tab_x(index);
        let tab_width = self.calculate_tab_width(&self.tabs[index]);

        if tab_x < self.bounds.x {
            // Tab is to the left, scroll left
            let diff = (self.bounds.x - tab_x) as usize;
            self.scroll_offset = self.scroll_offset.saturating_sub(diff);
        } else if tab_x + tab_width as isize > self.bounds.x + self.bounds.width as isize {
            // Tab is to the right, scroll right
            let diff = (tab_x + tab_width as isize - self.bounds.x - self.bounds.width as isize) as usize;
            self.scroll_offset += diff;
        }
    }

    /// Get tab at x position
    fn tab_at_x(&self, x: isize) -> Option<usize> {
        let mut tab_x = self.bounds.x - self.scroll_offset as isize;
        for (i, tab) in self.tabs.iter().enumerate() {
            let tab_width = self.calculate_tab_width(tab);
            if x >= tab_x && x < tab_x + tab_width as isize {
                return Some(i);
            }
            tab_x += tab_width as isize;
        }
        None
    }

    /// Check if x is in close button area for tab
    fn is_in_close_area(&self, x: isize, tab_index: usize) -> bool {
        if tab_index >= self.tabs.len() || !self.tabs[tab_index].closable {
            return false;
        }

        let tab_x = self.get_tab_x(tab_index);
        let tab_width = self.calculate_tab_width(&self.tabs[tab_index]);
        let close_x = tab_x + tab_width as isize - 18;

        x >= close_x && x < close_x + 14
    }

    /// Get tab bar y position
    fn tab_bar_y(&self) -> isize {
        match self.position {
            TabPosition::Top => self.bounds.y,
            TabPosition::Bottom => self.bounds.y + (self.bounds.height - self.tab_height) as isize,
        }
    }
}

// Helper drawing functions
fn draw_string(surface: &mut Surface, x: isize, y: isize, text: &str, color: Color) {
    use crate::drivers::font::DEFAULT_FONT;

    if x < 0 || y < 0 {
        return;
    }

    let mut cx = x as usize;
    let cy = y as usize;

    for c in text.chars() {
        if let Some(glyph) = DEFAULT_FONT.get_glyph(c) {
            for row in 0..DEFAULT_FONT.height {
                let byte = glyph[row];
                for col in 0..DEFAULT_FONT.width {
                    if (byte >> (DEFAULT_FONT.width - 1 - col)) & 1 != 0 {
                        surface.set_pixel(cx + col, cy + row, color);
                    }
                }
            }
        }
        cx += DEFAULT_FONT.width;
    }
}

fn fill_rect_safe(surface: &mut Surface, x: isize, y: isize, width: usize, height: usize, color: Color) {
    if x < 0 || y < 0 || width == 0 || height == 0 {
        return;
    }
    surface.fill_rect(x as usize, y as usize, width, height, color);
}

fn draw_rect_safe(surface: &mut Surface, x: isize, y: isize, width: usize, height: usize, color: Color) {
    if x < 0 || y < 0 || width == 0 || height == 0 {
        return;
    }
    surface.draw_rect(x as usize, y as usize, width, height, color);
}

fn draw_line_h(surface: &mut Surface, x: isize, y: isize, length: usize, color: Color) {
    if x < 0 || y < 0 {
        return;
    }
    for i in 0..length {
        surface.set_pixel((x as usize) + i, y as usize, color);
    }
}

fn draw_line_v(surface: &mut Surface, x: isize, y: isize, length: usize, color: Color) {
    if x < 0 || y < 0 {
        return;
    }
    for i in 0..length {
        surface.set_pixel(x as usize, (y as usize) + i, color);
    }
}

impl Widget for TabControl {
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
        } else {
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

        let tab_bar_y = self.tab_bar_y();

        match event {
            WidgetEvent::MouseMove { x, y } => {
                // Check if in tab bar area
                if *y >= tab_bar_y && *y < tab_bar_y + self.tab_height as isize {
                    self.hovered_index = self.tab_at_x(*x);
                    if let Some(idx) = self.hovered_index {
                        if self.is_in_close_area(*x, idx) {
                            self.hovered_close = Some(idx);
                        } else {
                            self.hovered_close = None;
                        }
                    } else {
                        self.hovered_close = None;
                    }
                } else {
                    self.hovered_index = None;
                    self.hovered_close = None;
                }
                return self.hovered_index.is_some();
            }
            WidgetEvent::MouseLeave => {
                self.hovered_index = None;
                self.hovered_close = None;
                return true;
            }
            WidgetEvent::MouseDown { button: MouseButton::Left, x, y } => {
                if *y >= tab_bar_y && *y < tab_bar_y + self.tab_height as isize {
                    if let Some(idx) = self.tab_at_x(*x) {
                        // Check if clicking close button
                        if self.is_in_close_area(*x, idx) {
                            let tab_id = self.tabs[idx].id;
                            let should_close = if let Some(callback) = self.on_tab_close {
                                callback(tab_id)
                            } else {
                                true
                            };
                            if should_close {
                                self.remove_tab(tab_id);
                            }
                        } else if self.tabs[idx].enabled {
                            // Select tab
                            self.set_active_index(idx);
                        }
                        return true;
                    }
                }
            }
            WidgetEvent::Scroll { delta_x, .. } => {
                let total_width = self.total_tabs_width();
                if total_width > self.bounds.width {
                    let max_scroll = total_width - self.bounds.width;
                    if *delta_x > 0 {
                        self.scroll_offset = self.scroll_offset.saturating_sub(20);
                    } else if *delta_x < 0 {
                        self.scroll_offset = (self.scroll_offset + 20).min(max_scroll);
                    }
                    return true;
                }
            }
            WidgetEvent::KeyDown { key, .. } => {
                if self.state == WidgetState::Focused {
                    match *key {
                        0x4B => { // Left arrow
                            if self.active_index > 0 {
                                // Find previous enabled tab
                                let mut idx = self.active_index - 1;
                                while idx > 0 && !self.tabs[idx].enabled {
                                    idx -= 1;
                                }
                                if self.tabs[idx].enabled {
                                    self.set_active_index(idx);
                                }
                            }
                            return true;
                        }
                        0x4D => { // Right arrow
                            if self.active_index < self.tabs.len() - 1 {
                                // Find next enabled tab
                                let mut idx = self.active_index + 1;
                                while idx < self.tabs.len() - 1 && !self.tabs[idx].enabled {
                                    idx += 1;
                                }
                                if self.tabs[idx].enabled {
                                    self.set_active_index(idx);
                                }
                            }
                            return true;
                        }
                        0x47 => { // Home
                            // Find first enabled tab
                            for (i, tab) in self.tabs.iter().enumerate() {
                                if tab.enabled {
                                    self.set_active_index(i);
                                    break;
                                }
                            }
                            return true;
                        }
                        0x4F => { // End
                            // Find last enabled tab
                            for i in (0..self.tabs.len()).rev() {
                                if self.tabs[i].enabled {
                                    self.set_active_index(i);
                                    break;
                                }
                            }
                            return true;
                        }
                        _ => {}
                    }
                }
            }
            WidgetEvent::Focus => {
                self.state = WidgetState::Focused;
                return true;
            }
            WidgetEvent::Blur => {
                self.state = WidgetState::Normal;
                return true;
            }
            _ => {}
        }
        false
    }

    fn render(&self, surface: &mut Surface) {
        if !self.visible {
            return;
        }

        let theme = theme();
        let bg = if self.enabled { theme.bg } else { theme.bg_disabled };
        let fg = if self.enabled { theme.fg } else { theme.fg_disabled };
        let border = if self.state == WidgetState::Focused { theme.border_focused } else { theme.border };
        let accent = theme.accent;

        let tab_bar_y = self.tab_bar_y();

        // Draw tab bar background
        fill_rect_safe(surface, self.bounds.x, tab_bar_y, self.bounds.width, self.tab_height, Color::new(45, 45, 52));

        // Draw content area background
        let content = self.content_bounds();
        fill_rect_safe(surface, content.x, content.y, content.width, content.height, bg);
        draw_rect_safe(surface, content.x, content.y, content.width, content.height, border);

        // Draw tabs
        let mut tab_x = self.bounds.x - self.scroll_offset as isize;

        for (i, tab) in self.tabs.iter().enumerate() {
            let tab_width = self.calculate_tab_width(tab);
            let is_active = i == self.active_index;
            let is_hovered = self.hovered_index == Some(i);
            let is_close_hovered = self.hovered_close == Some(i);

            // Skip if tab is completely out of view
            if tab_x + (tab_width as isize) < self.bounds.x {
                tab_x += tab_width as isize;
                continue;
            }
            if tab_x > self.bounds.x + (self.bounds.width as isize) {
                break;
            }

            // Tab background
            let tab_bg = if !tab.enabled {
                theme.bg_disabled
            } else if is_active {
                bg
            } else if is_hovered {
                theme.bg_hover
            } else {
                Color::new(50, 50, 58)
            };

            match self.style {
                TabStyle::Standard => {
                    // Draw tab with borders
                    let tab_y = if self.position == TabPosition::Top {
                        tab_bar_y + 4
                    } else {
                        tab_bar_y
                    };
                    let actual_height = if is_active {
                        self.tab_height - 4
                    } else {
                        self.tab_height - 6
                    };

                    fill_rect_safe(surface, tab_x, tab_y, tab_width, actual_height, tab_bg);

                    // Tab borders (top, left, right)
                    if self.position == TabPosition::Top {
                        draw_line_h(surface, tab_x, tab_y, tab_width, border);
                        draw_line_v(surface, tab_x, tab_y, actual_height, border);
                        draw_line_v(surface, tab_x + tab_width as isize - 1, tab_y, actual_height, border);

                        if is_active {
                            // Active tab covers the content border
                            draw_line_h(surface, tab_x + 1, tab_bar_y + self.tab_height as isize - 1, tab_width - 2, bg);
                        }
                    } else {
                        let tab_bottom = tab_y + actual_height as isize - 1;
                        draw_line_h(surface, tab_x, tab_bottom, tab_width, border);
                        draw_line_v(surface, tab_x, tab_y, actual_height, border);
                        draw_line_v(surface, tab_x + tab_width as isize - 1, tab_y, actual_height, border);

                        if is_active {
                            draw_line_h(surface, tab_x + 1, tab_y, tab_width - 2, bg);
                        }
                    }
                }
                TabStyle::Pill => {
                    // Rounded pill tabs
                    let tab_y = tab_bar_y + 4;
                    let pill_height = self.tab_height - 8;

                    if is_active {
                        fill_rect_safe(surface, tab_x + 2, tab_y, tab_width - 4, pill_height, accent);
                    } else if is_hovered {
                        fill_rect_safe(surface, tab_x + 2, tab_y, tab_width - 4, pill_height, theme.bg_hover);
                    }
                }
                TabStyle::Underline => {
                    // Underline style tabs
                    if is_active {
                        let underline_y = if self.position == TabPosition::Top {
                            tab_bar_y + self.tab_height as isize - 3
                        } else {
                            tab_bar_y + 1
                        };
                        fill_rect_safe(surface, tab_x + 4, underline_y, tab_width - 8, 3, accent);
                    }
                }
                TabStyle::Flat => {
                    // Just a background color for active/hovered
                    if is_active || is_hovered {
                        let tab_y = tab_bar_y + 2;
                        fill_rect_safe(surface, tab_x, tab_y, tab_width, self.tab_height - 4, tab_bg);
                    }
                }
            }

            // Draw tab content
            let text_y = tab_bar_y + (self.tab_height / 2) as isize - 6;
            let text_color = if !tab.enabled {
                theme.fg_disabled
            } else if is_active && self.style == TabStyle::Pill {
                Color::WHITE
            } else {
                fg
            };

            let mut content_x = tab_x + self.tab_padding as isize;

            // Draw icon if present
            if let Some(icon_char) = tab.icon {
                draw_string(surface, content_x, text_y, &alloc::string::String::from(icon_char), text_color);
                content_x += 12;
            }

            // Draw label
            draw_string(surface, content_x, text_y, &tab.label, text_color);

            // Draw close button if closable
            if tab.closable {
                let close_x = tab_x + tab_width as isize - 18;
                let close_y = tab_bar_y + (self.tab_height / 2) as isize - 5;

                let close_color = if is_close_hovered {
                    Color::new(255, 100, 100)
                } else {
                    Color::new(150, 150, 150)
                };

                // Draw X
                for i in 0..10 {
                    surface.set_pixel((close_x + i) as usize, (close_y + i) as usize, close_color);
                    surface.set_pixel((close_x + 9 - i) as usize, (close_y + i) as usize, close_color);
                }
            }

            tab_x += tab_width as isize;
        }

        // Draw scroll indicators if needed
        let total_width = self.total_tabs_width();
        if total_width > self.bounds.width {
            // Left arrow if scrolled
            if self.scroll_offset > 0 {
                let arrow_x = self.bounds.x + 4;
                let arrow_y = tab_bar_y + (self.tab_height / 2) as isize;
                for i in 0..6 {
                    draw_line_v(surface, arrow_x + i, arrow_y - i, 1 + i as usize * 2, Color::new(180, 180, 180));
                }
            }

            // Right arrow if more tabs
            if self.scroll_offset < total_width - self.bounds.width {
                let arrow_x = self.bounds.x + self.bounds.width as isize - 10;
                let arrow_y = tab_bar_y + (self.tab_height / 2) as isize;
                for i in 0..6 {
                    let ii = 5 - i;
                    draw_line_v(surface, arrow_x + ii, arrow_y - ii, 1 + ii as usize * 2, Color::new(180, 180, 180));
                }
            }
        }
    }
}
