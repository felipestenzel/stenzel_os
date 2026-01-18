//! Cross-Compilation Support
//!
//! Provides functionality for cross-compiling packages:
//! - Target architecture configuration
//! - Cross-compiler toolchain management
//! - Sysroot management
//! - Cross-compilation environment setup

#![allow(dead_code)]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::format;
use alloc::vec::Vec;
use alloc::vec;
use alloc::collections::BTreeMap;
use crate::sync::IrqSafeMutex;
use crate::util::{KResult, KError};

// ============================================================================
// Target Architecture
// ============================================================================

/// Target architecture for cross-compilation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetArch {
    /// Architecture name (x86_64, aarch64, arm, riscv64, etc.)
    pub arch: String,
    /// Vendor (unknown, pc, apple, etc.)
    pub vendor: String,
    /// Operating system (linux, none, windows, etc.)
    pub os: String,
    /// ABI/environment (gnu, musl, eabi, etc.)
    pub abi: String,
    /// Endianness
    pub endian: Endianness,
    /// Pointer size in bits
    pub pointer_width: u32,
    /// CPU features
    pub features: Vec<String>,
}

/// Endianness
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endianness {
    Little,
    Big,
}

impl TargetArch {
    /// Create a new target from triple string
    pub fn from_triple(triple: &str) -> Option<Self> {
        let parts: Vec<&str> = triple.split('-').collect();
        if parts.len() < 2 {
            return None;
        }

        let arch = parts[0].to_string();
        let vendor = if parts.len() >= 3 { parts[1].to_string() } else { "unknown".to_string() };
        let os = if parts.len() >= 3 { parts[2].to_string() } else { parts[1].to_string() };
        let abi = if parts.len() >= 4 { parts[3].to_string() } else { "gnu".to_string() };

        let (endian, pointer_width) = match arch.as_str() {
            "x86_64" | "amd64" => (Endianness::Little, 64),
            "i386" | "i486" | "i586" | "i686" | "x86" => (Endianness::Little, 32),
            "aarch64" | "arm64" => (Endianness::Little, 64),
            "arm" | "armv7" | "armv7l" => (Endianness::Little, 32),
            "armeb" => (Endianness::Big, 32),
            "riscv64" => (Endianness::Little, 64),
            "riscv32" => (Endianness::Little, 32),
            "mips" | "mips64" => (Endianness::Big, 64),
            "mipsel" | "mips64el" => (Endianness::Little, 64),
            "powerpc64" | "ppc64" => (Endianness::Big, 64),
            "powerpc64le" | "ppc64le" => (Endianness::Little, 64),
            "s390x" => (Endianness::Big, 64),
            _ => (Endianness::Little, 64), // Default
        };

        Some(TargetArch {
            arch,
            vendor,
            os,
            abi,
            endian,
            pointer_width,
            features: Vec::new(),
        })
    }

    /// Get the target triple string
    pub fn triple(&self) -> String {
        format!("{}-{}-{}-{}", self.arch, self.vendor, self.os, self.abi)
    }

    /// Get the short triple (arch-os-abi)
    pub fn short_triple(&self) -> String {
        format!("{}-{}-{}", self.arch, self.os, self.abi)
    }

    /// Check if target is native (same as host)
    pub fn is_native(&self, host: &TargetArch) -> bool {
        self.arch == host.arch && self.os == host.os && self.abi == host.abi
    }

    /// Get architecture family
    pub fn arch_family(&self) -> &'static str {
        match self.arch.as_str() {
            "x86_64" | "amd64" => "x86_64",
            "i386" | "i486" | "i586" | "i686" | "x86" => "x86",
            "aarch64" | "arm64" => "aarch64",
            "arm" | "armv7" | "armv7l" | "armeb" => "arm",
            "riscv64" | "riscv32" => "riscv",
            "mips" | "mips64" | "mipsel" | "mips64el" => "mips",
            "powerpc64" | "ppc64" | "powerpc64le" | "ppc64le" => "powerpc",
            "s390x" => "s390",
            _ => "unknown",
        }
    }
}

/// Common target configurations
impl TargetArch {
    pub fn x86_64_linux_gnu() -> Self {
        TargetArch {
            arch: "x86_64".to_string(),
            vendor: "unknown".to_string(),
            os: "linux".to_string(),
            abi: "gnu".to_string(),
            endian: Endianness::Little,
            pointer_width: 64,
            features: Vec::new(),
        }
    }

