//! Bluetooth HID Profile
//!
//! Implements the Bluetooth Human Interface Device profile for:
//! - Keyboards
//! - Mice
//! - Gamepads/Joysticks
//! - Other HID devices
//!
//! Uses L2CAP channels:
//! - Control channel (PSM 0x0011)
//! - Interrupt channel (PSM 0x0013)
//!
//! References:
//! - Bluetooth HID Profile 1.1
//! - USB HID Usage Tables 1.12

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use crate::sync::TicketSpinlock;
use super::{BdAddr, l2cap};

/// L2CAP PSM values for HID
pub mod psm {
    /// HID Control channel PSM
    pub const HID_CONTROL: u16 = 0x0011;
    /// HID Interrupt channel PSM
    pub const HID_INTERRUPT: u16 = 0x0013;
}

/// HID transaction types (4 bits)
pub mod transaction {
    pub const HANDSHAKE: u8 = 0x00;
    pub const HID_CONTROL: u8 = 0x01;
    pub const GET_REPORT: u8 = 0x04;
    pub const SET_REPORT: u8 = 0x05;
    pub const GET_PROTOCOL: u8 = 0x06;
    pub const SET_PROTOCOL: u8 = 0x07;
    pub const GET_IDLE: u8 = 0x08;
    pub const SET_IDLE: u8 = 0x09;
    pub const DATA: u8 = 0x0A;
    pub const DATC: u8 = 0x0B;
}

/// HID handshake result codes
pub mod handshake {
    pub const SUCCESSFUL: u8 = 0x00;
    pub const NOT_READY: u8 = 0x01;
    pub const ERR_INVALID_REPORT_ID: u8 = 0x02;
    pub const ERR_UNSUPPORTED_REQUEST: u8 = 0x03;
    pub const ERR_INVALID_PARAMETER: u8 = 0x04;
    pub const ERR_UNKNOWN: u8 = 0x0E;
    pub const ERR_FATAL: u8 = 0x0F;
}

/// HID control parameters
pub mod control {
    pub const SUSPEND: u8 = 0x03;
    pub const EXIT_SUSPEND: u8 = 0x04;
    pub const VIRTUAL_CABLE_UNPLUG: u8 = 0x05;
}

/// HID report types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ReportType {
    Input = 0x01,
    Output = 0x02,
    Feature = 0x03,
}

impl ReportType {
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0x01 => Some(ReportType::Input),
            0x02 => Some(ReportType::Output),
            0x03 => Some(ReportType::Feature),
            _ => None,
        }
    }
}

/// HID protocol mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ProtocolMode {
    /// Boot protocol (simplified, for BIOS)
    Boot = 0x00,
    /// Report protocol (full HID descriptor)
    Report = 0x01,
}

/// HID device subclass
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HidSubclass {
    None = 0x00,
    BootInterface = 0x01,
}

/// HID device class (major device class 0x05 = Peripheral)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HidDeviceType {
    Unknown,
    Keyboard,
    Mouse,
    ComboKeyboardMouse,
    Gamepad,
    Joystick,
    Digitizer,
    CardReader,
    RemoteControl,
}

impl HidDeviceType {
    /// Determine device type from device class
    pub fn from_device_class(class: &super::DeviceClass) -> Self {
        let major = class.major_class();
        let minor = class.minor_class();

        if major != 0x05 {
            // Not a peripheral
            return HidDeviceType::Unknown;
        }

        // Minor class bits for peripherals:
        // Bits 7-6: Device type
        //   00 = Not keyboard/pointing device
        //   01 = Keyboard
        //   10 = Pointing device
        //   11 = Combo keyboard/pointing device
        // Bits 5-2: Device subtype (if applicable)

        let type_bits = (minor >> 4) & 0x03;

        match type_bits {
            0x01 => HidDeviceType::Keyboard,
            0x02 => {
                // Pointing device subtype
                let subtype = minor & 0x0F;
                match subtype {
                    0x01 => HidDeviceType::Mouse,
                    0x02 => HidDeviceType::Joystick,
                    0x03 => HidDeviceType::Gamepad,
                    0x04 => HidDeviceType::RemoteControl,
                    0x05 => HidDeviceType::Digitizer,
                    0x06 => HidDeviceType::CardReader,
                    _ => HidDeviceType::Mouse, // Default pointing device
                }
            }
            0x03 => HidDeviceType::ComboKeyboardMouse,
            _ => HidDeviceType::Unknown,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            HidDeviceType::Unknown => "Unknown HID",
            HidDeviceType::Keyboard => "Keyboard",
            HidDeviceType::Mouse => "Mouse",
            HidDeviceType::ComboKeyboardMouse => "Keyboard+Mouse",
            HidDeviceType::Gamepad => "Gamepad",
            HidDeviceType::Joystick => "Joystick",
            HidDeviceType::Digitizer => "Digitizer",
            HidDeviceType::CardReader => "Card Reader",
            HidDeviceType::RemoteControl => "Remote Control",
        }
    }
}

