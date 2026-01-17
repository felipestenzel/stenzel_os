//! Linux-compatible Capabilities
//!
//! Implements POSIX.1e capabilities for fine-grained privilege control.
//! Instead of the all-or-nothing root model, capabilities allow processes
//! to have only the specific privileges they need.
//!
//! ## Capability Sets
//! Each process has three capability sets:
//! - **Effective**: capabilities currently in effect for permission checks
//! - **Permitted**: upper bound of capabilities the process can use
//! - **Inheritable**: capabilities preserved across execve
//!
//! Plus:
//! - **Bounding**: caps that can be gained through execve of privileged programs
//!
//! ## File Capabilities
//! Executables can have capabilities attached:
//! - Permitted: caps added to process permitted set
//! - Inheritable: caps that can be inherited if process also has them
//! - Effective: flag indicating permitted caps should be in effective set
//!
//! ## Usage
//! ```ignore
//! // Check if process has a capability
//! if caps::has_cap_effective(Cap::CAP_NET_BIND_SERVICE) {
//!     // Can bind to privileged ports
//! }
//!
//! // Drop a capability
//! caps::drop_cap(Cap::CAP_SYS_ADMIN)?;
//! ```

#![allow(dead_code)]

use core::sync::atomic::{AtomicU64, Ordering};
use bitflags::bitflags;

use crate::util::{KError, KResult};

/// Capability version for syscall interface
pub const _LINUX_CAPABILITY_VERSION_1: u32 = 0x19980330;
pub const _LINUX_CAPABILITY_VERSION_2: u32 = 0x20071026;
pub const _LINUX_CAPABILITY_VERSION_3: u32 = 0x20080522;
pub const LINUX_CAPABILITY_VERSION: u32 = _LINUX_CAPABILITY_VERSION_3;

/// Maximum capability number
pub const CAP_LAST_CAP: u32 = 40;

bitflags! {
    /// Linux-compatible capability bits
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CapSet: u64 {
        /// Bypass file read, write, and execute permission checks
        const CAP_CHOWN             = 1 << 0;
        /// Bypass file permission checks for owner (except when CAP_FSETID applies)
        const CAP_DAC_OVERRIDE      = 1 << 1;
        /// Bypass file read permission checks and directory read+execute checks
        const CAP_DAC_READ_SEARCH   = 1 << 2;
        /// Bypass permission checks on operations that normally require process UID
        const CAP_FOWNER            = 1 << 3;
        /// Don't clear set-user-ID and set-group-ID bits when a file is modified
        const CAP_FSETID            = 1 << 4;
        /// Bypass permission checks for sending signals
        const CAP_KILL              = 1 << 5;
        /// Make arbitrary manipulations of process GIDs and supplementary GID list
        const CAP_SETGID            = 1 << 6;
        /// Make arbitrary manipulations of process UIDs
        const CAP_SETUID            = 1 << 7;
        /// Transfer capabilities to other processes
        const CAP_SETPCAP           = 1 << 8;
        /// Bypass file extended attribute restrictions
        const CAP_LINUX_IMMUTABLE   = 1 << 9;
        /// Bind to privileged ports (port numbers less than 1024)
        const CAP_NET_BIND_SERVICE  = 1 << 10;
        /// Broadcast, listen to multicast
        const CAP_NET_BROADCAST     = 1 << 11;
        /// Perform various network-related operations
        const CAP_NET_ADMIN         = 1 << 12;
        /// Use RAW and PACKET sockets
        const CAP_NET_RAW           = 1 << 13;
        /// Lock memory (mlock, mlockall, mmap, shmctl)
        const CAP_IPC_LOCK          = 1 << 14;
        /// Bypass permission checks for System V IPC operations
        const CAP_IPC_OWNER         = 1 << 15;
        /// Load and unload kernel modules
        const CAP_SYS_MODULE        = 1 << 16;
        /// Perform I/O port operations (iopl, ioperm)
        const CAP_SYS_RAWIO         = 1 << 17;
        /// Use chroot()
        const CAP_SYS_CHROOT        = 1 << 18;
        /// Trace arbitrary processes using ptrace
        const CAP_SYS_PTRACE        = 1 << 19;
        /// Use acct()
        const CAP_SYS_PACCT         = 1 << 20;
        /// Perform system administration operations
        const CAP_SYS_ADMIN         = 1 << 21;
        /// Use reboot(), kexec_load()
        const CAP_SYS_BOOT          = 1 << 22;
        /// Raise process nice value, change nice value for arbitrary processes
        const CAP_SYS_NICE          = 1 << 23;
        /// Override resource limits
        const CAP_SYS_RESOURCE      = 1 << 24;
        /// Set system time, set real-time clock
        const CAP_SYS_TIME          = 1 << 25;
        /// Perform privileged terminal operations
        const CAP_SYS_TTY_CONFIG    = 1 << 26;
        /// Create special files using mknod()
        const CAP_MKNOD             = 1 << 27;
        /// Establish leases on arbitrary files
        const CAP_LEASE             = 1 << 28;
        /// Write records to kernel auditing log
        const CAP_AUDIT_WRITE       = 1 << 29;
        /// Configure audit subsystem
        const CAP_AUDIT_CONTROL     = 1 << 30;
        /// Set file capabilities
        const CAP_SETFCAP           = 1 << 31;
        /// Override Mandatory Access Control (MAC)
        const CAP_MAC_OVERRIDE      = 1 << 32;
        /// Allow MAC configuration or state changes
        const CAP_MAC_ADMIN         = 1 << 33;
        /// Configure syslog
        const CAP_SYSLOG            = 1 << 34;
        /// Trigger wake_alarm
        const CAP_WAKE_ALARM        = 1 << 35;
        /// Bypass kernel block suspend
        const CAP_BLOCK_SUSPEND     = 1 << 36;
        /// Read audit log via netlink socket
        const CAP_AUDIT_READ        = 1 << 37;
        /// Bypass permission checks for sendto/recvfrom
        const CAP_PERFMON           = 1 << 38;
        /// Use BPF programs
        const CAP_BPF               = 1 << 39;
        /// Checkpoint/restore
        const CAP_CHECKPOINT_RESTORE = 1 << 40;

        /// All capabilities
        const ALL = (1 << 41) - 1;
    }
}

