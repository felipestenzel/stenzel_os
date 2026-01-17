//! Implementação de syscalls para userspace.
//!
//! File descriptor table por processo, syscalls padrão Linux x86_64.
//!
//! NOTA: Constantes de syscall e errno mantidas para compatibilidade Linux.

#![allow(dead_code)]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use crate::fs::{self, Inode, InodeKind, Mode};
use crate::security::Cred;
use crate::sync::IrqSafeMutex;
use crate::util::KError;

// ======================== User address validation ========================

/// Maximum valid user-space address (canonical address boundary)
/// On x86_64, user space is typically 0x0000_0000_0000_0000 to 0x0000_7FFF_FFFF_FFFF
const USER_SPACE_END: u64 = 0x0000_7FFF_FFFF_FFFF;

/// Validates that a pointer is within user address space
#[inline]
pub fn is_user_addr(addr: u64) -> bool {
    addr <= USER_SPACE_END
}

/// Validates that a memory range is within user address space
#[inline]
pub fn is_user_range(addr: u64, len: usize) -> bool {
    if addr == 0 || len == 0 {
        return false;
    }
    // Check for overflow
    let end = match addr.checked_add(len as u64) {
        Some(e) => e,
        None => return false,
    };
    addr <= USER_SPACE_END && end <= USER_SPACE_END + 1
}

/// Validates that a user pointer is non-null and in user space, then returns it as a slice reference
///
/// # Safety
/// Caller must ensure the memory is actually mapped and accessible
#[inline]
pub unsafe fn validate_user_buffer(addr: u64, len: usize) -> Option<&'static [u8]> {
    if !is_user_range(addr, len) {
        return None;
    }
    Some(core::slice::from_raw_parts(addr as *const u8, len))
}

/// Validates that a user pointer is non-null and in user space, then returns it as a mutable slice reference
///
/// # Safety
/// Caller must ensure the memory is actually mapped and accessible
#[inline]
pub unsafe fn validate_user_buffer_mut(addr: u64, len: usize) -> Option<&'static mut [u8]> {
    if !is_user_range(addr, len) {
        return None;
    }
    Some(core::slice::from_raw_parts_mut(addr as *mut u8, len))
}

// ======================== Pipe implementation ========================

const PIPE_BUF_SIZE: usize = 4096;

/// Buffer circular para pipe.
struct PipeBuffer {
    data: UnsafeCell<[u8; PIPE_BUF_SIZE]>,
    read_pos: AtomicUsize,
    write_pos: AtomicUsize,
    /// Número de bytes disponíveis para leitura.
    count: AtomicUsize,
}

impl PipeBuffer {
    fn new() -> Self {
        Self {
            data: UnsafeCell::new([0u8; PIPE_BUF_SIZE]),
            read_pos: AtomicUsize::new(0),
            write_pos: AtomicUsize::new(0),
            count: AtomicUsize::new(0),
        }
    }

    fn is_empty(&self) -> bool {
        self.count.load(Ordering::Acquire) == 0
    }

    fn is_full(&self) -> bool {
        self.count.load(Ordering::Acquire) >= PIPE_BUF_SIZE
    }

    fn available(&self) -> usize {
        self.count.load(Ordering::Acquire)
    }

    fn space(&self) -> usize {
        PIPE_BUF_SIZE - self.count.load(Ordering::Acquire)
    }

    /// Lê até `max` bytes do buffer. Retorna quantos foram lidos.
    fn read(&self, out: &mut [u8], max: usize) -> usize {
        let to_read = core::cmp::min(max, core::cmp::min(out.len(), self.available()));
        if to_read == 0 {
            return 0;
        }

        let data = unsafe { &*self.data.get() };
        let mut read_pos = self.read_pos.load(Ordering::Acquire);

        for i in 0..to_read {
            out[i] = data[read_pos];
            read_pos = (read_pos + 1) % PIPE_BUF_SIZE;
        }

        self.read_pos.store(read_pos, Ordering::Release);
        self.count.fetch_sub(to_read, Ordering::AcqRel);
        to_read
    }

    /// Escreve até `max` bytes no buffer. Retorna quantos foram escritos.
    fn write(&self, input: &[u8], max: usize) -> usize {
        let to_write = core::cmp::min(max, core::cmp::min(input.len(), self.space()));
        if to_write == 0 {
            return 0;
        }

        let data = unsafe { &mut *self.data.get() };
        let mut write_pos = self.write_pos.load(Ordering::Acquire);

        for i in 0..to_write {
            data[write_pos] = input[i];
            write_pos = (write_pos + 1) % PIPE_BUF_SIZE;
        }

        self.write_pos.store(write_pos, Ordering::Release);
        self.count.fetch_add(to_write, Ordering::AcqRel);
        to_write
    }
}

unsafe impl Send for PipeBuffer {}
unsafe impl Sync for PipeBuffer {}

/// Pipe: canal unidirecional de comunicação entre processos.
pub struct Pipe {
    buffer: PipeBuffer,
    /// Contador de referências ao lado de leitura.
    readers: AtomicUsize,
    /// Contador de referências ao lado de escrita.
    writers: AtomicUsize,
}

impl Pipe {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            buffer: PipeBuffer::new(),
            readers: AtomicUsize::new(1),
            writers: AtomicUsize::new(1),
        })
    }

    pub fn add_reader(&self) {
        self.readers.fetch_add(1, Ordering::AcqRel);
    }

    pub fn add_writer(&self) {
        self.writers.fetch_add(1, Ordering::AcqRel);
    }

    pub fn remove_reader(&self) {
        self.readers.fetch_sub(1, Ordering::AcqRel);
    }

    pub fn remove_writer(&self) {
        self.writers.fetch_sub(1, Ordering::AcqRel);
    }

    pub fn has_readers(&self) -> bool {
        self.readers.load(Ordering::Acquire) > 0
    }

    pub fn has_writers(&self) -> bool {
        self.writers.load(Ordering::Acquire) > 0
    }

    /// Lê do pipe. Retorna 0 se não há writers e buffer vazio (EOF).
    pub fn read(&self, out: &mut [u8]) -> usize {
        self.buffer.read(out, out.len())
    }

    /// Escreve no pipe. Retorna 0 se não há readers (EPIPE).
    pub fn write(&self, input: &[u8]) -> usize {
        if !self.has_readers() {
            return 0;
        }
        self.buffer.write(input, input.len())
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.buffer.is_full()
    }

    /// Returns the number of bytes available to read
    pub fn available(&self) -> usize {
        self.buffer.available()
    }
}

/// Números de syscall (ABI Linux x86_64).
pub mod nr {
    pub const READ: u64 = 0;
    pub const WRITE: u64 = 1;
    pub const OPEN: u64 = 2;
    pub const CLOSE: u64 = 3;
    pub const STAT: u64 = 4;
    pub const FSTAT: u64 = 5;
    pub const LSEEK: u64 = 8;
    pub const MMAP: u64 = 9;
    pub const BRK: u64 = 12;
    pub const IOCTL: u64 = 16;
    pub const ACCESS: u64 = 21;
    pub const PIPE: u64 = 22;
    pub const DUP: u64 = 32;
    pub const DUP2: u64 = 33;
    pub const GETPID: u64 = 39;
    pub const FORK: u64 = 57;
    pub const EXECVE: u64 = 59;
    pub const EXIT: u64 = 60;
    pub const WAIT4: u64 = 61;
    pub const KILL: u64 = 62;
    pub const WAITID: u64 = 247;
    pub const UNAME: u64 = 63;
    pub const GETCWD: u64 = 79;
    pub const CHDIR: u64 = 80;
    pub const MKDIR: u64 = 83;
    pub const RMDIR: u64 = 84;
    pub const UNLINK: u64 = 87;
    // Permission syscalls
    pub const CHMOD: u64 = 90;
    pub const FCHMOD: u64 = 91;
    pub const CHOWN: u64 = 92;
    pub const FCHOWN: u64 = 93;
    pub const LCHOWN: u64 = 94;
    pub const GETUID: u64 = 102;
    pub const GETGID: u64 = 104;
    pub const SETUID: u64 = 105;
    pub const SETGID: u64 = 106;
    pub const GETEUID: u64 = 107;
    pub const GETEGID: u64 = 108;
    pub const SETREUID: u64 = 113;
    pub const SETREGID: u64 = 114;
    pub const GETGROUPS: u64 = 115;
    pub const SETGROUPS: u64 = 116;
    pub const SETRESUID: u64 = 117;
    pub const GETRESUID: u64 = 118;
    pub const SETRESGID: u64 = 119;
    pub const GETRESGID: u64 = 120;
    pub const MPROTECT: u64 = 10;
    pub const MUNMAP: u64 = 11;
    pub const ARCH_PRCTL: u64 = 158;
    pub const OPENAT: u64 = 257;
    // Socket syscalls
    pub const SOCKET: u64 = 41;
    pub const CONNECT: u64 = 42;
    pub const ACCEPT: u64 = 43;
    pub const SENDTO: u64 = 44;
    pub const RECVFROM: u64 = 45;
    pub const BIND: u64 = 49;
    pub const LISTEN: u64 = 50;
    pub const GETSOCKNAME: u64 = 51;
    pub const GETPEERNAME: u64 = 52;
    pub const SETSOCKOPT: u64 = 54;
    pub const GETSOCKOPT: u64 = 55;
    pub const SHUTDOWN: u64 = 48;
    // Time syscalls
    pub const NANOSLEEP: u64 = 35;
    pub const GETTIMEOFDAY: u64 = 96;
    pub const CLOCK_GETTIME: u64 = 228;
    pub const CLOCK_GETRES: u64 = 229;
    // I/O multiplexing syscalls
    pub const POLL: u64 = 7;
    pub const SELECT: u64 = 23;
    pub const PSELECT6: u64 = 270;
    pub const PPOLL: u64 = 271;
    // Thread syscalls
    pub const CLONE: u64 = 56;
    pub const CLONE3: u64 = 435;
    pub const SET_TID_ADDRESS: u64 = 218;
    pub const GETTID: u64 = 186;
    pub const GETPPID: u64 = 110;
    pub const SCHED_YIELD: u64 = 24;
    pub const SCHED_SETAFFINITY: u64 = 203;
    pub const SCHED_GETAFFINITY: u64 = 204;
    pub const EXIT_GROUP: u64 = 231;
    pub const TGKILL: u64 = 234;
    pub const TKILL: u64 = 200;
    // Signal syscalls
    pub const RT_SIGACTION: u64 = 13;
    pub const RT_SIGPROCMASK: u64 = 14;
    pub const RT_SIGRETURN: u64 = 15;
    pub const SIGALTSTACK: u64 = 131;
    pub const RT_SIGPENDING: u64 = 127;
    pub const RT_SIGSUSPEND: u64 = 130;
    // Futex syscall
    pub const FUTEX: u64 = 202;
    // Process group/session syscalls
    pub const SETPGID: u64 = 109;
    pub const GETPGID: u64 = 121;
    pub const GETPGRP: u64 = 111;
    pub const SETSID: u64 = 112;
    pub const GETSID: u64 = 124;
    // Directory syscalls
    pub const GETDENTS64: u64 = 217;
    // Symlink syscalls
    pub const SYMLINK: u64 = 88;
    pub const READLINK: u64 = 89;
    pub const SYMLINKAT: u64 = 266;
    pub const READLINKAT: u64 = 267;
    // Rename
    pub const RENAME: u64 = 82;
    pub const RENAMEAT: u64 = 264;
    pub const RENAMEAT2: u64 = 316;
    // Stat
    pub const LSTAT: u64 = 6;
    pub const NEWFSTATAT: u64 = 262;
    // Access
    pub const FACCESSAT: u64 = 269;
    pub const FACCESSAT2: u64 = 439;
    // File truncation
    pub const TRUNCATE: u64 = 76;
    pub const FTRUNCATE: u64 = 77;
    // File sync
    pub const FSYNC: u64 = 74;
    pub const FDATASYNC: u64 = 75;
    // Special file creation
    pub const MKNOD: u64 = 133;
    pub const MKNODAT: u64 = 259;
    // Shared memory syscalls
    pub const SHMGET: u64 = 29;
    pub const SHMAT: u64 = 30;
    pub const SHMCTL: u64 = 31;
    pub const SHMDT: u64 = 67;
    // Message queue syscalls
    pub const MSGGET: u64 = 68;
    pub const MSGSND: u64 = 69;
    pub const MSGRCV: u64 = 70;
    pub const MSGCTL: u64 = 71;
    // eventfd syscalls
    pub const EVENTFD: u64 = 284;
    pub const EVENTFD2: u64 = 290;
    // File descriptor control
    pub const FCNTL: u64 = 72;
    // Resource limits
    pub const GETRLIMIT: u64 = 97;
    pub const SETRLIMIT: u64 = 160;
    pub const PRLIMIT64: u64 = 302;
    // Time
    pub const SETTIMEOFDAY: u64 = 164;
    // Reboot/shutdown
    pub const REBOOT: u64 = 169;
}

/// Erros de syscall (negativo = errno).
pub mod errno {
    pub const EPERM: i64 = -1;
    pub const ENOENT: i64 = -2;
    pub const ESRCH: i64 = -3;
    pub const EINTR: i64 = -4;
    pub const EIO: i64 = -5;
    pub const ENXIO: i64 = -6;
    pub const ENOEXEC: i64 = -8;
    pub const EBADF: i64 = -9;
    pub const ECHILD: i64 = -10;
    pub const EAGAIN: i64 = -11;
    pub const ENOMEM: i64 = -12;
    pub const EACCES: i64 = -13;
    pub const EFAULT: i64 = -14;
    pub const ENOTDIR: i64 = -20;
    pub const EISDIR: i64 = -21;
    pub const EINVAL: i64 = -22;
    pub const EMFILE: i64 = -24;
    pub const ENOSPC: i64 = -28;
    pub const ESPIPE: i64 = -29;
    pub const EPIPE: i64 = -32;
    pub const ENOSYS: i64 = -38;
    pub const ENOTEMPTY: i64 = -39;
    pub const ENOTSOCK: i64 = -88;
    pub const EADDRINUSE: i64 = -98;
    pub const ECONNREFUSED: i64 = -111;
    pub const ENETUNREACH: i64 = -101;
    pub const ENOTCONN: i64 = -107;
    pub const ETIMEDOUT: i64 = -110;
    pub const EALREADY: i64 = -114;
    pub const EAFNOSUPPORT: i64 = -97;
    pub const ELOOP: i64 = -40;
    pub const ENAMETOOLONG: i64 = -36;
    pub const EEXIST: i64 = -17;
    pub const EOPNOTSUPP: i64 = -95;
    pub const E2BIG: i64 = -7;      // Argument list too long
    pub const ENOMSG: i64 = -42;    // No message of desired type
    pub const EIDRM: i64 = -43;     // Identifier removed
}

// ======================== Resource Limits ========================

/// Resource limit constants
pub mod rlimit {
    pub const RLIMIT_CPU: u32 = 0;        // CPU time in seconds
    pub const RLIMIT_FSIZE: u32 = 1;      // Maximum file size
    pub const RLIMIT_DATA: u32 = 2;       // Max data segment size
    pub const RLIMIT_STACK: u32 = 3;      // Max stack size
    pub const RLIMIT_CORE: u32 = 4;       // Max core file size
    pub const RLIMIT_RSS: u32 = 5;        // Max resident set size
    pub const RLIMIT_NPROC: u32 = 6;      // Max number of processes
    pub const RLIMIT_NOFILE: u32 = 7;     // Max number of open files
    pub const RLIMIT_MEMLOCK: u32 = 8;    // Max locked-in-memory address space
    pub const RLIMIT_AS: u32 = 9;         // Address space limit
    pub const RLIMIT_LOCKS: u32 = 10;     // Max file locks held
    pub const RLIMIT_SIGPENDING: u32 = 11; // Max number of pending signals
    pub const RLIMIT_MSGQUEUE: u32 = 12;  // Max bytes in POSIX message queues
    pub const RLIMIT_NICE: u32 = 13;      // Max nice value
    pub const RLIMIT_RTPRIO: u32 = 14;    // Max real-time priority
    pub const RLIMIT_RTTIME: u32 = 15;    // Real-time timeout
    pub const RLIM_NLIMITS: u32 = 16;     // Number of limit types

    pub const RLIM_INFINITY: u64 = u64::MAX;
}

/// Resource limit value (soft/hard limits)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Rlimit {
    pub rlim_cur: u64,  // Soft limit
    pub rlim_max: u64,  // Hard limit (ceiling for rlim_cur)
}

impl Default for Rlimit {
    fn default() -> Self {
        Self {
            rlim_cur: rlimit::RLIM_INFINITY,
            rlim_max: rlimit::RLIM_INFINITY,
        }
    }
}

/// Per-process resource limits
#[derive(Clone)]
pub struct ResourceLimits {
    limits: [Rlimit; rlimit::RLIM_NLIMITS as usize],
}

impl Default for ResourceLimits {
    fn default() -> Self {
        let mut limits = [Rlimit::default(); rlimit::RLIM_NLIMITS as usize];

        // Set some sensible defaults
        limits[rlimit::RLIMIT_NOFILE as usize] = Rlimit { rlim_cur: 1024, rlim_max: 4096 };
        limits[rlimit::RLIMIT_NPROC as usize] = Rlimit { rlim_cur: 4096, rlim_max: 4096 };
        limits[rlimit::RLIMIT_STACK as usize] = Rlimit { rlim_cur: 8 * 1024 * 1024, rlim_max: rlimit::RLIM_INFINITY };
        limits[rlimit::RLIMIT_CORE as usize] = Rlimit { rlim_cur: 0, rlim_max: rlimit::RLIM_INFINITY }; // Disabled by default

        Self { limits }
    }
}

impl ResourceLimits {
    pub fn get(&self, resource: u32) -> Option<Rlimit> {
        if resource < rlimit::RLIM_NLIMITS {
            Some(self.limits[resource as usize])
        } else {
            None
        }
    }

    pub fn set(&mut self, resource: u32, limit: Rlimit) -> bool {
        if resource < rlimit::RLIM_NLIMITS {
            self.limits[resource as usize] = limit;
            true
        } else {
            false
        }
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct OpenFlags: u32 {
        const O_RDONLY    = 0;
        const O_WRONLY    = 1;
        const O_RDWR      = 2;
        const O_CREAT     = 0o100;
        const O_EXCL      = 0o200;
        const O_TRUNC     = 0o1000;
        const O_APPEND    = 0o2000;
        const O_NONBLOCK  = 0o4000;
        const O_DIRECTORY = 0o200000;
        const O_CLOEXEC   = 0o2000000;
    }
}

/// Tipo de file descriptor.
#[derive(Clone)]
pub enum FdType {
    /// Console (stdin/stdout/stderr).
    Console,
    /// Arquivo no VFS.
    File { inode: Inode, offset: usize, flags: OpenFlags },
    /// Diretório aberto.
    Dir { inode: Inode, offset: usize },
    /// Lado de leitura de um pipe.
    PipeRead { pipe: Arc<Pipe> },
    /// Lado de escrita de um pipe.
    PipeWrite { pipe: Arc<Pipe> },
    /// Socket de rede.
    Socket { socket_id: u64 },
    /// eventfd for event notification.
    EventFd { eventfd: crate::ipc::EventFdFile },
}

/// File descriptor entry.
#[derive(Clone)]
pub struct FdEntry {
    pub fd_type: FdType,
}

/// Tabela de file descriptors por processo.
pub struct FdTable {
    fds: BTreeMap<i32, FdEntry>,
    next_fd: i32,
}

impl FdTable {
    pub fn new() -> Self {
        let mut table = Self {
            fds: BTreeMap::new(),
            next_fd: 3,
        };
        // FDs padrão: 0=stdin, 1=stdout, 2=stderr (console).
        table.fds.insert(0, FdEntry { fd_type: FdType::Console });
        table.fds.insert(1, FdEntry { fd_type: FdType::Console });
        table.fds.insert(2, FdEntry { fd_type: FdType::Console });
        table
    }

    pub fn alloc(&mut self) -> i32 {
        let fd = self.next_fd;
        self.next_fd += 1;
        fd
    }

    pub fn insert(&mut self, fd: i32, entry: FdEntry) {
        self.fds.insert(fd, entry);
    }

    pub fn get(&self, fd: i32) -> Option<&FdEntry> {
        self.fds.get(&fd)
    }

    pub fn get_mut(&mut self, fd: i32) -> Option<&mut FdEntry> {
        self.fds.get_mut(&fd)
    }

    pub fn remove(&mut self, fd: i32) -> Option<FdEntry> {
        self.fds.remove(&fd)
    }
}

/// Tabela de FDs global (single-process por enquanto).
static FD_TABLE: IrqSafeMutex<Option<FdTable>> = IrqSafeMutex::new(None);

fn fd_table() -> crate::sync::IrqSafeGuard<'static, Option<FdTable>> {
    FD_TABLE.lock()
}

pub fn init() {
    let mut table = fd_table();
    *table = Some(FdTable::new());
    crate::kprintln!("syscall: fd table inicializada");
}

/// Allocate a file descriptor for an eventfd
pub fn alloc_fd_for_eventfd(eventfd: crate::ipc::EventFdFile) -> Option<i32> {
    let mut table = fd_table();
    let table = table.as_mut()?;

    let fd = table.alloc();
    table.insert(fd, FdEntry {
        fd_type: FdType::EventFd { eventfd },
    });

    Some(fd)
}

/// Lê uma string C terminada em nulo de memória do usuário.
///
/// Valida que o endereço está em user space antes de acessar.
pub unsafe fn read_user_string(ptr: u64, max_len: usize) -> Option<String> {
    // Valida endereço base
    if ptr == 0 || !is_user_addr(ptr) {
        return None;
    }

    // Verifica se o range máximo não ultrapassa user space
    if !is_user_range(ptr, max_len) {
        return None;
    }

    let mut s = String::new();
    let base = ptr as *const u8;
    for i in 0..max_len {
        // Verifica cada byte individualmente para o caso de strings longas
        let addr = ptr + i as u64;
        if !is_user_addr(addr) {
            return if s.is_empty() { None } else { Some(s) };
        }
        let c = *base.add(i);
        if c == 0 {
            return Some(s);
        }
        s.push(c as char);
    }
    Some(s)
}

/// Obtém credenciais do processo atual.
fn current_cred() -> Cred {
    crate::task::current_cred()
}

// ======================== Syscall implementations ========================

