//! Source Package Management
//!
//! Provides functionality for managing source packages:
//! - Source package repository
//! - Version control integration (Git)
//! - Patch management
//! - Local source builds
//! - Source package distribution

#![allow(dead_code)]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::format;
use alloc::vec;
use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use crate::sync::IrqSafeMutex;
use crate::util::{KResult, KError};

use super::metadata::Version;
use super::build::BuildRecipe;

// ============================================================================
// Source Package Format
// ============================================================================

/// Unique source package identifier
pub type SourcePackageId = u64;

/// Source package definition
#[derive(Debug, Clone)]
pub struct SourcePackage {
    /// Unique ID
    pub id: SourcePackageId,
    /// Package name
    pub name: String,
    /// Package version
    pub version: Version,
    /// Source type
    pub source_type: SourceType,
    /// Build recipe
    pub recipe: BuildRecipe,
    /// Patches to apply
    pub patches: Vec<Patch>,
    /// Local source directory (if extracted)
    pub source_dir: Option<String>,
    /// Timestamp
    pub timestamp: u64,
    /// Maintainer
    pub maintainer: String,
    /// Category/group
    pub category: String,
}

/// Type of source
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceType {
    /// Tarball (tar.gz, tar.xz, tar.bz2)
    Tarball {
        url: String,
        checksum: Option<String>,
    },
    /// Git repository
    Git {
        url: String,
        branch: Option<String>,
        tag: Option<String>,
        commit: Option<String>,
    },
    /// SVN repository
    Svn {
        url: String,
        revision: Option<u64>,
    },
    /// Mercurial repository
    Hg {
        url: String,
        revision: Option<String>,
    },
    /// Local path
    Local {
        path: String,
    },
}

impl SourceType {
    /// Get the URL for display
    pub fn url(&self) -> &str {
        match self {
            SourceType::Tarball { url, .. } => url,
            SourceType::Git { url, .. } => url,
            SourceType::Svn { url, .. } => url,
            SourceType::Hg { url, .. } => url,
            SourceType::Local { path } => path,
        }
    }

    /// Check if this is a VCS source
    pub fn is_vcs(&self) -> bool {
        matches!(self, SourceType::Git { .. } | SourceType::Svn { .. } | SourceType::Hg { .. })
    }
}

// ============================================================================
// Patch Management
// ============================================================================

/// A patch to apply to source
#[derive(Debug, Clone)]
pub struct Patch {
    /// Patch name/ID
    pub name: String,
    /// Patch content (unified diff format)
    pub content: String,
    /// Strip level for patch -p
    pub strip_level: u32,
    /// Apply conditionally
    pub condition: Option<PatchCondition>,
    /// Description
    pub description: String,
    /// Is this a security fix?
    pub is_security: bool,
}

/// Condition for applying a patch
#[derive(Debug, Clone)]
pub enum PatchCondition {
    /// Apply only for specific architecture
    Arch(String),
    /// Apply only if certain feature is enabled
    Feature(String),
    /// Apply only for version range
    Version { min: Option<Version>, max: Option<Version> },
    /// Custom condition expression
    Custom(String),
}

impl Patch {
    /// Create a new patch
    pub fn new(name: &str, content: &str) -> Self {
        Patch {
            name: name.to_string(),
            content: content.to_string(),
            strip_level: 1,
            condition: None,
            description: String::new(),
            is_security: false,
        }
    }

    /// Check if patch should be applied given conditions
    pub fn should_apply(&self, arch: &str, features: &[String], version: &Version) -> bool {
        match &self.condition {
            None => true,
            Some(PatchCondition::Arch(a)) => a == arch,
            Some(PatchCondition::Feature(f)) => features.contains(f),
            Some(PatchCondition::Version { min, max }) => {
                let min_ok = min.as_ref().map(|m| version >= m).unwrap_or(true);
                let max_ok = max.as_ref().map(|m| version <= m).unwrap_or(true);
                min_ok && max_ok
            }
            Some(PatchCondition::Custom(_)) => true, // Would need expression evaluator
        }
    }
}

// ============================================================================
// Source Repository
// ============================================================================

