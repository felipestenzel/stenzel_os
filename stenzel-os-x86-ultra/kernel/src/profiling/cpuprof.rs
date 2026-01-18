//! CPU Profiler for Stenzel OS.
//!
//! Provides sample-based CPU profiling with:
//! - Timer-based sampling
//! - Stack trace collection
//! - Per-function statistics
//! - Flame graph generation
//! - Hot spot detection

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use spin::{Mutex, Once};

// ============================================================================
// Profiler Configuration
// ============================================================================

/// Profiler sampling mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SamplingMode {
    /// Timer-based sampling (periodic interrupts)
    Timer,
    /// Event-based sampling (perf events)
    Event,
    /// Instruction-based sampling (IBS on AMD)
    Ibs,
    /// Precise Event Based Sampling (PEBS on Intel)
    Pebs,
}

/// Profiler state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfilerState {
    /// Profiler is stopped
    Stopped,
    /// Profiler is running
    Running,
    /// Profiler is paused
    Paused,
}

/// Profiler configuration
#[derive(Debug, Clone)]
pub struct ProfilerConfig {
    /// Sampling frequency in Hz
    pub sample_freq: u32,
    /// Sampling mode
    pub sampling_mode: SamplingMode,
    /// Maximum stack depth to capture
    pub max_stack_depth: usize,
    /// Include kernel frames
    pub include_kernel: bool,
    /// Include user frames
    pub include_user: bool,
    /// Target PID (0 for all)
    pub target_pid: u32,
    /// Target CPU (-1 for all)
    pub target_cpu: i32,
    /// Sample buffer size
    pub buffer_size: usize,
}

impl Default for ProfilerConfig {
    fn default() -> Self {
        Self {
            sample_freq: 99, // 99 Hz to avoid aliasing
            sampling_mode: SamplingMode::Timer,
            max_stack_depth: 64,
            include_kernel: true,
            include_user: true,
            target_pid: 0,
            target_cpu: -1,
            buffer_size: 65536,
        }
    }
}

// ============================================================================
// Stack Frame
// ============================================================================

/// Stack frame information
#[derive(Debug, Clone)]
pub struct StackFrame {
    /// Instruction pointer
    pub ip: u64,
    /// Stack pointer
    pub sp: u64,
    /// Base pointer
    pub bp: u64,
    /// Function name (if resolved)
    pub func_name: Option<String>,
    /// Module/binary name
    pub module: Option<String>,
    /// Offset within function
    pub offset: u64,
    /// Is kernel frame
    pub is_kernel: bool,
}

impl StackFrame {
    /// Create new stack frame
    pub fn new(ip: u64, sp: u64, bp: u64) -> Self {
        Self {
            ip,
            sp,
            bp,
            func_name: None,
            module: None,
            offset: 0,
            is_kernel: ip >= 0xFFFF_8000_0000_0000,
        }
    }

    /// Format frame for display
    pub fn format(&self) -> String {
        match (&self.func_name, &self.module) {
            (Some(func), Some(module)) => {
                format!("{}!{}+0x{:x}", module, func, self.offset)
            }
            (Some(func), None) => {
                format!("{}+0x{:x}", func, self.offset)
            }
            (None, Some(module)) => {
                format!("{}!0x{:016x}", module, self.ip)
            }
            (None, None) => {
                format!("0x{:016x}", self.ip)
            }
        }
    }
}

/// Stack trace (collection of frames)
#[derive(Debug, Clone)]
pub struct StackTrace {
    /// Frames in the stack trace (top to bottom)
    pub frames: Vec<StackFrame>,
    /// Total frame count (may be truncated)
    pub total_frames: usize,
    /// Truncated (hit max depth)
    pub truncated: bool,
}

impl StackTrace {
    /// Create empty stack trace
    pub fn new() -> Self {
        Self {
            frames: Vec::new(),
            total_frames: 0,
            truncated: false,
        }
    }

    /// Add a frame
    pub fn push(&mut self, frame: StackFrame) {
        self.frames.push(frame);
        self.total_frames += 1;
    }

