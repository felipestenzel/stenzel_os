//! Stenzel OS Shell (/bin/sh)
//!
//! Shell interativo simples para o Stenzel OS.
//! Suporta comandos básicos e execução de programas externos.

#![no_std]
#![no_main]

use stenzel_libc::*;

const MAX_LINE: usize = 256;
const MAX_ARGS: usize = 16;

/// Entry point
#[no_mangle]
pub extern "C" fn main(_argc: i32, _argv: *const *const u8, _envp: *const *const u8) -> i32 {
    println("Stenzel Shell v0.1");
    println("Type 'help' for available commands.\n");

    let mut line_buf = [0u8; MAX_LINE];

    loop {
        // Mostra prompt
        print_prompt();

        // Lê linha de comando
        let len = read_line(&mut line_buf);
        if len == 0 {
            continue;
        }

        // Faz parse da linha em argumentos
        let mut args: [*const u8; MAX_ARGS + 1] = [core::ptr::null(); MAX_ARGS + 1];
        let argc = parse_line(&mut line_buf[..len], &mut args);
        if argc == 0 {
            continue;
        }

        // Primeiro argumento é o comando
        let cmd = unsafe { cstr_to_slice(args[0]) };

        // Processa comandos built-in
        if cmd == b"exit" || cmd == b"quit" {
            println("Bye!");
            return 0;
        }

        if cmd == b"help" {
            show_help();
            continue;
        }

        if cmd == b"cd" {
            if argc > 1 {
                let path = unsafe { cstr_to_slice(args[1]) };
                // Converte para &str para usar com chdir
                if let Ok(s) = core::str::from_utf8(path) {
                    let ret = chdir(s);
                    if ret < 0 {
                        println("cd: failed to change directory");
                    }
                }
            } else {
                let ret = chdir("/");
                if ret < 0 {
                    println("cd: failed to change directory");
                }
            }
            continue;
        }

        if cmd == b"pwd" {
            let mut buf = [0u8; 128];
            let ret = getcwd(&mut buf);
            if ret >= 0 {
                // Encontra o final da string
                let mut end = 0;
                while end < buf.len() && buf[end] != 0 {
                    end += 1;
                }
                write(1, &buf[..end]);
                print("\n");
            } else {
                println("pwd: failed to get current directory");
            }
            continue;
        }

        if cmd == b"whoami" {
            print("uid=");
            print_num(getuid() as i64);
            print(" gid=");
            print_num(getgid() as i64);
            print(" euid=");
            print_num(geteuid() as i64);
            print(" egid=");
            print_num(getegid() as i64);
            print("\n");
            continue;
        }

        if cmd == b"pid" {
            print("PID=");
            print_num(getpid() as i64);
            print(" PPID=");
            print_num(getppid() as i64);
            print("\n");
            continue;
        }

        if cmd == b"echo" {
            for i in 1..argc {
                if i > 1 {
                    print(" ");
                }
                let arg = unsafe { cstr_to_slice(args[i]) };
                write(1, arg);
            }
            print("\n");
            continue;
        }

        if cmd == b"cat" {
            if argc < 2 {
                println("Usage: cat <file>");
                continue;
            }
            let path = unsafe { cstr_to_slice(args[1]) };
            if let Ok(s) = core::str::from_utf8(path) {
                cat_file(s);
            }
            continue;
        }

        if cmd == b"ls" {
            let path = if argc > 1 {
                unsafe { cstr_to_slice(args[1]) }
            } else {
                b"."
            };
            if let Ok(s) = core::str::from_utf8(path) {
                ls_dir(s);
            }
            continue;
        }

        // Comando externo - tenta executar
        execute_external(args[0], &args[..argc + 1]);
    }
}

fn print_prompt() {
    let uid = getuid();
    if uid == 0 {
        print("root@stenzel# ");
    } else {
        print("user@stenzel$ ");
    }
}

