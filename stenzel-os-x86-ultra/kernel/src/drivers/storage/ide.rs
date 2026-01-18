//! IDE/ATA Driver for Legacy Hard Drives
//!
//! Implements PIO (Programmed I/O) mode for legacy IDE/ATA drives.
//! This driver supports:
//! - Primary and Secondary IDE channels
//! - Master and Slave devices per channel
//! - PIO read/write modes 0-4
//! - ATAPI CD/DVD detection (read-only support)
//! - 28-bit and 48-bit LBA addressing
//!
//! References:
//! - ATA/ATAPI-6 Specification
//! - https://wiki.osdev.org/ATA_PIO_Mode

#![allow(dead_code)]

extern crate alloc;

use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use alloc::string::{String, ToString};
use core::sync::atomic::{AtomicBool, Ordering};
use spin::Mutex;
use x86_64::instructions::port::{Port, PortReadOnly, PortWriteOnly};

use crate::drivers::pci::PciDevice;
use crate::storage::{BlockDevice, BlockDeviceId};
use crate::util::{KError, KResult};

// ============================================================================
// Constants
// ============================================================================

// Standard IDE I/O port addresses
const PRIMARY_IO_BASE: u16 = 0x1F0;
const PRIMARY_CTRL_BASE: u16 = 0x3F6;
const SECONDARY_IO_BASE: u16 = 0x170;
const SECONDARY_CTRL_BASE: u16 = 0x376;

// I/O port offsets from base
const REG_DATA: u16 = 0;          // Data register (R/W)
const REG_ERROR: u16 = 1;         // Error register (R)
const REG_FEATURES: u16 = 1;      // Features register (W)
const REG_SECCOUNT: u16 = 2;      // Sector count register
const REG_LBA_LO: u16 = 3;        // LBA low byte
const REG_LBA_MID: u16 = 4;       // LBA mid byte
const REG_LBA_HI: u16 = 5;        // LBA high byte
const REG_DRIVE: u16 = 6;         // Drive/head register
const REG_STATUS: u16 = 7;        // Status register (R)
const REG_COMMAND: u16 = 7;       // Command register (W)

// Control port offset
const REG_ALT_STATUS: u16 = 0;    // Alternate status (R)
const REG_DEVICE_CTRL: u16 = 0;   // Device control (W)

// Status register bits
const STATUS_ERR: u8 = 0x01;      // Error occurred
const STATUS_DRQ: u8 = 0x08;      // Data request ready
const STATUS_SRV: u8 = 0x10;      // Service request
const STATUS_DF: u8 = 0x20;       // Drive fault
const STATUS_RDY: u8 = 0x40;      // Drive ready
const STATUS_BSY: u8 = 0x80;      // Drive busy

// Error register bits
const ERR_AMNF: u8 = 0x01;        // Address mark not found
const ERR_TK0NF: u8 = 0x02;       // Track 0 not found
const ERR_ABRT: u8 = 0x04;        // Command aborted
const ERR_MCR: u8 = 0x08;         // Media change request
const ERR_IDNF: u8 = 0x10;        // ID not found
const ERR_MC: u8 = 0x20;          // Media changed
const ERR_UNC: u8 = 0x40;         // Uncorrectable data error
const ERR_BBK: u8 = 0x80;         // Bad block

// Device control register bits
const CTRL_NIEN: u8 = 0x02;       // Disable interrupts
const CTRL_SRST: u8 = 0x04;       // Software reset
const CTRL_HOB: u8 = 0x80;        // High order byte (for LBA48)

// Drive/head register bits
const DRIVE_LBA: u8 = 0x40;       // Use LBA addressing
const DRIVE_MASTER: u8 = 0xA0;    // Select master
const DRIVE_SLAVE: u8 = 0xB0;     // Select slave

