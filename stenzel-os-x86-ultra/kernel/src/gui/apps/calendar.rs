//! Calendar Application
//!
//! Full-featured calendar with events, reminders, and multiple views.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use alloc::collections::BTreeMap;

use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use crate::gui::widgets::{Widget, WidgetId, WidgetEvent, Bounds, MouseButton, theme};

/// Days of the week
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weekday {
    Sunday,
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
}

impl Weekday {
    pub fn name(&self) -> &'static str {
        match self {
            Weekday::Sunday => "Sunday",
            Weekday::Monday => "Monday",
            Weekday::Tuesday => "Tuesday",
            Weekday::Wednesday => "Wednesday",
            Weekday::Thursday => "Thursday",
            Weekday::Friday => "Friday",
            Weekday::Saturday => "Saturday",
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            Weekday::Sunday => "Sun",
            Weekday::Monday => "Mon",
            Weekday::Tuesday => "Tue",
            Weekday::Wednesday => "Wed",
            Weekday::Thursday => "Thu",
            Weekday::Friday => "Fri",
            Weekday::Saturday => "Sat",
        }
    }

    pub fn from_number(n: u8) -> Self {
        match n % 7 {
            0 => Weekday::Sunday,
            1 => Weekday::Monday,
            2 => Weekday::Tuesday,
            3 => Weekday::Wednesday,
            4 => Weekday::Thursday,
            5 => Weekday::Friday,
            _ => Weekday::Saturday,
        }
    }

    pub fn as_number(&self) -> u8 {
        match self {
            Weekday::Sunday => 0,
            Weekday::Monday => 1,
            Weekday::Tuesday => 2,
            Weekday::Wednesday => 3,
            Weekday::Thursday => 4,
            Weekday::Friday => 5,
            Weekday::Saturday => 6,
        }
    }
}

/// Month names
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Month {
    January,
    February,
    March,
    April,
    May,
    June,
    July,
    August,
    September,
    October,
    November,
    December,
}

impl Month {
    pub fn name(&self) -> &'static str {
        match self {
            Month::January => "January",
            Month::February => "February",
            Month::March => "March",
            Month::April => "April",
            Month::May => "May",
            Month::June => "June",
            Month::July => "July",
            Month::August => "August",
            Month::September => "September",
            Month::October => "October",
            Month::November => "November",
            Month::December => "December",
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            Month::January => "Jan",
            Month::February => "Feb",
            Month::March => "Mar",
            Month::April => "Apr",
            Month::May => "May",
            Month::June => "Jun",
            Month::July => "Jul",
            Month::August => "Aug",
            Month::September => "Sep",
            Month::October => "Oct",
            Month::November => "Nov",
            Month::December => "Dec",
        }
    }

    pub fn from_number(n: u8) -> Self {
        match n {
            1 => Month::January,
            2 => Month::February,
            3 => Month::March,
            4 => Month::April,
            5 => Month::May,
            6 => Month::June,
            7 => Month::July,
            8 => Month::August,
            9 => Month::September,
            10 => Month::October,
            11 => Month::November,
            _ => Month::December,
        }
    }

    pub fn as_number(&self) -> u8 {
        match self {
            Month::January => 1,
            Month::February => 2,
            Month::March => 3,
            Month::April => 4,
            Month::May => 5,
            Month::June => 6,
            Month::July => 7,
            Month::August => 8,
            Month::September => 9,
            Month::October => 10,
            Month::November => 11,
            Month::December => 12,
        }
    }

    pub fn days(&self, year: u16) -> u8 {
        match self {
            Month::January | Month::March | Month::May | Month::July |
            Month::August | Month::October | Month::December => 31,
            Month::April | Month::June | Month::September | Month::November => 30,
            Month::February => {
                if is_leap_year(year) { 29 } else { 28 }
            }
        }
    }
}

/// Check if a year is a leap year
pub fn is_leap_year(year: u16) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Date structure
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

impl Date {
    pub fn new(year: u16, month: u8, day: u8) -> Self {
        Self { year, month, day }
    }

    pub fn today() -> Self {
        // For now, return a fixed date - would need RTC in real implementation
        Self::new(2026, 1, 18)
    }

    pub fn month_enum(&self) -> Month {
        Month::from_number(self.month)
    }

    pub fn weekday(&self) -> Weekday {
        // Zeller's formula
        let m = if self.month < 3 { self.month + 12 } else { self.month } as i32;
        let y = if self.month < 3 { self.year - 1 } else { self.year } as i32;
        let k = y % 100;
        let j = y / 100;
        let d = self.day as i32;

        let h = (d + (13 * (m + 1)) / 5 + k + k / 4 + j / 4 - 2 * j) % 7;
        let h = ((h + 7) % 7) as u8;

        // Convert: 0=Sat, 1=Sun, 2=Mon, etc. to our format: 0=Sun, 1=Mon, etc.
        Weekday::from_number(if h == 0 { 6 } else { h - 1 })
    }

    pub fn days_in_month(&self) -> u8 {
        self.month_enum().days(self.year)
    }

    pub fn first_day_of_month(&self) -> Date {
        Date::new(self.year, self.month, 1)
    }

