//! HPET (High Precision Event Timer) Driver
//!
//! Provides high-resolution timing capabilities for the kernel.
//! HPET typically runs at frequencies of 10MHz or higher.

#![allow(dead_code)]

use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::drivers::acpi;

// HPET Register Offsets
const HPET_CAP_ID: u64 = 0x000;          // General Capabilities and ID
const HPET_CONFIG: u64 = 0x010;          // General Configuration
const HPET_INT_STATUS: u64 = 0x020;      // General Interrupt Status
const HPET_MAIN_COUNTER: u64 = 0x0F0;    // Main Counter Value

// Timer N registers (N = 0..31)
const fn timer_config_cap(n: u64) -> u64 {
    0x100 + 0x20 * n
}
const fn timer_comparator(n: u64) -> u64 {
    0x108 + 0x20 * n
}
const fn timer_fsb_int_route(n: u64) -> u64 {
    0x110 + 0x20 * n
}

// Configuration bits
const HPET_CFG_ENABLE: u64 = 1 << 0;         // Enable main counter
const HPET_CFG_LEGACY_RT: u64 = 1 << 1;      // Legacy Replacement Route

// Timer configuration bits
const TIMER_CFG_INT_TYPE_LEVEL: u64 = 1 << 1;   // Level-triggered interrupts
const TIMER_CFG_INT_ENABLE: u64 = 1 << 2;       // Enable interrupt
const TIMER_CFG_PERIODIC: u64 = 1 << 3;         // Periodic mode
const TIMER_CFG_PERIODIC_CAP: u64 = 1 << 4;     // Periodic capable
const TIMER_CFG_SIZE_64: u64 = 1 << 5;          // 64-bit counter capable
const TIMER_CFG_SET_VALUE: u64 = 1 << 6;        // Set comparator value
const TIMER_CFG_32BIT_MODE: u64 = 1 << 8;       // Force 32-bit mode

/// HPET state
static HPET_ENABLED: AtomicBool = AtomicBool::new(false);
static HPET_BASE: AtomicU64 = AtomicU64::new(0);
static HPET_PERIOD_FS: AtomicU64 = AtomicU64::new(0);  // Period in femtoseconds
static HPET_FREQ_HZ: AtomicU64 = AtomicU64::new(0);    // Frequency in Hz

/// Convert HPET physical address to virtual address
fn hpet_phys_to_virt(phys: u64) -> u64 {
    crate::mm::phys_to_virt(x86_64::PhysAddr::new(phys)).as_u64()
}

/// Read a 64-bit HPET register
unsafe fn hpet_read(offset: u64) -> u64 {
    let base = HPET_BASE.load(Ordering::Relaxed);
    if base == 0 {
        return 0;
    }
    let virt = hpet_phys_to_virt(base + offset);
    read_volatile(virt as *const u64)
}

/// Write a 64-bit HPET register
unsafe fn hpet_write(offset: u64, value: u64) {
    let base = HPET_BASE.load(Ordering::Relaxed);
    if base == 0 {
        return;
    }
    let virt = hpet_phys_to_virt(base + offset);
    write_volatile(virt as *mut u64, value);
}

/// Initialize the HPET
pub fn init() -> bool {
    // Get HPET info from ACPI
    let hpet_info = match acpi::parse_hpet() {
        Some(info) => info,
        None => {
            crate::kprintln!("hpet: tabela ACPI não encontrada");
            return false;
        }
    };

    crate::kprintln!("hpet: encontrado @ {:#x}", hpet_info.base_address);
    crate::kprintln!("hpet: {} comparadores, {}bit counter, vendor={:#x}",
                     hpet_info.num_comparators,
                     if hpet_info.counter_size_64 { 64 } else { 32 },
                     hpet_info.vendor_id);

    HPET_BASE.store(hpet_info.base_address, Ordering::Relaxed);

    unsafe {
        // Read capabilities
        let cap = hpet_read(HPET_CAP_ID);
        let period_fs = cap >> 32;  // Period in femtoseconds (10^-15 seconds)
        let num_timers = ((cap >> 8) & 0x1F) + 1;
        let counter_64bit = (cap & (1 << 13)) != 0;
        let legacy_capable = (cap & (1 << 15)) != 0;

        HPET_PERIOD_FS.store(period_fs, Ordering::Relaxed);

        // Calculate frequency: freq = 10^15 / period_fs
        let freq_hz = 1_000_000_000_000_000u64 / period_fs;
        HPET_FREQ_HZ.store(freq_hz, Ordering::Relaxed);

        crate::kprintln!("hpet: period={}fs, freq={}Hz ({} MHz)",
                         period_fs, freq_hz, freq_hz / 1_000_000);
        crate::kprintln!("hpet: {} timers, {}bit, legacy={}",
                         num_timers, if counter_64bit { 64 } else { 32 }, legacy_capable);

        // Stop the counter first
        let config = hpet_read(HPET_CONFIG);
        hpet_write(HPET_CONFIG, config & !HPET_CFG_ENABLE);

        // Reset main counter to 0
        hpet_write(HPET_MAIN_COUNTER, 0);

        // Disable all timer interrupts
        for i in 0..num_timers {
            let timer_cfg = hpet_read(timer_config_cap(i));
            hpet_write(timer_config_cap(i), timer_cfg & !TIMER_CFG_INT_ENABLE);
        }

        // Clear any pending interrupts
        hpet_write(HPET_INT_STATUS, 0xFFFFFFFF);

        // Enable the counter (without legacy replacement to not interfere with APIC)
        hpet_write(HPET_CONFIG, HPET_CFG_ENABLE);
    }

    HPET_ENABLED.store(true, Ordering::Relaxed);
    crate::kprintln!("hpet: inicializado e rodando");
    true
}

