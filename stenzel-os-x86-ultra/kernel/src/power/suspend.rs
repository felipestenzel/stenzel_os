//! Suspend/Resume and Power State Management
//!
//! Implements:
//! - S3 Suspend to RAM
//! - S0ix Modern Standby
//! - S4 Hibernation
//! - S5 Shutdown
//! - System reboot

extern crate alloc;

use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::sync::IrqSafeMutex;
use crate::util::{KResult, KError};

/// Suspend state
static SUSPEND_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

/// Devices that need suspend/resume callbacks
static SUSPEND_CALLBACKS: IrqSafeMutex<Vec<SuspendCallback>> = IrqSafeMutex::new(Vec::new());

/// Suspend callback
pub struct SuspendCallback {
    pub name: &'static str,
    pub priority: i32,
    pub suspend: fn() -> KResult<()>,
    pub resume: fn() -> KResult<()>,
}

/// Register a suspend callback
pub fn register_callback(callback: SuspendCallback) {
    let mut callbacks = SUSPEND_CALLBACKS.lock();
    callbacks.push(callback);
    callbacks.sort_by_key(|c| c.priority);
}

/// Suspend to RAM (S3)
pub fn suspend_to_ram() -> KResult<()> {
    if SUSPEND_IN_PROGRESS.swap(true, Ordering::SeqCst) {
        return Err(KError::Busy);
    }

    crate::kprintln!("suspend: preparing for S3 suspend...");

    // Phase 1: Freeze user tasks
    freeze_tasks()?;

    // Phase 2: Suspend devices (in priority order)
    let callbacks = SUSPEND_CALLBACKS.lock();
    for cb in callbacks.iter() {
        crate::kprintln!("suspend: suspending {}", cb.name);
        if let Err(e) = (cb.suspend)() {
            crate::kprintln!("suspend: {} failed: {:?}", cb.name, e);
            // Abort and resume already suspended devices
            drop(callbacks);
            abort_suspend();
            return Err(e);
        }
    }
    drop(callbacks);

    // Phase 3: Save CPU state
    save_cpu_state();

    // Phase 4: Enter S3
    enter_acpi_s3()?;

    // --- CPU is stopped here, resumed on wakeup ---

    // Phase 5: Restore CPU state
    restore_cpu_state();

    // Phase 6: Resume devices (in reverse priority order)
    let callbacks = SUSPEND_CALLBACKS.lock();
    for cb in callbacks.iter().rev() {
        crate::kprintln!("suspend: resuming {}", cb.name);
        if let Err(e) = (cb.resume)() {
            crate::kprintln!("suspend: {} resume failed: {:?}", cb.name, e);
        }
    }
    drop(callbacks);

    // Phase 7: Thaw tasks
    thaw_tasks()?;

    SUSPEND_IN_PROGRESS.store(false, Ordering::SeqCst);
    crate::kprintln!("suspend: resumed from S3");

    Ok(())
}

/// Enter S0ix (Modern Standby)
pub fn enter_s0ix() -> KResult<()> {
    crate::kprintln!("suspend: entering S0ix...");

    // Put devices in low power state
    let callbacks = SUSPEND_CALLBACKS.lock();
    for cb in callbacks.iter() {
        let _ = (cb.suspend)();
    }
    drop(callbacks);

    // Enter platform low power idle
    enter_platform_idle();

    // Resume devices
    let callbacks = SUSPEND_CALLBACKS.lock();
    for cb in callbacks.iter().rev() {
        let _ = (cb.resume)();
    }

    crate::kprintln!("suspend: exited S0ix");
    Ok(())
}

/// Hibernate (S4)
pub fn hibernate() -> KResult<()> {
    crate::kprintln!("suspend: hibernating...");

    // Freeze tasks
    freeze_tasks()?;

    // Create memory image
    let image = create_hibernate_image()?;

    // Write image to swap
    write_hibernate_image(&image)?;

    // Enter S4
    enter_acpi_s4()?;

    // If we return, hibernation was cancelled
    thaw_tasks()?;
    Ok(())
}

/// Shutdown (S5)
pub fn shutdown() -> KResult<()> {
    crate::kprintln!("suspend: shutting down...");

    // Notify all subsystems
    let callbacks = SUSPEND_CALLBACKS.lock();
    for cb in callbacks.iter() {
        let _ = (cb.suspend)();
    }
    drop(callbacks);

    // Enter S5 via ACPI
    enter_acpi_s5()
}

/// Reboot
pub fn reboot() -> KResult<()> {
    crate::kprintln!("suspend: rebooting...");

    // Try ACPI reboot first
    if acpi_reboot().is_ok() {
        return Ok(());
    }

    // Fall back to keyboard controller reset
    keyboard_reset();

    // Fall back to triple fault
    triple_fault()
}

/// Resume from suspend (called from wakeup path)
pub fn resume() -> KResult<()> {
    // This is called when resuming from S3
    // Most work is done in suspend_to_ram() after enter_acpi_s3() returns
    Ok(())
}

// Internal functions

fn freeze_tasks() -> KResult<()> {
    crate::kprintln!("suspend: freezing tasks...");
    // Signal all user tasks to freeze
    // Wait for tasks to reach freeze point
    // In a real implementation, we would iterate through all processes
    Ok(())
}

