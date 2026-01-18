//! Touchpad Driver
//!
//! Supports various touchpad protocols:
//! - Synaptics PS/2 (most common on laptops)
//! - ALPS PS/2
//! - Generic PS/2 (relative mode fallback)
//!
//! Features:
//! - Single-finger tracking
//! - Two-finger gestures (scroll, tap for right-click)
//! - Three-finger gestures (tap for middle-click)
//! - Palm rejection
//! - Edge scrolling
//! - Tap-to-click

use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use spin::Mutex;
use x86_64::instructions::port::{PortReadOnly, PortWriteOnly};

use crate::sync::IrqSafeMutex;
use crate::util::{KResult, KError};

/// Touchpad event types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchpadEventType {
    /// Finger moved
    Move,
    /// Finger pressed (tap or physical click)
    Press,
    /// Finger released
    Release,
    /// Scroll (two-finger or edge scroll)
    Scroll,
    /// Gesture (three-finger swipe, etc.)
    Gesture,
}

/// Touchpad event
#[derive(Debug, Clone, Copy)]
pub struct TouchpadEvent {
    /// Event type
    pub event_type: TouchpadEventType,
    /// X position (absolute, 0-65535 scaled)
    pub x: i32,
    /// Y position (absolute, 0-65535 scaled)
    pub y: i32,
    /// X delta (for relative movement)
    pub dx: i32,
    /// Y delta (for relative movement)
    pub dy: i32,
    /// Pressure (0-255)
    pub pressure: u8,
    /// Number of fingers
    pub fingers: u8,
    /// Button state
    pub buttons: ButtonState,
    /// Timestamp
    pub timestamp: u64,
}

/// Button state
#[derive(Debug, Clone, Copy, Default)]
pub struct ButtonState {
    pub left: bool,
    pub right: bool,
    pub middle: bool,
}

/// Touchpad protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchpadProtocol {
    /// Generic PS/2 mouse (relative mode only)
    GenericPs2,
    /// Synaptics absolute mode
    Synaptics,
    /// ALPS absolute mode
    Alps,
    /// Elantech
    Elantech,
    /// I2C HID
    I2cHid,
}

/// Touchpad capabilities
#[derive(Debug, Clone, Copy)]
pub struct TouchpadCapabilities {
    /// Protocol
    pub protocol: TouchpadProtocol,
    /// Supports absolute positioning
    pub absolute: bool,
    /// Maximum X coordinate
    pub max_x: u32,
    /// Maximum Y coordinate
    pub max_y: u32,
    /// Number of supported fingers
    pub max_fingers: u8,
    /// Has physical buttons
    pub has_buttons: bool,
    /// Supports pressure sensing
    pub has_pressure: bool,
    /// Supports palm detection
    pub has_palm_detect: bool,
    /// Resolution (units per mm)
    pub resolution_x: u32,
    pub resolution_y: u32,
}

impl Default for TouchpadCapabilities {
    fn default() -> Self {
        Self {
            protocol: TouchpadProtocol::GenericPs2,
            absolute: false,
            max_x: 6143,
            max_y: 6143,
            max_fingers: 2,
            has_buttons: true,
            has_pressure: false,
            has_palm_detect: false,
            resolution_x: 85,
            resolution_y: 94,
        }
    }
}

/// Touchpad configuration
#[derive(Debug, Clone, Copy)]
pub struct TouchpadConfig {
    /// Enable tap-to-click
    pub tap_to_click: bool,
    /// Two-finger tap for right-click
    pub two_finger_tap: bool,
    /// Three-finger tap for middle-click
    pub three_finger_tap: bool,
    /// Enable two-finger scrolling
    pub two_finger_scroll: bool,
    /// Enable edge scrolling
    pub edge_scroll: bool,
    /// Sensitivity (1-10)
    pub sensitivity: u8,
    /// Pointer speed (1-10)
    pub speed: u8,
    /// Palm rejection threshold
    pub palm_threshold: u8,
    /// Tap time in ms
    pub tap_time: u32,
    /// Minimum tap distance (to reject accidental taps)
    pub tap_distance: u32,
    /// Scroll speed
    pub scroll_speed: u8,
    /// Natural scrolling (reverse direction)
    pub natural_scrolling: bool,
    /// Disable while typing
    pub disable_while_typing: bool,
}

impl Default for TouchpadConfig {
    fn default() -> Self {
        Self {
            tap_to_click: true,
            two_finger_tap: true,
            three_finger_tap: true,
            two_finger_scroll: true,
            edge_scroll: true,
            sensitivity: 5,
            speed: 5,
            palm_threshold: 200,
            tap_time: 180,
            tap_distance: 50,
            scroll_speed: 5,
            natural_scrolling: false,
            disable_while_typing: true,
        }
    }
}

/// Finger tracking state
#[derive(Debug, Clone, Copy, Default)]
struct FingerState {
    /// Finger is touching
    active: bool,
    /// Current X position
    x: i32,
    /// Current Y position
    y: i32,
    /// Starting X (for gesture detection)
    start_x: i32,
    /// Starting Y
    start_y: i32,
    /// Pressure
    pressure: u8,
    /// When finger touched
    touch_time: u64,
}

/// Touchpad state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TouchpadState {
    /// No fingers touching
    Idle,
    /// One finger touching
    OneFinger,
    /// Two fingers touching
    TwoFinger,
    /// Three or more fingers
    MultiFinger,
    /// Tap detected, waiting to see if it's a double-tap
    TapWait,
    /// Dragging (tap-and-hold)
    Dragging,
    /// Scrolling
    Scrolling,
}

/// Touchpad driver
struct TouchpadDriver {
    /// Protocol in use
    protocol: TouchpadProtocol,
    /// Capabilities
    caps: TouchpadCapabilities,
    /// Configuration
    config: TouchpadConfig,
    /// Current state
    state: TouchpadState,
    /// Finger tracking (up to 5 fingers)
    fingers: [FingerState; 5],
    /// Number of active fingers
    num_fingers: u8,
    /// Previous finger count (for detecting changes)
    prev_fingers: u8,
    /// Accumulated scroll
    scroll_x: i32,
    scroll_y: i32,
    /// Last event time
    last_event_time: u64,
    /// Last tap time (for double-tap detection)
    last_tap_time: u64,
    /// Last position
    last_x: i32,
    last_y: i32,
    /// Physical button state
    buttons: ButtonState,
    /// Packet buffer for PS/2
    packet_buf: [u8; 6],
    packet_idx: usize,
    /// Is initialized
    initialized: bool,
}

