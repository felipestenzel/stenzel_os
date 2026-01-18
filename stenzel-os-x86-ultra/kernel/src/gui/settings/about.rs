//! About Settings
//!
//! System information, hardware details, and software version info.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use spin::Mutex;

/// Global about settings state
static ABOUT_SETTINGS: Mutex<Option<AboutSettings>> = Mutex::new(None);

/// About settings state
pub struct AboutSettings {
    /// System info
    pub system: SystemInfo,
    /// Hardware info
    pub hardware: HardwareInfo,
    /// Software info
    pub software: SoftwareInfo,
    /// Storage info
    pub storage: Vec<StorageDevice>,
}

/// System information
#[derive(Debug, Clone)]
pub struct SystemInfo {
    /// Computer name
    pub hostname: String,
    /// Device model
    pub device_model: String,
    /// Manufacturer
    pub manufacturer: String,
    /// Serial number
    pub serial_number: Option<String>,
    /// System UUID
    pub system_uuid: Option<String>,
    /// BIOS/UEFI vendor
    pub bios_vendor: String,
    /// BIOS/UEFI version
    pub bios_version: String,
    /// BIOS/UEFI date
    pub bios_date: String,
    /// Boot mode
    pub boot_mode: BootMode,
    /// Secure boot status
    pub secure_boot: bool,
    /// System uptime (seconds)
    pub uptime: u64,
}

/// Boot mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootMode {
    Bios,
    Uefi,
}

impl BootMode {
    pub fn name(&self) -> &'static str {
        match self {
            BootMode::Bios => "Legacy BIOS",
            BootMode::Uefi => "UEFI",
        }
    }
}

/// Hardware information
#[derive(Debug, Clone)]
pub struct HardwareInfo {
    /// CPU info
    pub cpu: CpuInfo,
    /// Memory info
    pub memory: MemoryInfo,
    /// Graphics info
    pub graphics: Vec<GpuInfo>,
    /// Network adapters
    pub network: Vec<NetworkAdapterInfo>,
    /// Audio devices
    pub audio: Vec<AudioDeviceInfo>,
}

/// CPU information
#[derive(Debug, Clone)]
pub struct CpuInfo {
    /// CPU model name
    pub model: String,
    /// Vendor (Intel, AMD, etc.)
    pub vendor: String,
    /// Number of physical cores
    pub cores: u32,
    /// Number of logical processors
    pub threads: u32,
    /// Base frequency (MHz)
    pub base_freq_mhz: u32,
    /// Max turbo frequency (MHz)
    pub max_freq_mhz: Option<u32>,
    /// Cache sizes
    pub cache_l1: u32, // KB
    pub cache_l2: u32, // KB
    pub cache_l3: u32, // KB
    /// Architecture
    pub architecture: String,
    /// Features
    pub features: Vec<String>,
}

/// Memory information
#[derive(Debug, Clone)]
pub struct MemoryInfo {
    /// Total RAM (bytes)
    pub total: u64,
    /// Available RAM (bytes)
    pub available: u64,
    /// Used RAM (bytes)
    pub used: u64,
    /// Memory type
    pub memory_type: String,
    /// Speed (MT/s)
    pub speed: u32,
    /// Number of slots
    pub slots_total: u32,
    /// Number of slots used
    pub slots_used: u32,
}

impl MemoryInfo {
    /// Get total in GB
    pub fn total_gb(&self) -> f32 {
        self.total as f32 / (1024.0 * 1024.0 * 1024.0)
    }

    /// Get used percentage
    pub fn used_percent(&self) -> u32 {
        if self.total > 0 {
            ((self.used as f64 / self.total as f64) * 100.0) as u32
        } else {
            0
        }
    }
}

/// GPU information
#[derive(Debug, Clone)]
pub struct GpuInfo {
    /// GPU name
    pub name: String,
    /// Vendor
    pub vendor: String,
    /// VRAM (bytes)
    pub vram: u64,
    /// Driver name
    pub driver: String,
    /// Driver version
    pub driver_version: String,
    /// Is discrete GPU
    pub discrete: bool,
    /// Current resolution
    pub resolution: Option<(u32, u32)>,
}

