//! Battery Indicator Widget
//!
//! A popup widget for displaying battery status, power settings,
//! and energy management options.

use alloc::string::String;
use alloc::vec::Vec;
use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use super::{Widget, WidgetId, WidgetState, WidgetEvent, Bounds, MouseButton, theme};

/// Battery state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatteryState {
    /// Running on battery power
    Discharging,
    /// Plugged in and charging
    Charging,
    /// Plugged in, battery full
    Full,
    /// No battery present (desktop/always plugged)
    NoBattery,
    /// Battery status unknown
    Unknown,
}

/// Power profile
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerProfile {
    /// Maximum performance, higher power consumption
    Performance,
    /// Balanced between performance and battery life
    Balanced,
    /// Power saving mode, reduced performance
    PowerSaver,
    /// Automatic based on battery state
    Automatic,
}

impl PowerProfile {
    /// Get display name
    pub fn name(&self) -> &'static str {
        match self {
            PowerProfile::Performance => "Performance",
            PowerProfile::Balanced => "Balanced",
            PowerProfile::PowerSaver => "Power Saver",
            PowerProfile::Automatic => "Automatic",
        }
    }

    /// Get description
    pub fn description(&self) -> &'static str {
        match self {
            PowerProfile::Performance => "Maximum performance",
            PowerProfile::Balanced => "Balance performance and battery",
            PowerProfile::PowerSaver => "Extend battery life",
            PowerProfile::Automatic => "Adapts to your usage",
        }
    }
}

/// Battery information
#[derive(Debug, Clone)]
pub struct BatteryInfo {
    /// Current charge percentage (0-100)
    pub percentage: u8,
    /// Current state
    pub state: BatteryState,
    /// Estimated time remaining in minutes (if discharging)
    pub time_remaining: Option<u32>,
    /// Estimated time to full in minutes (if charging)
    pub time_to_full: Option<u32>,
    /// Current power draw in mW (negative if discharging)
    pub power_draw: Option<i32>,
    /// Battery health percentage
    pub health: u8,
    /// Design capacity in mWh
    pub design_capacity: u32,
    /// Current full capacity in mWh
    pub full_capacity: u32,
    /// Battery cycle count
    pub cycle_count: u32,
    /// Battery temperature in Celsius * 10
    pub temperature: Option<i16>,
    /// Battery manufacturer
    pub manufacturer: String,
    /// Battery model
    pub model: String,
}

impl BatteryInfo {
    /// Create a new battery info with defaults
    pub fn new() -> Self {
        Self {
            percentage: 100,
            state: BatteryState::Unknown,
            time_remaining: None,
            time_to_full: None,
            power_draw: None,
            health: 100,
            design_capacity: 50000,
            full_capacity: 50000,
            cycle_count: 0,
            temperature: None,
            manufacturer: String::from("Unknown"),
            model: String::from("Unknown"),
        }
    }

    /// Create battery info for desktop (no battery)
    pub fn no_battery() -> Self {
        let mut info = Self::new();
        info.state = BatteryState::NoBattery;
        info.percentage = 100;
        info
    }

    /// Get status text
    pub fn status_text(&self) -> &'static str {
        match self.state {
            BatteryState::Discharging => "On battery",
            BatteryState::Charging => "Charging",
            BatteryState::Full => "Fully charged",
            BatteryState::NoBattery => "Plugged in",
            BatteryState::Unknown => "Unknown",
        }
    }

    /// Get time remaining as formatted string
    pub fn time_string(&self) -> Option<String> {
        let minutes = match self.state {
            BatteryState::Discharging => self.time_remaining?,
            BatteryState::Charging => self.time_to_full?,
            _ => return None,
        };

        let hours = minutes / 60;
        let mins = minutes % 60;

        Some(if hours > 0 {
            format_time(hours, mins)
        } else {
            format_minutes(mins)
        })
    }

    /// Check if battery is critical (< 10%)
    pub fn is_critical(&self) -> bool {
        self.state == BatteryState::Discharging && self.percentage < 10
    }

    /// Check if battery is low (< 20%)
    pub fn is_low(&self) -> bool {
        self.state == BatteryState::Discharging && self.percentage < 20
    }
}

