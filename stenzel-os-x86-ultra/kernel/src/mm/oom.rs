//! Out-of-Memory (OOM) Killer
//!
//! Intelligent OOM killer that selects processes to kill when the system
//! runs critically low on memory. Uses a scoring system based on:
//! - Process memory usage
//! - Process priority/niceness
//! - Process age (uptime)
//! - User preferences (protected processes)
//! - OOM score adjustment

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use crate::sync::IrqSafeMutex;

/// Process ID type
pub type Pid = u64;

/// OOM score for a process (higher = more likely to be killed)
pub type OomScore = i32;

/// OOM adjustment value (-1000 to 1000)
/// -1000 = protected (never kill)
/// 0 = default
/// 1000 = always prefer to kill
pub type OomAdjustment = i16;

/// Memory threshold levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryPressure {
    /// Normal memory usage
    None,
    /// Low memory warning (start reclaiming)
    Low,
    /// Critical memory (OOM imminent)
    Critical,
    /// OOM triggered
    OomTriggered,
}

/// OOM policy for a process
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OomPolicy {
    /// Normal OOM handling
    Normal,
    /// Protected from OOM killer (try to kill others first)
    Protected,
    /// Never kill (system critical)
    NeverKill,
    /// Prefer to kill this process
    PreferKill,
    /// Kill entire process group
    KillGroup,
}

/// Memory usage statistics for a process
#[derive(Debug, Clone, Default)]
pub struct ProcessMemoryStats {
    /// Resident set size (bytes)
    pub rss: u64,
    /// Virtual memory size (bytes)
    pub vms: u64,
    /// Shared memory (bytes)
    pub shared: u64,
    /// Private memory (bytes)
    pub private: u64,
    /// Swap usage (bytes)
    pub swap: u64,
    /// Page cache usage (bytes)
    pub page_cache: u64,
}

impl ProcessMemoryStats {
    /// Total memory impact (weighted for OOM scoring)
    pub fn total_impact(&self) -> u64 {
        // RSS is most important, then private, then swap
        self.rss + (self.private / 2) + (self.swap / 4)
    }
}

/// OOM process info
#[derive(Debug, Clone)]
pub struct OomProcessInfo {
    /// Process ID
    pub pid: Pid,
    /// Process name
    pub name: String,
    /// Parent process ID
    pub ppid: Pid,
    /// User ID
    pub uid: u32,
    /// Memory stats
    pub memory: ProcessMemoryStats,
    /// Process niceness (-20 to 19)
    pub nice: i8,
    /// Process start time (seconds since boot)
    pub start_time: u64,
    /// OOM adjustment
    pub oom_adj: OomAdjustment,
    /// OOM policy
    pub policy: OomPolicy,
    /// Is this a kernel thread?
    pub is_kernel: bool,
    /// Children PIDs
    pub children: Vec<Pid>,
}

impl OomProcessInfo {
    /// Calculate OOM score for this process
    /// Higher score = more likely to be killed
    pub fn calculate_score(&self, total_memory: u64) -> OomScore {
        // Never kill kernel threads
        if self.is_kernel {
            return OomScore::MIN;
        }

        // Check policy
        match self.policy {
            OomPolicy::NeverKill => return OomScore::MIN,
            OomPolicy::Protected => {
                // Very low base score for protected processes
                if self.oom_adj >= 0 {
                    return -900;
                }
            }
            OomPolicy::PreferKill => {
                // Start with high base score
                return 900 + self.oom_adj as i32;
            }
            _ => {}
        }

        // Base score from memory usage percentage (0-1000)
        let mem_pct = if total_memory > 0 {
            ((self.memory.total_impact() * 1000) / total_memory) as i32
        } else {
            0
        };

        // Adjust for niceness (nice processes are more likely to be killed)
        // nice -20 to 19 maps to -50 to +50
        let nice_adj = ((self.nice as i32) * 50) / 20;

        // Adjust for process age (older processes less likely to be killed)
        // Processes older than 10 minutes get bonus protection
        let age_adj = if self.start_time > 600 {
            -50
        } else if self.start_time > 60 {
            -25
        } else {
            0
        };

        // Calculate final score
        let mut score = mem_pct + nice_adj + age_adj;

        // Apply user adjustment (-1000 to 1000)
        score += self.oom_adj as i32;

        // Clamp to valid range
        score.clamp(-1000, 1000)
    }
}

