//! Date & Time Settings
//!
//! Timezone, automatic time sync, date/time formats, and calendar settings.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use spin::Mutex;

/// Global date/time settings state
static DATETIME_SETTINGS: Mutex<Option<DateTimeSettings>> = Mutex::new(None);

/// Date/time settings state
pub struct DateTimeSettings {
    /// Automatic date/time
    pub automatic_datetime: bool,
    /// Automatic timezone
    pub automatic_timezone: bool,
    /// Current timezone
    pub timezone: Timezone,
    /// NTP servers
    pub ntp_servers: Vec<String>,
    /// Time format
    pub time_format: TimeFormat,
    /// Date format
    pub date_format: DateFormat,
    /// First day of week
    pub first_day_of_week: Weekday,
    /// Show seconds in clock
    pub show_seconds: bool,
    /// Show date in clock
    pub show_date: bool,
    /// Show week numbers in calendar
    pub show_week_numbers: bool,
    /// Current time (seconds since epoch)
    pub current_time: u64,
}

/// Timezone
#[derive(Debug, Clone)]
pub struct Timezone {
    /// IANA timezone name (e.g., "America/New_York")
    pub name: String,
    /// Display name (e.g., "Eastern Time")
    pub display_name: String,
    /// UTC offset (minutes)
    pub utc_offset: i32,
    /// Has DST
    pub has_dst: bool,
    /// Currently in DST
    pub in_dst: bool,
}

impl Timezone {
    /// Common timezones
    pub fn utc() -> Self {
        Timezone {
            name: "UTC".to_string(),
            display_name: "Coordinated Universal Time".to_string(),
            utc_offset: 0,
            has_dst: false,
            in_dst: false,
        }
    }

    pub fn new_york() -> Self {
        Timezone {
            name: "America/New_York".to_string(),
            display_name: "Eastern Time".to_string(),
            utc_offset: -300, // -5 hours
            has_dst: true,
            in_dst: false,
        }
    }

    pub fn los_angeles() -> Self {
        Timezone {
            name: "America/Los_Angeles".to_string(),
            display_name: "Pacific Time".to_string(),
            utc_offset: -480, // -8 hours
            has_dst: true,
            in_dst: false,
        }
    }

    pub fn london() -> Self {
        Timezone {
            name: "Europe/London".to_string(),
            display_name: "London".to_string(),
            utc_offset: 0,
            has_dst: true,
            in_dst: false,
        }
    }

    pub fn berlin() -> Self {
        Timezone {
            name: "Europe/Berlin".to_string(),
            display_name: "Central European Time".to_string(),
            utc_offset: 60, // +1 hour
            has_dst: true,
            in_dst: false,
        }
    }

    pub fn tokyo() -> Self {
        Timezone {
            name: "Asia/Tokyo".to_string(),
            display_name: "Japan Standard Time".to_string(),
            utc_offset: 540, // +9 hours
            has_dst: false,
            in_dst: false,
        }
    }

    pub fn sao_paulo() -> Self {
        Timezone {
            name: "America/Sao_Paulo".to_string(),
            display_name: "Brasilia Time".to_string(),
            utc_offset: -180, // -3 hours
            has_dst: false,
            in_dst: false,
        }
    }

    /// Get offset string (e.g., "UTC-05:00")
    pub fn offset_string(&self) -> String {
        let sign = if self.utc_offset >= 0 { '+' } else { '-' };
        let hours = self.utc_offset.abs() / 60;
        let mins = self.utc_offset.abs() % 60;
        alloc::format!("UTC{}{:02}:{:02}", sign, hours, mins)
    }
}

/// Time format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeFormat {
    /// 12-hour (AM/PM)
    Hour12,
    /// 24-hour
    Hour24,
}

impl TimeFormat {
    pub fn name(&self) -> &'static str {
        match self {
            TimeFormat::Hour12 => "12-hour (AM/PM)",
            TimeFormat::Hour24 => "24-hour",
        }
    }

    pub fn example(&self) -> &'static str {
        match self {
            TimeFormat::Hour12 => "3:45 PM",
            TimeFormat::Hour24 => "15:45",
        }
    }
}

/// Date format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateFormat {
    /// YYYY-MM-DD (ISO)
    Iso,
    /// MM/DD/YYYY (US)
    US,
    /// DD/MM/YYYY (European)
    European,
    /// DD.MM.YYYY (German)
    German,
    /// YYYY/MM/DD (Asian)
    Asian,
}

impl DateFormat {
    pub fn name(&self) -> &'static str {
        match self {
            DateFormat::Iso => "YYYY-MM-DD",
            DateFormat::US => "MM/DD/YYYY",
            DateFormat::European => "DD/MM/YYYY",
            DateFormat::German => "DD.MM.YYYY",
            DateFormat::Asian => "YYYY/MM/DD",
        }
    }

    pub fn example(&self) -> &'static str {
        match self {
            DateFormat::Iso => "2026-01-17",
            DateFormat::US => "01/17/2026",
            DateFormat::European => "17/01/2026",
            DateFormat::German => "17.01.2026",
            DateFormat::Asian => "2026/01/17",
        }
    }
}

