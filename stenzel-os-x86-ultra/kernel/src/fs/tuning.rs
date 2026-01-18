//! Filesystem Tuning Subsystem for Stenzel OS.
//!
//! Provides runtime tuning parameters for filesystem performance optimization.
//!
//! Features:
//! - Per-filesystem mount options
//! - Journal mode configuration
//! - Block allocation tuning
//! - Read-ahead configuration
//! - Commit interval tuning
//! - Barrier/sync options
//! - Cache size limits
//! - Automatic tuning profiles

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use spin::{Mutex, Once};

// ============================================================================
// Mount Options
// ============================================================================

/// Journal mode for journaling filesystems
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalMode {
    /// Journal data and metadata (safest, slowest)
    Data,
    /// Journal metadata only, ordered data writes
    Ordered,
    /// Journal metadata only, unordered data writes (fastest)
    Writeback,
    /// No journaling
    None,
}

impl Default for JournalMode {
    fn default() -> Self {
        JournalMode::Ordered
    }
}

/// Filesystem mount options
#[derive(Debug, Clone)]
pub struct MountOptions {
    /// Read-only mount
    pub read_only: bool,
    /// No access time updates
    pub noatime: bool,
    /// Relative access time (only update if older than mtime)
    pub relatime: bool,
    /// Strict access time (always update)
    pub strictatime: bool,
    /// No directory access time
    pub nodiratime: bool,
    /// Synchronous I/O
    pub sync: bool,
    /// Asynchronous I/O (default)
    pub async_io: bool,
    /// Enable POSIX ACLs
    pub acl: bool,
    /// Enable user extended attributes
    pub user_xattr: bool,
    /// No setuid/setgid
    pub nosuid: bool,
    /// No device files
    pub nodev: bool,
    /// No execution
    pub noexec: bool,
    /// Enable barriers
    pub barrier: bool,
    /// Enable data integrity protection
    pub data_integrity: bool,
    /// Enable discard/TRIM
    pub discard: bool,
    /// Journal mode
    pub journal_mode: JournalMode,
    /// Commit interval in seconds
    pub commit_interval: u32,
    /// Maximum percent of filesystem for reservation
    pub reserved_blocks_pct: u8,
    /// Enable quotas
    pub quota: bool,
    /// User quota
    pub usrquota: bool,
    /// Group quota
    pub grpquota: bool,
}

impl Default for MountOptions {
    fn default() -> Self {
        Self {
            read_only: false,
            noatime: false,
            relatime: true,
            strictatime: false,
            nodiratime: false,
            sync: false,
            async_io: true,
            acl: true,
            user_xattr: true,
            nosuid: false,
            nodev: false,
            noexec: false,
            barrier: true,
            data_integrity: true,
            discard: false,
            journal_mode: JournalMode::Ordered,
            commit_interval: 5,
            reserved_blocks_pct: 5,
            quota: false,
            usrquota: false,
            grpquota: false,
        }
    }
}

impl MountOptions {
    /// Parse mount options from string
    pub fn parse(options: &str) -> Self {
        let mut opts = Self::default();

        for opt in options.split(',') {
            let opt = opt.trim();
            match opt {
                "ro" => opts.read_only = true,
                "rw" => opts.read_only = false,
                "noatime" => opts.noatime = true,
                "atime" => opts.noatime = false,
                "relatime" => opts.relatime = true,
                "norelatime" => opts.relatime = false,
                "strictatime" => opts.strictatime = true,
                "nodiratime" => opts.nodiratime = true,
                "diratime" => opts.nodiratime = false,
                "sync" => {
                    opts.sync = true;
                    opts.async_io = false;
                }
                "async" => {
                    opts.async_io = true;
                    opts.sync = false;
                }
                "acl" => opts.acl = true,
                "noacl" => opts.acl = false,
                "user_xattr" => opts.user_xattr = true,
                "nouser_xattr" => opts.user_xattr = false,
                "suid" => opts.nosuid = false,
                "nosuid" => opts.nosuid = true,
                "dev" => opts.nodev = false,
                "nodev" => opts.nodev = true,
                "exec" => opts.noexec = false,
                "noexec" => opts.noexec = true,
                "barrier" | "barrier=1" => opts.barrier = true,
                "nobarrier" | "barrier=0" => opts.barrier = false,
                "discard" => opts.discard = true,
                "nodiscard" => opts.discard = false,
                "data=journal" => opts.journal_mode = JournalMode::Data,
                "data=ordered" => opts.journal_mode = JournalMode::Ordered,
                "data=writeback" => opts.journal_mode = JournalMode::Writeback,
                "quota" => opts.quota = true,
                "noquota" => opts.quota = false,
                "usrquota" => opts.usrquota = true,
                "grpquota" => opts.grpquota = true,
                _ => {
                    // Parse commit=N
                    if let Some(val) = opt.strip_prefix("commit=") {
                        if let Ok(n) = val.parse::<u32>() {
                            opts.commit_interval = n;
                        }
                    }
                    // Parse reserved=N (percent)
                    if let Some(val) = opt.strip_prefix("reserved=") {
                        if let Ok(n) = val.parse::<u8>() {
                            opts.reserved_blocks_pct = n.min(50);
                        }
                    }
                }
            }
        }

        opts
    }

