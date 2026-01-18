//! I/O Scheduler
//!
//! Implements I/O scheduling algorithms:
//! - mq-deadline: Multi-queue deadline scheduler
//! - BFQ: Budget Fair Queueing
//! - none: No-op scheduler (passthrough)
//! - Priority-based scheduling

use alloc::collections::{BTreeMap, VecDeque};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use crate::sync::IrqSafeMutex;

/// Block device ID
pub type DeviceId = u32;

/// Logical Block Address
pub type Lba = u64;

/// I/O request type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoRequestType {
    /// Read operation
    Read,
    /// Write operation
    Write,
    /// Discard/TRIM operation
    Discard,
    /// Flush operation
    Flush,
}

/// I/O priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IoPriority {
    /// Real-time priority (highest)
    RealTime = 0,
    /// Best-effort priority (normal)
    BestEffort = 1,
    /// Idle priority (lowest)
    Idle = 2,
}

impl Default for IoPriority {
    fn default() -> Self {
        IoPriority::BestEffort
    }
}

/// I/O request
#[derive(Debug, Clone)]
pub struct IoRequest {
    /// Request ID
    pub id: u64,
    /// Device ID
    pub device: DeviceId,
    /// Request type
    pub req_type: IoRequestType,
    /// Starting LBA
    pub lba: Lba,
    /// Number of sectors
    pub sectors: u32,
    /// Priority
    pub priority: IoPriority,
    /// Submission time (microseconds)
    pub submit_time: u64,
    /// Deadline (microseconds from now)
    pub deadline: u64,
    /// Process ID that submitted
    pub pid: u64,
    /// Whether this is a sync request
    pub sync: bool,
    /// Merge candidate
    pub mergeable: bool,
}

impl IoRequest {
    pub fn new(
        id: u64,
        device: DeviceId,
        req_type: IoRequestType,
        lba: Lba,
        sectors: u32,
    ) -> Self {
        Self {
            id,
            device,
            req_type,
            lba,
            sectors,
            priority: IoPriority::default(),
            submit_time: 0,
            deadline: 0,
            pid: 0,
            sync: false,
            mergeable: true,
        }
    }

    pub fn with_priority(mut self, priority: IoPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_deadline(mut self, deadline_us: u64) -> Self {
        self.deadline = deadline_us;
        self
    }

    pub fn sync(mut self) -> Self {
        self.sync = true;
        self
    }

    /// End LBA (exclusive)
    pub fn end_lba(&self) -> Lba {
        self.lba + self.sectors as u64
    }

    /// Check if request can merge with another (back merge)
    pub fn can_merge_back(&self, other: &IoRequest) -> bool {
        self.mergeable && other.mergeable &&
        self.req_type == other.req_type &&
        self.device == other.device &&
        self.end_lba() == other.lba &&
        self.priority == other.priority
    }

    /// Check if request can merge with another (front merge)
    pub fn can_merge_front(&self, other: &IoRequest) -> bool {
        other.can_merge_back(self)
    }

    /// Merge another request (back merge)
    pub fn merge_back(&mut self, other: &IoRequest) {
        self.sectors += other.sectors;
        // Keep earlier deadline
        if other.deadline < self.deadline {
            self.deadline = other.deadline;
        }
    }
}

/// Scheduler type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulerType {
    /// No-op scheduler (FIFO)
    None,
    /// Multi-queue deadline scheduler
    MqDeadline,
    /// Budget Fair Queueing
    Bfq,
    /// Completely Fair Queueing (legacy)
    Cfq,
}

impl Default for SchedulerType {
    fn default() -> Self {
        SchedulerType::MqDeadline
    }
}

/// Scheduler configuration
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    /// Scheduler type
    pub scheduler_type: SchedulerType,
    /// Read deadline (microseconds)
    pub read_deadline_us: u64,
    /// Write deadline (microseconds)
    pub write_deadline_us: u64,
    /// Number of requests batched before dispatch
    pub batch_size: u32,
    /// Enable request merging
    pub merge_enabled: bool,
    /// Maximum merge distance (sectors)
    pub max_merge_distance: u32,
    /// Enable write starvation prevention
    pub writes_starved: u32,
    /// FIFO batch (requests per batch for deadline)
    pub fifo_batch: u32,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            scheduler_type: SchedulerType::MqDeadline,
            read_deadline_us: 500_000,   // 500ms for reads
            write_deadline_us: 5_000_000, // 5s for writes
            batch_size: 16,
            merge_enabled: true,
            max_merge_distance: 128, // 64KB
            writes_starved: 4,       // Dispatch writes after 4 read batches
            fifo_batch: 8,
        }
    }
}

