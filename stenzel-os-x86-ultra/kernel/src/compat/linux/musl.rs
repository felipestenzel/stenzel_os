//! musl libc Compatibility Layer
//!
//! Provides compatibility with musl libc, a lightweight alternative to glibc.
//! musl is often used for statically-linked binaries and Alpine Linux.
//!
//! Key differences from glibc:
//! - Simpler TLS model
//! - Different __libc_start_main signature
//! - Simpler errno handling
//! - No versioned symbols

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use core::sync::atomic::{AtomicI32, AtomicU64, AtomicUsize, Ordering};
use spin::Mutex;

/// musl version we emulate (1.2.4)
pub const MUSL_VERSION: &str = "1.2.4";
pub const MUSL_VERSION_MAJOR: u32 = 1;
pub const MUSL_VERSION_MINOR: u32 = 2;
pub const MUSL_VERSION_PATCH: u32 = 4;

/// Per-thread errno location for musl
static ERRNO: AtomicI32 = AtomicI32::new(0);

/// Thread-local storage for musl threads
pub struct MuslTls {
    /// Thread ID
    pub tid: u64,
    /// errno for this thread
    pub errno: i32,
    /// Thread-local data pointer
    pub tls_data: usize,
    /// Stack canary
    pub stack_canary: u64,
    /// Locale pointer
    pub locale: usize,
    /// Self pointer (musl uses this for pthread_self)
    pub self_ptr: usize,
    /// Thread descriptor pointer
    pub dtv: usize,
}

impl MuslTls {
    pub const fn new() -> Self {
        MuslTls {
            tid: 0,
            errno: 0,
            tls_data: 0,
            stack_canary: 0x0123456789ABCDEF,
            locale: 0,
            self_ptr: 0,
            dtv: 0,
        }
    }
}

/// Global TLS storage (thread ID -> TLS)
static MUSL_TLS: Mutex<BTreeMap<u64, MuslTls>> = Mutex::new(BTreeMap::new());

/// Initialize musl TLS for a thread
pub fn init_musl_tls(tid: u64) -> usize {
    let mut tls_map = MUSL_TLS.lock();
    let mut tls = MuslTls::new();
    tls.tid = tid;
    tls.self_ptr = &tls as *const _ as usize;

    // Generate stack canary from TSC
    let canary = unsafe {
        let lo: u32;
        let hi: u32;
        core::arch::asm!("rdtsc", out("eax") lo, out("edx") hi);
        ((hi as u64) << 32) | (lo as u64)
    };
    tls.stack_canary = canary;

    let self_ptr = tls.self_ptr;
    tls_map.insert(tid, tls);
    self_ptr
}

/// Get musl TLS for current thread
pub fn get_musl_tls(tid: u64) -> Option<usize> {
    let tls_map = MUSL_TLS.lock();
    tls_map.get(&tid).map(|t| t.self_ptr)
}

/// Clean up musl TLS for a thread
pub fn cleanup_musl_tls(tid: u64) {
    let mut tls_map = MUSL_TLS.lock();
    tls_map.remove(&tid);
}

// ============================================================================
// musl __libc_start_main
// ============================================================================

/// musl's __libc_start_main is simpler than glibc's
///
/// Signature: int __libc_start_main(
///     int (*main)(int, char**, char**),
///     int argc,
///     char** argv
/// )
///
/// Note: musl doesn't have the extra init/fini/rtld_fini parameters
pub fn musl_libc_start_main(
    main_fn: usize,
    argc: i32,
    argv: usize,
) -> i32 {
    crate::kprintln!("musl: __libc_start_main(main={:#x}, argc={}, argv={:#x})",
                     main_fn, argc, argv);

    // Initialize musl-specific runtime
    init_musl_runtime();

    // Call main
    // In real execution, this would jump to main_fn
    // For now, we return 0 to indicate success
    0
}

/// Alternative entry point used by some musl binaries
pub fn musl_start_c(p: usize) -> ! {
    crate::kprintln!("musl: __start_c called with p={:#x}", p);

    // p points to argc on the stack
    // Stack layout: argc, argv[0], argv[1], ..., NULL, envp[0], ...

    init_musl_runtime();

    // This should never return
    loop {
        core::hint::spin_loop();
    }
}

