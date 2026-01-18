//! ACPI Battery Driver
//!
//! Provides battery status monitoring via ACPI methods.
//! Exposes battery information through /sys/class/power_supply interface.

use alloc::string::String;
use alloc::vec::Vec;
use crate::sync::IrqSafeMutex;

/// Battery state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatteryState {
    Unknown,
    Charging,
    Discharging,
    Full,
    NotCharging,
    Critical,
}

impl BatteryState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unknown => "Unknown",
            Self::Charging => "Charging",
            Self::Discharging => "Discharging",
            Self::Full => "Full",
            Self::NotCharging => "Not charging",
            Self::Critical => "Critical",
        }
    }

    /// Parse from ACPI _BST state field
    pub fn from_acpi_state(state: u32) -> Self {
        // ACPI _BST state bits:
        // Bit 0: Discharging
        // Bit 1: Charging
        // Bit 2: Critical
        if state & 0x4 != 0 {
            Self::Critical
        } else if state & 0x2 != 0 {
            Self::Charging
        } else if state & 0x1 != 0 {
            Self::Discharging
        } else {
            Self::Full
        }
    }
}

/// Battery technology type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatteryTechnology {
    Unknown,
    LithiumIon,
    LithiumPolymer,
    NickelMetalHydride,
    NickelCadmium,
    LeadAcid,
}

impl BatteryTechnology {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unknown => "Unknown",
            Self::LithiumIon => "Li-ion",
            Self::LithiumPolymer => "Li-poly",
            Self::NickelMetalHydride => "NiMH",
            Self::NickelCadmium => "NiCd",
            Self::LeadAcid => "Lead-acid",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "LION" | "LI-ION" => Self::LithiumIon,
            "LIPO" | "LI-POLY" => Self::LithiumPolymer,
            "NIMH" => Self::NickelMetalHydride,
            "NICD" => Self::NickelCadmium,
            "PBAC" => Self::LeadAcid,
            _ => Self::Unknown,
        }
    }
}

/// Battery information (from ACPI _BIF/_BIX)
#[derive(Debug, Clone)]
pub struct BatteryInfo {
    /// Battery name/identifier
    pub name: String,
    /// Manufacturer name
    pub manufacturer: String,
    /// Model number
    pub model: String,
    /// Serial number
    pub serial: String,
    /// Technology type
    pub technology: BatteryTechnology,
    /// Design capacity in mWh or mAh
    pub design_capacity: u32,
    /// Last full charge capacity in mWh or mAh
    pub full_charge_capacity: u32,
    /// Design voltage in mV
    pub design_voltage: u32,
    /// Whether capacity is reported in mWh (true) or mAh (false)
    pub capacity_in_mwh: bool,
    /// Cycle count (if available)
    pub cycle_count: Option<u32>,
}

impl Default for BatteryInfo {
    fn default() -> Self {
        Self {
            name: String::from("BAT0"),
            manufacturer: String::from("Unknown"),
            model: String::from("Unknown"),
            serial: String::from("Unknown"),
            technology: BatteryTechnology::Unknown,
            design_capacity: 0,
            full_charge_capacity: 0,
            design_voltage: 0,
            capacity_in_mwh: true,
            cycle_count: None,
        }
    }
}

/// Battery status (from ACPI _BST)
#[derive(Debug, Clone)]
pub struct BatteryStatus {
    /// Current state
    pub state: BatteryState,
    /// Present rate in mW or mA (depending on capacity_in_mwh)
    pub present_rate: u32,
    /// Remaining capacity in mWh or mAh
    pub remaining_capacity: u32,
    /// Present voltage in mV
    pub present_voltage: u32,
    /// Calculated percentage (0-100)
    pub percentage: u8,
    /// Estimated time to empty in minutes (if discharging)
    pub time_to_empty: Option<u32>,
    /// Estimated time to full in minutes (if charging)
    pub time_to_full: Option<u32>,
}

impl Default for BatteryStatus {
    fn default() -> Self {
        Self {
            state: BatteryState::Unknown,
            present_rate: 0,
            remaining_capacity: 0,
            present_voltage: 0,
            percentage: 0,
            time_to_empty: None,
            time_to_full: None,
        }
    }
}

/// Complete battery data
#[derive(Debug, Clone)]
pub struct Battery {
    pub info: BatteryInfo,
    pub status: BatteryStatus,
    pub present: bool,
}

impl Battery {
    pub fn new(name: &str) -> Self {
        Self {
            info: BatteryInfo {
                name: String::from(name),
                ..Default::default()
            },
            status: BatteryStatus::default(),
            present: false,
        }
    }

    /// Calculate percentage from remaining capacity and full charge capacity
    pub fn calculate_percentage(&mut self) {
        if self.info.full_charge_capacity > 0 {
            let pct = (self.status.remaining_capacity as u64 * 100)
                / self.info.full_charge_capacity as u64;
            self.status.percentage = pct.min(100) as u8;
        }
    }

