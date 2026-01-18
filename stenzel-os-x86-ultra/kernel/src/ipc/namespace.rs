//! Linux-compatible Namespaces
//!
//! Namespaces provide isolation for system resources between processes:
//! - PID namespace: Isolate process IDs
//! - Mount namespace: Isolate filesystem mounts
//! - Network namespace: Isolate network stack
//! - UTS namespace: Isolate hostname/domainname
//! - IPC namespace: Isolate System V IPC/POSIX message queues
//! - User namespace: Isolate UIDs/GIDs
//! - Cgroup namespace: Isolate cgroup root
//!
//! Each namespace type has its own ID and can be created via clone() or unshare().

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::sync::IrqSafeMutex;
use crate::util::{KError, KResult};

/// Namespace types (flags for clone/unshare)
pub mod flags {
    pub const CLONE_NEWNS: u64 = 0x00020000;     // Mount namespace
    pub const CLONE_NEWUTS: u64 = 0x04000000;    // UTS namespace
    pub const CLONE_NEWIPC: u64 = 0x08000000;    // IPC namespace
    pub const CLONE_NEWUSER: u64 = 0x10000000;   // User namespace
    pub const CLONE_NEWPID: u64 = 0x20000000;    // PID namespace
    pub const CLONE_NEWNET: u64 = 0x40000000;    // Network namespace
    pub const CLONE_NEWCGROUP: u64 = 0x02000000; // Cgroup namespace

    /// All namespace flags
    pub const CLONE_NEWNS_ALL: u64 = CLONE_NEWNS | CLONE_NEWUTS | CLONE_NEWIPC |
                                     CLONE_NEWUSER | CLONE_NEWPID | CLONE_NEWNET |
                                     CLONE_NEWCGROUP;
}

/// Namespace types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NamespaceType {
    Mount,      // Filesystem mounts
    Uts,        // Hostname and domain name
    Ipc,        // System V IPC, POSIX message queues
    User,       // User and group IDs
    Pid,        // Process IDs
    Net,        // Network devices, stacks, ports
    Cgroup,     // Cgroup root directory
}

impl NamespaceType {
    /// Convert from clone flag to namespace type
    pub fn from_flag(flag: u64) -> Option<Self> {
        match flag {
            flags::CLONE_NEWNS => Some(NamespaceType::Mount),
            flags::CLONE_NEWUTS => Some(NamespaceType::Uts),
            flags::CLONE_NEWIPC => Some(NamespaceType::Ipc),
            flags::CLONE_NEWUSER => Some(NamespaceType::User),
            flags::CLONE_NEWPID => Some(NamespaceType::Pid),
            flags::CLONE_NEWNET => Some(NamespaceType::Net),
            flags::CLONE_NEWCGROUP => Some(NamespaceType::Cgroup),
            _ => None,
        }
    }

    /// Get the clone flag for this namespace type
    pub fn to_flag(&self) -> u64 {
        match self {
            NamespaceType::Mount => flags::CLONE_NEWNS,
            NamespaceType::Uts => flags::CLONE_NEWUTS,
            NamespaceType::Ipc => flags::CLONE_NEWIPC,
            NamespaceType::User => flags::CLONE_NEWUSER,
            NamespaceType::Pid => flags::CLONE_NEWPID,
            NamespaceType::Net => flags::CLONE_NEWNET,
            NamespaceType::Cgroup => flags::CLONE_NEWCGROUP,
        }
    }

    /// Get the /proc/pid/ns link name
    pub fn proc_name(&self) -> &'static str {
        match self {
            NamespaceType::Mount => "mnt",
            NamespaceType::Uts => "uts",
            NamespaceType::Ipc => "ipc",
            NamespaceType::User => "user",
            NamespaceType::Pid => "pid",
            NamespaceType::Net => "net",
            NamespaceType::Cgroup => "cgroup",
        }
    }
}

/// Namespace ID counter
static NAMESPACE_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Generate a new unique namespace ID
fn next_namespace_id() -> u64 {
    NAMESPACE_ID_COUNTER.fetch_add(1, Ordering::SeqCst)
}

