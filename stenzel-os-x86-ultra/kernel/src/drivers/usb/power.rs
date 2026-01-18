//! USB Power Management
//!
//! Implements USB power management features:
//! - Device suspend/resume
//! - Selective suspend (auto-suspend idle devices)
//! - Remote wakeup
//! - Link Power Management (LPM)
//! - USB Type-C Power Delivery basic support

#![allow(dead_code)]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use crate::sync::IrqSafeMutex;

/// USB power state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbPowerState {
    /// Device is active and operational
    Active,
    /// Device is suspended (low power)
    Suspended,
    /// Device is in selective suspend (auto-suspended)
    SelectiveSuspend,
    /// Device is in LPM L1 state
    LpmL1,
    /// Device is disconnected
    Disconnected,
    /// Device is in error state
    Error,
}

impl UsbPowerState {
    pub fn as_str(&self) -> &'static str {
        match self {
            UsbPowerState::Active => "Active",
            UsbPowerState::Suspended => "Suspended",
            UsbPowerState::SelectiveSuspend => "Selective Suspend",
            UsbPowerState::LpmL1 => "LPM L1",
            UsbPowerState::Disconnected => "Disconnected",
            UsbPowerState::Error => "Error",
        }
    }

    pub fn is_low_power(&self) -> bool {
        matches!(
            self,
            UsbPowerState::Suspended | UsbPowerState::SelectiveSuspend | UsbPowerState::LpmL1
        )
    }
}

/// USB Link Power Management state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LpmState {
    /// L0 - Active
    L0Active,
    /// L1 - Sleep (fast wake)
    L1Sleep,
    /// L2 - Suspend
    L2Suspend,
    /// L3 - Off
    L3Off,
}

impl LpmState {
    pub fn as_str(&self) -> &'static str {
        match self {
            LpmState::L0Active => "L0 (Active)",
            LpmState::L1Sleep => "L1 (Sleep)",
            LpmState::L2Suspend => "L2 (Suspend)",
            LpmState::L3Off => "L3 (Off)",
        }
    }
}

/// Remote wakeup capability
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteWakeup {
    /// Device does not support remote wakeup
    NotSupported,
    /// Remote wakeup supported but disabled
    Disabled,
    /// Remote wakeup enabled
    Enabled,
}

/// USB Power Delivery role
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdRole {
    /// No Power Delivery
    None,
    /// Sink (receiving power)
    Sink,
    /// Source (providing power)
    Source,
    /// Dual role (can be either)
    DualRole,
}

/// USB Power Delivery power level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdPowerLevel {
    /// Standard USB (5V, 500mA = 2.5W)
    Usb2Standard,
    /// USB 3.0 (5V, 900mA = 4.5W)
    Usb3Standard,
    /// USB BC 1.2 (5V, 1.5A = 7.5W)
    Bc12,
    /// USB Type-C 1.5A (5V, 1.5A = 7.5W)
    TypeC1_5A,
    /// USB Type-C 3.0A (5V, 3.0A = 15W)
    TypeC3A,
    /// USB PD (various levels up to 100W)
    PowerDelivery(u32), // watts
}

impl PdPowerLevel {
    pub fn max_power_mw(&self) -> u32 {
        match self {
            PdPowerLevel::Usb2Standard => 2500,
            PdPowerLevel::Usb3Standard => 4500,
            PdPowerLevel::Bc12 => 7500,
            PdPowerLevel::TypeC1_5A => 7500,
            PdPowerLevel::TypeC3A => 15000,
            PdPowerLevel::PowerDelivery(w) => w * 1000,
        }
    }
}

/// Auto-suspend configuration
#[derive(Debug, Clone)]
pub struct AutoSuspendConfig {
    /// Enable auto-suspend
    pub enabled: bool,
    /// Idle timeout before auto-suspend (milliseconds)
    pub idle_timeout_ms: u32,
    /// Minimum active time after resume (milliseconds)
    pub min_active_time_ms: u32,
    /// Allow remote wakeup
    pub allow_remote_wakeup: bool,
}

