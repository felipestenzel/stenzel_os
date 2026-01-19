//! Boot Cache
//!
//! Caches files accessed during boot for faster subsequent boots.
//! Tracks file access patterns during boot and preloads them on next boot.

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;

use crate::sync::IrqSafeMutex;

/// Maximum number of files to cache
const MAX_CACHED_FILES: usize = 1024;

/// Maximum cache size in bytes (64 MB default)
const MAX_CACHE_SIZE: usize = 64 * 1024 * 1024;

/// Boot phase
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootPhase {
    /// Early boot (kernel initialization)
    Early,
    /// Driver initialization
    Drivers,
    /// Filesystem mount
    Filesystem,
    /// Service startup
    Services,
    /// User session
    UserSession,
    /// Boot complete
    Complete,
}

impl BootPhase {
    pub fn name(&self) -> &'static str {
        match self {
            BootPhase::Early => "early",
            BootPhase::Drivers => "drivers",
            BootPhase::Filesystem => "filesystem",
            BootPhase::Services => "services",
            BootPhase::UserSession => "user_session",
            BootPhase::Complete => "complete",
        }
    }

    pub fn priority(&self) -> u8 {
        match self {
            BootPhase::Early => 0,
            BootPhase::Drivers => 1,
            BootPhase::Filesystem => 2,
            BootPhase::Services => 3,
            BootPhase::UserSession => 4,
            BootPhase::Complete => 5,
        }
    }
}

/// File access record
#[derive(Debug, Clone)]
pub struct FileAccessRecord {
    /// File path
    pub path: String,
    /// Boot phase when accessed
    pub phase: BootPhase,
    /// Timestamp (boot-relative ms)
    pub timestamp_ms: u64,
    /// File size in bytes
    pub size: usize,
    /// Number of times accessed during boot
    pub access_count: u32,
    /// Average read offset (for sequential detection)
    pub avg_offset: u64,
    /// Is sequential access pattern
    pub is_sequential: bool,
}

impl FileAccessRecord {
    pub fn new(path: String, phase: BootPhase, timestamp_ms: u64, size: usize) -> Self {
        Self {
            path,
            phase,
            timestamp_ms,
            size,
            access_count: 1,
            avg_offset: 0,
            is_sequential: true,
        }
    }
}

/// Cached file data
#[derive(Debug)]
pub struct CachedFile {
    /// File path
    pub path: String,
    /// File data
    pub data: Vec<u8>,
    /// Last access timestamp
    pub last_access_ms: u64,
    /// Access count since cache
    pub access_count: u32,
}

/// Boot cache configuration
#[derive(Debug, Clone)]
pub struct BootCacheConfig {
    /// Enable boot cache
    pub enabled: bool,
    /// Maximum cache size in bytes
    pub max_size: usize,
    /// Maximum number of files
    pub max_files: usize,
    /// Minimum file size to cache
    pub min_file_size: usize,
    /// Maximum file size to cache
    pub max_file_size: usize,
    /// Enable profiling (record access patterns)
    pub profiling_enabled: bool,
    /// Enable preloading on boot
    pub preload_enabled: bool,
    /// Preload ahead of time (ms)
    pub preload_ahead_ms: u64,
}

impl Default for BootCacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_size: MAX_CACHE_SIZE,
            max_files: MAX_CACHED_FILES,
            min_file_size: 4096,
            max_file_size: 16 * 1024 * 1024, // 16 MB
            profiling_enabled: true,
            preload_enabled: true,
            preload_ahead_ms: 100,
        }
    }
}

/// Boot cache statistics
#[derive(Debug, Default, Clone)]
pub struct BootCacheStats {
    /// Total cache hits
    pub hits: u64,
    /// Total cache misses
    pub misses: u64,
    /// Bytes served from cache
    pub bytes_served: u64,
    /// Files currently cached
    pub files_cached: usize,
    /// Current cache size
    pub cache_size: usize,
    /// Files profiled this boot
    pub files_profiled: usize,
    /// Time saved (estimated ms)
    pub time_saved_ms: u64,
}

impl BootCacheStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            (self.hits as f64 / total as f64) * 100.0
        }
    }
}

