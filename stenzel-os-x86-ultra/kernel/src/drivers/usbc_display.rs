//! USB-C Display Driver - DisplayPort Alt Mode
//!
//! Implements USB Type-C DisplayPort Alternate Mode for:
//! - DisplayPort 1.4/2.0 over USB-C
//! - Thunderbolt 3/4 display tunneling
//! - USB4 display support
//! - Multi-stream transport (MST) over USB-C

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use spin::Mutex;

/// USB-C connector orientation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Normal,
    Flipped,
    Unknown,
}

/// USB-C mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbcMode {
    /// USB 2.0 only
    Usb20,
    /// USB 3.x SuperSpeed
    Usb3SuperSpeed,
    /// USB 3.x + DisplayPort 2 lanes
    Usb3PlusDp2Lane,
    /// DisplayPort 4 lanes (no USB 3.x)
    Dp4Lane,
    /// Thunderbolt 3
    Thunderbolt3,
    /// Thunderbolt 4
    Thunderbolt4,
    /// USB4
    Usb4,
}

/// DisplayPort lane count
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DpLaneCount {
    None = 0,
    TwoLanes = 2,
    FourLanes = 4,
}

/// DisplayPort version
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DpVersion {
    Dp12,     // 5.4 Gbps/lane
    Dp13,     // 8.1 Gbps/lane (HBR3)
    Dp14,     // 8.1 Gbps/lane + DSC
    Dp20,     // 20 Gbps/lane (UHBR10)
    Dp21,     // 40 Gbps/lane (UHBR20)
}

impl DpVersion {
    /// Get bandwidth per lane in Gbps
    pub fn bandwidth_per_lane_gbps(&self) -> u32 {
        match self {
            Self::Dp12 => 5,
            Self::Dp13 | Self::Dp14 => 8,
            Self::Dp20 => 20,
            Self::Dp21 => 40,
        }
    }

    /// Get max supported resolution at 60Hz
    pub fn max_resolution_60hz(&self, lanes: DpLaneCount) -> (u32, u32) {
        let total_bw = self.bandwidth_per_lane_gbps() * lanes as u32;
        match total_bw {
            0..=10 => (1920, 1080),    // Full HD
            11..=20 => (2560, 1440),   // QHD
            21..=32 => (3840, 2160),   // 4K
            33..=60 => (5120, 2880),   // 5K
            61..=80 => (7680, 4320),   // 8K
            _ => (15360, 8640),        // 16K
        }
    }
}

/// USB-C Power Delivery role
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdRole {
    /// No PD
    None,
    /// Sink only
    Sink,
    /// Source only
    Source,
    /// Dual role
    DualRole,
}

/// DisplayPort Alt Mode status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DpAltModeStatus {
    /// Not configured
    NotConfigured,
    /// Configuring (entering alt mode)
    Configuring,
    /// Active and connected
    Active,
    /// Connected but no HPD
    NoHpd,
    /// Error state
    Error,
}

/// Type-C Port Controller registers (TCPC)
pub mod tcpc_regs {
    // TCPC register addresses
    pub const VENDOR_ID: u8 = 0x00;
    pub const PRODUCT_ID: u8 = 0x02;
    pub const DEVICE_ID: u8 = 0x04;
    pub const USBTYPEC_REV: u8 = 0x06;
    pub const PD_REV: u8 = 0x08;
    pub const PD_INTERFACE_REV: u8 = 0x0A;

    pub const ALERT: u8 = 0x10;
    pub const ALERT_MASK: u8 = 0x12;
    pub const POWER_STATUS_MASK: u8 = 0x14;
    pub const FAULT_STATUS_MASK: u8 = 0x15;
    pub const EXT_STATUS_MASK: u8 = 0x16;

    pub const CONFIG_STD_OUTPUT: u8 = 0x18;
    pub const TCPC_CONTROL: u8 = 0x19;
    pub const ROLE_CONTROL: u8 = 0x1A;
    pub const FAULT_CONTROL: u8 = 0x1B;
    pub const POWER_CONTROL: u8 = 0x1C;

