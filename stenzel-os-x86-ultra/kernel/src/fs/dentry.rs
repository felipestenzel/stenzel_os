//! Directory Entry Cache (Dentry Cache)
//!
//! The dentry cache speeds up pathname resolution by caching
//! directory entries. It stores mappings from (parent_inode, name)
//! to child inode.
//!
//! Features:
//! - Hash table for O(1) lookup
//! - LRU eviction when cache is full
//! - Negative entries (caching "not found" results)
//! - Automatic invalidation on filesystem changes
//! - Per-mount namespace support

#![allow(dead_code)]

use alloc::collections::{BTreeMap, VecDeque};
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::hash::{Hash, Hasher};
use core::sync::atomic::{AtomicU64, Ordering};

use spin::Mutex;

use super::vfs::Inode;

/// Maximum number of entries in the dentry cache
const DENTRY_CACHE_SIZE: usize = 4096;

/// Maximum number of negative entries
const MAX_NEGATIVE_ENTRIES: usize = 512;

/// Entry lifetime in ticks before considered stale (for negative entries)
const NEGATIVE_ENTRY_TTL: u64 = 10000;

/// Simple FNV-1a hasher for dentry keys
fn fnv1a_hash(parent_ino: u64, name: &str) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET_BASIS;

    // Hash parent inode number
    for byte in parent_ino.to_le_bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    // Hash name bytes
    for byte in name.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    hash
}

/// Unique identifier for dentry entries
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DentryId(u64);

impl DentryId {
    pub fn new(parent_ino: u64, name: &str) -> Self {
        DentryId(fnv1a_hash(parent_ino, name))
    }
}

/// A dentry cache entry
#[derive(Clone)]
pub struct Dentry {
    /// Parent directory inode number
    pub parent_ino: u64,
    /// Entry name
    pub name: String,
    /// Cached inode (None for negative entry)
    pub inode: Option<Inode>,
    /// Time when this entry was cached (for TTL)
    pub cached_at: u64,
    /// Number of times this entry was accessed
    pub access_count: u64,
    /// Hash of (parent_ino, name) for quick comparison
    hash: u64,
}

impl Dentry {
    /// Create a new positive dentry
    pub fn new(parent_ino: u64, name: &str, inode: Inode) -> Self {
        let hash = fnv1a_hash(parent_ino, name);
        Self {
            parent_ino,
            name: name.to_string(),
            inode: Some(inode),
            cached_at: current_tick(),
            access_count: 1,
            hash,
        }
    }

    /// Create a negative dentry (entry doesn't exist)
    pub fn negative(parent_ino: u64, name: &str) -> Self {
        let hash = fnv1a_hash(parent_ino, name);
        Self {
            parent_ino,
            name: name.to_string(),
            inode: None,
            cached_at: current_tick(),
            access_count: 1,
            hash,
        }
    }

    /// Check if this is a negative entry
    pub fn is_negative(&self) -> bool {
        self.inode.is_none()
    }

    /// Check if this entry is expired (for negative entries)
    pub fn is_expired(&self) -> bool {
        if self.is_negative() {
            let now = current_tick();
            now.saturating_sub(self.cached_at) > NEGATIVE_ENTRY_TTL
        } else {
            false // Positive entries don't expire
        }
    }

    /// Update access statistics
    pub fn touch(&mut self) {
        self.access_count += 1;
    }

    /// Get the dentry ID
    pub fn id(&self) -> DentryId {
        DentryId(self.hash)
    }
}

/// Statistics for the dentry cache
#[derive(Debug, Clone, Copy, Default)]
pub struct DentryCacheStats {
    /// Total lookups
    pub lookups: u64,
    /// Cache hits
    pub hits: u64,
    /// Cache misses
    pub misses: u64,
    /// Negative hits (cached "not found")
    pub negative_hits: u64,
    /// Entries added
    pub entries_added: u64,
    /// Entries evicted
    pub entries_evicted: u64,
    /// Entries invalidated
    pub entries_invalidated: u64,
    /// Current entry count
    pub entry_count: u64,
    /// Negative entry count
    pub negative_count: u64,
}