impl TouchpadDriver {
    const fn new() -> Self {
        Self {
            protocol: TouchpadProtocol::GenericPs2,
            caps: TouchpadCapabilities {
                protocol: TouchpadProtocol::GenericPs2,
                absolute: false,
                max_x: 6143,
                max_y: 6143,
                max_fingers: 2,
                has_buttons: true,
                has_pressure: false,
                has_palm_detect: false,
                resolution_x: 85,
                resolution_y: 94,
            },
            config: TouchpadConfig {
                tap_to_click: true,
                two_finger_tap: true,
                three_finger_tap: true,
                two_finger_scroll: true,
                edge_scroll: true,
                sensitivity: 5,
                speed: 5,
                palm_threshold: 200,
                tap_time: 180,
                tap_distance: 50,
                scroll_speed: 5,
                natural_scrolling: false,
                disable_while_typing: true,
            },
            state: TouchpadState::Idle,
            fingers: [FingerState {
                active: false,
                x: 0,
                y: 0,
                start_x: 0,
                start_y: 0,
                pressure: 0,
                touch_time: 0,
            }; 5],
            num_fingers: 0,
            prev_fingers: 0,
            scroll_x: 0,
            scroll_y: 0,
            last_event_time: 0,
            last_tap_time: 0,
            last_x: 0,
            last_y: 0,
            buttons: ButtonState { left: false, right: false, middle: false },
            packet_buf: [0; 6],
            packet_idx: 0,
            initialized: false,
        }
    }
}

/// Global touchpad driver
static TOUCHPAD: IrqSafeMutex<TouchpadDriver> = IrqSafeMutex::new(TouchpadDriver::new());

/// Event buffer
static EVENT_BUFFER: Mutex<VecDeque<TouchpadEvent>> = Mutex::new(VecDeque::new());

/// Maximum events in buffer
const MAX_EVENTS: usize = 64;

/// Whether touchpad is enabled
static ENABLED: AtomicBool = AtomicBool::new(true);

/// Last keyboard activity time (for disable-while-typing)
static LAST_KEYBOARD_TIME: AtomicU32 = AtomicU32::new(0);

// i8042 constants
const I8042_DATA_PORT: u16 = 0x60;
const I8042_STATUS_PORT: u16 = 0x64;
const I8042_COMMAND_PORT: u16 = 0x64;

// Synaptics constants
const SYNAPTICS_QUERY_IDENTIFY: u8 = 0x00;
const SYNAPTICS_QUERY_MODES: u8 = 0x01;
const SYNAPTICS_QUERY_CAPABILITIES: u8 = 0x02;
const SYNAPTICS_QUERY_MODEL: u8 = 0x03;
const SYNAPTICS_QUERY_RESOLUTION: u8 = 0x08;
const SYNAPTICS_QUERY_EXT_MODEL: u8 = 0x09;

// Synaptics mode bits
const SYNAPTICS_MODE_ABSOLUTE: u8 = 0x80;
const SYNAPTICS_MODE_HIGH_RATE: u8 = 0x40;
const SYNAPTICS_MODE_SLEEP: u8 = 0x08;
const SYNAPTICS_MODE_EXT_W: u8 = 0x04;
const SYNAPTICS_MODE_TRANSPARENT: u8 = 0x02;
const SYNAPTICS_MODE_WMODE: u8 = 0x01;

/// Wait for i8042 read ready
fn wait_read() {
    let mut status: PortReadOnly<u8> = PortReadOnly::new(I8042_STATUS_PORT);
    for _ in 0..100_000 {
        if unsafe { status.read() } & 0x01 != 0 {
            return;
        }
        core::hint::spin_loop();
    }
}

/// Wait for i8042 write ready
fn wait_write() {
    let mut status: PortReadOnly<u8> = PortReadOnly::new(I8042_STATUS_PORT);
    for _ in 0..100_000 {
        if unsafe { status.read() } & 0x02 == 0 {
            return;
        }
        core::hint::spin_loop();
    }
}

/// Send command to i8042 controller
fn send_controller_cmd(cmd: u8) {
    wait_write();
    let mut port: PortWriteOnly<u8> = PortWriteOnly::new(I8042_COMMAND_PORT);
    unsafe { port.write(cmd) };
}

/// Send command to mouse/touchpad via i8042
fn send_mouse_cmd(cmd: u8) {
    send_controller_cmd(0xD4); // Write to auxiliary device
    wait_write();
    let mut port: PortWriteOnly<u8> = PortWriteOnly::new(I8042_DATA_PORT);
    unsafe { port.write(cmd) };
}

/// Read data from i8042
fn read_data() -> u8 {
    wait_read();
    let mut port: PortReadOnly<u8> = PortReadOnly::new(I8042_DATA_PORT);
    unsafe { port.read() }
}

/// Read data with timeout
fn read_data_timeout(timeout_us: u32) -> Option<u8> {
    let mut status: PortReadOnly<u8> = PortReadOnly::new(I8042_STATUS_PORT);
    for _ in 0..timeout_us {
        if unsafe { status.read() } & 0x01 != 0 {
            let mut port: PortReadOnly<u8> = PortReadOnly::new(I8042_DATA_PORT);
            return Some(unsafe { port.read() });
        }
        core::hint::spin_loop();
    }
    None
}

/// Wait for ACK from device
fn wait_ack() -> bool {
    for _ in 0..10 {
        if let Some(data) = read_data_timeout(10000) {
            if data == 0xFA {
                return true;
            }
        }
    }
    false
}

/// Send Synaptics special sequence for queries/modes
fn synaptics_special_seq(arg: u8) -> bool {
    // Send 0xE8 0x00 four times, then 0xE8 with bits
    for i in 0..4 {
        send_mouse_cmd(0xE8); // Set Resolution
        if !wait_ack() { return false; }
        let nibble = (arg >> (6 - i * 2)) & 0x03;
        send_mouse_cmd(nibble);
        if !wait_ack() { return false; }
    }
    true
}

/// Query Synaptics info
fn synaptics_query(query: u8) -> Option<[u8; 3]> {
    if !synaptics_special_seq(query) {
        return None;
    }

    // Send status request
    send_mouse_cmd(0xE9); // Status Request
    if !wait_ack() { return None; }

    let b1 = read_data_timeout(10000)?;
    let b2 = read_data_timeout(10000)?;
    let b3 = read_data_timeout(10000)?;

    Some([b1, b2, b3])
}

