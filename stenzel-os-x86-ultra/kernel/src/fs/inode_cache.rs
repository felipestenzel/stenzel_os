//! Inode Cache
//!
//! The inode cache stores inode objects to avoid repeated disk I/O
//! for frequently accessed files. Inodes are cached by (device, ino)
//! pair.
//!
//! Features:
//! - Hash table for O(1) lookup by inode number
//! - LRU eviction when cache is full
//! - Reference counting for active inodes
//! - Dirty inode tracking for write-back
//! - Per-device inode tables

#![allow(dead_code)]

use alloc::collections::{BTreeMap, VecDeque};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, AtomicU32, Ordering};

use spin::Mutex;

use super::vfs::Inode;

/// Maximum number of inodes in the cache
const INODE_CACHE_SIZE: usize = 2048;

/// Maximum dirty inodes before forced writeback
const MAX_DIRTY_INODES: usize = 256;

/// Device ID type
pub type DeviceId = u32;

/// Unique identifier for cached inodes
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InodeKey {
    /// Device ID
    pub device: DeviceId,
    /// Inode number
    pub ino: u64,
}

impl InodeKey {
    pub fn new(device: DeviceId, ino: u64) -> Self {
        InodeKey { device, ino }
    }
}

/// Inode state flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InodeFlags(u32);

impl InodeFlags {
    pub const NONE: Self = InodeFlags(0);
    /// Inode has been modified
    pub const DIRTY: Self = InodeFlags(1 << 0);
    /// Inode is being written back
    pub const WRITEBACK: Self = InodeFlags(1 << 1);
    /// Inode is locked
    pub const LOCKED: Self = InodeFlags(1 << 2);
    /// Inode is new (not yet on disk)
    pub const NEW: Self = InodeFlags(1 << 3);
    /// Inode is being freed
    pub const FREEING: Self = InodeFlags(1 << 4);

    pub fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }
}

/// A cached inode entry
pub struct CachedInode {
    /// The key (device, ino)
    pub key: InodeKey,
    /// The actual inode
    pub inode: Inode,
    /// Reference count (number of active users)
    pub ref_count: AtomicU32,
    /// State flags
    pub flags: InodeFlags,
    /// Time when last accessed
    pub last_access: u64,
    /// Time when last modified (for dirty tracking)
    pub last_modified: u64,
    /// Number of open file handles
    pub open_count: u32,
}

impl CachedInode {
    /// Create a new cached inode
    pub fn new(device: DeviceId, ino: u64, inode: Inode) -> Self {
        Self {
            key: InodeKey::new(device, ino),
            inode,
            ref_count: AtomicU32::new(1),
            flags: InodeFlags::NONE,
            last_access: current_tick(),
            last_modified: 0,
            open_count: 0,
        }
    }

