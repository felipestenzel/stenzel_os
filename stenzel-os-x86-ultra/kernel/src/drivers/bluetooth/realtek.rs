//! Realtek Bluetooth driver.
//!
//! Supports common Realtek Bluetooth chipsets:
//! - RTL8761B/RTL8761BU (USB Bluetooth 5.0)
//! - RTL8821C/RTL8821CU (USB Bluetooth 5.0)
//! - RTL8822C/RTL8822CU (USB Bluetooth 5.0)
//! - RTL8852A/RTL8852AU (USB Bluetooth 5.2)
//! - RTL8852B/RTL8852BU (USB Bluetooth 5.2)
//! - RTL8852C/RTL8852CU (USB Bluetooth 5.3)

use alloc::vec::Vec;
use alloc::vec;
use alloc::string::String;
use core::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use crate::sync::IrqSafeMutex;
use crate::util::{KResult, KError};

/// Realtek Bluetooth vendor/product IDs
mod device_ids {
    pub const REALTEK_VENDOR: u16 = 0x0bda;

    // RTL8761B (Bluetooth 5.0)
    pub const RTL8761B: u16 = 0x8771;
    pub const RTL8761BU: u16 = 0xb009;
    pub const RTL8761BUV: u16 = 0xb00a;

    // RTL8821C (Bluetooth 5.0)
    pub const RTL8821C: u16 = 0xb00c;
    pub const RTL8821CU: u16 = 0xc821;

    // RTL8822C (Bluetooth 5.0)
    pub const RTL8822C: u16 = 0xb00e;
    pub const RTL8822CU: u16 = 0xc822;
    pub const RTL8822CE: u16 = 0xc82f;

    // RTL8852A (Bluetooth 5.2)
    pub const RTL8852A: u16 = 0x8852;
    pub const RTL8852AU: u16 = 0x2852;
    pub const RTL8852AE: u16 = 0x8a5e;

    // RTL8852B (Bluetooth 5.2)
    pub const RTL8852B: u16 = 0x885b;
    pub const RTL8852BU: u16 = 0x385b;
    pub const RTL8852BE: u16 = 0x887b;

    // RTL8852C (Bluetooth 5.3)
    pub const RTL8852C: u16 = 0x885c;
    pub const RTL8852CU: u16 = 0x385c;
    pub const RTL8852CE: u16 = 0xc852;

    pub fn is_supported(vendor: u16, product: u16) -> bool {
        vendor == REALTEK_VENDOR && matches!(
            product,
            RTL8761B | RTL8761BU | RTL8761BUV |
            RTL8821C | RTL8821CU |
            RTL8822C | RTL8822CU | RTL8822CE |
            RTL8852A | RTL8852AU | RTL8852AE |
            RTL8852B | RTL8852BU | RTL8852BE |
            RTL8852C | RTL8852CU | RTL8852CE
        )
    }

    pub fn chip_name(product: u16) -> &'static str {
        match product {
            RTL8761B | RTL8761BU | RTL8761BUV => "RTL8761B",
            RTL8821C | RTL8821CU => "RTL8821C",
            RTL8822C | RTL8822CU | RTL8822CE => "RTL8822C",
            RTL8852A | RTL8852AU | RTL8852AE => "RTL8852A",
            RTL8852B | RTL8852BU | RTL8852BE => "RTL8852B",
            RTL8852C | RTL8852CU | RTL8852CE => "RTL8852C",
            _ => "Unknown",
        }
    }

    pub fn bluetooth_version(product: u16) -> &'static str {
        match product {
            RTL8761B | RTL8761BU | RTL8761BUV |
            RTL8821C | RTL8821CU |
            RTL8822C | RTL8822CU | RTL8822CE => "5.0",
            RTL8852A | RTL8852AU | RTL8852AE |
            RTL8852B | RTL8852BU | RTL8852BE => "5.2",
            RTL8852C | RTL8852CU | RTL8852CE => "5.3",
            _ => "Unknown",
        }
    }
}