/// Set Synaptics mode
fn synaptics_set_mode(mode: u8) -> bool {
    if !synaptics_special_seq(mode) {
        return false;
    }

    // Send set sample rate with rate 0x14 (20)
    send_mouse_cmd(0xF3); // Set Sample Rate
    if !wait_ack() { return false; }
    send_mouse_cmd(0x14);
    wait_ack()
}

/// Detect touchpad type
fn detect_touchpad() -> TouchpadProtocol {
    // Try Synaptics identification
    if let Some(info) = synaptics_query(SYNAPTICS_QUERY_IDENTIFY) {
        // Check for Synaptics magic: middle byte should have bit 2 set
        // and info[0] should be model byte
        if (info[1] & 0x47) == 0x47 {
            crate::kprintln!("touchpad: Synaptics detected (model: {:#x})", info[0]);
            return TouchpadProtocol::Synaptics;
        }
    }

    // Try ALPS detection
    // ALPS uses E7 sequence for identification
    send_mouse_cmd(0xE7); // Set Scaling 2:1
    if wait_ack() {
        send_mouse_cmd(0xE7);
        if wait_ack() {
            send_mouse_cmd(0xE7);
            if wait_ack() {
                send_mouse_cmd(0xE9); // Status Request
                if wait_ack() {
                    let b1 = read_data_timeout(10000).unwrap_or(0);
                    let b2 = read_data_timeout(10000).unwrap_or(0);
                    let b3 = read_data_timeout(10000).unwrap_or(0);

                    // Check for ALPS signatures
                    if b1 == 0x33 || b1 == 0x63 || b1 == 0x73 {
                        crate::kprintln!("touchpad: ALPS detected (signature: {:#x})", b1);
                        return TouchpadProtocol::Alps;
                    }
                }
            }
        }
    }

    crate::kprintln!("touchpad: Using generic PS/2 protocol");
    TouchpadProtocol::GenericPs2
}

/// Initialize Synaptics touchpad
fn init_synaptics(driver: &mut TouchpadDriver) -> bool {
    // Query capabilities
    if let Some(caps) = synaptics_query(SYNAPTICS_QUERY_CAPABILITIES) {
        let cap_word = ((caps[0] as u32) << 16) | ((caps[1] as u32) << 8) | (caps[2] as u32);

        driver.caps.max_fingers = if cap_word & (1 << 1) != 0 { 2 } else { 1 };
        driver.caps.has_palm_detect = cap_word & (1 << 0) != 0;
        driver.caps.has_pressure = true;

        crate::kprintln!("touchpad: Synaptics caps: {:#x}", cap_word);
    }

    // Query resolution
    if let Some(res) = synaptics_query(SYNAPTICS_QUERY_RESOLUTION) {
        driver.caps.resolution_x = res[0] as u32;
        driver.caps.resolution_y = res[2] as u32;
    }

    // Set absolute mode with high rate
    let mode = SYNAPTICS_MODE_ABSOLUTE | SYNAPTICS_MODE_HIGH_RATE | SYNAPTICS_MODE_WMODE;
    if !synaptics_set_mode(mode) {
        crate::kprintln!("touchpad: Failed to set Synaptics mode");
        return false;
    }

    driver.caps.absolute = true;
    driver.caps.max_x = 6143;
    driver.caps.max_y = 6143;
    driver.caps.protocol = TouchpadProtocol::Synaptics;

    // Enable data reporting
    send_mouse_cmd(0xF4); // Enable
    wait_ack();

    crate::kprintln!("touchpad: Synaptics initialized in absolute mode");
    true
}

/// Initialize generic PS/2 (relative mode)
fn init_generic_ps2(driver: &mut TouchpadDriver) -> bool {
    // Reset
    send_mouse_cmd(0xFF);
    wait_ack();
    let _ = read_data_timeout(10000); // Self-test result
    let _ = read_data_timeout(10000); // Device ID

    // Set defaults
    send_mouse_cmd(0xF6);
    wait_ack();

    // Set sample rate 100
    send_mouse_cmd(0xF3);
    wait_ack();
    send_mouse_cmd(100);
    wait_ack();

    // Enable
    send_mouse_cmd(0xF4);
    wait_ack();

    driver.caps.absolute = false;
    driver.caps.max_fingers = 1;
    driver.caps.has_pressure = false;
    driver.caps.protocol = TouchpadProtocol::GenericPs2;

    crate::kprintln!("touchpad: Generic PS/2 initialized in relative mode");
    true
}

/// Initialize touchpad
pub fn init() {
    crate::kprintln!("touchpad: initializing");

    let mut driver = TOUCHPAD.lock();

    // Detect touchpad type
    let protocol = detect_touchpad();
    driver.protocol = protocol;

    // Initialize based on protocol
    let success = match protocol {
        TouchpadProtocol::Synaptics => init_synaptics(&mut driver),
        TouchpadProtocol::Alps => {
            // ALPS has complex initialization, fall back to generic
            init_generic_ps2(&mut driver)
        }
        _ => init_generic_ps2(&mut driver),
    };

    if success {
        driver.initialized = true;
        crate::kprintln!("touchpad: initialized successfully");
    } else {
        crate::kprintln!("touchpad: initialization failed");
    }
}

/// Process a byte from PS/2 IRQ
pub fn process_byte(byte: u8) {
    if !ENABLED.load(Ordering::Relaxed) {
        return;
    }

    let mut driver = TOUCHPAD.lock();
    if !driver.initialized {
        return;
    }

    match driver.protocol {
        TouchpadProtocol::Synaptics => process_synaptics_byte(&mut driver, byte),
        TouchpadProtocol::Alps => process_alps_byte(&mut driver, byte),
        _ => process_generic_byte(&mut driver, byte),
    }
}

