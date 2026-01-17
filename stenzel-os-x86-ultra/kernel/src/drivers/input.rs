//! Input Event System (/dev/input/eventN)
//!
//! Provides a Linux-like input event interface for keyboards, mice, and other input devices.

use alloc::collections::VecDeque;
use alloc::vec::Vec;
use spin::Mutex;

/// Input event type (matches Linux input.h)
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventType {
    /// Synchronization event
    Syn = 0x00,
    /// Key/button event
    Key = 0x01,
    /// Relative movement (mouse)
    Rel = 0x02,
    /// Absolute position (touchscreen, tablet)
    Abs = 0x03,
    /// Miscellaneous
    Msc = 0x04,
    /// LED state
    Led = 0x05,
    /// Sound
    Snd = 0x06,
    /// Force feedback
    Rep = 0x14,
    /// Force feedback status
    Ff = 0x15,
}

/// Synchronization codes
#[repr(u16)]
#[derive(Debug, Clone, Copy)]
pub enum SynCode {
    Report = 0,
    Config = 1,
    MtReport = 2,
    Dropped = 3,
}

/// Key codes (partial list matching Linux)
#[repr(u16)]
#[derive(Debug, Clone, Copy)]
pub enum KeyCode {
    Reserved = 0,
    Esc = 1,
    Key1 = 2,
    Key2 = 3,
    Key3 = 4,
    Key4 = 5,
    Key5 = 6,
    Key6 = 7,
    Key7 = 8,
    Key8 = 9,
    Key9 = 10,
    Key0 = 11,
    Minus = 12,
    Equal = 13,
    Backspace = 14,
    Tab = 15,
    Q = 16,
    W = 17,
    E = 18,
    R = 19,
    T = 20,
    Y = 21,
    U = 22,
    I = 23,
    O = 24,
    P = 25,
    LeftBrace = 26,
    RightBrace = 27,
    Enter = 28,
    LeftCtrl = 29,
    A = 30,
    S = 31,
    D = 32,
    F = 33,
    G = 34,
    H = 35,
    J = 36,
    K = 37,
    L = 38,
    Semicolon = 39,
    Apostrophe = 40,
    Grave = 41,
    LeftShift = 42,
    Backslash = 43,
    Z = 44,
    X = 45,
    C = 46,
    V = 47,
    B = 48,
    N = 49,
    M = 50,
    Comma = 51,
    Dot = 52,
    Slash = 53,
    RightShift = 54,
    KpAsterisk = 55,
    LeftAlt = 56,
    Space = 57,
    CapsLock = 58,
    F1 = 59,
    F2 = 60,
    F3 = 61,
    F4 = 62,
    F5 = 63,
    F6 = 64,
    F7 = 65,
    F8 = 66,
    F9 = 67,
    F10 = 68,
    // Mouse buttons (BTN_MOUSE = 0x110)
    BtnLeft = 0x110,
    BtnRight = 0x111,
    BtnMiddle = 0x112,
}

/// Relative axis codes (mouse movement)
#[repr(u16)]
#[derive(Debug, Clone, Copy)]
pub enum RelCode {
    X = 0x00,
    Y = 0x01,
    Z = 0x02,
    Wheel = 0x08,
    HWheel = 0x06,
}

/// Input event structure (matches Linux struct input_event)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct InputEvent {
    /// Timestamp seconds
    pub sec: i64,
    /// Timestamp microseconds
    pub usec: i64,
    /// Event type
    pub event_type: u16,
    /// Event code (key code, axis, etc.)
    pub code: u16,
    /// Event value (1=press, 0=release, relative delta, etc.)
    pub value: i32,
}

impl InputEvent {
    pub fn new(event_type: u16, code: u16, value: i32) -> Self {
        let time = crate::time::realtime();
        Self {
            sec: time.tv_sec,
            usec: time.tv_nsec / 1000,
            event_type,
            code,
            value,
        }
    }

    /// Create a key press/release event
    pub fn key(code: u16, pressed: bool) -> Self {
        Self::new(EventType::Key as u16, code, if pressed { 1 } else { 0 })
    }

    /// Create a relative movement event
    pub fn rel(axis: RelCode, delta: i32) -> Self {
        Self::new(EventType::Rel as u16, axis as u16, delta)
    }

    /// Create a synchronization event (marks end of event batch)
    pub fn syn() -> Self {
        Self::new(EventType::Syn as u16, SynCode::Report as u16, 0)
    }

