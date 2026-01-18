//! CPU Frequency Scaling (P-States)
//!
//! Implements:
//! - Intel P-State driver
//! - AMD P-State driver
//! - CPU frequency governors
//! - Per-core frequency control

extern crate alloc;

use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, Ordering};

use crate::sync::IrqSafeMutex;
use crate::util::{KResult, KError};
use super::PowerProfile;

/// CPU frequency driver type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuFreqDriver {
    IntelPState,
    AmdPState,
    AcpiCpufreq,
    None,
}

/// CPU frequency governor
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Governor {
    Performance,
    Powersave,
    Ondemand,
    Conservative,
    Schedutil,
}

impl Default for Governor {
    fn default() -> Self {
        Governor::Schedutil
    }
}

/// Per-CPU frequency state
#[derive(Debug)]
pub struct CpuFreqState {
    pub cpu_id: u32,
    pub current_freq: AtomicU32,
    pub min_freq: u32,
    pub max_freq: u32,
    pub base_freq: u32,
    pub turbo_freq: u32,
    pub governor: Governor,
    pub scaling_min: u32,
    pub scaling_max: u32,
}

impl CpuFreqState {
    pub fn new(cpu_id: u32) -> Self {
        CpuFreqState {
            cpu_id,
            current_freq: AtomicU32::new(0),
            min_freq: 0,
            max_freq: 0,
            base_freq: 0,
            turbo_freq: 0,
            governor: Governor::default(),
            scaling_min: 0,
            scaling_max: 0,
        }
    }
}

/// Energy Performance Preference (EPP)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnergyPerformancePreference {
    Performance = 0,
    BalancePerformance = 128,
    BalancePower = 192,
    Power = 255,
}

/// Global cpufreq state
pub static CPUFREQ: IrqSafeMutex<CpuFreqManager> = IrqSafeMutex::new(CpuFreqManager::new());

/// CPU frequency manager
pub struct CpuFreqManager {
    driver: CpuFreqDriver,
    pub cpus: Vec<CpuFreqState>,
    global_governor: Governor,
    turbo_enabled: bool,
    hwp_enabled: bool,
    epp: EnergyPerformancePreference,
    initialized: bool,
}

impl CpuFreqManager {
    pub const fn new() -> Self {
        CpuFreqManager {
            driver: CpuFreqDriver::None,
            cpus: Vec::new(),
            global_governor: Governor::Schedutil,
            turbo_enabled: true,
            hwp_enabled: false,
            epp: EnergyPerformancePreference::BalancePerformance,
            initialized: false,
        }
    }

    /// Initialize cpufreq
    pub fn init(&mut self) {
        // Detect CPU vendor and available features
        let (vendor, family, model) = detect_cpu();

        crate::kprintln!("cpufreq: detected {} family {} model {}", vendor, family, model);

        // Choose driver based on CPU
        if vendor == "GenuineIntel" {
            self.init_intel_pstate();
        } else if vendor == "AuthenticAMD" {
            self.init_amd_pstate();
        } else {
            self.init_acpi_cpufreq();
        }

        self.initialized = true;
    }

    /// Initialize Intel P-State driver
    fn init_intel_pstate(&mut self) {
        crate::kprintln!("cpufreq: initializing Intel P-State driver");
        self.driver = CpuFreqDriver::IntelPState;

        // Check for Hardware P-State (HWP) support
        let hwp_supported = check_hwp_support();

        if hwp_supported {
            self.hwp_enabled = true;
            enable_hwp();
            crate::kprintln!("cpufreq: HWP enabled");
        }

        // Get frequency limits from MSRs
        let num_cpus = crate::arch::cpu_count();
        for cpu_id in 0..num_cpus {
            let mut state = CpuFreqState::new(cpu_id as u32);

            // Read MSR_PLATFORM_INFO for base frequency
            let platform_info = read_msr(0xCE);
            state.base_freq = ((platform_info >> 8) & 0xFF) as u32 * 100; // In MHz

            // Read MSR_TURBO_RATIO_LIMIT for turbo frequency
            let turbo_ratio = read_msr(0x1AD);
            state.turbo_freq = (turbo_ratio & 0xFF) as u32 * 100;

            // Set min/max from CPUID
            let (min_ratio, max_ratio) = get_pstate_limits();
            state.min_freq = min_ratio * 100;
            state.max_freq = if self.turbo_enabled { state.turbo_freq } else { state.base_freq };
            state.scaling_min = state.min_freq;
            state.scaling_max = state.max_freq;

            // Read current frequency
            let perf_status = read_msr(0x198);
            state.current_freq.store(((perf_status >> 8) & 0xFF) as u32 * 100, Ordering::Relaxed);

            self.cpus.push(state);
        }

        crate::kprintln!("cpufreq: Intel P-State initialized for {} CPUs", self.cpus.len());
    }