/// Initialize musl runtime
fn init_musl_runtime() {
    crate::kprintln!("musl: initializing runtime v{}", MUSL_VERSION);

    // Initialize errno
    ERRNO.store(0, Ordering::SeqCst);

    // Set up locale to "C"
    // musl defaults to C locale
}

// ============================================================================
// musl errno handling
// ============================================================================

/// musl's __errno_location
/// Returns pointer to thread-local errno
pub fn musl_errno_location() -> *mut i32 {
    // For simplicity, use the global errno
    // In a full implementation, this would be per-thread
    &ERRNO as *const AtomicI32 as *mut i32
}

/// Set errno
pub fn set_errno(val: i32) {
    ERRNO.store(val, Ordering::SeqCst);
}

/// Get errno
pub fn get_errno() -> i32 {
    ERRNO.load(Ordering::SeqCst)
}

// musl errno constants (same as Linux)
pub mod errno {
    pub const EPERM: i32 = 1;
    pub const ENOENT: i32 = 2;
    pub const ESRCH: i32 = 3;
    pub const EINTR: i32 = 4;
    pub const EIO: i32 = 5;
    pub const ENXIO: i32 = 6;
    pub const E2BIG: i32 = 7;
    pub const ENOEXEC: i32 = 8;
    pub const EBADF: i32 = 9;
    pub const ECHILD: i32 = 10;
    pub const EAGAIN: i32 = 11;
    pub const ENOMEM: i32 = 12;
    pub const EACCES: i32 = 13;
    pub const EFAULT: i32 = 14;
    pub const ENOTBLK: i32 = 15;
    pub const EBUSY: i32 = 16;
    pub const EEXIST: i32 = 17;
    pub const EXDEV: i32 = 18;
    pub const ENODEV: i32 = 19;
    pub const ENOTDIR: i32 = 20;
    pub const EISDIR: i32 = 21;
    pub const EINVAL: i32 = 22;
    pub const ENFILE: i32 = 23;
    pub const EMFILE: i32 = 24;
    pub const ENOTTY: i32 = 25;
    pub const ETXTBSY: i32 = 26;
    pub const EFBIG: i32 = 27;
    pub const ENOSPC: i32 = 28;
    pub const ESPIPE: i32 = 29;
    pub const EROFS: i32 = 30;
    pub const EMLINK: i32 = 31;
    pub const EPIPE: i32 = 32;
    pub const EDOM: i32 = 33;
    pub const ERANGE: i32 = 34;
    pub const EDEADLK: i32 = 35;
    pub const ENAMETOOLONG: i32 = 36;
    pub const ENOLCK: i32 = 37;
    pub const ENOSYS: i32 = 38;
    pub const ENOTEMPTY: i32 = 39;
    pub const ELOOP: i32 = 40;
    pub const EWOULDBLOCK: i32 = EAGAIN;
    pub const ENOMSG: i32 = 42;
    pub const EIDRM: i32 = 43;
    pub const ENOSTR: i32 = 60;
    pub const ENODATA: i32 = 61;
    pub const ETIME: i32 = 62;
    pub const ENOSR: i32 = 63;
    pub const ENOLINK: i32 = 67;
    pub const EPROTO: i32 = 71;
    pub const EBADMSG: i32 = 74;
    pub const EOVERFLOW: i32 = 75;
    pub const EILSEQ: i32 = 84;
    pub const ENOTSOCK: i32 = 88;
    pub const EDESTADDRREQ: i32 = 89;
    pub const EMSGSIZE: i32 = 90;
    pub const EPROTOTYPE: i32 = 91;
    pub const ENOPROTOOPT: i32 = 92;
    pub const EPROTONOSUPPORT: i32 = 93;
    pub const ESOCKTNOSUPPORT: i32 = 94;
    pub const EOPNOTSUPP: i32 = 95;
    pub const ENOTSUP: i32 = EOPNOTSUPP;
    pub const EPFNOSUPPORT: i32 = 96;
    pub const EAFNOSUPPORT: i32 = 97;
    pub const EADDRINUSE: i32 = 98;
    pub const EADDRNOTAVAIL: i32 = 99;
    pub const ENETDOWN: i32 = 100;
    pub const ENETUNREACH: i32 = 101;
    pub const ENETRESET: i32 = 102;
    pub const ECONNABORTED: i32 = 103;
    pub const ECONNRESET: i32 = 104;
    pub const ENOBUFS: i32 = 105;
    pub const EISCONN: i32 = 106;
    pub const ENOTCONN: i32 = 107;
    pub const ESHUTDOWN: i32 = 108;
    pub const ETIMEDOUT: i32 = 110;
    pub const ECONNREFUSED: i32 = 111;
    pub const EHOSTDOWN: i32 = 112;
    pub const EHOSTUNREACH: i32 = 113;
    pub const EALREADY: i32 = 114;
    pub const EINPROGRESS: i32 = 115;
    pub const ESTALE: i32 = 116;
    pub const EDQUOT: i32 = 122;
    pub const ECANCELED: i32 = 125;
    pub const EOWNERDEAD: i32 = 130;
    pub const ENOTRECOVERABLE: i32 = 131;
}

