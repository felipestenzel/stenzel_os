//! Page Cache - Block I/O caching layer
//!
//! Implements a write-back cache for block devices with LRU eviction policy.
//! Each cached page corresponds to a device block, allowing faster repeated reads
//! and batched writes.
//!
//! Features:
//! - LRU (Least Recently Used) eviction policy
//! - Write-back caching with dirty page tracking
//! - Per-device caching with device ID
//! - Configurable cache size
//! - Sync/flush support

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use spin::RwLock;

use crate::storage::{BlockDevice, BlockDeviceId};
use crate::util::{KError, KResult};

/// Default maximum number of cached pages
const DEFAULT_MAX_PAGES: usize = 1024; // 4MB with 4KB pages

/// Cache statistics
#[derive(Debug, Default)]
pub struct CacheStats {
    pub hits: AtomicU64,
    pub misses: AtomicU64,
    pub evictions: AtomicU64,
    pub writebacks: AtomicU64,
}

impl CacheStats {
    pub const fn new() -> Self {
        Self {
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
            writebacks: AtomicU64::new(0),
        }
    }
}

/// Key for cache entries: (device_id, block_number)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct CacheKey {
    device_id: u32,
    block: u64,
}

impl CacheKey {
    fn new(device_id: BlockDeviceId, block: u64) -> Self {
        Self {
            device_id: device_id.0,
            block,
        }
    }
}

/// A cached page entry
struct CachePage {
    /// The data buffer (block_size bytes)
    data: Vec<u8>,
    /// Whether the page has been modified
    dirty: bool,
    /// Access counter for LRU tracking
    access_count: u64,
    /// Block size (for validation)
    block_size: u32,
}

impl CachePage {
    fn new(data: Vec<u8>, block_size: u32) -> Self {
        Self {
            data,
            dirty: false,
            access_count: 0,
            block_size,
        }
    }

    fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    fn mark_clean(&mut self) {
        self.dirty = false;
    }

    fn touch(&mut self, counter: u64) {
        self.access_count = counter;
    }
}

/// The page cache
pub struct PageCache {
    /// Cached pages indexed by (device_id, block)
    pages: RwLock<BTreeMap<CacheKey, CachePage>>,
    /// Maximum number of pages to cache
    max_pages: AtomicUsize,
    /// Global access counter for LRU
    access_counter: AtomicU64,
    /// Statistics
    stats: CacheStats,
}

impl PageCache {
    /// Create a new page cache with default settings
    pub const fn new() -> Self {
        Self {
            pages: RwLock::new(BTreeMap::new()),
            max_pages: AtomicUsize::new(DEFAULT_MAX_PAGES),
            access_counter: AtomicU64::new(0),
            stats: CacheStats::new(),
        }
    }

    /// Set the maximum number of cached pages
    pub fn set_max_pages(&self, max: usize) {
        self.max_pages.store(max, Ordering::Relaxed);
    }

    /// Get cache statistics
    pub fn stats(&self) -> (u64, u64, u64, u64) {
        (
            self.stats.hits.load(Ordering::Relaxed),
            self.stats.misses.load(Ordering::Relaxed),
            self.stats.evictions.load(Ordering::Relaxed),
            self.stats.writebacks.load(Ordering::Relaxed),
        )
    }

    /// Read a block through the cache
    pub fn read_block(
        &self,
        device: &Arc<dyn BlockDevice>,
        block: u64,
        out: &mut [u8],
    ) -> KResult<()> {
        let key = CacheKey::new(device.id(), block);
        let block_size = device.block_size() as usize;

        if out.len() != block_size {
            return Err(KError::Invalid);
        }

        // Try to read from cache first
        {
            let mut pages = self.pages.write();
            if let Some(page) = pages.get_mut(&key) {
                // Cache hit
                self.stats.hits.fetch_add(1, Ordering::Relaxed);
                let counter = self.access_counter.fetch_add(1, Ordering::Relaxed);
                page.touch(counter);
                out.copy_from_slice(&page.data);
                return Ok(());
            }
        }

        // Cache miss - read from device
        self.stats.misses.fetch_add(1, Ordering::Relaxed);
        device.read_blocks(block, 1, out)?;

        // Add to cache
        self.add_page(device, block, out.to_vec())?;

        Ok(())
    }

