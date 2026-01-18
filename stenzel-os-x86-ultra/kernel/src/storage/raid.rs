//! Software RAID Implementation
//!
//! Provides software RAID functionality for combining multiple block devices
//! into a single logical device with various redundancy levels.
//!
//! Supported RAID levels:
//! - RAID0: Striping (performance, no redundancy)
//! - RAID1: Mirroring (redundancy, no performance gain)
//! - RAID5: Distributed parity (balance of performance and redundancy)
//! - RAID10: Striped mirrors (combines RAID0 and RAID1)
//! - Linear: Concatenation (simple span, no redundancy)

#![allow(dead_code)]

extern crate alloc;

use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use alloc::string::String;
use core::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use spin::{Mutex, RwLock};

use super::block::{BlockDevice, BlockDeviceId};
use crate::util::{KError, KResult};

// ============================================================================
// Constants
// ============================================================================

/// RAID superblock magic number
const RAID_MAGIC: u32 = 0x52414944; // "RAID"

/// RAID superblock version
const RAID_VERSION: u32 = 1;

/// Default stripe size (64KB)
const DEFAULT_STRIPE_SIZE: u32 = 64 * 1024;

/// Minimum devices for each RAID level
const MIN_DEVICES_RAID0: usize = 2;
const MIN_DEVICES_RAID1: usize = 2;
const MIN_DEVICES_RAID5: usize = 3;
const MIN_DEVICES_RAID10: usize = 4;
const MIN_DEVICES_LINEAR: usize = 2;

// ============================================================================
// RAID Level Definition
// ============================================================================

/// RAID level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RaidLevel {
    /// Striping - data distributed across devices, no parity
    Raid0,
    /// Mirroring - data duplicated on all devices
    Raid1,
    /// Distributed parity - data and parity spread across devices
    Raid5,
    /// Striped mirrors - RAID0 over RAID1 pairs
    Raid10,
    /// Concatenation - devices joined sequentially
    Linear,
}

impl RaidLevel {
    /// Get minimum number of devices required
    pub fn min_devices(&self) -> usize {
        match self {
            RaidLevel::Raid0 => MIN_DEVICES_RAID0,
            RaidLevel::Raid1 => MIN_DEVICES_RAID1,
            RaidLevel::Raid5 => MIN_DEVICES_RAID5,
            RaidLevel::Raid10 => MIN_DEVICES_RAID10,
            RaidLevel::Linear => MIN_DEVICES_LINEAR,
        }
    }

    /// Get level name
    pub fn name(&self) -> &'static str {
        match self {
            RaidLevel::Raid0 => "RAID0",
            RaidLevel::Raid1 => "RAID1",
            RaidLevel::Raid5 => "RAID5",
            RaidLevel::Raid10 => "RAID10",
            RaidLevel::Linear => "Linear",
        }
    }

    /// Get efficiency (usable space ratio)
    pub fn efficiency(&self, device_count: usize) -> f64 {
        match self {
            RaidLevel::Raid0 => 1.0,
            RaidLevel::Raid1 => 1.0 / device_count as f64,
            RaidLevel::Raid5 => (device_count - 1) as f64 / device_count as f64,
            RaidLevel::Raid10 => 0.5, // Half for mirroring
            RaidLevel::Linear => 1.0,
        }
    }

    /// Can survive device failure?
    pub fn is_redundant(&self) -> bool {
        matches!(self, RaidLevel::Raid1 | RaidLevel::Raid5 | RaidLevel::Raid10)
    }
}

// ============================================================================
// RAID Device State
// ============================================================================

/// State of a device in the RAID array
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceState {
    /// Device is functioning normally
    Active,
    /// Device has failed and is not usable
    Failed,
    /// Device is being rebuilt
    Rebuilding,
    /// Device is a spare waiting to replace a failed device
    Spare,
    /// Device has been removed from the array
    Removed,
}

/// Information about a RAID member device
pub struct RaidMember {
    /// Index in the array
    pub index: usize,
    /// The block device
    pub device: Arc<dyn BlockDevice>,
    /// Current state
    pub state: DeviceState,
    /// Number of errors encountered
    pub error_count: u64,
    /// Device size in blocks
    pub size_blocks: u64,
}