    /// Get frame count
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Format as stack trace string
    pub fn format(&self) -> String {
        let mut result = String::new();
        for (i, frame) in self.frames.iter().enumerate() {
            result.push_str(&format!("  #{}: {}\n", i, frame.format()));
        }
        if self.truncated {
            result.push_str(&format!("  ... ({} more frames)\n",
                self.total_frames - self.frames.len()));
        }
        result
    }

    /// Format as folded stack (for flame graphs)
    pub fn format_folded(&self) -> String {
        let names: Vec<String> = self.frames.iter()
            .rev()
            .map(|f| f.func_name.clone().unwrap_or_else(|| format!("0x{:x}", f.ip)))
            .collect();
        names.join(";")
    }
}

impl Default for StackTrace {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Profile Sample
// ============================================================================

/// A single profile sample
#[derive(Debug, Clone)]
pub struct ProfileSample {
    /// Timestamp (nanoseconds since boot)
    pub timestamp: u64,
    /// CPU that took the sample
    pub cpu: u32,
    /// Process ID
    pub pid: u32,
    /// Thread ID
    pub tid: u32,
    /// Process name
    pub comm: String,
    /// Instruction pointer at sample time
    pub ip: u64,
    /// Stack trace
    pub stack: StackTrace,
    /// Kernel mode at sample time
    pub kernel_mode: bool,
    /// Additional context
    pub context: SampleContext,
}

/// Additional sample context
#[derive(Debug, Clone)]
pub struct SampleContext {
    /// CPU cycles at sample
    pub cycles: u64,
    /// Instructions retired
    pub instructions: u64,
    /// Cache references
    pub cache_refs: u64,
    /// Cache misses
    pub cache_misses: u64,
    /// Branch instructions
    pub branches: u64,
    /// Branch misses
    pub branch_misses: u64,
}

impl Default for SampleContext {
    fn default() -> Self {
        Self {
            cycles: 0,
            instructions: 0,
            cache_refs: 0,
            cache_misses: 0,
            branches: 0,
            branch_misses: 0,
        }
    }
}

// ============================================================================
// Profile Statistics
// ============================================================================

/// Per-function statistics
#[derive(Debug, Clone)]
pub struct FunctionStats {
    /// Function name/address
    pub name: String,
    /// Module
    pub module: Option<String>,
    /// Sample count (function on top of stack)
    pub self_samples: u64,
    /// Sample count (function anywhere in stack)
    pub total_samples: u64,
    /// Percentage of total (self)
    pub self_percent: f64,
    /// Percentage of total
    pub total_percent: f64,
    /// Child functions
    pub children: Vec<String>,
}

/// Profiling statistics
#[derive(Debug, Clone)]
pub struct ProfileStats {
    /// Total samples collected
    pub total_samples: u64,
    /// Samples lost (buffer overflow)
    pub lost_samples: u64,
    /// Profiling duration (nanoseconds)
    pub duration_ns: u64,
    /// Per-function statistics
    pub functions: BTreeMap<String, FunctionStats>,
    /// Top N hot functions
    pub hotspots: Vec<FunctionStats>,
    /// Per-CPU sample counts
    pub per_cpu_samples: Vec<u64>,
    /// Per-process sample counts
    pub per_process_samples: BTreeMap<u32, u64>,
}

impl ProfileStats {
    /// Create new stats
    pub fn new() -> Self {
        Self {
            total_samples: 0,
            lost_samples: 0,
            duration_ns: 0,
            functions: BTreeMap::new(),
            hotspots: Vec::new(),
            per_cpu_samples: Vec::new(),
            per_process_samples: BTreeMap::new(),
        }
    }
}

impl Default for ProfileStats {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Flame Graph Generation
// ============================================================================

/// Flame graph node
#[derive(Debug, Clone)]
pub struct FlameNode {
    /// Function name
    pub name: String,
    /// Sample count at this level
    pub self_count: u64,
    /// Total sample count (self + children)
    pub total_count: u64,
    /// Child nodes
    pub children: BTreeMap<String, FlameNode>,
}

impl FlameNode {
    /// Create new node
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            self_count: 0,
            total_count: 0,
            children: BTreeMap::new(),
        }
    }

    /// Add a stack trace to the flame graph
    pub fn add_stack(&mut self, stack: &[String], count: u64) {
        self.total_count += count;

        if stack.is_empty() {
            self.self_count += count;
            return;
        }

        let first = &stack[0];
        let child = self.children.entry(first.clone())
            .or_insert_with(|| FlameNode::new(first));
        child.add_stack(&stack[1..], count);
    }