// ATA commands
const CMD_READ_PIO: u8 = 0x20;         // Read sectors (PIO)
const CMD_READ_PIO_EXT: u8 = 0x24;     // Read sectors (PIO) LBA48
const CMD_WRITE_PIO: u8 = 0x30;        // Write sectors (PIO)
const CMD_WRITE_PIO_EXT: u8 = 0x34;    // Write sectors (PIO) LBA48
const CMD_CACHE_FLUSH: u8 = 0xE7;      // Flush write cache
const CMD_CACHE_FLUSH_EXT: u8 = 0xEA;  // Flush write cache LBA48
const CMD_IDENTIFY: u8 = 0xEC;         // Identify device
const CMD_IDENTIFY_PACKET: u8 = 0xA1;  // Identify packet device (ATAPI)
const CMD_SET_FEATURES: u8 = 0xEF;     // Set features

// ATAPI commands
const CMD_PACKET: u8 = 0xA0;           // ATAPI packet command

// PCI class/subclass for IDE controllers
const IDE_CLASS: u8 = 0x01;            // Mass storage controller
const IDE_SUBCLASS: u8 = 0x01;         // IDE controller

// Timeouts (in loop iterations)
const TIMEOUT_DEFAULT: u32 = 1_000_000;
const TIMEOUT_IDENTIFY: u32 = 100_000;

// ============================================================================
// Data Structures
// ============================================================================

/// IDE channel (primary or secondary)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdeChannel {
    Primary,
    Secondary,
}

impl IdeChannel {
    fn io_base(&self) -> u16 {
        match self {
            IdeChannel::Primary => PRIMARY_IO_BASE,
            IdeChannel::Secondary => SECONDARY_IO_BASE,
        }
    }

    fn ctrl_base(&self) -> u16 {
        match self {
            IdeChannel::Primary => PRIMARY_CTRL_BASE,
            IdeChannel::Secondary => SECONDARY_CTRL_BASE,
        }
    }
}

/// Device role (master or slave)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdeRole {
    Master,
    Slave,
}

impl IdeRole {
    fn drive_select(&self) -> u8 {
        match self {
            IdeRole::Master => DRIVE_MASTER,
            IdeRole::Slave => DRIVE_SLAVE,
        }
    }
}

/// Device type detected
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdeDeviceType {
    Ata,      // Hard drive
    Atapi,    // CD/DVD
    Unknown,
}

/// IDE device identity information
#[derive(Debug, Clone)]
pub struct IdeIdentity {
    pub model: String,
    pub serial: String,
    pub firmware: String,
    pub device_type: IdeDeviceType,
    pub supports_lba: bool,
    pub supports_lba48: bool,
    pub sectors_28: u32,
    pub sectors_48: u64,
    pub sector_size: u16,
}

impl IdeIdentity {
    fn from_identify_data(data: &[u16; 256], device_type: IdeDeviceType) -> Self {
        // Parse model string (words 27-46)
        let model = Self::parse_ata_string(&data[27..47]);
        // Parse serial (words 10-19)
        let serial = Self::parse_ata_string(&data[10..20]);
        // Parse firmware (words 23-26)
        let firmware = Self::parse_ata_string(&data[23..27]);

        // Capabilities (word 49)
        let caps = data[49];
        let supports_lba = (caps & 0x0200) != 0;

        // Command set supported (word 83)
        let cmd_set = data[83];
        let supports_lba48 = (cmd_set & 0x0400) != 0;

        // Total sectors 28-bit (words 60-61)
        let sectors_28 = (data[61] as u32) << 16 | (data[60] as u32);

        // Total sectors 48-bit (words 100-103)
        let sectors_48 = if supports_lba48 {
            (data[103] as u64) << 48 |
            (data[102] as u64) << 32 |
            (data[101] as u64) << 16 |
            (data[100] as u64)
        } else {
            sectors_28 as u64
        };

        // Logical sector size (word 117-118 if bit 12 of word 106 is set)
        let physical_logical_sector = data[106];
        let sector_size = if (physical_logical_sector & 0x1000) != 0 {
            // Words 117-118 contain logical sector size in words
            let words = (data[118] as u32) << 16 | (data[117] as u32);
            (words * 2) as u16
        } else {
            512
        };

        Self {
            model,
            serial,
            firmware,
            device_type,
            supports_lba,
            supports_lba48,
            sectors_28,
            sectors_48,
            sector_size,
        }
    }

