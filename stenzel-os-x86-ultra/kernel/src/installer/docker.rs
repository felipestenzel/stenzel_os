//! Docker/OCI Base Image Builder
//!
//! Generates OCI-compliant container images from Stenzel OS for:
//! - Docker Hub distribution
//! - Kubernetes deployment
//! - CI/CD pipelines
//! - Development environments

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use alloc::collections::BTreeMap;

/// OCI Image format specification version
pub const OCI_IMAGE_SPEC_VERSION: &str = "1.0.2";

/// Docker manifest version
pub const DOCKER_MANIFEST_VERSION: u32 = 2;

/// Default layer media type
pub const LAYER_MEDIA_TYPE_GZIP: &str = "application/vnd.oci.image.layer.v1.tar+gzip";
pub const LAYER_MEDIA_TYPE_ZSTD: &str = "application/vnd.oci.image.layer.v1.tar+zstd";

/// Docker/OCI error types
#[derive(Debug, Clone)]
pub enum DockerError {
    InvalidConfig(String),
    LayerCreationFailed(String),
    ManifestError(String),
    CompressionError(String),
    IoError(String),
    RegistryError(String),
    AuthenticationFailed,
    ImageNotFound(String),
    DigestMismatch(String),
    UnsupportedArchitecture(String),
}

pub type DockerResult<T> = Result<T, DockerError>;

/// Target architecture for container image
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerArch {
    Amd64,
    Arm64,
    Arm32v7,
    I386,
    Ppc64le,
    S390x,
    Riscv64,
}

impl ContainerArch {
    pub fn as_str(&self) -> &'static str {
        match self {
            ContainerArch::Amd64 => "amd64",
            ContainerArch::Arm64 => "arm64",
            ContainerArch::Arm32v7 => "arm",
            ContainerArch::I386 => "386",
            ContainerArch::Ppc64le => "ppc64le",
            ContainerArch::S390x => "s390x",
            ContainerArch::Riscv64 => "riscv64",
        }
    }

    pub fn variant(&self) -> Option<&'static str> {
        match self {
            ContainerArch::Arm32v7 => Some("v7"),
            ContainerArch::Arm64 => Some("v8"),
            _ => None,
        }
    }
}

/// Compression algorithm for layers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerCompression {
    None,
    Gzip,
    Zstd,
    Lz4,
}

impl LayerCompression {
    pub fn media_type(&self) -> &'static str {
        match self {
            LayerCompression::None => "application/vnd.oci.image.layer.v1.tar",
            LayerCompression::Gzip => LAYER_MEDIA_TYPE_GZIP,
            LayerCompression::Zstd => LAYER_MEDIA_TYPE_ZSTD,
            LayerCompression::Lz4 => "application/vnd.oci.image.layer.v1.tar+lz4",
        }
    }
}

/// Image variant/flavor
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageVariant {
    /// Full OS image with all packages
    Full,
    /// Minimal base image
    Minimal,
    /// Alpine-like small image
    Micro,
    /// Development image with build tools
    Dev,
    /// Runtime only (no compiler)
    Runtime,
}

impl ImageVariant {
    pub fn tag_suffix(&self) -> &'static str {
        match self {
            ImageVariant::Full => "",
            ImageVariant::Minimal => "-minimal",
            ImageVariant::Micro => "-micro",
            ImageVariant::Dev => "-dev",
            ImageVariant::Runtime => "-runtime",
        }
    }
}

/// OCI image configuration
#[derive(Debug, Clone)]
pub struct OciImageConfig {
    pub architecture: ContainerArch,
    pub os: String,
    pub os_version: Option<String>,
    pub created: String,
    pub author: String,
    pub config: ContainerConfig,
    pub rootfs: RootfsConfig,
    pub history: Vec<HistoryEntry>,
}

impl Default for OciImageConfig {
    fn default() -> Self {
        Self {
            architecture: ContainerArch::Amd64,
            os: String::from("linux"),
            os_version: None,
            created: String::from("2026-01-18T00:00:00Z"),
            author: String::from("Stenzel OS"),
            config: ContainerConfig::default(),
            rootfs: RootfsConfig::default(),
            history: Vec::new(),
        }
    }
}

impl OciImageConfig {
    /// Generate JSON representation
    pub fn to_json(&self) -> String {
        let mut json = String::from("{\n");
        json.push_str(&format!("  \"architecture\": \"{}\",\n", self.architecture.as_str()));
        json.push_str(&format!("  \"os\": \"{}\",\n", self.os));
        if let Some(ref ver) = self.os_version {
            json.push_str(&format!("  \"os.version\": \"{}\",\n", ver));
        }
        json.push_str(&format!("  \"created\": \"{}\",\n", self.created));
        json.push_str(&format!("  \"author\": \"{}\",\n", self.author));
        json.push_str("  \"config\": ");
        json.push_str(&self.config.to_json());
        json.push_str(",\n");
        json.push_str("  \"rootfs\": ");
        json.push_str(&self.rootfs.to_json());
        json.push_str(",\n");
        json.push_str("  \"history\": [\n");
        for (i, h) in self.history.iter().enumerate() {
            json.push_str(&format!("    {}", h.to_json()));
            if i < self.history.len() - 1 {
                json.push(',');
            }
            json.push('\n');
        }
        json.push_str("  ]\n");
        json.push_str("}\n");
        json
    }
}