/// Individual capability as an enum (for API clarity)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Cap {
    Chown = 0,
    DacOverride = 1,
    DacReadSearch = 2,
    Fowner = 3,
    Fsetid = 4,
    Kill = 5,
    Setgid = 6,
    Setuid = 7,
    Setpcap = 8,
    LinuxImmutable = 9,
    NetBindService = 10,
    NetBroadcast = 11,
    NetAdmin = 12,
    NetRaw = 13,
    IpcLock = 14,
    IpcOwner = 15,
    SysModule = 16,
    SysRawio = 17,
    SysChroot = 18,
    SysPtrace = 19,
    SysPacct = 20,
    SysAdmin = 21,
    SysBoot = 22,
    SysNice = 23,
    SysResource = 24,
    SysTime = 25,
    SysTtyConfig = 26,
    Mknod = 27,
    Lease = 28,
    AuditWrite = 29,
    AuditControl = 30,
    Setfcap = 31,
    MacOverride = 32,
    MacAdmin = 33,
    Syslog = 34,
    WakeAlarm = 35,
    BlockSuspend = 36,
    AuditRead = 37,
    Perfmon = 38,
    Bpf = 39,
    CheckpointRestore = 40,
}

impl Cap {
    /// Convert capability number to Cap enum
    pub fn from_number(n: u32) -> Option<Self> {
        if n > CAP_LAST_CAP {
            return None;
        }
        // Safety: n is checked to be in valid range
        Some(unsafe { core::mem::transmute(n) })
    }

    /// Convert to CapSet bitmask
    pub fn to_set(self) -> CapSet {
        CapSet::from_bits_truncate(1u64 << (self as u32))
    }