pub fn sys_read(fd: i32, buf: u64, count: usize) -> i64 {
    // Validate buffer address
    if count > 0 && !is_user_range(buf, count) {
        return errno::EFAULT;
    }

    // Primeiro verifica se é Console (precisa de tratamento especial para blocking).
    // Liberamos o lock antes de bloquear para evitar deadlocks.
    let is_console = {
        let guard = fd_table();
        match guard.as_ref() {
            Some(t) => matches!(t.get(fd).map(|e| &e.fd_type), Some(FdType::Console)),
            None => return errno::EBADF,
        }
    }; // guard é dropado aqui automaticamente

    if is_console {
        // Define este processo como foreground (para receber SIGINT)
        if fd == 0 {
            crate::sched::set_foreground(crate::sched::current_pid());
        }
        // Lê do console com blocking.
        if count == 0 {
            return 0;
        }
        let mut read_count = 0usize;
        let out = buf as *mut u8;
        loop {
            if let Some(b) = crate::console::read_byte() {
                unsafe { *out.add(read_count) = b; }
                read_count += 1;
                // Line-based: se for newline, para.
                if b == b'\n' || b == b'\r' {
                    break;
                }
                if read_count >= count {
                    break;
                }
            } else if read_count > 0 {
                // Já lemos algo, retorna o que temos.
                break;
            } else {
                // Nada lido ainda, bloqueia (yield) e tenta de novo.
                crate::task::yield_now();
            }
        }
        return read_count as i64;
    }

    // Para outros tipos de FD, mantém o lock durante a leitura.
    let mut table = fd_table();
    let table = match table.as_mut() {
        Some(t) => t,
        None => return errno::EBADF,
    };

    let entry = match table.get_mut(fd) {
        Some(e) => e,
        None => return errno::EBADF,
    };

    match &mut entry.fd_type {
        FdType::Console => {
            // Já tratado acima, não deveria chegar aqui.
            unreachable!()
        }
        FdType::File { inode, offset, flags: _ } => {
            if count == 0 {
                return 0;
            }
            let mut buf_vec = vec![0u8; count];
            match inode.0.read_at(*offset, &mut buf_vec) {
                Ok(n) => {
                    unsafe {
                        core::ptr::copy_nonoverlapping(buf_vec.as_ptr(), buf as *mut u8, n);
                    }
                    *offset += n;
                    n as i64
                }
                Err(_) => errno::EIO,
            }
        }
        FdType::Dir { .. } => errno::EISDIR,
        FdType::PipeRead { pipe } => {
            if count == 0 {
                return 0;
            }
            // Se o pipe está vazio e não há writers, retorna EOF (0)
            if pipe.is_empty() && !pipe.has_writers() {
                return 0;
            }
            // Lê do pipe
            let mut buf_vec = vec![0u8; count];
            let n = pipe.read(&mut buf_vec);
            if n > 0 {
                unsafe {
                    core::ptr::copy_nonoverlapping(buf_vec.as_ptr(), buf as *mut u8, n);
                }
            }
            n as i64
        }
        FdType::PipeWrite { .. } => errno::EBADF, // Não pode ler de um pipe de escrita
        FdType::Socket { socket_id } => {
            if count == 0 {
                return 0;
            }
            let sock_id = *socket_id;
            // Lock já foi liberado ao sair do escopo do guard original
            let mut buf_vec = vec![0u8; count];
            match crate::net::socket::recv(sock_id, &mut buf_vec) {
                Ok(n) => {
                    unsafe {
                        core::ptr::copy_nonoverlapping(buf_vec.as_ptr(), buf as *mut u8, n);
                    }
                    n as i64
                }
                Err(_) => errno::EIO,
            }
        }
        FdType::EventFd { eventfd } => {
            if count < 8 {
                return errno::EINVAL;
            }
            let mut buf_vec = [0u8; 8];
            match eventfd.read(&mut buf_vec) {
                Ok(n) => {
                    unsafe {
                        core::ptr::copy_nonoverlapping(buf_vec.as_ptr(), buf as *mut u8, n);
                    }
                    n as i64
                }
                Err(e) => e as i64,
            }
        }
    }
}

pub fn sys_write(fd: i32, buf: u64, count: usize) -> i64 {
    // Validate buffer address
    if count > 0 && !is_user_range(buf, count) {
        return errno::EFAULT;
    }

    let mut table = fd_table();
    let table = match table.as_mut() {
        Some(t) => t,
        None => return errno::EBADF,
    };

    let entry = match table.get_mut(fd) {
        Some(e) => e,
        None => return errno::EBADF,
    };

    match &mut entry.fd_type {
        FdType::Console => {
            // Escreve no console (serial).
            if count == 0 {
                return 0;
            }
            let max = core::cmp::min(count, 4096);
            let slice = unsafe { core::slice::from_raw_parts(buf as *const u8, max) };
            for &b in slice {
                crate::serial::write_byte(b);
            }
            max as i64
        }
        FdType::File { inode, offset, flags } => {
            if count == 0 {
                return 0;
            }
            // Verifica se é append.
            let write_offset = if flags.contains(OpenFlags::O_APPEND) {
                inode.0.size().unwrap_or(0)
            } else {
                *offset
            };
            let slice = unsafe { core::slice::from_raw_parts(buf as *const u8, count) };
            match inode.0.write_at(write_offset, slice) {
                Ok(n) => {
                    *offset = write_offset + n;
                    n as i64
                }
                Err(_) => errno::EIO,
            }
        }
        FdType::Dir { .. } => errno::EISDIR,
        FdType::PipeWrite { pipe } => {
            if count == 0 {
                return 0;
            }
            // Se não há readers, envia SIGPIPE e retorna EPIPE
            if !pipe.has_readers() {
                // Envia SIGPIPE para o processo atual
                crate::sched::current_task().signals().send(crate::signal::sig::SIGPIPE);
                return errno::EPIPE;
            }
            // Escreve no pipe
            let slice = unsafe { core::slice::from_raw_parts(buf as *const u8, count) };
            let n = pipe.write(slice);
            n as i64
        }
        FdType::PipeRead { .. } => errno::EBADF, // Não pode escrever em um pipe de leitura
        FdType::Socket { socket_id } => {
            if count == 0 {
                return 0;
            }
            let slice = unsafe { core::slice::from_raw_parts(buf as *const u8, count) };
            let sock_id = *socket_id;
            // Lock já foi liberado ao sair do escopo do guard original
            match crate::net::socket::send(sock_id, slice) {
                Ok(n) => n as i64,
                Err(_) => errno::EIO,
            }
        }
        FdType::EventFd { eventfd } => {
            if count < 8 {
                return errno::EINVAL;
            }
            let slice = unsafe { core::slice::from_raw_parts(buf as *const u8, 8) };
            match eventfd.write(slice) {
                Ok(n) => n as i64,
                Err(e) => e as i64,
            }
        }
    }
}

pub fn sys_open(pathname: u64, flags: u32, mode: u32) -> i64 {
    let path = match unsafe { read_user_string(pathname, 1024) } {
        Some(p) => p,
        None => return errno::EFAULT,
    };

    let oflags = OpenFlags::from_bits_truncate(flags);
    let cred = current_cred();

    let vfs = fs::vfs_lock();

    // Tenta resolver o path.
    let inode = match vfs.resolve(&path, &cred) {
        Ok(i) => i,
        Err(KError::NotFound) => {
            // Se O_CREAT, tenta criar.
            if oflags.contains(OpenFlags::O_CREAT) {
                // Encontra o parent e cria arquivo.
                let (parent_path, name) = match path.rfind('/') {
                    Some(idx) if idx > 0 => (&path[..idx], &path[idx + 1..]),
                    Some(_) => ("/", &path[1..]),
                    None => ("/", path.as_str()),
                };
                let parent = match vfs.resolve(parent_path, &cred) {
                    Ok(p) => p,
                    Err(_) => return errno::ENOENT,
                };
                let meta = fs::Metadata::simple(
                    cred.uid,
                    cred.gid,
                    Mode::from_octal(mode as u16),
                    InodeKind::File,
                );
                match parent.0.create(name, InodeKind::File, meta) {
                    Ok(i) => i,
                    Err(_) => return errno::EIO,
                }
            } else {
                return errno::ENOENT;
            }
        }
        Err(KError::PermissionDenied) => return errno::EACCES,
        Err(_) => return errno::EIO,
    };

    // Verifica se é diretório quando O_DIRECTORY.
    if oflags.contains(OpenFlags::O_DIRECTORY) && inode.kind() != InodeKind::Dir {
        return errno::ENOTDIR;
    }

    // Check file permissions based on open flags
    let meta = inode.metadata();
    let access_mode = oflags.bits() & 0o3; // O_RDONLY=0, O_WRONLY=1, O_RDWR=2

    // For directories, we check different permissions
    if meta.kind == InodeKind::Dir {
        // Directories require execute permission for traversal
        if !fs::perm::can_exec_dir(&meta, &cred) {
            return errno::EACCES;
        }
    } else {
        // For files, check read/write permissions based on flags
        match access_mode {
            0 => { // O_RDONLY
                if !fs::perm::can_read_file(&meta, &cred) {
                    return errno::EACCES;
                }
            }
            1 => { // O_WRONLY
                if !fs::perm::can_write_file(&meta, &cred) {
                    return errno::EACCES;
                }
            }
            2 => { // O_RDWR
                if !fs::perm::can_read_write(&meta, &cred) {
                    return errno::EACCES;
                }
            }
            _ => {} // O_PATH or other special modes - no permission check
        }
    }

    // Aloca fd.
    drop(vfs);
    let mut table = fd_table();
    let table = match table.as_mut() {
        Some(t) => t,
        None => return errno::ENOMEM,
    };

    let fd = table.alloc();
    let fd_type = if inode.kind() == InodeKind::Dir {
        FdType::Dir { inode, offset: 0 }
    } else {
        // O_TRUNC: trunca arquivo.
        if oflags.contains(OpenFlags::O_TRUNC) {
            let _ = inode.0.truncate(0);
        }
        FdType::File { inode, offset: 0, flags: oflags }
    };
    table.insert(fd, FdEntry { fd_type });

    fd as i64
}

pub fn sys_close(fd: i32) -> i64 {
    let mut table = fd_table();
    let table = match table.as_mut() {
        Some(t) => t,
        None => return errno::EBADF,
    };

    match table.remove(fd) {
        Some(entry) => {
            // Decrementa contadores de pipe se necessário
            match entry.fd_type {
                FdType::PipeRead { pipe } => pipe.remove_reader(),
                FdType::PipeWrite { pipe } => pipe.remove_writer(),
                FdType::Socket { socket_id } => {
                    let _ = crate::net::socket::close(socket_id);
                }
                _ => {}
            }
            0
        }
        None => errno::EBADF,
    }
}

// fcntl commands
const F_DUPFD: i32 = 0;       // Duplicate file descriptor
const F_GETFD: i32 = 1;       // Get file descriptor flags
const F_SETFD: i32 = 2;       // Set file descriptor flags
const F_GETFL: i32 = 3;       // Get file status flags
const F_SETFL: i32 = 4;       // Set file status flags
const F_DUPFD_CLOEXEC: i32 = 1030; // Duplicate with close-on-exec

// File descriptor flags
const FD_CLOEXEC: i32 = 1;

/// fcntl - File control operations
pub fn sys_fcntl(fd: i32, cmd: i32, arg: u64) -> i64 {
    let mut table = fd_table();
    let table = match table.as_mut() {
        Some(t) => t,
        None => return errno::EBADF,
    };

    match cmd {
        F_DUPFD | F_DUPFD_CLOEXEC => {
            // Duplicate fd to lowest available >= arg
            let min_fd = arg as i32;
            let entry = match table.get(fd) {
                Some(e) => e.clone(),
                None => return errno::EBADF,
            };

            // Find lowest available fd >= min_fd
            let mut new_fd = min_fd;
            while table.get(new_fd).is_some() {
                new_fd += 1;
                if new_fd > 1024 {
                    return errno::EMFILE;
                }
            }

            table.insert(new_fd, entry);

            // For pipe types, increment reference counts
            if let Some(e) = table.get(new_fd) {
                match &e.fd_type {
                    FdType::PipeRead { pipe } => pipe.add_reader(),
                    FdType::PipeWrite { pipe } => pipe.add_writer(),
                    _ => {}
                }
            }

            new_fd as i64
        }

        F_GETFD => {
            // Get file descriptor flags (FD_CLOEXEC)
            match table.get(fd) {
                Some(_) => 0, // For now, always return 0 (no FD_CLOEXEC)
                None => errno::EBADF,
            }
        }

        F_SETFD => {
            // Set file descriptor flags
            match table.get(fd) {
                Some(_) => 0, // Silently ignore for now
                None => errno::EBADF,
            }
        }

        F_GETFL => {
            // Get file status flags
            match table.get(fd) {
                Some(entry) => {
                    match &entry.fd_type {
                        FdType::File { flags, .. } => flags.bits() as i64,
                        FdType::Console => 0o2, // O_RDWR
                        FdType::Dir { .. } => 0o0, // O_RDONLY
                        FdType::PipeRead { .. } => 0o0, // O_RDONLY
                        FdType::PipeWrite { .. } => 0o1, // O_WRONLY
                        FdType::Socket { .. } => 0o2, // O_RDWR
                        FdType::EventFd { eventfd } => {
                            let mut flags = 0o2; // O_RDWR
                            if eventfd.inner.is_nonblock() {
                                flags |= 0o4000; // O_NONBLOCK
                            }
                            flags
                        }
                    }
                }
                None => errno::EBADF,
            }
        }

        F_SETFL => {
            // Set file status flags (only O_NONBLOCK can be changed on most types)
            match table.get_mut(fd) {
                Some(entry) => {
                    match &mut entry.fd_type {
                        FdType::File { flags, .. } => {
                            // Keep access mode, update other flags
                            let access = flags.bits() & 0o3;
                            let new_flags = (arg as u32) & !0o3 | access;
                            *flags = OpenFlags::from_bits_truncate(new_flags);
                            0
                        }
                        _ => 0, // Silently ignore for other types
                    }
                }
                None => errno::EBADF,
            }
        }

        _ => errno::EINVAL,
    }
}

/// getrlimit - Get resource limits
///
/// resource: Resource type (RLIMIT_*)
/// rlim: Pointer to rlimit structure to fill
pub fn sys_getrlimit(resource: u32, rlim: u64) -> i64 {
    if rlim == 0 {
        return errno::EFAULT;
    }

    let task = crate::sched::current_task();
    match task.get_rlimit(resource) {
        Some(limit) => {
            unsafe {
                let rlim_ptr = rlim as *mut Rlimit;
                *rlim_ptr = limit;
            }
            0
        }
        None => errno::EINVAL,
    }
}

/// setrlimit - Set resource limits
///
/// resource: Resource type (RLIMIT_*)
/// rlim: Pointer to rlimit structure with new limits
pub fn sys_setrlimit(resource: u32, rlim: u64) -> i64 {
    if rlim == 0 {
        return errno::EFAULT;
    }

    let task = crate::sched::current_task();

    // Read the new limit from user space
    let new_limit = unsafe { *(rlim as *const Rlimit) };

    // Get current limit to validate
    let current = match task.get_rlimit(resource) {
        Some(l) => l,
        None => return errno::EINVAL,
    };

    // Soft limit cannot exceed hard limit
    if new_limit.rlim_cur > new_limit.rlim_max {
        return errno::EINVAL;
    }

    // Only root can raise hard limit
    let cred = crate::sched::current_cred();
    if new_limit.rlim_max > current.rlim_max && cred.euid.0 != 0 {
        return errno::EPERM;
    }

    if task.set_rlimit(resource, new_limit) {
        0
    } else {
        errno::EINVAL
    }
}

/// prlimit64 - Get/set resource limits for a specific process
///
/// pid: Process ID (0 = current process)
/// resource: Resource type (RLIMIT_*)
/// new_limit: Pointer to new rlimit (can be NULL)
/// old_limit: Pointer to store current rlimit (can be NULL)
pub fn sys_prlimit64(pid: i32, resource: u32, new_limit: u64, old_limit: u64) -> i64 {
    // For now, only support current process
    if pid != 0 && pid as u64 != crate::sched::current_pid() {
        return errno::EPERM;
    }

    let task = crate::sched::current_task();

    // Get current limit first if requested
    if old_limit != 0 {
        match task.get_rlimit(resource) {
            Some(limit) => unsafe {
                *(old_limit as *mut Rlimit) = limit;
            },
            None => return errno::EINVAL,
        }
    }

    // Set new limit if provided
    if new_limit != 0 {
        let new = unsafe { *(new_limit as *const Rlimit) };

        // Soft limit cannot exceed hard limit
        if new.rlim_cur > new.rlim_max {
            return errno::EINVAL;
        }

        // Get current to validate hard limit change
        let current = match task.get_rlimit(resource) {
            Some(l) => l,
            None => return errno::EINVAL,
        };

        // Only root can raise hard limit
        let cred = crate::sched::current_cred();
        if new.rlim_max > current.rlim_max && cred.euid.0 != 0 {
            return errno::EPERM;
        }

        if !task.set_rlimit(resource, new) {
            return errno::EINVAL;
        }
    }

    0
}

pub fn sys_lseek(fd: i32, offset: i64, whence: i32) -> i64 {
    const SEEK_SET: i32 = 0;
    const SEEK_CUR: i32 = 1;
    const SEEK_END: i32 = 2;

    let mut table = fd_table();
    let table = match table.as_mut() {
        Some(t) => t,
        None => return errno::EBADF,
    };

    let entry = match table.get_mut(fd) {
        Some(e) => e,
        None => return errno::EBADF,
    };

    match &mut entry.fd_type {
        FdType::Console => errno::ESPIPE,
        FdType::File { inode, offset: current, .. } => {
            let size = inode.0.size().unwrap_or(0) as i64;
            let new_offset = match whence {
                SEEK_SET => offset,
                SEEK_CUR => *current as i64 + offset,
                SEEK_END => size + offset,
                _ => return errno::EINVAL,
            };
            if new_offset < 0 {
                return errno::EINVAL;
            }
            *current = new_offset as usize;
            new_offset
        }
        FdType::Dir { .. } => errno::EISDIR,
        FdType::PipeRead { .. } | FdType::PipeWrite { .. } | FdType::Socket { .. } | FdType::EventFd { .. } => errno::ESPIPE,
    }
}

// ======================== Stat syscalls ========================

/// Linux stat structure (for x86_64)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Stat {
    pub st_dev: u64,      // Device ID
    pub st_ino: u64,      // Inode number
    pub st_nlink: u64,    // Number of hard links
    pub st_mode: u32,     // File mode (permissions + type)
    pub st_uid: u32,      // User ID
    pub st_gid: u32,      // Group ID
    _pad0: u32,           // Padding
    pub st_rdev: u64,     // Device ID (if special file)
    pub st_size: i64,     // Total size in bytes
    pub st_blksize: i64,  // Block size for I/O
    pub st_blocks: i64,   // Number of 512-byte blocks
    pub st_atime: i64,    // Access time
    pub st_atime_nsec: i64,
    pub st_mtime: i64,    // Modification time
    pub st_mtime_nsec: i64,
    pub st_ctime: i64,    // Status change time
    pub st_ctime_nsec: i64,
    _reserved: [i64; 3],
}

impl Stat {
    pub fn new() -> Self {
        Self {
            st_dev: 0,
            st_ino: 0,
            st_nlink: 1,
            st_mode: 0,
            st_uid: 0,
            st_gid: 0,
            _pad0: 0,
            st_rdev: 0,
            st_size: 0,
            st_blksize: 4096,
            st_blocks: 0,
            st_atime: 0,
            st_atime_nsec: 0,
            st_mtime: 0,
            st_mtime_nsec: 0,
            st_ctime: 0,
            st_ctime_nsec: 0,
            _reserved: [0; 3],
        }
    }

    pub fn from_inode(inode: &Arc<dyn fs::vfs::InodeOps>) -> Self {
        let meta = inode.metadata();
        let size = inode.size().unwrap_or(0);

        // Convert InodeKind to mode bits
        let type_bits = match meta.kind {
            InodeKind::File => 0o100000,      // S_IFREG
            InodeKind::Dir => 0o040000,       // S_IFDIR
            InodeKind::Symlink => 0o120000,   // S_IFLNK
            InodeKind::CharDev => 0o020000,   // S_IFCHR
            InodeKind::BlockDev => 0o060000,  // S_IFBLK
            InodeKind::Fifo => 0o010000,      // S_IFIFO
            InodeKind::Socket => 0o140000,    // S_IFSOCK
        };

        let perm_bits = meta.mode.bits() as u32 & 0o7777;
        let mode = type_bits | perm_bits;

        Self {
            st_dev: 1,
            st_ino: meta.ino,
            st_nlink: meta.nlink as u64,
            st_mode: mode,
            st_uid: meta.uid.0,
            st_gid: meta.gid.0,
            _pad0: 0,
            st_rdev: 0,
            st_size: size as i64,
            st_blksize: 4096,
            st_blocks: ((size + 511) / 512) as i64,
            st_atime: meta.atime.secs as i64,
            st_atime_nsec: meta.atime.nsecs as i64,
            st_mtime: meta.mtime.secs as i64,
            st_mtime_nsec: meta.mtime.nsecs as i64,
            st_ctime: meta.ctime.secs as i64,
            st_ctime_nsec: meta.ctime.nsecs as i64,
            _reserved: [0; 3],
        }
    }
}

/// stat syscall - get file status by path
pub fn sys_stat(path_ptr: u64, statbuf: u64) -> i64 {
    // Validate pointers
    if !is_user_addr(path_ptr) || !is_user_range(statbuf, core::mem::size_of::<Stat>()) {
        return errno::EFAULT;
    }

    // Read path string
    let path = match unsafe { read_user_string(path_ptr, 4096) } {
        Some(p) => p,
        None => return errno::EFAULT,
    };

    if path.is_empty() {
        return errno::ENOENT;
    }

    // Get credentials
    let cred = current_cred();

    // Resolve path and get inode
    let vfs = fs::vfs_lock();
    let inode = match vfs.resolve(&path, &cred) {
        Ok(i) => i,
        Err(KError::NotFound) => return errno::ENOENT,
        Err(KError::PermissionDenied) => return errno::EACCES,
        Err(KError::NotADirectory) => return errno::ENOTDIR,
        Err(_) => return errno::EIO,
    };

    // Fill stat structure
    let stat = Stat::from_inode(&inode.0);

    // Write to user buffer
    unsafe {
        core::ptr::write(statbuf as *mut Stat, stat);
    }

    0
}

