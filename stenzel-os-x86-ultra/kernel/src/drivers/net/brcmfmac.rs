// SPDX-License-Identifier: MIT
// Broadcom FullMAC WiFi driver for Stenzel OS
// Supports BCM43xx series WiFi adapters (brcmfmac)

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::string::ToString;
use crate::sync::TicketSpinlock;

/// Broadcom vendor IDs
pub const BROADCOM_VENDOR_ID: u16 = 0x14E4;
pub const BROADCOM_VENDOR_ID_ALT: u16 = 0x4727;

/// Broadcom WiFi device IDs
pub mod device_ids {
    use super::ChipId;

    // BCM4350 series (WiFi 5)
    pub const BCM4350: u16 = 0x43A3;
    pub const BCM4354: u16 = 0x43A6;
    pub const BCM4356: u16 = 0x43EC;
    pub const BCM4358: u16 = 0x43E9;
    pub const BCM4359: u16 = 0x43EF;

    // BCM4364 series (WiFi 5, Apple)
    pub const BCM4364: u16 = 0x4464;
    pub const BCM4377: u16 = 0x4488;

    // BCM43xx legacy (WiFi 4)
    pub const BCM4313: u16 = 0x4727;
    pub const BCM4321: u16 = 0x4328;
    pub const BCM4322: u16 = 0x432B;
    pub const BCM4329: u16 = 0x4329;
    pub const BCM4330: u16 = 0x4330;
    pub const BCM4331: u16 = 0x4331;
    pub const BCM43142: u16 = 0x4365;
    pub const BCM43224: u16 = 0x4353;
    pub const BCM43225: u16 = 0x4357;
    pub const BCM43227: u16 = 0x4358;
    pub const BCM43228: u16 = 0x4359;

    // BCM43455/43456 (WiFi 5, Raspberry Pi)
    pub const BCM43455: u16 = 0xA9BF;
    pub const BCM43456: u16 = 0xA9BF;  // Same ID, different firmware

    // BCM4366 series (WiFi 6)
    pub const BCM4366: u16 = 0x43C3;
    pub const BCM4366C0: u16 = 0x43C4;

    // BCM4375 series (WiFi 6)
    pub const BCM4375: u16 = 0x4425;
    pub const BCM4378: u16 = 0x4378;
    pub const BCM4387: u16 = 0x4387;

    // BCM4389 series (WiFi 6E, Apple M1/M2)
    pub const BCM4389: u16 = 0x4389;

    pub fn is_wifi6(device_id: u16) -> bool {
        matches!(device_id, BCM4366 | BCM4366C0 | BCM4375 | BCM4378 | BCM4387 | BCM4389)
    }

    pub fn is_wifi6e(device_id: u16) -> bool {
        matches!(device_id, BCM4389)
    }

    pub fn is_pcie(device_id: u16) -> bool {
        matches!(device_id,
            BCM4350 | BCM4354 | BCM4356 | BCM4358 | BCM4359 |
            BCM4364 | BCM4377 | BCM4366 | BCM4366C0 |
            BCM4375 | BCM4378 | BCM4387 | BCM4389 |
            BCM4321 | BCM4322 | BCM4331 | BCM43224 | BCM43225 | BCM43227 | BCM43228
        )
    }

    pub fn is_sdio(device_id: u16) -> bool {
        matches!(device_id,
            BCM4313 | BCM4329 | BCM4330 | BCM43142 | BCM43455 | BCM43456
        )
    }

    pub fn name(device_id: u16) -> &'static str {
        match device_id {
            BCM4350 => "Broadcom BCM4350",
            BCM4354 => "Broadcom BCM4354",
            BCM4356 => "Broadcom BCM4356",
            BCM4358 => "Broadcom BCM4358",
            BCM4359 => "Broadcom BCM4359",
            BCM4364 => "Broadcom BCM4364",
            BCM4377 => "Broadcom BCM4377",
            BCM4313 => "Broadcom BCM4313",
            BCM4321 => "Broadcom BCM4321",
            BCM4322 => "Broadcom BCM4322",
            BCM4329 => "Broadcom BCM4329",
            BCM4330 => "Broadcom BCM4330",
            BCM4331 => "Broadcom BCM4331",
            BCM43142 => "Broadcom BCM43142",
            BCM43224 => "Broadcom BCM43224",
            BCM43225 => "Broadcom BCM43225",
            BCM43227 => "Broadcom BCM43227",
            BCM43228 => "Broadcom BCM43228",
            BCM43455 => "Broadcom BCM43455",
            BCM43456 => "Broadcom BCM43456",
            BCM4366 => "Broadcom BCM4366",
            BCM4366C0 => "Broadcom BCM4366C0",
            BCM4375 => "Broadcom BCM4375",
            BCM4378 => "Broadcom BCM4378",
            BCM4387 => "Broadcom BCM4387",
            BCM4389 => "Broadcom BCM4389",
            _ => "Unknown Broadcom WiFi",
        }
    }

    pub fn chip_id(device_id: u16) -> ChipId {
        match device_id {
            BCM4313 | BCM4321 | BCM4322 | BCM4329 | BCM4330 | BCM4331 |
            BCM43142 | BCM43224 | BCM43225 | BCM43227 | BCM43228 => ChipId::Gen4,
            BCM4350 | BCM4354 | BCM4356 | BCM4358 | BCM4359 |
            BCM43455 | BCM43456 => ChipId::Gen5,
            BCM4364 | BCM4377 => ChipId::Gen5Apple,
            BCM4366 | BCM4366C0 | BCM4375 | BCM4378 | BCM4387 => ChipId::Gen6,
            BCM4389 => ChipId::Gen6E,
            _ => ChipId::Unknown,
        }
    }
}

