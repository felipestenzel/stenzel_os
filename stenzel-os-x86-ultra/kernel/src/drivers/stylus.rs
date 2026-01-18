//! Stylus/Pen Input Driver
//!
//! Provides support for pressure-sensitive stylus and pen input devices
//! including Wacom tablets, Surface Pen, and other digitizer pens.

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;

use crate::sync::IrqSafeMutex;

/// Stylus tool type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StylusToolType {
    /// Standard pen tip
    Pen,
    /// Eraser end of pen
    Eraser,
    /// Brush tool
    Brush,
    /// Pencil tool
    Pencil,
    /// Airbrush tool
    Airbrush,
    /// Finger touch (some digitizers)
    Finger,
    /// Mouse mode (some Wacom tablets)
    Mouse,
    /// Lens cursor (puck)
    Lens,
    /// Unknown tool
    Unknown,
}

impl StylusToolType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pen => "pen",
            Self::Eraser => "eraser",
            Self::Brush => "brush",
            Self::Pencil => "pencil",
            Self::Airbrush => "airbrush",
            Self::Finger => "finger",
            Self::Mouse => "mouse",
            Self::Lens => "lens",
            Self::Unknown => "unknown",
        }
    }

    pub fn from_usb_id(tool_id: u16) -> Self {
        match tool_id {
            0x0802 | 0x0822 | 0x0842 | 0x0852 => Self::Pen,
            0x080a | 0x082a | 0x084a => Self::Eraser,
            0x0812 | 0x0832 => Self::Brush,
            0x0813 | 0x0833 => Self::Pencil,
            0x0814 | 0x0834 => Self::Airbrush,
            0x0021 | 0x0061 | 0x0017 => Self::Mouse,
            0x0096 | 0x0097 => Self::Lens,
            _ => Self::Unknown,
        }
    }
}

/// Stylus button state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StylusButtons {
    /// Primary button (tip switch)
    pub tip: bool,
    /// Secondary button (barrel button 1)
    pub barrel1: bool,
    /// Tertiary button (barrel button 2)
    pub barrel2: bool,
    /// Eraser button
    pub eraser: bool,
    /// In-range indicator
    pub in_range: bool,
    /// Inverted (eraser end)
    pub inverted: bool,
    /// Touch contact
    pub touch: bool,
}

impl StylusButtons {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn any_pressed(&self) -> bool {
        self.tip || self.barrel1 || self.barrel2 || self.eraser
    }
}

/// Stylus event type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StylusEventType {
    /// Pen entered proximity
    ProximityIn,
    /// Pen left proximity
    ProximityOut,
    /// Pen tip down (contact)
    PenDown,
    /// Pen tip up (no contact)
    PenUp,
    /// Pen moved
    Move,
    /// Button pressed
    ButtonDown,
    /// Button released
    ButtonUp,
    /// Tool changed (e.g., pen to eraser)
    ToolChange,
    /// Pressure changed
    PressureChange,
    /// Tilt changed
    TiltChange,
}

/// Stylus input event
#[derive(Debug, Clone, Copy)]
pub struct StylusEvent {
    /// Event type
    pub event_type: StylusEventType,
    /// X coordinate (absolute)
    pub x: i32,
    /// Y coordinate (absolute)
    pub y: i32,
    /// Pressure (0-4096 typically, 0 = no contact)
    pub pressure: u16,
    /// X tilt (-90 to 90 degrees)
    pub tilt_x: i16,
    /// Y tilt (-90 to 90 degrees)
    pub tilt_y: i16,
    /// Rotation angle (0-3600 in 0.1 degree units)
    pub rotation: u16,
    /// Distance from surface (when in proximity but not touching)
    pub distance: u16,
    /// Current tool type
    pub tool_type: StylusToolType,
    /// Button state
    pub buttons: StylusButtons,
    /// Serial number of tool (for multi-tool identification)
    pub tool_serial: u32,
    /// Timestamp in milliseconds
    pub timestamp: u64,
}

impl StylusEvent {
    pub fn new(event_type: StylusEventType) -> Self {
        Self {
            event_type,
            x: 0,
            y: 0,
            pressure: 0,
            tilt_x: 0,
            tilt_y: 0,
            rotation: 0,
            distance: 0,
            tool_type: StylusToolType::Pen,
            buttons: StylusButtons::new(),
            tool_serial: 0,
            timestamp: crate::time::uptime_ms(),
        }
    }
}

