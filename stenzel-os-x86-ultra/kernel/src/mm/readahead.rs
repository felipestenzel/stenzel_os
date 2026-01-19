//! Readahead - Predictive File Preloading
//!
//! Preloads file data into page cache based on sequential access patterns.

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::sync::IrqSafeMutex;

/// Default readahead window size (128 KB)
const DEFAULT_READAHEAD_SIZE: usize = 128 * 1024;

/// Maximum readahead window (2 MB)
const MAX_READAHEAD_SIZE: usize = 2 * 1024 * 1024;

/// Minimum readahead window (16 KB)
const MIN_READAHEAD_SIZE: usize = 16 * 1024;

/// Readahead state for a file
#[derive(Debug, Clone)]
pub struct ReadaheadState {
    /// File identifier
    pub file_id: u64,
    /// Current readahead window start
    pub start: u64,
    /// Current readahead window size
    pub size: usize,
    /// Async readahead trigger point
    pub async_size: usize,
    /// Previous read position
    pub prev_pos: u64,
    /// Previous read index (for pattern detection)
    pub prev_index: u64,
    /// Number of sequential accesses
    pub sequential_count: u32,
    /// Number of random accesses
    pub random_count: u32,
    /// Mismatch count (failed predictions)
    pub mismatch_count: u32,
    /// Is currently doing async readahead
    pub async_pending: bool,
}

impl ReadaheadState {
    pub fn new(file_id: u64) -> Self {
        Self {
            file_id,
            start: 0,
            size: DEFAULT_READAHEAD_SIZE,
            async_size: DEFAULT_READAHEAD_SIZE / 2,
            prev_pos: 0,
            prev_index: 0,
            sequential_count: 0,
            random_count: 0,
            mismatch_count: 0,
            async_pending: false,
        }
    }

    /// Check if access pattern is sequential
    pub fn is_sequential(&self) -> bool {
        self.sequential_count > self.random_count + 2
    }

    /// Detect access pattern from new position
    pub fn update_pattern(&mut self, pos: u64, len: usize, page_size: usize) {
        let index = pos / page_size as u64;
        let expected = self.prev_index + 1;

        if index == expected || (index == self.prev_index && pos == self.prev_pos) {
            // Sequential or same page
            self.sequential_count = self.sequential_count.saturating_add(1);
        } else if index > self.prev_index && index <= self.prev_index + 2 {
            // Small skip, still somewhat sequential
            self.sequential_count = self.sequential_count.saturating_add(1);
        } else {
            // Random access
            self.random_count = self.random_count.saturating_add(1);
            // Reset readahead on random access
            if self.random_count > 3 {
                self.size = MIN_READAHEAD_SIZE;
                self.async_size = MIN_READAHEAD_SIZE / 2;
                self.sequential_count = 0;
            }
        }

        self.prev_pos = pos + len as u64;
        self.prev_index = (pos + len as u64 - 1) / page_size as u64;
    }

    /// Grow readahead window after successful prediction
    pub fn grow_window(&mut self) {
        if self.is_sequential() {
            // Double the window size on sequential success
            self.size = core::cmp::min(self.size * 2, MAX_READAHEAD_SIZE);
            self.async_size = self.size / 2;
            self.mismatch_count = 0;
        }
    }

    /// Shrink readahead window after mismatch
    pub fn shrink_window(&mut self) {
        self.mismatch_count += 1;
        if self.mismatch_count > 2 {
            // Halve the window size on repeated mismatches
            self.size = core::cmp::max(self.size / 2, MIN_READAHEAD_SIZE);
            self.async_size = self.size / 2;
        }
    }

    /// Check if position triggers async readahead
    pub fn should_async_readahead(&self, pos: u64, page_size: usize) -> bool {
        if !self.is_sequential() || self.async_pending {
            return false;
        }

        // Trigger async readahead when we hit the async trigger point
        let trigger_point = self.start + self.async_size as u64;
        pos >= trigger_point && pos < self.start + self.size as u64
    }

    /// Get readahead range for a position
    pub fn get_readahead_range(&self, pos: u64, file_size: u64, page_size: usize) -> Option<(u64, usize)> {
        if !self.is_sequential() {
            return None;
        }

        // Align to page boundary
        let page_mask = !(page_size as u64 - 1);
        let aligned_pos = pos & page_mask;

        let ra_start = self.start + self.size as u64;
        let ra_end = core::cmp::min(ra_start + self.size as u64, file_size);

        if ra_start < ra_end {
            Some((ra_start, (ra_end - ra_start) as usize))
        } else {
            None
        }
    }
}

/// Readahead configuration
#[derive(Debug, Clone)]
pub struct ReadaheadConfig {
    /// Enable readahead
    pub enabled: bool,
    /// Default readahead size
    pub default_size: usize,
    /// Maximum readahead size
    pub max_size: usize,
    /// Minimum readahead size
    pub min_size: usize,
    /// Page size
    pub page_size: usize,
    /// Enable async readahead
    pub async_enabled: bool,
    /// Maximum concurrent async reads
    pub max_async_reads: usize,
}

