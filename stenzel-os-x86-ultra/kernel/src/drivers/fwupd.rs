//! Firmware Update Daemon (fwupd) Support
//!
//! Provides firmware update capabilities compatible with Linux fwupd interface.
//! Supports LVFS (Linux Vendor Firmware Service), UEFI capsules, and vendor-specific updates.

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use alloc::collections::BTreeMap;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use crate::sync::TicketSpinlock;

/// Firmware update status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FwupdStatus {
    /// Unknown status
    Unknown,
    /// Idle, waiting for work
    Idle,
    /// Loading firmware image
    Loading,
    /// Decompressing firmware
    Decompressing,
    /// Verifying firmware signature
    Verifying,
    /// Scheduling update for next boot
    Scheduling,
    /// Update needs user action
    NeedsReboot,
    /// Downloading firmware
    Downloading,
    /// Writing firmware
    Writing,
    /// Update complete
    Complete,
    /// Update failed
    Failed,
}

impl FwupdStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Idle => "idle",
            Self::Loading => "loading",
            Self::Decompressing => "decompressing",
            Self::Verifying => "verifying",
            Self::Scheduling => "scheduling",
            Self::NeedsReboot => "needs-reboot",
            Self::Downloading => "downloading",
            Self::Writing => "writing",
            Self::Complete => "complete",
            Self::Failed => "failed",
        }
    }
}

/// Firmware device flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceFlags(pub u64);

impl DeviceFlags {
    /// No special flags
    pub const NONE: Self = Self(0);
    /// Device is internal
    pub const INTERNAL: Self = Self(1 << 0);
    /// Device is updatable
    pub const UPDATABLE: Self = Self(1 << 1);
    /// Device only supports offline updates
    pub const ONLY_OFFLINE: Self = Self(1 << 2);
    /// Device requires AC power
    pub const REQUIRE_AC: Self = Self(1 << 3);
    /// Device is locked
    pub const LOCKED: Self = Self(1 << 4);
    /// Device is supported by fwupd
    pub const SUPPORTED: Self = Self(1 << 5);
    /// Device needs bootloader mode
    pub const NEEDS_BOOTLOADER: Self = Self(1 << 6);
    /// Device has been registered
    pub const REGISTERED: Self = Self(1 << 7);
    /// Device needs reboot after update
    pub const NEEDS_REBOOT: Self = Self(1 << 8);
    /// Device needs shutdown after update
    pub const NEEDS_SHUTDOWN: Self = Self(1 << 9);
    /// Device is reported to remote server
    pub const REPORTED: Self = Self(1 << 10);
    /// Device has been notified
    pub const NOTIFIED: Self = Self(1 << 11);
    /// Use runtime version
    pub const USE_RUNTIME_VERSION: Self = Self(1 << 12);
    /// Install parent first
    pub const INSTALL_PARENT_FIRST: Self = Self(1 << 13);
    /// Historical device (no longer present)
    pub const IS_BOOTLOADER: Self = Self(1 << 14);
    /// Wait for replug
    pub const WAIT_FOR_REPLUG: Self = Self(1 << 15);
    /// Ignore validation
    pub const IGNORE_VALIDATION: Self = Self(1 << 16);
    /// Trusted device
    pub const TRUSTED: Self = Self(1 << 17);
    /// Can verify updates
    pub const CAN_VERIFY: Self = Self(1 << 18);
    /// Can verify image
    pub const CAN_VERIFY_IMAGE: Self = Self(1 << 19);
    /// Has multiple branches
    pub const HAS_MULTIPLE_BRANCHES: Self = Self(1 << 20);
    /// Backup before install
    pub const BACKUP_BEFORE_INSTALL: Self = Self(1 << 21);
    /// MD only set version
    pub const MD_SET_VERSION: Self = Self(1 << 22);
    /// Will disappear
    pub const WILL_DISAPPEAR: Self = Self(1 << 23);
    /// Has signed payload
    pub const SIGNED_PAYLOAD: Self = Self(1 << 24);
    /// Unsigned payload
    pub const UNSIGNED_PAYLOAD: Self = Self(1 << 25);
    /// Emulated device
    pub const EMULATED: Self = Self(1 << 26);

