//! USB Mass Storage driver (Bulk-Only Transport protocol).
//!
//! Supports USB flash drives, external hard drives, and card readers
//! using SCSI Bulk-Only Transport (BBB) protocol.

#![allow(dead_code)]

use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

use crate::storage::{BlockDevice, BlockDeviceId};
use crate::util::{KError, KResult};

use super::{class, EndpointDescriptor, EndpointDirection, InterfaceDescriptor};

/// USB Mass Storage Class-specific requests
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum MassStorageRequest {
    GetMaxLun = 0xFE,
    BulkOnlyReset = 0xFF,
}

/// Command Block Wrapper (CBW) - 31 bytes
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Cbw {
    pub signature: u32,       // 0x43425355 ('USBC')
    pub tag: u32,             // Transaction tag
    pub data_transfer_length: u32, // Expected data length
    pub flags: u8,            // Direction: bit7 = 0 out, 1 in
    pub lun: u8,              // Logical unit number
    pub cb_length: u8,        // Command block length (1-16)
    pub cb: [u8; 16],         // Command block (SCSI command)
}

impl Cbw {
    pub const SIGNATURE: u32 = 0x43425355; // 'USBC'
    pub const SIZE: usize = 31;

    /// Create a new CBW
    pub fn new(tag: u32, data_length: u32, direction_in: bool, lun: u8, command: &[u8]) -> Self {
        let mut cb = [0u8; 16];
        let len = command.len().min(16);
        cb[..len].copy_from_slice(&command[..len]);

        Self {
            signature: Self::SIGNATURE,
            tag,
            data_transfer_length: data_length,
            flags: if direction_in { 0x80 } else { 0x00 },
            lun,
            cb_length: len as u8,
            cb,
        }
    }

    /// Convert to bytes
    pub fn to_bytes(&self) -> [u8; 31] {
        let mut bytes = [0u8; 31];
        bytes[0..4].copy_from_slice(&self.signature.to_le_bytes());
        bytes[4..8].copy_from_slice(&self.tag.to_le_bytes());
        bytes[8..12].copy_from_slice(&self.data_transfer_length.to_le_bytes());
        bytes[12] = self.flags;
        bytes[13] = self.lun;
        bytes[14] = self.cb_length;
        bytes[15..31].copy_from_slice(&self.cb);
        bytes
    }
}

/// Command Status Wrapper (CSW) - 13 bytes
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Csw {
    pub signature: u32,  // 0x53425355 ('USBS')
    pub tag: u32,        // Must match CBW tag
    pub data_residue: u32, // Difference between expected and actual data
    pub status: u8,      // 0 = passed, 1 = failed, 2 = phase error
}

impl Csw {
    pub const SIGNATURE: u32 = 0x53425355; // 'USBS'
    pub const SIZE: usize = 13;

    pub const STATUS_PASSED: u8 = 0;
    pub const STATUS_FAILED: u8 = 1;
    pub const STATUS_PHASE_ERROR: u8 = 2;

    /// Parse from bytes
    pub fn from_bytes(bytes: &[u8; 13]) -> Self {
        Self {
            signature: u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            tag: u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
            data_residue: u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
            status: bytes[12],
        }
    }

    pub fn is_valid(&self, expected_tag: u32) -> bool {
        self.signature == Self::SIGNATURE && self.tag == expected_tag
    }

    pub fn is_success(&self) -> bool {
        self.status == Self::STATUS_PASSED
    }
}

/// SCSI Commands
pub mod scsi {
    pub const TEST_UNIT_READY: u8 = 0x00;
    pub const REQUEST_SENSE: u8 = 0x03;
    pub const INQUIRY: u8 = 0x12;
    pub const MODE_SENSE_6: u8 = 0x1A;
    pub const START_STOP_UNIT: u8 = 0x1B;
    pub const PREVENT_ALLOW_MEDIUM_REMOVAL: u8 = 0x1E;
    pub const READ_CAPACITY_10: u8 = 0x25;
    pub const READ_10: u8 = 0x28;
    pub const WRITE_10: u8 = 0x2A;
    pub const VERIFY_10: u8 = 0x2F;
    pub const MODE_SENSE_10: u8 = 0x5A;
    pub const READ_CAPACITY_16: u8 = 0x9E;
    pub const READ_16: u8 = 0x88;
    pub const WRITE_16: u8 = 0x8A;
}