    /// Calculate time estimates
    pub fn calculate_time_estimates(&mut self) {
        if self.status.present_rate == 0 {
            self.status.time_to_empty = None;
            self.status.time_to_full = None;
            return;
        }

        match self.status.state {
            BatteryState::Discharging => {
                // Time to empty in minutes
                let minutes = (self.status.remaining_capacity as u64 * 60)
                    / self.status.present_rate as u64;
                self.status.time_to_empty = Some(minutes as u32);
                self.status.time_to_full = None;
            }
            BatteryState::Charging => {
                // Time to full in minutes
                let remaining = self.info.full_charge_capacity
                    .saturating_sub(self.status.remaining_capacity);
                let minutes = (remaining as u64 * 60) / self.status.present_rate as u64;
                self.status.time_to_full = Some(minutes as u32);
                self.status.time_to_empty = None;
            }
            _ => {
                self.status.time_to_empty = None;
                self.status.time_to_full = None;
            }
        }
    }
}

/// AC adapter state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcAdapterState {
    Unknown,
    Online,
    Offline,
}

impl AcAdapterState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unknown => "Unknown",
            Self::Online => "Online",
            Self::Offline => "Offline",
        }
    }
}

/// AC adapter information
#[derive(Debug, Clone)]
pub struct AcAdapter {
    pub name: String,
    pub state: AcAdapterState,
}

impl Default for AcAdapter {
    fn default() -> Self {
        Self {
            name: String::from("AC0"),
            state: AcAdapterState::Unknown,
        }
    }
}

/// Power supply subsystem
pub struct PowerSupply {
    batteries: Vec<Battery>,
    ac_adapters: Vec<AcAdapter>,
}

impl PowerSupply {
    pub const fn new() -> Self {
        Self {
            batteries: Vec::new(),
            ac_adapters: Vec::new(),
        }
    }

    /// Add a battery
    pub fn add_battery(&mut self, battery: Battery) {
        self.batteries.push(battery);
    }

    /// Add an AC adapter
    pub fn add_ac_adapter(&mut self, adapter: AcAdapter) {
        self.ac_adapters.push(adapter);
    }

    /// Get all batteries
    pub fn batteries(&self) -> &[Battery] {
        &self.batteries
    }

    /// Get all AC adapters
    pub fn ac_adapters(&self) -> &[AcAdapter] {
        &self.ac_adapters
    }

    /// Get battery by name
    pub fn battery(&self, name: &str) -> Option<&Battery> {
        self.batteries.iter().find(|b| b.info.name == name)
    }

    /// Get battery by name (mutable)
    pub fn battery_mut(&mut self, name: &str) -> Option<&mut Battery> {
        self.batteries.iter_mut().find(|b| b.info.name == name)
    }

    /// Update battery status (called periodically or on event)
    pub fn update_battery(&mut self, name: &str, status: BatteryStatus) {
        if let Some(battery) = self.battery_mut(name) {
            battery.status = status;
            battery.calculate_percentage();
            battery.calculate_time_estimates();
        }
    }

    /// Get total battery percentage (average across all batteries)
    pub fn total_percentage(&self) -> u8 {
        if self.batteries.is_empty() {
            return 0;
        }
        let total: u32 = self.batteries.iter()
            .filter(|b| b.present)
            .map(|b| b.status.percentage as u32)
            .sum();
        let count = self.batteries.iter().filter(|b| b.present).count() as u32;
        if count == 0 {
            0
        } else {
            (total / count) as u8
        }
    }

    /// Check if any battery is charging
    pub fn is_charging(&self) -> bool {
        self.batteries.iter()
            .any(|b| b.present && b.status.state == BatteryState::Charging)
    }

    /// Check if on AC power
    pub fn on_ac_power(&self) -> bool {
        self.ac_adapters.iter()
            .any(|a| a.state == AcAdapterState::Online)
    }

    /// Get combined battery state
    pub fn combined_state(&self) -> BatteryState {
        if self.batteries.iter().any(|b| b.status.state == BatteryState::Critical) {
            return BatteryState::Critical;
        }
        if self.batteries.iter().any(|b| b.status.state == BatteryState::Charging) {
            return BatteryState::Charging;
        }
        if self.batteries.iter().all(|b| b.status.state == BatteryState::Full) {
            return BatteryState::Full;
        }
        if self.batteries.iter().any(|b| b.status.state == BatteryState::Discharging) {
            return BatteryState::Discharging;
        }
        BatteryState::Unknown
    }
}

/// Global power supply subsystem
static POWER_SUPPLY: IrqSafeMutex<PowerSupply> = IrqSafeMutex::new(PowerSupply::new());