    pub fn contains(&self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub fn set(&mut self, flag: Self) {
        self.0 |= flag.0;
    }

    pub fn clear(&mut self, flag: Self) {
        self.0 &= !flag.0;
    }
}

/// Update flags for install
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstallFlags(pub u32);

impl InstallFlags {
    /// No special flags
    pub const NONE: Self = Self(0);
    /// Allow reinstalling same version
    pub const ALLOW_REINSTALL: Self = Self(1 << 0);
    /// Allow older version
    pub const ALLOW_OLDER: Self = Self(1 << 1);
    /// Force update
    pub const FORCE: Self = Self(1 << 2);
    /// Offline update
    pub const OFFLINE: Self = Self(1 << 3);
    /// Allow branch switch
    pub const ALLOW_BRANCH_SWITCH: Self = Self(1 << 4);
    /// Ignore checksums
    pub const IGNORE_CHECKSUM: Self = Self(1 << 5);
    /// Ignore vendor ID
    pub const IGNORE_VID_PID: Self = Self(1 << 6);
    /// Ignore power state
    pub const IGNORE_POWER: Self = Self(1 << 7);
    /// No history
    pub const NO_HISTORY: Self = Self(1 << 8);
}

/// Firmware release urgency
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseUrgency {
    Unknown,
    Low,
    Medium,
    High,
    Critical,
}

impl ReleaseUrgency {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

/// Update protocol type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateProtocol {
    /// Unknown protocol
    Unknown,
    /// UEFI capsule update
    UefiCapsule,
    /// UEFI ESRT (EFI System Resource Table)
    UefiEsrt,
    /// DFU (Device Firmware Upgrade)
    Dfu,
    /// Redfish
    Redfish,
    /// Vendor-specific
    VendorSpecific,
    /// Flashrom
    Flashrom,
    /// UEFI dbx (Forbidden Signature Database)
    UefiDbx,
    /// Logitech Unifying
    LogitechUnifying,
    /// Synaptics
    Synaptics,
    /// Dell ESRT
    DellEsrt,
    /// NVME
    Nvme,
    /// Thunderbolt
    Thunderbolt,
    /// Intel ME
    IntelMe,
    /// Intel SPI
    IntelSpi,
}

impl UpdateProtocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::UefiCapsule => "org.uefi.capsule",
            Self::UefiEsrt => "org.uefi.esrt",
            Self::Dfu => "org.usb.dfu",
            Self::Redfish => "org.dmtf.redfish",
            Self::VendorSpecific => "vendor-specific",
            Self::Flashrom => "org.flashrom",
            Self::UefiDbx => "org.uefi.dbx",
            Self::LogitechUnifying => "com.logitech.unifying",
            Self::Synaptics => "com.synaptics",
            Self::DellEsrt => "com.dell.esrt",
            Self::Nvme => "org.nvmexpress",
            Self::Thunderbolt => "org.thunderbolt",
            Self::IntelMe => "com.intel.me",
            Self::IntelSpi => "com.intel.spi",
        }
    }
}

/// Firmware release information
#[derive(Debug, Clone)]
pub struct FirmwareRelease {
    /// Version string
    pub version: String,
    /// Remote ID (LVFS, etc.)
    pub remote_id: String,
    /// URI to download
    pub uri: String,
    /// File size in bytes
    pub size: u64,
    /// SHA256 checksum
    pub checksum_sha256: String,
    /// Release urgency
    pub urgency: ReleaseUrgency,
    /// Release description
    pub description: String,
    /// Vendor name
    pub vendor: String,
    /// Release date (UNIX timestamp)
    pub release_date: u64,
    /// Install duration estimate (seconds)
    pub install_duration: u32,
    /// Protocol used
    pub protocol: UpdateProtocol,
    /// Is this a downgrade?
    pub is_downgrade: bool,
}

