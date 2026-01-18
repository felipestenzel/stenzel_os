//! Core Parking - Dynamic CPU power management
//!
//! Implements Windows-style core parking to reduce power consumption
//! by putting idle CPU cores into low-power states.
//!
//! Features:
//! - Policy-based core parking decisions
//! - C-state management for parked cores
//! - Load monitoring and threshold-based parking
//! - Integration with cpu_hotplug and cpufreq
//! - Thermal-aware parking (park more cores when hot)
//!
//! References:
//! - Windows Core Parking documentation
//! - Intel C-state management
//! - Linux cpuidle framework

#![allow(dead_code)]

extern crate alloc;

use alloc::vec::Vec;
use alloc::string::String;
use alloc::format;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use spin::{Mutex, RwLock};

use crate::util::{KResult, KError};

/// Maximum CPUs supported for core parking
const MAX_PARKING_CPUS: usize = 256;

/// Core parking policy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParkingPolicy {
    /// Never park cores - maximum performance
    Disabled,
    /// Conservative parking - park only when very idle
    Conservative,
    /// Balanced parking - reasonable balance
    Balanced,
    /// Aggressive parking - maximize power savings
    Aggressive,
    /// Custom thresholds
    Custom,
}

impl ParkingPolicy {
    /// Get threshold percentage for unparking (high load)
    pub fn unpark_threshold(&self) -> u8 {
        match self {
            Self::Disabled => 100,  // Never unpark (already unparked)
            Self::Conservative => 90,
            Self::Balanced => 70,
            Self::Aggressive => 50,
            Self::Custom => 70,
        }
    }

    /// Get threshold percentage for parking (low load)
    pub fn park_threshold(&self) -> u8 {
        match self {
            Self::Disabled => 0,    // Never park
            Self::Conservative => 20,
            Self::Balanced => 30,
            Self::Aggressive => 40,
            Self::Custom => 30,
        }
    }

    /// Get minimum cores that must stay unparked
    pub fn min_unparked_percent(&self) -> u8 {
        match self {
            Self::Disabled => 100,
            Self::Conservative => 50,
            Self::Balanced => 25,
            Self::Aggressive => 12, // At least 1 core on 8-core system
            Self::Custom => 25,
        }
    }

    /// Get maximum cores that can be parked
    pub fn max_parked_percent(&self) -> u8 {
        match self {
            Self::Disabled => 0,
            Self::Conservative => 50,
            Self::Balanced => 75,
            Self::Aggressive => 87, // Leave at least 1 core
            Self::Custom => 75,
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "disabled" | "off" => Some(Self::Disabled),
            "conservative" => Some(Self::Conservative),
            "balanced" => Some(Self::Balanced),
            "aggressive" => Some(Self::Aggressive),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Conservative => "conservative",
            Self::Balanced => "balanced",
            Self::Aggressive => "aggressive",
            Self::Custom => "custom",
        }
    }
}

/// C-state for parked cores
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CState {
    /// C0 - Active, running
    C0,
    /// C1 - Halt, clock gated
    C1,
    /// C1E - Enhanced halt, clock gated + voltage reduction
    C1E,
    /// C3 - Sleep, L1/L2 cache flushed
    C3,
    /// C6 - Deep sleep, core voltage reduced
    C6,
    /// C7 - Deeper sleep, L3 cache flushed
    C7,
    /// C8 - Deepest package C-state
    C8,
    /// C10 - Ultra-deep sleep (modern CPUs)
    C10,
}

impl CState {
    /// Get approximate exit latency in microseconds
    pub fn exit_latency_us(&self) -> u32 {
        match self {
            Self::C0 => 0,
            Self::C1 => 1,
            Self::C1E => 10,
            Self::C3 => 100,
            Self::C6 => 500,
            Self::C7 => 1000,
            Self::C8 => 2000,
            Self::C10 => 5000,
        }
    }