    fn parse_ata_string(words: &[u16]) -> String {
        let mut bytes = Vec::with_capacity(words.len() * 2);
        for word in words {
            // ATA strings are stored with bytes swapped
            bytes.push((word >> 8) as u8);
            bytes.push((word & 0xFF) as u8);
        }
        // Trim trailing spaces and null bytes
        while bytes.last().map(|&b| b == 0x20 || b == 0).unwrap_or(false) {
            bytes.pop();
        }
        String::from_utf8_lossy(&bytes).trim().to_string()
    }
}

/// Low-level IDE channel access
struct IdeChannelPorts {
    // I/O ports
    data: Port<u16>,
    error: PortReadOnly<u8>,
    features: PortWriteOnly<u8>,
    sector_count: Port<u8>,
    lba_lo: Port<u8>,
    lba_mid: Port<u8>,
    lba_hi: Port<u8>,
    drive: Port<u8>,
    status: PortReadOnly<u8>,
    command: PortWriteOnly<u8>,
    // Control ports
    alt_status: PortReadOnly<u8>,
    device_ctrl: PortWriteOnly<u8>,
}

impl IdeChannelPorts {
    fn new(channel: IdeChannel) -> Self {
        let io_base = channel.io_base();
        let ctrl_base = channel.ctrl_base();

        Self {
            data: Port::new(io_base + REG_DATA),
            error: PortReadOnly::new(io_base + REG_ERROR),
            features: PortWriteOnly::new(io_base + REG_FEATURES),
            sector_count: Port::new(io_base + REG_SECCOUNT),
            lba_lo: Port::new(io_base + REG_LBA_LO),
            lba_mid: Port::new(io_base + REG_LBA_MID),
            lba_hi: Port::new(io_base + REG_LBA_HI),
            drive: Port::new(io_base + REG_DRIVE),
            status: PortReadOnly::new(io_base + REG_STATUS),
            command: PortWriteOnly::new(io_base + REG_COMMAND),
            alt_status: PortReadOnly::new(ctrl_base + REG_ALT_STATUS),
            device_ctrl: PortWriteOnly::new(ctrl_base + REG_DEVICE_CTRL),
        }
    }

    /// Read status (clears pending interrupt)
    unsafe fn read_status(&mut self) -> u8 {
        self.status.read()
    }

    /// Read alternate status (doesn't clear interrupt)
    unsafe fn read_alt_status(&mut self) -> u8 {
        self.alt_status.read()
    }

    /// Read error register
    unsafe fn read_error(&mut self) -> u8 {
        self.error.read()
    }

    /// Wait for BSY to clear
    unsafe fn wait_not_busy(&mut self, timeout: u32) -> bool {
        for _ in 0..timeout {
            let status = self.read_alt_status();
            if (status & STATUS_BSY) == 0 {
                return true;
            }
            core::hint::spin_loop();
        }
        false
    }

    /// Wait for DRQ or error
    unsafe fn wait_drq_or_error(&mut self, timeout: u32) -> Result<(), u8> {
        for _ in 0..timeout {
            let status = self.read_alt_status();
            if (status & STATUS_BSY) == 0 {
                if (status & STATUS_ERR) != 0 {
                    return Err(self.read_error());
                }
                if (status & STATUS_DRQ) != 0 {
                    return Ok(());
                }
            }
            core::hint::spin_loop();
        }
        Err(0xFF) // Timeout
    }

    /// Wait for ready
    unsafe fn wait_ready(&mut self, timeout: u32) -> bool {
        for _ in 0..timeout {
            let status = self.read_alt_status();
            if (status & STATUS_BSY) == 0 && (status & STATUS_RDY) != 0 {
                return true;
            }
            core::hint::spin_loop();
        }
        false
    }

    /// Select a drive
    unsafe fn select_drive(&mut self, role: IdeRole) {
        self.drive.write(role.drive_select());
        // Wait 400ns for drive select to take effect
        for _ in 0..15 {
            let _ = self.read_alt_status();
        }
    }

