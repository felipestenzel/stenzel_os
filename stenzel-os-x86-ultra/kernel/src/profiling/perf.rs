//! Performance Monitoring Subsystem (perf) for Stenzel OS.
//!
//! Provides hardware and software performance counter support for profiling.
//!
//! Features:
//! - Hardware Performance Counters (PMU)
//! - Software event tracing
//! - CPU cycle counting
//! - Cache miss tracking
//! - Branch misprediction counting
//! - Process-level and system-wide profiling
//! - Sampling and counting modes
//! - perf_event-like interface

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use spin::{Mutex, Once};

// ============================================================================
// Event Types
// ============================================================================

/// Hardware performance event types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HardwareEvent {
    /// CPU cycles
    Cycles,
    /// Instructions retired
    Instructions,
    /// Cache references
    CacheReferences,
    /// Cache misses
    CacheMisses,
    /// Branch instructions
    BranchInstructions,
    /// Branch misses
    BranchMisses,
    /// Bus cycles
    BusCycles,
    /// Stalled cycles (frontend)
    StalledCyclesFrontend,
    /// Stalled cycles (backend)
    StalledCyclesBackend,
    /// Reference CPU cycles
    RefCycles,
}

impl HardwareEvent {
    /// Get PMC select value for Intel/AMD
    pub fn pmc_select(&self) -> Option<u64> {
        // These are Intel Core i7+ event selects
        // Real implementation would detect CPU and use appropriate values
        match self {
            HardwareEvent::Cycles => Some(0x003C),
            HardwareEvent::Instructions => Some(0x00C0),
            HardwareEvent::CacheReferences => Some(0x4F2E),
            HardwareEvent::CacheMisses => Some(0x412E),
            HardwareEvent::BranchInstructions => Some(0x00C4),
            HardwareEvent::BranchMisses => Some(0x00C5),
            HardwareEvent::BusCycles => Some(0x013C),
            _ => None,
        }
    }

    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            HardwareEvent::Cycles => "cycles",
            HardwareEvent::Instructions => "instructions",
            HardwareEvent::CacheReferences => "cache-references",
            HardwareEvent::CacheMisses => "cache-misses",
            HardwareEvent::BranchInstructions => "branch-instructions",
            HardwareEvent::BranchMisses => "branch-misses",
            HardwareEvent::BusCycles => "bus-cycles",
            HardwareEvent::StalledCyclesFrontend => "stalled-cycles-frontend",
            HardwareEvent::StalledCyclesBackend => "stalled-cycles-backend",
            HardwareEvent::RefCycles => "ref-cycles",
        }
    }
}

/// Software performance event types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoftwareEvent {
    /// CPU clock
    CpuClock,
    /// Task clock
    TaskClock,
    /// Page faults
    PageFaults,
    /// Context switches
    ContextSwitches,
    /// CPU migrations
    CpuMigrations,
    /// Minor faults
    MinorFaults,
    /// Major faults
    MajorFaults,
    /// Alignment faults
    AlignmentFaults,
    /// Emulation faults
    EmulationFaults,
}

impl SoftwareEvent {
    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            SoftwareEvent::CpuClock => "cpu-clock",
            SoftwareEvent::TaskClock => "task-clock",
            SoftwareEvent::PageFaults => "page-faults",
            SoftwareEvent::ContextSwitches => "context-switches",
            SoftwareEvent::CpuMigrations => "cpu-migrations",
            SoftwareEvent::MinorFaults => "minor-faults",
            SoftwareEvent::MajorFaults => "major-faults",
            SoftwareEvent::AlignmentFaults => "alignment-faults",
            SoftwareEvent::EmulationFaults => "emulation-faults",
        }
    }
}

/// Cache event types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheEvent {
    /// L1 data cache
    L1D,
    /// L1 instruction cache
    L1I,
    /// Last level cache
    LL,
    /// Data TLB
    DTLB,
    /// Instruction TLB
    ITLB,
    /// Branch prediction unit
    BPU,
    /// Node (NUMA)
    Node,
}

/// Cache operation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheOp {
    /// Read
    Read,
    /// Write
    Write,
    /// Prefetch
    Prefetch,
}

/// Cache result
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheResult {
    /// Access (hit or miss)
    Access,
    /// Miss
    Miss,
}

/// Complete event specification
#[derive(Debug, Clone)]
pub enum PerfEvent {
    /// Hardware event
    Hardware(HardwareEvent),
    /// Software event
    Software(SoftwareEvent),
    /// Cache event
    Cache(CacheEvent, CacheOp, CacheResult),
    /// Raw PMU event
    Raw(u64),
    /// Tracepoint (subsystem:event)
    Tracepoint(String),
}