/// Stylus device capabilities
#[derive(Debug, Clone, Copy)]
pub struct StylusCapabilities {
    /// Maximum X coordinate
    pub x_max: i32,
    /// Maximum Y coordinate
    pub y_max: i32,
    /// Resolution in points per inch (X)
    pub x_resolution: u32,
    /// Resolution in points per inch (Y)
    pub y_resolution: u32,
    /// Maximum pressure value
    pub pressure_max: u16,
    /// Pressure levels (e.g., 2048, 4096, 8192)
    pub pressure_levels: u16,
    /// Maximum tilt angle
    pub tilt_max: i16,
    /// Has pressure support
    pub has_pressure: bool,
    /// Has tilt support
    pub has_tilt: bool,
    /// Has rotation support
    pub has_rotation: bool,
    /// Has distance/hover support
    pub has_distance: bool,
    /// Has eraser support
    pub has_eraser: bool,
    /// Has barrel button 1
    pub has_barrel1: bool,
    /// Has barrel button 2
    pub has_barrel2: bool,
    /// Number of supported tools
    pub num_tools: u8,
    /// Supports tool serial identification
    pub has_tool_serial: bool,
}

impl StylusCapabilities {
    pub fn default_tablet() -> Self {
        Self {
            x_max: 21600,
            y_max: 13500,
            x_resolution: 2540,
            y_resolution: 2540,
            pressure_max: 2048,
            pressure_levels: 2048,
            tilt_max: 60,
            has_pressure: true,
            has_tilt: true,
            has_rotation: false,
            has_distance: true,
            has_eraser: true,
            has_barrel1: true,
            has_barrel2: true,
            num_tools: 2,
            has_tool_serial: true,
        }
    }

    pub fn simple_pen() -> Self {
        Self {
            x_max: 32767,
            y_max: 32767,
            x_resolution: 100,
            y_resolution: 100,
            pressure_max: 1024,
            pressure_levels: 1024,
            tilt_max: 0,
            has_pressure: true,
            has_tilt: false,
            has_rotation: false,
            has_distance: false,
            has_eraser: false,
            has_barrel1: true,
            has_barrel2: false,
            num_tools: 1,
            has_tool_serial: false,
        }
    }

    pub fn surface_pen() -> Self {
        Self {
            x_max: 32767,
            y_max: 32767,
            x_resolution: 2540,
            y_resolution: 2540,
            pressure_max: 4096,
            pressure_levels: 4096,
            tilt_max: 60,
            has_pressure: true,
            has_tilt: true,
            has_rotation: false,
            has_distance: true,
            has_eraser: true,
            has_barrel1: true,
            has_barrel2: false,
            num_tools: 1,
            has_tool_serial: false,
        }
    }

    pub fn wacom_intuos() -> Self {
        Self {
            x_max: 21600,
            y_max: 13500,
            x_resolution: 2540,
            y_resolution: 2540,
            pressure_max: 8192,
            pressure_levels: 8192,
            tilt_max: 60,
            has_pressure: true,
            has_tilt: true,
            has_rotation: true,
            has_distance: true,
            has_eraser: true,
            has_barrel1: true,
            has_barrel2: true,
            num_tools: 16,
            has_tool_serial: true,
        }
    }
}

/// Stylus interface type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StylusInterface {
    /// USB HID interface
    UsbHid,
    /// Bluetooth
    Bluetooth,
    /// I2C HID
    I2cHid,
    /// Serial (legacy Wacom)
    Serial,
    /// Integrated digitizer (e.g., Surface)
    Integrated,
    /// Unknown
    Unknown,
}

impl StylusInterface {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UsbHid => "USB HID",
            Self::Bluetooth => "Bluetooth",
            Self::I2cHid => "I2C HID",
            Self::Serial => "Serial",
            Self::Integrated => "Integrated",
            Self::Unknown => "Unknown",
        }
    }
}

/// Stylus device vendor
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StylusVendor {
    Wacom,
    Microsoft,
    Huion,
    XpPen,
    Gaomon,
    Ugee,
    Veikk,
    Apple,
    Samsung,
    Lenovo,
    Dell,
    Hp,
    Unknown,
}

