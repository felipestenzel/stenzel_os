//! System File Copy
//!
//! Provides functionality for copying the OS to the installation target.

extern crate alloc;

use alloc::string::String;
use alloc::format;
use alloc::vec;
use alloc::vec::Vec;

/// Copy progress information
#[derive(Debug, Clone)]
pub struct CopyProgress {
    /// Total files to copy
    pub total_files: u64,
    /// Files copied so far
    pub files_copied: u64,
    /// Total bytes to copy
    pub total_bytes: u64,
    /// Bytes copied so far
    pub bytes_copied: u64,
    /// Current file being copied
    pub current_file: String,
    /// Progress percentage (0-100)
    pub percent: u8,
}

impl Default for CopyProgress {
    fn default() -> Self {
        Self {
            total_files: 0,
            files_copied: 0,
            total_bytes: 0,
            bytes_copied: 0,
            current_file: String::new(),
            percent: 0,
        }
    }
}

/// System file copier
pub struct SystemCopier {
    target_root: String,
    source_root: String,
    exclude_patterns: Vec<String>,
    progress: CopyProgress,
}

impl SystemCopier {
    /// Create a new system copier
    pub fn new(target_root: &str) -> Self {
        Self {
            target_root: String::from(target_root),
            source_root: String::from("/"),
            exclude_patterns: vec![
                String::from("/proc/*"),
                String::from("/sys/*"),
                String::from("/dev/*"),
                String::from("/tmp/*"),
                String::from("/run/*"),
                String::from("/mnt/*"),
                String::from("/media/*"),
                String::from("/live/*"),
                String::from("/lost+found"),
            ],
            progress: CopyProgress::default(),
        }
    }

    /// Set source root (default is /)
    pub fn set_source(&mut self, source: &str) {
        self.source_root = String::from(source);
    }

    /// Add exclusion pattern
    pub fn add_exclude(&mut self, pattern: &str) {
        self.exclude_patterns.push(String::from(pattern));
    }

    /// Get current progress
    pub fn progress(&self) -> &CopyProgress {
        &self.progress
    }

    /// Copy system with progress callback
    pub fn copy_with_progress<F>(&mut self, mut progress_callback: F) -> Result<(), String>
    where
        F: FnMut(u8),
    {
        // Phase 1: Calculate total size
        self.progress.total_bytes = self.calculate_total_size()?;
        self.progress.total_files = self.count_files()?;

        crate::kprintln!("Copying {} files ({} MB) to {}",
            self.progress.total_files,
            self.progress.total_bytes / 1024 / 1024,
            self.target_root
        );

        // Phase 2: Create directory structure
        self.create_directories()?;
        progress_callback(5);

        // Phase 3: Copy files
        self.copy_files(&mut progress_callback)?;

        // Phase 4: Copy special files (device nodes, etc.)
        self.create_special_files()?;
        progress_callback(95);

        // Phase 5: Set permissions
        self.set_permissions()?;
        progress_callback(100);

        crate::kprintln!("System copy complete");
        Ok(())
    }

    /// Calculate total size of files to copy
    fn calculate_total_size(&self) -> Result<u64, String> {
        let mut total = 0u64;

        // Walk directory tree
        for entry in walk_directory(&self.source_root) {
            if self.should_copy(&entry.path) {
                if entry.is_file {
                    total += entry.size;
                }
            }
        }

        Ok(total)
    }

    /// Count files to copy
    fn count_files(&self) -> Result<u64, String> {
        let mut count = 0u64;

        for entry in walk_directory(&self.source_root) {
            if self.should_copy(&entry.path) && entry.is_file {
                count += 1;
            }
        }

        Ok(count)
    }

    /// Check if a path should be copied
    fn should_copy(&self, path: &str) -> bool {
        for pattern in &self.exclude_patterns {
            if matches_pattern(path, pattern) {
                return false;
            }
        }
        true
    }

    /// Create directory structure
    fn create_directories(&self) -> Result<(), String> {
        let directories = [
            "/bin", "/boot", "/dev", "/etc", "/home", "/lib", "/lib64",
            "/mnt", "/opt", "/proc", "/root", "/run", "/sbin", "/srv",
            "/sys", "/tmp", "/usr", "/var",
            "/usr/bin", "/usr/lib", "/usr/lib64", "/usr/sbin", "/usr/share",
            "/usr/local", "/usr/local/bin", "/usr/local/lib",
            "/var/cache", "/var/lib", "/var/log", "/var/run", "/var/tmp",
            "/etc/stenzel", "/etc/pkg",
        ];

        for dir in &directories {
            let target = format!("{}{}", self.target_root, dir);
            create_directory(&target)?;
        }

        Ok(())
    }

    /// Copy files from source to target
    fn copy_files<F>(&mut self, progress_callback: &mut F) -> Result<(), String>
    where
        F: FnMut(u8),
    {
        let entries: Vec<DirEntry> = walk_directory(&self.source_root).collect();

        for entry in entries {
            if !self.should_copy(&entry.path) {
                continue;
            }

            let relative_path = entry.path.strip_prefix(&self.source_root)
                .unwrap_or(&entry.path);
            let target_path = format!("{}{}", self.target_root, relative_path);

            self.progress.current_file = String::from(relative_path);

            if entry.is_dir {
                create_directory(&target_path)?;
            } else if entry.is_file {
                copy_file(&entry.path, &target_path)?;
                self.progress.files_copied += 1;
                self.progress.bytes_copied += entry.size;
            } else if entry.is_symlink {
                copy_symlink(&entry.path, &target_path)?;
            }

            // Update progress
            if self.progress.total_bytes > 0 {
                self.progress.percent = (self.progress.bytes_copied * 90 / self.progress.total_bytes) as u8 + 5;
                progress_callback(self.progress.percent);
            }
        }

        Ok(())
    }

