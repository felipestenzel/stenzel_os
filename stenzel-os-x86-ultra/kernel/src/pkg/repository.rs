//! Package Repository Support
//!
//! Handles repository management, package searching, and downloads.

use alloc::string::String;
use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use crate::sync::IrqSafeMutex;
use crate::util::{KResult, KError};
use super::{PackageMetadata, PackageInfo, Version, Dependency, VersionConstraint};

/// Repository URL type
#[derive(Debug, Clone)]
pub enum RepoUrl {
    /// HTTP/HTTPS URL
    Http(String),
    /// Local file path
    File(String),
}

/// Repository mirror
#[derive(Debug, Clone)]
pub struct Mirror {
    /// Mirror URL
    pub url: RepoUrl,
    /// Mirror location/country
    pub location: Option<String>,
    /// Is this the primary mirror?
    pub primary: bool,
}

/// Repository configuration
#[derive(Debug, Clone)]
pub struct Repository {
    /// Repository name
    pub name: String,
    /// Repository description
    pub description: String,
    /// Mirror list
    pub mirrors: Vec<Mirror>,
    /// Is repository enabled?
    pub enabled: bool,
    /// Signature required?
    pub sig_required: bool,
    /// Repository priority (lower = higher priority)
    pub priority: u32,
}

impl Repository {
    pub fn new(name: &str) -> Self {
        Self {
            name: String::from(name),
            description: String::new(),
            mirrors: Vec::new(),
            enabled: true,
            sig_required: true,
            priority: 100,
        }
    }

    /// Add a mirror
    pub fn add_mirror(&mut self, url: RepoUrl) {
        self.mirrors.push(Mirror {
            url,
            location: None,
            primary: self.mirrors.is_empty(),
        });
    }

    /// Get primary mirror URL
    pub fn primary_url(&self) -> Option<&RepoUrl> {
        self.mirrors.iter()
            .find(|m| m.primary)
            .or(self.mirrors.first())
            .map(|m| &m.url)
    }
}

/// Package entry in repository database
#[derive(Debug, Clone)]
pub struct RepoPackage {
    /// Package metadata
    pub metadata: PackageMetadata,
    /// Download size (compressed)
    pub download_size: u64,
    /// Package filename
    pub filename: String,
    /// SHA256 checksum
    pub sha256: Option<String>,
    /// Repository this package is from
    pub repo_name: String,
}

impl RepoPackage {
    pub fn info(&self) -> PackageInfo {
        PackageInfo::from(&self.metadata)
    }
}

/// Repository manager state
struct RepoManager {
    /// Configured repositories
    repos: Vec<Repository>,
    /// Package index (name -> packages from all repos)
    packages: BTreeMap<String, Vec<RepoPackage>>,
    /// Last sync time per repo
    last_sync: BTreeMap<String, u64>,
}

impl RepoManager {
    const fn new() -> Self {
        Self {
            repos: Vec::new(),
            packages: BTreeMap::new(),
            last_sync: BTreeMap::new(),
        }
    }

    /// Add a repository
    fn add_repo(&mut self, repo: Repository) {
        self.repos.push(repo);
    }

    /// Remove a repository
    fn remove_repo(&mut self, name: &str) {
        self.repos.retain(|r| r.name != name);
        // Remove packages from this repo
        for pkgs in self.packages.values_mut() {
            pkgs.retain(|p| p.repo_name != name);
        }
    }

    /// Get a repository by name
    fn get_repo(&self, name: &str) -> Option<&Repository> {
        self.repos.iter().find(|r| r.name == name)
    }

    /// Get all enabled repositories
    fn enabled_repos(&self) -> Vec<&Repository> {
        self.repos.iter().filter(|r| r.enabled).collect()
    }

    /// Update package index from repository database
    fn update_index(&mut self, repo_name: &str, packages: Vec<RepoPackage>) {
        // Remove old packages from this repo
        for pkgs in self.packages.values_mut() {
            pkgs.retain(|p| p.repo_name != repo_name);
        }

        // Add new packages
        for pkg in packages {
            self.packages
                .entry(pkg.metadata.name.clone())
                .or_insert_with(Vec::new)
                .push(pkg);
        }

        // Record sync time
        let now = crate::time::realtime().tv_sec as u64;
        self.last_sync.insert(String::from(repo_name), now);
    }

    /// Search packages by name/description
    fn search(&self, query: &str) -> Vec<PackageInfo> {
        let query = query.to_lowercase();
        let mut results = Vec::new();

        for pkgs in self.packages.values() {
            for pkg in pkgs {
                let name_match = pkg.metadata.name.to_lowercase().contains(&query);
                let desc_match = pkg.metadata.description.to_lowercase().contains(&query);

                if name_match || desc_match {
                    results.push(pkg.info());
                    break; // Only add once per package name
                }
            }
        }

        results
    }

