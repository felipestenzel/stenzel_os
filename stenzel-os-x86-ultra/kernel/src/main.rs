    #![no_std]
    #![no_main]
    #![feature(alloc_error_handler)]
    #![feature(abi_x86_interrupt)]

    extern crate alloc;

    mod arch;
    mod console;
    mod drivers;
    mod fs;
    mod mm;
    mod net;
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
    mod userland;
    mod util;

    use bootloader_api::config::{BootloaderConfig, Mapping};
    use bootloader_api::{entry_point, BootInfo};

    pub static BOOTLOADER_CONFIG: BootloaderConfig = {
        let mut config = BootloaderConfig::new_default();
        // Precisamos do mapeamento de memória física para:
        // - ler page tables ativas via CR3
        // - mapear MMIO no futuro (PCI BARs, APIC, etc.)
        config.mappings.physical_memory = Some(Mapping::Dynamic);
        // Stack do kernel relativamente grande (ISR + stacks por thread ficam separados).
        config.kernel_stack_size = 256 * 1024;
        config
    };

    entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

    fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
        serial::init();
        util::banner();

        util::kprintln!("boot: inicializando arch/x86_64...");
        arch::init();

        util::kprintln!("boot: inicializando memória...");
        mm::init(boot_info);
        mm::vma::init();

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

        util::kprintln!("boot: inicializando time/RTC...");
        time::init();

        util::kprintln!("boot: inicializando syscalls...");
        syscall::init();

        util::kprintln!("boot: inicializando network stack...");
        net::init();

        // Inicializa o scheduler (apenas idle task por enquanto)
        util::kprintln!("boot: inicializando scheduler...");
        sched::init_scheduler_only();

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
