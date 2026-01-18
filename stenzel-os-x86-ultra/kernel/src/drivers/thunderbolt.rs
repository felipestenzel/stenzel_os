//! Thunderbolt/USB4 Controller Driver
//!
//! Implements support for Thunderbolt 3/4 and USB4 controllers with:
//! - Controller detection (Intel Alpine Ridge, Titan Ridge, Maple Ridge, AMD)
//! - Device enumeration and connection manager
//! - Security levels (none, user, secure, dponly)
//! - Hot-plug support via NHI (Native Host Interface)
//! - PCIe tunneling
//! - DisplayPort tunneling
//! - USB tunneling

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use crate::drivers::pci::{self, PciDevice};

/// Thunderbolt device ID
pub type ThunderboltId = u64;

/// Global device ID counter
static NEXT_DEVICE_ID: AtomicU64 = AtomicU64::new(1);

fn next_device_id() -> ThunderboltId {
    NEXT_DEVICE_ID.fetch_add(1, Ordering::Relaxed)
}

/// Intel vendor ID
const INTEL_VENDOR_ID: u16 = 0x8086;

/// AMD vendor ID
const AMD_VENDOR_ID: u16 = 0x1022;

/// Device IDs for Thunderbolt controllers
pub mod device_ids {
    // Intel Alpine Ridge (Thunderbolt 3, 1st gen)
    pub const ALPINE_RIDGE_LP: u16 = 0x156C;
    pub const ALPINE_RIDGE_2C: u16 = 0x156D;
    pub const ALPINE_RIDGE_4C: u16 = 0x1575;
    pub const ALPINE_RIDGE_C_2C: u16 = 0x1576;
    pub const ALPINE_RIDGE_C_4C: u16 = 0x1577;

    // Intel Titan Ridge (Thunderbolt 3, 2nd gen)
    pub const TITAN_RIDGE_2C: u16 = 0x15D2;
    pub const TITAN_RIDGE_4C: u16 = 0x15D9;
    pub const TITAN_RIDGE_LP: u16 = 0x15DA;

    // Intel Ice Lake Thunderbolt
    pub const ICE_LAKE_TB: u16 = 0x8A0D;
    pub const ICE_LAKE_TB2: u16 = 0x8A17;

    // Intel Tiger Lake (Thunderbolt 4)
    pub const TIGER_LAKE_TB: u16 = 0x9A1B;
    pub const TIGER_LAKE_TB_H: u16 = 0x9A1D;

    // Intel Maple Ridge (Thunderbolt 4, discrete)
    pub const MAPLE_RIDGE_2C: u16 = 0x1137;
    pub const MAPLE_RIDGE_4C: u16 = 0x1136;

    // Intel Alder Lake (Thunderbolt 4)
    pub const ALDER_LAKE_TB: u16 = 0x463E;
    pub const ALDER_LAKE_TB_H: u16 = 0x466D;

    // Intel Raptor Lake
    pub const RAPTOR_LAKE_TB: u16 = 0xA73E;

    // Intel Meteor Lake (Thunderbolt 5)
    pub const METEOR_LAKE_TB: u16 = 0x7EC4;

    // AMD USB4/Thunderbolt
    pub const AMD_USB4_0: u16 = 0x162E;
    pub const AMD_USB4_1: u16 = 0x162F;
}

/// Controller generation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThunderboltGeneration {
    /// Thunderbolt 1 (Light Peak - very old, not commonly supported)
    Thunderbolt1,
    /// Thunderbolt 2 (Falcon Ridge)
    Thunderbolt2,
    /// Thunderbolt 3 (Alpine Ridge, Titan Ridge)
    Thunderbolt3,
    /// Thunderbolt 4 (Tiger Lake, Maple Ridge)
    Thunderbolt4,
    /// Thunderbolt 5 (Meteor Lake)
    Thunderbolt5,
    /// USB4 (compatible with Thunderbolt 3/4)
    Usb4,
    /// Unknown generation
    Unknown,
}

