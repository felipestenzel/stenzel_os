//! Dual Boot Detection and Configuration for Stenzel OS.
//!
//! Detects existing operating systems on the machine and configures
//! the bootloader for dual-boot scenarios.
//!
//! Features:
//! - Windows detection (all versions)
//! - Linux distribution detection
//! - macOS detection
//! - BSD detection
//! - EFI boot entry management
//! - GRUB/systemd-boot configuration
//! - Boot order configuration
//! - Safe boot entry preservation

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use spin::{Mutex, Once};

// ============================================================================
// OS Detection Types
// ============================================================================

/// Detected operating system type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsType {
    /// Microsoft Windows
    Windows,
    /// Linux distribution
    Linux,
    /// Apple macOS
    MacOs,
    /// FreeBSD
    FreeBsd,
    /// OpenBSD
    OpenBsd,
    /// NetBSD
    NetBsd,
    /// Haiku
    Haiku,
    /// ChromeOS/ChromiumOS
    ChromeOs,
    /// Unknown OS
    Unknown,
}

/// Windows version
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsVersion {
    Windows7,
    Windows8,
    Windows81,
    Windows10,
    Windows11,
    WindowsServer,
    Unknown,
}

/// Linux distribution
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinuxDistro {
    Ubuntu,
    Debian,
    Fedora,
    Arch,
    Manjaro,
    OpenSuse,
    Mint,
    PopOs,
    ElementaryOs,
    Gentoo,
    Slackware,
    RedHat,
    CentOs,
    Rocky,
    Alma,
    Void,
    Alpine,
    NixOs,
    Unknown,
}

/// Boot mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootMode {
    /// Legacy BIOS boot
    Bios,
    /// UEFI boot
    Uefi,
    /// UEFI with Secure Boot
    UefiSecureBoot,
}

/// Partition type for OS
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionScheme {
    /// Master Boot Record
    Mbr,
    /// GUID Partition Table
    Gpt,
}

// ============================================================================
// Detected OS Information
// ============================================================================

/// Information about a detected operating system
#[derive(Debug, Clone)]
pub struct DetectedOs {
    /// OS type
    pub os_type: OsType,
    /// OS name (display name)
    pub name: String,
    /// Version string
    pub version: String,
    /// Architecture
    pub arch: String,
    /// Boot partition path (e.g., /dev/sda1)
    pub boot_partition: String,
    /// Root partition path
    pub root_partition: String,
    /// EFI partition path (if UEFI)
    pub efi_partition: Option<String>,
    /// Boot mode used
    pub boot_mode: BootMode,
    /// Partition scheme
    pub partition_scheme: PartitionScheme,
    /// Boot loader type
    pub bootloader: BootloaderType,
    /// Is this the current default boot?
    pub is_default: bool,
    /// Additional metadata
    pub metadata: BTreeMap<String, String>,
}

/// Bootloader type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootloaderType {
    /// Windows Boot Manager
    WindowsBoot,
    /// GRUB/GRUB2
    Grub,
    /// systemd-boot
    SystemdBoot,
    /// rEFInd
    Refind,
    /// SYSLINUX
    Syslinux,
    /// LILO
    Lilo,
    /// Apple Boot
    AppleBoot,
    /// BSD boot
    BsdBoot,
    /// Unknown
    Unknown,
}

// ============================================================================
// Detection Logic
// ============================================================================

/// Dual boot detector
pub struct DualBootDetector {
    /// Detected operating systems
    detected_os: Vec<DetectedOs>,
    /// Current boot mode
    current_boot_mode: BootMode,
    /// EFI system partition
    efi_partition: Option<String>,
    /// Detection completed
    scan_complete: bool,
}

impl DualBootDetector {
    /// Create new detector
    pub fn new() -> Self {
        Self {
            detected_os: Vec::new(),
            current_boot_mode: BootMode::Bios,
            efi_partition: None,
            scan_complete: false,
        }
    }

    /// Detect current boot mode
    pub fn detect_boot_mode(&mut self) {
        // Check for EFI variables presence
        // In real implementation, check /sys/firmware/efi
        // For now, check if EFI partition exists

        // Check UEFI by looking for EFI system partition
        let has_efi = self.find_efi_partition().is_some();

        self.current_boot_mode = if has_efi {
            // Check for secure boot
            if self.check_secure_boot() {
                BootMode::UefiSecureBoot
            } else {
                BootMode::Uefi
            }
        } else {
            BootMode::Bios
        };
    }

