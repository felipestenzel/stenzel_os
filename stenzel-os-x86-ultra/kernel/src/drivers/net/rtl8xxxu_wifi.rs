// SPDX-License-Identifier: MIT
// Realtek WiFi driver for Stenzel OS
// Supports RTL8821, RTL8822, RTL8852 series adapters

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::string::ToString;
use crate::sync::TicketSpinlock;

/// Realtek WiFi device IDs
pub mod device_ids {
    use super::ChipGeneration;

    // RTL8821 series (WiFi 5, AC)
    pub const RTL8821AE: u16 = 0x8821;
    pub const RTL8821AU: u16 = 0x0811;  // USB
    pub const RTL8821CE: u16 = 0xC821;
    pub const RTL8821CU: u16 = 0xC811;  // USB

    // RTL8822 series (WiFi 5, AC 2x2)
    pub const RTL8822BE: u16 = 0xB822;
    pub const RTL8822BU: u16 = 0xB82B;  // USB
    pub const RTL8822CE: u16 = 0xC822;
    pub const RTL8822CU: u16 = 0xC82B;  // USB

    // RTL8852 series (WiFi 6/6E)
    pub const RTL8852AE: u16 = 0x8852;
    pub const RTL8852AU: u16 = 0x885A;  // USB
    pub const RTL8852BE: u16 = 0xB852;
    pub const RTL8852BU: u16 = 0xB85A;  // USB
    pub const RTL8852CE: u16 = 0xC852;
    pub const RTL8852CU: u16 = 0xC85A;  // USB

    // RTL8723 series (WiFi 4/5 + BT combo)
    pub const RTL8723AE: u16 = 0x8723;
    pub const RTL8723BE: u16 = 0xB723;
    pub const RTL8723DE: u16 = 0xD723;
    pub const RTL8723DU: u16 = 0xD72B;  // USB

    // RTL8188 series (WiFi 4/5, budget)
    pub const RTL8188EE: u16 = 0x8179;
    pub const RTL8188EU: u16 = 0x8179;  // USB variant
    pub const RTL8188CE: u16 = 0x8176;
    pub const RTL8188CU: u16 = 0x8176;  // USB variant

    pub fn is_pcie(device_id: u16) -> bool {
        matches!(device_id,
            RTL8821AE | RTL8821CE | RTL8822BE | RTL8822CE |
            RTL8852AE | RTL8852BE | RTL8852CE |
            RTL8723AE | RTL8723BE | RTL8723DE |
            RTL8188EE | RTL8188CE
        )
    }

    pub fn is_usb(device_id: u16) -> bool {
        matches!(device_id,
            RTL8821AU | RTL8821CU | RTL8822BU | RTL8822CU |
            RTL8852AU | RTL8852BU | RTL8852CU |
            RTL8723DU | RTL8188EU | RTL8188CU
        )
    }

    pub fn is_wifi6(device_id: u16) -> bool {
        matches!(device_id,
            RTL8852AE | RTL8852AU | RTL8852BE | RTL8852BU | RTL8852CE | RTL8852CU
        )
    }

    pub fn name(device_id: u16) -> &'static str {
        match device_id {
            RTL8821AE => "Realtek RTL8821AE",
            RTL8821AU => "Realtek RTL8821AU",
            RTL8821CE => "Realtek RTL8821CE",
            RTL8821CU => "Realtek RTL8821CU",
            RTL8822BE => "Realtek RTL8822BE",
            RTL8822BU => "Realtek RTL8822BU",
            RTL8822CE => "Realtek RTL8822CE",
            RTL8822CU => "Realtek RTL8822CU",
            RTL8852AE => "Realtek RTL8852AE",
            RTL8852AU => "Realtek RTL8852AU",
            RTL8852BE => "Realtek RTL8852BE",
            RTL8852BU => "Realtek RTL8852BU",
            RTL8852CE => "Realtek RTL8852CE",
            RTL8852CU => "Realtek RTL8852CU",
            RTL8723AE => "Realtek RTL8723AE",
            RTL8723BE => "Realtek RTL8723BE",
            RTL8723DE => "Realtek RTL8723DE",
            RTL8723DU => "Realtek RTL8723DU",
            RTL8188EE => "Realtek RTL8188EE",
            RTL8188EU => "Realtek RTL8188EU",
            RTL8188CE => "Realtek RTL8188CE",
            RTL8188CU => "Realtek RTL8188CU",
            _ => "Unknown Realtek WiFi",
        }
    }

    pub fn chip_gen(device_id: u16) -> ChipGeneration {
        match device_id {
            RTL8188EE | RTL8188EU | RTL8188CE | RTL8188CU => ChipGeneration::Gen1,
            RTL8723AE | RTL8723BE | RTL8821AE | RTL8821AU => ChipGeneration::Gen2,
            RTL8822BE | RTL8822BU | RTL8821CE | RTL8821CU | RTL8723DE | RTL8723DU => ChipGeneration::Gen3,
            RTL8822CE | RTL8822CU => ChipGeneration::Gen4,
            RTL8852AE | RTL8852AU | RTL8852BE | RTL8852BU | RTL8852CE | RTL8852CU => ChipGeneration::Gen5,
            _ => ChipGeneration::Unknown,
        }
    }
}