/// Boot cache manager
pub struct BootCache {
    /// Configuration
    config: BootCacheConfig,
    /// Current boot phase
    phase: BootPhase,
    /// Boot start timestamp
    boot_start_ms: u64,
    /// File access records (for profiling)
    access_records: BTreeMap<String, FileAccessRecord>,
    /// Cached file data
    cached_files: BTreeMap<String, CachedFile>,
    /// Current cache size
    current_size: usize,
    /// Statistics
    stats: BootCacheStats,
    /// Is recording (profiling mode)
    is_recording: bool,
    /// Preload list from previous boot
    preload_list: Vec<FileAccessRecord>,
}

impl BootCache {
    /// Create new boot cache
    pub fn new() -> Self {
        Self {
            config: BootCacheConfig::default(),
            phase: BootPhase::Early,
            boot_start_ms: 0,
            access_records: BTreeMap::new(),
            cached_files: BTreeMap::new(),
            current_size: 0,
            stats: BootCacheStats::default(),
            is_recording: false,
            preload_list: Vec::new(),
        }
    }

    /// Initialize boot cache
    pub fn init(&mut self, boot_start_ms: u64) {
        self.boot_start_ms = boot_start_ms;
        if self.config.profiling_enabled {
            self.is_recording = true;
        }
    }

    /// Set boot phase
    pub fn set_phase(&mut self, phase: BootPhase) {
        self.phase = phase;
        if phase == BootPhase::Complete {
            self.is_recording = false;
        }
    }

    /// Get current phase
    pub fn phase(&self) -> BootPhase {
        self.phase
    }

    /// Record file access (during profiling)
    pub fn record_access(&mut self, path: &str, size: usize, offset: u64) {
        if !self.is_recording || !self.config.enabled {
            return;
        }

        let timestamp = self.current_time_ms();
        let path_str = path.to_string();

        if let Some(record) = self.access_records.get_mut(&path_str) {
            record.access_count += 1;
            // Update sequential detection
            let expected_offset = record.avg_offset + (record.size as u64 / record.access_count as u64);
            if offset != expected_offset && offset != 0 {
                record.is_sequential = false;
            }
            record.avg_offset = (record.avg_offset + offset) / 2;
        } else {
            if self.access_records.len() < self.config.max_files {
                self.access_records.insert(
                    path_str.clone(),
                    FileAccessRecord::new(path_str, self.phase, timestamp, size),
                );
                self.stats.files_profiled += 1;
            }
        }
    }

    /// Try to get file from cache
    pub fn get(&mut self, path: &str) -> Option<&[u8]> {
        if !self.config.enabled {
            return None;
        }

        // Get current time before mutable borrow
        let current_time = self.current_time_ms();

        if let Some(cached) = self.cached_files.get_mut(path) {
            cached.last_access_ms = current_time;
            cached.access_count += 1;
            let data_len = cached.data.len();
            self.stats.hits += 1;
            self.stats.bytes_served += data_len as u64;
            // Estimate time saved (assume 1ms per 64KB from disk)
            self.stats.time_saved_ms += (data_len / (64 * 1024)) as u64 + 1;
        } else {
            self.stats.misses += 1;
            return None;
        }

        // Re-borrow immutably to return data
        self.cached_files.get(path).map(|f| f.data.as_slice())
    }

    /// Check if file is in cache
    pub fn contains(&self, path: &str) -> bool {
        self.cached_files.contains_key(path)
    }

    /// Add file to cache
    pub fn cache_file(&mut self, path: &str, data: Vec<u8>) -> bool {
        if !self.config.enabled {
            return false;
        }

        let size = data.len();

        // Check size limits
        if size < self.config.min_file_size || size > self.config.max_file_size {
            return false;
        }

        // Check if we have room
        if self.cached_files.len() >= self.config.max_files {
            self.evict_one();
        }

        // Check total size
        while self.current_size + size > self.config.max_size && !self.cached_files.is_empty() {
            self.evict_one();
        }

        if self.current_size + size > self.config.max_size {
            return false;
        }

        // Add to cache
        self.cached_files.insert(path.to_string(), CachedFile {
            path: path.to_string(),
            data,
            last_access_ms: self.current_time_ms(),
            access_count: 0,
        });

        self.current_size += size;
        self.stats.files_cached = self.cached_files.len();
        self.stats.cache_size = self.current_size;

        true
    }

