//! VirtIO Block Device Driver
//!
//! Provides block device access via VirtIO protocol.

#![allow(dead_code)]

use alloc::vec::Vec;
use alloc::vec;
use alloc::string::String;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use super::virtqueue::Virtqueue;
use super::{VirtioDevice, VirtioDeviceType, features};

/// Block device feature flags
pub mod block_features {
    pub const VIRTIO_BLK_F_SIZE_MAX: u64 = 1 << 1;
    pub const VIRTIO_BLK_F_SEG_MAX: u64 = 1 << 2;
    pub const VIRTIO_BLK_F_GEOMETRY: u64 = 1 << 4;
    pub const VIRTIO_BLK_F_RO: u64 = 1 << 5;
    pub const VIRTIO_BLK_F_BLK_SIZE: u64 = 1 << 6;
    pub const VIRTIO_BLK_F_FLUSH: u64 = 1 << 9;
    pub const VIRTIO_BLK_F_TOPOLOGY: u64 = 1 << 10;
    pub const VIRTIO_BLK_F_CONFIG_WCE: u64 = 1 << 11;
    pub const VIRTIO_BLK_F_MQ: u64 = 1 << 12;
    pub const VIRTIO_BLK_F_DISCARD: u64 = 1 << 13;
    pub const VIRTIO_BLK_F_WRITE_ZEROES: u64 = 1 << 14;
}

/// Block request types
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockRequestType {
    Read = 0,
    Write = 1,
    Flush = 4,
    Discard = 11,
    WriteZeroes = 13,
}

/// Block request header
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VirtioBlockHeader {
    pub request_type: u32,
    pub reserved: u32,
    pub sector: u64,
}

/// Block request status
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockStatus {
    Ok = 0,
    IoError = 1,
    Unsupported = 2,
}

/// Block device configuration
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct VirtioBlockConfig {
    /// Total capacity in 512-byte sectors
    pub capacity: u64,
    /// Maximum segment size
    pub size_max: u32,
    /// Maximum number of segments
    pub seg_max: u32,
    /// Geometry
    pub geometry: VirtioBlockGeometry,
    /// Block size
    pub blk_size: u32,
    /// Topology
    pub topology: VirtioBlockTopology,
    /// Writeback mode
    pub writeback: u8,
    pub _padding0: [u8; 3],
    /// Number of queues
    pub num_queues: u16,
    pub _padding1: [u8; 2],
}

/// Block device geometry
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct VirtioBlockGeometry {
    pub cylinders: u16,
    pub heads: u8,
    pub sectors: u8,
}

/// Block device topology
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct VirtioBlockTopology {
    pub physical_block_exp: u8,
    pub alignment_offset: u8,
    pub min_io_size: u16,
    pub opt_io_size: u32,
}

/// Pending block request
#[derive(Debug)]
struct PendingRequest {
    descriptor_id: u16,
    request_type: BlockRequestType,
    sector: u64,
    data_len: usize,
}

/// VirtIO block device
pub struct VirtioBlockDevice {
    /// Device configuration
    config: VirtioBlockConfig,
    /// Request queue
    queue: Virtqueue,
    /// Negotiated features
    features: u64,
    /// Device is initialized
    initialized: AtomicBool,
    /// Read-only flag
    read_only: bool,
    /// Pending requests
    pending: Vec<PendingRequest>,
    /// Statistics
    stats: BlockStats,
}

/// Block device statistics
#[derive(Debug, Default)]
pub struct BlockStats {
    pub reads: AtomicU64,
    pub writes: AtomicU64,
    pub flushes: AtomicU64,
    pub bytes_read: AtomicU64,
    pub bytes_written: AtomicU64,
    pub errors: AtomicU64,
}

impl VirtioBlockDevice {
    /// Create new block device
    pub fn new(queue_size: u16) -> Self {
        Self {
            config: VirtioBlockConfig::default(),
            queue: Virtqueue::new(0, queue_size),
            features: 0,
            initialized: AtomicBool::new(false),
            read_only: false,
            pending: Vec::new(),
            stats: BlockStats::default(),
        }
    }

    /// Get device capacity in bytes
    pub fn capacity(&self) -> u64 {
        self.config.capacity * 512
    }

    /// Get block size
    pub fn block_size(&self) -> u32 {
        if self.config.blk_size > 0 {
            self.config.blk_size
        } else {
            512
        }
    }

    /// Check if device is read-only
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// Read sectors
    pub fn read(&mut self, sector: u64, buffer: &mut [u8]) -> Result<(), BlockStatus> {
        if !self.initialized.load(Ordering::Acquire) {
            return Err(BlockStatus::IoError);
        }

        let sector_count = buffer.len() / 512;
        if sector + sector_count as u64 > self.config.capacity {
            return Err(BlockStatus::IoError);
        }

        // Create request header
        let header = VirtioBlockHeader {
            request_type: BlockRequestType::Read as u32,
            reserved: 0,
            sector,
        };

        // In real implementation, we would:
        // 1. Allocate DMA buffer
        // 2. Set up scatter-gather list with header, data buffer, status
        // 3. Add to virtqueue
        // 4. Notify device
        // 5. Wait for completion

        self.stats.reads.fetch_add(1, Ordering::Relaxed);
        self.stats.bytes_read.fetch_add(buffer.len() as u64, Ordering::Relaxed);

        // Placeholder - would actually submit request to virtqueue
        let _ = header;
        Ok(())
    }