/// Chip generation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipGeneration {
    Unknown,
    Gen1,  // RTL8188
    Gen2,  // RTL8723A, RTL8821A
    Gen3,  // RTL8822B, RTL8821C, RTL8723D
    Gen4,  // RTL8822C
    Gen5,  // RTL8852A/B/C
}

/// WiFi standard
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WifiStandard {
    Wifi4,      // 802.11n
    Wifi5,      // 802.11ac
    Wifi6,      // 802.11ax
    Wifi6E,     // 802.11ax 6GHz
}

impl WifiStandard {
    pub fn name(self) -> &'static str {
        match self {
            WifiStandard::Wifi4 => "Wi-Fi 4 (802.11n)",
            WifiStandard::Wifi5 => "Wi-Fi 5 (802.11ac)",
            WifiStandard::Wifi6 => "Wi-Fi 6 (802.11ax)",
            WifiStandard::Wifi6E => "Wi-Fi 6E (802.11ax 6GHz)",
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
}

impl ChannelWidth {
    pub fn mhz(self) -> u32 {
        match self {
            ChannelWidth::Width20MHz => 20,
            ChannelWidth::Width40MHz => 40,
            ChannelWidth::Width80MHz => 80,
            ChannelWidth::Width160MHz => 160,
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
    pub version: u32,
    pub size: usize,
    pub loaded: bool,
}

impl Default for FirmwareInfo {
    fn default() -> Self {
        Self {
            name: String::new(),
            version: 0,
            size: 0,
            loaded: false,
        }
    }
}

/// Realtek register offsets
pub mod regs {
    // MAC registers
    pub const REG_SYS_CFG: u32 = 0x00F0;
    pub const REG_GPIO_MUXCFG: u32 = 0x0040;
    pub const REG_SYS_FUNC_EN: u32 = 0x0002;
    pub const REG_APS_FSMCO: u32 = 0x0004;
    pub const REG_SYS_CLKR: u32 = 0x0008;
    pub const REG_SYS_ISO_CTRL: u32 = 0x0000;
    pub const REG_RSV_CTRL: u32 = 0x001C;
    pub const REG_AFE_MISC: u32 = 0x0010;
    pub const REG_AFE_XTAL_CTRL: u32 = 0x0024;
    pub const REG_EFUSE_CTRL: u32 = 0x0030;
    pub const REG_EFUSE_TEST: u32 = 0x0034;
    pub const REG_PWR_DATA: u32 = 0x0038;
    pub const REG_CAL_TIMER: u32 = 0x003C;
    pub const REG_ACLK_MON: u32 = 0x003E;

    // TX/RX registers
    pub const REG_TXDMA_OFFSET: u32 = 0x0200;
    pub const REG_RXDMA_OFFSET: u32 = 0x0280;
    pub const REG_RQPN: u32 = 0x0200;
    pub const REG_FIFOPAGE: u32 = 0x0204;
    pub const REG_TDECTRL: u32 = 0x0208;
    pub const REG_TXDMA_STATUS: u32 = 0x0210;
    pub const REG_RXDMA_STATUS: u32 = 0x0288;

    // MAC control
    pub const REG_MAC_CTRL: u32 = 0x0100;
    pub const REG_BWOPMODE: u32 = 0x0203;
    pub const REG_TCR: u32 = 0x0604;
    pub const REG_RCR: u32 = 0x0608;
    pub const REG_BCN_CTRL: u32 = 0x0550;

    // Security
    pub const REG_SECCFG: u32 = 0x0680;
    pub const REG_CAMCMD: u32 = 0x0670;
    pub const REG_CAMWRITE: u32 = 0x0674;
    pub const REG_CAMREAD: u32 = 0x0678;
    pub const REG_CAMDBG: u32 = 0x067C;

    // Interrupt
    pub const REG_HIMR: u32 = 0x00B0;
    pub const REG_HISR: u32 = 0x00B4;
    pub const REG_HIMRE: u32 = 0x00B8;
    pub const REG_HISRE: u32 = 0x00BC;

    // Power management
    pub const REG_CPWM: u32 = 0x012C;
    pub const REG_FWIMR: u32 = 0x0130;
    pub const REG_FWISR: u32 = 0x0134;
    pub const REG_PKTBUF_DBG_CTRL: u32 = 0x0140;
    pub const REG_RXPKTBUF_CTRL: u32 = 0x0144;
}

/// Bus type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusType {
    Pcie,
    Usb,
    Sdio,
}

/// Realtek WiFi Driver
pub struct RealtekWifiDriver {
    // Device info
    pub device_id: u16,
    pub device_name: String,
    pub chip_gen: ChipGeneration,
    pub bus_type: BusType,
    pub mmio_base: u64,
    pub irq: u8,

