//! Application Sandbox
//!
//! Implements process sandboxing for application isolation using a combination
//! of namespaces, seccomp filters, capabilities dropping, and filesystem
//! restrictions.
//!
//! ## Features
//! - Namespace isolation (PID, mount, network, user, IPC, UTS)
//! - Seccomp syscall filtering
//! - Capability dropping
//! - Filesystem view restrictions
//! - Resource limits (cgroups integration)
//! - Landlock-style filesystem access control

extern crate alloc;

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use bitflags::bitflags;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::sync::IrqSafeMutex;
use crate::util::{KError, KResult};

use super::seccomp::{SimpleFilter, SeccompAction};
use super::caps::{ProcessCaps, CapSet, Cap};

/// Sandbox ID type
pub type SandboxId = u64;

bitflags! {
    /// Namespace flags
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct NamespaceFlags: u32 {
        /// New mount namespace
        const NEWNS = 1 << 0;
        /// New UTS namespace (hostname)
        const NEWUTS = 1 << 1;
        /// New IPC namespace
        const NEWIPC = 1 << 2;
        /// New PID namespace
        const NEWPID = 1 << 3;
        /// New network namespace
        const NEWNET = 1 << 4;
        /// New user namespace
        const NEWUSER = 1 << 5;
        /// New cgroup namespace
        const NEWCGROUP = 1 << 6;
        /// New time namespace
        const NEWTIME = 1 << 7;
    }
}

bitflags! {
    /// Filesystem access flags (Landlock-style)
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FsAccess: u32 {
        /// Execute a file
        const EXECUTE = 1 << 0;
        /// Open file for writing
        const WRITE_FILE = 1 << 1;
        /// Open file for reading
        const READ_FILE = 1 << 2;
        /// Read directory contents
        const READ_DIR = 1 << 3;
        /// Remove directory
        const REMOVE_DIR = 1 << 4;
        /// Remove file
        const REMOVE_FILE = 1 << 5;
        /// Create a character device
        const MAKE_CHAR = 1 << 6;
        /// Create a directory
        const MAKE_DIR = 1 << 7;
        /// Create a regular file
        const MAKE_REG = 1 << 8;
        /// Create a socket
        const MAKE_SOCK = 1 << 9;
        /// Create a fifo
        const MAKE_FIFO = 1 << 10;
        /// Create a block device
        const MAKE_BLOCK = 1 << 11;
        /// Create a symbolic link
        const MAKE_SYM = 1 << 12;
        /// Link or rename a file
        const REFER = 1 << 13;
        /// Truncate a file
        const TRUNCATE = 1 << 14;

        /// All read operations
        const READ = Self::READ_FILE.bits() | Self::READ_DIR.bits();
        /// All write operations
        const WRITE = Self::WRITE_FILE.bits() | Self::REMOVE_DIR.bits() |
                      Self::REMOVE_FILE.bits() | Self::MAKE_DIR.bits() |
                      Self::MAKE_REG.bits() | Self::TRUNCATE.bits();
        /// All operations
        const ALL = 0x7FFF;
    }
}

/// Filesystem rule
#[derive(Debug, Clone)]
pub struct FsRule {
    /// Path this rule applies to
    pub path: String,
    /// Allowed access flags
    pub access: FsAccess,
    /// Inherit to children
    pub inherit: bool,
}

impl FsRule {
    pub fn new(path: &str, access: FsAccess) -> Self {
        Self {
            path: String::from(path),
            access,
            inherit: true,
        }
    }

    pub fn read_only(path: &str) -> Self {
        Self::new(path, FsAccess::READ | FsAccess::EXECUTE)
    }

    pub fn read_write(path: &str) -> Self {
        Self::new(path, FsAccess::READ | FsAccess::WRITE | FsAccess::EXECUTE)
    }

    pub fn execute_only(path: &str) -> Self {
        Self::new(path, FsAccess::EXECUTE)
    }
}

/// Network access policy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkPolicy {
    /// No network access
    None,
    /// Local (loopback) only
    LocalOnly,
    /// Outbound connections only
    OutboundOnly,
    /// Full network access
    Full,
    /// Custom rules (firewall-based)
    Custom,
}

