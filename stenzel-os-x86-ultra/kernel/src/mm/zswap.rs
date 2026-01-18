//! Memory Compression (zswap/zram)
//!
//! Implements compressed memory storage for swap:
//! - zswap: Compressed cache for swap pages
//! - zram: Compressed RAM block device
//! - LZ4/ZSTD compression algorithms
//! - Memory pool management

use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use crate::sync::IrqSafeMutex;

/// Physical page frame number
pub type Pfn = u64;

/// Compression algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionAlgo {
    /// LZ4 - fastest
    Lz4,
    /// LZ4HC - slower but better ratio
    Lz4Hc,
    /// ZSTD - best ratio
    Zstd,
    /// LZO - legacy
    Lzo,
    /// No compression (for testing)
    None,
}

impl Default for CompressionAlgo {
    fn default() -> Self {
        CompressionAlgo::Lz4
    }
}

impl CompressionAlgo {
    /// Get compression level (1-22 for ZSTD, 1-12 for LZ4HC)
    pub fn default_level(&self) -> u8 {
        match self {
            CompressionAlgo::Lz4 => 1,
            CompressionAlgo::Lz4Hc => 9,
            CompressionAlgo::Zstd => 3,
            CompressionAlgo::Lzo => 1,
            CompressionAlgo::None => 0,
        }
    }
}

/// Compressed page entry
#[derive(Debug, Clone)]
pub struct CompressedPage {
    /// Original page frame number
    pub pfn: Pfn,
    /// Compressed data
    pub data: Vec<u8>,
    /// Original size (usually 4096)
    pub original_size: u32,
    /// Compression algorithm used
    pub algo: CompressionAlgo,
    /// Access count for LRU
    pub access_count: u32,
    /// Last access timestamp
    pub last_access: u64,
}

impl CompressedPage {
    pub fn compression_ratio(&self) -> f32 {
        if self.data.is_empty() {
            0.0
        } else {
            (self.original_size as f32) / (self.data.len() as f32)
        }
    }

    pub fn saved_bytes(&self) -> i64 {
        (self.original_size as i64) - (self.data.len() as i64)
    }
}

/// zswap statistics
#[derive(Debug, Clone, Default)]
pub struct ZswapStats {
    /// Total pages stored
    pub stored_pages: u64,
    /// Total bytes of original data
    pub original_bytes: u64,
    /// Total bytes of compressed data
    pub compressed_bytes: u64,
    /// Pages rejected (incompressible)
    pub rejected_pages: u64,
    /// Pages written back to swap
    pub writeback_pages: u64,
    /// Duplicate pages (same content)
    pub duplicate_pages: u64,
    /// Cache hits
    pub hits: u64,
    /// Cache misses
    pub misses: u64,
    /// Compression failures
    pub compress_failures: u64,
    /// Decompression failures
    pub decompress_failures: u64,
}

impl ZswapStats {
    pub fn compression_ratio(&self) -> f32 {
        if self.compressed_bytes == 0 {
            0.0
        } else {
            (self.original_bytes as f32) / (self.compressed_bytes as f32)
        }
    }

    pub fn hit_ratio(&self) -> f32 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            (self.hits as f32) / (total as f32)
        }
    }

    pub fn saved_bytes(&self) -> i64 {
        (self.original_bytes as i64) - (self.compressed_bytes as i64)
    }
}

/// zswap configuration
#[derive(Debug, Clone)]
pub struct ZswapConfig {
    /// Enable zswap
    pub enabled: bool,
    /// Compression algorithm
    pub compressor: CompressionAlgo,
    /// Maximum pool size (percentage of total RAM)
    pub max_pool_percent: u8,
    /// Accept pages that don't compress well
    pub accept_incompressible: bool,
    /// Same-filled page optimization
    pub same_filled_pages: bool,
    /// Writeback when pool is full
    pub writeback_enabled: bool,
    /// Writeback threshold (percentage of max pool)
    pub writeback_threshold: u8,
}

impl Default for ZswapConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            compressor: CompressionAlgo::Lz4,
            max_pool_percent: 20, // 20% of RAM
            accept_incompressible: false,
            same_filled_pages: true,
            writeback_enabled: true,
            writeback_threshold: 90,
        }
    }
}

/// zswap pool manager
pub struct ZswapPool {
    /// Configuration
    config: ZswapConfig,
    /// Compressed pages
    pages: BTreeMap<Pfn, CompressedPage>,
    /// Statistics
    stats: ZswapStats,
    /// Maximum pool size in bytes
    max_pool_bytes: u64,
    /// Current pool size in bytes
    current_pool_bytes: u64,
    /// Same-filled page hashes (for dedup)
    same_filled_cache: BTreeMap<u8, Pfn>,
}

