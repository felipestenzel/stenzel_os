//! Game Controller/Gamepad Driver
//!
//! Provides support for game controllers including:
//! - Xbox One/Series controllers
//! - PlayStation DualShock 4 / DualSense 5
//! - Nintendo Switch Pro Controller
//! - Generic USB/Bluetooth gamepads

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;

use crate::sync::IrqSafeMutex;

/// Gamepad button definitions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GamepadButton {
    /// A/Cross button
    South,
    /// B/Circle button
    East,
    /// X/Square button
    West,
    /// Y/Triangle button
    North,
    /// Left bumper/shoulder (L1/LB)
    LeftBumper,
    /// Right bumper/shoulder (R1/RB)
    RightBumper,
    /// Back/Select/Share/Minus
    Select,
    /// Start/Options/Plus
    Start,
    /// Guide/Home/PS button
    Guide,
    /// Left stick press (L3)
    LeftThumb,
    /// Right stick press (R3)
    RightThumb,
    /// D-pad up
    DPadUp,
    /// D-pad down
    DPadDown,
    /// D-pad left
    DPadLeft,
    /// D-pad right
    DPadRight,
    /// Left trigger (L2/LT) as button
    LeftTrigger,
    /// Right trigger (R2/RT) as button
    RightTrigger,
    /// Touchpad click (DualShock 4/5)
    Touchpad,
    /// Capture button (Switch Pro)
    Capture,
    /// Mute button (DualSense)
    Mute,
    /// Paddle 1 (Elite controllers)
    Paddle1,
    /// Paddle 2
    Paddle2,
    /// Paddle 3
    Paddle3,
    /// Paddle 4
    Paddle4,
}

impl GamepadButton {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::South => "south",
            Self::East => "east",
            Self::West => "west",
            Self::North => "north",
            Self::LeftBumper => "left_bumper",
            Self::RightBumper => "right_bumper",
            Self::Select => "select",
            Self::Start => "start",
            Self::Guide => "guide",
            Self::LeftThumb => "left_thumb",
            Self::RightThumb => "right_thumb",
            Self::DPadUp => "dpad_up",
            Self::DPadDown => "dpad_down",
            Self::DPadLeft => "dpad_left",
            Self::DPadRight => "dpad_right",
            Self::LeftTrigger => "left_trigger",
            Self::RightTrigger => "right_trigger",
            Self::Touchpad => "touchpad",
            Self::Capture => "capture",
            Self::Mute => "mute",
            Self::Paddle1 => "paddle1",
            Self::Paddle2 => "paddle2",
            Self::Paddle3 => "paddle3",
            Self::Paddle4 => "paddle4",
        }
    }

    /// Get Xbox-style name
    pub fn xbox_name(&self) -> &'static str {
        match self {
            Self::South => "A",
            Self::East => "B",
            Self::West => "X",
            Self::North => "Y",
            Self::LeftBumper => "LB",
            Self::RightBumper => "RB",
            Self::Select => "View",
            Self::Start => "Menu",
            Self::Guide => "Xbox",
            Self::LeftThumb => "LS",
            Self::RightThumb => "RS",
            Self::LeftTrigger => "LT",
            Self::RightTrigger => "RT",
            _ => self.as_str(),
        }
    }

    /// Get PlayStation-style name
    pub fn playstation_name(&self) -> &'static str {
        match self {
            Self::South => "Cross",
            Self::East => "Circle",
            Self::West => "Square",
            Self::North => "Triangle",
            Self::LeftBumper => "L1",
            Self::RightBumper => "R1",
            Self::Select => "Share",
            Self::Start => "Options",
            Self::Guide => "PS",
            Self::LeftThumb => "L3",
            Self::RightThumb => "R3",
            Self::LeftTrigger => "L2",
            Self::RightTrigger => "R2",
            _ => self.as_str(),
        }
    }

    /// Get Nintendo-style name
    pub fn nintendo_name(&self) -> &'static str {
        match self {
            Self::South => "B",
            Self::East => "A",
            Self::West => "Y",
            Self::North => "X",
            Self::LeftBumper => "L",
            Self::RightBumper => "R",
            Self::Select => "Minus",
            Self::Start => "Plus",
            Self::Guide => "Home",
            Self::LeftThumb => "LS",
            Self::RightThumb => "RS",
            Self::LeftTrigger => "ZL",
            Self::RightTrigger => "ZR",
            _ => self.as_str(),
        }
    }
}

/// Gamepad axis definitions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GamepadAxis {
    /// Left stick X (-1.0 to 1.0)
    LeftStickX,
    /// Left stick Y (-1.0 to 1.0)
    LeftStickY,
    /// Right stick X
    RightStickX,
    /// Right stick Y
    RightStickY,
    /// Left trigger (0.0 to 1.0)
    LeftTrigger,
    /// Right trigger (0.0 to 1.0)
    RightTrigger,
    /// D-pad X (-1.0, 0.0, 1.0)
    DPadX,
    /// D-pad Y (-1.0, 0.0, 1.0)
    DPadY,
    /// Gyroscope X (DualShock/DualSense/Switch)
    GyroX,
    /// Gyroscope Y
    GyroY,
    /// Gyroscope Z
    GyroZ,
    /// Accelerometer X
    AccelX,
    /// Accelerometer Y
    AccelY,
    /// Accelerometer Z
    AccelZ,
}

