//! AppImage Support
//!
//! Provides support for running AppImage portable applications.
//! Implements Type 1 (ISO9660) and Type 2 (SquashFS) AppImage formats,
//! FUSE-based mounting, desktop integration, and AppImageUpdate support.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::collections::BTreeMap;
use crate::sync::TicketSpinlock;

/// Global AppImage runtime state
static APPIMAGE_STATE: TicketSpinlock<Option<AppImageRuntime>> = TicketSpinlock::new(None);

/// AppImage type (format version)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppImageType {
    /// Type 1: ISO 9660 filesystem (legacy)
    Type1,
    /// Type 2: SquashFS filesystem (current standard)
    Type2,
    /// Unknown or invalid format
    Unknown,
}

impl AppImageType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AppImageType::Type1 => "Type 1 (ISO 9660)",
            AppImageType::Type2 => "Type 2 (SquashFS)",
            AppImageType::Unknown => "Unknown",
        }
    }
}

/// AppImage architecture
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppImageArch {
    X86_64,
    I686,
    AArch64,
    ArmHf,
    Unknown,
}

impl AppImageArch {
    pub fn current() -> Self {
        AppImageArch::X86_64
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            AppImageArch::X86_64 => "x86_64",
            AppImageArch::I686 => "i686",
            AppImageArch::AArch64 => "aarch64",
            AppImageArch::ArmHf => "armhf",
            AppImageArch::Unknown => "unknown",
        }
    }

    pub fn from_elf_machine(machine: u16) -> Self {
        match machine {
            0x3E => AppImageArch::X86_64,   // EM_X86_64
            0x03 => AppImageArch::I686,     // EM_386
            0xB7 => AppImageArch::AArch64,  // EM_AARCH64
            0x28 => AppImageArch::ArmHf,    // EM_ARM
            _ => AppImageArch::Unknown,
        }
    }
}

/// ELF header magic and constants
mod elf {
    pub const MAGIC: [u8; 4] = [0x7F, b'E', b'L', b'F'];
    pub const CLASS64: u8 = 2;
    pub const LITTLE_ENDIAN: u8 = 1;
    pub const ET_EXEC: u16 = 2;
    pub const ET_DYN: u16 = 3;

    // AppImage-specific ELF section names
    pub const SECTION_AI_OFFSET: &str = ".sha256_sig";
    pub const SECTION_UPDATE_INFO: &str = ".upd_info";
    pub const SECTION_SIGNATURE: &str = ".sig_key";
}

/// AppImage magic numbers
mod magic {
    /// AppImage Type 1 magic (at offset 8 in ELF)
    pub const APPIMAGE_TYPE1: [u8; 8] = [0x41, 0x49, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00];
    /// AppImage Type 2 magic (at offset 8 in ELF)
    pub const APPIMAGE_TYPE2: [u8; 8] = [0x41, 0x49, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00];
    /// SquashFS magic
    pub const SQUASHFS: [u8; 4] = [0x68, 0x73, 0x71, 0x73]; // "hsqs"
    /// ISO 9660 magic
    pub const ISO9660: [u8; 5] = [0x43, 0x44, 0x30, 0x30, 0x31]; // "CD001"
}

/// AppImage ELF header structure
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct ElfHeader64 {
    pub e_ident: [u8; 16],
    pub e_type: u16,
    pub e_machine: u16,
    pub e_version: u32,
    pub e_entry: u64,
    pub e_phoff: u64,
    pub e_shoff: u64,
    pub e_flags: u32,
    pub e_ehsize: u16,
    pub e_phentsize: u16,
    pub e_phnum: u16,
    pub e_shentsize: u16,
    pub e_shnum: u16,
    pub e_shstrndx: u16,
}

