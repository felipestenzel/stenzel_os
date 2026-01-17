//! USB Hub driver
//!
//! Handles USB hub enumeration, port power, and downstream device detection.

use alloc::vec::Vec;
use spin::Mutex;

use super::{class, SetupPacket, UsbSpeed};
use crate::util::KError;

/// Hub descriptor (USB 2.0)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct HubDescriptor {
    pub length: u8,
    pub descriptor_type: u8,
    pub num_ports: u8,
    pub characteristics: u16,
    pub power_on_time: u8,  // in 2ms units
    pub current: u8,
}

/// SuperSpeed Hub descriptor (USB 3.0)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct SsHubDescriptor {
    pub length: u8,
    pub descriptor_type: u8,
    pub num_ports: u8,
    pub characteristics: u16,
    pub power_on_time: u8,
    pub current: u8,
    pub header_decode_latency: u8,
    pub hub_delay: u16,
    pub removable_ports: u16,
}

/// Hub port status bits
pub mod port_status {
    pub const CONNECTION: u16 = 1 << 0;
    pub const ENABLE: u16 = 1 << 1;
    pub const SUSPEND: u16 = 1 << 2;
    pub const OVER_CURRENT: u16 = 1 << 3;
    pub const RESET: u16 = 1 << 4;
    pub const POWER: u16 = 1 << 8;
    pub const LOW_SPEED: u16 = 1 << 9;
    pub const HIGH_SPEED: u16 = 1 << 10;
    pub const TEST_MODE: u16 = 1 << 11;
    pub const INDICATOR: u16 = 1 << 12;
}

/// Hub port change bits
pub mod port_change {
    pub const C_CONNECTION: u16 = 1 << 0;
    pub const C_ENABLE: u16 = 1 << 1;
    pub const C_SUSPEND: u16 = 1 << 2;
    pub const C_OVER_CURRENT: u16 = 1 << 3;
    pub const C_RESET: u16 = 1 << 4;
}

/// Hub class-specific requests
pub mod hub_request {
    pub const GET_STATUS: u8 = 0;
    pub const CLEAR_FEATURE: u8 = 1;
    pub const SET_FEATURE: u8 = 3;
    pub const GET_DESCRIPTOR: u8 = 6;
    pub const SET_DESCRIPTOR: u8 = 7;
    pub const CLEAR_TT_BUFFER: u8 = 8;
    pub const RESET_TT: u8 = 9;
    pub const GET_TT_STATE: u8 = 10;
    pub const STOP_TT: u8 = 11;
}

/// Hub feature selectors
pub mod hub_feature {
    // Hub features
    pub const C_HUB_LOCAL_POWER: u16 = 0;
    pub const C_HUB_OVER_CURRENT: u16 = 1;

    // Port features
    pub const PORT_CONNECTION: u16 = 0;
    pub const PORT_ENABLE: u16 = 1;
    pub const PORT_SUSPEND: u16 = 2;
    pub const PORT_OVER_CURRENT: u16 = 3;
    pub const PORT_RESET: u16 = 4;
    pub const PORT_POWER: u16 = 8;
    pub const PORT_LOW_SPEED: u16 = 9;
    pub const C_PORT_CONNECTION: u16 = 16;
    pub const C_PORT_ENABLE: u16 = 17;
    pub const C_PORT_SUSPEND: u16 = 18;
    pub const C_PORT_OVER_CURRENT: u16 = 19;
    pub const C_PORT_RESET: u16 = 20;
    pub const PORT_TEST: u16 = 21;
    pub const PORT_INDICATOR: u16 = 22;
}

/// Port status and change (4 bytes returned by GET_PORT_STATUS)
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct PortStatus {
    pub status: u16,
    pub change: u16,
}

impl PortStatus {
    /// Check if device is connected
    pub fn connected(&self) -> bool {
        self.status & port_status::CONNECTION != 0
    }

    /// Check if port is enabled
    pub fn enabled(&self) -> bool {
        self.status & port_status::ENABLE != 0
    }

    /// Check if port is powered
    pub fn powered(&self) -> bool {
        self.status & port_status::POWER != 0
    }

    /// Get device speed
    pub fn speed(&self) -> UsbSpeed {
        if self.status & port_status::LOW_SPEED != 0 {
            UsbSpeed::Low
        } else if self.status & port_status::HIGH_SPEED != 0 {
            UsbSpeed::High
        } else {
            UsbSpeed::Full
        }
    }