    /// Format as folded stack format (for flamegraph.pl)
    pub fn format_folded(&self, prefix: &str) -> String {
        let mut result = String::new();

        let my_prefix = if prefix.is_empty() {
            self.name.clone()
        } else {
            format!("{};{}", prefix, self.name)
        };

        if self.self_count > 0 {
            result.push_str(&format!("{} {}\n", my_prefix, self.self_count));
        }

        for child in self.children.values() {
            result.push_str(&child.format_folded(&my_prefix));
        }

        result
    }

    /// Format as SVG flame graph
    pub fn format_svg(&self, _width: u32, _height: u32) -> String {
        // Simplified SVG output
        let mut svg = String::new();
        svg.push_str("<?xml version=\"1.0\" standalone=\"no\"?>\n");
        svg.push_str("<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\">\n");
        svg.push_str("  <!-- Stenzel OS CPU Profile Flame Graph -->\n");
        svg.push_str(&format!("  <!-- Total samples: {} -->\n", self.total_count));
        // TODO: Proper SVG rendering
        svg.push_str("</svg>\n");
        svg
    }
}

/// Flame graph generator
pub struct FlameGraph {
    /// Root node
    root: FlameNode,
    /// Title
    title: String,
}

impl FlameGraph {
    /// Create new flame graph
    pub fn new(title: &str) -> Self {
        Self {
            root: FlameNode::new("all"),
            title: title.to_string(),
        }
    }

    /// Add samples from profiler
    pub fn add_samples(&mut self, samples: &[ProfileSample]) {
        for sample in samples {
            let stack: Vec<String> = sample.stack.frames.iter()
                .rev()
                .map(|f| f.func_name.clone().unwrap_or_else(|| format!("0x{:x}", f.ip)))
                .collect();
            self.root.add_stack(&stack, 1);
        }
    }

    /// Generate folded stack format
    pub fn generate_folded(&self) -> String {
        let mut result = String::new();
        for child in self.root.children.values() {
            result.push_str(&child.format_folded(""));
        }
        result
    }

    /// Generate SVG
    pub fn generate_svg(&self, width: u32, height: u32) -> String {
        self.root.format_svg(width, height)
    }
}

// ============================================================================
// Sample Buffer
// ============================================================================

/// Ring buffer for samples
pub struct SampleBuffer {
    /// Samples
    samples: Vec<ProfileSample>,
    /// Head index
    head: AtomicUsize,
    /// Tail index
    tail: AtomicUsize,
    /// Capacity
    capacity: usize,
    /// Overflow count
    overflow: AtomicU64,
    /// Lock for thread safety
    lock: Mutex<()>,
}

impl SampleBuffer {
    /// Create new buffer
    pub fn new(capacity: usize) -> Self {
        Self {
            samples: Vec::with_capacity(capacity),
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            capacity,
            overflow: AtomicU64::new(0),
            lock: Mutex::new(()),
        }
    }