    /// Find EFI system partition
    fn find_efi_partition(&mut self) -> Option<String> {
        // Look for EFI System Partition (ESP)
        // GPT partition type GUID: C12A7328-F81F-11D2-BA4B-00A0C93EC93B

        // In real implementation, scan GPT partitions
        // For demonstration, return placeholder
        let esp = self.scan_for_esp();
        self.efi_partition = esp.clone();
        esp
    }

    /// Scan for EFI System Partition
    fn scan_for_esp(&self) -> Option<String> {
        // Real implementation would:
        // 1. Read GPT from all disks
        // 2. Find partition with ESP type GUID
        // 3. Verify FAT32 filesystem

        // Placeholder - in real scenario, scan actual partitions
        None
    }

    /// Check if secure boot is enabled
    fn check_secure_boot(&self) -> bool {
        // Check EFI variable for secure boot status
        // SecureBoot-8be4df61-93ca-11d2-aa0d-00e098032b8c
        false
    }

    /// Perform full system scan for operating systems
    pub fn scan(&mut self) -> Result<(), DualBootError> {
        self.detect_boot_mode();

        // Get all partitions
        let partitions = self.enumerate_partitions()?;

        // Scan each partition for OS signatures
        for partition in &partitions {
            if let Some(os) = self.probe_partition(partition) {
                self.detected_os.push(os);
            }
        }

        self.scan_complete = true;
        Ok(())
    }

    /// Enumerate all disk partitions
    fn enumerate_partitions(&self) -> Result<Vec<PartitionInfo>, DualBootError> {
        // Real implementation would use storage subsystem
        // Placeholder for demonstration
        Ok(Vec::new())
    }

    /// Probe a partition for operating system
    fn probe_partition(&self, partition: &PartitionInfo) -> Option<DetectedOs> {
        // Try different probes in order
        if let Some(os) = self.probe_windows(partition) {
            return Some(os);
        }
        if let Some(os) = self.probe_linux(partition) {
            return Some(os);
        }
        if let Some(os) = self.probe_macos(partition) {
            return Some(os);
        }
        if let Some(os) = self.probe_bsd(partition) {
            return Some(os);
        }
        None
    }

    /// Probe for Windows installation
    fn probe_windows(&self, partition: &PartitionInfo) -> Option<DetectedOs> {
        // Check for Windows signatures:
        // - NTFS filesystem
        // - /Windows/System32 directory
        // - /bootmgr or /EFI/Microsoft/Boot

        // Check filesystem type
        if partition.fs_type != FsType::Ntfs {
            return None;
        }

        // Look for Windows directory
        let has_windows_dir = self.check_path_exists(partition, "/Windows/System32");
        if !has_windows_dir {
            return None;
        }

        // Determine Windows version from registry/version info
        let version = self.detect_windows_version(partition);
        let version_name = match version {
            WindowsVersion::Windows11 => "Windows 11",
            WindowsVersion::Windows10 => "Windows 10",
            WindowsVersion::Windows81 => "Windows 8.1",
            WindowsVersion::Windows8 => "Windows 8",
            WindowsVersion::Windows7 => "Windows 7",
            WindowsVersion::WindowsServer => "Windows Server",
            WindowsVersion::Unknown => "Windows",
        };

        Some(DetectedOs {
            os_type: OsType::Windows,
            name: version_name.to_string(),
            version: version_name.to_string(),
            arch: "x86_64".to_string(),
            boot_partition: partition.path.clone(),
            root_partition: partition.path.clone(),
            efi_partition: self.efi_partition.clone(),
            boot_mode: self.current_boot_mode,
            partition_scheme: partition.scheme,
            bootloader: BootloaderType::WindowsBoot,
            is_default: false,
            metadata: BTreeMap::new(),
        })
    }

    /// Detect Windows version
    fn detect_windows_version(&self, _partition: &PartitionInfo) -> WindowsVersion {
        // In real implementation:
        // - Parse registry SOFTWARE hive
        // - Read /Windows/System32/ntoskrnl.exe version
        WindowsVersion::Unknown
    }

