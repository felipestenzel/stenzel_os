//! Clock Widget
//!
//! System clock display for the taskbar.

use alloc::string::String;
use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use super::{Widget, WidgetId, WidgetState, WidgetEvent, Bounds, MouseButton, theme};

/// Clock format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockFormat {
    /// 12-hour format (1:30 PM)
    Hour12,
    /// 24-hour format (13:30)
    Hour24,
}

/// Date format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateFormat {
    /// MM/DD/YYYY
    MDY,
    /// DD/MM/YYYY
    DMY,
    /// YYYY-MM-DD
    YMD,
}

/// Clock click callback
pub type ClockCallback = fn(WidgetId);

/// Clock widget for taskbar
pub struct Clock {
    id: WidgetId,
    bounds: Bounds,
    hour: u8,
    minute: u8,
    second: u8,
    day: u8,
    month: u8,
    year: u16,
    clock_format: ClockFormat,
    date_format: DateFormat,
    show_seconds: bool,
    show_date: bool,
    state: WidgetState,
    visible: bool,
    on_click: Option<ClockCallback>,
}

impl Clock {
    /// Create a new clock widget
    pub fn new(x: isize, y: isize) -> Self {
        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(x, y, 70, 40), // Width adjusted based on content
            hour: 12,
            minute: 0,
            second: 0,
            day: 1,
            month: 1,
            year: 2025,
            clock_format: ClockFormat::Hour24,
            date_format: DateFormat::YMD,
            show_seconds: false,
            show_date: true,
            state: WidgetState::Normal,
            visible: true,
            on_click: None,
        }
    }

    /// Set time
    pub fn set_time(&mut self, hour: u8, minute: u8, second: u8) {
        self.hour = hour % 24;
        self.minute = minute % 60;
        self.second = second % 60;
    }

    /// Set date
    pub fn set_date(&mut self, day: u8, month: u8, year: u16) {
        self.day = day.clamp(1, 31);
        self.month = month.clamp(1, 12);
        self.year = year;
    }

    /// Set clock format
    pub fn set_clock_format(&mut self, format: ClockFormat) {
        self.clock_format = format;
        self.update_width();
    }

    /// Set date format
    pub fn set_date_format(&mut self, format: DateFormat) {
        self.date_format = format;
        self.update_width();
    }

    /// Show/hide seconds
    pub fn set_show_seconds(&mut self, show: bool) {
        self.show_seconds = show;
        self.update_width();
    }

    /// Show/hide date
    pub fn set_show_date(&mut self, show: bool) {
        self.show_date = show;
        self.update_width();
    }

    /// Set click callback
    pub fn set_on_click(&mut self, callback: ClockCallback) {
        self.on_click = Some(callback);
    }

    /// Get formatted time string
    pub fn time_string(&self) -> String {
        let (hour, suffix) = match self.clock_format {
            ClockFormat::Hour12 => {
                let h = if self.hour == 0 {
                    12
                } else if self.hour > 12 {
                    self.hour - 12
                } else {
                    self.hour
                };
                let s = if self.hour >= 12 { " PM" } else { " AM" };
                (h, s)
            }
            ClockFormat::Hour24 => (self.hour, ""),
        };

        if self.show_seconds {
            format_time_with_seconds(hour, self.minute, self.second, suffix)
        } else {
            format_time(hour, self.minute, suffix)
        }
    }

    /// Get formatted date string
    pub fn date_string(&self) -> String {
        match self.date_format {
            DateFormat::MDY => format_date_mdy(self.month, self.day, self.year),
            DateFormat::DMY => format_date_dmy(self.day, self.month, self.year),
            DateFormat::YMD => format_date_ymd(self.year, self.month, self.day),
        }
    }

    /// Update width based on content
    fn update_width(&mut self) {
        let time_len = if self.show_seconds { 8 } else { 5 };
        let suffix_len = if self.clock_format == ClockFormat::Hour12 { 3 } else { 0 };
        let date_len = if self.show_date { 10 } else { 0 };

        let max_len = (time_len + suffix_len).max(date_len);
        self.bounds.width = max_len * 8 + 16; // 8px per char + padding
    }

    /// Tick one second
    pub fn tick(&mut self) {
        self.second += 1;
        if self.second >= 60 {
            self.second = 0;
            self.minute += 1;
            if self.minute >= 60 {
                self.minute = 0;
                self.hour += 1;
                if self.hour >= 24 {
                    self.hour = 0;
                    // Would advance date here
                    self.day += 1;
                    if self.day > days_in_month(self.month, self.year) {
                        self.day = 1;
                        self.month += 1;
                        if self.month > 12 {
                            self.month = 1;
                            self.year += 1;
                        }
                    }
                }
            }
        }
    }

    /// Sync with RTC
    pub fn sync_from_rtc(&mut self, rtc_time: u64) {
        // Simple conversion from Unix timestamp
        // This is a simplified implementation
        let secs_per_min = 60u64;
        let secs_per_hour = 3600u64;
        let secs_per_day = 86400u64;

        let days_since_epoch = rtc_time / secs_per_day;
        let remaining_secs = rtc_time % secs_per_day;

        self.hour = (remaining_secs / secs_per_hour) as u8;
        self.minute = ((remaining_secs % secs_per_hour) / secs_per_min) as u8;
        self.second = (remaining_secs % secs_per_min) as u8;

        // Calculate date from days since 1970-01-01
        let (year, month, day) = days_to_date(days_since_epoch);
        self.year = year;
        self.month = month;
        self.day = day;
    }
}

