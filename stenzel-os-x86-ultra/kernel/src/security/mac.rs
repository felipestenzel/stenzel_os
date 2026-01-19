//! Mandatory Access Control (MAC) Framework
//!
//! Implements MAC policies similar to SELinux/AppArmor.
//! Provides fine-grained access control based on security labels.

#![allow(dead_code)]

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use alloc::collections::BTreeMap;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::sync::IrqSafeMutex;
use crate::util::kprintln;

/// MAC system state
static MAC_SYSTEM: IrqSafeMutex<Option<MacSystem>> = IrqSafeMutex::new(None);

/// Statistics
static STATS: MacStats = MacStats::new();

/// MAC framework type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacFramework {
    /// SELinux-style type enforcement
    SeLinux,
    /// AppArmor-style profile-based
    AppArmor,
    /// Combined/hybrid
    Hybrid,
}

/// MAC mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacMode {
    /// Disabled
    Disabled,
    /// Permissive (log but don't enforce)
    Permissive,
    /// Enforcing
    Enforcing,
}

/// Security label (SELinux-style)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityLabel {
    /// User
    pub user: String,
    /// Role
    pub role: String,
    /// Type
    pub stype: String,
    /// Level/category (MLS/MCS)
    pub level: Option<String>,
}

impl SecurityLabel {
    pub fn new(user: &str, role: &str, stype: &str) -> Self {
        Self {
            user: user.to_string(),
            role: role.to_string(),
            stype: stype.to_string(),
            level: None,
        }
    }

    pub fn with_level(mut self, level: &str) -> Self {
        self.level = Some(level.to_string());
        self
    }

    pub fn to_string(&self) -> String {
        if let Some(ref level) = self.level {
            alloc::format!("{}:{}:{}:{}", self.user, self.role, self.stype, level)
        } else {
            alloc::format!("{}:{}:{}", self.user, self.role, self.stype)
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() < 3 {
            return None;
        }
        let mut label = Self::new(parts[0], parts[1], parts[2]);
        if parts.len() >= 4 {
            label.level = Some(parts[3].to_string());
        }
        Some(label)
    }

    /// Default unconfined label
    pub fn unconfined() -> Self {
        Self::new("unconfined_u", "unconfined_r", "unconfined_t")
    }

    /// System label
    pub fn system() -> Self {
        Self::new("system_u", "system_r", "kernel_t")
    }
}

/// Object class (what's being accessed)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ObjectClass {
    File,
    Dir,
    Socket,
    Process,
    Ipc,
    Sem,
    Shm,
    Msg,
    Msgq,
    Fd,
    Blk,
    Chr,
    Fifo,
    Lnk,
    Key,
    Capability,
    NetlinkSocket,
    UnixStreamSocket,
    UnixDgramSocket,
    TcpSocket,
    UdpSocket,
    RawIpSocket,
    Node,
    Netif,
    Packet,
    Security,
    System,
    Filesystem,
    Kernel,
    Bpf,
}

impl ObjectClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Dir => "dir",
            Self::Socket => "socket",
            Self::Process => "process",
            Self::Ipc => "ipc",
            Self::Sem => "sem",
            Self::Shm => "shm",
            Self::Msg => "msg",
            Self::Msgq => "msgq",
            Self::Fd => "fd",
            Self::Blk => "blk_file",
            Self::Chr => "chr_file",
            Self::Fifo => "fifo_file",
            Self::Lnk => "lnk_file",
            Self::Key => "key",
            Self::Capability => "capability",
            Self::NetlinkSocket => "netlink_socket",
            Self::UnixStreamSocket => "unix_stream_socket",
            Self::UnixDgramSocket => "unix_dgram_socket",
            Self::TcpSocket => "tcp_socket",
            Self::UdpSocket => "udp_socket",
            Self::RawIpSocket => "rawip_socket",
            Self::Node => "node",
            Self::Netif => "netif",
            Self::Packet => "packet",
            Self::Security => "security",
            Self::System => "system",
            Self::Filesystem => "filesystem",
            Self::Kernel => "kernel",
            Self::Bpf => "bpf",
        }
    }
}