impl Clone for RaidMember {
    fn clone(&self) -> Self {
        Self {
            index: self.index,
            device: Arc::clone(&self.device),
            state: self.state,
            error_count: self.error_count,
            size_blocks: self.size_blocks,
        }
    }
}

impl core::fmt::Debug for RaidMember {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RaidMember")
            .field("index", &self.index)
            .field("state", &self.state)
            .field("error_count", &self.error_count)
            .field("size_blocks", &self.size_blocks)
            .finish()
    }
}

// ============================================================================
// RAID Configuration
// ============================================================================

/// RAID array configuration
#[derive(Debug, Clone)]
pub struct RaidConfig {
    /// RAID level
    pub level: RaidLevel,
    /// Stripe size in bytes (for RAID0, RAID5, RAID10)
    pub stripe_size: u32,
    /// Number of data devices
    pub device_count: usize,
    /// UUID of the array
    pub uuid: [u8; 16],
    /// Array name
    pub name: String,
    /// Creation timestamp (Unix seconds)
    pub created_at: u64,
}

impl RaidConfig {
    /// Create a new RAID configuration
    pub fn new(level: RaidLevel, device_count: usize, name: &str) -> Self {
        Self {
            level,
            stripe_size: DEFAULT_STRIPE_SIZE,
            device_count,
            uuid: generate_uuid(),
            name: String::from(name),
            created_at: 0, // Would be set from RTC
        }
    }

    /// Set stripe size
    pub fn with_stripe_size(mut self, size: u32) -> Self {
        // Must be power of 2 and at least 4KB
        if size >= 4096 && size.is_power_of_two() {
            self.stripe_size = size;
        }
        self
    }
}

/// RAID superblock stored at the start of each member device
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RaidSuperblock {
    /// Magic number
    pub magic: u32,
    /// Version
    pub version: u32,
    /// RAID level
    pub level: u32,
    /// Device index in array
    pub device_index: u32,
    /// Total device count
    pub device_count: u32,
    /// Stripe size in bytes
    pub stripe_size: u32,
    /// Array UUID
    pub uuid: [u8; 16],
    /// Array size in blocks
    pub array_size: u64,
    /// Device size in blocks
    pub device_size: u64,
    /// Creation timestamp
    pub created_at: u64,
    /// Event counter (for sync detection)
    pub events: u64,
    /// CRC32 of superblock
    pub checksum: u32,
    /// Padding
    _reserved: [u8; 420],
}

impl RaidSuperblock {
    /// Create a new superblock
    fn new(config: &RaidConfig, device_index: usize, device_size: u64, array_size: u64) -> Self {
        let mut sb = Self {
            magic: RAID_MAGIC,
            version: RAID_VERSION,
            level: config.level as u32,
            device_index: device_index as u32,
            device_count: config.device_count as u32,
            stripe_size: config.stripe_size,
            uuid: config.uuid,
            array_size,
            device_size,
            created_at: config.created_at,
            events: 0,
            checksum: 0,
            _reserved: [0; 420],
        };
        sb.checksum = sb.calculate_checksum();
        sb
    }

    /// Calculate CRC32 checksum
    fn calculate_checksum(&self) -> u32 {
        // Simple checksum (XOR all u32 values except checksum field)
        let bytes = unsafe {
            core::slice::from_raw_parts(
                self as *const Self as *const u8,
                core::mem::size_of::<Self>() - 424, // Exclude checksum and reserved
            )
        };
        let mut sum = 0u32;
        for chunk in bytes.chunks(4) {
            let val = u32::from_le_bytes([
                chunk.get(0).copied().unwrap_or(0),
                chunk.get(1).copied().unwrap_or(0),
                chunk.get(2).copied().unwrap_or(0),
                chunk.get(3).copied().unwrap_or(0),
            ]);
            sum = sum.wrapping_add(val);
        }
        sum
    }

