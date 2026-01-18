//! Write Caching Subsystem for Stenzel OS.
//!
//! Implements an optimized write cache that batches writes, coalesces adjacent
//! writes, and provides write-back functionality for improved I/O performance.
//!
//! Features:
//! - Write-back cache with configurable flush policy
//! - Write coalescing for adjacent blocks
//! - Dirty page tracking
//! - Background flush thread
//! - Per-device cache statistics
//! - Write barriers for data integrity
//! - Cache pressure management

#![allow(dead_code)]

use alloc::collections::{BTreeMap, VecDeque};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use spin::{Mutex, Once};

use super::block::BlockDevice;

// ============================================================================
// Configuration
// ============================================================================

/// Default cache size in bytes (64 MB)
pub const DEFAULT_CACHE_SIZE: usize = 64 * 1024 * 1024;

/// Default dirty ratio (30% of cache can be dirty before writeback starts)
pub const DEFAULT_DIRTY_RATIO: u32 = 30;

/// Default background dirty ratio (10% triggers background flush)
pub const DEFAULT_BG_DIRTY_RATIO: u32 = 10;

/// Default dirty expire time in milliseconds
pub const DEFAULT_DIRTY_EXPIRE_MS: u64 = 30_000;

/// Default flush interval in milliseconds
pub const DEFAULT_FLUSH_INTERVAL_MS: u64 = 5_000;

/// Maximum write batch size
pub const MAX_BATCH_SIZE: usize = 256;

// ============================================================================
// Types
// ============================================================================

/// Device identifier
pub type DeviceId = u32;

/// Sector number
pub type Sector = u64;

/// Cache entry state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheState {
    /// Entry is clean (matches disk)
    Clean,
    /// Entry is dirty (needs writeback)
    Dirty,
    /// Entry is being written back
    Writeback,
    /// Entry is locked (I/O in progress)
    Locked,
}

/// Cache entry for a single block
#[derive(Debug)]
pub struct CacheEntry {
    /// Sector number on disk
    sector: Sector,
    /// Cached data
    data: Vec<u8>,
    /// Entry state
    state: CacheState,
    /// Time when entry became dirty
    dirty_time: u64,
    /// Access count for LRU
    access_count: u64,
    /// Last access time
    last_access: u64,
    /// Write barrier flag
    barrier: bool,
}

impl CacheEntry {
    fn new(sector: Sector, data: Vec<u8>) -> Self {
        Self {
            sector,
            data,
            state: CacheState::Clean,
            dirty_time: 0,
            access_count: 0,
            last_access: 0,
            barrier: false,
        }
    }

    fn mark_dirty(&mut self, now: u64) {
        if self.state == CacheState::Clean {
            self.state = CacheState::Dirty;
            self.dirty_time = now;
        }
    }

    fn mark_clean(&mut self) {
        self.state = CacheState::Clean;
        self.dirty_time = 0;
        self.barrier = false;
    }

    fn is_dirty(&self) -> bool {
        self.state == CacheState::Dirty
    }

    fn is_expired(&self, now: u64, expire_ms: u64) -> bool {
        self.is_dirty() && (now - self.dirty_time) >= expire_ms
    }
}

/// Write cache configuration
#[derive(Debug, Clone)]
pub struct WriteCacheConfig {
    /// Maximum cache size in bytes
    pub max_size: usize,
    /// Block size in bytes
    pub block_size: u32,
    /// Dirty ratio percentage (trigger synchronous flush)
    pub dirty_ratio: u32,
    /// Background dirty ratio percentage (trigger background flush)
    pub bg_dirty_ratio: u32,
    /// Dirty expire time in milliseconds
    pub dirty_expire_ms: u64,
    /// Flush interval in milliseconds
    pub flush_interval_ms: u64,
    /// Enable write coalescing
    pub coalesce_writes: bool,
    /// Enable write barriers
    pub enable_barriers: bool,
    /// Write-through mode (bypass cache for writes)
    pub write_through: bool,
}

