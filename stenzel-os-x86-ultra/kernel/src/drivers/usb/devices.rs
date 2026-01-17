//! USB Device Manager
//!
//! Unified device registry for all USB devices across controllers (xHCI, EHCI).
//! Provides:
//! - Global device tracking
//! - Device enumeration APIs
//! - sysfs integration
//! - Hotplug event notifications

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::sync::Arc;
use alloc::format;
use spin::Mutex;

use crate::util::{KError, KResult};
use super::{UsbSpeed, DeviceDescriptor, ConfigDescriptor, InterfaceDescriptor};

/// USB controller type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControllerType {
    /// EHCI (USB 2.0)
    Ehci,
    /// xHCI (USB 3.x)
    Xhci,
    /// UHCI (USB 1.x legacy)
    Uhci,
    /// OHCI (USB 1.x legacy)
    Ohci,
}

impl ControllerType {
    pub fn name(&self) -> &'static str {
        match self {
            ControllerType::Ehci => "ehci",
            ControllerType::Xhci => "xhci",
            ControllerType::Uhci => "uhci",
            ControllerType::Ohci => "ohci",
        }
    }
}

/// Unique identifier for a USB device
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UsbDeviceId {
    /// Controller index
    pub controller: u8,
    /// Bus number
    pub bus: u8,
    /// Device address
    pub address: u8,
}

impl UsbDeviceId {
    pub fn new(controller: u8, bus: u8, address: u8) -> Self {
        Self { controller, bus, address }
    }

    /// Format as "bus-address" string (e.g., "1-3")
    pub fn to_string(&self) -> String {
        format!("{}-{}", self.bus, self.address)
    }

    /// Format as full path (e.g., "usb1/1-3")
    pub fn to_path(&self) -> String {
        format!("usb{}/{}-{}", self.bus, self.bus, self.address)
    }
}

/// USB device interface information
#[derive(Debug, Clone)]
pub struct UsbInterface {
    /// Interface number
    pub number: u8,
    /// Alternate setting
    pub alternate: u8,
    /// Class code
    pub class: u8,
    /// Subclass code
    pub subclass: u8,
    /// Protocol
    pub protocol: u8,
    /// Number of endpoints
    pub num_endpoints: u8,
    /// Interface name (from string descriptor)
    pub name: Option<String>,
    /// Driver name (if bound)
    pub driver: Option<String>,
}

impl UsbInterface {
    /// Get class name
    pub fn class_name(&self) -> &'static str {
        match self.class {
            0x00 => "interface-specific",
            0x01 => "audio",
            0x02 => "cdc",
            0x03 => "hid",
            0x05 => "physical",
            0x06 => "image",
            0x07 => "printer",
            0x08 => "mass-storage",
            0x09 => "hub",
            0x0A => "cdc-data",
            0x0B => "smart-card",
            0x0D => "content-security",
            0x0E => "video",
            0x0F => "healthcare",
            0x10 => "audio-video",
            0xDC => "diagnostic",
            0xE0 => "wireless",
            0xEF => "misc",
            0xFE => "application",
            0xFF => "vendor-specific",
            _ => "unknown",
        }
    }
}

/// USB configuration information
#[derive(Debug, Clone)]
pub struct UsbConfiguration {
    /// Configuration value
    pub value: u8,
    /// Configuration name (from string descriptor)
    pub name: Option<String>,
    /// Self-powered
    pub self_powered: bool,
    /// Remote wakeup capable
    pub remote_wakeup: bool,
    /// Max power in mA
    pub max_power_ma: u16,
    /// Interfaces
    pub interfaces: Vec<UsbInterface>,
}

/// Complete USB device information
#[derive(Debug, Clone)]
pub struct UsbDeviceInfo {
    /// Unique device ID
    pub id: UsbDeviceId,
    /// Controller type
    pub controller_type: ControllerType,
    /// Slot ID (for xHCI)
    pub slot_id: u8,
    /// Port number on controller
    pub port: u8,
    /// USB speed
    pub speed: UsbSpeed,
    /// Vendor ID
    pub vendor_id: u16,
    /// Product ID
    pub product_id: u16,
    /// Device class
    pub device_class: u8,
    /// Device subclass
    pub device_subclass: u8,
    /// Device protocol
    pub device_protocol: u8,
    /// USB specification version (BCD)
    pub usb_version: u16,
    /// Device version (BCD)
    pub device_version: u16,
    /// Manufacturer name
    pub manufacturer: Option<String>,
    /// Product name
    pub product: Option<String>,
    /// Serial number
    pub serial: Option<String>,
    /// Number of configurations
    pub num_configurations: u8,
    /// Active configuration value
    pub active_config: u8,
    /// Configurations
    pub configurations: Vec<UsbConfiguration>,
    /// Parent device (for hub-attached devices)
    pub parent: Option<UsbDeviceId>,
    /// Time when device was enumerated (ticks)
    pub enumerated_at: u64,
    /// Device state
    pub state: UsbDeviceState,
}

