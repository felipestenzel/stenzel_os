//! Function Keys (Fn) Support
//!
//! Handles laptop function key combinations (Fn+F1-F12) for:
//! - Brightness control (Fn+F5/F6)
//! - Volume control (Fn+F1/F2/F3)
//! - Mute toggle
//! - Display switching (Fn+F7)
//! - Wireless toggle (Fn+F9)
//! - Keyboard backlight (Fn+Space)
//! - Sleep/Suspend (Fn+F4)
//! - Touchpad toggle (Fn+F8)
//! - Performance modes
//!
//! Function keys are typically handled via:
//! - ACPI hotkey events (most laptops)
//! - WMI events (some vendors)
//! - Special scancodes (EC-translated)

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::sync::IrqSafeMutex;

/// Function key action
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FnKeyAction {
    // Brightness
    BrightnessUp,
    BrightnessDown,
    BrightnessMin,
    BrightnessMax,

    // Volume
    VolumeUp,
    VolumeDown,
    VolumeMute,
    MicMute,

    // Display
    DisplaySwitch,      // Toggle internal/external
    DisplayMirror,
    DisplayExtend,
    DisplayOff,

    // Wireless
    WirelessToggle,     // Toggle WiFi/Bluetooth
    WifiToggle,
    BluetoothToggle,
    AirplaneMode,

    // Power
    Sleep,              // Suspend to RAM
    Hibernate,
    PowerMenu,

    // Keyboard
    KeyboardBacklightUp,
    KeyboardBacklightDown,
    KeyboardBacklightToggle,

    // Touchpad
    TouchpadToggle,

    // Performance
    PerformanceMode,    // Cycle through power profiles
    FanBoost,           // Toggle fan boost

    // Media
    PlayPause,
    Stop,
    Previous,
    Next,

    // Misc
    Calculator,
    Browser,
    Mail,
    Search,
    ScreenLock,
    ScreenCapture,
    Settings,

    // Custom action
    Custom(u32),
}

/// ACPI hotkey event codes (common values)
pub mod acpi_events {
    pub const BRIGHTNESS_UP: u32 = 0x86;
    pub const BRIGHTNESS_DOWN: u32 = 0x87;
    pub const VIDEO_SWITCH: u32 = 0x88;
    pub const WIRELESS_TOGGLE: u32 = 0x89;
    pub const DISPLAY_OFF: u32 = 0x8A;
    pub const TOUCHPAD_TOGGLE: u32 = 0x8B;
    pub const SUSPEND: u32 = 0x8C;
    pub const HIBERNATE: u32 = 0x8D;
    pub const VOLUME_UP: u32 = 0x8E;
    pub const VOLUME_DOWN: u32 = 0x8F;
    pub const VOLUME_MUTE: u32 = 0x90;
    pub const FAN_BOOST: u32 = 0x91;
}

/// Scancode set 1 extended codes for function keys
/// These are sent by some laptops via EC
pub mod fn_scancodes {
    // Common extended scancodes (after E0 prefix)
    pub const SC_BRIGHTNESS_DOWN: u8 = 0x50;  // Varies by vendor
    pub const SC_BRIGHTNESS_UP: u8 = 0x51;
    pub const SC_DISPLAY_SWITCH: u8 = 0x52;
    pub const SC_WIRELESS: u8 = 0x53;

    // Multimedia keys (E0 prefix)
    pub const SC_MUTE: u8 = 0x20;
    pub const SC_VOLUME_DOWN: u8 = 0x2E;
    pub const SC_VOLUME_UP: u8 = 0x30;
    pub const SC_PLAY_PAUSE: u8 = 0x22;
    pub const SC_STOP: u8 = 0x24;
    pub const SC_PREV_TRACK: u8 = 0x10;
    pub const SC_NEXT_TRACK: u8 = 0x19;

    // Application launch keys
    pub const SC_CALCULATOR: u8 = 0x21;
    pub const SC_BROWSER: u8 = 0x32;
    pub const SC_MAIL: u8 = 0x6C;
    pub const SC_SEARCH: u8 = 0x65;
}

/// Brightness step (percentage points)
const BRIGHTNESS_STEP: u32 = 10;

/// Volume step (percentage points)
const VOLUME_STEP: u32 = 5;