/// Initialize the power supply subsystem
pub fn init() {
    let mut ps = POWER_SUPPLY.lock();

    // For now, create a simulated battery for testing
    // On real hardware, this would scan ACPI for battery devices
    let mut bat0 = Battery::new("BAT0");
    bat0.present = true;
    bat0.info.manufacturer = String::from("StenzelOS");
    bat0.info.model = String::from("Virtual Battery");
    bat0.info.technology = BatteryTechnology::LithiumIon;
    bat0.info.design_capacity = 50000; // 50Wh
    bat0.info.full_charge_capacity = 45000; // 45Wh (some wear)
    bat0.info.design_voltage = 11100; // 11.1V (3-cell)
    bat0.status.state = BatteryState::Full;
    bat0.status.remaining_capacity = 45000;
    bat0.status.present_voltage = 12600;
    bat0.status.percentage = 100;
    ps.add_battery(bat0);

    // Add AC adapter
    let ac = AcAdapter {
        name: String::from("AC0"),
        state: AcAdapterState::Online,
    };
    ps.add_ac_adapter(ac);

    crate::kprintln!("battery: power supply subsystem initialized");
}

/// Get current battery percentage (0-100)
pub fn percentage() -> u8 {
    POWER_SUPPLY.lock().total_percentage()
}

/// Check if any battery is charging
pub fn is_charging() -> bool {
    POWER_SUPPLY.lock().is_charging()
}

/// Check if on AC power
pub fn on_ac_power() -> bool {
    POWER_SUPPLY.lock().on_ac_power()
}

/// Get combined battery state
pub fn state() -> BatteryState {
    POWER_SUPPLY.lock().combined_state()
}

/// Get battery info for a specific battery
pub fn battery_info(name: &str) -> Option<BatteryInfo> {
    POWER_SUPPLY.lock().battery(name).map(|b| b.info.clone())
}

/// Get battery status for a specific battery
pub fn battery_status(name: &str) -> Option<BatteryStatus> {
    POWER_SUPPLY.lock().battery(name).map(|b| b.status.clone())
}

/// Get number of batteries
pub fn battery_count() -> usize {
    POWER_SUPPLY.lock().batteries().len()
}

/// Get all battery names
pub fn battery_names() -> Vec<String> {
    POWER_SUPPLY.lock()
        .batteries()
        .iter()
        .map(|b| b.info.name.clone())
        .collect()
}

/// Update battery status (for testing or ACPI updates)
pub fn update_status(name: &str, state: BatteryState, remaining: u32, rate: u32, voltage: u32) {
    let status = BatteryStatus {
        state,
        present_rate: rate,
        remaining_capacity: remaining,
        present_voltage: voltage,
        percentage: 0, // Will be calculated
        time_to_empty: None,
        time_to_full: None,
    };
    POWER_SUPPLY.lock().update_battery(name, status);
}

/// Set AC adapter state
pub fn set_ac_state(name: &str, online: bool) {
    let mut ps = POWER_SUPPLY.lock();
    if let Some(ac) = ps.ac_adapters.iter_mut().find(|a| a.name == name) {
        ac.state = if online {
            AcAdapterState::Online
        } else {
            AcAdapterState::Offline
        };
    }
}

/// Check if battery is critical (< 5%)
pub fn is_critical() -> bool {
    let ps = POWER_SUPPLY.lock();
    ps.batteries.iter().any(|b| {
        b.present && (b.status.state == BatteryState::Critical || b.status.percentage < 5)
    })
}

/// Get estimated time to empty in minutes (None if charging or unknown)
pub fn time_to_empty() -> Option<u32> {
    let ps = POWER_SUPPLY.lock();
    // Return minimum time to empty across all discharging batteries
    ps.batteries.iter()
        .filter(|b| b.present && b.status.state == BatteryState::Discharging)
        .filter_map(|b| b.status.time_to_empty)
        .min()
}

/// Get estimated time to full in minutes (None if not charging)
pub fn time_to_full() -> Option<u32> {
    let ps = POWER_SUPPLY.lock();
    // Return maximum time to full across all charging batteries
    ps.batteries.iter()
        .filter(|b| b.present && b.status.state == BatteryState::Charging)
        .filter_map(|b| b.status.time_to_full)
        .max()
}

// ============================================================================
// Battery Health Monitoring
// ============================================================================

/// Battery health status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Excellent,  // > 90% capacity retention
    Good,       // 80-90% capacity retention
    Fair,       // 60-80% capacity retention
    Poor,       // 40-60% capacity retention
    Critical,   // < 40% capacity retention
    Unknown,
}

impl HealthStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Excellent => "Excellent",
            Self::Good => "Good",
            Self::Fair => "Fair",
            Self::Poor => "Poor",
            Self::Critical => "Critical",
            Self::Unknown => "Unknown",
        }
    }

    pub fn from_percentage(pct: u8) -> Self {
        match pct {
            91..=100 => Self::Excellent,
            80..=90 => Self::Good,
            60..=79 => Self::Fair,
            40..=59 => Self::Poor,
            0..=39 => Self::Critical,
            _ => Self::Unknown,
        }
    }
}

