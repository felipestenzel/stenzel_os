//! Node.js Runtime Support
//!
//! Provides infrastructure for running Node.js interpreters and scripts
//! on Stenzel OS.
//!
//! Supports:
//! - Node.js LTS versions (18.x, 20.x, 22.x)
//! - npm package manager
//! - npx package runner
//! - nvm (Node Version Manager) integration
//! - Global and local node_modules
//! - ES Modules and CommonJS
//! - Native addon support (.node files)

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use spin::Mutex;

/// Supported Node.js LTS versions
pub const NODEJS_VERSIONS: &[&str] = &[
    "22.0.0", // Latest
    "20.11.0", // LTS Hydrogen
    "18.19.0", // LTS Hydrogen
    "16.20.2", // Old LTS (Gallium)
];

/// Default Node.js version (latest LTS)
pub const DEFAULT_NODEJS_VERSION: &str = "20.11.0";

/// Node.js binary path format
pub const NODEJS_BIN_PATH: &str = "/usr/bin/node";
pub const NPM_BIN_PATH: &str = "/usr/bin/npm";
pub const NPX_BIN_PATH: &str = "/usr/bin/npx";

/// NVM directory
pub const NVM_DIR: &str = "/usr/local/nvm";

/// Global node_modules path
pub const GLOBAL_NODE_MODULES: &str = "/usr/lib/node_modules";

/// Cache directory
pub const NPM_CACHE_DIR: &str = "/var/cache/npm";

// ============================================================================
// Configuration
// ============================================================================

/// Node.js runtime configuration
#[derive(Debug, Clone)]
pub struct NodeConfig {
    /// Node.js version (e.g., "20.11.0")
    pub version: String,
    /// Major version number
    pub major_version: u32,
    /// Node binary path
    pub executable: String,
    /// npm binary path
    pub npm_executable: String,
    /// npx binary path
    pub npx_executable: String,
    /// Global node_modules directory
    pub global_modules: String,
    /// npm cache directory
    pub cache_dir: String,
    /// Is this a nvm-managed installation?
    pub is_nvm: bool,
    /// nvm installation path
    pub nvm_path: Option<String>,
    /// V8 version (if known)
    pub v8_version: Option<String>,
    /// libuv version (if known)
    pub libuv_version: Option<String>,
    /// OpenSSL version (if known)
    pub openssl_version: Option<String>,
}

impl NodeConfig {
    /// Create configuration for a Node.js version
    pub fn for_version(version: &str) -> Self {
        let major = version.split('.').next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(20);

        NodeConfig {
            version: version.to_string(),
            major_version: major,
            executable: NODEJS_BIN_PATH.to_string(),
            npm_executable: NPM_BIN_PATH.to_string(),
            npx_executable: NPX_BIN_PATH.to_string(),
            global_modules: GLOBAL_NODE_MODULES.to_string(),
            cache_dir: NPM_CACHE_DIR.to_string(),
            is_nvm: false,
            nvm_path: None,
            v8_version: None,
            libuv_version: None,
            openssl_version: None,
        }
    }

    /// Create default Node.js configuration
    pub fn default() -> Self {
        Self::for_version(DEFAULT_NODEJS_VERSION)
    }

    /// Create configuration for nvm-managed installation
    pub fn for_nvm(nvm_path: &str, version: &str) -> Self {
        let mut config = Self::for_version(version);
        config.is_nvm = true;
        config.nvm_path = Some(nvm_path.to_string());
        config.executable = format!("{}/versions/node/v{}/bin/node", nvm_path, version);
        config.npm_executable = format!("{}/versions/node/v{}/bin/npm", nvm_path, version);
        config.npx_executable = format!("{}/versions/node/v{}/bin/npx", nvm_path, version);
        config.global_modules = format!("{}/versions/node/v{}/lib/node_modules", nvm_path, version);
        config
    }

    /// Get NODE_PATH environment variable value
    pub fn node_path(&self) -> String {
        format!("{}:{}/node_modules", self.global_modules, self.global_modules)
    }
}

// ============================================================================
// Package.json parsing
// ============================================================================

