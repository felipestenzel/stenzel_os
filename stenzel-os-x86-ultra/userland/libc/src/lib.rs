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
pub const SYS_RMDIR: u64 = 84;
pub const SYS_UNLINK: u64 = 87;
pub const SYS_GETUID: u64 = 102;
pub const SYS_GETGID: u64 = 104;
pub const SYS_GETEUID: u64 = 107;
pub const SYS_GETEGID: u64 = 108;
pub const SYS_GETPPID: u64 = 110;
pub const SYS_ARCH_PRCTL: u64 = 158;
pub const SYS_GETTID: u64 = 186;
pub const SYS_GETDENTS64: u64 = 217;
pub const SYS_CLONE: u64 = 56;
pub const SYS_FUTEX: u64 = 202;
pub const SYS_EXIT_GROUP: u64 = 231;
pub const SYS_SET_TID_ADDRESS: u64 = 218;

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
pub unsafe fn syscall5(nr: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64) -> i64 {
    let ret: i64;
    asm!(
        "syscall",
        in("rax") nr,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        in("r10") a4,
        in("r8") a5,
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

/// rmdir - remove diretório vazio
pub fn rmdir(path: &str) -> i32 {
    let mut buf = [0u8; 256];
    let len = path.len().min(255);
    buf[..len].copy_from_slice(&path.as_bytes()[..len]);
    buf[len] = 0;
    unsafe { syscall1(SYS_RMDIR, buf.as_ptr() as u64) as i32 }
}

/// unlink - remove arquivo
pub fn unlink(path: &str) -> i32 {
    let mut buf = [0u8; 256];
    let len = path.len().min(255);
    buf[..len].copy_from_slice(&path.as_bytes()[..len]);
    buf[len] = 0;
    unsafe { syscall1(SYS_UNLINK, buf.as_ptr() as u64) as i32 }
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

/// getdents64 - lê entradas de diretório
pub fn getdents64(fd: i32, buf: &mut [u8]) -> isize {
    unsafe { syscall3(SYS_GETDENTS64, fd as u64, buf.as_mut_ptr() as u64, buf.len() as u64) as isize }
}

// ============================================================================
// Directory entry structures
// ============================================================================

/// Tipos de arquivo (d_type)
pub const DT_UNKNOWN: u8 = 0;
pub const DT_FIFO: u8 = 1;
pub const DT_CHR: u8 = 2;
pub const DT_DIR: u8 = 4;
pub const DT_BLK: u8 = 6;
pub const DT_REG: u8 = 8;
pub const DT_LNK: u8 = 10;
pub const DT_SOCK: u8 = 12;

/// linux_dirent64 header (excluindo d_name)
#[repr(C)]
pub struct Dirent64Header {
    pub d_ino: u64,
    pub d_off: i64,
    pub d_reclen: u16,
    pub d_type: u8,
    // d_name follows (variable length, null-terminated)
}

impl Dirent64Header {
    /// Retorna o nome do arquivo como slice de bytes
    pub unsafe fn name(&self) -> &[u8] {
        let name_ptr = (self as *const Self as *const u8).add(19);
        let mut len = 0;
        while *name_ptr.add(len) != 0 {
            len += 1;
        }
        core::slice::from_raw_parts(name_ptr, len)
    }
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
// pthread - POSIX Threads support
// ============================================================================

pub mod pthread {
    use super::*;
    use core::sync::atomic::{AtomicI32, AtomicU32, Ordering};
    use core::ptr;

    // Clone flags for pthread_create
    pub const CLONE_VM: u64 = 0x00000100;
    pub const CLONE_FS: u64 = 0x00000200;
    pub const CLONE_FILES: u64 = 0x00000400;
    pub const CLONE_SIGHAND: u64 = 0x00000800;
    pub const CLONE_THREAD: u64 = 0x00010000;
    pub const CLONE_PARENT_SETTID: u64 = 0x00100000;
    pub const CLONE_CHILD_CLEARTID: u64 = 0x00200000;
    pub const CLONE_SETTLS: u64 = 0x00080000;
    pub const CLONE_CHILD_SETTID: u64 = 0x01000000;

    // Futex operations
    pub const FUTEX_WAIT: i32 = 0;
    pub const FUTEX_WAKE: i32 = 1;
    pub const FUTEX_PRIVATE_FLAG: i32 = 128;

    /// Thread handle
    pub type pthread_t = u64;

    /// Thread attributes (simplified)
    #[repr(C)]
    pub struct pthread_attr_t {
        pub stack_size: usize,
        pub detached: bool,
    }

    impl pthread_attr_t {
        pub const fn default() -> Self {
            Self {
                stack_size: 64 * 1024, // 64KB default
                detached: false,
            }
        }
    }

    /// Mutex (using futex)
    #[repr(C)]
    pub struct pthread_mutex_t {
        state: AtomicI32, // 0 = unlocked, 1 = locked (no waiters), 2 = locked (with waiters)
    }

    impl pthread_mutex_t {
        pub const fn new() -> Self {
            Self {
                state: AtomicI32::new(0),
            }
        }
    }

    /// Mutex attributes
    #[repr(C)]
    pub struct pthread_mutexattr_t {
        pub kind: i32,
    }

    /// Condition variable (using futex)
    #[repr(C)]
    pub struct pthread_cond_t {
        seq: AtomicU32, // Sequence number for wake
    }

    impl pthread_cond_t {
        pub const fn new() -> Self {
            Self {
                seq: AtomicU32::new(0),
            }
        }
    }

    /// Condition variable attributes
    #[repr(C)]
    pub struct pthread_condattr_t {
        pub _unused: i32,
    }

    /// Thread-local storage for thread info
    #[repr(C)]
    struct ThreadInfo {
        tid: AtomicI32,          // Thread ID (for child_tidptr)
        start_routine: fn(*mut u8) -> *mut u8,
        arg: *mut u8,
        result: *mut u8,
        joined: AtomicI32,
    }

    /// Stack size for threads
    const DEFAULT_STACK_SIZE: usize = 64 * 1024; // 64KB

    /// Futex syscall wrapper
    #[inline]
    pub fn futex(uaddr: *const i32, op: i32, val: u32, timeout: u64, uaddr2: u64, val3: u32) -> i64 {
        unsafe {
            syscall6(
                SYS_FUTEX,
                uaddr as u64,
                op as u64,
                val as u64,
                timeout,
                uaddr2,
                val3 as u64,
            )
        }
    }

    /// Clone syscall wrapper for thread creation
    #[inline]
    pub fn clone_thread(
        flags: u64,
        stack: *mut u8,
        parent_tidptr: *mut i32,
        child_tidptr: *mut i32,
        tls: u64,
    ) -> i64 {
        unsafe {
            syscall5(
                SYS_CLONE,
                flags,
                stack as u64,
                parent_tidptr as u64,
                child_tidptr as u64,
                tls,
            )
        }
    }

    /// gettid syscall wrapper
    #[inline]
    pub fn gettid() -> i32 {
        unsafe { syscall0(SYS_GETTID) as i32 }
    }

    /// Create a new thread
    ///
    /// # Arguments
    /// * `thread` - Where to store the new thread ID
    /// * `attr` - Thread attributes (can be null for defaults)
    /// * `start_routine` - Function to execute in the new thread
    /// * `arg` - Argument to pass to start_routine
    ///
    /// # Returns
    /// 0 on success, error code on failure
    pub fn pthread_create(
        thread: &mut pthread_t,
        _attr: *const pthread_attr_t,
        start_routine: fn(*mut u8) -> *mut u8,
        arg: *mut u8,
    ) -> i32 {
        // Allocate stack for new thread (using brk for simplicity)
        let stack_size = DEFAULT_STACK_SIZE;
        let stack_base = unsafe {
            let current = brk(ptr::null_mut());
            let new_brk = brk(current.add(stack_size + 64)); // Extra space for ThreadInfo
            if new_brk == current {
                return -1; // Out of memory
            }
            current
        };

        // Set up ThreadInfo at the beginning of the allocated space
        let thread_info = stack_base as *mut ThreadInfo;
        unsafe {
            (*thread_info).tid = AtomicI32::new(0);
            (*thread_info).start_routine = start_routine;
            (*thread_info).arg = arg;
            (*thread_info).result = ptr::null_mut();
            (*thread_info).joined = AtomicI32::new(0);
        }

        // Stack pointer (grows down) - align to 16 bytes
        let stack_top = unsafe { stack_base.add(stack_size + 64) };
        let stack_ptr = ((stack_top as usize) & !0xF) as *mut u8;

        // Clone flags for thread creation
        let flags = CLONE_VM | CLONE_FS | CLONE_FILES | CLONE_SIGHAND |
                    CLONE_THREAD | CLONE_PARENT_SETTID | CLONE_CHILD_CLEARTID;

        // Get address for parent_tidptr and child_tidptr
        let tid_ptr = unsafe { &(*thread_info).tid as *const AtomicI32 as *mut i32 };

        let result = clone_thread(
            flags,
            stack_ptr,
            tid_ptr,      // parent_tidptr
            tid_ptr,      // child_tidptr (same, will be cleared on exit)
            0,            // No TLS for now
        );

        if result < 0 {
            return result as i32;
        }

        if result == 0 {
            // Child thread - call start routine
            let info = thread_info;
            let func = unsafe { (*info).start_routine };
            let arg = unsafe { (*info).arg };

            let ret = func(arg);

            // Store result
            unsafe { (*info).result = ret; }

            // Exit thread
            exit(0);
        }

        // Parent - store thread handle
        *thread = thread_info as u64;

        0
    }

    /// Wait for a thread to terminate
    ///
    /// # Arguments
    /// * `thread` - Thread to wait for
    /// * `retval` - Where to store the return value (can be null)
    ///
    /// # Returns
    /// 0 on success, error code on failure
    pub fn pthread_join(thread: pthread_t, retval: *mut *mut u8) -> i32 {
        let info = thread as *mut ThreadInfo;

        // Wait for thread to exit using futex on tid
        // The kernel will write 0 to tid and do futex_wake when thread exits
        let tid_ptr = unsafe { &(*info).tid as *const AtomicI32 as *const i32 };

        loop {
            let tid = unsafe { (*info).tid.load(Ordering::Acquire) };
            if tid == 0 {
                break; // Thread has exited
            }

            // Wait on futex
            futex(tid_ptr, FUTEX_WAIT | FUTEX_PRIVATE_FLAG, tid as u32, 0, 0, 0);
        }

        // Get return value
        if !retval.is_null() {
            unsafe { *retval = (*info).result; }
        }

        0
    }

    /// Terminate calling thread
    pub fn pthread_exit(retval: *mut u8) -> ! {
        // Store return value if we have access to our ThreadInfo
        // For simplicity, just exit
        let _ = retval;
        exit(0)
    }

    /// Get the calling thread's ID
    pub fn pthread_self() -> pthread_t {
        gettid() as pthread_t
    }

    // ========================================================================
    // Mutex operations
    // ========================================================================

    /// Initialize a mutex
    pub fn pthread_mutex_init(mutex: *mut pthread_mutex_t, _attr: *const pthread_mutexattr_t) -> i32 {
        unsafe {
            (*mutex).state.store(0, Ordering::Release);
        }
        0
    }

    /// Destroy a mutex
    pub fn pthread_mutex_destroy(_mutex: *mut pthread_mutex_t) -> i32 {
        0 // Nothing to do
    }

    /// Lock a mutex
    pub fn pthread_mutex_lock(mutex: *mut pthread_mutex_t) -> i32 {
        unsafe {
            // Fast path: try to acquire unlocked mutex
            if (*mutex).state.compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed).is_ok() {
                return 0;
            }

            // Slow path: mutex is locked
            loop {
                // Try to set state to 2 (locked with waiters)
                let old = (*mutex).state.swap(2, Ordering::Acquire);
                if old == 0 {
                    // We acquired it!
                    return 0;
                }

                // Wait on futex
                let state_ptr = &(*mutex).state as *const AtomicI32 as *const i32;
                futex(state_ptr, FUTEX_WAIT | FUTEX_PRIVATE_FLAG, 2, 0, 0, 0);
            }
        }
    }

    /// Try to lock a mutex (non-blocking)
    pub fn pthread_mutex_trylock(mutex: *mut pthread_mutex_t) -> i32 {
        unsafe {
            if (*mutex).state.compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed).is_ok() {
                0
            } else {
                -1 // EBUSY
            }
        }
    }

    /// Unlock a mutex
    pub fn pthread_mutex_unlock(mutex: *mut pthread_mutex_t) -> i32 {
        unsafe {
            // Set to unlocked
            let old = (*mutex).state.swap(0, Ordering::Release);

            // If there were waiters, wake one
            if old == 2 {
                let state_ptr = &(*mutex).state as *const AtomicI32 as *const i32;
                futex(state_ptr, FUTEX_WAKE | FUTEX_PRIVATE_FLAG, 1, 0, 0, 0);
            }
        }
        0
    }

    // ========================================================================
    // Condition variable operations
    // ========================================================================

    /// Initialize a condition variable
    pub fn pthread_cond_init(cond: *mut pthread_cond_t, _attr: *const pthread_condattr_t) -> i32 {
        unsafe {
            (*cond).seq.store(0, Ordering::Release);
        }
        0
    }

    /// Destroy a condition variable
    pub fn pthread_cond_destroy(_cond: *mut pthread_cond_t) -> i32 {
        0 // Nothing to do
    }

    /// Wait on a condition variable
    pub fn pthread_cond_wait(cond: *mut pthread_cond_t, mutex: *mut pthread_mutex_t) -> i32 {
        unsafe {
            let seq = (*cond).seq.load(Ordering::Acquire);

            // Release mutex
            pthread_mutex_unlock(mutex);

            // Wait for signal
            let seq_ptr = &(*cond).seq as *const AtomicU32 as *const i32;
            futex(seq_ptr, FUTEX_WAIT | FUTEX_PRIVATE_FLAG, seq, 0, 0, 0);

            // Reacquire mutex
            pthread_mutex_lock(mutex);
        }
        0
    }

    /// Signal a condition variable (wake one waiter)
    pub fn pthread_cond_signal(cond: *mut pthread_cond_t) -> i32 {
        unsafe {
            (*cond).seq.fetch_add(1, Ordering::Release);
            let seq_ptr = &(*cond).seq as *const AtomicU32 as *const i32;
            futex(seq_ptr, FUTEX_WAKE | FUTEX_PRIVATE_FLAG, 1, 0, 0, 0);
        }
        0
    }

    /// Broadcast to a condition variable (wake all waiters)
    pub fn pthread_cond_broadcast(cond: *mut pthread_cond_t) -> i32 {
        unsafe {
            (*cond).seq.fetch_add(1, Ordering::Release);
            let seq_ptr = &(*cond).seq as *const AtomicU32 as *const i32;
            futex(seq_ptr, FUTEX_WAKE | FUTEX_PRIVATE_FLAG, i32::MAX as u32, 0, 0, 0);
        }
        0
    }
}

// Re-export pthread types at crate level for convenience
pub use pthread::{
    pthread_t, pthread_attr_t, pthread_mutex_t, pthread_mutexattr_t,
    pthread_cond_t, pthread_condattr_t,
    pthread_create, pthread_join, pthread_exit, pthread_self,
    pthread_mutex_init, pthread_mutex_destroy, pthread_mutex_lock,
    pthread_mutex_trylock, pthread_mutex_unlock,
    pthread_cond_init, pthread_cond_destroy, pthread_cond_wait,
    pthread_cond_signal, pthread_cond_broadcast,
};

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
