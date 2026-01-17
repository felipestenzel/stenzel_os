//! WiFi Driver Infrastructure
//!
//! Hardware abstraction layer for WiFi drivers.

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use crate::util::{KResult, KError};
use crate::sync::IrqSafeMutex;
use super::mac::MacAddress;
use super::{WifiCapabilities, WifiChannel, WifiBand, ChannelWidth, WifiNetwork};

/// WiFi driver trait
pub trait WifiDriver: Send + Sync {
    /// Get driver name
    fn name(&self) -> &str;

    /// Get driver version
    fn version(&self) -> &str {
        "1.0.0"
    }

    /// Power on the radio
    fn power_on(&self) -> KResult<()>;

    /// Power off the radio
    fn power_off(&self) -> KResult<()>;

    /// Get power state
    fn is_powered(&self) -> bool;

    /// Get MAC address
    fn mac_address(&self) -> MacAddress;

    /// Get hardware capabilities
    fn capabilities(&self) -> WifiCapabilities;

    /// Set current channel
    fn set_channel(&self, channel: &WifiChannel) -> KResult<()>;

    /// Get current channel
    fn get_channel(&self) -> Option<WifiChannel>;

    /// Start scanning
    fn start_scan(&self) -> KResult<()>;

    /// Stop scanning
    fn stop_scan(&self) -> KResult<()>;

    /// Connect to network
    fn connect(&self, network: &WifiNetwork, password: Option<&str>) -> KResult<()>;

    /// Disconnect from network
    fn disconnect(&self) -> KResult<()>;

    /// Send raw frame
    fn send_frame(&self, frame: &[u8]) -> KResult<()>;

    /// Receive raw frame (non-blocking)
    fn recv_frame(&self) -> Option<Vec<u8>>;

    /// Get receive signal strength indicator (RSSI) in dBm
    fn get_rssi(&self) -> Option<i8>;

    /// Set transmit power (dBm)
    fn set_tx_power(&self, power: u8) -> KResult<()>;

    /// Get transmit power (dBm)
    fn get_tx_power(&self) -> u8;

    /// Check if connected
    fn is_connected(&self) -> bool;

    /// Get current BSSID
    fn current_bssid(&self) -> Option<MacAddress>;

    /// Handle interrupt
    fn handle_interrupt(&self) -> bool;

    /// Authenticate with AP (Open System or Shared Key)
    fn authenticate(&self, bssid: &MacAddress) -> KResult<()> {
        let _ = bssid;
        Err(KError::NotSupported)
    }

    /// Associate with AP
    fn associate(&self, bssid: &MacAddress, ssid: &str) -> KResult<()> {
        let _ = (bssid, ssid);
        Err(KError::NotSupported)
    }

    /// Deauthenticate from AP
    fn deauthenticate(&self, bssid: &MacAddress) -> KResult<()> {
        let _ = bssid;
        Err(KError::NotSupported)
    }

    /// Send EAPOL frame for WPA handshake
    fn send_eapol(&self, bssid: &MacAddress, data: &[u8]) -> KResult<()> {
        let _ = (bssid, data);
        Err(KError::NotSupported)
    }

    /// Install encryption key
    fn install_key(&self, key: &[u8], is_group: bool) -> KResult<()> {
        let _ = (key, is_group);
        Err(KError::NotSupported)
    }

    /// Remove encryption key
    fn remove_key(&self, is_group: bool) -> KResult<()> {
        let _ = is_group;
        Err(KError::NotSupported)
    }

    /// Get statistics
    fn statistics(&self) -> DriverStatistics {
        DriverStatistics::default()
    }

    /// Reset hardware
    fn reset(&self) -> KResult<()> {
        self.power_off()?;
        self.power_on()
    }

    /// Set promiscuous mode
    fn set_promiscuous(&self, _enabled: bool) -> KResult<()> {
        Err(KError::NotSupported)
    }

    /// Set monitor mode
    fn set_monitor_mode(&self, _enabled: bool) -> KResult<()> {
        Err(KError::NotSupported)
    }
}