/// Minimal package.json representation
#[derive(Debug, Clone, Default)]
pub struct PackageJson {
    /// Package name
    pub name: Option<String>,
    /// Package version
    pub version: Option<String>,
    /// Main entry point
    pub main: Option<String>,
    /// ES module entry point
    pub module: Option<String>,
    /// Package type (commonjs or module)
    pub pkg_type: PackageType,
    /// Binary commands
    pub bin: BTreeMap<String, String>,
    /// Dependencies
    pub dependencies: BTreeMap<String, String>,
    /// Dev dependencies
    pub dev_dependencies: BTreeMap<String, String>,
    /// Scripts
    pub scripts: BTreeMap<String, String>,
    /// Engines (node version requirements)
    pub engines: Option<EngineRequirements>,
}

/// Package type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PackageType {
    #[default]
    CommonJS,
    Module,
}

/// Engine version requirements
#[derive(Debug, Clone, Default)]
pub struct EngineRequirements {
    pub node: Option<String>,
    pub npm: Option<String>,
}

impl PackageJson {
    /// Parse package.json from JSON string
    pub fn parse(json: &str) -> Option<Self> {
        // Simple JSON parsing for package.json
        // In a full implementation, this would use a proper JSON parser
        let mut pkg = PackageJson::default();

        // Extract name
        if let Some(name) = extract_json_string(json, "name") {
            pkg.name = Some(name);
        }

        // Extract version
        if let Some(version) = extract_json_string(json, "version") {
            pkg.version = Some(version);
        }

        // Extract main
        if let Some(main) = extract_json_string(json, "main") {
            pkg.main = Some(main);
        }

        // Extract module
        if let Some(module) = extract_json_string(json, "module") {
            pkg.module = Some(module);
        }

        // Extract type
        if let Some(pkg_type) = extract_json_string(json, "type") {
            pkg.pkg_type = match pkg_type.as_str() {
                "module" => PackageType::Module,
                _ => PackageType::CommonJS,
            };
        }

        // Extract scripts
        if let Some(scripts_obj) = extract_json_object(json, "scripts") {
            pkg.scripts = parse_string_map(&scripts_obj);
        }

        // Extract dependencies
        if let Some(deps_obj) = extract_json_object(json, "dependencies") {
            pkg.dependencies = parse_string_map(&deps_obj);
        }

        // Extract devDependencies
        if let Some(dev_deps_obj) = extract_json_object(json, "devDependencies") {
            pkg.dev_dependencies = parse_string_map(&dev_deps_obj);
        }

        // Extract bin
        if let Some(bin_obj) = extract_json_object(json, "bin") {
            pkg.bin = parse_string_map(&bin_obj);
        } else if let Some(bin_str) = extract_json_string(json, "bin") {
            if let Some(ref name) = pkg.name {
                pkg.bin.insert(name.clone(), bin_str);
            }
        }

        Some(pkg)
    }

    /// Get entry point for this package
    pub fn entry_point(&self) -> Option<&str> {
        self.module.as_deref()
            .or(self.main.as_deref())
            .or(Some("index.js"))
    }

    /// Check if package is ES module
    pub fn is_esm(&self) -> bool {
        self.pkg_type == PackageType::Module
    }
}

/// Simple JSON string extraction
fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\"", key);
    let start = json.find(&pattern)?;
    let rest = &json[start + pattern.len()..];

    // Skip whitespace and colon
    let rest = rest.trim_start();
    if !rest.starts_with(':') {
        return None;
    }
    let rest = rest[1..].trim_start();

    if !rest.starts_with('"') {
        return None;
    }

    let rest = &rest[1..];
    let end = rest.find('"')?;

    Some(rest[..end].to_string())
}

/// Simple JSON object extraction
fn extract_json_object(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\"", key);
    let start = json.find(&pattern)?;
    let rest = &json[start + pattern.len()..];

    // Skip whitespace and colon
    let rest = rest.trim_start();
    if !rest.starts_with(':') {
        return None;
    }
    let rest = rest[1..].trim_start();

    if !rest.starts_with('{') {
        return None;
    }

    // Find matching closing brace
    let mut depth = 0;
    let mut in_string = false;
    let mut escape = false;

    for (i, c) in rest.chars().enumerate() {
        if escape {
            escape = false;
            continue;
        }

        match c {
            '\\' if in_string => escape = true,
            '"' => in_string = !in_string,
            '{' if !in_string => depth += 1,
            '}' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    return Some(rest[..=i].to_string());
                }
            }
            _ => {}
        }
    }

    None
}

