//! AMD StoreMI Support
//!
//! Storage acceleration technology combining SSD and HDD:
//! - Tiered storage with automatic data migration
//! - Hot data caching on SSD
//! - Cold data on HDD
//! - Transparent to applications

extern crate alloc;

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::sync::IrqSafeMutex;
use crate::util::{KResult, KError};

/// StoreMI tier type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageTier {
    /// Fast tier (SSD/NVMe)
    Fast,
    /// Capacity tier (HDD)
    Capacity,
}

/// StoreMI device state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreMiState {
    /// Not initialized
    Uninitialized,
    /// Initializing
    Initializing,
    /// Active and caching
    Active,
    /// Degraded (one tier offline)
    Degraded,
    /// Rebuilding/migrating
    Rebuilding,
    /// Stopped
    Stopped,
    /// Error
    Error,
}

/// Data temperature (access frequency)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DataTemperature {
    /// Cold - rarely accessed
    Cold = 0,
    /// Cool - occasionally accessed
    Cool = 1,
    /// Warm - moderately accessed
    Warm = 2,
    /// Hot - frequently accessed
    Hot = 3,
}

/// Block tracking entry
#[derive(Debug, Clone, Copy)]
pub struct BlockEntry {
    /// Logical block address
    pub lba: u64,
    /// Current tier
    pub tier: StorageTier,
    /// Access count
    pub access_count: u32,
    /// Last access timestamp
    pub last_access: u64,
    /// Data temperature
    pub temperature: DataTemperature,
    /// Dirty (needs writeback)
    pub dirty: bool,
}

/// Storage device info
#[derive(Debug, Clone)]
pub struct StorageDeviceInfo {
    /// Device path
    pub path: String,
    /// Device size in bytes
    pub size: u64,
    /// Block size
    pub block_size: u32,
    /// Is SSD
    pub is_ssd: bool,
    /// Device tier
    pub tier: StorageTier,
    /// Read speed (MB/s estimated)
    pub read_speed: u32,
    /// Write speed (MB/s estimated)
    pub write_speed: u32,
    /// Device online
    pub online: bool,
}

/// StoreMI configuration
#[derive(Debug, Clone)]
pub struct StoreMiConfig {
    /// Fast tier device
    pub fast_device: String,
    /// Capacity tier device
    pub capacity_device: String,
    /// Cache size on fast tier (bytes)
    pub cache_size: u64,
    /// Hot threshold (access count)
    pub hot_threshold: u32,
    /// Cold threshold (seconds since last access)
    pub cold_threshold: u64,
    /// Migration batch size (blocks)
    pub migration_batch: u32,
    /// Enable write caching
    pub write_cache: bool,
    /// Enable read caching
    pub read_cache: bool,
    /// Background migration enabled
    pub background_migration: bool,
}

impl Default for StoreMiConfig {
    fn default() -> Self {
        StoreMiConfig {
            fast_device: String::new(),
            capacity_device: String::new(),
            cache_size: 32 * 1024 * 1024 * 1024, // 32 GB default
            hot_threshold: 10,
            cold_threshold: 86400, // 24 hours
            migration_batch: 1024,
            write_cache: true,
            read_cache: true,
            background_migration: true,
        }
    }
}

/// StoreMI statistics
#[derive(Debug, Default)]
pub struct StoreMiStats {
    /// Total reads
    pub total_reads: AtomicU64,
    /// Total writes
    pub total_writes: AtomicU64,
    /// Fast tier hits
    pub fast_hits: AtomicU64,
    /// Capacity tier hits
    pub capacity_hits: AtomicU64,
    /// Promotions (capacity -> fast)
    pub promotions: AtomicU64,
    /// Demotions (fast -> capacity)
    pub demotions: AtomicU64,
    /// Bytes promoted
    pub bytes_promoted: AtomicU64,
    /// Bytes demoted
    pub bytes_demoted: AtomicU64,
    /// Cache hit ratio (x100)
    pub hit_ratio: AtomicU64,
}

/// StoreMI volume
#[derive(Debug)]
pub struct StoreMiVolume {
    /// Volume ID
    pub id: u32,
    /// Volume name
    pub name: String,
    /// Current state
    pub state: StoreMiState,
    /// Configuration
    pub config: StoreMiConfig,
    /// Fast tier info
    pub fast_tier: Option<StorageDeviceInfo>,
    /// Capacity tier info
    pub capacity_tier: Option<StorageDeviceInfo>,
    /// Total capacity
    pub total_capacity: u64,
    /// Used capacity
    pub used_capacity: u64,
    /// Fast tier used
    pub fast_used: u64,
    /// Block map (simplified - would be on-disk in real impl)
    pub block_map: Vec<BlockEntry>,
    /// Statistics
    pub stats: StoreMiStats,
}

