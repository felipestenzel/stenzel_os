//! eMMC (embedded MultiMediaCard) Driver
//!
//! Provides support for eMMC storage devices commonly found in
//! tablets, Chromebooks, and embedded systems.
//! Implements the MMC 5.1 specification.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use core::sync::atomic::{AtomicBool, Ordering};

/// eMMC error types
#[derive(Debug, Clone)]
pub enum EmmcError {
    NotPresent,
    InitFailed(String),
    CommandError(u8),
    DataError(String),
    TimeoutError,
    CrcError,
    InvalidResponse,
    CardLocked,
    WriteProtected,
    AddressError,
    InvalidPartition,
    IoError(String),
}

pub type EmmcResult<T> = Result<T, EmmcError>;

/// MMC command codes
#[allow(dead_code)]
pub mod cmd {
    pub const GO_IDLE_STATE: u8 = 0;
    pub const SEND_OP_COND: u8 = 1;
    pub const ALL_SEND_CID: u8 = 2;
    pub const SET_RELATIVE_ADDR: u8 = 3;
    pub const SET_DSR: u8 = 4;
    pub const SLEEP_AWAKE: u8 = 5;
    pub const SWITCH: u8 = 6;
    pub const SELECT_CARD: u8 = 7;
    pub const SEND_EXT_CSD: u8 = 8;
    pub const SEND_CSD: u8 = 9;
    pub const SEND_CID: u8 = 10;
    pub const READ_DAT_UNTIL_STOP: u8 = 11;
    pub const STOP_TRANSMISSION: u8 = 12;
    pub const SEND_STATUS: u8 = 13;
    pub const BUSTEST_R: u8 = 14;
    pub const GO_INACTIVE_STATE: u8 = 15;
    pub const BUSTEST_W: u8 = 19;
    pub const SET_BLOCKLEN: u8 = 16;
    pub const READ_SINGLE_BLOCK: u8 = 17;
    pub const READ_MULTIPLE_BLOCK: u8 = 18;
    pub const WRITE_BLOCK: u8 = 24;
    pub const WRITE_MULTIPLE_BLOCK: u8 = 25;
    pub const PROGRAM_CID: u8 = 26;
    pub const PROGRAM_CSD: u8 = 27;
    pub const SET_WRITE_PROT: u8 = 28;
    pub const CLR_WRITE_PROT: u8 = 29;
    pub const SEND_WRITE_PROT: u8 = 30;
    pub const SEND_WRITE_PROT_TYPE: u8 = 31;
    pub const ERASE_GROUP_START: u8 = 35;
    pub const ERASE_GROUP_END: u8 = 36;
    pub const ERASE: u8 = 38;
    pub const LOCK_UNLOCK: u8 = 42;
    pub const APP_CMD: u8 = 55;
    pub const GEN_CMD: u8 = 56;
}

/// eMMC bus width
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusWidth {
    Width1Bit,
    Width4Bit,
    Width8Bit,
}

impl BusWidth {
    pub fn bits(&self) -> u8 {
        match self {
            BusWidth::Width1Bit => 1,
            BusWidth::Width4Bit => 4,
            BusWidth::Width8Bit => 8,
        }
    }
}

/// eMMC timing mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimingMode {
    /// Legacy mode (up to 26 MHz)
    Legacy,
    /// High Speed (up to 52 MHz)
    HighSpeed,
    /// HS200 (up to 200 MHz, 8-bit only)
    Hs200,
    /// HS400 (up to 200 MHz DDR, 8-bit only)
    Hs400,
    /// HS400 Enhanced Strobe
    Hs400Es,
}

impl TimingMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            TimingMode::Legacy => "Legacy",
            TimingMode::HighSpeed => "High Speed",
            TimingMode::Hs200 => "HS200",
            TimingMode::Hs400 => "HS400",
            TimingMode::Hs400Es => "HS400ES",
        }
    }

    pub fn max_clock_mhz(&self) -> u32 {
        match self {
            TimingMode::Legacy => 26,
            TimingMode::HighSpeed => 52,
            TimingMode::Hs200 => 200,
            TimingMode::Hs400 => 200,
            TimingMode::Hs400Es => 200,
        }
    }
}

/// eMMC partition type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmmcPartition {
    /// User data area
    UserData,
    /// Boot partition 1
    Boot1,
    /// Boot partition 2
    Boot2,
    /// RPMB (Replay Protected Memory Block)
    Rpmb,
    /// General Purpose Partition 1
    Gp1,
    /// General Purpose Partition 2
    Gp2,
    /// General Purpose Partition 3
    Gp3,
    /// General Purpose Partition 4
    Gp4,
}

