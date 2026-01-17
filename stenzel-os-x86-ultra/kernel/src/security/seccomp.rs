//! Seccomp - Secure Computing Mode
//!
//! Seccomp allows processes to restrict the system calls they can make.
//! This provides defense-in-depth security, limiting the damage that
//! can be done if a process is compromised.
//!
//! ## Modes
//! - **Disabled**: No filtering (default)
//! - **Strict**: Only read, write, exit, sigreturn allowed
//! - **Filter**: BPF filter determines allowed syscalls
//!
//! ## Usage
//! ```ignore
//! // Enable strict mode
//! seccomp::enable_strict();
//!
//! // Or use a filter
//! let filter = SeccompFilter::new()
//!     .allow(Syscall::Read)
//!     .allow(Syscall::Write)
//!     .allow(Syscall::Exit);
//! seccomp::set_filter(filter);
//! ```

#![allow(dead_code)]

use alloc::vec::Vec;
use alloc::boxed::Box;
use core::sync::atomic::{AtomicU32, Ordering};

use crate::util::{KError, KResult};

/// Seccomp mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SeccompMode {
    /// No filtering (default)
    Disabled = 0,
    /// Strict mode: only read, write, _exit, sigreturn
    Strict = 1,
    /// Filter mode: BPF filter
    Filter = 2,
}

/// Seccomp return action
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SeccompAction {
    /// Kill the process (SIGKILL)
    Kill = 0x00000000,
    /// Kill the thread (SIGKILL)
    KillThread = 0x10000000,
    /// Send SIGSYS with info
    Trap = 0x00030000,
    /// Return an errno
    Errno(u16) = 0x00050000,
    /// Allow call after logging
    Trace = 0x7ff00000,
    /// Log the call but allow it
    Log = 0x7ffc0000,
    /// Allow the syscall
    Allow = 0x7fff0000,
}

impl SeccompAction {
    /// Create an errno action
    pub const fn errno(errno: u16) -> Self {
        SeccompAction::Errno(errno)
    }

    /// Convert to raw value for filter
    pub fn to_raw(&self) -> u32 {
        match self {
            SeccompAction::Kill => 0x00000000,
            SeccompAction::KillThread => 0x10000000,
            SeccompAction::Trap => 0x00030000,
            SeccompAction::Errno(e) => 0x00050000 | (*e as u32),
            SeccompAction::Trace => 0x7ff00000,
            SeccompAction::Log => 0x7ffc0000,
            SeccompAction::Allow => 0x7fff0000,
        }
    }

    /// Parse from raw value
    pub fn from_raw(raw: u32) -> Self {
        let action = raw & 0xffff0000;
        match action {
            0x00000000 => SeccompAction::Kill,
            0x10000000 => SeccompAction::KillThread,
            0x00030000 => SeccompAction::Trap,
            0x00050000 => SeccompAction::Errno((raw & 0xffff) as u16),
            0x7ff00000 => SeccompAction::Trace,
            0x7ffc0000 => SeccompAction::Log,
            0x7fff0000 => SeccompAction::Allow,
            _ => SeccompAction::Kill, // Unknown -> kill
        }
    }
}

/// Seccomp data passed to BPF filter
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct SeccompData {
    /// System call number
    pub nr: i32,
    /// AUDIT_ARCH value
    pub arch: u32,
    /// Instruction pointer at time of syscall
    pub instruction_pointer: u64,
    /// Syscall arguments (6 total)
    pub args: [u64; 6],
}

/// Architecture for seccomp
pub const AUDIT_ARCH_X86_64: u32 = 0xc000003e;

/// BPF instruction
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BpfInsn {
    /// Operation code
    pub code: u16,
    /// Jump if true offset
    pub jt: u8,
    /// Jump if false offset
    pub jf: u8,
    /// Constant or memory offset
    pub k: u32,
}

impl BpfInsn {
    /// Create a new BPF instruction
    pub const fn new(code: u16, jt: u8, jf: u8, k: u32) -> Self {
        Self { code, jt, jf, k }
    }
}

// BPF instruction classes
pub const BPF_LD: u16 = 0x00;
pub const BPF_LDX: u16 = 0x01;
pub const BPF_ST: u16 = 0x02;
pub const BPF_STX: u16 = 0x03;
pub const BPF_ALU: u16 = 0x04;
pub const BPF_JMP: u16 = 0x05;
pub const BPF_RET: u16 = 0x06;
pub const BPF_MISC: u16 = 0x07;