/// PID namespace data
#[derive(Debug)]
pub struct PidNamespace {
    pub id: u64,
    /// Parent PID namespace (None for root)
    pub parent: Option<Arc<PidNamespace>>,
    /// Depth (root = 0)
    pub level: u32,
    /// Next PID to allocate in this namespace
    next_pid: AtomicU64,
    /// PID 1 process in this namespace (init)
    pub init_pid: u64,
}

impl PidNamespace {
    pub fn new(parent: Option<Arc<PidNamespace>>) -> Arc<Self> {
        let level = parent.as_ref().map(|p| p.level + 1).unwrap_or(0);
        Arc::new(Self {
            id: next_namespace_id(),
            parent,
            level,
            next_pid: AtomicU64::new(1),
            init_pid: 0,
        })
    }

    /// Allocate a new PID in this namespace
    pub fn alloc_pid(&self) -> u64 {
        self.next_pid.fetch_add(1, Ordering::SeqCst)
    }

    /// Check if a global PID is visible in this namespace
    pub fn is_pid_visible(&self, _global_pid: u64) -> bool {
        // In a full implementation, we'd track which PIDs belong to which namespace
        true
    }

    /// Translate global PID to namespace-local PID
    pub fn translate_pid(&self, _global_pid: u64) -> Option<u64> {
        // In a full implementation, this would look up the mapping
        Some(_global_pid)
    }
}

/// UTS namespace data (hostname/domainname)
#[derive(Debug, Clone)]
pub struct UtsNamespace {
    pub id: u64,
    pub hostname: String,
    pub domainname: String,
    pub sysname: String,
    pub release: String,
    pub version: String,
    pub machine: String,
}

impl UtsNamespace {
    pub fn new() -> Arc<IrqSafeMutex<Self>> {
        Arc::new(IrqSafeMutex::new(Self {
            id: next_namespace_id(),
            hostname: String::from("stenzel"),
            domainname: String::new(),
            sysname: String::from("Stenzel OS"),
            release: String::from("0.1.0"),
            version: String::from("#1 SMP"),
            machine: String::from("x86_64"),
        }))
    }

    pub fn clone_ns(&self) -> Self {
        Self {
            id: next_namespace_id(),
            hostname: self.hostname.clone(),
            domainname: self.domainname.clone(),
            sysname: self.sysname.clone(),
            release: self.release.clone(),
            version: self.version.clone(),
            machine: self.machine.clone(),
        }
    }
}

/// IPC namespace data
#[derive(Debug)]
pub struct IpcNamespace {
    pub id: u64,
    /// Shared memory segments
    pub shm_segments: BTreeMap<u64, IpcShmSegment>,
    /// Semaphore sets
    pub sem_sets: BTreeMap<u64, IpcSemSet>,
    /// Message queues
    pub msg_queues: BTreeMap<u64, IpcMsgQueue>,
    /// Next key to allocate
    next_key: AtomicU64,
}

#[derive(Debug, Clone)]
pub struct IpcShmSegment {
    pub key: u64,
    pub size: usize,
    pub perm: u32,
    pub owner_uid: u32,
}

#[derive(Debug, Clone)]
pub struct IpcSemSet {
    pub key: u64,
    pub num_sems: u32,
    pub perm: u32,
}

#[derive(Debug, Clone)]
pub struct IpcMsgQueue {
    pub key: u64,
    pub max_size: usize,
    pub perm: u32,
}

impl IpcNamespace {
    pub fn new() -> Arc<IrqSafeMutex<Self>> {
        Arc::new(IrqSafeMutex::new(Self {
            id: next_namespace_id(),
            shm_segments: BTreeMap::new(),
            sem_sets: BTreeMap::new(),
            msg_queues: BTreeMap::new(),
            next_key: AtomicU64::new(1),
        }))
    }

    pub fn alloc_key(&self) -> u64 {
        self.next_key.fetch_add(1, Ordering::SeqCst)
    }
}

/// User namespace data
#[derive(Debug)]
pub struct UserNamespace {
    pub id: u64,
    /// Parent user namespace
    pub parent: Option<Arc<UserNamespace>>,
    /// UID mappings: (inside_start, outside_start, count)
    pub uid_map: Vec<(u32, u32, u32)>,
    /// GID mappings
    pub gid_map: Vec<(u32, u32, u32)>,
    /// Owner UID in parent namespace
    pub owner_uid: u32,
    /// Owner GID in parent namespace
    pub owner_gid: u32,
    /// Depth (root = 0)
    pub level: u32,
}

