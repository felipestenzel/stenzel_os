//! Control Groups (cgroups) - Resource Limits
//!
//! Linux-compatible cgroups v2 implementation for resource management.
//! Supports: CPU, memory, I/O, PIDs, and freezer controllers.

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

use crate::sync::IrqSafeMutex;
use crate::process::Pid;

/// Cgroup ID type
pub type CgroupId = u64;

/// Maximum cgroup hierarchy depth
pub const CGROUP_MAX_DEPTH: usize = 32;

/// Default CPU period (100ms in microseconds)
pub const DEFAULT_CPU_PERIOD_US: u64 = 100_000;

/// Memory page size for accounting
pub const PAGE_SIZE: usize = 4096;

// ============================================================================
// Cgroup Controllers
// ============================================================================

/// CPU controller - limits CPU usage
#[derive(Debug)]
pub struct CpuController {
    /// CPU shares (relative weight, default 1024)
    pub shares: AtomicU64,
    /// CFS quota in microseconds (-1 = unlimited)
    pub cfs_quota_us: AtomicU64,
    /// CFS period in microseconds
    pub cfs_period_us: AtomicU64,
    /// Accumulated CPU time used (nanoseconds)
    pub usage_ns: AtomicU64,
    /// Number of throttled periods
    pub nr_throttled: AtomicU64,
    /// Total throttled time (nanoseconds)
    pub throttled_time_ns: AtomicU64,
}

impl CpuController {
    pub fn new() -> Self {
        Self {
            shares: AtomicU64::new(1024),
            cfs_quota_us: AtomicU64::new(u64::MAX), // Unlimited
            cfs_period_us: AtomicU64::new(DEFAULT_CPU_PERIOD_US),
            usage_ns: AtomicU64::new(0),
            nr_throttled: AtomicU64::new(0),
            throttled_time_ns: AtomicU64::new(0),
        }
    }

    /// Set CPU shares (relative weight)
    pub fn set_shares(&self, shares: u64) {
        self.shares.store(shares.max(2).min(262144), Ordering::Release);
    }

    /// Set CFS quota (-1 for unlimited)
    pub fn set_quota(&self, quota_us: i64) {
        if quota_us < 0 {
            self.cfs_quota_us.store(u64::MAX, Ordering::Release);
        } else {
            self.cfs_quota_us.store(quota_us as u64, Ordering::Release);
        }
    }

    /// Set CFS period
    pub fn set_period(&self, period_us: u64) {
        let period = period_us.max(1000).min(1_000_000); // 1ms to 1s
        self.cfs_period_us.store(period, Ordering::Release);
    }

    /// Check if CPU quota allows execution
    pub fn can_run(&self, runtime_ns: u64) -> bool {
        let quota_us = self.cfs_quota_us.load(Ordering::Acquire);
        if quota_us == u64::MAX {
            return true; // Unlimited
        }

        let quota_ns = quota_us * 1000;
        let used = self.usage_ns.load(Ordering::Acquire);
        used + runtime_ns <= quota_ns
    }

    /// Account CPU usage
    pub fn charge(&self, runtime_ns: u64) {
        self.usage_ns.fetch_add(runtime_ns, Ordering::AcqRel);
    }

    /// Reset quota for new period
    pub fn reset_period(&self) {
        self.usage_ns.store(0, Ordering::Release);
    }

    /// Record throttling event
    pub fn throttle(&self, duration_ns: u64) {
        self.nr_throttled.fetch_add(1, Ordering::AcqRel);
        self.throttled_time_ns.fetch_add(duration_ns, Ordering::AcqRel);
    }

    /// Get CPU bandwidth limit as percentage (0 = unlimited)
    pub fn get_bandwidth_percent(&self) -> u32 {
        let quota = self.cfs_quota_us.load(Ordering::Acquire);
        let period = self.cfs_period_us.load(Ordering::Acquire);

        if quota == u64::MAX || period == 0 {
            return 0; // Unlimited
        }

        ((quota * 100) / period) as u32
    }
}