// ============================================================================
// musl pthread implementation
// ============================================================================

/// musl pthread_t is just a pointer to the thread structure
pub type MuslPthread = usize;

/// musl pthread structure (simplified)
#[repr(C)]
pub struct MuslPthreadStruct {
    pub self_ptr: usize,
    pub dtv: usize,
    pub prev: usize,
    pub next: usize,
    pub sysinfo: usize,
    pub canary: u64,
    pub tid: i32,
    pub errno_val: i32,
    pub detach_state: i32,
    pub cancel: i32,
    pub canceldisable: u8,
    pub cancelasync: u8,
    pub tsd_used: u8,
    pub dlerror_flag: u8,
    pub map_base: usize,
    pub map_size: usize,
    pub stack: usize,
    pub stack_size: usize,
    pub guard_size: usize,
    pub result: usize,
    pub cancelbuf: usize,
    pub tsd: [usize; 128],
    pub locale: usize,
    pub killlock: [i32; 1],
    pub dlerror_buf: usize,
    pub stdio_locks: usize,
}

/// pthread_self for musl
pub fn musl_pthread_self() -> MuslPthread {
    let tid = crate::sched::current_tid();
    get_musl_tls(tid).unwrap_or(0)
}

/// pthread_create for musl
pub fn musl_pthread_create(
    thread: *mut MuslPthread,
    _attr: usize,
    start_routine: usize,
    arg: usize,
) -> i32 {
    crate::kprintln!("musl: pthread_create(start={:#x}, arg={:#x})", start_routine, arg);

    // Create a new thread
    // For now, return EAGAIN to indicate we can't create threads
    // In a full implementation, this would use clone()

    if !thread.is_null() {
        unsafe { *thread = 0; }
    }

    errno::EAGAIN
}

/// pthread_join for musl
pub fn musl_pthread_join(thread: MuslPthread, retval: *mut usize) -> i32 {
    crate::kprintln!("musl: pthread_join(thread={:#x})", thread);

    if !retval.is_null() {
        unsafe { *retval = 0; }
    }

    0
}

/// pthread_exit for musl
pub fn musl_pthread_exit(retval: usize) -> ! {
    crate::kprintln!("musl: pthread_exit(retval={:#x})", retval);

    // Clean up TLS
    let tid = crate::sched::current_tid();
    cleanup_musl_tls(tid);

    // Exit the thread
    crate::syscall::sys_exit(0)
}

/// pthread_mutex_init for musl
pub fn musl_pthread_mutex_init(mutex: *mut i32, _attr: usize) -> i32 {
    if !mutex.is_null() {
        unsafe { *mutex = 0; }
    }
    0
}

/// pthread_mutex_lock for musl
pub fn musl_pthread_mutex_lock(mutex: *mut i32) -> i32 {
    if mutex.is_null() {
        return errno::EINVAL;
    }

    // Simple spinlock implementation using AtomicI32
    unsafe {
        let atomic = &*(mutex as *const AtomicI32);
        while atomic.compare_exchange_weak(0, 1, Ordering::Acquire, Ordering::Relaxed).is_err() {
            core::hint::spin_loop();
        }
    }
    0
}

/// pthread_mutex_trylock for musl
pub fn musl_pthread_mutex_trylock(mutex: *mut i32) -> i32 {
    if mutex.is_null() {
        return errno::EINVAL;
    }

    unsafe {
        let atomic = &*(mutex as *const AtomicI32);
        if atomic.compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed).is_ok() {
            0
        } else {
            errno::EBUSY
        }
    }
}

/// pthread_mutex_unlock for musl
pub fn musl_pthread_mutex_unlock(mutex: *mut i32) -> i32 {
    if mutex.is_null() {
        return errno::EINVAL;
    }

    unsafe {
        let atomic = &*(mutex as *const AtomicI32);
        atomic.store(0, Ordering::Release);
    }
    0
}

