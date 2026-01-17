//! CFS-like (Completely Fair Scheduler) implementation.
//!
//! This module implements a scheduler inspired by Linux's CFS:
//! - Virtual runtime (vruntime) tracks "fair" CPU time
//! - Tasks with lower vruntime get scheduled first
//! - Nice values (-20 to +19) affect weight and vruntime accumulation rate
//! - Uses a sorted list (simulating red-black tree) for O(log n) operations

extern crate alloc;

use alloc::collections::BinaryHeap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

/// Weight table from Linux kernel (sched_prio_to_weight)
/// Index 0 = nice -20, Index 40 = nice +20
/// Nice 0 has weight 1024 (NICE_0_LOAD)
const SCHED_PRIO_TO_WEIGHT: [u32; 40] = [
    /* -20 */ 88761, 71755, 56483, 46273, 36291,
    /* -15 */ 29154, 23254, 18705, 14949, 11916,
    /* -10 */ 9548, 7620, 6100, 4904, 3906,
    /*  -5 */ 3121, 2501, 1991, 1586, 1277,
    /*   0 */ 1024, 820, 655, 526, 423,
    /*   5 */ 335, 272, 215, 172, 137,
    /*  10 */ 110, 87, 70, 56, 45,
    /*  15 */ 36, 29, 23, 18, 15,
];

/// Inverse of weights for fast division (wmult values from Linux)
/// Multiplied by 2^32, so we can do (vruntime_delta * wmult) >> 32
const SCHED_PRIO_TO_WMULT: [u64; 40] = [
    /* -20 */ 48388, 59856, 76040, 92818, 118348,
    /* -15 */ 147320, 184698, 229616, 287308, 360437,
    /* -10 */ 449829, 563644, 704093, 875809, 1099582,
    /*  -5 */ 1376151, 1717300, 2157191, 2708050, 3363326,
    /*   0 */ 4194304, 5237765, 6557202, 8165337, 10153587,
    /*   5 */ 12820798, 15790321, 19976592, 24970740, 31350126,
    /*  10 */ 39045157, 49367440, 61356676, 76695844, 95443717,
    /*  15 */ 119304647, 148102320, 186737708, 238609294, 286331153,
];

/// Nice 0 weight (base weight)
pub const NICE_0_LOAD: u32 = 1024;

/// Minimum granularity in nanoseconds (1ms = 1_000_000 ns)
/// Tasks run at least this long before preemption
pub const SCHED_MIN_GRANULARITY_NS: u64 = 1_000_000;

/// Target latency in nanoseconds (6ms)
/// All runnable tasks should run once within this period
pub const SCHED_LATENCY_NS: u64 = 6_000_000;

/// Number of ticks per millisecond (assuming ~1000 Hz timer)
const TICKS_PER_MS: u64 = 1;

/// Convert nice value (-20 to +19) to weight index (0 to 39)
#[inline]
fn nice_to_index(nice: i8) -> usize {
    ((nice as i32 + 20) as usize).clamp(0, 39)
}

/// Get weight for a nice value
pub fn weight_from_nice(nice: i8) -> u32 {
    SCHED_PRIO_TO_WEIGHT[nice_to_index(nice)]
}

/// Get inverse weight multiplier for a nice value
pub fn wmult_from_nice(nice: i8) -> u64 {
    SCHED_PRIO_TO_WMULT[nice_to_index(nice)]
}

/// CFS task entity - stores per-task CFS scheduling data
#[derive(Debug)]
pub struct SchedEntity {
    /// Virtual runtime in nanoseconds
    /// Lower vruntime = higher scheduling priority
    vruntime: AtomicU64,
    /// Task weight based on nice value
    weight: u32,
    /// Inverse weight for fast calculation
    inv_weight: u64,
    /// Last update time (for calculating delta)
    exec_start: AtomicU64,
    /// Total runtime in nanoseconds
    sum_exec_runtime: AtomicU64,
    /// Time slice in ticks
    time_slice: AtomicU64,
}

impl SchedEntity {
    /// Create a new scheduling entity with given nice value
    pub fn new(nice: i8) -> Self {
        let weight = weight_from_nice(nice);
        let inv_weight = wmult_from_nice(nice);
        Self {
            vruntime: AtomicU64::new(0),
            weight,
            inv_weight,
            exec_start: AtomicU64::new(0),
            sum_exec_runtime: AtomicU64::new(0),
            time_slice: AtomicU64::new(0),
        }
    }

    /// Update weight when nice value changes
    pub fn update_weight(&mut self, nice: i8) {
        self.weight = weight_from_nice(nice);
        self.inv_weight = wmult_from_nice(nice);
    }

    /// Get current vruntime
    pub fn vruntime(&self) -> u64 {
        self.vruntime.load(AtomicOrdering::Relaxed)
    }