/// SCSI INQUIRY response
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct InquiryResponse {
    pub peripheral_info: u8, // bits 0-4: device type, bits 5-7: qualifier
    pub removable: u8,       // bit 7: removable
    pub version: u8,
    pub response_format: u8,
    pub additional_length: u8,
    pub flags: [u8; 3],
    pub vendor_id: [u8; 8],
    pub product_id: [u8; 16],
    pub product_revision: [u8; 4],
}

impl InquiryResponse {
    pub fn device_type(&self) -> u8 {
        self.peripheral_info & 0x1F
    }

    pub fn is_removable(&self) -> bool {
        (self.removable & 0x80) != 0
    }

    pub fn vendor(&self) -> &str {
        core::str::from_utf8(&self.vendor_id)
            .unwrap_or("")
            .trim()
    }

    pub fn product(&self) -> &str {
        core::str::from_utf8(&self.product_id)
            .unwrap_or("")
            .trim()
    }
}

/// READ CAPACITY (10) response
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ReadCapacity10Response {
    pub last_lba: u32,     // Big-endian
    pub block_size: u32,   // Big-endian
}

impl ReadCapacity10Response {
    pub fn from_bytes(bytes: &[u8; 8]) -> Self {
        Self {
            last_lba: u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            block_size: u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
        }
    }

    pub fn total_blocks(&self) -> u64 {
        (self.last_lba as u64) + 1
    }
}

/// USB Mass Storage device
#[derive(Debug)]
pub struct MassStorageDevice {
    pub slot_id: u8,
    pub interface_number: u8,
    pub bulk_in_endpoint: u8,
    pub bulk_out_endpoint: u8,
    pub max_packet_size: u16,
    pub max_lun: u8,
    pub block_size: u32,
    pub total_blocks: u64,
    pub tag: Mutex<u32>,
}

impl MassStorageDevice {
    pub fn new(
        slot_id: u8,
        interface_number: u8,
        bulk_in_endpoint: u8,
        bulk_out_endpoint: u8,
        max_packet_size: u16,
    ) -> Self {
        Self {
            slot_id,
            interface_number,
            bulk_in_endpoint,
            bulk_out_endpoint,
            max_packet_size,
            max_lun: 0,
            block_size: 512,
            total_blocks: 0,
            tag: Mutex::new(1),
        }
    }

    /// Get next transaction tag
    fn next_tag(&self) -> u32 {
        let mut tag = self.tag.lock();
        let t = *tag;
        *tag = tag.wrapping_add(1);
        if *tag == 0 {
            *tag = 1;
        }
        t
    }

    /// Build INQUIRY command
    pub fn build_inquiry(&self) -> Cbw {
        let mut cmd = [0u8; 6];
        cmd[0] = scsi::INQUIRY;
        cmd[4] = 36; // Allocation length
        Cbw::new(self.next_tag(), 36, true, 0, &cmd)
    }

    /// Build TEST UNIT READY command
    pub fn build_test_unit_ready(&self) -> Cbw {
        let cmd = [scsi::TEST_UNIT_READY, 0, 0, 0, 0, 0];
        Cbw::new(self.next_tag(), 0, false, 0, &cmd)
    }

    /// Build READ CAPACITY (10) command
    pub fn build_read_capacity_10(&self) -> Cbw {
        let mut cmd = [0u8; 10];
        cmd[0] = scsi::READ_CAPACITY_10;
        Cbw::new(self.next_tag(), 8, true, 0, &cmd)
    }

