//! Memory Profiler for Stenzel OS.
//!
//! Provides memory allocation profiling with:
//! - Allocation tracking
//! - Leak detection
//! - Per-callsite statistics
//! - Memory usage reports
//! - Peak memory tracking

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use spin::{Mutex, Once};

// ============================================================================
// Allocation Types
// ============================================================================

/// Allocation type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AllocType {
    /// Kernel heap (kmalloc)
    Kernel,
    /// User heap
    User,
    /// Slab cache
    Slab,
    /// Page allocation
    Page,
    /// DMA allocation
    Dma,
    /// MMIO mapping
    Mmio,
    /// Stack allocation
    Stack,
    /// Per-CPU allocation
    PerCpu,
}

impl AllocType {
    /// Get string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            AllocType::Kernel => "kernel",
            AllocType::User => "user",
            AllocType::Slab => "slab",
            AllocType::Page => "page",
            AllocType::Dma => "dma",
            AllocType::Mmio => "mmio",
            AllocType::Stack => "stack",
            AllocType::PerCpu => "percpu",
        }
    }
}

/// Allocation flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllocFlags(u32);

impl AllocFlags {
    pub const NONE: Self = Self(0);
    pub const ZERO: Self = Self(1 << 0);
    pub const DMA: Self = Self(1 << 1);
    pub const NOWAIT: Self = Self(1 << 2);
    pub const ATOMIC: Self = Self(1 << 3);
    pub const KERNEL: Self = Self(1 << 4);
    pub const USER: Self = Self(1 << 5);
}

// ============================================================================
// Allocation Record
// ============================================================================

/// Record of a single allocation
#[derive(Debug, Clone)]
pub struct AllocationRecord {
    /// Allocation address
    pub addr: u64,
    /// Allocation size
    pub size: usize,
    /// Allocation type
    pub alloc_type: AllocType,
    /// Allocation flags
    pub flags: AllocFlags,
    /// Timestamp (nanoseconds since boot)
    pub timestamp: u64,
    /// Process ID that made the allocation
    pub pid: u32,
    /// Thread ID
    pub tid: u32,
    /// Call stack
    pub stack: Vec<u64>,
    /// File/function name (if known)
    pub caller: Option<String>,
    /// Line number (if known)
    pub line: u32,
}

impl AllocationRecord {
    /// Create new record
    pub fn new(
        addr: u64,
        size: usize,
        alloc_type: AllocType,
        flags: AllocFlags,
        pid: u32,
        tid: u32,
    ) -> Self {
        Self {
            addr,
            size,
            alloc_type,
            flags,
            timestamp: crate::time::uptime_ns(),
            pid,
            tid,
            stack: Vec::new(),
            caller: None,
            line: 0,
        }
    }

    /// Set call stack
    pub fn with_stack(mut self, stack: Vec<u64>) -> Self {
        self.stack = stack;
        self
    }

    /// Set caller info
    pub fn with_caller(mut self, caller: &str, line: u32) -> Self {
        self.caller = Some(caller.to_string());
        self.line = line;
        self
    }

    /// Get age in nanoseconds
    pub fn age_ns(&self) -> u64 {
        crate::time::uptime_ns().saturating_sub(self.timestamp)
    }
}

// ============================================================================
// Callsite Statistics
// ============================================================================

/// Statistics for a single call site
#[derive(Debug, Clone)]
pub struct CallsiteStats {
    /// Caller address/identifier
    pub caller: u64,
    /// Caller name (if resolved)
    pub caller_name: Option<String>,
    /// Total allocations from this site
    pub alloc_count: u64,
    /// Total deallocations from this site
    pub free_count: u64,
    /// Total bytes allocated
    pub total_bytes_allocated: u64,
    /// Total bytes freed
    pub total_bytes_freed: u64,
    /// Current live allocations
    pub live_count: u64,
    /// Current live bytes
    pub live_bytes: u64,
    /// Peak live count
    pub peak_count: u64,
    /// Peak live bytes
    pub peak_bytes: u64,
    /// Average allocation size
    pub avg_size: usize,
    /// Min allocation size
    pub min_size: usize,
    /// Max allocation size
    pub max_size: usize,
}