    /// Increment reference count
    pub fn get(&self) -> u32 {
        self.ref_count.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Decrement reference count
    pub fn put(&self) -> u32 {
        self.ref_count.fetch_sub(1, Ordering::SeqCst) - 1
    }

    /// Get current reference count
    pub fn refs(&self) -> u32 {
        self.ref_count.load(Ordering::SeqCst)
    }

    /// Check if inode is dirty
    pub fn is_dirty(&self) -> bool {
        self.flags.contains(InodeFlags::DIRTY)
    }

    /// Mark inode as dirty
    pub fn mark_dirty(&mut self) {
        self.flags.insert(InodeFlags::DIRTY);
        self.last_modified = current_tick();
    }

    /// Clear dirty flag
    pub fn clear_dirty(&mut self) {
        self.flags.remove(InodeFlags::DIRTY);
    }

    /// Check if inode can be evicted (ref_count == 0)
    pub fn can_evict(&self) -> bool {
        self.refs() == 0 && self.open_count == 0 && !self.is_dirty()
    }
}

/// Statistics for the inode cache
#[derive(Debug, Clone, Copy, Default)]
pub struct InodeCacheStats {
    /// Total lookups
    pub lookups: u64,
    /// Cache hits
    pub hits: u64,
    /// Cache misses
    pub misses: u64,
    /// Inodes added
    pub inodes_added: u64,
    /// Inodes evicted
    pub inodes_evicted: u64,
    /// Dirty inodes written back
    pub writebacks: u64,
    /// Current inode count
    pub inode_count: u64,
    /// Current dirty inode count
    pub dirty_count: u64,
}

impl InodeCacheStats {
    /// Get hit rate as percentage (0-100)
    pub fn hit_rate(&self) -> f64 {
        if self.lookups == 0 {
            0.0
        } else {
            (self.hits as f64 / self.lookups as f64) * 100.0
        }
    }
}

/// LRU tracking node
struct LruNode {
    key: InodeKey,
}

/// The inode cache
pub struct InodeCache {
    /// Hash table: InodeKey -> index in inodes
    index: BTreeMap<InodeKey, usize>,
    /// Inode storage
    inodes: Vec<Option<CachedInode>>,
    /// Free list indices
    free_list: VecDeque<usize>,
    /// LRU order (back = most recently used)
    lru: VecDeque<LruNode>,
    /// Dirty inode list
    dirty_list: VecDeque<InodeKey>,
    /// Statistics
    stats: InodeCacheStats,
}

impl InodeCache {
    /// Create a new inode cache
    pub fn new() -> Self {
        let mut inodes = Vec::with_capacity(INODE_CACHE_SIZE);
        let mut free_list = VecDeque::with_capacity(INODE_CACHE_SIZE);

        for i in 0..INODE_CACHE_SIZE {
            inodes.push(None);
            free_list.push_back(i);
        }

        Self {
            index: BTreeMap::new(),
            inodes,
            free_list,
            lru: VecDeque::with_capacity(INODE_CACHE_SIZE),
            dirty_list: VecDeque::with_capacity(MAX_DIRTY_INODES),
            stats: InodeCacheStats::default(),
        }
    }

    /// Lookup an inode by device and inode number
    pub fn lookup(&mut self, device: DeviceId, ino: u64) -> Option<Inode> {
        self.stats.lookups += 1;

        let key = InodeKey::new(device, ino);

        let idx = match self.index.get(&key) {
            Some(&i) => i,
            None => {
                self.stats.misses += 1;
                return None;
            }
        };

        // Get the inode reference
        let result = if let Some(ref cached) = self.inodes[idx] {
            cached.get();
            self.stats.hits += 1;
            Some(cached.inode.clone())
        } else {
            self.stats.misses += 1;
            None
        };

        // Update LRU (do this separately to avoid borrow issues)
        if result.is_some() {
            self.move_to_back(&key);

            // Update last_access
            if let Some(ref mut cached) = self.inodes[idx] {
                cached.last_access = current_tick();
            }
        }

        result
    }

    /// Insert an inode into the cache
    pub fn insert(&mut self, device: DeviceId, ino: u64, inode: Inode) {
        let key = InodeKey::new(device, ino);

        // If already exists, just update reference
        if let Some(&idx) = self.index.get(&key) {
            if let Some(ref mut cached) = self.inodes[idx] {
                cached.get();
                cached.last_access = current_tick();
                self.move_to_back(&key);
            }
            return;
        }

        // Evict if cache is full
        while self.free_list.is_empty() {
            if !self.evict_one() {
                // Can't evict anything, force evict LRU
                if let Some(lru_node) = self.lru.pop_front() {
                    self.force_evict(&lru_node.key);
                } else {
                    break;
                }
            }
        }

        if let Some(idx) = self.free_list.pop_front() {
            let cached = CachedInode::new(device, ino, inode);
            self.inodes[idx] = Some(cached);
            self.index.insert(key, idx);
            self.lru.push_back(LruNode { key });

            self.stats.inodes_added += 1;
            self.stats.inode_count += 1;
        }
    }