impl StylusVendor {
    pub fn from_vendor_id(vendor_id: u16) -> Self {
        match vendor_id {
            0x056a => Self::Wacom,
            0x045e => Self::Microsoft,
            0x256c => Self::Huion,
            0x28bd => Self::XpPen,
            0x256d => Self::Gaomon,
            0x28b3 => Self::Ugee,
            0x2feb => Self::Veikk,
            0x05ac => Self::Apple,
            0x04e8 => Self::Samsung,
            0x17ef => Self::Lenovo,
            0x413c => Self::Dell,
            0x03f0 => Self::Hp,
            _ => Self::Unknown,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Wacom => "Wacom",
            Self::Microsoft => "Microsoft",
            Self::Huion => "Huion",
            Self::XpPen => "XP-Pen",
            Self::Gaomon => "Gaomon",
            Self::Ugee => "Ugee",
            Self::Veikk => "Veikk",
            Self::Apple => "Apple",
            Self::Samsung => "Samsung",
            Self::Lenovo => "Lenovo",
            Self::Dell => "Dell",
            Self::Hp => "HP",
            Self::Unknown => "Unknown",
        }
    }
}

/// Stylus device configuration
#[derive(Debug, Clone)]
pub struct StylusConfig {
    /// Pressure curve (0-100, 50 = linear)
    pub pressure_curve: u8,
    /// Pressure threshold for touch
    pub pressure_threshold: u16,
    /// Raw pressure multiplier (percentage)
    pub pressure_sensitivity: u8,
    /// Tilt sensitivity (percentage)
    pub tilt_sensitivity: u8,
    /// Enable palm rejection
    pub palm_rejection: bool,
    /// Mapping mode
    pub mapping_mode: MappingMode,
    /// Map to specific display (None = full virtual desktop)
    pub map_to_display: Option<u32>,
    /// Aspect ratio correction
    pub aspect_ratio_correction: bool,
    /// Active area left (percentage)
    pub active_area_left: u8,
    /// Active area top (percentage)
    pub active_area_top: u8,
    /// Active area right (percentage)
    pub active_area_right: u8,
    /// Active area bottom (percentage)
    pub active_area_bottom: u8,
}

impl StylusConfig {
    pub fn default() -> Self {
        Self {
            pressure_curve: 50,
            pressure_threshold: 5,
            pressure_sensitivity: 100,
            tilt_sensitivity: 100,
            palm_rejection: true,
            mapping_mode: MappingMode::Absolute,
            map_to_display: None,
            aspect_ratio_correction: true,
            active_area_left: 0,
            active_area_top: 0,
            active_area_right: 100,
            active_area_bottom: 100,
        }
    }
}

/// Stylus mapping mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappingMode {
    /// Absolute positioning
    Absolute,
    /// Relative (mouse-like)
    Relative,
    /// Pen mode (for drawing)
    Pen,
}

/// Stylus device state
#[derive(Debug, Clone, Copy)]
pub struct StylusState {
    /// Current X position
    pub x: i32,
    /// Current Y position
    pub y: i32,
    /// Current pressure
    pub pressure: u16,
    /// Current tilt X
    pub tilt_x: i16,
    /// Current tilt Y
    pub tilt_y: i16,
    /// Current rotation
    pub rotation: u16,
    /// Current distance
    pub distance: u16,
    /// Current tool type
    pub tool_type: StylusToolType,
    /// Current buttons
    pub buttons: StylusButtons,
    /// In proximity
    pub in_proximity: bool,
    /// In contact (touching surface)
    pub in_contact: bool,
    /// Tool serial
    pub tool_serial: u32,
}

impl StylusState {
    pub fn new() -> Self {
        Self {
            x: 0,
            y: 0,
            pressure: 0,
            tilt_x: 0,
            tilt_y: 0,
            rotation: 0,
            distance: 0,
            tool_type: StylusToolType::Unknown,
            buttons: StylusButtons::new(),
            in_proximity: false,
            in_contact: false,
            tool_serial: 0,
        }
    }
}

/// Statistics for stylus device
#[derive(Debug, Clone, Copy, Default)]
pub struct StylusStats {
    /// Total events processed
    pub events_processed: u64,
    /// Total strokes (pen down to pen up)
    pub strokes: u64,
    /// Total distance traveled (in device units)
    pub distance_traveled: u64,
    /// Peak pressure recorded
    pub peak_pressure: u16,
    /// Average pressure during contact
    pub avg_pressure: u16,
    /// Time in contact (milliseconds)
    pub contact_time_ms: u64,
    /// Tool changes
    pub tool_changes: u64,
    /// Last event timestamp
    pub last_event_time: u64,
}

