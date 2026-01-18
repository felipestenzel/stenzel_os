//! Touchscreen Driver
//!
//! Comprehensive multi-touch touchscreen support for various interfaces:
//! - USB HID touchscreens
//! - I2C touchscreens (common on laptops/tablets)
//! - SPI touchscreens
//! - Resistive and capacitive touch panels
//!
//! ## Features
//! - Multi-touch support (up to 10 simultaneous touch points)
//! - Gesture recognition (tap, swipe, pinch, rotate)
//! - Touch event coordination with display
//! - Calibration support
//! - Palm rejection
//! - Stylus/pen support with pressure sensitivity
//! - Edge gestures for window management

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use crate::sync::IrqSafeMutex;
use crate::util::{KError, KResult};

/// Software square root using Newton's method
fn sqrt_f32(x: f32) -> f32 {
    if x <= 0.0 { return 0.0; }
    let mut guess = x / 2.0;
    for _ in 0..10 {
        guess = (guess + x / guess) / 2.0;
    }
    guess
}

/// Software atan2 approximation
fn atan2_f32(y: f32, x: f32) -> f32 {
    // Simple atan2 approximation returning degrees
    if x == 0.0 && y == 0.0 { return 0.0; }

    let abs_y = if y < 0.0 { -y } else { y };
    let abs_x = if x < 0.0 { -x } else { x };

    // Approximation using the ratio
    let r = if abs_x > abs_y {
        let t = abs_y / abs_x;
        let angle = 45.0 * t;
        if x >= 0.0 { angle } else { 180.0 - angle }
    } else {
        let t = abs_x / abs_y;
        let angle = 90.0 - 45.0 * t;
        if x >= 0.0 { angle } else { 180.0 - angle }
    };

    if y >= 0.0 { r } else { -r }
}

/// Maximum simultaneous touch points
pub const MAX_TOUCH_POINTS: usize = 10;

/// Touch event type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchEventType {
    /// Finger/stylus touched the screen
    Down,
    /// Touch moved on screen
    Move,
    /// Finger/stylus lifted from screen
    Up,
    /// Touch cancelled (palm rejection, etc.)
    Cancel,
    /// Hover detected (for styluses)
    Hover,
    /// Stylus button pressed
    StylusButton,
}

/// Touch point state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchPointState {
    /// Not tracking
    Inactive,
    /// Touch is active
    Active,
    /// Touch moving
    Moving,
    /// Touch ended but still processing
    Ended,
}

/// Touch tool type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchToolType {
    /// Finger touch
    Finger,
    /// Stylus/pen
    Stylus,
    /// Eraser end of stylus
    Eraser,
    /// Mouse pointer emulation
    Mouse,
    /// Unknown tool
    Unknown,
}

/// Touch point data
#[derive(Debug, Clone, Copy)]
pub struct TouchPoint {
    /// Tracking ID (unique per touch)
    pub tracking_id: i32,
    /// Touch slot (0 to MAX_TOUCH_POINTS-1)
    pub slot: u8,
    /// Current state
    pub state: TouchPointState,
    /// Tool type
    pub tool: TouchToolType,
    /// X position in screen coordinates
    pub x: i32,
    /// Y position in screen coordinates
    pub y: i32,
    /// Raw X from device
    pub raw_x: i32,
    /// Raw Y from device
    pub raw_y: i32,
    /// Touch width (major axis)
    pub width_major: u16,
    /// Touch height (minor axis)
    pub width_minor: u16,
    /// Touch pressure (0-1023)
    pub pressure: u16,
    /// Touch orientation (0-360)
    pub orientation: i16,
    /// Distance from screen (for hover)
    pub distance: u16,
    /// Timestamp of last update
    pub timestamp: u64,
    /// Delta X since last event
    pub dx: i32,
    /// Delta Y since last event
    pub dy: i32,
}

impl Default for TouchPoint {
    fn default() -> Self {
        Self {
            tracking_id: -1,
            slot: 0,
            state: TouchPointState::Inactive,
            tool: TouchToolType::Finger,
            x: 0,
            y: 0,
            raw_x: 0,
            raw_y: 0,
            width_major: 0,
            width_minor: 0,
            pressure: 0,
            orientation: 0,
            distance: 0,
            timestamp: 0,
            dx: 0,
            dy: 0,
        }
    }
}

impl TouchPoint {
    /// Check if this touch point is active
    pub fn is_active(&self) -> bool {
        self.state == TouchPointState::Active || self.state == TouchPointState::Moving
    }
}