/// Battery health information
#[derive(Debug, Clone)]
pub struct BatteryHealth {
    /// Health status
    pub status: HealthStatus,
    /// Health percentage (capacity retention)
    pub health_percentage: u8,
    /// Design capacity in mWh
    pub design_capacity: u32,
    /// Current full charge capacity in mWh
    pub full_charge_capacity: u32,
    /// Cycle count
    pub cycle_count: u32,
    /// Estimated cycles remaining (based on typical battery life)
    pub estimated_cycles_remaining: Option<u32>,
    /// Manufacturing date (if available)
    pub manufacture_date: Option<String>,
    /// First use date (if tracked)
    pub first_use_date: Option<u64>,
    /// Total energy consumed (mWh lifetime)
    pub total_energy_consumed: u64,
    /// Average discharge rate
    pub avg_discharge_rate: u32,
    /// Max observed temperature (Celsius * 10)
    pub max_temperature: Option<i16>,
    /// Current temperature (Celsius * 10)
    pub current_temperature: Option<i16>,
    /// Number of deep discharge events (< 10%)
    pub deep_discharge_count: u32,
    /// Number of overcharge events
    pub overcharge_count: u32,
    /// Last calibration timestamp
    pub last_calibration: Option<u64>,
    /// Needs calibration
    pub needs_calibration: bool,
}

impl BatteryHealth {
    pub fn new(design_capacity: u32, full_charge_capacity: u32) -> Self {
        let health_pct = if design_capacity > 0 {
            ((full_charge_capacity as u64 * 100) / design_capacity as u64).min(100) as u8
        } else {
            0
        };

        Self {
            status: HealthStatus::from_percentage(health_pct),
            health_percentage: health_pct,
            design_capacity,
            full_charge_capacity,
            cycle_count: 0,
            estimated_cycles_remaining: None,
            manufacture_date: None,
            first_use_date: None,
            total_energy_consumed: 0,
            avg_discharge_rate: 0,
            max_temperature: None,
            current_temperature: None,
            deep_discharge_count: 0,
            overcharge_count: 0,
            last_calibration: None,
            needs_calibration: false,
        }
    }

    /// Update health from current battery data
    pub fn update(&mut self, info: &BatteryInfo, _status: &BatteryStatus) {
        self.design_capacity = info.design_capacity;
        self.full_charge_capacity = info.full_charge_capacity;

        if self.design_capacity > 0 {
            self.health_percentage = ((self.full_charge_capacity as u64 * 100)
                / self.design_capacity as u64).min(100) as u8;
            self.status = HealthStatus::from_percentage(self.health_percentage);
        }

        if let Some(cycles) = info.cycle_count {
            self.cycle_count = cycles;
            // Estimate remaining cycles (typical Li-ion: 500-1000 cycles)
            let max_cycles: u32 = match info.technology {
                BatteryTechnology::LithiumIon | BatteryTechnology::LithiumPolymer => 800,
                BatteryTechnology::NickelMetalHydride => 500,
                _ => 500,
            };
            self.estimated_cycles_remaining = Some(max_cycles.saturating_sub(cycles));
        }

        // Check if calibration is needed (every 3 months or 100 cycles)
        self.needs_calibration = self.cycle_count > 0 && self.cycle_count % 100 == 0;
    }

    /// Get wear level as percentage
    pub fn wear_level(&self) -> u8 {
        100u8.saturating_sub(self.health_percentage)
    }

    /// Estimate battery lifespan in months remaining
    pub fn estimated_lifespan_months(&self) -> Option<u32> {
        if let Some(cycles_remaining) = self.estimated_cycles_remaining {
            // Assume ~1 cycle per day average
            Some(cycles_remaining / 30)
        } else {
            None
        }
    }

    /// Check if battery should be replaced
    pub fn should_replace(&self) -> bool {
        self.health_percentage < 60 || self.status == HealthStatus::Poor || self.status == HealthStatus::Critical
    }
}

/// Battery health history entry
#[derive(Debug, Clone)]
pub struct HealthHistoryEntry {
    pub timestamp: u64,
    pub health_percentage: u8,
    pub cycle_count: u32,
    pub full_charge_capacity: u32,
}

/// Battery health tracker
pub struct BatteryHealthTracker {
    health_data: BTreeMap<String, BatteryHealth>,
    history: BTreeMap<String, Vec<HealthHistoryEntry>>,
    max_history_entries: usize,
    current_time: u64,
}

use alloc::collections::BTreeMap;

impl BatteryHealthTracker {
    pub fn new() -> Self {
        Self {
            health_data: BTreeMap::new(),
            history: BTreeMap::new(),
            max_history_entries: 365, // ~1 year of daily entries
            current_time: 0,
        }
    }

    /// Get health for a battery
    pub fn get_health(&self, name: &str) -> Option<&BatteryHealth> {
        self.health_data.get(name)
    }