    /// Push a sample
    pub fn push(&mut self, sample: ProfileSample) {
        let _guard = self.lock.lock();

        if self.samples.len() < self.capacity {
            self.samples.push(sample);
        } else {
            let head = self.head.load(Ordering::Relaxed);
            self.samples[head] = sample;
            self.head.store((head + 1) % self.capacity, Ordering::Relaxed);
            self.overflow.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Drain all samples
    pub fn drain(&mut self) -> Vec<ProfileSample> {
        let _guard = self.lock.lock();
        let result = core::mem::take(&mut self.samples);
        self.head.store(0, Ordering::Relaxed);
        self.tail.store(0, Ordering::Relaxed);
        result
    }

    /// Get sample count
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Get overflow count
    pub fn overflow_count(&self) -> u64 {
        self.overflow.load(Ordering::Relaxed)
    }

    /// Clear the buffer
    pub fn clear(&mut self) {
        let _guard = self.lock.lock();
        self.samples.clear();
        self.head.store(0, Ordering::Relaxed);
        self.tail.store(0, Ordering::Relaxed);
    }
}

// ============================================================================
// Stack Walker
// ============================================================================

/// Stack walking method
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackWalkMethod {
    /// Frame pointer based (requires -fno-omit-frame-pointer)
    FramePointer,
    /// DWARF unwinding
    Dwarf,
    /// ORC unwinding (Linux-style)
    Orc,
    /// LBR (Last Branch Record)
    Lbr,
}

/// Stack walker
pub struct StackWalker {
    /// Walking method
    method: StackWalkMethod,
    /// Maximum depth
    max_depth: usize,
    /// Symbol resolver
    resolver: SymbolResolver,
}

impl StackWalker {
    /// Create new stack walker
    pub fn new(method: StackWalkMethod, max_depth: usize) -> Self {
        Self {
            method,
            max_depth,
            resolver: SymbolResolver::new(),
        }
    }

    /// Walk the stack from current context
    pub fn walk(&self, ip: u64, bp: u64, sp: u64) -> StackTrace {
        match self.method {
            StackWalkMethod::FramePointer => self.walk_frame_pointer(ip, bp, sp),
            StackWalkMethod::Dwarf => self.walk_dwarf(ip, bp, sp),
            StackWalkMethod::Orc => self.walk_orc(ip, bp, sp),
            StackWalkMethod::Lbr => self.walk_lbr(ip),
        }
    }

    /// Walk using frame pointers
    fn walk_frame_pointer(&self, ip: u64, mut bp: u64, sp: u64) -> StackTrace {
        let mut trace = StackTrace::new();

        // First frame (current IP)
        let mut frame = StackFrame::new(ip, sp, bp);
        self.resolver.resolve(&mut frame);
        trace.push(frame);

        // Walk the frame pointer chain
        for _ in 1..self.max_depth {
            if bp == 0 || bp < 0x1000 {
                break;
            }

            // Read return address and previous BP
            let (ret_addr, prev_bp) = unsafe {
                // Safety: Assuming valid stack memory
                let ret_ptr = (bp + 8) as *const u64;
                let bp_ptr = bp as *const u64;

                // Validate pointers are in valid memory range
                if !self.is_valid_stack_addr(bp) {
                    break;
                }

                (*ret_ptr, *bp_ptr)
            };

            if ret_addr == 0 {
                break;
            }

            let mut frame = StackFrame::new(ret_addr, bp, prev_bp);
            self.resolver.resolve(&mut frame);
            trace.push(frame);

            bp = prev_bp;
        }

        if bp != 0 && trace.frames.len() == self.max_depth {
            trace.truncated = true;
        }

        trace
    }

    /// Walk using DWARF info
    fn walk_dwarf(&self, ip: u64, bp: u64, sp: u64) -> StackTrace {
        // TODO: Implement DWARF unwinding
        self.walk_frame_pointer(ip, bp, sp)
    }

    /// Walk using ORC tables
    fn walk_orc(&self, ip: u64, bp: u64, sp: u64) -> StackTrace {
        // TODO: Implement ORC unwinding
        self.walk_frame_pointer(ip, bp, sp)
    }

    /// Walk using LBR
    fn walk_lbr(&self, ip: u64) -> StackTrace {
        let mut trace = StackTrace::new();

        // Read LBR entries from MSRs
        // MSR_LBR_TOS: 0x1C9 (top of stack)
        // MSR_LBR_SELECT: 0x1C8
        // MSR_LASTBRANCH_x_FROM_IP: 0x680-0x68F
        // MSR_LASTBRANCH_x_TO_IP: 0x6C0-0x6CF

        let mut frame = StackFrame::new(ip, 0, 0);
        self.resolver.resolve(&mut frame);
        trace.push(frame);

        // Read up to 32 LBR entries
        for i in 0..32.min(self.max_depth.saturating_sub(1)) {
            let from_msr = 0x680 + i as u32;
            let from_ip = unsafe { x86_64::registers::model_specific::Msr::new(from_msr).read() };

            if from_ip == 0 {
                break;
            }

            let mut frame = StackFrame::new(from_ip, 0, 0);
            self.resolver.resolve(&mut frame);
            trace.push(frame);
        }

        trace
    }

    /// Check if address is valid stack address
    fn is_valid_stack_addr(&self, addr: u64) -> bool {
        // Kernel stack range or user stack range
        (addr >= 0xFFFF_8000_0000_0000) || (addr >= 0x7F00_0000_0000 && addr < 0x8000_0000_0000)
    }
}

// ============================================================================
// Symbol Resolver
// ============================================================================

/// Symbol table entry
#[derive(Debug, Clone)]
pub struct Symbol {
    /// Address
    pub addr: u64,
    /// Size
    pub size: u64,
    /// Name
    pub name: String,
    /// Module
    pub module: Option<String>,
}

/// Symbol resolver
pub struct SymbolResolver {
    /// Kernel symbols
    symbols: BTreeMap<u64, Symbol>,
}

impl SymbolResolver {
    /// Create new resolver
    pub fn new() -> Self {
        Self {
            symbols: BTreeMap::new(),
        }
    }

    /// Add a symbol
    pub fn add_symbol(&mut self, sym: Symbol) {
        self.symbols.insert(sym.addr, sym);
    }

    /// Load kernel symbols from kallsyms format
    pub fn load_kallsyms(&mut self, data: &str) {
        for line in data.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                if let Ok(addr) = u64::from_str_radix(parts[0], 16) {
                    let sym = Symbol {
                        addr,
                        size: 0,
                        name: parts[2].to_string(),
                        module: parts.get(3).map(|s| s.to_string()),
                    };
                    self.add_symbol(sym);
                }
            }
        }
    }

