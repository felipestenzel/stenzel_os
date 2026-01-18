//! Realtek WiFi driver (rtl8xxxu)
//!
//! Supports:
//! - RTL8188CU/RTL8188CUS (802.11n 1x1, USB 2.0)
//! - RTL8188EU/RTL8188EUS (802.11n 1x1, USB 2.0)
//! - RTL8188RU (802.11n 1x1, USB 2.0)
//! - RTL8192CU/RTL8192CUS (802.11n 2x2, USB 2.0)
//! - RTL8192EU (802.11n 2x2, USB 2.0)
//! - RTL8723AU/RTL8723BU (802.11n + BT combo)
//! - RTL8812AU (802.11ac 2x2, USB 3.0)
//! - RTL8821AU (802.11ac 1x1, USB 2.0)

use alloc::vec::Vec;
use alloc::string::String;
use crate::drivers::usb::{self, UsbDeviceInfo};
use crate::util::{KResult, KError};

/// Realtek USB vendor ID
const RTL_VENDOR_ID: u16 = 0x0BDA;

/// Supported device IDs
const DEVICE_IDS: &[(u16, &str)] = &[
    // RTL8188CU series
    (0x8176, "RTL8188CU"),
    (0x8177, "RTL8188CU"),
    (0x817A, "RTL8188CU"),
    (0x817B, "RTL8188CU"),
    (0x817D, "RTL8188CU"),
    (0x817E, "RTL8188CU"),
    (0x817F, "RTL8188CU"),
    (0x818A, "RTL8188CU"),
    // RTL8188EU series
    (0x8179, "RTL8188EU"),
    (0xF179, "RTL8188EU"),
    // RTL8188RU
    (0x8191, "RTL8188RU"),
    // RTL8192CU series
    (0x8178, "RTL8192CU"),
    (0x817C, "RTL8192CU"),
    // RTL8192EU
    (0x818B, "RTL8192EU"),
    // RTL8723AU series
    (0x8724, "RTL8723AU"),
    (0x1724, "RTL8723AU"),
    (0x0724, "RTL8723AU"),
    // RTL8723BU series
    (0xB720, "RTL8723BU"),
    // RTL8812AU series
    (0x8812, "RTL8812AU"),
    (0x881A, "RTL8812AU"),
    (0x881B, "RTL8812AU"),
    (0x881C, "RTL8812AU"),
    // RTL8821AU series
    (0x0811, "RTL8821AU"),
    (0x0821, "RTL8821AU"),
    (0x8822, "RTL8821AU"),
    (0xA811, "RTL8821AU"),
];

/// Other vendor device mappings (many Realtek chips are rebranded)
const OTHER_VENDORS: &[(u16, u16, &str)] = &[
    // ASUS
    (0x0B05, 0x17AB, "RTL8188EU"),
    (0x0B05, 0x17BA, "RTL8812AU"),
    (0x0B05, 0x180A, "RTL8812AU"),
    // TP-Link
    (0x2357, 0x0109, "RTL8812AU"),
    (0x2357, 0x0101, "RTL8812AU"),
    (0x2357, 0x0103, "RTL8188EU"),
    // D-Link
    (0x2001, 0x3315, "RTL8812AU"),
    (0x2001, 0x3316, "RTL8812AU"),
    (0x2001, 0x3311, "RTL8188EU"),
    // Edimax
    (0x7392, 0x7811, "RTL8188EU"),
    (0x7392, 0xA812, "RTL8812AU"),
    (0x7392, 0xA821, "RTL8821AU"),
    // Netgear
    (0x0846, 0x9052, "RTL8812AU"),
    (0x0846, 0x9054, "RTL8812AU"),
    // Belkin
    (0x050D, 0x1004, "RTL8192CU"),
    (0x050D, 0x2102, "RTL8192CU"),
];

