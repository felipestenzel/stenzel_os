//! Power Management Subsystem
//!
//! Provides comprehensive power management for laptops and desktops:
//! - Suspend to RAM (S3)
//! - Modern Standby (S0ix)
//! - Hibernation (S4)
//! - CPU frequency scaling (P-states)
//! - Power profiles
//! - Battery monitoring
//! - Thermal management integration

extern crate alloc;

pub mod suspend;
pub mod cpufreq;
pub mod profiles;
pub mod resume_speed;

use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use crate::sync::IrqSafeMutex;
use crate::util::{KResult, KError};

/// Power states (ACPI S-states)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PowerState {
    S0Working = 0,
    S0ix = 1,
    S1Standby = 2,
    S2 = 3,
    S3Suspend = 4,
    S4Hibernate = 5,
    S5SoftOff = 6,
}

/// Power profile
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PowerProfile {
    Performance = 0,
    Balanced = 1,
    PowerSaver = 2,
    BatterySaver = 3,
}

impl Default for PowerProfile {
    fn default() -> Self {
        PowerProfile::Balanced
    }
}

/// Battery status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatteryStatus {
    Charging,
    Discharging,
    Full,
    NotPresent,
    Unknown,
}

/// Battery information
#[derive(Debug, Clone)]
pub struct BatteryInfo {
    pub present: bool,
    pub status: BatteryStatus,
    pub percentage: u8,
    pub design_capacity: u32,
    pub full_capacity: u32,
    pub current_capacity: u32,
    pub voltage: u32,
    pub rate: i32,
    pub time_remaining: Option<u32>,
    pub health: u8,
    pub cycle_count: u32,
    pub temperature: u32,
    pub manufacturer: String,
    pub model: String,
    pub serial: String,
}

impl Default for BatteryInfo {
    fn default() -> Self {
        BatteryInfo {
            present: false,
            status: BatteryStatus::Unknown,
            percentage: 0,
            design_capacity: 0,
            full_capacity: 0,
            current_capacity: 0,
            voltage: 0,
            rate: 0,
            time_remaining: None,
            health: 0,
            cycle_count: 0,
            temperature: 0,
            manufacturer: String::new(),
            model: String::new(),
            serial: String::new(),
        }
    }
}

/// Power event type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerEvent {
    AcConnected,
    AcDisconnected,
    BatteryLow,
    BatteryCritical,
    LidClosed,
    LidOpened,
    PowerButton,
    SleepButton,
    ThermalEmergency,
    Suspending,
    Resuming,
}

pub type PowerEventCallback = fn(PowerEvent);

/// Action on critical battery
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CriticalBatteryAction {
    Nothing,
    Suspend,
    Hibernate,
    Shutdown,
}

/// Action on lid close
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LidCloseAction {
    Nothing,
    Lock,
    Suspend,
    Hibernate,
    Shutdown,
}

/// Action on power button
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerButtonAction {
    Nothing,
    Suspend,
    Hibernate,
    Shutdown,
    Interactive,
}

pub static POWER_MANAGER: IrqSafeMutex<PowerManager> = IrqSafeMutex::new(PowerManager::new());

pub struct PowerManager {
    state: PowerState,
    profile: PowerProfile,
    ac_connected: bool,
    lid_open: bool,
    batteries: Vec<BatteryInfo>,
    low_battery_threshold: u8,
    critical_battery_threshold: u8,
    critical_battery_action: CriticalBatteryAction,
    lid_close_action: LidCloseAction,
    power_button_action: PowerButtonAction,
    callbacks: Vec<PowerEventCallback>,
    s3_supported: bool,
    s0ix_supported: bool,
    hibernate_supported: bool,
    initialized: bool,
}

impl PowerManager {
    pub const fn new() -> Self {
        PowerManager {
            state: PowerState::S0Working,
            profile: PowerProfile::Balanced,
            ac_connected: true,
            lid_open: true,
            batteries: Vec::new(),
            low_battery_threshold: 20,
            critical_battery_threshold: 5,
            critical_battery_action: CriticalBatteryAction::Hibernate,
            lid_close_action: LidCloseAction::Suspend,
            power_button_action: PowerButtonAction::Interactive,
            callbacks: Vec::new(),
            s3_supported: false,
            s0ix_supported: false,
            hibernate_supported: false,
            initialized: false,
        }
    }