/// Container runtime configuration
#[derive(Debug, Clone)]
pub struct ContainerConfig {
    pub user: String,
    pub exposed_ports: Vec<u16>,
    pub env: Vec<(String, String)>,
    pub entrypoint: Vec<String>,
    pub cmd: Vec<String>,
    pub volumes: Vec<String>,
    pub working_dir: String,
    pub labels: BTreeMap<String, String>,
    pub stop_signal: String,
    pub shell: Vec<String>,
}

impl Default for ContainerConfig {
    fn default() -> Self {
        Self {
            user: String::new(),
            exposed_ports: Vec::new(),
            env: vec![
                (String::from("PATH"), String::from("/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin")),
            ],
            entrypoint: Vec::new(),
            cmd: vec![String::from("/bin/sh")],
            volumes: Vec::new(),
            working_dir: String::from("/"),
            labels: BTreeMap::new(),
            stop_signal: String::from("SIGTERM"),
            shell: vec![String::from("/bin/sh"), String::from("-c")],
        }
    }
}

impl ContainerConfig {
    pub fn to_json(&self) -> String {
        let mut json = String::from("{\n");

        if !self.user.is_empty() {
            json.push_str(&format!("    \"User\": \"{}\",\n", self.user));
        }

        // Exposed ports
        if !self.exposed_ports.is_empty() {
            json.push_str("    \"ExposedPorts\": {\n");
            for (i, port) in self.exposed_ports.iter().enumerate() {
                json.push_str(&format!("      \"{}/tcp\": {{}}", port));
                if i < self.exposed_ports.len() - 1 {
                    json.push(',');
                }
                json.push('\n');
            }
            json.push_str("    },\n");
        }

        // Environment variables
        json.push_str("    \"Env\": [\n");
        for (i, (k, v)) in self.env.iter().enumerate() {
            json.push_str(&format!("      \"{}={}\"", k, v));
            if i < self.env.len() - 1 {
                json.push(',');
            }
            json.push('\n');
        }
        json.push_str("    ],\n");

        // Entrypoint
        if !self.entrypoint.is_empty() {
            json.push_str("    \"Entrypoint\": [\n");
            for (i, e) in self.entrypoint.iter().enumerate() {
                json.push_str(&format!("      \"{}\"", e));
                if i < self.entrypoint.len() - 1 {
                    json.push(',');
                }
                json.push('\n');
            }
            json.push_str("    ],\n");
        }

        // Cmd
        json.push_str("    \"Cmd\": [\n");
        for (i, c) in self.cmd.iter().enumerate() {
            json.push_str(&format!("      \"{}\"", c));
            if i < self.cmd.len() - 1 {
                json.push(',');
            }
            json.push('\n');
        }
        json.push_str("    ],\n");

        // Working directory
        json.push_str(&format!("    \"WorkingDir\": \"{}\",\n", self.working_dir));

        // Labels
        if !self.labels.is_empty() {
            json.push_str("    \"Labels\": {\n");
            let labels_vec: Vec<_> = self.labels.iter().collect();
            for (i, (k, v)) in labels_vec.iter().enumerate() {
                json.push_str(&format!("      \"{}\": \"{}\"", k, v));
                if i < labels_vec.len() - 1 {
                    json.push(',');
                }
                json.push('\n');
            }
            json.push_str("    },\n");
        }

        json.push_str(&format!("    \"StopSignal\": \"{}\"\n", self.stop_signal));
        json.push_str("  }");
        json
    }
}

/// Rootfs configuration
#[derive(Debug, Clone)]
pub struct RootfsConfig {
    pub fs_type: String,
    pub diff_ids: Vec<String>,
}

impl Default for RootfsConfig {
    fn default() -> Self {
        Self {
            fs_type: String::from("layers"),
            diff_ids: Vec::new(),
        }
    }
}

impl RootfsConfig {
    pub fn to_json(&self) -> String {
        let mut json = String::from("{\n");
        json.push_str(&format!("    \"type\": \"{}\",\n", self.fs_type));
        json.push_str("    \"diff_ids\": [\n");
        for (i, id) in self.diff_ids.iter().enumerate() {
            json.push_str(&format!("      \"{}\"", id));
            if i < self.diff_ids.len() - 1 {
                json.push(',');
            }
            json.push('\n');
        }
        json.push_str("    ]\n");
        json.push_str("  }");
        json
    }
}

/// Layer history entry
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub created: String,
    pub created_by: String,
    pub author: Option<String>,
    pub comment: Option<String>,
    pub empty_layer: bool,
}

impl HistoryEntry {
    pub fn new(created_by: &str) -> Self {
        Self {
            created: String::from("2026-01-18T00:00:00Z"),
            created_by: String::from(created_by),
            author: None,
            comment: None,
            empty_layer: false,
        }
    }

    pub fn to_json(&self) -> String {
        let mut json = String::from("{");
        json.push_str(&format!("\"created\": \"{}\", ", self.created));
        json.push_str(&format!("\"created_by\": \"{}\"", self.created_by));
        if let Some(ref author) = self.author {
            json.push_str(&format!(", \"author\": \"{}\"", author));
        }
        if let Some(ref comment) = self.comment {
            json.push_str(&format!(", \"comment\": \"{}\"", comment));
        }
        if self.empty_layer {
            json.push_str(", \"empty_layer\": true");
        }
        json.push('}');
        json
    }
}