impl CallsiteStats {
    /// Create new stats
    pub fn new(caller: u64) -> Self {
        Self {
            caller,
            caller_name: None,
            alloc_count: 0,
            free_count: 0,
            total_bytes_allocated: 0,
            total_bytes_freed: 0,
            live_count: 0,
            live_bytes: 0,
            peak_count: 0,
            peak_bytes: 0,
            avg_size: 0,
            min_size: usize::MAX,
            max_size: 0,
        }
    }

    /// Record an allocation
    pub fn record_alloc(&mut self, size: usize) {
        self.alloc_count += 1;
        self.total_bytes_allocated += size as u64;
        self.live_count += 1;
        self.live_bytes += size as u64;

        if self.live_count > self.peak_count {
            self.peak_count = self.live_count;
        }
        if self.live_bytes > self.peak_bytes {
            self.peak_bytes = self.live_bytes;
        }

        if size < self.min_size {
            self.min_size = size;
        }
        if size > self.max_size {
            self.max_size = size;
        }

        self.avg_size = (self.total_bytes_allocated / self.alloc_count) as usize;
    }

    /// Record a deallocation
    pub fn record_free(&mut self, size: usize) {
        self.free_count += 1;
        self.total_bytes_freed += size as u64;
        self.live_count = self.live_count.saturating_sub(1);
        self.live_bytes = self.live_bytes.saturating_sub(size as u64);
    }

    /// Get potential leak count
    pub fn potential_leaks(&self) -> u64 {
        self.live_count
    }
}

// ============================================================================
// Memory Statistics
// ============================================================================

/// Global memory statistics
#[derive(Debug, Clone)]
pub struct MemoryStats {
    /// Total allocations
    pub total_allocs: u64,
    /// Total deallocations
    pub total_frees: u64,
    /// Total bytes allocated
    pub total_bytes_allocated: u64,
    /// Total bytes freed
    pub total_bytes_freed: u64,
    /// Current live allocations
    pub current_allocs: u64,
    /// Current live bytes
    pub current_bytes: u64,
    /// Peak allocations
    pub peak_allocs: u64,
    /// Peak bytes
    pub peak_bytes: u64,
    /// Failed allocations
    pub failed_allocs: u64,
    /// Per-type statistics
    pub per_type: BTreeMap<String, TypeStats>,
    /// Per-callsite statistics (top N)
    pub per_callsite: Vec<CallsiteStats>,
    /// Per-process statistics
    pub per_process: BTreeMap<u32, ProcessStats>,
}

impl MemoryStats {
    /// Create new stats
    pub fn new() -> Self {
        Self {
            total_allocs: 0,
            total_frees: 0,
            total_bytes_allocated: 0,
            total_bytes_freed: 0,
            current_allocs: 0,
            current_bytes: 0,
            peak_allocs: 0,
            peak_bytes: 0,
            failed_allocs: 0,
            per_type: BTreeMap::new(),
            per_callsite: Vec::new(),
            per_process: BTreeMap::new(),
        }
    }

    /// Format as text report
    pub fn format_report(&self) -> String {
        let mut report = String::new();

        report.push_str("=== Memory Profiler Report ===\n\n");
        report.push_str(&format!("Total allocations:   {}\n", self.total_allocs));
        report.push_str(&format!("Total frees:         {}\n", self.total_frees));
        report.push_str(&format!("Total allocated:     {} bytes\n", self.total_bytes_allocated));
        report.push_str(&format!("Total freed:         {} bytes\n", self.total_bytes_freed));
        report.push_str(&format!("Current allocs:      {}\n", self.current_allocs));
        report.push_str(&format!("Current bytes:       {} bytes\n", self.current_bytes));
        report.push_str(&format!("Peak allocs:         {}\n", self.peak_allocs));
        report.push_str(&format!("Peak bytes:          {} bytes\n", self.peak_bytes));
        report.push_str(&format!("Failed allocations:  {}\n\n", self.failed_allocs));

        if !self.per_type.is_empty() {
            report.push_str("=== Per-Type Statistics ===\n");
            for (type_name, stats) in &self.per_type {
                report.push_str(&format!("  {}: {} allocs, {} bytes\n",
                    type_name, stats.alloc_count, stats.live_bytes));
            }
            report.push_str("\n");
        }

        if !self.per_callsite.is_empty() {
            report.push_str("=== Top Call Sites ===\n");
            for (i, cs) in self.per_callsite.iter().take(20).enumerate() {
                let name = cs.caller_name.as_ref()
                    .map(|s| s.as_str())
                    .unwrap_or("unknown");
                report.push_str(&format!("{:2}. {} - {} live allocs, {} bytes\n",
                    i + 1, name, cs.live_count, cs.live_bytes));
            }
            report.push_str("\n");
        }

        report
    }
}

