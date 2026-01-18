//! SSD TRIM Support
//!
//! Implements TRIM/DISCARD commands for SSDs:
//! - ATA TRIM (DATA SET MANAGEMENT) for SATA SSDs
//! - NVMe DEALLOCATE for NVMe SSDs
//! - Batch TRIM operations
//! - Periodic TRIM scheduler (fstrim-like)
//! - Filesystem discard support

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use crate::sync::IrqSafeMutex;

/// Disk identifier
pub type DiskId = u32;

/// LBA (Logical Block Address)
pub type Lba = u64;

/// TRIM range (start LBA, count)
#[derive(Debug, Clone, Copy)]
pub struct TrimRange {
    /// Starting LBA
    pub start: Lba,
    /// Number of sectors
    pub count: u64,
}

impl TrimRange {
    pub fn new(start: Lba, count: u64) -> Self {
        Self { start, count }
    }

    /// Get end LBA (exclusive)
    pub fn end(&self) -> Lba {
        self.start + self.count
    }

    /// Check if ranges overlap
    pub fn overlaps(&self, other: &TrimRange) -> bool {
        self.start < other.end() && other.start < self.end()
    }

    /// Merge with another range if adjacent or overlapping
    pub fn merge(&self, other: &TrimRange) -> Option<TrimRange> {
        if self.end() >= other.start && other.end() >= self.start {
            let start = self.start.min(other.start);
            let end = self.end().max(other.end());
            Some(TrimRange::new(start, end - start))
        } else {
            None
        }
    }

    /// Size in bytes (assuming 512-byte sectors)
    pub fn size_bytes(&self) -> u64 {
        self.count * 512
    }
}

/// TRIM result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrimResult {
    /// Success
    Success,
    /// Partial success (some ranges failed)
    Partial,
    /// Not supported
    NotSupported,
    /// Device error
    DeviceError,
    /// Invalid range
    InvalidRange,
    /// Device busy
    Busy,
}

/// Disk TRIM capabilities
#[derive(Debug, Clone)]
pub struct TrimCapabilities {
    /// TRIM is supported
    pub supported: bool,
    /// Maximum TRIM range size (sectors)
    pub max_range_sectors: u64,
    /// Maximum number of ranges per command
    pub max_ranges_per_cmd: u32,
    /// Deterministic read after TRIM
    pub deterministic_read: bool,
    /// Read returns zeros after TRIM
    pub read_zeros_after_trim: bool,
    /// Supports queued TRIM
    pub queued_trim: bool,
    /// Minimum alignment (sectors)
    pub alignment: u32,
    /// Granularity (sectors)
    pub granularity: u32,
}

impl Default for TrimCapabilities {
    fn default() -> Self {
        Self {
            supported: false,
            max_range_sectors: 0,
            max_ranges_per_cmd: 0,
            deterministic_read: false,
            read_zeros_after_trim: false,
            queued_trim: false,
            alignment: 1,
            granularity: 1,
        }
    }
}

/// Disk type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskType {
    /// SATA/AHCI (ATA TRIM command)
    Sata,
    /// NVMe (DEALLOCATE command)
    Nvme,
    /// virtio-blk (DISCARD request)
    VirtioBlk,
    /// Unknown
    Unknown,
}

/// Disk TRIM info
#[derive(Debug, Clone)]
pub struct TrimDiskInfo {
    /// Disk ID
    pub id: DiskId,
    /// Disk name
    pub name: String,
    /// Disk type
    pub disk_type: DiskType,
    /// TRIM capabilities
    pub capabilities: TrimCapabilities,
    /// Total sectors
    pub total_sectors: u64,
}

/// TRIM statistics
#[derive(Debug, Clone, Default)]
pub struct TrimStats {
    /// Total TRIM commands issued
    pub total_commands: u64,
    /// Total sectors trimmed
    pub total_sectors_trimmed: u64,
    /// Total bytes trimmed
    pub total_bytes_trimmed: u64,
    /// Failed TRIM commands
    pub failed_commands: u64,
    /// Last TRIM timestamp
    pub last_trim_time: u64,
    /// Last scheduled TRIM timestamp
    pub last_scheduled_trim: u64,
}

/// TRIM batch for accumulating ranges
pub struct TrimBatch {
    /// Disk ID
    disk_id: DiskId,
    /// Accumulated ranges
    ranges: Vec<TrimRange>,
    /// Maximum batch size
    max_ranges: usize,
    /// Maximum total sectors
    max_sectors: u64,
    /// Current total sectors
    total_sectors: u64,
}