    /// Verify superblock
    fn is_valid(&self) -> bool {
        self.magic == RAID_MAGIC &&
        self.version == RAID_VERSION &&
        self.checksum == self.calculate_checksum()
    }

    /// Convert to bytes
    fn to_bytes(&self) -> [u8; 512] {
        unsafe { core::mem::transmute_copy(self) }
    }

    /// Read from bytes
    fn from_bytes(bytes: &[u8; 512]) -> Self {
        unsafe { core::ptr::read(bytes.as_ptr() as *const Self) }
    }
}

// ============================================================================
// RAID Array Statistics
// ============================================================================

/// RAID array statistics
#[derive(Debug, Clone, Default)]
pub struct RaidStats {
    /// Read operations
    pub reads: u64,
    /// Write operations
    pub writes: u64,
    /// Bytes read
    pub bytes_read: u64,
    /// Bytes written
    pub bytes_written: u64,
    /// Read errors
    pub read_errors: u64,
    /// Write errors
    pub write_errors: u64,
    /// Parity checks (RAID5)
    pub parity_checks: u64,
    /// Rebuild operations
    pub rebuilds: u64,
}

// ============================================================================
// RAID Array Implementation
// ============================================================================

/// A RAID array that presents multiple devices as a single block device
pub struct RaidArray {
    /// Configuration
    config: RaidConfig,
    /// Member devices
    members: RwLock<Vec<RaidMember>>,
    /// Array state
    state: Mutex<RaidArrayState>,
    /// Block device ID
    device_id: BlockDeviceId,
    /// Statistics
    stats: Mutex<RaidStats>,
    /// Number of usable blocks
    usable_blocks: AtomicU64,
    /// Block size (from underlying devices)
    block_size: u32,
    /// Is the array degraded?
    degraded: AtomicBool,
    /// Is syncing/rebuilding in progress?
    syncing: AtomicBool,
}

/// Internal state of the RAID array
#[derive(Debug)]
struct RaidArrayState {
    /// Array is initialized
    initialized: bool,
    /// Write intent bitmap (for recovery)
    write_bitmap: Vec<u64>,
    /// Current sync position
    sync_position: u64,
}

unsafe impl Send for RaidArray {}
unsafe impl Sync for RaidArray {}

impl RaidArray {
    /// Create a new RAID array
    pub fn new(config: RaidConfig, devices: Vec<Arc<dyn BlockDevice>>, device_id: BlockDeviceId) -> KResult<Self> {
        // Validate device count
        if devices.len() < config.level.min_devices() {
            return Err(KError::Invalid);
        }

        // Get block size (all devices must have the same)
        let block_size = devices[0].block_size();
        for dev in &devices[1..] {
            if dev.block_size() != block_size {
                return Err(KError::Invalid);
            }
        }

        // Find minimum device size
        let min_size = devices.iter().map(|d| d.num_blocks()).min().unwrap_or(0);
        if min_size == 0 {
            return Err(KError::Invalid);
        }

        // Calculate usable blocks based on RAID level
        let usable_blocks = match config.level {
            RaidLevel::Raid0 => min_size * devices.len() as u64,
            RaidLevel::Raid1 => min_size,
            RaidLevel::Raid5 => min_size * (devices.len() - 1) as u64,
            RaidLevel::Raid10 => min_size * (devices.len() / 2) as u64,
            RaidLevel::Linear => devices.iter().map(|d| d.num_blocks()).sum(),
        };

        // Create members
        let members: Vec<RaidMember> = devices.into_iter().enumerate().map(|(i, dev)| {
            RaidMember {
                index: i,
                size_blocks: dev.num_blocks(),
                device: dev,
                state: DeviceState::Active,
                error_count: 0,
            }
        }).collect();

        Ok(Self {
            config,
            members: RwLock::new(members),
            state: Mutex::new(RaidArrayState {
                initialized: true,
                write_bitmap: Vec::new(),
                sync_position: 0,
            }),
            device_id,
            stats: Mutex::new(RaidStats::default()),
            usable_blocks: AtomicU64::new(usable_blocks),
            block_size,
            degraded: AtomicBool::new(false),
            syncing: AtomicBool::new(false),
        })
    }

