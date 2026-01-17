//! Backlight Control
//!
//! Controls screen brightness on laptops and monitors.
//! Supports multiple backlight control methods:
//! - ACPI backlight (most compatible)
//! - Intel integrated graphics backlight
//! - Raw/platform backlight
//!
//! Provides a unified interface via /sys/class/backlight

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::sync::Arc;
use alloc::collections::BTreeMap;
use core::sync::atomic::{AtomicU32, Ordering};

use crate::sync::IrqSafeMutex;
use crate::util::{KResult, KError};

/// Backlight type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BacklightType {
    /// Raw backlight (direct hardware control)
    Raw,
    /// Platform-specific backlight
    Platform,
    /// Firmware (ACPI) controlled backlight
    Firmware,
    /// Intel integrated graphics backlight
    IntelBacklight,
}

/// Backlight device
pub struct BacklightDevice {
    /// Device name
    name: String,
    /// Type of backlight
    backlight_type: BacklightType,
    /// Maximum brightness value
    max_brightness: u32,
    /// Current brightness
    current: AtomicU32,
    /// Hardware operations
    ops: Arc<dyn BacklightOps>,
}

/// Backlight hardware operations
pub trait BacklightOps: Send + Sync {
    /// Get current brightness
    fn get_brightness(&self) -> u32;

    /// Set brightness
    fn set_brightness(&self, value: u32) -> KResult<()>;

    /// Get maximum brightness
    fn max_brightness(&self) -> u32;

    /// Get actual (hardware) brightness
    fn actual_brightness(&self) -> u32 {
        self.get_brightness()
    }

    /// Power on/off
    fn set_power(&self, on: bool) -> KResult<()> {
        if on {
            // Restore to previous brightness
            Ok(())
        } else {
            // Set brightness to 0
            self.set_brightness(0)
        }
    }
}

impl BacklightDevice {
    /// Create a new backlight device
    pub fn new(
        name: &str,
        backlight_type: BacklightType,
        ops: Arc<dyn BacklightOps>,
    ) -> Self {
        let max = ops.max_brightness();
        let current = ops.get_brightness();

        Self {
            name: String::from(name),
            backlight_type,
            max_brightness: max,
            current: AtomicU32::new(current),
            ops,
        }
    }

    /// Get device name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get backlight type
    pub fn device_type(&self) -> BacklightType {
        self.backlight_type
    }

    /// Get current brightness
    pub fn brightness(&self) -> u32 {
        self.current.load(Ordering::SeqCst)
    }

    /// Set brightness
    pub fn set_brightness(&self, value: u32) -> KResult<()> {
        let clamped = value.min(self.max_brightness);
        self.ops.set_brightness(clamped)?;
        self.current.store(clamped, Ordering::SeqCst);
        Ok(())
    }

    /// Get maximum brightness
    pub fn max_brightness(&self) -> u32 {
        self.max_brightness
    }

    /// Get actual brightness from hardware
    pub fn actual_brightness(&self) -> u32 {
        self.ops.actual_brightness()
    }

    /// Get brightness as percentage (0-100)
    pub fn brightness_percent(&self) -> u32 {
        let curr = self.brightness();
        if self.max_brightness > 0 {
            (curr * 100) / self.max_brightness
        } else {
            0
        }
    }

    /// Set brightness as percentage (0-100)
    pub fn set_brightness_percent(&self, percent: u32) -> KResult<()> {
        let percent = percent.min(100);
        let value = (percent * self.max_brightness) / 100;
        self.set_brightness(value)
    }

    /// Increase brightness by step
    pub fn increase(&self, step: u32) -> KResult<()> {
        let current = self.brightness();
        let new = current.saturating_add(step).min(self.max_brightness);
        self.set_brightness(new)
    }

    /// Decrease brightness by step
    pub fn decrease(&self, step: u32) -> KResult<()> {
        let current = self.brightness();
        let new = current.saturating_sub(step);
        self.set_brightness(new)
    }

    /// Power on
    pub fn power_on(&self) -> KResult<()> {
        self.ops.set_power(true)
    }

    /// Power off (blank screen)
    pub fn power_off(&self) -> KResult<()> {
        self.ops.set_power(false)
    }
}

