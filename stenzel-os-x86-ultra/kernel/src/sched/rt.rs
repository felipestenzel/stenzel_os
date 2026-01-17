//! Real-Time Scheduling Support
//!
//! Implements POSIX real-time scheduling policies:
//! - SCHED_FIFO: First-In-First-Out (runs until yields or blocks)
//! - SCHED_RR: Round-Robin (time-sliced real-time)
//!
//! Real-time priorities range from 1 (lowest) to 99 (highest).
//! RT tasks always preempt non-RT (SCHED_OTHER) tasks.
//!
//! References:
//! - POSIX.1b real-time extensions
//! - Linux sched(7) man page
//! - https://man7.org/linux/man-pages/man7/sched.7.html

#![allow(dead_code)]

extern crate alloc;

use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, AtomicU32, Ordering};

// ============================================================================
// Constants
// ============================================================================

/// Minimum real-time priority
pub const RT_PRIO_MIN: u8 = 1;

/// Maximum real-time priority
pub const RT_PRIO_MAX: u8 = 99;

/// Default RR time slice in ticks (100ms at 100Hz)
pub const SCHED_RR_TIMESLICE: u64 = 10;

/// Maximum RR time slice in ticks
pub const SCHED_RR_MAX_TIMESLICE: u64 = 100;

// ============================================================================
// Scheduling Policies
// ============================================================================

/// Scheduling policy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SchedPolicy {
    /// Normal time-sharing (CFS-like)
    Other = 0,
    /// Real-time First-In-First-Out
    Fifo = 1,
    /// Real-time Round-Robin
    RoundRobin = 2,
    /// Batch processing (lower priority than Other)
    Batch = 3,
    /// Idle priority (only runs when nothing else)
    Idle = 5,
    /// Deadline scheduling (not yet implemented)
    Deadline = 6,
}

impl SchedPolicy {
    /// Create from policy number
    pub fn from_u32(policy: u32) -> Option<Self> {
        match policy {
            0 => Some(SchedPolicy::Other),
            1 => Some(SchedPolicy::Fifo),
            2 => Some(SchedPolicy::RoundRobin),
            3 => Some(SchedPolicy::Batch),
            5 => Some(SchedPolicy::Idle),
            6 => Some(SchedPolicy::Deadline),
            _ => None,
        }
    }

    /// Check if this is a real-time policy
    pub fn is_realtime(&self) -> bool {
        matches!(self, SchedPolicy::Fifo | SchedPolicy::RoundRobin)
    }

    /// Check if this is a normal (non-RT) policy
    pub fn is_normal(&self) -> bool {
        matches!(self, SchedPolicy::Other | SchedPolicy::Batch | SchedPolicy::Idle)
    }

    /// Get minimum valid priority for this policy
    pub fn min_priority(&self) -> u8 {
        match self {
            SchedPolicy::Fifo | SchedPolicy::RoundRobin => RT_PRIO_MIN,
            _ => 0,
        }
    }

    /// Get maximum valid priority for this policy
    pub fn max_priority(&self) -> u8 {
        match self {
            SchedPolicy::Fifo | SchedPolicy::RoundRobin => RT_PRIO_MAX,
            _ => 0,
        }
    }
}

impl Default for SchedPolicy {
    fn default() -> Self {
        SchedPolicy::Other
    }
}

// ============================================================================
// Scheduling Parameters
// ============================================================================

/// Scheduling parameters (sched_param structure)
#[derive(Debug, Clone, Copy)]
pub struct SchedParam {
    /// Scheduling priority (1-99 for RT, ignored for other policies)
    pub sched_priority: i32,
}

impl Default for SchedParam {
    fn default() -> Self {
        SchedParam { sched_priority: 0 }
    }
}