/// USB device state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbDeviceState {
    /// Device just attached, not yet configured
    Attached,
    /// Device addressed
    Addressed,
    /// Device configured
    Configured,
    /// Device suspended
    Suspended,
    /// Device being removed
    Removing,
}

impl UsbDeviceInfo {
    /// Get device class name
    pub fn class_name(&self) -> &'static str {
        match self.device_class {
            0x00 => "interface-specific",
            0x09 => "hub",
            0xDC => "diagnostic",
            0xE0 => "wireless",
            0xEF => "misc",
            0xFF => "vendor-specific",
            _ => "unknown",
        }
    }

    /// Get speed string
    pub fn speed_string(&self) -> &'static str {
        match self.speed {
            UsbSpeed::Low => "1.5Mb/s",
            UsbSpeed::Full => "12Mb/s",
            UsbSpeed::High => "480Mb/s",
            UsbSpeed::Super => "5Gb/s",
            UsbSpeed::SuperPlus => "10Gb/s",
        }
    }

    /// Get USB version string
    pub fn usb_version_string(&self) -> String {
        let major = (self.usb_version >> 8) & 0xFF;
        let minor = (self.usb_version >> 4) & 0x0F;
        let patch = self.usb_version & 0x0F;
        format!("{}.{}.{}", major, minor, patch)
    }

    /// Check if device is a hub
    pub fn is_hub(&self) -> bool {
        self.device_class == 0x09
    }

    /// Find interface by class
    pub fn find_interface_by_class(&self, class: u8) -> Option<&UsbInterface> {
        for config in &self.configurations {
            for iface in &config.interfaces {
                if iface.class == class {
                    return Some(iface);
                }
            }
        }
        None
    }
}

/// USB device manager
pub struct UsbDeviceManager {
    /// All registered devices
    devices: BTreeMap<UsbDeviceId, UsbDeviceInfo>,
    /// Next bus number
    next_bus: u8,
    /// Device event callbacks
    callbacks: Vec<DeviceCallback>,
    /// Statistics
    stats: UsbStats,
}

/// Device event callback
type DeviceCallback = Arc<dyn Fn(UsbEvent) + Send + Sync>;

/// USB device event
#[derive(Debug, Clone)]
pub enum UsbEvent {
    /// Device attached
    Attached(UsbDeviceId),
    /// Device detached
    Detached(UsbDeviceId),
    /// Device configured
    Configured(UsbDeviceId),
    /// Device suspended
    Suspended(UsbDeviceId),
    /// Device resumed
    Resumed(UsbDeviceId),
}

/// USB statistics
#[derive(Debug, Clone, Default)]
pub struct UsbStats {
    /// Total devices ever enumerated
    pub total_enumerated: u64,
    /// Current device count
    pub current_devices: u64,
    /// Failed enumerations
    pub failed_enumerations: u64,
    /// Control transfers
    pub control_transfers: u64,
    /// Bulk transfers
    pub bulk_transfers: u64,
    /// Interrupt transfers
    pub interrupt_transfers: u64,
    /// Isochronous transfers
    pub iso_transfers: u64,
    /// Transfer errors
    pub transfer_errors: u64,
}

impl UsbDeviceManager {
    /// Create a new device manager
    pub fn new() -> Self {
        Self {
            devices: BTreeMap::new(),
            next_bus: 1,
            callbacks: Vec::new(),
            stats: UsbStats::default(),
        }
    }

    /// Allocate a new bus number
    pub fn allocate_bus(&mut self) -> u8 {
        let bus = self.next_bus;
        self.next_bus = self.next_bus.saturating_add(1);
        bus
    }

