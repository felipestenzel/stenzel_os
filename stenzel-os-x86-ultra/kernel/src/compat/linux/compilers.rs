//! GCC/Clang Native Compilers Support
//!
//! Provides infrastructure and configuration for running native compilers
//! (GCC, Clang, rustc) on Stenzel OS.
//!
//! This module handles:
//! - Compiler toolchain detection
//! - Default include/library paths
//! - Target triple configuration
//! - Cross-compilation support
//! - Preprocessor macros

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;

/// Target triple for Stenzel OS
pub const TARGET_TRIPLE: &str = "x86_64-stenzel-elf";

/// Alternative target triples we can compile for
pub const SUPPORTED_TARGETS: &[&str] = &[
    "x86_64-stenzel-elf",
    "x86_64-unknown-linux-gnu",
    "x86_64-unknown-linux-musl",
    "x86_64-pc-linux-gnu",
    "x86_64-linux-gnu",
];

/// Compiler type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompilerType {
    Gcc,
    Clang,
    Rustc,
    Nasm,
    As,
    Ld,
    Ar,
}

impl CompilerType {
    pub fn as_str(&self) -> &'static str {
        match self {
            CompilerType::Gcc => "gcc",
            CompilerType::Clang => "clang",
            CompilerType::Rustc => "rustc",
            CompilerType::Nasm => "nasm",
            CompilerType::As => "as",
            CompilerType::Ld => "ld",
            CompilerType::Ar => "ar",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "gcc" | "g++" | "cc" => Some(CompilerType::Gcc),
            "clang" | "clang++" => Some(CompilerType::Clang),
            "rustc" => Some(CompilerType::Rustc),
            "nasm" => Some(CompilerType::Nasm),
            "as" | "gas" => Some(CompilerType::As),
            "ld" | "ld.bfd" | "ld.gold" | "ld.lld" => Some(CompilerType::Ld),
            "ar" => Some(CompilerType::Ar),
            _ => None,
        }
    }
}

/// Compiler toolchain configuration
#[derive(Debug, Clone)]
pub struct ToolchainConfig {
    /// Compiler type
    pub compiler_type: CompilerType,
    /// Version string
    pub version: String,
    /// Target triple
    pub target: String,
    /// System include paths
    pub include_paths: Vec<String>,
    /// System library paths
    pub library_paths: Vec<String>,
    /// Default preprocessor defines
    pub defines: Vec<(String, Option<String>)>,
    /// Compiler binary path
    pub binary_path: String,
}

impl ToolchainConfig {
    /// Create a new GCC configuration
    pub fn gcc() -> Self {
        ToolchainConfig {
            compiler_type: CompilerType::Gcc,
            version: String::from("13.2.0"),
            target: String::from(TARGET_TRIPLE),
            include_paths: default_include_paths(),
            library_paths: default_library_paths(),
            defines: default_defines(),
            binary_path: String::from("/usr/bin/gcc"),
        }
    }

    /// Create a new Clang configuration
    pub fn clang() -> Self {
        ToolchainConfig {
            compiler_type: CompilerType::Clang,
            version: String::from("17.0.0"),
            target: String::from(TARGET_TRIPLE),
            include_paths: default_include_paths(),
            library_paths: default_library_paths(),
            defines: default_defines(),
            binary_path: String::from("/usr/bin/clang"),
        }
    }

    /// Create a new Rust compiler configuration
    pub fn rustc() -> Self {
        ToolchainConfig {
            compiler_type: CompilerType::Rustc,
            version: String::from("1.75.0"),
            target: String::from("x86_64-unknown-linux-gnu"),
            include_paths: Vec::new(),
            library_paths: alloc::vec![
                String::from("/usr/lib/rustlib/x86_64-unknown-linux-gnu/lib"),
            ],
            defines: Vec::new(),
            binary_path: String::from("/usr/bin/rustc"),
        }
    }