    /// Write sectors
    pub fn write(&mut self, sector: u64, buffer: &[u8]) -> Result<(), BlockStatus> {
        if !self.initialized.load(Ordering::Acquire) {
            return Err(BlockStatus::IoError);
        }

        if self.read_only {
            return Err(BlockStatus::Unsupported);
        }

        let sector_count = buffer.len() / 512;
        if sector + sector_count as u64 > self.config.capacity {
            return Err(BlockStatus::IoError);
        }

        let header = VirtioBlockHeader {
            request_type: BlockRequestType::Write as u32,
            reserved: 0,
            sector,
        };

        self.stats.writes.fetch_add(1, Ordering::Relaxed);
        self.stats.bytes_written.fetch_add(buffer.len() as u64, Ordering::Relaxed);

        let _ = header;
        Ok(())
    }

    /// Flush data to disk
    pub fn flush(&mut self) -> Result<(), BlockStatus> {
        if !self.initialized.load(Ordering::Acquire) {
            return Err(BlockStatus::IoError);
        }

        if self.features & block_features::VIRTIO_BLK_F_FLUSH == 0 {
            return Ok(()); // Flush not supported, treat as success
        }

        let header = VirtioBlockHeader {
            request_type: BlockRequestType::Flush as u32,
            reserved: 0,
            sector: 0,
        };

        self.stats.flushes.fetch_add(1, Ordering::Relaxed);

        let _ = header;
        Ok(())
    }

    /// Discard sectors
    pub fn discard(&mut self, sector: u64, num_sectors: u64) -> Result<(), BlockStatus> {
        if self.features & block_features::VIRTIO_BLK_F_DISCARD == 0 {
            return Err(BlockStatus::Unsupported);
        }

        if sector + num_sectors > self.config.capacity {
            return Err(BlockStatus::IoError);
        }

        let header = VirtioBlockHeader {
            request_type: BlockRequestType::Discard as u32,
            reserved: 0,
            sector,
        };

        let _ = header;
        let _ = num_sectors;
        Ok(())
    }

    /// Process completed requests
    pub fn process_completions(&mut self) {
        while let Some((desc_id, _len)) = self.queue.get_used() {
            // Find and remove pending request
            if let Some(pos) = self.pending.iter().position(|r| r.descriptor_id == desc_id) {
                self.pending.remove(pos);
            }
        }
    }

    /// Get statistics
    pub fn stats(&self) -> &BlockStats {
        &self.stats
    }

    /// Format status
    pub fn format_status(&self) -> String {
        let capacity_mb = self.capacity() / (1024 * 1024);
        alloc::format!(
            "VirtIO Block: {}MB {} blk_size={}",
            capacity_mb,
            if self.read_only { "(ro)" } else { "(rw)" },
            self.block_size()
        )
    }
}

impl VirtioDevice for VirtioBlockDevice {
    fn device_type(&self) -> VirtioDeviceType {
        VirtioDeviceType::Block
    }

    fn init(&mut self) -> Result<(), &'static str> {
        // Read device configuration
        // In real implementation, read from MMIO/PCI config space
        self.config.capacity = 2 * 1024 * 1024; // 1GB default (in 512-byte sectors)
        self.config.blk_size = 512;
        Ok(())
    }

    fn reset(&mut self) {
        self.initialized.store(false, Ordering::Release);
        self.pending.clear();
        self.queue = Virtqueue::new(0, self.queue.size);
    }

    fn negotiate_features(&mut self, offered: u64) -> u64 {
        let mut wanted = features::VIRTIO_F_VERSION_1;

        // Request block-specific features
        if offered & block_features::VIRTIO_BLK_F_BLK_SIZE != 0 {
            wanted |= block_features::VIRTIO_BLK_F_BLK_SIZE;
        }
        if offered & block_features::VIRTIO_BLK_F_FLUSH != 0 {
            wanted |= block_features::VIRTIO_BLK_F_FLUSH;
        }
        if offered & block_features::VIRTIO_BLK_F_DISCARD != 0 {
            wanted |= block_features::VIRTIO_BLK_F_DISCARD;
        }

        // Check read-only
        if offered & block_features::VIRTIO_BLK_F_RO != 0 {
            self.read_only = true;
        }

        self.features = wanted & offered;
        self.features
    }

    fn activate(&mut self) -> Result<(), &'static str> {
        self.initialized.store(true, Ordering::Release);
        crate::kprintln!("virtio-blk: Activated, capacity={}MB", self.capacity() / (1024 * 1024));
        Ok(())
    }

    fn handle_interrupt(&mut self) {
        self.process_completions();
    }
}

/// Block device manager
pub struct VirtioBlockManager {
    devices: Vec<VirtioBlockDevice>,
}

impl VirtioBlockManager {
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
        }
    }

    /// Add device
    pub fn add_device(&mut self, device: VirtioBlockDevice) -> usize {
        let idx = self.devices.len();
        self.devices.push(device);
        idx
    }

    /// Get device
    pub fn get_device(&mut self, idx: usize) -> Option<&mut VirtioBlockDevice> {
        self.devices.get_mut(idx)
    }

    /// Device count
    pub fn device_count(&self) -> usize {
        self.devices.len()
    }
}

impl Default for VirtioBlockManager {
    fn default() -> Self {
        Self::new()
    }
}