/// Function key state
pub struct FnKeyState {
    /// Whether Fn key is currently pressed
    fn_pressed: AtomicBool,
    /// Whether wireless is enabled
    wireless_enabled: AtomicBool,
    /// Whether touchpad is enabled
    touchpad_enabled: AtomicBool,
    /// Whether airplane mode is active
    airplane_mode: AtomicBool,
    /// Custom key bindings: scancode -> action
    key_bindings: BTreeMap<u8, FnKeyAction>,
    /// ACPI event bindings: event code -> action
    acpi_bindings: BTreeMap<u32, FnKeyAction>,
    /// Whether OSD notifications are enabled
    osd_enabled: bool,
}

impl FnKeyState {
    pub const fn new() -> Self {
        Self {
            fn_pressed: AtomicBool::new(false),
            wireless_enabled: AtomicBool::new(true),
            touchpad_enabled: AtomicBool::new(true),
            airplane_mode: AtomicBool::new(false),
            key_bindings: BTreeMap::new(),
            acpi_bindings: BTreeMap::new(),
            osd_enabled: true,
        }
    }

    /// Initialize with default bindings
    fn init_default_bindings(&mut self) {
        use fn_scancodes::*;

        // Extended scancode bindings (after E0)
        self.key_bindings.insert(SC_BRIGHTNESS_DOWN, FnKeyAction::BrightnessDown);
        self.key_bindings.insert(SC_BRIGHTNESS_UP, FnKeyAction::BrightnessUp);
        self.key_bindings.insert(SC_DISPLAY_SWITCH, FnKeyAction::DisplaySwitch);
        self.key_bindings.insert(SC_WIRELESS, FnKeyAction::WirelessToggle);
        self.key_bindings.insert(SC_MUTE, FnKeyAction::VolumeMute);
        self.key_bindings.insert(SC_VOLUME_DOWN, FnKeyAction::VolumeDown);
        self.key_bindings.insert(SC_VOLUME_UP, FnKeyAction::VolumeUp);
        self.key_bindings.insert(SC_PLAY_PAUSE, FnKeyAction::PlayPause);
        self.key_bindings.insert(SC_STOP, FnKeyAction::Stop);
        self.key_bindings.insert(SC_PREV_TRACK, FnKeyAction::Previous);
        self.key_bindings.insert(SC_NEXT_TRACK, FnKeyAction::Next);
        self.key_bindings.insert(SC_CALCULATOR, FnKeyAction::Calculator);
        self.key_bindings.insert(SC_BROWSER, FnKeyAction::Browser);
        self.key_bindings.insert(SC_MAIL, FnKeyAction::Mail);
        self.key_bindings.insert(SC_SEARCH, FnKeyAction::Search);

        // ACPI event bindings
        use acpi_events::*;
        self.acpi_bindings.insert(BRIGHTNESS_UP, FnKeyAction::BrightnessUp);
        self.acpi_bindings.insert(BRIGHTNESS_DOWN, FnKeyAction::BrightnessDown);
        self.acpi_bindings.insert(VIDEO_SWITCH, FnKeyAction::DisplaySwitch);
        self.acpi_bindings.insert(WIRELESS_TOGGLE, FnKeyAction::WirelessToggle);
        self.acpi_bindings.insert(DISPLAY_OFF, FnKeyAction::DisplayOff);
        self.acpi_bindings.insert(TOUCHPAD_TOGGLE, FnKeyAction::TouchpadToggle);
        self.acpi_bindings.insert(SUSPEND, FnKeyAction::Sleep);
        self.acpi_bindings.insert(HIBERNATE, FnKeyAction::Hibernate);
        self.acpi_bindings.insert(VOLUME_UP, FnKeyAction::VolumeUp);
        self.acpi_bindings.insert(VOLUME_DOWN, FnKeyAction::VolumeDown);
        self.acpi_bindings.insert(VOLUME_MUTE, FnKeyAction::VolumeMute);
        self.acpi_bindings.insert(FAN_BOOST, FnKeyAction::FanBoost);
    }
}

/// Global function key state
static FN_STATE: IrqSafeMutex<FnKeyState> = IrqSafeMutex::new(FnKeyState::new());

