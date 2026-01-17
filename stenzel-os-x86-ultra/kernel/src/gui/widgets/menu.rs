//! Menu widgets
//!
//! Menu bar, menu items, and context menus.

use alloc::string::String;
use alloc::vec::Vec;
use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use super::{Widget, WidgetId, WidgetState, WidgetEvent, Bounds, MouseButton, theme};

/// Menu item action callback
pub type MenuCallback = fn(WidgetId, &str);

/// Menu item type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuItemType {
    /// Normal clickable item
    Normal,
    /// Separator line
    Separator,
    /// Submenu
    Submenu,
    /// Checkbox item
    Checkbox { checked: bool },
}

/// A menu item
#[derive(Debug, Clone)]
pub struct MenuItem {
    pub id: String,
    pub label: String,
    pub shortcut: Option<String>,
    pub item_type: MenuItemType,
    pub enabled: bool,
    pub submenu: Option<Vec<MenuItem>>,
}

impl MenuItem {
    /// Create a normal menu item
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: String::from(id),
            label: String::from(label),
            shortcut: None,
            item_type: MenuItemType::Normal,
            enabled: true,
            submenu: None,
        }
    }

    /// Create a menu item with shortcut
    pub fn with_shortcut(id: &str, label: &str, shortcut: &str) -> Self {
        Self {
            id: String::from(id),
            label: String::from(label),
            shortcut: Some(String::from(shortcut)),
            item_type: MenuItemType::Normal,
            enabled: true,
            submenu: None,
        }
    }

    /// Create a separator
    pub fn separator() -> Self {
        Self {
            id: String::from("separator"),
            label: String::new(),
            shortcut: None,
            item_type: MenuItemType::Separator,
            enabled: true,
            submenu: None,
        }
    }

    /// Create a submenu
    pub fn submenu(id: &str, label: &str, items: Vec<MenuItem>) -> Self {
        Self {
            id: String::from(id),
            label: String::from(label),
            shortcut: None,
            item_type: MenuItemType::Submenu,
            enabled: true,
            submenu: Some(items),
        }
    }

    /// Create a checkbox item
    pub fn checkbox(id: &str, label: &str, checked: bool) -> Self {
        Self {
            id: String::from(id),
            label: String::from(label),
            shortcut: None,
            item_type: MenuItemType::Checkbox { checked },
            enabled: true,
            submenu: None,
        }
    }

    /// Set enabled state
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Toggle checkbox
    pub fn toggle_checkbox(&mut self) {
        if let MenuItemType::Checkbox { ref mut checked } = self.item_type {
            *checked = !*checked;
        }
    }
}

/// A menu (collection of items with position)
pub struct Menu {
    id: WidgetId,
    bounds: Bounds,
    items: Vec<MenuItem>,
    hover_index: Option<usize>,
    open_submenu: Option<usize>,
    visible: bool,
    on_select: Option<MenuCallback>,
}

impl Menu {
    const ITEM_HEIGHT: usize = 24;
    const SEPARATOR_HEIGHT: usize = 9;
    const PADDING_X: usize = 8;