    /// Set vruntime (used when placing back on runqueue)
    pub fn set_vruntime(&self, vrt: u64) {
        self.vruntime.store(vrt, AtomicOrdering::Relaxed);
    }

    /// Get weight
    pub fn weight(&self) -> u32 {
        self.weight
    }

    /// Get time slice in ticks
    pub fn time_slice(&self) -> u64 {
        self.time_slice.load(AtomicOrdering::Relaxed)
    }

    /// Set time slice
    pub fn set_time_slice(&self, slice: u64) {
        self.time_slice.store(slice, AtomicOrdering::Relaxed);
    }

    /// Get sum of execution runtime
    pub fn sum_exec_runtime(&self) -> u64 {
        self.sum_exec_runtime.load(AtomicOrdering::Relaxed)
    }
}

/// CFS run queue
pub struct CfsRunqueue {
    /// Current time in nanoseconds (monotonic)
    clock: u64,
    /// Minimum vruntime on the queue (for placing new tasks)
    min_vruntime: u64,
    /// Total weight of all runnable tasks
    load_weight: u64,
    /// Number of runnable tasks
    nr_running: usize,
    /// Tasks stored as (vruntime, task_id) for sorting
    /// Using BinaryHeap with reverse ordering for min-heap behavior
    tasks: Vec<CfsEntry>,
}

/// Entry in the CFS run queue
#[derive(Clone)]
pub struct CfsEntry {
    pub task_id: u64,
    pub vruntime: u64,
    pub weight: u32,
}

impl PartialEq for CfsEntry {
    fn eq(&self, other: &Self) -> bool {
        self.vruntime == other.vruntime && self.task_id == other.task_id
    }
}

impl Eq for CfsEntry {}

impl PartialOrd for CfsEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CfsEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for min-heap (smallest vruntime first)
        match other.vruntime.cmp(&self.vruntime) {
            Ordering::Equal => other.task_id.cmp(&self.task_id),
            ord => ord,
        }
    }
}

impl CfsRunqueue {
    /// Create a new CFS runqueue
    pub const fn new() -> Self {
        Self {
            clock: 0,
            min_vruntime: 0,
            load_weight: 0,
            nr_running: 0,
            tasks: Vec::new(),
        }
    }

    /// Update the runqueue clock
    pub fn update_clock(&mut self, delta_ns: u64) {
        self.clock = self.clock.saturating_add(delta_ns);
    }

    /// Get current clock
    pub fn clock(&self) -> u64 {
        self.clock
    }

    /// Get minimum vruntime
    pub fn min_vruntime(&self) -> u64 {
        self.min_vruntime
    }

    /// Get number of runnable tasks
    pub fn nr_running(&self) -> usize {
        self.nr_running
    }

    /// Get total load weight
    pub fn load_weight(&self) -> u64 {
        self.load_weight
    }

    /// Calculate time slice for a task based on its weight and total load
    /// slice = (weight / total_weight) * sched_latency
    pub fn calc_time_slice(&self, weight: u32) -> u64 {
        if self.load_weight == 0 || self.nr_running == 0 {
            return SCHED_LATENCY_NS / TICKS_PER_MS;
        }

        // Ensure minimum granularity
        let period = if self.nr_running > (SCHED_LATENCY_NS / SCHED_MIN_GRANULARITY_NS) as usize {
            self.nr_running as u64 * SCHED_MIN_GRANULARITY_NS
        } else {
            SCHED_LATENCY_NS
        };

        // slice = weight * period / total_weight
        let slice_ns = (weight as u64 * period) / self.load_weight;
        let slice_ns = slice_ns.max(SCHED_MIN_GRANULARITY_NS);

        // Convert to ticks (assuming 1 tick ≈ 1ms)
        (slice_ns / 1_000_000).max(1)
    }

    /// Calculate delta vruntime for a given delta execution time
    /// vruntime_delta = delta_exec * NICE_0_LOAD / weight
    pub fn calc_delta_vruntime(delta_exec_ns: u64, weight: u32, inv_weight: u64) -> u64 {
        if weight >= NICE_0_LOAD {
            // For high-priority tasks (weight >= 1024), vruntime grows slower
            // Use inverse weight multiplication for precision
            ((delta_exec_ns as u128 * inv_weight as u128) >> 32) as u64
        } else {
            // For low-priority tasks, vruntime grows faster
            (delta_exec_ns * NICE_0_LOAD as u64) / weight as u64
        }
    }

    /// Enqueue a task
    pub fn enqueue(&mut self, entry: CfsEntry) {
        self.load_weight += entry.weight as u64;
        self.nr_running += 1;
        self.tasks.push(entry);
        // Keep sorted by vruntime (ascending)
        self.tasks.sort_by(|a, b| a.vruntime.cmp(&b.vruntime));
    }