    pub const CC_STATUS: u8 = 0x1D;
    pub const POWER_STATUS: u8 = 0x1E;
    pub const FAULT_STATUS: u8 = 0x1F;
    pub const EXT_STATUS: u8 = 0x20;

    pub const COMMAND: u8 = 0x23;
    pub const DEV_CAPABILITIES_1: u8 = 0x24;
    pub const DEV_CAPABILITIES_2: u8 = 0x26;
    pub const STD_INPUT_CAPABILITIES: u8 = 0x28;
    pub const STD_OUTPUT_CAPABILITIES: u8 = 0x29;

    pub const MSG_HEADER_INFO: u8 = 0x2E;
    pub const RX_DETECT: u8 = 0x2F;
    pub const RX_BYTE_CNT: u8 = 0x30;
    pub const RX_BUF_FRAME_TYPE: u8 = 0x31;
    pub const RX_BUF_HEADER: u8 = 0x32;
    pub const RX_BUF_DATA: u8 = 0x34;
    pub const TRANSMIT: u8 = 0x50;
    pub const TX_BYTE_CNT: u8 = 0x51;
    pub const TX_BUF_HEADER: u8 = 0x52;
    pub const TX_BUF_DATA: u8 = 0x54;

    pub const VBUS_VOLTAGE: u8 = 0x70;
    pub const VBUS_SINK_DISCONNECT_THRESHOLD: u8 = 0x72;
    pub const VBUS_STOP_DISCHARGE_THRESHOLD: u8 = 0x74;
    pub const VBUS_ALARM_HI_CFG: u8 = 0x76;
    pub const VBUS_ALARM_LO_CFG: u8 = 0x78;

    // Commands
    pub const CMD_WAKE: u8 = 0x11;
    pub const CMD_LOOK4CONNECTION: u8 = 0x99;
    pub const CMD_DISABLE_VBUS_DETECT: u8 = 0x22;
    pub const CMD_ENABLE_VBUS_DETECT: u8 = 0x33;
    pub const CMD_DISABLE_SINK_VBUS: u8 = 0x44;
    pub const CMD_SINK_VBUS: u8 = 0x55;
    pub const CMD_DISABLE_SRC_VBUS: u8 = 0x66;
    pub const CMD_SRC_VBUS_DEFAULT: u8 = 0x77;
    pub const CMD_SRC_VBUS_HV: u8 = 0x88;
}

/// VDM (Vendor Defined Message) commands for DP Alt Mode
pub mod dp_vdm {
    /// DP Alt Mode SID (Standard ID)
    pub const DP_SID: u16 = 0xFF01;

    /// DP Alt Mode commands
    pub const DP_CMD_STATUS_UPDATE: u8 = 0x10;
    pub const DP_CMD_CONFIGURE: u8 = 0x11;
    pub const DP_CMD_ATTENTION: u8 = 0x06;

    /// DP Configuration bits
    pub const DP_CFG_PIN_ASSIGNMENT_C: u32 = 0x04;  // DP 4 lanes
    pub const DP_CFG_PIN_ASSIGNMENT_D: u32 = 0x08;  // DP 2 + USB 2 lanes
    pub const DP_CFG_PIN_ASSIGNMENT_E: u32 = 0x10;  // DP 4 lanes (alternative)

    pub const DP_CFG_UFP_D_CONNECTED: u32 = 1 << 2;
    pub const DP_CFG_DFP_D_CONNECTED: u32 = 1 << 1;

    /// DP Status bits
    pub const DP_STATUS_HPD: u32 = 1 << 7;
    pub const DP_STATUS_IRQ_HPD: u32 = 1 << 8;
    pub const DP_STATUS_ENABLED: u32 = 1 << 0;
}