/// ACPI backlight operations
pub struct AcpiBacklight {
    /// ACPI handle/path
    path: String,
    /// Available brightness levels
    levels: Vec<u32>,
    /// Maximum level index
    max_level: u32,
    /// Current level index
    current_level: AtomicU32,
}

impl AcpiBacklight {
    /// Create ACPI backlight
    pub fn new(path: &str) -> Option<Self> {
        // Query ACPI for brightness levels (_BCL)
        // In a real implementation, this would call ACPI methods
        // For now, use simulated levels
        let levels = vec![0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
        let max_level = (levels.len() - 1) as u32;

        Some(Self {
            path: String::from(path),
            levels,
            max_level,
            current_level: AtomicU32::new(max_level), // Start at max
        })
    }

    /// Get ACPI path
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Get available levels
    pub fn levels(&self) -> &[u32] {
        &self.levels
    }

    /// Set brightness level by index
    pub fn set_level(&self, index: u32) -> KResult<()> {
        if index > self.max_level {
            return Err(KError::Invalid);
        }

        // In a real implementation, call _BCM ACPI method
        // with self.levels[index] as the value
        self.current_level.store(index, Ordering::SeqCst);

        crate::kprintln!("backlight: ACPI set level {} ({}%)",
            index, self.levels[index as usize]);

        Ok(())
    }

    /// Get current level index
    pub fn level(&self) -> u32 {
        self.current_level.load(Ordering::SeqCst)
    }
}

impl BacklightOps for AcpiBacklight {
    fn get_brightness(&self) -> u32 {
        let level = self.current_level.load(Ordering::SeqCst);
        self.levels.get(level as usize).copied().unwrap_or(0)
    }

    fn set_brightness(&self, value: u32) -> KResult<()> {
        // Find closest level
        let mut closest_idx = 0;
        let mut closest_diff = u32::MAX;

        for (i, &level) in self.levels.iter().enumerate() {
            let diff = if level > value { level - value } else { value - level };
            if diff < closest_diff {
                closest_diff = diff;
                closest_idx = i;
            }
        }

        self.set_level(closest_idx as u32)
    }

    fn max_brightness(&self) -> u32 {
        *self.levels.last().unwrap_or(&100)
    }
}

/// Intel GPU backlight operations
pub struct IntelBacklight {
    /// MMIO base address
    mmio_base: usize,
    /// Backlight register offset
    bl_reg_offset: usize,
    /// Maximum brightness value (from PWM max)
    max_value: u32,
}

impl IntelBacklight {
    // Intel backlight registers (relative to MMIO base)
    const BLC_PWM_CTL: usize = 0x61254; // Gen4+
    const BLC_PWM_CTL2: usize = 0x61250; // Gen4+

    // For newer generations
    const PCH_BLC_PWM_CTL1: usize = 0xC8250;
    const PCH_BLC_PWM_CTL2: usize = 0xC8254;

    /// Create Intel backlight from GPU MMIO base
    pub fn new(mmio_base: usize) -> Option<Self> {
        // Read PWM control register to determine max value
        // In a real implementation, this would read from hardware
        let max_value = 0xFFFF; // 16-bit PWM typical

        Some(Self {
            mmio_base,
            bl_reg_offset: Self::BLC_PWM_CTL,
            max_value,
        })
    }

    /// Read backlight register
    fn read_reg(&self) -> u32 {
        unsafe {
            core::ptr::read_volatile((self.mmio_base + self.bl_reg_offset) as *const u32)
        }
    }

    /// Write backlight register
    fn write_reg(&self, value: u32) {
        unsafe {
            core::ptr::write_volatile((self.mmio_base + self.bl_reg_offset) as *mut u32, value);
        }
    }
}

impl BacklightOps for IntelBacklight {
    fn get_brightness(&self) -> u32 {
        let reg = self.read_reg();
        // Duty cycle is typically in lower 16 bits
        reg & 0xFFFF
    }

    fn set_brightness(&self, value: u32) -> KResult<()> {
        let value = value.min(self.max_value);
        let reg = self.read_reg();
        // Preserve upper bits, set duty cycle in lower 16 bits
        let new_reg = (reg & 0xFFFF0000) | value;
        self.write_reg(new_reg);
        Ok(())
    }