// BPF LD/LDX fields
pub const BPF_W: u16 = 0x00;
pub const BPF_H: u16 = 0x08;
pub const BPF_B: u16 = 0x10;
pub const BPF_ABS: u16 = 0x20;
pub const BPF_MEM: u16 = 0x60;

// BPF JMP fields
pub const BPF_JA: u16 = 0x00;
pub const BPF_JEQ: u16 = 0x10;
pub const BPF_JGT: u16 = 0x20;
pub const BPF_JGE: u16 = 0x30;
pub const BPF_JSET: u16 = 0x40;
pub const BPF_K: u16 = 0x00;
pub const BPF_X: u16 = 0x08;

/// Helper macro for BPF instructions
macro_rules! bpf_stmt {
    ($code:expr, $k:expr) => {
        BpfInsn::new($code, 0, 0, $k)
    };
}

macro_rules! bpf_jump {
    ($code:expr, $k:expr, $jt:expr, $jf:expr) => {
        BpfInsn::new($code, $jt, $jf, $k)
    };
}

/// Seccomp BPF filter
#[derive(Debug, Clone)]
pub struct SeccompFilter {
    /// BPF program instructions
    instructions: Vec<BpfInsn>,
    /// Default action for unmatched syscalls
    default_action: SeccompAction,
}

impl SeccompFilter {
    /// Create a new filter with default KILL action
    pub fn new() -> Self {
        Self {
            instructions: Vec::new(),
            default_action: SeccompAction::Kill,
        }
    }

    /// Set default action for unmatched syscalls
    pub fn default_action(mut self, action: SeccompAction) -> Self {
        self.default_action = action;
        self
    }

    /// Allow a specific syscall
    pub fn allow(mut self, syscall_nr: u64) -> Self {
        self.add_rule(syscall_nr, SeccompAction::Allow);
        self
    }

    /// Deny a specific syscall with errno
    pub fn deny(mut self, syscall_nr: u64, errno: u16) -> Self {
        self.add_rule(syscall_nr, SeccompAction::errno(errno));
        self
    }

    /// Add a rule for a syscall
    pub fn add_rule(&mut self, syscall_nr: u64, action: SeccompAction) {
        // This simplified implementation stores rules, actual BPF generation
        // would happen in build()
        // For now, we use a direct rule storage approach
    }

    /// Build the BPF program
    pub fn build(&self) -> Vec<BpfInsn> {
        let mut prog = Vec::new();

        // Validate architecture (x86_64)
        // Load architecture
        prog.push(bpf_stmt!(BPF_LD | BPF_W | BPF_ABS, 4)); // offsetof(seccomp_data, arch)
        // Jump if arch != x86_64
        prog.push(bpf_jump!(BPF_JMP | BPF_JEQ | BPF_K, AUDIT_ARCH_X86_64, 1, 0));
        // Kill if wrong architecture
        prog.push(bpf_stmt!(BPF_RET | BPF_K, SeccompAction::Kill.to_raw()));

        // Load syscall number
        prog.push(bpf_stmt!(BPF_LD | BPF_W | BPF_ABS, 0)); // offsetof(seccomp_data, nr)

        // Default action at the end
        prog.push(bpf_stmt!(BPF_RET | BPF_K, self.default_action.to_raw()));

        prog
    }
}

impl Default for SeccompFilter {
    fn default() -> Self {
        Self::new()
    }
}

/// Seccomp state for a process
#[derive(Debug, Clone)]
pub struct SeccompState {
    /// Current mode
    mode: SeccompMode,
    /// Filter (if mode == Filter)
    filter: Option<Box<SeccompFilter>>,
    /// Filter flags
    flags: SeccompFlags,
}

bitflags::bitflags! {
    /// Seccomp filter flags
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct SeccompFlags: u32 {
        /// Filter should be inherited across fork
        const TSYNC = 1 << 0;
        /// Log all filtered syscalls
        const LOG = 1 << 1;
        /// Disable speculation store bypass mitigation
        const SPEC_ALLOW = 1 << 2;
        /// Create a listener fd for filter notifications
        const NEW_LISTENER = 1 << 3;
        /// Wait for tracer
        const WAIT_KILLABLE_RECV = 1 << 4;
    }
}

impl SeccompState {
    /// Create new disabled seccomp state
    pub const fn new() -> Self {
        Self {
            mode: SeccompMode::Disabled,
            filter: None,
            flags: SeccompFlags::empty(),
        }
    }

