//! Container Runtime Support
//!
//! Docker/OCI-compatible container runtime for running isolated workloads.
//! Uses namespaces and cgroups for isolation and resource limits.

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use alloc::vec;
use core::sync::atomic::{AtomicU64, AtomicBool, Ordering};

use crate::sync::IrqSafeMutex;
use crate::process::Pid;
use crate::ipc::namespace::{NamespaceSet, NamespaceType};
use crate::cgroups::{Cgroup, CgroupId};

/// Container ID type (64-byte hex string truncated)
pub type ContainerId = String;

/// Maximum containers per system
pub const MAX_CONTAINERS: usize = 256;

/// Container states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerState {
    /// Container is created but not started
    Created,
    /// Container is running
    Running,
    /// Container is paused (frozen)
    Paused,
    /// Container is stopped
    Stopped,
    /// Container is being removed
    Removing,
}

impl ContainerState {
    pub fn as_str(&self) -> &'static str {
        match self {
            ContainerState::Created => "created",
            ContainerState::Running => "running",
            ContainerState::Paused => "paused",
            ContainerState::Stopped => "stopped",
            ContainerState::Removing => "removing",
        }
    }
}

/// Container resource limits
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// CPU shares (relative weight, default 1024)
    pub cpu_shares: u64,
    /// CPU quota in microseconds (per 100ms period, 0 = unlimited)
    pub cpu_quota: i64,
    /// Number of CPU cores (0 = all)
    pub cpu_count: u32,
    /// Memory limit in bytes (0 = unlimited)
    pub memory_limit: u64,
    /// Memory + swap limit (0 = unlimited)
    pub memory_swap_limit: u64,
    /// Maximum PIDs (0 = unlimited)
    pub pids_limit: u64,
    /// Block I/O weight (10-1000)
    pub blkio_weight: u16,
    /// OOM kill disable
    pub oom_kill_disable: bool,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            cpu_shares: 1024,
            cpu_quota: 0,
            cpu_count: 0,
            memory_limit: 0,
            memory_swap_limit: 0,
            pids_limit: 0,
            blkio_weight: 500,
            oom_kill_disable: false,
        }
    }
}

/// Container network mode
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkMode {
    /// No network access
    None,
    /// Use host networking (no isolation)
    Host,
    /// Private network namespace with bridge
    Bridge(String),
    /// Share another container's network
    Container(ContainerId),
}

impl Default for NetworkMode {
    fn default() -> Self {
        NetworkMode::Bridge(String::from("docker0"))
    }
}

/// Container mount point
#[derive(Debug, Clone)]
pub struct Mount {
    /// Source path on host
    pub source: String,
    /// Destination path in container
    pub destination: String,
    /// Mount type (bind, tmpfs, etc.)
    pub mount_type: String,
    /// Mount options
    pub options: Vec<String>,
    /// Read-only mount
    pub read_only: bool,
}

/// Container configuration
#[derive(Debug, Clone)]
pub struct ContainerConfig {
    /// Container image name
    pub image: String,
    /// Command to run
    pub command: Vec<String>,
    /// Working directory
    pub working_dir: String,
    /// Environment variables
    pub env: Vec<(String, String)>,
    /// User to run as (uid:gid)
    pub user: Option<String>,
    /// Hostname
    pub hostname: String,
    /// Domain name
    pub domainname: String,
    /// Mount points
    pub mounts: Vec<Mount>,
    /// Network mode
    pub network_mode: NetworkMode,
    /// Resource limits
    pub resources: ResourceLimits,
    /// Privileged mode (no security restrictions)
    pub privileged: bool,
    /// Read-only rootfs
    pub read_only_rootfs: bool,
    /// Container labels
    pub labels: BTreeMap<String, String>,
    /// Restart policy
    pub restart_policy: RestartPolicy,
    /// Exposed ports (container_port -> host_port)
    pub port_bindings: Vec<(u16, u16)>,
}

impl Default for ContainerConfig {
    fn default() -> Self {
        Self {
            image: String::new(),
            command: Vec::new(),
            working_dir: String::from("/"),
            env: Vec::new(),
            user: None,
            hostname: String::new(),
            domainname: String::new(),
            mounts: Vec::new(),
            network_mode: NetworkMode::default(),
            resources: ResourceLimits::default(),
            privileged: false,
            read_only_rootfs: false,
            labels: BTreeMap::new(),
            restart_policy: RestartPolicy::No,
            port_bindings: Vec::new(),
        }
    }
}

