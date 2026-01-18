//! Thermal Management Driver
//!
//! Monitors system temperatures via ACPI thermal zones and CPU thermal
//! sensors (MSRs), and triggers actions (throttling, shutdown) when
//! thresholds are exceeded.
//!
//! Supports:
//! - Intel Core temperature reading via MSR 0x19C / 0x1B1
//! - AMD Ryzen temperature reading via SMN 0x59800
//! - ACPI thermal zones (_TMP evaluation)
//! - Fan control via EC or ACPI _FAN devices
//! - Thermal throttling via cpufreq integration

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use crate::sync::IrqSafeMutex;
use x86_64::registers::model_specific::Msr;

// Intel MSRs for thermal monitoring
const MSR_TEMPERATURE_TARGET: u32 = 0x1A2;     // TjMax (maximum junction temperature)
const MSR_THERM_STATUS: u32 = 0x19C;           // Core thermal status/reading
const MSR_THERM_INTERRUPT: u32 = 0x19B;        // Thermal interrupt control
const MSR_PACKAGE_THERM_STATUS: u32 = 0x1B1;   // Package thermal status
const MSR_PACKAGE_THERM_INTERRUPT: u32 = 0x1B2; // Package thermal interrupt

// AMD MSRs for thermal monitoring (Family 17h+, Ryzen)
const MSR_AMD_HARDWARE_THERMAL_CONTROL: u32 = 0xC0010015;
const SMN_SMUIO_THM: u32 = 0x00059800;  // SMN address for Tctl

// CPU vendor detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuVendor {
    Intel,
    Amd,
    Unknown,
}

/// Execute CPUID instruction
/// Note: EBX is reserved by LLVM, so we save/restore it manually
#[inline]
fn cpuid(leaf: u32) -> (u32, u32, u32, u32) {
    let eax: u32;
    let ebx: u32;
    let ecx: u32;
    let edx: u32;

    unsafe {
        core::arch::asm!(
            "push rbx",        // Save RBX (LLVM uses it)
            "cpuid",
            "mov {0:e}, ebx",  // Copy EBX to output register
            "pop rbx",         // Restore RBX
            out(reg) ebx,
            inout("eax") leaf => eax,
            out("ecx") ecx,
            out("edx") edx,
            options(nomem, nostack)
        );
    }

    (eax, ebx, ecx, edx)
}

/// Detect CPU vendor
pub fn detect_cpu_vendor() -> CpuVendor {
    let (_, ebx, ecx, edx) = cpuid(0);

    // Vendor string is in EBX, EDX, ECX (12 bytes)
    // GenuineIntel: EBX=0x756e6547 "Genu", EDX=0x49656e69 "ineI", ECX=0x6c65746e "ntel"
    // AuthenticAMD: EBX=0x68747541 "Auth", EDX=0x69746e65 "enti", ECX=0x444d4163 "cAMD"

    if ebx == 0x756e6547 && edx == 0x49656e69 && ecx == 0x6c65746e {
        CpuVendor::Intel
    } else if ebx == 0x68747541 && edx == 0x69746e65 && ecx == 0x444d4163 {
        CpuVendor::Amd
    } else {
        CpuVendor::Unknown
    }
}

/// Check if CPU has thermal sensors
pub fn has_thermal_sensors() -> bool {
    let (_, _, ecx, edx) = cpuid(1);

    // EDX bit 22 = ACPI - thermal monitor support
    // ECX bit 8 = TM2 - thermal monitor 2
    // ECX bit 5 = VMX - but we check for DTS
    let has_acpi = (edx & (1 << 22)) != 0;
    let has_tm2 = (ecx & (1 << 8)) != 0;

    // Also check CPUID leaf 6 for digital thermal sensor
    let (eax, _, _, _) = cpuid(6);
    let has_dts = (eax & 1) != 0; // Bit 0 = Digital Temperature Sensor

    has_acpi || has_tm2 || has_dts
}

/// Get TjMax (maximum junction temperature) for Intel CPUs
/// Returns temperature in millidegrees Celsius
pub fn get_tjmax() -> Temperature {
    // Default TjMax values (most Intel CPUs use 100°C)
    let default_tjmax = 100000; // 100°C in mC

    if detect_cpu_vendor() != CpuVendor::Intel {
        return default_tjmax;
    }

    // Try to read from MSR
    let msr = Msr::new(MSR_TEMPERATURE_TARGET);
    let value = unsafe { msr.read() };

    // TjMax is in bits 23:16
    let tjmax_celsius = ((value >> 16) & 0xFF) as i32;

    if tjmax_celsius > 0 && tjmax_celsius < 150 {
        tjmax_celsius * 1000 // Convert to mC
    } else {
        default_tjmax
    }
}

/// Read Intel CPU core temperature from MSR
/// Returns temperature in millidegrees Celsius
fn read_intel_core_temp(core: u8) -> Option<Temperature> {
    // TODO: Pin to specific core for multi-core reading
    // For now, read from current core

    let msr = Msr::new(MSR_THERM_STATUS);
    let value = unsafe { msr.read() };

    // Check if reading is valid (bit 31 = Reading Valid)
    if (value & (1 << 31)) == 0 {
        return None;
    }

    // Temperature reading is in bits 22:16 (digital readout)
    // This is the offset from TjMax
    let temp_offset = ((value >> 16) & 0x7F) as i32;

    let tjmax = get_tjmax();
    let temp = tjmax - (temp_offset * 1000);

    Some(temp)
}

