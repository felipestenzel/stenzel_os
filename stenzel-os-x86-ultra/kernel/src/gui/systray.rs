//! System Tray
//!
//! Notification area / system tray with icons for:
//! - Network status
//! - Volume control
//! - Battery indicator
//! - Date/time
//! - Custom application icons

use alloc::string::String;
use alloc::vec::Vec;
use alloc::boxed::Box;
use spin::Mutex;

use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use crate::gui::widgets::{Widget, WidgetId, WidgetEvent, Bounds, MouseButton};

/// System tray icon size (16x16)
pub const ICON_SIZE: usize = 16;

/// System tray icon padding
pub const ICON_PADDING: usize = 4;

/// System tray item ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TrayItemId(u64);

impl TrayItemId {
    /// Create a new unique tray item ID
    pub fn new() -> Self {
        static NEXT_ID: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(1);
        TrayItemId(NEXT_ID.fetch_add(1, core::sync::atomic::Ordering::Relaxed))
    }

    /// Get the raw ID
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

/// Click callback type
pub type TrayClickCallback = fn(TrayItemId, MouseButton);

/// Tray icon type - determines the built-in icon to show
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayIconType {
    /// Network status icon
    Network(NetworkStatus),
    /// Volume/audio icon
    Volume(VolumeLevel),
    /// Battery status
    Battery(BatteryStatus),
    /// Date/time
    DateTime,
    /// Generic notification
    Notification(bool), // bool = has unread
    /// Custom icon (use icon data)
    Custom,
}

/// Network connection status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkStatus {
    Disconnected,
    Ethernet,
    WiFi(u8), // signal strength 0-4 bars
    WiFiConnecting,
}

/// Volume level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumeLevel {
    Muted,
    Low,    // 1-33%
    Medium, // 34-66%
    High,   // 67-100%
}

/// Battery status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatteryStatus {
    Charging(u8),     // 0-100%
    Discharging(u8),  // 0-100%
    Full,
    NoBattery,
}

/// System tray item
pub struct TrayItem {
    /// Unique ID
    pub id: TrayItemId,
    /// Icon type
    pub icon_type: TrayIconType,
    /// Custom icon data (16x16 RGBA)
    pub icon_data: Option<Vec<u8>>,
    /// Tooltip text
    pub tooltip: String,
    /// Whether to show notification badge
    pub badge: Option<u8>,
    /// Click callback
    pub on_click: Option<TrayClickCallback>,
    /// Whether item is visible
    pub visible: bool,
    /// X position (calculated)
    x: usize,
}

impl TrayItem {
    /// Create a new tray item
    pub fn new(icon_type: TrayIconType, tooltip: &str) -> Self {
        Self {
            id: TrayItemId::new(),
            icon_type,
            icon_data: None,
            tooltip: String::from(tooltip),
            badge: None,
            on_click: None,
            visible: true,
            x: 0,
        }
    }

    /// Create with custom icon
    pub fn custom(icon_data: Vec<u8>, tooltip: &str) -> Self {
        Self {
            id: TrayItemId::new(),
            icon_type: TrayIconType::Custom,
            icon_data: Some(icon_data),
            tooltip: String::from(tooltip),
            badge: None,
            on_click: None,
            visible: true,
            x: 0,
        }
    }

    /// Set click callback
    pub fn set_on_click(&mut self, callback: TrayClickCallback) {
        self.on_click = Some(callback);
    }

    /// Set notification badge
    pub fn set_badge(&mut self, count: Option<u8>) {
        self.badge = count;
    }

    /// Update icon type
    pub fn set_icon_type(&mut self, icon_type: TrayIconType) {
        self.icon_type = icon_type;
    }

    /// Set tooltip
    pub fn set_tooltip(&mut self, tooltip: &str) {
        self.tooltip = String::from(tooltip);
    }
}

/// System Tray widget
pub struct SystemTray {
    id: WidgetId,
    bounds: Bounds,

    /// Tray items (ordered)
    items: Vec<TrayItem>,

    /// Background color
    bg_color: Color,
    /// Separator color
    separator_color: Color,
    /// Icon color
    icon_color: Color,
    /// Badge color
    badge_color: Color,

    /// Hovered item index
    hovered: Option<usize>,

    /// Tooltip to show
    tooltip: Option<(usize, usize, String)>, // x, y, text

    /// Whether tray is visible
    visible: bool,
    /// Whether tray needs redraw
    dirty: bool,
}

