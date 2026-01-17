//! Live USB Support
//!
//! Provides functionality for booting and running from a USB drive
//! without installing to the hard disk.

extern crate alloc;

use alloc::string::String;
use alloc::format;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::sync::RwSpinlock;

/// Static flag indicating if we're in live boot mode
static LIVE_BOOT: AtomicBool = AtomicBool::new(false);

/// Live environment configuration
static LIVE_CONFIG: RwSpinlock<Option<LiveConfig>> = RwSpinlock::new(None);

/// Live boot modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveMode {
    /// Standard live mode - all changes lost on reboot
    Standard,
    /// Live mode with persistent storage partition
    Persistent,
    /// Live mode with persistence file (casper-rw)
    PersistentFile,
    /// Copy to RAM mode - entire system copied to RAM
    ToRam,
}

/// Persistence mode for writable data
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistenceMode {
    /// No persistence - tmpfs overlay
    None,
    /// Persistence partition (e.g., persistence label)
    Partition,
    /// Persistence file on USB
    File,
}

/// Live environment configuration
#[derive(Debug, Clone)]
pub struct LiveConfig {
    /// Mode of live boot
    pub mode: LiveMode,
    /// Persistence configuration
    pub persistence: PersistenceMode,
    /// Path to persistence storage
    pub persistence_path: Option<String>,
    /// Size limit for persistence (bytes)
    pub persistence_limit: u64,
    /// Boot device path
    pub boot_device: String,
    /// Squashfs image path
    pub squashfs_path: String,
    /// Whether overlay is mounted
    pub overlay_mounted: bool,
}

impl Default for LiveConfig {
    fn default() -> Self {
        Self {
            mode: LiveMode::Standard,
            persistence: PersistenceMode::None,
            persistence_path: None,
            persistence_limit: 0,
            boot_device: String::new(),
            squashfs_path: String::new(),
            overlay_mounted: false,
        }
    }
}

/// Live USB manager
pub struct LiveUsb {
    config: LiveConfig,
}

impl LiveUsb {
    /// Create a new live USB manager
    pub fn new() -> Self {
        Self {
            config: LiveConfig::default(),
        }
    }

    /// Check if running in live mode
    pub fn is_live(&self) -> bool {
        LIVE_BOOT.load(Ordering::Relaxed)
    }

    /// Get current configuration
    pub fn config(&self) -> &LiveConfig {
        &self.config
    }

    /// Detect live boot environment
    pub fn detect(&mut self) -> bool {
        // Check kernel command line for live boot indicators
        if let Some(cmdline) = get_kernel_cmdline() {
            // Look for common live boot parameters
            if cmdline.contains("boot=live") ||
               cmdline.contains("root=live") ||
               cmdline.contains("live") ||
               cmdline.contains("toram") {
                self.config.mode = if cmdline.contains("toram") {
                    LiveMode::ToRam
                } else if cmdline.contains("persistent") || cmdline.contains("persistence") {
                    LiveMode::Persistent
                } else {
                    LiveMode::Standard
                };
                return true;
            }
        }

        // Check if boot device is removable (USB)
        if let Some(boot_dev) = detect_boot_device() {
            if is_device_removable(&boot_dev) {
                self.config.boot_device = boot_dev;
                return true;
            }
        }

        // Check for squashfs image presence
        if detect_squashfs_image().is_some() {
            return true;
        }

        false
    }

    /// Initialize the live environment
    pub fn init(&mut self) -> Result<(), String> {
        if !self.detect() {
            return Err(String::from("Not in live boot mode"));
        }

        LIVE_BOOT.store(true, Ordering::SeqCst);

        // Find and mount squashfs image
        let squashfs_path = detect_squashfs_image()
            .ok_or_else(|| String::from("Squashfs image not found"))?;
        self.config.squashfs_path = squashfs_path.clone();

        // Set up overlay filesystem
        self.setup_overlay()?;

        // Set up persistence if configured
        if self.config.mode == LiveMode::Persistent || self.config.mode == LiveMode::PersistentFile {
            self.setup_persistence()?;
        }

        // If toram mode, copy to RAM
        if self.config.mode == LiveMode::ToRam {
            self.copy_to_ram()?;
        }

        // Update global config
        {
            let mut config = LIVE_CONFIG.write();
            *config = Some(self.config.clone());
        }

        Ok(())
    }

