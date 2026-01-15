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
    pub const PIPE: u64 = 22;
    pub const DUP: u64 = 32;
    pub const DUP2: u64 = 33;
    pub const GETPID: u64 = 39;
    pub const FORK: u64 = 57;
    pub const EXECVE: u64 = 59;
    pub const EXIT: u64 = 60;
    pub const WAIT4: u64 = 61;
    pub const KILL: u64 = 62;
    pub const UNAME: u64 = 63;
    pub const GETCWD: u64 = 79;
    pub const CHDIR: u64 = 80;
    pub const MKDIR: u64 = 83;
    pub const RMDIR: u64 = 84;
    pub const UNLINK: u64 = 87;
    pub const GETUID: u64 = 102;
    pub const GETGID: u64 = 104;
    pub const GETEUID: u64 = 107;
    pub const GETEGID: u64 = 108;
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
    Dir { inode: Inode },
    /// Lado de leitura de um pipe.
    PipeRead { pipe: Arc<Pipe> },
    /// Lado de escrita de um pipe.
    PipeWrite { pipe: Arc<Pipe> },
    /// Socket de rede.
    Socket { socket_id: u64 },
}

/// File descriptor entry.
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
            // Lê do console (serial).
            if count == 0 {
                return 0;
            }
            let mut read_count = 0usize;
            let out = buf as *mut u8;
            for i in 0..count {
                if let Some(b) = crate::console::read_byte() {
                    unsafe { *out.add(i) = b; }
                    read_count += 1;
                    // Echo e line-based: se for newline, para.
                    if b == b'\n' || b == b'\r' {
                        break;
                    }
                } else {
                    break;
                }
            }
            read_count as i64
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
                let meta = fs::Metadata {
                    uid: cred.uid,
                    gid: cred.gid,
                    mode: Mode::from_octal(mode as u16),
                    kind: InodeKind::File,
                };
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

    // Aloca fd.
    drop(vfs);
    let mut table = fd_table();
    let table = match table.as_mut() {
        Some(t) => t,
        None => return errno::ENOMEM,
    };

    let fd = table.alloc();
    let fd_type = if inode.kind() == InodeKind::Dir {
        FdType::Dir { inode }
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
        FdType::PipeRead { .. } | FdType::PipeWrite { .. } | FdType::Socket { .. } => errno::ESPIPE,
    }
}

pub fn sys_getcwd(buf: u64, size: usize) -> i64 {
    // Por enquanto, sempre retorna "/".
    let cwd = b"/\0";
    if size < cwd.len() {
        return errno::EINVAL;
    }
    unsafe {
        core::ptr::copy_nonoverlapping(cwd.as_ptr(), buf as *mut u8, cwd.len());
    }
    buf as i64
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
    current_cred().uid.0 as i64
}

pub fn sys_getegid() -> i64 {
    current_cred().gid.0 as i64
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

pub fn sys_ioctl(_fd: i32, _request: u64, _arg: u64) -> i64 {
    // Stub: retorna sucesso para ioctls comuns de terminal.
    0
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

/// wait4 - aguarda um processo filho terminar
///
/// pid: -1 para qualquer filho, ou PID específico
/// status: ponteiro para guardar o status de saída
/// options: flags (WNOHANG etc - não implementado ainda)
/// rusage: NULL por enquanto
pub fn sys_wait4(pid: i64, status: u64, _options: i32, _rusage: u64) -> i64 {
    match crate::sched::wait_for_child(pid, status) {
        Ok(child_pid) => child_pid as i64,
        Err(KError::NoChild) => errno::ECHILD,
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
pub fn sys_bind(sockfd: i32, addr: u64, _addrlen: u32) -> i64 {
    let table = fd_table();
    let table = match table.as_ref() {
        Some(t) => t,
        None => return errno::EBADF,
    };

    let socket_id = match table.get(sockfd) {
        Some(FdEntry { fd_type: FdType::Socket { socket_id } }) => *socket_id,
        _ => return errno::ENOTSOCK,
    };

    // Lê o sockaddr_in do userspace
    let addr_bytes = unsafe { core::slice::from_raw_parts(addr as *const u8, 16) };
    let sock_addr = match SocketAddr::from_sockaddr_in(addr_bytes) {
        Some(a) => a,
        None => return errno::EINVAL,
    };

    // Nota: table é uma referência, o lock será liberado ao final da função

    match sock::bind(socket_id, &sock_addr) {
        Ok(()) => 0,
        Err(KError::AlreadyExists) => errno::EADDRINUSE,
        Err(KError::Invalid) => errno::EINVAL,
        Err(_) => errno::EIO,
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

    // Nota: table é uma referência, o lock será liberado ao final da função

    match sock::listen(socket_id, backlog) {
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
        Err(_) => errno::EIO,
    }
}

/// connect - conecta um socket a um endereço remoto
pub fn sys_connect(sockfd: i32, addr: u64, _addrlen: u32) -> i64 {
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

    // Lê o sockaddr_in
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

    // Agora precisamos substituir o address space atual e saltar para o novo código
    // Isso é feito através do scheduler
    crate::sched::exec_replace(loaded.cr3, loaded.entry, loaded.stack_pointer);
}