    /// Dequeue the task with smallest vruntime
    pub fn dequeue_next(&mut self) -> Option<CfsEntry> {
        if self.tasks.is_empty() {
            return None;
        }

        let entry = self.tasks.remove(0);
        self.load_weight = self.load_weight.saturating_sub(entry.weight as u64);
        self.nr_running = self.nr_running.saturating_sub(1);

        // Update min_vruntime
        if let Some(first) = self.tasks.first() {
            self.min_vruntime = self.min_vruntime.max(first.vruntime);
        }

        Some(entry)
    }

    /// Remove a specific task by ID
    pub fn remove(&mut self, task_id: u64) -> Option<CfsEntry> {
        if let Some(idx) = self.tasks.iter().position(|e| e.task_id == task_id) {
            let entry = self.tasks.remove(idx);
            self.load_weight = self.load_weight.saturating_sub(entry.weight as u64);
            self.nr_running = self.nr_running.saturating_sub(1);
            Some(entry)
        } else {
            None
        }
    }

    /// Update min_vruntime to ensure new tasks don't starve existing ones
    pub fn update_min_vruntime(&mut self) {
        let mut vruntime = self.min_vruntime;

        if let Some(first) = self.tasks.first() {
            vruntime = vruntime.max(first.vruntime);
        }

        // Only increase min_vruntime, never decrease
        self.min_vruntime = self.min_vruntime.max(vruntime);
    }

    /// Place a new or waking task, setting appropriate vruntime
    pub fn place_entity(&self, entity: &SchedEntity, initial: bool) -> u64 {
        let mut vruntime = self.min_vruntime;

        if initial {
            // New task: start at min_vruntime + half a latency period
            // This prevents new tasks from immediately preempting running tasks
            let thresh = SCHED_LATENCY_NS / 2;
            vruntime = vruntime.saturating_add(thresh);
        }

        vruntime
    }

    /// Check if preemption is needed (current task ran its slice)
    pub fn check_preempt_tick(&self, current_vruntime: u64, current_slice: u64, ticks_run: u64) -> bool {
        // Check if time slice expired
        if ticks_run >= current_slice {
            return true;
        }

        // Check if another task has lower vruntime
        if let Some(first) = self.tasks.first() {
            // Add some threshold to prevent too frequent switches
            let ideal_runtime_ns = SCHED_MIN_GRANULARITY_NS;
            if first.vruntime + ideal_runtime_ns < current_vruntime {
                return true;
            }
        }

        false
    }

    /// Get task with smallest vruntime (peek)
    pub fn peek_next(&self) -> Option<&CfsEntry> {
        self.tasks.first()
    }
}

/// CFS statistics for debugging/monitoring
#[derive(Debug, Default)]
pub struct CfsStats {
    pub nr_switches: u64,
    pub nr_preemptions: u64,
    pub nr_voluntary_switches: u64,
    pub total_wait_time_ns: u64,
    pub total_run_time_ns: u64,
}

impl CfsStats {
    pub fn record_switch(&mut self, preempted: bool) {
        self.nr_switches += 1;
        if preempted {
            self.nr_preemptions += 1;
        } else {
            self.nr_voluntary_switches += 1;
        }
    }
}

/// Global CFS statistics
static mut CFS_STATS: CfsStats = CfsStats {
    nr_switches: 0,
    nr_preemptions: 0,
    nr_voluntary_switches: 0,
    total_wait_time_ns: 0,
    total_run_time_ns: 0,
};

/// Get CFS statistics
pub fn get_cfs_stats() -> CfsStats {
    unsafe {
        CfsStats {
            nr_switches: CFS_STATS.nr_switches,
            nr_preemptions: CFS_STATS.nr_preemptions,
            nr_voluntary_switches: CFS_STATS.nr_voluntary_switches,
            total_wait_time_ns: CFS_STATS.total_wait_time_ns,
            total_run_time_ns: CFS_STATS.total_run_time_ns,
        }
    }
}

/// Record a context switch
pub fn record_switch(preempted: bool) {
    unsafe {
        CFS_STATS.record_switch(preempted);
    }
}

/// Record runtime
pub fn record_runtime(ns: u64) {
    unsafe {
        CFS_STATS.total_run_time_ns += ns;
    }
}

/// Record wait time
pub fn record_wait_time(ns: u64) {
    unsafe {
        CFS_STATS.total_wait_time_ns += ns;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weight_from_nice() {
        assert_eq!(weight_from_nice(0), 1024);  // NICE_0_LOAD
        assert_eq!(weight_from_nice(-20), 88761);  // Highest priority
        assert_eq!(weight_from_nice(19), 15);  // Lowest priority
    }

    #[test]
    fn test_nice_to_index() {
        assert_eq!(nice_to_index(-20), 0);
        assert_eq!(nice_to_index(0), 20);
        assert_eq!(nice_to_index(19), 39);
    }
}