impl Default for BatteryInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Battery action callback
pub type BatteryCallback = fn(BatteryAction);

/// Battery actions
#[derive(Debug, Clone, Copy)]
pub enum BatteryAction {
    /// Change power profile
    SetProfile(PowerProfile),
    /// Open power settings
    OpenSettings,
    /// Toggle battery saver
    ToggleBatterySaver,
}

/// Battery indicator popup widget
pub struct BatteryIndicator {
    id: WidgetId,
    bounds: Bounds,

    /// Battery information
    battery: BatteryInfo,
    /// Current power profile
    power_profile: PowerProfile,
    /// Battery saver enabled
    battery_saver: bool,
    /// Show detailed info
    show_details: bool,

    /// Whether popup is visible
    visible: bool,
    /// Widget state
    state: WidgetState,
    /// Hovered item
    hovered_item: HoveredItem,

    /// Action callback
    on_action: Option<BatteryCallback>,

    /// Colors
    bg_color: Color,
    text_color: Color,
    accent_color: Color,
    good_color: Color,
    warning_color: Color,
    critical_color: Color,
    charging_color: Color,
}

/// Hovered item in the popup
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HoveredItem {
    None,
    Profile(usize),
    BatterySaver,
    Details,
    Settings,
}

impl BatteryIndicator {
    /// Popup dimensions
    const WIDTH: usize = 300;
    const BASE_HEIGHT: usize = 240;
    const PROFILE_HEIGHT: usize = 44;
    const PADDING: usize = 16;