impl ThunderboltGeneration {
    pub fn from_device_id(vendor: u16, device: u16) -> Self {
        if vendor == INTEL_VENDOR_ID {
            match device {
                device_ids::ALPINE_RIDGE_LP
                | device_ids::ALPINE_RIDGE_2C
                | device_ids::ALPINE_RIDGE_4C
                | device_ids::ALPINE_RIDGE_C_2C
                | device_ids::ALPINE_RIDGE_C_4C
                | device_ids::TITAN_RIDGE_2C
                | device_ids::TITAN_RIDGE_4C
                | device_ids::TITAN_RIDGE_LP => ThunderboltGeneration::Thunderbolt3,

                device_ids::ICE_LAKE_TB
                | device_ids::ICE_LAKE_TB2
                | device_ids::TIGER_LAKE_TB
                | device_ids::TIGER_LAKE_TB_H
                | device_ids::MAPLE_RIDGE_2C
                | device_ids::MAPLE_RIDGE_4C
                | device_ids::ALDER_LAKE_TB
                | device_ids::ALDER_LAKE_TB_H
                | device_ids::RAPTOR_LAKE_TB => ThunderboltGeneration::Thunderbolt4,

                device_ids::METEOR_LAKE_TB => ThunderboltGeneration::Thunderbolt5,

                _ => ThunderboltGeneration::Unknown,
            }
        } else if vendor == AMD_VENDOR_ID {
            match device {
                device_ids::AMD_USB4_0 | device_ids::AMD_USB4_1 => ThunderboltGeneration::Usb4,
                _ => ThunderboltGeneration::Unknown,
            }
        } else {
            ThunderboltGeneration::Unknown
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            ThunderboltGeneration::Thunderbolt1 => "Thunderbolt 1",
            ThunderboltGeneration::Thunderbolt2 => "Thunderbolt 2",
            ThunderboltGeneration::Thunderbolt3 => "Thunderbolt 3",
            ThunderboltGeneration::Thunderbolt4 => "Thunderbolt 4",
            ThunderboltGeneration::Thunderbolt5 => "Thunderbolt 5",
            ThunderboltGeneration::Usb4 => "USB4",
            ThunderboltGeneration::Unknown => "Unknown",
        }
    }

    pub fn max_speed_gbps(&self) -> u32 {
        match self {
            ThunderboltGeneration::Thunderbolt1 => 10,
            ThunderboltGeneration::Thunderbolt2 => 20,
            ThunderboltGeneration::Thunderbolt3 => 40,
            ThunderboltGeneration::Thunderbolt4 => 40,
            ThunderboltGeneration::Thunderbolt5 => 80, // Up to 120 Gbps with bonding
            ThunderboltGeneration::Usb4 => 40,        // USB4 v1 = 40 Gbps, v2 = 80 Gbps
            ThunderboltGeneration::Unknown => 0,
        }
    }
}

/// Security level for Thunderbolt
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityLevel {
    /// No security (all devices allowed)
    None,
    /// User approval required for new devices
    User,
    /// Secure connect (cryptographic verification)
    Secure,
    /// DisplayPort only (no PCIe tunneling)
    DpOnly,
    /// USB only (no PCIe/DP tunneling)
    UsbOnly,
    /// Unknown/not configured
    Unknown,
}

impl SecurityLevel {
    pub fn name(&self) -> &'static str {
        match self {
            SecurityLevel::None => "none",
            SecurityLevel::User => "user",
            SecurityLevel::Secure => "secure",
            SecurityLevel::DpOnly => "dponly",
            SecurityLevel::UsbOnly => "usbonly",
            SecurityLevel::Unknown => "unknown",
        }
    }

    pub fn allows_pcie(&self) -> bool {
        matches!(self, SecurityLevel::None | SecurityLevel::User | SecurityLevel::Secure)
    }

    pub fn allows_displayport(&self) -> bool {
        !matches!(self, SecurityLevel::UsbOnly)
    }

    pub fn requires_approval(&self) -> bool {
        matches!(self, SecurityLevel::User | SecurityLevel::Secure)
    }
}

/// Tunnel type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunnelType {
    /// PCIe tunneling (for external GPUs, storage, etc.)
    Pcie,
    /// DisplayPort tunneling (for monitors)
    DisplayPort,
    /// USB tunneling (for USB devices)
    Usb,
    /// DMA/Thunderbolt Networking
    Dma,
}

impl TunnelType {
    pub fn name(&self) -> &'static str {
        match self {
            TunnelType::Pcie => "PCIe",
            TunnelType::DisplayPort => "DisplayPort",
            TunnelType::Usb => "USB",
            TunnelType::Dma => "DMA",
        }
    }
}