    /// Register a new device
    pub fn register_device(&mut self, mut device: UsbDeviceInfo) -> KResult<UsbDeviceId> {
        let id = device.id;

        // Check for duplicate
        if self.devices.contains_key(&id) {
            return Err(KError::AlreadyExists);
        }

        device.enumerated_at = crate::time::ticks();
        device.state = UsbDeviceState::Addressed;

        self.devices.insert(id, device);
        self.stats.total_enumerated += 1;
        self.stats.current_devices += 1;

        // Notify listeners
        self.notify(UsbEvent::Attached(id));

        crate::kprintln!(
            "usb: registered device {}-{}: {:04x}:{:04x}",
            id.bus, id.address,
            self.devices.get(&id).map(|d| d.vendor_id).unwrap_or(0),
            self.devices.get(&id).map(|d| d.product_id).unwrap_or(0)
        );

        Ok(id)
    }

    /// Unregister a device
    pub fn unregister_device(&mut self, id: &UsbDeviceId) -> KResult<()> {
        if let Some(mut device) = self.devices.remove(id) {
            device.state = UsbDeviceState::Removing;
            self.stats.current_devices = self.stats.current_devices.saturating_sub(1);
            self.notify(UsbEvent::Detached(*id));
            Ok(())
        } else {
            Err(KError::NotFound)
        }
    }

    /// Get device by ID
    pub fn get_device(&self, id: &UsbDeviceId) -> Option<&UsbDeviceInfo> {
        self.devices.get(id)
    }

    /// Get device by ID (mutable)
    pub fn get_device_mut(&mut self, id: &UsbDeviceId) -> Option<&mut UsbDeviceInfo> {
        self.devices.get_mut(id)
    }

    /// List all devices
    pub fn list_devices(&self) -> Vec<&UsbDeviceInfo> {
        self.devices.values().collect()
    }

    /// Find devices by class
    pub fn find_by_class(&self, class: u8) -> Vec<&UsbDeviceInfo> {
        self.devices
            .values()
            .filter(|d| d.device_class == class)
            .collect()
    }

    /// Find devices by vendor/product ID
    pub fn find_by_ids(&self, vendor_id: u16, product_id: u16) -> Vec<&UsbDeviceInfo> {
        self.devices
            .values()
            .filter(|d| d.vendor_id == vendor_id && d.product_id == product_id)
            .collect()
    }

    /// Find devices with interface of given class
    pub fn find_by_interface_class(&self, class: u8) -> Vec<&UsbDeviceInfo> {
        self.devices
            .values()
            .filter(|d| d.find_interface_by_class(class).is_some())
            .collect()
    }

    /// Set device configuration
    pub fn set_device_configured(&mut self, id: &UsbDeviceId, config: u8) -> KResult<()> {
        if let Some(device) = self.devices.get_mut(id) {
            device.state = UsbDeviceState::Configured;
            device.active_config = config;
            self.notify(UsbEvent::Configured(*id));
            Ok(())
        } else {
            Err(KError::NotFound)
        }
    }

    /// Register event callback
    pub fn register_callback(&mut self, callback: DeviceCallback) {
        self.callbacks.push(callback);
    }

    /// Notify all callbacks
    fn notify(&self, event: UsbEvent) {
        for callback in &self.callbacks {
            callback(event.clone());
        }
    }

    /// Get statistics
    pub fn stats(&self) -> &UsbStats {
        &self.stats
    }

    /// Record a transfer
    pub fn record_transfer(&mut self, transfer_type: u8, success: bool) {
        match transfer_type {
            0 => self.stats.control_transfers += 1,
            1 => self.stats.iso_transfers += 1,
            2 => self.stats.bulk_transfers += 1,
            3 => self.stats.interrupt_transfers += 1,
            _ => {}
        }
        if !success {
            self.stats.transfer_errors += 1;
        }
    }

    /// Record failed enumeration
    pub fn record_enumeration_failure(&mut self) {
        self.stats.failed_enumerations += 1;
    }