impl Default for CpuController {
    fn default() -> Self {
        Self::new()
    }
}

/// Memory controller - limits memory usage
#[derive(Debug)]
pub struct MemoryController {
    /// Memory limit in bytes (u64::MAX = unlimited)
    pub limit_bytes: AtomicU64,
    /// Soft limit (for reclaim pressure)
    pub soft_limit_bytes: AtomicU64,
    /// Current memory usage in bytes
    pub usage_bytes: AtomicU64,
    /// Maximum memory usage observed
    pub max_usage_bytes: AtomicU64,
    /// Number of times limit was hit
    pub failcnt: AtomicU64,
    /// Swap limit in bytes
    pub swap_limit_bytes: AtomicU64,
    /// Current swap usage
    pub swap_usage_bytes: AtomicU64,
    /// OOM control enabled
    pub oom_control: AtomicBool,
    /// Under OOM condition
    pub under_oom: AtomicBool,
}

impl MemoryController {
    pub fn new() -> Self {
        Self {
            limit_bytes: AtomicU64::new(u64::MAX),
            soft_limit_bytes: AtomicU64::new(u64::MAX),
            usage_bytes: AtomicU64::new(0),
            max_usage_bytes: AtomicU64::new(0),
            failcnt: AtomicU64::new(0),
            swap_limit_bytes: AtomicU64::new(u64::MAX),
            swap_usage_bytes: AtomicU64::new(0),
            oom_control: AtomicBool::new(true),
            under_oom: AtomicBool::new(false),
        }
    }

    /// Set memory limit
    pub fn set_limit(&self, limit: u64) {
        self.limit_bytes.store(limit, Ordering::Release);
    }

    /// Set soft limit
    pub fn set_soft_limit(&self, limit: u64) {
        self.soft_limit_bytes.store(limit, Ordering::Release);
    }

    /// Try to charge memory allocation
    pub fn try_charge(&self, bytes: u64) -> bool {
        let limit = self.limit_bytes.load(Ordering::Acquire);

        loop {
            let current = self.usage_bytes.load(Ordering::Acquire);
            let new_usage = current.saturating_add(bytes);

            if new_usage > limit {
                self.failcnt.fetch_add(1, Ordering::AcqRel);
                return false;
            }

            if self.usage_bytes.compare_exchange_weak(
                current,
                new_usage,
                Ordering::AcqRel,
                Ordering::Acquire,
            ).is_ok() {
                // Update max usage
                let mut max = self.max_usage_bytes.load(Ordering::Acquire);
                while new_usage > max {
                    match self.max_usage_bytes.compare_exchange_weak(
                        max,
                        new_usage,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    ) {
                        Ok(_) => break,
                        Err(current_max) => max = current_max,
                    }
                }
                return true;
            }
        }
    }

    /// Uncharge memory (free)
    pub fn uncharge(&self, bytes: u64) {
        self.usage_bytes.fetch_sub(bytes.min(self.usage_bytes.load(Ordering::Acquire)), Ordering::AcqRel);
    }

    /// Check if under memory pressure
    pub fn under_pressure(&self) -> bool {
        let usage = self.usage_bytes.load(Ordering::Acquire);
        let soft_limit = self.soft_limit_bytes.load(Ordering::Acquire);
        usage >= soft_limit
    }

    /// Get memory usage percentage
    pub fn usage_percent(&self) -> u32 {
        let usage = self.usage_bytes.load(Ordering::Acquire);
        let limit = self.limit_bytes.load(Ordering::Acquire);

        if limit == 0 || limit == u64::MAX {
            return 0;
        }

        ((usage * 100) / limit) as u32
    }

    /// Trigger OOM condition
    pub fn trigger_oom(&self) {
        if self.oom_control.load(Ordering::Acquire) {
            self.under_oom.store(true, Ordering::Release);
        }
    }

    /// Reset OOM state
    pub fn clear_oom(&self) {
        self.under_oom.store(false, Ordering::Release);
    }
}