    /// Get capability name
    pub fn name(&self) -> &'static str {
        match self {
            Cap::Chown => "CAP_CHOWN",
            Cap::DacOverride => "CAP_DAC_OVERRIDE",
            Cap::DacReadSearch => "CAP_DAC_READ_SEARCH",
            Cap::Fowner => "CAP_FOWNER",
            Cap::Fsetid => "CAP_FSETID",
            Cap::Kill => "CAP_KILL",
            Cap::Setgid => "CAP_SETGID",
            Cap::Setuid => "CAP_SETUID",
            Cap::Setpcap => "CAP_SETPCAP",
            Cap::LinuxImmutable => "CAP_LINUX_IMMUTABLE",
            Cap::NetBindService => "CAP_NET_BIND_SERVICE",
            Cap::NetBroadcast => "CAP_NET_BROADCAST",
            Cap::NetAdmin => "CAP_NET_ADMIN",
            Cap::NetRaw => "CAP_NET_RAW",
            Cap::IpcLock => "CAP_IPC_LOCK",
            Cap::IpcOwner => "CAP_IPC_OWNER",
            Cap::SysModule => "CAP_SYS_MODULE",
            Cap::SysRawio => "CAP_SYS_RAWIO",
            Cap::SysChroot => "CAP_SYS_CHROOT",
            Cap::SysPtrace => "CAP_SYS_PTRACE",
            Cap::SysPacct => "CAP_SYS_PACCT",
            Cap::SysAdmin => "CAP_SYS_ADMIN",
            Cap::SysBoot => "CAP_SYS_BOOT",
            Cap::SysNice => "CAP_SYS_NICE",
            Cap::SysResource => "CAP_SYS_RESOURCE",
            Cap::SysTime => "CAP_SYS_TIME",
            Cap::SysTtyConfig => "CAP_SYS_TTY_CONFIG",
            Cap::Mknod => "CAP_MKNOD",
            Cap::Lease => "CAP_LEASE",
            Cap::AuditWrite => "CAP_AUDIT_WRITE",
            Cap::AuditControl => "CAP_AUDIT_CONTROL",
            Cap::Setfcap => "CAP_SETFCAP",
            Cap::MacOverride => "CAP_MAC_OVERRIDE",
            Cap::MacAdmin => "CAP_MAC_ADMIN",
            Cap::Syslog => "CAP_SYSLOG",
            Cap::WakeAlarm => "CAP_WAKE_ALARM",
            Cap::BlockSuspend => "CAP_BLOCK_SUSPEND",
            Cap::AuditRead => "CAP_AUDIT_READ",
            Cap::Perfmon => "CAP_PERFMON",
            Cap::Bpf => "CAP_BPF",
            Cap::CheckpointRestore => "CAP_CHECKPOINT_RESTORE",
        }
    }
}

/// Process capability state
#[derive(Debug, Clone, Copy)]
pub struct ProcessCaps {
    /// Effective capabilities (used for permission checks)
    pub effective: CapSet,
    /// Permitted capabilities (maximum capabilities process can use)
    pub permitted: CapSet,
    /// Inheritable capabilities (preserved across execve)
    pub inheritable: CapSet,
    /// Bounding set (limits capabilities that can be gained)
    pub bounding: CapSet,
    /// Ambient capabilities (automatically raised in effective on execve)
    pub ambient: CapSet,
}

impl ProcessCaps {
    /// Create empty capability state (no capabilities)
    pub const fn empty() -> Self {
        Self {
            effective: CapSet::empty(),
            permitted: CapSet::empty(),
            inheritable: CapSet::empty(),
            bounding: CapSet::ALL,
            ambient: CapSet::empty(),
        }
    }

    /// Create full capability state (all capabilities - root-like)
    pub const fn full() -> Self {
        Self {
            effective: CapSet::ALL,
            permitted: CapSet::ALL,
            inheritable: CapSet::empty(),
            bounding: CapSet::ALL,
            ambient: CapSet::empty(),
        }
    }

    /// Create capability state for root user
    pub fn root() -> Self {
        Self::full()
    }

    /// Create capability state for non-root user
    pub fn user() -> Self {
        Self::empty()
    }

    /// Check if a capability is in the effective set
    pub fn has_effective(&self, cap: Cap) -> bool {
        self.effective.contains(cap.to_set())
    }

    /// Check if a capability is in the permitted set
    pub fn has_permitted(&self, cap: Cap) -> bool {
        self.permitted.contains(cap.to_set())
    }