    /// Check if connection status changed
    pub fn connection_changed(&self) -> bool {
        self.change & port_change::C_CONNECTION != 0
    }

    /// Check if reset completed
    pub fn reset_changed(&self) -> bool {
        self.change & port_change::C_RESET != 0
    }
}

/// USB Hub instance
pub struct UsbHub {
    pub slot_id: u8,
    pub num_ports: u8,
    pub power_on_delay_ms: u16,
    pub is_superspeed: bool,
    pub route_string: u32,
    pub port_statuses: Vec<PortStatus>,
}

impl UsbHub {
    /// Create setup packet for getting hub descriptor
    pub fn get_hub_descriptor_setup(length: u16, is_superspeed: bool) -> SetupPacket {
        let desc_type = if is_superspeed { 0x2A } else { 0x29 };
        SetupPacket {
            request_type: 0xA0, // Device to Host, Class, Device
            request: hub_request::GET_DESCRIPTOR,
            value: (desc_type as u16) << 8,
            index: 0,
            length,
        }
    }

    /// Create setup packet for getting port status
    pub fn get_port_status_setup(port: u8) -> SetupPacket {
        SetupPacket {
            request_type: 0xA3, // Device to Host, Class, Other
            request: hub_request::GET_STATUS,
            value: 0,
            index: port as u16,
            length: 4,
        }
    }

    /// Create setup packet for setting port feature
    pub fn set_port_feature_setup(port: u8, feature: u16) -> SetupPacket {
        SetupPacket {
            request_type: 0x23, // Host to Device, Class, Other
            request: hub_request::SET_FEATURE,
            value: feature,
            index: port as u16,
            length: 0,
        }
    }

    /// Create setup packet for clearing port feature
    pub fn clear_port_feature_setup(port: u8, feature: u16) -> SetupPacket {
        SetupPacket {
            request_type: 0x23, // Host to Device, Class, Other
            request: hub_request::CLEAR_FEATURE,
            value: feature,
            index: port as u16,
            length: 0,
        }
    }
}

/// List of known hubs
static HUBS: Mutex<Vec<UsbHub>> = Mutex::new(Vec::new());

/// Initialize hub subsystem
pub fn init() {
    crate::kprintln!("usb_hub: initialized");
}

/// Register a new hub
pub fn register_hub(hub: UsbHub) {
    let num_ports = hub.num_ports;
    let slot = hub.slot_id;
    HUBS.lock().push(hub);
    crate::kprintln!("usb_hub: registered hub slot {} with {} ports", slot, num_ports);
}

/// Check if a device class indicates it's a hub
pub fn is_hub_class(device_class: u8) -> bool {
    device_class == class::HUB
}

/// Get number of registered hubs
pub fn hub_count() -> usize {
    HUBS.lock().len()
}

/// Process a hub device that was just enumerated
/// This function should be called by xhci when it detects a hub
pub fn setup_hub(
    slot_id: u8,
    is_superspeed: bool,
    num_ports: u8,
    power_on_delay_2ms: u8,
    route_string: u32,
) {
    let hub = UsbHub {
        slot_id,
        num_ports,
        power_on_delay_ms: (power_on_delay_2ms as u16) * 2,
        is_superspeed,
        route_string,
        port_statuses: alloc::vec![PortStatus::default(); num_ports as usize],
    };

    crate::kprintln!(
        "usb_hub: setup hub slot {} ({} ports, {}ms power delay, route=0x{:x})",
        slot_id,
        num_ports,
        hub.power_on_delay_ms,
        route_string
    );

    register_hub(hub);
}

/// Calculate route string for a device behind a hub
/// Route string encodes the path through hub ports (4 bits per tier)
pub fn calculate_route_string(parent_route: u32, hub_port: u8) -> u32 {
    // Find the first empty nibble in the route string
    // Each nibble holds a port number (1-15, 0 means unused)
    let port = (hub_port.min(15)) as u32;

    if parent_route == 0 {
        // First hub tier - just the port number
        port
    } else {
        // Find position to insert
        let mut route = parent_route;
        let mut shift = 0u32;

        while shift < 20 && ((route >> shift) & 0xF) != 0 {
            shift += 4;
        }

        if shift >= 20 {
            // Too many tiers (max 5)
            parent_route
        } else {
            route | (port << shift)
        }
    }
}