/// Firmware device information
#[derive(Debug, Clone)]
pub struct FirmwareDevice {
    /// Device unique ID
    pub device_id: String,
    /// Parent device ID (if any)
    pub parent_id: Option<String>,
    /// Device name
    pub name: String,
    /// Vendor name
    pub vendor: String,
    /// Vendor ID (USB VID, PCI vendor, etc.)
    pub vendor_id: u32,
    /// Current firmware version
    pub version: String,
    /// Bootloader version (if applicable)
    pub version_bootloader: Option<String>,
    /// Lowest supported version
    pub version_lowest: Option<String>,
    /// Device flags
    pub flags: DeviceFlags,
    /// GUID list for matching
    pub guids: Vec<String>,
    /// Update protocol
    pub protocol: UpdateProtocol,
    /// Update status
    pub status: FwupdStatus,
    /// Update progress (0-100)
    pub progress: u8,
    /// Available releases
    pub releases: Vec<FirmwareRelease>,
    /// Icons for display
    pub icons: Vec<String>,
    /// Serial number
    pub serial: Option<String>,
    /// Plugin that handles this device
    pub plugin: String,
    /// Created timestamp
    pub created: u64,
    /// Modified timestamp
    pub modified: u64,
}

impl FirmwareDevice {
    /// Check if device is updatable
    pub fn is_updatable(&self) -> bool {
        self.flags.contains(DeviceFlags::UPDATABLE)
    }

    /// Check if device needs reboot
    pub fn needs_reboot(&self) -> bool {
        self.flags.contains(DeviceFlags::NEEDS_REBOOT)
    }

    /// Get the latest available release
    pub fn latest_release(&self) -> Option<&FirmwareRelease> {
        self.releases.first()
    }

    /// Check if update is available
    pub fn has_update(&self) -> bool {
        if let Some(latest) = self.latest_release() {
            latest.version != self.version && !latest.is_downgrade
        } else {
            false
        }
    }
}

/// ESRT entry (EFI System Resource Table)
#[derive(Debug, Clone, Copy)]
pub struct EsrtEntry {
    /// Firmware class GUID
    pub fw_class: [u8; 16],
    /// Firmware type (system/device)
    pub fw_type: u32,
    /// Current firmware version
    pub fw_version: u32,
    /// Lowest supported version
    pub lowest_supported_fw_version: u32,
    /// Capsule flags
    pub capsule_flags: u32,
    /// Last update attempt version
    pub last_attempt_version: u32,
    /// Last update attempt status
    pub last_attempt_status: u32,
}

/// ESRT table header
#[derive(Debug, Clone, Copy)]
pub struct EsrtHeader {
    /// Resource count
    pub fw_resource_count: u32,
    /// Max resource count
    pub fw_resource_count_max: u32,
    /// Resource version
    pub fw_resource_version: u64,
}

/// Firmware history entry
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    /// Device ID
    pub device_id: String,
    /// Old version
    pub version_old: String,
    /// New version
    pub version_new: String,
    /// Update timestamp
    pub timestamp: u64,
    /// Update status
    pub status: FwupdStatus,
    /// Error message (if failed)
    pub error: Option<String>,
}

/// Remote firmware source (LVFS, etc.)
#[derive(Debug, Clone)]
pub struct Remote {
    /// Remote ID
    pub id: String,
    /// Remote title
    pub title: String,
    /// Remote type (download, local)
    pub kind: String,
    /// Is enabled
    pub enabled: bool,
    /// Keyring type
    pub keyring: String,
    /// Metadata URI
    pub metadata_uri: String,
    /// Report URI
    pub report_uri: Option<String>,
    /// Firmware base URI
    pub firmware_base_uri: Option<String>,
    /// Last refresh timestamp
    pub mtime: u64,
    /// Priority
    pub priority: i32,
}

/// Fwupd daemon manager
pub struct FwupdManager {
    /// Registered devices
    devices: BTreeMap<String, FirmwareDevice>,
    /// Update history
    history: Vec<HistoryEntry>,
    /// Configured remotes
    remotes: Vec<Remote>,
    /// ESRT entries from UEFI
    esrt_entries: Vec<EsrtEntry>,
    /// Current status
    status: FwupdStatus,
    /// Is daemon running
    running: bool,
    /// Pending updates count
    pending_updates: u32,
    /// Config: check for updates on startup
    check_on_startup: bool,
    /// Config: auto-download updates
    auto_download: bool,
    /// Config: allow prereleases
    allow_prereleases: bool,
    /// Percentage complete for current operation
    percentage: u8,
}

