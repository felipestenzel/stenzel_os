//! CPU Frequency Scaling (cpufreq)
//!
//! Supports:
//! - Intel SpeedStep (EIST)
//! - Intel Turbo Boost
//! - AMD Cool'n'Quiet
//! - P-state management via MSRs
//!
//! Provides governors:
//! - performance: always max frequency
//! - powersave: always min frequency
//! - ondemand: scale based on load (default)

#![allow(dead_code)]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;
use spin::Mutex;

/// MSR definitions for frequency control
mod msr {
    pub const IA32_PERF_STATUS: u32 = 0x198;      // Current P-state
    pub const IA32_PERF_CTL: u32 = 0x199;         // Target P-state control
    pub const MSR_PLATFORM_INFO: u32 = 0xCE;      // Platform info (base/max ratio)
    pub const MSR_TURBO_RATIO_LIMIT: u32 = 0x1AD; // Turbo ratio limits
    pub const IA32_MISC_ENABLE: u32 = 0x1A0;      // Enable/disable EIST
    pub const IA32_ENERGY_PERF_BIAS: u32 = 0x1B0; // Energy performance bias
    pub const MSR_PKG_POWER_LIMIT: u32 = 0x610;   // Package power limit

    // AMD
    pub const AMD_PSTATE_DEF_BASE: u32 = 0xC0010064; // P-state definitions
    pub const AMD_COFVID_STATUS: u32 = 0xC0010071;   // Current frequency info
}

/// CPUID feature bits
mod cpuid {
    pub const EIST_BIT: u32 = 1 << 7;        // Enhanced SpeedStep
    pub const TURBO_BIT: u32 = 1 << 1;       // Turbo Boost (in IA32_MISC_ENABLE)
    pub const HWPSTATE_BIT: u32 = 1 << 7;    // Hardware P-state (AMD)
}

/// Frequency governor
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Governor {
    /// Always run at maximum frequency
    Performance,
    /// Always run at minimum frequency
    Powersave,
    /// Scale frequency based on load
    Ondemand,
    /// User-defined frequency
    Userspace,
}

impl Governor {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.trim() {
            "performance" => Some(Self::Performance),
            "powersave" => Some(Self::Powersave),
            "ondemand" => Some(Self::Ondemand),
            "userspace" => Some(Self::Userspace),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Performance => "performance",
            Self::Powersave => "powersave",
            Self::Ondemand => "ondemand",
            Self::Userspace => "userspace",
        }
    }
}

/// P-state definition
#[derive(Debug, Clone, Copy)]
pub struct PState {
    /// P-state index (0 = highest performance)
    pub index: u8,
    /// Frequency in kHz
    pub frequency_khz: u32,
    /// Voltage in mV (if available)
    pub voltage_mv: Option<u16>,
    /// Whether this is a turbo state
    pub turbo: bool,
}

/// CPU frequency information
#[derive(Debug)]
pub struct CpuFreqInfo {
    /// CPU index
    pub cpu: u8,
    /// Base frequency (non-turbo max)
    pub base_freq_khz: u32,
    /// Minimum frequency
    pub min_freq_khz: u32,
    /// Maximum frequency (including turbo)
    pub max_freq_khz: u32,
    /// Current frequency
    pub cur_freq_khz: u32,
    /// Available P-states
    pub pstates: Vec<PState>,
    /// Current governor
    pub governor: Governor,
    /// EIST (SpeedStep) supported
    pub eist_supported: bool,
    /// Turbo supported
    pub turbo_supported: bool,
    /// Turbo enabled
    pub turbo_enabled: bool,
}

/// Global cpufreq state
struct CpuFreqState {
    /// Per-CPU info
    cpus: Vec<CpuFreqInfo>,
    /// Global governor (applied to all CPUs)
    global_governor: Governor,
    /// Ondemand threshold (% load to scale up)
    ondemand_up_threshold: u8,
    /// Ondemand sample rate in ms
    ondemand_sample_rate_ms: u32,
    /// Is frequency scaling initialized
    initialized: bool,
}

impl CpuFreqState {
    const fn new() -> Self {
        Self {
            cpus: Vec::new(),
            global_governor: Governor::Ondemand,
            ondemand_up_threshold: 80,
            ondemand_sample_rate_ms: 100,
            initialized: false,
        }
    }
}