impl GamepadAxis {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::LeftStickX => "left_stick_x",
            Self::LeftStickY => "left_stick_y",
            Self::RightStickX => "right_stick_x",
            Self::RightStickY => "right_stick_y",
            Self::LeftTrigger => "left_trigger",
            Self::RightTrigger => "right_trigger",
            Self::DPadX => "dpad_x",
            Self::DPadY => "dpad_y",
            Self::GyroX => "gyro_x",
            Self::GyroY => "gyro_y",
            Self::GyroZ => "gyro_z",
            Self::AccelX => "accel_x",
            Self::AccelY => "accel_y",
            Self::AccelZ => "accel_z",
        }
    }

    pub fn is_trigger(&self) -> bool {
        matches!(self, Self::LeftTrigger | Self::RightTrigger)
    }

    pub fn is_stick(&self) -> bool {
        matches!(
            self,
            Self::LeftStickX | Self::LeftStickY | Self::RightStickX | Self::RightStickY
        )
    }

    pub fn is_motion(&self) -> bool {
        matches!(
            self,
            Self::GyroX | Self::GyroY | Self::GyroZ | Self::AccelX | Self::AccelY | Self::AccelZ
        )
    }
}

/// Gamepad type/brand
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamepadType {
    /// Xbox One/Series controller
    Xbox,
    /// Xbox 360 controller
    Xbox360,
    /// Xbox Elite controller
    XboxElite,
    /// PlayStation DualShock 4
    DualShock4,
    /// PlayStation DualSense 5
    DualSense,
    /// Nintendo Switch Pro Controller
    SwitchPro,
    /// Nintendo Joy-Con (pair)
    JoyCon,
    /// Steam Controller
    Steam,
    /// Generic USB gamepad
    GenericUsb,
    /// Generic Bluetooth gamepad
    GenericBluetooth,
    /// 8BitDo controller
    EightBitDo,
    /// Unknown type
    Unknown,
}

impl GamepadType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Xbox => "Xbox One/Series",
            Self::Xbox360 => "Xbox 360",
            Self::XboxElite => "Xbox Elite",
            Self::DualShock4 => "DualShock 4",
            Self::DualSense => "DualSense",
            Self::SwitchPro => "Switch Pro",
            Self::JoyCon => "Joy-Con",
            Self::Steam => "Steam Controller",
            Self::GenericUsb => "Generic USB",
            Self::GenericBluetooth => "Generic Bluetooth",
            Self::EightBitDo => "8BitDo",
            Self::Unknown => "Unknown",
        }
    }

    pub fn has_gyro(&self) -> bool {
        matches!(
            self,
            Self::DualShock4 | Self::DualSense | Self::SwitchPro | Self::JoyCon | Self::Steam
        )
    }

    pub fn has_touchpad(&self) -> bool {
        matches!(self, Self::DualShock4 | Self::DualSense | Self::Steam)
    }

    pub fn has_rumble(&self) -> bool {
        !matches!(self, Self::Unknown)
    }

    pub fn has_lightbar(&self) -> bool {
        matches!(self, Self::DualShock4 | Self::DualSense)
    }

    pub fn has_adaptive_triggers(&self) -> bool {
        matches!(self, Self::DualSense)
    }

    pub fn from_vendor_product(vendor_id: u16, product_id: u16) -> Self {
        match (vendor_id, product_id) {
            // Microsoft Xbox
            (0x045e, 0x02d1) => Self::Xbox,           // Xbox One
            (0x045e, 0x02e0) => Self::Xbox,           // Xbox One S (wireless adapter)
            (0x045e, 0x02ea) => Self::Xbox,           // Xbox One S
            (0x045e, 0x02fd) => Self::Xbox,           // Xbox One S Bluetooth
            (0x045e, 0x0b00) => Self::Xbox,           // Xbox Series X|S
            (0x045e, 0x0b12) => Self::Xbox,           // Xbox Series X|S Bluetooth
            (0x045e, 0x028e) => Self::Xbox360,        // Xbox 360
            (0x045e, 0x028f) => Self::Xbox360,        // Xbox 360 Wireless
            (0x045e, 0x02e3) => Self::XboxElite,      // Xbox Elite
            (0x045e, 0x0b05) => Self::XboxElite,      // Xbox Elite Series 2

            // Sony PlayStation
            (0x054c, 0x05c4) => Self::DualShock4,     // DualShock 4 v1
            (0x054c, 0x09cc) => Self::DualShock4,     // DualShock 4 v2
            (0x054c, 0x0ba0) => Self::DualShock4,     // DualShock 4 Wireless Adapter
            (0x054c, 0x0ce6) => Self::DualSense,      // DualSense
            (0x054c, 0x0df2) => Self::DualSense,      // DualSense Edge

            // Nintendo
            (0x057e, 0x2009) => Self::SwitchPro,      // Switch Pro Controller
            (0x057e, 0x2006) => Self::JoyCon,         // Joy-Con (L)
            (0x057e, 0x2007) => Self::JoyCon,         // Joy-Con (R)

            // Valve Steam
            (0x28de, 0x1102) => Self::Steam,          // Steam Controller
            (0x28de, 0x1142) => Self::Steam,          // Steam Controller Wireless

            // 8BitDo
            (0x2dc8, _) => Self::EightBitDo,

            // Generic HID gamepads
            _ => Self::GenericUsb,
        }
    }
}

