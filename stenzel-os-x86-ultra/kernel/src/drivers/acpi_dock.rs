//! ACPI Docking Station Support
//!
//! Provides support for laptop docking stations via ACPI:
//! - Dock/undock detection
//! - Hot-plug device handling
//! - Power and display switching
//! - Peripheral enumeration

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::sync::IrqSafeMutex;
use crate::util::{KResult, KError};

/// Dock state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockState {
    /// Not docked
    Undocked,
    /// Docking in progress
    Docking,
    /// Fully docked
    Docked,
    /// Undocking in progress
    Undocking,
    /// Dock ejection requested
    EjectPending,
    /// Error state
    Error,
}

/// Dock type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockType {
    /// Traditional mechanical dock
    Mechanical,
    /// USB-C/Thunderbolt dock
    UsbC,
    /// Thunderbolt specific dock
    Thunderbolt,
    /// USB 3.0 dock
    Usb3,
    /// Proprietary dock
    Proprietary,
    /// Unknown type
    Unknown,
}

/// Dock event type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockEvent {
    /// Dock inserted
    DockInserted,
    /// Dock removed
    DockRemoved,
    /// Eject request from dock button
    EjectRequest,
    /// Device connected to dock
    DeviceConnected,
    /// Device disconnected from dock
    DeviceDisconnected,
    /// Power changed
    PowerChanged,
    /// Display connected
    DisplayConnected,
    /// Display disconnected
    DisplayDisconnected,
}

/// Dock device type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockDeviceType {
    /// Network adapter
    Network,
    /// Display/Monitor
    Display,
    /// USB hub
    UsbHub,
    /// Storage device
    Storage,
    /// Audio device
    Audio,
    /// Power delivery
    Power,
    /// Keyboard
    Keyboard,
    /// Mouse
    Mouse,
    /// Other device
    Other,
}

/// Device connected to dock
#[derive(Debug, Clone)]
pub struct DockDevice {
    /// Device ID
    pub id: u32,
    /// Device type
    pub device_type: DockDeviceType,
    /// Device name
    pub name: String,
    /// Is device present/connected
    pub present: bool,
    /// Device path/address
    pub path: String,
    /// PCI device if applicable
    pub pci_address: Option<(u8, u8, u8)>,
    /// USB port if applicable
    pub usb_port: Option<u8>,
}

/// Dock information
#[derive(Debug, Clone)]
pub struct DockInfo {
    /// Dock ID
    pub id: u32,
    /// ACPI path
    pub acpi_path: String,
    /// Dock type
    pub dock_type: DockType,
    /// Dock manufacturer
    pub manufacturer: String,
    /// Dock model
    pub model: String,
    /// Serial number
    pub serial: String,
    /// Current state
    pub state: DockState,
    /// Connected devices
    pub devices: Vec<DockDevice>,
    /// Supports hot-undock
    pub supports_hot_undock: bool,
    /// Supports eject
    pub supports_eject: bool,
    /// Has power delivery
    pub has_power_delivery: bool,
    /// Power delivery watts
    pub power_watts: u32,
    /// Number of display ports
    pub display_ports: u8,
    /// Number of USB ports
    pub usb_ports: u8,
    /// Has ethernet
    pub has_ethernet: bool,
}

impl Default for DockInfo {
    fn default() -> Self {
        DockInfo {
            id: 0,
            acpi_path: String::new(),
            dock_type: DockType::Unknown,
            manufacturer: String::new(),
            model: String::new(),
            serial: String::new(),
            state: DockState::Undocked,
            devices: Vec::new(),
            supports_hot_undock: true,
            supports_eject: true,
            has_power_delivery: false,
            power_watts: 0,
            display_ports: 0,
            usb_ports: 0,
            has_ethernet: false,
        }
    }
}

