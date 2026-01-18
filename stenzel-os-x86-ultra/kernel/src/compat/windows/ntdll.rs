//! NTDLL Emulation
//!
//! Emulates Windows NT kernel interface (ntdll.dll).
//! This is the lowest-level Windows API that talks directly to the kernel.
//!
//! Includes:
//! - Nt* functions (syscall wrappers)
//! - Rtl* functions (runtime library)
//! - Ldr* functions (loader functions)
//! - Csr* functions (client-server runtime)

extern crate alloc;

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;

use super::ntsyscall::{self, NtSyscallContext};

/// NT Status codes
pub mod status {
    pub const SUCCESS: u32 = 0x00000000;
    pub const WAIT_0: u32 = 0x00000000;
    pub const WAIT_1: u32 = 0x00000001;
    pub const WAIT_2: u32 = 0x00000002;
    pub const WAIT_3: u32 = 0x00000003;
    pub const WAIT_63: u32 = 0x0000003F;
    pub const ABANDONED_WAIT_0: u32 = 0x00000080;
    pub const USER_APC: u32 = 0x000000C0;
    pub const TIMEOUT: u32 = 0x00000102;
    pub const PENDING: u32 = 0x00000103;
    pub const UNSUCCESSFUL: u32 = 0xC0000001;
    pub const NOT_IMPLEMENTED: u32 = 0xC0000002;
    pub const INVALID_INFO_CLASS: u32 = 0xC0000003;
    pub const INFO_LENGTH_MISMATCH: u32 = 0xC0000004;
    pub const ACCESS_VIOLATION: u32 = 0xC0000005;
    pub const INVALID_HANDLE: u32 = 0xC0000008;
    pub const INVALID_PARAMETER: u32 = 0xC000000D;
    pub const NO_SUCH_FILE: u32 = 0xC000000F;
    pub const END_OF_FILE: u32 = 0xC0000011;
    pub const MORE_PROCESSING_REQUIRED: u32 = 0xC0000016;
    pub const NO_MEMORY: u32 = 0xC0000017;
    pub const CONFLICTING_ADDRESSES: u32 = 0xC0000018;
    pub const NOT_MAPPED_VIEW: u32 = 0xC0000019;
    pub const UNABLE_TO_FREE_VM: u32 = 0xC000001A;
    pub const ACCESS_DENIED: u32 = 0xC0000022;
    pub const BUFFER_TOO_SMALL: u32 = 0xC0000023;
    pub const OBJECT_TYPE_MISMATCH: u32 = 0xC0000024;
    pub const OBJECT_NAME_INVALID: u32 = 0xC0000033;
    pub const OBJECT_NAME_NOT_FOUND: u32 = 0xC0000034;
    pub const OBJECT_NAME_COLLISION: u32 = 0xC0000035;
    pub const OBJECT_PATH_INVALID: u32 = 0xC0000039;
    pub const OBJECT_PATH_NOT_FOUND: u32 = 0xC000003A;
    pub const OBJECT_PATH_SYNTAX_BAD: u32 = 0xC000003B;
    pub const DATA_OVERRUN: u32 = 0xC000003C;
    pub const DATA_LATE_ERROR: u32 = 0xC000003D;
    pub const SHARING_VIOLATION: u32 = 0xC0000043;
    pub const QUOTA_EXCEEDED: u32 = 0xC0000044;
    pub const INSUFFICIENT_RESOURCES: u32 = 0xC000009A;
    pub const FILE_IS_A_DIRECTORY: u32 = 0xC00000BA;
    pub const NOT_SUPPORTED: u32 = 0xC00000BB;
    pub const DIRECTORY_NOT_EMPTY: u32 = 0xC0000101;
    pub const NOT_A_DIRECTORY: u32 = 0xC0000103;
    pub const CANCELLED: u32 = 0xC0000120;
    pub const NOT_FOUND: u32 = 0xC0000225;

    /// Check if status is success
    pub fn is_success(status: u32) -> bool {
        status < 0x80000000
    }

    /// Check if status is informational
    pub fn is_informational(status: u32) -> bool {
        (status >= 0x40000000) && (status < 0x80000000)
    }

    /// Check if status is warning
    pub fn is_warning(status: u32) -> bool {
        (status >= 0x80000000) && (status < 0xC0000000)
    }

    /// Check if status is error
    pub fn is_error(status: u32) -> bool {
        status >= 0xC0000000
    }
}

/// NT Object Attributes
#[repr(C)]
#[derive(Debug, Clone, Default)]
pub struct ObjectAttributes {
    pub length: u32,
    pub root_directory: u64,
    pub object_name: u64, // Pointer to UNICODE_STRING
    pub attributes: u32,
    pub security_descriptor: u64,
    pub security_quality_of_service: u64,
}

/// Object attribute flags
pub mod oa_flags {
    pub const OBJ_INHERIT: u32 = 0x00000002;
    pub const OBJ_PERMANENT: u32 = 0x00000010;
    pub const OBJ_EXCLUSIVE: u32 = 0x00000020;
    pub const OBJ_CASE_INSENSITIVE: u32 = 0x00000040;
    pub const OBJ_OPENIF: u32 = 0x00000080;
    pub const OBJ_OPENLINK: u32 = 0x00000100;
    pub const OBJ_KERNEL_HANDLE: u32 = 0x00000200;
    pub const OBJ_FORCE_ACCESS_CHECK: u32 = 0x00000400;
}