impl FwupdManager {
    /// Create a new fwupd manager
    pub fn new() -> Self {
        Self {
            devices: BTreeMap::new(),
            history: Vec::new(),
            remotes: Vec::new(),
            esrt_entries: Vec::new(),
            status: FwupdStatus::Idle,
            running: false,
            pending_updates: 0,
            check_on_startup: true,
            auto_download: false,
            allow_prereleases: false,
            percentage: 0,
        }
    }

    /// Initialize the fwupd manager
    pub fn init(&mut self) {
        self.running = true;

        // Setup default remotes
        self.setup_default_remotes();

        // Scan for ESRT entries
        self.scan_esrt();

        // Enumerate devices
        self.enumerate_devices();
    }

    /// Setup default remote sources
    fn setup_default_remotes(&mut self) {
        // LVFS (Linux Vendor Firmware Service)
        self.remotes.push(Remote {
            id: String::from("lvfs"),
            title: String::from("Linux Vendor Firmware Service"),
            kind: String::from("download"),
            enabled: true,
            keyring: String::from("gpg"),
            metadata_uri: String::from("https://cdn.fwupd.org/downloads/firmware.xml.gz"),
            report_uri: Some(String::from("https://fwupd.org/report")),
            firmware_base_uri: Some(String::from("https://cdn.fwupd.org/downloads/")),
            mtime: 0,
            priority: 0,
        });

        // LVFS testing (disabled by default)
        self.remotes.push(Remote {
            id: String::from("lvfs-testing"),
            title: String::from("Linux Vendor Firmware Service (Testing)"),
            kind: String::from("download"),
            enabled: false,
            keyring: String::from("gpg"),
            metadata_uri: String::from("https://cdn.fwupd.org/downloads/firmware-testing.xml.gz"),
            report_uri: Some(String::from("https://fwupd.org/report")),
            firmware_base_uri: Some(String::from("https://cdn.fwupd.org/downloads/")),
            mtime: 0,
            priority: -1,
        });
    }

    /// Scan UEFI ESRT table for firmware resources
    fn scan_esrt(&mut self) {
        // In a real implementation, this would read from UEFI variables
        // For now, we'll check if ESRT is available
        // The ESRT table is typically at EFI_SYSTEM_RESOURCE_TABLE_GUID
    }

    /// Enumerate all firmware devices
    pub fn enumerate_devices(&mut self) {
        self.devices.clear();

        // Enumerate UEFI devices from ESRT
        self.enumerate_uefi_devices();

        // Enumerate USB DFU devices
        self.enumerate_usb_dfu_devices();

        // Enumerate NVMe devices
        self.enumerate_nvme_devices();

        // Enumerate Thunderbolt devices
        self.enumerate_thunderbolt_devices();

        // Count pending updates
        self.count_pending_updates();
    }

    /// Enumerate UEFI firmware devices
    fn enumerate_uefi_devices(&mut self) {
        // System firmware
        let system_fw = FirmwareDevice {
            device_id: String::from("system-firmware"),
            parent_id: None,
            name: String::from("System Firmware"),
            vendor: String::from("Unknown"),
            vendor_id: 0,
            version: String::from("0.0.0"),
            version_bootloader: None,
            version_lowest: None,
            flags: DeviceFlags(
                DeviceFlags::INTERNAL.0 |
                DeviceFlags::UPDATABLE.0 |
                DeviceFlags::NEEDS_REBOOT.0 |
                DeviceFlags::SUPPORTED.0
            ),
            guids: vec![String::from("system-firmware-guid")],
            protocol: UpdateProtocol::UefiCapsule,
            status: FwupdStatus::Idle,
            progress: 0,
            releases: Vec::new(),
            icons: vec![String::from("computer")],
            serial: None,
            plugin: String::from("uefi_capsule"),
            created: 0,
            modified: 0,
        };
        self.devices.insert(system_fw.device_id.clone(), system_fw);
    }

    /// Enumerate USB DFU devices
    fn enumerate_usb_dfu_devices(&mut self) {
        // Would scan USB devices for DFU capability
    }

    /// Enumerate NVMe devices
    fn enumerate_nvme_devices(&mut self) {
        // Would scan NVMe controllers for firmware update capability
    }