/// Source package repository configuration
#[derive(Debug, Clone)]
pub struct SourceRepository {
    /// Repository name
    pub name: String,
    /// Repository URL
    pub url: String,
    /// Is enabled
    pub enabled: bool,
    /// Priority (lower = higher priority)
    pub priority: u32,
    /// Type of repository
    pub repo_type: SourceRepoType,
    /// GPG key ID for verification
    pub gpg_key: Option<String>,
}

/// Type of source repository
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceRepoType {
    /// Simple HTTP/HTTPS index
    Http,
    /// Git repository with recipes
    Git,
    /// Local filesystem path
    Local,
}

// ============================================================================
// Build Environment
// ============================================================================

/// Build environment configuration
#[derive(Debug, Clone)]
pub struct BuildEnvironment {
    /// Build root directory
    pub build_root: String,
    /// Source directory
    pub source_dir: String,
    /// Package output directory
    pub pkg_dir: String,
    /// Log directory
    pub log_dir: String,
    /// Architecture
    pub arch: String,
    /// Host triple
    pub host: String,
    /// Target triple
    pub target: String,
    /// Number of parallel jobs
    pub jobs: u32,
    /// Extra CFLAGS
    pub cflags: String,
    /// Extra CXXFLAGS
    pub cxxflags: String,
    /// Extra LDFLAGS
    pub ldflags: String,
    /// Environment variables
    pub env: BTreeMap<String, String>,
    /// Use chroot/container for building
    pub use_sandbox: bool,
    /// Keep build directory on failure
    pub keep_build_on_fail: bool,
    /// Enable debug info
    pub debug_info: bool,
}

impl Default for BuildEnvironment {
    fn default() -> Self {
        BuildEnvironment {
            build_root: "/var/tmp/spkg-build".to_string(),
            source_dir: "/var/cache/spkg/sources".to_string(),
            pkg_dir: "/var/cache/spkg/packages".to_string(),
            log_dir: "/var/log/spkg".to_string(),
            arch: "x86_64".to_string(),
            host: "x86_64-stenzel-linux".to_string(),
            target: "x86_64-stenzel-linux".to_string(),
            jobs: 4,
            cflags: "-O2 -pipe -march=x86-64".to_string(),
            cxxflags: "-O2 -pipe -march=x86-64".to_string(),
            ldflags: String::new(),
            env: BTreeMap::new(),
            use_sandbox: true,
            keep_build_on_fail: false,
            debug_info: false,
        }
    }
}

// ============================================================================
// Build Status & Results
// ============================================================================

/// Build status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildStatus {
    /// Not started
    Pending,
    /// Fetching sources
    Fetching,
    /// Extracting sources
    Extracting,
    /// Applying patches
    Patching,
    /// Configuring
    Configuring,
    /// Building
    Building,
    /// Running tests
    Testing,
    /// Packaging
    Packaging,
    /// Completed successfully
    Success,
    /// Failed
    Failed,
    /// Cancelled
    Cancelled,
}

impl BuildStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            BuildStatus::Pending => "pending",
            BuildStatus::Fetching => "fetching",
            BuildStatus::Extracting => "extracting",
            BuildStatus::Patching => "patching",
            BuildStatus::Configuring => "configuring",
            BuildStatus::Building => "building",
            BuildStatus::Testing => "testing",
            BuildStatus::Packaging => "packaging",
            BuildStatus::Success => "success",
            BuildStatus::Failed => "failed",
            BuildStatus::Cancelled => "cancelled",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, BuildStatus::Success | BuildStatus::Failed | BuildStatus::Cancelled)
    }
}

/// Build result
#[derive(Debug, Clone)]
pub struct BuildResult {
    /// Package name
    pub package: String,
    /// Version
    pub version: Version,
    /// Final status
    pub status: BuildStatus,
    /// Start time
    pub start_time: u64,
    /// End time
    pub end_time: Option<u64>,
    /// Output package paths
    pub packages: Vec<String>,
    /// Build log path
    pub log_path: Option<String>,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Warnings
    pub warnings: Vec<String>,
}

// ============================================================================
// Source Package Manager
// ============================================================================

