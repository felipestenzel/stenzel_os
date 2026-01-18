//! Function Tracing (ftrace) Subsystem for Stenzel OS.
//!
//! Provides kernel function tracing and event tracing capabilities.
//!
//! Features:
//! - Function entry/exit tracing
//! - Function graph tracing
//! - Event tracing (tracepoints)
//! - Per-CPU trace buffers
//! - Trace filters
//! - Trace triggers
//! - Stack traces
//! - Latency tracing

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use spin::{Mutex, Once, RwLock};

// ============================================================================
// Trace Entry Types
// ============================================================================

/// Trace entry type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceEntryType {
    /// Function entry
    FuncEntry,
    /// Function exit
    FuncExit,
    /// Function graph entry
    GraphEntry,
    /// Function graph exit
    GraphExit,
    /// Custom tracepoint
    Tracepoint,
    /// Print event
    Print,
    /// Wakeup event
    Wakeup,
    /// Schedule event
    Sched,
    /// IRQ entry
    IrqEntry,
    /// IRQ exit
    IrqExit,
    /// Softirq entry
    SoftirqEntry,
    /// Softirq exit
    SoftirqExit,
    /// Syscall entry
    SyscallEntry,
    /// Syscall exit
    SyscallExit,
    /// Marker (user-defined)
    Marker,
    /// Stack trace
    StackTrace,
}

/// Trace entry header
#[derive(Debug, Clone)]
pub struct TraceEntryHeader {
    /// Entry type
    pub entry_type: TraceEntryType,
    /// Timestamp (nanoseconds)
    pub timestamp: u64,
    /// CPU number
    pub cpu: u32,
    /// Process ID
    pub pid: u32,
    /// Thread ID
    pub tid: u32,
    /// Preempt count
    pub preempt_count: u8,
    /// IRQ flags
    pub irq_flags: u8,
}

impl TraceEntryHeader {
    /// Create new header
    pub fn new(entry_type: TraceEntryType, cpu: u32, pid: u32) -> Self {
        Self {
            entry_type,
            timestamp: crate::time::uptime_ns(),
            cpu,
            pid,
            tid: pid, // Simplified
            preempt_count: 0,
            irq_flags: 0,
        }
    }
}

/// Function trace entry
#[derive(Debug, Clone)]
pub struct FuncTraceEntry {
    /// Header
    pub header: TraceEntryHeader,
    /// Function address
    pub func_addr: u64,
    /// Parent address (caller)
    pub parent_addr: u64,
    /// Function name (if resolved)
    pub func_name: Option<String>,
}

/// Function graph entry (with timing)
#[derive(Debug, Clone)]
pub struct GraphTraceEntry {
    /// Header
    pub header: TraceEntryHeader,
    /// Function address
    pub func_addr: u64,
    /// Call depth
    pub depth: u32,
    /// Duration (for exit)
    pub duration_ns: u64,
    /// Flags
    pub flags: u8,
}

/// Tracepoint entry
#[derive(Debug, Clone)]
pub struct TracepointEntry {
    /// Header
    pub header: TraceEntryHeader,
    /// Tracepoint name
    pub name: String,
    /// Subsystem
    pub subsystem: String,
    /// Data
    pub data: Vec<u8>,
}

/// Print trace entry
#[derive(Debug, Clone)]
pub struct PrintTraceEntry {
    /// Header
    pub header: TraceEntryHeader,
    /// Message
    pub message: String,
}

/// Stack trace entry
#[derive(Debug, Clone)]
pub struct StackTraceEntry {
    /// Header
    pub header: TraceEntryHeader,
    /// Stack frames (addresses)
    pub frames: Vec<u64>,
}

/// IRQ trace entry
#[derive(Debug, Clone)]
pub struct IrqTraceEntry {
    /// Header
    pub header: TraceEntryHeader,
    /// IRQ number
    pub irq: u32,
    /// Handler name
    pub handler: String,
}

/// Schedule trace entry
#[derive(Debug, Clone)]
pub struct SchedTraceEntry {
    /// Header
    pub header: TraceEntryHeader,
    /// Previous PID
    pub prev_pid: u32,
    /// Previous task name
    pub prev_comm: String,
    /// Previous priority
    pub prev_prio: i32,
    /// Previous state
    pub prev_state: u8,
    /// Next PID
    pub next_pid: u32,
    /// Next task name
    pub next_comm: String,
    /// Next priority
    pub next_prio: i32,
}

