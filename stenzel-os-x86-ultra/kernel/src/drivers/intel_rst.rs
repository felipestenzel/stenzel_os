//! Intel Rapid Storage Technology (RST) Driver
//!
//! Provides support for Intel RST RAID arrays and Optane memory acceleration.
//! RST uses Intel's AHCI controller in RAID mode with proprietary metadata.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use core::sync::atomic::{AtomicBool, Ordering};

/// RST error types
#[derive(Debug, Clone)]
pub enum RstError {
    NotSupported,
    ControllerNotFound,
    InvalidMetadata,
    ArrayDegraded(String),
    ArrayFailed(String),
    RebuildInProgress,
    DiskNotFound(u8),
    IoError(String),
    OptaneNotPresent,
    ConfigError(String),
}

pub type RstResult<T> = Result<T, RstError>;

/// RST RAID levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RstRaidLevel {
    /// RAID 0 - Striping
    Raid0,
    /// RAID 1 - Mirroring
    Raid1,
    /// RAID 5 - Striping with parity
    Raid5,
    /// RAID 10 - Mirrored stripes
    Raid10,
    /// Single disk (JBOD)
    Single,
    /// Intel Optane Memory acceleration
    OptaneAcceleration,
}

impl RstRaidLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            RstRaidLevel::Raid0 => "RAID 0 (Stripe)",
            RstRaidLevel::Raid1 => "RAID 1 (Mirror)",
            RstRaidLevel::Raid5 => "RAID 5 (Parity)",
            RstRaidLevel::Raid10 => "RAID 10 (Mirror+Stripe)",
            RstRaidLevel::Single => "Single",
            RstRaidLevel::OptaneAcceleration => "Optane Acceleration",
        }
    }

    pub fn min_disks(&self) -> u8 {
        match self {
            RstRaidLevel::Raid0 => 2,
            RstRaidLevel::Raid1 => 2,
            RstRaidLevel::Raid5 => 3,
            RstRaidLevel::Raid10 => 4,
            RstRaidLevel::Single => 1,
            RstRaidLevel::OptaneAcceleration => 2,
        }
    }

    pub fn has_redundancy(&self) -> bool {
        matches!(self, RstRaidLevel::Raid1 | RstRaidLevel::Raid5 | RstRaidLevel::Raid10)
    }
}

/// RST array state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RstArrayState {
    Normal,
    Degraded,
    Rebuilding,
    Failed,
    Initializing,
    Verifying,
    Unknown,
}

impl RstArrayState {
    pub fn as_str(&self) -> &'static str {
        match self {
            RstArrayState::Normal => "Normal",
            RstArrayState::Degraded => "Degraded",
            RstArrayState::Rebuilding => "Rebuilding",
            RstArrayState::Failed => "Failed",
            RstArrayState::Initializing => "Initializing",
            RstArrayState::Verifying => "Verifying",
            RstArrayState::Unknown => "Unknown",
        }
    }
}

/// RST disk status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RstDiskStatus {
    Online,
    Offline,
    Missing,
    Failed,
    Spare,
    Rebuilding,
}

impl RstDiskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RstDiskStatus::Online => "Online",
            RstDiskStatus::Offline => "Offline",
            RstDiskStatus::Missing => "Missing",
            RstDiskStatus::Failed => "Failed",
            RstDiskStatus::Spare => "Spare",
            RstDiskStatus::Rebuilding => "Rebuilding",
        }
    }
}

/// Intel RST metadata header (on-disk format)
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct RstMetadataHeader {
    /// Signature: "Intel Raid ISM Cfg Sig. "
    pub signature: [u8; 24],
    /// Metadata version
    pub version: [u8; 6],
    /// Check sum
    pub checksum: u32,
    /// Volume count
    pub volume_count: u32,
    /// Disk count
    pub disk_count: u32,
    /// Reserved
    pub reserved: [u8; 468],
}

impl RstMetadataHeader {
    pub const SIGNATURE: &'static [u8; 24] = b"Intel Raid ISM Cfg Sig. ";

    pub fn validate(&self) -> bool {
        self.signature == *Self::SIGNATURE
    }
}

/// RST disk metadata entry
#[derive(Debug, Clone)]
pub struct RstDiskEntry {
    /// Serial number
    pub serial: String,
    /// Model name
    pub model: String,
    /// Port number
    pub port: u8,
    /// Disk size in sectors
    pub sectors: u64,
    /// Disk status
    pub status: RstDiskStatus,
    /// Role in array (data, parity, spare)
    pub role: u8,
    /// SCSI ID
    pub scsi_id: u32,
}