/// Stylus device
#[derive(Debug)]
pub struct StylusDevice {
    /// Device ID
    pub id: u32,
    /// Vendor ID
    pub vendor_id: u16,
    /// Product ID
    pub product_id: u16,
    /// Device name
    pub name: String,
    /// Vendor
    pub vendor: StylusVendor,
    /// Interface type
    pub interface: StylusInterface,
    /// Capabilities
    pub capabilities: StylusCapabilities,
    /// Configuration
    pub config: StylusConfig,
    /// Current state
    pub state: StylusState,
    /// Statistics
    pub stats: StylusStats,
    /// Is initialized
    pub initialized: bool,
    /// Stroke start time (for calculating stroke duration)
    stroke_start_time: u64,
    /// Last position (for distance calculation)
    last_x: i32,
    last_y: i32,
    /// Pressure accumulator for average
    pressure_sum: u64,
    pressure_count: u64,
}

impl StylusDevice {
    pub fn new(id: u32, vendor_id: u16, product_id: u16, name: String) -> Self {
        let vendor = StylusVendor::from_vendor_id(vendor_id);
        let capabilities = Self::detect_capabilities(vendor_id, product_id);

        Self {
            id,
            vendor_id,
            product_id,
            name,
            vendor,
            interface: StylusInterface::Unknown,
            capabilities,
            config: StylusConfig::default(),
            state: StylusState::new(),
            stats: StylusStats::default(),
            initialized: false,
            stroke_start_time: 0,
            last_x: 0,
            last_y: 0,
            pressure_sum: 0,
            pressure_count: 0,
        }
    }

    fn detect_capabilities(vendor_id: u16, product_id: u16) -> StylusCapabilities {
        // Detect capabilities based on known devices
        match vendor_id {
            0x056a => {
                // Wacom
                match product_id {
                    0x0357..=0x0359 => StylusCapabilities::wacom_intuos(), // Intuos Pro
                    0x0300..=0x030F => StylusCapabilities::wacom_intuos(), // Intuos
                    _ => StylusCapabilities::default_tablet(),
                }
            }
            0x045e => {
                // Microsoft (Surface Pen)
                StylusCapabilities::surface_pen()
            }
            0x256c | 0x28bd | 0x256d | 0x28b3 | 0x2feb => {
                // Huion, XP-Pen, Gaomon, Ugee, Veikk
                StylusCapabilities::default_tablet()
            }
            _ => StylusCapabilities::simple_pen(),
        }
    }

    /// Apply pressure curve
    fn apply_pressure_curve(&self, raw_pressure: u16) -> u16 {
        let curve = self.config.pressure_curve as f32 / 50.0;
        let normalized = raw_pressure as f32 / self.capabilities.pressure_max as f32;

        let curved = if curve < 1.0 {
            // Soft curve (more pressure needed)
            pow_f32(normalized, 2.0 - curve)
        } else if curve > 1.0 {
            // Hard curve (less pressure needed)
            pow_f32(normalized, 1.0 / curve)
        } else {
            // Linear
            normalized
        };

        let scaled = curved * self.capabilities.pressure_max as f32;
        let adjusted = scaled * (self.config.pressure_sensitivity as f32 / 100.0);

        (adjusted as u16).min(self.capabilities.pressure_max)
    }

    /// Map coordinates to display space
    pub fn map_to_display(&self, x: i32, y: i32, display_width: u32, display_height: u32) -> (i32, i32) {
        // Apply active area
        let area_left = (self.capabilities.x_max as f32 * self.config.active_area_left as f32 / 100.0) as i32;
        let area_right = (self.capabilities.x_max as f32 * self.config.active_area_right as f32 / 100.0) as i32;
        let area_top = (self.capabilities.y_max as f32 * self.config.active_area_top as f32 / 100.0) as i32;
        let area_bottom = (self.capabilities.y_max as f32 * self.config.active_area_bottom as f32 / 100.0) as i32;

        let area_width = (area_right - area_left).max(1);
        let area_height = (area_bottom - area_top).max(1);

        // Clamp to active area
        let clamped_x = (x - area_left).clamp(0, area_width);
        let clamped_y = (y - area_top).clamp(0, area_height);

        // Map to display
        let mut mapped_x = (clamped_x as f32 / area_width as f32 * display_width as f32) as i32;
        let mut mapped_y = (clamped_y as f32 / area_height as f32 * display_height as f32) as i32;

        // Apply aspect ratio correction if enabled
        if self.config.aspect_ratio_correction {
            let tablet_ratio = area_width as f32 / area_height as f32;
            let display_ratio = display_width as f32 / display_height as f32;

            if tablet_ratio > display_ratio {
                // Tablet is wider, adjust X
                let scale = display_ratio / tablet_ratio;
                let offset = (display_width as f32 * (1.0 - scale) / 2.0) as i32;
                mapped_x = (mapped_x as f32 * scale) as i32 + offset;
            } else {
                // Tablet is taller, adjust Y
                let scale = tablet_ratio / display_ratio;
                let offset = (display_height as f32 * (1.0 - scale) / 2.0) as i32;
                mapped_y = (mapped_y as f32 * scale) as i32 + offset;
            }
        }

        (mapped_x, mapped_y)
    }