/// IO Status Block
#[repr(C)]
#[derive(Debug, Clone, Default)]
pub struct IoStatusBlock {
    pub status: u32,
    pub _pad: u32,
    pub information: u64,
}

/// Large Integer (64-bit)
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct LargeInteger {
    pub low_part: u32,
    pub high_part: i32,
}

impl LargeInteger {
    pub fn from_i64(value: i64) -> Self {
        Self {
            low_part: value as u32,
            high_part: (value >> 32) as i32,
        }
    }

    pub fn from_u64(value: u64) -> Self {
        Self {
            low_part: value as u32,
            high_part: (value >> 32) as i32,
        }
    }

    pub fn to_i64(&self) -> i64 {
        ((self.high_part as i64) << 32) | (self.low_part as u64 as i64)
    }

    pub fn to_u64(&self) -> u64 {
        ((self.high_part as u64) << 32) | (self.low_part as u64)
    }
}

/// Unicode String
#[repr(C)]
#[derive(Debug, Clone, Default)]
pub struct UnicodeString {
    pub length: u16,        // Length in bytes (not including null)
    pub maximum_length: u16, // Buffer size in bytes
    pub _pad: u32,
    pub buffer: u64,        // Pointer to wide char buffer
}

impl UnicodeString {
    /// Create from a Rust string
    pub fn from_str(s: &str) -> (Self, Vec<u16>) {
        let utf16: Vec<u16> = s.encode_utf16().collect();
        let len_bytes = (utf16.len() * 2) as u16;
        let us = Self {
            length: len_bytes,
            maximum_length: len_bytes + 2,
            _pad: 0,
            buffer: 0, // Will be set by caller
        };
        (us, utf16)
    }

    /// Convert to Rust string
    pub fn to_string(&self) -> Option<String> {
        if self.buffer == 0 || self.length == 0 {
            return None;
        }

        let char_count = (self.length / 2) as usize;
        let mut utf16 = Vec::with_capacity(char_count);

        unsafe {
            for i in 0..char_count {
                let c = *((self.buffer + i as u64 * 2) as *const u16);
                utf16.push(c);
            }
        }

        String::from_utf16(&utf16).ok()
    }
}

/// ANSI String
#[repr(C)]
#[derive(Debug, Clone, Default)]
pub struct AnsiString {
    pub length: u16,
    pub maximum_length: u16,
    pub _pad: u32,
    pub buffer: u64,
}

/// Client ID (process/thread)
#[repr(C)]
#[derive(Debug, Clone, Default)]
pub struct ClientId {
    pub unique_process: u64,
    pub unique_thread: u64,
}

/// Process Basic Information
#[repr(C)]
#[derive(Debug, Clone, Default)]
pub struct ProcessBasicInformation {
    pub exit_status: u32,
    pub _pad1: u32,
    pub peb_base_address: u64,
    pub affinity_mask: u64,
    pub base_priority: u32,
    pub _pad2: u32,
    pub unique_process_id: u64,
    pub inherited_from_unique_process_id: u64,
}

/// Thread Basic Information
#[repr(C)]
#[derive(Debug, Clone, Default)]
pub struct ThreadBasicInformation {
    pub exit_status: u32,
    pub _pad1: u32,
    pub teb_base_address: u64,
    pub client_id: ClientId,
    pub affinity_mask: u64,
    pub priority: u32,
    pub base_priority: u32,
}

/// System Basic Information
#[repr(C)]
#[derive(Debug, Clone, Default)]
pub struct SystemBasicInformation {
    pub reserved: u32,
    pub timer_resolution: u32,
    pub page_size: u32,
    pub number_of_physical_pages: u32,
    pub lowest_physical_page_number: u32,
    pub highest_physical_page_number: u32,
    pub allocation_granularity: u32,
    pub minimum_user_mode_address: u64,
    pub maximum_user_mode_address: u64,
    pub active_processors_affinity_mask: u64,
    pub number_of_processors: u8,
    pub _pad: [u8; 7],
}

/// Memory Basic Information
#[repr(C)]
#[derive(Debug, Clone, Default)]
pub struct MemoryBasicInformation {
    pub base_address: u64,
    pub allocation_base: u64,
    pub allocation_protect: u32,
    pub _pad1: u32,
    pub region_size: u64,
    pub state: u32,
    pub protect: u32,
    pub mem_type: u32,
    pub _pad2: u32,
}

/// File Basic Information
#[repr(C)]
#[derive(Debug, Clone, Default)]
pub struct FileBasicInformation {
    pub creation_time: LargeInteger,
    pub last_access_time: LargeInteger,
    pub last_write_time: LargeInteger,
    pub change_time: LargeInteger,
    pub file_attributes: u32,
    pub _pad: u32,
}

/// File Standard Information
#[repr(C)]
#[derive(Debug, Clone, Default)]
pub struct FileStandardInformation {
    pub allocation_size: LargeInteger,
    pub end_of_file: LargeInteger,
    pub number_of_links: u32,
    pub delete_pending: u8,
    pub directory: u8,
    pub _pad: [u8; 2],
}

/// NTDLL function implementations
pub struct NtdllEmulator {
    /// Syscall context (lazily initialized to break circular dependency)
    syscall_ctx: Option<Box<NtSyscallContext>>,
    /// Handles mapping
    handles: BTreeMap<u64, HandleInfo>,
    next_handle: u64,
    /// Loaded modules for Ldr functions
    loaded_modules: BTreeMap<String, LoadedModule>,
    /// RTL heap handles
    heaps: BTreeMap<u64, RtlHeap>,
    next_heap: u64,
}