impl DentryCacheStats {
    /// Get hit rate as percentage (0-100)
    pub fn hit_rate(&self) -> f64 {
        if self.lookups == 0 {
            0.0
        } else {
            (self.hits as f64 / self.lookups as f64) * 100.0
        }
    }
}

/// LRU node for tracking access order
struct LruNode {
    id: DentryId,
    is_negative: bool,
}

/// The dentry cache
pub struct DentryCache {
    /// Hash table: DentryId -> index in entries
    index: BTreeMap<DentryId, usize>,
    /// Entry storage
    entries: Vec<Option<Dentry>>,
    /// Free list indices
    free_list: VecDeque<usize>,
    /// LRU order (back = most recently used)
    lru: VecDeque<LruNode>,
    /// Statistics
    stats: DentryCacheStats,
    /// Negative entry count
    negative_count: usize,
}

impl DentryCache {
    /// Create a new dentry cache
    pub fn new() -> Self {
        let mut entries = Vec::with_capacity(DENTRY_CACHE_SIZE);
        let mut free_list = VecDeque::with_capacity(DENTRY_CACHE_SIZE);

        for i in 0..DENTRY_CACHE_SIZE {
            entries.push(None);
            free_list.push_back(i);
        }

        Self {
            index: BTreeMap::new(),
            entries,
            free_list,
            lru: VecDeque::with_capacity(DENTRY_CACHE_SIZE),
            stats: DentryCacheStats::default(),
            negative_count: 0,
        }
    }

    /// Lookup a dentry by parent inode and name
    pub fn lookup(&mut self, parent_ino: u64, name: &str) -> Option<Dentry> {
        self.stats.lookups += 1;

        let id = DentryId::new(parent_ino, name);

        let idx = match self.index.get(&id) {
            Some(&i) => i,
            None => {
                self.stats.misses += 1;
                return None;
            }
        };

        // Check if entry exists and get info we need
        let (is_expired, is_negative) = match &self.entries[idx] {
            Some(entry) => (entry.is_expired(), entry.is_negative()),
            None => {
                self.stats.misses += 1;
                return None;
            }
        };

        // Handle expired entry
        if is_expired {
            self.remove_entry(id);
            self.stats.misses += 1;
            return None;
        }

        // Update LRU (before touching entry)
        self.move_to_back(&id);

        // Now we can safely get a mutable reference and update
        if let Some(ref mut entry) = self.entries[idx] {
            entry.touch();

            if is_negative {
                self.stats.negative_hits += 1;
            } else {
                self.stats.hits += 1;
            }

            return Some(entry.clone());
        }

        self.stats.misses += 1;
        None
    }

    /// Insert a positive dentry
    pub fn insert(&mut self, parent_ino: u64, name: &str, inode: Inode) {
        let id = DentryId::new(parent_ino, name);

        // Remove existing entry if present
        if self.index.contains_key(&id) {
            self.remove_entry(id);
        }

        // Evict if cache is full
        while self.free_list.is_empty() {
            self.evict_one();
        }

        let idx = self.free_list.pop_front().unwrap();
        let dentry = Dentry::new(parent_ino, name, inode);

        self.entries[idx] = Some(dentry);
        self.index.insert(id, idx);
        self.lru.push_back(LruNode {
            id,
            is_negative: false,
        });

        self.stats.entries_added += 1;
        self.stats.entry_count += 1;
    }