/// Realtek HCI vendor commands
mod hci_vendor {
    // Realtek vendor opcodes
    pub const RTK_DOWNLOAD_FW: u16 = 0xfc20;
    pub const RTK_READ_ROM_VERSION: u16 = 0xfc6d;
    pub const RTK_READ_CHIP_TYPE: u16 = 0xfc61;
    pub const RTK_CONFIG_CHANGE: u16 = 0xfc17;
    pub const RTK_SET_TX_POWER: u16 = 0xfc0a;
    pub const RTK_READ_LOCAL_VERSION: u16 = 0xfc10;

    // Download fragment sizes
    pub const FW_FRAGMENT_SIZE: usize = 252;
    pub const FW_FRAGMENT_MAX: usize = 0x7f;

    // Firmware file extensions
    pub const FW_EXT: &str = "bin";
    pub const CONFIG_EXT: &str = "config";
}

/// Chip type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipType {
    Rtl8761b,
    Rtl8821c,
    Rtl8822c,
    Rtl8852a,
    Rtl8852b,
    Rtl8852c,
    Unknown,
}

impl ChipType {
    pub fn from_product_id(product: u16) -> Self {
        use device_ids::*;
        match product {
            RTL8761B | RTL8761BU | RTL8761BUV => ChipType::Rtl8761b,
            RTL8821C | RTL8821CU => ChipType::Rtl8821c,
            RTL8822C | RTL8822CU | RTL8822CE => ChipType::Rtl8822c,
            RTL8852A | RTL8852AU | RTL8852AE => ChipType::Rtl8852a,
            RTL8852B | RTL8852BU | RTL8852BE => ChipType::Rtl8852b,
            RTL8852C | RTL8852CU | RTL8852CE => ChipType::Rtl8852c,
            _ => ChipType::Unknown,
        }
    }

    pub fn firmware_name(&self) -> &'static str {
        match self {
            ChipType::Rtl8761b => "rtl8761b_fw",
            ChipType::Rtl8821c => "rtl8821c_fw",
            ChipType::Rtl8822c => "rtl8822c_fw",
            ChipType::Rtl8852a => "rtl8852a_fw",
            ChipType::Rtl8852b => "rtl8852b_fw",
            ChipType::Rtl8852c => "rtl8852c_fw",
            ChipType::Unknown => "unknown",
        }
    }

    pub fn config_name(&self) -> &'static str {
        match self {
            ChipType::Rtl8761b => "rtl8761b_config",
            ChipType::Rtl8821c => "rtl8821c_config",
            ChipType::Rtl8822c => "rtl8822c_config",
            ChipType::Rtl8852a => "rtl8852a_config",
            ChipType::Rtl8852b => "rtl8852b_config",
            ChipType::Rtl8852c => "rtl8852c_config",
            ChipType::Unknown => "unknown",
        }
    }

    pub fn supports_le_extended(&self) -> bool {
        matches!(self, ChipType::Rtl8852a | ChipType::Rtl8852b | ChipType::Rtl8852c)
    }

    pub fn supports_le_coded_phy(&self) -> bool {
        matches!(self, ChipType::Rtl8852a | ChipType::Rtl8852b | ChipType::Rtl8852c)
    }
}

/// Driver state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverState {
    Uninitialized,
    FirmwareLoading,
    ConfigLoading,
    Initializing,
    Ready,
    Error,
}

/// Bluetooth address
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BdAddr(pub [u8; 6]);

impl BdAddr {
    pub fn new(addr: [u8; 6]) -> Self {
        Self(addr)
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 6]
    }

    pub fn to_string(&self) -> String {
        use core::fmt::Write;
        let mut s = String::with_capacity(17);
        for (i, b) in self.0.iter().rev().enumerate() {
            if i > 0 { s.push(':'); }
            let _ = write!(s, "{:02X}", b);
        }
        s
    }
}