    /// Write a block through the cache (write-back)
    pub fn write_block(
        &self,
        device: &Arc<dyn BlockDevice>,
        block: u64,
        data: &[u8],
    ) -> KResult<()> {
        let key = CacheKey::new(device.id(), block);
        let block_size = device.block_size() as usize;

        if data.len() != block_size {
            return Err(KError::Invalid);
        }

        let mut pages = self.pages.write();

        if let Some(page) = pages.get_mut(&key) {
            // Update existing cached page
            page.data.copy_from_slice(data);
            page.mark_dirty();
            let counter = self.access_counter.fetch_add(1, Ordering::Relaxed);
            page.touch(counter);
        } else {
            // Add new page to cache
            drop(pages); // Release lock before potential eviction
            self.add_page_dirty(device, block, data.to_vec())?;
        }

        Ok(())
    }

    /// Write a block directly to device, bypassing cache
    pub fn write_block_sync(
        &self,
        device: &Arc<dyn BlockDevice>,
        block: u64,
        data: &[u8],
    ) -> KResult<()> {
        let key = CacheKey::new(device.id(), block);

        // Update cache if present
        {
            let mut pages = self.pages.write();
            if let Some(page) = pages.get_mut(&key) {
                page.data.copy_from_slice(data);
                page.mark_clean(); // Will be written immediately
            }
        }

        // Write to device
        device.write_blocks(block, 1, data)
    }

    /// Add a page to the cache
    fn add_page(
        &self,
        device: &Arc<dyn BlockDevice>,
        block: u64,
        data: Vec<u8>,
    ) -> KResult<()> {
        let key = CacheKey::new(device.id(), block);
        let block_size = device.block_size();

        // Check if we need to evict
        self.maybe_evict(device)?;

        let counter = self.access_counter.fetch_add(1, Ordering::Relaxed);
        let mut page = CachePage::new(data, block_size);
        page.touch(counter);

        let mut pages = self.pages.write();
        pages.insert(key, page);

        Ok(())
    }

    /// Add a dirty page to the cache
    fn add_page_dirty(
        &self,
        device: &Arc<dyn BlockDevice>,
        block: u64,
        data: Vec<u8>,
    ) -> KResult<()> {
        let key = CacheKey::new(device.id(), block);
        let block_size = device.block_size();

        // Check if we need to evict
        self.maybe_evict(device)?;

        let counter = self.access_counter.fetch_add(1, Ordering::Relaxed);
        let mut page = CachePage::new(data, block_size);
        page.touch(counter);
        page.mark_dirty();

        let mut pages = self.pages.write();
        pages.insert(key, page);

        Ok(())
    }

    /// Evict pages if cache is full (LRU policy)
    fn maybe_evict(&self, device: &Arc<dyn BlockDevice>) -> KResult<()> {
        let max_pages = self.max_pages.load(Ordering::Relaxed);

        let mut pages = self.pages.write();
        while pages.len() >= max_pages {
            // Find the least recently used page
            let lru_key = {
                let mut min_access = u64::MAX;
                let mut min_key: Option<CacheKey> = None;

                for (key, page) in pages.iter() {
                    if page.access_count < min_access {
                        min_access = page.access_count;
                        min_key = Some(*key);
                    }
                }

                min_key
            };

            if let Some(key) = lru_key {
                if let Some(page) = pages.remove(&key) {
                    self.stats.evictions.fetch_add(1, Ordering::Relaxed);

                    // Write back if dirty
                    if page.dirty && key.device_id == device.id().0 {
                        self.stats.writebacks.fetch_add(1, Ordering::Relaxed);
                        // We need to write back the data
                        // Note: This is a simplified approach; in a real implementation
                        // we'd need to keep track of the device reference per page
                        drop(pages);
                        let _ = device.write_blocks(key.block, 1, &page.data);
                        pages = self.pages.write();
                    }
                }
            } else {
                break;
            }
        }

        Ok(())
    }

    /// Sync all dirty pages for a device
    pub fn sync_device(&self, device: &Arc<dyn BlockDevice>) -> KResult<()> {
        let device_id = device.id().0;
        let mut pages = self.pages.write();

        for (key, page) in pages.iter_mut() {
            if key.device_id == device_id && page.dirty {
                device.write_blocks(key.block, 1, &page.data)?;
                page.mark_clean();
                self.stats.writebacks.fetch_add(1, Ordering::Relaxed);
            }
        }

        Ok(())
    }