impl Default for MemoryController {
    fn default() -> Self {
        Self::new()
    }
}

/// I/O controller - limits block device I/O
#[derive(Debug)]
pub struct IoController {
    /// Read bytes per second limit (0 = unlimited)
    pub read_bps_limit: AtomicU64,
    /// Write bytes per second limit
    pub write_bps_limit: AtomicU64,
    /// Read IOPS limit
    pub read_iops_limit: AtomicU64,
    /// Write IOPS limit
    pub write_iops_limit: AtomicU64,
    /// Total bytes read
    pub bytes_read: AtomicU64,
    /// Total bytes written
    pub bytes_written: AtomicU64,
    /// Total read operations
    pub read_ops: AtomicU64,
    /// Total write operations
    pub write_ops: AtomicU64,
    /// Weight for proportional I/O (default 100)
    pub weight: AtomicU64,
}

impl IoController {
    pub fn new() -> Self {
        Self {
            read_bps_limit: AtomicU64::new(0),
            write_bps_limit: AtomicU64::new(0),
            read_iops_limit: AtomicU64::new(0),
            write_iops_limit: AtomicU64::new(0),
            bytes_read: AtomicU64::new(0),
            bytes_written: AtomicU64::new(0),
            read_ops: AtomicU64::new(0),
            write_ops: AtomicU64::new(0),
            weight: AtomicU64::new(100),
        }
    }

    /// Set read bandwidth limit
    pub fn set_read_bps(&self, bps: u64) {
        self.read_bps_limit.store(bps, Ordering::Release);
    }

    /// Set write bandwidth limit
    pub fn set_write_bps(&self, bps: u64) {
        self.write_bps_limit.store(bps, Ordering::Release);
    }

    /// Set read IOPS limit
    pub fn set_read_iops(&self, iops: u64) {
        self.read_iops_limit.store(iops, Ordering::Release);
    }

    /// Set write IOPS limit
    pub fn set_write_iops(&self, iops: u64) {
        self.write_iops_limit.store(iops, Ordering::Release);
    }

    /// Set I/O weight
    pub fn set_weight(&self, weight: u64) {
        self.weight.store(weight.max(1).min(10000), Ordering::Release);
    }

    /// Account read operation
    pub fn account_read(&self, bytes: u64) {
        self.bytes_read.fetch_add(bytes, Ordering::AcqRel);
        self.read_ops.fetch_add(1, Ordering::AcqRel);
    }

    /// Account write operation
    pub fn account_write(&self, bytes: u64) {
        self.bytes_written.fetch_add(bytes, Ordering::AcqRel);
        self.write_ops.fetch_add(1, Ordering::AcqRel);
    }
}

impl Default for IoController {
    fn default() -> Self {
        Self::new()
    }
}

/// PIDs controller - limits number of processes
#[derive(Debug)]
pub struct PidsController {
    /// Maximum number of processes (0 = unlimited)
    pub max: AtomicU64,
    /// Current number of processes
    pub current: AtomicU64,
    /// Number of times limit was hit
    pub events_max: AtomicU64,
}

impl PidsController {
    pub fn new() -> Self {
        Self {
            max: AtomicU64::new(0), // 0 = unlimited
            current: AtomicU64::new(0),
            events_max: AtomicU64::new(0),
        }
    }

    /// Set PID limit
    pub fn set_max(&self, max: u64) {
        self.max.store(max, Ordering::Release);
    }

    /// Try to allocate a new PID slot
    pub fn try_charge(&self) -> bool {
        let max = self.max.load(Ordering::Acquire);

        if max == 0 {
            // Unlimited
            self.current.fetch_add(1, Ordering::AcqRel);
            return true;
        }

        loop {
            let current = self.current.load(Ordering::Acquire);
            if current >= max {
                self.events_max.fetch_add(1, Ordering::AcqRel);
                return false;
            }

            if self.current.compare_exchange_weak(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ).is_ok() {
                return true;
            }
        }
    }

