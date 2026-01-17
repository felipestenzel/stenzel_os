//! Stenzel OS Shell (/bin/sh)
//!
//! Shell interativo simples para o Stenzel OS.
//! Suporta comandos básicos, pipes e execução de programas externos.

#![no_std]
#![no_main]

use stenzel_libc::*;

const MAX_LINE: usize = 256;
const MAX_ARGS: usize = 16;
const MAX_PIPE_CMDS: usize = 4; // Máximo de comandos em um pipeline

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

        if cmd == b"touch" {
            if argc < 2 {
                println("Usage: touch <file>");
                continue;
            }
            let path = unsafe { cstr_to_slice(args[1]) };
            if let Ok(s) = core::str::from_utf8(path) {
                touch_file(s);
            }
            continue;
        }

        if cmd == b"mkdir" {
            if argc < 2 {
                println("Usage: mkdir <dir>");
                continue;
            }
            let path = unsafe { cstr_to_slice(args[1]) };
            if let Ok(s) = core::str::from_utf8(path) {
                if mkdir(s, 0o755) < 0 {
                    print("mkdir: cannot create directory '");
                    print(s);
                    println("'");
                }
            }
            continue;
        }

        if cmd == b"rm" {
            if argc < 2 {
                println("Usage: rm <file>");
                continue;
            }
            let path = unsafe { cstr_to_slice(args[1]) };
            if let Ok(s) = core::str::from_utf8(path) {
                if unlink(s) < 0 {
                    print("rm: cannot remove '");
                    print(s);
                    println("'");
                }
            }
            continue;
        }

        if cmd == b"rmdir" {
            if argc < 2 {
                println("Usage: rmdir <dir>");
                continue;
            }
            let path = unsafe { cstr_to_slice(args[1]) };
            if let Ok(s) = core::str::from_utf8(path) {
                if rmdir(s) < 0 {
                    print("rmdir: cannot remove directory '");
                    print(s);
                    println("'");
                }
            }
            continue;
        }

        // Verifica se há pipe na linha
        if has_pipe(&line_buf[..len]) {
            execute_pipeline(&mut line_buf[..len]);
            continue;
        }

        // Comando externo - tenta executar
        execute_external(args[0], &args[..argc + 1]);
    }
}

/// Verifica se a linha contém um pipe
fn has_pipe(line: &[u8]) -> bool {
    for &c in line {
        if c == b'|' {
            return true;
        }
        if c == 0 {
            break;
        }
    }
    false
}

/// Divide a linha em comandos separados por |
fn split_pipeline(line: &mut [u8]) -> ([usize; MAX_PIPE_CMDS], usize) {
    let mut cmd_starts = [0usize; MAX_PIPE_CMDS];
    let mut cmd_count = 0;
    let mut i = 0;

    // Primeiro comando começa no início
    // Pula espaços iniciais
    while i < line.len() && (line[i] == b' ' || line[i] == b'\t') {
        i += 1;
    }
    if i < line.len() && line[i] != 0 {
        cmd_starts[cmd_count] = i;
        cmd_count += 1;
    }

    while i < line.len() && line[i] != 0 && cmd_count < MAX_PIPE_CMDS {
        if line[i] == b'|' {
            // Termina o comando anterior
            line[i] = 0;
            i += 1;

            // Pula espaços após o pipe
            while i < line.len() && (line[i] == b' ' || line[i] == b'\t') {
                i += 1;
            }

            // Início do próximo comando
            if i < line.len() && line[i] != 0 {
                cmd_starts[cmd_count] = i;
                cmd_count += 1;
            }
        } else {
            i += 1;
        }
    }

    (cmd_starts, cmd_count)
}