    /// Get size of event in bytes
    pub const fn size() -> usize {
        core::mem::size_of::<InputEvent>()
    }

    /// Convert to bytes
    pub fn to_bytes(&self) -> [u8; 24] {
        let mut bytes = [0u8; 24];
        bytes[0..8].copy_from_slice(&self.sec.to_ne_bytes());
        bytes[8..16].copy_from_slice(&self.usec.to_ne_bytes());
        bytes[16..18].copy_from_slice(&self.event_type.to_ne_bytes());
        bytes[18..20].copy_from_slice(&self.code.to_ne_bytes());
        bytes[20..24].copy_from_slice(&self.value.to_ne_bytes());
        bytes
    }
}

/// Input event device
pub struct InputDevice {
    /// Device name (e.g., "USB Keyboard", "USB Mouse")
    pub name: &'static str,
    /// Device type (keyboard, mouse, etc.)
    pub device_type: InputDeviceType,
    /// Event queue
    events: Mutex<VecDeque<InputEvent>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputDeviceType {
    Keyboard,
    Mouse,
    Touchpad,
    Tablet,
}

impl InputDevice {
    pub const fn new(name: &'static str, device_type: InputDeviceType) -> Self {
        Self {
            name,
            device_type,
            events: Mutex::new(VecDeque::new()),
        }
    }

    /// Queue an event
    pub fn queue_event(&self, event: InputEvent) {
        let mut events = self.events.lock();
        if events.len() < 256 {
            events.push_back(event);
        }
    }

    /// Queue multiple events followed by a SYN
    pub fn queue_events(&self, events_list: &[InputEvent]) {
        let mut events = self.events.lock();
        for event in events_list {
            if events.len() < 256 {
                events.push_back(*event);
            }
        }
        // Add sync event
        if events.len() < 256 {
            events.push_back(InputEvent::syn());
        }
    }

    /// Read events (returns number of bytes read)
    pub fn read(&self, buf: &mut [u8]) -> usize {
        let event_size = InputEvent::size();
        let max_events = buf.len() / event_size;
        let mut bytes_read = 0;
        let mut events = self.events.lock();

        for _ in 0..max_events {
            if let Some(event) = events.pop_front() {
                let event_bytes = event.to_bytes();
                buf[bytes_read..bytes_read + event_size].copy_from_slice(&event_bytes);
                bytes_read += event_size;
            } else {
                break;
            }
        }

        bytes_read
    }

    /// Check if there are events available
    pub fn has_events(&self) -> bool {
        !self.events.lock().is_empty()
    }
}

/// Global input device registry
static INPUT_DEVICES: Mutex<Vec<&'static InputDevice>> = Mutex::new(Vec::new());

/// Default keyboard device
pub static KEYBOARD_DEVICE: InputDevice = InputDevice::new("System Keyboard", InputDeviceType::Keyboard);

/// Default mouse device
pub static MOUSE_DEVICE: InputDevice = InputDevice::new("System Mouse", InputDeviceType::Mouse);

/// Initialize input subsystem
pub fn init() {
    let mut devices = INPUT_DEVICES.lock();
    devices.push(&KEYBOARD_DEVICE);
    devices.push(&MOUSE_DEVICE);
    crate::kprintln!("input: registered {} input devices", devices.len());
}

/// Get input device by index
pub fn get_device(index: usize) -> Option<&'static InputDevice> {
    let devices = INPUT_DEVICES.lock();
    devices.get(index).copied()
}

/// Get number of input devices
pub fn device_count() -> usize {
    INPUT_DEVICES.lock().len()
}

