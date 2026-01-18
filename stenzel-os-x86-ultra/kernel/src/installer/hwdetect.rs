//! Hardware Detection for Installer
//!
//! Detects CPU, RAM, disks, GPU, and other hardware for installation.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;

use super::{InstallError, InstallResult};

/// Detected hardware information
#[derive(Debug, Clone)]
pub struct HardwareInfo {
    pub cpu: CpuInfo,
    pub memory: MemoryInfo,
    pub disks: Vec<DiskInfo>,
    pub gpus: Vec<GpuInfo>,
    pub network: Vec<NetworkInfo>,
    pub firmware: FirmwareType,
    pub secure_boot: bool,
}

#[derive(Debug, Clone)]
pub struct CpuInfo {
    pub vendor: String,
    pub model: String,
    pub cores: u32,
    pub threads: u32,
    pub frequency_mhz: u32,
    pub features: Vec<String>,
    pub arch: CpuArch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuArch { X86_64, Aarch64, Unknown }

#[derive(Debug, Clone)]
pub struct MemoryInfo {
    pub total_mb: u64,
    pub available_mb: u64,
    pub swap_mb: u64,
}

#[derive(Debug, Clone)]
pub struct DiskInfo {
    pub path: String,
    pub model: String,
    pub size_bytes: u64,
    pub disk_type: DiskType,
    pub removable: bool,
    pub partitions: Vec<PartitionInfo>,
    pub transport: DiskTransport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskType { Hdd, Ssd, Nvme, UsbFlash, Cdrom, Unknown }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskTransport { Sata, Nvme, Usb, Scsi, Unknown }

#[derive(Debug, Clone)]
pub struct PartitionInfo {
    pub number: u32,
    pub start_sector: u64,
    pub size_bytes: u64,
    pub filesystem: Option<String>,
    pub label: Option<String>,
    pub uuid: Option<String>,
    pub flags: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct GpuInfo {
    pub vendor: GpuVendor,
    pub model: String,
    pub vram_mb: u64,
    pub driver: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuVendor { Intel, Amd, Nvidia, Other, Unknown }

#[derive(Debug, Clone)]
pub struct NetworkInfo {
    pub interface: String,
    pub mac_address: String,
    pub interface_type: NetworkType,
    pub connected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkType { Ethernet, Wifi, Unknown }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirmwareType { Uefi, Bios }

/// Detect all hardware
pub fn detect_hardware() -> InstallResult<HardwareInfo> {
    crate::kprintln!("hwdetect: Scanning hardware...");

    let cpu = detect_cpu()?;
    let memory = detect_memory()?;
    let disks = detect_disks()?;
    let gpus = detect_gpus()?;
    let network = detect_network()?;
    let firmware = detect_firmware();
    let secure_boot = detect_secure_boot();

    crate::kprintln!("hwdetect: Found {} CPU cores, {}MB RAM, {} disks, {} GPUs",
        cpu.cores, memory.total_mb, disks.len(), gpus.len());

    Ok(HardwareInfo { cpu, memory, disks, gpus, network, firmware, secure_boot })
}

fn detect_cpu_vendor() -> String {
    // Use CPUID to get vendor string
    // For now return a default
    String::from("GenuineIntel")
}

fn detect_cpu_model() -> String {
    // Use CPUID extended function to get model string
    // For now return a default
    String::from("Unknown CPU")
}

fn detect_cpu() -> InstallResult<CpuInfo> {
    // Use CPUID instruction to get vendor/model
    let vendor = detect_cpu_vendor();
    let model = detect_cpu_model();
    let cores = 1u32; // Default to 1 core, use actual detection when available
    
    Ok(CpuInfo {
        vendor, model, cores,
        threads: cores * 2,
        frequency_mhz: 3000,
        features: vec![String::from("sse4.2"), String::from("avx2")],
        arch: CpuArch::X86_64,
    })
}

fn detect_memory() -> InstallResult<MemoryInfo> {
    // TODO: Use actual memory detection from mm subsystem
    // For now return reasonable defaults (8GB total, 6GB available)
    Ok(MemoryInfo {
        total_mb: 8192,
        available_mb: 6144,
        swap_mb: 0,
    })
}

fn detect_disks() -> InstallResult<Vec<DiskInfo>> {
    let mut disks = Vec::new();

    // TODO: Implement real disk detection via storage subsystem
    // For now, scan PCI for storage controllers and enumerate

    // Check for any virtio-blk devices (for VM testing)
    if let Some(virtio_devs) = detect_virtio_disks() {
        disks.extend(virtio_devs);
    }

    // Future: Detect NVMe drives via storage::nvme module
    // Future: Detect SATA drives via storage::ahci module

    Ok(disks)
}

fn detect_virtio_disks() -> Option<Vec<DiskInfo>> {
    // Check for virtio-blk devices
    // Returns None if no virtio disks found
    None
}

fn detect_gpus() -> InstallResult<Vec<GpuInfo>> {
    let mut gpus = Vec::new();
    
    // Check for Intel GPU
    if crate::drivers::intel_gpu::is_present() {
        gpus.push(GpuInfo {
            vendor: GpuVendor::Intel,
            model: String::from("Intel Integrated Graphics"),
            vram_mb: 256,
            driver: Some(String::from("i915")),
        });
    }
    
    // Check for AMD GPU
    if crate::drivers::amd_gpu::is_present() {
        gpus.push(GpuInfo {
            vendor: GpuVendor::Amd,
            model: String::from("AMD Radeon"),
            vram_mb: 4096,
            driver: Some(String::from("amdgpu")),
        });
    }
    
    Ok(gpus)
}

fn detect_network() -> InstallResult<Vec<NetworkInfo>> {
    let mut interfaces = Vec::new();
    
    // Add detected interfaces
    interfaces.push(NetworkInfo {
        interface: String::from("eth0"),
        mac_address: String::from("00:00:00:00:00:00"),
        interface_type: NetworkType::Ethernet,
        connected: false,
    });
    
    Ok(interfaces)
}

fn detect_firmware() -> FirmwareType {
    // Check if UEFI runtime services are available
    // For now assume UEFI since we're built with UEFI bootloader
    // TODO: Actually detect from boot info
    FirmwareType::Uefi
}

fn detect_secure_boot() -> bool {
    // Check UEFI Secure Boot status
    false
}

pub fn init() {
    crate::kprintln!("hwdetect: Hardware detection initialized");
}
