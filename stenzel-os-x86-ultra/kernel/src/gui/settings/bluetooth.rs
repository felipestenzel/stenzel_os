//! Bluetooth Settings
//!
//! Bluetooth device pairing, management, and configuration.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use spin::Mutex;

/// Global Bluetooth settings state
static BLUETOOTH_SETTINGS: Mutex<Option<BluetoothSettings>> = Mutex::new(None);

/// Bluetooth settings state
pub struct BluetoothSettings {
    /// Bluetooth enabled
    pub enabled: bool,
    /// Discoverable
    pub discoverable: bool,
    /// Device name
    pub device_name: String,
    /// Paired devices
    pub paired_devices: Vec<BluetoothDevice>,
    /// Available devices
    pub available_devices: Vec<BluetoothDevice>,
    /// Currently scanning
    pub scanning: bool,
}

/// Bluetooth device
#[derive(Debug, Clone)]
pub struct BluetoothDevice {
    /// Device address
    pub address: String,
    /// Device name
    pub name: String,
    /// Device type
    pub device_type: BluetoothDeviceType,
    /// Is paired
    pub paired: bool,
    /// Is connected
    pub connected: bool,
    /// Is trusted
    pub trusted: bool,
    /// Battery level (if available)
    pub battery_level: Option<u8>,
    /// Signal strength (RSSI)
    pub rssi: Option<i8>,
}

/// Bluetooth device type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BluetoothDeviceType {
    /// Unknown device
    Unknown,
    /// Computer
    Computer,
    /// Phone
    Phone,
    /// Headset
    Headset,
    /// Headphones
    Headphones,
    /// Speaker
    Speaker,
    /// Keyboard
    Keyboard,
    /// Mouse
    Mouse,
    /// Gamepad
    Gamepad,
    /// Watch
    Watch,
    /// Printer
    Printer,
    /// Camera
    Camera,
}

impl BluetoothDeviceType {
    pub fn name(&self) -> &'static str {
        match self {
            BluetoothDeviceType::Unknown => "Unknown",
            BluetoothDeviceType::Computer => "Computer",
            BluetoothDeviceType::Phone => "Phone",
            BluetoothDeviceType::Headset => "Headset",
            BluetoothDeviceType::Headphones => "Headphones",
            BluetoothDeviceType::Speaker => "Speaker",
            BluetoothDeviceType::Keyboard => "Keyboard",
            BluetoothDeviceType::Mouse => "Mouse",
            BluetoothDeviceType::Gamepad => "Gamepad",
            BluetoothDeviceType::Watch => "Watch",
            BluetoothDeviceType::Printer => "Printer",
            BluetoothDeviceType::Camera => "Camera",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            BluetoothDeviceType::Unknown => "bluetooth",
            BluetoothDeviceType::Computer => "computer",
            BluetoothDeviceType::Phone => "phone",
            BluetoothDeviceType::Headset => "audio-headset",
            BluetoothDeviceType::Headphones => "audio-headphones",
            BluetoothDeviceType::Speaker => "audio-speakers",
            BluetoothDeviceType::Keyboard => "input-keyboard",
            BluetoothDeviceType::Mouse => "input-mouse",
            BluetoothDeviceType::Gamepad => "input-gaming",
            BluetoothDeviceType::Watch => "watch",
            BluetoothDeviceType::Printer => "printer",
            BluetoothDeviceType::Camera => "camera-photo",
        }
    }
}

/// Initialize Bluetooth settings
pub fn init() {
    let mut state = BLUETOOTH_SETTINGS.lock();
    if state.is_some() {
        return;
    }

    *state = Some(BluetoothSettings {
        enabled: false,
        discoverable: false,
        device_name: "Stenzel OS".to_string(),
        paired_devices: Vec::new(),
        available_devices: Vec::new(),
        scanning: false,
    });

    crate::kprintln!("bluetooth settings: initialized");
}

/// Set Bluetooth enabled
pub fn set_enabled(enabled: bool) {
    let mut state = BLUETOOTH_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.enabled = enabled;
        if !enabled {
            s.discoverable = false;
            s.scanning = false;
        }
    }
}

/// Is Bluetooth enabled
pub fn is_enabled() -> bool {
    let state = BLUETOOTH_SETTINGS.lock();
    state.as_ref().map(|s| s.enabled).unwrap_or(false)
}

/// Set discoverable
pub fn set_discoverable(discoverable: bool) {
    let mut state = BLUETOOTH_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        if s.enabled {
            s.discoverable = discoverable;
        }
    }
}