    pub fn format(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    pub fn format_display(&self) -> String {
        format!("{} {}, {}", self.month_enum().name(), self.day, self.year)
    }

    pub fn add_days(&self, days: i32) -> Self {
        let mut year = self.year;
        let mut month = self.month;
        let mut day = self.day as i32 + days;

        while day > Month::from_number(month).days(year) as i32 {
            day -= Month::from_number(month).days(year) as i32;
            month += 1;
            if month > 12 {
                month = 1;
                year += 1;
            }
        }

        while day < 1 {
            month -= 1;
            if month < 1 {
                month = 12;
                year -= 1;
            }
            day += Month::from_number(month).days(year) as i32;
        }

        Date::new(year, month, day as u8)
    }

    pub fn add_months(&self, months: i32) -> Self {
        let mut year = self.year as i32;
        let mut month = self.month as i32 + months;

        while month > 12 {
            month -= 12;
            year += 1;
        }
        while month < 1 {
            month += 12;
            year -= 1;
        }

        let max_day = Month::from_number(month as u8).days(year as u16);
        let day = self.day.min(max_day);

        Date::new(year as u16, month as u8, day)
    }
}

/// Time structure (24-hour format)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Time {
    pub hour: u8,
    pub minute: u8,
}

impl Time {
    pub fn new(hour: u8, minute: u8) -> Self {
        Self {
            hour: hour.min(23),
            minute: minute.min(59),
        }
    }

    pub fn format(&self) -> String {
        format!("{:02}:{:02}", self.hour, self.minute)
    }

    pub fn format_12h(&self) -> String {
        let (h, ampm) = if self.hour == 0 {
            (12, "AM")
        } else if self.hour < 12 {
            (self.hour, "AM")
        } else if self.hour == 12 {
            (12, "PM")
        } else {
            (self.hour - 12, "PM")
        };
        format!("{}:{:02} {}", h, self.minute, ampm)
    }

    pub fn total_minutes(&self) -> u16 {
        self.hour as u16 * 60 + self.minute as u16
    }
}

/// DateTime combination
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateTime {
    pub date: Date,
    pub time: Time,
}

impl DateTime {
    pub fn new(date: Date, time: Time) -> Self {
        Self { date, time }
    }

    pub fn format(&self) -> String {
        format!("{} {}", self.date.format(), self.time.format())
    }
}

/// Event recurrence rule
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecurrenceRule {
    None,
    Daily,
    Weekly,
    Biweekly,
    Monthly,
    Yearly,
    Custom { days: u16 },
}

impl RecurrenceRule {
    pub fn name(&self) -> &'static str {
        match self {
            RecurrenceRule::None => "Does not repeat",
            RecurrenceRule::Daily => "Daily",
            RecurrenceRule::Weekly => "Weekly",
            RecurrenceRule::Biweekly => "Every 2 weeks",
            RecurrenceRule::Monthly => "Monthly",
            RecurrenceRule::Yearly => "Yearly",
            RecurrenceRule::Custom { .. } => "Custom",
        }
    }
}

/// Reminder time before event
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReminderTime {
    AtTime,
    Minutes5,
    Minutes10,
    Minutes15,
    Minutes30,
    Hour1,
    Hours2,
    Day1,
    Days2,
    Week1,
}

impl ReminderTime {
    pub fn name(&self) -> &'static str {
        match self {
            ReminderTime::AtTime => "At time of event",
            ReminderTime::Minutes5 => "5 minutes before",
            ReminderTime::Minutes10 => "10 minutes before",
            ReminderTime::Minutes15 => "15 minutes before",
            ReminderTime::Minutes30 => "30 minutes before",
            ReminderTime::Hour1 => "1 hour before",
            ReminderTime::Hours2 => "2 hours before",
            ReminderTime::Day1 => "1 day before",
            ReminderTime::Days2 => "2 days before",
            ReminderTime::Week1 => "1 week before",
        }
    }

    pub fn minutes(&self) -> u32 {
        match self {
            ReminderTime::AtTime => 0,
            ReminderTime::Minutes5 => 5,
            ReminderTime::Minutes10 => 10,
            ReminderTime::Minutes15 => 15,
            ReminderTime::Minutes30 => 30,
            ReminderTime::Hour1 => 60,
            ReminderTime::Hours2 => 120,
            ReminderTime::Day1 => 1440,
            ReminderTime::Days2 => 2880,
            ReminderTime::Week1 => 10080,
        }
    }
}

/// Calendar color for event display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventColor {
    Blue,
    Green,
    Red,
    Yellow,
    Purple,
    Orange,
    Cyan,
    Pink,
    Gray,
}

impl EventColor {
    pub fn to_color(&self) -> Color {
        match self {
            EventColor::Blue => Color::new(66, 133, 244),
            EventColor::Green => Color::new(52, 168, 83),
            EventColor::Red => Color::new(234, 67, 53),
            EventColor::Yellow => Color::new(251, 188, 4),
            EventColor::Purple => Color::new(142, 68, 173),
            EventColor::Orange => Color::new(230, 126, 34),
            EventColor::Cyan => Color::new(26, 188, 156),
            EventColor::Pink => Color::new(232, 67, 147),
            EventColor::Gray => Color::new(128, 128, 128),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            EventColor::Blue => "Blue",
            EventColor::Green => "Green",
            EventColor::Red => "Red",
            EventColor::Yellow => "Yellow",
            EventColor::Purple => "Purple",
            EventColor::Orange => "Orange",
            EventColor::Cyan => "Cyan",
            EventColor::Pink => "Pink",
            EventColor::Gray => "Gray",
        }
    }
}