/// Firmware info
#[derive(Debug, Clone)]
pub struct FirmwareInfo {
    pub version: u32,
    pub subversion: u16,
    pub build: u8,
    pub rom_version: u8,
    pub loaded: bool,
}

impl Default for FirmwareInfo {
    fn default() -> Self {
        Self {
            version: 0,
            subversion: 0,
            build: 0,
            rom_version: 0,
            loaded: false,
        }
    }
}

/// Realtek Bluetooth device
pub struct RealtekBtDevice {
    /// USB device handle
    usb_device: u32,
    /// Chip type
    chip_type: ChipType,
    /// Product ID
    product_id: u16,
    /// Driver state
    state: DriverState,
    /// Bluetooth address
    bd_addr: BdAddr,
    /// Firmware info
    fw_info: FirmwareInfo,
    /// HCI version
    hci_version: u8,
    /// LMP version
    lmp_version: u8,
    /// Bulk IN endpoint
    bulk_in: u8,
    /// Bulk OUT endpoint
    bulk_out: u8,
    /// Interrupt endpoint
    interrupt_ep: u8,
    /// Command buffer
    cmd_buffer: Vec<u8>,
    /// Event buffer
    event_buffer: Vec<u8>,
    /// ACL buffer
    acl_buffer: Vec<u8>,
}

impl RealtekBtDevice {
    fn new(usb_device: u32, product_id: u16, bulk_in: u8, bulk_out: u8, interrupt_ep: u8) -> Self {
        Self {
            usb_device,
            chip_type: ChipType::from_product_id(product_id),
            product_id,
            state: DriverState::Uninitialized,
            bd_addr: BdAddr::default(),
            fw_info: FirmwareInfo::default(),
            hci_version: 0,
            lmp_version: 0,
            bulk_in,
            bulk_out,
            interrupt_ep,
            cmd_buffer: vec![0u8; 260],
            event_buffer: vec![0u8; 260],
            acl_buffer: vec![0u8; 4096],
        }
    }

    /// Initialize the device
    pub fn init(&mut self) -> KResult<()> {
        self.state = DriverState::FirmwareLoading;

        // Read ROM version
        self.read_rom_version()?;

        // Download firmware
        self.download_firmware()?;

        self.state = DriverState::ConfigLoading;

        // Download config
        self.download_config()?;

        self.state = DriverState::Initializing;

        // Reset controller
        self.hci_reset()?;

        // Read local version
        self.read_local_version()?;

        // Read BD address
        self.read_bd_address()?;

        self.state = DriverState::Ready;
        Ok(())
    }

    /// Read ROM version
    fn read_rom_version(&mut self) -> KResult<()> {
        // Build HCI command
        let opcode = hci_vendor::RTK_READ_ROM_VERSION;
        self.cmd_buffer[0] = (opcode & 0xff) as u8;
        self.cmd_buffer[1] = ((opcode >> 8) & 0xff) as u8;
        self.cmd_buffer[2] = 0; // No parameters

        // In real implementation, would send HCI command and read response
        // let resp = self.send_hci_command(&self.cmd_buffer[..3])?;
        // self.fw_info.rom_version = resp[6];

        self.fw_info.rom_version = 1; // Placeholder
        Ok(())
    }

    /// Download firmware
    fn download_firmware(&mut self) -> KResult<()> {
        // In real implementation, would:
        // 1. Read firmware from filesystem based on chip type
        // 2. Parse firmware header
        // 3. Download firmware in fragments using RTK_DOWNLOAD_FW command

        // For now, simulate firmware loaded
        self.fw_info.loaded = true;
        self.fw_info.version = 0x8761_0001; // Example version
        Ok(())
    }

    /// Download config
    fn download_config(&mut self) -> KResult<()> {
        // In real implementation, would:
        // 1. Read config from filesystem based on chip type
        // 2. Parse config data
        // 3. Apply configuration patches

        Ok(())
    }

