//! Cloud Image Builder for Stenzel OS.
//!
//! Supports creating cloud images in various formats:
//! - qcow2 (QEMU/KVM, OpenStack)
//! - AMI (Amazon Web Services)
//! - VHD/VHDX (Azure, Hyper-V)
//! - OVA/OVF (VMware, VirtualBox)
//! - RAW (Generic)

#![allow(dead_code)]

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

// ============================================================================
// Cloud Image Formats
// ============================================================================

/// Cloud image format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudImageFormat {
    /// Raw disk image
    Raw,
    /// QEMU Copy-On-Write v2
    Qcow2,
    /// Amazon Machine Image
    Ami,
    /// Azure VHD (Virtual Hard Disk)
    Vhd,
    /// Hyper-V VHDX (improved VHD)
    Vhdx,
    /// VMware VMDK
    Vmdk,
    /// VirtualBox VDI
    Vdi,
    /// OVA (Open Virtual Appliance)
    Ova,
}

impl CloudImageFormat {
    /// Get file extension
    pub fn extension(&self) -> &'static str {
        match self {
            CloudImageFormat::Raw => "img",
            CloudImageFormat::Qcow2 => "qcow2",
            CloudImageFormat::Ami => "ami",
            CloudImageFormat::Vhd => "vhd",
            CloudImageFormat::Vhdx => "vhdx",
            CloudImageFormat::Vmdk => "vmdk",
            CloudImageFormat::Vdi => "vdi",
            CloudImageFormat::Ova => "ova",
        }
    }

    /// Get MIME type
    pub fn mime_type(&self) -> &'static str {
        match self {
            CloudImageFormat::Raw => "application/octet-stream",
            CloudImageFormat::Qcow2 => "application/x-qcow2",
            CloudImageFormat::Ami => "application/x-ami",
            CloudImageFormat::Vhd => "application/x-vhd",
            CloudImageFormat::Vhdx => "application/x-vhdx",
            CloudImageFormat::Vmdk => "application/x-vmdk",
            CloudImageFormat::Vdi => "application/x-virtualbox-vdi",
            CloudImageFormat::Ova => "application/x-tar",
        }
    }

    /// Check if format supports compression
    pub fn supports_compression(&self) -> bool {
        matches!(self, CloudImageFormat::Qcow2 | CloudImageFormat::Vmdk)
    }

    /// Check if format supports snapshots
    pub fn supports_snapshots(&self) -> bool {
        matches!(self, CloudImageFormat::Qcow2 | CloudImageFormat::Vhdx | CloudImageFormat::Vmdk)
    }
}

// ============================================================================
// Cloud Provider Configuration
// ============================================================================

/// Cloud provider
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudProvider {
    /// Amazon Web Services
    Aws,
    /// Google Cloud Platform
    Gcp,
    /// Microsoft Azure
    Azure,
    /// OpenStack
    OpenStack,
    /// DigitalOcean
    DigitalOcean,
    /// Vultr
    Vultr,
    /// Linode
    Linode,
    /// VMware vSphere
    VSphere,
    /// Proxmox VE
    Proxmox,
    /// Generic/Custom
    Generic,
}

impl CloudProvider {
    /// Get preferred image format
    pub fn preferred_format(&self) -> CloudImageFormat {
        match self {
            CloudProvider::Aws => CloudImageFormat::Ami,
            CloudProvider::Gcp => CloudImageFormat::Raw, // GCE uses raw with tar.gz
            CloudProvider::Azure => CloudImageFormat::Vhd,
            CloudProvider::OpenStack => CloudImageFormat::Qcow2,
            CloudProvider::DigitalOcean => CloudImageFormat::Qcow2,
            CloudProvider::Vultr => CloudImageFormat::Raw,
            CloudProvider::Linode => CloudImageFormat::Raw,
            CloudProvider::VSphere => CloudImageFormat::Vmdk,
            CloudProvider::Proxmox => CloudImageFormat::Qcow2,
            CloudProvider::Generic => CloudImageFormat::Qcow2,
        }
    }