impl TrimBatch {
    pub fn new(disk_id: DiskId, max_ranges: usize, max_sectors: u64) -> Self {
        Self {
            disk_id,
            ranges: Vec::new(),
            max_ranges,
            max_sectors,
            total_sectors: 0,
        }
    }

    /// Add a range to the batch
    pub fn add(&mut self, range: TrimRange) -> bool {
        if self.ranges.len() >= self.max_ranges {
            return false;
        }
        if self.total_sectors + range.count > self.max_sectors {
            return false;
        }

        // Try to merge with existing range
        for existing in &mut self.ranges {
            if let Some(merged) = existing.merge(&range) {
                self.total_sectors -= existing.count;
                self.total_sectors += merged.count;
                *existing = merged;
                return true;
            }
        }

        self.total_sectors += range.count;
        self.ranges.push(range);
        true
    }

    /// Check if batch is full
    pub fn is_full(&self) -> bool {
        self.ranges.len() >= self.max_ranges || self.total_sectors >= self.max_sectors
    }

    /// Get ranges
    pub fn ranges(&self) -> &[TrimRange] {
        &self.ranges
    }

    /// Get total sectors
    pub fn total_sectors(&self) -> u64 {
        self.total_sectors
    }

    /// Clear batch
    pub fn clear(&mut self) {
        self.ranges.clear();
        self.total_sectors = 0;
    }
}

/// ATA TRIM command handler
pub struct AtaTrimHandler;

impl AtaTrimHandler {
    /// ATA DATA SET MANAGEMENT command (TRIM)
    const ATA_CMD_DATA_SET_MANAGEMENT: u8 = 0x06;

    /// Build ATA TRIM payload (DSM range entries)
    pub fn build_payload(ranges: &[TrimRange]) -> Vec<u8> {
        // ATA DSM uses 8-byte entries:
        // Bytes 0-5: LBA (6 bytes)
        // Bytes 6-7: Range length (2 bytes)
        // Maximum 64 entries per 512-byte sector

        let mut payload = Vec::new();

        for range in ranges {
            // Check if range fits in ATA DSM format
            if range.count > 0xFFFF {
                // Split large ranges
                let mut remaining = range.count;
                let mut lba = range.start;
                while remaining > 0 {
                    let count = remaining.min(0xFFFF);

                    // LBA (6 bytes, little-endian)
                    payload.push((lba & 0xFF) as u8);
                    payload.push(((lba >> 8) & 0xFF) as u8);
                    payload.push(((lba >> 16) & 0xFF) as u8);
                    payload.push(((lba >> 24) & 0xFF) as u8);
                    payload.push(((lba >> 32) & 0xFF) as u8);
                    payload.push(((lba >> 40) & 0xFF) as u8);

                    // Count (2 bytes, little-endian)
                    payload.push((count & 0xFF) as u8);
                    payload.push(((count >> 8) & 0xFF) as u8);

                    remaining -= count;
                    lba += count;
                }
            } else {
                // LBA (6 bytes)
                payload.push((range.start & 0xFF) as u8);
                payload.push(((range.start >> 8) & 0xFF) as u8);
                payload.push(((range.start >> 16) & 0xFF) as u8);
                payload.push(((range.start >> 24) & 0xFF) as u8);
                payload.push(((range.start >> 32) & 0xFF) as u8);
                payload.push(((range.start >> 40) & 0xFF) as u8);

                // Count (2 bytes)
                payload.push((range.count & 0xFF) as u8);
                payload.push(((range.count >> 8) & 0xFF) as u8);
            }
        }

        // Pad to 512-byte alignment
        while payload.len() % 512 != 0 {
            payload.push(0);
        }

        payload
    }
}

/// NVMe DEALLOCATE command handler
pub struct NvmeTrimHandler;

impl NvmeTrimHandler {
    /// Build NVMe Dataset Management (DSM) payload
    pub fn build_payload(ranges: &[TrimRange]) -> Vec<u8> {
        // NVMe DSM uses 16-byte range entries:
        // Bytes 0-3: Context attributes (4 bytes)
        // Bytes 4-7: Length in logical blocks (4 bytes)
        // Bytes 8-15: Starting LBA (8 bytes)

        let mut payload = Vec::new();

        // Number of ranges (0-indexed) in CDW10
        let num_ranges = ranges.len() as u32;

        for range in ranges {
            // Context attributes (0 = no special attributes)
            payload.extend_from_slice(&0u32.to_le_bytes());

            // Length in logical blocks
            let length = range.count.min(u32::MAX as u64) as u32;
            payload.extend_from_slice(&length.to_le_bytes());

            // Starting LBA
            payload.extend_from_slice(&range.start.to_le_bytes());
        }

        // Pad to DWORD alignment
        while payload.len() % 4 != 0 {
            payload.push(0);
        }

        payload
    }
}

