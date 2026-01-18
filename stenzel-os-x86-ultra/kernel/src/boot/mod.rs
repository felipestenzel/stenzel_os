//! Boot optimization for fast boot (<10 seconds)
//!
//! Implements various optimizations to achieve sub-10-second boot times:
//! - Lazy initialization of non-critical services
//! - Parallel device probing
//! - Boot time measurement and profiling
//! - Service dependency optimization
//! - Deferred driver initialization

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use crate::sync::IrqSafeMutex;

/// Boot stage for timing
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BootStage {
    /// Bootloader handoff
    BootloaderHandoff = 0,
    /// Early serial/console init
    EarlyConsole = 1,
    /// Memory management init
    MemoryInit = 2,
    /// Architecture init (GDT, IDT, etc.)
    ArchInit = 3,
    /// ACPI parsing
    AcpiInit = 4,
    /// APIC/interrupt init
    InterruptInit = 5,
    /// Timer init (HPET, TSC)
    TimerInit = 6,
    /// Basic drivers (keyboard, mouse)
    BasicDrivers = 7,
    /// Storage init
    StorageInit = 8,
    /// Filesystem mount
    FilesystemMount = 9,
    /// Network init
    NetworkInit = 10,
    /// USB enumeration
    UsbInit = 11,
    /// Security init
    SecurityInit = 12,
    /// Scheduler start
    SchedulerStart = 13,
    /// Init process spawn
    InitSpawn = 14,
    /// Desktop ready
    DesktopReady = 15,
    /// Boot complete
    BootComplete = 16,
}

impl BootStage {
    pub fn name(&self) -> &'static str {
        match self {
            BootStage::BootloaderHandoff => "Bootloader Handoff",
            BootStage::EarlyConsole => "Early Console",
            BootStage::MemoryInit => "Memory Init",
            BootStage::ArchInit => "Architecture Init",
            BootStage::AcpiInit => "ACPI Init",
            BootStage::InterruptInit => "Interrupt Init",
            BootStage::TimerInit => "Timer Init",
            BootStage::BasicDrivers => "Basic Drivers",
            BootStage::StorageInit => "Storage Init",
            BootStage::FilesystemMount => "Filesystem Mount",
            BootStage::NetworkInit => "Network Init",
            BootStage::UsbInit => "USB Init",
            BootStage::SecurityInit => "Security Init",
            BootStage::SchedulerStart => "Scheduler Start",
            BootStage::InitSpawn => "Init Spawn",
            BootStage::DesktopReady => "Desktop Ready",
            BootStage::BootComplete => "Boot Complete",
        }
    }

    /// Check if this stage is critical (must complete before user interaction)
    pub fn is_critical(&self) -> bool {
        matches!(self,
            BootStage::BootloaderHandoff |
            BootStage::EarlyConsole |
            BootStage::MemoryInit |
            BootStage::ArchInit |
            BootStage::InterruptInit |
            BootStage::SchedulerStart |
            BootStage::InitSpawn
        )
    }

    /// Check if this stage can be deferred
    pub fn can_defer(&self) -> bool {
        matches!(self,
            BootStage::UsbInit |
            BootStage::NetworkInit |
            BootStage::SecurityInit
        )
    }
}

/// Boot time measurement
#[derive(Debug, Clone, Copy)]
pub struct BootTiming {
    /// Stage
    pub stage: BootStage,
    /// Start timestamp (TSC cycles or microseconds)
    pub start_us: u64,
    /// End timestamp
    pub end_us: u64,
}

impl BootTiming {
    pub fn duration_us(&self) -> u64 {
        self.end_us.saturating_sub(self.start_us)
    }

    pub fn duration_ms(&self) -> u64 {
        self.duration_us() / 1000
    }
}

/// Boot profiler
pub struct BootProfiler {
    /// Timing records
    timings: Vec<BootTiming>,
    /// Current stage start
    current_stage: Option<(BootStage, u64)>,
    /// Boot start timestamp
    boot_start_us: u64,
    /// Boot complete flag
    boot_complete: bool,
}

impl BootProfiler {
    pub const fn new() -> Self {
        Self {
            timings: Vec::new(),
            current_stage: None,
            boot_start_us: 0,
            boot_complete: false,
        }
    }

    /// Initialize with boot start time
    pub fn init(&mut self, start_us: u64) {
        self.boot_start_us = start_us;
    }