    /// Build READ (10) command
    pub fn build_read_10(&self, lba: u32, block_count: u16) -> Cbw {
        let mut cmd = [0u8; 10];
        cmd[0] = scsi::READ_10;
        cmd[2] = (lba >> 24) as u8;
        cmd[3] = (lba >> 16) as u8;
        cmd[4] = (lba >> 8) as u8;
        cmd[5] = lba as u8;
        cmd[7] = (block_count >> 8) as u8;
        cmd[8] = block_count as u8;

        let data_length = (block_count as u32) * self.block_size;
        Cbw::new(self.next_tag(), data_length, true, 0, &cmd)
    }

    /// Build WRITE (10) command
    pub fn build_write_10(&self, lba: u32, block_count: u16) -> Cbw {
        let mut cmd = [0u8; 10];
        cmd[0] = scsi::WRITE_10;
        cmd[2] = (lba >> 24) as u8;
        cmd[3] = (lba >> 16) as u8;
        cmd[4] = (lba >> 8) as u8;
        cmd[5] = lba as u8;
        cmd[7] = (block_count >> 8) as u8;
        cmd[8] = block_count as u8;

        let data_length = (block_count as u32) * self.block_size;
        Cbw::new(self.next_tag(), data_length, false, 0, &cmd)
    }

    /// Build REQUEST SENSE command
    pub fn build_request_sense(&self) -> Cbw {
        let mut cmd = [0u8; 6];
        cmd[0] = scsi::REQUEST_SENSE;
        cmd[4] = 18; // Allocation length
        Cbw::new(self.next_tag(), 18, true, 0, &cmd)
    }
}

/// USB Mass Storage to BlockDevice adapter
pub struct UsbBlockDevice {
    device: Arc<Mutex<MassStorageDevice>>,
    device_id: BlockDeviceId,
}

impl UsbBlockDevice {
    pub fn new(device: Arc<Mutex<MassStorageDevice>>, id: u32) -> Self {
        Self {
            device,
            device_id: BlockDeviceId(id),
        }
    }

    /// Initialize the device (INQUIRY, READ CAPACITY, etc.)
    pub fn initialize(&self) -> KResult<()> {
        let mut dev = self.device.lock();

        // Test Unit Ready
        let cbw = dev.build_test_unit_ready();
        self.send_cbw(&cbw)?;
        self.recv_csw(cbw.tag)?;

        // INQUIRY
        let cbw = dev.build_inquiry();
        self.send_cbw(&cbw)?;

        let mut inquiry_data = [0u8; 36];
        self.bulk_in(&mut inquiry_data)?;
        self.recv_csw(cbw.tag)?;

        let inquiry = unsafe {
            core::ptr::read_unaligned(inquiry_data.as_ptr() as *const InquiryResponse)
        };
        crate::kprintln!("usb-storage: {} {}", inquiry.vendor(), inquiry.product());

        // READ CAPACITY (10)
        let cbw = dev.build_read_capacity_10();
        self.send_cbw(&cbw)?;

        let mut cap_data = [0u8; 8];
        self.bulk_in(&mut cap_data)?;
        self.recv_csw(cbw.tag)?;

        let capacity = ReadCapacity10Response::from_bytes(&cap_data);
        dev.block_size = capacity.block_size;
        dev.total_blocks = capacity.total_blocks();

        crate::kprintln!(
            "usb-storage: {} blocks x {} bytes = {} MB",
            dev.total_blocks,
            dev.block_size,
            (dev.total_blocks * dev.block_size as u64) / (1024 * 1024)
        );

        Ok(())
    }

    /// Send CBW (Command Block Wrapper)
    fn send_cbw(&self, cbw: &Cbw) -> KResult<()> {
        let dev = self.device.lock();
        let bytes = cbw.to_bytes();

        if let Some(ctrl_arc) = super::xhci::controller() {
            let mut ctrl = ctrl_arc.lock();
            ctrl.bulk_transfer_out(dev.slot_id, dev.bulk_out_endpoint, &bytes)?;
            Ok(())
        } else {
            Err(KError::NotSupported)
        }
    }