    /// Release a PID slot
    pub fn uncharge(&self) {
        let current = self.current.load(Ordering::Acquire);
        if current > 0 {
            self.current.fetch_sub(1, Ordering::AcqRel);
        }
    }

    /// Get current count
    pub fn count(&self) -> u64 {
        self.current.load(Ordering::Acquire)
    }
}

impl Default for PidsController {
    fn default() -> Self {
        Self::new()
    }
}

/// Freezer controller - freeze/thaw processes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FreezerState {
    Thawed,
    Freezing,
    Frozen,
}

#[derive(Debug)]
pub struct FreezerController {
    /// Current state
    state: AtomicUsize, // 0=Thawed, 1=Freezing, 2=Frozen
    /// Self-freezing (for debugging)
    self_freezing: AtomicBool,
}

impl FreezerController {
    pub fn new() -> Self {
        Self {
            state: AtomicUsize::new(0), // Thawed
            self_freezing: AtomicBool::new(false),
        }
    }

    /// Get current freezer state
    pub fn state(&self) -> FreezerState {
        match self.state.load(Ordering::Acquire) {
            0 => FreezerState::Thawed,
            1 => FreezerState::Freezing,
            _ => FreezerState::Frozen,
        }
    }

    /// Request freeze
    pub fn freeze(&self) {
        self.state.store(1, Ordering::Release); // Freezing
    }

    /// Complete freeze
    pub fn frozen(&self) {
        self.state.store(2, Ordering::Release); // Frozen
    }

    /// Thaw processes
    pub fn thaw(&self) {
        self.state.store(0, Ordering::Release); // Thawed
    }

    /// Check if should stop execution
    pub fn should_stop(&self) -> bool {
        self.state.load(Ordering::Acquire) != 0
    }
}

impl Default for FreezerController {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Cgroup Node
// ============================================================================

/// A single cgroup in the hierarchy
pub struct Cgroup {
    /// Unique identifier
    pub id: CgroupId,
    /// Cgroup name
    pub name: String,
    /// Parent cgroup (None for root)
    pub parent: Option<Arc<Cgroup>>,
    /// Child cgroups
    pub children: IrqSafeMutex<BTreeMap<String, Arc<Cgroup>>>,
    /// Member PIDs
    pub members: IrqSafeMutex<Vec<Pid>>,
    /// CPU controller
    pub cpu: CpuController,
    /// Memory controller
    pub memory: MemoryController,
    /// I/O controller
    pub io: IoController,
    /// PIDs controller
    pub pids: PidsController,
    /// Freezer controller
    pub freezer: FreezerController,
    /// Depth in hierarchy
    pub depth: usize,
}

impl Cgroup {
    /// Create a new cgroup
    pub fn new(id: CgroupId, name: String, parent: Option<Arc<Cgroup>>) -> Self {
        let depth = parent.as_ref().map_or(0, |p| p.depth + 1);

        Self {
            id,
            name,
            parent,
            children: IrqSafeMutex::new(BTreeMap::new()),
            members: IrqSafeMutex::new(Vec::new()),
            cpu: CpuController::new(),
            memory: MemoryController::new(),
            io: IoController::new(),
            pids: PidsController::new(),
            freezer: FreezerController::new(),
            depth,
        }
    }

    /// Add a process to this cgroup
    pub fn add_process(&self, pid: Pid) -> bool {
        if !self.pids.try_charge() {
            return false;
        }

        let mut members = self.members.lock();
        if !members.contains(&pid) {
            members.push(pid);
        }
        true
    }

    /// Remove a process from this cgroup
    pub fn remove_process(&self, pid: Pid) {
        let mut members = self.members.lock();
        if let Some(pos) = members.iter().position(|&p| p == pid) {
            members.remove(pos);
            self.pids.uncharge();
        }
    }

    /// Check if process is member
    pub fn contains(&self, pid: Pid) -> bool {
        self.members.lock().contains(&pid)
    }

    /// Get number of processes
    pub fn process_count(&self) -> usize {
        self.members.lock().len()
    }