/// Syscall trace entry
#[derive(Debug, Clone)]
pub struct SyscallTraceEntry {
    /// Header
    pub header: TraceEntryHeader,
    /// Syscall number
    pub syscall_nr: u64,
    /// Arguments (up to 6)
    pub args: [u64; 6],
    /// Return value (for exit)
    pub ret: i64,
}

/// Generic trace entry
#[derive(Debug, Clone)]
pub enum TraceEntry {
    Func(FuncTraceEntry),
    Graph(GraphTraceEntry),
    Tracepoint(TracepointEntry),
    Print(PrintTraceEntry),
    Stack(StackTraceEntry),
    Irq(IrqTraceEntry),
    Sched(SchedTraceEntry),
    Syscall(SyscallTraceEntry),
}

// ============================================================================
// Trace Buffer
// ============================================================================

/// Per-CPU trace buffer
pub struct TraceBuffer {
    /// Buffer ID (CPU number)
    id: u32,
    /// Entries
    entries: Vec<TraceEntry>,
    /// Maximum size
    max_size: usize,
    /// Write index
    write_idx: AtomicUsize,
    /// Overrun count
    overruns: AtomicU64,
    /// Enabled
    enabled: AtomicBool,
}

impl TraceBuffer {
    /// Create new buffer
    pub fn new(id: u32, size: usize) -> Self {
        Self {
            id,
            entries: Vec::with_capacity(size),
            max_size: size,
            write_idx: AtomicUsize::new(0),
            overruns: AtomicU64::new(0),
            enabled: AtomicBool::new(true),
        }
    }

    /// Write entry to buffer
    pub fn write(&mut self, entry: TraceEntry) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }

        let idx = self.write_idx.load(Ordering::Relaxed);

        if self.entries.len() < self.max_size {
            self.entries.push(entry);
        } else {
            self.entries[idx % self.max_size] = entry;
            self.overruns.fetch_add(1, Ordering::Relaxed);
        }

        self.write_idx.fetch_add(1, Ordering::Relaxed);
    }

    /// Read all entries
    pub fn read_all(&self) -> Vec<TraceEntry> {
        self.entries.clone()
    }

    /// Clear buffer
    pub fn clear(&mut self) {
        self.entries.clear();
        self.write_idx.store(0, Ordering::Relaxed);
    }

    /// Get entry count
    pub fn len(&self) -> usize {
        self.entries.len().min(self.max_size)
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get overrun count
    pub fn overruns(&self) -> u64 {
        self.overruns.load(Ordering::Relaxed)
    }

    /// Enable/disable
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }
}

// ============================================================================
// Tracepoints
// ============================================================================

/// Tracepoint definition
#[derive(Debug)]
pub struct Tracepoint {
    /// Name
    pub name: String,
    /// Subsystem
    pub subsystem: String,
    /// Format string
    pub format: String,
    /// Enabled
    enabled: AtomicBool,
    /// Hit count
    hit_count: AtomicU64,
}

impl Tracepoint {
    /// Create new tracepoint
    pub fn new(subsystem: &str, name: &str, format: &str) -> Self {
        Self {
            name: name.to_string(),
            subsystem: subsystem.to_string(),
            format: format.to_string(),
            enabled: AtomicBool::new(false),
            hit_count: AtomicU64::new(0),
        }
    }

    /// Enable tracepoint
    pub fn enable(&self) {
        self.enabled.store(true, Ordering::Relaxed);
    }

    /// Disable tracepoint
    pub fn disable(&self) {
        self.enabled.store(false, Ordering::Relaxed);
    }

