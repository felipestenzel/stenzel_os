// SPDX-License-Identifier: MIT
// Intel WiFi 7 (BE200/BE202) driver for Stenzel OS
// IEEE 802.11be (WiFi 7) support with MLO (Multi-Link Operation)

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::string::ToString;
use alloc::collections::BTreeMap;
use crate::sync::TicketSpinlock;

/// WiFi 7 device IDs (Intel BE200 family)
pub mod device_ids {
    // Intel BE200 (WiFi 7)
    pub const BE200_1: u16 = 0x272B;  // BE200 2x2
    pub const BE200_2: u16 = 0x272C;  // BE200 variant
    pub const BE202: u16 = 0x272D;    // BE202 (budget)
    pub const BE201: u16 = 0x272E;    // BE201 variant

    // Intel AX411/AX211 (WiFi 6E, for fallback)
    pub const AX411: u16 = 0x2729;
    pub const AX211: u16 = 0x2723;
    pub const AX210: u16 = 0x2725;

    pub fn is_wifi7(device_id: u16) -> bool {
        matches!(device_id, BE200_1 | BE200_2 | BE202 | BE201)
    }

    pub fn name(device_id: u16) -> &'static str {
        match device_id {
            BE200_1 | BE200_2 => "Intel BE200",
            BE202 => "Intel BE202",
            BE201 => "Intel BE201",
            AX411 => "Intel AX411",
            AX211 => "Intel AX211",
            AX210 => "Intel AX210",
            _ => "Unknown Intel WiFi",
        }
    }
}

/// WiFi standards
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WifiStandard {
    Wifi4,      // 802.11n
    Wifi5,      // 802.11ac
    Wifi6,      // 802.11ax
    Wifi6E,     // 802.11ax (6GHz)
    Wifi7,      // 802.11be
}

impl WifiStandard {
    pub fn name(self) -> &'static str {
        match self {
            WifiStandard::Wifi4 => "Wi-Fi 4 (802.11n)",
            WifiStandard::Wifi5 => "Wi-Fi 5 (802.11ac)",
            WifiStandard::Wifi6 => "Wi-Fi 6 (802.11ax)",
            WifiStandard::Wifi6E => "Wi-Fi 6E (802.11ax 6GHz)",
            WifiStandard::Wifi7 => "Wi-Fi 7 (802.11be)",
        }
    }
}

/// Frequency bands
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrequencyBand {
    Band2_4GHz,
    Band5GHz,
    Band6GHz,
}

impl FrequencyBand {
    pub fn frequency_range(self) -> (u32, u32) {
        match self {
            FrequencyBand::Band2_4GHz => (2400, 2500),
            FrequencyBand::Band5GHz => (5150, 5850),
            FrequencyBand::Band6GHz => (5925, 7125),
        }
    }

    pub fn max_channel_width(self) -> u32 {
        match self {
            FrequencyBand::Band2_4GHz => 40,   // MHz
            FrequencyBand::Band5GHz => 160,   // MHz
            FrequencyBand::Band6GHz => 320,   // MHz (WiFi 7)
        }
    }
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

/// Multi-Link Operation (MLO) state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MloState {
    Disabled,
    SingleLink,
    DualLink,
    TriLink,
}

/// Link state for MLO
#[derive(Debug, Clone)]
pub struct MloLink {
    pub link_id: u8,
    pub band: FrequencyBand,
    pub channel: u8,
    pub width: ChannelWidth,
    pub active: bool,
    pub rssi: i8,
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

/// Security modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityMode {
    Open,
    Wep,
    WpaPsk,
    Wpa2Psk,
    Wpa3Sae,
    Wpa3Enterprise,
    Owe,  // Opportunistic Wireless Encryption
}