/// Touch event
#[derive(Debug, Clone, Copy)]
pub struct TouchEvent {
    /// Event type
    pub event_type: TouchEventType,
    /// Touch point data
    pub point: TouchPoint,
    /// Number of active touches
    pub touch_count: u8,
    /// Timestamp
    pub timestamp: u64,
}

/// Gesture type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GestureType {
    /// No gesture
    None,
    /// Single tap
    Tap,
    /// Double tap
    DoubleTap,
    /// Long press
    LongPress,
    /// Swipe direction
    SwipeLeft,
    SwipeRight,
    SwipeUp,
    SwipeDown,
    /// Two-finger pinch (zoom out)
    PinchIn,
    /// Two-finger spread (zoom in)
    PinchOut,
    /// Two-finger rotate
    Rotate,
    /// Two-finger scroll
    Scroll,
    /// Three-finger swipe
    ThreeFingerSwipeLeft,
    ThreeFingerSwipeRight,
    ThreeFingerSwipeUp,
    ThreeFingerSwipeDown,
    /// Four-finger gesture
    FourFingerSwipeUp,
    FourFingerSwipeDown,
    /// Edge swipe (from screen edge)
    EdgeSwipeLeft,
    EdgeSwipeRight,
    EdgeSwipeTop,
    EdgeSwipeBottom,
}

/// Gesture event
#[derive(Debug, Clone, Copy)]
pub struct GestureEvent {
    /// Gesture type
    pub gesture: GestureType,
    /// Gesture state (begin, update, end)
    pub state: GestureState,
    /// Center X position
    pub x: i32,
    /// Center Y position
    pub y: i32,
    /// Delta X (for swipes and scrolls)
    pub dx: i32,
    /// Delta Y (for swipes and scrolls)
    pub dy: i32,
    /// Scale factor (for pinch gestures, 1.0 = no change)
    pub scale: f32,
    /// Rotation angle in degrees (for rotate gestures)
    pub rotation: f32,
    /// Number of fingers involved
    pub finger_count: u8,
    /// Timestamp
    pub timestamp: u64,
}

/// Gesture state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GestureState {
    /// Gesture starting
    Begin,
    /// Gesture continuing
    Update,
    /// Gesture ending
    End,
    /// Gesture cancelled
    Cancel,
}

/// Touchscreen interface type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchInterface {
    /// USB HID
    UsbHid,
    /// I2C HID
    I2cHid,
    /// SPI
    Spi,
    /// Serial
    Serial,
    /// Platform-specific
    Platform,
    /// Unknown
    Unknown,
}

/// Touch panel technology
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchTechnology {
    /// Capacitive (most common)
    Capacitive,
    /// Resistive (older panels)
    Resistive,
    /// Surface Acoustic Wave
    Saw,
    /// Infrared
    Infrared,
    /// Optical
    Optical,
    /// Unknown
    Unknown,
}

/// Touchscreen vendor IDs
pub mod vendor_ids {
    pub const GOODIX: u16 = 0x27C6;
    pub const ELAN: u16 = 0x04F3;
    pub const ATMEL: u16 = 0x03EB;
    pub const SYNAPTICS: u16 = 0x06CB;
    pub const WACOM: u16 = 0x056A;
    pub const NTRIG: u16 = 0x1B96;
    pub const ILITEK: u16 = 0x222A;
    pub const FOCALTECH: u16 = 0x2808;
    pub const HIMAX: u16 = 0x1FF7;
    pub const SILEAD: u16 = 0x1680;
    pub const SIS: u16 = 0x0457;
    pub const RAYDIUM: u16 = 0x2386;
}

/// Touchscreen capabilities
#[derive(Debug, Clone)]
pub struct TouchscreenCapabilities {
    /// Interface type
    pub interface: TouchInterface,
    /// Technology
    pub technology: TouchTechnology,
    /// Maximum X coordinate
    pub max_x: u32,
    /// Maximum Y coordinate
    pub max_y: u32,
    /// Maximum pressure
    pub max_pressure: u32,
    /// Maximum touch width
    pub max_width: u32,
    /// Maximum simultaneous touches
    pub max_touches: u8,
    /// Supports pressure
    pub has_pressure: bool,
    /// Supports touch width/size
    pub has_width: bool,
    /// Supports orientation
    pub has_orientation: bool,
    /// Supports stylus
    pub has_stylus: bool,
    /// Supports hover
    pub has_hover: bool,
    /// Supports palm rejection
    pub has_palm_rejection: bool,
    /// Resolution in units per mm (X)
    pub resolution_x: u32,
    /// Resolution in units per mm (Y)
    pub resolution_y: u32,
}

