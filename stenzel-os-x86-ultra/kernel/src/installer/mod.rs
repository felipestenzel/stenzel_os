//! Stenzel OS Installer
//!
//! Complete installation system for deploying Stenzel OS to hardware.
//! Supports Live USB boot, partitioning, formatting, and system installation.

extern crate alloc;

pub mod live;
pub mod hwdetect;
pub mod partition;
pub mod format;
pub mod copy;
pub mod user;
pub mod bootloader;
pub mod uefi_entry;
pub mod iso;
pub mod updater;
pub mod recovery;
pub mod dualboot;
pub mod timezone;
pub mod recovery_partition;
pub mod luks;
pub mod cloud;
pub mod docker;
pub mod ab_partitions;
pub mod factory_reset;
pub mod backup;

use alloc::string::String;
use alloc::vec::Vec;

/// Installation error types
#[derive(Debug, Clone)]
pub enum InstallError {
    HardwareDetectionFailed(String),
    DiskNotFound,
    PartitionError(String),
    FormatError(String),
    CopyError(String),
    BootloaderError(String),
    UserConfigError(String),
    InsufficientSpace,
    PermissionDenied,
    IoError(String),
    InvalidConfig(String),
}

pub type InstallResult<T> = Result<T, InstallError>;
pub type ProgressCallback = fn(stage: &str, percent: u8, message: &str);

#[derive(Debug, Clone)]
pub struct InstallConfig {
    pub target_disk: String,
    pub hostname: String,
    pub username: String,
    pub password_hash: String,
    pub timezone: String,
    pub locale: String,
    pub keyboard_layout: String,
    pub encrypt: bool,
    pub encryption_password: Option<String>,
    pub root_fs: FilesystemType,
    pub create_swap: bool,
    pub swap_size_mb: u64,
    pub install_bootloader: bool,
    pub bootloader_type: BootloaderType,
    pub preserve_partitions: bool,
    pub preserved_partitions: Vec<u32>,
}

impl Default for InstallConfig {
    fn default() -> Self {
        Self {
            target_disk: String::new(),
            hostname: String::from("stenzel"),
            username: String::from("user"),
            password_hash: String::new(),
            timezone: String::from("UTC"),
            locale: String::from("en_US.UTF-8"),
            keyboard_layout: String::from("us"),
            encrypt: false,
            encryption_password: None,
            root_fs: FilesystemType::Ext4,
            create_swap: true,
            swap_size_mb: 0,
            install_bootloader: true,
            bootloader_type: BootloaderType::SystemdBoot,
            preserve_partitions: false,
            preserved_partitions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemType {
    Ext4, Btrfs, Xfs, Fat32,
}

impl FilesystemType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FilesystemType::Ext4 => "ext4",
            FilesystemType::Btrfs => "btrfs",
            FilesystemType::Xfs => "xfs",
            FilesystemType::Fat32 => "vfat",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootloaderType {
    SystemdBoot, Grub2, EfiStub,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallStage {
    NotStarted, DetectingHardware, Partitioning, Formatting,
    CopyingSystem, ConfiguringUsers, InstallingBootloader,
    Finalizing, Complete, Failed,
}

impl InstallStage {
    pub fn as_str(&self) -> &'static str {
        match self {
            InstallStage::NotStarted => "Not started",
            InstallStage::DetectingHardware => "Detecting hardware",
            InstallStage::Partitioning => "Partitioning disk",
            InstallStage::Formatting => "Formatting partitions",
            InstallStage::CopyingSystem => "Copying system files",
            InstallStage::ConfiguringUsers => "Configuring users",
            InstallStage::InstallingBootloader => "Installing bootloader",
            InstallStage::Finalizing => "Finalizing installation",
            InstallStage::Complete => "Installation complete",
            InstallStage::Failed => "Installation failed",
        }
    }
}

pub struct Installer {
    config: InstallConfig,
    stage: InstallStage,
    progress_callback: Option<ProgressCallback>,
    error_message: Option<String>,
}

impl Installer {
    pub fn new(config: InstallConfig) -> Self {
        Self {
            config,
            stage: InstallStage::NotStarted,
            progress_callback: None,
            error_message: None,
        }
    }

    pub fn set_progress_callback(&mut self, callback: ProgressCallback) {
        self.progress_callback = Some(callback);
    }

    pub fn stage(&self) -> InstallStage { self.stage }
    pub fn error_message(&self) -> Option<&str> { self.error_message.as_deref() }

    fn report_progress(&self, percent: u8, message: &str) {
        if let Some(callback) = self.progress_callback {
            callback(self.stage.as_str(), percent, message);
        }
    }

    pub fn install(&mut self) -> InstallResult<()> {
        self.stage = InstallStage::DetectingHardware;
        self.report_progress(0, "Scanning hardware...");
        let hw_info = hwdetect::detect_hardware()?;
        self.report_progress(10, "Hardware detected");

        self.stage = InstallStage::Partitioning;
        self.report_progress(15, "Creating partitions...");
        let partitions = partition::partition_disk(&self.config, &hw_info)?;
        self.report_progress(25, "Partitions created");

        self.stage = InstallStage::Formatting;
        self.report_progress(30, "Formatting partitions...");
        format::format_partitions(&partitions, &self.config)?;
        self.report_progress(40, "Partitions formatted");

        self.stage = InstallStage::CopyingSystem;
        self.report_progress(45, "Copying system files...");
        copy::copy_system(&partitions, &self.config)?;
        self.report_progress(75, "System files copied");

        self.stage = InstallStage::ConfiguringUsers;
        self.report_progress(80, "Configuring users...");
        user::configure_users(&partitions, &self.config)?;
        self.report_progress(85, "Users configured");

        self.stage = InstallStage::InstallingBootloader;
        self.report_progress(90, "Installing bootloader...");
        if self.config.install_bootloader {
            bootloader::install_bootloader(&partitions, &self.config)?;
        }
        self.report_progress(95, "Bootloader installed");

        self.stage = InstallStage::Finalizing;
        self.report_progress(98, "Finalizing...");

        self.stage = InstallStage::Complete;
        self.report_progress(100, "Installation complete!");
        Ok(())
    }
}

pub fn init() {
    crate::kprintln!("installer: Stenzel OS installer initialized");
}

pub fn is_live_environment() -> bool {
    live::is_live_boot()
}

pub fn format_status() -> String {
    String::from("Installer: Ready")
}