// ============================================================================
// Event Configuration
// ============================================================================

/// Event sampling mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleMode {
    /// Count events only
    Counting,
    /// Sample on overflow
    Sampling,
}

/// Event scope
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventScope {
    /// Process-local
    Process(u32),
    /// Thread-local
    Thread(u32),
    /// System-wide (per-CPU)
    SystemWide,
    /// CPU-specific
    Cpu(u32),
}

/// Perf event attributes
#[derive(Debug, Clone)]
pub struct PerfEventAttr {
    /// Event type
    pub event: PerfEvent,
    /// Sample mode
    pub mode: SampleMode,
    /// Event scope
    pub scope: EventScope,
    /// Sample period (for sampling mode)
    pub sample_period: u64,
    /// Sample frequency (alternative to period)
    pub sample_freq: u64,
    /// Use frequency instead of period
    pub freq: bool,
    /// Exclude user space
    pub exclude_user: bool,
    /// Exclude kernel
    pub exclude_kernel: bool,
    /// Exclude hypervisor
    pub exclude_hv: bool,
    /// Exclude idle
    pub exclude_idle: bool,
    /// Enable on exec
    pub enable_on_exec: bool,
    /// Inherit to child tasks
    pub inherit: bool,
    /// Inherit to child tasks with statistics
    pub inherit_stat: bool,
    /// Sample TID
    pub sample_id_all: bool,
    /// Enabled by default
    pub disabled: bool,
}

impl Default for PerfEventAttr {
    fn default() -> Self {
        Self {
            event: PerfEvent::Hardware(HardwareEvent::Cycles),
            mode: SampleMode::Counting,
            scope: EventScope::Process(0),
            sample_period: 0,
            sample_freq: 1000,
            freq: false,
            exclude_user: false,
            exclude_kernel: false,
            exclude_hv: true,
            exclude_idle: false,
            enable_on_exec: false,
            inherit: false,
            inherit_stat: false,
            sample_id_all: false,
            disabled: true,
        }
    }
}

// ============================================================================
// Performance Counter
// ============================================================================

/// Performance counter handle
pub type PerfFd = u64;

/// Counter state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CounterState {
    /// Disabled
    Disabled,
    /// Enabled and counting
    Enabled,
    /// Paused
    Paused,
}

/// Performance counter
pub struct PerfCounter {
    /// Counter ID
    id: PerfFd,
    /// Attributes
    attr: PerfEventAttr,
    /// Current count
    count: AtomicU64,
    /// Time enabled (nanoseconds)
    time_enabled: AtomicU64,
    /// Time running (nanoseconds)
    time_running: AtomicU64,
    /// State
    state: CounterState,
    /// PMC index (if hardware)
    pmc_index: Option<u8>,
    /// Last TSC value (for time tracking)
    last_tsc: u64,
    /// Overflow count
    overflow_count: AtomicU64,
}

impl PerfCounter {
    /// Create new counter
    fn new(id: PerfFd, attr: PerfEventAttr) -> Self {
        Self {
            id,
            attr,
            count: AtomicU64::new(0),
            time_enabled: AtomicU64::new(0),
            time_running: AtomicU64::new(0),
            state: CounterState::Disabled,
            pmc_index: None,
            last_tsc: 0,
            overflow_count: AtomicU64::new(0),
        }
    }

