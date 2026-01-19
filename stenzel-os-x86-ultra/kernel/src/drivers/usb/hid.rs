//! USB HID (Human Interface Device) driver.
//!
//! Supports USB keyboards and mice in boot protocol mode.

#![allow(dead_code)]

use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

use crate::util::KError;

use super::{
    class, DescriptorType, EndpointDescriptor, EndpointDirection, EndpointType,
    InterfaceDescriptor, SetupPacket, UsbDevice,
};

/// HID Descriptor (follows Interface Descriptor)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct HidDescriptor {
    pub length: u8,
    pub descriptor_type: u8,
    pub hid_version: u16,
    pub country_code: u8,
    pub num_descriptors: u8,
    pub descriptor_type_report: u8,
    pub descriptor_length: u16,
}

/// HID Subclass
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HidSubclass {
    NoSubclass = 0,
    BootInterface = 1,
}

/// HID Protocol (for Boot Interface subclass)
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HidProtocol {
    None = 0,
    Keyboard = 1,
    Mouse = 2,
}

impl From<u8> for HidProtocol {
    fn from(v: u8) -> Self {
        match v {
            1 => HidProtocol::Keyboard,
            2 => HidProtocol::Mouse,
            _ => HidProtocol::None,
        }
    }
}

/// HID Class Requests
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum HidRequest {
    GetReport = 0x01,
    GetIdle = 0x02,
    GetProtocol = 0x03,
    SetReport = 0x09,
    SetIdle = 0x0A,
    SetProtocol = 0x0B,
}

/// Boot Protocol Keyboard Report (8 bytes)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Default)]
pub struct BootKeyboardReport {
    pub modifiers: u8,
    pub reserved: u8,
    pub keys: [u8; 6],
}

impl BootKeyboardReport {
    /// Check if a specific modifier is pressed
    pub fn has_modifier(&self, modifier: KeyModifier) -> bool {
        (self.modifiers & (modifier as u8)) != 0
    }

    /// Get all currently pressed keys (non-zero entries)
    pub fn pressed_keys(&self) -> impl Iterator<Item = u8> + '_ {
        self.keys.iter().copied().filter(|&k| k != 0)
    }
}

/// Keyboard modifier bits
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyModifier {
    LeftCtrl = 0x01,
    LeftShift = 0x02,
    LeftAlt = 0x04,
    LeftGui = 0x08,
    RightCtrl = 0x10,
    RightShift = 0x20,
    RightAlt = 0x40,
    RightGui = 0x80,
}

/// Boot Protocol Mouse Report (3+ bytes)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Default)]
pub struct BootMouseReport {
    pub buttons: u8,
    pub x_delta: i8,
    pub y_delta: i8,
}

impl BootMouseReport {
    pub fn left_button(&self) -> bool {
        (self.buttons & 0x01) != 0
    }

    pub fn right_button(&self) -> bool {
        (self.buttons & 0x02) != 0
    }

    pub fn middle_button(&self) -> bool {
        (self.buttons & 0x04) != 0
    }
}

/// USB Tablet Report (absolute coordinates)
/// QEMU usb-tablet format: buttons (1 byte), x_abs (2 bytes LE), y_abs (2 bytes LE)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Default)]
pub struct TabletReport {
    pub buttons: u8,
    pub x_abs_lo: u8,
    pub x_abs_hi: u8,
    pub y_abs_lo: u8,
    pub y_abs_hi: u8,
}

impl TabletReport {
    pub fn left_button(&self) -> bool {
        (self.buttons & 0x01) != 0
    }

    pub fn right_button(&self) -> bool {
        (self.buttons & 0x02) != 0
    }

    pub fn middle_button(&self) -> bool {
        (self.buttons & 0x04) != 0
    }

    pub fn x_abs(&self) -> u16 {
        (self.x_abs_lo as u16) | ((self.x_abs_hi as u16) << 8)
    }

    pub fn y_abs(&self) -> u16 {
        (self.y_abs_lo as u16) | ((self.y_abs_hi as u16) << 8)
    }
}

/// USB HID Device
#[derive(Debug)]
pub struct HidDevice {
    pub slot_id: u8,
    pub interface_number: u8,
    pub protocol: HidProtocol,
    pub endpoint_number: u8,
    pub endpoint_interval: u8,
    pub max_packet_size: u16,
}

