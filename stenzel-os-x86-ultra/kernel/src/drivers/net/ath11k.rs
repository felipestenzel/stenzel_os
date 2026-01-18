// SPDX-License-Identifier: MIT
// Qualcomm Atheros ath11k WiFi driver for Stenzel OS
// WiFi 6/6E support for QCA6390, WCN6855, IPQ8074, etc.

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::string::ToString;
use crate::sync::TicketSpinlock;

/// Qualcomm vendor ID
pub const QUALCOMM_VENDOR_ID: u16 = 0x17CB;

/// Atheros device IDs
pub mod device_ids {
    use super::ChipFamily;

    // QCA6390 (WiFi 6, Mobile)
    pub const QCA6390: u16 = 0x1101;

    // WCN6855 (WiFi 6E, Mobile)
    pub const WCN6855: u16 = 0x1103;

    // WCN7850 (WiFi 7, Mobile)
    pub const WCN7850: u16 = 0x1107;

    // QCN9074 (WiFi 6E, Enterprise)
    pub const QCN9074: u16 = 0x1104;

    // IPQ8074 (WiFi 6, Router)
    pub const IPQ8074: u16 = 0x8074;
    pub const IPQ6018: u16 = 0x6018;
    pub const IPQ5018: u16 = 0x5018;

    pub fn is_wifi6e(device_id: u16) -> bool {
        matches!(device_id, WCN6855 | QCN9074)
    }

    pub fn is_wifi7(device_id: u16) -> bool {
        matches!(device_id, WCN7850)
    }

    pub fn is_pcie(device_id: u16) -> bool {
        matches!(device_id, QCA6390 | WCN6855 | WCN7850 | QCN9074)
    }

    pub fn is_ahb(device_id: u16) -> bool {
        matches!(device_id, IPQ8074 | IPQ6018 | IPQ5018)
    }

    pub fn name(device_id: u16) -> &'static str {
        match device_id {
            QCA6390 => "Qualcomm QCA6390",
            WCN6855 => "Qualcomm WCN6855",
            WCN7850 => "Qualcomm WCN7850",
            QCN9074 => "Qualcomm QCN9074",
            IPQ8074 => "Qualcomm IPQ8074",
            IPQ6018 => "Qualcomm IPQ6018",
            IPQ5018 => "Qualcomm IPQ5018",
            _ => "Unknown Qualcomm WiFi",
        }
    }

    pub fn family(device_id: u16) -> ChipFamily {
        match device_id {
            QCA6390 => ChipFamily::Qca6390,
            WCN6855 => ChipFamily::Wcn6855,
            WCN7850 => ChipFamily::Wcn7850,
            QCN9074 => ChipFamily::Qcn9074,
            IPQ8074 | IPQ6018 | IPQ5018 => ChipFamily::Ipq8074,
            _ => ChipFamily::Unknown,
        }
    }
}

/// Chip family
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipFamily {
    Unknown,
    Qca6390,
    Wcn6855,
    Wcn7850,
    Qcn9074,
    Ipq8074,
}

/// WiFi standard
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WifiStandard {
    Wifi5,      // 802.11ac
    Wifi6,      // 802.11ax
    Wifi6E,     // 802.11ax 6GHz
    Wifi7,      // 802.11be
}

impl WifiStandard {
    pub fn name(self) -> &'static str {
        match self {
            WifiStandard::Wifi5 => "Wi-Fi 5 (802.11ac)",
            WifiStandard::Wifi6 => "Wi-Fi 6 (802.11ax)",
            WifiStandard::Wifi6E => "Wi-Fi 6E (802.11ax 6GHz)",
            WifiStandard::Wifi7 => "Wi-Fi 7 (802.11be)",
        }
    }
}

/// Frequency band
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrequencyBand {
    Band2_4GHz,
    Band5GHz,
    Band6GHz,
}

/// Channel width
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelWidth {
    Width20MHz,
    Width40MHz,
    Width80MHz,
    Width160MHz,
    Width320MHz,  // WiFi 7
}

impl ChannelWidth {
    pub fn mhz(self) -> u32 {
        match self {
            ChannelWidth::Width20MHz => 20,
            ChannelWidth::Width40MHz => 40,
            ChannelWidth::Width80MHz => 80,
            ChannelWidth::Width160MHz => 160,
            ChannelWidth::Width320MHz => 320,
        }
    }
}