impl Default for TouchscreenCapabilities {
    fn default() -> Self {
        Self {
            interface: TouchInterface::Unknown,
            technology: TouchTechnology::Capacitive,
            max_x: 32767,
            max_y: 32767,
            max_pressure: 1023,
            max_width: 255,
            max_touches: 10,
            has_pressure: true,
            has_width: true,
            has_orientation: false,
            has_stylus: false,
            has_hover: false,
            has_palm_rejection: true,
            resolution_x: 40,
            resolution_y: 40,
        }
    }
}

/// Calibration data
#[derive(Debug, Clone, Copy)]
pub struct CalibrationData {
    /// X offset
    pub x_offset: i32,
    /// Y offset
    pub y_offset: i32,
    /// X scale factor
    pub x_scale: f32,
    /// Y scale factor
    pub y_scale: f32,
    /// Swap X and Y axes
    pub swap_xy: bool,
    /// Invert X axis
    pub invert_x: bool,
    /// Invert Y axis
    pub invert_y: bool,
    /// Screen width in pixels
    pub screen_width: u32,
    /// Screen height in pixels
    pub screen_height: u32,
}

impl Default for CalibrationData {
    fn default() -> Self {
        Self {
            x_offset: 0,
            y_offset: 0,
            x_scale: 1.0,
            y_scale: 1.0,
            swap_xy: false,
            invert_x: false,
            invert_y: false,
            screen_width: 1920,
            screen_height: 1080,
        }
    }
}

/// Touchscreen configuration
#[derive(Debug, Clone)]
pub struct TouchscreenConfig {
    /// Enable touch
    pub enabled: bool,
    /// Enable gestures
    pub gestures_enabled: bool,
    /// Tap timeout (ms)
    pub tap_timeout_ms: u32,
    /// Double tap timeout (ms)
    pub double_tap_timeout_ms: u32,
    /// Long press timeout (ms)
    pub long_press_timeout_ms: u32,
    /// Minimum swipe distance (pixels)
    pub swipe_threshold: u32,
    /// Edge swipe width (pixels from edge)
    pub edge_swipe_width: u32,
    /// Palm rejection threshold (touch size)
    pub palm_threshold: u16,
    /// Finger rejection (when stylus is detected)
    pub stylus_finger_rejection: bool,
    /// Calibration data
    pub calibration: CalibrationData,
}

impl Default for TouchscreenConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            gestures_enabled: true,
            tap_timeout_ms: 180,
            double_tap_timeout_ms: 300,
            long_press_timeout_ms: 500,
            swipe_threshold: 50,
            edge_swipe_width: 20,
            palm_threshold: 60,
            stylus_finger_rejection: true,
            calibration: CalibrationData::default(),
        }
    }
}

/// Touchscreen statistics
#[derive(Debug, Clone, Default)]
pub struct TouchscreenStats {
    /// Total touch events
    pub touch_events: u64,
    /// Total touch down events
    pub touch_downs: u64,
    /// Total touch up events
    pub touch_ups: u64,
    /// Total gestures detected
    pub gestures_detected: u64,
    /// Palm rejections
    pub palm_rejections: u64,
    /// Calibration runs
    pub calibration_runs: u32,
    /// Last touch timestamp
    pub last_touch_time: u64,
}

/// Touch event callback type
pub type TouchCallback = fn(TouchEvent);
/// Gesture event callback type
pub type GestureCallback = fn(GestureEvent);

/// Touchscreen device
pub struct TouchscreenDevice {
    /// Device ID
    pub id: u32,
    /// Device name
    pub name: String,
    /// Vendor ID
    pub vendor_id: u16,
    /// Product ID
    pub product_id: u16,
    /// Capabilities
    pub capabilities: TouchscreenCapabilities,
    /// Configuration
    pub config: TouchscreenConfig,
    /// Current touch points
    pub touch_points: [TouchPoint; MAX_TOUCH_POINTS],
    /// Active touch count
    pub active_touches: u8,
    /// Statistics
    pub stats: TouchscreenStats,
    /// Touch event callback
    pub on_touch: Option<TouchCallback>,
    /// Gesture event callback
    pub on_gesture: Option<GestureCallback>,
    /// Gesture recognizer state
    gesture_state: GestureRecognizerState,
    /// Initialized flag
    pub initialized: bool,
}