/// Gamepad connection type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionType {
    /// USB wired
    Usb,
    /// Bluetooth
    Bluetooth,
    /// Proprietary wireless (Xbox dongle, etc.)
    ProprietaryWireless,
    /// Unknown
    Unknown,
}

impl ConnectionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Usb => "USB",
            Self::Bluetooth => "Bluetooth",
            Self::ProprietaryWireless => "Wireless",
            Self::Unknown => "Unknown",
        }
    }
}

/// Gamepad event type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamepadEventType {
    /// Button pressed
    ButtonPressed,
    /// Button released
    ButtonReleased,
    /// Axis moved
    AxisMoved,
    /// Controller connected
    Connected,
    /// Controller disconnected
    Disconnected,
    /// Battery level changed
    BatteryChanged,
}

/// Gamepad event
#[derive(Debug, Clone, Copy)]
pub struct GamepadEvent {
    /// Event type
    pub event_type: GamepadEventType,
    /// Controller ID
    pub controller_id: u32,
    /// Button (if button event)
    pub button: Option<GamepadButton>,
    /// Axis (if axis event)
    pub axis: Option<GamepadAxis>,
    /// Value (button: 0/1, axis: -1.0 to 1.0 scaled to i32)
    pub value: i32,
    /// Timestamp
    pub timestamp: u64,
}

impl GamepadEvent {
    pub fn connected(id: u32) -> Self {
        Self {
            event_type: GamepadEventType::Connected,
            controller_id: id,
            button: None,
            axis: None,
            value: 0,
            timestamp: crate::time::uptime_ms(),
        }
    }

    pub fn disconnected(id: u32) -> Self {
        Self {
            event_type: GamepadEventType::Disconnected,
            controller_id: id,
            button: None,
            axis: None,
            value: 0,
            timestamp: crate::time::uptime_ms(),
        }
    }

    pub fn button_pressed(id: u32, button: GamepadButton) -> Self {
        Self {
            event_type: GamepadEventType::ButtonPressed,
            controller_id: id,
            button: Some(button),
            axis: None,
            value: 1,
            timestamp: crate::time::uptime_ms(),
        }
    }

    pub fn button_released(id: u32, button: GamepadButton) -> Self {
        Self {
            event_type: GamepadEventType::ButtonReleased,
            controller_id: id,
            button: Some(button),
            axis: None,
            value: 0,
            timestamp: crate::time::uptime_ms(),
        }
    }

    pub fn axis_moved(id: u32, axis: GamepadAxis, value: i32) -> Self {
        Self {
            event_type: GamepadEventType::AxisMoved,
            controller_id: id,
            button: None,
            axis: Some(axis),
            value,
            timestamp: crate::time::uptime_ms(),
        }
    }
}

/// Rumble effect
#[derive(Debug, Clone, Copy)]
pub struct RumbleEffect {
    /// Strong motor intensity (0-255)
    pub strong_magnitude: u8,
    /// Weak motor intensity (0-255)
    pub weak_magnitude: u8,
    /// Duration in milliseconds (0 = indefinite)
    pub duration_ms: u32,
}

impl RumbleEffect {
    pub fn new(strong: u8, weak: u8, duration_ms: u32) -> Self {
        Self {
            strong_magnitude: strong,
            weak_magnitude: weak,
            duration_ms,
        }
    }

    pub fn off() -> Self {
        Self::new(0, 0, 0)
    }

    pub fn light(duration_ms: u32) -> Self {
        Self::new(64, 64, duration_ms)
    }

    pub fn medium(duration_ms: u32) -> Self {
        Self::new(128, 128, duration_ms)
    }

    pub fn heavy(duration_ms: u32) -> Self {
        Self::new(255, 255, duration_ms)
    }
}

/// Lightbar color (DualShock 4/DualSense)
#[derive(Debug, Clone, Copy)]
pub struct LightbarColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl LightbarColor {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn off() -> Self {
        Self::new(0, 0, 0)
    }

    pub fn red() -> Self {
        Self::new(255, 0, 0)
    }

    pub fn green() -> Self {
        Self::new(0, 255, 0)
    }

    pub fn blue() -> Self {
        Self::new(0, 0, 255)
    }

    pub fn white() -> Self {
        Self::new(255, 255, 255)
    }

    pub fn player_color(player: u8) -> Self {
        match player {
            1 => Self::new(0, 0, 255),   // Blue
            2 => Self::new(255, 0, 0),   // Red
            3 => Self::new(0, 255, 0),   // Green
            4 => Self::new(255, 0, 255), // Magenta
            _ => Self::white(),
        }
    }
}

/// Adaptive trigger effect (DualSense)
#[derive(Debug, Clone, Copy)]
pub enum AdaptiveTriggerEffect {
    /// No effect
    Off,
    /// Continuous resistance
    Continuous { start: u8, force: u8 },
    /// Sectioned resistance
    Section { start: u8, end: u8, force: u8 },
    /// Vibrating trigger
    Vibration { position: u8, amplitude: u8, frequency: u8 },
    /// Weapon-like effect
    Weapon { start: u8, end: u8, force: u8 },
}