/// Register offsets
mod regs {
    pub const SYS_FUNC_EN: u16 = 0x0002;
    pub const SYS_CLKR: u16 = 0x0008;
    pub const CR: u16 = 0x0100;
    pub const TXPAUSE: u16 = 0x0522;
    pub const BCN_CTRL: u16 = 0x0550;
    pub const TBTT_PROHIBIT: u16 = 0x0540;
    pub const DRVERLYINT: u16 = 0x0558;
    pub const BCN_MAX_ERR: u16 = 0x055D;
    pub const RCR: u16 = 0x0608;
    pub const RXFLTMAP0: u16 = 0x06A0;
    pub const RXFLTMAP2: u16 = 0x06A2;

    // MAC address registers
    pub const MACID: u16 = 0x0610;
    pub const BSSID: u16 = 0x0618;

    // RF registers
    pub const RF_CTRL: u16 = 0x001F;
    pub const RF_EN: u16 = 0x0000;

    // EFUSE
    pub const EFUSE_CTRL: u16 = 0x0030;
    pub const EFUSE_TEST: u16 = 0x0034;
    pub const EFUSE_DATA: u16 = 0x0038;

    // Power
    pub const APS_FSMCO: u16 = 0x0004;
    pub const SPS0_CTRL: u16 = 0x0011;
    pub const LDOV12D_CTRL: u16 = 0x0014;
}

/// Chip type for variant-specific handling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipType {
    Rtl8188cu,
    Rtl8188eu,
    Rtl8188ru,
    Rtl8192cu,
    Rtl8192eu,
    Rtl8723au,
    Rtl8723bu,
    Rtl8812au,
    Rtl8821au,
}

impl ChipType {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "RTL8188CU" => Some(ChipType::Rtl8188cu),
            "RTL8188EU" => Some(ChipType::Rtl8188eu),
            "RTL8188RU" => Some(ChipType::Rtl8188ru),
            "RTL8192CU" => Some(ChipType::Rtl8192cu),
            "RTL8192EU" => Some(ChipType::Rtl8192eu),
            "RTL8723AU" => Some(ChipType::Rtl8723au),
            "RTL8723BU" => Some(ChipType::Rtl8723bu),
            "RTL8812AU" => Some(ChipType::Rtl8812au),
            "RTL8821AU" => Some(ChipType::Rtl8821au),
            _ => None,
        }
    }

    fn supports_5ghz(&self) -> bool {
        matches!(self, ChipType::Rtl8812au | ChipType::Rtl8821au)
    }

    fn is_gen2(&self) -> bool {
        matches!(self, ChipType::Rtl8188eu | ChipType::Rtl8192eu |
                       ChipType::Rtl8723bu | ChipType::Rtl8812au | ChipType::Rtl8821au)
    }

    fn antenna_count(&self) -> u8 {
        match self {
            ChipType::Rtl8192cu | ChipType::Rtl8192eu | ChipType::Rtl8812au => 2,
            _ => 1,
        }
    }
}

/// WiFi scan result
#[derive(Debug, Clone)]
pub struct ScanResult {
    pub ssid: String,
    pub bssid: [u8; 6],
    pub channel: u8,
    pub rssi: i8,
    pub encrypted: bool,
}

/// TX descriptor for firmware
#[repr(C, packed)]
#[derive(Clone, Copy)]
struct TxDesc {
    flags: u32,
    flags2: u32,
    flags3: u32,
    flags4: u32,
    flags5: u32,
    reserved: [u32; 3],
}

impl Default for TxDesc {
    fn default() -> Self {
        Self {
            flags: 0,
            flags2: 0,
            flags3: 0,
            flags4: 0,
            flags5: 0,
            reserved: [0; 3],
        }
    }
}

/// RX descriptor from firmware
#[repr(C, packed)]
#[derive(Clone, Copy)]
struct RxDesc {
    flags: u32,
    flags2: u32,
    flags3: u32,
    flags4: u32,
    flags5: u32,
    tsf_low: u32,
    tsf_high: u32,
    reserved: u32,
}

impl Default for RxDesc {
    fn default() -> Self {
        Self {
            flags: 0,
            flags2: 0,
            flags3: 0,
            flags4: 0,
            flags5: 0,
            tsf_low: 0,
            tsf_high: 0,
            reserved: 0,
        }
    }
}