    /// Set up overlay filesystem (squashfs + tmpfs)
    fn setup_overlay(&mut self) -> Result<(), String> {
        // Create mount points
        create_mount_point("/live")?;
        create_mount_point("/live/squashfs")?;
        create_mount_point("/live/overlay")?;
        create_mount_point("/live/merged")?;

        // Mount squashfs (read-only root)
        mount_squashfs(&self.config.squashfs_path, "/live/squashfs")?;

        // Create tmpfs for writable layer
        mount_tmpfs("/live/overlay", self.get_overlay_size())?;

        // Create overlay directories
        create_dir("/live/overlay/upper")?;
        create_dir("/live/overlay/work")?;

        // Mount overlayfs
        mount_overlay(
            "/live/squashfs",      // lower (read-only)
            "/live/overlay/upper", // upper (writable)
            "/live/overlay/work",  // work directory
            "/live/merged",        // merged view
        )?;

        self.config.overlay_mounted = true;

        // Pivot root or bind mount to make /live/merged the new root
        // This would require careful handling of /proc, /sys, /dev
        setup_live_root("/live/merged")?;

        Ok(())
    }

    /// Set up persistence storage
    fn setup_persistence(&mut self) -> Result<(), String> {
        // Look for persistence partition (labeled "persistence" or "casper-rw")
        if let Some(persistence_dev) = find_persistence_partition() {
            self.config.persistence = PersistenceMode::Partition;
            self.config.persistence_path = Some(persistence_dev.clone());

            // Mount persistence partition
            create_mount_point("/live/persistence")?;
            mount_ext4(&persistence_dev, "/live/persistence")?;

            // Use persistence as upper layer
            remount_overlay_with_persistence("/live/persistence")?;

            return Ok(());
        }

        // Look for persistence file on boot device
        if let Some(persistence_file) = find_persistence_file(&self.config.boot_device) {
            self.config.persistence = PersistenceMode::File;
            self.config.persistence_path = Some(persistence_file.clone());

            // Mount persistence file as loop device
            create_mount_point("/live/persistence")?;
            mount_loop_file(&persistence_file, "/live/persistence")?;

            // Use persistence as upper layer
            remount_overlay_with_persistence("/live/persistence")?;

            return Ok(());
        }

        // No persistence found
        self.config.persistence = PersistenceMode::None;
        Ok(())
    }

    /// Copy system to RAM (toram mode)
    fn copy_to_ram(&mut self) -> Result<(), String> {
        // Create ramdisk
        let squashfs_size = get_file_size(&self.config.squashfs_path)?;
        let ramdisk_size = squashfs_size + 100 * 1024 * 1024; // +100MB headroom

        create_mount_point("/live/toram")?;
        mount_tmpfs("/live/toram", ramdisk_size)?;

        // Copy squashfs to RAM
        let ram_squashfs = "/live/toram/filesystem.squashfs";
        copy_file(&self.config.squashfs_path, ram_squashfs)?;

        // Remount using RAM copy
        umount("/live/squashfs")?;
        mount_squashfs(ram_squashfs, "/live/squashfs")?;

        // Can now safely eject USB
        crate::kprintln!("System copied to RAM - USB can be removed");

        Ok(())
    }

    /// Get overlay tmpfs size based on available RAM
    fn get_overlay_size(&self) -> u64 {
        // Get total system memory
        let total_mem = get_total_memory();

        // Use 50% of RAM for overlay (max 4GB)
        let overlay_size = total_mem / 2;
        core::cmp::min(overlay_size, 4 * 1024 * 1024 * 1024)
    }

    /// Create a persistence file
    pub fn create_persistence_file(&self, path: &str, size_mb: u64) -> Result<(), String> {
        let size = size_mb * 1024 * 1024;

        // Create sparse file
        create_sparse_file(path, size)?;

        // Create ext4 filesystem
        let creator = super::FilesystemCreator::new();
        creator.mkfs(path, super::FilesystemType::Ext4, super::MkfsOptions {
            label: Some(String::from("persistence")),
            ..Default::default()
        })?;

        // Write persistence.conf
        let conf_content = "/ union\n";
        let mount_point = "/tmp/persistence_init";
        create_mount_point(mount_point)?;
        mount_loop_file(path, mount_point)?;
        write_file(&format!("{}/persistence.conf", mount_point), conf_content)?;
        umount(mount_point)?;

        Ok(())
    }

    /// Get live session info
    pub fn session_info(&self) -> LiveSessionInfo {
        let (used, total) = if self.config.overlay_mounted {
            get_overlay_usage()
        } else {
            (0, 0)
        };

        let persistence_used = if self.config.persistence != PersistenceMode::None {
            get_persistence_usage()
        } else {
            None
        };

        LiveSessionInfo {
            mode: self.config.mode,
            persistence: self.config.persistence,
            overlay_used: used,
            overlay_total: total,
            persistence_used,
            boot_device: self.config.boot_device.clone(),
            can_eject: self.config.mode == LiveMode::ToRam,
        }
    }
}

/// Live session information
#[derive(Debug, Clone)]
pub struct LiveSessionInfo {
    pub mode: LiveMode,
    pub persistence: PersistenceMode,
    pub overlay_used: u64,
    pub overlay_total: u64,
    pub persistence_used: Option<(u64, u64)>,
    pub boot_device: String,
    pub can_eject: bool,
}

