//! TSC (Time Stamp Counter) Support
//!
//! The TSC is a 64-bit register that counts processor cycles.
//! On modern processors with "invariant TSC", it runs at a constant rate
//! regardless of power state changes.

#![allow(dead_code)]

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// TSC state
static TSC_AVAILABLE: AtomicBool = AtomicBool::new(false);
static TSC_INVARIANT: AtomicBool = AtomicBool::new(false);
static TSC_HAS_RDTSCP: AtomicBool = AtomicBool::new(false);
static TSC_FREQ_HZ: AtomicU64 = AtomicU64::new(0);
static TSC_FREQ_KHZ: AtomicU64 = AtomicU64::new(0);

/// Check if TSC is available
pub fn is_available() -> bool {
    let cpuid = core::arch::x86_64::__cpuid(1);
    // TSC is indicated by bit 4 of EDX
    (cpuid.edx & (1 << 4)) != 0
}

/// Check if invariant TSC is available (constant rate regardless of power state)
pub fn is_invariant() -> bool {
    // Check for advanced power management features
    let cpuid = core::arch::x86_64::__cpuid(0x80000000);
    if cpuid.eax < 0x80000007 {
        return false;
    }

    let cpuid = core::arch::x86_64::__cpuid(0x80000007);
    // Invariant TSC is indicated by bit 8 of EDX
    (cpuid.edx & (1 << 8)) != 0
}

/// Read the current TSC value
#[inline(always)]
pub fn read() -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe {
        core::arch::asm!(
            "rdtsc",
            out("eax") lo,
            out("edx") hi,
            options(nomem, nostack, preserves_flags)
        );
    }
    ((hi as u64) << 32) | (lo as u64)
}

/// Read TSC with serialization (RDTSCP if available, otherwise LFENCE + RDTSC)
/// More accurate for timing measurements as it waits for all previous instructions
#[inline(always)]
pub fn read_serialized() -> u64 {
    // Use RDTSCP if available (check cached value for performance)
    if TSC_HAS_RDTSCP.load(Ordering::Relaxed) {
        let lo: u32;
        let hi: u32;
        let _aux: u32;
        unsafe {
            core::arch::asm!(
                "rdtscp",
                out("eax") lo,
                out("edx") hi,
                out("ecx") _aux,
                options(nomem, nostack, preserves_flags)
            );
        }
        ((hi as u64) << 32) | (lo as u64)
    } else {
        // Fallback: use LFENCE + RDTSC for serialization
        let lo: u32;
        let hi: u32;
        unsafe {
            core::arch::asm!(
                "lfence",
                "rdtsc",
                out("eax") lo,
                out("edx") hi,
                options(nomem, nostack, preserves_flags)
            );
        }
        ((hi as u64) << 32) | (lo as u64)
    }
}

/// Check if RDTSCP is available
pub fn has_rdtscp() -> bool {
    let cpuid = core::arch::x86_64::__cpuid(0x80000000);
    if cpuid.eax < 0x80000001 {
        return false;
    }

    let cpuid = core::arch::x86_64::__cpuid(0x80000001);
    // RDTSCP is indicated by bit 27 of EDX
    (cpuid.edx & (1 << 27)) != 0
}

/// Calibrate TSC frequency using HPET
fn calibrate_with_hpet() -> Option<u64> {
    use crate::drivers::hpet;

    if !hpet::is_enabled() {
        return None;
    }

    crate::kprintln!("tsc: calibrando via HPET...");

    // Wait for HPET counter to stabilize
    hpet::sleep_ms(1);

    // Measure TSC over 50ms using HPET
    let tsc_start = read();
    hpet::sleep_ms(50);
    let tsc_end = read();

    let tsc_diff = tsc_end.wrapping_sub(tsc_start);

    if tsc_diff == 0 {
        return None;
    }

    // Calculate frequency: tsc_diff / 50ms = tsc_diff * 20 = Hz
    let freq_hz = tsc_diff * 20;

    Some(freq_hz)
}