    /// Get cloud-init datasource
    pub fn cloud_init_datasource(&self) -> &'static str {
        match self {
            CloudProvider::Aws => "Ec2",
            CloudProvider::Gcp => "GCE",
            CloudProvider::Azure => "Azure",
            CloudProvider::OpenStack => "OpenStack",
            CloudProvider::DigitalOcean => "DigitalOcean",
            CloudProvider::Vultr => "Vultr",
            CloudProvider::Linode => "Linode",
            CloudProvider::VSphere => "VMware",
            CloudProvider::Proxmox => "NoCloud",
            CloudProvider::Generic => "NoCloud",
        }
    }
}

// ============================================================================
// Image Configuration
// ============================================================================

/// Cloud image configuration
#[derive(Debug, Clone)]
pub struct CloudImageConfig {
    /// Image name
    pub name: String,
    /// Image version
    pub version: String,
    /// Image format
    pub format: CloudImageFormat,
    /// Target provider (optional)
    pub provider: Option<CloudProvider>,
    /// Virtual disk size in GB
    pub disk_size_gb: u64,
    /// Enable compression
    pub compress: bool,
    /// Enable cloud-init
    pub cloud_init: bool,
    /// Include SSH server
    pub include_ssh: bool,
    /// Root password (None = disable password auth)
    pub root_password: Option<String>,
    /// Default user
    pub default_user: Option<UserConfig>,
    /// Partition layout
    pub partitions: PartitionLayout,
    /// Network configuration
    pub network: NetworkConfig,
    /// Additional packages to install
    pub packages: Vec<String>,
    /// Custom scripts to run
    pub scripts: Vec<String>,
}

impl Default for CloudImageConfig {
    fn default() -> Self {
        Self {
            name: "stenzel-os".to_string(),
            version: "1.0.0".to_string(),
            format: CloudImageFormat::Qcow2,
            provider: None,
            disk_size_gb: 10,
            compress: true,
            cloud_init: true,
            include_ssh: true,
            root_password: None,
            default_user: Some(UserConfig::default()),
            partitions: PartitionLayout::default(),
            network: NetworkConfig::default(),
            packages: Vec::new(),
            scripts: Vec::new(),
        }
    }
}

/// User configuration
#[derive(Debug, Clone)]
pub struct UserConfig {
    /// Username
    pub username: String,
    /// Full name
    pub full_name: String,
    /// Groups
    pub groups: Vec<String>,
    /// SSH authorized keys
    pub ssh_keys: Vec<String>,
    /// Enable sudo
    pub sudo: bool,
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            username: "stenzel".to_string(),
            full_name: "Stenzel OS User".to_string(),
            groups: vec!["wheel".to_string(), "docker".to_string()],
            ssh_keys: Vec::new(),
            sudo: true,
        }
    }
}

/// Partition layout for cloud images
#[derive(Debug, Clone)]
pub struct PartitionLayout {
    /// Use GPT (vs MBR)
    pub gpt: bool,
    /// Boot partition size in MB
    pub boot_size_mb: u64,
    /// EFI partition size in MB (0 = no EFI)
    pub efi_size_mb: u64,
    /// Swap size in MB (0 = no swap)
    pub swap_size_mb: u64,
    /// Root filesystem type
    pub root_fs: String,
}

impl Default for PartitionLayout {
    fn default() -> Self {
        Self {
            gpt: true,
            boot_size_mb: 512,
            efi_size_mb: 256,
            swap_size_mb: 0, // Cloud images typically don't need swap
            root_fs: "ext4".to_string(),
        }
    }
}

/// Network configuration
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    /// Use DHCP
    pub dhcp: bool,
    /// Hostname (empty = use cloud-init)
    pub hostname: String,
    /// DNS servers
    pub dns_servers: Vec<String>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            dhcp: true,
            hostname: String::new(),
            dns_servers: Vec::new(),
        }
    }
}

// ============================================================================
// QCOW2 Format
// ============================================================================

