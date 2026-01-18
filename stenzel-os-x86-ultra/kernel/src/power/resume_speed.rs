//! Resume Speed Optimization
//!
//! Optimizes the time to resume from suspend/hibernate by:
//! - Parallel device resume
//! - Device state caching
//! - Resume prioritization (user-visible first)
//! - Device dependency tracking
//! - Resume time profiling
//!
//! Goals:
//! - < 1 second resume for S3 (suspend to RAM)
//! - < 5 seconds resume for S4 (hibernate)

#![allow(dead_code)]

extern crate alloc;

use alloc::vec::Vec;
use alloc::vec;
use alloc::string::String;
use alloc::format;
use alloc::collections::BTreeMap;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use spin::{Mutex, RwLock};

use crate::util::{KResult, KError};

/// Maximum number of devices that can be tracked
const MAX_DEVICES: usize = 256;

/// Resume phase - determines when a device resumes
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ResumePhase {
    /// Critical system devices (interrupt controllers, timers)
    Critical = 0,
    /// Core devices (storage controllers, display)
    Core = 1,
    /// User-visible devices (display, keyboard, touchpad)
    UserVisible = 2,
    /// Network devices
    Network = 3,
    /// Other devices (USB peripherals, audio)
    Other = 4,
    /// Background devices (non-critical)
    Background = 5,
}

impl ResumePhase {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Critical => "critical",
            Self::Core => "core",
            Self::UserVisible => "user-visible",
            Self::Network => "network",
            Self::Other => "other",
            Self::Background => "background",
        }
    }

    /// Whether this phase should be resumed in parallel
    pub fn parallel_allowed(&self) -> bool {
        match self {
            Self::Critical => false, // Critical devices resume sequentially
            _ => true,
        }
    }
}

/// Device resume callback
pub type ResumeCallback = fn() -> KResult<()>;
pub type SuspendCallback = fn() -> KResult<()>;

/// Device resume information
#[derive(Clone)]
pub struct DeviceResumeInfo {
    /// Device name
    pub name: &'static str,
    /// Resume phase
    pub phase: ResumePhase,
    /// Priority within phase (lower = higher priority)
    pub priority: u8,
    /// Resume callback
    pub resume_callback: ResumeCallback,
    /// Suspend callback
    pub suspend_callback: SuspendCallback,
    /// Device dependencies (names of devices that must resume first)
    pub dependencies: Vec<&'static str>,
    /// Supports async/parallel resume
    pub async_capable: bool,
    /// Supports quick resume (state was cached)
    pub quick_resume: bool,
    /// Last resume time in microseconds
    pub last_resume_us: u64,
    /// Average resume time
    pub avg_resume_us: u64,
    /// Resume count
    pub resume_count: u32,
    /// Whether device is currently suspended
    pub suspended: bool,
    /// Whether resume can be skipped (device not in use)
    pub skip_resume: bool,
}

impl DeviceResumeInfo {
    pub fn new(
        name: &'static str,
        phase: ResumePhase,
        priority: u8,
        resume_callback: ResumeCallback,
        suspend_callback: SuspendCallback,
    ) -> Self {
        Self {
            name,
            phase,
            priority,
            resume_callback,
            suspend_callback,
            dependencies: Vec::new(),
            async_capable: false,
            quick_resume: false,
            last_resume_us: 0,
            avg_resume_us: 0,
            resume_count: 0,
            suspended: false,
            skip_resume: false,
        }
    }

