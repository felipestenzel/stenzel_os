//! Rust Runtime Support
//!
//! Provides infrastructure for running Rust programs and the Rust toolchain
//! (rustc, cargo, rustup) on Stenzel OS.
//!
//! Supports:
//! - rustc compiler
//! - cargo package manager
//! - rustup toolchain management
//! - Cross-compilation targets
//! - crates.io registry

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use alloc::collections::BTreeMap;
use spin::Mutex;

/// Default Rust toolchain version
pub const DEFAULT_RUST_VERSION: &str = "1.75.0";

/// Supported Rust editions
pub const SUPPORTED_EDITIONS: &[&str] = &["2015", "2018", "2021", "2024"];

/// Default edition
pub const DEFAULT_EDITION: &str = "2021";

/// Rust toolchain channels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustChannel {
    Stable,
    Beta,
    Nightly,
}

impl RustChannel {
    pub fn as_str(&self) -> &'static str {
        match self {
            RustChannel::Stable => "stable",
            RustChannel::Beta => "beta",
            RustChannel::Nightly => "nightly",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "stable" => Some(RustChannel::Stable),
            "beta" => Some(RustChannel::Beta),
            "nightly" => Some(RustChannel::Nightly),
            _ => None,
        }
    }
}

/// Rust target triple
#[derive(Debug, Clone)]
pub struct RustTarget {
    /// Target triple string
    pub triple: String,
    /// Architecture
    pub arch: String,
    /// Vendor
    pub vendor: String,
    /// OS
    pub os: String,
    /// Environment/ABI
    pub env: Option<String>,
}

impl RustTarget {
    /// x86_64-unknown-linux-gnu
    pub fn x86_64_linux_gnu() -> Self {
        RustTarget {
            triple: String::from("x86_64-unknown-linux-gnu"),
            arch: String::from("x86_64"),
            vendor: String::from("unknown"),
            os: String::from("linux"),
            env: Some(String::from("gnu")),
        }
    }

    /// x86_64-unknown-linux-musl
    pub fn x86_64_linux_musl() -> Self {
        RustTarget {
            triple: String::from("x86_64-unknown-linux-musl"),
            arch: String::from("x86_64"),
            vendor: String::from("unknown"),
            os: String::from("linux"),
            env: Some(String::from("musl")),
        }
    }

    /// x86_64-stenzel-elf (native)
    pub fn stenzel_os() -> Self {
        RustTarget {
            triple: String::from("x86_64-stenzel-elf"),
            arch: String::from("x86_64"),
            vendor: String::from("stenzel"),
            os: String::from("none"),
            env: Some(String::from("elf")),
        }
    }

    /// x86_64-unknown-none (bare metal)
    pub fn x86_64_none() -> Self {
        RustTarget {
            triple: String::from("x86_64-unknown-none"),
            arch: String::from("x86_64"),
            vendor: String::from("unknown"),
            os: String::from("none"),
            env: None,
        }
    }
}

/// Built-in targets available
pub const BUILTIN_TARGETS: &[&str] = &[
    "x86_64-unknown-linux-gnu",
    "x86_64-unknown-linux-musl",
    "x86_64-unknown-none",
    "x86_64-pc-windows-gnu",
    "x86_64-pc-windows-msvc",
    "x86_64-apple-darwin",
    "aarch64-unknown-linux-gnu",
    "aarch64-unknown-linux-musl",
    "aarch64-apple-darwin",
    "wasm32-unknown-unknown",
    "wasm32-wasi",
];

/// Rust toolchain configuration
#[derive(Debug, Clone)]
pub struct RustToolchain {
    /// Channel (stable, beta, nightly)
    pub channel: RustChannel,
    /// Version
    pub version: String,
    /// Target
    pub target: RustTarget,
    /// rustc path
    pub rustc: String,
    /// cargo path
    pub cargo: String,
    /// rustdoc path
    pub rustdoc: String,
    /// rustfmt path
    pub rustfmt: String,
    /// clippy path
    pub clippy: String,
    /// Sysroot path
    pub sysroot: String,
    /// Library path
    pub lib_path: String,
}