/// USB-HID to PS/2 scancode conversion table (for boot protocol)
/// Only covers basic keys for simplicity
pub fn hid_to_scancode(hid_code: u8) -> Option<u8> {
    match hid_code {
        // Letters A-Z (HID 0x04-0x1D -> ASCII lowercase with scancode offset)
        0x04..=0x1D => {
            // A = 0x04 -> scancode 0x1E
            // B = 0x05 -> scancode 0x30
            // etc. (these mappings are approximate)
            let scancode_map: [u8; 26] = [
                0x1E, 0x30, 0x2E, 0x20, 0x12, 0x21, 0x22, 0x23, 0x17, 0x24, 0x25, 0x26, 0x32, 0x31,
                0x18, 0x19, 0x10, 0x13, 0x1F, 0x14, 0x16, 0x2F, 0x11, 0x2D, 0x15, 0x2C,
            ];
            Some(scancode_map[(hid_code - 0x04) as usize])
        }
        // Numbers 1-9, 0 (HID 0x1E-0x27)
        0x1E => Some(0x02), // 1
        0x1F => Some(0x03), // 2
        0x20 => Some(0x04), // 3
        0x21 => Some(0x05), // 4
        0x22 => Some(0x06), // 5
        0x23 => Some(0x07), // 6
        0x24 => Some(0x08), // 7
        0x25 => Some(0x09), // 8
        0x26 => Some(0x0A), // 9
        0x27 => Some(0x0B), // 0
        // Common keys
        0x28 => Some(0x1C), // Enter
        0x29 => Some(0x01), // Escape
        0x2A => Some(0x0E), // Backspace
        0x2B => Some(0x0F), // Tab
        0x2C => Some(0x39), // Space
        0x2D => Some(0x0C), // Minus
        0x2E => Some(0x0D), // Equals
        0x2F => Some(0x1A), // Left bracket
        0x30 => Some(0x1B), // Right bracket
        0x31 => Some(0x2B), // Backslash
        0x33 => Some(0x27), // Semicolon
        0x34 => Some(0x28), // Apostrophe
        0x35 => Some(0x29), // Grave accent (backtick)
        0x36 => Some(0x33), // Comma
        0x37 => Some(0x34), // Period
        0x38 => Some(0x35), // Slash
        0x39 => Some(0x3A), // Caps Lock
        // Function keys F1-F12
        0x3A => Some(0x3B), // F1
        0x3B => Some(0x3C), // F2
        0x3C => Some(0x3D), // F3
        0x3D => Some(0x3E), // F4
        0x3E => Some(0x3F), // F5
        0x3F => Some(0x40), // F6
        0x40 => Some(0x41), // F7
        0x41 => Some(0x42), // F8
        0x42 => Some(0x43), // F9
        0x43 => Some(0x44), // F10
        0x44 => Some(0x57), // F11
        0x45 => Some(0x58), // F12
        // Navigation
        0x4F => Some(0x4D), // Right arrow
        0x50 => Some(0x4B), // Left arrow
        0x51 => Some(0x50), // Down arrow
        0x52 => Some(0x48), // Up arrow
        // Other
        0x49 => Some(0x52), // Insert
        0x4A => Some(0x47), // Home
        0x4B => Some(0x49), // Page Up
        0x4C => Some(0x53), // Delete
        0x4D => Some(0x4F), // End
        0x4E => Some(0x51), // Page Down
        _ => None,
    }
}

/// HID keyboard state tracker
pub struct HidKeyboard {
    pub device: HidDevice,
    pub last_report: BootKeyboardReport,
}

impl HidKeyboard {
    pub fn new(device: HidDevice) -> Self {
        Self {
            device,
            last_report: BootKeyboardReport::default(),
        }
    }

