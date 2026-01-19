//! Wake on USB Support
//!
//! Enables waking the system from sleep states via USB device activity:
//! - USB device insertion/removal
//! - HID device activity (keyboard, mouse)
//! - Remote wakeup signaling
//! - Per-port wake configuration

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::sync::IrqSafeMutex;
use crate::util::{KResult, KError};

/// USB wake capability flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum UsbWakeCapability {
    /// No wake capability
    None = 0,
    /// Wake on device connect
    Connect = 1,
    /// Wake on device disconnect
    Disconnect = 2,
    /// Wake on remote wakeup signal from device
    RemoteWakeup = 4,
    /// Wake on over-current condition
    OverCurrent = 8,
}

/// USB port power state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum UsbPortPowerState {
    /// Port powered off
    Off = 0,
    /// Port in low power mode
    Suspend = 1,
    /// Port active but low power
    L1 = 2,
    /// Port fully powered
    Active = 3,
}

/// USB wake event type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbWakeEvent {
    /// Device connected
    DeviceConnect { port: u8, device_id: u16 },
    /// Device disconnected
    DeviceDisconnect { port: u8 },
    /// Remote wakeup from device
    RemoteWakeup { port: u8, device_id: u16 },
    /// HID activity (keyboard/mouse)
    HidActivity { port: u8, device_id: u16 },
    /// Over-current detected
    OverCurrent { port: u8 },
    /// Port status changed
    PortStatusChange { port: u8 },
}

/// USB wake configuration for a single port
#[derive(Debug, Clone)]
pub struct UsbPortWakeConfig {
    /// Port number
    pub port: u8,
    /// Controller index (for multiple USB controllers)
    pub controller: u8,
    /// Wake on connect enabled
    pub wake_on_connect: bool,
    /// Wake on disconnect enabled
    pub wake_on_disconnect: bool,
    /// Wake on remote wakeup enabled
    pub wake_on_remote: bool,
    /// Wake on over-current enabled
    pub wake_on_overcurrent: bool,
    /// Port is currently enabled for wake
    pub enabled: bool,
    /// Device currently attached
    pub device_attached: bool,
    /// Device supports remote wakeup
    pub device_supports_wakeup: bool,
}

impl Default for UsbPortWakeConfig {
    fn default() -> Self {
        UsbPortWakeConfig {
            port: 0,
            controller: 0,
            wake_on_connect: true,
            wake_on_disconnect: false,
            wake_on_remote: true,
            wake_on_overcurrent: true,
            enabled: false,
            device_attached: false,
            device_supports_wakeup: false,
        }
    }
}

/// USB controller wake configuration
#[derive(Debug, Clone)]
pub struct UsbControllerConfig {
    /// Controller index
    pub index: u8,
    /// Controller type
    pub controller_type: UsbControllerType,
    /// Base address
    pub base_address: u64,
    /// Number of ports
    pub num_ports: u8,
    /// Controller supports wake
    pub wake_capable: bool,
    /// Per-port configurations
    pub ports: Vec<UsbPortWakeConfig>,
}

/// USB controller type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbControllerType {
    Uhci,
    Ohci,
    Ehci,
    Xhci,
    Unknown,
}

impl Default for UsbControllerType {
    fn default() -> Self {
        UsbControllerType::Unknown
    }
}

/// USB wake reason for resume
#[derive(Debug, Clone)]
pub struct UsbWakeReason {
    /// The event that caused wake
    pub event: UsbWakeEvent,
    /// Timestamp of wake
    pub timestamp: u64,
    /// Controller that triggered wake
    pub controller: u8,
    /// Device class if known
    pub device_class: Option<u8>,
    /// Device subclass if known
    pub device_subclass: Option<u8>,
    /// Vendor ID if known
    pub vendor_id: Option<u16>,
    /// Product ID if known
    pub product_id: Option<u16>,
}