    /// Check if a capability is in the bounding set
    pub fn has_bounding(&self, cap: Cap) -> bool {
        self.bounding.contains(cap.to_set())
    }

    /// Raise a capability to effective (if in permitted)
    pub fn raise(&mut self, cap: Cap) -> KResult<()> {
        let cap_set = cap.to_set();
        if !self.permitted.contains(cap_set) {
            return Err(KError::PermissionDenied);
        }
        self.effective |= cap_set;
        Ok(())
    }

    /// Drop a capability from effective
    pub fn drop(&mut self, cap: Cap) {
        self.effective &= !cap.to_set();
    }

    /// Drop a capability from permitted (and effective)
    pub fn drop_permitted(&mut self, cap: Cap) {
        let cap_set = cap.to_set();
        self.permitted &= !cap_set;
        self.effective &= !cap_set;
    }

    /// Drop a capability from bounding set
    pub fn drop_bounding(&mut self, cap: Cap) -> KResult<()> {
        // Need CAP_SETPCAP to modify bounding set
        if !self.has_effective(Cap::Setpcap) {
            return Err(KError::PermissionDenied);
        }
        self.bounding &= !cap.to_set();
        Ok(())
    }

    /// Set inheritable capability
    pub fn set_inheritable(&mut self, cap: Cap, value: bool) -> KResult<()> {
        let cap_set = cap.to_set();

        if value {
            // To add to inheritable, must be in permitted and bounding
            if !self.permitted.contains(cap_set) || !self.bounding.contains(cap_set) {
                return Err(KError::PermissionDenied);
            }
            self.inheritable |= cap_set;
        } else {
            self.inheritable &= !cap_set;
        }
        Ok(())
    }

    /// Set ambient capability
    pub fn set_ambient(&mut self, cap: Cap, value: bool) -> KResult<()> {
        let cap_set = cap.to_set();

        if value {
            // To set ambient, must be in both permitted and inheritable
            if !self.permitted.contains(cap_set) || !self.inheritable.contains(cap_set) {
                return Err(KError::PermissionDenied);
            }
            self.ambient |= cap_set;
        } else {
            self.ambient &= !cap_set;
        }
        Ok(())
    }

    /// Clear all ambient capabilities
    pub fn clear_ambient(&mut self) {
        self.ambient = CapSet::empty();
    }

    /// Transform capabilities for execve
    /// Returns new capability state for the new process image
    pub fn transform_for_exec(&self, file_caps: Option<&FileCaps>, is_setuid_root: bool) -> Self {
        // New capability calculation based on Linux capability rules
        // P'(effective) = file(effective) ? P'(permitted) : P'(ambient)
        // P'(permitted) = (P(inheritable) & F(inheritable)) | (F(permitted) & P(bounding)) | P(ambient)
        // P'(inheritable) = P(inheritable)
        // P'(bounding) = P(bounding)
        // P'(ambient) = (file has capabilities) ? 0 : P(ambient)

        let (file_permitted, file_inheritable, file_effective) = match file_caps {
            Some(fc) => (fc.permitted, fc.inheritable, fc.effective),
            None => (CapSet::empty(), CapSet::empty(), false),
        };

        let new_permitted = (self.inheritable & file_inheritable)
            | (file_permitted & self.bounding)
            | self.ambient;

        let new_effective = if file_effective || is_setuid_root {
            new_permitted
        } else {
            self.ambient
        };

        let new_ambient = if file_caps.is_some() {
            CapSet::empty()
        } else {
            self.ambient & new_permitted
        };

        Self {
            effective: new_effective,
            permitted: new_permitted,
            inheritable: self.inheritable,
            bounding: self.bounding,
            ambient: new_ambient,
        }
    }
}

impl Default for ProcessCaps {
    fn default() -> Self {
        Self::empty()
    }
}

/// File capabilities (attached to executable files)
pub struct FileCaps {
    /// Permitted capabilities (added to process)
    pub permitted: CapSet,
    /// Inheritable capabilities (and'd with process inheritable)
    pub inheritable: CapSet,
    /// If true, permitted caps are also made effective
    pub effective: bool,
    /// Root-UID for the capabilities (usually 0)
    pub rootid: u32,
}