/// Tunnel state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunnelState {
    /// Tunnel not established
    Inactive,
    /// Tunnel being established
    Activating,
    /// Tunnel active
    Active,
    /// Tunnel being deactivated
    Deactivating,
    /// Tunnel error
    Error,
}

/// A Thunderbolt tunnel
#[derive(Debug, Clone)]
pub struct Tunnel {
    pub id: u32,
    pub tunnel_type: TunnelType,
    pub state: TunnelState,
    pub source_port: u8,
    pub dest_port: u8,
    pub bandwidth_gbps: u32,
}

impl Tunnel {
    pub fn new(id: u32, tunnel_type: TunnelType, source: u8, dest: u8) -> Self {
        Self {
            id,
            tunnel_type,
            state: TunnelState::Inactive,
            source_port: source,
            dest_port: dest,
            bandwidth_gbps: 0,
        }
    }
}

/// Device type connected via Thunderbolt
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    /// Thunderbolt dock
    Dock,
    /// eGPU enclosure
    Egpu,
    /// External storage
    Storage,
    /// Display/monitor
    Display,
    /// Hub/switch
    Hub,
    /// Thunderbolt-to-PCIe adapter
    PcieAdapter,
    /// Networking device (Thunderbolt Bridge)
    Network,
    /// Generic device
    Generic,
    /// Unknown device type
    Unknown,
}

impl DeviceType {
    pub fn name(&self) -> &'static str {
        match self {
            DeviceType::Dock => "Dock",
            DeviceType::Egpu => "eGPU",
            DeviceType::Storage => "Storage",
            DeviceType::Display => "Display",
            DeviceType::Hub => "Hub",
            DeviceType::PcieAdapter => "PCIe Adapter",
            DeviceType::Network => "Network",
            DeviceType::Generic => "Generic",
            DeviceType::Unknown => "Unknown",
        }
    }
}

/// Connection state of a device
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Device disconnected
    Disconnected,
    /// Device connected, waiting for authorization
    PendingAuthorization,
    /// Device authorized, being configured
    Authorizing,
    /// Device connected and authorized
    Connected,
    /// Device suspended
    Suspended,
    /// Device in error state
    Error,
}

/// A connected Thunderbolt device
#[derive(Clone)]
pub struct ThunderboltDevice {
    /// Unique device ID
    pub id: ThunderboltId,
    /// Device UUID (from DROM)
    pub uuid: [u8; 16],
    /// Vendor ID
    pub vendor_id: u16,
    /// Device ID
    pub device_id: u16,
    /// Vendor name
    pub vendor_name: String,
    /// Device name
    pub device_name: String,
    /// Device type
    pub device_type: DeviceType,
    /// Connection state
    pub state: ConnectionState,
    /// Thunderbolt generation
    pub generation: ThunderboltGeneration,
    /// Port number on controller
    pub port: u8,
    /// Route string (path through switches)
    pub route_string: u64,
    /// Is device authorized
    pub authorized: bool,
    /// Security key (for secure mode)
    pub security_key: Option<[u8; 32]>,
    /// Active tunnels
    pub tunnels: Vec<Tunnel>,
    /// Upstream device ID (for daisy-chained devices)
    pub upstream_id: Option<ThunderboltId>,
    /// Downstream device IDs
    pub downstream_ids: Vec<ThunderboltId>,
    /// Link speed in Gbps
    pub link_speed_gbps: u32,
    /// Number of lanes
    pub num_lanes: u8,
}

impl ThunderboltDevice {
    pub fn new(vendor_id: u16, device_id: u16, port: u8) -> Self {
        Self {
            id: next_device_id(),
            uuid: [0; 16],
            vendor_id,
            device_id,
            vendor_name: String::new(),
            device_name: String::new(),
            device_type: DeviceType::Unknown,
            state: ConnectionState::Disconnected,
            generation: ThunderboltGeneration::Unknown,
            port,
            route_string: 0,
            authorized: false,
            security_key: None,
            tunnels: Vec::new(),
            upstream_id: None,
            downstream_ids: Vec::new(),
            link_speed_gbps: 0,
            num_lanes: 0,
        }
    }

    pub fn is_connected(&self) -> bool {
        matches!(
            self.state,
            ConnectionState::Connected | ConnectionState::Suspended
        )
    }