/// Internal gesture recognizer state
#[derive(Debug, Clone)]
struct GestureRecognizerState {
    /// First touch down position
    initial_x: i32,
    initial_y: i32,
    /// First touch down time
    initial_time: u64,
    /// Last tap time (for double-tap detection)
    last_tap_time: u64,
    /// Last tap position
    last_tap_x: i32,
    last_tap_y: i32,
    /// Initial distance between fingers (for pinch)
    initial_distance: f32,
    /// Initial angle between fingers (for rotate)
    initial_angle: f32,
    /// Current gesture being tracked
    current_gesture: GestureType,
    /// Gesture started
    gesture_started: bool,
}

impl Default for GestureRecognizerState {
    fn default() -> Self {
        Self {
            initial_x: 0,
            initial_y: 0,
            initial_time: 0,
            last_tap_time: 0,
            last_tap_x: 0,
            last_tap_y: 0,
            initial_distance: 0.0,
            initial_angle: 0.0,
            current_gesture: GestureType::None,
            gesture_started: false,
        }
    }
}

impl TouchscreenDevice {
    /// Create a new touchscreen device
    pub fn new(id: u32, vendor_id: u16, product_id: u16) -> Self {
        Self {
            id,
            name: alloc::format!("Touchscreen {:04X}:{:04X}", vendor_id, product_id),
            vendor_id,
            product_id,
            capabilities: TouchscreenCapabilities::default(),
            config: TouchscreenConfig::default(),
            touch_points: [TouchPoint::default(); MAX_TOUCH_POINTS],
            active_touches: 0,
            stats: TouchscreenStats::default(),
            on_touch: None,
            on_gesture: None,
            gesture_state: GestureRecognizerState::default(),
            initialized: false,
        }
    }

    /// Initialize the device
    pub fn init(&mut self) -> KResult<()> {
        // Device-specific initialization would go here
        // For now, mark as initialized

        self.initialized = true;
        crate::kprintln!("touchscreen: {} initialized", self.name);

        Ok(())
    }

    /// Process a raw touch event from the hardware
    pub fn process_touch(&mut self, slot: u8, tracking_id: i32, raw_x: i32, raw_y: i32,
                          pressure: u16, width: u16, tool: TouchToolType) {

        if slot as usize >= MAX_TOUCH_POINTS {
            return;
        }

        let now = crate::time::uptime_ms();

        // Apply palm rejection
        if self.config.palm_threshold > 0 && width > self.config.palm_threshold {
            // Treat as palm, emit cancel if was tracking
            if self.touch_points[slot as usize].is_active() {
                self.emit_touch_event(TouchEventType::Cancel, slot);
                self.stats.palm_rejections += 1;
            }
            return;
        }

        // Get old values before modifying
        let old_x = self.touch_points[slot as usize].x;
        let old_y = self.touch_points[slot as usize].y;
        let was_active = self.touch_points[slot as usize].is_active();

        // Apply calibration (borrowing self immutably)
        let (screen_x, screen_y) = self.calibrate(raw_x, raw_y);

        // Update touch point (scoped mutable borrow)
        let event_type = {
            let point = &mut self.touch_points[slot as usize];
            point.tracking_id = tracking_id;
            point.slot = slot;
            point.tool = tool;
            point.raw_x = raw_x;
            point.raw_y = raw_y;
            point.x = screen_x;
            point.y = screen_y;
            point.pressure = pressure;
            point.width_major = width;
            point.dx = screen_x - old_x;
            point.dy = screen_y - old_y;
            point.timestamp = now;

            // Determine event type
            if tracking_id >= 0 {
                if !was_active {
                    point.state = TouchPointState::Active;
                    TouchEventType::Down
                } else {
                    point.state = TouchPointState::Moving;
                    TouchEventType::Move
                }
            } else {
                point.state = TouchPointState::Ended;
                TouchEventType::Up
            }
        };

        // Update counters
        if event_type == TouchEventType::Down {
            self.active_touches += 1;
            self.stats.touch_downs += 1;
        } else if event_type == TouchEventType::Up {
            if self.active_touches > 0 {
                self.active_touches -= 1;
            }
            self.stats.touch_ups += 1;
        }

        // Update stats
        self.stats.touch_events += 1;
        self.stats.last_touch_time = now;

        // Emit event
        self.emit_touch_event(event_type, slot);

        // Process gestures if enabled
        if self.config.gestures_enabled {
            self.process_gesture(event_type, slot);
        }

        // Clear inactive point
        if event_type == TouchEventType::Up || event_type == TouchEventType::Cancel {
            self.touch_points[slot as usize].state = TouchPointState::Inactive;
            self.touch_points[slot as usize].tracking_id = -1;
        }
    }