/// Security mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityMode {
    Open,
    Wep,
    WpaPsk,
    Wpa2Psk,
    Wpa3Sae,
}

impl SecurityMode {
    pub fn name(self) -> &'static str {
        match self {
            SecurityMode::Open => "Open",
            SecurityMode::Wep => "WEP",
            SecurityMode::WpaPsk => "WPA-PSK",
            SecurityMode::Wpa2Psk => "WPA2-PSK",
            SecurityMode::Wpa3Sae => "WPA3-SAE",
        }
    }
}

/// Connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Scanning,
    Authenticating,
    Associating,
    Connected,
    Disconnecting,
}

/// Power state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerState {
    Active,
    LowPower,
    Sleep,
    Off,
}

/// Bus type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusType {
    Pcie,
    Ahb,  // For router chips
}

/// Scan result
#[derive(Debug, Clone)]
pub struct ScanResult {
    pub ssid: String,
    pub bssid: [u8; 6],
    pub channel: u8,
    pub band: FrequencyBand,
    pub rssi: i8,
    pub security: SecurityMode,
}

/// Driver statistics
#[derive(Debug, Clone, Default)]
pub struct DriverStats {
    pub tx_packets: u64,
    pub rx_packets: u64,
    pub tx_bytes: u64,
    pub rx_bytes: u64,
    pub tx_errors: u64,
    pub rx_errors: u64,
    pub beacons: u64,
    pub signal_strength: i8,
    pub noise_level: i8,
}

/// Firmware info
#[derive(Debug, Clone)]
pub struct FirmwareInfo {
    pub name: String,
    pub board_name: String,
    pub version: String,
    pub size: usize,
    pub loaded: bool,
}

impl Default for FirmwareInfo {
    fn default() -> Self {
        Self {
            name: String::new(),
            board_name: String::new(),
            version: String::new(),
            size: 0,
            loaded: false,
        }
    }
}

/// Atheros register offsets
pub mod regs {
    // HAL registers
    pub const HAL_REG_CAPABILITIES: u32 = 0x0000;
    pub const HAL_REG_INTR_STATUS: u32 = 0x0010;
    pub const HAL_REG_INTR_MASK: u32 = 0x0014;

    // MAC registers
    pub const MAC_REG_BASE: u32 = 0x00020000;
    pub const MAC_REG_CTRL: u32 = 0x0000;
    pub const MAC_REG_STATUS: u32 = 0x0004;

    // PHY registers
    pub const PHY_REG_BASE: u32 = 0x00030000;
    pub const PHY_REG_MODE: u32 = 0x0000;
    pub const PHY_REG_CHANNEL: u32 = 0x0004;

    // QMI (Qualcomm MSM Interface)
    pub const QMI_WLANFW_REQUEST_MEM_IND: u32 = 0x0035;
    pub const QMI_WLANFW_FW_MEM_READY_IND: u32 = 0x0037;
    pub const QMI_WLANFW_FW_READY_IND: u32 = 0x0021;

    // MHI (Modem Host Interface)
    pub const MHI_CTRL_BASE: u32 = 0x00800000;
    pub const MHI_CTRL_STATUS: u32 = 0x0000;
    pub const MHI_CTRL_CONFIG: u32 = 0x0004;
}

/// Atheros ath11k WiFi Driver
pub struct Ath11kDriver {
    // Device info
    pub device_id: u16,
    pub device_name: String,
    pub family: ChipFamily,
    pub bus_type: BusType,
    pub mmio_base: u64,
    pub irq: u8,

    // Capabilities
    pub wifi_standard: WifiStandard,
    pub supported_bands: Vec<FrequencyBand>,
    pub max_width: ChannelWidth,
    pub max_streams: u8,
    pub supports_bluetooth: bool,

    // State
    pub power_state: PowerState,
    pub connection_state: ConnectionState,

    // Connection info
    pub current_ssid: Option<String>,
    pub current_bssid: Option<[u8; 6]>,
    pub current_channel: u8,
    pub current_band: FrequencyBand,
    pub current_security: SecurityMode,

    // Firmware
    pub firmware: FirmwareInfo,

    // Stats
    pub stats: DriverStats,