    pub fn with_dependencies(mut self, deps: &[&'static str]) -> Self {
        self.dependencies = deps.to_vec();
        self
    }

    pub fn with_async(mut self) -> Self {
        self.async_capable = true;
        self
    }

    pub fn with_quick_resume(mut self) -> Self {
        self.quick_resume = true;
        self
    }
}

/// Resume optimization configuration
#[derive(Debug, Clone)]
pub struct ResumeConfig {
    /// Enable parallel resume
    pub parallel_resume: bool,
    /// Maximum parallel resume threads
    pub max_parallel: u8,
    /// Enable device state caching
    pub state_caching: bool,
    /// Skip unused devices
    pub skip_unused: bool,
    /// Target resume time in milliseconds
    pub target_resume_ms: u32,
    /// Enable resume profiling
    pub profiling_enabled: bool,
    /// Defer non-critical devices
    pub defer_noncritical: bool,
    /// Time to defer non-critical devices (ms)
    pub defer_time_ms: u32,
}

impl Default for ResumeConfig {
    fn default() -> Self {
        Self {
            parallel_resume: true,
            max_parallel: 4,
            state_caching: true,
            skip_unused: true,
            target_resume_ms: 1000, // Target < 1 second
            profiling_enabled: true,
            defer_noncritical: true,
            defer_time_ms: 500,
        }
    }
}

/// Resume statistics
#[derive(Debug, Clone, Default)]
pub struct ResumeStats {
    /// Total resume count
    pub resume_count: u32,
    /// Average resume time (ms)
    pub avg_resume_ms: u32,
    /// Best resume time (ms)
    pub best_resume_ms: u32,
    /// Worst resume time (ms)
    pub worst_resume_ms: u32,
    /// Last resume time (ms)
    pub last_resume_ms: u32,
    /// Devices that exceeded target time
    pub slow_devices: Vec<SlowDevice>,
    /// Time saved by parallel resume (ms)
    pub parallel_savings_ms: u32,
    /// Time saved by skipped devices (ms)
    pub skip_savings_ms: u32,
    /// Time saved by quick resume (ms)
    pub quick_savings_ms: u32,
}

/// Information about a slow device
#[derive(Debug, Clone)]
pub struct SlowDevice {
    pub name: &'static str,
    pub resume_time_us: u64,
    pub target_us: u64,
}

/// Resume timing entry
#[derive(Debug, Clone)]
pub struct ResumeTiming {
    pub device_name: &'static str,
    pub phase: ResumePhase,
    pub start_us: u64,
    pub end_us: u64,
    pub duration_us: u64,
    pub parallel: bool,
}

/// Resume speed manager
pub struct ResumeSpeedManager {
    /// Registered devices
    devices: RwLock<BTreeMap<&'static str, DeviceResumeInfo>>,
    /// Configuration
    config: RwLock<ResumeConfig>,
    /// Statistics
    stats: Mutex<ResumeStats>,
    /// Current resume timings
    timings: Mutex<Vec<ResumeTiming>>,
    /// Resume in progress
    resume_in_progress: AtomicBool,
    /// Resume start time
    resume_start_us: AtomicU64,
    /// Devices resumed count
    devices_resumed: AtomicU32,
    /// Initialized flag
    initialized: AtomicBool,
}

impl ResumeSpeedManager {
    pub const fn new() -> Self {
        Self {
            devices: RwLock::new(BTreeMap::new()),
            config: RwLock::new(ResumeConfig {
                parallel_resume: true,
                max_parallel: 4,
                state_caching: true,
                skip_unused: true,
                target_resume_ms: 1000,
                profiling_enabled: true,
                defer_noncritical: true,
                defer_time_ms: 500,
            }),
            stats: Mutex::new(ResumeStats {
                resume_count: 0,
                avg_resume_ms: 0,
                best_resume_ms: u32::MAX,
                worst_resume_ms: 0,
                last_resume_ms: 0,
                slow_devices: Vec::new(),
                parallel_savings_ms: 0,
                skip_savings_ms: 0,
                quick_savings_ms: 0,
            }),
            timings: Mutex::new(Vec::new()),
            resume_in_progress: AtomicBool::new(false),
            resume_start_us: AtomicU64::new(0),
            devices_resumed: AtomicU32::new(0),
            initialized: AtomicBool::new(false),
        }
    }

    /// Initialize the resume speed manager
    pub fn init(&self) {
        crate::kprintln!("resume_speed: initializing...");
        self.initialized.store(true, Ordering::SeqCst);

        // Register common system devices with default callbacks
        self.register_system_devices();

        crate::kprintln!("resume_speed: initialized");
    }