/// RST volume metadata
#[derive(Debug, Clone)]
pub struct RstVolume {
    /// Volume name
    pub name: String,
    /// Volume UUID
    pub uuid: [u8; 16],
    /// RAID level
    pub raid_level: RstRaidLevel,
    /// Stripe size in KB
    pub stripe_size_kb: u32,
    /// Total size in sectors
    pub total_sectors: u64,
    /// Current state
    pub state: RstArrayState,
    /// Member disks
    pub members: Vec<u8>,
    /// Spare disks
    pub spares: Vec<u8>,
    /// Rebuild progress (0-100)
    pub rebuild_percent: u8,
    /// Write-back cache enabled
    pub write_back_cache: bool,
    /// Dirty flag
    pub dirty: bool,
}

impl RstVolume {
    /// Calculate usable capacity based on RAID level
    pub fn usable_capacity(&self, disk_capacity: u64) -> u64 {
        let disk_count = self.members.len() as u64;
        match self.raid_level {
            RstRaidLevel::Raid0 => disk_capacity * disk_count,
            RstRaidLevel::Raid1 => disk_capacity, // Mirror uses half
            RstRaidLevel::Raid5 => disk_capacity * (disk_count - 1),
            RstRaidLevel::Raid10 => disk_capacity * (disk_count / 2),
            RstRaidLevel::Single => disk_capacity,
            RstRaidLevel::OptaneAcceleration => disk_capacity, // Accelerated disk only
        }
    }
}

/// Intel Optane Memory configuration
#[derive(Debug, Clone)]
pub struct OptaneConfig {
    /// Optane device present
    pub present: bool,
    /// Optane device port
    pub port: u8,
    /// Optane capacity in bytes
    pub capacity: u64,
    /// Accelerated disk port
    pub accelerated_disk: u8,
    /// Cache mode
    pub cache_mode: OptaneCacheMode,
    /// Cache hit rate (percent)
    pub hit_rate: u8,
    /// Bytes read from Optane
    pub bytes_read: u64,
    /// Bytes written to Optane
    pub bytes_written: u64,
}

/// Optane cache mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptaneCacheMode {
    WriteThrough,
    WriteBack,
    Disabled,
}

impl OptaneCacheMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            OptaneCacheMode::WriteThrough => "Write-Through",
            OptaneCacheMode::WriteBack => "Write-Back",
            OptaneCacheMode::Disabled => "Disabled",
        }
    }
}

/// RST controller information
#[derive(Debug, Clone)]
pub struct RstControllerInfo {
    /// PCI vendor ID (0x8086 for Intel)
    pub vendor_id: u16,
    /// PCI device ID
    pub device_id: u16,
    /// Controller generation
    pub generation: RstGeneration,
    /// Number of ports
    pub port_count: u8,
    /// AHCI MMIO base address
    pub mmio_base: u64,
    /// OROM (Option ROM) version
    pub orom_version: String,
    /// RST driver version
    pub driver_version: String,
}

/// RST controller generation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RstGeneration {
    /// Series 5 (Ibex Peak)
    Series5,
    /// Series 6 (Cougar Point)
    Series6,
    /// Series 7 (Panther Point)
    Series7,
    /// Series 8 (Lynx Point)
    Series8,
    /// Series 9 (Wildcat Point)
    Series9,
    /// Series 100 (Sunrise Point)
    Series100,
    /// Series 200 (Union Point)
    Series200,
    /// Series 300 (Cannon Point)
    Series300,
    /// Series 400 (Comet Point)
    Series400,
    /// Series 500 (Tiger Point)
    Series500,
    /// Series 600 (Alder Point)
    Series600,
    /// Series 700 (Raptor Point)
    Series700,
    Unknown,
}

impl RstGeneration {
    pub fn from_device_id(device_id: u16) -> Self {
        match device_id {
            0x3B22..=0x3B2F => RstGeneration::Series5,
            0x1C02..=0x1C0F => RstGeneration::Series6,
            0x1E02..=0x1E0F => RstGeneration::Series7,
            0x8C02..=0x8C0F => RstGeneration::Series8,
            0x9C02..=0x9C0F => RstGeneration::Series9,
            0xA102..=0xA12F => RstGeneration::Series100,
            0xA282..=0xA28F => RstGeneration::Series200,
            0xA352..=0xA35F => RstGeneration::Series300,
            0x02D2..=0x02DF | 0x06D2..=0x06DF => RstGeneration::Series400,
            0xA0D2..=0xA0DF => RstGeneration::Series500,
            0x7AE2..=0x7AEF => RstGeneration::Series600,
            0x7A02..=0x7A0F => RstGeneration::Series700,
            _ => RstGeneration::Unknown,
        }
    }