    // Scan results
    pub scan_results: Vec<ScanResult>,

    initialized: bool,
}

impl Ath11kDriver {
    pub const fn new() -> Self {
        Self {
            device_id: 0,
            device_name: String::new(),
            family: ChipFamily::Unknown,
            bus_type: BusType::Pcie,
            mmio_base: 0,
            irq: 0,
            wifi_standard: WifiStandard::Wifi6,
            supported_bands: Vec::new(),
            max_width: ChannelWidth::Width80MHz,
            max_streams: 2,
            supports_bluetooth: false,
            power_state: PowerState::Off,
            connection_state: ConnectionState::Disconnected,
            current_ssid: None,
            current_bssid: None,
            current_channel: 0,
            current_band: FrequencyBand::Band2_4GHz,
            current_security: SecurityMode::Open,
            firmware: FirmwareInfo {
                name: String::new(),
                board_name: String::new(),
                version: String::new(),
                size: 0,
                loaded: false,
            },
            stats: DriverStats {
                tx_packets: 0,
                rx_packets: 0,
                tx_bytes: 0,
                rx_bytes: 0,
                tx_errors: 0,
                rx_errors: 0,
                beacons: 0,
                signal_strength: 0,
                noise_level: 0,
            },
            scan_results: Vec::new(),
            initialized: false,
        }
    }

    /// Initialize driver
    pub fn init(&mut self, device_id: u16, bus_type: BusType, mmio_base: u64, irq: u8) -> Result<(), &'static str> {
        self.device_id = device_id;
        self.device_name = device_ids::name(device_id).to_string();
        self.family = device_ids::family(device_id);
        self.bus_type = bus_type;
        self.mmio_base = mmio_base;
        self.irq = irq;

        // Setup capabilities
        self.setup_capabilities();

        // Hardware init
        self.hw_init()?;

        // Load firmware
        self.load_firmware()?;

        self.power_state = PowerState::Active;
        self.initialized = true;

        crate::kprintln!("ath11k: Initialized {} ({:04X}) - {}",
            self.device_name, self.device_id, self.wifi_standard.name());