impl AdaptiveTriggerEffect {
    pub fn to_bytes(&self) -> [u8; 11] {
        let mut data = [0u8; 11];
        match self {
            Self::Off => {
                data[0] = 0x05;
            }
            Self::Continuous { start, force } => {
                data[0] = 0x01;
                data[1] = *start;
                data[2] = *force;
            }
            Self::Section { start, end, force } => {
                data[0] = 0x02;
                data[1] = *start;
                data[2] = *end;
                data[3] = *force;
            }
            Self::Vibration { position, amplitude, frequency } => {
                data[0] = 0x06;
                data[1] = *position;
                data[2] = *amplitude;
                data[3] = *frequency;
            }
            Self::Weapon { start, end, force } => {
                data[0] = 0x26;
                data[1] = *start;
                data[2] = *end;
                data[3] = *force;
            }
        }
        data
    }
}

/// Battery status
#[derive(Debug, Clone, Copy)]
pub struct BatteryStatus {
    /// Battery level (0-100)
    pub level: u8,
    /// Is charging
    pub charging: bool,
    /// Is connected to power
    pub powered: bool,
}

impl BatteryStatus {
    pub fn wired() -> Self {
        Self {
            level: 100,
            charging: false,
            powered: true,
        }
    }

    pub fn full() -> Self {
        Self {
            level: 100,
            charging: false,
            powered: false,
        }
    }
}

/// Gamepad button state (bitmap)
#[derive(Debug, Clone, Copy, Default)]
pub struct ButtonState {
    bits: u32,
}

impl ButtonState {
    pub fn new() -> Self {
        Self { bits: 0 }
    }

    pub fn is_pressed(&self, button: GamepadButton) -> bool {
        let bit = button as u32;
        (self.bits & (1 << bit)) != 0
    }

    pub fn set_pressed(&mut self, button: GamepadButton, pressed: bool) {
        let bit = button as u32;
        if pressed {
            self.bits |= 1 << bit;
        } else {
            self.bits &= !(1 << bit);
        }
    }

    pub fn any_pressed(&self) -> bool {
        self.bits != 0
    }

    pub fn pressed_buttons(&self) -> Vec<GamepadButton> {
        let mut result = Vec::new();
        let buttons = [
            GamepadButton::South,
            GamepadButton::East,
            GamepadButton::West,
            GamepadButton::North,
            GamepadButton::LeftBumper,
            GamepadButton::RightBumper,
            GamepadButton::Select,
            GamepadButton::Start,
            GamepadButton::Guide,
            GamepadButton::LeftThumb,
            GamepadButton::RightThumb,
            GamepadButton::DPadUp,
            GamepadButton::DPadDown,
            GamepadButton::DPadLeft,
            GamepadButton::DPadRight,
            GamepadButton::LeftTrigger,
            GamepadButton::RightTrigger,
            GamepadButton::Touchpad,
            GamepadButton::Capture,
            GamepadButton::Mute,
        ];
        for button in buttons {
            if self.is_pressed(button) {
                result.push(button);
            }
        }
        result
    }
}

/// Gamepad axis state
#[derive(Debug, Clone, Copy, Default)]
pub struct AxisState {
    /// Left stick X (-32768 to 32767)
    pub left_stick_x: i16,
    /// Left stick Y
    pub left_stick_y: i16,
    /// Right stick X
    pub right_stick_x: i16,
    /// Right stick Y
    pub right_stick_y: i16,
    /// Left trigger (0 to 255)
    pub left_trigger: u8,
    /// Right trigger
    pub right_trigger: u8,
    /// Gyroscope X
    pub gyro_x: i16,
    /// Gyroscope Y
    pub gyro_y: i16,
    /// Gyroscope Z
    pub gyro_z: i16,
    /// Accelerometer X
    pub accel_x: i16,
    /// Accelerometer Y
    pub accel_y: i16,
    /// Accelerometer Z
    pub accel_z: i16,
}

impl AxisState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get axis value normalized to -1000..1000 for sticks, 0..1000 for triggers
    pub fn get_axis(&self, axis: GamepadAxis) -> i32 {
        match axis {
            GamepadAxis::LeftStickX => (self.left_stick_x as i32 * 1000) / 32768,
            GamepadAxis::LeftStickY => (self.left_stick_y as i32 * 1000) / 32768,
            GamepadAxis::RightStickX => (self.right_stick_x as i32 * 1000) / 32768,
            GamepadAxis::RightStickY => (self.right_stick_y as i32 * 1000) / 32768,
            GamepadAxis::LeftTrigger => (self.left_trigger as i32 * 1000) / 255,
            GamepadAxis::RightTrigger => (self.right_trigger as i32 * 1000) / 255,
            GamepadAxis::GyroX => self.gyro_x as i32,
            GamepadAxis::GyroY => self.gyro_y as i32,
            GamepadAxis::GyroZ => self.gyro_z as i32,
            GamepadAxis::AccelX => self.accel_x as i32,
            GamepadAxis::AccelY => self.accel_y as i32,
            GamepadAxis::AccelZ => self.accel_z as i32,
            GamepadAxis::DPadX | GamepadAxis::DPadY => 0, // Handled as buttons
        }
    }
}

