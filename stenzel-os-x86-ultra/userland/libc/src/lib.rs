//! Stenzel OS mini libc
//!
//! Biblioteca C mínima para programas userspace do Stenzel OS.
//! Provê syscall wrappers e funções básicas.

#![no_std]
#![allow(dead_code)]

use core::arch::{asm, naked_asm};

// ============================================================================
// Syscall numbers (Linux x86_64 ABI)
// ============================================================================

pub const SYS_READ: u64 = 0;
pub const SYS_WRITE: u64 = 1;
pub const SYS_OPEN: u64 = 2;
pub const SYS_CLOSE: u64 = 3;
pub const SYS_STAT: u64 = 4;
pub const SYS_FSTAT: u64 = 5;
pub const SYS_LSEEK: u64 = 8;
pub const SYS_MMAP: u64 = 9;
pub const SYS_MPROTECT: u64 = 10;
pub const SYS_MUNMAP: u64 = 11;
pub const SYS_BRK: u64 = 12;
pub const SYS_IOCTL: u64 = 16;
pub const SYS_PIPE: u64 = 22;
pub const SYS_DUP: u64 = 32;
pub const SYS_DUP2: u64 = 33;
pub const SYS_NANOSLEEP: u64 = 35;
pub const SYS_GETPID: u64 = 39;
pub const SYS_FORK: u64 = 57;
pub const SYS_EXECVE: u64 = 59;
pub const SYS_EXIT: u64 = 60;
pub const SYS_WAIT4: u64 = 61;
pub const SYS_KILL: u64 = 62;
pub const SYS_UNAME: u64 = 63;
pub const SYS_GETCWD: u64 = 79;
pub const SYS_CHDIR: u64 = 80;
pub const SYS_MKDIR: u64 = 83;
pub const SYS_GETUID: u64 = 102;
pub const SYS_GETGID: u64 = 104;
pub const SYS_GETEUID: u64 = 107;
pub const SYS_GETEGID: u64 = 108;
pub const SYS_GETPPID: u64 = 110;
pub const SYS_ARCH_PRCTL: u64 = 158;
pub const SYS_GETTID: u64 = 186;
pub const SYS_EXIT_GROUP: u64 = 231;

// ============================================================================
// Raw syscall interface
// ============================================================================

#[inline(always)]
pub unsafe fn syscall0(nr: u64) -> i64 {
    let ret: i64;
    asm!(
        "syscall",
        in("rax") nr,
        lateout("rax") ret,
        out("rcx") _,
        out("r11") _,
        options(nostack)
    );
    ret
}

#[inline(always)]
pub unsafe fn syscall1(nr: u64, a1: u64) -> i64 {
    let ret: i64;
    asm!(
        "syscall",
        in("rax") nr,
        in("rdi") a1,
        lateout("rax") ret,
        out("rcx") _,
        out("r11") _,
        options(nostack)
    );
    ret
}

#[inline(always)]
pub unsafe fn syscall2(nr: u64, a1: u64, a2: u64) -> i64 {
    let ret: i64;
    asm!(
        "syscall",
        in("rax") nr,
        in("rdi") a1,
        in("rsi") a2,
        lateout("rax") ret,
        out("rcx") _,
        out("r11") _,
        options(nostack)
    );
    ret
}

#[inline(always)]
pub unsafe fn syscall3(nr: u64, a1: u64, a2: u64, a3: u64) -> i64 {
    let ret: i64;
    asm!(
        "syscall",
        in("rax") nr,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        lateout("rax") ret,
        out("rcx") _,
        out("r11") _,
        options(nostack)
    );
    ret
}

#[inline(always)]
pub unsafe fn syscall4(nr: u64, a1: u64, a2: u64, a3: u64, a4: u64) -> i64 {
    let ret: i64;
    asm!(
        "syscall",
        in("rax") nr,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        in("r10") a4,
        lateout("rax") ret,
        out("rcx") _,
        out("r11") _,
        options(nostack)
    );
    ret
}

#[inline(always)]
pub unsafe fn syscall6(nr: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64, a6: u64) -> i64 {
    let ret: i64;
    asm!(
        "syscall",
        in("rax") nr,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        in("r10") a4,
        in("r8") a5,
        in("r9") a6,
        lateout("rax") ret,
        out("rcx") _,
        out("r11") _,
        options(nostack)
    );
    ret
}

// ============================================================================
// POSIX-like syscall wrappers
// ============================================================================

