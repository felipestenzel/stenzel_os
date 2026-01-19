//! VirtIO Input Device Driver
//!
//! Provides keyboard, mouse, and tablet input via VirtIO protocol.

#![allow(dead_code)]

use alloc::vec::Vec;
use alloc::vec;
use alloc::string::String;
use alloc::collections::VecDeque;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use super::virtqueue::Virtqueue;
use super::{VirtioDevice, VirtioDeviceType, features};

/// Input configuration select values
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputConfigSelect {
    Unset = 0x00,
    IdName = 0x01,
    IdSerial = 0x02,
    IdDevids = 0x03,
    PropBits = 0x10,
    EvBits = 0x11,
    AbsInfo = 0x12,
}

/// Input device IDs
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct InputDevids {
    pub bustype: u16,
    pub vendor: u16,
    pub product: u16,
    pub version: u16,
}

/// Absolute axis info
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct InputAbsinfo {
    pub min: u32,
    pub max: u32,
    pub fuzz: u32,
    pub flat: u32,
    pub res: u32,
}

/// Input device configuration
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VirtioInputConfig {
    pub select: u8,
    pub subsel: u8,
    pub size: u8,
    pub reserved: [u8; 5],
    pub data: [u8; 128],
}

impl Default for VirtioInputConfig {
    fn default() -> Self {
        Self {
            select: 0,
            subsel: 0,
            size: 0,
            reserved: [0; 5],
            data: [0; 128],
        }
    }
}

/// Input event types (Linux input event codes)
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventType {
    Syn = 0x00,
    Key = 0x01,
    Rel = 0x02,
    Abs = 0x03,
    Msc = 0x04,
    Sw = 0x05,
    Led = 0x11,
    Snd = 0x12,
    Rep = 0x14,
    Ff = 0x15,
    Pwr = 0x16,
    FfStatus = 0x17,
}

/// Relative axis codes
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelCode {
    X = 0x00,
    Y = 0x01,
    Z = 0x02,
    Rx = 0x03,
    Ry = 0x04,
    Rz = 0x05,
    Hwheel = 0x06,
    Dial = 0x07,
    Wheel = 0x08,
    Misc = 0x09,
}

/// Absolute axis codes
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbsCode {
    X = 0x00,
    Y = 0x01,
    Z = 0x02,
    Rx = 0x03,
    Ry = 0x04,
    Rz = 0x05,
    Throttle = 0x06,
    Rudder = 0x07,
    Wheel = 0x08,
    Gas = 0x09,
    Brake = 0x0a,
    Pressure = 0x18,
    MtSlot = 0x2f,
    MtTouchMajor = 0x30,
    MtTouchMinor = 0x31,
    MtPositionX = 0x35,
    MtPositionY = 0x36,
    MtTrackingId = 0x39,
}

/// Input event
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct VirtioInputEvent {
    pub event_type: u16,
    pub code: u16,
    pub value: u32,
}

/// Input device type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputDeviceType {
    Keyboard,
    Mouse,
    Tablet,
    Touchscreen,
    Unknown,
}

/// Processed input event
#[derive(Debug, Clone, Copy)]
pub enum InputEvent {
    /// Key press/release (code, pressed)
    Key(u16, bool),
    /// Relative motion (dx, dy)
    MouseMove(i32, i32),
    /// Mouse button (button, pressed)
    MouseButton(u8, bool),
    /// Scroll wheel (delta)
    Scroll(i32),
    /// Absolute position (x, y, max_x, max_y)
    AbsolutePos(u32, u32, u32, u32),
    /// Touch event (slot, x, y, contact)
    Touch(u8, u32, u32, bool),
    /// Sync event
    Sync,
}

/// Input statistics
#[derive(Debug, Default)]
pub struct InputStats {
    pub events_received: AtomicU64,
    pub key_events: AtomicU64,
    pub motion_events: AtomicU64,
    pub button_events: AtomicU64,
}

/// VirtIO input device
pub struct VirtioInputDevice {
    /// Device configuration
    config: VirtioInputConfig,
    /// Event queue
    event_queue: Virtqueue,
    /// Status queue
    status_queue: Virtqueue,
    /// Negotiated features
    features: u64,
    /// Initialized
    initialized: AtomicBool,
    /// Device type
    device_type: InputDeviceType,
    /// Device name
    name: String,
    /// Device IDs
    devids: InputDevids,
    /// Event buffer
    event_buffer: VecDeque<InputEvent>,
    /// Absolute info (for tablet/touch)
    abs_info: [InputAbsinfo; 64],
    /// Current mouse position (relative mode)
    mouse_x: i32,
    mouse_y: i32,
    /// Current touch state
    touch_slots: [TouchSlot; 10],
    /// Statistics
    stats: InputStats,
}