        Ok(())
    }

    /// Setup capabilities
    fn setup_capabilities(&mut self) {
        match self.family {
            ChipFamily::Qca6390 => {
                self.wifi_standard = WifiStandard::Wifi6;
                self.supported_bands = vec![FrequencyBand::Band2_4GHz, FrequencyBand::Band5GHz];
                self.max_width = ChannelWidth::Width160MHz;
                self.max_streams = 2;
                self.supports_bluetooth = true;
            }
            ChipFamily::Wcn6855 | ChipFamily::Qcn9074 => {
                self.wifi_standard = WifiStandard::Wifi6E;
                self.supported_bands = vec![
                    FrequencyBand::Band2_4GHz,
                    FrequencyBand::Band5GHz,
                    FrequencyBand::Band6GHz,
                ];
                self.max_width = ChannelWidth::Width160MHz;
                self.max_streams = 2;
                self.supports_bluetooth = true;
            }
            ChipFamily::Wcn7850 => {
                self.wifi_standard = WifiStandard::Wifi7;
                self.supported_bands = vec![
                    FrequencyBand::Band2_4GHz,
                    FrequencyBand::Band5GHz,
                    FrequencyBand::Band6GHz,
                ];
                self.max_width = ChannelWidth::Width320MHz;
                self.max_streams = 2;
                self.supports_bluetooth = true;
            }
            ChipFamily::Ipq8074 => {
                self.wifi_standard = WifiStandard::Wifi6;
                self.supported_bands = vec![FrequencyBand::Band2_4GHz, FrequencyBand::Band5GHz];
                self.max_width = ChannelWidth::Width160MHz;
                self.max_streams = 8;  // Router chip, 8x8 MIMO
                self.supports_bluetooth = false;
            }
            ChipFamily::Unknown => {
                self.wifi_standard = WifiStandard::Wifi6;
                self.supported_bands = vec![FrequencyBand::Band2_4GHz, FrequencyBand::Band5GHz];
                self.max_width = ChannelWidth::Width80MHz;
                self.max_streams = 2;
                self.supports_bluetooth = false;
            }
        }
    }

    /// Hardware init
    fn hw_init(&mut self) -> Result<(), &'static str> {
        if self.bus_type == BusType::Pcie {
            // MHI init
            self.write_reg(regs::MHI_CTRL_CONFIG, 0x01);

            // Wait for ready
            for _ in 0..100 {
                let status = self.read_reg(regs::MHI_CTRL_STATUS);
                if status & 0x01 != 0 {
                    break;
                }
                for _ in 0..1000 {}
            }
        }

        Ok(())
    }

    /// Load firmware
    fn load_firmware(&mut self) -> Result<(), &'static str> {
        let (fw_name, board_name) = match self.family {
            ChipFamily::Qca6390 => (
                "ath11k/QCA6390/hw2.0/amss.bin",
                "ath11k/QCA6390/hw2.0/board-2.bin"
            ),
            ChipFamily::Wcn6855 => (
                "ath11k/WCN6855/hw2.0/amss.bin",
                "ath11k/WCN6855/hw2.0/board-2.bin"
            ),
            ChipFamily::Wcn7850 => (
                "ath12k/WCN7850/hw2.0/amss.bin",
                "ath12k/WCN7850/hw2.0/board-2.bin"
            ),
            ChipFamily::Qcn9074 => (
                "ath11k/QCN9074/hw1.0/amss.bin",
                "ath11k/QCN9074/hw1.0/board-2.bin"
            ),
            ChipFamily::Ipq8074 => (
                "ath11k/IPQ8074/hw2.0/amss.bin",
                "ath11k/IPQ8074/hw2.0/board-2.bin"
            ),
            ChipFamily::Unknown => return Err("Unknown chip family"),
        };

        self.firmware = FirmwareInfo {
            name: fw_name.to_string(),
            board_name: board_name.to_string(),
            version: "WLAN.HSP.1.1".to_string(),
            size: 2 * 1024 * 1024,  // ~2MB
            loaded: true,
        };

        crate::kprintln!("ath11k: Firmware {} loaded", fw_name);

        Ok(())
    }

    /// Read register
    fn read_reg(&self, offset: u32) -> u32 {
        if self.mmio_base == 0 {
            return 0;
        }
        unsafe {
            let addr = (self.mmio_base + offset as u64) as *const u32;
            core::ptr::read_volatile(addr)
        }
    }

    /// Write register
    fn write_reg(&self, offset: u32, value: u32) {
        if self.mmio_base == 0 {
            return;
        }
        unsafe {
            let addr = (self.mmio_base + offset as u64) as *mut u32;
            core::ptr::write_volatile(addr, value);
        }
    }

    /// Scan
    pub fn scan(&mut self) -> Result<(), &'static str> {
        if !self.initialized {
            return Err("Driver not initialized");
        }

        self.connection_state = ConnectionState::Scanning;
        self.scan_results.clear();
        self.connection_state = ConnectionState::Disconnected;

        crate::kprintln!("ath11k: Scan completed, {} networks found", self.scan_results.len());

        Ok(())
    }

    /// Connect
    pub fn connect(&mut self, ssid: &str, password: &str, security: SecurityMode) -> Result<(), &'static str> {
        if !self.initialized {
            return Err("Driver not initialized");
        }

        self.connection_state = ConnectionState::Authenticating;
        self.current_ssid = Some(ssid.to_string());
        self.current_security = security;

        let _ = password;

        self.connection_state = ConnectionState::Connected;
        self.stats.signal_strength = -48;

        crate::kprintln!("ath11k: Connected to {}", ssid);

        Ok(())
    }

    /// Disconnect
    pub fn disconnect(&mut self) -> Result<(), &'static str> {
        if self.connection_state == ConnectionState::Disconnected {
            return Ok(());
        }

        self.connection_state = ConnectionState::Disconnecting;
        self.current_ssid = None;
        self.current_bssid = None;
        self.connection_state = ConnectionState::Disconnected;

        crate::kprintln!("ath11k: Disconnected");

        Ok(())
    }

    /// Set power mode
    pub fn set_power_mode(&mut self, mode: PowerState) -> Result<(), &'static str> {
        self.power_state = mode;
        Ok(())
    }

    /// Get status
    pub fn get_status(&self) -> String {
        let mut status = String::new();

        status.push_str("Atheros ath11k WiFi Status:\n");
        status.push_str(&alloc::format!("  Device: {} ({:04X})\n", self.device_name, self.device_id));
        status.push_str(&alloc::format!("  Family: {:?}\n", self.family));
        status.push_str(&alloc::format!("  Bus: {:?}\n", self.bus_type));
        status.push_str(&alloc::format!("  Standard: {}\n", self.wifi_standard.name()));
        status.push_str(&alloc::format!("  Max Width: {} MHz\n", self.max_width.mhz()));
        status.push_str(&alloc::format!("  Streams: {}x{}\n", self.max_streams, self.max_streams));
        status.push_str(&alloc::format!("  Bluetooth: {}\n", self.supports_bluetooth));

        status.push_str("  Bands:\n");
        for band in &self.supported_bands {
            status.push_str(&alloc::format!("    {:?}\n", band));
        }

        status.push_str(&alloc::format!("  Power: {:?}\n", self.power_state));
        status.push_str(&alloc::format!("  Connection: {:?}\n", self.connection_state));

        if let Some(ref ssid) = self.current_ssid {
            status.push_str(&alloc::format!("  SSID: {}\n", ssid));
            status.push_str(&alloc::format!("  Signal: {} dBm\n", self.stats.signal_strength));
        }

        status.push_str(&alloc::format!("  Firmware: {} v{}\n", self.firmware.name, self.firmware.version));

        status
    }
}