    /// Probe for Linux installation
    fn probe_linux(&self, partition: &PartitionInfo) -> Option<DetectedOs> {
        // Check for Linux signatures:
        // - ext2/ext3/ext4/btrfs/xfs filesystem
        // - /etc/os-release file
        // - /bin or /usr/bin directories

        match partition.fs_type {
            FsType::Ext2 | FsType::Ext3 | FsType::Ext4 | FsType::Btrfs | FsType::Xfs => {}
            _ => return None,
        }

        // Check for Linux FHS structure
        if !self.check_path_exists(partition, "/etc") {
            return None;
        }

        // Try to read /etc/os-release
        let (distro, name, version) = self.parse_os_release(partition);

        Some(DetectedOs {
            os_type: OsType::Linux,
            name,
            version,
            arch: "x86_64".to_string(),
            boot_partition: partition.path.clone(),
            root_partition: partition.path.clone(),
            efi_partition: self.efi_partition.clone(),
            boot_mode: self.current_boot_mode,
            partition_scheme: partition.scheme,
            bootloader: self.detect_linux_bootloader(partition),
            is_default: false,
            metadata: {
                let mut m = BTreeMap::new();
                m.insert("distro".to_string(), format!("{:?}", distro));
                m
            },
        })
    }

    /// Parse /etc/os-release file
    fn parse_os_release(&self, _partition: &PartitionInfo) -> (LinuxDistro, String, String) {
        // In real implementation, read and parse the file
        // Format: KEY="value" lines
        // Relevant keys: NAME, VERSION, ID, VERSION_ID, PRETTY_NAME

        (LinuxDistro::Unknown, "Linux".to_string(), "Unknown".to_string())
    }

    /// Detect Linux bootloader
    fn detect_linux_bootloader(&self, _partition: &PartitionInfo) -> BootloaderType {
        // Check for:
        // - /boot/grub or /boot/grub2 -> GRUB
        // - /boot/loader -> systemd-boot
        // - /boot/EFI/refind -> rEFInd
        // - /boot/syslinux -> SYSLINUX

        BootloaderType::Grub
    }

    /// Probe for macOS installation
    fn probe_macos(&self, partition: &PartitionInfo) -> Option<DetectedOs> {
        // Check for macOS signatures:
        // - HFS+ or APFS filesystem
        // - /System/Library directory

        match partition.fs_type {
            FsType::Hfs | FsType::Apfs => {}
            _ => return None,
        }

        if !self.check_path_exists(partition, "/System/Library") {
            return None;
        }

        Some(DetectedOs {
            os_type: OsType::MacOs,
            name: "macOS".to_string(),
            version: "Unknown".to_string(),
            arch: "x86_64".to_string(),
            boot_partition: partition.path.clone(),
            root_partition: partition.path.clone(),
            efi_partition: self.efi_partition.clone(),
            boot_mode: BootMode::Uefi,
            partition_scheme: PartitionScheme::Gpt,
            bootloader: BootloaderType::AppleBoot,
            is_default: false,
            metadata: BTreeMap::new(),
        })
    }

    /// Probe for BSD installation
    fn probe_bsd(&self, partition: &PartitionInfo) -> Option<DetectedOs> {
        // Check for BSD signatures:
        // - UFS/ZFS filesystem
        // - /etc/rc.conf file

        match partition.fs_type {
            FsType::Ufs | FsType::Zfs => {}
            _ => return None,
        }

        if !self.check_path_exists(partition, "/etc/rc.conf") {
            return None;
        }

        let os_type = self.detect_bsd_variant(partition);

        Some(DetectedOs {
            os_type,
            name: format!("{:?}", os_type),
            version: "Unknown".to_string(),
            arch: "x86_64".to_string(),
            boot_partition: partition.path.clone(),
            root_partition: partition.path.clone(),
            efi_partition: self.efi_partition.clone(),
            boot_mode: self.current_boot_mode,
            partition_scheme: partition.scheme,
            bootloader: BootloaderType::BsdBoot,
            is_default: false,
            metadata: BTreeMap::new(),
        })
    }