    /// Start timing a stage
    pub fn start_stage(&mut self, stage: BootStage, timestamp_us: u64) {
        // End previous stage if any
        if let Some((prev_stage, start)) = self.current_stage.take() {
            self.timings.push(BootTiming {
                stage: prev_stage,
                start_us: start,
                end_us: timestamp_us,
            });
        }
        self.current_stage = Some((stage, timestamp_us));
    }

    /// End current stage
    pub fn end_stage(&mut self, timestamp_us: u64) {
        if let Some((stage, start)) = self.current_stage.take() {
            self.timings.push(BootTiming {
                stage,
                start_us: start,
                end_us: timestamp_us,
            });
        }
    }

    /// Mark boot as complete
    pub fn complete(&mut self, timestamp_us: u64) {
        self.end_stage(timestamp_us);
        self.boot_complete = true;
    }

    /// Get total boot time in milliseconds
    pub fn total_boot_time_ms(&self) -> u64 {
        if let Some(last) = self.timings.last() {
            (last.end_us - self.boot_start_us) / 1000
        } else {
            0
        }
    }

    /// Get all timings
    pub fn timings(&self) -> &[BootTiming] {
        &self.timings
    }

    /// Generate boot timing report
    pub fn report(&self) -> String {
        let mut s = String::from("=== Boot Timing Report ===\n");
        let total = self.total_boot_time_ms();

        s.push_str(&alloc::format!("Total boot time: {} ms\n\n", total));
        s.push_str("Stage breakdown:\n");

        for timing in &self.timings {
            let duration_ms = timing.duration_ms();
            let pct = if total > 0 { (duration_ms * 100) / total } else { 0 };
            s.push_str(&alloc::format!(
                "  {:20} {:6} ms ({:2}%)\n",
                timing.stage.name(),
                duration_ms,
                pct
            ));
        }

        s
    }
}

/// Service priority for initialization
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ServicePriority {
    /// Critical - must init immediately
    Critical = 0,
    /// High - init early, but not blocking
    High = 1,
    /// Normal - init in normal boot sequence
    Normal = 2,
    /// Low - can be deferred
    Low = 3,
    /// Background - init after desktop ready
    Background = 4,
}

/// Service state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceState {
    /// Not started
    Pending,
    /// Currently initializing
    Initializing,
    /// Ready
    Ready,
    /// Failed
    Failed,
    /// Deferred for later
    Deferred,
}

/// Service descriptor for parallel init
pub struct ServiceDescriptor {
    /// Service name
    pub name: String,
    /// Priority
    pub priority: ServicePriority,
    /// Dependencies (service names)
    pub dependencies: Vec<String>,
    /// State
    pub state: ServiceState,
    /// Init function (called when ready)
    pub init_fn: Option<fn() -> Result<(), &'static str>>,
    /// Estimated init time in ms
    pub estimated_time_ms: u32,
    /// Can run in parallel
    pub parallelizable: bool,
}

impl ServiceDescriptor {
    pub fn new(name: &str, priority: ServicePriority) -> Self {
        Self {
            name: name.to_string(),
            priority,
            dependencies: Vec::new(),
            state: ServiceState::Pending,
            init_fn: None,
            estimated_time_ms: 100,
            parallelizable: true,
        }
    }

    pub fn with_dependency(mut self, dep: &str) -> Self {
        self.dependencies.push(dep.to_string());
        self
    }

    pub fn with_init(mut self, f: fn() -> Result<(), &'static str>) -> Self {
        self.init_fn = Some(f);
        self
    }

    pub fn with_estimated_time(mut self, ms: u32) -> Self {
        self.estimated_time_ms = ms;
        self
    }

    pub fn not_parallelizable(mut self) -> Self {
        self.parallelizable = false;
        self
    }
}

/// Parallel service initializer
pub struct ParallelInitializer {
    /// Services to initialize
    services: BTreeMap<String, ServiceDescriptor>,
    /// Initialization order (computed from dependencies)
    init_order: Vec<String>,
    /// Currently initializing services
    active: Vec<String>,
    /// Max parallel services
    max_parallel: usize,
}

impl ParallelInitializer {
    pub fn new(max_parallel: usize) -> Self {
        Self {
            services: BTreeMap::new(),
            init_order: Vec::new(),
            active: Vec::new(),
            max_parallel,
        }
    }

    /// Register a service
    pub fn register(&mut self, service: ServiceDescriptor) {
        self.services.insert(service.name.clone(), service);
    }