impl Default for MemoryStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Per-type statistics
#[derive(Debug, Clone)]
pub struct TypeStats {
    /// Allocation count
    pub alloc_count: u64,
    /// Free count
    pub free_count: u64,
    /// Live count
    pub live_count: u64,
    /// Live bytes
    pub live_bytes: u64,
}

impl TypeStats {
    pub fn new() -> Self {
        Self {
            alloc_count: 0,
            free_count: 0,
            live_count: 0,
            live_bytes: 0,
        }
    }
}

impl Default for TypeStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Per-process statistics
#[derive(Debug, Clone)]
pub struct ProcessStats {
    /// Process ID
    pub pid: u32,
    /// Process name
    pub name: String,
    /// Allocation count
    pub alloc_count: u64,
    /// Free count
    pub free_count: u64,
    /// Live bytes
    pub live_bytes: u64,
    /// Peak bytes
    pub peak_bytes: u64,
}

impl ProcessStats {
    pub fn new(pid: u32, name: &str) -> Self {
        Self {
            pid,
            name: name.to_string(),
            alloc_count: 0,
            free_count: 0,
            live_bytes: 0,
            peak_bytes: 0,
        }
    }
}

// ============================================================================
// Leak Detection
// ============================================================================

/// Potential memory leak
#[derive(Debug, Clone)]
pub struct PotentialLeak {
    /// Allocation record
    pub record: AllocationRecord,
    /// Age in nanoseconds
    pub age_ns: u64,
    /// Confidence level (0-100)
    pub confidence: u8,
    /// Leak reason/heuristic
    pub reason: LeakReason,
}

/// Leak detection reason
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeakReason {
    /// Long-lived allocation
    LongLived,
    /// Unreachable memory (no references found)
    Unreachable,
    /// Orphaned (owning process exited)
    Orphaned,
    /// Growing allocation (same caller keeps allocating without freeing)
    Growing,
    /// User-specified threshold exceeded
    ThresholdExceeded,
}

/// Leak detector configuration
#[derive(Debug, Clone)]
pub struct LeakDetectorConfig {
    /// Minimum age (nanoseconds) to consider as potential leak
    pub min_age_ns: u64,
    /// Track kernel allocations
    pub track_kernel: bool,
    /// Track user allocations
    pub track_user: bool,
    /// Maximum tracked allocations
    pub max_tracked: usize,
    /// Enable stack traces
    pub capture_stack: bool,
    /// Stack trace depth
    pub stack_depth: usize,
}

impl Default for LeakDetectorConfig {
    fn default() -> Self {
        Self {
            min_age_ns: 60_000_000_000, // 60 seconds
            track_kernel: true,
            track_user: true,
            max_tracked: 100_000,
            capture_stack: true,
            stack_depth: 32,
        }
    }
}

/// Leak detector
pub struct LeakDetector {
    /// Configuration
    config: LeakDetectorConfig,
    /// Active allocations
    allocations: BTreeMap<u64, AllocationRecord>,
    /// Per-callsite statistics
    callsites: BTreeMap<u64, CallsiteStats>,
    /// Detected leaks
    detected_leaks: Vec<PotentialLeak>,
    /// Last scan timestamp
    last_scan: u64,
}

impl LeakDetector {
    /// Create new detector
    pub fn new(config: LeakDetectorConfig) -> Self {
        Self {
            config,
            allocations: BTreeMap::new(),
            callsites: BTreeMap::new(),
            detected_leaks: Vec::new(),
            last_scan: 0,
        }
    }