/// OCI image manifest
#[derive(Debug, Clone)]
pub struct OciManifest {
    pub schema_version: u32,
    pub media_type: String,
    pub config: ManifestDescriptor,
    pub layers: Vec<ManifestDescriptor>,
    pub annotations: BTreeMap<String, String>,
}

impl OciManifest {
    pub fn new() -> Self {
        Self {
            schema_version: 2,
            media_type: String::from("application/vnd.oci.image.manifest.v1+json"),
            config: ManifestDescriptor::default(),
            layers: Vec::new(),
            annotations: BTreeMap::new(),
        }
    }

    pub fn to_json(&self) -> String {
        let mut json = String::from("{\n");
        json.push_str(&format!("  \"schemaVersion\": {},\n", self.schema_version));
        json.push_str(&format!("  \"mediaType\": \"{}\",\n", self.media_type));
        json.push_str("  \"config\": ");
        json.push_str(&self.config.to_json());
        json.push_str(",\n");
        json.push_str("  \"layers\": [\n");
        for (i, layer) in self.layers.iter().enumerate() {
            json.push_str(&format!("    {}", layer.to_json()));
            if i < self.layers.len() - 1 {
                json.push(',');
            }
            json.push('\n');
        }
        json.push_str("  ]");

        if !self.annotations.is_empty() {
            json.push_str(",\n  \"annotations\": {\n");
            let anns: Vec<_> = self.annotations.iter().collect();
            for (i, (k, v)) in anns.iter().enumerate() {
                json.push_str(&format!("    \"{}\": \"{}\"", k, v));
                if i < anns.len() - 1 {
                    json.push(',');
                }
                json.push('\n');
            }
            json.push_str("  }");
        }

        json.push_str("\n}\n");
        json
    }
}

/// Manifest descriptor (for config and layers)
#[derive(Debug, Clone, Default)]
pub struct ManifestDescriptor {
    pub media_type: String,
    pub digest: String,
    pub size: u64,
    pub urls: Vec<String>,
    pub annotations: BTreeMap<String, String>,
}

impl ManifestDescriptor {
    pub fn new(media_type: &str, digest: &str, size: u64) -> Self {
        Self {
            media_type: String::from(media_type),
            digest: String::from(digest),
            size,
            urls: Vec::new(),
            annotations: BTreeMap::new(),
        }
    }

    pub fn to_json(&self) -> String {
        let mut json = String::from("{");
        json.push_str(&format!("\"mediaType\": \"{}\", ", self.media_type));
        json.push_str(&format!("\"digest\": \"{}\", ", self.digest));
        json.push_str(&format!("\"size\": {}", self.size));
        if !self.urls.is_empty() {
            json.push_str(", \"urls\": [");
            for (i, url) in self.urls.iter().enumerate() {
                json.push_str(&format!("\"{}\"", url));
                if i < self.urls.len() - 1 {
                    json.push_str(", ");
                }
            }
            json.push(']');
        }
        json.push('}');
        json
    }
}

/// Multi-architecture image index
#[derive(Debug, Clone)]
pub struct OciImageIndex {
    pub schema_version: u32,
    pub media_type: String,
    pub manifests: Vec<IndexManifest>,
    pub annotations: BTreeMap<String, String>,
}

impl OciImageIndex {
    pub fn new() -> Self {
        Self {
            schema_version: 2,
            media_type: String::from("application/vnd.oci.image.index.v1+json"),
            manifests: Vec::new(),
            annotations: BTreeMap::new(),
        }
    }

    pub fn to_json(&self) -> String {
        let mut json = String::from("{\n");
        json.push_str(&format!("  \"schemaVersion\": {},\n", self.schema_version));
        json.push_str(&format!("  \"mediaType\": \"{}\",\n", self.media_type));
        json.push_str("  \"manifests\": [\n");
        for (i, m) in self.manifests.iter().enumerate() {
            json.push_str(&format!("    {}", m.to_json()));
            if i < self.manifests.len() - 1 {
                json.push(',');
            }
            json.push('\n');
        }
        json.push_str("  ]\n");
        json.push_str("}\n");
        json
    }
}

/// Index manifest entry for multi-arch images
#[derive(Debug, Clone)]
pub struct IndexManifest {
    pub media_type: String,
    pub digest: String,
    pub size: u64,
    pub platform: Platform,
}

impl IndexManifest {
    pub fn to_json(&self) -> String {
        let mut json = String::from("{");
        json.push_str(&format!("\"mediaType\": \"{}\", ", self.media_type));
        json.push_str(&format!("\"digest\": \"{}\", ", self.digest));
        json.push_str(&format!("\"size\": {}, ", self.size));
        json.push_str(&format!("\"platform\": {}", self.platform.to_json()));
        json.push('}');
        json
    }
}

/// Platform specification
#[derive(Debug, Clone)]
pub struct Platform {
    pub architecture: String,
    pub os: String,
    pub os_version: Option<String>,
    pub variant: Option<String>,
}

impl Platform {
    pub fn new(arch: ContainerArch) -> Self {
        Self {
            architecture: String::from(arch.as_str()),
            os: String::from("linux"),
            os_version: None,
            variant: arch.variant().map(String::from),
        }
    }

