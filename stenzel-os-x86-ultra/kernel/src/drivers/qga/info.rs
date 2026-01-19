//! QGA System Information
//!
//! System information gathering for QEMU Guest Agent.

#![allow(dead_code)]

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use alloc::collections::BTreeMap;

use super::JsonValue;

/// OS information
#[derive(Debug, Clone)]
pub struct OsInfo {
    pub id: String,
    pub name: String,
    pub pretty_name: String,
    pub version: String,
    pub version_id: String,
    pub machine: String,
    pub kernel_release: String,
    pub kernel_version: String,
    pub variant: Option<String>,
    pub variant_id: Option<String>,
}

impl OsInfo {
    /// Get OS information
    pub fn gather() -> Self {
        Self {
            id: "stenzel".to_string(),
            name: "Stenzel OS".to_string(),
            pretty_name: "Stenzel OS 1.0".to_string(),
            version: "1.0".to_string(),
            version_id: "1.0".to_string(),
            machine: "x86_64".to_string(),
            kernel_release: "1.0.0-stenzel".to_string(),
            kernel_version: "Stenzel OS Kernel".to_string(),
            variant: None,
            variant_id: None,
        }
    }

    /// Convert to JSON
    pub fn to_json(&self) -> JsonValue {
        let mut obj = BTreeMap::new();
        obj.insert("id".to_string(), JsonValue::String(self.id.clone()));
        obj.insert("name".to_string(), JsonValue::String(self.name.clone()));
        obj.insert("pretty-name".to_string(), JsonValue::String(self.pretty_name.clone()));
        obj.insert("version".to_string(), JsonValue::String(self.version.clone()));
        obj.insert("version-id".to_string(), JsonValue::String(self.version_id.clone()));
        obj.insert("machine".to_string(), JsonValue::String(self.machine.clone()));
        obj.insert("kernel-release".to_string(), JsonValue::String(self.kernel_release.clone()));
        obj.insert("kernel-version".to_string(), JsonValue::String(self.kernel_version.clone()));

        if let Some(ref variant) = self.variant {
            obj.insert("variant".to_string(), JsonValue::String(variant.clone()));
        }
        if let Some(ref variant_id) = self.variant_id {
            obj.insert("variant-id".to_string(), JsonValue::String(variant_id.clone()));
        }

        JsonValue::Object(obj)
    }
}

/// Timezone information
#[derive(Debug, Clone)]
pub struct TimezoneInfo {
    pub zone: String,
    pub offset: i32,
}

impl TimezoneInfo {
    /// Get timezone information
    pub fn gather() -> Self {
        Self {
            zone: "UTC".to_string(),
            offset: 0,
        }
    }

    /// Convert to JSON
    pub fn to_json(&self) -> JsonValue {
        let mut obj = BTreeMap::new();
        obj.insert("zone".to_string(), JsonValue::String(self.zone.clone()));
        obj.insert("offset".to_string(), JsonValue::Number(self.offset as i64));
        JsonValue::Object(obj)
    }
}

/// Host name information
#[derive(Debug, Clone)]
pub struct HostNameInfo {
    pub host_name: String,
}

impl HostNameInfo {
    /// Get hostname
    pub fn gather() -> Self {
        Self {
            host_name: "stenzel".to_string(),
        }
    }

    /// Convert to JSON
    pub fn to_json(&self) -> JsonValue {
        let mut obj = BTreeMap::new();
        obj.insert("host-name".to_string(), JsonValue::String(self.host_name.clone()));
        JsonValue::Object(obj)
    }
}

/// vCPU information
#[derive(Debug, Clone)]
pub struct VcpuInfo {
    pub logical_id: u32,
    pub online: bool,
    pub can_offline: bool,
}

impl VcpuInfo {
    /// Convert to JSON
    pub fn to_json(&self) -> JsonValue {
        let mut obj = BTreeMap::new();
        obj.insert("logical-id".to_string(), JsonValue::Number(self.logical_id as i64));
        obj.insert("online".to_string(), JsonValue::Bool(self.online));
        obj.insert("can-offline".to_string(), JsonValue::Bool(self.can_offline));
        JsonValue::Object(obj)
    }
}

/// Get vCPU list
pub fn get_vcpus() -> Vec<VcpuInfo> {
    // TODO: Get actual CPU count from SMP
    vec![VcpuInfo {
        logical_id: 0,
        online: true,
        can_offline: false,
    }]
}

/// Filesystem info
#[derive(Debug, Clone)]
pub struct FsInfo {
    pub name: String,
    pub mountpoint: String,
    pub fstype: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub disk: Vec<DiskInfo>,
}