    /// Update health for a battery
    pub fn update_health(&mut self, battery: &Battery) {
        let name = battery.info.name.clone();

        let health = self.health_data
            .entry(name.clone())
            .or_insert_with(|| BatteryHealth::new(
                battery.info.design_capacity,
                battery.info.full_charge_capacity
            ));

        health.update(&battery.info, &battery.status);

        // Record history entry (once per day or significant change)
        let should_record = {
            let history = self.history.entry(name.clone()).or_insert_with(Vec::new);
            history.last().map_or(true, |last| {
                // Record if > 1 day has passed or health changed by > 1%
                let time_diff = self.current_time.saturating_sub(last.timestamp) > 86400;
                let health_diff = (health.health_percentage as i16 - last.health_percentage as i16).abs() > 1;
                time_diff || health_diff
            })
        };

        if should_record {
            let entry = HealthHistoryEntry {
                timestamp: self.current_time,
                health_percentage: health.health_percentage,
                cycle_count: health.cycle_count,
                full_charge_capacity: health.full_charge_capacity,
            };

            let history = self.history.entry(name).or_insert_with(Vec::new);
            history.push(entry);

            // Trim old entries
            if history.len() > self.max_history_entries {
                history.remove(0);
            }
        }
    }

    /// Get health history for a battery
    pub fn get_history(&self, name: &str) -> Option<&Vec<HealthHistoryEntry>> {
        self.history.get(name)
    }

    /// Get health degradation rate (percentage per month)
    pub fn degradation_rate(&self, name: &str) -> Option<f32> {
        let history = self.history.get(name)?;
        if history.len() < 2 {
            return None;
        }

        let first = history.first()?;
        let last = history.last()?;

        let months = (last.timestamp - first.timestamp) as f32 / (30.0 * 24.0 * 3600.0);
        if months < 1.0 {
            return None;
        }

        let health_loss = first.health_percentage as f32 - last.health_percentage as f32;
        Some(health_loss / months)
    }

    /// Set current time
    pub fn set_current_time(&mut self, time: u64) {
        self.current_time = time;
    }

    /// Add sample data for demo
    pub fn add_sample_data(&mut self) {
        self.current_time = 1705600000;

        // Create sample health data
        let mut health = BatteryHealth::new(57000, 52000);
        health.cycle_count = 245;
        health.estimated_cycles_remaining = Some(555);
        health.manufacture_date = Some(String::from("2022-06"));
        health.first_use_date = Some(1656633600); // July 2022
        health.total_energy_consumed = 890000;
        health.avg_discharge_rate = 15000;
        health.max_temperature = Some(450); // 45.0°C
        health.current_temperature = Some(320); // 32.0°C
        health.deep_discharge_count = 3;
        health.overcharge_count = 0;
        health.last_calibration = Some(1702944000);
        health.needs_calibration = false;

        self.health_data.insert(String::from("BAT0"), health);

        // Add some history
        let mut history = Vec::new();
        for i in 0..12 {
            history.push(HealthHistoryEntry {
                timestamp: 1673049600 + i * 30 * 86400, // Monthly entries for a year
                health_percentage: 95 - i as u8,
                cycle_count: 20 * (i as u32 + 1),
                full_charge_capacity: 57000 - i as u32 * 500,
            });
        }
        self.history.insert(String::from("BAT0"), history);
    }
}

impl Default for BatteryHealthTracker {
    fn default() -> Self {
        Self::new()
    }
}

// Global health tracker
static HEALTH_TRACKER: IrqSafeMutex<Option<BatteryHealthTracker>> = IrqSafeMutex::new(None);

/// Initialize health tracking
pub fn init_health_tracking() {
    let mut tracker = BatteryHealthTracker::new();
    tracker.add_sample_data();
    *HEALTH_TRACKER.lock() = Some(tracker);
}

/// Get battery health
pub fn get_battery_health(name: &str) -> Option<BatteryHealth> {
    HEALTH_TRACKER.lock().as_ref().and_then(|t| t.get_health(name).cloned())
}

/// Get health status
pub fn health_status(name: &str) -> Option<HealthStatus> {
    get_battery_health(name).map(|h| h.status)
}

/// Get wear level
pub fn wear_level(name: &str) -> Option<u8> {
    get_battery_health(name).map(|h| h.wear_level())
}

/// Check if battery should be replaced
pub fn should_replace_battery(name: &str) -> Option<bool> {
    get_battery_health(name).map(|h| h.should_replace())
}

/// Get estimated lifespan
pub fn estimated_lifespan_months(name: &str) -> Option<u32> {
    get_battery_health(name).and_then(|h| h.estimated_lifespan_months())
}

/// Get health degradation rate
pub fn degradation_rate(name: &str) -> Option<f32> {
    HEALTH_TRACKER.lock().as_ref().and_then(|t| t.degradation_rate(name))
}

/// Update health tracking (call periodically)
pub fn update_health_tracking() {
    let ps = POWER_SUPPLY.lock();
    let mut tracker = HEALTH_TRACKER.lock();

    if let Some(ref mut tracker) = *tracker {
        for battery in ps.batteries.iter() {
            if battery.present {
                tracker.update_health(battery);
            }
        }
    }
}

// ============================================================================
// Charge Limit (Battery Conservation Mode)
// ============================================================================