impl FileCaps {
    /// Create empty file capabilities
    pub const fn empty() -> Self {
        Self {
            permitted: CapSet::empty(),
            inheritable: CapSet::empty(),
            effective: false,
            rootid: 0,
        }
    }

    /// Parse from xattr value (VFS_CAP_REVISION_2 or _3 format)
    pub fn from_xattr(data: &[u8]) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }

        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let version = magic & 0xFF000000;

        match version {
            0x02000000 => Self::parse_v2(data),
            0x03000000 => Self::parse_v3(data),
            _ => None,
        }
    }

    fn parse_v2(data: &[u8]) -> Option<Self> {
        // VFS_CAP_REVISION_2: 20 bytes
        // [0-3]: magic | effective_flag
        // [4-7]: permitted low
        // [8-11]: inheritable low
        // [12-15]: permitted high
        // [16-19]: inheritable high
        if data.len() < 20 {
            return None;
        }

        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let effective = (magic & 0x01) != 0;

        let permitted_lo = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let inheritable_lo = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
        let permitted_hi = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
        let inheritable_hi = u32::from_le_bytes([data[16], data[17], data[18], data[19]]);

        let permitted = (permitted_hi as u64) << 32 | permitted_lo as u64;
        let inheritable = (inheritable_hi as u64) << 32 | inheritable_lo as u64;

        Some(Self {
            permitted: CapSet::from_bits_truncate(permitted),
            inheritable: CapSet::from_bits_truncate(inheritable),
            effective,
            rootid: 0,
        })
    }

    fn parse_v3(data: &[u8]) -> Option<Self> {
        // VFS_CAP_REVISION_3: 24 bytes (v2 + rootid)
        if data.len() < 24 {
            return None;
        }

        let mut caps = Self::parse_v2(data)?;
        caps.rootid = u32::from_le_bytes([data[20], data[21], data[22], data[23]]);
        Some(caps)
    }

    /// Serialize to xattr value (VFS_CAP_REVISION_2 format)
    pub fn to_xattr(&self) -> [u8; 20] {
        let mut buf = [0u8; 20];

        let magic = 0x02000000u32 | if self.effective { 1 } else { 0 };
        buf[0..4].copy_from_slice(&magic.to_le_bytes());

        let permitted = self.permitted.bits();
        let inheritable = self.inheritable.bits();

        buf[4..8].copy_from_slice(&(permitted as u32).to_le_bytes());
        buf[8..12].copy_from_slice(&(inheritable as u32).to_le_bytes());
        buf[12..16].copy_from_slice(&((permitted >> 32) as u32).to_le_bytes());
        buf[16..20].copy_from_slice(&((inheritable >> 32) as u32).to_le_bytes());

        buf
    }
}

/// Capability header for capget/capset syscalls
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CapUserHeader {
    pub version: u32,
    pub pid: i32,
}

/// Capability data for capget/capset syscalls
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CapUserData {
    pub effective: u32,
    pub permitted: u32,
    pub inheritable: u32,
}

// ============================================================
// Syscall implementations
// ============================================================

/// capget syscall implementation
pub fn sys_capget(header: &CapUserHeader, data: &mut [CapUserData]) -> KResult<()> {
    // Validate version
    if header.version != _LINUX_CAPABILITY_VERSION_1
        && header.version != _LINUX_CAPABILITY_VERSION_2
        && header.version != _LINUX_CAPABILITY_VERSION_3
    {
        return Err(KError::Invalid);
    }

    // Get target process caps
    let caps = if header.pid == 0 {
        // Current process
        current_caps()
    } else {
        // Other process (requires CAP_SYS_PTRACE for arbitrary processes)
        get_process_caps(header.pid as u32)?
    };

    // Fill data
    let num_words = if header.version == _LINUX_CAPABILITY_VERSION_1 { 1 } else { 2 };

    if data.len() < num_words {
        return Err(KError::Invalid);
    }

    // Low 32 bits
    data[0].effective = caps.effective.bits() as u32;
    data[0].permitted = caps.permitted.bits() as u32;
    data[0].inheritable = caps.inheritable.bits() as u32;

    // High 32 bits (v2/v3)
    if num_words > 1 {
        data[1].effective = (caps.effective.bits() >> 32) as u32;
        data[1].permitted = (caps.permitted.bits() >> 32) as u32;
        data[1].inheritable = (caps.inheritable.bits() >> 32) as u32;
    }

    Ok(())
}