    pub fn is_pending(&self) -> bool {
        matches!(self.state, ConnectionState::PendingAuthorization)
    }

    pub fn has_pcie_tunnel(&self) -> bool {
        self.tunnels.iter().any(|t| {
            t.tunnel_type == TunnelType::Pcie && t.state == TunnelState::Active
        })
    }

    pub fn has_dp_tunnel(&self) -> bool {
        self.tunnels.iter().any(|t| {
            t.tunnel_type == TunnelType::DisplayPort && t.state == TunnelState::Active
        })
    }
}

/// NHI (Native Host Interface) registers
pub mod nhi_regs {
    /// Transmit ring base address low
    pub const TX_RING_BASE_LO: u32 = 0x0000;
    /// Transmit ring base address high
    pub const TX_RING_BASE_HI: u32 = 0x0004;
    /// Transmit ring size
    pub const TX_RING_SIZE: u32 = 0x0008;
    /// Transmit ring head/tail
    pub const TX_RING_HEAD_TAIL: u32 = 0x000C;
    /// Receive ring base address low
    pub const RX_RING_BASE_LO: u32 = 0x1000;
    /// Receive ring base address high
    pub const RX_RING_BASE_HI: u32 = 0x1004;
    /// Receive ring size
    pub const RX_RING_SIZE: u32 = 0x1008;
    /// Receive ring head/tail
    pub const RX_RING_HEAD_TAIL: u32 = 0x100C;
    /// Interrupt status
    pub const INTR_STATUS: u32 = 0x0800;
    /// Interrupt mask
    pub const INTR_MASK: u32 = 0x0804;
    /// Control register
    pub const CONTROL: u32 = 0x0810;
    /// Security register
    pub const SECURITY: u32 = 0x0820;
    /// Firmware version
    pub const FW_VERSION: u32 = 0x0830;
    /// Port status
    pub const PORT_STATUS: u32 = 0x0840;
}

/// Control register bits
pub mod control_bits {
    pub const ENABLE: u32 = 1 << 0;
    pub const RESET: u32 = 1 << 1;
    pub const FORCE_POWER: u32 = 1 << 2;
    pub const HOT_PLUG_ENABLE: u32 = 1 << 3;
    pub const TUNNEL_PCIE: u32 = 1 << 4;
    pub const TUNNEL_DP: u32 = 1 << 5;
    pub const TUNNEL_USB: u32 = 1 << 6;
}

/// Port status bits
pub mod port_status_bits {
    pub const CONNECTED: u32 = 1 << 0;
    pub const READY: u32 = 1 << 1;
    pub const DEVICE_PRESENT: u32 = 1 << 2;
    pub const LINK_UP: u32 = 1 << 3;
    pub const SPEED_MASK: u32 = 0xF << 4;
    pub const LANES_MASK: u32 = 0x3 << 8;
}

/// Thunderbolt controller
pub struct ThunderboltController {
    /// PCI device info
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    /// Vendor ID
    pub vendor_id: u16,
    /// Device ID
    pub device_id: u16,
    /// Controller generation
    pub generation: ThunderboltGeneration,
    /// MMIO base address
    pub mmio_base: u64,
    /// Security level
    pub security_level: SecurityLevel,
    /// Number of ports
    pub num_ports: u8,
    /// Connected devices
    pub devices: BTreeMap<ThunderboltId, ThunderboltDevice>,
    /// Hot plug enabled
    pub hotplug_enabled: bool,
    /// Firmware version
    pub firmware_version: u32,
    /// Controller initialized
    pub initialized: bool,
    /// NVM (Non-Volatile Memory) version
    pub nvm_version: u32,
}

impl ThunderboltController {
    pub fn new(bus: u8, device: u8, function: u8, vendor_id: u16, device_id: u16) -> Self {
        Self {
            bus,
            device,
            function,
            vendor_id,
            device_id,
            generation: ThunderboltGeneration::from_device_id(vendor_id, device_id),
            mmio_base: 0,
            security_level: SecurityLevel::Unknown,
            num_ports: 0,
            devices: BTreeMap::new(),
            hotplug_enabled: false,
            firmware_version: 0,
            nvm_version: 0,
            initialized: false,
        }
    }