/// Driver statistics
#[derive(Debug, Clone, Default)]
pub struct DriverStatistics {
    /// Frames transmitted
    pub tx_frames: u64,
    /// Frames received
    pub rx_frames: u64,
    /// Bytes transmitted
    pub tx_bytes: u64,
    /// Bytes received
    pub rx_bytes: u64,
    /// Transmit errors
    pub tx_errors: u64,
    /// Receive errors
    pub rx_errors: u64,
    /// Transmit retries
    pub tx_retries: u64,
    /// Dropped frames
    pub rx_dropped: u64,
    /// CRC errors
    pub rx_crc_errors: u64,
    /// Decryption errors
    pub rx_decrypt_errors: u64,
    /// Beacon count
    pub beacons: u64,
    /// Noise level (dBm)
    pub noise: i8,
}

/// Driver operations for async events
pub trait WifiDriverOps: WifiDriver {
    /// Called when scan is complete
    fn on_scan_complete(&self, results: &[WifiNetwork]) {
        let _ = results;
    }

    /// Called when connected to network
    fn on_connected(&self, bssid: MacAddress) {
        let _ = bssid;
    }

    /// Called when disconnected
    fn on_disconnected(&self, reason: u16) {
        let _ = reason;
    }

    /// Called when authentication completed
    fn on_auth_complete(&self, success: bool) {
        let _ = success;
    }

    /// Called when association completed
    fn on_assoc_complete(&self, success: bool, aid: u16) {
        let _ = (success, aid);
    }
}

/// Registered driver information
struct RegisteredDriver {
    /// Driver name
    name: String,
    /// PCI vendor ID
    vendor_id: u16,
    /// PCI device ID
    device_id: u16,
    /// Driver creation function
    create: fn(base_addr: usize) -> KResult<Arc<dyn WifiDriver>>,
}

/// Driver registry
static DRIVER_REGISTRY: IrqSafeMutex<Vec<RegisteredDriver>> = IrqSafeMutex::new(Vec::new());

/// Active drivers
static ACTIVE_DRIVERS: IrqSafeMutex<Vec<Arc<dyn WifiDriver>>> = IrqSafeMutex::new(Vec::new());

/// Register a WiFi driver
pub fn register_driver(
    name: &str,
    vendor_id: u16,
    device_id: u16,
    create: fn(base_addr: usize) -> KResult<Arc<dyn WifiDriver>>,
) {
    let mut registry = DRIVER_REGISTRY.lock();
    registry.push(RegisteredDriver {
        name: String::from(name),
        vendor_id,
        device_id,
        create,
    });
    crate::kprintln!("wifi: registered driver {} for {:04x}:{:04x}",
        name, vendor_id, device_id);
}

/// Find driver for PCI device
pub fn find_driver(vendor_id: u16, device_id: u16) -> Option<fn(usize) -> KResult<Arc<dyn WifiDriver>>> {
    let registry = DRIVER_REGISTRY.lock();
    for driver in registry.iter() {
        if driver.vendor_id == vendor_id && driver.device_id == device_id {
            return Some(driver.create);
        }
    }
    None
}

/// Probe and initialize driver for PCI device
pub fn probe_device(vendor_id: u16, device_id: u16, base_addr: usize) -> KResult<Arc<dyn WifiDriver>> {
    let create_fn = find_driver(vendor_id, device_id)
        .ok_or(KError::NotFound)?;

    let driver = create_fn(base_addr)?;

    // Add to active drivers
    let mut active = ACTIVE_DRIVERS.lock();
    active.push(driver.clone());

    crate::kprintln!("wifi: initialized {} driver", driver.name());

    Ok(driver)
}

/// Get all active drivers
pub fn active_drivers() -> Vec<Arc<dyn WifiDriver>> {
    ACTIVE_DRIVERS.lock().clone()
}

/// Get driver by name
pub fn get_driver(name: &str) -> Option<Arc<dyn WifiDriver>> {
    let active = ACTIVE_DRIVERS.lock();
    for driver in active.iter() {
        if driver.name() == name {
            return Some(driver.clone());
        }
    }
    None
}

/// Get first available driver
pub fn first_driver() -> Option<Arc<dyn WifiDriver>> {
    let active = ACTIVE_DRIVERS.lock();
    active.first().cloned()
}

/// Firmware loading support
pub mod firmware {
    use alloc::vec::Vec;
    use crate::util::{KResult, KError};