/// Gamepad configuration
#[derive(Debug, Clone)]
pub struct GamepadConfig {
    /// Dead zone for sticks (0-32767)
    pub stick_deadzone: u16,
    /// Trigger threshold (0-255)
    pub trigger_threshold: u8,
    /// Invert left stick Y
    pub invert_left_y: bool,
    /// Invert right stick Y
    pub invert_right_y: bool,
    /// Enable rumble
    pub rumble_enabled: bool,
    /// Rumble intensity scale (0-100)
    pub rumble_scale: u8,
    /// Enable gyro
    pub gyro_enabled: bool,
    /// Gyro sensitivity scale (0-200, 100 = default)
    pub gyro_sensitivity: u8,
    /// Player number (1-4, for lightbar color)
    pub player_number: u8,
}

impl GamepadConfig {
    pub fn default() -> Self {
        Self {
            stick_deadzone: 4000,
            trigger_threshold: 10,
            invert_left_y: false,
            invert_right_y: false,
            rumble_enabled: true,
            rumble_scale: 100,
            gyro_enabled: false,
            gyro_sensitivity: 100,
            player_number: 1,
        }
    }
}

/// Gamepad statistics
#[derive(Debug, Clone, Copy, Default)]
pub struct GamepadStats {
    /// Total events processed
    pub events_processed: u64,
    /// Button presses
    pub button_presses: u64,
    /// Total runtime (ms)
    pub runtime_ms: u64,
    /// Connection count
    pub connection_count: u32,
    /// Disconnection count
    pub disconnection_count: u32,
    /// Rumble commands sent
    pub rumble_commands: u64,
    /// Last event time
    pub last_event_time: u64,
}

/// Gamepad device
pub struct GamepadDevice {
    /// Device ID
    pub id: u32,
    /// Vendor ID
    pub vendor_id: u16,
    /// Product ID
    pub product_id: u16,
    /// Device name
    pub name: String,
    /// Gamepad type
    pub gamepad_type: GamepadType,
    /// Connection type
    pub connection: ConnectionType,
    /// Button state
    pub buttons: ButtonState,
    /// Axis state
    pub axes: AxisState,
    /// Battery status
    pub battery: BatteryStatus,
    /// Configuration
    pub config: GamepadConfig,
    /// Statistics
    pub stats: GamepadStats,
    /// Is connected
    pub connected: bool,
    /// Connect time
    pub connect_time: u64,
    /// Previous button state (for edge detection)
    prev_buttons: ButtonState,
    /// Previous axis state (for change detection)
    prev_axes: AxisState,
}

impl GamepadDevice {
    pub fn new(
        id: u32,
        vendor_id: u16,
        product_id: u16,
        name: String,
        connection: ConnectionType,
    ) -> Self {
        let gamepad_type = GamepadType::from_vendor_product(vendor_id, product_id);
        let battery = if connection == ConnectionType::Usb {
            BatteryStatus::wired()
        } else {
            BatteryStatus::full()
        };

        Self {
            id,
            vendor_id,
            product_id,
            name,
            gamepad_type,
            connection,
            buttons: ButtonState::new(),
            axes: AxisState::new(),
            battery,
            config: GamepadConfig::default(),
            stats: GamepadStats::default(),
            connected: true,
            connect_time: crate::time::uptime_ms(),
            prev_buttons: ButtonState::new(),
            prev_axes: AxisState::new(),
        }
    }

    /// Apply deadzone to stick value
    fn apply_deadzone(&self, value: i16) -> i16 {
        let deadzone = self.config.stick_deadzone as i32;
        let v = value as i32;
        if v.abs() < deadzone {
            0
        } else {
            // Scale to full range after deadzone
            let sign = if v < 0 { -1 } else { 1 };
            let scaled = ((v.abs() - deadzone) * 32767) / (32767 - deadzone);
            (scaled * sign) as i16
        }
    }

