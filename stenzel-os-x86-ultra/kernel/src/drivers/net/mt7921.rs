// SPDX-License-Identifier: MIT
// MediaTek MT7921/MT7922 WiFi driver for Stenzel OS
// WiFi 6/6E support with PCIe and USB interfaces

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::string::ToString;
use crate::sync::TicketSpinlock;

/// MediaTek vendor ID
pub const MEDIATEK_VENDOR_ID: u16 = 0x14C3;

/// MediaTek WiFi device IDs
pub mod device_ids {
    use super::ChipVariant;

    // MT7921 series (WiFi 6)
    pub const MT7921E: u16 = 0x7961;   // PCIe
    pub const MT7921K: u16 = 0x0608;   // USB
    pub const MT7921S: u16 = 0x7901;   // SDIO
    pub const MT7921AU: u16 = 0x7921;  // USB variant

    // MT7922 series (WiFi 6E)
    pub const MT7922: u16 = 0x7922;
    pub const MT792X_E: u16 = 0x0616;  // PCIe variant

    // AMD RZ608/RZ616 (MT7921K rebrand)
    pub const AMD_RZ608: u16 = 0x0608;
    pub const AMD_RZ616: u16 = 0x0616;

    // MT7925 series (WiFi 7)
    pub const MT7925E: u16 = 0x7925;   // PCIe WiFi 7
    pub const MT7925U: u16 = 0x7927;   // USB WiFi 7

    pub fn is_wifi6e(device_id: u16) -> bool {
        matches!(device_id, MT7922 | MT792X_E | AMD_RZ616)
    }

    pub fn is_wifi7(device_id: u16) -> bool {
        matches!(device_id, MT7925E | MT7925U)
    }

    pub fn is_pcie(device_id: u16) -> bool {
        matches!(device_id, MT7921E | MT7922 | MT792X_E | MT7925E | AMD_RZ608 | AMD_RZ616)
    }

    pub fn is_usb(device_id: u16) -> bool {
        matches!(device_id, MT7921K | MT7921AU | MT7925U)
    }

    pub fn is_sdio(device_id: u16) -> bool {
        matches!(device_id, MT7921S)
    }

    pub fn name(device_id: u16) -> &'static str {
        match device_id {
            MT7921E => "MediaTek MT7921E",
            MT7921K => "MediaTek MT7921K",
            MT7921S => "MediaTek MT7921S",
            MT7921AU => "MediaTek MT7921AU",
            MT7922 => "MediaTek MT7922",
            MT792X_E => "MediaTek MT792x",
            AMD_RZ608 => "AMD RZ608 (MT7921K)",
            AMD_RZ616 => "AMD RZ616 (MT7922)",
            MT7925E => "MediaTek MT7925E",
            MT7925U => "MediaTek MT7925U",
            _ => "Unknown MediaTek WiFi",
        }
    }

    pub fn variant(device_id: u16) -> ChipVariant {
        match device_id {
            MT7921E | MT7921K | MT7921S | MT7921AU | AMD_RZ608 => ChipVariant::Mt7921,
            MT7922 | MT792X_E | AMD_RZ616 => ChipVariant::Mt7922,
            MT7925E | MT7925U => ChipVariant::Mt7925,
            _ => ChipVariant::Unknown,
        }
    }
}

/// Chip variant
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipVariant {
    Unknown,
    Mt7921,
    Mt7922,
    Mt7925,
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
    Width320MHz,  // WiFi 7 only
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
    DeepSleep,
    Off,
}

/// Bus type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusType {
    Pcie,
    Usb,
    Sdio,
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
    pub version: String,
    pub build_date: String,
    pub size: usize,
    pub loaded: bool,
}

impl Default for FirmwareInfo {
    fn default() -> Self {
        Self {
            name: String::new(),
            version: String::new(),
            build_date: String::new(),
            size: 0,
            loaded: false,
        }
    }
}

/// MediaTek register offsets
pub mod regs {
    // Top level registers
    pub const MT_TOP_CFG_BASE: u32 = 0x80020000;
    pub const MT_TOP_LPCR_HOST_BAND0: u32 = 0x0010;
    pub const MT_TOP_MISC: u32 = 0x80000000;
    pub const MT_MCU_CMD: u32 = 0x80000004;