/// HID Usage Page (top 16 bits of usage)
pub mod usage_page {
    pub const GENERIC_DESKTOP: u16 = 0x01;
    pub const SIMULATION: u16 = 0x02;
    pub const VR: u16 = 0x03;
    pub const SPORT: u16 = 0x04;
    pub const GAME: u16 = 0x05;
    pub const GENERIC_DEVICE: u16 = 0x06;
    pub const KEYBOARD: u16 = 0x07;
    pub const LED: u16 = 0x08;
    pub const BUTTON: u16 = 0x09;
    pub const ORDINAL: u16 = 0x0A;
    pub const TELEPHONY: u16 = 0x0B;
    pub const CONSUMER: u16 = 0x0C;
    pub const DIGITIZER: u16 = 0x0D;
}

/// Generic Desktop Usage IDs
pub mod generic_desktop {
    pub const POINTER: u16 = 0x01;
    pub const MOUSE: u16 = 0x02;
    pub const JOYSTICK: u16 = 0x04;
    pub const GAMEPAD: u16 = 0x05;
    pub const KEYBOARD: u16 = 0x06;
    pub const KEYPAD: u16 = 0x07;
    pub const MULTI_AXIS: u16 = 0x08;
    pub const X: u16 = 0x30;
    pub const Y: u16 = 0x31;
    pub const Z: u16 = 0x32;
    pub const RX: u16 = 0x33;
    pub const RY: u16 = 0x34;
    pub const RZ: u16 = 0x35;
    pub const WHEEL: u16 = 0x38;
    pub const HAT_SWITCH: u16 = 0x39;
}