    /// Check if enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Record hit
    pub fn hit(&self) {
        self.hit_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Get hit count
    pub fn hit_count(&self) -> u64 {
        self.hit_count.load(Ordering::Relaxed)
    }

    /// Get full name (subsystem:name)
    pub fn full_name(&self) -> String {
        format!("{}:{}", self.subsystem, self.name)
    }
}

// ============================================================================
// Trace Filters
// ============================================================================

/// Filter type
#[derive(Debug, Clone)]
pub enum TraceFilter {
    /// Filter by function name prefix
    FuncPrefix(String),
    /// Filter by function name suffix
    FuncSuffix(String),
    /// Filter by function name contains
    FuncContains(String),
    /// Filter by PID
    Pid(u32),
    /// Filter by CPU
    Cpu(u32),
    /// Filter by subsystem
    Subsystem(String),
    /// Filter by module
    Module(String),
    /// No filter (all pass)
    None,
}

impl TraceFilter {
    /// Check if entry passes filter
    pub fn matches(&self, entry: &TraceEntry) -> bool {
        match (self, entry) {
            (TraceFilter::None, _) => true,
            (TraceFilter::Pid(pid), entry) => {
                let header = match entry {
                    TraceEntry::Func(e) => &e.header,
                    TraceEntry::Graph(e) => &e.header,
                    TraceEntry::Tracepoint(e) => &e.header,
                    TraceEntry::Print(e) => &e.header,
                    TraceEntry::Stack(e) => &e.header,
                    TraceEntry::Irq(e) => &e.header,
                    TraceEntry::Sched(e) => &e.header,
                    TraceEntry::Syscall(e) => &e.header,
                };
                header.pid == *pid
            }
            (TraceFilter::Cpu(cpu), entry) => {
                let header = match entry {
                    TraceEntry::Func(e) => &e.header,
                    TraceEntry::Graph(e) => &e.header,
                    TraceEntry::Tracepoint(e) => &e.header,
                    TraceEntry::Print(e) => &e.header,
                    TraceEntry::Stack(e) => &e.header,
                    TraceEntry::Irq(e) => &e.header,
                    TraceEntry::Sched(e) => &e.header,
                    TraceEntry::Syscall(e) => &e.header,
                };
                header.cpu == *cpu
            }
            (TraceFilter::FuncPrefix(prefix), TraceEntry::Func(e)) => {
                e.func_name.as_ref().map(|n| n.starts_with(prefix)).unwrap_or(false)
            }
            (TraceFilter::Subsystem(subsys), TraceEntry::Tracepoint(e)) => {
                e.subsystem == *subsys
            }
            _ => true,
        }
    }
}

// ============================================================================
// Tracers
// ============================================================================

/// Tracer type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TracerType {
    /// No tracer (events only)
    Nop,
    /// Function tracer
    Function,
    /// Function graph tracer
    FunctionGraph,
    /// IRQ off tracer
    IrqsOff,
    /// Preempt off tracer
    PreemptOff,
    /// Wakeup tracer
    Wakeup,
    /// Wakeup real-time tracer
    WakeupRt,
    /// Hardware latency tracer
    HwLat,
    /// Block tracer
    Blk,
}

/// Tracer configuration
#[derive(Debug, Clone)]
pub struct TracerConfig {
    /// Tracer type
    pub tracer: TracerType,
    /// Function filter
    pub func_filter: Option<String>,
    /// No-trace filter
    pub notrace_filter: Option<String>,
    /// Graph depth limit
    pub graph_depth: u32,
    /// Graph time (show time)
    pub graph_time: bool,
    /// Options
    pub options: TraceOptions,
}

impl Default for TracerConfig {
    fn default() -> Self {
        Self {
            tracer: TracerType::Nop,
            func_filter: None,
            notrace_filter: None,
            graph_depth: 10,
            graph_time: true,
            options: TraceOptions::default(),
        }
    }
}

/// Trace options
#[derive(Debug, Clone)]
pub struct TraceOptions {
    /// Print parent function
    pub print_parent: bool,
    /// Print symbols
    pub sym_offset: bool,
    /// Print symbol address
    pub sym_addr: bool,
    /// Print verbose
    pub verbose: bool,
    /// Print raw
    pub raw: bool,
    /// Print hex
    pub hex: bool,
    /// Print binary
    pub bin: bool,
    /// Print block
    pub block: bool,
    /// Stack trace on event
    pub stacktrace: bool,
    /// Use trace marker
    pub markers: bool,
    /// Record on context switch
    pub context_info: bool,
    /// Latency format
    pub latency_format: bool,
    /// Function fork
    pub function_fork: bool,
    /// Display graph
    pub display_graph: bool,
    /// IRQs info
    pub irq_info: bool,
}

impl Default for TraceOptions {
    fn default() -> Self {
        Self {
            print_parent: true,
            sym_offset: false,
            sym_addr: false,
            verbose: false,
            raw: false,
            hex: false,
            bin: false,
            block: false,
            stacktrace: false,
            markers: true,
            context_info: true,
            latency_format: false,
            function_fork: false,
            display_graph: true,
            irq_info: true,
        }
    }
}

// ============================================================================
// Ftrace Manager
// ============================================================================