    /// Get best package version for a name
    fn get_package(&self, name: &str) -> Option<&RepoPackage> {
        self.packages.get(name).and_then(|pkgs| {
            // Sort by repo priority, then version
            let mut best: Option<&RepoPackage> = None;

            for pkg in pkgs {
                let repo = self.repos.iter().find(|r| r.name == pkg.repo_name);
                let priority = repo.map(|r| r.priority).unwrap_or(u32::MAX);

                if let Some(current_best) = best {
                    let current_repo = self.repos.iter().find(|r| r.name == current_best.repo_name);
                    let current_priority = current_repo.map(|r| r.priority).unwrap_or(u32::MAX);

                    if priority < current_priority {
                        best = Some(pkg);
                    } else if priority == current_priority && pkg.metadata.version > current_best.metadata.version {
                        best = Some(pkg);
                    }
                } else {
                    best = Some(pkg);
                }
            }

            best
        })
    }
}

/// Global repository manager
static REPO_MANAGER: IrqSafeMutex<RepoManager> = IrqSafeMutex::new(RepoManager::new());

/// Load repositories from config
pub fn load_repos() -> KResult<()> {
    crate::kprintln!("spkg: loading repositories");

    // Default repository (hardcoded for now)
    let mut core_repo = Repository::new("core");
    core_repo.description = String::from("Stenzel OS Core Repository");
    core_repo.add_mirror(RepoUrl::Http(String::from("https://pkg.stenzel-os.org/core")));
    core_repo.priority = 10;

    let mut extra_repo = Repository::new("extra");
    extra_repo.description = String::from("Stenzel OS Extra Repository");
    extra_repo.add_mirror(RepoUrl::Http(String::from("https://pkg.stenzel-os.org/extra")));
    extra_repo.priority = 20;

    let mut manager = REPO_MANAGER.lock();
    manager.add_repo(core_repo);
    manager.add_repo(extra_repo);

    crate::kprintln!("spkg: loaded {} repositories", manager.repos.len());
    Ok(())
}

/// Add a new repository
pub fn add_repository(name: &str, url: &str) -> KResult<()> {
    let mut repo = Repository::new(name);
    repo.add_mirror(RepoUrl::Http(String::from(url)));

    REPO_MANAGER.lock().add_repo(repo);
    crate::kprintln!("spkg: added repository {}", name);
    Ok(())
}

/// Remove a repository
pub fn remove_repository(name: &str) -> KResult<()> {
    REPO_MANAGER.lock().remove_repo(name);
    crate::kprintln!("spkg: removed repository {}", name);
    Ok(())
}

/// Enable/disable a repository
pub fn set_repo_enabled(name: &str, enabled: bool) -> KResult<()> {
    let mut manager = REPO_MANAGER.lock();
    for repo in &mut manager.repos {
        if repo.name == name {
            repo.enabled = enabled;
            return Ok(());
        }
    }
    Err(KError::NotFound)
}

/// Synchronize all repositories
pub fn sync_all() -> KResult<()> {
    crate::kprintln!("spkg: synchronizing repositories");

    let repos: Vec<String> = {
        let manager = REPO_MANAGER.lock();
        manager.enabled_repos().iter().map(|r| r.name.clone()).collect()
    };

    for repo_name in repos {
        sync_repo(&repo_name)?;
    }

    Ok(())
}

/// Synchronize a single repository
pub fn sync_repo(name: &str) -> KResult<()> {
    let repo = {
        let manager = REPO_MANAGER.lock();
        manager.get_repo(name).cloned()
    };

    let repo = repo.ok_or(KError::NotFound)?;
    crate::kprintln!("spkg: syncing {}", name);

    // Get repository database URL
    let db_url = match repo.primary_url() {
        Some(RepoUrl::Http(url)) => alloc::format!("{}/{}.db", url, name),
        Some(RepoUrl::File(path)) => alloc::format!("{}/{}.db", path, name),
        None => return Err(KError::NotFound),
    };

    // In a full implementation, this would:
    // 1. Download the database file
    // 2. Verify signature if required
    // 3. Parse package entries
    // 4. Update local index

    // For now, create some mock packages for testing
    let packages = create_mock_packages(&repo.name);

    REPO_MANAGER.lock().update_index(&repo.name, packages);
    crate::kprintln!("spkg: synced {}", name);

    let _ = db_url; // Would be used for actual download

    Ok(())
}

/// Create mock packages for testing
fn create_mock_packages(repo_name: &str) -> Vec<RepoPackage> {
    let mut packages = Vec::new();

    if repo_name == "core" {
        // Base system packages
        packages.push(create_package("base", "1.0.0", "Base system", &[], repo_name));
        packages.push(create_package("linux", "6.1.0", "Linux kernel", &["base"], repo_name));
        packages.push(create_package("glibc", "2.38", "GNU C Library", &["base"], repo_name));
        packages.push(create_package("bash", "5.2", "GNU Bourne Again Shell", &["glibc"], repo_name));
        packages.push(create_package("coreutils", "9.4", "GNU core utilities", &["glibc"], repo_name));
        packages.push(create_package("util-linux", "2.39", "System utilities", &["glibc"], repo_name));
    } else if repo_name == "extra" {
        // Additional packages
        packages.push(create_package("vim", "9.0", "Vi Improved text editor", &["glibc"], repo_name));
        packages.push(create_package("git", "2.42", "Version control system", &["glibc", "openssl"], repo_name));
        packages.push(create_package("openssl", "3.1", "Cryptography library", &["glibc"], repo_name));
        packages.push(create_package("curl", "8.4", "URL transfer tool", &["glibc", "openssl"], repo_name));
        packages.push(create_package("python", "3.12", "Python interpreter", &["glibc", "openssl"], repo_name));
    }

    packages
}