impl Default for ReadaheadConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_size: DEFAULT_READAHEAD_SIZE,
            max_size: MAX_READAHEAD_SIZE,
            min_size: MIN_READAHEAD_SIZE,
            page_size: 4096,
            async_enabled: true,
            max_async_reads: 8,
        }
    }
}

/// Readahead statistics
#[derive(Debug, Default, Clone)]
pub struct ReadaheadStats {
    /// Total readahead requests
    pub requests: u64,
    /// Total pages read ahead
    pub pages_read: u64,
    /// Async readahead triggers
    pub async_triggers: u64,
    /// Hits (readahead data was used)
    pub hits: u64,
    /// Misses (readahead data was not used)
    pub misses: u64,
    /// Sequential patterns detected
    pub sequential_detected: u64,
    /// Random patterns detected
    pub random_detected: u64,
}

impl ReadaheadStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            (self.hits as f64 / total as f64) * 100.0
        }
    }
}

/// Pending async readahead request
#[derive(Debug, Clone)]
pub struct AsyncReadaheadRequest {
    /// File ID
    pub file_id: u64,
    /// Start offset
    pub start: u64,
    /// Length
    pub len: usize,
    /// Priority (lower = higher priority)
    pub priority: u8,
}

/// Readahead manager
pub struct ReadaheadManager {
    /// Configuration
    config: ReadaheadConfig,
    /// Per-file readahead state
    states: BTreeMap<u64, ReadaheadState>,
    /// Pending async requests
    async_queue: Vec<AsyncReadaheadRequest>,
    /// Statistics
    stats: ReadaheadStats,
}

impl ReadaheadManager {
    /// Create new readahead manager
    pub fn new() -> Self {
        Self {
            config: ReadaheadConfig::default(),
            states: BTreeMap::new(),
            async_queue: Vec::new(),
            stats: ReadaheadStats::default(),
        }
    }

    /// Get or create readahead state for a file
    fn get_or_create_state(&mut self, file_id: u64) -> &mut ReadaheadState {
        self.states.entry(file_id).or_insert_with(|| ReadaheadState::new(file_id))
    }

    /// Record a file read and check for readahead
    pub fn on_read(&mut self, file_id: u64, pos: u64, len: usize, file_size: u64) -> Option<(u64, usize)> {
        if !self.config.enabled {
            return None;
        }

        let page_size = self.config.page_size;
        let async_enabled = self.config.async_enabled;

        // Ensure state exists
        if !self.states.contains_key(&file_id) {
            self.states.insert(file_id, ReadaheadState::new(file_id));
        }

        // Variables to collect from state
        let (should_async, async_range, is_seq, ra_range);

        // First pass: update pattern and check conditions
        {
            let state = self.states.get_mut(&file_id).unwrap();
            state.update_pattern(pos, len, page_size);
            should_async = async_enabled && state.should_async_readahead(pos, page_size);
            async_range = if should_async {
                state.get_readahead_range(pos, file_size, page_size)
            } else {
                None
            };
            is_seq = state.is_sequential();
            ra_range = if is_seq {
                state.get_readahead_range(pos, file_size, page_size)
            } else {
                None
            };
        }

        // Process async readahead
        if let Some((start, ra_len)) = async_range {
            self.queue_async_readahead(file_id, start, ra_len);
            if let Some(state) = self.states.get_mut(&file_id) {
                state.async_pending = true;
            }
            self.stats.async_triggers += 1;
        }

        // Process sequential readahead
        if is_seq {
            self.stats.sequential_detected += 1;
            if let Some((start, ra_len)) = ra_range {
                self.stats.requests += 1;
                self.stats.pages_read += (ra_len / page_size) as u64;

                // Update state
                if let Some(state) = self.states.get_mut(&file_id) {
                    state.start = start;
                    state.grow_window();
                }

                return Some((start, ra_len));
            }
        } else {
            self.stats.random_detected += 1;
        }

        None
    }

    /// Record a readahead hit (data was actually used)
    pub fn record_hit(&mut self, file_id: u64) {
        self.stats.hits += 1;
        if let Some(state) = self.states.get_mut(&file_id) {
            state.grow_window();
        }
    }

    /// Record a readahead miss (data was not used)
    pub fn record_miss(&mut self, file_id: u64) {
        self.stats.misses += 1;
        if let Some(state) = self.states.get_mut(&file_id) {
            state.shrink_window();
        }
    }

    /// Queue async readahead request
    fn queue_async_readahead(&mut self, file_id: u64, start: u64, len: usize) {
        if self.async_queue.len() >= self.config.max_async_reads {
            // Drop lowest priority request
            if let Some(idx) = self.async_queue.iter().position(|r| r.priority > 0) {
                self.async_queue.remove(idx);
            } else {
                return; // Queue full
            }
        }

        self.async_queue.push(AsyncReadaheadRequest {
            file_id,
            start,
            len,
            priority: 0,
        });
    }