/// USB-C Port information
#[derive(Debug, Clone)]
pub struct UsbcPort {
    /// Port index
    pub port_index: u8,
    /// TCPC I2C address
    pub tcpc_addr: u8,
    /// TCPC controller base address
    pub tcpc_base: u64,
    /// Orientation
    pub orientation: Orientation,
    /// Current mode
    pub mode: UsbcMode,
    /// PD role
    pub pd_role: PdRole,
    /// DP Alt Mode status
    pub dp_status: DpAltModeStatus,
    /// DP version supported
    pub dp_version: DpVersion,
    /// DP lane count
    pub dp_lanes: DpLaneCount,
    /// HPD (Hot Plug Detect) state
    pub hpd_state: bool,
    /// Connected device info
    pub device_info: Option<ConnectedDevice>,
}

/// Connected device information
#[derive(Debug, Clone)]
pub struct ConnectedDevice {
    /// Device type
    pub device_type: ConnectedDeviceType,
    /// Vendor ID
    pub vendor_id: u16,
    /// Product ID
    pub product_id: u16,
    /// Supported modes
    pub supported_modes: Vec<UsbcMode>,
    /// Max DP version
    pub max_dp_version: DpVersion,
    /// Supports DSC (Display Stream Compression)
    pub supports_dsc: bool,
    /// Supports MST (Multi-Stream Transport)
    pub supports_mst: bool,
}

/// Type of connected device
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectedDeviceType {
    /// Display (monitor, TV)
    Display,
    /// Dock/Hub
    Dock,
    /// USB-C to DisplayPort adapter
    DpAdapter,
    /// USB-C to HDMI adapter
    HdmiAdapter,
    /// Thunderbolt device
    ThunderboltDevice,
    /// Unknown
    Unknown,
}

/// Display output routing
#[derive(Debug, Clone)]
pub struct DisplayRoute {
    /// USB-C port index
    pub port_index: u8,
    /// GPU output (e.g., "DDI-A", "DP-1")
    pub gpu_output: String,
    /// Active
    pub active: bool,
    /// Resolution
    pub resolution: (u32, u32),
    /// Refresh rate (Hz)
    pub refresh_rate: u32,
}

/// USB-C Display Manager
pub struct UsbcDisplayManager {
    /// USB-C ports
    ports: Vec<UsbcPort>,
    /// Display routes
    display_routes: Vec<DisplayRoute>,
    /// Initialized
    initialized: bool,
    /// Thunderbolt supported
    thunderbolt_supported: bool,
    /// USB4 supported
    usb4_supported: bool,
}

impl UsbcDisplayManager {
    /// Create new USB-C Display manager
    pub fn new() -> Self {
        Self {
            ports: Vec::new(),
            display_routes: Vec::new(),
            initialized: false,
            thunderbolt_supported: false,
            usb4_supported: false,
        }
    }

    /// Initialize USB-C display support
    pub fn init(&mut self) -> Result<(), &'static str> {
        crate::kprintln!("[usbc-display] Initializing USB-C display support");

        // Scan for TCPC controllers
        self.scan_tcpc_controllers()?;

        // Check for Thunderbolt support
        self.detect_thunderbolt();

        // Check for USB4 support
        self.detect_usb4();

        // Initialize each port
        for i in 0..self.ports.len() {
            self.init_port(i)?;
        }

        self.initialized = true;

        crate::kprintln!("[usbc-display] Found {} USB-C ports", self.ports.len());
        if self.thunderbolt_supported {
            crate::kprintln!("[usbc-display] Thunderbolt 3/4 supported");
        }
        if self.usb4_supported {
            crate::kprintln!("[usbc-display] USB4 supported");
        }