static CPUFREQ: Mutex<CpuFreqState> = Mutex::new(CpuFreqState::new());

/// Initialize CPU frequency scaling
pub fn init() {
    crate::kprintln!("cpufreq: initializing...");

    let mut state = CPUFREQ.lock();

    // Check for EIST support
    if !check_eist_support() {
        crate::kprintln!("cpufreq: EIST not supported");
        return;
    }

    // Enable EIST if disabled
    enable_eist();

    // Get CPU info for BSP (CPU 0)
    if let Some(info) = probe_cpu(0) {
        crate::kprintln!("cpufreq: CPU 0 - base {}MHz, min {}MHz, max {}MHz",
            info.base_freq_khz / 1000,
            info.min_freq_khz / 1000,
            info.max_freq_khz / 1000);

        if info.turbo_supported {
            crate::kprintln!("cpufreq: Turbo Boost {}",
                if info.turbo_enabled { "enabled" } else { "disabled" });
        }

        state.cpus.push(info);
    }

    // Probe additional CPUs
    let num_cpus = crate::arch::x86_64_arch::smp::cpu_count() as u8;
    for cpu in 1..num_cpus {
        if let Some(info) = probe_cpu(cpu) {
            state.cpus.push(info);
        }
    }

    state.initialized = true;
    crate::kprintln!("cpufreq: {} CPUs configured, governor: {}",
        state.cpus.len(), state.global_governor.as_str());
}

/// Check if EIST (Enhanced Intel SpeedStep) is supported
fn check_eist_support() -> bool {
    // Check CPUID for EIST support
    let result = unsafe { core::arch::x86_64::__cpuid(1) };
    (result.ecx & cpuid::EIST_BIT) != 0
}

/// Enable EIST via IA32_MISC_ENABLE
fn enable_eist() {
    unsafe {
        let misc = read_msr(msr::IA32_MISC_ENABLE);
        if misc & (1 << 16) == 0 {
            // Enable EIST (bit 16)
            write_msr(msr::IA32_MISC_ENABLE, misc | (1 << 16));
        }
    }
}

/// Probe a specific CPU for frequency info
fn probe_cpu(cpu: u8) -> Option<CpuFreqInfo> {
    // For now, we assume all CPUs have the same capabilities as BSP
    // In a full implementation, we would execute this on each CPU

    let is_intel = check_intel_cpu();

    if is_intel {
        probe_intel_cpu(cpu)
    } else {
        probe_amd_cpu(cpu)
    }
}

fn check_intel_cpu() -> bool {
    let result = unsafe { core::arch::x86_64::__cpuid(0) };
    let vendor = [
        (result.ebx as u8) as char,
        ((result.ebx >> 8) as u8) as char,
        ((result.ebx >> 16) as u8) as char,
        ((result.ebx >> 24) as u8) as char,
    ];
    vendor == ['G', 'e', 'n', 'u'] // "GenuineIntel"
}