    pub fn aarch64_linux_gnu() -> Self {
        TargetArch {
            arch: "aarch64".to_string(),
            vendor: "unknown".to_string(),
            os: "linux".to_string(),
            abi: "gnu".to_string(),
            endian: Endianness::Little,
            pointer_width: 64,
            features: Vec::new(),
        }
    }

    pub fn arm_linux_gnueabihf() -> Self {
        TargetArch {
            arch: "arm".to_string(),
            vendor: "unknown".to_string(),
            os: "linux".to_string(),
            abi: "gnueabihf".to_string(),
            endian: Endianness::Little,
            pointer_width: 32,
            features: Vec::new(),
        }
    }

    pub fn riscv64_linux_gnu() -> Self {
        TargetArch {
            arch: "riscv64".to_string(),
            vendor: "unknown".to_string(),
            os: "linux".to_string(),
            abi: "gnu".to_string(),
            endian: Endianness::Little,
            pointer_width: 64,
            features: Vec::new(),
        }
    }

    pub fn x86_64_linux_musl() -> Self {
        TargetArch {
            arch: "x86_64".to_string(),
            vendor: "unknown".to_string(),
            os: "linux".to_string(),
            abi: "musl".to_string(),
            endian: Endianness::Little,
            pointer_width: 64,
            features: Vec::new(),
        }
    }
}

// ============================================================================
// Toolchain
// ============================================================================

/// Cross-compiler toolchain
#[derive(Debug, Clone)]
pub struct Toolchain {
    /// Toolchain name/ID
    pub name: String,
    /// Target architecture
    pub target: TargetArch,
    /// Toolchain prefix (e.g., "aarch64-linux-gnu-")
    pub prefix: String,
    /// Toolchain root directory
    pub root: String,
    /// Path to GCC/compiler
    pub cc: String,
    /// Path to C++ compiler
    pub cxx: String,
    /// Path to assembler
    pub as_path: String,
    /// Path to linker
    pub ld: String,
    /// Path to archiver
    pub ar: String,
    /// Path to strip
    pub strip: String,
    /// Path to objcopy
    pub objcopy: String,
    /// Path to objdump
    pub objdump: String,
    /// Path to ranlib
    pub ranlib: String,
    /// Path to nm
    pub nm: String,
    /// Sysroot path
    pub sysroot: Option<String>,
    /// GCC version
    pub gcc_version: Option<String>,
    /// Is LLVM/Clang toolchain
    pub is_llvm: bool,
}

impl Toolchain {
    /// Create a GNU toolchain with standard paths
    pub fn gnu(name: &str, target: TargetArch, prefix: &str, root: &str) -> Self {
        let bin_dir = format!("{}/bin", root);

        Toolchain {
            name: name.to_string(),
            target,
            prefix: prefix.to_string(),
            root: root.to_string(),
            cc: format!("{}/{}gcc", bin_dir, prefix),
            cxx: format!("{}/{}g++", bin_dir, prefix),
            as_path: format!("{}/{}as", bin_dir, prefix),
            ld: format!("{}/{}ld", bin_dir, prefix),
            ar: format!("{}/{}ar", bin_dir, prefix),
            strip: format!("{}/{}strip", bin_dir, prefix),
            objcopy: format!("{}/{}objcopy", bin_dir, prefix),
            objdump: format!("{}/{}objdump", bin_dir, prefix),
            ranlib: format!("{}/{}ranlib", bin_dir, prefix),
            nm: format!("{}/{}nm", bin_dir, prefix),
            sysroot: Some(format!("{}/{}/sysroot", root, prefix.trim_end_matches('-'))),
            gcc_version: None,
            is_llvm: false,
        }
    }

    /// Create an LLVM/Clang toolchain
    pub fn llvm(name: &str, target: TargetArch, root: &str, sysroot: Option<&str>) -> Self {
        let bin_dir = format!("{}/bin", root);
        let triple = target.triple();

        Toolchain {
            name: name.to_string(),
            target,
            prefix: String::new(),
            root: root.to_string(),
            cc: format!("{}/clang", bin_dir),
            cxx: format!("{}/clang++", bin_dir),
            as_path: format!("{}/llvm-as", bin_dir),
            ld: format!("{}/ld.lld", bin_dir),
            ar: format!("{}/llvm-ar", bin_dir),
            strip: format!("{}/llvm-strip", bin_dir),
            objcopy: format!("{}/llvm-objcopy", bin_dir),
            objdump: format!("{}/llvm-objdump", bin_dir),
            ranlib: format!("{}/llvm-ranlib", bin_dir),
            nm: format!("{}/llvm-nm", bin_dir),
            sysroot: sysroot.map(|s| s.to_string()),
            gcc_version: None,
            is_llvm: true,
        }
    }

