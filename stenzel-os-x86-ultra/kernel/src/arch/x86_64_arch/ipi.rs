//! Inter-Processor Interrupts (IPI) for SMP
//!
//! This module provides high-level IPI functionality for communication
//! between CPUs in an SMP system.
//!
//! IPI Vectors (240-247):
//! - 240: Reschedule (trigger scheduler on another CPU)
//! - 241: TLB Shootdown (invalidate TLB entries)
//! - 242: Call Function (execute a function on another CPU)
//! - 243: Stop CPU (halt the target CPU)
//! - 244: Panic (stop all CPUs for kernel panic)
//! - 245-247: Reserved for future use

use core::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, AtomicUsize, Ordering};
use alloc::boxed::Box;

use super::apic;
use super::smp::MAX_CPUS;
use crate::sync::RwSpinlock;

/// IPI Vector definitions
pub const IPI_RESCHEDULE: u8 = 240;
pub const IPI_TLB_SHOOTDOWN: u8 = 241;
pub const IPI_CALL_FUNCTION: u8 = 242;
pub const IPI_STOP: u8 = 243;
pub const IPI_PANIC: u8 = 244;

/// Statistics for IPI
pub static IPI_RESCHEDULE_COUNT: AtomicU64 = AtomicU64::new(0);
pub static IPI_TLB_SHOOTDOWN_COUNT: AtomicU64 = AtomicU64::new(0);
pub static IPI_CALL_FUNCTION_COUNT: AtomicU64 = AtomicU64::new(0);

// ============================================================================
// TLB Shootdown State
// ============================================================================

/// TLB shootdown request state
struct TlbShootdownState {
    /// Address to invalidate (0 = flush all)
    address: AtomicU64,
    /// Number of pages to invalidate (0 = flush all)
    num_pages: AtomicUsize,
    /// Bitmask of CPUs that have acknowledged
    ack_mask: AtomicU64,
    /// Bitmask of target CPUs
    target_mask: AtomicU64,
    /// Whether a shootdown is in progress
    in_progress: AtomicBool,
}

impl TlbShootdownState {
    const fn new() -> Self {
        Self {
            address: AtomicU64::new(0),
            num_pages: AtomicUsize::new(0),
            ack_mask: AtomicU64::new(0),
            target_mask: AtomicU64::new(0),
            in_progress: AtomicBool::new(false),
        }
    }
}

static TLB_STATE: TlbShootdownState = TlbShootdownState::new();

// ============================================================================
// Call Function State
// ============================================================================

/// Function type for remote calls
pub type RemoteFunction = fn(u64) -> u64;

/// State for a pending remote function call
struct CallFunctionState {
    /// The function to call
    func: AtomicU64, // Actually *const fn(u64) -> u64
    /// Argument to pass
    arg: AtomicU64,
    /// Result from the function
    result: AtomicU64,
    /// Whether the call is complete
    complete: AtomicBool,
    /// Target CPU (u8::MAX = broadcast)
    target_cpu: AtomicU8,
}

impl CallFunctionState {
    const fn new() -> Self {
        Self {
            func: AtomicU64::new(0),
            arg: AtomicU64::new(0),
            result: AtomicU64::new(0),
            complete: AtomicBool::new(true),
            target_cpu: AtomicU8::new(u8::MAX),
        }
    }
}

static CALL_STATE: CallFunctionState = CallFunctionState::new();

// ============================================================================
// Stop/Panic State
// ============================================================================

/// Flag indicating CPUs should stop
static STOP_ALL_CPUS: AtomicBool = AtomicBool::new(false);

/// CPU stop spinlock to prevent concurrent stops
static STOP_LOCK: AtomicBool = AtomicBool::new(false);

// ============================================================================
// IPI Handlers
// ============================================================================

/// Handle reschedule IPI - triggers scheduler on this CPU
pub fn handle_reschedule() {
    IPI_RESCHEDULE_COUNT.fetch_add(1, Ordering::Relaxed);
    // Send EOI first
    apic::eoi();
    // The scheduler will be invoked on next timer tick or explicitly
    // For now, we just acknowledge the IPI
    // In a more complete implementation, we'd trigger a reschedule immediately
}

/// Handle TLB shootdown IPI - invalidates TLB entries
pub fn handle_tlb_shootdown() {
    IPI_TLB_SHOOTDOWN_COUNT.fetch_add(1, Ordering::Relaxed);

    let address = TLB_STATE.address.load(Ordering::Acquire);
    let num_pages = TLB_STATE.num_pages.load(Ordering::Acquire);

    if address == 0 && num_pages == 0 {
        // Flush entire TLB
        unsafe {
            // Write to CR3 to flush TLB (read and write back same value)
            let cr3: u64;
            core::arch::asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack));
            core::arch::asm!("mov cr3, {}", in(reg) cr3, options(nomem, nostack));
        }
    } else {
        // Invalidate specific pages
        for i in 0..num_pages {
            let page_addr = address + (i as u64 * 4096);
            unsafe {
                core::arch::asm!("invlpg [{}]", in(reg) page_addr, options(nostack));
            }
        }
    }

    // Mark this CPU as having completed the shootdown
    let cpu_id = apic::lapic_id();
    let cpu_bit = 1u64 << cpu_id;
    TLB_STATE.ack_mask.fetch_or(cpu_bit, Ordering::Release);

    apic::eoi();
}