    /// Initialize AMD P-State driver
    fn init_amd_pstate(&mut self) {
        crate::kprintln!("cpufreq: initializing AMD P-State driver");
        self.driver = CpuFreqDriver::AmdPState;

        // Check for CPPC (Collaborative Processor Performance Control)
        let cppc_supported = check_cppc_support();

        let num_cpus = crate::arch::cpu_count();
        for cpu_id in 0..num_cpus {
            let mut state = CpuFreqState::new(cpu_id as u32);

            if cppc_supported {
                // Read CPPC registers
                let (lowest, nominal, highest) = read_cppc_caps();
                state.min_freq = lowest;
                state.base_freq = nominal;
                state.max_freq = highest;
                state.turbo_freq = highest;
            } else {
                // Fall back to ACPI P-states
                state.min_freq = 800;
                state.base_freq = 2000;
                state.max_freq = 4000;
                state.turbo_freq = 4500;
            }

            state.scaling_min = state.min_freq;
            state.scaling_max = state.max_freq;
            state.current_freq.store(state.base_freq, Ordering::Relaxed);

            self.cpus.push(state);
        }

        crate::kprintln!("cpufreq: AMD P-State initialized for {} CPUs", self.cpus.len());
    }

    /// Initialize ACPI cpufreq driver (fallback)
    fn init_acpi_cpufreq(&mut self) {
        crate::kprintln!("cpufreq: initializing ACPI cpufreq driver");
        self.driver = CpuFreqDriver::AcpiCpufreq;

        let num_cpus = crate::arch::cpu_count();
        for cpu_id in 0..num_cpus {
            let mut state = CpuFreqState::new(cpu_id as u32);

            // Use defaults; real implementation would parse ACPI _PSS
            state.min_freq = 800;
            state.base_freq = 2000;
            state.max_freq = 3000;
            state.turbo_freq = 3500;
            state.scaling_min = state.min_freq;
            state.scaling_max = state.max_freq;
            state.current_freq.store(state.base_freq, Ordering::Relaxed);

            self.cpus.push(state);
        }
    }

    /// Get driver type
    pub fn driver(&self) -> CpuFreqDriver {
        self.driver
    }

    /// Get CPU frequency state
    pub fn get_cpu(&self, cpu_id: u32) -> Option<&CpuFreqState> {
        self.cpus.iter().find(|c| c.cpu_id == cpu_id)
    }

    /// Set CPU frequency
    pub fn set_frequency(&mut self, cpu_id: u32, freq_mhz: u32) -> KResult<()> {
        let cpu = self.cpus.iter_mut().find(|c| c.cpu_id == cpu_id)
            .ok_or(KError::NotFound)?;

        let freq = freq_mhz.clamp(cpu.scaling_min, cpu.scaling_max);

        match self.driver {
            CpuFreqDriver::IntelPState => {
                if self.hwp_enabled {
                    // Set HWP desired performance
                    let perf = (freq * 255 / cpu.max_freq).min(255);
                    set_hwp_request(cpu_id, perf as u8);
                } else {
                    // Set P-state via MSR
                    let ratio = freq / 100;
                    write_msr_on_cpu(cpu_id, 0x199, (ratio as u64) << 8);
                }
            }
            CpuFreqDriver::AmdPState => {
                // Set CPPC desired performance
                let perf = (freq * 255 / cpu.max_freq).min(255);
                set_cppc_desired(cpu_id, perf as u8);
            }
            CpuFreqDriver::AcpiCpufreq => {
                // Use ACPI _PCT to set P-state
            }
            CpuFreqDriver::None => return Err(KError::NotSupported),
        }

        cpu.current_freq.store(freq, Ordering::Relaxed);
        Ok(())
    }