/// Read Intel CPU package temperature from MSR
/// Returns temperature in millidegrees Celsius
fn read_intel_package_temp() -> Option<Temperature> {
    let msr = Msr::new(MSR_PACKAGE_THERM_STATUS);
    let value = unsafe { msr.read() };

    // Check if reading is valid (bit 31 = Reading Valid)
    if (value & (1 << 31)) == 0 {
        return None;
    }

    // Temperature reading is in bits 22:16
    let temp_offset = ((value >> 16) & 0x7F) as i32;

    let tjmax = get_tjmax();
    let temp = tjmax - (temp_offset * 1000);

    Some(temp)
}

/// Read AMD CPU temperature via PCI config space (older CPUs)
/// For AMD Family 10h-15h
fn read_amd_legacy_temp() -> Option<Temperature> {
    use x86_64::instructions::port::Port;

    // AMD northbridge PCI config (Bus 0, Device 0x18, Function 3)
    // Register 0xA4 contains temperature
    const PCI_CONFIG_ADDR: u16 = 0xCF8;
    const PCI_CONFIG_DATA: u16 = 0xCFC;

    // Construct PCI config address: Bus 0, Device 0x18, Function 3, Register 0xA4
    let address: u32 = 0x80000000 | (0x18 << 11) | (3 << 8) | (0xA4 & 0xFC);

    let temp_raw = unsafe {
        let mut addr_port: Port<u32> = Port::new(PCI_CONFIG_ADDR);
        let mut data_port: Port<u32> = Port::new(PCI_CONFIG_DATA);

        addr_port.write(address);
        data_port.read()
    };

    // Temperature in bits 31:21 (offset from zero)
    // Formula: temp = (value >> 21) * 125 / 1000 - 49 (in °C)
    let temp_raw_bits = (temp_raw >> 21) & 0x7FF;
    let temp_celsius = (temp_raw_bits as i32 * 125) / 1000 - 49;

    if temp_celsius > -40 && temp_celsius < 150 {
        Some(temp_celsius * 1000) // Convert to mC
    } else {
        None
    }
}

/// Read AMD Ryzen temperature via SMN (System Management Network)
/// For AMD Family 17h+ (Zen architecture)
fn read_amd_ryzen_temp() -> Option<Temperature> {
    use x86_64::instructions::port::Port;

    // SMN access via PCI: write address to 0xB8, read data from 0xBC
    // PCI device: Bus 0, Device 0, Function 0
    const PCI_CONFIG_ADDR: u16 = 0xCF8;
    const PCI_CONFIG_DATA: u16 = 0xCFC;

    // SMN address register (offset 0x60)
    let smn_addr_reg: u32 = 0x80000000 | (0 << 11) | (0 << 8) | 0x60;
    // SMN data register (offset 0x64)
    let smn_data_reg: u32 = 0x80000000 | (0 << 11) | (0 << 8) | 0x64;

    let temp_raw = unsafe {
        let mut addr_port: Port<u32> = Port::new(PCI_CONFIG_ADDR);
        let mut data_port: Port<u32> = Port::new(PCI_CONFIG_DATA);

        // Write SMN address
        addr_port.write(smn_addr_reg);
        data_port.write(SMN_SMUIO_THM);

        // Read SMN data
        addr_port.write(smn_data_reg);
        data_port.read()
    };

    // Tctl value is in bits 31:21 (in units of 0.125°C)
    // Some Ryzen CPUs have a Tctl offset (e.g., Ryzen 1800X has +10°C offset)
    let tctl_raw = (temp_raw >> 21) & 0x7FF;
    let temp_mcelsius = (tctl_raw as i32 * 125); // Already in mC

    if temp_mcelsius > -40000 && temp_mcelsius < 150000 {
        Some(temp_mcelsius)
    } else {
        None
    }
}

/// Read CPU temperature (auto-detect vendor)
/// Returns temperature in millidegrees Celsius
pub fn read_cpu_temperature() -> Option<Temperature> {
    match detect_cpu_vendor() {
        CpuVendor::Intel => {
            // Prefer package temperature, fallback to core 0
            read_intel_package_temp()
                .or_else(|| read_intel_core_temp(0))
        }
        CpuVendor::Amd => {
            // Try Ryzen first, then legacy
            read_amd_ryzen_temp()
                .or_else(read_amd_legacy_temp)
        }
        CpuVendor::Unknown => None,
    }
}

/// Read temperature for a specific CPU core
/// Returns temperature in millidegrees Celsius
pub fn read_core_temperature(core: u8) -> Option<Temperature> {
    match detect_cpu_vendor() {
        CpuVendor::Intel => read_intel_core_temp(core),
        CpuVendor::Amd => {
            // AMD doesn't have per-core temperature reading via MSR
            // Return package temperature
            read_amd_ryzen_temp().or_else(read_amd_legacy_temp)
        }
        CpuVendor::Unknown => None,
    }
}

/// Get logical processor count from CPUID
fn get_logical_processor_count() -> u8 {
    let (_, ebx, _, _) = cpuid(1);
    // EBX bits 23:16 = Maximum number of logical processors per package
    let count = ((ebx >> 16) & 0xFF) as u8;
    if count == 0 { 1 } else { count }
}