/// Scheduler statistics
#[derive(Debug, Clone, Default)]
pub struct SchedulerStats {
    /// Total requests submitted
    pub requests_submitted: u64,
    /// Total requests completed
    pub requests_completed: u64,
    /// Requests merged
    pub requests_merged: u64,
    /// Read requests
    pub read_requests: u64,
    /// Write requests
    pub write_requests: u64,
    /// Discard requests
    pub discard_requests: u64,
    /// Total read sectors
    pub read_sectors: u64,
    /// Total write sectors
    pub write_sectors: u64,
    /// Average latency (microseconds)
    pub avg_latency_us: u64,
    /// Maximum latency (microseconds)
    pub max_latency_us: u64,
    /// Requests expired (missed deadline)
    pub expired_requests: u64,
}

/// mq-deadline scheduler queue
pub struct DeadlineQueue {
    /// Read requests sorted by deadline
    read_fifo: VecDeque<IoRequest>,
    /// Write requests sorted by deadline
    write_fifo: VecDeque<IoRequest>,
    /// Read requests sorted by LBA
    read_sorted: Vec<IoRequest>,
    /// Write requests sorted by LBA
    write_sorted: Vec<IoRequest>,
    /// Batches since last write dispatch
    batches_since_write: u32,
    /// Configuration
    config: SchedulerConfig,
}

impl DeadlineQueue {
    pub fn new(config: SchedulerConfig) -> Self {
        Self {
            read_fifo: VecDeque::new(),
            write_fifo: VecDeque::new(),
            read_sorted: Vec::new(),
            write_sorted: Vec::new(),
            batches_since_write: 0,
            config,
        }
    }

    /// Add a request to the queue
    pub fn add(&mut self, mut request: IoRequest) {
        let now = current_time_us();
        request.submit_time = now;

        // Set deadline based on type
        let deadline = match request.req_type {
            IoRequestType::Read => self.config.read_deadline_us,
            IoRequestType::Write => self.config.write_deadline_us,
            _ => self.config.read_deadline_us,
        };
        request.deadline = now + deadline;

        // Try to merge
        if self.config.merge_enabled {
            if self.try_merge(&request) {
                return;
            }
        }

        // Add to appropriate queue
        match request.req_type {
            IoRequestType::Read => {
                self.read_fifo.push_back(request.clone());
                // Insert sorted by LBA
                let pos = self.read_sorted.binary_search_by_key(&request.lba, |r| r.lba)
                    .unwrap_or_else(|i| i);
                self.read_sorted.insert(pos, request);
            }
            IoRequestType::Write | IoRequestType::Discard => {
                self.write_fifo.push_back(request.clone());
                let pos = self.write_sorted.binary_search_by_key(&request.lba, |r| r.lba)
                    .unwrap_or_else(|i| i);
                self.write_sorted.insert(pos, request);
            }
            IoRequestType::Flush => {
                // Flush requests go to write queue
                self.write_fifo.push_back(request);
            }
        }
    }

    /// Try to merge request with existing ones
    fn try_merge(&mut self, request: &IoRequest) -> bool {
        let queue = match request.req_type {
            IoRequestType::Read => &mut self.read_sorted,
            IoRequestType::Write | IoRequestType::Discard => &mut self.write_sorted,
            IoRequestType::Flush => return false,
        };

        // Find adjacent request for back merge
        for existing in queue.iter_mut() {
            if existing.can_merge_back(request) &&
               request.lba.saturating_sub(existing.end_lba()) <= self.config.max_merge_distance as u64 {
                existing.merge_back(request);
                return true;
            }
        }

        false
    }