        Ok(())
    }

    /// Scan for TCPC controllers
    fn scan_tcpc_controllers(&mut self) -> Result<(), &'static str> {
        // Common TCPC I2C addresses
        let tcpc_addrs = [0x4C, 0x4D, 0x4E, 0x4F, 0x50, 0x51];

        for (index, &addr) in tcpc_addrs.iter().enumerate() {
            // Try to read vendor ID from I2C
            // In real implementation, would use I2C/SMBus driver
            // For now, assume ports exist based on platform

            // Create port entry (assuming 2 USB-C ports for typical laptop)
            if index < 2 {
                let port = UsbcPort {
                    port_index: index as u8,
                    tcpc_addr: addr,
                    tcpc_base: 0,
                    orientation: Orientation::Unknown,
                    mode: UsbcMode::Usb20,
                    pd_role: PdRole::DualRole,
                    dp_status: DpAltModeStatus::NotConfigured,
                    dp_version: DpVersion::Dp14,
                    dp_lanes: DpLaneCount::None,
                    hpd_state: false,
                    device_info: None,
                };
                self.ports.push(port);
            }
        }

        Ok(())
    }

    /// Detect Thunderbolt support
    fn detect_thunderbolt(&mut self) {
        // Check for Thunderbolt controller on PCI bus
        for bus in 0..=5u8 {
            for device in 0..32u8 {
                let vendor_id = crate::drivers::pci::read_u16(bus, device, 0, 0x00);
                let device_id = crate::drivers::pci::read_u16(bus, device, 0, 0x02);

                // Intel Thunderbolt controllers
                if vendor_id == 0x8086 {
                    match device_id {
                        0x15D2 | 0x15D3 | 0x15D9 | 0x15DA => {
                            // Alpine Ridge (TB3)
                            self.thunderbolt_supported = true;
                        }
                        0x15E8 | 0x15EB | 0x15EF => {
                            // Titan Ridge (TB3)
                            self.thunderbolt_supported = true;
                        }
                        0x9A1B | 0x9A1D | 0x9A1F | 0x9A21 | 0x9A23 | 0x9A25 => {
                            // Ice Lake/Tiger Lake (TB4)
                            self.thunderbolt_supported = true;
                        }
                        0xA0E0 | 0xA0E4 | 0xA73D | 0xA73E => {
                            // Alder Lake/Raptor Lake (TB4)
                            self.thunderbolt_supported = true;
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// Detect USB4 support
    fn detect_usb4(&mut self) {
        // USB4 is often integrated with TB4 on Intel
        // For AMD, check for USB4 controller
        for bus in 0..=5u8 {
            for device in 0..32u8 {
                let vendor_id = crate::drivers::pci::read_u16(bus, device, 0, 0x00);
                let class_code = crate::drivers::pci::read_u8(bus, device, 0, 0x0B);
                let subclass = crate::drivers::pci::read_u8(bus, device, 0, 0x0A);
                let prog_if = crate::drivers::pci::read_u8(bus, device, 0, 0x09);

                // USB4 Host Controller (class 0x0C, subclass 0x0D)
                if class_code == 0x0C && subclass == 0x0D {
                    self.usb4_supported = true;
                }

                // AMD USB4 (Pink Sardine)
                if vendor_id == 0x1022 {
                    // AMD USB4 controllers
                    let device_id = crate::drivers::pci::read_u16(bus, device, 0, 0x02);
                    if matches!(device_id, 0x162D | 0x162E | 0x162F) {
                        self.usb4_supported = true;
                    }
                }
            }
        }
    }

    /// Initialize a USB-C port
    fn init_port(&mut self, port_idx: usize) -> Result<(), &'static str> {
        if port_idx >= self.ports.len() {
            return Err("Invalid port index");
        }

        crate::kprintln!("[usbc-display] Initializing port {}", port_idx);

        // In real implementation, would:
        // 1. Reset TCPC
        // 2. Configure role control
        // 3. Set up interrupt handling
        // 4. Enable PD communication

        Ok(())
    }

    /// Enter DisplayPort Alt Mode on a port
    pub fn enter_dp_alt_mode(&mut self, port_idx: usize, lanes: DpLaneCount) -> Result<(), &'static str> {
        if port_idx >= self.ports.len() {
            return Err("Invalid port index");
        }

        let port = &mut self.ports[port_idx];
        crate::kprintln!("[usbc-display] Entering DP Alt Mode on port {} with {} lanes",
                        port_idx, lanes as u32);

        // Set status to configuring
        port.dp_status = DpAltModeStatus::Configuring;

        // In real implementation, would:
        // 1. Send VDM Discover Identity
        // 2. Send VDM Discover SVIDs
        // 3. Send VDM Discover Modes
        // 4. Send VDM Enter Mode (DP SID)
        // 5. Send DP Status Update VDM
        // 6. Send DP Configure VDM with lane assignment

        port.dp_lanes = lanes;
        port.mode = match lanes {
            DpLaneCount::FourLanes => UsbcMode::Dp4Lane,
            DpLaneCount::TwoLanes => UsbcMode::Usb3PlusDp2Lane,
            DpLaneCount::None => UsbcMode::Usb3SuperSpeed,
        };

        port.dp_status = DpAltModeStatus::Active;

        crate::kprintln!("[usbc-display] DP Alt Mode active on port {}", port_idx);

        Ok(())
    }

    /// Exit DisplayPort Alt Mode
    pub fn exit_dp_alt_mode(&mut self, port_idx: usize) -> Result<(), &'static str> {
        if port_idx >= self.ports.len() {
            return Err("Invalid port index");
        }

        let port = &mut self.ports[port_idx];
        crate::kprintln!("[usbc-display] Exiting DP Alt Mode on port {}", port_idx);

        // Send VDM Exit Mode
        port.dp_status = DpAltModeStatus::NotConfigured;
        port.dp_lanes = DpLaneCount::None;
        port.mode = UsbcMode::Usb3SuperSpeed;

        Ok(())
    }

    /// Handle HPD (Hot Plug Detect) event
    pub fn handle_hpd(&mut self, port_idx: usize, hpd_state: bool) -> Result<(), &'static str> {
        if port_idx >= self.ports.len() {
            return Err("Invalid port index");
        }

        // Get previous state and status first
        let (was_connected, needs_dp_mode) = {
            let port = &self.ports[port_idx];
            let needs_dp = port.dp_status == DpAltModeStatus::NotConfigured ||
                          port.dp_status == DpAltModeStatus::NoHpd;
            (port.hpd_state, needs_dp)
        };

        // Update HPD state
        self.ports[port_idx].hpd_state = hpd_state;

        if hpd_state && !was_connected {
            crate::kprintln!("[usbc-display] Display connected on port {}", port_idx);

            // If not already in DP mode, try to enter
            if needs_dp_mode {
                // Auto-enter DP Alt Mode with 4 lanes
                let _ = self.enter_dp_alt_mode(port_idx, DpLaneCount::FourLanes);
            }

            self.ports[port_idx].dp_status = DpAltModeStatus::Active;
        } else if !hpd_state && was_connected {
            crate::kprintln!("[usbc-display] Display disconnected on port {}", port_idx);
            self.ports[port_idx].dp_status = DpAltModeStatus::NoHpd;

            // Remove any display routes for this port
            self.display_routes.retain(|r| r.port_index != port_idx as u8);
        }

        Ok(())
    }

    /// Configure display output routing
    pub fn configure_route(&mut self, port_idx: usize, gpu_output: &str,
                          resolution: (u32, u32), refresh_rate: u32) -> Result<(), &'static str> {
        if port_idx >= self.ports.len() {
            return Err("Invalid port index");
        }

        let port = &self.ports[port_idx];
        if port.dp_status != DpAltModeStatus::Active {
            return Err("DP Alt Mode not active on port");
        }

        crate::kprintln!("[usbc-display] Configuring route: port {} -> {} @ {}x{}@{}Hz",
                        port_idx, gpu_output, resolution.0, resolution.1, refresh_rate);

        // Check bandwidth
        let max_res = port.dp_version.max_resolution_60hz(port.dp_lanes);
        if resolution.0 > max_res.0 || resolution.1 > max_res.1 {
            return Err("Resolution exceeds port bandwidth");
        }

        // Add or update route
        if let Some(route) = self.display_routes.iter_mut()
            .find(|r| r.port_index == port_idx as u8) {
            route.gpu_output = gpu_output.into();
            route.resolution = resolution;
            route.refresh_rate = refresh_rate;
            route.active = true;
        } else {
            self.display_routes.push(DisplayRoute {
                port_index: port_idx as u8,
                gpu_output: gpu_output.into(),
                active: true,
                resolution,
                refresh_rate,
            });
        }

        Ok(())
    }

    /// Get port information
    pub fn get_port(&self, port_idx: usize) -> Option<&UsbcPort> {
        self.ports.get(port_idx)
    }

    /// Get all ports
    pub fn get_ports(&self) -> &[UsbcPort] {
        &self.ports
    }

    /// Get active display routes
    pub fn get_display_routes(&self) -> &[DisplayRoute] {
        &self.display_routes
    }

    /// Get status string
    pub fn get_status(&self) -> String {
        let mut status = format!(
            "USB-C Display Manager:\n\
             Initialized: {}\n\
             Thunderbolt: {}\n\
             USB4: {}\n\
             Ports: {}\n",
            self.initialized,
            if self.thunderbolt_supported { "Yes" } else { "No" },
            if self.usb4_supported { "Yes" } else { "No" },
            self.ports.len()
        );

        for port in &self.ports {
            status.push_str(&format!(
                "\nPort {}:\n\
                 Mode: {:?}\n\
                 DP Status: {:?}\n\
                 DP Lanes: {:?}\n\
                 HPD: {}\n",
                port.port_index,
                port.mode,
                port.dp_status,
                port.dp_lanes,
                if port.hpd_state { "Connected" } else { "Disconnected" }
            ));
        }

        for route in &self.display_routes {
            status.push_str(&format!(
                "\nRoute: Port {} -> {} ({}x{}@{}Hz) {}\n",
                route.port_index,
                route.gpu_output,
                route.resolution.0,
                route.resolution.1,
                route.refresh_rate,
                if route.active { "[Active]" } else { "[Inactive]" }
            ));
        }

        status
    }

    /// Check if Thunderbolt is supported
    pub fn has_thunderbolt(&self) -> bool {
        self.thunderbolt_supported
    }

    /// Check if USB4 is supported
    pub fn has_usb4(&self) -> bool {
        self.usb4_supported
    }
}