    /// Get environment variables for this toolchain
    pub fn env_vars(&self) -> BTreeMap<String, String> {
        let mut env = BTreeMap::new();

        env.insert("CC".to_string(), self.cc.clone());
        env.insert("CXX".to_string(), self.cxx.clone());
        env.insert("AS".to_string(), self.as_path.clone());
        env.insert("LD".to_string(), self.ld.clone());
        env.insert("AR".to_string(), self.ar.clone());
        env.insert("STRIP".to_string(), self.strip.clone());
        env.insert("OBJCOPY".to_string(), self.objcopy.clone());
        env.insert("OBJDUMP".to_string(), self.objdump.clone());
        env.insert("RANLIB".to_string(), self.ranlib.clone());
        env.insert("NM".to_string(), self.nm.clone());

        if let Some(ref sysroot) = self.sysroot {
            env.insert("SYSROOT".to_string(), sysroot.clone());
        }

        // Target triple
        env.insert("TARGET".to_string(), self.target.triple());
        env.insert("CROSS_COMPILE".to_string(), self.prefix.clone());

        env
    }

    /// Get compiler flags for cross-compilation
    pub fn compiler_flags(&self) -> Vec<String> {
        let mut flags = Vec::new();

        if self.is_llvm {
            flags.push(format!("--target={}", self.target.triple()));
        }

        if let Some(ref sysroot) = self.sysroot {
            flags.push(format!("--sysroot={}", sysroot));
        }

        flags
    }
}

// ============================================================================
// Sysroot
// ============================================================================

/// Sysroot for cross-compilation
#[derive(Debug, Clone)]
pub struct Sysroot {
    /// Sysroot name
    pub name: String,
    /// Target architecture
    pub target: TargetArch,
    /// Root path
    pub path: String,
    /// Include directories
    pub include_dirs: Vec<String>,
    /// Library directories
    pub lib_dirs: Vec<String>,
    /// PKG-config path
    pub pkg_config_path: Option<String>,
    /// Is complete (has libc, headers, etc.)
    pub is_complete: bool,
}

impl Sysroot {
    /// Create a new sysroot
    pub fn new(name: &str, target: TargetArch, path: &str) -> Self {
        let include_dirs = vec![
            format!("{}/usr/include", path),
            format!("{}/include", path),
        ];

        let lib_dirs = vec![
            format!("{}/usr/lib", path),
            format!("{}/usr/lib64", path),
            format!("{}/lib", path),
            format!("{}/lib64", path),
        ];

        Sysroot {
            name: name.to_string(),
            target,
            path: path.to_string(),
            include_dirs,
            lib_dirs,
            pkg_config_path: Some(format!("{}/usr/lib/pkgconfig", path)),
            is_complete: false,
        }
    }

    /// Get include flags (-I)
    pub fn include_flags(&self) -> Vec<String> {
        self.include_dirs.iter()
            .map(|d| format!("-I{}", d))
            .collect()
    }

    /// Get library flags (-L)
    pub fn lib_flags(&self) -> Vec<String> {
        self.lib_dirs.iter()
            .map(|d| format!("-L{}", d))
            .collect()
    }
}

// ============================================================================
// Cross-Build Environment
// ============================================================================

/// Cross-compilation build environment
#[derive(Debug, Clone)]
pub struct CrossBuildEnv {
    /// Host architecture (machine doing the building)
    pub host: TargetArch,
    /// Target architecture (machine that will run the code)
    pub target: TargetArch,
    /// Build architecture (usually same as host)
    pub build: TargetArch,
    /// Toolchain to use
    pub toolchain: Toolchain,
    /// Sysroot to use
    pub sysroot: Option<Sysroot>,
    /// Extra CFLAGS
    pub cflags: String,
    /// Extra CXXFLAGS
    pub cxxflags: String,
    /// Extra LDFLAGS
    pub ldflags: String,
    /// Extra environment variables
    pub env: BTreeMap<String, String>,
    /// Configure flags for autotools
    pub configure_flags: Vec<String>,
    /// CMake flags
    pub cmake_flags: Vec<String>,
    /// Meson cross file path
    pub meson_cross_file: Option<String>,
}