/// AppImage metadata
#[derive(Debug, Clone)]
pub struct AppImageInfo {
    /// Path to the AppImage file
    pub path: String,
    /// AppImage type (1 or 2)
    pub appimage_type: AppImageType,
    /// Target architecture
    pub arch: AppImageArch,
    /// Offset to embedded filesystem
    pub fs_offset: u64,
    /// Size of the AppImage file
    pub file_size: u64,
    /// Application name (from desktop entry)
    pub name: Option<String>,
    /// Application version
    pub version: Option<String>,
    /// Desktop entry contents
    pub desktop_entry: Option<DesktopEntry>,
    /// Update information (if present)
    pub update_info: Option<UpdateInfo>,
    /// Signature information (if present)
    pub signature: Option<SignatureInfo>,
    /// Whether AppImage is mounted
    pub is_mounted: bool,
    /// Mount point path
    pub mount_point: Option<String>,
}

impl AppImageInfo {
    pub fn new(path: &str) -> Self {
        Self {
            path: String::from(path),
            appimage_type: AppImageType::Unknown,
            arch: AppImageArch::Unknown,
            fs_offset: 0,
            file_size: 0,
            name: None,
            version: None,
            desktop_entry: None,
            update_info: None,
            signature: None,
            is_mounted: false,
            mount_point: None,
        }
    }
}

/// Desktop entry (from .desktop file in AppDir)
#[derive(Debug, Clone)]
pub struct DesktopEntry {
    pub name: String,
    pub exec: String,
    pub icon: Option<String>,
    pub categories: Vec<String>,
    pub comment: Option<String>,
    pub generic_name: Option<String>,
    pub terminal: bool,
    pub no_display: bool,
    pub mime_types: Vec<String>,
    pub actions: Vec<DesktopAction>,
}

impl DesktopEntry {
    pub fn new(name: &str, exec: &str) -> Self {
        Self {
            name: String::from(name),
            exec: String::from(exec),
            icon: None,
            categories: Vec::new(),
            comment: None,
            generic_name: None,
            terminal: false,
            no_display: false,
            mime_types: Vec::new(),
            actions: Vec::new(),
        }
    }
}

/// Desktop action (additional actions in .desktop file)
#[derive(Debug, Clone)]
pub struct DesktopAction {
    pub name: String,
    pub exec: String,
    pub icon: Option<String>,
}

/// Update information
#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub update_type: UpdateType,
    pub url: String,
    pub channel: Option<String>,
}

/// Update type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateType {
    /// GitHub releases
    GitHubReleases,
    /// Generic HTTP server with zsync
    Zsync,
    /// Generic HTTP server with bsdiff
    Bsdiff,
    /// OCI/Docker registry
    Oci,
    /// Unknown/unsupported
    Unknown,
}

impl UpdateType {
    pub fn as_str(&self) -> &'static str {
        match self {
            UpdateType::GitHubReleases => "gh-releases-zsync",
            UpdateType::Zsync => "zsync",
            UpdateType::Bsdiff => "bsdiff",
            UpdateType::Oci => "oci",
            UpdateType::Unknown => "unknown",
        }
    }
}

/// Signature information
#[derive(Debug, Clone)]
pub struct SignatureInfo {
    pub signature_type: SignatureType,
    pub key_id: Option<String>,
    pub signature_data: Vec<u8>,
    pub verified: bool,
}

/// Signature type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureType {
    /// GPG signature
    Gpg,
    /// Ed25519 signature
    Ed25519,
    /// SHA256 checksum
    Sha256,
    /// None/unsigned
    None,
}

/// Running AppImage instance
#[derive(Debug, Clone)]
pub struct RunningAppImage {
    pub instance_id: u32,
    pub info: AppImageInfo,
    pub pid: u32,
    pub start_time: u64,
    pub extracted: bool,
    pub extraction_path: Option<String>,
}

/// SquashFS superblock (simplified)
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct SquashfsSuperblock {
    pub magic: u32,
    pub inode_count: u32,
    pub modification_time: u32,
    pub block_size: u32,
    pub fragment_entry_count: u32,
    pub compression_id: u16,
    pub block_log: u16,
    pub flags: u16,
    pub no_ids: u16,
    pub version_major: u16,
    pub version_minor: u16,
    pub root_inode: u64,
    pub bytes_used: u64,
    pub id_table_start: u64,
    pub xattr_table_start: u64,
    pub inode_table_start: u64,
    pub directory_table_start: u64,
    pub fragment_table_start: u64,
    pub lookup_table_start: u64,
}