    fn max_brightness(&self) -> u32 {
        self.max_value
    }
}

/// Raw/platform backlight operations
pub struct RawBacklight {
    /// I/O port or MMIO address
    address: usize,
    /// Address type
    is_mmio: bool,
    /// Maximum value
    max_value: u32,
    /// Current value
    current: AtomicU32,
}

impl RawBacklight {
    /// Create raw backlight
    pub fn new(address: usize, is_mmio: bool, max_value: u32) -> Self {
        Self {
            address,
            is_mmio,
            max_value,
            current: AtomicU32::new(max_value),
        }
    }
}

impl BacklightOps for RawBacklight {
    fn get_brightness(&self) -> u32 {
        self.current.load(Ordering::SeqCst)
    }

    fn set_brightness(&self, value: u32) -> KResult<()> {
        let value = value.min(self.max_value);

        if self.is_mmio {
            unsafe {
                core::ptr::write_volatile(self.address as *mut u32, value);
            }
        } else {
            // I/O port access would go here
        }

        self.current.store(value, Ordering::SeqCst);
        Ok(())
    }

    fn max_brightness(&self) -> u32 {
        self.max_value
    }
}

/// Backlight manager
struct BacklightManager {
    /// Registered devices
    devices: BTreeMap<String, BacklightDevice>,
    /// Primary device name
    primary: Option<String>,
}

impl BacklightManager {
    const fn new() -> Self {
        Self {
            devices: BTreeMap::new(),
            primary: None,
        }
    }

    /// Register a backlight device
    fn register(&mut self, device: BacklightDevice) {
        let name = device.name.clone();

        // Set as primary if this is the first device or it's firmware type
        if self.primary.is_none() ||
           (device.backlight_type == BacklightType::Firmware &&
            self.devices.get(self.primary.as_ref().unwrap())
                .map(|d| d.backlight_type != BacklightType::Firmware)
                .unwrap_or(true))
        {
            self.primary = Some(name.clone());
        }

        crate::kprintln!("backlight: registered {} ({:?}, max={})",
            name, device.backlight_type, device.max_brightness);

        self.devices.insert(name, device);
    }

    /// Unregister a device
    fn unregister(&mut self, name: &str) {
        self.devices.remove(name);
        if self.primary.as_ref().map(|n| n == name).unwrap_or(false) {
            self.primary = self.devices.keys().next().cloned();
        }
    }

    /// Get device by name
    fn get(&self, name: &str) -> Option<&BacklightDevice> {
        self.devices.get(name)
    }

    /// Get primary device
    fn primary(&self) -> Option<&BacklightDevice> {
        self.primary.as_ref().and_then(|n| self.devices.get(n))
    }