/// OOM kill result
#[derive(Debug, Clone)]
pub struct OomKillResult {
    /// Process that was killed
    pub pid: Pid,
    /// Process name
    pub name: String,
    /// Memory freed (estimated)
    pub memory_freed: u64,
    /// OOM score at time of kill
    pub score: OomScore,
    /// Why this process was selected
    pub reason: String,
}

/// OOM statistics
#[derive(Debug, Clone, Default)]
pub struct OomStats {
    /// Total OOM kills
    pub total_kills: u64,
    /// Total memory freed by OOM kills
    pub total_freed: u64,
    /// Last OOM kill timestamp
    pub last_kill_time: u64,
    /// Number of OOM situations
    pub oom_count: u64,
    /// Number of times OOM was avoided by reclaim
    pub reclaim_success: u64,
}

/// OOM killer configuration
#[derive(Debug, Clone)]
pub struct OomConfig {
    /// Enable OOM killer
    pub enabled: bool,
    /// Panic instead of killing processes
    pub panic_on_oom: bool,
    /// Minimum score to consider for killing
    pub min_score_threshold: OomScore,
    /// Try memory reclaim before killing
    pub try_reclaim_first: bool,
    /// Maximum processes to consider
    pub max_candidates: usize,
    /// Kill children with parent
    pub kill_children: bool,
    /// Memory pressure thresholds (percentage of total)
    pub low_memory_threshold: u8,     // e.g., 20%
    pub critical_threshold: u8,        // e.g., 5%
    /// Notification callback (if set)
    pub notify_on_kill: bool,
}

impl Default for OomConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            panic_on_oom: false,
            min_score_threshold: -900,
            try_reclaim_first: true,
            max_candidates: 100,
            kill_children: true,
            low_memory_threshold: 20,
            critical_threshold: 5,
            notify_on_kill: true,
        }
    }
}

/// OOM killer notification
#[derive(Debug, Clone)]
pub struct OomNotification {
    /// Notification type
    pub kind: OomNotificationKind,
    /// Timestamp
    pub timestamp: u64,
    /// Details
    pub message: String,
}

/// OOM notification types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OomNotificationKind {
    /// Low memory warning
    LowMemory,
    /// Critical memory
    CriticalMemory,
    /// Process was killed
    ProcessKilled,
    /// OOM panic
    OomPanic,
}

/// Process info provider callback type
pub type ProcessInfoProvider = fn() -> Vec<OomProcessInfo>;

/// Memory info provider callback type
pub type MemoryInfoProvider = fn() -> (u64, u64); // (total, available)

/// Process kill callback type
pub type ProcessKillFn = fn(Pid) -> bool;

/// OOM Killer manager
pub struct OomKiller {
    /// Configuration
    config: OomConfig,
    /// Statistics
    stats: OomStats,
    /// Per-process OOM adjustments
    adjustments: BTreeMap<Pid, OomAdjustment>,
    /// Per-process policies
    policies: BTreeMap<Pid, OomPolicy>,
    /// Protected processes
    protected: Vec<Pid>,
    /// Notification history
    notifications: Vec<OomNotification>,
    /// Current memory pressure level
    pressure: MemoryPressure,
    /// Process info provider
    process_info_fn: Option<ProcessInfoProvider>,
    /// Memory info provider
    memory_info_fn: Option<MemoryInfoProvider>,
    /// Process kill function
    kill_fn: Option<ProcessKillFn>,
}

