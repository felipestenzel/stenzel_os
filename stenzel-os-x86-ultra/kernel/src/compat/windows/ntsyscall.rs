//! NT Syscall Dispatcher
//!
//! Provides complete Windows NT syscall translation layer.
//! Translates NT syscalls to Stenzel OS syscalls.
//!
//! NT syscalls use the SYSCALL instruction with syscall number in EAX.
//! Arguments are passed in RCX, RDX, R8, R9, and on the stack.

extern crate alloc;

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use core::sync::atomic::{AtomicU64, Ordering};

use super::ntdll::{self, status, IoStatusBlock, LargeInteger, ObjectAttributes, UnicodeString, ClientId};
use super::fs_translate;

/// NT Syscall numbers (Windows 10 21H2 x64)
/// These are the syscall numbers used by ntdll.dll
pub mod syscall_nr {
    // Process
    pub const NT_TERMINATE_PROCESS: u32 = 0x002C;
    pub const NT_CREATE_PROCESS: u32 = 0x00B4;
    pub const NT_CREATE_PROCESS_EX: u32 = 0x004D;
    pub const NT_OPEN_PROCESS: u32 = 0x0026;
    pub const NT_QUERY_INFORMATION_PROCESS: u32 = 0x0019;
    pub const NT_SET_INFORMATION_PROCESS: u32 = 0x001C;
    pub const NT_SUSPEND_PROCESS: u32 = 0x017A;
    pub const NT_RESUME_PROCESS: u32 = 0x0177;

    // Thread
    pub const NT_CREATE_THREAD: u32 = 0x004E;
    pub const NT_CREATE_THREAD_EX: u32 = 0x00C1;
    pub const NT_OPEN_THREAD: u32 = 0x0038;
    pub const NT_TERMINATE_THREAD: u32 = 0x0053;
    pub const NT_SUSPEND_THREAD: u32 = 0x017B;
    pub const NT_RESUME_THREAD: u32 = 0x0052;
    pub const NT_GET_CONTEXT_THREAD: u32 = 0x00F2;
    pub const NT_SET_CONTEXT_THREAD: u32 = 0x018B;
    pub const NT_QUERY_INFORMATION_THREAD: u32 = 0x0025;

    // Memory
    pub const NT_ALLOCATE_VIRTUAL_MEMORY: u32 = 0x0018;
    pub const NT_FREE_VIRTUAL_MEMORY: u32 = 0x001E;
    pub const NT_PROTECT_VIRTUAL_MEMORY: u32 = 0x0050;
    pub const NT_QUERY_VIRTUAL_MEMORY: u32 = 0x0023;
    pub const NT_READ_VIRTUAL_MEMORY: u32 = 0x003F;
    pub const NT_WRITE_VIRTUAL_MEMORY: u32 = 0x003A;
    pub const NT_LOCK_VIRTUAL_MEMORY: u32 = 0x000B;
    pub const NT_UNLOCK_VIRTUAL_MEMORY: u32 = 0x000C;
    pub const NT_FLUSH_VIRTUAL_MEMORY: u32 = 0x0012;

    // File
    pub const NT_CREATE_FILE: u32 = 0x0055;
    pub const NT_OPEN_FILE: u32 = 0x0033;
    pub const NT_READ_FILE: u32 = 0x0006;
    pub const NT_WRITE_FILE: u32 = 0x0008;
    pub const NT_CLOSE: u32 = 0x000F;
    pub const NT_DELETE_FILE: u32 = 0x010D;
    pub const NT_QUERY_INFORMATION_FILE: u32 = 0x0011;
    pub const NT_SET_INFORMATION_FILE: u32 = 0x0027;
    pub const NT_QUERY_DIRECTORY_FILE: u32 = 0x0035;
    pub const NT_FLUSH_BUFFERS_FILE: u32 = 0x004B;
    pub const NT_LOCK_FILE: u32 = 0x010E;
    pub const NT_UNLOCK_FILE: u32 = 0x0100;
    pub const NT_QUERY_VOLUME_INFORMATION_FILE: u32 = 0x0073;
    pub const NT_QUERY_ATTRIBUTES_FILE: u32 = 0x003D;
    pub const NT_QUERY_FULL_ATTRIBUTES_FILE: u32 = 0x015D;
    pub const NT_CREATE_NAMED_PIPE_FILE: u32 = 0x00A0;
    pub const NT_DEVICE_IO_CONTROL_FILE: u32 = 0x0007;
    pub const NT_FS_CONTROL_FILE: u32 = 0x0039;

    // Section (memory mapping)
    pub const NT_CREATE_SECTION: u32 = 0x004A;
    pub const NT_OPEN_SECTION: u32 = 0x0037;
    pub const NT_MAP_VIEW_OF_SECTION: u32 = 0x0028;
    pub const NT_UNMAP_VIEW_OF_SECTION: u32 = 0x002A;
    pub const NT_EXTEND_SECTION: u32 = 0x015C;
    pub const NT_QUERY_SECTION: u32 = 0x0051;

    // Synchronization
    pub const NT_CREATE_EVENT: u32 = 0x0048;
    pub const NT_OPEN_EVENT: u32 = 0x0040;
    pub const NT_SET_EVENT: u32 = 0x000E;
    pub const NT_RESET_EVENT: u32 = 0x017F;
    pub const NT_PULSE_EVENT: u32 = 0x0102;
    pub const NT_CLEAR_EVENT: u32 = 0x00E5;
    pub const NT_QUERY_EVENT: u32 = 0x0056;
    pub const NT_CREATE_MUTANT: u32 = 0x00A1;
    pub const NT_OPEN_MUTANT: u32 = 0x0032;
    pub const NT_RELEASE_MUTANT: u32 = 0x001D;
    pub const NT_QUERY_MUTANT: u32 = 0x006D;
    pub const NT_CREATE_SEMAPHORE: u32 = 0x00CE;
    pub const NT_OPEN_SEMAPHORE: u32 = 0x0069;
    pub const NT_RELEASE_SEMAPHORE: u32 = 0x0010;
    pub const NT_QUERY_SEMAPHORE: u32 = 0x006E;
    pub const NT_CREATE_TIMER: u32 = 0x00B5;
    pub const NT_OPEN_TIMER: u32 = 0x0068;
    pub const NT_SET_TIMER: u32 = 0x01A0;
    pub const NT_CANCEL_TIMER: u32 = 0x00B6;
    pub const NT_WAIT_FOR_SINGLE_OBJECT: u32 = 0x0004;
    pub const NT_WAIT_FOR_MULTIPLE_OBJECTS: u32 = 0x001B;
    pub const NT_SIGNAL_AND_WAIT_FOR_SINGLE_OBJECT: u32 = 0x014C;

    // Registry
    pub const NT_CREATE_KEY: u32 = 0x001D;
    pub const NT_OPEN_KEY: u32 = 0x0012;
    pub const NT_OPEN_KEY_EX: u32 = 0x00F0;
    pub const NT_DELETE_KEY: u32 = 0x003E;
    pub const NT_QUERY_KEY: u32 = 0x0013;
    pub const NT_SET_VALUE_KEY: u32 = 0x0060;
    pub const NT_QUERY_VALUE_KEY: u32 = 0x0017;
    pub const NT_DELETE_VALUE_KEY: u32 = 0x003F;
    pub const NT_ENUMERATE_KEY: u32 = 0x0032;
    pub const NT_ENUMERATE_VALUE_KEY: u32 = 0x0014;
    pub const NT_FLUSH_KEY: u32 = 0x0045;
    pub const NT_RENAME_KEY: u32 = 0x014D;

    // System
    pub const NT_QUERY_SYSTEM_INFORMATION: u32 = 0x0036;
    pub const NT_SET_SYSTEM_INFORMATION: u32 = 0x01A4;
    pub const NT_QUERY_SYSTEM_TIME: u32 = 0x005D;
    pub const NT_SET_SYSTEM_TIME: u32 = 0x01A5;
    pub const NT_QUERY_TIMER_RESOLUTION: u32 = 0x006B;
    pub const NT_SET_TIMER_RESOLUTION: u32 = 0x0146;
    pub const NT_QUERY_PERFORMANCE_COUNTER: u32 = 0x0031;
    pub const NT_DELAY_EXECUTION: u32 = 0x0034;
    pub const NT_YIELD_EXECUTION: u32 = 0x0046;

    // Object
    pub const NT_DUPLICATE_OBJECT: u32 = 0x003C;
    pub const NT_MAKE_TEMPORARY_OBJECT: u32 = 0x0141;
    pub const NT_QUERY_OBJECT: u32 = 0x0010;
    pub const NT_SET_INFORMATION_OBJECT: u32 = 0x005C;
    pub const NT_WAIT_FOR_KEY_CHANGE: u32 = 0x0066;

    // Security
    pub const NT_OPEN_PROCESS_TOKEN: u32 = 0x0123;
    pub const NT_OPEN_THREAD_TOKEN: u32 = 0x0024;
    pub const NT_ADJUST_PRIVILEGES_TOKEN: u32 = 0x0029;
    pub const NT_QUERY_INFORMATION_TOKEN: u32 = 0x0021;
    pub const NT_SET_INFORMATION_TOKEN: u32 = 0x00E7;
    pub const NT_CREATE_TOKEN: u32 = 0x00BB;
    pub const NT_DUPLICATE_TOKEN: u32 = 0x0125;
    pub const NT_COMPARE_TOKENS: u32 = 0x00EE;
    pub const NT_IMPERSONATE_THREAD: u32 = 0x014B;