    /// Format device info for sysfs
    pub fn format_device_info(&self, id: &UsbDeviceId) -> Option<String> {
        let device = self.devices.get(id)?;

        let mut info = String::new();
        use core::fmt::Write;

        let _ = writeln!(info, "Bus {:03} Device {:03}: ID {:04x}:{:04x}",
            id.bus, id.address, device.vendor_id, device.product_id);

        if let Some(ref mfr) = device.manufacturer {
            let _ = writeln!(info, "Manufacturer: {}", mfr);
        }
        if let Some(ref prod) = device.product {
            let _ = writeln!(info, "Product: {}", prod);
        }
        if let Some(ref serial) = device.serial {
            let _ = writeln!(info, "Serial: {}", serial);
        }

        let _ = writeln!(info, "Speed: {}", device.speed_string());
        let _ = writeln!(info, "USB Version: {}", device.usb_version_string());
        let _ = writeln!(info, "Class: {} ({:02x})", device.class_name(), device.device_class);
        let _ = writeln!(info, "Configurations: {}", device.num_configurations);

        for config in &device.configurations {
            let _ = writeln!(info, "  Configuration {}:", config.value);
            let _ = writeln!(info, "    Max Power: {} mA", config.max_power_ma);
            for iface in &config.interfaces {
                let _ = writeln!(info, "    Interface {}:", iface.number);
                let _ = writeln!(info, "      Class: {} ({:02x})", iface.class_name(), iface.class);
                let _ = writeln!(info, "      Endpoints: {}", iface.num_endpoints);
                if let Some(ref driver) = iface.driver {
                    let _ = writeln!(info, "      Driver: {}", driver);
                }
            }
        }

        Some(info)
    }

    /// Format all devices (lsusb style)
    pub fn format_all_devices(&self) -> String {
        let mut output = String::new();
        use core::fmt::Write;

        for device in self.devices.values() {
            let _ = writeln!(output, "Bus {:03} Device {:03}: ID {:04x}:{:04x} {}",
                device.id.bus,
                device.id.address,
                device.vendor_id,
                device.product_id,
                device.product.as_deref().unwrap_or("Unknown Device")
            );
        }

        output
    }

    /// Format statistics
    pub fn format_stats(&self) -> String {
        let mut output = String::new();
        use core::fmt::Write;

        let _ = writeln!(output, "USB Statistics:");
        let _ = writeln!(output, "  Total enumerated: {}", self.stats.total_enumerated);
        let _ = writeln!(output, "  Current devices: {}", self.stats.current_devices);
        let _ = writeln!(output, "  Failed enumerations: {}", self.stats.failed_enumerations);
        let _ = writeln!(output, "  Control transfers: {}", self.stats.control_transfers);
        let _ = writeln!(output, "  Bulk transfers: {}", self.stats.bulk_transfers);
        let _ = writeln!(output, "  Interrupt transfers: {}", self.stats.interrupt_transfers);
        let _ = writeln!(output, "  Isochronous transfers: {}", self.stats.iso_transfers);
        let _ = writeln!(output, "  Transfer errors: {}", self.stats.transfer_errors);

        output
    }
}

/// Global USB device manager
static USB_MANAGER: Mutex<Option<UsbDeviceManager>> = Mutex::new(None);

/// Initialize the USB device manager
pub fn init() {
    let mut manager = USB_MANAGER.lock();
    if manager.is_none() {
        *manager = Some(UsbDeviceManager::new());
        crate::kprintln!("usb: device manager initialized");
    }
}

/// Get the USB device manager
fn with_manager<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&UsbDeviceManager) -> R,
{
    USB_MANAGER.lock().as_ref().map(f)
}

/// Get the USB device manager (mutable)
fn with_manager_mut<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut UsbDeviceManager) -> R,
{
    USB_MANAGER.lock().as_mut().map(f)
}

/// Allocate a new bus number
pub fn allocate_bus() -> u8 {
    with_manager_mut(|m| m.allocate_bus()).unwrap_or(1)
}

/// Register a new USB device
pub fn register_device(device: UsbDeviceInfo) -> KResult<UsbDeviceId> {
    with_manager_mut(|m| m.register_device(device))
        .ok_or(KError::Invalid)?
}

/// Unregister a USB device
pub fn unregister_device(id: &UsbDeviceId) -> KResult<()> {
    with_manager_mut(|m| m.unregister_device(id))
        .ok_or(KError::Invalid)?
}

/// Get device info
pub fn get_device(id: &UsbDeviceId) -> Option<UsbDeviceInfo> {
    with_manager(|m| m.get_device(id).cloned()).flatten()
}

/// List all devices
pub fn list_devices() -> Vec<UsbDeviceInfo> {
    with_manager(|m| m.list_devices().into_iter().cloned().collect())
        .unwrap_or_default()
}

/// Find devices by class
pub fn find_by_class(class: u8) -> Vec<UsbDeviceInfo> {
    with_manager(|m| m.find_by_class(class).into_iter().cloned().collect())
        .unwrap_or_default()
}

