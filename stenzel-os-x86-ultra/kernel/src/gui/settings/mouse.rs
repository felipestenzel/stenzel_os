//! Mouse & Touchpad Settings
//!
//! Mouse speed, acceleration, touchpad gestures, and input settings.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use spin::Mutex;

/// Global mouse settings state
static MOUSE_SETTINGS: Mutex<Option<MouseSettings>> = Mutex::new(None);

/// Mouse settings state
pub struct MouseSettings {
    /// Mouse settings
    pub mouse: MouseConfig,
    /// Touchpad settings
    pub touchpad: TouchpadConfig,
    /// Connected pointing devices
    pub devices: Vec<PointingDevice>,
}

/// Mouse configuration
#[derive(Debug, Clone)]
pub struct MouseConfig {
    /// Speed (1-100)
    pub speed: u32,
    /// Acceleration enabled
    pub acceleration: bool,
    /// Acceleration profile
    pub acceleration_profile: AccelerationProfile,
    /// Natural scrolling (reverse scroll direction)
    pub natural_scroll: bool,
    /// Scroll speed (1-100)
    pub scroll_speed: u32,
    /// Primary button
    pub primary_button: PrimaryButton,
    /// Double click interval (ms)
    pub double_click_interval: u32,
    /// Middle click emulation
    pub middle_emulation: bool,
}

/// Touchpad configuration
#[derive(Debug, Clone)]
pub struct TouchpadConfig {
    /// Touchpad enabled
    pub enabled: bool,
    /// Disable while typing
    pub disable_while_typing: bool,
    /// Tap to click
    pub tap_to_click: bool,
    /// Two-finger tap for right click
    pub two_finger_right_click: bool,
    /// Three-finger tap action
    pub three_finger_tap: ThreeFingerTapAction,
    /// Natural scrolling
    pub natural_scroll: bool,
    /// Speed (1-100)
    pub speed: u32,
    /// Scroll method
    pub scroll_method: ScrollMethod,
    /// Edge scrolling enabled
    pub edge_scroll: bool,
    /// Click method
    pub click_method: ClickMethod,
    /// Gestures enabled
    pub gestures_enabled: bool,
    /// Gesture settings
    pub gestures: GestureSettings,
}

/// Pointing device
#[derive(Debug, Clone)]
pub struct PointingDevice {
    /// Device ID
    pub id: u32,
    /// Device name
    pub name: String,
    /// Device type
    pub device_type: PointingDeviceType,
    /// Is available
    pub available: bool,
}

/// Pointing device type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointingDeviceType {
    Mouse,
    Touchpad,
    Trackball,
    Trackpoint,
    Stylus,
    Unknown,
}

impl PointingDeviceType {
    pub fn name(&self) -> &'static str {
        match self {
            PointingDeviceType::Mouse => "Mouse",
            PointingDeviceType::Touchpad => "Touchpad",
            PointingDeviceType::Trackball => "Trackball",
            PointingDeviceType::Trackpoint => "TrackPoint",
            PointingDeviceType::Stylus => "Stylus",
            PointingDeviceType::Unknown => "Unknown",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            PointingDeviceType::Mouse => "input-mouse",
            PointingDeviceType::Touchpad => "input-touchpad",
            PointingDeviceType::Trackball => "input-mouse",
            PointingDeviceType::Trackpoint => "input-mouse",
            PointingDeviceType::Stylus => "input-tablet",
            PointingDeviceType::Unknown => "input-mouse",
        }
    }
}

/// Acceleration profile
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccelerationProfile {
    /// Flat - No acceleration
    Flat,
    /// Adaptive - Speed-dependent acceleration
    Adaptive,
}

impl AccelerationProfile {
    pub fn name(&self) -> &'static str {
        match self {
            AccelerationProfile::Flat => "None (Flat)",
            AccelerationProfile::Adaptive => "Adaptive",
        }
    }
}

/// Primary button
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimaryButton {
    Left,
    Right,
}

impl PrimaryButton {
    pub fn name(&self) -> &'static str {
        match self {
            PrimaryButton::Left => "Left",
            PrimaryButton::Right => "Right",
        }
    }
}