/// Initialize function key support
pub fn init() {
    let mut state = FN_STATE.lock();
    state.init_default_bindings();

    crate::kprintln!("fnkeys: initialized with {} key bindings, {} ACPI bindings",
        state.key_bindings.len(),
        state.acpi_bindings.len());
}

/// Process an extended scancode (after E0 prefix)
/// Called from keyboard driver
pub fn process_extended_scancode(scancode: u8, pressed: bool) -> bool {
    // Only act on key press, not release
    if !pressed {
        return false;
    }

    let state = FN_STATE.lock();
    if let Some(&action) = state.key_bindings.get(&scancode) {
        drop(state); // Release lock before executing action
        execute_action(action);
        return true;
    }

    false
}

/// Process an ACPI hotkey event
/// Called from ACPI event handler
pub fn process_acpi_event(event_code: u32) -> bool {
    let state = FN_STATE.lock();
    if let Some(&action) = state.acpi_bindings.get(&event_code) {
        drop(state);
        execute_action(action);
        return true;
    }

    false
}

/// Execute a function key action
pub fn execute_action(action: FnKeyAction) {
    match action {
        // Brightness control
        FnKeyAction::BrightnessUp => {
            brightness_up();
        }
        FnKeyAction::BrightnessDown => {
            brightness_down();
        }
        FnKeyAction::BrightnessMin => {
            set_brightness_percent(0);
        }
        FnKeyAction::BrightnessMax => {
            set_brightness_percent(100);
        }

        // Volume control
        FnKeyAction::VolumeUp => {
            volume_up();
        }
        FnKeyAction::VolumeDown => {
            volume_down();
        }
        FnKeyAction::VolumeMute => {
            toggle_mute();
        }
        FnKeyAction::MicMute => {
            toggle_mic_mute();
        }

        // Display
        FnKeyAction::DisplaySwitch => {
            switch_display();
        }
        FnKeyAction::DisplayOff => {
            display_off();
        }
        FnKeyAction::DisplayMirror | FnKeyAction::DisplayExtend => {
            // TODO: implement display modes
        }

        // Wireless
        FnKeyAction::WirelessToggle => {
            toggle_wireless();
        }
        FnKeyAction::WifiToggle => {
            toggle_wifi();
        }
        FnKeyAction::BluetoothToggle => {
            toggle_bluetooth();
        }
        FnKeyAction::AirplaneMode => {
            toggle_airplane_mode();
        }

        // Power
        FnKeyAction::Sleep => {
            suspend_system();
        }
        FnKeyAction::Hibernate => {
            hibernate_system();
        }
        FnKeyAction::PowerMenu => {
            show_power_menu();
        }

        // Keyboard
        FnKeyAction::KeyboardBacklightUp => {
            keyboard_backlight_up();
        }
        FnKeyAction::KeyboardBacklightDown => {
            keyboard_backlight_down();
        }
        FnKeyAction::KeyboardBacklightToggle => {
            toggle_keyboard_backlight();
        }

        // Touchpad
        FnKeyAction::TouchpadToggle => {
            toggle_touchpad();
        }

        // Performance
        FnKeyAction::PerformanceMode => {
            cycle_performance_mode();
        }
        FnKeyAction::FanBoost => {
            toggle_fan_boost();
        }

        // Media
        FnKeyAction::PlayPause => {
            media_play_pause();
        }
        FnKeyAction::Stop => {
            media_stop();
        }
        FnKeyAction::Previous => {
            media_previous();
        }
        FnKeyAction::Next => {
            media_next();
        }

        // Misc
        FnKeyAction::Calculator => {
            launch_calculator();
        }
        FnKeyAction::Browser => {
            launch_browser();
        }
        FnKeyAction::Mail => {
            launch_mail();
        }
        FnKeyAction::Search => {
            open_search();
        }
        FnKeyAction::ScreenLock => {
            lock_screen();
        }
        FnKeyAction::ScreenCapture => {
            capture_screen();
        }
        FnKeyAction::Settings => {
            open_settings();
        }

        FnKeyAction::Custom(code) => {
            crate::kprintln!("fnkeys: custom action {:#x}", code);
        }
    }
}

// =============================================================================
// Brightness Control
// =============================================================================