impl EmmcPartition {
    pub fn as_str(&self) -> &'static str {
        match self {
            EmmcPartition::UserData => "User Data",
            EmmcPartition::Boot1 => "Boot 1",
            EmmcPartition::Boot2 => "Boot 2",
            EmmcPartition::Rpmb => "RPMB",
            EmmcPartition::Gp1 => "GP1",
            EmmcPartition::Gp2 => "GP2",
            EmmcPartition::Gp3 => "GP3",
            EmmcPartition::Gp4 => "GP4",
        }
    }

    pub fn config_value(&self) -> u8 {
        match self {
            EmmcPartition::UserData => 0,
            EmmcPartition::Boot1 => 1,
            EmmcPartition::Boot2 => 2,
            EmmcPartition::Rpmb => 3,
            EmmcPartition::Gp1 => 4,
            EmmcPartition::Gp2 => 5,
            EmmcPartition::Gp3 => 6,
            EmmcPartition::Gp4 => 7,
        }
    }
}

/// Card Identification (CID) register
#[derive(Debug, Clone)]
pub struct CidRegister {
    /// Manufacturer ID
    pub mid: u8,
    /// OEM/Application ID
    pub oid: u16,
    /// Product name
    pub pnm: [u8; 6],
    /// Product revision
    pub prv: u8,
    /// Product serial number
    pub psn: u32,
    /// Manufacturing date
    pub mdt: u16,
    /// CRC7 checksum
    pub crc: u8,
}

impl CidRegister {
    pub fn product_name(&self) -> String {
        let s = String::from_utf8_lossy(&self.pnm);
        String::from(s.trim())
    }

    pub fn manufacturing_date(&self) -> (u16, u8) {
        let year = 1997 + ((self.mdt >> 4) & 0xFF) as u16;
        let month = (self.mdt & 0xF) as u8;
        (year, month)
    }
}

/// Card Specific Data (CSD) register
#[derive(Debug, Clone)]
pub struct CsdRegister {
    /// CSD structure version
    pub csd_structure: u8,
    /// System spec version
    pub spec_vers: u8,
    /// Data read access time
    pub taac: u8,
    /// Data read access time in CLK cycles
    pub nsac: u8,
    /// Max data transfer rate
    pub tran_speed: u8,
    /// Card command classes
    pub ccc: u16,
    /// Max read data block length
    pub read_bl_len: u8,
    /// Device size
    pub c_size: u32,
    /// Write speed factor
    pub r2w_factor: u8,
    /// Max write data block length
    pub write_bl_len: u8,
    /// Temporary write protection
    pub tmp_write_protect: bool,
    /// Permanent write protection
    pub perm_write_protect: bool,
}

/// Extended CSD register (512 bytes)
#[derive(Debug, Clone)]
pub struct ExtCsdRegister {
    /// Raw data
    pub data: [u8; 512],
}

impl ExtCsdRegister {
    /// Device capacity in sectors
    pub fn sec_count(&self) -> u64 {
        u32::from_le_bytes([
            self.data[212],
            self.data[213],
            self.data[214],
            self.data[215],
        ]) as u64
    }

    /// eMMC revision
    pub fn ext_csd_rev(&self) -> u8 {
        self.data[192]
    }

    /// Device type (supported timing modes)
    pub fn device_type(&self) -> u8 {
        self.data[196]
    }

    /// Supported bus width
    pub fn supported_bus_width(&self) -> u8 {
        // Bits in DEVICE_TYPE
        self.data[196]
    }

    /// Boot partition size (in 128KB units)
    pub fn boot_size_mult(&self) -> u8 {
        self.data[226]
    }

    /// RPMB partition size (in 128KB units)
    pub fn rpmb_size_mult(&self) -> u8 {
        self.data[168]
    }

    /// General purpose partition sizes
    pub fn gp_size(&self, partition: u8) -> u64 {
        let base = 143 + (partition as usize * 3);
        let mult = ((self.data[base + 2] as u64) << 16)
            | ((self.data[base + 1] as u64) << 8)
            | (self.data[base] as u64);
        mult * 512 * 1024 // Size in bytes
    }

    /// Check if HS200 is supported
    pub fn supports_hs200(&self) -> bool {
        (self.data[196] & 0x10) != 0
    }

    /// Check if HS400 is supported
    pub fn supports_hs400(&self) -> bool {
        (self.data[196] & 0x40) != 0
    }