/// lstat syscall - like stat but don't follow symlinks
pub fn sys_lstat(path_ptr: u64, statbuf: u64) -> i64 {
    // Validate pointers
    if !is_user_addr(path_ptr) || !is_user_range(statbuf, core::mem::size_of::<Stat>()) {
        return errno::EFAULT;
    }

    // Read path string
    let path = match unsafe { read_user_string(path_ptr, 4096) } {
        Some(p) => p,
        None => return errno::EFAULT,
    };

    if path.is_empty() {
        return errno::ENOENT;
    }

    // Get credentials
    let cred = current_cred();

    // Resolve path without following final symlink
    let vfs = fs::vfs_lock();
    let inode = match vfs.resolve_nofollow(&path, &cred) {
        Ok(i) => i,
        Err(KError::NotFound) => return errno::ENOENT,
        Err(KError::PermissionDenied) => return errno::EACCES,
        Err(KError::NotADirectory) => return errno::ENOTDIR,
        Err(_) => return errno::EIO,
    };

    // Fill stat structure
    let stat = Stat::from_inode(&inode.0);

    // Write to user buffer
    unsafe {
        core::ptr::write(statbuf as *mut Stat, stat);
    }

    0
}

/// fstat syscall - get file status by file descriptor
pub fn sys_fstat(fd: i32, statbuf: u64) -> i64 {
    // Validate buffer
    if !is_user_range(statbuf, core::mem::size_of::<Stat>()) {
        return errno::EFAULT;
    }

    let table = fd_table();
    let table = match table.as_ref() {
        Some(t) => t,
        None => return errno::EBADF,
    };

    let entry = match table.get(fd) {
        Some(e) => e,
        None => return errno::EBADF,
    };

    let stat = match &entry.fd_type {
        FdType::File { inode, .. } | FdType::Dir { inode, .. } => {
            Stat::from_inode(&inode.0)
        }
        FdType::Console => {
            // Console is a character device
            let mut stat = Stat::new();
            stat.st_mode = 0o020666; // S_IFCHR | rw-rw-rw-
            stat.st_rdev = 5; // tty device
            stat
        }
        FdType::PipeRead { .. } | FdType::PipeWrite { .. } => {
            let mut stat = Stat::new();
            stat.st_mode = 0o010600; // S_IFIFO | rw-------
            stat
        }
        FdType::Socket { .. } => {
            let mut stat = Stat::new();
            stat.st_mode = 0o140600; // S_IFSOCK | rw-------
            stat
        }
        FdType::EventFd { .. } => {
            let mut stat = Stat::new();
            stat.st_mode = 0o100600; // S_IFREG | rw-------
            stat
        }
    };

    // Write to user buffer
    unsafe {
        core::ptr::write(statbuf as *mut Stat, stat);
    }

    0
}

// ======================== Access syscall ========================

/// access() mode flags
pub mod access_mode {
    pub const F_OK: u32 = 0; // Test for existence
    pub const X_OK: u32 = 1; // Test for execute permission
    pub const W_OK: u32 = 2; // Test for write permission
    pub const R_OK: u32 = 4; // Test for read permission
}

/// access syscall - check real user's permissions for a file
/// Uses REAL uid/gid, not effective (unlike normal file operations)
pub fn sys_access(path_ptr: u64, mode: u32) -> i64 {
    // Validate pointer
    if !is_user_addr(path_ptr) {
        return errno::EFAULT;
    }

    // Read path string
    let path = match unsafe { read_user_string(path_ptr, 4096) } {
        Some(p) => p,
        None => return errno::EFAULT,
    };

    if path.is_empty() {
        return errno::ENOENT;
    }

    // Validate mode
    if mode > 7 {
        return errno::EINVAL;
    }

    // Get credentials
    let cred = current_cred();

    // Resolve path and get inode
    let vfs = fs::vfs_lock();
    let inode = match vfs.resolve(&path, &cred) {
        Ok(i) => i,
        Err(KError::NotFound) => return errno::ENOENT,
        Err(KError::PermissionDenied) => return errno::EACCES,
        Err(KError::NotADirectory) => return errno::ENOTDIR,
        Err(_) => return errno::EIO,
    };

    // Check permissions using real UID/GID
    let meta = inode.metadata();
    if fs::perm::check_access(&meta, &cred, mode) {
        0
    } else {
        errno::EACCES
    }
}

/// faccessat syscall - check permissions relative to directory fd
pub fn sys_faccessat(dirfd: i32, path_ptr: u64, mode: u32, flags: u32) -> i64 {
    // AT_FDCWD = -100
    const AT_FDCWD: i32 = -100;
    const AT_EACCESS: u32 = 0x200; // Use effective UID/GID
    const AT_SYMLINK_NOFOLLOW: u32 = 0x100;

    // Validate pointer
    if !is_user_addr(path_ptr) {
        return errno::EFAULT;
    }

    // Read path string
    let path = match unsafe { read_user_string(path_ptr, 4096) } {
        Some(p) => p,
        None => return errno::EFAULT,
    };

    if path.is_empty() {
        return errno::ENOENT;
    }

    // Validate mode
    if mode > 7 {
        return errno::EINVAL;
    }

    // Get credentials
    let cred = current_cred();

    // Handle relative path with dirfd
    let full_path = if path.starts_with('/') {
        path
    } else if dirfd == AT_FDCWD {
        // Relative to current working directory
        let task = crate::sched::current_task();
        let cwd = task.cwd();
        if cwd == "/" {
            alloc::format!("/{}", path)
        } else {
            alloc::format!("{}/{}", cwd, path)
        }
    } else {
        // Relative to dirfd - get directory path from fd
        let table = fd_table();
        let table = match table.as_ref() {
            Some(t) => t,
            None => return errno::EBADF,
        };
        let entry = match table.get(dirfd) {
            Some(e) => e,
            None => return errno::EBADF,
        };
        match &entry.fd_type {
            FdType::Dir { inode, .. } => {
                // Get the path from the inode (simplified - use current cwd + name)
                let _meta = inode.metadata();
                // For now, just use cwd-relative
                let task = crate::sched::current_task();
                let cwd = task.cwd();
                if cwd == "/" {
                    alloc::format!("/{}", path)
                } else {
                    alloc::format!("{}/{}", cwd, path)
                }
            }
            _ => return errno::ENOTDIR,
        }
    };

    // Resolve path
    let vfs = fs::vfs_lock();
    let inode = if flags & AT_SYMLINK_NOFOLLOW != 0 {
        match vfs.resolve_nofollow(&full_path, &cred) {
            Ok(i) => i,
            Err(KError::NotFound) => return errno::ENOENT,
            Err(KError::PermissionDenied) => return errno::EACCES,
            Err(KError::NotADirectory) => return errno::ENOTDIR,
            Err(_) => return errno::EIO,
        }
    } else {
        match vfs.resolve(&full_path, &cred) {
            Ok(i) => i,
            Err(KError::NotFound) => return errno::ENOENT,
            Err(KError::PermissionDenied) => return errno::EACCES,
            Err(KError::NotADirectory) => return errno::ENOTDIR,
            Err(_) => return errno::EIO,
        }
    };

    // Check permissions
    let meta = inode.metadata();

    // AT_EACCESS: use effective UID/GID (normal permission check)
    // Without AT_EACCESS: use real UID/GID (access() semantics)
    let has_access = if flags & AT_EACCESS != 0 {
        // Use effective UID/GID (standard permission check)
        let need_r = (mode & access_mode::R_OK) != 0;
        let need_w = (mode & access_mode::W_OK) != 0;
        let need_x = (mode & access_mode::X_OK) != 0;

        if mode == access_mode::F_OK {
            true // File exists
        } else if need_r && !fs::perm::can_read(&meta, &cred) {
            false
        } else if need_w && !fs::perm::can_write(&meta, &cred) {
            false
        } else if need_x && !fs::perm::can_exec(&meta, &cred) {
            false
        } else {
            true
        }
    } else {
        // Use real UID/GID
        fs::perm::check_access(&meta, &cred, mode)
    };

    if has_access {
        0
    } else {
        errno::EACCES
    }
}

// ======================== Rename syscall ========================

/// rename syscall - rename a file
pub fn sys_rename(oldpath_ptr: u64, newpath_ptr: u64) -> i64 {
    // Validate pointers
    if !is_user_addr(oldpath_ptr) || !is_user_addr(newpath_ptr) {
        return errno::EFAULT;
    }

    // Read path strings
    let oldpath = match unsafe { read_user_string(oldpath_ptr, 4096) } {
        Some(p) => p,
        None => return errno::EFAULT,
    };

    let newpath = match unsafe { read_user_string(newpath_ptr, 4096) } {
        Some(p) => p,
        None => return errno::EFAULT,
    };

    if oldpath.is_empty() || newpath.is_empty() {
        return errno::ENOENT;
    }

    // Get credentials
    let cred = current_cred();

    // Perform rename through VFS
    let mut vfs = fs::vfs_lock();
    match vfs.rename(&oldpath, &newpath, &cred) {
        Ok(()) => 0,
        Err(KError::NotFound) => errno::ENOENT,
        Err(KError::PermissionDenied) => errno::EACCES,
        Err(KError::NotADirectory) => errno::ENOTDIR,
        Err(KError::IsADirectory) => errno::EISDIR,
        Err(KError::NotEmpty) => errno::ENOTEMPTY,
        Err(KError::AlreadyExists) => errno::EEXIST,
        Err(KError::Invalid) => errno::EINVAL,
        Err(_) => errno::EIO,
    }
}

/// linux_dirent64 structure layout:
/// - d_ino: u64 (inode number)
/// - d_off: i64 (offset to next entry)
/// - d_reclen: u16 (length of this record)
/// - d_type: u8 (file type)
/// - d_name: [u8] (null-terminated name)
pub fn sys_getdents64(fd: i32, dirp: u64, count: usize) -> i64 {
    // Validate buffer
    if !is_user_range(dirp, count) {
        return errno::EFAULT;
    }

    let mut table = fd_table();
    let table = match table.as_mut() {
        Some(t) => t,
        None => return errno::EBADF,
    };

    let entry = match table.get_mut(fd) {
        Some(e) => e,
        None => return errno::EBADF,
    };

    // Must be a directory
    let (inode, offset) = match &mut entry.fd_type {
        FdType::Dir { inode, offset } => (inode.clone(), offset),
        FdType::File { .. } => return errno::ENOTDIR,
        _ => return errno::EBADF,
    };

    // Get directory entries
    let entries = match inode.0.readdir() {
        Ok(e) => e,
        Err(_) => return errno::EIO,
    };

    // Skip already-read entries
    let start_offset = *offset;
    let mut bytes_written: usize = 0;
    let buf = dirp as *mut u8;

    // Constants for d_type
    const DT_UNKNOWN: u8 = 0;
    const DT_FIFO: u8 = 1;
    const DT_CHR: u8 = 2;
    const DT_DIR: u8 = 4;
    const DT_BLK: u8 = 6;
    const DT_REG: u8 = 8;
    const DT_LNK: u8 = 10;
    const DT_SOCK: u8 = 12;

    for (i, entry) in entries.iter().enumerate() {
        if i < start_offset {
            continue;
        }

        // Calculate record length (must be 8-byte aligned)
        // Header: d_ino(8) + d_off(8) + d_reclen(2) + d_type(1) = 19 bytes
        // Plus name + null terminator
        let name_len = entry.name.len() + 1; // +1 for null terminator
        let reclen = ((19 + name_len + 7) / 8) * 8; // align to 8 bytes

        // Check if there's space
        if bytes_written + reclen > count {
            break;
        }

        // Determine d_type
        let d_type = match entry.kind {
            InodeKind::File => DT_REG,
            InodeKind::Dir => DT_DIR,
            InodeKind::Symlink => DT_LNK,
            InodeKind::CharDev => DT_CHR,
            InodeKind::BlockDev => DT_BLK,
            InodeKind::Fifo => DT_FIFO,
            InodeKind::Socket => DT_SOCK,
        };

        // Write the entry
        unsafe {
            let base = buf.add(bytes_written);

            // d_ino (8 bytes) - use index as fake inode
            let d_ino: u64 = (i + 1) as u64;
            core::ptr::copy_nonoverlapping(&d_ino as *const u64 as *const u8, base, 8);

            // d_off (8 bytes) - offset to next entry
            let d_off: i64 = (i + 1) as i64;
            core::ptr::copy_nonoverlapping(&d_off as *const i64 as *const u8, base.add(8), 8);

            // d_reclen (2 bytes)
            let d_reclen: u16 = reclen as u16;
            core::ptr::copy_nonoverlapping(&d_reclen as *const u16 as *const u8, base.add(16), 2);

            // d_type (1 byte)
            *base.add(18) = d_type;

            // d_name (null-terminated string)
            let name_bytes = entry.name.as_bytes();
            core::ptr::copy_nonoverlapping(name_bytes.as_ptr(), base.add(19), name_bytes.len());
            *base.add(19 + name_bytes.len()) = 0; // null terminator
        }

        bytes_written += reclen;
        *offset = i + 1;
    }

    bytes_written as i64
}

pub fn sys_getcwd(buf: u64, size: usize) -> i64 {
    use crate::sched::current_task;

    let task = current_task();
    let cwd = task.cwd();

    // Need space for path + null terminator
    if size < cwd.len() + 1 {
        return errno::EINVAL;
    }

    unsafe {
        core::ptr::copy_nonoverlapping(cwd.as_ptr(), buf as *mut u8, cwd.len());
        // Add null terminator
        *((buf + cwd.len() as u64) as *mut u8) = 0;
    }
    // Return 0 on success (compatible with i32 cast in userspace libc)
    0
}

/// Change current working directory
pub fn sys_chdir(pathname: u64) -> i64 {
    use crate::sched::current_task;

    let path = match unsafe { read_user_string(pathname, 1024) } {
        Some(p) => p,
        None => return errno::EFAULT,
    };

    // Resolve the path (handle relative paths)
    let task = current_task();
    let cwd = task.cwd();

    let abs_path = if path.starts_with('/') {
        path.clone()
    } else {
        // Relative path: join with current cwd
        if cwd == "/" {
            alloc::format!("/{}", path)
        } else {
            alloc::format!("{}/{}", cwd, path)
        }
    };

    // Normalize the path (handle . and ..)
    let normalized = normalize_path(&abs_path);

    // Verify the directory exists using VFS
    let cred = current_cred();
    let vfs = fs::vfs_lock();
    match vfs.resolve(&normalized, &cred) {
        Ok(inode) => {
            // Check if it's a directory
            let meta = inode.metadata();
            if meta.kind != InodeKind::Dir {
                return errno::ENOTDIR;
            }
            // Check execute (traverse) permission on the directory
            if !fs::perm::can_exec_dir(&meta, &cred) {
                return errno::EACCES;
            }
            drop(vfs); // Release lock before modifying task
            // Update the task's cwd
            task.set_cwd(normalized);
            0
        }
        Err(KError::NotFound) => errno::ENOENT,
        Err(KError::PermissionDenied) => errno::EACCES,
        Err(_) => errno::EIO,
    }
}

/// Normalize a path by resolving . and .. components
fn normalize_path(path: &str) -> alloc::string::String {
    let mut components: alloc::vec::Vec<&str> = alloc::vec::Vec::new();

    for component in path.split('/') {
        match component {
            "" | "." => {} // Skip empty and current dir
            ".." => {
                components.pop(); // Go up one level
            }
            _ => {
                components.push(component);
            }
        }
    }

    if components.is_empty() {
        "/".into()
    } else {
        alloc::format!("/{}", components.join("/"))
    }
}

pub fn sys_mkdir(pathname: u64, mode: u32) -> i64 {
    let path = match unsafe { read_user_string(pathname, 1024) } {
        Some(p) => p,
        None => return errno::EFAULT,
    };

    let cred = current_cred();
    let mut vfs = fs::vfs_lock();

    match vfs.mkdir_all(&path, &cred, Mode::from_octal(mode as u16)) {
        Ok(()) => 0,
        Err(KError::PermissionDenied) => errno::EACCES,
        Err(_) => errno::EIO,
    }
}

pub fn sys_rmdir(pathname: u64) -> i64 {
    let path = match unsafe { read_user_string(pathname, 1024) } {
        Some(p) => p,
        None => return errno::EFAULT,
    };

    let cred = current_cred();
    let mut vfs = fs::vfs_lock();

    match vfs.rmdir(&path, &cred) {
        Ok(()) => 0,
        Err(KError::NotFound) => errno::ENOENT,
        Err(KError::PermissionDenied) => errno::EACCES,
        Err(KError::NotEmpty) => errno::ENOTEMPTY,
        Err(KError::Invalid) => errno::ENOTDIR,
        Err(_) => errno::EIO,
    }
}

pub fn sys_unlink(pathname: u64) -> i64 {
    let path = match unsafe { read_user_string(pathname, 1024) } {
        Some(p) => p,
        None => return errno::EFAULT,
    };

    let cred = current_cred();
    let mut vfs = fs::vfs_lock();

    match vfs.unlink(&path, &cred) {
        Ok(()) => 0,
        Err(KError::NotFound) => errno::ENOENT,
        Err(KError::PermissionDenied) => errno::EACCES,
        Err(KError::Invalid) => errno::EISDIR, // unlink on a directory
        Err(_) => errno::EIO,
    }
}

// ==================== Symlink syscalls ====================

/// Create a symbolic link
pub fn sys_symlink(target: u64, linkpath: u64) -> i64 {
    let target_str = match unsafe { read_user_string(target, 4096) } {
        Some(p) => p,
        None => return errno::EFAULT,
    };

    let link_str = match unsafe { read_user_string(linkpath, 4096) } {
        Some(p) => p,
        None => return errno::EFAULT,
    };

    let cred = current_cred();
    let mut vfs = fs::vfs_lock();

    match vfs.symlink(&link_str, &target_str, &cred) {
        Ok(_) => 0,
        Err(KError::NotFound) => errno::ENOENT,
        Err(KError::AlreadyExists) => errno::EEXIST,
        Err(KError::PermissionDenied) => errno::EACCES,
        Err(KError::NotSupported) => errno::EPERM,
        Err(_) => errno::EIO,
    }
}

/// Read the target of a symbolic link
pub fn sys_readlink(pathname: u64, buf: u64, bufsiz: usize) -> i64 {
    let path = match unsafe { read_user_string(pathname, 4096) } {
        Some(p) => p,
        None => return errno::EFAULT,
    };

    if buf == 0 || bufsiz == 0 {
        return errno::EINVAL;
    }

    if !is_user_range(buf, bufsiz) {
        return errno::EFAULT;
    }

    let cred = current_cred();
    let vfs = fs::vfs_lock();

    match vfs.readlink(&path, &cred) {
        Ok(target) => {
            let target_bytes = target.as_bytes();
            let copy_len = core::cmp::min(target_bytes.len(), bufsiz);

            let out = unsafe { core::slice::from_raw_parts_mut(buf as *mut u8, copy_len) };
            out.copy_from_slice(&target_bytes[..copy_len]);

            copy_len as i64
        }
        Err(KError::NotFound) => errno::ENOENT,
        Err(KError::Invalid) => errno::EINVAL, // Not a symlink
        Err(KError::PermissionDenied) => errno::EACCES,
        Err(_) => errno::EIO,
    }
}

// ==================== Truncate syscalls ====================

/// Truncate a file to a specified length
pub fn sys_truncate(pathname: u64, length: i64) -> i64 {
    let path = match unsafe { read_user_string(pathname, 4096) } {
        Some(p) => p,
        None => return errno::EFAULT,
    };

    if length < 0 {
        return errno::EINVAL;
    }

    let cred = current_cred();
    let vfs = fs::vfs_lock();

    let inode = match vfs.resolve(&path, &cred) {
        Ok(i) => i,
        Err(KError::NotFound) => return errno::ENOENT,
        Err(KError::PermissionDenied) => return errno::EACCES,
        Err(_) => return errno::EIO,
    };

    // Check if it's a regular file
    if inode.metadata().kind != InodeKind::File {
        return errno::EISDIR;
    }

    // Check write permission
    if !crate::fs::perm::can_write(&inode.metadata(), &cred) {
        return errno::EACCES;
    }

    match inode.0.truncate(length as usize) {
        Ok(()) => 0,
        Err(KError::PermissionDenied) => errno::EACCES,
        Err(_) => errno::EIO,
    }
}

/// Truncate an open file to a specified length
pub fn sys_ftruncate(fd: i32, length: i64) -> i64 {
    if length < 0 {
        return errno::EINVAL;
    }

    let table = fd_table();
    let table = match table.as_ref() {
        Some(t) => t,
        None => return errno::EBADF,
    };

    let entry = match table.get(fd) {
        Some(e) => e,
        None => return errno::EBADF,
    };

    match &entry.fd_type {
        FdType::File { inode, flags, .. } => {
            // Check if it's a regular file
            if inode.metadata().kind != InodeKind::File {
                return errno::EINVAL;
            }

            // Check write permission on fd (O_WRONLY or O_RDWR)
            let can_write = flags.contains(OpenFlags::O_WRONLY) || flags.contains(OpenFlags::O_RDWR);
            if !can_write {
                return errno::EINVAL;
            }

            match inode.0.truncate(length as usize) {
                Ok(()) => 0,
                Err(_) => errno::EIO,
            }
        }
        _ => errno::EINVAL,
    }
}

// ==================== Fsync syscalls ====================

/// Synchronize a file's data and metadata to disk
pub fn sys_fsync(fd: i32) -> i64 {
    let table = fd_table();
    let table = match table.as_ref() {
        Some(t) => t,
        None => return errno::EBADF,
    };

    let entry = match table.get(fd) {
        Some(e) => e,
        None => return errno::EBADF,
    };

    match &entry.fd_type {
        FdType::File { .. } => {
            // For now, we don't have actual disk sync, so just return success
            // In a real implementation, this would flush the page cache to disk
            0
        }
        _ => errno::EINVAL,
    }
}

/// Synchronize a file's data (but not metadata) to disk
pub fn sys_fdatasync(fd: i32) -> i64 {
    // For now, same as fsync
    sys_fsync(fd)
}

// ==================== Permission syscalls ====================

pub fn sys_chmod(pathname: u64, mode: u32) -> i64 {
    use crate::fs::Mode;

    let path = match unsafe { read_user_string(pathname, 1024) } {
        Some(p) => p,
        None => return errno::EFAULT,
    };

    let cred = current_cred();
    let mut vfs = fs::vfs_lock();

    match vfs.chmod(&path, Mode::from_octal(mode as u16), &cred) {
        Ok(()) => 0,
        Err(KError::NotFound) => errno::ENOENT,
        Err(KError::PermissionDenied) => errno::EPERM,
        Err(_) => errno::EIO,
    }
}