    /// Enumerate Thunderbolt devices
    fn enumerate_thunderbolt_devices(&mut self) {
        // Would scan Thunderbolt domain for devices
    }

    /// Count pending updates
    fn count_pending_updates(&mut self) {
        self.pending_updates = 0;
        for device in self.devices.values() {
            if device.has_update() && device.is_updatable() {
                self.pending_updates += 1;
            }
        }
    }

    /// Get device by ID
    pub fn get_device(&self, device_id: &str) -> Option<&FirmwareDevice> {
        self.devices.get(device_id)
    }

    /// Get all devices
    pub fn get_devices(&self) -> Vec<&FirmwareDevice> {
        self.devices.values().collect()
    }

    /// Get devices with updates available
    pub fn get_updates(&self) -> Vec<&FirmwareDevice> {
        self.devices.values()
            .filter(|d| d.has_update() && d.is_updatable())
            .collect()
    }

    /// Get update history
    pub fn get_history(&self) -> &[HistoryEntry] {
        &self.history
    }

    /// Get remotes
    pub fn get_remotes(&self) -> &[Remote] {
        &self.remotes
    }

    /// Enable/disable a remote
    pub fn set_remote_enabled(&mut self, remote_id: &str, enabled: bool) -> bool {
        for remote in &mut self.remotes {
            if remote.id == remote_id {
                remote.enabled = enabled;
                return true;
            }
        }
        false
    }

    /// Refresh metadata from enabled remotes
    pub fn refresh(&mut self) -> Result<(), FwupdError> {
        self.status = FwupdStatus::Downloading;
        self.percentage = 0;

        let current_time = self.current_time();

        for remote in &mut self.remotes {
            if !remote.enabled {
                continue;
            }

            // In real implementation, would download and parse metadata
            remote.mtime = current_time;
        }

        self.status = FwupdStatus::Idle;
        self.percentage = 100;
        Ok(())
    }

    /// Install firmware to a device
    pub fn install(
        &mut self,
        device_id: &str,
        release: &FirmwareRelease,
        flags: InstallFlags,
    ) -> Result<(), FwupdError> {
        // Get timestamp first to avoid borrow issues
        let timestamp = self.current_time();
        let new_version = release.version.clone();
        let is_downgrade = release.is_downgrade;

        let device = self.devices.get_mut(device_id)
            .ok_or(FwupdError::DeviceNotFound)?;

        if !device.is_updatable() {
            return Err(FwupdError::NotSupported);
        }

        // Check if downgrade is allowed
        if is_downgrade && flags.0 & InstallFlags::ALLOW_OLDER.0 == 0 {
            return Err(FwupdError::DowngradeNotAllowed);
        }

        // Check if reinstall is allowed
        if new_version == device.version && flags.0 & InstallFlags::ALLOW_REINSTALL.0 == 0 {
            return Err(FwupdError::AlreadyInstalled);
        }

        let old_version = device.version.clone();

        // Update status
        device.status = FwupdStatus::Loading;
        device.progress = 0;

        // Download firmware if needed
        device.status = FwupdStatus::Downloading;
        device.progress = 25;

        // Verify firmware
        device.status = FwupdStatus::Verifying;
        device.progress = 50;

        // Schedule or write firmware based on protocol
        match device.protocol {
            UpdateProtocol::UefiCapsule => {
                device.status = FwupdStatus::Scheduling;
                device.progress = 75;
                // Would stage capsule for UEFI update manager
                device.flags.set(DeviceFlags::NEEDS_REBOOT);
            }
            UpdateProtocol::Dfu | UpdateProtocol::VendorSpecific => {
                device.status = FwupdStatus::Writing;
                device.progress = 75;
                // Would write directly to device
            }
            _ => {
                return Err(FwupdError::NotSupported);
            }
        }

        // Update version
        device.version = new_version.clone();
        let final_status = if device.needs_reboot() {
            FwupdStatus::NeedsReboot
        } else {
            FwupdStatus::Complete
        };
        device.status = final_status;
        device.progress = 100;

        // Add to history
        self.history.push(HistoryEntry {
            device_id: device_id.into(),
            version_old: old_version,
            version_new: new_version,
            timestamp,
            status: final_status,
            error: None,
        });

        Ok(())
    }