    /// Release a reference to an inode
    pub fn release(&mut self, device: DeviceId, ino: u64) {
        let key = InodeKey::new(device, ino);

        if let Some(&idx) = self.index.get(&key) {
            if let Some(ref cached) = self.inodes[idx] {
                cached.put();
            }
        }
    }

    /// Mark an inode as dirty
    pub fn mark_dirty(&mut self, device: DeviceId, ino: u64) {
        let key = InodeKey::new(device, ino);

        if let Some(&idx) = self.index.get(&key) {
            if let Some(ref mut cached) = self.inodes[idx] {
                if !cached.is_dirty() {
                    cached.mark_dirty();
                    self.dirty_list.push_back(key);
                    self.stats.dirty_count += 1;
                }
            }
        }

        // Trigger writeback if too many dirty inodes
        if self.dirty_list.len() >= MAX_DIRTY_INODES {
            self.writeback_some(MAX_DIRTY_INODES / 2);
        }
    }

    /// Clear dirty flag for an inode
    pub fn clear_dirty(&mut self, device: DeviceId, ino: u64) {
        let key = InodeKey::new(device, ino);

        if let Some(&idx) = self.index.get(&key) {
            if let Some(ref mut cached) = self.inodes[idx] {
                if cached.is_dirty() {
                    cached.clear_dirty();
                    self.dirty_list.retain(|k| k != &key);
                    self.stats.dirty_count = self.stats.dirty_count.saturating_sub(1);
                }
            }
        }
    }

    /// Increment open count for an inode
    pub fn open(&mut self, device: DeviceId, ino: u64) {
        let key = InodeKey::new(device, ino);

        if let Some(&idx) = self.index.get(&key) {
            if let Some(ref mut cached) = self.inodes[idx] {
                cached.open_count += 1;
            }
        }
    }

    /// Decrement open count for an inode
    pub fn close(&mut self, device: DeviceId, ino: u64) {
        let key = InodeKey::new(device, ino);

        if let Some(&idx) = self.index.get(&key) {
            if let Some(ref mut cached) = self.inodes[idx] {
                cached.open_count = cached.open_count.saturating_sub(1);
            }
        }
    }

    /// Remove an inode from the cache
    pub fn remove(&mut self, device: DeviceId, ino: u64) {
        let key = InodeKey::new(device, ino);
        self.remove_entry(&key);
    }

    /// Invalidate all inodes for a device
    pub fn invalidate_device(&mut self, device: DeviceId) {
        let to_remove: Vec<InodeKey> = self
            .inodes
            .iter()
            .filter_map(|opt| {
                opt.as_ref().and_then(|c| {
                    if c.key.device == device {
                        Some(c.key)
                    } else {
                        None
                    }
                })
            })
            .collect();

        for key in to_remove {
            self.remove_entry(&key);
        }
    }

    /// Invalidate all inodes
    pub fn invalidate_all(&mut self) {
        // First writeback all dirty inodes
        self.sync_all();

        for i in 0..self.inodes.len() {
            if self.inodes[i].is_some() {
                self.inodes[i] = None;
                self.free_list.push_back(i);
            }
        }
        self.index.clear();
        self.lru.clear();
        self.dirty_list.clear();
        self.stats.inode_count = 0;
        self.stats.dirty_count = 0;
    }

    /// Write back some dirty inodes
    pub fn writeback_some(&mut self, count: usize) {
        let mut written = 0;

        while written < count {
            if let Some(key) = self.dirty_list.pop_front() {
                // In a real implementation, we would call the filesystem's
                // writeback function here
                if let Some(&idx) = self.index.get(&key) {
                    if let Some(ref mut cached) = self.inodes[idx] {
                        cached.clear_dirty();
                        self.stats.writebacks += 1;
                        self.stats.dirty_count = self.stats.dirty_count.saturating_sub(1);
                        written += 1;
                    }
                }
            } else {
                break;
            }
        }
    }

    /// Sync all dirty inodes
    pub fn sync_all(&mut self) {
        let count = self.dirty_list.len();
        self.writeback_some(count);
    }

