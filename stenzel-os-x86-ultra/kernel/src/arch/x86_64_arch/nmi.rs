//! Non-Maskable Interrupt (NMI) Handling
//!
//! NMIs are critical hardware interrupts that cannot be disabled via CLI.
//! They are used for:
//! - Hardware memory errors (ECC/parity)
//! - Watchdog timeouts
//! - System board errors
//! - Performance counter overflow (profiling)
//! - Cross-CPU panic notification
//! - Debug purposes

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use x86_64::structures::idt::InterruptStackFrame;

/// NMI source/reason
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NmiReason {
    /// Memory parity error (from system control port B)
    MemoryParity,
    /// I/O channel check (from system control port B)
    IoCheck,
    /// Watchdog timeout
    Watchdog,
    /// Performance counter overflow
    PerformanceCounter,
    /// Unknown source
    Unknown,
}

// System control port addresses (x86 standard)
const SYSTEM_CONTROL_PORT_A: u16 = 0x92;
const SYSTEM_CONTROL_PORT_B: u16 = 0x61;

// System control port B bits
const SCB_MEMORY_PARITY_ERR: u8 = 1 << 7;  // Memory parity error occurred
const SCB_IO_CHANNEL_CHECK: u8 = 1 << 6;   // I/O channel check occurred
const SCB_TIMER2_OUTPUT: u8 = 1 << 5;      // Timer 2 output status
const SCB_REFRESH_REQUEST: u8 = 1 << 4;    // Refresh request
const SCB_IO_CHANNEL_CHECK_ENABLE: u8 = 1 << 3;
const SCB_MEMORY_PARITY_ENABLE: u8 = 1 << 2;
const SCB_SPEAKER_DATA: u8 = 1 << 1;
const SCB_TIMER2_GATE: u8 = 1 << 0;

/// Flag indicating a panic NMI was sent
static PANIC_NMI_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Counter for NMI events
static NMI_COUNT: AtomicU64 = AtomicU64::new(0);

/// Counter for memory errors
static MEMORY_ERROR_COUNT: AtomicU64 = AtomicU64::new(0);

/// Counter for I/O errors
static IO_ERROR_COUNT: AtomicU64 = AtomicU64::new(0);

/// Counter for watchdog timeouts
static WATCHDOG_COUNT: AtomicU64 = AtomicU64::new(0);

/// Counter for performance counter overflows
static PERF_COUNTER_COUNT: AtomicU64 = AtomicU64::new(0);

/// Read system control port B
fn read_system_control_b() -> u8 {
    unsafe { x86_64::instructions::port::Port::<u8>::new(SYSTEM_CONTROL_PORT_B).read() }
}

/// Write system control port B
fn write_system_control_b(value: u8) {
    unsafe {
        x86_64::instructions::port::Port::<u8>::new(SYSTEM_CONTROL_PORT_B).write(value);
    }
}

/// Determine the reason for the NMI
pub fn get_nmi_reason() -> NmiReason {
    NMI_COUNT.fetch_add(1, Ordering::SeqCst);

    // Read system control port B to determine source
    let status = read_system_control_b();

    if status & SCB_MEMORY_PARITY_ERR != 0 {
        MEMORY_ERROR_COUNT.fetch_add(1, Ordering::SeqCst);
        return NmiReason::MemoryParity;
    }

    if status & SCB_IO_CHANNEL_CHECK != 0 {
        IO_ERROR_COUNT.fetch_add(1, Ordering::SeqCst);
        return NmiReason::IoCheck;
    }

    // Check if it's from performance monitoring (LAPIC)
    if is_perf_counter_nmi() {
        PERF_COUNTER_COUNT.fetch_add(1, Ordering::SeqCst);
        return NmiReason::PerformanceCounter;
    }

    // Check for watchdog (platform-specific)
    if is_watchdog_nmi() {
        WATCHDOG_COUNT.fetch_add(1, Ordering::SeqCst);
        return NmiReason::Watchdog;
    }

    NmiReason::Unknown
}

