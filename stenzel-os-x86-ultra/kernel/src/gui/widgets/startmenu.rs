//! Start Menu widget
//!
//! Windows-style start menu for launching applications.

use alloc::string::String;
use alloc::vec::Vec;
use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use super::{Widget, WidgetId, WidgetState, WidgetEvent, Bounds, MouseButton, theme};

/// Start menu item type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartMenuItemType {
    /// Normal application item
    Application,
    /// Folder/submenu
    Folder,
    /// Separator
    Separator,
    /// Special item (shutdown, settings, etc.)
    Special,
}

/// Start menu item
#[derive(Debug, Clone)]
pub struct StartMenuItem {
    pub id: String,
    pub label: String,
    pub item_type: StartMenuItemType,
    pub icon_index: Option<usize>,
    pub command: Option<String>,
    pub children: Option<Vec<StartMenuItem>>,
}

impl StartMenuItem {
    /// Create application item
    pub fn application(id: &str, label: &str, command: &str) -> Self {
        Self {
            id: String::from(id),
            label: String::from(label),
            item_type: StartMenuItemType::Application,
            icon_index: None,
            command: Some(String::from(command)),
            children: None,
        }
    }

    /// Create folder item
    pub fn folder(id: &str, label: &str, children: Vec<StartMenuItem>) -> Self {
        Self {
            id: String::from(id),
            label: String::from(label),
            item_type: StartMenuItemType::Folder,
            icon_index: None,
            command: None,
            children: Some(children),
        }
    }

    /// Create separator
    pub fn separator() -> Self {
        Self {
            id: String::from("separator"),
            label: String::new(),
            item_type: StartMenuItemType::Separator,
            icon_index: None,
            command: None,
            children: None,
        }
    }

    /// Create special item (shutdown, settings, etc.)
    pub fn special(id: &str, label: &str) -> Self {
        Self {
            id: String::from(id),
            label: String::from(label),
            item_type: StartMenuItemType::Special,
            icon_index: None,
            command: None,
            children: None,
        }
    }

    /// Set icon index
    pub fn with_icon(mut self, icon_index: usize) -> Self {
        self.icon_index = Some(icon_index);
        self
    }
}

/// Start menu callback
pub type StartMenuCallback = fn(WidgetId, &str, Option<&str>);

/// Start menu widget
pub struct StartMenu {
    id: WidgetId,
    bounds: Bounds,
    items: Vec<StartMenuItem>,
    pinned_items: Vec<StartMenuItem>,
    recent_items: Vec<StartMenuItem>,
    hover_index: Option<usize>,
    hover_section: MenuSection,
    open_submenu: Option<usize>,
    submenu_bounds: Option<Bounds>,
    submenu_hover: Option<usize>,
    visible: bool,
    on_item_click: Option<StartMenuCallback>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuSection {
    Pinned,
    AllApps,
    Footer,
    Submenu,
}

impl StartMenu {
    const WIDTH: usize = 320;
    const PINNED_HEIGHT: usize = 200;
    const FOOTER_HEIGHT: usize = 60;
    const ITEM_HEIGHT: usize = 36;
    const SEPARATOR_HEIGHT: usize = 9;
    const ICON_SIZE: usize = 24;
    const SUBMENU_WIDTH: usize = 250;

    /// Create a new start menu
    pub fn new(screen_height: usize) -> Self {
        let height = screen_height.min(600);

        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(0, (screen_height - height) as isize, Self::WIDTH, height),
            items: Vec::new(),
            pinned_items: Vec::new(),
            recent_items: Vec::new(),
            hover_index: None,
            hover_section: MenuSection::Pinned,
            open_submenu: None,
            submenu_bounds: None,
            submenu_hover: None,
            visible: false,
            on_item_click: None,
        }
    }

    /// Set all apps items
    pub fn set_items(&mut self, items: Vec<StartMenuItem>) {
        self.items = items;
    }

    /// Add an item
    pub fn add_item(&mut self, item: StartMenuItem) {
        self.items.push(item);
    }

    /// Set pinned items
    pub fn set_pinned(&mut self, items: Vec<StartMenuItem>) {
        self.pinned_items = items;
    }

    /// Add pinned item
    pub fn add_pinned(&mut self, item: StartMenuItem) {
        self.pinned_items.push(item);
    }

    /// Set recent items
    pub fn set_recent(&mut self, items: Vec<StartMenuItem>) {
        self.recent_items = items;
    }