fn brightness_up() {
    let _ = super::backlight::increase_brightness(BRIGHTNESS_STEP);
    let current = get_brightness_percent();
    show_osd("Brightness", current);
}

fn brightness_down() {
    let _ = super::backlight::decrease_brightness(BRIGHTNESS_STEP);
    let current = get_brightness_percent();
    show_osd("Brightness", current);
}

fn get_brightness_percent() -> u32 {
    super::backlight::get_brightness().unwrap_or(50)
}

fn set_brightness_percent(percent: u32) {
    let _ = super::backlight::set_brightness(percent);
    crate::kprintln!("fnkeys: brightness set to {}%", percent);
}

// =============================================================================
// Volume Control
// =============================================================================

fn volume_up() {
    let current = get_volume_percent();
    let new = (current + VOLUME_STEP).min(100);
    set_volume_percent(new);
    show_osd("Volume", new);
}

fn volume_down() {
    let current = get_volume_percent();
    let new = current.saturating_sub(VOLUME_STEP);
    set_volume_percent(new);
    show_osd("Volume", new);
}

fn toggle_mute() {
    let muted = is_muted();
    set_muted(!muted);
    if !muted {
        crate::kprintln!("fnkeys: audio muted");
    } else {
        crate::kprintln!("fnkeys: audio unmuted");
    }
}

fn toggle_mic_mute() {
    // TODO: implement mic mute
    crate::kprintln!("fnkeys: mic mute toggled");
}

fn get_volume_percent() -> u32 {
    super::audio::mixer::get_master_volume() as u32
}

fn set_volume_percent(percent: u32) {
    super::audio::mixer::set_master_volume(percent.min(100) as u8);
    crate::kprintln!("fnkeys: volume set to {}%", percent);
}

fn is_muted() -> bool {
    super::audio::mixer::is_master_muted()
}

fn set_muted(muted: bool) {
    super::audio::mixer::set_master_mute(muted);
}

// =============================================================================
// Display Control
// =============================================================================

fn switch_display() {
    // Cycle through display modes: internal -> clone -> extend -> external -> internal
    crate::kprintln!("fnkeys: display switch requested");
    // TODO: implement display switching
}

fn display_off() {
    // Turn off display (DPMS off)
    crate::kprintln!("fnkeys: display off");
    // Set brightness to 0 as a simple way to turn off display
    let _ = super::backlight::set_brightness(0);
}

// =============================================================================
// Wireless Control
// =============================================================================

fn toggle_wireless() {
    let state = FN_STATE.lock();
    let enabled = state.wireless_enabled.load(Ordering::Relaxed);
    drop(state);

    let new_state = !enabled;
    set_wireless_enabled(new_state);

    if new_state {
        crate::kprintln!("fnkeys: wireless enabled");
    } else {
        crate::kprintln!("fnkeys: wireless disabled");
    }
}

fn toggle_wifi() {
    crate::kprintln!("fnkeys: WiFi toggle");
    // TODO: toggle WiFi specifically
}

fn toggle_bluetooth() {
    crate::kprintln!("fnkeys: Bluetooth toggle");
    // TODO: toggle Bluetooth specifically
}

fn toggle_airplane_mode() {
    let state = FN_STATE.lock();
    let current = state.airplane_mode.load(Ordering::Relaxed);
    state.airplane_mode.store(!current, Ordering::Relaxed);
    drop(state);

    if !current {
        crate::kprintln!("fnkeys: airplane mode ON");
        set_wireless_enabled(false);
    } else {
        crate::kprintln!("fnkeys: airplane mode OFF");
        set_wireless_enabled(true);
    }
}

fn set_wireless_enabled(enabled: bool) {
    FN_STATE.lock().wireless_enabled.store(enabled, Ordering::Relaxed);
    // TODO: actually enable/disable wireless hardware via rfkill
}

// =============================================================================
// Power Control
// =============================================================================

fn suspend_system() {
    crate::kprintln!("fnkeys: suspend requested");
    crate::power::suspend();
}

fn hibernate_system() {
    crate::kprintln!("fnkeys: hibernate requested");
    // TODO: implement hibernate
}

fn show_power_menu() {
    crate::kprintln!("fnkeys: power menu requested");
    // TODO: show GUI power menu
}

// =============================================================================
// Keyboard Backlight
// =============================================================================