fn read_line(buf: &mut [u8]) -> usize {
    let mut idx = 0;

    loop {
        let mut c = [0u8; 1];
        let n = read(0, &mut c);

        if n <= 0 {
            // EOF ou erro
            if idx == 0 {
                return 0;
            }
            break;
        }

        let ch = c[0];

        // Enter - fim da linha
        if ch == b'\n' || ch == b'\r' {
            print("\n");
            break;
        }

        // Ctrl+C - cancela linha
        if ch == 3 {
            println("^C");
            return 0;
        }

        // Ctrl+D - EOF
        if ch == 4 {
            if idx == 0 {
                println("");
                exit(0);
            }
            break;
        }

        // Backspace
        if ch == 127 || ch == 8 {
            if idx > 0 {
                idx -= 1;
                print("\x08 \x08");
            }
            continue;
        }

        // Caractere normal
        if idx < buf.len() - 1 && ch >= 32 && ch < 127 {
            buf[idx] = ch;
            idx += 1;
            write(1, &c);
        }
    }

    buf[idx] = 0;
    idx
}

/// Parse da linha em tokens separados por espaço
fn parse_line(line: &mut [u8], args: &mut [*const u8]) -> usize {
    let mut argc = 0;
    let mut in_token = false;
    let mut i = 0;

    while i < line.len() && line[i] != 0 && argc < args.len() - 1 {
        if line[i] == b' ' || line[i] == b'\t' {
            if in_token {
                line[i] = 0; // Termina o token
                in_token = false;
            }
        } else {
            if !in_token {
                args[argc] = &line[i] as *const u8;
                argc += 1;
                in_token = true;
            }
        }
        i += 1;
    }

    args[argc] = core::ptr::null(); // Termina a lista
    argc
}

/// Converte C string para slice
unsafe fn cstr_to_slice(s: *const u8) -> &'static [u8] {
    let len = strlen(s);
    core::slice::from_raw_parts(s, len)
}

fn show_help() {
    println("Built-in commands:");
    println("  help     - Show this message");
    println("  exit     - Exit the shell");
    println("  cd <dir> - Change directory");
    println("  pwd      - Print working directory");
    println("  whoami   - Show current user");
    println("  pid      - Show process ID");
    println("  echo     - Print arguments");
    println("  cat      - Display file contents");
    println("  ls       - List directory");
    println("");
    println("External commands:");
    println("  Type the path to an executable (e.g., /bin/program)");
}

fn cat_file(path: &str) {
    let fd = open(path, 0, 0); // O_RDONLY = 0
    if fd < 0 {
        print("cat: cannot open '");
        print(path);
        println("'");
        return;
    }

    let mut buf = [0u8; 512];
    loop {
        // Usa o fd diretamente
        let n = unsafe {
            syscall3(SYS_READ, fd as u64, buf.as_mut_ptr() as u64, buf.len() as u64) as isize
        };
        if n <= 0 {
            break;
        }
        write(1, &buf[..n as usize]);
    }

    close(fd);
}

fn ls_dir(path: &str) {
    // TODO: implementar getdents quando disponível
    // Por enquanto, mostra mensagem
    print("ls: listing '");
    print(path);
    println("' (not fully implemented)");
    println("(Need getdents syscall)");
}

fn execute_external(cmd: *const u8, args: &[*const u8]) {
    let pid = fork();

    if pid < 0 {
        println("fork failed");
        return;
    }

    if pid == 0 {
        // Filho - executa o programa
        let cmd_str = unsafe { cstr_to_slice(cmd) };

        // Se não começa com /, procura em /bin
        let mut path_buf = [0u8; 128];
        let path_ptr: *const u8;

        if cmd_str.first() != Some(&b'/') {
            // Prepend /bin/
            let prefix = b"/bin/";
            let mut i = 0;
            for &c in prefix {
                path_buf[i] = c;
                i += 1;
            }
            for &c in cmd_str {
                if i < path_buf.len() - 1 {
                    path_buf[i] = c;
                    i += 1;
                }
            }
            path_buf[i] = 0;
            path_ptr = path_buf.as_ptr();
        } else {
            path_ptr = cmd;
        }

        let envp: [*const u8; 1] = [core::ptr::null()];

        // Converte para &str
        let path_slice = unsafe { cstr_to_slice(path_ptr) };
        if let Ok(path_str) = core::str::from_utf8(path_slice) {
            let ret = execve(path_str, args, &envp);
            // Se chegou aqui, execve falhou
            print("sh: command not found: ");
            write(1, path_slice);
            print(" (error ");
            print_num(ret as i64);
            println(")");
        } else {
            println("sh: invalid command path");
        }

        exit(127);
    }

    // Pai - aguarda o filho
    let mut status: i32 = 0;
    waitpid(pid, &mut status, 0);
}