/// Check if this is a panic NMI from another CPU
pub fn is_panic_nmi() -> bool {
    PANIC_NMI_ACTIVE.load(Ordering::SeqCst)
}

/// Set panic NMI flag (called before sending NMI to other CPUs)
pub fn set_panic_nmi() {
    PANIC_NMI_ACTIVE.store(true, Ordering::SeqCst);
}

/// Clear panic NMI flag
pub fn clear_panic_nmi() {
    PANIC_NMI_ACTIVE.store(false, Ordering::SeqCst);
}

/// Handle panic NMI (received from another CPU during panic)
pub fn handle_panic_nmi(_stack_frame: InterruptStackFrame) -> ! {
    crate::kprintln!("NMI: Panic from another CPU - halting");

    // Disable interrupts and halt
    loop {
        x86_64::instructions::interrupts::disable();
        x86_64::instructions::hlt();
    }
}

/// Handle memory error (ECC/parity)
pub fn handle_memory_error(stack_frame: &InterruptStackFrame) {
    crate::kprintln!("NMI: Memory error at RIP={:#x}", stack_frame.instruction_pointer.as_u64());

    // Clear the error by toggling the enable bit
    let status = read_system_control_b();
    // Disable memory parity checking
    write_system_control_b(status & !SCB_MEMORY_PARITY_ENABLE);
    // Re-enable memory parity checking
    write_system_control_b(status | SCB_MEMORY_PARITY_ENABLE);

    // Log for diagnostics
    crate::kprintln!("NMI: Memory error cleared (total: {})", MEMORY_ERROR_COUNT.load(Ordering::SeqCst));

    // In a production system, we might want to:
    // - Log to a persistent error log
    // - Mark memory pages as bad
    // - Notify monitoring systems
    // - Potentially panic if errors are frequent
}

/// Handle I/O error
pub fn handle_io_error(stack_frame: &InterruptStackFrame) {
    crate::kprintln!("NMI: I/O channel check error at RIP={:#x}", stack_frame.instruction_pointer.as_u64());

    // Clear the error by toggling the enable bit
    let status = read_system_control_b();
    // Disable I/O channel check
    write_system_control_b(status & !SCB_IO_CHANNEL_CHECK_ENABLE);
    // Re-enable I/O channel check
    write_system_control_b(status | SCB_IO_CHANNEL_CHECK_ENABLE);

    crate::kprintln!("NMI: I/O error cleared (total: {})", IO_ERROR_COUNT.load(Ordering::SeqCst));
}

/// Handle watchdog timeout
pub fn handle_watchdog(stack_frame: &InterruptStackFrame) {
    crate::kprintln!("NMI: Watchdog timeout at RIP={:#x}", stack_frame.instruction_pointer.as_u64());
    crate::kprintln!("NMI: System may be unresponsive");

    // In a production system, we might want to:
    // - Dump register state
    // - Dump stack trace
    // - Reset watchdog timer
    // - Potentially trigger a controlled reboot
}

/// Handle performance counter overflow (for profiling)
pub fn handle_perf_counter() {
    // This is used for CPU profiling
    // The overflow triggers a sample to be recorded

    // Clear the overflow condition in the LAPIC
    clear_perf_counter_overflow();

    // In a full implementation, we would:
    // - Record the current instruction pointer
    // - Record the current stack trace
    // - Store the sample for later analysis
}

/// Check if NMI is from performance counter
fn is_perf_counter_nmi() -> bool {
    // Check LAPIC LVT Performance Counter entry
    // If the delivery mode is NMI and the mask bit is clear, it could be from perf counter

    // Read APIC base
    let apic_base = unsafe {
        x86_64::registers::model_specific::Msr::new(0x1B).read() & 0xFFFFF000
    };

    if apic_base == 0 {
        return false;
    }

    // Check LVT Performance Counter (offset 0x340)
    let lvt_perf = unsafe {
        let addr = crate::mm::phys_to_virt(x86_64::PhysAddr::new(apic_base + 0x340));
        *(addr.as_ptr::<u32>())
    };

    // Check if delivery mode is NMI (bits 10:8 = 4) and not masked
    let delivery_mode = (lvt_perf >> 8) & 0x7;
    let masked = (lvt_perf >> 16) & 0x1;

    delivery_mode == 4 && masked == 0
}