    /// Convert to mount option string
    pub fn to_string(&self) -> String {
        let mut parts = Vec::new();

        if self.read_only {
            parts.push("ro");
        } else {
            parts.push("rw");
        }

        if self.noatime {
            parts.push("noatime");
        } else if self.relatime {
            parts.push("relatime");
        } else if self.strictatime {
            parts.push("strictatime");
        }

        if self.nodiratime {
            parts.push("nodiratime");
        }

        if self.sync {
            parts.push("sync");
        }

        if !self.barrier {
            parts.push("nobarrier");
        }

        if self.discard {
            parts.push("discard");
        }

        match self.journal_mode {
            JournalMode::Data => parts.push("data=journal"),
            JournalMode::Ordered => parts.push("data=ordered"),
            JournalMode::Writeback => parts.push("data=writeback"),
            JournalMode::None => {}
        }

        parts.join(",")
    }
}

// ============================================================================
// Block Allocation Tuning
// ============================================================================

/// Block allocation strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocationStrategy {
    /// First fit - use first available block
    FirstFit,
    /// Best fit - find smallest suitable hole
    BestFit,
    /// Worst fit - find largest hole
    WorstFit,
    /// Next fit - continue from last allocation
    NextFit,
    /// Multiblock allocation (ext4-style)
    Mballoc,
}

impl Default for AllocationStrategy {
    fn default() -> Self {
        AllocationStrategy::Mballoc
    }
}

/// Block allocation tuning parameters
#[derive(Debug, Clone)]
pub struct AllocationTuning {
    /// Allocation strategy
    pub strategy: AllocationStrategy,
    /// Target group for allocation (for locality)
    pub goal_group: u32,
    /// Minimum extent size (in blocks)
    pub min_extent: u32,
    /// Maximum extent size (in blocks)
    pub max_extent: u32,
    /// Preallocation size for regular files (in blocks)
    pub file_prealloc: u32,
    /// Preallocation size for directories (in blocks)
    pub dir_prealloc: u32,
    /// Stream allocation threshold
    pub stream_threshold: u64,
    /// Enable delayed allocation
    pub delayed_alloc: bool,
    /// Enable bigalloc (large clusters)
    pub bigalloc: bool,
    /// Cluster size for bigalloc (in blocks)
    pub cluster_size: u32,
}

impl Default for AllocationTuning {
    fn default() -> Self {
        Self {
            strategy: AllocationStrategy::Mballoc,
            goal_group: 0,
            min_extent: 1,
            max_extent: 32768,
            file_prealloc: 16,
            dir_prealloc: 4,
            stream_threshold: 64 * 1024,
            delayed_alloc: true,
            bigalloc: false,
            cluster_size: 16,
        }
    }
}

// ============================================================================
// Read-ahead Configuration
// ============================================================================

/// Read-ahead configuration
#[derive(Debug, Clone)]
pub struct ReadaheadConfig {
    /// Enable read-ahead
    pub enabled: bool,
    /// Initial read-ahead size (in KB)
    pub initial_size: u32,
    /// Maximum read-ahead size (in KB)
    pub max_size: u32,
    /// Read-ahead for directories
    pub dir_readahead: bool,
    /// Adaptive read-ahead (auto-tune based on access patterns)
    pub adaptive: bool,
    /// Read-ahead for sequential access pattern
    pub sequential_threshold: u32,
    /// Read-ahead multiplier for sequential access
    pub sequential_multiplier: u32,
}