    /// Get approximate power savings percentage vs C0
    pub fn power_savings_percent(&self) -> u8 {
        match self {
            Self::C0 => 0,
            Self::C1 => 20,
            Self::C1E => 30,
            Self::C3 => 50,
            Self::C6 => 70,
            Self::C7 => 80,
            Self::C8 => 85,
            Self::C10 => 95,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::C0 => "C0",
            Self::C1 => "C1",
            Self::C1E => "C1E",
            Self::C3 => "C3",
            Self::C6 => "C6",
            Self::C7 => "C7",
            Self::C8 => "C8",
            Self::C10 => "C10",
        }
    }
}

/// Core parking state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreState {
    /// Core is active and running tasks
    Active,
    /// Core is being parked (transitioning)
    Parking,
    /// Core is parked (in low power state)
    Parked,
    /// Core is being unparked (transitioning)
    Unparking,
    /// Core is not available for parking (e.g., BSP)
    NotParkable,
}

/// Per-core parking information
#[derive(Debug, Clone)]
pub struct CoreInfo {
    /// CPU ID
    pub cpu_id: u32,
    /// Current parking state
    pub state: CoreState,
    /// Current C-state (when parked)
    pub cstate: CState,
    /// Load average (0-100)
    pub load: u8,
    /// Is this the BSP (boot CPU)?
    pub is_bsp: bool,
    /// Time parked (in ticks)
    pub parked_time: u64,
    /// Time active (in ticks)
    pub active_time: u64,
    /// Park/unpark count
    pub transitions: u32,
    /// Preferred parking order (lower = park first)
    pub park_order: u8,
}

impl Default for CoreInfo {
    fn default() -> Self {
        Self {
            cpu_id: 0,
            state: CoreState::Active,
            cstate: CState::C0,
            load: 0,
            is_bsp: false,
            parked_time: 0,
            active_time: 0,
            transitions: 0,
            park_order: 128,
        }
    }
}

/// Core parking configuration
#[derive(Debug, Clone)]
pub struct ParkingConfig {
    /// Parking policy
    pub policy: ParkingPolicy,
    /// Custom unpark threshold (if policy is Custom)
    pub custom_unpark_threshold: u8,
    /// Custom park threshold (if policy is Custom)
    pub custom_park_threshold: u8,
    /// Custom min unparked percent
    pub custom_min_unparked: u8,
    /// Target C-state for parked cores
    pub target_cstate: CState,
    /// Maximum C-state allowed (limit deep sleep)
    pub max_cstate: CState,
    /// Sample interval in milliseconds
    pub sample_interval_ms: u32,
    /// Hysteresis: consecutive samples needed to change state
    pub hysteresis_samples: u8,
    /// Enable thermal-aware parking
    pub thermal_aware: bool,
    /// Temperature threshold to start aggressive parking (Celsius)
    pub thermal_threshold: u8,
    /// Allow parking performance cores (on hybrid CPUs)
    pub allow_parking_pcores: bool,
    /// Prefer parking efficiency cores first (on hybrid CPUs)
    pub prefer_parking_ecores: bool,
}

impl Default for ParkingConfig {
    fn default() -> Self {
        Self {
            policy: ParkingPolicy::Balanced,
            custom_unpark_threshold: 70,
            custom_park_threshold: 30,
            custom_min_unparked: 25,
            target_cstate: CState::C6,
            max_cstate: CState::C7,
            sample_interval_ms: 100,
            hysteresis_samples: 3,
            thermal_aware: true,
            thermal_threshold: 80,
            allow_parking_pcores: true,
            prefer_parking_ecores: true,
        }
    }
}

/// Core parking statistics
#[derive(Debug, Clone, Default)]
pub struct ParkingStats {
    /// Total park operations
    pub total_parks: u64,
    /// Total unpark operations
    pub total_unparks: u64,
    /// Time spent with cores parked (in ticks)
    pub total_parked_time: u64,
    /// Average parked cores
    pub avg_parked_cores: f32,
    /// Estimated power saved (relative units)
    pub power_saved: u64,
    /// Park/unpark failures
    pub failures: u32,
    /// Thermal-triggered parks
    pub thermal_parks: u32,
}

