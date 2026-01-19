//! Memory Control Groups (memcg)
//!
//! Per-process/group memory limits and accounting.

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::sync::IrqSafeMutex;

/// Memory cgroup ID
pub type CgroupId = u64;

/// Memory limit type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitType {
    /// Hard limit (OOM if exceeded)
    Hard,
    /// Soft limit (reclaim pressure)
    Soft,
    /// Swap limit
    Swap,
}

/// Memory statistics for a cgroup
#[derive(Debug, Clone, Default)]
pub struct MemcgStats {
    /// Current memory usage (bytes)
    pub usage: u64,
    /// Maximum usage seen (bytes)
    pub max_usage: u64,
    /// Memory limit (bytes)
    pub limit: u64,
    /// Soft limit (bytes)
    pub soft_limit: u64,
    /// Swap usage (bytes)
    pub swap_usage: u64,
    /// Swap limit (bytes)
    pub swap_limit: u64,
    /// Number of times limit hit
    pub limit_hits: u64,
    /// OOM events
    pub oom_events: u64,
    /// Pages charged
    pub pages_charged: u64,
    /// Pages uncharged
    pub pages_uncharged: u64,
    /// Reclaim attempts
    pub reclaim_attempts: u64,
    /// Pages reclaimed
    pub pages_reclaimed: u64,
}

/// Memory cgroup
#[derive(Debug, Clone)]
pub struct MemoryCgroup {
    /// Cgroup ID
    pub id: CgroupId,
    /// Name
    pub name: String,
    /// Parent cgroup ID (0 for root)
    pub parent_id: CgroupId,
    /// Memory limit (bytes, 0 = unlimited)
    pub limit: u64,
    /// Soft limit (bytes)
    pub soft_limit: u64,
    /// Swap limit (bytes, 0 = unlimited)
    pub swap_limit: u64,
    /// Current usage (bytes)
    pub usage: u64,
    /// Swap usage (bytes)
    pub swap_usage: u64,
    /// Maximum usage seen
    pub max_usage: u64,
    /// OOM control enabled
    pub oom_control: bool,
    /// Under OOM condition
    pub under_oom: bool,
    /// Associated process IDs
    pub pids: Vec<u32>,
    /// Statistics
    pub stats: MemcgStats,
}

impl MemoryCgroup {
    pub fn new(id: CgroupId, name: String) -> Self {
        Self {
            id,
            name,
            parent_id: 0,
            limit: 0, // Unlimited
            soft_limit: 0,
            swap_limit: 0,
            usage: 0,
            swap_usage: 0,
            max_usage: 0,
            oom_control: true,
            under_oom: false,
            pids: Vec::new(),
            stats: MemcgStats::default(),
        }
    }

    /// Check if limit is exceeded
    pub fn is_over_limit(&self) -> bool {
        self.limit > 0 && self.usage > self.limit
    }

    /// Check if soft limit is exceeded
    pub fn is_over_soft_limit(&self) -> bool {
        self.soft_limit > 0 && self.usage > self.soft_limit
    }

    /// Get available memory
    pub fn available(&self) -> u64 {
        if self.limit == 0 {
            u64::MAX
        } else {
            self.limit.saturating_sub(self.usage)
        }
    }

    /// Try to charge memory
    pub fn try_charge(&mut self, bytes: u64) -> bool {
        if self.limit > 0 && self.usage + bytes > self.limit {
            self.stats.limit_hits += 1;
            return false;
        }

        self.usage += bytes;
        self.stats.pages_charged += bytes / 4096;

        if self.usage > self.max_usage {
            self.max_usage = self.usage;
        }

        true
    }

    /// Uncharge memory
    pub fn uncharge(&mut self, bytes: u64) {
        self.usage = self.usage.saturating_sub(bytes);
        self.stats.pages_uncharged += bytes / 4096;
    }

    /// Try to charge swap
    pub fn try_charge_swap(&mut self, bytes: u64) -> bool {
        if self.swap_limit > 0 && self.swap_usage + bytes > self.swap_limit {
            return false;
        }
        self.swap_usage += bytes;
        true
    }

    /// Uncharge swap
    pub fn uncharge_swap(&mut self, bytes: u64) {
        self.swap_usage = self.swap_usage.saturating_sub(bytes);
    }

    /// Update stats
    pub fn update_stats(&mut self) {
        self.stats.usage = self.usage;
        self.stats.max_usage = self.max_usage;
        self.stats.limit = self.limit;
        self.stats.soft_limit = self.soft_limit;
        self.stats.swap_usage = self.swap_usage;
        self.stats.swap_limit = self.swap_limit;
    }
}