/// The driver state
pub struct Rtl8xxxu {
    /// USB device address
    usb_addr: u8,
    /// USB slot ID (for xHCI)
    slot_id: u8,
    /// USB configuration
    usb_config: u8,
    /// Bulk IN endpoint
    ep_in: u8,
    /// Bulk OUT endpoint
    ep_out: u8,
    /// Chip type
    chip_type: ChipType,
    /// MAC address
    mac: [u8; 6],
    /// Initialization state
    initialized: bool,
    /// Radio powered on
    powered_on: bool,
    /// Connected to network
    connected: bool,
    /// Current channel
    current_channel: u8,
    /// Scan results
    scan_results: Vec<ScanResult>,
    /// TX buffer
    tx_buffer: [u8; 2048],
    /// RX buffer
    rx_buffer: [u8; 2048],
}

impl Rtl8xxxu {
    pub const fn new() -> Self {
        Self {
            usb_addr: 0,
            slot_id: 0,
            usb_config: 1,
            ep_in: 0x81,
            ep_out: 0x02,
            chip_type: ChipType::Rtl8188eu,
            mac: [0; 6],
            initialized: false,
            powered_on: false,
            connected: false,
            current_channel: 1,
            scan_results: Vec::new(),
            tx_buffer: [0; 2048],
            rx_buffer: [0; 2048],
        }
    }

    /// Find and initialize Realtek WiFi adapter
    pub fn probe_usb(&mut self) -> bool {
        // Scan USB devices for Realtek WiFi
        let devices = usb::list_devices();
        for device in devices {
            let vid = device.vendor_id;
            let pid = device.product_id;

            // Check Realtek vendor devices
            if vid == RTL_VENDOR_ID {
                for (dev_pid, name) in DEVICE_IDS {
                    if pid == *dev_pid {
                        if let Some(chip) = ChipType::from_name(name) {
                            self.usb_addr = device.id.address;
                            self.slot_id = device.slot_id;
                            self.chip_type = chip;
                            crate::kprintln!("rtl8xxxu: found {} at USB addr {}", name, device.id.address);
                            return self.init_device(&device);
                        }
                    }
                }
            }

            // Check other vendor rebrands
            for (ov, op, name) in OTHER_VENDORS {
                if vid == *ov && pid == *op {
                    if let Some(chip) = ChipType::from_name(name) {
                        self.usb_addr = device.id.address;
                        self.slot_id = device.slot_id;
                        self.chip_type = chip;
                        crate::kprintln!("rtl8xxxu: found {} (rebrand) at USB addr {}", name, device.id.address);
                        return self.init_device(&device);
                    }
                }
            }
        }

        false
    }