/// Calendar event
#[derive(Debug, Clone)]
pub struct CalendarEvent {
    pub id: u64,
    pub calendar_id: u64,
    pub title: String,
    pub description: String,
    pub location: String,
    pub start: DateTime,
    pub end: DateTime,
    pub all_day: bool,
    pub color: EventColor,
    pub recurrence: RecurrenceRule,
    pub reminder: Option<ReminderTime>,
    pub attendees: Vec<String>,
    pub created: u64,
    pub modified: u64,
    pub is_busy: bool,
}

impl CalendarEvent {
    pub fn new(title: &str, start: DateTime, end: DateTime) -> Self {
        Self {
            id: 0,
            calendar_id: 0,
            title: title.to_string(),
            description: String::new(),
            location: String::new(),
            start,
            end,
            all_day: false,
            color: EventColor::Blue,
            recurrence: RecurrenceRule::None,
            reminder: Some(ReminderTime::Minutes15),
            attendees: Vec::new(),
            created: 0,
            modified: 0,
            is_busy: true,
        }
    }

    pub fn duration_minutes(&self) -> u32 {
        if self.all_day {
            1440 // Full day
        } else {
            let start_mins = self.start.time.total_minutes() as u32;
            let end_mins = self.end.time.total_minutes() as u32;
            if end_mins > start_mins {
                end_mins - start_mins
            } else {
                1440 - start_mins + end_mins // Crosses midnight
            }
        }
    }

    pub fn format_time_range(&self) -> String {
        if self.all_day {
            "All day".to_string()
        } else {
            format!("{} - {}", self.start.time.format_12h(), self.end.time.format_12h())
        }
    }

    pub fn is_on_date(&self, date: Date) -> bool {
        self.start.date == date ||
        self.end.date == date ||
        (self.start.date < date && self.end.date > date)
    }
}

/// Calendar (group of events)
#[derive(Debug, Clone)]
pub struct Calendar {
    pub id: u64,
    pub name: String,
    pub color: EventColor,
    pub is_visible: bool,
    pub is_default: bool,
    pub is_local: bool,
    pub account_email: Option<String>,
}

impl Calendar {
    pub fn new(name: &str, color: EventColor) -> Self {
        Self {
            id: 0,
            name: name.to_string(),
            color,
            is_visible: true,
            is_default: false,
            is_local: true,
            account_email: None,
        }
    }
}

/// View mode for the calendar
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalendarView {
    Day,
    Week,
    Month,
    Year,
    Agenda,
}

impl CalendarView {
    pub fn name(&self) -> &'static str {
        match self {
            CalendarView::Day => "Day",
            CalendarView::Week => "Week",
            CalendarView::Month => "Month",
            CalendarView::Year => "Year",
            CalendarView::Agenda => "Agenda",
        }
    }
}

/// Week start day setting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeekStart {
    Sunday,
    Monday,
    Saturday,
}

/// Calendar settings
#[derive(Debug, Clone)]
pub struct CalendarSettings {
    pub week_start: WeekStart,
    pub show_week_numbers: bool,
    pub time_format_24h: bool,
    pub default_view: CalendarView,
    pub default_event_duration: u32,
    pub working_hours_start: u8,
    pub working_hours_end: u8,
    pub show_declined_events: bool,
}

impl Default for CalendarSettings {
    fn default() -> Self {
        Self {
            week_start: WeekStart::Sunday,
            show_week_numbers: false,
            time_format_24h: false,
            default_view: CalendarView::Month,
            default_event_duration: 60,
            working_hours_start: 9,
            working_hours_end: 17,
            show_declined_events: false,
        }
    }
}

/// Calendar widget
pub struct CalendarWidget {
    id: WidgetId,
    bounds: Bounds,
    enabled: bool,
    visible: bool,

    // Data
    calendars: Vec<Calendar>,
    events: Vec<CalendarEvent>,
    next_calendar_id: u64,
    next_event_id: u64,

    // View state
    current_date: Date,
    selected_date: Date,
    view: CalendarView,
    settings: CalendarSettings,

    // UI state
    sidebar_width: usize,
    selected_event_id: Option<u64>,
    hovered_date: Option<Date>,
    scroll_offset: usize,
    show_event_editor: bool,
    editing_event: Option<CalendarEvent>,
}

impl CalendarWidget {
    pub fn new(id: WidgetId) -> Self {
        let today = Date::today();

        let mut widget = Self {
            id,
            bounds: Bounds { x: 0, y: 0, width: 800, height: 600 },
            enabled: true,
            visible: true,
            calendars: Vec::new(),
            events: Vec::new(),
            next_calendar_id: 1,
            next_event_id: 1,
            current_date: today,
            selected_date: today,
            view: CalendarView::Month,
            settings: CalendarSettings::default(),
            sidebar_width: 200,
            selected_event_id: None,
            hovered_date: None,
            scroll_offset: 0,
            show_event_editor: false,
            editing_event: None,
        };

        widget.add_sample_data();
        widget
    }

