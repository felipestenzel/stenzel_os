//! KERNEL32 Emulation
//!
//! Emulates Windows kernel32.dll - provides process, thread, memory,
//! file, console, and other core Windows APIs.
//!
//! This is the main Win32 API layer used by most Windows applications.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;

use super::ntdll::{self, status as nt_status, LargeInteger, IoStatusBlock, ObjectAttributes, UnicodeString};
use super::ntsyscall::{self, NtSyscallContext};
use super::fs_translate;

/// Win32 Error codes (GetLastError)
pub mod error {
    pub const SUCCESS: u32 = 0;
    pub const INVALID_FUNCTION: u32 = 1;
    pub const FILE_NOT_FOUND: u32 = 2;
    pub const PATH_NOT_FOUND: u32 = 3;
    pub const TOO_MANY_OPEN_FILES: u32 = 4;
    pub const ACCESS_DENIED: u32 = 5;
    pub const INVALID_HANDLE: u32 = 6;
    pub const ARENA_TRASHED: u32 = 7;
    pub const NOT_ENOUGH_MEMORY: u32 = 8;
    pub const INVALID_BLOCK: u32 = 9;
    pub const BAD_ENVIRONMENT: u32 = 10;
    pub const BAD_FORMAT: u32 = 11;
    pub const INVALID_ACCESS: u32 = 12;
    pub const INVALID_DATA: u32 = 13;
    pub const OUTOFMEMORY: u32 = 14;
    pub const INVALID_DRIVE: u32 = 15;
    pub const CURRENT_DIRECTORY: u32 = 16;
    pub const NOT_SAME_DEVICE: u32 = 17;
    pub const NO_MORE_FILES: u32 = 18;
    pub const WRITE_PROTECT: u32 = 19;
    pub const BAD_UNIT: u32 = 20;
    pub const NOT_READY: u32 = 21;
    pub const BAD_COMMAND: u32 = 22;
    pub const CRC: u32 = 23;
    pub const BAD_LENGTH: u32 = 24;
    pub const SEEK: u32 = 25;
    pub const NOT_DOS_DISK: u32 = 26;
    pub const SECTOR_NOT_FOUND: u32 = 27;
    pub const OUT_OF_PAPER: u32 = 28;
    pub const WRITE_FAULT: u32 = 29;
    pub const READ_FAULT: u32 = 30;
    pub const GEN_FAILURE: u32 = 31;
    pub const SHARING_VIOLATION: u32 = 32;
    pub const LOCK_VIOLATION: u32 = 33;
    pub const WRONG_DISK: u32 = 34;
    pub const SHARING_BUFFER_EXCEEDED: u32 = 36;
    pub const HANDLE_EOF: u32 = 38;
    pub const HANDLE_DISK_FULL: u32 = 39;
    pub const NOT_SUPPORTED: u32 = 50;
    pub const REM_NOT_LIST: u32 = 51;
    pub const DUP_NAME: u32 = 52;
    pub const BAD_NETPATH: u32 = 53;
    pub const NETWORK_BUSY: u32 = 54;
    pub const DEV_NOT_EXIST: u32 = 55;
    pub const FILE_EXISTS: u32 = 80;
    pub const CANNOT_MAKE: u32 = 82;
    pub const FAIL_I24: u32 = 83;
    pub const INVALID_PARAMETER: u32 = 87;
    pub const NET_WRITE_FAULT: u32 = 88;
    pub const NO_PROC_SLOTS: u32 = 89;
    pub const BROKEN_PIPE: u32 = 109;
    pub const DISK_FULL: u32 = 112;
    pub const INSUFFICIENT_BUFFER: u32 = 122;
    pub const INVALID_NAME: u32 = 123;
    pub const INVALID_LEVEL: u32 = 124;
    pub const NO_VOLUME_LABEL: u32 = 125;
    pub const MOD_NOT_FOUND: u32 = 126;
    pub const PROC_NOT_FOUND: u32 = 127;
    pub const WAIT_NO_CHILDREN: u32 = 128;
    pub const NEGATIVE_SEEK: u32 = 131;
    pub const SEEK_ON_DEVICE: u32 = 132;
    pub const ALREADY_EXISTS: u32 = 183;
    pub const ENVVAR_NOT_FOUND: u32 = 203;
    pub const NO_MORE_ITEMS: u32 = 259;
    pub const TIMEOUT: u32 = 1460;
    pub const OPERATION_ABORTED: u32 = 995;
    pub const IO_INCOMPLETE: u32 = 996;
    pub const IO_PENDING: u32 = 997;
    pub const NOACCESS: u32 = 998;
}

/// Standard handles
pub const STD_INPUT_HANDLE: u32 = 0xFFFFFFF6;  // -10
pub const STD_OUTPUT_HANDLE: u32 = 0xFFFFFFF5; // -11
pub const STD_ERROR_HANDLE: u32 = 0xFFFFFFF4;  // -12

/// Invalid handle value
pub const INVALID_HANDLE_VALUE: u64 = 0xFFFFFFFF_FFFFFFFF;