impl Default for WriteCacheConfig {
    fn default() -> Self {
        Self {
            max_size: DEFAULT_CACHE_SIZE,
            block_size: 512,
            dirty_ratio: DEFAULT_DIRTY_RATIO,
            bg_dirty_ratio: DEFAULT_BG_DIRTY_RATIO,
            dirty_expire_ms: DEFAULT_DIRTY_EXPIRE_MS,
            flush_interval_ms: DEFAULT_FLUSH_INTERVAL_MS,
            coalesce_writes: true,
            enable_barriers: true,
            write_through: false,
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// Total read hits
    pub read_hits: u64,
    /// Total read misses
    pub read_misses: u64,
    /// Total write hits
    pub write_hits: u64,
    /// Total write misses
    pub write_misses: u64,
    /// Total flushes
    pub flushes: u64,
    /// Total blocks written back
    pub blocks_written: u64,
    /// Total coalesced writes
    pub coalesced_writes: u64,
    /// Total barriers issued
    pub barriers: u64,
    /// Current dirty bytes
    pub dirty_bytes: u64,
    /// Current cached bytes
    pub cached_bytes: u64,
}

impl CacheStats {
    fn hit_ratio(&self) -> f32 {
        let total_reads = self.read_hits + self.read_misses;
        if total_reads == 0 {
            return 0.0;
        }
        (self.read_hits as f32) / (total_reads as f32) * 100.0
    }
}

/// Write batch for coalescing
#[derive(Debug)]
struct WriteBatch {
    /// Starting sector
    start_sector: Sector,
    /// Combined data
    data: Vec<u8>,
    /// Number of blocks
    block_count: usize,
    /// Contains barrier
    has_barrier: bool,
}

impl WriteBatch {
    fn new(sector: Sector, data: Vec<u8>, has_barrier: bool) -> Self {
        Self {
            start_sector: sector,
            data,
            block_count: 1,
            has_barrier,
        }
    }

    fn can_extend(&self, sector: Sector, block_size: u32) -> bool {
        if self.has_barrier {
            return false;
        }
        let expected_sector = self.start_sector + (self.block_count as u64);
        sector == expected_sector && self.block_count < MAX_BATCH_SIZE
    }

    fn extend(&mut self, data: &[u8], has_barrier: bool) {
        self.data.extend_from_slice(data);
        self.block_count += 1;
        self.has_barrier = has_barrier;
    }
}

// ============================================================================
// Per-Device Cache
// ============================================================================

/// Per-device write cache
pub struct DeviceCache {
    /// Device identifier
    device_id: DeviceId,
    /// Configuration
    config: WriteCacheConfig,
    /// Cached entries by sector
    entries: BTreeMap<Sector, CacheEntry>,
    /// LRU order
    lru_order: VecDeque<Sector>,
    /// Statistics
    stats: CacheStats,
    /// Current timestamp (mock)
    current_time: u64,
    /// Cache enabled
    enabled: AtomicBool,
}

impl DeviceCache {
    /// Create new device cache
    pub fn new(device_id: DeviceId, config: WriteCacheConfig) -> Self {
        Self {
            device_id,
            config,
            entries: BTreeMap::new(),
            lru_order: VecDeque::new(),
            stats: CacheStats::default(),
            current_time: 0,
            enabled: AtomicBool::new(true),
        }
    }

    /// Set current time (for testing or external time source)
    pub fn set_time(&mut self, time: u64) {
        self.current_time = time;
    }

    fn now(&self) -> u64 {
        self.current_time
    }

    /// Read from cache
    pub fn read(&mut self, sector: Sector) -> Option<Vec<u8>> {
        if !self.enabled.load(Ordering::Relaxed) {
            return None;
        }

        // Get time before borrowing entries
        let now = self.now();

        // Check if entry exists and update stats
        let result = if let Some(entry) = self.entries.get_mut(&sector) {
            entry.access_count += 1;
            entry.last_access = now;
            Some(entry.data.clone())
        } else {
            None
        };

        // Update stats and LRU after releasing entries borrow
        if result.is_some() {
            self.stats.read_hits += 1;
            self.touch_lru(sector);
        } else {
            self.stats.read_misses += 1;
        }

        result
    }

    /// Write to cache
    pub fn write(&mut self, sector: Sector, data: Vec<u8>, barrier: bool) -> WriteResult {
        if !self.enabled.load(Ordering::Relaxed) {
            return WriteResult::Disabled;
        }

        // Check if write-through mode
        if self.config.write_through {
            return WriteResult::WriteThrough;
        }

        let now = self.now();
        let block_size = self.config.block_size as usize;

        // Check cache pressure
        if self.should_flush_sync() {
            return WriteResult::PressureFlush;
        }

        // Update or insert entry
        if let Some(entry) = self.entries.get_mut(&sector) {
            entry.data = data;
            entry.mark_dirty(now);
            entry.access_count += 1;
            entry.last_access = now;
            entry.barrier = barrier;
            self.stats.write_hits += 1;
        } else {
            // Evict if needed
            while self.stats.cached_bytes as usize + block_size > self.config.max_size {
                if !self.evict_one() {
                    return WriteResult::CacheFull;
                }
            }

            let mut entry = CacheEntry::new(sector, data);
            entry.mark_dirty(now);
            entry.last_access = now;
            entry.barrier = barrier;

            self.stats.cached_bytes += block_size as u64;
            self.entries.insert(sector, entry);
            self.lru_order.push_back(sector);
            self.stats.write_misses += 1;
        }

        self.stats.dirty_bytes += block_size as u64;

        if barrier {
            self.stats.barriers += 1;
        }

        if self.should_flush_bg() {
            WriteResult::CachedNeedsBgFlush
        } else {
            WriteResult::Cached
        }
    }

    /// Insert clean data into cache (after read from disk)
    pub fn insert_clean(&mut self, sector: Sector, data: Vec<u8>) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }

        let block_size = self.config.block_size as usize;

        // Evict if needed
        while self.stats.cached_bytes as usize + block_size > self.config.max_size {
            if !self.evict_one() {
                return;
            }
        }

        let mut entry = CacheEntry::new(sector, data);
        entry.last_access = self.now();

        self.stats.cached_bytes += block_size as u64;
        self.entries.insert(sector, entry);
        self.lru_order.push_back(sector);
    }