    /// Sync all dirty pages for all devices
    /// Note: This requires devices to be passed in since we don't store device references
    pub fn sync_all(&self, devices: &[Arc<dyn BlockDevice>]) -> KResult<()> {
        for device in devices {
            self.sync_device(device)?;
        }
        Ok(())
    }

    /// Invalidate all cached pages for a device
    pub fn invalidate_device(&self, device_id: BlockDeviceId) {
        let mut pages = self.pages.write();
        pages.retain(|key, _| key.device_id != device_id.0);
    }

    /// Invalidate a specific block
    pub fn invalidate_block(&self, device_id: BlockDeviceId, block: u64) {
        let key = CacheKey::new(device_id, block);
        let mut pages = self.pages.write();
        pages.remove(&key);
    }

    /// Get current number of cached pages
    pub fn cached_pages(&self) -> usize {
        self.pages.read().len()
    }

    /// Get number of dirty pages
    pub fn dirty_pages(&self) -> usize {
        self.pages.read().values().filter(|p| p.dirty).count()
    }
}

// Global page cache instance
static PAGE_CACHE: PageCache = PageCache::new();

/// Get the global page cache
pub fn cache() -> &'static PageCache {
    &PAGE_CACHE
}

/// Initialize the page cache
pub fn init() {
    crate::kprintln!("page_cache: inicializado (max_pages={})", DEFAULT_MAX_PAGES);
}

/// Wrapper for BlockDevice that uses the page cache
pub struct CachedBlockDevice {
    inner: Arc<dyn BlockDevice>,
}

impl CachedBlockDevice {
    pub fn new(device: Arc<dyn BlockDevice>) -> Self {
        Self { inner: device }
    }

    pub fn read_blocks(&self, lba: u64, count: u32, out: &mut [u8]) -> KResult<()> {
        let block_size = self.inner.block_size() as usize;

        // Read each block through the cache
        for i in 0..count as u64 {
            let offset = i as usize * block_size;
            let block_out = &mut out[offset..offset + block_size];
            PAGE_CACHE.read_block(&self.inner, lba + i, block_out)?;
        }

        Ok(())
    }

    pub fn write_blocks(&self, lba: u64, count: u32, data: &[u8]) -> KResult<()> {
        let block_size = self.inner.block_size() as usize;

        // Write each block through the cache
        for i in 0..count as u64 {
            let offset = i as usize * block_size;
            let block_data = &data[offset..offset + block_size];
            PAGE_CACHE.write_block(&self.inner, lba + i, block_data)?;
        }

        Ok(())
    }

    pub fn sync(&self) -> KResult<()> {
        PAGE_CACHE.sync_device(&self.inner)
    }

    pub fn inner(&self) -> &Arc<dyn BlockDevice> {
        &self.inner
    }
}

/// Read blocks through the global page cache
pub fn read_blocks(
    device: &Arc<dyn BlockDevice>,
    lba: u64,
    count: u32,
    out: &mut [u8],
) -> KResult<()> {
    let block_size = device.block_size() as usize;

    for i in 0..count as u64 {
        let offset = i as usize * block_size;
        let block_out = &mut out[offset..offset + block_size];
        PAGE_CACHE.read_block(device, lba + i, block_out)?;
    }

    Ok(())
}

/// Write blocks through the global page cache
pub fn write_blocks(
    device: &Arc<dyn BlockDevice>,
    lba: u64,
    count: u32,
    data: &[u8],
) -> KResult<()> {
    let block_size = device.block_size() as usize;

    for i in 0..count as u64 {
        let offset = i as usize * block_size;
        let block_data = &data[offset..offset + block_size];
        PAGE_CACHE.write_block(device, lba + i, block_data)?;
    }

    Ok(())
}

/// Sync all dirty pages for a device
pub fn sync(device: &Arc<dyn BlockDevice>) -> KResult<()> {
    PAGE_CACHE.sync_device(device)
}

/// Print cache statistics
pub fn print_stats() {
    let (hits, misses, evictions, writebacks) = PAGE_CACHE.stats();
    let total = hits + misses;
    let hit_rate = if total > 0 {
        (hits as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    crate::kprintln!("page_cache: pages={} dirty={}",
                     PAGE_CACHE.cached_pages(),
                     PAGE_CACHE.dirty_pages());
    crate::kprintln!("page_cache: hits={} misses={} hit_rate={:.1}%",
                     hits, misses, hit_rate);
    crate::kprintln!("page_cache: evictions={} writebacks={}",
                     evictions, writebacks);
}