/// Handle call function IPI - executes function on this CPU
pub fn handle_call_function() {
    IPI_CALL_FUNCTION_COUNT.fetch_add(1, Ordering::Relaxed);

    let func_ptr = CALL_STATE.func.load(Ordering::Acquire);
    let arg = CALL_STATE.arg.load(Ordering::Acquire);
    let target = CALL_STATE.target_cpu.load(Ordering::Acquire);
    let my_cpu = apic::lapic_id() as u8;

    // Check if this is for us (specific target or broadcast)
    if target == u8::MAX || target == my_cpu {
        if func_ptr != 0 {
            // Cast and call the function
            let func: RemoteFunction = unsafe { core::mem::transmute(func_ptr) };
            let result = func(arg);
            CALL_STATE.result.store(result, Ordering::Release);
        }
        CALL_STATE.complete.store(true, Ordering::Release);
    }

    apic::eoi();
}

/// Handle stop IPI - halts this CPU
pub fn handle_stop() {
    apic::eoi();

    // Disable interrupts and halt forever
    unsafe {
        core::arch::asm!("cli");
        loop {
            core::arch::asm!("hlt");
        }
    }
}

/// Handle panic IPI - emergency stop all CPUs
pub fn handle_panic() {
    apic::eoi();

    // Mark that we're stopping
    STOP_ALL_CPUS.store(true, Ordering::Release);

    // Disable interrupts and halt
    unsafe {
        core::arch::asm!("cli");
        loop {
            core::arch::asm!("hlt");
        }
    }
}

/// Main IPI dispatch function called from interrupt handler
pub fn handle_ipi(vector: u8) {
    match vector {
        IPI_RESCHEDULE => handle_reschedule(),
        IPI_TLB_SHOOTDOWN => handle_tlb_shootdown(),
        IPI_CALL_FUNCTION => handle_call_function(),
        IPI_STOP => handle_stop(),
        IPI_PANIC => handle_panic(),
        _ => {
            // Unknown IPI vector
            apic::eoi();
        }
    }
}

// ============================================================================
// High-Level IPI Functions
// ============================================================================

/// Send a reschedule IPI to a specific CPU
///
/// This causes the target CPU to run the scheduler.
#[inline]
pub fn send_reschedule(cpu_id: u8) {
    apic::send_ipi(cpu_id, IPI_RESCHEDULE);
}

/// Send a reschedule IPI to all other CPUs
#[inline]
pub fn send_reschedule_all() {
    apic::send_ipi_all_excluding_self(IPI_RESCHEDULE);
}

/// Perform a TLB shootdown on all CPUs
///
/// Invalidates TLB entries for the specified address range on all CPUs.
/// If `address` is 0 and `num_pages` is 0, flushes entire TLB.
///
/// This function blocks until all target CPUs have acknowledged.
pub fn tlb_shootdown(address: u64, num_pages: usize) {
    // Only one shootdown at a time
    while TLB_STATE.in_progress.compare_exchange(
        false, true,
        Ordering::Acquire, Ordering::Relaxed
    ).is_err() {
        core::hint::spin_loop();
    }

    // Set up shootdown parameters
    TLB_STATE.address.store(address, Ordering::Release);
    TLB_STATE.num_pages.store(num_pages, Ordering::Release);
    TLB_STATE.ack_mask.store(0, Ordering::Release);

    // Create target mask (all CPUs except self)
    let my_cpu = apic::lapic_id();
    let num_cpus = super::percpu::num_cpus() as u64;
    let mut target_mask: u64 = 0;
    for i in 0..num_cpus {
        if i as u32 != my_cpu {
            target_mask |= 1u64 << i;
        }
    }
    TLB_STATE.target_mask.store(target_mask, Ordering::Release);

    // Send IPI to all other CPUs
    apic::send_ipi_all_excluding_self(IPI_TLB_SHOOTDOWN);

    // Invalidate on local CPU as well
    if address == 0 && num_pages == 0 {
        unsafe {
            let cr3: u64;
            core::arch::asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack));
            core::arch::asm!("mov cr3, {}", in(reg) cr3, options(nomem, nostack));
        }
    } else {
        for i in 0..num_pages {
            let page_addr = address + (i as u64 * 4096);
            unsafe {
                core::arch::asm!("invlpg [{}]", in(reg) page_addr, options(nostack));
            }
        }
    }

    // Wait for all other CPUs to acknowledge
    while TLB_STATE.ack_mask.load(Ordering::Acquire) != target_mask {
        core::hint::spin_loop();
    }

    // Clean up
    TLB_STATE.in_progress.store(false, Ordering::Release);
}