/// Access vector (permissions)
bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Permission: u64 {
        // Common permissions
        const READ = 1 << 0;
        const WRITE = 1 << 1;
        const EXECUTE = 1 << 2;
        const CREATE = 1 << 3;
        const UNLINK = 1 << 4;
        const LINK = 1 << 5;
        const RENAME = 1 << 6;
        const APPEND = 1 << 7;
        const LOCK = 1 << 8;
        const IOCTL = 1 << 9;
        const GETATTR = 1 << 10;
        const SETATTR = 1 << 11;

        // File-specific
        const OPEN = 1 << 12;
        const MAP = 1 << 13;
        const MOUNTON = 1 << 14;
        const QUOTAON = 1 << 15;
        const ENTRYPOINT = 1 << 16;

        // Directory-specific
        const ADD_NAME = 1 << 17;
        const REMOVE_NAME = 1 << 18;
        const REPARENT = 1 << 19;
        const SEARCH = 1 << 20;
        const RMDIR = 1 << 21;

        // Process-specific
        const FORK = 1 << 22;
        const SIGNAL = 1 << 23;
        const PTRACE = 1 << 24;
        const GETSCHED = 1 << 25;
        const SETSCHED = 1 << 26;
        const GETSESSION = 1 << 27;
        const GETCAP = 1 << 28;
        const SETCAP = 1 << 29;
        const TRANSITION = 1 << 30;
        const DYNTRANSITION = 1 << 31;

        // Socket-specific
        const CONNECT = 1 << 32;
        const LISTEN = 1 << 33;
        const ACCEPT = 1 << 34;
        const BIND = 1 << 35;
        const SEND = 1 << 36;
        const RECV = 1 << 37;
        const SENDTO = 1 << 38;
        const RECVFROM = 1 << 39;
        const SHUTDOWN = 1 << 40;

        // System-specific
        const HALT = 1 << 41;
        const REBOOT = 1 << 42;
        const SYSLOG_READ = 1 << 43;
        const SYSLOG_MOD = 1 << 44;
        const MODULE_LOAD = 1 << 45;
        const MODULE_REQUEST = 1 << 46;

        // Capability-specific
        const CHOWN = 1 << 47;
        const DAC_OVERRIDE = 1 << 48;
        const DAC_READ_SEARCH = 1 << 49;
        const FOWNER = 1 << 50;
        const FSETID = 1 << 51;
        const KILL = 1 << 52;
        const SETGID = 1 << 53;
        const SETUID = 1 << 54;
        const NET_BIND_SERVICE = 1 << 55;
        const NET_RAW = 1 << 56;
        const SYS_ADMIN = 1 << 57;
        const SYS_BOOT = 1 << 58;
        const SYS_MODULE = 1 << 59;
        const SYS_PTRACE = 1 << 60;
        const SYS_RAWIO = 1 << 61;
    }
}

impl Permission {
    pub fn file_read() -> Self {
        Self::READ | Self::OPEN | Self::GETATTR
    }

    pub fn file_write() -> Self {
        Self::WRITE | Self::OPEN | Self::GETATTR | Self::SETATTR
    }

    pub fn file_execute() -> Self {
        Self::EXECUTE | Self::READ | Self::OPEN | Self::GETATTR | Self::MAP
    }

    pub fn dir_search() -> Self {
        Self::SEARCH | Self::GETATTR
    }

    pub fn dir_read() -> Self {
        Self::READ | Self::SEARCH | Self::GETATTR
    }

    pub fn dir_write() -> Self {
        Self::WRITE | Self::ADD_NAME | Self::REMOVE_NAME | Self::SEARCH | Self::GETATTR | Self::SETATTR
    }
}