/// TRIM callback type
pub type TrimCallback = fn(DiskId, &[TrimRange]) -> TrimResult;

/// TRIM Manager
pub struct TrimManager {
    /// Registered disks
    disks: BTreeMap<DiskId, TrimDiskInfo>,
    /// Per-disk statistics
    stats: BTreeMap<DiskId, TrimStats>,
    /// Pending TRIM batches
    batches: BTreeMap<DiskId, TrimBatch>,
    /// TRIM callback (actual disk I/O)
    trim_callback: Option<TrimCallback>,
    /// Scheduled TRIM interval (seconds)
    scheduled_interval: u64,
    /// Auto-TRIM enabled
    auto_trim: bool,
    /// Minimum free sectors before auto-trim
    auto_trim_threshold: u64,
    /// Global stats
    global_stats: TrimStats,
}

impl TrimManager {
    pub const fn new() -> Self {
        Self {
            disks: BTreeMap::new(),
            stats: BTreeMap::new(),
            batches: BTreeMap::new(),
            trim_callback: None,
            scheduled_interval: 3600, // 1 hour default
            auto_trim: true,
            auto_trim_threshold: 0,
            global_stats: TrimStats {
                total_commands: 0,
                total_sectors_trimmed: 0,
                total_bytes_trimmed: 0,
                failed_commands: 0,
                last_trim_time: 0,
                last_scheduled_trim: 0,
            },
        }
    }

    /// Register a disk
    pub fn register_disk(&mut self, info: TrimDiskInfo) {
        let id = info.id;
        let max_ranges = info.capabilities.max_ranges_per_cmd.max(64) as usize;
        let max_sectors = info.capabilities.max_range_sectors.max(65535);

        self.disks.insert(id, info);
        self.stats.insert(id, TrimStats::default());
        self.batches.insert(id, TrimBatch::new(id, max_ranges, max_sectors));
    }

    /// Unregister a disk
    pub fn unregister_disk(&mut self, disk_id: DiskId) {
        self.disks.remove(&disk_id);
        self.stats.remove(&disk_id);
        self.batches.remove(&disk_id);
    }

    /// Get disk info
    pub fn get_disk(&self, disk_id: DiskId) -> Option<&TrimDiskInfo> {
        self.disks.get(&disk_id)
    }

    /// Check if disk supports TRIM
    pub fn supports_trim(&self, disk_id: DiskId) -> bool {
        self.disks
            .get(&disk_id)
            .map(|d| d.capabilities.supported)
            .unwrap_or(false)
    }

    /// Set TRIM callback
    pub fn set_callback(&mut self, callback: TrimCallback) {
        self.trim_callback = Some(callback);
    }

    /// Set scheduled TRIM interval
    pub fn set_scheduled_interval(&mut self, seconds: u64) {
        self.scheduled_interval = seconds;
    }

    /// Enable/disable auto-TRIM
    pub fn set_auto_trim(&mut self, enabled: bool) {
        self.auto_trim = enabled;
    }

    /// Queue a TRIM operation (batched)
    pub fn queue_trim(&mut self, disk_id: DiskId, range: TrimRange) -> TrimResult {
        // Check if disk supports TRIM
        if !self.supports_trim(disk_id) {
            return TrimResult::NotSupported;
        }

        // Validate range
        if let Some(disk) = self.disks.get(&disk_id) {
            if range.start + range.count > disk.total_sectors {
                return TrimResult::InvalidRange;
            }
        } else {
            return TrimResult::DeviceError;
        }

        // Check if batch is full (without holding mutable borrow)
        let needs_flush = match self.batches.get(&disk_id) {
            Some(b) => b.is_full(),
            None => return TrimResult::DeviceError,
        };

        // Flush if needed before adding
        if needs_flush {
            let _ = self.flush_batch(disk_id);
        }

        // Now add to batch
        let batch = match self.batches.get_mut(&disk_id) {
            Some(b) => b,
            None => return TrimResult::DeviceError,
        };

        if !batch.add(range) {
            return TrimResult::DeviceError;
        }

        TrimResult::Success
    }