    // I/O Completion
    pub const NT_CREATE_IO_COMPLETION: u32 = 0x00A6;
    pub const NT_OPEN_IO_COMPLETION: u32 = 0x011C;
    pub const NT_SET_IO_COMPLETION: u32 = 0x019C;
    pub const NT_REMOVE_IO_COMPLETION: u32 = 0x0149;
    pub const NT_QUERY_IO_COMPLETION: u32 = 0x011D;

    // Debug
    pub const NT_DEBUG_ACTIVE_PROCESS: u32 = 0x00B9;
    pub const NT_DEBUG_CONTINUE: u32 = 0x0018;
    pub const NT_REMOVE_PROCESS_DEBUG: u32 = 0x00BA;
    pub const NT_WAIT_FOR_DEBUG_EVENT: u32 = 0x0164;

    // Misc
    pub const NT_RAISE_EXCEPTION: u32 = 0x0093;
    pub const NT_CONTINUE: u32 = 0x0044;
    pub const NT_ALERT_THREAD: u32 = 0x0185;
    pub const NT_ALERT_RESUME_THREAD: u32 = 0x0054;
    pub const NT_TEST_ALERT: u32 = 0x0105;
    pub const NT_QUEUE_APC_THREAD: u32 = 0x0115;
}

/// Statistics for syscall tracking
pub struct NtSyscallStats {
    pub total_calls: AtomicU64,
    pub successful_calls: AtomicU64,
    pub failed_calls: AtomicU64,
    pub unimplemented_calls: AtomicU64,
}

impl NtSyscallStats {
    pub const fn new() -> Self {
        Self {
            total_calls: AtomicU64::new(0),
            successful_calls: AtomicU64::new(0),
            failed_calls: AtomicU64::new(0),
            unimplemented_calls: AtomicU64::new(0),
        }
    }
}

static STATS: NtSyscallStats = NtSyscallStats::new();

/// NT Syscall context - holds state for syscall emulation
pub struct NtSyscallContext {
    /// NTDLL emulator instance (boxed to break circular dependency)
    ntdll: Option<Box<ntdll::NtdllEmulator>>,
    /// Handle table: Windows handle -> Stenzel FD
    handle_to_fd: BTreeMap<u64, i32>,
    /// Reverse mapping: Stenzel FD -> Windows handle
    fd_to_handle: BTreeMap<i32, u64>,
    /// Next handle to allocate
    next_handle: u64,
    /// Section handles (for memory mapping)
    sections: BTreeMap<u64, SectionInfo>,
    /// Pending I/O operations
    pending_io: BTreeMap<u64, PendingIo>,
}

#[derive(Debug, Clone)]
struct SectionInfo {
    size: u64,
    file_handle: Option<u64>,
    protect: u32,
}

#[derive(Debug)]
struct PendingIo {
    handle: u64,
    operation: IoOperation,
}

#[derive(Debug)]
enum IoOperation {
    Read { offset: u64, length: u32 },
    Write { offset: u64, length: u32 },
}

impl NtSyscallContext {
    pub fn new() -> Self {
        let mut ctx = Self {
            ntdll: None, // Will be set after ntdll::NtdllEmulator is created separately
            handle_to_fd: BTreeMap::new(),
            fd_to_handle: BTreeMap::new(),
            next_handle: 0x100, // Start after reserved handles
            sections: BTreeMap::new(),
            pending_io: BTreeMap::new(),
        };

        // Map standard handles
        ctx.register_handle(0x10, 0);  // stdin
        ctx.register_handle(0x14, 1);  // stdout
        ctx.register_handle(0x18, 2);  // stderr

        ctx
    }

    /// Get or create the ntdll emulator
    fn get_ntdll(&mut self) -> &mut ntdll::NtdllEmulator {
        if self.ntdll.is_none() {
            self.ntdll = Some(Box::new(ntdll::NtdllEmulator::new()));
        }
        self.ntdll.as_mut().unwrap()
    }

    fn register_handle(&mut self, win_handle: u64, fd: i32) {
        self.handle_to_fd.insert(win_handle, fd);
        self.fd_to_handle.insert(fd, win_handle);
    }

    fn alloc_handle(&mut self, fd: i32) -> u64 {
        let handle = self.next_handle;
        self.next_handle += 4;
        self.register_handle(handle, fd);
        handle
    }

    fn translate_handle(&self, win_handle: u64) -> Option<i32> {
        // Handle special pseudo-handles
        match win_handle {
            0xFFFFFFFF_FFFFFFFF => Some(-1), // Current process
            0xFFFFFFFF_FFFFFFFE => Some(-2), // Current thread
            _ => self.handle_to_fd.get(&win_handle).copied(),
        }
    }

    fn close_handle(&mut self, win_handle: u64) -> bool {
        if let Some(fd) = self.handle_to_fd.remove(&win_handle) {
            self.fd_to_handle.remove(&fd);
            true
        } else {
            false
        }
    }
}