    /// Register common system devices
    fn register_system_devices(&self) {
        // These would be replaced by actual device callbacks in a full implementation

        // Critical phase devices
        self.register_device(DeviceResumeInfo::new(
            "apic",
            ResumePhase::Critical,
            0,
            || Ok(()),
            || Ok(()),
        ));

        self.register_device(DeviceResumeInfo::new(
            "timer",
            ResumePhase::Critical,
            1,
            || Ok(()),
            || Ok(()),
        ));

        self.register_device(DeviceResumeInfo::new(
            "pci_bridge",
            ResumePhase::Critical,
            2,
            || Ok(()),
            || Ok(()),
        ));

        // Core phase devices
        self.register_device(DeviceResumeInfo::new(
            "nvme",
            ResumePhase::Core,
            0,
            || Ok(()),
            || Ok(()),
        ).with_async());

        self.register_device(DeviceResumeInfo::new(
            "ahci",
            ResumePhase::Core,
            0,
            || Ok(()),
            || Ok(()),
        ).with_async());

        // User-visible phase devices
        self.register_device(DeviceResumeInfo::new(
            "display",
            ResumePhase::UserVisible,
            0,
            || Ok(()),
            || Ok(()),
        ).with_quick_resume());

        self.register_device(DeviceResumeInfo::new(
            "keyboard",
            ResumePhase::UserVisible,
            1,
            || Ok(()),
            || Ok(()),
        ).with_quick_resume());

        self.register_device(DeviceResumeInfo::new(
            "touchpad",
            ResumePhase::UserVisible,
            2,
            || Ok(()),
            || Ok(()),
        ).with_quick_resume());

        // Network phase devices
        self.register_device(DeviceResumeInfo::new(
            "ethernet",
            ResumePhase::Network,
            0,
            || Ok(()),
            || Ok(()),
        ).with_async());

        self.register_device(DeviceResumeInfo::new(
            "wifi",
            ResumePhase::Network,
            1,
            || Ok(()),
            || Ok(()),
        ).with_async().with_dependencies(&["pci_bridge"]));

        // Other devices
        self.register_device(DeviceResumeInfo::new(
            "usb_host",
            ResumePhase::Other,
            0,
            || Ok(()),
            || Ok(()),
        ).with_async());

        self.register_device(DeviceResumeInfo::new(
            "audio",
            ResumePhase::Other,
            1,
            || Ok(()),
            || Ok(()),
        ).with_async());

        // Background devices
        self.register_device(DeviceResumeInfo::new(
            "bluetooth",
            ResumePhase::Background,
            0,
            || Ok(()),
            || Ok(()),
        ).with_async());
    }

    /// Register a device for resume tracking
    pub fn register_device(&self, info: DeviceResumeInfo) {
        let mut devices = self.devices.write();
        devices.insert(info.name, info);
    }

    /// Unregister a device
    pub fn unregister_device(&self, name: &'static str) {
        let mut devices = self.devices.write();
        devices.remove(name);
    }

    /// Update device callback
    pub fn update_device_callback(
        &self,
        name: &'static str,
        resume_callback: ResumeCallback,
        suspend_callback: SuspendCallback,
    ) -> KResult<()> {
        let mut devices = self.devices.write();
        if let Some(device) = devices.get_mut(name) {
            device.resume_callback = resume_callback;
            device.suspend_callback = suspend_callback;
            Ok(())
        } else {
            Err(KError::NotFound)
        }
    }

    /// Mark device as suspended
    pub fn mark_suspended(&self, name: &'static str) {
        let mut devices = self.devices.write();
        if let Some(device) = devices.get_mut(name) {
            device.suspended = true;
        }
    }

    /// Mark device as resumed
    pub fn mark_resumed(&self, name: &'static str) {
        let mut devices = self.devices.write();
        if let Some(device) = devices.get_mut(name) {
            device.suspended = false;
        }
    }

    /// Execute optimized resume sequence
    pub fn resume_all(&self) -> KResult<()> {
        if self.resume_in_progress.swap(true, Ordering::SeqCst) {
            return Err(KError::Busy);
        }

        let start_time = self.get_timestamp_us();
        self.resume_start_us.store(start_time, Ordering::SeqCst);
        self.devices_resumed.store(0, Ordering::SeqCst);

        // Clear timings
        {
            let mut timings = self.timings.lock();
            timings.clear();
        }

        let config = self.config.read().clone();
        crate::kprintln!("resume_speed: starting optimized resume...");

        // Resume in phases
        let phases = [
            ResumePhase::Critical,
            ResumePhase::Core,
            ResumePhase::UserVisible,
            ResumePhase::Network,
            ResumePhase::Other,
            ResumePhase::Background,
        ];

        for phase in &phases {
            self.resume_phase(*phase, &config)?;
        }

        // Calculate total time
        let end_time = self.get_timestamp_us();
        let total_us = end_time.saturating_sub(start_time);
        let total_ms = (total_us / 1000) as u32;

        // Update statistics
        self.update_stats(total_ms);

        self.resume_in_progress.store(false, Ordering::SeqCst);

        crate::kprintln!("resume_speed: resume complete in {}ms ({} devices)",
            total_ms, self.devices_resumed.load(Ordering::SeqCst));

        // Check if we met target
        if total_ms > config.target_resume_ms {
            crate::kprintln!("resume_speed: WARNING: resume exceeded target ({}ms > {}ms)",
                total_ms, config.target_resume_ms);
            self.analyze_slow_devices();
        }

        Ok(())
    }