    /// Read a 32-bit register
    fn read32(&self, offset: u32) -> u32 {
        if self.mmio_base == 0 {
            return 0;
        }
        unsafe {
            let addr = (self.mmio_base + offset as u64) as *const u32;
            core::ptr::read_volatile(addr)
        }
    }

    /// Write a 32-bit register
    fn write32(&self, offset: u32, value: u32) {
        if self.mmio_base == 0 {
            return;
        }
        unsafe {
            let addr = (self.mmio_base + offset as u64) as *mut u32;
            core::ptr::write_volatile(addr, value);
        }
    }

    /// Initialize the controller
    pub fn init(&mut self) -> Result<(), &'static str> {
        // Get BAR0 (MMIO base)
        let bar0 = pci::read_u32(self.bus, self.device, self.function, 0x10);
        if bar0 == 0 || bar0 == 0xFFFFFFFF {
            return Err("Invalid BAR0");
        }

        // Check if it's a 64-bit BAR
        if bar0 & 0x4 != 0 {
            let bar1 = pci::read_u32(self.bus, self.device, self.function, 0x14);
            self.mmio_base = ((bar1 as u64) << 32) | ((bar0 & 0xFFFFFFF0) as u64);
        } else {
            self.mmio_base = (bar0 & 0xFFFFFFF0) as u64;
        }

        // Enable bus master and memory space
        let cmd = pci::read_u16(self.bus, self.device, self.function, 0x04);
        pci::write_u16(self.bus, self.device, self.function, 0x04, cmd | 0x6);

        // Read firmware version
        self.firmware_version = self.read32(nhi_regs::FW_VERSION);

        // Read security level
        let sec_reg = self.read32(nhi_regs::SECURITY);
        self.security_level = match sec_reg & 0x7 {
            0 => SecurityLevel::None,
            1 => SecurityLevel::User,
            2 => SecurityLevel::Secure,
            3 => SecurityLevel::DpOnly,
            4 => SecurityLevel::UsbOnly,
            _ => SecurityLevel::Unknown,
        };

        // Detect number of ports (simplified)
        self.num_ports = match self.generation {
            ThunderboltGeneration::Thunderbolt4 | ThunderboltGeneration::Thunderbolt5 => 4,
            ThunderboltGeneration::Thunderbolt3 => 2,
            _ => 2,
        };

        // Reset controller
        self.write32(nhi_regs::CONTROL, control_bits::RESET);
        for _ in 0..1000 {
            if self.read32(nhi_regs::CONTROL) & control_bits::RESET == 0 {
                break;
            }
        }

        // Enable controller
        let mut ctrl = control_bits::ENABLE | control_bits::HOT_PLUG_ENABLE;

        if self.security_level.allows_pcie() {
            ctrl |= control_bits::TUNNEL_PCIE;
        }
        if self.security_level.allows_displayport() {
            ctrl |= control_bits::TUNNEL_DP;
        }
        ctrl |= control_bits::TUNNEL_USB;

        self.write32(nhi_regs::CONTROL, ctrl);

        // Enable interrupts
        self.write32(nhi_regs::INTR_MASK, 0xFFFFFFFF);

        self.hotplug_enabled = true;
        self.initialized = true;

        // Scan for connected devices
        self.scan_devices();