    /// Get current mode
    pub fn mode(&self) -> SeccompMode {
        self.mode
    }

    /// Is seccomp enabled?
    pub fn is_enabled(&self) -> bool {
        self.mode != SeccompMode::Disabled
    }

    /// Enable strict mode
    pub fn enable_strict(&mut self) -> KResult<()> {
        if self.mode != SeccompMode::Disabled {
            return Err(KError::PermissionDenied);
        }
        self.mode = SeccompMode::Strict;
        Ok(())
    }

    /// Set filter mode
    pub fn set_filter(&mut self, filter: SeccompFilter, flags: SeccompFlags) -> KResult<()> {
        // Can add filters when disabled or already in filter mode
        if self.mode == SeccompMode::Strict {
            return Err(KError::Invalid);
        }

        self.mode = SeccompMode::Filter;
        self.filter = Some(Box::new(filter));
        self.flags = flags;
        Ok(())
    }

    /// Check if a syscall is allowed
    pub fn check_syscall(&self, data: &SeccompData) -> SeccompAction {
        match self.mode {
            SeccompMode::Disabled => SeccompAction::Allow,
            SeccompMode::Strict => self.check_strict(data.nr as u64),
            SeccompMode::Filter => self.run_filter(data),
        }
    }

    /// Check syscall in strict mode
    fn check_strict(&self, syscall_nr: u64) -> SeccompAction {
        // Strict mode only allows: read, write, exit, exit_group, sigreturn, rt_sigreturn
        match syscall_nr {
            0 => SeccompAction::Allow,  // read
            1 => SeccompAction::Allow,  // write
            60 => SeccompAction::Allow, // exit
            231 => SeccompAction::Allow, // exit_group
            15 => SeccompAction::Allow, // rt_sigreturn
            _ => SeccompAction::Kill,
        }
    }

    /// Run BPF filter
    fn run_filter(&self, data: &SeccompData) -> SeccompAction {
        let filter = match &self.filter {
            Some(f) => f,
            None => return SeccompAction::Allow,
        };

        // Simple filter execution
        // A full implementation would run the BPF program
        // For now, return the default action
        filter.default_action
    }
}

impl Default for SeccompState {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// Simple rule-based filter (alternative to full BPF)
// ============================================================

/// Simplified syscall rule
#[derive(Debug, Clone, Copy)]
pub struct SyscallRule {
    /// Syscall number
    pub nr: u64,
    /// Action to take
    pub action: SeccompAction,
}

/// Simple rule-based filter (easier to use than full BPF)
#[derive(Debug, Clone)]
pub struct SimpleFilter {
    /// Rules for specific syscalls
    rules: Vec<SyscallRule>,
    /// Default action
    default_action: SeccompAction,
}

impl SimpleFilter {
    /// Create a new filter
    pub fn new(default_action: SeccompAction) -> Self {
        Self {
            rules: Vec::new(),
            default_action,
        }
    }

    /// Create a filter that allows everything by default
    pub fn allow_all() -> Self {
        Self::new(SeccompAction::Allow)
    }

    /// Create a filter that kills on unrecognized syscalls
    pub fn deny_all() -> Self {
        Self::new(SeccompAction::Kill)
    }

    /// Add a rule to allow a syscall
    pub fn allow(&mut self, nr: u64) {
        self.rules.push(SyscallRule {
            nr,
            action: SeccompAction::Allow,
        });
    }

    /// Add a rule to deny a syscall
    pub fn deny(&mut self, nr: u64) {
        self.rules.push(SyscallRule {
            nr,
            action: SeccompAction::Kill,
        });
    }

    /// Add a rule to return an error
    pub fn error(&mut self, nr: u64, errno: u16) {
        self.rules.push(SyscallRule {
            nr,
            action: SeccompAction::errno(errno),
        });
    }

    /// Check a syscall against the filter
    pub fn check(&self, nr: u64) -> SeccompAction {
        for rule in &self.rules {
            if rule.nr == nr {
                return rule.action;
            }
        }
        self.default_action
    }