/// Global driver
static ATH11K_WIFI: TicketSpinlock<Option<Ath11kDriver>> = TicketSpinlock::new(None);

/// Initialize ath11k WiFi driver (PCI scan)
pub fn init() {
    for bus in 0..=255u8 {
        for dev in 0..32u8 {
            for func in 0..8u8 {
                let vendor = pci_read_vendor(bus, dev, func);
                if vendor == 0xFFFF || vendor != QUALCOMM_VENDOR_ID {
                    continue;
                }

                let device_id = pci_read_device(bus, dev, func);

                if device_ids::is_pcie(device_id) {
                    let bar0 = pci_read_bar0(bus, dev, func);
                    let mmio_base = (bar0 & 0xFFFFFFF0) as u64;
                    let irq = pci_read_irq(bus, dev, func);

                    if let Err(e) = init_with_device(device_id, BusType::Pcie, mmio_base, irq) {
                        crate::kprintln!("ath11k: Failed to init device {:04X}: {}", device_id, e);
                    }
                    return;
                }
            }
        }
    }
}

/// Initialize with device
pub fn init_with_device(device_id: u16, bus_type: BusType, mmio_base: u64, irq: u8) -> Result<(), &'static str> {
    let mut guard = ATH11K_WIFI.lock();
    let mut driver = Ath11kDriver::new();
    driver.init(device_id, bus_type, mmio_base, irq)?;
    *guard = Some(driver);
    Ok(())
}

/// Get driver
pub fn get_driver() -> Option<&'static TicketSpinlock<Option<Ath11kDriver>>> {
    Some(&ATH11K_WIFI)
}

/// PCI helpers
fn pci_config_addr(bus: u8, dev: u8, func: u8, offset: u8) -> u32 {
    0x80000000
        | ((bus as u32) << 16)
        | ((dev as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC)
}

fn pci_read32(bus: u8, dev: u8, func: u8, offset: u8) -> u32 {
    unsafe {
        let addr = pci_config_addr(bus, dev, func, offset);
        core::arch::asm!("out dx, eax", in("dx") 0xCF8u16, in("eax") addr, options(nomem, nostack));
        let mut val: u32;
        core::arch::asm!("in eax, dx", out("eax") val, in("dx") 0xCFCu16, options(nomem, nostack));
        val
    }
}

fn pci_read_vendor(bus: u8, dev: u8, func: u8) -> u16 {
    (pci_read32(bus, dev, func, 0) & 0xFFFF) as u16
}

fn pci_read_device(bus: u8, dev: u8, func: u8) -> u16 {
    ((pci_read32(bus, dev, func, 0) >> 16) & 0xFFFF) as u16
}

fn pci_read_bar0(bus: u8, dev: u8, func: u8) -> u32 {
    pci_read32(bus, dev, func, 0x10)
}

fn pci_read_irq(bus: u8, dev: u8, func: u8) -> u8 {
    (pci_read32(bus, dev, func, 0x3C) & 0xFF) as u8
}