/// Flush TLB on all CPUs
pub fn tlb_flush_all() {
    tlb_shootdown(0, 0);
}

/// Execute a function on a remote CPU and return the result
///
/// # Safety
/// The function must be safe to call on the target CPU.
pub fn call_function_on_cpu(cpu_id: u8, func: RemoteFunction, arg: u64) -> Option<u64> {
    // Wait for any previous call to complete
    while !CALL_STATE.complete.load(Ordering::Acquire) {
        core::hint::spin_loop();
    }

    // Set up the call
    CALL_STATE.func.store(func as u64, Ordering::Release);
    CALL_STATE.arg.store(arg, Ordering::Release);
    CALL_STATE.result.store(0, Ordering::Release);
    CALL_STATE.target_cpu.store(cpu_id, Ordering::Release);
    CALL_STATE.complete.store(false, Ordering::Release);

    // Send IPI
    apic::send_ipi(cpu_id, IPI_CALL_FUNCTION);

    // Wait for completion (with timeout)
    let mut timeout = 10_000_000u64;
    while !CALL_STATE.complete.load(Ordering::Acquire) {
        core::hint::spin_loop();
        timeout = timeout.saturating_sub(1);
        if timeout == 0 {
            return None; // Timeout
        }
    }

    Some(CALL_STATE.result.load(Ordering::Acquire))
}

/// Execute a function on all other CPUs (broadcast)
pub fn call_function_all(func: RemoteFunction, arg: u64) {
    while !CALL_STATE.complete.load(Ordering::Acquire) {
        core::hint::spin_loop();
    }

    CALL_STATE.func.store(func as u64, Ordering::Release);
    CALL_STATE.arg.store(arg, Ordering::Release);
    CALL_STATE.result.store(0, Ordering::Release);
    CALL_STATE.target_cpu.store(u8::MAX, Ordering::Release);
    CALL_STATE.complete.store(false, Ordering::Release);

    apic::send_ipi_all_excluding_self(IPI_CALL_FUNCTION);

    // Wait for completion
    let mut timeout = 10_000_000u64;
    while !CALL_STATE.complete.load(Ordering::Acquire) {
        core::hint::spin_loop();
        timeout = timeout.saturating_sub(1);
        if timeout == 0 {
            break;
        }
    }
}

/// Stop a specific CPU
pub fn stop_cpu(cpu_id: u8) {
    apic::send_ipi(cpu_id, IPI_STOP);
}

/// Wake a specific CPU (send NMI to wake from halt/sleep)
pub fn wake_cpu(cpu_id: u8) {
    // Send a reschedule IPI to wake the CPU
    // This will cause the CPU to exit from HLT instruction
    apic::send_ipi(cpu_id, IPI_RESCHEDULE);
}

/// Stop all other CPUs (emergency stop)
pub fn stop_all_cpus() {
    STOP_ALL_CPUS.store(true, Ordering::Release);
    apic::send_ipi_all_excluding_self(IPI_PANIC);
}

/// Check if CPUs should stop (for panic handling)
pub fn should_stop() -> bool {
    STOP_ALL_CPUS.load(Ordering::Acquire)
}

// ============================================================================
// Statistics
// ============================================================================

/// Get IPI statistics
pub struct IpiStats {
    pub reschedule_count: u64,
    pub tlb_shootdown_count: u64,
    pub call_function_count: u64,
}

pub fn get_stats() -> IpiStats {
    IpiStats {
        reschedule_count: IPI_RESCHEDULE_COUNT.load(Ordering::Relaxed),
        tlb_shootdown_count: IPI_TLB_SHOOTDOWN_COUNT.load(Ordering::Relaxed),
        call_function_count: IPI_CALL_FUNCTION_COUNT.load(Ordering::Relaxed),
    }
}

// ============================================================================
// Initialization
// ============================================================================

/// Initialize the IPI subsystem
///
/// This must be called after APIC initialization.
pub fn init() {
    // Reset statistics
    IPI_RESCHEDULE_COUNT.store(0, Ordering::Relaxed);
    IPI_TLB_SHOOTDOWN_COUNT.store(0, Ordering::Relaxed);
    IPI_CALL_FUNCTION_COUNT.store(0, Ordering::Relaxed);

    // Reset state
    STOP_ALL_CPUS.store(false, Ordering::Relaxed);
    TLB_STATE.in_progress.store(false, Ordering::Relaxed);
    CALL_STATE.complete.store(true, Ordering::Relaxed);

    crate::kprintln!("ipi: initialized (vectors {}-{})", IPI_RESCHEDULE, IPI_PANIC);
}