    /// Resolve an address to a symbol
    pub fn resolve(&self, frame: &mut StackFrame) {
        // Find the symbol containing this address
        if let Some((&addr, sym)) = self.symbols.range(..=frame.ip).next_back() {
            if sym.size == 0 || frame.ip < addr + sym.size {
                frame.func_name = Some(sym.name.clone());
                frame.module = sym.module.clone();
                frame.offset = frame.ip - addr;
            }
        }
    }

    /// Lookup symbol by name
    pub fn lookup(&self, name: &str) -> Option<&Symbol> {
        self.symbols.values().find(|s| s.name == name)
    }
}

impl Default for SymbolResolver {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// CPU Profiler
// ============================================================================

/// Global CPU profiler
static CPU_PROFILER: Once<Mutex<CpuProfiler>> = Once::new();

/// Get the CPU profiler instance
pub fn profiler() -> &'static Mutex<CpuProfiler> {
    CPU_PROFILER.call_once(|| Mutex::new(CpuProfiler::new()))
}

/// CPU Profiler
pub struct CpuProfiler {
    /// Configuration
    config: ProfilerConfig,
    /// Current state
    state: ProfilerState,
    /// Sample buffer
    buffer: SampleBuffer,
    /// Stack walker
    walker: StackWalker,
    /// Symbol resolver
    resolver: SymbolResolver,
    /// Start timestamp
    start_time: u64,
    /// Profiling enabled flag
    enabled: AtomicBool,
    /// Per-CPU sample counts
    per_cpu_counts: Vec<AtomicU64>,
}

impl CpuProfiler {
    /// Create new profiler
    pub fn new() -> Self {
        Self {
            config: ProfilerConfig::default(),
            state: ProfilerState::Stopped,
            buffer: SampleBuffer::new(65536),
            walker: StackWalker::new(StackWalkMethod::FramePointer, 64),
            resolver: SymbolResolver::new(),
            start_time: 0,
            enabled: AtomicBool::new(false),
            per_cpu_counts: (0..256).map(|_| AtomicU64::new(0)).collect(),
        }
    }

    /// Configure the profiler
    pub fn configure(&mut self, config: ProfilerConfig) {
        if self.state != ProfilerState::Stopped {
            return;
        }

        self.config = config.clone();
        self.buffer = SampleBuffer::new(config.buffer_size);
        self.walker = StackWalker::new(
            StackWalkMethod::FramePointer,
            config.max_stack_depth,
        );
    }

    /// Start profiling
    pub fn start(&mut self) -> Result<(), &'static str> {
        if self.state == ProfilerState::Running {
            return Err("Profiler already running");
        }

        self.buffer.clear();
        self.start_time = crate::time::uptime_ns();
        self.state = ProfilerState::Running;
        self.enabled.store(true, Ordering::SeqCst);

        // Setup timer interrupt or perf event
        self.setup_sampling()?;