fn create_package(name: &str, version: &str, desc: &str, deps: &[&str], repo: &str) -> RepoPackage {
    let mut meta = PackageMetadata::new(name, Version::parse(version).unwrap());
    meta.description = String::from(desc);

    for &dep in deps {
        meta.dependencies.push(Dependency {
            name: String::from(dep),
            version_constraint: VersionConstraint::Any,
            optional: false,
        });
    }

    RepoPackage {
        metadata: meta,
        download_size: 1024 * 100, // Mock size
        filename: alloc::format!("{}-{}.spkg", name, version),
        sha256: None,
        repo_name: String::from(repo),
    }
}

/// Search for packages
pub fn search_packages(query: &str) -> Vec<PackageInfo> {
    REPO_MANAGER.lock().search(query)
}

/// Get package information
pub fn get_package_info(name: &str) -> Option<RepoPackage> {
    REPO_MANAGER.lock().get_package(name).cloned()
}

/// Download a package
pub fn download_package(name: &str) -> KResult<Vec<u8>> {
    let pkg_info = get_package_info(name).ok_or(KError::NotFound)?;

    let manager = REPO_MANAGER.lock();
    let repo = manager.get_repo(&pkg_info.repo_name).ok_or(KError::NotFound)?;

    let pkg_url = match repo.primary_url() {
        Some(RepoUrl::Http(url)) => alloc::format!("{}/{}", url, pkg_info.filename),
        Some(RepoUrl::File(path)) => alloc::format!("{}/{}", path, pkg_info.filename),
        None => return Err(KError::NotFound),
    };

    crate::kprintln!("spkg: downloading {}", pkg_url);

    // In a full implementation, this would:
    // 1. Download the package file
    // 2. Verify checksum
    // 3. Cache locally

    // For now, return empty package data (would need network stack)
    // Real implementation would use HTTP client

    // Create a minimal valid package for testing
    let pkg_data = create_mock_package_data(&pkg_info);

    Ok(pkg_data)
}

/// Create mock package data for testing
fn create_mock_package_data(info: &RepoPackage) -> Vec<u8> {
    use super::format::{PackageHeader, MAGIC, FORMAT_VERSION};

    let meta_str = info.metadata.serialize();
    let meta_bytes = meta_str.as_bytes();

    // Create empty tar archive (just end markers)
    let tar_data = [0u8; 1024]; // Two 512-byte zero blocks

    let mut header = PackageHeader::new();
    header.magic = MAGIC;
    header.version = FORMAT_VERSION;
    header.metadata_offset = PackageHeader::SIZE as u32;
    header.metadata_size = meta_bytes.len() as u32;
    header.signature_offset = 0;
    header.signature_size = 0;
    header.data_offset = (PackageHeader::SIZE + meta_bytes.len()) as u32;
    header.data_size = tar_data.len() as u32;
    header.data_uncompressed_size = tar_data.len() as u32;

    let mut data = Vec::new();
    data.extend_from_slice(&header.to_bytes());
    data.extend_from_slice(meta_bytes);
    data.extend_from_slice(&tar_data);

    data
}

/// Get repository list
pub fn list_repositories() -> Vec<Repository> {
    REPO_MANAGER.lock().repos.clone()
}

/// Get repository by name
pub fn get_repository(name: &str) -> Option<Repository> {
    REPO_MANAGER.lock().get_repo(name).cloned()
}

/// Get all available packages
pub fn list_available() -> Vec<PackageInfo> {
    let manager = REPO_MANAGER.lock();
    manager.packages.values()
        .filter_map(|pkgs| pkgs.first())
        .map(|p| p.info())
        .collect()
}

/// Check for available updates
pub fn check_updates() -> Vec<(String, Version, Version)> {
    let mut updates = Vec::new();
    let manager = REPO_MANAGER.lock();

    for (name, pkgs) in &manager.packages {
        if let Some(remote) = pkgs.first() {
            if let Some(installed_ver) = super::database::installed_version(name) {
                if remote.metadata.version > installed_ver {
                    updates.push((
                        name.clone(),
                        installed_ver,
                        remote.metadata.version.clone(),
                    ));
                }
            }
        }
    }

    updates
}

/// Get package providers (packages that provide a virtual package)
pub fn get_providers(virtual_pkg: &str) -> Vec<PackageInfo> {
    let manager = REPO_MANAGER.lock();
    let mut providers = Vec::new();

    for pkgs in manager.packages.values() {
        for pkg in pkgs {
            if pkg.metadata.provides.iter().any(|p| p == virtual_pkg) {
                providers.push(pkg.info());
                break;
            }
        }
    }

    providers
}