impl Default for AutoSuspendConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            idle_timeout_ms: 2000,   // 2 seconds default
            min_active_time_ms: 100, // 100ms minimum
            allow_remote_wakeup: true,
        }
    }
}

/// Device power information
#[derive(Debug, Clone)]
pub struct DevicePowerInfo {
    /// Device address
    pub address: u8,
    /// Current power state
    pub state: UsbPowerState,
    /// LPM state
    pub lpm_state: LpmState,
    /// Remote wakeup capability
    pub remote_wakeup: RemoteWakeup,
    /// Power Delivery role
    pub pd_role: PdRole,
    /// Power level
    pub power_level: PdPowerLevel,
    /// Max power consumption (mW)
    pub max_power_mw: u32,
    /// Current power consumption estimate (mW)
    pub current_power_mw: u32,
    /// Last activity timestamp (ms since boot)
    pub last_activity_ms: u64,
    /// Auto-suspend configuration
    pub auto_suspend: AutoSuspendConfig,
    /// Device supports LPM
    pub lpm_capable: bool,
    /// Device supports USB 3.0 U1/U2 states
    pub u1u2_capable: bool,
    /// BESL (Best Effort Service Latency) for LPM
    pub besl: u8,
}

impl DevicePowerInfo {
    pub fn new(address: u8) -> Self {
        Self {
            address,
            state: UsbPowerState::Active,
            lpm_state: LpmState::L0Active,
            remote_wakeup: RemoteWakeup::NotSupported,
            pd_role: PdRole::None,
            power_level: PdPowerLevel::Usb2Standard,
            max_power_mw: 500, // Default USB 2.0
            current_power_mw: 100,
            last_activity_ms: 0,
            auto_suspend: AutoSuspendConfig::default(),
            lpm_capable: false,
            u1u2_capable: false,
            besl: 0,
        }
    }

    /// Check if device should be auto-suspended
    pub fn should_auto_suspend(&self, current_time_ms: u64) -> bool {
        if !self.auto_suspend.enabled {
            return false;
        }
        if self.state != UsbPowerState::Active {
            return false;
        }
        let idle_time = current_time_ms.saturating_sub(self.last_activity_ms);
        idle_time >= self.auto_suspend.idle_timeout_ms as u64
    }

    /// Update last activity time
    pub fn touch(&mut self, current_time_ms: u64) {
        self.last_activity_ms = current_time_ms;
    }
}

/// Port power information
#[derive(Debug, Clone)]
pub struct PortPowerInfo {
    /// Port number
    pub port: u8,
    /// Port is powered
    pub powered: bool,
    /// Port power state
    pub state: UsbPowerState,
    /// Device connected
    pub device_connected: bool,
    /// Connected device address (if any)
    pub device_address: Option<u8>,
    /// Port supports PD
    pub pd_capable: bool,
    /// Current power level
    pub power_level: PdPowerLevel,
    /// Over-current condition
    pub over_current: bool,
}

impl PortPowerInfo {
    pub fn new(port: u8) -> Self {
        Self {
            port,
            powered: true,
            state: UsbPowerState::Active,
            device_connected: false,
            device_address: None,
            pd_capable: false,
            power_level: PdPowerLevel::Usb2Standard,
            over_current: false,
        }
    }
}

/// Power management error
#[derive(Debug, Clone)]
pub enum PowerError {
    DeviceNotFound,
    PortNotFound,
    InvalidState,
    SuspendFailed,
    ResumeFailed,
    RemoteWakeupNotSupported,
    OverCurrent,
    Timeout,
    NotSupported,
}

pub type PowerResult<T> = Result<T, PowerError>;