/// Wake on USB manager
pub struct WakeOnUsbManager {
    /// Enabled globally
    enabled: bool,
    /// USB controllers
    controllers: Vec<UsbControllerConfig>,
    /// Event history
    wake_history: Vec<UsbWakeReason>,
    /// Maximum history entries
    max_history: usize,
    /// Statistics
    stats: WakeOnUsbStats,
    /// Callback for wake events
    callbacks: Vec<fn(UsbWakeEvent)>,
    /// Initialized
    initialized: bool,
}

/// Wake on USB statistics
#[derive(Debug, Default)]
pub struct WakeOnUsbStats {
    /// Total wake events
    pub total_wakes: AtomicU64,
    /// Connect wakes
    pub connect_wakes: AtomicU64,
    /// Disconnect wakes
    pub disconnect_wakes: AtomicU64,
    /// Remote wakeup count
    pub remote_wakes: AtomicU64,
    /// HID activity wakes
    pub hid_wakes: AtomicU64,
    /// Over-current events
    pub overcurrent_events: AtomicU64,
}

pub static WAKE_USB_MANAGER: IrqSafeMutex<WakeOnUsbManager> = IrqSafeMutex::new(WakeOnUsbManager::new());

impl WakeOnUsbManager {
    pub const fn new() -> Self {
        WakeOnUsbManager {
            enabled: false,
            controllers: Vec::new(),
            wake_history: Vec::new(),
            max_history: 100,
            stats: WakeOnUsbStats {
                total_wakes: AtomicU64::new(0),
                connect_wakes: AtomicU64::new(0),
                disconnect_wakes: AtomicU64::new(0),
                remote_wakes: AtomicU64::new(0),
                hid_wakes: AtomicU64::new(0),
                overcurrent_events: AtomicU64::new(0),
            },
            callbacks: Vec::new(),
            initialized: false,
        }
    }

    /// Initialize the wake on USB manager
    pub fn init(&mut self) -> KResult<()> {
        if self.initialized {
            return Ok(());
        }

        // Enumerate USB controllers
        self.enumerate_controllers()?;

        // Configure default wake settings
        for controller in &mut self.controllers {
            if controller.wake_capable {
                for port in &mut controller.ports {
                    port.enabled = true;
                    port.wake_on_connect = true;
                    port.wake_on_remote = true;
                }
            }
        }

        self.enabled = true;
        self.initialized = true;
        crate::kprintln!("wake_usb: initialized with {} controllers", self.controllers.len());
        Ok(())
    }

    /// Enumerate USB controllers from PCI
    fn enumerate_controllers(&mut self) -> KResult<()> {
        // Query PCI for USB controllers
        let devices = crate::drivers::pci::scan();
        let mut controller_idx = 0u8;

        for device in devices {
            // USB controllers have class code 0x0C, subclass 0x03
            if device.class.class_code == 0x0C && device.class.subclass == 0x03 {
                let controller_type = match device.class.prog_if {
                    0x00 => UsbControllerType::Uhci,
                    0x10 => UsbControllerType::Ohci,
                    0x20 => UsbControllerType::Ehci,
                    0x30 => UsbControllerType::Xhci,
                    _ => UsbControllerType::Unknown,
                };

                // Read BAR0 for base address
                let (bar0, _is_mmio) = crate::drivers::pci::read_bar(&device, 0);

                let num_ports = match controller_type {
                    UsbControllerType::Uhci => 2,
                    UsbControllerType::Ohci => 4,
                    UsbControllerType::Ehci => 6,
                    UsbControllerType::Xhci => self.detect_xhci_ports(bar0),
                    UsbControllerType::Unknown => 4,
                };

                let mut ports = Vec::new();
                for port_num in 0..num_ports {
                    let mut config = UsbPortWakeConfig::default();
                    config.port = port_num;
                    config.controller = controller_idx;
                    ports.push(config);
                }

                let config = UsbControllerConfig {
                    index: controller_idx,
                    controller_type,
                    base_address: bar0,
                    num_ports,
                    wake_capable: true, // Most modern controllers support wake
                    ports,
                };

                self.controllers.push(config);
                controller_idx += 1;
            }
        }

        Ok(())
    }

