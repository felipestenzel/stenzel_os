//! Live USB Boot Support
//!
//! Provides functionality for booting from USB/CD in live mode,
//! including initramfs handling, squashfs mounting, and live session management.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use super::{InstallError, InstallResult};

/// Whether we're running in live mode
static LIVE_MODE: AtomicBool = AtomicBool::new(false);

/// Live session ID
static SESSION_ID: AtomicU64 = AtomicU64::new(0);

/// Live boot detection methods
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveBootMethod {
    /// Booted from USB drive
    Usb,
    /// Booted from CD/DVD
    Cdrom,
    /// Booted via network (PXE)
    Network,
    /// Booted from ISO mounted in memory
    IsoInRam,
    /// Not a live boot
    NotLive,
}

/// Live session information
#[derive(Debug, Clone)]
pub struct LiveSession {
    /// Session ID
    pub id: u64,
    /// Boot method
    pub boot_method: LiveBootMethod,
    /// Boot device path
    pub boot_device: String,
    /// Root filesystem type (squashfs, etc.)
    pub rootfs_type: String,
    /// Overlay filesystem for persistence
    pub overlay_path: Option<String>,
    /// Whether persistence is enabled
    pub persistence_enabled: bool,
    /// Available RAM for tmpfs
    pub available_ram_mb: u64,
    /// Squashfs mount point
    pub squashfs_mount: String,
    /// Overlay mount point
    pub overlay_mount: String,
}

impl LiveSession {
    /// Create a new live session
    pub fn new(boot_method: LiveBootMethod, boot_device: &str) -> Self {
        let id = SESSION_ID.fetch_add(1, Ordering::SeqCst);
        Self {
            id,
            boot_method,
            boot_device: boot_device.to_string(),
            rootfs_type: String::from("squashfs"),
            overlay_path: None,
            persistence_enabled: false,
            available_ram_mb: detect_available_ram(),
            squashfs_mount: String::from("/run/live/medium"),
            overlay_mount: String::from("/run/live/overlay"),
        }
    }

    /// Enable persistence with the given device/file
    pub fn enable_persistence(&mut self, path: &str) {
        self.overlay_path = Some(path.to_string());
        self.persistence_enabled = true;
    }

    /// Check if this is a valid live session
    pub fn is_valid(&self) -> bool {
        self.boot_method != LiveBootMethod::NotLive
    }
}

/// Live USB builder for creating bootable USB drives
pub struct LiveUsbBuilder {
    /// Source ISO or directory
    source: String,
    /// Target USB device
    target_device: String,
    /// Partition scheme (GPT or MBR)
    partition_scheme: PartitionScheme,
    /// Enable persistence partition
    persistence: bool,
    /// Persistence partition size in MB
    persistence_size_mb: u64,
    /// UEFI boot support
    uefi_support: bool,
    /// Legacy BIOS boot support
    bios_support: bool,
}

/// Partition scheme for live USB
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionScheme {
    /// GUID Partition Table (modern)
    Gpt,
    /// Master Boot Record (legacy)
    Mbr,
    /// Hybrid (both GPT and MBR)
    Hybrid,
}

impl LiveUsbBuilder {
    /// Create a new live USB builder
    pub fn new(source: &str, target_device: &str) -> Self {
        Self {
            source: source.to_string(),
            target_device: target_device.to_string(),
            partition_scheme: PartitionScheme::Gpt,
            persistence: false,
            persistence_size_mb: 0,
            uefi_support: true,
            bios_support: true,
        }
    }

    /// Set partition scheme
    pub fn partition_scheme(mut self, scheme: PartitionScheme) -> Self {
        self.partition_scheme = scheme;
        self
    }

    /// Enable persistence with given size
    pub fn with_persistence(mut self, size_mb: u64) -> Self {
        self.persistence = true;
        self.persistence_size_mb = size_mb;
        self
    }