    /// Record an allocation
    pub fn record_alloc(&mut self, record: AllocationRecord) {
        if self.allocations.len() >= self.config.max_tracked {
            return;
        }

        let addr = record.addr;
        let size = record.size;
        let caller = record.stack.first().copied().unwrap_or(0);

        self.allocations.insert(addr, record);

        // Update callsite stats
        let stats = self.callsites.entry(caller)
            .or_insert_with(|| CallsiteStats::new(caller));
        stats.record_alloc(size);
    }

    /// Record a deallocation
    pub fn record_free(&mut self, addr: u64) {
        if let Some(record) = self.allocations.remove(&addr) {
            let caller = record.stack.first().copied().unwrap_or(0);

            if let Some(stats) = self.callsites.get_mut(&caller) {
                stats.record_free(record.size);
            }
        }
    }

    /// Scan for leaks
    pub fn scan(&mut self) -> Vec<PotentialLeak> {
        let now = crate::time::uptime_ns();
        self.last_scan = now;
        self.detected_leaks.clear();

        for record in self.allocations.values() {
            let age = now.saturating_sub(record.timestamp);

            if age >= self.config.min_age_ns {
                let leak = PotentialLeak {
                    record: record.clone(),
                    age_ns: age,
                    confidence: self.calculate_confidence(record, age),
                    reason: LeakReason::LongLived,
                };
                self.detected_leaks.push(leak);
            }
        }

        // Check for growing allocations
        for stats in self.callsites.values() {
            if stats.live_count > 100 && stats.free_count == 0 {
                // This callsite has many allocations but no frees - suspicious
                // Mark all allocations from this site as potential leaks
                for record in self.allocations.values() {
                    let caller = record.stack.first().copied().unwrap_or(0);
                    if caller == stats.caller {
                        if !self.detected_leaks.iter().any(|l| l.record.addr == record.addr) {
                            let leak = PotentialLeak {
                                record: record.clone(),
                                age_ns: record.age_ns(),
                                confidence: 70,
                                reason: LeakReason::Growing,
                            };
                            self.detected_leaks.push(leak);
                        }
                    }
                }
            }
        }

        // Sort by confidence (descending)
        self.detected_leaks.sort_by(|a, b| b.confidence.cmp(&a.confidence));

        self.detected_leaks.clone()
    }

    /// Calculate leak confidence
    fn calculate_confidence(&self, record: &AllocationRecord, age: u64) -> u8 {
        let mut confidence: u32 = 0;

        // Age-based confidence
        let age_secs = age / 1_000_000_000;
        confidence += (age_secs.min(60) * 10 / 60) as u32;

        // Size-based confidence (larger allocations more suspicious)
        if record.size > 1024 * 1024 {
            confidence += 20;
        } else if record.size > 64 * 1024 {
            confidence += 10;
        }

        // Stack trace presence
        if !record.stack.is_empty() {
            confidence += 10;
        }

        confidence.min(100) as u8
    }

    /// Get statistics
    pub fn get_stats(&self) -> MemoryStats {
        let mut stats = MemoryStats::new();

        stats.current_allocs = self.allocations.len() as u64;

        for record in self.allocations.values() {
            stats.current_bytes += record.size as u64;
        }

        // Top callsites
        let mut callsites: Vec<_> = self.callsites.values().cloned().collect();
        callsites.sort_by(|a, b| b.live_bytes.cmp(&a.live_bytes));
        stats.per_callsite = callsites.into_iter().take(50).collect();

        stats
    }

    /// Get active allocation count
    pub fn allocation_count(&self) -> usize {
        self.allocations.len()
    }

    /// Clear all tracked allocations
    pub fn clear(&mut self) {
        self.allocations.clear();
        self.callsites.clear();
        self.detected_leaks.clear();
    }
}

// ============================================================================
// Memory Profiler
// ============================================================================

/// Global memory profiler
static MEM_PROFILER: Once<Mutex<MemoryProfiler>> = Once::new();

/// Get the memory profiler instance
pub fn profiler() -> &'static Mutex<MemoryProfiler> {
    MEM_PROFILER.call_once(|| Mutex::new(MemoryProfiler::new()))
}