fn probe_intel_cpu(cpu: u8) -> Option<CpuFreqInfo> {
    // Read platform info for base ratio
    let platform_info = unsafe { read_msr(msr::MSR_PLATFORM_INFO) };

    // Max non-turbo ratio (bits 15:8)
    let base_ratio = ((platform_info >> 8) & 0xFF) as u32;
    // Min ratio (bits 47:40)
    let min_ratio = ((platform_info >> 40) & 0xFF) as u32;

    // Bus frequency is typically 100MHz for modern Intel CPUs
    let bus_freq_khz = 100_000u32;

    let base_freq_khz = base_ratio * bus_freq_khz;
    let min_freq_khz = min_ratio * bus_freq_khz;

    // Check turbo support
    let misc_enable = unsafe { read_msr(msr::IA32_MISC_ENABLE) };
    let turbo_supported = check_eist_support(); // Turbo requires EIST
    let turbo_enabled = (misc_enable & (1 << 38)) == 0; // Turbo disable bit

    // Get max turbo ratio
    let max_ratio = if turbo_supported && turbo_enabled {
        let turbo_ratio = unsafe { read_msr(msr::MSR_TURBO_RATIO_LIMIT) };
        // Single-core turbo ratio (bits 7:0)
        (turbo_ratio & 0xFF) as u32
    } else {
        base_ratio
    };

    let max_freq_khz = max_ratio * bus_freq_khz;

    // Get current frequency
    let perf_status = unsafe { read_msr(msr::IA32_PERF_STATUS) };
    let cur_ratio = ((perf_status >> 8) & 0xFF) as u32;
    let cur_freq_khz = cur_ratio * bus_freq_khz;

    // Build P-state list
    let mut pstates = Vec::new();

    // Add turbo states if available
    if turbo_supported && turbo_enabled && max_ratio > base_ratio {
        pstates.push(PState {
            index: 0,
            frequency_khz: max_freq_khz,
            voltage_mv: None,
            turbo: true,
        });
    }

    // Add base ratio
    let base_idx = if pstates.is_empty() { 0 } else { 1 };
    pstates.push(PState {
        index: base_idx,
        frequency_khz: base_freq_khz,
        voltage_mv: None,
        turbo: false,
    });

    // Add intermediate ratios (simplified - real systems have more complex P-state tables)
    let mut idx = base_idx + 1;
    let ratio_step = (base_ratio - min_ratio) / 4;
    if ratio_step > 0 {
        for i in 1..4 {
            let ratio = base_ratio - (ratio_step * i);
            if ratio > min_ratio {
                pstates.push(PState {
                    index: idx,
                    frequency_khz: ratio * bus_freq_khz,
                    voltage_mv: None,
                    turbo: false,
                });
                idx += 1;
            }
        }
    }

    // Add minimum ratio
    pstates.push(PState {
        index: idx,
        frequency_khz: min_freq_khz,
        voltage_mv: None,
        turbo: false,
    });

    Some(CpuFreqInfo {
        cpu,
        base_freq_khz,
        min_freq_khz,
        max_freq_khz,
        cur_freq_khz,
        pstates,
        governor: Governor::Ondemand,
        eist_supported: true,
        turbo_supported,
        turbo_enabled,
    })
}

fn probe_amd_cpu(cpu: u8) -> Option<CpuFreqInfo> {
    // AMD P-state handling is different
    // Read P0 state definition
    let p0_def = unsafe { read_msr(msr::AMD_PSTATE_DEF_BASE) };

    // AMD uses different encoding depending on family
    // This is simplified for modern Zen-based CPUs

    // For Zen, frequency = (FID[7:0] / DID[5:0]) * 200 MHz
    let fid = (p0_def & 0xFF) as u32;
    let did = ((p0_def >> 6) & 0x1F) as u32;

    let max_freq_khz = if did > 0 {
        (fid * 200_000) / did
    } else {
        fid * 200_000
    };

    // Get min frequency from lowest P-state
    let p7_def = unsafe { read_msr(msr::AMD_PSTATE_DEF_BASE + 7) };
    let min_fid = (p7_def & 0xFF) as u32;
    let min_did = ((p7_def >> 6) & 0x1F) as u32;
    let min_freq_khz = if min_did > 0 {
        (min_fid * 200_000) / min_did
    } else {
        min_fid * 200_000
    };

    // Get current state
    let cof_status = unsafe { read_msr(msr::AMD_COFVID_STATUS) };
    let cur_fid = (cof_status & 0xFF) as u32;
    let cur_did = ((cof_status >> 6) & 0x1F) as u32;
    let cur_freq_khz = if cur_did > 0 {
        (cur_fid * 200_000) / cur_did
    } else {
        cur_fid * 200_000
    };

    // Build P-states
    let mut pstates = Vec::new();
    for i in 0..8 {
        let pdef = unsafe { read_msr(msr::AMD_PSTATE_DEF_BASE + i) };
        if pdef & (1 << 63) != 0 {
            // P-state is valid
            let f = (pdef & 0xFF) as u32;
            let d = ((pdef >> 6) & 0x1F) as u32;
            let freq = if d > 0 { (f * 200_000) / d } else { f * 200_000 };
            pstates.push(PState {
                index: i as u8,
                frequency_khz: freq,
                voltage_mv: Some(((pdef >> 14) & 0xFF) as u16 * 25), // Approximate
                turbo: i == 0,
            });
        }
    }

    Some(CpuFreqInfo {
        cpu,
        base_freq_khz: max_freq_khz,
        min_freq_khz,
        max_freq_khz,
        cur_freq_khz,
        pstates,
        governor: Governor::Ondemand,
        eist_supported: true,
        turbo_supported: true,
        turbo_enabled: true,
    })
}