/// Network adapter info
#[derive(Debug, Clone)]
pub struct NetworkAdapterInfo {
    /// Adapter name
    pub name: String,
    /// Interface name
    pub interface: String,
    /// Is wireless
    pub wireless: bool,
    /// MAC address
    pub mac_address: String,
    /// Speed (Mbps)
    pub speed: Option<u32>,
    /// Is connected
    pub connected: bool,
}

/// Audio device info
#[derive(Debug, Clone)]
pub struct AudioDeviceInfo {
    /// Device name
    pub name: String,
    /// Device type
    pub device_type: String,
    /// Codec
    pub codec: String,
}

/// Storage device
#[derive(Debug, Clone)]
pub struct StorageDevice {
    /// Device name
    pub name: String,
    /// Device model
    pub model: String,
    /// Device type
    pub device_type: StorageType,
    /// Total size (bytes)
    pub size: u64,
    /// Used space (bytes)
    pub used: u64,
    /// Interface (SATA, NVMe, USB)
    pub interface: String,
    /// Mount point
    pub mount_point: Option<String>,
    /// File system
    pub filesystem: Option<String>,
    /// Is removable
    pub removable: bool,
}

/// Storage type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageType {
    Ssd,
    Hdd,
    Nvme,
    UsbDrive,
    SdCard,
    OpticalDrive,
    Unknown,
}

impl StorageType {
    pub fn name(&self) -> &'static str {
        match self {
            StorageType::Ssd => "SSD",
            StorageType::Hdd => "HDD",
            StorageType::Nvme => "NVMe SSD",
            StorageType::UsbDrive => "USB Drive",
            StorageType::SdCard => "SD Card",
            StorageType::OpticalDrive => "Optical Drive",
            StorageType::Unknown => "Unknown",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            StorageType::Ssd | StorageType::Hdd | StorageType::Nvme => "drive-harddisk",
            StorageType::UsbDrive => "drive-removable-media-usb",
            StorageType::SdCard => "media-flash-sd-mmc",
            StorageType::OpticalDrive => "drive-optical",
            StorageType::Unknown => "drive-harddisk",
        }
    }
}

/// Software information
#[derive(Debug, Clone)]
pub struct SoftwareInfo {
    /// OS name
    pub os_name: String,
    /// OS version
    pub os_version: String,
    /// OS build
    pub os_build: String,
    /// Kernel version
    pub kernel_version: String,
    /// Kernel build date
    pub kernel_build_date: String,
    /// Desktop environment
    pub desktop: String,
    /// Desktop version
    pub desktop_version: String,
    /// Shell
    pub shell: String,
    /// Architecture
    pub architecture: String,
    /// License
    pub license: String,
}

/// Initialize about settings
pub fn init() {
    let mut state = ABOUT_SETTINGS.lock();
    if state.is_some() {
        return;
    }

    *state = Some(AboutSettings {
        system: SystemInfo {
            hostname: "stenzel-os".to_string(),
            device_model: "Generic PC".to_string(),
            manufacturer: "Unknown".to_string(),
            serial_number: None,
            system_uuid: None,
            bios_vendor: "Unknown".to_string(),
            bios_version: "Unknown".to_string(),
            bios_date: "Unknown".to_string(),
            boot_mode: BootMode::Uefi,
            secure_boot: false,
            uptime: 0,
        },
        hardware: HardwareInfo {
            cpu: CpuInfo {
                model: "Unknown CPU".to_string(),
                vendor: "Unknown".to_string(),
                cores: 1,
                threads: 1,
                base_freq_mhz: 1000,
                max_freq_mhz: None,
                cache_l1: 32,
                cache_l2: 256,
                cache_l3: 0,
                architecture: "x86_64".to_string(),
                features: Vec::new(),
            },
            memory: MemoryInfo {
                total: 0,
                available: 0,
                used: 0,
                memory_type: "Unknown".to_string(),
                speed: 0,
                slots_total: 0,
                slots_used: 0,
            },
            graphics: Vec::new(),
            network: Vec::new(),
            audio: Vec::new(),
        },
        software: SoftwareInfo {
            os_name: "Stenzel OS".to_string(),
            os_version: "1.0.0".to_string(),
            os_build: "2026.01".to_string(),
            kernel_version: "1.0.0".to_string(),
            kernel_build_date: "2026-01-17".to_string(),
            desktop: "Stenzel Desktop".to_string(),
            desktop_version: "1.0.0".to_string(),
            shell: "stenzel-shell".to_string(),
            architecture: "x86_64".to_string(),
            license: "MIT".to_string(),
        },
        storage: Vec::new(),
    });

    crate::kprintln!("about settings: initialized");
}