pub fn sys_fchmod(fd: i32, mode: u32) -> i64 {
    use crate::fs::Mode;

    let table = fd_table();
    let table = match table.as_ref() {
        Some(t) => t,
        None => return errno::EBADF,
    };

    let entry = match table.get(fd) {
        Some(e) => e,
        None => return errno::EBADF,
    };

    let inode = match &entry.fd_type {
        FdType::File { inode, .. } => inode.clone(),
        _ => return errno::EINVAL, // Não é um arquivo regular
    };

    let cred = current_cred();
    let mut meta = inode.metadata();

    // Somente root ou owner pode alterar permissões
    if cred.uid.0 != 0 && meta.uid != cred.uid {
        return errno::EPERM;
    }

    meta.mode = Mode::from_octal(mode as u16);
    inode.0.set_metadata(meta);
    0
}

pub fn sys_chown(pathname: u64, owner: u32, group: u32) -> i64 {
    use crate::security::{Uid, Gid};

    let path = match unsafe { read_user_string(pathname, 1024) } {
        Some(p) => p,
        None => return errno::EFAULT,
    };

    let cred = current_cred();
    let mut vfs = fs::vfs_lock();

    // -1 significa não alterar
    let uid = if owner == u32::MAX { cred.uid } else { Uid(owner) };
    let gid = if group == u32::MAX { cred.gid } else { Gid(group) };

    match vfs.chown(&path, uid, gid, &cred) {
        Ok(()) => 0,
        Err(KError::NotFound) => errno::ENOENT,
        Err(KError::PermissionDenied) => errno::EPERM,
        Err(_) => errno::EIO,
    }
}

pub fn sys_fchown(fd: i32, owner: u32, group: u32) -> i64 {
    use crate::security::{Uid, Gid};

    let table = fd_table();
    let table = match table.as_ref() {
        Some(t) => t,
        None => return errno::EBADF,
    };

    let entry = match table.get(fd) {
        Some(e) => e,
        None => return errno::EBADF,
    };

    let inode = match &entry.fd_type {
        FdType::File { inode, .. } => inode.clone(),
        _ => return errno::EINVAL,
    };

    let cred = current_cred();
    let mut meta = inode.metadata();

    // -1 significa não alterar
    let uid = if owner == u32::MAX { meta.uid } else { Uid(owner) };
    let gid = if group == u32::MAX { meta.gid } else { Gid(group) };

    // Somente root pode alterar owner
    if cred.uid.0 != 0 {
        if uid != meta.uid {
            return errno::EPERM;
        }
        if meta.uid != cred.uid {
            return errno::EPERM;
        }
    }

    meta.uid = uid;
    meta.gid = gid;
    inode.0.set_metadata(meta);
    0
}

pub fn sys_lchown(pathname: u64, owner: u32, group: u32) -> i64 {
    // Por enquanto, tratamos igual a chown (sem seguir symlinks seria diferente)
    sys_chown(pathname, owner, group)
}

pub fn sys_getpid() -> i64 {
    // Retorna TID do processo atual (simplificado).
    crate::task::current()
        .map(|t| t.id.0 as i64)
        .unwrap_or(1)
}

pub fn sys_getuid() -> i64 {
    current_cred().uid.0 as i64
}

pub fn sys_getgid() -> i64 {
    current_cred().gid.0 as i64
}

pub fn sys_geteuid() -> i64 {
    current_cred().euid.0 as i64
}

pub fn sys_getegid() -> i64 {
    current_cred().egid.0 as i64
}

// ==================== setuid/setgid syscalls ====================

/// Set user ID
pub fn sys_setuid(uid: u64) -> i64 {
    let mut cred = current_cred();
    match cred.setuid(uid as u32) {
        Ok(()) => {
            crate::task::set_current_cred(cred);
            0
        }
        Err(_) => errno::EPERM,
    }
}

/// Set group ID
pub fn sys_setgid(gid: u64) -> i64 {
    let mut cred = current_cred();
    match cred.setgid(gid as u32) {
        Ok(()) => {
            crate::task::set_current_cred(cred);
            0
        }
        Err(_) => errno::EPERM,
    }
}

/// Set real and effective user IDs
pub fn sys_setreuid(ruid: i64, euid: i64) -> i64 {
    let mut cred = current_cred();
    match cred.setreuid(ruid as i32, euid as i32) {
        Ok(()) => {
            crate::task::set_current_cred(cred);
            0
        }
        Err(_) => errno::EPERM,
    }
}

/// Set real and effective group IDs
pub fn sys_setregid(rgid: i64, egid: i64) -> i64 {
    let mut cred = current_cred();
    match cred.setregid(rgid as i32, egid as i32) {
        Ok(()) => {
            crate::task::set_current_cred(cred);
            0
        }
        Err(_) => errno::EPERM,
    }
}

/// Get supplementary group IDs
pub fn sys_getgroups(size: u64, list: u64) -> i64 {
    let cred = current_cred();
    let groups = cred.getgroups();

    // If size is 0, return the number of groups
    if size == 0 {
        return groups.len() as i64;
    }

    // Check buffer size
    if (size as usize) < groups.len() {
        return errno::EINVAL;
    }

    // Validate user buffer
    if list == 0 || !is_user_range(list, groups.len() * 4) {
        return errno::EFAULT;
    }

    // Copy groups to user space
    let out = unsafe { core::slice::from_raw_parts_mut(list as *mut u32, groups.len()) };
    for (i, g) in groups.iter().enumerate() {
        out[i] = g.0;
    }

    groups.len() as i64
}

/// Set supplementary group IDs
pub fn sys_setgroups(size: u64, list: u64) -> i64 {
    use crate::security::{Gid, MAX_GROUPS};

    // Only root can set groups
    let mut cred = current_cred();
    if cred.euid.0 != 0 {
        return errno::EPERM;
    }

    if size as usize > MAX_GROUPS {
        return errno::EINVAL;
    }

    if size > 0 {
        if list == 0 || !is_user_range(list, size as usize * 4) {
            return errno::EFAULT;
        }

        let input = unsafe { core::slice::from_raw_parts(list as *const u32, size as usize) };
        let groups: Vec<Gid> = input.iter().map(|&g| Gid(g)).collect();

        match cred.setgroups(&groups) {
            Ok(()) => {
                crate::task::set_current_cred(cred);
                0
            }
            Err(_) => errno::EPERM,
        }
    } else {
        // Clear all groups
        cred.ngroups = 0;
        crate::task::set_current_cred(cred);
        0
    }
}

/// Get real, effective, and saved user IDs
pub fn sys_getresuid(ruid_ptr: u64, euid_ptr: u64, suid_ptr: u64) -> i64 {
    let cred = current_cred();

    if ruid_ptr != 0 {
        if !is_user_range(ruid_ptr, 4) {
            return errno::EFAULT;
        }
        unsafe { *(ruid_ptr as *mut u32) = cred.uid.0 };
    }

    if euid_ptr != 0 {
        if !is_user_range(euid_ptr, 4) {
            return errno::EFAULT;
        }
        unsafe { *(euid_ptr as *mut u32) = cred.euid.0 };
    }

    if suid_ptr != 0 {
        if !is_user_range(suid_ptr, 4) {
            return errno::EFAULT;
        }
        unsafe { *(suid_ptr as *mut u32) = cred.suid.0 };
    }

    0
}

/// Get real, effective, and saved group IDs
pub fn sys_getresgid(rgid_ptr: u64, egid_ptr: u64, sgid_ptr: u64) -> i64 {
    let cred = current_cred();

    if rgid_ptr != 0 {
        if !is_user_range(rgid_ptr, 4) {
            return errno::EFAULT;
        }
        unsafe { *(rgid_ptr as *mut u32) = cred.gid.0 };
    }

    if egid_ptr != 0 {
        if !is_user_range(egid_ptr, 4) {
            return errno::EFAULT;
        }
        unsafe { *(egid_ptr as *mut u32) = cred.egid.0 };
    }

    if sgid_ptr != 0 {
        if !is_user_range(sgid_ptr, 4) {
            return errno::EFAULT;
        }
        unsafe { *(sgid_ptr as *mut u32) = cred.sgid.0 };
    }

    0
}

/// Set real, effective, and saved user IDs
pub fn sys_setresuid(ruid: i64, euid: i64, suid: i64) -> i64 {
    let mut cred = current_cred();
    let is_root = cred.euid.0 == 0;

    // -1 means "don't change"
    let new_ruid = if ruid == -1 { cred.uid.0 } else { ruid as u32 };
    let new_euid = if euid == -1 { cred.euid.0 } else { euid as u32 };
    let new_suid = if suid == -1 { cred.suid.0 } else { suid as u32 };

    // Check permissions
    if !is_root {
        // Non-root can only set each ID to one of the current real, effective, or saved values
        let valid = |id: u32| id == cred.uid.0 || id == cred.euid.0 || id == cred.suid.0;
        if (ruid != -1 && !valid(new_ruid)) ||
           (euid != -1 && !valid(new_euid)) ||
           (suid != -1 && !valid(new_suid)) {
            return errno::EPERM;
        }
    }

    cred.uid = crate::security::Uid(new_ruid);
    cred.euid = crate::security::Uid(new_euid);
    cred.suid = crate::security::Uid(new_suid);

    // Update caps based on effective UID
    if cred.euid.0 == 0 {
        cred.caps = crate::security::Caps::ALL;
    } else {
        cred.caps = crate::security::Caps::empty();
    }

    crate::task::set_current_cred(cred);
    0
}

/// Set real, effective, and saved group IDs
pub fn sys_setresgid(rgid: i64, egid: i64, sgid: i64) -> i64 {
    let mut cred = current_cred();
    let is_root = cred.euid.0 == 0;

    let new_rgid = if rgid == -1 { cred.gid.0 } else { rgid as u32 };
    let new_egid = if egid == -1 { cred.egid.0 } else { egid as u32 };
    let new_sgid = if sgid == -1 { cred.sgid.0 } else { sgid as u32 };

    if !is_root {
        let valid = |id: u32| id == cred.gid.0 || id == cred.egid.0 || id == cred.sgid.0;
        if (rgid != -1 && !valid(new_rgid)) ||
           (egid != -1 && !valid(new_egid)) ||
           (sgid != -1 && !valid(new_sgid)) {
            return errno::EPERM;
        }
    }

    cred.gid = crate::security::Gid(new_rgid);
    cred.egid = crate::security::Gid(new_egid);
    cred.sgid = crate::security::Gid(new_sgid);

    crate::task::set_current_cred(cred);
    0
}

// ==================== Process group/session syscalls ====================

pub fn sys_getpgid(pid: i64) -> i64 {
    if pid == 0 {
        // pid == 0 significa o processo atual
        crate::sched::current_task().pgid() as i64
    } else {
        // Procura o processo com o pid especificado
        match crate::sched::get_task_pgid(pid as u64) {
            Some(pgid) => pgid as i64,
            None => errno::ESRCH,
        }
    }
}

pub fn sys_getpgrp() -> i64 {
    // getpgrp() é equivalente a getpgid(0)
    crate::sched::current_task().pgid() as i64
}

pub fn sys_getsid(pid: i64) -> i64 {
    if pid == 0 {
        crate::sched::current_task().sid() as i64
    } else {
        match crate::sched::get_task_sid(pid as u64) {
            Some(sid) => sid as i64,
            None => errno::ESRCH,
        }
    }
}

pub fn sys_setpgid(pid: i64, pgid: i64) -> i64 {
    let target_pid = if pid == 0 {
        crate::sched::current_pid()
    } else {
        pid as u64
    };

    let new_pgid = if pgid == 0 {
        target_pid // setpgid(pid, 0) = setpgid(pid, pid)
    } else {
        pgid as u64
    };

    match crate::sched::set_task_pgid(target_pid, new_pgid) {
        Ok(()) => 0,
        Err(KError::NotFound) => errno::ESRCH,
        Err(KError::PermissionDenied) => errno::EPERM,
        Err(_) => errno::EINVAL,
    }
}

pub fn sys_setsid() -> i64 {
    match crate::sched::create_session() {
        Ok(sid) => sid as i64,
        Err(KError::PermissionDenied) => errno::EPERM, // Já é líder de grupo
        Err(_) => errno::EINVAL,
    }
}

/// struct utsname layout (65 bytes each field).
const UNAME_LENGTH: usize = 65;

pub fn sys_uname(buf: u64) -> i64 {
    if buf == 0 {
        return errno::EFAULT;
    }

    #[repr(C)]
    struct Utsname {
        sysname: [u8; UNAME_LENGTH],
        nodename: [u8; UNAME_LENGTH],
        release: [u8; UNAME_LENGTH],
        version: [u8; UNAME_LENGTH],
        machine: [u8; UNAME_LENGTH],
        domainname: [u8; UNAME_LENGTH],
    }

    fn copy_str(dest: &mut [u8; UNAME_LENGTH], src: &[u8]) {
        let len = core::cmp::min(src.len(), UNAME_LENGTH - 1);
        dest[..len].copy_from_slice(&src[..len]);
        dest[len] = 0;
    }

    let mut uname = Utsname {
        sysname: [0; UNAME_LENGTH],
        nodename: [0; UNAME_LENGTH],
        release: [0; UNAME_LENGTH],
        version: [0; UNAME_LENGTH],
        machine: [0; UNAME_LENGTH],
        domainname: [0; UNAME_LENGTH],
    };

    copy_str(&mut uname.sysname, b"StenzelOS");
    copy_str(&mut uname.nodename, b"stenzel");
    copy_str(&mut uname.release, b"0.1.0");
    copy_str(&mut uname.version, b"#1 SMP");
    copy_str(&mut uname.machine, b"x86_64");
    copy_str(&mut uname.domainname, b"(none)");

    unsafe {
        core::ptr::write(buf as *mut Utsname, uname);
    }
    0
}

pub fn sys_brk(addr: u64) -> i64 {
    // Simplificado: retorna o endereço atual do heap (fixo por enquanto).
    // Um OS real gerenciaria o break do processo.
    static HEAP_END: AtomicU64 = AtomicU64::new(0x0000_0000_1000_0000);

    if addr == 0 {
        return HEAP_END.load(Ordering::Relaxed) as i64;
    }

    // Aceita qualquer valor dentro de limites razoáveis.
    let current = HEAP_END.load(Ordering::Relaxed);
    if addr >= current && addr < 0x0000_7FFF_FFFF_0000 {
        HEAP_END.store(addr, Ordering::Relaxed);
        return addr as i64;
    }

    current as i64
}

/// mmap - mapeia memória virtual
///
/// addr: endereço sugerido (0 para auto)
/// length: tamanho em bytes
/// prot: proteção (PROT_READ, PROT_WRITE, PROT_EXEC)
/// flags: MAP_SHARED, MAP_PRIVATE, MAP_ANONYMOUS, MAP_FIXED
/// fd: file descriptor (-1 para anônimo)
/// offset: offset no arquivo
pub fn sys_mmap(addr: u64, length: usize, prot: i32, flags: i32, fd: i32, offset: i64) -> i64 {
    match crate::mm::vma::sys_mmap(addr, length, prot, flags, fd, offset) {
        Ok(mapped_addr) => mapped_addr as i64,
        Err(KError::NoMemory) => errno::ENOMEM,
        Err(KError::Invalid) => errno::EINVAL,
        Err(KError::NotSupported) => errno::ENOSYS,
        Err(_) => errno::EIO,
    }
}

/// munmap - remove mapeamento de memória
pub fn sys_munmap(addr: u64, length: usize) -> i64 {
    match crate::mm::vma::sys_munmap(addr, length) {
        Ok(()) => 0,
        Err(KError::Invalid) => errno::EINVAL,
        Err(_) => errno::EIO,
    }
}

/// mprotect - altera proteção de memória
pub fn sys_mprotect(addr: u64, length: usize, prot: i32) -> i64 {
    match crate::mm::vma::sys_mprotect(addr, length, prot) {
        Ok(()) => 0,
        Err(KError::Invalid) => errno::EINVAL,
        Err(KError::NoMemory) => errno::ENOMEM,
        Err(_) => errno::EIO,
    }
}

// ======================== Time syscalls ========================

/// gettimeofday - obtém tempo atual (wall clock)
pub fn sys_gettimeofday(tv: u64, tz: u64) -> i64 {
    // Validate output pointers
    if tv != 0 && !is_user_range(tv, core::mem::size_of::<crate::time::Timeval>()) {
        return errno::EFAULT;
    }
    if tz != 0 && !is_user_range(tz, core::mem::size_of::<crate::time::Timezone>()) {
        return errno::EFAULT;
    }

    let (timeval, timezone) = crate::time::gettimeofday();

    if tv != 0 {
        unsafe {
            core::ptr::write(tv as *mut crate::time::Timeval, timeval);
        }
    }

    if tz != 0 {
        unsafe {
            core::ptr::write(tz as *mut crate::time::Timezone, timezone);
        }
    }

    0
}

/// settimeofday - set time and/or timezone
///
/// tv: Pointer to timeval (can be NULL to only set timezone)
/// tz: Pointer to timezone (can be NULL to only set time)
pub fn sys_settimeofday(tv: u64, tz: u64) -> i64 {
    // Only root can set time/timezone
    let cred = crate::sched::current_cred();
    if cred.euid.0 != 0 {
        return errno::EPERM;
    }

    // Validate pointers
    if tv != 0 && !is_user_range(tv, core::mem::size_of::<crate::time::Timeval>()) {
        return errno::EFAULT;
    }
    if tz != 0 && !is_user_range(tz, core::mem::size_of::<crate::time::Timezone>()) {
        return errno::EFAULT;
    }

    // Set timezone if provided
    if tz != 0 {
        let timezone = unsafe { *(tz as *const crate::time::Timezone) };
        crate::time::set_timezone(&timezone);
    }

    // Setting time (tv) is not supported yet - would need RTC write
    // For now, just acknowledge success if only setting timezone
    if tv != 0 {
        // Time setting not implemented - would need RTC access
        return errno::EINVAL;
    }

    0
}

// ==================== Reboot/Shutdown Syscalls ====================

/// Linux reboot magic numbers
pub mod reboot_magic {
    pub const LINUX_REBOOT_MAGIC1: u32 = 0xfee1dead;
    pub const LINUX_REBOOT_MAGIC2: u32 = 672274793;   // LINUS
    pub const LINUX_REBOOT_MAGIC2A: u32 = 85072278;   // 0x05121996 (Linus bday)
    pub const LINUX_REBOOT_MAGIC2B: u32 = 369367448;  // 0x16041998
    pub const LINUX_REBOOT_MAGIC2C: u32 = 537993216;  // 0x20112000
}

/// Reboot commands
pub mod reboot_cmd {
    pub const LINUX_REBOOT_CMD_RESTART: u32 = 0x01234567;
    pub const LINUX_REBOOT_CMD_HALT: u32 = 0xCDEF0123;
    pub const LINUX_REBOOT_CMD_CAD_ON: u32 = 0x89ABCDEF;
    pub const LINUX_REBOOT_CMD_CAD_OFF: u32 = 0x00000000;
    pub const LINUX_REBOOT_CMD_POWER_OFF: u32 = 0x4321FEDC;
    pub const LINUX_REBOOT_CMD_RESTART2: u32 = 0xA1B2C3D4;
    pub const LINUX_REBOOT_CMD_SW_SUSPEND: u32 = 0xD000FCE2;
    pub const LINUX_REBOOT_CMD_KEXEC: u32 = 0x45584543;
}

/// reboot syscall - Reboot, halt, or power off the system
///
/// magic1: Must be LINUX_REBOOT_MAGIC1
/// magic2: Must be one of LINUX_REBOOT_MAGIC2*
/// cmd: Command to execute (RESTART, HALT, POWER_OFF, etc.)
/// arg: Optional argument (unused for most commands)
pub fn sys_reboot(magic1: u32, magic2: u32, cmd: u32, _arg: u64) -> i64 {
    use reboot_magic::*;
    use reboot_cmd::*;

    // Only root can reboot
    let cred = crate::sched::current_cred();
    if cred.euid.0 != 0 {
        return errno::EPERM;
    }

    // Validate magic numbers (Linux compatibility)
    if magic1 != LINUX_REBOOT_MAGIC1 {
        return errno::EINVAL;
    }
    if magic2 != LINUX_REBOOT_MAGIC2
        && magic2 != LINUX_REBOOT_MAGIC2A
        && magic2 != LINUX_REBOOT_MAGIC2B
        && magic2 != LINUX_REBOOT_MAGIC2C
    {
        return errno::EINVAL;
    }

    match cmd {
        LINUX_REBOOT_CMD_RESTART | LINUX_REBOOT_CMD_RESTART2 => {
            crate::kprintln!("sys_reboot: restarting system...");
            // Sync filesystems
            // TODO: crate::fs::sync_all();
            crate::drivers::acpi::reboot();
        }
        LINUX_REBOOT_CMD_HALT => {
            crate::kprintln!("sys_reboot: halting system...");
            // Sync filesystems
            // TODO: crate::fs::sync_all();
            // Disable interrupts and halt
            x86_64::instructions::interrupts::disable();
            loop {
                x86_64::instructions::hlt();
            }
        }
        LINUX_REBOOT_CMD_POWER_OFF => {
            crate::kprintln!("sys_reboot: powering off...");
            // Sync filesystems
            // TODO: crate::fs::sync_all();
            crate::drivers::acpi::shutdown();
        }
        LINUX_REBOOT_CMD_SW_SUSPEND => {
            crate::kprintln!("sys_reboot: suspending to RAM (S3)...");
            match crate::drivers::acpi::suspend_to_ram() {
                Ok(()) => 0,
                Err(_) => errno::EINVAL,
            }
        }
        LINUX_REBOOT_CMD_CAD_ON | LINUX_REBOOT_CMD_CAD_OFF => {
            // Ctrl-Alt-Del handling - not implemented
            0
        }
        LINUX_REBOOT_CMD_KEXEC => {
            // kexec not implemented
            errno::EINVAL
        }
        _ => errno::EINVAL,
    }
}

/// clock_gettime - obtém tempo de um clock específico
pub fn sys_clock_gettime(clock_id: i32, tp: u64) -> i64 {
    if tp == 0 || !is_user_range(tp, core::mem::size_of::<crate::time::Timespec>()) {
        return errno::EFAULT;
    }

    match crate::time::clock_gettime(clock_id) {
        Some(ts) => {
            unsafe {
                core::ptr::write(tp as *mut crate::time::Timespec, ts);
            }
            0
        }
        None => errno::EINVAL,
    }
}

/// clock_getres - obtém resolução de um clock
pub fn sys_clock_getres(clock_id: i32, res: u64) -> i64 {
    // Todos os clocks têm resolução de 10ms (100Hz)
    if crate::time::clock_gettime(clock_id).is_none() {
        return errno::EINVAL;
    }

    if res != 0 {
        let resolution = crate::time::Timespec {
            tv_sec: 0,
            tv_nsec: 10_000_000, // 10ms em nanosegundos
        };
        unsafe {
            core::ptr::write(res as *mut crate::time::Timespec, resolution);
        }
    }

    0
}