    /// Get compiler invocation arguments
    pub fn get_base_args(&self) -> Vec<String> {
        let mut args = Vec::new();

        // Target
        if self.compiler_type == CompilerType::Clang {
            args.push(format!("--target={}", self.target));
        }

        // Include paths
        for path in &self.include_paths {
            args.push(format!("-I{}", path));
        }

        // Library paths
        for path in &self.library_paths {
            args.push(format!("-L{}", path));
        }

        // Defines
        for (name, value) in &self.defines {
            if let Some(val) = value {
                args.push(format!("-D{}={}", name, val));
            } else {
                args.push(format!("-D{}", name));
            }
        }

        args
    }
}

/// Default system include paths
pub fn default_include_paths() -> Vec<String> {
    alloc::vec![
        String::from("/usr/include"),
        String::from("/usr/local/include"),
        String::from("/usr/include/x86_64-linux-gnu"),
        String::from("/usr/lib/gcc/x86_64-linux-gnu/13/include"),
    ]
}

/// Default system library paths
pub fn default_library_paths() -> Vec<String> {
    alloc::vec![
        String::from("/lib"),
        String::from("/lib64"),
        String::from("/usr/lib"),
        String::from("/usr/lib64"),
        String::from("/usr/local/lib"),
        String::from("/usr/lib/x86_64-linux-gnu"),
    ]
}

/// Default preprocessor defines for Stenzel OS
pub fn default_defines() -> Vec<(String, Option<String>)> {
    alloc::vec![
        // Standard defines
        (String::from("__linux__"), None),
        (String::from("__unix__"), None),
        (String::from("__STENZEL_OS__"), Some(String::from("1"))),

        // Architecture
        (String::from("__x86_64__"), None),
        (String::from("__x86_64"), None),
        (String::from("__amd64__"), None),
        (String::from("__amd64"), None),
        (String::from("__LP64__"), None),
        (String::from("_LP64"), None),

        // ABI
        (String::from("__ELF__"), None),

        // POSIX/XSI
        (String::from("_POSIX_VERSION"), Some(String::from("200809L"))),
        (String::from("_POSIX_C_SOURCE"), Some(String::from("200809L"))),
        (String::from("_XOPEN_SOURCE"), Some(String::from("700"))),

        // GNU extensions (for glibc compatibility)
        (String::from("_GNU_SOURCE"), None),

        // Size types
        (String::from("__SIZEOF_POINTER__"), Some(String::from("8"))),
        (String::from("__SIZEOF_LONG__"), Some(String::from("8"))),
        (String::from("__SIZEOF_INT__"), Some(String::from("4"))),
        (String::from("__SIZEOF_SHORT__"), Some(String::from("2"))),
        (String::from("__SIZEOF_FLOAT__"), Some(String::from("4"))),
        (String::from("__SIZEOF_DOUBLE__"), Some(String::from("8"))),

        // Byte order
        (String::from("__BYTE_ORDER__"), Some(String::from("__ORDER_LITTLE_ENDIAN__"))),
        (String::from("__LITTLE_ENDIAN__"), None),
    ]
}

/// Linker configuration
#[derive(Debug, Clone)]
pub struct LinkerConfig {
    /// Linker type (ld.bfd, ld.gold, ld.lld)
    pub linker_type: String,
    /// Search paths for libraries
    pub library_paths: Vec<String>,
    /// Default libraries to link
    pub default_libs: Vec<String>,
    /// Entry point symbol
    pub entry_point: String,
    /// Dynamic linker path
    pub dynamic_linker: String,
    /// Output format
    pub output_format: String,
}

impl LinkerConfig {
    /// Create default linker configuration
    pub fn default_ld() -> Self {
        LinkerConfig {
            linker_type: String::from("ld.bfd"),
            library_paths: default_library_paths(),
            default_libs: alloc::vec![
                String::from("c"),
                String::from("m"),
                String::from("pthread"),
            ],
            entry_point: String::from("_start"),
            dynamic_linker: String::from("/lib64/ld-linux-x86-64.so.2"),
            output_format: String::from("elf64-x86-64"),
        }
    }