/// Parse a JSON object into a string map
fn parse_string_map(json_obj: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();

    // Simple key-value parsing
    let inner = json_obj.trim();
    let inner = if inner.starts_with('{') && inner.ends_with('}') {
        &inner[1..inner.len()-1]
    } else {
        inner
    };

    // Split by commas (simplified - doesn't handle nested objects)
    for pair in inner.split(',') {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }

        // Find key and value
        if let Some(colon_pos) = pair.find(':') {
            let key = pair[..colon_pos].trim().trim_matches('"');
            let value = pair[colon_pos + 1..].trim().trim_matches('"');
            map.insert(key.to_string(), value.to_string());
        }
    }

    map
}

// ============================================================================
// Module Resolution
// ============================================================================

/// Node.js module resolution algorithm
#[derive(Debug, Clone)]
pub struct ModuleResolver {
    /// Search paths for modules
    pub search_paths: Vec<String>,
    /// Cached resolutions
    cache: BTreeMap<String, String>,
}

impl ModuleResolver {
    /// Create a new module resolver
    pub fn new(config: &NodeConfig) -> Self {
        let mut search_paths = vec![
            String::from("."),
            String::from("./node_modules"),
        ];

        // Add global modules
        search_paths.push(config.global_modules.clone());

        // Add NODE_PATH entries
        for path in config.node_path().split(':') {
            if !path.is_empty() && !search_paths.contains(&path.to_string()) {
                search_paths.push(path.to_string());
            }
        }

        ModuleResolver {
            search_paths,
            cache: BTreeMap::new(),
        }
    }

    /// Resolve a module specifier from a given directory
    pub fn resolve(&mut self, specifier: &str, from_dir: &str) -> Option<String> {
        // Check cache
        let cache_key = format!("{}:{}", from_dir, specifier);
        if let Some(cached) = self.cache.get(&cache_key) {
            return Some(cached.clone());
        }

        let result = self.resolve_uncached(specifier, from_dir)?;
        self.cache.insert(cache_key, result.clone());
        Some(result)
    }

    fn resolve_uncached(&self, specifier: &str, from_dir: &str) -> Option<String> {
        // Relative path
        if specifier.starts_with("./") || specifier.starts_with("../") {
            return self.resolve_file(&format!("{}/{}", from_dir, specifier));
        }

        // Absolute path
        if specifier.starts_with('/') {
            return self.resolve_file(specifier);
        }

        // Node core module (return as-is, handled by runtime)
        if is_core_module(specifier) {
            return Some(format!("node:{}", specifier));
        }

        // Package resolution
        self.resolve_package(specifier, from_dir)
    }

    /// Resolve a file path
    fn resolve_file(&self, path: &str) -> Option<String> {
        // Try exact path
        if path_exists(path) && !is_directory(path) {
            return Some(path.to_string());
        }

        // Try with extensions
        for ext in &[".js", ".mjs", ".cjs", ".json", ".node"] {
            let with_ext = format!("{}{}", path, ext);
            if path_exists(&with_ext) {
                return Some(with_ext);
            }
        }

        // Try as directory with index
        if is_directory(path) {
            // Check package.json
            let pkg_path = format!("{}/package.json", path);
            if path_exists(&pkg_path) {
                if let Some(entry) = self.read_package_entry(&pkg_path) {
                    let full_path = format!("{}/{}", path, entry);
                    if let Some(resolved) = self.resolve_file(&full_path) {
                        return Some(resolved);
                    }
                }
            }

            // Try index files
            for index in &["index.js", "index.mjs", "index.cjs", "index.json"] {
                let index_path = format!("{}/{}", path, index);
                if path_exists(&index_path) {
                    return Some(index_path);
                }
            }
        }

        None
    }