    /// Verify firmware on a device
    pub fn verify(&mut self, device_id: &str) -> Result<(), FwupdError> {
        let device = self.devices.get_mut(device_id)
            .ok_or(FwupdError::DeviceNotFound)?;

        if !device.flags.contains(DeviceFlags::CAN_VERIFY) {
            return Err(FwupdError::NotSupported);
        }

        device.status = FwupdStatus::Verifying;
        device.progress = 0;

        // Would verify firmware integrity
        device.progress = 100;
        device.status = FwupdStatus::Idle;

        Ok(())
    }

    /// Unlock a device for updates
    pub fn unlock(&mut self, device_id: &str) -> Result<(), FwupdError> {
        let device = self.devices.get_mut(device_id)
            .ok_or(FwupdError::DeviceNotFound)?;

        if !device.flags.contains(DeviceFlags::LOCKED) {
            return Ok(()); // Already unlocked
        }

        // Would perform unlock operation
        device.flags.clear(DeviceFlags::LOCKED);

        Ok(())
    }

    /// Clear update results
    pub fn clear_results(&mut self, device_id: &str) -> Result<(), FwupdError> {
        let device = self.devices.get_mut(device_id)
            .ok_or(FwupdError::DeviceNotFound)?;

        device.status = FwupdStatus::Idle;
        device.progress = 0;

        Ok(())
    }

    /// Get current daemon status
    pub fn get_status(&self) -> FwupdStatus {
        self.status
    }

    /// Get percentage complete
    pub fn get_percentage(&self) -> u8 {
        self.percentage
    }

    /// Get pending updates count
    pub fn get_pending_count(&self) -> u32 {
        self.pending_updates
    }

    /// Check if daemon is running
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Stop the daemon
    pub fn stop(&mut self) {
        self.running = false;
        self.status = FwupdStatus::Idle;
    }

    /// Get current timestamp
    fn current_time(&self) -> u64 {
        // Would get actual time
        0
    }

    /// Report update result to remote
    pub fn report_update(&self, entry: &HistoryEntry) -> Result<(), FwupdError> {
        // Would send report to LVFS
        Ok(())
    }

    /// Format status for display
    pub fn format_status(&self) -> String {
        let mut s = String::from("Firmware Update Daemon (fwupd):\n");
        s.push_str(&format!("  Status: {}\n", self.status.as_str()));
        s.push_str(&format!("  Running: {}\n", self.running));
        s.push_str(&format!("  Devices: {}\n", self.devices.len()));
        s.push_str(&format!("  Pending updates: {}\n", self.pending_updates));
        s.push_str(&format!("  Remotes: {} ({} enabled)\n",
            self.remotes.len(),
            self.remotes.iter().filter(|r| r.enabled).count()
        ));

        if !self.devices.is_empty() {
            s.push_str("  Devices:\n");
            for device in self.devices.values() {
                let status = if device.has_update() { "update available" } else { "up-to-date" };
                s.push_str(&format!("    {} v{} [{}]\n",
                    device.name, device.version, status));
            }
        }

        s
    }
}

/// Fwupd error type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FwupdError {
    /// Generic internal error
    Internal,
    /// Device not found
    DeviceNotFound,
    /// Operation not supported
    NotSupported,
    /// Version already installed
    AlreadyInstalled,
    /// Downgrade not allowed
    DowngradeNotAllowed,
    /// Signature verification failed
    SignatureInvalid,
    /// Insufficient permissions
    PermissionDenied,
    /// Network error
    NetworkError,
    /// AC power required
    AcPowerRequired,
    /// Battery too low
    BatteryLow,
    /// Device is locked
    DeviceLocked,
    /// Checksum mismatch
    ChecksumMismatch,
    /// Invalid firmware file
    InvalidFirmware,
    /// Read error
    ReadError,
    /// Write error
    WriteError,
    /// Timeout
    Timeout,
    /// Busy
    Busy,
}