/// Core parking manager
pub struct CoreParkingManager {
    /// Per-core information
    cores: RwLock<[CoreInfo; MAX_PARKING_CPUS]>,
    /// Configuration
    config: RwLock<ParkingConfig>,
    /// Statistics
    stats: Mutex<ParkingStats>,
    /// Number of CPUs
    num_cpus: AtomicU32,
    /// Number of currently parked cores
    parked_count: AtomicU32,
    /// Is parking enabled
    enabled: AtomicBool,
    /// Initialization complete
    initialized: AtomicBool,
    /// Load history for hysteresis
    load_history: Mutex<[u8; 16]>,
    /// Current load history index
    history_index: AtomicU32,
    /// Last sample timestamp
    last_sample: AtomicU64,
}

const DEFAULT_CORE_INFO: CoreInfo = CoreInfo {
    cpu_id: 0,
    state: CoreState::Active,
    cstate: CState::C0,
    load: 0,
    is_bsp: false,
    parked_time: 0,
    active_time: 0,
    transitions: 0,
    park_order: 128,
};

impl CoreParkingManager {
    pub const fn new() -> Self {
        Self {
            cores: RwLock::new([DEFAULT_CORE_INFO; MAX_PARKING_CPUS]),
            config: RwLock::new(ParkingConfig {
                policy: ParkingPolicy::Balanced,
                custom_unpark_threshold: 70,
                custom_park_threshold: 30,
                custom_min_unparked: 25,
                target_cstate: CState::C6,
                max_cstate: CState::C7,
                sample_interval_ms: 100,
                hysteresis_samples: 3,
                thermal_aware: true,
                thermal_threshold: 80,
                allow_parking_pcores: true,
                prefer_parking_ecores: true,
            }),
            stats: Mutex::new(ParkingStats {
                total_parks: 0,
                total_unparks: 0,
                total_parked_time: 0,
                avg_parked_cores: 0.0,
                power_saved: 0,
                failures: 0,
                thermal_parks: 0,
            }),
            num_cpus: AtomicU32::new(0),
            parked_count: AtomicU32::new(0),
            enabled: AtomicBool::new(false),
            initialized: AtomicBool::new(false),
            load_history: Mutex::new([0u8; 16]),
            history_index: AtomicU32::new(0),
            last_sample: AtomicU64::new(0),
        }
    }

    /// Initialize the core parking subsystem
    pub fn init(&self) {
        crate::kprintln!("core_parking: initializing...");

        // Get CPU count from SMP
        let cpu_count = super::smp::cpu_count();
        self.num_cpus.store(cpu_count as u32, Ordering::SeqCst);

        // Initialize per-core info
        let bsp_id = super::smp::bsp_apic_id() as u32;
        {
            let mut cores = self.cores.write();
            for i in 0..cpu_count as usize {
                let is_bsp = i as u32 == bsp_id;
                cores[i] = CoreInfo {
                    cpu_id: i as u32,
                    state: if is_bsp { CoreState::NotParkable } else { CoreState::Active },
                    cstate: CState::C0,
                    load: 0,
                    is_bsp,
                    parked_time: 0,
                    active_time: 0,
                    transitions: 0,
                    // Park higher-numbered cores first (they're usually E-cores on hybrid)
                    park_order: (255 - i) as u8,
                };
            }
        }

        // Check C-state support
        let max_cstate = self.detect_max_cstate();
        {
            let mut config = self.config.write();
            if config.max_cstate > max_cstate {
                config.max_cstate = max_cstate;
            }
            if config.target_cstate > max_cstate {
                config.target_cstate = max_cstate;
            }
        }

        self.initialized.store(true, Ordering::SeqCst);
        self.enabled.store(true, Ordering::SeqCst);

        crate::kprintln!("core_parking: {} CPUs, max C-state: {:?}",
            cpu_count, max_cstate);
    }