#[derive(Debug, Clone)]
enum HandleInfo {
    File { fd: i32, path: String },
    Process { pid: u32 },
    Thread { tid: u32 },
    Event { signaled: bool, manual_reset: bool },
    Semaphore { count: u32, max: u32 },
    Mutex { owner: Option<u32>, recursive_count: u32 },
    Key { path: String },
    Token { process: u64 },
    Section { size: u64, file: Option<u64> },
}

#[derive(Debug, Clone)]
struct LoadedModule {
    name: String,
    base_address: u64,
    size: u32,
    entry_point: u64,
    ref_count: u32,
}

#[derive(Debug)]
struct RtlHeap {
    base: u64,
    size: usize,
    flags: u32,
    allocations: BTreeMap<u64, usize>, // address -> size
}

impl NtdllEmulator {
    pub fn new() -> Self {
        Self {
            syscall_ctx: None, // Lazily initialized to break circular dependency
            handles: BTreeMap::new(),
            next_handle: 0x100,
            loaded_modules: BTreeMap::new(),
            heaps: BTreeMap::new(),
            next_heap: 0x1000_0000,
        }
    }

    /// Get or create the syscall context
    fn get_syscall_ctx(&mut self) -> &mut NtSyscallContext {
        if self.syscall_ctx.is_none() {
            self.syscall_ctx = Some(Box::new(NtSyscallContext::new()));
        }
        self.syscall_ctx.as_mut().unwrap()
    }

    /// Allocate a new handle
    fn alloc_handle(&mut self, info: HandleInfo) -> u64 {
        let handle = self.next_handle;
        self.next_handle += 4;
        self.handles.insert(handle, info);
        handle
    }

    /// Close a handle
    pub fn close_handle(&mut self, handle: u64) -> u32 {
        if self.handles.remove(&handle).is_some() {
            status::SUCCESS
        } else {
            status::INVALID_HANDLE
        }
    }

    // =========================================================================
    // Nt* Functions (System Call Wrappers)
    // =========================================================================

    /// NtTerminateProcess
    pub fn terminate_process(&mut self, handle: u64, exit_status: u32) -> u32 {
        let args = [handle, exit_status as u64];
        ntsyscall::dispatch_nt_syscall(
            self.get_syscall_ctx(),
            ntsyscall::syscall_nr::NT_TERMINATE_PROCESS,
            &args
        )
    }

    /// NtQueryInformationProcess
    pub fn query_information_process(
        &mut self,
        handle: u64,
        info_class: u32,
        buffer: u64,
        buffer_size: u32,
        return_length: &mut u32,
    ) -> u32 {
        let mut ret_len_storage = 0u32;
        let args = [
            handle,
            info_class as u64,
            buffer,
            buffer_size as u64,
            &mut ret_len_storage as *mut u32 as u64,
        ];
        let result = ntsyscall::dispatch_nt_syscall(
            self.get_syscall_ctx(),
            ntsyscall::syscall_nr::NT_QUERY_INFORMATION_PROCESS,
            &args
        );
        *return_length = ret_len_storage;
        result
    }

    /// NtCreateFile
    pub fn create_file(
        &mut self,
        file_handle: &mut u64,
        desired_access: u32,
        object_attributes: &ObjectAttributes,
        io_status_block: &mut IoStatusBlock,
        allocation_size: Option<&LargeInteger>,
        file_attributes: u32,
        share_access: u32,
        create_disposition: u32,
        create_options: u32,
    ) -> u32 {
        let alloc_size_ptr = allocation_size.map(|a| a as *const LargeInteger as u64).unwrap_or(0);
        let args = [
            file_handle as *mut u64 as u64,
            desired_access as u64,
            object_attributes as *const ObjectAttributes as u64,
            io_status_block as *mut IoStatusBlock as u64,
            alloc_size_ptr,
            file_attributes as u64,
            share_access as u64,
            create_disposition as u64,
            create_options as u64,
        ];
        ntsyscall::dispatch_nt_syscall(
            self.get_syscall_ctx(),
            ntsyscall::syscall_nr::NT_CREATE_FILE,
            &args
        )
    }

    /// NtReadFile
    pub fn read_file(
        &mut self,
        handle: u64,
        event: u64,
        apc_routine: u64,
        apc_context: u64,
        io_status_block: &mut IoStatusBlock,
        buffer: u64,
        length: u32,
        byte_offset: Option<&LargeInteger>,
    ) -> u32 {
        let offset_ptr = byte_offset.map(|o| o as *const LargeInteger as u64).unwrap_or(0);
        let args = [
            handle,
            event,
            apc_routine,
            apc_context,
            io_status_block as *mut IoStatusBlock as u64,
            buffer,
            length as u64,
            offset_ptr,
        ];
        ntsyscall::dispatch_nt_syscall(
            self.get_syscall_ctx(),
            ntsyscall::syscall_nr::NT_READ_FILE,
            &args
        )
    }