    /// Create a new battery indicator
    pub fn new(x: isize, y: isize) -> Self {
        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(x, y, Self::WIDTH, Self::BASE_HEIGHT),
            battery: BatteryInfo::new(),
            power_profile: PowerProfile::Balanced,
            battery_saver: false,
            show_details: false,
            visible: false,
            state: WidgetState::Normal,
            hovered_item: HoveredItem::None,
            on_action: None,
            bg_color: Color::new(40, 40, 48),
            text_color: Color::WHITE,
            accent_color: Color::new(0, 120, 215),
            good_color: Color::new(100, 255, 100),
            warning_color: Color::new(255, 200, 50),
            critical_color: Color::new(255, 80, 80),
            charging_color: Color::new(100, 200, 255),
        }
    }

    /// Show the popup
    pub fn show(&mut self) {
        self.visible = true;
        self.update_height();
    }

    /// Hide the popup
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Toggle visibility
    pub fn toggle(&mut self) {
        if self.visible {
            self.hide();
        } else {
            self.show();
        }
    }

    /// Update battery information
    pub fn update_battery(&mut self, info: BatteryInfo) {
        self.battery = info;
    }

    /// Set battery percentage
    pub fn set_percentage(&mut self, percentage: u8) {
        self.battery.percentage = percentage.min(100);
    }

    /// Set battery state
    pub fn set_state(&mut self, state: BatteryState) {
        self.battery.state = state;
    }

    /// Set power profile
    pub fn set_power_profile(&mut self, profile: PowerProfile) {
        self.power_profile = profile;
        if let Some(callback) = self.on_action {
            callback(BatteryAction::SetProfile(profile));
        }
    }

    /// Get current power profile
    pub fn power_profile(&self) -> PowerProfile {
        self.power_profile
    }

    /// Toggle battery saver
    pub fn toggle_battery_saver(&mut self) {
        self.battery_saver = !self.battery_saver;
        if let Some(callback) = self.on_action {
            callback(BatteryAction::ToggleBatterySaver);
        }
    }

    /// Check if battery saver is on
    pub fn is_battery_saver(&self) -> bool {
        self.battery_saver
    }

    /// Toggle detailed view
    pub fn toggle_details(&mut self) {
        self.show_details = !self.show_details;
        self.update_height();
    }

    /// Set action callback
    pub fn set_on_action(&mut self, callback: BatteryCallback) {
        self.on_action = Some(callback);
    }

    /// Update height based on content
    fn update_height(&mut self) {
        let mut height = Self::BASE_HEIGHT;
        if self.show_details {
            height += 100; // Extra space for details
        }
        self.bounds.height = height;
    }

    /// Get battery color based on percentage and state
    fn battery_color(&self) -> Color {
        match self.battery.state {
            BatteryState::Charging => self.charging_color,
            BatteryState::Full => self.good_color,
            BatteryState::NoBattery => self.accent_color,
            _ => {
                if self.battery.percentage < 10 {
                    self.critical_color
                } else if self.battery.percentage < 20 {
                    self.warning_color
                } else {
                    self.good_color
                }
            }
        }
    }

    /// Get profile item bounds
    fn profile_bounds(&self, index: usize) -> Bounds {
        let x = self.bounds.x + Self::PADDING as isize;
        let y = self.bounds.y + 100 + (index * Self::PROFILE_HEIGHT) as isize;
        let width = self.bounds.width - Self::PADDING * 2;
        Bounds::new(x, y, width, Self::PROFILE_HEIGHT - 4)
    }

    /// Draw the battery icon
    fn draw_battery_icon(&self, surface: &mut Surface, x: usize, y: usize, size: usize) {
        let color = self.battery_color();
        let outline_color = self.text_color;

        // Battery body outline
        let body_width = size;
        let body_height = (size as f32 * 0.5) as usize;
        let body_y = y + (size - body_height) / 2;

        // Outline
        for px in 0..body_width {
            surface.set_pixel(x + px, body_y, outline_color);
            surface.set_pixel(x + px, body_y + body_height - 1, outline_color);
        }
        for py in 0..body_height {
            surface.set_pixel(x, body_y + py, outline_color);
            surface.set_pixel(x + body_width - 1, body_y + py, outline_color);
        }

        // Battery cap
        let cap_height = body_height / 2;
        let cap_y = body_y + (body_height - cap_height) / 2;
        for py in 0..cap_height {
            surface.set_pixel(x + body_width, cap_y + py, outline_color);
        }

        // Fill based on percentage
        let fill_width = ((body_width - 4) * self.battery.percentage as usize) / 100;
        let fill_height = body_height - 4;
        for py in 0..fill_height {
            for px in 0..fill_width {
                surface.set_pixel(x + 2 + px, body_y + 2 + py, color);
            }
        }

        // Charging lightning bolt
        if self.battery.state == BatteryState::Charging {
            let bolt_color = Color::new(255, 255, 100);
            let cx = x + body_width / 2;
            let cy = body_y + body_height / 2;

            surface.set_pixel(cx + 2, cy - 4, bolt_color);
            surface.set_pixel(cx + 1, cy - 3, bolt_color);
            surface.set_pixel(cx, cy - 2, bolt_color);
            surface.set_pixel(cx - 1, cy - 1, bolt_color);
            surface.set_pixel(cx, cy - 1, bolt_color);
            surface.set_pixel(cx + 1, cy - 1, bolt_color);
            surface.set_pixel(cx + 2, cy - 1, bolt_color);
            surface.set_pixel(cx + 1, cy, bolt_color);
            surface.set_pixel(cx, cy + 1, bolt_color);
            surface.set_pixel(cx - 1, cy + 2, bolt_color);
            surface.set_pixel(cx - 2, cy + 3, bolt_color);
        }
    }

    /// Draw power profile icon
    fn draw_profile_icon(&self, surface: &mut Surface, x: usize, y: usize, profile: PowerProfile, selected: bool) {
        let color = if selected { self.accent_color } else { self.text_color };

        match profile {
            PowerProfile::Performance => {
                // Lightning bolt / rocket
                for i in 0..8 {
                    surface.set_pixel(x + 4 + i, y + 2, color);
                    surface.set_pixel(x + 4 + i, y + 12, color);
                }
                for i in 0..10 {
                    surface.set_pixel(x + 4, y + 2 + i, color);
                }
                surface.set_pixel(x + 8, y + 7, color);
                surface.set_pixel(x + 9, y + 6, color);
                surface.set_pixel(x + 10, y + 5, color);
            }
            PowerProfile::Balanced => {
                // Scale / balance icon
                for px in 2..14 {
                    surface.set_pixel(x + px, y + 7, color);
                }
                for py in 2..12 {
                    surface.set_pixel(x + 8, y + py, color);
                }
                // Pans
                for px in 2..6 {
                    surface.set_pixel(x + px, y + 10, color);
                }
                for px in 10..14 {
                    surface.set_pixel(x + px, y + 10, color);
                }
            }
            PowerProfile::PowerSaver => {
                // Leaf icon
                for i in 0..6 {
                    surface.set_pixel(x + 4 + i, y + 2 + i, color);
                    surface.set_pixel(x + 10 - i, y + 2 + i, color);
                }
                for py in 8..13 {
                    surface.set_pixel(x + 7, y + py, color);
                }
            }
            PowerProfile::Automatic => {
                // Gear / auto icon
                let cx = x + 8;
                let cy = y + 7;
                // Simple gear shape
                for i in 0..3 {
                    surface.set_pixel(cx, cy - 4 + i, color);
                    surface.set_pixel(cx, cy + 2 + i, color);
                    surface.set_pixel(cx - 4 + i, cy, color);
                    surface.set_pixel(cx + 2 + i, cy, color);
                }
                // Diagonals
                surface.set_pixel(cx - 2, cy - 2, color);
                surface.set_pixel(cx + 2, cy - 2, color);
                surface.set_pixel(cx - 2, cy + 2, color);
                surface.set_pixel(cx + 2, cy + 2, color);
            }
        }
    }
}