/// Is discoverable
pub fn is_discoverable() -> bool {
    let state = BLUETOOTH_SETTINGS.lock();
    state.as_ref().map(|s| s.discoverable).unwrap_or(false)
}

/// Set device name
pub fn set_device_name(name: &str) {
    let mut state = BLUETOOTH_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.device_name = name.to_string();
    }
}

/// Get device name
pub fn get_device_name() -> String {
    let state = BLUETOOTH_SETTINGS.lock();
    state.as_ref().map(|s| s.device_name.clone()).unwrap_or_default()
}

/// Get paired devices
pub fn get_paired_devices() -> Vec<BluetoothDevice> {
    let state = BLUETOOTH_SETTINGS.lock();
    state.as_ref().map(|s| s.paired_devices.clone()).unwrap_or_default()
}

/// Get available devices
pub fn get_available_devices() -> Vec<BluetoothDevice> {
    let state = BLUETOOTH_SETTINGS.lock();
    state.as_ref().map(|s| s.available_devices.clone()).unwrap_or_default()
}

/// Start scanning
pub fn start_scan() -> Result<(), BluetoothError> {
    let mut state = BLUETOOTH_SETTINGS.lock();
    let state = state.as_mut().ok_or(BluetoothError::NotInitialized)?;

    if !state.enabled {
        return Err(BluetoothError::Disabled);
    }

    state.scanning = true;
    state.available_devices.clear();

    // TODO: Actually start Bluetooth scan via driver

    Ok(())
}

/// Stop scanning
pub fn stop_scan() {
    let mut state = BLUETOOTH_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.scanning = false;
    }
}

/// Is scanning
pub fn is_scanning() -> bool {
    let state = BLUETOOTH_SETTINGS.lock();
    state.as_ref().map(|s| s.scanning).unwrap_or(false)
}

/// Pair device
pub fn pair_device(address: &str) -> Result<(), BluetoothError> {
    let mut state = BLUETOOTH_SETTINGS.lock();
    let state = state.as_mut().ok_or(BluetoothError::NotInitialized)?;

    if !state.enabled {
        return Err(BluetoothError::Disabled);
    }

    let device = state.available_devices.iter()
        .find(|d| d.address == address)
        .ok_or(BluetoothError::DeviceNotFound)?
        .clone();

    // TODO: Actually pair via Bluetooth driver

    let mut paired_device = device;
    paired_device.paired = true;
    state.paired_devices.push(paired_device);

    Ok(())
}

/// Unpair device
pub fn unpair_device(address: &str) -> Result<(), BluetoothError> {
    let mut state = BLUETOOTH_SETTINGS.lock();
    let state = state.as_mut().ok_or(BluetoothError::NotInitialized)?;

    let idx = state.paired_devices.iter()
        .position(|d| d.address == address)
        .ok_or(BluetoothError::DeviceNotFound)?;

    // Disconnect first if connected
    if state.paired_devices[idx].connected {
        state.paired_devices[idx].connected = false;
    }

    state.paired_devices.remove(idx);

    Ok(())
}

/// Connect device
pub fn connect_device(address: &str) -> Result<(), BluetoothError> {
    let mut state = BLUETOOTH_SETTINGS.lock();
    let state = state.as_mut().ok_or(BluetoothError::NotInitialized)?;

    if !state.enabled {
        return Err(BluetoothError::Disabled);
    }

    let device = state.paired_devices.iter_mut()
        .find(|d| d.address == address)
        .ok_or(BluetoothError::DeviceNotFound)?;

    // TODO: Actually connect via Bluetooth driver
    device.connected = true;

    Ok(())
}

/// Disconnect device
pub fn disconnect_device(address: &str) -> Result<(), BluetoothError> {
    let mut state = BLUETOOTH_SETTINGS.lock();
    let state = state.as_mut().ok_or(BluetoothError::NotInitialized)?;

    let device = state.paired_devices.iter_mut()
        .find(|d| d.address == address)
        .ok_or(BluetoothError::DeviceNotFound)?;

    device.connected = false;

    Ok(())
}

/// Set device trusted
pub fn set_device_trusted(address: &str, trusted: bool) -> Result<(), BluetoothError> {
    let mut state = BLUETOOTH_SETTINGS.lock();
    let state = state.as_mut().ok_or(BluetoothError::NotInitialized)?;

    let device = state.paired_devices.iter_mut()
        .find(|d| d.address == address)
        .ok_or(BluetoothError::DeviceNotFound)?;

    device.trusted = trusted;

    Ok(())
}

/// Bluetooth error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BluetoothError {
    NotInitialized,
    Disabled,
    DeviceNotFound,
    PairingFailed,
    ConnectionFailed,
}