    /// Firmware information
    #[derive(Debug, Clone)]
    pub struct FirmwareInfo {
        pub name: &'static str,
        pub version: u32,
        pub size: usize,
    }

    /// Request firmware from filesystem
    pub fn request_firmware(name: &str) -> KResult<Vec<u8>> {
        // Try multiple paths
        let paths = [
            alloc::format!("/lib/firmware/{}", name),
            alloc::format!("/usr/lib/firmware/{}", name),
            alloc::format!("/boot/firmware/{}", name),
        ];

        let cred = crate::security::Cred::root();

        for path in &paths {
            if let Ok(data) = crate::fs::read_file(path, &cred) {
                crate::kprintln!("wifi: loaded firmware {} ({} bytes)", name, data.len());
                return Ok(data);
            }
        }

        crate::kprintln!("wifi: firmware {} not found", name);
        Err(KError::NotFound)
    }

    /// Release firmware (no-op, but for API compatibility)
    pub fn release_firmware(_data: Vec<u8>) {
        // Memory is automatically freed when Vec is dropped
    }

    /// Parse firmware header
    pub fn parse_header(data: &[u8]) -> Option<FirmwareInfo> {
        if data.len() < 16 {
            return None;
        }

        // Generic firmware header format
        // Actual format depends on driver
        Some(FirmwareInfo {
            name: "unknown",
            version: u32::from_le_bytes([data[0], data[1], data[2], data[3]]),
            size: data.len(),
        })
    }
}

/// Generic null driver for testing
pub struct NullWifiDriver {
    mac: MacAddress,
    powered: IrqSafeMutex<bool>,
    channel: IrqSafeMutex<Option<WifiChannel>>,
}

impl NullWifiDriver {
    pub fn new() -> Self {
        Self {
            mac: MacAddress::new([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]),
            powered: IrqSafeMutex::new(false),
            channel: IrqSafeMutex::new(None),
        }
    }
}

impl WifiDriver for NullWifiDriver {
    fn name(&self) -> &str {
        "null"
    }

    fn power_on(&self) -> KResult<()> {
        *self.powered.lock() = true;
        Ok(())
    }

    fn power_off(&self) -> KResult<()> {
        *self.powered.lock() = false;
        Ok(())
    }

    fn is_powered(&self) -> bool {
        *self.powered.lock()
    }

    fn mac_address(&self) -> MacAddress {
        self.mac
    }

    fn capabilities(&self) -> WifiCapabilities {
        WifiCapabilities {
            bands: alloc::vec![WifiBand::Band2_4GHz, WifiBand::Band5GHz],
            max_scan_ssids: 4,
            max_sched_scan_ssids: 0,
            signal_type: 0,
            supports_ap: false,
            supports_p2p: false,
            supports_monitor: false,
            supports_mesh: false,
        }
    }

    fn set_channel(&self, channel: &WifiChannel) -> KResult<()> {
        *self.channel.lock() = Some(channel.clone());
        Ok(())
    }

    fn get_channel(&self) -> Option<WifiChannel> {
        self.channel.lock().clone()
    }

    fn start_scan(&self) -> KResult<()> {
        Ok(())
    }

    fn stop_scan(&self) -> KResult<()> {
        Ok(())
    }

    fn connect(&self, _network: &WifiNetwork, _password: Option<&str>) -> KResult<()> {
        Err(KError::NotSupported)
    }

    fn disconnect(&self) -> KResult<()> {
        Ok(())
    }

    fn send_frame(&self, _frame: &[u8]) -> KResult<()> {
        Ok(())
    }

    fn recv_frame(&self) -> Option<Vec<u8>> {
        None
    }

    fn get_rssi(&self) -> Option<i8> {
        None
    }

    fn set_tx_power(&self, _power: u8) -> KResult<()> {
        Ok(())
    }

    fn get_tx_power(&self) -> u8 {
        20
    }

    fn is_connected(&self) -> bool {
        false
    }

    fn current_bssid(&self) -> Option<MacAddress> {
        None
    }

    fn handle_interrupt(&self) -> bool {
        false
    }
}

impl Default for NullWifiDriver {
    fn default() -> Self {
        Self::new()
    }
}

/// Common register access helpers for MMIO
pub mod mmio {
    use core::ptr::{read_volatile, write_volatile};