    /// NtWriteFile
    pub fn write_file(
        &mut self,
        handle: u64,
        event: u64,
        apc_routine: u64,
        apc_context: u64,
        io_status_block: &mut IoStatusBlock,
        buffer: u64,
        length: u32,
        byte_offset: Option<&LargeInteger>,
    ) -> u32 {
        let offset_ptr = byte_offset.map(|o| o as *const LargeInteger as u64).unwrap_or(0);
        let args = [
            handle,
            event,
            apc_routine,
            apc_context,
            io_status_block as *mut IoStatusBlock as u64,
            buffer,
            length as u64,
            offset_ptr,
        ];
        ntsyscall::dispatch_nt_syscall(
            self.get_syscall_ctx(),
            ntsyscall::syscall_nr::NT_WRITE_FILE,
            &args
        )
    }

    /// NtClose
    pub fn close(&mut self, handle: u64) -> u32 {
        let args = [handle];
        ntsyscall::dispatch_nt_syscall(
            self.get_syscall_ctx(),
            ntsyscall::syscall_nr::NT_CLOSE,
            &args
        )
    }

    /// NtAllocateVirtualMemory
    pub fn allocate_virtual_memory(
        &mut self,
        process_handle: u64,
        base_address: &mut u64,
        zero_bits: u64,
        region_size: &mut u64,
        allocation_type: u32,
        protect: u32,
    ) -> u32 {
        let args = [
            process_handle,
            base_address as *mut u64 as u64,
            zero_bits,
            region_size as *mut u64 as u64,
            allocation_type as u64,
            protect as u64,
        ];
        ntsyscall::dispatch_nt_syscall(
            self.get_syscall_ctx(),
            ntsyscall::syscall_nr::NT_ALLOCATE_VIRTUAL_MEMORY,
            &args
        )
    }

    /// NtFreeVirtualMemory
    pub fn free_virtual_memory(
        &mut self,
        process_handle: u64,
        base_address: &mut u64,
        region_size: &mut u64,
        free_type: u32,
    ) -> u32 {
        let args = [
            process_handle,
            base_address as *mut u64 as u64,
            region_size as *mut u64 as u64,
            free_type as u64,
        ];
        ntsyscall::dispatch_nt_syscall(
            self.get_syscall_ctx(),
            ntsyscall::syscall_nr::NT_FREE_VIRTUAL_MEMORY,
            &args
        )
    }

    /// NtProtectVirtualMemory
    pub fn protect_virtual_memory(
        &mut self,
        process_handle: u64,
        base_address: &mut u64,
        region_size: &mut u64,
        new_protect: u32,
        old_protect: &mut u32,
    ) -> u32 {
        let args = [
            process_handle,
            base_address as *mut u64 as u64,
            region_size as *mut u64 as u64,
            new_protect as u64,
            old_protect as *mut u32 as u64,
        ];
        ntsyscall::dispatch_nt_syscall(
            self.get_syscall_ctx(),
            ntsyscall::syscall_nr::NT_PROTECT_VIRTUAL_MEMORY,
            &args
        )
    }

    /// NtWaitForSingleObject
    pub fn wait_for_single_object(
        &mut self,
        handle: u64,
        alertable: bool,
        timeout: Option<&LargeInteger>,
    ) -> u32 {
        let timeout_ptr = timeout.map(|t| t as *const LargeInteger as u64).unwrap_or(0);
        let args = [
            handle,
            if alertable { 1 } else { 0 },
            timeout_ptr,
        ];
        ntsyscall::dispatch_nt_syscall(
            self.get_syscall_ctx(),
            ntsyscall::syscall_nr::NT_WAIT_FOR_SINGLE_OBJECT,
            &args
        )
    }

    /// NtCreateEvent
    pub fn create_event(
        &mut self,
        event_handle: &mut u64,
        desired_access: u32,
        object_attributes: Option<&ObjectAttributes>,
        event_type: u32,
        initial_state: bool,
    ) -> u32 {
        // Event type: 0 = NotificationEvent (manual reset), 1 = SynchronizationEvent (auto reset)
        let handle = self.alloc_handle(HandleInfo::Event {
            signaled: initial_state,
            manual_reset: event_type == 0,
        });
        *event_handle = handle;
        status::SUCCESS
    }

    /// NtSetEvent
    pub fn set_event(&mut self, event_handle: u64, previous_state: &mut u32) -> u32 {
        if let Some(HandleInfo::Event { signaled, .. }) = self.handles.get_mut(&event_handle) {
            *previous_state = if *signaled { 1 } else { 0 };
            *signaled = true;
            status::SUCCESS
        } else {
            status::INVALID_HANDLE
        }
    }

    /// NtResetEvent
    pub fn reset_event(&mut self, event_handle: u64, previous_state: &mut u32) -> u32 {
        if let Some(HandleInfo::Event { signaled, .. }) = self.handles.get_mut(&event_handle) {
            *previous_state = if *signaled { 1 } else { 0 };
            *signaled = false;
            status::SUCCESS
        } else {
            status::INVALID_HANDLE
        }
    }

    /// NtQuerySystemInformation
    pub fn query_system_information(
        &mut self,
        system_info_class: u32,
        system_info: u64,
        system_info_length: u32,
        return_length: &mut u32,
    ) -> u32 {
        let mut ret_storage = 0u32;
        let args = [
            system_info_class as u64,
            system_info,
            system_info_length as u64,
            &mut ret_storage as *mut u32 as u64,
        ];
        let result = ntsyscall::dispatch_nt_syscall(
            self.get_syscall_ctx(),
            ntsyscall::syscall_nr::NT_QUERY_SYSTEM_INFORMATION,
            &args
        );
        *return_length = ret_storage;
        result
    }