    /// Apply calibration to raw coordinates
    fn calibrate(&self, raw_x: i32, raw_y: i32) -> (i32, i32) {
        let cal = &self.config.calibration;
        let caps = &self.capabilities;

        // Apply offset
        let x = raw_x + cal.x_offset;
        let y = raw_y + cal.y_offset;

        // Swap if needed
        let (x, y) = if cal.swap_xy { (y, x) } else { (x, y) };

        // Scale to screen coordinates
        let x = ((x as f32 * cal.x_scale * cal.screen_width as f32) / caps.max_x as f32) as i32;
        let y = ((y as f32 * cal.y_scale * cal.screen_height as f32) / caps.max_y as f32) as i32;

        // Invert if needed
        let x = if cal.invert_x { cal.screen_width as i32 - x } else { x };
        let y = if cal.invert_y { cal.screen_height as i32 - y } else { y };

        // Clamp to screen bounds
        let x = x.clamp(0, cal.screen_width as i32 - 1);
        let y = y.clamp(0, cal.screen_height as i32 - 1);

        (x, y)
    }

    /// Emit a touch event
    fn emit_touch_event(&self, event_type: TouchEventType, slot: u8) {
        if let Some(callback) = self.on_touch {
            let event = TouchEvent {
                event_type,
                point: self.touch_points[slot as usize],
                touch_count: self.active_touches,
                timestamp: crate::time::uptime_ms(),
            };
            callback(event);
        }
    }