/// Extended scheduling attributes (for sched_setattr)
#[derive(Debug, Clone, Copy)]
pub struct SchedAttr {
    /// Size of this structure
    pub size: u32,
    /// Scheduling policy
    pub sched_policy: SchedPolicy,
    /// Flags
    pub sched_flags: u64,
    /// Nice value (for SCHED_OTHER/BATCH)
    pub sched_nice: i32,
    /// Real-time priority (for SCHED_FIFO/RR)
    pub sched_priority: u32,
    /// Deadline period (for SCHED_DEADLINE)
    pub sched_runtime: u64,
    pub sched_deadline: u64,
    pub sched_period: u64,
}

impl Default for SchedAttr {
    fn default() -> Self {
        SchedAttr {
            size: core::mem::size_of::<Self>() as u32,
            sched_policy: SchedPolicy::Other,
            sched_flags: 0,
            sched_nice: 0,
            sched_priority: 0,
            sched_runtime: 0,
            sched_deadline: 0,
            sched_period: 0,
        }
    }
}

// ============================================================================
// Real-Time Runqueue
// ============================================================================

/// Entry in the RT runqueue
#[derive(Debug, Clone)]
pub struct RtEntry {
    /// Task ID
    pub task_id: u64,
    /// Real-time priority (1-99)
    pub priority: u8,
    /// Scheduling policy (FIFO or RR)
    pub policy: SchedPolicy,
    /// Remaining time slice (for RR)
    pub time_slice: u64,
}

/// Real-time runqueue
///
/// Organized by priority levels (1-99).
/// Higher priority tasks are scheduled first.
pub struct RtRunqueue {
    /// Priority queues (index 0 = priority 1, index 98 = priority 99)
    queues: [VecDeque<RtEntry>; 99],
    /// Number of runnable RT tasks
    nr_running: AtomicU32,
    /// Bitmap of active priorities (for fast lookup)
    active_bitmap: [AtomicU64; 2], // 128 bits, use lower 99
    /// Current highest priority with runnable task
    highest_prio: AtomicU32,
    /// Statistics
    stats: RtStats,
}

/// Real-time scheduler statistics
#[derive(Debug, Default)]
pub struct RtStats {
    /// Total context switches for RT tasks
    pub context_switches: AtomicU64,
    /// Number of FIFO tasks scheduled
    pub fifo_scheduled: AtomicU64,
    /// Number of RR tasks scheduled
    pub rr_scheduled: AtomicU64,
    /// Number of RR time slice expirations
    pub rr_timeslice_expired: AtomicU64,
    /// Number of RT preemptions
    pub rt_preemptions: AtomicU64,
}

impl RtRunqueue {
    /// Create a new RT runqueue
    pub const fn new() -> Self {
        const EMPTY_DEQUE: VecDeque<RtEntry> = VecDeque::new();
        RtRunqueue {
            queues: [EMPTY_DEQUE; 99],
            nr_running: AtomicU32::new(0),
            active_bitmap: [AtomicU64::new(0), AtomicU64::new(0)],
            highest_prio: AtomicU32::new(0),
            stats: RtStats {
                context_switches: AtomicU64::new(0),
                fifo_scheduled: AtomicU64::new(0),
                rr_scheduled: AtomicU64::new(0),
                rr_timeslice_expired: AtomicU64::new(0),
                rt_preemptions: AtomicU64::new(0),
            },
        }
    }

    /// Check if there are any runnable RT tasks
    pub fn is_empty(&self) -> bool {
        self.nr_running.load(Ordering::Relaxed) == 0
    }

    /// Get number of runnable RT tasks
    pub fn len(&self) -> usize {
        self.nr_running.load(Ordering::Relaxed) as usize
    }

    /// Add a task to the RT runqueue
    pub fn enqueue(&mut self, entry: RtEntry) {
        let prio = entry.priority.clamp(RT_PRIO_MIN, RT_PRIO_MAX);
        let idx = (prio - 1) as usize;

        self.queues[idx].push_back(entry);
        self.nr_running.fetch_add(1, Ordering::Relaxed);

        // Update bitmap
        self.set_bitmap_bit(prio);

        // Update highest priority
        let current_highest = self.highest_prio.load(Ordering::Relaxed) as u8;
        if prio > current_highest {
            self.highest_prio.store(prio as u32, Ordering::Relaxed);
        }
    }