/// Check if currently running in live boot mode
pub fn is_live_boot() -> bool {
    LIVE_BOOT.load(Ordering::Relaxed)
}

/// Get live configuration if in live mode
pub fn get_live_config() -> Option<LiveConfig> {
    LIVE_CONFIG.read().clone()
}

/// Initialize live environment
pub fn init_live_environment() -> Result<(), String> {
    let mut live = LiveUsb::new();
    live.init()
}

// ============================================================================
// Helper functions (would interface with actual kernel subsystems)
// ============================================================================

fn get_kernel_cmdline() -> Option<String> {
    // Read from /proc/cmdline or kernel boot args
    // For now, check boot_params if available
    #[cfg(feature = "boot_params")]
    {
        use crate::boot::get_cmdline;
        return get_cmdline();
    }

    // Fallback: try to detect from environment
    None
}

fn detect_boot_device() -> Option<String> {
    // Check kernel parameters for root device
    // Look at mounted filesystems
    None
}

fn is_device_removable(device: &str) -> bool {
    // Check /sys/block/xxx/removable
    // Or query USB subsystem
    true // Assume removable for now
}

fn detect_squashfs_image() -> Option<String> {
    // Look for common squashfs locations
    let common_paths = [
        "/live/filesystem.squashfs",
        "/casper/filesystem.squashfs",
        "/live/squashfs/filesystem.squashfs",
        "/cdrom/casper/filesystem.squashfs",
        "/cdrom/live/filesystem.squashfs",
    ];

    for path in &common_paths {
        if file_exists(path) {
            return Some(String::from(*path));
        }
    }

    None
}

fn find_persistence_partition() -> Option<String> {
    // Look for partition with label "persistence" or "casper-rw"
    // Check /dev/disk/by-label/
    None
}

fn find_persistence_file(boot_device: &str) -> Option<String> {
    // Look for persistence file on boot device
    // Common names: persistence, casper-rw
    None
}

fn get_total_memory() -> u64 {
    // Get from memory manager
    4 * 1024 * 1024 * 1024 // 4GB default
}

fn get_file_size(path: &str) -> Result<u64, String> {
    // Get file size via VFS
    Ok(2 * 1024 * 1024 * 1024) // 2GB default
}

fn file_exists(path: &str) -> bool {
    // Check via VFS
    false
}

fn create_mount_point(path: &str) -> Result<(), String> {
    // Create directory for mount point
    Ok(())
}

fn create_dir(path: &str) -> Result<(), String> {
    // Create directory
    Ok(())
}

fn mount_squashfs(image: &str, mount_point: &str) -> Result<(), String> {
    // Mount squashfs filesystem
    crate::kprintln!("Mounting squashfs {} at {}", image, mount_point);
    Ok(())
}

fn mount_tmpfs(mount_point: &str, size: u64) -> Result<(), String> {
    // Mount tmpfs with size limit
    crate::kprintln!("Mounting tmpfs at {} (size: {} MB)", mount_point, size / 1024 / 1024);
    Ok(())
}

fn mount_overlay(lower: &str, upper: &str, work: &str, merged: &str) -> Result<(), String> {
    // Mount overlayfs
    crate::kprintln!("Mounting overlay: lower={}, upper={}, merged={}", lower, upper, merged);
    Ok(())
}

fn mount_ext4(device: &str, mount_point: &str) -> Result<(), String> {
    // Mount ext4 filesystem
    Ok(())
}

fn mount_loop_file(file: &str, mount_point: &str) -> Result<(), String> {
    // Mount file as loop device
    Ok(())
}

fn umount(mount_point: &str) -> Result<(), String> {
    // Unmount filesystem
    Ok(())
}

fn setup_live_root(merged_root: &str) -> Result<(), String> {
    // Set up the merged overlay as the new root
    // This involves:
    // 1. Moving /proc, /sys, /dev to new root
    // 2. pivot_root or switch_root
    // 3. Cleaning up old root

    crate::kprintln!("Setting up live root at {}", merged_root);
    Ok(())
}

fn remount_overlay_with_persistence(persistence_path: &str) -> Result<(), String> {
    // Remount overlay using persistence as upper layer
    crate::kprintln!("Remounting overlay with persistence at {}", persistence_path);
    Ok(())
}

fn copy_file(src: &str, dst: &str) -> Result<(), String> {
    // Copy file
    crate::kprintln!("Copying {} to {}", src, dst);
    Ok(())
}

fn create_sparse_file(path: &str, size: u64) -> Result<(), String> {
    // Create sparse file
    Ok(())
}

fn write_file(path: &str, content: &str) -> Result<(), String> {
    // Write content to file
    Ok(())
}

fn get_overlay_usage() -> (u64, u64) {
    // Get overlay filesystem usage (used, total)
    (0, 4 * 1024 * 1024 * 1024)
}

fn get_persistence_usage() -> Option<(u64, u64)> {
    // Get persistence storage usage (used, total)
    None
}