    /// Detect BSD variant
    fn detect_bsd_variant(&self, _partition: &PartitionInfo) -> OsType {
        // Check /etc/os-release or /etc/release for variant
        OsType::FreeBsd
    }

    /// Check if path exists on partition
    fn check_path_exists(&self, _partition: &PartitionInfo, _path: &str) -> bool {
        // In real implementation, mount partition and check path
        false
    }

    /// Get list of detected operating systems
    pub fn detected_systems(&self) -> &[DetectedOs] {
        &self.detected_os
    }

    /// Get current boot mode
    pub fn boot_mode(&self) -> BootMode {
        self.current_boot_mode
    }
}

impl Default for DualBootDetector {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Partition Information
// ============================================================================

/// Partition information for probing
#[derive(Debug, Clone)]
struct PartitionInfo {
    /// Device path (e.g., /dev/sda1)
    path: String,
    /// Filesystem type
    fs_type: FsType,
    /// Partition scheme
    scheme: PartitionScheme,
    /// Size in bytes
    size: u64,
    /// GPT type GUID (if GPT)
    type_guid: Option<[u8; 16]>,
    /// Label
    label: Option<String>,
}

/// Filesystem types for probing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FsType {
    Ntfs,
    Fat32,
    Ext2,
    Ext3,
    Ext4,
    Btrfs,
    Xfs,
    Hfs,
    Apfs,
    Ufs,
    Zfs,
    Unknown,
}

// ============================================================================
// Boot Entry Management
// ============================================================================

/// EFI boot entry
#[derive(Debug, Clone)]
pub struct EfiBootEntry {
    /// Boot number (e.g., 0000, 0001)
    pub boot_num: u16,
    /// Description
    pub description: String,
    /// Device path
    pub device_path: String,
    /// Is active
    pub active: bool,
    /// Optional data
    pub optional_data: Option<Vec<u8>>,
}

/// Boot entry manager for EFI systems
pub struct BootEntryManager {
    /// Current boot entries
    entries: Vec<EfiBootEntry>,
    /// Boot order
    boot_order: Vec<u16>,
    /// Next boot number
    next_boot_num: u16,
}

impl BootEntryManager {
    /// Create new manager
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            boot_order: Vec::new(),
            next_boot_num: 0,
        }
    }

    /// Read current EFI boot entries
    pub fn read_entries(&mut self) -> Result<(), DualBootError> {
        // Read from EFI variables:
        // - BootOrder
        // - Boot0000, Boot0001, etc.

        // In real implementation, read EFI variables via runtime services
        self.entries.clear();

        // Find highest boot number
        self.next_boot_num = self.entries.iter()
            .map(|e| e.boot_num)
            .max()
            .map(|n| n + 1)
            .unwrap_or(0);

        Ok(())
    }

    /// Add new boot entry
    pub fn add_entry(&mut self, description: &str, device_path: &str) -> Result<u16, DualBootError> {
        let boot_num = self.next_boot_num;
        self.next_boot_num += 1;

        let entry = EfiBootEntry {
            boot_num,
            description: description.to_string(),
            device_path: device_path.to_string(),
            active: true,
            optional_data: None,
        };

        self.entries.push(entry);
        self.boot_order.push(boot_num);

        Ok(boot_num)
    }

    /// Remove boot entry
    pub fn remove_entry(&mut self, boot_num: u16) -> Result<(), DualBootError> {
        self.entries.retain(|e| e.boot_num != boot_num);
        self.boot_order.retain(|&n| n != boot_num);
        Ok(())
    }

    /// Set boot order
    pub fn set_boot_order(&mut self, order: Vec<u16>) -> Result<(), DualBootError> {
        // Validate all entries exist
        for &num in &order {
            if !self.entries.iter().any(|e| e.boot_num == num) {
                return Err(DualBootError::InvalidBootEntry);
            }
        }
        self.boot_order = order;
        Ok(())
    }

    /// Get boot order
    pub fn boot_order(&self) -> &[u16] {
        &self.boot_order
    }

    /// Set default boot entry
    pub fn set_default(&mut self, boot_num: u16) -> Result<(), DualBootError> {
        // Move entry to front of boot order
        self.boot_order.retain(|&n| n != boot_num);
        self.boot_order.insert(0, boot_num);
        Ok(())
    }

    /// Write changes to EFI variables
    pub fn commit(&self) -> Result<(), DualBootError> {
        // Write to EFI variables via runtime services
        // This requires EFI runtime services to be available
        Ok(())
    }

    /// Get all entries
    pub fn entries(&self) -> &[EfiBootEntry] {
        &self.entries
    }
}