/// Charge limit mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChargeLimitMode {
    /// No charge limit (charge to 100%)
    Full,
    /// Stop charging at 80% (better for battery longevity)
    Conservation,
    /// Stop charging at 60% (maximum longevity, for always-plugged devices)
    MaxLongevity,
    /// Custom limit (user-defined percentage)
    Custom(u8),
}

impl ChargeLimitMode {
    pub fn limit_percentage(&self) -> u8 {
        match self {
            ChargeLimitMode::Full => 100,
            ChargeLimitMode::Conservation => 80,
            ChargeLimitMode::MaxLongevity => 60,
            ChargeLimitMode::Custom(pct) => *pct,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ChargeLimitMode::Full => "Full (100%)",
            ChargeLimitMode::Conservation => "Conservation (80%)",
            ChargeLimitMode::MaxLongevity => "Max Longevity (60%)",
            ChargeLimitMode::Custom(_) => "Custom",
        }
    }
}

/// Charge threshold type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChargeThreshold {
    /// Start charging when below this percentage
    Start,
    /// Stop charging when above this percentage
    Stop,
}

/// Charge limit settings
#[derive(Debug, Clone)]
pub struct ChargeLimitSettings {
    /// Battery name this applies to
    pub battery_name: String,
    /// Whether charge limit is enabled
    pub enabled: bool,
    /// Charge limit mode
    pub mode: ChargeLimitMode,
    /// Start charging threshold (hysteresis)
    pub start_threshold: u8,
    /// Stop charging threshold
    pub stop_threshold: u8,
    /// Schedule enabled (only limit during certain hours)
    pub schedule_enabled: bool,
    /// Schedule start hour (0-23)
    pub schedule_start: u8,
    /// Schedule end hour (0-23)
    pub schedule_end: u8,
    /// Allow full charge on specific days (bitmap: bit 0=Sun, bit 6=Sat)
    pub full_charge_days: u8,
    /// Bypass limit when full charge is needed soon (e.g., trip)
    pub bypass_until: Option<u64>,
    /// Express charge mode (ignore limit when battery very low)
    pub express_charge_threshold: u8,
}

impl ChargeLimitSettings {
    pub fn new(battery_name: &str) -> Self {
        Self {
            battery_name: String::from(battery_name),
            enabled: false,
            mode: ChargeLimitMode::Conservation,
            start_threshold: 75, // Start charging when below 75%
            stop_threshold: 80,  // Stop when reaches 80%
            schedule_enabled: false,
            schedule_start: 22,  // 10 PM
            schedule_end: 8,     // 8 AM
            full_charge_days: 0, // No automatic full charge days
            bypass_until: None,
            express_charge_threshold: 20, // Express charge if below 20%
        }
    }

    /// Set conservation mode (80%)
    pub fn conservation() -> Self {
        let mut settings = Self::new("BAT0");
        settings.enabled = true;
        settings.mode = ChargeLimitMode::Conservation;
        settings.start_threshold = 75;
        settings.stop_threshold = 80;
        settings
    }

    /// Set max longevity mode (60%)
    pub fn max_longevity() -> Self {
        let mut settings = Self::new("BAT0");
        settings.enabled = true;
        settings.mode = ChargeLimitMode::MaxLongevity;
        settings.start_threshold = 55;
        settings.stop_threshold = 60;
        settings
    }

    /// Set custom limit
    pub fn custom(limit: u8) -> Self {
        let mut settings = Self::new("BAT0");
        settings.enabled = true;
        settings.mode = ChargeLimitMode::Custom(limit);
        settings.start_threshold = limit.saturating_sub(5);
        settings.stop_threshold = limit;
        settings
    }

    /// Should charging be stopped?
    pub fn should_stop_charging(&self, current_percentage: u8, current_time: u64) -> bool {
        if !self.enabled {
            return false;
        }

        // Check if bypass is active
        if let Some(bypass_until) = self.bypass_until {
            if current_time < bypass_until {
                return false;
            }
        }

        // Express charge - don't stop if battery is very low
        if current_percentage < self.express_charge_threshold {
            return false;
        }

        // Check schedule
        if self.schedule_enabled && !self.is_in_schedule(current_time) {
            return false;
        }

        current_percentage >= self.stop_threshold
    }

    /// Should charging be started?
    pub fn should_start_charging(&self, current_percentage: u8, current_time: u64) -> bool {
        if !self.enabled {
            return true; // Always allow charging if not enabled
        }

        // Check if bypass is active
        if let Some(bypass_until) = self.bypass_until {
            if current_time < bypass_until {
                return true;
            }
        }

        // Express charge - always charge if very low
        if current_percentage < self.express_charge_threshold {
            return true;
        }

        current_percentage <= self.start_threshold
    }

    /// Check if current time is within schedule
    fn is_in_schedule(&self, current_time: u64) -> bool {
        // Extract hour from timestamp (simplified)
        let hour = ((current_time / 3600) % 24) as u8;

        if self.schedule_start <= self.schedule_end {
            // Normal range (e.g., 8-18)
            hour >= self.schedule_start && hour < self.schedule_end
        } else {
            // Overnight range (e.g., 22-8)
            hour >= self.schedule_start || hour < self.schedule_end
        }
    }