/// Process Synaptics packet byte
fn process_synaptics_byte(driver: &mut TouchpadDriver, byte: u8) {
    driver.packet_buf[driver.packet_idx] = byte;
    driver.packet_idx += 1;

    // Synaptics uses 6-byte packets in W-mode
    if driver.packet_idx < 6 {
        return;
    }

    driver.packet_idx = 0;

    // Parse packet
    let b0 = driver.packet_buf[0];
    let b1 = driver.packet_buf[1];
    let b2 = driver.packet_buf[2];
    let b3 = driver.packet_buf[3];
    let b4 = driver.packet_buf[4];
    let b5 = driver.packet_buf[5];

    // Validate packet (bit 3 of byte 0 and 3 must be set)
    if (b0 & 0x08) == 0 || (b3 & 0x08) == 0 {
        return;
    }

    // Extract W value (finger width / number of fingers indicator)
    let w = ((b0 & 0x30) >> 2) | ((b0 & 0x04) >> 1) | ((b3 & 0x04) >> 2);

    // Extract X and Y (13-bit each)
    let x = ((b3 as i32 & 0x10) << 8) | ((b1 as i32 & 0x0F) << 8) | (b4 as i32);
    let y = ((b3 as i32 & 0x20) << 7) | ((b1 as i32 & 0xF0) << 4) | (b5 as i32);

    // Extract Z (pressure)
    let z = b2;

    // Extract buttons
    let left = (b0 & 0x01) != 0;
    let right = (b0 & 0x02) != 0;

    // Determine finger count from W value
    let fingers = match w {
        0 => 2,      // Two fingers
        1 => 3,      // Three fingers
        2 => 4,      // Four or more fingers
        4..=15 => 1, // One finger with width
        _ => 0,      // No finger
    };

    process_touch_data(driver, x, y, z, fingers, left, right);
}

/// Process ALPS packet byte
fn process_alps_byte(driver: &mut TouchpadDriver, byte: u8) {
    // ALPS typically uses 6-byte packets too
    driver.packet_buf[driver.packet_idx] = byte;
    driver.packet_idx += 1;

    if driver.packet_idx < 6 {
        return;
    }

    driver.packet_idx = 0;

    // Basic ALPS parsing (v1/v2 format)
    let b0 = driver.packet_buf[0];
    let b1 = driver.packet_buf[1];
    let b2 = driver.packet_buf[2];
    let b3 = driver.packet_buf[3];
    let b4 = driver.packet_buf[4];
    let b5 = driver.packet_buf[5];

    let x = ((b1 as i32) << 1) | ((b2 as i32 & 0x0F) << 9) | ((b0 as i32 & 0x10) >> 4);
    let y = ((b4 as i32) << 1) | ((b5 as i32 & 0x0F) << 9) | ((b3 as i32 & 0x10) >> 4);
    let z = (b5 & 0x7F) as u8;

    let left = (b0 & 0x01) != 0;
    let right = (b0 & 0x02) != 0;

    let fingers = if z > 30 { 1 } else { 0 };

    process_touch_data(driver, x, y, z, fingers, left, right);
}

/// Process generic PS/2 mouse packet
fn process_generic_byte(driver: &mut TouchpadDriver, byte: u8) {
    driver.packet_buf[driver.packet_idx] = byte;

    // First byte validation
    if driver.packet_idx == 0 && (byte & 0x08) == 0 {
        return; // Invalid first byte
    }

    driver.packet_idx += 1;

    if driver.packet_idx < 3 {
        return;
    }

    driver.packet_idx = 0;

    let b0 = driver.packet_buf[0];
    let b1 = driver.packet_buf[1];
    let b2 = driver.packet_buf[2];

    // Extract relative movement
    let dx = if (b0 & 0x10) != 0 {
        (b1 as i32) - 256
    } else {
        b1 as i32
    };

    let dy = if (b0 & 0x20) != 0 {
        (b2 as i32) - 256
    } else {
        b2 as i32
    };

    let left = (b0 & 0x01) != 0;
    let right = (b0 & 0x02) != 0;

    // Convert relative to pseudo-absolute for state machine
    let x = driver.last_x + dx;
    let y = driver.last_y - dy; // Y is inverted in PS/2

    // Treat as single finger when moving
    let fingers = if dx != 0 || dy != 0 { 1 } else { 0 };
    let pressure = if fingers > 0 { 100 } else { 0 };

    process_touch_data(driver, x, y, pressure, fingers, left, right);
}