    /// HCI reset
    fn hci_reset(&mut self) -> KResult<()> {
        // HCI_Reset opcode: 0x0c03
        let opcode: u16 = 0x0c03;
        self.cmd_buffer[0] = (opcode & 0xff) as u8;
        self.cmd_buffer[1] = ((opcode >> 8) & 0xff) as u8;
        self.cmd_buffer[2] = 0; // No parameters

        // In real implementation, would send HCI command
        // self.send_hci_command(&self.cmd_buffer[..3])?;

        Ok(())
    }

    /// Read local version
    fn read_local_version(&mut self) -> KResult<()> {
        // HCI_Read_Local_Version_Information opcode: 0x1001
        let opcode: u16 = 0x1001;
        self.cmd_buffer[0] = (opcode & 0xff) as u8;
        self.cmd_buffer[1] = ((opcode >> 8) & 0xff) as u8;
        self.cmd_buffer[2] = 0; // No parameters

        // In real implementation, would send HCI command and parse response
        // Placeholder values for Bluetooth 5.x
        self.hci_version = 12; // Bluetooth 5.3
        self.lmp_version = 12;

        Ok(())
    }

    /// Read BD address
    fn read_bd_address(&mut self) -> KResult<()> {
        // HCI_Read_BD_Addr opcode: 0x1009
        let opcode: u16 = 0x1009;
        self.cmd_buffer[0] = (opcode & 0xff) as u8;
        self.cmd_buffer[1] = ((opcode >> 8) & 0xff) as u8;
        self.cmd_buffer[2] = 0; // No parameters

        // In real implementation, would send HCI command and parse response
        // Placeholder address
        self.bd_addr = BdAddr::new([0x00, 0x1A, 0x7D, 0xDA, 0x71, 0x13]);

        Ok(())
    }

    /// Send HCI command
    pub fn send_hci_command(&mut self, data: &[u8]) -> KResult<Vec<u8>> {
        if self.state != DriverState::Ready && self.state != DriverState::Initializing {
            return Err(KError::Busy);
        }

        // In real implementation, would send via USB bulk OUT
        // and receive response via interrupt endpoint

        Ok(Vec::new())
    }

    /// Send ACL data
    pub fn send_acl(&mut self, data: &[u8]) -> KResult<()> {
        if self.state != DriverState::Ready {
            return Err(KError::Busy);
        }

        if data.len() > self.acl_buffer.len() {
            return Err(KError::Invalid);
        }

        // In real implementation, would send via USB bulk OUT

        Ok(())
    }

    /// Receive ACL data
    pub fn recv_acl(&mut self) -> Option<Vec<u8>> {
        if self.state != DriverState::Ready {
            return None;
        }

        // In real implementation, would receive via USB bulk IN

        None
    }

    /// Get chip type
    pub fn chip_type(&self) -> ChipType {
        self.chip_type
    }

    /// Get state
    pub fn state(&self) -> DriverState {
        self.state
    }

    /// Get BD address
    pub fn bd_addr(&self) -> BdAddr {
        self.bd_addr
    }

    /// Get firmware info
    pub fn fw_info(&self) -> &FirmwareInfo {
        &self.fw_info
    }

    /// Is ready
    pub fn is_ready(&self) -> bool {
        self.state == DriverState::Ready
    }

    /// Get chip name
    pub fn chip_name(&self) -> &'static str {
        device_ids::chip_name(self.product_id)
    }

    /// Get Bluetooth version string
    pub fn bt_version(&self) -> &'static str {
        device_ids::bluetooth_version(self.product_id)
    }
}

/// Driver statistics
#[derive(Debug)]
pub struct RealtekBtStats {
    pub commands_sent: AtomicU64,
    pub events_received: AtomicU64,
    pub acl_tx_packets: AtomicU64,
    pub acl_rx_packets: AtomicU64,
    pub acl_tx_bytes: AtomicU64,
    pub acl_rx_bytes: AtomicU64,
    pub errors: AtomicU64,
}