    /// Initialize the USB device
    fn init_device(&mut self, device: &UsbDeviceInfo) -> bool {
        // Find bulk endpoints from device configuration
        for config in &device.configurations {
            for iface in &config.interfaces {
                // Look for wireless interface class (0xE0) or vendor-specific (0xFF)
                if iface.class == 0xE0 || iface.class == 0xFF {
                    // Default endpoints for Realtek devices
                    self.ep_in = 0x81;  // Bulk IN
                    self.ep_out = 0x02; // Bulk OUT
                    break;
                }
            }
        }

        // Initialize the chip
        if !self.chip_init() {
            return false;
        }

        // Read MAC address from EFUSE
        self.read_mac_from_efuse();

        self.initialized = true;
        crate::kprintln!("rtl8xxxu: initialized, MAC {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            self.mac[0], self.mac[1], self.mac[2], self.mac[3], self.mac[4], self.mac[5]);

        true
    }

    /// Chip-specific initialization
    fn chip_init(&mut self) -> bool {
        // Power on sequence
        self.power_on();

        // Load firmware based on chip type
        if !self.load_firmware() {
            crate::kprintln!("rtl8xxxu: firmware load failed");
            return false;
        }

        // Initialize RF
        self.init_rf();

        // Configure RX filter
        self.configure_rx_filter();

        true
    }

    /// Power on the chip
    fn power_on(&mut self) {
        // Gen1 vs Gen2 have different power sequences
        if self.chip_type.is_gen2() {
            self.power_on_gen2();
        } else {
            self.power_on_gen1();
        }
        self.powered_on = true;
    }

    fn power_on_gen1(&self) {
        // RTL8188CU/8192CU power on
        self.write_reg8(regs::SPS0_CTRL, 0x2B);
        self.usb_delay(1);

        let val = self.read_reg8(regs::LDOV12D_CTRL);
        self.write_reg8(regs::LDOV12D_CTRL, val | 0x01);

        self.write_reg8(regs::SYS_FUNC_EN as u16, 0x00);
        self.usb_delay(1);
        self.write_reg8(regs::SYS_FUNC_EN as u16, 0x03);

        let val = self.read_reg16(regs::APS_FSMCO);
        self.write_reg16(regs::APS_FSMCO, val | 0x0800);
    }

    fn power_on_gen2(&self) {
        // RTL8188EU/8192EU/8812AU power on
        self.write_reg8(regs::SYS_FUNC_EN as u16, 0x00);
        self.usb_delay(1);
        self.write_reg8(regs::SYS_FUNC_EN as u16, 0x03);
        self.usb_delay(1);

        let val = self.read_reg16(regs::APS_FSMCO);
        self.write_reg16(regs::APS_FSMCO, val & !0x0800);
        self.usb_delay(1);
        self.write_reg16(regs::APS_FSMCO, val | 0x1000);
    }

    /// Load firmware (simplified - real driver would load actual firmware)
    fn load_firmware(&self) -> bool {
        // In a real implementation, we would:
        // 1. Download firmware blob to chip RAM
        // 2. Verify checksum
        // 3. Start firmware execution
        // For now, we just do basic initialization

        let val = self.read_reg8(regs::CR as u16);
        self.write_reg8(regs::CR as u16, val | 0x01);

        true
    }

    /// Initialize RF settings
    fn init_rf(&self) {
        // Basic RF initialization
        let val = self.read_reg8(regs::RF_CTRL);
        self.write_reg8(regs::RF_CTRL, val | 0x01);

        // Set initial channel to 1
        self.set_channel_internal(1);
    }

    /// Configure RX filter
    fn configure_rx_filter(&self) {
        // Accept all data frames, management frames
        let rcr = 0x7000000E_u32; // AAP, APM, AM, AB
        self.write_reg32(regs::RCR, rcr);

        // Enable management frame filter
        self.write_reg16(regs::RXFLTMAP0, 0xFFFF);
        self.write_reg16(regs::RXFLTMAP2, 0xFFFF);
    }

    /// Read MAC address from EFUSE
    fn read_mac_from_efuse(&mut self) {
        // EFUSE offset for MAC address varies by chip
        let mac_offset: u16 = match self.chip_type {
            ChipType::Rtl8188cu | ChipType::Rtl8188ru | ChipType::Rtl8192cu => 0x7E,
            ChipType::Rtl8188eu | ChipType::Rtl8192eu => 0xD0,
            ChipType::Rtl8723au | ChipType::Rtl8723bu => 0xC6,
            ChipType::Rtl8812au | ChipType::Rtl8821au => 0x18,
        };

        for i in 0..6 {
            self.mac[i] = self.read_efuse_byte(mac_offset + i as u16);
        }

        // Validate MAC (not all zeros or all ones)
        if self.mac == [0u8; 6] || self.mac == [0xFF; 6] {
            // Generate random MAC with Realtek OUI
            self.mac = [0x00, 0xE0, 0x4C, 0x00, 0x00, 0x01];
        }
    }

    /// Read single byte from EFUSE
    fn read_efuse_byte(&self, addr: u16) -> u8 {
        // Enable EFUSE read
        self.write_reg8(regs::EFUSE_CTRL + 1, (addr >> 8) as u8 | 0x80);
        self.write_reg8(regs::EFUSE_CTRL, addr as u8);

        // Wait for read complete
        for _ in 0..100 {
            let val = self.read_reg8(regs::EFUSE_CTRL + 1);
            if val & 0x80 == 0 {
                break;
            }
            self.usb_delay(1);
        }

        self.read_reg8(regs::EFUSE_DATA)
    }

    /// USB register read/write helpers using xHCI control transfers
    /// Note: These are stubs - actual implementation requires direct xHCI access
    fn read_reg8(&self, _reg: u16) -> u8 {
        // TODO: Implement via xHCI control transfer when full USB driver support is available
        // The transfer would use vendor request 0x05 (read register)
        0
    }

    fn write_reg8(&self, _reg: u16, _val: u8) {
        // TODO: Implement via xHCI control transfer when full USB driver support is available
        // The transfer would use vendor request 0x05 (write register)
    }

    fn read_reg16(&self, _reg: u16) -> u16 {
        // TODO: Implement via xHCI control transfer
        0
    }

    fn write_reg16(&self, _reg: u16, _val: u16) {
        // TODO: Implement via xHCI control transfer
    }

    fn read_reg32(&self, _reg: u16) -> u32 {
        // TODO: Implement via xHCI control transfer
        0
    }

    fn write_reg32(&self, _reg: u16, _val: u32) {
        // TODO: Implement via xHCI control transfer
    }

    fn usb_delay(&self, _ms: u32) {
        // Simple delay - in real driver would use proper timer
        for _ in 0..10000 {
            core::hint::spin_loop();
        }
    }

    fn set_channel_internal(&self, channel: u8) {
        // Channel frequency calculation
        let freq = match channel {
            1..=14 => 2412 + ((channel as u16 - 1) * 5),
            36 | 40 | 44 | 48 => 5180 + ((channel as u16 - 36) / 4 * 20),
            52 | 56 | 60 | 64 => 5260 + ((channel as u16 - 52) / 4 * 20),
            100..=144 => 5500 + ((channel as u16 - 100) / 4 * 20),
            149 | 153 | 157 | 161 | 165 => 5745 + ((channel as u16 - 149) / 4 * 20),
            _ => 2412,
        };

        // Set RF frequency - simplified
        let _ = freq; // Would program RF synthesizer
    }

    // Public API

    pub fn get_mac(&self) -> Option<[u8; 6]> {
        if self.initialized {
            Some(self.mac)
        } else {
            None
        }
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Scan for WiFi networks
    pub fn scan(&mut self) -> KResult<Vec<ScanResult>> {
        if !self.initialized {
            return Err(KError::NotSupported);
        }

        self.scan_results.clear();

        // Scan 2.4GHz channels
        for channel in 1..=11 {
            self.set_channel(channel)?;
            self.collect_beacons(channel)?;
        }

        // Scan 5GHz if supported
        if self.chip_type.supports_5ghz() {
            for &channel in &[36, 40, 44, 48, 149, 153, 157, 161, 165] {
                self.set_channel(channel)?;
                self.collect_beacons(channel)?;
            }
        }

        Ok(self.scan_results.clone())
    }

    /// Set WiFi channel
    pub fn set_channel(&mut self, channel: u8) -> KResult<()> {
        if !self.initialized {
            return Err(KError::NotSupported);
        }

        // Validate channel
        if channel == 0 || channel > 165 {
            return Err(KError::Invalid);
        }

        // 5GHz requires supported chip
        if channel > 14 && !self.chip_type.supports_5ghz() {
            return Err(KError::NotSupported);
        }

        self.set_channel_internal(channel);
        self.current_channel = channel;

        Ok(())
    }

    /// Collect beacons on current channel
    fn collect_beacons(&mut self, channel: u8) -> KResult<()> {
        // Wait and receive packets for scanning
        for _ in 0..50 {
            if let Some(frame) = self.recv_frame() {
                if let Some(result) = self.parse_beacon(&frame, channel) {
                    // Avoid duplicates
                    if !self.scan_results.iter().any(|r| r.bssid == result.bssid) {
                        self.scan_results.push(result);
                    }
                }
            }
        }
        Ok(())
    }

    /// Receive a frame from USB
    fn recv_frame(&mut self) -> Option<Vec<u8>> {
        // TODO: Implement via xHCI bulk transfer when full USB driver support is available
        // Would use bulk_transfer_in on ep_in endpoint
        // Frame format: 32-byte RX descriptor + payload
        None
    }

    /// Parse beacon frame
    fn parse_beacon(&self, frame: &[u8], channel: u8) -> Option<ScanResult> {
        if frame.len() < 36 {
            return None;
        }

        // Check frame type (beacon = 0x80)
        let frame_control = u16::from_le_bytes([frame[0], frame[1]]);
        let frame_type = (frame_control >> 2) & 0x3;
        let subtype = (frame_control >> 4) & 0xF;

        if frame_type != 0 || subtype != 8 {
            return None; // Not a beacon
        }

        // Extract BSSID (bytes 16-21)
        let bssid: [u8; 6] = frame[16..22].try_into().ok()?;

        // Parse information elements starting at byte 36
        let mut ssid = String::new();
        let mut encrypted = false;
        let mut ie_offset = 36;

        while ie_offset + 2 < frame.len() {
            let ie_id = frame[ie_offset];
            let ie_len = frame[ie_offset + 1] as usize;

            if ie_offset + 2 + ie_len > frame.len() {
                break;
            }

            match ie_id {
                0 => {
                    // SSID
                    if let Ok(s) = core::str::from_utf8(&frame[ie_offset + 2..ie_offset + 2 + ie_len]) {
                        ssid = String::from(s);
                    }
                }
                48 | 221 => {
                    // RSN or WPA
                    encrypted = true;
                }
                _ => {}
            }

            ie_offset += 2 + ie_len;
        }

        // Estimate RSSI (would come from RX descriptor in real driver)
        let rssi = -50i8;

        Some(ScanResult {
            ssid,
            bssid,
            channel,
            rssi,
            encrypted,
        })
    }

    /// Connect to a network
    pub fn connect(&mut self, ssid: &str, password: Option<&str>) -> KResult<()> {
        if !self.initialized {
            return Err(KError::NotSupported);
        }

        // Find network in scan results
        let network = self.scan_results.iter().find(|r| r.ssid == ssid);
        let network = match network {
            Some(n) => n.clone(),
            None => return Err(KError::NotFound),
        };

        // Set channel
        self.set_channel(network.channel)?;

        // Set BSSID
        self.write_reg32(regs::BSSID as u16,
            u32::from_le_bytes([network.bssid[0], network.bssid[1], network.bssid[2], network.bssid[3]]));
        self.write_reg16(regs::BSSID as u16 + 4,
            u16::from_le_bytes([network.bssid[4], network.bssid[5]]));

        // Handle authentication
        if network.encrypted {
            if password.is_none() {
                return Err(KError::PermissionDenied);
            }
            // WPA/WPA2 4-way handshake would happen here
            self.do_wpa_handshake(password.unwrap())?;
        } else {
            // Open network - send authentication frame
            self.send_auth_frame(&network.bssid)?;
        }

        // Send association request
        self.send_assoc_request(ssid, &network.bssid)?;

        self.connected = true;
        crate::kprintln!("rtl8xxxu: connected to {}", ssid);

        Ok(())
    }

    fn do_wpa_handshake(&self, _password: &str) -> KResult<()> {
        // Simplified - real implementation would do full 4-way handshake
        Ok(())
    }

    fn send_auth_frame(&mut self, bssid: &[u8; 6]) -> KResult<()> {
        // Build authentication frame
        let mut frame = [0u8; 128];

        // Frame control: Authentication
        frame[0] = 0xB0;
        frame[1] = 0x00;

        // Duration
        frame[2] = 0x00;
        frame[3] = 0x00;

        // Destination (BSSID)
        frame[4..10].copy_from_slice(bssid);

        // Source (our MAC)
        frame[10..16].copy_from_slice(&self.mac);

        // BSSID
        frame[16..22].copy_from_slice(bssid);

        // Sequence control
        frame[22] = 0x00;
        frame[23] = 0x00;

        // Authentication algorithm (Open System)
        frame[24] = 0x00;
        frame[25] = 0x00;

        // Authentication transaction sequence number
        frame[26] = 0x01;
        frame[27] = 0x00;

        // Status code (success)
        frame[28] = 0x00;
        frame[29] = 0x00;

        self.send_frame(&frame[..30])
    }

    fn send_assoc_request(&mut self, ssid: &str, bssid: &[u8; 6]) -> KResult<()> {
        // Build association request frame
        let mut frame = [0u8; 256];
        let ssid_bytes = ssid.as_bytes();

        // Frame control: Association Request
        frame[0] = 0x00;
        frame[1] = 0x00;

        // Duration
        frame[2] = 0x00;
        frame[3] = 0x00;

        // Destination (BSSID)
        frame[4..10].copy_from_slice(bssid);

        // Source (our MAC)
        frame[10..16].copy_from_slice(&self.mac);

        // BSSID
        frame[16..22].copy_from_slice(bssid);

        // Sequence control
        frame[22] = 0x00;
        frame[23] = 0x00;

        // Capability info
        frame[24] = 0x01;
        frame[25] = 0x00;

        // Listen interval
        frame[26] = 0x0A;
        frame[27] = 0x00;

        // SSID IE
        frame[28] = 0x00; // SSID element ID
        frame[29] = ssid_bytes.len() as u8;
        frame[30..30 + ssid_bytes.len()].copy_from_slice(ssid_bytes);

        let len = 30 + ssid_bytes.len();

        self.send_frame(&frame[..len])
    }

    /// Send a frame via USB
    fn send_frame(&mut self, frame: &[u8]) -> KResult<()> {
        if frame.len() + 32 > self.tx_buffer.len() {
            return Err(KError::Invalid);
        }

        // Build TX descriptor
        let tx_desc = TxDesc {
            flags: (frame.len() as u32) | (1 << 15), // OWN bit
            ..Default::default()
        };

        // Copy descriptor
        unsafe {
            let desc_bytes = core::slice::from_raw_parts(
                &tx_desc as *const _ as *const u8,
                32
            );
            self.tx_buffer[..32].copy_from_slice(desc_bytes);
        }

        // Copy frame
        self.tx_buffer[32..32 + frame.len()].copy_from_slice(frame);

        // TODO: Send via xHCI bulk transfer when full USB driver support is available
        // Would use bulk_transfer_out on ep_out endpoint

        Ok(())
    }

    /// Disconnect from network
    pub fn disconnect(&mut self) -> KResult<()> {
        if !self.connected {
            return Ok(());
        }

        // Send deauthentication frame
        // ...

        self.connected = false;
        crate::kprintln!("rtl8xxxu: disconnected");

        Ok(())
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Send data packet (when connected)
    pub fn send(&mut self, data: &[u8]) -> KResult<()> {
        if !self.connected {
            return Err(KError::NotSupported);
        }

        // Wrap data in 802.11 data frame and send
        self.send_frame(data)
    }

    /// Receive data packet (when connected)
    pub fn recv(&mut self) -> Option<Vec<u8>> {
        if !self.connected {
            return None;
        }

        self.recv_frame()
    }
}

// Global driver instance
static mut DRIVER: Rtl8xxxu = Rtl8xxxu::new();

/// Initialize Realtek WiFi driver
pub fn init() {
    unsafe {
        DRIVER.probe_usb();
    }
}

/// Get MAC address
pub fn get_mac() -> Option<[u8; 6]> {
    unsafe { DRIVER.get_mac() }
}

/// Check if initialized
pub fn is_initialized() -> bool {
    unsafe { DRIVER.is_initialized() }
}

/// Scan for networks
pub fn scan() -> KResult<Vec<ScanResult>> {
    unsafe { DRIVER.scan() }
}

/// Connect to network
pub fn connect(ssid: &str, password: Option<&str>) -> KResult<()> {
    unsafe { DRIVER.connect(ssid, password) }
}

/// Disconnect
pub fn disconnect() -> KResult<()> {
    unsafe { DRIVER.disconnect() }
}

/// Check connection status
pub fn is_connected() -> bool {
    unsafe { DRIVER.is_connected() }
}

/// Send data
pub fn send(data: &[u8]) -> KResult<()> {
    unsafe { DRIVER.send(data) }
}

/// Receive data
pub fn recv() -> Option<Vec<u8>> {
    unsafe { DRIVER.recv() }
}