    /// Set click callback
    pub fn set_on_item_click(&mut self, callback: StartMenuCallback) {
        self.on_item_click = Some(callback);
    }

    /// Toggle visibility
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        if !self.visible {
            self.close_submenu();
        }
    }

    /// Show
    pub fn show(&mut self) {
        self.visible = true;
    }

    /// Hide
    pub fn hide(&mut self) {
        self.visible = false;
        self.close_submenu();
    }

    /// Is visible?
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    fn close_submenu(&mut self) {
        self.open_submenu = None;
        self.submenu_bounds = None;
        self.submenu_hover = None;
    }

    /// Get section and index at position
    fn hit_test(&self, x: isize, y: isize) -> Option<(MenuSection, usize)> {
        // Check submenu first
        if let Some(ref submenu_bounds) = self.submenu_bounds {
            if submenu_bounds.contains(x, y) {
                let rel_y = (y - submenu_bounds.y) as usize;
                let index = rel_y / Self::ITEM_HEIGHT;
                return Some((MenuSection::Submenu, index));
            }
        }

        if !self.bounds.contains(x, y) {
            return None;
        }

        let rel_y = (y - self.bounds.y) as usize;

        // Pinned section
        if rel_y < Self::PINNED_HEIGHT {
            let index = rel_y / Self::ITEM_HEIGHT;
            if index < self.pinned_items.len() {
                return Some((MenuSection::Pinned, index));
            }
        }

        // All apps section
        let apps_start = Self::PINNED_HEIGHT;
        let apps_end = self.bounds.height - Self::FOOTER_HEIGHT;
        if rel_y >= apps_start && rel_y < apps_end {
            let index = (rel_y - apps_start) / Self::ITEM_HEIGHT;
            if index < self.items.len() {
                return Some((MenuSection::AllApps, index));
            }
        }

        // Footer section
        if rel_y >= self.bounds.height - Self::FOOTER_HEIGHT {
            let footer_y = rel_y - (self.bounds.height - Self::FOOTER_HEIGHT);
            let index = footer_y / (Self::FOOTER_HEIGHT / 3);
            return Some((MenuSection::Footer, index));
        }

        None
    }

    fn notify_click(&self, id: &str, command: Option<&str>) {
        if let Some(callback) = self.on_item_click {
            callback(self.id, id, command);
        }
    }
}