    /// Detect maximum supported C-state
    fn detect_max_cstate(&self) -> CState {
        // Check CPUID for MWAIT support and C-state enumeration
        let result = unsafe { core::arch::x86_64::__cpuid(5) };

        // EDX contains C-state support info
        // Bits 3:0 = number of C0 sub-states
        // Bits 7:4 = number of C1 sub-states, etc.
        let c1_support = (result.edx >> 4) & 0xF;
        let c2_support = (result.edx >> 8) & 0xF;
        let c3_support = (result.edx >> 12) & 0xF;
        let c4_support = (result.edx >> 16) & 0xF;

        // Also check CPUID leaf 6 for C-state capability
        let leaf6 = unsafe { core::arch::x86_64::__cpuid(6) };
        let _has_arat = (leaf6.eax & (1 << 2)) != 0; // Always Running APIC Timer

        // Determine max C-state based on support
        if c4_support > 0 {
            // Check for deeper states via MSR
            CState::C7
        } else if c3_support > 0 {
            CState::C6
        } else if c2_support > 0 {
            CState::C3
        } else if c1_support > 0 {
            CState::C1E
        } else {
            CState::C1
        }
    }

    /// Update load for a specific CPU
    pub fn update_load(&self, cpu_id: u32, load: u8) {
        if !self.initialized.load(Ordering::SeqCst) {
            return;
        }

        let id = cpu_id as usize;
        if id >= MAX_PARKING_CPUS {
            return;
        }

        let mut cores = self.cores.write();
        cores[id].load = load;
    }

    /// Process parking decisions based on current load
    pub fn process(&self) -> KResult<()> {
        if !self.enabled.load(Ordering::SeqCst) {
            return Ok(());
        }

        let config = self.config.read();
        if config.policy == ParkingPolicy::Disabled {
            return Ok(());
        }

        let num_cpus = self.num_cpus.load(Ordering::SeqCst) as usize;

        // Calculate average load across all active cores
        let mut total_load = 0u32;
        let mut active_count = 0u32;

        {
            let cores = self.cores.read();
            for i in 0..num_cpus {
                if cores[i].state == CoreState::Active {
                    total_load += cores[i].load as u32;
                    active_count += 1;
                }
            }
        }

        if active_count == 0 {
            return Ok(());
        }

        let avg_load = (total_load / active_count) as u8;

        // Update load history for hysteresis
        {
            let mut history = self.load_history.lock();
            let idx = (self.history_index.fetch_add(1, Ordering::SeqCst) % 16) as usize;
            history[idx] = avg_load;
        }

        // Calculate smoothed load
        let smoothed_load = {
            let history = self.load_history.lock();
            let samples = config.hysteresis_samples.min(16) as usize;
            let mut sum = 0u32;
            for i in 0..samples {
                sum += history[i] as u32;
            }
            (sum / samples as u32) as u8
        };

        // Get thresholds
        let unpark_threshold = if config.policy == ParkingPolicy::Custom {
            config.custom_unpark_threshold
        } else {
            config.policy.unpark_threshold()
        };

        let park_threshold = if config.policy == ParkingPolicy::Custom {
            config.custom_park_threshold
        } else {
            config.policy.park_threshold()
        };

        // Check thermal conditions
        let thermal_pressure = if config.thermal_aware {
            self.get_thermal_pressure()
        } else {
            0
        };

        // Adjust thresholds based on thermal pressure
        let adjusted_park_threshold = park_threshold.saturating_add(thermal_pressure);

        drop(config);

        // Decide whether to park or unpark
        if smoothed_load > unpark_threshold {
            // High load - unpark cores
            self.try_unpark_cores(1)?;
        } else if smoothed_load < adjusted_park_threshold {
            // Low load - park cores
            self.try_park_cores(1, thermal_pressure > 0)?;
        }

        // Update statistics
        self.update_stats();

        Ok(())
    }