/// Get all available core temperatures
/// Returns Vec of (core_id, temperature_mC)
pub fn read_all_core_temperatures() -> Vec<(u8, Temperature)> {
    let mut temps = Vec::new();

    // Get CPU count
    let core_count = get_logical_processor_count();

    match detect_cpu_vendor() {
        CpuVendor::Intel => {
            for core in 0..core_count {
                if let Some(temp) = read_intel_core_temp(core) {
                    temps.push((core, temp));
                }
            }
        }
        CpuVendor::Amd => {
            // AMD: single CCD temperature
            if let Some(temp) = read_amd_ryzen_temp().or_else(read_amd_legacy_temp) {
                temps.push((0, temp));
            }
        }
        CpuVendor::Unknown => {}
    }

    temps
}

/// Thermal sensor information
#[derive(Debug, Clone)]
pub struct ThermalSensorInfo {
    pub vendor: CpuVendor,
    pub has_digital_thermal_sensor: bool,
    pub tjmax: Temperature,
    pub core_count: u8,
    pub supports_package_temp: bool,
}

/// Get thermal sensor capabilities
pub fn get_sensor_info() -> ThermalSensorInfo {
    let vendor = detect_cpu_vendor();

    // Check for digital thermal sensor in CPUID leaf 6
    let (eax, _, _, _) = cpuid(6);
    let has_dts = (eax & 1) != 0;

    let core_count = get_logical_processor_count();

    let supports_package = vendor == CpuVendor::Intel && has_dts;

    ThermalSensorInfo {
        vendor,
        has_digital_thermal_sensor: has_dts,
        tjmax: get_tjmax(),
        core_count,
        supports_package_temp: supports_package,
    }
}

/// Temperature in millidegrees Celsius (mC)
/// Example: 50000 mC = 50.0°C
pub type Temperature = i32;

/// Convert Kelvin (ACPI format) to millidegrees Celsius
pub fn kelvin_to_mcelsius(kelvin: u32) -> Temperature {
    // ACPI reports temperature in tenths of Kelvin
    // T(°C) = T(K/10) / 10 - 273.15
    // T(mC) = (K - 2732) * 100
    ((kelvin as i32) - 2732) * 100
}

/// Convert millidegrees Celsius to Kelvin (ACPI format)
pub fn mcelsius_to_kelvin(mcelsius: Temperature) -> u32 {
    // T(K/10) = (T(mC) / 100) + 2732
    ((mcelsius / 100) + 2732) as u32
}

/// Thermal trip point type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TripPointType {
    /// Active cooling (fan turns on)
    Active,
    /// Passive cooling (CPU throttling)
    Passive,
    /// Hot threshold (warning)
    Hot,
    /// Critical threshold (emergency shutdown)
    Critical,
}

/// A thermal trip point
#[derive(Debug, Clone)]
pub struct TripPoint {
    pub trip_type: TripPointType,
    pub temperature: Temperature,
    pub hysteresis: Temperature,
}

/// Thermal zone state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThermalZoneState {
    Normal,
    Throttling,
    Hot,
    Critical,
}

/// A thermal zone
#[derive(Debug, Clone)]
pub struct ThermalZone {
    /// Zone name (e.g., "thermal_zone0", "cpu-thermal")
    pub name: String,
    /// Current temperature in mC
    pub temperature: Temperature,
    /// Trip points
    pub trip_points: Vec<TripPoint>,
    /// Current state
    pub state: ThermalZoneState,
    /// Polling interval in ms (0 = event-driven)
    pub polling_interval: u32,
}

impl ThermalZone {
    pub fn new(name: &str) -> Self {
        Self {
            name: String::from(name),
            temperature: 0,
            trip_points: Vec::new(),
            state: ThermalZoneState::Normal,
            polling_interval: 1000, // Default 1 second
        }
    }

    /// Add a trip point
    pub fn add_trip_point(&mut self, trip: TripPoint) {
        self.trip_points.push(trip);
        // Sort by temperature (ascending)
        self.trip_points.sort_by_key(|t| t.temperature);
    }

    /// Get critical temperature threshold
    pub fn critical_temp(&self) -> Option<Temperature> {
        self.trip_points.iter()
            .find(|t| t.trip_type == TripPointType::Critical)
            .map(|t| t.temperature)
    }

    /// Get hot temperature threshold
    pub fn hot_temp(&self) -> Option<Temperature> {
        self.trip_points.iter()
            .find(|t| t.trip_type == TripPointType::Hot)
            .map(|t| t.temperature)
    }

    /// Get passive cooling threshold
    pub fn passive_temp(&self) -> Option<Temperature> {
        self.trip_points.iter()
            .find(|t| t.trip_type == TripPointType::Passive)
            .map(|t| t.temperature)
    }

    /// Update temperature and check thresholds
    pub fn update(&mut self, new_temp: Temperature) -> ThermalZoneState {
        self.temperature = new_temp;

        // Check thresholds from highest to lowest
        if let Some(crit) = self.critical_temp() {
            if new_temp >= crit {
                self.state = ThermalZoneState::Critical;
                return self.state;
            }
        }

        if let Some(hot) = self.hot_temp() {
            if new_temp >= hot {
                self.state = ThermalZoneState::Hot;
                return self.state;
            }
        }

        if let Some(passive) = self.passive_temp() {
            if new_temp >= passive {
                self.state = ThermalZoneState::Throttling;
                return self.state;
            }
        }

        self.state = ThermalZoneState::Normal;
        self.state
    }

    /// Check if temperature is critical
    pub fn is_critical(&self) -> bool {
        self.state == ThermalZoneState::Critical
    }

    /// Get temperature in degrees Celsius (float approximation)
    pub fn temp_celsius(&self) -> f32 {
        self.temperature as f32 / 1000.0
    }
}

