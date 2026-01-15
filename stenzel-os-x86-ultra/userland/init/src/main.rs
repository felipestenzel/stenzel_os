//! Stenzel OS init (PID 1)
//!
//! O primeiro processo userspace. Responsável por:
//! - Montar filesystems básicos (/dev, /proc, /sys)
//! - Iniciar o shell de login
//! - Recolher processos órfãos (wait loop)

#![no_std]
#![no_main]

use stenzel_libc::*;

/// Mensagem de boas-vindas
const BANNER: &str = r#"
   _____ _                       _    ___  ____
  / ____| |                     | |  / _ \/ ___|
 | (___ | |_ ___ _ __  _______| | | | | \___ \
  \___ \| __/ _ \ '_ \|_  / _ \ | | | | |___) |
  ____) | ||  __/ | | |/ /  __/ | |_| |____) |
 |_____/ \__\___|_| |_/___\___|_|\___/|_____/

"#;

/// Entry point (chamado por _start da libc)
#[no_mangle]
pub extern "C" fn main(_argc: i32, _argv: *const *const u8, _envp: *const *const u8) -> i32 {
    // Mostra banner
    print(BANNER);
    println("Stenzel OS init starting...");
    print("  PID: ");
    print_num(getpid() as i64);
    print("\n");

    // Loop principal do init
    // Fork e exec do shell, depois wait por filhos
    loop {
        println("[init] Spawning /bin/sh...");

        let pid = fork();

        if pid < 0 {
            println("[init] fork() failed!");
            // Aguarda um pouco e tenta novamente
            continue;
        }

        if pid == 0 {
            // Processo filho - executa o shell
            // Prepara argv e envp para execve
            let path = b"/bin/sh\0";
            let arg0 = b"/bin/sh\0";
            let argv: [*const u8; 2] = [arg0.as_ptr(), core::ptr::null()];
            let envp: [*const u8; 1] = [core::ptr::null()];

            let ret = execve(
                "/bin/sh",
                &argv,
                &envp,
            );

            // Se chegou aqui, execve falhou
            print("[init] execve failed with code: ");
            print_num(ret as i64);
            print("\n");

            // Shell não existe ainda, vamos ao menos mostrar algo útil
            println("[init] No /bin/sh found. Running built-in shell...");
            builtin_shell();
            exit(1);
        }

        // Processo pai - aguarda o shell terminar
        print("[init] Shell started with PID ");
        print_num(pid as i64);
        print("\n");

        let mut status: i32 = 0;
        let result = waitpid(pid, &mut status, 0);

        if result > 0 {
            print("[init] Shell exited with status ");
            print_num((status >> 8) as i64);
            print("\n");
        }

        // Loop de reaping de processos órfãos
        loop {
            let orphan = waitpid(-1, &mut status, 1); // WNOHANG = 1
            if orphan <= 0 {
                break;
            }
            print("[init] Reaped orphan PID ");
            print_num(orphan as i64);
            print("\n");
        }

        // Pequena pausa antes de reiniciar o shell
        println("[init] Restarting shell in 1 second...");
        // TODO: nanosleep quando implementado
    }
}

/// Shell embutido simples caso /bin/sh não exista
fn builtin_shell() {
    println("\n=== Stenzel OS Built-in Shell ===");
    println("Commands: help, whoami, pid, exit\n");

    let mut buf = [0u8; 128];

    loop {
        print("stenzel# ");

        // Lê uma linha
        let mut idx = 0;
        loop {
            let mut c = [0u8; 1];
            let n = read(0, &mut c);
            if n <= 0 {
                continue;
            }

            if c[0] == b'\n' || c[0] == b'\r' {
                print("\n");
                break;
            }

            if c[0] == 127 || c[0] == 8 {
                // Backspace
                if idx > 0 {
                    idx -= 1;
                    print("\x08 \x08");
                }
                continue;
            }

            if idx < buf.len() - 1 {
                buf[idx] = c[0];
                idx += 1;
                write(1, &c);
            }
        }

        buf[idx] = 0;

        // Processa comando
        let cmd = &buf[..idx];

        if cmd == b"help" {
            println("Available commands:");
            println("  help   - Show this message");
            println("  whoami - Show current user");
            println("  pid    - Show current PID");
            println("  exit   - Exit shell");
        } else if cmd == b"whoami" {
            print("uid=");
            print_num(getuid() as i64);
            print(" gid=");
            print_num(getgid() as i64);
            print("\n");
        } else if cmd == b"pid" {
            print("PID: ");
            print_num(getpid() as i64);
            print("\n");
        } else if cmd == b"exit" {
            println("Goodbye!");
            return;
        } else if idx > 0 {
            print("Unknown command: ");
            write(1, cmd);
            print("\n");
        }
    }
}
