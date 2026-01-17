//! Windows Filesystem Path Translation
//!
//! Converts Windows-style paths to Unix paths and provides
//! drive letter mapping.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

/// Drive mapping configuration
#[derive(Debug, Clone)]
pub struct DriveMapping {
    /// Unix path for this drive
    pub unix_path: String,
    /// Whether this drive is read-only
    pub read_only: bool,
    /// Drive label
    pub label: String,
    /// Drive type
    pub drive_type: DriveType,
}

/// Drive types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriveType {
    Unknown,
    NoRootDir,
    Removable,
    Fixed,
    Remote,
    CdRom,
    RamDisk,
}

/// Filesystem translator
pub struct FsTranslator {
    /// Drive letter mappings (lowercase)
    drives: BTreeMap<char, DriveMapping>,
    /// Special folder mappings
    special_folders: BTreeMap<String, String>,
    /// Default drive (usually C)
    default_drive: char,
}

impl FsTranslator {
    pub fn new() -> Self {
        let mut translator = Self {
            drives: BTreeMap::new(),
            special_folders: BTreeMap::new(),
            default_drive: 'c',
        };

        // Set up default drive mappings
        translator.init_defaults();
        translator
    }

    /// Initialize default mappings
    fn init_defaults(&mut self) {
        // Map C: to root filesystem
        self.drives.insert('c', DriveMapping {
            unix_path: String::from("/mnt/c"),
            read_only: false,
            label: String::from("Stenzel OS"),
            drive_type: DriveType::Fixed,
        });

        // Map Z: to real root (for Wine compatibility)
        self.drives.insert('z', DriveMapping {
            unix_path: String::from("/"),
            read_only: true,
            label: String::from("Unix Root"),
            drive_type: DriveType::Fixed,
        });

        // Special Windows folders
        self.special_folders.insert(
            String::from("windows"),
            String::from("/mnt/c/Windows"),
        );
        self.special_folders.insert(
            String::from("system32"),
            String::from("/mnt/c/Windows/System32"),
        );
        self.special_folders.insert(
            String::from("programfiles"),
            String::from("/mnt/c/Program Files"),
        );
        self.special_folders.insert(
            String::from("programfiles(x86)"),
            String::from("/mnt/c/Program Files (x86)"),
        );
        self.special_folders.insert(
            String::from("users"),
            String::from("/mnt/c/Users"),
        );
        self.special_folders.insert(
            String::from("temp"),
            String::from("/tmp"),
        );
    }

    /// Add a drive mapping
    pub fn add_drive(&mut self, letter: char, mapping: DriveMapping) {
        self.drives.insert(letter.to_ascii_lowercase(), mapping);
    }

    /// Remove a drive mapping
    pub fn remove_drive(&mut self, letter: char) {
        self.drives.remove(&letter.to_ascii_lowercase());
    }

    /// Get drive mapping
    pub fn get_drive(&self, letter: char) -> Option<&DriveMapping> {
        self.drives.get(&letter.to_ascii_lowercase())
    }

    /// List all drives
    pub fn list_drives(&self) -> Vec<char> {
        self.drives.keys().cloned().collect()
    }

    /// Convert Windows path to Unix path
    pub fn to_unix_path(&self, windows_path: &str) -> String {
        let path = windows_path.trim();

        // Handle empty path
        if path.is_empty() {
            return String::from(".");
        }

        // Handle UNC paths (\\server\share\...)
        if path.starts_with("\\\\") || path.starts_with("//") {
            // Convert to /mnt/network/server/share/...
            let rest = &path[2..];
            let unix = rest.replace('\\', "/");
            return alloc::format!("/mnt/network/{}", unix);
        }

        // Handle drive letter paths (C:\...)
        let chars: Vec<char> = path.chars().collect();
        if chars.len() >= 2 && chars[1] == ':' {
            let drive = chars[0].to_ascii_lowercase();
            let rest = if chars.len() > 2 {
                &path[2..]
            } else {
                ""
            };

            if let Some(mapping) = self.drives.get(&drive) {
                let unix_rest = rest.replace('\\', "/");
                // Remove leading slash if mapping already ends with one
                let combined = if mapping.unix_path.ends_with('/') && unix_rest.starts_with('/') {
                    alloc::format!("{}{}", mapping.unix_path, &unix_rest[1..])
                } else if !mapping.unix_path.ends_with('/') && !unix_rest.starts_with('/') && !unix_rest.is_empty() {
                    alloc::format!("{}/{}", mapping.unix_path, unix_rest)
                } else {
                    alloc::format!("{}{}", mapping.unix_path, unix_rest)
                };
                return combined;
            } else {
                // Unknown drive, map to /mnt/X
                let unix_rest = rest.replace('\\', "/");
                return alloc::format!("/mnt/{}{}", drive, unix_rest);
            }
        }

        // Handle absolute paths without drive letter (\Windows\...)
        if path.starts_with('\\') || path.starts_with('/') {
            let drive_mapping = self.drives.get(&self.default_drive)
                .map(|m| m.unix_path.as_str())
                .unwrap_or("/mnt/c");
            let unix_rest = path.replace('\\', "/");
            return alloc::format!("{}{}", drive_mapping, unix_rest);
        }

        // Handle relative paths
        path.replace('\\', "/")
    }