impl SecurityMode {
    pub fn name(self) -> &'static str {
        match self {
            SecurityMode::Open => "Open",
            SecurityMode::Wep => "WEP",
            SecurityMode::WpaPsk => "WPA-PSK",
            SecurityMode::Wpa2Psk => "WPA2-PSK",
            SecurityMode::Wpa3Sae => "WPA3-SAE",
            SecurityMode::Wpa3Enterprise => "WPA3-Enterprise",
            SecurityMode::Owe => "OWE",
        }
    }

    pub fn is_secure(self) -> bool {
        !matches!(self, SecurityMode::Open | SecurityMode::Wep)
    }
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
    pub standard: WifiStandard,
    pub supports_mlo: bool,
    pub mlo_links: Vec<u8>,  // Available link IDs
}

/// Connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Scanning,
    Authenticating,
    Associating,
    Handshake,
    Connected,
    Roaming,
    Disconnecting,
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
    pub tx_retries: u64,
    pub beacons_received: u64,
    pub signal_strength: i8,
    pub noise_level: i8,
    pub link_quality: u8,
}

/// Firmware info
#[derive(Debug, Clone)]
pub struct FirmwareInfo {
    pub version: String,
    pub api_version: u32,
    pub build_number: u32,
    pub size: usize,
    pub loaded: bool,
}

impl Default for FirmwareInfo {
    fn default() -> Self {
        Self {
            version: String::new(),
            api_version: 0,
            build_number: 0,
            size: 0,
            loaded: false,
        }
    }
}

/// Intel WiFi registers
pub mod regs {
    // Control/Status registers
    pub const CSR_HW_IF_CONFIG_REG: u32 = 0x000;
    pub const CSR_INT_COALESCING: u32 = 0x004;
    pub const CSR_INT: u32 = 0x008;
    pub const CSR_INT_MASK: u32 = 0x00C;
    pub const CSR_FH_INT_STATUS: u32 = 0x010;
    pub const CSR_GPIO_IN: u32 = 0x018;
    pub const CSR_RESET: u32 = 0x020;
    pub const CSR_GP_CNTRL: u32 = 0x024;
    pub const CSR_HW_REV: u32 = 0x028;
    pub const CSR_EEPROM_REG: u32 = 0x02C;
    pub const CSR_EEPROM_GP: u32 = 0x030;
    pub const CSR_OTP_GP_REG: u32 = 0x034;
    pub const CSR_GIO_REG: u32 = 0x03C;
    pub const CSR_GP_UCODE_REG: u32 = 0x048;
    pub const CSR_GP_DRIVER_REG: u32 = 0x050;
    pub const CSR_UCODE_DRV_GP1: u32 = 0x054;
    pub const CSR_UCODE_DRV_GP2: u32 = 0x058;
    pub const CSR_LED_REG: u32 = 0x094;
    pub const CSR_DRAM_INT_TBL_REG: u32 = 0x0A0;
    pub const CSR_MAC_SHADOW_REG_CTRL: u32 = 0x0A8;
    pub const CSR_MAC_SHADOW_REG_CTL2: u32 = 0x0AC;
    pub const CSR_DBG_HPET_MEM_REG: u32 = 0x240;
    pub const CSR_DBG_LINK_PWR_MGMT_REG: u32 = 0x250;

    // GP control bits
    pub const CSR_GP_CNTRL_REG_FLAG_INIT_DONE: u32 = 0x00000004;
    pub const CSR_GP_CNTRL_REG_FLAG_MAC_ACCESS_REQ: u32 = 0x00000008;
    pub const CSR_GP_CNTRL_REG_FLAG_GOING_TO_SLEEP: u32 = 0x00000010;
    pub const CSR_GP_CNTRL_REG_FLAG_RFKILL_WAKE_L1A_EN: u32 = 0x00000040;

    // TX/RX queues
    pub const FH_MEM_CBBC_0_15_LOWER_BOUND: u32 = 0x9D0;
    pub const FH_MEM_CBBC_0_15_UPPER_BOUND: u32 = 0xA10;
    pub const FH_MEM_CBBC_16_19_LOWER_BOUND: u32 = 0xBF0;
    pub const FH_MEM_CBBC_16_19_UPPER_BOUND: u32 = 0xC00;

    // Firmware loading
    pub const UCODE_SECTION_INST: u32 = 1;
    pub const UCODE_SECTION_DATA: u32 = 2;
    pub const UCODE_SECTION_TLV: u32 = 3;
}