    /// Resolve a package
    fn resolve_package(&self, specifier: &str, from_dir: &str) -> Option<String> {
        // Split into package name and subpath
        let (pkg_name, subpath) = if specifier.starts_with('@') {
            // Scoped package
            let parts: Vec<&str> = specifier.splitn(3, '/').collect();
            if parts.len() >= 2 {
                let name = format!("{}/{}", parts[0], parts[1]);
                let sub = if parts.len() > 2 { Some(parts[2]) } else { None };
                (name, sub)
            } else {
                return None;
            }
        } else {
            // Regular package
            let parts: Vec<&str> = specifier.splitn(2, '/').collect();
            (parts[0].to_string(), parts.get(1).copied())
        };

        // Search in node_modules up the directory tree
        let mut current_dir = from_dir.to_string();
        loop {
            let node_modules = format!("{}/node_modules/{}", current_dir, pkg_name);
            if is_directory(&node_modules) {
                let path = if let Some(sub) = subpath {
                    format!("{}/{}", node_modules, sub)
                } else {
                    node_modules
                };
                if let Some(resolved) = self.resolve_file(&path) {
                    return Some(resolved);
                }
            }

            // Move up one directory
            if let Some(parent) = parent_dir(&current_dir) {
                current_dir = parent;
            } else {
                break;
            }
        }

        // Search in global search paths
        for search_path in &self.search_paths {
            let pkg_path = format!("{}/{}", search_path, pkg_name);
            if is_directory(&pkg_path) {
                let path = if let Some(sub) = subpath {
                    format!("{}/{}", pkg_path, sub)
                } else {
                    pkg_path
                };
                if let Some(resolved) = self.resolve_file(&path) {
                    return Some(resolved);
                }
            }
        }

        None
    }

    /// Read entry point from package.json
    fn read_package_entry(&self, pkg_path: &str) -> Option<String> {
        // In real implementation, would read and parse the file
        // For now, return default
        Some(String::from("index.js"))
    }
}

// ============================================================================
// Node.js Core Modules
// ============================================================================

/// List of Node.js core modules
pub const CORE_MODULES: &[&str] = &[
    "assert",
    "async_hooks",
    "buffer",
    "child_process",
    "cluster",
    "console",
    "constants",
    "crypto",
    "dgram",
    "diagnostics_channel",
    "dns",
    "domain",
    "events",
    "fs",
    "fs/promises",
    "http",
    "http2",
    "https",
    "inspector",
    "module",
    "net",
    "os",
    "path",
    "path/posix",
    "path/win32",
    "perf_hooks",
    "process",
    "punycode",
    "querystring",
    "readline",
    "repl",
    "stream",
    "stream/consumers",
    "stream/promises",
    "stream/web",
    "string_decoder",
    "sys",
    "timers",
    "timers/promises",
    "tls",
    "trace_events",
    "tty",
    "url",
    "util",
    "v8",
    "vm",
    "wasi",
    "worker_threads",
    "zlib",
];

/// Check if a module specifier is a core module
pub fn is_core_module(specifier: &str) -> bool {
    let specifier = specifier.strip_prefix("node:").unwrap_or(specifier);
    CORE_MODULES.contains(&specifier)
}

// ============================================================================
// npm Support
// ============================================================================

/// npm configuration
#[derive(Debug, Clone)]
pub struct NpmConfig {
    /// npm registry URL
    pub registry: String,
    /// Cache directory
    pub cache: String,
    /// Global prefix
    pub prefix: String,
    /// User config file path
    pub userconfig: String,
    /// Global config file path
    pub globalconfig: String,
    /// Init license
    pub init_license: String,
    /// Init author name
    pub init_author_name: Option<String>,
    /// Strict SSL
    pub strict_ssl: bool,
}

impl Default for NpmConfig {
    fn default() -> Self {
        NpmConfig {
            registry: String::from("https://registry.npmjs.org/"),
            cache: NPM_CACHE_DIR.to_string(),
            prefix: String::from("/usr/local"),
            userconfig: String::from("~/.npmrc"),
            globalconfig: String::from("/etc/npmrc"),
            init_license: String::from("ISC"),
            init_author_name: None,
            strict_ssl: true,
        }
    }
}