    /// Create static linker configuration
    pub fn static_ld() -> Self {
        LinkerConfig {
            linker_type: String::from("ld.bfd"),
            library_paths: default_library_paths(),
            default_libs: alloc::vec![
                String::from("c"),
                String::from("m"),
            ],
            entry_point: String::from("_start"),
            dynamic_linker: String::new(), // No dynamic linker for static
            output_format: String::from("elf64-x86-64"),
        }
    }

    /// Get linker invocation arguments
    pub fn get_base_args(&self) -> Vec<String> {
        let mut args = Vec::new();

        // Output format
        args.push(String::from("-m"));
        args.push(String::from("elf_x86_64"));

        // Entry point
        args.push(String::from("-e"));
        args.push(self.entry_point.clone());

        // Dynamic linker (if dynamic linking)
        if !self.dynamic_linker.is_empty() {
            args.push(String::from("--dynamic-linker"));
            args.push(self.dynamic_linker.clone());
        }

        // Library paths
        for path in &self.library_paths {
            args.push(format!("-L{}", path));
        }

        // Default libraries
        for lib in &self.default_libs {
            args.push(format!("-l{}", lib));
        }

        args
    }
}

/// Assembler configuration
#[derive(Debug, Clone)]
pub struct AssemblerConfig {
    /// Assembler type (gas, nasm)
    pub assembler_type: String,
    /// Target architecture
    pub arch: String,
    /// Output format
    pub output_format: String,
    /// Include paths
    pub include_paths: Vec<String>,
}

impl AssemblerConfig {
    /// Create GNU assembler configuration
    pub fn gas() -> Self {
        AssemblerConfig {
            assembler_type: String::from("gas"),
            arch: String::from("x86_64"),
            output_format: String::from("elf64"),
            include_paths: alloc::vec![
                String::from("/usr/include"),
            ],
        }
    }

    /// Create NASM configuration
    pub fn nasm() -> Self {
        AssemblerConfig {
            assembler_type: String::from("nasm"),
            arch: String::from("x86_64"),
            output_format: String::from("elf64"),
            include_paths: alloc::vec![
                String::from("/usr/include"),
            ],
        }
    }
}

/// Compiler specs for target
#[derive(Debug, Clone)]
pub struct TargetSpecs {
    /// Target triple
    pub triple: String,
    /// Data model (LP64, ILP32, etc.)
    pub data_model: String,
    /// Pointer size in bits
    pub pointer_width: u32,
    /// Endianness
    pub endian: String,
    /// ABI
    pub abi: String,
    /// CPU features
    pub cpu_features: Vec<String>,
    /// Relocation model
    pub relocation_model: String,
    /// Code model
    pub code_model: String,
}

impl TargetSpecs {
    /// Create x86_64 target specs
    pub fn x86_64_linux() -> Self {
        TargetSpecs {
            triple: String::from("x86_64-unknown-linux-gnu"),
            data_model: String::from("LP64"),
            pointer_width: 64,
            endian: String::from("little"),
            abi: String::from("sysv"),
            cpu_features: alloc::vec![
                String::from("+sse"),
                String::from("+sse2"),
            ],
            relocation_model: String::from("pic"),
            code_model: String::from("small"),
        }
    }

    /// Create Stenzel OS target specs
    pub fn stenzel_os() -> Self {
        TargetSpecs {
            triple: String::from(TARGET_TRIPLE),
            data_model: String::from("LP64"),
            pointer_width: 64,
            endian: String::from("little"),
            abi: String::from("sysv"),
            cpu_features: alloc::vec![
                String::from("+sse"),
                String::from("+sse2"),
                String::from("+sse3"),
                String::from("+ssse3"),
                String::from("+sse4.1"),
                String::from("+sse4.2"),
            ],
            relocation_model: String::from("pic"),
            code_model: String::from("small"),
        }
    }
}