    /// Immediate TRIM (no batching)
    pub fn trim_immediate(&mut self, disk_id: DiskId, ranges: &[TrimRange]) -> TrimResult {
        // Check if disk supports TRIM
        if !self.supports_trim(disk_id) {
            return TrimResult::NotSupported;
        }

        // Validate ranges
        if let Some(disk) = self.disks.get(&disk_id) {
            for range in ranges {
                if range.start + range.count > disk.total_sectors {
                    return TrimResult::InvalidRange;
                }
            }
        } else {
            return TrimResult::DeviceError;
        }

        // Execute TRIM
        self.execute_trim(disk_id, ranges)
    }

    /// Flush pending TRIM batch for a disk
    pub fn flush_batch(&mut self, disk_id: DiskId) -> TrimResult {
        let batch = match self.batches.get_mut(&disk_id) {
            Some(b) => b,
            None => return TrimResult::DeviceError,
        };

        if batch.ranges().is_empty() {
            return TrimResult::Success;
        }

        let ranges: Vec<TrimRange> = batch.ranges().to_vec();
        batch.clear();

        self.execute_trim(disk_id, &ranges)
    }

    /// Flush all pending batches
    pub fn flush_all(&mut self) -> TrimResult {
        let disk_ids: Vec<DiskId> = self.disks.keys().copied().collect();
        let mut result = TrimResult::Success;

        for disk_id in disk_ids {
            match self.flush_batch(disk_id) {
                TrimResult::Success => {}
                TrimResult::Partial => result = TrimResult::Partial,
                other => {
                    if result == TrimResult::Success {
                        result = other;
                    }
                }
            }
        }

        result
    }

    /// Execute TRIM via callback
    fn execute_trim(&mut self, disk_id: DiskId, ranges: &[TrimRange]) -> TrimResult {
        let callback = match self.trim_callback {
            Some(f) => f,
            None => return TrimResult::NotSupported,
        };

        let result = callback(disk_id, ranges);

        // Update statistics
        let now = crate::time::realtime().tv_sec as u64;

        if let Some(stats) = self.stats.get_mut(&disk_id) {
            stats.total_commands += 1;

            match result {
                TrimResult::Success => {
                    let sectors: u64 = ranges.iter().map(|r| r.count).sum();
                    stats.total_sectors_trimmed += sectors;
                    stats.total_bytes_trimmed += sectors * 512;
                    stats.last_trim_time = now;
                }
                TrimResult::Partial => {
                    // Count partial as half success for stats
                    let sectors: u64 = ranges.iter().map(|r| r.count).sum();
                    stats.total_sectors_trimmed += sectors / 2;
                    stats.total_bytes_trimmed += (sectors / 2) * 512;
                }
                _ => {
                    stats.failed_commands += 1;
                }
            }
        }

        // Update global stats
        self.global_stats.total_commands += 1;
        if result == TrimResult::Success {
            let sectors: u64 = ranges.iter().map(|r| r.count).sum();
            self.global_stats.total_sectors_trimmed += sectors;
            self.global_stats.total_bytes_trimmed += sectors * 512;
            self.global_stats.last_trim_time = now;
        } else if result != TrimResult::Success && result != TrimResult::Partial {
            self.global_stats.failed_commands += 1;
        }

        result
    }

    /// Run scheduled TRIM (fstrim-like)
    pub fn run_scheduled_trim(&mut self) -> u64 {
        let now = crate::time::realtime().tv_sec as u64;
        self.global_stats.last_scheduled_trim = now;

        // Flush all pending batches
        let _ = self.flush_all();

        self.global_stats.total_sectors_trimmed
    }

    /// Get disk statistics
    pub fn get_disk_stats(&self, disk_id: DiskId) -> Option<&TrimStats> {
        self.stats.get(&disk_id)
    }

    /// Get global statistics
    pub fn global_stats(&self) -> &TrimStats {
        &self.global_stats
    }

    /// List all registered disks
    pub fn list_disks(&self) -> Vec<DiskId> {
        self.disks.keys().copied().collect()
    }

    /// TRIM an entire disk (dangerous!)
    pub fn secure_erase_range(
        &mut self,
        disk_id: DiskId,
        start: Lba,
        count: u64,
    ) -> TrimResult {
        self.trim_immediate(disk_id, &[TrimRange::new(start, count)])
    }
}

/// Global TRIM manager
static TRIM_MANAGER: IrqSafeMutex<TrimManager> = IrqSafeMutex::new(TrimManager::new());

/// TRIM enabled flag
static TRIM_ENABLED: AtomicBool = AtomicBool::new(true);

/// Initialize TRIM subsystem
pub fn init() {
    crate::util::kprintln!("trim: initializing SSD TRIM support...");
    TRIM_ENABLED.store(true, Ordering::Release);
}