/// Keyboard scan codes (USB HID)
pub mod keyboard_codes {
    pub const A: u8 = 0x04;
    pub const B: u8 = 0x05;
    pub const C: u8 = 0x06;
    pub const D: u8 = 0x07;
    pub const E: u8 = 0x08;
    pub const F: u8 = 0x09;
    pub const G: u8 = 0x0A;
    pub const H: u8 = 0x0B;
    pub const I: u8 = 0x0C;
    pub const J: u8 = 0x0D;
    pub const K: u8 = 0x0E;
    pub const L: u8 = 0x0F;
    pub const M: u8 = 0x10;
    pub const N: u8 = 0x11;
    pub const O: u8 = 0x12;
    pub const P: u8 = 0x13;
    pub const Q: u8 = 0x14;
    pub const R: u8 = 0x15;
    pub const S: u8 = 0x16;
    pub const T: u8 = 0x17;
    pub const U: u8 = 0x18;
    pub const V: u8 = 0x19;
    pub const W: u8 = 0x1A;
    pub const X: u8 = 0x1B;
    pub const Y: u8 = 0x1C;
    pub const Z: u8 = 0x1D;
    pub const NUM_1: u8 = 0x1E;
    pub const NUM_2: u8 = 0x1F;
    pub const NUM_3: u8 = 0x20;
    pub const NUM_4: u8 = 0x21;
    pub const NUM_5: u8 = 0x22;
    pub const NUM_6: u8 = 0x23;
    pub const NUM_7: u8 = 0x24;
    pub const NUM_8: u8 = 0x25;
    pub const NUM_9: u8 = 0x26;
    pub const NUM_0: u8 = 0x27;
    pub const ENTER: u8 = 0x28;
    pub const ESCAPE: u8 = 0x29;
    pub const BACKSPACE: u8 = 0x2A;
    pub const TAB: u8 = 0x2B;
    pub const SPACE: u8 = 0x2C;
    pub const MINUS: u8 = 0x2D;
    pub const EQUAL: u8 = 0x2E;
    pub const LEFT_BRACKET: u8 = 0x2F;
    pub const RIGHT_BRACKET: u8 = 0x30;
    pub const BACKSLASH: u8 = 0x31;
    pub const SEMICOLON: u8 = 0x33;
    pub const QUOTE: u8 = 0x34;
    pub const GRAVE: u8 = 0x35;
    pub const COMMA: u8 = 0x36;
    pub const PERIOD: u8 = 0x37;
    pub const SLASH: u8 = 0x38;
    pub const CAPS_LOCK: u8 = 0x39;
    pub const F1: u8 = 0x3A;
    pub const F2: u8 = 0x3B;
    pub const F3: u8 = 0x3C;
    pub const F4: u8 = 0x3D;
    pub const F5: u8 = 0x3E;
    pub const F6: u8 = 0x3F;
    pub const F7: u8 = 0x40;
    pub const F8: u8 = 0x41;
    pub const F9: u8 = 0x42;
    pub const F10: u8 = 0x43;
    pub const F11: u8 = 0x44;
    pub const F12: u8 = 0x45;
    pub const PRINT_SCREEN: u8 = 0x46;
    pub const SCROLL_LOCK: u8 = 0x47;
    pub const PAUSE: u8 = 0x48;
    pub const INSERT: u8 = 0x49;
    pub const HOME: u8 = 0x4A;
    pub const PAGE_UP: u8 = 0x4B;
    pub const DELETE: u8 = 0x4C;
    pub const END: u8 = 0x4D;
    pub const PAGE_DOWN: u8 = 0x4E;
    pub const RIGHT_ARROW: u8 = 0x4F;
    pub const LEFT_ARROW: u8 = 0x50;
    pub const DOWN_ARROW: u8 = 0x51;
    pub const UP_ARROW: u8 = 0x52;
    pub const NUM_LOCK: u8 = 0x53;
    pub const LEFT_CTRL: u8 = 0xE0;
    pub const LEFT_SHIFT: u8 = 0xE1;
    pub const LEFT_ALT: u8 = 0xE2;
    pub const LEFT_GUI: u8 = 0xE3;
    pub const RIGHT_CTRL: u8 = 0xE4;
    pub const RIGHT_SHIFT: u8 = 0xE5;
    pub const RIGHT_ALT: u8 = 0xE6;
    pub const RIGHT_GUI: u8 = 0xE7;
}

/// Keyboard modifier flags (boot protocol)
pub mod keyboard_modifiers {
    pub const LEFT_CTRL: u8 = 1 << 0;
    pub const LEFT_SHIFT: u8 = 1 << 1;
    pub const LEFT_ALT: u8 = 1 << 2;
    pub const LEFT_GUI: u8 = 1 << 3;
    pub const RIGHT_CTRL: u8 = 1 << 4;
    pub const RIGHT_SHIFT: u8 = 1 << 5;
    pub const RIGHT_ALT: u8 = 1 << 6;
    pub const RIGHT_GUI: u8 = 1 << 7;
}

/// Mouse button flags (boot protocol)
pub mod mouse_buttons {
    pub const LEFT: u8 = 1 << 0;
    pub const RIGHT: u8 = 1 << 1;
    pub const MIDDLE: u8 = 1 << 2;
    pub const BUTTON_4: u8 = 1 << 3;
    pub const BUTTON_5: u8 = 1 << 4;
}

/// Boot protocol keyboard report (8 bytes)
#[derive(Debug, Clone, Copy, Default)]
#[repr(C, packed)]
pub struct BootKeyboardReport {
    /// Modifier keys (ctrl, shift, alt, gui)
    pub modifiers: u8,
    /// Reserved byte
    pub reserved: u8,
    /// Key codes (up to 6 simultaneous keys)
    pub keys: [u8; 6],
}

impl BootKeyboardReport {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 8 {
            return None;
        }

        Some(Self {
            modifiers: data[0],
            reserved: data[1],
            keys: [data[2], data[3], data[4], data[5], data[6], data[7]],
        })
    }

    /// Check if a modifier is pressed
    pub fn is_modifier_pressed(&self, modifier: u8) -> bool {
        self.modifiers & modifier != 0
    }

    /// Check if a key is pressed (by scan code)
    pub fn is_key_pressed(&self, key: u8) -> bool {
        self.keys.contains(&key)
    }

    /// Get list of pressed keys
    pub fn pressed_keys(&self) -> Vec<u8> {
        self.keys.iter().filter(|&&k| k != 0).copied().collect()
    }
}