    /// Process a new keyboard report and return newly pressed keys
    pub fn process_report(&mut self, report: &BootKeyboardReport) -> Vec<u8> {
        let mut new_keys = Vec::new();

        // Find keys in new report that weren't in last report
        for &key in &report.keys {
            if key != 0 && !self.last_report.keys.contains(&key) {
                if let Some(scancode) = hid_to_scancode(key) {
                    new_keys.push(scancode);
                }
            }
        }

        // Check modifier changes
        if report.modifiers != self.last_report.modifiers {
            // Handle modifier key presses (simplified)
            let changed = report.modifiers ^ self.last_report.modifiers;
            if changed & 0x01 != 0 {
                // Left Ctrl
                new_keys.push(if report.modifiers & 0x01 != 0 {
                    0x1D
                } else {
                    0x9D
                });
            }
            if changed & 0x02 != 0 {
                // Left Shift
                new_keys.push(if report.modifiers & 0x02 != 0 {
                    0x2A
                } else {
                    0xAA
                });
            }
            if changed & 0x04 != 0 {
                // Left Alt
                new_keys.push(if report.modifiers & 0x04 != 0 {
                    0x38
                } else {
                    0xB8
                });
            }
        }

        self.last_report = *report;
        new_keys
    }
}

/// HID mouse state tracker
pub struct HidMouse {
    pub device: HidDevice,
    pub last_buttons: u8,
}

impl HidMouse {
    pub fn new(device: HidDevice) -> Self {
        Self {
            device,
            last_buttons: 0,
        }
    }

    /// Process a mouse report
    pub fn process_report(&mut self, report: &BootMouseReport) {
        self.last_buttons = report.buttons;

        // Integrate with mouse driver
        // Note: This would need to integrate with crate::drivers::mouse
    }
}

/// Global HID device list
static HID_DEVICES: Mutex<Vec<HidDevice>> = Mutex::new(Vec::new());
static HID_KEYBOARDS: Mutex<Vec<HidKeyboard>> = Mutex::new(Vec::new());
static HID_MICE: Mutex<Vec<HidMouse>> = Mutex::new(Vec::new());

/// Register a HID device
pub fn register_device(device: HidDevice) {
    let mut devices = HID_DEVICES.lock();

    match device.protocol {
        HidProtocol::Keyboard => {
            crate::kprintln!(
                "usb-hid: keyboard registered (slot {}, endpoint {})",
                device.slot_id,
                device.endpoint_number
            );
            let keyboard = HidKeyboard::new(HidDevice {
                slot_id: device.slot_id,
                interface_number: device.interface_number,
                protocol: device.protocol,
                endpoint_number: device.endpoint_number,
                endpoint_interval: device.endpoint_interval,
                max_packet_size: device.max_packet_size,
            });
            let mut keyboards = HID_KEYBOARDS.lock();
            keyboards.push(keyboard);
        }
        HidProtocol::Mouse => {
            // USB tablets report as HID mice but use absolute positioning
            // Detect tablets by max_packet_size (tablets typically need 5+ bytes)
            let is_tablet = device.max_packet_size >= 5;
            crate::kprintln!(
                "usb-hid: {} registered (slot {}, endpoint {}, max_pkt={})",
                if is_tablet { "tablet" } else { "mouse" },
                device.slot_id,
                device.endpoint_number,
                device.max_packet_size
            );
            let mouse = HidMouse::new(HidDevice {
                slot_id: device.slot_id,
                interface_number: device.interface_number,
                protocol: device.protocol,
                endpoint_number: device.endpoint_number,
                endpoint_interval: device.endpoint_interval,
                max_packet_size: device.max_packet_size,
            });
            let mut mice = HID_MICE.lock();
            mice.push(mouse);
            // Track tablet status
            let mut tablets = MOUSE_IS_TABLET.lock();
            tablets.push(is_tablet);
        }
        HidProtocol::None => {
            // Unknown protocol - try to detect device type from max_packet_size
            // USB tablets (like QEMU's usb-tablet) use protocol 0 with larger packets
            // Tablets need at least 5 bytes: buttons (1) + x_abs (2) + y_abs (2)
            if device.max_packet_size >= 5 {
                crate::kprintln!(
                    "usb-hid: tablet detected via packet size (slot {}, endpoint {}, max_pkt={})",
                    device.slot_id,
                    device.endpoint_number,
                    device.max_packet_size
                );
                let mouse = HidMouse::new(HidDevice {
                    slot_id: device.slot_id,
                    interface_number: device.interface_number,
                    protocol: HidProtocol::Mouse, // Treat as mouse for polling
                    endpoint_number: device.endpoint_number,
                    endpoint_interval: device.endpoint_interval,
                    max_packet_size: device.max_packet_size,
                });
                let mut mice = HID_MICE.lock();
                mice.push(mouse);
                // Mark as tablet for absolute coordinate handling
                let mut tablets = MOUSE_IS_TABLET.lock();
                tablets.push(true);
            } else {
                crate::kprintln!(
                    "usb-hid: unknown HID device (slot {}, endpoint {}, protocol=0, max_pkt={})",
                    device.slot_id,
                    device.endpoint_number,
                    device.max_packet_size
                );
            }
        }
    }

    devices.push(device);
}