/// Process normalized touch data and generate events
fn process_touch_data(
    driver: &mut TouchpadDriver,
    x: i32,
    y: i32,
    pressure: u8,
    fingers: u8,
    left: bool,
    right: bool,
) {
    let now = crate::time::ticks();
    let config = &driver.config;

    // Palm rejection
    if config.palm_threshold > 0 && pressure > config.palm_threshold {
        return;
    }

    // Disable while typing check
    if config.disable_while_typing {
        let last_kbd = LAST_KEYBOARD_TIME.load(Ordering::Relaxed);
        if now.saturating_sub(last_kbd as u64) < 300 {
            return;
        }
    }

    // Track finger count changes
    let prev_fingers = driver.num_fingers;
    driver.num_fingers = fingers;
    driver.prev_fingers = prev_fingers;

    // Calculate deltas
    let dx = if driver.caps.absolute {
        x - driver.last_x
    } else {
        x - driver.last_x // Already delta in generic mode
    };
    let dy = if driver.caps.absolute {
        y - driver.last_y
    } else {
        y - driver.last_y
    };

    // Update button state
    driver.buttons.left = left;
    driver.buttons.right = right;

    // State machine transitions
    let new_state = match (driver.state, fingers) {
        // Idle transitions
        (TouchpadState::Idle, 0) => TouchpadState::Idle,
        (TouchpadState::Idle, 1) => {
            // Finger touched
            driver.fingers[0] = FingerState {
                active: true,
                x,
                y,
                start_x: x,
                start_y: y,
                pressure,
                touch_time: now,
            };
            TouchpadState::OneFinger
        }
        (TouchpadState::Idle, 2) => TouchpadState::TwoFinger,
        (TouchpadState::Idle, _) => TouchpadState::MultiFinger,

        // One finger transitions
        (TouchpadState::OneFinger, 0) => {
            // Finger lifted - check for tap
            let finger = &driver.fingers[0];
            let touch_duration = now - finger.touch_time;
            let distance = ((x - finger.start_x).abs() + (y - finger.start_y).abs()) as u32;

            if config.tap_to_click &&
               touch_duration < config.tap_time as u64 &&
               distance < config.tap_distance
            {
                // Tap detected
                emit_event(TouchpadEvent {
                    event_type: TouchpadEventType::Press,
                    x, y, dx: 0, dy: 0,
                    pressure: 0,
                    fingers: 1,
                    buttons: ButtonState { left: true, right: false, middle: false },
                    timestamp: now,
                });
                driver.last_tap_time = now;
            }

            driver.fingers[0].active = false;
            TouchpadState::Idle
        }
        (TouchpadState::OneFinger, 1) => {
            // Continued single finger - generate move event
            if dx != 0 || dy != 0 {
                emit_move_event(driver, x, y, dx, dy, pressure, 1);
            }
            TouchpadState::OneFinger
        }
        (TouchpadState::OneFinger, 2) => {
            driver.scroll_x = 0;
            driver.scroll_y = 0;
            TouchpadState::TwoFinger
        }
        (TouchpadState::OneFinger, _) => TouchpadState::MultiFinger,

        // Two finger transitions
        (TouchpadState::TwoFinger, 0) => {
            // Both fingers lifted - check for two-finger tap
            if config.two_finger_tap {
                let touch_duration = now - driver.fingers[0].touch_time;
                if touch_duration < config.tap_time as u64 {
                    emit_event(TouchpadEvent {
                        event_type: TouchpadEventType::Press,
                        x, y, dx: 0, dy: 0,
                        pressure: 0,
                        fingers: 2,
                        buttons: ButtonState { left: false, right: true, middle: false },
                        timestamp: now,
                    });
                }
            }
            TouchpadState::Idle
        }
        (TouchpadState::TwoFinger, 1) => TouchpadState::OneFinger,
        (TouchpadState::TwoFinger, 2) => {
            // Two-finger scroll
            if config.two_finger_scroll && (dx.abs() > 2 || dy.abs() > 2) {
                let scroll_y = if config.natural_scrolling { dy } else { -dy };
                let scroll_x = if config.natural_scrolling { dx } else { -dx };

                driver.scroll_y += scroll_y;
                driver.scroll_x += scroll_x;

                // Emit scroll event when accumulated enough
                let scroll_threshold = 20;
                if driver.scroll_y.abs() >= scroll_threshold || driver.scroll_x.abs() >= scroll_threshold {
                    emit_event(TouchpadEvent {
                        event_type: TouchpadEventType::Scroll,
                        x, y,
                        dx: driver.scroll_x / 10,
                        dy: driver.scroll_y / 10,
                        pressure,
                        fingers: 2,
                        buttons: driver.buttons,
                        timestamp: now,
                    });
                    driver.scroll_y = 0;
                    driver.scroll_x = 0;
                }
            }
            TouchpadState::TwoFinger
        }
        (TouchpadState::TwoFinger, _) => TouchpadState::MultiFinger,

        // Multi-finger transitions
        (TouchpadState::MultiFinger, 0) => {
            // Three-finger tap
            if config.three_finger_tap && prev_fingers >= 3 {
                let touch_duration = now - driver.fingers[0].touch_time;
                if touch_duration < config.tap_time as u64 {
                    emit_event(TouchpadEvent {
                        event_type: TouchpadEventType::Press,
                        x, y, dx: 0, dy: 0,
                        pressure: 0,
                        fingers: 3,
                        buttons: ButtonState { left: false, right: false, middle: true },
                        timestamp: now,
                    });
                }
            }
            TouchpadState::Idle
        }
        (TouchpadState::MultiFinger, 1) => TouchpadState::OneFinger,
        (TouchpadState::MultiFinger, 2) => TouchpadState::TwoFinger,
        (TouchpadState::MultiFinger, _) => {
            // Multi-finger gestures (swipes, etc.)
            if dx.abs() > 50 || dy.abs() > 50 {
                emit_event(TouchpadEvent {
                    event_type: TouchpadEventType::Gesture,
                    x, y, dx, dy,
                    pressure,
                    fingers,
                    buttons: driver.buttons,
                    timestamp: now,
                });
            }
            TouchpadState::MultiFinger
        }

        // Other states default to Idle
        (_, 0) => TouchpadState::Idle,
        (_, 1) => TouchpadState::OneFinger,
        (_, 2) => TouchpadState::TwoFinger,
        (_, _) => TouchpadState::MultiFinger,
    };

    driver.state = new_state;
    driver.last_x = x;
    driver.last_y = y;
    driver.last_event_time = now;

    // Handle physical button clicks
    if left || right {
        let button_state = ButtonState { left, right, middle: false };
        emit_event(TouchpadEvent {
            event_type: TouchpadEventType::Press,
            x, y, dx: 0, dy: 0,
            pressure,
            fingers,
            buttons: button_state,
            timestamp: now,
        });
    }
}

/// Emit a move event and report to mouse driver
fn emit_move_event(
    driver: &TouchpadDriver,
    x: i32,
    y: i32,
    dx: i32,
    dy: i32,
    pressure: u8,
    fingers: u8,
) {
    let now = crate::time::ticks();

    // Scale movement by speed setting
    let speed_factor = driver.config.speed as i32;
    let scaled_dx = (dx * speed_factor) / 5;
    let scaled_dy = (dy * speed_factor) / 5;

    emit_event(TouchpadEvent {
        event_type: TouchpadEventType::Move,
        x, y,
        dx: scaled_dx,
        dy: scaled_dy,
        pressure,
        fingers,
        buttons: driver.buttons,
        timestamp: now,
    });

    // Also report to unified mouse driver
    super::mouse::queue_event(
        scaled_dx as i16,
        scaled_dy as i16,
        driver.buttons.left,
        driver.buttons.right,
        driver.buttons.middle,
    );
}

/// Emit an event to the buffer
fn emit_event(event: TouchpadEvent) {
    let mut buf = EVENT_BUFFER.lock();
    if buf.len() < MAX_EVENTS {
        buf.push_back(event);
    }

    // Also report button events to mouse driver
    if event.event_type == TouchpadEventType::Press {
        super::mouse::queue_event(
            0, 0,
            event.buttons.left,
            event.buttons.right,
            event.buttons.middle,
        );
    }
}

// Public API

/// Read next touchpad event
pub fn read_event() -> Option<TouchpadEvent> {
    EVENT_BUFFER.lock().pop_front()
}

/// Check if events available
pub fn has_events() -> bool {
    !EVENT_BUFFER.lock().is_empty()
}

/// Enable touchpad
pub fn enable() {
    ENABLED.store(true, Ordering::SeqCst);
    send_mouse_cmd(0xF4);
    wait_ack();
}