/// Memory profiler state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfilerState {
    /// Profiler disabled
    Disabled,
    /// Profiler enabled
    Enabled,
    /// Profiler paused
    Paused,
}

/// Memory profiler
pub struct MemoryProfiler {
    /// Profiler state
    state: ProfilerState,
    /// Leak detector
    leak_detector: LeakDetector,
    /// Global statistics
    stats: MemoryStatCounters,
    /// Enabled flag (for fast checking)
    enabled: AtomicBool,
    /// Start timestamp
    start_time: u64,
}

/// Atomic counters for statistics
pub struct MemoryStatCounters {
    pub total_allocs: AtomicU64,
    pub total_frees: AtomicU64,
    pub total_bytes_allocated: AtomicU64,
    pub total_bytes_freed: AtomicU64,
    pub current_allocs: AtomicU64,
    pub current_bytes: AtomicU64,
    pub peak_allocs: AtomicU64,
    pub peak_bytes: AtomicU64,
    pub failed_allocs: AtomicU64,
}

impl MemoryStatCounters {
    fn new() -> Self {
        Self {
            total_allocs: AtomicU64::new(0),
            total_frees: AtomicU64::new(0),
            total_bytes_allocated: AtomicU64::new(0),
            total_bytes_freed: AtomicU64::new(0),
            current_allocs: AtomicU64::new(0),
            current_bytes: AtomicU64::new(0),
            peak_allocs: AtomicU64::new(0),
            peak_bytes: AtomicU64::new(0),
            failed_allocs: AtomicU64::new(0),
        }
    }

    fn snapshot(&self) -> MemoryStats {
        let mut stats = MemoryStats::new();
        stats.total_allocs = self.total_allocs.load(Ordering::Relaxed);
        stats.total_frees = self.total_frees.load(Ordering::Relaxed);
        stats.total_bytes_allocated = self.total_bytes_allocated.load(Ordering::Relaxed);
        stats.total_bytes_freed = self.total_bytes_freed.load(Ordering::Relaxed);
        stats.current_allocs = self.current_allocs.load(Ordering::Relaxed);
        stats.current_bytes = self.current_bytes.load(Ordering::Relaxed);
        stats.peak_allocs = self.peak_allocs.load(Ordering::Relaxed);
        stats.peak_bytes = self.peak_bytes.load(Ordering::Relaxed);
        stats.failed_allocs = self.failed_allocs.load(Ordering::Relaxed);
        stats
    }
}

impl MemoryProfiler {
    /// Create new profiler
    pub fn new() -> Self {
        Self {
            state: ProfilerState::Disabled,
            leak_detector: LeakDetector::new(LeakDetectorConfig::default()),
            stats: MemoryStatCounters::new(),
            enabled: AtomicBool::new(false),
            start_time: 0,
        }
    }

    /// Enable profiling
    pub fn enable(&mut self) {
        self.state = ProfilerState::Enabled;
        self.enabled.store(true, Ordering::SeqCst);
        self.start_time = crate::time::uptime_ns();
    }

    /// Disable profiling
    pub fn disable(&mut self) {
        self.state = ProfilerState::Disabled;
        self.enabled.store(false, Ordering::SeqCst);
    }

    /// Pause profiling
    pub fn pause(&mut self) {
        if self.state == ProfilerState::Enabled {
            self.state = ProfilerState::Paused;
            self.enabled.store(false, Ordering::SeqCst);
        }
    }

    /// Resume profiling
    pub fn resume(&mut self) {
        if self.state == ProfilerState::Paused {
            self.state = ProfilerState::Enabled;
            self.enabled.store(true, Ordering::SeqCst);
        }
    }