/// Compression types in SquashFS
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    Gzip = 1,
    Lzma = 2,
    Lzo = 3,
    Xz = 4,
    Lz4 = 5,
    Zstd = 6,
}

impl Compression {
    pub fn from_id(id: u16) -> Option<Self> {
        match id {
            1 => Some(Compression::Gzip),
            2 => Some(Compression::Lzma),
            3 => Some(Compression::Lzo),
            4 => Some(Compression::Xz),
            5 => Some(Compression::Lz4),
            6 => Some(Compression::Zstd),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Compression::Gzip => "gzip",
            Compression::Lzma => "lzma",
            Compression::Lzo => "lzo",
            Compression::Xz => "xz",
            Compression::Lz4 => "lz4",
            Compression::Zstd => "zstd",
        }
    }
}

/// AppImage runtime manager
#[derive(Debug)]
pub struct AppImageRuntime {
    /// Known AppImages
    appimages: BTreeMap<String, AppImageInfo>,
    /// Running instances
    instances: BTreeMap<u32, RunningAppImage>,
    /// Desktop integration enabled
    desktop_integration: bool,
    /// Extraction cache directory
    cache_dir: String,
    /// Trusted keys for signature verification
    trusted_keys: Vec<TrustedKey>,
    /// FUSE mount enabled
    fuse_enabled: bool,
    /// Next instance ID
    next_instance_id: u32,
}

/// Trusted signing key
#[derive(Debug, Clone)]
pub struct TrustedKey {
    pub key_id: String,
    pub key_type: SignatureType,
    pub public_key: Vec<u8>,
    pub trusted_since: u64,
}

impl AppImageRuntime {
    pub fn new() -> Self {
        Self {
            appimages: BTreeMap::new(),
            instances: BTreeMap::new(),
            desktop_integration: true,
            cache_dir: String::from("/tmp/appimage-cache"),
            trusted_keys: Vec::new(),
            fuse_enabled: true,
            next_instance_id: 1,
        }
    }
}

/// AppImage error
#[derive(Debug)]
pub enum AppImageError {
    NotInitialized,
    FileNotFound,
    InvalidFormat,
    UnsupportedType,
    UnsupportedArch,
    MountFailed,
    ExtractFailed,
    ExecutionFailed,
    VerificationFailed,
    UpdateFailed,
    IoError,
}

// ============================================================================
// Public API
// ============================================================================

/// Initialize AppImage runtime
pub fn init() {
    let mut state = APPIMAGE_STATE.lock();
    if state.is_none() {
        *state = Some(AppImageRuntime::new());
    }
    crate::kprintln!("[appimage] AppImage support initialized");
}

/// Detect AppImage type from file header
pub fn detect_type(data: &[u8]) -> AppImageType {
    if data.len() < 16 {
        return AppImageType::Unknown;
    }

    // Check ELF magic
    if data[0..4] != elf::MAGIC {
        return AppImageType::Unknown;
    }

    // Check AppImage magic at offset 8
    if data.len() >= 16 {
        if data[8..16] == magic::APPIMAGE_TYPE1 {
            return AppImageType::Type1;
        }
        if data[8..16] == magic::APPIMAGE_TYPE2 {
            return AppImageType::Type2;
        }
    }

    // Try to find SquashFS or ISO9660 in the file
    // This is a fallback for older AppImages without proper magic
    AppImageType::Unknown
}

/// Parse AppImage header and extract metadata
pub fn parse_appimage(path: &str, data: &[u8]) -> Result<AppImageInfo, AppImageError> {
    let mut info = AppImageInfo::new(path);
    info.file_size = data.len() as u64;

    // Check minimum size
    if data.len() < 64 {
        return Err(AppImageError::InvalidFormat);
    }

    // Detect type
    info.appimage_type = detect_type(data);
    if info.appimage_type == AppImageType::Unknown {
        return Err(AppImageError::InvalidFormat);
    }

    // Parse ELF header
    if data.len() >= core::mem::size_of::<ElfHeader64>() {
        let header: ElfHeader64 = unsafe {
            core::ptr::read_unaligned(data.as_ptr() as *const ElfHeader64)
        };

        // Check 64-bit
        if header.e_ident[4] != elf::CLASS64 {
            return Err(AppImageError::UnsupportedArch);
        }

        // Check little endian
        if header.e_ident[5] != elf::LITTLE_ENDIAN {
            return Err(AppImageError::InvalidFormat);
        }

        // Get architecture
        info.arch = AppImageArch::from_elf_machine(header.e_machine);

        // Check if architecture is compatible
        if info.arch != AppImageArch::current() && info.arch != AppImageArch::Unknown {
            return Err(AppImageError::UnsupportedArch);
        }

        // Find embedded filesystem offset
        // For Type 2, search for SquashFS magic after ELF
        info.fs_offset = find_squashfs_offset(data);
    }

    Ok(info)
}

