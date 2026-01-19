//! Kernel Same-page Merging (KSM)
//!
//! Deduplicates identical memory pages across processes to save RAM.
//! Similar to Linux's KSM feature.

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;

use crate::sync::IrqSafeMutex;

/// Page size (4 KB)
const PAGE_SIZE: usize = 4096;

/// Default scan batch size
const DEFAULT_SCAN_BATCH: usize = 256;

/// Default sleep between scans (ms)
const DEFAULT_SLEEP_MS: u64 = 20;

/// Page state in KSM
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageState {
    /// Not yet scanned
    Unscanned,
    /// Being scanned
    Scanning,
    /// Candidate for merging (has hash)
    Candidate,
    /// Merged (copy-on-write)
    Merged,
    /// Volatile (changes frequently)
    Volatile,
}

/// Information about a tracked page
#[derive(Debug, Clone)]
pub struct TrackedPage {
    /// Virtual address
    pub vaddr: u64,
    /// Physical frame number
    pub pfn: u64,
    /// Process ID owning this page
    pub pid: u32,
    /// Page hash for comparison
    pub hash: u64,
    /// Current state
    pub state: PageState,
    /// Number of times content changed
    pub change_count: u32,
    /// Last scan timestamp
    pub last_scan_ms: u64,
}

impl TrackedPage {
    pub fn new(vaddr: u64, pfn: u64, pid: u32) -> Self {
        Self {
            vaddr,
            pfn,
            pid,
            hash: 0,
            state: PageState::Unscanned,
            change_count: 0,
            last_scan_ms: 0,
        }
    }
}

/// Merged page group
#[derive(Debug, Clone)]
pub struct MergedGroup {
    /// Hash of the page content
    pub hash: u64,
    /// Physical frame containing the canonical copy
    pub canonical_pfn: u64,
    /// Pages sharing this content (vaddr, pid)
    pub members: Vec<(u64, u32)>,
    /// Reference count
    pub ref_count: u32,
}

impl MergedGroup {
    pub fn new(hash: u64, canonical_pfn: u64) -> Self {
        Self {
            hash,
            canonical_pfn,
            members: Vec::new(),
            ref_count: 1,
        }
    }

    pub fn add_member(&mut self, vaddr: u64, pid: u32) {
        self.members.push((vaddr, pid));
        self.ref_count += 1;
    }

    pub fn remove_member(&mut self, vaddr: u64, pid: u32) -> bool {
        if let Some(idx) = self.members.iter().position(|&(v, p)| v == vaddr && p == pid) {
            self.members.remove(idx);
            self.ref_count = self.ref_count.saturating_sub(1);
            true
        } else {
            false
        }
    }
}

/// KSM configuration
#[derive(Debug, Clone)]
pub struct KsmConfig {
    /// Enable KSM
    pub enabled: bool,
    /// Pages to scan per batch
    pub pages_to_scan: usize,
    /// Sleep time between scans (ms)
    pub sleep_ms: u64,
    /// Maximum pages to track
    pub max_pages: usize,
    /// Merge threshold (pages must match this many times)
    pub merge_threshold: u32,
    /// Mark page volatile after this many changes
    pub volatile_threshold: u32,
    /// Run in background
    pub run_background: bool,
}

impl Default for KsmConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            pages_to_scan: DEFAULT_SCAN_BATCH,
            sleep_ms: DEFAULT_SLEEP_MS,
            max_pages: 1_000_000,
            merge_threshold: 2,
            volatile_threshold: 10,
            run_background: true,
        }
    }
}

/// KSM statistics
#[derive(Debug, Default, Clone)]
pub struct KsmStats {
    /// Total pages tracked
    pub pages_tracked: u64,
    /// Pages scanned
    pub pages_scanned: u64,
    /// Pages merged
    pub pages_merged: u64,
    /// Pages unmerged (CoW triggered)
    pub pages_unmerged: u64,
    /// Merged groups
    pub merge_groups: u64,
    /// Memory saved (bytes)
    pub memory_saved: u64,
    /// Full scans completed
    pub full_scans: u64,
    /// Volatile pages
    pub volatile_pages: u64,
}

