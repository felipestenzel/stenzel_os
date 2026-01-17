//! SMP Load Balancing
//!
//! Implements load balancing across CPUs in SMP systems.
//! This module distributes tasks evenly across available CPUs
//! to maximize throughput and minimize latency.
//!
//! Features:
//! - Per-CPU runqueues with load tracking
//! - Periodic load balancing
//! - Task migration between CPUs
//! - Push/pull balancing strategies
//! - NUMA-aware balancing (future)
//!
//! References:
//! - Linux kernel scheduler load balancing
//! - https://www.kernel.org/doc/Documentation/scheduler/sched-domains.txt

#![allow(dead_code)]

extern crate alloc;

use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use spin::Mutex;

use crate::arch::x86_64_arch::smp::MAX_CPUS;

// ============================================================================
// Constants
// ============================================================================

/// Load balancing interval in ticks (100ms = 10 ticks at 100Hz)
pub const BALANCE_INTERVAL: u64 = 10;

/// Maximum imbalance before triggering migration
pub const IMBALANCE_THRESHOLD: u32 = 25; // 25% imbalance

/// Maximum tasks to migrate in one balance operation
pub const MAX_MIGRATE_PER_BALANCE: usize = 4;

/// Load weight for running task
pub const RUNNING_WEIGHT: u32 = 1024;

/// Load calculation period in ticks
pub const LOAD_PERIOD: u64 = 50; // ~500ms at 100Hz

/// Number of load averaging samples
pub const LOAD_SAMPLES: usize = 5;

// ============================================================================
// Load Tracking
// ============================================================================

/// Per-CPU load statistics
#[derive(Debug)]
pub struct CpuLoad {
    /// Current load (sum of task weights)
    current: AtomicU32,
    /// Running load average (exponential moving average)
    avg: AtomicU32,
    /// Number of runnable tasks
    nr_running: AtomicU32,
    /// Total runtime in this period (nanoseconds)
    runtime_ns: AtomicU64,
    /// Last update tick
    last_update: AtomicU64,
    /// Load samples for averaging
    samples: [AtomicU32; LOAD_SAMPLES],
    /// Current sample index
    sample_idx: AtomicU32,
}

impl CpuLoad {
    /// Create new CPU load tracker
    pub const fn new() -> Self {
        CpuLoad {
            current: AtomicU32::new(0),
            avg: AtomicU32::new(0),
            nr_running: AtomicU32::new(0),
            runtime_ns: AtomicU64::new(0),
            last_update: AtomicU64::new(0),
            samples: [
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
            ],
            sample_idx: AtomicU32::new(0),
        }
    }

    /// Get current load
    pub fn current_load(&self) -> u32 {
        self.current.load(Ordering::Relaxed)
    }

    /// Get load average
    pub fn avg_load(&self) -> u32 {
        self.avg.load(Ordering::Relaxed)
    }

    /// Get number of runnable tasks
    pub fn nr_running(&self) -> u32 {
        self.nr_running.load(Ordering::Relaxed)
    }

    /// Add task to load
    pub fn task_enqueued(&self, weight: u32) {
        self.current.fetch_add(weight, Ordering::Relaxed);
        self.nr_running.fetch_add(1, Ordering::Relaxed);
    }

    /// Remove task from load
    pub fn task_dequeued(&self, weight: u32) {
        self.current.fetch_sub(weight.min(self.current.load(Ordering::Relaxed)), Ordering::Relaxed);
        let nr = self.nr_running.load(Ordering::Relaxed);
        if nr > 0 {
            self.nr_running.fetch_sub(1, Ordering::Relaxed);
        }
    }

    /// Update load average (called periodically)
    pub fn update_avg(&self, current_tick: u64) {
        let last = self.last_update.load(Ordering::Relaxed);
        if current_tick - last < LOAD_PERIOD {
            return;
        }

        self.last_update.store(current_tick, Ordering::Relaxed);

        // Store sample
        let idx = self.sample_idx.fetch_add(1, Ordering::Relaxed) as usize % LOAD_SAMPLES;
        self.samples[idx].store(self.current.load(Ordering::Relaxed), Ordering::Relaxed);

        // Calculate average
        let mut sum = 0u32;
        for sample in &self.samples {
            sum += sample.load(Ordering::Relaxed);
        }
        let avg = sum / LOAD_SAMPLES as u32;
        self.avg.store(avg, Ordering::Relaxed);
    }

    /// Add runtime
    pub fn add_runtime(&self, ns: u64) {
        self.runtime_ns.fetch_add(ns, Ordering::Relaxed);
    }

    /// Get and reset runtime
    pub fn take_runtime(&self) -> u64 {
        self.runtime_ns.swap(0, Ordering::Relaxed)
    }
}