    /// Set frequency scaling limits
    pub fn set_scaling_limits(&mut self, cpu_id: u32, min: u32, max: u32) -> KResult<()> {
        let cpu = self.cpus.iter_mut().find(|c| c.cpu_id == cpu_id)
            .ok_or(KError::NotFound)?;

        cpu.scaling_min = min.clamp(cpu.min_freq, cpu.max_freq);
        cpu.scaling_max = max.clamp(cpu.min_freq, cpu.max_freq);

        // Update HWP limits if enabled
        if self.hwp_enabled {
            let min_perf = (cpu.scaling_min * 255 / cpu.max_freq) as u8;
            let max_perf = (cpu.scaling_max * 255 / cpu.max_freq) as u8;
            set_hwp_limits(cpu_id, min_perf, max_perf);
        }

        Ok(())
    }

    /// Set governor
    pub fn set_governor(&mut self, governor: Governor) {
        self.global_governor = governor;
        for cpu in &mut self.cpus {
            cpu.governor = governor;
        }

        // Adjust EPP based on governor
        match governor {
            Governor::Performance => {
                self.epp = EnergyPerformancePreference::Performance;
            }
            Governor::Powersave => {
                self.epp = EnergyPerformancePreference::Power;
            }
            _ => {
                self.epp = EnergyPerformancePreference::BalancePerformance;
            }
        }

        if self.hwp_enabled {
            for cpu in &self.cpus {
                set_hwp_epp(cpu.cpu_id, self.epp as u8);
            }
        }

        crate::kprintln!("cpufreq: governor set to {:?}", governor);
    }

    /// Enable/disable turbo boost
    pub fn set_turbo(&mut self, enabled: bool) {
        self.turbo_enabled = enabled;

        // Update max frequency for all CPUs
        for cpu in &mut self.cpus {
            cpu.scaling_max = if enabled { cpu.turbo_freq } else { cpu.base_freq };
        }

        // Set MSR_IA32_MISC_ENABLE bit 38 (turbo disable)
        let misc_enable = read_msr(0x1A0);
        if enabled {
            write_msr(0x1A0, misc_enable & !(1u64 << 38));
        } else {
            write_msr(0x1A0, misc_enable | (1u64 << 38));
        }

        crate::kprintln!("cpufreq: turbo boost {}", if enabled { "enabled" } else { "disabled" });
    }

    /// Is turbo enabled
    pub fn turbo_enabled(&self) -> bool {
        self.turbo_enabled
    }

    /// Set energy performance preference
    pub fn set_epp(&mut self, epp: EnergyPerformancePreference) {
        self.epp = epp;
        if self.hwp_enabled {
            for cpu in &self.cpus {
                set_hwp_epp(cpu.cpu_id, epp as u8);
            }
        }
    }
}

/// Apply power profile to cpufreq
pub fn apply_profile(profile: PowerProfile) {
    let mut cpufreq = CPUFREQ.lock();

    match profile {
        PowerProfile::Performance => {
            cpufreq.set_governor(Governor::Performance);
            cpufreq.set_turbo(true);
            cpufreq.set_epp(EnergyPerformancePreference::Performance);
        }
        PowerProfile::Balanced => {
            cpufreq.set_governor(Governor::Schedutil);
            cpufreq.set_turbo(true);
            cpufreq.set_epp(EnergyPerformancePreference::BalancePerformance);
        }
        PowerProfile::PowerSaver => {
            cpufreq.set_governor(Governor::Powersave);
            cpufreq.set_turbo(false);
            cpufreq.set_epp(EnergyPerformancePreference::BalancePower);
        }
        PowerProfile::BatterySaver => {
            cpufreq.set_governor(Governor::Powersave);
            cpufreq.set_turbo(false);
            cpufreq.set_epp(EnergyPerformancePreference::Power);
            // Set all CPUs to minimum frequency
            for i in 0..cpufreq.cpus.len() {
                let min = cpufreq.cpus[i].min_freq;
                let _ = cpufreq.set_frequency(i as u32, min);
            }
        }
    }
}

/// Initialize cpufreq subsystem
pub fn init() {
    CPUFREQ.lock().init();
    crate::kprintln!("cpufreq: subsystem initialized");
}

// Helper functions