    /// Get thermal pressure (0-50, higher = more aggressive parking)
    fn get_thermal_pressure(&self) -> u8 {
        // Try to get temperature from thermal driver
        // For now, return 0 (no thermal pressure)
        // TODO: integrate with crate::drivers::thermal
        0
    }

    /// Try to park one or more cores
    fn try_park_cores(&self, count: usize, thermal_triggered: bool) -> KResult<()> {
        let config = self.config.read();
        let num_cpus = self.num_cpus.load(Ordering::SeqCst) as usize;

        // Calculate minimum unparked cores
        let min_unparked = if config.policy == ParkingPolicy::Custom {
            config.custom_min_unparked
        } else {
            config.policy.min_unparked_percent()
        };
        let min_unparked_cores = ((num_cpus as u32 * min_unparked as u32) / 100).max(1) as usize;

        // Get current active count
        let mut active_count = 0usize;
        {
            let cores = self.cores.read();
            for i in 0..num_cpus {
                if cores[i].state == CoreState::Active {
                    active_count += 1;
                }
            }
        }

        // Check if we can park more
        if active_count <= min_unparked_cores {
            return Ok(()); // Already at minimum
        }

        let can_park = active_count - min_unparked_cores;
        let to_park = count.min(can_park);

        drop(config);

        // Find cores to park (lowest load, highest park_order)
        let mut parked = 0;
        for _ in 0..to_park {
            if let Some(cpu_id) = self.select_core_to_park() {
                if self.park_core(cpu_id).is_ok() {
                    parked += 1;
                    if thermal_triggered {
                        let mut stats = self.stats.lock();
                        stats.thermal_parks += 1;
                    }
                }
            }
        }

        if parked > 0 {
            crate::kprintln!("core_parking: parked {} cores", parked);
        }

        Ok(())
    }

    /// Try to unpark one or more cores
    fn try_unpark_cores(&self, count: usize) -> KResult<()> {
        let num_cpus = self.num_cpus.load(Ordering::SeqCst) as usize;
        let parked_count = self.parked_count.load(Ordering::SeqCst) as usize;

        if parked_count == 0 {
            return Ok(()); // No cores to unpark
        }

        let to_unpark = count.min(parked_count);

        // Find cores to unpark (highest priority first)
        let mut unparked = 0;
        for _ in 0..to_unpark {
            if let Some(cpu_id) = self.select_core_to_unpark() {
                if self.unpark_core(cpu_id).is_ok() {
                    unparked += 1;
                }
            }
        }

        if unparked > 0 {
            crate::kprintln!("core_parking: unparked {} cores", unparked);
        }

        Ok(())
    }

    /// Select the best core to park
    fn select_core_to_park(&self) -> Option<u32> {
        let cores = self.cores.read();
        let num_cpus = self.num_cpus.load(Ordering::SeqCst) as usize;

        // Find active core with lowest load and highest park_order
        let mut best: Option<(u32, u8, u8)> = None; // (cpu_id, load, park_order)

        for i in 0..num_cpus {
            let core = &cores[i];
            if core.state != CoreState::Active || core.is_bsp {
                continue;
            }

            match best {
                None => best = Some((core.cpu_id, core.load, core.park_order)),
                Some((_, best_load, best_order)) => {
                    // Prefer cores with lower load
                    // If load is similar, prefer higher park_order
                    if core.load < best_load ||
                       (core.load == best_load && core.park_order > best_order) {
                        best = Some((core.cpu_id, core.load, core.park_order));
                    }
                }
            }
        }

        best.map(|(id, _, _)| id)
    }