/// Restart policy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartPolicy {
    /// Never restart
    No,
    /// Restart on failure
    OnFailure(u32), // max retry count
    /// Always restart
    Always,
    /// Restart unless explicitly stopped
    UnlessStopped,
}

/// Container information
pub struct Container {
    /// Unique container ID
    pub id: ContainerId,
    /// Short ID (first 12 chars)
    pub short_id: String,
    /// Container name
    pub name: String,
    /// Configuration
    pub config: ContainerConfig,
    /// Current state
    pub state: ContainerState,
    /// Process ID of init process
    pub pid: Option<Pid>,
    /// Exit code (when stopped)
    pub exit_code: Option<i32>,
    /// Creation timestamp
    pub created_at: u64,
    /// Start timestamp
    pub started_at: Option<u64>,
    /// Finish timestamp
    pub finished_at: Option<u64>,
    /// Namespace set
    pub namespaces: Option<Arc<NamespaceSet>>,
    /// Cgroup path
    pub cgroup_path: String,
    /// Cgroup ID
    pub cgroup_id: Option<CgroupId>,
    /// Root filesystem path
    pub rootfs: String,
    /// Restart count
    pub restart_count: u32,
    /// Is running
    running: AtomicBool,
}

impl Container {
    pub fn new(id: ContainerId, name: &str, config: ContainerConfig) -> Self {
        let short_id = id.chars().take(12).collect();
        Self {
            id: id.clone(),
            short_id,
            name: name.to_string(),
            config,
            state: ContainerState::Created,
            pid: None,
            exit_code: None,
            created_at: crate::time::uptime_secs(),
            started_at: None,
            finished_at: None,
            namespaces: None,
            cgroup_path: alloc::format!("/docker/{}", id.chars().take(12).collect::<String>()),
            cgroup_id: None,
            rootfs: String::new(),
            restart_count: 0,
            running: AtomicBool::new(false),
        }
    }

    /// Check if container is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    /// Get container info string
    pub fn info(&self) -> String {
        alloc::format!(
            "ID: {}\nName: {}\nImage: {}\nState: {}\nPID: {}\n",
            self.short_id,
            self.name,
            self.config.image,
            self.state.as_str(),
            self.pid.map_or(String::from("-"), |p| alloc::format!("{}", p.0))
        )
    }
}

/// Container runtime error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerError {
    NotFound,
    AlreadyExists,
    AlreadyRunning,
    NotRunning,
    InvalidConfig,
    ResourceLimit,
    NamespaceError,
    CgroupError,
    MountError,
    NetworkError,
    ImageNotFound,
    StartFailed,
    StopFailed,
    ExecFailed,
}

/// Container runtime
pub struct ContainerRuntime {
    /// All containers
    containers: IrqSafeMutex<BTreeMap<ContainerId, Arc<IrqSafeMutex<Container>>>>,
    /// Container name to ID mapping
    name_to_id: IrqSafeMutex<BTreeMap<String, ContainerId>>,
    /// Next container number (for auto-naming)
    next_num: AtomicU64,
    /// Bridge network interface
    bridge_name: String,
    /// Bridge network subnet
    bridge_subnet: String,
    /// Next container IP
    next_ip: AtomicU64,
}

impl ContainerRuntime {
    pub fn new() -> Self {
        Self {
            containers: IrqSafeMutex::new(BTreeMap::new()),
            name_to_id: IrqSafeMutex::new(BTreeMap::new()),
            next_num: AtomicU64::new(1),
            bridge_name: String::from("docker0"),
            bridge_subnet: String::from("172.17.0.0/16"),
            next_ip: AtomicU64::new(2), // .1 is the bridge
        }
    }

    /// Generate a new container ID
    fn generate_id(&self) -> ContainerId {
        let mut id = String::new();
        for _ in 0..64 {
            let byte = crate::crypto::random::get_random_u8();
            id.push_str(&alloc::format!("{:02x}", byte));
        }
        id.truncate(64);
        id
    }