    /// Check if profiling is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Record an allocation
    pub fn record_alloc(
        &mut self,
        addr: u64,
        size: usize,
        alloc_type: AllocType,
        flags: AllocFlags,
        pid: u32,
        tid: u32,
        stack: Option<Vec<u64>>,
    ) {
        if !self.is_enabled() {
            return;
        }

        // Update atomic counters
        self.stats.total_allocs.fetch_add(1, Ordering::Relaxed);
        self.stats.total_bytes_allocated.fetch_add(size as u64, Ordering::Relaxed);

        let current_allocs = self.stats.current_allocs.fetch_add(1, Ordering::Relaxed) + 1;
        let current_bytes = self.stats.current_bytes.fetch_add(size as u64, Ordering::Relaxed) + size as u64;

        // Update peak values
        loop {
            let peak = self.stats.peak_allocs.load(Ordering::Relaxed);
            if current_allocs <= peak {
                break;
            }
            if self.stats.peak_allocs.compare_exchange_weak(
                peak,
                current_allocs,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ).is_ok() {
                break;
            }
        }

        loop {
            let peak = self.stats.peak_bytes.load(Ordering::Relaxed);
            if current_bytes <= peak {
                break;
            }
            if self.stats.peak_bytes.compare_exchange_weak(
                peak,
                current_bytes,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ).is_ok() {
                break;
            }
        }

        // Create detailed record for leak detector
        let mut record = AllocationRecord::new(addr, size, alloc_type, flags, pid, tid);
        if let Some(stack) = stack {
            record = record.with_stack(stack);
        }
        self.leak_detector.record_alloc(record);
    }

    /// Record a deallocation
    pub fn record_free(&mut self, addr: u64, size: usize) {
        if !self.is_enabled() {
            return;
        }

        self.stats.total_frees.fetch_add(1, Ordering::Relaxed);
        self.stats.total_bytes_freed.fetch_add(size as u64, Ordering::Relaxed);
        self.stats.current_allocs.fetch_sub(1, Ordering::Relaxed);
        self.stats.current_bytes.fetch_sub(size as u64, Ordering::Relaxed);

        self.leak_detector.record_free(addr);
    }

    /// Record a failed allocation
    pub fn record_failed_alloc(&mut self, _size: usize) {
        self.stats.failed_allocs.fetch_add(1, Ordering::Relaxed);
    }

    /// Scan for memory leaks
    pub fn scan_for_leaks(&mut self) -> Vec<PotentialLeak> {
        self.leak_detector.scan()
    }

    /// Get current statistics
    pub fn get_stats(&self) -> MemoryStats {
        let mut stats = self.stats.snapshot();

        // Add leak detector stats
        let leak_stats = self.leak_detector.get_stats();
        stats.per_callsite = leak_stats.per_callsite;

        stats
    }

    /// Get profiling duration
    pub fn duration_ns(&self) -> u64 {
        if self.start_time == 0 {
            return 0;
        }
        crate::time::uptime_ns() - self.start_time
    }

    /// Reset all statistics
    pub fn reset(&mut self) {
        self.stats = MemoryStatCounters::new();
        self.leak_detector.clear();
        self.start_time = crate::time::uptime_ns();
    }

    /// Get state
    pub fn state(&self) -> ProfilerState {
        self.state
    }
}