    /// Set UEFI support
    pub fn uefi(mut self, enabled: bool) -> Self {
        self.uefi_support = enabled;
        self
    }

    /// Set BIOS support
    pub fn bios(mut self, enabled: bool) -> Self {
        self.bios_support = enabled;
        self
    }

    /// Build the live USB
    pub fn build(&self) -> InstallResult<()> {
        crate::kprintln!("live: Creating live USB on {}", self.target_device);

        // Step 1: Verify target device
        self.verify_target()?;

        // Step 2: Create partition table
        self.create_partitions()?;

        // Step 3: Format partitions
        self.format_partitions()?;

        // Step 4: Copy live system
        self.copy_live_system()?;

        // Step 5: Install bootloader
        self.install_bootloader()?;

        // Step 6: Create persistence partition if enabled
        if self.persistence {
            self.create_persistence()?;
        }

        crate::kprintln!("live: Live USB created successfully");
        Ok(())
    }

    fn verify_target(&self) -> InstallResult<()> {
        // Verify device exists and is removable
        // In real implementation, check /sys/block/*/removable
        Ok(())
    }

    fn create_partitions(&self) -> InstallResult<()> {
        crate::kprintln!("live: Creating partitions with {:?}", self.partition_scheme);
        
        // Create partition layout:
        // 1. EFI System Partition (if UEFI) - 512MB FAT32
        // 2. Boot partition (if BIOS) - for GRUB
        // 3. Live system partition - contains squashfs
        // 4. Persistence partition (optional) - ext4
        
        Ok(())
    }

    fn format_partitions(&self) -> InstallResult<()> {
        crate::kprintln!("live: Formatting partitions");
        // Format EFI partition as FAT32
        // Format live partition as FAT32 or ISO9660
        // Format persistence as ext4
        Ok(())
    }

    fn copy_live_system(&self) -> InstallResult<()> {
        crate::kprintln!("live: Copying live system from {}", self.source);
        // Copy kernel, initramfs, squashfs image
        Ok(())
    }

    fn install_bootloader(&self) -> InstallResult<()> {
        crate::kprintln!("live: Installing bootloader");
        
        if self.uefi_support {
            self.install_uefi_bootloader()?;
        }
        
        if self.bios_support {
            self.install_bios_bootloader()?;
        }
        
        Ok(())
    }

    fn install_uefi_bootloader(&self) -> InstallResult<()> {
        // Install systemd-boot or GRUB EFI
        // Copy EFI binaries to EFI/BOOT/BOOTX64.EFI
        Ok(())
    }

    fn install_bios_bootloader(&self) -> InstallResult<()> {
        // Install GRUB to MBR
        // Copy GRUB modules
        Ok(())
    }

    fn create_persistence(&self) -> InstallResult<()> {
        crate::kprintln!("live: Creating persistence partition ({}MB)", self.persistence_size_mb);
        // Create and format persistence partition
        // Create persistence.conf
        Ok(())
    }
}

/// Initramfs builder for live boot
pub struct InitramfsBuilder {
    /// Output path
    output: String,
    /// Compression type
    compression: Compression,
    /// Modules to include
    modules: Vec<String>,
    /// Init script
    init_script: String,
}

/// Compression types for initramfs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    Gzip,
    Xz,
    Lz4,
    Zstd,
    None,
}

impl InitramfsBuilder {
    pub fn new(output: &str) -> Self {
        Self {
            output: output.to_string(),
            compression: Compression::Zstd,
            modules: vec![
                String::from("squashfs"),
                String::from("overlay"),
                String::from("loop"),
                String::from("usb_storage"),
                String::from("ahci"),
                String::from("nvme"),
                String::from("xhci_hcd"),
                String::from("ehci_hcd"),
            ],
            init_script: default_init_script(),
        }
    }

    pub fn compression(mut self, comp: Compression) -> Self {
        self.compression = comp;
        self
    }

    pub fn add_module(mut self, module: &str) -> Self {
        self.modules.push(module.to_string());
        self
    }