    /// Compute initialization order using topological sort
    pub fn compute_order(&mut self) -> Result<(), &'static str> {
        // Kahn's algorithm for topological sort
        let mut in_degree: BTreeMap<String, usize> = BTreeMap::new();
        let mut queue: Vec<String> = Vec::new();

        // Calculate in-degrees
        for (name, _) in &self.services {
            in_degree.insert(name.clone(), 0);
        }

        for (_, service) in &self.services {
            for dep in &service.dependencies {
                if let Some(count) = in_degree.get_mut(dep) {
                    *count += 1;
                }
            }
        }

        // Find services with no dependencies
        for (name, &degree) in &in_degree {
            if degree == 0 {
                queue.push(name.clone());
            }
        }

        // Sort by priority within same dependency level
        queue.sort_by(|a, b| {
            let pa = self.services.get(a).map(|s| s.priority).unwrap_or(ServicePriority::Normal);
            let pb = self.services.get(b).map(|s| s.priority).unwrap_or(ServicePriority::Normal);
            pa.cmp(&pb)
        });

        self.init_order.clear();

        while let Some(name) = queue.pop() {
            self.init_order.push(name.clone());

            // Reduce in-degree of dependents
            if let Some(service) = self.services.get(&name) {
                for dep in &service.dependencies {
                    if let Some(count) = in_degree.get_mut(dep) {
                        *count = count.saturating_sub(1);
                        if *count == 0 {
                            queue.push(dep.clone());
                        }
                    }
                }
            }
        }

        if self.init_order.len() != self.services.len() {
            return Err("Circular dependency detected");
        }

        Ok(())
    }

    /// Check if service can start (all dependencies ready)
    pub fn can_start(&self, name: &str) -> bool {
        if let Some(service) = self.services.get(name) {
            if service.state != ServiceState::Pending {
                return false;
            }

            for dep in &service.dependencies {
                if let Some(dep_service) = self.services.get(dep) {
                    if dep_service.state != ServiceState::Ready {
                        return false;
                    }
                }
            }

            true
        } else {
            false
        }
    }

    /// Get next services that can be initialized in parallel
    pub fn get_ready_services(&self) -> Vec<String> {
        let available_slots = self.max_parallel.saturating_sub(self.active.len());
        if available_slots == 0 {
            return Vec::new();
        }

        let mut ready = Vec::new();

        for name in &self.init_order {
            if ready.len() >= available_slots {
                break;
            }

            if self.can_start(name) {
                if let Some(service) = self.services.get(name) {
                    if service.parallelizable || self.active.is_empty() {
                        ready.push(name.clone());
                    }
                }
            }
        }

        ready
    }

    /// Mark service as started
    pub fn mark_started(&mut self, name: &str) {
        if let Some(service) = self.services.get_mut(name) {
            service.state = ServiceState::Initializing;
            self.active.push(name.to_string());
        }
    }

    /// Mark service as complete
    pub fn mark_complete(&mut self, name: &str, success: bool) {
        if let Some(service) = self.services.get_mut(name) {
            service.state = if success {
                ServiceState::Ready
            } else {
                ServiceState::Failed
            };
        }
        self.active.retain(|n| n != name);
    }

    /// Check if all services are done
    pub fn all_done(&self) -> bool {
        self.services.values().all(|s| {
            matches!(s.state, ServiceState::Ready | ServiceState::Failed | ServiceState::Deferred)
        })
    }

    /// Get initialization progress (0-100)
    pub fn progress(&self) -> u8 {
        let total = self.services.len();
        if total == 0 {
            return 100;
        }

        let done = self.services.values().filter(|s| {
            matches!(s.state, ServiceState::Ready | ServiceState::Failed | ServiceState::Deferred)
        }).count();

        ((done * 100) / total) as u8
    }
}

/// Lazy initialization wrapper
pub struct LazyInit<T> {
    /// Value once initialized
    value: Option<T>,
    /// Initialization function
    init_fn: Option<fn() -> T>,
    /// Initialized flag
    initialized: AtomicBool,
}

impl<T> LazyInit<T> {
    pub const fn new(init_fn: fn() -> T) -> Self {
        Self {
            value: None,
            init_fn: Some(init_fn),
            initialized: AtomicBool::new(false),
        }
    }