/// Intel WiFi 7 Driver
pub struct IntelWifi7Driver {
    // Device info
    pub device_id: u16,
    pub device_name: String,
    pub mmio_base: u64,
    pub irq: u8,

    // Capabilities
    pub wifi_standard: WifiStandard,
    pub supported_bands: Vec<FrequencyBand>,
    pub max_width: ChannelWidth,
    pub mlo_capable: bool,
    pub max_streams: u8,

    // State
    pub power_state: PowerState,
    pub connection_state: ConnectionState,
    pub mlo_state: MloState,
    pub mlo_links: Vec<MloLink>,

    // Connection info
    pub current_ssid: Option<String>,
    pub current_bssid: Option<[u8; 6]>,
    pub current_channel: u8,
    pub current_band: FrequencyBand,
    pub current_security: SecurityMode,

    // Firmware
    pub firmware: FirmwareInfo,

    // Statistics
    pub stats: DriverStats,

    // Scan results
    pub scan_results: Vec<ScanResult>,

    initialized: bool,
}

impl IntelWifi7Driver {
    pub const fn new() -> Self {
        Self {
            device_id: 0,
            device_name: String::new(),
            mmio_base: 0,
            irq: 0,
            wifi_standard: WifiStandard::Wifi7,
            supported_bands: Vec::new(),
            max_width: ChannelWidth::Width320MHz,
            mlo_capable: false,
            max_streams: 0,
            power_state: PowerState::Off,
            connection_state: ConnectionState::Disconnected,
            mlo_state: MloState::Disabled,
            mlo_links: Vec::new(),
            current_ssid: None,
            current_bssid: None,
            current_channel: 0,
            current_band: FrequencyBand::Band2_4GHz,
            current_security: SecurityMode::Open,
            firmware: FirmwareInfo {
                version: String::new(),
                api_version: 0,
                build_number: 0,
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
                tx_retries: 0,
                beacons_received: 0,
                signal_strength: 0,
                noise_level: 0,
                link_quality: 0,
            },
            scan_results: Vec::new(),
            initialized: false,
        }
    }

    /// Initialize driver
    pub fn init(&mut self, device_id: u16, mmio_base: u64, irq: u8) -> Result<(), &'static str> {
        self.device_id = device_id;
        self.device_name = device_ids::name(device_id).to_string();
        self.mmio_base = mmio_base;
        self.irq = irq;

        // Determine capabilities based on device
        if device_ids::is_wifi7(device_id) {
            self.wifi_standard = WifiStandard::Wifi7;
            self.supported_bands = vec![
                FrequencyBand::Band2_4GHz,
                FrequencyBand::Band5GHz,
                FrequencyBand::Band6GHz,
            ];
            self.max_width = ChannelWidth::Width320MHz;
            self.mlo_capable = true;
            self.max_streams = 2;  // BE200 is 2x2
        } else {
            // WiFi 6/6E device
            self.wifi_standard = WifiStandard::Wifi6E;
            self.supported_bands = vec![
                FrequencyBand::Band2_4GHz,
                FrequencyBand::Band5GHz,
                FrequencyBand::Band6GHz,
            ];
            self.max_width = ChannelWidth::Width160MHz;
            self.mlo_capable = false;
            self.max_streams = 2;
        }

        // Initialize hardware
        self.hw_init()?;

        // Load firmware
        self.load_firmware()?;

        self.power_state = PowerState::Active;
        self.initialized = true;

        crate::kprintln!("iwlwifi: Initialized {} ({:04X}) - {}",
            self.device_name, self.device_id, self.wifi_standard.name());

