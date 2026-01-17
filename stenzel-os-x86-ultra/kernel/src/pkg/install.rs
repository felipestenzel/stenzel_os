//! Package Installation and Removal
//!
//! Handles installing, removing, and upgrading packages.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use alloc::collections::BTreeSet;
use crate::util::{KResult, KError};
use crate::fs::vfs::{Inode, Mode, InodeKind};
use crate::security::Cred;
use super::{
    Package, PackageMetadata, TarArchive,
    database::{self, InstallReason, InstalledPackage},
    repository,
    sign,
};

/// Installation options
#[derive(Debug, Clone)]
pub struct InstallOptions {
    /// Force installation even if package is already installed
    pub force: bool,
    /// Skip dependency resolution
    pub no_deps: bool,
    /// Skip signature verification
    pub no_verify: bool,
    /// Install as dependency (not explicit)
    pub as_dependency: bool,
    /// Dry run (don't actually install)
    pub dry_run: bool,
}

impl Default for InstallOptions {
    fn default() -> Self {
        Self {
            force: false,
            no_deps: false,
            no_verify: false,
            as_dependency: false,
            dry_run: false,
        }
    }
}

/// Removal options
#[derive(Debug, Clone)]
pub struct RemoveOptions {
    /// Also remove packages that depend on this one
    pub cascade: bool,
    /// Also remove orphaned dependencies
    pub remove_deps: bool,
    /// Don't remove dependencies even if orphaned
    pub no_save: bool,
    /// Dry run (don't actually remove)
    pub dry_run: bool,
}

impl Default for RemoveOptions {
    fn default() -> Self {
        Self {
            cascade: false,
            remove_deps: true,
            no_save: false,
            dry_run: false,
        }
    }
}

/// Package transaction type
#[derive(Debug, Clone)]
pub enum TransactionType {
    Install,
    Remove,
    Upgrade,
    Reinstall,
}

/// A single transaction item
#[derive(Debug, Clone)]
pub struct TransactionItem {
    pub package_name: String,
    pub action: TransactionType,
    pub old_version: Option<super::Version>,
    pub new_version: Option<super::Version>,
}

/// Transaction state
pub struct Transaction {
    items: Vec<TransactionItem>,
    to_install: Vec<Package>,
    to_remove: Vec<String>,
}

impl Transaction {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            to_install: Vec::new(),
            to_remove: Vec::new(),
        }
    }

    /// Add a package to install
    pub fn add_install(&mut self, pkg: Package) {
        let name = pkg.metadata.name.clone();
        let version = pkg.metadata.version.clone();

        let old_version = database::installed_version(&name);
        let action = if old_version.is_some() {
            TransactionType::Upgrade
        } else {
            TransactionType::Install
        };

        self.items.push(TransactionItem {
            package_name: name,
            action,
            old_version,
            new_version: Some(version),
        });

        self.to_install.push(pkg);
    }

    /// Add a package to remove
    pub fn add_remove(&mut self, name: String) {
        if let Some(pkg) = database::get_package(&name) {
            self.items.push(TransactionItem {
                package_name: name.clone(),
                action: TransactionType::Remove,
                old_version: Some(pkg.metadata.version),
                new_version: None,
            });
            self.to_remove.push(name);
        }
    }

    /// Get transaction summary
    pub fn summary(&self) -> Vec<&TransactionItem> {
        self.items.iter().collect()
    }

    /// Execute the transaction
    pub fn execute(self) -> KResult<()> {
        // First, remove packages
        for name in &self.to_remove {
            remove_package_files(name)?;
        }

        // Then, install packages
        for pkg in self.to_install {
            install_package_files(&pkg)?;
        }

        Ok(())
    }
}

impl Default for Transaction {
    fn default() -> Self {
        Self::new()
    }
}

/// Install a package by name
pub fn install(name: &str) -> KResult<()> {
    install_with_options(name, &InstallOptions::default())
}