impl UserNamespace {
    pub fn new(parent: Option<Arc<UserNamespace>>, owner_uid: u32, owner_gid: u32) -> Arc<Self> {
        let level = parent.as_ref().map(|p| p.level + 1).unwrap_or(0);
        Arc::new(Self {
            id: next_namespace_id(),
            parent,
            uid_map: Vec::new(),
            gid_map: Vec::new(),
            owner_uid,
            owner_gid,
            level,
        })
    }

    /// Add a UID mapping
    pub fn add_uid_mapping(&mut self, inside: u32, outside: u32, count: u32) {
        self.uid_map.push((inside, outside, count));
    }

    /// Add a GID mapping
    pub fn add_gid_mapping(&mut self, inside: u32, outside: u32, count: u32) {
        self.gid_map.push((inside, outside, count));
    }

    /// Map UID from inside to outside namespace
    pub fn map_uid_to_parent(&self, uid: u32) -> Option<u32> {
        for &(inside, outside, count) in &self.uid_map {
            if uid >= inside && uid < inside + count {
                return Some(outside + (uid - inside));
            }
        }
        None
    }

    /// Map UID from outside to inside namespace
    pub fn map_uid_from_parent(&self, uid: u32) -> Option<u32> {
        for &(inside, outside, count) in &self.uid_map {
            if uid >= outside && uid < outside + count {
                return Some(inside + (uid - outside));
            }
        }
        None
    }

    /// Map GID from inside to outside namespace
    pub fn map_gid_to_parent(&self, gid: u32) -> Option<u32> {
        for &(inside, outside, count) in &self.gid_map {
            if gid >= inside && gid < inside + count {
                return Some(outside + (gid - inside));
            }
        }
        None
    }
}

/// Mount namespace data
#[derive(Debug)]
pub struct MountNamespace {
    pub id: u64,
    /// Root mount point
    pub root: String,
    /// All mount points in this namespace
    pub mounts: Vec<MountPoint>,
}

#[derive(Debug, Clone)]
pub struct MountPoint {
    pub source: String,
    pub target: String,
    pub fstype: String,
    pub flags: u64,
    pub data: String,
}

impl MountNamespace {
    pub fn new() -> Arc<IrqSafeMutex<Self>> {
        Arc::new(IrqSafeMutex::new(Self {
            id: next_namespace_id(),
            root: String::from("/"),
            mounts: Vec::new(),
        }))
    }

    /// Clone mount namespace (creates new namespace with same mounts)
    pub fn clone_ns(&self) -> Self {
        Self {
            id: next_namespace_id(),
            root: self.root.clone(),
            mounts: self.mounts.clone(),
        }
    }

    /// Add a mount point
    pub fn add_mount(&mut self, mount: MountPoint) {
        self.mounts.push(mount);
    }

    /// Remove a mount point
    pub fn remove_mount(&mut self, target: &str) -> bool {
        let orig_len = self.mounts.len();
        self.mounts.retain(|m| m.target != target);
        self.mounts.len() != orig_len
    }

    /// Find mount for a path
    pub fn find_mount(&self, path: &str) -> Option<&MountPoint> {
        self.mounts.iter()
            .filter(|m| path.starts_with(&m.target))
            .max_by_key(|m| m.target.len())
    }
}

/// Network namespace data
#[derive(Debug)]
pub struct NetNamespace {
    pub id: u64,
    /// Network interfaces
    pub interfaces: Vec<NetInterface>,
    /// Routing table
    pub routes: Vec<NetRoute>,
    /// Iptables rules (simplified)
    pub iptables: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct NetInterface {
    pub name: String,
    pub mac: [u8; 6],
    pub ipv4: Option<u32>,
    pub ipv6: Option<[u8; 16]>,
    pub mtu: u32,
    pub up: bool,
}

#[derive(Debug, Clone)]
pub struct NetRoute {
    pub destination: u32,
    pub gateway: u32,
    pub mask: u32,
    pub interface: String,
    pub metric: u32,
}

impl NetNamespace {
    pub fn new() -> Arc<IrqSafeMutex<Self>> {
        Arc::new(IrqSafeMutex::new(Self {
            id: next_namespace_id(),
            interfaces: alloc::vec![
                // Default loopback interface
                NetInterface {
                    name: String::from("lo"),
                    mac: [0, 0, 0, 0, 0, 0],
                    ipv4: Some(0x7f000001), // 127.0.0.1
                    ipv6: Some([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]),
                    mtu: 65536,
                    up: true,
                }
            ],
            routes: Vec::new(),
            iptables: Vec::new(),
        }))
    }