impl KsmStats {
    pub fn memory_saved_mb(&self) -> f64 {
        self.memory_saved as f64 / (1024.0 * 1024.0)
    }
}

/// Hash function for page content
fn hash_page(data: &[u8; PAGE_SIZE]) -> u64 {
    // Simple FNV-1a hash
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in data.iter() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// KSM manager
pub struct KsmManager {
    /// Configuration
    config: KsmConfig,
    /// Tracked pages by (pid, vaddr)
    tracked: BTreeMap<(u32, u64), TrackedPage>,
    /// Candidate pages by hash
    candidates: BTreeMap<u64, Vec<(u32, u64)>>,
    /// Merged groups by hash
    merged: BTreeMap<u64, MergedGroup>,
    /// Statistics
    stats: KsmStats,
    /// Current scan position
    scan_cursor: usize,
    /// Scan list (copy of keys for iteration)
    scan_list: Vec<(u32, u64)>,
    /// Is currently running
    running: bool,
}

impl KsmManager {
    /// Create new KSM manager
    pub fn new() -> Self {
        Self {
            config: KsmConfig::default(),
            tracked: BTreeMap::new(),
            candidates: BTreeMap::new(),
            merged: BTreeMap::new(),
            stats: KsmStats::default(),
            scan_cursor: 0,
            scan_list: Vec::new(),
            running: false,
        }
    }

    /// Register a page for KSM tracking
    pub fn register_page(&mut self, vaddr: u64, pfn: u64, pid: u32) {
        if !self.config.enabled {
            return;
        }

        if self.tracked.len() >= self.config.max_pages {
            return;
        }

        let key = (pid, vaddr);
        if !self.tracked.contains_key(&key) {
            self.tracked.insert(key, TrackedPage::new(vaddr, pfn, pid));
            self.stats.pages_tracked += 1;
        }
    }

    /// Unregister a page from KSM
    pub fn unregister_page(&mut self, vaddr: u64, pid: u32) {
        let key = (pid, vaddr);

        if let Some(page) = self.tracked.remove(&key) {
            // Remove from candidates
            if let Some(candidates) = self.candidates.get_mut(&page.hash) {
                candidates.retain(|&(p, v)| !(p == pid && v == vaddr));
                if candidates.is_empty() {
                    self.candidates.remove(&page.hash);
                }
            }

            // Remove from merged group
            if page.state == PageState::Merged {
                self.unmerge_page(vaddr, pid, page.hash);
            }

            self.stats.pages_tracked = self.stats.pages_tracked.saturating_sub(1);
        }
    }

    /// Scan a batch of pages
    pub fn scan_batch(&mut self) -> usize {
        if !self.config.enabled || !self.running {
            return 0;
        }

        // Rebuild scan list if needed
        if self.scan_list.is_empty() || self.scan_cursor >= self.scan_list.len() {
            self.scan_list = self.tracked.keys().cloned().collect();
            self.scan_cursor = 0;
            if !self.scan_list.is_empty() {
                self.stats.full_scans += 1;
            }
        }

        let batch_size = self.config.pages_to_scan.min(self.scan_list.len() - self.scan_cursor);
        let mut scanned = 0;

        for i in 0..batch_size {
            let idx = self.scan_cursor + i;
            if idx >= self.scan_list.len() {
                break;
            }

            let key = self.scan_list[idx];
            self.scan_page(key.0, key.1);
            scanned += 1;
        }

        self.scan_cursor += scanned;
        self.stats.pages_scanned += scanned as u64;

        scanned
    }

    /// Scan a single page
    fn scan_page(&mut self, pid: u32, vaddr: u64) {
        let key = (pid, vaddr);

        // Get page data (in real implementation, read from physical memory)
        let page_data = self.read_page_data(vaddr, pid);
        let volatile_threshold = self.config.volatile_threshold;
        let current_time = crate::time::uptime_ms();

        // First pass: gather information and update page state
        let (old_hash, new_hash, was_merged, should_unmerge, should_add_candidate, became_volatile);
        {
            let page = match self.tracked.get_mut(&key) {
                Some(p) => p,
                None => return,
            };

            if page.state == PageState::Volatile {
                return;
            }

            old_hash = page.hash;
            new_hash = hash_page(&page_data);
            was_merged = page.state == PageState::Merged;

            if was_merged && new_hash != old_hash {
                // Content changed, need to unmerge
                should_unmerge = true;
                page.hash = new_hash;
                page.state = PageState::Candidate;
                page.change_count += 1;
                became_volatile = page.change_count >= volatile_threshold;
                if became_volatile {
                    page.state = PageState::Volatile;
                }
                should_add_candidate = false;
            } else if !was_merged {
                should_unmerge = false;
                if old_hash != new_hash {
                    page.hash = new_hash;
                    page.change_count += 1;
                    became_volatile = page.change_count >= volatile_threshold;
                    if became_volatile {
                        page.state = PageState::Volatile;
                    }
                } else {
                    became_volatile = false;
                }
                should_add_candidate = !became_volatile;
                if should_add_candidate {
                    page.state = PageState::Candidate;
                }
            } else {
                should_unmerge = false;
                should_add_candidate = false;
                became_volatile = false;
            }

            page.last_scan_ms = current_time;
        }

        // Second pass: update other structures
        if should_unmerge {
            self.unmerge_page(vaddr, pid, old_hash);
        }

        if became_volatile {
            self.stats.volatile_pages += 1;
            return;
        }

        // Remove from old candidates if hash changed
        if old_hash != 0 && old_hash != new_hash && !was_merged {
            if let Some(candidates) = self.candidates.get_mut(&old_hash) {
                candidates.retain(|&(p, v)| !(p == pid && v == vaddr));
            }
        }

        // Add to candidates
        if should_add_candidate {
            self.candidates.entry(new_hash)
                .or_insert_with(Vec::new)
                .push((pid, vaddr));

            // Try to merge
            self.try_merge(new_hash);
        }
    }

    /// Try to merge pages with the same hash
    fn try_merge(&mut self, hash: u64) {
        let candidates = match self.candidates.get(&hash) {
            Some(c) if c.len() >= self.config.merge_threshold as usize => c.clone(),
            _ => return,
        };

        // Verify content matches (hash collision check)
        let first = &candidates[0];
        let first_data = self.read_page_data(first.1, first.0);

        let mut matching = vec![*first];
        for &(pid, vaddr) in candidates.iter().skip(1) {
            let data = self.read_page_data(vaddr, pid);
            if data == first_data {
                matching.push((pid, vaddr));
            }
        }

        if matching.len() < self.config.merge_threshold as usize {
            return;
        }

        // Create merged group
        let canonical_pfn = self.tracked.get(&(matching[0].0, matching[0].1))
            .map(|p| p.pfn)
            .unwrap_or(0);

        let mut group = MergedGroup::new(hash, canonical_pfn);

        for &(pid, vaddr) in &matching {
            if let Some(page) = self.tracked.get_mut(&(pid, vaddr)) {
                if page.state != PageState::Merged {
                    page.state = PageState::Merged;
                    group.add_member(vaddr, pid);

                    // In real implementation, remap page to canonical and mark CoW
                    self.stats.pages_merged += 1;
                    self.stats.memory_saved += PAGE_SIZE as u64;
                }
            }
        }

        if group.ref_count > 1 {
            self.merged.insert(hash, group);
            self.stats.merge_groups += 1;
        }

        // Remove from candidates
        self.candidates.remove(&hash);
    }

    /// Unmerge a page (CoW triggered)
    fn unmerge_page(&mut self, vaddr: u64, pid: u32, hash: u64) {
        if let Some(group) = self.merged.get_mut(&hash) {
            group.remove_member(vaddr, pid);
            self.stats.pages_unmerged += 1;
            self.stats.memory_saved = self.stats.memory_saved.saturating_sub(PAGE_SIZE as u64);

            if group.ref_count <= 1 {
                self.merged.remove(&hash);
                self.stats.merge_groups = self.stats.merge_groups.saturating_sub(1);
            }
        }
    }

    /// Read page data (placeholder - real implementation reads physical memory)
    fn read_page_data(&self, _vaddr: u64, _pid: u32) -> [u8; PAGE_SIZE] {
        // In real implementation:
        // 1. Look up page table for pid
        // 2. Get physical frame
        // 3. Map and read content
        [0u8; PAGE_SIZE]
    }

    /// Start KSM scanning
    pub fn start(&mut self) {
        if self.config.enabled {
            self.running = true;
        }
    }

    /// Stop KSM scanning
    pub fn stop(&mut self) {
        self.running = false;
    }

    /// Check if running
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Enable KSM
    pub fn enable(&mut self) {
        self.config.enabled = true;
    }

    /// Disable KSM
    pub fn disable(&mut self) {
        self.config.enabled = false;
        self.stop();
    }

    /// Check if enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get configuration
    pub fn config(&self) -> &KsmConfig {
        &self.config
    }

    /// Set configuration
    pub fn set_config(&mut self, config: KsmConfig) {
        self.config = config;
    }

    /// Get statistics
    pub fn stats(&self) -> &KsmStats {
        &self.stats
    }

    /// Clear all tracking
    pub fn clear(&mut self) {
        self.tracked.clear();
        self.candidates.clear();
        self.merged.clear();
        self.scan_list.clear();
        self.scan_cursor = 0;
        self.stats = KsmStats::default();
    }

    /// Format status string
    pub fn format_status(&self) -> String {
        alloc::format!(
            "KSM: {} | Tracked: {} | Merged: {} | Saved: {:.2} MB | Groups: {}",
            if self.running { "running" } else { "stopped" },
            self.stats.pages_tracked,
            self.stats.pages_merged,
            self.stats.memory_saved_mb(),
            self.stats.merge_groups
        )
    }
}

impl Default for KsmManager {
    fn default() -> Self {
        Self::new()
    }
}

// Global KSM manager
static KSM: IrqSafeMutex<Option<KsmManager>> = IrqSafeMutex::new(None);

/// Initialize KSM
pub fn init() {
    let mut ksm = KSM.lock();
    *ksm = Some(KsmManager::new());
    crate::kprintln!("mm: KSM (memory deduplication) initialized");
}

/// Register page for tracking
pub fn register_page(vaddr: u64, pfn: u64, pid: u32) {
    if let Some(ref mut ksm) = *KSM.lock() {
        ksm.register_page(vaddr, pfn, pid);
    }
}

/// Unregister page
pub fn unregister_page(vaddr: u64, pid: u32) {
    if let Some(ref mut ksm) = *KSM.lock() {
        ksm.unregister_page(vaddr, pid);
    }
}

/// Run scan batch
pub fn scan_batch() -> usize {
    KSM.lock().as_mut().map(|ksm| ksm.scan_batch()).unwrap_or(0)
}

/// Start KSM
pub fn start() {
    if let Some(ref mut ksm) = *KSM.lock() {
        ksm.start();
    }
}

/// Stop KSM
pub fn stop() {
    if let Some(ref mut ksm) = *KSM.lock() {
        ksm.stop();
    }
}

/// Enable KSM
pub fn enable() {
    if let Some(ref mut ksm) = *KSM.lock() {
        ksm.enable();
    }
}

/// Disable KSM
pub fn disable() {
    if let Some(ref mut ksm) = *KSM.lock() {
        ksm.disable();
    }
}

/// Get statistics
pub fn stats() -> Option<KsmStats> {
    KSM.lock().as_ref().map(|ksm| ksm.stats().clone())
}

/// Get status string
pub fn status() -> String {
    KSM.lock().as_ref()
        .map(|ksm| ksm.format_status())
        .unwrap_or_else(|| "KSM not initialized".to_string())
}