impl RustToolchain {
    /// Create default stable toolchain
    pub fn stable() -> Self {
        let target = RustTarget::x86_64_linux_gnu();
        let sysroot = String::from("/usr/lib/rustlib");

        RustToolchain {
            channel: RustChannel::Stable,
            version: DEFAULT_RUST_VERSION.to_string(),
            target: target.clone(),
            rustc: String::from("/usr/bin/rustc"),
            cargo: String::from("/usr/bin/cargo"),
            rustdoc: String::from("/usr/bin/rustdoc"),
            rustfmt: String::from("/usr/bin/rustfmt"),
            clippy: String::from("/usr/bin/clippy-driver"),
            sysroot: sysroot.clone(),
            lib_path: format!("{}/{}/lib", sysroot, target.triple),
        }
    }

    /// Create nightly toolchain
    pub fn nightly() -> Self {
        let mut tc = Self::stable();
        tc.channel = RustChannel::Nightly;
        tc.version = String::from("nightly");
        tc
    }

    /// Get rustc invocation arguments
    pub fn rustc_args(&self) -> Vec<String> {
        vec![
            format!("--edition={}", DEFAULT_EDITION),
            format!("--target={}", self.target.triple),
            format!("--sysroot={}", self.sysroot),
        ]
    }

    /// Get cargo invocation arguments
    pub fn cargo_args(&self) -> Vec<String> {
        vec![
            format!("--target={}", self.target.triple),
        ]
    }
}

/// Cargo configuration
#[derive(Debug, Clone)]
pub struct CargoConfig {
    /// Cargo home directory
    pub cargo_home: String,
    /// Registry index URL
    pub registry_url: String,
    /// Download directory for crates
    pub registry_cache: String,
    /// Git checkouts directory
    pub git_checkouts: String,
    /// Target directory for builds
    pub target_dir: String,
    /// Incremental compilation
    pub incremental: bool,
    /// Number of parallel jobs
    pub jobs: Option<u32>,
}

impl CargoConfig {
    pub fn default() -> Self {
        CargoConfig {
            cargo_home: String::from("/root/.cargo"),
            registry_url: String::from("https://github.com/rust-lang/crates.io-index"),
            registry_cache: String::from("/root/.cargo/registry/cache"),
            git_checkouts: String::from("/root/.cargo/git/checkouts"),
            target_dir: String::from("target"),
            incremental: true,
            jobs: None,
        }
    }

    /// Get environment variables
    pub fn env_vars(&self) -> Vec<(String, String)> {
        let mut vars = vec![
            (String::from("CARGO_HOME"), self.cargo_home.clone()),
            (String::from("CARGO_TARGET_DIR"), self.target_dir.clone()),
        ];

        if self.incremental {
            vars.push((String::from("CARGO_INCREMENTAL"), String::from("1")));
        }

        if let Some(jobs) = self.jobs {
            vars.push((String::from("CARGO_BUILD_JOBS"), jobs.to_string()));
        }

        vars
    }
}

/// Cargo.toml manifest structure (simplified)
#[derive(Debug, Clone)]
pub struct CargoManifest {
    pub name: String,
    pub version: String,
    pub edition: String,
    pub authors: Vec<String>,
    pub description: Option<String>,
    pub license: Option<String>,
    pub dependencies: BTreeMap<String, DependencySpec>,
    pub dev_dependencies: BTreeMap<String, DependencySpec>,
    pub build_dependencies: BTreeMap<String, DependencySpec>,
}

/// Dependency specification
#[derive(Debug, Clone)]
pub struct DependencySpec {
    pub version: Option<String>,
    pub git: Option<String>,
    pub branch: Option<String>,
    pub path: Option<String>,
    pub features: Vec<String>,
    pub optional: bool,
    pub default_features: bool,
}

impl DependencySpec {
    pub fn version(ver: &str) -> Self {
        DependencySpec {
            version: Some(ver.to_string()),
            git: None,
            branch: None,
            path: None,
            features: Vec::new(),
            optional: false,
            default_features: true,
        }
    }