/// C Runtime (CRT) files
#[derive(Debug, Clone)]
pub struct CrtFiles {
    /// crt1.o - contains _start
    pub crt1: String,
    /// crti.o - init section prologue
    pub crti: String,
    /// crtn.o - init section epilogue
    pub crtn: String,
    /// crtbegin.o - C++ constructor start
    pub crtbegin: String,
    /// crtend.o - C++ constructor end
    pub crtend: String,
    /// Scrt1.o - position-independent _start
    pub scrt1: String,
}

impl CrtFiles {
    /// Default CRT file locations
    pub fn default_paths() -> Self {
        CrtFiles {
            crt1: String::from("/usr/lib/x86_64-linux-gnu/crt1.o"),
            crti: String::from("/usr/lib/x86_64-linux-gnu/crti.o"),
            crtn: String::from("/usr/lib/x86_64-linux-gnu/crtn.o"),
            crtbegin: String::from("/usr/lib/gcc/x86_64-linux-gnu/13/crtbegin.o"),
            crtend: String::from("/usr/lib/gcc/x86_64-linux-gnu/13/crtend.o"),
            scrt1: String::from("/usr/lib/x86_64-linux-gnu/Scrt1.o"),
        }
    }

    /// Get CRT files for dynamic executable
    pub fn for_dynamic(&self) -> Vec<String> {
        alloc::vec![
            self.scrt1.clone(),
            self.crti.clone(),
            self.crtbegin.clone(),
        ]
    }

    /// Get CRT files for static executable
    pub fn for_static(&self) -> Vec<String> {
        alloc::vec![
            self.crt1.clone(),
            self.crti.clone(),
            self.crtbegin.clone(),
        ]
    }

    /// Get CRT end files
    pub fn end_files(&self) -> Vec<String> {
        alloc::vec![
            self.crtend.clone(),
            self.crtn.clone(),
        ]
    }
}

/// Language standard versions
pub mod standards {
    /// C standards
    pub mod c {
        pub const C89: &str = "c89";
        pub const C90: &str = "c90";
        pub const C99: &str = "c99";
        pub const C11: &str = "c11";
        pub const C17: &str = "c17";
        pub const C23: &str = "c23";
        pub const GNU89: &str = "gnu89";
        pub const GNU99: &str = "gnu99";
        pub const GNU11: &str = "gnu11";
        pub const GNU17: &str = "gnu17";
        pub const GNU23: &str = "gnu23";
    }

    /// C++ standards
    pub mod cpp {
        pub const CPP98: &str = "c++98";
        pub const CPP03: &str = "c++03";
        pub const CPP11: &str = "c++11";
        pub const CPP14: &str = "c++14";
        pub const CPP17: &str = "c++17";
        pub const CPP20: &str = "c++20";
        pub const CPP23: &str = "c++23";
        pub const GNUPP11: &str = "gnu++11";
        pub const GNUPP14: &str = "gnu++14";
        pub const GNUPP17: &str = "gnu++17";
        pub const GNUPP20: &str = "gnu++20";
    }
}

/// Optimization levels
pub mod optimization {
    pub const O0: &str = "-O0";
    pub const O1: &str = "-O1";
    pub const O2: &str = "-O2";
    pub const O3: &str = "-O3";
    pub const OS: &str = "-Os";
    pub const OZ: &str = "-Oz";
    pub const OG: &str = "-Og";
    pub const OFAST: &str = "-Ofast";
}

/// Warning flags
pub mod warnings {
    pub const WALL: &str = "-Wall";
    pub const WEXTRA: &str = "-Wextra";
    pub const WERROR: &str = "-Werror";
    pub const WPEDANTIC: &str = "-Wpedantic";
    pub const WNO_UNUSED: &str = "-Wno-unused";
    pub const WNO_SIGN_COMPARE: &str = "-Wno-sign-compare";
}

/// Debug info levels
pub mod debug {
    pub const G0: &str = "-g0";
    pub const G1: &str = "-g1";
    pub const G2: &str = "-g2";
    pub const G3: &str = "-g3";
    pub const GDWARF4: &str = "-gdwarf-4";
    pub const GDWARF5: &str = "-gdwarf-5";
}