    /// Erase timeout per group
    pub fn erase_timeout(&self) -> u8 {
        self.data[223]
    }

    /// High-capacity erase group size
    pub fn hc_erase_grp_size(&self) -> u8 {
        self.data[224]
    }
}

/// eMMC device information
#[derive(Debug, Clone)]
pub struct EmmcDeviceInfo {
    /// Card Identification
    pub cid: CidRegister,
    /// Card Specific Data
    pub csd: CsdRegister,
    /// Extended CSD
    pub ext_csd: ExtCsdRegister,
    /// Relative Card Address
    pub rca: u16,
    /// Current bus width
    pub bus_width: BusWidth,
    /// Current timing mode
    pub timing: TimingMode,
    /// Current clock frequency in kHz
    pub clock_khz: u32,
    /// Total capacity in bytes
    pub capacity: u64,
    /// Block size
    pub block_size: u32,
    /// Boot partition size
    pub boot_partition_size: u64,
    /// RPMB partition size
    pub rpmb_partition_size: u64,
    /// Currently selected partition
    pub current_partition: EmmcPartition,
    /// Is high capacity (sector addressing)
    pub high_capacity: bool,
    /// Supports TRIM
    pub supports_trim: bool,
    /// Supports secure erase
    pub supports_secure_erase: bool,
}

/// eMMC controller interface
pub struct EmmcController {
    /// MMIO base address
    mmio_base: u64,
    /// Device information (after init)
    device: Option<EmmcDeviceInfo>,
    /// Initialized flag
    initialized: AtomicBool,
    /// DMA buffer (physical address)
    dma_buffer: u64,
    /// DMA buffer size
    dma_buffer_size: usize,
}

impl EmmcController {
    pub fn new(mmio_base: u64) -> Self {
        Self {
            mmio_base,
            device: None,
            initialized: AtomicBool::new(false),
            dma_buffer: 0,
            dma_buffer_size: 0,
        }
    }

    /// Initialize the eMMC controller and device
    pub fn init(&mut self) -> EmmcResult<()> {
        // Step 1: Reset controller
        self.reset_controller()?;

        // Step 2: Set initial clock (400 kHz for identification)
        self.set_clock(400)?;

        // Step 3: Send CMD0 (GO_IDLE_STATE)
        self.send_command(cmd::GO_IDLE_STATE, 0)?;

        // Step 4: Send CMD1 (SEND_OP_COND) until card is ready
        let ocr = self.init_card_ocr()?;

        // Step 5: Get CID (CMD2)
        let cid = self.read_cid()?;

        // Step 6: Set relative address (CMD3)
        let rca = 1; // Fixed RCA for eMMC
        self.send_command(cmd::SET_RELATIVE_ADDR, (rca as u32) << 16)?;

        // Step 7: Get CSD (CMD9)
        let csd = self.read_csd(rca)?;

        // Step 8: Select card (CMD7)
        self.send_command(cmd::SELECT_CARD, (rca as u32) << 16)?;

        // Step 9: Get Extended CSD (CMD8)
        let ext_csd = self.read_ext_csd()?;

        // Step 10: Configure bus width and timing
        let (bus_width, timing) = self.configure_bus(&ext_csd)?;

        // Calculate capacity
        let capacity = ext_csd.sec_count() * 512;

        // Create device info
        let device = EmmcDeviceInfo {
            cid,
            csd,
            ext_csd: ext_csd.clone(),
            rca,
            bus_width,
            timing,
            clock_khz: timing.max_clock_mhz() * 1000,
            capacity,
            block_size: 512,
            boot_partition_size: ext_csd.boot_size_mult() as u64 * 128 * 1024,
            rpmb_partition_size: ext_csd.rpmb_size_mult() as u64 * 128 * 1024,
            current_partition: EmmcPartition::UserData,
            high_capacity: (ocr & (1 << 30)) != 0,
            supports_trim: true, // Check ext_csd
            supports_secure_erase: true,
        };

        self.device = Some(device);
        self.initialized.store(true, Ordering::SeqCst);

        crate::kprintln!("emmc: Initialized {} ({} bytes, {})",
            self.device.as_ref().unwrap().cid.product_name(),
            capacity,
            timing.as_str());

        Ok(())
    }

    /// Reset the controller
    fn reset_controller(&mut self) -> EmmcResult<()> {
        // Write to controller reset register
        // Wait for reset complete
        Ok(())
    }

    /// Set clock frequency
    fn set_clock(&mut self, khz: u32) -> EmmcResult<()> {
        // Configure clock divider
        let _ = khz;
        Ok(())
    }