/// Calibrate TSC frequency using PIT
fn calibrate_with_pit() -> u64 {
    use x86_64::instructions::port::Port;

    unsafe {
        let mut pit_cmd: Port<u8> = Port::new(0x43);
        let mut pit_ch2: Port<u8> = Port::new(0x42);
        let mut port_61: Port<u8> = Port::new(0x61);

        // Enable speaker gate for channel 2
        let old_61 = port_61.read();
        port_61.write((old_61 & 0xFD) | 0x01);

        // Configure PIT channel 2 for mode 0 (one-shot)
        pit_cmd.write(0xB0);

        // Count for ~50ms (1193182 Hz / 20 = 59659)
        let pit_count: u16 = 59659;
        pit_ch2.write((pit_count & 0xFF) as u8);
        pit_ch2.write((pit_count >> 8) as u8);

        // Read TSC before
        let tsc_start = read();

        // Wait for PIT to finish
        while (port_61.read() & 0x20) == 0 {
            core::hint::spin_loop();
        }

        // Read TSC after
        let tsc_end = read();

        // Restore port 61
        port_61.write(old_61);

        let tsc_diff = tsc_end.wrapping_sub(tsc_start);

        // Calculate frequency: tsc_diff / 50ms = tsc_diff * 20 = Hz
        tsc_diff * 20
    }
}

/// Try to get TSC frequency from CPUID (leaf 0x15)
fn get_freq_from_cpuid() -> Option<u64> {
    // Check if leaf 0x15 is available
    let cpuid = core::arch::x86_64::__cpuid(0);
    if cpuid.eax < 0x15 {
        return None;
    }

    let cpuid = core::arch::x86_64::__cpuid(0x15);
    let denominator = cpuid.eax;  // TSC/core crystal clock ratio denominator
    let numerator = cpuid.ebx;    // TSC/core crystal clock ratio numerator
    let crystal_freq = cpuid.ecx; // Core crystal clock frequency (if available)

    if denominator == 0 || numerator == 0 {
        return None;
    }

    // If crystal frequency is provided, calculate TSC frequency
    if crystal_freq != 0 {
        let freq = (crystal_freq as u64 * numerator as u64) / denominator as u64;
        return Some(freq);
    }

    // On some processors, we need to use leaf 0x16 for the base frequency
    let cpuid0 = core::arch::x86_64::__cpuid(0);
    if cpuid0.eax >= 0x16 {
        let cpuid16 = core::arch::x86_64::__cpuid(0x16);
        let base_freq_mhz = cpuid16.eax & 0xFFFF;
        if base_freq_mhz != 0 {
            return Some(base_freq_mhz as u64 * 1_000_000);
        }
    }

    None
}

/// Initialize TSC
pub fn init() -> bool {
    if !is_available() {
        crate::kprintln!("tsc: não disponível");
        return false;
    }

    TSC_AVAILABLE.store(true, Ordering::Relaxed);

    let invariant = is_invariant();
    TSC_INVARIANT.store(invariant, Ordering::Relaxed);

    let rdtscp = has_rdtscp();
    TSC_HAS_RDTSCP.store(rdtscp, Ordering::Relaxed);

    crate::kprintln!("tsc: disponível (invariant={}, rdtscp={})", invariant, rdtscp);

    // Try to get frequency from CPUID first
    let freq_hz = if let Some(freq) = get_freq_from_cpuid() {
        crate::kprintln!("tsc: frequência via CPUID = {} MHz", freq / 1_000_000);
        freq
    } else if let Some(freq) = calibrate_with_hpet() {
        freq
    } else {
        crate::kprintln!("tsc: calibrando via PIT...");
        let freq = calibrate_with_pit();
        crate::kprintln!("tsc: calibrado via PIT = {} MHz", freq / 1_000_000);
        freq
    };

    if freq_hz == 0 {
        crate::kprintln!("tsc: falha na calibração");
        return false;
    }

    TSC_FREQ_HZ.store(freq_hz, Ordering::Relaxed);
    TSC_FREQ_KHZ.store(freq_hz / 1000, Ordering::Relaxed);

    crate::kprintln!("tsc: inicializado @ {} Hz ({} MHz)",
                     freq_hz, freq_hz / 1_000_000);
    true
}