    /// Create a new menu
    pub fn new(x: isize, y: isize) -> Self {
        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(x, y, 0, 0),
            items: Vec::new(),
            hover_index: None,
            open_submenu: None,
            visible: false,
            on_select: None,
        }
    }

    /// Add an item
    pub fn add_item(&mut self, item: MenuItem) {
        self.items.push(item);
        self.recalculate_size();
    }

    /// Set items
    pub fn set_items(&mut self, items: Vec<MenuItem>) {
        self.items = items;
        self.recalculate_size();
    }

    /// Clear items
    pub fn clear(&mut self) {
        self.items.clear();
        self.recalculate_size();
    }

    /// Set position
    pub fn set_position(&mut self, x: isize, y: isize) {
        self.bounds.x = x;
        self.bounds.y = y;
    }

    /// Show the menu
    pub fn show(&mut self) {
        self.visible = true;
        self.hover_index = None;
        self.open_submenu = None;
    }

    /// Hide the menu
    pub fn hide(&mut self) {
        self.visible = false;
        self.hover_index = None;
        self.open_submenu = None;
    }

    /// Is the menu visible?
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Set callback
    pub fn set_on_select(&mut self, callback: MenuCallback) {
        self.on_select = Some(callback);
    }

    /// Recalculate menu size based on items
    fn recalculate_size(&mut self) {
        let mut max_width = 100;
        let mut total_height = 4; // Top/bottom padding

        for item in &self.items {
            if item.item_type == MenuItemType::Separator {
                total_height += Self::SEPARATOR_HEIGHT;
            } else {
                total_height += Self::ITEM_HEIGHT;

                // Calculate item width
                let label_width = item.label.chars().count() * 8;
                let shortcut_width = item.shortcut.as_ref()
                    .map(|s| s.chars().count() * 8 + 32)
                    .unwrap_or(0);
                let arrow_width = if item.item_type == MenuItemType::Submenu { 16 } else { 0 };
                let item_width = label_width + shortcut_width + arrow_width + Self::PADDING_X * 2 + 24;

                max_width = max_width.max(item_width);
            }
        }

        self.bounds.width = max_width;
        self.bounds.height = total_height;
    }

    /// Get item at y position
    fn item_at_y(&self, y: isize) -> Option<usize> {
        let mut current_y = self.bounds.y + 2;

        for (i, item) in self.items.iter().enumerate() {
            let item_height = if item.item_type == MenuItemType::Separator {
                Self::SEPARATOR_HEIGHT
            } else {
                Self::ITEM_HEIGHT
            };

            if y >= current_y && y < current_y + item_height as isize {
                return Some(i);
            }
            current_y += item_height as isize;
        }
        None
    }

    /// Handle mouse move
    pub fn handle_mouse_move(&mut self, x: isize, y: isize) -> bool {
        if !self.visible || !self.bounds.contains(x, y) {
            return false;
        }

        self.hover_index = self.item_at_y(y);

        // Open submenu on hover
        if let Some(index) = self.hover_index {
            if let Some(item) = self.items.get(index) {
                if item.item_type == MenuItemType::Submenu {
                    self.open_submenu = Some(index);
                } else {
                    self.open_submenu = None;
                }
            }
        }
        true
    }

    /// Handle click
    pub fn handle_click(&mut self, x: isize, y: isize) -> Option<String> {
        if !self.visible || !self.bounds.contains(x, y) {
            return None;
        }

        if let Some(index) = self.item_at_y(y) {
            if let Some(item) = self.items.get_mut(index) {
                if !item.enabled || item.item_type == MenuItemType::Separator {
                    return None;
                }

                if item.item_type == MenuItemType::Submenu {
                    // Don't close on submenu click
                    return None;
                }

                // Toggle checkbox
                item.toggle_checkbox();

                // Notify callback
                if let Some(callback) = self.on_select {
                    callback(self.id, &item.id);
                }

                let id = item.id.clone();
                self.hide();
                return Some(id);
            }
        }
        None
    }

    /// Render the menu
    pub fn render(&self, surface: &mut Surface) {
        if !self.visible {
            return;
        }

        let theme = theme();
        let x = self.bounds.x.max(0) as usize;
        let y = self.bounds.y.max(0) as usize;
        let w = self.bounds.width;
        let h = self.bounds.height;

        let bg_color = Color::new(50, 50, 58);
        let border_color = Color::new(70, 70, 78);

        // Draw background with shadow
        for py in 2..h + 2 {
            for px in 2..w + 2 {
                surface.set_pixel(x + px, y + py, Color::new(0, 0, 0));
            }
        }

        // Draw background
        for py in 0..h {
            for px in 0..w {
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

        // Draw items
        let mut current_y = y + 2;

        for (i, item) in self.items.iter().enumerate() {
            if item.item_type == MenuItemType::Separator {
                // Draw separator line
                let sep_y = current_y + Self::SEPARATOR_HEIGHT / 2;
                for px in 4..w - 4 {
                    surface.set_pixel(x + px, sep_y, Color::new(70, 70, 78));
                }
                current_y += Self::SEPARATOR_HEIGHT;
                continue;
            }

            // Draw item background
            let is_hover = self.hover_index == Some(i);
            if is_hover && item.enabled {
                for py in 0..Self::ITEM_HEIGHT {
                    for px in 1..w - 1 {
                        surface.set_pixel(x + px, current_y + py, theme.accent);
                    }
                }
            }

            // Draw checkbox mark
            if let MenuItemType::Checkbox { checked } = item.item_type {
                if checked {
                    let check_x = x + 8;
                    let check_y = current_y + (Self::ITEM_HEIGHT - 16) / 2;
                    // Simple checkmark
                    for i in 0..3 {
                        surface.set_pixel(check_x + i, check_y + 6 + i, theme.fg);
                    }
                    for i in 0..5 {
                        surface.set_pixel(check_x + 2 + i, check_y + 8 - i, theme.fg);
                    }
                }
            }

            // Draw label
            let text_color = if !item.enabled {
                theme.fg_disabled
            } else if is_hover {
                Color::WHITE
            } else {
                theme.fg
            };

            let label_x = x + Self::PADDING_X + 16;
            let label_y = current_y + (Self::ITEM_HEIGHT - 16) / 2;

            for (j, c) in item.label.chars().enumerate() {
                draw_char_simple(surface, label_x + j * 8, label_y, c, text_color);
            }

            // Draw shortcut
            if let Some(ref shortcut) = item.shortcut {
                let shortcut_x = x + w - Self::PADDING_X - shortcut.chars().count() * 8 - 8;
                let shortcut_color = if !item.enabled {
                    theme.fg_disabled
                } else {
                    Color::new(128, 128, 136)
                };

                for (j, c) in shortcut.chars().enumerate() {
                    draw_char_simple(surface, shortcut_x + j * 8, label_y, c, shortcut_color);
                }
            }

            // Draw submenu arrow
            if item.item_type == MenuItemType::Submenu {
                let arrow_x = x + w - Self::PADDING_X - 8;
                let arrow_y = current_y + Self::ITEM_HEIGHT / 2;
                for i in 0..5 {
                    surface.set_pixel(arrow_x + i, arrow_y - i, text_color);
                    surface.set_pixel(arrow_x + i, arrow_y + i, text_color);
                }
            }

            current_y += Self::ITEM_HEIGHT;
        }
    }
}

/// Menu bar widget
pub struct MenuBar {
    id: WidgetId,
    bounds: Bounds,
    items: Vec<(String, String, Menu)>, // (id, label, menu)
    active_index: Option<usize>,
    hover_index: Option<usize>,
    state: WidgetState,
    enabled: bool,
    visible: bool,
}

impl MenuBar {
    const HEIGHT: usize = 24;

    /// Create a new menu bar
    pub fn new(x: isize, y: isize, width: usize) -> Self {
        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(x, y, width, Self::HEIGHT),
            items: Vec::new(),
            active_index: None,
            hover_index: None,
            state: WidgetState::Normal,
            enabled: true,
            visible: true,
        }
    }

    /// Add a menu
    pub fn add_menu(&mut self, id: &str, label: &str, items: Vec<MenuItem>) {
        let mut menu = Menu::new(0, self.bounds.y + Self::HEIGHT as isize);
        menu.set_items(items);
        self.items.push((String::from(id), String::from(label), menu));
    }

    /// Get menu by index
    pub fn get_menu(&mut self, index: usize) -> Option<&mut Menu> {
        self.items.get_mut(index).map(|(_, _, m)| m)
    }

    /// Close all menus
    pub fn close_menus(&mut self) {
        self.active_index = None;
        for (_, _, menu) in &mut self.items {
            menu.hide();
        }
    }

    /// Get menu item position
    fn item_position(&self, index: usize) -> (isize, usize) {
        let mut x = self.bounds.x + 8;
        for i in 0..index {
            if let Some((_, label, _)) = self.items.get(i) {
                x += (label.chars().count() * 8 + 16) as isize;
            }
        }
        let width = self.items.get(index)
            .map(|(_, label, _)| label.chars().count() * 8 + 16)
            .unwrap_or(0);
        (x, width)
    }

    /// Get item at x position
    fn item_at_x(&self, x: isize) -> Option<usize> {
        let mut current_x = self.bounds.x + 8;

        for (i, (_, label, _)) in self.items.iter().enumerate() {
            let item_width = label.chars().count() * 8 + 16;
            if x >= current_x && x < current_x + item_width as isize {
                return Some(i);
            }
            current_x += item_width as isize;
        }
        None
    }
}