impl FsInfo {
    /// Convert to JSON
    pub fn to_json(&self) -> JsonValue {
        let mut obj = BTreeMap::new();
        obj.insert("name".to_string(), JsonValue::String(self.name.clone()));
        obj.insert("mountpoint".to_string(), JsonValue::String(self.mountpoint.clone()));
        obj.insert("type".to_string(), JsonValue::String(self.fstype.clone()));
        obj.insert("total-bytes".to_string(), JsonValue::Number(self.total_bytes as i64));
        obj.insert("used-bytes".to_string(), JsonValue::Number(self.used_bytes as i64));

        let disk_json: Vec<JsonValue> = self.disk.iter().map(|d| d.to_json()).collect();
        obj.insert("disk".to_string(), JsonValue::Array(disk_json));

        JsonValue::Object(obj)
    }
}

/// Disk info for filesystem
#[derive(Debug, Clone)]
pub struct DiskInfo {
    pub dev: String,
    pub bus_type: String,
    pub bus: u32,
    pub target: u32,
    pub unit: u32,
}

impl DiskInfo {
    /// Convert to JSON
    pub fn to_json(&self) -> JsonValue {
        let mut obj = BTreeMap::new();
        obj.insert("dev".to_string(), JsonValue::String(self.dev.clone()));
        obj.insert("bus-type".to_string(), JsonValue::String(self.bus_type.clone()));
        obj.insert("bus".to_string(), JsonValue::Number(self.bus as i64));
        obj.insert("target".to_string(), JsonValue::Number(self.target as i64));
        obj.insert("unit".to_string(), JsonValue::Number(self.unit as i64));
        JsonValue::Object(obj)
    }
}

/// Get filesystem list
pub fn get_fsinfo() -> Vec<FsInfo> {
    vec![
        FsInfo {
            name: "rootfs".to_string(),
            mountpoint: "/".to_string(),
            fstype: "ext4".to_string(),
            total_bytes: 10737418240, // 10GB
            used_bytes: 2147483648,   // 2GB
            disk: vec![DiskInfo {
                dev: "/dev/vda1".to_string(),
                bus_type: "virtio".to_string(),
                bus: 0,
                target: 0,
                unit: 0,
            }],
        },
        FsInfo {
            name: "tmpfs".to_string(),
            mountpoint: "/tmp".to_string(),
            fstype: "tmpfs".to_string(),
            total_bytes: 536870912, // 512MB
            used_bytes: 0,
            disk: vec![],
        },
    ]
}

/// Network interface info
#[derive(Debug, Clone)]
pub struct NetworkInterface {
    pub name: String,
    pub hardware_address: String,
    pub ip_addresses: Vec<IpAddress>,
    pub statistics: Option<NetworkStats>,
}

impl NetworkInterface {
    /// Convert to JSON
    pub fn to_json(&self) -> JsonValue {
        let mut obj = BTreeMap::new();
        obj.insert("name".to_string(), JsonValue::String(self.name.clone()));
        obj.insert("hardware-address".to_string(), JsonValue::String(self.hardware_address.clone()));

        let ip_json: Vec<JsonValue> = self.ip_addresses.iter().map(|ip| ip.to_json()).collect();
        obj.insert("ip-addresses".to_string(), JsonValue::Array(ip_json));

        if let Some(ref stats) = self.statistics {
            obj.insert("statistics".to_string(), stats.to_json());
        }

        JsonValue::Object(obj)
    }
}

/// IP address info
#[derive(Debug, Clone)]
pub struct IpAddress {
    pub ip_address: String,
    pub ip_address_type: String,
    pub prefix: u32,
}

impl IpAddress {
    /// Convert to JSON
    pub fn to_json(&self) -> JsonValue {
        let mut obj = BTreeMap::new();
        obj.insert("ip-address".to_string(), JsonValue::String(self.ip_address.clone()));
        obj.insert("ip-address-type".to_string(), JsonValue::String(self.ip_address_type.clone()));
        obj.insert("prefix".to_string(), JsonValue::Number(self.prefix as i64));
        JsonValue::Object(obj)
    }
}

/// Network statistics
#[derive(Debug, Clone)]
pub struct NetworkStats {
    pub rx_bytes: u64,
    pub rx_packets: u64,
    pub rx_errs: u64,
    pub rx_dropped: u64,
    pub tx_bytes: u64,
    pub tx_packets: u64,
    pub tx_errs: u64,
    pub tx_dropped: u64,
}