/// Check if NMI is from watchdog
fn is_watchdog_nmi() -> bool {
    // Platform-specific watchdog detection
    // This would check platform-specific registers

    // For now, return false - specific platforms need their own detection
    false
}

/// Clear performance counter overflow condition
fn clear_perf_counter_overflow() {
    // Clear overflow status in IA32_PERF_GLOBAL_STATUS (MSR 0x38E)
    // by writing to IA32_PERF_GLOBAL_OVF_CTRL (MSR 0x390)

    unsafe {
        // Read current global status
        let _status = x86_64::registers::model_specific::Msr::new(0x38E).read();

        // Write to overflow control to clear (write 1s to clear)
        x86_64::registers::model_specific::Msr::new(0x390).write(0xFFFFFFFF_FFFFFFFF);
    }
}

/// Re-enable NMI after handling
/// NMI is automatically blocked until IRET, but some cases need manual re-enable
pub fn reenable_nmi() {
    // On x86, NMI is automatically re-enabled by IRET
    // However, we can manually re-enable by reading/writing to port 0x61

    let status = read_system_control_b();
    // Toggle memory parity and I/O channel check bits to acknowledge
    write_system_control_b(status | SCB_MEMORY_PARITY_ENABLE | SCB_IO_CHANNEL_CHECK_ENABLE);
}

/// Disable NMI (not recommended except for critical sections)
pub fn disable_nmi() {
    // Disable by setting bit 7 of port 0x70 (CMOS address register)
    unsafe {
        let mut port = x86_64::instructions::port::Port::<u8>::new(0x70);
        let val = port.read();
        port.write(val | 0x80);
    }
}

/// Enable NMI
pub fn enable_nmi() {
    // Enable by clearing bit 7 of port 0x70 (CMOS address register)
    unsafe {
        let mut port = x86_64::instructions::port::Port::<u8>::new(0x70);
        let val = port.read();
        port.write(val & 0x7F);
    }
}

/// Get NMI statistics
pub fn get_nmi_stats() -> NmiStats {
    NmiStats {
        total: NMI_COUNT.load(Ordering::SeqCst),
        memory_errors: MEMORY_ERROR_COUNT.load(Ordering::SeqCst),
        io_errors: IO_ERROR_COUNT.load(Ordering::SeqCst),
        watchdog: WATCHDOG_COUNT.load(Ordering::SeqCst),
        perf_counter: PERF_COUNTER_COUNT.load(Ordering::SeqCst),
    }
}

/// NMI statistics
#[derive(Debug, Clone, Copy)]
pub struct NmiStats {
    pub total: u64,
    pub memory_errors: u64,
    pub io_errors: u64,
    pub watchdog: u64,
    pub perf_counter: u64,
}

/// Initialize NMI handling
pub fn init() {
    crate::kprintln!("nmi: initializing NMI handling");

    // Enable NMI
    enable_nmi();

    // Enable memory parity and I/O channel check NMIs
    let status = read_system_control_b();
    write_system_control_b(status | SCB_MEMORY_PARITY_ENABLE | SCB_IO_CHANNEL_CHECK_ENABLE);

    crate::kprintln!("nmi: NMI handling initialized");
}

/// Send NMI to other CPUs (for panic)
pub fn send_nmi_to_all_cpus() {
    // Set flag before sending
    set_panic_nmi();

    // Use APIC to send NMI IPI to all other CPUs
    if super::apic::is_enabled() {
        super::apic::send_nmi_all_excluding_self();
    }
}