/// Type Enforcement rule
#[derive(Debug, Clone)]
pub struct TeRule {
    /// Source type
    pub source: String,
    /// Target type
    pub target: String,
    /// Object class
    pub class: ObjectClass,
    /// Allowed permissions
    pub permissions: Permission,
    /// Rule type
    pub rule_type: TeRuleType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeRuleType {
    Allow,
    Auditallow,
    Dontaudit,
    Neverallow,
}

/// Type transition rule
#[derive(Debug, Clone)]
pub struct TypeTransition {
    /// Source type
    pub source: String,
    /// Target type
    pub target: String,
    /// Object class
    pub class: ObjectClass,
    /// New type
    pub new_type: String,
    /// Optional object name
    pub object_name: Option<String>,
}

/// Role transition rule
#[derive(Debug, Clone)]
pub struct RoleTransition {
    /// Source role
    pub source: String,
    /// Target type
    pub target: String,
    /// New role
    pub new_role: String,
}

/// AppArmor-style profile
#[derive(Debug, Clone)]
pub struct AppArmorProfile {
    /// Profile name
    pub name: String,
    /// Attached path (for file profiles)
    pub attach: Option<String>,
    /// Flags
    pub flags: ProfileFlags,
    /// File rules
    pub file_rules: Vec<FileRule>,
    /// Network rules
    pub network_rules: Vec<NetworkRule>,
    /// Capability rules
    pub capability_rules: Vec<CapabilityRule>,
    /// Mount rules
    pub mount_rules: Vec<MountRule>,
    /// Dbus rules
    pub dbus_rules: Vec<DbusRule>,
    /// Signal rules
    pub signal_rules: Vec<SignalRule>,
    /// Ptrace rules
    pub ptrace_rules: Vec<PtraceRule>,
    /// Child profiles
    pub children: Vec<AppArmorProfile>,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ProfileFlags: u32 {
        const ENFORCE = 1 << 0;
        const COMPLAIN = 1 << 1;
        const AUDIT = 1 << 2;
        const MEDIATE_DELETED = 1 << 3;
        const ATTACH_DISCONNECTED = 1 << 4;
        const NO_NEW_PRIVS = 1 << 5;
        const CHROOT_RELATIVE = 1 << 6;
    }
}

/// File access rule
#[derive(Debug, Clone)]
pub struct FileRule {
    pub path: String,
    pub permissions: FilePermission,
    pub exec_mode: Option<ExecMode>,
    pub owner: bool,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FilePermission: u32 {
        const READ = 1 << 0;
        const WRITE = 1 << 1;
        const APPEND = 1 << 2;
        const EXEC = 1 << 3;
        const MMAP_EXEC = 1 << 4;
        const LINK = 1 << 5;
        const LOCK = 1 << 6;
        const CREATE = 1 << 7;
        const DELETE = 1 << 8;
        const RENAME_SRC = 1 << 9;
        const RENAME_DST = 1 << 10;
        const SETATTR = 1 << 11;
        const GETATTR = 1 << 12;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecMode {
    /// Inherit profile
    Ix,
    /// Profile transition
    Px,
    /// Profile transition with cleanup
    Cx,
    /// Unconfined
    Ux,
}

/// Network rule
#[derive(Debug, Clone)]
pub struct NetworkRule {
    pub domain: NetworkDomain,
    pub sock_type: Option<NetworkType>,
    pub permission: NetworkPermission,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkDomain {
    Inet,
    Inet6,
    Unix,
    Netlink,
    Packet,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkType {
    Stream,
    Dgram,
    Raw,
    Seqpacket,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct NetworkPermission: u32 {
        const CREATE = 1 << 0;
        const BIND = 1 << 1;
        const LISTEN = 1 << 2;
        const ACCEPT = 1 << 3;
        const CONNECT = 1 << 4;
        const SEND = 1 << 5;
        const RECEIVE = 1 << 6;
        const GETATTR = 1 << 7;
        const SETATTR = 1 << 8;
        const GETOPT = 1 << 9;
        const SETOPT = 1 << 10;
        const SHUTDOWN = 1 << 11;
    }
}

/// Capability rule
#[derive(Debug, Clone)]
pub struct CapabilityRule {
    pub capability: String,
    pub allow: bool,
}

/// Mount rule
#[derive(Debug, Clone)]
pub struct MountRule {
    pub source: Option<String>,
    pub target: String,
    pub fstype: Option<String>,
    pub flags: MountFlags,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct MountFlags: u32 {
        const MOUNT = 1 << 0;
        const UMOUNT = 1 << 1;
        const REMOUNT = 1 << 2;
        const BIND = 1 << 3;
        const MOVE = 1 << 4;
        const RBIND = 1 << 5;
        const MAKE_PRIVATE = 1 << 6;
        const MAKE_SLAVE = 1 << 7;
        const MAKE_SHARED = 1 << 8;
    }
}

/// D-Bus rule
#[derive(Debug, Clone)]
pub struct DbusRule {
    pub bus: String,
    pub name: Option<String>,
    pub path: Option<String>,
    pub interface: Option<String>,
    pub member: Option<String>,
    pub permission: DbusPermission,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct DbusPermission: u32 {
        const SEND = 1 << 0;
        const RECEIVE = 1 << 1;
        const BIND = 1 << 2;
        const EAVESDROP = 1 << 3;
    }
}

/// Signal rule
#[derive(Debug, Clone)]
pub struct SignalRule {
    pub set: Vec<i32>,
    pub peer: Option<String>,
    pub send: bool,
    pub receive: bool,
}

/// Ptrace rule
#[derive(Debug, Clone)]
pub struct PtraceRule {
    pub peer: Option<String>,
    pub permission: PtracePermission,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PtracePermission: u32 {
        const TRACE = 1 << 0;
        const READ = 1 << 1;
        const TRACEBY = 1 << 2;
        const READBY = 1 << 3;
    }
}

/// Policy decision
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Deny,
    Audit,
}

/// Access check result
#[derive(Debug, Clone)]
pub struct AccessResult {
    pub decision: Decision,
    pub audit: bool,
    pub reason: String,
}

/// MAC error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacError {
    NotInitialized,
    Denied,
    InvalidLabel,
    InvalidPolicy,
    PolicyNotFound,
    InternalError,
}

pub type MacResult<T> = Result<T, MacError>;

/// Statistics
pub struct MacStats {
    checks: AtomicU64,
    allowed: AtomicU64,
    denied: AtomicU64,
    audited: AtomicU64,
    cache_hits: AtomicU64,
    cache_misses: AtomicU64,
}

impl MacStats {
    const fn new() -> Self {
        Self {
            checks: AtomicU64::new(0),
            allowed: AtomicU64::new(0),
            denied: AtomicU64::new(0),
            audited: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
        }
    }
}

/// Access decision cache entry
#[derive(Debug, Clone)]
struct CacheEntry {
    source: String,
    target: String,
    class: ObjectClass,
    permissions: Permission,
    decision: Decision,
    timestamp: u64,
}

/// MAC System
pub struct MacSystem {
    /// Framework type
    framework: MacFramework,
    /// Mode
    mode: MacMode,
    /// Type enforcement rules
    te_rules: Vec<TeRule>,
    /// Type transitions
    type_transitions: Vec<TypeTransition>,
    /// Role transitions
    role_transitions: Vec<RoleTransition>,
    /// AppArmor profiles
    aa_profiles: BTreeMap<String, AppArmorProfile>,
    /// Process labels
    process_labels: BTreeMap<u32, SecurityLabel>,
    /// File labels
    file_labels: BTreeMap<String, SecurityLabel>,
    /// Access cache
    access_cache: Vec<CacheEntry>,
    /// Cache size limit
    cache_size: usize,
    /// Cache TTL (ms)
    cache_ttl: u64,
    /// Audit all denials
    audit_denials: bool,
    /// Audit all grants
    audit_grants: bool,
}

impl MacSystem {
    fn new() -> Self {
        Self {
            framework: MacFramework::Hybrid,
            mode: MacMode::Permissive,
            te_rules: Vec::new(),
            type_transitions: Vec::new(),
            role_transitions: Vec::new(),
            aa_profiles: BTreeMap::new(),
            process_labels: BTreeMap::new(),
            file_labels: BTreeMap::new(),
            access_cache: Vec::new(),
            cache_size: 1024,
            cache_ttl: 60000, // 60 seconds
            audit_denials: true,
            audit_grants: false,
        }
    }

    /// Set mode
    pub fn set_mode(&mut self, mode: MacMode) {
        self.mode = mode;
        kprintln!("mac: Mode set to {:?}", mode);
    }

    /// Get mode
    pub fn get_mode(&self) -> MacMode {
        self.mode
    }

    /// Add TE rule
    pub fn add_te_rule(&mut self, rule: TeRule) {
        kprintln!(
            "mac: Added TE rule: {:?} {} {} {} -> {:?}",
            rule.rule_type,
            rule.source,
            rule.target,
            rule.class.as_str(),
            rule.permissions
        );
        self.te_rules.push(rule);
        self.invalidate_cache();
    }

    /// Add type transition
    pub fn add_type_transition(&mut self, trans: TypeTransition) {
        kprintln!(
            "mac: Added type transition: {} {} {} -> {}",
            trans.source,
            trans.target,
            trans.class.as_str(),
            trans.new_type
        );
        self.type_transitions.push(trans);
    }

    /// Add role transition
    pub fn add_role_transition(&mut self, trans: RoleTransition) {
        self.role_transitions.push(trans);
    }

    /// Add AppArmor profile
    pub fn add_aa_profile(&mut self, profile: AppArmorProfile) {
        kprintln!("mac: Added AppArmor profile: {}", profile.name);
        self.aa_profiles.insert(profile.name.clone(), profile);
    }

    /// Set process label
    pub fn set_process_label(&mut self, pid: u32, label: SecurityLabel) {
        kprintln!("mac: Set process {} label to {}", pid, label.to_string());
        self.process_labels.insert(pid, label);
    }

    /// Get process label
    pub fn get_process_label(&self, pid: u32) -> SecurityLabel {
        self.process_labels
            .get(&pid)
            .cloned()
            .unwrap_or_else(SecurityLabel::unconfined)
    }

    /// Set file label
    pub fn set_file_label(&mut self, path: &str, label: SecurityLabel) {
        self.file_labels.insert(path.to_string(), label);
    }

    /// Get file label
    pub fn get_file_label(&self, path: &str) -> SecurityLabel {
        // Check exact match
        if let Some(label) = self.file_labels.get(path) {
            return label.clone();
        }
        // Check prefix match
        for (pattern, label) in &self.file_labels {
            if pattern.ends_with("/*") {
                let prefix = &pattern[..pattern.len() - 2];
                if path.starts_with(prefix) {
                    return label.clone();
                }
            }
        }
        SecurityLabel::unconfined()
    }

    /// Check access
    pub fn check_access(
        &mut self,
        source_pid: u32,
        target_path: &str,
        class: ObjectClass,
        requested: Permission,
    ) -> AccessResult {
        STATS.checks.fetch_add(1, Ordering::Relaxed);

        if self.mode == MacMode::Disabled {
            return AccessResult {
                decision: Decision::Allow,
                audit: false,
                reason: "MAC disabled".to_string(),
            };
        }

        let source_label = self.get_process_label(source_pid);
        let target_label = self.get_file_label(target_path);

        // Check cache
        if let Some(cached) = self.check_cache(&source_label.stype, &target_label.stype, class, requested) {
            STATS.cache_hits.fetch_add(1, Ordering::Relaxed);
            return cached;
        }
        STATS.cache_misses.fetch_add(1, Ordering::Relaxed);

        // Check TE rules
        let result = self.evaluate_te_rules(&source_label, &target_label, class, requested);

        // Update cache
        self.update_cache(&source_label.stype, &target_label.stype, class, requested, result.decision);

        // Handle permissive mode
        if self.mode == MacMode::Permissive && result.decision == Decision::Deny {
            STATS.denied.fetch_add(1, Ordering::Relaxed);
            if self.audit_denials {
                kprintln!(
                    "mac: PERMISSIVE DENIAL: {} ({}) -> {} ({}) {} {:?}",
                    source_pid,
                    source_label.to_string(),
                    target_path,
                    target_label.to_string(),
                    class.as_str(),
                    requested
                );
            }
            return AccessResult {
                decision: Decision::Allow,
                audit: true,
                reason: result.reason,
            };
        }

        if result.decision == Decision::Allow {
            STATS.allowed.fetch_add(1, Ordering::Relaxed);
        } else {
            STATS.denied.fetch_add(1, Ordering::Relaxed);
            if self.audit_denials {
                kprintln!(
                    "mac: DENIED: {} ({}) -> {} ({}) {} {:?}",
                    source_pid,
                    source_label.to_string(),
                    target_path,
                    target_label.to_string(),
                    class.as_str(),
                    requested
                );
            }
        }

        result
    }

    /// Evaluate TE rules
    fn evaluate_te_rules(
        &self,
        source: &SecurityLabel,
        target: &SecurityLabel,
        class: ObjectClass,
        requested: Permission,
    ) -> AccessResult {
        let mut allowed = Permission::empty();
        let mut dontaudit = Permission::empty();

        for rule in &self.te_rules {
            if !self.type_matches(&rule.source, &source.stype) {
                continue;
            }
            if !self.type_matches(&rule.target, &target.stype) {
                continue;
            }
            if rule.class != class {
                continue;
            }

            match rule.rule_type {
                TeRuleType::Allow => {
                    allowed |= rule.permissions;
                }
                TeRuleType::Dontaudit => {
                    dontaudit |= rule.permissions;
                }
                TeRuleType::Neverallow => {
                    let intersection = rule.permissions & requested;
                    if !intersection.is_empty() {
                        return AccessResult {
                            decision: Decision::Deny,
                            audit: true,
                            reason: "neverallow rule".to_string(),
                        };
                    }
                }
                TeRuleType::Auditallow => {
                    // Mark for audit but don't change decision
                }
            }
        }

        if requested.difference(allowed).is_empty() {
            AccessResult {
                decision: Decision::Allow,
                audit: self.audit_grants,
                reason: "allowed by policy".to_string(),
            }
        } else {
            let denied = requested.difference(allowed);
            AccessResult {
                decision: Decision::Deny,
                audit: !dontaudit.contains(denied),
                reason: alloc::format!("denied permissions: {:?}", denied),
            }
        }
    }

    /// Check if type matches (supports wildcards)
    fn type_matches(&self, pattern: &str, stype: &str) -> bool {
        if pattern == "*" || pattern == stype {
            return true;
        }
        if pattern.ends_with('*') {
            let prefix = &pattern[..pattern.len() - 1];
            return stype.starts_with(prefix);
        }
        false
    }

    /// Check AppArmor profile access
    pub fn check_aa_access(
        &self,
        profile_name: &str,
        path: &str,
        requested: FilePermission,
    ) -> AccessResult {
        let profile = match self.aa_profiles.get(profile_name) {
            Some(p) => p,
            None => {
                return AccessResult {
                    decision: Decision::Allow,
                    audit: false,
                    reason: "no profile".to_string(),
                };
            }
        };

        if profile.flags.contains(ProfileFlags::COMPLAIN) {
            // Complain mode - log but allow
            return AccessResult {
                decision: Decision::Allow,
                audit: true,
                reason: "complain mode".to_string(),
            };
        }

        // Check file rules
        for rule in &profile.file_rules {
            if self.path_matches(&rule.path, path) {
                if rule.permissions.contains(requested) {
                    return AccessResult {
                        decision: Decision::Allow,
                        audit: profile.flags.contains(ProfileFlags::AUDIT),
                        reason: "allowed by profile".to_string(),
                    };
                }
            }
        }

        AccessResult {
            decision: Decision::Deny,
            audit: true,
            reason: "denied by profile".to_string(),
        }
    }

    /// Path matching with glob support
    fn path_matches(&self, pattern: &str, path: &str) -> bool {
        if pattern == path {
            return true;
        }
        if pattern.ends_with("**") {
            let prefix = &pattern[..pattern.len() - 2];
            return path.starts_with(prefix);
        }
        if pattern.ends_with('*') {
            let prefix = &pattern[..pattern.len() - 1];
            if path.starts_with(prefix) {
                // Only match one level
                let rest = &path[prefix.len()..];
                return !rest.contains('/');
            }
        }
        false
    }

    /// Check cache
    fn check_cache(
        &self,
        source: &str,
        target: &str,
        class: ObjectClass,
        permissions: Permission,
    ) -> Option<AccessResult> {
        let now = crate::time::uptime_ms();

        for entry in &self.access_cache {
            if entry.source == source
                && entry.target == target
                && entry.class == class
                && entry.permissions.contains(permissions)
                && now - entry.timestamp < self.cache_ttl
            {
                return Some(AccessResult {
                    decision: entry.decision,
                    audit: false,
                    reason: "cached".to_string(),
                });
            }
        }
        None
    }

    /// Update cache
    fn update_cache(
        &mut self,
        source: &str,
        target: &str,
        class: ObjectClass,
        permissions: Permission,
        decision: Decision,
    ) {
        // Remove old entry if exists
        self.access_cache.retain(|e| {
            !(e.source == source && e.target == target && e.class == class)
        });

        // Add new entry
        self.access_cache.push(CacheEntry {
            source: source.to_string(),
            target: target.to_string(),
            class,
            permissions,
            decision,
            timestamp: crate::time::uptime_ms(),
        });

        // Trim if too large
        while self.access_cache.len() > self.cache_size {
            self.access_cache.remove(0);
        }
    }

    /// Invalidate cache
    fn invalidate_cache(&mut self) {
        self.access_cache.clear();
    }

    /// Compute type transition
    pub fn compute_transition(
        &self,
        source_pid: u32,
        target_path: &str,
        class: ObjectClass,
    ) -> Option<SecurityLabel> {
        let source_label = self.get_process_label(source_pid);
        let target_label = self.get_file_label(target_path);

        for trans in &self.type_transitions {
            if self.type_matches(&trans.source, &source_label.stype)
                && self.type_matches(&trans.target, &target_label.stype)
                && trans.class == class
            {
                return Some(SecurityLabel::new(
                    &source_label.user,
                    &source_label.role,
                    &trans.new_type,
                ));
            }
        }
        None
    }

    /// Get statistics
    pub fn get_stats(&self) -> (u64, u64, u64, u64, u64, u64) {
        (
            STATS.checks.load(Ordering::Relaxed),
            STATS.allowed.load(Ordering::Relaxed),
            STATS.denied.load(Ordering::Relaxed),
            STATS.audited.load(Ordering::Relaxed),
            STATS.cache_hits.load(Ordering::Relaxed),
            STATS.cache_misses.load(Ordering::Relaxed),
        )
    }
}

// Public API

/// Initialize MAC system
pub fn init() {
    let mut guard = MAC_SYSTEM.lock();
    if guard.is_none() {
        *guard = Some(MacSystem::new());
        kprintln!("mac: Initialized");
    }
}

/// Set mode
pub fn set_mode(mode: MacMode) {
    let mut guard = MAC_SYSTEM.lock();
    if let Some(system) = guard.as_mut() {
        system.set_mode(mode);
    }
}

/// Get mode
pub fn get_mode() -> MacMode {
    let guard = MAC_SYSTEM.lock();
    guard.as_ref().map(|s| s.get_mode()).unwrap_or(MacMode::Disabled)
}

/// Add TE rule
pub fn add_te_rule(rule: TeRule) {
    let mut guard = MAC_SYSTEM.lock();
    if let Some(system) = guard.as_mut() {
        system.add_te_rule(rule);
    }
}

/// Add type transition
pub fn add_type_transition(trans: TypeTransition) {
    let mut guard = MAC_SYSTEM.lock();
    if let Some(system) = guard.as_mut() {
        system.add_type_transition(trans);
    }
}

/// Add AppArmor profile
pub fn add_aa_profile(profile: AppArmorProfile) {
    let mut guard = MAC_SYSTEM.lock();
    if let Some(system) = guard.as_mut() {
        system.add_aa_profile(profile);
    }
}

/// Set process label
pub fn set_process_label(pid: u32, label: SecurityLabel) {
    let mut guard = MAC_SYSTEM.lock();
    if let Some(system) = guard.as_mut() {
        system.set_process_label(pid, label);
    }
}

/// Get process label
pub fn get_process_label(pid: u32) -> SecurityLabel {
    let guard = MAC_SYSTEM.lock();
    guard.as_ref()
        .map(|s| s.get_process_label(pid))
        .unwrap_or_else(SecurityLabel::unconfined)
}

/// Set file label
pub fn set_file_label(path: &str, label: SecurityLabel) {
    let mut guard = MAC_SYSTEM.lock();
    if let Some(system) = guard.as_mut() {
        system.set_file_label(path, label);
    }
}

/// Check access
pub fn check_access(
    source_pid: u32,
    target_path: &str,
    class: ObjectClass,
    requested: Permission,
) -> AccessResult {
    let mut guard = MAC_SYSTEM.lock();
    match guard.as_mut() {
        Some(system) => system.check_access(source_pid, target_path, class, requested),
        None => AccessResult {
            decision: Decision::Allow,
            audit: false,
            reason: "MAC not initialized".to_string(),
        },
    }
}

/// Check AppArmor access
pub fn check_aa_access(profile_name: &str, path: &str, requested: FilePermission) -> AccessResult {
    let guard = MAC_SYSTEM.lock();
    match guard.as_ref() {
        Some(system) => system.check_aa_access(profile_name, path, requested),
        None => AccessResult {
            decision: Decision::Allow,
            audit: false,
            reason: "MAC not initialized".to_string(),
        },
    }
}

/// Compute transition
pub fn compute_transition(source_pid: u32, target_path: &str, class: ObjectClass) -> Option<SecurityLabel> {
    let guard = MAC_SYSTEM.lock();
    guard.as_ref().and_then(|s| s.compute_transition(source_pid, target_path, class))
}

/// Get statistics
pub fn get_stats() -> (u64, u64, u64, u64, u64, u64) {
    let guard = MAC_SYSTEM.lock();
    guard.as_ref()
        .map(|s| s.get_stats())
        .unwrap_or((0, 0, 0, 0, 0, 0))
}