impl NetworkStats {
    /// Convert to JSON
    pub fn to_json(&self) -> JsonValue {
        let mut obj = BTreeMap::new();
        obj.insert("rx-bytes".to_string(), JsonValue::Number(self.rx_bytes as i64));
        obj.insert("rx-packets".to_string(), JsonValue::Number(self.rx_packets as i64));
        obj.insert("rx-errs".to_string(), JsonValue::Number(self.rx_errs as i64));
        obj.insert("rx-dropped".to_string(), JsonValue::Number(self.rx_dropped as i64));
        obj.insert("tx-bytes".to_string(), JsonValue::Number(self.tx_bytes as i64));
        obj.insert("tx-packets".to_string(), JsonValue::Number(self.tx_packets as i64));
        obj.insert("tx-errs".to_string(), JsonValue::Number(self.tx_errs as i64));
        obj.insert("tx-dropped".to_string(), JsonValue::Number(self.tx_dropped as i64));
        JsonValue::Object(obj)
    }
}

/// Get network interfaces
pub fn get_network_interfaces() -> Vec<NetworkInterface> {
    vec![
        NetworkInterface {
            name: "lo".to_string(),
            hardware_address: "00:00:00:00:00:00".to_string(),
            ip_addresses: vec![
                IpAddress {
                    ip_address: "127.0.0.1".to_string(),
                    ip_address_type: "ipv4".to_string(),
                    prefix: 8,
                },
                IpAddress {
                    ip_address: "::1".to_string(),
                    ip_address_type: "ipv6".to_string(),
                    prefix: 128,
                },
            ],
            statistics: Some(NetworkStats {
                rx_bytes: 0,
                rx_packets: 0,
                rx_errs: 0,
                rx_dropped: 0,
                tx_bytes: 0,
                tx_packets: 0,
                tx_errs: 0,
                tx_dropped: 0,
            }),
        },
        NetworkInterface {
            name: "eth0".to_string(),
            hardware_address: "52:54:00:12:34:56".to_string(),
            ip_addresses: vec![IpAddress {
                ip_address: "10.0.2.15".to_string(),
                ip_address_type: "ipv4".to_string(),
                prefix: 24,
            }],
            statistics: Some(NetworkStats {
                rx_bytes: 1024,
                rx_packets: 10,
                rx_errs: 0,
                rx_dropped: 0,
                tx_bytes: 512,
                tx_packets: 5,
                tx_errs: 0,
                tx_dropped: 0,
            }),
        },
    ]
}

/// Memory information
#[derive(Debug, Clone)]
pub struct MemoryInfo {
    pub total: u64,
    pub free: u64,
    pub available: u64,
    pub buffers: u64,
    pub cached: u64,
    pub swap_total: u64,
    pub swap_free: u64,
}

impl MemoryInfo {
    /// Gather memory info
    pub fn gather() -> Self {
        // TODO: Get actual memory info
        Self {
            total: 4294967296,      // 4GB
            free: 2147483648,       // 2GB
            available: 3221225472,  // 3GB
            buffers: 134217728,     // 128MB
            cached: 536870912,      // 512MB
            swap_total: 2147483648, // 2GB
            swap_free: 2147483648,  // 2GB
        }
    }

    /// Convert to JSON
    pub fn to_json(&self) -> JsonValue {
        let mut obj = BTreeMap::new();
        obj.insert("total".to_string(), JsonValue::Number(self.total as i64));
        obj.insert("free".to_string(), JsonValue::Number(self.free as i64));
        obj.insert("available".to_string(), JsonValue::Number(self.available as i64));
        obj.insert("buffers".to_string(), JsonValue::Number(self.buffers as i64));
        obj.insert("cached".to_string(), JsonValue::Number(self.cached as i64));
        obj.insert("swap-total".to_string(), JsonValue::Number(self.swap_total as i64));
        obj.insert("swap-free".to_string(), JsonValue::Number(self.swap_free as i64));
        JsonValue::Object(obj)
    }
}

/// Uptime information
#[derive(Debug, Clone)]
pub struct UptimeInfo {
    pub uptime_seconds: u64,
    pub idle_seconds: u64,
}

impl UptimeInfo {
    /// Gather uptime
    pub fn gather() -> Self {
        let ticks = crate::time::ticks();
        // Assume 1000 ticks per second
        Self {
            uptime_seconds: ticks / 1000,
            idle_seconds: 0, // TODO: Track idle time
        }
    }

    /// Convert to JSON
    pub fn to_json(&self) -> JsonValue {
        let mut obj = BTreeMap::new();
        obj.insert("uptime".to_string(), JsonValue::Number(self.uptime_seconds as i64));
        obj.insert("idle".to_string(), JsonValue::Number(self.idle_seconds as i64));
        JsonValue::Object(obj)
    }
}

