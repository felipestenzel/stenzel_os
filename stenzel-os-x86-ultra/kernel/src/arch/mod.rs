    pub mod x86_64_arch;

    pub use x86_64_arch::{enable_interrupts, halt_loop, init, init_late};
    pub use x86_64_arch::tsc;
    pub use x86_64_arch::smp::cpu_count;

    pub mod interrupts {
        pub use crate::arch::x86_64_arch::interrupts::{disable, restore};
    }