// ============================================================================
// Load Balancer
// ============================================================================

/// Task migration request
#[derive(Debug, Clone)]
pub struct MigrationRequest {
    /// Task ID to migrate
    pub task_id: u64,
    /// Source CPU
    pub from_cpu: u32,
    /// Destination CPU
    pub to_cpu: u32,
    /// Task weight
    pub weight: u32,
}

/// Load balancer state
pub struct LoadBalancer {
    /// Per-CPU load statistics
    cpu_loads: [CpuLoad; MAX_CPUS],
    /// Number of online CPUs
    nr_cpus: AtomicU32,
    /// Last balance tick
    last_balance: AtomicU64,
    /// Whether balancing is enabled
    enabled: AtomicBool,
    /// Pending migrations
    migrations: Mutex<VecDeque<MigrationRequest>>,
    /// Statistics
    stats: BalanceStats,
}

/// Load balancing statistics
#[derive(Debug, Default)]
pub struct BalanceStats {
    /// Number of balance operations
    pub balance_count: AtomicU64,
    /// Number of tasks migrated
    pub migrations: AtomicU64,
    /// Number of failed migrations
    pub failed_migrations: AtomicU64,
    /// Number of times balancing was skipped (no imbalance)
    pub balanced_skips: AtomicU64,
}

impl LoadBalancer {
    /// Create a new load balancer
    pub const fn new() -> Self {
        const CPU_LOAD: CpuLoad = CpuLoad::new();
        LoadBalancer {
            cpu_loads: [CPU_LOAD; MAX_CPUS],
            nr_cpus: AtomicU32::new(1),
            last_balance: AtomicU64::new(0),
            enabled: AtomicBool::new(true),
            migrations: Mutex::new(VecDeque::new()),
            stats: BalanceStats {
                balance_count: AtomicU64::new(0),
                migrations: AtomicU64::new(0),
                failed_migrations: AtomicU64::new(0),
                balanced_skips: AtomicU64::new(0),
            },
        }
    }

    /// Set number of online CPUs
    pub fn set_nr_cpus(&self, nr: u32) {
        self.nr_cpus.store(nr.min(MAX_CPUS as u32), Ordering::Relaxed);
    }

    /// Get number of online CPUs
    pub fn nr_cpus(&self) -> u32 {
        self.nr_cpus.load(Ordering::Relaxed)
    }

    /// Get CPU load tracker
    pub fn cpu_load(&self, cpu: u32) -> &CpuLoad {
        &self.cpu_loads[cpu as usize % MAX_CPUS]
    }

    /// Enable/disable load balancing
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    /// Check if load balancing is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Check if balancing is needed (called from timer)
    pub fn should_balance(&self, current_tick: u64) -> bool {
        if !self.is_enabled() {
            return false;
        }

        if self.nr_cpus() <= 1 {
            return false;
        }

        let last = self.last_balance.load(Ordering::Relaxed);
        current_tick - last >= BALANCE_INTERVAL
    }

    /// Perform load balancing
    ///
    /// Returns list of tasks to migrate
    pub fn balance(&self, current_tick: u64) -> Vec<MigrationRequest> {
        self.last_balance.store(current_tick, Ordering::Relaxed);
        self.stats.balance_count.fetch_add(1, Ordering::Relaxed);

        let nr_cpus = self.nr_cpus() as usize;
        if nr_cpus <= 1 {
            return Vec::new();
        }

        // Update all CPU load averages
        for cpu in 0..nr_cpus {
            self.cpu_loads[cpu].update_avg(current_tick);
        }

        // Calculate total and average load
        let mut total_load = 0u64;
        for cpu in 0..nr_cpus {
            total_load += self.cpu_loads[cpu].avg_load() as u64;
        }
        let avg_load = total_load / nr_cpus as u64;

        if avg_load == 0 {
            self.stats.balanced_skips.fetch_add(1, Ordering::Relaxed);
            return Vec::new();
        }

        // Find busiest and idlest CPUs
        let mut busiest_cpu = 0;
        let mut busiest_load = 0u32;
        let mut idlest_cpu = 0;
        let mut idlest_load = u32::MAX;

        for cpu in 0..nr_cpus {
            let load = self.cpu_loads[cpu].avg_load();
            if load > busiest_load {
                busiest_load = load;
                busiest_cpu = cpu;
            }
            if load < idlest_load {
                idlest_load = load;
                idlest_cpu = cpu;
            }
        }

        // Check if imbalance is significant
        if busiest_cpu == idlest_cpu {
            self.stats.balanced_skips.fetch_add(1, Ordering::Relaxed);
            return Vec::new();
        }

        let imbalance = if avg_load > 0 {
            ((busiest_load as i64 - idlest_load as i64).unsigned_abs() * 100 / avg_load as u64) as u32
        } else {
            0
        };

        if imbalance < IMBALANCE_THRESHOLD {
            self.stats.balanced_skips.fetch_add(1, Ordering::Relaxed);
            return Vec::new();
        }

        // Calculate how much load to migrate
        let target_load = avg_load as u32;
        let to_migrate = busiest_load.saturating_sub(target_load);

        // In a real implementation, we would iterate through tasks on busiest_cpu
        // and select appropriate tasks to migrate. For now, return the migration intent.
        let migrations = Vec::new();

        // Note: Actual task selection happens in the scheduler with access to the runqueue
        // This function provides the balancing decision and target
        migrations
    }