    /// Remove a task from the RT runqueue
    pub fn dequeue(&mut self, task_id: u64) -> Option<RtEntry> {
        // Search all priority queues
        for prio in (RT_PRIO_MIN..=RT_PRIO_MAX).rev() {
            let idx = (prio - 1) as usize;
            if let Some(pos) = self.queues[idx].iter().position(|e| e.task_id == task_id) {
                let entry = self.queues[idx].remove(pos)?;
                self.nr_running.fetch_sub(1, Ordering::Relaxed);

                // Update bitmap if queue is now empty
                if self.queues[idx].is_empty() {
                    self.clear_bitmap_bit(prio);
                }

                // Recalculate highest priority
                self.recalc_highest_prio();

                return Some(entry);
            }
        }
        None
    }

    /// Pick the next task to run (highest priority, FIFO within priority)
    pub fn pick_next(&mut self) -> Option<RtEntry> {
        let highest = self.highest_prio.load(Ordering::Relaxed) as u8;
        if highest == 0 {
            return None;
        }

        let idx = (highest - 1) as usize;
        let entry = self.queues[idx].pop_front()?;

        self.nr_running.fetch_sub(1, Ordering::Relaxed);

        // Update stats
        match entry.policy {
            SchedPolicy::Fifo => {
                self.stats.fifo_scheduled.fetch_add(1, Ordering::Relaxed);
            }
            SchedPolicy::RoundRobin => {
                self.stats.rr_scheduled.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
        self.stats.context_switches.fetch_add(1, Ordering::Relaxed);

        // Update bitmap if queue is now empty
        if self.queues[idx].is_empty() {
            self.clear_bitmap_bit(highest);
            self.recalc_highest_prio();
        }

        Some(entry)
    }

    /// Peek at the next task without removing it
    pub fn peek_next(&self) -> Option<&RtEntry> {
        let highest = self.highest_prio.load(Ordering::Relaxed) as u8;
        if highest == 0 {
            return None;
        }

        let idx = (highest - 1) as usize;
        self.queues[idx].front()
    }

    /// Move a RR task to the back of its priority queue (time slice expired)
    pub fn requeue_rr(&mut self, entry: RtEntry) {
        let prio = entry.priority.clamp(RT_PRIO_MIN, RT_PRIO_MAX);
        let idx = (prio - 1) as usize;

        // Reset time slice
        let mut new_entry = entry;
        new_entry.time_slice = SCHED_RR_TIMESLICE;

        self.queues[idx].push_back(new_entry);
        self.nr_running.fetch_add(1, Ordering::Relaxed);
        self.stats.rr_timeslice_expired.fetch_add(1, Ordering::Relaxed);

        // Bitmap should already be set since task was running at this priority
        self.set_bitmap_bit(prio);

        // Update highest priority
        let current_highest = self.highest_prio.load(Ordering::Relaxed) as u8;
        if prio > current_highest {
            self.highest_prio.store(prio as u32, Ordering::Relaxed);
        }
    }

    /// Update a task's priority (move between queues)
    pub fn update_priority(&mut self, task_id: u64, new_priority: u8) -> bool {
        if let Some(mut entry) = self.dequeue(task_id) {
            entry.priority = new_priority.clamp(RT_PRIO_MIN, RT_PRIO_MAX);
            self.enqueue(entry);
            true
        } else {
            false
        }
    }

    /// Get task entry by ID
    pub fn get_entry(&self, task_id: u64) -> Option<&RtEntry> {
        for prio in (RT_PRIO_MIN..=RT_PRIO_MAX).rev() {
            let idx = (prio - 1) as usize;
            if let Some(entry) = self.queues[idx].iter().find(|e| e.task_id == task_id) {
                return Some(entry);
            }
        }
        None
    }

    /// Check if a higher priority task is runnable (for preemption check)
    pub fn should_preempt(&self, current_priority: u8) -> bool {
        let highest = self.highest_prio.load(Ordering::Relaxed) as u8;
        highest > current_priority
    }

    /// Get statistics
    pub fn stats(&self) -> &RtStats {
        &self.stats
    }

    // Helper: Set a bit in the active bitmap
    fn set_bitmap_bit(&self, prio: u8) {
        let idx = if prio <= 64 { 0 } else { 1 };
        let bit = if prio <= 64 { prio - 1 } else { prio - 65 };
        self.active_bitmap[idx].fetch_or(1 << bit, Ordering::Relaxed);
    }

    // Helper: Clear a bit in the active bitmap
    fn clear_bitmap_bit(&self, prio: u8) {
        let idx = if prio <= 64 { 0 } else { 1 };
        let bit = if prio <= 64 { prio - 1 } else { prio - 65 };
        self.active_bitmap[idx].fetch_and(!(1 << bit), Ordering::Relaxed);
    }

    // Helper: Recalculate highest priority from bitmap
    fn recalc_highest_prio(&self) {
        // Check upper half first (priorities 65-99)
        let upper = self.active_bitmap[1].load(Ordering::Relaxed);
        if upper != 0 {
            let highest_in_upper = 64 - upper.leading_zeros() as u8; // 0-34
            self.highest_prio.store((65 + highest_in_upper) as u32, Ordering::Relaxed);
            return;
        }

        // Check lower half (priorities 1-64)
        let lower = self.active_bitmap[0].load(Ordering::Relaxed);
        if lower != 0 {
            let highest_in_lower = 64 - lower.leading_zeros() as u8; // 0-63
            self.highest_prio.store((1 + highest_in_lower) as u32, Ordering::Relaxed);
            return;
        }

        self.highest_prio.store(0, Ordering::Relaxed);
    }
}

// ============================================================================
// Real-Time Entity (per-task state)
// ============================================================================

/// Per-task real-time scheduling state
#[derive(Debug)]
pub struct RtEntity {
    /// Scheduling policy
    policy: SchedPolicy,
    /// Real-time priority (1-99 for RT, 0 for non-RT)
    priority: AtomicU32,
    /// Remaining time slice for RR
    time_slice: AtomicU64,
    /// Whether task is on RT runqueue
    on_rq: core::sync::atomic::AtomicBool,
    /// Run count
    run_cnt: AtomicU64,
}

impl RtEntity {
    /// Create a new RT entity with default (non-RT) scheduling
    pub fn new() -> Self {
        RtEntity {
            policy: SchedPolicy::Other,
            priority: AtomicU32::new(0),
            time_slice: AtomicU64::new(0),
            on_rq: core::sync::atomic::AtomicBool::new(false),
            run_cnt: AtomicU64::new(0),
        }
    }

    /// Create a new RT entity with specified policy and priority
    pub fn with_policy(policy: SchedPolicy, priority: u8) -> Self {
        let prio = if policy.is_realtime() {
            priority.clamp(RT_PRIO_MIN, RT_PRIO_MAX) as u32
        } else {
            0
        };

        let time_slice = if policy == SchedPolicy::RoundRobin {
            SCHED_RR_TIMESLICE
        } else {
            0
        };

        RtEntity {
            policy,
            priority: AtomicU32::new(prio),
            time_slice: AtomicU64::new(time_slice),
            on_rq: core::sync::atomic::AtomicBool::new(false),
            run_cnt: AtomicU64::new(0),
        }
    }

    /// Get scheduling policy
    pub fn policy(&self) -> SchedPolicy {
        self.policy
    }

    /// Set scheduling policy
    pub fn set_policy(&mut self, policy: SchedPolicy) {
        self.policy = policy;
        if policy == SchedPolicy::RoundRobin {
            self.time_slice.store(SCHED_RR_TIMESLICE, Ordering::Relaxed);
        }
    }

    /// Get real-time priority
    pub fn priority(&self) -> u8 {
        self.priority.load(Ordering::Relaxed) as u8
    }

    /// Set real-time priority
    pub fn set_priority(&self, priority: u8) {
        let prio = if self.policy.is_realtime() {
            priority.clamp(RT_PRIO_MIN, RT_PRIO_MAX) as u32
        } else {
            0
        };
        self.priority.store(prio, Ordering::Relaxed);
    }

    /// Check if this is a real-time task
    pub fn is_rt(&self) -> bool {
        self.policy.is_realtime()
    }

    /// Get remaining time slice
    pub fn time_slice(&self) -> u64 {
        self.time_slice.load(Ordering::Relaxed)
    }

    /// Decrement time slice (returns true if expired)
    pub fn tick(&self) -> bool {
        if self.policy != SchedPolicy::RoundRobin {
            return false;
        }

        let remaining = self.time_slice.load(Ordering::Relaxed);
        if remaining > 0 {
            self.time_slice.store(remaining - 1, Ordering::Relaxed);
            remaining <= 1
        } else {
            true
        }
    }

    /// Reset time slice (for RR after requeue)
    pub fn reset_time_slice(&self) {
        if self.policy == SchedPolicy::RoundRobin {
            self.time_slice.store(SCHED_RR_TIMESLICE, Ordering::Relaxed);
        }
    }

    /// Check if on runqueue
    pub fn on_rq(&self) -> bool {
        self.on_rq.load(Ordering::Relaxed)
    }

    /// Set on runqueue flag
    pub fn set_on_rq(&self, on: bool) {
        self.on_rq.store(on, Ordering::Relaxed);
    }

    /// Increment run count
    pub fn inc_run_cnt(&self) {
        self.run_cnt.fetch_add(1, Ordering::Relaxed);
    }

    /// Get run count
    pub fn run_cnt(&self) -> u64 {
        self.run_cnt.load(Ordering::Relaxed)
    }
}

impl Default for RtEntity {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Check if a priority is valid for a given policy
pub fn is_valid_priority(policy: SchedPolicy, priority: i32) -> bool {
    match policy {
        SchedPolicy::Fifo | SchedPolicy::RoundRobin => {
            priority >= RT_PRIO_MIN as i32 && priority <= RT_PRIO_MAX as i32
        }
        SchedPolicy::Other | SchedPolicy::Batch => {
            priority == 0
        }
        SchedPolicy::Idle => {
            priority == 0
        }
        SchedPolicy::Deadline => {
            // Deadline scheduling uses different parameters
            true
        }
    }
}

/// Get the default time slice for a policy/priority combination
pub fn get_time_slice(policy: SchedPolicy, _priority: u8) -> u64 {
    match policy {
        SchedPolicy::RoundRobin => SCHED_RR_TIMESLICE,
        _ => 0, // FIFO and non-RT don't use time slices in the same way
    }
}

/// Convert nice value to effective priority (for sorting with RT)
/// RT priorities (1-99) are always higher than nice-based (100+)
pub fn effective_priority(policy: SchedPolicy, rt_priority: u8, nice: i8) -> u32 {
    match policy {
        SchedPolicy::Fifo | SchedPolicy::RoundRobin => {
            // RT priorities 1-99 map to effective 1-99 (higher = more urgent)
            rt_priority as u32
        }
        SchedPolicy::Other | SchedPolicy::Batch => {
            // Nice -20 to +19 maps to effective 100-139 (lower = more urgent)
            (120 + nice as i32) as u32
        }
        SchedPolicy::Idle => {
            // Idle priority is lowest
            u32::MAX
        }
        SchedPolicy::Deadline => {
            // Deadline uses different scheduling
            0
        }
    }
}

/// Initialize real-time scheduling support
pub fn init() {
    crate::kprintln!("rt: initializing real-time scheduling support");
    crate::kprintln!("rt: SCHED_FIFO and SCHED_RR enabled");
    crate::kprintln!("rt: RT priority range: {}-{}", RT_PRIO_MIN, RT_PRIO_MAX);
    crate::kprintln!("rt: RR default time slice: {} ticks", SCHED_RR_TIMESLICE);
}
