//! Bluetooth Subsystem
//!
//! Provides Bluetooth connectivity with:
//! - HCI (Host Controller Interface) layer
//! - L2CAP connection support
//! - USB Bluetooth transport
//! - Bluetooth device management

extern crate alloc;

pub mod a2dp;
pub mod hci;
pub mod hid;
pub mod l2cap;
pub mod pairing;
pub mod usb_transport;

use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use crate::sync::TicketSpinlock;

/// Bluetooth device address (BD_ADDR) - 6 bytes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BdAddr(pub [u8; 6]);

impl BdAddr {
    pub const ZERO: Self = BdAddr([0; 6]);

    pub fn new(bytes: [u8; 6]) -> Self {
        Self(bytes)
    }

    pub fn from_slice(slice: &[u8]) -> Option<Self> {
        if slice.len() >= 6 {
            let mut addr = [0u8; 6];
            addr.copy_from_slice(&slice[..6]);
            Some(Self(addr))
        } else {
            None
        }
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0; 6]
    }

    pub fn to_string(&self) -> String {
        use core::fmt::Write;
        let mut s = String::new();
        write!(s, "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            self.0[5], self.0[4], self.0[3], self.0[2], self.0[1], self.0[0]).ok();
        s
    }
}

/// Bluetooth device class (24 bits)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DeviceClass(pub [u8; 3]);

impl DeviceClass {
    pub fn service_classes(&self) -> u16 {
        ((self.0[2] as u16) << 3) | ((self.0[1] as u16 >> 5) & 0x07)
    }

    pub fn major_class(&self) -> u8 {
        (self.0[1] >> 2) & 0x1F
    }

    pub fn minor_class(&self) -> u8 {
        (self.0[0] >> 2) & 0x3F
    }

    pub fn major_class_name(&self) -> &'static str {
        match self.major_class() {
            0x00 => "Miscellaneous",
            0x01 => "Computer",
            0x02 => "Phone",
            0x03 => "LAN/Network",
            0x04 => "Audio/Video",
            0x05 => "Peripheral",
            0x06 => "Imaging",
            0x07 => "Wearable",
            0x08 => "Toy",
            0x09 => "Health",
            0x1F => "Uncategorized",
            _ => "Unknown",
        }
    }
}

/// Bluetooth link type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkType {
    Sco = 0x00,
    Acl = 0x01,
    Esco = 0x02,
}

/// Bluetooth link key type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkKeyType {
    Combination = 0x00,
    LocalUnit = 0x01,
    RemoteUnit = 0x02,
    DebugCombination = 0x03,
    UnauthenticatedP192 = 0x04,
    AuthenticatedP192 = 0x05,
    ChangedCombination = 0x06,
    UnauthenticatedP256 = 0x07,
    AuthenticatedP256 = 0x08,
}

/// Bluetooth controller state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControllerState {
    Off,
    Initializing,
    Ready,
    Scanning,
    Advertising,
    Connecting,
    Connected,
    Error,
}

/// Bluetooth controller capabilities
#[derive(Debug, Clone, Default)]
pub struct ControllerCapabilities {
    pub supports_le: bool,
    pub supports_bredr: bool,
    pub supports_sco: bool,
    pub supports_esco: bool,
    pub supports_secure_connections: bool,
    pub max_acl_size: u16,
    pub max_sco_size: u8,
    pub max_acl_packets: u16,
    pub max_sco_packets: u16,
    pub le_features: u64,
    pub lmp_version: u8,
    pub lmp_subversion: u16,
    pub manufacturer: u16,
    pub hci_version: u8,
    pub hci_revision: u16,
}

/// Remote Bluetooth device info
#[derive(Debug, Clone)]
pub struct RemoteDevice {
    pub address: BdAddr,
    pub name: Option<String>,
    pub device_class: DeviceClass,
    pub rssi: i8,
    pub connected: bool,
    pub paired: bool,
    pub link_key: Option<[u8; 16]>,
    pub link_type: Option<LinkType>,
}

impl RemoteDevice {
    pub fn new(address: BdAddr) -> Self {
        Self {
            address,
            name: None,
            device_class: DeviceClass::default(),
            rssi: 0,
            connected: false,
            paired: false,
            link_key: None,
            link_type: None,
        }
    }
}

