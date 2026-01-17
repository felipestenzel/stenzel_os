//! KERNEL32 Emulation
//!
//! Emulates Windows kernel32.dll - provides process, thread, memory,
//! file, console, and other core Windows APIs.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;

/// Error codes (GetLastError)
pub mod error {
    pub const SUCCESS: u32 = 0;
    pub const INVALID_FUNCTION: u32 = 1;
    pub const FILE_NOT_FOUND: u32 = 2;
    pub const PATH_NOT_FOUND: u32 = 3;
    pub const TOO_MANY_OPEN_FILES: u32 = 4;
    pub const ACCESS_DENIED: u32 = 5;
    pub const INVALID_HANDLE: u32 = 6;
    pub const NOT_ENOUGH_MEMORY: u32 = 8;
    pub const INVALID_PARAMETER: u32 = 87;
    pub const INSUFFICIENT_BUFFER: u32 = 122;
    pub const ALREADY_EXISTS: u32 = 183;
    pub const ENVVAR_NOT_FOUND: u32 = 203;
    pub const NOT_SUPPORTED: u32 = 50;
}

/// Standard handles
pub const STD_INPUT_HANDLE: u32 = 0xFFFFFFF6;  // -10
pub const STD_OUTPUT_HANDLE: u32 = 0xFFFFFFF5; // -11
pub const STD_ERROR_HANDLE: u32 = 0xFFFFFFF4;  // -12

/// File access flags
pub mod access {
    pub const GENERIC_READ: u32 = 0x80000000;
    pub const GENERIC_WRITE: u32 = 0x40000000;
    pub const GENERIC_EXECUTE: u32 = 0x20000000;
    pub const GENERIC_ALL: u32 = 0x10000000;
}

/// File share mode
pub mod share {
    pub const FILE_SHARE_READ: u32 = 0x1;
    pub const FILE_SHARE_WRITE: u32 = 0x2;
    pub const FILE_SHARE_DELETE: u32 = 0x4;
}

/// Creation disposition
pub mod creation {
    pub const CREATE_NEW: u32 = 1;
    pub const CREATE_ALWAYS: u32 = 2;
    pub const OPEN_EXISTING: u32 = 3;
    pub const OPEN_ALWAYS: u32 = 4;
    pub const TRUNCATE_EXISTING: u32 = 5;
}

/// Memory allocation flags
pub mod mem {
    pub const MEM_COMMIT: u32 = 0x1000;
    pub const MEM_RESERVE: u32 = 0x2000;
    pub const MEM_DECOMMIT: u32 = 0x4000;
    pub const MEM_RELEASE: u32 = 0x8000;
    pub const MEM_FREE: u32 = 0x10000;
    pub const MEM_PRIVATE: u32 = 0x20000;
    pub const MEM_MAPPED: u32 = 0x40000;
    pub const MEM_RESET: u32 = 0x80000;
    pub const MEM_TOP_DOWN: u32 = 0x100000;
    pub const MEM_LARGE_PAGES: u32 = 0x20000000;
}

/// Memory protection flags
pub mod protect {
    pub const PAGE_NOACCESS: u32 = 0x01;
    pub const PAGE_READONLY: u32 = 0x02;
    pub const PAGE_READWRITE: u32 = 0x04;
    pub const PAGE_WRITECOPY: u32 = 0x08;
    pub const PAGE_EXECUTE: u32 = 0x10;
    pub const PAGE_EXECUTE_READ: u32 = 0x20;
    pub const PAGE_EXECUTE_READWRITE: u32 = 0x40;
    pub const PAGE_EXECUTE_WRITECOPY: u32 = 0x80;
    pub const PAGE_GUARD: u32 = 0x100;
    pub const PAGE_NOCACHE: u32 = 0x200;
}

/// Wait return values
pub const WAIT_OBJECT_0: u32 = 0;
pub const WAIT_ABANDONED: u32 = 0x80;
pub const WAIT_TIMEOUT: u32 = 0x102;
pub const WAIT_FAILED: u32 = 0xFFFFFFFF;
pub const INFINITE: u32 = 0xFFFFFFFF;

/// Thread local storage
const TLS_OUT_OF_INDEXES: u32 = 0xFFFFFFFF;

/// KERNEL32 emulator state
pub struct Kernel32Emulator {
    /// Last error code per thread (simplified: global for now)
    last_error: u32,
    /// Environment variables
    environment: BTreeMap<String, String>,
    /// TLS slots
    tls_slots: Vec<bool>,
    /// Heap handles
    heaps: Vec<HeapInfo>,
    /// Module handles
    modules: BTreeMap<String, u64>,
    /// Current directory
    current_dir: String,
}