/// Install a package with options
pub fn install_with_options(name: &str, options: &InstallOptions) -> KResult<()> {
    crate::kprintln!("spkg: installing {}", name);

    // Check if already installed
    if !options.force && database::is_installed(name) {
        crate::kprintln!("spkg: {} is already installed", name);
        return Ok(());
    }

    // Resolve dependencies
    let mut to_install = if options.no_deps {
        vec![String::from(name)]
    } else {
        resolve_dependencies(name)?
    };

    // Filter out already installed packages
    to_install.retain(|n| options.force || !database::is_installed(n));

    if to_install.is_empty() {
        crate::kprintln!("spkg: nothing to do");
        return Ok(());
    }

    crate::kprintln!("spkg: packages to install: {:?}", to_install);

    if options.dry_run {
        return Ok(());
    }

    // Download and install each package
    for pkg_name in &to_install {
        let pkg_data = repository::download_package(pkg_name)?;
        let pkg = Package::from_bytes(&pkg_data)?;

        // Verify signature
        if !options.no_verify && pkg.is_signed() {
            if !sign::verify_trusted_signature(&pkg.data, pkg.signature.as_ref().unwrap())? {
                crate::kprintln!("spkg: signature verification failed for {}", pkg_name);
                return Err(KError::PermissionDenied);
            }
        }

        // Check for file conflicts
        let decompressed = pkg.decompress_data()?;
        let files = list_package_files(&decompressed)?;
        let conflicts = database::check_conflicts(&files);
        if !conflicts.is_empty() {
            crate::kprintln!("spkg: file conflicts detected:");
            for (file, owner) in &conflicts {
                crate::kprintln!("  {} is owned by {}", file, owner);
            }
            return Err(KError::AlreadyExists);
        }

        // Install files
        install_package_files(&pkg)?;

        // Register in database
        let reason = if pkg_name == name && !options.as_dependency {
            InstallReason::Explicit
        } else {
            InstallReason::Dependency
        };

        let dirs = list_package_dirs(&decompressed)?;
        database::register_package(pkg.metadata.clone(), files, dirs, reason)?;

        crate::kprintln!("spkg: installed {} {}", pkg_name, pkg.metadata.version);
    }

    Ok(())
}

/// Install package from local file
pub fn install_from_file(path: &str, options: &InstallOptions) -> KResult<()> {
    crate::kprintln!("spkg: installing from {}", path);

    // Use root credentials for package installation
    let cred = root_cred();

    // Read package file
    let data = crate::fs::read_file(path, &cred)?;
    let pkg = Package::from_bytes(&data)?;

    // Verify signature if required
    if !options.no_verify && pkg.is_signed() {
        if !sign::verify_trusted_signature(&pkg.data, pkg.signature.as_ref().unwrap())? {
            crate::kprintln!("spkg: signature verification failed");
            return Err(KError::PermissionDenied);
        }
    }

    // Install the package
    let name = pkg.metadata.name.clone();

    // Resolve and install dependencies first
    if !options.no_deps {
        for dep in &pkg.metadata.dependencies {
            if !database::is_installed(&dep.name) {
                let mut dep_options = options.clone();
                dep_options.as_dependency = true;
                install_with_options(&dep.name, &dep_options)?;
            }
        }
    }

    // Check for file conflicts
    let decompressed = pkg.decompress_data()?;
    let files = list_package_files(&decompressed)?;

    if !options.force {
        let conflicts = database::check_conflicts(&files);
        if !conflicts.is_empty() {
            for (file, owner) in &conflicts {
                crate::kprintln!("  {} is owned by {}", file, owner);
            }
            return Err(KError::AlreadyExists);
        }
    }

    // Install files
    install_package_files(&pkg)?;

    // Register in database
    let reason = if options.as_dependency {
        InstallReason::Dependency
    } else {
        InstallReason::Explicit
    };

    let dirs = list_package_dirs(&decompressed)?;
    database::register_package(pkg.metadata, files, dirs, reason)?;

    crate::kprintln!("spkg: installed {}", name);
    Ok(())
}

/// Remove a package by name
pub fn remove(name: &str) -> KResult<()> {
    remove_with_options(name, &RemoveOptions::default())
}

