//! System Installer Module
//!
//! Provides functionality for:
//! - Live USB boot and operation
//! - Disk partitioning
//! - Filesystem creation
//! - System installation
//! - Bootloader installation
//! - Initial system configuration

extern crate alloc;

use alloc::format;

pub mod liveusb;
pub mod partition;
pub mod filesystem;
pub mod bootloader;
pub mod copy;
pub mod setup;

pub use liveusb::{LiveUsb, LiveMode, PersistenceMode, is_live_boot, init_live_environment};
pub use partition::{PartitionManager, Partition, PartitionType, PartitionScheme};
pub use filesystem::{FilesystemCreator, FilesystemType, MkfsOptions};
pub use bootloader::{BootloaderInstaller, BootloaderType, install_bootloader};
pub use copy::{SystemCopier, CopyProgress, copy_system};
pub use setup::{SetupWizard, SetupStep, UserSetup, TimezoneSetup, KeyboardSetup, NetworkSetup};

use alloc::string::String;
use alloc::vec::Vec;

/// Installation target disk
#[derive(Debug, Clone)]
pub struct InstallTarget {
    /// Device path (e.g., /dev/sda)
    pub device: String,
    /// Device size in bytes
    pub size: u64,
    /// Device model/name
    pub model: String,
    /// Whether device is removable (USB, etc.)
    pub removable: bool,
    /// Existing partitions
    pub partitions: Vec<Partition>,
}

/// Installation configuration
#[derive(Debug, Clone)]
pub struct InstallConfig {
    /// Target disk
    pub target: InstallTarget,
    /// Partition scheme to use
    pub partition_scheme: PartitionScheme,
    /// Root filesystem type
    pub root_fs: FilesystemType,
    /// EFI system partition size (if applicable)
    pub efi_size: u64,
    /// Boot partition size
    pub boot_size: u64,
    /// Swap size (0 for no swap)
    pub swap_size: u64,
    /// Root partition size (0 for remaining space)
    pub root_size: u64,
    /// Create separate /home partition
    pub separate_home: bool,
    /// Username for initial user
    pub username: String,
    /// Password for initial user (hashed)
    pub password_hash: String,
    /// Hostname
    pub hostname: String,
    /// Timezone (e.g., "America/Sao_Paulo")
    pub timezone: String,
    /// Keyboard layout
    pub keyboard_layout: String,
    /// Enable network during install
    pub network_enabled: bool,
}

impl Default for InstallConfig {
    fn default() -> Self {
        Self {
            target: InstallTarget {
                device: String::new(),
                size: 0,
                model: String::new(),
                removable: false,
                partitions: Vec::new(),
            },
            partition_scheme: PartitionScheme::Gpt,
            root_fs: FilesystemType::Ext4,
            efi_size: 512 * 1024 * 1024, // 512 MB
            boot_size: 1024 * 1024 * 1024, // 1 GB
            swap_size: 2 * 1024 * 1024 * 1024, // 2 GB
            root_size: 0, // Use remaining space
            separate_home: false,
            username: String::from("user"),
            password_hash: String::new(),
            hostname: String::from("stenzel"),
            timezone: String::from("UTC"),
            keyboard_layout: String::from("us"),
            network_enabled: true,
        }
    }
}

/// Installation progress tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallPhase {
    NotStarted,
    Partitioning,
    CreatingFilesystems,
    Mounting,
    CopyingSystem,
    InstallingBootloader,
    ConfiguringSystem,
    CreatingUser,
    Finalizing,
    Complete,
    Failed,
}

/// Installation progress
#[derive(Debug, Clone)]
pub struct InstallProgress {
    pub phase: InstallPhase,
    pub phase_progress: u8, // 0-100
    pub overall_progress: u8, // 0-100
    pub status_message: String,
    pub error_message: Option<String>,
}

impl Default for InstallProgress {
    fn default() -> Self {
        Self {
            phase: InstallPhase::NotStarted,
            phase_progress: 0,
            overall_progress: 0,
            status_message: String::from("Ready to install"),
            error_message: None,
        }
    }
}

/// Main installer struct
pub struct Installer {
    config: InstallConfig,
    progress: InstallProgress,
    live_mode: bool,
}

impl Installer {
    /// Create a new installer
    pub fn new() -> Self {
        Self {
            config: InstallConfig::default(),
            progress: InstallProgress::default(),
            live_mode: is_live_boot(),
        }
    }

    /// Check if running in live mode
    pub fn is_live_mode(&self) -> bool {
        self.live_mode
    }

    /// Get installation configuration
    pub fn config(&self) -> &InstallConfig {
        &self.config
    }