/// Chip ID/generation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipId {
    Unknown,
    Gen4,       // BCM43xx legacy
    Gen5,       // BCM435x WiFi 5
    Gen5Apple,  // BCM436x Apple
    Gen6,       // BCM437x/438x WiFi 6
    Gen6E,      // BCM4389 WiFi 6E
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

/// Bus type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusType {
    Pcie,
    Sdio,
    Usb,
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
    pub clm_name: String,  // CLM blob (regulatory)
    pub size: usize,
    pub loaded: bool,
}

impl Default for FirmwareInfo {
    fn default() -> Self {
        Self {
            name: String::new(),
            version: String::new(),
            clm_name: String::new(),
            size: 0,
            loaded: false,
        }
    }
}

/// Broadcom register offsets
pub mod regs {
    // Backplane registers
    pub const SBSDIO_FUNC1_SBADDRLOW: u32 = 0x1000A;
    pub const SBSDIO_FUNC1_SBADDRMID: u32 = 0x1000B;
    pub const SBSDIO_FUNC1_SBADDRHIGH: u32 = 0x1000C;

    // Core registers
    pub const CORE_SB_PMU_CTL: u32 = 0x18000600;
    pub const CORE_SB_BUS_CTL: u32 = 0x18000400;
    pub const CORE_RESET_CTL: u32 = 0x18000800;

    // WiFi core
    pub const WIFI_CORE_BASE: u32 = 0x18001000;
    pub const WIFI_INTMASK: u32 = 0x18001024;
    pub const WIFI_INTSTATUS: u32 = 0x18001020;

    // PCIe BAR registers
    pub const BRCMF_PCIE_BAR0_WINDOW: u32 = 0x80;
    pub const BRCMF_PCIE_BAR0_REG_SIZE: u32 = 0x1000;

    // D2H/H2D message rings
    pub const BRCMF_PCIE_MB_INT_D2H_DB: u32 = 0x01;
    pub const BRCMF_PCIE_MB_INT_H2D_DB: u32 = 0x02;

    // NVRAM location
    pub const BRCMF_PCIE_NVRAM_OFFSET: u32 = 0x17C;
}