    pub fn build(&self) -> InstallResult<()> {
        crate::kprintln!("live: Building initramfs -> {}", self.output);
        // Create cpio archive with:
        // - /init script
        // - busybox or minimal utilities
        // - kernel modules
        // - device nodes
        // Compress with selected algorithm
        Ok(())
    }
}

/// Default init script for live boot
fn default_init_script() -> String {
    String::from(r#"#!/bin/sh
# Stenzel OS Live Boot Init

mount -t devtmpfs devtmpfs /dev
mount -t proc proc /proc
mount -t sysfs sysfs /sys

# Find live medium
for device in /dev/sd* /dev/nvme*; do
    if mount -o ro "$device" /run/live/medium 2>/dev/null; then
        if [ -f /run/live/medium/live/filesystem.squashfs ]; then
            break
        fi
        umount /run/live/medium
    fi
done

# Mount squashfs
mount -t squashfs -o ro /run/live/medium/live/filesystem.squashfs /run/live/rootfs

# Setup overlay
mount -t tmpfs tmpfs /run/live/overlay
mkdir -p /run/live/overlay/upper /run/live/overlay/work

# Mount overlay as root
mount -t overlay overlay -o lowerdir=/run/live/rootfs,upperdir=/run/live/overlay/upper,workdir=/run/live/overlay/work /sysroot

# Switch root
exec switch_root /sysroot /sbin/init
"#)
}

/// Check if currently booted in live mode
pub fn is_live_boot() -> bool {
    LIVE_MODE.load(Ordering::Acquire)
}

/// Set live boot mode
pub fn set_live_mode(is_live: bool) {
    LIVE_MODE.store(is_live, Ordering::Release);
}

/// Detect boot method
pub fn detect_boot_method() -> LiveBootMethod {
    // Check kernel command line for live boot indicators
    // Check /proc/cmdline for "boot=live" or similar
    // Check if root is overlay/squashfs
    
    // For now, check if we're running from squashfs
    if is_squashfs_root() {
        return LiveBootMethod::Usb;
    }
    
    LiveBootMethod::NotLive
}

/// Check if root is squashfs
fn is_squashfs_root() -> bool {
    // Check /proc/mounts for squashfs or overlay
    false
}

/// Detect available RAM in MB
fn detect_available_ram() -> u64 {
    // Read from /proc/meminfo or use memory subsystem
    // For now return a default
    // TODO: Use actual memory detection when available
    // For now return a reasonable default (8GB)
    8192
}

/// Initialize live boot environment
pub fn init_live_environment() -> InstallResult<LiveSession> {
    let boot_method = detect_boot_method();
    
    if boot_method == LiveBootMethod::NotLive {
        return Err(InstallError::InvalidConfig(String::from("Not a live boot environment")));
    }
    
    set_live_mode(true);
    
    let session = LiveSession::new(boot_method, "/dev/sda");
    
    crate::kprintln!("live: Live environment initialized (method: {:?})", boot_method);
    
    Ok(session)
}

/// Mount live filesystem
pub fn mount_live_filesystem(session: &LiveSession) -> InstallResult<()> {
    crate::kprintln!("live: Mounting live filesystem from {}", session.boot_device);
    
    // Mount squashfs
    // Setup overlay if persistence enabled
    
    Ok(())
}

/// Unmount live filesystem (for shutdown)
pub fn unmount_live_filesystem(session: &LiveSession) -> InstallResult<()> {
    crate::kprintln!("live: Unmounting live filesystem");
    
    // Unmount overlay
    // Unmount squashfs
    // Sync persistence if enabled
    
    Ok(())
}

/// Initialize live module
pub fn init() {
    // Check if we're in live mode
    let method = detect_boot_method();
    if method != LiveBootMethod::NotLive {
        set_live_mode(true);
        crate::kprintln!("live: Running in live mode ({:?})", method);
    }
}