impl ZswapPool {
    pub fn new(config: ZswapConfig, total_ram_bytes: u64) -> Self {
        let max_pool_bytes = (total_ram_bytes * (config.max_pool_percent as u64)) / 100;

        Self {
            config,
            pages: BTreeMap::new(),
            stats: ZswapStats::default(),
            max_pool_bytes,
            current_pool_bytes: 0,
            same_filled_cache: BTreeMap::new(),
        }
    }

    /// Store a page in zswap
    pub fn store(&mut self, pfn: Pfn, page_data: &[u8]) -> Result<(), ZswapError> {
        if !self.config.enabled {
            return Err(ZswapError::Disabled);
        }

        let page_size = page_data.len();
        if page_size != 4096 {
            return Err(ZswapError::InvalidPageSize);
        }

        // Check for same-filled pages
        if self.config.same_filled_pages {
            if let Some(fill_byte) = self.is_same_filled(page_data) {
                // Store just the fill byte
                if let Some(&existing_pfn) = self.same_filled_cache.get(&fill_byte) {
                    self.stats.duplicate_pages += 1;
                    // Reference existing page
                    return Ok(());
                }

                self.same_filled_cache.insert(fill_byte, pfn);
                let entry = CompressedPage {
                    pfn,
                    data: vec![fill_byte],
                    original_size: page_size as u32,
                    algo: CompressionAlgo::None,
                    access_count: 0,
                    last_access: self.current_time(),
                };

                self.pages.insert(pfn, entry);
                self.stats.stored_pages += 1;
                self.stats.original_bytes += page_size as u64;
                self.stats.compressed_bytes += 1;
                self.current_pool_bytes += 1;

                return Ok(());
            }
        }

        // Compress the page
        let compressed = self.compress(page_data)?;

        // Check compression ratio
        if compressed.len() >= page_data.len() && !self.config.accept_incompressible {
            self.stats.rejected_pages += 1;
            return Err(ZswapError::IncompressiblePage);
        }

        // Check pool size
        if self.current_pool_bytes + (compressed.len() as u64) > self.max_pool_bytes {
            if self.config.writeback_enabled {
                self.writeback_lru()?;
            } else {
                return Err(ZswapError::PoolFull);
            }
        }

        // Store compressed page
        let entry = CompressedPage {
            pfn,
            data: compressed.clone(),
            original_size: page_size as u32,
            algo: self.config.compressor,
            access_count: 0,
            last_access: self.current_time(),
        };

        self.current_pool_bytes += compressed.len() as u64;
        self.stats.stored_pages += 1;
        self.stats.original_bytes += page_size as u64;
        self.stats.compressed_bytes += compressed.len() as u64;

        self.pages.insert(pfn, entry);

        Ok(())
    }

    /// Load a page from zswap
    pub fn load(&mut self, pfn: Pfn) -> Result<Vec<u8>, ZswapError> {
        if !self.config.enabled {
            return Err(ZswapError::Disabled);
        }

        // Get the current time outside of the borrow
        let now = self.current_time();

        // First check if page exists and extract needed data
        let page_data = match self.pages.get_mut(&pfn) {
            Some(entry) => {
                entry.access_count += 1;
                entry.last_access = now;

                // Check for same-filled page
                if entry.data.len() == 1 && entry.algo == CompressionAlgo::None {
                    let fill_byte = entry.data[0];
                    return {
                        self.stats.hits += 1;
                        Ok(vec![fill_byte; entry.original_size as usize])
                    };
                }

                // Clone data for decompression (releases the borrow)
                Some((entry.data.clone(), entry.original_size as usize))
            }
            None => None,
        };

        // Now process outside the borrow
        match page_data {
            Some((data, original_size)) => {
                self.stats.hits += 1;
                self.decompress(&data, original_size)
            }
            None => {
                self.stats.misses += 1;
                Err(ZswapError::NotFound)
            }
        }
    }

    /// Invalidate a page
    pub fn invalidate(&mut self, pfn: Pfn) -> bool {
        if let Some(entry) = self.pages.remove(&pfn) {
            self.current_pool_bytes -= entry.data.len() as u64;

            // Remove from same-filled cache if applicable
            if entry.data.len() == 1 {
                self.same_filled_cache.retain(|_, &mut p| p != pfn);
            }

            true
        } else {
            false
        }
    }