/// Boot protocol mouse report (3-4 bytes)
#[derive(Debug, Clone, Copy, Default)]
pub struct BootMouseReport {
    /// Button states
    pub buttons: u8,
    /// X movement (signed)
    pub x: i8,
    /// Y movement (signed)
    pub y: i8,
    /// Wheel movement (optional, signed)
    pub wheel: i8,
}

impl BootMouseReport {
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 3 {
            return None;
        }

        Some(Self {
            buttons: data[0],
            x: data[1] as i8,
            y: data[2] as i8,
            wheel: if data.len() > 3 { data[3] as i8 } else { 0 },
        })
    }

    /// Check if a button is pressed
    pub fn is_button_pressed(&self, button: u8) -> bool {
        self.buttons & button != 0
    }
}

/// HID connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HidConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Ready,
    Suspended,
    Error,
}

/// Bluetooth HID device
pub struct BluetoothHidDevice {
    /// Device address
    pub address: BdAddr,
    /// Device name
    pub name: Option<String>,
    /// Device type
    pub device_type: HidDeviceType,
    /// Connection state
    pub state: HidConnectionState,
    /// ACL connection handle
    pub acl_handle: Option<u16>,
    /// Control channel CID
    pub control_cid: Option<u16>,
    /// Interrupt channel CID
    pub interrupt_cid: Option<u16>,
    /// Current protocol mode
    pub protocol_mode: ProtocolMode,
    /// HID descriptor
    pub hid_descriptor: Vec<u8>,
    /// Last keyboard report
    pub last_keyboard_report: Option<BootKeyboardReport>,
    /// Last mouse report
    pub last_mouse_report: Option<BootMouseReport>,
    /// Report ID to type mapping
    pub report_ids: BTreeMap<u8, ReportType>,
}

impl BluetoothHidDevice {
    pub fn new(address: BdAddr) -> Self {
        Self {
            address,
            name: None,
            device_type: HidDeviceType::Unknown,
            state: HidConnectionState::Disconnected,
            acl_handle: None,
            control_cid: None,
            interrupt_cid: None,
            protocol_mode: ProtocolMode::Report,
            hid_descriptor: Vec::new(),
            last_keyboard_report: None,
            last_mouse_report: None,
            report_ids: BTreeMap::new(),
        }
    }

    /// Set device type from device class
    pub fn set_device_class(&mut self, class: &super::DeviceClass) {
        self.device_type = HidDeviceType::from_device_class(class);
    }
}

/// HID input event (forwarded to input subsystem)
#[derive(Debug, Clone)]
pub enum HidInputEvent {
    /// Key press/release
    KeyEvent {
        address: BdAddr,
        scan_code: u8,
        pressed: bool,
        modifiers: u8,
    },
    /// Mouse button press/release
    MouseButton {
        address: BdAddr,
        button: u8,
        pressed: bool,
    },
    /// Mouse movement
    MouseMove {
        address: BdAddr,
        dx: i8,
        dy: i8,
    },
    /// Mouse wheel
    MouseWheel {
        address: BdAddr,
        delta: i8,
    },
    /// Gamepad button
    GamepadButton {
        address: BdAddr,
        button: u8,
        pressed: bool,
    },
    /// Gamepad axis
    GamepadAxis {
        address: BdAddr,
        axis: u8,
        value: i16,
    },
}

/// Bluetooth HID manager
pub struct BluetoothHidManager {
    /// Connected HID devices
    devices: Vec<BluetoothHidDevice>,
    /// Event callbacks
    event_callbacks: Vec<fn(HidInputEvent)>,
    /// Auto-reconnect enabled
    auto_reconnect: bool,
    /// Known devices for auto-reconnect (addresses)
    known_devices: Vec<BdAddr>,
}

impl BluetoothHidManager {
    pub const fn new() -> Self {
        Self {
            devices: Vec::new(),
            event_callbacks: Vec::new(),
            auto_reconnect: true,
            known_devices: Vec::new(),
        }
    }

    /// Register event callback
    pub fn on_input(&mut self, callback: fn(HidInputEvent)) {
        self.event_callbacks.push(callback);
    }

    /// Fire input event to callbacks
    fn fire_event(&self, event: HidInputEvent) {
        for callback in &self.event_callbacks {
            callback(event.clone());
        }
    }