    /// Find best CPU for a new task
    pub fn find_idlest_cpu(&self) -> u32 {
        let nr_cpus = self.nr_cpus() as usize;
        if nr_cpus <= 1 {
            return 0;
        }

        let mut idlest = 0;
        let mut min_load = u32::MAX;

        for cpu in 0..nr_cpus {
            let load = self.cpu_loads[cpu].avg_load();
            let nr_running = self.cpu_loads[cpu].nr_running();

            // Prefer completely idle CPUs
            if nr_running == 0 {
                return cpu as u32;
            }

            // Otherwise use lowest load
            if load < min_load {
                min_load = load;
                idlest = cpu;
            }
        }

        idlest as u32
    }

    /// Find busiest CPU (for work stealing)
    pub fn find_busiest_cpu(&self, current_cpu: u32) -> Option<u32> {
        let nr_cpus = self.nr_cpus() as usize;
        if nr_cpus <= 1 {
            return None;
        }

        let mut busiest = None;
        let mut max_load = 0;
        let my_load = self.cpu_loads[current_cpu as usize].avg_load();

        for cpu in 0..nr_cpus {
            if cpu == current_cpu as usize {
                continue;
            }

            let load = self.cpu_loads[cpu].avg_load();
            let nr_running = self.cpu_loads[cpu].nr_running();

            // Only steal from CPUs with multiple runnable tasks
            if nr_running > 1 && load > max_load && load > my_load {
                max_load = load;
                busiest = Some(cpu as u32);
            }
        }

        busiest
    }

    /// Queue a migration request
    pub fn queue_migration(&self, request: MigrationRequest) {
        let mut migrations = self.migrations.lock();
        migrations.push_back(request);
    }

    /// Get next pending migration
    pub fn pop_migration(&self) -> Option<MigrationRequest> {
        let mut migrations = self.migrations.lock();
        migrations.pop_front()
    }

    /// Record successful migration
    pub fn migration_succeeded(&self) {
        self.stats.migrations.fetch_add(1, Ordering::Relaxed);
    }

    /// Record failed migration
    pub fn migration_failed(&self) {
        self.stats.failed_migrations.fetch_add(1, Ordering::Relaxed);
    }

    /// Get statistics
    pub fn stats(&self) -> &BalanceStats {
        &self.stats
    }

    /// Get load summary for all CPUs
    pub fn load_summary(&self) -> LoadSummary {
        let nr_cpus = self.nr_cpus() as usize;
        let mut total_load = 0u32;
        let mut total_running = 0u32;
        let mut min_load = u32::MAX;
        let mut max_load = 0u32;

        for cpu in 0..nr_cpus {
            let load = self.cpu_loads[cpu].avg_load();
            let running = self.cpu_loads[cpu].nr_running();

            total_load += load;
            total_running += running;
            min_load = min_load.min(load);
            max_load = max_load.max(load);
        }

        LoadSummary {
            nr_cpus: nr_cpus as u32,
            total_load,
            total_running,
            avg_load: if nr_cpus > 0 { total_load / nr_cpus as u32 } else { 0 },
            min_load,
            max_load,
            imbalance: if total_load > 0 {
                (max_load.saturating_sub(min_load)) * 100 / total_load.max(1)
            } else {
                0
            },
        }
    }
}

/// Load summary across all CPUs
#[derive(Debug, Clone)]
pub struct LoadSummary {
    /// Number of CPUs
    pub nr_cpus: u32,
    /// Total system load
    pub total_load: u32,
    /// Total runnable tasks
    pub total_running: u32,
    /// Average load per CPU
    pub avg_load: u32,
    /// Minimum CPU load
    pub min_load: u32,
    /// Maximum CPU load
    pub max_load: u32,
    /// Imbalance percentage
    pub imbalance: u32,
}

// ============================================================================
// CPU Affinity
// ============================================================================

/// CPU affinity mask (supports up to 256 CPUs)
#[derive(Clone, Copy, Default)]
pub struct CpuMask {
    bits: [u64; 4],
}