    /// Dispatch next request(s)
    pub fn dispatch(&mut self) -> Vec<IoRequest> {
        let mut batch = Vec::new();
        let now = current_time_us();

        // Check for expired requests first
        self.dispatch_expired(&mut batch, now);

        if batch.len() >= self.config.batch_size as usize {
            return batch;
        }

        // Determine which queue to service
        let service_writes = self.batches_since_write >= self.config.writes_starved;

        if service_writes && !self.write_fifo.is_empty() {
            // Dispatch writes
            self.dispatch_from_queue(&mut batch, true);
            self.batches_since_write = 0;
        } else if !self.read_fifo.is_empty() {
            // Dispatch reads
            self.dispatch_from_queue(&mut batch, false);
            self.batches_since_write += 1;
        } else if !self.write_fifo.is_empty() {
            // Fallback to writes
            self.dispatch_from_queue(&mut batch, true);
            self.batches_since_write = 0;
        }

        batch
    }

    /// Dispatch expired requests
    fn dispatch_expired(&mut self, batch: &mut Vec<IoRequest>, now: u64) {
        // Check read FIFO
        while let Some(req) = self.read_fifo.front() {
            if req.deadline <= now {
                if let Some(req) = self.read_fifo.pop_front() {
                    self.remove_from_sorted(&req, false);
                    batch.push(req);
                }
            } else {
                break;
            }
        }

        // Check write FIFO
        while let Some(req) = self.write_fifo.front() {
            if req.deadline <= now {
                if let Some(req) = self.write_fifo.pop_front() {
                    self.remove_from_sorted(&req, true);
                    batch.push(req);
                }
            } else {
                break;
            }
        }
    }

    /// Dispatch from sorted queue (sector order)
    fn dispatch_from_queue(&mut self, batch: &mut Vec<IoRequest>, is_write: bool) {
        let fifo = if is_write { &mut self.write_fifo } else { &mut self.read_fifo };
        let sorted = if is_write { &mut self.write_sorted } else { &mut self.read_sorted };

        // Dispatch in sector order up to batch size
        while batch.len() < self.config.fifo_batch as usize && !sorted.is_empty() {
            let req = sorted.remove(0);
            // Also remove from FIFO
            if let Some(pos) = fifo.iter().position(|r| r.id == req.id) {
                fifo.remove(pos);
            }
            batch.push(req);
        }
    }

    /// Remove request from sorted queue
    fn remove_from_sorted(&mut self, req: &IoRequest, is_write: bool) {
        let sorted = if is_write { &mut self.write_sorted } else { &mut self.read_sorted };
        if let Some(pos) = sorted.iter().position(|r| r.id == req.id) {
            sorted.remove(pos);
        }
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.read_fifo.is_empty() && self.write_fifo.is_empty()
    }

    /// Get queue depth
    pub fn depth(&self) -> usize {
        self.read_fifo.len() + self.write_fifo.len()
    }
}

/// BFQ (Budget Fair Queueing) process queue
pub struct BfqProcessQueue {
    /// Process ID
    pub pid: u64,
    /// Request queue
    pub requests: VecDeque<IoRequest>,
    /// Budget remaining (sectors)
    pub budget: u32,
    /// Weight (1-1000)
    pub weight: u16,
    /// Last service time
    pub last_service: u64,
    /// Total sectors served
    pub sectors_served: u64,
}

/// BFQ scheduler
pub struct BfqScheduler {
    /// Per-process queues
    process_queues: BTreeMap<u64, BfqProcessQueue>,
    /// Active process (being serviced)
    active_pid: Option<u64>,
    /// Budget per slice
    budget_per_slice: u32,
    /// Configuration
    config: SchedulerConfig,
}

impl BfqScheduler {
    pub fn new(config: SchedulerConfig) -> Self {
        Self {
            process_queues: BTreeMap::new(),
            active_pid: None,
            budget_per_slice: 128, // 64KB
            config,
        }
    }

    /// Add request
    pub fn add(&mut self, request: IoRequest) {
        let pid = request.pid;

        let queue = self.process_queues.entry(pid).or_insert_with(|| {
            BfqProcessQueue {
                pid,
                requests: VecDeque::new(),
                budget: self.budget_per_slice,
                weight: 100, // Default weight
                last_service: 0,
                sectors_served: 0,
            }
        });

        queue.requests.push_back(request);
    }

    /// Set weight for a process
    pub fn set_weight(&mut self, pid: u64, weight: u16) {
        if let Some(queue) = self.process_queues.get_mut(&pid) {
            queue.weight = weight.clamp(1, 1000);
        }
    }