    pub fn git(url: &str) -> Self {
        DependencySpec {
            version: None,
            git: Some(url.to_string()),
            branch: None,
            path: None,
            features: Vec::new(),
            optional: false,
            default_features: true,
        }
    }

    pub fn path(p: &str) -> Self {
        DependencySpec {
            version: None,
            git: None,
            branch: None,
            path: Some(p.to_string()),
            features: Vec::new(),
            optional: false,
            default_features: true,
        }
    }
}

/// rustup toolchain management
pub struct RustupManager {
    /// Installed toolchains
    toolchains: BTreeMap<String, RustToolchain>,
    /// Default toolchain name
    default_toolchain: String,
    /// rustup home
    rustup_home: String,
}

impl RustupManager {
    pub const fn new() -> Self {
        RustupManager {
            toolchains: BTreeMap::new(),
            default_toolchain: String::new(),
            rustup_home: String::new(),
        }
    }

    /// Initialize rustup
    pub fn init(&mut self) {
        self.rustup_home = String::from("/root/.rustup");
        self.default_toolchain = String::from("stable-x86_64-unknown-linux-gnu");

        // Register default toolchains
        let stable = RustToolchain::stable();
        self.toolchains.insert(String::from("stable-x86_64-unknown-linux-gnu"), stable);

        let nightly = RustToolchain::nightly();
        self.toolchains.insert(String::from("nightly-x86_64-unknown-linux-gnu"), nightly);
    }

    /// Get default toolchain
    pub fn default(&self) -> Option<&RustToolchain> {
        self.toolchains.get(&self.default_toolchain)
    }

    /// Set default toolchain
    pub fn set_default(&mut self, name: &str) -> bool {
        if self.toolchains.contains_key(name) {
            self.default_toolchain = name.to_string();
            true
        } else {
            false
        }
    }

    /// List installed toolchains
    pub fn list(&self) -> Vec<&String> {
        self.toolchains.keys().collect()
    }

    /// Add a toolchain
    pub fn add_toolchain(&mut self, name: &str, toolchain: RustToolchain) {
        self.toolchains.insert(name.to_string(), toolchain);
    }

    /// Remove a toolchain
    pub fn remove_toolchain(&mut self, name: &str) -> bool {
        self.toolchains.remove(name).is_some()
    }

    /// Get toolchain by name
    pub fn get(&self, name: &str) -> Option<&RustToolchain> {
        self.toolchains.get(name)
    }
}

/// Global rustup manager
pub static RUSTUP: Mutex<RustupManager> = Mutex::new(RustupManager::new());

/// Crate types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrateType {
    Bin,
    Lib,
    Rlib,
    Dylib,
    Cdylib,
    Staticlib,
    ProcMacro,
}

impl CrateType {
    pub fn as_str(&self) -> &'static str {
        match self {
            CrateType::Bin => "bin",
            CrateType::Lib => "lib",
            CrateType::Rlib => "rlib",
            CrateType::Dylib => "dylib",
            CrateType::Cdylib => "cdylib",
            CrateType::Staticlib => "staticlib",
            CrateType::ProcMacro => "proc-macro",
        }
    }

    pub fn file_extension(&self) -> &'static str {
        match self {
            CrateType::Bin => "",
            CrateType::Lib | CrateType::Rlib => ".rlib",
            CrateType::Dylib | CrateType::Cdylib => ".so",
            CrateType::Staticlib => ".a",
            CrateType::ProcMacro => ".so",
        }
    }
}

/// Optimization levels
pub mod opt_level {
    pub const O0: &str = "0";
    pub const O1: &str = "1";
    pub const O2: &str = "2";
    pub const O3: &str = "3";
    pub const OS: &str = "s";
    pub const OZ: &str = "z";
}

/// Debug info levels
pub mod debuginfo {
    pub const NONE: &str = "0";
    pub const LINE_TABLES: &str = "line-tables-only";
    pub const LIMITED: &str = "1";
    pub const FULL: &str = "2";
}