/// Memory cgroup manager
pub struct MemcgManager {
    /// All cgroups by ID
    cgroups: BTreeMap<CgroupId, MemoryCgroup>,
    /// Process to cgroup mapping
    pid_to_cgroup: BTreeMap<u32, CgroupId>,
    /// Next cgroup ID
    next_id: CgroupId,
    /// Global memory limit
    global_limit: u64,
    /// Global usage
    global_usage: u64,
}

impl MemcgManager {
    /// Create new manager
    pub fn new() -> Self {
        let mut mgr = Self {
            cgroups: BTreeMap::new(),
            pid_to_cgroup: BTreeMap::new(),
            next_id: 1,
            global_limit: 0,
            global_usage: 0,
        };

        // Create root cgroup
        let root = MemoryCgroup::new(0, "/".to_string());
        mgr.cgroups.insert(0, root);

        mgr
    }

    /// Create a new cgroup
    pub fn create_cgroup(&mut self, name: &str, parent_id: CgroupId) -> Option<CgroupId> {
        if !self.cgroups.contains_key(&parent_id) {
            return None;
        }

        let id = self.next_id;
        self.next_id += 1;

        let mut cgroup = MemoryCgroup::new(id, name.to_string());
        cgroup.parent_id = parent_id;

        self.cgroups.insert(id, cgroup);
        Some(id)
    }

    /// Delete a cgroup
    pub fn delete_cgroup(&mut self, id: CgroupId) -> bool {
        if id == 0 {
            return false; // Cannot delete root
        }

        // Move processes to parent
        if let Some(cgroup) = self.cgroups.get(&id) {
            let parent_id = cgroup.parent_id;
            let pids: Vec<u32> = cgroup.pids.clone();

            for pid in pids {
                self.move_process(pid, parent_id);
            }
        }

        self.cgroups.remove(&id).is_some()
    }

    /// Set memory limit for a cgroup
    pub fn set_limit(&mut self, id: CgroupId, limit: u64) -> bool {
        if let Some(cgroup) = self.cgroups.get_mut(&id) {
            cgroup.limit = limit;
            cgroup.stats.limit = limit;
            true
        } else {
            false
        }
    }

    /// Set soft limit for a cgroup
    pub fn set_soft_limit(&mut self, id: CgroupId, limit: u64) -> bool {
        if let Some(cgroup) = self.cgroups.get_mut(&id) {
            cgroup.soft_limit = limit;
            cgroup.stats.soft_limit = limit;
            true
        } else {
            false
        }
    }

    /// Set swap limit for a cgroup
    pub fn set_swap_limit(&mut self, id: CgroupId, limit: u64) -> bool {
        if let Some(cgroup) = self.cgroups.get_mut(&id) {
            cgroup.swap_limit = limit;
            cgroup.stats.swap_limit = limit;
            true
        } else {
            false
        }
    }

    /// Add process to cgroup
    pub fn add_process(&mut self, pid: u32, cgroup_id: CgroupId) -> bool {
        if !self.cgroups.contains_key(&cgroup_id) {
            return false;
        }

        // Remove from current cgroup
        if let Some(&old_id) = self.pid_to_cgroup.get(&pid) {
            if let Some(old_cgroup) = self.cgroups.get_mut(&old_id) {
                old_cgroup.pids.retain(|&p| p != pid);
            }
        }

        // Add to new cgroup
        if let Some(cgroup) = self.cgroups.get_mut(&cgroup_id) {
            cgroup.pids.push(pid);
        }

        self.pid_to_cgroup.insert(pid, cgroup_id);
        true
    }

    /// Move process to different cgroup
    pub fn move_process(&mut self, pid: u32, new_cgroup_id: CgroupId) -> bool {
        self.add_process(pid, new_cgroup_id)
    }

    /// Remove process
    pub fn remove_process(&mut self, pid: u32) {
        if let Some(cgroup_id) = self.pid_to_cgroup.remove(&pid) {
            if let Some(cgroup) = self.cgroups.get_mut(&cgroup_id) {
                cgroup.pids.retain(|&p| p != pid);
            }
        }
    }

    /// Get cgroup for process
    pub fn get_cgroup_for_pid(&self, pid: u32) -> CgroupId {
        self.pid_to_cgroup.get(&pid).copied().unwrap_or(0)
    }