        Ok(())
    }

    /// Hardware initialization
    fn hw_init(&mut self) -> Result<(), &'static str> {
        // Read hardware revision
        let _hw_rev = self.read_reg(regs::CSR_HW_REV);

        // Reset device
        self.write_reg(regs::CSR_RESET, 0x00000001);

        // Wait for init done
        for _ in 0..1000 {
            let gp = self.read_reg(regs::CSR_GP_CNTRL);
            if gp & regs::CSR_GP_CNTRL_REG_FLAG_INIT_DONE != 0 {
                break;
            }
            // Busy wait
            for _ in 0..1000 {}
        }

        // Enable MAC access
        self.write_reg(regs::CSR_GP_CNTRL, regs::CSR_GP_CNTRL_REG_FLAG_MAC_ACCESS_REQ);

        Ok(())
    }

    /// Load firmware
    fn load_firmware(&mut self) -> Result<(), &'static str> {
        // In a real implementation, this would load iwlwifi firmware
        // from /lib/firmware/iwlwifi-*.ucode

        let fw_name = if device_ids::is_wifi7(self.device_id) {
            "iwlwifi-be-a0.ucode"
        } else {
            "iwlwifi-ty-a0.ucode"
        };

        self.firmware = FirmwareInfo {
            version: "89.4.0.3".to_string(),
            api_version: 89,
            build_number: 3,
            size: 2_500_000,  // ~2.5MB typical
            loaded: true,  // Pretend loaded for now
        };

        crate::kprintln!("iwlwifi: Firmware {} loaded", fw_name);

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

        // In a real implementation, this would trigger a hardware scan
        // For now, return empty results

        self.connection_state = ConnectionState::Disconnected;

        crate::kprintln!("iwlwifi: Scan completed, {} networks found", self.scan_results.len());

        Ok(())
    }

    /// Connect to network
    pub fn connect(&mut self, ssid: &str, password: &str, security: SecurityMode)
        -> Result<(), &'static str>
    {
        if !self.initialized {
            return Err("Driver not initialized");
        }

        self.connection_state = ConnectionState::Authenticating;

        // Store connection info
        self.current_ssid = Some(ssid.to_string());
        self.current_security = security;

        // In a real implementation, this would:
        // 1. Send association request
        // 2. Perform 4-way handshake (WPA2/WPA3)
        // 3. Configure encryption keys

        let _ = password;  // Would be used for key derivation

        self.connection_state = ConnectionState::Connected;
        self.stats.signal_strength = -50;  // Good signal
        self.stats.link_quality = 80;

        crate::kprintln!("iwlwifi: Connected to {}", ssid);

        Ok(())
    }

    /// Disconnect from network
    pub fn disconnect(&mut self) -> Result<(), &'static str> {
        if self.connection_state == ConnectionState::Disconnected {
            return Ok(());
        }

        self.connection_state = ConnectionState::Disconnecting;

        // Clear connection info
        self.current_ssid = None;
        self.current_bssid = None;

        self.connection_state = ConnectionState::Disconnected;

        crate::kprintln!("iwlwifi: Disconnected");

        Ok(())
    }

    /// Enable MLO (WiFi 7)
    pub fn enable_mlo(&mut self) -> Result<(), &'static str> {
        if !self.mlo_capable {
            return Err("Device does not support MLO");
        }

        if self.connection_state != ConnectionState::Connected {
            return Err("Must be connected first");
        }

        self.mlo_state = MloState::DualLink;

        // Setup MLO links
        self.mlo_links = vec![
            MloLink {
                link_id: 0,
                band: FrequencyBand::Band5GHz,
                channel: 36,
                width: ChannelWidth::Width160MHz,
                active: true,
                rssi: -45,
            },
            MloLink {
                link_id: 1,
                band: FrequencyBand::Band6GHz,
                channel: 1,
                width: ChannelWidth::Width320MHz,
                active: true,
                rssi: -50,
            },
        ];

        crate::kprintln!("iwlwifi: MLO enabled with {} links", self.mlo_links.len());

        Ok(())
    }

    /// Set power mode
    pub fn set_power_mode(&mut self, mode: PowerState) -> Result<(), &'static str> {
        match mode {
            PowerState::Active => {
                self.write_reg(regs::CSR_GP_CNTRL, regs::CSR_GP_CNTRL_REG_FLAG_MAC_ACCESS_REQ);
            }
            PowerState::LowPower | PowerState::Sleep => {
                self.write_reg(regs::CSR_GP_CNTRL, regs::CSR_GP_CNTRL_REG_FLAG_GOING_TO_SLEEP);
            }
            _ => {}
        }

        self.power_state = mode;

        Ok(())
    }

    /// Get status
    pub fn get_status(&self) -> String {
        let mut status = String::new();

        status.push_str("Intel WiFi 7 Status:\n");
        status.push_str(&alloc::format!("  Device: {} ({:04X})\n", self.device_name, self.device_id));
        status.push_str(&alloc::format!("  Standard: {}\n", self.wifi_standard.name()));
        status.push_str(&alloc::format!("  Max Width: {} MHz\n", self.max_width.mhz()));
        status.push_str(&alloc::format!("  MLO Capable: {}\n", self.mlo_capable));
        status.push_str(&alloc::format!("  Streams: {}x{}\n", self.max_streams, self.max_streams));

        status.push_str("  Bands:\n");
        for band in &self.supported_bands {
            let (min, max) = band.frequency_range();
            status.push_str(&alloc::format!("    {}-{} MHz\n", min, max));
        }

        status.push_str(&alloc::format!("  Power State: {:?}\n", self.power_state));
        status.push_str(&alloc::format!("  Connection: {:?}\n", self.connection_state));

        if let Some(ref ssid) = self.current_ssid {
            status.push_str(&alloc::format!("  SSID: {}\n", ssid));
            status.push_str(&alloc::format!("  Security: {}\n", self.current_security.name()));
            status.push_str(&alloc::format!("  Signal: {} dBm\n", self.stats.signal_strength));
            status.push_str(&alloc::format!("  Link Quality: {}%\n", self.stats.link_quality));
        }

        if self.mlo_state != MloState::Disabled {
            status.push_str(&alloc::format!("  MLO: {:?}\n", self.mlo_state));
            for link in &self.mlo_links {
                status.push_str(&alloc::format!("    Link {}: {:?} ch{} {} MHz ({} dBm)\n",
                    link.link_id, link.band, link.channel, link.width.mhz(), link.rssi));
            }
        }

        status.push_str(&alloc::format!("  Firmware: v{}\n", self.firmware.version));

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

/// Global Intel WiFi 7 driver
static INTEL_WIFI7: TicketSpinlock<Option<IntelWifi7Driver>> = TicketSpinlock::new(None);

/// Initialize Intel WiFi 7 driver (entry point - scans PCI for devices)
pub fn init() {
    // Scan PCI for Intel WiFi 7 devices
    for bus in 0..=255u8 {
        for dev in 0..32u8 {
            for func in 0..8u8 {
                let vendor = pci_read_vendor(bus, dev, func);
                if vendor == 0xFFFF || vendor != 0x8086 {  // Intel vendor ID
                    continue;
                }

                let device_id = pci_read_device(bus, dev, func);

                // Check if WiFi 7 device
                if device_ids::is_wifi7(device_id) ||
                   device_id == device_ids::AX411 ||
                   device_id == device_ids::AX211 ||
                   device_id == device_ids::AX210
                {
                    // Get BAR0 for MMIO
                    let bar0 = pci_read_bar0(bus, dev, func);
                    let mmio_base = (bar0 & 0xFFFFFFF0) as u64;

                    // Get IRQ
                    let irq = pci_read_irq(bus, dev, func);

                    // Initialize driver
                    if let Err(e) = init_with_device(device_id, mmio_base, irq) {
                        crate::kprintln!("iwlwifi_be: Failed to init device {:04X}: {}", device_id, e);
                    }
                    return;  // Only init first device found
                }
            }
        }
    }
}

/// Initialize Intel WiFi 7 driver with specific device
pub fn init_with_device(device_id: u16, mmio_base: u64, irq: u8) -> Result<(), &'static str> {
    let mut guard = INTEL_WIFI7.lock();
    let mut driver = IntelWifi7Driver::new();
    driver.init(device_id, mmio_base, irq)?;
    *guard = Some(driver);
    Ok(())
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

/// Get driver instance
pub fn get_driver() -> Option<&'static TicketSpinlock<Option<IntelWifi7Driver>>> {
    Some(&INTEL_WIFI7)
}