/// Cooling device type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoolingDeviceType {
    Fan,
    Processor,
    Other,
}

/// A cooling device
#[derive(Debug, Clone)]
pub struct CoolingDevice {
    pub name: String,
    pub device_type: CoolingDeviceType,
    pub max_state: u32,
    pub current_state: u32,
}

impl CoolingDevice {
    pub fn new(name: &str, device_type: CoolingDeviceType, max_state: u32) -> Self {
        Self {
            name: String::from(name),
            device_type,
            max_state,
            current_state: 0,
        }
    }

    /// Set cooling level (0 = off, max_state = full)
    pub fn set_state(&mut self, state: u32) {
        self.current_state = state.min(self.max_state);
    }

    /// Get cooling level as percentage
    pub fn level_percent(&self) -> u8 {
        if self.max_state == 0 {
            0
        } else {
            ((self.current_state as u64 * 100) / self.max_state as u64) as u8
        }
    }
}

// =============================================================================
// Fan Control Implementation
// =============================================================================

/// Fan control mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FanControlMode {
    /// BIOS/firmware controlled
    Auto,
    /// Manual PWM control
    Manual,
    /// Full speed
    FullSpeed,
}

/// Fan speed source
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FanSpeedSource {
    /// Embedded Controller (most laptops)
    EmbeddedController,
    /// ACPI _FST method
    AcpiFst,
    /// SuperIO chip
    SuperIo,
    /// Unknown/not readable
    Unknown,
}

/// Fan information
#[derive(Debug, Clone)]
pub struct FanInfo {
    pub id: u8,
    pub name: String,
    pub speed_rpm: u32,
    pub pwm_percent: u8,
    pub mode: FanControlMode,
    pub source: FanSpeedSource,
    pub min_rpm: u32,
    pub max_rpm: u32,
}

impl FanInfo {
    pub fn new(id: u8, name: &str) -> Self {
        Self {
            id,
            name: String::from(name),
            speed_rpm: 0,
            pwm_percent: 0,
            mode: FanControlMode::Auto,
            source: FanSpeedSource::Unknown,
            min_rpm: 0,
            max_rpm: 5000,
        }
    }
}

/// EC (Embedded Controller) ports for fan control
const EC_SC: u16 = 0x66;   // EC Status/Command port
const EC_DATA: u16 = 0x62;  // EC Data port

// EC Status bits
const EC_OBF: u8 = 0x01;    // Output Buffer Full
const EC_IBF: u8 = 0x02;    // Input Buffer Full

// EC Commands
const EC_READ_CMD: u8 = 0x80;   // Read byte
const EC_WRITE_CMD: u8 = 0x81;  // Write byte
const EC_BURST_ENABLE: u8 = 0x82;
const EC_BURST_DISABLE: u8 = 0x83;

/// Wait for EC input buffer empty
fn ec_wait_ibe() -> bool {
    use x86_64::instructions::port::Port;

    for _ in 0..10000 {
        let status = unsafe {
            let mut port: Port<u8> = Port::new(EC_SC);
            port.read()
        };
        if (status & EC_IBF) == 0 {
            return true;
        }
        // Small delay
        for _ in 0..100 {
            core::hint::spin_loop();
        }
    }
    false
}

/// Wait for EC output buffer full
fn ec_wait_obf() -> bool {
    use x86_64::instructions::port::Port;

    for _ in 0..10000 {
        let status = unsafe {
            let mut port: Port<u8> = Port::new(EC_SC);
            port.read()
        };
        if (status & EC_OBF) != 0 {
            return true;
        }
        for _ in 0..100 {
            core::hint::spin_loop();
        }
    }
    false
}

/// Read byte from EC
fn ec_read(addr: u8) -> Option<u8> {
    use x86_64::instructions::port::Port;

    if !ec_wait_ibe() {
        return None;
    }

    unsafe {
        let mut cmd_port: Port<u8> = Port::new(EC_SC);
        cmd_port.write(EC_READ_CMD);
    }

    if !ec_wait_ibe() {
        return None;
    }

    unsafe {
        let mut data_port: Port<u8> = Port::new(EC_DATA);
        data_port.write(addr);
    }

    if !ec_wait_obf() {
        return None;
    }

    let value = unsafe {
        let mut data_port: Port<u8> = Port::new(EC_DATA);
        data_port.read()
    };

    Some(value)
}

/// Write byte to EC
fn ec_write(addr: u8, value: u8) -> bool {
    use x86_64::instructions::port::Port;

    if !ec_wait_ibe() {
        return false;
    }

    unsafe {
        let mut cmd_port: Port<u8> = Port::new(EC_SC);
        cmd_port.write(EC_WRITE_CMD);
    }

    if !ec_wait_ibe() {
        return false;
    }

    unsafe {
        let mut data_port: Port<u8> = Port::new(EC_DATA);
        data_port.write(addr);
    }

    if !ec_wait_ibe() {
        return false;
    }

    unsafe {
        let mut data_port: Port<u8> = Port::new(EC_DATA);
        data_port.write(value);
    }

    true
}

// Common EC fan registers (vendor-specific, these are Dell-like defaults)
// Other vendors (Lenovo, HP, ASUS) have different layouts
const EC_FAN1_SPEED_OFFSET: u8 = 0x84;      // Fan 1 speed (RPM / 100)
const EC_FAN2_SPEED_OFFSET: u8 = 0x85;      // Fan 2 speed
const EC_FAN1_PWM_OFFSET: u8 = 0x86;        // Fan 1 PWM duty cycle
const EC_FAN2_PWM_OFFSET: u8 = 0x87;        // Fan 2 PWM duty cycle
const EC_FAN_CONTROL_OFFSET: u8 = 0x88;     // Fan control mode register