    /// Process raw input report
    pub fn process_input(&mut self, report: &GamepadRawInput) -> Vec<GamepadEvent> {
        let mut events = Vec::new();

        // Store previous state
        self.prev_buttons = self.buttons;
        self.prev_axes = self.axes;

        // Update button state
        for button in &report.buttons {
            self.buttons.set_pressed(*button, true);
        }

        // Check for button state changes
        let all_buttons = [
            GamepadButton::South,
            GamepadButton::East,
            GamepadButton::West,
            GamepadButton::North,
            GamepadButton::LeftBumper,
            GamepadButton::RightBumper,
            GamepadButton::Select,
            GamepadButton::Start,
            GamepadButton::Guide,
            GamepadButton::LeftThumb,
            GamepadButton::RightThumb,
            GamepadButton::DPadUp,
            GamepadButton::DPadDown,
            GamepadButton::DPadLeft,
            GamepadButton::DPadRight,
            GamepadButton::Touchpad,
            GamepadButton::Capture,
            GamepadButton::Mute,
        ];

        // Clear buttons not in report
        for button in all_buttons {
            if !report.buttons.contains(&button) {
                self.buttons.set_pressed(button, false);
            }
        }

        // Generate button events
        for button in all_buttons {
            let was_pressed = self.prev_buttons.is_pressed(button);
            let is_pressed = self.buttons.is_pressed(button);

            if is_pressed && !was_pressed {
                events.push(GamepadEvent::button_pressed(self.id, button));
                self.stats.button_presses += 1;
            } else if !is_pressed && was_pressed {
                events.push(GamepadEvent::button_released(self.id, button));
            }
        }

        // Update and apply deadzone to axes
        self.axes.left_stick_x = self.apply_deadzone(report.left_stick_x);
        self.axes.left_stick_y = if self.config.invert_left_y {
            self.apply_deadzone(-report.left_stick_y)
        } else {
            self.apply_deadzone(report.left_stick_y)
        };
        self.axes.right_stick_x = self.apply_deadzone(report.right_stick_x);
        self.axes.right_stick_y = if self.config.invert_right_y {
            self.apply_deadzone(-report.right_stick_y)
        } else {
            self.apply_deadzone(report.right_stick_y)
        };

        // Triggers with threshold
        self.axes.left_trigger = if report.left_trigger > self.config.trigger_threshold {
            report.left_trigger
        } else {
            0
        };
        self.axes.right_trigger = if report.right_trigger > self.config.trigger_threshold {
            report.right_trigger
        } else {
            0
        };

        // Trigger as button
        let lt_pressed = self.axes.left_trigger > 128;
        let rt_pressed = self.axes.right_trigger > 128;
        let prev_lt = self.prev_axes.left_trigger > 128;
        let prev_rt = self.prev_axes.right_trigger > 128;

        if lt_pressed && !prev_lt {
            events.push(GamepadEvent::button_pressed(self.id, GamepadButton::LeftTrigger));
        } else if !lt_pressed && prev_lt {
            events.push(GamepadEvent::button_released(self.id, GamepadButton::LeftTrigger));
        }

        if rt_pressed && !prev_rt {
            events.push(GamepadEvent::button_pressed(self.id, GamepadButton::RightTrigger));
        } else if !rt_pressed && prev_rt {
            events.push(GamepadEvent::button_released(self.id, GamepadButton::RightTrigger));
        }

        // Motion (if enabled)
        if self.config.gyro_enabled {
            let scale = self.config.gyro_sensitivity as i32;
            self.axes.gyro_x = ((report.gyro_x as i32 * scale) / 100) as i16;
            self.axes.gyro_y = ((report.gyro_y as i32 * scale) / 100) as i16;
            self.axes.gyro_z = ((report.gyro_z as i32 * scale) / 100) as i16;
            self.axes.accel_x = report.accel_x;
            self.axes.accel_y = report.accel_y;
            self.axes.accel_z = report.accel_z;
        }

        // Generate axis events for significant changes
        let axes_to_check = [
            (GamepadAxis::LeftStickX, self.prev_axes.left_stick_x as i32, self.axes.left_stick_x as i32),
            (GamepadAxis::LeftStickY, self.prev_axes.left_stick_y as i32, self.axes.left_stick_y as i32),
            (GamepadAxis::RightStickX, self.prev_axes.right_stick_x as i32, self.axes.right_stick_x as i32),
            (GamepadAxis::RightStickY, self.prev_axes.right_stick_y as i32, self.axes.right_stick_y as i32),
            (GamepadAxis::LeftTrigger, self.prev_axes.left_trigger as i32, self.axes.left_trigger as i32),
            (GamepadAxis::RightTrigger, self.prev_axes.right_trigger as i32, self.axes.right_trigger as i32),
        ];

        for (axis, prev, curr) in axes_to_check {
            if (curr - prev).abs() > 100 {
                events.push(GamepadEvent::axis_moved(self.id, axis, self.axes.get_axis(axis)));
            }
        }

        // Update battery
        if let Some(battery_level) = report.battery_level {
            self.battery.level = battery_level;
            self.battery.charging = report.battery_charging;
        }

        // Update stats
        self.stats.events_processed += events.len() as u64;
        self.stats.last_event_time = crate::time::uptime_ms();

        events
    }

    /// Get rumble command bytes (device-specific)
    pub fn build_rumble_command(&self, effect: &RumbleEffect) -> Vec<u8> {
        match self.gamepad_type {
            GamepadType::Xbox | GamepadType::Xbox360 | GamepadType::XboxElite => {
                // Xbox rumble command
                vec![
                    0x09,                              // Report ID
                    0x00,                              // Subtype
                    0x0F,                              // Enable flags
                    effect.strong_magnitude / 26,     // Left trigger
                    effect.weak_magnitude / 26,       // Right trigger
                    effect.strong_magnitude,          // Left motor
                    effect.weak_magnitude,            // Right motor
                    (effect.duration_ms / 10) as u8,  // Duration
                    0x00,                              // Delay
                    0x00,                              // Repeat
                ]
            }
            GamepadType::DualShock4 => {
                // DS4 output report
                let mut cmd = vec![0u8; 32];
                cmd[0] = 0x05;  // Report ID
                cmd[1] = 0xFF;  // Flags
                cmd[4] = effect.weak_magnitude;
                cmd[5] = effect.strong_magnitude;
                cmd
            }
            GamepadType::DualSense => {
                // DualSense output report
                let mut cmd = vec![0u8; 48];
                cmd[0] = 0x02;  // Report ID
                cmd[1] = 0xFF;  // Flags low
                cmd[2] = 0xF7;  // Flags high
                cmd[3] = effect.weak_magnitude;
                cmd[4] = effect.strong_magnitude;
                cmd
            }
            GamepadType::SwitchPro => {
                // Switch Pro rumble (simplified)
                vec![
                    0x10,                              // Subcommand
                    0x00,                              // Rumble data (simplified)
                    effect.strong_magnitude,
                    0x00,
                    effect.weak_magnitude,
                    0x00, 0x00, 0x00, 0x00,
                ]
            }
            _ => {
                // Generic HID rumble
                vec![effect.strong_magnitude, effect.weak_magnitude]
            }
        }
    }
}