    /// Select the best core to unpark
    fn select_core_to_unpark(&self) -> Option<u32> {
        let cores = self.cores.read();
        let num_cpus = self.num_cpus.load(Ordering::SeqCst) as usize;

        // Find parked core with lowest park_order (highest priority)
        let mut best: Option<(u32, u8)> = None; // (cpu_id, park_order)

        for i in 0..num_cpus {
            let core = &cores[i];
            if core.state != CoreState::Parked {
                continue;
            }

            match best {
                None => best = Some((core.cpu_id, core.park_order)),
                Some((_, best_order)) => {
                    if core.park_order < best_order {
                        best = Some((core.cpu_id, core.park_order));
                    }
                }
            }
        }

        best.map(|(id, _)| id)
    }

    /// Park a specific core
    fn park_core(&self, cpu_id: u32) -> KResult<()> {
        let id = cpu_id as usize;
        if id >= MAX_PARKING_CPUS {
            return Err(KError::Invalid);
        }

        // Update state to parking
        {
            let mut cores = self.cores.write();
            if cores[id].state != CoreState::Active || cores[id].is_bsp {
                return Err(KError::Invalid);
            }
            cores[id].state = CoreState::Parking;
        }

        // Get target C-state
        let target_cstate = {
            let config = self.config.read();
            config.target_cstate
        };

        // Put core into C-state
        self.enter_cstate(cpu_id, target_cstate)?;

        // Update state to parked
        {
            let mut cores = self.cores.write();
            cores[id].state = CoreState::Parked;
            cores[id].cstate = target_cstate;
            cores[id].transitions += 1;
        }

        self.parked_count.fetch_add(1, Ordering::SeqCst);

        // Update stats
        {
            let mut stats = self.stats.lock();
            stats.total_parks += 1;
        }

        Ok(())
    }

    /// Unpark a specific core
    fn unpark_core(&self, cpu_id: u32) -> KResult<()> {
        let id = cpu_id as usize;
        if id >= MAX_PARKING_CPUS {
            return Err(KError::Invalid);
        }

        // Update state to unparking
        {
            let mut cores = self.cores.write();
            if cores[id].state != CoreState::Parked {
                return Err(KError::Invalid);
            }
            cores[id].state = CoreState::Unparking;
        }

        // Wake core from C-state
        self.exit_cstate(cpu_id)?;

        // Update state to active
        {
            let mut cores = self.cores.write();
            cores[id].state = CoreState::Active;
            cores[id].cstate = CState::C0;
            cores[id].transitions += 1;
        }

        self.parked_count.fetch_sub(1, Ordering::SeqCst);

        // Update stats
        {
            let mut stats = self.stats.lock();
            stats.total_unparks += 1;
        }

        Ok(())
    }

    /// Enter a C-state for a core
    fn enter_cstate(&self, cpu_id: u32, cstate: CState) -> KResult<()> {
        // For real implementation, we would:
        // 1. Send IPI to the target core
        // 2. Have it execute MWAIT with appropriate hints
        //
        // For now, use cpu_hotplug to take core offline for deeper states

        match cstate {
            CState::C0 => Ok(()),
            CState::C1 | CState::C1E => {
                // Light sleep - just use HLT
                // The core will wake on any interrupt
                Ok(())
            }
            CState::C3 | CState::C6 | CState::C7 | CState::C8 | CState::C10 => {
                // Deep sleep - use cpu_hotplug to take offline
                super::cpu_hotplug::cpu_down(cpu_id)
            }
        }
    }

    /// Exit C-state and wake core
    fn exit_cstate(&self, cpu_id: u32) -> KResult<()> {
        let cstate = {
            let cores = self.cores.read();
            cores[cpu_id as usize].cstate
        };

        match cstate {
            CState::C0 => Ok(()),
            CState::C1 | CState::C1E => {
                // Core will wake on any interrupt - send IPI
                super::ipi::wake_cpu(cpu_id as u8);
                Ok(())
            }
            CState::C3 | CState::C6 | CState::C7 | CState::C8 | CState::C10 => {
                // Bring core back online
                super::cpu_hotplug::cpu_up(cpu_id)
            }
        }
    }