impl Default for BootEntryManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Bootloader Configuration
// ============================================================================

/// GRUB configuration generator
pub struct GrubConfigGenerator {
    /// Menu entries
    entries: Vec<GrubEntry>,
    /// Default entry index
    default_entry: usize,
    /// Timeout in seconds
    timeout: u32,
    /// Theme path
    theme: Option<String>,
}

/// GRUB menu entry
#[derive(Debug, Clone)]
pub struct GrubEntry {
    /// Entry title
    pub title: String,
    /// OS type
    pub os_type: OsType,
    /// Kernel path (for Linux)
    pub kernel: Option<String>,
    /// Initrd path (for Linux)
    pub initrd: Option<String>,
    /// Boot parameters
    pub params: String,
    /// Chainload target (for Windows/other)
    pub chainload: Option<String>,
    /// Is submenu
    pub is_submenu: bool,
}

impl GrubConfigGenerator {
    /// Create new generator
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            default_entry: 0,
            timeout: 5,
            theme: None,
        }
    }

    /// Add boot entry
    pub fn add_entry(&mut self, entry: GrubEntry) {
        self.entries.push(entry);
    }

    /// Add Stenzel OS entry
    pub fn add_stenzel_entry(&mut self, kernel_path: &str, initrd_path: &str, root_uuid: &str) {
        self.entries.push(GrubEntry {
            title: "Stenzel OS".to_string(),
            os_type: OsType::Linux,
            kernel: Some(kernel_path.to_string()),
            initrd: Some(initrd_path.to_string()),
            params: format!("root=UUID={} ro quiet splash", root_uuid),
            chainload: None,
            is_submenu: false,
        });
    }

    /// Add Windows chainload entry
    pub fn add_windows_entry(&mut self, name: &str, efi_path: &str) {
        self.entries.push(GrubEntry {
            title: name.to_string(),
            os_type: OsType::Windows,
            kernel: None,
            initrd: None,
            params: String::new(),
            chainload: Some(efi_path.to_string()),
            is_submenu: false,
        });
    }

    /// Add Linux chainload entry
    pub fn add_linux_chainload(&mut self, name: &str, efi_path: &str) {
        self.entries.push(GrubEntry {
            title: name.to_string(),
            os_type: OsType::Linux,
            kernel: None,
            initrd: None,
            params: String::new(),
            chainload: Some(efi_path.to_string()),
            is_submenu: false,
        });
    }

    /// Set default entry
    pub fn set_default(&mut self, index: usize) {
        if index < self.entries.len() {
            self.default_entry = index;
        }
    }

    /// Set timeout
    pub fn set_timeout(&mut self, seconds: u32) {
        self.timeout = seconds;
    }

    /// Set theme
    pub fn set_theme(&mut self, theme_path: &str) {
        self.theme = Some(theme_path.to_string());
    }

    /// Generate grub.cfg content
    pub fn generate(&self) -> String {
        let mut config = String::new();

        // Header
        config.push_str("# GRUB Configuration - Generated by Stenzel OS Installer\n");
        config.push_str("# Do not edit manually unless you know what you're doing\n\n");

        // Settings
        config.push_str(&format!("set timeout={}\n", self.timeout));
        config.push_str(&format!("set default={}\n", self.default_entry));
        config.push_str("set gfxmode=auto\n");
        config.push_str("insmod all_video\n");
        config.push_str("insmod gfxterm\n");
        config.push_str("terminal_output gfxterm\n\n");

        // Theme if set
        if let Some(ref theme) = self.theme {
            config.push_str(&format!("insmod gfxmenu\n"));
            config.push_str(&format!("loadfont unicode\n"));
            config.push_str(&format!("set theme=\"{}\"\n\n", theme));
        }

        // Menu entries
        for entry in &self.entries {
            config.push_str(&self.generate_entry(entry));
            config.push_str("\n");
        }

        config
    }

    /// Generate single entry
    fn generate_entry(&self, entry: &GrubEntry) -> String {
        let mut s = String::new();

        s.push_str(&format!("menuentry \"{}\" {{\n", entry.title));

        if let Some(ref chainload) = entry.chainload {
            // Chainload entry (Windows, other Linux, etc.)
            match entry.os_type {
                OsType::Windows => {
                    s.push_str("    insmod part_gpt\n");
                    s.push_str("    insmod fat\n");
                    s.push_str("    insmod chain\n");
                    s.push_str(&format!("    chainloader {}\n", chainload));
                }
                _ => {
                    s.push_str("    insmod chain\n");
                    s.push_str(&format!("    chainloader {}\n", chainload));
                }
            }
        } else if let Some(ref kernel) = entry.kernel {
            // Direct boot entry (our kernel)
            s.push_str("    insmod gzio\n");
            s.push_str("    insmod part_gpt\n");
            s.push_str("    insmod ext2\n");
            s.push_str(&format!("    linux {} {}\n", kernel, entry.params));
            if let Some(ref initrd) = entry.initrd {
                s.push_str(&format!("    initrd {}\n", initrd));
            }
        }

        s.push_str("}\n");
        s
    }

    /// Write configuration to file
    pub fn write_to(&self, _path: &str) -> Result<(), DualBootError> {
        let _config = self.generate();
        // In real implementation, write to filesystem
        Ok(())
    }
}