    /// Evict least recently used file
    fn evict_one(&mut self) {
        if self.cached_files.is_empty() {
            return;
        }

        // Find LRU file
        let lru_path = self.cached_files.iter()
            .min_by_key(|(_, f)| f.last_access_ms)
            .map(|(path, _)| path.clone());

        if let Some(path) = lru_path {
            if let Some(removed) = self.cached_files.remove(&path) {
                self.current_size -= removed.data.len();
            }
        }

        self.stats.files_cached = self.cached_files.len();
        self.stats.cache_size = self.current_size;
    }

    /// Remove file from cache
    pub fn invalidate(&mut self, path: &str) {
        if let Some(removed) = self.cached_files.remove(path) {
            self.current_size -= removed.data.len();
            self.stats.files_cached = self.cached_files.len();
            self.stats.cache_size = self.current_size;
        }
    }

    /// Clear all cache
    pub fn clear(&mut self) {
        self.cached_files.clear();
        self.current_size = 0;
        self.stats.files_cached = 0;
        self.stats.cache_size = 0;
    }

    /// Get preload list for next boot
    pub fn get_preload_list(&self) -> Vec<FileAccessRecord> {
        let mut records: Vec<_> = self.access_records.values().cloned().collect();

        // Sort by phase priority and timestamp
        records.sort_by(|a, b| {
            let phase_cmp = a.phase.priority().cmp(&b.phase.priority());
            if phase_cmp != core::cmp::Ordering::Equal {
                phase_cmp
            } else {
                a.timestamp_ms.cmp(&b.timestamp_ms)
            }
        });

        // Filter by criteria
        records.into_iter()
            .filter(|r| {
                r.size >= self.config.min_file_size &&
                r.size <= self.config.max_file_size &&
                r.access_count > 0
            })
            .take(self.config.max_files)
            .collect()
    }

    /// Set preload list from previous boot
    pub fn set_preload_list(&mut self, list: Vec<FileAccessRecord>) {
        self.preload_list = list;
    }

    /// Get files to preload for current phase
    pub fn get_phase_preload(&self) -> Vec<&FileAccessRecord> {
        let current_time = self.current_time_ms();
        let ahead = self.config.preload_ahead_ms;

        self.preload_list.iter()
            .filter(|r| {
                r.phase.priority() <= self.phase.priority() &&
                r.timestamp_ms <= current_time + ahead
            })
            .collect()
    }

    /// Serialize preload list for persistence
    pub fn serialize_preload_list(&self) -> Vec<u8> {
        let list = self.get_preload_list();
        let mut data = Vec::new();

        // Simple format: count, then entries
        let count = list.len() as u32;
        data.extend_from_slice(&count.to_le_bytes());

        for record in &list {
            // Path length and path
            let path_bytes = record.path.as_bytes();
            let path_len = path_bytes.len() as u32;
            data.extend_from_slice(&path_len.to_le_bytes());
            data.extend_from_slice(path_bytes);

            // Phase
            data.push(record.phase.priority());

            // Timestamp
            data.extend_from_slice(&record.timestamp_ms.to_le_bytes());

            // Size
            data.extend_from_slice(&(record.size as u64).to_le_bytes());

            // Access count
            data.extend_from_slice(&record.access_count.to_le_bytes());
        }

        data
    }

    /// Deserialize preload list
    pub fn deserialize_preload_list(&mut self, data: &[u8]) -> bool {
        if data.len() < 4 {
            return false;
        }

        let mut offset = 0;

        // Read count
        let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        offset += 4;

        let mut list = Vec::with_capacity(count);

        for _ in 0..count {
            if offset + 4 > data.len() {
                return false;
            }

            // Path length
            let path_len = u32::from_le_bytes([
                data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
            ]) as usize;
            offset += 4;

            if offset + path_len > data.len() {
                return false;
            }

            // Path
            let path = match core::str::from_utf8(&data[offset..offset + path_len]) {
                Ok(s) => s.to_string(),
                Err(_) => return false,
            };
            offset += path_len;

            if offset + 1 + 8 + 8 + 4 > data.len() {
                return false;
            }

            // Phase
            let phase_priority = data[offset];
            offset += 1;
            let phase = match phase_priority {
                0 => BootPhase::Early,
                1 => BootPhase::Drivers,
                2 => BootPhase::Filesystem,
                3 => BootPhase::Services,
                4 => BootPhase::UserSession,
                _ => BootPhase::Complete,
            };

            // Timestamp
            let timestamp_ms = u64::from_le_bytes([
                data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
                data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7],
            ]);
            offset += 8;

            // Size
            let size = u64::from_le_bytes([
                data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
                data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7],
            ]) as usize;
            offset += 8;