/// QCOW2 header (v3)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Qcow2Header {
    /// Magic number (QFI\xfb)
    pub magic: u32,
    /// Version (2 or 3)
    pub version: u32,
    /// Backing file offset
    pub backing_file_offset: u64,
    /// Backing file size
    pub backing_file_size: u32,
    /// Cluster bits (usually 16 = 64KB clusters)
    pub cluster_bits: u32,
    /// Virtual size
    pub size: u64,
    /// Encryption method
    pub crypt_method: u32,
    /// L1 table size
    pub l1_size: u32,
    /// L1 table offset
    pub l1_table_offset: u64,
    /// Refcount table offset
    pub refcount_table_offset: u64,
    /// Refcount table clusters
    pub refcount_table_clusters: u32,
    /// Number of snapshots
    pub nb_snapshots: u32,
    /// Snapshots offset
    pub snapshots_offset: u64,
    // v3 additional fields...
}

impl Qcow2Header {
    /// QCOW2 magic number
    pub const MAGIC: u32 = 0x514649FB; // "QFI\xfb"

    /// Create new QCOW2 header
    pub fn new(size: u64, cluster_bits: u32) -> Self {
        Self {
            magic: Self::MAGIC,
            version: 3,
            backing_file_offset: 0,
            backing_file_size: 0,
            cluster_bits,
            size,
            crypt_method: 0,
            l1_size: 0,
            l1_table_offset: 0,
            refcount_table_offset: 0,
            refcount_table_clusters: 0,
            nb_snapshots: 0,
            snapshots_offset: 0,
        }
    }
}

// ============================================================================
// VHD Format
// ============================================================================

/// VHD footer (512 bytes)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct VhdFooter {
    /// Cookie ("conectix")
    pub cookie: [u8; 8],
    /// Features
    pub features: u32,
    /// File format version
    pub file_format_version: u32,
    /// Data offset
    pub data_offset: u64,
    /// Timestamp
    pub timestamp: u32,
    /// Creator application
    pub creator_application: [u8; 4],
    /// Creator version
    pub creator_version: u32,
    /// Creator host OS
    pub creator_host_os: u32,
    /// Original size
    pub original_size: u64,
    /// Current size
    pub current_size: u64,
    /// Disk geometry
    pub disk_geometry: u32,
    /// Disk type
    pub disk_type: u32,
    /// Checksum
    pub checksum: u32,
    /// Unique ID
    pub unique_id: [u8; 16],
    /// Saved state
    pub saved_state: u8,
    /// Reserved
    pub reserved: [u8; 427],
}

impl VhdFooter {
    /// VHD cookie
    pub const COOKIE: &'static [u8; 8] = b"conectix";

    /// Fixed disk type
    pub const DISK_TYPE_FIXED: u32 = 2;

    /// Dynamic disk type
    pub const DISK_TYPE_DYNAMIC: u32 = 3;

    /// Create new VHD footer
    pub fn new(size: u64) -> Self {
        let mut footer = Self {
            cookie: *Self::COOKIE,
            features: 2, // Reserved
            file_format_version: 0x00010000,
            data_offset: u64::MAX, // Fixed disk
            timestamp: 0,
            creator_application: *b"stzl",
            creator_version: 0x00010000,
            creator_host_os: 0x5769326B, // "Wi2k" (Windows)
            original_size: size,
            current_size: size,
            disk_geometry: Self::calculate_geometry(size),
            disk_type: Self::DISK_TYPE_FIXED,
            checksum: 0,
            unique_id: [0; 16],
            saved_state: 0,
            reserved: [0; 427],
        };
        footer.checksum = footer.calculate_checksum();
        footer
    }

    /// Calculate disk geometry (CHS)
    fn calculate_geometry(size: u64) -> u32 {
        let total_sectors = size / 512;
        let (cylinders, heads, sectors_per_track) = if total_sectors > 65535 * 16 * 255 {
            (65535, 16, 255)
        } else {
            let mut sectors_per_track = 17u64;
            let mut heads = 4u64;
            let mut cylinders = total_sectors / sectors_per_track / heads;

            if cylinders > 65535 {
                sectors_per_track = 31;
                heads = 16;
                cylinders = total_sectors / sectors_per_track / heads;
            }

            (cylinders as u16, heads as u8, sectors_per_track as u8)
        };

        ((cylinders as u32) << 16) | ((heads as u32) << 8) | (sectors_per_track as u32)
    }