/// Get system info
pub fn get_system_info() -> Option<SystemInfo> {
    let state = ABOUT_SETTINGS.lock();
    state.as_ref().map(|s| s.system.clone())
}

/// Get hardware info
pub fn get_hardware_info() -> Option<HardwareInfo> {
    let state = ABOUT_SETTINGS.lock();
    state.as_ref().map(|s| s.hardware.clone())
}

/// Get software info
pub fn get_software_info() -> Option<SoftwareInfo> {
    let state = ABOUT_SETTINGS.lock();
    state.as_ref().map(|s| s.software.clone())
}

/// Get storage devices
pub fn get_storage_devices() -> Vec<StorageDevice> {
    let state = ABOUT_SETTINGS.lock();
    state.as_ref().map(|s| s.storage.clone()).unwrap_or_default()
}

/// Get hostname
pub fn get_hostname() -> String {
    let state = ABOUT_SETTINGS.lock();
    state.as_ref().map(|s| s.system.hostname.clone()).unwrap_or_else(|| "stenzel-os".to_string())
}

/// Set hostname
pub fn set_hostname(hostname: &str) {
    let mut state = ABOUT_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.system.hostname = hostname.to_string();
        // TODO: Actually set system hostname
    }
}

/// Update CPU info (called by hardware detection)
pub fn update_cpu_info(cpu: CpuInfo) {
    let mut state = ABOUT_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.hardware.cpu = cpu;
    }
}

/// Update memory info (called by memory manager)
pub fn update_memory_info(memory: MemoryInfo) {
    let mut state = ABOUT_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.hardware.memory = memory;
    }
}

/// Add GPU info
pub fn add_gpu_info(gpu: GpuInfo) {
    let mut state = ABOUT_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.hardware.graphics.push(gpu);
    }
}

/// Add network adapter info
pub fn add_network_adapter(adapter: NetworkAdapterInfo) {
    let mut state = ABOUT_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.hardware.network.push(adapter);
    }
}

/// Add audio device info
pub fn add_audio_device(device: AudioDeviceInfo) {
    let mut state = ABOUT_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.hardware.audio.push(device);
    }
}

/// Update storage devices
pub fn update_storage_devices(devices: Vec<StorageDevice>) {
    let mut state = ABOUT_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.storage = devices;
    }
}

/// Update uptime
pub fn update_uptime(uptime_seconds: u64) {
    let mut state = ABOUT_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.system.uptime = uptime_seconds;
    }
}

/// Format uptime as human-readable string
pub fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;

    if days > 0 {
        alloc::format!("{} days, {} hours, {} minutes", days, hours, minutes)
    } else if hours > 0 {
        alloc::format!("{} hours, {} minutes", hours, minutes)
    } else {
        alloc::format!("{} minutes", minutes)
    }
}

/// Format bytes as human-readable string
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        alloc::format!("{:.1} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        alloc::format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        alloc::format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        alloc::format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        alloc::format!("{} bytes", bytes)
    }
}

/// Update BIOS info
pub fn update_bios_info(vendor: &str, version: &str, date: &str) {
    let mut state = ABOUT_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.system.bios_vendor = vendor.to_string();
        s.system.bios_version = version.to_string();
        s.system.bios_date = date.to_string();
    }
}

/// Set boot mode
pub fn set_boot_mode(mode: BootMode) {
    let mut state = ABOUT_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.system.boot_mode = mode;
    }
}

/// Set secure boot status
pub fn set_secure_boot(enabled: bool) {
    let mut state = ABOUT_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.system.secure_boot = enabled;
    }
}

/// Set device model
pub fn set_device_model(model: &str, manufacturer: &str) {
    let mut state = ABOUT_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.system.device_model = model.to_string();
        s.system.manufacturer = manufacturer.to_string();
    }
}
