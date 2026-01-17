//! Python Runtime Support
//!
//! Provides infrastructure for running Python interpreters and scripts
//! on Stenzel OS.
//!
//! Supports:
//! - Python 3.x interpreters
//! - Standard library paths
//! - C extension loading
//! - Virtual environments (venv)
//! - pip package manager

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use alloc::collections::BTreeMap;
use spin::Mutex;

/// Supported Python versions
pub const PYTHON_VERSIONS: &[&str] = &[
    "3.12",
    "3.11",
    "3.10",
    "3.9",
    "3.8",
];

/// Default Python version
pub const DEFAULT_PYTHON_VERSION: &str = "3.12";

/// Python interpreter path format
pub const PYTHON_BIN_FORMAT: &str = "/usr/bin/python{}";

/// Python configuration
#[derive(Debug, Clone)]
pub struct PythonConfig {
    /// Python version (e.g., "3.12")
    pub version: String,
    /// Major.minor version for paths
    pub version_short: String,
    /// Interpreter binary path
    pub executable: String,
    /// Site-packages directory
    pub site_packages: String,
    /// Standard library path
    pub stdlib_path: String,
    /// Include directory for C extensions
    pub include_path: String,
    /// Library path for libpython
    pub lib_path: String,
    /// Platform tag (e.g., linux-x86_64)
    pub platform: String,
    /// Is this a virtual environment?
    pub is_venv: bool,
    /// Virtual environment base path
    pub venv_path: Option<String>,
}

impl PythonConfig {
    /// Create configuration for a Python version
    pub fn for_version(version: &str) -> Self {
        let version_short = version.to_string();
        let major_minor = if version.len() >= 4 {
            &version[0..4]
        } else {
            version
        };

        PythonConfig {
            version: version.to_string(),
            version_short: version_short.clone(),
            executable: format!("/usr/bin/python{}", major_minor),
            site_packages: format!("/usr/lib/python{}/site-packages", major_minor),
            stdlib_path: format!("/usr/lib/python{}", major_minor),
            include_path: format!("/usr/include/python{}", major_minor),
            lib_path: format!("/usr/lib/libpython{}.so", major_minor),
            platform: String::from("linux-x86_64"),
            is_venv: false,
            venv_path: None,
        }
    }

    /// Create default Python 3.12 configuration
    pub fn default() -> Self {
        Self::for_version(DEFAULT_PYTHON_VERSION)
    }

    /// Create configuration for a virtual environment
    pub fn for_venv(venv_path: &str, base_version: &str) -> Self {
        let mut config = Self::for_version(base_version);
        config.is_venv = true;
        config.venv_path = Some(venv_path.to_string());
        config.executable = format!("{}/bin/python", venv_path);
        config.site_packages = format!("{}/lib/python{}/site-packages",
                                       venv_path, &base_version[0..4]);
        config
    }

    /// Get sys.path entries
    pub fn get_sys_path(&self) -> Vec<String> {
        let mut paths = Vec::new();

        // Current directory (for scripts)
        paths.push(String::from("."));

        // Virtual environment site-packages (if applicable)
        if self.is_venv {
            paths.push(self.site_packages.clone());
        }

        // System site-packages
        if !self.is_venv {
            paths.push(self.site_packages.clone());
        }

        // Standard library
        paths.push(self.stdlib_path.clone());
        paths.push(format!("{}/lib-dynload", self.stdlib_path));

        // User site-packages
        paths.push(format!("{}/.local/lib/python{}/site-packages",
                           get_home_dir(), &self.version[0..4]));

        paths
    }

    /// Get environment variables for Python
    pub fn get_env_vars(&self) -> Vec<(String, String)> {
        let mut vars = Vec::new();

        vars.push((String::from("PYTHONHOME"), self.stdlib_path.clone()));
        vars.push((String::from("PYTHONPATH"), self.get_sys_path().join(":")));

        if self.is_venv {
            if let Some(ref venv) = self.venv_path {
                vars.push((String::from("VIRTUAL_ENV"), venv.clone()));
            }
        }

        // Encoding
        vars.push((String::from("PYTHONIOENCODING"), String::from("utf-8")));
        vars.push((String::from("PYTHONUTF8"), String::from("1")));

        // Don't write bytecode by default
        vars.push((String::from("PYTHONDONTWRITEBYTECODE"), String::from("1")));

        vars
    }
}