/// capset syscall implementation
pub fn sys_capset(header: &CapUserHeader, data: &[CapUserData]) -> KResult<()> {
    // Validate version
    if header.version != _LINUX_CAPABILITY_VERSION_1
        && header.version != _LINUX_CAPABILITY_VERSION_2
        && header.version != _LINUX_CAPABILITY_VERSION_3
    {
        return Err(KError::Invalid);
    }

    // Can only set own caps (pid 0 or own pid)
    if header.pid != 0 {
        let current_pid = crate::sched::current_pid();
        if header.pid as u64 != current_pid {
            return Err(KError::PermissionDenied);
        }
    }

    let num_words = if header.version == _LINUX_CAPABILITY_VERSION_1 { 1 } else { 2 };

    if data.len() < num_words {
        return Err(KError::Invalid);
    }

    // Build new capability sets
    let new_effective = if num_words > 1 {
        CapSet::from_bits_truncate((data[1].effective as u64) << 32 | data[0].effective as u64)
    } else {
        CapSet::from_bits_truncate(data[0].effective as u64)
    };

    let new_permitted = if num_words > 1 {
        CapSet::from_bits_truncate((data[1].permitted as u64) << 32 | data[0].permitted as u64)
    } else {
        CapSet::from_bits_truncate(data[0].permitted as u64)
    };

    let new_inheritable = if num_words > 1 {
        CapSet::from_bits_truncate((data[1].inheritable as u64) << 32 | data[0].inheritable as u64)
    } else {
        CapSet::from_bits_truncate(data[0].inheritable as u64)
    };

    // Apply changes
    set_current_caps(new_effective, new_permitted, new_inheritable)
}

// ============================================================
// Internal helpers
// ============================================================

/// Get current process capabilities
fn current_caps() -> ProcessCaps {
    // Get from current task
    let task = crate::sched::current_task();
    task.caps()
}

/// Get capabilities of another process
fn get_process_caps(pid: u32) -> KResult<ProcessCaps> {
    // Check if we have permission
    let current = current_caps();
    let current_pid = crate::sched::current_pid();

    if !current.has_effective(Cap::SysPtrace) {
        // Can only get caps of own process without CAP_SYS_PTRACE
        if pid as u64 != current_pid {
            return Err(KError::PermissionDenied);
        }
    }

    // For now, only support getting own caps
    // A full implementation would look up the task by PID
    if pid as u64 == current_pid || pid == 0 {
        Ok(current_caps())
    } else {
        // Would need to implement task lookup by PID
        Err(KError::NotSupported)
    }
}

/// Set current process capabilities
fn set_current_caps(effective: CapSet, permitted: CapSet, inheritable: CapSet) -> KResult<()> {
    let task = crate::sched::current_task();
    let mut caps = task.caps();

    // Validate changes:
    // 1. New permitted must be subset of old permitted
    if !caps.permitted.contains(permitted) {
        return Err(KError::PermissionDenied);
    }

    // 2. New effective must be subset of new permitted
    if !permitted.contains(effective) {
        return Err(KError::PermissionDenied);
    }

    // 3. New inheritable: can only add caps that are in permitted and bounding
    let added_inheritable = inheritable & !caps.inheritable;
    if !caps.permitted.contains(added_inheritable) || !caps.bounding.contains(added_inheritable) {
        return Err(KError::PermissionDenied);
    }

    // Apply changes
    caps.effective = effective;
    caps.permitted = permitted;
    caps.inheritable = inheritable;

    task.set_caps(caps);
    Ok(())
}

// ============================================================
// Capability checking helpers
// ============================================================

/// Check if current process has a capability
pub fn capable(cap: Cap) -> bool {
    let caps = current_caps();
    caps.has_effective(cap)
}