/// Check if an interface is HID
pub fn is_hid_interface(iface: &InterfaceDescriptor) -> bool {
    iface.interface_class == class::HID
}

/// Check if an interface is a boot protocol keyboard
pub fn is_boot_keyboard(iface: &InterfaceDescriptor) -> bool {
    iface.interface_class == class::HID
        && iface.interface_subclass == HidSubclass::BootInterface as u8
        && iface.interface_protocol == HidProtocol::Keyboard as u8
}

/// Check if an interface is a boot protocol mouse
pub fn is_boot_mouse(iface: &InterfaceDescriptor) -> bool {
    iface.interface_class == class::HID
        && iface.interface_subclass == HidSubclass::BootInterface as u8
        && iface.interface_protocol == HidProtocol::Mouse as u8
}

/// Create Setup packet for SET_PROTOCOL (boot protocol = 0, report protocol = 1)
pub fn set_protocol_packet(interface: u8, protocol: u8) -> SetupPacket {
    SetupPacket {
        request_type: 0x21, // Host to Device, Class, Interface
        request: HidRequest::SetProtocol as u8,
        value: protocol as u16,
        index: interface as u16,
        length: 0,
    }
}

/// Create Setup packet for SET_IDLE (rate 0 = only report on change)
pub fn set_idle_packet(interface: u8, duration: u8, report_id: u8) -> SetupPacket {
    SetupPacket {
        request_type: 0x21, // Host to Device, Class, Interface
        request: HidRequest::SetIdle as u8,
        value: ((duration as u16) << 8) | (report_id as u16),
        index: interface as u16,
        length: 0,
    }
}

/// Create Setup packet for GET_REPORT
pub fn get_report_packet(interface: u8, report_type: u8, report_id: u8, length: u16) -> SetupPacket {
    SetupPacket {
        request_type: 0xA1, // Device to Host, Class, Interface
        request: HidRequest::GetReport as u8,
        value: ((report_type as u16) << 8) | (report_id as u16),
        index: interface as u16,
        length,
    }
}

/// Keyboard report buffer for each registered keyboard (static allocation)
static KEYBOARD_BUFFERS: Mutex<Vec<[u8; 8]>> = Mutex::new(Vec::new());
/// Pending keyboard polls (slot_id, endpoint_num)
static PENDING_KEYBOARD_POLLS: Mutex<Vec<(u8, u8)>> = Mutex::new(Vec::new());

/// Mouse report buffer for each registered mouse (6 bytes to support tablets too)
static MOUSE_BUFFERS: Mutex<Vec<[u8; 8]>> = Mutex::new(Vec::new());
/// Track which mice are actually tablets (absolute positioning)
static MOUSE_IS_TABLET: Mutex<Vec<bool>> = Mutex::new(Vec::new());
/// Pending mouse polls (slot_id, endpoint_num)
static PENDING_MOUSE_POLLS: Mutex<Vec<(u8, u8)>> = Mutex::new(Vec::new());

/// Track first poll for debug output
static FIRST_POLL: spin::Once<()> = spin::Once::new();

