//! Stenzel OS Package Manager (spkg)
//!
//! A simple package manager for Stenzel OS supporting:
//! - .spkg package format (tar archive with metadata + compressed files)
//! - Package metadata (name, version, dependencies, etc.)
//! - Package compression (zstd)
//! - Package signing (Ed25519)
//! - Local package database
//! - Dependency resolution
//! - Repository support

pub mod format;
pub mod metadata;
pub mod database;
pub mod install;
pub mod repository;
pub mod compress;
pub mod sign;
pub mod build;

pub use format::*;
pub use metadata::*;
pub use database::*;
pub use install::*;
pub use repository::*;

use alloc::string::String;
use alloc::vec::Vec;
use crate::util::{KResult, KError};

/// Package manager version
pub const VERSION: &str = "1.0.0";

/// Default package cache directory
pub const CACHE_DIR: &str = "/var/cache/spkg";

/// Default package database directory
pub const DB_DIR: &str = "/var/lib/spkg";

/// Default repository config directory
pub const REPO_DIR: &str = "/etc/spkg/repos.d";

/// Initialize the package manager subsystem
pub fn init() -> KResult<()> {
    crate::kprintln!("spkg: initializing package manager v{}", VERSION);

    // Initialize database
    database::init()?;

    // Load repositories
    repository::load_repos()?;

    crate::kprintln!("spkg: package manager ready");
    Ok(())
}

/// Search for packages matching a query
pub fn search(query: &str) -> Vec<PackageInfo> {
    repository::search_packages(query)
}

/// Install a package by name
pub fn install_package(name: &str) -> KResult<()> {
    install::install(name)
}

/// Remove a package by name
pub fn remove_package(name: &str) -> KResult<()> {
    install::remove(name)
}

/// Upgrade all installed packages
pub fn upgrade_all() -> KResult<()> {
    install::upgrade_all()
}

/// Get list of installed packages
pub fn list_installed() -> Vec<InstalledPackage> {
    database::list_installed()
}

/// Check if a package is installed
pub fn is_installed(name: &str) -> bool {
    database::is_installed(name)
}

/// Get info about an installed package
pub fn package_info(name: &str) -> Option<InstalledPackage> {
    database::get_package(name)
}