    /// Set installation configuration
    pub fn set_config(&mut self, config: InstallConfig) {
        self.config = config;
    }

    /// Get installation progress
    pub fn progress(&self) -> &InstallProgress {
        &self.progress
    }

    /// Detect available disks for installation
    pub fn detect_disks(&self) -> Vec<InstallTarget> {
        let mut targets = Vec::new();

        // Check block devices from storage subsystem
        // This would interface with the storage module

        // For now, return simulated disks based on detected storage
        #[cfg(feature = "storage_detection")]
        {
            use crate::storage::get_block_devices;
            for dev in get_block_devices() {
                if !dev.is_read_only() {
                    targets.push(InstallTarget {
                        device: dev.name().to_string(),
                        size: dev.size(),
                        model: dev.model().unwrap_or("Unknown").to_string(),
                        removable: dev.is_removable(),
                        partitions: Vec::new(), // Would be populated by partition detection
                    });
                }
            }
        }

        targets
    }

    /// Start the installation process
    pub fn start(&mut self) -> Result<(), String> {
        if self.config.target.device.is_empty() {
            return Err(String::from("No target device selected"));
        }

        self.progress.phase = InstallPhase::Partitioning;
        self.progress.status_message = String::from("Creating partitions...");

        // Phase 1: Partition the disk
        self.partition_disk()?;

        // Phase 2: Create filesystems
        self.progress.phase = InstallPhase::CreatingFilesystems;
        self.progress.status_message = String::from("Creating filesystems...");
        self.progress.overall_progress = 15;
        self.create_filesystems()?;

        // Phase 3: Mount filesystems
        self.progress.phase = InstallPhase::Mounting;
        self.progress.status_message = String::from("Mounting filesystems...");
        self.progress.overall_progress = 25;
        self.mount_filesystems()?;

        // Phase 4: Copy system files
        self.progress.phase = InstallPhase::CopyingSystem;
        self.progress.status_message = String::from("Copying system files...");
        self.progress.overall_progress = 30;
        self.copy_system_files()?;

        // Phase 5: Install bootloader
        self.progress.phase = InstallPhase::InstallingBootloader;
        self.progress.status_message = String::from("Installing bootloader...");
        self.progress.overall_progress = 80;
        self.install_bootloader()?;

        // Phase 6: Configure system
        self.progress.phase = InstallPhase::ConfiguringSystem;
        self.progress.status_message = String::from("Configuring system...");
        self.progress.overall_progress = 90;
        self.configure_system()?;

        // Phase 7: Create user
        self.progress.phase = InstallPhase::CreatingUser;
        self.progress.status_message = String::from("Creating user account...");
        self.progress.overall_progress = 95;
        self.create_user()?;

        // Phase 8: Finalize
        self.progress.phase = InstallPhase::Finalizing;
        self.progress.status_message = String::from("Finalizing installation...");
        self.progress.overall_progress = 98;
        self.finalize()?;

        self.progress.phase = InstallPhase::Complete;
        self.progress.status_message = String::from("Installation complete!");
        self.progress.overall_progress = 100;

        Ok(())
    }

    fn partition_disk(&mut self) -> Result<(), String> {
        let manager = PartitionManager::new(&self.config.target.device)?;

        match self.config.partition_scheme {
            PartitionScheme::Gpt => {
                // Create GPT partition table
                manager.create_gpt()?;

                // EFI System Partition
                let efi_part = manager.create_partition(
                    PartitionType::EfiSystem,
                    self.config.efi_size,
                )?;

                // Boot partition
                let boot_part = manager.create_partition(
                    PartitionType::LinuxFilesystem,
                    self.config.boot_size,
                )?;

                // Swap partition (if requested)
                if self.config.swap_size > 0 {
                    manager.create_partition(
                        PartitionType::LinuxSwap,
                        self.config.swap_size,
                    )?;
                }

                // Root partition
                let root_size = if self.config.root_size == 0 {
                    0 // Use remaining space
                } else {
                    self.config.root_size
                };
                manager.create_partition(
                    PartitionType::LinuxFilesystem,
                    root_size,
                )?;

                // Home partition (if requested)
                if self.config.separate_home {
                    manager.create_partition(
                        PartitionType::LinuxFilesystem,
                        0, // Use remaining space
                    )?;
                }
            }
            PartitionScheme::Mbr => {
                // Create MBR partition table
                manager.create_mbr()?;

                // Boot partition
                manager.create_partition(
                    PartitionType::LinuxFilesystem,
                    self.config.boot_size,
                )?;

                // Swap partition
                if self.config.swap_size > 0 {
                    manager.create_partition(
                        PartitionType::LinuxSwap,
                        self.config.swap_size,
                    )?;
                }

                // Root partition
                manager.create_partition(
                    PartitionType::LinuxFilesystem,
                    0,
                )?;
            }
        }

        self.progress.phase_progress = 100;
        self.progress.overall_progress = 10;
        Ok(())
    }