    /// Clone network namespace
    pub fn clone_ns(&self) -> Self {
        Self {
            id: next_namespace_id(),
            interfaces: alloc::vec![
                // New namespace gets only loopback
                NetInterface {
                    name: String::from("lo"),
                    mac: [0, 0, 0, 0, 0, 0],
                    ipv4: Some(0x7f000001),
                    ipv6: Some([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]),
                    mtu: 65536,
                    up: false, // Starts down in new namespace
                }
            ],
            routes: Vec::new(),
            iptables: Vec::new(),
        }
    }

    /// Add an interface
    pub fn add_interface(&mut self, iface: NetInterface) {
        self.interfaces.push(iface);
    }

    /// Get interface by name
    pub fn get_interface(&self, name: &str) -> Option<&NetInterface> {
        self.interfaces.iter().find(|i| i.name == name)
    }

    /// Get interface by name (mutable)
    pub fn get_interface_mut(&mut self, name: &str) -> Option<&mut NetInterface> {
        self.interfaces.iter_mut().find(|i| i.name == name)
    }
}

/// Cgroup namespace data
#[derive(Debug)]
pub struct CgroupNamespace {
    pub id: u64,
    /// Root cgroup path for this namespace
    pub root: String,
}

impl CgroupNamespace {
    pub fn new(root: &str) -> Arc<Self> {
        Arc::new(Self {
            id: next_namespace_id(),
            root: String::from(root),
        })
    }
}

/// Set of namespaces for a process
#[derive(Clone)]
pub struct NamespaceSet {
    pub pid_ns: Arc<PidNamespace>,
    pub uts_ns: Arc<IrqSafeMutex<UtsNamespace>>,
    pub ipc_ns: Arc<IrqSafeMutex<IpcNamespace>>,
    pub user_ns: Arc<UserNamespace>,
    pub mnt_ns: Arc<IrqSafeMutex<MountNamespace>>,
    pub net_ns: Arc<IrqSafeMutex<NetNamespace>>,
    pub cgroup_ns: Arc<CgroupNamespace>,
}

impl NamespaceSet {
    /// Create new namespace set with root (init) namespaces
    pub fn new_root() -> Self {
        Self {
            pid_ns: PidNamespace::new(None),
            uts_ns: UtsNamespace::new(),
            ipc_ns: IpcNamespace::new(),
            user_ns: UserNamespace::new(None, 0, 0),
            mnt_ns: MountNamespace::new(),
            net_ns: NetNamespace::new(),
            cgroup_ns: CgroupNamespace::new("/"),
        }
    }

    /// Clone namespace set (shares all namespaces)
    pub fn clone_share(&self) -> Self {
        self.clone()
    }

    /// Create new namespace(s) based on flags
    pub fn unshare(&self, ns_flags: u64) -> Self {
        let mut new = self.clone();

        if ns_flags & flags::CLONE_NEWPID != 0 {
            new.pid_ns = PidNamespace::new(Some(self.pid_ns.clone()));
        }

        if ns_flags & flags::CLONE_NEWUTS != 0 {
            let uts = self.uts_ns.lock().clone_ns();
            new.uts_ns = Arc::new(IrqSafeMutex::new(uts));
        }

        if ns_flags & flags::CLONE_NEWIPC != 0 {
            new.ipc_ns = IpcNamespace::new();
        }

        if ns_flags & flags::CLONE_NEWUSER != 0 {
            // Get current credentials for owner
            let owner_uid = 0; // In real impl, get from current cred
            let owner_gid = 0;
            new.user_ns = UserNamespace::new(Some(self.user_ns.clone()), owner_uid, owner_gid);
        }

        if ns_flags & flags::CLONE_NEWNS != 0 {
            let mnt = self.mnt_ns.lock().clone_ns();
            new.mnt_ns = Arc::new(IrqSafeMutex::new(mnt));
        }

        if ns_flags & flags::CLONE_NEWNET != 0 {
            let net = self.net_ns.lock().clone_ns();
            new.net_ns = Arc::new(IrqSafeMutex::new(net));
        }

        if ns_flags & flags::CLONE_NEWCGROUP != 0 {
            // Create new cgroup namespace with current cgroup as root
            new.cgroup_ns = CgroupNamespace::new("/");
        }

        new
    }

