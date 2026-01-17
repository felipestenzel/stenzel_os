//! glibc Compatibility Stubs
//!
//! Provides basic glibc function implementations and stubs
//! for running Linux binaries that depend on glibc.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

/// __libc_start_main prototype
/// int __libc_start_main(
///     int (*main)(int, char**, char**),
///     int argc,
///     char** argv,
///     int (*init)(void),
///     void (*fini)(void),
///     void (*rtld_fini)(void),
///     void* stack_end
/// );
#[derive(Debug)]
pub struct LibcStartMainArgs {
    pub main_fn: u64,
    pub argc: i32,
    pub argv: u64,
    pub init_fn: u64,
    pub fini_fn: u64,
    pub rtld_fini_fn: u64,
    pub stack_end: u64,
}

/// __libc_csu_init/fini stubs
pub struct CsuFunctions {
    pub init: u64,
    pub fini: u64,
}

/// glibc version info
#[derive(Debug, Clone)]
pub struct GlibcVersion {
    pub major: u32,
    pub minor: u32,
    pub release: u32,
}

impl GlibcVersion {
    pub const fn new(major: u32, minor: u32, release: u32) -> Self {
        Self { major, minor, release }
    }

    /// Our emulated glibc version
    pub const fn emulated() -> Self {
        Self::new(2, 31, 0)
    }

    pub fn as_string(&self) -> String {
        alloc::format!("{}.{}.{}", self.major, self.minor, self.release)
    }
}

/// __cxa_atexit registration
static mut ATEXIT_HANDLERS: Vec<AtexitHandler> = Vec::new();

#[derive(Clone)]
struct AtexitHandler {
    func: u64,
    arg: u64,
    dso_handle: u64,
}

/// Register an atexit handler
pub fn cxa_atexit(func: u64, arg: u64, dso_handle: u64) -> i32 {
    unsafe {
        ATEXIT_HANDLERS.push(AtexitHandler { func, arg, dso_handle });
    }
    0
}

/// Run atexit handlers for a DSO
pub fn cxa_finalize(dso_handle: u64) {
    unsafe {
        let handlers: Vec<_> = ATEXIT_HANDLERS.iter()
            .filter(|h| dso_handle == 0 || h.dso_handle == dso_handle)
            .cloned()
            .collect();

        for handler in handlers.iter().rev() {
            // In a real implementation, we would call the handler function
            crate::kprintln!("glibc: would call atexit handler at {:#x}", handler.func);
        }

        if dso_handle != 0 {
            ATEXIT_HANDLERS.retain(|h| h.dso_handle != dso_handle);
        } else {
            ATEXIT_HANDLERS.clear();
        }
    }
}

/// Get environment pointer
pub fn environ() -> u64 {
    // Would return pointer to environment array
    0
}

/// __errno_location stub
/// Returns a pointer to the thread-local errno variable
pub fn errno_location() -> u64 {
    // In a real implementation, this would be TLS
    static mut ERRNO: i32 = 0;
    unsafe { &mut ERRNO as *mut i32 as u64 }
}

/// Error numbers
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
}

/// Stack protector support
static mut STACK_CHK_GUARD: u64 = 0;

/// Get stack canary value
pub fn stack_chk_guard() -> u64 {
    unsafe {
        if STACK_CHK_GUARD == 0 {
            // Generate a random canary
            STACK_CHK_GUARD = 0x00000aff0aff0aff; // Placeholder
        }
        STACK_CHK_GUARD
    }
}

/// Stack check failure handler
pub fn stack_chk_fail() -> ! {
    crate::kprintln!("*** stack smashing detected ***");
    // Should abort the process
    loop {}
}

/// Pointer guard for function pointers
static mut POINTER_CHK_GUARD: u64 = 0;

pub fn pointer_chk_guard() -> u64 {
    unsafe {
        if POINTER_CHK_GUARD == 0 {
            POINTER_CHK_GUARD = 0x00000aff0aff0aff; // Placeholder
        }
        POINTER_CHK_GUARD
    }
}

/// pthread stubs for single-threaded emulation
pub mod pthread {
    /// Mutex type
    #[repr(C)]
    pub struct PthreadMutex {
        pub lock: i32,
        pub count: u32,
        pub owner: u64,
        pub kind: i32,
    }

    /// Mutex attributes
    #[repr(C)]
    pub struct PthreadMutexattr {
        pub kind: i32,
    }

    /// Initialize a mutex
    pub fn mutex_init(_mutex: *mut PthreadMutex, _attr: *const PthreadMutexattr) -> i32 {
        0
    }

    /// Lock a mutex
    pub fn mutex_lock(_mutex: *mut PthreadMutex) -> i32 {
        0 // Always succeed in single-threaded
    }

    /// Unlock a mutex
    pub fn mutex_unlock(_mutex: *mut PthreadMutex) -> i32 {
        0
    }

    /// Destroy a mutex
    pub fn mutex_destroy(_mutex: *mut PthreadMutex) -> i32 {
        0
    }

    /// Get thread ID
    pub fn self_() -> u64 {
        // Return current task ID
        crate::sched::current_tid()
    }