    /// Software reset
    unsafe fn software_reset(&mut self) {
        self.device_ctrl.write(CTRL_SRST | CTRL_NIEN);
        // Wait 5us
        for _ in 0..100 {
            core::hint::spin_loop();
        }
        self.device_ctrl.write(CTRL_NIEN);
        // Wait for BSY to clear
        for _ in 0..TIMEOUT_DEFAULT {
            if (self.read_alt_status() & STATUS_BSY) == 0 {
                break;
            }
            core::hint::spin_loop();
        }
    }

    /// Disable interrupts
    unsafe fn disable_interrupts(&mut self) {
        self.device_ctrl.write(CTRL_NIEN);
    }
}

/// A single IDE device
pub struct IdeDevice {
    channel: IdeChannel,
    role: IdeRole,
    identity: IdeIdentity,
    ports: Mutex<IdeChannelPorts>,
    device_id: BlockDeviceId,
}

unsafe impl Send for IdeDevice {}
unsafe impl Sync for IdeDevice {}

impl IdeDevice {
    /// Create a new IDE device
    fn new(channel: IdeChannel, role: IdeRole, identity: IdeIdentity, device_id: BlockDeviceId) -> Self {
        Self {
            channel,
            role,
            identity,
            ports: Mutex::new(IdeChannelPorts::new(channel)),
            device_id,
        }
    }

    /// Read sectors using PIO mode
    fn read_sectors(&self, lba: u64, count: u8, buffer: &mut [u8]) -> KResult<()> {
        if self.identity.device_type != IdeDeviceType::Ata {
            return Err(KError::NotSupported);
        }

        let sector_size = self.identity.sector_size as usize;
        let expected_len = count as usize * sector_size;
        if buffer.len() < expected_len {
            return Err(KError::Invalid);
        }

        let use_lba48 = lba >= (1 << 28) || self.identity.supports_lba48;

        let mut ports = self.ports.lock();

        unsafe {
            // Wait for not busy
            if !ports.wait_not_busy(TIMEOUT_DEFAULT) {
                return Err(KError::Timeout);
            }

            // Select drive with LBA mode
            if use_lba48 {
                // LBA48 addressing
                ports.drive.write(DRIVE_LBA | (self.role.drive_select() & 0x10));
                for _ in 0..4 { let _ = ports.read_alt_status(); }

                // Write high bytes first
                ports.sector_count.write(0); // High byte of sector count
                ports.lba_lo.write(((lba >> 24) & 0xFF) as u8);
                ports.lba_mid.write(((lba >> 32) & 0xFF) as u8);
                ports.lba_hi.write(((lba >> 40) & 0xFF) as u8);

                // Then low bytes
                ports.sector_count.write(count);
                ports.lba_lo.write((lba & 0xFF) as u8);
                ports.lba_mid.write(((lba >> 8) & 0xFF) as u8);
                ports.lba_hi.write(((lba >> 16) & 0xFF) as u8);

                // Send command
                ports.command.write(CMD_READ_PIO_EXT);
            } else {
                // LBA28 addressing
                ports.drive.write(DRIVE_LBA | (self.role.drive_select() & 0x10) | ((lba >> 24) & 0x0F) as u8);
                for _ in 0..4 { let _ = ports.read_alt_status(); }

                ports.sector_count.write(count);
                ports.lba_lo.write((lba & 0xFF) as u8);
                ports.lba_mid.write(((lba >> 8) & 0xFF) as u8);
                ports.lba_hi.write(((lba >> 16) & 0xFF) as u8);

                // Send command
                ports.command.write(CMD_READ_PIO);
            }

            // Read each sector
            let words_per_sector = sector_size / 2;
            for sector in 0..count as usize {
                // Wait for DRQ
                if let Err(err_code) = ports.wait_drq_or_error(TIMEOUT_DEFAULT) {
                    crate::kprintln!("ide: read error: 0x{:02x}", err_code);
                    return Err(KError::IO);
                }

                // Read data
                let offset = sector * sector_size;
                for word in 0..words_per_sector {
                    let data = ports.data.read();
                    let idx = offset + word * 2;
                    buffer[idx] = (data & 0xFF) as u8;
                    buffer[idx + 1] = (data >> 8) as u8;
                }
            }
        }

        Ok(())
    }