    /// Generate a container name if not provided
    fn generate_name(&self) -> String {
        let num = self.next_num.fetch_add(1, Ordering::AcqRel);
        // Docker-style random names (simplified)
        let adjectives = ["happy", "sleepy", "brave", "clever", "gentle"];
        let nouns = ["alpine", "panda", "phoenix", "whale", "dolphin"];
        let adj_idx = (num as usize) % adjectives.len();
        let noun_idx = ((num / 5) as usize) % nouns.len();
        alloc::format!("{}_{}", adjectives[adj_idx], nouns[noun_idx])
    }

    /// Create a new container
    pub fn create(
        &self,
        name: Option<&str>,
        config: ContainerConfig,
    ) -> Result<ContainerId, ContainerError> {
        let container_name = name.map(String::from).unwrap_or_else(|| self.generate_name());

        // Check if name already exists
        if self.name_to_id.lock().contains_key(&container_name) {
            return Err(ContainerError::AlreadyExists);
        }

        // Check container limit
        if self.containers.lock().len() >= MAX_CONTAINERS {
            return Err(ContainerError::ResourceLimit);
        }

        let id = self.generate_id();
        let container = Container::new(id.clone(), &container_name, config);

        // Store container
        let container_arc = Arc::new(IrqSafeMutex::new(container));
        self.containers.lock().insert(id.clone(), container_arc);
        self.name_to_id.lock().insert(container_name, id.clone());

        crate::kprintln!("container: Created container {}", &id[..12]);
        Ok(id)
    }

    /// Start a container
    pub fn start(&self, id_or_name: &str) -> Result<(), ContainerError> {
        let id = self.resolve_id(id_or_name)?;
        let container = self.get_container(&id)?;
        let mut c = container.lock();

        if c.is_running() {
            return Err(ContainerError::AlreadyRunning);
        }

        // Create namespaces
        let ns = Arc::new(NamespaceSet::new_root());
        c.namespaces = Some(ns.clone());

        // Create cgroup
        let cgroup_path = c.cgroup_path.clone();
        if let Ok(cgroup_id) = crate::cgroups::sys_cgroup_create(&cgroup_path) {
            c.cgroup_id = Some(cgroup_id);

            // Apply resource limits
            let _ = crate::cgroups::sys_cgroup_set_cpu_shares(&cgroup_path, c.config.resources.cpu_shares);
            let _ = crate::cgroups::sys_cgroup_set_cpu_quota(&cgroup_path, c.config.resources.cpu_quota, 100_000);
            if c.config.resources.memory_limit > 0 {
                let _ = crate::cgroups::sys_cgroup_set_memory_limit(&cgroup_path, c.config.resources.memory_limit);
            }
            if c.config.resources.pids_limit > 0 {
                let _ = crate::cgroups::sys_cgroup_set_pids_max(&cgroup_path, c.config.resources.pids_limit);
            }
        }

        // Set hostname in UTS namespace
        if let Some(ref ns) = c.namespaces {
            let hostname = if c.config.hostname.is_empty() {
                c.short_id.clone()
            } else {
                c.config.hostname.clone()
            };
            let _ = crate::ipc::namespace::set_hostname(ns, &hostname);
        }

        // Update state
        c.state = ContainerState::Running;
        c.started_at = Some(crate::time::uptime_secs());
        c.running.store(true, Ordering::Release);

        // In a real implementation, we would:
        // 1. Pivot root to container rootfs
        // 2. Apply mounts
        // 3. Setup networking
        // 4. Drop capabilities (unless privileged)
        // 5. Fork and exec the init process

        crate::kprintln!("container: Started container {}", &c.short_id);
        Ok(())
    }

    /// Stop a container
    pub fn stop(&self, id_or_name: &str, timeout_secs: u32) -> Result<(), ContainerError> {
        let id = self.resolve_id(id_or_name)?;
        let container = self.get_container(&id)?;
        let mut c = container.lock();

        if !c.is_running() {
            return Err(ContainerError::NotRunning);
        }

        // In a real implementation:
        // 1. Send SIGTERM to init process
        // 2. Wait timeout
        // 3. Send SIGKILL if still running

        let _ = timeout_secs; // Would use for wait

        // Freeze cgroup
        if let Some(_cgroup_id) = c.cgroup_id {
            let _ = crate::cgroups::sys_cgroup_freeze(&c.cgroup_path);
        }

        // Update state
        c.state = ContainerState::Stopped;
        c.finished_at = Some(crate::time::uptime_secs());
        c.running.store(false, Ordering::Release);
        c.exit_code = Some(0);

        crate::kprintln!("container: Stopped container {}", &c.short_id);
        Ok(())
    }

