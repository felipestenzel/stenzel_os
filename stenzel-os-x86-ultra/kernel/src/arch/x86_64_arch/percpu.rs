//! Per-CPU Data Structures for SMP
//!
//! Each CPU has its own data area, accessed via the GS segment base.
//! This provides fast, lock-free access to CPU-local data.
//!
//! Memory layout:
//! - GS:0x00 = kernel_stack_top (used by syscall entry)
//! - GS:0x08 = user_rsp_tmp (used by syscall entry)
//! - GS:0x10 = cpu_id (APIC ID)
//! - GS:0x18 = self pointer
//! - GS:0x20 = current_task pointer
//! - GS:0x28 = preempt_count (preemption disable counter)
//! - GS:0x30 = irq_count (interrupt nesting level)
//! - GS:0x38 = in_interrupt flag

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, AtomicPtr, AtomicU32, AtomicU64, Ordering};
use alloc::boxed::Box;
use spin::Once;

use super::smp::MAX_CPUS;

/// Wrapper for *mut PerCpu that implements Send + Sync
/// This is safe because:
/// - Each CPU only accesses its own PerCpu area via GS base
/// - Cross-CPU access only happens through proper synchronization
#[derive(Clone, Copy)]
struct PerCpuPtr(*mut PerCpu);
unsafe impl Send for PerCpuPtr {}
unsafe impl Sync for PerCpuPtr {}

/// Wrapper for UnsafeCell<PerCpu> that implements Sync
/// This is safe because BSP_PERCPU is only accessed by the BSP
/// and cross-CPU access happens through proper synchronization
struct SyncPerCpu(UnsafeCell<PerCpu>);
unsafe impl Sync for SyncPerCpu {}

/// Per-CPU data structure
///
/// IMPORTANT: The first two fields MUST match the layout expected by
/// the syscall assembly code (syscall.rs):
/// - offset 0: kernel_stack_top
/// - offset 8: user_rsp_tmp
#[repr(C, align(64))] // Cache-line aligned to prevent false sharing
pub struct PerCpu {
    // === Fields used by syscall entry (DO NOT REORDER) ===
    /// Kernel stack top for current task (offset 0x00)
    pub kernel_stack_top: u64,
    /// Temporary storage for user RSP during syscall (offset 0x08)
    pub user_rsp_tmp: u64,

    // === Per-CPU identification ===
    /// CPU ID (APIC ID) (offset 0x10)
    pub cpu_id: u32,
    /// Padding for alignment
    _pad0: u32,
    /// Self pointer (for GS-relative access) (offset 0x18)
    pub self_ptr: *mut PerCpu,

    // === Current execution state ===
    /// Pointer to current task (offset 0x20)
    pub current_task: AtomicPtr<u8>,

    // === Preemption and interrupt state ===
    /// Preemption disable counter (offset 0x28)
    /// When > 0, preemption is disabled
    pub preempt_count: AtomicU32,
    /// Interrupt nesting level (offset 0x2C)
    /// Incremented on interrupt entry, decremented on exit
    pub irq_count: AtomicU32,
    /// Flag indicating we're in an interrupt context (offset 0x30)
    pub in_interrupt: AtomicBool,
    _pad1: [u8; 7],

    // === Statistics ===
    /// Total interrupts handled by this CPU
    pub interrupt_count: AtomicU64,
    /// Total context switches on this CPU
    pub context_switches: AtomicU64,
    /// Total syscalls handled by this CPU
    pub syscall_count: AtomicU64,

    // === Idle state ===
    /// Time spent idle (in TSC ticks)
    pub idle_ticks: AtomicU64,
    /// Whether this CPU is currently idle
    pub is_idle: AtomicBool,
    _pad2: [u8; 7],
}

impl PerCpu {
    /// Creates a new PerCpu structure for a given CPU
    pub const fn new(cpu_id: u32) -> Self {
        Self {
            kernel_stack_top: 0,
            user_rsp_tmp: 0,
            cpu_id,
            _pad0: 0,
            self_ptr: core::ptr::null_mut(),
            current_task: AtomicPtr::new(core::ptr::null_mut()),
            preempt_count: AtomicU32::new(0),
            irq_count: AtomicU32::new(0),
            in_interrupt: AtomicBool::new(false),
            _pad1: [0; 7],
            interrupt_count: AtomicU64::new(0),
            context_switches: AtomicU64::new(0),
            syscall_count: AtomicU64::new(0),
            idle_ticks: AtomicU64::new(0),
            is_idle: AtomicBool::new(false),
            _pad2: [0; 7],
        }
    }
}

/// Global array of per-CPU data pointers
/// Index is the CPU number (0..MAX_CPUS), not APIC ID
static PERCPU_AREAS: Once<[Option<PerCpuPtr>; MAX_CPUS]> = Once::new();