/// pthread_mutex_destroy for musl
pub fn musl_pthread_mutex_destroy(_mutex: *mut i32) -> i32 {
    0
}

/// pthread_once for musl
static ONCE_LOCK: AtomicI32 = AtomicI32::new(0);

pub fn musl_pthread_once(once_control: *mut i32, init_routine: fn()) -> i32 {
    if once_control.is_null() {
        return errno::EINVAL;
    }

    unsafe {
        let state = core::ptr::read_volatile(once_control);
        if state == 0 {
            // Try to acquire
            if ONCE_LOCK.compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
                if core::ptr::read_volatile(once_control) == 0 {
                    init_routine();
                    core::ptr::write_volatile(once_control, 1);
                }
                ONCE_LOCK.store(0, Ordering::SeqCst);
            }
        }
    }

    0
}

// ============================================================================
// musl thread-specific data (TSD)
// ============================================================================

/// Maximum number of TSD keys
const MUSL_PTHREAD_KEYS_MAX: usize = 128;

/// TSD key destructors
static TSD_DESTRUCTORS: Mutex<[Option<fn(usize)>; MUSL_PTHREAD_KEYS_MAX]> =
    Mutex::new([None; MUSL_PTHREAD_KEYS_MAX]);

/// Next available key
static NEXT_TSD_KEY: AtomicUsize = AtomicUsize::new(0);

/// pthread_key_create for musl
pub fn musl_pthread_key_create(key: *mut usize, destructor: Option<fn(usize)>) -> i32 {
    let next = NEXT_TSD_KEY.fetch_add(1, Ordering::SeqCst);

    if next >= MUSL_PTHREAD_KEYS_MAX {
        NEXT_TSD_KEY.fetch_sub(1, Ordering::SeqCst);
        return errno::EAGAIN;
    }

    if !key.is_null() {
        unsafe { *key = next; }
    }

    if let Some(dtor) = destructor {
        let mut dtors = TSD_DESTRUCTORS.lock();
        dtors[next] = Some(dtor);
    }

    0
}

/// pthread_key_delete for musl
pub fn musl_pthread_key_delete(key: usize) -> i32 {
    if key >= MUSL_PTHREAD_KEYS_MAX {
        return errno::EINVAL;
    }

    let mut dtors = TSD_DESTRUCTORS.lock();
    dtors[key] = None;

    0
}

/// pthread_getspecific for musl
pub fn musl_pthread_getspecific(key: usize) -> usize {
    if key >= MUSL_PTHREAD_KEYS_MAX {
        return 0;
    }

    // In a full implementation, this would access the thread's TSD array
    0
}

/// pthread_setspecific for musl
pub fn musl_pthread_setspecific(key: usize, value: usize) -> i32 {
    if key >= MUSL_PTHREAD_KEYS_MAX {
        return errno::EINVAL;
    }

    // In a full implementation, this would set the thread's TSD[key] = value
    let _ = value;
    0
}

// ============================================================================
// musl condition variables
// ============================================================================

/// pthread_cond_init for musl
pub fn musl_pthread_cond_init(cond: *mut i32, _attr: usize) -> i32 {
    if !cond.is_null() {
        unsafe { *cond = 0; }
    }
    0
}

/// pthread_cond_destroy for musl
pub fn musl_pthread_cond_destroy(_cond: *mut i32) -> i32 {
    0
}

/// pthread_cond_wait for musl
pub fn musl_pthread_cond_wait(cond: *mut i32, mutex: *mut i32) -> i32 {
    if cond.is_null() || mutex.is_null() {
        return errno::EINVAL;
    }

    // Unlock mutex, wait, relock
    musl_pthread_mutex_unlock(mutex);

    // Wait for signal (in real implementation, use futex)
    let mut spins = 0;
    unsafe {
        while core::ptr::read_volatile(cond) == 0 && spins < 1000000 {
            core::hint::spin_loop();
            spins += 1;
        }
    }

    musl_pthread_mutex_lock(mutex);
    0
}

/// pthread_cond_signal for musl
pub fn musl_pthread_cond_signal(cond: *mut i32) -> i32 {
    if cond.is_null() {
        return errno::EINVAL;
    }

    unsafe {
        core::ptr::write_volatile(cond, 1);
    }

    0
}