/// Three-finger tap action
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreeFingerTapAction {
    Nothing,
    MiddleClick,
    PasteClipboard,
    OpenAppMenu,
}

impl ThreeFingerTapAction {
    pub fn name(&self) -> &'static str {
        match self {
            ThreeFingerTapAction::Nothing => "Nothing",
            ThreeFingerTapAction::MiddleClick => "Middle Click",
            ThreeFingerTapAction::PasteClipboard => "Paste Clipboard",
            ThreeFingerTapAction::OpenAppMenu => "Open App Menu",
        }
    }
}

/// Scroll method
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollMethod {
    /// Two-finger scrolling
    TwoFinger,
    /// Edge scrolling
    Edge,
    /// Button scrolling
    Button,
    /// No scrolling
    None,
}

impl ScrollMethod {
    pub fn name(&self) -> &'static str {
        match self {
            ScrollMethod::TwoFinger => "Two-finger",
            ScrollMethod::Edge => "Edge",
            ScrollMethod::Button => "Button",
            ScrollMethod::None => "Disabled",
        }
    }
}

/// Click method
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickMethod {
    /// Clickpad buttons (areas)
    ButtonAreas,
    /// Clickpad with finger count
    ClickFinger,
    /// Physical buttons
    Physical,
}

impl ClickMethod {
    pub fn name(&self) -> &'static str {
        match self {
            ClickMethod::ButtonAreas => "Button Areas",
            ClickMethod::ClickFinger => "Finger Count",
            ClickMethod::Physical => "Physical Buttons",
        }
    }
}

/// Gesture settings
#[derive(Debug, Clone)]
pub struct GestureSettings {
    /// Three-finger swipe left
    pub swipe_3_left: GestureAction,
    /// Three-finger swipe right
    pub swipe_3_right: GestureAction,
    /// Three-finger swipe up
    pub swipe_3_up: GestureAction,
    /// Three-finger swipe down
    pub swipe_3_down: GestureAction,
    /// Four-finger swipe left
    pub swipe_4_left: GestureAction,
    /// Four-finger swipe right
    pub swipe_4_right: GestureAction,
    /// Four-finger swipe up
    pub swipe_4_up: GestureAction,
    /// Four-finger swipe down
    pub swipe_4_down: GestureAction,
    /// Pinch in
    pub pinch_in: GestureAction,
    /// Pinch out
    pub pinch_out: GestureAction,
}

/// Gesture action
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GestureAction {
    Nothing,
    SwitchWorkspaceNext,
    SwitchWorkspacePrev,
    ShowOverview,
    ShowAppGrid,
    MinimizeAll,
    NotificationCenter,
    ZoomIn,
    ZoomOut,
    Custom(u32),
}

impl GestureAction {
    pub fn name(&self) -> &'static str {
        match self {
            GestureAction::Nothing => "Nothing",
            GestureAction::SwitchWorkspaceNext => "Next Workspace",
            GestureAction::SwitchWorkspacePrev => "Previous Workspace",
            GestureAction::ShowOverview => "Show Overview",
            GestureAction::ShowAppGrid => "Show Application Grid",
            GestureAction::MinimizeAll => "Minimize All Windows",
            GestureAction::NotificationCenter => "Notification Center",
            GestureAction::ZoomIn => "Zoom In",
            GestureAction::ZoomOut => "Zoom Out",
            GestureAction::Custom(_) => "Custom Action",
        }
    }
}