/// Raw input from device
#[derive(Debug, Clone)]
pub struct GamepadRawInput {
    pub buttons: Vec<GamepadButton>,
    pub left_stick_x: i16,
    pub left_stick_y: i16,
    pub right_stick_x: i16,
    pub right_stick_y: i16,
    pub left_trigger: u8,
    pub right_trigger: u8,
    pub gyro_x: i16,
    pub gyro_y: i16,
    pub gyro_z: i16,
    pub accel_x: i16,
    pub accel_y: i16,
    pub accel_z: i16,
    pub battery_level: Option<u8>,
    pub battery_charging: bool,
}

impl GamepadRawInput {
    pub fn new() -> Self {
        Self {
            buttons: Vec::new(),
            left_stick_x: 0,
            left_stick_y: 0,
            right_stick_x: 0,
            right_stick_y: 0,
            left_trigger: 0,
            right_trigger: 0,
            gyro_x: 0,
            gyro_y: 0,
            gyro_z: 0,
            accel_x: 0,
            accel_y: 0,
            accel_z: 0,
            battery_level: None,
            battery_charging: false,
        }
    }
}

/// Event callback type
pub type GamepadEventCallback = fn(GamepadEvent);

/// Gamepad manager state
struct GamepadManagerState {
    /// Connected gamepads
    gamepads: BTreeMap<u32, GamepadDevice>,
    /// Next ID
    next_id: u32,
    /// Event callback
    on_event: Option<GamepadEventCallback>,
    /// Initialized
    initialized: bool,
}

impl GamepadManagerState {
    const fn new() -> Self {
        Self {
            gamepads: BTreeMap::new(),
            next_id: 1,
            on_event: None,
            initialized: false,
        }
    }
}

/// Global gamepad manager
static GAMEPAD_MANAGER: IrqSafeMutex<GamepadManagerState> = IrqSafeMutex::new(GamepadManagerState::new());

/// Gamepad manager
pub struct GamepadManager;

impl GamepadManager {
    /// Initialize gamepad manager
    pub fn init() {
        let mut state = GAMEPAD_MANAGER.lock();
        if state.initialized {
            return;
        }
        state.initialized = true;
        crate::kprintln!("[gamepad] Gamepad manager initialized");
    }

    /// Register a new gamepad
    pub fn register_gamepad(
        vendor_id: u16,
        product_id: u16,
        name: &str,
        connection: ConnectionType,
    ) -> u32 {
        let mut state = GAMEPAD_MANAGER.lock();

        let id = state.next_id;
        state.next_id += 1;

        let device = GamepadDevice::new(id, vendor_id, product_id, String::from(name), connection);

        crate::kprintln!(
            "[gamepad] Registered {} ({}) as controller {} via {}",
            name,
            device.gamepad_type.as_str(),
            id,
            connection.as_str()
        );

        state.gamepads.insert(id, device);

        // Fire connected event
        if let Some(callback) = state.on_event {
            callback(GamepadEvent::connected(id));
        }

        id
    }

    /// Unregister a gamepad
    pub fn unregister_gamepad(id: u32) -> bool {
        let mut state = GAMEPAD_MANAGER.lock();

        if state.gamepads.remove(&id).is_some() {
            crate::kprintln!("[gamepad] Unregistered controller {}", id);

            // Fire disconnected event
            if let Some(callback) = state.on_event {
                callback(GamepadEvent::disconnected(id));
            }

            true
        } else {
            false
        }
    }

    /// Process input for a gamepad
    pub fn process_input(id: u32, input: GamepadRawInput) -> Vec<GamepadEvent> {
        let mut state = GAMEPAD_MANAGER.lock();

        if let Some(gamepad) = state.gamepads.get_mut(&id) {
            let events = gamepad.process_input(&input);

            // Fire events
            if let Some(callback) = state.on_event {
                for event in &events {
                    callback(*event);
                }
            }

            events
        } else {
            Vec::new()
        }
    }

    /// Set rumble on a gamepad
    pub fn set_rumble(id: u32, effect: RumbleEffect) -> Option<Vec<u8>> {
        let mut state = GAMEPAD_MANAGER.lock();

        if let Some(gamepad) = state.gamepads.get_mut(&id) {
            if !gamepad.config.rumble_enabled {
                return None;
            }

            // Scale rumble
            let scaled_effect = RumbleEffect {
                strong_magnitude: ((effect.strong_magnitude as u32 * gamepad.config.rumble_scale as u32) / 100) as u8,
                weak_magnitude: ((effect.weak_magnitude as u32 * gamepad.config.rumble_scale as u32) / 100) as u8,
                duration_ms: effect.duration_ms,
            };

            gamepad.stats.rumble_commands += 1;
            Some(gamepad.build_rumble_command(&scaled_effect))
        } else {
            None
        }
    }

    /// Stop rumble
    pub fn stop_rumble(id: u32) -> Option<Vec<u8>> {
        Self::set_rumble(id, RumbleEffect::off())
    }