    fn add_sample_data(&mut self) {
        // Add default calendar
        let mut cal = Calendar::new("Personal", EventColor::Blue);
        cal.id = self.next_calendar_id;
        cal.is_default = true;
        self.next_calendar_id += 1;
        let cal_id = cal.id;
        self.calendars.push(cal);

        // Add work calendar
        let mut work_cal = Calendar::new("Work", EventColor::Green);
        work_cal.id = self.next_calendar_id;
        self.next_calendar_id += 1;
        let work_cal_id = work_cal.id;
        self.calendars.push(work_cal);

        // Add sample events
        let today = Date::today();

        let sample_events = [
            ("Team Meeting", today, Time::new(10, 0), Time::new(11, 0), work_cal_id, EventColor::Green, false),
            ("Lunch with Alice", today, Time::new(12, 30), Time::new(13, 30), cal_id, EventColor::Blue, false),
            ("Project Deadline", today.add_days(2), Time::new(0, 0), Time::new(0, 0), work_cal_id, EventColor::Red, true),
            ("Birthday Party", today.add_days(5), Time::new(18, 0), Time::new(22, 0), cal_id, EventColor::Purple, false),
            ("Weekly Sync", today.add_days(1), Time::new(9, 0), Time::new(9, 30), work_cal_id, EventColor::Green, false),
            ("Dentist Appointment", today.add_days(3), Time::new(14, 0), Time::new(15, 0), cal_id, EventColor::Orange, false),
        ];

        for (title, date, start_time, end_time, cal_id, color, all_day) in sample_events {
            let mut event = CalendarEvent::new(
                title,
                DateTime::new(date, start_time),
                DateTime::new(date, end_time),
            );
            event.id = self.next_event_id;
            self.next_event_id += 1;
            event.calendar_id = cal_id;
            event.color = color;
            event.all_day = all_day;
            self.events.push(event);
        }
    }

    /// Add a new calendar
    pub fn add_calendar(&mut self, mut calendar: Calendar) {
        calendar.id = self.next_calendar_id;
        self.next_calendar_id += 1;
        if self.calendars.is_empty() {
            calendar.is_default = true;
        }
        self.calendars.push(calendar);
    }

    /// Remove a calendar and its events
    pub fn remove_calendar(&mut self, calendar_id: u64) {
        self.calendars.retain(|c| c.id != calendar_id);
        self.events.retain(|e| e.calendar_id != calendar_id);
    }

    /// Add a new event
    pub fn add_event(&mut self, mut event: CalendarEvent) {
        event.id = self.next_event_id;
        self.next_event_id += 1;
        if event.calendar_id == 0 {
            event.calendar_id = self.calendars.iter()
                .find(|c| c.is_default)
                .map(|c| c.id)
                .unwrap_or(1);
        }
        self.events.push(event);
    }

    /// Remove an event
    pub fn remove_event(&mut self, event_id: u64) {
        self.events.retain(|e| e.id != event_id);
        if self.selected_event_id == Some(event_id) {
            self.selected_event_id = None;
        }
    }

    /// Get events for a specific date
    pub fn events_for_date(&self, date: Date) -> Vec<&CalendarEvent> {
        self.events.iter()
            .filter(|e| {
                let calendar = self.calendars.iter().find(|c| c.id == e.calendar_id);
                calendar.map(|c| c.is_visible).unwrap_or(false) && e.is_on_date(date)
            })
            .collect()
    }

    /// Go to today
    pub fn go_to_today(&mut self) {
        self.current_date = Date::today();
        self.selected_date = self.current_date;
    }

    /// Navigate to previous period
    pub fn previous(&mut self) {
        match self.view {
            CalendarView::Day => self.current_date = self.current_date.add_days(-1),
            CalendarView::Week => self.current_date = self.current_date.add_days(-7),
            CalendarView::Month => self.current_date = self.current_date.add_months(-1),
            CalendarView::Year => self.current_date = Date::new(self.current_date.year - 1, self.current_date.month, self.current_date.day),
            CalendarView::Agenda => self.current_date = self.current_date.add_days(-7),
        }
    }

    /// Navigate to next period
    pub fn next(&mut self) {
        match self.view {
            CalendarView::Day => self.current_date = self.current_date.add_days(1),
            CalendarView::Week => self.current_date = self.current_date.add_days(7),
            CalendarView::Month => self.current_date = self.current_date.add_months(1),
            CalendarView::Year => self.current_date = Date::new(self.current_date.year + 1, self.current_date.month, self.current_date.day),
            CalendarView::Agenda => self.current_date = self.current_date.add_days(7),
        }
    }

    /// Set view mode
    pub fn set_view(&mut self, view: CalendarView) {
        self.view = view;
        self.scroll_offset = 0;
    }

    /// Select a date
    pub fn select_date(&mut self, date: Date) {
        self.selected_date = date;
        self.current_date = date;
    }