/// Touch slot state
#[derive(Debug, Clone, Copy, Default)]
struct TouchSlot {
    tracking_id: i32,
    x: u32,
    y: u32,
    active: bool,
}

impl VirtioInputDevice {
    /// Create new input device
    pub fn new(queue_size: u16) -> Self {
        Self {
            config: VirtioInputConfig::default(),
            event_queue: Virtqueue::new(0, queue_size),
            status_queue: Virtqueue::new(1, queue_size),
            features: 0,
            initialized: AtomicBool::new(false),
            device_type: InputDeviceType::Unknown,
            name: String::new(),
            devids: InputDevids::default(),
            event_buffer: VecDeque::with_capacity(256),
            abs_info: [InputAbsinfo::default(); 64],
            mouse_x: 0,
            mouse_y: 0,
            touch_slots: [TouchSlot::default(); 10],
            stats: InputStats::default(),
        }
    }

    /// Get device type
    pub fn device_type(&self) -> InputDeviceType {
        self.device_type
    }

    /// Get device name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Check if device has pending events
    pub fn has_events(&self) -> bool {
        !self.event_buffer.is_empty()
    }

    /// Get next event
    pub fn next_event(&mut self) -> Option<InputEvent> {
        self.event_buffer.pop_front()
    }

    /// Process raw input event
    fn process_event(&mut self, event: &VirtioInputEvent) {
        self.stats.events_received.fetch_add(1, Ordering::Relaxed);

        match event.event_type {
            x if x == EventType::Syn as u16 => {
                self.event_buffer.push_back(InputEvent::Sync);
            }
            x if x == EventType::Key as u16 => {
                self.stats.key_events.fetch_add(1, Ordering::Relaxed);
                let pressed = event.value != 0;

                // Check if it's a mouse button
                if event.code >= 0x110 && event.code <= 0x117 {
                    let button = (event.code - 0x110) as u8;
                    self.stats.button_events.fetch_add(1, Ordering::Relaxed);
                    self.event_buffer.push_back(InputEvent::MouseButton(button, pressed));
                } else {
                    self.event_buffer.push_back(InputEvent::Key(event.code, pressed));
                }
            }
            x if x == EventType::Rel as u16 => {
                self.stats.motion_events.fetch_add(1, Ordering::Relaxed);
                let value = event.value as i32;

                match event.code {
                    x if x == RelCode::X as u16 => {
                        self.mouse_x += value;
                        self.event_buffer.push_back(InputEvent::MouseMove(value, 0));
                    }
                    x if x == RelCode::Y as u16 => {
                        self.mouse_y += value;
                        self.event_buffer.push_back(InputEvent::MouseMove(0, value));
                    }
                    x if x == RelCode::Wheel as u16 => {
                        self.event_buffer.push_back(InputEvent::Scroll(value));
                    }
                    _ => {}
                }
            }
            x if x == EventType::Abs as u16 => {
                self.stats.motion_events.fetch_add(1, Ordering::Relaxed);

                match event.code {
                    x if x == AbsCode::X as u16 => {
                        let info = &self.abs_info[AbsCode::X as usize];
                        self.event_buffer.push_back(InputEvent::AbsolutePos(
                            event.value,
                            0,
                            info.max,
                            0,
                        ));
                    }
                    x if x == AbsCode::Y as u16 => {
                        let info = &self.abs_info[AbsCode::Y as usize];
                        self.event_buffer.push_back(InputEvent::AbsolutePos(
                            0,
                            event.value,
                            0,
                            info.max,
                        ));
                    }
                    x if x == AbsCode::MtSlot as u16 => {
                        // Multi-touch slot selection
                    }
                    x if x == AbsCode::MtPositionX as u16 => {
                        // Multi-touch X position
                    }
                    x if x == AbsCode::MtPositionY as u16 => {
                        // Multi-touch Y position
                    }
                    x if x == AbsCode::MtTrackingId as u16 => {
                        // Multi-touch tracking ID (-1 = lift)
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    /// Get absolute axis info
    pub fn get_abs_info(&self, axis: u16) -> Option<&InputAbsinfo> {
        self.abs_info.get(axis as usize)
    }

    /// Get statistics
    pub fn stats(&self) -> &InputStats {
        &self.stats
    }

    /// Format status
    pub fn format_status(&self) -> String {
        alloc::format!(
            "VirtIO Input: {} ({:?})",
            self.name, self.device_type
        )
    }
}

impl VirtioDevice for VirtioInputDevice {
    fn device_type(&self) -> VirtioDeviceType {
        VirtioDeviceType::Input
    }

    fn init(&mut self) -> Result<(), &'static str> {
        // Query device name
        self.config.select = InputConfigSelect::IdName as u8;
        // In real implementation, read config space
        self.name = String::from("VirtIO Input Device");

        // Query device IDs
        self.config.select = InputConfigSelect::IdDevids as u8;
        // In real implementation, read device IDs

        // Determine device type from capabilities
        self.config.select = InputConfigSelect::EvBits as u8;
        self.config.subsel = EventType::Rel as u8;
        // Check if device has relative axes (mouse)
        // For now, default to keyboard
        self.device_type = InputDeviceType::Keyboard;

        // Query absolute axis info if tablet/touch
        if self.device_type == InputDeviceType::Tablet || self.device_type == InputDeviceType::Touchscreen {
            self.config.select = InputConfigSelect::AbsInfo as u8;
            for axis in 0..64u8 {
                self.config.subsel = axis;
                // Read abs info from config
            }
        }

        // Populate event queue with buffers
        for _ in 0..16 {
            let buffer_size = core::mem::size_of::<VirtioInputEvent>() as u32;
            // In real implementation, allocate DMA buffer
            let _ = self.event_queue.add_buffer(0, buffer_size, true);
        }

        Ok(())
    }

    fn reset(&mut self) {
        self.initialized.store(false, Ordering::Release);
        self.event_buffer.clear();
        self.event_queue = Virtqueue::new(0, self.event_queue.size);
        self.status_queue = Virtqueue::new(1, self.status_queue.size);
    }

    fn negotiate_features(&mut self, offered: u64) -> u64 {
        let wanted = features::VIRTIO_F_VERSION_1;
        self.features = wanted & offered;
        self.features
    }

    fn activate(&mut self) -> Result<(), &'static str> {
        self.initialized.store(true, Ordering::Release);
        crate::kprintln!("virtio-input: Activated, type={:?}", self.device_type);
        Ok(())
    }

    fn handle_interrupt(&mut self) {
        // Process events from queue
        while let Some((desc_id, len)) = self.event_queue.get_used() {
            // In real implementation:
            // 1. Get buffer from descriptor
            // 2. Parse as VirtioInputEvent
            // 3. Process event

            let _ = (desc_id, len);

            // Placeholder: create dummy event
            let event = VirtioInputEvent::default();
            self.process_event(&event);

            // Replenish buffer
            let buffer_size = core::mem::size_of::<VirtioInputEvent>() as u32;
            let _ = self.event_queue.add_buffer(0, buffer_size, true);
        }
    }
}

/// Input device manager
pub struct VirtioInputManager {
    devices: Vec<VirtioInputDevice>,
}

impl VirtioInputManager {
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
        }
    }

    pub fn add_device(&mut self, device: VirtioInputDevice) -> usize {
        let idx = self.devices.len();
        self.devices.push(device);
        idx
    }

    pub fn get_device(&mut self, idx: usize) -> Option<&mut VirtioInputDevice> {
        self.devices.get_mut(idx)
    }

    /// Get keyboard device
    pub fn get_keyboard(&mut self) -> Option<&mut VirtioInputDevice> {
        self.devices.iter_mut()
            .find(|d| d.device_type == InputDeviceType::Keyboard)
    }

    /// Get mouse device
    pub fn get_mouse(&mut self) -> Option<&mut VirtioInputDevice> {
        self.devices.iter_mut()
            .find(|d| d.device_type == InputDeviceType::Mouse)
    }

    /// Get tablet device
    pub fn get_tablet(&mut self) -> Option<&mut VirtioInputDevice> {
        self.devices.iter_mut()
            .find(|d| d.device_type == InputDeviceType::Tablet)
    }

    pub fn device_count(&self) -> usize {
        self.devices.len()
    }

    /// Poll all devices for events
    pub fn poll_events(&mut self) -> Vec<(usize, InputEvent)> {
        let mut events = Vec::new();
        for (idx, device) in self.devices.iter_mut().enumerate() {
            while let Some(event) = device.next_event() {
                events.push((idx, event));
            }
        }
        events
    }
}

impl Default for VirtioInputManager {
    fn default() -> Self {
        Self::new()
    }
}