/// Manager state
struct SourceManagerState {
    /// Source packages by name
    packages: BTreeMap<String, SourcePackage>,
    /// Source repositories
    repositories: Vec<SourceRepository>,
    /// Current builds
    active_builds: BTreeMap<String, BuildStatus>,
    /// Build history
    build_history: Vec<BuildResult>,
    /// Next package ID
    next_id: SourcePackageId,
    /// Default build environment
    default_env: BuildEnvironment,
}

impl SourceManagerState {
    const fn new() -> Self {
        SourceManagerState {
            packages: BTreeMap::new(),
            repositories: Vec::new(),
            active_builds: BTreeMap::new(),
            build_history: Vec::new(),
            next_id: 1,
            default_env: BuildEnvironment {
                build_root: String::new(),
                source_dir: String::new(),
                pkg_dir: String::new(),
                log_dir: String::new(),
                arch: String::new(),
                host: String::new(),
                target: String::new(),
                jobs: 4,
                cflags: String::new(),
                cxxflags: String::new(),
                ldflags: String::new(),
                env: BTreeMap::new(),
                use_sandbox: true,
                keep_build_on_fail: false,
                debug_info: false,
            },
        }
    }
}

/// Global source manager
static SOURCE_MANAGER: IrqSafeMutex<SourceManagerState> = IrqSafeMutex::new(SourceManagerState::new());

// ============================================================================
// Public API
// ============================================================================

/// Initialize the source package system
pub fn init() -> KResult<()> {
    let mut state = SOURCE_MANAGER.lock();

    // Set default paths
    state.default_env = BuildEnvironment::default();

    // In a full implementation:
    // 1. Create required directories
    // 2. Load repository configuration
    // 3. Load cached source package metadata

    crate::kprintln!("spkg: source package system initialized");
    Ok(())
}

/// Add a source repository
pub fn add_repository(repo: SourceRepository) -> KResult<()> {
    let mut state = SOURCE_MANAGER.lock();

    // Check for duplicate
    if state.repositories.iter().any(|r| r.name == repo.name) {
        return Err(KError::AlreadyExists);
    }

    state.repositories.push(repo);
    Ok(())
}

/// Remove a source repository
pub fn remove_repository(name: &str) -> KResult<()> {
    let mut state = SOURCE_MANAGER.lock();

    let idx = state.repositories.iter()
        .position(|r| r.name == name)
        .ok_or(KError::NotFound)?;

    state.repositories.remove(idx);
    Ok(())
}

/// List source repositories
pub fn list_repositories() -> Vec<SourceRepository> {
    let state = SOURCE_MANAGER.lock();
    state.repositories.clone()
}

/// Sync repository metadata
pub fn sync_repositories() -> KResult<()> {
    let state = SOURCE_MANAGER.lock();

    for repo in &state.repositories {
        if !repo.enabled {
            continue;
        }

        // In a full implementation:
        // 1. Download/update repository index
        // 2. Verify GPG signatures
        // 3. Update local package cache

        crate::kprintln!("spkg: syncing repository '{}'", repo.name);
    }

    Ok(())
}

/// Register a source package
pub fn register_package(
    name: &str,
    version: Version,
    source_type: SourceType,
    recipe: BuildRecipe,
) -> KResult<SourcePackageId> {
    let mut state = SOURCE_MANAGER.lock();

    let id = state.next_id;
    state.next_id += 1;

    let pkg = SourcePackage {
        id,
        name: name.to_string(),
        version,
        source_type,
        recipe,
        patches: Vec::new(),
        source_dir: None,
        timestamp: crate::time::realtime().tv_sec as u64,
        maintainer: String::new(),
        category: String::new(),
    };

    state.packages.insert(name.to_string(), pkg);
    Ok(id)
}

/// Get a source package by name
pub fn get_package(name: &str) -> Option<SourcePackage> {
    let state = SOURCE_MANAGER.lock();
    state.packages.get(name).cloned()
}

/// Search for source packages
pub fn search_packages(query: &str) -> Vec<SourcePackage> {
    let state = SOURCE_MANAGER.lock();
    let query_lower = query.to_lowercase();

    state.packages.values()
        .filter(|p| {
            p.name.to_lowercase().contains(&query_lower) ||
            p.recipe.description.to_lowercase().contains(&query_lower)
        })
        .cloned()
        .collect()
}