    /// Get the title for current view
    fn get_view_title(&self) -> String {
        match self.view {
            CalendarView::Day => self.current_date.format_display(),
            CalendarView::Week => {
                let week_start = self.get_week_start();
                let week_end = week_start.add_days(6);
                if week_start.month == week_end.month {
                    format!("{} {}-{}, {}", week_start.month_enum().name(), week_start.day, week_end.day, week_start.year)
                } else {
                    format!("{} {} - {} {}", week_start.month_enum().short_name(), week_start.day, week_end.month_enum().short_name(), week_end.day)
                }
            }
            CalendarView::Month => format!("{} {}", self.current_date.month_enum().name(), self.current_date.year),
            CalendarView::Year => format!("{}", self.current_date.year),
            CalendarView::Agenda => "Upcoming Events".to_string(),
        }
    }

    /// Get the start of the current week
    fn get_week_start(&self) -> Date {
        let weekday = self.current_date.weekday().as_number();
        let days_back = match self.settings.week_start {
            WeekStart::Sunday => weekday,
            WeekStart::Monday => (weekday + 6) % 7,
            WeekStart::Saturday => (weekday + 1) % 7,
        };
        self.current_date.add_days(-(days_back as i32))
    }

    fn date_at_point_month(&self, x: isize, y: isize) -> Option<Date> {
        let grid_x = self.bounds.x + self.sidebar_width as isize + 10;
        let grid_y = self.bounds.y + 80;
        let grid_width = self.bounds.width.saturating_sub(self.sidebar_width + 20);
        let grid_height = self.bounds.height.saturating_sub(100);

        if x < grid_x || x >= grid_x + grid_width as isize ||
           y < grid_y || y >= grid_y + grid_height as isize {
            return None;
        }

        let cell_width = grid_width / 7;
        let cell_height = grid_height / 6;

        let col = ((x - grid_x) as usize / cell_width) as i32;
        let row = ((y - grid_y) as usize / cell_height) as i32;

        let first_day = self.current_date.first_day_of_month();
        let first_weekday = first_day.weekday().as_number() as i32;
        let day_offset = row * 7 + col - first_weekday;

        Some(first_day.add_days(day_offset))
    }
}

fn draw_char_at(surface: &mut Surface, x: usize, y: usize, c: char, color: Color) {
    use crate::drivers::font::DEFAULT_FONT;
    if let Some(glyph) = DEFAULT_FONT.get_glyph(c) {
        for row in 0..DEFAULT_FONT.height {
            let byte = glyph[row];
            for col in 0..DEFAULT_FONT.width {
                if (byte >> (7 - col)) & 1 != 0 {
                    surface.set_pixel(x + col, y + row, color);
                }
            }
        }
    }
}

fn draw_char(surface: &mut Surface, x: isize, y: isize, c: char, color: Color) {
    if x >= 0 && y >= 0 {
        draw_char_at(surface, x as usize, y as usize, c, color);
    }
}

fn draw_string(surface: &mut Surface, x: isize, y: isize, s: &str, color: Color) {
    if x < 0 || y < 0 {
        return;
    }
    let mut px = x as usize;
    for c in s.chars() {
        draw_char_at(surface, px, y as usize, c, color);
        px += 8;
    }
}

impl Widget for CalendarWidget {
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
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn handle_event(&mut self, event: &WidgetEvent) -> bool {
        match event {
            WidgetEvent::MouseDown { x, y, button } => {
                if *button == MouseButton::Left {
                    // Check toolbar buttons
                    let toolbar_y = self.bounds.y;
                    let toolbar_x = self.bounds.x + self.sidebar_width as isize;

                    if *y >= toolbar_y && *y < toolbar_y + 40 {
                        // Today button
                        if *x >= toolbar_x && *x < toolbar_x + 60 {
                            self.go_to_today();
                            return true;
                        }
                        // Previous button
                        if *x >= toolbar_x + 70 && *x < toolbar_x + 90 {
                            self.previous();
                            return true;
                        }
                        // Next button
                        if *x >= toolbar_x + 100 && *x < toolbar_x + 120 {
                            self.next();
                            return true;
                        }
                        // View buttons
                        if *x >= toolbar_x + 300 {
                            let btn_offset = (*x - (toolbar_x + 300)) / 50;
                            match btn_offset {
                                0 => self.set_view(CalendarView::Day),
                                1 => self.set_view(CalendarView::Week),
                                2 => self.set_view(CalendarView::Month),
                                _ => {}
                            }
                            return true;
                        }
                    }

                    // Check date click in month view
                    if self.view == CalendarView::Month {
                        if let Some(date) = self.date_at_point_month(*x, *y) {
                            self.select_date(date);
                            return true;
                        }
                    }
                }
                false
            }

            WidgetEvent::MouseMove { x, y } => {
                if self.view == CalendarView::Month {
                    self.hovered_date = self.date_at_point_month(*x, *y);
                }
                true
            }

            WidgetEvent::KeyDown { key, .. } => {
                match *key {
                    0x4B => { // Left
                        self.selected_date = self.selected_date.add_days(-1);
                        true
                    }
                    0x4D => { // Right
                        self.selected_date = self.selected_date.add_days(1);
                        true
                    }
                    0x48 => { // Up
                        self.selected_date = self.selected_date.add_days(-7);
                        true
                    }
                    0x50 => { // Down
                        self.selected_date = self.selected_date.add_days(7);
                        true
                    }
                    _ => false,
                }
            }

            _ => false,
        }
    }