    /// Get next async readahead request
    pub fn pop_async_request(&mut self) -> Option<AsyncReadaheadRequest> {
        if self.async_queue.is_empty() {
            return None;
        }

        // Sort by priority and pop
        self.async_queue.sort_by_key(|r| r.priority);
        Some(self.async_queue.remove(0))
    }

    /// Mark async readahead as complete
    pub fn complete_async(&mut self, file_id: u64) {
        if let Some(state) = self.states.get_mut(&file_id) {
            state.async_pending = false;
        }
    }

    /// Reset readahead state for a file
    pub fn reset(&mut self, file_id: u64) {
        self.states.remove(&file_id);
    }

    /// Set readahead size for a file
    pub fn set_size(&mut self, file_id: u64, size: usize) {
        let size = size.clamp(self.config.min_size, self.config.max_size);
        let state = self.get_or_create_state(file_id);
        state.size = size;
        state.async_size = size / 2;
    }

    /// Get readahead size for a file
    pub fn get_size(&self, file_id: u64) -> usize {
        self.states.get(&file_id)
            .map(|s| s.size)
            .unwrap_or(self.config.default_size)
    }

    /// Enable readahead
    pub fn enable(&mut self) {
        self.config.enabled = true;
    }

    /// Disable readahead
    pub fn disable(&mut self) {
        self.config.enabled = false;
    }

    /// Check if enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get configuration
    pub fn config(&self) -> &ReadaheadConfig {
        &self.config
    }

    /// Set configuration
    pub fn set_config(&mut self, config: ReadaheadConfig) {
        self.config = config;
    }

    /// Get statistics
    pub fn stats(&self) -> &ReadaheadStats {
        &self.stats
    }

    /// Clear all state
    pub fn clear(&mut self) {
        self.states.clear();
        self.async_queue.clear();
    }

    /// Format status string
    pub fn format_status(&self) -> String {
        alloc::format!(
            "Readahead: {} | Files: {} | Pages: {} | Hit rate: {:.1}% | Async: {}",
            if self.config.enabled { "enabled" } else { "disabled" },
            self.states.len(),
            self.stats.pages_read,
            self.stats.hit_rate(),
            self.async_queue.len()
        )
    }
}

impl Default for ReadaheadManager {
    fn default() -> Self {
        Self::new()
    }
}

// Global readahead manager
static READAHEAD: IrqSafeMutex<Option<ReadaheadManager>> = IrqSafeMutex::new(None);

/// Initialize readahead
pub fn init() {
    let mut ra = READAHEAD.lock();
    *ra = Some(ReadaheadManager::new());
    crate::kprintln!("mm: Readahead initialized");
}

/// Process a file read
pub fn on_read(file_id: u64, pos: u64, len: usize, file_size: u64) -> Option<(u64, usize)> {
    READAHEAD.lock().as_mut().and_then(|ra| ra.on_read(file_id, pos, len, file_size))
}

/// Record hit
pub fn record_hit(file_id: u64) {
    if let Some(ref mut ra) = *READAHEAD.lock() {
        ra.record_hit(file_id);
    }
}

/// Record miss
pub fn record_miss(file_id: u64) {
    if let Some(ref mut ra) = *READAHEAD.lock() {
        ra.record_miss(file_id);
    }
}

/// Reset file state
pub fn reset(file_id: u64) {
    if let Some(ref mut ra) = *READAHEAD.lock() {
        ra.reset(file_id);
    }
}

/// Set readahead size
pub fn set_size(file_id: u64, size: usize) {
    if let Some(ref mut ra) = *READAHEAD.lock() {
        ra.set_size(file_id, size);
    }
}

/// Get readahead size
pub fn get_size(file_id: u64) -> usize {
    READAHEAD.lock().as_ref().map(|ra| ra.get_size(file_id)).unwrap_or(DEFAULT_READAHEAD_SIZE)
}

/// Enable readahead
pub fn enable() {
    if let Some(ref mut ra) = *READAHEAD.lock() {
        ra.enable();
    }
}

/// Disable readahead
pub fn disable() {
    if let Some(ref mut ra) = *READAHEAD.lock() {
        ra.disable();
    }
}

/// Check if enabled
pub fn is_enabled() -> bool {
    READAHEAD.lock().as_ref().map(|ra| ra.is_enabled()).unwrap_or(false)
}

/// Get statistics
pub fn stats() -> Option<ReadaheadStats> {
    READAHEAD.lock().as_ref().map(|ra| ra.stats().clone())
}

/// Get status string
pub fn status() -> String {
    READAHEAD.lock().as_ref()
        .map(|ra| ra.format_status())
        .unwrap_or_else(|| "Readahead not initialized".to_string())
}