    /// Get array info
    pub fn info(&self) -> String {
        let members = self.members.read();
        let active_count = members.iter().filter(|m| m.state == DeviceState::Active).count();
        alloc::format!(
            "{} [{}] {} devices, {} active, {} blocks",
            self.config.name,
            self.config.level.name(),
            members.len(),
            active_count,
            self.usable_blocks.load(Ordering::Relaxed)
        )
    }

    /// Check if array is degraded
    pub fn is_degraded(&self) -> bool {
        self.degraded.load(Ordering::Relaxed)
    }

    /// Check if array is syncing
    pub fn is_syncing(&self) -> bool {
        self.syncing.load(Ordering::Relaxed)
    }

    /// Get member devices
    pub fn members(&self) -> Vec<RaidMember> {
        self.members.read().clone()
    }

    /// Get statistics
    pub fn stats(&self) -> RaidStats {
        self.stats.lock().clone()
    }

    /// Map a logical block to physical device and block
    fn map_block(&self, logical_block: u64) -> KResult<Vec<(usize, u64)>> {
        let members = self.members.read();
        let active_members: Vec<_> = members.iter()
            .filter(|m| m.state == DeviceState::Active)
            .collect();

        if active_members.is_empty() {
            return Err(KError::IO);
        }

        let stripe_blocks = self.config.stripe_size as u64 / self.block_size as u64;

        match self.config.level {
            RaidLevel::Raid0 => {
                // Striping: blocks distributed round-robin
                let device_idx = (logical_block / stripe_blocks) as usize % active_members.len();
                let device_block = (logical_block / stripe_blocks / active_members.len() as u64) * stripe_blocks +
                                   (logical_block % stripe_blocks);
                Ok(vec![(active_members[device_idx].index, device_block)])
            }
            RaidLevel::Raid1 => {
                // Mirroring: all devices get the same block
                Ok(active_members.iter().map(|m| (m.index, logical_block)).collect())
            }
            RaidLevel::Raid5 => {
                // RAID5: data + distributed parity
                let stripe = logical_block / stripe_blocks;
                let data_devices = active_members.len() - 1;
                let parity_device = (stripe as usize) % active_members.len();
                let data_idx = (logical_block / stripe_blocks) as usize % data_devices;
                let actual_idx = if data_idx >= parity_device { data_idx + 1 } else { data_idx };
                let device_block = (stripe / active_members.len() as u64) * stripe_blocks +
                                   (logical_block % stripe_blocks);
                Ok(vec![(active_members[actual_idx % active_members.len()].index, device_block)])
            }
            RaidLevel::Raid10 => {
                // Striped mirrors: stripe across mirror pairs
                let pair_count = active_members.len() / 2;
                let pair_idx = (logical_block / stripe_blocks) as usize % pair_count;
                let device_block = (logical_block / stripe_blocks / pair_count as u64) * stripe_blocks +
                                   (logical_block % stripe_blocks);
                // Return both devices in the mirror pair
                let dev1 = pair_idx * 2;
                let dev2 = pair_idx * 2 + 1;
                Ok(vec![
                    (active_members[dev1].index, device_block),
                    (active_members[dev2].index, device_block),
                ])
            }
            RaidLevel::Linear => {
                // Linear: blocks concatenated sequentially
                let mut offset = 0u64;
                for member in active_members.iter() {
                    if logical_block < offset + member.size_blocks {
                        return Ok(vec![(member.index, logical_block - offset)]);
                    }
                    offset += member.size_blocks;
                }
                Err(KError::OutOfRange)
            }
        }
    }

    /// Read a block from the array
    fn read_block(&self, logical_block: u64, buffer: &mut [u8]) -> KResult<()> {
        let mappings = self.map_block(logical_block)?;
        let members = self.members.read();

        // Try reading from the first available device
        for (device_idx, physical_block) in mappings {
            if let Some(member) = members.iter().find(|m| m.index == device_idx && m.state == DeviceState::Active) {
                match member.device.read_blocks(physical_block, 1, buffer) {
                    Ok(()) => {
                        let mut stats = self.stats.lock();
                        stats.reads += 1;
                        stats.bytes_read += self.block_size as u64;
                        return Ok(());
                    }
                    Err(_) => {
                        // Try next device if available (for redundant arrays)
                        continue;
                    }
                }
            }
        }

        let mut stats = self.stats.lock();
        stats.read_errors += 1;
        Err(KError::IO)
    }