        Ok(())
    }

    /// Scan for connected devices
    pub fn scan_devices(&mut self) {
        for port in 0..self.num_ports {
            let status = self.read32(nhi_regs::PORT_STATUS + (port as u32) * 0x10);

            if status & port_status_bits::DEVICE_PRESENT != 0 {
                let mut device = ThunderboltDevice::new(0, 0, port);

                device.state = if status & port_status_bits::CONNECTED != 0 {
                    ConnectionState::Connected
                } else {
                    ConnectionState::PendingAuthorization
                };

                // Get link speed
                let speed_bits = (status & port_status_bits::SPEED_MASK) >> 4;
                device.link_speed_gbps = match speed_bits {
                    0 => 10,  // Gen 1
                    1 => 20,  // Gen 2
                    2 => 40,  // Gen 3
                    3 => 80,  // Gen 4
                    4 => 120, // Gen 5 bonded
                    _ => 0,
                };

                // Get number of lanes
                device.num_lanes = match (status & port_status_bits::LANES_MASK) >> 8 {
                    0 => 1,
                    1 => 2,
                    2 => 4,
                    _ => 1,
                };

                device.generation = self.generation;

                // Set default authorization based on security level
                device.authorized = !self.security_level.requires_approval();

                self.devices.insert(device.id, device);
            }
        }
    }

    /// Authorize a device
    pub fn authorize_device(&mut self, device_id: ThunderboltId) -> Result<(), &'static str> {
        let device = self.devices.get_mut(&device_id).ok_or("Device not found")?;

        if device.state != ConnectionState::PendingAuthorization {
            return Err("Device not pending authorization");
        }

        device.authorized = true;
        device.state = ConnectionState::Authorizing;

        // In real implementation, this would set up tunnels and configure the device
        // For now, we just mark it as connected
        device.state = ConnectionState::Connected;

        Ok(())
    }

    /// Deauthorize a device
    pub fn deauthorize_device(&mut self, device_id: ThunderboltId) -> Result<(), &'static str> {
        let device = self.devices.get_mut(&device_id).ok_or("Device not found")?;

        device.authorized = false;
        device.state = ConnectionState::Disconnected;
        device.tunnels.clear();

        Ok(())
    }

    /// Create a tunnel
    pub fn create_tunnel(
        &mut self,
        device_id: ThunderboltId,
        tunnel_type: TunnelType,
    ) -> Result<u32, &'static str> {
        // Check security level
        match tunnel_type {
            TunnelType::Pcie if !self.security_level.allows_pcie() => {
                return Err("PCIe tunneling not allowed by security level");
            }
            TunnelType::DisplayPort if !self.security_level.allows_displayport() => {
                return Err("DisplayPort tunneling not allowed by security level");
            }
            _ => {}
        }

        let device = self.devices.get_mut(&device_id).ok_or("Device not found")?;

        if !device.authorized {
            return Err("Device not authorized");
        }

        let tunnel_id = device.tunnels.len() as u32;
        let mut tunnel = Tunnel::new(tunnel_id, tunnel_type, 0, device.port);

        // Configure tunnel bandwidth based on device link speed
        tunnel.bandwidth_gbps = match tunnel_type {
            TunnelType::Pcie => device.link_speed_gbps.min(32), // PCIe usually limited
            TunnelType::DisplayPort => device.link_speed_gbps.min(16), // DP 2.0 max
            TunnelType::Usb => 10,                              // USB 3.2 Gen 2
            TunnelType::Dma => device.link_speed_gbps,
        };

        tunnel.state = TunnelState::Active;
        device.tunnels.push(tunnel);

        Ok(tunnel_id)
    }

    /// Get device by ID
    pub fn get_device(&self, device_id: ThunderboltId) -> Option<&ThunderboltDevice> {
        self.devices.get(&device_id)
    }

    /// Get connected device count
    pub fn connected_count(&self) -> usize {
        self.devices.values().filter(|d| d.is_connected()).count()
    }

    /// Get pending device count
    pub fn pending_count(&self) -> usize {
        self.devices.values().filter(|d| d.is_pending()).count()
    }

    /// Handle hot-plug event
    pub fn handle_hotplug(&mut self) {
        // Re-scan devices
        self.scan_devices();
    }

    /// Get controller info string
    pub fn info_string(&self) -> String {
        alloc::format!(
            "{} Controller ({}:{:04X}:{:04X})\n  Firmware: {:08X}\n  Security: {}\n  Ports: {}\n  Devices: {} connected, {} pending",
            self.generation.name(),
            self.bus,
            self.vendor_id,
            self.device_id,
            self.firmware_version,
            self.security_level.name(),
            self.num_ports,
            self.connected_count(),
            self.pending_count(),
        )
    }
}

/// Thunderbolt subsystem manager
pub struct ThunderboltManager {
    /// Controllers
    pub controllers: Vec<ThunderboltController>,
    /// Default security level
    pub default_security: SecurityLevel,
    /// Event callbacks
    pub event_callbacks: Vec<fn(ThunderboltEvent)>,
    /// Initialized
    pub initialized: bool,
}

/// Thunderbolt events
#[derive(Debug, Clone)]
pub enum ThunderboltEvent {
    /// Device connected
    DeviceConnected(ThunderboltId),
    /// Device disconnected
    DeviceDisconnected(ThunderboltId),
    /// Device pending authorization
    DevicePending(ThunderboltId),
    /// Device authorized
    DeviceAuthorized(ThunderboltId),
    /// Tunnel established
    TunnelCreated(ThunderboltId, TunnelType),
    /// Tunnel removed
    TunnelRemoved(ThunderboltId, TunnelType),
    /// Error occurred
    Error(String),
}