    /// Process raw input data
    pub fn process_input(&mut self, data: &StylusRawInput) -> Option<StylusEvent> {
        let now = crate::time::uptime_ms();
        let prev_state = self.state;

        // Update state
        self.state.x = data.x;
        self.state.y = data.y;
        self.state.pressure = self.apply_pressure_curve(data.pressure);
        self.state.tilt_x = data.tilt_x;
        self.state.tilt_y = data.tilt_y;
        self.state.rotation = data.rotation;
        self.state.distance = data.distance;
        self.state.buttons = data.buttons;
        self.state.tool_type = data.tool_type;
        self.state.tool_serial = data.tool_serial;

        // Determine event type
        let event_type = if data.in_range && !prev_state.in_proximity {
            self.state.in_proximity = true;
            Some(StylusEventType::ProximityIn)
        } else if !data.in_range && prev_state.in_proximity {
            self.state.in_proximity = false;
            self.state.in_contact = false;
            Some(StylusEventType::ProximityOut)
        } else if self.state.pressure > self.config.pressure_threshold && !prev_state.in_contact {
            self.state.in_contact = true;
            self.stroke_start_time = now;
            self.stats.strokes += 1;
            Some(StylusEventType::PenDown)
        } else if self.state.pressure <= self.config.pressure_threshold && prev_state.in_contact {
            self.state.in_contact = false;
            // Update contact time
            self.stats.contact_time_ms += now - self.stroke_start_time;
            // Calculate average pressure
            if self.pressure_count > 0 {
                self.stats.avg_pressure = (self.pressure_sum / self.pressure_count) as u16;
            }
            self.pressure_sum = 0;
            self.pressure_count = 0;
            Some(StylusEventType::PenUp)
        } else if data.tool_type != prev_state.tool_type {
            self.stats.tool_changes += 1;
            Some(StylusEventType::ToolChange)
        } else if self.state.in_proximity {
            // Check for button changes
            if data.buttons.barrel1 != prev_state.buttons.barrel1 ||
               data.buttons.barrel2 != prev_state.buttons.barrel2 {
                if data.buttons.barrel1 || data.buttons.barrel2 {
                    Some(StylusEventType::ButtonDown)
                } else {
                    Some(StylusEventType::ButtonUp)
                }
            } else if self.state.x != prev_state.x || self.state.y != prev_state.y {
                Some(StylusEventType::Move)
            } else if self.state.pressure != prev_state.pressure {
                Some(StylusEventType::PressureChange)
            } else if self.state.tilt_x != prev_state.tilt_x || self.state.tilt_y != prev_state.tilt_y {
                Some(StylusEventType::TiltChange)
            } else {
                None
            }
        } else {
            None
        };

        // Update statistics
        if let Some(_) = event_type {
            self.stats.events_processed += 1;
            self.stats.last_event_time = now;

            // Track peak pressure
            if self.state.pressure > self.stats.peak_pressure {
                self.stats.peak_pressure = self.state.pressure;
            }

            // Accumulate for average pressure during contact
            if self.state.in_contact {
                self.pressure_sum += self.state.pressure as u64;
                self.pressure_count += 1;
            }

            // Calculate distance traveled
            if self.state.in_contact {
                let dx = (self.state.x - self.last_x) as i64;
                let dy = (self.state.y - self.last_y) as i64;
                let dist = sqrt_u64((dx * dx + dy * dy) as u64);
                self.stats.distance_traveled += dist;
            }

            self.last_x = self.state.x;
            self.last_y = self.state.y;
        }

        event_type.map(|et| StylusEvent {
            event_type: et,
            x: self.state.x,
            y: self.state.y,
            pressure: self.state.pressure,
            tilt_x: self.state.tilt_x,
            tilt_y: self.state.tilt_y,
            rotation: self.state.rotation,
            distance: self.state.distance,
            tool_type: self.state.tool_type,
            buttons: self.state.buttons,
            tool_serial: self.state.tool_serial,
            timestamp: now,
        })
    }
}