/// Day of week
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
}

/// Initialize date/time settings
pub fn init() {
    let mut state = DATETIME_SETTINGS.lock();
    if state.is_some() {
        return;
    }

    *state = Some(DateTimeSettings {
        automatic_datetime: true,
        automatic_timezone: false,
        timezone: Timezone::utc(),
        ntp_servers: vec![
            "pool.ntp.org".to_string(),
            "time.google.com".to_string(),
            "time.cloudflare.com".to_string(),
        ],
        time_format: TimeFormat::Hour24,
        date_format: DateFormat::Iso,
        first_day_of_week: Weekday::Sunday,
        show_seconds: false,
        show_date: true,
        show_week_numbers: false,
        current_time: 0,
    });

    crate::kprintln!("datetime settings: initialized");
}

/// Get current time
pub fn get_current_time() -> u64 {
    let state = DATETIME_SETTINGS.lock();
    state.as_ref().map(|s| s.current_time).unwrap_or(0)
}

/// Update current time (called by time driver)
pub fn update_time(timestamp: u64) {
    let mut state = DATETIME_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.current_time = timestamp;
    }
}

/// Get timezone
pub fn get_timezone() -> Option<Timezone> {
    let state = DATETIME_SETTINGS.lock();
    state.as_ref().map(|s| s.timezone.clone())
}

/// Set timezone
pub fn set_timezone(timezone: Timezone) {
    let mut state = DATETIME_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.timezone = timezone;
    }
}

/// Get available timezones
pub fn get_available_timezones() -> Vec<Timezone> {
    vec![
        Timezone::utc(),
        Timezone::new_york(),
        Timezone::los_angeles(),
        Timezone::london(),
        Timezone::berlin(),
        Timezone::tokyo(),
        Timezone::sao_paulo(),
    ]
}

/// Set automatic date/time
pub fn set_automatic_datetime(enabled: bool) {
    let mut state = DATETIME_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.automatic_datetime = enabled;
    }
}

/// Is automatic date/time enabled
pub fn is_automatic_datetime() -> bool {
    let state = DATETIME_SETTINGS.lock();
    state.as_ref().map(|s| s.automatic_datetime).unwrap_or(true)
}

/// Set automatic timezone
pub fn set_automatic_timezone(enabled: bool) {
    let mut state = DATETIME_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.automatic_timezone = enabled;
    }
}

/// Get NTP servers
pub fn get_ntp_servers() -> Vec<String> {
    let state = DATETIME_SETTINGS.lock();
    state.as_ref().map(|s| s.ntp_servers.clone()).unwrap_or_default()
}

/// Set NTP servers
pub fn set_ntp_servers(servers: Vec<String>) {
    let mut state = DATETIME_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.ntp_servers = servers;
    }
}

/// Get time format
pub fn get_time_format() -> TimeFormat {
    let state = DATETIME_SETTINGS.lock();
    state.as_ref().map(|s| s.time_format).unwrap_or(TimeFormat::Hour24)
}

/// Set time format
pub fn set_time_format(format: TimeFormat) {
    let mut state = DATETIME_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.time_format = format;
    }
}

/// Get date format
pub fn get_date_format() -> DateFormat {
    let state = DATETIME_SETTINGS.lock();
    state.as_ref().map(|s| s.date_format).unwrap_or(DateFormat::Iso)
}

/// Set date format
pub fn set_date_format(format: DateFormat) {
    let mut state = DATETIME_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.date_format = format;
    }
}

/// Get first day of week
pub fn get_first_day_of_week() -> Weekday {
    let state = DATETIME_SETTINGS.lock();
    state.as_ref().map(|s| s.first_day_of_week).unwrap_or(Weekday::Sunday)
}

/// Set first day of week
pub fn set_first_day_of_week(day: Weekday) {
    let mut state = DATETIME_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.first_day_of_week = day;
    }
}

/// Set show seconds
pub fn set_show_seconds(show: bool) {
    let mut state = DATETIME_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.show_seconds = show;
    }
}

/// Is show seconds enabled
pub fn is_show_seconds() -> bool {
    let state = DATETIME_SETTINGS.lock();
    state.as_ref().map(|s| s.show_seconds).unwrap_or(false)
}

/// Set show date in clock
pub fn set_show_date(show: bool) {
    let mut state = DATETIME_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.show_date = show;
    }
}

/// Set show week numbers
pub fn set_show_week_numbers(show: bool) {
    let mut state = DATETIME_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.show_week_numbers = show;
    }
}

/// Manually set date/time
pub fn set_datetime(timestamp: u64) -> Result<(), DateTimeError> {
    let mut state = DATETIME_SETTINGS.lock();
    let state = state.as_mut().ok_or(DateTimeError::NotInitialized)?;

    if state.automatic_datetime {
        return Err(DateTimeError::AutomaticEnabled);
    }

    state.current_time = timestamp;

    // TODO: Actually set system time via RTC

    Ok(())
}

/// Date/time error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateTimeError {
    NotInitialized,
    AutomaticEnabled,
    InvalidTimezone,
    NtpSyncFailed,
}