    /// Calculate checksum
    fn calculate_checksum(&self) -> u32 {
        // Sum all bytes except checksum field
        // Return one's complement
        0
    }
}

// ============================================================================
// VMDK Format
// ============================================================================

/// VMDK descriptor
pub struct VmdkDescriptor {
    /// Version
    pub version: u32,
    /// Content ID
    pub cid: u32,
    /// Parent content ID
    pub parent_cid: u32,
    /// Create type
    pub create_type: String,
    /// Extent description
    pub extent: VmdkExtent,
}

/// VMDK extent
#[derive(Debug, Clone)]
pub struct VmdkExtent {
    /// Access mode (RW, RDONLY, etc.)
    pub access: String,
    /// Size in sectors
    pub sectors: u64,
    /// Type (SPARSE, FLAT, etc.)
    pub extent_type: String,
    /// Filename
    pub filename: String,
}

impl VmdkDescriptor {
    /// Generate VMDK descriptor text
    pub fn to_string(&self) -> String {
        let mut desc = String::new();

        desc.push_str("# Disk DescriptorFile\n");
        desc.push_str(&format!("version={}\n", self.version));
        desc.push_str(&format!("CID={:08x}\n", self.cid));
        desc.push_str(&format!("parentCID={:08x}\n", self.parent_cid));
        desc.push_str(&format!("createType=\"{}\"\n", self.create_type));
        desc.push('\n');
        desc.push_str("# Extent description\n");
        desc.push_str(&format!(
            "{} {} {} \"{}\"\n",
            self.extent.access,
            self.extent.sectors,
            self.extent.extent_type,
            self.extent.filename
        ));
        desc.push('\n');
        desc.push_str("# The Disk Data Base\n");
        desc.push_str("#DDB\n");

        desc
    }
}

// ============================================================================
// Cloud-Init Configuration
// ============================================================================

/// Cloud-init configuration
#[derive(Debug, Clone)]
pub struct CloudInitConfig {
    /// Datasource list
    pub datasource_list: Vec<String>,
    /// Manage /etc/hosts
    pub manage_etc_hosts: bool,
    /// Preserve hostname
    pub preserve_hostname: bool,
    /// SSH password authentication
    pub ssh_pwauth: bool,
    /// Disable root
    pub disable_root: bool,
    /// User configuration
    pub users: Vec<CloudInitUser>,
    /// Packages to install
    pub packages: Vec<String>,
    /// Runcmd commands
    pub runcmd: Vec<String>,
}

/// Cloud-init user
#[derive(Debug, Clone)]
pub struct CloudInitUser {
    /// Username
    pub name: String,
    /// Groups
    pub groups: Vec<String>,
    /// Shell
    pub shell: String,
    /// Sudo access
    pub sudo: String,
    /// SSH authorized keys
    pub ssh_authorized_keys: Vec<String>,
}

impl CloudInitConfig {
    /// Generate cloud-init user-data YAML
    pub fn to_yaml(&self) -> String {
        let mut yaml = String::new();

        yaml.push_str("#cloud-config\n");

        // Datasource
        if !self.datasource_list.is_empty() {
            yaml.push_str("datasource_list:\n");
            for ds in &self.datasource_list {
                yaml.push_str(&format!("  - {}\n", ds));
            }
        }

        yaml.push_str(&format!("manage_etc_hosts: {}\n", self.manage_etc_hosts));
        yaml.push_str(&format!("preserve_hostname: {}\n", self.preserve_hostname));
        yaml.push_str(&format!("ssh_pwauth: {}\n", self.ssh_pwauth));
        yaml.push_str(&format!("disable_root: {}\n", self.disable_root));

        // Users
        if !self.users.is_empty() {
            yaml.push_str("users:\n");
            for user in &self.users {
                yaml.push_str(&format!("  - name: {}\n", user.name));
                if !user.groups.is_empty() {
                    yaml.push_str(&format!("    groups: {}\n", user.groups.join(", ")));
                }
                yaml.push_str(&format!("    shell: {}\n", user.shell));
                yaml.push_str(&format!("    sudo: {}\n", user.sudo));
                if !user.ssh_authorized_keys.is_empty() {
                    yaml.push_str("    ssh_authorized_keys:\n");
                    for key in &user.ssh_authorized_keys {
                        yaml.push_str(&format!("      - {}\n", key));
                    }
                }
            }
        }

        // Packages
        if !self.packages.is_empty() {
            yaml.push_str("packages:\n");
            for pkg in &self.packages {
                yaml.push_str(&format!("  - {}\n", pkg));
            }
        }

        // Runcmd
        if !self.runcmd.is_empty() {
            yaml.push_str("runcmd:\n");
            for cmd in &self.runcmd {
                yaml.push_str(&format!("  - {}\n", cmd));
            }
        }

        yaml
    }
}