impl CrossBuildEnv {
    /// Create a new cross-build environment
    pub fn new(host: TargetArch, target: TargetArch, toolchain: Toolchain) -> Self {
        let configure_flags = vec![
            format!("--host={}", target.triple()),
            format!("--build={}", host.triple()),
        ];

        let cmake_flags = vec![
            format!("-DCMAKE_SYSTEM_NAME={}", Self::cmake_system_name(&target.os)),
            format!("-DCMAKE_SYSTEM_PROCESSOR={}", target.arch),
            format!("-DCMAKE_C_COMPILER={}", toolchain.cc),
            format!("-DCMAKE_CXX_COMPILER={}", toolchain.cxx),
        ];

        CrossBuildEnv {
            build: host.clone(),
            host,
            target,
            toolchain,
            sysroot: None,
            cflags: String::new(),
            cxxflags: String::new(),
            ldflags: String::new(),
            env: BTreeMap::new(),
            configure_flags,
            cmake_flags,
            meson_cross_file: None,
        }
    }

    /// Set sysroot
    pub fn with_sysroot(mut self, sysroot: Sysroot) -> Self {
        if let Some(ref sr) = sysroot.pkg_config_path {
            self.env.insert("PKG_CONFIG_PATH".to_string(), sr.clone());
            self.env.insert("PKG_CONFIG_SYSROOT_DIR".to_string(), sysroot.path.clone());
        }

        // Add sysroot to compiler flags
        let sysroot_flag = format!("--sysroot={}", sysroot.path);
        if !self.cflags.is_empty() {
            self.cflags.push(' ');
        }
        self.cflags.push_str(&sysroot_flag);

        if !self.cxxflags.is_empty() {
            self.cxxflags.push(' ');
        }
        self.cxxflags.push_str(&sysroot_flag);

        if !self.ldflags.is_empty() {
            self.ldflags.push(' ');
        }
        self.ldflags.push_str(&sysroot_flag);

        // Update cmake flags
        self.cmake_flags.push(format!("-DCMAKE_SYSROOT={}", sysroot.path));
        self.cmake_flags.push(format!("-DCMAKE_FIND_ROOT_PATH={}", sysroot.path));
        self.cmake_flags.push("-DCMAKE_FIND_ROOT_PATH_MODE_PROGRAM=NEVER".to_string());
        self.cmake_flags.push("-DCMAKE_FIND_ROOT_PATH_MODE_LIBRARY=ONLY".to_string());
        self.cmake_flags.push("-DCMAKE_FIND_ROOT_PATH_MODE_INCLUDE=ONLY".to_string());

        self.sysroot = Some(sysroot);
        self
    }

    /// Get all environment variables
    pub fn get_env(&self) -> BTreeMap<String, String> {
        let mut env = self.toolchain.env_vars();

        // Add compiler flags
        if !self.cflags.is_empty() {
            env.insert("CFLAGS".to_string(), self.cflags.clone());
        }
        if !self.cxxflags.is_empty() {
            env.insert("CXXFLAGS".to_string(), self.cxxflags.clone());
        }
        if !self.ldflags.is_empty() {
            env.insert("LDFLAGS".to_string(), self.ldflags.clone());
        }

        // Add custom env
        for (k, v) in &self.env {
            env.insert(k.clone(), v.clone());
        }

        env
    }

    /// Get CMake system name for OS
    fn cmake_system_name(os: &str) -> &'static str {
        match os {
            "linux" => "Linux",
            "windows" => "Windows",
            "darwin" | "macos" => "Darwin",
            "freebsd" => "FreeBSD",
            "none" => "Generic",
            _ => "Linux",
        }
    }

    /// Generate meson cross file content
    pub fn generate_meson_cross_file(&self) -> String {
        let cpu_family = self.target.arch_family();
        let endian = match self.target.endian {
            Endianness::Little => "little",
            Endianness::Big => "big",
        };

        let mut content = String::new();

        content.push_str("[binaries]\n");
        content.push_str(&format!("c = '{}'\n", self.toolchain.cc));
        content.push_str(&format!("cpp = '{}'\n", self.toolchain.cxx));
        content.push_str(&format!("ar = '{}'\n", self.toolchain.ar));
        content.push_str(&format!("strip = '{}'\n", self.toolchain.strip));
        content.push_str(&format!("ld = '{}'\n", self.toolchain.ld));
        content.push_str("\n");

        content.push_str("[host_machine]\n");
        content.push_str(&format!("system = '{}'\n", self.target.os));
        content.push_str(&format!("cpu_family = '{}'\n", cpu_family));
        content.push_str(&format!("cpu = '{}'\n", self.target.arch));
        content.push_str(&format!("endian = '{}'\n", endian));

        if let Some(ref sysroot) = self.sysroot {
            content.push_str("\n[built-in options]\n");
            content.push_str(&format!("sys_root = '{}'\n", sysroot.path));
        }

        content
    }
}