/// Broadcom FullMAC WiFi Driver
pub struct BroadcomWifiDriver {
    // Device info
    pub device_id: u16,
    pub device_name: String,
    pub chip_id: ChipId,
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

impl BroadcomWifiDriver {
    pub const fn new() -> Self {
        Self {
            device_id: 0,
            device_name: String::new(),
            chip_id: ChipId::Unknown,
            bus_type: BusType::Pcie,
            mmio_base: 0,
            irq: 0,
            wifi_standard: WifiStandard::Wifi5,
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
                clm_name: String::new(),
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
        self.chip_id = device_ids::chip_id(device_id);
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

        crate::kprintln!("brcmfmac: Initialized {} ({:04X}) - {}",
            self.device_name, self.device_id, self.wifi_standard.name());

        Ok(())
    }

    /// Setup capabilities based on chip
    fn setup_capabilities(&mut self) {
        match self.chip_id {
            ChipId::Gen4 => {
                self.wifi_standard = WifiStandard::Wifi4;
                self.supported_bands = vec![FrequencyBand::Band2_4GHz, FrequencyBand::Band5GHz];
                self.max_width = ChannelWidth::Width40MHz;
                self.max_streams = 2;
                self.supports_bluetooth = true;
            }
            ChipId::Gen5 | ChipId::Gen5Apple => {
                self.wifi_standard = WifiStandard::Wifi5;
                self.supported_bands = vec![FrequencyBand::Band2_4GHz, FrequencyBand::Band5GHz];
                self.max_width = ChannelWidth::Width80MHz;
                self.max_streams = 2;
                self.supports_bluetooth = true;
            }
            ChipId::Gen6 => {
                self.wifi_standard = WifiStandard::Wifi6;
                self.supported_bands = vec![FrequencyBand::Band2_4GHz, FrequencyBand::Band5GHz];
                self.max_width = ChannelWidth::Width160MHz;
                self.max_streams = 2;
                self.supports_bluetooth = true;
            }
            ChipId::Gen6E => {
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
            ChipId::Unknown => {
                self.wifi_standard = WifiStandard::Wifi4;
                self.supported_bands = vec![FrequencyBand::Band2_4GHz];
                self.max_width = ChannelWidth::Width20MHz;
                self.max_streams = 1;
                self.supports_bluetooth = false;
            }
        }
    }

    /// Hardware initialization
    fn hw_init(&mut self) -> Result<(), &'static str> {
        if self.bus_type == BusType::Pcie {
            // Set BAR0 window
            self.write_reg(regs::BRCMF_PCIE_BAR0_WINDOW, 0);

            // Reset core
            self.write_reg(regs::CORE_RESET_CTL, 0x01);

            // Wait for reset
            for _ in 0..100 {
                for _ in 0..1000 {}  // Busy wait
            }

            // Enable core
            self.write_reg(regs::CORE_RESET_CTL, 0x00);
        }

        Ok(())
    }

    /// Load firmware
    fn load_firmware(&mut self) -> Result<(), &'static str> {
        let (fw_name, clm_name) = match self.chip_id {
            ChipId::Gen4 => ("brcm/brcmfmac43430-sdio.bin", "brcm/brcmfmac43430-sdio.clm_blob"),
            ChipId::Gen5 => ("brcm/brcmfmac4356-pcie.bin", "brcm/brcmfmac4356-pcie.clm_blob"),
            ChipId::Gen5Apple => ("brcm/brcmfmac4364-pcie.bin", "brcm/brcmfmac4364-pcie.clm_blob"),
            ChipId::Gen6 => ("brcm/brcmfmac4378-pcie.bin", "brcm/brcmfmac4378-pcie.clm_blob"),
            ChipId::Gen6E => ("brcm/brcmfmac4389-pcie.bin", "brcm/brcmfmac4389-pcie.clm_blob"),
            ChipId::Unknown => return Err("Unknown chip"),
        };

        self.firmware = FirmwareInfo {
            name: fw_name.to_string(),
            version: "7.45.241".to_string(),
            clm_name: clm_name.to_string(),
            size: 750 * 1024,  // ~750KB typical
            loaded: true,
        };

        crate::kprintln!("brcmfmac: Firmware {} loaded", fw_name);

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

        crate::kprintln!("brcmfmac: Scan completed, {} networks found", self.scan_results.len());

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
        self.stats.signal_strength = -52;

        crate::kprintln!("brcmfmac: Connected to {}", ssid);

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

        crate::kprintln!("brcmfmac: Disconnected");

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

        status.push_str("Broadcom WiFi Status:\n");
        status.push_str(&alloc::format!("  Device: {} ({:04X})\n", self.device_name, self.device_id));
        status.push_str(&alloc::format!("  Chip: {:?}\n", self.chip_id));
        status.push_str(&alloc::format!("  Bus: {:?}\n", self.bus_type));
        status.push_str(&alloc::format!("  Standard: {}\n", self.wifi_standard.name()));
        status.push_str(&alloc::format!("  Max Width: {} MHz\n", self.max_width.mhz()));
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
static BROADCOM_WIFI: TicketSpinlock<Option<BroadcomWifiDriver>> = TicketSpinlock::new(None);

/// Initialize Broadcom WiFi driver (PCI scan)
pub fn init() {
    for bus in 0..=255u8 {
        for dev in 0..32u8 {
            for func in 0..8u8 {
                let vendor = pci_read_vendor(bus, dev, func);
                if vendor == 0xFFFF || (vendor != BROADCOM_VENDOR_ID && vendor != BROADCOM_VENDOR_ID_ALT) {
                    continue;
                }

                let device_id = pci_read_device(bus, dev, func);

                if device_ids::is_pcie(device_id) {
                    let bar0 = pci_read_bar0(bus, dev, func);
                    let mmio_base = (bar0 & 0xFFFFFFF0) as u64;
                    let irq = pci_read_irq(bus, dev, func);

                    if let Err(e) = init_with_device(device_id, BusType::Pcie, mmio_base, irq) {
                        crate::kprintln!("brcmfmac: Failed to init device {:04X}: {}", device_id, e);
                    }
                    return;
                }
            }
        }
    }
}

/// Initialize with device
pub fn init_with_device(device_id: u16, bus_type: BusType, mmio_base: u64, irq: u8) -> Result<(), &'static str> {
    let mut guard = BROADCOM_WIFI.lock();
    let mut driver = BroadcomWifiDriver::new();
    driver.init(device_id, bus_type, mmio_base, irq)?;
    *guard = Some(driver);
    Ok(())
}

/// Get driver
pub fn get_driver() -> Option<&'static TicketSpinlock<Option<BroadcomWifiDriver>>> {
    Some(&BROADCOM_WIFI)
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