    fn create_filesystems(&mut self) -> Result<(), String> {
        let creator = FilesystemCreator::new();

        // Create EFI filesystem (FAT32)
        if matches!(self.config.partition_scheme, PartitionScheme::Gpt) {
            let efi_dev = format!("{}1", self.config.target.device);
            creator.mkfs(&efi_dev, FilesystemType::Fat32, MkfsOptions::default())?;
        }

        // Create boot filesystem
        let boot_dev = if matches!(self.config.partition_scheme, PartitionScheme::Gpt) {
            format!("{}2", self.config.target.device)
        } else {
            format!("{}1", self.config.target.device)
        };
        creator.mkfs(&boot_dev, FilesystemType::Ext4, MkfsOptions::default())?;

        // Create root filesystem
        let root_dev = if matches!(self.config.partition_scheme, PartitionScheme::Gpt) {
            if self.config.swap_size > 0 { format!("{}4", self.config.target.device) }
            else { format!("{}3", self.config.target.device) }
        } else {
            if self.config.swap_size > 0 { format!("{}3", self.config.target.device) }
            else { format!("{}2", self.config.target.device) }
        };
        creator.mkfs(&root_dev, self.config.root_fs, MkfsOptions::default())?;

        // Setup swap
        if self.config.swap_size > 0 {
            let swap_dev = if matches!(self.config.partition_scheme, PartitionScheme::Gpt) {
                format!("{}3", self.config.target.device)
            } else {
                format!("{}2", self.config.target.device)
            };
            creator.mkswap(&swap_dev)?;
        }

        self.progress.phase_progress = 100;
        Ok(())
    }

    fn mount_filesystems(&mut self) -> Result<(), String> {
        // Mount root at /mnt/install
        // Mount boot at /mnt/install/boot
        // Mount efi at /mnt/install/boot/efi
        // Mount home at /mnt/install/home (if separate)

        // This would use the VFS mount system
        self.progress.phase_progress = 100;
        Ok(())
    }

    fn copy_system_files(&mut self) -> Result<(), String> {
        let mut copier = SystemCopier::new("/mnt/install");

        // Copy from live filesystem or installation image
        copier.copy_with_progress(|progress| {
            self.progress.phase_progress = progress;
            self.progress.overall_progress = 30 + (progress as u32 * 50 / 100) as u8;
        })?;

        Ok(())
    }

    fn install_bootloader(&mut self) -> Result<(), String> {
        let installer = BootloaderInstaller::new();

        let boot_type = if matches!(self.config.partition_scheme, PartitionScheme::Gpt) {
            BootloaderType::Uefi
        } else {
            BootloaderType::Bios
        };

        installer.install(boot_type, &self.config.target.device, "/mnt/install")?;

        self.progress.phase_progress = 100;
        Ok(())
    }

    fn configure_system(&mut self) -> Result<(), String> {
        let setup = SetupWizard::new("/mnt/install");

        // Set hostname
        setup.set_hostname(&self.config.hostname)?;

        // Set timezone
        setup.set_timezone(&self.config.timezone)?;

        // Set keyboard layout
        setup.set_keyboard_layout(&self.config.keyboard_layout)?;

        // Configure fstab
        setup.generate_fstab(&self.config.target)?;

        self.progress.phase_progress = 100;
        Ok(())
    }

    fn create_user(&mut self) -> Result<(), String> {
        let setup = SetupWizard::new("/mnt/install");

        // Create root user
        setup.set_root_password(&self.config.password_hash)?;

        // Create regular user
        setup.create_user(&self.config.username, &self.config.password_hash)?;

        // Add user to wheel group for sudo
        setup.add_user_to_group(&self.config.username, "wheel")?;

        self.progress.phase_progress = 100;
        Ok(())
    }

    fn finalize(&mut self) -> Result<(), String> {
        // Unmount filesystems
        // Sync disks
        // Generate initramfs if needed

        self.progress.phase_progress = 100;
        Ok(())
    }
}

/// Initialize the installer subsystem
pub fn init() {
    // Initialize live USB environment if detected
    if is_live_boot() {
        if let Err(e) = init_live_environment() {
            crate::kprintln!("Warning: Failed to initialize live environment: {}", e);
        } else {
            crate::kprintln!("Live USB environment initialized");
        }
    }
}