        Ok(())
    }

    /// Stop profiling
    pub fn stop(&mut self) -> Result<ProfileStats, &'static str> {
        if self.state != ProfilerState::Running {
            return Err("Profiler not running");
        }

        self.enabled.store(false, Ordering::SeqCst);
        self.state = ProfilerState::Stopped;

        // Stop sampling
        self.teardown_sampling();

        // Generate statistics
        let stats = self.generate_stats();

        Ok(stats)
    }

    /// Pause profiling
    pub fn pause(&mut self) {
        if self.state == ProfilerState::Running {
            self.enabled.store(false, Ordering::SeqCst);
            self.state = ProfilerState::Paused;
        }
    }

    /// Resume profiling
    pub fn resume(&mut self) {
        if self.state == ProfilerState::Paused {
            self.enabled.store(true, Ordering::SeqCst);
            self.state = ProfilerState::Running;
        }
    }

    /// Record a sample (called from interrupt handler)
    pub fn record_sample(&mut self, ip: u64, bp: u64, sp: u64, cpu: u32, pid: u32, tid: u32, comm: &str) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }

        // Check filters
        if self.config.target_pid != 0 && pid != self.config.target_pid {
            return;
        }
        if self.config.target_cpu >= 0 && cpu != self.config.target_cpu as u32 {
            return;
        }

        let kernel_mode = ip >= 0xFFFF_8000_0000_0000;
        if kernel_mode && !self.config.include_kernel {
            return;
        }
        if !kernel_mode && !self.config.include_user {
            return;
        }

        // Walk the stack
        let stack = self.walker.walk(ip, bp, sp);

        // Create sample
        let sample = ProfileSample {
            timestamp: crate::time::uptime_ns(),
            cpu,
            pid,
            tid,
            comm: comm.to_string(),
            ip,
            stack,
            kernel_mode,
            context: SampleContext::default(),
        };

        self.buffer.push(sample);

        // Update per-CPU count
        if (cpu as usize) < self.per_cpu_counts.len() {
            self.per_cpu_counts[cpu as usize].fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Setup sampling mechanism
    fn setup_sampling(&self) -> Result<(), &'static str> {
        match self.config.sampling_mode {
            SamplingMode::Timer => {
                // Configure timer interrupt for sampling
                // This would hook into the scheduler tick
                Ok(())
            }
            SamplingMode::Event => {
                // Setup perf event
                Ok(())
            }
            SamplingMode::Ibs | SamplingMode::Pebs => {
                // Hardware-specific setup
                Ok(())
            }
        }
    }

    /// Teardown sampling
    fn teardown_sampling(&self) {
        // Cleanup sampling mechanism
    }

    /// Generate statistics from samples
    fn generate_stats(&mut self) -> ProfileStats {
        let samples = self.buffer.drain();
        let mut stats = ProfileStats::new();

        stats.total_samples = samples.len() as u64;
        stats.lost_samples = self.buffer.overflow_count();
        stats.duration_ns = crate::time::uptime_ns() - self.start_time;

        // Collect per-CPU samples
        for count in &self.per_cpu_counts {
            stats.per_cpu_samples.push(count.load(Ordering::Relaxed));
        }

        // Analyze samples
        for sample in &samples {
            // Per-process count
            *stats.per_process_samples.entry(sample.pid).or_insert(0) += 1;

            // Per-function statistics
            if let Some(frame) = sample.stack.frames.first() {
                let name = frame.func_name.clone()
                    .unwrap_or_else(|| format!("0x{:x}", frame.ip));

                let func_stats = stats.functions.entry(name.clone()).or_insert_with(|| {
                    FunctionStats {
                        name: name.clone(),
                        module: frame.module.clone(),
                        self_samples: 0,
                        total_samples: 0,
                        self_percent: 0.0,
                        total_percent: 0.0,
                        children: Vec::new(),
                    }
                });
                func_stats.self_samples += 1;
            }

            // Count all functions in stack
            for frame in &sample.stack.frames {
                let name = frame.func_name.clone()
                    .unwrap_or_else(|| format!("0x{:x}", frame.ip));

                let func_stats = stats.functions.entry(name.clone()).or_insert_with(|| {
                    FunctionStats {
                        name: name.clone(),
                        module: frame.module.clone(),
                        self_samples: 0,
                        total_samples: 0,
                        self_percent: 0.0,
                        total_percent: 0.0,
                        children: Vec::new(),
                    }
                });
                func_stats.total_samples += 1;
            }
        }

        // Calculate percentages
        let total = stats.total_samples as f64;
        for func in stats.functions.values_mut() {
            func.self_percent = (func.self_samples as f64 / total) * 100.0;
            func.total_percent = (func.total_samples as f64 / total) * 100.0;
        }

        // Get top hotspots
        let mut hotspots: Vec<_> = stats.functions.values().cloned().collect();
        hotspots.sort_by(|a, b| b.self_samples.cmp(&a.self_samples));
        stats.hotspots = hotspots.into_iter().take(20).collect();

        stats
    }

    /// Get collected samples
    pub fn get_samples(&mut self) -> Vec<ProfileSample> {
        self.buffer.drain()
    }

    /// Generate flame graph from samples
    pub fn generate_flame_graph(&mut self, title: &str) -> FlameGraph {
        let samples = self.buffer.drain();
        let mut fg = FlameGraph::new(title);
        fg.add_samples(&samples);
        fg
    }

    /// Load symbols for resolution
    pub fn load_symbols(&mut self, kallsyms: &str) {
        self.resolver.load_kallsyms(kallsyms);
    }

    /// Get current state
    pub fn state(&self) -> ProfilerState {
        self.state
    }

    /// Check if enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }
}