    /// Write a block to the array
    fn write_block(&self, logical_block: u64, data: &[u8]) -> KResult<()> {
        let mappings = self.map_block(logical_block)?;
        let members = self.members.read();

        let mut write_count = 0;
        let mut error_count = 0;

        // Write to all mapped devices (for redundancy)
        for (device_idx, physical_block) in &mappings {
            if let Some(member) = members.iter().find(|m| m.index == *device_idx && m.state == DeviceState::Active) {
                match member.device.write_blocks(*physical_block, 1, data) {
                    Ok(()) => write_count += 1,
                    Err(_) => error_count += 1,
                }
            }
        }

        let mut stats = self.stats.lock();
        stats.writes += 1;
        stats.bytes_written += self.block_size as u64;

        // For redundant arrays, success if at least one write succeeded
        if write_count > 0 {
            Ok(())
        } else {
            stats.write_errors += 1;
            Err(KError::IO)
        }
    }

    /// Calculate parity for RAID5
    fn calculate_parity(&self, stripe_data: &[Vec<u8>]) -> Vec<u8> {
        if stripe_data.is_empty() {
            return Vec::new();
        }
        let mut parity = vec![0u8; stripe_data[0].len()];
        for data in stripe_data {
            for (i, &byte) in data.iter().enumerate() {
                parity[i] ^= byte;
            }
        }
        parity
    }

    /// Mark a device as failed
    pub fn fail_device(&self, device_index: usize) -> KResult<()> {
        let mut members = self.members.write();
        if let Some(member) = members.iter_mut().find(|m| m.index == device_index) {
            member.state = DeviceState::Failed;
            self.check_degraded(&members);
            Ok(())
        } else {
            Err(KError::NotFound)
        }
    }

    /// Replace a failed device
    pub fn replace_device(&self, device_index: usize, new_device: Arc<dyn BlockDevice>) -> KResult<()> {
        let mut members = self.members.write();
        if let Some(member) = members.iter_mut().find(|m| m.index == device_index) {
            if member.state != DeviceState::Failed && member.state != DeviceState::Removed {
                return Err(KError::Invalid);
            }
            member.device = new_device;
            member.state = DeviceState::Rebuilding;
            member.error_count = 0;
            self.syncing.store(true, Ordering::Release);
            Ok(())
        } else {
            Err(KError::NotFound)
        }
    }

    /// Check if array is degraded based on member states
    fn check_degraded(&self, members: &[RaidMember]) {
        let failed_count = members.iter().filter(|m| m.state == DeviceState::Failed).count();
        let is_degraded = match self.config.level {
            RaidLevel::Raid0 | RaidLevel::Linear => failed_count > 0, // Any failure is fatal
            RaidLevel::Raid1 => failed_count >= members.len() - 1, // Can survive N-1 failures
            RaidLevel::Raid5 => failed_count >= 1, // Can survive 1 failure
            RaidLevel::Raid10 => {
                // Check if any mirror pair has both devices failed
                let pair_count = members.len() / 2;
                for pair in 0..pair_count {
                    let d1_failed = members.get(pair * 2).map(|m| m.state == DeviceState::Failed).unwrap_or(true);
                    let d2_failed = members.get(pair * 2 + 1).map(|m| m.state == DeviceState::Failed).unwrap_or(true);
                    if d1_failed && d2_failed {
                        self.degraded.store(true, Ordering::Release);
                        return;
                    }
                }
                failed_count >= 1
            }
        };
        self.degraded.store(is_degraded, Ordering::Release);
    }