    /// Update statistics
    fn update_stats(&self) {
        let parked = self.parked_count.load(Ordering::SeqCst);
        let num_cpus = self.num_cpus.load(Ordering::SeqCst);

        if num_cpus == 0 {
            return;
        }

        let mut stats = self.stats.lock();

        // Update average parked cores (exponential moving average)
        let parked_ratio = parked as f32 / num_cpus as f32;
        stats.avg_parked_cores = stats.avg_parked_cores * 0.9 + parked_ratio * 0.1;

        // Update total parked time
        if parked > 0 {
            stats.total_parked_time += 1;

            // Estimate power saved based on C-states
            let config = self.config.read();
            let savings = config.target_cstate.power_savings_percent() as u64;
            stats.power_saved += (parked as u64 * savings) / 100;
        }
    }

    /// Enable or disable core parking
    pub fn set_enabled(&self, enabled: bool) {
        let was_enabled = self.enabled.swap(enabled, Ordering::SeqCst);

        if was_enabled && !enabled {
            // Unpark all cores
            let num_cpus = self.num_cpus.load(Ordering::SeqCst) as usize;
            for i in 0..num_cpus {
                let state = {
                    let cores = self.cores.read();
                    cores[i].state
                };
                if state == CoreState::Parked {
                    let _ = self.unpark_core(i as u32);
                }
            }
        }

        crate::kprintln!("core_parking: {}", if enabled { "enabled" } else { "disabled" });
    }

    /// Set parking policy
    pub fn set_policy(&self, policy: ParkingPolicy) {
        let mut config = self.config.write();
        config.policy = policy;
        crate::kprintln!("core_parking: policy set to {}", policy.as_str());
    }

    /// Set target C-state
    pub fn set_target_cstate(&self, cstate: CState) {
        let mut config = self.config.write();
        if cstate <= config.max_cstate {
            config.target_cstate = cstate;
            crate::kprintln!("core_parking: target C-state set to {}", cstate.as_str());
        }
    }

    /// Get current configuration
    pub fn get_config(&self) -> ParkingConfig {
        self.config.read().clone()
    }

    /// Get statistics
    pub fn get_stats(&self) -> ParkingStats {
        self.stats.lock().clone()
    }

    /// Get core information
    pub fn get_core_info(&self, cpu_id: u32) -> Option<CoreInfo> {
        let id = cpu_id as usize;
        if id >= MAX_PARKING_CPUS {
            return None;
        }

        let cores = self.cores.read();
        if id < self.num_cpus.load(Ordering::SeqCst) as usize {
            Some(cores[id].clone())
        } else {
            None
        }
    }

    /// Get all core information
    pub fn get_all_cores(&self) -> Vec<CoreInfo> {
        let cores = self.cores.read();
        let num_cpus = self.num_cpus.load(Ordering::SeqCst) as usize;
        cores[..num_cpus].to_vec()
    }

    /// Get number of parked cores
    pub fn parked_count(&self) -> u32 {
        self.parked_count.load(Ordering::SeqCst)
    }

    /// Get number of active cores
    pub fn active_count(&self) -> u32 {
        let num_cpus = self.num_cpus.load(Ordering::SeqCst);
        let parked = self.parked_count.load(Ordering::SeqCst);
        num_cpus.saturating_sub(parked)
    }