/// Disable touchpad
pub fn disable() {
    ENABLED.store(false, Ordering::SeqCst);
    send_mouse_cmd(0xF5);
    wait_ack();
}

/// Check if enabled
pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Get current configuration
pub fn get_config() -> TouchpadConfig {
    TOUCHPAD.lock().config
}

/// Set configuration
pub fn set_config(config: TouchpadConfig) {
    TOUCHPAD.lock().config = config;
}

/// Get capabilities
pub fn get_capabilities() -> TouchpadCapabilities {
    TOUCHPAD.lock().caps
}

/// Get protocol
pub fn get_protocol() -> TouchpadProtocol {
    TOUCHPAD.lock().protocol
}

/// Notify of keyboard activity (for disable-while-typing)
pub fn notify_keyboard_activity() {
    LAST_KEYBOARD_TIME.store(crate::time::ticks() as u32, Ordering::Relaxed);
}

/// Check if touchpad is initialized
pub fn is_initialized() -> bool {
    TOUCHPAD.lock().initialized
}

// =============================================================================
// USB HID Touchpad Support
// =============================================================================

/// USB HID touchpad device
pub struct UsbTouchpad {
    /// Slot ID in USB controller
    pub slot_id: u8,
    /// Interface number
    pub interface_number: u8,
    /// Endpoint number
    pub endpoint_number: u8,
    /// Endpoint interval
    pub endpoint_interval: u8,
    /// Max packet size
    pub max_packet_size: u16,
    /// Device capabilities
    pub caps: UsbTouchpadCaps,
    /// Last known finger states
    pub fingers: [UsbFingerState; 5],
    /// Number of active fingers
    pub finger_count: u8,
    /// Button state
    pub buttons: u8,
}

/// USB touchpad capabilities (from HID Report Descriptor)
#[derive(Debug, Clone, Copy, Default)]
pub struct UsbTouchpadCaps {
    /// Max X coordinate
    pub max_x: u32,
    /// Max Y coordinate
    pub max_y: u32,
    /// Max pressure
    pub max_pressure: u32,
    /// Max contact count
    pub max_contacts: u8,
    /// Has pressure sensing
    pub has_pressure: bool,
    /// Has width/height (contact size)
    pub has_size: bool,
    /// Device type
    pub device_type: UsbTouchpadType,
    /// Report ID for touch data
    pub touch_report_id: u8,
}

/// USB touchpad device type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UsbTouchpadType {
    #[default]
    Unknown,
    /// Standard touchpad
    Touchpad,
    /// Digitizer/tablet
    Digitizer,
    /// Touch screen
    Touchscreen,
    /// Precision touchpad (Windows 8+ spec)
    PrecisionTouchpad,
}

/// USB finger state
#[derive(Debug, Clone, Copy, Default)]
pub struct UsbFingerState {
    /// Contact ID
    pub contact_id: u8,
    /// Finger is touching
    pub tip_switch: bool,
    /// Finger in range (hovering)
    pub in_range: bool,
    /// Confidence (real finger vs palm)
    pub confidence: bool,
    /// X coordinate
    pub x: u16,
    /// Y coordinate
    pub y: u16,
    /// Pressure
    pub pressure: u16,
    /// Contact width
    pub width: u16,
    /// Contact height
    pub height: u16,
}

/// HID Usage Pages
pub mod hid_usage {
    pub const GENERIC_DESKTOP: u16 = 0x01;
    pub const DIGITIZER: u16 = 0x0D;
    pub const BUTTON: u16 = 0x09;

    // Generic Desktop usages
    pub const USAGE_X: u16 = 0x30;
    pub const USAGE_Y: u16 = 0x31;

    // Digitizer usages
    pub const USAGE_TIP_SWITCH: u16 = 0x42;
    pub const USAGE_IN_RANGE: u16 = 0x32;
    pub const USAGE_TOUCH_VALID: u16 = 0x47;
    pub const USAGE_CONTACT_ID: u16 = 0x51;
    pub const USAGE_CONTACT_COUNT: u16 = 0x54;
    pub const USAGE_CONTACT_MAX: u16 = 0x55;
    pub const USAGE_TIP_PRESSURE: u16 = 0x30;
    pub const USAGE_CONFIDENCE: u16 = 0x47;
    pub const USAGE_WIDTH: u16 = 0x48;
    pub const USAGE_HEIGHT: u16 = 0x49;
    pub const USAGE_SCAN_TIME: u16 = 0x56;

    // Digitizer device types
    pub const USAGE_TOUCHPAD: u16 = 0x05;
    pub const USAGE_TOUCH_SCREEN: u16 = 0x04;
    pub const USAGE_PEN: u16 = 0x02;
    pub const USAGE_FINGER: u16 = 0x22;
}

/// Global USB touchpad list
static USB_TOUCHPADS: Mutex<Vec<UsbTouchpad>> = Mutex::new(Vec::new());

/// USB touchpad report buffer
static USB_TOUCHPAD_BUFFERS: Mutex<Vec<[u8; 64]>> = Mutex::new(Vec::new());

/// Pending USB touchpad polls
static PENDING_USB_TOUCHPAD_POLLS: Mutex<Vec<(u8, u8)>> = Mutex::new(Vec::new());

impl UsbTouchpad {
    /// Create a new USB touchpad device
    pub fn new(
        slot_id: u8,
        interface_number: u8,
        endpoint_number: u8,
        endpoint_interval: u8,
        max_packet_size: u16,
    ) -> Self {
        Self {
            slot_id,
            interface_number,
            endpoint_number,
            endpoint_interval,
            max_packet_size,
            caps: UsbTouchpadCaps::default(),
            fingers: [UsbFingerState::default(); 5],
            finger_count: 0,
            buttons: 0,
        }
    }