/// Remove a package with options
pub fn remove_with_options(name: &str, options: &RemoveOptions) -> KResult<()> {
    crate::kprintln!("spkg: removing {}", name);

    // Check if installed
    if !database::is_installed(name) {
        crate::kprintln!("spkg: {} is not installed", name);
        return Err(KError::NotFound);
    }

    // Check reverse dependencies
    let rdeps = database::reverse_dependencies(name);
    if !rdeps.is_empty() {
        if options.cascade {
            crate::kprintln!("spkg: also removing: {:?}", rdeps);
            for rdep in &rdeps {
                remove_with_options(rdep, options)?;
            }
        } else {
            crate::kprintln!("spkg: {} is required by: {:?}", name, rdeps);
            return Err(KError::Busy);
        }
    }

    if options.dry_run {
        return Ok(());
    }

    // Remove package files
    remove_package_files(name)?;

    // Unregister from database
    database::unregister_package(name)?;

    crate::kprintln!("spkg: removed {}", name);

    // Clean up orphaned dependencies
    if options.remove_deps && !options.no_save {
        let orphans = database::orphaned_packages();
        for orphan in orphans {
            crate::kprintln!("spkg: removing orphan {}", orphan);
            remove_package_files(&orphan)?;
            database::unregister_package(&orphan)?;
        }
    }

    Ok(())
}

/// Upgrade all installed packages
pub fn upgrade_all() -> KResult<()> {
    crate::kprintln!("spkg: checking for upgrades");

    // Sync repositories first
    repository::sync_all()?;

    let installed = database::list_installed();
    let mut upgrades = Vec::new();

    for pkg in installed {
        if let Some(remote) = repository::get_package_info(&pkg.metadata.name) {
            if remote.metadata.version > pkg.metadata.version {
                crate::kprintln!("  {} {} -> {}", pkg.metadata.name, pkg.metadata.version, remote.metadata.version);
                upgrades.push(pkg.metadata.name.clone());
            }
        }
    }

    if upgrades.is_empty() {
        crate::kprintln!("spkg: all packages up to date");
        return Ok(());
    }

    crate::kprintln!("spkg: upgrading {} packages", upgrades.len());

    for name in upgrades {
        let options = InstallOptions {
            force: true,
            ..Default::default()
        };
        install_with_options(&name, &options)?;
    }

    Ok(())
}

/// Resolve package dependencies recursively
fn resolve_dependencies(name: &str) -> KResult<Vec<String>> {
    let mut resolved = Vec::new();
    let mut seen = BTreeSet::new();

    resolve_deps_recursive(name, &mut resolved, &mut seen)?;

    Ok(resolved)
}

fn resolve_deps_recursive(
    name: &str,
    resolved: &mut Vec<String>,
    seen: &mut BTreeSet<String>,
) -> KResult<()> {
    if seen.contains(name) {
        return Ok(());
    }
    seen.insert(String::from(name));

    // Get package info from repository
    let info = repository::get_package_info(name).ok_or(KError::NotFound)?;

    // Resolve dependencies first (depth-first)
    for dep in &info.metadata.dependencies {
        // Check if installed version satisfies constraint
        if let Some(installed_ver) = database::installed_version(&dep.name) {
            if dep.version_constraint.satisfies(&installed_ver) {
                continue;
            }
        }

        resolve_deps_recursive(&dep.name, resolved, seen)?;
    }

    // Add this package after dependencies
    resolved.push(String::from(name));

    Ok(())
}

/// Get root credentials for package operations
fn root_cred() -> Cred {
    Cred::root()
}