    /// Create a child cgroup
    pub fn create_child(self: &Arc<Self>, name: String, id: CgroupId) -> Option<Arc<Cgroup>> {
        if self.depth >= CGROUP_MAX_DEPTH - 1 {
            return None; // Max depth reached
        }

        let mut children = self.children.lock();
        if children.contains_key(&name) {
            return None; // Already exists
        }

        let child = Arc::new(Cgroup::new(id, name.clone(), Some(self.clone())));
        children.insert(name, child.clone());
        Some(child)
    }

    /// Remove a child cgroup (must be empty)
    pub fn remove_child(&self, name: &str) -> bool {
        let mut children = self.children.lock();

        if let Some(child) = children.get(name) {
            // Must be empty
            if child.process_count() > 0 || !child.children.lock().is_empty() {
                return false;
            }
            children.remove(name);
            return true;
        }
        false
    }

    /// Get child by name
    pub fn get_child(&self, name: &str) -> Option<Arc<Cgroup>> {
        self.children.lock().get(name).cloned()
    }

    /// Check all resource limits for a process
    pub fn check_limits(&self, _pid: Pid) -> bool {
        // Check freezer
        if self.freezer.should_stop() {
            return false;
        }

        // Check memory pressure
        if self.memory.under_oom.load(Ordering::Acquire) {
            return false;
        }

        true
    }

    /// Get full path from root
    pub fn path(&self) -> String {
        let mut parts = Vec::new();
        parts.push(self.name.clone());

        let mut current = self.parent.clone();
        while let Some(p) = current {
            parts.push(p.name.clone());
            current = p.parent.clone();
        }

        parts.reverse();
        parts.join("/")
    }
}

// ============================================================================
// Cgroup Hierarchy Manager
// ============================================================================

/// Global cgroup hierarchy manager
pub struct CgroupManager {
    /// Root cgroup
    root: Arc<Cgroup>,
    /// Next cgroup ID
    next_id: AtomicU64,
    /// Process to cgroup mapping
    process_cgroups: IrqSafeMutex<BTreeMap<Pid, Arc<Cgroup>>>,
}

impl CgroupManager {
    /// Create a new cgroup manager
    pub fn new() -> Self {
        let root = Arc::new(Cgroup::new(0, String::from("/"), None));

        Self {
            root,
            next_id: AtomicU64::new(1),
            process_cgroups: IrqSafeMutex::new(BTreeMap::new()),
        }
    }

    /// Get root cgroup
    pub fn root(&self) -> Arc<Cgroup> {
        self.root.clone()
    }

    /// Allocate a new cgroup ID
    fn alloc_id(&self) -> CgroupId {
        self.next_id.fetch_add(1, Ordering::AcqRel)
    }

    /// Create a cgroup by path
    pub fn create(&self, path: &str) -> Option<Arc<Cgroup>> {
        let parts: Vec<&str> = path.trim_matches('/').split('/').filter(|s| !s.is_empty()).collect();

        if parts.is_empty() {
            return Some(self.root.clone());
        }

        let mut current = self.root.clone();

        for (i, part) in parts.iter().enumerate() {
            if let Some(child) = current.get_child(part) {
                current = child;
            } else {
                // Create remaining path
                for create_part in &parts[i..] {
                    let id = self.alloc_id();
                    current = current.create_child(String::from(*create_part), id)?;
                }
                return Some(current);
            }
        }

        Some(current)
    }

    /// Get cgroup by path
    pub fn get(&self, path: &str) -> Option<Arc<Cgroup>> {
        let parts: Vec<&str> = path.trim_matches('/').split('/').filter(|s| !s.is_empty()).collect();

        if parts.is_empty() {
            return Some(self.root.clone());
        }

        let mut current = self.root.clone();

        for part in parts {
            current = current.get_child(part)?;
        }

        Some(current)
    }