/// pthread_cond_broadcast for musl
pub fn musl_pthread_cond_broadcast(cond: *mut i32) -> i32 {
    musl_pthread_cond_signal(cond)
}

// ============================================================================
// musl malloc implementation stubs
// ============================================================================

/// musl uses a simple malloc based on mmap/brk
/// We provide stubs that use the kernel heap

pub fn musl_malloc(size: usize) -> usize {
    if size == 0 {
        return 0;
    }

    // Use kernel allocator
    let layout = core::alloc::Layout::from_size_align(size, 8).unwrap();
    unsafe {
        let ptr = alloc::alloc::alloc(layout);
        ptr as usize
    }
}

pub fn musl_free(ptr: usize) {
    if ptr == 0 {
        return;
    }

    // We can't free without knowing the size
    // In a full implementation, we'd track allocations
    let _ = ptr;
}

pub fn musl_calloc(nmemb: usize, size: usize) -> usize {
    let total = nmemb.saturating_mul(size);
    if total == 0 {
        return 0;
    }

    let layout = core::alloc::Layout::from_size_align(total, 8).unwrap();
    unsafe {
        let ptr = alloc::alloc::alloc_zeroed(layout);
        ptr as usize
    }
}

pub fn musl_realloc(ptr: usize, size: usize) -> usize {
    if ptr == 0 {
        return musl_malloc(size);
    }

    if size == 0 {
        musl_free(ptr);
        return 0;
    }

    // Simple implementation: allocate new, copy, free old
    let new_ptr = musl_malloc(size);
    if new_ptr != 0 {
        // Copy old data (we don't know the old size, so copy size bytes)
        unsafe {
            core::ptr::copy_nonoverlapping(ptr as *const u8, new_ptr as *mut u8, size);
        }
        musl_free(ptr);
    }
    new_ptr
}

// ============================================================================
// musl string functions
// ============================================================================

pub fn musl_strlen(s: *const u8) -> usize {
    if s.is_null() {
        return 0;
    }

    let mut len = 0;
    unsafe {
        while *s.add(len) != 0 {
            len += 1;
        }
    }
    len
}

pub fn musl_strcpy(dest: *mut u8, src: *const u8) -> *mut u8 {
    if dest.is_null() || src.is_null() {
        return dest;
    }

    let mut i = 0;
    unsafe {
        loop {
            let c = *src.add(i);
            *dest.add(i) = c;
            if c == 0 {
                break;
            }
            i += 1;
        }
    }
    dest
}

pub fn musl_strcmp(s1: *const u8, s2: *const u8) -> i32 {
    if s1.is_null() || s2.is_null() {
        return 0;
    }

    let mut i = 0;
    unsafe {
        loop {
            let c1 = *s1.add(i);
            let c2 = *s2.add(i);
            if c1 != c2 || c1 == 0 {
                return (c1 as i32) - (c2 as i32);
            }
            i += 1;
        }
    }
}

pub fn musl_memset(dest: *mut u8, c: i32, n: usize) -> *mut u8 {
    if dest.is_null() {
        return dest;
    }

    unsafe {
        for i in 0..n {
            *dest.add(i) = c as u8;
        }
    }
    dest
}

pub fn musl_memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if dest.is_null() || src.is_null() {
        return dest;
    }

    unsafe {
        core::ptr::copy_nonoverlapping(src, dest, n);
    }
    dest
}

pub fn musl_memmove(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if dest.is_null() || src.is_null() {
        return dest;
    }

    unsafe {
        core::ptr::copy(src, dest, n);
    }
    dest
}

// ============================================================================
// musl-specific syscall wrappers
// ============================================================================

/// musl's syscall wrapper
/// musl uses inline syscalls extensively
pub fn musl_syscall(num: i64, a1: i64, a2: i64, a3: i64, a4: i64, a5: i64, a6: i64) -> i64 {
    // Route syscalls to our implementations
    // This is a simplified dispatcher for common syscalls
    match num {
        0 => crate::syscall::sys_read(a1 as i32, a2 as u64, a3 as usize),
        1 => crate::syscall::sys_write(a1 as i32, a2 as u64, a3 as usize),
        2 => crate::syscall::sys_open(a1 as u64, a2 as u32, a3 as u32),
        3 => crate::syscall::sys_close(a1 as i32),
        9 => crate::syscall::sys_mmap(a1 as u64, a2 as usize, a3 as i32, a4 as i32, a5 as i32, a6 as i64),
        11 => crate::syscall::sys_munmap(a1 as u64, a2 as usize),
        12 => crate::syscall::sys_brk(a1 as u64),
        39 => crate::syscall::sys_getpid() as i64,
        60 => { crate::syscall::sys_exit(a1 as u64); }
        62 => crate::syscall::sys_kill(a1 as i64, a2 as i32),
        _ => {
            crate::kprintln!("musl: unhandled syscall {}", num);
            -errno::ENOSYS as i64
        }
    }
}

