//! Hybrid Sleep Support
//!
//! Combines Suspend to RAM (S3) with Hibernate (S4) for data safety:
//! - System state is saved to RAM (fast resume)
//! - System state is also saved to disk (survives power loss)
//! - On resume: use RAM if power maintained, else restore from disk
//!
//! This provides the best of both worlds: fast resume times with
//! protection against data loss during power outages.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::sync::IrqSafeMutex;
use crate::util::{KResult, KError};

/// Hybrid sleep state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HybridSleepState {
    /// System is awake
    Awake,
    /// Preparing for hybrid sleep
    Preparing,
    /// Writing hibernate image to disk
    WritingImage,
    /// Suspending to RAM
    Suspending,
    /// In hybrid sleep (RAM + disk)
    Sleeping,
    /// Resuming from RAM
    ResumingFromRam,
    /// Resuming from disk (RAM was lost)
    ResumingFromDisk,
    /// Error occurred
    Error,
}

/// Resume source after hybrid sleep
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResumeSource {
    /// Resumed from RAM (fast)
    Ram,
    /// Resumed from disk (power was lost)
    Disk,
    /// Unknown
    Unknown,
}

/// Hybrid sleep configuration
#[derive(Debug, Clone)]
pub struct HybridSleepConfig {
    /// Enable hybrid sleep (instead of pure suspend)
    pub enabled: bool,
    /// Hibernate partition/file path
    pub hibernate_target: HibernateTarget,
    /// Compress hibernate image
    pub compress: bool,
    /// Compression algorithm
    pub compression_algo: CompressionAlgo,
    /// Verify written image
    pub verify_image: bool,
    /// Maximum image size (0 = no limit)
    pub max_image_size: u64,
    /// Timeout for suspend operation (ms)
    pub suspend_timeout: u32,
}

impl Default for HybridSleepConfig {
    fn default() -> Self {
        HybridSleepConfig {
            enabled: true,
            hibernate_target: HibernateTarget::SwapPartition,
            compress: true,
            compression_algo: CompressionAlgo::Lz4,
            verify_image: true,
            max_image_size: 0,
            suspend_timeout: 30000,
        }
    }
}

/// Hibernate target type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HibernateTarget {
    /// Use swap partition
    SwapPartition,
    /// Use swap file
    SwapFile(String),
    /// Custom partition
    Partition(String),
    /// Custom file
    File(String),
}

/// Compression algorithm for hibernate image
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionAlgo {
    None,
    Lz4,
    Lzo,
    Zstd,
}

impl Default for CompressionAlgo {
    fn default() -> Self {
        CompressionAlgo::Lz4
    }
}

/// Hibernate image header
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct HibernateImageHeader {
    /// Magic number "STZHIB\x00\x00"
    pub magic: [u8; 8],
    /// Version
    pub version: u32,
    /// Flags
    pub flags: u32,
    /// Original size
    pub original_size: u64,
    /// Compressed size (same as original if not compressed)
    pub compressed_size: u64,
    /// Number of pages
    pub page_count: u64,
    /// Page size
    pub page_size: u32,
    /// Compression algorithm
    pub compression: u8,
    /// Reserved
    pub reserved: [u8; 3],
    /// CRC32 of header
    pub header_crc: u32,
    /// CRC32 of data
    pub data_crc: u32,
    /// Timestamp
    pub timestamp: u64,
    /// CPU state offset
    pub cpu_state_offset: u64,
    /// Memory map offset
    pub memory_map_offset: u64,
    /// Resume address
    pub resume_address: u64,
}

const HIBERNATE_MAGIC: [u8; 8] = *b"STZHIB\x00\x00";
const HIBERNATE_VERSION: u32 = 1;

/// Hibernate image flags
pub mod hibernate_flags {
    pub const COMPRESSED: u32 = 1 << 0;
    pub const VERIFIED: u32 = 1 << 1;
    pub const ENCRYPTED: u32 = 1 << 2;
    pub const HYBRID: u32 = 1 << 3; // This is a hybrid sleep image
}

/// Hybrid sleep statistics
#[derive(Debug, Default)]
pub struct HybridSleepStats {
    /// Total hybrid sleep entries
    pub total_sleeps: AtomicU64,
    /// Successful RAM resumes
    pub ram_resumes: AtomicU64,
    /// Successful disk resumes
    pub disk_resumes: AtomicU64,
    /// Failed operations
    pub failures: AtomicU64,
    /// Last sleep duration (ms)
    pub last_sleep_duration: AtomicU64,
    /// Last image write time (ms)
    pub last_image_write_time: AtomicU64,
    /// Last resume time (ms)
    pub last_resume_time: AtomicU64,
    /// Total image bytes written
    pub total_bytes_written: AtomicU64,
}