fn detect_cpu() -> (&'static str, u32, u32) {
    let mut vendor = [0u32; 3];
    let mut family: u32;

    unsafe {
        // Save rbx, use cpuid, then restore rbx (LLVM reserves rbx)
        core::arch::asm!(
            "push rbx",
            "mov eax, 0",
            "cpuid",
            "mov {0:e}, ebx",
            "mov {1:e}, edx",
            "mov {2:e}, ecx",
            "pop rbx",
            out(reg) vendor[0],
            out(reg) vendor[1],
            out(reg) vendor[2],
            out("eax") _,
            out("edx") _,
            out("ecx") _,
        );

        core::arch::asm!(
            "push rbx",
            "mov eax, 1",
            "cpuid",
            "pop rbx",
            lateout("eax") family,
            out("ecx") _,
            out("edx") _,
        );
    }

    let model = (family >> 4) & 0xF;
    let family_extracted = (family >> 8) & 0xF;

    let vendor_str = if vendor == [0x756E6547, 0x49656E69, 0x6C65746E] {
        "GenuineIntel"
    } else if vendor == [0x68747541, 0x69746E65, 0x444D4163] {
        "AuthenticAMD"
    } else {
        "Unknown"
    };

    (vendor_str, family_extracted, model)
}

fn check_hwp_support() -> bool {
    let eax: u32;
    unsafe {
        core::arch::asm!(
            "push rbx",
            "mov eax, 6",
            "cpuid",
            "pop rbx",
            lateout("eax") eax,
            out("ecx") _,
            out("edx") _,
        );
    }
    (eax & (1 << 7)) != 0 // HWP bit
}

fn enable_hwp() {
    // Set IA32_PM_ENABLE.HWP_ENABLE (bit 0)
    let pm_enable = read_msr(0x770);
    write_msr(0x770, pm_enable | 1);
}

fn check_cppc_support() -> bool {
    // Check CPUID for CPPC support
    let edx: u32;
    unsafe {
        core::arch::asm!(
            "push rbx",
            "mov eax, 0x80000008",
            "cpuid",
            "pop rbx",
            out("eax") _,
            out("ecx") _,
            lateout("edx") edx,
        );
    }
    (edx & (1 << 25)) != 0 // CPPC bit
}

fn read_cppc_caps() -> (u32, u32, u32) {
    // Read from MSR 0xC0010061-0xC0010063
    let lowest = (read_msr(0xC0010061) & 0xFF) as u32 * 100;
    let nominal = ((read_msr(0xC0010061) >> 8) & 0xFF) as u32 * 100;
    let highest = ((read_msr(0xC0010061) >> 16) & 0xFF) as u32 * 100;
    (lowest, nominal, highest)
}

fn get_pstate_limits() -> (u32, u32) {
    let platform_info = read_msr(0xCE);
    let min_ratio = ((platform_info >> 40) & 0xFF) as u32;
    let max_ratio = ((platform_info >> 8) & 0xFF) as u32;
    (min_ratio, max_ratio)
}

fn read_msr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;
    unsafe {
        core::arch::asm!(
            "rdmsr",
            in("ecx") msr,
            out("eax") low,
            out("edx") high,
        );
    }
    ((high as u64) << 32) | (low as u64)
}

fn write_msr(msr: u32, value: u64) {
    let low = value as u32;
    let high = (value >> 32) as u32;
    unsafe {
        core::arch::asm!(
            "wrmsr",
            in("ecx") msr,
            in("eax") low,
            in("edx") high,
        );
    }
}

fn write_msr_on_cpu(_cpu_id: u32, msr: u32, value: u64) {
    // In SMP, would send IPI to target CPU
    // For now, just write on current CPU
    write_msr(msr, value);
}

fn set_hwp_request(cpu_id: u32, desired: u8) {
    // MSR_HWP_REQUEST (0x774)
    let request = read_msr(0x774);
    let new_request = (request & !0xFF0000u64) | ((desired as u64) << 16);
    write_msr_on_cpu(cpu_id, 0x774, new_request);
}

fn set_hwp_limits(cpu_id: u32, min: u8, max: u8) {
    let request = read_msr(0x774);
    let new_request = (request & !0xFFFFu64) | (min as u64) | ((max as u64) << 8);
    write_msr_on_cpu(cpu_id, 0x774, new_request);
}

fn set_hwp_epp(cpu_id: u32, epp: u8) {
    let request = read_msr(0x774);
    let new_request = (request & !0xFF000000u64) | ((epp as u64) << 24);
    write_msr_on_cpu(cpu_id, 0x774, new_request);
}

fn set_cppc_desired(cpu_id: u32, perf: u8) {
    // Write to AMD CPPC desired performance MSR
    let cppc_req = read_msr(0xC0010062);
    let new_req = (cppc_req & !0xFFu64) | (perf as u64);
    write_msr_on_cpu(cpu_id, 0xC0010062, new_req);
}