    /// Set bypass for temporary full charge
    pub fn set_bypass(&mut self, duration_hours: u32, current_time: u64) {
        self.bypass_until = Some(current_time + (duration_hours as u64 * 3600));
    }

    /// Clear bypass
    pub fn clear_bypass(&mut self) {
        self.bypass_until = None;
    }

    /// Check if full charge day
    pub fn is_full_charge_day(&self, day_of_week: u8) -> bool {
        (self.full_charge_days >> day_of_week) & 1 == 1
    }

    /// Set full charge day
    pub fn set_full_charge_day(&mut self, day_of_week: u8, enabled: bool) {
        if enabled {
            self.full_charge_days |= 1 << day_of_week;
        } else {
            self.full_charge_days &= !(1 << day_of_week);
        }
    }
}

/// Charge limit controller interface
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChargeLimitController {
    /// No hardware support
    None,
    /// ThinkPad EC (via /sys/class/power_supply/BAT0/charge_control_*)
    ThinkPad,
    /// ASUS ACPI (_SB.PCI0.LPCB.EC0)
    Asus,
    /// Dell BIOS/EC
    Dell,
    /// HP BIOS
    Hp,
    /// Lenovo IdeaPad
    LenovoIdeaPad,
    /// MSI EC
    Msi,
    /// Samsung BIOS
    Samsung,
    /// Surface UEFI
    Surface,
    /// Generic EC (may work on some laptops)
    GenericEc,
}

impl ChargeLimitController {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChargeLimitController::None => "None",
            ChargeLimitController::ThinkPad => "ThinkPad",
            ChargeLimitController::Asus => "ASUS",
            ChargeLimitController::Dell => "Dell",
            ChargeLimitController::Hp => "HP",
            ChargeLimitController::LenovoIdeaPad => "Lenovo IdeaPad",
            ChargeLimitController::Msi => "MSI",
            ChargeLimitController::Samsung => "Samsung",
            ChargeLimitController::Surface => "Surface",
            ChargeLimitController::GenericEc => "Generic EC",
        }
    }
}

/// Charge limit manager
pub struct ChargeLimitManager {
    settings: BTreeMap<String, ChargeLimitSettings>,
    controller: ChargeLimitController,
    supported: bool,
    charging_inhibited: BTreeMap<String, bool>,
    current_time: u64,
}

impl ChargeLimitManager {
    pub fn new() -> Self {
        Self {
            settings: BTreeMap::new(),
            controller: ChargeLimitController::None,
            supported: false,
            charging_inhibited: BTreeMap::new(),
            current_time: 0,
        }
    }

    /// Detect hardware support
    pub fn detect_hardware(&mut self) {
        // In real implementation, would detect via DMI/SMBIOS, ACPI, etc.
        // For demo, assume ThinkPad support
        self.controller = ChargeLimitController::ThinkPad;
        self.supported = true;
    }

    /// Check if charge limit is supported
    pub fn is_supported(&self) -> bool {
        self.supported
    }

    /// Get controller type
    pub fn controller(&self) -> ChargeLimitController {
        self.controller
    }

    /// Set settings for a battery
    pub fn set_settings(&mut self, settings: ChargeLimitSettings) {
        let name = settings.battery_name.clone();
        self.settings.insert(name, settings);
    }

    /// Get settings for a battery
    pub fn get_settings(&self, battery_name: &str) -> Option<&ChargeLimitSettings> {
        self.settings.get(battery_name)
    }

    /// Get settings for a battery (mutable)
    pub fn get_settings_mut(&mut self, battery_name: &str) -> Option<&mut ChargeLimitSettings> {
        self.settings.get_mut(battery_name)
    }

    /// Enable charge limit for a battery
    pub fn enable(&mut self, battery_name: &str) {
        if let Some(settings) = self.settings.get_mut(battery_name) {
            settings.enabled = true;
        } else {
            let mut settings = ChargeLimitSettings::conservation();
            settings.battery_name = String::from(battery_name);
            settings.enabled = true;
            self.settings.insert(String::from(battery_name), settings);
        }
    }

    /// Disable charge limit for a battery
    pub fn disable(&mut self, battery_name: &str) {
        if let Some(settings) = self.settings.get_mut(battery_name) {
            settings.enabled = false;
        }
    }

    /// Set charge limit percentage
    pub fn set_limit(&mut self, battery_name: &str, limit: u8) {
        if let Some(settings) = self.settings.get_mut(battery_name) {
            settings.mode = ChargeLimitMode::Custom(limit);
            settings.stop_threshold = limit;
            settings.start_threshold = limit.saturating_sub(5);
        }
    }