/// Mapping from APIC ID to CPU number
static APIC_TO_CPU: [AtomicU32; MAX_CPUS] = {
    const INIT: AtomicU32 = AtomicU32::new(u32::MAX);
    [INIT; MAX_CPUS]
};

/// Number of initialized CPUs
static NUM_CPUS: AtomicU32 = AtomicU32::new(0);

/// BSP's per-CPU data (statically allocated for bootstrap)
static BSP_PERCPU: SyncPerCpu = SyncPerCpu(UnsafeCell::new(PerCpu::new(0)));

/// Early initialization of per-CPU data for the BSP
/// This MUST be called before syscall::init() to set up GS base
/// for the syscall entry code.
pub fn early_init_bsp() {
    let bsp_percpu = unsafe { &mut *BSP_PERCPU.0.get() };
    bsp_percpu.self_ptr = bsp_percpu as *mut PerCpu;

    // Set GS base for BSP (before syscall init which depends on it)
    unsafe {
        set_gs_base(bsp_percpu as *mut PerCpu);
    }
}

/// Initialize per-CPU data for the BSP (Bootstrap Processor)
/// This is called later (after mm::init) to complete initialization
pub fn init_bsp() {
    let bsp_apic_id = super::apic::lapic_id() as u32;

    // Initialize BSP's per-CPU area
    let bsp_percpu = unsafe { &mut *BSP_PERCPU.0.get() };
    bsp_percpu.cpu_id = bsp_apic_id;
    bsp_percpu.self_ptr = bsp_percpu as *mut PerCpu;

    // Initialize the global array
    PERCPU_AREAS.call_once(|| {
        let mut areas: [Option<PerCpuPtr>; MAX_CPUS] = [None; MAX_CPUS];
        areas[0] = Some(PerCpuPtr(bsp_percpu as *mut PerCpu));
        areas
    });

    // Map APIC ID to CPU 0
    APIC_TO_CPU[0].store(bsp_apic_id, Ordering::Release);
    NUM_CPUS.store(1, Ordering::Release);

    // Set GS base for BSP
    unsafe {
        set_gs_base(bsp_percpu as *mut PerCpu);
    }

    crate::kprintln!("percpu: BSP (APIC {}) initialized at {:p}", bsp_apic_id, bsp_percpu);
}

/// Initialize per-CPU data for an AP (Application Processor)
/// Called during AP startup
pub fn init_ap(cpu_num: usize, apic_id: u32) -> *mut PerCpu {
    // Allocate per-CPU area for this AP
    let percpu = Box::leak(Box::new(PerCpu::new(apic_id)));
    percpu.self_ptr = percpu as *mut PerCpu;

    // Store in global array (need interior mutability pattern)
    // For now, we use the APIC_TO_CPU mapping
    APIC_TO_CPU[cpu_num].store(apic_id, Ordering::Release);
    NUM_CPUS.fetch_add(1, Ordering::AcqRel);

    // Set GS base for this AP
    unsafe {
        set_gs_base(percpu as *mut PerCpu);
    }

    crate::kprintln!("percpu: AP {} (APIC {}) initialized at {:p}", cpu_num, apic_id, percpu);

    percpu as *mut PerCpu
}

/// Set the GS base MSR to point to the per-CPU data
unsafe fn set_gs_base(percpu: *mut PerCpu) {
    use x86_64::registers::model_specific::Msr;

    const IA32_GS_BASE: u32 = 0xC0000101;
    const IA32_KERNEL_GS_BASE: u32 = 0xC0000102;

    // Set KERNEL_GS_BASE (used after swapgs in syscall entry)
    Msr::new(IA32_KERNEL_GS_BASE).write(percpu as u64);

    // Also set GS_BASE for kernel mode (when not in syscall)
    // This is swapped with KERNEL_GS_BASE by swapgs
    Msr::new(IA32_GS_BASE).write(percpu as u64);
}

/// Get the current CPU's per-CPU data pointer
///
/// This returns the BSP's per-CPU data. In a multi-CPU system,
/// this would use GS base to get the current CPU's data, but
/// for simplicity (and to avoid GS corruption issues during
/// context switches), we use the static BSP_PERCPU for now.
#[inline(always)]
pub fn current() -> &'static PerCpu {
    // For single-CPU or BSP, just return the static BSP_PERCPU
    // This avoids issues with GS base corruption during context switches
    unsafe { &*BSP_PERCPU.0.get() }
}

/// Get a mutable reference to current CPU's per-CPU data
///
/// # Safety
/// Caller must ensure exclusive access (e.g., interrupts disabled)
#[inline(always)]
pub unsafe fn current_mut() -> &'static mut PerCpu {
    let ptr: *mut PerCpu;
    // Read self_ptr from GS:0x18
    core::arch::asm!(
        "mov {}, gs:0x18",
        out(reg) ptr,
        options(nostack, preserves_flags)
    );
    &mut *ptr
}