/// Load average
#[derive(Debug, Clone)]
pub struct LoadAverage {
    pub load1: f64,
    pub load5: f64,
    pub load15: f64,
}

impl LoadAverage {
    /// Gather load average
    pub fn gather() -> Self {
        // TODO: Implement actual load tracking
        Self {
            load1: 0.0,
            load5: 0.0,
            load15: 0.0,
        }
    }

    /// Convert to JSON
    pub fn to_json(&self) -> JsonValue {
        let mut obj = BTreeMap::new();
        obj.insert("load1".to_string(), JsonValue::Float(self.load1));
        obj.insert("load5".to_string(), JsonValue::Float(self.load5));
        obj.insert("load15".to_string(), JsonValue::Float(self.load15));
        JsonValue::Object(obj)
    }
}

/// Process information
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cmdline: String,
    pub user: String,
    pub state: char,
}

impl ProcessInfo {
    /// Convert to JSON
    pub fn to_json(&self) -> JsonValue {
        let mut obj = BTreeMap::new();
        obj.insert("pid".to_string(), JsonValue::Number(self.pid as i64));
        obj.insert("name".to_string(), JsonValue::String(self.name.clone()));
        obj.insert("cmdline".to_string(), JsonValue::String(self.cmdline.clone()));
        obj.insert("user".to_string(), JsonValue::String(self.user.clone()));
        obj.insert("state".to_string(), JsonValue::String(self.state.to_string()));
        JsonValue::Object(obj)
    }
}

/// Get process list
pub fn get_processes() -> Vec<ProcessInfo> {
    // Return mock process list
    vec![
        ProcessInfo {
            pid: 1,
            name: "init".to_string(),
            cmdline: "/sbin/init".to_string(),
            user: "root".to_string(),
            state: 'S',
        },
        ProcessInfo {
            pid: 2,
            name: "kthreadd".to_string(),
            cmdline: "[kthreadd]".to_string(),
            user: "root".to_string(),
            state: 'S',
        },
    ]
}

/// CPU information
#[derive(Debug, Clone)]
pub struct CpuInfo {
    pub vendor_id: String,
    pub model_name: String,
    pub cpu_mhz: f64,
    pub cache_size: u32,
    pub cpu_cores: u32,
}

impl CpuInfo {
    /// Gather CPU info
    pub fn gather() -> Self {
        // TODO: Read from CPUID
        Self {
            vendor_id: "GenuineIntel".to_string(),
            model_name: "QEMU Virtual CPU".to_string(),
            cpu_mhz: 2400.0,
            cache_size: 4096,
            cpu_cores: 1,
        }
    }

    /// Convert to JSON
    pub fn to_json(&self) -> JsonValue {
        let mut obj = BTreeMap::new();
        obj.insert("vendor-id".to_string(), JsonValue::String(self.vendor_id.clone()));
        obj.insert("model-name".to_string(), JsonValue::String(self.model_name.clone()));
        obj.insert("cpu-mhz".to_string(), JsonValue::Float(self.cpu_mhz));
        obj.insert("cache-size".to_string(), JsonValue::Number(self.cache_size as i64));
        obj.insert("cpu-cores".to_string(), JsonValue::Number(self.cpu_cores as i64));
        JsonValue::Object(obj)
    }
}

/// Gather all system information
pub fn gather_all_info() -> JsonValue {
    let mut info = BTreeMap::new();

    info.insert("os".to_string(), OsInfo::gather().to_json());
    info.insert("hostname".to_string(), HostNameInfo::gather().to_json());
    info.insert("timezone".to_string(), TimezoneInfo::gather().to_json());
    info.insert("memory".to_string(), MemoryInfo::gather().to_json());
    info.insert("uptime".to_string(), UptimeInfo::gather().to_json());
    info.insert("loadavg".to_string(), LoadAverage::gather().to_json());
    info.insert("cpu".to_string(), CpuInfo::gather().to_json());

    let vcpus: Vec<JsonValue> = get_vcpus().iter().map(|v| v.to_json()).collect();
    info.insert("vcpus".to_string(), JsonValue::Array(vcpus));

    let fsinfo: Vec<JsonValue> = get_fsinfo().iter().map(|f| f.to_json()).collect();
    info.insert("filesystems".to_string(), JsonValue::Array(fsinfo));

    let netifs: Vec<JsonValue> = get_network_interfaces().iter().map(|n| n.to_json()).collect();
    info.insert("network-interfaces".to_string(), JsonValue::Array(netifs));

    JsonValue::Object(info)
}