    /// Parse HID Report Descriptor to extract touchpad capabilities
    pub fn parse_report_descriptor(&mut self, desc: &[u8]) {
        // Simple HID Report Descriptor parser
        let mut i = 0;
        let mut current_usage_page: u16 = 0;
        let mut logical_max: u32 = 0;
        let mut report_size: u8 = 0;
        let mut report_count: u8 = 0;
        let mut usage_stack: Vec<u16> = Vec::new();

        while i < desc.len() {
            let header = desc[i];
            let size = match header & 0x03 {
                0 => 0,
                1 => 1,
                2 => 2,
                3 => 4,
                _ => 0,
            };
            let item_type = (header >> 2) & 0x03;
            let tag = (header >> 4) & 0x0F;

            // Read data bytes
            let data: u32 = if size == 0 {
                0
            } else if i + 1 < desc.len() {
                match size {
                    1 => desc[i + 1] as u32,
                    2 if i + 2 < desc.len() => {
                        (desc[i + 1] as u32) | ((desc[i + 2] as u32) << 8)
                    }
                    4 if i + 4 < desc.len() => {
                        (desc[i + 1] as u32)
                            | ((desc[i + 2] as u32) << 8)
                            | ((desc[i + 3] as u32) << 16)
                            | ((desc[i + 4] as u32) << 24)
                    }
                    _ => 0,
                }
            } else {
                0
            };

            match item_type {
                // Main items
                0 => match tag {
                    // Input
                    8 => {
                        // Process accumulated usages
                        for usage in &usage_stack {
                            match current_usage_page {
                                hid_usage::DIGITIZER => match *usage {
                                    hid_usage::USAGE_CONTACT_MAX => {
                                        self.caps.max_contacts = logical_max as u8;
                                    }
                                    hid_usage::USAGE_TOUCHPAD => {
                                        self.caps.device_type = UsbTouchpadType::Touchpad;
                                    }
                                    hid_usage::USAGE_TOUCH_SCREEN => {
                                        self.caps.device_type = UsbTouchpadType::Touchscreen;
                                    }
                                    _ => {}
                                },
                                hid_usage::GENERIC_DESKTOP => match *usage {
                                    hid_usage::USAGE_X => {
                                        self.caps.max_x = logical_max;
                                    }
                                    hid_usage::USAGE_Y => {
                                        self.caps.max_y = logical_max;
                                    }
                                    _ => {}
                                },
                                _ => {}
                            }
                        }
                        usage_stack.clear();
                    }
                    // Collection
                    10 => {}
                    // End Collection
                    12 => {}
                    _ => {}
                },
                // Global items
                1 => match tag {
                    // Usage Page
                    0 => {
                        current_usage_page = data as u16;
                    }
                    // Logical Minimum
                    1 => {}
                    // Logical Maximum
                    2 => {
                        logical_max = data;
                    }
                    // Report Size
                    7 => {
                        report_size = data as u8;
                    }
                    // Report ID
                    8 => {
                        self.caps.touch_report_id = data as u8;
                    }
                    // Report Count
                    9 => {
                        report_count = data as u8;
                    }
                    _ => {}
                },
                // Local items
                2 => match tag {
                    // Usage
                    0 => {
                        usage_stack.push(data as u16);
                    }
                    _ => {}
                },
                _ => {}
            }

            i += 1 + size as usize;
        }

        // Set defaults if not found
        if self.caps.max_x == 0 {
            self.caps.max_x = 4096;
        }
        if self.caps.max_y == 0 {
            self.caps.max_y = 4096;
        }
        if self.caps.max_contacts == 0 {
            self.caps.max_contacts = 5;
        }
        if self.caps.device_type == UsbTouchpadType::Unknown {
            self.caps.device_type = UsbTouchpadType::Touchpad;
        }

        crate::kprintln!(
            "touchpad: USB touchpad caps: max_x={}, max_y={}, max_contacts={}, type={:?}",
            self.caps.max_x,
            self.caps.max_y,
            self.caps.max_contacts,
            self.caps.device_type
        );
    }

    /// Process a USB HID touchpad report
    pub fn process_report(&mut self, report: &[u8]) {
        if report.is_empty() {
            return;
        }

        // Check report ID if applicable
        let offset = if self.caps.touch_report_id != 0 {
            if report[0] != self.caps.touch_report_id {
                return;
            }
            1
        } else {
            0
        };

        let data = &report[offset..];
        if data.len() < 6 {
            return;
        }

        // Parse based on typical touchpad report format:
        // [tip_switch/contact_id] [x_lo] [x_hi] [y_lo] [y_hi] [optional: pressure, width, height]
        // This is a simplified parser for common formats

        // Try to parse as Windows Precision Touchpad format
        self.parse_precision_touchpad(data);
    }

    /// Parse Windows Precision Touchpad format
    fn parse_precision_touchpad(&mut self, data: &[u8]) {
        if data.len() < 7 {
            return;
        }

        // Common PTP format:
        // Byte 0: Button state
        // Byte 1: Contact count
        // Per finger (variable, typically 9 bytes each):
        //   Byte 0: Tip switch | confidence (bits 0-1), contact ID (bits 2-7)
        //   Byte 1-2: X coordinate (16-bit)
        //   Byte 3-4: Y coordinate (16-bit)
        //   Optional: width, height, pressure

        self.buttons = data[0] & 0x01;
        let contact_count = data[1].min(5);
        self.finger_count = contact_count;

        let mut offset = 2;
        for i in 0..contact_count as usize {
            if offset + 5 > data.len() {
                break;
            }

            let flags = data[offset];
            self.fingers[i].tip_switch = (flags & 0x01) != 0;
            self.fingers[i].confidence = (flags & 0x02) != 0;
            self.fingers[i].contact_id = (flags >> 2) & 0x3F;

            self.fingers[i].x = (data[offset + 1] as u16) | ((data[offset + 2] as u16) << 8);
            self.fingers[i].y = (data[offset + 3] as u16) | ((data[offset + 4] as u16) << 8);

            // Optional: pressure if available
            if offset + 7 <= data.len() {
                self.fingers[i].pressure =
                    (data[offset + 5] as u16) | ((data[offset + 6] as u16) << 8);
                offset += 7;
            } else {
                self.fingers[i].pressure = if self.fingers[i].tip_switch { 100 } else { 0 };
                offset += 5;
            }
        }

        // Generate touchpad events
        self.generate_events();
    }