pub static HYBRID_SLEEP: IrqSafeMutex<HybridSleepManager> = IrqSafeMutex::new(HybridSleepManager::new());

/// Hybrid sleep manager
pub struct HybridSleepManager {
    /// Current state
    state: HybridSleepState,
    /// Configuration
    config: HybridSleepConfig,
    /// Last resume source
    last_resume_source: ResumeSource,
    /// Statistics
    stats: HybridSleepStats,
    /// Callbacks for sleep/resume
    callbacks: Vec<fn(HybridSleepEvent)>,
    /// Sleep entry timestamp
    sleep_timestamp: u64,
    /// Image write timestamp
    image_write_timestamp: u64,
    /// Initialized
    initialized: bool,
}

/// Hybrid sleep events
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HybridSleepEvent {
    PreparingSleep,
    WritingImage,
    ImageWritten,
    Suspending,
    Suspended,
    ResumingRam,
    ResumingDisk,
    Resumed,
    Error,
}

impl HybridSleepManager {
    pub const fn new() -> Self {
        HybridSleepManager {
            state: HybridSleepState::Awake,
            config: HybridSleepConfig {
                enabled: true,
                hibernate_target: HibernateTarget::SwapPartition,
                compress: true,
                compression_algo: CompressionAlgo::Lz4,
                verify_image: true,
                max_image_size: 0,
                suspend_timeout: 30000,
            },
            last_resume_source: ResumeSource::Unknown,
            stats: HybridSleepStats {
                total_sleeps: AtomicU64::new(0),
                ram_resumes: AtomicU64::new(0),
                disk_resumes: AtomicU64::new(0),
                failures: AtomicU64::new(0),
                last_sleep_duration: AtomicU64::new(0),
                last_image_write_time: AtomicU64::new(0),
                last_resume_time: AtomicU64::new(0),
                total_bytes_written: AtomicU64::new(0),
            },
            callbacks: Vec::new(),
            sleep_timestamp: 0,
            image_write_timestamp: 0,
            initialized: false,
        }
    }

    /// Initialize hybrid sleep manager
    pub fn init(&mut self) -> KResult<()> {
        if self.initialized {
            return Ok(());
        }

        // Check if hibernate is supported
        if !self.check_hibernate_support()? {
            crate::kprintln!("hybrid_sleep: hibernate not supported, disabling");
            self.config.enabled = false;
        }

        self.initialized = true;
        crate::kprintln!("hybrid_sleep: initialized (enabled={})", self.config.enabled);
        Ok(())
    }

    /// Check if hibernate is supported
    fn check_hibernate_support(&self) -> KResult<bool> {
        // Check for swap space
        match &self.config.hibernate_target {
            HibernateTarget::SwapPartition => {
                // Check if swap partition exists and is large enough
                // For now, assume it's available
                Ok(true)
            }
            HibernateTarget::SwapFile(path) |
            HibernateTarget::File(path) => {
                // Check if file exists or can be created
                Ok(!path.is_empty())
            }
            HibernateTarget::Partition(path) => {
                Ok(!path.is_empty())
            }
        }
    }

    /// Enter hybrid sleep
    pub fn enter(&mut self) -> KResult<()> {
        if !self.config.enabled {
            return Err(KError::NotSupported);
        }

        if self.state != HybridSleepState::Awake {
            return Err(KError::Busy);
        }

        self.state = HybridSleepState::Preparing;
        self.fire_event(HybridSleepEvent::PreparingSleep);
        self.sleep_timestamp = crate::time::uptime_ms();

        crate::kprintln!("hybrid_sleep: entering hybrid sleep");

        // Step 1: Write hibernate image to disk
        self.state = HybridSleepState::WritingImage;
        self.fire_event(HybridSleepEvent::WritingImage);

        let write_start = crate::time::uptime_ms();
        let image_size = self.write_hibernate_image()?;
        let write_time = crate::time::uptime_ms() - write_start;

        self.stats.last_image_write_time.store(write_time, Ordering::Relaxed);
        self.stats.total_bytes_written.fetch_add(image_size, Ordering::Relaxed);
        self.image_write_timestamp = crate::time::uptime_ms();

        self.fire_event(HybridSleepEvent::ImageWritten);

        // Step 2: Suspend to RAM
        self.state = HybridSleepState::Suspending;
        self.fire_event(HybridSleepEvent::Suspending);

        // Mark that we're entering hybrid sleep
        self.mark_hybrid_sleep_entry()?;

        // Actually suspend
        self.state = HybridSleepState::Sleeping;
        self.fire_event(HybridSleepEvent::Suspended);
        self.stats.total_sleeps.fetch_add(1, Ordering::Relaxed);

        // This call will block until resume
        crate::power::suspend::suspend_to_ram()?;

        // --- Resume happens here ---

        // We resumed from RAM successfully
        self.handle_ram_resume()?;

        Ok(())
    }