/// Bluetooth controller
pub struct BluetoothController {
    pub id: u8,
    pub state: ControllerState,
    pub address: BdAddr,
    pub name: String,
    pub capabilities: ControllerCapabilities,
    pub devices: Vec<RemoteDevice>,
    transport: Option<usb_transport::UsbBluetoothTransport>,
    scanning: AtomicBool,
    scan_filter: Option<DeviceClass>,
}

impl BluetoothController {
    pub fn new(id: u8) -> Self {
        Self {
            id,
            state: ControllerState::Off,
            address: BdAddr::ZERO,
            name: String::new(),
            capabilities: ControllerCapabilities::default(),
            devices: Vec::new(),
            transport: None,
            scanning: AtomicBool::new(false),
            scan_filter: None,
        }
    }

    /// Initialize the controller
    pub fn init(&mut self) -> Result<(), &'static str> {
        self.state = ControllerState::Initializing;

        // Send HCI reset
        self.send_command(&hci::commands::reset())?;

        // Read local version
        self.send_command(&hci::commands::read_local_version())?;

        // Read BD_ADDR
        self.send_command(&hci::commands::read_bd_addr())?;

        // Read buffer sizes
        self.send_command(&hci::commands::read_buffer_size())?;

        // Read local supported features
        self.send_command(&hci::commands::read_local_features())?;