/// Add a patch to a source package
pub fn add_patch(package_name: &str, patch: Patch) -> KResult<()> {
    let mut state = SOURCE_MANAGER.lock();

    let pkg = state.packages.get_mut(package_name)
        .ok_or(KError::NotFound)?;

    pkg.patches.push(patch);
    Ok(())
}

/// Remove a patch from a source package
pub fn remove_patch(package_name: &str, patch_name: &str) -> KResult<()> {
    let mut state = SOURCE_MANAGER.lock();

    let pkg = state.packages.get_mut(package_name)
        .ok_or(KError::NotFound)?;

    let idx = pkg.patches.iter()
        .position(|p| p.name == patch_name)
        .ok_or(KError::NotFound)?;

    pkg.patches.remove(idx);
    Ok(())
}

/// Fetch source for a package
pub fn fetch_source(package_name: &str) -> KResult<String> {
    let state = SOURCE_MANAGER.lock();

    let pkg = state.packages.get(package_name)
        .ok_or(KError::NotFound)?;

    let source_dir = format!("{}/{}-{}", state.default_env.source_dir, pkg.name, pkg.version);

    // In a full implementation:
    match &pkg.source_type {
        SourceType::Tarball { url, checksum } => {
            // 1. Download tarball from URL
            // 2. Verify checksum if provided
            // 3. Extract to source directory
            crate::kprintln!("spkg: fetching tarball from {}", url);
            if let Some(sum) = checksum {
                crate::kprintln!("spkg: verifying checksum {}", sum);
            }
        }
        SourceType::Git { url, branch, tag, commit } => {
            // 1. Clone or update git repository
            // 2. Checkout specified branch/tag/commit
            crate::kprintln!("spkg: cloning git repo from {}", url);
            if let Some(b) = branch {
                crate::kprintln!("spkg: checking out branch {}", b);
            }
            if let Some(t) = tag {
                crate::kprintln!("spkg: checking out tag {}", t);
            }
            if let Some(c) = commit {
                crate::kprintln!("spkg: checking out commit {}", c);
            }
        }
        SourceType::Svn { url, revision } => {
            crate::kprintln!("spkg: checking out svn from {}", url);
            if let Some(r) = revision {
                crate::kprintln!("spkg: at revision {}", r);
            }
        }
        SourceType::Hg { url, revision } => {
            crate::kprintln!("spkg: cloning hg repo from {}", url);
            if let Some(r) = revision {
                crate::kprintln!("spkg: at revision {}", r);
            }
        }
        SourceType::Local { path } => {
            crate::kprintln!("spkg: using local source from {}", path);
        }
    }

    Ok(source_dir)
}

/// Apply patches to source
pub fn apply_patches(package_name: &str, source_dir: &str) -> KResult<()> {
    let state = SOURCE_MANAGER.lock();

    let pkg = state.packages.get(package_name)
        .ok_or(KError::NotFound)?;

    let arch = &state.default_env.arch;
    let features: Vec<String> = Vec::new(); // Would come from config

    for patch in &pkg.patches {
        if patch.should_apply(arch, &features, &pkg.version) {
            crate::kprintln!("spkg: applying patch '{}' to {}", patch.name, source_dir);
            // In a full implementation: apply patch using patch utility
        }
    }

    Ok(())
}

/// Build a source package
pub fn build_package(package_name: &str, env: Option<BuildEnvironment>) -> KResult<BuildResult> {
    let state = SOURCE_MANAGER.lock();

    let pkg = state.packages.get(package_name)
        .ok_or(KError::NotFound)?
        .clone();

    let build_env = env.unwrap_or_else(|| state.default_env.clone());
    drop(state);

    // Update build status
    {
        let mut state = SOURCE_MANAGER.lock();
        state.active_builds.insert(package_name.to_string(), BuildStatus::Pending);
    }

    let start_time = crate::time::realtime().tv_sec as u64;

    // Build process
    let result = do_build(&pkg, &build_env);

    // Record result
    let build_result = match result {
        Ok(packages) => BuildResult {
            package: pkg.name.clone(),
            version: pkg.version.clone(),
            status: BuildStatus::Success,
            start_time,
            end_time: Some(crate::time::realtime().tv_sec as u64),
            packages,
            log_path: Some(format!("{}/{}-{}.log", build_env.log_dir, pkg.name, pkg.version)),
            error: None,
            warnings: Vec::new(),
        },
        Err(e) => BuildResult {
            package: pkg.name.clone(),
            version: pkg.version.clone(),
            status: BuildStatus::Failed,
            start_time,
            end_time: Some(crate::time::realtime().tv_sec as u64),
            packages: Vec::new(),
            log_path: Some(format!("{}/{}-{}.log", build_env.log_dir, pkg.name, pkg.version)),
            error: Some(format!("{:?}", e)),
            warnings: Vec::new(),
        },
    };

    // Update state
    {
        let mut state = SOURCE_MANAGER.lock();
        state.active_builds.remove(package_name);
        state.build_history.push(build_result.clone());
    }

    if build_result.status == BuildStatus::Success {
        Ok(build_result)
    } else {
        Err(KError::Invalid)
    }
}