    /// Get statistics
    pub fn stats(&self) -> &ZswapStats {
        &self.stats
    }

    /// Get pool usage
    pub fn pool_usage(&self) -> (u64, u64) {
        (self.current_pool_bytes, self.max_pool_bytes)
    }

    /// Check if page is same-filled
    fn is_same_filled(&self, data: &[u8]) -> Option<u8> {
        if data.is_empty() {
            return None;
        }

        let first = data[0];
        if data.iter().all(|&b| b == first) {
            Some(first)
        } else {
            None
        }
    }

    /// Compress data using configured algorithm
    fn compress(&mut self, data: &[u8]) -> Result<Vec<u8>, ZswapError> {
        match self.config.compressor {
            CompressionAlgo::Lz4 | CompressionAlgo::Lz4Hc => {
                self.compress_lz4(data)
            }
            CompressionAlgo::Zstd => {
                self.compress_zstd(data)
            }
            CompressionAlgo::Lzo => {
                self.compress_lzo(data)
            }
            CompressionAlgo::None => {
                Ok(data.to_vec())
            }
        }
    }

    /// Decompress data
    fn decompress(&self, data: &[u8], original_size: usize) -> Result<Vec<u8>, ZswapError> {
        // For now, use simple RLE-like decompression as placeholder
        // In real implementation, would use proper LZ4/ZSTD libs

        // Simple placeholder: just return the data if it matches original size
        if data.len() == original_size {
            return Ok(data.to_vec());
        }

        // Try to decompress
        let mut output = Vec::with_capacity(original_size);

        // Simple RLE decode for testing
        let mut i = 0;
        while i < data.len() && output.len() < original_size {
            if i + 1 < data.len() && data[i] == 0xFF {
                // Escape sequence
                let count = data[i + 1] as usize;
                if i + 2 < data.len() {
                    let byte = data[i + 2];
                    for _ in 0..count {
                        output.push(byte);
                    }
                    i += 3;
                } else {
                    break;
                }
            } else {
                output.push(data[i]);
                i += 1;
            }
        }

        // Pad to original size if needed
        while output.len() < original_size {
            output.push(0);
        }

        Ok(output)
    }

    /// LZ4 compression (placeholder)
    fn compress_lz4(&mut self, data: &[u8]) -> Result<Vec<u8>, ZswapError> {
        // Simple RLE-like compression as placeholder
        let mut output = Vec::new();
        let mut i = 0;

        while i < data.len() {
            let byte = data[i];
            let mut count = 1usize;

            // Count repetitions
            while i + count < data.len() && data[i + count] == byte && count < 255 {
                count += 1;
            }

            if count >= 4 {
                // RLE encode: 0xFF, count, byte
                output.push(0xFF);
                output.push(count as u8);
                output.push(byte);
            } else {
                // Literal
                for _ in 0..count {
                    if byte == 0xFF {
                        output.push(0xFF);
                        output.push(1);
                        output.push(0xFF);
                    } else {
                        output.push(byte);
                    }
                }
            }

            i += count;
        }

        Ok(output)
    }

    /// ZSTD compression (placeholder)
    fn compress_zstd(&mut self, data: &[u8]) -> Result<Vec<u8>, ZswapError> {
        // Use LZ4 as placeholder
        self.compress_lz4(data)
    }

    /// LZO compression (placeholder)
    fn compress_lzo(&mut self, data: &[u8]) -> Result<Vec<u8>, ZswapError> {
        // Use LZ4 as placeholder
        self.compress_lz4(data)
    }

    /// Writeback least recently used pages to swap
    fn writeback_lru(&mut self) -> Result<(), ZswapError> {
        // Find LRU pages
        let mut pages_by_access: Vec<(Pfn, u64)> = self.pages
            .iter()
            .map(|(&pfn, entry)| (pfn, entry.last_access))
            .collect();

        pages_by_access.sort_by_key(|&(_, access)| access);

        // Writeback oldest 10% of pages
        let writeback_count = (self.pages.len() / 10).max(1);

        for (pfn, _) in pages_by_access.into_iter().take(writeback_count) {
            if let Some(entry) = self.pages.remove(&pfn) {
                self.current_pool_bytes -= entry.data.len() as u64;
                self.stats.writeback_pages += 1;
                // In real implementation, would write to actual swap device
            }
        }

        Ok(())
    }

    /// Get current timestamp
    fn current_time(&self) -> u64 {
        crate::time::realtime().tv_sec as u64
    }
}