    /// Process gesture recognition
    fn process_gesture(&mut self, event_type: TouchEventType, slot: u8) {
        let now = crate::time::uptime_ms();

        // Extract point coordinates to avoid borrowing issues
        let point_x = self.touch_points[slot as usize].x;
        let point_y = self.touch_points[slot as usize].y;

        match event_type {
            TouchEventType::Down => {
                if self.active_touches == 1 {
                    // First finger down
                    self.gesture_state.initial_x = point_x;
                    self.gesture_state.initial_y = point_y;
                    self.gesture_state.initial_time = now;
                    self.gesture_state.gesture_started = false;
                } else if self.active_touches == 2 {
                    // Second finger - calculate initial distance for pinch
                    self.gesture_state.initial_distance = self.calculate_finger_distance();
                    self.gesture_state.initial_angle = self.calculate_finger_angle();
                }
            }

            TouchEventType::Move => {
                if self.active_touches == 1 && !self.gesture_state.gesture_started {
                    // Check for swipe
                    let dx = point_x - self.gesture_state.initial_x;
                    let dy = point_y - self.gesture_state.initial_y;
                    let distance = sqrt_f32((dx * dx + dy * dy) as f32);

                    if distance > self.config.swipe_threshold as f32 {
                        let gesture = self.determine_swipe_direction(dx, dy, 1);
                        if gesture != GestureType::None {
                            self.emit_gesture(gesture, GestureState::Begin, dx, dy, 1.0, 0.0, 1);
                            self.gesture_state.current_gesture = gesture;
                            self.gesture_state.gesture_started = true;
                        }
                    }
                } else if self.active_touches == 2 {
                    // Two-finger gesture (pinch/rotate/scroll)
                    let current_distance = self.calculate_finger_distance();
                    let current_angle = self.calculate_finger_angle();

                    let scale = if self.gesture_state.initial_distance > 0.0 {
                        current_distance / self.gesture_state.initial_distance
                    } else {
                        1.0
                    };

                    let rotation = current_angle - self.gesture_state.initial_angle;

                    // Determine gesture type
                    if (scale - 1.0).abs() > 0.1 {
                        // Pinch gesture
                        let gesture = if scale > 1.0 { GestureType::PinchOut } else { GestureType::PinchIn };
                        self.emit_gesture(gesture, GestureState::Update, 0, 0, scale, rotation, 2);
                        self.gesture_state.current_gesture = gesture;
                    } else if rotation.abs() > 10.0 {
                        // Rotate gesture
                        self.emit_gesture(GestureType::Rotate, GestureState::Update, 0, 0, scale, rotation, 2);
                        self.gesture_state.current_gesture = GestureType::Rotate;
                    } else {
                        // Scroll gesture
                        let (center_x, center_y) = self.calculate_center();
                        let dx = center_x - self.gesture_state.initial_x;
                        let dy = center_y - self.gesture_state.initial_y;
                        self.emit_gesture(GestureType::Scroll, GestureState::Update, dx, dy, 1.0, 0.0, 2);
                        self.gesture_state.current_gesture = GestureType::Scroll;
                    }
                    self.gesture_state.gesture_started = true;
                } else if self.active_touches >= 3 {
                    // Multi-finger swipe
                    let (center_x, center_y) = self.calculate_center();
                    let dx = center_x - self.gesture_state.initial_x;
                    let dy = center_y - self.gesture_state.initial_y;
                    let distance = sqrt_f32((dx * dx + dy * dy) as f32);

                    if distance > self.config.swipe_threshold as f32 {
                        let gesture = self.determine_swipe_direction(dx, dy, self.active_touches);
                        if gesture != GestureType::None && !self.gesture_state.gesture_started {
                            self.emit_gesture(gesture, GestureState::Begin, dx, dy, 1.0, 0.0, self.active_touches);
                            self.gesture_state.current_gesture = gesture;
                            self.gesture_state.gesture_started = true;
                        }
                    }
                }
            }

            TouchEventType::Up => {
                if self.gesture_state.gesture_started {
                    // End current gesture
                    self.emit_gesture(self.gesture_state.current_gesture, GestureState::End, 0, 0, 1.0, 0.0, self.active_touches + 1);
                    self.gesture_state.gesture_started = false;
                    self.gesture_state.current_gesture = GestureType::None;
                } else if self.active_touches == 0 {
                    // Check for tap
                    let elapsed = now - self.gesture_state.initial_time;
                    let dx = point_x - self.gesture_state.initial_x;
                    let dy = point_y - self.gesture_state.initial_y;
                    let distance = sqrt_f32((dx * dx + dy * dy) as f32);

                    if elapsed < self.config.tap_timeout_ms as u64 && distance < 20.0 {
                        // Check for double tap
                        let tap_elapsed = now - self.gesture_state.last_tap_time;
                        let tap_dx = point_x - self.gesture_state.last_tap_x;
                        let tap_dy = point_y - self.gesture_state.last_tap_y;
                        let tap_distance = sqrt_f32((tap_dx * tap_dx + tap_dy * tap_dy) as f32);

                        if tap_elapsed < self.config.double_tap_timeout_ms as u64 && tap_distance < 50.0 {
                            self.emit_gesture(GestureType::DoubleTap, GestureState::End, 0, 0, 1.0, 0.0, 1);
                        } else {
                            self.emit_gesture(GestureType::Tap, GestureState::End, 0, 0, 1.0, 0.0, 1);
                        }

                        self.gesture_state.last_tap_time = now;
                        self.gesture_state.last_tap_x = point_x;
                        self.gesture_state.last_tap_y = point_y;
                    } else if elapsed >= self.config.long_press_timeout_ms as u64 && distance < 20.0 {
                        self.emit_gesture(GestureType::LongPress, GestureState::End, 0, 0, 1.0, 0.0, 1);
                    }
                }
            }

            TouchEventType::Cancel => {
                if self.gesture_state.gesture_started {
                    self.emit_gesture(self.gesture_state.current_gesture, GestureState::Cancel, 0, 0, 1.0, 0.0, self.active_touches);
                    self.gesture_state.gesture_started = false;
                    self.gesture_state.current_gesture = GestureType::None;
                }
            }

            _ => {}
        }
    }

    /// Calculate distance between first two fingers
    fn calculate_finger_distance(&self) -> f32 {
        let mut active: Vec<&TouchPoint> = self.touch_points.iter()
            .filter(|p| p.is_active())
            .take(2)
            .collect();

        if active.len() < 2 {
            return 0.0;
        }

        let dx = active[1].x - active[0].x;
        let dy = active[1].y - active[0].y;
        sqrt_f32((dx * dx + dy * dy) as f32)
    }