/// nanosleep - dorme por um período específico
pub fn sys_nanosleep(req: u64, rem: u64) -> i64 {
    if req == 0 {
        return errno::EFAULT;
    }

    let request = unsafe { core::ptr::read(req as *const crate::time::Timespec) };

    // Valida os valores
    if request.tv_sec < 0 || request.tv_nsec < 0 || request.tv_nsec >= 1_000_000_000 {
        return errno::EINVAL;
    }

    let remaining = crate::time::nanosleep(&request);

    if rem != 0 {
        unsafe {
            core::ptr::write(rem as *mut crate::time::Timespec, remaining);
        }
    }

    0
}

// ======================== I/O Multiplexing syscalls ========================

/// Estrutura pollfd para poll()
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Pollfd {
    pub fd: i32,
    pub events: i16,
    pub revents: i16,
}

/// Flags de eventos para poll
pub mod poll_events {
    pub const POLLIN: i16 = 0x0001;      // Dados disponíveis para leitura
    pub const POLLPRI: i16 = 0x0002;     // Dados urgentes disponíveis
    pub const POLLOUT: i16 = 0x0004;     // Escrita não bloqueará
    pub const POLLERR: i16 = 0x0008;     // Erro ocorreu
    pub const POLLHUP: i16 = 0x0010;     // Hang up
    pub const POLLNVAL: i16 = 0x0020;    // fd inválido
    pub const POLLRDNORM: i16 = 0x0040;  // Normal read
    pub const POLLRDBAND: i16 = 0x0080;  // Priority read
    pub const POLLWRNORM: i16 = 0x0100;  // Normal write
    pub const POLLWRBAND: i16 = 0x0200;  // Priority write
}

/// poll - aguarda eventos em múltiplos file descriptors
///
/// fds: array de pollfd
/// nfds: número de file descriptors
/// timeout: timeout em milissegundos (-1 = infinito, 0 = não-bloqueante)
pub fn sys_poll(fds: u64, nfds: u64, timeout: i32) -> i64 {
    use poll_events::*;

    if fds == 0 && nfds > 0 {
        return errno::EFAULT;
    }

    let nfds = nfds as usize;
    if nfds > 1024 {
        return errno::EINVAL;
    }

    // Lê o array de pollfd
    let pollfds = unsafe {
        core::slice::from_raw_parts_mut(fds as *mut Pollfd, nfds)
    };

    let start_ticks = crate::time::ticks();
    let timeout_ticks = if timeout < 0 {
        u64::MAX // Infinito
    } else if timeout == 0 {
        0 // Não-bloqueante
    } else {
        // Converte ms para ticks (100 Hz = 10ms por tick)
        ((timeout as u64) + 9) / 10
    };

    loop {
        let mut ready_count = 0i64;

        {
            let table = fd_table();
            let table = match table.as_ref() {
                Some(t) => t,
                None => return errno::EBADF,
            };

            for pfd in pollfds.iter_mut() {
                pfd.revents = 0;

                if pfd.fd < 0 {
                    continue;
                }

                let entry = match table.get(pfd.fd) {
                    Some(e) => e,
                    None => {
                        pfd.revents = POLLNVAL;
                        ready_count += 1;
                        continue;
                    }
                };

                // Verifica eventos baseado no tipo de FD
                match &entry.fd_type {
                    FdType::Console => {
                        // Console sempre pronto para escrita
                        if pfd.events & POLLOUT != 0 {
                            pfd.revents |= POLLOUT;
                        }
                        // Verifica se tem dados para leitura
                        if pfd.events & POLLIN != 0 {
                            if crate::console::has_data() {
                                pfd.revents |= POLLIN;
                            }
                        }
                    }
                    FdType::File { .. } => {
                        // Arquivos sempre prontos para leitura e escrita
                        if pfd.events & POLLIN != 0 {
                            pfd.revents |= POLLIN;
                        }
                        if pfd.events & POLLOUT != 0 {
                            pfd.revents |= POLLOUT;
                        }
                    }
                    FdType::Dir { .. } => {
                        // Diretórios sempre prontos para leitura
                        if pfd.events & POLLIN != 0 {
                            pfd.revents |= POLLIN;
                        }
                    }
                    FdType::PipeRead { pipe } => {
                        // Pipe de leitura: POLLIN se tem dados
                        if pfd.events & POLLIN != 0 {
                            if !pipe.is_empty() {
                                pfd.revents |= POLLIN;
                            }
                        }
                        // POLLHUP se não há escritores
                        if !pipe.has_writers() {
                            pfd.revents |= POLLHUP;
                        }
                    }
                    FdType::PipeWrite { pipe } => {
                        // Pipe de escrita: POLLOUT se tem espaço
                        if pfd.events & POLLOUT != 0 {
                            if !pipe.is_full() {
                                pfd.revents |= POLLOUT;
                            }
                        }
                        // POLLERR se não há leitores
                        if !pipe.has_readers() {
                            pfd.revents |= POLLERR;
                        }
                    }
                    FdType::Socket { socket_id } => {
                        // Socket: verifica estado
                        let sock_id = *socket_id;
                        if pfd.events & POLLIN != 0 {
                            if crate::net::socket::poll_read(sock_id) {
                                pfd.revents |= POLLIN;
                            }
                        }
                        if pfd.events & POLLOUT != 0 {
                            if crate::net::socket::poll_write(sock_id) {
                                pfd.revents |= POLLOUT;
                            }
                        }
                    }
                    FdType::EventFd { eventfd } => {
                        // EventFd: check readability and writability
                        let (readable, writable) = eventfd.poll();
                        if pfd.events & POLLIN != 0 && readable {
                            pfd.revents |= POLLIN;
                        }
                        if pfd.events & POLLOUT != 0 && writable {
                            pfd.revents |= POLLOUT;
                        }
                    }
                }

                if pfd.revents != 0 {
                    ready_count += 1;
                }
            }
        }

        // Se algum FD está pronto ou timeout é 0, retorna
        if ready_count > 0 || timeout_ticks == 0 {
            return ready_count;
        }

        // Verifica timeout
        let elapsed = crate::time::ticks() - start_ticks;
        if timeout_ticks != u64::MAX && elapsed >= timeout_ticks {
            return 0; // Timeout
        }

        // Yield e tenta novamente
        crate::task::yield_now();
    }
}

/// select - aguarda eventos em múltiplos file descriptors (interface antiga)
///
/// nfds: maior fd + 1
/// readfds: set de fds para leitura
/// writefds: set de fds para escrita
/// exceptfds: set de fds para exceções
/// timeout: ponteiro para struct timeval
pub fn sys_select(nfds: i32, readfds: u64, writefds: u64, exceptfds: u64, timeout: u64) -> i64 {
    

    if nfds < 0 || nfds > 1024 {
        return errno::EINVAL;
    }

    // Lê o timeout
    let timeout_ms = if timeout == 0 {
        -1i32 // Infinito
    } else {
        let tv = unsafe { core::ptr::read(timeout as *const crate::time::Timeval) };
        if tv.tv_sec < 0 || tv.tv_usec < 0 {
            return errno::EINVAL;
        }
        // Converte para milissegundos
        let ms = tv.tv_sec * 1000 + tv.tv_usec / 1000;
        if ms > i32::MAX as i64 {
            i32::MAX
        } else {
            ms as i32
        }
    };

    // fd_set é um bitmap de 1024 bits (128 bytes)
    const FD_SETSIZE: usize = 1024;
    const NFDBITS: usize = 64;

    fn fd_isset(fd: i32, set: u64) -> bool {
        if set == 0 {
            return false;
        }
        let bits = unsafe { core::slice::from_raw_parts(set as *const u64, FD_SETSIZE / NFDBITS) };
        let word = fd as usize / NFDBITS;
        let bit = fd as usize % NFDBITS;
        (bits[word] & (1u64 << bit)) != 0
    }

    fn fd_set(fd: i32, set: u64) {
        if set == 0 {
            return;
        }
        let bits = unsafe { core::slice::from_raw_parts_mut(set as *mut u64, FD_SETSIZE / NFDBITS) };
        let word = fd as usize / NFDBITS;
        let bit = fd as usize % NFDBITS;
        bits[word] |= 1u64 << bit;
    }

    fn fd_zero(set: u64) {
        if set == 0 {
            return;
        }
        let bits = unsafe { core::slice::from_raw_parts_mut(set as *mut u64, FD_SETSIZE / NFDBITS) };
        for word in bits.iter_mut() {
            *word = 0;
        }
    }

    let start_ticks = crate::time::ticks();
    let timeout_ticks = if timeout_ms < 0 {
        u64::MAX
    } else if timeout_ms == 0 {
        0
    } else {
        ((timeout_ms as u64) + 9) / 10
    };

    loop {
        let mut ready_count = 0i64;

        // Copia os sets originais para restaurar depois
        let orig_read = if readfds != 0 {
            let mut v = [0u64; FD_SETSIZE / NFDBITS];
            unsafe {
                core::ptr::copy_nonoverlapping(readfds as *const u64, v.as_mut_ptr(), FD_SETSIZE / NFDBITS);
            }
            Some(v)
        } else {
            None
        };

        let orig_write = if writefds != 0 {
            let mut v = [0u64; FD_SETSIZE / NFDBITS];
            unsafe {
                core::ptr::copy_nonoverlapping(writefds as *const u64, v.as_mut_ptr(), FD_SETSIZE / NFDBITS);
            }
            Some(v)
        } else {
            None
        };

        // Limpa os sets de saída
        fd_zero(readfds);
        fd_zero(writefds);
        fd_zero(exceptfds);

        {
            let table = fd_table();
            let table = match table.as_ref() {
                Some(t) => t,
                None => return errno::EBADF,
            };

            for fd in 0..nfds {
                let check_read = orig_read.as_ref().map_or(false, |v| {
                    let word = fd as usize / NFDBITS;
                    let bit = fd as usize % NFDBITS;
                    (v[word] & (1u64 << bit)) != 0
                });

                let check_write = orig_write.as_ref().map_or(false, |v| {
                    let word = fd as usize / NFDBITS;
                    let bit = fd as usize % NFDBITS;
                    (v[word] & (1u64 << bit)) != 0
                });

                if !check_read && !check_write {
                    continue;
                }

                let entry = match table.get(fd) {
                    Some(e) => e,
                    None => continue,
                };

                match &entry.fd_type {
                    FdType::Console => {
                        if check_read && crate::console::has_data() {
                            fd_set(fd, readfds);
                            ready_count += 1;
                        }
                        if check_write {
                            fd_set(fd, writefds);
                            ready_count += 1;
                        }
                    }
                    FdType::File { .. } => {
                        if check_read {
                            fd_set(fd, readfds);
                            ready_count += 1;
                        }
                        if check_write {
                            fd_set(fd, writefds);
                            ready_count += 1;
                        }
                    }
                    FdType::Dir { .. } => {
                        if check_read {
                            fd_set(fd, readfds);
                            ready_count += 1;
                        }
                    }
                    FdType::PipeRead { pipe } => {
                        if check_read && !pipe.is_empty() {
                            fd_set(fd, readfds);
                            ready_count += 1;
                        }
                    }
                    FdType::PipeWrite { pipe } => {
                        if check_write && !pipe.is_full() {
                            fd_set(fd, writefds);
                            ready_count += 1;
                        }
                    }
                    FdType::Socket { socket_id } => {
                        let sock_id = *socket_id;
                        if check_read && crate::net::socket::poll_read(sock_id) {
                            fd_set(fd, readfds);
                            ready_count += 1;
                        }
                        if check_write && crate::net::socket::poll_write(sock_id) {
                            fd_set(fd, writefds);
                            ready_count += 1;
                        }
                    }
                    FdType::EventFd { eventfd } => {
                        let (readable, writable) = eventfd.poll();
                        if check_read && readable {
                            fd_set(fd, readfds);
                            ready_count += 1;
                        }
                        if check_write && writable {
                            fd_set(fd, writefds);
                            ready_count += 1;
                        }
                    }
                }
            }
        }

        if ready_count > 0 || timeout_ticks == 0 {
            return ready_count;
        }

        let elapsed = crate::time::ticks() - start_ticks;
        if timeout_ticks != u64::MAX && elapsed >= timeout_ticks {
            return 0;
        }

        crate::task::yield_now();
    }
}

/// ppoll - poll com timeout em timespec e máscara de sinais
///
/// fds: array de pollfd
/// nfds: número de file descriptors
/// tmo_p: ponteiro para timespec (ou NULL para infinito)
/// sigmask: máscara de sinais durante o wait
/// sigsetsize: tamanho da máscara (deve ser 8)
pub fn sys_ppoll(fds: u64, nfds: u64, tmo_p: u64, sigmask: u64, sigsetsize: u64) -> i64 {
    use poll_events::*;

    // Validate sigsetsize if sigmask is provided
    if sigmask != 0 && sigsetsize != 8 {
        return errno::EINVAL;
    }

    if fds == 0 && nfds > 0 {
        return errno::EFAULT;
    }

    let nfds = nfds as usize;
    if nfds > 1024 {
        return errno::EINVAL;
    }

    // Lê o array de pollfd
    let pollfds = unsafe {
        core::slice::from_raw_parts_mut(fds as *mut Pollfd, nfds)
    };

    // Lê o timeout (timespec)
    let timeout_ticks = if tmo_p == 0 {
        u64::MAX // NULL = infinito
    } else {
        let ts = unsafe { core::ptr::read(tmo_p as *const crate::time::Timespec) };
        if ts.tv_sec < 0 || ts.tv_nsec < 0 || ts.tv_nsec >= 1_000_000_000 {
            return errno::EINVAL;
        }
        // Converte para ticks (100 Hz = 10ms por tick)
        let ms = ts.tv_sec * 1000 + ts.tv_nsec / 1_000_000;
        if ms == 0 && ts.tv_nsec > 0 {
            1 // Pelo menos 1 tick para timeouts < 10ms
        } else if ms == 0 {
            0 // Non-blocking
        } else {
            ((ms as u64) + 9) / 10
        }
    };

    // Salva máscara de sinais antiga e aplica nova
    let old_mask = if sigmask != 0 {
        let new_mask = unsafe { core::ptr::read(sigmask as *const u64) };
        let old = crate::sched::get_signal_mask();
        crate::sched::set_signal_mask(new_mask);
        Some(old)
    } else {
        None
    };

    let start_ticks = crate::time::ticks();

    let result = loop {
        let mut ready_count = 0i64;

        {
            let table = fd_table();
            let table = match table.as_ref() {
                Some(t) => t,
                None => break errno::EBADF,
            };

            for pfd in pollfds.iter_mut() {
                pfd.revents = 0;

                if pfd.fd < 0 {
                    continue;
                }

                let entry = match table.get(pfd.fd) {
                    Some(e) => e,
                    None => {
                        pfd.revents = POLLNVAL;
                        ready_count += 1;
                        continue;
                    }
                };

                // Verifica eventos baseado no tipo de FD
                match &entry.fd_type {
                    FdType::Console => {
                        if pfd.events & POLLOUT != 0 {
                            pfd.revents |= POLLOUT;
                        }
                        if pfd.events & POLLIN != 0 {
                            if crate::console::has_data() {
                                pfd.revents |= POLLIN;
                            }
                        }
                    }
                    FdType::File { .. } => {
                        if pfd.events & POLLIN != 0 {
                            pfd.revents |= POLLIN;
                        }
                        if pfd.events & POLLOUT != 0 {
                            pfd.revents |= POLLOUT;
                        }
                    }
                    FdType::Dir { .. } => {
                        if pfd.events & POLLIN != 0 {
                            pfd.revents |= POLLIN;
                        }
                    }
                    FdType::PipeRead { pipe } => {
                        if pfd.events & POLLIN != 0 {
                            if !pipe.is_empty() {
                                pfd.revents |= POLLIN;
                            }
                        }
                        if !pipe.has_writers() {
                            pfd.revents |= POLLHUP;
                        }
                    }
                    FdType::PipeWrite { pipe } => {
                        if pfd.events & POLLOUT != 0 {
                            if !pipe.is_full() {
                                pfd.revents |= POLLOUT;
                            }
                        }
                        if !pipe.has_readers() {
                            pfd.revents |= POLLERR;
                        }
                    }
                    FdType::Socket { socket_id } => {
                        let sock_id = *socket_id;
                        if pfd.events & POLLIN != 0 {
                            if crate::net::socket::poll_read(sock_id) {
                                pfd.revents |= POLLIN;
                            }
                        }
                        if pfd.events & POLLOUT != 0 {
                            if crate::net::socket::poll_write(sock_id) {
                                pfd.revents |= POLLOUT;
                            }
                        }
                    }
                    FdType::EventFd { eventfd } => {
                        let (readable, writable) = eventfd.poll();
                        if pfd.events & POLLIN != 0 && readable {
                            pfd.revents |= POLLIN;
                        }
                        if pfd.events & POLLOUT != 0 && writable {
                            pfd.revents |= POLLOUT;
                        }
                    }
                }

                if pfd.revents != 0 {
                    ready_count += 1;
                }
            }
        }

        // Se algum FD está pronto ou timeout é 0, retorna
        if ready_count > 0 || timeout_ticks == 0 {
            break ready_count;
        }

        // Verifica timeout
        let elapsed = crate::time::ticks() - start_ticks;
        if timeout_ticks != u64::MAX && elapsed >= timeout_ticks {
            break 0; // Timeout
        }

        // Check for pending signals (would return EINTR)
        if crate::sched::has_pending_signals() {
            break errno::EINTR;
        }

        crate::task::yield_now();
    };

    // Restaura máscara de sinais antiga
    if let Some(old) = old_mask {
        crate::sched::set_signal_mask(old);
    }

    result
}

/// pselect6 - select com timeout em timespec e máscara de sinais
///
/// nfds: maior fd + 1
/// readfds: set de fds para leitura
/// writefds: set de fds para escrita
/// exceptfds: set de fds para exceções
/// timeout: ponteiro para timespec (ou NULL para infinito)
/// sig: ponteiro para struct { sigmask, sigsetsize }
pub fn sys_pselect6(nfds: i32, readfds: u64, writefds: u64, exceptfds: u64, timeout: u64, sig: u64) -> i64 {
    if nfds < 0 || nfds > 1024 {
        return errno::EINVAL;
    }

    // Lê o timeout (timespec)
    let timeout_ticks = if timeout == 0 {
        u64::MAX // NULL = infinito
    } else {
        let ts = unsafe { core::ptr::read(timeout as *const crate::time::Timespec) };
        if ts.tv_sec < 0 || ts.tv_nsec < 0 || ts.tv_nsec >= 1_000_000_000 {
            return errno::EINVAL;
        }
        let ms = ts.tv_sec * 1000 + ts.tv_nsec / 1_000_000;
        if ms == 0 && ts.tv_nsec > 0 {
            1
        } else if ms == 0 {
            0
        } else {
            ((ms as u64) + 9) / 10
        }
    };

    // Lê e aplica máscara de sinais
    // sig aponta para: struct { const sigset_t *ss; size_t ss_len; }
    let old_mask = if sig != 0 {
        #[repr(C)]
        struct SigData {
            ss: u64,      // ponteiro para sigset
            ss_len: u64,  // tamanho (deve ser 8)
        }
        let sig_data = unsafe { core::ptr::read(sig as *const SigData) };
        if sig_data.ss != 0 {
            if sig_data.ss_len != 8 {
                return errno::EINVAL;
            }
            let new_mask = unsafe { core::ptr::read(sig_data.ss as *const u64) };
            let old = crate::sched::get_signal_mask();
            crate::sched::set_signal_mask(new_mask);
            Some(old)
        } else {
            None
        }
    } else {
        None
    };

    // fd_set é um bitmap de 1024 bits (128 bytes)
    const FD_SETSIZE: usize = 1024;
    const NFDBITS: usize = 64;

    fn fd_set(fd: i32, set: u64) {
        if set == 0 {
            return;
        }
        let bits = unsafe { core::slice::from_raw_parts_mut(set as *mut u64, FD_SETSIZE / NFDBITS) };
        let word = fd as usize / NFDBITS;
        let bit = fd as usize % NFDBITS;
        bits[word] |= 1u64 << bit;
    }

    fn fd_zero(set: u64) {
        if set == 0 {
            return;
        }
        let bits = unsafe { core::slice::from_raw_parts_mut(set as *mut u64, FD_SETSIZE / NFDBITS) };
        for word in bits.iter_mut() {
            *word = 0;
        }
    }

    let start_ticks = crate::time::ticks();

    // Copia os sets originais
    let orig_read = if readfds != 0 {
        let mut v = [0u64; FD_SETSIZE / NFDBITS];
        unsafe {
            core::ptr::copy_nonoverlapping(readfds as *const u64, v.as_mut_ptr(), FD_SETSIZE / NFDBITS);
        }
        Some(v)
    } else {
        None
    };

    let orig_write = if writefds != 0 {
        let mut v = [0u64; FD_SETSIZE / NFDBITS];
        unsafe {
            core::ptr::copy_nonoverlapping(writefds as *const u64, v.as_mut_ptr(), FD_SETSIZE / NFDBITS);
        }
        Some(v)
    } else {
        None
    };

    let result = loop {
        let mut ready_count = 0i64;

        // Limpa os sets de saída
        fd_zero(readfds);
        fd_zero(writefds);
        fd_zero(exceptfds);

        {
            let table = fd_table();
            let table = match table.as_ref() {
                Some(t) => t,
                None => break errno::EBADF,
            };

            for fd in 0..nfds {
                let check_read = orig_read.as_ref().map_or(false, |v| {
                    let word = fd as usize / NFDBITS;
                    let bit = fd as usize % NFDBITS;
                    (v[word] & (1u64 << bit)) != 0
                });

                let check_write = orig_write.as_ref().map_or(false, |v| {
                    let word = fd as usize / NFDBITS;
                    let bit = fd as usize % NFDBITS;
                    (v[word] & (1u64 << bit)) != 0
                });

                if !check_read && !check_write {
                    continue;
                }

                let entry = match table.get(fd) {
                    Some(e) => e,
                    None => continue,
                };

                match &entry.fd_type {
                    FdType::Console => {
                        if check_read && crate::console::has_data() {
                            fd_set(fd, readfds);
                            ready_count += 1;
                        }
                        if check_write {
                            fd_set(fd, writefds);
                            ready_count += 1;
                        }
                    }
                    FdType::File { .. } => {
                        if check_read {
                            fd_set(fd, readfds);
                            ready_count += 1;
                        }
                        if check_write {
                            fd_set(fd, writefds);
                            ready_count += 1;
                        }
                    }
                    FdType::Dir { .. } => {
                        if check_read {
                            fd_set(fd, readfds);
                            ready_count += 1;
                        }
                    }
                    FdType::PipeRead { pipe } => {
                        if check_read && !pipe.is_empty() {
                            fd_set(fd, readfds);
                            ready_count += 1;
                        }
                    }
                    FdType::PipeWrite { pipe } => {
                        if check_write && !pipe.is_full() {
                            fd_set(fd, writefds);
                            ready_count += 1;
                        }
                    }
                    FdType::Socket { socket_id } => {
                        let sock_id = *socket_id;
                        if check_read && crate::net::socket::poll_read(sock_id) {
                            fd_set(fd, readfds);
                            ready_count += 1;
                        }
                        if check_write && crate::net::socket::poll_write(sock_id) {
                            fd_set(fd, writefds);
                            ready_count += 1;
                        }
                    }
                    FdType::EventFd { eventfd } => {
                        let (readable, writable) = eventfd.poll();
                        if check_read && readable {
                            fd_set(fd, readfds);
                            ready_count += 1;
                        }
                        if check_write && writable {
                            fd_set(fd, writefds);
                            ready_count += 1;
                        }
                    }
                }
            }
        }

        if ready_count > 0 || timeout_ticks == 0 {
            break ready_count;
        }

        let elapsed = crate::time::ticks() - start_ticks;
        if timeout_ticks != u64::MAX && elapsed >= timeout_ticks {
            break 0;
        }

        // Check for pending signals (would return EINTR)
        if crate::sched::has_pending_signals() {
            break errno::EINTR;
        }

        crate::task::yield_now();
    };

    // Restaura máscara de sinais antiga
    if let Some(old) = old_mask {
        crate::sched::set_signal_mask(old);
    }

    result
}