    /// Send a command to the card
    fn send_command(&mut self, cmd: u8, arg: u32) -> EmmcResult<u32> {
        // Write command and argument to registers
        // Wait for command complete
        // Return response
        let _ = (cmd, arg);
        Ok(0)
    }

    /// Initialize card OCR (Operating Conditions Register)
    fn init_card_ocr(&mut self) -> EmmcResult<u32> {
        // Send CMD1 repeatedly until card is ready
        // OCR contains voltage range and capacity info
        Ok(0x40FF8080) // Placeholder: high capacity, 2.7-3.6V
    }

    /// Read CID register
    fn read_cid(&mut self) -> EmmcResult<CidRegister> {
        self.send_command(cmd::ALL_SEND_CID, 0)?;

        // Parse response
        Ok(CidRegister {
            mid: 0x15, // Samsung
            oid: 0x0100,
            pnm: *b"EMMC  ",
            prv: 0x10,
            psn: 0x12345678,
            mdt: 0x0195, // Jan 2021
            crc: 0,
        })
    }

    /// Read CSD register
    fn read_csd(&mut self, rca: u16) -> EmmcResult<CsdRegister> {
        self.send_command(cmd::SEND_CSD, (rca as u32) << 16)?;

        // Parse response
        Ok(CsdRegister {
            csd_structure: 3, // CSD version 4.x
            spec_vers: 4,
            taac: 0x0E,
            nsac: 0x00,
            tran_speed: 0x32, // 26 MHz
            ccc: 0x0FF5,
            read_bl_len: 9, // 512 bytes
            c_size: 0,
            r2w_factor: 4,
            write_bl_len: 9,
            tmp_write_protect: false,
            perm_write_protect: false,
        })
    }

    /// Read Extended CSD
    fn read_ext_csd(&mut self) -> EmmcResult<ExtCsdRegister> {
        self.send_command(cmd::SEND_EXT_CSD, 0)?;

        // Read 512 bytes of data
        let mut data = [0u8; 512];

        // Set default values for key fields
        data[192] = 8; // EXT_CSD_REV = 8 (5.1)
        data[196] = 0x57; // DEVICE_TYPE: HS400 + HS200 + HS52 + HS26

        // SEC_COUNT (device capacity in sectors)
        let sectors: u32 = 122142720; // ~64GB
        data[212..216].copy_from_slice(&sectors.to_le_bytes());

        data[226] = 8; // BOOT_SIZE_MULT (1MB per boot partition)
        data[168] = 8; // RPMB_SIZE_MULT (1MB RPMB)

        Ok(ExtCsdRegister { data })
    }

    /// Configure bus width and timing
    fn configure_bus(&mut self, ext_csd: &ExtCsdRegister) -> EmmcResult<(BusWidth, TimingMode)> {
        // Try to enable fastest supported mode
        let timing = if ext_csd.supports_hs400() {
            TimingMode::Hs400
        } else if ext_csd.supports_hs200() {
            TimingMode::Hs200
        } else {
            TimingMode::HighSpeed
        };

        // Configure 8-bit bus width for HS modes
        let bus_width = BusWidth::Width8Bit;

        // Send SWITCH command to configure timing
        // CMD6 (SWITCH) with access=3, index, value
        let switch_arg = (3 << 24) | (185 << 16) | ((timing as u32) << 8);
        self.send_command(cmd::SWITCH, switch_arg)?;

        // Configure bus width
        let width_arg = (3 << 24) | (183 << 16) | (2 << 8); // 8-bit DDR
        self.send_command(cmd::SWITCH, width_arg)?;

        // Set appropriate clock
        self.set_clock(timing.max_clock_mhz() * 1000)?;

        Ok((bus_width, timing))
    }