#[derive(Debug)]
struct HeapInfo {
    base: u64,
    size: usize,
    flags: u32,
}

impl Kernel32Emulator {
    pub fn new() -> Self {
        let mut env = BTreeMap::new();
        // Set default environment
        env.insert(String::from("PATH"), String::from("C:\\Windows;C:\\Windows\\System32"));
        env.insert(String::from("SYSTEMROOT"), String::from("C:\\Windows"));
        env.insert(String::from("WINDIR"), String::from("C:\\Windows"));
        env.insert(String::from("TEMP"), String::from("C:\\Windows\\Temp"));
        env.insert(String::from("TMP"), String::from("C:\\Windows\\Temp"));
        env.insert(String::from("COMPUTERNAME"), String::from("STENZEL-OS"));
        env.insert(String::from("USERNAME"), String::from("user"));
        env.insert(String::from("OS"), String::from("Windows_NT"));

        Self {
            last_error: 0,
            environment: env,
            tls_slots: vec![false; 64], // 64 TLS slots
            heaps: Vec::new(),
            modules: BTreeMap::new(),
            current_dir: String::from("C:\\"),
        }
    }

    /// Set last error
    pub fn set_last_error(&mut self, error: u32) {
        self.last_error = error;
    }

    /// Get last error
    pub fn get_last_error(&self) -> u32 {
        self.last_error
    }

    // ===== Console Functions =====

    /// GetStdHandle
    pub fn get_std_handle(&self, std_handle: u32) -> u64 {
        match std_handle {
            STD_INPUT_HANDLE => 0, // STDIN
            STD_OUTPUT_HANDLE => 1, // STDOUT
            STD_ERROR_HANDLE => 2, // STDERR
            _ => u64::MAX, // INVALID_HANDLE_VALUE
        }
    }

    /// WriteConsoleA
    pub fn write_console_a(
        &mut self,
        handle: u64,
        buffer: &[u8],
        chars_written: &mut u32,
    ) -> bool {
        *chars_written = buffer.len() as u32;

        // Write to our console
        for &b in buffer {
            crate::console::write_byte(b);
        }

        self.last_error = error::SUCCESS;
        true
    }

    /// WriteFile
    pub fn write_file(
        &mut self,
        handle: u64,
        buffer: &[u8],
        bytes_written: &mut u32,
    ) -> bool {
        if handle <= 2 {
            // Standard handles
            return self.write_console_a(handle, buffer, bytes_written);
        }

        // TODO: Implement actual file writing
        self.last_error = error::NOT_SUPPORTED;
        false
    }

    /// ReadFile
    pub fn read_file(
        &mut self,
        handle: u64,
        buffer: &mut [u8],
        bytes_read: &mut u32,
    ) -> bool {
        *bytes_read = 0;

        if handle == 0 {
            // STDIN
            // TODO: Implement console reading
            self.last_error = error::NOT_SUPPORTED;
            return false;
        }

        // TODO: Implement actual file reading
        self.last_error = error::NOT_SUPPORTED;
        false
    }

    // ===== File Functions =====

    /// CreateFileA
    pub fn create_file_a(
        &mut self,
        filename: &str,
        desired_access: u32,
        share_mode: u32,
        creation_disposition: u32,
    ) -> u64 {
        crate::kprintln!("kernel32: CreateFileA(\"{}\")", filename);

        // Translate Windows path to Unix path
        let unix_path = super::fs_translate::windows_to_unix(filename);

        // Map to our open syscall
        let flags = match creation_disposition {
            creation::CREATE_NEW => 0o101, // O_CREAT | O_EXCL
            creation::CREATE_ALWAYS => 0o1101, // O_CREAT | O_TRUNC | O_WRONLY
            creation::OPEN_EXISTING => 0, // O_RDONLY
            creation::OPEN_ALWAYS => 0o100, // O_CREAT
            creation::TRUNCATE_EXISTING => 0o1000, // O_TRUNC
            _ => 0,
        };

        // TODO: Call our open syscall
        self.last_error = error::NOT_SUPPORTED;
        u64::MAX // INVALID_HANDLE_VALUE
    }

    /// CloseHandle
    pub fn close_handle(&mut self, handle: u64) -> bool {
        if handle == u64::MAX {
            self.last_error = error::INVALID_HANDLE;
            return false;
        }

        // TODO: Call our close syscall
        self.last_error = error::SUCCESS;
        true
    }

    /// GetFileSize
    pub fn get_file_size(&mut self, handle: u64, high_part: Option<&mut u32>) -> u32 {
        if let Some(h) = high_part {
            *h = 0;
        }
        // TODO: Implement
        self.last_error = error::INVALID_HANDLE;
        u32::MAX // INVALID_FILE_SIZE
    }