impl Default for NtSyscallContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Dispatch an NT syscall
///
/// Arguments:
/// - syscall_nr: The NT syscall number (from EAX)
/// - args: Arguments array [RCX, RDX, R8, R9, stack...]
///
/// Returns: NTSTATUS code
pub fn dispatch_nt_syscall(ctx: &mut NtSyscallContext, syscall_nr: u32, args: &[u64]) -> u32 {
    STATS.total_calls.fetch_add(1, Ordering::Relaxed);

    let result = match syscall_nr {
        // === Process ===
        syscall_nr::NT_TERMINATE_PROCESS => {
            let handle = args.get(0).copied().unwrap_or(u64::MAX);
            let exit_status = args.get(1).copied().unwrap_or(0) as u32;
            nt_terminate_process(ctx, handle, exit_status)
        }
        syscall_nr::NT_QUERY_INFORMATION_PROCESS => {
            let handle = args.get(0).copied().unwrap_or(0);
            let info_class = args.get(1).copied().unwrap_or(0) as u32;
            let buffer = args.get(2).copied().unwrap_or(0);
            let buffer_size = args.get(3).copied().unwrap_or(0) as u32;
            let return_length = args.get(4).copied().unwrap_or(0);
            nt_query_information_process(ctx, handle, info_class, buffer, buffer_size, return_length)
        }

        // === Thread ===
        syscall_nr::NT_TERMINATE_THREAD => {
            let handle = args.get(0).copied().unwrap_or(u64::MAX);
            let exit_status = args.get(1).copied().unwrap_or(0) as u32;
            nt_terminate_thread(ctx, handle, exit_status)
        }
        syscall_nr::NT_CREATE_THREAD_EX => {
            let thread_handle = args.get(0).copied().unwrap_or(0);
            let desired_access = args.get(1).copied().unwrap_or(0) as u32;
            let object_attributes = args.get(2).copied().unwrap_or(0);
            let process_handle = args.get(3).copied().unwrap_or(0);
            let start_routine = args.get(4).copied().unwrap_or(0);
            let argument = args.get(5).copied().unwrap_or(0);
            let create_flags = args.get(6).copied().unwrap_or(0) as u32;
            nt_create_thread_ex(ctx, thread_handle, desired_access, process_handle,
                               start_routine, argument, create_flags)
        }

        // === Memory ===
        syscall_nr::NT_ALLOCATE_VIRTUAL_MEMORY => {
            let process_handle = args.get(0).copied().unwrap_or(0);
            let base_address_ptr = args.get(1).copied().unwrap_or(0);
            let zero_bits = args.get(2).copied().unwrap_or(0);
            let region_size_ptr = args.get(3).copied().unwrap_or(0);
            let allocation_type = args.get(4).copied().unwrap_or(0) as u32;
            let protect = args.get(5).copied().unwrap_or(0) as u32;
            nt_allocate_virtual_memory(ctx, process_handle, base_address_ptr,
                                       zero_bits, region_size_ptr, allocation_type, protect)
        }
        syscall_nr::NT_FREE_VIRTUAL_MEMORY => {
            let process_handle = args.get(0).copied().unwrap_or(0);
            let base_address_ptr = args.get(1).copied().unwrap_or(0);
            let region_size_ptr = args.get(2).copied().unwrap_or(0);
            let free_type = args.get(3).copied().unwrap_or(0) as u32;
            nt_free_virtual_memory(ctx, process_handle, base_address_ptr, region_size_ptr, free_type)
        }
        syscall_nr::NT_PROTECT_VIRTUAL_MEMORY => {
            let process_handle = args.get(0).copied().unwrap_or(0);
            let base_address_ptr = args.get(1).copied().unwrap_or(0);
            let region_size_ptr = args.get(2).copied().unwrap_or(0);
            let new_protect = args.get(3).copied().unwrap_or(0) as u32;
            let old_protect_ptr = args.get(4).copied().unwrap_or(0);
            nt_protect_virtual_memory(ctx, process_handle, base_address_ptr,
                                      region_size_ptr, new_protect, old_protect_ptr)
        }
        syscall_nr::NT_QUERY_VIRTUAL_MEMORY => {
            let process_handle = args.get(0).copied().unwrap_or(0);
            let base_address = args.get(1).copied().unwrap_or(0);
            let info_class = args.get(2).copied().unwrap_or(0) as u32;
            let buffer = args.get(3).copied().unwrap_or(0);
            let buffer_size = args.get(4).copied().unwrap_or(0);
            let return_length = args.get(5).copied().unwrap_or(0);
            nt_query_virtual_memory(ctx, process_handle, base_address, info_class,
                                    buffer, buffer_size, return_length)
        }

        // === File ===
        syscall_nr::NT_CREATE_FILE => {
            let file_handle_ptr = args.get(0).copied().unwrap_or(0);
            let desired_access = args.get(1).copied().unwrap_or(0) as u32;
            let object_attributes = args.get(2).copied().unwrap_or(0);
            let io_status_block = args.get(3).copied().unwrap_or(0);
            let allocation_size = args.get(4).copied().unwrap_or(0);
            let file_attributes = args.get(5).copied().unwrap_or(0) as u32;
            let share_access = args.get(6).copied().unwrap_or(0) as u32;
            let create_disposition = args.get(7).copied().unwrap_or(0) as u32;
            let create_options = args.get(8).copied().unwrap_or(0) as u32;
            nt_create_file(ctx, file_handle_ptr, desired_access, object_attributes,
                          io_status_block, allocation_size, file_attributes,
                          share_access, create_disposition, create_options)
        }
        syscall_nr::NT_READ_FILE => {
            let file_handle = args.get(0).copied().unwrap_or(0);
            let event = args.get(1).copied().unwrap_or(0);
            let apc_routine = args.get(2).copied().unwrap_or(0);
            let apc_context = args.get(3).copied().unwrap_or(0);
            let io_status_block = args.get(4).copied().unwrap_or(0);
            let buffer = args.get(5).copied().unwrap_or(0);
            let length = args.get(6).copied().unwrap_or(0) as u32;
            let byte_offset = args.get(7).copied().unwrap_or(0);
            nt_read_file(ctx, file_handle, event, apc_routine, apc_context,
                        io_status_block, buffer, length, byte_offset)
        }
        syscall_nr::NT_WRITE_FILE => {
            let file_handle = args.get(0).copied().unwrap_or(0);
            let event = args.get(1).copied().unwrap_or(0);
            let apc_routine = args.get(2).copied().unwrap_or(0);
            let apc_context = args.get(3).copied().unwrap_or(0);
            let io_status_block = args.get(4).copied().unwrap_or(0);
            let buffer = args.get(5).copied().unwrap_or(0);
            let length = args.get(6).copied().unwrap_or(0) as u32;
            let byte_offset = args.get(7).copied().unwrap_or(0);
            nt_write_file(ctx, file_handle, event, apc_routine, apc_context,
                         io_status_block, buffer, length, byte_offset)
        }
        syscall_nr::NT_CLOSE => {
            let handle = args.get(0).copied().unwrap_or(0);
            nt_close(ctx, handle)
        }
        syscall_nr::NT_QUERY_INFORMATION_FILE => {
            let file_handle = args.get(0).copied().unwrap_or(0);
            let io_status_block = args.get(1).copied().unwrap_or(0);
            let file_info = args.get(2).copied().unwrap_or(0);
            let length = args.get(3).copied().unwrap_or(0) as u32;
            let file_info_class = args.get(4).copied().unwrap_or(0) as u32;
            nt_query_information_file(ctx, file_handle, io_status_block,
                                      file_info, length, file_info_class)
        }
        syscall_nr::NT_SET_INFORMATION_FILE => {
            let file_handle = args.get(0).copied().unwrap_or(0);
            let io_status_block = args.get(1).copied().unwrap_or(0);
            let file_info = args.get(2).copied().unwrap_or(0);
            let length = args.get(3).copied().unwrap_or(0) as u32;
            let file_info_class = args.get(4).copied().unwrap_or(0) as u32;
            nt_set_information_file(ctx, file_handle, io_status_block,
                                    file_info, length, file_info_class)
        }
        syscall_nr::NT_QUERY_DIRECTORY_FILE => {
            let file_handle = args.get(0).copied().unwrap_or(0);
            let event = args.get(1).copied().unwrap_or(0);
            let apc_routine = args.get(2).copied().unwrap_or(0);
            let apc_context = args.get(3).copied().unwrap_or(0);
            let io_status_block = args.get(4).copied().unwrap_or(0);
            let file_info = args.get(5).copied().unwrap_or(0);
            let length = args.get(6).copied().unwrap_or(0) as u32;
            let file_info_class = args.get(7).copied().unwrap_or(0) as u32;
            nt_query_directory_file(ctx, file_handle, io_status_block, file_info,
                                    length, file_info_class)
        }
        syscall_nr::NT_DELETE_FILE => {
            let object_attributes = args.get(0).copied().unwrap_or(0);
            nt_delete_file(ctx, object_attributes)
        }

        // === Section (Memory Mapping) ===
        syscall_nr::NT_CREATE_SECTION => {
            let section_handle_ptr = args.get(0).copied().unwrap_or(0);
            let desired_access = args.get(1).copied().unwrap_or(0) as u32;
            let object_attributes = args.get(2).copied().unwrap_or(0);
            let maximum_size = args.get(3).copied().unwrap_or(0);
            let section_page_protect = args.get(4).copied().unwrap_or(0) as u32;
            let allocation_attributes = args.get(5).copied().unwrap_or(0) as u32;
            let file_handle = args.get(6).copied().unwrap_or(0);
            nt_create_section(ctx, section_handle_ptr, desired_access, object_attributes,
                             maximum_size, section_page_protect, allocation_attributes, file_handle)
        }
        syscall_nr::NT_MAP_VIEW_OF_SECTION => {
            let section_handle = args.get(0).copied().unwrap_or(0);
            let process_handle = args.get(1).copied().unwrap_or(0);
            let base_address_ptr = args.get(2).copied().unwrap_or(0);
            let zero_bits = args.get(3).copied().unwrap_or(0);
            let commit_size = args.get(4).copied().unwrap_or(0);
            let section_offset = args.get(5).copied().unwrap_or(0);
            let view_size = args.get(6).copied().unwrap_or(0);
            let inherit_disposition = args.get(7).copied().unwrap_or(0) as u32;
            let allocation_type = args.get(8).copied().unwrap_or(0) as u32;
            let win32_protect = args.get(9).copied().unwrap_or(0) as u32;
            nt_map_view_of_section(ctx, section_handle, process_handle, base_address_ptr,
                                   zero_bits, commit_size, section_offset, view_size,
                                   inherit_disposition, allocation_type, win32_protect)
        }
        syscall_nr::NT_UNMAP_VIEW_OF_SECTION => {
            let process_handle = args.get(0).copied().unwrap_or(0);
            let base_address = args.get(1).copied().unwrap_or(0);
            nt_unmap_view_of_section(ctx, process_handle, base_address)
        }

        // === Synchronization ===
        syscall_nr::NT_CREATE_EVENT => {
            let event_handle_ptr = args.get(0).copied().unwrap_or(0);
            let desired_access = args.get(1).copied().unwrap_or(0) as u32;
            let object_attributes = args.get(2).copied().unwrap_or(0);
            let event_type = args.get(3).copied().unwrap_or(0) as u32;
            let initial_state = args.get(4).copied().unwrap_or(0) != 0;
            nt_create_event(ctx, event_handle_ptr, desired_access, event_type, initial_state)
        }
        syscall_nr::NT_SET_EVENT => {
            let event_handle = args.get(0).copied().unwrap_or(0);
            let previous_state_ptr = args.get(1).copied().unwrap_or(0);
            nt_set_event(ctx, event_handle, previous_state_ptr)
        }
        syscall_nr::NT_RESET_EVENT => {
            let event_handle = args.get(0).copied().unwrap_or(0);
            let previous_state_ptr = args.get(1).copied().unwrap_or(0);
            nt_reset_event(ctx, event_handle, previous_state_ptr)
        }
        syscall_nr::NT_WAIT_FOR_SINGLE_OBJECT => {
            let handle = args.get(0).copied().unwrap_or(0);
            let alertable = args.get(1).copied().unwrap_or(0) != 0;
            let timeout = args.get(2).copied().unwrap_or(0);
            nt_wait_for_single_object(ctx, handle, alertable, timeout)
        }
        syscall_nr::NT_WAIT_FOR_MULTIPLE_OBJECTS => {
            let count = args.get(0).copied().unwrap_or(0) as u32;
            let handles_ptr = args.get(1).copied().unwrap_or(0);
            let wait_type = args.get(2).copied().unwrap_or(0) as u32;
            let alertable = args.get(3).copied().unwrap_or(0) != 0;
            let timeout = args.get(4).copied().unwrap_or(0);
            nt_wait_for_multiple_objects(ctx, count, handles_ptr, wait_type, alertable, timeout)
        }
        syscall_nr::NT_CREATE_MUTANT => {
            let mutant_handle_ptr = args.get(0).copied().unwrap_or(0);
            let desired_access = args.get(1).copied().unwrap_or(0) as u32;
            let object_attributes = args.get(2).copied().unwrap_or(0);
            let initial_owner = args.get(3).copied().unwrap_or(0) != 0;
            nt_create_mutant(ctx, mutant_handle_ptr, desired_access, initial_owner)
        }
        syscall_nr::NT_RELEASE_MUTANT => {
            let mutant_handle = args.get(0).copied().unwrap_or(0);
            let previous_count_ptr = args.get(1).copied().unwrap_or(0);
            nt_release_mutant(ctx, mutant_handle, previous_count_ptr)
        }
        syscall_nr::NT_CREATE_SEMAPHORE => {
            let sem_handle_ptr = args.get(0).copied().unwrap_or(0);
            let desired_access = args.get(1).copied().unwrap_or(0) as u32;
            let object_attributes = args.get(2).copied().unwrap_or(0);
            let initial_count = args.get(3).copied().unwrap_or(0) as i32;
            let maximum_count = args.get(4).copied().unwrap_or(1) as i32;
            nt_create_semaphore(ctx, sem_handle_ptr, desired_access, initial_count, maximum_count)
        }
        syscall_nr::NT_RELEASE_SEMAPHORE => {
            let sem_handle = args.get(0).copied().unwrap_or(0);
            let release_count = args.get(1).copied().unwrap_or(1) as i32;
            let previous_count_ptr = args.get(2).copied().unwrap_or(0);
            nt_release_semaphore(ctx, sem_handle, release_count, previous_count_ptr)
        }

        // === System ===
        syscall_nr::NT_QUERY_SYSTEM_INFORMATION => {
            let system_info_class = args.get(0).copied().unwrap_or(0) as u32;
            let system_info = args.get(1).copied().unwrap_or(0);
            let system_info_length = args.get(2).copied().unwrap_or(0) as u32;
            let return_length = args.get(3).copied().unwrap_or(0);
            nt_query_system_information(ctx, system_info_class, system_info,
                                        system_info_length, return_length)
        }
        syscall_nr::NT_QUERY_PERFORMANCE_COUNTER => {
            let performance_counter = args.get(0).copied().unwrap_or(0);
            let performance_frequency = args.get(1).copied().unwrap_or(0);
            nt_query_performance_counter(ctx, performance_counter, performance_frequency)
        }
        syscall_nr::NT_DELAY_EXECUTION => {
            let alertable = args.get(0).copied().unwrap_or(0) != 0;
            let delay_interval = args.get(1).copied().unwrap_or(0);
            nt_delay_execution(ctx, alertable, delay_interval)
        }
        syscall_nr::NT_YIELD_EXECUTION => {
            nt_yield_execution(ctx)
        }
        syscall_nr::NT_QUERY_SYSTEM_TIME => {
            let system_time = args.get(0).copied().unwrap_or(0);
            nt_query_system_time(ctx, system_time)
        }

        // === Registry ===
        syscall_nr::NT_CREATE_KEY | syscall_nr::NT_OPEN_KEY | syscall_nr::NT_OPEN_KEY_EX => {
            let key_handle_ptr = args.get(0).copied().unwrap_or(0);
            let desired_access = args.get(1).copied().unwrap_or(0) as u32;
            let object_attributes = args.get(2).copied().unwrap_or(0);
            nt_open_key(ctx, key_handle_ptr, desired_access, object_attributes)
        }
        syscall_nr::NT_QUERY_VALUE_KEY => {
            let key_handle = args.get(0).copied().unwrap_or(0);
            let value_name = args.get(1).copied().unwrap_or(0);
            let key_value_info_class = args.get(2).copied().unwrap_or(0) as u32;
            let key_value_info = args.get(3).copied().unwrap_or(0);
            let length = args.get(4).copied().unwrap_or(0) as u32;
            let result_length = args.get(5).copied().unwrap_or(0);
            nt_query_value_key(ctx, key_handle, value_name, key_value_info_class,
                              key_value_info, length, result_length)
        }
        syscall_nr::NT_SET_VALUE_KEY => {
            let key_handle = args.get(0).copied().unwrap_or(0);
            let value_name = args.get(1).copied().unwrap_or(0);
            let title_index = args.get(2).copied().unwrap_or(0) as u32;
            let value_type = args.get(3).copied().unwrap_or(0) as u32;
            let data = args.get(4).copied().unwrap_or(0);
            let data_size = args.get(5).copied().unwrap_or(0) as u32;
            nt_set_value_key(ctx, key_handle, value_name, title_index, value_type, data, data_size)
        }

        // === Token/Security ===
        syscall_nr::NT_OPEN_PROCESS_TOKEN => {
            let process_handle = args.get(0).copied().unwrap_or(0);
            let desired_access = args.get(1).copied().unwrap_or(0) as u32;
            let token_handle = args.get(2).copied().unwrap_or(0);
            nt_open_process_token(ctx, process_handle, desired_access, token_handle)
        }
        syscall_nr::NT_QUERY_INFORMATION_TOKEN => {
            let token_handle = args.get(0).copied().unwrap_or(0);
            let token_info_class = args.get(1).copied().unwrap_or(0) as u32;
            let token_info = args.get(2).copied().unwrap_or(0);
            let token_info_length = args.get(3).copied().unwrap_or(0) as u32;
            let return_length = args.get(4).copied().unwrap_or(0);
            nt_query_information_token(ctx, token_handle, token_info_class,
                                       token_info, token_info_length, return_length)
        }

        // Default: not implemented
        _ => {
            STATS.unimplemented_calls.fetch_add(1, Ordering::Relaxed);
            crate::kprintln!("ntsyscall: unimplemented syscall {:#x}", syscall_nr);
            status::NOT_IMPLEMENTED
        }
    };

    if result == status::SUCCESS {
        STATS.successful_calls.fetch_add(1, Ordering::Relaxed);
    } else if result != status::NOT_IMPLEMENTED {
        STATS.failed_calls.fetch_add(1, Ordering::Relaxed);
    }

    result
}