/// Install package files to filesystem
fn install_package_files(pkg: &Package) -> KResult<()> {
    let data = pkg.decompress_data()?;
    let mut archive = TarArchive::new(&data);
    let cred = root_cred();

    while let Some(entry) = archive.next_entry() {
        let path = entry.header.name();
        if path.is_empty() {
            continue;
        }

        // Prepend root path
        let full_path = if path.starts_with('/') {
            path
        } else {
            alloc::format!("/{}", path)
        };

        if entry.header.is_directory() {
            // Create directory
            let mode = Mode::from_octal(entry.header.mode() as u16);
            crate::fs::mkdir(&full_path, &cred, mode)?;
        } else if entry.header.is_file() {
            // Write file
            let mode = Mode::from_octal(entry.header.mode() as u16);
            crate::fs::write_file(&full_path, &cred, mode, entry.data)?;
        } else if entry.header.is_symlink() {
            // Create symlink
            let target = entry.header.link_target();
            crate::fs::symlink(&target, &full_path, &cred)?;
        }
    }

    Ok(())
}

/// Remove package files from filesystem
fn remove_package_files(name: &str) -> KResult<()> {
    let pkg = database::get_package(name).ok_or(KError::NotFound)?;
    let cred = root_cred();

    // Remove files first
    for file in &pkg.files {
        if let Err(e) = crate::fs::unlink(file, &cred) {
            crate::kprintln!("spkg: warning: failed to remove {}: {:?}", file, e);
        }
    }

    // Remove directories (in reverse order, deepest first)
    let mut dirs = pkg.dirs.clone();
    dirs.sort_by(|a, b| b.len().cmp(&a.len()));

    for dir in &dirs {
        // Only remove if empty
        if let Err(_) = crate::fs::rmdir(dir, &cred) {
            // Directory not empty or other error, skip
        }
    }

    Ok(())
}

/// List files in a package archive
fn list_package_files(data: &[u8]) -> KResult<Vec<String>> {
    let mut files = Vec::new();
    let mut archive = TarArchive::new(data);

    while let Some(entry) = archive.next_entry() {
        let path = entry.header.name();
        if !path.is_empty() && entry.header.is_file() {
            let full_path = if path.starts_with('/') {
                path
            } else {
                alloc::format!("/{}", path)
            };
            files.push(full_path);
        }
    }

    Ok(files)
}

/// List directories in a package archive
fn list_package_dirs(data: &[u8]) -> KResult<Vec<String>> {
    let mut dirs = Vec::new();
    let mut archive = TarArchive::new(data);

    while let Some(entry) = archive.next_entry() {
        let path = entry.header.name();
        if !path.is_empty() && entry.header.is_directory() {
            let full_path = if path.starts_with('/') {
                path
            } else {
                alloc::format!("/{}", path)
            };
            dirs.push(full_path);
        }
    }

    Ok(dirs)
}

/// Check if a package can be removed safely
pub fn can_remove(name: &str) -> KResult<bool> {
    if !database::is_installed(name) {
        return Ok(false);
    }

    let rdeps = database::reverse_dependencies(name);
    Ok(rdeps.is_empty())
}

/// Get packages that would be removed if removing a package
pub fn removal_impact(name: &str) -> Vec<String> {
    let mut impact = Vec::new();
    collect_removal_impact(name, &mut impact);
    impact
}

fn collect_removal_impact(name: &str, impact: &mut Vec<String>) {
    let rdeps = database::reverse_dependencies(name);
    for rdep in rdeps {
        if !impact.contains(&rdep) {
            impact.push(rdep.clone());
            collect_removal_impact(&rdep, impact);
        }
    }
}

/// Clean package cache
pub fn clean_cache() -> KResult<()> {
    crate::kprintln!("spkg: cleaning cache");

    // In a full implementation, this would:
    // 1. Remove downloaded packages from /var/cache/spkg/pkg/
    // 2. Keep only installed versions or most recent N versions

    Ok(())
}

/// Verify installed package integrity
pub fn verify_package(name: &str) -> KResult<Vec<String>> {
    let pkg = database::get_package(name).ok_or(KError::NotFound)?;
    let cred = root_cred();
    let mut issues = Vec::new();

    for file in &pkg.files {
        match crate::fs::stat(file, &cred) {
            Ok(_) => {
                // File exists, could add checksum verification here
            }
            Err(_) => {
                issues.push(alloc::format!("missing: {}", file));
            }
        }
    }

    Ok(issues)
}