impl ThunderboltManager {
    pub fn new() -> Self {
        Self {
            controllers: Vec::new(),
            default_security: SecurityLevel::User,
            event_callbacks: Vec::new(),
            initialized: false,
        }
    }

    /// Probe for Thunderbolt controllers
    pub fn probe(&mut self) {
        // Scan PCI bus for Thunderbolt controllers
        for bus in 0..=255u8 {
            for device in 0..32u8 {
                for function in 0..8u8 {
                    let vendor_id = pci::read_u16(bus, device, function, 0x00);
                    if vendor_id == 0xFFFF {
                        continue;
                    }

                    let device_id = pci::read_u16(bus, device, function, 0x02);
                    let class = pci::read_u8(bus, device, function, 0x0B);
                    let subclass = pci::read_u8(bus, device, function, 0x0A);

                    // Thunderbolt controllers appear as:
                    // - PCI bridge (class 0x06, subclass 0x04) for NHI
                    // - Serial bus controller (class 0x0C, subclass 0x0A) for USB4
                    let is_thunderbolt = if vendor_id == INTEL_VENDOR_ID {
                        matches!(
                            device_id,
                            device_ids::ALPINE_RIDGE_LP
                                | device_ids::ALPINE_RIDGE_2C
                                | device_ids::ALPINE_RIDGE_4C
                                | device_ids::ALPINE_RIDGE_C_2C
                                | device_ids::ALPINE_RIDGE_C_4C
                                | device_ids::TITAN_RIDGE_2C
                                | device_ids::TITAN_RIDGE_4C
                                | device_ids::TITAN_RIDGE_LP
                                | device_ids::ICE_LAKE_TB
                                | device_ids::ICE_LAKE_TB2
                                | device_ids::TIGER_LAKE_TB
                                | device_ids::TIGER_LAKE_TB_H
                                | device_ids::MAPLE_RIDGE_2C
                                | device_ids::MAPLE_RIDGE_4C
                                | device_ids::ALDER_LAKE_TB
                                | device_ids::ALDER_LAKE_TB_H
                                | device_ids::RAPTOR_LAKE_TB
                                | device_ids::METEOR_LAKE_TB
                        )
                    } else if vendor_id == AMD_VENDOR_ID {
                        matches!(device_id, device_ids::AMD_USB4_0 | device_ids::AMD_USB4_1)
                    } else {
                        false
                    };

                    if is_thunderbolt {
                        let mut controller =
                            ThunderboltController::new(bus, device, function, vendor_id, device_id);

                        if let Ok(()) = controller.init() {
                            crate::kprintln!(
                                "thunderbolt: Found {} controller at {:02X}:{:02X}.{:X}",
                                controller.generation.name(),
                                bus,
                                device,
                                function
                            );
                            self.controllers.push(controller);
                        }
                    }
                }
            }
        }

        self.initialized = true;
    }

    /// Get total controller count
    pub fn controller_count(&self) -> usize {
        self.controllers.len()
    }

    /// Get total connected device count
    pub fn device_count(&self) -> usize {
        self.controllers.iter().map(|c| c.connected_count()).sum()
    }

    /// Get total pending device count
    pub fn pending_count(&self) -> usize {
        self.controllers.iter().map(|c| c.pending_count()).sum()
    }

    /// Find a device by ID
    pub fn find_device(&self, device_id: ThunderboltId) -> Option<&ThunderboltDevice> {
        for controller in &self.controllers {
            if let Some(device) = controller.get_device(device_id) {
                return Some(device);
            }
        }
        None
    }