impl RealtekBtStats {
    const fn new() -> Self {
        Self {
            commands_sent: AtomicU64::new(0),
            events_received: AtomicU64::new(0),
            acl_tx_packets: AtomicU64::new(0),
            acl_rx_packets: AtomicU64::new(0),
            acl_tx_bytes: AtomicU64::new(0),
            acl_rx_bytes: AtomicU64::new(0),
            errors: AtomicU64::new(0),
        }
    }

    pub fn snapshot(&self) -> RealtekBtStatsSnapshot {
        RealtekBtStatsSnapshot {
            commands_sent: self.commands_sent.load(Ordering::Relaxed),
            events_received: self.events_received.load(Ordering::Relaxed),
            acl_tx_packets: self.acl_tx_packets.load(Ordering::Relaxed),
            acl_rx_packets: self.acl_rx_packets.load(Ordering::Relaxed),
            acl_tx_bytes: self.acl_tx_bytes.load(Ordering::Relaxed),
            acl_rx_bytes: self.acl_rx_bytes.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RealtekBtStatsSnapshot {
    pub commands_sent: u64,
    pub events_received: u64,
    pub acl_tx_packets: u64,
    pub acl_rx_packets: u64,
    pub acl_tx_bytes: u64,
    pub acl_rx_bytes: u64,
    pub errors: u64,
}

/// Realtek Bluetooth manager
pub struct RealtekBtManager {
    devices: Vec<RealtekBtDevice>,
    active_device: Option<usize>,
    stats: RealtekBtStats,
    initialized: bool,
}

impl RealtekBtManager {
    const fn new() -> Self {
        Self {
            devices: Vec::new(),
            active_device: None,
            stats: RealtekBtStats::new(),
            initialized: false,
        }
    }

    /// Initialize
    pub fn init(&mut self) {
        if self.initialized {
            return;
        }

        self.initialized = true;
    }

    /// Register a device
    pub fn register_device(
        &mut self,
        vendor_id: u16,
        product_id: u16,
        usb_device: u32,
        bulk_in: u8,
        bulk_out: u8,
        interrupt_ep: u8,
    ) -> Option<usize> {
        if !device_ids::is_supported(vendor_id, product_id) {
            return None;
        }

        let mut device = RealtekBtDevice::new(
            usb_device,
            product_id,
            bulk_in,
            bulk_out,
            interrupt_ep,
        );

        // Initialize device
        if device.init().is_err() {
            return None;
        }

        let idx = self.devices.len();
        self.devices.push(device);

        if self.active_device.is_none() {
            self.active_device = Some(idx);
        }

        Some(idx)
    }

    /// Unregister a device
    pub fn unregister_device(&mut self, idx: usize) {
        if idx < self.devices.len() {
            self.devices.remove(idx);

            if let Some(active) = self.active_device {
                if active == idx {
                    self.active_device = if self.devices.is_empty() {
                        None
                    } else {
                        Some(0)
                    };
                } else if active > idx {
                    self.active_device = Some(active - 1);
                }
            }
        }
    }

    /// Get active device
    pub fn active_device(&self) -> Option<&RealtekBtDevice> {
        self.active_device.and_then(|idx| self.devices.get(idx))
    }

    /// Get active device mutable
    pub fn active_device_mut(&mut self) -> Option<&mut RealtekBtDevice> {
        self.active_device.and_then(|idx| self.devices.get_mut(idx))
    }

    /// Set active device
    pub fn set_active_device(&mut self, idx: usize) -> bool {
        if idx < self.devices.len() {
            self.active_device = Some(idx);
            true
        } else {
            false
        }
    }

    /// Send HCI command
    pub fn send_command(&mut self, data: &[u8]) -> KResult<Vec<u8>> {
        if let Some(device) = self.active_device_mut() {
            let result = device.send_hci_command(data);
            if result.is_ok() {
                self.stats.commands_sent.fetch_add(1, Ordering::Relaxed);
            } else {
                self.stats.errors.fetch_add(1, Ordering::Relaxed);
            }
            result
        } else {
            Err(KError::NotSupported)
        }
    }

    /// Send ACL data
    pub fn send_acl(&mut self, data: &[u8]) -> KResult<()> {
        if let Some(device) = self.active_device_mut() {
            let result = device.send_acl(data);
            if result.is_ok() {
                self.stats.acl_tx_packets.fetch_add(1, Ordering::Relaxed);
                self.stats.acl_tx_bytes.fetch_add(data.len() as u64, Ordering::Relaxed);
            } else {
                self.stats.errors.fetch_add(1, Ordering::Relaxed);
            }
            result
        } else {
            Err(KError::NotSupported)
        }
    }

    /// Receive ACL data
    pub fn recv_acl(&mut self) -> Option<Vec<u8>> {
        if let Some(device) = self.active_device_mut() {
            if let Some(data) = device.recv_acl() {
                self.stats.acl_rx_packets.fetch_add(1, Ordering::Relaxed);
                self.stats.acl_rx_bytes.fetch_add(data.len() as u64, Ordering::Relaxed);
                Some(data)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Get BD address
    pub fn bd_addr(&self) -> Option<BdAddr> {
        self.active_device().map(|d| d.bd_addr())
    }

    /// Is ready
    pub fn is_ready(&self) -> bool {
        self.active_device().map(|d| d.is_ready()).unwrap_or(false)
    }

    /// Device count
    pub fn device_count(&self) -> usize {
        self.devices.len()
    }

    /// Has devices
    pub fn has_devices(&self) -> bool {
        !self.devices.is_empty()
    }

    /// Get stats
    pub fn stats(&self) -> RealtekBtStatsSnapshot {
        self.stats.snapshot()
    }

    /// Is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
}

/// Global manager
static REALTEK_BT: IrqSafeMutex<RealtekBtManager> = IrqSafeMutex::new(RealtekBtManager::new());

/// Initialize
pub fn init() {
    REALTEK_BT.lock().init();
}

/// Register device
pub fn register_device(
    vendor_id: u16,
    product_id: u16,
    usb_device: u32,
    bulk_in: u8,
    bulk_out: u8,
    interrupt_ep: u8,
) -> Option<usize> {
    REALTEK_BT.lock().register_device(vendor_id, product_id, usb_device, bulk_in, bulk_out, interrupt_ep)
}

/// Unregister device
pub fn unregister_device(idx: usize) {
    REALTEK_BT.lock().unregister_device(idx);
}

/// Send HCI command
pub fn send_command(data: &[u8]) -> KResult<Vec<u8>> {
    REALTEK_BT.lock().send_command(data)
}

/// Send ACL
pub fn send_acl(data: &[u8]) -> KResult<()> {
    REALTEK_BT.lock().send_acl(data)
}

/// Receive ACL
pub fn recv_acl() -> Option<Vec<u8>> {
    REALTEK_BT.lock().recv_acl()
}

/// Get BD address
pub fn bd_addr() -> Option<BdAddr> {
    REALTEK_BT.lock().bd_addr()
}

/// Is ready
pub fn is_ready() -> bool {
    REALTEK_BT.lock().is_ready()
}

/// Device count
pub fn device_count() -> usize {
    REALTEK_BT.lock().device_count()
}

/// Has devices
pub fn has_devices() -> bool {
    REALTEK_BT.lock().has_devices()
}

/// Get stats
pub fn stats() -> RealtekBtStatsSnapshot {
    REALTEK_BT.lock().stats()
}

/// Is initialized
pub fn is_initialized() -> bool {
    REALTEK_BT.lock().is_initialized()
}

/// Driver name
pub fn driver_name() -> &'static str {
    "realtek-bluetooth"
}

/// Is device supported
pub fn is_supported(vendor_id: u16, product_id: u16) -> bool {
    device_ids::is_supported(vendor_id, product_id)
}