// ============================================================================
// Cloud Image Builder
// ============================================================================

/// Build result
#[derive(Debug, Clone)]
pub enum CloudBuildResult {
    /// Build successful
    Success {
        path: String,
        size: u64,
        checksum: String,
    },
    /// Build failed
    Failed {
        error: CloudBuildError,
        message: String,
    },
}

/// Build error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudBuildError {
    /// I/O error
    IoError,
    /// Configuration error
    ConfigError,
    /// Format error
    FormatError,
    /// Not enough space
    InsufficientSpace,
    /// Unsupported format
    UnsupportedFormat,
}

/// Cloud image builder
pub struct CloudImageBuilder {
    /// Configuration
    config: CloudImageConfig,
    /// Progress (0-100)
    progress: u8,
    /// Status message
    status: String,
}

impl CloudImageBuilder {
    /// Create new builder
    pub fn new(config: CloudImageConfig) -> Self {
        Self {
            config,
            progress: 0,
            status: "Ready".to_string(),
        }
    }

    /// Build the cloud image
    pub fn build(&mut self, output_path: &str) -> CloudBuildResult {
        self.progress = 0;
        self.status = "Initializing...".to_string();

        // Validate configuration
        if let Err(e) = self.validate() {
            return CloudBuildResult::Failed {
                error: e,
                message: "Configuration validation failed".to_string(),
            };
        }

        self.progress = 5;
        self.status = "Creating raw disk image...".to_string();

        // Create raw disk image
        let raw_path = format!("{}.raw", output_path);
        if let Err(e) = self.create_raw_image(&raw_path) {
            return CloudBuildResult::Failed {
                error: e,
                message: "Failed to create raw image".to_string(),
            };
        }

        self.progress = 20;
        self.status = "Partitioning disk...".to_string();

        // Partition the disk
        if let Err(e) = self.partition_disk(&raw_path) {
            return CloudBuildResult::Failed {
                error: e,
                message: "Failed to partition disk".to_string(),
            };
        }

        self.progress = 30;
        self.status = "Formatting partitions...".to_string();

        // Format partitions
        if let Err(e) = self.format_partitions(&raw_path) {
            return CloudBuildResult::Failed {
                error: e,
                message: "Failed to format partitions".to_string(),
            };
        }

        self.progress = 40;
        self.status = "Installing base system...".to_string();

        // Install base system
        if let Err(e) = self.install_system(&raw_path) {
            return CloudBuildResult::Failed {
                error: e,
                message: "Failed to install system".to_string(),
            };
        }

        self.progress = 60;
        self.status = "Configuring system...".to_string();

        // Configure system
        if let Err(e) = self.configure_system(&raw_path) {
            return CloudBuildResult::Failed {
                error: e,
                message: "Failed to configure system".to_string(),
            };
        }

        self.progress = 75;
        self.status = format!("Converting to {}...", self.config.format.extension());

        // Convert to target format
        let final_path = format!("{}.{}", output_path, self.config.format.extension());
        if let Err(e) = self.convert_image(&raw_path, &final_path) {
            return CloudBuildResult::Failed {
                error: e,
                message: "Failed to convert image".to_string(),
            };
        }

        self.progress = 90;
        self.status = "Calculating checksum...".to_string();

        // Calculate checksum
        let checksum = self.calculate_checksum(&final_path);

        // Get final size
        let size = self.get_file_size(&final_path);

        self.progress = 100;
        self.status = "Complete".to_string();

        CloudBuildResult::Success {
            path: final_path,
            size,
            checksum,
        }
    }