// Public API

/// Get frequency info for a CPU
pub fn get_cpu_info(cpu: u8) -> Option<CpuFreqInfo> {
    let state = CPUFREQ.lock();
    state.cpus.iter().find(|c| c.cpu == cpu).cloned()
}

impl Clone for CpuFreqInfo {
    fn clone(&self) -> Self {
        Self {
            cpu: self.cpu,
            base_freq_khz: self.base_freq_khz,
            min_freq_khz: self.min_freq_khz,
            max_freq_khz: self.max_freq_khz,
            cur_freq_khz: self.cur_freq_khz,
            pstates: self.pstates.clone(),
            governor: self.governor,
            eist_supported: self.eist_supported,
            turbo_supported: self.turbo_supported,
            turbo_enabled: self.turbo_enabled,
        }
    }
}

/// Get current frequency for a CPU (in kHz)
pub fn get_frequency(cpu: u8) -> Option<u32> {
    let state = CPUFREQ.lock();
    state.cpus.iter().find(|c| c.cpu == cpu).map(|c| c.cur_freq_khz)
}

/// Set target frequency for a CPU (in kHz)
pub fn set_frequency(cpu: u8, freq_khz: u32) -> Result<(), &'static str> {
    let mut state = CPUFREQ.lock();

    let info = state.cpus.iter_mut().find(|c| c.cpu == cpu)
        .ok_or("CPU not found")?;

    // Find nearest P-state
    let pstate = info.pstates.iter()
        .min_by_key(|p| (p.frequency_khz as i64 - freq_khz as i64).abs() as u64)
        .ok_or("No P-states available")?;

    // Set via MSR
    if check_intel_cpu() {
        set_intel_pstate(pstate.frequency_khz / 100_000)?;
    } else {
        set_amd_pstate(pstate.index)?;
    }

    info.cur_freq_khz = pstate.frequency_khz;
    Ok(())
}

fn set_intel_pstate(ratio: u32) -> Result<(), &'static str> {
    // Write target ratio to IA32_PERF_CTL
    let value = (ratio as u64) << 8;
    unsafe { write_msr(msr::IA32_PERF_CTL, value); }
    Ok(())
}

fn set_amd_pstate(pstate: u8) -> Result<(), &'static str> {
    if pstate > 7 {
        return Err("Invalid P-state");
    }
    // AMD uses different mechanism - this is simplified
    // Real implementation would use ACPI _PCT/_PSS methods
    Ok(())
}

/// Get current governor
pub fn get_governor() -> Governor {
    CPUFREQ.lock().global_governor
}

/// Set governor for all CPUs
pub fn set_governor(gov: Governor) {
    let mut state = CPUFREQ.lock();
    state.global_governor = gov;

    for cpu in &mut state.cpus {
        cpu.governor = gov;

        // Apply governor policy
        match gov {
            Governor::Performance => {
                let _ = set_frequency_internal(cpu, cpu.max_freq_khz);
            }
            Governor::Powersave => {
                let _ = set_frequency_internal(cpu, cpu.min_freq_khz);
            }
            Governor::Ondemand | Governor::Userspace => {
                // Ondemand will be handled by periodic sampling
            }
        }
    }
}

fn set_frequency_internal(info: &mut CpuFreqInfo, freq_khz: u32) -> Result<(), &'static str> {
    if check_intel_cpu() {
        let ratio = freq_khz / 100_000;
        set_intel_pstate(ratio)?;
    }
    info.cur_freq_khz = freq_khz;
    Ok(())
}

/// Enable or disable turbo boost
pub fn set_turbo_enabled(enabled: bool) -> Result<(), &'static str> {
    let mut state = CPUFREQ.lock();

    for cpu in &mut state.cpus {
        if cpu.turbo_supported {
            unsafe {
                let misc = read_msr(msr::IA32_MISC_ENABLE);
                if enabled {
                    // Clear turbo disable bit
                    write_msr(msr::IA32_MISC_ENABLE, misc & !(1 << 38));
                } else {
                    // Set turbo disable bit
                    write_msr(msr::IA32_MISC_ENABLE, misc | (1 << 38));
                }
            }
            cpu.turbo_enabled = enabled;
        }
    }

    Ok(())
}