/// Dock statistics
#[derive(Debug, Default)]
pub struct DockStats {
    /// Total dock events
    pub total_events: AtomicU64,
    /// Dock count
    pub dock_count: AtomicU64,
    /// Undock count
    pub undock_count: AtomicU64,
    /// Eject requests
    pub eject_requests: AtomicU64,
    /// Failed operations
    pub failures: AtomicU64,
    /// Last dock timestamp
    pub last_dock_time: AtomicU64,
    /// Last undock timestamp
    pub last_undock_time: AtomicU64,
}

pub type DockEventCallback = fn(DockEvent, u32);

/// ACPI dock manager
pub struct AcpiDockManager {
    /// Detected docks
    docks: Vec<DockInfo>,
    /// Next dock ID
    next_id: u32,
    /// Statistics
    stats: DockStats,
    /// Event callbacks
    callbacks: Vec<DockEventCallback>,
    /// Initialized
    initialized: bool,
}

pub static DOCK_MANAGER: IrqSafeMutex<AcpiDockManager> = IrqSafeMutex::new(AcpiDockManager::new());

impl AcpiDockManager {
    pub const fn new() -> Self {
        AcpiDockManager {
            docks: Vec::new(),
            next_id: 1,
            stats: DockStats {
                total_events: AtomicU64::new(0),
                dock_count: AtomicU64::new(0),
                undock_count: AtomicU64::new(0),
                eject_requests: AtomicU64::new(0),
                failures: AtomicU64::new(0),
                last_dock_time: AtomicU64::new(0),
                last_undock_time: AtomicU64::new(0),
            },
            callbacks: Vec::new(),
            initialized: false,
        }
    }

    /// Initialize dock manager
    pub fn init(&mut self) -> KResult<()> {
        if self.initialized {
            return Ok(());
        }

        // Scan for ACPI dock devices
        self.scan_acpi_docks()?;

        // Scan for USB-C/Thunderbolt docks
        self.scan_usbc_docks()?;

        self.initialized = true;
        crate::kprintln!("acpi_dock: initialized with {} dock(s)", self.docks.len());
        Ok(())
    }

    /// Scan for ACPI dock devices
    fn scan_acpi_docks(&mut self) -> KResult<()> {
        // Look for ACPI devices with _DCK (dock) method
        // Common ACPI paths for docks:
        // \_SB.DOCK, \_SB.PCI0.DOCK, etc.

        // Check for dock in ACPI namespace
        // This would query the ACPI tables
        // For now, we'll check some common indicators

        // Check PnP IDs for dock devices
        let dock_pnp_ids = [
            "PNP0C15", // Generic dock
            "ACPI0003", // AC adapter (often on docks)
        ];

        // If we find a dock device, add it
        // In real implementation, this would enumerate ACPI namespace

        Ok(())
    }

    /// Scan for USB-C/Thunderbolt docks
    fn scan_usbc_docks(&mut self) -> KResult<()> {
        // Check for Thunderbolt controllers
        let pci_devices = crate::drivers::pci::scan();

        for device in pci_devices {
            // Thunderbolt controllers: class 0x0C, subclass 0x04
            if device.class.class_code == 0x0C && device.class.subclass == 0x04 {
                // Found Thunderbolt controller - check for attached dock
                self.check_thunderbolt_dock(&device)?;
            }

            // USB controllers with hubs might indicate USB dock
            if device.class.class_code == 0x0C && device.class.subclass == 0x03 {
                // USB controller - check for USB dock
                self.check_usb_dock(&device)?;
            }
        }

        Ok(())
    }

    /// Check for Thunderbolt dock
    fn check_thunderbolt_dock(&mut self, _device: &crate::drivers::pci::PciDevice) -> KResult<()> {
        // In real implementation, query Thunderbolt controller for attached devices
        Ok(())
    }

    /// Check for USB dock
    fn check_usb_dock(&mut self, _device: &crate::drivers::pci::PciDevice) -> KResult<()> {
        // In real implementation, enumerate USB hub for multi-port hubs that might be docks
        Ok(())
    }