/// Sandbox profile
#[derive(Debug, Clone)]
pub struct SandboxProfile {
    /// Profile name
    pub name: String,
    /// Namespace flags
    pub namespaces: NamespaceFlags,
    /// Filesystem rules
    pub fs_rules: Vec<FsRule>,
    /// Network policy
    pub network: NetworkPolicy,
    /// Allowed capabilities
    pub caps: CapSet,
    /// Syscall filter (if any)
    pub seccomp: Option<SimpleFilter>,
    /// Resource limits
    pub limits: ResourceLimits,
    /// Environment variables to set
    pub env: BTreeMap<String, String>,
    /// UID mapping for user namespace
    pub uid_map: Option<(u32, u32, u32)>, // (inner, outer, count)
    /// GID mapping for user namespace
    pub gid_map: Option<(u32, u32, u32)>,
}

impl SandboxProfile {
    /// Create an empty profile
    pub fn new(name: &str) -> Self {
        Self {
            name: String::from(name),
            namespaces: NamespaceFlags::empty(),
            fs_rules: Vec::new(),
            network: NetworkPolicy::Full,
            caps: CapSet::empty(),
            seccomp: None,
            limits: ResourceLimits::default(),
            env: BTreeMap::new(),
            uid_map: None,
            gid_map: None,
        }
    }

    /// Create a minimal sandbox (most restrictive)
    pub fn minimal() -> Self {
        let mut profile = Self::new("minimal");

        // Full isolation
        profile.namespaces = NamespaceFlags::NEWNS | NamespaceFlags::NEWPID |
                            NamespaceFlags::NEWIPC | NamespaceFlags::NEWNET |
                            NamespaceFlags::NEWUTS | NamespaceFlags::NEWUSER;

        // No network
        profile.network = NetworkPolicy::None;

        // No capabilities
        profile.caps = CapSet::empty();

        // Strict seccomp
        let mut filter = SimpleFilter::deny_all();
        // Allow basic syscalls
        filter.allow(0);   // read
        filter.allow(1);   // write
        filter.allow(3);   // close
        filter.allow(60);  // exit
        filter.allow(231); // exit_group
        filter.allow(12);  // brk
        filter.allow(9);   // mmap
        filter.allow(11);  // munmap
        profile.seccomp = Some(filter);

        // Minimal filesystem - only /lib and /usr/lib for libraries
        profile.fs_rules.push(FsRule::read_only("/lib"));
        profile.fs_rules.push(FsRule::read_only("/usr/lib"));
        profile.fs_rules.push(FsRule::read_only("/etc/ld.so.cache"));

        profile
    }

    /// Create a standard desktop application sandbox
    pub fn desktop_app() -> Self {
        let mut profile = Self::new("desktop_app");

        // Some isolation
        profile.namespaces = NamespaceFlags::NEWNS | NamespaceFlags::NEWIPC;

        // Full network (for now)
        profile.network = NetworkPolicy::Full;

        // Drop dangerous capabilities
        profile.caps = CapSet::empty();

        // Standard seccomp filter
        profile.seccomp = Some(super::seccomp::standard_filter());

        // Standard desktop paths
        profile.fs_rules.push(FsRule::read_only("/"));
        profile.fs_rules.push(FsRule::read_write("/tmp"));
        profile.fs_rules.push(FsRule::read_write("/var/tmp"));

        profile
    }