/// Get the current CPU ID (APIC ID)
#[inline(always)]
pub fn cpu_id() -> u32 {
    unsafe {
        let id: u32;
        // Read cpu_id from GS:0x10
        core::arch::asm!(
            "mov {:e}, gs:0x10",
            out(reg) id,
            options(nostack, preserves_flags)
        );
        id
    }
}

/// Get the kernel stack top for current CPU
#[inline(always)]
pub fn kernel_stack_top() -> u64 {
    unsafe {
        let top: u64;
        core::arch::asm!(
            "mov {}, gs:0x00",
            out(reg) top,
            options(nostack, preserves_flags)
        );
        top
    }
}

/// Set the kernel stack top for current CPU
#[inline(always)]
pub unsafe fn set_kernel_stack_top(top: u64) {
    core::arch::asm!(
        "mov gs:0x00, {}",
        in(reg) top,
        options(nostack, preserves_flags)
    );
}

/// Disable preemption on the current CPU
#[inline]
pub fn preempt_disable() {
    current().preempt_count.fetch_add(1, Ordering::Relaxed);
    // Memory barrier to ensure the increment is visible
    core::sync::atomic::fence(Ordering::SeqCst);
}

/// Enable preemption on the current CPU
#[inline]
pub fn preempt_enable() {
    // Memory barrier before decrement
    core::sync::atomic::fence(Ordering::SeqCst);
    let old = current().preempt_count.fetch_sub(1, Ordering::Relaxed);
    debug_assert!(old > 0, "preempt_enable called with preempt_count already 0");
}

/// Check if preemption is enabled on the current CPU
#[inline]
pub fn preempt_enabled() -> bool {
    current().preempt_count.load(Ordering::Relaxed) == 0
}

/// Check if we're in an interrupt context
#[inline]
pub fn in_interrupt() -> bool {
    current().in_interrupt.load(Ordering::Relaxed)
}

/// Enter interrupt context (called at interrupt entry)
#[inline]
pub fn irq_enter() {
    let percpu = current();
    percpu.irq_count.fetch_add(1, Ordering::Relaxed);
    percpu.in_interrupt.store(true, Ordering::Release);
    percpu.interrupt_count.fetch_add(1, Ordering::Relaxed);
}

/// Exit interrupt context (called at interrupt exit)
#[inline]
pub fn irq_exit() {
    let percpu = current();
    let old = percpu.irq_count.fetch_sub(1, Ordering::Relaxed);
    if old == 1 {
        percpu.in_interrupt.store(false, Ordering::Release);
    }
}

/// Get the total number of initialized CPUs
pub fn num_cpus() -> u32 {
    NUM_CPUS.load(Ordering::Acquire)
}

/// Get per-CPU data for a specific CPU number
pub fn get_percpu(cpu_num: usize) -> Option<&'static PerCpu> {
    if cpu_num >= MAX_CPUS {
        return None;
    }

    // For BSP (cpu 0), use the static area
    if cpu_num == 0 {
        return Some(unsafe { &*BSP_PERCPU.0.get() });
    }

    // For APs, we need to look up by APIC ID
    // This is a simplified implementation - in production you'd want
    // to store the pointers in PERCPU_AREAS during init_ap
    None
}

/// Increment syscall counter
#[inline]
pub fn count_syscall() {
    current().syscall_count.fetch_add(1, Ordering::Relaxed);
}

/// Increment context switch counter
#[inline]
pub fn count_context_switch() {
    current().context_switches.fetch_add(1, Ordering::Relaxed);
}

/// Get CPU statistics
pub struct CpuStats {
    pub cpu_id: u32,
    pub interrupt_count: u64,
    pub context_switches: u64,
    pub syscall_count: u64,
    pub idle_ticks: u64,
}

/// Get statistics for the current CPU
pub fn current_stats() -> CpuStats {
    let percpu = current();
    CpuStats {
        cpu_id: percpu.cpu_id,
        interrupt_count: percpu.interrupt_count.load(Ordering::Relaxed),
        context_switches: percpu.context_switches.load(Ordering::Relaxed),
        syscall_count: percpu.syscall_count.load(Ordering::Relaxed),
        idle_ticks: percpu.idle_ticks.load(Ordering::Relaxed),
    }
}

// ============================================================================
// Legacy compatibility - these functions match the old syscall.rs interface
// ============================================================================

/// Get the CPU-local data pointer (legacy interface)
/// This returns a pointer compatible with the old CpuLocal struct
#[inline]
pub fn cpu_local_ptr() -> *mut PerCpu {
    // Return pointer to static BSP_PERCPU directly
    // This is safe for single-CPU systems and avoids GS issues
    unsafe { BSP_PERCPU.0.get() }
}

/// Set kernel stack top (legacy interface)
pub unsafe fn set_kernel_stack_top_legacy(top: u64) {
    (*cpu_local_ptr()).kernel_stack_top = top;
}
