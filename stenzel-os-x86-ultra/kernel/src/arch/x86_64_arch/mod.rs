    pub mod apic;
pub mod core_parking;
pub mod cpu_hotplug;
pub mod cpufreq;
    pub mod gdt;
    pub mod interrupts;
pub mod ipi;
pub mod nmi;
pub mod percpu;
    pub mod pic;
    pub mod pit;
pub mod smp;
pub mod switch;
pub mod syscall;
pub mod tsc;

    /// Inicialização inicial (antes de mm::init)
    /// Configura GDT, IDT, PIC legado (para ter interrupções mínimas)
    pub fn init() {
        crate::kprintln!("arch: gdt::init()...");
        gdt::init();
        crate::kprintln!("arch: interrupts::init()...");
        interrupts::init();

        // Usa PIC legado inicialmente (APIC será habilitado depois de mm::init)
        crate::kprintln!("arch: pic::init(32, 40)...");
        unsafe { pic::init(32, 40) };
        crate::kprintln!("arch: pit::init(1000)...");
        pit::init(1000); // 1000 Hz tick

        // Early per-CPU setup (sets GS base for syscall entry)
        crate::kprintln!("arch: percpu::early_init_bsp()...");
        percpu::early_init_bsp();

        crate::kprintln!("arch: syscall::init()...");
        syscall::init();
        crate::kprintln!("arch: init complete");
    }

    /// Inicialização tardia (após mm::init)
    /// Tenta migrar para APIC se disponível
    pub fn init_late() {
        if apic::is_apic_available() {
            crate::kprintln!("arch: tentando migrar para APIC...");
            if apic::init() {
                crate::kprintln!("arch: APIC habilitado com sucesso");
                // Initialize IPI subsystem (requires APIC)
                ipi::init();
            } else {
                crate::kprintln!("arch: falha ao habilitar APIC, mantendo PIC 8259");
            }
        } else {
            crate::kprintln!("arch: APIC não disponível, mantendo PIC 8259");
        }

        // Initialize NMI handling
        nmi::init();

        // Initialize per-CPU data for BSP
        percpu::init_bsp();

        // Initialize SMP CPU detection
        smp::init();

        // Start Application Processors (if multiple CPUs detected)
        smp::start_aps();
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
