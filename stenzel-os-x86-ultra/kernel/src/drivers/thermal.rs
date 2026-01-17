//! Thermal Management Driver
//!
//! Monitors system temperatures via ACPI thermal zones and triggers
//! actions (throttling, shutdown) when thresholds are exceeded.

use alloc::string::String;
use alloc::vec::Vec;
use crate::sync::IrqSafeMutex;

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
    let mut thermal = THERMAL.lock();

    // Create a default CPU thermal zone
    let mut cpu_zone = ThermalZone::new("cpu-thermal");

    // Add default trip points (typical laptop values)
    cpu_zone.add_trip_point(TripPoint {
        trip_type: TripPointType::Active,
        temperature: 50000, // 50°C - fan kicks in
        hysteresis: 3000,
    });
    cpu_zone.add_trip_point(TripPoint {
        trip_type: TripPointType::Passive,
        temperature: 80000, // 80°C - throttling starts
        hysteresis: 5000,
    });
    cpu_zone.add_trip_point(TripPoint {
        trip_type: TripPointType::Hot,
        temperature: 95000, // 95°C - warning
        hysteresis: 5000,
    });
    cpu_zone.add_trip_point(TripPoint {
        trip_type: TripPointType::Critical,
        temperature: 105000, // 105°C - emergency shutdown
        hysteresis: 0,
    });

    // Set initial temperature to room temperature
    cpu_zone.temperature = 35000; // 35°C

    thermal.add_zone(cpu_zone);

    // Add a simulated CPU fan
    let fan = CoolingDevice::new("cpu-fan", CoolingDeviceType::Fan, 7);
    thermal.add_cooling_device(fan);

    crate::kprintln!("thermal: subsystem initialized with {} zone(s)", thermal.zones.len());
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
    // In a real implementation, this would read from ACPI thermal zones
    // For now, we just check the current state

    let critical = is_critical();
    if critical {
        handle_critical_temperature();
    }
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