impl CpuMask {
    /// Create an empty mask
    pub const fn empty() -> Self {
        CpuMask { bits: [0; 4] }
    }

    /// Create a mask with all CPUs set
    pub const fn all() -> Self {
        CpuMask { bits: [!0; 4] }
    }

    /// Create a mask with a single CPU
    pub fn single(cpu: u32) -> Self {
        let mut mask = Self::empty();
        mask.set(cpu);
        mask
    }

    /// Set a CPU in the mask
    pub fn set(&mut self, cpu: u32) {
        let idx = (cpu / 64) as usize;
        let bit = cpu % 64;
        if idx < 4 {
            self.bits[idx] |= 1u64 << bit;
        }
    }

    /// Clear a CPU from the mask
    pub fn clear(&mut self, cpu: u32) {
        let idx = (cpu / 64) as usize;
        let bit = cpu % 64;
        if idx < 4 {
            self.bits[idx] &= !(1u64 << bit);
        }
    }

    /// Check if a CPU is set
    pub fn is_set(&self, cpu: u32) -> bool {
        let idx = (cpu / 64) as usize;
        let bit = cpu % 64;
        if idx < 4 {
            (self.bits[idx] & (1u64 << bit)) != 0
        } else {
            false
        }
    }

    /// Check if mask is empty
    pub fn is_empty(&self) -> bool {
        self.bits.iter().all(|&b| b == 0)
    }

    /// Count number of CPUs in mask
    pub fn count(&self) -> u32 {
        self.bits.iter().map(|b| b.count_ones()).sum()
    }

    /// Get first set CPU
    pub fn first(&self) -> Option<u32> {
        for (idx, &bits) in self.bits.iter().enumerate() {
            if bits != 0 {
                return Some(idx as u32 * 64 + bits.trailing_zeros());
            }
        }
        None
    }

    /// Get next set CPU after given CPU
    pub fn next(&self, cpu: u32) -> Option<u32> {
        let start_idx = (cpu / 64) as usize;
        let start_bit = cpu % 64 + 1;

        // Check remaining bits in current word
        if start_idx < 4 && start_bit < 64 {
            let masked = self.bits[start_idx] & (!0u64 << start_bit);
            if masked != 0 {
                return Some(start_idx as u32 * 64 + masked.trailing_zeros());
            }
        }

        // Check remaining words
        for idx in (start_idx + 1)..4 {
            if self.bits[idx] != 0 {
                return Some(idx as u32 * 64 + self.bits[idx].trailing_zeros());
            }
        }

        None
    }

    /// Bitwise AND
    pub fn and(&self, other: &CpuMask) -> CpuMask {
        CpuMask {
            bits: [
                self.bits[0] & other.bits[0],
                self.bits[1] & other.bits[1],
                self.bits[2] & other.bits[2],
                self.bits[3] & other.bits[3],
            ],
        }
    }

    /// Bitwise OR
    pub fn or(&self, other: &CpuMask) -> CpuMask {
        CpuMask {
            bits: [
                self.bits[0] | other.bits[0],
                self.bits[1] | other.bits[1],
                self.bits[2] | other.bits[2],
                self.bits[3] | other.bits[3],
            ],
        }
    }

    /// Convert from Linux-style cpu_set_t (first 8 bytes)
    pub fn from_cpu_set(set: &[u8]) -> Self {
        let mut mask = Self::empty();
        for (i, chunk) in set.chunks(8).enumerate().take(4) {
            if chunk.len() == 8 {
                mask.bits[i] = u64::from_ne_bytes([
                    chunk[0], chunk[1], chunk[2], chunk[3],
                    chunk[4], chunk[5], chunk[6], chunk[7],
                ]);
            }
        }
        mask
    }

    /// Convert to bytes (for syscall return)
    pub fn to_bytes(&self) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        for (i, &word) in self.bits.iter().enumerate() {
            let word_bytes = word.to_ne_bytes();
            bytes[i * 8..i * 8 + 8].copy_from_slice(&word_bytes);
        }
        bytes
    }
}

impl core::fmt::Debug for CpuMask {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "CpuMask({:016x}{:016x}{:016x}{:016x})",
               self.bits[3], self.bits[2], self.bits[1], self.bits[0])
    }
}

// ============================================================================
// Global State
// ============================================================================

/// Global load balancer
pub static LOAD_BALANCER: LoadBalancer = LoadBalancer::new();

/// Initialize load balancing
pub fn init(nr_cpus: u32) {
    LOAD_BALANCER.set_nr_cpus(nr_cpus);
    crate::kprintln!("balance: load balancer initialized for {} CPUs", nr_cpus);
}

/// Get the load balancer
pub fn balancer() -> &'static LoadBalancer {
    &LOAD_BALANCER
}