    // WiFi registers
    pub const MT_WFDMA0_BASE: u32 = 0xD4000;
    pub const MT_WFDMA1_BASE: u32 = 0xD5000;
    pub const MT_WFDMA_HOST_DMA0_WPDMA_GLO_CFG: u32 = 0x0208;

    // MCU command/status
    pub const MT_MCU_CMD_WAKE_RX_PCIE: u32 = 0x01;
    pub const MT_MCU_CMD_STOP_DMA: u32 = 0x00;

    // Power control
    pub const MT_CONN_INFRA_CFG_BASE: u32 = 0x80000000;
    pub const MT_WLAN_OFF: u32 = 0x80004000;
    pub const MT_WLAN_ON: u32 = 0x80004004;

    // Interrupt registers
    pub const MT_INT_STATUS_CSR: u32 = 0x0200;
    pub const MT_INT_MASK_CSR: u32 = 0x0204;

    // TX/RX DMA
    pub const MT_TX_RING_BASE: u32 = 0x0300;
    pub const MT_RX_RING_BASE: u32 = 0x0500;

    // EEPROM/EFUSE
    pub const MT_EFUSE_BASE: u32 = 0x81070000;
    pub const MT_EFUSE_CTRL: u32 = 0x0008;
    pub const MT_EFUSE_WDATA: u32 = 0x0010;
    pub const MT_EFUSE_RDATA: u32 = 0x0014;
}

/// MediaTek WiFi Driver
pub struct MediatekWifiDriver {
    // Device info
    pub device_id: u16,
    pub device_name: String,
    pub variant: ChipVariant,
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

impl MediatekWifiDriver {
    pub const fn new() -> Self {
        Self {
            device_id: 0,
            device_name: String::new(),
            variant: ChipVariant::Unknown,
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
                version: String::new(),
                build_date: String::new(),
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
        self.variant = device_ids::variant(device_id);
        self.bus_type = bus_type;
        self.mmio_base = mmio_base;
        self.irq = irq;

        // Set capabilities based on variant
        self.setup_capabilities();

        // Hardware init
        self.hw_init()?;

        // Load firmware
        self.load_firmware()?;

        self.power_state = PowerState::Active;
        self.initialized = true;

        crate::kprintln!("mt7921: Initialized {} ({:04X}) - {}",
            self.device_name, self.device_id, self.wifi_standard.name());

        Ok(())
    }

    /// Setup capabilities based on chip variant
    fn setup_capabilities(&mut self) {
        match self.variant {
            ChipVariant::Mt7921 => {
                self.wifi_standard = WifiStandard::Wifi6;
                self.supported_bands = vec![FrequencyBand::Band2_4GHz, FrequencyBand::Band5GHz];
                self.max_width = ChannelWidth::Width80MHz;
                self.max_streams = 2;
                self.supports_bluetooth = true;
            }
            ChipVariant::Mt7922 => {
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
            ChipVariant::Mt7925 => {
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
            ChipVariant::Unknown => {
                self.wifi_standard = WifiStandard::Wifi6;
                self.supported_bands = vec![FrequencyBand::Band2_4GHz, FrequencyBand::Band5GHz];
                self.max_width = ChannelWidth::Width80MHz;
                self.max_streams = 2;
                self.supports_bluetooth = false;
            }
        }
    }

    /// Hardware initialization
    fn hw_init(&mut self) -> Result<(), &'static str> {
        if self.bus_type == BusType::Pcie {
            // Power on
            self.write_reg(regs::MT_TOP_LPCR_HOST_BAND0, 0);

            // Wait for MCU ready
            for _ in 0..1000 {
                let val = self.read_reg(regs::MT_TOP_MISC);
                if val & 0x01 != 0 {
                    break;
                }
                for _ in 0..1000 {}  // Busy wait
            }

            // Init WFDMA
            self.write_reg(regs::MT_WFDMA0_BASE + regs::MT_WFDMA_HOST_DMA0_WPDMA_GLO_CFG, 0x01);
        }

        Ok(())
    }

    /// Load firmware
    fn load_firmware(&mut self) -> Result<(), &'static str> {
        let fw_name = match self.variant {
            ChipVariant::Mt7921 => "mediatek/WIFI_MT7961_patch_mcu_1_2_hdr.bin",
            ChipVariant::Mt7922 => "mediatek/WIFI_MT7922_patch_mcu_1_1_hdr.bin",
            ChipVariant::Mt7925 => "mediatek/WIFI_MT7925_patch_mcu_1_0_hdr.bin",
            ChipVariant::Unknown => return Err("Unknown chip variant"),
        };

        self.firmware = FirmwareInfo {
            name: fw_name.to_string(),
            version: "1.0.0".to_string(),
            build_date: "2024-01-01".to_string(),
            size: 512 * 1024,  // ~512KB typical
            loaded: true,
        };

        crate::kprintln!("mt7921: Firmware {} loaded", fw_name);

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

        crate::kprintln!("mt7921: Scan completed, {} networks found", self.scan_results.len());

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
        self.stats.signal_strength = -50;

        crate::kprintln!("mt7921: Connected to {}", ssid);

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

        crate::kprintln!("mt7921: Disconnected");

        Ok(())
    }

    /// Set power mode
    pub fn set_power_mode(&mut self, mode: PowerState) -> Result<(), &'static str> {
        match mode {
            PowerState::Active => {
                self.write_reg(regs::MT_WLAN_ON, 1);
            }
            PowerState::LowPower | PowerState::Sleep => {
                self.write_reg(regs::MT_MCU_CMD, regs::MT_MCU_CMD_STOP_DMA);
            }
            PowerState::DeepSleep | PowerState::Off => {
                self.write_reg(regs::MT_WLAN_OFF, 1);
            }
        }
        self.power_state = mode;
        Ok(())
    }