    /// Register a dock
    pub fn register_dock(&mut self, mut info: DockInfo) -> u32 {
        info.id = self.next_id;
        self.next_id += 1;

        let id = info.id;
        self.docks.push(info);

        crate::kprintln!("acpi_dock: registered dock {}", id);
        id
    }

    /// Handle ACPI dock event
    pub fn handle_acpi_event(&mut self, acpi_path: &str, event_type: u32) {
        self.stats.total_events.fetch_add(1, Ordering::Relaxed);

        // Find dock by ACPI path
        let dock_id = self.docks.iter()
            .find(|d| d.acpi_path == acpi_path)
            .map(|d| d.id);

        let dock_id = match dock_id {
            Some(id) => id,
            None => {
                // New dock detected - register it
                let info = DockInfo {
                    acpi_path: String::from(acpi_path),
                    ..Default::default()
                };
                self.register_dock(info)
            }
        };

        // Parse event type
        let event = match event_type {
            0x00 => DockEvent::DockInserted,
            0x01 => DockEvent::DockRemoved,
            0x03 => DockEvent::EjectRequest,
            _ => return,
        };

        self.process_event(dock_id, event);
    }

    /// Handle dock insertion
    pub fn handle_dock_inserted(&mut self, dock_id: u32) {
        self.process_event(dock_id, DockEvent::DockInserted);
    }

    /// Handle dock removal
    pub fn handle_dock_removed(&mut self, dock_id: u32) {
        self.process_event(dock_id, DockEvent::DockRemoved);
    }

    /// Handle eject request
    pub fn handle_eject_request(&mut self, dock_id: u32) {
        self.process_event(dock_id, DockEvent::EjectRequest);
    }

    /// Process dock event
    fn process_event(&mut self, dock_id: u32, event: DockEvent) {
        if let Some(dock) = self.docks.iter_mut().find(|d| d.id == dock_id) {
            match event {
                DockEvent::DockInserted => {
                    dock.state = DockState::Docked;
                    self.stats.dock_count.fetch_add(1, Ordering::Relaxed);
                    self.stats.last_dock_time.store(crate::time::uptime_ms(), Ordering::Relaxed);

                    // Enumerate devices
                    self.enumerate_dock_devices(dock_id);

                    crate::kprintln!("acpi_dock: dock {} inserted", dock_id);
                }
                DockEvent::DockRemoved => {
                    dock.state = DockState::Undocked;
                    dock.devices.clear();
                    self.stats.undock_count.fetch_add(1, Ordering::Relaxed);
                    self.stats.last_undock_time.store(crate::time::uptime_ms(), Ordering::Relaxed);

                    crate::kprintln!("acpi_dock: dock {} removed", dock_id);
                }
                DockEvent::EjectRequest => {
                    dock.state = DockState::EjectPending;
                    self.stats.eject_requests.fetch_add(1, Ordering::Relaxed);

                    crate::kprintln!("acpi_dock: eject requested for dock {}", dock_id);
                }
                _ => {}
            }
        }

        // Fire callbacks
        for cb in &self.callbacks {
            cb(event, dock_id);
        }
    }

    /// Enumerate devices connected to dock
    fn enumerate_dock_devices(&mut self, dock_id: u32) {
        // Find new devices connected through the dock
        // This would check PCI, USB, etc. for new devices

        // Check for new network adapters
        // Check for new displays
        // Check for new USB devices
        // etc.

        crate::kprintln!("acpi_dock: enumerating devices for dock {}", dock_id);
    }

    /// Request dock ejection
    pub fn request_eject(&mut self, dock_id: u32) -> KResult<()> {
        // First check conditions and get acpi_path
        let acpi_path = {
            let dock = self.docks.iter().find(|d| d.id == dock_id)
                .ok_or(KError::NotFound)?;

            if dock.state != DockState::Docked {
                return Err(KError::Invalid);
            }

            if !dock.supports_eject {
                return Err(KError::NotSupported);
            }

            dock.acpi_path.clone()
        };

        // Update state
        if let Some(dock) = self.docks.iter_mut().find(|d| d.id == dock_id) {
            dock.state = DockState::EjectPending;
        }

        // Notify devices to prepare for removal
        self.prepare_undock(dock_id)?;

        // Call ACPI _EJ0 method
        self.execute_acpi_eject(&acpi_path)?;

        Ok(())
    }