    /// pthread_once
    pub fn once(_control: *mut i32, _init_routine: extern "C" fn()) -> i32 {
        // In single-threaded mode, just call if not already done
        0
    }

    /// Create thread-specific data key
    pub fn key_create(_key: *mut u32, _destructor: extern "C" fn(*mut u8)) -> i32 {
        0
    }

    /// Get thread-specific data
    pub fn getspecific(_key: u32) -> *mut u8 {
        core::ptr::null_mut()
    }

    /// Set thread-specific data
    pub fn setspecific(_key: u32, _value: *const u8) -> i32 {
        0
    }
}

/// Locale stubs
pub mod locale {
    use alloc::string::String;

    /// Current locale
    static mut CURRENT_LOCALE: Option<String> = None;

    /// Set locale
    pub fn setlocale(_category: i32, locale: Option<&str>) -> Option<String> {
        match locale {
            Some(l) => {
                unsafe { CURRENT_LOCALE = Some(String::from(l)); }
                unsafe { CURRENT_LOCALE.clone() }
            }
            None => {
                unsafe { CURRENT_LOCALE.clone() }
            }
        }
    }

    /// Locale categories
    pub const LC_ALL: i32 = 6;
    pub const LC_COLLATE: i32 = 3;
    pub const LC_CTYPE: i32 = 0;
    pub const LC_MESSAGES: i32 = 5;
    pub const LC_MONETARY: i32 = 4;
    pub const LC_NUMERIC: i32 = 1;
    pub const LC_TIME: i32 = 2;
}

/// getauxval stub
pub fn getauxval(type_: u64) -> u64 {
    // Auxiliary vector values
    match type_ {
        6 => 4096,   // AT_PAGESZ
        25 => 0,     // AT_RANDOM (would be pointer to 16 random bytes)
        _ => 0,
    }
}

/// sysconf stub
pub fn sysconf(name: i32) -> i64 {
    // System configuration values
    match name {
        30 => 4096,      // _SC_PAGE_SIZE
        84 => 1,         // _SC_NPROCESSORS_ONLN
        11 => 4096,      // _SC_PAGESIZE (same as PAGE_SIZE)
        _ => -1,
    }
}

/// sysconf constants
pub mod sc {
    pub const PAGE_SIZE: i32 = 30;
    pub const NPROCESSORS_ONLN: i32 = 84;
    pub const NPROCESSORS_CONF: i32 = 83;
    pub const PAGESIZE: i32 = 30;
    pub const CLK_TCK: i32 = 2;
    pub const OPEN_MAX: i32 = 4;
    pub const ARG_MAX: i32 = 0;
    pub const CHILD_MAX: i32 = 1;
}

/// TLS (Thread-Local Storage) support stubs
pub mod tls {
    /// TLS model types
    pub const TLS_MODEL_LOCAL_EXEC: u32 = 0;
    pub const TLS_MODEL_INITIAL_EXEC: u32 = 1;
    pub const TLS_MODEL_LOCAL_DYNAMIC: u32 = 2;
    pub const TLS_MODEL_GLOBAL_DYNAMIC: u32 = 3;

    /// Get TLS base address
    pub fn get_tls_base() -> u64 {
        // Read from FS segment base
        0
    }

    /// Set TLS base address
    pub fn set_tls_base(_base: u64) {
        // Would set FS segment base via arch_prctl
    }
}

/// Memory allocation hooks (glibc internals)
pub struct MallocHooks {
    pub malloc_hook: Option<extern "C" fn(usize) -> *mut u8>,
    pub free_hook: Option<extern "C" fn(*mut u8)>,
    pub realloc_hook: Option<extern "C" fn(*mut u8, usize) -> *mut u8>,
}

static mut MALLOC_HOOKS: MallocHooks = MallocHooks {
    malloc_hook: None,
    free_hook: None,
    realloc_hook: None,
};

/// Get glibc function addresses for the symbol table
pub fn get_glibc_functions() -> alloc::vec::Vec<(&'static str, u64)> {
    alloc::vec![
        ("__libc_start_main", libc_start_main as u64),
        ("__cxa_atexit", cxa_atexit as *const () as u64),
        ("__cxa_finalize", cxa_finalize as *const () as u64),
        ("__errno_location", errno_location as *const () as u64),
        ("__stack_chk_guard", stack_chk_guard as *const () as u64),
        ("__stack_chk_fail", stack_chk_fail as *const () as u64),
        ("getauxval", getauxval as *const () as u64),
        ("sysconf", sysconf as *const () as u64),
    ]
}

/// Stub for __libc_start_main
fn libc_start_main(
    _main: u64,
    _argc: i32,
    _argv: u64,
    _init: u64,
    _fini: u64,
    _rtld_fini: u64,
    _stack_end: u64,
) -> i32 {
    crate::kprintln!("glibc: __libc_start_main called");
    // In a real implementation, this would:
    // 1. Set up TLS
    // 2. Call init functions
    // 3. Call main(argc, argv, envp)
    // 4. Call exit with main's return value
    0
}