/// Ftrace subsystem manager
pub struct FtraceManager {
    /// Per-CPU buffers
    buffers: Vec<Mutex<TraceBuffer>>,
    /// Registered tracepoints
    tracepoints: RwLock<BTreeMap<String, Tracepoint>>,
    /// Current tracer config
    config: RwLock<TracerConfig>,
    /// Global enable
    enabled: AtomicBool,
    /// Function trace enabled
    func_trace_enabled: AtomicBool,
    /// Graph trace enabled
    graph_trace_enabled: AtomicBool,
    /// Event trace enabled
    event_trace_enabled: AtomicBool,
    /// Active filters
    filters: RwLock<Vec<TraceFilter>>,
    /// Total entries recorded
    total_entries: AtomicU64,
}

impl FtraceManager {
    /// Create new manager
    pub fn new(num_cpus: usize, buffer_size: usize) -> Self {
        let buffers = (0..num_cpus)
            .map(|i| Mutex::new(TraceBuffer::new(i as u32, buffer_size)))
            .collect();

        let mut manager = Self {
            buffers,
            tracepoints: RwLock::new(BTreeMap::new()),
            config: RwLock::new(TracerConfig::default()),
            enabled: AtomicBool::new(false),
            func_trace_enabled: AtomicBool::new(false),
            graph_trace_enabled: AtomicBool::new(false),
            event_trace_enabled: AtomicBool::new(true),
            filters: RwLock::new(Vec::new()),
            total_entries: AtomicU64::new(0),
        };

        // Register built-in tracepoints
        manager.register_builtin_tracepoints();

        manager
    }

    /// Register built-in tracepoints
    fn register_builtin_tracepoints(&mut self) {
        let tracepoints = [
            ("sched", "sched_switch", "prev_comm=%s prev_pid=%d prev_prio=%d prev_state=%s ==> next_comm=%s next_pid=%d next_prio=%d"),
            ("sched", "sched_wakeup", "comm=%s pid=%d prio=%d target_cpu=%03d"),
            ("sched", "sched_process_fork", "comm=%s pid=%d child_comm=%s child_pid=%d"),
            ("sched", "sched_process_exec", "filename=%s pid=%d old_pid=%d"),
            ("sched", "sched_process_exit", "comm=%s pid=%d prio=%d"),
            ("irq", "irq_handler_entry", "irq=%d name=%s"),
            ("irq", "irq_handler_exit", "irq=%d ret=%s"),
            ("irq", "softirq_entry", "vec=%u [action=%s]"),
            ("irq", "softirq_exit", "vec=%u [action=%s]"),
            ("syscalls", "sys_enter", "NR %d (%x, %x, %x, %x, %x, %x)"),
            ("syscalls", "sys_exit", "NR %d = %d"),
            ("kmem", "kmalloc", "call_site=%x ptr=%p bytes_req=%zu bytes_alloc=%zu gfp_flags=%s"),
            ("kmem", "kfree", "call_site=%x ptr=%p"),
            ("block", "block_rq_issue", "%d,%d %s %u (%s) %llu + %u [%s]"),
            ("block", "block_rq_complete", "%d,%d %s (%s) %llu + %u [%d]"),
            ("net", "netif_receive_skb", "dev=%s skbaddr=%p len=%u"),
            ("net", "net_dev_xmit", "dev=%s skbaddr=%p len=%u rc=%d"),
        ];

        let mut tps = self.tracepoints.write();
        for (subsys, name, format) in tracepoints.iter() {
            let tp = Tracepoint::new(subsys, name, format);
            tps.insert(tp.full_name(), tp);
        }
    }

    /// Enable tracing
    pub fn enable(&self) {
        self.enabled.store(true, Ordering::Relaxed);
    }

    /// Disable tracing
    pub fn disable(&self) {
        self.enabled.store(false, Ordering::Relaxed);
    }

    /// Check if enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Set tracer
    pub fn set_tracer(&self, tracer: TracerType) {
        let mut config = self.config.write();
        config.tracer = tracer;

        // Update trace flags
        self.func_trace_enabled.store(
            tracer == TracerType::Function || tracer == TracerType::FunctionGraph,
            Ordering::Relaxed,
        );
        self.graph_trace_enabled.store(
            tracer == TracerType::FunctionGraph,
            Ordering::Relaxed,
        );
    }

    /// Get current tracer
    pub fn current_tracer(&self) -> TracerType {
        self.config.read().tracer
    }

    /// Trace function entry
    pub fn trace_func_entry(&self, cpu: u32, pid: u32, func_addr: u64, parent_addr: u64) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        if !self.func_trace_enabled.load(Ordering::Relaxed) {
            return;
        }