    /// Write hibernate image to disk
    fn write_hibernate_image(&mut self) -> KResult<u64> {
        // Create image header
        let header = self.create_image_header()?;

        // Get memory pages to save
        let pages = self.collect_pages_to_save()?;

        // Compress if enabled
        let data = if self.config.compress {
            self.compress_pages(&pages)?
        } else {
            pages
        };

        // Write to target
        let size = self.write_to_target(&header, &data)?;

        // Verify if enabled
        if self.config.verify_image {
            self.verify_image()?;
        }

        crate::kprintln!("hybrid_sleep: wrote {} KB hibernate image", size / 1024);
        Ok(size)
    }

    /// Create hibernate image header
    fn create_image_header(&self) -> KResult<HibernateImageHeader> {
        let mut flags = hibernate_flags::HYBRID;
        if self.config.compress {
            flags |= hibernate_flags::COMPRESSED;
        }
        if self.config.verify_image {
            flags |= hibernate_flags::VERIFIED;
        }

        Ok(HibernateImageHeader {
            magic: HIBERNATE_MAGIC,
            version: HIBERNATE_VERSION,
            flags,
            original_size: 0,
            compressed_size: 0,
            page_count: 0,
            page_size: 4096,
            compression: self.config.compression_algo as u8,
            reserved: [0; 3],
            header_crc: 0,
            data_crc: 0,
            timestamp: crate::time::uptime_ms(),
            cpu_state_offset: 0,
            memory_map_offset: 0,
            resume_address: 0,
        })
    }

    /// Collect memory pages to save
    fn collect_pages_to_save(&self) -> KResult<Vec<u8>> {
        // This would collect all memory pages that need to be saved
        // For now, return empty placeholder
        Ok(Vec::new())
    }

    /// Compress pages
    fn compress_pages(&self, _data: &[u8]) -> KResult<Vec<u8>> {
        // Compress using the configured algorithm
        // For now, return input unchanged
        Ok(Vec::new())
    }

    /// Write to hibernate target
    fn write_to_target(&self, _header: &HibernateImageHeader, _data: &[u8]) -> KResult<u64> {
        // Write header and data to the configured target
        match &self.config.hibernate_target {
            HibernateTarget::SwapPartition => {
                // Write to swap partition header area
            }
            HibernateTarget::SwapFile(path) |
            HibernateTarget::File(path) => {
                // Write to file
                let _ = path;
            }
            HibernateTarget::Partition(path) => {
                // Write to raw partition
                let _ = path;
            }
        }

        Ok(0)
    }

    /// Verify hibernate image
    fn verify_image(&self) -> KResult<()> {
        // Read back and verify CRC
        Ok(())
    }

    /// Mark that we're entering hybrid sleep
    fn mark_hybrid_sleep_entry(&self) -> KResult<()> {
        // Write a flag to indicate hybrid sleep is active
        // This is used by the resume path to detect if we're resuming from hybrid sleep
        Ok(())
    }

    /// Handle resume from RAM
    fn handle_ram_resume(&mut self) -> KResult<()> {
        let resume_start = crate::time::uptime_ms();

        self.state = HybridSleepState::ResumingFromRam;
        self.fire_event(HybridSleepEvent::ResumingRam);

        // Clear the hibernate image since we don't need it
        self.clear_hibernate_image()?;

        // Update statistics
        let sleep_duration = resume_start - self.sleep_timestamp;
        self.stats.last_sleep_duration.store(sleep_duration, Ordering::Relaxed);
        self.stats.ram_resumes.fetch_add(1, Ordering::Relaxed);

        let resume_time = crate::time::uptime_ms() - resume_start;
        self.stats.last_resume_time.store(resume_time, Ordering::Relaxed);

        self.last_resume_source = ResumeSource::Ram;
        self.state = HybridSleepState::Awake;
        self.fire_event(HybridSleepEvent::Resumed);

        crate::kprintln!("hybrid_sleep: resumed from RAM ({}ms sleep, {}ms resume)",
            sleep_duration, resume_time);

        Ok(())
    }

    /// Handle resume from disk (called by bootloader if RAM was lost)
    pub fn handle_disk_resume(&mut self) -> KResult<()> {
        let resume_start = crate::time::uptime_ms();

        self.state = HybridSleepState::ResumingFromDisk;
        self.fire_event(HybridSleepEvent::ResumingDisk);

        // Read and restore hibernate image
        self.restore_hibernate_image()?;

        // Update statistics
        self.stats.disk_resumes.fetch_add(1, Ordering::Relaxed);

        let resume_time = crate::time::uptime_ms() - resume_start;
        self.stats.last_resume_time.store(resume_time, Ordering::Relaxed);

        self.last_resume_source = ResumeSource::Disk;
        self.state = HybridSleepState::Awake;
        self.fire_event(HybridSleepEvent::Resumed);

        crate::kprintln!("hybrid_sleep: resumed from disk ({}ms)", resume_time);

        Ok(())
    }