/// zswap error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZswapError {
    /// zswap is disabled
    Disabled,
    /// Pool is full
    PoolFull,
    /// Page not found
    NotFound,
    /// Invalid page size
    InvalidPageSize,
    /// Page doesn't compress well
    IncompressiblePage,
    /// Compression failed
    CompressionFailed,
    /// Decompression failed
    DecompressionFailed,
}

// ============================================================================
// zram (compressed RAM disk)
// ============================================================================

/// zram configuration
#[derive(Debug, Clone)]
pub struct ZramConfig {
    /// Device size in bytes
    pub size_bytes: u64,
    /// Compression algorithm
    pub compressor: CompressionAlgo,
    /// Number of compression streams
    pub num_streams: u8,
    /// Memory limit (0 = no limit)
    pub mem_limit: u64,
}

impl Default for ZramConfig {
    fn default() -> Self {
        Self {
            size_bytes: 512 * 1024 * 1024, // 512 MB
            compressor: CompressionAlgo::Lz4,
            num_streams: 4,
            mem_limit: 0,
        }
    }
}

/// zram statistics
#[derive(Debug, Clone, Default)]
pub struct ZramStats {
    /// Disk size
    pub disk_size: u64,
    /// Number of stored pages
    pub pages_stored: u64,
    /// Original data size
    pub orig_data_size: u64,
    /// Compressed data size
    pub compr_data_size: u64,
    /// Memory used
    pub mem_used_total: u64,
    /// Same pages (deduplicated)
    pub same_pages: u64,
    /// Number of reads
    pub num_reads: u64,
    /// Number of writes
    pub num_writes: u64,
    /// Invalid I/O
    pub invalid_io: u64,
}

/// zram device
pub struct ZramDevice {
    /// Device ID
    id: u32,
    /// Configuration
    config: ZramConfig,
    /// Compressed sectors
    sectors: BTreeMap<u64, Vec<u8>>,
    /// Statistics
    stats: ZramStats,
    /// Sector size (usually 4096)
    sector_size: u32,
    /// Number of sectors
    num_sectors: u64,
}

impl ZramDevice {
    pub fn new(id: u32, config: ZramConfig) -> Self {
        let sector_size = 4096u32;
        let num_sectors = config.size_bytes / (sector_size as u64);

        Self {
            id,
            config: config.clone(),
            sectors: BTreeMap::new(),
            stats: ZramStats {
                disk_size: config.size_bytes,
                ..Default::default()
            },
            sector_size,
            num_sectors,
        }
    }

    /// Read a sector
    pub fn read_sector(&mut self, sector: u64) -> Result<Vec<u8>, ZramError> {
        if sector >= self.num_sectors {
            self.stats.invalid_io += 1;
            return Err(ZramError::InvalidSector);
        }

        self.stats.num_reads += 1;

        match self.sectors.get(&sector) {
            Some(compressed) => {
                // Decompress
                self.decompress(compressed)
            }
            None => {
                // Return zeros for unwritten sectors
                Ok(vec![0u8; self.sector_size as usize])
            }
        }
    }

    /// Write a sector
    pub fn write_sector(&mut self, sector: u64, data: &[u8]) -> Result<(), ZramError> {
        if sector >= self.num_sectors {
            self.stats.invalid_io += 1;
            return Err(ZramError::InvalidSector);
        }

        if data.len() != self.sector_size as usize {
            return Err(ZramError::InvalidSize);
        }

        self.stats.num_writes += 1;

        // Check for zero-filled sector
        if data.iter().all(|&b| b == 0) {
            // Don't store zero sectors
            if self.sectors.remove(&sector).is_some() {
                self.stats.pages_stored -= 1;
            }
            return Ok(());
        }

        // Compress
        let compressed = self.compress(data)?;

        // Update stats
        let old_size = self.sectors.get(&sector).map(|v| v.len()).unwrap_or(0);

        if old_size == 0 {
            self.stats.pages_stored += 1;
            self.stats.orig_data_size += data.len() as u64;
        }

        self.stats.compr_data_size = self.stats.compr_data_size
            .saturating_sub(old_size as u64)
            + compressed.len() as u64;

        self.sectors.insert(sector, compressed);

        Ok(())
    }

    /// Discard a sector
    pub fn discard_sector(&mut self, sector: u64) -> Result<(), ZramError> {
        if sector >= self.num_sectors {
            return Err(ZramError::InvalidSector);
        }

        if let Some(data) = self.sectors.remove(&sector) {
            self.stats.pages_stored -= 1;
            self.stats.orig_data_size -= self.sector_size as u64;
            self.stats.compr_data_size -= data.len() as u64;
        }

        Ok(())
    }