    /// Convert Unix path to Windows path
    pub fn to_windows_path(&self, unix_path: &str) -> String {
        let path = unix_path.trim();

        // Handle empty path
        if path.is_empty() {
            return String::from(".");
        }

        // Check each drive mapping
        for (letter, mapping) in &self.drives {
            if path.starts_with(&mapping.unix_path) {
                let rest = &path[mapping.unix_path.len()..];
                let win_rest = rest.replace('/', "\\");
                return alloc::format!("{}:{}", letter.to_ascii_uppercase(), win_rest);
            }
        }

        // Check for /mnt/X format
        if path.starts_with("/mnt/") && path.len() >= 6 {
            let chars: Vec<char> = path.chars().collect();
            let drive = chars[5];
            if drive.is_ascii_alphabetic() {
                let rest = if path.len() > 6 { &path[6..] } else { "" };
                let win_rest = rest.replace('/', "\\");
                return alloc::format!("{}:{}", drive.to_ascii_uppercase(), win_rest);
            }
        }

        // Map to Z: (root)
        let win_path = path.replace('/', "\\");
        alloc::format!("Z:{}", win_path)
    }

    /// Normalize a Windows path
    pub fn normalize_windows_path(&self, path: &str) -> String {
        let mut result = Vec::new();
        let parts: Vec<&str> = path.split(|c| c == '\\' || c == '/')
            .filter(|s| !s.is_empty() && *s != ".")
            .collect();

        // Keep drive letter if present
        let start_idx = if parts.first().map(|s| s.ends_with(':')).unwrap_or(false) {
            result.push(parts[0]);
            1
        } else {
            0
        };

        for part in &parts[start_idx..] {
            if *part == ".." {
                if result.len() > start_idx {
                    result.pop();
                }
            } else {
                result.push(part);
            }
        }

        result.join("\\")
    }

    /// Check if path is absolute (Windows)
    pub fn is_absolute_windows(&self, path: &str) -> bool {
        let chars: Vec<char> = path.chars().collect();

        // UNC path
        if path.starts_with("\\\\") || path.starts_with("//") {
            return true;
        }

        // Drive letter path
        if chars.len() >= 2 && chars[1] == ':' {
            return true;
        }

        // Absolute without drive
        if path.starts_with('\\') || path.starts_with('/') {
            return true;
        }

        false
    }

    /// Get drive type
    pub fn get_drive_type(&self, letter: char) -> DriveType {
        self.drives.get(&letter.to_ascii_lowercase())
            .map(|m| m.drive_type)
            .unwrap_or(DriveType::NoRootDir)
    }

    /// Get volume information
    pub fn get_volume_info(&self, letter: char) -> Option<VolumeInfo> {
        let mapping = self.drives.get(&letter.to_ascii_lowercase())?;

        Some(VolumeInfo {
            label: mapping.label.clone(),
            serial_number: 0x12345678, // Fake serial
            max_component_length: 255,
            file_system_flags: 0x2F, // Case preserving, Unicode, etc.
            file_system_name: String::from("NTFS"), // Pretend to be NTFS
        })
    }

    /// Set default drive
    pub fn set_default_drive(&mut self, letter: char) -> bool {
        let lower = letter.to_ascii_lowercase();
        if self.drives.contains_key(&lower) {
            self.default_drive = lower;
            true
        } else {
            false
        }
    }

    /// Get default drive
    pub fn get_default_drive(&self) -> char {
        self.default_drive
    }

    /// Add special folder mapping
    pub fn add_special_folder(&mut self, name: &str, path: &str) {
        self.special_folders.insert(name.to_lowercase(), String::from(path));
    }

    /// Get special folder path
    pub fn get_special_folder(&self, name: &str) -> Option<&String> {
        self.special_folders.get(&name.to_lowercase())
    }
}

impl Default for FsTranslator {
    fn default() -> Self {
        Self::new()
    }
}

/// Volume information
#[derive(Debug, Clone)]
pub struct VolumeInfo {
    pub label: String,
    pub serial_number: u32,
    pub max_component_length: u32,
    pub file_system_flags: u32,
    pub file_system_name: String,
}

/// Global filesystem translator
static mut FS_TRANSLATOR: Option<FsTranslator> = None;

/// Initialize filesystem translation
pub fn init() {
    unsafe {
        FS_TRANSLATOR = Some(FsTranslator::new());
    }
    crate::kprintln!("winfs: filesystem translation initialized");
}

/// Get translator instance
pub fn translator() -> &'static mut FsTranslator {
    unsafe {
        FS_TRANSLATOR.as_mut().expect("FS translator not initialized")
    }
}

/// Convert Windows path to Unix path
pub fn windows_to_unix(path: &str) -> String {
    translator().to_unix_path(path)
}

/// Convert Unix path to Windows path
pub fn unix_to_windows(path: &str) -> String {
    translator().to_windows_path(path)
}

/// Normalize Windows path
pub fn normalize(path: &str) -> String {
    translator().normalize_windows_path(path)
}

/// Check if path is absolute
pub fn is_absolute(path: &str) -> bool {
    translator().is_absolute_windows(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_conversion() {
        let translator = FsTranslator::new();

        // Test drive letter paths
        assert_eq!(
            translator.to_unix_path("C:\\Windows\\System32"),
            "/mnt/c/Windows/System32"
        );

        // Test root path
        assert_eq!(
            translator.to_unix_path("C:\\"),
            "/mnt/c/"
        );

        // Test relative path
        assert_eq!(
            translator.to_unix_path("foo\\bar"),
            "foo/bar"
        );
    }
}
