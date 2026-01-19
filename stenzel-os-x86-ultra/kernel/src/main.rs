    #![no_std]
    #![no_main]
    #![feature(alloc_error_handler)]
    #![feature(abi_x86_interrupt)]

    extern crate alloc;

    mod arch;
    mod boot;
    mod cgroups;
    mod console;
    mod crypto;
    mod drivers;
    mod fs;
    mod gui;
    mod ipc;
    mod mm;
    mod net;
    mod pkg;
    mod process;
    mod security;
    mod serial;
    mod sched;
    mod signal;
    mod storage;
    mod sync;
    mod syscall;
    mod task;
    mod time;
    mod unicode;
    mod i18n;
    mod userland;
    mod users;
    mod installer;
    mod tests;
    mod help;
    mod compat;
    mod power;
    mod profiling;
    mod cloud;
    mod util;

    use bootloader_api::config::{BootloaderConfig, FrameBuffer, Mapping};
    use bootloader_api::{entry_point, BootInfo};

    pub static BOOTLOADER_CONFIG: BootloaderConfig = {
        let mut config = BootloaderConfig::new_default();
        // Precisamos do mapeamento de memória física para:
        // - ler page tables ativas via CR3
        // - mapear MMIO no futuro (PCI BARs, APIC, etc.)
        config.mappings.physical_memory = Some(Mapping::Dynamic);
        // Stack do kernel relativamente grande (ISR + stacks por thread ficam separados).
        config.kernel_stack_size = 256 * 1024;
        // Request a framebuffer for GOP/UEFI graphics
        let mut fb = FrameBuffer::new_default();
        fb.minimum_framebuffer_width = Some(800);
        fb.minimum_framebuffer_height = Some(600);
        config.frame_buffer = fb;
        config
    };

    entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

    fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
        serial::init();
        util::banner();

        // Initialize GOP/UEFI framebuffer if available
        if let Some(framebuffer) = boot_info.framebuffer.take() {
            util::kprintln!("boot: inicializando GOP framebuffer...");
            drivers::framebuffer::init(framebuffer);
        } else {
            util::kprintln!("boot: GOP framebuffer not available");
        }

        util::kprintln!("boot: inicializando arch/x86_64...");
        arch::init();

        util::kprintln!("boot: inicializando memória...");
        mm::init(boot_info);
        mm::vma::init();
        ipc::init();
        cgroups::init();

        // ACPI deve ser inicializado antes do APIC para que o MADT seja usado
        util::kprintln!("boot: detectando ACPI...");
        drivers::acpi::init();

        // Tenta migrar para APIC (requer mm inicializado + ACPI para MADT)
        arch::init_late();

        // Inicializa HPET para timing de alta precisão
        util::kprintln!("boot: inicializando HPET...");
        if drivers::hpet::init() {
            util::kprintln!("boot: HPET disponível");
        } else {
            util::kprintln!("boot: HPET não disponível, usando timers alternativos");
        }

        // Inicializa TSC (Time Stamp Counter)
        util::kprintln!("boot: inicializando TSC...");
        if arch::tsc::init() {
            util::kprintln!("boot: TSC disponível");
        } else {
            util::kprintln!("boot: TSC não disponível");
        }

        // Parse DSDT/SSDT for ACPI device discovery
        util::kprintln!("boot: parsing ACPI DSDT/SSDT...");
        drivers::acpi::init_dsdt();

        util::kprintln!("boot: inicializando PS/2 mouse...");
        drivers::mouse::init();

        util::kprintln!("boot: inicializando input event system...");
        drivers::input::init();

        util::kprintln!("boot: inicializando GUI subsystem...");
        gui::init();

        util::kprintln!("boot: inicializando segurança/usuários...");
        security::init();

        util::kprintln!("boot: inicializando VFS (tmpfs)...");
        fs::init();

        // Popula uma estrutura de diretórios e arquivos default.
        fs::bootstrap_filesystem();

        // Instala binários userland (/bin/init, /bin/sh)
        util::kprintln!("boot: instalando userland binaries...");
        userland::install_userland_binaries();

        util::kprintln!("boot: inicializando storage (PCI scan + virtio-blk)...");
        storage::init();

        util::kprintln!("boot: inicializando USB (xHCI)...");
        drivers::usb::init();

        util::kprintln!("boot: enumerando dispositivos USB...");
        drivers::usb::xhci::enumerate_all_devices();

        util::kprintln!("boot: inicializando time/RTC...");
        time::init();

        util::kprintln!("boot: inicializando syscalls...");
        syscall::init();

        util::kprintln!("boot: inicializando network stack...");
        net::init();

        // Inicializa o scheduler (apenas idle task por enquanto)
        util::kprintln!("boot: inicializando scheduler...");
        sched::init_scheduler_only();

        // Test kernel thread before enabling interrupts
        // (This spawns a kernel thread that will run once scheduler starts)
        if let Err(e) = sched::spawn_kernel_thread("test-kthread", test_kernel_thread, 42) {
            util::kprintln!("boot: WARN: failed to spawn test kernel thread: {:?}", e);
        }

        util::kprintln!("boot: habilitando interrupções...");
        arch::enable_interrupts();

        // Tenta iniciar /bin/init (PID 1)
        util::kprintln!("boot: iniciando /bin/init...\n");
        // Debug sem lock
        unsafe {
            use x86_64::instructions::port::Port;
            let mut port: Port<u8> = Port::new(0x3F8);
            for b in b"[DBG:calling_spawn_init]\n" {
                port.write(*b);
            }
        }
        match sched::spawn_init() {
            Ok(()) => {
                util::kprintln!("boot: init spawned, entrando no scheduler loop...\n");
                // Loop do scheduler (idle loop)
                loop {
                    x86_64::instructions::hlt();
                }
            }
            Err(e) => {
                // Fallback para kernel shell se init falhar
                util::kprintln!("boot: FALHA ao iniciar /bin/init: {:?}", e);
                util::kprintln!("boot: fallback para kernel shell...\n");
                task::shell::shell_thread(0);
            }
        }

        // Se o shell retornar, entra em halt
        loop {
            x86_64::instructions::hlt();
        }
    }

    use core::panic::PanicInfo;

    #[panic_handler]
    fn panic(info: &PanicInfo) -> ! {
        util::kprintln!("\n\n!!! KERNEL PANIC !!!");
        util::kprintln!("mensagem: {}", info.message());
        if let Some(loc) = info.location() {
            util::kprintln!("local: {}:{}:{}", loc.file(), loc.line(), loc.column());
        }
        util::kprintln!("halt.");
        arch::halt_loop();
    }

    #[alloc_error_handler]
    fn alloc_error(layout: core::alloc::Layout) -> ! {
        util::kprintln!("ERRO: alocação falhou: {:?}", layout);
        arch::halt_loop();
    }

    /// Test kernel thread entry function
    fn test_kernel_thread(arg: u64) -> ! {
        util::kprintln!("kthread: test kernel thread running with arg={}", arg);
        util::kprintln!("kthread: test kernel thread completing successfully");

        // Exit the kernel thread with success
        sched::kthread_exit(0);
    }