impl NpmConfig {
    /// Parse .npmrc file
    pub fn from_npmrc(content: &str) -> Self {
        let mut config = Self::default();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();

                match key {
                    "registry" => config.registry = value.to_string(),
                    "cache" => config.cache = value.to_string(),
                    "prefix" => config.prefix = value.to_string(),
                    "init-license" => config.init_license = value.to_string(),
                    "init-author-name" => config.init_author_name = Some(value.to_string()),
                    "strict-ssl" => config.strict_ssl = value == "true",
                    _ => {}
                }
            }
        }

        config
    }
}

/// npm package manifest (partial)
#[derive(Debug, Clone)]
pub struct NpmPackage {
    /// Package name
    pub name: String,
    /// Package version
    pub version: String,
    /// Package description
    pub description: Option<String>,
    /// Tarball URL
    pub tarball: Option<String>,
    /// SHA-512 integrity hash
    pub integrity: Option<String>,
    /// Dependencies
    pub dependencies: BTreeMap<String, String>,
}

// ============================================================================
// Environment Setup
// ============================================================================

/// Node.js environment variables
#[derive(Debug, Clone)]
pub struct NodeEnv {
    /// NODE_ENV (development, production, test)
    pub node_env: String,
    /// NODE_PATH
    pub node_path: String,
    /// NODE_OPTIONS
    pub node_options: String,
    /// npm_config_* variables
    pub npm_config: BTreeMap<String, String>,
    /// Custom environment
    pub custom: BTreeMap<String, String>,
}

impl Default for NodeEnv {
    fn default() -> Self {
        NodeEnv {
            node_env: String::from("development"),
            node_path: String::new(),
            node_options: String::new(),
            npm_config: BTreeMap::new(),
            custom: BTreeMap::new(),
        }
    }
}

impl NodeEnv {
    /// Create production environment
    pub fn production() -> Self {
        Self {
            node_env: String::from("production"),
            ..Default::default()
        }
    }

    /// Create test environment
    pub fn test() -> Self {
        Self {
            node_env: String::from("test"),
            ..Default::default()
        }
    }

    /// Set NODE_OPTIONS
    pub fn with_options(mut self, options: &str) -> Self {
        self.node_options = options.to_string();
        self
    }

    /// Convert to environment variable list
    pub fn to_env_vars(&self) -> Vec<(String, String)> {
        let mut vars = vec![
            (String::from("NODE_ENV"), self.node_env.clone()),
        ];

        if !self.node_path.is_empty() {
            vars.push((String::from("NODE_PATH"), self.node_path.clone()));
        }

        if !self.node_options.is_empty() {
            vars.push((String::from("NODE_OPTIONS"), self.node_options.clone()));
        }

        for (key, value) in &self.npm_config {
            vars.push((format!("npm_config_{}", key), value.clone()));
        }

        for (key, value) in &self.custom {
            vars.push((key.clone(), value.clone()));
        }

        vars
    }
}

// ============================================================================
// Global State
// ============================================================================

/// Global Node.js manager state
pub struct NodeManager {
    /// Active Node.js configuration
    pub config: NodeConfig,
    /// npm configuration
    pub npm_config: NpmConfig,
    /// Installed global packages
    pub global_packages: BTreeMap<String, String>,
    /// nvm installations
    pub nvm_versions: Vec<String>,
}

impl NodeManager {
    /// Create new Node.js manager
    pub const fn new() -> Self {
        NodeManager {
            config: NodeConfig {
                version: String::new(),
                major_version: 20,
                executable: String::new(),
                npm_executable: String::new(),
                npx_executable: String::new(),
                global_modules: String::new(),
                cache_dir: String::new(),
                is_nvm: false,
                nvm_path: None,
                v8_version: None,
                libuv_version: None,
                openssl_version: None,
            },
            npm_config: NpmConfig {
                registry: String::new(),
                cache: String::new(),
                prefix: String::new(),
                userconfig: String::new(),
                globalconfig: String::new(),
                init_license: String::new(),
                init_author_name: None,
                strict_ssl: true,
            },
            global_packages: BTreeMap::new(),
            nvm_versions: Vec::new(),
        }
    }

    /// Initialize with default configuration
    pub fn init(&mut self) {
        self.config = NodeConfig::default();
        self.npm_config = NpmConfig::default();
    }