impl Default for GrubConfigGenerator {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// systemd-boot Configuration
// ============================================================================

/// systemd-boot configuration generator
pub struct SystemdBootConfigGenerator {
    /// Loader configuration
    timeout: u32,
    /// Default entry
    default_entry: String,
    /// Editor enabled
    editor: bool,
    /// Console mode
    console_mode: String,
    /// Boot entries
    entries: Vec<SystemdBootEntry>,
}

/// systemd-boot entry
#[derive(Debug, Clone)]
pub struct SystemdBootEntry {
    /// Entry filename (without .conf)
    pub filename: String,
    /// Title
    pub title: String,
    /// Linux kernel path
    pub linux: Option<String>,
    /// Initrd path
    pub initrd: Option<String>,
    /// Boot options
    pub options: String,
    /// EFI binary for chainloading
    pub efi: Option<String>,
}

impl SystemdBootConfigGenerator {
    /// Create new generator
    pub fn new() -> Self {
        Self {
            timeout: 5,
            default_entry: String::from("stenzel"),
            editor: true,
            console_mode: String::from("auto"),
            entries: Vec::new(),
        }
    }

    /// Add entry
    pub fn add_entry(&mut self, entry: SystemdBootEntry) {
        self.entries.push(entry);
    }

    /// Add Stenzel OS entry
    pub fn add_stenzel_entry(&mut self, root_uuid: &str) {
        self.entries.push(SystemdBootEntry {
            filename: "stenzel".to_string(),
            title: "Stenzel OS".to_string(),
            linux: Some("/vmlinuz-stenzel".to_string()),
            initrd: Some("/initramfs-stenzel.img".to_string()),
            options: format!("root=UUID={} ro quiet splash", root_uuid),
            efi: None,
        });
    }

    /// Add Windows entry
    pub fn add_windows_entry(&mut self, name: &str) {
        self.entries.push(SystemdBootEntry {
            filename: "windows".to_string(),
            title: name.to_string(),
            linux: None,
            initrd: None,
            options: String::new(),
            efi: Some("/EFI/Microsoft/Boot/bootmgfw.efi".to_string()),
        });
    }

    /// Set default entry
    pub fn set_default(&mut self, filename: &str) {
        self.default_entry = filename.to_string();
    }

    /// Set timeout
    pub fn set_timeout(&mut self, seconds: u32) {
        self.timeout = seconds;
    }

    /// Generate loader.conf content
    pub fn generate_loader_conf(&self) -> String {
        let mut config = String::new();

        config.push_str("# systemd-boot loader configuration\n");
        config.push_str("# Generated by Stenzel OS Installer\n\n");

        config.push_str(&format!("default {}.conf\n", self.default_entry));
        config.push_str(&format!("timeout {}\n", self.timeout));
        config.push_str(&format!("console-mode {}\n", self.console_mode));
        config.push_str(&format!("editor {}\n", if self.editor { "yes" } else { "no" }));

        config
    }