/// Queue interrupt transfers for all keyboards
fn queue_keyboard_polls() {
    if let Some(ctrl_arc) = super::xhci::controller() {
        let mut ctrl = ctrl_arc.lock();
        let keyboards = HID_KEYBOARDS.lock();
        let mut buffers = KEYBOARD_BUFFERS.lock();
        let mut pending = PENDING_KEYBOARD_POLLS.lock();

        // Ensure we have enough buffers
        while buffers.len() < keyboards.len() {
            buffers.push([0u8; 8]);
        }

        // Debug: show that polling is active
        FIRST_POLL.call_once(|| {
            if !keyboards.is_empty() {
                crate::kprintln!("usb-hid: starting keyboard polling for {} keyboard(s)", keyboards.len());
            }
        });

        for (i, kb) in keyboards.iter().enumerate() {
            // Check if we already have a pending poll for this keyboard
            if pending.iter().any(|(s, e)| *s == kb.device.slot_id && *e == kb.device.endpoint_number) {
                continue;
            }

            // Queue an interrupt IN transfer
            if let Err(_e) = ctrl.queue_interrupt_in(
                kb.device.slot_id,
                kb.device.endpoint_number,
                &mut buffers[i],
            ) {
                // Failed to queue, try again later
                continue;
            }

            pending.push((kb.device.slot_id, kb.device.endpoint_number));
        }
    }
}

/// Track first mouse poll for debug output
static FIRST_MOUSE_POLL: spin::Once<()> = spin::Once::new();

/// Queue interrupt transfers for all mice
fn queue_mouse_polls() {
    if let Some(ctrl_arc) = super::xhci::controller() {
        let mut ctrl = ctrl_arc.lock();
        let mice = HID_MICE.lock();
        let mut buffers = MOUSE_BUFFERS.lock();
        let mut pending = PENDING_MOUSE_POLLS.lock();

        // Ensure we have enough buffers (8 bytes each to support tablets)
        while buffers.len() < mice.len() {
            buffers.push([0u8; 8]);
        }

        // Debug: show that polling is active
        FIRST_MOUSE_POLL.call_once(|| {
            if !mice.is_empty() {
                crate::kprintln!("usb-hid: starting mouse polling for {} mouse/mice", mice.len());
            }
        });

        for (i, mouse) in mice.iter().enumerate() {
            // Check if we already have a pending poll for this mouse
            if pending.iter().any(|(s, e)| *s == mouse.device.slot_id && *e == mouse.device.endpoint_number) {
                continue;
            }

            // Queue an interrupt IN transfer
            if let Err(_e) = ctrl.queue_interrupt_in(
                mouse.device.slot_id,
                mouse.device.endpoint_number,
                &mut buffers[i],
            ) {
                continue;
            }

            pending.push((mouse.device.slot_id, mouse.device.endpoint_number));
        }
    }
}