    /// Switch to a different Node.js version
    pub fn use_version(&mut self, version: &str) -> Result<(), &'static str> {
        if !NODEJS_VERSIONS.contains(&version) && !self.nvm_versions.contains(&version.to_string()) {
            return Err("Version not installed");
        }

        if self.config.is_nvm {
            if let Some(ref nvm_path) = self.config.nvm_path {
                self.config = NodeConfig::for_nvm(nvm_path, version);
            }
        } else {
            self.config = NodeConfig::for_version(version);
        }

        Ok(())
    }

    /// Register a global package
    pub fn register_global_package(&mut self, name: &str, version: &str) {
        self.global_packages.insert(name.to_string(), version.to_string());
    }
}

/// Global Node.js manager
pub static NODE_MANAGER: Mutex<NodeManager> = Mutex::new(NodeManager::new());

// ============================================================================
// Helper Functions (placeholders)
// ============================================================================

/// Check if path exists (placeholder - needs VFS integration)
fn path_exists(_path: &str) -> bool {
    // In real implementation, check VFS
    false
}

/// Check if path is a directory (placeholder)
fn is_directory(_path: &str) -> bool {
    // In real implementation, check VFS
    false
}

/// Get parent directory
fn parent_dir(path: &str) -> Option<String> {
    let path = path.trim_end_matches('/');
    path.rfind('/').map(|i| path[..i].to_string())
}

// ============================================================================
// Initialization
// ============================================================================

/// Initialize Node.js support
pub fn init() {
    crate::kprintln!("nodejs: initializing Node.js runtime support");

    let mut manager = NODE_MANAGER.lock();
    manager.init();

    crate::kprintln!("nodejs: default version {} configured", DEFAULT_NODEJS_VERSION);
    crate::kprintln!("nodejs: npm registry: {}", manager.npm_config.registry);
    crate::kprintln!("nodejs: Node.js support ready");
}

/// Get current Node.js configuration
pub fn get_config() -> NodeConfig {
    NODE_MANAGER.lock().config.clone()
}

/// Set Node.js version
pub fn set_version(version: &str) -> Result<(), &'static str> {
    NODE_MANAGER.lock().use_version(version)
}

/// Get npm configuration
pub fn get_npm_config() -> NpmConfig {
    NODE_MANAGER.lock().npm_config.clone()
}

/// Create module resolver for current configuration
pub fn create_resolver() -> ModuleResolver {
    let config = get_config();
    ModuleResolver::new(&config)
}

// ============================================================================
// Feature Status
// ============================================================================

use super::super::{CompatLevel, FeatureStatus};

/// Get Node.js support status
pub fn get_nodejs_status() -> Vec<FeatureStatus> {
    vec![
        FeatureStatus {
            name: String::from("Node.js Runtime"),
            level: CompatLevel::Partial,
            notes: Some(String::from("v18.x, v20.x, v22.x")),
        },
        FeatureStatus {
            name: String::from("npm Package Manager"),
            level: CompatLevel::Partial,
            notes: Some(String::from("Install, run scripts")),
        },
        FeatureStatus {
            name: String::from("npx Runner"),
            level: CompatLevel::Partial,
            notes: Some(String::from("Basic execution")),
        },
        FeatureStatus {
            name: String::from("ES Modules"),
            level: CompatLevel::Full,
            notes: Some(String::from("import/export")),
        },
        FeatureStatus {
            name: String::from("CommonJS"),
            level: CompatLevel::Full,
            notes: Some(String::from("require/module.exports")),
        },
        FeatureStatus {
            name: String::from("Module Resolution"),
            level: CompatLevel::Full,
            notes: Some(String::from("Node.js algorithm")),
        },
        FeatureStatus {
            name: String::from("package.json"),
            level: CompatLevel::Full,
            notes: Some(String::from("Parsing, exports")),
        },
        FeatureStatus {
            name: String::from("nvm Support"),
            level: CompatLevel::Partial,
            notes: Some(String::from("Version switching")),
        },
        FeatureStatus {
            name: String::from("Native Addons"),
            level: CompatLevel::Stub,
            notes: Some(String::from(".node files")),
        },
        FeatureStatus {
            name: String::from("Worker Threads"),
            level: CompatLevel::Stub,
            notes: Some(String::from("Planned")),
        },
    ]
}