        self.state = ControllerState::Ready;
        Ok(())
    }

    /// Send HCI command
    fn send_command(&mut self, cmd: &[u8]) -> Result<(), &'static str> {
        if let Some(ref mut transport) = self.transport {
            transport.send_command(cmd)
        } else {
            Err("No transport")
        }
    }

    /// Start scanning for devices
    pub fn start_scan(&mut self, filter: Option<DeviceClass>) -> Result<(), &'static str> {
        if self.state != ControllerState::Ready {
            return Err("Controller not ready");
        }

        self.scan_filter = filter;
        self.scanning.store(true, Ordering::SeqCst);
        self.state = ControllerState::Scanning;

        // Send inquiry command
        self.send_command(&hci::commands::inquiry(
            hci::LAP_GIAC, // General Inquiry Access Code
            0x30,          // 30 * 1.28s = ~38 seconds
            0xFF,          // Max responses
        ))?;

        Ok(())
    }

    /// Stop scanning
    pub fn stop_scan(&mut self) -> Result<(), &'static str> {
        if self.state != ControllerState::Scanning {
            return Err("Not scanning");
        }

        self.scanning.store(false, Ordering::SeqCst);

        // Cancel inquiry
        self.send_command(&hci::commands::inquiry_cancel())?;

        self.state = ControllerState::Ready;
        Ok(())
    }

    /// Check if scanning
    pub fn is_scanning(&self) -> bool {
        self.scanning.load(Ordering::SeqCst)
    }

    /// Connect to a remote device
    pub fn connect(&mut self, address: &BdAddr) -> Result<(), &'static str> {
        if self.state != ControllerState::Ready {
            return Err("Controller not ready");
        }

        self.state = ControllerState::Connecting;

        // Send create connection command
        self.send_command(&hci::commands::create_connection(
            address,
            0x18,    // Packet types (DM1, DH1)
            0x01,    // Page scan repetition mode R1
            0x00,    // Reserved
            0x0000,  // Clock offset
            0x01,    // Allow role switch
        ))?;

        Ok(())
    }

    /// Disconnect from a device
    pub fn disconnect(&mut self, handle: u16, reason: u8) -> Result<(), &'static str> {
        self.send_command(&hci::commands::disconnect(handle, reason))?;
        Ok(())
    }

    /// Get discovered devices
    pub fn discovered_devices(&self) -> &[RemoteDevice] {
        &self.devices
    }

    /// Get device by address
    pub fn get_device(&self, address: &BdAddr) -> Option<&RemoteDevice> {
        self.devices.iter().find(|d| d.address == *address)
    }

    /// Get device by address (mutable)
    pub fn get_device_mut(&mut self, address: &BdAddr) -> Option<&mut RemoteDevice> {
        self.devices.iter_mut().find(|d| d.address == *address)
    }

    /// Process HCI event
    pub fn process_event(&mut self, event: &[u8]) {
        if event.len() < 2 {
            return;
        }

        let event_code = event[0];
        let _param_len = event[1];
        let params = &event[2..];

        match event_code {
            hci::events::INQUIRY_COMPLETE => {
                self.scanning.store(false, Ordering::SeqCst);
                self.state = ControllerState::Ready;
            }
            hci::events::INQUIRY_RESULT => {
                self.handle_inquiry_result(params);
            }
            hci::events::INQUIRY_RESULT_WITH_RSSI => {
                self.handle_inquiry_result_rssi(params);
            }
            hci::events::EXTENDED_INQUIRY_RESULT => {
                self.handle_extended_inquiry_result(params);
            }
            hci::events::CONNECTION_COMPLETE => {
                self.handle_connection_complete(params);
            }
            hci::events::DISCONNECTION_COMPLETE => {
                self.handle_disconnection_complete(params);
            }
            hci::events::REMOTE_NAME_REQUEST_COMPLETE => {
                self.handle_remote_name(params);
            }
            hci::events::COMMAND_COMPLETE => {
                self.handle_command_complete(params);
            }
            hci::events::COMMAND_STATUS => {
                self.handle_command_status(params);
            }
            // Pairing events
            pairing::events::LINK_KEY_REQUEST => {
                let cmd = pairing::PAIRING_MANAGER.lock().handle_link_key_request(params);
                if !cmd.is_empty() {
                    let _ = self.send_command(&cmd);
                }
            }
            pairing::events::LINK_KEY_NOTIFICATION => {
                pairing::PAIRING_MANAGER.lock().handle_link_key_notification(params);
            }
            pairing::events::PIN_CODE_REQUEST => {
                if let Some(cmd) = pairing::PAIRING_MANAGER.lock().handle_pin_code_request(params) {
                    let _ = self.send_command(&cmd);
                }
            }
            pairing::events::IO_CAPABILITY_REQUEST => {
                if let Some(address) = BdAddr::from_slice(params) {
                    if let Some(cmd) = pairing::PAIRING_MANAGER.lock().handle_io_capability_request(&address) {
                        let _ = self.send_command(&cmd);
                    }
                }
            }
            pairing::events::IO_CAPABILITY_RESPONSE => {
                pairing::PAIRING_MANAGER.lock().handle_io_capability_response(params);
            }
            pairing::events::USER_CONFIRMATION_REQUEST => {
                if let Some(cmd) = pairing::PAIRING_MANAGER.lock().handle_user_confirmation_request(params) {
                    let _ = self.send_command(&cmd);
                }
            }
            pairing::events::USER_PASSKEY_REQUEST => {
                pairing::PAIRING_MANAGER.lock().handle_user_passkey_request(params);
            }
            pairing::events::USER_PASSKEY_NOTIFICATION => {
                pairing::PAIRING_MANAGER.lock().handle_user_passkey_notification(params);
            }
            pairing::events::SIMPLE_PAIRING_COMPLETE => {
                pairing::PAIRING_MANAGER.lock().handle_simple_pairing_complete(params);
            }
            pairing::events::AUTHENTICATION_COMPLETE => {
                pairing::PAIRING_MANAGER.lock().handle_authentication_complete(params);
            }
            _ => {
                crate::kprintln!("bt: unknown event {:02X}", event_code);
            }
        }
    }

    fn handle_inquiry_result(&mut self, params: &[u8]) {
        if params.is_empty() {
            return;
        }

        let num_responses = params[0] as usize;
        let mut offset = 1;

        for _ in 0..num_responses {
            if offset + 14 > params.len() {
                break;
            }

            let address = BdAddr::from_slice(&params[offset..]).unwrap();
            offset += 6;

            let _page_scan_rep = params[offset];
            offset += 1;

            // Reserved
            offset += 2;

            let class = DeviceClass([params[offset], params[offset + 1], params[offset + 2]]);
            offset += 3;

            let _clock_offset = u16::from_le_bytes([params[offset], params[offset + 1]]);
            offset += 2;

            // Add or update device
            if self.get_device(&address).is_none() {
                let mut device = RemoteDevice::new(address);
                device.device_class = class;
                self.devices.push(device);
            }
        }
    }

    fn handle_inquiry_result_rssi(&mut self, params: &[u8]) {
        if params.is_empty() {
            return;
        }

        let num_responses = params[0] as usize;
        let mut offset = 1;

        for _ in 0..num_responses {
            if offset + 15 > params.len() {
                break;
            }

            let address = BdAddr::from_slice(&params[offset..]).unwrap();
            offset += 6;

            let _page_scan_rep = params[offset];
            offset += 1;

            // Reserved
            offset += 1;

            let class = DeviceClass([params[offset], params[offset + 1], params[offset + 2]]);
            offset += 3;

            let _clock_offset = u16::from_le_bytes([params[offset], params[offset + 1]]);
            offset += 2;

            let rssi = params[offset] as i8;
            offset += 1;

            // Add or update device
            if let Some(device) = self.get_device_mut(&address) {
                device.rssi = rssi;
            } else {
                let mut device = RemoteDevice::new(address);
                device.device_class = class;
                device.rssi = rssi;
                self.devices.push(device);
            }
        }
    }

    fn handle_extended_inquiry_result(&mut self, params: &[u8]) {
        if params.len() < 255 {
            return;
        }

        let _num_responses = params[0]; // Always 1 for extended
        let address = BdAddr::from_slice(&params[1..]).unwrap();
        let _page_scan_rep = params[7];
        // Reserved byte
        let class = DeviceClass([params[9], params[10], params[11]]);
        let _clock_offset = u16::from_le_bytes([params[12], params[13]]);
        let rssi = params[14] as i8;

        // Extended inquiry response data (240 bytes)
        let eir = &params[15..255];

        let mut name = None;

        // Parse EIR data
        let mut pos = 0;
        while pos < eir.len() {
            let len = eir[pos] as usize;
            if len == 0 || pos + 1 + len > eir.len() {
                break;
            }

            let data_type = eir[pos + 1];
            let data = &eir[pos + 2..pos + 1 + len];

            match data_type {
                0x08 | 0x09 => {
                    // Shortened or complete local name
                    if let Ok(s) = core::str::from_utf8(data) {
                        name = Some(String::from(s.trim_end_matches('\0')));
                    }
                }
                _ => {}
            }

            pos += 1 + len;
        }

        // Add or update device
        if let Some(device) = self.get_device_mut(&address) {
            device.rssi = rssi;
            if name.is_some() {
                device.name = name;
            }
        } else {
            let mut device = RemoteDevice::new(address);
            device.device_class = class;
            device.rssi = rssi;
            device.name = name;
            self.devices.push(device);
        }
    }

    fn handle_connection_complete(&mut self, params: &[u8]) {
        if params.len() < 11 {
            return;
        }

        let status = params[0];
        let _handle = u16::from_le_bytes([params[1], params[2]]);
        let address = BdAddr::from_slice(&params[3..]).unwrap();
        let link_type = params[9];

        if status == 0 {
            if let Some(device) = self.get_device_mut(&address) {
                device.connected = true;
                device.link_type = Some(match link_type {
                    0x00 => LinkType::Sco,
                    0x01 => LinkType::Acl,
                    0x02 => LinkType::Esco,
                    _ => LinkType::Acl,
                });
            }
            self.state = ControllerState::Connected;
        } else {
            self.state = ControllerState::Ready;
        }
    }

    fn handle_disconnection_complete(&mut self, params: &[u8]) {
        if params.len() < 4 {
            return;
        }

        let _status = params[0];
        let _handle = u16::from_le_bytes([params[1], params[2]]);
        let _reason = params[3];

        // Mark devices as disconnected
        for device in &mut self.devices {
            if device.connected {
                device.connected = false;
            }
        }

        self.state = ControllerState::Ready;
    }

    fn handle_remote_name(&mut self, params: &[u8]) {
        if params.len() < 255 {
            return;
        }

        let status = params[0];
        if status != 0 {
            return;
        }

        let address = BdAddr::from_slice(&params[1..]).unwrap();
        let name_bytes = &params[7..255];

        // Find null terminator
        let end = name_bytes.iter().position(|&b| b == 0).unwrap_or(name_bytes.len());
        let name = String::from(String::from_utf8_lossy(&name_bytes[..end]));

        if let Some(device) = self.get_device_mut(&address) {
            device.name = Some(name);
        }
    }

    fn handle_command_complete(&mut self, params: &[u8]) {
        if params.len() < 4 {
            return;
        }

        let _num_packets = params[0];
        let opcode = u16::from_le_bytes([params[1], params[2]]);
        let _status = params[3];
        let data = &params[4..];

        match opcode {
            hci::commands::READ_BD_ADDR => {
                if data.len() >= 6 {
                    self.address = BdAddr::from_slice(data).unwrap();
                }
            }
            hci::commands::READ_LOCAL_VERSION => {
                if data.len() >= 8 {
                    self.capabilities.hci_version = data[0];
                    self.capabilities.hci_revision = u16::from_le_bytes([data[1], data[2]]);
                    self.capabilities.lmp_version = data[3];
                    self.capabilities.manufacturer = u16::from_le_bytes([data[4], data[5]]);
                    self.capabilities.lmp_subversion = u16::from_le_bytes([data[6], data[7]]);
                }
            }
            hci::commands::READ_BUFFER_SIZE => {
                if data.len() >= 7 {
                    self.capabilities.max_acl_size = u16::from_le_bytes([data[0], data[1]]);
                    self.capabilities.max_sco_size = data[2];
                    self.capabilities.max_acl_packets = u16::from_le_bytes([data[3], data[4]]);
                    self.capabilities.max_sco_packets = u16::from_le_bytes([data[5], data[6]]);
                }
            }
            hci::commands::READ_LOCAL_FEATURES => {
                if data.len() >= 8 {
                    let features = u64::from_le_bytes([
                        data[0], data[1], data[2], data[3],
                        data[4], data[5], data[6], data[7],
                    ]);
                    self.capabilities.supports_sco = features & (1 << 0) != 0;
                    self.capabilities.supports_esco = features & (1 << 7) != 0;
                    self.capabilities.supports_secure_connections = features & (1 << 8) != 0;
                }
            }
            _ => {}
        }
    }

    fn handle_command_status(&mut self, params: &[u8]) {
        if params.len() < 4 {
            return;
        }

        let status = params[0];
        let _num_packets = params[1];
        let opcode = u16::from_le_bytes([params[2], params[3]]);

        if status != 0 {
            crate::kprintln!("bt: command {:04X} failed with status {:02X}", opcode, status);
        }
    }

    /// Set transport
    pub fn set_transport(&mut self, transport: usb_transport::UsbBluetoothTransport) {
        self.transport = Some(transport);
    }

    /// Check if transport is available
    pub fn has_transport(&self) -> bool {
        self.transport.is_some()
    }
}

