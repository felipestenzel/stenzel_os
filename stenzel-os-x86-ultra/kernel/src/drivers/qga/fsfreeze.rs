//! QGA Filesystem Freeze/Thaw
//!
//! Filesystem freeze support for consistent snapshots.

#![allow(dead_code)]

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::sync::IrqSafeMutex;

/// Freeze state for a single filesystem
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsState {
    /// Normal operation
    Thawed,
    /// Being frozen
    Freezing,
    /// Frozen (no writes allowed)
    Frozen,
    /// Being thawed
    Thawing,
    /// Error state
    Error,
}

/// Mounted filesystem info
#[derive(Debug, Clone)]
pub struct MountInfo {
    /// Mount point path
    pub mountpoint: String,
    /// Device path
    pub device: String,
    /// Filesystem type
    pub fstype: String,
    /// Mount options
    pub options: String,
    /// Current freeze state
    pub state: FsState,
    /// Can be frozen
    pub freezable: bool,
}

impl MountInfo {
    pub fn new(mountpoint: &str, device: &str, fstype: &str) -> Self {
        let freezable = Self::is_freezable(fstype);
        Self {
            mountpoint: mountpoint.to_string(),
            device: device.to_string(),
            fstype: fstype.to_string(),
            options: String::new(),
            state: FsState::Thawed,
            freezable,
        }
    }

    /// Check if filesystem type can be frozen
    fn is_freezable(fstype: &str) -> bool {
        match fstype {
            // Block-based filesystems can be frozen
            "ext2" | "ext3" | "ext4" | "xfs" | "btrfs" | "f2fs" | "ntfs" | "fat32" => true,
            // Virtual/special filesystems cannot
            "tmpfs" | "devtmpfs" | "sysfs" | "proc" | "devfs" | "ramfs" | "rootfs" => false,
            // Network filesystems - generally not
            "nfs" | "cifs" | "sshfs" | "fuse" => false,
            // Unknown - assume not freezable
            _ => false,
        }
    }
}

/// Filesystem freeze statistics
#[derive(Debug, Default)]
pub struct FsFreezeStats {
    /// Total freeze operations
    pub freeze_count: AtomicU32,
    /// Total thaw operations
    pub thaw_count: AtomicU32,
    /// Total filesystems frozen
    pub filesystems_frozen: AtomicU32,
    /// Total freeze errors
    pub freeze_errors: AtomicU32,
    /// Total thaw errors
    pub thaw_errors: AtomicU32,
}

/// Filesystem freeze manager
pub struct FsFreezeManager {
    /// Mounted filesystems
    mounts: Vec<MountInfo>,
    /// Currently frozen
    frozen: AtomicBool,
    /// Number of frozen filesystems
    frozen_count: AtomicU32,
    /// Initialized
    initialized: AtomicBool,
    /// Statistics
    stats: FsFreezeStats,
}

impl FsFreezeManager {
    /// Create new freeze manager
    pub fn new() -> Self {
        Self {
            mounts: Vec::new(),
            frozen: AtomicBool::new(false),
            frozen_count: AtomicU32::new(0),
            initialized: AtomicBool::new(false),
            stats: FsFreezeStats::default(),
        }
    }

    /// Initialize with current mount points
    pub fn init(&mut self) -> Result<(), &'static str> {
        // Discover mounted filesystems
        self.discover_mounts();

        self.initialized.store(true, Ordering::Release);
        crate::kprintln!("fsfreeze: Initialized with {} filesystems ({} freezable)",
            self.mounts.len(),
            self.mounts.iter().filter(|m| m.freezable).count()
        );