/// Initialize mouse settings
pub fn init() {
    let mut state = MOUSE_SETTINGS.lock();
    if state.is_some() {
        return;
    }

    *state = Some(MouseSettings {
        mouse: MouseConfig {
            speed: 50,
            acceleration: true,
            acceleration_profile: AccelerationProfile::Adaptive,
            natural_scroll: false,
            scroll_speed: 50,
            primary_button: PrimaryButton::Left,
            double_click_interval: 400,
            middle_emulation: false,
        },
        touchpad: TouchpadConfig {
            enabled: true,
            disable_while_typing: true,
            tap_to_click: true,
            two_finger_right_click: true,
            three_finger_tap: ThreeFingerTapAction::MiddleClick,
            natural_scroll: true,
            speed: 50,
            scroll_method: ScrollMethod::TwoFinger,
            edge_scroll: false,
            click_method: ClickMethod::ClickFinger,
            gestures_enabled: true,
            gestures: GestureSettings {
                swipe_3_left: GestureAction::SwitchWorkspaceNext,
                swipe_3_right: GestureAction::SwitchWorkspacePrev,
                swipe_3_up: GestureAction::ShowOverview,
                swipe_3_down: GestureAction::Nothing,
                swipe_4_left: GestureAction::Nothing,
                swipe_4_right: GestureAction::Nothing,
                swipe_4_up: GestureAction::ShowAppGrid,
                swipe_4_down: GestureAction::NotificationCenter,
                pinch_in: GestureAction::ZoomOut,
                pinch_out: GestureAction::ZoomIn,
            },
        },
        devices: vec![
            PointingDevice {
                id: 0,
                name: "Built-in Touchpad".to_string(),
                device_type: PointingDeviceType::Touchpad,
                available: true,
            },
        ],
    });

    crate::kprintln!("mouse settings: initialized");
}

/// Get mouse config
pub fn get_mouse_config() -> Option<MouseConfig> {
    let state = MOUSE_SETTINGS.lock();
    state.as_ref().map(|s| s.mouse.clone())
}

/// Get touchpad config
pub fn get_touchpad_config() -> Option<TouchpadConfig> {
    let state = MOUSE_SETTINGS.lock();
    state.as_ref().map(|s| s.touchpad.clone())
}

/// Get pointing devices
pub fn get_devices() -> Vec<PointingDevice> {
    let state = MOUSE_SETTINGS.lock();
    state.as_ref().map(|s| s.devices.clone()).unwrap_or_default()
}

/// Set mouse speed
pub fn set_mouse_speed(speed: u32) {
    let mut state = MOUSE_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.mouse.speed = speed.clamp(1, 100);
    }
}

/// Set mouse acceleration
pub fn set_mouse_acceleration(enabled: bool) {
    let mut state = MOUSE_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.mouse.acceleration = enabled;
    }
}

/// Set acceleration profile
pub fn set_acceleration_profile(profile: AccelerationProfile) {
    let mut state = MOUSE_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.mouse.acceleration_profile = profile;
    }
}

/// Set mouse natural scroll
pub fn set_mouse_natural_scroll(enabled: bool) {
    let mut state = MOUSE_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.mouse.natural_scroll = enabled;
    }
}

/// Set primary button
pub fn set_primary_button(button: PrimaryButton) {
    let mut state = MOUSE_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.mouse.primary_button = button;
    }
}

/// Set double click interval
pub fn set_double_click_interval(ms: u32) {
    let mut state = MOUSE_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.mouse.double_click_interval = ms.clamp(100, 1000);
    }
}

/// Set touchpad enabled
pub fn set_touchpad_enabled(enabled: bool) {
    let mut state = MOUSE_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.touchpad.enabled = enabled;
    }
}

/// Set tap to click
pub fn set_tap_to_click(enabled: bool) {
    let mut state = MOUSE_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.touchpad.tap_to_click = enabled;
    }
}

/// Set touchpad natural scroll
pub fn set_touchpad_natural_scroll(enabled: bool) {
    let mut state = MOUSE_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.touchpad.natural_scroll = enabled;
    }
}

/// Set touchpad speed
pub fn set_touchpad_speed(speed: u32) {
    let mut state = MOUSE_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.touchpad.speed = speed.clamp(1, 100);
    }
}

/// Set scroll method
pub fn set_scroll_method(method: ScrollMethod) {
    let mut state = MOUSE_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.touchpad.scroll_method = method;
    }
}

/// Set click method
pub fn set_click_method(method: ClickMethod) {
    let mut state = MOUSE_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.touchpad.click_method = method;
    }
}

/// Set gestures enabled
pub fn set_gestures_enabled(enabled: bool) {
    let mut state = MOUSE_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.touchpad.gestures_enabled = enabled;
    }
}

/// Set disable while typing
pub fn set_disable_while_typing(enabled: bool) {
    let mut state = MOUSE_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.touchpad.disable_while_typing = enabled;
    }
}

/// Mouse error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseError {
    NotInitialized,
    DeviceNotFound,
    ConfigFailed,
}