    /// Dispatch next request(s)
    pub fn dispatch(&mut self) -> Vec<IoRequest> {
        let mut batch = Vec::new();

        // Select process to service
        if self.active_pid.is_none() || !self.has_budget() {
            self.select_next_process();
        }

        if let Some(pid) = self.active_pid {
            if let Some(queue) = self.process_queues.get_mut(&pid) {
                while batch.len() < self.config.batch_size as usize && queue.budget > 0 {
                    if let Some(req) = queue.requests.pop_front() {
                        let sectors = req.sectors;
                        batch.push(req);
                        queue.budget = queue.budget.saturating_sub(sectors);
                        queue.sectors_served += sectors as u64;
                    } else {
                        break;
                    }
                }

                queue.last_service = current_time_us();

                // If queue empty, clear active
                if queue.requests.is_empty() {
                    self.active_pid = None;
                }
            }
        }

        batch
    }

    /// Check if active process has budget
    fn has_budget(&self) -> bool {
        if let Some(pid) = self.active_pid {
            if let Some(queue) = self.process_queues.get(&pid) {
                return queue.budget > 0 && !queue.requests.is_empty();
            }
        }
        false
    }

    /// Select next process to service
    fn select_next_process(&mut self) {
        // Find process with requests and best (weighted) service ratio
        let mut best_pid: Option<u64> = None;
        let mut best_score: u64 = u64::MAX;

        for (pid, queue) in &self.process_queues {
            if queue.requests.is_empty() {
                continue;
            }

            // Score = sectors_served / weight (lower is better)
            let score = queue.sectors_served / (queue.weight as u64).max(1);
            if score < best_score {
                best_score = score;
                best_pid = Some(*pid);
            }
        }

        self.active_pid = best_pid;

        // Reset budget for new process
        if let Some(pid) = self.active_pid {
            if let Some(queue) = self.process_queues.get_mut(&pid) {
                queue.budget = self.budget_per_slice;
            }
        }
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.process_queues.values().all(|q| q.requests.is_empty())
    }
}

/// I/O Scheduler wrapper
pub enum IoScheduler {
    /// No-op (FIFO)
    None(VecDeque<IoRequest>),
    /// mq-deadline
    Deadline(DeadlineQueue),
    /// BFQ
    Bfq(BfqScheduler),
}

impl IoScheduler {
    pub fn new(config: SchedulerConfig) -> Self {
        match config.scheduler_type {
            SchedulerType::None => IoScheduler::None(VecDeque::new()),
            SchedulerType::MqDeadline => IoScheduler::Deadline(DeadlineQueue::new(config)),
            SchedulerType::Bfq | SchedulerType::Cfq => IoScheduler::Bfq(BfqScheduler::new(config)),
        }
    }

    /// Add a request
    pub fn add(&mut self, request: IoRequest) {
        match self {
            IoScheduler::None(queue) => queue.push_back(request),
            IoScheduler::Deadline(deadline) => deadline.add(request),
            IoScheduler::Bfq(bfq) => bfq.add(request),
        }
    }

    /// Dispatch requests
    pub fn dispatch(&mut self) -> Vec<IoRequest> {
        match self {
            IoScheduler::None(queue) => {
                let mut batch = Vec::new();
                while let Some(req) = queue.pop_front() {
                    batch.push(req);
                    if batch.len() >= 16 {
                        break;
                    }
                }
                batch
            }
            IoScheduler::Deadline(deadline) => deadline.dispatch(),
            IoScheduler::Bfq(bfq) => bfq.dispatch(),
        }
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        match self {
            IoScheduler::None(queue) => queue.is_empty(),
            IoScheduler::Deadline(deadline) => deadline.is_empty(),
            IoScheduler::Bfq(bfq) => bfq.is_empty(),
        }
    }

    /// Get scheduler type
    pub fn scheduler_type(&self) -> SchedulerType {
        match self {
            IoScheduler::None(_) => SchedulerType::None,
            IoScheduler::Deadline(_) => SchedulerType::MqDeadline,
            IoScheduler::Bfq(_) => SchedulerType::Bfq,
        }
    }
}

/// Per-device I/O scheduler manager
pub struct DeviceScheduler {
    /// Device ID
    device_id: DeviceId,
    /// Scheduler
    scheduler: IoScheduler,
    /// Statistics
    stats: SchedulerStats,
    /// Next request ID
    next_id: u64,
}