/// Register a disk for TRIM
pub fn register_disk(info: TrimDiskInfo) {
    TRIM_MANAGER.lock().register_disk(info);
}

/// Unregister a disk
pub fn unregister_disk(disk_id: DiskId) {
    TRIM_MANAGER.lock().unregister_disk(disk_id);
}

/// Check if disk supports TRIM
pub fn supports_trim(disk_id: DiskId) -> bool {
    TRIM_MANAGER.lock().supports_trim(disk_id)
}

/// Queue a TRIM operation
pub fn queue_trim(disk_id: DiskId, start: Lba, count: u64) -> TrimResult {
    if !TRIM_ENABLED.load(Ordering::Acquire) {
        return TrimResult::NotSupported;
    }
    TRIM_MANAGER.lock().queue_trim(disk_id, TrimRange::new(start, count))
}

/// Immediate TRIM
pub fn trim_now(disk_id: DiskId, ranges: &[TrimRange]) -> TrimResult {
    if !TRIM_ENABLED.load(Ordering::Acquire) {
        return TrimResult::NotSupported;
    }
    TRIM_MANAGER.lock().trim_immediate(disk_id, ranges)
}

/// Flush pending TRIM for a disk
pub fn flush(disk_id: DiskId) -> TrimResult {
    TRIM_MANAGER.lock().flush_batch(disk_id)
}

/// Flush all pending TRIMs
pub fn flush_all() -> TrimResult {
    TRIM_MANAGER.lock().flush_all()
}

/// Run scheduled TRIM (fstrim)
pub fn fstrim() -> u64 {
    TRIM_MANAGER.lock().run_scheduled_trim()
}

/// Get disk statistics
pub fn stats(disk_id: DiskId) -> Option<TrimStats> {
    TRIM_MANAGER.lock().get_disk_stats(disk_id).cloned()
}

/// Get global statistics
pub fn global_stats() -> TrimStats {
    TRIM_MANAGER.lock().global_stats().clone()
}

/// Set TRIM callback
pub fn set_callback(callback: TrimCallback) {
    TRIM_MANAGER.lock().set_callback(callback);
}

/// Enable/disable TRIM
pub fn set_enabled(enabled: bool) {
    TRIM_ENABLED.store(enabled, Ordering::Release);
}

/// Check if TRIM is enabled
pub fn is_enabled() -> bool {
    TRIM_ENABLED.load(Ordering::Acquire)
}

/// Discard helper for filesystems
pub fn discard(disk_id: DiskId, byte_offset: u64, byte_length: u64) -> TrimResult {
    // Convert bytes to sectors (512-byte sectors)
    let start_sector = byte_offset / 512;
    let end_sector = (byte_offset + byte_length + 511) / 512;
    let sector_count = end_sector - start_sector;

    queue_trim(disk_id, start_sector, sector_count)
}

/// Helper to detect TRIM capabilities from ATA IDENTIFY
pub fn detect_ata_trim_caps(identify_data: &[u16; 256]) -> TrimCapabilities {
    // Word 169: DATA SET MANAGEMENT support
    let dsm_support = identify_data[169];
    let trim_supported = (dsm_support & 0x0001) != 0;

    // Word 105: Maximum number of 512-byte blocks per DATA SET MANAGEMENT command
    let max_dsm_blocks = identify_data[105] as u64;

    // Word 69: Additional supported features
    let features = identify_data[69];
    let deterministic_read = (features & 0x4000) != 0;
    let read_zeros = (features & 0x0020) != 0;

    TrimCapabilities {
        supported: trim_supported,
        max_range_sectors: max_dsm_blocks * 64, // 64 entries per 512-byte block
        max_ranges_per_cmd: (max_dsm_blocks * 64).min(65535) as u32,
        deterministic_read,
        read_zeros_after_trim: read_zeros,
        queued_trim: false, // Would need more checking
        alignment: 1,
        granularity: 1,
    }
}

/// Helper to detect TRIM capabilities from NVMe IDENTIFY
pub fn detect_nvme_trim_caps(ns_data: &[u8]) -> TrimCapabilities {
    // NVMe Namespace Identify - check DLFEAT and ONCS

    // Assume basic support for now
    TrimCapabilities {
        supported: true,
        max_range_sectors: u64::MAX,
        max_ranges_per_cmd: 256,
        deterministic_read: true,
        read_zeros_after_trim: true,
        queued_trim: true,
        alignment: 1,
        granularity: 1,
    }
}