    /// Kill a container
    pub fn kill(&self, id_or_name: &str, signal: i32) -> Result<(), ContainerError> {
        let id = self.resolve_id(id_or_name)?;
        let container = self.get_container(&id)?;
        let c = container.lock();

        if !c.is_running() {
            return Err(ContainerError::NotRunning);
        }

        // Send signal to init process
        if let Some(pid) = c.pid {
            let _ = crate::sched::send_signal(pid.0 as i64, signal as u32);
        }

        Ok(())
    }

    /// Pause a container
    pub fn pause(&self, id_or_name: &str) -> Result<(), ContainerError> {
        let id = self.resolve_id(id_or_name)?;
        let container = self.get_container(&id)?;
        let mut c = container.lock();

        if !c.is_running() {
            return Err(ContainerError::NotRunning);
        }

        if c.state == ContainerState::Paused {
            return Ok(());
        }

        // Freeze cgroup
        let _ = crate::cgroups::sys_cgroup_freeze(&c.cgroup_path);
        c.state = ContainerState::Paused;

        crate::kprintln!("container: Paused container {}", &c.short_id);
        Ok(())
    }

    /// Unpause a container
    pub fn unpause(&self, id_or_name: &str) -> Result<(), ContainerError> {
        let id = self.resolve_id(id_or_name)?;
        let container = self.get_container(&id)?;
        let mut c = container.lock();

        if c.state != ContainerState::Paused {
            return Err(ContainerError::NotRunning);
        }

        // Thaw cgroup
        let _ = crate::cgroups::sys_cgroup_thaw(&c.cgroup_path);
        c.state = ContainerState::Running;

        crate::kprintln!("container: Unpaused container {}", &c.short_id);
        Ok(())
    }

    /// Remove a container
    pub fn remove(&self, id_or_name: &str, force: bool) -> Result<(), ContainerError> {
        let id = self.resolve_id(id_or_name)?;
        let container = self.get_container(&id)?;

        {
            let mut c = container.lock();
            if c.is_running() {
                if force {
                    c.state = ContainerState::Removing;
                    c.running.store(false, Ordering::Release);
                } else {
                    return Err(ContainerError::AlreadyRunning);
                }
            }

            // Clean up cgroup
            if c.cgroup_id.is_some() {
                let _ = crate::cgroups::sys_cgroup_delete(&c.cgroup_path);
            }

            // Remove name mapping
            self.name_to_id.lock().remove(&c.name);
        }

        // Remove container
        self.containers.lock().remove(&id);

        crate::kprintln!("container: Removed container {}", &id[..12]);
        Ok(())
    }

    /// List containers
    pub fn list(&self, all: bool) -> Vec<ContainerInfo> {
        let containers = self.containers.lock();
        let mut result = Vec::new();

        for (_id, container) in containers.iter() {
            let c = container.lock();
            if all || c.is_running() {
                result.push(ContainerInfo {
                    id: c.id.clone(),
                    short_id: c.short_id.clone(),
                    name: c.name.clone(),
                    image: c.config.image.clone(),
                    state: c.state,
                    created_at: c.created_at,
                    started_at: c.started_at,
                });
            }
        }

        result
    }

    /// Get container by ID or name
    fn get_container(&self, id: &str) -> Result<Arc<IrqSafeMutex<Container>>, ContainerError> {
        self.containers
            .lock()
            .get(id)
            .cloned()
            .ok_or(ContainerError::NotFound)
    }

    /// Resolve container ID from name or partial ID
    fn resolve_id(&self, id_or_name: &str) -> Result<ContainerId, ContainerError> {
        // Try as name first
        if let Some(id) = self.name_to_id.lock().get(id_or_name) {
            return Ok(id.clone());
        }

        // Try as full ID
        if self.containers.lock().contains_key(id_or_name) {
            return Ok(id_or_name.to_string());
        }

        // Try as partial ID
        let containers = self.containers.lock();
        let matches: Vec<_> = containers
            .keys()
            .filter(|k| k.starts_with(id_or_name))
            .collect();

        match matches.len() {
            0 => Err(ContainerError::NotFound),
            1 => Ok(matches[0].clone()),
            _ => Err(ContainerError::InvalidConfig), // Ambiguous
        }
    }