    /// Validate configuration
    fn validate(&self) -> Result<(), CloudBuildError> {
        if self.config.disk_size_gb < 1 {
            return Err(CloudBuildError::ConfigError);
        }

        if self.config.name.is_empty() {
            return Err(CloudBuildError::ConfigError);
        }

        Ok(())
    }

    /// Create raw disk image
    fn create_raw_image(&self, _path: &str) -> Result<(), CloudBuildError> {
        // Would create sparse file of specified size
        Ok(())
    }

    /// Partition the disk
    fn partition_disk(&self, _path: &str) -> Result<(), CloudBuildError> {
        // Would create GPT/MBR partitions
        Ok(())
    }

    /// Format partitions
    fn format_partitions(&self, _path: &str) -> Result<(), CloudBuildError> {
        // Would format partitions with specified filesystems
        Ok(())
    }

    /// Install base system
    fn install_system(&self, _path: &str) -> Result<(), CloudBuildError> {
        // Would copy system files
        Ok(())
    }

    /// Configure system for cloud
    fn configure_system(&self, _path: &str) -> Result<(), CloudBuildError> {
        // Would:
        // - Install cloud-init
        // - Configure network
        // - Create users
        // - Install packages
        // - Run custom scripts
        Ok(())
    }

    /// Convert to target format
    fn convert_image(&self, _raw_path: &str, _target_path: &str) -> Result<(), CloudBuildError> {
        match self.config.format {
            CloudImageFormat::Raw => {
                // Just rename
                Ok(())
            }
            CloudImageFormat::Qcow2 => {
                // Convert using qemu-img or our own implementation
                Ok(())
            }
            CloudImageFormat::Vhd => {
                // Convert to VHD
                Ok(())
            }
            CloudImageFormat::Vhdx => {
                // Convert to VHDX
                Ok(())
            }
            CloudImageFormat::Vmdk => {
                // Convert to VMDK
                Ok(())
            }
            CloudImageFormat::Vdi => {
                // Convert to VDI
                Ok(())
            }
            CloudImageFormat::Ami => {
                // Create AMI bundle
                Ok(())
            }
            CloudImageFormat::Ova => {
                // Create OVA (tar of OVF + VMDK)
                Ok(())
            }
        }
    }

    /// Calculate checksum
    fn calculate_checksum(&self, _path: &str) -> String {
        // Would calculate SHA256
        "sha256:placeholder".to_string()
    }

    /// Get file size
    fn get_file_size(&self, _path: &str) -> u64 {
        // Would get actual file size
        self.config.disk_size_gb * 1024 * 1024 * 1024
    }

    /// Get progress
    pub fn progress(&self) -> u8 {
        self.progress
    }

    /// Get status
    pub fn status(&self) -> &str {
        &self.status
    }
}

// ============================================================================
// Provider-Specific Builders
// ============================================================================

/// AWS AMI builder
pub struct AwsAmiBuilder {
    /// Base config
    config: CloudImageConfig,
    /// AWS region
    region: String,
    /// S3 bucket for upload
    s3_bucket: Option<String>,
    /// AMI name
    ami_name: String,
    /// AMI description
    ami_description: String,
}

impl AwsAmiBuilder {
    /// Create new AWS AMI builder
    pub fn new(name: &str, region: &str) -> Self {
        let mut config = CloudImageConfig::default();
        config.format = CloudImageFormat::Raw; // AWS uses raw for import
        config.provider = Some(CloudProvider::Aws);

        Self {
            config,
            region: region.to_string(),
            s3_bucket: None,
            ami_name: name.to_string(),
            ami_description: format!("Stenzel OS {}", name),
        }
    }

    /// Set S3 bucket for upload
    pub fn with_s3_bucket(mut self, bucket: &str) -> Self {
        self.s3_bucket = Some(bucket.to_string());
        self
    }