    /// Delete a cgroup by path
    pub fn delete(&self, path: &str) -> bool {
        let parts: Vec<&str> = path.trim_matches('/').split('/').filter(|s| !s.is_empty()).collect();

        if parts.is_empty() {
            return false; // Cannot delete root
        }

        // Find parent
        let parent_parts = &parts[..parts.len() - 1];
        let name = parts[parts.len() - 1];

        let parent = if parent_parts.is_empty() {
            self.root.clone()
        } else {
            match self.get(&parent_parts.join("/")) {
                Some(p) => p,
                None => return false,
            }
        };

        parent.remove_child(name)
    }

    /// Move a process to a cgroup
    pub fn attach_process(&self, pid: Pid, cgroup: &Arc<Cgroup>) -> bool {
        let mut mapping = self.process_cgroups.lock();

        // Remove from old cgroup
        if let Some(old_cg) = mapping.get(&pid) {
            old_cg.remove_process(pid);
        }

        // Add to new cgroup
        if cgroup.add_process(pid) {
            mapping.insert(pid, cgroup.clone());
            true
        } else {
            false
        }
    }

    /// Get cgroup for a process
    pub fn get_process_cgroup(&self, pid: Pid) -> Option<Arc<Cgroup>> {
        self.process_cgroups.lock().get(&pid).cloned()
    }

    /// Remove a process from tracking (on exit)
    pub fn detach_process(&self, pid: Pid) {
        let mut mapping = self.process_cgroups.lock();

        if let Some(cg) = mapping.remove(&pid) {
            cg.remove_process(pid);
        }
    }

    /// Check if process can run (all controllers)
    pub fn can_run(&self, pid: Pid) -> bool {
        if let Some(cg) = self.get_process_cgroup(pid) {
            cg.check_limits(pid)
        } else {
            true // Not in any cgroup = unrestricted
        }
    }
}

impl Default for CgroupManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Global Instance
// ============================================================================

use spin::Once;

static CGROUP_MANAGER: Once<CgroupManager> = Once::new();

/// Initialize cgroups subsystem
pub fn init() {
    CGROUP_MANAGER.call_once(CgroupManager::new);
    crate::kprintln!("cgroups: Control groups subsystem initialized");
}

/// Get the global cgroup manager
pub fn manager() -> &'static CgroupManager {
    CGROUP_MANAGER.get().expect("cgroups not initialized")
}

// ============================================================================
// Syscall Interface
// ============================================================================

use crate::util::KError;

/// Create a new cgroup
///
/// # Arguments
/// * `path` - Cgroup path (e.g., "/docker/container1")
pub fn sys_cgroup_create(path: &str) -> Result<CgroupId, KError> {
    let cg = manager()
        .create(path)
        .ok_or(KError::Invalid)?;
    Ok(cg.id)
}

/// Delete a cgroup
pub fn sys_cgroup_delete(path: &str) -> Result<(), KError> {
    if manager().delete(path) {
        Ok(())
    } else {
        Err(KError::Busy)
    }
}

/// Attach current process to a cgroup
pub fn sys_cgroup_attach(path: &str, pid: Pid) -> Result<(), KError> {
    let cg = manager()
        .get(path)
        .ok_or(KError::NotFound)?;

    if manager().attach_process(pid, &cg) {
        Ok(())
    } else {
        Err(KError::NoMemory)
    }
}

/// Set CPU shares for a cgroup
pub fn sys_cgroup_set_cpu_shares(path: &str, shares: u64) -> Result<(), KError> {
    let cg = manager()
        .get(path)
        .ok_or(KError::NotFound)?;

    cg.cpu.set_shares(shares);
    Ok(())
}

/// Set CPU quota for a cgroup
pub fn sys_cgroup_set_cpu_quota(path: &str, quota_us: i64, period_us: u64) -> Result<(), KError> {
    let cg = manager()
        .get(path)
        .ok_or(KError::NotFound)?;

    cg.cpu.set_quota(quota_us);
    if period_us > 0 {
        cg.cpu.set_period(period_us);
    }
    Ok(())
}