    /// Add a new HID device
    pub fn add_device(&mut self, address: BdAddr, device_type: HidDeviceType) {
        // Check if already exists
        if self.get_device(&address).is_some() {
            return;
        }

        let mut device = BluetoothHidDevice::new(address);
        device.device_type = device_type;
        self.devices.push(device);

        crate::kprintln!("bt_hid: added device {} ({})",
            address.to_string(),
            device_type.name());
    }

    /// Get device by address
    pub fn get_device(&self, address: &BdAddr) -> Option<&BluetoothHidDevice> {
        self.devices.iter().find(|d| d.address == *address)
    }

    /// Get device by address (mutable)
    pub fn get_device_mut(&mut self, address: &BdAddr) -> Option<&mut BluetoothHidDevice> {
        self.devices.iter_mut().find(|d| d.address == *address)
    }

    /// Remove device
    pub fn remove_device(&mut self, address: &BdAddr) {
        self.devices.retain(|d| d.address != *address);
    }

    /// Get all connected devices
    pub fn connected_devices(&self) -> Vec<&BluetoothHidDevice> {
        self.devices.iter()
            .filter(|d| d.state == HidConnectionState::Ready)
            .collect()
    }

    /// Handle L2CAP connection for HID
    pub fn handle_l2cap_connect(&mut self, address: &BdAddr, psm: u16, cid: u16) {
        if let Some(device) = self.get_device_mut(address) {
            match psm {
                psm::HID_CONTROL => {
                    device.control_cid = Some(cid);
                    crate::kprintln!("bt_hid: control channel connected (CID {})", cid);
                }
                psm::HID_INTERRUPT => {
                    device.interrupt_cid = Some(cid);
                    crate::kprintln!("bt_hid: interrupt channel connected (CID {})", cid);

                    // Both channels connected = ready
                    if device.control_cid.is_some() {
                        device.state = HidConnectionState::Ready;
                        crate::kprintln!("bt_hid: device {} ready", address.to_string());
                    }
                }
                _ => {}
            }
        }
    }

    /// Handle L2CAP disconnection
    pub fn handle_l2cap_disconnect(&mut self, address: &BdAddr, cid: u16) {
        if let Some(device) = self.get_device_mut(address) {
            if device.control_cid == Some(cid) {
                device.control_cid = None;
            }
            if device.interrupt_cid == Some(cid) {
                device.interrupt_cid = None;
            }

            // If either channel disconnected, device is no longer ready
            if device.control_cid.is_none() || device.interrupt_cid.is_none() {
                device.state = HidConnectionState::Disconnected;
            }
        }
    }

    /// Process HID data packet
    pub fn process_hid_data(&mut self, address: &BdAddr, data: &[u8]) {
        if data.is_empty() {
            return;
        }

        let header = data[0];
        let trans_type = (header >> 4) & 0x0F;
        let param = header & 0x0F;

        match trans_type {
            transaction::DATA | transaction::DATC => {
                // Input report
                let report_type = param;
                let report_data = &data[1..];

                self.process_input_report(address, report_type, report_data);
            }
            transaction::HANDSHAKE => {
                crate::kprintln!("bt_hid: handshake result {}", param);
            }
            _ => {
                crate::kprintln!("bt_hid: unhandled transaction type {:X}", trans_type);
            }
        }
    }

    /// Process HID input report
    fn process_input_report(&mut self, address: &BdAddr, report_type: u8, data: &[u8]) {
        let device_type = self.get_device(address)
            .map(|d| d.device_type)
            .unwrap_or(HidDeviceType::Unknown);

        let addr = *address;

        match device_type {
            HidDeviceType::Keyboard => {
                self.process_keyboard_report(&addr, data);
            }
            HidDeviceType::Mouse => {
                self.process_mouse_report(&addr, data);
            }
            HidDeviceType::ComboKeyboardMouse => {
                // Try to determine from report type/size
                if data.len() >= 8 {
                    self.process_keyboard_report(&addr, data);
                } else if data.len() >= 3 {
                    self.process_mouse_report(&addr, data);
                }
            }
            HidDeviceType::Gamepad | HidDeviceType::Joystick => {
                self.process_gamepad_report(&addr, data);
            }
            _ => {
                crate::kprintln!("bt_hid: unknown report type {} ({} bytes)",
                    report_type, data.len());
            }
        }
    }