/// Read fan speed from EC (RPM)
pub fn ec_read_fan_speed(fan_id: u8) -> Option<u32> {
    let offset = match fan_id {
        0 => EC_FAN1_SPEED_OFFSET,
        1 => EC_FAN2_SPEED_OFFSET,
        _ => return None,
    };

    ec_read(offset).map(|v| (v as u32) * 100) // Convert to RPM
}

/// Read fan PWM duty cycle from EC (0-255)
pub fn ec_read_fan_pwm(fan_id: u8) -> Option<u8> {
    let offset = match fan_id {
        0 => EC_FAN1_PWM_OFFSET,
        1 => EC_FAN2_PWM_OFFSET,
        _ => return None,
    };

    ec_read(offset)
}

/// Set fan PWM duty cycle via EC (0-255)
pub fn ec_set_fan_pwm(fan_id: u8, pwm: u8) -> bool {
    let offset = match fan_id {
        0 => EC_FAN1_PWM_OFFSET,
        1 => EC_FAN2_PWM_OFFSET,
        _ => return false,
    };

    ec_write(offset, pwm)
}

/// Set fan control mode via EC
pub fn ec_set_fan_mode(mode: FanControlMode) -> bool {
    let value = match mode {
        FanControlMode::Auto => 0x00,
        FanControlMode::Manual => 0x01,
        FanControlMode::FullSpeed => 0xFF,
    };

    ec_write(EC_FAN_CONTROL_OFFSET, value)
}

/// Check if EC fan control is available
pub fn ec_fan_available() -> bool {
    use x86_64::instructions::port::Port;

    // Check if EC responds
    let status = unsafe {
        let mut port: Port<u8> = Port::new(EC_SC);
        port.read()
    };

    // If status is 0xFF, EC is not present
    status != 0xFF
}

/// Fan controller state
pub struct FanController {
    fans: Vec<FanInfo>,
    ec_available: bool,
    global_mode: FanControlMode,
}

impl FanController {
    pub const fn new() -> Self {
        Self {
            fans: Vec::new(),
            ec_available: false,
            global_mode: FanControlMode::Auto,
        }
    }

    /// Initialize fan controller
    pub fn init(&mut self) {
        self.ec_available = ec_fan_available();

        if self.ec_available {
            crate::kprintln!("thermal: EC fan control available");

            // Try to detect fans
            for fan_id in 0..2 {
                if let Some(speed) = ec_read_fan_speed(fan_id) {
                    let mut fan = FanInfo::new(fan_id, if fan_id == 0 { "cpu-fan" } else { "gpu-fan" });
                    fan.speed_rpm = speed;
                    fan.source = FanSpeedSource::EmbeddedController;

                    if let Some(pwm) = ec_read_fan_pwm(fan_id) {
                        fan.pwm_percent = ((pwm as u32 * 100) / 255) as u8;
                    }

                    self.fans.push(fan);
                    crate::kprintln!("thermal: detected fan{}: {} RPM", fan_id, speed);
                }
            }
        } else {
            crate::kprintln!("thermal: EC fan control not available");
        }
    }

    /// Get all fans
    pub fn fans(&self) -> &[FanInfo] {
        &self.fans
    }

    /// Get fan count
    pub fn fan_count(&self) -> usize {
        self.fans.len()
    }

    /// Update fan readings
    pub fn update(&mut self) {
        if !self.ec_available {
            return;
        }

        for fan in &mut self.fans {
            if let Some(speed) = ec_read_fan_speed(fan.id) {
                fan.speed_rpm = speed;
            }
            if let Some(pwm) = ec_read_fan_pwm(fan.id) {
                fan.pwm_percent = ((pwm as u32 * 100) / 255) as u8;
            }
        }
    }

    /// Set fan speed (0-100%)
    pub fn set_fan_speed(&mut self, fan_id: u8, percent: u8) -> bool {
        if !self.ec_available {
            return false;
        }

        // Enable manual mode first
        if self.global_mode != FanControlMode::Manual {
            if !ec_set_fan_mode(FanControlMode::Manual) {
                return false;
            }
            self.global_mode = FanControlMode::Manual;
        }

        let pwm = ((percent.min(100) as u32 * 255) / 100) as u8;
        if ec_set_fan_pwm(fan_id, pwm) {
            if let Some(fan) = self.fans.iter_mut().find(|f| f.id == fan_id) {
                fan.pwm_percent = percent;
                fan.mode = FanControlMode::Manual;
            }
            true
        } else {
            false
        }
    }

    /// Set all fans to auto mode
    pub fn set_auto_mode(&mut self) -> bool {
        if !self.ec_available {
            return false;
        }

        if ec_set_fan_mode(FanControlMode::Auto) {
            self.global_mode = FanControlMode::Auto;
            for fan in &mut self.fans {
                fan.mode = FanControlMode::Auto;
            }
            true
        } else {
            false
        }
    }

    /// Set all fans to full speed
    pub fn set_full_speed(&mut self) -> bool {
        if !self.ec_available {
            return false;
        }

        if ec_set_fan_mode(FanControlMode::FullSpeed) {
            self.global_mode = FanControlMode::FullSpeed;
            for fan in &mut self.fans {
                fan.mode = FanControlMode::FullSpeed;
                fan.pwm_percent = 100;
            }
            true
        } else {
            false
        }
    }