/// Debug: track number of keyboard reports received
static KEYBOARD_REPORT_COUNT: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// Debug: track number of mouse reports received
static MOUSE_REPORT_COUNT: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// Unified poll function that handles both keyboards and mice from the same event ring.
/// This prevents the race condition where poll_keyboards could consume mouse events.
pub fn poll_all() {
    // Queue transfers for all devices
    queue_keyboard_polls();
    queue_mouse_polls();

    // Process all completed transfers from the single event ring
    if let Some(ctrl_arc) = super::xhci::controller() {
        let ctrl = ctrl_arc.lock();

        while let Some((slot_id, ep_id, _residual)) = ctrl.poll_interrupt_transfer() {
            let endpoint_num = if ep_id > 0 { (ep_id - 1) / 2 } else { 0 };

            // Try to match as a keyboard first
            {
                let mut pending = PENDING_KEYBOARD_POLLS.lock();
                if let Some(pos) = pending.iter().position(|(s, e)| *s == slot_id && *e == endpoint_num) {
                    pending.remove(pos);
                    drop(pending);

                    let mut keyboards = HID_KEYBOARDS.lock();
                    let buffers = KEYBOARD_BUFFERS.lock();

                    for (i, kb) in keyboards.iter_mut().enumerate() {
                        if kb.device.slot_id == slot_id && kb.device.endpoint_number == endpoint_num {
                            let report = BootKeyboardReport {
                                modifiers: buffers[i][0],
                                reserved: buffers[i][1],
                                keys: [
                                    buffers[i][2],
                                    buffers[i][3],
                                    buffers[i][4],
                                    buffers[i][5],
                                    buffers[i][6],
                                    buffers[i][7],
                                ],
                            };

                            let count = KEYBOARD_REPORT_COUNT.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
                            if count < 5 {
                                crate::kprintln!(
                                    "usb-hid: keyboard report #{}: mods={:#x} keys=[{:#x},{:#x},{:#x},{:#x},{:#x},{:#x}]",
                                    count + 1,
                                    report.modifiers,
                                    report.keys[0], report.keys[1], report.keys[2],
                                    report.keys[3], report.keys[4], report.keys[5]
                                );
                            }

                            let scancodes = kb.process_report(&report);
                            for scancode in &scancodes {
                                crate::drivers::keyboard::process_scancode(*scancode);
                            }
                            break;
                        }
                    }
                    continue;
                }
            }

            // Try to match as a mouse/tablet
            {
                let mut pending = PENDING_MOUSE_POLLS.lock();
                if let Some(pos) = pending.iter().position(|(s, e)| *s == slot_id && *e == endpoint_num) {
                    pending.remove(pos);
                    drop(pending);

                    let mut mice = HID_MICE.lock();
                    let buffers = MOUSE_BUFFERS.lock();
                    let tablets = MOUSE_IS_TABLET.lock();

                    for (i, mouse) in mice.iter_mut().enumerate() {
                        if mouse.device.slot_id == slot_id && mouse.device.endpoint_number == endpoint_num {
                            let is_tablet = tablets.get(i).copied().unwrap_or(false);
                            let buttons = buffers[i][0];
                            let left = (buttons & 0x01) != 0;
                            let right = (buttons & 0x02) != 0;
                            let middle = (buttons & 0x04) != 0;

                            if is_tablet {
                                // Tablet: absolute coordinates (2 bytes each, little-endian)
                                let x_abs = (buffers[i][1] as u16) | ((buffers[i][2] as u16) << 8);
                                let y_abs = (buffers[i][3] as u16) | ((buffers[i][4] as u16) << 8);

                                crate::drivers::mouse::queue_absolute_event(
                                    x_abs,
                                    y_abs,
                                    left,
                                    right,
                                    middle,
                                );
                            } else {
                                // Mouse: relative coordinates
                                let x_delta = buffers[i][1] as i8;
                                let y_delta = buffers[i][2] as i8;

                                let report = BootMouseReport {
                                    buttons,
                                    x_delta,
                                    y_delta,
                                };
                                mouse.process_report(&report);
                                crate::drivers::mouse::queue_event(
                                    x_delta as i16,
                                    -(y_delta as i16),
                                    left,
                                    right,
                                    middle,
                                );
                            }
                            break;
                        }
                    }
                    continue;
                }
            }
        }
    }
}

/// Poll all registered HID keyboards and send scancodes to kernel keyboard driver
/// (Deprecated: use poll_all() instead to avoid race conditions)
pub fn poll_keyboards() {
    // Just call the unified poll function
    poll_all();
}

/// Poll all registered HID mice and send events to kernel mouse driver
/// (Deprecated: use poll_all() instead to avoid race conditions)
pub fn poll_mice() {
    // No-op: poll_keyboards already calls poll_all which handles both
}

/// Initialize HID subsystem
pub fn init() {
    crate::kprintln!("usb-hid: initialized");
}

/// Process HID configuration for a newly enumerated USB device
pub fn configure_device(
    slot_id: u8,
    _config_desc: &[u8],
    iface_desc: &InterfaceDescriptor,
    ep_desc: &EndpointDescriptor,
) -> Result<(), KError> {
    // Check if this is a HID interface
    if !is_hid_interface(iface_desc) {
        return Ok(());
    }

    // Only support interrupt IN endpoints for now
    if ep_desc.transfer_type() != EndpointType::Interrupt
        || ep_desc.direction() != EndpointDirection::In
    {
        return Ok(());
    }

    let protocol = HidProtocol::from(iface_desc.interface_protocol);

    let device = HidDevice {
        slot_id,
        interface_number: iface_desc.interface_number,
        protocol,
        endpoint_number: ep_desc.endpoint_number(),
        endpoint_interval: ep_desc.interval,
        max_packet_size: ep_desc.max_packet_size,
    };

    register_device(device);

    Ok(())
}