/// Global USB-C Display manager
static USBC_DISPLAY: Mutex<Option<UsbcDisplayManager>> = Mutex::new(None);

/// Initialize USB-C display support
pub fn init() -> Result<(), &'static str> {
    let mut manager = UsbcDisplayManager::new();
    manager.init()?;

    *USBC_DISPLAY.lock() = Some(manager);

    Ok(())
}

/// Get USB-C display manager
pub fn get_manager() -> Option<spin::MutexGuard<'static, Option<UsbcDisplayManager>>> {
    let guard = USBC_DISPLAY.lock();
    if guard.is_some() {
        Some(guard)
    } else {
        None
    }
}

/// Enter DP Alt Mode on a port
pub fn enter_dp_mode(port_idx: usize, lanes: DpLaneCount) -> Result<(), &'static str> {
    if let Some(mut guard) = get_manager() {
        if let Some(mgr) = guard.as_mut() {
            return mgr.enter_dp_alt_mode(port_idx, lanes);
        }
    }
    Err("USB-C display not initialized")
}

/// Handle HPD event
pub fn handle_hpd(port_idx: usize, connected: bool) -> Result<(), &'static str> {
    if let Some(mut guard) = get_manager() {
        if let Some(mgr) = guard.as_mut() {
            return mgr.handle_hpd(port_idx, connected);
        }
    }
    Err("USB-C display not initialized")
}

/// Get status
pub fn get_status() -> String {
    if let Some(guard) = get_manager() {
        if let Some(mgr) = guard.as_ref() {
            return mgr.get_status();
        }
    }
    "USB-C Display: Not initialized".into()
}

/// Check Thunderbolt support
pub fn has_thunderbolt() -> bool {
    if let Some(guard) = get_manager() {
        if let Some(mgr) = guard.as_ref() {
            return mgr.has_thunderbolt();
        }
    }
    false
}

/// Check USB4 support
pub fn has_usb4() -> bool {
    if let Some(guard) = get_manager() {
        if let Some(mgr) = guard.as_ref() {
            return mgr.has_usb4();
        }
    }
    false
}