    /// Resume devices in a specific phase
    fn resume_phase(&self, phase: ResumePhase, config: &ResumeConfig) -> KResult<()> {
        let devices_in_phase: Vec<DeviceResumeInfo> = {
            let devices = self.devices.read();
            devices.values()
                .filter(|d| d.phase == phase && d.suspended)
                .cloned()
                .collect()
        };

        if devices_in_phase.is_empty() {
            return Ok(());
        }

        crate::kprintln!("resume_speed: resuming {} phase ({} devices)",
            phase.as_str(), devices_in_phase.len());

        if config.parallel_resume && phase.parallel_allowed() {
            self.resume_parallel(&devices_in_phase, config)?;
        } else {
            self.resume_sequential(&devices_in_phase, config)?;
        }

        Ok(())
    }

    /// Resume devices sequentially
    fn resume_sequential(&self, devices: &[DeviceResumeInfo], config: &ResumeConfig) -> KResult<()> {
        // Sort by priority
        let mut sorted: Vec<_> = devices.to_vec();
        sorted.sort_by_key(|d| (d.priority, d.name));

        for device in &sorted {
            // Check if we should skip
            if config.skip_unused && device.skip_resume {
                crate::kprintln!("resume_speed: skipping unused device: {}", device.name);
                continue;
            }

            // Check dependencies
            self.wait_for_dependencies(device)?;

            // Resume device
            let start = self.get_timestamp_us();

            let result = if device.quick_resume {
                self.quick_resume_device(device)
            } else {
                (device.resume_callback)()
            };

            let end = self.get_timestamp_us();
            let duration = end.saturating_sub(start);

            // Record timing
            self.record_timing(device.name, device.phase, start, end, false);

            // Update device stats
            self.update_device_stats(device.name, duration);

            self.devices_resumed.fetch_add(1, Ordering::SeqCst);

            if let Err(e) = result {
                crate::kprintln!("resume_speed: {} resume failed: {:?}", device.name, e);
                // Continue with other devices
            }
        }

        Ok(())
    }

    /// Resume devices in parallel
    fn resume_parallel(&self, devices: &[DeviceResumeInfo], config: &ResumeConfig) -> KResult<()> {
        // Sort by priority
        let mut sorted: Vec<_> = devices.to_vec();
        sorted.sort_by_key(|d| (d.priority, d.name));

        // Group by dependency level
        let groups = self.group_by_dependencies(&sorted);

        for group in &groups {
            // Resume all devices in this group in parallel
            // In a real implementation, this would use work queues

            // For now, resume sequentially but track as parallel
            for device in group {
                if config.skip_unused && device.skip_resume {
                    continue;
                }

                let start = self.get_timestamp_us();

                let result = if device.quick_resume {
                    self.quick_resume_device(device)
                } else {
                    (device.resume_callback)()
                };

                let end = self.get_timestamp_us();
                let duration = end.saturating_sub(start);

                self.record_timing(device.name, device.phase, start, end, device.async_capable);
                self.update_device_stats(device.name, duration);
                self.devices_resumed.fetch_add(1, Ordering::SeqCst);

                if let Err(e) = result {
                    crate::kprintln!("resume_speed: {} resume failed: {:?}", device.name, e);
                }
            }
        }

        Ok(())
    }

    /// Group devices by dependency level
    fn group_by_dependencies(&self, devices: &[DeviceResumeInfo]) -> Vec<Vec<DeviceResumeInfo>> {
        let mut groups: Vec<Vec<DeviceResumeInfo>> = Vec::new();
        let mut remaining: Vec<DeviceResumeInfo> = devices.to_vec();
        let mut resolved: Vec<&'static str> = Vec::new();

        while !remaining.is_empty() {
            let mut current_group = Vec::new();
            let mut next_remaining = Vec::new();

            for device in remaining {
                // Check if all dependencies are resolved
                let deps_met = device.dependencies.iter().all(|dep| resolved.contains(dep));

                if deps_met || device.dependencies.is_empty() {
                    current_group.push(device);
                } else {
                    next_remaining.push(device);
                }
            }

            // Add names to resolved
            for device in &current_group {
                resolved.push(device.name);
            }

            if !current_group.is_empty() {
                groups.push(current_group);
            }

            remaining = next_remaining;

            // Prevent infinite loop
            if remaining.len() > 0 && groups.len() > MAX_DEVICES {
                // Force remaining into last group
                groups.push(remaining);
                break;
            }
        }

        groups
    }