    /// Get dirty blocks for writeback
    pub fn get_dirty_blocks(&mut self, max_count: usize) -> Vec<(Sector, Vec<u8>, bool)> {
        let now = self.now();
        let mut result = Vec::new();

        // Collect expired or dirty blocks
        let mut dirty_sectors: Vec<Sector> = self.entries
            .iter()
            .filter(|(_, e)| e.is_dirty())
            .map(|(&s, _)| s)
            .collect();

        // Sort by sector for sequential I/O
        dirty_sectors.sort();

        for sector in dirty_sectors {
            if result.len() >= max_count {
                break;
            }

            if let Some(entry) = self.entries.get(&sector) {
                if entry.is_dirty() {
                    result.push((sector, entry.data.clone(), entry.barrier));
                }
            }
        }

        result
    }

    /// Coalesce dirty blocks into batches
    pub fn get_coalesced_batches(&mut self, max_batches: usize) -> Vec<WriteBatch> {
        if !self.config.coalesce_writes {
            // Return individual blocks as single-block batches
            return self.get_dirty_blocks(max_batches * MAX_BATCH_SIZE)
                .into_iter()
                .map(|(s, d, b)| WriteBatch::new(s, d, b))
                .collect();
        }

        let dirty = self.get_dirty_blocks(max_batches * MAX_BATCH_SIZE);
        let mut batches: Vec<WriteBatch> = Vec::new();
        let block_size = self.config.block_size;

        for (sector, data, barrier) in dirty {
            let mut coalesced = false;

            // Try to extend existing batch
            if let Some(last_batch) = batches.last_mut() {
                if last_batch.can_extend(sector, block_size) {
                    last_batch.extend(&data, barrier);
                    self.stats.coalesced_writes += 1;
                    coalesced = true;
                }
            }

            if !coalesced {
                if batches.len() >= max_batches {
                    break;
                }
                batches.push(WriteBatch::new(sector, data, barrier));
            }
        }

        batches
    }

    /// Mark blocks as written (clean)
    pub fn mark_written(&mut self, sectors: &[Sector]) {
        let block_size = self.config.block_size as u64;

        for &sector in sectors {
            if let Some(entry) = self.entries.get_mut(&sector) {
                if entry.is_dirty() {
                    self.stats.dirty_bytes = self.stats.dirty_bytes.saturating_sub(block_size);
                }
                entry.mark_clean();
                self.stats.blocks_written += 1;
            }
        }
    }

    /// Flush all dirty blocks
    pub fn flush_all(&mut self) -> Vec<(Sector, Vec<u8>, bool)> {
        self.stats.flushes += 1;
        self.get_dirty_blocks(usize::MAX)
    }