    pub fn to_json(&self) -> String {
        let mut json = String::from("{");
        json.push_str(&format!("\"architecture\": \"{}\", ", self.architecture));
        json.push_str(&format!("\"os\": \"{}\"", self.os));
        if let Some(ref ver) = self.os_version {
            json.push_str(&format!(", \"os.version\": \"{}\"", ver));
        }
        if let Some(ref var) = self.variant {
            json.push_str(&format!(", \"variant\": \"{}\"", var));
        }
        json.push('}');
        json
    }
}

/// Layer content descriptor
#[derive(Debug, Clone)]
pub struct LayerContent {
    pub files: Vec<LayerFile>,
    pub directories: Vec<String>,
    pub symlinks: Vec<(String, String)>,
    pub hardlinks: Vec<(String, String)>,
    pub whiteouts: Vec<String>,
}

impl LayerContent {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            directories: Vec::new(),
            symlinks: Vec::new(),
            hardlinks: Vec::new(),
            whiteouts: Vec::new(),
        }
    }

    /// Add a file to the layer
    pub fn add_file(&mut self, path: &str, content: Vec<u8>, mode: u32) {
        self.files.push(LayerFile {
            path: String::from(path),
            content,
            mode,
            uid: 0,
            gid: 0,
        });
    }

    /// Add a directory to the layer
    pub fn add_directory(&mut self, path: &str) {
        self.directories.push(String::from(path));
    }

    /// Add a symlink
    pub fn add_symlink(&mut self, path: &str, target: &str) {
        self.symlinks.push((String::from(path), String::from(target)));
    }

    /// Add a whiteout (file deletion marker)
    pub fn add_whiteout(&mut self, path: &str) {
        self.whiteouts.push(String::from(path));
    }
}

/// File in a layer
#[derive(Debug, Clone)]
pub struct LayerFile {
    pub path: String,
    pub content: Vec<u8>,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
}

/// Dockerfile instruction representation
#[derive(Debug, Clone)]
pub enum DockerInstruction {
    From(String, Option<String>),  // image, tag
    Run(String),
    Copy(String, String),
    Add(String, String),
    Env(String, String),
    Workdir(String),
    Expose(u16),
    Volume(String),
    User(String),
    Cmd(Vec<String>),
    Entrypoint(Vec<String>),
    Label(String, String),
    Arg(String, Option<String>),
    Shell(Vec<String>),
    Healthcheck(HealthcheckConfig),
    Stopsignal(String),
}

/// Healthcheck configuration
#[derive(Debug, Clone)]
pub struct HealthcheckConfig {
    pub cmd: Vec<String>,
    pub interval: u64,     // nanoseconds
    pub timeout: u64,
    pub start_period: u64,
    pub retries: u32,
}

/// Docker image builder
pub struct DockerImageBuilder {
    base_config: OciImageConfig,
    layers: Vec<LayerContent>,
    instructions: Vec<DockerInstruction>,
    variant: ImageVariant,
    compression: LayerCompression,
    target_architectures: Vec<ContainerArch>,
    registry: Option<String>,
    repository: String,
    tag: String,
}

impl DockerImageBuilder {
    pub fn new(repository: &str) -> Self {
        let mut config = OciImageConfig::default();
        let mut labels = BTreeMap::new();
        labels.insert(String::from("org.opencontainers.image.title"), String::from("Stenzel OS"));
        labels.insert(String::from("org.opencontainers.image.vendor"), String::from("Stenzel OS Project"));
        labels.insert(String::from("org.opencontainers.image.version"), String::from("1.0.0"));
        config.config.labels = labels;

        Self {
            base_config: config,
            layers: Vec::new(),
            instructions: Vec::new(),
            variant: ImageVariant::Minimal,
            compression: LayerCompression::Gzip,
            target_architectures: vec![ContainerArch::Amd64],
            registry: None,
            repository: String::from(repository),
            tag: String::from("latest"),
        }
    }

    /// Set the registry (e.g., "docker.io", "ghcr.io")
    pub fn registry(mut self, registry: &str) -> Self {
        self.registry = Some(String::from(registry));
        self
    }

    /// Set the image tag
    pub fn tag(mut self, tag: &str) -> Self {
        self.tag = String::from(tag);
        self
    }