/// Get home directory
fn get_home_dir() -> String {
    String::from("/root")
}

/// Python module types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleType {
    /// Pure Python source (.py)
    Source,
    /// Compiled bytecode (.pyc)
    Bytecode,
    /// C extension module (.so)
    Extension,
    /// Package (__init__.py)
    Package,
    /// Namespace package (PEP 420)
    NamespacePackage,
    /// Built-in module
    Builtin,
    /// Frozen module (compiled into interpreter)
    Frozen,
}

/// Python module information
#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub name: String,
    pub module_type: ModuleType,
    pub path: Option<String>,
    pub package: Option<String>,
}

/// Built-in modules that are always available
pub const BUILTIN_MODULES: &[&str] = &[
    "_abc",
    "_ast",
    "_bisect",
    "_blake2",
    "_codecs",
    "_collections",
    "_datetime",
    "_functools",
    "_heapq",
    "_io",
    "_json",
    "_locale",
    "_md5",
    "_operator",
    "_pickle",
    "_posixsubprocess",
    "_random",
    "_sha1",
    "_sha256",
    "_sha512",
    "_signal",
    "_socket",
    "_sre",
    "_stat",
    "_string",
    "_struct",
    "_symtable",
    "_thread",
    "_tracemalloc",
    "_typing",
    "_warnings",
    "_weakref",
    "array",
    "atexit",
    "binascii",
    "builtins",
    "cmath",
    "errno",
    "faulthandler",
    "gc",
    "itertools",
    "marshal",
    "math",
    "posix",
    "pwd",
    "select",
    "sys",
    "time",
    "unicodedata",
    "zlib",
];

/// Standard library modules (subset)
pub const STDLIB_MODULES: &[&str] = &[
    "abc",
    "argparse",
    "ast",
    "asyncio",
    "base64",
    "bisect",
    "calendar",
    "collections",
    "contextlib",
    "copy",
    "csv",
    "dataclasses",
    "datetime",
    "decimal",
    "difflib",
    "email",
    "enum",
    "fileinput",
    "fnmatch",
    "fractions",
    "functools",
    "getopt",
    "glob",
    "gzip",
    "hashlib",
    "heapq",
    "hmac",
    "html",
    "http",
    "importlib",
    "inspect",
    "io",
    "ipaddress",
    "itertools",
    "json",
    "keyword",
    "linecache",
    "locale",
    "logging",
    "lzma",
    "math",
    "mimetypes",
    "multiprocessing",
    "numbers",
    "operator",
    "os",
    "pathlib",
    "pickle",
    "platform",
    "pprint",
    "queue",
    "random",
    "re",
    "secrets",
    "select",
    "shlex",
    "shutil",
    "signal",
    "socket",
    "sqlite3",
    "ssl",
    "stat",
    "statistics",
    "string",
    "struct",
    "subprocess",
    "sys",
    "tarfile",
    "tempfile",
    "textwrap",
    "threading",
    "time",
    "timeit",
    "traceback",
    "types",
    "typing",
    "unittest",
    "urllib",
    "uuid",
    "venv",
    "warnings",
    "weakref",
    "xml",
    "zipfile",
];

/// Package manager (pip) support
#[derive(Debug, Clone)]
pub struct PipConfig {
    /// pip executable path
    pub executable: String,
    /// Package index URL
    pub index_url: String,
    /// Extra index URLs
    pub extra_index_urls: Vec<String>,
    /// Trusted hosts
    pub trusted_hosts: Vec<String>,
    /// Cache directory
    pub cache_dir: String,
    /// Disable cache
    pub no_cache: bool,
}

impl PipConfig {
    pub fn default() -> Self {
        PipConfig {
            executable: String::from("/usr/bin/pip3"),
            index_url: String::from("https://pypi.org/simple/"),
            extra_index_urls: Vec::new(),
            trusted_hosts: Vec::new(),
            cache_dir: String::from("/root/.cache/pip"),
            no_cache: false,
        }
    }