/// Panic strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanicStrategy {
    Unwind,
    Abort,
}

impl PanicStrategy {
    pub fn as_str(&self) -> &'static str {
        match self {
            PanicStrategy::Unwind => "unwind",
            PanicStrategy::Abort => "abort",
        }
    }
}

/// LTO modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LtoMode {
    Off,
    Thin,
    Fat,
}

impl LtoMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            LtoMode::Off => "off",
            LtoMode::Thin => "thin",
            LtoMode::Fat => "fat",
        }
    }
}

/// Build profile
#[derive(Debug, Clone)]
pub struct BuildProfile {
    pub name: String,
    pub opt_level: String,
    pub debug: bool,
    pub debug_assertions: bool,
    pub overflow_checks: bool,
    pub lto: LtoMode,
    pub panic: PanicStrategy,
    pub incremental: bool,
    pub codegen_units: Option<u32>,
    pub rpath: bool,
}

impl BuildProfile {
    /// Debug profile
    pub fn debug() -> Self {
        BuildProfile {
            name: String::from("dev"),
            opt_level: opt_level::O0.to_string(),
            debug: true,
            debug_assertions: true,
            overflow_checks: true,
            lto: LtoMode::Off,
            panic: PanicStrategy::Unwind,
            incremental: true,
            codegen_units: Some(256),
            rpath: false,
        }
    }

    /// Release profile
    pub fn release() -> Self {
        BuildProfile {
            name: String::from("release"),
            opt_level: opt_level::O3.to_string(),
            debug: false,
            debug_assertions: false,
            overflow_checks: false,
            lto: LtoMode::Off,
            panic: PanicStrategy::Unwind,
            incremental: false,
            codegen_units: Some(16),
            rpath: false,
        }
    }

    /// Release with optimizations for size
    pub fn release_small() -> Self {
        BuildProfile {
            name: String::from("release-small"),
            opt_level: opt_level::OZ.to_string(),
            debug: false,
            debug_assertions: false,
            overflow_checks: false,
            lto: LtoMode::Thin,
            panic: PanicStrategy::Abort,
            incremental: false,
            codegen_units: Some(1),
            rpath: false,
        }
    }

    /// Get rustc flags for this profile
    pub fn rustc_flags(&self) -> Vec<String> {
        let mut flags = vec![
            format!("-C opt-level={}", self.opt_level),
        ];

        if self.debug {
            flags.push(String::from("-C debuginfo=2"));
        }

        flags.push(format!("-C panic={}", self.panic.as_str()));

        if self.lto != LtoMode::Off {
            flags.push(format!("-C lto={}", self.lto.as_str()));
        }

        if let Some(units) = self.codegen_units {
            flags.push(format!("-C codegen-units={}", units));
        }

        flags
    }
}

/// Commonly used crates from crates.io
pub const COMMON_CRATES: &[(&str, &str)] = &[
    ("serde", "1.0"),
    ("serde_json", "1.0"),
    ("tokio", "1.0"),
    ("async-std", "1.0"),
    ("reqwest", "0.11"),
    ("clap", "4.0"),
    ("log", "0.4"),
    ("env_logger", "0.10"),
    ("rand", "0.8"),
    ("regex", "1.0"),
    ("chrono", "0.4"),
    ("uuid", "1.0"),
    ("anyhow", "1.0"),
    ("thiserror", "1.0"),
    ("lazy_static", "1.0"),
    ("once_cell", "1.0"),
    ("parking_lot", "0.12"),
    ("crossbeam", "0.8"),
    ("rayon", "1.0"),
    ("itertools", "0.12"),
    ("bytes", "1.0"),
    ("futures", "0.3"),
    ("hyper", "1.0"),
    ("axum", "0.7"),
    ("sqlx", "0.7"),
    ("diesel", "2.0"),
    ("tracing", "0.1"),
    ("proc-macro2", "1.0"),
    ("syn", "2.0"),
    ("quote", "1.0"),
];