impl Widget for MenuBar {
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
            self.close_menus();
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
            self.close_menus();
        }
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
                if self.active_index.is_none() {
                    self.state = WidgetState::Normal;
                    self.hover_index = None;
                }
                true
            }
            WidgetEvent::MouseMove { x, y } => {
                // Check if in menu bar
                if self.bounds.contains(*x, *y) {
                    self.hover_index = self.item_at_x(*x);

                    // If a menu is open, switch to hovered menu
                    if self.active_index.is_some() {
                        if let Some(hover) = self.hover_index {
                            if self.active_index != Some(hover) {
                                // Close current menu
                                if let Some(idx) = self.active_index {
                                    if let Some((_, _, menu)) = self.items.get_mut(idx) {
                                        menu.hide();
                                    }
                                }
                                // Open new menu
                                let (menu_x, _) = self.item_position(hover);
                                if let Some((_, _, menu)) = self.items.get_mut(hover) {
                                    menu.set_position(menu_x, self.bounds.y + Self::HEIGHT as isize);
                                    menu.show();
                                }
                                self.active_index = Some(hover);
                            }
                        }
                    }
                    return true;
                }

                // Check if in open menu
                if let Some(idx) = self.active_index {
                    if let Some((_, _, menu)) = self.items.get_mut(idx) {
                        if menu.handle_mouse_move(*x, *y) {
                            return true;
                        }
                    }
                }
                false
            }
            WidgetEvent::MouseDown { button: MouseButton::Left, x, y } => {
                // Check if in menu bar
                if self.bounds.contains(*x, *y) {
                    if let Some(index) = self.item_at_x(*x) {
                        if self.active_index == Some(index) {
                            // Close menu if clicking same item
                            self.close_menus();
                        } else {
                            // Close current and open new
                            self.close_menus();
                            let (menu_x, _) = self.item_position(index);
                            if let Some((_, _, menu)) = self.items.get_mut(index) {
                                menu.set_position(menu_x, self.bounds.y + Self::HEIGHT as isize);
                                menu.show();
                            }
                            self.active_index = Some(index);
                        }
                    }
                    return true;
                }

                // Check if in open menu
                if let Some(idx) = self.active_index {
                    if let Some((_, _, menu)) = self.items.get_mut(idx) {
                        if menu.bounds.contains(*x, *y) {
                            menu.handle_click(*x, *y);
                            self.active_index = None;
                            return true;
                        }
                    }
                }

                // Click outside - close menus
                self.close_menus();
                false
            }
            WidgetEvent::Blur => {
                self.close_menus();
                self.state = WidgetState::Normal;
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

        let bg_color = Color::new(45, 45, 53);

        // Draw background
        for py in 0..h {
            for px in 0..w {
                surface.set_pixel(x + px, y + py, bg_color);
            }
        }

        // Draw bottom border
        for px in 0..w {
            surface.set_pixel(x + px, y + h - 1, theme.border);
        }

        // Draw menu items
        let mut current_x = x + 8;

        for (i, (_, label, _)) in self.items.iter().enumerate() {
            let item_width = label.chars().count() * 8 + 16;
            let is_active = self.active_index == Some(i);
            let is_hover = self.hover_index == Some(i);

            // Draw item background
            if is_active || is_hover {
                let bg = if is_active { theme.accent } else { theme.bg_hover };
                for py in 2..h - 2 {
                    for px in 0..item_width {
                        surface.set_pixel(current_x + px, y + py, bg);
                    }
                }
            }

            // Draw label
            let text_color = if is_active { Color::WHITE } else { theme.fg };
            let label_x = current_x + 8;
            let label_y = y + (h - 16) / 2;

            for (j, c) in label.chars().enumerate() {
                draw_char_simple(surface, label_x + j * 8, label_y, c, text_color);
            }

            current_x += item_width;
        }

        // Render open menu
        if let Some(idx) = self.active_index {
            if let Some((_, _, menu)) = self.items.get(idx) {
                menu.render(surface);
            }
        }
    }
}