impl Default for CpuProfiler {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Public API
// ============================================================================

/// Initialize the CPU profiler
pub fn init() {
    let _ = profiler();
}

/// Start CPU profiling with default config
pub fn start_profiling() -> Result<(), &'static str> {
    profiler().lock().start()
}

/// Start CPU profiling with custom config
pub fn start_profiling_with_config(config: ProfilerConfig) -> Result<(), &'static str> {
    let mut prof = profiler().lock();
    prof.configure(config);
    prof.start()
}

/// Stop profiling and get results
pub fn stop_profiling() -> Result<ProfileStats, &'static str> {
    profiler().lock().stop()
}

/// Pause profiling
pub fn pause_profiling() {
    profiler().lock().pause();
}

/// Resume profiling
pub fn resume_profiling() {
    profiler().lock().resume();
}

/// Record a sample (called from timer interrupt)
pub fn record_cpu_sample(ip: u64, bp: u64, sp: u64, cpu: u32, pid: u32, tid: u32, comm: &str) {
    profiler().lock().record_sample(ip, bp, sp, cpu, pid, tid, comm);
}

/// Generate flame graph
pub fn generate_flame_graph(title: &str) -> String {
    profiler().lock().generate_flame_graph(title).generate_folded()
}

/// Load kernel symbols
pub fn load_kernel_symbols(kallsyms: &str) {
    profiler().lock().load_symbols(kallsyms);
}

/// Get profiler state
pub fn profiler_state() -> ProfilerState {
    profiler().lock().state()
}

// ============================================================================
// Report Generation
// ============================================================================

/// Profile report
pub struct ProfileReport {
    /// Statistics
    pub stats: ProfileStats,
    /// Flame graph (folded format)
    pub flame_graph: String,
}

impl ProfileReport {
    /// Generate report from profiler
    pub fn generate() -> Result<Self, &'static str> {
        let stats = stop_profiling()?;
        let flame_graph = generate_flame_graph("CPU Profile");

        Ok(Self { stats, flame_graph })
    }

    /// Format as text report
    pub fn format_text(&self) -> String {
        let mut report = String::new();

        report.push_str("=== CPU Profile Report ===\n\n");
        report.push_str(&format!("Total samples: {}\n", self.stats.total_samples));
        report.push_str(&format!("Lost samples: {}\n", self.stats.lost_samples));
        report.push_str(&format!("Duration: {} ms\n", self.stats.duration_ns / 1_000_000));
        report.push_str("\n");

        report.push_str("=== Top Functions (by self time) ===\n");
        for (i, func) in self.stats.hotspots.iter().enumerate() {
            report.push_str(&format!(
                "{:2}. {:6.2}% {:6} samples  {}\n",
                i + 1,
                func.self_percent,
                func.self_samples,
                func.name
            ));
        }

        report
    }
}