// ============================================================================
// Syscall Implementations
// ============================================================================

fn nt_terminate_process(ctx: &mut NtSyscallContext, handle: u64, exit_status: u32) -> u32 {
    if handle == u64::MAX || handle == 0xFFFFFFFF_FFFFFFFF {
        // Terminate current process
        crate::syscall::sys_exit(exit_status as u64);
    }
    status::SUCCESS
}

fn nt_query_information_process(
    ctx: &mut NtSyscallContext,
    handle: u64,
    info_class: u32,
    buffer: u64,
    buffer_size: u32,
    return_length: u64,
) -> u32 {
    // ProcessBasicInformation = 0
    // ProcessDebugPort = 7
    // ProcessWow64Information = 26
    // ProcessImageFileName = 27

    if buffer == 0 {
        return status::INVALID_PARAMETER;
    }

    match info_class {
        0 => { // ProcessBasicInformation
            if buffer_size < 48 {
                return status::BUFFER_TOO_SMALL;
            }
            // Fill with basic info
            let current = crate::sched::current_task();
            let pid = current.id() as u64;
            unsafe {
                let ptr = buffer as *mut u64;
                *ptr = 0; // ExitStatus
                *ptr.add(1) = 0; // PebBaseAddress
                *ptr.add(2) = 0; // AffinityMask
                *ptr.add(3) = 8; // BasePriority
                *ptr.add(4) = pid; // UniqueProcessId
                *ptr.add(5) = 0; // InheritedFromUniqueProcessId
            }
            if return_length != 0 {
                unsafe { *(return_length as *mut u32) = 48; }
            }
            status::SUCCESS
        }
        26 => { // ProcessWow64Information - we're 64-bit, return 0
            if buffer_size < 8 {
                return status::BUFFER_TOO_SMALL;
            }
            unsafe { *(buffer as *mut u64) = 0; }
            if return_length != 0 {
                unsafe { *(return_length as *mut u32) = 8; }
            }
            status::SUCCESS
        }
        _ => status::NOT_IMPLEMENTED
    }
}

fn nt_terminate_thread(ctx: &mut NtSyscallContext, handle: u64, exit_status: u32) -> u32 {
    if handle == u64::MAX || handle == 0xFFFFFFFF_FFFFFFFE {
        // Terminate current thread - in single-threaded process this exits
        crate::syscall::sys_exit(exit_status as u64);
    }
    status::NOT_IMPLEMENTED
}

fn nt_create_thread_ex(
    ctx: &mut NtSyscallContext,
    thread_handle_ptr: u64,
    desired_access: u32,
    process_handle: u64,
    start_routine: u64,
    argument: u64,
    create_flags: u32,
) -> u32 {
    // Use clone() syscall to create thread
    let flags = 0x10900; // CLONE_VM | CLONE_FS | CLONE_FILES | CLONE_SIGHAND | CLONE_THREAD

    // This would need to set up the thread properly
    // For now, return not implemented
    status::NOT_IMPLEMENTED
}