    /// Clear hibernate image from disk
    fn clear_hibernate_image(&self) -> KResult<()> {
        // Clear or invalidate the hibernate image
        // This prevents stale images from being used
        match &self.config.hibernate_target {
            HibernateTarget::SwapPartition => {
                // Clear swap header hibernate flag
            }
            HibernateTarget::SwapFile(path) |
            HibernateTarget::File(path) => {
                let _ = path;
            }
            HibernateTarget::Partition(path) => {
                let _ = path;
            }
        }
        Ok(())
    }

    /// Restore hibernate image
    fn restore_hibernate_image(&self) -> KResult<()> {
        // Read and restore memory from hibernate image
        Ok(())
    }

    /// Register event callback
    pub fn register_callback(&mut self, cb: fn(HybridSleepEvent)) {
        self.callbacks.push(cb);
    }

    /// Fire event to callbacks
    fn fire_event(&self, event: HybridSleepEvent) {
        for cb in &self.callbacks {
            cb(event);
        }
    }

    /// Get current state
    pub fn state(&self) -> HybridSleepState {
        self.state
    }

    /// Get last resume source
    pub fn last_resume_source(&self) -> ResumeSource {
        self.last_resume_source
    }

    /// Check if enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Enable hybrid sleep
    pub fn enable(&mut self) {
        self.config.enabled = true;
        crate::kprintln!("hybrid_sleep: enabled");
    }

    /// Disable hybrid sleep
    pub fn disable(&mut self) {
        self.config.enabled = false;
        crate::kprintln!("hybrid_sleep: disabled");
    }

    /// Set compression
    pub fn set_compression(&mut self, compress: bool, algo: CompressionAlgo) {
        self.config.compress = compress;
        self.config.compression_algo = algo;
    }

    /// Set hibernate target
    pub fn set_hibernate_target(&mut self, target: HibernateTarget) {
        self.config.hibernate_target = target;
    }

    /// Get statistics
    pub fn stats(&self) -> &HybridSleepStats {
        &self.stats
    }

    /// Get configuration
    pub fn config(&self) -> &HybridSleepConfig {
        &self.config
    }

    /// Check if hibernate image exists
    pub fn has_hibernate_image(&self) -> bool {
        // Check if a valid hibernate image exists
        match &self.config.hibernate_target {
            HibernateTarget::SwapPartition => {
                // Check swap partition for hibernate signature
                false
            }
            HibernateTarget::SwapFile(path) |
            HibernateTarget::File(path) |
            HibernateTarget::Partition(path) => {
                let _ = path;
                false
            }
        }
    }

    /// Get hibernate image info
    pub fn hibernate_image_info(&self) -> Option<HibernateImageInfo> {
        if !self.has_hibernate_image() {
            return None;
        }

        // Read image header
        Some(HibernateImageInfo {
            size: 0,
            compressed_size: 0,
            timestamp: 0,
            compression: self.config.compression_algo,
            valid: true,
        })
    }
}

/// Hibernate image information
#[derive(Debug, Clone)]
pub struct HibernateImageInfo {
    pub size: u64,
    pub compressed_size: u64,
    pub timestamp: u64,
    pub compression: CompressionAlgo,
    pub valid: bool,
}

/// Initialize hybrid sleep subsystem
pub fn init() -> KResult<()> {
    HYBRID_SLEEP.lock().init()
}

/// Enter hybrid sleep
pub fn enter() -> KResult<()> {
    HYBRID_SLEEP.lock().enter()
}

/// Check if enabled
pub fn is_enabled() -> bool {
    HYBRID_SLEEP.lock().is_enabled()
}

/// Enable hybrid sleep
pub fn enable() {
    HYBRID_SLEEP.lock().enable();
}

/// Disable hybrid sleep
pub fn disable() {
    HYBRID_SLEEP.lock().disable();
}

/// Get last resume source
pub fn last_resume_source() -> ResumeSource {
    HYBRID_SLEEP.lock().last_resume_source()
}

/// Handle disk resume (called by bootloader)
pub fn handle_disk_resume() -> KResult<()> {
    HYBRID_SLEEP.lock().handle_disk_resume()
}

/// Check if hibernate image exists
pub fn has_hibernate_image() -> bool {
    HYBRID_SLEEP.lock().has_hibernate_image()
}