    /// NtQueryPerformanceCounter
    pub fn query_performance_counter(
        &mut self,
        performance_counter: &mut LargeInteger,
        performance_frequency: Option<&mut LargeInteger>,
    ) -> u32 {
        let freq_ptr = performance_frequency
            .map(|f| f as *mut LargeInteger as u64)
            .unwrap_or(0);
        let args = [
            performance_counter as *mut LargeInteger as u64,
            freq_ptr,
        ];
        ntsyscall::dispatch_nt_syscall(
            self.get_syscall_ctx(),
            ntsyscall::syscall_nr::NT_QUERY_PERFORMANCE_COUNTER,
            &args
        )
    }

    /// NtDelayExecution
    pub fn delay_execution(&mut self, alertable: bool, delay_interval: &LargeInteger) -> u32 {
        let args = [
            if alertable { 1 } else { 0 },
            delay_interval as *const LargeInteger as u64,
        ];
        ntsyscall::dispatch_nt_syscall(
            self.get_syscall_ctx(),
            ntsyscall::syscall_nr::NT_DELAY_EXECUTION,
            &args
        )
    }

    // =========================================================================
    // Rtl* Functions (Runtime Library)
    // =========================================================================

    /// RtlInitUnicodeString
    pub fn rtl_init_unicode_string(dest: &mut UnicodeString, source: u64) {
        if source == 0 {
            dest.length = 0;
            dest.maximum_length = 0;
            dest.buffer = 0;
            return;
        }

        // Count characters in null-terminated wide string
        let mut len = 0usize;
        unsafe {
            while *((source + len as u64 * 2) as *const u16) != 0 {
                len += 1;
            }
        }

        dest.length = (len * 2) as u16;
        dest.maximum_length = ((len + 1) * 2) as u16;
        dest.buffer = source;
    }

    /// RtlFreeUnicodeString
    pub fn rtl_free_unicode_string(string: &mut UnicodeString) {
        // In a real implementation, we'd free the buffer if it was allocated
        string.length = 0;
        string.maximum_length = 0;
        string.buffer = 0;
    }

    /// RtlInitAnsiString
    pub fn rtl_init_ansi_string(dest: &mut AnsiString, source: u64) {
        if source == 0 {
            dest.length = 0;
            dest.maximum_length = 0;
            dest.buffer = 0;
            return;
        }

        let mut len = 0usize;
        unsafe {
            while *((source + len as u64) as *const u8) != 0 {
                len += 1;
            }
        }

        dest.length = len as u16;
        dest.maximum_length = (len + 1) as u16;
        dest.buffer = source;
    }

    /// RtlAnsiStringToUnicodeString
    pub fn rtl_ansi_string_to_unicode_string(
        dest: &mut UnicodeString,
        source: &AnsiString,
        alloc_dest: bool,
    ) -> u32 {
        if source.buffer == 0 {
            return status::INVALID_PARAMETER;
        }

        let ansi_len = source.length as usize;
        let unicode_len = ansi_len * 2;

        if alloc_dest {
            // Would need to allocate buffer
            return status::NOT_IMPLEMENTED;
        }

        if dest.maximum_length < (unicode_len + 2) as u16 {
            return status::BUFFER_TOO_SMALL;
        }

        // Convert ANSI to Unicode (simple ASCII extension)
        unsafe {
            for i in 0..ansi_len {
                let c = *((source.buffer + i as u64) as *const u8) as u16;
                *((dest.buffer + i as u64 * 2) as *mut u16) = c;
            }
            *((dest.buffer + ansi_len as u64 * 2) as *mut u16) = 0;
        }

        dest.length = unicode_len as u16;
        status::SUCCESS
    }

    /// RtlUnicodeStringToAnsiString
    pub fn rtl_unicode_string_to_ansi_string(
        dest: &mut AnsiString,
        source: &UnicodeString,
        alloc_dest: bool,
    ) -> u32 {
        if source.buffer == 0 {
            return status::INVALID_PARAMETER;
        }

        let unicode_len = (source.length / 2) as usize;

        if alloc_dest {
            return status::NOT_IMPLEMENTED;
        }

        if dest.maximum_length < (unicode_len + 1) as u16 {
            return status::BUFFER_TOO_SMALL;
        }

        // Convert Unicode to ANSI (simple truncation)
        unsafe {
            for i in 0..unicode_len {
                let c = *((source.buffer + i as u64 * 2) as *const u16);
                *((dest.buffer + i as u64) as *mut u8) = c as u8;
            }
            *((dest.buffer + unicode_len as u64) as *mut u8) = 0;
        }

        dest.length = unicode_len as u16;
        status::SUCCESS
    }

    /// RtlCopyMemory
    pub fn rtl_copy_memory(dest: u64, source: u64, length: usize) {
        unsafe {
            core::ptr::copy_nonoverlapping(
                source as *const u8,
                dest as *mut u8,
                length
            );
        }
    }

    /// RtlMoveMemory
    pub fn rtl_move_memory(dest: u64, source: u64, length: usize) {
        unsafe {
            core::ptr::copy(
                source as *const u8,
                dest as *mut u8,
                length
            );
        }
    }

    /// RtlZeroMemory
    pub fn rtl_zero_memory(dest: u64, length: usize) {
        unsafe {
            core::ptr::write_bytes(dest as *mut u8, 0, length);
        }
    }