fn nt_allocate_virtual_memory(
    ctx: &mut NtSyscallContext,
    process_handle: u64,
    base_address_ptr: u64,
    zero_bits: u64,
    region_size_ptr: u64,
    allocation_type: u32,
    protect: u32,
) -> u32 {
    if region_size_ptr == 0 {
        return status::INVALID_PARAMETER;
    }

    let size = unsafe { *(region_size_ptr as *const u64) };
    let hint = if base_address_ptr != 0 {
        unsafe { *(base_address_ptr as *const u64) }
    } else {
        0
    };

    // Map protection to POSIX
    let prot = win_protect_to_posix(protect);

    // Map to mmap
    let flags = 0x22i32; // MAP_ANONYMOUS | MAP_PRIVATE

    let result = crate::syscall::sys_mmap(hint, size as usize, prot as i32, flags, -1, 0);

    if result < 0x1000 {
        // Error
        return status::NO_MEMORY;
    }

    // Write back allocated address and size
    if base_address_ptr != 0 {
        unsafe { *(base_address_ptr as *mut u64) = result as u64; }
    }

    // Round up size to page boundary
    let aligned_size = (size + 0xFFF) & !0xFFF;
    unsafe { *(region_size_ptr as *mut u64) = aligned_size; }

    status::SUCCESS
}

fn nt_free_virtual_memory(
    ctx: &mut NtSyscallContext,
    process_handle: u64,
    base_address_ptr: u64,
    region_size_ptr: u64,
    free_type: u32,
) -> u32 {
    if base_address_ptr == 0 {
        return status::INVALID_PARAMETER;
    }

    let address = unsafe { *(base_address_ptr as *const u64) };
    let size = if region_size_ptr != 0 {
        unsafe { *(region_size_ptr as *const u64) }
    } else {
        0x1000 // Default to one page
    };

    const MEM_RELEASE: u32 = 0x8000;
    const MEM_DECOMMIT: u32 = 0x4000;

    if free_type & MEM_RELEASE != 0 {
        // munmap
        let result = crate::syscall::sys_munmap(address, size as usize);
        if result != 0 {
            return status::UNSUCCESSFUL;
        }
    }

    status::SUCCESS
}

fn nt_protect_virtual_memory(
    ctx: &mut NtSyscallContext,
    process_handle: u64,
    base_address_ptr: u64,
    region_size_ptr: u64,
    new_protect: u32,
    old_protect_ptr: u64,
) -> u32 {
    if base_address_ptr == 0 || region_size_ptr == 0 {
        return status::INVALID_PARAMETER;
    }

    let address = unsafe { *(base_address_ptr as *const u64) };
    let size = unsafe { *(region_size_ptr as *const u64) };
    let prot = win_protect_to_posix(new_protect);

    // For now, assume old protection was PAGE_READWRITE
    if old_protect_ptr != 0 {
        unsafe { *(old_protect_ptr as *mut u32) = 0x04; } // PAGE_READWRITE
    }

    // Call mprotect
    let result = crate::syscall::sys_mprotect(address, size as usize, prot as i32);
    if result != 0 {
        return status::UNSUCCESSFUL;
    }

    status::SUCCESS
}

fn nt_query_virtual_memory(
    ctx: &mut NtSyscallContext,
    process_handle: u64,
    base_address: u64,
    info_class: u32,
    buffer: u64,
    buffer_size: u64,
    return_length: u64,
) -> u32 {
    // MemoryBasicInformation = 0
    if info_class == 0 {
        if buffer_size < 48 {
            return status::BUFFER_TOO_SMALL;
        }

        // Fill with placeholder data
        unsafe {
            let ptr = buffer as *mut u64;
            *ptr = base_address & !0xFFF; // BaseAddress (page-aligned)
            *ptr.add(1) = base_address & !0xFFF; // AllocationBase
            *ptr.add(2) = 0x04; // AllocationProtect (PAGE_READWRITE)
            *ptr.add(3) = 0x1000; // RegionSize
            *ptr.add(4) = 0x1000; // State (MEM_COMMIT)
            *ptr.add(5) = 0x04; // Protect
        }

        if return_length != 0 {
            unsafe { *(return_length as *mut u64) = 48; }
        }

        return status::SUCCESS;
    }

    status::NOT_IMPLEMENTED
}

fn nt_create_file(
    ctx: &mut NtSyscallContext,
    file_handle_ptr: u64,
    desired_access: u32,
    object_attributes: u64,
    io_status_block: u64,
    allocation_size: u64,
    file_attributes: u32,
    share_access: u32,
    create_disposition: u32,
    create_options: u32,
) -> u32 {
    if file_handle_ptr == 0 || object_attributes == 0 {
        return status::INVALID_PARAMETER;
    }

    // Parse object attributes to get path
    let path = read_object_attributes_path(object_attributes);
    if path.is_none() {
        return status::OBJECT_NAME_NOT_FOUND;
    }
    let win_path = path.unwrap();

    // Translate Windows path to Unix path
    let unix_path = fs_translate::windows_to_unix(&win_path);

    // Map creation disposition to flags
    let mut flags: u64 = 0;
    const FILE_CREATE: u32 = 2;
    const FILE_OPEN: u32 = 1;
    const FILE_OPEN_IF: u32 = 3;
    const FILE_OVERWRITE: u32 = 4;
    const FILE_OVERWRITE_IF: u32 = 5;
    const FILE_SUPERSEDE: u32 = 0;

    match create_disposition {
        FILE_CREATE => flags |= 0o100 | 0o200, // O_CREAT | O_EXCL
        FILE_OPEN => {}, // O_RDONLY
        FILE_OPEN_IF => flags |= 0o100, // O_CREAT
        FILE_OVERWRITE => flags |= 0o1000, // O_TRUNC
        FILE_OVERWRITE_IF => flags |= 0o100 | 0o1000, // O_CREAT | O_TRUNC
        FILE_SUPERSEDE => flags |= 0o100 | 0o1000, // O_CREAT | O_TRUNC
        _ => {}
    }

    // Map access to flags
    const FILE_READ_DATA: u32 = 1;
    const FILE_WRITE_DATA: u32 = 2;

    if (desired_access & FILE_WRITE_DATA) != 0 {
        if (desired_access & FILE_READ_DATA) != 0 {
            flags |= 0o2; // O_RDWR
        } else {
            flags |= 0o1; // O_WRONLY
        }
    }

    // Call open syscall
    let fd = crate::syscall::sys_open(unix_path.as_ptr() as u64, flags as u32, 0o644);

    if fd < 0 {
        set_io_status(io_status_block, status::OBJECT_NAME_NOT_FOUND, 0);
        return status::OBJECT_NAME_NOT_FOUND;
    }

    // Allocate Windows handle
    let handle = ctx.alloc_handle(fd as i32);
    unsafe { *(file_handle_ptr as *mut u64) = handle; }

    // Set I/O status
    let information = match create_disposition {
        FILE_CREATE => 2, // FILE_CREATED
        FILE_OPEN => 1, // FILE_OPENED
        FILE_OPEN_IF => 1,
        FILE_OVERWRITE => 3, // FILE_OVERWRITTEN
        FILE_OVERWRITE_IF => 3,
        _ => 1,
    };
    set_io_status(io_status_block, status::SUCCESS, information);

    status::SUCCESS
}

fn nt_read_file(
    ctx: &mut NtSyscallContext,
    file_handle: u64,
    event: u64,
    apc_routine: u64,
    apc_context: u64,
    io_status_block: u64,
    buffer: u64,
    length: u32,
    byte_offset: u64,
) -> u32 {
    let fd = match ctx.translate_handle(file_handle) {
        Some(fd) => fd,
        None => {
            set_io_status(io_status_block, status::INVALID_HANDLE, 0);
            return status::INVALID_HANDLE;
        }
    };

    // Handle byte offset if provided
    if byte_offset != 0 {
        let offset = unsafe { *(byte_offset as *const i64) };
        if offset >= 0 {
            crate::syscall::sys_lseek(fd, offset as i64, 0); // SEEK_SET
        }
    }

    // Call read syscall
    let result = crate::syscall::sys_read(fd, buffer, length as usize);

    if result < 0 {
        set_io_status(io_status_block, status::UNSUCCESSFUL, 0);
        return status::UNSUCCESSFUL;
    }

    set_io_status(io_status_block, status::SUCCESS, result as u64);
    status::SUCCESS
}

fn nt_write_file(
    ctx: &mut NtSyscallContext,
    file_handle: u64,
    event: u64,
    apc_routine: u64,
    apc_context: u64,
    io_status_block: u64,
    buffer: u64,
    length: u32,
    byte_offset: u64,
) -> u32 {
    let fd = match ctx.translate_handle(file_handle) {
        Some(fd) => fd,
        None => {
            set_io_status(io_status_block, status::INVALID_HANDLE, 0);
            return status::INVALID_HANDLE;
        }
    };

    // Handle byte offset if provided
    if byte_offset != 0 {
        let offset = unsafe { *(byte_offset as *const i64) };
        if offset >= 0 {
            crate::syscall::sys_lseek(fd, offset as i64, 0); // SEEK_SET
        }
    }

    // Call write syscall
    let result = crate::syscall::sys_write(fd, buffer, length as usize);

    if result < 0 {
        set_io_status(io_status_block, status::UNSUCCESSFUL, 0);
        return status::UNSUCCESSFUL;
    }

    set_io_status(io_status_block, status::SUCCESS, result as u64);
    status::SUCCESS
}

