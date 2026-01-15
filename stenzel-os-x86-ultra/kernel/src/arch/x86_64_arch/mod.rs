    pub mod gdt;
    pub mod interrupts;
    pub mod pic;
    pub mod pit;
pub mod switch;
pub mod syscall;

    pub fn init() {
        crate::kprintln!("arch: gdt::init()...");
        gdt::init();
        crate::kprintln!("arch: interrupts::init()...");
    interrupts::init();
        crate::kprintln!("arch: pic::init(32, 40)...");
        unsafe { pic::init(32, 40) };
        crate::kprintln!("arch: pit::init(1000)...");
        pit::init(1000); // 1000 Hz tick
        crate::kprintln!("arch: syscall::init()...");
    syscall::init();
        crate::kprintln!("arch: init complete");
    }

    #[inline]
    pub fn enable_interrupts() {
        let before = x86_64::instructions::interrupts::are_enabled();
        x86_64::instructions::interrupts::enable();
        let after = x86_64::instructions::interrupts::are_enabled();
        crate::kprintln!("arch: enable_interrupts() IF: {} -> {}", before, after);
    }

    pub fn halt_loop() -> ! {
        loop {
            x86_64::instructions::hlt();
        }
    }