/// Rust standard library crates
pub const STD_CRATES: &[&str] = &[
    "alloc",
    "core",
    "std",
    "proc_macro",
    "test",
];

/// Installed crate registry
pub static INSTALLED_CRATES: Mutex<BTreeMap<String, String>> = Mutex::new(BTreeMap::new());

/// Register an installed crate
pub fn register_crate(name: &str, version: &str) {
    let mut crates = INSTALLED_CRATES.lock();
    crates.insert(name.to_string(), version.to_string());
}

/// Check if a crate is installed
pub fn is_crate_installed(name: &str) -> bool {
    let crates = INSTALLED_CRATES.lock();
    crates.contains_key(name)
}

/// Get installed crate version
pub fn get_crate_version(name: &str) -> Option<String> {
    let crates = INSTALLED_CRATES.lock();
    crates.get(name).cloned()
}

/// Initialize Rust runtime support
pub fn init() {
    crate::kprintln!("rust: initializing Rust runtime support");

    // Initialize rustup
    let mut rustup = RUSTUP.lock();
    rustup.init();

    crate::kprintln!("rust: default toolchain: {}", DEFAULT_RUST_VERSION);
    crate::kprintln!("rust: default edition: {}", DEFAULT_EDITION);
    crate::kprintln!("rust: {} built-in targets available", BUILTIN_TARGETS.len());
    crate::kprintln!("rust: Rust runtime support ready");
}

/// Check if Rust is available
pub fn is_rust_available() -> bool {
    let rustup = RUSTUP.lock();
    rustup.default().is_some()
}

/// Get rustc version
pub fn rustc_version() -> String {
    format!("rustc {} (Stenzel OS)", DEFAULT_RUST_VERSION)
}

/// Get cargo version
pub fn cargo_version() -> String {
    format!("cargo {}", DEFAULT_RUST_VERSION)
}

/// Create a new Cargo project
pub fn cargo_new(name: &str, is_lib: bool) -> CargoManifest {
    CargoManifest {
        name: name.to_string(),
        version: String::from("0.1.0"),
        edition: DEFAULT_EDITION.to_string(),
        authors: vec![String::from("Stenzel OS User")],
        description: None,
        license: None,
        dependencies: BTreeMap::new(),
        dev_dependencies: BTreeMap::new(),
        build_dependencies: BTreeMap::new(),
    }
}

/// Parse Cargo.toml (simplified)
pub fn parse_cargo_toml(_content: &str) -> Option<CargoManifest> {
    // In a full implementation, this would parse TOML
    // For now, return a default manifest
    Some(CargoManifest {
        name: String::from("unknown"),
        version: String::from("0.0.0"),
        edition: DEFAULT_EDITION.to_string(),
        authors: Vec::new(),
        description: None,
        license: None,
        dependencies: BTreeMap::new(),
        dev_dependencies: BTreeMap::new(),
        build_dependencies: BTreeMap::new(),
    })
}

/// Get rustc command for compiling a file
pub fn get_rustc_command(
    source: &str,
    output: &str,
    crate_type: CrateType,
    profile: &BuildProfile,
) -> Vec<String> {
    let rustup = RUSTUP.lock();
    let toolchain = rustup.default().unwrap();

    let mut cmd = vec![
        toolchain.rustc.clone(),
        source.to_string(),
        format!("-o {}", output),
        format!("--crate-type={}", crate_type.as_str()),
        format!("--edition={}", DEFAULT_EDITION),
    ];

    cmd.extend(profile.rustc_flags());
    cmd.extend(toolchain.rustc_args());

    cmd
}

/// Get cargo build command
pub fn get_cargo_build_command(release: bool, target: Option<&str>) -> Vec<String> {
    let rustup = RUSTUP.lock();
    let toolchain = rustup.default().unwrap();

    let mut cmd = vec![
        toolchain.cargo.clone(),
        String::from("build"),
    ];

    if release {
        cmd.push(String::from("--release"));
    }

    if let Some(t) = target {
        cmd.push(format!("--target={}", t));
    }

    cmd
}