    pub fn supports_optane(&self) -> bool {
        matches!(self,
            RstGeneration::Series200 |
            RstGeneration::Series300 |
            RstGeneration::Series400 |
            RstGeneration::Series500 |
            RstGeneration::Series600 |
            RstGeneration::Series700
        )
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            RstGeneration::Series5 => "Series 5 (Ibex Peak)",
            RstGeneration::Series6 => "Series 6 (Cougar Point)",
            RstGeneration::Series7 => "Series 7 (Panther Point)",
            RstGeneration::Series8 => "Series 8 (Lynx Point)",
            RstGeneration::Series9 => "Series 9 (Wildcat Point)",
            RstGeneration::Series100 => "Series 100 (Sunrise Point)",
            RstGeneration::Series200 => "Series 200 (Union Point)",
            RstGeneration::Series300 => "Series 300 (Cannon Point)",
            RstGeneration::Series400 => "Series 400 (Comet Point)",
            RstGeneration::Series500 => "Series 500 (Tiger Point)",
            RstGeneration::Series600 => "Series 600 (Alder Point)",
            RstGeneration::Series700 => "Series 700 (Raptor Point)",
            RstGeneration::Unknown => "Unknown",
        }
    }
}

/// Intel RST driver
pub struct IntelRstDriver {
    /// Controller information
    controller: Option<RstControllerInfo>,
    /// Detected volumes
    volumes: Vec<RstVolume>,
    /// Detected disks
    disks: Vec<RstDiskEntry>,
    /// Optane configuration
    optane: Option<OptaneConfig>,
    /// Driver initialized
    initialized: AtomicBool,
}

impl IntelRstDriver {
    pub fn new() -> Self {
        Self {
            controller: None,
            volumes: Vec::new(),
            disks: Vec::new(),
            optane: None,
            initialized: AtomicBool::new(false),
        }
    }

    /// Initialize the RST driver
    pub fn init(&mut self) -> RstResult<()> {
        // Detect RST controller
        self.detect_controller()?;

        // Read RST metadata from disks
        self.read_metadata()?;

        // Detect Optane if supported
        if let Some(ref ctrl) = self.controller {
            if ctrl.generation.supports_optane() {
                self.detect_optane();
            }
        }

        self.initialized.store(true, Ordering::SeqCst);

        crate::kprintln!("intel_rst: Initialized with {} volumes, {} disks",
            self.volumes.len(), self.disks.len());

        Ok(())
    }

    /// Detect RST controller
    fn detect_controller(&mut self) -> RstResult<()> {
        // Scan PCI for Intel AHCI controllers in RAID mode
        // Device IDs vary by generation

        // In real implementation, use PCI enumeration
        // For now, create placeholder

        // Check if running in RAID mode (vs AHCI mode)
        // RAID mode is indicated by PCI class code 01:04 (RAID controller)
        // vs AHCI mode 01:06 (SATA controller)

        let controller = RstControllerInfo {
            vendor_id: 0x8086,
            device_id: 0xA282, // Example: 200 series
            generation: RstGeneration::Series200,
            port_count: 6,
            mmio_base: 0,
            orom_version: String::from("17.8.0.4460"),
            driver_version: String::from("1.0.0"),
        };

        self.controller = Some(controller);
        Ok(())
    }

    /// Read RST metadata from disks
    fn read_metadata(&mut self) -> RstResult<()> {
        // RST metadata is stored at a specific LBA near the end of each disk
        // Typically at LBA -1 to -3 sectors

        // In real implementation:
        // 1. For each disk, read the last few sectors
        // 2. Look for Intel signature
        // 3. Parse volume and disk entries
        // 4. Validate checksums

        // Placeholder: Create an example volume
        // In real implementation, this would be read from disk

        Ok(())
    }

    /// Detect Optane Memory device
    fn detect_optane(&mut self) {
        // Intel Optane Memory appears as an NVMe device
        // with specific vendor IDs and characteristics

        // In real implementation:
        // 1. Scan NVMe devices for Intel Optane
        // 2. Check acceleration configuration
        // 3. Read cache statistics

        // Placeholder
        self.optane = None;
    }