    /// Insert a negative dentry (caches "not found")
    pub fn insert_negative(&mut self, parent_ino: u64, name: &str) {
        // Limit negative entries
        if self.negative_count >= MAX_NEGATIVE_ENTRIES {
            self.evict_negative();
        }

        let id = DentryId::new(parent_ino, name);

        // Remove existing entry if present
        if self.index.contains_key(&id) {
            self.remove_entry(id);
        }

        // Evict if cache is full
        while self.free_list.is_empty() {
            self.evict_one();
        }

        let idx = self.free_list.pop_front().unwrap();
        let dentry = Dentry::negative(parent_ino, name);

        self.entries[idx] = Some(dentry);
        self.index.insert(id, idx);
        self.lru.push_back(LruNode {
            id,
            is_negative: true,
        });

        self.negative_count += 1;
        self.stats.entries_added += 1;
        self.stats.entry_count += 1;
        self.stats.negative_count = self.negative_count as u64;
    }

    /// Remove a dentry by parent and name
    pub fn remove(&mut self, parent_ino: u64, name: &str) {
        let id = DentryId::new(parent_ino, name);
        self.remove_entry(id);
    }

    /// Invalidate all entries for a parent directory
    /// Call this when a directory is modified (file created/deleted/renamed)
    pub fn invalidate_dir(&mut self, parent_ino: u64) {
        // Collect IDs to remove
        let to_remove: Vec<DentryId> = self
            .entries
            .iter()
            .filter_map(|opt| {
                opt.as_ref().and_then(|e| {
                    if e.parent_ino == parent_ino {
                        Some(e.id())
                    } else {
                        None
                    }
                })
            })
            .collect();

        for id in to_remove {
            self.remove_entry(id);
            self.stats.entries_invalidated += 1;
        }
    }

    /// Invalidate all entries (e.g., after umount)
    pub fn invalidate_all(&mut self) {
        for i in 0..self.entries.len() {
            if self.entries[i].is_some() {
                self.entries[i] = None;
                self.free_list.push_back(i);
            }
        }
        self.index.clear();
        self.lru.clear();
        self.negative_count = 0;
        self.stats.entry_count = 0;
        self.stats.negative_count = 0;
    }

    /// Remove entry by ID
    fn remove_entry(&mut self, id: DentryId) {
        if let Some(idx) = self.index.remove(&id) {
            if let Some(entry) = self.entries[idx].take() {
                if entry.is_negative() {
                    self.negative_count = self.negative_count.saturating_sub(1);
                }
                self.free_list.push_back(idx);
                self.stats.entry_count = self.stats.entry_count.saturating_sub(1);
            }

            // Remove from LRU
            self.lru.retain(|n| n.id != id);
        }
    }

    /// Evict one entry (LRU)
    fn evict_one(&mut self) {
        if let Some(node) = self.lru.pop_front() {
            self.remove_entry(node.id);
            self.stats.entries_evicted += 1;
        }
    }

    /// Evict oldest negative entry
    fn evict_negative(&mut self) {
        // Find first negative entry in LRU
        if let Some(pos) = self.lru.iter().position(|n| n.is_negative) {
            let node = self.lru.remove(pos).unwrap();
            self.remove_entry(node.id);
            self.stats.entries_evicted += 1;
        }
    }

    /// Move entry to back of LRU (most recently used)
    fn move_to_back(&mut self, id: &DentryId) {
        if let Some(pos) = self.lru.iter().position(|n| n.id == *id) {
            let node = self.lru.remove(pos).unwrap();
            self.lru.push_back(node);
        }
    }

    /// Get cache statistics
    pub fn stats(&self) -> DentryCacheStats {
        self.stats
    }