/// Bluetooth manager
pub struct BluetoothManager {
    controllers: Vec<BluetoothController>,
    default_controller: Option<usize>,
    next_id: AtomicU8,
}

impl BluetoothManager {
    pub const fn new() -> Self {
        Self {
            controllers: Vec::new(),
            default_controller: None,
            next_id: AtomicU8::new(0),
        }
    }

    /// Register a new controller
    pub fn register_controller(&mut self, transport: usb_transport::UsbBluetoothTransport) -> u8 {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let mut controller = BluetoothController::new(id);
        controller.set_transport(transport);

        if self.default_controller.is_none() {
            self.default_controller = Some(self.controllers.len());
        }

        self.controllers.push(controller);
        id
    }

    /// Unregister controller
    pub fn unregister_controller(&mut self, id: u8) {
        if let Some(pos) = self.controllers.iter().position(|c| c.id == id) {
            self.controllers.remove(pos);
            if self.default_controller == Some(pos) {
                self.default_controller = if self.controllers.is_empty() {
                    None
                } else {
                    Some(0)
                };
            }
        }
    }

    /// Get controller by id
    pub fn get_controller(&mut self, id: u8) -> Option<&mut BluetoothController> {
        self.controllers.iter_mut().find(|c| c.id == id)
    }

    /// Get default controller
    pub fn default_controller(&mut self) -> Option<&mut BluetoothController> {
        self.default_controller.and_then(|i| self.controllers.get_mut(i))
    }