impl OomKiller {
    /// Create new OOM killer
    pub const fn new() -> Self {
        Self {
            config: OomConfig {
                enabled: true,
                panic_on_oom: false,
                min_score_threshold: -900,
                try_reclaim_first: true,
                max_candidates: 100,
                kill_children: true,
                low_memory_threshold: 20,
                critical_threshold: 5,
                notify_on_kill: true,
            },
            stats: OomStats {
                total_kills: 0,
                total_freed: 0,
                last_kill_time: 0,
                oom_count: 0,
                reclaim_success: 0,
            },
            adjustments: BTreeMap::new(),
            policies: BTreeMap::new(),
            protected: Vec::new(),
            notifications: Vec::new(),
            pressure: MemoryPressure::None,
            process_info_fn: None,
            memory_info_fn: None,
            kill_fn: None,
        }
    }

    /// Set configuration
    pub fn set_config(&mut self, config: OomConfig) {
        self.config = config;
    }

    /// Get configuration
    pub fn config(&self) -> &OomConfig {
        &self.config
    }

    /// Set process info provider
    pub fn set_process_info_provider(&mut self, f: ProcessInfoProvider) {
        self.process_info_fn = Some(f);
    }

    /// Set memory info provider
    pub fn set_memory_info_provider(&mut self, f: MemoryInfoProvider) {
        self.memory_info_fn = Some(f);
    }

    /// Set kill function
    pub fn set_kill_fn(&mut self, f: ProcessKillFn) {
        self.kill_fn = Some(f);
    }

    /// Set OOM adjustment for a process
    pub fn set_oom_adj(&mut self, pid: Pid, adj: OomAdjustment) {
        if adj == -1000 {
            self.protected.push(pid);
        }
        self.adjustments.insert(pid, adj.clamp(-1000, 1000));
    }

    /// Get OOM adjustment for a process
    pub fn get_oom_adj(&self, pid: Pid) -> OomAdjustment {
        self.adjustments.get(&pid).copied().unwrap_or(0)
    }

    /// Set OOM policy for a process
    pub fn set_policy(&mut self, pid: Pid, policy: OomPolicy) {
        if policy == OomPolicy::NeverKill {
            self.protected.push(pid);
        }
        self.policies.insert(pid, policy);
    }

    /// Get OOM policy for a process
    pub fn get_policy(&self, pid: Pid) -> OomPolicy {
        self.policies.get(&pid).copied().unwrap_or(OomPolicy::Normal)
    }

    /// Add protected process
    pub fn protect(&mut self, pid: Pid) {
        if !self.protected.contains(&pid) {
            self.protected.push(pid);
        }
        self.policies.insert(pid, OomPolicy::Protected);
    }

    /// Remove protection from process
    pub fn unprotect(&mut self, pid: Pid) {
        self.protected.retain(|&p| p != pid);
        self.policies.remove(&pid);
    }

    /// Check current memory pressure
    pub fn check_pressure(&mut self) -> MemoryPressure {
        let (total, available) = match self.memory_info_fn {
            Some(f) => f(),
            None => return MemoryPressure::None,
        };

        if total == 0 {
            return MemoryPressure::None;
        }

        let available_pct = ((available * 100) / total) as u8;

        self.pressure = if available_pct <= self.config.critical_threshold {
            MemoryPressure::Critical
        } else if available_pct <= self.config.low_memory_threshold {
            MemoryPressure::Low
        } else {
            MemoryPressure::None
        };

        self.pressure
    }

    /// Get current memory pressure
    pub fn pressure(&self) -> MemoryPressure {
        self.pressure
    }

    /// Get statistics
    pub fn stats(&self) -> &OomStats {
        &self.stats
    }

    /// Get notifications
    pub fn notifications(&self) -> &[OomNotification] {
        &self.notifications
    }

    /// Clear notifications
    pub fn clear_notifications(&mut self) {
        self.notifications.clear();
    }

    /// Add notification
    fn notify(&mut self, kind: OomNotificationKind, message: String) {
        let ts = crate::time::realtime().tv_sec as u64;
        self.notifications.push(OomNotification {
            kind,
            timestamp: ts,
            message,
        });

        // Keep only last 100 notifications
        if self.notifications.len() > 100 {
            self.notifications.remove(0);
        }
    }