    /// Get current entry count
    pub fn len(&self) -> usize {
        self.stats.entry_count as usize
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Shrink cache to target size (for memory pressure)
    pub fn shrink_to(&mut self, target_size: usize) {
        while self.len() > target_size {
            self.evict_one();
        }
    }

    /// Prune expired negative entries
    pub fn prune_expired(&mut self) {
        let expired: Vec<DentryId> = self
            .entries
            .iter()
            .filter_map(|opt| {
                opt.as_ref().and_then(|e| {
                    if e.is_expired() {
                        Some(e.id())
                    } else {
                        None
                    }
                })
            })
            .collect();

        for id in expired {
            self.remove_entry(id);
        }
    }
}

impl Default for DentryCache {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Path Cache - Caches full path resolutions
// ============================================================================

/// Cache entry for full path resolution
#[derive(Clone)]
pub struct PathCacheEntry {
    /// The full path
    pub path: String,
    /// Resolved inode
    pub inode: Inode,
    /// Time cached
    pub cached_at: u64,
    /// Access count
    pub access_count: u64,
}

/// Maximum entries in path cache
const PATH_CACHE_SIZE: usize = 256;

/// Path cache for full path lookups
pub struct PathCache {
    /// Path -> entry mapping
    entries: BTreeMap<String, PathCacheEntry>,
    /// LRU order (back = most recently used)
    lru: VecDeque<String>,
    /// Statistics
    lookups: u64,
    hits: u64,
}

impl PathCache {
    /// Create a new path cache
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            lru: VecDeque::with_capacity(PATH_CACHE_SIZE),
            lookups: 0,
            hits: 0,
        }
    }

    /// Lookup a path
    pub fn lookup(&mut self, path: &str) -> Option<Inode> {
        self.lookups += 1;

        if let Some(entry) = self.entries.get_mut(path) {
            entry.access_count += 1;
            self.hits += 1;

            // Move to back of LRU
            if let Some(pos) = self.lru.iter().position(|p| p == path) {
                let p = self.lru.remove(pos).unwrap();
                self.lru.push_back(p);
            }

            return Some(entry.inode.clone());
        }

        None
    }

    /// Insert a path
    pub fn insert(&mut self, path: &str, inode: Inode) {
        // Evict if full
        while self.entries.len() >= PATH_CACHE_SIZE {
            if let Some(oldest) = self.lru.pop_front() {
                self.entries.remove(&oldest);
            }
        }

        let entry = PathCacheEntry {
            path: path.to_string(),
            inode,
            cached_at: current_tick(),
            access_count: 1,
        };

        // Remove old entry if exists
        if self.entries.contains_key(path) {
            self.lru.retain(|p| p != path);
        }

        self.entries.insert(path.to_string(), entry);
        self.lru.push_back(path.to_string());
    }

    /// Invalidate entries under a path prefix
    pub fn invalidate_prefix(&mut self, prefix: &str) {
        let to_remove: Vec<String> = self
            .entries
            .keys()
            .filter(|p| p.starts_with(prefix))
            .cloned()
            .collect();

        for path in to_remove {
            self.entries.remove(&path);
            self.lru.retain(|p| p != &path);
        }
    }

    /// Invalidate a specific path
    pub fn invalidate(&mut self, path: &str) {
        self.entries.remove(path);
        self.lru.retain(|p| p != path);
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        self.entries.clear();
        self.lru.clear();
    }

    /// Get hit rate
    pub fn hit_rate(&self) -> f64 {
        if self.lookups == 0 {
            0.0
        } else {
            (self.hits as f64 / self.lookups as f64) * 100.0
        }
    }