    /// Receive CSW (Command Status Wrapper)
    fn recv_csw(&self, expected_tag: u32) -> KResult<Csw> {
        let dev = self.device.lock();
        let mut csw_bytes = [0u8; 13];

        if let Some(ctrl_arc) = super::xhci::controller() {
            let mut ctrl = ctrl_arc.lock();
            ctrl.bulk_transfer_in(dev.slot_id, dev.bulk_in_endpoint, &mut csw_bytes)?;

            let csw = Csw::from_bytes(&csw_bytes);
            if !csw.is_valid(expected_tag) {
                return Err(KError::Invalid);
            }
            if !csw.is_success() {
                return Err(KError::IO);
            }
            Ok(csw)
        } else {
            Err(KError::NotSupported)
        }
    }

    /// Bulk OUT transfer
    fn bulk_out(&self, data: &[u8]) -> KResult<()> {
        let dev = self.device.lock();

        if let Some(ctrl_arc) = super::xhci::controller() {
            let mut ctrl = ctrl_arc.lock();
            ctrl.bulk_transfer_out(dev.slot_id, dev.bulk_out_endpoint, data)?;
            Ok(())
        } else {
            Err(KError::NotSupported)
        }
    }

    /// Bulk IN transfer
    fn bulk_in(&self, buffer: &mut [u8]) -> KResult<usize> {
        let dev = self.device.lock();

        if let Some(ctrl_arc) = super::xhci::controller() {
            let mut ctrl = ctrl_arc.lock();
            ctrl.bulk_transfer_in(dev.slot_id, dev.bulk_in_endpoint, buffer)
        } else {
            Err(KError::NotSupported)
        }
    }
}

impl BlockDevice for UsbBlockDevice {
    fn id(&self) -> BlockDeviceId {
        self.device_id
    }

    fn block_size(&self) -> u32 {
        self.device.lock().block_size
    }

    fn num_blocks(&self) -> u64 {
        self.device.lock().total_blocks
    }

    fn read_blocks(&self, lba: u64, count: u32, out: &mut [u8]) -> KResult<()> {
        crate::storage::block::check_io_args(self.block_size(), count, out.len())?;

        let block_size = self.block_size();

        // Read in chunks (max 128 blocks at a time to avoid timeouts)
        let max_blocks_per_transfer = 128u32;
        let mut offset = 0usize;
        let mut remaining = count;
        let mut current_lba = lba;

        while remaining > 0 {
            let blocks_to_read = remaining.min(max_blocks_per_transfer);

            // Build READ(10) command
            let cbw = {
                let dev = self.device.lock();
                dev.build_read_10(current_lba as u32, blocks_to_read as u16)
            };

            let tag = cbw.tag;
            self.send_cbw(&cbw)?;

            // Receive data
            let bytes_to_read = (blocks_to_read * block_size) as usize;
            self.bulk_in(&mut out[offset..offset + bytes_to_read])?;

            // Get status
            self.recv_csw(tag)?;

            offset += bytes_to_read;
            current_lba += blocks_to_read as u64;
            remaining -= blocks_to_read;
        }

        Ok(())
    }

    fn write_blocks(&self, lba: u64, count: u32, data: &[u8]) -> KResult<()> {
        crate::storage::block::check_io_args(self.block_size(), count, data.len())?;

        let block_size = self.block_size();

        // Write in chunks
        let max_blocks_per_transfer = 128u32;
        let mut offset = 0usize;
        let mut remaining = count;
        let mut current_lba = lba;

        while remaining > 0 {
            let blocks_to_write = remaining.min(max_blocks_per_transfer);

            // Build WRITE(10) command
            let cbw = {
                let dev = self.device.lock();
                dev.build_write_10(current_lba as u32, blocks_to_write as u16)
            };

            let tag = cbw.tag;
            self.send_cbw(&cbw)?;

            // Send data
            let bytes_to_write = (blocks_to_write * block_size) as usize;
            self.bulk_out(&data[offset..offset + bytes_to_write])?;

            // Get status
            self.recv_csw(tag)?;

            offset += bytes_to_write;
            current_lba += blocks_to_write as u64;
            remaining -= blocks_to_write;
        }

        Ok(())
    }
}