    /// Detect number of ports on an xHCI controller
    fn detect_xhci_ports(&self, base: u64) -> u8 {
        if base == 0 {
            return 16; // Default
        }

        // Read HCSPARAMS1 from xHCI capability registers
        // The number of ports is in bits 24-31
        unsafe {
            let hcsparams1 = core::ptr::read_volatile((base + 0x04) as *const u32);
            ((hcsparams1 >> 24) & 0xFF) as u8
        }
    }

    /// Enable wake on USB globally
    pub fn enable(&mut self) {
        self.enabled = true;
        self.apply_all_settings();
        crate::kprintln!("wake_usb: enabled");
    }

    /// Disable wake on USB globally
    pub fn disable(&mut self) {
        self.enabled = false;
        self.clear_all_wake_settings();
        crate::kprintln!("wake_usb: disabled");
    }

    /// Check if enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Configure wake for a specific port
    pub fn configure_port(
        &mut self,
        controller: u8,
        port: u8,
        wake_connect: bool,
        wake_disconnect: bool,
        wake_remote: bool,
    ) -> KResult<()> {
        let ctrl = self.controllers
            .iter_mut()
            .find(|c| c.index == controller)
            .ok_or(KError::NotFound)?;

        let port_cfg = ctrl.ports
            .iter_mut()
            .find(|p| p.port == port)
            .ok_or(KError::NotFound)?;

        port_cfg.wake_on_connect = wake_connect;
        port_cfg.wake_on_disconnect = wake_disconnect;
        port_cfg.wake_on_remote = wake_remote;

        if self.enabled {
            self.apply_port_setting(controller, port)?;
        }

        Ok(())
    }

    /// Enable wake for a port
    pub fn enable_port(&mut self, controller: u8, port: u8) -> KResult<()> {
        let ctrl = self.controllers
            .iter_mut()
            .find(|c| c.index == controller)
            .ok_or(KError::NotFound)?;

        let port_cfg = ctrl.ports
            .iter_mut()
            .find(|p| p.port == port)
            .ok_or(KError::NotFound)?;

        port_cfg.enabled = true;

        if self.enabled {
            self.apply_port_setting(controller, port)?;
        }

        Ok(())
    }

    /// Disable wake for a port
    pub fn disable_port(&mut self, controller: u8, port: u8) -> KResult<()> {
        let ctrl = self.controllers
            .iter_mut()
            .find(|c| c.index == controller)
            .ok_or(KError::NotFound)?;

        let port_cfg = ctrl.ports
            .iter_mut()
            .find(|p| p.port == port)
            .ok_or(KError::NotFound)?;

        port_cfg.enabled = false;
        self.clear_port_wake_setting(controller, port)?;

        Ok(())
    }

    /// Apply wake settings for a specific port
    fn apply_port_setting(&mut self, controller: u8, port: u8) -> KResult<()> {
        let ctrl = self.controllers
            .iter()
            .find(|c| c.index == controller)
            .ok_or(KError::NotFound)?;

        let port_cfg = ctrl.ports
            .iter()
            .find(|p| p.port == port)
            .ok_or(KError::NotFound)?;

        if !port_cfg.enabled {
            return Ok(());
        }

        match ctrl.controller_type {
            UsbControllerType::Xhci => self.apply_xhci_wake(ctrl, port_cfg),
            UsbControllerType::Ehci => self.apply_ehci_wake(ctrl, port_cfg),
            UsbControllerType::Uhci => self.apply_uhci_wake(ctrl, port_cfg),
            UsbControllerType::Ohci => self.apply_ohci_wake(ctrl, port_cfg),
            UsbControllerType::Unknown => Ok(()),
        }
    }