/// Get TSC frequency in Hz
pub fn frequency_hz() -> u64 {
    TSC_FREQ_HZ.load(Ordering::Relaxed)
}

/// Get TSC frequency in kHz
pub fn frequency_khz() -> u64 {
    TSC_FREQ_KHZ.load(Ordering::Relaxed)
}

/// Check if TSC is enabled
pub fn is_enabled() -> bool {
    TSC_AVAILABLE.load(Ordering::Relaxed) && TSC_FREQ_HZ.load(Ordering::Relaxed) != 0
}

/// Check if TSC has invariant frequency
pub fn has_invariant_tsc() -> bool {
    TSC_INVARIANT.load(Ordering::Relaxed)
}

/// Convert TSC ticks to nanoseconds
pub fn ticks_to_ns(ticks: u64) -> u64 {
    let freq_khz = TSC_FREQ_KHZ.load(Ordering::Relaxed);
    if freq_khz == 0 {
        return 0;
    }
    // ticks * 1_000_000 / freq_khz = ns
    // Use u128 to avoid overflow
    ((ticks as u128 * 1_000_000) / freq_khz as u128) as u64
}

/// Convert TSC ticks to microseconds
pub fn ticks_to_us(ticks: u64) -> u64 {
    let freq_khz = TSC_FREQ_KHZ.load(Ordering::Relaxed);
    if freq_khz == 0 {
        return 0;
    }
    // ticks * 1000 / freq_khz = us
    (ticks * 1000) / freq_khz
}

/// Convert TSC ticks to milliseconds
pub fn ticks_to_ms(ticks: u64) -> u64 {
    let freq_hz = TSC_FREQ_HZ.load(Ordering::Relaxed);
    if freq_hz == 0 {
        return 0;
    }
    // ticks * 1000 / freq_hz = ms
    (ticks * 1000) / freq_hz
}

/// Get current time in nanoseconds (since boot)
pub fn now_ns() -> u64 {
    ticks_to_ns(read())
}

/// Get current time in microseconds (since boot)
pub fn now_us() -> u64 {
    ticks_to_us(read())
}

/// Get current time in milliseconds (since boot)
pub fn now_ms() -> u64 {
    ticks_to_ms(read())
}

/// Busy-wait for a given number of CPU cycles
#[inline]
pub fn delay_cycles(cycles: u64) {
    let start = read();
    let target = start.wrapping_add(cycles);

    if target > start {
        while read() < target {
            core::hint::spin_loop();
        }
    } else {
        // Handle wrap-around
        while read() >= start {
            core::hint::spin_loop();
        }
        while read() < target {
            core::hint::spin_loop();
        }
    }
}

/// Busy-wait for a given number of nanoseconds
pub fn delay_ns(ns: u64) {
    let freq_khz = TSC_FREQ_KHZ.load(Ordering::Relaxed);
    if freq_khz == 0 {
        // Fallback
        for _ in 0..ns / 100 {
            core::hint::spin_loop();
        }
        return;
    }

    // cycles = ns * freq_khz / 1_000_000
    let cycles = (ns as u128 * freq_khz as u128) / 1_000_000;
    delay_cycles(cycles as u64);
}

/// Busy-wait for a given number of microseconds
pub fn delay_us(us: u64) {
    delay_ns(us * 1000);
}

/// Busy-wait for a given number of milliseconds
pub fn delay_ms(ms: u64) {
    delay_ns(ms * 1_000_000);
}

/// Measure the execution time of a closure in TSC ticks
pub fn measure_ticks<F: FnOnce()>(f: F) -> u64 {
    let start = read_serialized();
    f();
    let end = read_serialized();
    end.wrapping_sub(start)
}

/// Measure the execution time of a closure in nanoseconds
pub fn measure_ns<F: FnOnce()>(f: F) -> u64 {
    ticks_to_ns(measure_ticks(f))
}