    pub fn init(&mut self) {
        self.s3_supported = true;
        self.s0ix_supported = false;
        self.ac_connected = crate::drivers::battery::on_ac_power();
        self.lid_open = true; // Default to open; will be updated by ACPI events
        self.detect_batteries();
        self.initialized = true;
        crate::kprintln!("power: manager initialized");
    }

    fn detect_batteries(&mut self) {
        let names = crate::drivers::battery::battery_names();
        for name in names {
            if let Some(info) = crate::drivers::battery::battery_info(&name) {
                if let Some(status) = crate::drivers::battery::battery_status(&name) {
                    let battery = BatteryInfo {
                        present: true,
                        status: match status.state {
                            crate::drivers::battery::BatteryState::Charging => BatteryStatus::Charging,
                            crate::drivers::battery::BatteryState::Discharging => BatteryStatus::Discharging,
                            crate::drivers::battery::BatteryState::Full => BatteryStatus::Full,
                            _ => BatteryStatus::Unknown,
                        },
                        percentage: status.percentage,
                        design_capacity: info.design_capacity,
                        full_capacity: info.full_charge_capacity,
                        current_capacity: status.remaining_capacity,
                        voltage: status.present_voltage,
                        rate: status.present_rate as i32,
                        time_remaining: status.time_to_empty.or(status.time_to_full),
                        health: 100,
                        cycle_count: info.cycle_count.unwrap_or(0),
                        ..Default::default()
                    };
                    self.batteries.push(battery);
                }
            }
        }
    }

    pub fn state(&self) -> PowerState { self.state }
    pub fn profile(&self) -> PowerProfile { self.profile }

    pub fn set_profile(&mut self, profile: PowerProfile) {
        self.profile = profile;
        cpufreq::apply_profile(profile);
        crate::kprintln!("power: profile set to {:?}", profile);
    }

    pub fn is_ac_connected(&self) -> bool { self.ac_connected }
    pub fn is_lid_open(&self) -> bool { self.lid_open }
    pub fn battery_info(&self, idx: usize) -> Option<&BatteryInfo> { self.batteries.get(idx) }
    pub fn battery_count(&self) -> usize { self.batteries.len() }

    pub fn combined_battery_percentage(&self) -> Option<u8> {
        if self.batteries.is_empty() { return None; }
        let total_current: u32 = self.batteries.iter().filter(|b| b.present).map(|b| b.current_capacity).sum();
        let total_full: u32 = self.batteries.iter().filter(|b| b.present).map(|b| b.full_capacity).sum();
        if total_full == 0 { return None; }
        Some(((total_current * 100) / total_full) as u8)
    }

    pub fn time_remaining(&self) -> Option<u32> {
        if self.ac_connected {
            let total_remaining: u32 = self.batteries.iter()
                .filter(|b| b.present && b.status == BatteryStatus::Charging && b.rate > 0)
                .map(|b| (b.full_capacity.saturating_sub(b.current_capacity) * 60) / b.rate as u32)
                .sum();
            if total_remaining > 0 { Some(total_remaining) } else { None }
        } else {
            let total_rate: i32 = self.batteries.iter()
                .filter(|b| b.present && b.status == BatteryStatus::Discharging)
                .map(|b| b.rate.abs()).sum();
            if total_rate <= 0 { return None; }
            let total_current: u32 = self.batteries.iter().filter(|b| b.present).map(|b| b.current_capacity).sum();
            Some((total_current * 60) / total_rate as u32)
        }
    }

    pub fn register_callback(&mut self, cb: PowerEventCallback) { self.callbacks.push(cb); }

    fn fire_event(&self, event: PowerEvent) {
        for cb in &self.callbacks { cb(event); }
    }

    pub fn handle_ac_change(&mut self, connected: bool) {
        self.ac_connected = connected;
        self.fire_event(if connected { PowerEvent::AcConnected } else { PowerEvent::AcDisconnected });
    }