    /// Calculate angle between first two fingers
    fn calculate_finger_angle(&self) -> f32 {
        let mut active: Vec<&TouchPoint> = self.touch_points.iter()
            .filter(|p| p.is_active())
            .take(2)
            .collect();

        if active.len() < 2 {
            return 0.0;
        }

        let dx = (active[1].x - active[0].x) as f32;
        let dy = (active[1].y - active[0].y) as f32;

        // atan2 returns radians, convert to degrees
        atan2_f32(dy, dx)
    }

    /// Calculate center point of all active touches
    fn calculate_center(&self) -> (i32, i32) {
        let active: Vec<&TouchPoint> = self.touch_points.iter()
            .filter(|p| p.is_active())
            .collect();

        if active.is_empty() {
            return (0, 0);
        }

        let sum_x: i32 = active.iter().map(|p| p.x).sum();
        let sum_y: i32 = active.iter().map(|p| p.y).sum();
        let count = active.len() as i32;

        (sum_x / count, sum_y / count)
    }

    /// Determine swipe direction
    fn determine_swipe_direction(&self, dx: i32, dy: i32, finger_count: u8) -> GestureType {
        let abs_dx = dx.abs();
        let abs_dy = dy.abs();

        // Check for edge swipe first
        let cal = &self.config.calibration;
        let edge_width = self.config.edge_swipe_width as i32;
        let initial_x = self.gesture_state.initial_x;
        let initial_y = self.gesture_state.initial_y;

        if initial_x < edge_width {
            return GestureType::EdgeSwipeLeft;
        } else if initial_x > (cal.screen_width as i32 - edge_width) {
            return GestureType::EdgeSwipeRight;
        } else if initial_y < edge_width {
            return GestureType::EdgeSwipeTop;
        } else if initial_y > (cal.screen_height as i32 - edge_width) {
            return GestureType::EdgeSwipeBottom;
        }

        // Regular swipes
        if abs_dx > abs_dy {
            // Horizontal swipe
            match finger_count {
                3 => if dx > 0 { GestureType::ThreeFingerSwipeRight } else { GestureType::ThreeFingerSwipeLeft },
                _ => if dx > 0 { GestureType::SwipeRight } else { GestureType::SwipeLeft },
            }
        } else {
            // Vertical swipe
            match finger_count {
                3 => if dy > 0 { GestureType::ThreeFingerSwipeDown } else { GestureType::ThreeFingerSwipeUp },
                4 => if dy > 0 { GestureType::FourFingerSwipeDown } else { GestureType::FourFingerSwipeUp },
                _ => if dy > 0 { GestureType::SwipeDown } else { GestureType::SwipeUp },
            }
        }
    }

    /// Emit a gesture event
    fn emit_gesture(&mut self, gesture: GestureType, state: GestureState, dx: i32, dy: i32, scale: f32, rotation: f32, finger_count: u8) {
        if gesture == GestureType::None {
            return;
        }

        self.stats.gestures_detected += 1;

        if let Some(callback) = self.on_gesture {
            let (x, y) = self.calculate_center();
            let event = GestureEvent {
                gesture,
                state,
                x,
                y,
                dx,
                dy,
                scale,
                rotation,
                finger_count,
                timestamp: crate::time::uptime_ms(),
            };
            callback(event);
        }
    }

    /// Get active touch points
    pub fn get_active_touches(&self) -> Vec<&TouchPoint> {
        self.touch_points.iter().filter(|p| p.is_active()).collect()
    }

    /// Set calibration data
    pub fn set_calibration(&mut self, calibration: CalibrationData) {
        self.config.calibration = calibration;
        self.stats.calibration_runs += 1;
        crate::kprintln!("touchscreen: calibration updated");
    }

    /// Get device info string
    pub fn info_string(&self) -> String {
        alloc::format!(
            "{} ({:04X}:{:04X})\n  Interface: {:?}\n  Technology: {:?}\n  Max touches: {}\n  Resolution: {}x{}\n  Active touches: {}",
            self.name,
            self.vendor_id,
            self.product_id,
            self.capabilities.interface,
            self.capabilities.technology,
            self.capabilities.max_touches,
            self.capabilities.max_x,
            self.capabilities.max_y,
            self.active_touches
        )
    }
}

/// Touchscreen manager
pub struct TouchscreenManager {
    /// Registered devices
    devices: BTreeMap<u32, TouchscreenDevice>,
    /// Next device ID
    next_id: AtomicU32,
    /// Primary device ID
    primary_device: Option<u32>,
    /// Initialized flag
    initialized: bool,
}