// ======================== Thread syscalls ========================

/// Flags de clone
pub mod clone_flags {
    pub const CLONE_VM: u64 = 0x00000100;        // Compartilha memória virtual
    pub const CLONE_FS: u64 = 0x00000200;        // Compartilha informações do filesystem
    pub const CLONE_FILES: u64 = 0x00000400;     // Compartilha tabela de file descriptors
    pub const CLONE_SIGHAND: u64 = 0x00000800;   // Compartilha handlers de signal
    pub const CLONE_THREAD: u64 = 0x00010000;    // Mesmo thread group
    pub const CLONE_PARENT: u64 = 0x00008000;    // Mesmo parent que caller
    pub const CLONE_CHILD_CLEARTID: u64 = 0x00200000; // Limpa TID no child ao sair
    pub const CLONE_CHILD_SETTID: u64 = 0x01000000;   // Seta TID no child
    pub const CLONE_PARENT_SETTID: u64 = 0x00100000;  // Seta TID no parent
    pub const CLONE_SETTLS: u64 = 0x00080000;    // Novo TLS para o child
    pub const CLONE_NEWNS: u64 = 0x00020000;     // Novo namespace de mount
    pub const CLONE_SYSVSEM: u64 = 0x00040000;   // Compartilha System V semaphores
    pub const CLONE_DETACHED: u64 = 0x00400000;  // Thread detached
    pub const CLONE_VFORK: u64 = 0x00004000;     // Parent espera child execve/exit
}

/// clone - cria um novo processo ou thread
///
/// flags: flags de clone (define o que é compartilhado)
/// child_stack: ponteiro para o stack do novo processo/thread
/// parent_tidptr: onde guardar o TID no parent
/// child_tidptr: onde guardar o TID no child
/// tls: endereço do TLS
pub fn sys_clone(
    sf: &crate::arch::x86_64_arch::syscall::SyscallFrame,
    flags: u64,
    child_stack: u64,
    parent_tidptr: u64,
    child_tidptr: u64,
    tls: u64,
) -> i64 {
    use clone_flags::*;

    // Verifica se é uma criação de thread (flags mínimas para pthread_create)
    let is_thread = (flags & CLONE_VM) != 0;

    if is_thread {
        // Criação de thread
        match crate::sched::clone_thread(sf, flags, child_stack, parent_tidptr, child_tidptr, tls) {
            Ok(tid) => tid as i64,
            Err(crate::util::KError::NoMemory) => errno::ENOMEM,
            Err(crate::util::KError::Invalid) => errno::EINVAL,
            Err(_) => errno::EIO,
        }
    } else {
        // Fork-like (sem compartilhamento de memória)
        // Reutiliza o fork existente mas com algumas diferenças
        match crate::sched::fork_current_from_syscall(sf) {
            Ok(child_pid) => {
                if (flags & CLONE_PARENT_SETTID) != 0 && parent_tidptr != 0 {
                    unsafe {
                        *(parent_tidptr as *mut i32) = child_pid as i32;
                    }
                }
                child_pid as i64
            }
            Err(crate::util::KError::NoMemory) => errno::ENOMEM,
            Err(_) => errno::EIO,
        }
    }
}

/// set_tid_address - define o endereço para limpar ao sair
///
/// O kernel escreverá 0 neste endereço e fará futex_wake quando a thread terminar.
pub fn sys_set_tid_address(tidptr: u64) -> i64 {
    crate::sched::set_clear_child_tid(tidptr);
    crate::sched::current_tid() as i64
}

/// gettid - retorna o ID da thread atual
pub fn sys_gettid() -> i64 {
    crate::sched::current_tid() as i64
}

/// getppid - retorna o ID do processo pai
pub fn sys_getppid() -> i64 {
    crate::sched::current_ppid() as i64
}

/// sched_yield - cede o processador
pub fn sys_sched_yield() -> i64 {
    crate::task::yield_now();
    0
}

/// sched_setaffinity - set CPU affinity mask for a task
pub fn sys_sched_setaffinity(pid: u64, cpusetsize: usize, mask_ptr: u64) -> i64 {
    use crate::sched::balance::CpuMask;

    // Validate user buffer
    if cpusetsize == 0 || cpusetsize > 32 {
        return errno::EINVAL;
    }

    let mask_bytes = unsafe {
        match validate_user_buffer(mask_ptr, cpusetsize) {
            Some(buf) => buf,
            None => return errno::EFAULT,
        }
    };

    // Parse the CPU mask
    let cpu_mask = CpuMask::from_cpu_set(mask_bytes);

    // Mask must have at least one CPU set
    if cpu_mask.is_empty() {
        return errno::EINVAL;
    }

    // pid == 0 means current process
    let target_pid = if pid == 0 {
        crate::sched::current_pid()
    } else {
        pid
    };

    // Set affinity
    if crate::sched::set_task_affinity(target_pid, cpu_mask) {
        0
    } else {
        errno::ESRCH // No such process
    }
}

/// sched_getaffinity - get CPU affinity mask for a task
pub fn sys_sched_getaffinity(pid: u64, cpusetsize: usize, mask_ptr: u64) -> i64 {
    // Validate user buffer
    if cpusetsize == 0 || cpusetsize > 32 {
        return errno::EINVAL;
    }

    let mask_bytes = unsafe {
        match validate_user_buffer_mut(mask_ptr, cpusetsize) {
            Some(buf) => buf,
            None => return errno::EFAULT,
        }
    };

    // pid == 0 means current process
    let target_pid = if pid == 0 {
        crate::sched::current_pid()
    } else {
        pid
    };

    // Get affinity
    match crate::sched::get_task_affinity(target_pid) {
        Some(cpu_mask) => {
            let mask_data = cpu_mask.to_bytes();
            let copy_len = cpusetsize.min(32);
            mask_bytes[..copy_len].copy_from_slice(&mask_data[..copy_len]);
            copy_len as i64
        }
        None => errno::ESRCH // No such process
    }
}

/// exit_group - termina todas as threads do grupo
pub fn sys_exit_group(status: u64) -> ! {
    crate::sched::exit_thread_group(status);
}

/// tgkill - envia signal para uma thread específica
pub fn sys_tgkill(tgid: i32, tid: i32, sig: i32) -> i64 {
    if sig < 0 {
        return errno::EINVAL;
    }

    match crate::sched::send_signal_to_thread(tgid as u64, tid as u64, sig as u32) {
        Ok(()) => 0,
        Err(crate::util::KError::NotFound) => errno::ESRCH,
        Err(crate::util::KError::Invalid) => errno::EINVAL,
        Err(crate::util::KError::PermissionDenied) => errno::EPERM,
        Err(_) => errno::EIO,
    }
}

/// tkill - envia signal para uma thread (obsoleto, use tgkill)
pub fn sys_tkill(tid: i32, sig: i32) -> i64 {
    // Para tkill, usamos tgid = -1 que significa "qualquer processo"
    sys_tgkill(-1, tid, sig)
}

// ======================== Signal syscalls ========================

/// rt_sigaction - define handler para um signal
///
/// signum: número do signal
/// act: ponteiro para nova sigaction (NULL para apenas consultar)
/// oldact: ponteiro para sigaction antiga (NULL para não retornar)
/// sigsetsize: tamanho da máscara de signals (deve ser 8 bytes)
pub fn sys_rt_sigaction(signum: i32, act: u64, oldact: u64, sigsetsize: usize) -> i64 {
    use crate::signal::{Sigaction, sig};

    // Tamanho da máscara deve ser 8 bytes (64 bits)
    if sigsetsize != 8 {
        return errno::EINVAL;
    }

    // Valida o signal
    if signum <= 0 || signum >= sig::NSIG as i32 {
        return errno::EINVAL;
    }

    // SIGKILL e SIGSTOP não podem ter handlers customizados
    if signum == sig::SIGKILL as i32 || signum == sig::SIGSTOP as i32 {
        return errno::EINVAL;
    }

    let task = crate::sched::current_task();
    let handlers = task.signal_handlers();

    // Obtém a ação antiga se requisitado
    if oldact != 0 {
        if let Some(old) = handlers.get(signum as u32) {
            unsafe {
                core::ptr::write(oldact as *mut Sigaction, old);
            }
        }
    }

    // Define nova ação se fornecida
    if act != 0 {
        let new_act = unsafe { core::ptr::read(act as *const Sigaction) };
        handlers.set(signum as u32, &new_act);
    }

    0
}

/// rt_sigprocmask - altera a máscara de signals bloqueados
///
/// how: SIG_BLOCK, SIG_UNBLOCK, ou SIG_SETMASK
/// set: ponteiro para nova máscara
/// oldset: ponteiro para máscara antiga
/// sigsetsize: tamanho da máscara (deve ser 8)
pub fn sys_rt_sigprocmask(how: i32, set: u64, oldset: u64, sigsetsize: usize) -> i64 {
    use crate::signal::sigprocmask::*;

    if sigsetsize != 8 {
        return errno::EINVAL;
    }

    let task = crate::sched::current_task();
    let signals = task.signals();

    // Retorna a máscara antiga se requisitado
    if oldset != 0 {
        let old_mask = signals.blocked_mask();
        unsafe {
            core::ptr::write(oldset as *mut u64, old_mask);
        }
    }

    // Modifica a máscara se fornecida
    if set != 0 {
        let new_mask = unsafe { core::ptr::read(set as *const u64) };
        let old_mask = signals.blocked_mask();

        let result_mask = match how {
            SIG_BLOCK => old_mask | new_mask,
            SIG_UNBLOCK => old_mask & !new_mask,
            SIG_SETMASK => new_mask,
            _ => return errno::EINVAL,
        };

        signals.set_blocked_mask(result_mask);
    }

    0
}

/// rt_sigreturn - retorna de um signal handler
///
/// Esta syscall restaura o contexto salvo no SignalFrame e continua
/// a execução do código original.
pub fn sys_rt_sigreturn(frame: &mut crate::arch::x86_64_arch::syscall::SyscallFrame) -> i64 {
    use crate::signal;

    // O frame do signal está na stack do usuário
    // RSP aponta para logo após o endereço de retorno (que foi o sa_restorer)
    let user_rsp = unsafe { (*crate::arch::x86_64_arch::syscall::cpu_local_ptr()).user_rsp_tmp };

    // O SignalFrame está 8 bytes abaixo (o restorer foi chamado)
    // Na verdade, precisamos encontrar o início do SignalFrame
    // O layout é: [restorer addr][SignalFrame]
    // Quando o handler retorna (ret), RSP aponta para o restorer
    // O restorer chama sigreturn, então RSP está 8 bytes acima do frame
    let frame_ptr = user_rsp;

    if let Some((rip, rsp, rflags, regs, new_mask)) = signal::restore_signal_frame(frame_ptr) {
        // Restaura a máscara de signals
        let task = crate::sched::current_task();
        task.signals().set_blocked_mask(new_mask);

        // Atualiza o frame do syscall para retornar ao contexto original
        frame.rcx = rip;           // RIP de retorno
        frame.r11 = rflags;        // RFLAGS
        frame.r15 = regs[0];
        frame.r14 = regs[1];
        frame.r13 = regs[2];
        frame.r12 = regs[3];
        // r11 já é rflags
        frame.r10 = regs[5];
        frame.r9 = regs[6];
        frame.r8 = regs[7];
        frame.rbp = regs[8];
        frame.rdi = regs[9];
        frame.rsi = regs[10];
        frame.rdx = regs[11];
        // rcx já é rip
        frame.rbx = regs[13];
        frame.rax = regs[14];

        // Atualiza o RSP do usuário
        unsafe {
            (*crate::arch::x86_64_arch::syscall::cpu_local_ptr()).user_rsp_tmp = rsp;
        }

        // Retorna o valor de RAX salvo (o retorno original)
        regs[14] as i64
    } else {
        // Erro ao restaurar frame - mata o processo
        crate::kprintln!("sigreturn: erro ao restaurar frame, terminando processo");
        errno::EFAULT
    }
}

/// rt_sigpending - obtém signals pendentes
pub fn sys_rt_sigpending(set: u64, sigsetsize: usize) -> i64 {
    if sigsetsize != 8 {
        return errno::EINVAL;
    }

    if set == 0 {
        return errno::EFAULT;
    }

    let task = crate::sched::current_task();
    let pending = task.signals().pending_mask();

    unsafe {
        core::ptr::write(set as *mut u64, pending);
    }

    0
}

/// rt_sigsuspend - suspende esperando um signal
pub fn sys_rt_sigsuspend(mask: u64, sigsetsize: usize) -> i64 {
    if sigsetsize != 8 {
        return errno::EINVAL;
    }

    let task = crate::sched::current_task();
    let signals = task.signals();

    // Salva a máscara atual
    let old_mask = signals.blocked_mask();

    // Define a nova máscara temporária
    if mask != 0 {
        let new_mask = unsafe { core::ptr::read(mask as *const u64) };
        signals.set_blocked_mask(new_mask);
    }

    // Espera por um signal
    loop {
        if signals.has_pending() {
            break;
        }
        crate::task::yield_now();
    }

    // Restaura a máscara original
    signals.set_blocked_mask(old_mask);

    // sigsuspend sempre retorna -EINTR
    errno::EINTR
}

/// sigaltstack - define stack alternativo para signals
pub fn sys_sigaltstack(_ss: u64, old_ss: u64) -> i64 {
    // Por enquanto, apenas retorna sucesso sem fazer nada
    // Um OS completo manteria um stack alternativo por processo

    if old_ss != 0 {
        // Retorna stack vazio
        unsafe {
            let stack = crate::signal::StackT::empty();
            core::ptr::write(old_ss as *mut crate::signal::StackT, stack);
        }
    }

    // TODO: salvar o novo stack alternativo se ss != 0

    0
}

// ======================== Futex syscalls ========================

/// Operações do futex
pub mod futex_op {
    pub const FUTEX_WAIT: i32 = 0;          // Espera se valor == expected
    pub const FUTEX_WAKE: i32 = 1;          // Acorda até N waiters
    pub const FUTEX_FD: i32 = 2;            // (deprecated)
    pub const FUTEX_REQUEUE: i32 = 3;       // Acorda alguns, move outros para outro futex
    pub const FUTEX_CMP_REQUEUE: i32 = 4;   // Requeue com comparação
    pub const FUTEX_WAKE_OP: i32 = 5;       // Wake com operação atômica
    pub const FUTEX_LOCK_PI: i32 = 6;       // Priority inheritance lock
    pub const FUTEX_UNLOCK_PI: i32 = 7;     // Priority inheritance unlock
    pub const FUTEX_TRYLOCK_PI: i32 = 8;    // Trylock with PI
    pub const FUTEX_WAIT_BITSET: i32 = 9;   // Wait com bitmask
    pub const FUTEX_WAKE_BITSET: i32 = 10;  // Wake com bitmask

    // Flags adicionais
    pub const FUTEX_PRIVATE_FLAG: i32 = 128;  // Privado ao processo
    pub const FUTEX_CLOCK_REALTIME: i32 = 256; // Usa CLOCK_REALTIME

    // Operação base (sem flags)
    pub const fn op_base(op: i32) -> i32 {
        op & !FUTEX_PRIVATE_FLAG & !FUTEX_CLOCK_REALTIME
    }
}

/// Estrutura para rastrear waiters em um futex

/// Waiter em um futex
#[derive(Debug, Clone)]
struct FutexWaiter {
    task_id: u64,
    bitset: u32,
}

/// Tabela global de futex waiters
/// Chave: endereço do futex (físico para evitar problemas com mmap)
/// Valor: lista de tasks esperando
static FUTEX_WAITERS: IrqSafeMutex<BTreeMap<u64, Vec<FutexWaiter>>> = IrqSafeMutex::new(BTreeMap::new());

/// futex - fast userspace mutex
///
/// uaddr: endereço do futex (32 bits alinhado)
/// op: operação a realizar
/// val: valor esperado (para WAIT) ou número de waiters (para WAKE)
/// timeout: ponteiro para timespec (ou NULL)
/// uaddr2: segundo endereço (para REQUEUE)
/// val3: valor adicional (bitset para WAIT_BITSET/WAKE_BITSET)
pub fn sys_futex(uaddr: u64, op: i32, val: u32, timeout: u64, uaddr2: u64, val3: u32) -> i64 {
    use futex_op::*;

    // Verifica alinhamento
    if uaddr & 3 != 0 {
        return errno::EINVAL;
    }

    let op_base = op_base(op);

    match op_base {
        FUTEX_WAIT | FUTEX_WAIT_BITSET => {
            futex_wait(uaddr, val, timeout, if op_base == FUTEX_WAIT_BITSET { val3 } else { u32::MAX })
        }
        FUTEX_WAKE | FUTEX_WAKE_BITSET => {
            futex_wake(uaddr, val as i32, if op_base == FUTEX_WAKE_BITSET { val3 } else { u32::MAX })
        }
        FUTEX_REQUEUE => {
            futex_requeue(uaddr, val as i32, uaddr2, val3 as i32, false, 0)
        }
        FUTEX_CMP_REQUEUE => {
            // val3 é o valor esperado para comparação
            futex_requeue(uaddr, val as i32, uaddr2, timeout as i32, true, val3)
        }
        _ => {
            // Operação não suportada
            errno::ENOSYS
        }
    }
}

/// FUTEX_WAIT: espera se *uaddr == val
fn futex_wait(uaddr: u64, val: u32, timeout: u64, bitset: u32) -> i64 {
    // Verifica se o valor atual é igual ao esperado
    let current = unsafe { core::ptr::read_volatile(uaddr as *const u32) };
    if current != val {
        return errno::EAGAIN; // Valor mudou, não espera
    }

    let task_id = crate::sched::current_tid();

    // Adiciona à lista de waiters
    {
        let mut waiters = FUTEX_WAITERS.lock();
        let list = waiters.entry(uaddr).or_insert_with(Vec::new);
        list.push(FutexWaiter { task_id, bitset });
    }

    // Calcula deadline se timeout fornecido
    let deadline = if timeout != 0 {
        let ts = unsafe { core::ptr::read(timeout as *const crate::time::Timespec) };
        if ts.tv_sec < 0 || ts.tv_nsec < 0 {
            return errno::EINVAL;
        }
        let timeout_ns = ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64;
        Some(crate::time::uptime_ns() + timeout_ns)
    } else {
        None
    };

    // Espera até ser acordado ou timeout
    loop {
        // Verifica se ainda está na lista de waiters
        {
            let waiters = FUTEX_WAITERS.lock();
            if let Some(list) = waiters.get(&uaddr) {
                if !list.iter().any(|w| w.task_id == task_id) {
                    // Foi removido da lista - acordado!
                    return 0;
                }
            } else {
                // Lista não existe mais - acordado!
                return 0;
            }
        }

        // Verifica timeout
        if let Some(dl) = deadline {
            if crate::time::uptime_ns() >= dl {
                // Remove da lista de waiters
                let mut waiters = FUTEX_WAITERS.lock();
                if let Some(list) = waiters.get_mut(&uaddr) {
                    list.retain(|w| w.task_id != task_id);
                    if list.is_empty() {
                        waiters.remove(&uaddr);
                    }
                }
                return errno::ETIMEDOUT;
            }
        }

        // Yield
        crate::task::yield_now();
    }
}

/// FUTEX_WAKE: acorda até n waiters
fn futex_wake(uaddr: u64, n: i32, bitset: u32) -> i64 {
    if n < 0 {
        return errno::EINVAL;
    }

    let mut waiters = FUTEX_WAITERS.lock();
    let list = match waiters.get_mut(&uaddr) {
        Some(l) => l,
        None => return 0, // Nenhum waiter
    };

    let mut woken = 0i64;
    let n = n as usize;

    // Acorda waiters cujo bitset faz match
    let mut i = 0;
    while i < list.len() && woken < n as i64 {
        if (list[i].bitset & bitset) != 0 {
            list.remove(i);
            woken += 1;
        } else {
            i += 1;
        }
    }

    // Remove a entrada se não há mais waiters
    if list.is_empty() {
        waiters.remove(&uaddr);
    }

    woken
}

/// Internal function to wake futex waiters (used by sched for clear_child_tid)
pub fn futex_wake_internal(uaddr: u64, n: i32) {
    futex_wake(uaddr, n, u32::MAX);
}

