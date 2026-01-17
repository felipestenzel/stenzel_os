//! Package Database
//!
//! Tracks installed packages and their files.

use alloc::string::String;
use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use crate::sync::IrqSafeMutex;
use crate::util::{KResult, KError};
use super::{PackageMetadata, Version};

/// Installed package record
#[derive(Debug, Clone)]
pub struct InstalledPackage {
    /// Package metadata
    pub metadata: PackageMetadata,
    /// Installation timestamp
    pub install_date: u64,
    /// Install reason
    pub reason: InstallReason,
    /// Files installed by this package
    pub files: Vec<String>,
    /// Directories created by this package
    pub dirs: Vec<String>,
}

/// Why was this package installed?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallReason {
    /// Explicitly requested by user
    Explicit,
    /// Installed as a dependency
    Dependency,
}

/// Package database
struct PackageDatabase {
    /// Installed packages by name
    packages: BTreeMap<String, InstalledPackage>,
    /// File to package mapping (for conflict detection)
    file_owners: BTreeMap<String, String>,
}

impl PackageDatabase {
    const fn new() -> Self {
        Self {
            packages: BTreeMap::new(),
            file_owners: BTreeMap::new(),
        }
    }

    /// Add a package to the database
    fn add(&mut self, pkg: InstalledPackage) {
        // Register file ownership
        for file in &pkg.files {
            self.file_owners.insert(file.clone(), pkg.metadata.name.clone());
        }
        self.packages.insert(pkg.metadata.name.clone(), pkg);
    }

    /// Remove a package from the database
    fn remove(&mut self, name: &str) -> Option<InstalledPackage> {
        if let Some(pkg) = self.packages.remove(name) {
            // Remove file ownership
            for file in &pkg.files {
                self.file_owners.remove(file);
            }
            Some(pkg)
        } else {
            None
        }
    }

    /// Get a package by name
    fn get(&self, name: &str) -> Option<&InstalledPackage> {
        self.packages.get(name)
    }

    /// Check if a package is installed
    fn is_installed(&self, name: &str) -> bool {
        self.packages.contains_key(name)
    }

    /// Get all installed packages
    fn list(&self) -> Vec<InstalledPackage> {
        self.packages.values().cloned().collect()
    }

    /// Check if a file is owned by another package
    fn file_owner(&self, path: &str) -> Option<&str> {
        self.file_owners.get(path).map(|s| s.as_str())
    }

    /// Get packages that depend on a given package
    fn reverse_deps(&self, name: &str) -> Vec<String> {
        self.packages
            .values()
            .filter(|pkg| {
                pkg.metadata.dependencies.iter().any(|d| d.name == name)
            })
            .map(|pkg| pkg.metadata.name.clone())
            .collect()
    }

    /// Get packages with explicit install reason
    fn explicit_packages(&self) -> Vec<String> {
        self.packages
            .values()
            .filter(|pkg| pkg.reason == InstallReason::Explicit)
            .map(|pkg| pkg.metadata.name.clone())
            .collect()
    }

    /// Get orphaned packages (dependencies no longer needed)
    fn orphans(&self) -> Vec<String> {
        self.packages
            .values()
            .filter(|pkg| {
                pkg.reason == InstallReason::Dependency
                    && self.reverse_deps(&pkg.metadata.name).is_empty()
            })
            .map(|pkg| pkg.metadata.name.clone())
            .collect()
    }
}

/// Global package database
static DATABASE: IrqSafeMutex<PackageDatabase> = IrqSafeMutex::new(PackageDatabase::new());

/// Initialize the package database
pub fn init() -> KResult<()> {
    // In a full implementation, this would:
    // 1. Create /var/lib/spkg if it doesn't exist
    // 2. Load existing database from disk
    // 3. Verify database integrity

    crate::kprintln!("spkg: database initialized");
    Ok(())
}

/// Add an installed package to the database
pub fn register_package(
    metadata: PackageMetadata,
    files: Vec<String>,
    dirs: Vec<String>,
    reason: InstallReason,
) -> KResult<()> {
    let pkg = InstalledPackage {
        metadata,
        install_date: crate::time::realtime().tv_sec as u64,
        reason,
        files,
        dirs,
    };

    DATABASE.lock().add(pkg);
    // In full implementation: save to disk
    Ok(())
}

/// Remove a package from the database
pub fn unregister_package(name: &str) -> KResult<InstalledPackage> {
    DATABASE.lock()
        .remove(name)
        .ok_or(KError::NotFound)
}

/// Get package information
pub fn get_package(name: &str) -> Option<InstalledPackage> {
    DATABASE.lock().get(name).cloned()
}

/// Check if a package is installed
pub fn is_installed(name: &str) -> bool {
    DATABASE.lock().is_installed(name)
}

/// Get installed version of a package
pub fn installed_version(name: &str) -> Option<Version> {
    DATABASE.lock().get(name).map(|p| p.metadata.version.clone())
}

/// List all installed packages
pub fn list_installed() -> Vec<InstalledPackage> {
    DATABASE.lock().list()
}

/// Check which package owns a file
pub fn file_owner(path: &str) -> Option<String> {
    DATABASE.lock().file_owner(path).map(String::from)
}

/// Get packages that depend on a package
pub fn reverse_dependencies(name: &str) -> Vec<String> {
    DATABASE.lock().reverse_deps(name)
}

/// Get orphaned packages
pub fn orphaned_packages() -> Vec<String> {
    DATABASE.lock().orphans()
}

/// Get explicitly installed packages
pub fn explicit_packages() -> Vec<String> {
    DATABASE.lock().explicit_packages()
}

/// Check for file conflicts with a new package
pub fn check_conflicts(files: &[String]) -> Vec<(String, String)> {
    let db = DATABASE.lock();
    files
        .iter()
        .filter_map(|f| {
            db.file_owner(f).map(|owner| (f.clone(), String::from(owner)))
        })
        .collect()
}

/// Get total number of installed packages
pub fn package_count() -> usize {
    DATABASE.lock().packages.len()
}

/// Get total installed size
pub fn total_installed_size() -> u64 {
    DATABASE.lock()
        .packages
        .values()
        .map(|p| p.metadata.installed_size)
        .sum()
}