    /// Get pip install command
    pub fn install_cmd(&self, packages: &[&str]) -> Vec<String> {
        let mut cmd = vec![
            self.executable.clone(),
            String::from("install"),
        ];

        cmd.push(format!("--index-url={}", self.index_url));

        for url in &self.extra_index_urls {
            cmd.push(format!("--extra-index-url={}", url));
        }

        for host in &self.trusted_hosts {
            cmd.push(format!("--trusted-host={}", host));
        }

        if self.no_cache {
            cmd.push(String::from("--no-cache-dir"));
        } else {
            cmd.push(format!("--cache-dir={}", self.cache_dir));
        }

        for pkg in packages {
            cmd.push(pkg.to_string());
        }

        cmd
    }
}

/// Virtual environment manager
pub struct VenvManager {
    /// Registered virtual environments
    venvs: BTreeMap<String, PythonConfig>,
}

impl VenvManager {
    pub const fn new() -> Self {
        VenvManager {
            venvs: BTreeMap::new(),
        }
    }

    /// Create a new virtual environment
    pub fn create_venv(&mut self, path: &str, python_version: &str) -> Result<PythonConfig, &'static str> {
        // In a real implementation, this would:
        // 1. Create the directory structure
        // 2. Copy/symlink the Python interpreter
        // 3. Create bin/activate scripts
        // 4. Set up site-packages

        let config = PythonConfig::for_venv(path, python_version);
        self.venvs.insert(path.to_string(), config.clone());

        Ok(config)
    }

    /// Get a registered virtual environment
    pub fn get_venv(&self, path: &str) -> Option<&PythonConfig> {
        self.venvs.get(path)
    }

    /// List all virtual environments
    pub fn list_venvs(&self) -> Vec<&String> {
        self.venvs.keys().collect()
    }

    /// Remove a virtual environment
    pub fn remove_venv(&mut self, path: &str) -> bool {
        self.venvs.remove(path).is_some()
    }
}

/// Global venv manager
pub static VENV_MANAGER: Mutex<VenvManager> = Mutex::new(VenvManager::new());

/// Python bytecode header (PEP 552)
#[repr(C)]
pub struct PycHeader {
    /// Magic number (identifies Python version)
    pub magic: u32,
    /// Bit field (PEP 552)
    pub bit_field: u32,
    /// Source modification time (or hash)
    pub mtime_or_hash: u32,
    /// Source file size
    pub source_size: u32,
}

/// Python magic numbers by version
pub mod magic {
    pub const PYTHON_3_8: u32 = 3413;
    pub const PYTHON_3_9: u32 = 3425;
    pub const PYTHON_3_10: u32 = 3439;
    pub const PYTHON_3_11: u32 = 3495;
    pub const PYTHON_3_12: u32 = 3531;
}

/// Parse .pyc file header
pub fn parse_pyc_header(data: &[u8]) -> Option<PycHeader> {
    if data.len() < 16 {
        return None;
    }

    let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let bit_field = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let mtime_or_hash = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
    let source_size = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);

    Some(PycHeader {
        magic,
        bit_field,
        mtime_or_hash,
        source_size,
    })
}

/// Get Python version from magic number
pub fn python_version_from_magic(magic: u32) -> Option<&'static str> {
    // Magic number format: lower 16 bits are the actual magic
    let magic_num = magic & 0xFFFF;

    match magic_num {
        3413 => Some("3.8"),
        3425 => Some("3.9"),
        3439 => Some("3.10"),
        3495 => Some("3.11"),
        3531 => Some("3.12"),
        _ => None,
    }
}

/// C extension ABI tag
pub fn get_abi_tag(python_version: &str) -> String {
    // Example: cpython-312-x86_64-linux-gnu
    let version_nodot: String = python_version.chars().filter(|c| *c != '.').collect();
    format!("cpython-{}-x86_64-linux-gnu", version_nodot)
}

/// Platform tag for wheel files
pub fn get_platform_tag() -> &'static str {
    "manylinux_2_28_x86_64"
}

/// Check if a module is a C extension
pub fn is_c_extension(path: &str) -> bool {
    path.ends_with(".so") ||
    path.contains(".cpython-") && path.ends_with(".so")
}

/// Get expected C extension filename
pub fn c_extension_filename(module_name: &str, python_version: &str) -> String {
    let abi = get_abi_tag(python_version);
    format!("{}.{}.so", module_name, abi)
}