impl StoreMiVolume {
    pub fn new(id: u32, name: String, config: StoreMiConfig) -> Self {
        StoreMiVolume {
            id,
            name,
            state: StoreMiState::Uninitialized,
            config,
            fast_tier: None,
            capacity_tier: None,
            total_capacity: 0,
            used_capacity: 0,
            fast_used: 0,
            block_map: Vec::new(),
            stats: StoreMiStats::default(),
        }
    }

    /// Calculate hit ratio
    pub fn hit_ratio(&self) -> f32 {
        let fast = self.stats.fast_hits.load(Ordering::Relaxed);
        let total = fast + self.stats.capacity_hits.load(Ordering::Relaxed);
        if total == 0 {
            0.0
        } else {
            (fast as f32 / total as f32) * 100.0
        }
    }
}

/// StoreMI manager
pub struct StoreMiManager {
    /// Volumes
    volumes: Vec<StoreMiVolume>,
    /// Next volume ID
    next_id: u32,
    /// AMD StoreMI supported
    supported: bool,
    /// Initialized
    initialized: bool,
}

pub static STOREMI_MANAGER: IrqSafeMutex<StoreMiManager> = IrqSafeMutex::new(StoreMiManager::new());

impl StoreMiManager {
    pub const fn new() -> Self {
        StoreMiManager {
            volumes: Vec::new(),
            next_id: 1,
            supported: false,
            initialized: false,
        }
    }

    /// Initialize StoreMI
    pub fn init(&mut self) -> KResult<()> {
        if self.initialized {
            return Ok(());
        }

        // Check for AMD platform
        self.supported = self.detect_amd_platform();

        self.initialized = true;
        crate::kprintln!("amd_storemi: initialized (supported={})", self.supported);
        Ok(())
    }

    /// Detect AMD platform
    fn detect_amd_platform(&self) -> bool {
        // Check CPUID for AMD
        let cpuid = unsafe {
            let mut vendor: [u32; 3] = [0; 3];
            core::arch::x86_64::__cpuid(0);
            let result = core::arch::x86_64::__cpuid(0);
            vendor[0] = result.ebx;
            vendor[1] = result.edx;
            vendor[2] = result.ecx;
            vendor
        };

        // "AuthenticAMD"
        cpuid[0] == 0x68747541 && cpuid[1] == 0x69746e65 && cpuid[2] == 0x444d4163
    }

    /// Create a StoreMI volume
    pub fn create_volume(
        &mut self,
        name: String,
        fast_device: String,
        capacity_device: String,
    ) -> KResult<u32> {
        if !self.supported {
            return Err(KError::NotSupported);
        }

        let mut config = StoreMiConfig::default();
        config.fast_device = fast_device.clone();
        config.capacity_device = capacity_device.clone();

        let id = self.next_id;
        self.next_id += 1;

        let mut volume = StoreMiVolume::new(id, name, config);

        // Probe devices
        volume.fast_tier = self.probe_device(&fast_device, StorageTier::Fast)?;
        volume.capacity_tier = self.probe_device(&capacity_device, StorageTier::Capacity)?;

        // Calculate total capacity
        if let (Some(fast), Some(cap)) = (&volume.fast_tier, &volume.capacity_tier) {
            volume.total_capacity = fast.size + cap.size;
        }

        volume.state = StoreMiState::Active;
        self.volumes.push(volume);

        crate::kprintln!("amd_storemi: created volume {} (id={})",
            self.volumes.last().map(|v| v.name.as_str()).unwrap_or(""), id);

        Ok(id)
    }

    /// Probe storage device
    fn probe_device(&self, path: &str, tier: StorageTier) -> KResult<Option<StorageDeviceInfo>> {
        // In real implementation, would query block device
        // For now, return placeholder

        let is_ssd = tier == StorageTier::Fast;

        Ok(Some(StorageDeviceInfo {
            path: String::from(path),
            size: if is_ssd { 512 * 1024 * 1024 * 1024 } else { 2 * 1024 * 1024 * 1024 * 1024 }, // 512GB SSD / 2TB HDD
            block_size: 4096,
            is_ssd,
            tier,
            read_speed: if is_ssd { 3500 } else { 150 },
            write_speed: if is_ssd { 3000 } else { 140 },
            online: true,
        }))
    }