    /// Read current count
    pub fn read(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    /// Read with time information
    pub fn read_format(&self) -> PerfReadFormat {
        PerfReadFormat {
            value: self.count.load(Ordering::Relaxed),
            time_enabled: self.time_enabled.load(Ordering::Relaxed),
            time_running: self.time_running.load(Ordering::Relaxed),
            id: self.id,
        }
    }

    /// Add to count (for software events)
    pub fn add(&self, delta: u64) {
        self.count.fetch_add(delta, Ordering::Relaxed);
    }

    /// Reset counter
    pub fn reset(&self) {
        self.count.store(0, Ordering::Relaxed);
        self.overflow_count.store(0, Ordering::Relaxed);
    }
}

/// Read format structure
#[derive(Debug, Clone)]
pub struct PerfReadFormat {
    /// Counter value
    pub value: u64,
    /// Time enabled
    pub time_enabled: u64,
    /// Time running
    pub time_running: u64,
    /// Counter ID
    pub id: PerfFd,
}

// ============================================================================
// Sample Record
// ============================================================================

/// Sample record (when sampling mode is enabled)
#[derive(Debug, Clone)]
pub struct PerfSample {
    /// Sample timestamp
    pub time: u64,
    /// Thread ID
    pub tid: u32,
    /// Process ID
    pub pid: u32,
    /// CPU number
    pub cpu: u32,
    /// Instruction pointer
    pub ip: u64,
    /// Period
    pub period: u64,
    /// Counter value
    pub value: u64,
    /// Call chain (stack trace)
    pub callchain: Vec<u64>,
}

/// Sample buffer
pub struct SampleBuffer {
    /// Samples
    samples: Vec<PerfSample>,
    /// Maximum size
    max_size: usize,
    /// Write index
    write_idx: usize,
    /// Overflow count
    overflow: u64,
}

impl SampleBuffer {
    /// Create new buffer
    pub fn new(size: usize) -> Self {
        Self {
            samples: Vec::with_capacity(size),
            max_size: size,
            write_idx: 0,
            overflow: 0,
        }
    }

    /// Add sample
    pub fn push(&mut self, sample: PerfSample) {
        if self.samples.len() < self.max_size {
            self.samples.push(sample);
        } else {
            self.samples[self.write_idx] = sample;
            self.overflow += 1;
        }
        self.write_idx = (self.write_idx + 1) % self.max_size;
    }

    /// Get all samples
    pub fn drain(&mut self) -> Vec<PerfSample> {
        let samples = core::mem::take(&mut self.samples);
        self.write_idx = 0;
        samples
    }

    /// Get sample count
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
}

// ============================================================================
// PMU (Performance Monitoring Unit)
// ============================================================================

/// PMU capabilities
#[derive(Debug, Clone)]
pub struct PmuCapabilities {
    /// Number of general purpose counters
    pub num_counters: u8,
    /// Number of fixed counters
    pub num_fixed: u8,
    /// Counter width (bits)
    pub counter_width: u8,
    /// Supported events bitmap
    pub events_available: u64,
    /// Has branch tracing
    pub has_bts: bool,
    /// Has precise event sampling
    pub has_pebs: bool,
    /// Has last branch records
    pub has_lbr: bool,
    /// Vendor name
    pub vendor: PmuVendor,
}

/// PMU vendor
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PmuVendor {
    Intel,
    Amd,
    Unknown,
}

impl PmuCapabilities {
    /// Detect PMU capabilities
    pub fn detect() -> Self {
        // Use CPUID to detect PMU capabilities
        let vendor = Self::detect_vendor();
        let (num_counters, num_fixed, counter_width) = Self::detect_counters(vendor);

        Self {
            num_counters,
            num_fixed,
            counter_width,
            events_available: Self::detect_events(vendor),
            has_bts: false,
            has_pebs: false,
            has_lbr: false,
            vendor,
        }
    }

    fn detect_vendor() -> PmuVendor {
        // Check CPUID for vendor string
        // Real implementation would read CPUID leaf 0
        PmuVendor::Intel // Default for now
    }

    fn detect_counters(vendor: PmuVendor) -> (u8, u8, u8) {
        match vendor {
            PmuVendor::Intel => {
                // CPUID leaf 0x0A for Intel PMU
                // Typically: 4-8 general, 3 fixed, 48-bit width
                (4, 3, 48)
            }
            PmuVendor::Amd => {
                // AMD typically has 4-6 counters, 48-bit
                (4, 0, 48)
            }
            PmuVendor::Unknown => (2, 0, 48),
        }
    }