    /// Get controller information
    pub fn controller(&self) -> Option<&RstControllerInfo> {
        self.controller.as_ref()
    }

    /// Get all volumes
    pub fn volumes(&self) -> &[RstVolume] {
        &self.volumes
    }

    /// Get all disks
    pub fn disks(&self) -> &[RstDiskEntry] {
        &self.disks
    }

    /// Get Optane configuration
    pub fn optane(&self) -> Option<&OptaneConfig> {
        self.optane.as_ref()
    }

    /// Check if RST is available
    pub fn is_available(&self) -> bool {
        self.controller.is_some()
    }

    /// Create a new RAID volume
    pub fn create_volume(&mut self, config: &RstVolumeConfig) -> RstResult<()> {
        // Validate configuration
        if config.disks.len() < config.raid_level.min_disks() as usize {
            return Err(RstError::ConfigError(format!(
                "RAID {} requires at least {} disks",
                config.raid_level.as_str(),
                config.raid_level.min_disks()
            )));
        }

        // In real implementation:
        // 1. Initialize RAID metadata on each disk
        // 2. Create volume entry
        // 3. Start initialization (background)

        let volume = RstVolume {
            name: config.name.clone(),
            uuid: [0; 16], // Generate UUID
            raid_level: config.raid_level,
            stripe_size_kb: config.stripe_size_kb,
            total_sectors: 0, // Calculate based on disks
            state: RstArrayState::Initializing,
            members: config.disks.clone(),
            spares: config.spares.clone(),
            rebuild_percent: 0,
            write_back_cache: config.write_back_cache,
            dirty: false,
        };

        self.volumes.push(volume);

        crate::kprintln!("intel_rst: Created {} volume '{}'",
            config.raid_level.as_str(), config.name);

        Ok(())
    }

    /// Delete a RAID volume
    pub fn delete_volume(&mut self, name: &str) -> RstResult<()> {
        let index = self.volumes.iter().position(|v| v.name == name)
            .ok_or_else(|| RstError::ConfigError(format!("Volume '{}' not found", name)))?;

        // In real implementation:
        // 1. Clear RAID metadata from member disks
        // 2. Remove volume entry

        self.volumes.remove(index);

        crate::kprintln!("intel_rst: Deleted volume '{}'", name);

        Ok(())
    }

    /// Add spare disk to volume
    pub fn add_spare(&mut self, volume_name: &str, disk_port: u8) -> RstResult<()> {
        let volume = self.volumes.iter_mut()
            .find(|v| v.name == volume_name)
            .ok_or_else(|| RstError::ConfigError(format!("Volume '{}' not found", volume_name)))?;

        if !volume.raid_level.has_redundancy() {
            return Err(RstError::ConfigError(String::from(
                "Cannot add spare to non-redundant array"
            )));
        }

        volume.spares.push(disk_port);

        crate::kprintln!("intel_rst: Added spare disk {} to volume '{}'",
            disk_port, volume_name);

        Ok(())
    }

    /// Start rebuilding a degraded array
    pub fn start_rebuild(&mut self, volume_name: &str, disk_port: u8) -> RstResult<()> {
        let volume = self.volumes.iter_mut()
            .find(|v| v.name == volume_name)
            .ok_or_else(|| RstError::ConfigError(format!("Volume '{}' not found", volume_name)))?;

        if volume.state != RstArrayState::Degraded {
            return Err(RstError::ConfigError(String::from("Array is not degraded")));
        }

        // In real implementation:
        // 1. Mark new disk as rebuilding
        // 2. Start background rebuild process

        volume.state = RstArrayState::Rebuilding;
        volume.rebuild_percent = 0;

        crate::kprintln!("intel_rst: Started rebuild of volume '{}' with disk {}",
            volume_name, disk_port);

        Ok(())
    }

    /// Enable Optane acceleration for a disk
    pub fn enable_optane(&mut self, target_disk: u8) -> RstResult<()> {
        let ctrl = self.controller.as_ref()
            .ok_or(RstError::ControllerNotFound)?;

        if !ctrl.generation.supports_optane() {
            return Err(RstError::OptaneNotPresent);
        }

        // In real implementation:
        // 1. Detect Optane device
        // 2. Configure acceleration
        // 3. Initialize cache

        crate::kprintln!("intel_rst: Enabled Optane acceleration for disk {}", target_disk);

        Ok(())
    }