    /// Apply xHCI wake settings
    fn apply_xhci_wake(&self, ctrl: &UsbControllerConfig, port_cfg: &UsbPortWakeConfig) -> KResult<()> {
        let base = ctrl.base_address;
        if base == 0 {
            return Err(KError::Invalid);
        }

        // Calculate port status/control register offset
        // xHCI port registers start at operational base + 0x400
        let op_base = unsafe {
            let caplength = core::ptr::read_volatile(base as *const u8);
            base + caplength as u64
        };

        let port_sc_offset = 0x400 + (port_cfg.port as u64 * 0x10);
        let port_sc_addr = (op_base + port_sc_offset) as *mut u32;

        unsafe {
            let mut port_sc = core::ptr::read_volatile(port_sc_addr);

            // Clear wake enable bits first (bits 25-27)
            port_sc &= !(0x7 << 25);

            // Set wake enables based on configuration
            if port_cfg.wake_on_connect {
                port_sc |= 1 << 25; // Wake on Connect Enable
            }
            if port_cfg.wake_on_disconnect {
                port_sc |= 1 << 26; // Wake on Disconnect Enable
            }
            if port_cfg.wake_on_overcurrent {
                port_sc |= 1 << 27; // Wake on Over-current Enable
            }

            core::ptr::write_volatile(port_sc_addr, port_sc);
        }

        Ok(())
    }

    /// Apply EHCI wake settings
    fn apply_ehci_wake(&self, ctrl: &UsbControllerConfig, port_cfg: &UsbPortWakeConfig) -> KResult<()> {
        let base = ctrl.base_address;
        if base == 0 {
            return Err(KError::Invalid);
        }

        // EHCI port status/control registers start at operational base + 0x44
        let cap_length = unsafe {
            core::ptr::read_volatile(base as *const u8)
        };
        let op_base = base + cap_length as u64;
        let port_sc_addr = (op_base + 0x44 + (port_cfg.port as u64 * 4)) as *mut u32;

        unsafe {
            let mut port_sc = core::ptr::read_volatile(port_sc_addr);

            // EHCI PORTSC wake bits
            // Bit 20: Wake on Connect Enable
            // Bit 21: Wake on Disconnect Enable
            // Bit 22: Wake on Over-current Enable
            port_sc &= !(0x7 << 20);

            if port_cfg.wake_on_connect {
                port_sc |= 1 << 20;
            }
            if port_cfg.wake_on_disconnect {
                port_sc |= 1 << 21;
            }
            if port_cfg.wake_on_overcurrent {
                port_sc |= 1 << 22;
            }

            core::ptr::write_volatile(port_sc_addr, port_sc);
        }

        Ok(())
    }

    /// Apply UHCI wake settings
    fn apply_uhci_wake(&self, ctrl: &UsbControllerConfig, port_cfg: &UsbPortWakeConfig) -> KResult<()> {
        let base = ctrl.base_address;
        if base == 0 {
            return Err(KError::Invalid);
        }

        // UHCI port status/control at base + 0x10 + (port * 2)
        let port_sc_addr = (base + 0x10 + (port_cfg.port as u64 * 2)) as u16;

        unsafe {
            use x86_64::instructions::port::Port;
            let mut port: Port<u16> = Port::new(port_sc_addr);
            let mut port_sc = port.read();

            // UHCI PORTSC bits for wake
            // Bit 6: Resume Detect
            if port_cfg.wake_on_remote {
                port_sc |= 1 << 6;
            } else {
                port_sc &= !(1 << 6);
            }

            port.write(port_sc);
        }

        Ok(())
    }