/// USB Power Manager
pub struct UsbPowerManager {
    /// Device power info
    devices: BTreeMap<u8, DevicePowerInfo>,
    /// Port power info
    ports: BTreeMap<u8, PortPowerInfo>,
    /// Global auto-suspend enabled
    auto_suspend_enabled: AtomicBool,
    /// Total power consumption (mW)
    total_power_mw: AtomicU32,
    /// Power budget (mW)
    power_budget_mw: u32,
    /// System time for idle tracking (ms)
    current_time_ms: AtomicU64,
    /// LPM globally enabled
    lpm_enabled: AtomicBool,
    /// Suspend callbacks
    suspend_callbacks: Vec<fn(u8)>,
    /// Resume callbacks
    resume_callbacks: Vec<fn(u8)>,
}

impl UsbPowerManager {
    pub const fn new() -> Self {
        Self {
            devices: BTreeMap::new(),
            ports: BTreeMap::new(),
            auto_suspend_enabled: AtomicBool::new(true),
            total_power_mw: AtomicU32::new(0),
            power_budget_mw: 500_000, // 500W default budget
            current_time_ms: AtomicU64::new(0),
            lpm_enabled: AtomicBool::new(true),
            suspend_callbacks: Vec::new(),
            resume_callbacks: Vec::new(),
        }
    }

    /// Register a new device
    pub fn register_device(&mut self, address: u8, max_power_mw: u32) {
        let mut info = DevicePowerInfo::new(address);
        info.max_power_mw = max_power_mw;
        info.current_power_mw = max_power_mw;
        info.last_activity_ms = self.current_time_ms.load(Ordering::Relaxed);

        self.devices.insert(address, info);
        self.update_total_power();
    }

    /// Unregister a device
    pub fn unregister_device(&mut self, address: u8) {
        self.devices.remove(&address);
        self.update_total_power();
    }

    /// Register a port
    pub fn register_port(&mut self, port: u8) {
        self.ports.insert(port, PortPowerInfo::new(port));
    }

    /// Get device power info
    pub fn get_device_info(&self, address: u8) -> Option<&DevicePowerInfo> {
        self.devices.get(&address)
    }

    /// Get mutable device power info
    pub fn get_device_info_mut(&mut self, address: u8) -> Option<&mut DevicePowerInfo> {
        self.devices.get_mut(&address)
    }

    /// Get port power info
    pub fn get_port_info(&self, port: u8) -> Option<&PortPowerInfo> {
        self.ports.get(&port)
    }

    /// Suspend a device
    pub fn suspend_device(&mut self, address: u8) -> PowerResult<()> {
        let device = self.devices.get_mut(&address).ok_or(PowerError::DeviceNotFound)?;

        if device.state == UsbPowerState::Suspended {
            return Ok(()); // Already suspended
        }

        if device.state != UsbPowerState::Active {
            return Err(PowerError::InvalidState);
        }

        // Notify callbacks
        for callback in &self.suspend_callbacks {
            callback(address);
        }

        device.state = UsbPowerState::Suspended;
        device.lpm_state = LpmState::L2Suspend;
        device.current_power_mw = 10; // Minimal power in suspend

        self.update_total_power();

        crate::kprintln!("usb_power: device {} suspended", address);
        Ok(())
    }

    /// Resume a device
    pub fn resume_device(&mut self, address: u8) -> PowerResult<()> {
        let device = self.devices.get_mut(&address).ok_or(PowerError::DeviceNotFound)?;

        if device.state == UsbPowerState::Active {
            return Ok(()); // Already active
        }

        if !device.state.is_low_power() {
            return Err(PowerError::InvalidState);
        }

        device.state = UsbPowerState::Active;
        device.lpm_state = LpmState::L0Active;
        device.current_power_mw = device.max_power_mw;
        device.last_activity_ms = self.current_time_ms.load(Ordering::Relaxed);

        // Notify callbacks
        for callback in &self.resume_callbacks {
            callback(address);
        }

        self.update_total_power();

        crate::kprintln!("usb_power: device {} resumed", address);
        Ok(())
    }

