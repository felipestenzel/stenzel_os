    pub mod x86_64_arch;

    pub use x86_64_arch::{enable_interrupts, halt_loop, init};

    pub mod interrupts {
        pub use crate::arch::x86_64_arch::interrupts::{disable, restore};
    }