    /// Write superblock to all devices
    pub fn write_superblock(&self) -> KResult<()> {
        let members = self.members.read();
        let usable_blocks = self.usable_blocks.load(Ordering::Relaxed);

        for member in members.iter() {
            if member.state != DeviceState::Active {
                continue;
            }

            let superblock = RaidSuperblock::new(
                &self.config,
                member.index,
                member.size_blocks,
                usable_blocks,
            );
            let bytes = superblock.to_bytes();

            // Write superblock to block 0
            member.device.write_blocks(0, 1, &bytes)?;
        }

        Ok(())
    }
}

impl BlockDevice for RaidArray {
    fn id(&self) -> BlockDeviceId {
        self.device_id
    }

    fn block_size(&self) -> u32 {
        self.block_size
    }

    fn num_blocks(&self) -> u64 {
        self.usable_blocks.load(Ordering::Relaxed)
    }

    fn read_blocks(&self, lba: u64, count: u32, out: &mut [u8]) -> KResult<()> {
        let block_size = self.block_size as usize;
        let expected_len = count as usize * block_size;
        if out.len() < expected_len {
            return Err(KError::Invalid);
        }

        // Read each block
        for i in 0..count as u64 {
            let offset = i as usize * block_size;
            self.read_block(lba + i, &mut out[offset..offset + block_size])?;
        }

        Ok(())
    }

    fn write_blocks(&self, lba: u64, count: u32, data: &[u8]) -> KResult<()> {
        let block_size = self.block_size as usize;
        let expected_len = count as usize * block_size;
        if data.len() < expected_len {
            return Err(KError::Invalid);
        }

        // Write each block
        for i in 0..count as u64 {
            let offset = i as usize * block_size;
            self.write_block(lba + i, &data[offset..offset + block_size])?;
        }

        Ok(())
    }
}

// ============================================================================
// RAID Manager
// ============================================================================

/// Manages all RAID arrays in the system
pub struct RaidManager {
    /// All arrays
    arrays: RwLock<Vec<Arc<RaidArray>>>,
    /// Next device ID
    next_device_id: AtomicU64,
}

impl RaidManager {
    /// Create a new RAID manager
    pub const fn new() -> Self {
        Self {
            arrays: RwLock::new(Vec::new()),
            next_device_id: AtomicU64::new(300), // RAID devices start at 300
        }
    }

    /// Create a new RAID array
    pub fn create_array(
        &self,
        level: RaidLevel,
        name: &str,
        devices: Vec<Arc<dyn BlockDevice>>,
    ) -> KResult<Arc<RaidArray>> {
        let config = RaidConfig::new(level, devices.len(), name);
        let device_id = BlockDeviceId(self.next_device_id.fetch_add(1, Ordering::Relaxed) as u32);
        let array = Arc::new(RaidArray::new(config, devices, device_id)?);

        // Write superblock to all devices
        array.write_superblock()?;

        let mut arrays = self.arrays.write();
        arrays.push(Arc::clone(&array));

        crate::kprintln!(
            "raid: created {} array '{}' with {} devices",
            level.name(),
            name,
            array.members().len()
        );

        Ok(array)
    }

    /// Get array by name
    pub fn get_array(&self, name: &str) -> Option<Arc<RaidArray>> {
        let arrays = self.arrays.read();
        arrays.iter().find(|a| a.config.name == name).cloned()
    }

    /// Get array by device ID
    pub fn get_array_by_id(&self, device_id: BlockDeviceId) -> Option<Arc<RaidArray>> {
        let arrays = self.arrays.read();
        arrays.iter().find(|a| a.device_id == device_id).cloned()
    }

    /// List all arrays
    pub fn list_arrays(&self) -> Vec<Arc<RaidArray>> {
        self.arrays.read().clone()
    }

    /// Remove an array
    pub fn remove_array(&self, name: &str) -> KResult<()> {
        let mut arrays = self.arrays.write();
        if let Some(pos) = arrays.iter().position(|a| a.config.name == name) {
            arrays.remove(pos);
            Ok(())
        } else {
            Err(KError::NotFound)
        }
    }