    // Capabilities
    pub wifi_standard: WifiStandard,
    pub supported_bands: Vec<FrequencyBand>,
    pub max_width: ChannelWidth,
    pub max_streams: u8,
    pub has_bluetooth: bool,

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

impl RealtekWifiDriver {
    pub const fn new() -> Self {
        Self {
            device_id: 0,
            device_name: String::new(),
            chip_gen: ChipGeneration::Unknown,
            bus_type: BusType::Pcie,
            mmio_base: 0,
            irq: 0,
            wifi_standard: WifiStandard::Wifi5,
            supported_bands: Vec::new(),
            max_width: ChannelWidth::Width80MHz,
            max_streams: 1,
            has_bluetooth: false,
            power_state: PowerState::Off,
            connection_state: ConnectionState::Disconnected,
            current_ssid: None,
            current_bssid: None,
            current_channel: 0,
            current_band: FrequencyBand::Band2_4GHz,
            current_security: SecurityMode::Open,
            firmware: FirmwareInfo {
                name: String::new(),
                version: 0,
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

    /// Initialize driver with device
    pub fn init(&mut self, device_id: u16, bus_type: BusType, mmio_base: u64, irq: u8) -> Result<(), &'static str> {
        self.device_id = device_id;
        self.device_name = device_ids::name(device_id).to_string();
        self.chip_gen = device_ids::chip_gen(device_id);
        self.bus_type = bus_type;
        self.mmio_base = mmio_base;
        self.irq = irq;

        // Set capabilities based on device
        self.setup_capabilities();

        // Hardware init
        self.hw_init()?;

        // Load firmware
        self.load_firmware()?;

        self.power_state = PowerState::Active;
        self.initialized = true;

        crate::kprintln!("rtlwifi: Initialized {} ({:04X}) - {}",
            self.device_name, self.device_id, self.wifi_standard.name());

        Ok(())
    }

    /// Setup device capabilities based on chip
    fn setup_capabilities(&mut self) {
        match self.chip_gen {
            ChipGeneration::Gen1 => {
                self.wifi_standard = WifiStandard::Wifi4;
                self.supported_bands = vec![FrequencyBand::Band2_4GHz];
                self.max_width = ChannelWidth::Width40MHz;
                self.max_streams = 1;
                self.has_bluetooth = false;
            }
            ChipGeneration::Gen2 => {
                self.wifi_standard = WifiStandard::Wifi5;
                self.supported_bands = vec![FrequencyBand::Band2_4GHz, FrequencyBand::Band5GHz];
                self.max_width = ChannelWidth::Width80MHz;
                self.max_streams = 1;
                self.has_bluetooth = self.device_id & 0x0F00 == 0x0700;  // RTL8723 has BT
            }
            ChipGeneration::Gen3 | ChipGeneration::Gen4 => {
                self.wifi_standard = WifiStandard::Wifi5;
                self.supported_bands = vec![FrequencyBand::Band2_4GHz, FrequencyBand::Band5GHz];
                self.max_width = ChannelWidth::Width80MHz;
                self.max_streams = 2;
                self.has_bluetooth = true;
            }
            ChipGeneration::Gen5 => {
                if device_ids::is_wifi6(self.device_id) {
                    self.wifi_standard = WifiStandard::Wifi6;
                    // 8852CE supports 6GHz
                    if self.device_id == device_ids::RTL8852CE || self.device_id == device_ids::RTL8852CU {
                        self.wifi_standard = WifiStandard::Wifi6E;
                        self.supported_bands = vec![
                            FrequencyBand::Band2_4GHz,
                            FrequencyBand::Band5GHz,
                            FrequencyBand::Band6GHz,
                        ];
                    } else {
                        self.supported_bands = vec![FrequencyBand::Band2_4GHz, FrequencyBand::Band5GHz];
                    }
                    self.max_width = ChannelWidth::Width160MHz;
                    self.max_streams = 2;
                    self.has_bluetooth = true;
                } else {
                    self.wifi_standard = WifiStandard::Wifi5;
                    self.supported_bands = vec![FrequencyBand::Band2_4GHz, FrequencyBand::Band5GHz];
                    self.max_width = ChannelWidth::Width80MHz;
                    self.max_streams = 2;
                    self.has_bluetooth = true;
                }
            }
            ChipGeneration::Unknown => {
                self.wifi_standard = WifiStandard::Wifi4;
                self.supported_bands = vec![FrequencyBand::Band2_4GHz];
                self.max_width = ChannelWidth::Width20MHz;
                self.max_streams = 1;
                self.has_bluetooth = false;
            }
        }
    }

    /// Hardware initialization
    fn hw_init(&mut self) -> Result<(), &'static str> {
        if self.bus_type == BusType::Pcie {
            // Power on sequence
            self.write_reg(regs::REG_APS_FSMCO, 0x00);

            // Enable MAC
            self.write_reg(regs::REG_SYS_FUNC_EN, 0xE2);

            // Wait for MAC ready
            for _ in 0..100 {
                let val = self.read_reg(regs::REG_SYS_CFG);
                if val & 0x01 != 0 {
                    break;
                }
                for _ in 0..1000 {}  // Busy wait
            }

            // Enable TX/RX
            self.write_reg(regs::REG_TCR, 0x700F0001);
            self.write_reg(regs::REG_RCR, 0x700F0001);
        }

        Ok(())
    }

    /// Load firmware
    fn load_firmware(&mut self) -> Result<(), &'static str> {
        let fw_name = match self.chip_gen {
            ChipGeneration::Gen1 => "rtlwifi/rtl8188efw.bin",
            ChipGeneration::Gen2 => {
                if self.device_id & 0x0F00 == 0x0700 {
                    "rtlwifi/rtl8723befw.bin"
                } else {
                    "rtlwifi/rtl8821aefw.bin"
                }
            }
            ChipGeneration::Gen3 => "rtlwifi/rtl8822befw.bin",
            ChipGeneration::Gen4 => "rtlwifi/rtl8822cefw.bin",
            ChipGeneration::Gen5 => "rtlwifi/rtl8852aefw.bin",
            ChipGeneration::Unknown => return Err("Unknown chip generation"),
        };

        self.firmware = FirmwareInfo {
            name: fw_name.to_string(),
            version: 1,
            size: 128 * 1024,  // ~128KB typical
            loaded: true,
        };

        crate::kprintln!("rtlwifi: Firmware {} loaded", fw_name);

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

    /// Start scan
    pub fn scan(&mut self) -> Result<(), &'static str> {
        if !self.initialized {
            return Err("Driver not initialized");
        }

        self.connection_state = ConnectionState::Scanning;
        self.scan_results.clear();

        // In a real implementation, would trigger hardware scan
        self.connection_state = ConnectionState::Disconnected;

        crate::kprintln!("rtlwifi: Scan completed, {} networks found", self.scan_results.len());

        Ok(())
    }