/// Internal build implementation
fn do_build(pkg: &SourcePackage, env: &BuildEnvironment) -> KResult<Vec<String>> {
    let build_dir = format!("{}/{}-{}", env.build_root, pkg.name, pkg.version);
    let src_dir = format!("{}/src", build_dir);
    let dest_dir = format!("{}/pkg", build_dir);

    // Update status throughout the build
    update_build_status(&pkg.name, BuildStatus::Fetching);

    // 1. Fetch source
    let _ = fetch_source(&pkg.name)?;

    update_build_status(&pkg.name, BuildStatus::Extracting);

    // 2. Extract/prepare source
    // In a full implementation: extract tarball or copy from VCS

    update_build_status(&pkg.name, BuildStatus::Patching);

    // 3. Apply patches
    apply_patches(&pkg.name, &src_dir)?;

    update_build_status(&pkg.name, BuildStatus::Configuring);

    // 4. Run prepare() function
    for cmd in &pkg.recipe.prepare {
        run_build_command(cmd, &src_dir, env)?;
    }

    update_build_status(&pkg.name, BuildStatus::Building);

    // 5. Run build() function
    for cmd in &pkg.recipe.build {
        run_build_command(cmd, &src_dir, env)?;
    }

    update_build_status(&pkg.name, BuildStatus::Testing);

    // 6. Run check() function (optional tests)
    for cmd in &pkg.recipe.check {
        let _ = run_build_command(cmd, &src_dir, env);
    }

    update_build_status(&pkg.name, BuildStatus::Packaging);

    // 7. Run package() function
    for cmd in &pkg.recipe.package {
        run_build_command(cmd, &src_dir, env)?;
    }

    // 8. Create package archive
    let pkg_file = format!("{}/{}-{}.spkg", env.pkg_dir, pkg.name, pkg.version);

    // In a full implementation: create .spkg archive from dest_dir

    Ok(vec![pkg_file])
}

/// Update build status for a package
fn update_build_status(package: &str, status: BuildStatus) {
    let mut state = SOURCE_MANAGER.lock();
    state.active_builds.insert(package.to_string(), status);
}

/// Run a build command
fn run_build_command(cmd: &str, work_dir: &str, env: &BuildEnvironment) -> KResult<()> {
    // In a full implementation:
    // 1. Set up environment variables
    // 2. Execute command in work directory
    // 3. Capture output
    // 4. Check return code

    crate::kprintln!("spkg: running '{}' in {}", cmd, work_dir);

    let _ = env; // Would be used to set CFLAGS, etc.

    Ok(())
}

/// Get current build status
pub fn get_build_status(package_name: &str) -> Option<BuildStatus> {
    let state = SOURCE_MANAGER.lock();
    state.active_builds.get(package_name).copied()
}

/// Get build history
pub fn get_build_history(limit: Option<usize>) -> Vec<BuildResult> {
    let state = SOURCE_MANAGER.lock();
    let limit = limit.unwrap_or(state.build_history.len());
    state.build_history.iter().rev().take(limit).cloned().collect()
}