/// Compiler environment variables
pub struct CompilerEnv;

impl CompilerEnv {
    /// Get CC (C compiler)
    pub fn cc() -> &'static str {
        "gcc"
    }

    /// Get CXX (C++ compiler)
    pub fn cxx() -> &'static str {
        "g++"
    }

    /// Get AR (archiver)
    pub fn ar() -> &'static str {
        "ar"
    }

    /// Get LD (linker)
    pub fn ld() -> &'static str {
        "ld"
    }

    /// Get AS (assembler)
    pub fn as_() -> &'static str {
        "as"
    }

    /// Get NM (symbol lister)
    pub fn nm() -> &'static str {
        "nm"
    }

    /// Get OBJCOPY
    pub fn objcopy() -> &'static str {
        "objcopy"
    }

    /// Get OBJDUMP
    pub fn objdump() -> &'static str {
        "objdump"
    }

    /// Get STRIP
    pub fn strip() -> &'static str {
        "strip"
    }

    /// Get RANLIB
    pub fn ranlib() -> &'static str {
        "ranlib"
    }
}

/// Detect installed compilers
pub fn detect_compilers() -> Vec<ToolchainConfig> {
    let mut compilers = Vec::new();

    // Check for GCC
    // In a real implementation, we'd check if /usr/bin/gcc exists
    compilers.push(ToolchainConfig::gcc());

    // Check for Clang
    compilers.push(ToolchainConfig::clang());

    // Check for Rust
    compilers.push(ToolchainConfig::rustc());

    compilers
}

/// Get compiler by name
pub fn get_compiler(name: &str) -> Option<ToolchainConfig> {
    match CompilerType::from_name(name) {
        Some(CompilerType::Gcc) => Some(ToolchainConfig::gcc()),
        Some(CompilerType::Clang) => Some(ToolchainConfig::clang()),
        Some(CompilerType::Rustc) => Some(ToolchainConfig::rustc()),
        _ => None,
    }
}

/// Create a complete compilation command
pub fn create_compile_command(
    compiler: &ToolchainConfig,
    source_files: &[&str],
    output: &str,
    extra_args: &[&str],
) -> Vec<String> {
    let mut cmd = Vec::new();

    // Compiler binary
    cmd.push(compiler.binary_path.clone());

    // Base arguments
    cmd.extend(compiler.get_base_args());

    // Extra arguments
    for arg in extra_args {
        cmd.push(String::from(*arg));
    }

    // Source files
    for src in source_files {
        cmd.push(String::from(*src));
    }

    // Output
    cmd.push(String::from("-o"));
    cmd.push(String::from(output));

    cmd
}

/// Create a link command
pub fn create_link_command(
    linker: &LinkerConfig,
    object_files: &[&str],
    output: &str,
    extra_args: &[&str],
) -> Vec<String> {
    let mut cmd = Vec::new();

    // Linker binary
    cmd.push(String::from("/usr/bin/ld"));

    // CRT files (start)
    let crt = CrtFiles::default_paths();
    for f in crt.for_dynamic() {
        cmd.push(f);
    }

    // Base arguments
    cmd.extend(linker.get_base_args());

    // Extra arguments
    for arg in extra_args {
        cmd.push(String::from(*arg));
    }

    // Object files
    for obj in object_files {
        cmd.push(String::from(*obj));
    }

    // CRT files (end)
    for f in crt.end_files() {
        cmd.push(f);
    }

    // Output
    cmd.push(String::from("-o"));
    cmd.push(String::from(output));

    cmd
}

/// Initialize compilers subsystem
pub fn init() {
    crate::kprintln!("compilers: initializing native compiler support");
    crate::kprintln!("compilers: target triple: {}", TARGET_TRIPLE);
    crate::kprintln!("compilers: supported: GCC 13.2, Clang 17.0, rustc 1.75");
    crate::kprintln!("compilers: native compiler support ready");
}
