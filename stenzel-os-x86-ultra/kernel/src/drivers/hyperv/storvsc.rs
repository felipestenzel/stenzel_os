//! Hyper-V StorVSC (Storage Virtual Service Client)
//!
//! Synthetic SCSI storage controller for Hyper-V guests.

#![allow(dead_code)]

use alloc::string::{String, ToString};
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use super::vmbus::VmbusChannel;

/// StorVSC protocol versions
pub const STORVSC_WIN6: u16 = 0x0001;
pub const STORVSC_WIN7: u16 = 0x0002;
pub const STORVSC_WIN8: u16 = 0x0003;
pub const STORVSC_WIN8_1: u16 = 0x0004;
pub const STORVSC_WIN10: u16 = 0x0005;

/// VStor operation codes
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VstorOperation {
    CompleteIo = 1,
    RemoveDevice = 2,
    ExecuteScsiCmd = 3,
    ResetLun = 4,
    ResetAdapter = 5,
    ResetBus = 6,
    BeginInitialization = 7,
    EndInitialization = 8,
    QueryProtocolVersion = 9,
    QueryProperties = 10,
    EnumerateBus = 11,
    FchmCreateSubChannels = 12,
    FchmCreateSubChannelsComplete = 13,
}

/// SCSI commands
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScsiCommand {
    TestUnitReady = 0x00,
    RequestSense = 0x03,
    Inquiry = 0x12,
    ModeSense6 = 0x1A,
    StartStopUnit = 0x1B,
    ReadCapacity10 = 0x25,
    Read10 = 0x28,
    Write10 = 0x2A,
    SynchronizeCache10 = 0x35,
    ReadCapacity16 = 0x9E,
    Read16 = 0x88,
    Write16 = 0x8A,
}

/// VStor packet header
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct VstorPacketHeader {
    pub operation: u8,
    pub flags: u8,
    pub status: u16,
}

/// SCSI request
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ScsiRequest {
    pub target_id: u8,
    pub path_id: u8,
    pub lun: u8,
    pub reserved: u8,
    pub cdb_length: u8,
    pub sense_length: u8,
    pub data_in: u8,
    pub reserved2: u8,
    pub data_transfer_length: u32,
    pub cdb: [u8; 16],
    pub sense_buffer: [u8; 20],
}

/// Query properties request
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct VstorQueryProperties {
    pub header: VstorPacketHeader,
    pub path_id: u8,
    pub target_id: u8,
    pub lun: u8,
    pub reserved: u8,
}

/// Properties response
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct VstorPropertiesResponse {
    pub max_target: u8,
    pub max_lun: u8,
    pub max_channels: u16,
    pub unique_id: u64,
}

/// Disk information
#[derive(Debug, Clone)]
pub struct DiskInfo {
    pub target_id: u8,
    pub lun: u8,
    pub vendor: String,
    pub product: String,
    pub revision: String,
    pub sector_size: u32,
    pub sector_count: u64,
    pub removable: bool,
}

/// StorVSC statistics
#[derive(Debug, Default)]
pub struct StorvscStats {
    pub read_requests: AtomicU64,
    pub write_requests: AtomicU64,
    pub bytes_read: AtomicU64,
    pub bytes_written: AtomicU64,
    pub errors: AtomicU64,
}

/// StorVSC device
pub struct StorvscDevice {
    /// VMBus channel ID
    channel_id: u32,
    /// Protocol version
    protocol_version: u16,
    /// Max targets
    max_targets: u8,
    /// Max LUNs per target
    max_luns: u8,
    /// Discovered disks
    disks: Vec<DiskInfo>,
    /// Request ID counter
    request_id: u64,
    /// Initialized flag
    initialized: AtomicBool,
    /// Statistics
    stats: StorvscStats,
    /// Data buffer
    data_buffer: Vec<u8>,
}

impl StorvscDevice {
    /// Default sector size
    pub const DEFAULT_SECTOR_SIZE: u32 = 512;
    /// Max transfer size
    pub const MAX_TRANSFER_SIZE: usize = 256 * 1024; // 256 KB