/// write - escreve em um file descriptor
pub fn write(fd: i32, buf: &[u8]) -> isize {
    unsafe { syscall3(SYS_WRITE, fd as u64, buf.as_ptr() as u64, buf.len() as u64) as isize }
}

/// read - lê de um file descriptor
pub fn read(fd: i32, buf: &mut [u8]) -> isize {
    unsafe { syscall3(SYS_READ, fd as u64, buf.as_mut_ptr() as u64, buf.len() as u64) as isize }
}

/// open - abre um arquivo
pub fn open(path: &str, flags: i32, mode: u32) -> i32 {
    // Precisamos de um buffer null-terminated
    let mut buf = [0u8; 256];
    let len = path.len().min(255);
    buf[..len].copy_from_slice(&path.as_bytes()[..len]);
    buf[len] = 0;
    unsafe { syscall3(SYS_OPEN, buf.as_ptr() as u64, flags as u64, mode as u64) as i32 }
}

/// close - fecha um file descriptor
pub fn close(fd: i32) -> i32 {
    unsafe { syscall1(SYS_CLOSE, fd as u64) as i32 }
}

/// exit - termina o processo
pub fn exit(status: i32) -> ! {
    unsafe {
        syscall1(SYS_EXIT, status as u64);
    }
    loop {}
}

/// exit_group - termina todas as threads
pub fn exit_group(status: i32) -> ! {
    unsafe {
        syscall1(SYS_EXIT_GROUP, status as u64);
    }
    loop {}
}

/// fork - cria um processo filho
pub fn fork() -> i32 {
    unsafe { syscall0(SYS_FORK) as i32 }
}

/// execve - executa um programa
pub fn execve(path: &str, argv: &[*const u8], envp: &[*const u8]) -> i32 {
    let mut path_buf = [0u8; 256];
    let len = path.len().min(255);
    path_buf[..len].copy_from_slice(&path.as_bytes()[..len]);
    path_buf[len] = 0;

    unsafe {
        syscall3(
            SYS_EXECVE,
            path_buf.as_ptr() as u64,
            argv.as_ptr() as u64,
            envp.as_ptr() as u64,
        ) as i32
    }
}

/// wait4 - aguarda término de processo filho
pub fn wait4(pid: i32, status: *mut i32, options: i32) -> i32 {
    unsafe { syscall4(SYS_WAIT4, pid as u64, status as u64, options as u64, 0) as i32 }
}

/// waitpid - wrapper para wait4
pub fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32 {
    wait4(pid, status, options)
}

/// getpid - retorna PID do processo
pub fn getpid() -> i32 {
    unsafe { syscall0(SYS_GETPID) as i32 }
}

/// getppid - retorna PID do pai
pub fn getppid() -> i32 {
    unsafe { syscall0(SYS_GETPPID) as i32 }
}

/// getuid - retorna UID
pub fn getuid() -> u32 {
    unsafe { syscall0(SYS_GETUID) as u32 }
}

/// getgid - retorna GID
pub fn getgid() -> u32 {
    unsafe { syscall0(SYS_GETGID) as u32 }
}

/// geteuid - retorna UID efetivo
pub fn geteuid() -> u32 {
    unsafe { syscall0(SYS_GETEUID) as u32 }
}

/// getegid - retorna GID efetivo
pub fn getegid() -> u32 {
    unsafe { syscall0(SYS_GETEGID) as u32 }
}

/// brk - altera o program break (heap)
pub fn brk(addr: *mut u8) -> *mut u8 {
    unsafe { syscall1(SYS_BRK, addr as u64) as *mut u8 }
}

/// mkdir - cria diretório
pub fn mkdir(path: &str, mode: u32) -> i32 {
    let mut buf = [0u8; 256];
    let len = path.len().min(255);
    buf[..len].copy_from_slice(&path.as_bytes()[..len]);
    buf[len] = 0;
    unsafe { syscall2(SYS_MKDIR, buf.as_ptr() as u64, mode as u64) as i32 }
}

/// getcwd - obtém diretório atual
pub fn getcwd(buf: &mut [u8]) -> i32 {
    unsafe { syscall2(SYS_GETCWD, buf.as_mut_ptr() as u64, buf.len() as u64) as i32 }
}

/// chdir - muda diretório
pub fn chdir(path: &str) -> i32 {
    let mut buf = [0u8; 256];
    let len = path.len().min(255);
    buf[..len].copy_from_slice(&path.as_bytes()[..len]);
    buf[len] = 0;
    unsafe { syscall1(SYS_CHDIR, buf.as_ptr() as u64) as i32 }
}