    /// Enable remote wakeup for a device
    pub fn enable_remote_wakeup(&mut self, address: u8) -> PowerResult<()> {
        let device = self.devices.get_mut(&address).ok_or(PowerError::DeviceNotFound)?;

        if device.remote_wakeup == RemoteWakeup::NotSupported {
            return Err(PowerError::RemoteWakeupNotSupported);
        }

        device.remote_wakeup = RemoteWakeup::Enabled;
        Ok(())
    }

    /// Disable remote wakeup for a device
    pub fn disable_remote_wakeup(&mut self, address: u8) -> PowerResult<()> {
        let device = self.devices.get_mut(&address).ok_or(PowerError::DeviceNotFound)?;

        if device.remote_wakeup == RemoteWakeup::NotSupported {
            return Err(PowerError::RemoteWakeupNotSupported);
        }

        device.remote_wakeup = RemoteWakeup::Disabled;
        Ok(())
    }

    /// Set LPM state for a device
    pub fn set_lpm_state(&mut self, address: u8, state: LpmState) -> PowerResult<()> {
        let device = self.devices.get_mut(&address).ok_or(PowerError::DeviceNotFound)?;

        if !device.lpm_capable {
            return Err(PowerError::NotSupported);
        }

        device.lpm_state = state;

        // Update power state based on LPM
        match state {
            LpmState::L0Active => {
                device.state = UsbPowerState::Active;
                device.current_power_mw = device.max_power_mw;
            }
            LpmState::L1Sleep => {
                device.state = UsbPowerState::LpmL1;
                device.current_power_mw = device.max_power_mw / 2;
            }
            LpmState::L2Suspend => {
                device.state = UsbPowerState::Suspended;
                device.current_power_mw = 10;
            }
            LpmState::L3Off => {
                device.state = UsbPowerState::Disconnected;
                device.current_power_mw = 0;
            }
        }

        self.update_total_power();
        Ok(())
    }

    /// Configure auto-suspend for a device
    pub fn configure_auto_suspend(&mut self, address: u8, config: AutoSuspendConfig) -> PowerResult<()> {
        let device = self.devices.get_mut(&address).ok_or(PowerError::DeviceNotFound)?;
        device.auto_suspend = config;
        Ok(())
    }

    /// Disable auto-suspend for a device
    pub fn disable_auto_suspend(&mut self, address: u8) -> PowerResult<()> {
        let device = self.devices.get_mut(&address).ok_or(PowerError::DeviceNotFound)?;
        device.auto_suspend.enabled = false;
        Ok(())
    }

    /// Record device activity (prevents auto-suspend)
    pub fn record_activity(&mut self, address: u8) {
        if let Some(device) = self.devices.get_mut(&address) {
            device.touch(self.current_time_ms.load(Ordering::Relaxed));
        }
    }

    /// Set port power state
    pub fn set_port_power(&mut self, port: u8, powered: bool) -> PowerResult<()> {
        let port_info = self.ports.get_mut(&port).ok_or(PowerError::PortNotFound)?;
        port_info.powered = powered;
        port_info.state = if powered {
            UsbPowerState::Active
        } else {
            UsbPowerState::Disconnected
        };
        Ok(())
    }

    /// Check for over-current on a port
    pub fn check_over_current(&mut self, port: u8) -> PowerResult<bool> {
        let port_info = self.ports.get(&port).ok_or(PowerError::PortNotFound)?;
        Ok(port_info.over_current)
    }

    /// Set over-current flag
    pub fn set_over_current(&mut self, port: u8, over_current: bool) -> PowerResult<()> {
        let port_info = self.ports.get_mut(&port).ok_or(PowerError::PortNotFound)?;
        port_info.over_current = over_current;
        if over_current {
            port_info.powered = false;
            port_info.state = UsbPowerState::Error;
            crate::kprintln!("usb_power: over-current on port {}", port);
        }
        Ok(())
    }

    /// Update total power consumption
    fn update_total_power(&self) {
        let total: u32 = self.devices.values().map(|d| d.current_power_mw).sum();
        self.total_power_mw.store(total, Ordering::Relaxed);
    }

    /// Get total power consumption
    pub fn total_power_mw(&self) -> u32 {
        self.total_power_mw.load(Ordering::Relaxed)
    }