    fn detect_events(_vendor: PmuVendor) -> u64 {
        // Return bitmap of supported architectural events
        0x3F // First 6 events supported
    }
}

// ============================================================================
// Perf Subsystem
// ============================================================================

/// Perf subsystem manager
pub struct PerfManager {
    /// PMU capabilities
    capabilities: PmuCapabilities,
    /// Active counters
    counters: BTreeMap<PerfFd, PerfCounter>,
    /// Next counter ID
    next_id: PerfFd,
    /// Sample buffers per CPU
    sample_buffers: Vec<SampleBuffer>,
    /// Software event counters (global)
    sw_counters: SoftwareCounters,
    /// Enabled
    enabled: AtomicBool,
}

/// Global software event counters
struct SoftwareCounters {
    page_faults: AtomicU64,
    context_switches: AtomicU64,
    cpu_migrations: AtomicU64,
    minor_faults: AtomicU64,
    major_faults: AtomicU64,
}

impl Default for SoftwareCounters {
    fn default() -> Self {
        Self {
            page_faults: AtomicU64::new(0),
            context_switches: AtomicU64::new(0),
            cpu_migrations: AtomicU64::new(0),
            minor_faults: AtomicU64::new(0),
            major_faults: AtomicU64::new(0),
        }
    }
}

impl PerfManager {
    /// Create new manager
    pub fn new() -> Self {
        let caps = PmuCapabilities::detect();
        let num_cpus = 1; // TODO: Get actual CPU count

        Self {
            capabilities: caps,
            counters: BTreeMap::new(),
            next_id: 1,
            sample_buffers: (0..num_cpus).map(|_| SampleBuffer::new(4096)).collect(),
            sw_counters: SoftwareCounters::default(),
            enabled: AtomicBool::new(true),
        }
    }

    /// Get PMU capabilities
    pub fn capabilities(&self) -> &PmuCapabilities {
        &self.capabilities
    }

    /// Open a performance counter
    pub fn open(&mut self, attr: PerfEventAttr) -> Result<PerfFd, PerfError> {
        if !self.enabled.load(Ordering::Relaxed) {
            return Err(PerfError::Disabled);
        }

        // Validate event
        match &attr.event {
            PerfEvent::Hardware(hw) => {
                if hw.pmc_select().is_none() {
                    return Err(PerfError::EventNotSupported);
                }
            }
            PerfEvent::Software(_) => {}
            PerfEvent::Cache(_, _, _) => {}
            PerfEvent::Raw(val) => {
                if *val == 0 {
                    return Err(PerfError::InvalidConfig);
                }
            }
            PerfEvent::Tracepoint(_) => {
                // Tracepoints need special handling
            }
        }

        let id = self.next_id;
        self.next_id += 1;

        let counter = PerfCounter::new(id, attr);
        self.counters.insert(id, counter);

        Ok(id)
    }

    /// Close a performance counter
    pub fn close(&mut self, fd: PerfFd) -> Result<(), PerfError> {
        self.counters.remove(&fd)
            .ok_or(PerfError::InvalidFd)?;
        Ok(())
    }

    /// Enable a counter
    pub fn enable(&mut self, fd: PerfFd) -> Result<(), PerfError> {
        let counter = self.counters.get_mut(&fd)
            .ok_or(PerfError::InvalidFd)?;

        if counter.state != CounterState::Disabled {
            return Ok(());
        }

        // For hardware events, program the PMC
        if let PerfEvent::Hardware(hw) = &counter.attr.event {
            if let Some(pmc) = hw.pmc_select() {
                // Would program MSRs here
                let _pmc = pmc;
            }
        }

        counter.state = CounterState::Enabled;
        counter.last_tsc = read_tsc();

        Ok(())
    }

    /// Disable a counter
    pub fn disable(&mut self, fd: PerfFd) -> Result<(), PerfError> {
        let counter = self.counters.get_mut(&fd)
            .ok_or(PerfError::InvalidFd)?;

        if counter.state == CounterState::Disabled {
            return Ok(());
        }

        // Update time tracking
        let now = read_tsc();
        let elapsed = now.saturating_sub(counter.last_tsc);
        counter.time_running.fetch_add(elapsed, Ordering::Relaxed);

        counter.state = CounterState::Disabled;

        Ok(())
    }

    /// Read counter value
    pub fn read(&self, fd: PerfFd) -> Result<PerfReadFormat, PerfError> {
        let counter = self.counters.get(&fd)
            .ok_or(PerfError::InvalidFd)?;

        Ok(counter.read_format())
    }

    /// Reset counter
    pub fn reset(&self, fd: PerfFd) -> Result<(), PerfError> {
        let counter = self.counters.get(&fd)
            .ok_or(PerfError::InvalidFd)?;

        counter.reset();
        Ok(())
    }