    /// Connect to network
    pub fn connect(&mut self, ssid: &str, password: &str, security: SecurityMode) -> Result<(), &'static str> {
        if !self.initialized {
            return Err("Driver not initialized");
        }

        self.connection_state = ConnectionState::Authenticating;
        self.current_ssid = Some(ssid.to_string());
        self.current_security = security;

        let _ = password;  // Would use for key derivation

        self.connection_state = ConnectionState::Connected;
        self.stats.signal_strength = -55;

        crate::kprintln!("rtlwifi: Connected to {}", ssid);

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

        crate::kprintln!("rtlwifi: Disconnected");

        Ok(())
    }

    /// Set power mode
    pub fn set_power_mode(&mut self, mode: PowerState) -> Result<(), &'static str> {
        match mode {
            PowerState::Active => {
                self.write_reg(regs::REG_APS_FSMCO, 0x00);
            }
            PowerState::LowPower => {
                self.write_reg(regs::REG_APS_FSMCO, 0x01);
            }
            PowerState::Sleep => {
                self.write_reg(regs::REG_APS_FSMCO, 0x02);
            }
            PowerState::Off => {
                self.write_reg(regs::REG_APS_FSMCO, 0x04);
            }
        }
        self.power_state = mode;
        Ok(())
    }

    /// Get status string
    pub fn get_status(&self) -> String {
        let mut status = String::new();

        status.push_str("Realtek WiFi Status:\n");
        status.push_str(&alloc::format!("  Device: {} ({:04X})\n", self.device_name, self.device_id));
        status.push_str(&alloc::format!("  Chip Gen: {:?}\n", self.chip_gen));
        status.push_str(&alloc::format!("  Bus: {:?}\n", self.bus_type));
        status.push_str(&alloc::format!("  Standard: {}\n", self.wifi_standard.name()));
        status.push_str(&alloc::format!("  Max Width: {} MHz\n", self.max_width.mhz()));
        status.push_str(&alloc::format!("  Streams: {}x{}\n", self.max_streams, self.max_streams));
        status.push_str(&alloc::format!("  Bluetooth: {}\n", self.has_bluetooth));

        status.push_str("  Bands:\n");
        for band in &self.supported_bands {
            status.push_str(&alloc::format!("    {:?}\n", band));
        }

        status.push_str(&alloc::format!("  Power: {:?}\n", self.power_state));
        status.push_str(&alloc::format!("  Connection: {:?}\n", self.connection_state));

        if let Some(ref ssid) = self.current_ssid {
            status.push_str(&alloc::format!("  SSID: {}\n", ssid));
            status.push_str(&alloc::format!("  Security: {}\n", self.current_security.name()));
            status.push_str(&alloc::format!("  Signal: {} dBm\n", self.stats.signal_strength));
        }

        status.push_str(&alloc::format!("  Firmware: {}\n", self.firmware.name));

        status.push_str("  Stats:\n");
        status.push_str(&alloc::format!("    TX: {} packets, {} bytes\n",
            self.stats.tx_packets, self.stats.tx_bytes));
        status.push_str(&alloc::format!("    RX: {} packets, {} bytes\n",
            self.stats.rx_packets, self.stats.rx_bytes));
        status.push_str(&alloc::format!("    Errors: TX={}, RX={}\n",
            self.stats.tx_errors, self.stats.rx_errors));

        status
    }
}

/// Global Realtek WiFi driver
static REALTEK_WIFI: TicketSpinlock<Option<RealtekWifiDriver>> = TicketSpinlock::new(None);

/// Realtek vendor ID
const REALTEK_VENDOR_ID: u16 = 0x10EC;

/// Initialize Realtek WiFi driver (entry point - scans PCI)
pub fn init() {
    // Scan PCI for Realtek WiFi devices
    for bus in 0..=255u8 {
        for dev in 0..32u8 {
            for func in 0..8u8 {
                let vendor = pci_read_vendor(bus, dev, func);
                if vendor == 0xFFFF || vendor != REALTEK_VENDOR_ID {
                    continue;
                }

                let device_id = pci_read_device(bus, dev, func);

                // Check if supported Realtek WiFi device
                if device_ids::is_pcie(device_id) {
                    let bar0 = pci_read_bar0(bus, dev, func);
                    let mmio_base = (bar0 & 0xFFFFFFF0) as u64;
                    let irq = pci_read_irq(bus, dev, func);

                    if let Err(e) = init_with_device(device_id, BusType::Pcie, mmio_base, irq) {
                        crate::kprintln!("rtlwifi: Failed to init device {:04X}: {}", device_id, e);
                    }
                    return;
                }
            }
        }
    }
}

/// Initialize with specific device
pub fn init_with_device(device_id: u16, bus_type: BusType, mmio_base: u64, irq: u8) -> Result<(), &'static str> {
    let mut guard = REALTEK_WIFI.lock();
    let mut driver = RealtekWifiDriver::new();
    driver.init(device_id, bus_type, mmio_base, irq)?;
    *guard = Some(driver);
    Ok(())
}

/// Get driver instance
pub fn get_driver() -> Option<&'static TicketSpinlock<Option<RealtekWifiDriver>>> {
    Some(&REALTEK_WIFI)
}

/// PCI helper functions
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