impl Default for ReadaheadConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            initial_size: 128,
            max_size: 256,
            dir_readahead: true,
            adaptive: true,
            sequential_threshold: 3,
            sequential_multiplier: 2,
        }
    }
}

// ============================================================================
// Cache Configuration
// ============================================================================

/// Filesystem cache configuration
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Inode cache size (number of inodes)
    pub inode_cache_size: u32,
    /// Dentry cache size (number of entries)
    pub dentry_cache_size: u32,
    /// Buffer cache size (in MB)
    pub buffer_cache_mb: u32,
    /// Page cache pressure (0-100, higher = more aggressive reclaim)
    pub page_cache_pressure: u32,
    /// Enable negative dentry caching
    pub negative_dentry_cache: bool,
    /// Negative dentry cache timeout (seconds)
    pub negative_dentry_timeout: u32,
    /// Writeback cache enabled
    pub writeback_cache: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            inode_cache_size: 16384,
            dentry_cache_size: 32768,
            buffer_cache_mb: 128,
            page_cache_pressure: 100,
            negative_dentry_cache: true,
            negative_dentry_timeout: 30,
            writeback_cache: true,
        }
    }
}

// ============================================================================
// Tuning Profiles
// ============================================================================

/// Predefined tuning profiles
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuningProfile {
    /// Balanced (default)
    Balanced,
    /// Optimize for throughput (large sequential I/O)
    Throughput,
    /// Optimize for latency (small random I/O)
    Latency,
    /// Optimize for SSD
    Ssd,
    /// Optimize for NVMe
    Nvme,
    /// Optimize for spinning disk
    Hdd,
    /// Maximum data safety
    DataSafety,
    /// Maximum performance (less safety)
    Performance,
    /// Laptop (power saving)
    Laptop,
    /// Server workload
    Server,
    /// Desktop workload
    Desktop,
}

impl Default for TuningProfile {
    fn default() -> Self {
        TuningProfile::Balanced
    }
}

/// Complete filesystem tuning configuration
#[derive(Debug, Clone)]
pub struct FilesystemTuning {
    /// Profile name
    pub profile: TuningProfile,
    /// Mount options
    pub mount_options: MountOptions,
    /// Allocation tuning
    pub allocation: AllocationTuning,
    /// Read-ahead configuration
    pub readahead: ReadaheadConfig,
    /// Cache configuration
    pub cache: CacheConfig,
    /// Dirty ratio (percent of memory that can be dirty)
    pub dirty_ratio: u32,
    /// Background dirty ratio (percent to start writeback)
    pub dirty_background_ratio: u32,
    /// Dirty writeback centisecs
    pub dirty_writeback_centisecs: u32,
    /// Dirty expire centisecs
    pub dirty_expire_centisecs: u32,
    /// Sync on close
    pub sync_on_close: bool,
    /// Lazy inode init (ext4)
    pub lazy_itable_init: bool,
    /// Discard at mount time
    pub discard_at_mount: bool,
}

impl Default for FilesystemTuning {
    fn default() -> Self {
        Self::from_profile(TuningProfile::Balanced)
    }
}