    /// Get current entry count
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for PathCache {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Global dentry cache instance
// ============================================================================

use spin::Once;

static DENTRY_CACHE: Once<Mutex<DentryCache>> = Once::new();
static PATH_CACHE: Once<Mutex<PathCache>> = Once::new();

/// Global tick counter for TTL
static TICK_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Get current tick (for TTL calculations)
fn current_tick() -> u64 {
    TICK_COUNTER.load(Ordering::Relaxed)
}

/// Update tick counter (call from timer interrupt)
pub fn tick() {
    TICK_COUNTER.fetch_add(1, Ordering::Relaxed);
}

/// Initialize the dentry cache subsystem
pub fn init() {
    DENTRY_CACHE.call_once(|| Mutex::new(DentryCache::new()));
    PATH_CACHE.call_once(|| Mutex::new(PathCache::new()));
    crate::kprintln!("dentry: cache initialized (max {} entries)", DENTRY_CACHE_SIZE);
}

/// Lookup a dentry in the global cache
pub fn lookup(parent_ino: u64, name: &str) -> Option<Dentry> {
    if let Some(cache) = DENTRY_CACHE.get() {
        cache.lock().lookup(parent_ino, name)
    } else {
        None
    }
}

/// Insert a positive entry into the global cache
pub fn insert(parent_ino: u64, name: &str, inode: Inode) {
    if let Some(cache) = DENTRY_CACHE.get() {
        cache.lock().insert(parent_ino, name, inode);
    }
}

/// Insert a negative entry (not found) into the global cache
pub fn insert_negative(parent_ino: u64, name: &str) {
    if let Some(cache) = DENTRY_CACHE.get() {
        cache.lock().insert_negative(parent_ino, name);
    }
}

/// Remove an entry from the global cache
pub fn remove(parent_ino: u64, name: &str) {
    if let Some(cache) = DENTRY_CACHE.get() {
        cache.lock().remove(parent_ino, name);
    }
}

/// Invalidate all entries for a directory
pub fn invalidate_dir(parent_ino: u64) {
    if let Some(cache) = DENTRY_CACHE.get() {
        cache.lock().invalidate_dir(parent_ino);
    }
}

/// Invalidate all entries
pub fn invalidate_all() {
    if let Some(cache) = DENTRY_CACHE.get() {
        cache.lock().invalidate_all();
    }
    if let Some(cache) = PATH_CACHE.get() {
        cache.lock().clear();
    }
}

/// Get cache statistics
pub fn get_stats() -> DentryCacheStats {
    DENTRY_CACHE
        .get()
        .map(|c| c.lock().stats())
        .unwrap_or_default()
}

/// Shrink cache (for memory pressure)
pub fn shrink(target_size: usize) {
    if let Some(cache) = DENTRY_CACHE.get() {
        cache.lock().shrink_to(target_size);
    }
}

/// Prune expired entries
pub fn prune() {
    if let Some(cache) = DENTRY_CACHE.get() {
        cache.lock().prune_expired();
    }
}

/// Lookup a full path in the path cache
pub fn lookup_path(path: &str) -> Option<Inode> {
    if let Some(cache) = PATH_CACHE.get() {
        cache.lock().lookup(path)
    } else {
        None
    }
}

/// Insert a full path into the path cache
pub fn insert_path(path: &str, inode: Inode) {
    if let Some(cache) = PATH_CACHE.get() {
        cache.lock().insert(path, inode);
    }
}

/// Invalidate paths under a prefix
pub fn invalidate_path_prefix(prefix: &str) {
    if let Some(cache) = PATH_CACHE.get() {
        cache.lock().invalidate_prefix(prefix);
    }
}

/// Invalidate a specific path
pub fn invalidate_path(path: &str) {
    if let Some(cache) = PATH_CACHE.get() {
        cache.lock().invalidate(path);
    }
}

/// Get path cache hit rate
pub fn path_cache_hit_rate() -> f64 {
    PATH_CACHE.get().map(|c| c.lock().hit_rate()).unwrap_or(0.0)
}

// ============================================================================
// procfs interface
// ============================================================================

/// Format dentry cache stats for /proc/slabinfo style output
pub fn format_stats() -> String {
    let stats = get_stats();

    alloc::format!(
        "Dentry Cache Statistics:\n\
         Lookups:      {}\n\
         Hits:         {} ({:.1}%)\n\
         Misses:       {}\n\
         Negative hits: {}\n\
         Entries:      {}\n\
         Negative:     {}\n\
         Added:        {}\n\
         Evicted:      {}\n\
         Invalidated:  {}\n\
         Path cache:   {:.1}% hit rate\n",
        stats.lookups,
        stats.hits,
        stats.hit_rate(),
        stats.misses,
        stats.negative_hits,
        stats.entry_count,
        stats.negative_count,
        stats.entries_added,
        stats.entries_evicted,
        stats.entries_invalidated,
        path_cache_hit_rate()
    )
}