    pub fn handle_lid_switch(&mut self, open: bool) {
        self.lid_open = open;
        self.fire_event(if open { PowerEvent::LidOpened } else { PowerEvent::LidClosed });
        if !open {
            match self.lid_close_action {
                LidCloseAction::Suspend => { let _ = self.suspend(); }
                LidCloseAction::Hibernate => { let _ = self.hibernate(); }
                LidCloseAction::Shutdown => { let _ = self.shutdown(); }
                _ => {}
            }
        }
    }

    pub fn handle_power_button(&mut self) {
        self.fire_event(PowerEvent::PowerButton);
        match self.power_button_action {
            PowerButtonAction::Suspend => { let _ = self.suspend(); }
            PowerButtonAction::Hibernate => { let _ = self.hibernate(); }
            PowerButtonAction::Shutdown => { let _ = self.shutdown(); }
            _ => {}
        }
    }

    pub fn update_batteries(&mut self) {
        let names = crate::drivers::battery::battery_names();
        for (i, name) in names.iter().enumerate() {
            if let Some(status) = crate::drivers::battery::battery_status(name) {
                if let Some(battery) = self.batteries.get_mut(i) {
                    battery.percentage = status.percentage;
                    battery.current_capacity = status.remaining_capacity;
                    battery.rate = status.present_rate as i32;
                    battery.time_remaining = status.time_to_empty.or(status.time_to_full);
                }
            }
        }
        self.ac_connected = crate::drivers::battery::on_ac_power();
        if !self.ac_connected {
            if let Some(pct) = self.combined_battery_percentage() {
                if pct <= self.critical_battery_threshold {
                    self.fire_event(PowerEvent::BatteryCritical);
                } else if pct <= self.low_battery_threshold {
                    self.fire_event(PowerEvent::BatteryLow);
                }
            }
        }
    }

    pub fn suspend(&mut self) -> KResult<()> {
        if !self.s3_supported { return Err(KError::NotSupported); }
        crate::kprintln!("power: suspending to RAM (S3)...");
        self.fire_event(PowerEvent::Suspending);
        self.state = PowerState::S3Suspend;
        suspend::suspend_to_ram()?;
        self.state = PowerState::S0Working;
        self.fire_event(PowerEvent::Resuming);
        Ok(())
    }

    pub fn enter_s0ix(&mut self) -> KResult<()> {
        if !self.s0ix_supported { return Err(KError::NotSupported); }
        self.state = PowerState::S0ix;
        suspend::enter_s0ix()?;
        self.state = PowerState::S0Working;
        Ok(())
    }

    pub fn hibernate(&mut self) -> KResult<()> {
        if !self.hibernate_supported { return Err(KError::NotSupported); }
        self.fire_event(PowerEvent::Suspending);
        self.state = PowerState::S4Hibernate;
        suspend::hibernate()?;
        self.state = PowerState::S0Working;
        self.fire_event(PowerEvent::Resuming);
        Ok(())
    }

    pub fn shutdown(&mut self) -> KResult<()> {
        self.state = PowerState::S5SoftOff;
        suspend::shutdown()
    }

    pub fn reboot(&mut self) -> KResult<()> {
        suspend::reboot()
    }
}

pub fn init() {
    cpufreq::init();
    profiles::init();
    POWER_MANAGER.lock().init();
    crate::kprintln!("power: subsystem initialized");
}

pub fn battery_percentage() -> Option<u8> { POWER_MANAGER.lock().combined_battery_percentage() }
pub fn time_remaining() -> Option<u32> { POWER_MANAGER.lock().time_remaining() }
pub fn on_ac_power() -> bool { POWER_MANAGER.lock().is_ac_connected() }
pub fn current_profile() -> PowerProfile { POWER_MANAGER.lock().profile() }
pub fn set_profile(profile: PowerProfile) { POWER_MANAGER.lock().set_profile(profile); }
pub fn suspend() -> KResult<()> { POWER_MANAGER.lock().suspend() }
pub fn hibernate() -> KResult<()> { POWER_MANAGER.lock().hibernate() }
pub fn shutdown() -> KResult<()> { POWER_MANAGER.lock().shutdown() }
pub fn reboot() -> KResult<()> { POWER_MANAGER.lock().reboot() }