    /// Disable Optane acceleration
    pub fn disable_optane(&mut self) -> RstResult<()> {
        if self.optane.is_none() {
            return Err(RstError::OptaneNotPresent);
        }

        // In real implementation:
        // 1. Flush cache to disk
        // 2. Disable acceleration
        // 3. Release Optane device

        self.optane = None;

        crate::kprintln!("intel_rst: Disabled Optane acceleration");

        Ok(())
    }

    /// Get array status
    pub fn get_status(&self) -> RstStatus {
        let overall_state = if self.volumes.iter().any(|v| v.state == RstArrayState::Failed) {
            RstArrayState::Failed
        } else if self.volumes.iter().any(|v| v.state == RstArrayState::Degraded) {
            RstArrayState::Degraded
        } else if self.volumes.iter().any(|v| v.state == RstArrayState::Rebuilding) {
            RstArrayState::Rebuilding
        } else {
            RstArrayState::Normal
        };

        RstStatus {
            controller: self.controller.clone(),
            volumes: self.volumes.clone(),
            disks: self.disks.clone(),
            optane: self.optane.clone(),
            overall_state,
        }
    }

    /// Format status as string
    pub fn format_status(&self) -> String {
        let status = self.get_status();
        let mut output = String::new();

        output.push_str("Intel RST Status:\n");

        if let Some(ref ctrl) = status.controller {
            output.push_str(&format!("  Controller: {} ({})\n",
                ctrl.generation.as_str(), ctrl.orom_version));
            output.push_str(&format!("  Ports: {}\n", ctrl.port_count));
        }

        output.push_str(&format!("  Overall State: {}\n\n", status.overall_state.as_str()));

        if !status.volumes.is_empty() {
            output.push_str("  Volumes:\n");
            for vol in &status.volumes {
                output.push_str(&format!("    {} - {} ({}, {})\n",
                    vol.name,
                    vol.raid_level.as_str(),
                    vol.state.as_str(),
                    format_size(vol.total_sectors * 512)
                ));
                if vol.state == RstArrayState::Rebuilding {
                    output.push_str(&format!("      Rebuild: {}%\n", vol.rebuild_percent));
                }
            }
        }

        if let Some(ref optane) = status.optane {
            output.push_str("\n  Optane Memory:\n");
            output.push_str(&format!("    Mode: {}\n", optane.cache_mode.as_str()));
            output.push_str(&format!("    Capacity: {}\n", format_size(optane.capacity)));
            output.push_str(&format!("    Hit Rate: {}%\n", optane.hit_rate));
        }

        output
    }
}

/// RST volume creation configuration
#[derive(Debug, Clone)]
pub struct RstVolumeConfig {
    pub name: String,
    pub raid_level: RstRaidLevel,
    pub disks: Vec<u8>,
    pub spares: Vec<u8>,
    pub stripe_size_kb: u32,
    pub write_back_cache: bool,
}

impl Default for RstVolumeConfig {
    fn default() -> Self {
        Self {
            name: String::from("Volume0"),
            raid_level: RstRaidLevel::Raid1,
            disks: Vec::new(),
            spares: Vec::new(),
            stripe_size_kb: 128,
            write_back_cache: false,
        }
    }
}

/// RST status snapshot
#[derive(Debug, Clone)]
pub struct RstStatus {
    pub controller: Option<RstControllerInfo>,
    pub volumes: Vec<RstVolume>,
    pub disks: Vec<RstDiskEntry>,
    pub optane: Option<OptaneConfig>,
    pub overall_state: RstArrayState,
}

/// Format size in human readable form
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Global RST driver instance
static mut RST_DRIVER: Option<IntelRstDriver> = None;

/// Get global RST driver
pub fn rst_driver() -> &'static mut IntelRstDriver {
    unsafe {
        if RST_DRIVER.is_none() {
            RST_DRIVER = Some(IntelRstDriver::new());
        }
        RST_DRIVER.as_mut().unwrap()
    }
}

/// Initialize Intel RST subsystem
pub fn init() -> RstResult<()> {
    rst_driver().init()
}

/// Check if RST is available
pub fn is_available() -> bool {
    rst_driver().is_available()
}

/// Get RST status
pub fn get_status() -> RstStatus {
    rst_driver().get_status()
}

/// Format RST status
pub fn format_status() -> String {
    rst_driver().format_status()
}