    /// RtlFillMemory
    pub fn rtl_fill_memory(dest: u64, length: usize, fill: u8) {
        unsafe {
            core::ptr::write_bytes(dest as *mut u8, fill, length);
        }
    }

    /// RtlCompareMemory
    pub fn rtl_compare_memory(source1: u64, source2: u64, length: usize) -> usize {
        unsafe {
            let s1 = core::slice::from_raw_parts(source1 as *const u8, length);
            let s2 = core::slice::from_raw_parts(source2 as *const u8, length);
            for i in 0..length {
                if s1[i] != s2[i] {
                    return i;
                }
            }
            length
        }
    }

    /// RtlEqualUnicodeString
    pub fn rtl_equal_unicode_string(
        string1: &UnicodeString,
        string2: &UnicodeString,
        case_insensitive: bool,
    ) -> bool {
        if string1.length != string2.length {
            return false;
        }

        let len = (string1.length / 2) as usize;
        unsafe {
            for i in 0..len {
                let mut c1 = *((string1.buffer + i as u64 * 2) as *const u16);
                let mut c2 = *((string2.buffer + i as u64 * 2) as *const u16);

                if case_insensitive {
                    // Simple ASCII case folding
                    if c1 >= 'A' as u16 && c1 <= 'Z' as u16 {
                        c1 += 32;
                    }
                    if c2 >= 'A' as u16 && c2 <= 'Z' as u16 {
                        c2 += 32;
                    }
                }

                if c1 != c2 {
                    return false;
                }
            }
        }
        true
    }

    /// RtlCreateHeap
    pub fn rtl_create_heap(
        &mut self,
        flags: u32,
        heap_base: u64,
        reserve_size: usize,
        commit_size: usize,
    ) -> u64 {
        let base = if heap_base != 0 {
            heap_base
        } else {
            let b = self.next_heap;
            self.next_heap += reserve_size as u64;
            b
        };

        let heap = RtlHeap {
            base,
            size: reserve_size,
            flags,
            allocations: BTreeMap::new(),
        };

        self.heaps.insert(base, heap);
        base
    }

    /// RtlDestroyHeap
    pub fn rtl_destroy_heap(&mut self, heap: u64) -> u64 {
        if self.heaps.remove(&heap).is_some() {
            0 // Success
        } else {
            heap // Failure
        }
    }

    /// RtlAllocateHeap
    pub fn rtl_allocate_heap(&mut self, heap: u64, flags: u32, size: usize) -> u64 {
        let heap_entry = match self.heaps.get_mut(&heap) {
            Some(h) => h,
            None => return 0,
        };

        // Simple bump allocator within heap
        let alloc_base = heap_entry.base + heap_entry.allocations.len() as u64 * 0x1000;
        heap_entry.allocations.insert(alloc_base, size);

        alloc_base
    }

    /// RtlFreeHeap
    pub fn rtl_free_heap(&mut self, heap: u64, flags: u32, ptr: u64) -> bool {
        let heap_entry = match self.heaps.get_mut(&heap) {
            Some(h) => h,
            None => return false,
        };

        heap_entry.allocations.remove(&ptr).is_some()
    }

    /// RtlSizeHeap
    pub fn rtl_size_heap(&self, heap: u64, flags: u32, ptr: u64) -> usize {
        let heap_entry = match self.heaps.get(&heap) {
            Some(h) => h,
            None => return 0,
        };

        heap_entry.allocations.get(&ptr).copied().unwrap_or(0)
    }

    /// RtlGetNtGlobalFlags
    pub fn rtl_get_nt_global_flags() -> u32 {
        0 // No special flags set
    }

    /// RtlNtStatusToDosError
    pub fn rtl_nt_status_to_dos_error(nt_status: u32) -> u32 {
        match nt_status {
            status::SUCCESS => 0, // ERROR_SUCCESS
            status::INVALID_HANDLE => 6, // ERROR_INVALID_HANDLE
            status::INVALID_PARAMETER => 87, // ERROR_INVALID_PARAMETER
            status::NO_MEMORY => 8, // ERROR_NOT_ENOUGH_MEMORY
            status::ACCESS_DENIED => 5, // ERROR_ACCESS_DENIED
            status::OBJECT_NAME_NOT_FOUND => 2, // ERROR_FILE_NOT_FOUND
            status::OBJECT_PATH_NOT_FOUND => 3, // ERROR_PATH_NOT_FOUND
            status::NOT_IMPLEMENTED => 50, // ERROR_NOT_SUPPORTED
            status::BUFFER_TOO_SMALL => 122, // ERROR_INSUFFICIENT_BUFFER
            status::SHARING_VIOLATION => 32, // ERROR_SHARING_VIOLATION
            _ => 1, // ERROR_INVALID_FUNCTION
        }
    }

    // =========================================================================
    // Ldr* Functions (Loader)
    // =========================================================================