// ============================================================================
// Cross-Build Manager
// ============================================================================

/// Cross-build manager state
struct CrossBuildManager {
    /// Registered toolchains
    toolchains: BTreeMap<String, Toolchain>,
    /// Registered sysroots
    sysroots: BTreeMap<String, Sysroot>,
    /// Current host architecture
    host: Option<TargetArch>,
    /// Default toolchain per target
    default_toolchains: BTreeMap<String, String>,
}

impl CrossBuildManager {
    const fn new() -> Self {
        CrossBuildManager {
            toolchains: BTreeMap::new(),
            sysroots: BTreeMap::new(),
            host: None,
            default_toolchains: BTreeMap::new(),
        }
    }
}

/// Global cross-build manager
static CROSS_MANAGER: IrqSafeMutex<CrossBuildManager> = IrqSafeMutex::new(CrossBuildManager::new());

// ============================================================================
// Public API
// ============================================================================

/// Initialize cross-compilation system
pub fn init() -> KResult<()> {
    let mut mgr = CROSS_MANAGER.lock();

    // Set host architecture
    mgr.host = Some(TargetArch::x86_64_linux_gnu());

    crate::kprintln!("spkg: cross-compilation system initialized");
    Ok(())
}

/// Register a toolchain
pub fn register_toolchain(toolchain: Toolchain) -> KResult<()> {
    let mut mgr = CROSS_MANAGER.lock();

    if mgr.toolchains.contains_key(&toolchain.name) {
        return Err(KError::AlreadyExists);
    }

    mgr.toolchains.insert(toolchain.name.clone(), toolchain);
    Ok(())
}

/// Remove a toolchain
pub fn remove_toolchain(name: &str) -> KResult<()> {
    let mut mgr = CROSS_MANAGER.lock();
    mgr.toolchains.remove(name).ok_or(KError::NotFound)?;
    Ok(())
}

/// Get a toolchain by name
pub fn get_toolchain(name: &str) -> Option<Toolchain> {
    let mgr = CROSS_MANAGER.lock();
    mgr.toolchains.get(name).cloned()
}

/// List all toolchains
pub fn list_toolchains() -> Vec<Toolchain> {
    let mgr = CROSS_MANAGER.lock();
    mgr.toolchains.values().cloned().collect()
}

/// Set default toolchain for a target
pub fn set_default_toolchain(target_triple: &str, toolchain_name: &str) -> KResult<()> {
    let mut mgr = CROSS_MANAGER.lock();

    if !mgr.toolchains.contains_key(toolchain_name) {
        return Err(KError::NotFound);
    }

    mgr.default_toolchains.insert(target_triple.to_string(), toolchain_name.to_string());
    Ok(())
}

/// Get default toolchain for a target
pub fn get_default_toolchain(target_triple: &str) -> Option<Toolchain> {
    let mgr = CROSS_MANAGER.lock();
    mgr.default_toolchains.get(target_triple)
        .and_then(|name| mgr.toolchains.get(name))
        .cloned()
}

/// Register a sysroot
pub fn register_sysroot(sysroot: Sysroot) -> KResult<()> {
    let mut mgr = CROSS_MANAGER.lock();

    if mgr.sysroots.contains_key(&sysroot.name) {
        return Err(KError::AlreadyExists);
    }

    mgr.sysroots.insert(sysroot.name.clone(), sysroot);
    Ok(())
}

/// Remove a sysroot
pub fn remove_sysroot(name: &str) -> KResult<()> {
    let mut mgr = CROSS_MANAGER.lock();
    mgr.sysroots.remove(name).ok_or(KError::NotFound)?;
    Ok(())
}