    /// Apply OHCI wake settings
    fn apply_ohci_wake(&self, ctrl: &UsbControllerConfig, port_cfg: &UsbPortWakeConfig) -> KResult<()> {
        let base = ctrl.base_address;
        if base == 0 {
            return Err(KError::Invalid);
        }

        // OHCI HcRhPortStatus at base + 0x54 + (port * 4)
        let port_status_addr = (base + 0x54 + (port_cfg.port as u64 * 4)) as *mut u32;

        unsafe {
            let mut port_status = core::ptr::read_volatile(port_status_addr);

            // OHCI uses different mechanism - enable port for wake
            // Set Port Power Status bit to enable wake detection
            if port_cfg.wake_on_connect || port_cfg.wake_on_disconnect || port_cfg.wake_on_remote {
                port_status |= 1 << 8; // Port Power
            }

            core::ptr::write_volatile(port_status_addr, port_status);
        }

        Ok(())
    }

    /// Apply all wake settings
    fn apply_all_settings(&mut self) {
        for ctrl_idx in 0..self.controllers.len() {
            let controller = self.controllers[ctrl_idx].index;
            let num_ports = self.controllers[ctrl_idx].num_ports;
            for port in 0..num_ports {
                let _ = self.apply_port_setting(controller, port);
            }
        }
    }