/// File access flags
pub mod access {
    pub const GENERIC_READ: u32 = 0x80000000;
    pub const GENERIC_WRITE: u32 = 0x40000000;
    pub const GENERIC_EXECUTE: u32 = 0x20000000;
    pub const GENERIC_ALL: u32 = 0x10000000;
    pub const FILE_READ_DATA: u32 = 0x0001;
    pub const FILE_WRITE_DATA: u32 = 0x0002;
    pub const FILE_APPEND_DATA: u32 = 0x0004;
    pub const FILE_READ_EA: u32 = 0x0008;
    pub const FILE_WRITE_EA: u32 = 0x0010;
    pub const FILE_EXECUTE: u32 = 0x0020;
    pub const FILE_READ_ATTRIBUTES: u32 = 0x0080;
    pub const FILE_WRITE_ATTRIBUTES: u32 = 0x0100;
    pub const DELETE: u32 = 0x00010000;
    pub const READ_CONTROL: u32 = 0x00020000;
    pub const WRITE_DAC: u32 = 0x00040000;
    pub const WRITE_OWNER: u32 = 0x00080000;
    pub const SYNCHRONIZE: u32 = 0x00100000;
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

/// File attributes
pub mod file_attr {
    pub const READONLY: u32 = 0x00000001;
    pub const HIDDEN: u32 = 0x00000002;
    pub const SYSTEM: u32 = 0x00000004;
    pub const DIRECTORY: u32 = 0x00000010;
    pub const ARCHIVE: u32 = 0x00000020;
    pub const DEVICE: u32 = 0x00000040;
    pub const NORMAL: u32 = 0x00000080;
    pub const TEMPORARY: u32 = 0x00000100;
    pub const SPARSE_FILE: u32 = 0x00000200;
    pub const REPARSE_POINT: u32 = 0x00000400;
    pub const COMPRESSED: u32 = 0x00000800;
    pub const OFFLINE: u32 = 0x00001000;
    pub const NOT_CONTENT_INDEXED: u32 = 0x00002000;
    pub const ENCRYPTED: u32 = 0x00004000;
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
    pub const MEM_WRITE_WATCH: u32 = 0x200000;
    pub const MEM_PHYSICAL: u32 = 0x400000;
    pub const MEM_LARGE_PAGES: u32 = 0x20000000;
    pub const MEM_4MB_PAGES: u32 = 0x80000000;
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
    pub const PAGE_WRITECOMBINE: u32 = 0x400;
}

/// Wait return values
pub const WAIT_OBJECT_0: u32 = 0;
pub const WAIT_ABANDONED: u32 = 0x80;
pub const WAIT_TIMEOUT: u32 = 0x102;
pub const WAIT_FAILED: u32 = 0xFFFFFFFF;
pub const INFINITE: u32 = 0xFFFFFFFF;

/// Thread local storage
const TLS_OUT_OF_INDEXES: u32 = 0xFFFFFFFF;

/// File type
pub mod file_type {
    pub const FILE_TYPE_UNKNOWN: u32 = 0x0000;
    pub const FILE_TYPE_DISK: u32 = 0x0001;
    pub const FILE_TYPE_CHAR: u32 = 0x0002;
    pub const FILE_TYPE_PIPE: u32 = 0x0003;
    pub const FILE_TYPE_REMOTE: u32 = 0x8000;
}

/// Move method for SetFilePointer
pub mod move_method {
    pub const FILE_BEGIN: u32 = 0;
    pub const FILE_CURRENT: u32 = 1;
    pub const FILE_END: u32 = 2;
}

/// Heap flags
pub mod heap_flags {
    pub const HEAP_NO_SERIALIZE: u32 = 0x00000001;
    pub const HEAP_GROWABLE: u32 = 0x00000002;
    pub const HEAP_GENERATE_EXCEPTIONS: u32 = 0x00000004;
    pub const HEAP_ZERO_MEMORY: u32 = 0x00000008;
    pub const HEAP_REALLOC_IN_PLACE_ONLY: u32 = 0x00000010;
    pub const HEAP_CREATE_ENABLE_TRACING: u32 = 0x00020000;
}

/// KERNEL32 emulator state
pub struct Kernel32Emulator {
    /// Syscall context
    syscall_ctx: NtSyscallContext,
    /// Last error code per thread (simplified: global for now)
    last_error: u32,
    /// Environment variables
    environment: BTreeMap<String, String>,
    /// TLS slots (index -> in_use)
    tls_slots: Vec<bool>,
    /// TLS values per slot
    tls_values: Vec<u64>,
    /// Heap handles
    heaps: BTreeMap<u64, HeapInfo>,
    next_heap_handle: u64,
    /// Default process heap
    process_heap: u64,
    /// Module handles
    modules: BTreeMap<String, u64>,
    /// Current directory
    current_dir: String,
    /// Command line
    command_line: String,
    /// Handle to fd mapping
    handles: BTreeMap<u64, HandleInfo>,
    next_handle: u64,
    /// Critical sections
    critical_sections: BTreeMap<u64, CriticalSection>,
}

#[derive(Debug)]
struct HeapInfo {
    base: u64,
    size: usize,
    flags: u32,
    allocations: BTreeMap<u64, usize>,
    next_alloc: u64,
}

#[derive(Debug, Clone)]
enum HandleInfo {
    File { fd: i32, path: String, access: u32 },
    Console { fd: i32 },
    Event { signaled: bool, manual_reset: bool },
    Mutex { owner: Option<u32> },
    Semaphore { count: u32, max: u32 },
    Thread { tid: u32 },
    Process { pid: u32 },
    FindFile { path: String, index: usize },
}

#[derive(Debug, Clone, Default)]
struct CriticalSection {
    owner_thread: u32,
    recursion_count: u32,
    lock_count: i32,
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
        env.insert(String::from("USERPROFILE"), String::from("C:\\Users\\user"));
        env.insert(String::from("HOMEDRIVE"), String::from("C:"));
        env.insert(String::from("HOMEPATH"), String::from("\\Users\\user"));
        env.insert(String::from("OS"), String::from("Windows_NT"));
        env.insert(String::from("PROCESSOR_ARCHITECTURE"), String::from("AMD64"));
        env.insert(String::from("NUMBER_OF_PROCESSORS"), String::from("1"));

        let mut handles = BTreeMap::new();
        // Pre-register standard handles
        handles.insert(0x10, HandleInfo::Console { fd: 0 }); // stdin
        handles.insert(0x14, HandleInfo::Console { fd: 1 }); // stdout
        handles.insert(0x18, HandleInfo::Console { fd: 2 }); // stderr

        // Create default process heap
        let process_heap = 0x00500000;
        let mut heaps = BTreeMap::new();
        heaps.insert(process_heap, HeapInfo {
            base: process_heap,
            size: 0x100000,
            flags: heap_flags::HEAP_GROWABLE,
            allocations: BTreeMap::new(),
            next_alloc: process_heap + 0x1000,
        });

        Self {
            syscall_ctx: NtSyscallContext::new(),
            last_error: 0,
            environment: env,
            tls_slots: vec![false; 1088], // TLS_MINIMUM_AVAILABLE (64) + expansion slots
            tls_values: vec![0; 1088],
            heaps,
            next_heap_handle: 0x00600000,
            process_heap,
            modules: BTreeMap::new(),
            current_dir: String::from("C:\\"),
            command_line: String::from(""),
            handles,
            next_handle: 0x100,
            critical_sections: BTreeMap::new(),
        }
    }

    fn alloc_handle(&mut self, info: HandleInfo) -> u64 {
        let handle = self.next_handle;
        self.next_handle += 4;
        self.handles.insert(handle, info);
        handle
    }