/// Get a sysroot by name
pub fn get_sysroot(name: &str) -> Option<Sysroot> {
    let mgr = CROSS_MANAGER.lock();
    mgr.sysroots.get(name).cloned()
}

/// List all sysroots
pub fn list_sysroots() -> Vec<Sysroot> {
    let mgr = CROSS_MANAGER.lock();
    mgr.sysroots.values().cloned().collect()
}

/// Get host architecture
pub fn get_host() -> Option<TargetArch> {
    let mgr = CROSS_MANAGER.lock();
    mgr.host.clone()
}

/// Create a cross-build environment
pub fn create_cross_env(target_triple: &str) -> KResult<CrossBuildEnv> {
    let mgr = CROSS_MANAGER.lock();

    let host = mgr.host.clone().ok_or(KError::NotSupported)?;
    let target = TargetArch::from_triple(target_triple).ok_or(KError::Invalid)?;

    // Find toolchain
    let toolchain_name = mgr.default_toolchains.get(target_triple)
        .or_else(|| {
            // Try to find any matching toolchain
            mgr.toolchains.iter()
                .find(|(_, tc)| tc.target.triple() == target_triple)
                .map(|(name, _)| name)
        })
        .ok_or(KError::NotFound)?;

    let toolchain = mgr.toolchains.get(toolchain_name)
        .ok_or(KError::NotFound)?
        .clone();

    drop(mgr);

    let mut env = CrossBuildEnv::new(host, target.clone(), toolchain);

    // Find matching sysroot
    if let Some(sysroot) = find_sysroot_for_target(&target) {
        env = env.with_sysroot(sysroot);
    }

    Ok(env)
}

/// Find a sysroot for a target
fn find_sysroot_for_target(target: &TargetArch) -> Option<Sysroot> {
    let mgr = CROSS_MANAGER.lock();
    mgr.sysroots.values()
        .find(|sr| sr.target.triple() == target.triple())
        .cloned()
}

/// Check if cross-compilation is available for a target
pub fn is_target_supported(target_triple: &str) -> bool {
    let mgr = CROSS_MANAGER.lock();

    mgr.default_toolchains.contains_key(target_triple) ||
    mgr.toolchains.values().any(|tc| tc.target.triple() == target_triple)
}

/// List all supported target architectures
pub fn list_supported_targets() -> Vec<String> {
    let mgr = CROSS_MANAGER.lock();

    let mut targets: Vec<String> = mgr.toolchains.values()
        .map(|tc| tc.target.triple())
        .collect();

    targets.sort();
    targets.dedup();
    targets
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Get common cross-compile targets
pub fn common_targets() -> Vec<TargetArch> {
    vec![
        TargetArch::x86_64_linux_gnu(),
        TargetArch::aarch64_linux_gnu(),
        TargetArch::arm_linux_gnueabihf(),
        TargetArch::riscv64_linux_gnu(),
        TargetArch::x86_64_linux_musl(),
    ]
}

/// Parse target from environment (CROSS_COMPILE, etc.)
pub fn parse_target_from_env(env: &BTreeMap<String, String>) -> Option<TargetArch> {
    if let Some(target) = env.get("TARGET") {
        return TargetArch::from_triple(target);
    }

    if let Some(cross) = env.get("CROSS_COMPILE") {
        // CROSS_COMPILE is usually like "aarch64-linux-gnu-"
        let triple = cross.trim_end_matches('-');
        return TargetArch::from_triple(triple);
    }

    None
}

// ============================================================================
// Statistics
// ============================================================================

/// Cross-compilation statistics
#[derive(Debug, Clone)]
pub struct CrossStats {
    /// Number of registered toolchains
    pub toolchain_count: usize,
    /// Number of registered sysroots
    pub sysroot_count: usize,
    /// Number of supported targets
    pub target_count: usize,
    /// Has LLVM toolchain
    pub has_llvm: bool,
}

/// Get cross-compilation statistics
pub fn get_stats() -> CrossStats {
    let mgr = CROSS_MANAGER.lock();

    let has_llvm = mgr.toolchains.values().any(|tc| tc.is_llvm);
    let target_count = list_supported_targets().len();

    CrossStats {
        toolchain_count: mgr.toolchains.len(),
        sysroot_count: mgr.sysroots.len(),
        target_count,
        has_llvm,
    }
}