    /// Check if within power budget
    pub fn within_budget(&self) -> bool {
        self.total_power_mw() <= self.power_budget_mw
    }

    /// Set power budget
    pub fn set_power_budget(&mut self, budget_mw: u32) {
        self.power_budget_mw = budget_mw;
    }

    /// Update system time (call periodically)
    pub fn update_time(&self, time_ms: u64) {
        self.current_time_ms.store(time_ms, Ordering::Relaxed);
    }

    /// Run periodic auto-suspend check
    pub fn periodic_check(&mut self) {
        if !self.auto_suspend_enabled.load(Ordering::Relaxed) {
            return;
        }

        let current_time = self.current_time_ms.load(Ordering::Relaxed);
        let mut to_suspend = Vec::new();

        for (addr, device) in &self.devices {
            if device.should_auto_suspend(current_time) {
                to_suspend.push(*addr);
            }
        }

        for addr in to_suspend {
            if let Err(e) = self.suspend_device(addr) {
                crate::kprintln!("usb_power: auto-suspend failed for device {}: {:?}", addr, e);
            }
        }
    }

    /// Enable global auto-suspend
    pub fn enable_global_auto_suspend(&self) {
        self.auto_suspend_enabled.store(true, Ordering::Relaxed);
    }

    /// Disable global auto-suspend
    pub fn disable_global_auto_suspend(&self) {
        self.auto_suspend_enabled.store(false, Ordering::Relaxed);
    }

    /// Enable global LPM
    pub fn enable_lpm(&self) {
        self.lpm_enabled.store(true, Ordering::Relaxed);
    }

    /// Disable global LPM
    pub fn disable_lpm(&self) {
        self.lpm_enabled.store(false, Ordering::Relaxed);
    }

    /// Is LPM enabled globally
    pub fn is_lpm_enabled(&self) -> bool {
        self.lpm_enabled.load(Ordering::Relaxed)
    }

    /// Register suspend callback
    pub fn on_suspend(&mut self, callback: fn(u8)) {
        self.suspend_callbacks.push(callback);
    }

    /// Register resume callback
    pub fn on_resume(&mut self, callback: fn(u8)) {
        self.resume_callbacks.push(callback);
    }

    /// Suspend all devices
    pub fn suspend_all(&mut self) -> PowerResult<()> {
        let addresses: Vec<u8> = self.devices.keys().copied().collect();
        for addr in addresses {
            self.suspend_device(addr)?;
        }
        Ok(())
    }

    /// Resume all devices
    pub fn resume_all(&mut self) -> PowerResult<()> {
        let addresses: Vec<u8> = self.devices.keys().copied().collect();
        for addr in addresses {
            self.resume_device(addr)?;
        }
        Ok(())
    }

    /// Get power statistics
    pub fn get_stats(&self) -> PowerStats {
        let mut active = 0;
        let mut suspended = 0;
        let mut lpm = 0;

        for device in self.devices.values() {
            match device.state {
                UsbPowerState::Active => active += 1,
                UsbPowerState::Suspended | UsbPowerState::SelectiveSuspend => suspended += 1,
                UsbPowerState::LpmL1 => lpm += 1,
                _ => {}
            }
        }

        let total_ports = self.ports.len() as u32;
        let powered_ports = self.ports.values().filter(|p| p.powered).count() as u32;
        let over_current_ports = self.ports.values().filter(|p| p.over_current).count() as u32;

        PowerStats {
            total_devices: self.devices.len() as u32,
            active_devices: active,
            suspended_devices: suspended,
            lpm_devices: lpm,
            total_power_mw: self.total_power_mw(),
            power_budget_mw: self.power_budget_mw,
            total_ports,
            powered_ports,
            over_current_ports,
            auto_suspend_enabled: self.auto_suspend_enabled.load(Ordering::Relaxed),
            lpm_enabled: self.lpm_enabled.load(Ordering::Relaxed),
        }
    }