    /// Get number of rules
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

impl Default for SimpleFilter {
    fn default() -> Self {
        Self::allow_all()
    }
}

// ============================================================
// Syscall interface
// ============================================================

/// prctl() seccomp operations
pub const PR_SET_SECCOMP: i32 = 22;
pub const PR_GET_SECCOMP: i32 = 21;

/// seccomp() syscall operations
pub const SECCOMP_SET_MODE_STRICT: u32 = 0;
pub const SECCOMP_SET_MODE_FILTER: u32 = 1;
pub const SECCOMP_GET_ACTION_AVAIL: u32 = 2;
pub const SECCOMP_GET_NOTIF_SIZES: u32 = 3;

/// BPF program for seccomp syscall
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SockFprog {
    /// Number of instructions
    pub len: u16,
    /// Pointer to filter
    pub filter: u64, // Actually *const BpfInsn, but we use u64 for userspace
}

/// sys_seccomp implementation
pub fn sys_seccomp(op: u32, flags: u32, args: u64) -> KResult<i64> {
    match op {
        SECCOMP_SET_MODE_STRICT => {
            if flags != 0 {
                return Err(KError::Invalid);
            }
            enable_strict()?;
            Ok(0)
        }
        SECCOMP_SET_MODE_FILTER => {
            // Would need to copy BPF program from userspace
            // For now, not fully implemented
            Err(KError::NotSupported)
        }
        SECCOMP_GET_ACTION_AVAIL => {
            // Check if an action is available
            let action = args as u32;
            match SeccompAction::from_raw(action) {
                SeccompAction::Kill
                | SeccompAction::KillThread
                | SeccompAction::Trap
                | SeccompAction::Errno(_)
                | SeccompAction::Log
                | SeccompAction::Allow => Ok(0),
                _ => Err(KError::Invalid),
            }
        }
        _ => Err(KError::Invalid),
    }
}

/// Enable strict mode for current task
pub fn enable_strict() -> KResult<()> {
    let task = crate::sched::current_task();
    let mut state = task.seccomp_state();
    state.enable_strict()?;
    task.set_seccomp_state(state);
    Ok(())
}

/// Check if a syscall is allowed for current task
pub fn check_syscall(nr: u64, args: &[u64; 6]) -> SeccompAction {
    let task = crate::sched::current_task();
    let state = task.seccomp_state();

    if !state.is_enabled() {
        return SeccompAction::Allow;
    }

    let data = SeccompData {
        nr: nr as i32,
        arch: AUDIT_ARCH_X86_64,
        instruction_pointer: 0, // Would need to get from trap frame
        args: *args,
    };

    state.check_syscall(&data)
}

/// Predefined filter: minimal filter for sandboxed processes
pub fn minimal_filter() -> SimpleFilter {
    let mut filter = SimpleFilter::deny_all();

    // Allow basic I/O
    filter.allow(0);   // read
    filter.allow(1);   // write
    filter.allow(3);   // close

    // Allow process control
    filter.allow(60);  // exit
    filter.allow(231); // exit_group

    // Allow signals
    filter.allow(15);  // rt_sigreturn
    filter.allow(13);  // rt_sigaction
    filter.allow(14);  // rt_sigprocmask

    // Allow memory operations
    filter.allow(9);   // mmap
    filter.allow(10);  // mprotect
    filter.allow(11);  // munmap
    filter.allow(12);  // brk

    filter
}

/// Predefined filter: common operations for most programs
pub fn standard_filter() -> SimpleFilter {
    let mut filter = minimal_filter();

    // File operations
    filter.allow(2);   // open
    filter.allow(257); // openat
    filter.allow(4);   // stat
    filter.allow(5);   // fstat
    filter.allow(6);   // lstat
    filter.allow(8);   // lseek
    filter.allow(17);  // pread64
    filter.allow(18);  // pwrite64
    filter.allow(72);  // fcntl
    filter.allow(79);  // getcwd
    filter.allow(80);  // chdir

    // Directory operations
    filter.allow(78);  // getdents
    filter.allow(217); // getdents64

    // Process info
    filter.allow(39);  // getpid
    filter.allow(110); // getppid
    filter.allow(102); // getuid
    filter.allow(104); // getgid
    filter.allow(107); // geteuid
    filter.allow(108); // getegid

    // Time
    filter.allow(228); // clock_gettime
    filter.allow(96);  // gettimeofday

    // Futex
    filter.allow(202); // futex

    filter
}

/// Format seccomp state for display
pub fn format_state(state: &SeccompState) -> alloc::string::String {
    use alloc::format;

    let mode_str = match state.mode() {
        SeccompMode::Disabled => "disabled",
        SeccompMode::Strict => "strict",
        SeccompMode::Filter => "filter",
    };

    format!("Seccomp: mode={}", mode_str)
}