    /// List controllers
    pub fn list_controllers(&self) -> Vec<(u8, &str, ControllerState)> {
        self.controllers
            .iter()
            .map(|c| (c.id, c.name.as_str(), c.state))
            .collect()
    }

    /// Controller count
    pub fn controller_count(&self) -> usize {
        self.controllers.len()
    }
}

/// Global Bluetooth manager
pub static BLUETOOTH_MANAGER: TicketSpinlock<BluetoothManager> =
    TicketSpinlock::new(BluetoothManager::new());

/// Initialize Bluetooth subsystem
pub fn init() {
    crate::kprintln!("bluetooth: initializing subsystem");

    // Initialize pairing subsystem
    pairing::init();

    // Initialize HID profile
    hid::init();

    // Scan for USB Bluetooth adapters
    usb_transport::scan_usb_adapters();

    let manager = BLUETOOTH_MANAGER.lock();
    crate::kprintln!("bluetooth: {} controller(s) found", manager.controller_count());
}

/// Get controller count
pub fn controller_count() -> usize {
    BLUETOOTH_MANAGER.lock().controller_count()
}

/// Start scanning on default controller
pub fn start_scan() -> Result<(), &'static str> {
    let mut manager = BLUETOOTH_MANAGER.lock();
    let controller = manager.default_controller().ok_or("No controller")?;
    controller.start_scan(None)
}