    /// Set image variant
    pub fn variant(mut self, variant: ImageVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Set compression algorithm
    pub fn compression(mut self, compression: LayerCompression) -> Self {
        self.compression = compression;
        self
    }

    /// Add target architecture for multi-arch build
    pub fn add_architecture(mut self, arch: ContainerArch) -> Self {
        if !self.target_architectures.contains(&arch) {
            self.target_architectures.push(arch);
        }
        self
    }

    /// Set entrypoint
    pub fn entrypoint(mut self, entrypoint: Vec<&str>) -> Self {
        self.base_config.config.entrypoint = entrypoint.iter().map(|s| String::from(*s)).collect();
        self.instructions.push(DockerInstruction::Entrypoint(
            entrypoint.iter().map(|s| String::from(*s)).collect()
        ));
        self
    }

    /// Set CMD
    pub fn cmd(mut self, cmd: Vec<&str>) -> Self {
        self.base_config.config.cmd = cmd.iter().map(|s| String::from(*s)).collect();
        self.instructions.push(DockerInstruction::Cmd(
            cmd.iter().map(|s| String::from(*s)).collect()
        ));
        self
    }

    /// Add environment variable
    pub fn env(mut self, key: &str, value: &str) -> Self {
        self.base_config.config.env.push((String::from(key), String::from(value)));
        self.instructions.push(DockerInstruction::Env(
            String::from(key),
            String::from(value)
        ));
        self
    }

    /// Set working directory
    pub fn workdir(mut self, dir: &str) -> Self {
        self.base_config.config.working_dir = String::from(dir);
        self.instructions.push(DockerInstruction::Workdir(String::from(dir)));
        self
    }

    /// Expose a port
    pub fn expose(mut self, port: u16) -> Self {
        self.base_config.config.exposed_ports.push(port);
        self.instructions.push(DockerInstruction::Expose(port));
        self
    }

    /// Add a volume
    pub fn volume(mut self, path: &str) -> Self {
        self.base_config.config.volumes.push(String::from(path));
        self.instructions.push(DockerInstruction::Volume(String::from(path)));
        self
    }

    /// Set the user
    pub fn user(mut self, user: &str) -> Self {
        self.base_config.config.user = String::from(user);
        self.instructions.push(DockerInstruction::User(String::from(user)));
        self
    }

    /// Add a label
    pub fn label(mut self, key: &str, value: &str) -> Self {
        self.base_config.config.labels.insert(String::from(key), String::from(value));
        self.instructions.push(DockerInstruction::Label(
            String::from(key),
            String::from(value)
        ));
        self
    }

    /// Add a RUN instruction (creates a new layer)
    pub fn run(mut self, command: &str) -> Self {
        self.instructions.push(DockerInstruction::Run(String::from(command)));
        self.base_config.history.push(HistoryEntry::new(&format!("RUN {}", command)));
        self
    }

    /// Add layer with files
    pub fn add_layer(&mut self, layer: LayerContent) {
        self.layers.push(layer);
    }

    /// Create base layer with essential OS files
    fn create_base_layer(&self) -> LayerContent {
        let mut layer = LayerContent::new();

        // Essential directories
        let dirs = [
            "/bin", "/sbin", "/usr", "/usr/bin", "/usr/sbin", "/usr/lib", "/usr/lib64",
            "/lib", "/lib64", "/etc", "/var", "/var/log", "/var/run", "/var/tmp",
            "/tmp", "/home", "/root", "/proc", "/sys", "/dev", "/run",
        ];

        for dir in &dirs {
            layer.add_directory(dir);
        }

        // /etc/os-release
        let os_release = b"NAME=\"Stenzel OS\"\n\
            VERSION=\"1.0.0\"\n\
            ID=stenzel\n\
            ID_LIKE=linux\n\
            VERSION_ID=\"1.0.0\"\n\
            PRETTY_NAME=\"Stenzel OS 1.0.0\"\n\
            HOME_URL=\"https://stenzel-os.example.com\"\n";
        layer.add_file("/etc/os-release", os_release.to_vec(), 0o644);

        // /etc/passwd
        let passwd = b"root:x:0:0:root:/root:/bin/sh\n\
            nobody:x:65534:65534:nobody:/nonexistent:/usr/sbin/nologin\n";
        layer.add_file("/etc/passwd", passwd.to_vec(), 0o644);

        // /etc/group
        let group = b"root:x:0:\n\
            nogroup:x:65534:\n";
        layer.add_file("/etc/group", group.to_vec(), 0o644);

        // /etc/shadow (empty but present)
        let shadow = b"root:*:19700:0:99999:7:::\n";
        layer.add_file("/etc/shadow", shadow.to_vec(), 0o640);

        // /etc/resolv.conf placeholder
        layer.add_file("/etc/resolv.conf", b"# DNS resolver\n".to_vec(), 0o644);

        // /etc/hosts
        let hosts = b"127.0.0.1\tlocalhost\n::1\tlocalhost\n";
        layer.add_file("/etc/hosts", hosts.to_vec(), 0o644);

        layer
    }

    /// Create variant-specific layers
    fn create_variant_layers(&self) -> Vec<LayerContent> {
        let mut layers = Vec::new();

        match self.variant {
            ImageVariant::Full => {
                // Full includes everything
                let mut dev_layer = LayerContent::new();
                dev_layer.add_directory("/usr/include");
                dev_layer.add_directory("/usr/share/doc");
                layers.push(dev_layer);
            }
            ImageVariant::Dev => {
                // Dev includes build tools
                let mut dev_layer = LayerContent::new();
                dev_layer.add_directory("/usr/include");
                dev_layer.add_directory("/usr/lib/pkgconfig");
                layers.push(dev_layer);
            }
            ImageVariant::Minimal | ImageVariant::Micro | ImageVariant::Runtime => {
                // Minimal layers already in base
            }
        }

        layers
    }

    /// Build the Docker image
    pub fn build(&mut self) -> DockerResult<OciManifest> {
        // Create base layer
        let base_layer = self.create_base_layer();
        self.layers.insert(0, base_layer);

        // Add variant-specific layers
        let variant_layers = self.create_variant_layers();
        for layer in variant_layers {
            self.layers.push(layer);
        }

        // Create manifest
        let mut manifest = OciManifest::new();

        // Add config descriptor
        let config_json = self.base_config.to_json();
        let config_digest = Self::compute_sha256(&config_json.as_bytes());
        manifest.config = ManifestDescriptor::new(
            "application/vnd.oci.image.config.v1+json",
            &format!("sha256:{}", config_digest),
            config_json.len() as u64,
        );

        // Add layer descriptors
        for (i, layer) in self.layers.iter().enumerate() {
            let layer_data = self.serialize_layer(layer)?;
            let compressed = self.compress_layer(&layer_data)?;
            let digest = Self::compute_sha256(&compressed);

            let desc = ManifestDescriptor::new(
                self.compression.media_type(),
                &format!("sha256:{}", digest),
                compressed.len() as u64,
            );
            manifest.layers.push(desc);

            // Update rootfs diff_ids
            let uncompressed_digest = Self::compute_sha256(&layer_data);
            self.base_config.rootfs.diff_ids.push(
                format!("sha256:{}", uncompressed_digest)
            );

            // Add history
            if i == 0 {
                self.base_config.history.push(HistoryEntry::new("base layer"));
            }
        }

        // Add annotations
        manifest.annotations.insert(
            String::from("org.opencontainers.image.created"),
            String::from("2026-01-18T00:00:00Z"),
        );
        manifest.annotations.insert(
            String::from("org.opencontainers.image.title"),
            String::from("Stenzel OS"),
        );

        Ok(manifest)
    }

    /// Build multi-architecture image index
    pub fn build_multiarch(&mut self) -> DockerResult<OciImageIndex> {
        let mut index = OciImageIndex::new();

        for arch in &self.target_architectures.clone() {
            self.base_config.architecture = *arch;

            let manifest = self.build()?;
            let manifest_json = manifest.to_json();
            let digest = Self::compute_sha256(manifest_json.as_bytes());

            index.manifests.push(IndexManifest {
                media_type: String::from("application/vnd.oci.image.manifest.v1+json"),
                digest: format!("sha256:{}", digest),
                size: manifest_json.len() as u64,
                platform: Platform::new(*arch),
            });
        }

        Ok(index)
    }

    /// Serialize layer to tar format (simplified)
    fn serialize_layer(&self, layer: &LayerContent) -> DockerResult<Vec<u8>> {
        let mut tar_data = Vec::new();

        // Directories
        for dir in &layer.directories {
            Self::write_tar_header(&mut tar_data, dir, 0, 0o755, true);
        }

        // Files
        for file in &layer.files {
            Self::write_tar_header(&mut tar_data, &file.path, file.content.len() as u64, file.mode, false);
            tar_data.extend_from_slice(&file.content);
            // Pad to 512-byte boundary
            let padding = (512 - (file.content.len() % 512)) % 512;
            tar_data.extend(core::iter::repeat(0u8).take(padding));
        }

        // Symlinks
        for (path, target) in &layer.symlinks {
            Self::write_tar_symlink(&mut tar_data, path, target);
        }

        // Whiteouts
        for path in &layer.whiteouts {
            let whiteout_path = format!("{}/.wh.{}",
                path.rsplit('/').skip(1).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("/"),
                path.rsplit('/').next().unwrap_or("")
            );
            Self::write_tar_header(&mut tar_data, &whiteout_path, 0, 0o644, false);
        }

        // End of archive (two zero blocks)
        tar_data.extend(core::iter::repeat(0u8).take(1024));

        Ok(tar_data)
    }

    /// Write tar header (simplified USTAR format)
    fn write_tar_header(tar: &mut Vec<u8>, path: &str, size: u64, mode: u32, is_dir: bool) {
        let mut header = [0u8; 512];

        // Name (0-99)
        let name_bytes = path.as_bytes();
        let name_len = core::cmp::min(name_bytes.len(), 100);
        header[..name_len].copy_from_slice(&name_bytes[..name_len]);

        // Mode (100-107) - octal
        let mode_str = format!("{:07o}\0", mode);
        header[100..108].copy_from_slice(mode_str.as_bytes());

        // UID (108-115)
        header[108..116].copy_from_slice(b"0000000\0");

        // GID (116-123)
        header[116..124].copy_from_slice(b"0000000\0");

        // Size (124-135) - octal
        let size_str = format!("{:011o}\0", size);
        header[124..136].copy_from_slice(size_str.as_bytes());

        // Mtime (136-147)
        header[136..148].copy_from_slice(b"00000000000\0");

        // Checksum placeholder (148-155)
        header[148..156].copy_from_slice(b"        ");

        // Type flag (156)
        header[156] = if is_dir { b'5' } else { b'0' };

        // Magic (257-262)
        header[257..263].copy_from_slice(b"ustar\0");

        // Version (263-264)
        header[263..265].copy_from_slice(b"00");

        // Calculate checksum
        let checksum: u32 = header.iter().map(|&b| b as u32).sum();
        let checksum_str = format!("{:06o}\0 ", checksum);
        header[148..156].copy_from_slice(checksum_str.as_bytes());

        tar.extend_from_slice(&header);
    }

    /// Write tar symlink header
    fn write_tar_symlink(tar: &mut Vec<u8>, path: &str, target: &str) {
        let mut header = [0u8; 512];

        // Name
        let name_bytes = path.as_bytes();
        let name_len = core::cmp::min(name_bytes.len(), 100);
        header[..name_len].copy_from_slice(&name_bytes[..name_len]);

        // Mode
        header[100..108].copy_from_slice(b"0000777\0");

        // UID, GID
        header[108..116].copy_from_slice(b"0000000\0");
        header[116..124].copy_from_slice(b"0000000\0");

        // Size (0 for symlinks)
        header[124..136].copy_from_slice(b"00000000000\0");

        // Mtime
        header[136..148].copy_from_slice(b"00000000000\0");

        // Checksum placeholder
        header[148..156].copy_from_slice(b"        ");

        // Type flag (2 = symlink)
        header[156] = b'2';

        // Link name (157-256)
        let target_bytes = target.as_bytes();
        let target_len = core::cmp::min(target_bytes.len(), 100);
        header[157..157 + target_len].copy_from_slice(&target_bytes[..target_len]);

        // Magic
        header[257..263].copy_from_slice(b"ustar\0");
        header[263..265].copy_from_slice(b"00");

        // Calculate checksum
        let checksum: u32 = header.iter().map(|&b| b as u32).sum();
        let checksum_str = format!("{:06o}\0 ", checksum);
        header[148..156].copy_from_slice(checksum_str.as_bytes());

        tar.extend_from_slice(&header);
    }

    /// Compress layer data
    fn compress_layer(&self, data: &[u8]) -> DockerResult<Vec<u8>> {
        match self.compression {
            LayerCompression::None => Ok(data.to_vec()),
            LayerCompression::Gzip => {
                // Simplified gzip header + raw data
                // In real implementation, use actual gzip compression
                let mut compressed = Vec::new();
                // Gzip header
                compressed.extend_from_slice(&[0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03]);
                // Store as uncompressed for now (compression level 0)
                compressed.extend_from_slice(data);
                // CRC32 and size (simplified)
                compressed.extend_from_slice(&[0x00; 8]);
                Ok(compressed)
            }
            LayerCompression::Zstd => {
                // Zstd magic + data (simplified)
                let mut compressed = Vec::new();
                compressed.extend_from_slice(&[0x28, 0xb5, 0x2f, 0xfd]);
                compressed.extend_from_slice(data);
                Ok(compressed)
            }
            LayerCompression::Lz4 => {
                // LZ4 frame magic + data (simplified)
                let mut compressed = Vec::new();
                compressed.extend_from_slice(&[0x04, 0x22, 0x4d, 0x18]);
                compressed.extend_from_slice(data);
                Ok(compressed)
            }
        }
    }

    /// Compute SHA256 hash (simplified)
    fn compute_sha256(data: &[u8]) -> String {
        // Simplified hash computation
        // In real implementation, use proper SHA256
        let mut hash = [0u8; 32];
        for (i, chunk) in data.chunks(32).enumerate() {
            for (j, &byte) in chunk.iter().enumerate() {
                hash[j] ^= byte.wrapping_add(i as u8);
            }
        }

        let mut hex = String::new();
        for byte in &hash {
            hex.push_str(&format!("{:02x}", byte));
        }
        hex
    }

    /// Get full image name
    pub fn full_image_name(&self) -> String {
        let tag_suffix = self.variant.tag_suffix();
        let tag = format!("{}{}", self.tag, tag_suffix);

        if let Some(ref registry) = self.registry {
            format!("{}/{}:{}", registry, self.repository, tag)
        } else {
            format!("{}:{}", self.repository, tag)
        }
    }

    /// Generate Dockerfile
    pub fn generate_dockerfile(&self) -> String {
        let mut dockerfile = String::new();

        dockerfile.push_str("# Stenzel OS Docker Base Image\n");
        dockerfile.push_str("# Auto-generated Dockerfile\n\n");
        dockerfile.push_str("FROM scratch\n\n");

        // Labels
        for (key, value) in &self.base_config.config.labels {
            dockerfile.push_str(&format!("LABEL {}=\"{}\"\n", key, value));
        }
        dockerfile.push('\n');

        // Environment variables
        for (key, value) in &self.base_config.config.env {
            dockerfile.push_str(&format!("ENV {}=\"{}\"\n", key, value));
        }
        dockerfile.push('\n');

        // Process recorded instructions
        for instruction in &self.instructions {
            match instruction {
                DockerInstruction::Run(cmd) => {
                    dockerfile.push_str(&format!("RUN {}\n", cmd));
                }
                DockerInstruction::Copy(src, dst) => {
                    dockerfile.push_str(&format!("COPY {} {}\n", src, dst));
                }
                DockerInstruction::Add(src, dst) => {
                    dockerfile.push_str(&format!("ADD {} {}\n", src, dst));
                }
                DockerInstruction::Workdir(dir) => {
                    dockerfile.push_str(&format!("WORKDIR {}\n", dir));
                }
                DockerInstruction::Expose(port) => {
                    dockerfile.push_str(&format!("EXPOSE {}\n", port));
                }
                DockerInstruction::Volume(path) => {
                    dockerfile.push_str(&format!("VOLUME {}\n", path));
                }
                DockerInstruction::User(user) => {
                    dockerfile.push_str(&format!("USER {}\n", user));
                }
                _ => {}
            }
        }

        // Entrypoint
        if !self.base_config.config.entrypoint.is_empty() {
            dockerfile.push_str(&format!("ENTRYPOINT {:?}\n", self.base_config.config.entrypoint));
        }

        // CMD
        if !self.base_config.config.cmd.is_empty() {
            dockerfile.push_str(&format!("CMD {:?}\n", self.base_config.config.cmd));
        }

        dockerfile
    }
}

/// Registry client for pushing/pulling images
pub struct RegistryClient {
    registry: String,
    auth_token: Option<String>,
    username: Option<String>,
    password: Option<String>,
}

impl RegistryClient {
    pub fn new(registry: &str) -> Self {
        Self {
            registry: String::from(registry),
            auth_token: None,
            username: None,
            password: None,
        }
    }