impl DeviceScheduler {
    pub fn new(device_id: DeviceId, config: SchedulerConfig) -> Self {
        Self {
            device_id,
            scheduler: IoScheduler::new(config),
            stats: SchedulerStats::default(),
            next_id: 1,
        }
    }

    /// Submit a request
    pub fn submit(&mut self, req_type: IoRequestType, lba: Lba, sectors: u32) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        let request = IoRequest::new(id, self.device_id, req_type, lba, sectors);
        self.scheduler.add(request);

        // Update stats
        self.stats.requests_submitted += 1;
        match req_type {
            IoRequestType::Read => {
                self.stats.read_requests += 1;
                self.stats.read_sectors += sectors as u64;
            }
            IoRequestType::Write => {
                self.stats.write_requests += 1;
                self.stats.write_sectors += sectors as u64;
            }
            IoRequestType::Discard => {
                self.stats.discard_requests += 1;
            }
            _ => {}
        }

        id
    }

    /// Get next batch of requests to dispatch
    pub fn dispatch(&mut self) -> Vec<IoRequest> {
        self.scheduler.dispatch()
    }

    /// Mark request as complete
    pub fn complete(&mut self, id: u64, latency_us: u64) {
        self.stats.requests_completed += 1;

        // Update average latency
        let total = self.stats.requests_completed;
        let old_avg = self.stats.avg_latency_us;
        self.stats.avg_latency_us = ((old_avg * (total - 1)) + latency_us) / total;

        if latency_us > self.stats.max_latency_us {
            self.stats.max_latency_us = latency_us;
        }
    }

    /// Get statistics
    pub fn stats(&self) -> &SchedulerStats {
        &self.stats
    }
}

/// Global device schedulers
static DEVICE_SCHEDULERS: IrqSafeMutex<BTreeMap<DeviceId, DeviceScheduler>> =
    IrqSafeMutex::new(BTreeMap::new());

/// Default scheduler type
static DEFAULT_SCHEDULER: IrqSafeMutex<SchedulerType> =
    IrqSafeMutex::new(SchedulerType::MqDeadline);

/// Get current time in microseconds
fn current_time_us() -> u64 {
    if crate::arch::tsc::is_enabled() {
        crate::arch::tsc::now_us()
    } else {
        let ts = crate::time::realtime();
        (ts.tv_sec as u64) * 1_000_000 + (ts.tv_nsec as u64) / 1000
    }
}

/// Initialize I/O scheduler subsystem
pub fn init() {
    crate::util::kprintln!("iosched: initializing I/O scheduler subsystem...");
}

/// Register a device
pub fn register_device(device_id: DeviceId) {
    let scheduler_type = *DEFAULT_SCHEDULER.lock();
    let config = SchedulerConfig {
        scheduler_type,
        ..Default::default()
    };
    let scheduler = DeviceScheduler::new(device_id, config);
    DEVICE_SCHEDULERS.lock().insert(device_id, scheduler);
}

/// Unregister a device
pub fn unregister_device(device_id: DeviceId) {
    DEVICE_SCHEDULERS.lock().remove(&device_id);
}

/// Submit I/O request
pub fn submit(device_id: DeviceId, req_type: IoRequestType, lba: Lba, sectors: u32) -> Option<u64> {
    DEVICE_SCHEDULERS.lock().get_mut(&device_id).map(|s| s.submit(req_type, lba, sectors))
}

/// Dispatch requests for a device
pub fn dispatch(device_id: DeviceId) -> Vec<IoRequest> {
    DEVICE_SCHEDULERS.lock().get_mut(&device_id).map(|s| s.dispatch()).unwrap_or_default()
}

/// Complete a request
pub fn complete(device_id: DeviceId, id: u64, latency_us: u64) {
    if let Some(s) = DEVICE_SCHEDULERS.lock().get_mut(&device_id) {
        s.complete(id, latency_us);
    }
}

/// Get device statistics
pub fn stats(device_id: DeviceId) -> Option<SchedulerStats> {
    DEVICE_SCHEDULERS.lock().get(&device_id).map(|s| s.stats().clone())
}

/// Set default scheduler type
pub fn set_default_scheduler(sched_type: SchedulerType) {
    *DEFAULT_SCHEDULER.lock() = sched_type;
}

/// Get default scheduler type
pub fn get_default_scheduler() -> SchedulerType {
    *DEFAULT_SCHEDULER.lock()
}