    /// Record software event
    pub fn record_sw_event(&self, event: SoftwareEvent) {
        match event {
            SoftwareEvent::PageFaults => {
                self.sw_counters.page_faults.fetch_add(1, Ordering::Relaxed);
            }
            SoftwareEvent::ContextSwitches => {
                self.sw_counters.context_switches.fetch_add(1, Ordering::Relaxed);
            }
            SoftwareEvent::CpuMigrations => {
                self.sw_counters.cpu_migrations.fetch_add(1, Ordering::Relaxed);
            }
            SoftwareEvent::MinorFaults => {
                self.sw_counters.minor_faults.fetch_add(1, Ordering::Relaxed);
            }
            SoftwareEvent::MajorFaults => {
                self.sw_counters.major_faults.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }

        // Update any software event counters
        for counter in self.counters.values() {
            if counter.state == CounterState::Enabled {
                if let PerfEvent::Software(sw) = &counter.attr.event {
                    if *sw == event {
                        counter.add(1);
                    }
                }
            }
        }
    }

    /// Get software event count
    pub fn get_sw_count(&self, event: SoftwareEvent) -> u64 {
        match event {
            SoftwareEvent::PageFaults => self.sw_counters.page_faults.load(Ordering::Relaxed),
            SoftwareEvent::ContextSwitches => self.sw_counters.context_switches.load(Ordering::Relaxed),
            SoftwareEvent::CpuMigrations => self.sw_counters.cpu_migrations.load(Ordering::Relaxed),
            SoftwareEvent::MinorFaults => self.sw_counters.minor_faults.load(Ordering::Relaxed),
            SoftwareEvent::MajorFaults => self.sw_counters.major_faults.load(Ordering::Relaxed),
            _ => 0,
        }
    }

    /// Get active counter count
    pub fn active_count(&self) -> usize {
        self.counters.values()
            .filter(|c| c.state == CounterState::Enabled)
            .count()
    }

    /// List all counters
    pub fn list_counters(&self) -> Vec<PerfFd> {
        self.counters.keys().copied().collect()
    }
}

impl Default for PerfManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Error Types
// ============================================================================

/// Perf error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerfError {
    /// Invalid file descriptor
    InvalidFd,
    /// Invalid configuration
    InvalidConfig,
    /// Event not supported
    EventNotSupported,
    /// Too many open counters
    TooManyCounters,
    /// Permission denied
    PermissionDenied,
    /// Perf subsystem disabled
    Disabled,
    /// No hardware support
    NoHardwareSupport,
    /// Busy (counter in use)
    Busy,
}

// ============================================================================
// Helpers
// ============================================================================

/// Read TSC (Time Stamp Counter)
fn read_tsc() -> u64 {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        core::arch::x86_64::_rdtsc()
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        0
    }
}

// ============================================================================
// Global Instance
// ============================================================================

static PERF_MANAGER: Once<Mutex<PerfManager>> = Once::new();

/// Initialize perf subsystem
pub fn init() {
    PERF_MANAGER.call_once(|| Mutex::new(PerfManager::new()));

    let caps = PERF_MANAGER.get().unwrap().lock().capabilities().clone();
    crate::kprintln!(
        "perf: initialized ({:?}, {} GP + {} fixed counters, {}-bit)",
        caps.vendor,
        caps.num_counters,
        caps.num_fixed,
        caps.counter_width
    );
}

/// Get perf manager
pub fn manager() -> &'static Mutex<PerfManager> {
    PERF_MANAGER.get().expect("Perf not initialized")
}

/// Open a counter (convenience function)
pub fn perf_event_open(attr: PerfEventAttr) -> Result<PerfFd, PerfError> {
    manager().lock().open(attr)
}

/// Close a counter
pub fn perf_event_close(fd: PerfFd) -> Result<(), PerfError> {
    manager().lock().close(fd)
}

/// Enable a counter
pub fn perf_event_enable(fd: PerfFd) -> Result<(), PerfError> {
    manager().lock().enable(fd)
}

/// Disable a counter
pub fn perf_event_disable(fd: PerfFd) -> Result<(), PerfError> {
    manager().lock().disable(fd)
}

/// Read a counter
pub fn perf_event_read(fd: PerfFd) -> Result<PerfReadFormat, PerfError> {
    manager().lock().read(fd)
}

/// Reset a counter
pub fn perf_event_reset(fd: PerfFd) -> Result<(), PerfError> {
    manager().lock().reset(fd)
}

/// Record software event
pub fn perf_sw_event(event: SoftwareEvent) {
    if let Some(mgr) = PERF_MANAGER.get() {
        mgr.lock().record_sw_event(event);
    }
}

/// Quick cycles count (for benchmarking)
pub fn perf_cycles() -> u64 {
    read_tsc()
}

/// Measure cycles for a closure
pub fn measure_cycles<F, R>(f: F) -> (R, u64)
where
    F: FnOnce() -> R,
{
    let start = read_tsc();
    let result = f();
    let end = read_tsc();
    (result, end.saturating_sub(start))
}