static KEYBOARD_BACKLIGHT_LEVEL: core::sync::atomic::AtomicU8 =
    core::sync::atomic::AtomicU8::new(0);

const KEYBOARD_BACKLIGHT_MAX: u8 = 3;

fn keyboard_backlight_up() {
    let current = KEYBOARD_BACKLIGHT_LEVEL.load(Ordering::Relaxed);
    if current < KEYBOARD_BACKLIGHT_MAX {
        let new = current + 1;
        set_keyboard_backlight(new);
    }
}

fn keyboard_backlight_down() {
    let current = KEYBOARD_BACKLIGHT_LEVEL.load(Ordering::Relaxed);
    if current > 0 {
        let new = current - 1;
        set_keyboard_backlight(new);
    }
}

fn toggle_keyboard_backlight() {
    let current = KEYBOARD_BACKLIGHT_LEVEL.load(Ordering::Relaxed);
    if current > 0 {
        set_keyboard_backlight(0);
    } else {
        set_keyboard_backlight(KEYBOARD_BACKLIGHT_MAX);
    }
}

fn set_keyboard_backlight(level: u8) {
    KEYBOARD_BACKLIGHT_LEVEL.store(level, Ordering::Relaxed);
    crate::kprintln!("fnkeys: keyboard backlight level {}", level);
    // TODO: actually set keyboard backlight via EC
}

// =============================================================================
// Touchpad Control
// =============================================================================

fn toggle_touchpad() {
    let state = FN_STATE.lock();
    let enabled = state.touchpad_enabled.load(Ordering::Relaxed);
    state.touchpad_enabled.store(!enabled, Ordering::Relaxed);
    drop(state);

    if !enabled {
        crate::kprintln!("fnkeys: touchpad enabled");
    } else {
        crate::kprintln!("fnkeys: touchpad disabled");
    }
    // TODO: actually enable/disable touchpad
}

/// Check if touchpad is enabled
pub fn is_touchpad_enabled() -> bool {
    FN_STATE.lock().touchpad_enabled.load(Ordering::Relaxed)
}

// =============================================================================
// Performance Control
// =============================================================================

static PERFORMANCE_MODE: core::sync::atomic::AtomicU8 =
    core::sync::atomic::AtomicU8::new(1); // 0=power save, 1=balanced, 2=performance

fn cycle_performance_mode() {
    let current = PERFORMANCE_MODE.load(Ordering::Relaxed);
    let new = (current + 1) % 3;
    PERFORMANCE_MODE.store(new, Ordering::Relaxed);

    let mode_name = match new {
        0 => "Power Save",
        1 => "Balanced",
        2 => "Performance",
        _ => "Unknown",
    };

    crate::kprintln!("fnkeys: performance mode: {}", mode_name);

    // Apply to cpufreq governor
    use crate::arch::x86_64_arch::cpufreq;
    match new {
        0 => { let _ = cpufreq::set_governor(cpufreq::Governor::Powersave); }
        1 => { let _ = cpufreq::set_governor(cpufreq::Governor::Ondemand); }
        2 => { let _ = cpufreq::set_governor(cpufreq::Governor::Performance); }
        _ => {}
    }
}

fn toggle_fan_boost() {
    // Toggle fans to full speed
    use super::thermal;

    if thermal::get_fan_mode() == thermal::FanControlMode::FullSpeed {
        thermal::set_fans_auto();
        crate::kprintln!("fnkeys: fan boost OFF");
    } else {
        thermal::set_fans_full();
        crate::kprintln!("fnkeys: fan boost ON");
    }
}

// =============================================================================
// Media Control
// =============================================================================

fn media_play_pause() {
    crate::kprintln!("fnkeys: play/pause");
    // TODO: send to media player
}

fn media_stop() {
    crate::kprintln!("fnkeys: stop");
}

fn media_previous() {
    crate::kprintln!("fnkeys: previous track");
}

fn media_next() {
    crate::kprintln!("fnkeys: next track");
}

// =============================================================================
// Application Launch
// =============================================================================

fn launch_calculator() {
    crate::kprintln!("fnkeys: launch calculator");
    // TODO: launch calculator app
}

fn launch_browser() {
    crate::kprintln!("fnkeys: launch browser");
}