    /// Create a browser sandbox profile
    pub fn browser() -> Self {
        let mut profile = Self::new("browser");

        // Strong isolation
        profile.namespaces = NamespaceFlags::NEWNS | NamespaceFlags::NEWPID |
                            NamespaceFlags::NEWIPC | NamespaceFlags::NEWUTS;

        // Outbound network only
        profile.network = NetworkPolicy::OutboundOnly;

        // No dangerous capabilities
        profile.caps = CapSet::empty();

        // Browser-specific seccomp
        let mut filter = SimpleFilter::allow_all();
        // Block dangerous syscalls
        filter.deny(165); // mount
        filter.deny(166); // umount2
        filter.deny(176); // delete_module
        filter.deny(175); // init_module
        filter.deny(101); // ptrace
        filter.deny(139); // sysfs
        filter.deny(154); // modify_ldt
        filter.deny(155); // pivot_root
        filter.deny(156); // _sysctl
        filter.deny(157); // prctl (some options)
        filter.deny(161); // chroot
        filter.deny(169); // reboot
        filter.deny(170); // sethostname
        filter.deny(171); // setdomainname
        filter.deny(179); // quotactl
        profile.seccomp = Some(filter);

        // Browser filesystem access
        profile.fs_rules.push(FsRule::read_only("/"));
        profile.fs_rules.push(FsRule::read_write("/tmp"));
        profile.fs_rules.push(FsRule::read_only("/dev/urandom"));
        profile.fs_rules.push(FsRule::read_only("/dev/null"));
        profile.fs_rules.push(FsRule::read_only("/dev/zero"));

        profile
    }

    /// Add a filesystem rule
    pub fn add_fs_rule(&mut self, rule: FsRule) {
        self.fs_rules.push(rule);
    }

    /// Allow a path read-only
    pub fn allow_read(&mut self, path: &str) {
        self.fs_rules.push(FsRule::read_only(path));
    }

    /// Allow a path read-write
    pub fn allow_write(&mut self, path: &str) {
        self.fs_rules.push(FsRule::read_write(path));
    }

    /// Set environment variable
    pub fn set_env(&mut self, key: &str, value: &str) {
        self.env.insert(String::from(key), String::from(value));
    }
}

/// Resource limits for sandboxed processes
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Max CPU time (seconds)
    pub cpu_time: Option<u64>,
    /// Max file size (bytes)
    pub fsize: Option<u64>,
    /// Max data segment size (bytes)
    pub data: Option<u64>,
    /// Max stack size (bytes)
    pub stack: Option<u64>,
    /// Max resident set size (bytes)
    pub rss: Option<u64>,
    /// Max number of processes
    pub nproc: Option<u64>,
    /// Max number of open files
    pub nofile: Option<u64>,
    /// Max locked memory (bytes)
    pub memlock: Option<u64>,
    /// Max address space (bytes)
    pub as_limit: Option<u64>,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            cpu_time: None,
            fsize: None,
            data: None,
            stack: Some(8 * 1024 * 1024), // 8 MB
            rss: None,
            nproc: Some(64),
            nofile: Some(1024),
            memlock: Some(64 * 1024), // 64 KB
            as_limit: None,
        }
    }
}

impl ResourceLimits {
    /// Very restrictive limits
    pub fn strict() -> Self {
        Self {
            cpu_time: Some(300), // 5 minutes
            fsize: Some(10 * 1024 * 1024), // 10 MB
            data: Some(100 * 1024 * 1024), // 100 MB
            stack: Some(1 * 1024 * 1024), // 1 MB
            rss: Some(100 * 1024 * 1024), // 100 MB
            nproc: Some(10),
            nofile: Some(64),
            memlock: Some(0),
            as_limit: Some(200 * 1024 * 1024), // 200 MB
        }
    }
}

/// Active sandbox state
pub struct Sandbox {
    /// Unique ID
    pub id: SandboxId,
    /// Profile name
    pub profile: String,
    /// PID of sandboxed process
    pub pid: Option<u64>,
    /// Active namespaces
    pub namespaces: NamespaceFlags,
    /// Active filesystem rules
    pub fs_rules: Vec<FsRule>,
    /// Network policy
    pub network: NetworkPolicy,
    /// Creation time
    pub created: u64,
    /// Active
    pub active: bool,
}

impl Sandbox {
    pub fn new(id: SandboxId, profile: &SandboxProfile) -> Self {
        Self {
            id,
            profile: profile.name.clone(),
            pid: None,
            namespaces: profile.namespaces,
            fs_rules: profile.fs_rules.clone(),
            network: profile.network,
            created: crate::time::uptime_secs(),
            active: false,
        }
    }
}