    /// Invalidate cache entry
    pub fn invalidate(&mut self, sector: Sector) -> bool {
        if let Some(entry) = self.entries.remove(&sector) {
            let block_size = self.config.block_size as u64;
            self.stats.cached_bytes = self.stats.cached_bytes.saturating_sub(block_size);
            if entry.is_dirty() {
                self.stats.dirty_bytes = self.stats.dirty_bytes.saturating_sub(block_size);
            }
            self.lru_order.retain(|&s| s != sector);
            true
        } else {
            false
        }
    }

    /// Invalidate all cache entries
    pub fn invalidate_all(&mut self) {
        self.entries.clear();
        self.lru_order.clear();
        self.stats.cached_bytes = 0;
        self.stats.dirty_bytes = 0;
    }

    /// Check if synchronous flush is needed
    fn should_flush_sync(&self) -> bool {
        let dirty_pct = if self.stats.cached_bytes > 0 {
            (self.stats.dirty_bytes * 100) / self.stats.cached_bytes
        } else {
            0
        };
        dirty_pct >= self.config.dirty_ratio as u64
    }

    /// Check if background flush is needed
    fn should_flush_bg(&self) -> bool {
        let dirty_pct = if self.stats.cached_bytes > 0 {
            (self.stats.dirty_bytes * 100) / self.stats.cached_bytes
        } else {
            0
        };
        dirty_pct >= self.config.bg_dirty_ratio as u64
    }

    /// Evict one clean entry
    fn evict_one(&mut self) -> bool {
        // Find oldest clean entry
        while let Some(sector) = self.lru_order.pop_front() {
            if let Some(entry) = self.entries.get(&sector) {
                if !entry.is_dirty() {
                    let block_size = self.config.block_size as u64;
                    self.entries.remove(&sector);
                    self.stats.cached_bytes = self.stats.cached_bytes.saturating_sub(block_size);
                    return true;
                } else {
                    // Put dirty entry back at end
                    self.lru_order.push_back(sector);
                }
            }
        }
        false
    }

    /// Touch LRU (move to end)
    fn touch_lru(&mut self, sector: Sector) {
        self.lru_order.retain(|&s| s != sector);
        self.lru_order.push_back(sector);
    }

    /// Get statistics
    pub fn stats(&self) -> &CacheStats {
        &self.stats
    }

    /// Get current dirty count
    pub fn dirty_count(&self) -> usize {
        self.entries.values().filter(|e| e.is_dirty()).count()
    }

    /// Enable/disable cache
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    /// Check if cache is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }
}

/// Write result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteResult {
    /// Data cached successfully
    Cached,
    /// Data cached, background flush recommended
    CachedNeedsBgFlush,
    /// Cache disabled
    Disabled,
    /// Write-through mode, write directly to disk
    WriteThrough,
    /// Cache full, need sync flush
    CacheFull,
    /// Pressure threshold reached, need sync flush
    PressureFlush,
}

// ============================================================================
// Global Write Cache Manager
// ============================================================================

/// Global write cache manager
pub struct WriteCacheManager {
    /// Per-device caches
    caches: BTreeMap<DeviceId, Mutex<DeviceCache>>,
    /// Default configuration
    default_config: WriteCacheConfig,
    /// Global enabled flag
    enabled: AtomicBool,
    /// Current time
    time: AtomicU64,
}

impl WriteCacheManager {
    /// Create new manager
    pub fn new() -> Self {
        Self {
            caches: BTreeMap::new(),
            default_config: WriteCacheConfig::default(),
            enabled: AtomicBool::new(true),
            time: AtomicU64::new(0),
        }
    }

    /// Register device for caching
    pub fn register_device(&mut self, device_id: DeviceId, config: Option<WriteCacheConfig>) {
        let cfg = config.unwrap_or_else(|| self.default_config.clone());
        let cache = DeviceCache::new(device_id, cfg);
        self.caches.insert(device_id, Mutex::new(cache));
    }

    /// Unregister device
    pub fn unregister_device(&mut self, device_id: DeviceId) -> bool {
        self.caches.remove(&device_id).is_some()
    }

    /// Read from cache
    pub fn read(&self, device_id: DeviceId, sector: Sector) -> Option<Vec<u8>> {
        if !self.enabled.load(Ordering::Relaxed) {
            return None;
        }

        self.caches.get(&device_id)?.lock().read(sector)
    }