    /// Delete a volume
    pub fn delete_volume(&mut self, id: u32) -> KResult<()> {
        let pos = self.volumes.iter().position(|v| v.id == id)
            .ok_or(KError::NotFound)?;

        let volume = &self.volumes[pos];
        if volume.state == StoreMiState::Rebuilding {
            return Err(KError::Busy);
        }

        self.volumes.remove(pos);
        crate::kprintln!("amd_storemi: deleted volume {}", id);
        Ok(())
    }

    /// Handle read I/O
    pub fn handle_read(&mut self, volume_id: u32, lba: u64, count: u32) -> KResult<(StorageTier, Vec<u8>)> {
        let volume = self.volumes.iter_mut().find(|v| v.id == volume_id)
            .ok_or(KError::NotFound)?;

        if volume.state != StoreMiState::Active && volume.state != StoreMiState::Degraded {
            return Err(KError::IO);
        }

        volume.stats.total_reads.fetch_add(1, Ordering::Relaxed);

        // Check block map for location
        let tier = Self::lookup_block_tier_static(volume, lba);

        match tier {
            StorageTier::Fast => {
                volume.stats.fast_hits.fetch_add(1, Ordering::Relaxed);
            }
            StorageTier::Capacity => {
                volume.stats.capacity_hits.fetch_add(1, Ordering::Relaxed);
                // Consider promotion
                Self::maybe_promote_static(volume, lba);
            }
        }

        // Update access tracking
        Self::update_access_static(volume, lba);

        // Return data (placeholder)
        let data = vec![0u8; (count * 512) as usize];
        Ok((tier, data))
    }

    /// Handle write I/O
    pub fn handle_write(&mut self, volume_id: u32, lba: u64, _data: &[u8]) -> KResult<StorageTier> {
        let volume = self.volumes.iter_mut().find(|v| v.id == volume_id)
            .ok_or(KError::NotFound)?;

        if volume.state != StoreMiState::Active {
            return Err(KError::IO);
        }

        volume.stats.total_writes.fetch_add(1, Ordering::Relaxed);

        // Write to fast tier if caching enabled and space available
        let tier = if volume.config.write_cache && volume.fast_used < volume.config.cache_size {
            StorageTier::Fast
        } else {
            StorageTier::Capacity
        };

        // Update block map
        Self::update_block_entry_static(volume, lba, tier, true);

        Ok(tier)
    }

    /// Lookup which tier a block is on (static)
    fn lookup_block_tier_static(volume: &StoreMiVolume, lba: u64) -> StorageTier {
        volume.block_map.iter()
            .find(|b| b.lba == lba)
            .map(|b| b.tier)
            .unwrap_or(StorageTier::Capacity)
    }

    /// Maybe promote a block to fast tier (static)
    fn maybe_promote_static(volume: &mut StoreMiVolume, lba: u64) {
        let should_promote = volume.block_map.iter().find(|b| b.lba == lba)
            .map(|entry| {
                entry.access_count >= volume.config.hot_threshold
                    && entry.tier == StorageTier::Capacity
                    && volume.fast_used < volume.config.cache_size
            })
            .unwrap_or(false);

        if should_promote {
            Self::promote_block_static(volume, lba);
        }
    }

    /// Promote a block to fast tier (static)
    fn promote_block_static(volume: &mut StoreMiVolume, lba: u64) {
        if let Some(entry) = volume.block_map.iter_mut().find(|b| b.lba == lba) {
            entry.tier = StorageTier::Fast;
            entry.temperature = DataTemperature::Hot;
            volume.stats.promotions.fetch_add(1, Ordering::Relaxed);
            volume.stats.bytes_promoted.fetch_add(4096, Ordering::Relaxed);
            volume.fast_used += 4096;
        }
    }

    /// Demote a block to capacity tier (static)
    fn demote_block_static(volume: &mut StoreMiVolume, lba: u64) {
        if let Some(entry) = volume.block_map.iter_mut().find(|b| b.lba == lba) {
            entry.tier = StorageTier::Capacity;
            entry.temperature = DataTemperature::Cold;
            volume.stats.demotions.fetch_add(1, Ordering::Relaxed);
            volume.stats.bytes_demoted.fetch_add(4096, Ordering::Relaxed);
            volume.fast_used = volume.fast_used.saturating_sub(4096);
        }
    }