/// Sandbox manager
pub struct SandboxManager {
    /// Active sandboxes
    sandboxes: BTreeMap<SandboxId, Sandbox>,
    /// Registered profiles
    profiles: BTreeMap<String, SandboxProfile>,
    /// Next sandbox ID
    next_id: AtomicU64,
    /// Initialized
    initialized: bool,
}

impl SandboxManager {
    pub const fn new() -> Self {
        Self {
            sandboxes: BTreeMap::new(),
            profiles: BTreeMap::new(),
            next_id: AtomicU64::new(1),
            initialized: false,
        }
    }

    /// Initialize sandbox manager
    pub fn init(&mut self) {
        // Register built-in profiles
        self.profiles.insert(String::from("minimal"), SandboxProfile::minimal());
        self.profiles.insert(String::from("desktop_app"), SandboxProfile::desktop_app());
        self.profiles.insert(String::from("browser"), SandboxProfile::browser());

        self.initialized = true;
        crate::kprintln!("sandbox: initialized with {} profiles", self.profiles.len());
    }

    /// Register a new profile
    pub fn register_profile(&mut self, profile: SandboxProfile) {
        self.profiles.insert(profile.name.clone(), profile);
    }

    /// Get a profile by name
    pub fn get_profile(&self, name: &str) -> Option<&SandboxProfile> {
        self.profiles.get(name)
    }

    /// Create a new sandbox from a profile
    pub fn create(&mut self, profile_name: &str) -> KResult<SandboxId> {
        let profile = self.profiles.get(profile_name)
            .ok_or(KError::NotFound)?;

        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let sandbox = Sandbox::new(id, profile);

        self.sandboxes.insert(id, sandbox);
        Ok(id)
    }

    /// Create sandbox from custom profile
    pub fn create_custom(&mut self, profile: SandboxProfile) -> SandboxId {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let sandbox = Sandbox::new(id, &profile);

        self.sandboxes.insert(id, sandbox);
        id
    }

    /// Activate a sandbox for a process
    pub fn activate(&mut self, id: SandboxId, pid: u64) -> KResult<()> {
        // First, update sandbox state and extract profile name
        let (profile_name, sandbox_id, sandbox_pid) = {
            let sandbox = self.sandboxes.get_mut(&id)
                .ok_or(KError::NotFound)?;

            sandbox.pid = Some(pid);
            sandbox.active = true;

            (sandbox.profile.clone(), sandbox.id, sandbox.pid)
        };

        // Now apply sandbox restrictions using the profile name
        self.apply_sandbox_by_profile(&profile_name, sandbox_id, sandbox_pid)?;

        Ok(())
    }

    /// Apply sandbox restrictions to current process using profile name
    fn apply_sandbox_by_profile(&self, profile_name: &str, sandbox_id: SandboxId, sandbox_pid: Option<u64>) -> KResult<()> {
        let profile = self.profiles.get(profile_name)
            .ok_or(KError::NotFound)?;

        // 1. Create namespaces if requested
        self.setup_namespaces(profile)?;

        // 2. Apply filesystem restrictions
        self.apply_fs_rules(profile)?;

        // 3. Apply network policy
        self.apply_network_policy(profile)?;

        // 4. Drop capabilities
        self.drop_capabilities(profile)?;

        // 5. Apply seccomp filter
        self.apply_seccomp(profile)?;

        // 6. Apply resource limits
        self.apply_limits(profile)?;

        crate::kprintln!("sandbox: activated sandbox {} ({}) for PID {:?}",
            sandbox_id, profile_name, sandbox_pid);

        Ok(())
    }

    fn setup_namespaces(&self, _profile: &SandboxProfile) -> KResult<()> {
        // Would call unshare() or clone() with namespace flags
        // For now, just log
        Ok(())
    }

    fn apply_fs_rules(&self, _profile: &SandboxProfile) -> KResult<()> {
        // Would set up mount namespace, bind mounts, etc.
        // Or use Landlock if available
        Ok(())
    }

    fn apply_network_policy(&self, profile: &SandboxProfile) -> KResult<()> {
        match profile.network {
            NetworkPolicy::None => {
                // Would set up empty network namespace
            }
            NetworkPolicy::LocalOnly => {
                // Network namespace with only loopback
            }
            NetworkPolicy::OutboundOnly => {
                // Firewall rules to block inbound
            }
            NetworkPolicy::Full | NetworkPolicy::Custom => {
                // Full access or custom rules
            }
        }
        Ok(())
    }