    /// Clear wake setting for a port
    fn clear_port_wake_setting(&self, controller: u8, port: u8) -> KResult<()> {
        let ctrl = self.controllers
            .iter()
            .find(|c| c.index == controller)
            .ok_or(KError::NotFound)?;

        match ctrl.controller_type {
            UsbControllerType::Xhci => {
                let base = ctrl.base_address;
                if base == 0 {
                    return Ok(());
                }

                let op_base = unsafe {
                    let caplength = core::ptr::read_volatile(base as *const u8);
                    base + caplength as u64
                };

                let port_sc_offset = 0x400 + (port as u64 * 0x10);
                let port_sc_addr = (op_base + port_sc_offset) as *mut u32;

                unsafe {
                    let mut port_sc = core::ptr::read_volatile(port_sc_addr);
                    port_sc &= !(0x7 << 25); // Clear all wake enables
                    core::ptr::write_volatile(port_sc_addr, port_sc);
                }
            }
            UsbControllerType::Ehci => {
                let base = ctrl.base_address;
                if base == 0 {
                    return Ok(());
                }

                let cap_length = unsafe {
                    core::ptr::read_volatile(base as *const u8)
                };
                let op_base = base + cap_length as u64;
                let port_sc_addr = (op_base + 0x44 + (port as u64 * 4)) as *mut u32;

                unsafe {
                    let mut port_sc = core::ptr::read_volatile(port_sc_addr);
                    port_sc &= !(0x7 << 20);
                    core::ptr::write_volatile(port_sc_addr, port_sc);
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Clear all wake settings
    fn clear_all_wake_settings(&self) {
        for ctrl in &self.controllers {
            for port_cfg in &ctrl.ports {
                let _ = self.clear_port_wake_setting(ctrl.index, port_cfg.port);
            }
        }
    }

    /// Handle a USB wake event
    pub fn handle_wake_event(&mut self, event: UsbWakeEvent) {
        // Update statistics
        self.stats.total_wakes.fetch_add(1, Ordering::Relaxed);

        match &event {
            UsbWakeEvent::DeviceConnect { .. } => {
                self.stats.connect_wakes.fetch_add(1, Ordering::Relaxed);
            }
            UsbWakeEvent::DeviceDisconnect { .. } => {
                self.stats.disconnect_wakes.fetch_add(1, Ordering::Relaxed);
            }
            UsbWakeEvent::RemoteWakeup { .. } => {
                self.stats.remote_wakes.fetch_add(1, Ordering::Relaxed);
            }
            UsbWakeEvent::HidActivity { .. } => {
                self.stats.hid_wakes.fetch_add(1, Ordering::Relaxed);
            }
            UsbWakeEvent::OverCurrent { .. } => {
                self.stats.overcurrent_events.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }

        // Get port and controller info
        let (port, controller) = match &event {
            UsbWakeEvent::DeviceConnect { port, .. } => (*port, 0),
            UsbWakeEvent::DeviceDisconnect { port } => (*port, 0),
            UsbWakeEvent::RemoteWakeup { port, .. } => (*port, 0),
            UsbWakeEvent::HidActivity { port, .. } => (*port, 0),
            UsbWakeEvent::OverCurrent { port } => (*port, 0),
            UsbWakeEvent::PortStatusChange { port } => (*port, 0),
        };

        // Record wake reason
        let reason = UsbWakeReason {
            event: event.clone(),
            timestamp: crate::time::uptime_ms(),
            controller,
            device_class: None,
            device_subclass: None,
            vendor_id: None,
            product_id: None,
        };

        if self.wake_history.len() >= self.max_history {
            self.wake_history.remove(0);
        }
        self.wake_history.push(reason);

        // Fire callbacks
        for cb in &self.callbacks {
            cb(event.clone());
        }

        crate::kprintln!("wake_usb: wake event {:?}", event);
    }

    /// Register callback for wake events
    pub fn register_callback(&mut self, cb: fn(UsbWakeEvent)) {
        self.callbacks.push(cb);
    }

    /// Get wake history
    pub fn wake_history(&self) -> &[UsbWakeReason] {
        &self.wake_history
    }

    /// Get last wake reason
    pub fn last_wake_reason(&self) -> Option<&UsbWakeReason> {
        self.wake_history.last()
    }

    /// Get statistics
    pub fn stats(&self) -> &WakeOnUsbStats {
        &self.stats
    }

    /// List all controllers
    pub fn controllers(&self) -> &[UsbControllerConfig] {
        &self.controllers
    }

    /// Get controller info
    pub fn controller(&self, index: u8) -> Option<&UsbControllerConfig> {
        self.controllers.iter().find(|c| c.index == index)
    }

    /// Update device attachment status
    pub fn update_device_status(&mut self, controller: u8, port: u8, attached: bool, supports_wakeup: bool) {
        if let Some(ctrl) = self.controllers.iter_mut().find(|c| c.index == controller) {
            if let Some(port_cfg) = ctrl.ports.iter_mut().find(|p| p.port == port) {
                port_cfg.device_attached = attached;
                port_cfg.device_supports_wakeup = supports_wakeup;
            }
        }
    }

    /// Prepare for system suspend
    pub fn prepare_suspend(&mut self) -> KResult<()> {
        if !self.enabled {
            return Ok(());
        }

        crate::kprintln!("wake_usb: preparing for suspend");

        // Ensure wake settings are applied
        self.apply_all_settings();

        // Enable USB controller wake capability via PCI PM
        for ctrl in &self.controllers {
            self.enable_pci_wake(ctrl)?;
        }

        Ok(())
    }

    /// Enable PCI wake capability for controller
    fn enable_pci_wake(&self, ctrl: &UsbControllerConfig) -> KResult<()> {
        // Find PCI device and enable PME
        let devices = crate::drivers::pci::scan();

        for device in devices {
            if device.class.class_code == 0x0C && device.class.subclass == 0x03 {
                let (bar0, _) = crate::drivers::pci::read_bar(&device, 0);
                if bar0 == ctrl.base_address {
                    // Enable PME (Power Management Event)
                    // This is done through PCI PM capability
                    if let Some(pm_cap) = crate::drivers::pci::find_capability(&device, 0x01) {
                        let pmcsr = crate::drivers::pci::read_u16(
                            device.addr.bus, device.addr.device, device.addr.function,
                            pm_cap + 4
                        );
                        // Set PME_Enable (bit 8)
                        crate::drivers::pci::write_u16(
                            device.addr.bus, device.addr.device, device.addr.function,
                            pm_cap + 4, pmcsr | (1 << 8)
                        );
                    }
                    break;
                }
            }
        }

        Ok(())
    }

    /// Resume from suspend
    pub fn resume(&mut self) -> KResult<()> {
        if !self.enabled {
            return Ok(());
        }

        crate::kprintln!("wake_usb: resuming");

        // Check for pending wake events
        for ctrl in &self.controllers {
            self.check_wake_status(ctrl);
        }

        Ok(())
    }

    /// Check wake status on controller
    fn check_wake_status(&self, ctrl: &UsbControllerConfig) {
        match ctrl.controller_type {
            UsbControllerType::Xhci => self.check_xhci_wake_status(ctrl),
            UsbControllerType::Ehci => self.check_ehci_wake_status(ctrl),
            _ => {}
        }
    }

    /// Check xHCI wake status
    fn check_xhci_wake_status(&self, ctrl: &UsbControllerConfig) {
        let base = ctrl.base_address;
        if base == 0 {
            return;
        }

        let op_base = unsafe {
            let caplength = core::ptr::read_volatile(base as *const u8);
            base + caplength as u64
        };

        for port in 0..ctrl.num_ports {
            let port_sc_offset = 0x400 + (port as u64 * 0x10);
            let port_sc_addr = (op_base + port_sc_offset) as *const u32;

            unsafe {
                let port_sc = core::ptr::read_volatile(port_sc_addr);

                // Check Connect Status Change (bit 17)
                if port_sc & (1 << 17) != 0 {
                    crate::kprintln!("wake_usb: port {} connect status changed", port);
                }

                // Check Port Link State Change (bit 22)
                if port_sc & (1 << 22) != 0 {
                    crate::kprintln!("wake_usb: port {} link state changed", port);
                }
            }
        }
    }

    /// Check EHCI wake status
    fn check_ehci_wake_status(&self, ctrl: &UsbControllerConfig) {
        let base = ctrl.base_address;
        if base == 0 {
            return;
        }

        let cap_length = unsafe {
            core::ptr::read_volatile(base as *const u8)
        };
        let op_base = base + cap_length as u64;

        for port in 0..ctrl.num_ports {
            let port_sc_addr = (op_base + 0x44 + (port as u64 * 4)) as *const u32;

            unsafe {
                let port_sc = core::ptr::read_volatile(port_sc_addr);

                // Check Connect Status Change (bit 1)
                if port_sc & (1 << 1) != 0 {
                    crate::kprintln!("wake_usb: port {} connect status changed", port);
                }
            }
        }
    }
}

/// Initialize wake on USB subsystem
pub fn init() -> KResult<()> {
    WAKE_USB_MANAGER.lock().init()
}

/// Enable wake on USB
pub fn enable() {
    WAKE_USB_MANAGER.lock().enable();
}

/// Disable wake on USB
pub fn disable() {
    WAKE_USB_MANAGER.lock().disable();
}

/// Check if enabled
pub fn is_enabled() -> bool {
    WAKE_USB_MANAGER.lock().is_enabled()
}

/// Configure port wake settings
pub fn configure_port(
    controller: u8,
    port: u8,
    wake_connect: bool,
    wake_disconnect: bool,
    wake_remote: bool,
) -> KResult<()> {
    WAKE_USB_MANAGER.lock().configure_port(controller, port, wake_connect, wake_disconnect, wake_remote)
}

/// Enable wake for a port
pub fn enable_port(controller: u8, port: u8) -> KResult<()> {
    WAKE_USB_MANAGER.lock().enable_port(controller, port)
}

/// Disable wake for a port
pub fn disable_port(controller: u8, port: u8) -> KResult<()> {
    WAKE_USB_MANAGER.lock().disable_port(controller, port)
}

/// Prepare for suspend
pub fn prepare_suspend() -> KResult<()> {
    WAKE_USB_MANAGER.lock().prepare_suspend()
}

/// Resume from suspend
pub fn resume() -> KResult<()> {
    WAKE_USB_MANAGER.lock().resume()
}

/// Handle wake event
pub fn handle_wake_event(event: UsbWakeEvent) {
    WAKE_USB_MANAGER.lock().handle_wake_event(event);
}

/// Get last wake reason
pub fn last_wake_reason() -> Option<UsbWakeReason> {
    WAKE_USB_MANAGER.lock().last_wake_reason().cloned()
}