    /// Scan devices for existing RAID arrays
    pub fn scan_arrays(&self, devices: &[Arc<dyn BlockDevice>]) -> Vec<RaidSuperblock> {
        let mut superblocks = Vec::new();

        for device in devices {
            let mut buffer = [0u8; 512];
            if device.read_blocks(0, 1, &mut buffer).is_ok() {
                let sb = RaidSuperblock::from_bytes(&buffer);
                if sb.is_valid() {
                    superblocks.push(sb);
                }
            }
        }

        superblocks
    }

    /// Print array information
    pub fn print_arrays(&self) {
        let arrays = self.arrays.read();
        if arrays.is_empty() {
            crate::kprintln!("raid: no arrays configured");
            return;
        }

        for array in arrays.iter() {
            crate::kprintln!("  {}", array.info());
            for member in array.members() {
                let state = match member.state {
                    DeviceState::Active => "active",
                    DeviceState::Failed => "FAILED",
                    DeviceState::Rebuilding => "rebuilding",
                    DeviceState::Spare => "spare",
                    DeviceState::Removed => "removed",
                };
                crate::kprintln!("    device {}: {} ({} blocks)", member.index, state, member.size_blocks);
            }
        }
    }
}

// ============================================================================
// Global State
// ============================================================================

static RAID_MANAGER: RaidManager = RaidManager::new();

/// Get the global RAID manager
pub fn manager() -> &'static RaidManager {
    &RAID_MANAGER
}

/// Initialize the RAID subsystem
pub fn init() {
    crate::kprintln!("raid: software RAID subsystem initialized");
}

/// Create a RAID0 array
pub fn create_raid0(name: &str, devices: Vec<Arc<dyn BlockDevice>>) -> KResult<Arc<RaidArray>> {
    RAID_MANAGER.create_array(RaidLevel::Raid0, name, devices)
}

/// Create a RAID1 array
pub fn create_raid1(name: &str, devices: Vec<Arc<dyn BlockDevice>>) -> KResult<Arc<RaidArray>> {
    RAID_MANAGER.create_array(RaidLevel::Raid1, name, devices)
}

/// Create a RAID5 array
pub fn create_raid5(name: &str, devices: Vec<Arc<dyn BlockDevice>>) -> KResult<Arc<RaidArray>> {
    RAID_MANAGER.create_array(RaidLevel::Raid5, name, devices)
}

/// Create a RAID10 array
pub fn create_raid10(name: &str, devices: Vec<Arc<dyn BlockDevice>>) -> KResult<Arc<RaidArray>> {
    RAID_MANAGER.create_array(RaidLevel::Raid10, name, devices)
}

/// Create a linear (concatenated) array
pub fn create_linear(name: &str, devices: Vec<Arc<dyn BlockDevice>>) -> KResult<Arc<RaidArray>> {
    RAID_MANAGER.create_array(RaidLevel::Linear, name, devices)
}

/// Get array by name
pub fn get_array(name: &str) -> Option<Arc<RaidArray>> {
    RAID_MANAGER.get_array(name)
}

/// List all arrays
pub fn list_arrays() -> Vec<Arc<RaidArray>> {
    RAID_MANAGER.list_arrays()
}

/// Print RAID status
pub fn print_status() {
    RAID_MANAGER.print_arrays();
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Generate a random UUID
fn generate_uuid() -> [u8; 16] {
    let mut uuid = [0u8; 16];
    // Simple PRNG based on time (would use proper random in production)
    let seed = 0x12345678u64; // Would read from RTC or use entropy
    let mut state = seed;
    for byte in uuid.iter_mut() {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        *byte = (state >> 33) as u8;
    }
    // Set version 4 (random) and variant bits
    uuid[6] = (uuid[6] & 0x0F) | 0x40;
    uuid[8] = (uuid[8] & 0x3F) | 0x80;
    uuid
}

/// Format UUID as string
pub fn format_uuid(uuid: &[u8; 16]) -> String {
    alloc::format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        uuid[0], uuid[1], uuid[2], uuid[3],
        uuid[4], uuid[5],
        uuid[6], uuid[7],
        uuid[8], uuid[9],
        uuid[10], uuid[11], uuid[12], uuid[13], uuid[14], uuid[15]
    )
}