impl TouchscreenManager {
    /// Create a new touchscreen manager
    pub const fn new() -> Self {
        Self {
            devices: BTreeMap::new(),
            next_id: AtomicU32::new(1),
            primary_device: None,
            initialized: false,
        }
    }

    /// Initialize the manager
    pub fn init(&mut self) {
        if self.initialized {
            return;
        }

        crate::kprintln!("touchscreen: manager initialized");
        self.initialized = true;
    }

    /// Register a touchscreen device
    pub fn register_device(&mut self, vendor_id: u16, product_id: u16) -> u32 {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let mut device = TouchscreenDevice::new(id, vendor_id, product_id);

        if let Err(e) = device.init() {
            crate::kprintln!("touchscreen: failed to init device {}: {:?}", id, e);
        }

        self.devices.insert(id, device);

        if self.primary_device.is_none() {
            self.primary_device = Some(id);
        }

        crate::kprintln!("touchscreen: registered device {} ({:04X}:{:04X})", id, vendor_id, product_id);
        id
    }

    /// Unregister a device
    pub fn unregister_device(&mut self, id: u32) {
        self.devices.remove(&id);
        if self.primary_device == Some(id) {
            self.primary_device = self.devices.keys().next().copied();
        }
    }

    /// Get device by ID
    pub fn get_device(&mut self, id: u32) -> Option<&mut TouchscreenDevice> {
        self.devices.get_mut(&id)
    }

    /// Get primary device
    pub fn get_primary_device(&mut self) -> Option<&mut TouchscreenDevice> {
        self.primary_device.and_then(|id| self.devices.get_mut(&id))
    }

    /// Set primary device
    pub fn set_primary_device(&mut self, id: u32) {
        if self.devices.contains_key(&id) {
            self.primary_device = Some(id);
        }
    }

    /// Get device count
    pub fn device_count(&self) -> usize {
        self.devices.len()
    }

    /// List all devices
    pub fn list_devices(&self) -> Vec<u32> {
        self.devices.keys().copied().collect()
    }

    /// Format status string
    pub fn format_status(&self) -> String {
        let mut s = String::from("Touchscreen Manager:\n");
        use core::fmt::Write;

        let _ = writeln!(s, "  Devices: {}", self.devices.len());
        let _ = writeln!(s, "  Primary: {:?}", self.primary_device);

        for device in self.devices.values() {
            let _ = writeln!(s, "\n{}", device.info_string());
        }

        s
    }
}

// =============================================================================
// Global Instance
// =============================================================================

pub static TOUCHSCREEN_MANAGER: IrqSafeMutex<TouchscreenManager> = IrqSafeMutex::new(TouchscreenManager::new());

/// Initialize touchscreen subsystem
pub fn init() {
    TOUCHSCREEN_MANAGER.lock().init();
}

/// Register a touchscreen device
pub fn register_device(vendor_id: u16, product_id: u16) -> u32 {
    TOUCHSCREEN_MANAGER.lock().register_device(vendor_id, product_id)
}

/// Unregister a device
pub fn unregister_device(id: u32) {
    TOUCHSCREEN_MANAGER.lock().unregister_device(id);
}

/// Get device count
pub fn device_count() -> usize {
    TOUCHSCREEN_MANAGER.lock().device_count()
}

/// List devices
pub fn list_devices() -> Vec<u32> {
    TOUCHSCREEN_MANAGER.lock().list_devices()
}

/// Format status
pub fn status() -> String {
    TOUCHSCREEN_MANAGER.lock().format_status()
}

/// Process touch event on a device
pub fn process_touch(device_id: u32, slot: u8, tracking_id: i32, x: i32, y: i32, pressure: u16, width: u16) {
    if let Some(device) = TOUCHSCREEN_MANAGER.lock().get_device(device_id) {
        device.process_touch(slot, tracking_id, x, y, pressure, width, TouchToolType::Finger);
    }
}

/// Check if device is a known touchscreen
pub fn is_touchscreen_device(vendor_id: u16, _product_id: u16) -> bool {
    matches!(vendor_id,
        vendor_ids::GOODIX |
        vendor_ids::ELAN |
        vendor_ids::ATMEL |
        vendor_ids::SYNAPTICS |
        vendor_ids::WACOM |
        vendor_ids::NTRIG |
        vendor_ids::ILITEK |
        vendor_ids::FOCALTECH |
        vendor_ids::HIMAX |
        vendor_ids::SILEAD |
        vendor_ids::SIS |
        vendor_ids::RAYDIUM
    )
}
