//! VirtIO Drivers
//!
//! Provides optimized VirtIO drivers for virtualized environments.

#![allow(dead_code)]

pub mod virtqueue;
pub mod block;
pub mod net;
pub mod console;
pub mod balloon;
pub mod gpu;
pub mod input;

use alloc::vec::Vec;
use alloc::string::String;

use crate::sync::IrqSafeMutex;

/// VirtIO device type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtioDeviceType {
    Network = 1,
    Block = 2,
    Console = 3,
    Entropy = 4,
    Balloon = 5,
    Scsi = 8,
    Gpu = 16,
    Input = 18,
    Vsock = 19,
    Crypto = 20,
    Fs = 26,
}

impl VirtioDeviceType {
    pub fn from_id(id: u32) -> Option<Self> {
        match id {
            1 => Some(Self::Network),
            2 => Some(Self::Block),
            3 => Some(Self::Console),
            4 => Some(Self::Entropy),
            5 => Some(Self::Balloon),
            8 => Some(Self::Scsi),
            16 => Some(Self::Gpu),
            18 => Some(Self::Input),
            19 => Some(Self::Vsock),
            20 => Some(Self::Crypto),
            26 => Some(Self::Fs),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Network => "virtio-net",
            Self::Block => "virtio-blk",
            Self::Console => "virtio-console",
            Self::Entropy => "virtio-rng",
            Self::Balloon => "virtio-balloon",
            Self::Scsi => "virtio-scsi",
            Self::Gpu => "virtio-gpu",
            Self::Input => "virtio-input",
            Self::Vsock => "virtio-vsock",
            Self::Crypto => "virtio-crypto",
            Self::Fs => "virtio-fs",
        }
    }
}

/// VirtIO device status flags
#[derive(Debug, Clone, Copy)]
pub struct VirtioStatus(u8);

impl VirtioStatus {
    pub const ACKNOWLEDGE: u8 = 1;
    pub const DRIVER: u8 = 2;
    pub const DRIVER_OK: u8 = 4;
    pub const FEATURES_OK: u8 = 8;
    pub const DEVICE_NEEDS_RESET: u8 = 64;
    pub const FAILED: u8 = 128;

    pub fn new() -> Self {
        Self(0)
    }

    pub fn set(&mut self, flag: u8) {
        self.0 |= flag;
    }

    pub fn clear(&mut self, flag: u8) {
        self.0 &= !flag;
    }

    pub fn has(&self, flag: u8) -> bool {
        self.0 & flag != 0
    }

    pub fn value(&self) -> u8 {
        self.0
    }
}

impl Default for VirtioStatus {
    fn default() -> Self {
        Self::new()
    }
}

/// VirtIO common feature bits
pub mod features {
    pub const VIRTIO_F_RING_INDIRECT_DESC: u64 = 1 << 28;
    pub const VIRTIO_F_RING_EVENT_IDX: u64 = 1 << 29;
    pub const VIRTIO_F_VERSION_1: u64 = 1 << 32;
    pub const VIRTIO_F_ACCESS_PLATFORM: u64 = 1 << 33;
    pub const VIRTIO_F_RING_PACKED: u64 = 1 << 34;
    pub const VIRTIO_F_IN_ORDER: u64 = 1 << 35;
    pub const VIRTIO_F_ORDER_PLATFORM: u64 = 1 << 36;
    pub const VIRTIO_F_SR_IOV: u64 = 1 << 37;
    pub const VIRTIO_F_NOTIFICATION_DATA: u64 = 1 << 38;
}

/// VirtIO transport type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtioTransport {
    Pci,
    Mmio,
    ChannelIo,
}

/// VirtIO device info
#[derive(Debug, Clone)]
pub struct VirtioDeviceInfo {
    pub device_type: VirtioDeviceType,
    pub transport: VirtioTransport,
    pub vendor_id: u32,
    pub device_id: u32,
    pub features: u64,
    pub num_queues: u32,
    pub status: u8,
}

/// VirtIO device trait
pub trait VirtioDevice: Send + Sync {
    fn device_type(&self) -> VirtioDeviceType;
    fn init(&mut self) -> Result<(), &'static str>;
    fn reset(&mut self);
    fn negotiate_features(&mut self, offered: u64) -> u64;
    fn activate(&mut self) -> Result<(), &'static str>;
    fn handle_interrupt(&mut self);
}

/// VirtIO manager
pub struct VirtioManager {
    devices: Vec<VirtioDeviceInfo>,
}

impl VirtioManager {
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
        }
    }

    /// Scan for VirtIO devices
    pub fn scan(&mut self) {
        // Scan PCI for VirtIO devices
        self.scan_pci();
        // Scan MMIO for VirtIO devices
        self.scan_mmio();
    }

    fn scan_pci(&mut self) {
        // VirtIO PCI vendor ID
        const VIRTIO_PCI_VENDOR: u16 = 0x1AF4;
        // VirtIO device IDs range: 0x1000-0x107F (transitional), 0x1040-0x107F (modern)

        // In real implementation, iterate PCI devices
        // For now, just log
        crate::kprintln!("virtio: Scanning PCI bus for VirtIO devices");
    }

    fn scan_mmio(&mut self) {
        // MMIO VirtIO devices are platform-specific
        // Usually provided via device tree or ACPI
        crate::kprintln!("virtio: Scanning MMIO for VirtIO devices");
    }

    /// Register a detected device
    pub fn register_device(&mut self, info: VirtioDeviceInfo) {
        crate::kprintln!("virtio: Found {} device", info.device_type.name());
        self.devices.push(info);
    }

    /// Get device count
    pub fn device_count(&self) -> usize {
        self.devices.len()
    }

    /// Get devices
    pub fn devices(&self) -> &[VirtioDeviceInfo] {
        &self.devices
    }

    /// Format status
    pub fn format_status(&self) -> String {
        alloc::format!("VirtIO: {} devices detected", self.devices.len())
    }
}

impl Default for VirtioManager {
    fn default() -> Self {
        Self::new()
    }
}

// Global VirtIO manager
static VIRTIO: IrqSafeMutex<Option<VirtioManager>> = IrqSafeMutex::new(None);

/// Initialize VirtIO subsystem
pub fn init() {
    let mut mgr = VirtioManager::new();
    mgr.scan();

    let count = mgr.device_count();
    *VIRTIO.lock() = Some(mgr);

    crate::kprintln!("virtio: Initialized with {} devices", count);
}

/// Get device count
pub fn device_count() -> usize {
    VIRTIO.lock().as_ref().map(|m| m.device_count()).unwrap_or(0)
}

/// Get status
pub fn status() -> String {
    VIRTIO.lock().as_ref()
        .map(|m| m.format_status())
        .unwrap_or_else(|| "VirtIO not initialized".into())
}