/// Raw input from device
#[derive(Debug, Clone, Copy)]
pub struct StylusRawInput {
    pub x: i32,
    pub y: i32,
    pub pressure: u16,
    pub tilt_x: i16,
    pub tilt_y: i16,
    pub rotation: u16,
    pub distance: u16,
    pub tool_type: StylusToolType,
    pub buttons: StylusButtons,
    pub tool_serial: u32,
    pub in_range: bool,
}

impl StylusRawInput {
    pub fn new() -> Self {
        Self {
            x: 0,
            y: 0,
            pressure: 0,
            tilt_x: 0,
            tilt_y: 0,
            rotation: 0,
            distance: 0,
            tool_type: StylusToolType::Unknown,
            buttons: StylusButtons::new(),
            tool_serial: 0,
            in_range: false,
        }
    }
}

/// Callback type for stylus events
pub type StylusEventCallback = fn(StylusEvent);

/// Stylus manager state
struct StylusManagerState {
    /// Registered devices
    devices: BTreeMap<u32, StylusDevice>,
    /// Next device ID
    next_device_id: u32,
    /// Event callback
    on_event: Option<StylusEventCallback>,
    /// Active device ID
    active_device: Option<u32>,
    /// Initialized
    initialized: bool,
}

impl StylusManagerState {
    const fn new() -> Self {
        Self {
            devices: BTreeMap::new(),
            next_device_id: 1,
            on_event: None,
            active_device: None,
            initialized: false,
        }
    }
}

/// Global stylus manager
static STYLUS_MANAGER: IrqSafeMutex<StylusManagerState> = IrqSafeMutex::new(StylusManagerState::new());

/// Stylus manager
pub struct StylusManager;

impl StylusManager {
    /// Initialize the stylus manager
    pub fn init() {
        let mut state = STYLUS_MANAGER.lock();
        if state.initialized {
            return;
        }
        state.initialized = true;
        crate::kprintln!("[stylus] Stylus manager initialized");
    }

    /// Register a new stylus device
    pub fn register_device(vendor_id: u16, product_id: u16, name: &str, interface: StylusInterface) -> u32 {
        let mut state = STYLUS_MANAGER.lock();

        let id = state.next_device_id;
        state.next_device_id += 1;

        let mut device = StylusDevice::new(id, vendor_id, product_id, String::from(name));
        device.interface = interface;
        device.initialized = true;

        crate::kprintln!(
            "[stylus] Registered device {}: {} ({}) via {}",
            id,
            name,
            device.vendor.as_str(),
            interface.as_str()
        );

        state.devices.insert(id, device);

        // Set as active if first device
        if state.active_device.is_none() {
            state.active_device = Some(id);
        }

        id
    }

    /// Unregister a device
    pub fn unregister_device(device_id: u32) -> bool {
        let mut state = STYLUS_MANAGER.lock();

        if state.devices.remove(&device_id).is_some() {
            if state.active_device == Some(device_id) {
                state.active_device = state.devices.keys().next().copied();
            }
            crate::kprintln!("[stylus] Unregistered device {}", device_id);
            true
        } else {
            false
        }
    }

    /// Process raw input for a device
    pub fn process_input(device_id: u32, input: StylusRawInput) -> Option<StylusEvent> {
        let mut state = STYLUS_MANAGER.lock();

        if let Some(device) = state.devices.get_mut(&device_id) {
            let event = device.process_input(&input);

            if let Some(ref evt) = event {
                if let Some(callback) = state.on_event {
                    callback(*evt);
                }
            }

            event
        } else {
            None
        }
    }

    /// Set event callback
    pub fn set_event_callback(callback: StylusEventCallback) {
        let mut state = STYLUS_MANAGER.lock();
        state.on_event = Some(callback);
    }

    /// Get device info
    pub fn get_device(device_id: u32) -> Option<(String, StylusVendor, StylusCapabilities)> {
        let state = STYLUS_MANAGER.lock();
        state.devices.get(&device_id).map(|d| (d.name.clone(), d.vendor, d.capabilities))
    }

    /// Get device state
    pub fn get_device_state(device_id: u32) -> Option<StylusState> {
        let state = STYLUS_MANAGER.lock();
        state.devices.get(&device_id).map(|d| d.state)
    }