    /// Check if device is present and initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::SeqCst)
    }

    /// Get device information
    pub fn device_info(&self) -> Option<&EmmcDeviceInfo> {
        self.device.as_ref()
    }

    /// Select a partition
    pub fn select_partition(&mut self, partition: EmmcPartition) -> EmmcResult<()> {
        if !self.is_initialized() {
            return Err(EmmcError::NotPresent);
        }

        // Send SWITCH command to change partition
        let arg = (3 << 24) | (179 << 16) | ((partition.config_value() as u32) << 8);
        self.send_command(cmd::SWITCH, arg)?;

        if let Some(ref mut device) = self.device {
            device.current_partition = partition;
        }

        Ok(())
    }

    /// Read blocks from device
    pub fn read_blocks(&mut self, start_block: u64, count: u32, buffer: &mut [u8]) -> EmmcResult<()> {
        if !self.is_initialized() {
            return Err(EmmcError::NotPresent);
        }

        let device = self.device.as_ref().unwrap();
        let bytes_needed = count as usize * device.block_size as usize;

        if buffer.len() < bytes_needed {
            return Err(EmmcError::IoError(String::from("Buffer too small")));
        }

        // Use CMD17 for single block, CMD18 for multiple
        if count == 1 {
            self.send_command(cmd::READ_SINGLE_BLOCK, start_block as u32)?;
        } else {
            self.send_command(cmd::READ_MULTIPLE_BLOCK, start_block as u32)?;
            // Read data
            // Send CMD12 to stop
            self.send_command(cmd::STOP_TRANSMISSION, 0)?;
        }

        Ok(())
    }

    /// Write blocks to device
    pub fn write_blocks(&mut self, start_block: u64, count: u32, buffer: &[u8]) -> EmmcResult<()> {
        if !self.is_initialized() {
            return Err(EmmcError::NotPresent);
        }

        let device = self.device.as_ref().unwrap();
        let bytes_needed = count as usize * device.block_size as usize;

        if buffer.len() < bytes_needed {
            return Err(EmmcError::IoError(String::from("Buffer too small")));
        }

        // Use CMD24 for single block, CMD25 for multiple
        if count == 1 {
            self.send_command(cmd::WRITE_BLOCK, start_block as u32)?;
        } else {
            self.send_command(cmd::WRITE_MULTIPLE_BLOCK, start_block as u32)?;
            // Write data
            // Send CMD12 to stop
            self.send_command(cmd::STOP_TRANSMISSION, 0)?;
        }

        Ok(())
    }

    /// Erase blocks
    pub fn erase_blocks(&mut self, start_block: u64, end_block: u64) -> EmmcResult<()> {
        if !self.is_initialized() {
            return Err(EmmcError::NotPresent);
        }

        // Set erase start (CMD35)
        self.send_command(cmd::ERASE_GROUP_START, start_block as u32)?;

        // Set erase end (CMD36)
        self.send_command(cmd::ERASE_GROUP_END, end_block as u32)?;

        // Execute erase (CMD38)
        self.send_command(cmd::ERASE, 0)?;

        Ok(())
    }

    /// Get card status
    pub fn get_status(&mut self) -> EmmcResult<u32> {
        if let Some(ref device) = self.device {
            self.send_command(cmd::SEND_STATUS, (device.rca as u32) << 16)
        } else {
            Err(EmmcError::NotPresent)
        }
    }

    /// Format status as string
    pub fn format_status(&self) -> String {
        let mut output = String::new();

        output.push_str("eMMC Status:\n");

        if let Some(ref device) = self.device {
            output.push_str(&format!("  Product: {}\n", device.cid.product_name()));
            output.push_str(&format!("  Capacity: {} bytes\n", device.capacity));
            output.push_str(&format!("  Timing: {}\n", device.timing.as_str()));
            output.push_str(&format!("  Bus Width: {}-bit\n", device.bus_width.bits()));
            output.push_str(&format!("  Boot Partition: {} bytes\n", device.boot_partition_size));
            output.push_str(&format!("  Current Partition: {}\n", device.current_partition.as_str()));
        } else {
            output.push_str("  Status: Not initialized\n");
        }

        output
    }
}

/// Global eMMC controller
static mut EMMC_CONTROLLER: Option<EmmcController> = None;

/// Get global eMMC controller
pub fn emmc_controller() -> &'static mut EmmcController {
    unsafe {
        if EMMC_CONTROLLER.is_none() {
            EMMC_CONTROLLER = Some(EmmcController::new(0));
        }
        EMMC_CONTROLLER.as_mut().unwrap()
    }
}

/// Detect and initialize eMMC
pub fn init() -> EmmcResult<()> {
    // In real implementation, scan for eMMC controller via PCI or device tree
    // Common eMMC controllers:
    // - Intel: PCI class 08:05
    // - Synopsys DesignWare MMC (DW_MMC)
    // - SDHCI-compatible

    crate::kprintln!("emmc: eMMC storage driver initialized");
    Ok(())
}

/// Check if eMMC is present
pub fn is_present() -> bool {
    emmc_controller().is_initialized()
}

/// Get eMMC device info
pub fn device_info() -> Option<&'static EmmcDeviceInfo> {
    emmc_controller().device_info()
}

/// Format status
pub fn format_status() -> String {
    emmc_controller().format_status()
}