    /// Check if core parking is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    /// Format status for display
    pub fn format_status(&self) -> String {
        let config = self.config.read();
        let stats = self.stats.lock();
        let num_cpus = self.num_cpus.load(Ordering::SeqCst);
        let parked = self.parked_count.load(Ordering::SeqCst);
        let active = num_cpus.saturating_sub(parked);

        format!(
            "Core Parking Status:\n\
             - Enabled: {}\n\
             - Policy: {}\n\
             - CPUs: {} total, {} active, {} parked\n\
             - Target C-state: {}\n\
             - Total parks: {}\n\
             - Total unparks: {}\n\
             - Power saved: {} units\n",
            self.enabled.load(Ordering::SeqCst),
            config.policy.as_str(),
            num_cpus, active, parked,
            config.target_cstate.as_str(),
            stats.total_parks,
            stats.total_unparks,
            stats.power_saved
        )
    }
}

fn to_lowercase_helper(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_uppercase() {
            result.push((c as u8 + 32) as char);
        } else {
            result.push(c);
        }
    }
    result
}

// Helper for lowercase since to_lowercase() requires std
impl ParkingPolicy {
    fn matches_str(&self, s: &str) -> bool {
        let lower = to_lowercase_helper(s);
        let lower = lower.trim();
        match self {
            Self::Disabled => lower == "disabled" || lower == "off",
            Self::Conservative => lower == "conservative",
            Self::Balanced => lower == "balanced",
            Self::Aggressive => lower == "aggressive",
            Self::Custom => lower == "custom",
        }
    }
}

// ============================================================================
// Global instance
// ============================================================================

static PARKING_MANAGER: CoreParkingManager = CoreParkingManager::new();

/// Initialize core parking
pub fn init() {
    PARKING_MANAGER.init();
}

/// Process parking decisions (call periodically)
pub fn process() -> KResult<()> {
    PARKING_MANAGER.process()
}

/// Update CPU load for parking decisions
pub fn update_load(cpu_id: u32, load: u8) {
    PARKING_MANAGER.update_load(cpu_id, load);
}

/// Enable or disable core parking
pub fn set_enabled(enabled: bool) {
    PARKING_MANAGER.set_enabled(enabled);
}

/// Check if core parking is enabled
pub fn is_enabled() -> bool {
    PARKING_MANAGER.is_enabled()
}

/// Set parking policy
pub fn set_policy(policy: ParkingPolicy) {
    PARKING_MANAGER.set_policy(policy);
}

/// Get current policy
pub fn get_policy() -> ParkingPolicy {
    PARKING_MANAGER.get_config().policy
}

/// Set target C-state for parked cores
pub fn set_target_cstate(cstate: CState) {
    PARKING_MANAGER.set_target_cstate(cstate);
}

/// Get number of parked cores
pub fn parked_count() -> u32 {
    PARKING_MANAGER.parked_count()
}

/// Get number of active cores
pub fn active_count() -> u32 {
    PARKING_MANAGER.active_count()
}

/// Get core information
pub fn get_core_info(cpu_id: u32) -> Option<CoreInfo> {
    PARKING_MANAGER.get_core_info(cpu_id)
}

/// Get all cores information
pub fn get_all_cores() -> Vec<CoreInfo> {
    PARKING_MANAGER.get_all_cores()
}

/// Get statistics
pub fn get_stats() -> ParkingStats {
    PARKING_MANAGER.get_stats()
}

/// Get configuration
pub fn get_config() -> ParkingConfig {
    PARKING_MANAGER.get_config()
}

/// Format status for display
pub fn format_status() -> String {
    PARKING_MANAGER.format_status()
}

/// Manually park a core (for testing/debugging)
pub fn park_core(cpu_id: u32) -> KResult<()> {
    PARKING_MANAGER.park_core(cpu_id)
}

/// Manually unpark a core
pub fn unpark_core(cpu_id: u32) -> KResult<()> {
    PARKING_MANAGER.unpark_core(cpu_id)
}

/// Unpark all cores
pub fn unpark_all() -> KResult<()> {
    let cores = PARKING_MANAGER.get_all_cores();
    for core in &cores {
        if core.state == CoreState::Parked {
            PARKING_MANAGER.unpark_core(core.cpu_id)?;
        }
    }
    Ok(())
}