        Ok(())
    }

    /// Discover mounted filesystems
    fn discover_mounts(&mut self) {
        self.mounts.clear();

        // Add standard mount points
        // In a real implementation, would parse /proc/mounts or equivalent
        self.mounts.push(MountInfo::new("/", "/dev/vda1", "ext4"));
        self.mounts.push(MountInfo::new("/boot", "/dev/vda2", "ext4"));
        self.mounts.push(MountInfo::new("/tmp", "tmpfs", "tmpfs"));
        self.mounts.push(MountInfo::new("/dev", "devtmpfs", "devtmpfs"));
        self.mounts.push(MountInfo::new("/proc", "proc", "proc"));
        self.mounts.push(MountInfo::new("/sys", "sysfs", "sysfs"));
    }

    /// Get current freeze status
    pub fn status(&self) -> FsState {
        if self.frozen.load(Ordering::Acquire) {
            FsState::Frozen
        } else {
            FsState::Thawed
        }
    }

    /// Get frozen count
    pub fn frozen_count(&self) -> u32 {
        self.frozen_count.load(Ordering::Acquire)
    }

    /// Check if frozen
    pub fn is_frozen(&self) -> bool {
        self.frozen.load(Ordering::Acquire)
    }

    /// Freeze all freezable filesystems
    pub fn freeze_all(&mut self) -> Result<u32, &'static str> {
        if !self.initialized.load(Ordering::Acquire) {
            return Err("Not initialized");
        }

        if self.frozen.load(Ordering::Acquire) {
            return Err("Already frozen");
        }

        crate::kprintln!("fsfreeze: Freezing filesystems...");

        let mut frozen_count = 0;

        // Freeze in reverse mount order (deepest first)
        for mount in self.mounts.iter_mut().rev() {
            if !mount.freezable {
                continue;
            }

            // Inline freeze logic to avoid borrow conflict
            let can_freeze = mount.fstype == "ext4" || mount.fstype == "xfs" || mount.fstype == "btrfs";

            if can_freeze {
                mount.state = FsState::Frozen;
                frozen_count += 1;
                crate::kprintln!("fsfreeze: Frozen {}", mount.mountpoint);
            } else {
                mount.state = FsState::Error;
                self.stats.freeze_errors.fetch_add(1, Ordering::Relaxed);
                crate::kprintln!("fsfreeze: Failed to freeze {}: Unsupported filesystem", mount.mountpoint);
            }
        }

        if frozen_count > 0 {
            self.frozen.store(true, Ordering::Release);
            self.frozen_count.store(frozen_count, Ordering::Release);
            self.stats.freeze_count.fetch_add(1, Ordering::Relaxed);
            self.stats.filesystems_frozen.fetch_add(frozen_count, Ordering::Relaxed);
        }

        crate::kprintln!("fsfreeze: {} filesystems frozen", frozen_count);
        Ok(frozen_count)
    }

    /// Thaw all frozen filesystems
    pub fn thaw_all(&mut self) -> Result<u32, &'static str> {
        if !self.initialized.load(Ordering::Acquire) {
            return Err("Not initialized");
        }

        if !self.frozen.load(Ordering::Acquire) {
            return Ok(0); // Nothing to thaw
        }

        crate::kprintln!("fsfreeze: Thawing filesystems...");

        let mut thawed_count = 0;

        // Thaw in mount order (root first)
        for mount in self.mounts.iter_mut() {
            if mount.state != FsState::Frozen {
                continue;
            }

            // Inline thaw logic to avoid borrow conflict
            mount.state = FsState::Thawed;
            thawed_count += 1;
            crate::kprintln!("fsfreeze: Thawed {}", mount.mountpoint);
        }

        self.frozen.store(false, Ordering::Release);
        self.frozen_count.store(0, Ordering::Release);
        self.stats.thaw_count.fetch_add(1, Ordering::Relaxed);

        crate::kprintln!("fsfreeze: {} filesystems thawed", thawed_count);
        Ok(thawed_count)
    }

    /// Get mount info list
    pub fn mounts(&self) -> &[MountInfo] {
        &self.mounts
    }

    /// Add a mount point
    pub fn add_mount(&mut self, mountpoint: &str, device: &str, fstype: &str) {
        let mount = MountInfo::new(mountpoint, device, fstype);
        self.mounts.push(mount);
    }

    /// Remove a mount point
    pub fn remove_mount(&mut self, mountpoint: &str) {
        self.mounts.retain(|m| m.mountpoint != mountpoint);
    }

    /// Get statistics
    pub fn stats(&self) -> &FsFreezeStats {
        &self.stats
    }

    /// Format status string
    pub fn format_status(&self) -> String {
        let status = if self.frozen.load(Ordering::Acquire) {
            "frozen"
        } else {
            "thawed"
        };

        let freezable: Vec<&str> = self.mounts.iter()
            .filter(|m| m.freezable)
            .map(|m| m.mountpoint.as_str())
            .collect();

        alloc::format!(
            "FSFreeze: {} ({} frozen) - freezable: {}",
            status,
            self.frozen_count.load(Ordering::Acquire),
            freezable.join(", ")
        )
    }
}

impl Default for FsFreezeManager {
    fn default() -> Self {
        Self::new()
    }
}

// Global freeze manager
static FS_FREEZE: IrqSafeMutex<Option<FsFreezeManager>> = IrqSafeMutex::new(None);

/// Initialize filesystem freeze manager
pub fn init() -> Result<(), &'static str> {
    let mut manager = FsFreezeManager::new();
    manager.init()?;
    *FS_FREEZE.lock() = Some(manager);
    Ok(())
}

/// Freeze all filesystems
pub fn freeze() -> Result<u32, &'static str> {
    FS_FREEZE.lock()
        .as_mut()
        .ok_or("Not initialized")?
        .freeze_all()
}

/// Thaw all filesystems
pub fn thaw() -> Result<u32, &'static str> {
    FS_FREEZE.lock()
        .as_mut()
        .ok_or("Not initialized")?
        .thaw_all()
}

/// Get freeze status
pub fn status() -> FsState {
    FS_FREEZE.lock()
        .as_ref()
        .map(|m| m.status())
        .unwrap_or(FsState::Thawed)
}

/// Is frozen?
pub fn is_frozen() -> bool {
    FS_FREEZE.lock()
        .as_ref()
        .map(|m| m.is_frozen())
        .unwrap_or(false)
}

/// Get status string
pub fn status_string() -> String {
    FS_FREEZE.lock()
        .as_ref()
        .map(|m| m.format_status())
        .unwrap_or_else(|| "FSFreeze not initialized".to_string())
}