    /// Try to charge memory for a process
    pub fn charge(&mut self, pid: u32, bytes: u64) -> bool {
        let cgroup_id = self.get_cgroup_for_pid(pid);

        // Walk up the hierarchy
        let mut id = cgroup_id;
        let mut charged = Vec::new();

        loop {
            if let Some(cgroup) = self.cgroups.get_mut(&id) {
                if !cgroup.try_charge(bytes) {
                    // Rollback
                    for charged_id in charged {
                        if let Some(c) = self.cgroups.get_mut(&charged_id) {
                            c.uncharge(bytes);
                        }
                    }
                    return false;
                }
                charged.push(id);

                if id == 0 {
                    break;
                }
                id = cgroup.parent_id;
            } else {
                break;
            }
        }

        self.global_usage += bytes;
        true
    }

    /// Uncharge memory for a process
    pub fn uncharge(&mut self, pid: u32, bytes: u64) {
        let cgroup_id = self.get_cgroup_for_pid(pid);

        let mut id = cgroup_id;
        loop {
            if let Some(cgroup) = self.cgroups.get_mut(&id) {
                cgroup.uncharge(bytes);
                if id == 0 {
                    break;
                }
                id = cgroup.parent_id;
            } else {
                break;
            }
        }

        self.global_usage = self.global_usage.saturating_sub(bytes);
    }

    /// Get cgroup
    pub fn get_cgroup(&self, id: CgroupId) -> Option<&MemoryCgroup> {
        self.cgroups.get(&id)
    }

    /// Get cgroup mut
    pub fn get_cgroup_mut(&mut self, id: CgroupId) -> Option<&mut MemoryCgroup> {
        self.cgroups.get_mut(&id)
    }

    /// Get all cgroups
    pub fn cgroups(&self) -> impl Iterator<Item = &MemoryCgroup> {
        self.cgroups.values()
    }

    /// Get stats for cgroup
    pub fn get_stats(&self, id: CgroupId) -> Option<MemcgStats> {
        self.cgroups.get(&id).map(|c| c.stats.clone())
    }

    /// Set global limit
    pub fn set_global_limit(&mut self, limit: u64) {
        self.global_limit = limit;
    }

    /// Get global usage
    pub fn global_usage(&self) -> u64 {
        self.global_usage
    }

    /// Format status string
    pub fn format_status(&self) -> String {
        alloc::format!(
            "Memcg: {} groups | Global usage: {} MB",
            self.cgroups.len(),
            self.global_usage / (1024 * 1024)
        )
    }
}

impl Default for MemcgManager {
    fn default() -> Self {
        Self::new()
    }
}

// Global memcg manager
static MEMCG: IrqSafeMutex<Option<MemcgManager>> = IrqSafeMutex::new(None);

/// Initialize memcg
pub fn init() {
    let mut mgr = MEMCG.lock();
    *mgr = Some(MemcgManager::new());
    crate::kprintln!("mm: Memory cgroups initialized");
}

/// Create cgroup
pub fn create_cgroup(name: &str, parent_id: CgroupId) -> Option<CgroupId> {
    MEMCG.lock().as_mut().and_then(|m| m.create_cgroup(name, parent_id))
}

/// Delete cgroup
pub fn delete_cgroup(id: CgroupId) -> bool {
    MEMCG.lock().as_mut().map(|m| m.delete_cgroup(id)).unwrap_or(false)
}

/// Set limit
pub fn set_limit(id: CgroupId, limit: u64) -> bool {
    MEMCG.lock().as_mut().map(|m| m.set_limit(id, limit)).unwrap_or(false)
}

/// Set soft limit
pub fn set_soft_limit(id: CgroupId, limit: u64) -> bool {
    MEMCG.lock().as_mut().map(|m| m.set_soft_limit(id, limit)).unwrap_or(false)
}

/// Add process to cgroup
pub fn add_process(pid: u32, cgroup_id: CgroupId) -> bool {
    MEMCG.lock().as_mut().map(|m| m.add_process(pid, cgroup_id)).unwrap_or(false)
}

/// Charge memory
pub fn charge(pid: u32, bytes: u64) -> bool {
    MEMCG.lock().as_mut().map(|m| m.charge(pid, bytes)).unwrap_or(true)
}

/// Uncharge memory
pub fn uncharge(pid: u32, bytes: u64) {
    if let Some(ref mut mgr) = *MEMCG.lock() {
        mgr.uncharge(pid, bytes);
    }
}

/// Get stats
pub fn get_stats(id: CgroupId) -> Option<MemcgStats> {
    MEMCG.lock().as_ref().and_then(|m| m.get_stats(id))
}

/// Get status string
pub fn status() -> String {
    MEMCG.lock().as_ref()
        .map(|m| m.format_status())
        .unwrap_or_else(|| "Memcg not initialized".to_string())
}