impl SystemTray {
    /// Create a new system tray
    pub fn new(x: isize, y: isize, height: usize) -> Self {
        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(x, y, 0, height), // width calculated from items
            items: Vec::new(),
            bg_color: Color::new(32, 32, 40),
            separator_color: Color::new(64, 64, 72),
            icon_color: Color::WHITE,
            badge_color: Color::new(255, 80, 80),
            hovered: None,
            tooltip: None,
            visible: true,
            dirty: true,
        }
    }

    /// Add a tray item
    pub fn add_item(&mut self, item: TrayItem) -> TrayItemId {
        let id = item.id;
        self.items.push(item);
        self.recalculate_layout();
        self.dirty = true;
        id
    }

    /// Remove a tray item
    pub fn remove_item(&mut self, id: TrayItemId) {
        self.items.retain(|i| i.id != id);
        self.recalculate_layout();
        self.dirty = true;
    }

    /// Get a mutable item by ID
    pub fn get_item_mut(&mut self, id: TrayItemId) -> Option<&mut TrayItem> {
        self.items.iter_mut().find(|i| i.id == id)
    }

    /// Get item by ID
    pub fn get_item(&self, id: TrayItemId) -> Option<&TrayItem> {
        self.items.iter().find(|i| i.id == id)
    }

    /// Update network status
    pub fn update_network(&mut self, status: NetworkStatus) {
        for item in &mut self.items {
            if let TrayIconType::Network(_) = item.icon_type {
                item.icon_type = TrayIconType::Network(status);
                self.dirty = true;
                return;
            }
        }
        // Add network icon if not present
        let mut item = TrayItem::new(TrayIconType::Network(status), "Network");
        self.items.insert(0, item);
        self.recalculate_layout();
        self.dirty = true;
    }

    /// Update volume level
    pub fn update_volume(&mut self, level: VolumeLevel) {
        for item in &mut self.items {
            if let TrayIconType::Volume(_) = item.icon_type {
                item.icon_type = TrayIconType::Volume(level);
                self.dirty = true;
                return;
            }
        }
        // Add volume icon if not present
        let item = TrayItem::new(TrayIconType::Volume(level), "Volume");
        self.items.insert(0, item);
        self.recalculate_layout();
        self.dirty = true;
    }

    /// Update battery status
    pub fn update_battery(&mut self, status: BatteryStatus) {
        for item in &mut self.items {
            if let TrayIconType::Battery(_) = item.icon_type {
                item.icon_type = TrayIconType::Battery(status);
                self.dirty = true;
                return;
            }
        }
    }

    /// Recalculate item positions
    fn recalculate_layout(&mut self) {
        let mut x = ICON_PADDING;
        for item in &mut self.items {
            if item.visible {
                item.x = x;
                x += ICON_SIZE + ICON_PADDING;
            }
        }
        self.bounds.width = x;
    }

    /// Get item at position
    fn item_at(&self, px: usize, py: usize) -> Option<usize> {
        if py < ICON_PADDING || py >= self.bounds.height - ICON_PADDING {
            return None;
        }

        for (i, item) in self.items.iter().enumerate() {
            if item.visible && px >= item.x && px < item.x + ICON_SIZE {
                return Some(i);
            }
        }
        None
    }

    /// Get tooltip to display
    pub fn current_tooltip(&self) -> Option<(&str, usize, usize)> {
        self.tooltip.as_ref().map(|(x, y, text)| (text.as_str(), *x, *y))
    }

    /// Draw a built-in icon
    fn draw_icon(&self, surface: &mut Surface, item: &TrayItem, icon_x: usize, icon_y: usize) {
        match item.icon_type {
            TrayIconType::Network(status) => {
                self.draw_network_icon(surface, icon_x, icon_y, status);
            }
            TrayIconType::Volume(level) => {
                self.draw_volume_icon(surface, icon_x, icon_y, level);
            }
            TrayIconType::Battery(status) => {
                self.draw_battery_icon(surface, icon_x, icon_y, status);
            }
            TrayIconType::DateTime => {
                self.draw_datetime_icon(surface, icon_x, icon_y);
            }
            TrayIconType::Notification(has_unread) => {
                self.draw_notification_icon(surface, icon_x, icon_y, has_unread);
            }
            TrayIconType::Custom => {
                if let Some(ref data) = item.icon_data {
                    self.draw_custom_icon(surface, icon_x, icon_y, data);
                }
            }
        }

        // Draw badge if present
        if let Some(count) = item.badge {
            self.draw_badge(surface, icon_x + ICON_SIZE - 6, icon_y, count);
        }
    }

    /// Draw network icon
    fn draw_network_icon(&self, surface: &mut Surface, x: usize, y: usize, status: NetworkStatus) {
        let color = match status {
            NetworkStatus::Disconnected => Color::new(128, 128, 128),
            _ => self.icon_color,
        };

        match status {
            NetworkStatus::Disconnected => {
                // X mark
                for i in 0..12 {
                    surface.set_pixel(x + 2 + i, y + 2 + i, color);
                    surface.set_pixel(x + 13 - i, y + 2 + i, color);
                }
            }
            NetworkStatus::Ethernet => {
                // Computer/network icon
                // Monitor shape
                for px in 3..13 {
                    surface.set_pixel(x + px, y + 2, color);
                    surface.set_pixel(x + px, y + 9, color);
                }
                for py in 2..10 {
                    surface.set_pixel(x + 3, y + py, color);
                    surface.set_pixel(x + 12, y + py, color);
                }
                // Stand
                for px in 6..10 {
                    surface.set_pixel(x + px, y + 10, color);
                    surface.set_pixel(x + px, y + 11, color);
                }
                for px in 4..12 {
                    surface.set_pixel(x + px, y + 12, color);
                }
            }
            NetworkStatus::WiFi(bars) => {
                // WiFi signal arcs
                let center_x = x + 8;
                let base_y = y + 13;

                // Dot at bottom
                surface.set_pixel(center_x, base_y, color);
                surface.set_pixel(center_x - 1, base_y, color);
                surface.set_pixel(center_x + 1, base_y, color);
                surface.set_pixel(center_x, base_y - 1, color);

                // Arcs based on signal strength
                let arc_color = |bar: u8, current: u8| -> Color {
                    if bar <= current {
                        color
                    } else {
                        Color::new(64, 64, 72)
                    }
                };

                // First arc (innermost)
                for i in 0..3 {
                    surface.set_pixel(center_x - 2 - i, base_y - 3 - i, arc_color(1, bars));
                    surface.set_pixel(center_x + 2 + i, base_y - 3 - i, arc_color(1, bars));
                }

                // Second arc
                for i in 0..3 {
                    surface.set_pixel(center_x - 4 - i, base_y - 5 - i, arc_color(2, bars));
                    surface.set_pixel(center_x + 4 + i, base_y - 5 - i, arc_color(2, bars));
                }

                // Third arc
                for i in 0..3 {
                    surface.set_pixel(center_x - 6 - i, base_y - 7 - i, arc_color(3, bars));
                    surface.set_pixel(center_x + 6 + i, base_y - 7 - i, arc_color(3, bars));
                }
            }
            NetworkStatus::WiFiConnecting => {
                // Animated/hollow wifi icon
                let center_x = x + 8;
                let base_y = y + 13;

                // Hollow dot
                surface.set_pixel(center_x, base_y, self.icon_color);

                // Dashed arcs (connecting animation)
                for i in (0..3).step_by(2) {
                    surface.set_pixel(center_x - 2 - i, base_y - 3 - i, self.icon_color);
                    surface.set_pixel(center_x + 2 + i, base_y - 3 - i, self.icon_color);
                }
            }
        }
    }

    /// Draw volume icon
    fn draw_volume_icon(&self, surface: &mut Surface, x: usize, y: usize, level: VolumeLevel) {
        let color = self.icon_color;

        // Speaker shape (left side)
        for py in 5..11 {
            surface.set_pixel(x + 2, y + py, color);
            surface.set_pixel(x + 3, y + py, color);
        }
        // Cone
        for py in 3..13 {
            let width = ((py as isize - 3).abs() as usize).min(4);
            for px in 0..width {
                surface.set_pixel(x + 4 + px, y + py, color);
            }
        }

        match level {
            VolumeLevel::Muted => {
                // X over speaker
                let mute_color = Color::new(255, 100, 100);
                for i in 0..6 {
                    surface.set_pixel(x + 9 + i, y + 5 + i, mute_color);
                    surface.set_pixel(x + 14 - i, y + 5 + i, mute_color);
                }
            }
            VolumeLevel::Low => {
                // One sound wave
                for py in 6..10 {
                    surface.set_pixel(x + 10, y + py, color);
                }
            }
            VolumeLevel::Medium => {
                // Two sound waves
                for py in 6..10 {
                    surface.set_pixel(x + 10, y + py, color);
                }
                for py in 4..12 {
                    surface.set_pixel(x + 12, y + py, color);
                }
            }
            VolumeLevel::High => {
                // Three sound waves
                for py in 6..10 {
                    surface.set_pixel(x + 10, y + py, color);
                }
                for py in 4..12 {
                    surface.set_pixel(x + 12, y + py, color);
                }
                for py in 2..14 {
                    surface.set_pixel(x + 14, y + py, color);
                }
            }
        }
    }

    /// Draw battery icon
    fn draw_battery_icon(&self, surface: &mut Surface, x: usize, y: usize, status: BatteryStatus) {
        let (percent, charging) = match status {
            BatteryStatus::Charging(p) => (p, true),
            BatteryStatus::Discharging(p) => (p, false),
            BatteryStatus::Full => (100, false),
            BatteryStatus::NoBattery => {
                // Draw power plug icon
                for py in 4..12 {
                    surface.set_pixel(x + 6, y + py, self.icon_color);
                    surface.set_pixel(x + 10, y + py, self.icon_color);
                }
                for px in 4..12 {
                    surface.set_pixel(x + px, y + 8, self.icon_color);
                }
                return;
            }
        };

        // Battery outline
        let outline_color = self.icon_color;

        // Body
        for px in 2..14 {
            surface.set_pixel(x + px, y + 4, outline_color);
            surface.set_pixel(x + px, y + 11, outline_color);
        }
        for py in 4..12 {
            surface.set_pixel(x + 2, y + py, outline_color);
            surface.set_pixel(x + 13, y + py, outline_color);
        }
        // Cap
        for py in 6..10 {
            surface.set_pixel(x + 14, y + py, outline_color);
        }

        // Fill based on percentage
        let fill_width = ((percent as usize * 10) / 100).min(10);
        let fill_color = if percent <= 20 {
            Color::new(255, 80, 80) // Red for low
        } else if charging {
            Color::new(100, 255, 100) // Green for charging
        } else {
            Color::WHITE
        };

        for py in 5..11 {
            for px in 0..fill_width {
                surface.set_pixel(x + 3 + px, y + py, fill_color);
            }
        }

        // Lightning bolt for charging
        if charging {
            let bolt_color = Color::new(255, 255, 0);
            surface.set_pixel(x + 8, y + 5, bolt_color);
            surface.set_pixel(x + 7, y + 6, bolt_color);
            surface.set_pixel(x + 6, y + 7, bolt_color);
            surface.set_pixel(x + 7, y + 7, bolt_color);
            surface.set_pixel(x + 8, y + 7, bolt_color);
            surface.set_pixel(x + 9, y + 7, bolt_color);
            surface.set_pixel(x + 8, y + 8, bolt_color);
            surface.set_pixel(x + 9, y + 9, bolt_color);
            surface.set_pixel(x + 8, y + 10, bolt_color);
        }
    }

    /// Draw date/time icon
    fn draw_datetime_icon(&self, surface: &mut Surface, x: usize, y: usize) {
        let color = self.icon_color;

        // Clock circle (manually positioned points for circle)
        let cx = x + 8;
        let cy = y + 8;

        // Draw circle using 8 points (octagon approximation)
        // Top
        surface.set_pixel(cx, cy - 6, color);
        surface.set_pixel(cx - 1, cy - 6, color);
        surface.set_pixel(cx + 1, cy - 6, color);
        // Bottom
        surface.set_pixel(cx, cy + 6, color);
        surface.set_pixel(cx - 1, cy + 6, color);
        surface.set_pixel(cx + 1, cy + 6, color);
        // Left
        surface.set_pixel(cx - 6, cy, color);
        surface.set_pixel(cx - 6, cy - 1, color);
        surface.set_pixel(cx - 6, cy + 1, color);
        // Right
        surface.set_pixel(cx + 6, cy, color);
        surface.set_pixel(cx + 6, cy - 1, color);
        surface.set_pixel(cx + 6, cy + 1, color);
        // Diagonals
        surface.set_pixel(cx - 4, cy - 4, color);
        surface.set_pixel(cx + 4, cy - 4, color);
        surface.set_pixel(cx - 4, cy + 4, color);
        surface.set_pixel(cx + 4, cy + 4, color);

        // Clock hands
        // Hour hand (pointing to ~10 o'clock)
        for i in 0..4 {
            surface.set_pixel(cx - i, cy - i, color);
        }
        // Minute hand (pointing to 12)
        for i in 0..5 {
            surface.set_pixel(cx, cy - i, color);
        }
        // Center dot
        surface.set_pixel(cx, cy, color);
    }

    /// Draw notification icon
    fn draw_notification_icon(&self, surface: &mut Surface, x: usize, y: usize, has_unread: bool) {
        let color = self.icon_color;

        // Bell shape
        // Top
        surface.set_pixel(x + 8, y + 2, color);

        // Dome
        for py in 3..10 {
            let half_width = (py - 3).min(5);
            for px in 0..=half_width * 2 {
                surface.set_pixel(x + 8 - half_width + px, y + py, color);
            }
        }

        // Base
        for px in 3..14 {
            surface.set_pixel(x + px, y + 10, color);
        }

        // Clapper
        surface.set_pixel(x + 7, y + 12, color);
        surface.set_pixel(x + 8, y + 12, color);
        surface.set_pixel(x + 9, y + 12, color);
        surface.set_pixel(x + 8, y + 13, color);

        // Unread indicator
        if has_unread {
            for py in 0..4 {
                for px in 0..4 {
                    surface.set_pixel(x + 12 + px, y + py, Color::new(255, 80, 80));
                }
            }
        }
    }

    /// Draw custom icon from RGBA data
    fn draw_custom_icon(&self, surface: &mut Surface, x: usize, y: usize, data: &[u8]) {
        if data.len() < ICON_SIZE * ICON_SIZE * 4 {
            return;
        }

        for py in 0..ICON_SIZE {
            for px in 0..ICON_SIZE {
                let i = (py * ICON_SIZE + px) * 4;
                let r = data[i];
                let g = data[i + 1];
                let b = data[i + 2];
                let a = data[i + 3];

                if a > 128 {
                    surface.set_pixel(x + px, y + py, Color::new(r, g, b));
                }
            }
        }
    }

    /// Draw notification badge
    fn draw_badge(&self, surface: &mut Surface, x: usize, y: usize, count: u8) {
        // Red circle
        for py in 0..8 {
            for px in 0..8 {
                let dx = px as isize - 4;
                let dy = py as isize - 4;
                if dx * dx + dy * dy <= 16 {
                    surface.set_pixel(x + px, y + py, self.badge_color);
                }
            }
        }

        // Number (simple, just show 9+ for > 9)
        if count > 9 {
            // Draw "+"
            surface.set_pixel(x + 4, y + 2, Color::WHITE);
            surface.set_pixel(x + 4, y + 3, Color::WHITE);
            surface.set_pixel(x + 4, y + 4, Color::WHITE);
            surface.set_pixel(x + 4, y + 5, Color::WHITE);
            surface.set_pixel(x + 2, y + 3, Color::WHITE);
            surface.set_pixel(x + 3, y + 3, Color::WHITE);
            surface.set_pixel(x + 5, y + 3, Color::WHITE);
            surface.set_pixel(x + 6, y + 3, Color::WHITE);
        }
    }

    /// Check if dirty
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Clear dirty flag
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    /// Get total width
    pub fn width(&self) -> usize {
        self.bounds.width
    }
}