impl FilesystemTuning {
    /// Create tuning from profile
    pub fn from_profile(profile: TuningProfile) -> Self {
        let mut tuning = Self {
            profile,
            mount_options: MountOptions::default(),
            allocation: AllocationTuning::default(),
            readahead: ReadaheadConfig::default(),
            cache: CacheConfig::default(),
            dirty_ratio: 20,
            dirty_background_ratio: 10,
            dirty_writeback_centisecs: 500,
            dirty_expire_centisecs: 3000,
            sync_on_close: false,
            lazy_itable_init: true,
            discard_at_mount: false,
        };

        match profile {
            TuningProfile::Balanced => {}

            TuningProfile::Throughput => {
                tuning.readahead.max_size = 512;
                tuning.allocation.max_extent = 65536;
                tuning.dirty_ratio = 40;
                tuning.dirty_background_ratio = 20;
            }

            TuningProfile::Latency => {
                tuning.readahead.max_size = 64;
                tuning.allocation.min_extent = 1;
                tuning.dirty_ratio = 10;
                tuning.dirty_background_ratio = 5;
                tuning.dirty_writeback_centisecs = 100;
            }

            TuningProfile::Ssd => {
                tuning.mount_options.discard = true;
                tuning.mount_options.noatime = true;
                tuning.allocation.delayed_alloc = true;
                tuning.readahead.max_size = 128;
                tuning.discard_at_mount = true;
            }

            TuningProfile::Nvme => {
                tuning.mount_options.discard = true;
                tuning.mount_options.noatime = true;
                tuning.allocation.delayed_alloc = true;
                tuning.allocation.max_extent = 65536;
                tuning.readahead.max_size = 256;
                tuning.readahead.adaptive = true;
                tuning.dirty_ratio = 30;
                tuning.discard_at_mount = true;
            }

            TuningProfile::Hdd => {
                tuning.readahead.max_size = 512;
                tuning.readahead.sequential_multiplier = 4;
                tuning.allocation.strategy = AllocationStrategy::Mballoc;
                tuning.allocation.file_prealloc = 64;
                tuning.mount_options.barrier = true;
            }

            TuningProfile::DataSafety => {
                tuning.mount_options.sync = true;
                tuning.mount_options.barrier = true;
                tuning.mount_options.data_integrity = true;
                tuning.mount_options.journal_mode = JournalMode::Data;
                tuning.mount_options.commit_interval = 1;
                tuning.dirty_ratio = 5;
                tuning.dirty_background_ratio = 1;
                tuning.sync_on_close = true;
            }

            TuningProfile::Performance => {
                tuning.mount_options.noatime = true;
                tuning.mount_options.journal_mode = JournalMode::Writeback;
                tuning.mount_options.barrier = false;
                tuning.mount_options.commit_interval = 30;
                tuning.dirty_ratio = 50;
                tuning.dirty_background_ratio = 25;
                tuning.allocation.delayed_alloc = true;
            }

            TuningProfile::Laptop => {
                tuning.mount_options.noatime = true;
                tuning.mount_options.commit_interval = 15;
                tuning.dirty_writeback_centisecs = 1500;
                tuning.dirty_expire_centisecs = 6000;
                tuning.dirty_ratio = 30;
            }

            TuningProfile::Server => {
                tuning.readahead.max_size = 512;
                tuning.cache.inode_cache_size = 65536;
                tuning.cache.dentry_cache_size = 131072;
                tuning.cache.buffer_cache_mb = 256;
                tuning.dirty_ratio = 30;
            }

            TuningProfile::Desktop => {
                tuning.mount_options.relatime = true;
                tuning.readahead.max_size = 256;
                tuning.dirty_ratio = 20;
            }
        }

        tuning
    }

    /// Apply tuning to system
    pub fn apply(&self) {
        // Set dirty ratios (these would write to /proc/sys/vm/ equivalent)
        DIRTY_RATIO.store(self.dirty_ratio, Ordering::Relaxed);
        DIRTY_BG_RATIO.store(self.dirty_background_ratio, Ordering::Relaxed);
        DIRTY_WRITEBACK.store(self.dirty_writeback_centisecs, Ordering::Relaxed);
        DIRTY_EXPIRE.store(self.dirty_expire_centisecs, Ordering::Relaxed);

        crate::kprintln!(
            "fs-tuning: applied profile {:?} (dirty_ratio={}%, commit={}s)",
            self.profile,
            self.dirty_ratio,
            self.mount_options.commit_interval
        );
    }
}

// ============================================================================
// Global Tuning State
// ============================================================================

static DIRTY_RATIO: AtomicU32 = AtomicU32::new(20);
static DIRTY_BG_RATIO: AtomicU32 = AtomicU32::new(10);
static DIRTY_WRITEBACK: AtomicU32 = AtomicU32::new(500);
static DIRTY_EXPIRE: AtomicU32 = AtomicU32::new(3000);

/// Per-filesystem tuning settings
struct FsTuningState {
    /// Tuning by mount point
    by_mountpoint: BTreeMap<String, FilesystemTuning>,
    /// Default tuning
    default_tuning: FilesystemTuning,
    /// Global profile
    global_profile: TuningProfile,
}

impl FsTuningState {
    fn new() -> Self {
        Self {
            by_mountpoint: BTreeMap::new(),
            default_tuning: FilesystemTuning::default(),
            global_profile: TuningProfile::Balanced,
        }
    }
}

static FS_TUNING: Once<Mutex<FsTuningState>> = Once::new();

fn get_state() -> &'static Mutex<FsTuningState> {
    FS_TUNING.call_once(|| Mutex::new(FsTuningState::new()))
}

// ============================================================================
// Public API
// ============================================================================