/// Stop scanning on default controller
pub fn stop_scan() -> Result<(), &'static str> {
    let mut manager = BLUETOOTH_MANAGER.lock();
    let controller = manager.default_controller().ok_or("No controller")?;
    controller.stop_scan()
}

/// Get discovered devices
pub fn discovered_devices() -> Vec<(BdAddr, Option<String>, DeviceClass, i8)> {
    let manager = BLUETOOTH_MANAGER.lock();
    if let Some(idx) = manager.default_controller {
        manager.controllers.get(idx)
            .map(|c| {
                c.devices.iter()
                    .map(|d| (d.address, d.name.clone(), d.device_class, d.rssi))
                    .collect()
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    }
}

/// Format status for display
pub fn format_status() -> String {
    use core::fmt::Write;
    let mut output = String::new();
    let manager = BLUETOOTH_MANAGER.lock();

    writeln!(output, "Bluetooth Controllers: {}", manager.controllers.len()).ok();

    for (i, controller) in manager.controllers.iter().enumerate() {
        let default = manager.default_controller == Some(i);
        writeln!(output, "  [{} {}] {} - {:?}",
            controller.id,
            if default { "*" } else { " " },
            controller.address.to_string(),
            controller.state
        ).ok();

        if !controller.name.is_empty() {
            writeln!(output, "    Name: {}", controller.name).ok();
        }

        writeln!(output, "    Devices: {}", controller.devices.len()).ok();
        for device in &controller.devices {
            writeln!(output, "      {} {} ({}dBm) {}{}",
                device.address.to_string(),
                device.name.as_deref().unwrap_or("Unknown"),
                device.rssi,
                device.device_class.major_class_name(),
                if device.connected { " [Connected]" } else { "" }
            ).ok();
        }
    }

    // Add pairing status
    writeln!(output, "\nPairing:").ok();
    let storage = pairing::LINK_KEY_STORAGE.lock();
    writeln!(output, "  Paired devices: {}", storage.count()).ok();

    output
}

/// Pair with a discovered device
pub fn pair(address: &BdAddr) -> Result<(), &'static str> {
    let mut manager = BLUETOOTH_MANAGER.lock();
    let controller = manager.default_controller().ok_or("No controller")?;

    // First connect to the device
    controller.connect(address)?;

    // Pairing will be initiated automatically when connection completes
    Ok(())
}

/// Check if device is paired
pub fn is_paired(address: &BdAddr) -> bool {
    pairing::is_paired(address)
}

/// Unpair a device
pub fn unpair(address: &BdAddr) -> bool {
    pairing::unpair(address)
}

/// Get list of paired devices
pub fn paired_devices() -> Vec<(BdAddr, bool)> {
    pairing::paired_devices()
}