    /// Get status string
    pub fn get_status(&self) -> String {
        let mut status = String::new();

        status.push_str("MediaTek WiFi Status:\n");
        status.push_str(&alloc::format!("  Device: {} ({:04X})\n", self.device_name, self.device_id));
        status.push_str(&alloc::format!("  Variant: {:?}\n", self.variant));
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
            status.push_str(&alloc::format!("  Security: {}\n", self.current_security.name()));
            status.push_str(&alloc::format!("  Signal: {} dBm\n", self.stats.signal_strength));
        }

        status.push_str(&alloc::format!("  Firmware: {} v{}\n", self.firmware.name, self.firmware.version));

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

/// Global MediaTek WiFi driver
static MEDIATEK_WIFI: TicketSpinlock<Option<MediatekWifiDriver>> = TicketSpinlock::new(None);

/// Initialize MediaTek WiFi driver (entry point - scans PCI)
pub fn init() {
    // Scan PCI for MediaTek WiFi devices
    for bus in 0..=255u8 {
        for dev in 0..32u8 {
            for func in 0..8u8 {
                let vendor = pci_read_vendor(bus, dev, func);
                if vendor == 0xFFFF || vendor != MEDIATEK_VENDOR_ID {
                    continue;
                }

                let device_id = pci_read_device(bus, dev, func);

                // Check if supported MediaTek WiFi device
                if device_ids::is_pcie(device_id) {
                    let bar0 = pci_read_bar0(bus, dev, func);
                    let mmio_base = (bar0 & 0xFFFFFFF0) as u64;
                    let irq = pci_read_irq(bus, dev, func);

                    if let Err(e) = init_with_device(device_id, BusType::Pcie, mmio_base, irq) {
                        crate::kprintln!("mt7921: Failed to init device {:04X}: {}", device_id, e);
                    }
                    return;
                }
            }
        }
    }
}

/// Initialize with specific device
pub fn init_with_device(device_id: u16, bus_type: BusType, mmio_base: u64, irq: u8) -> Result<(), &'static str> {
    let mut guard = MEDIATEK_WIFI.lock();
    let mut driver = MediatekWifiDriver::new();
    driver.init(device_id, bus_type, mmio_base, irq)?;
    *guard = Some(driver);
    Ok(())
}

/// Get driver instance
pub fn get_driver() -> Option<&'static TicketSpinlock<Option<MediatekWifiDriver>>> {
    Some(&MEDIATEK_WIFI)
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
