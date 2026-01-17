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