    /// Write sectors using PIO mode
    fn write_sectors(&self, lba: u64, count: u8, data: &[u8]) -> KResult<()> {
        if self.identity.device_type != IdeDeviceType::Ata {
            return Err(KError::NotSupported);
        }

        let sector_size = self.identity.sector_size as usize;
        let expected_len = count as usize * sector_size;
        if data.len() < expected_len {
            return Err(KError::Invalid);
        }

        let use_lba48 = lba >= (1 << 28) || self.identity.supports_lba48;

        let mut ports = self.ports.lock();

        unsafe {
            // Wait for not busy
            if !ports.wait_not_busy(TIMEOUT_DEFAULT) {
                return Err(KError::Timeout);
            }

            // Select drive with LBA mode
            if use_lba48 {
                // LBA48 addressing
                ports.drive.write(DRIVE_LBA | (self.role.drive_select() & 0x10));
                for _ in 0..4 { let _ = ports.read_alt_status(); }

                // Write high bytes first
                ports.sector_count.write(0);
                ports.lba_lo.write(((lba >> 24) & 0xFF) as u8);
                ports.lba_mid.write(((lba >> 32) & 0xFF) as u8);
                ports.lba_hi.write(((lba >> 40) & 0xFF) as u8);

                // Then low bytes
                ports.sector_count.write(count);
                ports.lba_lo.write((lba & 0xFF) as u8);
                ports.lba_mid.write(((lba >> 8) & 0xFF) as u8);
                ports.lba_hi.write(((lba >> 16) & 0xFF) as u8);

                // Send command
                ports.command.write(CMD_WRITE_PIO_EXT);
            } else {
                // LBA28 addressing
                ports.drive.write(DRIVE_LBA | (self.role.drive_select() & 0x10) | ((lba >> 24) & 0x0F) as u8);
                for _ in 0..4 { let _ = ports.read_alt_status(); }

                ports.sector_count.write(count);
                ports.lba_lo.write((lba & 0xFF) as u8);
                ports.lba_mid.write(((lba >> 8) & 0xFF) as u8);
                ports.lba_hi.write(((lba >> 16) & 0xFF) as u8);

                // Send command
                ports.command.write(CMD_WRITE_PIO);
            }

            // Write each sector
            let words_per_sector = sector_size / 2;
            for sector in 0..count as usize {
                // Wait for DRQ
                if let Err(err_code) = ports.wait_drq_or_error(TIMEOUT_DEFAULT) {
                    crate::kprintln!("ide: write error: 0x{:02x}", err_code);
                    return Err(KError::IO);
                }

                // Write data
                let offset = sector * sector_size;
                for word in 0..words_per_sector {
                    let idx = offset + word * 2;
                    let word_data = (data[idx] as u16) | ((data[idx + 1] as u16) << 8);
                    ports.data.write(word_data);
                }
            }

            // Flush cache
            ports.command.write(if use_lba48 { CMD_CACHE_FLUSH_EXT } else { CMD_CACHE_FLUSH });
            if !ports.wait_not_busy(TIMEOUT_DEFAULT) {
                return Err(KError::Timeout);
            }
        }

        Ok(())
    }
}

impl BlockDevice for IdeDevice {
    fn id(&self) -> BlockDeviceId {
        self.device_id
    }

    fn block_size(&self) -> u32 {
        self.identity.sector_size as u32
    }

    fn num_blocks(&self) -> u64 {
        if self.identity.supports_lba48 {
            self.identity.sectors_48
        } else {
            self.identity.sectors_28 as u64
        }
    }

    fn read_blocks(&self, lba: u64, count: u32, out: &mut [u8]) -> KResult<()> {
        // Split large reads into chunks of 256 sectors max
        let sector_size = self.block_size() as usize;
        let mut remaining = count;
        let mut current_lba = lba;
        let mut offset = 0;

        while remaining > 0 {
            let chunk = remaining.min(255) as u8;
            let chunk_len = chunk as usize * sector_size;
            self.read_sectors(current_lba, chunk, &mut out[offset..offset + chunk_len])?;
            remaining -= chunk as u32;
            current_lba += chunk as u64;
            offset += chunk_len;
        }

        Ok(())
    }