fn nt_close(ctx: &mut NtSyscallContext, handle: u64) -> u32 {
    // Check for pseudo-handles
    if handle == u64::MAX || handle == 0xFFFFFFFF_FFFFFFFE {
        return status::SUCCESS; // Pseudo-handles don't need closing
    }

    // Check for section handle
    if ctx.sections.remove(&handle).is_some() {
        return status::SUCCESS;
    }

    let fd = match ctx.translate_handle(handle) {
        Some(fd) => fd,
        None => return status::INVALID_HANDLE,
    };

    // Don't close standard handles
    if fd >= 0 && fd <= 2 {
        return status::SUCCESS;
    }

    crate::syscall::sys_close(fd);
    ctx.close_handle(handle);

    status::SUCCESS
}

fn nt_query_information_file(
    ctx: &mut NtSyscallContext,
    file_handle: u64,
    io_status_block: u64,
    file_info: u64,
    length: u32,
    file_info_class: u32,
) -> u32 {
    let fd = match ctx.translate_handle(file_handle) {
        Some(fd) => fd,
        None => {
            set_io_status(io_status_block, status::INVALID_HANDLE, 0);
            return status::INVALID_HANDLE;
        }
    };

    // FileStandardInformation = 5
    // FilePositionInformation = 14
    // FileBasicInformation = 4

    match file_info_class {
        5 => { // FileStandardInformation
            if length < 24 {
                return status::BUFFER_TOO_SMALL;
            }

            // Get file size via fstat
            let mut stat_buf = [0u8; 144];
            let result = crate::syscall::sys_fstat(fd, stat_buf.as_mut_ptr() as u64);
            if result != 0 {
                set_io_status(io_status_block, status::UNSUCCESSFUL, 0);
                return status::UNSUCCESSFUL;
            }

            // Parse size from stat buffer (offset 48 in Linux stat)
            let size = u64::from_ne_bytes(stat_buf[48..56].try_into().unwrap());

            unsafe {
                let ptr = file_info as *mut u64;
                *ptr = size; // AllocationSize
                *ptr.add(1) = size; // EndOfFile
                *ptr.add(2) = 1; // NumberOfLinks
                *(file_info as *mut u8).add(20) = 0; // DeletePending
                *(file_info as *mut u8).add(21) = 0; // Directory
            }

            set_io_status(io_status_block, status::SUCCESS, 24);
            status::SUCCESS
        }
        14 => { // FilePositionInformation
            if length < 8 {
                return status::BUFFER_TOO_SMALL;
            }

            let pos = crate::syscall::sys_lseek(fd, 0, 1); // SEEK_CUR
            unsafe { *(file_info as *mut u64) = pos as u64; }

            set_io_status(io_status_block, status::SUCCESS, 8);
            status::SUCCESS
        }
        _ => {
            set_io_status(io_status_block, status::NOT_IMPLEMENTED, 0);
            status::NOT_IMPLEMENTED
        }
    }
}

fn nt_set_information_file(
    ctx: &mut NtSyscallContext,
    file_handle: u64,
    io_status_block: u64,
    file_info: u64,
    length: u32,
    file_info_class: u32,
) -> u32 {
    let fd = match ctx.translate_handle(file_handle) {
        Some(fd) => fd,
        None => {
            set_io_status(io_status_block, status::INVALID_HANDLE, 0);
            return status::INVALID_HANDLE;
        }
    };

    match file_info_class {
        14 => { // FilePositionInformation
            if length < 8 {
                return status::BUFFER_TOO_SMALL;
            }
            let pos = unsafe { *(file_info as *const u64) };
            crate::syscall::sys_lseek(fd, pos as i64, 0); // SEEK_SET
            set_io_status(io_status_block, status::SUCCESS, 0);
            status::SUCCESS
        }
        13 => { // FileEndOfFileInformation - truncate
            if length < 8 {
                return status::BUFFER_TOO_SMALL;
            }
            let size = unsafe { *(file_info as *const u64) };
            crate::syscall::sys_ftruncate(fd, size as i64);
            set_io_status(io_status_block, status::SUCCESS, 0);
            status::SUCCESS
        }
        _ => {
            set_io_status(io_status_block, status::NOT_IMPLEMENTED, 0);
            status::NOT_IMPLEMENTED
        }
    }
}

fn nt_query_directory_file(
    ctx: &mut NtSyscallContext,
    file_handle: u64,
    io_status_block: u64,
    file_info: u64,
    length: u32,
    file_info_class: u32,
) -> u32 {
    let fd = match ctx.translate_handle(file_handle) {
        Some(fd) => fd,
        None => {
            set_io_status(io_status_block, status::INVALID_HANDLE, 0);
            return status::INVALID_HANDLE;
        }
    };

    // Use getdents64
    let result = crate::syscall::sys_getdents64(fd, file_info, length as usize);

    if result < 0 {
        set_io_status(io_status_block, status::UNSUCCESSFUL, 0);
        return status::UNSUCCESSFUL;
    }

    if result == 0 {
        set_io_status(io_status_block, status::SUCCESS, 0);
        return status::SUCCESS; // No more entries
    }

    set_io_status(io_status_block, status::SUCCESS, result as u64);
    status::SUCCESS
}

fn nt_delete_file(ctx: &mut NtSyscallContext, object_attributes: u64) -> u32 {
    let path = read_object_attributes_path(object_attributes);
    if path.is_none() {
        return status::OBJECT_NAME_NOT_FOUND;
    }
    let unix_path = fs_translate::windows_to_unix(&path.unwrap());

    let result = crate::syscall::sys_unlink(unix_path.as_ptr() as u64);
    if result != 0 {
        return status::OBJECT_NAME_NOT_FOUND;
    }

    status::SUCCESS
}

fn nt_create_section(
    ctx: &mut NtSyscallContext,
    section_handle_ptr: u64,
    desired_access: u32,
    object_attributes: u64,
    maximum_size: u64,
    section_page_protect: u32,
    allocation_attributes: u32,
    file_handle: u64,
) -> u32 {
    if section_handle_ptr == 0 {
        return status::INVALID_PARAMETER;
    }

    let size = if maximum_size != 0 {
        unsafe { *(maximum_size as *const u64) }
    } else if file_handle != 0 {
        // Get file size
        let fd = ctx.translate_handle(file_handle);
        if fd.is_none() {
            return status::INVALID_HANDLE;
        }
        // Would need to stat the file - for now use a default
        0x10000
    } else {
        return status::INVALID_PARAMETER;
    };

    // Create a section handle
    let handle = ctx.next_handle;
    ctx.next_handle += 4;

    ctx.sections.insert(handle, SectionInfo {
        size,
        file_handle: if file_handle != 0 { Some(file_handle) } else { None },
        protect: section_page_protect,
    });

    unsafe { *(section_handle_ptr as *mut u64) = handle; }

    status::SUCCESS
}

fn nt_map_view_of_section(
    ctx: &mut NtSyscallContext,
    section_handle: u64,
    process_handle: u64,
    base_address_ptr: u64,
    zero_bits: u64,
    commit_size: u64,
    section_offset: u64,
    view_size: u64,
    inherit_disposition: u32,
    allocation_type: u32,
    win32_protect: u32,
) -> u32 {
    let section = match ctx.sections.get(&section_handle) {
        Some(s) => s.clone(),
        None => return status::INVALID_HANDLE,
    };

    let size = if view_size != 0 {
        unsafe { *(view_size as *const u64) }
    } else {
        section.size
    };

    let offset = if section_offset != 0 {
        unsafe { *(section_offset as *const u64) }
    } else {
        0
    };

    let hint = if base_address_ptr != 0 {
        unsafe { *(base_address_ptr as *const u64) }
    } else {
        0
    };

    let prot = win_protect_to_posix(win32_protect);

    // Determine if file-backed or anonymous
    let (flags, fd): (i32, i32) = if let Some(fh) = section.file_handle {
        (0x01, ctx.translate_handle(fh).unwrap_or(-1)) // MAP_SHARED
    } else {
        (0x22, -1) // MAP_ANONYMOUS | MAP_PRIVATE
    };

    let result = crate::syscall::sys_mmap(hint, size as usize, prot as i32, flags, fd, offset as i64);

    if result < 0x1000 {
        return status::NO_MEMORY;
    }

    if base_address_ptr != 0 {
        unsafe { *(base_address_ptr as *mut u64) = result as u64; }
    }

    if view_size != 0 {
        let aligned = (size + 0xFFF) & !0xFFF;
        unsafe { *(view_size as *mut u64) = aligned; }
    }

    status::SUCCESS
}

fn nt_unmap_view_of_section(
    ctx: &mut NtSyscallContext,
    process_handle: u64,
    base_address: u64,
) -> u32 {
    // We don't track the exact size, assume one page minimum
    // In a full implementation we'd track mapped views
    let result = crate::syscall::sys_munmap(base_address, 0x1000);
    if result != 0 {
        return status::UNSUCCESSFUL;
    }
    status::SUCCESS
}