    /// List all devices
    fn list(&self) -> Vec<&str> {
        self.devices.keys().map(|s| s.as_str()).collect()
    }
}

/// Global backlight manager
static BACKLIGHT_MANAGER: IrqSafeMutex<BacklightManager> =
    IrqSafeMutex::new(BacklightManager::new());

/// Initialize backlight subsystem
pub fn init() {
    crate::kprintln!("backlight: initializing");

    // Try to detect ACPI backlight
    if let Some(acpi_bl) = AcpiBacklight::new("\\_SB.PCI0.GFX0.LCD._BCM") {
        let device = BacklightDevice::new(
            "acpi_video0",
            BacklightType::Firmware,
            Arc::new(acpi_bl),
        );
        BACKLIGHT_MANAGER.lock().register(device);
    }

    // Try to detect Intel GPU backlight
    // In a real implementation, we would get this from PCI enumeration
    // For now, check if Intel GPU exists
    detect_intel_backlight();

    crate::kprintln!("backlight: initialized");
}

/// Detect Intel GPU backlight
fn detect_intel_backlight() {
    // Scan PCI for Intel GPU
    for device in crate::drivers::pci::scan() {
        // Intel vendor ID with display class
        if device.id.vendor_id == 0x8086 && device.class.class_code == 0x03 {
            crate::kprintln!("backlight: found Intel GPU at {:02x}:{:02x}.{}",
                device.addr.bus, device.addr.device, device.addr.function);

            // Get BAR0 for MMIO
            let (bar_addr, _) = crate::drivers::pci::read_bar(&device, 0);
            if bar_addr != 0 {
                if let Some(intel_bl) = IntelBacklight::new(bar_addr as usize) {
                    let bl_device = BacklightDevice::new(
                        "intel_backlight",
                        BacklightType::IntelBacklight,
                        Arc::new(intel_bl),
                    );
                    BACKLIGHT_MANAGER.lock().register(bl_device);
                }
            }
            break;
        }
    }
}

/// Register a backlight device
pub fn register_device(device: BacklightDevice) {
    BACKLIGHT_MANAGER.lock().register(device);
}

/// Unregister a backlight device
pub fn unregister_device(name: &str) {
    BACKLIGHT_MANAGER.lock().unregister(name);
}

/// Get current brightness of primary device (0-100%)
pub fn get_brightness() -> Option<u32> {
    BACKLIGHT_MANAGER.lock().primary().map(|d| d.brightness_percent())
}

/// Set brightness of primary device (0-100%)
pub fn set_brightness(percent: u32) -> KResult<()> {
    let manager = BACKLIGHT_MANAGER.lock();
    manager.primary()
        .ok_or(KError::NotFound)?
        .set_brightness_percent(percent)
}

/// Increase brightness of primary device
pub fn increase_brightness(step_percent: u32) -> KResult<()> {
    let manager = BACKLIGHT_MANAGER.lock();
    let device = manager.primary().ok_or(KError::NotFound)?;
    let step = (step_percent * device.max_brightness()) / 100;
    device.increase(step)
}

/// Decrease brightness of primary device
pub fn decrease_brightness(step_percent: u32) -> KResult<()> {
    let manager = BACKLIGHT_MANAGER.lock();
    let device = manager.primary().ok_or(KError::NotFound)?;
    let step = (step_percent * device.max_brightness()) / 100;
    device.decrease(step)
}

/// Get maximum brightness of primary device
pub fn max_brightness() -> Option<u32> {
    BACKLIGHT_MANAGER.lock().primary().map(|d| d.max_brightness())
}

/// Get raw brightness value of primary device
pub fn get_brightness_raw() -> Option<u32> {
    BACKLIGHT_MANAGER.lock().primary().map(|d| d.brightness())
}

/// Set raw brightness value of primary device
pub fn set_brightness_raw(value: u32) -> KResult<()> {
    let manager = BACKLIGHT_MANAGER.lock();
    manager.primary()
        .ok_or(KError::NotFound)?
        .set_brightness(value)
}

/// List available backlight devices
pub fn list_devices() -> Vec<String> {
    BACKLIGHT_MANAGER.lock().list().iter().map(|s| String::from(*s)).collect()
}

/// Get brightness of specific device
pub fn get_device_brightness(name: &str) -> Option<u32> {
    BACKLIGHT_MANAGER.lock().get(name).map(|d| d.brightness_percent())
}

/// Set brightness of specific device
pub fn set_device_brightness(name: &str, percent: u32) -> KResult<()> {
    let manager = BACKLIGHT_MANAGER.lock();
    manager.get(name)
        .ok_or(KError::NotFound)?
        .set_brightness_percent(percent)
}

/// Get device info
pub fn device_info(name: &str) -> Option<BacklightInfo> {
    let manager = BACKLIGHT_MANAGER.lock();
    manager.get(name).map(|d| BacklightInfo {
        name: String::from(d.name()),
        device_type: d.device_type(),
        brightness: d.brightness(),
        max_brightness: d.max_brightness(),
        brightness_percent: d.brightness_percent(),
    })
}

/// Backlight device information
#[derive(Debug, Clone)]
pub struct BacklightInfo {
    pub name: String,
    pub device_type: BacklightType,
    pub brightness: u32,
    pub max_brightness: u32,
    pub brightness_percent: u32,
}

/// Handle function key events for brightness
pub fn handle_brightness_key(increase: bool) {
    const STEP: u32 = 10; // 10% step

    if increase {
        let _ = increase_brightness(STEP);
    } else {
        let _ = decrease_brightness(STEP);
    }

    // Log new brightness
    if let Some(percent) = get_brightness() {
        crate::kprintln!("backlight: brightness {}%", percent);
    }
}

/// Get primary device name
pub fn primary_device() -> Option<String> {
    BACKLIGHT_MANAGER.lock().primary.clone()
}

/// Check if backlight control is available
pub fn is_available() -> bool {
    BACKLIGHT_MANAGER.lock().primary.is_some()
}