/// Executa um pipeline de comandos (cmd1 | cmd2 | cmd3 ...)
fn execute_pipeline(line: &mut [u8]) {
    let (cmd_starts, cmd_count) = split_pipeline(line);

    if cmd_count == 0 {
        return;
    }

    if cmd_count == 1 {
        // Sem pipe real, executa normalmente
        let mut args: [*const u8; MAX_ARGS + 1] = [core::ptr::null(); MAX_ARGS + 1];
        let start = cmd_starts[0];
        let argc = parse_args(&mut line[start..], &mut args);
        if argc > 0 {
            execute_external(args[0], &args[..argc + 1]);
        }
        return;
    }

    // Pipeline com múltiplos comandos
    // Cria pipes: precisamos de (cmd_count - 1) pipes
    let mut pipes = [[0i32; 2]; MAX_PIPE_CMDS];

    for i in 0..(cmd_count - 1) {
        if pipe(&mut pipes[i]) < 0 {
            println("sh: pipe failed");
            return;
        }
    }

    // Cria processos filhos
    let mut pids = [0i32; MAX_PIPE_CMDS];

    for i in 0..cmd_count {
        let pid = fork();

        if pid < 0 {
            println("sh: fork failed");
            // Fecha pipes já criados
            for j in 0..(cmd_count - 1) {
                close(pipes[j][0]);
                close(pipes[j][1]);
            }
            return;
        }

        if pid == 0 {
            // Processo filho

            // Se não for o primeiro comando, redireciona stdin do pipe anterior
            if i > 0 {
                dup2(pipes[i - 1][0], 0); // stdin = read end do pipe anterior
            }

            // Se não for o último comando, redireciona stdout para o pipe
            if i < cmd_count - 1 {
                dup2(pipes[i][1], 1); // stdout = write end do pipe atual
            }

            // Fecha todos os file descriptors dos pipes (já duplicados)
            for j in 0..(cmd_count - 1) {
                close(pipes[j][0]);
                close(pipes[j][1]);
            }

            // Parse argumentos deste comando
            let mut args: [*const u8; MAX_ARGS + 1] = [core::ptr::null(); MAX_ARGS + 1];
            let start = cmd_starts[i];
            let argc = parse_args(&mut line[start..], &mut args);

            if argc == 0 {
                exit(1);
            }

            // Executa o comando
            exec_command(args[0], &args[..argc + 1]);
            exit(127);
        }

        pids[i] = pid;
    }

    // Processo pai: fecha todos os pipes
    for i in 0..(cmd_count - 1) {
        close(pipes[i][0]);
        close(pipes[i][1]);
    }

    // Aguarda todos os filhos
    for i in 0..cmd_count {
        let mut status: i32 = 0;
        waitpid(pids[i], &mut status, 0);
    }
}

/// Parse de argumentos de um comando (similar a parse_line mas para substring)
fn parse_args(cmd: &mut [u8], args: &mut [*const u8]) -> usize {
    let mut argc = 0;
    let mut in_token = false;
    let mut i = 0;

    // Trim trailing spaces
    let mut end = cmd.len();
    while end > 0 && (cmd[end - 1] == b' ' || cmd[end - 1] == b'\t' || cmd[end - 1] == 0) {
        end -= 1;
    }

    while i < end && argc < args.len() - 1 {
        if cmd[i] == b' ' || cmd[i] == b'\t' {
            if in_token {
                cmd[i] = 0;
                in_token = false;
            }
        } else {
            if !in_token {
                args[argc] = &cmd[i] as *const u8;
                argc += 1;
                in_token = true;
            }
        }
        i += 1;
    }

    args[argc] = core::ptr::null();
    argc
}

/// Executa um comando (constrói path e chama execve)
fn exec_command(cmd: *const u8, args: &[*const u8]) {
    let cmd_str = unsafe { cstr_to_slice(cmd) };

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
            if c == 0 {
                break;
            }
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

    let path_slice = unsafe { cstr_to_slice(path_ptr) };
    if let Ok(path_str) = core::str::from_utf8(path_slice) {
        execve(path_str, args, &envp);
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
    println("  help      - Show this message");
    println("  exit      - Exit the shell");
    println("  cd <dir>  - Change directory");
    println("  pwd       - Print working directory");
    println("  whoami    - Show current user");
    println("  pid       - Show process ID");
    println("  echo      - Print arguments");
    println("  cat       - Display file contents");
    println("  ls        - List directory");
    println("  touch     - Create empty file");
    println("  mkdir     - Create directory");
    println("  rm        - Remove file");
    println("  rmdir     - Remove empty directory");
    println("");
    println("External commands:");
    println("  Type the path to an executable (e.g., /bin/program)");
}

fn touch_file(path: &str) {
    // O_CREAT | O_WRONLY = 0x41 (64 + 1)
    let fd = open(path, 0x41, 0o644);
    if fd < 0 {
        print("touch: cannot create '");
        print(path);
        println("'");
        return;
    }
    close(fd);
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
    use stenzel_libc::{getdents64, Dirent64Header, DT_DIR, DT_REG, DT_LNK, DT_CHR, DT_BLK};

    // Open the directory
    // O_RDONLY = 0, O_DIRECTORY = 0x10000 (Linux)
    let fd = open(path, 0x10000, 0);
    if fd < 0 {
        print("ls: cannot access '");
        print(path);
        println("'");
        return;
    }

    // Buffer for directory entries
    let mut buf = [0u8; 1024];

    loop {
        let n = getdents64(fd, &mut buf);
        if n <= 0 {
            break;
        }

        // Parse entries
        let mut offset = 0usize;
        while offset < n as usize {
            let entry = unsafe { &*(buf.as_ptr().add(offset) as *const Dirent64Header) };
            let name = unsafe { entry.name() };

            // Print type indicator
            match entry.d_type {
                DT_DIR => print("d "),
                DT_REG => print("- "),
                DT_LNK => print("l "),
                DT_CHR => print("c "),
                DT_BLK => print("b "),
                _ => print("? "),
            }

            // Print name
            write(1, name);
            print("\n");

            offset += entry.d_reclen as usize;
        }
    }

    close(fd);
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