    /// LdrLoadDll
    pub fn ldr_load_dll(
        &mut self,
        search_path: u64,
        flags: u32,
        module_name: &UnicodeString,
        module_handle: &mut u64,
    ) -> u32 {
        let name = match module_name.to_string() {
            Some(n) => n.to_lowercase(),
            None => return status::OBJECT_NAME_INVALID,
        };

        // Check if already loaded
        if let Some(module) = self.loaded_modules.get(&name) {
            *module_handle = module.base_address;
            return status::SUCCESS;
        }

        // Check for built-in modules
        let base = match name.as_str() {
            "kernel32.dll" => 0x7FF0_0000u64,
            "ntdll.dll" => 0x7FFE_0000u64,
            "user32.dll" => 0x7FF1_0000u64,
            "gdi32.dll" => 0x7FF2_0000u64,
            "advapi32.dll" => 0x7FF3_0000u64,
            "msvcrt.dll" => 0x7FF4_0000u64,
            _ => {
                crate::kprintln!("ntdll: LdrLoadDll failed for '{}'", name);
                return status::OBJECT_NAME_NOT_FOUND;
            }
        };

        let module = LoadedModule {
            name: name.clone(),
            base_address: base,
            size: 0x10000,
            entry_point: base + 0x1000,
            ref_count: 1,
        };

        self.loaded_modules.insert(name, module);
        *module_handle = base;

        status::SUCCESS
    }

    /// LdrGetProcedureAddress
    pub fn ldr_get_procedure_address(
        &self,
        module_handle: u64,
        function_name: Option<&AnsiString>,
        ordinal: u32,
        function_address: &mut u64,
    ) -> u32 {
        // Find module by base address
        let module = self.loaded_modules.values()
            .find(|m| m.base_address == module_handle);

        if module.is_none() {
            return status::OBJECT_NAME_NOT_FOUND;
        }

        // Get function exports based on module
        let exports = match module_handle {
            0x7FF0_0000 => super::kernel32::get_exports(),
            0x7FFE_0000 => get_exports(),
            0x7FF1_0000 => super::user32::get_exports(),
            0x7FF2_0000 => super::gdi32::get_exports(),
            0x7FF3_0000 => super::advapi32::get_exports(),
            0x7FF4_0000 => super::msvcrt::get_exports(),
            _ => return status::OBJECT_NAME_NOT_FOUND,
        };

        if let Some(func_name) = function_name {
            // Look up by name
            let name = unsafe {
                let len = func_name.length as usize;
                let bytes = core::slice::from_raw_parts(func_name.buffer as *const u8, len);
                core::str::from_utf8_unchecked(bytes)
            };

            if let Some(&addr) = exports.get(name) {
                *function_address = addr;
                return status::SUCCESS;
            }
        } else if ordinal != 0 {
            // Look up by ordinal (simple: use ordinal as index)
            if let Some((&_, &addr)) = exports.iter().nth(ordinal as usize - 1) {
                *function_address = addr;
                return status::SUCCESS;
            }
        }

        status::OBJECT_NAME_NOT_FOUND
    }

    /// LdrUnloadDll
    pub fn ldr_unload_dll(&mut self, module_handle: u64) -> u32 {
        let module_name = self.loaded_modules.iter()
            .find(|(_, m)| m.base_address == module_handle)
            .map(|(n, _)| n.clone());

        if let Some(name) = module_name {
            if let Some(module) = self.loaded_modules.get_mut(&name) {
                module.ref_count = module.ref_count.saturating_sub(1);
                if module.ref_count == 0 {
                    self.loaded_modules.remove(&name);
                }
            }
            status::SUCCESS
        } else {
            status::INVALID_HANDLE
        }
    }

    /// LdrGetDllHandle
    pub fn ldr_get_dll_handle(
        &self,
        _search_path: u64,
        _flags: u32,
        module_name: &UnicodeString,
        module_handle: &mut u64,
    ) -> u32 {
        let name = match module_name.to_string() {
            Some(n) => n.to_lowercase(),
            None => return status::OBJECT_NAME_INVALID,
        };

        if let Some(module) = self.loaded_modules.get(&name) {
            *module_handle = module.base_address;
            status::SUCCESS
        } else {
            status::OBJECT_NAME_NOT_FOUND
        }
    }
}

impl Default for NtdllEmulator {
    fn default() -> Self {
        Self::new()
    }
}