fn launch_mail() {
    crate::kprintln!("fnkeys: launch mail");
}

fn open_search() {
    crate::kprintln!("fnkeys: open search");
}

fn lock_screen() {
    crate::kprintln!("fnkeys: lock screen");
    // TODO: trigger screen lock
}

fn capture_screen() {
    crate::kprintln!("fnkeys: screen capture");
    // TODO: trigger screenshot
}

fn open_settings() {
    crate::kprintln!("fnkeys: open settings");
}

// =============================================================================
// OSD (On-Screen Display)
// =============================================================================

fn show_osd(label: &str, value: u32) {
    let state = FN_STATE.lock();
    if !state.osd_enabled {
        return;
    }
    drop(state);

    // Just log for now; in a real implementation this would show a GUI popup
    crate::kprintln!("fnkeys: {} {}%", label, value);
}

/// Enable or disable OSD notifications
pub fn set_osd_enabled(enabled: bool) {
    FN_STATE.lock().osd_enabled = enabled;
}

// =============================================================================
// Custom Key Binding
// =============================================================================

/// Add a custom scancode binding
pub fn add_scancode_binding(scancode: u8, action: FnKeyAction) {
    FN_STATE.lock().key_bindings.insert(scancode, action);
}

/// Add a custom ACPI event binding
pub fn add_acpi_binding(event_code: u32, action: FnKeyAction) {
    FN_STATE.lock().acpi_bindings.insert(event_code, action);
}

/// Remove a scancode binding
pub fn remove_scancode_binding(scancode: u8) {
    FN_STATE.lock().key_bindings.remove(&scancode);
}

/// Get all key bindings
pub fn get_bindings() -> Vec<(u8, FnKeyAction)> {
    FN_STATE.lock()
        .key_bindings
        .iter()
        .map(|(&k, &v)| (k, v))
        .collect()
}

// =============================================================================
// sysfs Interface
// =============================================================================

/// Read function key status for sysfs
pub fn sysfs_read_status() -> String {
    let state = FN_STATE.lock();
    alloc::format!(
        "wireless_enabled: {}\n\
         touchpad_enabled: {}\n\
         airplane_mode: {}\n\
         performance_mode: {}\n\
         keyboard_backlight: {}\n\
         osd_enabled: {}\n",
        state.wireless_enabled.load(Ordering::Relaxed),
        state.touchpad_enabled.load(Ordering::Relaxed),
        state.airplane_mode.load(Ordering::Relaxed),
        PERFORMANCE_MODE.load(Ordering::Relaxed),
        KEYBOARD_BACKLIGHT_LEVEL.load(Ordering::Relaxed),
        state.osd_enabled,
    )
}

/// Write function key control for sysfs
pub fn sysfs_write_control(data: &str) -> bool {
    let parts: Vec<&str> = data.trim().split('=').collect();
    if parts.len() != 2 {
        return false;
    }

    let key = parts[0].trim();
    let value = parts[1].trim();

    match key {
        "wireless" => {
            if let Ok(enabled) = value.parse::<bool>() {
                set_wireless_enabled(enabled);
                return true;
            }
        }
        "touchpad" => {
            if let Ok(enabled) = value.parse::<bool>() {
                FN_STATE.lock().touchpad_enabled.store(enabled, Ordering::Relaxed);
                return true;
            }
        }
        "airplane_mode" => {
            if let Ok(enabled) = value.parse::<bool>() {
                FN_STATE.lock().airplane_mode.store(enabled, Ordering::Relaxed);
                if enabled {
                    set_wireless_enabled(false);
                }
                return true;
            }
        }
        "performance_mode" => {
            if let Ok(mode) = value.parse::<u8>() {
                if mode <= 2 {
                    PERFORMANCE_MODE.store(mode, Ordering::Relaxed);
                    return true;
                }
            }
        }
        "keyboard_backlight" => {
            if let Ok(level) = value.parse::<u8>() {
                if level <= KEYBOARD_BACKLIGHT_MAX {
                    set_keyboard_backlight(level);
                    return true;
                }
            }
        }
        "osd" => {
            if let Ok(enabled) = value.parse::<bool>() {
                set_osd_enabled(enabled);
                return true;
            }
        }
        _ => {}
    }

    false
}