impl FwupdError {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Internal => "internal-error",
            Self::DeviceNotFound => "device-not-found",
            Self::NotSupported => "not-supported",
            Self::AlreadyInstalled => "already-installed",
            Self::DowngradeNotAllowed => "downgrade-not-allowed",
            Self::SignatureInvalid => "signature-invalid",
            Self::PermissionDenied => "permission-denied",
            Self::NetworkError => "network-error",
            Self::AcPowerRequired => "ac-power-required",
            Self::BatteryLow => "battery-level-too-low",
            Self::DeviceLocked => "device-locked",
            Self::ChecksumMismatch => "checksum-mismatch",
            Self::InvalidFirmware => "invalid-firmware",
            Self::ReadError => "read-error",
            Self::WriteError => "write-error",
            Self::Timeout => "timeout",
            Self::Busy => "busy",
        }
    }
}

/// Global fwupd manager
static FWUPD_INIT: AtomicBool = AtomicBool::new(false);
static FWUPD: TicketSpinlock<Option<FwupdManager>> = TicketSpinlock::new(None);

/// Initialize the fwupd subsystem
pub fn init() {
    if FWUPD_INIT.swap(true, Ordering::SeqCst) {
        return;
    }

    let mut manager = FwupdManager::new();
    manager.init();

    *FWUPD.lock() = Some(manager);
}

/// Check if fwupd is initialized
pub fn is_initialized() -> bool {
    FWUPD_INIT.load(Ordering::SeqCst)
}

/// Get all firmware devices
pub fn get_devices() -> Vec<FirmwareDevice> {
    let guard = FWUPD.lock();
    if let Some(manager) = guard.as_ref() {
        manager.get_devices().iter().map(|d| (*d).clone()).collect()
    } else {
        Vec::new()
    }
}

/// Get devices with updates available
pub fn get_updates() -> Vec<FirmwareDevice> {
    let guard = FWUPD.lock();
    if let Some(manager) = guard.as_ref() {
        manager.get_updates().iter().map(|d| (*d).clone()).collect()
    } else {
        Vec::new()
    }
}

/// Get a specific device
pub fn get_device(device_id: &str) -> Option<FirmwareDevice> {
    let guard = FWUPD.lock();
    if let Some(manager) = guard.as_ref() {
        manager.get_device(device_id).cloned()
    } else {
        None
    }
}

/// Refresh metadata from remotes
pub fn refresh() -> Result<(), FwupdError> {
    let mut guard = FWUPD.lock();
    if let Some(manager) = guard.as_mut() {
        manager.refresh()
    } else {
        Err(FwupdError::Internal)
    }
}

/// Install firmware update
pub fn install(device_id: &str, release: &FirmwareRelease, flags: InstallFlags) -> Result<(), FwupdError> {
    let mut guard = FWUPD.lock();
    if let Some(manager) = guard.as_mut() {
        manager.install(device_id, release, flags)
    } else {
        Err(FwupdError::Internal)
    }
}

/// Get daemon status
pub fn get_status() -> FwupdStatus {
    let guard = FWUPD.lock();
    if let Some(manager) = guard.as_ref() {
        manager.get_status()
    } else {
        FwupdStatus::Unknown
    }
}

/// Get pending updates count
pub fn get_pending_count() -> u32 {
    let guard = FWUPD.lock();
    if let Some(manager) = guard.as_ref() {
        manager.get_pending_count()
    } else {
        0
    }
}

/// Get remotes list
pub fn get_remotes() -> Vec<Remote> {
    let guard = FWUPD.lock();
    if let Some(manager) = guard.as_ref() {
        manager.get_remotes().to_vec()
    } else {
        Vec::new()
    }
}

/// Enable or disable a remote
pub fn set_remote_enabled(remote_id: &str, enabled: bool) -> bool {
    let mut guard = FWUPD.lock();
    if let Some(manager) = guard.as_mut() {
        manager.set_remote_enabled(remote_id, enabled)
    } else {
        false
    }
}

/// Get update history
pub fn get_history() -> Vec<HistoryEntry> {
    let guard = FWUPD.lock();
    if let Some(manager) = guard.as_ref() {
        manager.get_history().to_vec()
    } else {
        Vec::new()
    }
}

/// Format status for display
pub fn format_status() -> String {
    let guard = FWUPD.lock();
    if let Some(manager) = guard.as_ref() {
        manager.format_status()
    } else {
        String::from("fwupd: Not initialized\n")
    }
}
