//! Power Settings
//!
//! Battery, power profiles, sleep/hibernate, and power management settings.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use spin::Mutex;

/// Global power settings state
static POWER_SETTINGS: Mutex<Option<PowerSettings>> = Mutex::new(None);

/// Power settings state
pub struct PowerSettings {
    /// Battery info
    pub battery: Option<BatteryInfo>,
    /// Power source
    pub power_source: PowerSource,
    /// Current power profile
    pub power_profile: PowerProfile,
    /// Screen timeout (seconds, 0 = never)
    pub screen_timeout: u32,
    /// Sleep timeout (seconds, 0 = never)
    pub sleep_timeout: u32,
    /// Hibernate timeout (seconds, 0 = never)
    pub hibernate_timeout: u32,
    /// Lid close action
    pub lid_close_action: LidAction,
    /// Power button action
    pub power_button_action: PowerButtonAction,
    /// Show battery percentage
    pub show_percentage: bool,
    /// Low battery warning level (%)
    pub low_battery_level: u8,
    /// Critical battery level (%)
    pub critical_battery_level: u8,
    /// Critical battery action
    pub critical_battery_action: CriticalBatteryAction,
    /// Automatic brightness
    pub auto_brightness: bool,
    /// Keyboard backlight timeout (seconds)
    pub keyboard_backlight_timeout: u32,
}

/// Battery info
#[derive(Debug, Clone)]
pub struct BatteryInfo {
    /// Charge percentage (0-100)
    pub percentage: u8,
    /// Battery state
    pub state: BatteryState,
    /// Time to empty (minutes)
    pub time_to_empty: Option<u32>,
    /// Time to full (minutes)
    pub time_to_full: Option<u32>,
    /// Energy (Wh)
    pub energy: f32,
    /// Energy full (Wh)
    pub energy_full: f32,
    /// Energy full design (Wh)
    pub energy_full_design: f32,
    /// Voltage (V)
    pub voltage: f32,
    /// Temperature (°C)
    pub temperature: Option<f32>,
    /// Battery health (%)
    pub health: u8,
    /// Cycle count
    pub cycle_count: Option<u32>,
    /// Manufacturer
    pub manufacturer: String,
    /// Model
    pub model: String,
}

/// Battery state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatteryState {
    /// Charging
    Charging,
    /// Discharging
    Discharging,
    /// Not charging (full)
    NotCharging,
    /// Full
    Full,
    /// Unknown
    Unknown,
}

impl BatteryState {
    pub fn name(&self) -> &'static str {
        match self {
            BatteryState::Charging => "Charging",
            BatteryState::Discharging => "On Battery",
            BatteryState::NotCharging => "Not Charging",
            BatteryState::Full => "Fully Charged",
            BatteryState::Unknown => "Unknown",
        }
    }

    pub fn icon(&self, percentage: u8) -> &'static str {
        match self {
            BatteryState::Charging => {
                if percentage >= 80 { "battery-full-charging" }
                else if percentage >= 60 { "battery-good-charging" }
                else if percentage >= 40 { "battery-low-charging" }
                else if percentage >= 20 { "battery-caution-charging" }
                else { "battery-empty-charging" }
            }
            _ => {
                if percentage >= 80 { "battery-full" }
                else if percentage >= 60 { "battery-good" }
                else if percentage >= 40 { "battery-low" }
                else if percentage >= 20 { "battery-caution" }
                else { "battery-empty" }
            }
        }
    }
}

/// Power source
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerSource {
    Battery,
    AC,
    UPS,
    Unknown,
}

/// Power profile
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerProfile {
    /// Performance - Maximum power, no throttling
    Performance,
    /// Balanced - Default settings
    Balanced,
    /// Power Saver - Extended battery life
    PowerSaver,
}

impl PowerProfile {
    pub fn name(&self) -> &'static str {
        match self {
            PowerProfile::Performance => "Performance",
            PowerProfile::Balanced => "Balanced",
            PowerProfile::PowerSaver => "Power Saver",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            PowerProfile::Performance => "Maximum performance, higher power consumption",
            PowerProfile::Balanced => "Balance between performance and battery life",
            PowerProfile::PowerSaver => "Extended battery life, reduced performance",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            PowerProfile::Performance => "power-profile-performance",
            PowerProfile::Balanced => "power-profile-balanced",
            PowerProfile::PowerSaver => "power-profile-power-saver",
        }
    }
}

/// Lid close action
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LidAction {
    Nothing,
    Blank,
    Lock,
    Suspend,
    Hibernate,
    Shutdown,
}

impl LidAction {
    pub fn name(&self) -> &'static str {
        match self {
            LidAction::Nothing => "Do Nothing",
            LidAction::Blank => "Blank Screen",
            LidAction::Lock => "Lock Screen",
            LidAction::Suspend => "Suspend",
            LidAction::Hibernate => "Hibernate",
            LidAction::Shutdown => "Shut Down",
        }
    }
}

/// Power button action
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerButtonAction {
    Nothing,
    Suspend,
    Hibernate,
    Shutdown,
    Interactive,
}

impl PowerButtonAction {
    pub fn name(&self) -> &'static str {
        match self {
            PowerButtonAction::Nothing => "Do Nothing",
            PowerButtonAction::Suspend => "Suspend",
            PowerButtonAction::Hibernate => "Hibernate",
            PowerButtonAction::Shutdown => "Shut Down",
            PowerButtonAction::Interactive => "Ask What to Do",
        }
    }
}