/// Initialize filesystem tuning subsystem
pub fn init() {
    let _ = get_state();
    crate::kprintln!("fs-tuning: initialized with balanced profile");
}

/// Set global tuning profile
pub fn set_global_profile(profile: TuningProfile) {
    let mut state = get_state().lock();
    state.global_profile = profile;
    state.default_tuning = FilesystemTuning::from_profile(profile);
    state.default_tuning.apply();
}

/// Get current global profile
pub fn get_global_profile() -> TuningProfile {
    get_state().lock().global_profile
}

/// Set tuning for specific mount point
pub fn set_mountpoint_tuning(mountpoint: &str, tuning: FilesystemTuning) {
    let mut state = get_state().lock();
    state.by_mountpoint.insert(mountpoint.to_string(), tuning);
}

/// Get tuning for mount point (or default)
pub fn get_mountpoint_tuning(mountpoint: &str) -> FilesystemTuning {
    let state = get_state().lock();
    state.by_mountpoint.get(mountpoint)
        .cloned()
        .unwrap_or_else(|| state.default_tuning.clone())
}

/// Get default tuning
pub fn get_default_tuning() -> FilesystemTuning {
    get_state().lock().default_tuning.clone()
}

/// Auto-detect best profile for device
pub fn auto_detect_profile(is_ssd: bool, is_nvme: bool, _is_rotational: bool) -> TuningProfile {
    if is_nvme {
        TuningProfile::Nvme
    } else if is_ssd {
        TuningProfile::Ssd
    } else {
        TuningProfile::Hdd
    }
}

// ============================================================================
// Sysctl-style Parameters
// ============================================================================

/// Get dirty ratio
pub fn get_dirty_ratio() -> u32 {
    DIRTY_RATIO.load(Ordering::Relaxed)
}

/// Set dirty ratio
pub fn set_dirty_ratio(ratio: u32) {
    DIRTY_RATIO.store(ratio.min(100), Ordering::Relaxed);
}

/// Get background dirty ratio
pub fn get_dirty_background_ratio() -> u32 {
    DIRTY_BG_RATIO.load(Ordering::Relaxed)
}

/// Set background dirty ratio
pub fn set_dirty_background_ratio(ratio: u32) {
    DIRTY_BG_RATIO.store(ratio.min(100), Ordering::Relaxed);
}

/// Get dirty writeback interval (centisecs)
pub fn get_dirty_writeback_centisecs() -> u32 {
    DIRTY_WRITEBACK.load(Ordering::Relaxed)
}

/// Set dirty writeback interval
pub fn set_dirty_writeback_centisecs(centisecs: u32) {
    DIRTY_WRITEBACK.store(centisecs, Ordering::Relaxed);
}

/// Get dirty expire time (centisecs)
pub fn get_dirty_expire_centisecs() -> u32 {
    DIRTY_EXPIRE.load(Ordering::Relaxed)
}

/// Set dirty expire time
pub fn set_dirty_expire_centisecs(centisecs: u32) {
    DIRTY_EXPIRE.store(centisecs, Ordering::Relaxed);
}

// ============================================================================
// Ext4-specific Tuning
// ============================================================================

/// Ext4-specific tuning options
#[derive(Debug, Clone)]
pub struct Ext4Tuning {
    /// Enable inline data
    pub inline_data: bool,
    /// Enable metadata checksums
    pub metadata_csum: bool,
    /// Enable 64-bit block numbers
    pub bit64: bool,
    /// Enable flex_bg
    pub flex_bg: bool,
    /// Flex bg size (groups)
    pub flex_bg_size: u32,
    /// Enable huge_file
    pub huge_file: bool,
    /// Enable dir_nlink
    pub dir_nlink: bool,
    /// Enable extra_isize
    pub extra_isize: bool,
    /// Inode size
    pub inode_size: u32,
    /// Enable uninit_bg
    pub uninit_bg: bool,
    /// Enable dir_index
    pub dir_index: bool,
    /// Enable filetype
    pub filetype: bool,
}

impl Default for Ext4Tuning {
    fn default() -> Self {
        Self {
            inline_data: true,
            metadata_csum: true,
            bit64: true,
            flex_bg: true,
            flex_bg_size: 16,
            huge_file: true,
            dir_nlink: true,
            extra_isize: true,
            inode_size: 256,
            uninit_bg: true,
            dir_index: true,
            filetype: true,
        }
    }
}