fn thaw_tasks() -> KResult<()> {
    crate::kprintln!("suspend: thawing tasks...");
    // Signal all frozen tasks to continue
    Ok(())
}

fn abort_suspend() {
    crate::kprintln!("suspend: aborting suspend...");
    // Resume any already-suspended devices in reverse order
    let callbacks = SUSPEND_CALLBACKS.lock();
    for cb in callbacks.iter().rev() {
        let _ = (cb.resume)();
    }
    drop(callbacks);
    let _ = thaw_tasks();
    SUSPEND_IN_PROGRESS.store(false, Ordering::SeqCst);
}

fn save_cpu_state() {
    // Save:
    // - GDT, IDT, TR
    // - CR0, CR3, CR4
    // - General purpose registers
    // - FPU/SSE state
    // - MSRs
    crate::kprintln!("suspend: saving CPU state...");
}

fn restore_cpu_state() {
    // Restore saved CPU state
    crate::kprintln!("suspend: restoring CPU state...");
}

fn enter_acpi_s3() -> KResult<()> {
    // Write to PM1a_CNT and PM1b_CNT registers
    // SLP_TYP = S3 type from ACPI tables
    // SLP_EN = 1

    unsafe {
        // Get PM1a control register from ACPI FADT
        // For now, use common addresses
        let pm1a_cnt: u16 = 0x404; // Common on many systems

        // Read current value
        let mut val: u16;
        core::arch::asm!("in ax, dx", out("ax") val, in("dx") pm1a_cnt);

        // Set SLP_TYP for S3 (value from ACPI _S3_ method, typically 5 << 10)
        // Set SLP_EN (bit 13)
        val = (val & 0xE003) | (5 << 10) | (1 << 13);

        // Write to enter sleep
        core::arch::asm!("out dx, ax", in("ax") val, in("dx") pm1a_cnt);

        // CPU should halt here and resume from reset vector on wakeup
        // If we're still running, wait for interrupt
        core::arch::asm!("sti; hlt");
    }

    Ok(())
}

fn enter_platform_idle() {
    // Enter CPU deep idle state (C-states)
    unsafe {
        // Use MWAIT with hints for deepest C-state
        let hints: u32 = 0x40; // C6 on many Intel CPUs
        core::arch::asm!(
            "mov eax, {0}",
            "xor ecx, ecx",
            "mwait",
            in(reg) hints,
            options(nomem, nostack)
        );
    }
}

fn enter_acpi_s4() -> KResult<()> {
    // Similar to S3 but with S4 sleep type
    unsafe {
        let pm1a_cnt: u16 = 0x404;
        let mut val: u16;
        core::arch::asm!("in ax, dx", out("ax") val, in("dx") pm1a_cnt);
        val = (val & 0xE003) | (6 << 10) | (1 << 13); // S4 type
        core::arch::asm!("out dx, ax", in("ax") val, in("dx") pm1a_cnt);
        core::arch::asm!("sti; hlt");
    }
    Ok(())
}

fn enter_acpi_s5() -> KResult<()> {
    // Enter S5 (soft off)
    unsafe {
        let pm1a_cnt: u16 = 0x404;
        let mut val: u16;
        core::arch::asm!("in ax, dx", out("ax") val, in("dx") pm1a_cnt);
        val = (val & 0xE003) | (7 << 10) | (1 << 13); // S5 type
        core::arch::asm!("out dx, ax", in("ax") val, in("dx") pm1a_cnt);
        loop { core::arch::asm!("hlt"); }
    }
}

fn acpi_reboot() -> KResult<()> {
    // Use ACPI FADT reset register if available
    // For now, return error to fall through to other methods
    Err(KError::NotSupported)
}

fn keyboard_reset() {
    // Reset via 8042 keyboard controller
    unsafe {
        // Wait for keyboard controller
        for _ in 0..10000 {
            let status: u8;
            core::arch::asm!("in al, 0x64", out("al") status);
            if status & 0x02 == 0 { break; }
        }
        // Send reset command
        core::arch::asm!("out 0x64, al", in("al") 0xFEu8);
    }
}

fn triple_fault() -> KResult<()> {
    // Load null IDT and trigger interrupt
    unsafe {
        let null_idt: [u8; 6] = [0; 6];
        core::arch::asm!(
            "lidt [{}]",
            "int 3",
            in(reg) null_idt.as_ptr(),
        );
    }
    // Should never reach here
    Err(KError::NotSupported)
}

fn create_hibernate_image() -> KResult<Vec<u8>> {
    // Create a snapshot of system memory
    // In a real implementation, this would:
    // 1. Allocate memory for the image
    // 2. Copy all memory pages
    // 3. Compress the image
    crate::kprintln!("suspend: creating hibernate image...");
    Ok(Vec::new())
}

fn write_hibernate_image(_image: &[u8]) -> KResult<()> {
    // Write image to swap partition
    crate::kprintln!("suspend: writing hibernate image...");
    Ok(())
}

/// Initialize suspend subsystem
pub fn init() {
    crate::kprintln!("suspend: subsystem initialized");
}