    fn write_blocks(&self, lba: u64, count: u32, data: &[u8]) -> KResult<()> {
        // Split large writes into chunks of 256 sectors max
        let sector_size = self.block_size() as usize;
        let mut remaining = count;
        let mut current_lba = lba;
        let mut offset = 0;

        while remaining > 0 {
            let chunk = remaining.min(255) as u8;
            let chunk_len = chunk as usize * sector_size;
            self.write_sectors(current_lba, chunk, &data[offset..offset + chunk_len])?;
            remaining -= chunk as u32;
            current_lba += chunk as u64;
            offset += chunk_len;
        }

        Ok(())
    }
}

// ============================================================================
// IDE Controller (manages both channels)
// ============================================================================

/// IDE Controller managing primary and secondary channels
pub struct IdeController {
    devices: Vec<Arc<IdeDevice>>,
    initialized: AtomicBool,
}

impl IdeController {
    const fn new() -> Self {
        Self {
            devices: Vec::new(),
            initialized: AtomicBool::new(false),
        }
    }

    /// Detect and identify a device on a channel
    fn identify_device(channel: IdeChannel, role: IdeRole, device_id: u32) -> Option<IdeDevice> {
        let mut ports = IdeChannelPorts::new(channel);

        unsafe {
            // Disable interrupts
            ports.disable_interrupts();

            // Software reset
            ports.software_reset();

            // Select the drive
            ports.select_drive(role);

            // Clear registers
            ports.sector_count.write(0);
            ports.lba_lo.write(0);
            ports.lba_mid.write(0);
            ports.lba_hi.write(0);

            // Send IDENTIFY command
            ports.command.write(CMD_IDENTIFY);

            // Check if device exists
            let status = ports.read_alt_status();
            if status == 0 || status == 0xFF {
                return None; // No device
            }

            // Wait for BSY to clear
            if !ports.wait_not_busy(TIMEOUT_IDENTIFY) {
                return None;
            }

            // Check LBA_MID and LBA_HI to detect device type
            let lba_mid = ports.lba_mid.read();
            let lba_hi = ports.lba_hi.read();

            let device_type = if lba_mid == 0 && lba_hi == 0 {
                IdeDeviceType::Ata
            } else if lba_mid == 0x14 && lba_hi == 0xEB {
                IdeDeviceType::Atapi
            } else if lba_mid == 0x69 && lba_hi == 0x96 {
                IdeDeviceType::Atapi
            } else if lba_mid == 0x3C && lba_hi == 0xC3 {
                // SATA device in IDE compatibility mode
                IdeDeviceType::Ata
            } else {
                IdeDeviceType::Unknown
            };

            // For ATAPI, send IDENTIFY PACKET command instead
            if device_type == IdeDeviceType::Atapi {
                ports.command.write(CMD_IDENTIFY_PACKET);
                if !ports.wait_not_busy(TIMEOUT_IDENTIFY) {
                    return None;
                }
            }

            // Wait for DRQ
            if ports.wait_drq_or_error(TIMEOUT_IDENTIFY).is_err() {
                return None;
            }

            // Read identify data (256 words = 512 bytes)
            let mut identify_data = [0u16; 256];
            for word in identify_data.iter_mut() {
                *word = ports.data.read();
            }

            let identity = IdeIdentity::from_identify_data(&identify_data, device_type);

            // Skip devices with no sectors
            if identity.sectors_28 == 0 && identity.sectors_48 == 0 && device_type == IdeDeviceType::Ata {
                return None;
            }

            Some(IdeDevice::new(channel, role, identity, BlockDeviceId(device_id)))
        }
    }