    /// Generate touchpad events from current state
    fn generate_events(&mut self) {
        let now = crate::time::ticks();

        // Count active fingers
        let active_count = self.fingers[..self.finger_count as usize]
            .iter()
            .filter(|f| f.tip_switch)
            .count() as u8;

        if active_count == 0 && self.buttons == 0 {
            return;
        }

        // Use first active finger for position
        let (x, y, pressure) = if let Some(finger) = self.fingers[..self.finger_count as usize]
            .iter()
            .find(|f| f.tip_switch)
        {
            // Scale to standard touchpad coordinate space
            let scaled_x = ((finger.x as u32 * 65535) / self.caps.max_x.max(1)) as i32;
            let scaled_y = ((finger.y as u32 * 65535) / self.caps.max_y.max(1)) as i32;
            (scaled_x, scaled_y, finger.pressure as u8)
        } else {
            (0, 0, 0)
        };

        // Get previous position for delta calculation
        let mut driver = TOUCHPAD.lock();
        let dx = x - driver.last_x;
        let dy = y - driver.last_y;

        // Generate event
        let event = TouchpadEvent {
            event_type: if active_count > 0 {
                TouchpadEventType::Move
            } else {
                TouchpadEventType::Release
            },
            x,
            y,
            dx,
            dy,
            pressure,
            fingers: active_count,
            buttons: ButtonState {
                left: (self.buttons & 0x01) != 0,
                right: (self.buttons & 0x02) != 0,
                middle: (self.buttons & 0x04) != 0,
            },
            timestamp: now,
        };

        // Update driver state
        driver.last_x = x;
        driver.last_y = y;
        driver.num_fingers = active_count;

        drop(driver);

        // Emit event
        emit_event(event);

        // Also update mouse driver for cursor movement
        if dx != 0 || dy != 0 {
            // Scale movement for mouse
            let speed = TOUCHPAD.lock().config.speed as i32;
            let mouse_dx = (dx * speed) / 500;
            let mouse_dy = (dy * speed) / 500;
            super::mouse::queue_event(
                mouse_dx as i16,
                mouse_dy as i16,
                (self.buttons & 0x01) != 0,
                (self.buttons & 0x02) != 0,
                (self.buttons & 0x04) != 0,
            );
        }
    }
}

/// Register a USB touchpad device
pub fn register_usb_touchpad(touchpad: UsbTouchpad) {
    let mut touchpads = USB_TOUCHPADS.lock();

    crate::kprintln!(
        "touchpad: USB touchpad registered (slot {}, endpoint {})",
        touchpad.slot_id,
        touchpad.endpoint_number
    );

    // Update main touchpad driver to indicate USB touchpad present
    {
        let mut driver = TOUCHPAD.lock();
        driver.caps.protocol = TouchpadProtocol::I2cHid; // Reuse for USB
        driver.caps.absolute = true;
        driver.caps.max_x = touchpad.caps.max_x;
        driver.caps.max_y = touchpad.caps.max_y;
        driver.caps.max_fingers = touchpad.caps.max_contacts;
        driver.caps.has_pressure = touchpad.caps.has_pressure;
        driver.initialized = true;
    }

    touchpads.push(touchpad);
}

/// Queue USB touchpad interrupt polls
fn queue_usb_touchpad_polls() {
    if let Some(ctrl_arc) = super::usb::xhci::controller() {
        let mut ctrl = ctrl_arc.lock();
        let touchpads = USB_TOUCHPADS.lock();
        let mut buffers = USB_TOUCHPAD_BUFFERS.lock();
        let mut pending = PENDING_USB_TOUCHPAD_POLLS.lock();

        // Ensure we have enough buffers
        while buffers.len() < touchpads.len() {
            buffers.push([0u8; 64]);
        }

        for (i, tp) in touchpads.iter().enumerate() {
            // Check if we already have a pending poll
            if pending
                .iter()
                .any(|(s, e)| *s == tp.slot_id && *e == tp.endpoint_number)
            {
                continue;
            }

            // Queue an interrupt IN transfer
            if ctrl
                .queue_interrupt_in(tp.slot_id, tp.endpoint_number, &mut buffers[i])
                .is_ok()
            {
                pending.push((tp.slot_id, tp.endpoint_number));
            }
        }
    }
}

/// Poll all USB touchpads for new data
pub fn poll_usb_touchpads() {
    // Queue any pending polls
    queue_usb_touchpad_polls();

    // Check for completed transfers
    if let Some(ctrl_arc) = super::usb::xhci::controller() {
        let ctrl = ctrl_arc.lock();

        while let Some((slot_id, ep_id, _residual)) = ctrl.poll_interrupt_transfer() {
            let mut pending = PENDING_USB_TOUCHPAD_POLLS.lock();
            let mut touchpads = USB_TOUCHPADS.lock();
            let buffers = USB_TOUCHPAD_BUFFERS.lock();

            let endpoint_num = if ep_id > 0 { (ep_id - 1) / 2 } else { 0 };

            if let Some(pos) = pending
                .iter()
                .position(|(s, e)| *s == slot_id && *e == endpoint_num)
            {
                pending.remove(pos);

                // Find the touchpad
                for (i, tp) in touchpads.iter_mut().enumerate() {
                    if tp.slot_id == slot_id && tp.endpoint_number == endpoint_num {
                        // Process the report
                        tp.process_report(&buffers[i]);
                        break;
                    }
                }
            }
        }
    }
}

/// Check if any USB touchpads are registered
pub fn has_usb_touchpad() -> bool {
    !USB_TOUCHPADS.lock().is_empty()
}

/// Get count of USB touchpads
pub fn usb_touchpad_count() -> usize {
    USB_TOUCHPADS.lock().len()
}

// HID interface detection for touchpads

/// Check if a HID interface is a touchpad
pub fn is_touchpad_interface(interface_class: u8, interface_subclass: u8, interface_protocol: u8) -> bool {
    // HID class (0x03) with no boot protocol or digitizer-like
    if interface_class != 0x03 {
        return false;
    }
    // Not boot keyboard (1) or boot mouse (2)
    if interface_subclass == 1 && (interface_protocol == 1 || interface_protocol == 2) {
        return false;
    }
    true
}

/// Configure a USB touchpad from device descriptors
pub fn configure_usb_touchpad(
    slot_id: u8,
    interface_number: u8,
    endpoint_number: u8,
    endpoint_interval: u8,
    max_packet_size: u16,
    report_descriptor: Option<&[u8]>,
) {
    let mut touchpad = UsbTouchpad::new(
        slot_id,
        interface_number,
        endpoint_number,
        endpoint_interval,
        max_packet_size,
    );

    // Parse report descriptor if available
    if let Some(desc) = report_descriptor {
        touchpad.parse_report_descriptor(desc);
    } else {
        // Use defaults
        touchpad.caps.max_x = 4096;
        touchpad.caps.max_y = 4096;
        touchpad.caps.max_contacts = 5;
        touchpad.caps.device_type = UsbTouchpadType::Touchpad;
    }

    register_usb_touchpad(touchpad);
}