    /// Create new device
    pub fn new(channel_id: u32) -> Self {
        Self {
            channel_id,
            protocol_version: 0,
            max_targets: 0,
            max_luns: 0,
            disks: Vec::new(),
            request_id: 0,
            initialized: AtomicBool::new(false),
            stats: StorvscStats::default(),
            data_buffer: vec![0u8; Self::MAX_TRANSFER_SIZE],
        }
    }

    /// Get next request ID
    fn next_request_id(&mut self) -> u64 {
        self.request_id += 1;
        self.request_id
    }

    /// Initialize device
    pub fn init(&mut self, channel: &mut VmbusChannel) -> Result<(), &'static str> {
        // Open channel if not already open
        if !channel.is_open() {
            channel.open()?;
        }

        // Begin initialization
        self.send_begin_init(channel)?;

        // Query protocol version
        self.negotiate_version(channel)?;

        // Query properties
        self.query_properties(channel)?;

        // End initialization
        self.send_end_init(channel)?;

        // Enumerate disks
        self.enumerate_disks(channel)?;

        self.initialized.store(true, Ordering::Release);
        crate::kprintln!("storvsc: Initialized, {} disks found", self.disks.len());

        Ok(())
    }

    /// Send begin initialization
    fn send_begin_init(&mut self, channel: &mut VmbusChannel) -> Result<(), &'static str> {
        let header = VstorPacketHeader {
            operation: VstorOperation::BeginInitialization as u8,
            flags: 0,
            status: 0,
        };

        let bytes = unsafe {
            core::slice::from_raw_parts(
                &header as *const _ as *const u8,
                core::mem::size_of::<VstorPacketHeader>()
            )
        };

        channel.write(bytes)
    }

    /// Send end initialization
    fn send_end_init(&mut self, channel: &mut VmbusChannel) -> Result<(), &'static str> {
        let header = VstorPacketHeader {
            operation: VstorOperation::EndInitialization as u8,
            flags: 0,
            status: 0,
        };

        let bytes = unsafe {
            core::slice::from_raw_parts(
                &header as *const _ as *const u8,
                core::mem::size_of::<VstorPacketHeader>()
            )
        };

        channel.write(bytes)
    }

    /// Negotiate protocol version
    fn negotiate_version(&mut self, _channel: &mut VmbusChannel) -> Result<(), &'static str> {
        // In real implementation, send QueryProtocolVersion and wait for response
        self.protocol_version = STORVSC_WIN10;
        Ok(())
    }

    /// Query storage properties
    fn query_properties(&mut self, _channel: &mut VmbusChannel) -> Result<(), &'static str> {
        // In real implementation, send QueryProperties and wait for response
        self.max_targets = 2;
        self.max_luns = 64;
        Ok(())
    }

    /// Enumerate disks
    fn enumerate_disks(&mut self, channel: &mut VmbusChannel) -> Result<(), &'static str> {
        for target in 0..self.max_targets {
            for lun in 0..2 { // Check first 2 LUNs per target
                if let Some(disk) = self.probe_disk(channel, target, lun) {
                    self.disks.push(disk);
                }
            }
        }
        Ok(())
    }

    /// Probe for disk at target/lun
    fn probe_disk(&mut self, channel: &mut VmbusChannel, target: u8, lun: u8) -> Option<DiskInfo> {
        // Send INQUIRY
        if let Ok(inquiry_data) = self.scsi_inquiry(channel, target, lun) {
            let vendor = String::from_utf8_lossy(&inquiry_data[8..16]).trim().to_string();
            let product = String::from_utf8_lossy(&inquiry_data[16..32]).trim().to_string();
            let revision = String::from_utf8_lossy(&inquiry_data[32..36]).trim().to_string();
            let removable = inquiry_data[1] & 0x80 != 0;

            // Get capacity
            let (sector_size, sector_count) = self.scsi_read_capacity(channel, target, lun)
                .unwrap_or((Self::DEFAULT_SECTOR_SIZE, 0));

            if sector_count > 0 {
                return Some(DiskInfo {
                    target_id: target,
                    lun,
                    vendor,
                    product,
                    revision,
                    sector_size,
                    sector_count,
                    removable,
                });
            }
        }
        None
    }

    /// Send SCSI INQUIRY command
    fn scsi_inquiry(&mut self, channel: &mut VmbusChannel, target: u8, lun: u8) -> Result<Vec<u8>, &'static str> {
        let mut request = ScsiRequest::default();
        request.target_id = target;
        request.lun = lun;
        request.cdb_length = 6;
        request.data_in = 1; // Data in
        request.data_transfer_length = 96;
        request.cdb[0] = ScsiCommand::Inquiry as u8;
        request.cdb[4] = 96; // Allocation length

        let response = self.send_scsi_command(channel, &request)?;
        Ok(response)
    }

    /// Send SCSI READ CAPACITY command
    fn scsi_read_capacity(&mut self, channel: &mut VmbusChannel, target: u8, lun: u8) -> Result<(u32, u64), &'static str> {
        let mut request = ScsiRequest::default();
        request.target_id = target;
        request.lun = lun;
        request.cdb_length = 10;
        request.data_in = 1;
        request.data_transfer_length = 8;
        request.cdb[0] = ScsiCommand::ReadCapacity10 as u8;

        let response = self.send_scsi_command(channel, &request)?;

        if response.len() >= 8 {
            let lba = u32::from_be_bytes([response[0], response[1], response[2], response[3]]);
            let block_size = u32::from_be_bytes([response[4], response[5], response[6], response[7]]);
            Ok((block_size, (lba as u64) + 1))
        } else {
            Ok((Self::DEFAULT_SECTOR_SIZE, 0))
        }
    }

    /// Send SCSI command
    fn send_scsi_command(&mut self, channel: &mut VmbusChannel, request: &ScsiRequest) -> Result<Vec<u8>, &'static str> {
        let header = VstorPacketHeader {
            operation: VstorOperation::ExecuteScsiCmd as u8,
            flags: 0,
            status: 0,
        };

        // Build packet
        let header_bytes = unsafe {
            core::slice::from_raw_parts(
                &header as *const _ as *const u8,
                core::mem::size_of::<VstorPacketHeader>()
            )
        };

        let request_bytes = unsafe {
            core::slice::from_raw_parts(
                request as *const _ as *const u8,
                core::mem::size_of::<ScsiRequest>()
            )
        };

        // Combine header and request
        let mut packet = Vec::new();
        packet.extend_from_slice(header_bytes);
        packet.extend_from_slice(request_bytes);

        channel.write(&packet)?;

        // Wait for response (simulated)
        let mut response = vec![0u8; request.data_transfer_length as usize];

        // In real implementation, read from channel
        let _ = channel.read(&mut response);

        Ok(response)
    }

    /// Read sectors
    pub fn read_sectors(&mut self, channel: &mut VmbusChannel, disk_idx: usize, lba: u64, count: u32, buffer: &mut [u8]) -> Result<(), &'static str> {
        if !self.initialized.load(Ordering::Acquire) {
            return Err("Device not initialized");
        }

        let disk = self.disks.get(disk_idx).ok_or("Invalid disk index")?;
        let bytes_to_read = (count * disk.sector_size) as usize;

        if buffer.len() < bytes_to_read {
            return Err("Buffer too small");
        }

        if lba + count as u64 > disk.sector_count {
            return Err("Read beyond disk end");
        }

        // Build READ command
        let mut request = ScsiRequest::default();
        request.target_id = disk.target_id;
        request.lun = disk.lun;
        request.data_in = 1;
        request.data_transfer_length = bytes_to_read as u32;

        if lba <= u32::MAX as u64 && count <= 0xFFFF {
            // Use READ(10)
            request.cdb_length = 10;
            request.cdb[0] = ScsiCommand::Read10 as u8;
            request.cdb[2..6].copy_from_slice(&(lba as u32).to_be_bytes());
            request.cdb[7..9].copy_from_slice(&(count as u16).to_be_bytes());
        } else {
            // Use READ(16)
            request.cdb_length = 16;
            request.cdb[0] = ScsiCommand::Read16 as u8;
            request.cdb[2..10].copy_from_slice(&lba.to_be_bytes());
            request.cdb[10..14].copy_from_slice(&count.to_be_bytes());
        }

        let response = self.send_scsi_command(channel, &request)?;
        let copy_len = response.len().min(buffer.len());
        buffer[..copy_len].copy_from_slice(&response[..copy_len]);

        self.stats.read_requests.fetch_add(1, Ordering::Relaxed);
        self.stats.bytes_read.fetch_add(bytes_to_read as u64, Ordering::Relaxed);

        Ok(())
    }

    /// Write sectors
    pub fn write_sectors(&mut self, channel: &mut VmbusChannel, disk_idx: usize, lba: u64, count: u32, data: &[u8]) -> Result<(), &'static str> {
        if !self.initialized.load(Ordering::Acquire) {
            return Err("Device not initialized");
        }

        let disk = self.disks.get(disk_idx).ok_or("Invalid disk index")?;
        let bytes_to_write = (count * disk.sector_size) as usize;

        if data.len() < bytes_to_write {
            return Err("Data too short");
        }

        if lba + count as u64 > disk.sector_count {
            return Err("Write beyond disk end");
        }

        // Build WRITE command
        let mut request = ScsiRequest::default();
        request.target_id = disk.target_id;
        request.lun = disk.lun;
        request.data_in = 0; // Data out
        request.data_transfer_length = bytes_to_write as u32;

        if lba <= u32::MAX as u64 && count <= 0xFFFF {
            // Use WRITE(10)
            request.cdb_length = 10;
            request.cdb[0] = ScsiCommand::Write10 as u8;
            request.cdb[2..6].copy_from_slice(&(lba as u32).to_be_bytes());
            request.cdb[7..9].copy_from_slice(&(count as u16).to_be_bytes());
        } else {
            // Use WRITE(16)
            request.cdb_length = 16;
            request.cdb[0] = ScsiCommand::Write16 as u8;
            request.cdb[2..10].copy_from_slice(&lba.to_be_bytes());
            request.cdb[10..14].copy_from_slice(&count.to_be_bytes());
        }

        let _ = self.send_scsi_command(channel, &request)?;

        self.stats.write_requests.fetch_add(1, Ordering::Relaxed);
        self.stats.bytes_written.fetch_add(bytes_to_write as u64, Ordering::Relaxed);

        Ok(())
    }

    /// Flush cache
    pub fn flush(&mut self, channel: &mut VmbusChannel, disk_idx: usize) -> Result<(), &'static str> {
        if !self.initialized.load(Ordering::Acquire) {
            return Err("Device not initialized");
        }

        let disk = self.disks.get(disk_idx).ok_or("Invalid disk index")?;

        let mut request = ScsiRequest::default();
        request.target_id = disk.target_id;
        request.lun = disk.lun;
        request.cdb_length = 10;
        request.cdb[0] = ScsiCommand::SynchronizeCache10 as u8;

        let _ = self.send_scsi_command(channel, &request)?;

        Ok(())
    }

    /// Get disk list
    pub fn disks(&self) -> &[DiskInfo] {
        &self.disks
    }

    /// Get disk count
    pub fn disk_count(&self) -> usize {
        self.disks.len()
    }

    /// Get statistics
    pub fn stats(&self) -> &StorvscStats {
        &self.stats
    }

    /// Format status
    pub fn format_status(&self) -> String {
        alloc::format!(
            "StorVSC: version={} disks={} reads={} writes={}",
            self.protocol_version, self.disks.len(),
            self.stats.read_requests.load(Ordering::Relaxed),
            self.stats.write_requests.load(Ordering::Relaxed)
        )
    }
}

impl Default for StorvscDevice {
    fn default() -> Self {
        Self::new(0)
    }
}

/// StorVSC manager
pub struct StorvscManager {
    devices: Vec<StorvscDevice>,
}

impl StorvscManager {
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
        }
    }

    pub fn add_device(&mut self, device: StorvscDevice) -> usize {
        let idx = self.devices.len();
        self.devices.push(device);
        idx
    }

    pub fn get_device(&mut self, idx: usize) -> Option<&mut StorvscDevice> {
        self.devices.get_mut(idx)
    }

    pub fn device_count(&self) -> usize {
        self.devices.len()
    }
}

impl Default for StorvscManager {
    fn default() -> Self {
        Self::new()
    }
}