    /// Initialize the IDE controller and detect all devices
    fn init(&mut self) {
        if self.initialized.swap(true, Ordering::SeqCst) {
            return;
        }

        crate::kprintln!("ide: scanning for devices...");

        let mut device_id = 200u32; // Start device IDs at 200 for IDE

        // Scan primary channel
        for role in [IdeRole::Master, IdeRole::Slave] {
            if let Some(device) = Self::identify_device(IdeChannel::Primary, role, device_id) {
                crate::kprintln!(
                    "ide: primary {:?}: {} ({} sectors, {})",
                    role,
                    device.identity.model,
                    device.num_blocks(),
                    if device.identity.supports_lba48 { "LBA48" } else { "LBA28" }
                );
                self.devices.push(Arc::new(device));
                device_id += 1;
            }
        }

        // Scan secondary channel
        for role in [IdeRole::Master, IdeRole::Slave] {
            if let Some(device) = Self::identify_device(IdeChannel::Secondary, role, device_id) {
                crate::kprintln!(
                    "ide: secondary {:?}: {} ({} sectors, {})",
                    role,
                    device.identity.model,
                    device.num_blocks(),
                    if device.identity.supports_lba48 { "LBA48" } else { "LBA28" }
                );
                self.devices.push(Arc::new(device));
                device_id += 1;
            }
        }

        if self.devices.is_empty() {
            crate::kprintln!("ide: no devices found");
        } else {
            crate::kprintln!("ide: found {} device(s)", self.devices.len());
        }
    }

    /// Get all detected devices
    fn devices(&self) -> &[Arc<IdeDevice>] {
        &self.devices
    }

    /// Get the first ATA device (for root block)
    fn first_ata_device(&self) -> Option<Arc<IdeDevice>> {
        self.devices.iter()
            .find(|d| d.identity.device_type == IdeDeviceType::Ata)
            .cloned()
    }
}

// ============================================================================
// Global State
// ============================================================================

static IDE_CONTROLLER: Mutex<IdeController> = Mutex::new(IdeController::new());

/// Probe for IDE controller via PCI
pub fn probe(pci_device: &PciDevice) -> Option<()> {
    if pci_device.class.class_code == IDE_CLASS && pci_device.class.subclass == IDE_SUBCLASS {
        crate::kprintln!(
            "ide: found IDE controller @ {:02x}:{:02x}.{}",
            pci_device.addr.bus,
            pci_device.addr.device,
            pci_device.addr.function
        );
        Some(())
    } else {
        None
    }
}

/// Initialize the IDE subsystem
pub fn init() {
    IDE_CONTROLLER.lock().init();
}

/// Initialize from PCI device (compatibility with storage init pattern)
pub fn init_from_pci(_probe_result: ()) -> KResult<Arc<dyn BlockDevice>> {
    let mut controller = IDE_CONTROLLER.lock();
    controller.init();

    controller.first_ata_device()
        .map(|d| d as Arc<dyn BlockDevice>)
        .ok_or(KError::NotFound)
}

/// Get all IDE devices
pub fn devices() -> Vec<Arc<IdeDevice>> {
    IDE_CONTROLLER.lock().devices().to_vec()
}

/// Get the first ATA device as a block device
pub fn first_device() -> Option<Arc<dyn BlockDevice>> {
    IDE_CONTROLLER.lock().first_ata_device().map(|d| d as Arc<dyn BlockDevice>)
}

/// Get device by channel and role
pub fn get_device(channel: IdeChannel, role: IdeRole) -> Option<Arc<IdeDevice>> {
    IDE_CONTROLLER.lock().devices().iter()
        .find(|d| d.channel == channel && d.role == role)
        .cloned()
}

/// Print device information
pub fn print_devices() {
    let controller = IDE_CONTROLLER.lock();
    for device in controller.devices() {
        crate::kprintln!(
            "  {:?} {:?}: {} [{}]",
            device.channel,
            device.role,
            device.identity.model,
            match device.identity.device_type {
                IdeDeviceType::Ata => "ATA",
                IdeDeviceType::Atapi => "ATAPI",
                IdeDeviceType::Unknown => "Unknown",
            }
        );
        crate::kprintln!(
            "    Serial: {}, Firmware: {}",
            device.identity.serial,
            device.identity.firmware
        );
        crate::kprintln!(
            "    Sectors: {}, Sector size: {} bytes",
            device.num_blocks(),
            device.block_size()
        );
        crate::kprintln!(
            "    LBA48: {}, Total capacity: {} MB",
            device.identity.supports_lba48,
            device.num_blocks() * device.block_size() as u64 / (1024 * 1024)
        );
    }
}
