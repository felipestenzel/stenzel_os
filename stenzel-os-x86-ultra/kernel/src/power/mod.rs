//! Power Management
//!
//! Power state management including suspend, hibernate, and power off.

#![allow(dead_code)]

pub mod hibernate;

/// Power states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerState {
    /// Normal operation
    Running,
    /// Suspend to RAM (S3)
    Suspended,
    /// Hibernate to disk (S4)
    Hibernated,
    /// Power off (S5)
    PowerOff,
}

/// Initialize power management subsystem
pub fn init() {
    hibernate::init();
    crate::kprintln!("power: subsystem initialized");
}

/// Initiate system shutdown
pub fn shutdown() -> ! {
    crate::kprintln!("power: initiating shutdown...");

    // Try ACPI shutdown
    #[cfg(target_arch = "x86_64")]
    unsafe {
        // QEMU shutdown port
        core::arch::asm!(
            "out dx, ax",
            in("dx") 0x604u16,
            in("ax") 0x2000u16,
        );

        // Bochs/older QEMU
        core::arch::asm!(
            "out dx, ax",
            in("dx") 0xB004u16,
            in("ax") 0x2000u16,
        );
    }

    crate::kprintln!("power: ACPI shutdown failed, halting...");
    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}

/// Initiate system reboot
pub fn reboot() -> ! {
    crate::kprintln!("power: initiating reboot...");

    // Try keyboard controller reset
    #[cfg(target_arch = "x86_64")]
    unsafe {
        // Wait for keyboard controller to be ready
        loop {
            let status: u8;
            core::arch::asm!(
                "in al, dx",
                in("dx") 0x64u16,
                out("al") status,
            );
            if status & 0x02 == 0 {
                break;
            }
        }

        // Send reset command
        core::arch::asm!(
            "out dx, al",
            in("dx") 0x64u16,
            in("al") 0xFEu8,
        );
    }

    // If keyboard reset failed, try triple fault
    crate::kprintln!("power: keyboard reset failed, attempting triple fault...");

    #[cfg(target_arch = "x86_64")]
    unsafe {
        // Load a null IDT
        let null_idt: [u8; 10] = [0; 10];
        core::arch::asm!(
            "lidt [{0}]",
            "int3",
            in(reg) null_idt.as_ptr(),
        );
    }

    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}

/// Initiate suspend to RAM (S3)
pub fn suspend() -> crate::util::KResult<()> {
    crate::kprintln!("power: suspend to RAM not yet implemented");
    Err(crate::util::KError::NotSupported)
}