fn nt_create_event(
    ctx: &mut NtSyscallContext,
    event_handle_ptr: u64,
    desired_access: u32,
    event_type: u32,
    initial_state: bool,
) -> u32 {
    if event_handle_ptr == 0 {
        return status::INVALID_PARAMETER;
    }

    let mut handle = 0u64;
    let result = ctx.get_ntdll().create_event(&mut handle, desired_access, None, event_type, initial_state);

    if result == status::SUCCESS {
        unsafe { *(event_handle_ptr as *mut u64) = handle; }
    }

    result
}

fn nt_set_event(ctx: &mut NtSyscallContext, event_handle: u64, previous_state_ptr: u64) -> u32 {
    let mut previous = 0u32;
    let result = ctx.get_ntdll().set_event(event_handle, &mut previous);

    if result == status::SUCCESS && previous_state_ptr != 0 {
        unsafe { *(previous_state_ptr as *mut u32) = previous; }
    }

    result
}

fn nt_reset_event(ctx: &mut NtSyscallContext, event_handle: u64, previous_state_ptr: u64) -> u32 {
    let mut previous = 0u32;
    let result = ctx.get_ntdll().reset_event(event_handle, &mut previous);

    if result == status::SUCCESS && previous_state_ptr != 0 {
        unsafe { *(previous_state_ptr as *mut u32) = previous; }
    }

    result
}

fn nt_wait_for_single_object(
    ctx: &mut NtSyscallContext,
    handle: u64,
    alertable: bool,
    timeout_ptr: u64,
) -> u32 {
    // Convert timeout from 100ns units to nanoseconds
    let timeout = if timeout_ptr != 0 {
        let val = unsafe { *(timeout_ptr as *const i64) };
        if val < 0 {
            // Relative timeout - convert to positive ns
            Some((-val * 100) as u64)
        } else if val == 0 {
            None // Infinite wait
        } else {
            // Absolute timeout (not supported, treat as relative)
            Some((val * 100) as u64)
        }
    } else {
        None
    };

    // For now, just yield and return success
    crate::syscall::sys_sched_yield();

    status::SUCCESS
}

fn nt_wait_for_multiple_objects(
    ctx: &mut NtSyscallContext,
    count: u32,
    handles_ptr: u64,
    wait_type: u32,
    alertable: bool,
    timeout: u64,
) -> u32 {
    // For now, just yield
    crate::syscall::sys_sched_yield();
    status::SUCCESS
}

fn nt_create_mutant(
    ctx: &mut NtSyscallContext,
    mutant_handle_ptr: u64,
    desired_access: u32,
    initial_owner: bool,
) -> u32 {
    if mutant_handle_ptr == 0 {
        return status::INVALID_PARAMETER;
    }

    // Use futex for mutex implementation
    let handle = ctx.next_handle;
    ctx.next_handle += 4;

    unsafe { *(mutant_handle_ptr as *mut u64) = handle; }

    status::SUCCESS
}

fn nt_release_mutant(
    ctx: &mut NtSyscallContext,
    mutant_handle: u64,
    previous_count_ptr: u64,
) -> u32 {
    if previous_count_ptr != 0 {
        unsafe { *(previous_count_ptr as *mut u32) = 0; }
    }
    status::SUCCESS
}

fn nt_create_semaphore(
    ctx: &mut NtSyscallContext,
    sem_handle_ptr: u64,
    desired_access: u32,
    initial_count: i32,
    maximum_count: i32,
) -> u32 {
    if sem_handle_ptr == 0 || initial_count < 0 || maximum_count < 1 {
        return status::INVALID_PARAMETER;
    }

    let handle = ctx.next_handle;
    ctx.next_handle += 4;

    unsafe { *(sem_handle_ptr as *mut u64) = handle; }

    status::SUCCESS
}

fn nt_release_semaphore(
    ctx: &mut NtSyscallContext,
    sem_handle: u64,
    release_count: i32,
    previous_count_ptr: u64,
) -> u32 {
    if previous_count_ptr != 0 {
        unsafe { *(previous_count_ptr as *mut u32) = 0; }
    }
    status::SUCCESS
}

fn nt_query_system_information(
    ctx: &mut NtSyscallContext,
    system_info_class: u32,
    system_info: u64,
    system_info_length: u32,
    return_length: u64,
) -> u32 {
    // SystemBasicInformation = 0
    // SystemProcessorInformation = 1
    // SystemPerformanceInformation = 2
    // SystemTimeOfDayInformation = 3

    match system_info_class {
        0 => { // SystemBasicInformation
            if system_info_length < 44 {
                if return_length != 0 {
                    unsafe { *(return_length as *mut u32) = 44; }
                }
                return status::BUFFER_TOO_SMALL;
            }

            unsafe {
                let ptr = system_info as *mut u32;
                *ptr = 0; // Reserved
                *ptr.add(1) = 0; // TimerResolution
                *ptr.add(2) = 0x1000; // PageSize
                *ptr.add(3) = 1024; // NumberOfPhysicalPages
                *ptr.add(4) = 256; // LowestPhysicalPageNumber
                *ptr.add(5) = 0x100000; // HighestPhysicalPageNumber
                *ptr.add(6) = 0x1000; // AllocationGranularity
                *(system_info as *mut u64).add(4) = 0x10000; // MinimumUserModeAddress
                *(system_info as *mut u64).add(5) = 0x7FFFFFFEFFFF; // MaximumUserModeAddress
                *ptr.add(10) = 1; // ActiveProcessorsAffinityMask
                *(ptr.add(11) as *mut u8) = 1; // NumberOfProcessors
            }

            if return_length != 0 {
                unsafe { *(return_length as *mut u32) = 44; }
            }
            status::SUCCESS
        }
        3 => { // SystemTimeOfDayInformation
            if system_info_length < 48 {
                if return_length != 0 {
                    unsafe { *(return_length as *mut u32) = 48; }
                }
                return status::BUFFER_TOO_SMALL;
            }

            let ticks = crate::time::ticks();
            // Convert to Windows FILETIME (100ns since 1601)
            // Approximate: just use ticks * 10000 (ms to 100ns)
            let filetime = ticks * 10000 + 116444736000000000u64; // Offset to 1601

            unsafe {
                let ptr = system_info as *mut u64;
                *ptr = filetime; // BootTime
                *ptr.add(1) = filetime; // CurrentTime
                *ptr.add(2) = 0; // TimeZoneBias
                *ptr.add(3) = ticks; // TimeZoneId + CurrentTimeZoneId + etc
            }

            if return_length != 0 {
                unsafe { *(return_length as *mut u32) = 48; }
            }
            status::SUCCESS
        }
        _ => status::NOT_IMPLEMENTED
    }
}

fn nt_query_performance_counter(
    ctx: &mut NtSyscallContext,
    performance_counter: u64,
    performance_frequency: u64,
) -> u32 {
    let ticks = crate::time::ticks() as i64;

    if performance_counter != 0 {
        unsafe { *(performance_counter as *mut i64) = ticks; }
    }

    if performance_frequency != 0 {
        unsafe { *(performance_frequency as *mut i64) = 1000; } // 1000 Hz
    }

    status::SUCCESS
}

fn nt_delay_execution(ctx: &mut NtSyscallContext, alertable: bool, delay_interval: u64) -> u32 {
    if delay_interval == 0 {
        return status::SUCCESS;
    }

    let interval = unsafe { *(delay_interval as *const i64) };

    // Negative = relative, positive = absolute
    let nanos = if interval < 0 {
        (-interval * 100) as u64 // Convert 100ns to ns
    } else {
        (interval * 100) as u64
    };

    // Convert to timespec
    let secs = nanos / 1_000_000_000;
    let nsecs = nanos % 1_000_000_000;

    let mut ts = [secs, nsecs];
    crate::syscall::sys_nanosleep(ts.as_ptr() as u64, 0);

    status::SUCCESS
}

fn nt_yield_execution(ctx: &mut NtSyscallContext) -> u32 {
    crate::syscall::sys_sched_yield();
    status::SUCCESS
}

fn nt_query_system_time(ctx: &mut NtSyscallContext, system_time: u64) -> u32 {
    if system_time == 0 {
        return status::INVALID_PARAMETER;
    }

    let ticks = crate::time::ticks();
    // Convert to Windows FILETIME
    let filetime = ticks * 10000 + 116444736000000000u64;

    unsafe { *(system_time as *mut u64) = filetime; }

    status::SUCCESS
}

fn nt_open_key(
    ctx: &mut NtSyscallContext,
    key_handle_ptr: u64,
    desired_access: u32,
    object_attributes: u64,
) -> u32 {
    // Use our registry emulation
    let path = read_object_attributes_path(object_attributes);
    if path.is_none() {
        return status::OBJECT_NAME_NOT_FOUND;
    }

    // For now, return a pseudo-handle for common keys
    let handle = ctx.next_handle;
    ctx.next_handle += 4;

    if key_handle_ptr != 0 {
        unsafe { *(key_handle_ptr as *mut u64) = handle; }
    }

    status::SUCCESS
}

fn nt_query_value_key(
    ctx: &mut NtSyscallContext,
    key_handle: u64,
    value_name: u64,
    key_value_info_class: u32,
    key_value_info: u64,
    length: u32,
    result_length: u64,
) -> u32 {
    // Return not found for most queries
    if result_length != 0 {
        unsafe { *(result_length as *mut u32) = 0; }
    }
    status::OBJECT_NAME_NOT_FOUND
}