impl Widget for BatteryIndicator {
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
    }

    fn handle_event(&mut self, event: &WidgetEvent) -> bool {
        if !self.visible {
            return false;
        }

        match event {
            WidgetEvent::MouseMove { x, y } => {
                // Check profile items
                let profiles = [PowerProfile::Balanced, PowerProfile::PowerSaver, PowerProfile::Performance];
                for (i, _) in profiles.iter().enumerate() {
                    if self.profile_bounds(i).contains(*x, *y) {
                        self.hovered_item = HoveredItem::Profile(i);
                        return true;
                    }
                }

                self.hovered_item = HoveredItem::None;
                true
            }
            WidgetEvent::MouseDown { button: MouseButton::Left, x, y } => {
                // Check profile items
                let profiles = [PowerProfile::Balanced, PowerProfile::PowerSaver, PowerProfile::Performance];
                for (i, profile) in profiles.iter().enumerate() {
                    if self.profile_bounds(i).contains(*x, *y) {
                        self.set_power_profile(*profile);
                        return true;
                    }
                }

                // Click outside to close
                if !self.bounds.contains(*x, *y) {
                    self.hide();
                }

                true
            }
            WidgetEvent::Blur => {
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

        let x = self.bounds.x.max(0) as usize;
        let y = self.bounds.y.max(0) as usize;
        let w = self.bounds.width;
        let h = self.bounds.height;

        // Background
        for py in 0..h {
            for px in 0..w {
                surface.set_pixel(x + px, y + py, self.bg_color);
            }
        }

        // Border
        let border_color = Color::new(80, 80, 90);
        for px in 0..w {
            surface.set_pixel(x + px, y, border_color);
            surface.set_pixel(x + px, y + h - 1, border_color);
        }
        for py in 0..h {
            surface.set_pixel(x, y + py, border_color);
            surface.set_pixel(x + w - 1, y + py, border_color);
        }

        // Battery icon
        self.draw_battery_icon(surface, x + Self::PADDING, y + 16, 48);

        // Percentage
        let pct_text = format_percentage(self.battery.percentage);
        let pct_x = x + 80;
        let pct_y = y + 20;
        let pct_color = self.battery_color();
        for (i, c) in pct_text.chars().enumerate() {
            draw_char_large(surface, pct_x + i * 16, pct_y, c, pct_color);
        }

        // Status text
        let status = self.battery.status_text();
        let status_x = x + 80;
        let status_y = y + 48;
        for (i, c) in status.chars().enumerate() {
            draw_char(surface, status_x + i * 8, status_y, c, self.text_color);
        }

        // Time remaining
        if let Some(time_str) = self.battery.time_string() {
            let time_prefix = match self.battery.state {
                BatteryState::Charging => "until full",
                BatteryState::Discharging => "remaining",
                _ => "",
            };
            let time_x = x + 80;
            let time_y = y + 64;

            for (i, c) in time_str.chars().enumerate() {
                draw_char(surface, time_x + i * 8, time_y, c, Color::new(180, 180, 190));
            }
            let prefix_x = time_x + (time_str.len() + 1) * 8;
            for (i, c) in time_prefix.chars().enumerate() {
                draw_char(surface, prefix_x + i * 8, time_y, c, Color::new(150, 150, 160));
            }
        }

        // Divider
        let divider_y = y + 88;
        for px in Self::PADDING..(w - Self::PADDING) {
            surface.set_pixel(x + px, divider_y, border_color);
        }

        // Power profiles
        let profiles = [
            (PowerProfile::Balanced, "Balanced", "Balance performance and battery"),
            (PowerProfile::PowerSaver, "Power Saver", "Extend battery life"),
            (PowerProfile::Performance, "Performance", "Maximum performance"),
        ];

        for (i, (profile, name, desc)) in profiles.iter().enumerate() {
            let pb = self.profile_bounds(i);
            let px = pb.x.max(0) as usize;
            let py = pb.y.max(0) as usize;

            let is_selected = *profile == self.power_profile;
            let is_hovered = self.hovered_item == HoveredItem::Profile(i);

            // Highlight
            if is_selected || is_hovered {
                let bg = if is_selected {
                    Color::new(0, 80, 150)
                } else {
                    Color::new(60, 60, 70)
                };
                for fy in 0..pb.height {
                    for fx in 0..pb.width {
                        surface.set_pixel(px + fx, py + fy, bg);
                    }
                }
            }

            // Selection indicator
            if is_selected {
                for fy in 4..pb.height - 4 {
                    surface.set_pixel(px + 4, py + fy, self.accent_color);
                    surface.set_pixel(px + 5, py + fy, self.accent_color);
                }
            }

            // Profile icon
            self.draw_profile_icon(surface, px + 16, py + 8, *profile, is_selected);

            // Profile name
            let name_x = px + 40;
            let name_y = py + 8;
            for (ci, c) in name.chars().enumerate() {
                draw_char(surface, name_x + ci * 8, name_y, c, self.text_color);
            }

            // Description
            let desc_y = py + 24;
            for (ci, c) in desc.chars().take(35).enumerate() {
                draw_char(surface, name_x + ci * 8, desc_y, c, Color::new(150, 150, 160));
            }
        }

        // Details section if shown
        if self.show_details {
            let details_y = y + 100 + profiles.len() * Self::PROFILE_HEIGHT;

            // Divider
            for px in Self::PADDING..(w - Self::PADDING) {
                surface.set_pixel(x + px, details_y, border_color);
            }

            let detail_x = x + Self::PADDING;
            let mut detail_y = details_y + 16;

            // Health
            let health_text = format_health(self.battery.health);
            let health_label = "Battery health: ";
            for (i, c) in health_label.chars().enumerate() {
                draw_char(surface, detail_x + i * 8, detail_y, c, Color::new(150, 150, 160));
            }
            let health_color = if self.battery.health >= 80 {
                self.good_color
            } else if self.battery.health >= 50 {
                self.warning_color
            } else {
                self.critical_color
            };
            for (i, c) in health_text.chars().enumerate() {
                draw_char(surface, detail_x + health_label.len() * 8 + i * 8, detail_y, c, health_color);
            }
            detail_y += 18;

            // Cycle count
            let cycles_text = format_number(self.battery.cycle_count);
            let cycles_label = "Cycle count: ";
            for (i, c) in cycles_label.chars().enumerate() {
                draw_char(surface, detail_x + i * 8, detail_y, c, Color::new(150, 150, 160));
            }
            for (i, c) in cycles_text.chars().enumerate() {
                draw_char(surface, detail_x + cycles_label.len() * 8 + i * 8, detail_y, c, self.text_color);
            }
            detail_y += 18;

            // Capacity
            let capacity_pct = (self.battery.full_capacity * 100) / self.battery.design_capacity.max(1);
            let capacity_text = format_percentage(capacity_pct as u8);
            let capacity_label = "Capacity: ";
            for (i, c) in capacity_label.chars().enumerate() {
                draw_char(surface, detail_x + i * 8, detail_y, c, Color::new(150, 150, 160));
            }
            for (i, c) in capacity_text.chars().enumerate() {
                draw_char(surface, detail_x + capacity_label.len() * 8 + i * 8, detail_y, c, self.text_color);
            }
            detail_y += 18;

            // Temperature if available
            if let Some(temp) = self.battery.temperature {
                let temp_text = format_temp(temp);
                let temp_label = "Temperature: ";
                for (i, c) in temp_label.chars().enumerate() {
                    draw_char(surface, detail_x + i * 8, detail_y, c, Color::new(150, 150, 160));
                }
                for (i, c) in temp_text.chars().enumerate() {
                    draw_char(surface, detail_x + temp_label.len() * 8 + i * 8, detail_y, c, self.text_color);
                }
            }
        }
    }
}

// Helper functions

fn format_percentage(value: u8) -> String {
    use alloc::string::ToString;
    let mut s = value.to_string();
    s.push('%');
    s
}

fn format_health(value: u8) -> String {
    format_percentage(value)
}

fn format_number(value: u32) -> String {
    use alloc::string::ToString;
    value.to_string()
}

fn format_time(hours: u32, mins: u32) -> String {
    use alloc::string::ToString;
    let mut s = hours.to_string();
    s.push_str("h ");
    s.push_str(&mins.to_string());
    s.push('m');
    s
}

fn format_minutes(mins: u32) -> String {
    use alloc::string::ToString;
    let mut s = mins.to_string();
    s.push_str(" min");
    s
}

fn format_temp(temp: i16) -> String {
    use alloc::string::ToString;
    let degrees = temp / 10;
    let mut s = degrees.to_string();
    s.push_str(" C");
    s
}

fn draw_char(surface: &mut Surface, x: usize, y: usize, c: char, color: Color) {
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

fn draw_char_large(surface: &mut Surface, x: usize, y: usize, c: char, color: Color) {
    use crate::drivers::font::DEFAULT_FONT;

    if let Some(glyph) = DEFAULT_FONT.get_glyph(c) {
        for row in 0..DEFAULT_FONT.height {
            let byte = glyph[row];
            for col in 0..DEFAULT_FONT.width {
                if (byte >> (DEFAULT_FONT.width - 1 - col)) & 1 != 0 {
                    // 2x scale
                    surface.set_pixel(x + col * 2, y + row * 2, color);
                    surface.set_pixel(x + col * 2 + 1, y + row * 2, color);
                    surface.set_pixel(x + col * 2, y + row * 2 + 1, color);
                    surface.set_pixel(x + col * 2 + 1, y + row * 2 + 1, color);
                }
            }
        }
    }
}

/// Global battery state
use spin::Mutex;

static BATTERY_STATE: Mutex<BatteryInfo> = Mutex::new(BatteryInfo {
    percentage: 100,
    state: BatteryState::Unknown,
    time_remaining: None,
    time_to_full: None,
    power_draw: None,
    health: 100,
    design_capacity: 50000,
    full_capacity: 50000,
    cycle_count: 0,
    temperature: None,
    manufacturer: String::new(),
    model: String::new(),
});

/// Get battery percentage
pub fn get_percentage() -> u8 {
    BATTERY_STATE.lock().percentage
}

/// Get battery state
pub fn get_state() -> BatteryState {
    BATTERY_STATE.lock().state
}

/// Update battery info
pub fn update_battery(info: BatteryInfo) {
    *BATTERY_STATE.lock() = info;
}

/// Set battery percentage
pub fn set_percentage(percentage: u8) {
    BATTERY_STATE.lock().percentage = percentage.min(100);
}

/// Set battery state
pub fn set_state(state: BatteryState) {
    BATTERY_STATE.lock().state = state;
}

/// Check if battery is critical
pub fn is_critical() -> bool {
    let state = BATTERY_STATE.lock();
    state.state == BatteryState::Discharging && state.percentage < 10
}

/// Check if battery is low
pub fn is_low() -> bool {
    let state = BATTERY_STATE.lock();
    state.state == BatteryState::Discharging && state.percentage < 20
}