    /// Process charging decision for a battery
    pub fn process(&mut self, battery_name: &str, current_percentage: u8) -> ChargingDecision {
        let settings = match self.settings.get(battery_name) {
            Some(s) => s,
            None => return ChargingDecision::Allow,
        };

        let currently_inhibited = self.charging_inhibited
            .get(battery_name)
            .copied()
            .unwrap_or(false);

        if settings.should_stop_charging(current_percentage, self.current_time) {
            self.charging_inhibited.insert(String::from(battery_name), true);
            ChargingDecision::Stop
        } else if settings.should_start_charging(current_percentage, self.current_time) {
            self.charging_inhibited.insert(String::from(battery_name), false);
            ChargingDecision::Allow
        } else if currently_inhibited {
            ChargingDecision::Stop
        } else {
            ChargingDecision::Allow
        }
    }

    /// Apply charge limit to hardware
    pub fn apply_to_hardware(&self, battery_name: &str) -> bool {
        let settings = match self.settings.get(battery_name) {
            Some(s) => s,
            None => return false,
        };

        if !self.supported {
            return false;
        }

        match self.controller {
            ChargeLimitController::ThinkPad => {
                // Would write to /sys/class/power_supply/BAT0/charge_control_start_threshold
                // and charge_control_end_threshold
                // Simulated success
                true
            }
            ChargeLimitController::Asus => {
                // Would call ASUS ACPI method
                true
            }
            _ => false,
        }
    }

    /// Set current time
    pub fn set_current_time(&mut self, time: u64) {
        self.current_time = time;
    }

    /// Add sample data
    pub fn add_sample_data(&mut self) {
        self.detect_hardware();
        self.current_time = 1705600000;

        // Add default conservation settings
        let mut settings = ChargeLimitSettings::conservation();
        settings.battery_name = String::from("BAT0");
        settings.enabled = true;
        self.settings.insert(String::from("BAT0"), settings);
    }
}

impl Default for ChargeLimitManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Charging decision
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChargingDecision {
    Allow,
    Stop,
}

// Global charge limit manager
static CHARGE_LIMIT_MANAGER: IrqSafeMutex<Option<ChargeLimitManager>> = IrqSafeMutex::new(None);

/// Initialize charge limit feature
pub fn init_charge_limit() {
    let mut manager = ChargeLimitManager::new();
    manager.add_sample_data();
    *CHARGE_LIMIT_MANAGER.lock() = Some(manager);
}

/// Check if charge limit is supported
pub fn charge_limit_supported() -> bool {
    CHARGE_LIMIT_MANAGER.lock().as_ref().map_or(false, |m| m.is_supported())
}

/// Get charge limit controller type
pub fn charge_limit_controller() -> Option<ChargeLimitController> {
    CHARGE_LIMIT_MANAGER.lock().as_ref().map(|m| m.controller())
}

/// Enable charge limit
pub fn enable_charge_limit(battery_name: &str) {
    if let Some(ref mut manager) = *CHARGE_LIMIT_MANAGER.lock() {
        manager.enable(battery_name);
        manager.apply_to_hardware(battery_name);
    }
}

/// Disable charge limit
pub fn disable_charge_limit(battery_name: &str) {
    if let Some(ref mut manager) = *CHARGE_LIMIT_MANAGER.lock() {
        manager.disable(battery_name);
        manager.apply_to_hardware(battery_name);
    }
}

/// Set charge limit percentage
pub fn set_charge_limit(battery_name: &str, limit: u8) {
    if let Some(ref mut manager) = *CHARGE_LIMIT_MANAGER.lock() {
        manager.set_limit(battery_name, limit);
        manager.apply_to_hardware(battery_name);
    }
}

/// Get current charge limit settings
pub fn get_charge_limit_settings(battery_name: &str) -> Option<ChargeLimitSettings> {
    CHARGE_LIMIT_MANAGER.lock().as_ref().and_then(|m| m.get_settings(battery_name).cloned())
}

/// Process charge limit for a battery
pub fn process_charge_limit(battery_name: &str, current_percentage: u8) -> ChargingDecision {
    if let Some(ref mut manager) = *CHARGE_LIMIT_MANAGER.lock() {
        manager.process(battery_name, current_percentage)
    } else {
        ChargingDecision::Allow
    }
}

/// Set bypass for temporary full charge
pub fn set_charge_limit_bypass(battery_name: &str, duration_hours: u32) {
    if let Some(ref mut manager) = *CHARGE_LIMIT_MANAGER.lock() {
        let current_time = manager.current_time;
        if let Some(settings) = manager.get_settings_mut(battery_name) {
            settings.set_bypass(duration_hours, current_time);
        }
        manager.apply_to_hardware(battery_name);
    }
}

/// Clear charge limit bypass
pub fn clear_charge_limit_bypass(battery_name: &str) {
    if let Some(ref mut manager) = *CHARGE_LIMIT_MANAGER.lock() {
        if let Some(settings) = manager.get_settings_mut(battery_name) {
            settings.clear_bypass();
        }
        manager.apply_to_hardware(battery_name);
    }
}