/// Get list of available governors
pub fn available_governors() -> Vec<Governor> {
    alloc::vec![
        Governor::Performance,
        Governor::Powersave,
        Governor::Ondemand,
        Governor::Userspace,
    ]
}

/// Called periodically by scheduler to update frequencies (for ondemand governor)
pub fn periodic_update(cpu_load: u8) {
    let mut state = CPUFREQ.lock();

    if state.global_governor != Governor::Ondemand {
        return;
    }

    // Copy threshold to avoid borrow issues
    let threshold = state.ondemand_up_threshold;

    for cpu in &mut state.cpus {
        let target_freq = if cpu_load > threshold {
            cpu.max_freq_khz
        } else {
            // Linear scaling between min and base based on load
            let range = cpu.base_freq_khz - cpu.min_freq_khz;
            let scaled = (range as u64 * cpu_load as u64 / 100) as u32;
            cpu.min_freq_khz + scaled
        };

        if (target_freq as i64 - cpu.cur_freq_khz as i64).abs() > 100_000 {
            let _ = set_frequency_internal(cpu, target_freq);
        }
    }
}

// MSR helpers

unsafe fn read_msr(msr: u32) -> u64 {
    let (low, high): (u32, u32);
    core::arch::asm!(
        "rdmsr",
        in("ecx") msr,
        out("eax") low,
        out("edx") high,
    );
    ((high as u64) << 32) | (low as u64)
}

unsafe fn write_msr(msr: u32, value: u64) {
    let low = value as u32;
    let high = (value >> 32) as u32;
    core::arch::asm!(
        "wrmsr",
        in("ecx") msr,
        in("eax") low,
        in("edx") high,
    );
}

// sysfs interface

/// Get scaling_available_governors for sysfs
pub fn sysfs_available_governors() -> String {
    "performance powersave ondemand userspace\n".to_string()
}

use alloc::string::ToString;

/// Get scaling_governor for sysfs
pub fn sysfs_governor() -> String {
    format!("{}\n", get_governor().as_str())
}

/// Get scaling_cur_freq for sysfs (in kHz)
pub fn sysfs_cur_freq(cpu: u8) -> String {
    match get_frequency(cpu) {
        Some(freq) => format!("{}\n", freq),
        None => String::from("0\n"),
    }
}

/// Get scaling_min_freq for sysfs
pub fn sysfs_min_freq(cpu: u8) -> String {
    let state = CPUFREQ.lock();
    match state.cpus.iter().find(|c| c.cpu == cpu) {
        Some(info) => format!("{}\n", info.min_freq_khz),
        None => String::from("0\n"),
    }
}

/// Get scaling_max_freq for sysfs
pub fn sysfs_max_freq(cpu: u8) -> String {
    let state = CPUFREQ.lock();
    match state.cpus.iter().find(|c| c.cpu == cpu) {
        Some(info) => format!("{}\n", info.max_freq_khz),
        None => String::from("0\n"),
    }
}

/// Get cpuinfo_cur_freq for sysfs (actual hardware frequency)
pub fn sysfs_cpuinfo_freq(cpu: u8) -> String {
    // Read actual frequency from MSR
    let freq = unsafe {
        let status = read_msr(msr::IA32_PERF_STATUS);
        let ratio = ((status >> 8) & 0xFF) as u32;
        ratio * 100_000 // Assuming 100MHz bus
    };
    format!("{}\n", freq)
}

/// Get available_frequencies for sysfs
pub fn sysfs_available_freqs(cpu: u8) -> String {
    let state = CPUFREQ.lock();
    match state.cpus.iter().find(|c| c.cpu == cpu) {
        Some(info) => {
            let freqs: Vec<String> = info.pstates.iter()
                .map(|p| format!("{}", p.frequency_khz))
                .collect();
            format!("{}\n", freqs.join(" "))
        }
        None => String::new(),
    }
}