    /// Get value, initializing if needed
    pub fn get(&mut self) -> &T {
        if !self.initialized.load(Ordering::Acquire) {
            if let Some(f) = self.init_fn.take() {
                self.value = Some(f());
                self.initialized.store(true, Ordering::Release);
            }
        }
        self.value.as_ref().unwrap()
    }

    /// Get value without initializing
    pub fn try_get(&self) -> Option<&T> {
        if self.initialized.load(Ordering::Acquire) {
            self.value.as_ref()
        } else {
            None
        }
    }

    /// Check if initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::Acquire)
    }
}

/// Deferred task for post-boot initialization
pub struct DeferredTask {
    /// Task name
    pub name: String,
    /// Task function
    pub task_fn: fn(),
    /// Priority (lower = sooner)
    pub priority: u8,
    /// Delay after boot (ms)
    pub delay_ms: u32,
}

/// Deferred task manager
pub struct DeferredTaskManager {
    /// Pending tasks
    tasks: Vec<DeferredTask>,
    /// Boot complete time
    boot_complete_time: Option<u64>,
}

impl DeferredTaskManager {
    pub const fn new() -> Self {
        Self {
            tasks: Vec::new(),
            boot_complete_time: None,
        }
    }

    /// Add a deferred task
    pub fn add(&mut self, task: DeferredTask) {
        self.tasks.push(task);
        // Sort by priority then delay
        self.tasks.sort_by(|a, b| {
            a.priority.cmp(&b.priority).then(a.delay_ms.cmp(&b.delay_ms))
        });
    }

    /// Mark boot as complete
    pub fn boot_complete(&mut self, timestamp_us: u64) {
        self.boot_complete_time = Some(timestamp_us);
    }

    /// Get tasks ready to run
    pub fn get_ready_tasks(&mut self, current_time_us: u64) -> Vec<DeferredTask> {
        let boot_time = match self.boot_complete_time {
            Some(t) => t,
            None => return Vec::new(),
        };

        let elapsed_ms = (current_time_us.saturating_sub(boot_time)) / 1000;

        let mut ready = Vec::new();
        self.tasks.retain(|task| {
            if (task.delay_ms as u64) <= elapsed_ms {
                ready.push(DeferredTask {
                    name: task.name.clone(),
                    task_fn: task.task_fn,
                    priority: task.priority,
                    delay_ms: task.delay_ms,
                });
                false
            } else {
                true
            }
        });

        ready
    }
}

/// Boot configuration for fast boot
#[derive(Debug, Clone)]
pub struct FastBootConfig {
    /// Target boot time in ms
    pub target_boot_time_ms: u32,
    /// Enable parallel device probing
    pub parallel_device_probe: bool,
    /// Max parallel probe threads
    pub max_parallel_probes: u8,
    /// Enable lazy USB enumeration
    pub lazy_usb: bool,
    /// Enable lazy network init
    pub lazy_network: bool,
    /// Enable deferred service start
    pub deferred_services: bool,
    /// Show boot splash during init
    pub show_boot_splash: bool,
    /// Skip non-essential checks
    pub fast_mode: bool,
    /// Cache ACPI tables
    pub cache_acpi: bool,
    /// Preload common drivers
    pub preload_drivers: bool,
}

impl Default for FastBootConfig {
    fn default() -> Self {
        Self {
            target_boot_time_ms: 10000, // 10 second target
            parallel_device_probe: true,
            max_parallel_probes: 4,
            lazy_usb: true,
            lazy_network: true,
            deferred_services: true,
            show_boot_splash: true,
            fast_mode: false,
            cache_acpi: true,
            preload_drivers: true,
        }
    }
}

impl FastBootConfig {
    /// Ultra-fast boot configuration (minimal init)
    pub fn ultra_fast() -> Self {
        Self {
            target_boot_time_ms: 5000,
            parallel_device_probe: true,
            max_parallel_probes: 8,
            lazy_usb: true,
            lazy_network: true,
            deferred_services: true,
            show_boot_splash: false,
            fast_mode: true,
            cache_acpi: true,
            preload_drivers: false,
        }
    }

    /// Compatible boot configuration (safe defaults)
    pub fn compatible() -> Self {
        Self {
            target_boot_time_ms: 30000,
            parallel_device_probe: false,
            max_parallel_probes: 1,
            lazy_usb: false,
            lazy_network: false,
            deferred_services: false,
            show_boot_splash: true,
            fast_mode: false,
            cache_acpi: false,
            preload_drivers: false,
        }
    }
}