    /// Get current mode
    pub fn mode(&self) -> FanControlMode {
        self.global_mode
    }

    /// Check if EC is available
    pub fn is_available(&self) -> bool {
        self.ec_available
    }
}

/// Global fan controller
static FAN_CONTROLLER: IrqSafeMutex<FanController> = IrqSafeMutex::new(FanController::new());

/// Initialize fan controller
pub fn init_fans() {
    FAN_CONTROLLER.lock().init();
}

/// Get fan count
pub fn get_fan_count() -> usize {
    FAN_CONTROLLER.lock().fan_count()
}

/// Get fan speed in RPM
pub fn get_fan_speed(fan_id: u8) -> Option<u32> {
    let ctrl = FAN_CONTROLLER.lock();
    ctrl.fans().iter()
        .find(|f| f.id == fan_id)
        .map(|f| f.speed_rpm)
}

/// Get fan PWM percentage
pub fn get_fan_pwm(fan_id: u8) -> Option<u8> {
    let ctrl = FAN_CONTROLLER.lock();
    ctrl.fans().iter()
        .find(|f| f.id == fan_id)
        .map(|f| f.pwm_percent)
}

/// Set fan speed (0-100%)
pub fn set_fan_speed(fan_id: u8, percent: u8) -> bool {
    FAN_CONTROLLER.lock().set_fan_speed(fan_id, percent)
}

/// Set all fans to auto mode
pub fn set_fans_auto() -> bool {
    FAN_CONTROLLER.lock().set_auto_mode()
}

/// Set all fans to full speed
pub fn set_fans_full() -> bool {
    FAN_CONTROLLER.lock().set_full_speed()
}

/// Update all fan readings
pub fn update_fans() {
    FAN_CONTROLLER.lock().update();
}

/// Get fan control mode
pub fn get_fan_mode() -> FanControlMode {
    FAN_CONTROLLER.lock().mode()
}

/// Check if fan control is available
pub fn fan_control_available() -> bool {
    FAN_CONTROLLER.lock().is_available()
}

// =============================================================================
// Thermal Throttling Integration
// =============================================================================

/// Throttle level (percentage of max performance)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThrottleLevel {
    None,       // 100% performance
    Light,      // 75% performance
    Medium,     // 50% performance
    Heavy,      // 25% performance
    Maximum,    // Minimum performance
}

impl ThrottleLevel {
    /// Get performance percentage for this level
    pub fn performance_percent(&self) -> u8 {
        match self {
            ThrottleLevel::None => 100,
            ThrottleLevel::Light => 75,
            ThrottleLevel::Medium => 50,
            ThrottleLevel::Heavy => 25,
            ThrottleLevel::Maximum => 10,
        }
    }

    /// Get throttle level from temperature margin (how close to critical)
    pub fn from_temp_margin(margin_mcelsius: Temperature) -> Self {
        if margin_mcelsius >= 30000 {
            ThrottleLevel::None       // 30°C+ below critical
        } else if margin_mcelsius >= 20000 {
            ThrottleLevel::Light      // 20-30°C below critical
        } else if margin_mcelsius >= 10000 {
            ThrottleLevel::Medium     // 10-20°C below critical
        } else if margin_mcelsius >= 5000 {
            ThrottleLevel::Heavy      // 5-10°C below critical
        } else {
            ThrottleLevel::Maximum    // <5°C below critical
        }
    }
}

/// Current throttle state
static CURRENT_THROTTLE: IrqSafeMutex<ThrottleLevel> = IrqSafeMutex::new(ThrottleLevel::None);

/// Apply thermal throttling based on current temperature
pub fn apply_thermal_throttling(current_temp: Temperature, critical_temp: Temperature) {
    let margin = critical_temp - current_temp;
    let new_level = ThrottleLevel::from_temp_margin(margin);
    let old_level = *CURRENT_THROTTLE.lock();

    if new_level != old_level {
        *CURRENT_THROTTLE.lock() = new_level;

        let perf = new_level.performance_percent();

        // Integrate with cpufreq if available
        #[cfg(feature = "cpufreq")]
        {
            use crate::arch::x86_64_arch::cpufreq;

            // Calculate target frequency as percentage of max
            if let Some(max_freq) = cpufreq::get_max_freq() {
                let target_freq = (max_freq as u64 * perf as u64 / 100) as u32;
                let _ = cpufreq::set_max_freq_limit(target_freq);
            }
        }

        match new_level {
            ThrottleLevel::None => {
                crate::kprintln!("thermal: throttling disabled (temp: {}mC)", current_temp);
            }
            _ => {
                crate::kprintln!(
                    "thermal: throttling to {}% (temp: {}mC, margin: {}mC)",
                    perf, current_temp, margin
                );
            }
        }
    }
}

/// Get current throttle level
pub fn get_throttle_level() -> ThrottleLevel {
    *CURRENT_THROTTLE.lock()
}

/// Check if system is being throttled
pub fn is_throttled() -> bool {
    *CURRENT_THROTTLE.lock() != ThrottleLevel::None
}

/// Clear throttling (reset to full performance)
pub fn clear_throttling() {
    *CURRENT_THROTTLE.lock() = ThrottleLevel::None;

    #[cfg(feature = "cpufreq")]
    {
        use crate::arch::x86_64_arch::cpufreq;
        if let Some(max_freq) = cpufreq::get_hardware_max_freq() {
            let _ = cpufreq::set_max_freq_limit(max_freq);
        }
    }
}