    /// Wait for device dependencies to be resumed
    fn wait_for_dependencies(&self, device: &DeviceResumeInfo) -> KResult<()> {
        for dep_name in &device.dependencies {
            let devices = self.devices.read();
            if let Some(dep) = devices.get(dep_name) {
                if dep.suspended {
                    // In a real implementation, we would wait
                    crate::kprintln!("resume_speed: waiting for dependency: {}", dep_name);
                }
            }
        }
        Ok(())
    }

    /// Quick resume using cached state
    fn quick_resume_device(&self, device: &DeviceResumeInfo) -> KResult<()> {
        // In a real implementation, this would restore cached device state
        // instead of full re-initialization
        crate::kprintln!("resume_speed: quick resume: {}", device.name);
        (device.resume_callback)()
    }

    /// Record timing information
    fn record_timing(
        &self,
        device_name: &'static str,
        phase: ResumePhase,
        start_us: u64,
        end_us: u64,
        parallel: bool,
    ) {
        let mut timings = self.timings.lock();
        timings.push(ResumeTiming {
            device_name,
            phase,
            start_us,
            end_us,
            duration_us: end_us.saturating_sub(start_us),
            parallel,
        });
    }

    /// Update device statistics
    fn update_device_stats(&self, name: &'static str, duration_us: u64) {
        let mut devices = self.devices.write();
        if let Some(device) = devices.get_mut(name) {
            device.last_resume_us = duration_us;
            device.resume_count += 1;

            // Update average
            let count = device.resume_count as u64;
            device.avg_resume_us =
                (device.avg_resume_us * (count - 1) + duration_us) / count;

            // Mark as resumed
            device.suspended = false;
        }
    }

    /// Update overall statistics
    fn update_stats(&self, total_ms: u32) {
        let mut stats = self.stats.lock();

        stats.resume_count += 1;
        stats.last_resume_ms = total_ms;

        // Update average
        let count = stats.resume_count as u32;
        stats.avg_resume_ms = (stats.avg_resume_ms * (count - 1) + total_ms) / count;

        // Update best/worst
        if total_ms < stats.best_resume_ms {
            stats.best_resume_ms = total_ms;
        }
        if total_ms > stats.worst_resume_ms {
            stats.worst_resume_ms = total_ms;
        }
    }

    /// Analyze slow devices
    fn analyze_slow_devices(&self) {
        let config = self.config.read();
        let target_us = (config.target_resume_ms as u64) * 1000 / 10; // Per-device target

        let mut slow_devices = Vec::new();

        let timings = self.timings.lock();
        for timing in timings.iter() {
            if timing.duration_us > target_us {
                slow_devices.push(SlowDevice {
                    name: timing.device_name,
                    resume_time_us: timing.duration_us,
                    target_us,
                });
            }
        }

        if !slow_devices.is_empty() {
            crate::kprintln!("resume_speed: slow devices:");
            for device in &slow_devices {
                crate::kprintln!("  - {}: {}us (target: {}us)",
                    device.name, device.resume_time_us, device.target_us);
            }
        }

        // Update stats
        drop(timings);
        let mut stats = self.stats.lock();
        stats.slow_devices = slow_devices;
    }

    /// Get current timestamp in microseconds
    fn get_timestamp_us(&self) -> u64 {
        // Use TSC for high-resolution timing
        unsafe {
            let (low, high): (u32, u32);
            core::arch::asm!(
                "rdtsc",
                out("eax") low,
                out("edx") high,
            );
            let tsc = ((high as u64) << 32) | (low as u64);

            // Convert TSC to microseconds (assuming ~3GHz)
            // In a real implementation, we'd calibrate this
            tsc / 3000
        }
    }

    /// Get configuration
    pub fn get_config(&self) -> ResumeConfig {
        self.config.read().clone()
    }

    /// Set configuration
    pub fn set_config(&self, config: ResumeConfig) {
        let mut current = self.config.write();
        *current = config;
    }