        let entry = TraceEntry::Func(FuncTraceEntry {
            header: TraceEntryHeader::new(TraceEntryType::FuncEntry, cpu, pid),
            func_addr,
            parent_addr,
            func_name: None, // Would resolve from symbol table
        });

        self.write_entry(cpu, entry);
    }

    /// Trace function exit
    pub fn trace_func_exit(&self, cpu: u32, pid: u32, func_addr: u64) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        if !self.func_trace_enabled.load(Ordering::Relaxed) {
            return;
        }

        let entry = TraceEntry::Func(FuncTraceEntry {
            header: TraceEntryHeader::new(TraceEntryType::FuncExit, cpu, pid),
            func_addr,
            parent_addr: 0,
            func_name: None,
        });

        self.write_entry(cpu, entry);
    }

    /// Trace function graph entry
    pub fn trace_graph_entry(&self, cpu: u32, pid: u32, func_addr: u64, depth: u32) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        if !self.graph_trace_enabled.load(Ordering::Relaxed) {
            return;
        }

        let entry = TraceEntry::Graph(GraphTraceEntry {
            header: TraceEntryHeader::new(TraceEntryType::GraphEntry, cpu, pid),
            func_addr,
            depth,
            duration_ns: 0,
            flags: 0,
        });

        self.write_entry(cpu, entry);
    }

    /// Trace function graph exit
    pub fn trace_graph_exit(&self, cpu: u32, pid: u32, func_addr: u64, depth: u32, duration_ns: u64) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        if !self.graph_trace_enabled.load(Ordering::Relaxed) {
            return;
        }

        let entry = TraceEntry::Graph(GraphTraceEntry {
            header: TraceEntryHeader::new(TraceEntryType::GraphExit, cpu, pid),
            func_addr,
            depth,
            duration_ns,
            flags: 0,
        });

        self.write_entry(cpu, entry);
    }

    /// Trace tracepoint hit
    pub fn trace_event(&self, cpu: u32, pid: u32, subsystem: &str, name: &str, data: Vec<u8>) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        if !self.event_trace_enabled.load(Ordering::Relaxed) {
            return;
        }

        // Check if tracepoint is enabled
        let full_name = format!("{}:{}", subsystem, name);
        {
            let tps = self.tracepoints.read();
            if let Some(tp) = tps.get(&full_name) {
                if !tp.is_enabled() {
                    return;
                }
                tp.hit();
            }
        }

        let entry = TraceEntry::Tracepoint(TracepointEntry {
            header: TraceEntryHeader::new(TraceEntryType::Tracepoint, cpu, pid),
            name: name.to_string(),
            subsystem: subsystem.to_string(),
            data,
        });

        self.write_entry(cpu, entry);
    }

    /// Trace print (trace_printk equivalent)
    pub fn trace_print(&self, cpu: u32, pid: u32, message: &str) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }

        let entry = TraceEntry::Print(PrintTraceEntry {
            header: TraceEntryHeader::new(TraceEntryType::Print, cpu, pid),
            message: message.to_string(),
        });

        self.write_entry(cpu, entry);
    }

    /// Write marker
    pub fn trace_marker(&self, message: &str) {
        let cpu = 0; // Would get current CPU
        let pid = 0; // Would get current PID
        self.trace_print(cpu, pid, message);
    }

    /// Write entry to buffer
    fn write_entry(&self, cpu: u32, entry: TraceEntry) {
        // Apply filters
        let filters = self.filters.read();
        for filter in filters.iter() {
            if !filter.matches(&entry) {
                return;
            }
        }

        // Write to CPU buffer
        let cpu_idx = (cpu as usize) % self.buffers.len();
        self.buffers[cpu_idx].lock().write(entry);
        self.total_entries.fetch_add(1, Ordering::Relaxed);
    }

    /// Enable tracepoint
    pub fn enable_tracepoint(&self, full_name: &str) -> bool {
        let tps = self.tracepoints.read();
        if let Some(tp) = tps.get(full_name) {
            tp.enable();
            true
        } else {
            false
        }
    }

    /// Disable tracepoint
    pub fn disable_tracepoint(&self, full_name: &str) -> bool {
        let tps = self.tracepoints.read();
        if let Some(tp) = tps.get(full_name) {
            tp.disable();
            true
        } else {
            false
        }
    }

    /// List available tracepoints
    pub fn list_tracepoints(&self) -> Vec<String> {
        self.tracepoints.read().keys().cloned().collect()
    }

    /// Add filter
    pub fn add_filter(&self, filter: TraceFilter) {
        self.filters.write().push(filter);
    }

    /// Clear filters
    pub fn clear_filters(&self) {
        self.filters.write().clear();
    }

    /// Read trace from all CPUs
    pub fn read_trace(&self) -> Vec<TraceEntry> {
        let mut all_entries: Vec<TraceEntry> = Vec::new();

        for buffer in &self.buffers {
            let buf = buffer.lock();
            all_entries.extend(buf.read_all());
        }

        // Sort by timestamp
        all_entries.sort_by(|a, b| {
            let ts_a = match a {
                TraceEntry::Func(e) => e.header.timestamp,
                TraceEntry::Graph(e) => e.header.timestamp,
                TraceEntry::Tracepoint(e) => e.header.timestamp,
                TraceEntry::Print(e) => e.header.timestamp,
                TraceEntry::Stack(e) => e.header.timestamp,
                TraceEntry::Irq(e) => e.header.timestamp,
                TraceEntry::Sched(e) => e.header.timestamp,
                TraceEntry::Syscall(e) => e.header.timestamp,
            };
            let ts_b = match b {
                TraceEntry::Func(e) => e.header.timestamp,
                TraceEntry::Graph(e) => e.header.timestamp,
                TraceEntry::Tracepoint(e) => e.header.timestamp,
                TraceEntry::Print(e) => e.header.timestamp,
                TraceEntry::Stack(e) => e.header.timestamp,
                TraceEntry::Irq(e) => e.header.timestamp,
                TraceEntry::Sched(e) => e.header.timestamp,
                TraceEntry::Syscall(e) => e.header.timestamp,
            };
            ts_a.cmp(&ts_b)
        });

        all_entries
    }

    /// Clear all buffers
    pub fn clear_trace(&self) {
        for buffer in &self.buffers {
            buffer.lock().clear();
        }
    }

    /// Get total entries
    pub fn total_entries(&self) -> u64 {
        self.total_entries.load(Ordering::Relaxed)
    }

    /// Get buffer stats
    pub fn buffer_stats(&self) -> Vec<(u32, usize, u64)> {
        self.buffers.iter()
            .map(|b| {
                let buf = b.lock();
                (buf.id, buf.len(), buf.overruns())
            })
            .collect()
    }
}