impl Widget for SystemTray {
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
        self.dirty = true;
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
        self.dirty = true;
    }

    fn handle_event(&mut self, event: &WidgetEvent) -> bool {
        if !self.visible {
            return false;
        }

        match event {
            WidgetEvent::MouseMove { x, y, .. } => {
                let px = (*x as isize - self.bounds.x) as usize;
                let py = (*y as isize - self.bounds.y) as usize;

                let new_hovered = self.item_at(px, py);
                if new_hovered != self.hovered {
                    self.hovered = new_hovered;

                    // Update tooltip
                    if let Some(idx) = new_hovered {
                        let item = &self.items[idx];
                        self.tooltip = Some((
                            self.bounds.x as usize + item.x,
                            self.bounds.y as usize - 20,
                            item.tooltip.clone()
                        ));
                    } else {
                        self.tooltip = None;
                    }

                    self.dirty = true;
                }
                true
            }
            WidgetEvent::MouseDown { button, .. } => {
                if let Some(idx) = self.hovered {
                    let item = &self.items[idx];
                    if let Some(callback) = item.on_click {
                        callback(item.id, *button);
                    }
                    return true;
                }
                false
            }
            WidgetEvent::Blur => {
                self.hovered = None;
                self.tooltip = None;
                self.dirty = true;
                true
            }
            _ => false,
        }
    }

    fn render(&self, surface: &mut Surface) {
        if !self.visible {
            return;
        }

        let x = self.bounds.x as usize;
        let y = self.bounds.y as usize;

        // Background
        for py in 0..self.bounds.height {
            for px in 0..self.bounds.width {
                surface.set_pixel(x + px, y + py, self.bg_color);
            }
        }

        // Left separator
        for py in 4..(self.bounds.height - 4) {
            surface.set_pixel(x, y + py, self.separator_color);
        }

        // Draw items
        let icon_y = y + (self.bounds.height - ICON_SIZE) / 2;

        for (i, item) in self.items.iter().enumerate() {
            if !item.visible {
                continue;
            }

            let icon_x = x + item.x;

            // Hover highlight
            if Some(i) == self.hovered {
                for py in 2..(self.bounds.height - 2) {
                    for px in 0..(ICON_SIZE + 4) {
                        surface.set_pixel(icon_x - 2 + px, y + py, Color::new(64, 64, 72));
                    }
                }
            }

            self.draw_icon(surface, item, icon_x, icon_y);
        }
    }
}