fn nt_set_value_key(
    ctx: &mut NtSyscallContext,
    key_handle: u64,
    value_name: u64,
    title_index: u32,
    value_type: u32,
    data: u64,
    data_size: u32,
) -> u32 {
    // Accept but don't persist
    status::SUCCESS
}

fn nt_open_process_token(
    ctx: &mut NtSyscallContext,
    process_handle: u64,
    desired_access: u32,
    token_handle: u64,
) -> u32 {
    if token_handle == 0 {
        return status::INVALID_PARAMETER;
    }

    // Return a pseudo-handle for the token
    let handle = ctx.next_handle;
    ctx.next_handle += 4;

    unsafe { *(token_handle as *mut u64) = handle; }

    status::SUCCESS
}

fn nt_query_information_token(
    ctx: &mut NtSyscallContext,
    token_handle: u64,
    token_info_class: u32,
    token_info: u64,
    token_info_length: u32,
    return_length: u64,
) -> u32 {
    // TokenUser = 1, TokenGroups = 2, TokenPrivileges = 3
    // Return basic info for common classes

    match token_info_class {
        1 => { // TokenUser
            if token_info_length < 16 {
                if return_length != 0 {
                    unsafe { *(return_length as *mut u32) = 16; }
                }
                return status::BUFFER_TOO_SMALL;
            }

            // Return a placeholder SID
            unsafe {
                let ptr = token_info as *mut u64;
                *ptr = 0; // SID pointer
                *ptr.add(1) = 0; // Attributes
            }

            if return_length != 0 {
                unsafe { *(return_length as *mut u32) = 16; }
            }
            status::SUCCESS
        }
        _ => {
            if return_length != 0 {
                unsafe { *(return_length as *mut u32) = 0; }
            }
            status::NOT_IMPLEMENTED
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Convert Windows protection flags to POSIX
fn win_protect_to_posix(protect: u32) -> u32 {
    const PAGE_NOACCESS: u32 = 0x01;
    const PAGE_READONLY: u32 = 0x02;
    const PAGE_READWRITE: u32 = 0x04;
    const PAGE_WRITECOPY: u32 = 0x08;
    const PAGE_EXECUTE: u32 = 0x10;
    const PAGE_EXECUTE_READ: u32 = 0x20;
    const PAGE_EXECUTE_READWRITE: u32 = 0x40;

    const PROT_NONE: u32 = 0;
    const PROT_READ: u32 = 1;
    const PROT_WRITE: u32 = 2;
    const PROT_EXEC: u32 = 4;

    match protect & 0xFF {
        PAGE_NOACCESS => PROT_NONE,
        PAGE_READONLY => PROT_READ,
        PAGE_READWRITE | PAGE_WRITECOPY => PROT_READ | PROT_WRITE,
        PAGE_EXECUTE => PROT_EXEC,
        PAGE_EXECUTE_READ => PROT_READ | PROT_EXEC,
        PAGE_EXECUTE_READWRITE => PROT_READ | PROT_WRITE | PROT_EXEC,
        _ => PROT_READ | PROT_WRITE,
    }
}

/// Set I/O status block
fn set_io_status(io_status_block: u64, status_code: u32, information: u64) {
    if io_status_block != 0 {
        unsafe {
            *(io_status_block as *mut u32) = status_code;
            *((io_status_block + 8) as *mut u64) = information;
        }
    }
}

/// Read path from OBJECT_ATTRIBUTES structure
fn read_object_attributes_path(object_attributes: u64) -> Option<String> {
    if object_attributes == 0 {
        return None;
    }

    unsafe {
        // ObjectAttributes structure:
        // u32 Length
        // u64 RootDirectory
        // u64 ObjectName (pointer to UNICODE_STRING)
        // u32 Attributes
        // u64 SecurityDescriptor
        // u64 SecurityQualityOfService

        let object_name_ptr = *((object_attributes + 16) as *const u64);
        if object_name_ptr == 0 {
            return None;
        }

        // UNICODE_STRING:
        // u16 Length
        // u16 MaximumLength
        // u64 Buffer
        let length = *(object_name_ptr as *const u16) as usize;
        let buffer = *((object_name_ptr + 8) as *const u64);

        if buffer == 0 || length == 0 {
            return None;
        }

        // Read UTF-16 string
        let utf16_len = length / 2;
        let mut utf16 = Vec::with_capacity(utf16_len);
        for i in 0..utf16_len {
            let c = *((buffer + i as u64 * 2) as *const u16);
            utf16.push(c);
        }

        // Convert UTF-16 to String
        String::from_utf16(&utf16).ok()
    }
}

/// Get syscall statistics
pub fn get_stats() -> (u64, u64, u64, u64) {
    (
        STATS.total_calls.load(Ordering::Relaxed),
        STATS.successful_calls.load(Ordering::Relaxed),
        STATS.failed_calls.load(Ordering::Relaxed),
        STATS.unimplemented_calls.load(Ordering::Relaxed),
    )
}

/// Get syscall name from number
pub fn get_syscall_name(nr: u32) -> &'static str {
    match nr {
        syscall_nr::NT_TERMINATE_PROCESS => "NtTerminateProcess",
        syscall_nr::NT_CREATE_PROCESS => "NtCreateProcess",
        syscall_nr::NT_OPEN_PROCESS => "NtOpenProcess",
        syscall_nr::NT_QUERY_INFORMATION_PROCESS => "NtQueryInformationProcess",
        syscall_nr::NT_CREATE_THREAD_EX => "NtCreateThreadEx",
        syscall_nr::NT_TERMINATE_THREAD => "NtTerminateThread",
        syscall_nr::NT_ALLOCATE_VIRTUAL_MEMORY => "NtAllocateVirtualMemory",
        syscall_nr::NT_FREE_VIRTUAL_MEMORY => "NtFreeVirtualMemory",
        syscall_nr::NT_PROTECT_VIRTUAL_MEMORY => "NtProtectVirtualMemory",
        syscall_nr::NT_QUERY_VIRTUAL_MEMORY => "NtQueryVirtualMemory",
        syscall_nr::NT_CREATE_FILE => "NtCreateFile",
        syscall_nr::NT_READ_FILE => "NtReadFile",
        syscall_nr::NT_WRITE_FILE => "NtWriteFile",
        syscall_nr::NT_CLOSE => "NtClose",
        syscall_nr::NT_QUERY_INFORMATION_FILE => "NtQueryInformationFile",
        syscall_nr::NT_SET_INFORMATION_FILE => "NtSetInformationFile",
        syscall_nr::NT_QUERY_DIRECTORY_FILE => "NtQueryDirectoryFile",
        syscall_nr::NT_DELETE_FILE => "NtDeleteFile",
        syscall_nr::NT_CREATE_SECTION => "NtCreateSection",
        syscall_nr::NT_MAP_VIEW_OF_SECTION => "NtMapViewOfSection",
        syscall_nr::NT_UNMAP_VIEW_OF_SECTION => "NtUnmapViewOfSection",
        syscall_nr::NT_CREATE_EVENT => "NtCreateEvent",
        syscall_nr::NT_SET_EVENT => "NtSetEvent",
        syscall_nr::NT_RESET_EVENT => "NtResetEvent",
        syscall_nr::NT_WAIT_FOR_SINGLE_OBJECT => "NtWaitForSingleObject",
        syscall_nr::NT_WAIT_FOR_MULTIPLE_OBJECTS => "NtWaitForMultipleObjects",
        syscall_nr::NT_CREATE_MUTANT => "NtCreateMutant",
        syscall_nr::NT_RELEASE_MUTANT => "NtReleaseMutant",
        syscall_nr::NT_CREATE_SEMAPHORE => "NtCreateSemaphore",
        syscall_nr::NT_RELEASE_SEMAPHORE => "NtReleaseSemaphore",
        syscall_nr::NT_QUERY_SYSTEM_INFORMATION => "NtQuerySystemInformation",
        syscall_nr::NT_QUERY_PERFORMANCE_COUNTER => "NtQueryPerformanceCounter",
        syscall_nr::NT_DELAY_EXECUTION => "NtDelayExecution",
        syscall_nr::NT_YIELD_EXECUTION => "NtYieldExecution",
        syscall_nr::NT_QUERY_SYSTEM_TIME => "NtQuerySystemTime",
        syscall_nr::NT_OPEN_KEY => "NtOpenKey",
        syscall_nr::NT_QUERY_VALUE_KEY => "NtQueryValueKey",
        syscall_nr::NT_SET_VALUE_KEY => "NtSetValueKey",
        syscall_nr::NT_OPEN_PROCESS_TOKEN => "NtOpenProcessToken",
        syscall_nr::NT_QUERY_INFORMATION_TOKEN => "NtQueryInformationToken",
        _ => "Unknown",
    }
}

/// Initialize the NT syscall subsystem
pub fn init() {
    crate::kprintln!("ntsyscall: NT syscall dispatcher initialized");
    crate::kprintln!("ntsyscall: {} syscall handlers registered", 40);
}