    /// Process keyboard boot protocol report
    fn process_keyboard_report(&mut self, address: &BdAddr, data: &[u8]) {
        let report = match BootKeyboardReport::from_bytes(data) {
            Some(r) => r,
            None => return,
        };

        // Get previous report for comparison
        let prev_report = self.get_device(address)
            .and_then(|d| d.last_keyboard_report);

        let prev_keys = prev_report.map(|r| r.keys).unwrap_or([0; 6]);
        let prev_mods = prev_report.map(|r| r.modifiers).unwrap_or(0);

        // Check for newly pressed keys
        for &key in &report.keys {
            if key != 0 && !prev_keys.contains(&key) {
                self.fire_event(HidInputEvent::KeyEvent {
                    address: *address,
                    scan_code: key,
                    pressed: true,
                    modifiers: report.modifiers,
                });
            }
        }

        // Check for released keys
        for &key in &prev_keys {
            if key != 0 && !report.keys.contains(&key) {
                self.fire_event(HidInputEvent::KeyEvent {
                    address: *address,
                    scan_code: key,
                    pressed: false,
                    modifiers: report.modifiers,
                });
            }
        }

        // Check modifier changes
        let mod_changes = prev_mods ^ report.modifiers;
        if mod_changes != 0 {
            for i in 0..8 {
                let bit = 1 << i;
                if mod_changes & bit != 0 {
                    let modifier_key = match i {
                        0 => keyboard_codes::LEFT_CTRL,
                        1 => keyboard_codes::LEFT_SHIFT,
                        2 => keyboard_codes::LEFT_ALT,
                        3 => keyboard_codes::LEFT_GUI,
                        4 => keyboard_codes::RIGHT_CTRL,
                        5 => keyboard_codes::RIGHT_SHIFT,
                        6 => keyboard_codes::RIGHT_ALT,
                        7 => keyboard_codes::RIGHT_GUI,
                        _ => continue,
                    };

                    let pressed = report.modifiers & bit != 0;
                    self.fire_event(HidInputEvent::KeyEvent {
                        address: *address,
                        scan_code: modifier_key,
                        pressed,
                        modifiers: report.modifiers,
                    });
                }
            }
        }

        // Update stored report
        if let Some(device) = self.get_device_mut(address) {
            device.last_keyboard_report = Some(report);
        }
    }

    /// Process mouse boot protocol report
    fn process_mouse_report(&mut self, address: &BdAddr, data: &[u8]) {
        let report = match BootMouseReport::from_bytes(data) {
            Some(r) => r,
            None => return,
        };

        // Get previous report for button comparison
        let prev_buttons = self.get_device(address)
            .and_then(|d| d.last_mouse_report)
            .map(|r| r.buttons)
            .unwrap_or(0);

        // Check button changes
        let button_changes = prev_buttons ^ report.buttons;
        for i in 0..5 {
            let bit = 1 << i;
            if button_changes & bit != 0 {
                self.fire_event(HidInputEvent::MouseButton {
                    address: *address,
                    button: bit,
                    pressed: report.buttons & bit != 0,
                });
            }
        }

        // Report movement if any
        if report.x != 0 || report.y != 0 {
            self.fire_event(HidInputEvent::MouseMove {
                address: *address,
                dx: report.x,
                dy: report.y,
            });
        }

        // Report wheel if any
        if report.wheel != 0 {
            self.fire_event(HidInputEvent::MouseWheel {
                address: *address,
                delta: report.wheel,
            });
        }

        // Update stored report
        if let Some(device) = self.get_device_mut(address) {
            device.last_mouse_report = Some(report);
        }
    }

    /// Process gamepad report (basic implementation)
    fn process_gamepad_report(&mut self, address: &BdAddr, data: &[u8]) {
        // Gamepad reports vary widely, this is a simplified handler
        if data.is_empty() {
            return;
        }

        // Assume first byte is buttons
        let buttons = data[0];

        // Fire button events (simplified - 8 buttons)
        for i in 0..8 {
            let pressed = buttons & (1 << i) != 0;
            self.fire_event(HidInputEvent::GamepadButton {
                address: *address,
                button: i,
                pressed,
            });
        }

        // If more data, assume axes
        if data.len() >= 3 {
            // X axis
            self.fire_event(HidInputEvent::GamepadAxis {
                address: *address,
                axis: 0,
                value: (data[1] as i16) - 128,
            });
            // Y axis
            self.fire_event(HidInputEvent::GamepadAxis {
                address: *address,
                axis: 1,
                value: (data[2] as i16) - 128,
            });
        }
    }