impl Widget for StartMenu {
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
        if !visible {
            self.close_submenu();
        }
    }

    fn handle_event(&mut self, event: &WidgetEvent) -> bool {
        if !self.visible {
            return false;
        }

        match event {
            WidgetEvent::MouseMove { x, y } => {
                if let Some((section, index)) = self.hit_test(*x, *y) {
                    self.hover_section = section;

                    if section == MenuSection::Submenu {
                        self.submenu_hover = Some(index);
                    } else {
                        self.hover_index = Some(index);
                        self.submenu_hover = None;

                        // Open submenu on hover
                        if section == MenuSection::AllApps {
                            if let Some(item) = self.items.get(index) {
                                if item.item_type == StartMenuItemType::Folder {
                                    self.open_submenu = Some(index);
                                    // Calculate submenu position
                                    let submenu_y = self.bounds.y + Self::PINNED_HEIGHT as isize + (index * Self::ITEM_HEIGHT) as isize;
                                    let submenu_height = item.children.as_ref().map(|c| c.len() * Self::ITEM_HEIGHT).unwrap_or(0);
                                    self.submenu_bounds = Some(Bounds::new(
                                        self.bounds.x + self.bounds.width as isize,
                                        submenu_y,
                                        Self::SUBMENU_WIDTH,
                                        submenu_height,
                                    ));
                                } else {
                                    self.close_submenu();
                                }
                            }
                        } else {
                            self.close_submenu();
                        }
                    }
                    return true;
                }
                self.hover_index = None;
                false
            }
            WidgetEvent::MouseDown { button: MouseButton::Left, x, y } => {
                if let Some((section, index)) = self.hit_test(*x, *y) {
                    match section {
                        MenuSection::Pinned => {
                            if let Some(item) = self.pinned_items.get(index) {
                                if item.item_type != StartMenuItemType::Separator {
                                    self.notify_click(&item.id, item.command.as_deref());
                                    self.hide();
                                }
                            }
                        }
                        MenuSection::AllApps => {
                            if let Some(item) = self.items.get(index) {
                                if item.item_type == StartMenuItemType::Application {
                                    self.notify_click(&item.id, item.command.as_deref());
                                    self.hide();
                                }
                                // Folders open on hover, not click
                            }
                        }
                        MenuSection::Footer => {
                            let footer_items = ["settings", "power", "user"];
                            if let Some(id) = footer_items.get(index) {
                                self.notify_click(id, None);
                                if *id != "settings" {
                                    self.hide();
                                }
                            }
                        }
                        MenuSection::Submenu => {
                            if let Some(parent_idx) = self.open_submenu {
                                if let Some(parent) = self.items.get(parent_idx) {
                                    if let Some(ref children) = parent.children {
                                        if let Some(item) = children.get(index) {
                                            if item.item_type != StartMenuItemType::Separator {
                                                self.notify_click(&item.id, item.command.as_deref());
                                                self.hide();
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    return true;
                }
                false
            }
            WidgetEvent::Blur => {
                self.hide();
                true
            }
            WidgetEvent::KeyDown { key: 0x01, .. } => { // Escape
                self.hide();
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

        let bg_color = Color::new(30, 30, 38);
        let hover_color = Color::new(50, 50, 60);
        let accent = theme.accent;
        let border_color = Color::new(60, 60, 70);

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
            surface.set_pixel(x + w - 1, y + py, border_color);
        }

        // Draw pinned section header
        let header_y = y + 10;
        for (i, c) in "Pinned".chars().enumerate() {
            draw_char_simple(surface, x + 15 + i * 8, header_y, c, Color::new(150, 150, 160));
        }

        // Draw pinned items
        let mut item_y = y + 30;
        for (i, item) in self.pinned_items.iter().enumerate() {
            if item.item_type == StartMenuItemType::Separator {
                // Draw separator
                for px in 10..w - 10 {
                    surface.set_pixel(x + px, item_y + Self::SEPARATOR_HEIGHT / 2, border_color);
                }
                item_y += Self::SEPARATOR_HEIGHT;
                continue;
            }

            // Hover highlight
            if self.hover_section == MenuSection::Pinned && self.hover_index == Some(i) {
                for py in 0..Self::ITEM_HEIGHT {
                    for px in 5..w - 5 {
                        surface.set_pixel(x + px, item_y + py, hover_color);
                    }
                }
            }

            // Draw icon placeholder
            let icon_x = x + 15;
            let icon_y = item_y + (Self::ITEM_HEIGHT - Self::ICON_SIZE) / 2;
            for py in 0..Self::ICON_SIZE {
                for px in 0..Self::ICON_SIZE {
                    surface.set_pixel(icon_x + px, icon_y + py, accent);
                }
            }

            // Draw label
            let label_x = x + 50;
            let label_y = item_y + (Self::ITEM_HEIGHT - 16) / 2;
            for (j, c) in item.label.chars().take(25).enumerate() {
                draw_char_simple(surface, label_x + j * 8, label_y, c, theme.fg);
            }

            item_y += Self::ITEM_HEIGHT;
        }

        // Draw separator between pinned and all apps
        let sep_y = y + Self::PINNED_HEIGHT - 5;
        for px in 10..w - 10 {
            surface.set_pixel(x + px, sep_y, border_color);
        }

        // Draw "All apps" header
        let all_apps_y = y + Self::PINNED_HEIGHT + 5;
        for (i, c) in "All apps".chars().enumerate() {
            draw_char_simple(surface, x + 15 + i * 8, all_apps_y, c, Color::new(150, 150, 160));
        }

        // Draw all apps items
        item_y = y + Self::PINNED_HEIGHT + 25;
        let max_items = (h - Self::PINNED_HEIGHT - Self::FOOTER_HEIGHT - 25) / Self::ITEM_HEIGHT;
        for (i, item) in self.items.iter().take(max_items).enumerate() {
            if item.item_type == StartMenuItemType::Separator {
                for px in 10..w - 10 {
                    surface.set_pixel(x + px, item_y + Self::SEPARATOR_HEIGHT / 2, border_color);
                }
                item_y += Self::SEPARATOR_HEIGHT;
                continue;
            }

            // Hover highlight
            if self.hover_section == MenuSection::AllApps && self.hover_index == Some(i) {
                for py in 0..Self::ITEM_HEIGHT {
                    for px in 5..w - 5 {
                        surface.set_pixel(x + px, item_y + py, hover_color);
                    }
                }
            }

            // Draw icon placeholder
            let icon_x = x + 15;
            let icon_y = item_y + (Self::ITEM_HEIGHT - Self::ICON_SIZE) / 2;
            let icon_color = if item.item_type == StartMenuItemType::Folder {
                Color::new(255, 200, 80)
            } else {
                accent
            };
            for py in 0..Self::ICON_SIZE {
                for px in 0..Self::ICON_SIZE {
                    surface.set_pixel(icon_x + px, icon_y + py, icon_color);
                }
            }

            // Draw label
            let label_x = x + 50;
            let label_y = item_y + (Self::ITEM_HEIGHT - 16) / 2;
            for (j, c) in item.label.chars().take(25).enumerate() {
                draw_char_simple(surface, label_x + j * 8, label_y, c, theme.fg);
            }

            // Draw submenu arrow for folders
            if item.item_type == StartMenuItemType::Folder {
                let arrow_x = x + w - 20;
                let arrow_y = item_y + Self::ITEM_HEIGHT / 2;
                for i in 0..5 {
                    surface.set_pixel(arrow_x + i, arrow_y - i, theme.fg);
                    surface.set_pixel(arrow_x + i, arrow_y + i, theme.fg);
                }
            }

            item_y += Self::ITEM_HEIGHT;
        }

        // Draw footer separator
        let footer_sep_y = y + h - Self::FOOTER_HEIGHT;
        for px in 0..w {
            surface.set_pixel(x + px, footer_sep_y, border_color);
        }

        // Draw footer items (user, settings, power)
        let footer_y = y + h - Self::FOOTER_HEIGHT + 10;
        let footer_items = [("User", 0), ("Settings", 1), ("Power", 2)];

        for (label, idx) in footer_items.iter() {
            let item_y = footer_y + idx * 18;

            // Hover highlight
            if self.hover_section == MenuSection::Footer && self.hover_index == Some(*idx) {
                for py in 0..18 {
                    for px in 5..w - 5 {
                        surface.set_pixel(x + px, item_y + py, hover_color);
                    }
                }
            }

            // Draw label
            for (j, c) in label.chars().enumerate() {
                draw_char_simple(surface, x + 15 + j * 8, item_y + 2, c, theme.fg);
            }
        }

        // Draw submenu if open
        if let Some(ref bounds) = self.submenu_bounds {
            if let Some(parent_idx) = self.open_submenu {
                if let Some(parent) = self.items.get(parent_idx) {
                    if let Some(ref children) = parent.children {
                        let sx = bounds.x.max(0) as usize;
                        let sy = bounds.y.max(0) as usize;
                        let sw = bounds.width;
                        let sh = bounds.height;

                        // Background
                        for py in 0..sh {
                            for px in 0..sw {
                                surface.set_pixel(sx + px, sy + py, bg_color);
                            }
                        }

                        // Border
                        for px in 0..sw {
                            surface.set_pixel(sx + px, sy, border_color);
                            surface.set_pixel(sx + px, sy + sh.saturating_sub(1), border_color);
                        }
                        for py in 0..sh {
                            surface.set_pixel(sx, sy + py, border_color);
                            surface.set_pixel(sx + sw - 1, sy + py, border_color);
                        }

                        // Items
                        let mut sub_item_y = sy;
                        for (i, child) in children.iter().enumerate() {
                            if child.item_type == StartMenuItemType::Separator {
                                for px in 5..sw - 5 {
                                    surface.set_pixel(sx + px, sub_item_y + Self::SEPARATOR_HEIGHT / 2, border_color);
                                }
                                sub_item_y += Self::SEPARATOR_HEIGHT;
                                continue;
                            }

                            // Hover
                            if self.submenu_hover == Some(i) {
                                for py in 0..Self::ITEM_HEIGHT {
                                    for px in 1..sw - 1 {
                                        surface.set_pixel(sx + px, sub_item_y + py, hover_color);
                                    }
                                }
                            }

                            // Icon
                            let icon_x = sx + 10;
                            let icon_y = sub_item_y + (Self::ITEM_HEIGHT - 20) / 2;
                            for py in 0..20 {
                                for px in 0..20 {
                                    surface.set_pixel(icon_x + px, icon_y + py, accent);
                                }
                            }

                            // Label
                            let label_x = sx + 40;
                            let label_y = sub_item_y + (Self::ITEM_HEIGHT - 16) / 2;
                            for (j, c) in child.label.chars().take(20).enumerate() {
                                draw_char_simple(surface, label_x + j * 8, label_y, c, theme.fg);
                            }

                            sub_item_y += Self::ITEM_HEIGHT;
                        }
                    }
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