    /// Set basic auth credentials
    pub fn with_basic_auth(mut self, username: &str, password: &str) -> Self {
        self.username = Some(String::from(username));
        self.password = Some(String::from(password));
        self
    }

    /// Set bearer token
    pub fn with_token(mut self, token: &str) -> Self {
        self.auth_token = Some(String::from(token));
        self
    }

    /// Check if blob exists
    pub fn blob_exists(&self, _repository: &str, _digest: &str) -> DockerResult<bool> {
        // HEAD /v2/<name>/blobs/<digest>
        Ok(false) // Placeholder
    }

    /// Upload blob
    pub fn upload_blob(&self, _repository: &str, _data: &[u8]) -> DockerResult<String> {
        // POST /v2/<name>/blobs/uploads/
        // PATCH /v2/<name>/blobs/uploads/<uuid>
        // PUT /v2/<name>/blobs/uploads/<uuid>?digest=<digest>
        Ok(String::from("sha256:placeholder"))
    }

    /// Upload manifest
    pub fn upload_manifest(&self, _repository: &str, _tag: &str, _manifest: &OciManifest) -> DockerResult<String> {
        // PUT /v2/<name>/manifests/<reference>
        Ok(String::from("sha256:placeholder"))
    }

    /// Pull manifest
    pub fn pull_manifest(&self, _repository: &str, _reference: &str) -> DockerResult<OciManifest> {
        // GET /v2/<name>/manifests/<reference>
        Err(DockerError::ImageNotFound(String::from("not implemented")))
    }