/// Python exception types (for error handling)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyException {
    BaseException,
    SystemExit,
    KeyboardInterrupt,
    GeneratorExit,
    Exception,
    StopIteration,
    StopAsyncIteration,
    ArithmeticError,
    FloatingPointError,
    OverflowError,
    ZeroDivisionError,
    AssertionError,
    AttributeError,
    BufferError,
    EOFError,
    ImportError,
    ModuleNotFoundError,
    LookupError,
    IndexError,
    KeyError,
    MemoryError,
    NameError,
    UnboundLocalError,
    OSError,
    FileExistsError,
    FileNotFoundError,
    IsADirectoryError,
    NotADirectoryError,
    PermissionError,
    ProcessLookupError,
    TimeoutError,
    ReferenceError,
    RuntimeError,
    NotImplementedError,
    RecursionError,
    SyntaxError,
    IndentationError,
    TabError,
    SystemError,
    TypeError,
    ValueError,
    UnicodeError,
    UnicodeDecodeError,
    UnicodeEncodeError,
    UnicodeTranslateError,
    Warning,
    DeprecationWarning,
    PendingDeprecationWarning,
    RuntimeWarning,
    SyntaxWarning,
    UserWarning,
    FutureWarning,
    ImportWarning,
    UnicodeWarning,
    BytesWarning,
    EncodingWarning,
    ResourceWarning,
}

/// Installed Python interpreter registry
pub static PYTHON_INTERPRETERS: Mutex<Vec<PythonConfig>> = Mutex::new(Vec::new());

/// Register a Python interpreter
pub fn register_interpreter(config: PythonConfig) {
    let mut interpreters = PYTHON_INTERPRETERS.lock();
    interpreters.push(config);
}

/// Get all registered interpreters
pub fn get_interpreters() -> Vec<PythonConfig> {
    let interpreters = PYTHON_INTERPRETERS.lock();
    interpreters.clone()
}

/// Find interpreter by version
pub fn find_interpreter(version: &str) -> Option<PythonConfig> {
    let interpreters = PYTHON_INTERPRETERS.lock();
    interpreters.iter()
        .find(|c| c.version.starts_with(version))
        .cloned()
}

/// Get default interpreter
pub fn get_default_interpreter() -> PythonConfig {
    let interpreters = PYTHON_INTERPRETERS.lock();
    interpreters.first().cloned().unwrap_or_else(PythonConfig::default)
}

/// Initialize Python runtime support
pub fn init() {
    crate::kprintln!("python: initializing Python runtime support");

    // Register default interpreters
    for version in PYTHON_VERSIONS {
        register_interpreter(PythonConfig::for_version(version));
    }

    crate::kprintln!("python: registered {} Python versions", PYTHON_VERSIONS.len());
    crate::kprintln!("python: default version: {}", DEFAULT_PYTHON_VERSION);
    crate::kprintln!("python: {} builtin modules available", BUILTIN_MODULES.len());
    crate::kprintln!("python: Python runtime support ready");
}

/// Check if Python scripts can run
pub fn is_python_available() -> bool {
    // In a real implementation, check if python binary exists
    true
}

/// Get Python script invocation
pub fn get_python_command(script_path: &str, args: &[&str]) -> Vec<String> {
    let config = get_default_interpreter();
    let mut cmd = vec![config.executable];
    cmd.push(script_path.to_string());
    for arg in args {
        cmd.push(arg.to_string());
    }
    cmd
}

/// Parse shebang line for Python scripts
pub fn parse_shebang(line: &str) -> Option<PythonConfig> {
    if !line.starts_with("#!") {
        return None;
    }

    let path = line[2..].trim();

    // Handle #!/usr/bin/env python3
    if path.starts_with("/usr/bin/env ") {
        let cmd = path["/usr/bin/env ".len()..].trim();
        if cmd.starts_with("python") {
            return Some(PythonConfig::default());
        }
        return None;
    }

    // Handle #!/usr/bin/python3.x
    if path.contains("python") {
        // Extract version if present
        if let Some(ver_start) = path.rfind("python") {
            let version_str = &path[ver_start + 6..];
            if !version_str.is_empty() && version_str.chars().next().unwrap().is_ascii_digit() {
                // Has version number
                return Some(PythonConfig::for_version(version_str));
            }
        }
        return Some(PythonConfig::default());
    }

    None
}