    // ===== Memory Functions =====

    /// VirtualAlloc
    pub fn virtual_alloc(
        &mut self,
        address: u64,
        size: usize,
        allocation_type: u32,
        protect: u32,
    ) -> u64 {
        crate::kprintln!("kernel32: VirtualAlloc(addr={:#x}, size={:#x}, type={:#x})",
            address, size, allocation_type);

        // Map protection flags
        let _prot = match protect {
            protect::PAGE_NOACCESS => 0,
            protect::PAGE_READONLY => 1,
            protect::PAGE_READWRITE => 3,
            protect::PAGE_EXECUTE => 4,
            protect::PAGE_EXECUTE_READ => 5,
            protect::PAGE_EXECUTE_READWRITE => 7,
            _ => 3,
        };

        // TODO: Call our mmap syscall
        self.last_error = error::NOT_SUPPORTED;
        0
    }

    /// VirtualFree
    pub fn virtual_free(&mut self, address: u64, size: usize, free_type: u32) -> bool {
        crate::kprintln!("kernel32: VirtualFree(addr={:#x}, size={:#x})", address, size);

        // TODO: Call our munmap syscall
        self.last_error = error::NOT_SUPPORTED;
        false
    }

    /// GetProcessHeap
    pub fn get_process_heap(&mut self) -> u64 {
        // Return a pseudo-handle for the default heap
        0x10000
    }

    /// HeapAlloc
    pub fn heap_alloc(&mut self, heap: u64, flags: u32, size: usize) -> u64 {
        // Use our allocator
        // TODO: Actually allocate
        self.last_error = error::NOT_ENOUGH_MEMORY;
        0
    }

    /// HeapFree
    pub fn heap_free(&mut self, heap: u64, flags: u32, ptr: u64) -> bool {
        // TODO: Actually free
        self.last_error = error::SUCCESS;
        true
    }

    // ===== Process/Thread Functions =====

    /// GetCurrentProcess
    pub fn get_current_process(&self) -> u64 {
        // Pseudo-handle for current process
        u64::MAX // -1
    }

    /// GetCurrentProcessId
    pub fn get_current_process_id(&self) -> u32 {
        // Get from our task system
        let current = crate::sched::current_task();
        current.id() as u32
    }

    /// GetCurrentThread
    pub fn get_current_thread(&self) -> u64 {
        // Pseudo-handle for current thread
        u64::MAX - 1 // -2
    }

    /// GetCurrentThreadId
    pub fn get_current_thread_id(&self) -> u32 {
        self.get_current_process_id() // Simplified: one thread per process
    }

    /// ExitProcess
    pub fn exit_process(&self, exit_code: u32) -> ! {
        crate::kprintln!("kernel32: ExitProcess({})", exit_code);
        crate::syscall::sys_exit(exit_code as u64);
    }

    /// CreateThread
    pub fn create_thread(
        &mut self,
        stack_size: usize,
        start_address: u64,
        parameter: u64,
        creation_flags: u32,
        thread_id: &mut u32,
    ) -> u64 {
        crate::kprintln!("kernel32: CreateThread(start={:#x})", start_address);

        // TODO: Create thread via clone syscall
        self.last_error = error::NOT_SUPPORTED;
        0
    }

    /// Sleep
    pub fn sleep(&self, milliseconds: u32) {
        crate::kprintln!("kernel32: Sleep({}ms)", milliseconds);
        // TODO: Call nanosleep
    }

    /// WaitForSingleObject
    pub fn wait_for_single_object(&mut self, handle: u64, milliseconds: u32) -> u32 {
        crate::kprintln!("kernel32: WaitForSingleObject(handle={:#x}, ms={})", handle, milliseconds);
        WAIT_FAILED
    }

    // ===== TLS Functions =====

    /// TlsAlloc
    pub fn tls_alloc(&mut self) -> u32 {
        for (i, slot) in self.tls_slots.iter_mut().enumerate() {
            if !*slot {
                *slot = true;
                return i as u32;
            }
        }
        self.last_error = error::NOT_ENOUGH_MEMORY;
        TLS_OUT_OF_INDEXES
    }

    /// TlsFree
    pub fn tls_free(&mut self, index: u32) -> bool {
        if (index as usize) < self.tls_slots.len() {
            self.tls_slots[index as usize] = false;
            true
        } else {
            self.last_error = error::INVALID_PARAMETER;
            false
        }
    }

    // ===== Environment Functions =====