/// Clean build directory for a package
pub fn clean_build(package_name: &str) -> KResult<()> {
    let state = SOURCE_MANAGER.lock();
    let pkg = state.packages.get(package_name)
        .ok_or(KError::NotFound)?;

    let build_dir = format!("{}/{}-{}", state.default_env.build_root, pkg.name, pkg.version);

    // In a full implementation: remove build directory recursively
    crate::kprintln!("spkg: cleaning build directory {}", build_dir);

    Ok(())
}

/// Get default build environment
pub fn get_default_environment() -> BuildEnvironment {
    let state = SOURCE_MANAGER.lock();
    state.default_env.clone()
}

/// Set default build environment
pub fn set_default_environment(env: BuildEnvironment) -> KResult<()> {
    let mut state = SOURCE_MANAGER.lock();
    state.default_env = env;
    Ok(())
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Parse a source URL into SourceType
pub fn parse_source_url(url: &str) -> SourceType {
    if url.starts_with("git://") || url.starts_with("git+") || url.ends_with(".git") {
        let clean_url = url.trim_start_matches("git+");
        SourceType::Git {
            url: clean_url.to_string(),
            branch: None,
            tag: None,
            commit: None,
        }
    } else if url.starts_with("svn://") || url.starts_with("svn+") {
        let clean_url = url.trim_start_matches("svn+");
        SourceType::Svn {
            url: clean_url.to_string(),
            revision: None,
        }
    } else if url.starts_with("hg://") || url.starts_with("hg+") {
        let clean_url = url.trim_start_matches("hg+");
        SourceType::Hg {
            url: clean_url.to_string(),
            revision: None,
        }
    } else if url.starts_with("/") || url.starts_with("./") {
        SourceType::Local {
            path: url.to_string(),
        }
    } else {
        // Assume tarball
        SourceType::Tarball {
            url: url.to_string(),
            checksum: None,
        }
    }
}

/// Extract version from tarball filename
pub fn extract_version_from_filename(filename: &str) -> Option<Version> {
    // Common patterns: foo-1.2.3.tar.gz, foo_1.2.3.tar.gz

    let name = filename
        .trim_end_matches(".tar.gz")
        .trim_end_matches(".tar.xz")
        .trim_end_matches(".tar.bz2")
        .trim_end_matches(".tgz")
        .trim_end_matches(".zip");

    // Find version-like pattern (digits and dots)
    let parts: Vec<&str> = name.rsplitn(2, |c| c == '-' || c == '_').collect();

    if parts.len() >= 1 {
        let version_str = parts[0];
        let version_parts: Vec<&str> = version_str.split('.').collect();

        if version_parts.len() >= 2 {
            let major = version_parts[0].parse().ok()?;
            let minor = version_parts[1].parse().ok()?;
            let patch = version_parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);

            return Some(Version::new(major, minor, patch));
        }
    }

    None
}

/// Compute SHA-256 checksum of file
pub fn compute_checksum(path: &str) -> KResult<String> {
    // In a full implementation: read file and compute hash
    let _ = path;
    Ok(String::from("0".repeat(64)))
}

/// Verify checksum of file
pub fn verify_checksum(path: &str, expected: &str) -> KResult<bool> {
    let actual = compute_checksum(path)?;
    Ok(actual == expected)
}

// ============================================================================
// Statistics
// ============================================================================

/// Source package statistics
#[derive(Debug, Clone)]
pub struct SourceStats {
    /// Total source packages
    pub total_packages: usize,
    /// Active builds
    pub active_builds: usize,
    /// Successful builds
    pub successful_builds: usize,
    /// Failed builds
    pub failed_builds: usize,
    /// Total repositories
    pub repositories: usize,
    /// Enabled repositories
    pub enabled_repositories: usize,
}

/// Get source package statistics
pub fn get_stats() -> SourceStats {
    let state = SOURCE_MANAGER.lock();

    let successful = state.build_history.iter()
        .filter(|r| r.status == BuildStatus::Success)
        .count();

    let failed = state.build_history.iter()
        .filter(|r| r.status == BuildStatus::Failed)
        .count();

    let enabled = state.repositories.iter()
        .filter(|r| r.enabled)
        .count();

    SourceStats {
        total_packages: state.packages.len(),
        active_builds: state.active_builds.len(),
        successful_builds: successful,
        failed_builds: failed,
        repositories: state.repositories.len(),
        enabled_repositories: enabled,
    }
}