    /// Select process to kill
    pub fn select_victim(&self) -> Option<(Pid, OomScore)> {
        let process_info = self.process_info_fn?;
        let memory_info = self.memory_info_fn?;

        let processes = process_info();
        let (total_memory, _) = memory_info();

        if processes.is_empty() {
            return None;
        }

        // Calculate scores for all processes
        let mut candidates: Vec<(Pid, OomScore, String)> = processes
            .iter()
            .filter(|p| !p.is_kernel)
            .filter(|p| !self.protected.contains(&p.pid))
            .filter(|p| {
                let policy = self.policies.get(&p.pid).copied().unwrap_or(p.policy);
                policy != OomPolicy::NeverKill
            })
            .map(|p| {
                let mut info = p.clone();
                // Apply stored adjustment
                if let Some(&adj) = self.adjustments.get(&p.pid) {
                    info.oom_adj = adj;
                }
                // Apply stored policy
                if let Some(&policy) = self.policies.get(&p.pid) {
                    info.policy = policy;
                }
                let score = info.calculate_score(total_memory);
                (p.pid, score, p.name.clone())
            })
            .filter(|(_, score, _)| *score >= self.config.min_score_threshold)
            .take(self.config.max_candidates)
            .collect();

        // Sort by score descending (highest score = kill first)
        candidates.sort_by(|a, b| b.1.cmp(&a.1));

        candidates.first().map(|(pid, score, _)| (*pid, *score))
    }

    /// Try to reclaim memory before killing
    fn try_reclaim(&mut self) -> bool {
        // Try various memory reclaim strategies
        // This would call into the MM subsystem to:
        // 1. Shrink page cache
        // 2. Shrink slab caches
        // 3. Compact memory
        // 4. Drop caches

        // For now, just report if we'd try
        if self.config.try_reclaim_first {
            // Signal MM to try reclaiming
            // mm::try_reclaim_memory()
            self.stats.reclaim_success += 1;
            false // For now, always proceed to kill
        } else {
            false
        }
    }

    /// Kill a process
    fn do_kill(&mut self, pid: Pid) -> bool {
        match self.kill_fn {
            Some(f) => f(pid),
            None => false,
        }
    }

    /// Handle OOM situation
    pub fn handle_oom(&mut self) -> Option<OomKillResult> {
        if !self.config.enabled {
            return None;
        }

        self.stats.oom_count += 1;
        self.pressure = MemoryPressure::OomTriggered;

        // Try to reclaim first
        if self.try_reclaim() {
            self.notify(
                OomNotificationKind::LowMemory,
                "Memory reclaimed successfully".to_string(),
            );
            return None;
        }

        // Check if we should panic instead
        if self.config.panic_on_oom {
            self.notify(
                OomNotificationKind::OomPanic,
                "Out of memory - system panic".to_string(),
            );
            panic!("Out of memory");
        }

        // Select victim
        let (victim_pid, score) = self.select_victim()?;

        // Get process info for logging
        let process_info = self.process_info_fn?;
        let processes = process_info();
        let victim = processes.iter().find(|p| p.pid == victim_pid)?;

        let memory_to_free = victim.memory.rss;
        let name = victim.name.clone();
        let children = victim.children.clone();

        // Kill the process
        if !self.do_kill(victim_pid) {
            crate::util::kprintln!("OOM: Failed to kill process {} ({})", victim_pid, name);
            return None;
        }

        // Kill children if configured
        if self.config.kill_children {
            for child in &children {
                let _ = self.do_kill(*child);
            }
        }

        // Update stats
        self.stats.total_kills += 1;
        self.stats.total_freed += memory_to_free;
        self.stats.last_kill_time = crate::time::realtime().tv_sec as u64;

        let result = OomKillResult {
            pid: victim_pid,
            name: name.clone(),
            memory_freed: memory_to_free,
            score,
            reason: alloc::format!(
                "Highest OOM score ({}) among {} candidates",
                score,
                processes.len()
            ),
        };

        // Log
        crate::util::kprintln!(
            "OOM: Killed process {} ({}) score={} freed ~{} KB",
            victim_pid,
            name,
            score,
            memory_to_free / 1024
        );

        // Notify
        if self.config.notify_on_kill {
            self.notify(
                OomNotificationKind::ProcessKilled,
                alloc::format!(
                    "Killed process {} ({}) to free {} KB",
                    victim_pid,
                    name,
                    memory_to_free / 1024
                ),
            );
        }

        Some(result)
    }