    /// GetEnvironmentVariableA
    pub fn get_environment_variable_a(
        &mut self,
        name: &str,
        buffer: &mut [u8],
    ) -> u32 {
        if let Some(value) = self.environment.get(name) {
            let bytes = value.as_bytes();
            if bytes.len() < buffer.len() {
                buffer[..bytes.len()].copy_from_slice(bytes);
                buffer[bytes.len()] = 0;
                self.last_error = error::SUCCESS;
                bytes.len() as u32
            } else {
                self.last_error = error::INSUFFICIENT_BUFFER;
                (bytes.len() + 1) as u32
            }
        } else {
            self.last_error = error::ENVVAR_NOT_FOUND;
            0
        }
    }

    /// SetEnvironmentVariableA
    pub fn set_environment_variable_a(&mut self, name: &str, value: Option<&str>) -> bool {
        if let Some(v) = value {
            self.environment.insert(String::from(name), String::from(v));
        } else {
            self.environment.remove(name);
        }
        self.last_error = error::SUCCESS;
        true
    }

    // ===== Module Functions =====

    /// GetModuleHandleA
    pub fn get_module_handle_a(&mut self, module_name: Option<&str>) -> u64 {
        if module_name.is_none() {
            // Return handle to main executable
            return 0x0040_0000; // Default base address
        }

        let name = module_name.unwrap().to_lowercase();
        if let Some(&handle) = self.modules.get(&name) {
            handle
        } else {
            self.last_error = error::FILE_NOT_FOUND;
            0
        }
    }

    /// LoadLibraryA
    pub fn load_library_a(&mut self, filename: &str) -> u64 {
        crate::kprintln!("kernel32: LoadLibraryA(\"{}\")", filename);

        // Check if already loaded
        let name = filename.to_lowercase();
        if let Some(&handle) = self.modules.get(&name) {
            return handle;
        }

        // TODO: Actually load DLL from file
        self.last_error = error::FILE_NOT_FOUND;
        0
    }

    /// GetProcAddress
    pub fn get_proc_address(&mut self, module: u64, proc_name: &str) -> u64 {
        crate::kprintln!("kernel32: GetProcAddress(module={:#x}, \"{}\")", module, proc_name);

        // TODO: Look up in module exports
        self.last_error = error::FILE_NOT_FOUND;
        0
    }

    /// FreeLibrary
    pub fn free_library(&mut self, module: u64) -> bool {
        // TODO: Implement
        true
    }

    // ===== System Info Functions =====

    /// GetSystemInfo
    pub fn get_system_info(&self, info: &mut SystemInfo) {
        info.processor_architecture = 9; // AMD64
        info.page_size = 4096;
        info.minimum_application_address = 0x10000;
        info.maximum_application_address = 0x7FFFFFFEFFFF;
        info.active_processor_mask = 1;
        info.number_of_processors = 1;
        info.processor_type = 8664;
        info.allocation_granularity = 65536;
        info.processor_level = 6;
        info.processor_revision = 0;
    }

    /// GetTickCount
    pub fn get_tick_count(&self) -> u32 {
        (crate::time::ticks() & 0xFFFFFFFF) as u32
    }

    /// GetTickCount64
    pub fn get_tick_count64(&self) -> u64 {
        crate::time::ticks()
    }

    /// QueryPerformanceCounter
    pub fn query_performance_counter(&self, counter: &mut i64) -> bool {
        *counter = crate::time::ticks() as i64;
        true
    }

    /// QueryPerformanceFrequency
    pub fn query_performance_frequency(&self, frequency: &mut i64) -> bool {
        *frequency = 1000; // 1000 Hz (ms resolution)
        true
    }

    /// GetCurrentDirectoryA
    pub fn get_current_directory_a(&self, buffer: &mut [u8]) -> u32 {
        let dir = self.current_dir.as_bytes();
        if dir.len() < buffer.len() {
            buffer[..dir.len()].copy_from_slice(dir);
            buffer[dir.len()] = 0;
            dir.len() as u32
        } else {
            (dir.len() + 1) as u32
        }
    }

    /// SetCurrentDirectoryA
    pub fn set_current_directory_a(&mut self, path: &str) -> bool {
        self.current_dir = String::from(path);
        true
    }
}

impl Default for Kernel32Emulator {
    fn default() -> Self {
        Self::new()
    }
}

/// System Info structure
#[repr(C)]
#[derive(Debug, Default)]
pub struct SystemInfo {
    pub processor_architecture: u16,
    pub reserved: u16,
    pub page_size: u32,
    pub minimum_application_address: u64,
    pub maximum_application_address: u64,
    pub active_processor_mask: u64,
    pub number_of_processors: u32,
    pub processor_type: u32,
    pub allocation_granularity: u32,
    pub processor_level: u16,
    pub processor_revision: u16,
}