/// Check if HPET is enabled
pub fn is_enabled() -> bool {
    HPET_ENABLED.load(Ordering::Relaxed)
}

/// Get the HPET frequency in Hz
pub fn frequency() -> u64 {
    HPET_FREQ_HZ.load(Ordering::Relaxed)
}

/// Get the current HPET counter value
pub fn read_counter() -> u64 {
    if !HPET_ENABLED.load(Ordering::Relaxed) {
        return 0;
    }
    unsafe { hpet_read(HPET_MAIN_COUNTER) }
}

/// Get the current time in nanoseconds since HPET was enabled
pub fn nanoseconds() -> u64 {
    let counter = read_counter();
    let period_fs = HPET_PERIOD_FS.load(Ordering::Relaxed);

    if period_fs == 0 {
        return 0;
    }

    // Convert to nanoseconds: counter * period_fs / 10^6 (fs -> ns)
    // Use u128 to avoid overflow
    let ns = (counter as u128 * period_fs as u128) / 1_000_000;
    ns as u64
}

/// Get the current time in microseconds since HPET was enabled
pub fn microseconds() -> u64 {
    nanoseconds() / 1000
}

/// Get the current time in milliseconds since HPET was enabled
pub fn milliseconds() -> u64 {
    nanoseconds() / 1_000_000
}

/// Sleep for a given number of nanoseconds using busy-wait
pub fn sleep_ns(ns: u64) {
    if !HPET_ENABLED.load(Ordering::Relaxed) {
        // Fallback to busy loop
        for _ in 0..ns / 100 {
            core::hint::spin_loop();
        }
        return;
    }

    let period_fs = HPET_PERIOD_FS.load(Ordering::Relaxed);
    if period_fs == 0 {
        return;
    }

    // Calculate ticks to wait: ns * 10^6 / period_fs
    let ticks = (ns as u128 * 1_000_000) / period_fs as u128;
    let start = read_counter();
    let target = start.wrapping_add(ticks as u64);

    // Handle counter wrap-around
    if target > start {
        while read_counter() < target {
            core::hint::spin_loop();
        }
    } else {
        // Counter will wrap
        while read_counter() >= start {
            core::hint::spin_loop();
        }
        while read_counter() < target {
            core::hint::spin_loop();
        }
    }
}

/// Sleep for a given number of microseconds using busy-wait
pub fn sleep_us(us: u64) {
    sleep_ns(us * 1000);
}

/// Sleep for a given number of milliseconds using busy-wait
pub fn sleep_ms(ms: u64) {
    sleep_ns(ms * 1_000_000);
}

/// Configure timer 0 for periodic interrupts (used for APIC timer calibration alternative)
pub fn setup_periodic_timer(frequency_hz: u32, vector: u8) -> bool {
    if !HPET_ENABLED.load(Ordering::Relaxed) {
        return false;
    }

    let hpet_freq = HPET_FREQ_HZ.load(Ordering::Relaxed);
    if hpet_freq == 0 {
        return false;
    }

    let comparator_value = hpet_freq / frequency_hz as u64;

    unsafe {
        // Read timer 0 capabilities
        let cap = hpet_read(timer_config_cap(0));
        let periodic_capable = (cap & TIMER_CFG_PERIODIC_CAP) != 0;

        if !periodic_capable {
            crate::kprintln!("hpet: timer 0 não suporta modo periódico");
            return false;
        }

        // Get available interrupt routing
        let int_route_cap = (cap >> 32) as u32;
        let mut selected_irq = 0u8;
        for i in 0..32 {
            if (int_route_cap & (1 << i)) != 0 {
                selected_irq = i;
                break;
            }
        }

        // Stop the counter
        let config = hpet_read(HPET_CONFIG);
        hpet_write(HPET_CONFIG, config & !HPET_CFG_ENABLE);

        // Configure timer 0 for periodic mode
        let timer_config = TIMER_CFG_INT_ENABLE |
                          TIMER_CFG_PERIODIC |
                          TIMER_CFG_SET_VALUE |
                          ((selected_irq as u64) << 9);  // IRQ routing
        hpet_write(timer_config_cap(0), timer_config);

        // Set comparator value
        hpet_write(timer_comparator(0), comparator_value);

        // Reset and restart counter
        hpet_write(HPET_MAIN_COUNTER, 0);
        hpet_write(HPET_CONFIG, config | HPET_CFG_ENABLE);

        crate::kprintln!("hpet: timer 0 configurado para {}Hz (comparator={})",
                         frequency_hz, comparator_value);
    }

    true
}

/// Disable timer 0 periodic interrupts
pub fn disable_timer() {
    if !HPET_ENABLED.load(Ordering::Relaxed) {
        return;
    }

    unsafe {
        let timer_config = hpet_read(timer_config_cap(0));
        hpet_write(timer_config_cap(0), timer_config & !TIMER_CFG_INT_ENABLE);
    }
}