/// Set memory limit for a cgroup
pub fn sys_cgroup_set_memory_limit(path: &str, limit_bytes: u64) -> Result<(), KError> {
    let cg = manager()
        .get(path)
        .ok_or(KError::NotFound)?;

    cg.memory.set_limit(limit_bytes);
    Ok(())
}

/// Set PID limit for a cgroup
pub fn sys_cgroup_set_pids_max(path: &str, max: u64) -> Result<(), KError> {
    let cg = manager()
        .get(path)
        .ok_or(KError::NotFound)?;

    cg.pids.set_max(max);
    Ok(())
}

/// Freeze a cgroup
pub fn sys_cgroup_freeze(path: &str) -> Result<(), KError> {
    let cg = manager()
        .get(path)
        .ok_or(KError::NotFound)?;

    cg.freezer.freeze();
    Ok(())
}

/// Thaw a cgroup
pub fn sys_cgroup_thaw(path: &str) -> Result<(), KError> {
    let cg = manager()
        .get(path)
        .ok_or(KError::NotFound)?;

    cg.freezer.thaw();
    Ok(())
}

/// Get cgroup stats
pub fn sys_cgroup_stat(path: &str) -> Result<CgroupStat, KError> {
    let cg = manager()
        .get(path)
        .ok_or(KError::NotFound)?;

    Ok(CgroupStat {
        id: cg.id,
        nr_processes: cg.process_count() as u64,
        cpu_usage_ns: cg.cpu.usage_ns.load(Ordering::Acquire),
        cpu_throttled_ns: cg.cpu.throttled_time_ns.load(Ordering::Acquire),
        memory_usage_bytes: cg.memory.usage_bytes.load(Ordering::Acquire),
        memory_limit_bytes: cg.memory.limit_bytes.load(Ordering::Acquire),
        io_read_bytes: cg.io.bytes_read.load(Ordering::Acquire),
        io_write_bytes: cg.io.bytes_written.load(Ordering::Acquire),
        freezer_state: cg.freezer.state(),
    })
}

/// Cgroup statistics
#[derive(Debug, Clone)]
pub struct CgroupStat {
    pub id: CgroupId,
    pub nr_processes: u64,
    pub cpu_usage_ns: u64,
    pub cpu_throttled_ns: u64,
    pub memory_usage_bytes: u64,
    pub memory_limit_bytes: u64,
    pub io_read_bytes: u64,
    pub io_write_bytes: u64,
    pub freezer_state: FreezerState,
}

// ============================================================================
// Scheduler Integration
// ============================================================================

/// Hook called by scheduler to check if process can run
pub fn sched_check_cgroup(pid: Pid) -> bool {
    if let Some(mgr) = CGROUP_MANAGER.get() {
        mgr.can_run(pid)
    } else {
        true
    }
}

/// Hook called by scheduler to account CPU time
pub fn sched_charge_cpu(pid: Pid, runtime_ns: u64) {
    if let Some(mgr) = CGROUP_MANAGER.get() {
        if let Some(cg) = mgr.get_process_cgroup(pid) {
            cg.cpu.charge(runtime_ns);
        }
    }
}

/// Hook called by memory allocator to charge memory
pub fn mm_try_charge(pid: Pid, bytes: u64) -> bool {
    if let Some(mgr) = CGROUP_MANAGER.get() {
        if let Some(cg) = mgr.get_process_cgroup(pid) {
            return cg.memory.try_charge(bytes);
        }
    }
    true // No cgroup = allow
}

/// Hook called by memory allocator to uncharge memory
pub fn mm_uncharge(pid: Pid, bytes: u64) {
    if let Some(mgr) = CGROUP_MANAGER.get() {
        if let Some(cg) = mgr.get_process_cgroup(pid) {
            cg.memory.uncharge(bytes);
        }
    }
}

/// Hook called when process exits
pub fn on_process_exit(pid: Pid) {
    if let Some(mgr) = CGROUP_MANAGER.get() {
        mgr.detach_process(pid);
    }
}