    /// Remove entry by key
    fn remove_entry(&mut self, key: &InodeKey) {
        if let Some(idx) = self.index.remove(key) {
            if let Some(cached) = self.inodes[idx].take() {
                if cached.is_dirty() {
                    self.dirty_list.retain(|k| k != key);
                    self.stats.dirty_count = self.stats.dirty_count.saturating_sub(1);
                }
                self.free_list.push_back(idx);
                self.stats.inode_count = self.stats.inode_count.saturating_sub(1);
            }

            // Remove from LRU
            self.lru.retain(|n| n.key != *key);
        }
    }

    /// Evict one inode (LRU, only if ref_count == 0)
    fn evict_one(&mut self) -> bool {
        // Find an evictable inode from the LRU list
        let mut evicted = false;

        for i in 0..self.lru.len() {
            let key = self.lru[i].key;

            if let Some(&idx) = self.index.get(&key) {
                if let Some(ref cached) = self.inodes[idx] {
                    if cached.can_evict() {
                        self.lru.remove(i);
                        self.remove_entry(&key);
                        self.stats.inodes_evicted += 1;
                        evicted = true;
                        break;
                    }
                }
            }
        }

        evicted
    }

    /// Force evict an inode (even if dirty, writes back first)
    fn force_evict(&mut self, key: &InodeKey) {
        if let Some(&idx) = self.index.get(key) {
            if let Some(ref mut cached) = self.inodes[idx] {
                if cached.is_dirty() {
                    // Would write back here
                    cached.clear_dirty();
                    self.dirty_list.retain(|k| k != key);
                    self.stats.dirty_count = self.stats.dirty_count.saturating_sub(1);
                    self.stats.writebacks += 1;
                }
            }
        }
        self.remove_entry(key);
        self.stats.inodes_evicted += 1;
    }

    /// Move entry to back of LRU (most recently used)
    fn move_to_back(&mut self, key: &InodeKey) {
        if let Some(pos) = self.lru.iter().position(|n| n.key == *key) {
            let node = self.lru.remove(pos).unwrap();
            self.lru.push_back(node);
        }
    }

    /// Get cache statistics
    pub fn stats(&self) -> InodeCacheStats {
        self.stats
    }

    /// Get current inode count
    pub fn len(&self) -> usize {
        self.stats.inode_count as usize
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Shrink cache to target size (for memory pressure)
    pub fn shrink_to(&mut self, target_size: usize) {
        while self.len() > target_size {
            if !self.evict_one() {
                break; // Can't evict more without forcing
            }
        }
    }

    /// Get dirty inode count
    pub fn dirty_count(&self) -> usize {
        self.stats.dirty_count as usize
    }
}

impl Default for InodeCache {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Per-device inode tables (for filesystems)
// ============================================================================

/// Per-device inode table
pub struct DeviceInodeTable {
    /// Device ID
    device: DeviceId,
    /// Inode number allocator
    next_ino: AtomicU64,
    /// Maximum inode number (0 = unlimited)
    max_ino: u64,
}

impl DeviceInodeTable {
    /// Create a new device inode table
    pub fn new(device: DeviceId, max_ino: u64) -> Self {
        Self {
            device,
            next_ino: AtomicU64::new(1), // ino 0 is reserved
            max_ino,
        }
    }

    /// Allocate a new inode number
    pub fn alloc_ino(&self) -> Option<u64> {
        let ino = self.next_ino.fetch_add(1, Ordering::SeqCst);

        if self.max_ino > 0 && ino >= self.max_ino {
            None
        } else {
            Some(ino)
        }
    }