    /// Inspect a container
    pub fn inspect(&self, id_or_name: &str) -> Result<ContainerInspect, ContainerError> {
        let id = self.resolve_id(id_or_name)?;
        let container = self.get_container(&id)?;
        let c = container.lock();

        Ok(ContainerInspect {
            id: c.id.clone(),
            name: c.name.clone(),
            image: c.config.image.clone(),
            state: c.state,
            pid: c.pid.map(|p| p.0),
            exit_code: c.exit_code,
            created_at: c.created_at,
            started_at: c.started_at,
            finished_at: c.finished_at,
            restart_count: c.restart_count,
            rootfs: c.rootfs.clone(),
            hostname: c.config.hostname.clone(),
            working_dir: c.config.working_dir.clone(),
            env: c.config.env.clone(),
            cmd: c.config.command.clone(),
            mounts: c.config.mounts.iter().map(|m| (m.source.clone(), m.destination.clone())).collect(),
            port_bindings: c.config.port_bindings.clone(),
            resources: c.config.resources.clone(),
        })
    }

    /// Get container logs
    pub fn logs(&self, id_or_name: &str, _tail: Option<usize>) -> Result<Vec<String>, ContainerError> {
        let _id = self.resolve_id(id_or_name)?;
        // In a real implementation, we would read from the container's log files
        Ok(vec![String::from("[container logs not implemented]")])
    }

    /// Execute a command in a running container
    pub fn exec(&self, id_or_name: &str, _cmd: &[String]) -> Result<i32, ContainerError> {
        let id = self.resolve_id(id_or_name)?;
        let container = self.get_container(&id)?;
        let c = container.lock();

        if !c.is_running() {
            return Err(ContainerError::NotRunning);
        }

        // In a real implementation:
        // 1. Enter container namespaces
        // 2. Fork and exec command
        // 3. Wait for exit code

        Ok(0)
    }

    /// Get container stats
    pub fn stats(&self, id_or_name: &str) -> Result<ContainerStats, ContainerError> {
        let id = self.resolve_id(id_or_name)?;
        let container = self.get_container(&id)?;
        let c = container.lock();

        // Get cgroup stats
        let cgroup_stat = if let Some(_cgroup_id) = c.cgroup_id {
            crate::cgroups::sys_cgroup_stat(&c.cgroup_path).ok()
        } else {
            None
        };

        Ok(ContainerStats {
            id: c.short_id.clone(),
            name: c.name.clone(),
            cpu_usage_ns: cgroup_stat.as_ref().map_or(0, |s| s.cpu_usage_ns),
            memory_usage: cgroup_stat.as_ref().map_or(0, |s| s.memory_usage_bytes),
            memory_limit: c.config.resources.memory_limit,
            pids: cgroup_stat.as_ref().map_or(0, |s| s.nr_processes),
            net_rx_bytes: 0,
            net_tx_bytes: 0,
            block_read_bytes: cgroup_stat.as_ref().map_or(0, |s| s.io_read_bytes),
            block_write_bytes: cgroup_stat.as_ref().map_or(0, |s| s.io_write_bytes),
        })
    }
}

impl Default for ContainerRuntime {
    fn default() -> Self {
        Self::new()
    }
}

/// Container listing info
#[derive(Debug, Clone)]
pub struct ContainerInfo {
    pub id: ContainerId,
    pub short_id: String,
    pub name: String,
    pub image: String,
    pub state: ContainerState,
    pub created_at: u64,
    pub started_at: Option<u64>,
}

/// Container inspection info
#[derive(Debug, Clone)]
pub struct ContainerInspect {
    pub id: ContainerId,
    pub name: String,
    pub image: String,
    pub state: ContainerState,
    pub pid: Option<u64>,
    pub exit_code: Option<i32>,
    pub created_at: u64,
    pub started_at: Option<u64>,
    pub finished_at: Option<u64>,
    pub restart_count: u32,
    pub rootfs: String,
    pub hostname: String,
    pub working_dir: String,
    pub env: Vec<(String, String)>,
    pub cmd: Vec<String>,
    pub mounts: Vec<(String, String)>,
    pub port_bindings: Vec<(u16, u16)>,
    pub resources: ResourceLimits,
}