    /// Set event callback
    pub fn set_event_callback(callback: GamepadEventCallback) {
        let mut state = GAMEPAD_MANAGER.lock();
        state.on_event = Some(callback);
    }

    /// Get gamepad info
    pub fn get_gamepad(id: u32) -> Option<(String, GamepadType, ConnectionType)> {
        let state = GAMEPAD_MANAGER.lock();
        state.gamepads.get(&id).map(|g| {
            (g.name.clone(), g.gamepad_type, g.connection)
        })
    }

    /// Get button state
    pub fn get_button_state(id: u32) -> Option<ButtonState> {
        let state = GAMEPAD_MANAGER.lock();
        state.gamepads.get(&id).map(|g| g.buttons)
    }

    /// Get axis state
    pub fn get_axis_state(id: u32) -> Option<AxisState> {
        let state = GAMEPAD_MANAGER.lock();
        state.gamepads.get(&id).map(|g| g.axes)
    }

    /// Get battery status
    pub fn get_battery(id: u32) -> Option<BatteryStatus> {
        let state = GAMEPAD_MANAGER.lock();
        state.gamepads.get(&id).map(|g| g.battery)
    }

    /// List all gamepads
    pub fn list_gamepads() -> Vec<(u32, String, GamepadType)> {
        let state = GAMEPAD_MANAGER.lock();
        state.gamepads.iter()
            .map(|(&id, g)| (id, g.name.clone(), g.gamepad_type))
            .collect()
    }

    /// Configure gamepad
    pub fn configure(id: u32, config: GamepadConfig) -> bool {
        let mut state = GAMEPAD_MANAGER.lock();
        if let Some(gamepad) = state.gamepads.get_mut(&id) {
            gamepad.config = config;
            true
        } else {
            false
        }
    }

    /// Set stick deadzone
    pub fn set_deadzone(id: u32, deadzone: u16) -> bool {
        let mut state = GAMEPAD_MANAGER.lock();
        if let Some(gamepad) = state.gamepads.get_mut(&id) {
            gamepad.config.stick_deadzone = deadzone.min(32000);
            true
        } else {
            false
        }
    }

    /// Count connected gamepads
    pub fn gamepad_count() -> usize {
        let state = GAMEPAD_MANAGER.lock();
        state.gamepads.len()
    }

    /// Check if initialized
    pub fn is_initialized() -> bool {
        let state = GAMEPAD_MANAGER.lock();
        state.initialized
    }

    /// Format status
    pub fn format_status() -> String {
        let state = GAMEPAD_MANAGER.lock();
        use alloc::fmt::Write;
        let mut s = String::new();

        let _ = writeln!(s, "Gamepad Manager Status:");
        let _ = writeln!(s, "  Initialized: {}", state.initialized);
        let _ = writeln!(s, "  Controllers: {}", state.gamepads.len());

        for (id, gamepad) in &state.gamepads {
            let _ = writeln!(s, "\n  Controller {}:", id);
            let _ = writeln!(s, "    Name: {}", gamepad.name);
            let _ = writeln!(s, "    Type: {}", gamepad.gamepad_type.as_str());
            let _ = writeln!(s, "    Connection: {}", gamepad.connection.as_str());
            let _ = writeln!(s, "    VID:PID: {:04x}:{:04x}", gamepad.vendor_id, gamepad.product_id);
            let _ = writeln!(s, "    Battery: {}%{}", gamepad.battery.level,
                if gamepad.battery.charging { " (charging)" } else { "" });
            let _ = writeln!(s, "    Stats:");
            let _ = writeln!(s, "      Events: {}", gamepad.stats.events_processed);
            let _ = writeln!(s, "      Button presses: {}", gamepad.stats.button_presses);
            let _ = writeln!(s, "      Rumble commands: {}", gamepad.stats.rumble_commands);
        }

        s
    }
}

// ============================================================================
// Public API
// ============================================================================

/// Initialize gamepad subsystem
pub fn init() {
    GamepadManager::init();
}

/// Register a gamepad
pub fn register_gamepad(vendor_id: u16, product_id: u16, name: &str, connection: ConnectionType) -> u32 {
    GamepadManager::register_gamepad(vendor_id, product_id, name, connection)
}

/// Unregister a gamepad
pub fn unregister_gamepad(id: u32) -> bool {
    GamepadManager::unregister_gamepad(id)
}

/// Process input
pub fn process_input(id: u32, input: GamepadRawInput) -> Vec<GamepadEvent> {
    GamepadManager::process_input(id, input)
}

/// Set rumble
pub fn set_rumble(id: u32, strong: u8, weak: u8, duration_ms: u32) -> Option<Vec<u8>> {
    GamepadManager::set_rumble(id, RumbleEffect::new(strong, weak, duration_ms))
}

/// Stop rumble
pub fn stop_rumble(id: u32) -> Option<Vec<u8>> {
    GamepadManager::stop_rumble(id)
}

/// Set event callback
pub fn set_event_callback(callback: GamepadEventCallback) {
    GamepadManager::set_event_callback(callback);
}

/// List gamepads
pub fn list_gamepads() -> Vec<(u32, String, GamepadType)> {
    GamepadManager::list_gamepads()
}

/// Get status
pub fn status() -> String {
    GamepadManager::format_status()
}