    /// Update block access tracking (static)
    fn update_access_static(volume: &mut StoreMiVolume, lba: u64) {
        let now = crate::time::uptime_ms();

        if let Some(entry) = volume.block_map.iter_mut().find(|b| b.lba == lba) {
            entry.access_count += 1;
            entry.last_access = now;

            // Update temperature based on access pattern
            entry.temperature = match entry.access_count {
                0..=2 => DataTemperature::Cold,
                3..=5 => DataTemperature::Cool,
                6..=10 => DataTemperature::Warm,
                _ => DataTemperature::Hot,
            };
        } else {
            // New block
            volume.block_map.push(BlockEntry {
                lba,
                tier: StorageTier::Capacity,
                access_count: 1,
                last_access: now,
                temperature: DataTemperature::Cold,
                dirty: false,
            });
        }
    }

    /// Update block entry (static)
    fn update_block_entry_static(volume: &mut StoreMiVolume, lba: u64, tier: StorageTier, dirty: bool) {
        let now = crate::time::uptime_ms();

        if let Some(entry) = volume.block_map.iter_mut().find(|b| b.lba == lba) {
            entry.tier = tier;
            entry.dirty = dirty;
            entry.last_access = now;
        } else {
            volume.block_map.push(BlockEntry {
                lba,
                tier,
                access_count: 1,
                last_access: now,
                temperature: DataTemperature::Warm,
                dirty,
            });

            if tier == StorageTier::Fast {
                volume.fast_used += 4096;
            }
        }
    }

    /// Run background migration
    pub fn run_migration(&mut self, volume_id: u32) -> KResult<u32> {
        let volume = self.volumes.iter_mut().find(|v| v.id == volume_id)
            .ok_or(KError::NotFound)?;

        if !volume.config.background_migration {
            return Ok(0);
        }

        let now = crate::time::uptime_ms();
        let cold_threshold_ms = volume.config.cold_threshold * 1000;
        let mut migrated = 0u32;

        // Find cold blocks on fast tier to demote
        let cold_blocks: Vec<u64> = volume.block_map.iter()
            .filter(|b| b.tier == StorageTier::Fast
                && now.saturating_sub(b.last_access) > cold_threshold_ms
                && b.temperature <= DataTemperature::Cool)
            .take(volume.config.migration_batch as usize)
            .map(|b| b.lba)
            .collect();

        for lba in cold_blocks {
            Self::demote_block_static(volume, lba);
            migrated += 1;
        }

        Ok(migrated)
    }

    /// Get volume info
    pub fn get_volume(&self, id: u32) -> Option<&StoreMiVolume> {
        self.volumes.iter().find(|v| v.id == id)
    }

    /// List volumes
    pub fn list_volumes(&self) -> &[StoreMiVolume] {
        &self.volumes
    }

    /// Check if supported
    pub fn is_supported(&self) -> bool {
        self.supported
    }

    /// Flush all dirty blocks
    pub fn flush(&mut self, volume_id: u32) -> KResult<u32> {
        let volume = self.volumes.iter_mut().find(|v| v.id == volume_id)
            .ok_or(KError::NotFound)?;

        let dirty_count = volume.block_map.iter_mut()
            .filter(|b| b.dirty)
            .map(|b| { b.dirty = false; })
            .count();

        Ok(dirty_count as u32)
    }

    /// Get statistics for volume
    pub fn get_stats(&self, volume_id: u32) -> Option<&StoreMiStats> {
        self.volumes.iter()
            .find(|v| v.id == volume_id)
            .map(|v| &v.stats)
    }
}

/// Initialize AMD StoreMI
pub fn init() -> KResult<()> {
    STOREMI_MANAGER.lock().init()
}

/// Check if supported
pub fn is_supported() -> bool {
    STOREMI_MANAGER.lock().is_supported()
}

/// Create volume
pub fn create_volume(name: String, fast_device: String, capacity_device: String) -> KResult<u32> {
    STOREMI_MANAGER.lock().create_volume(name, fast_device, capacity_device)
}

/// Delete volume
pub fn delete_volume(id: u32) -> KResult<()> {
    STOREMI_MANAGER.lock().delete_volume(id)
}

/// Run migration
pub fn run_migration(volume_id: u32) -> KResult<u32> {
    STOREMI_MANAGER.lock().run_migration(volume_id)
}

/// Flush volume
pub fn flush(volume_id: u32) -> KResult<u32> {
    STOREMI_MANAGER.lock().flush(volume_id)
}