/// Check if current process has capability for network operations
pub fn capable_net(cap: Cap) -> bool {
    // Could add namespace checks here
    capable(cap)
}

/// Check capability with audit
pub fn capable_wrt_inode_uidgid(_inode_uid: u32, _inode_gid: u32, cap: Cap) -> bool {
    // For now, just check the capability
    // Could add user namespace checks here
    capable(cap)
}

/// Get capability from name
pub fn cap_from_name(name: &str) -> Option<Cap> {
    match name.to_uppercase().as_str() {
        "CAP_CHOWN" => Some(Cap::Chown),
        "CAP_DAC_OVERRIDE" => Some(Cap::DacOverride),
        "CAP_DAC_READ_SEARCH" => Some(Cap::DacReadSearch),
        "CAP_FOWNER" => Some(Cap::Fowner),
        "CAP_FSETID" => Some(Cap::Fsetid),
        "CAP_KILL" => Some(Cap::Kill),
        "CAP_SETGID" => Some(Cap::Setgid),
        "CAP_SETUID" => Some(Cap::Setuid),
        "CAP_SETPCAP" => Some(Cap::Setpcap),
        "CAP_LINUX_IMMUTABLE" => Some(Cap::LinuxImmutable),
        "CAP_NET_BIND_SERVICE" => Some(Cap::NetBindService),
        "CAP_NET_BROADCAST" => Some(Cap::NetBroadcast),
        "CAP_NET_ADMIN" => Some(Cap::NetAdmin),
        "CAP_NET_RAW" => Some(Cap::NetRaw),
        "CAP_IPC_LOCK" => Some(Cap::IpcLock),
        "CAP_IPC_OWNER" => Some(Cap::IpcOwner),
        "CAP_SYS_MODULE" => Some(Cap::SysModule),
        "CAP_SYS_RAWIO" => Some(Cap::SysRawio),
        "CAP_SYS_CHROOT" => Some(Cap::SysChroot),
        "CAP_SYS_PTRACE" => Some(Cap::SysPtrace),
        "CAP_SYS_PACCT" => Some(Cap::SysPacct),
        "CAP_SYS_ADMIN" => Some(Cap::SysAdmin),
        "CAP_SYS_BOOT" => Some(Cap::SysBoot),
        "CAP_SYS_NICE" => Some(Cap::SysNice),
        "CAP_SYS_RESOURCE" => Some(Cap::SysResource),
        "CAP_SYS_TIME" => Some(Cap::SysTime),
        "CAP_SYS_TTY_CONFIG" => Some(Cap::SysTtyConfig),
        "CAP_MKNOD" => Some(Cap::Mknod),
        "CAP_LEASE" => Some(Cap::Lease),
        "CAP_AUDIT_WRITE" => Some(Cap::AuditWrite),
        "CAP_AUDIT_CONTROL" => Some(Cap::AuditControl),
        "CAP_SETFCAP" => Some(Cap::Setfcap),
        "CAP_MAC_OVERRIDE" => Some(Cap::MacOverride),
        "CAP_MAC_ADMIN" => Some(Cap::MacAdmin),
        "CAP_SYSLOG" => Some(Cap::Syslog),
        "CAP_WAKE_ALARM" => Some(Cap::WakeAlarm),
        "CAP_BLOCK_SUSPEND" => Some(Cap::BlockSuspend),
        "CAP_AUDIT_READ" => Some(Cap::AuditRead),
        "CAP_PERFMON" => Some(Cap::Perfmon),
        "CAP_BPF" => Some(Cap::Bpf),
        "CAP_CHECKPOINT_RESTORE" => Some(Cap::CheckpointRestore),
        _ => None,
    }
}

/// Format capability set as string
pub fn format_caps(caps: CapSet) -> alloc::string::String {
    use alloc::string::String;
    use alloc::vec::Vec;

    let mut names: Vec<&str> = Vec::new();

    for i in 0..=CAP_LAST_CAP {
        if let Some(cap) = Cap::from_number(i) {
            if caps.contains(cap.to_set()) {
                names.push(cap.name());
            }
        }
    }

    if names.is_empty() {
        String::from("(none)")
    } else {
        names.join(",")
    }
}