/// dup - duplica file descriptor
pub fn dup(oldfd: i32) -> i32 {
    unsafe { syscall1(SYS_DUP, oldfd as u64) as i32 }
}

/// dup2 - duplica file descriptor para um fd específico
pub fn dup2(oldfd: i32, newfd: i32) -> i32 {
    unsafe { syscall2(SYS_DUP2, oldfd as u64, newfd as u64) as i32 }
}

/// pipe - cria um pipe
pub fn pipe(pipefd: &mut [i32; 2]) -> i32 {
    unsafe { syscall1(SYS_PIPE, pipefd.as_mut_ptr() as u64) as i32 }
}

/// kill - envia sinal
pub fn kill(pid: i32, sig: i32) -> i32 {
    unsafe { syscall2(SYS_KILL, pid as u64, sig as u64) as i32 }
}

// ============================================================================
// Convenience functions
// ============================================================================

/// Escreve uma string em stdout
pub fn print(s: &str) {
    write(1, s.as_bytes());
}

/// Escreve uma string em stdout com newline
pub fn println(s: &str) {
    print(s);
    print("\n");
}

/// Escreve um número em stdout
pub fn print_num(n: i64) {
    if n == 0 {
        print("0");
        return;
    }

    let mut buf = [0u8; 32];
    let mut idx = 31;
    let mut num = if n < 0 { -(n as i64) as u64 } else { n as u64 };

    while num > 0 && idx > 0 {
        buf[idx] = b'0' + (num % 10) as u8;
        num /= 10;
        idx -= 1;
    }

    if n < 0 && idx > 0 {
        buf[idx] = b'-';
        idx -= 1;
    }

    write(1, &buf[idx + 1..]);
}

// ============================================================================
// String utilities
// ============================================================================

/// Calcula o comprimento de uma string C (null-terminated)
pub unsafe fn strlen(s: *const u8) -> usize {
    let mut len = 0;
    while *s.add(len) != 0 {
        len += 1;
    }
    len
}

/// Compara duas strings C
pub unsafe fn strcmp(s1: *const u8, s2: *const u8) -> i32 {
    let mut i = 0;
    loop {
        let c1 = *s1.add(i);
        let c2 = *s2.add(i);
        if c1 != c2 {
            return (c1 as i32) - (c2 as i32);
        }
        if c1 == 0 {
            return 0;
        }
        i += 1;
    }
}

/// Copia uma string C
pub unsafe fn strcpy(dst: *mut u8, src: *const u8) -> *mut u8 {
    let mut i = 0;
    loop {
        let c = *src.add(i);
        *dst.add(i) = c;
        if c == 0 {
            break;
        }
        i += 1;
    }
    dst
}

/// Copia memória
pub unsafe fn memcpy(dst: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    for i in 0..n {
        *dst.add(i) = *src.add(i);
    }
    dst
}

/// Preenche memória
pub unsafe fn memset(dst: *mut u8, c: u8, n: usize) -> *mut u8 {
    for i in 0..n {
        *dst.add(i) = c;
    }
    dst
}

// ============================================================================
// _start entry point
// ============================================================================

/// Entry point para programas userspace.
/// O kernel passa argc, argv, envp na stack.
#[unsafe(naked)]
#[no_mangle]
#[link_section = ".text.start"]
pub extern "C" fn _start() -> ! {
    naked_asm!(
        // Stack layout do kernel:
        // rsp+0:  argc
        // rsp+8:  argv[0]
        // ...
        // rsp+8*(argc+1): NULL
        // rsp+8*(argc+2): envp[0]
        // ...
        // NULL

        // Alinha a stack em 16 bytes antes de chamar main
        "and rsp, -16",

        // Carrega argc em rdi (primeiro argumento)
        "mov rdi, [rsp]",

        // Carrega argv em rsi (segundo argumento)
        "lea rsi, [rsp + 8]",

        // Calcula envp: argv + (argc + 1) * 8
        "mov rax, rdi",
        "add rax, 1",
        "shl rax, 3",
        "lea rdx, [rsi + rax]",

        // Chama main(argc, argv, envp)
        "call main",

        // main retornou, chama exit(rax)
        "mov rdi, rax",
        "mov rax, 60",  // SYS_EXIT
        "syscall",

        // Nunca deve chegar aqui
        "ud2",
    )
}

// ============================================================================
// Panic handler (required for no_std)
// ============================================================================

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    print("PANIC: ");
    // Não podemos imprimir a mensagem completa sem alloc
    print("program panicked\n");
    exit(1)
}