/// Find devices by vendor/product ID
pub fn find_by_ids(vendor_id: u16, product_id: u16) -> Vec<UsbDeviceInfo> {
    with_manager(|m| m.find_by_ids(vendor_id, product_id).into_iter().cloned().collect())
        .unwrap_or_default()
}

/// Find devices with interface of given class
pub fn find_by_interface_class(class: u8) -> Vec<UsbDeviceInfo> {
    with_manager(|m| m.find_by_interface_class(class).into_iter().cloned().collect())
        .unwrap_or_default()
}

/// Set device as configured
pub fn set_device_configured(id: &UsbDeviceId, config: u8) -> KResult<()> {
    with_manager_mut(|m| m.set_device_configured(id, config))
        .ok_or(KError::Invalid)?
}

/// Record a transfer
pub fn record_transfer(transfer_type: u8, success: bool) {
    with_manager_mut(|m| m.record_transfer(transfer_type, success));
}

/// Record failed enumeration
pub fn record_enumeration_failure() {
    with_manager_mut(|m| m.record_enumeration_failure());
}

/// Format device info
pub fn format_device_info(id: &UsbDeviceId) -> Option<String> {
    with_manager(|m| m.format_device_info(id)).flatten()
}

/// Format all devices (lsusb style)
pub fn format_all_devices() -> String {
    with_manager(|m| m.format_all_devices()).unwrap_or_default()
}

/// Format statistics
pub fn format_stats() -> String {
    with_manager(|m| m.format_stats()).unwrap_or_default()
}

/// Get device count
pub fn device_count() -> usize {
    with_manager(|m| m.devices.len()).unwrap_or(0)
}

/// Create a UsbDeviceInfo from descriptor data
pub fn create_device_info(
    controller: u8,
    bus: u8,
    address: u8,
    slot_id: u8,
    port: u8,
    controller_type: ControllerType,
    speed: UsbSpeed,
    desc: &DeviceDescriptor,
) -> UsbDeviceInfo {
    UsbDeviceInfo {
        id: UsbDeviceId::new(controller, bus, address),
        controller_type,
        slot_id,
        port,
        speed,
        vendor_id: desc.vendor_id,
        product_id: desc.product_id,
        device_class: desc.device_class,
        device_subclass: desc.device_subclass,
        device_protocol: desc.device_protocol,
        usb_version: desc.usb_version,
        device_version: desc.device_version,
        manufacturer: None,
        product: None,
        serial: None,
        num_configurations: desc.num_configurations,
        active_config: 0,
        configurations: Vec::new(),
        parent: None,
        enumerated_at: 0,
        state: UsbDeviceState::Attached,
    }
}

/// Add configuration to device
pub fn add_configuration(
    id: &UsbDeviceId,
    config: &ConfigDescriptor,
    interfaces: Vec<UsbInterface>,
) -> KResult<()> {
    with_manager_mut(|m| {
        if let Some(device) = m.get_device_mut(id) {
            device.configurations.push(UsbConfiguration {
                value: config.configuration_value,
                name: None,
                self_powered: (config.attributes & 0x40) != 0,
                remote_wakeup: (config.attributes & 0x20) != 0,
                max_power_ma: (config.max_power as u16) * 2,
                interfaces,
            });
            Ok(())
        } else {
            Err(KError::NotFound)
        }
    }).ok_or(KError::Invalid)?
}

/// Set device strings
pub fn set_device_strings(
    id: &UsbDeviceId,
    manufacturer: Option<String>,
    product: Option<String>,
    serial: Option<String>,
) -> KResult<()> {
    with_manager_mut(|m| {
        if let Some(device) = m.get_device_mut(id) {
            device.manufacturer = manufacturer;
            device.product = product;
            device.serial = serial;
            Ok(())
        } else {
            Err(KError::NotFound)
        }
    }).ok_or(KError::Invalid)?
}

/// Bind driver to interface
pub fn bind_interface_driver(
    id: &UsbDeviceId,
    interface_num: u8,
    driver_name: &str,
) -> KResult<()> {
    with_manager_mut(|m| {
        if let Some(device) = m.get_device_mut(id) {
            for config in &mut device.configurations {
                for iface in &mut config.interfaces {
                    if iface.number == interface_num {
                        iface.driver = Some(driver_name.to_string());
                        return Ok(());
                    }
                }
            }
            Err(KError::NotFound)
        } else {
            Err(KError::NotFound)
        }
    }).ok_or(KError::Invalid)?
}