/// FUTEX_REQUEUE: acorda alguns e move outros para outro futex
fn futex_requeue(uaddr: u64, wake_n: i32, uaddr2: u64, requeue_n: i32, cmp: bool, expected: u32) -> i64 {
    // Se cmp, verifica o valor antes
    if cmp {
        let current = unsafe { core::ptr::read_volatile(uaddr as *const u32) };
        if current != expected {
            return errno::EAGAIN;
        }
    }

    let mut waiters = FUTEX_WAITERS.lock();

    // Obtém a lista do primeiro futex
    let list = match waiters.get_mut(&uaddr) {
        Some(l) => l,
        None => return 0,
    };

    let mut woken = 0i64;
    let mut requeued = 0i64;
    let wake_n = wake_n as usize;
    let requeue_n = requeue_n as usize;

    // Primeiro acorda até wake_n
    while !list.is_empty() && woken < wake_n as i64 {
        list.remove(0);
        woken += 1;
    }

    // Depois move até requeue_n para o segundo futex
    let mut to_requeue = Vec::new();
    while !list.is_empty() && requeued < requeue_n as i64 {
        to_requeue.push(list.remove(0));
        requeued += 1;
    }

    // Remove a entrada se vazia
    if list.is_empty() {
        waiters.remove(&uaddr);
    }

    // Adiciona ao segundo futex
    if !to_requeue.is_empty() {
        let list2 = waiters.entry(uaddr2).or_insert_with(Vec::new);
        list2.extend(to_requeue);
    }

    woken
}

pub fn sys_exit(status: u64) -> ! {
    crate::sched::exit_current_and_switch(status);
}

/// pipe - cria um pipe (canal unidirecional)
///
/// Retorna dois file descriptors: pipefd[0] para leitura, pipefd[1] para escrita.
pub fn sys_pipe(pipefd: u64) -> i64 {
    if pipefd == 0 {
        return errno::EFAULT;
    }

    let pipe = Pipe::new();

    let mut table = fd_table();
    let table = match table.as_mut() {
        Some(t) => t,
        None => return errno::ENOMEM,
    };

    // Aloca dois FDs
    let read_fd = table.alloc();
    let write_fd = table.alloc();

    // Insere os FDs
    table.insert(read_fd, FdEntry {
        fd_type: FdType::PipeRead { pipe: pipe.clone() },
    });
    table.insert(write_fd, FdEntry {
        fd_type: FdType::PipeWrite { pipe },
    });

    // Escreve os FDs no array do usuário
    unsafe {
        let fd_array = pipefd as *mut i32;
        *fd_array = read_fd;
        *fd_array.add(1) = write_fd;
    }

    0
}

/// dup - duplica um file descriptor
///
/// Retorna um novo FD que aponta para o mesmo recurso.
pub fn sys_dup(oldfd: i32) -> i64 {
    let mut table = fd_table();
    let table = match table.as_mut() {
        Some(t) => t,
        None => return errno::EBADF,
    };

    // Obtém o FD antigo
    let old_entry = match table.get(oldfd) {
        Some(e) => e,
        None => return errno::EBADF,
    };

    // Clona o tipo e incrementa contadores se for pipe
    let new_fd_type = match &old_entry.fd_type {
        FdType::PipeRead { pipe } => {
            pipe.add_reader();
            FdType::PipeRead { pipe: pipe.clone() }
        }
        FdType::PipeWrite { pipe } => {
            pipe.add_writer();
            FdType::PipeWrite { pipe: pipe.clone() }
        }
        other => other.clone(),
    };

    // Aloca novo FD
    let newfd = table.alloc();
    table.insert(newfd, FdEntry { fd_type: new_fd_type });

    newfd as i64
}

/// dup2 - duplica um file descriptor para um FD específico
///
/// Se newfd já estiver aberto, é fechado primeiro.
pub fn sys_dup2(oldfd: i32, newfd: i32) -> i64 {
    if oldfd == newfd {
        // Se são iguais, apenas verifica se oldfd é válido
        let table = fd_table();
        let table = match table.as_ref() {
            Some(t) => t,
            None => return errno::EBADF,
        };
        if table.get(oldfd).is_some() {
            return newfd as i64;
        } else {
            return errno::EBADF;
        }
    }

    let mut table = fd_table();
    let table = match table.as_mut() {
        Some(t) => t,
        None => return errno::EBADF,
    };

    // Obtém o FD antigo
    let old_entry = match table.get(oldfd) {
        Some(e) => e,
        None => return errno::EBADF,
    };

    // Clona o tipo e incrementa contadores se for pipe
    let new_fd_type = match &old_entry.fd_type {
        FdType::PipeRead { pipe } => {
            pipe.add_reader();
            FdType::PipeRead { pipe: pipe.clone() }
        }
        FdType::PipeWrite { pipe } => {
            pipe.add_writer();
            FdType::PipeWrite { pipe: pipe.clone() }
        }
        other => other.clone(),
    };

    // Se newfd já está aberto, fecha primeiro
    if let Some(old_new_entry) = table.remove(newfd) {
        match old_new_entry.fd_type {
            FdType::PipeRead { pipe } => pipe.remove_reader(),
            FdType::PipeWrite { pipe } => pipe.remove_writer(),
            _ => {}
        }
    }

    // Insere o novo FD
    table.insert(newfd, FdEntry { fd_type: new_fd_type });

    newfd as i64
}

/// ioctl request numbers (Linux x86_64)
pub mod ioctl_nr {
    // Terminal ioctls
    pub const TCGETS: u64 = 0x5401;      // Get termios
    pub const TCSETS: u64 = 0x5402;      // Set termios
    pub const TCSETSW: u64 = 0x5403;     // Set termios, wait for drain
    pub const TCSETSF: u64 = 0x5404;     // Set termios, flush
    pub const TIOCGWINSZ: u64 = 0x5413;  // Get window size
    pub const TIOCSWINSZ: u64 = 0x5414;  // Set window size
    pub const TIOCGPGRP: u64 = 0x540F;   // Get foreground pgrp
    pub const TIOCSPGRP: u64 = 0x5410;   // Set foreground pgrp
    pub const TIOCNOTTY: u64 = 0x5422;   // Give up controlling terminal
    pub const TIOCSCTTY: u64 = 0x540E;   // Become controlling terminal
    pub const TIOCGPTN: u64 = 0x80045430; // Get pty number
    pub const TIOCSPTLCK: u64 = 0x40045431; // Lock/unlock pty

    // File ioctls
    pub const FIONREAD: u64 = 0x541B;    // Bytes available to read
    pub const FIONBIO: u64 = 0x5421;     // Set/clear non-blocking I/O
    pub const FIONCLEX: u64 = 0x5450;    // Clear close-on-exec
    pub const FIOCLEX: u64 = 0x5451;     // Set close-on-exec

    // Socket ioctls
    pub const SIOCGIFNAME: u64 = 0x8910;
    pub const SIOCGIFINDEX: u64 = 0x8933;
    pub const SIOCGIFFLAGS: u64 = 0x8913;
    pub const SIOCSIFFLAGS: u64 = 0x8914;
    pub const SIOCGIFADDR: u64 = 0x8915;
    pub const SIOCSIFADDR: u64 = 0x8916;
    pub const SIOCGIFNETMASK: u64 = 0x891B;
    pub const SIOCSIFNETMASK: u64 = 0x891C;
    pub const SIOCGIFHWADDR: u64 = 0x8927;
    pub const SIOCSIFHWADDR: u64 = 0x8924;
}

/// termios structure (simplified)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Termios {
    pub c_iflag: u32,   // Input flags
    pub c_oflag: u32,   // Output flags
    pub c_cflag: u32,   // Control flags
    pub c_lflag: u32,   // Local flags
    pub c_line: u8,     // Line discipline
    pub c_cc: [u8; 32], // Control characters
    pub c_ispeed: u32,  // Input speed
    pub c_ospeed: u32,  // Output speed
}

impl Default for Termios {
    fn default() -> Self {
        let mut cc = [0u8; 32];
        // Default control characters
        cc[0] = 3;    // VINTR = Ctrl-C
        cc[1] = 28;   // VQUIT = Ctrl-\
        cc[2] = 127;  // VERASE = DEL
        cc[3] = 21;   // VKILL = Ctrl-U
        cc[4] = 4;    // VEOF = Ctrl-D
        cc[5] = 0;    // VTIME
        cc[6] = 1;    // VMIN
        cc[8] = 17;   // VSTART = Ctrl-Q
        cc[9] = 19;   // VSTOP = Ctrl-S
        cc[10] = 26;  // VSUSP = Ctrl-Z

        Self {
            c_iflag: 0x0500,  // ICRNL | IXON
            c_oflag: 0x0005,  // OPOST | ONLCR
            c_cflag: 0x00BF,  // CS8 | CREAD | HUPCL
            c_lflag: 0x8A3B,  // ISIG | ICANON | ECHO | ECHOE | ECHOK | ECHOCTL | ECHOKE | IEXTEN
            c_line: 0,
            c_cc: cc,
            c_ispeed: 38400,
            c_ospeed: 38400,
        }
    }
}

/// Window size structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Winsize {
    pub ws_row: u16,
    pub ws_col: u16,
    pub ws_xpixel: u16,
    pub ws_ypixel: u16,
}

impl Default for Winsize {
    fn default() -> Self {
        Self {
            ws_row: 25,
            ws_col: 80,
            ws_xpixel: 0,
            ws_ypixel: 0,
        }
    }
}

/// Global terminal state
use spin::Mutex as SpinMutex;

static TERMINAL_STATE: SpinMutex<TerminalState> = SpinMutex::new(TerminalState::new());

struct TerminalState {
    termios: Termios,
    winsize: Winsize,
    foreground_pgrp: i32,
}

impl TerminalState {
    const fn new() -> Self {
        Self {
            termios: Termios {
                c_iflag: 0x0500,
                c_oflag: 0x0005,
                c_cflag: 0x00BF,
                c_lflag: 0x8A3B,
                c_line: 0,
                c_cc: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                       0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                c_ispeed: 38400,
                c_ospeed: 38400,
            },
            winsize: Winsize {
                ws_row: 25,
                ws_col: 80,
                ws_xpixel: 0,
                ws_ypixel: 0,
            },
            foreground_pgrp: 1,
        }
    }
}

pub fn sys_ioctl(fd: i32, request: u64, arg: u64) -> i64 {
    use ioctl_nr::*;

    // Get the fd entry to determine the type
    let table = fd_table();
    let table = match table.as_ref() {
        Some(t) => t,
        None => return errno::EBADF,
    };

    let entry = match table.get(fd) {
        Some(e) => e,
        None => return errno::EBADF,
    };

    match request {
        // Terminal ioctls
        TCGETS => {
            if !is_user_range(arg, core::mem::size_of::<Termios>()) {
                return errno::EFAULT;
            }
            let state = TERMINAL_STATE.lock();
            unsafe {
                core::ptr::write(arg as *mut Termios, state.termios);
            }
            0
        }
        TCSETS | TCSETSW | TCSETSF => {
            if !is_user_range(arg, core::mem::size_of::<Termios>()) {
                return errno::EFAULT;
            }
            let mut state = TERMINAL_STATE.lock();
            unsafe {
                state.termios = core::ptr::read(arg as *const Termios);
            }
            0
        }
        TIOCGWINSZ => {
            if !is_user_range(arg, core::mem::size_of::<Winsize>()) {
                return errno::EFAULT;
            }
            let state = TERMINAL_STATE.lock();
            unsafe {
                core::ptr::write(arg as *mut Winsize, state.winsize);
            }
            0
        }
        TIOCSWINSZ => {
            if !is_user_range(arg, core::mem::size_of::<Winsize>()) {
                return errno::EFAULT;
            }
            let mut state = TERMINAL_STATE.lock();
            unsafe {
                state.winsize = core::ptr::read(arg as *const Winsize);
            }
            // Could send SIGWINCH to foreground process group here
            0
        }
        TIOCGPGRP => {
            if !is_user_range(arg, core::mem::size_of::<i32>()) {
                return errno::EFAULT;
            }
            let state = TERMINAL_STATE.lock();
            unsafe {
                core::ptr::write(arg as *mut i32, state.foreground_pgrp);
            }
            0
        }
        TIOCSPGRP => {
            if !is_user_range(arg, core::mem::size_of::<i32>()) {
                return errno::EFAULT;
            }
            let mut state = TERMINAL_STATE.lock();
            unsafe {
                state.foreground_pgrp = core::ptr::read(arg as *const i32);
            }
            0
        }
        TIOCNOTTY => {
            // Give up controlling terminal - stub
            0
        }
        TIOCSCTTY => {
            // Become controlling terminal - stub
            0
        }

        // File ioctls
        FIONREAD => {
            // Return bytes available to read
            if !is_user_range(arg, core::mem::size_of::<i32>()) {
                return errno::EFAULT;
            }
            let bytes_available: i32 = match &entry.fd_type {
                FdType::PipeRead { pipe } => pipe.available() as i32,
                FdType::Console => {
                    if crate::console::has_data() { 1 } else { 0 }
                }
                FdType::File { inode, offset, .. } => {
                    let size = inode.0.size().unwrap_or(0);
                    (size.saturating_sub(*offset)) as i32
                }
                FdType::Socket { socket_id } => {
                    crate::net::socket::available(*socket_id) as i32
                }
                _ => 0,
            };
            unsafe {
                core::ptr::write(arg as *mut i32, bytes_available);
            }
            0
        }
        FIONBIO => {
            // Set/clear non-blocking - stub (would need to modify fd flags)
            0
        }
        FIOCLEX | FIONCLEX => {
            // Set/clear close-on-exec - stub
            0
        }

        // Unknown ioctl - return success for compatibility
        _ => {
            // For unhandled ioctls, return success (many programs check for errors)
            0
        }
    }
}

pub fn sys_arch_prctl(code: i32, addr: u64) -> i64 {
    const ARCH_SET_FS: i32 = 0x1002;
    const ARCH_GET_FS: i32 = 0x1003;
    const ARCH_SET_GS: i32 = 0x1001;
    const ARCH_GET_GS: i32 = 0x1004;

    match code {
        ARCH_SET_FS => {
            // Define FS base (TLS).
            x86_64::registers::model_specific::FsBase::write(
                x86_64::VirtAddr::new(addr),
            );
            0
        }
        ARCH_GET_FS => {
            let fs = x86_64::registers::model_specific::FsBase::read();
            unsafe { *(addr as *mut u64) = fs.as_u64(); }
            0
        }
        _ => errno::EINVAL,
    }
}

/// Lê um array de strings terminado em NULL de memória do usuário.
unsafe fn read_user_string_array(ptr: u64, max_count: usize) -> Option<Vec<String>> {
    if ptr == 0 {
        return Some(Vec::new());
    }

    let mut result = Vec::new();
    let array = ptr as *const u64;

    for i in 0..max_count {
        let str_ptr = *array.add(i);
        if str_ptr == 0 {
            break;
        }
        if let Some(s) = read_user_string(str_ptr, 4096) {
            result.push(s);
        } else {
            return None;
        }
    }

    Some(result)
}

/// fork - cria uma cópia do processo atual
///
/// Retorna 0 para o filho e o PID do filho para o pai.
/// Precisa receber o SyscallFrame do processo para copiar o estado.
pub fn sys_fork(sf: &crate::arch::x86_64_arch::syscall::SyscallFrame) -> i64 {
    match crate::sched::fork_current_from_syscall(sf) {
        Ok(child_pid) => child_pid as i64,
        Err(KError::NoMemory) => errno::ENOMEM,
        Err(KError::NotSupported) => errno::ENOSYS,
        Err(_) => errno::EIO,
    }
}

/// Wait options for wait4/waitpid
pub mod wait_options {
    pub const WNOHANG: i32 = 1;      // Return immediately if no child has exited
    pub const WUNTRACED: i32 = 2;    // Also return for stopped children
    pub const WCONTINUED: i32 = 8;   // Also return for continued children
    pub const WNOWAIT: i32 = 0x01000000; // Leave child in waitable state

    // Macros to extract status information
    pub fn wifexited(status: i32) -> bool {
        (status & 0x7f) == 0
    }
    pub fn wexitstatus(status: i32) -> i32 {
        (status >> 8) & 0xff
    }
    pub fn wifsignaled(status: i32) -> bool {
        let s = status & 0x7f;
        s != 0 && s != 0x7f
    }
    pub fn wtermsig(status: i32) -> i32 {
        status & 0x7f
    }
    pub fn wifstopped(status: i32) -> bool {
        (status & 0xff) == 0x7f
    }
    pub fn wstopsig(status: i32) -> i32 {
        (status >> 8) & 0xff
    }
    pub fn wifcontinued(status: i32) -> bool {
        status == 0xffff
    }
}

/// wait4 - aguarda um processo filho terminar
///
/// pid: -1 para qualquer filho, ou PID específico
/// status: ponteiro para guardar o status de saída
/// options: flags (WNOHANG, WUNTRACED, etc)
/// rusage: ponteiro para resource usage (ignorado por enquanto)
pub fn sys_wait4(pid: i64, status: u64, options: i32, _rusage: u64) -> i64 {
    use wait_options::WNOHANG;

    match crate::sched::wait_for_child_with_options(pid, status, options) {
        Ok(Some(child_pid)) => child_pid as i64,
        Ok(None) => {
            // WNOHANG was set and no child was ready
            if (options & WNOHANG) != 0 {
                0
            } else {
                errno::ECHILD
            }
        }
        Err(KError::NoChild) => errno::ECHILD,
        Err(KError::WouldBlock) => 0, // WNOHANG with no ready child
        Err(_) => errno::EIO,
    }
}

/// waitid - wait for a child process to change state
///
/// idtype: P_PID, P_PGID, or P_ALL
/// id: child PID or PGID depending on idtype
/// infop: pointer to siginfo_t structure
/// options: wait options
pub fn sys_waitid(idtype: i32, id: u64, infop: u64, options: i32) -> i64 {
    use wait_options::*;

    // idtype constants
    const P_ALL: i32 = 0;
    const P_PID: i32 = 1;
    const P_PGID: i32 = 2;

    // Convert idtype to pid for wait_for_child
    let pid = match idtype {
        P_ALL => -1i64,
        P_PID => id as i64,
        P_PGID => -(id as i64), // Negative for process group
        _ => return errno::EINVAL,
    };

    match crate::sched::wait_for_child_with_options(pid, 0, options) {
        Ok(Some(child_pid)) => {
            // Fill in siginfo if provided
            if infop != 0 {
                let info = crate::signal::Siginfo::new(crate::signal::sig::SIGCHLD as i32);
                unsafe {
                    let info_ptr = infop as *mut crate::signal::Siginfo;
                    core::ptr::write(info_ptr, info);
                    (*info_ptr).si_pid = child_pid as i32;
                }
            }
            0
        }
        Ok(None) => {
            if (options & WNOHANG) != 0 {
                // Clear infop if WNOHANG and no child
                if infop != 0 {
                    unsafe {
                        let info_ptr = infop as *mut crate::signal::Siginfo;
                        (*info_ptr).si_pid = 0;
                        (*info_ptr).si_signo = 0;
                    }
                }
                0
            } else {
                errno::ECHILD
            }
        }
        Err(KError::NoChild) => errno::ECHILD,
        Err(KError::WouldBlock) => 0,
        Err(_) => errno::EIO,
    }
}

/// kill - envia um signal para um processo
///
/// pid: PID do processo alvo (ou valores especiais)
/// sig: número do signal a enviar
pub fn sys_kill(pid: i64, sig: i32) -> i64 {
    if sig < 0 {
        return errno::EINVAL;
    }

    match crate::sched::send_signal(pid, sig as u32) {
        Ok(()) => 0,
        Err(KError::NotFound) => errno::ESRCH,
        Err(KError::Invalid) => errno::EINVAL,
        Err(KError::PermissionDenied) => errno::EPERM,
        Err(_) => errno::EIO,
    }
}

// ======================== Socket syscalls ========================

use crate::net::socket::{self as sock, SocketAddr};

/// Errno adicional para socket
const EOPNOTSUPP: i64 = -95;

/// socket - cria um socket
pub fn sys_socket(domain: i32, sock_type: i32, protocol: i32) -> i64 {
    match sock::socket(domain, sock_type, protocol) {
        Ok(sock_id) => {
            // Aloca um FD para o socket
            let mut table = fd_table();
            let table = match table.as_mut() {
                Some(t) => t,
                None => return errno::ENOMEM,
            };
            let fd = table.alloc();
            table.insert(fd, FdEntry {
                fd_type: FdType::Socket { socket_id: sock_id },
            });
            fd as i64
        }
        Err(KError::NotSupported) => errno::EAFNOSUPPORT,
        Err(_) => errno::EIO,
    }
}

/// bind - associa um socket a um endereço
pub fn sys_bind(sockfd: i32, addr: u64, addrlen: u32) -> i64 {
    let table = fd_table();
    let table = match table.as_ref() {
        Some(t) => t,
        None => return errno::EBADF,
    };

    let socket_id = match table.get(sockfd) {
        Some(FdEntry { fd_type: FdType::Socket { socket_id } }) => *socket_id,
        _ => return errno::ENOTSOCK,
    };

    // Check socket domain to determine address type
    let domain = match sock::get_domain(socket_id) {
        Some(d) => d,
        None => return errno::ENOTSOCK,
    };

    match domain {
        sock::SocketDomain::Unix => {
            // Parse sockaddr_un
            let addr_bytes = unsafe { core::slice::from_raw_parts(addr as *const u8, addrlen as usize) };
            let unix_addr = match sock::UnixSocketAddr::from_sockaddr_un(addr_bytes) {
                Some(a) => a,
                None => return errno::EINVAL,
            };

            match sock::unix_bind(socket_id, &unix_addr) {
                Ok(()) => 0,
                Err(KError::AlreadyExists) => errno::EADDRINUSE,
                Err(KError::Invalid) => errno::EINVAL,
                Err(_) => errno::EIO,
            }
        }
        sock::SocketDomain::Inet => {
            // Parse sockaddr_in
            let addr_bytes = unsafe { core::slice::from_raw_parts(addr as *const u8, 16) };
            let sock_addr = match SocketAddr::from_sockaddr_in(addr_bytes) {
                Some(a) => a,
                None => return errno::EINVAL,
            };

            match sock::bind(socket_id, &sock_addr) {
                Ok(()) => 0,
                Err(KError::AlreadyExists) => errno::EADDRINUSE,
                Err(KError::Invalid) => errno::EINVAL,
                Err(_) => errno::EIO,
            }
        }
        _ => errno::EAFNOSUPPORT,
    }
}

/// listen - marca um socket como passivo (servidor)
pub fn sys_listen(sockfd: i32, backlog: i32) -> i64 {
    let table = fd_table();
    let table = match table.as_ref() {
        Some(t) => t,
        None => return errno::EBADF,
    };

    let socket_id = match table.get(sockfd) {
        Some(FdEntry { fd_type: FdType::Socket { socket_id } }) => *socket_id,
        _ => return errno::ENOTSOCK,
    };

    // Check socket domain
    let domain = match sock::get_domain(socket_id) {
        Some(d) => d,
        None => return errno::ENOTSOCK,
    };

    let result = match domain {
        sock::SocketDomain::Unix => sock::unix_listen(socket_id, backlog),
        sock::SocketDomain::Inet => sock::listen(socket_id, backlog),
        _ => return errno::EAFNOSUPPORT,
    };

    match result {
        Ok(()) => 0,
        Err(KError::AlreadyExists) => errno::EADDRINUSE,
        Err(KError::NotSupported) => EOPNOTSUPP,
        Err(_) => errno::EIO,
    }
}