    /// List tags
    pub fn list_tags(&self, _repository: &str) -> DockerResult<Vec<String>> {
        // GET /v2/<name>/tags/list
        Ok(Vec::new())
    }
}

/// OCI layout directory structure builder
pub struct OciLayoutBuilder {
    root_path: String,
    index: OciImageIndex,
    blobs: Vec<(String, Vec<u8>)>,
}

impl OciLayoutBuilder {
    pub fn new(root_path: &str) -> Self {
        Self {
            root_path: String::from(root_path),
            index: OciImageIndex::new(),
            blobs: Vec::new(),
        }
    }

    /// Add a blob
    pub fn add_blob(&mut self, digest: &str, data: Vec<u8>) {
        self.blobs.push((String::from(digest), data));
    }

    /// Add a manifest to the index
    pub fn add_manifest(&mut self, manifest: IndexManifest) {
        self.index.manifests.push(manifest);
    }

    /// Generate OCI layout structure description
    pub fn generate_layout(&self) -> String {
        let mut layout = String::new();

        layout.push_str(&format!("{}/\n", self.root_path));
        layout.push_str("├── oci-layout\n");
        layout.push_str("├── index.json\n");
        layout.push_str("└── blobs/\n");
        layout.push_str("    └── sha256/\n");

        for (digest, _) in &self.blobs {
            let short_digest = if digest.len() > 12 {
                &digest[..12]
            } else {
                digest
            };
            layout.push_str(&format!("        ├── {}...\n", short_digest));
        }

        layout
    }