/// Global system tray instance
static SYSTEM_TRAY: Mutex<Option<SystemTray>> = Mutex::new(None);

/// Initialize the system tray
pub fn init(x: isize, y: isize, height: usize) {
    let mut tray = SystemTray::new(x, y, height);

    // Add default system icons
    let network = TrayItem::new(TrayIconType::Network(NetworkStatus::Disconnected), "Network: Disconnected");
    let volume = TrayItem::new(TrayIconType::Volume(VolumeLevel::Medium), "Volume: 50%");

    tray.add_item(network);
    tray.add_item(volume);

    *SYSTEM_TRAY.lock() = Some(tray);
    crate::kprintln!("systray: initialized");
}

/// Update network status
pub fn update_network(status: NetworkStatus) {
    let mut tray = SYSTEM_TRAY.lock();
    if let Some(ref mut t) = *tray {
        let tooltip = match status {
            NetworkStatus::Disconnected => "Network: Disconnected",
            NetworkStatus::Ethernet => "Network: Ethernet",
            NetworkStatus::WiFi(bars) => match bars {
                0..=1 => "Network: WiFi (Weak)",
                2 => "Network: WiFi (Fair)",
                3 => "Network: WiFi (Good)",
                _ => "Network: WiFi (Excellent)",
            },
            NetworkStatus::WiFiConnecting => "Network: Connecting...",
        };
        t.update_network(status);
        // Update tooltip
        for item in &mut t.items {
            if let TrayIconType::Network(_) = item.icon_type {
                item.tooltip = String::from(tooltip);
                break;
            }
        }
    }
}