    fn render(&self, surface: &mut Surface) {
        let _theme = theme();
        let bg_color = Color::new(30, 30, 35);
        let sidebar_bg = Color::new(25, 25, 30);
        let text_color = Color::new(220, 220, 220);
        let dim_text = Color::new(150, 150, 150);
        let accent_color = Color::new(66, 133, 244);
        let border_color = Color::new(60, 60, 65);
        let today_bg = Color::new(66, 133, 244);

        // Clear background
        for y in 0..self.bounds.height {
            for x in 0..self.bounds.width {
                let px = self.bounds.x + x as isize;
                let py = self.bounds.y + y as isize;
                surface.set_pixel(px as usize, py as usize, bg_color);
            }
        }

        // Draw sidebar
        for y in 0..self.bounds.height {
            for x in 0..self.sidebar_width {
                let px = self.bounds.x + x as isize;
                let py = self.bounds.y + y as isize;
                surface.set_pixel(px as usize, py as usize, sidebar_bg);
            }
        }

        // Sidebar border
        for y in 0..self.bounds.height {
            let px = self.bounds.x + self.sidebar_width as isize;
            let py = self.bounds.y + y as isize;
            surface.set_pixel(px as usize, py as usize, border_color);
        }

        // Mini calendar in sidebar
        self.render_mini_calendar(surface, self.bounds.x + 10, self.bounds.y + 10);

        // Calendars list
        let cal_y = self.bounds.y + 180;
        draw_string(surface, self.bounds.x + 10, cal_y, "Calendars", text_color);

        for (i, calendar) in self.calendars.iter().enumerate() {
            let y = cal_y + 25 + (i * 22) as isize;

            // Visibility checkbox
            let checkbox_char = if calendar.is_visible { 'x' } else { ' ' };
            draw_char(surface, self.bounds.x + 10, y, '[', dim_text);
            draw_char(surface, self.bounds.x + 18, y, checkbox_char, calendar.color.to_color());
            draw_char(surface, self.bounds.x + 26, y, ']', dim_text);

            // Calendar name
            draw_string(surface, self.bounds.x + 40, y, &calendar.name, text_color);
        }

        // Main content area
        let content_x = self.bounds.x + self.sidebar_width as isize + 10;
        let content_y = self.bounds.y;

        // Toolbar
        draw_string(surface, content_x, content_y + 12, "[Today]", accent_color);
        draw_string(surface, content_x + 70, content_y + 12, "<", text_color);
        draw_string(surface, content_x + 100, content_y + 12, ">", text_color);

        // Title
        let title = self.get_view_title();
        draw_string(surface, content_x + 140, content_y + 12, &title, text_color);

        // View buttons
        let views = [("Day", CalendarView::Day), ("Week", CalendarView::Week), ("Month", CalendarView::Month)];
        for (i, (name, view)) in views.iter().enumerate() {
            let btn_x = content_x + 300 + (i * 50) as isize;
            let color = if self.view == *view { accent_color } else { dim_text };
            draw_string(surface, btn_x, content_y + 12, name, color);
        }

        // Toolbar separator
        for x in self.sidebar_width..self.bounds.width {
            let px = self.bounds.x + x as isize;
            let py = self.bounds.y + 39;
            surface.set_pixel(px as usize, py as usize, border_color);
        }

        // Render view
        match self.view {
            CalendarView::Month => self.render_month_view(surface),
            CalendarView::Day => self.render_day_view(surface),
            CalendarView::Week => self.render_week_view(surface),
            CalendarView::Agenda => self.render_agenda_view(surface),
            _ => {}
        }
    }
}

impl CalendarWidget {
    fn render_mini_calendar(&self, surface: &mut Surface, x: isize, y: isize) {
        let text_color = Color::new(220, 220, 220);
        let dim_text = Color::new(100, 100, 100);
        let today_bg = Color::new(66, 133, 244);
        let selected_bg = Color::new(80, 80, 85);

        // Month/Year header
        let header = format!("{} {}", self.current_date.month_enum().short_name(), self.current_date.year);
        draw_string(surface, x, y, &header, text_color);

        // Day headers
        let days = ["S", "M", "T", "W", "T", "F", "S"];
        for (i, day) in days.iter().enumerate() {
            draw_string(surface, x + (i * 22) as isize, y + 20, day, dim_text);
        }

        // Days grid
        let first_day = self.current_date.first_day_of_month();
        let first_weekday = first_day.weekday().as_number();
        let days_in_month = self.current_date.days_in_month();
        let today = Date::today();

        for day in 1..=days_in_month {
            let day_offset = first_weekday as usize + day as usize - 1;
            let col = day_offset % 7;
            let row = day_offset / 7;

            let dx = x + (col * 22) as isize;
            let dy = y + 40 + (row * 18) as isize;

            let current_day = Date::new(self.current_date.year, self.current_date.month, day);

            // Background for today or selected
            if current_day == today {
                for by in 0..16 {
                    for bx in 0..18 {
                        surface.set_pixel((dx + bx) as usize, (dy + by) as usize, today_bg);
                    }
                }
            } else if current_day == self.selected_date {
                for by in 0..16 {
                    for bx in 0..18 {
                        surface.set_pixel((dx + bx) as usize, (dy + by) as usize, selected_bg);
                    }
                }
            }

            let day_str = format!("{:2}", day);
            let color = if current_day == today { Color::new(255, 255, 255) } else { text_color };
            draw_string(surface, dx + 2, dy + 2, &day_str, color);
        }
    }