    /// Process cleanup (called when a process exits)
    pub fn process_exit(&mut self, pid: Pid) {
        self.adjustments.remove(&pid);
        self.policies.remove(&pid);
        self.protected.retain(|&p| p != pid);
    }
}

/// Global OOM killer instance
static OOM_KILLER: IrqSafeMutex<OomKiller> = IrqSafeMutex::new(OomKiller::new());

/// OOM killer enabled flag
static OOM_ENABLED: AtomicBool = AtomicBool::new(true);

/// Last OOM timestamp
static LAST_OOM: AtomicU64 = AtomicU64::new(0);

/// Initialize OOM killer
pub fn init() {
    crate::util::kprintln!("oom: initializing OOM killer...");
    OOM_ENABLED.store(true, Ordering::Release);
}

/// Check memory pressure and potentially trigger OOM
pub fn check_and_handle() -> Option<OomKillResult> {
    if !OOM_ENABLED.load(Ordering::Acquire) {
        return None;
    }

    let mut killer = OOM_KILLER.lock();
    let pressure = killer.check_pressure();

    match pressure {
        MemoryPressure::Critical | MemoryPressure::OomTriggered => {
            killer.handle_oom()
        }
        MemoryPressure::Low => {
            // Try memory reclaim without killing
            killer.try_reclaim();
            None
        }
        MemoryPressure::None => None,
    }
}

/// Explicitly trigger OOM handler (called from allocator on failure)
pub fn trigger() -> Option<OomKillResult> {
    if !OOM_ENABLED.load(Ordering::Acquire) {
        return None;
    }

    LAST_OOM.store(crate::time::realtime().tv_sec as u64, Ordering::Release);
    OOM_KILLER.lock().handle_oom()
}

/// Set OOM adjustment for a process
pub fn set_oom_adj(pid: Pid, adj: OomAdjustment) {
    OOM_KILLER.lock().set_oom_adj(pid, adj);
}

/// Get OOM adjustment for a process
pub fn get_oom_adj(pid: Pid) -> OomAdjustment {
    OOM_KILLER.lock().get_oom_adj(pid)
}

/// Set OOM policy for a process
pub fn set_policy(pid: Pid, policy: OomPolicy) {
    OOM_KILLER.lock().set_policy(pid, policy);
}

/// Protect a process from OOM killer
pub fn protect(pid: Pid) {
    OOM_KILLER.lock().protect(pid);
}

/// Remove OOM protection from a process
pub fn unprotect(pid: Pid) {
    OOM_KILLER.lock().unprotect(pid);
}

/// Get OOM statistics
pub fn stats() -> OomStats {
    OOM_KILLER.lock().stats().clone()
}

/// Get current memory pressure
pub fn pressure() -> MemoryPressure {
    OOM_KILLER.lock().pressure()
}

/// Configure OOM killer
pub fn configure(config: OomConfig) {
    OOM_KILLER.lock().set_config(config);
}

/// Enable/disable OOM killer
pub fn set_enabled(enabled: bool) {
    OOM_ENABLED.store(enabled, Ordering::Release);
}

/// Check if OOM killer is enabled
pub fn is_enabled() -> bool {
    OOM_ENABLED.load(Ordering::Acquire)
}

/// Process exit notification
pub fn process_exit(pid: Pid) {
    OOM_KILLER.lock().process_exit(pid);
}

/// Set process info provider
pub fn set_process_info_provider(f: ProcessInfoProvider) {
    OOM_KILLER.lock().set_process_info_provider(f);
}

/// Set memory info provider
pub fn set_memory_info_provider(f: MemoryInfoProvider) {
    OOM_KILLER.lock().set_memory_info_provider(f);
}

/// Set kill function
pub fn set_kill_fn(f: ProcessKillFn) {
    OOM_KILLER.lock().set_kill_fn(f);
}

/// Get last OOM timestamp
pub fn last_oom_time() -> u64 {
    LAST_OOM.load(Ordering::Acquire)
}