/// Context menu (right-click menu)
pub struct ContextMenu {
    menu: Menu,
}

impl ContextMenu {
    /// Create a new context menu
    pub fn new() -> Self {
        Self {
            menu: Menu::new(0, 0),
        }
    }

    /// Set items
    pub fn set_items(&mut self, items: Vec<MenuItem>) {
        self.menu.set_items(items);
    }

    /// Show at position
    pub fn show_at(&mut self, x: isize, y: isize) {
        self.menu.set_position(x, y);
        self.menu.show();
    }

    /// Hide
    pub fn hide(&mut self) {
        self.menu.hide();
    }

    /// Is visible?
    pub fn is_visible(&self) -> bool {
        self.menu.is_visible()
    }

    /// Get bounds
    pub fn bounds(&self) -> Bounds {
        self.menu.bounds
    }

    /// Set callback
    pub fn set_on_select(&mut self, callback: MenuCallback) {
        self.menu.set_on_select(callback);
    }

    /// Handle mouse move
    pub fn handle_mouse_move(&mut self, x: isize, y: isize) -> bool {
        self.menu.handle_mouse_move(x, y)
    }

    /// Handle click
    pub fn handle_click(&mut self, x: isize, y: isize) -> Option<String> {
        self.menu.handle_click(x, y)
    }

    /// Render
    pub fn render(&self, surface: &mut Surface) {
        self.menu.render(surface);
    }
}

impl Default for ContextMenu {
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