// ============================================================================
// musl locale support
// ============================================================================

/// musl locale structure
#[repr(C)]
pub struct MuslLocale {
    pub cat: [usize; 6],
}

// Safety: MuslLocale only contains usize values
unsafe impl Sync for MuslLocale {}
unsafe impl Send for MuslLocale {}

/// Default C locale
static C_LOCALE: MuslLocale = MuslLocale {
    cat: [0; 6],
};

/// Get C locale
pub fn musl_get_c_locale() -> *const MuslLocale {
    &C_LOCALE
}

/// uselocale for musl
pub fn musl_uselocale(loc: *const MuslLocale) -> *const MuslLocale {
    // musl always returns the current locale and sets the new one
    let _ = loc;
    &C_LOCALE
}

/// setlocale for musl
pub fn musl_setlocale(category: i32, locale: *const u8) -> *const u8 {
    let _ = category;
    let _ = locale;
    // Always return "C"
    b"C\0".as_ptr()
}

// ============================================================================
// musl environment
// ============================================================================

/// environ pointer for musl
static mut ENVIRON: *mut *mut u8 = core::ptr::null_mut();

/// Get environ
pub fn musl_get_environ() -> *mut *mut u8 {
    unsafe { ENVIRON }
}

/// Set environ
pub fn musl_set_environ(env: *mut *mut u8) {
    unsafe { ENVIRON = env; }
}

/// getenv for musl
pub fn musl_getenv(name: *const u8) -> *const u8 {
    if name.is_null() {
        return core::ptr::null();
    }

    let name_len = musl_strlen(name);
    if name_len == 0 {
        return core::ptr::null();
    }

    unsafe {
        let mut env = ENVIRON;
        if env.is_null() {
            return core::ptr::null();
        }

        while !(*env).is_null() {
            let entry = *env;

            // Check if entry starts with name=
            let mut matches = true;
            for i in 0..name_len {
                if *entry.add(i) != *name.add(i) {
                    matches = false;
                    break;
                }
            }

            if matches && *entry.add(name_len) == b'=' {
                return entry.add(name_len + 1);
            }

            env = env.add(1);
        }
    }

    core::ptr::null()
}

// ============================================================================
// musl auxiliary vector (auxv)
// ============================================================================

/// Auxiliary vector entry types
pub mod at {
    pub const NULL: u64 = 0;
    pub const IGNORE: u64 = 1;
    pub const EXECFD: u64 = 2;
    pub const PHDR: u64 = 3;
    pub const PHENT: u64 = 4;
    pub const PHNUM: u64 = 5;
    pub const PAGESZ: u64 = 6;
    pub const BASE: u64 = 7;
    pub const FLAGS: u64 = 8;
    pub const ENTRY: u64 = 9;
    pub const NOTELF: u64 = 10;
    pub const UID: u64 = 11;
    pub const EUID: u64 = 12;
    pub const GID: u64 = 13;
    pub const EGID: u64 = 14;
    pub const PLATFORM: u64 = 15;
    pub const HWCAP: u64 = 16;
    pub const CLKTCK: u64 = 17;
    pub const SECURE: u64 = 23;
    pub const BASE_PLATFORM: u64 = 24;
    pub const RANDOM: u64 = 25;
    pub const HWCAP2: u64 = 26;
    pub const EXECFN: u64 = 31;
    pub const SYSINFO_EHDR: u64 = 33;
}

/// getauxval for musl
pub fn musl_getauxval(type_: u64) -> u64 {
    match type_ {
        at::PAGESZ => 4096,
        at::CLKTCK => 100, // HZ
        at::UID | at::EUID => 0, // root
        at::GID | at::EGID => 0, // root
        at::SECURE => 0,
        at::HWCAP => {
            // x86_64 capabilities
            0
        }
        at::HWCAP2 => 0,
        _ => 0,
    }
}