    fn render_month_view(&self, surface: &mut Surface) {
        let text_color = Color::new(220, 220, 220);
        let dim_text = Color::new(100, 100, 100);
        let border_color = Color::new(60, 60, 65);
        let today_bg = Color::new(66, 133, 244);
        let hover_bg = Color::new(50, 50, 55);
        let selected_bg = Color::new(45, 85, 150);

        let grid_x = self.bounds.x + self.sidebar_width as isize + 10;
        let grid_y = self.bounds.y + 50;
        let grid_width = self.bounds.width.saturating_sub(self.sidebar_width + 20);
        let grid_height = self.bounds.height.saturating_sub(70);

        let cell_width = grid_width / 7;
        let cell_height = grid_height / 7; // 1 header row + 6 week rows

        // Day headers
        let days = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
        for (i, day) in days.iter().enumerate() {
            let x = grid_x + (i * cell_width) as isize + (cell_width / 2 - 12) as isize;
            draw_string(surface, x, grid_y + 5, day, dim_text);
        }

        // Header separator
        for x in 0..grid_width {
            let px = grid_x + x as isize;
            let py = grid_y + cell_height as isize - 1;
            surface.set_pixel(px as usize, py as usize, border_color);
        }

        // Days grid
        let first_day = self.current_date.first_day_of_month();
        let first_weekday = first_day.weekday().as_number();
        let days_in_month = self.current_date.days_in_month();
        let today = Date::today();

        for day in 1..=days_in_month {
            let day_offset = first_weekday as usize + day as usize - 1;
            let col = day_offset % 7;
            let row = day_offset / 7;

            let cell_x = grid_x + (col * cell_width) as isize;
            let cell_y = grid_y + ((row + 1) * cell_height) as isize;

            let current_day = Date::new(self.current_date.year, self.current_date.month, day);

            // Cell background
            let bg = if current_day == today {
                today_bg
            } else if current_day == self.selected_date {
                selected_bg
            } else if self.hovered_date == Some(current_day) {
                hover_bg
            } else {
                Color::new(30, 30, 35)
            };

            for cy in 0..cell_height.saturating_sub(1) {
                for cx in 0..cell_width.saturating_sub(1) {
                    surface.set_pixel(
                        (cell_x + cx as isize) as usize,
                        (cell_y + cy as isize) as usize,
                        bg
                    );
                }
            }

            // Day number
            let day_str = format!("{}", day);
            let color = if current_day == today { Color::new(255, 255, 255) } else { text_color };
            draw_string(surface, cell_x + 5, cell_y + 5, &day_str, color);

            // Events for this day
            let events = self.events_for_date(current_day);
            for (i, event) in events.iter().take(3).enumerate() {
                let event_y = cell_y + 22 + (i * 14) as isize;
                let event_title: String = event.title.chars().take(10).collect();

                // Event dot
                let dot_color = event.color.to_color();
                for dy in 0..4 {
                    for dx in 0..4 {
                        surface.set_pixel(
                            (cell_x + 5 + dx as isize) as usize,
                            (event_y + 4 + dy as isize) as usize,
                            dot_color
                        );
                    }
                }

                draw_string(surface, cell_x + 12, event_y + 2, &event_title, text_color);
            }

            // Show "+N more" if there are more events
            if events.len() > 3 {
                let more_y = cell_y + 22 + (3 * 14) as isize;
                let more_str = format!("+{} more", events.len() - 3);
                draw_string(surface, cell_x + 5, more_y + 2, &more_str, dim_text);
            }

            // Cell borders
            for cy in 0..cell_height {
                surface.set_pixel(
                    (cell_x + cell_width as isize - 1) as usize,
                    (cell_y + cy as isize) as usize,
                    border_color
                );
            }
            for cx in 0..cell_width {
                surface.set_pixel(
                    (cell_x + cx as isize) as usize,
                    (cell_y + cell_height as isize - 1) as usize,
                    border_color
                );
            }
        }
    }