    /// Generate entry file content
    pub fn generate_entry(&self, entry: &SystemdBootEntry) -> String {
        let mut config = String::new();

        config.push_str(&format!("title   {}\n", entry.title));

        if let Some(ref linux) = entry.linux {
            config.push_str(&format!("linux   {}\n", linux));
        }

        if let Some(ref initrd) = entry.initrd {
            config.push_str(&format!("initrd  {}\n", initrd));
        }

        if let Some(ref efi) = entry.efi {
            config.push_str(&format!("efi     {}\n", efi));
        }

        if !entry.options.is_empty() {
            config.push_str(&format!("options {}\n", entry.options));
        }

        config
    }

    /// Get all entries
    pub fn entries(&self) -> &[SystemdBootEntry] {
        &self.entries
    }
}

impl Default for SystemdBootConfigGenerator {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Error Types
// ============================================================================

/// Dual boot error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DualBootError {
    /// Scan not completed
    ScanNotComplete,
    /// No EFI partition found
    NoEfiPartition,
    /// Invalid boot entry
    InvalidBootEntry,
    /// Bootloader not supported
    BootloaderNotSupported,
    /// Write failed
    WriteFailed,
    /// Read failed
    ReadFailed,
    /// Partition not found
    PartitionNotFound,
    /// Filesystem not supported
    FilesystemNotSupported,
}

// ============================================================================
// Global Instance
// ============================================================================

static DUAL_BOOT_DETECTOR: Once<Mutex<DualBootDetector>> = Once::new();

/// Initialize dual boot detection
pub fn init() {
    DUAL_BOOT_DETECTOR.call_once(|| Mutex::new(DualBootDetector::new()));
    crate::kprintln!("dualboot: initialized");
}

/// Get dual boot detector
pub fn detector() -> &'static Mutex<DualBootDetector> {
    DUAL_BOOT_DETECTOR.get().expect("Dual boot detector not initialized")
}

/// Perform OS scan
pub fn scan() -> Result<(), DualBootError> {
    detector().lock().scan()
}

/// Get detected operating systems
pub fn get_detected_systems() -> Vec<DetectedOs> {
    detector().lock().detected_systems().to_vec()
}

/// Get current boot mode
pub fn get_boot_mode() -> BootMode {
    detector().lock().boot_mode()
}

/// Create GRUB configuration for dual boot
pub fn create_grub_config(stenzel_kernel: &str, stenzel_initrd: &str, root_uuid: &str) -> String {
    let systems = get_detected_systems();
    let mut gen = GrubConfigGenerator::new();

    // Add Stenzel OS first
    gen.add_stenzel_entry(stenzel_kernel, stenzel_initrd, root_uuid);

    // Add detected systems
    for os in systems {
        match os.os_type {
            OsType::Windows => {
                gen.add_windows_entry(&os.name, "/EFI/Microsoft/Boot/bootmgfw.efi");
            }
            OsType::Linux => {
                // Chainload other Linux distributions
                if let Some(efi) = os.efi_partition {
                    gen.add_linux_chainload(&os.name, &format!("{}/EFI/{}/grubx64.efi", efi, os.name.to_lowercase()));
                }
            }
            _ => {}
        }
    }

    gen.generate()
}

/// Create systemd-boot configuration for dual boot
pub fn create_systemd_boot_config(root_uuid: &str) -> (String, Vec<(String, String)>) {
    let systems = get_detected_systems();
    let mut gen = SystemdBootConfigGenerator::new();

    // Add Stenzel OS
    gen.add_stenzel_entry(root_uuid);

    // Add detected Windows
    for os in &systems {
        if os.os_type == OsType::Windows {
            gen.add_windows_entry(&os.name);
        }
    }

    // Generate all configs
    let loader_conf = gen.generate_loader_conf();
    let entries: Vec<(String, String)> = gen.entries().iter()
        .map(|e| (format!("{}.conf", e.filename), gen.generate_entry(e)))
        .collect();

    (loader_conf, entries)
}