// ============================================================================
// musl exit handling
// ============================================================================

/// Exit function list
static EXIT_FUNCS: Mutex<Vec<fn()>> = Mutex::new(Vec::new());

/// atexit for musl
pub fn musl_atexit(func: fn()) -> i32 {
    let mut funcs = EXIT_FUNCS.lock();
    funcs.push(func);
    0
}

/// __cxa_atexit for musl (C++ compatibility)
pub fn musl_cxa_atexit(func: fn(usize), arg: usize, dso: usize) -> i32 {
    let _ = (func, arg, dso);
    // Simplified: just register without arg/dso tracking
    0
}

/// Run exit functions
pub fn musl_run_exit_funcs() {
    let funcs = EXIT_FUNCS.lock();
    for func in funcs.iter().rev() {
        func();
    }
}

/// exit for musl
pub fn musl_exit(status: i32) -> ! {
    musl_run_exit_funcs();
    crate::syscall::sys_exit(status as u64)
}

/// _Exit for musl (no cleanup)
pub fn musl_quick_exit(status: i32) -> ! {
    crate::syscall::sys_exit(status as u64)
}

// ============================================================================
// Symbol table for musl binaries
// ============================================================================

/// Get symbol address for musl
pub fn get_musl_symbol(name: &str) -> Option<usize> {
    match name {
        "__libc_start_main" => Some(musl_libc_start_main as usize),
        "__errno_location" => Some(musl_errno_location as usize),
        "pthread_self" => Some(musl_pthread_self as usize),
        "pthread_create" => Some(musl_pthread_create as usize),
        "pthread_join" => Some(musl_pthread_join as usize),
        "pthread_exit" => Some(musl_pthread_exit as usize),
        "pthread_mutex_init" => Some(musl_pthread_mutex_init as usize),
        "pthread_mutex_lock" => Some(musl_pthread_mutex_lock as usize),
        "pthread_mutex_trylock" => Some(musl_pthread_mutex_trylock as usize),
        "pthread_mutex_unlock" => Some(musl_pthread_mutex_unlock as usize),
        "pthread_mutex_destroy" => Some(musl_pthread_mutex_destroy as usize),
        "pthread_once" => Some(musl_pthread_once as usize),
        "pthread_key_create" => Some(musl_pthread_key_create as usize),
        "pthread_key_delete" => Some(musl_pthread_key_delete as usize),
        "pthread_getspecific" => Some(musl_pthread_getspecific as usize),
        "pthread_setspecific" => Some(musl_pthread_setspecific as usize),
        "pthread_cond_init" => Some(musl_pthread_cond_init as usize),
        "pthread_cond_destroy" => Some(musl_pthread_cond_destroy as usize),
        "pthread_cond_wait" => Some(musl_pthread_cond_wait as usize),
        "pthread_cond_signal" => Some(musl_pthread_cond_signal as usize),
        "pthread_cond_broadcast" => Some(musl_pthread_cond_broadcast as usize),
        "malloc" => Some(musl_malloc as usize),
        "free" => Some(musl_free as usize),
        "calloc" => Some(musl_calloc as usize),
        "realloc" => Some(musl_realloc as usize),
        "strlen" => Some(musl_strlen as usize),
        "strcpy" => Some(musl_strcpy as usize),
        "strcmp" => Some(musl_strcmp as usize),
        "memset" => Some(musl_memset as usize),
        "memcpy" => Some(musl_memcpy as usize),
        "memmove" => Some(musl_memmove as usize),
        "setlocale" => Some(musl_setlocale as usize),
        "uselocale" => Some(musl_uselocale as usize),
        "__c_locale" => Some(musl_get_c_locale as usize),
        "getenv" => Some(musl_getenv as usize),
        "environ" => Some(musl_get_environ as usize),
        "getauxval" => Some(musl_getauxval as usize),
        "atexit" => Some(musl_atexit as usize),
        "__cxa_atexit" => Some(musl_cxa_atexit as usize),
        "exit" => Some(musl_exit as usize),
        "_Exit" => Some(musl_quick_exit as usize),
        "_exit" => Some(musl_quick_exit as usize),
        "syscall" => Some(musl_syscall as usize),
        _ => None,
    }
}

/// Check if a symbol is musl-specific
pub fn is_musl_symbol(name: &str) -> bool {
    get_musl_symbol(name).is_some()
}