    /// Get statistics
    pub fn stats(&self) -> &ZramStats {
        &self.stats
    }

    /// Reset device
    pub fn reset(&mut self) {
        self.sectors.clear();
        self.stats = ZramStats {
            disk_size: self.config.size_bytes,
            ..Default::default()
        };
    }

    /// Compress data
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, ZramError> {
        // Simple RLE compression
        let mut output = Vec::new();
        let mut i = 0;

        while i < data.len() {
            let byte = data[i];
            let mut count = 1usize;

            while i + count < data.len() && data[i + count] == byte && count < 255 {
                count += 1;
            }

            if count >= 4 {
                output.push(0xFF);
                output.push(count as u8);
                output.push(byte);
            } else {
                for _ in 0..count {
                    if byte == 0xFF {
                        output.push(0xFF);
                        output.push(1);
                        output.push(0xFF);
                    } else {
                        output.push(byte);
                    }
                }
            }

            i += count;
        }

        Ok(output)
    }

    /// Decompress data
    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, ZramError> {
        let mut output = Vec::with_capacity(self.sector_size as usize);
        let mut i = 0;

        while i < data.len() && output.len() < self.sector_size as usize {
            if i + 1 < data.len() && data[i] == 0xFF {
                let count = data[i + 1] as usize;
                if i + 2 < data.len() {
                    let byte = data[i + 2];
                    for _ in 0..count {
                        output.push(byte);
                    }
                    i += 3;
                } else {
                    break;
                }
            } else {
                output.push(data[i]);
                i += 1;
            }
        }

        while output.len() < self.sector_size as usize {
            output.push(0);
        }

        Ok(output)
    }
}

/// zram error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZramError {
    /// Invalid sector number
    InvalidSector,
    /// Invalid data size
    InvalidSize,
    /// Compression failed
    CompressionFailed,
    /// Decompression failed
    DecompressionFailed,
    /// Memory limit reached
    MemoryLimit,
}

// ============================================================================
// Global state
// ============================================================================

/// Global zswap pool
static ZSWAP_POOL: IrqSafeMutex<Option<ZswapPool>> = IrqSafeMutex::new(None);

/// zswap enabled flag
static ZSWAP_ENABLED: AtomicBool = AtomicBool::new(false);

/// zram devices
static ZRAM_DEVICES: IrqSafeMutex<Vec<ZramDevice>> = IrqSafeMutex::new(Vec::new());

/// Initialize zswap
pub fn init_zswap(total_ram: u64, config: ZswapConfig) {
    let pool = ZswapPool::new(config, total_ram);
    *ZSWAP_POOL.lock() = Some(pool);
    ZSWAP_ENABLED.store(true, Ordering::Release);
    crate::util::kprintln!("zswap: initialized with {}% pool", 20);
}

/// Store page in zswap
pub fn zswap_store(pfn: Pfn, data: &[u8]) -> Result<(), ZswapError> {
    let mut pool = ZSWAP_POOL.lock();
    match pool.as_mut() {
        Some(p) => p.store(pfn, data),
        None => Err(ZswapError::Disabled),
    }
}

/// Load page from zswap
pub fn zswap_load(pfn: Pfn) -> Result<Vec<u8>, ZswapError> {
    let mut pool = ZSWAP_POOL.lock();
    match pool.as_mut() {
        Some(p) => p.load(pfn),
        None => Err(ZswapError::Disabled),
    }
}

/// Invalidate page in zswap
pub fn zswap_invalidate(pfn: Pfn) -> bool {
    let mut pool = ZSWAP_POOL.lock();
    match pool.as_mut() {
        Some(p) => p.invalidate(pfn),
        None => false,
    }
}

/// Get zswap statistics
pub fn zswap_stats() -> Option<ZswapStats> {
    ZSWAP_POOL.lock().as_ref().map(|p| p.stats().clone())
}

/// Create a zram device
pub fn create_zram(config: ZramConfig) -> u32 {
    let mut devices = ZRAM_DEVICES.lock();
    let id = devices.len() as u32;
    devices.push(ZramDevice::new(id, config));
    crate::util::kprintln!("zram{}: created", id);
    id
}

/// Get zram statistics
pub fn zram_stats(id: u32) -> Option<ZramStats> {
    ZRAM_DEVICES.lock().get(id as usize).map(|d| d.stats().clone())
}

/// Check if zswap is enabled
pub fn is_zswap_enabled() -> bool {
    ZSWAP_ENABLED.load(Ordering::Acquire)
}