    /// Read 32-bit register
    #[inline]
    pub unsafe fn read32(base: usize, offset: usize) -> u32 {
        read_volatile((base + offset) as *const u32)
    }

    /// Write 32-bit register
    #[inline]
    pub unsafe fn write32(base: usize, offset: usize, value: u32) {
        write_volatile((base + offset) as *mut u32, value);
    }

    /// Read 16-bit register
    #[inline]
    pub unsafe fn read16(base: usize, offset: usize) -> u16 {
        read_volatile((base + offset) as *const u16)
    }

    /// Write 16-bit register
    #[inline]
    pub unsafe fn write16(base: usize, offset: usize, value: u16) {
        write_volatile((base + offset) as *mut u16, value);
    }

    /// Read 8-bit register
    #[inline]
    pub unsafe fn read8(base: usize, offset: usize) -> u8 {
        read_volatile((base + offset) as *const u8)
    }

    /// Write 8-bit register
    #[inline]
    pub unsafe fn write8(base: usize, offset: usize, value: u8) {
        write_volatile((base + offset) as *mut u8, value);
    }

    /// Set bits in register
    #[inline]
    pub unsafe fn set_bits32(base: usize, offset: usize, bits: u32) {
        let val = read32(base, offset);
        write32(base, offset, val | bits);
    }

    /// Clear bits in register
    #[inline]
    pub unsafe fn clear_bits32(base: usize, offset: usize, bits: u32) {
        let val = read32(base, offset);
        write32(base, offset, val & !bits);
    }

    /// Wait for bit to be set
    #[inline]
    pub unsafe fn wait_for_bit(base: usize, offset: usize, bit: u32, timeout_us: u32) -> bool {
        for _ in 0..timeout_us {
            if read32(base, offset) & bit != 0 {
                return true;
            }
            // Simple delay
            for _ in 0..100 {
                core::hint::spin_loop();
            }
        }
        false
    }

    /// Wait for bit to be clear
    #[inline]
    pub unsafe fn wait_for_bit_clear(base: usize, offset: usize, bit: u32, timeout_us: u32) -> bool {
        for _ in 0..timeout_us {
            if read32(base, offset) & bit == 0 {
                return true;
            }
            for _ in 0..100 {
                core::hint::spin_loop();
            }
        }
        false
    }
}

/// DMA ring buffer helpers
pub mod dma {
    use alloc::vec::Vec;

    /// DMA descriptor
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct DmaDescriptor {
        pub addr_low: u32,
        pub addr_high: u32,
        pub len: u32,
        pub flags: u32,
    }

    impl DmaDescriptor {
        pub fn set_address(&mut self, addr: u64) {
            self.addr_low = addr as u32;
            self.addr_high = (addr >> 32) as u32;
        }

        pub fn address(&self) -> u64 {
            (self.addr_low as u64) | ((self.addr_high as u64) << 32)
        }
    }

    /// DMA ring buffer
    pub struct DmaRing {
        descriptors: Vec<DmaDescriptor>,
        buffers: Vec<Vec<u8>>,
        head: usize,
        tail: usize,
        size: usize,
    }

    impl DmaRing {
        pub fn new(size: usize, buffer_size: usize) -> Self {
            let mut descriptors = Vec::with_capacity(size);
            let mut buffers = Vec::with_capacity(size);

            for _ in 0..size {
                descriptors.push(DmaDescriptor::default());
                buffers.push(alloc::vec![0u8; buffer_size]);
            }

            Self {
                descriptors,
                buffers,
                head: 0,
                tail: 0,
                size,
            }
        }

        pub fn descriptors_ptr(&self) -> *const DmaDescriptor {
            self.descriptors.as_ptr()
        }

        pub fn buffer_addr(&self, index: usize) -> usize {
            self.buffers[index].as_ptr() as usize
        }

        pub fn is_empty(&self) -> bool {
            self.head == self.tail
        }

        pub fn is_full(&self) -> bool {
            (self.head + 1) % self.size == self.tail
        }

        pub fn advance_head(&mut self) {
            self.head = (self.head + 1) % self.size;
        }

        pub fn advance_tail(&mut self) {
            self.tail = (self.tail + 1) % self.size;
        }
    }
}