impl Default for FtraceManager {
    fn default() -> Self {
        Self::new(1, 65536)
    }
}

// ============================================================================
// Global Instance
// ============================================================================

static FTRACE: Once<FtraceManager> = Once::new();

/// Initialize ftrace subsystem
pub fn init() {
    FTRACE.call_once(|| FtraceManager::new(1, 65536));
    crate::kprintln!("ftrace: initialized");
}

/// Get ftrace manager
pub fn manager() -> &'static FtraceManager {
    FTRACE.get().expect("Ftrace not initialized")
}

/// Enable tracing
pub fn tracing_on() {
    manager().enable();
}

/// Disable tracing
pub fn tracing_off() {
    manager().disable();
}

/// Set tracer
pub fn set_current_tracer(tracer: TracerType) {
    manager().set_tracer(tracer);
}

/// Get current tracer
pub fn current_tracer() -> TracerType {
    manager().current_tracer()
}

/// Enable tracepoint
pub fn trace_event_enable(subsystem: &str, name: &str) -> bool {
    manager().enable_tracepoint(&format!("{}:{}", subsystem, name))
}

/// Disable tracepoint
pub fn trace_event_disable(subsystem: &str, name: &str) -> bool {
    manager().disable_tracepoint(&format!("{}:{}", subsystem, name))
}

/// Write trace marker
pub fn trace_marker(message: &str) {
    manager().trace_marker(message);
}

/// Read trace
pub fn read_trace() -> Vec<TraceEntry> {
    manager().read_trace()
}

/// Clear trace
pub fn clear_trace() {
    manager().clear_trace();
}

/// List tracepoints
pub fn available_tracepoints() -> Vec<String> {
    manager().list_tracepoints()
}

// ============================================================================
// Trace Macros (would be actual macros in real implementation)
// ============================================================================

/// Trace function entry (would be placed at function entry)
pub fn trace_func(func_addr: u64, parent_addr: u64) {
    manager().trace_func_entry(0, 0, func_addr, parent_addr);
}

/// Trace event (for use from other modules)
pub fn trace_event(subsystem: &str, name: &str, data: Vec<u8>) {
    manager().trace_event(0, 0, subsystem, name, data);
}