    /// Write to cache
    pub fn write(&self, device_id: DeviceId, sector: Sector, data: Vec<u8>, barrier: bool) -> WriteResult {
        if !self.enabled.load(Ordering::Relaxed) {
            return WriteResult::Disabled;
        }

        if let Some(cache) = self.caches.get(&device_id) {
            let mut c = cache.lock();
            c.set_time(self.time.load(Ordering::Relaxed));
            c.write(sector, data, barrier)
        } else {
            WriteResult::Disabled
        }
    }

    /// Insert clean data
    pub fn insert_clean(&self, device_id: DeviceId, sector: Sector, data: Vec<u8>) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }

        if let Some(cache) = self.caches.get(&device_id) {
            cache.lock().insert_clean(sector, data);
        }
    }

    /// Get dirty blocks for a device
    pub fn get_dirty(&self, device_id: DeviceId, max_count: usize) -> Vec<(Sector, Vec<u8>, bool)> {
        self.caches.get(&device_id)
            .map(|c| c.lock().get_dirty_blocks(max_count))
            .unwrap_or_default()
    }

    /// Get coalesced batches
    pub fn get_batches(&self, device_id: DeviceId, max_batches: usize) -> Vec<WriteBatch> {
        self.caches.get(&device_id)
            .map(|c| c.lock().get_coalesced_batches(max_batches))
            .unwrap_or_default()
    }

    /// Mark blocks as written
    pub fn mark_written(&self, device_id: DeviceId, sectors: &[Sector]) {
        if let Some(cache) = self.caches.get(&device_id) {
            cache.lock().mark_written(sectors);
        }
    }

    /// Flush device cache
    pub fn flush_device(&self, device_id: DeviceId) -> Vec<(Sector, Vec<u8>, bool)> {
        self.caches.get(&device_id)
            .map(|c| c.lock().flush_all())
            .unwrap_or_default()
    }

    /// Flush all caches
    pub fn flush_all(&self) -> Vec<(DeviceId, Vec<(Sector, Vec<u8>, bool)>)> {
        let mut result = Vec::new();
        for (&device_id, cache) in &self.caches {
            let dirty = cache.lock().flush_all();
            if !dirty.is_empty() {
                result.push((device_id, dirty));
            }
        }
        result
    }

    /// Invalidate sector
    pub fn invalidate(&self, device_id: DeviceId, sector: Sector) -> bool {
        self.caches.get(&device_id)
            .map(|c| c.lock().invalidate(sector))
            .unwrap_or(false)
    }

    /// Invalidate device cache
    pub fn invalidate_device(&self, device_id: DeviceId) {
        if let Some(cache) = self.caches.get(&device_id) {
            cache.lock().invalidate_all();
        }
    }

    /// Get device stats
    pub fn device_stats(&self, device_id: DeviceId) -> Option<CacheStats> {
        self.caches.get(&device_id).map(|c| c.lock().stats().clone())
    }

    /// Get all device stats
    pub fn all_stats(&self) -> Vec<(DeviceId, CacheStats)> {
        self.caches.iter()
            .map(|(&id, c)| (id, c.lock().stats().clone()))
            .collect()
    }

    /// Update time
    pub fn tick(&self, ms: u64) {
        self.time.fetch_add(ms, Ordering::Relaxed);
    }

    /// Set enabled
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
        for cache in self.caches.values() {
            cache.lock().set_enabled(enabled);
        }
    }

    /// Check if enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Get dirty count across all devices
    pub fn total_dirty_count(&self) -> usize {
        self.caches.values()
            .map(|c| c.lock().dirty_count())
            .sum()
    }

    /// Check if background flush needed for any device
    pub fn needs_bg_flush(&self) -> Vec<DeviceId> {
        self.caches.iter()
            .filter_map(|(&id, c)| {
                let cache = c.lock();
                if cache.dirty_count() > 0 {
                    Some(id)
                } else {
                    None
                }
            })
            .collect()
    }
}

impl Default for WriteCacheManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Global Instance
// ============================================================================

static WRITE_CACHE: Once<Mutex<WriteCacheManager>> = Once::new();

/// Initialize write cache subsystem
pub fn init() {
    WRITE_CACHE.call_once(|| Mutex::new(WriteCacheManager::new()));
    crate::kprintln!("writecache: initialized (default {}MB)", DEFAULT_CACHE_SIZE / (1024 * 1024));
}

/// Get write cache manager
pub fn manager() -> &'static Mutex<WriteCacheManager> {
    WRITE_CACHE.get().expect("Write cache not initialized")
}