/// Thermal subsystem
pub struct ThermalSubsystem {
    zones: Vec<ThermalZone>,
    cooling_devices: Vec<CoolingDevice>,
    critical_shutdown_enabled: bool,
}

impl ThermalSubsystem {
    pub const fn new() -> Self {
        Self {
            zones: Vec::new(),
            cooling_devices: Vec::new(),
            critical_shutdown_enabled: true,
        }
    }

    /// Add a thermal zone
    pub fn add_zone(&mut self, zone: ThermalZone) {
        self.zones.push(zone);
    }

    /// Add a cooling device
    pub fn add_cooling_device(&mut self, device: CoolingDevice) {
        self.cooling_devices.push(device);
    }

    /// Get all thermal zones
    pub fn zones(&self) -> &[ThermalZone] {
        &self.zones
    }

    /// Get a thermal zone by name
    pub fn zone(&self, name: &str) -> Option<&ThermalZone> {
        self.zones.iter().find(|z| z.name == name)
    }

    /// Get a thermal zone by name (mutable)
    pub fn zone_mut(&mut self, name: &str) -> Option<&mut ThermalZone> {
        self.zones.iter_mut().find(|z| z.name == name)
    }

    /// Update temperature for a zone
    pub fn update_zone_temp(&mut self, name: &str, temp: Temperature) -> Option<ThermalZoneState> {
        if let Some(zone) = self.zone_mut(name) {
            Some(zone.update(temp))
        } else {
            None
        }
    }

    /// Check if any zone is in critical state
    pub fn any_critical(&self) -> bool {
        self.zones.iter().any(|z| z.is_critical())
    }

    /// Get maximum temperature across all zones
    pub fn max_temperature(&self) -> Temperature {
        self.zones.iter().map(|z| z.temperature).max().unwrap_or(0)
    }

    /// Enable/disable critical temperature shutdown
    pub fn set_critical_shutdown(&mut self, enabled: bool) {
        self.critical_shutdown_enabled = enabled;
    }

    /// Check if critical shutdown is enabled
    pub fn critical_shutdown_enabled(&self) -> bool {
        self.critical_shutdown_enabled
    }
}

/// Global thermal subsystem
static THERMAL: IrqSafeMutex<ThermalSubsystem> = IrqSafeMutex::new(ThermalSubsystem::new());

/// Initialize the thermal subsystem
pub fn init() {
    // Get sensor info
    let sensor_info = get_sensor_info();
    crate::kprintln!("thermal: CPU vendor: {:?}, DTS: {}, TjMax: {}°C",
        sensor_info.vendor,
        sensor_info.has_digital_thermal_sensor,
        sensor_info.tjmax / 1000);

    let mut thermal = THERMAL.lock();

    // Create CPU thermal zone
    let mut cpu_zone = ThermalZone::new("cpu-thermal");

    // Set trip points relative to TjMax for Intel, or use defaults
    let tjmax = sensor_info.tjmax;

    // Active trip point (fan kicks in) - 50°C below TjMax
    cpu_zone.add_trip_point(TripPoint {
        trip_type: TripPointType::Active,
        temperature: (tjmax - 50000).max(40000), // At least 40°C
        hysteresis: 3000,
    });

    // Passive trip point (throttling) - 20°C below TjMax
    cpu_zone.add_trip_point(TripPoint {
        trip_type: TripPointType::Passive,
        temperature: (tjmax - 20000).max(60000), // At least 60°C
        hysteresis: 5000,
    });

    // Hot trip point (warning) - 10°C below TjMax
    cpu_zone.add_trip_point(TripPoint {
        trip_type: TripPointType::Hot,
        temperature: (tjmax - 10000).max(80000), // At least 80°C
        hysteresis: 5000,
    });

    // Critical trip point (emergency shutdown) - 5°C below TjMax
    cpu_zone.add_trip_point(TripPoint {
        trip_type: TripPointType::Critical,
        temperature: (tjmax - 5000).max(95000), // At least 95°C
        hysteresis: 0,
    });

    // Read initial temperature from hardware
    cpu_zone.temperature = read_cpu_temperature().unwrap_or(35000);
    crate::kprintln!("thermal: CPU temperature: {}°C",
        cpu_zone.temperature / 1000);

    thermal.add_zone(cpu_zone);

    // Add processor cooling device (for throttling integration)
    let processor_cooler = CoolingDevice::new("processor", CoolingDeviceType::Processor, 10);
    thermal.add_cooling_device(processor_cooler);

    drop(thermal); // Release lock before init_fans

    // Initialize fan controller
    init_fans();

    let fan_count = get_fan_count();
    crate::kprintln!("thermal: initialized with {} thermal zone(s), {} fan(s)",
        zone_count(), fan_count);
}

/// Get number of thermal zones
pub fn zone_count() -> usize {
    THERMAL.lock().zones().len()
}

/// Get all zone names
pub fn zone_names() -> Vec<String> {
    THERMAL.lock()
        .zones()
        .iter()
        .map(|z| z.name.clone())
        .collect()
}

/// Get temperature for a zone in millidegrees Celsius
pub fn get_temperature(zone_name: &str) -> Option<Temperature> {
    THERMAL.lock().zone(zone_name).map(|z| z.temperature)
}