    /// Authorize a device
    pub fn authorize_device(&mut self, device_id: ThunderboltId) -> Result<(), &'static str> {
        for controller in &mut self.controllers {
            if controller.devices.contains_key(&device_id) {
                return controller.authorize_device(device_id);
            }
        }
        Err("Device not found")
    }

    /// Set security level for all controllers
    pub fn set_security_level(&mut self, level: SecurityLevel) {
        self.default_security = level;
        for controller in &mut self.controllers {
            controller.security_level = level;
        }
    }

    /// Get all devices
    pub fn all_devices(&self) -> Vec<&ThunderboltDevice> {
        self.controllers
            .iter()
            .flat_map(|c| c.devices.values())
            .collect()
    }

    /// Register event callback
    pub fn on_event(&mut self, callback: fn(ThunderboltEvent)) {
        self.event_callbacks.push(callback);
    }

    /// Format status string
    pub fn format_status(&self) -> String {
        let mut s = String::from("Thunderbolt/USB4 Subsystem:\n");

        if self.controllers.is_empty() {
            s.push_str("  No controllers found\n");
            return s;
        }

        for (i, controller) in self.controllers.iter().enumerate() {
            s.push_str(&alloc::format!("\nController {}:\n", i));
            s.push_str(&alloc::format!("  {}\n", controller.info_string()));

            for device in controller.devices.values() {
                s.push_str(&alloc::format!(
                    "    Device {}: {} (Port {}, {} Gbps x{}, {:?})\n",
                    device.id,
                    device.device_type.name(),
                    device.port,
                    device.link_speed_gbps,
                    device.num_lanes,
                    device.state,
                ));

                for tunnel in &device.tunnels {
                    s.push_str(&alloc::format!(
                        "      Tunnel {}: {} ({:?}, {} Gbps)\n",
                        tunnel.id,
                        tunnel.tunnel_type.name(),
                        tunnel.state,
                        tunnel.bandwidth_gbps,
                    ));
                }
            }
        }

        s
    }
}

impl Default for ThunderboltManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global Thunderbolt manager
static mut THUNDERBOLT_MANAGER: Option<ThunderboltManager> = None;

/// Initialize the Thunderbolt subsystem
pub fn init() {
    let mut manager = ThunderboltManager::new();
    manager.probe();

    let controller_count = manager.controller_count();
    let device_count = manager.device_count();

    unsafe {
        THUNDERBOLT_MANAGER = Some(manager);
    }

    crate::kprintln!(
        "thunderbolt: initialized ({} controllers, {} devices)",
        controller_count,
        device_count
    );
}

/// Get the global Thunderbolt manager
pub fn manager() -> &'static mut ThunderboltManager {
    unsafe {
        THUNDERBOLT_MANAGER
            .as_mut()
            .expect("Thunderbolt subsystem not initialized")
    }
}

/// Get controller count
pub fn controller_count() -> usize {
    manager().controller_count()
}

/// Get device count
pub fn device_count() -> usize {
    manager().device_count()
}

/// Get pending device count
pub fn pending_count() -> usize {
    manager().pending_count()
}

/// Authorize a device
pub fn authorize(device_id: ThunderboltId) -> Result<(), &'static str> {
    manager().authorize_device(device_id)
}

/// Set security level
pub fn set_security(level: SecurityLevel) {
    manager().set_security_level(level);
}

/// Get format status
pub fn format_status() -> String {
    manager().format_status()
}

/// Check if a device ID is for a Thunderbolt controller
pub fn is_thunderbolt_controller(vendor_id: u16, device_id: u16) -> bool {
    if vendor_id == INTEL_VENDOR_ID {
        matches!(
            device_id,
            device_ids::ALPINE_RIDGE_LP
                | device_ids::ALPINE_RIDGE_2C
                | device_ids::ALPINE_RIDGE_4C
                | device_ids::ALPINE_RIDGE_C_2C
                | device_ids::ALPINE_RIDGE_C_4C
                | device_ids::TITAN_RIDGE_2C
                | device_ids::TITAN_RIDGE_4C
                | device_ids::TITAN_RIDGE_LP
                | device_ids::ICE_LAKE_TB
                | device_ids::ICE_LAKE_TB2
                | device_ids::TIGER_LAKE_TB
                | device_ids::TIGER_LAKE_TB_H
                | device_ids::MAPLE_RIDGE_2C
                | device_ids::MAPLE_RIDGE_4C
                | device_ids::ALDER_LAKE_TB
                | device_ids::ALDER_LAKE_TB_H
                | device_ids::RAPTOR_LAKE_TB
                | device_ids::METEOR_LAKE_TB
        )
    } else if vendor_id == AMD_VENDOR_ID {
        matches!(device_id, device_ids::AMD_USB4_0 | device_ids::AMD_USB4_1)
    } else {
        false
    }
}