    fn render_day_view(&self, surface: &mut Surface) {
        let text_color = Color::new(220, 220, 220);
        let dim_text = Color::new(100, 100, 100);
        let border_color = Color::new(60, 60, 65);

        let content_x = self.bounds.x + self.sidebar_width as isize + 10;
        let content_y = self.bounds.y + 50;
        let content_width = self.bounds.width.saturating_sub(self.sidebar_width + 20);
        let content_height = self.bounds.height.saturating_sub(70);

        let hour_height = content_height / 24;

        // Time labels and grid
        for hour in 0..24 {
            let y = content_y + (hour * hour_height) as isize;

            // Time label
            let time_str = format!("{:02}:00", hour);
            draw_string(surface, content_x, y + 2, &time_str, dim_text);

            // Hour line
            for x in 50..content_width {
                surface.set_pixel(
                    (content_x + x as isize) as usize,
                    y as usize,
                    border_color
                );
            }
        }

        // Events for the day
        let events = self.events_for_date(self.current_date);
        for event in events {
            if event.all_day {
                // All-day events at top
                let event_bg = event.color.to_color();
                for y in 0..20 {
                    for x in 60..content_width.saturating_sub(10) {
                        surface.set_pixel(
                            (content_x + x as isize) as usize,
                            (content_y - 25 + y as isize) as usize,
                            event_bg
                        );
                    }
                }
                draw_string(surface, content_x + 65, content_y - 22, &event.title, text_color);
            } else {
                // Timed events
                let start_y = content_y + (event.start.time.hour as usize * hour_height + event.start.time.minute as usize * hour_height / 60) as isize;
                let event_height = (event.duration_minutes() as usize * hour_height / 60).max(20);

                let event_bg = event.color.to_color();
                for y in 0..event_height {
                    for x in 60..content_width.saturating_sub(10) {
                        surface.set_pixel(
                            (content_x + x as isize) as usize,
                            (start_y + y as isize) as usize,
                            event_bg
                        );
                    }
                }

                draw_string(surface, content_x + 65, start_y + 3, &event.title, text_color);
                draw_string(surface, content_x + 65, start_y + 18, &event.format_time_range(), Color::new(200, 200, 200));
            }
        }
    }

    fn render_week_view(&self, surface: &mut Surface) {
        let text_color = Color::new(220, 220, 220);
        let dim_text = Color::new(100, 100, 100);
        let border_color = Color::new(60, 60, 65);
        let today_bg = Color::new(45, 85, 150);

        let content_x = self.bounds.x + self.sidebar_width as isize + 50;
        let content_y = self.bounds.y + 70;
        let content_width = self.bounds.width.saturating_sub(self.sidebar_width + 60);
        let content_height = self.bounds.height.saturating_sub(90);

        let day_width = content_width / 7;
        let hour_height = content_height / 12; // Show 12 hours (working hours)
        let today = Date::today();

        // Day headers
        let week_start = self.get_week_start();
        for i in 0..7 {
            let day = week_start.add_days(i);
            let x = content_x + (i as usize * day_width) as isize;

            // Highlight today's column
            if day == today {
                for y in 0..content_height {
                    for dx in 0..day_width {
                        surface.set_pixel(
                            (x + dx as isize) as usize,
                            (content_y + y as isize) as usize,
                            today_bg
                        );
                    }
                }
            }

            // Day header
            let header = format!("{} {}", day.weekday().short_name(), day.day);
            let header_x = x + (day_width / 2 - header.len() * 4) as isize;
            draw_string(surface, header_x, content_y - 20, &header, if day == today { Color::new(255, 255, 255) } else { text_color });

            // Column separator
            for y in 0..content_height {
                surface.set_pixel(
                    (x + day_width as isize - 1) as usize,
                    (content_y + y as isize) as usize,
                    border_color
                );
            }
        }

        // Hour lines
        for hour in 0..12 {
            let actual_hour = hour + 8; // Start from 8 AM
            let y = content_y + (hour * hour_height) as isize;

            let time_str = format!("{:02}:00", actual_hour);
            draw_string(surface, self.bounds.x + self.sidebar_width as isize + 5, y + 2, &time_str, dim_text);

            for x in 0..content_width {
                surface.set_pixel(
                    (content_x + x as isize) as usize,
                    y as usize,
                    border_color
                );
            }
        }
    }

    fn render_agenda_view(&self, surface: &mut Surface) {
        let text_color = Color::new(220, 220, 220);
        let dim_text = Color::new(100, 100, 100);
        let border_color = Color::new(60, 60, 65);

        let content_x = self.bounds.x + self.sidebar_width as isize + 10;
        let content_y = self.bounds.y + 50;

        // Get upcoming events (next 30 days)
        let mut upcoming: Vec<_> = self.events.iter()
            .filter(|e| e.start.date >= self.current_date && e.start.date <= self.current_date.add_days(30))
            .collect();
        upcoming.sort_by_key(|e| (e.start.date, e.start.time));

        let mut y = content_y;
        let mut current_date = Date::new(0, 0, 0);

        for event in upcoming.iter().take(20) {
            if event.start.date != current_date {
                current_date = event.start.date;
                // Date header
                draw_string(surface, content_x, y, &current_date.format_display(), text_color);
                y += 25;
            }

            // Event
            let dot_color = event.color.to_color();
            for dy in 0..8 {
                for dx in 0..8 {
                    surface.set_pixel(
                        (content_x + dx) as usize,
                        (y + 4 + dy) as usize,
                        dot_color
                    );
                }
            }

            draw_string(surface, content_x + 15, y, &event.format_time_range(), dim_text);
            draw_string(surface, content_x + 120, y, &event.title, text_color);

            y += 22;

            if y > self.bounds.y + self.bounds.height as isize - 30 {
                break;
            }
        }

        if upcoming.is_empty() {
            draw_string(surface, content_x, content_y, "No upcoming events", dim_text);
        }
    }
}

/// Initialize calendar module
pub fn init() {
    // Initialization code
}