    /// Get the device ID
    pub fn device(&self) -> DeviceId {
        self.device
    }
}

// ============================================================================
// Global inode cache instance
// ============================================================================

use spin::Once;

static INODE_CACHE: Once<Mutex<InodeCache>> = Once::new();

/// Global tick counter for timestamps
static TICK_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Get current tick (for timestamps)
fn current_tick() -> u64 {
    TICK_COUNTER.load(Ordering::Relaxed)
}

/// Update tick counter (call from timer interrupt)
pub fn tick() {
    TICK_COUNTER.fetch_add(1, Ordering::Relaxed);
}

/// Initialize the inode cache subsystem
pub fn init() {
    INODE_CACHE.call_once(|| Mutex::new(InodeCache::new()));
    crate::kprintln!("icache: inode cache initialized (max {} entries)", INODE_CACHE_SIZE);
}

/// Lookup an inode in the global cache
pub fn lookup(device: DeviceId, ino: u64) -> Option<Inode> {
    INODE_CACHE.get().and_then(|c| c.lock().lookup(device, ino))
}

/// Insert an inode into the global cache
pub fn insert(device: DeviceId, ino: u64, inode: Inode) {
    if let Some(cache) = INODE_CACHE.get() {
        cache.lock().insert(device, ino, inode);
    }
}

/// Release a reference to an inode
pub fn release(device: DeviceId, ino: u64) {
    if let Some(cache) = INODE_CACHE.get() {
        cache.lock().release(device, ino);
    }
}

/// Mark an inode as dirty
pub fn mark_dirty(device: DeviceId, ino: u64) {
    if let Some(cache) = INODE_CACHE.get() {
        cache.lock().mark_dirty(device, ino);
    }
}

/// Clear dirty flag for an inode
pub fn clear_dirty(device: DeviceId, ino: u64) {
    if let Some(cache) = INODE_CACHE.get() {
        cache.lock().clear_dirty(device, ino);
    }
}

/// Increment open count for an inode
pub fn open(device: DeviceId, ino: u64) {
    if let Some(cache) = INODE_CACHE.get() {
        cache.lock().open(device, ino);
    }
}

/// Decrement open count for an inode
pub fn close(device: DeviceId, ino: u64) {
    if let Some(cache) = INODE_CACHE.get() {
        cache.lock().close(device, ino);
    }
}

/// Remove an inode from the cache
pub fn remove(device: DeviceId, ino: u64) {
    if let Some(cache) = INODE_CACHE.get() {
        cache.lock().remove(device, ino);
    }
}

/// Invalidate all inodes for a device
pub fn invalidate_device(device: DeviceId) {
    if let Some(cache) = INODE_CACHE.get() {
        cache.lock().invalidate_device(device);
    }
}

/// Invalidate all inodes
pub fn invalidate_all() {
    if let Some(cache) = INODE_CACHE.get() {
        cache.lock().invalidate_all();
    }
}

/// Write back some dirty inodes
pub fn writeback_some(count: usize) {
    if let Some(cache) = INODE_CACHE.get() {
        cache.lock().writeback_some(count);
    }
}

/// Sync all dirty inodes
pub fn sync_all() {
    if let Some(cache) = INODE_CACHE.get() {
        cache.lock().sync_all();
    }
}

/// Get cache statistics
pub fn get_stats() -> InodeCacheStats {
    INODE_CACHE
        .get()
        .map(|c| c.lock().stats())
        .unwrap_or_default()
}

/// Shrink cache (for memory pressure)
pub fn shrink(target_size: usize) {
    if let Some(cache) = INODE_CACHE.get() {
        cache.lock().shrink_to(target_size);
    }
}

// ============================================================================
// procfs interface
// ============================================================================

/// Format inode cache stats for /proc/slabinfo style output
pub fn format_stats() -> String {
    let stats = get_stats();

    alloc::format!(
        "Inode Cache Statistics:\n\
         Lookups:      {}\n\
         Hits:         {} ({:.1}%)\n\
         Misses:       {}\n\
         Cached:       {}\n\
         Dirty:        {}\n\
         Added:        {}\n\
         Evicted:      {}\n\
         Writebacks:   {}\n",
        stats.lookups,
        stats.hits,
        stats.hit_rate(),
        stats.misses,
        stats.inode_count,
        stats.dirty_count,
        stats.inodes_added,
        stats.inodes_evicted,
        stats.writebacks
    )
}