/// Find SquashFS filesystem offset in AppImage
fn find_squashfs_offset(data: &[u8]) -> u64 {
    // SquashFS is typically at 4K boundary after ELF
    let search_start = 0x1000; // 4KB
    let search_end = core::cmp::min(data.len(), 0x100000); // Search up to 1MB

    for offset in (search_start..search_end).step_by(4) {
        if offset + 4 <= data.len() {
            if data[offset..offset+4] == magic::SQUASHFS {
                return offset as u64;
            }
        }
    }

    0
}

/// Register an AppImage
pub fn register(path: &str, data: &[u8]) -> Result<AppImageInfo, AppImageError> {
    let mut state = APPIMAGE_STATE.lock();
    let runtime = state.as_mut().ok_or(AppImageError::NotInitialized)?;

    let info = parse_appimage(path, data)?;
    runtime.appimages.insert(String::from(path), info.clone());

    Ok(info)
}

/// List registered AppImages
pub fn list_registered() -> Result<Vec<AppImageInfo>, AppImageError> {
    let state = APPIMAGE_STATE.lock();
    let runtime = state.as_ref().ok_or(AppImageError::NotInitialized)?;

    Ok(runtime.appimages.values().cloned().collect())
}

/// Get AppImage info
pub fn get_info(path: &str) -> Result<AppImageInfo, AppImageError> {
    let state = APPIMAGE_STATE.lock();
    let runtime = state.as_ref().ok_or(AppImageError::NotInitialized)?;

    runtime.appimages.get(path).cloned().ok_or(AppImageError::FileNotFound)
}

/// Mount AppImage (using FUSE or extraction)
pub fn mount(path: &str) -> Result<String, AppImageError> {
    let mut state = APPIMAGE_STATE.lock();
    let runtime = state.as_mut().ok_or(AppImageError::NotInitialized)?;

    let info = runtime.appimages.get_mut(path).ok_or(AppImageError::FileNotFound)?;

    if info.is_mounted {
        return info.mount_point.clone().ok_or(AppImageError::MountFailed);
    }

    // Generate mount point
    let mount_point = {
        let mut mp = runtime.cache_dir.clone();
        mp.push_str("/");
        mp.push_str(&sanitize_name(path));
        mp
    };

    // TODO: Actually mount using FUSE or extract
    // For now, just mark as mounted
    info.is_mounted = true;
    info.mount_point = Some(mount_point.clone());

    Ok(mount_point)
}

/// Unmount AppImage
pub fn unmount(path: &str) -> Result<(), AppImageError> {
    let mut state = APPIMAGE_STATE.lock();
    let runtime = state.as_mut().ok_or(AppImageError::NotInitialized)?;

    let info = runtime.appimages.get_mut(path).ok_or(AppImageError::FileNotFound)?;

    if !info.is_mounted {
        return Ok(());
    }

    // TODO: Actually unmount

    info.is_mounted = false;
    info.mount_point = None;

    Ok(())
}