    /// Build GET_REPORT request
    pub fn build_get_report(&self, report_type: ReportType, report_id: u8) -> Vec<u8> {
        let header = (transaction::GET_REPORT << 4) | (report_type as u8);
        if report_id != 0 {
            vec![header, report_id]
        } else {
            vec![header]
        }
    }

    /// Build SET_REPORT request
    pub fn build_set_report(&self, report_type: ReportType, report_id: u8, data: &[u8]) -> Vec<u8> {
        let header = (transaction::SET_REPORT << 4) | (report_type as u8);
        let mut pdu = Vec::with_capacity(2 + data.len());
        pdu.push(header);
        if report_id != 0 {
            pdu.push(report_id);
        }
        pdu.extend_from_slice(data);
        pdu
    }

    /// Build SET_PROTOCOL request
    pub fn build_set_protocol(&self, mode: ProtocolMode) -> Vec<u8> {
        let header = (transaction::SET_PROTOCOL << 4) | (mode as u8);
        vec![header]
    }

    /// Build HANDSHAKE response
    pub fn build_handshake(&self, result: u8) -> Vec<u8> {
        let header = (transaction::HANDSHAKE << 4) | (result & 0x0F);
        vec![header]
    }

    /// Build HID_CONTROL request
    pub fn build_hid_control(&self, param: u8) -> Vec<u8> {
        let header = (transaction::HID_CONTROL << 4) | (param & 0x0F);
        vec![header]
    }

    /// Set auto-reconnect
    pub fn set_auto_reconnect(&mut self, enabled: bool) {
        self.auto_reconnect = enabled;
    }

    /// Add known device for auto-reconnect
    pub fn add_known_device(&mut self, address: BdAddr) {
        if !self.known_devices.contains(&address) {
            self.known_devices.push(address);
        }
    }

    /// Remove known device
    pub fn remove_known_device(&mut self, address: &BdAddr) {
        self.known_devices.retain(|a| a != address);
    }

    /// Get device count
    pub fn device_count(&self) -> usize {
        self.devices.len()
    }

    /// Get connected device count
    pub fn connected_count(&self) -> usize {
        self.devices.iter()
            .filter(|d| d.state == HidConnectionState::Ready)
            .count()
    }
}

/// Global HID manager
pub static HID_MANAGER: TicketSpinlock<BluetoothHidManager> =
    TicketSpinlock::new(BluetoothHidManager::new());

// =============================================================================
// Public API
// =============================================================================

/// Initialize Bluetooth HID subsystem
pub fn init() {
    crate::kprintln!("bluetooth: HID profile initialized");
}

/// Add HID device
pub fn add_device(address: BdAddr, device_type: HidDeviceType) {
    HID_MANAGER.lock().add_device(address, device_type);
}

/// Remove HID device
pub fn remove_device(address: &BdAddr) {
    HID_MANAGER.lock().remove_device(address);
}

/// Get device count
pub fn device_count() -> usize {
    HID_MANAGER.lock().device_count()
}

/// Get connected device count
pub fn connected_count() -> usize {
    HID_MANAGER.lock().connected_count()
}

/// Register input callback
pub fn on_input(callback: fn(HidInputEvent)) {
    HID_MANAGER.lock().on_input(callback);
}

/// Handle L2CAP connection
pub fn handle_connect(address: &BdAddr, psm: u16, cid: u16) {
    HID_MANAGER.lock().handle_l2cap_connect(address, psm, cid);
}

/// Handle L2CAP disconnection
pub fn handle_disconnect(address: &BdAddr, cid: u16) {
    HID_MANAGER.lock().handle_l2cap_disconnect(address, cid);
}

/// Process HID data
pub fn process_data(address: &BdAddr, data: &[u8]) {
    HID_MANAGER.lock().process_hid_data(address, data);
}