            // Access count
            let access_count = u32::from_le_bytes([
                data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
            ]);
            offset += 4;

            list.push(FileAccessRecord {
                path,
                phase,
                timestamp_ms,
                size,
                access_count,
                avg_offset: 0,
                is_sequential: true,
            });
        }

        self.preload_list = list;
        true
    }

    /// Get current time in ms since boot
    fn current_time_ms(&self) -> u64 {
        // In real implementation, get actual time
        // For now, use placeholder
        crate::time::uptime_ms().saturating_sub(self.boot_start_ms)
    }

    /// Get configuration
    pub fn config(&self) -> &BootCacheConfig {
        &self.config
    }

    /// Set configuration
    pub fn set_config(&mut self, config: BootCacheConfig) {
        self.config = config;
    }

    /// Enable boot cache
    pub fn enable(&mut self) {
        self.config.enabled = true;
    }

    /// Disable boot cache
    pub fn disable(&mut self) {
        self.config.enabled = false;
    }

    /// Check if enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get statistics
    pub fn stats(&self) -> &BootCacheStats {
        &self.stats
    }

    /// Format status string
    pub fn format_status(&self) -> String {
        alloc::format!(
            "Boot Cache: {} | Files: {} | Size: {} KB | Hit rate: {:.1}% | Time saved: {} ms",
            if self.config.enabled { "enabled" } else { "disabled" },
            self.stats.files_cached,
            self.stats.cache_size / 1024,
            self.stats.hit_rate(),
            self.stats.time_saved_ms
        )
    }
}

impl Default for BootCache {
    fn default() -> Self {
        Self::new()
    }
}

// Global boot cache instance
static BOOT_CACHE: IrqSafeMutex<Option<BootCache>> = IrqSafeMutex::new(None);

/// Initialize boot cache
pub fn init() {
    let mut cache = BOOT_CACHE.lock();
    let mut bc = BootCache::new();
    bc.init(crate::time::uptime_ms());
    *cache = Some(bc);
    crate::kprintln!("mm: Boot cache initialized");
}

/// Set boot phase
pub fn set_phase(phase: BootPhase) {
    if let Some(ref mut cache) = *BOOT_CACHE.lock() {
        cache.set_phase(phase);
    }
}

/// Record file access
pub fn record_access(path: &str, size: usize, offset: u64) {
    if let Some(ref mut cache) = *BOOT_CACHE.lock() {
        cache.record_access(path, size, offset);
    }
}

/// Get file from cache
pub fn get(path: &str) -> Option<Vec<u8>> {
    BOOT_CACHE.lock().as_mut().and_then(|cache| {
        cache.get(path).map(|data| data.to_vec())
    })
}

/// Cache file
pub fn cache_file(path: &str, data: Vec<u8>) -> bool {
    BOOT_CACHE.lock().as_mut().map(|cache| {
        cache.cache_file(path, data)
    }).unwrap_or(false)
}

/// Invalidate cache entry
pub fn invalidate(path: &str) {
    if let Some(ref mut cache) = *BOOT_CACHE.lock() {
        cache.invalidate(path);
    }
}

/// Clear cache
pub fn clear() {
    if let Some(ref mut cache) = *BOOT_CACHE.lock() {
        cache.clear();
    }
}

/// Get statistics
pub fn stats() -> Option<BootCacheStats> {
    BOOT_CACHE.lock().as_ref().map(|cache| cache.stats().clone())
}

/// Get status string
pub fn status() -> String {
    BOOT_CACHE.lock().as_ref()
        .map(|cache| cache.format_status())
        .unwrap_or_else(|| "Boot cache not initialized".to_string())
}