/// Get temperature for a zone in degrees Celsius
pub fn get_temperature_celsius(zone_name: &str) -> Option<f32> {
    THERMAL.lock().zone(zone_name).map(|z| z.temp_celsius())
}

/// Update temperature for a zone
pub fn update_temperature(zone_name: &str, temp_mcelsius: Temperature) -> Option<ThermalZoneState> {
    THERMAL.lock().update_zone_temp(zone_name, temp_mcelsius)
}

/// Get the thermal state for a zone
pub fn get_zone_state(zone_name: &str) -> Option<ThermalZoneState> {
    THERMAL.lock().zone(zone_name).map(|z| z.state)
}

/// Check if any zone is in critical state
pub fn is_critical() -> bool {
    THERMAL.lock().any_critical()
}

/// Get maximum temperature across all zones
pub fn max_temperature() -> Temperature {
    THERMAL.lock().max_temperature()
}

/// Get maximum temperature in degrees Celsius
pub fn max_temperature_celsius() -> f32 {
    max_temperature() as f32 / 1000.0
}

/// Handle critical temperature - emergency shutdown
pub fn handle_critical_temperature() {
    let thermal = THERMAL.lock();

    if !thermal.critical_shutdown_enabled {
        crate::kprintln!("thermal: CRITICAL temperature but shutdown disabled!");
        return;
    }

    // Find the critical zone
    for zone in thermal.zones() {
        if zone.is_critical() {
            crate::kprintln!(
                "THERMAL EMERGENCY: {} at {}°C (critical: {}°C)",
                zone.name,
                zone.temp_celsius(),
                zone.critical_temp().unwrap_or(0) as f32 / 1000.0
            );
        }
    }

    drop(thermal); // Release lock before shutdown

    crate::kprintln!("thermal: INITIATING EMERGENCY SHUTDOWN!");

    // Immediate shutdown - no waiting
    crate::drivers::acpi::shutdown();
}

/// Poll thermal zones and handle events
/// Should be called periodically (e.g., every second)
pub fn poll_thermal() {
    // Read hardware temperature
    if let Some(temp) = read_cpu_temperature() {
        // Update cpu-thermal zone
        if let Some(state) = update_temperature("cpu-thermal", temp) {
            // Get critical threshold for throttling
            let critical = get_critical_threshold("cpu-thermal")
                .map(|t| (t * 1000.0) as Temperature)
                .unwrap_or(105000);

            match state {
                ThermalZoneState::Normal => {
                    // Clear any throttling
                    if is_throttled() {
                        clear_throttling();
                    }
                }
                ThermalZoneState::Throttling => {
                    // Apply thermal throttling
                    apply_thermal_throttling(temp, critical);
                }
                ThermalZoneState::Hot => {
                    // Heavy throttling + fan full speed
                    apply_thermal_throttling(temp, critical);
                    let _ = set_fans_full();
                }
                ThermalZoneState::Critical => {
                    handle_critical_temperature();
                    return;
                }
            }
        }
    }

    // Update fan readings
    update_fans();
}

/// Poll thermal with full status report
/// Returns (temperature_mC, state, throttle_level, fan_rpm)
pub fn poll_thermal_status() -> (Temperature, ThermalZoneState, ThrottleLevel, Option<u32>) {
    poll_thermal();

    let temp = get_temperature("cpu-thermal").unwrap_or(0);
    let state = get_zone_state("cpu-thermal").unwrap_or(ThermalZoneState::Normal);
    let throttle = get_throttle_level();
    let fan = get_fan_speed(0);

    (temp, state, throttle, fan)
}

/// Simulate temperature reading for testing
/// In real hardware, this would read from ACPI _TMP method
pub fn simulate_temperature(zone_name: &str, temp_celsius: f32) {
    let temp_mcelsius = (temp_celsius * 1000.0) as Temperature;
    if let Some(state) = update_temperature(zone_name, temp_mcelsius) {
        match state {
            ThermalZoneState::Normal => {}
            ThermalZoneState::Throttling => {
                crate::kprintln!("thermal: {} entering throttling mode at {}°C", zone_name, temp_celsius);
            }
            ThermalZoneState::Hot => {
                crate::kprintln!("thermal: WARNING! {} is HOT at {}°C", zone_name, temp_celsius);
            }
            ThermalZoneState::Critical => {
                crate::kprintln!("thermal: CRITICAL! {} at {}°C - emergency shutdown!", zone_name, temp_celsius);
                handle_critical_temperature();
            }
        }
    }
}

/// Get critical temperature threshold for a zone in degrees Celsius
pub fn get_critical_threshold(zone_name: &str) -> Option<f32> {
    THERMAL.lock()
        .zone(zone_name)
        .and_then(|z| z.critical_temp())
        .map(|t| t as f32 / 1000.0)
}

/// Set critical temperature threshold for a zone
pub fn set_critical_threshold(zone_name: &str, temp_celsius: f32) {
    let temp_mcelsius = (temp_celsius * 1000.0) as Temperature;
    let mut thermal = THERMAL.lock();
    if let Some(zone) = thermal.zone_mut(zone_name) {
        // Remove existing critical trip point
        zone.trip_points.retain(|t| t.trip_type != TripPointType::Critical);
        // Add new one
        zone.add_trip_point(TripPoint {
            trip_type: TripPointType::Critical,
            temperature: temp_mcelsius,
            hysteresis: 0,
        });
    }
}

/// Enable or disable critical shutdown
pub fn set_critical_shutdown_enabled(enabled: bool) {
    THERMAL.lock().set_critical_shutdown(enabled);
}