    fn translate_handle_to_fd(&self, handle: u64) -> Option<i32> {
        match self.handles.get(&handle) {
            Some(HandleInfo::File { fd, .. }) => Some(*fd),
            Some(HandleInfo::Console { fd }) => Some(*fd),
            _ => None,
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

    // =========================================================================
    // Console Functions
    // =========================================================================

    /// GetStdHandle
    pub fn get_std_handle(&self, std_handle: u32) -> u64 {
        match std_handle {
            STD_INPUT_HANDLE => 0x10,
            STD_OUTPUT_HANDLE => 0x14,
            STD_ERROR_HANDLE => 0x18,
            _ => INVALID_HANDLE_VALUE,
        }
    }

    /// WriteConsoleA
    pub fn write_console_a(
        &mut self,
        handle: u64,
        buffer: &[u8],
        chars_written: &mut u32,
    ) -> bool {
        let fd = match self.translate_handle_to_fd(handle) {
            Some(fd) => fd,
            None => {
                self.last_error = error::INVALID_HANDLE;
                return false;
            }
        };

        let result = crate::syscall::sys_write(fd, buffer.as_ptr() as u64, buffer.len());

        if result < 0 {
            self.last_error = error::WRITE_FAULT;
            *chars_written = 0;
            false
        } else {
            self.last_error = error::SUCCESS;
            *chars_written = result as u32;
            true
        }
    }

    /// WriteConsoleW
    pub fn write_console_w(
        &mut self,
        handle: u64,
        buffer: &[u16],
        chars_written: &mut u32,
    ) -> bool {
        // Convert UTF-16 to UTF-8
        let utf8: Vec<u8> = buffer.iter()
            .filter_map(|&c| {
                if c < 128 {
                    Some(c as u8)
                } else {
                    None // Simplified: only handle ASCII
                }
            })
            .collect();

        self.write_console_a(handle, &utf8, chars_written)
    }

    /// ReadConsoleA
    pub fn read_console_a(
        &mut self,
        handle: u64,
        buffer: &mut [u8],
        chars_read: &mut u32,
    ) -> bool {
        let fd = match self.translate_handle_to_fd(handle) {
            Some(fd) => fd,
            None => {
                self.last_error = error::INVALID_HANDLE;
                return false;
            }
        };

        let result = crate::syscall::sys_read(fd, buffer.as_mut_ptr() as u64, buffer.len());

        if result < 0 {
            self.last_error = error::READ_FAULT;
            *chars_read = 0;
            false
        } else {
            self.last_error = error::SUCCESS;
            *chars_read = result as u32;
            true
        }
    }

    /// GetConsoleMode
    pub fn get_console_mode(&self, handle: u64, mode: &mut u32) -> bool {
        if self.translate_handle_to_fd(handle).is_some() {
            *mode = 0x1 | 0x2 | 0x4; // ENABLE_PROCESSED_INPUT | ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT
            true
        } else {
            false
        }
    }

    /// SetConsoleMode
    pub fn set_console_mode(&mut self, handle: u64, mode: u32) -> bool {
        self.translate_handle_to_fd(handle).is_some()
    }

    // =========================================================================
    // File Functions
    // =========================================================================

    /// WriteFile
    pub fn write_file(
        &mut self,
        handle: u64,
        buffer: &[u8],
        bytes_written: &mut u32,
        overlapped: u64,
    ) -> bool {
        let fd = match self.translate_handle_to_fd(handle) {
            Some(fd) => fd,
            None => {
                self.last_error = error::INVALID_HANDLE;
                return false;
            }
        };

        let result = crate::syscall::sys_write(fd, buffer.as_ptr() as u64, buffer.len());

        if result < 0 {
            self.last_error = error::WRITE_FAULT;
            *bytes_written = 0;
            false
        } else {
            self.last_error = error::SUCCESS;
            *bytes_written = result as u32;
            true
        }
    }

    /// ReadFile
    pub fn read_file(
        &mut self,
        handle: u64,
        buffer: &mut [u8],
        bytes_read: &mut u32,
        overlapped: u64,
    ) -> bool {
        let fd = match self.translate_handle_to_fd(handle) {
            Some(fd) => fd,
            None => {
                self.last_error = error::INVALID_HANDLE;
                return false;
            }
        };

        let result = crate::syscall::sys_read(fd, buffer.as_mut_ptr() as u64, buffer.len());

        if result < 0 {
            self.last_error = error::READ_FAULT;
            *bytes_read = 0;
            false
        } else {
            self.last_error = error::SUCCESS;
            *bytes_read = result as u32;
            true
        }
    }

    /// CreateFileA
    pub fn create_file_a(
        &mut self,
        filename: &str,
        desired_access: u32,
        share_mode: u32,
        security_attributes: u64,
        creation_disposition: u32,
        flags_and_attributes: u32,
        template_file: u64,
    ) -> u64 {
        // Translate Windows path to Unix path
        let unix_path = fs_translate::windows_to_unix(filename);

        // Map creation disposition to flags
        let mut flags: u64 = 0;

        match creation_disposition {
            creation::CREATE_NEW => flags |= 0o100 | 0o200, // O_CREAT | O_EXCL
            creation::CREATE_ALWAYS => flags |= 0o100 | 0o1000, // O_CREAT | O_TRUNC
            creation::OPEN_EXISTING => {}, // O_RDONLY (0)
            creation::OPEN_ALWAYS => flags |= 0o100, // O_CREAT
            creation::TRUNCATE_EXISTING => flags |= 0o1000, // O_TRUNC
            _ => {}
        }

        // Map access to flags
        if (desired_access & access::GENERIC_WRITE) != 0 {
            if (desired_access & access::GENERIC_READ) != 0 {
                flags |= 0o2; // O_RDWR
            } else {
                flags |= 0o1; // O_WRONLY
            }
        }

        if (desired_access & access::FILE_APPEND_DATA) != 0 {
            flags |= 0o2000; // O_APPEND
        }

        // Call open syscall
        let fd = crate::syscall::sys_open(unix_path.as_ptr() as u64, flags as u32, 0o644);

        if fd < 0 {
            self.last_error = match fd {
                -2 => error::FILE_NOT_FOUND,
                -13 => error::ACCESS_DENIED,
                -17 => error::FILE_EXISTS,
                _ => error::GEN_FAILURE,
            };
            return INVALID_HANDLE_VALUE;
        }

        // Allocate handle
        let handle = self.alloc_handle(HandleInfo::File {
            fd: fd as i32,
            path: filename.to_string(),
            access: desired_access,
        });

        self.last_error = error::SUCCESS;
        handle
    }

    /// CreateFileW
    pub fn create_file_w(
        &mut self,
        filename: &[u16],
        desired_access: u32,
        share_mode: u32,
        security_attributes: u64,
        creation_disposition: u32,
        flags_and_attributes: u32,
        template_file: u64,
    ) -> u64 {
        // Convert UTF-16 to UTF-8
        let name = String::from_utf16_lossy(filename);
        let name = name.trim_end_matches('\0');
        self.create_file_a(name, desired_access, share_mode, security_attributes,
                           creation_disposition, flags_and_attributes, template_file)
    }

    /// CloseHandle
    pub fn close_handle(&mut self, handle: u64) -> bool {
        if handle == INVALID_HANDLE_VALUE {
            self.last_error = error::INVALID_HANDLE;
            return false;
        }

        if let Some(info) = self.handles.remove(&handle) {
            match info {
                HandleInfo::File { fd, .. } | HandleInfo::Console { fd } => {
                    if fd > 2 { // Don't close stdio
                        crate::syscall::sys_close(fd);
                    }
                }
                _ => {}
            }
            self.last_error = error::SUCCESS;
            true
        } else {
            self.last_error = error::INVALID_HANDLE;
            false
        }
    }

    /// GetFileSize
    pub fn get_file_size(&mut self, handle: u64, high_part: Option<&mut u32>) -> u32 {
        let fd = match self.translate_handle_to_fd(handle) {
            Some(fd) => fd,
            None => {
                self.last_error = error::INVALID_HANDLE;
                return u32::MAX;
            }
        };

        // Save current position
        let current_pos = crate::syscall::sys_lseek(fd, 0, 1); // SEEK_CUR

        // Seek to end
        let size = crate::syscall::sys_lseek(fd, 0, 2); // SEEK_END

        // Restore position
        crate::syscall::sys_lseek(fd, current_pos, 0); // SEEK_SET

        if let Some(h) = high_part {
            *h = (size >> 32) as u32;
        }

        self.last_error = error::SUCCESS;
        size as u32
    }

    /// GetFileSizeEx
    pub fn get_file_size_ex(&mut self, handle: u64, file_size: &mut i64) -> bool {
        let fd = match self.translate_handle_to_fd(handle) {
            Some(fd) => fd,
            None => {
                self.last_error = error::INVALID_HANDLE;
                return false;
            }
        };

        let current_pos = crate::syscall::sys_lseek(fd, 0, 1);
        let size = crate::syscall::sys_lseek(fd, 0, 2);
        crate::syscall::sys_lseek(fd, current_pos, 0);

        *file_size = size as i64;
        self.last_error = error::SUCCESS;
        true
    }

    /// SetFilePointer
    pub fn set_file_pointer(
        &mut self,
        handle: u64,
        distance_to_move: i32,
        distance_to_move_high: Option<&mut i32>,
        move_method: u32,
    ) -> u32 {
        let fd = match self.translate_handle_to_fd(handle) {
            Some(fd) => fd,
            None => {
                self.last_error = error::INVALID_HANDLE;
                return u32::MAX;
            }
        };

        let distance = if let Some(ref high) = distance_to_move_high {
            ((**high as i64) << 32) | (distance_to_move as u32 as i64)
        } else {
            distance_to_move as i64
        };

        let whence = match move_method {
            move_method::FILE_BEGIN => 0,   // SEEK_SET
            move_method::FILE_CURRENT => 1, // SEEK_CUR
            move_method::FILE_END => 2,     // SEEK_END
            _ => 0,
        };

        let result = crate::syscall::sys_lseek(fd, distance, whence as i32);

        if let Some(high) = distance_to_move_high {
            *high = (result >> 32) as i32;
        }

        self.last_error = error::SUCCESS;
        result as u32
    }

    /// SetFilePointerEx
    pub fn set_file_pointer_ex(
        &mut self,
        handle: u64,
        distance_to_move: i64,
        new_file_pointer: Option<&mut i64>,
        move_method: u32,
    ) -> bool {
        let fd = match self.translate_handle_to_fd(handle) {
            Some(fd) => fd,
            None => {
                self.last_error = error::INVALID_HANDLE;
                return false;
            }
        };

        let whence = match move_method {
            move_method::FILE_BEGIN => 0,
            move_method::FILE_CURRENT => 1,
            move_method::FILE_END => 2,
            _ => 0,
        };

        let result = crate::syscall::sys_lseek(fd, distance_to_move, whence as i32);

        if let Some(ptr) = new_file_pointer {
            *ptr = result as i64;
        }

        self.last_error = error::SUCCESS;
        true
    }

    /// GetFileType
    pub fn get_file_type(&self, handle: u64) -> u32 {
        match self.handles.get(&handle) {
            Some(HandleInfo::Console { .. }) => file_type::FILE_TYPE_CHAR,
            Some(HandleInfo::File { .. }) => file_type::FILE_TYPE_DISK,
            _ => file_type::FILE_TYPE_UNKNOWN,
        }
    }

    /// FlushFileBuffers
    pub fn flush_file_buffers(&mut self, handle: u64) -> bool {
        let fd = match self.translate_handle_to_fd(handle) {
            Some(fd) => fd,
            None => {
                self.last_error = error::INVALID_HANDLE;
                return false;
            }
        };

        crate::syscall::sys_fsync(fd);
        self.last_error = error::SUCCESS;
        true
    }

    /// DeleteFileA
    pub fn delete_file_a(&mut self, filename: &str) -> bool {
        let unix_path = fs_translate::windows_to_unix(filename);
        let result = crate::syscall::sys_unlink(unix_path.as_ptr() as u64);

        if result != 0 {
            self.last_error = error::FILE_NOT_FOUND;
            false
        } else {
            self.last_error = error::SUCCESS;
            true
        }
    }

    /// MoveFileA
    pub fn move_file_a(&mut self, existing: &str, new: &str) -> bool {
        let old_path = fs_translate::windows_to_unix(existing);
        let new_path = fs_translate::windows_to_unix(new);

        let result = crate::syscall::sys_rename(
            old_path.as_ptr() as u64,
            new_path.as_ptr() as u64
        );

        if result != 0 {
            self.last_error = error::FILE_NOT_FOUND;
            false
        } else {
            self.last_error = error::SUCCESS;
            true
        }
    }

    /// CreateDirectoryA
    pub fn create_directory_a(&mut self, path: &str, security_attributes: u64) -> bool {
        let unix_path = fs_translate::windows_to_unix(path);
        let result = crate::syscall::sys_mkdir(unix_path.as_ptr() as u64, 0o755);

        if result != 0 {
            self.last_error = error::ALREADY_EXISTS;
            false
        } else {
            self.last_error = error::SUCCESS;
            true
        }
    }

    /// RemoveDirectoryA
    pub fn remove_directory_a(&mut self, path: &str) -> bool {
        let unix_path = fs_translate::windows_to_unix(path);
        let result = crate::syscall::sys_rmdir(unix_path.as_ptr() as u64);

        if result != 0 {
            self.last_error = error::PATH_NOT_FOUND;
            false
        } else {
            self.last_error = error::SUCCESS;
            true
        }
    }

    // =========================================================================
    // Memory Functions
    // =========================================================================

    /// VirtualAlloc
    pub fn virtual_alloc(
        &mut self,
        address: u64,
        size: usize,
        allocation_type: u32,
        protect: u32,
    ) -> u64 {
        // Map protection to POSIX
        let prot = self.win_protect_to_posix(protect);
        let flags = 0x22i32; // MAP_ANONYMOUS | MAP_PRIVATE

        let result = crate::syscall::sys_mmap(address, size, prot as i32, flags, -1, 0);

        if result < 0x1000 {
            self.last_error = error::NOT_ENOUGH_MEMORY;
            0
        } else {
            self.last_error = error::SUCCESS;
            result as u64
        }
    }

    /// VirtualFree
    pub fn virtual_free(&mut self, address: u64, size: usize, free_type: u32) -> bool {
        if free_type & mem::MEM_RELEASE != 0 {
            let result = crate::syscall::sys_munmap(address, size);
            if result != 0 {
                self.last_error = error::INVALID_PARAMETER;
                return false;
            }
        }

        self.last_error = error::SUCCESS;
        true
    }

    /// VirtualProtect
    pub fn virtual_protect(
        &mut self,
        address: u64,
        size: usize,
        new_protect: u32,
        old_protect: &mut u32,
    ) -> bool {
        let prot = self.win_protect_to_posix(new_protect);
        *old_protect = protect::PAGE_READWRITE; // Assume previous was RW

        let result = crate::syscall::sys_mprotect(address, size, prot as i32);

        if result != 0 {
            self.last_error = error::INVALID_PARAMETER;
            false
        } else {
            self.last_error = error::SUCCESS;
            true
        }
    }

    fn win_protect_to_posix(&self, protect: u32) -> u32 {
        match protect & 0xFF {
            protect::PAGE_NOACCESS => 0,
            protect::PAGE_READONLY => 1,
            protect::PAGE_READWRITE | protect::PAGE_WRITECOPY => 3,
            protect::PAGE_EXECUTE => 4,
            protect::PAGE_EXECUTE_READ => 5,
            protect::PAGE_EXECUTE_READWRITE | protect::PAGE_EXECUTE_WRITECOPY => 7,
            _ => 3,
        }
    }

    /// GetProcessHeap
    pub fn get_process_heap(&self) -> u64 {
        self.process_heap
    }

    /// HeapCreate
    pub fn heap_create(&mut self, options: u32, initial_size: usize, maximum_size: usize) -> u64 {
        let handle = self.next_heap_handle;
        self.next_heap_handle += 0x10000;

        let size = if maximum_size == 0 { 0x100000 } else { maximum_size };

        self.heaps.insert(handle, HeapInfo {
            base: handle,
            size,
            flags: options,
            allocations: BTreeMap::new(),
            next_alloc: handle + 0x1000,
        });

        self.last_error = error::SUCCESS;
        handle
    }

    /// HeapDestroy
    pub fn heap_destroy(&mut self, heap: u64) -> bool {
        if heap == self.process_heap {
            self.last_error = error::INVALID_PARAMETER;
            return false;
        }

        if self.heaps.remove(&heap).is_some() {
            self.last_error = error::SUCCESS;
            true
        } else {
            self.last_error = error::INVALID_HANDLE;
            false
        }
    }

    /// HeapAlloc
    pub fn heap_alloc(&mut self, heap: u64, flags: u32, size: usize) -> u64 {
        let heap_info = match self.heaps.get_mut(&heap) {
            Some(h) => h,
            None => {
                self.last_error = error::INVALID_HANDLE;
                return 0;
            }
        };

        // Align size to 16 bytes
        let aligned_size = (size + 15) & !15;
        let alloc_addr = heap_info.next_alloc;
        heap_info.next_alloc += aligned_size as u64;
        heap_info.allocations.insert(alloc_addr, aligned_size);

        if flags & heap_flags::HEAP_ZERO_MEMORY != 0 {
            // Zero the memory (would need actual memory access)
        }

        self.last_error = error::SUCCESS;
        alloc_addr
    }

    /// HeapReAlloc
    pub fn heap_realloc(&mut self, heap: u64, flags: u32, ptr: u64, size: usize) -> u64 {
        // Simple implementation: allocate new, copy, free old
        let new_ptr = self.heap_alloc(heap, flags, size);
        if new_ptr != 0 && ptr != 0 {
            // Would copy data here
            self.heap_free(heap, 0, ptr);
        }
        new_ptr
    }

    /// HeapFree
    pub fn heap_free(&mut self, heap: u64, flags: u32, ptr: u64) -> bool {
        let heap_info = match self.heaps.get_mut(&heap) {
            Some(h) => h,
            None => {
                self.last_error = error::INVALID_HANDLE;
                return false;
            }
        };

        if heap_info.allocations.remove(&ptr).is_some() {
            self.last_error = error::SUCCESS;
            true
        } else {
            self.last_error = error::INVALID_PARAMETER;
            false
        }
    }

    /// HeapSize
    pub fn heap_size(&self, heap: u64, flags: u32, ptr: u64) -> usize {
        match self.heaps.get(&heap) {
            Some(h) => h.allocations.get(&ptr).copied().unwrap_or(usize::MAX),
            None => usize::MAX,
        }
    }

    /// GlobalAlloc
    pub fn global_alloc(&mut self, flags: u32, size: usize) -> u64 {
        self.heap_alloc(self.process_heap, flags & heap_flags::HEAP_ZERO_MEMORY, size)
    }

    /// GlobalFree
    pub fn global_free(&mut self, ptr: u64) -> u64 {
        if self.heap_free(self.process_heap, 0, ptr) {
            0
        } else {
            ptr
        }
    }

    /// LocalAlloc
    pub fn local_alloc(&mut self, flags: u32, size: usize) -> u64 {
        self.global_alloc(flags, size)
    }

    /// LocalFree
    pub fn local_free(&mut self, ptr: u64) -> u64 {
        self.global_free(ptr)
    }

    // =========================================================================
    // Process/Thread Functions
    // =========================================================================

    /// GetCurrentProcess
    pub fn get_current_process(&self) -> u64 {
        u64::MAX // Pseudo-handle for current process
    }

    /// GetCurrentProcessId
    pub fn get_current_process_id(&self) -> u32 {
        let current = crate::sched::current_task();
        current.id() as u32
    }

    /// GetCurrentThread
    pub fn get_current_thread(&self) -> u64 {
        u64::MAX - 1 // Pseudo-handle for current thread
    }

    /// GetCurrentThreadId
    pub fn get_current_thread_id(&self) -> u32 {
        self.get_current_process_id()
    }

    /// ExitProcess
    pub fn exit_process(&self, exit_code: u32) -> ! {
        crate::syscall::sys_exit(exit_code as u64);
    }

    /// TerminateProcess
    pub fn terminate_process(&mut self, process: u64, exit_code: u32) -> bool {
        if process == u64::MAX || process == self.get_current_process() {
            crate::syscall::sys_exit(exit_code as u64);
        }
        self.last_error = error::SUCCESS;
        true
    }

    /// ExitThread
    pub fn exit_thread(&self, exit_code: u32) -> ! {
        crate::syscall::sys_exit(exit_code as u64);
    }

    /// Sleep
    pub fn sleep(&self, milliseconds: u32) {
        if milliseconds == 0 {
            crate::syscall::sys_sched_yield();
            return;
        }

        let nanos = (milliseconds as u64) * 1_000_000;
        let secs = nanos / 1_000_000_000;
        let nsecs = nanos % 1_000_000_000;

        let ts = [secs, nsecs];
        crate::syscall::sys_nanosleep(ts.as_ptr() as u64, 0);
    }

    /// SleepEx
    pub fn sleep_ex(&self, milliseconds: u32, alertable: bool) -> u32 {
        self.sleep(milliseconds);
        0 // Return 0 (not WAIT_IO_COMPLETION)
    }

    /// WaitForSingleObject
    pub fn wait_for_single_object(&mut self, handle: u64, milliseconds: u32) -> u32 {
        if !self.handles.contains_key(&handle) {
            self.last_error = error::INVALID_HANDLE;
            return WAIT_FAILED;
        }

        if milliseconds == 0 {
            // Non-blocking check
            crate::syscall::sys_sched_yield();
            return WAIT_OBJECT_0;
        }

        if milliseconds != INFINITE {
            self.sleep(milliseconds);
        }

        WAIT_OBJECT_0
    }

    /// WaitForMultipleObjects
    pub fn wait_for_multiple_objects(
        &mut self,
        count: u32,
        handles: &[u64],
        wait_all: bool,
        milliseconds: u32,
    ) -> u32 {
        if count == 0 || count > 64 {
            self.last_error = error::INVALID_PARAMETER;
            return WAIT_FAILED;
        }

        // Simplified: just wait for timeout and return first
        if milliseconds != INFINITE && milliseconds != 0 {
            self.sleep(milliseconds);
        }

        WAIT_OBJECT_0
    }

    /// CreateEventA
    pub fn create_event_a(
        &mut self,
        security_attributes: u64,
        manual_reset: bool,
        initial_state: bool,
        name: Option<&str>,
    ) -> u64 {
        let handle = self.alloc_handle(HandleInfo::Event {
            signaled: initial_state,
            manual_reset,
        });

        self.last_error = error::SUCCESS;
        handle
    }

    /// SetEvent
    pub fn set_event(&mut self, handle: u64) -> bool {
        if let Some(HandleInfo::Event { signaled, .. }) = self.handles.get_mut(&handle) {
            *signaled = true;
            self.last_error = error::SUCCESS;
            true
        } else {
            self.last_error = error::INVALID_HANDLE;
            false
        }
    }

    /// ResetEvent
    pub fn reset_event(&mut self, handle: u64) -> bool {
        if let Some(HandleInfo::Event { signaled, .. }) = self.handles.get_mut(&handle) {
            *signaled = false;
            self.last_error = error::SUCCESS;
            true
        } else {
            self.last_error = error::INVALID_HANDLE;
            false
        }
    }

    /// CreateMutexA
    pub fn create_mutex_a(
        &mut self,
        security_attributes: u64,
        initial_owner: bool,
        name: Option<&str>,
    ) -> u64 {
        let owner = if initial_owner {
            Some(self.get_current_thread_id())
        } else {
            None
        };

        let handle = self.alloc_handle(HandleInfo::Mutex { owner });
        self.last_error = error::SUCCESS;
        handle
    }

    /// ReleaseMutex
    pub fn release_mutex(&mut self, handle: u64) -> bool {
        if let Some(HandleInfo::Mutex { owner }) = self.handles.get_mut(&handle) {
            *owner = None;
            self.last_error = error::SUCCESS;
            true
        } else {
            self.last_error = error::INVALID_HANDLE;
            false
        }
    }

    // =========================================================================
    // TLS Functions
    // =========================================================================

    /// TlsAlloc
    pub fn tls_alloc(&mut self) -> u32 {
        for (i, slot) in self.tls_slots.iter_mut().enumerate() {
            if !*slot {
                *slot = true;
                self.last_error = error::SUCCESS;
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
            self.tls_values[index as usize] = 0;
            self.last_error = error::SUCCESS;
            true
        } else {
            self.last_error = error::INVALID_PARAMETER;
            false
        }
    }

    /// TlsGetValue
    pub fn tls_get_value(&self, index: u32) -> u64 {
        if (index as usize) < self.tls_values.len() {
            self.tls_values[index as usize]
        } else {
            0
        }
    }

    /// TlsSetValue
    pub fn tls_set_value(&mut self, index: u32, value: u64) -> bool {
        if (index as usize) < self.tls_values.len() {
            self.tls_values[index as usize] = value;
            self.last_error = error::SUCCESS;
            true
        } else {
            self.last_error = error::INVALID_PARAMETER;
            false
        }
    }

    // =========================================================================
    // Critical Section Functions
    // =========================================================================

    /// InitializeCriticalSection
    pub fn initialize_critical_section(&mut self, critical_section: u64) {
        self.critical_sections.insert(critical_section, CriticalSection::default());
    }

    /// DeleteCriticalSection
    pub fn delete_critical_section(&mut self, critical_section: u64) {
        self.critical_sections.remove(&critical_section);
    }

    /// EnterCriticalSection
    pub fn enter_critical_section(&mut self, critical_section: u64) {
        let tid = self.get_current_thread_id();
        let cs = self.critical_sections.entry(critical_section)
            .or_insert_with(CriticalSection::default);

        if cs.owner_thread == tid {
            cs.recursion_count += 1;
        } else {
            // Wait for lock (simplified: just take it)
            cs.owner_thread = tid;
            cs.recursion_count = 1;
            cs.lock_count = 0;
        }
    }

    /// LeaveCriticalSection
    pub fn leave_critical_section(&mut self, critical_section: u64) {
        if let Some(cs) = self.critical_sections.get_mut(&critical_section) {
            if cs.recursion_count > 0 {
                cs.recursion_count -= 1;
                if cs.recursion_count == 0 {
                    cs.owner_thread = 0;
                }
            }
        }
    }

    /// TryEnterCriticalSection
    pub fn try_enter_critical_section(&mut self, critical_section: u64) -> bool {
        let tid = self.get_current_thread_id();
        let cs = self.critical_sections.entry(critical_section)
            .or_insert_with(CriticalSection::default);

        if cs.owner_thread == 0 || cs.owner_thread == tid {
            cs.owner_thread = tid;
            cs.recursion_count += 1;
            true
        } else {
            false
        }
    }

    // =========================================================================
    // Environment Functions
    // =========================================================================

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

    /// GetEnvironmentStringsA
    pub fn get_environment_strings_a(&self) -> Vec<u8> {
        let mut result = Vec::new();
        for (k, v) in &self.environment {
            result.extend_from_slice(k.as_bytes());
            result.push(b'=');
            result.extend_from_slice(v.as_bytes());
            result.push(0);
        }
        result.push(0); // Double null terminator
        result
    }

    // =========================================================================
    // Module Functions
    // =========================================================================

    /// GetModuleHandleA
    pub fn get_module_handle_a(&mut self, module_name: Option<&str>) -> u64 {
        if module_name.is_none() {
            self.last_error = error::SUCCESS;
            return 0x0040_0000; // Default base address
        }

        let name = module_name.unwrap().to_lowercase();
        if let Some(&handle) = self.modules.get(&name) {
            self.last_error = error::SUCCESS;
            handle
        } else {
            // Check for built-in DLLs
            let handle = match name.as_str() {
                "kernel32.dll" | "kernel32" => 0x7FF0_0000,
                "ntdll.dll" | "ntdll" => 0x7FFE_0000,
                "user32.dll" | "user32" => 0x7FF1_0000,
                "gdi32.dll" | "gdi32" => 0x7FF2_0000,
                "advapi32.dll" | "advapi32" => 0x7FF3_0000,
                "msvcrt.dll" | "msvcrt" => 0x7FF4_0000,
                _ => {
                    self.last_error = error::MOD_NOT_FOUND;
                    return 0;
                }
            };
            self.modules.insert(name, handle);
            self.last_error = error::SUCCESS;
            handle
        }
    }

    /// GetModuleFileNameA
    pub fn get_module_file_name_a(
        &mut self,
        module: u64,
        buffer: &mut [u8],
    ) -> u32 {
        let path = if module == 0 || module == 0x0040_0000 {
            "C:\\Program.exe"
        } else {
            // Find module name
            match module {
                0x7FF0_0000 => "C:\\Windows\\System32\\kernel32.dll",
                0x7FFE_0000 => "C:\\Windows\\System32\\ntdll.dll",
                0x7FF1_0000 => "C:\\Windows\\System32\\user32.dll",
                0x7FF2_0000 => "C:\\Windows\\System32\\gdi32.dll",
                0x7FF3_0000 => "C:\\Windows\\System32\\advapi32.dll",
                0x7FF4_0000 => "C:\\Windows\\System32\\msvcrt.dll",
                _ => {
                    self.last_error = error::MOD_NOT_FOUND;
                    return 0;
                }
            }
        };

        let bytes = path.as_bytes();
        if bytes.len() < buffer.len() {
            buffer[..bytes.len()].copy_from_slice(bytes);
            buffer[bytes.len()] = 0;
            self.last_error = error::SUCCESS;
            bytes.len() as u32
        } else {
            self.last_error = error::INSUFFICIENT_BUFFER;
            0
        }
    }

    /// LoadLibraryA
    pub fn load_library_a(&mut self, filename: &str) -> u64 {
        self.get_module_handle_a(Some(filename))
    }

    /// GetProcAddress
    pub fn get_proc_address(&mut self, module: u64, proc_name: &str) -> u64 {
        let exports = match module {
            0x7FF0_0000 => get_exports(),
            0x7FFE_0000 => super::ntdll::get_exports(),
            0x7FF1_0000 => super::user32::get_exports(),
            0x7FF2_0000 => super::gdi32::get_exports(),
            0x7FF3_0000 => super::advapi32::get_exports(),
            0x7FF4_0000 => super::msvcrt::get_exports(),
            _ => {
                self.last_error = error::MOD_NOT_FOUND;
                return 0;
            }
        };

        if let Some(&addr) = exports.get(proc_name) {
            self.last_error = error::SUCCESS;
            addr
        } else {
            self.last_error = error::PROC_NOT_FOUND;
            0
        }
    }

    /// FreeLibrary
    pub fn free_library(&mut self, module: u64) -> bool {
        self.last_error = error::SUCCESS;
        true
    }

    // =========================================================================
    // System Info Functions
    // =========================================================================

    /// GetSystemInfo
    pub fn get_system_info(&self, info: &mut SystemInfo) {
        info.processor_architecture = 9; // AMD64
        info.reserved = 0;
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

    /// GetVersion
    pub fn get_version(&self) -> u32 {
        // Windows 10 version
        0x0A00_0000 | (10 << 8) | 0
    }

    /// GetVersionExA
    pub fn get_version_ex_a(&self, info: &mut OsVersionInfoA) -> bool {
        info.major_version = 10;
        info.minor_version = 0;
        info.build_number = 19041;
        info.platform_id = 2; // VER_PLATFORM_WIN32_NT

        let sp = b"";
        let len = sp.len().min(127);
        info.sz_csd_version[..len].copy_from_slice(&sp[..len]);
        info.sz_csd_version[len] = 0;

        true
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

    /// GetSystemTime
    pub fn get_system_time(&self, time: &mut SystemTime) {
        let ticks = crate::time::ticks();
        // Very simplified - just return epoch-ish time
        time.year = 2024;
        time.month = 1;
        time.day_of_week = 0;
        time.day = 1;
        time.hour = ((ticks / 3600000) % 24) as u16;
        time.minute = ((ticks / 60000) % 60) as u16;
        time.second = ((ticks / 1000) % 60) as u16;
        time.milliseconds = (ticks % 1000) as u16;
    }

    /// GetLocalTime
    pub fn get_local_time(&self, time: &mut SystemTime) {
        self.get_system_time(time);
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

    /// GetCommandLineA
    pub fn get_command_line_a(&self) -> &str {
        &self.command_line
    }

    /// SetCommandLine (internal)
    pub fn set_command_line(&mut self, cmd: &str) {
        self.command_line = String::from(cmd);
    }

    // =========================================================================
    // String Functions
    // =========================================================================

    /// lstrlenA
    pub fn lstrlen_a(s: u64) -> i32 {
        if s == 0 {
            return 0;
        }
        let mut len = 0i32;
        unsafe {
            while *((s + len as u64) as *const u8) != 0 {
                len += 1;
            }
        }
        len
    }

    /// lstrlenW
    pub fn lstrlen_w(s: u64) -> i32 {
        if s == 0 {
            return 0;
        }
        let mut len = 0i32;
        unsafe {
            while *((s + len as u64 * 2) as *const u16) != 0 {
                len += 1;
            }
        }
        len
    }

    // =========================================================================
    // Interlocked Functions
    // =========================================================================

    /// InterlockedIncrement
    pub fn interlocked_increment(addend: &mut i32) -> i32 {
        *addend += 1;
        *addend
    }

    /// InterlockedDecrement
    pub fn interlocked_decrement(addend: &mut i32) -> i32 {
        *addend -= 1;
        *addend
    }

    /// InterlockedExchange
    pub fn interlocked_exchange(target: &mut i32, value: i32) -> i32 {
        let old = *target;
        *target = value;
        old
    }

    /// InterlockedCompareExchange
    pub fn interlocked_compare_exchange(
        destination: &mut i32,
        exchange: i32,
        comparand: i32,
    ) -> i32 {
        let old = *destination;
        if old == comparand {
            *destination = exchange;
        }
        old
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

/// OS Version Info
#[repr(C)]
#[derive(Debug)]
pub struct OsVersionInfoA {
    pub os_version_info_size: u32,
    pub major_version: u32,
    pub minor_version: u32,
    pub build_number: u32,
    pub platform_id: u32,
    pub sz_csd_version: [u8; 128],
}

impl Default for OsVersionInfoA {
    fn default() -> Self {
        Self {
            os_version_info_size: core::mem::size_of::<Self>() as u32,
            major_version: 0,
            minor_version: 0,
            build_number: 0,
            platform_id: 0,
            sz_csd_version: [0; 128],
        }
    }
}

/// System Time
#[repr(C)]
#[derive(Debug, Default)]
pub struct SystemTime {
    pub year: u16,
    pub month: u16,
    pub day_of_week: u16,
    pub day: u16,
    pub hour: u16,
    pub minute: u16,
    pub second: u16,
    pub milliseconds: u16,
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
        "SetConsoleTitleA",
        "SetConsoleTitleW",
        "GetConsoleTitleA",
        "GetConsoleTitleW",
        "AllocConsole",
        "FreeConsole",
        "AttachConsole",
        "SetConsoleTextAttribute",
        "SetConsoleCursorPosition",
        "GetConsoleScreenBufferInfo",
        "FillConsoleOutputCharacterA",
        "FillConsoleOutputAttribute",

        // File I/O
        "CreateFileA",
        "CreateFileW",
        "ReadFile",
        "ReadFileEx",
        "WriteFile",
        "WriteFileEx",
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
        "MoveFileExA",
        "MoveFileExW",
        "CopyFileA",
        "CopyFileW",
        "CopyFileExA",
        "CopyFileExW",
        "GetFileAttributesA",
        "GetFileAttributesW",
        "GetFileAttributesExA",
        "GetFileAttributesExW",
        "SetFileAttributesA",
        "SetFileAttributesW",
        "GetFileTime",
        "SetFileTime",
        "LockFile",
        "LockFileEx",
        "UnlockFile",
        "UnlockFileEx",
        "SetEndOfFile",
        "GetFileInformationByHandle",
        "CreateHardLinkA",
        "CreateHardLinkW",

        // Directory
        "CreateDirectoryA",
        "CreateDirectoryW",
        "CreateDirectoryExA",
        "CreateDirectoryExW",
        "RemoveDirectoryA",
        "RemoveDirectoryW",
        "GetCurrentDirectoryA",
        "GetCurrentDirectoryW",
        "SetCurrentDirectoryA",
        "SetCurrentDirectoryW",
        "FindFirstFileA",
        "FindFirstFileW",
        "FindFirstFileExA",
        "FindFirstFileExW",
        "FindNextFileA",
        "FindNextFileW",
        "FindClose",
        "GetFullPathNameA",
        "GetFullPathNameW",
        "GetLongPathNameA",
        "GetLongPathNameW",
        "GetShortPathNameA",
        "GetShortPathNameW",
        "GetTempPathA",
        "GetTempPathW",
        "GetTempFileNameA",
        "GetTempFileNameW",

        // Memory
        "VirtualAlloc",
        "VirtualAllocEx",
        "VirtualFree",
        "VirtualFreeEx",
        "VirtualProtect",
        "VirtualProtectEx",
        "VirtualQuery",
        "VirtualQueryEx",
        "VirtualLock",
        "VirtualUnlock",
        "GetProcessHeap",
        "GetProcessHeaps",
        "HeapCreate",
        "HeapDestroy",
        "HeapAlloc",
        "HeapReAlloc",
        "HeapFree",
        "HeapSize",
        "HeapCompact",
        "HeapValidate",
        "HeapLock",
        "HeapUnlock",
        "HeapWalk",
        "GlobalAlloc",
        "GlobalReAlloc",
        "GlobalFree",
        "GlobalLock",
        "GlobalUnlock",
        "GlobalSize",
        "GlobalFlags",
        "GlobalHandle",
        "LocalAlloc",
        "LocalReAlloc",
        "LocalFree",
        "LocalLock",
        "LocalUnlock",
        "LocalSize",
        "LocalFlags",
        "LocalHandle",
        "IsBadReadPtr",
        "IsBadWritePtr",
        "IsBadCodePtr",
        "IsBadStringPtrA",
        "IsBadStringPtrW",

        // Process/Thread
        "GetCurrentProcess",
        "GetCurrentProcessId",
        "GetCurrentThread",
        "GetCurrentThreadId",
        "OpenProcess",
        "OpenThread",
        "ExitProcess",
        "TerminateProcess",
        "GetExitCodeProcess",
        "CreateThread",
        "CreateRemoteThread",
        "ExitThread",
        "TerminateThread",
        "GetExitCodeThread",
        "GetThreadId",
        "GetProcessId",
        "SuspendThread",
        "ResumeThread",
        "SwitchToThread",
        "Sleep",
        "SleepEx",
        "WaitForSingleObject",
        "WaitForSingleObjectEx",
        "WaitForMultipleObjects",
        "WaitForMultipleObjectsEx",
        "SignalObjectAndWait",
        "CreateEventA",
        "CreateEventW",
        "CreateEventExA",
        "CreateEventExW",
        "OpenEventA",
        "OpenEventW",
        "SetEvent",
        "ResetEvent",
        "PulseEvent",
        "CreateMutexA",
        "CreateMutexW",
        "CreateMutexExA",
        "CreateMutexExW",
        "OpenMutexA",
        "OpenMutexW",
        "ReleaseMutex",
        "CreateSemaphoreA",
        "CreateSemaphoreW",
        "CreateSemaphoreExA",
        "CreateSemaphoreExW",
        "OpenSemaphoreA",
        "OpenSemaphoreW",
        "ReleaseSemaphore",
        "CreateWaitableTimerA",
        "CreateWaitableTimerW",
        "CreateWaitableTimerExA",
        "CreateWaitableTimerExW",
        "OpenWaitableTimerA",
        "OpenWaitableTimerW",
        "SetWaitableTimer",
        "CancelWaitableTimer",

        // TLS
        "TlsAlloc",
        "TlsFree",
        "TlsGetValue",
        "TlsSetValue",
        "FlsAlloc",
        "FlsFree",
        "FlsGetValue",
        "FlsSetValue",

        // Critical Section
        "InitializeCriticalSection",
        "InitializeCriticalSectionAndSpinCount",
        "InitializeCriticalSectionEx",
        "DeleteCriticalSection",
        "EnterCriticalSection",
        "TryEnterCriticalSection",
        "LeaveCriticalSection",
        "SetCriticalSectionSpinCount",

        // Slim Reader/Writer Lock
        "InitializeSRWLock",
        "AcquireSRWLockExclusive",
        "AcquireSRWLockShared",
        "ReleaseSRWLockExclusive",
        "ReleaseSRWLockShared",
        "TryAcquireSRWLockExclusive",
        "TryAcquireSRWLockShared",

        // Condition Variable
        "InitializeConditionVariable",
        "WakeConditionVariable",
        "WakeAllConditionVariable",
        "SleepConditionVariableCS",
        "SleepConditionVariableSRW",

        // Environment
        "GetEnvironmentVariableA",
        "GetEnvironmentVariableW",
        "SetEnvironmentVariableA",
        "SetEnvironmentVariableW",
        "GetEnvironmentStringsA",
        "GetEnvironmentStringsW",
        "FreeEnvironmentStringsA",
        "FreeEnvironmentStringsW",
        "ExpandEnvironmentStringsA",
        "ExpandEnvironmentStringsW",

        // Module
        "GetModuleHandleA",
        "GetModuleHandleW",
        "GetModuleHandleExA",
        "GetModuleHandleExW",
        "GetModuleFileNameA",
        "GetModuleFileNameW",
        "LoadLibraryA",
        "LoadLibraryW",
        "LoadLibraryExA",
        "LoadLibraryExW",
        "GetProcAddress",
        "FreeLibrary",
        "FreeLibraryAndExitThread",
        "DisableThreadLibraryCalls",

        // System Info
        "GetSystemInfo",
        "GetNativeSystemInfo",
        "IsProcessorFeaturePresent",
        "GetVersionExA",
        "GetVersionExW",
        "GetVersion",
        "VerifyVersionInfoA",
        "VerifyVersionInfoW",
        "GetTickCount",
        "GetTickCount64",
        "QueryPerformanceCounter",
        "QueryPerformanceFrequency",
        "GetSystemTime",
        "GetLocalTime",
        "SetSystemTime",
        "SetLocalTime",
        "GetSystemTimeAsFileTime",
        "SystemTimeToFileTime",
        "FileTimeToSystemTime",
        "FileTimeToLocalFileTime",
        "LocalFileTimeToFileTime",
        "CompareFileTime",
        "GetTimeZoneInformation",
        "SetTimeZoneInformation",

        // Command Line
        "GetCommandLineA",
        "GetCommandLineW",

        // Error
        "GetLastError",
        "SetLastError",
        "FormatMessageA",
        "FormatMessageW",

        // String
        "lstrlenA",
        "lstrlenW",
        "lstrcpyA",
        "lstrcpyW",
        "lstrcpynA",
        "lstrcpynW",
        "lstrcatA",
        "lstrcatW",
        "lstrcmpA",
        "lstrcmpW",
        "lstrcmpiA",
        "lstrcmpiW",
        "CompareStringA",
        "CompareStringW",
        "CompareStringOrdinal",
        "MultiByteToWideChar",
        "WideCharToMultiByte",

        // Interlocked
        "InterlockedIncrement",
        "InterlockedDecrement",
        "InterlockedExchange",
        "InterlockedExchangeAdd",
        "InterlockedCompareExchange",
        "InterlockedExchange64",
        "InterlockedCompareExchange64",
        "InterlockedExchangePointer",
        "InterlockedCompareExchangePointer",

        // Atom
        "GlobalAddAtomA",
        "GlobalAddAtomW",
        "GlobalFindAtomA",
        "GlobalFindAtomW",
        "GlobalGetAtomNameA",
        "GlobalGetAtomNameW",
        "GlobalDeleteAtom",
        "AddAtomA",
        "AddAtomW",
        "FindAtomA",
        "FindAtomW",
        "GetAtomNameA",
        "GetAtomNameW",
        "DeleteAtom",

        // Exception
        "RaiseException",
        "UnhandledExceptionFilter",
        "SetUnhandledExceptionFilter",
        "SetErrorMode",
        "GetErrorMode",

        // Debug
        "IsDebuggerPresent",
        "CheckRemoteDebuggerPresent",
        "OutputDebugStringA",
        "OutputDebugStringW",
        "DebugBreak",

        // Misc
        "Beep",
        "GetComputerNameA",
        "GetComputerNameW",
        "SetComputerNameA",
        "SetComputerNameW",
        "GetUserNameA",
        "GetUserNameW",
        "GetLogicalDrives",
        "GetLogicalDriveStringsA",
        "GetLogicalDriveStringsW",
        "GetDriveTypeA",
        "GetDriveTypeW",
        "GetDiskFreeSpaceA",
        "GetDiskFreeSpaceW",
        "GetDiskFreeSpaceExA",
        "GetDiskFreeSpaceExW",
        "GetVolumeInformationA",
        "GetVolumeInformationW",
        "GetSystemDirectoryA",
        "GetSystemDirectoryW",
        "GetWindowsDirectoryA",
        "GetWindowsDirectoryW",
    ];

    let mut addr = 0x7FF0_0000u64;
    for func in funcs {
        exports.insert(String::from(func), addr);
        addr += 16;
    }

    exports
}

/// Initialize KERNEL32 emulation
pub fn init() {
    crate::kprintln!("kernel32: KERNEL32 emulation initialized ({} exports)", get_exports().len());
}