    /// Prepare for undocking
    fn prepare_undock(&mut self, dock_id: u32) -> KResult<()> {
        // Notify connected devices of impending removal
        // Flush pending I/O
        // Unmount removable storage
        // etc.

        crate::kprintln!("acpi_dock: preparing undock for {}", dock_id);
        Ok(())
    }

    /// Execute ACPI eject
    fn execute_acpi_eject(&self, _acpi_path: &str) -> KResult<()> {
        // Call ACPI _EJ0 method for the dock
        // This would use the ACPI interpreter
        Ok(())
    }

    /// Get dock info
    pub fn get_dock(&self, dock_id: u32) -> Option<&DockInfo> {
        self.docks.iter().find(|d| d.id == dock_id)
    }

    /// List all docks
    pub fn list_docks(&self) -> &[DockInfo] {
        &self.docks
    }

    /// Check if any dock is present
    pub fn is_docked(&self) -> bool {
        self.docks.iter().any(|d| d.state == DockState::Docked)
    }

    /// Get docked dock (if any)
    pub fn get_docked_dock(&self) -> Option<&DockInfo> {
        self.docks.iter().find(|d| d.state == DockState::Docked)
    }

    /// Register event callback
    pub fn register_callback(&mut self, cb: DockEventCallback) {
        self.callbacks.push(cb);
    }

    /// Get statistics
    pub fn stats(&self) -> &DockStats {
        &self.stats
    }

    /// Add device to dock
    pub fn add_device(&mut self, dock_id: u32, device: DockDevice) -> KResult<()> {
        let dock = self.docks.iter_mut().find(|d| d.id == dock_id)
            .ok_or(KError::NotFound)?;

        dock.devices.push(device);

        // Fire device connected event
        for cb in &self.callbacks {
            cb(DockEvent::DeviceConnected, dock_id);
        }

        Ok(())
    }

    /// Remove device from dock
    pub fn remove_device(&mut self, dock_id: u32, device_id: u32) -> KResult<()> {
        let dock = self.docks.iter_mut().find(|d| d.id == dock_id)
            .ok_or(KError::NotFound)?;

        let pos = dock.devices.iter().position(|d| d.id == device_id)
            .ok_or(KError::NotFound)?;

        dock.devices.remove(pos);

        // Fire device disconnected event
        for cb in &self.callbacks {
            cb(DockEvent::DeviceDisconnected, dock_id);
        }

        Ok(())
    }

    /// Get devices for dock
    pub fn get_devices(&self, dock_id: u32) -> Option<&[DockDevice]> {
        self.docks.iter()
            .find(|d| d.id == dock_id)
            .map(|d| d.devices.as_slice())
    }
}

/// Initialize ACPI dock subsystem
pub fn init() -> KResult<()> {
    DOCK_MANAGER.lock().init()
}

/// Check if docked
pub fn is_docked() -> bool {
    DOCK_MANAGER.lock().is_docked()
}

/// Request eject
pub fn request_eject(dock_id: u32) -> KResult<()> {
    DOCK_MANAGER.lock().request_eject(dock_id)
}

/// Handle ACPI dock event
pub fn handle_acpi_event(acpi_path: &str, event_type: u32) {
    DOCK_MANAGER.lock().handle_acpi_event(acpi_path, event_type);
}

/// List docks
pub fn list_docks() -> Vec<DockInfo> {
    DOCK_MANAGER.lock().list_docks().to_vec()
}

/// Get dock info
pub fn get_dock(dock_id: u32) -> Option<DockInfo> {
    DOCK_MANAGER.lock().get_dock(dock_id).cloned()
}

/// Register event callback
pub fn register_callback(cb: DockEventCallback) {
    DOCK_MANAGER.lock().register_callback(cb);
}