/// Container stats
#[derive(Debug, Clone)]
pub struct ContainerStats {
    pub id: String,
    pub name: String,
    pub cpu_usage_ns: u64,
    pub memory_usage: u64,
    pub memory_limit: u64,
    pub pids: u64,
    pub net_rx_bytes: u64,
    pub net_tx_bytes: u64,
    pub block_read_bytes: u64,
    pub block_write_bytes: u64,
}

// ============================================================================
// Global Instance
// ============================================================================

use spin::Once;

static CONTAINER_RUNTIME: Once<ContainerRuntime> = Once::new();

/// Initialize container runtime
pub fn init() {
    CONTAINER_RUNTIME.call_once(ContainerRuntime::new);
    crate::kprintln!("containers: Docker/OCI runtime initialized");
}

/// Get the global container runtime
pub fn runtime() -> &'static ContainerRuntime {
    CONTAINER_RUNTIME.get().expect("container runtime not initialized")
}

// ============================================================================
// Syscall Interface
// ============================================================================

use crate::util::KError;

/// Create a container
pub fn sys_container_create(name: Option<&str>, config: ContainerConfig) -> Result<ContainerId, KError> {
    runtime()
        .create(name, config)
        .map_err(|e| match e {
            ContainerError::AlreadyExists => KError::AlreadyExists,
            ContainerError::ResourceLimit => KError::NoMemory,
            _ => KError::IO,
        })
}

/// Start a container
pub fn sys_container_start(id: &str) -> Result<(), KError> {
    runtime()
        .start(id)
        .map_err(|e| match e {
            ContainerError::NotFound => KError::NotFound,
            ContainerError::AlreadyRunning => KError::Busy,
            _ => KError::IO,
        })
}

/// Stop a container
pub fn sys_container_stop(id: &str, timeout: u32) -> Result<(), KError> {
    runtime()
        .stop(id, timeout)
        .map_err(|e| match e {
            ContainerError::NotFound => KError::NotFound,
            ContainerError::NotRunning => KError::Invalid,
            _ => KError::IO,
        })
}

/// Kill a container
pub fn sys_container_kill(id: &str, signal: i32) -> Result<(), KError> {
    runtime()
        .kill(id, signal)
        .map_err(|e| match e {
            ContainerError::NotFound => KError::NotFound,
            ContainerError::NotRunning => KError::Invalid,
            _ => KError::IO,
        })
}

/// Remove a container
pub fn sys_container_remove(id: &str, force: bool) -> Result<(), KError> {
    runtime()
        .remove(id, force)
        .map_err(|e| match e {
            ContainerError::NotFound => KError::NotFound,
            ContainerError::AlreadyRunning => KError::Busy,
            _ => KError::IO,
        })
}

/// List containers
pub fn sys_container_list(all: bool) -> Vec<ContainerInfo> {
    runtime().list(all)
}

/// Pause a container
pub fn sys_container_pause(id: &str) -> Result<(), KError> {
    runtime()
        .pause(id)
        .map_err(|e| match e {
            ContainerError::NotFound => KError::NotFound,
            ContainerError::NotRunning => KError::Invalid,
            _ => KError::IO,
        })
}

/// Unpause a container
pub fn sys_container_unpause(id: &str) -> Result<(), KError> {
    runtime()
        .unpause(id)
        .map_err(|e| match e {
            ContainerError::NotFound => KError::NotFound,
            ContainerError::NotRunning => KError::Invalid,
            _ => KError::IO,
        })
}

/// Inspect a container
pub fn sys_container_inspect(id: &str) -> Result<ContainerInspect, KError> {
    runtime()
        .inspect(id)
        .map_err(|e| match e {
            ContainerError::NotFound => KError::NotFound,
            _ => KError::IO,
        })
}

/// Get container stats
pub fn sys_container_stats(id: &str) -> Result<ContainerStats, KError> {
    runtime()
        .stats(id)
        .map_err(|e| match e {
            ContainerError::NotFound => KError::NotFound,
            _ => KError::IO,
        })
}