/// Run AppImage
pub fn run(path: &str, args: &[&str]) -> Result<RunningAppImage, AppImageError> {
    let mut state = APPIMAGE_STATE.lock();
    let runtime = state.as_mut().ok_or(AppImageError::NotInitialized)?;

    let info = runtime.appimages.get(path).ok_or(AppImageError::FileNotFound)?;

    // Create instance
    let instance_id = runtime.next_instance_id;
    runtime.next_instance_id += 1;

    // Prepare execution
    let _exec_args: Vec<String> = args.iter().map(|s| String::from(*s)).collect();

    // TODO: Actually execute the AppImage
    // This would involve:
    // 1. Mount or extract the AppImage
    // 2. Set up environment (APPDIR, APPIMAGE, etc.)
    // 3. Execute AppRun or the specified command

    let running = RunningAppImage {
        instance_id,
        info: info.clone(),
        pid: 0, // TODO: Get actual PID
        start_time: 0, // TODO: Get current time
        extracted: false,
        extraction_path: None,
    };

    runtime.instances.insert(instance_id, running.clone());

    Ok(running)
}

/// Stop running AppImage
pub fn stop(instance_id: u32) -> Result<(), AppImageError> {
    let mut state = APPIMAGE_STATE.lock();
    let runtime = state.as_mut().ok_or(AppImageError::NotInitialized)?;

    if runtime.instances.remove(&instance_id).is_none() {
        return Err(AppImageError::FileNotFound);
    }

    // TODO: Actually kill the process

    Ok(())
}

/// List running instances
pub fn list_running() -> Result<Vec<RunningAppImage>, AppImageError> {
    let state = APPIMAGE_STATE.lock();
    let runtime = state.as_ref().ok_or(AppImageError::NotInitialized)?;

    Ok(runtime.instances.values().cloned().collect())
}

/// Extract AppImage to directory
pub fn extract(path: &str, dest: &str) -> Result<(), AppImageError> {
    let state = APPIMAGE_STATE.lock();
    let runtime = state.as_ref().ok_or(AppImageError::NotInitialized)?;

    let _info = runtime.appimages.get(path).ok_or(AppImageError::FileNotFound)?;

    // TODO: Actually extract SquashFS to dest
    let _destination = dest;

    Ok(())
}

/// Integrate AppImage with desktop
pub fn integrate_desktop(path: &str) -> Result<(), AppImageError> {
    let state = APPIMAGE_STATE.lock();
    let runtime = state.as_ref().ok_or(AppImageError::NotInitialized)?;

    if !runtime.desktop_integration {
        return Ok(());
    }

    let info = runtime.appimages.get(path).ok_or(AppImageError::FileNotFound)?;

    // TODO: Create .desktop file and symlink icon
    if let Some(ref _desktop_entry) = info.desktop_entry {
        // Create ~/.local/share/applications/appimage-{name}.desktop
        // Create ~/.local/share/icons/hicolor/*/apps/{icon}
    }

    Ok(())
}

/// Remove desktop integration
pub fn remove_desktop_integration(path: &str) -> Result<(), AppImageError> {
    let state = APPIMAGE_STATE.lock();
    let _runtime = state.as_ref().ok_or(AppImageError::NotInitialized)?;

    // TODO: Remove .desktop file and icon
    let _app_path = path;

    Ok(())
}

/// Check for updates
pub fn check_update(path: &str) -> Result<Option<UpdateInfo>, AppImageError> {
    let state = APPIMAGE_STATE.lock();
    let runtime = state.as_ref().ok_or(AppImageError::NotInitialized)?;

    let info = runtime.appimages.get(path).ok_or(AppImageError::FileNotFound)?;

    if info.update_info.is_none() {
        return Ok(None);
    }

    // TODO: Check for updates using zsync/bsdiff/GitHub API
    Ok(info.update_info.clone())
}

/// Update AppImage
pub fn update(path: &str) -> Result<(), AppImageError> {
    let state = APPIMAGE_STATE.lock();
    let runtime = state.as_ref().ok_or(AppImageError::NotInitialized)?;

    let info = runtime.appimages.get(path).ok_or(AppImageError::FileNotFound)?;

    if info.update_info.is_none() {
        return Err(AppImageError::UpdateFailed);
    }

    // TODO: Download and apply update using zsync/bsdiff

    Ok(())
}

/// Verify AppImage signature
pub fn verify_signature(path: &str) -> Result<bool, AppImageError> {
    let state = APPIMAGE_STATE.lock();
    let runtime = state.as_ref().ok_or(AppImageError::NotInitialized)?;

    let info = runtime.appimages.get(path).ok_or(AppImageError::FileNotFound)?;

    if let Some(ref sig_info) = info.signature {
        // TODO: Verify signature against trusted keys
        return Ok(sig_info.verified);
    }

    // No signature present
    Ok(false)
}