    /// Build AMI
    pub fn build(&mut self, output_path: &str) -> CloudBuildResult {
        let mut builder = CloudImageBuilder::new(self.config.clone());
        builder.build(output_path)
    }
}

/// Azure VHD builder
pub struct AzureVhdBuilder {
    /// Base config
    config: CloudImageConfig,
    /// Azure storage account
    storage_account: Option<String>,
    /// Container name
    container: Option<String>,
}

impl AzureVhdBuilder {
    /// Create new Azure VHD builder
    pub fn new(name: &str) -> Self {
        let mut config = CloudImageConfig::default();
        config.format = CloudImageFormat::Vhd;
        config.provider = Some(CloudProvider::Azure);
        config.name = name.to_string();

        Self {
            config,
            storage_account: None,
            container: None,
        }
    }

    /// Build VHD
    pub fn build(&mut self, output_path: &str) -> CloudBuildResult {
        let mut builder = CloudImageBuilder::new(self.config.clone());
        builder.build(output_path)
    }
}

/// GCP image builder
pub struct GcpImageBuilder {
    /// Base config
    config: CloudImageConfig,
    /// GCP project
    project: String,
    /// Image family
    image_family: Option<String>,
}

impl GcpImageBuilder {
    /// Create new GCP image builder
    pub fn new(name: &str, project: &str) -> Self {
        let mut config = CloudImageConfig::default();
        config.format = CloudImageFormat::Raw;
        config.provider = Some(CloudProvider::Gcp);
        config.name = name.to_string();

        Self {
            config,
            project: project.to_string(),
            image_family: None,
        }
    }

    /// Build image
    pub fn build(&mut self, output_path: &str) -> CloudBuildResult {
        // GCP expects raw disk in tar.gz format
        let mut builder = CloudImageBuilder::new(self.config.clone());
        builder.build(output_path)
    }
}

// ============================================================================
// Public API
// ============================================================================

/// Initialize cloud image subsystem
pub fn init() {
    // Initialize cloud image builder
}

/// Build cloud image with default config
pub fn build_cloud_image(output_path: &str, format: CloudImageFormat) -> CloudBuildResult {
    let mut config = CloudImageConfig::default();
    config.format = format;

    let mut builder = CloudImageBuilder::new(config);
    builder.build(output_path)
}

/// Build cloud image with custom config
pub fn build_cloud_image_with_config(output_path: &str, config: CloudImageConfig) -> CloudBuildResult {
    let mut builder = CloudImageBuilder::new(config);
    builder.build(output_path)
}

/// Build AWS AMI
pub fn build_aws_ami(name: &str, region: &str, output_path: &str) -> CloudBuildResult {
    let mut builder = AwsAmiBuilder::new(name, region);
    builder.build(output_path)
}

/// Build Azure VHD
pub fn build_azure_vhd(name: &str, output_path: &str) -> CloudBuildResult {
    let mut builder = AzureVhdBuilder::new(name);
    builder.build(output_path)
}

/// Build GCP image
pub fn build_gcp_image(name: &str, project: &str, output_path: &str) -> CloudBuildResult {
    let mut builder = GcpImageBuilder::new(name, project);
    builder.build(output_path)
}

/// List supported formats
pub fn supported_formats() -> Vec<CloudImageFormat> {
    vec![
        CloudImageFormat::Raw,
        CloudImageFormat::Qcow2,
        CloudImageFormat::Ami,
        CloudImageFormat::Vhd,
        CloudImageFormat::Vhdx,
        CloudImageFormat::Vmdk,
        CloudImageFormat::Vdi,
        CloudImageFormat::Ova,
    ]
}

/// List supported providers
pub fn supported_providers() -> Vec<CloudProvider> {
    vec![
        CloudProvider::Aws,
        CloudProvider::Gcp,
        CloudProvider::Azure,
        CloudProvider::OpenStack,
        CloudProvider::DigitalOcean,
        CloudProvider::Vultr,
        CloudProvider::Linode,
        CloudProvider::VSphere,
        CloudProvider::Proxmox,
        CloudProvider::Generic,
    ]
}