    /// Get namespace ID for a given type
    pub fn get_ns_id(&self, ns_type: NamespaceType) -> u64 {
        match ns_type {
            NamespaceType::Pid => self.pid_ns.id,
            NamespaceType::Uts => self.uts_ns.lock().id,
            NamespaceType::Ipc => self.ipc_ns.lock().id,
            NamespaceType::User => self.user_ns.id,
            NamespaceType::Mount => self.mnt_ns.lock().id,
            NamespaceType::Net => self.net_ns.lock().id,
            NamespaceType::Cgroup => self.cgroup_ns.id,
        }
    }
}

/// Global root namespace set
static ROOT_NAMESPACES: spin::Once<NamespaceSet> = spin::Once::new();

/// Initialize the root namespace set
pub fn init() {
    ROOT_NAMESPACES.call_once(NamespaceSet::new_root);
    crate::kprintln!("namespace: initialized root namespaces");
}

/// Get the root namespace set
pub fn root_namespaces() -> &'static NamespaceSet {
    ROOT_NAMESPACES.get().expect("Namespaces not initialized")
}

/// Create new namespaces based on clone flags
pub fn create_namespaces(parent: &NamespaceSet, flags: u64) -> NamespaceSet {
    if flags & flags::CLONE_NEWNS_ALL == 0 {
        // No new namespaces requested, share parent's
        parent.clone_share()
    } else {
        parent.unshare(flags)
    }
}

// =============================================================================
// System Calls
// =============================================================================

/// unshare - disassociate parts of the process execution context
/// Creates new namespace(s) for the calling process
pub fn sys_unshare(flags: u64) -> KResult<()> {
    // Validate flags
    if flags & !flags::CLONE_NEWNS_ALL != 0 {
        return Err(KError::Invalid);
    }

    // In a full implementation, this would update the current task's namespace set
    crate::kprintln!("namespace: unshare called with flags {:#x}", flags);

    Ok(())
}

/// setns - reassociate thread with a namespace
/// fd is an open file descriptor referring to a namespace
pub fn sys_setns(_fd: i32, _ns_type: u64) -> KResult<()> {
    // In a full implementation:
    // 1. Validate fd is a valid namespace fd
    // 2. Check permissions (CAP_SYS_ADMIN or matching user namespace)
    // 3. Switch to the target namespace

    crate::kprintln!("namespace: setns not fully implemented");
    Err(KError::NotSupported)
}

/// Get hostname (from UTS namespace)
pub fn get_hostname(ns: &NamespaceSet) -> String {
    ns.uts_ns.lock().hostname.clone()
}

/// Set hostname (in UTS namespace)
pub fn set_hostname(ns: &NamespaceSet, hostname: &str) -> KResult<()> {
    if hostname.len() > 64 {
        return Err(KError::Invalid);
    }
    ns.uts_ns.lock().hostname = String::from(hostname);
    Ok(())
}

/// Get domainname (from UTS namespace)
pub fn get_domainname(ns: &NamespaceSet) -> String {
    ns.uts_ns.lock().domainname.clone()
}

/// Set domainname (in UTS namespace)
pub fn set_domainname(ns: &NamespaceSet, domainname: &str) -> KResult<()> {
    if domainname.len() > 64 {
        return Err(KError::Invalid);
    }
    ns.uts_ns.lock().domainname = String::from(domainname);
    Ok(())
}

// =============================================================================
// procfs Interface
// =============================================================================

/// Get namespace info for procfs (/proc/pid/ns/*)
pub fn procfs_ns_info(ns: &NamespaceSet, ns_type: NamespaceType) -> String {
    let id = ns.get_ns_id(ns_type);
    alloc::format!("{}:[{}]", ns_type.proc_name(), id)
}

/// List all namespace types
pub fn list_namespace_types() -> Vec<&'static str> {
    alloc::vec!["mnt", "uts", "ipc", "user", "pid", "net", "cgroup"]
}