/// Format status
pub fn format_status() -> String {
    use core::fmt::Write;
    let mut output = String::new();

    let manager = HID_MANAGER.lock();

    writeln!(output, "Bluetooth HID Devices: {}", manager.device_count()).ok();

    for device in &manager.devices {
        writeln!(output, "  {} - {} ({:?})",
            device.address.to_string(),
            device.device_type.name(),
            device.state
        ).ok();

        if let Some(name) = &device.name {
            writeln!(output, "    Name: {}", name).ok();
        }

        if let Some(cid) = device.control_cid {
            writeln!(output, "    Control CID: {}", cid).ok();
        }
        if let Some(cid) = device.interrupt_cid {
            writeln!(output, "    Interrupt CID: {}", cid).ok();
        }
    }

    output
}

/// Convert HID keyboard scan code to ASCII
pub fn scancode_to_ascii(code: u8, shift: bool) -> Option<char> {
    use keyboard_codes::*;

    let base = match code {
        A..=Z => (b'a' + (code - A)) as char,
        NUM_1..=NUM_9 => (b'1' + (code - NUM_1)) as char,
        NUM_0 => '0',
        ENTER => '\n',
        TAB => '\t',
        SPACE => ' ',
        MINUS => '-',
        EQUAL => '=',
        LEFT_BRACKET => '[',
        RIGHT_BRACKET => ']',
        BACKSLASH => '\\',
        SEMICOLON => ';',
        QUOTE => '\'',
        GRAVE => '`',
        COMMA => ',',
        PERIOD => '.',
        SLASH => '/',
        _ => return None,
    };

    if shift {
        Some(match base {
            'a'..='z' => base.to_ascii_uppercase(),
            '1' => '!',
            '2' => '@',
            '3' => '#',
            '4' => '$',
            '5' => '%',
            '6' => '^',
            '7' => '&',
            '8' => '*',
            '9' => '(',
            '0' => ')',
            '-' => '_',
            '=' => '+',
            '[' => '{',
            ']' => '}',
            '\\' => '|',
            ';' => ':',
            '\'' => '"',
            '`' => '~',
            ',' => '<',
            '.' => '>',
            '/' => '?',
            c => c,
        })
    } else {
        Some(base)
    }
}

/// Get scancode name
pub fn scancode_name(code: u8) -> &'static str {
    use keyboard_codes::*;

    match code {
        A => "A",
        B => "B",
        C => "C",
        D => "D",
        E => "E",
        F => "F",
        G => "G",
        H => "H",
        I => "I",
        J => "J",
        K => "K",
        L => "L",
        M => "M",
        N => "N",
        O => "O",
        P => "P",
        Q => "Q",
        R => "R",
        S => "S",
        T => "T",
        U => "U",
        V => "V",
        W => "W",
        X => "X",
        Y => "Y",
        Z => "Z",
        NUM_1 => "1",
        NUM_2 => "2",
        NUM_3 => "3",
        NUM_4 => "4",
        NUM_5 => "5",
        NUM_6 => "6",
        NUM_7 => "7",
        NUM_8 => "8",
        NUM_9 => "9",
        NUM_0 => "0",
        ENTER => "Enter",
        ESCAPE => "Escape",
        BACKSPACE => "Backspace",
        TAB => "Tab",
        SPACE => "Space",
        F1 => "F1",
        F2 => "F2",
        F3 => "F3",
        F4 => "F4",
        F5 => "F5",
        F6 => "F6",
        F7 => "F7",
        F8 => "F8",
        F9 => "F9",
        F10 => "F10",
        F11 => "F11",
        F12 => "F12",
        INSERT => "Insert",
        HOME => "Home",
        PAGE_UP => "PageUp",
        DELETE => "Delete",
        END => "End",
        PAGE_DOWN => "PageDown",
        RIGHT_ARROW => "Right",
        LEFT_ARROW => "Left",
        DOWN_ARROW => "Down",
        UP_ARROW => "Up",
        LEFT_CTRL => "LCtrl",
        LEFT_SHIFT => "LShift",
        LEFT_ALT => "LAlt",
        LEFT_GUI => "LWin",
        RIGHT_CTRL => "RCtrl",
        RIGHT_SHIFT => "RShift",
        RIGHT_ALT => "RAlt",
        RIGHT_GUI => "RWin",
        CAPS_LOCK => "CapsLock",
        NUM_LOCK => "NumLock",
        SCROLL_LOCK => "ScrollLock",
        PRINT_SCREEN => "PrintScreen",
        PAUSE => "Pause",
        _ => "Unknown",
    }
}