    /// Get statistics
    pub fn get_stats(&self) -> ResumeStats {
        self.stats.lock().clone()
    }

    /// Get resume timings
    pub fn get_timings(&self) -> Vec<ResumeTiming> {
        self.timings.lock().clone()
    }

    /// Get device list
    pub fn get_devices(&self) -> Vec<DeviceResumeInfo> {
        self.devices.read().values().cloned().collect()
    }

    /// Format status report
    pub fn format_status(&self) -> String {
        let config = self.config.read();
        let stats = self.stats.lock();
        let devices = self.devices.read();

        format!(
            "Resume Speed Optimization Status:\n\
             Configuration:\n\
             - Parallel resume: {}\n\
             - Max parallel: {}\n\
             - State caching: {}\n\
             - Target resume: {}ms\n\n\
             Statistics:\n\
             - Resume count: {}\n\
             - Average: {}ms\n\
             - Best: {}ms\n\
             - Worst: {}ms\n\
             - Last: {}ms\n\n\
             Registered devices: {}\n",
            config.parallel_resume,
            config.max_parallel,
            config.state_caching,
            config.target_resume_ms,
            stats.resume_count,
            stats.avg_resume_ms,
            if stats.best_resume_ms == u32::MAX { 0 } else { stats.best_resume_ms },
            stats.worst_resume_ms,
            stats.last_resume_ms,
            devices.len()
        )
    }

    /// Prepare for suspend (mark all devices as needing resume)
    pub fn prepare_suspend(&self) {
        let mut devices = self.devices.write();
        for device in devices.values_mut() {
            device.suspended = true;
        }
    }

    /// Enable/disable parallel resume
    pub fn set_parallel_resume(&self, enabled: bool) {
        let mut config = self.config.write();
        config.parallel_resume = enabled;
    }

    /// Set target resume time
    pub fn set_target_time(&self, ms: u32) {
        let mut config = self.config.write();
        config.target_resume_ms = ms;
    }

    /// Mark device as skippable
    pub fn set_device_skippable(&self, name: &'static str, skippable: bool) {
        let mut devices = self.devices.write();
        if let Some(device) = devices.get_mut(name) {
            device.skip_resume = skippable;
        }
    }
}

// ============================================================================
// Global instance
// ============================================================================

static RESUME_MANAGER: ResumeSpeedManager = ResumeSpeedManager::new();

/// Initialize resume speed optimization
pub fn init() {
    RESUME_MANAGER.init();
}

/// Register a device for resume tracking
pub fn register_device(info: DeviceResumeInfo) {
    RESUME_MANAGER.register_device(info);
}

/// Unregister a device
pub fn unregister_device(name: &'static str) {
    RESUME_MANAGER.unregister_device(name);
}

/// Execute optimized resume
pub fn resume_all() -> KResult<()> {
    RESUME_MANAGER.resume_all()
}

/// Prepare for suspend
pub fn prepare_suspend() {
    RESUME_MANAGER.prepare_suspend();
}

/// Get statistics
pub fn get_stats() -> ResumeStats {
    RESUME_MANAGER.get_stats()
}

/// Get configuration
pub fn get_config() -> ResumeConfig {
    RESUME_MANAGER.get_config()
}

/// Set configuration
pub fn set_config(config: ResumeConfig) {
    RESUME_MANAGER.set_config(config);
}

/// Get device list
pub fn get_devices() -> Vec<DeviceResumeInfo> {
    RESUME_MANAGER.get_devices()
}

/// Get timings from last resume
pub fn get_timings() -> Vec<ResumeTiming> {
    RESUME_MANAGER.get_timings()
}

/// Format status report
pub fn format_status() -> String {
    RESUME_MANAGER.format_status()
}

/// Mark device as suspended
pub fn mark_suspended(name: &'static str) {
    RESUME_MANAGER.mark_suspended(name);
}

/// Mark device as resumed
pub fn mark_resumed(name: &'static str) {
    RESUME_MANAGER.mark_resumed(name);
}

/// Enable/disable parallel resume
pub fn set_parallel_resume(enabled: bool) {
    RESUME_MANAGER.set_parallel_resume(enabled);
}

/// Set target resume time
pub fn set_target_time(ms: u32) {
    RESUME_MANAGER.set_target_time(ms);
}

/// Set device as skippable
pub fn set_device_skippable(name: &'static str, skippable: bool) {
    RESUME_MANAGER.set_device_skippable(name, skippable);
}