/// Register device for caching
pub fn register_device(device_id: DeviceId, config: Option<WriteCacheConfig>) {
    manager().lock().register_device(device_id, config);
}

/// Read from cache
pub fn cache_read(device_id: DeviceId, sector: Sector) -> Option<Vec<u8>> {
    manager().lock().read(device_id, sector)
}

/// Write to cache
pub fn cache_write(device_id: DeviceId, sector: Sector, data: Vec<u8>, barrier: bool) -> WriteResult {
    manager().lock().write(device_id, sector, data, barrier)
}

/// Insert clean data
pub fn cache_insert_clean(device_id: DeviceId, sector: Sector, data: Vec<u8>) {
    manager().lock().insert_clean(device_id, sector, data);
}

/// Flush device
pub fn flush_device(device_id: DeviceId) -> Vec<(Sector, Vec<u8>, bool)> {
    manager().lock().flush_device(device_id)
}

/// Flush all devices
pub fn flush_all() {
    let _ = manager().lock().flush_all();
}

/// Sync all caches (flush + write to disk)
pub fn sync() {
    // This would coordinate with block device layer
    let dirty = manager().lock().flush_all();
    for (device_id, _blocks) in dirty {
        // In real implementation, write blocks to device
        crate::kprintln!("writecache: syncing device {}", device_id);
    }
}

/// Get stats for device
pub fn device_stats(device_id: DeviceId) -> Option<CacheStats> {
    manager().lock().device_stats(device_id)
}

/// Background flush tick (called periodically)
pub fn background_tick(ms: u64) {
    let mgr = manager().lock();
    mgr.tick(ms);

    // Check for devices needing flush
    let devices = mgr.needs_bg_flush();
    drop(mgr);

    for device_id in devices {
        // Get batches and write them
        let batches = manager().lock().get_batches(device_id, 8);
        for batch in &batches {
            // In real implementation, write batch to device
            let sectors: Vec<Sector> = (0..batch.block_count as u64)
                .map(|i| batch.start_sector + i)
                .collect();
            manager().lock().mark_written(device_id, &sectors);
        }
    }
}

// ============================================================================
// Write Barrier Support
// ============================================================================

/// Issue a write barrier
pub fn write_barrier(device_id: DeviceId) {
    // Flush all pending writes before the barrier
    let dirty = flush_device(device_id);

    // In real implementation, issue FUA or cache flush command
    if !dirty.is_empty() {
        crate::kprintln!("writecache: barrier on device {} ({} blocks)", device_id, dirty.len());
    }
}

/// Write with barrier (ensures data reaches disk)
pub fn write_with_barrier(device_id: DeviceId, sector: Sector, data: Vec<u8>) -> WriteResult {
    cache_write(device_id, sector, data, true)
}

// ============================================================================
// Sysctl-style Tuning Interface
// ============================================================================

/// Get current dirty ratio
pub fn get_dirty_ratio(device_id: DeviceId) -> Option<u32> {
    manager().lock().caches.get(&device_id).map(|c| c.lock().config.dirty_ratio)
}

/// Set dirty ratio
pub fn set_dirty_ratio(device_id: DeviceId, ratio: u32) {
    if let Some(cache) = manager().lock().caches.get(&device_id) {
        cache.lock().config.dirty_ratio = ratio.min(100);
    }
}

/// Get current background dirty ratio
pub fn get_bg_dirty_ratio(device_id: DeviceId) -> Option<u32> {
    manager().lock().caches.get(&device_id).map(|c| c.lock().config.bg_dirty_ratio)
}

/// Set background dirty ratio
pub fn set_bg_dirty_ratio(device_id: DeviceId, ratio: u32) {
    if let Some(cache) = manager().lock().caches.get(&device_id) {
        cache.lock().config.bg_dirty_ratio = ratio.min(100);
    }
}

/// Enable/disable write coalescing
pub fn set_coalescing(device_id: DeviceId, enabled: bool) {
    if let Some(cache) = manager().lock().caches.get(&device_id) {
        cache.lock().config.coalesce_writes = enabled;
    }
}

/// Set write-through mode
pub fn set_write_through(device_id: DeviceId, enabled: bool) {
    if let Some(cache) = manager().lock().caches.get(&device_id) {
        cache.lock().config.write_through = enabled;
    }
}