impl Widget for Clock {
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
                    if let Some(callback) = self.on_click {
                        callback(self.id);
                    }
                    self.state = WidgetState::Hovered;
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

        // Draw hover background
        if self.state == WidgetState::Hovered || self.state == WidgetState::Pressed {
            let bg = if self.state == WidgetState::Pressed {
                theme.bg_pressed
            } else {
                theme.bg_hover
            };
            for py in 2..h - 2 {
                for px in 2..w - 2 {
                    surface.set_pixel(x + px, y + py, bg);
                }
            }
        }

        let time_str = self.time_string();
        let date_str = self.date_string();

        let text_color = theme.fg;

        if self.show_date {
            // Two-line display: time on top, date below
            let time_y = y + 5;
            let date_y = y + 22;

            // Center time
            let time_x = x + (w - time_str.chars().count() * 8) / 2;
            for (i, c) in time_str.chars().enumerate() {
                draw_char_simple(surface, time_x + i * 8, time_y, c, text_color);
            }

            // Center date
            let date_x = x + (w - date_str.chars().count() * 8) / 2;
            for (i, c) in date_str.chars().enumerate() {
                draw_char_simple(surface, date_x + i * 8, date_y, c, text_color);
            }
        } else {
            // Single line display, centered vertically
            let time_y = y + (h - 16) / 2;
            let time_x = x + (w - time_str.chars().count() * 8) / 2;
            for (i, c) in time_str.chars().enumerate() {
                draw_char_simple(surface, time_x + i * 8, time_y, c, text_color);
            }
        }
    }
}

// Helper functions

fn format_time(hour: u8, minute: u8, suffix: &str) -> String {
    use alloc::string::ToString;
    let mut s = String::new();
    if hour < 10 { s.push('0'); }
    s.push_str(&hour.to_string());
    s.push(':');
    if minute < 10 { s.push('0'); }
    s.push_str(&minute.to_string());
    s.push_str(suffix);
    s
}

fn format_time_with_seconds(hour: u8, minute: u8, second: u8, suffix: &str) -> String {
    use alloc::string::ToString;
    let mut s = String::new();
    if hour < 10 { s.push('0'); }
    s.push_str(&hour.to_string());
    s.push(':');
    if minute < 10 { s.push('0'); }
    s.push_str(&minute.to_string());
    s.push(':');
    if second < 10 { s.push('0'); }
    s.push_str(&second.to_string());
    s.push_str(suffix);
    s
}

fn format_date_mdy(month: u8, day: u8, year: u16) -> String {
    use alloc::string::ToString;
    let mut s = String::new();
    if month < 10 { s.push('0'); }
    s.push_str(&month.to_string());
    s.push('/');
    if day < 10 { s.push('0'); }
    s.push_str(&day.to_string());
    s.push('/');
    s.push_str(&year.to_string());
    s
}

fn format_date_dmy(day: u8, month: u8, year: u16) -> String {
    use alloc::string::ToString;
    let mut s = String::new();
    if day < 10 { s.push('0'); }
    s.push_str(&day.to_string());
    s.push('/');
    if month < 10 { s.push('0'); }
    s.push_str(&month.to_string());
    s.push('/');
    s.push_str(&year.to_string());
    s
}

fn format_date_ymd(year: u16, month: u8, day: u8) -> String {
    use alloc::string::ToString;
    let mut s = String::new();
    s.push_str(&year.to_string());
    s.push('-');
    if month < 10 { s.push('0'); }
    s.push_str(&month.to_string());
    s.push('-');
    if day < 10 { s.push('0'); }
    s.push_str(&day.to_string());
    s
}

fn is_leap_year(year: u16) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

fn days_in_month(month: u8, year: u16) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => if is_leap_year(year) { 29 } else { 28 },
        _ => 30,
    }
}

fn days_to_date(days: u64) -> (u16, u8, u8) {
    let mut year = 1970u16;
    let mut remaining = days;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        year += 1;
    }

    let mut month = 1u8;
    loop {
        let dim = days_in_month(month, year) as u64;
        if remaining < dim {
            break;
        }
        remaining -= dim;
        month += 1;
    }

    (year, month, remaining as u8 + 1)
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