    /// Get device stats
    pub fn get_device_stats(device_id: u32) -> Option<StylusStats> {
        let state = STYLUS_MANAGER.lock();
        state.devices.get(&device_id).map(|d| d.stats)
    }

    /// List all devices
    pub fn list_devices() -> Vec<(u32, String, StylusVendor)> {
        let state = STYLUS_MANAGER.lock();
        state.devices.iter()
            .map(|(&id, d)| (id, d.name.clone(), d.vendor))
            .collect()
    }

    /// Set device configuration
    pub fn configure_device(device_id: u32, config: StylusConfig) -> bool {
        let mut state = STYLUS_MANAGER.lock();
        if let Some(device) = state.devices.get_mut(&device_id) {
            device.config = config;
            true
        } else {
            false
        }
    }

    /// Get device configuration
    pub fn get_config(device_id: u32) -> Option<StylusConfig> {
        let state = STYLUS_MANAGER.lock();
        state.devices.get(&device_id).map(|d| d.config.clone())
    }

    /// Set pressure curve (0-100, 50 = linear)
    pub fn set_pressure_curve(device_id: u32, curve: u8) -> bool {
        let mut state = STYLUS_MANAGER.lock();
        if let Some(device) = state.devices.get_mut(&device_id) {
            device.config.pressure_curve = curve.min(100);
            true
        } else {
            false
        }
    }

    /// Set active area (percentages)
    pub fn set_active_area(device_id: u32, left: u8, top: u8, right: u8, bottom: u8) -> bool {
        let mut state = STYLUS_MANAGER.lock();
        if let Some(device) = state.devices.get_mut(&device_id) {
            device.config.active_area_left = left.min(100);
            device.config.active_area_top = top.min(100);
            device.config.active_area_right = right.min(100).max(device.config.active_area_left);
            device.config.active_area_bottom = bottom.min(100).max(device.config.active_area_top);
            true
        } else {
            false
        }
    }

    /// Set mapping mode
    pub fn set_mapping_mode(device_id: u32, mode: MappingMode) -> bool {
        let mut state = STYLUS_MANAGER.lock();
        if let Some(device) = state.devices.get_mut(&device_id) {
            device.config.mapping_mode = mode;
            true
        } else {
            false
        }
    }

    /// Map to specific display
    pub fn map_to_display(device_id: u32, display_id: Option<u32>) -> bool {
        let mut state = STYLUS_MANAGER.lock();
        if let Some(device) = state.devices.get_mut(&device_id) {
            device.config.map_to_display = display_id;
            true
        } else {
            false
        }
    }

    /// Get active device
    pub fn active_device() -> Option<u32> {
        let state = STYLUS_MANAGER.lock();
        state.active_device
    }

    /// Set active device
    pub fn set_active_device(device_id: u32) -> bool {
        let mut state = STYLUS_MANAGER.lock();
        if state.devices.contains_key(&device_id) {
            state.active_device = Some(device_id);
            true
        } else {
            false
        }
    }

    /// Get count of registered devices
    pub fn device_count() -> usize {
        let state = STYLUS_MANAGER.lock();
        state.devices.len()
    }

    /// Check if manager is initialized
    pub fn is_initialized() -> bool {
        let state = STYLUS_MANAGER.lock();
        state.initialized
    }