/// Global boot profiler
static BOOT_PROFILER: IrqSafeMutex<BootProfiler> = IrqSafeMutex::new(BootProfiler::new());

/// Global deferred task manager
static DEFERRED_TASKS: IrqSafeMutex<DeferredTaskManager> = IrqSafeMutex::new(DeferredTaskManager::new());

/// Global fast boot config
static FAST_BOOT_CONFIG: IrqSafeMutex<FastBootConfig> = IrqSafeMutex::new(FastBootConfig {
    target_boot_time_ms: 10000,
    parallel_device_probe: true,
    max_parallel_probes: 4,
    lazy_usb: true,
    lazy_network: true,
    deferred_services: true,
    show_boot_splash: true,
    fast_mode: false,
    cache_acpi: true,
    preload_drivers: true,
});

/// Boot complete flag
static BOOT_COMPLETE: AtomicBool = AtomicBool::new(false);

/// Boot start timestamp
static BOOT_START_US: AtomicU64 = AtomicU64::new(0);

/// Initialize boot profiler
pub fn init(start_us: u64) {
    BOOT_START_US.store(start_us, Ordering::Release);
    BOOT_PROFILER.lock().init(start_us);
}

/// Get current timestamp in microseconds (fallback if TSC not calibrated)
fn get_timestamp_us() -> u64 {
    // Try TSC first if enabled
    if crate::arch::tsc::is_enabled() {
        return crate::arch::tsc::now_us();
    }

    // Fallback to RTC-based time
    let ts = crate::time::realtime();
    (ts.tv_sec as u64) * 1_000_000 + (ts.tv_nsec as u64) / 1000
}

/// Start timing a boot stage
pub fn start_stage(stage: BootStage) {
    let ts = get_timestamp_us();
    BOOT_PROFILER.lock().start_stage(stage, ts);
}

/// End current boot stage
pub fn end_stage() {
    let ts = get_timestamp_us();
    BOOT_PROFILER.lock().end_stage(ts);
}

/// Mark boot as complete
pub fn boot_complete() {
    let ts = get_timestamp_us();
    BOOT_PROFILER.lock().complete(ts);
    DEFERRED_TASKS.lock().boot_complete(ts);
    BOOT_COMPLETE.store(true, Ordering::Release);

    // Log boot time
    let boot_time_ms = get_boot_time_ms();
    crate::util::kprintln!("boot: completed in {} ms", boot_time_ms);
}

/// Check if boot is complete
pub fn is_boot_complete() -> bool {
    BOOT_COMPLETE.load(Ordering::Acquire)
}

/// Get total boot time in milliseconds
pub fn get_boot_time_ms() -> u64 {
    BOOT_PROFILER.lock().total_boot_time_ms()
}

/// Get boot timing report
pub fn get_boot_report() -> String {
    BOOT_PROFILER.lock().report()
}

/// Get fast boot configuration
pub fn config() -> FastBootConfig {
    FAST_BOOT_CONFIG.lock().clone()
}

/// Set fast boot configuration
pub fn set_config(config: FastBootConfig) {
    *FAST_BOOT_CONFIG.lock() = config;
}

/// Register a deferred task
pub fn defer_task(name: &str, task_fn: fn(), delay_ms: u32, priority: u8) {
    DEFERRED_TASKS.lock().add(DeferredTask {
        name: name.to_string(),
        task_fn,
        priority,
        delay_ms,
    });
}

/// Process ready deferred tasks
pub fn process_deferred_tasks() {
    let ts = get_timestamp_us();
    let ready = DEFERRED_TASKS.lock().get_ready_tasks(ts);

    for task in ready {
        crate::util::kprintln!("boot: running deferred task '{}'", task.name);
        (task.task_fn)();
    }
}

/// Check if we should defer a service based on config
pub fn should_defer(service: &str) -> bool {
    let config = FAST_BOOT_CONFIG.lock();

    if !config.deferred_services {
        return false;
    }

    match service {
        "usb" | "usb_enumeration" => config.lazy_usb,
        "network" | "wifi" | "dhcp" => config.lazy_network,
        "bluetooth" => true, // Always defer bluetooth
        "audio" => true, // Always defer audio
        _ => false,
    }
}

/// Quick boot check - are we in fast mode?
pub fn is_fast_mode() -> bool {
    FAST_BOOT_CONFIG.lock().fast_mode
}