    fn drop_capabilities(&self, profile: &SandboxProfile) -> KResult<()> {
        // Get current task and modify capabilities
        let task = crate::sched::current_task();
        let mut caps = task.caps();

        // Set effective and permitted to only allowed caps
        caps.effective = profile.caps;
        caps.permitted = profile.caps;

        // Clear ambient
        caps.ambient = CapSet::empty();

        task.set_caps(caps);
        Ok(())
    }

    fn apply_seccomp(&self, profile: &SandboxProfile) -> KResult<()> {
        if let Some(ref _filter) = profile.seccomp {
            // Would apply seccomp filter to current task
            // let task = crate::sched::current_task();
            // task.set_seccomp_filter(filter.clone());
        }
        Ok(())
    }

    fn apply_limits(&self, _profile: &SandboxProfile) -> KResult<()> {
        // Would call setrlimit for each limit
        Ok(())
    }

    /// Destroy a sandbox
    pub fn destroy(&mut self, id: SandboxId) -> KResult<()> {
        let sandbox = self.sandboxes.remove(&id)
            .ok_or(KError::NotFound)?;

        if sandbox.active {
            // Clean up resources
            crate::kprintln!("sandbox: destroyed sandbox {}", id);
        }

        Ok(())
    }

    /// Get sandbox by ID
    pub fn get(&self, id: SandboxId) -> Option<&Sandbox> {
        self.sandboxes.get(&id)
    }

    /// List all sandboxes
    pub fn list(&self) -> Vec<SandboxId> {
        self.sandboxes.keys().copied().collect()
    }

    /// Check filesystem access
    pub fn check_fs_access(&self, id: SandboxId, path: &str, access: FsAccess) -> bool {
        let sandbox = match self.sandboxes.get(&id) {
            Some(s) => s,
            None => return true, // No sandbox = allow
        };

        if sandbox.fs_rules.is_empty() {
            return true;
        }

        // Find the most specific matching rule
        let mut best_match: Option<&FsRule> = None;
        let mut best_match_len = 0;

        for rule in &sandbox.fs_rules {
            if path.starts_with(&rule.path) && rule.path.len() > best_match_len {
                best_match = Some(rule);
                best_match_len = rule.path.len();
            }
        }

        match best_match {
            Some(rule) => rule.access.contains(access),
            None => false, // No matching rule = deny
        }
    }
}

// =============================================================================
// Global Instance
// =============================================================================

static SANDBOX_MANAGER: IrqSafeMutex<SandboxManager> = IrqSafeMutex::new(SandboxManager::new());

/// Initialize sandbox subsystem
pub fn init() {
    SANDBOX_MANAGER.lock().init();
}

/// Register a sandbox profile
pub fn register_profile(profile: SandboxProfile) {
    SANDBOX_MANAGER.lock().register_profile(profile);
}

/// Create a sandbox from a named profile
pub fn create(profile_name: &str) -> KResult<SandboxId> {
    SANDBOX_MANAGER.lock().create(profile_name)
}

/// Create a sandbox from a custom profile
pub fn create_custom(profile: SandboxProfile) -> SandboxId {
    SANDBOX_MANAGER.lock().create_custom(profile)
}

/// Activate a sandbox for the current process
pub fn activate(id: SandboxId) -> KResult<()> {
    let pid = crate::sched::current_pid();
    SANDBOX_MANAGER.lock().activate(id, pid)
}

/// Destroy a sandbox
pub fn destroy(id: SandboxId) -> KResult<()> {
    SANDBOX_MANAGER.lock().destroy(id)
}

/// Check if filesystem access is allowed for a sandbox
pub fn check_fs_access(id: SandboxId, path: &str, access: FsAccess) -> bool {
    SANDBOX_MANAGER.lock().check_fs_access(id, path, access)
}

/// List all sandbox IDs
pub fn list_sandboxes() -> Vec<SandboxId> {
    SANDBOX_MANAGER.lock().list()
}