/// Update volume level
pub fn update_volume(level: VolumeLevel, percent: u8) {
    let mut tray = SYSTEM_TRAY.lock();
    if let Some(ref mut t) = *tray {
        let tooltip = match level {
            VolumeLevel::Muted => String::from("Volume: Muted"),
            _ => {
                use alloc::string::ToString;
                let mut s = String::from("Volume: ");
                s.push_str(&percent.to_string());
                s.push('%');
                s
            }
        };
        t.update_volume(level);
        // Update tooltip
        for item in &mut t.items {
            if let TrayIconType::Volume(_) = item.icon_type {
                item.tooltip = tooltip;
                break;
            }
        }
    }
}

/// Update battery status
pub fn update_battery(status: BatteryStatus) {
    let mut tray = SYSTEM_TRAY.lock();
    if let Some(ref mut t) = *tray {
        t.update_battery(status);
    }
}

/// Add a custom tray item
pub fn add_item(item: TrayItem) -> Option<TrayItemId> {
    let mut tray = SYSTEM_TRAY.lock();
    tray.as_mut().map(|t| t.add_item(item))
}

/// Remove a tray item
pub fn remove_item(id: TrayItemId) {
    let mut tray = SYSTEM_TRAY.lock();
    if let Some(ref mut t) = *tray {
        t.remove_item(id);
    }
}

/// Execute with system tray access
pub fn with_systray<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut SystemTray) -> R,
{
    let mut tray = SYSTEM_TRAY.lock();
    tray.as_mut().map(f)
}

/// Check if system tray is available
pub fn is_available() -> bool {
    SYSTEM_TRAY.lock().is_some()
}