// ============================================================================
// FAT32-specific Tuning
// ============================================================================

/// FAT32-specific tuning options
#[derive(Debug, Clone)]
pub struct Fat32Tuning {
    /// Use shortname display
    pub shortname: FatShortname,
    /// Codepage for short names
    pub codepage: u32,
    /// IO charset for long names
    pub iocharset: String,
    /// Default file permissions
    pub fmask: u16,
    /// Default directory permissions
    pub dmask: u16,
    /// Allow UID/GID in mount
    pub uid: u32,
    /// Default GID
    pub gid: u32,
    /// Enable NFS export
    pub nfs: bool,
    /// Quiet mode (don't report permission errors)
    pub quiet: bool,
    /// Flush after each write
    pub flush: bool,
    /// Use timezone setting
    pub tz_utc: bool,
}

/// FAT shortname display mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FatShortname {
    Lower,
    Win95,
    Winnt,
    Mixed,
}

impl Default for Fat32Tuning {
    fn default() -> Self {
        Self {
            shortname: FatShortname::Mixed,
            codepage: 437,
            iocharset: String::from("utf8"),
            fmask: 0o022,
            dmask: 0o022,
            uid: 0,
            gid: 0,
            nfs: false,
            quiet: false,
            flush: false,
            tz_utc: true,
        }
    }
}

// ============================================================================
// NTFS-specific Tuning
// ============================================================================

/// NTFS-specific tuning options
#[derive(Debug, Clone)]
pub struct NtfsTuning {
    /// Enable compression
    pub compression: bool,
    /// Enable sparse files
    pub sparse: bool,
    /// Default permissions mask
    pub permissions: u16,
    /// Enable case sensitivity
    pub case_sensitive: bool,
    /// Enable hidden files display
    pub show_hidden: bool,
    /// Enable system files display
    pub show_system: bool,
    /// Windows-compatible mode
    pub windows_compat: bool,
    /// Enable MFT zone reservation
    pub mft_zone: u8,
}

impl Default for NtfsTuning {
    fn default() -> Self {
        Self {
            compression: true,
            sparse: true,
            permissions: 0o755,
            case_sensitive: false,
            show_hidden: false,
            show_system: false,
            windows_compat: true,
            mft_zone: 1,
        }
    }
}

// ============================================================================
// Performance Monitoring
// ============================================================================

/// Filesystem performance statistics
#[derive(Debug, Clone, Default)]
pub struct FsPerformanceStats {
    /// Read operations
    pub reads: u64,
    /// Write operations
    pub writes: u64,
    /// Bytes read
    pub bytes_read: u64,
    /// Bytes written
    pub bytes_written: u64,
    /// Metadata operations
    pub metadata_ops: u64,
    /// Cache hits
    pub cache_hits: u64,
    /// Cache misses
    pub cache_misses: u64,
    /// Journal commits
    pub journal_commits: u64,
    /// Allocations
    pub allocations: u64,
    /// Deallocations
    pub deallocations: u64,
}

static FS_STATS: Once<Mutex<FsPerformanceStats>> = Once::new();

fn get_stats() -> &'static Mutex<FsPerformanceStats> {
    FS_STATS.call_once(|| Mutex::new(FsPerformanceStats::default()))
}

/// Get filesystem performance stats
pub fn get_performance_stats() -> FsPerformanceStats {
    get_stats().lock().clone()
}

/// Record read operation
pub fn record_read(bytes: u64) {
    let mut stats = get_stats().lock();
    stats.reads += 1;
    stats.bytes_read += bytes;
}

/// Record write operation
pub fn record_write(bytes: u64) {
    let mut stats = get_stats().lock();
    stats.writes += 1;
    stats.bytes_written += bytes;
}

/// Record cache hit
pub fn record_cache_hit() {
    get_stats().lock().cache_hits += 1;
}

/// Record cache miss
pub fn record_cache_miss() {
    get_stats().lock().cache_misses += 1;
}

/// Get cache hit ratio
pub fn cache_hit_ratio() -> f32 {
    let stats = get_stats().lock();
    let total = stats.cache_hits + stats.cache_misses;
    if total == 0 {
        return 0.0;
    }
    (stats.cache_hits as f32) / (total as f32) * 100.0
}

/// Reset performance stats
pub fn reset_stats() {
    *get_stats().lock() = FsPerformanceStats::default();
}