    /// Generate oci-layout file content
    pub fn oci_layout_json(&self) -> String {
        format!("{{\"imageLayoutVersion\": \"{}\"}}\n", OCI_IMAGE_SPEC_VERSION)
    }
}

/// Convenience function to build Stenzel OS base image
pub fn build_stenzel_base_image(variant: ImageVariant) -> DockerResult<(OciManifest, String)> {
    let mut builder = DockerImageBuilder::new("stenzel/stenzel-os")
        .registry("docker.io")
        .tag("1.0.0")
        .variant(variant)
        .compression(LayerCompression::Gzip)
        .env("STENZEL_VERSION", "1.0.0")
        .env("LANG", "en_US.UTF-8")
        .workdir("/")
        .cmd(vec!["/bin/sh"]);

    // Add standard labels
    builder = builder
        .label("org.opencontainers.image.source", "https://github.com/stenzel/stenzel-os")
        .label("org.opencontainers.image.licenses", "MIT");

    let manifest = builder.build()?;
    let dockerfile = builder.generate_dockerfile();

    Ok((manifest, dockerfile))
}

/// Build multi-architecture image
pub fn build_multiarch_image() -> DockerResult<OciImageIndex> {
    let mut builder = DockerImageBuilder::new("stenzel/stenzel-os")
        .registry("docker.io")
        .tag("1.0.0")
        .variant(ImageVariant::Minimal)
        .add_architecture(ContainerArch::Amd64)
        .add_architecture(ContainerArch::Arm64);

    builder.build_multiarch()
}

pub fn init() {
    crate::kprintln!("docker: OCI container image builder initialized");
}

pub fn format_status() -> String {
    String::from("Docker: Ready")
}