    /// Create special files (empty device nodes, etc.)
    fn create_special_files(&self) -> Result<(), String> {
        // Create essential device nodes
        let dev_dir = format!("{}/dev", self.target_root);

        // /dev/null
        create_device_node(&format!("{}/null", dev_dir), DeviceType::Char, 1, 3)?;

        // /dev/zero
        create_device_node(&format!("{}/zero", dev_dir), DeviceType::Char, 1, 5)?;

        // /dev/random
        create_device_node(&format!("{}/random", dev_dir), DeviceType::Char, 1, 8)?;

        // /dev/urandom
        create_device_node(&format!("{}/urandom", dev_dir), DeviceType::Char, 1, 9)?;

        // /dev/tty
        create_device_node(&format!("{}/tty", dev_dir), DeviceType::Char, 5, 0)?;

        // /dev/console
        create_device_node(&format!("{}/console", dev_dir), DeviceType::Char, 5, 1)?;

        // /dev/ptmx
        create_device_node(&format!("{}/ptmx", dev_dir), DeviceType::Char, 5, 2)?;

        // Create /dev/pts directory
        create_directory(&format!("{}/pts", dev_dir))?;

        // Create /dev/shm directory
        create_directory(&format!("{}/shm", dev_dir))?;

        Ok(())
    }

    /// Set permissions on copied files
    fn set_permissions(&self) -> Result<(), String> {
        // Set proper permissions on key directories
        set_permissions(&format!("{}/tmp", self.target_root), 0o1777)?;
        set_permissions(&format!("{}/var/tmp", self.target_root), 0o1777)?;
        set_permissions(&format!("{}/root", self.target_root), 0o700)?;

        // Set SUID on critical binaries
        let suid_binaries = [
            "/usr/bin/sudo",
            "/usr/bin/su",
            "/usr/bin/passwd",
        ];

        for binary in &suid_binaries {
            let path = format!("{}{}", self.target_root, binary);
            if file_exists(&path) {
                set_permissions(&path, 0o4755)?;
            }
        }

        Ok(())
    }
}

/// Copy system (convenience function)
pub fn copy_system(target_root: &str) -> Result<(), String> {
    let mut copier = SystemCopier::new(target_root);
    copier.copy_with_progress(|_| {})
}

/// Copy system from squashfs image
pub fn copy_from_squashfs(squashfs_path: &str, target_root: &str) -> Result<(), String> {
    // Mount squashfs temporarily
    let mount_point = "/tmp/squashfs_mount";
    create_directory(mount_point)?;
    mount_squashfs(squashfs_path, mount_point)?;

    // Copy from mounted squashfs
    let mut copier = SystemCopier::new(target_root);
    copier.set_source(mount_point);
    let result = copier.copy_with_progress(|_| {});

    // Unmount squashfs
    unmount(mount_point)?;

    result
}

// ============================================================================
// Directory entry for walking
// ============================================================================

/// Directory entry
struct DirEntry {
    path: String,
    is_file: bool,
    is_dir: bool,
    is_symlink: bool,
    size: u64,
}

/// Device type
enum DeviceType {
    Char,
    Block,
}

// ============================================================================
// Helper functions
// ============================================================================

fn walk_directory(path: &str) -> impl Iterator<Item = DirEntry> {
    // Would use VFS to walk directory tree
    // For now, return empty iterator
    core::iter::empty()
}

fn matches_pattern(path: &str, pattern: &str) -> bool {
    // Simple glob matching
    if pattern.ends_with("/*") {
        let prefix = &pattern[..pattern.len() - 2];
        path.starts_with(prefix)
    } else if pattern.ends_with('*') {
        let prefix = &pattern[..pattern.len() - 1];
        path.starts_with(prefix)
    } else {
        path == pattern
    }
}

fn create_directory(path: &str) -> Result<(), String> {
    crate::kprintln!("Creating directory: {}", path);
    Ok(())
}

fn copy_file(src: &str, dst: &str) -> Result<(), String> {
    // Would use VFS to copy file
    Ok(())
}

fn copy_symlink(src: &str, dst: &str) -> Result<(), String> {
    // Would read link target and create new symlink
    Ok(())
}

fn create_device_node(path: &str, dev_type: DeviceType, major: u32, minor: u32) -> Result<(), String> {
    crate::kprintln!("Creating device node: {} ({}, {})", path, major, minor);
    Ok(())
}

fn set_permissions(path: &str, mode: u32) -> Result<(), String> {
    // Would use VFS to set permissions
    Ok(())
}

fn file_exists(path: &str) -> bool {
    // Would check via VFS
    false
}

fn mount_squashfs(image: &str, mount_point: &str) -> Result<(), String> {
    crate::kprintln!("Mounting squashfs {} at {}", image, mount_point);
    Ok(())
}

fn unmount(mount_point: &str) -> Result<(), String> {
    crate::kprintln!("Unmounting {}", mount_point);
    Ok(())
}