/// Get NTDLL exports for the loader
pub fn get_exports() -> BTreeMap<String, u64> {
    let mut exports = BTreeMap::new();

    let funcs = [
        // Nt* functions
        "NtClose",
        "NtCreateFile",
        "NtOpenFile",
        "NtReadFile",
        "NtWriteFile",
        "NtDeleteFile",
        "NtQueryInformationFile",
        "NtSetInformationFile",
        "NtQueryDirectoryFile",
        "NtFlushBuffersFile",
        "NtLockFile",
        "NtUnlockFile",
        "NtDeviceIoControlFile",
        "NtFsControlFile",
        "NtCreateSection",
        "NtOpenSection",
        "NtMapViewOfSection",
        "NtUnmapViewOfSection",
        "NtQuerySection",
        "NtExtendSection",
        "NtAllocateVirtualMemory",
        "NtFreeVirtualMemory",
        "NtProtectVirtualMemory",
        "NtQueryVirtualMemory",
        "NtReadVirtualMemory",
        "NtWriteVirtualMemory",
        "NtFlushVirtualMemory",
        "NtCreateProcess",
        "NtCreateProcessEx",
        "NtOpenProcess",
        "NtTerminateProcess",
        "NtSuspendProcess",
        "NtResumeProcess",
        "NtQueryInformationProcess",
        "NtSetInformationProcess",
        "NtCreateThread",
        "NtCreateThreadEx",
        "NtOpenThread",
        "NtTerminateThread",
        "NtSuspendThread",
        "NtResumeThread",
        "NtGetContextThread",
        "NtSetContextThread",
        "NtQueryInformationThread",
        "NtSetInformationThread",
        "NtWaitForSingleObject",
        "NtWaitForMultipleObjects",
        "NtSignalAndWaitForSingleObject",
        "NtCreateEvent",
        "NtOpenEvent",
        "NtSetEvent",
        "NtResetEvent",
        "NtPulseEvent",
        "NtClearEvent",
        "NtQueryEvent",
        "NtCreateMutant",
        "NtOpenMutant",
        "NtReleaseMutant",
        "NtQueryMutant",
        "NtCreateSemaphore",
        "NtOpenSemaphore",
        "NtReleaseSemaphore",
        "NtQuerySemaphore",
        "NtCreateTimer",
        "NtOpenTimer",
        "NtSetTimer",
        "NtCancelTimer",
        "NtQueryTimer",
        "NtCreateKey",
        "NtOpenKey",
        "NtOpenKeyEx",
        "NtDeleteKey",
        "NtQueryKey",
        "NtSetValueKey",
        "NtQueryValueKey",
        "NtDeleteValueKey",
        "NtEnumerateKey",
        "NtEnumerateValueKey",
        "NtFlushKey",
        "NtRenameKey",
        "NtQuerySystemInformation",
        "NtSetSystemInformation",
        "NtQuerySystemTime",
        "NtSetSystemTime",
        "NtQueryPerformanceCounter",
        "NtDelayExecution",
        "NtYieldExecution",
        "NtAlertThread",
        "NtAlertResumeThread",
        "NtTestAlert",
        "NtQueueApcThread",
        "NtRaiseException",
        "NtContinue",
        "NtDuplicateObject",
        "NtQueryObject",
        "NtSetInformationObject",
        "NtOpenProcessToken",
        "NtOpenThreadToken",
        "NtAdjustPrivilegesToken",
        "NtQueryInformationToken",
        "NtSetInformationToken",
        "NtDuplicateToken",
        "NtCreateIoCompletion",
        "NtOpenIoCompletion",
        "NtSetIoCompletion",
        "NtRemoveIoCompletion",
        "NtQueryIoCompletion",

        // Rtl* functions
        "RtlInitUnicodeString",
        "RtlFreeUnicodeString",
        "RtlInitAnsiString",
        "RtlFreeAnsiString",
        "RtlAnsiStringToUnicodeString",
        "RtlUnicodeStringToAnsiString",
        "RtlEqualUnicodeString",
        "RtlCompareUnicodeString",
        "RtlCopyUnicodeString",
        "RtlAppendUnicodeStringToString",
        "RtlCopyMemory",
        "RtlMoveMemory",
        "RtlZeroMemory",
        "RtlFillMemory",
        "RtlCompareMemory",
        "RtlCreateHeap",
        "RtlDestroyHeap",
        "RtlAllocateHeap",
        "RtlFreeHeap",
        "RtlSizeHeap",
        "RtlReAllocateHeap",
        "RtlGetProcessHeap",
        "RtlGetNtGlobalFlags",
        "RtlNtStatusToDosError",
        "RtlSetLastWin32Error",
        "RtlGetLastWin32Error",
        "RtlEnterCriticalSection",
        "RtlLeaveCriticalSection",
        "RtlTryEnterCriticalSection",
        "RtlInitializeCriticalSection",
        "RtlDeleteCriticalSection",
        "RtlInitializeSListHead",
        "RtlInterlockedFlushSList",
        "RtlInterlockedPopEntrySList",
        "RtlInterlockedPushEntrySList",
        "RtlQueryDepthSList",
        "RtlGetVersion",
        "RtlVerifyVersionInfo",

        // Ldr* functions
        "LdrLoadDll",
        "LdrUnloadDll",
        "LdrGetDllHandle",
        "LdrGetDllHandleEx",
        "LdrGetProcedureAddress",
        "LdrGetProcedureAddressEx",
        "LdrAddRefDll",
        "LdrLockLoaderLock",
        "LdrUnlockLoaderLock",
        "LdrFindEntryForAddress",
        "LdrEnumerateLoadedModules",
        "LdrQueryProcessModuleInformation",
        "LdrRegisterDllNotification",
        "LdrUnregisterDllNotification",

        // Csr* functions
        "CsrClientCallServer",
        "CsrCaptureMessageBuffer",
        "CsrCaptureMessageString",
        "CsrFreeCaptureBuffer",

        // DbgUi* functions
        "DbgUiConnectToDbg",
        "DbgUiDebugActiveProcess",
        "DbgUiStopDebugging",
        "DbgUiWaitStateChange",
        "DbgUiContinue",
        "DbgPrint",
        "DbgPrintEx",

        // Exception handling
        "RtlRaiseException",
        "RtlUnwind",
        "RtlUnwindEx",
        "RtlVirtualUnwind",
        "RtlCaptureContext",
        "RtlRestoreContext",
        "RtlAddFunctionTable",
        "RtlDeleteFunctionTable",
        "RtlLookupFunctionEntry",
    ];

    let mut addr = 0x7FFE_0000u64;
    for func in funcs {
        exports.insert(String::from(func), addr);
        addr += 16;
    }

    exports
}

/// Initialize NTDLL emulation
pub fn init() {
    crate::kprintln!("ntdll: NTDLL emulation initialized ({} exports)", get_exports().len());
}