/// accept - aceita uma conexão em um socket
pub fn sys_accept(sockfd: i32, addr: u64, addrlen: u64) -> i64 {
    let socket_id = {
        let table = fd_table();
        let table = match table.as_ref() {
            Some(t) => t,
            None => return errno::EBADF,
        };
        match table.get(sockfd) {
            Some(FdEntry { fd_type: FdType::Socket { socket_id } }) => *socket_id,
            _ => return errno::ENOTSOCK,
        }
    };

    // Check socket domain
    let domain = match sock::get_domain(socket_id) {
        Some(d) => d,
        None => return errno::ENOTSOCK,
    };

    match domain {
        sock::SocketDomain::Unix => {
            match sock::unix_accept(socket_id) {
                Ok((new_sock_id, unix_addr)) => {
                    // Cria novo FD para a conexão
                    let mut table = fd_table();
                    let table = match table.as_mut() {
                        Some(t) => t,
                        None => return errno::ENOMEM,
                    };
                    let new_fd = table.alloc();
                    table.insert(new_fd, FdEntry {
                        fd_type: FdType::Socket { socket_id: new_sock_id },
                    });

                    // Escreve o endereço do peer se fornecido
                    if addr != 0 && addrlen != 0 {
                        let sockaddr_un = unix_addr.to_sockaddr_un();
                        unsafe {
                            let len_ptr = addrlen as *mut u32;
                            let out_len = core::cmp::min(*len_ptr as usize, sockaddr_un.len());
                            core::ptr::copy_nonoverlapping(sockaddr_un.as_ptr(), addr as *mut u8, out_len);
                            *len_ptr = sockaddr_un.len() as u32;
                        }
                    }

                    new_fd as i64
                }
                Err(KError::Invalid) => errno::EINVAL,
                Err(KError::WouldBlock) => errno::EAGAIN,
                Err(_) => errno::EIO,
            }
        }
        sock::SocketDomain::Inet => {
            match sock::accept(socket_id) {
                Ok((new_sock_id, peer_addr)) => {
                    // Cria novo FD para a conexão
                    let mut table = fd_table();
                    let table = match table.as_mut() {
                        Some(t) => t,
                        None => return errno::ENOMEM,
                    };
                    let new_fd = table.alloc();
                    table.insert(new_fd, FdEntry {
                        fd_type: FdType::Socket { socket_id: new_sock_id },
                    });

                    // Escreve o endereço do peer se fornecido
                    if addr != 0 && addrlen != 0 {
                        let sockaddr = peer_addr.to_sockaddr_in();
                        unsafe {
                            let len_ptr = addrlen as *mut u32;
                            let out_len = core::cmp::min(*len_ptr as usize, sockaddr.len());
                            core::ptr::copy_nonoverlapping(sockaddr.as_ptr(), addr as *mut u8, out_len);
                            *len_ptr = 16;
                        }
                    }

                    new_fd as i64
                }
                Err(KError::Invalid) => errno::EINVAL,
                Err(KError::WouldBlock) => errno::EAGAIN,
                Err(_) => errno::EIO,
            }
        }
        _ => errno::EAFNOSUPPORT,
    }
}

/// connect - conecta um socket a um endereço remoto
pub fn sys_connect(sockfd: i32, addr: u64, addrlen: u32) -> i64 {
    let socket_id = {
        let table = fd_table();
        let table = match table.as_ref() {
            Some(t) => t,
            None => return errno::EBADF,
        };
        match table.get(sockfd) {
            Some(FdEntry { fd_type: FdType::Socket { socket_id } }) => *socket_id,
            _ => return errno::ENOTSOCK,
        }
    };

    // Verifica o domínio do socket
    let domain = sock::get_domain(socket_id);
    match domain {
        Some(sock::SocketDomain::Unix) => {
            // Unix domain socket
            let addr_bytes = unsafe { core::slice::from_raw_parts(addr as *const u8, addrlen as usize) };
            let unix_addr = match sock::UnixSocketAddr::from_sockaddr_un(addr_bytes) {
                Some(a) => a,
                None => return errno::EINVAL,
            };

            match sock::unix_connect(socket_id, &unix_addr) {
                Ok(()) => 0,
                Err(KError::NotFound) => errno::ECONNREFUSED,
                Err(KError::WouldBlock) => errno::EAGAIN,
                Err(KError::Invalid) => errno::EINVAL,
                Err(_) => errno::EIO,
            }
        }
        Some(sock::SocketDomain::Inet) => {
            // IPv4 socket
            let addr_bytes = unsafe { core::slice::from_raw_parts(addr as *const u8, 16) };
            let sock_addr = match SocketAddr::from_sockaddr_in(addr_bytes) {
                Some(a) => a,
                None => return errno::EINVAL,
            };

            match sock::connect(socket_id, &sock_addr) {
                Ok(()) => 0,
                Err(KError::NotSupported) => errno::ECONNREFUSED,
                Err(KError::Invalid) => errno::EINVAL,
                Err(_) => errno::EIO,
            }
        }
        _ => errno::EAFNOSUPPORT,
    }
}

/// sendto - envia dados para um endereço específico
pub fn sys_sendto(sockfd: i32, buf: u64, len: usize, _flags: i32, dest_addr: u64, _addrlen: u32) -> i64 {
    let socket_id = {
        let table = fd_table();
        let table = match table.as_ref() {
            Some(t) => t,
            None => return errno::EBADF,
        };
        match table.get(sockfd) {
            Some(FdEntry { fd_type: FdType::Socket { socket_id } }) => *socket_id,
            _ => return errno::ENOTSOCK,
        }
    };

    let data = unsafe { core::slice::from_raw_parts(buf as *const u8, len) };

    // Verifica o domínio do socket
    let domain = sock::get_domain(socket_id);
    match domain {
        Some(sock::SocketDomain::Unix) => {
            // Unix domain socket - ignora dest_addr (já conectado)
            match sock::unix_send(socket_id, data) {
                Ok(n) => n as i64,
                Err(KError::Invalid) => errno::ENOTCONN,
                Err(KError::BrokenPipe) => errno::EPIPE,
                Err(KError::WouldBlock) => errno::EAGAIN,
                Err(_) => errno::EIO,
            }
        }
        Some(sock::SocketDomain::Inet) => {
            if dest_addr != 0 {
                // Com endereço de destino (UDP)
                let addr_bytes = unsafe { core::slice::from_raw_parts(dest_addr as *const u8, 16) };
                let sock_addr = match SocketAddr::from_sockaddr_in(addr_bytes) {
                    Some(a) => a,
                    None => return errno::EINVAL,
                };

                match sock::sendto(socket_id, data, &sock_addr) {
                    Ok(n) => n as i64,
                    Err(KError::NotSupported) => errno::ENETUNREACH,
                    Err(_) => errno::EIO,
                }
            } else {
                // Sem endereço (TCP ou UDP conectado)
                match sock::send(socket_id, data) {
                    Ok(n) => n as i64,
                    Err(KError::Invalid) => errno::ENOTCONN,
                    Err(_) => errno::EIO,
                }
            }
        }
        _ => errno::EAFNOSUPPORT,
    }
}

/// recvfrom - recebe dados e obtém endereço de origem
pub fn sys_recvfrom(sockfd: i32, buf: u64, len: usize, _flags: i32, src_addr: u64, addrlen: u64) -> i64 {
    let socket_id = {
        let table = fd_table();
        let table = match table.as_ref() {
            Some(t) => t,
            None => return errno::EBADF,
        };
        match table.get(sockfd) {
            Some(FdEntry { fd_type: FdType::Socket { socket_id } }) => *socket_id,
            _ => return errno::ENOTSOCK,
        }
    };

    let mut data = vec![0u8; len];

    // Verifica o domínio do socket
    let domain = sock::get_domain(socket_id);
    match domain {
        Some(sock::SocketDomain::Unix) => {
            // Unix domain socket
            match sock::unix_recv(socket_id, &mut data) {
                Ok(n) => {
                    unsafe {
                        core::ptr::copy_nonoverlapping(data.as_ptr(), buf as *mut u8, n);
                        // Para Unix sockets, o endereço de origem não é preenchido (unnamed)
                        if src_addr != 0 && addrlen != 0 {
                            let unix_addr = sock::UnixSocketAddr::unnamed();
                            let sockaddr = unix_addr.to_sockaddr_un();
                            let len_ptr = addrlen as *mut u32;
                            let out_len = core::cmp::min(*len_ptr as usize, sockaddr.len());
                            core::ptr::copy_nonoverlapping(sockaddr.as_ptr(), src_addr as *mut u8, out_len);
                            *len_ptr = 2; // apenas o family
                        }
                    }
                    n as i64
                }
                Err(KError::Invalid) => errno::ENOTCONN,
                Err(KError::WouldBlock) => errno::EAGAIN,
                Err(_) => errno::EIO,
            }
        }
        Some(sock::SocketDomain::Inet) => {
            if src_addr != 0 && addrlen != 0 {
                // Com endereço de origem (UDP)
                match sock::recvfrom(socket_id, &mut data) {
                    Ok((n, peer_addr)) => {
                        unsafe {
                            core::ptr::copy_nonoverlapping(data.as_ptr(), buf as *mut u8, n);
                            let sockaddr = peer_addr.to_sockaddr_in();
                            let len_ptr = addrlen as *mut u32;
                            let out_len = core::cmp::min(*len_ptr as usize, sockaddr.len());
                            core::ptr::copy_nonoverlapping(sockaddr.as_ptr(), src_addr as *mut u8, out_len);
                            *len_ptr = 16;
                        }
                        n as i64
                    }
                    Err(_) => errno::EIO,
                }
            } else {
                // Sem endereço (TCP)
                match sock::recv(socket_id, &mut data) {
                    Ok(n) => {
                        unsafe {
                            core::ptr::copy_nonoverlapping(data.as_ptr(), buf as *mut u8, n);
                        }
                        n as i64
                    }
                    Err(KError::Invalid) => errno::ENOTCONN,
                    Err(_) => errno::EIO,
                }
            }
        }
        _ => errno::EAFNOSUPPORT,
    }
}

/// shutdown - desliga parte de uma conexão
pub fn sys_shutdown(sockfd: i32, how: i32) -> i64 {
    let socket_id = {
        let table = fd_table();
        let table = match table.as_ref() {
            Some(t) => t,
            None => return errno::EBADF,
        };
        match table.get(sockfd) {
            Some(FdEntry { fd_type: FdType::Socket { socket_id } }) => *socket_id,
            _ => return errno::ENOTSOCK,
        }
    };

    match sock::shutdown(socket_id, how) {
        Ok(()) => 0,
        Err(_) => errno::EIO,
    }
}

/// getsockname - obtém o endereço local do socket
pub fn sys_getsockname(sockfd: i32, addr: u64, addrlen: u64) -> i64 {
    let socket_id = {
        let table = fd_table();
        let table = match table.as_ref() {
            Some(t) => t,
            None => return errno::EBADF,
        };
        match table.get(sockfd) {
            Some(FdEntry { fd_type: FdType::Socket { socket_id } }) => *socket_id,
            _ => return errno::ENOTSOCK,
        }
    };

    match sock::getsockname(socket_id) {
        Ok(sock_addr) => {
            let sockaddr = sock_addr.to_sockaddr_in();
            unsafe {
                let len_ptr = addrlen as *mut u32;
                let out_len = core::cmp::min(*len_ptr as usize, sockaddr.len());
                core::ptr::copy_nonoverlapping(sockaddr.as_ptr(), addr as *mut u8, out_len);
                *len_ptr = 16;
            }
            0
        }
        Err(_) => errno::EINVAL,
    }
}

/// getpeername - obtém o endereço remoto do socket
pub fn sys_getpeername(sockfd: i32, addr: u64, addrlen: u64) -> i64 {
    let socket_id = {
        let table = fd_table();
        let table = match table.as_ref() {
            Some(t) => t,
            None => return errno::EBADF,
        };
        match table.get(sockfd) {
            Some(FdEntry { fd_type: FdType::Socket { socket_id } }) => *socket_id,
            _ => return errno::ENOTSOCK,
        }
    };

    match sock::getpeername(socket_id) {
        Ok(sock_addr) => {
            let sockaddr = sock_addr.to_sockaddr_in();
            unsafe {
                let len_ptr = addrlen as *mut u32;
                let out_len = core::cmp::min(*len_ptr as usize, sockaddr.len());
                core::ptr::copy_nonoverlapping(sockaddr.as_ptr(), addr as *mut u8, out_len);
                *len_ptr = 16;
            }
            0
        }
        Err(_) => errno::ENOTCONN,
    }
}

/// setsockopt - configura opções do socket
pub fn sys_setsockopt(sockfd: i32, level: i32, optname: i32, optval: u64, optlen: u32) -> i64 {
    let socket_id = {
        let table = fd_table();
        let table = match table.as_ref() {
            Some(t) => t,
            None => return errno::EBADF,
        };
        match table.get(sockfd) {
            Some(FdEntry { fd_type: FdType::Socket { socket_id } }) => *socket_id,
            _ => return errno::ENOTSOCK,
        }
    };

    let optval_slice = unsafe { core::slice::from_raw_parts(optval as *const u8, optlen as usize) };
    match sock::setsockopt(socket_id, level, optname, optval_slice) {
        Ok(()) => 0,
        Err(_) => errno::EINVAL,
    }
}

/// getsockopt - obtém opções do socket
pub fn sys_getsockopt(sockfd: i32, level: i32, optname: i32, optval: u64, optlen: u64) -> i64 {
    let socket_id = {
        let table = fd_table();
        let table = match table.as_ref() {
            Some(t) => t,
            None => return errno::EBADF,
        };
        match table.get(sockfd) {
            Some(FdEntry { fd_type: FdType::Socket { socket_id } }) => *socket_id,
            _ => return errno::ENOTSOCK,
        }
    };

    unsafe {
        let len_ptr = optlen as *mut u32;
        let buf_len = *len_ptr as usize;
        let mut buf = vec![0u8; buf_len];

        match sock::getsockopt(socket_id, level, optname, &mut buf) {
            Ok(n) => {
                core::ptr::copy_nonoverlapping(buf.as_ptr(), optval as *mut u8, n);
                *len_ptr = n as u32;
                0
            }
            Err(_) => errno::EINVAL,
        }
    }
}

/// execve - substitui o processo atual por um novo programa
///
/// Esta syscall não retorna em caso de sucesso.
pub fn sys_execve(pathname: u64, argv_ptr: u64, envp_ptr: u64) -> i64 {
    use crate::process;

    // Lê o path do executável
    let path = match unsafe { read_user_string(pathname, 4096) } {
        Some(p) => p,
        None => return errno::EFAULT,
    };

    // Lê argv
    let argv_strings = match unsafe { read_user_string_array(argv_ptr, 256) } {
        Some(v) => v,
        None => return errno::EFAULT,
    };

    // Lê envp
    let envp_strings = match unsafe { read_user_string_array(envp_ptr, 256) } {
        Some(v) => v,
        None => return errno::EFAULT,
    };

    // Converte para slices de &str
    let argv: Vec<&str> = argv_strings.iter().map(|s| s.as_str()).collect();
    let envp: Vec<&str> = envp_strings.iter().map(|s| s.as_str()).collect();

    crate::kprintln!(
        "execve: path=\"{}\", argc={}, envc={}",
        path,
        argv.len(),
        envp.len()
    );

    // Check for setuid/setgid bits on the executable
    // Get file metadata first and check execute permission
    let cred = current_cred();
    let vfs = fs::vfs_lock();
    let exec_metadata = match vfs.resolve(&path, &cred) {
        Ok(inode) => {
            let meta = inode.metadata();
            // Check if it's a regular file
            if meta.kind != InodeKind::File {
                drop(vfs);
                return errno::EACCES;
            }
            // Check execute permission
            if !fs::perm::can_exec_file(&meta, &cred) {
                drop(vfs);
                crate::kprintln!("execve: permission denied (no execute): {}", path);
                return errno::EACCES;
            }
            Some(meta)
        }
        Err(KError::NotFound) => {
            drop(vfs);
            crate::kprintln!("execve: arquivo não encontrado: {}", path);
            return errno::ENOENT;
        }
        Err(KError::PermissionDenied) => {
            drop(vfs);
            return errno::EACCES;
        }
        Err(_) => {
            drop(vfs);
            return errno::EIO;
        }
    };
    drop(vfs); // Release VFS lock before loading ELF

    // Carrega o ELF
    let loaded = match process::load_elf_from_path(&path, &argv, &envp) {
        Ok(l) => l,
        Err(KError::NotFound) => {
            crate::kprintln!("execve: arquivo não encontrado: {}", path);
            return errno::ENOENT;
        }
        Err(KError::Invalid) => {
            crate::kprintln!("execve: arquivo inválido: {}", path);
            return errno::ENOEXEC;
        }
        Err(KError::NoMemory) => {
            crate::kprintln!("execve: memória insuficiente");
            return errno::ENOMEM;
        }
        Err(e) => {
            crate::kprintln!("execve: erro: {:?}", e);
            return errno::EIO;
        }
    };

    crate::kprintln!(
        "execve: loaded entry={:#x}, sp={:#x}",
        loaded.entry,
        loaded.stack_pointer
    );

    // Handle setuid/setgid bits
    // When executing a setuid/setgid program, update effective UID/GID
    if let Some(meta) = exec_metadata {
        let mut cred = current_cred();

        // Check for setuid bit
        if meta.mode.is_setuid() {
            crate::kprintln!("execve: setuid binary, setting euid to {}", meta.uid.0);
            cred.euid = crate::security::Uid(meta.uid.0);
            cred.suid = cred.euid; // Save for later restoration
            // Grant capabilities if becoming root
            if cred.euid.0 == 0 {
                cred.caps = crate::security::Caps::ALL;
            }
        } else {
            // On non-setuid exec, clear saved-set-user-ID if not root
            if cred.uid.0 != 0 {
                cred.suid = cred.euid;
            }
        }

        // Check for setgid bit
        if meta.mode.is_setgid() {
            crate::kprintln!("execve: setgid binary, setting egid to {}", meta.gid.0);
            cred.egid = crate::security::Gid(meta.gid.0);
            cred.sgid = cred.egid;
        } else {
            // On non-setgid exec, clear saved-set-group-ID
            cred.sgid = cred.egid;
        }

        crate::task::set_current_cred(cred);
    }

    // Agora precisamos substituir o address space atual e saltar para o novo código
    // Isso é feito através do scheduler
    crate::sched::exec_replace(loaded.cr3, loaded.entry, loaded.stack_pointer);
}

// ==================== Mknod syscall ====================

/// File type constants for mknod (S_IFMT mask = 0o170000)
pub mod filetype {
    pub const S_IFMT: u32 = 0o170000;   // File type mask
    pub const S_IFSOCK: u32 = 0o140000; // Socket
    pub const S_IFLNK: u32 = 0o120000;  // Symbolic link
    pub const S_IFREG: u32 = 0o100000;  // Regular file
    pub const S_IFBLK: u32 = 0o060000;  // Block device
    pub const S_IFDIR: u32 = 0o040000;  // Directory
    pub const S_IFCHR: u32 = 0o020000;  // Character device
    pub const S_IFIFO: u32 = 0o010000;  // FIFO (named pipe)
}

/// Create a special file (FIFO, block device, character device, etc.)
///
/// mode specifies both the file type and permissions:
/// - File type is (mode & S_IFMT)
/// - Permissions are (mode & 0o7777)
///
/// dev specifies device number for block/char devices (ignored for FIFOs)
pub fn sys_mknod(pathname: u64, mode: u32, _dev: u64) -> i64 {
    let path = match unsafe { read_user_string(pathname, 4096) } {
        Some(p) => p,
        None => return errno::EFAULT,
    };

    let cred = current_cred();
    let file_type = mode & filetype::S_IFMT;
    let perms = mode & 0o7777;

    // Only support FIFOs for now
    match file_type {
        filetype::S_IFIFO => {
            // Create a FIFO
            let mut vfs = fs::vfs_lock();
            match vfs.mkfifo(&path, Mode::from_octal(perms as u16), &cred) {
                Ok(_) => 0,
                Err(KError::NotFound) => errno::ENOENT,      // Parent dir not found
                Err(KError::AlreadyExists) => errno::EEXIST, // Already exists
                Err(KError::PermissionDenied) => errno::EACCES,
                Err(KError::NotSupported) => errno::EOPNOTSUPP, // FS doesn't support FIFOs
                Err(_) => errno::EIO,
            }
        }
        filetype::S_IFREG => {
            // Creating regular file via mknod - use write_file with empty data
            let mut vfs = fs::vfs_lock();
            match vfs.write_file(&path, &cred, Mode::from_octal(perms as u16), &[]) {
                Ok(_) => 0,
                Err(KError::NotFound) => errno::ENOENT,
                Err(KError::AlreadyExists) => errno::EEXIST,
                Err(KError::PermissionDenied) => errno::EACCES,
                Err(_) => errno::EIO,
            }
        }
        filetype::S_IFDIR => {
            // Use mkdir instead
            errno::EPERM
        }
        filetype::S_IFBLK | filetype::S_IFCHR => {
            // Block/char devices - only root can create
            if !cred.is_root() {
                return errno::EPERM;
            }
            // Not implemented yet
            errno::EOPNOTSUPP
        }
        filetype::S_IFSOCK => {
            // Unix domain sockets are created via bind(), not mknod
            errno::EOPNOTSUPP
        }
        _ => {
            // Invalid or unknown file type
            errno::EINVAL
        }
    }
}

/// Create a special file relative to a directory fd
pub fn sys_mknodat(dirfd: i32, pathname: u64, mode: u32, dev: u64) -> i64 {
    // If pathname is absolute, ignore dirfd
    let path = match unsafe { read_user_string(pathname, 4096) } {
        Some(p) => p,
        None => return errno::EFAULT,
    };

    if path.starts_with('/') {
        // Absolute path - ignore dirfd
        return sys_mknod(pathname, mode, dev);
    }

    // AT_FDCWD means current working directory
    const AT_FDCWD: i32 = -100;
    if dirfd == AT_FDCWD {
        return sys_mknod(pathname, mode, dev);
    }

    // Relative to dirfd - not fully implemented yet
    // For now, treat as relative to cwd
    sys_mknod(pathname, mode, dev)
}