/// Create a BlockDevice from a MassStorageDevice
pub fn create_block_device(device: Arc<Mutex<MassStorageDevice>>, id: u32) -> KResult<Arc<UsbBlockDevice>> {
    // Configure bulk endpoints
    {
        let dev = device.lock();
        if let Some(ctrl_arc) = super::xhci::controller() {
            let mut ctrl = ctrl_arc.lock();
            ctrl.configure_bulk_endpoint(dev.slot_id, dev.bulk_in_endpoint, true, dev.max_packet_size)?;
            ctrl.configure_bulk_endpoint(dev.slot_id, dev.bulk_out_endpoint, false, dev.max_packet_size)?;
        } else {
            return Err(KError::NotSupported);
        }
    }

    let block_dev = Arc::new(UsbBlockDevice::new(device, id));
    block_dev.initialize()?;

    Ok(block_dev)
}

/// Global mass storage device list
static MASS_STORAGE_DEVICES: Mutex<Vec<Arc<Mutex<MassStorageDevice>>>> = Mutex::new(Vec::new());

/// Register a mass storage device
pub fn register_device(device: MassStorageDevice) {
    let dev = Arc::new(Mutex::new(device));
    {
        let d = dev.lock();
        crate::kprintln!(
            "usb-storage: registered (slot {}, bulk_in={}, bulk_out={})",
            d.slot_id,
            d.bulk_in_endpoint,
            d.bulk_out_endpoint
        );
    }

    let mut devices = MASS_STORAGE_DEVICES.lock();
    devices.push(dev);
}

/// SCSI subclass and Bulk-Only protocol constants
const MASS_STORAGE_SCSI: u8 = 0x06;
const MASS_STORAGE_BULK_ONLY: u8 = 0x50;

/// Check if an interface is Mass Storage
pub fn is_mass_storage_interface(iface: &InterfaceDescriptor) -> bool {
    iface.interface_class == class::MASS_STORAGE
        && iface.interface_subclass == MASS_STORAGE_SCSI
        && iface.interface_protocol == MASS_STORAGE_BULK_ONLY
}

/// Get all registered mass storage devices
pub fn devices() -> Vec<Arc<Mutex<MassStorageDevice>>> {
    let devices = MASS_STORAGE_DEVICES.lock();
    devices.clone()
}

/// Configure a mass storage device
pub fn configure_device(
    slot_id: u8,
    iface_desc: &InterfaceDescriptor,
    endpoints: &[EndpointDescriptor],
) -> Result<(), KError> {
    if !is_mass_storage_interface(iface_desc) {
        return Ok(());
    }

    // Find bulk IN and bulk OUT endpoints
    let mut bulk_in = None;
    let mut bulk_out = None;
    let mut max_packet = 0u16;

    for ep in endpoints {
        if ep.transfer_type() == super::EndpointType::Bulk {
            max_packet = max_packet.max(ep.max_packet_size);
            match ep.direction() {
                EndpointDirection::In => bulk_in = Some(ep.endpoint_number()),
                EndpointDirection::Out => bulk_out = Some(ep.endpoint_number()),
            }
        }
    }

    match (bulk_in, bulk_out) {
        (Some(in_ep), Some(out_ep)) => {
            let device = MassStorageDevice::new(
                slot_id,
                iface_desc.interface_number,
                in_ep,
                out_ep,
                max_packet,
            );
            register_device(device);
            Ok(())
        }
        _ => {
            crate::kprintln!("usb-storage: missing bulk endpoints");
            Err(KError::Invalid)
        }
    }
}

/// Initialize mass storage subsystem
pub fn init() {
    crate::kprintln!("usb-storage: initialized");
}