/// Critical battery action
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CriticalBatteryAction {
    Nothing,
    Hibernate,
    Shutdown,
}

impl CriticalBatteryAction {
    pub fn name(&self) -> &'static str {
        match self {
            CriticalBatteryAction::Nothing => "Do Nothing",
            CriticalBatteryAction::Hibernate => "Hibernate",
            CriticalBatteryAction::Shutdown => "Shut Down",
        }
    }
}

/// Initialize power settings
pub fn init() {
    let mut state = POWER_SETTINGS.lock();
    if state.is_some() {
        return;
    }

    *state = Some(PowerSettings {
        battery: Some(BatteryInfo {
            percentage: 100,
            state: BatteryState::Unknown,
            time_to_empty: None,
            time_to_full: None,
            energy: 50.0,
            energy_full: 50.0,
            energy_full_design: 50.0,
            voltage: 12.0,
            temperature: None,
            health: 100,
            cycle_count: None,
            manufacturer: "Unknown".to_string(),
            model: "Unknown".to_string(),
        }),
        power_source: PowerSource::Unknown,
        power_profile: PowerProfile::Balanced,
        screen_timeout: 300, // 5 minutes
        sleep_timeout: 900,  // 15 minutes
        hibernate_timeout: 0, // Never
        lid_close_action: LidAction::Suspend,
        power_button_action: PowerButtonAction::Interactive,
        show_percentage: true,
        low_battery_level: 20,
        critical_battery_level: 5,
        critical_battery_action: CriticalBatteryAction::Hibernate,
        auto_brightness: true,
        keyboard_backlight_timeout: 10,
    });

    crate::kprintln!("power settings: initialized");
}

/// Get battery info
pub fn get_battery() -> Option<BatteryInfo> {
    let state = POWER_SETTINGS.lock();
    state.as_ref().and_then(|s| s.battery.clone())
}

/// Get power source
pub fn get_power_source() -> PowerSource {
    let state = POWER_SETTINGS.lock();
    state.as_ref().map(|s| s.power_source).unwrap_or(PowerSource::Unknown)
}

/// Get power profile
pub fn get_power_profile() -> PowerProfile {
    let state = POWER_SETTINGS.lock();
    state.as_ref().map(|s| s.power_profile).unwrap_or(PowerProfile::Balanced)
}

/// Set power profile
pub fn set_power_profile(profile: PowerProfile) {
    let mut state = POWER_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.power_profile = profile;
        // TODO: Apply to system (CPU governor, etc.)
    }
}

/// Set screen timeout
pub fn set_screen_timeout(seconds: u32) {
    let mut state = POWER_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.screen_timeout = seconds;
    }
}

/// Get screen timeout
pub fn get_screen_timeout() -> u32 {
    let state = POWER_SETTINGS.lock();
    state.as_ref().map(|s| s.screen_timeout).unwrap_or(300)
}

/// Set sleep timeout
pub fn set_sleep_timeout(seconds: u32) {
    let mut state = POWER_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.sleep_timeout = seconds;
    }
}

/// Set hibernate timeout
pub fn set_hibernate_timeout(seconds: u32) {
    let mut state = POWER_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.hibernate_timeout = seconds;
    }
}

/// Set lid close action
pub fn set_lid_close_action(action: LidAction) {
    let mut state = POWER_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.lid_close_action = action;
    }
}

/// Set power button action
pub fn set_power_button_action(action: PowerButtonAction) {
    let mut state = POWER_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.power_button_action = action;
    }
}

/// Set show battery percentage
pub fn set_show_percentage(show: bool) {
    let mut state = POWER_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.show_percentage = show;
    }
}

/// Set low battery level
pub fn set_low_battery_level(level: u8) {
    let mut state = POWER_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.low_battery_level = level.min(100);
    }
}

/// Set critical battery level
pub fn set_critical_battery_level(level: u8) {
    let mut state = POWER_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.critical_battery_level = level.min(100);
    }
}

/// Set critical battery action
pub fn set_critical_battery_action(action: CriticalBatteryAction) {
    let mut state = POWER_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.critical_battery_action = action;
    }
}

/// Set auto brightness
pub fn set_auto_brightness(enabled: bool) {
    let mut state = POWER_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.auto_brightness = enabled;
    }
}

/// Is auto brightness enabled
pub fn is_auto_brightness() -> bool {
    let state = POWER_SETTINGS.lock();
    state.as_ref().map(|s| s.auto_brightness).unwrap_or(false)
}

/// Set keyboard backlight timeout
pub fn set_keyboard_backlight_timeout(seconds: u32) {
    let mut state = POWER_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.keyboard_backlight_timeout = seconds;
    }
}

/// Update battery info (called by power management driver)
pub fn update_battery_info(info: BatteryInfo) {
    let mut state = POWER_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.battery = Some(info);
    }
}

/// Update power source (called by power management driver)
pub fn update_power_source(source: PowerSource) {
    let mut state = POWER_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.power_source = source;
    }
}

/// Power error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerError {
    NotInitialized,
    NoBattery,
    UnsupportedAction,
}