/// Get KERNEL32 exports for the loader
pub fn get_exports() -> BTreeMap<String, u64> {
    let mut exports = BTreeMap::new();

    let funcs = [
        // Console
        "GetStdHandle",
        "WriteConsoleA",
        "WriteConsoleW",
        "ReadConsoleA",
        "ReadConsoleW",
        "SetConsoleMode",
        "GetConsoleMode",

        // File I/O
        "CreateFileA",
        "CreateFileW",
        "ReadFile",
        "WriteFile",
        "CloseHandle",
        "GetFileSize",
        "GetFileSizeEx",
        "SetFilePointer",
        "SetFilePointerEx",
        "GetFileType",
        "FlushFileBuffers",
        "DeleteFileA",
        "DeleteFileW",
        "MoveFileA",
        "MoveFileW",
        "CopyFileA",
        "CopyFileW",

        // Directory
        "CreateDirectoryA",
        "CreateDirectoryW",
        "RemoveDirectoryA",
        "RemoveDirectoryW",
        "GetCurrentDirectoryA",
        "GetCurrentDirectoryW",
        "SetCurrentDirectoryA",
        "SetCurrentDirectoryW",
        "FindFirstFileA",
        "FindFirstFileW",
        "FindNextFileA",
        "FindNextFileW",
        "FindClose",

        // Memory
        "VirtualAlloc",
        "VirtualFree",
        "VirtualProtect",
        "VirtualQuery",
        "GetProcessHeap",
        "HeapCreate",
        "HeapDestroy",
        "HeapAlloc",
        "HeapReAlloc",
        "HeapFree",
        "HeapSize",
        "GlobalAlloc",
        "GlobalFree",
        "GlobalLock",
        "GlobalUnlock",
        "LocalAlloc",
        "LocalFree",

        // Process/Thread
        "GetCurrentProcess",
        "GetCurrentProcessId",
        "GetCurrentThread",
        "GetCurrentThreadId",
        "ExitProcess",
        "TerminateProcess",
        "CreateThread",
        "ExitThread",
        "TerminateThread",
        "GetExitCodeThread",
        "Sleep",
        "SleepEx",
        "WaitForSingleObject",
        "WaitForMultipleObjects",
        "CreateEventA",
        "CreateEventW",
        "SetEvent",
        "ResetEvent",
        "CreateMutexA",
        "CreateMutexW",
        "ReleaseMutex",

        // TLS
        "TlsAlloc",
        "TlsFree",
        "TlsGetValue",
        "TlsSetValue",

        // Environment
        "GetEnvironmentVariableA",
        "GetEnvironmentVariableW",
        "SetEnvironmentVariableA",
        "SetEnvironmentVariableW",
        "GetEnvironmentStringsA",
        "GetEnvironmentStringsW",
        "FreeEnvironmentStringsA",
        "FreeEnvironmentStringsW",

        // Module
        "GetModuleHandleA",
        "GetModuleHandleW",
        "GetModuleFileNameA",
        "GetModuleFileNameW",
        "LoadLibraryA",
        "LoadLibraryW",
        "LoadLibraryExA",
        "LoadLibraryExW",
        "GetProcAddress",
        "FreeLibrary",

        // System Info
        "GetSystemInfo",
        "GetNativeSystemInfo",
        "GetVersionExA",
        "GetVersionExW",
        "GetVersion",
        "GetTickCount",
        "GetTickCount64",
        "QueryPerformanceCounter",
        "QueryPerformanceFrequency",
        "GetSystemTime",
        "GetLocalTime",
        "GetSystemTimeAsFileTime",

        // Error
        "GetLastError",
        "SetLastError",

        // String
        "lstrlenA",
        "lstrlenW",
        "lstrcpyA",
        "lstrcpyW",
        "lstrcatA",
        "lstrcatW",
        "lstrcmpA",
        "lstrcmpW",
        "lstrcmpiA",
        "lstrcmpiW",

        // Interlocked
        "InterlockedIncrement",
        "InterlockedDecrement",
        "InterlockedExchange",
        "InterlockedCompareExchange",

        // Critical Section
        "InitializeCriticalSection",
        "DeleteCriticalSection",
        "EnterCriticalSection",
        "LeaveCriticalSection",
        "TryEnterCriticalSection",
    ];

    let mut addr = 0x7FF0_0000u64;
    for func in funcs {
        exports.insert(String::from(func), addr);
        addr += 16;
    }

    exports
}