/// Convert PS/2 scancode to Linux key code
pub fn scancode_to_keycode(scancode: u8) -> u16 {
    // Direct mapping for most common keys (PS/2 set 1 -> Linux keycodes)
    // PS/2 scancodes match Linux keycodes for basic keys
    match scancode {
        0x01 => KeyCode::Esc as u16,
        0x02 => KeyCode::Key1 as u16,
        0x03 => KeyCode::Key2 as u16,
        0x04 => KeyCode::Key3 as u16,
        0x05 => KeyCode::Key4 as u16,
        0x06 => KeyCode::Key5 as u16,
        0x07 => KeyCode::Key6 as u16,
        0x08 => KeyCode::Key7 as u16,
        0x09 => KeyCode::Key8 as u16,
        0x0A => KeyCode::Key9 as u16,
        0x0B => KeyCode::Key0 as u16,
        0x0C => KeyCode::Minus as u16,
        0x0D => KeyCode::Equal as u16,
        0x0E => KeyCode::Backspace as u16,
        0x0F => KeyCode::Tab as u16,
        0x10 => KeyCode::Q as u16,
        0x11 => KeyCode::W as u16,
        0x12 => KeyCode::E as u16,
        0x13 => KeyCode::R as u16,
        0x14 => KeyCode::T as u16,
        0x15 => KeyCode::Y as u16,
        0x16 => KeyCode::U as u16,
        0x17 => KeyCode::I as u16,
        0x18 => KeyCode::O as u16,
        0x19 => KeyCode::P as u16,
        0x1A => KeyCode::LeftBrace as u16,
        0x1B => KeyCode::RightBrace as u16,
        0x1C => KeyCode::Enter as u16,
        0x1D => KeyCode::LeftCtrl as u16,
        0x1E => KeyCode::A as u16,
        0x1F => KeyCode::S as u16,
        0x20 => KeyCode::D as u16,
        0x21 => KeyCode::F as u16,
        0x22 => KeyCode::G as u16,
        0x23 => KeyCode::H as u16,
        0x24 => KeyCode::J as u16,
        0x25 => KeyCode::K as u16,
        0x26 => KeyCode::L as u16,
        0x27 => KeyCode::Semicolon as u16,
        0x28 => KeyCode::Apostrophe as u16,
        0x29 => KeyCode::Grave as u16,
        0x2A => KeyCode::LeftShift as u16,
        0x2B => KeyCode::Backslash as u16,
        0x2C => KeyCode::Z as u16,
        0x2D => KeyCode::X as u16,
        0x2E => KeyCode::C as u16,
        0x2F => KeyCode::V as u16,
        0x30 => KeyCode::B as u16,
        0x31 => KeyCode::N as u16,
        0x32 => KeyCode::M as u16,
        0x33 => KeyCode::Comma as u16,
        0x34 => KeyCode::Dot as u16,
        0x35 => KeyCode::Slash as u16,
        0x36 => KeyCode::RightShift as u16,
        0x37 => KeyCode::KpAsterisk as u16,
        0x38 => KeyCode::LeftAlt as u16,
        0x39 => KeyCode::Space as u16,
        0x3A => KeyCode::CapsLock as u16,
        0x3B => KeyCode::F1 as u16,
        0x3C => KeyCode::F2 as u16,
        0x3D => KeyCode::F3 as u16,
        0x3E => KeyCode::F4 as u16,
        0x3F => KeyCode::F5 as u16,
        0x40 => KeyCode::F6 as u16,
        0x41 => KeyCode::F7 as u16,
        0x42 => KeyCode::F8 as u16,
        0x43 => KeyCode::F9 as u16,
        0x44 => KeyCode::F10 as u16,
        _ => scancode as u16, // Pass through unknown codes
    }
}

/// Report a key press/release to the input system
pub fn report_key(scancode: u8, pressed: bool) {
    let keycode = scancode_to_keycode(scancode);
    let event = InputEvent::key(keycode, pressed);
    KEYBOARD_DEVICE.queue_event(event);
    KEYBOARD_DEVICE.queue_event(InputEvent::syn());
}

/// Report mouse movement to the input system
pub fn report_mouse_move(dx: i32, dy: i32) {
    if dx != 0 {
        MOUSE_DEVICE.queue_event(InputEvent::rel(RelCode::X, dx));
    }
    if dy != 0 {
        MOUSE_DEVICE.queue_event(InputEvent::rel(RelCode::Y, dy));
    }
    if dx != 0 || dy != 0 {
        MOUSE_DEVICE.queue_event(InputEvent::syn());
    }
}

/// Report mouse button press/release
pub fn report_mouse_button(button: u16, pressed: bool) {
    MOUSE_DEVICE.queue_event(InputEvent::key(button, pressed));
    MOUSE_DEVICE.queue_event(InputEvent::syn());
}

/// Report mouse wheel scroll
pub fn report_mouse_wheel(delta: i32) {
    if delta != 0 {
        MOUSE_DEVICE.queue_event(InputEvent::rel(RelCode::Wheel, delta));
        MOUSE_DEVICE.queue_event(InputEvent::syn());
    }
}