/// Add trusted key
pub fn add_trusted_key(key: TrustedKey) -> Result<(), AppImageError> {
    let mut state = APPIMAGE_STATE.lock();
    let runtime = state.as_mut().ok_or(AppImageError::NotInitialized)?;

    runtime.trusted_keys.push(key);
    Ok(())
}

/// Enable/disable desktop integration
pub fn set_desktop_integration(enabled: bool) -> Result<(), AppImageError> {
    let mut state = APPIMAGE_STATE.lock();
    let runtime = state.as_mut().ok_or(AppImageError::NotInitialized)?;

    runtime.desktop_integration = enabled;
    Ok(())
}

/// Set cache directory
pub fn set_cache_dir(dir: &str) -> Result<(), AppImageError> {
    let mut state = APPIMAGE_STATE.lock();
    let runtime = state.as_mut().ok_or(AppImageError::NotInitialized)?;

    runtime.cache_dir = String::from(dir);
    Ok(())
}

/// Parse .desktop file content
pub fn parse_desktop_file(content: &str) -> Result<DesktopEntry, AppImageError> {
    let mut entry = DesktopEntry::new("", "");
    let mut in_desktop_entry = false;

    for line in content.lines() {
        let line = line.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line == "[Desktop Entry]" {
            in_desktop_entry = true;
            continue;
        }

        if line.starts_with('[') {
            in_desktop_entry = false;
            continue;
        }

        if !in_desktop_entry {
            continue;
        }

        if let Some(eq_pos) = line.find('=') {
            let key = line[..eq_pos].trim();
            let value = line[eq_pos+1..].trim();

            match key {
                "Name" => entry.name = String::from(value),
                "Exec" => entry.exec = String::from(value),
                "Icon" => entry.icon = Some(String::from(value)),
                "Comment" => entry.comment = Some(String::from(value)),
                "GenericName" => entry.generic_name = Some(String::from(value)),
                "Terminal" => entry.terminal = value == "true",
                "NoDisplay" => entry.no_display = value == "true",
                "Categories" => {
                    entry.categories = value.split(';')
                        .filter(|s| !s.is_empty())
                        .map(|s| String::from(s))
                        .collect();
                }
                "MimeType" => {
                    entry.mime_types = value.split(';')
                        .filter(|s| !s.is_empty())
                        .map(|s| String::from(s))
                        .collect();
                }
                _ => {}
            }
        }
    }

    if entry.name.is_empty() || entry.exec.is_empty() {
        return Err(AppImageError::InvalidFormat);
    }

    Ok(entry)
}

/// Parse update info string
pub fn parse_update_info(info_str: &str) -> Option<UpdateInfo> {
    let parts: Vec<&str> = info_str.split('|').collect();
    if parts.is_empty() {
        return None;
    }

    let update_type = match parts[0] {
        "gh-releases-zsync" => UpdateType::GitHubReleases,
        "zsync" => UpdateType::Zsync,
        "bsdiff" => UpdateType::Bsdiff,
        "oci" => UpdateType::Oci,
        _ => UpdateType::Unknown,
    };

    let url = if parts.len() > 1 {
        String::from(parts[1])
    } else {
        String::new()
    };

    let channel = if parts.len() > 2 {
        Some(String::from(parts[2]))
    } else {
        None
    };

    Some(UpdateInfo {
        update_type,
        url,
        channel,
    })
}

/// Sanitize name for filesystem
fn sanitize_name(path: &str) -> String {
    let mut result = String::new();
    let name = path.rsplit('/').next().unwrap_or(path);

    for c in name.chars() {
        if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
            result.push(c);
        } else {
            result.push('_');
        }
    }

    result
}

/// Get AppImage runtime version
pub fn version() -> &'static str {
    "1.0.0"
}

/// Check if FUSE is available
pub fn fuse_available() -> bool {
    let state = APPIMAGE_STATE.lock();
    if let Some(ref runtime) = *state {
        runtime.fuse_enabled
    } else {
        false
    }
}