    /// Format status as string
    pub fn format_status(&self) -> String {
        let stats = self.get_stats();
        let mut output = String::new();

        output.push_str("USB Power Management Status:\n");
        output.push_str(&alloc::format!(
            "  Devices: {} total, {} active, {} suspended, {} LPM\n",
            stats.total_devices, stats.active_devices, stats.suspended_devices, stats.lpm_devices
        ));
        output.push_str(&alloc::format!(
            "  Power: {} mW / {} mW budget\n",
            stats.total_power_mw, stats.power_budget_mw
        ));
        output.push_str(&alloc::format!(
            "  Ports: {} total, {} powered, {} over-current\n",
            stats.total_ports, stats.powered_ports, stats.over_current_ports
        ));
        output.push_str(&alloc::format!(
            "  Auto-suspend: {}, LPM: {}\n",
            if stats.auto_suspend_enabled { "enabled" } else { "disabled" },
            if stats.lpm_enabled { "enabled" } else { "disabled" }
        ));

        output
    }
}

/// Power statistics
#[derive(Debug, Clone)]
pub struct PowerStats {
    pub total_devices: u32,
    pub active_devices: u32,
    pub suspended_devices: u32,
    pub lpm_devices: u32,
    pub total_power_mw: u32,
    pub power_budget_mw: u32,
    pub total_ports: u32,
    pub powered_ports: u32,
    pub over_current_ports: u32,
    pub auto_suspend_enabled: bool,
    pub lpm_enabled: bool,
}

// =============================================================================
// Global State
// =============================================================================

static USB_POWER_MANAGER: IrqSafeMutex<UsbPowerManager> = IrqSafeMutex::new(UsbPowerManager::new());

/// Initialize USB power management
pub fn init() {
    crate::kprintln!("usb_power: USB power management initialized");
}

/// Get power manager lock
pub fn power_manager() -> impl core::ops::DerefMut<Target = UsbPowerManager> {
    USB_POWER_MANAGER.lock()
}

/// Register a device with power manager
pub fn register_device(address: u8, max_power_mw: u32) {
    USB_POWER_MANAGER.lock().register_device(address, max_power_mw);
}

/// Unregister a device
pub fn unregister_device(address: u8) {
    USB_POWER_MANAGER.lock().unregister_device(address);
}

/// Suspend a device
pub fn suspend_device(address: u8) -> PowerResult<()> {
    USB_POWER_MANAGER.lock().suspend_device(address)
}

/// Resume a device
pub fn resume_device(address: u8) -> PowerResult<()> {
    USB_POWER_MANAGER.lock().resume_device(address)
}

/// Record device activity
pub fn record_activity(address: u8) {
    USB_POWER_MANAGER.lock().record_activity(address);
}

/// Update time (call from timer)
pub fn update_time(time_ms: u64) {
    USB_POWER_MANAGER.lock().update_time(time_ms);
}

/// Run periodic power management check
pub fn periodic_check() {
    USB_POWER_MANAGER.lock().periodic_check();
}

/// Get total power consumption
pub fn total_power_mw() -> u32 {
    USB_POWER_MANAGER.lock().total_power_mw()
}

/// Get power stats
pub fn get_stats() -> PowerStats {
    USB_POWER_MANAGER.lock().get_stats()
}

/// Format status
pub fn format_status() -> String {
    USB_POWER_MANAGER.lock().format_status()
}

/// Suspend all USB devices (for system suspend)
pub fn suspend_all() -> PowerResult<()> {
    USB_POWER_MANAGER.lock().suspend_all()
}

/// Resume all USB devices (for system resume)
pub fn resume_all() -> PowerResult<()> {
    USB_POWER_MANAGER.lock().resume_all()
}

/// Enable global auto-suspend
pub fn enable_auto_suspend() {
    USB_POWER_MANAGER.lock().enable_global_auto_suspend();
}

/// Disable global auto-suspend
pub fn disable_auto_suspend() {
    USB_POWER_MANAGER.lock().disable_global_auto_suspend();
}