impl Default for MemoryProfiler {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Public API
// ============================================================================

/// Initialize the memory profiler
pub fn init() {
    let _ = profiler();
}

/// Enable memory profiling
pub fn enable_profiling() {
    profiler().lock().enable();
}

/// Disable memory profiling
pub fn disable_profiling() {
    profiler().lock().disable();
}

/// Pause memory profiling
pub fn pause_profiling() {
    profiler().lock().pause();
}

/// Resume memory profiling
pub fn resume_profiling() {
    profiler().lock().resume();
}

/// Check if profiling is enabled
pub fn is_profiling_enabled() -> bool {
    profiler().lock().is_enabled()
}

/// Record an allocation
pub fn record_alloc(
    addr: u64,
    size: usize,
    alloc_type: AllocType,
    flags: AllocFlags,
    pid: u32,
    tid: u32,
    stack: Option<Vec<u64>>,
) {
    profiler().lock().record_alloc(addr, size, alloc_type, flags, pid, tid, stack);
}

/// Record a deallocation
pub fn record_free(addr: u64, size: usize) {
    profiler().lock().record_free(addr, size);
}

/// Record a failed allocation
pub fn record_failed_alloc(size: usize) {
    profiler().lock().record_failed_alloc(size);
}

/// Scan for memory leaks
pub fn scan_for_leaks() -> Vec<PotentialLeak> {
    profiler().lock().scan_for_leaks()
}

/// Get memory statistics
pub fn get_stats() -> MemoryStats {
    profiler().lock().get_stats()
}

/// Generate memory report
pub fn generate_report() -> String {
    get_stats().format_report()
}

/// Reset profiler
pub fn reset_profiler() {
    profiler().lock().reset();
}

// ============================================================================
// Allocation Hooks (for integration with allocator)
// ============================================================================

/// Hook for kmalloc-style allocations
#[inline]
pub fn on_kmalloc(addr: *mut u8, size: usize, pid: u32) {
    if is_profiling_enabled() {
        record_alloc(
            addr as u64,
            size,
            AllocType::Kernel,
            AllocFlags::KERNEL,
            pid,
            pid, // tid = pid for simplicity
            None,
        );
    }
}

/// Hook for kfree-style deallocations
#[inline]
pub fn on_kfree(addr: *mut u8, size: usize) {
    if is_profiling_enabled() {
        record_free(addr as u64, size);
    }
}

/// Hook for page allocations
#[inline]
pub fn on_page_alloc(addr: u64, pages: usize, pid: u32) {
    if is_profiling_enabled() {
        record_alloc(
            addr,
            pages * 4096,
            AllocType::Page,
            AllocFlags::NONE,
            pid,
            pid,
            None,
        );
    }
}

/// Hook for page deallocations
#[inline]
pub fn on_page_free(addr: u64, pages: usize) {
    if is_profiling_enabled() {
        record_free(addr, pages * 4096);
    }
}

// ============================================================================
// Memory Usage Summary
// ============================================================================

/// Memory usage category
#[derive(Debug, Clone)]
pub struct MemoryCategory {
    /// Category name
    pub name: String,
    /// Used bytes
    pub used: u64,
    /// Available bytes
    pub available: u64,
    /// Percentage used
    pub percent_used: f64,
}

/// Get memory usage summary
pub fn get_memory_summary() -> Vec<MemoryCategory> {
    let stats = get_stats();

    vec![
        MemoryCategory {
            name: "Heap".to_string(),
            used: stats.current_bytes,
            available: 0, // Would come from allocator
            percent_used: 0.0,
        },
    ]
}

// ============================================================================
// Memory Histogram
// ============================================================================

/// Allocation size histogram
pub struct AllocationHistogram {
    /// Buckets: [0-64), [64-256), [256-1K), [1K-4K), [4K-16K), [16K-64K), [64K-256K), [256K-1M), [1M+)
    buckets: [u64; 9],
}

impl AllocationHistogram {
    /// Create new histogram
    pub fn new() -> Self {
        Self { buckets: [0; 9] }
    }

    /// Add a size to the histogram
    pub fn add(&mut self, size: usize) {
        let bucket = match size {
            0..=63 => 0,
            64..=255 => 1,
            256..=1023 => 2,
            1024..=4095 => 3,
            4096..=16383 => 4,
            16384..=65535 => 5,
            65536..=262143 => 6,
            262144..=1048575 => 7,
            _ => 8,
        };
        self.buckets[bucket] += 1;
    }

    /// Get bucket counts
    pub fn buckets(&self) -> &[u64; 9] {
        &self.buckets
    }

    /// Format as text
    pub fn format(&self) -> String {
        let labels = [
            "0-64B", "64-256B", "256B-1K", "1K-4K", "4K-16K",
            "16K-64K", "64K-256K", "256K-1M", "1M+",
        ];

        let mut result = String::new();
        let max = *self.buckets.iter().max().unwrap_or(&1);

        for (i, &count) in self.buckets.iter().enumerate() {
            let bar_len = if max > 0 { (count * 40 / max) as usize } else { 0 };
            let bar: String = core::iter::repeat('#').take(bar_len).collect();
            result.push_str(&format!("{:>10}: {:>8} |{}\n", labels[i], count, bar));
        }

        result
    }
}

impl Default for AllocationHistogram {
    fn default() -> Self {
        Self::new()
    }
}