    /// Format status for display
    pub fn format_status() -> String {
        let state = STYLUS_MANAGER.lock();
        use alloc::fmt::Write;
        let mut s = String::new();

        let _ = writeln!(s, "Stylus Manager Status:");
        let _ = writeln!(s, "  Initialized: {}", state.initialized);
        let _ = writeln!(s, "  Devices: {}", state.devices.len());
        let _ = writeln!(s, "  Active device: {:?}", state.active_device);

        for (id, device) in &state.devices {
            let _ = writeln!(s, "\n  Device {}:", id);
            let _ = writeln!(s, "    Name: {}", device.name);
            let _ = writeln!(s, "    Vendor: {} (0x{:04x})", device.vendor.as_str(), device.vendor_id);
            let _ = writeln!(s, "    Product: 0x{:04x}", device.product_id);
            let _ = writeln!(s, "    Interface: {}", device.interface.as_str());
            let _ = writeln!(s, "    Capabilities:");
            let _ = writeln!(s, "      Resolution: {}x{}", device.capabilities.x_max, device.capabilities.y_max);
            let _ = writeln!(s, "      Pressure levels: {}", device.capabilities.pressure_levels);
            let _ = writeln!(s, "      Has tilt: {}", device.capabilities.has_tilt);
            let _ = writeln!(s, "      Has eraser: {}", device.capabilities.has_eraser);
            let _ = writeln!(s, "    State:");
            let _ = writeln!(s, "      In proximity: {}", device.state.in_proximity);
            let _ = writeln!(s, "      In contact: {}", device.state.in_contact);
            let _ = writeln!(s, "      Tool: {}", device.state.tool_type.as_str());
            let _ = writeln!(s, "      Position: ({}, {})", device.state.x, device.state.y);
            let _ = writeln!(s, "      Pressure: {}", device.state.pressure);
            let _ = writeln!(s, "    Stats:");
            let _ = writeln!(s, "      Events: {}", device.stats.events_processed);
            let _ = writeln!(s, "      Strokes: {}", device.stats.strokes);
            let _ = writeln!(s, "      Peak pressure: {}", device.stats.peak_pressure);
        }

        s
    }
}

// ============================================================================
// Helper functions for no_std math
// ============================================================================

/// Software power function
fn pow_f32(base: f32, exp: f32) -> f32 {
    if exp == 0.0 {
        return 1.0;
    }
    if exp == 1.0 {
        return base;
    }
    if base <= 0.0 {
        return 0.0;
    }

    // Use exp(exp * ln(base)) approximation
    let ln_base = ln_f32(base);
    exp_f32(exp * ln_base)
}

/// Natural logarithm approximation
fn ln_f32(x: f32) -> f32 {
    if x <= 0.0 {
        return -1000.0; // Approximation for negative infinity
    }

    // Use the identity: ln(x) = 2 * artanh((x-1)/(x+1))
    // For small x close to 1, use Taylor series
    let y = (x - 1.0) / (x + 1.0);
    let y2 = y * y;

    // Taylor series: 2 * (y + y^3/3 + y^5/5 + y^7/7 + ...)
    let mut result = y;
    let mut term = y;
    for i in 1..10 {
        term *= y2;
        result += term / (2 * i + 1) as f32;
    }

    2.0 * result
}

/// Exponential function approximation
fn exp_f32(x: f32) -> f32 {
    // Clamp to prevent overflow
    let x = x.clamp(-20.0, 20.0);

    // Taylor series: e^x = 1 + x + x^2/2! + x^3/3! + ...
    let mut result = 1.0;
    let mut term = 1.0;

    for i in 1..20 {
        term *= x / i as f32;
        result += term;
        if term.abs() < 0.0001 {
            break;
        }
    }

    result
}

/// Software square root for u64
fn sqrt_u64(x: u64) -> u64 {
    if x == 0 {
        return 0;
    }

    let mut guess = x;
    let mut result = 0u64;

    // Binary search approach
    let mut bit = 1u64 << 31;
    while bit > x {
        bit >>= 1;
    }

    while bit != 0 {
        if x >= result + bit {
            let new_result = result + bit;
            if new_result <= x / new_result {
                result = new_result;
            }
        }
        bit >>= 1;
    }

    // Newton's method refinement
    if result > 0 {
        guess = result;
        for _ in 0..5 {
            let new_guess = (guess + x / guess) / 2;
            if new_guess >= guess {
                break;
            }
            guess = new_guess;
        }
        result = guess;
    }

    result
}

// ============================================================================
// Public API
// ============================================================================

/// Initialize the stylus subsystem
pub fn init() {
    StylusManager::init();
}

/// Register a stylus device
pub fn register_device(vendor_id: u16, product_id: u16, name: &str, interface: StylusInterface) -> u32 {
    StylusManager::register_device(vendor_id, product_id, name, interface)
}

/// Unregister a stylus device
pub fn unregister_device(device_id: u32) -> bool {
    StylusManager::unregister_device(device_id)
}

/// Process stylus input
pub fn process_input(device_id: u32, input: StylusRawInput) -> Option<StylusEvent> {
    StylusManager::process_input(device_id, input)
}

/// Set event callback
pub fn set_event_callback(callback: StylusEventCallback) {
    StylusManager::set_event_callback(callback);
}

/// List devices
pub fn list_devices() -> Vec<(u32, String, StylusVendor)> {
    StylusManager::list_devices()
}

/// Get status
pub fn status() -> String {
    StylusManager::format_status()
}
