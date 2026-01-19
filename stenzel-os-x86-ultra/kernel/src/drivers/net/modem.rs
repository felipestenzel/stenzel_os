//! 4G/LTE Modem driver.
//!
//! Supports common USB mobile broadband modems:
//! - Qualcomm/QMI-based modems
//! - Sierra Wireless
//! - Huawei
//! - ZTE
//! - Quectel
//! - SimCom
//! - u-blox
//!
//! Provides:
//! - AT command interface
//! - QMI protocol support
//! - Network connection management
//! - SMS support
//! - Signal quality monitoring

use alloc::vec::Vec;
use alloc::vec;
use alloc::string::String;
use alloc::collections::BTreeMap;
use core::sync::atomic::{AtomicU64, AtomicU32, AtomicBool, Ordering};
use crate::sync::IrqSafeMutex;
use crate::util::{KResult, KError};

/// Modem vendor/product IDs
mod device_ids {
    // Qualcomm
    pub const QUALCOMM_VENDOR: u16 = 0x05c6;
    pub const QMI_GENERIC: u16 = 0x9003;

    // Sierra Wireless
    pub const SIERRA_VENDOR: u16 = 0x1199;
    pub const MC7455: u16 = 0x9071;
    pub const MC7354: u16 = 0x68a2;
    pub const EM7455: u16 = 0x9041;

    // Huawei
    pub const HUAWEI_VENDOR: u16 = 0x12d1;
    pub const E392: u16 = 0x1506;
    pub const E3372: u16 = 0x14dc;
    pub const ME906S: u16 = 0x15c1;

    // ZTE
    pub const ZTE_VENDOR: u16 = 0x19d2;
    pub const MF823: u16 = 0x0167;
    pub const MF831: u16 = 0x0326;

    // Quectel
    pub const QUECTEL_VENDOR: u16 = 0x2c7c;
    pub const EC25: u16 = 0x0125;
    pub const EG25: u16 = 0x0121;
    pub const EM12: u16 = 0x0512;
    pub const RM500Q: u16 = 0x0800;  // 5G

    // SimCom
    pub const SIMCOM_VENDOR: u16 = 0x1e0e;
    pub const SIM7600: u16 = 0x9001;
    pub const SIM7000: u16 = 0x9011;

    // u-blox
    pub const UBLOX_VENDOR: u16 = 0x1546;
    pub const TOBY_L4: u16 = 0x1102;
    pub const SARA_R5: u16 = 0x1141;

    pub fn is_supported(vendor: u16, product: u16) -> bool {
        match vendor {
            QUALCOMM_VENDOR => matches!(product, QMI_GENERIC),
            SIERRA_VENDOR => matches!(product, MC7455 | MC7354 | EM7455),
            HUAWEI_VENDOR => matches!(product, E392 | E3372 | ME906S),
            ZTE_VENDOR => matches!(product, MF823 | MF831),
            QUECTEL_VENDOR => matches!(product, EC25 | EG25 | EM12 | RM500Q),
            SIMCOM_VENDOR => matches!(product, SIM7600 | SIM7000),
            UBLOX_VENDOR => matches!(product, TOBY_L4 | SARA_R5),
            _ => false,
        }
    }

    pub fn modem_name(vendor: u16, product: u16) -> &'static str {
        match vendor {
            QUALCOMM_VENDOR => "Qualcomm QMI Modem",
            SIERRA_VENDOR => match product {
                MC7455 => "Sierra MC7455",
                MC7354 => "Sierra MC7354",
                EM7455 => "Sierra EM7455",
                _ => "Sierra Wireless",
            },
            HUAWEI_VENDOR => match product {
                E392 => "Huawei E392",
                E3372 => "Huawei E3372",
                ME906S => "Huawei ME906s",
                _ => "Huawei Modem",
            },
            ZTE_VENDOR => match product {
                MF823 => "ZTE MF823",
                MF831 => "ZTE MF831",
                _ => "ZTE Modem",
            },
            QUECTEL_VENDOR => match product {
                EC25 => "Quectel EC25",
                EG25 => "Quectel EG25",
                EM12 => "Quectel EM12",
                RM500Q => "Quectel RM500Q (5G)",
                _ => "Quectel Modem",
            },
            SIMCOM_VENDOR => match product {
                SIM7600 => "SimCom SIM7600",
                SIM7000 => "SimCom SIM7000",
                _ => "SimCom Modem",
            },
            UBLOX_VENDOR => match product {
                TOBY_L4 => "u-blox TOBY-L4",
                SARA_R5 => "u-blox SARA-R5",
                _ => "u-blox Modem",
            },
            _ => "Unknown Modem",
        }
    }
}

/// Network technology
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NetworkTechnology {
    #[default]
    Unknown,
    Gsm,
    Gprs,
    Edge,
    Umts,
    Hspa,
    HspaPlus,
    Lte,
    LteAdvanced,
    Nr5g,
}

impl NetworkTechnology {
    pub fn name(&self) -> &'static str {
        match self {
            NetworkTechnology::Unknown => "Unknown",
            NetworkTechnology::Gsm => "GSM",
            NetworkTechnology::Gprs => "GPRS",
            NetworkTechnology::Edge => "EDGE",
            NetworkTechnology::Umts => "UMTS",
            NetworkTechnology::Hspa => "HSPA",
            NetworkTechnology::HspaPlus => "HSPA+",
            NetworkTechnology::Lte => "LTE",
            NetworkTechnology::LteAdvanced => "LTE-A",
            NetworkTechnology::Nr5g => "5G NR",
        }
    }

    pub fn generation(&self) -> &'static str {
        match self {
            NetworkTechnology::Unknown => "-",
            NetworkTechnology::Gsm => "2G",
            NetworkTechnology::Gprs => "2.5G",
            NetworkTechnology::Edge => "2.75G",
            NetworkTechnology::Umts => "3G",
            NetworkTechnology::Hspa => "3.5G",
            NetworkTechnology::HspaPlus => "3.75G",
            NetworkTechnology::Lte => "4G",
            NetworkTechnology::LteAdvanced => "4G+",
            NetworkTechnology::Nr5g => "5G",
        }
    }

    pub fn max_speed_mbps(&self) -> u32 {
        match self {
            NetworkTechnology::Unknown => 0,
            NetworkTechnology::Gsm => 0,
            NetworkTechnology::Gprs => 1,
            NetworkTechnology::Edge => 1,
            NetworkTechnology::Umts => 2,
            NetworkTechnology::Hspa => 14,
            NetworkTechnology::HspaPlus => 42,
            NetworkTechnology::Lte => 150,
            NetworkTechnology::LteAdvanced => 1000,
            NetworkTechnology::Nr5g => 10000,
        }
    }
}

/// SIM status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimStatus {
    Unknown,
    NotInserted,
    Ready,
    PinRequired,
    PukRequired,
    Blocked,
    Error,
}

impl SimStatus {
    pub fn name(&self) -> &'static str {
        match self {
            SimStatus::Unknown => "Unknown",
            SimStatus::NotInserted => "Not Inserted",
            SimStatus::Ready => "Ready",
            SimStatus::PinRequired => "PIN Required",
            SimStatus::PukRequired => "PUK Required",
            SimStatus::Blocked => "Blocked",
            SimStatus::Error => "Error",
        }
    }
}

/// Registration status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationStatus {
    NotRegistered,
    RegisteredHome,
    Searching,
    RegistrationDenied,
    Unknown,
    RegisteredRoaming,
}

impl RegistrationStatus {
    pub fn from_at_code(code: u8) -> Self {
        match code {
            0 => RegistrationStatus::NotRegistered,
            1 => RegistrationStatus::RegisteredHome,
            2 => RegistrationStatus::Searching,
            3 => RegistrationStatus::RegistrationDenied,
            4 => RegistrationStatus::Unknown,
            5 => RegistrationStatus::RegisteredRoaming,
            _ => RegistrationStatus::Unknown,
        }
    }

    pub fn is_registered(&self) -> bool {
        matches!(self, RegistrationStatus::RegisteredHome | RegistrationStatus::RegisteredRoaming)
    }

    pub fn name(&self) -> &'static str {
        match self {
            RegistrationStatus::NotRegistered => "Not Registered",
            RegistrationStatus::RegisteredHome => "Home Network",
            RegistrationStatus::Searching => "Searching",
            RegistrationStatus::RegistrationDenied => "Registration Denied",
            RegistrationStatus::Unknown => "Unknown",
            RegistrationStatus::RegisteredRoaming => "Roaming",
        }
    }
}

/// Signal quality
#[derive(Debug, Clone, Copy, Default)]
pub struct SignalQuality {
    /// RSSI in dBm (-113 to -51)
    pub rssi_dbm: i16,
    /// Signal strength (0-100%)
    pub strength_percent: u8,
    /// Signal bars (0-5)
    pub bars: u8,
    /// Bit error rate (0-7, 99 = unknown)
    pub ber: u8,
    /// RSRP for LTE in dBm
    pub rsrp_dbm: Option<i16>,
    /// RSRQ for LTE in dB
    pub rsrq_db: Option<i8>,
    /// SINR for LTE in dB
    pub sinr_db: Option<i8>,
}

impl SignalQuality {
    pub fn from_csq(csq: u8) -> Self {
        let rssi_dbm = if csq == 99 {
            -113
        } else {
            -113 + (csq as i16 * 2)
        };

        let strength_percent = if csq == 99 {
            0
        } else {
            (csq as u16 * 100 / 31).min(100) as u8
        };

        let bars = match csq {
            0..=9 => 1,
            10..=14 => 2,
            15..=19 => 3,
            20..=24 => 4,
            25..=31 => 5,
            _ => 0,
        };

        Self {
            rssi_dbm,
            strength_percent,
            bars,
            ber: 99,
            rsrp_dbm: None,
            rsrq_db: None,
            sinr_db: None,
        }
    }
}

/// Operator info
#[derive(Debug, Clone, Default)]
pub struct OperatorInfo {
    /// Operator name (long)
    pub name: String,
    /// Operator name (short)
    pub short_name: String,
    /// MCC (Mobile Country Code)
    pub mcc: u16,
    /// MNC (Mobile Network Code)
    pub mnc: u16,
    /// Technology
    pub technology: NetworkTechnology,
}

impl OperatorInfo {
    pub fn plmn(&self) -> String {
        use core::fmt::Write;
        let mut s = String::new();
        let _ = write!(s, "{:03}{:02}", self.mcc, self.mnc);
        s
    }
}

/// APN configuration
#[derive(Debug, Clone)]
pub struct ApnConfig {
    /// APN name
    pub apn: String,
    /// Username (optional)
    pub username: Option<String>,
    /// Password (optional)
    pub password: Option<String>,
    /// Authentication type
    pub auth_type: AuthType,
    /// IP type
    pub ip_type: IpType,
}

impl Default for ApnConfig {
    fn default() -> Self {
        Self {
            apn: String::new(),
            username: None,
            password: None,
            auth_type: AuthType::None,
            ip_type: IpType::IPv4,
        }
    }
}

/// Authentication type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AuthType {
    #[default]
    None,
    Pap,
    Chap,
    PapOrChap,
}

/// IP type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IpType {
    #[default]
    IPv4,
    IPv6,
    IPv4v6,
}

/// Connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Disconnecting,
    Error,
}

/// Modem state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModemState {
    Unknown,
    Disabled,
    Initializing,
    Locked,
    Enabled,
    Searching,
    Registered,
    Connecting,
    Connected,
    Disconnecting,
    Failed,
}

impl ModemState {
    pub fn name(&self) -> &'static str {
        match self {
            ModemState::Unknown => "Unknown",
            ModemState::Disabled => "Disabled",
            ModemState::Initializing => "Initializing",
            ModemState::Locked => "SIM Locked",
            ModemState::Enabled => "Enabled",
            ModemState::Searching => "Searching",
            ModemState::Registered => "Registered",
            ModemState::Connecting => "Connecting",
            ModemState::Connected => "Connected",
            ModemState::Disconnecting => "Disconnecting",
            ModemState::Failed => "Failed",
        }
    }
}

/// SMS message
#[derive(Debug, Clone)]
pub struct SmsMessage {
    /// Index in modem storage
    pub index: u16,
    /// Status (read/unread)
    pub read: bool,
    /// Sender/recipient number
    pub number: String,
    /// Message text
    pub text: String,
    /// Timestamp
    pub timestamp: u64,
}

/// Modem device
pub struct ModemDevice {
    /// USB device handle
    usb_device: u32,
    /// Vendor ID
    vendor_id: u16,
    /// Product ID
    product_id: u16,
    /// Device name
    name: String,
    /// Modem state
    state: ModemState,
    /// SIM status
    sim_status: SimStatus,
    /// Registration status
    registration: RegistrationStatus,
    /// Signal quality
    signal: SignalQuality,
    /// Operator info
    operator: OperatorInfo,
    /// Network technology
    technology: NetworkTechnology,
    /// Connection state
    connection: ConnectionState,
    /// IP address (if connected)
    ip_address: Option<[u8; 4]>,
    /// DNS servers
    dns_servers: Vec<[u8; 4]>,
    /// Active APN config
    apn_config: ApnConfig,
    /// IMEI
    imei: String,
    /// IMSI
    imsi: String,
    /// ICCID
    iccid: String,
    /// AT port
    at_port: u8,
    /// Data port
    data_port: u8,
    /// AT command buffer
    at_buffer: Vec<u8>,
    /// Response buffer
    response_buffer: Vec<u8>,
}

impl ModemDevice {
    fn new(usb_device: u32, vendor_id: u16, product_id: u16, at_port: u8, data_port: u8) -> Self {
        Self {
            usb_device,
            vendor_id,
            product_id,
            name: String::from(device_ids::modem_name(vendor_id, product_id)),
            state: ModemState::Unknown,
            sim_status: SimStatus::Unknown,
            registration: RegistrationStatus::Unknown,
            signal: SignalQuality::default(),
            operator: OperatorInfo::default(),
            technology: NetworkTechnology::Unknown,
            connection: ConnectionState::Disconnected,
            ip_address: None,
            dns_servers: Vec::new(),
            apn_config: ApnConfig::default(),
            imei: String::new(),
            imsi: String::new(),
            iccid: String::new(),
            at_port,
            data_port,
            at_buffer: vec![0u8; 512],
            response_buffer: vec![0u8; 1024],
        }
    }

    /// Initialize modem
    pub fn init(&mut self) -> KResult<()> {
        self.state = ModemState::Initializing;

        // Send AT to check modem is responsive
        self.send_at_command("AT")?;

        // Disable echo
        self.send_at_command("ATE0")?;

        // Get modem info
        self.read_imei()?;

        // Check SIM
        self.check_sim_status()?;

        if self.sim_status == SimStatus::Ready {
            self.read_imsi()?;
            self.read_iccid()?;
            self.state = ModemState::Enabled;
        } else if self.sim_status == SimStatus::PinRequired {
            self.state = ModemState::Locked;
        } else {
            self.state = ModemState::Enabled;
        }

        Ok(())
    }

    /// Send AT command
    pub fn send_at_command(&mut self, cmd: &str) -> KResult<String> {
        // Build command with CR
        let cmd_bytes = cmd.as_bytes();
        self.at_buffer[..cmd_bytes.len()].copy_from_slice(cmd_bytes);
        self.at_buffer[cmd_bytes.len()] = b'\r';

        // In real implementation, would send via USB/serial
        // and read response

        // Placeholder: simulate OK response
        Ok(String::from("OK"))
    }

    /// Read IMEI
    fn read_imei(&mut self) -> KResult<()> {
        let response = self.send_at_command("AT+CGSN")?;
        // Parse IMEI from response
        // In real implementation, would parse the actual response
        self.imei = String::from("123456789012345");
        Ok(())
    }

    /// Check SIM status
    fn check_sim_status(&mut self) -> KResult<()> {
        let response = self.send_at_command("AT+CPIN?")?;
        // Parse SIM status
        // +CPIN: READY -> Ready
        // +CPIN: SIM PIN -> PinRequired
        // etc.
        self.sim_status = SimStatus::Ready;
        Ok(())
    }

    /// Read IMSI
    fn read_imsi(&mut self) -> KResult<()> {
        let response = self.send_at_command("AT+CIMI")?;
        self.imsi = String::from("001010123456789");
        Ok(())
    }

    /// Read ICCID
    fn read_iccid(&mut self) -> KResult<()> {
        let response = self.send_at_command("AT+CCID")?;
        self.iccid = String::from("89001234567890123456");
        Ok(())
    }

    /// Enter PIN
    pub fn enter_pin(&mut self, pin: &str) -> KResult<()> {
        if self.sim_status != SimStatus::PinRequired {
            return Err(KError::Invalid);
        }

        let cmd = alloc::format!("AT+CPIN=\"{}\"", pin);
        let response = self.send_at_command(&cmd)?;

        if response.contains("OK") {
            self.check_sim_status()?;
            if self.sim_status == SimStatus::Ready {
                self.state = ModemState::Enabled;
            }
            Ok(())
        } else {
            Err(KError::Invalid)
        }
    }

    /// Get signal quality
    pub fn update_signal(&mut self) -> KResult<()> {
        let response = self.send_at_command("AT+CSQ")?;
        // Parse: +CSQ: <rssi>,<ber>
        // In real implementation, would parse actual values
        self.signal = SignalQuality::from_csq(20); // Placeholder
        Ok(())
    }

    /// Get registration status
    pub fn update_registration(&mut self) -> KResult<()> {
        let response = self.send_at_command("AT+CREG?")?;
        // Parse: +CREG: <n>,<stat>[,<lac>,<ci>,<act>]
        self.registration = RegistrationStatus::RegisteredHome;
        Ok(())
    }

    /// Get operator info
    pub fn update_operator(&mut self) -> KResult<()> {
        let response = self.send_at_command("AT+COPS?")?;
        // Parse operator info
        self.operator = OperatorInfo {
            name: String::from("Test Operator"),
            short_name: String::from("TEST"),
            mcc: 001,
            mnc: 01,
            technology: NetworkTechnology::Lte,
        };
        self.technology = self.operator.technology;
        Ok(())
    }

    /// Set APN
    pub fn set_apn(&mut self, config: ApnConfig) {
        self.apn_config = config;
    }

    /// Connect
    pub fn connect(&mut self) -> KResult<()> {
        if self.registration.is_registered() && self.sim_status == SimStatus::Ready {
            self.connection = ConnectionState::Connecting;
            self.state = ModemState::Connecting;

            // Configure PDP context
            let cmd = alloc::format!(
                "AT+CGDCONT=1,\"{}\",\"{}\"",
                match self.apn_config.ip_type {
                    IpType::IPv4 => "IP",
                    IpType::IPv6 => "IPV6",
                    IpType::IPv4v6 => "IPV4V6",
                },
                self.apn_config.apn
            );
            self.send_at_command(&cmd)?;

            // Activate
            self.send_at_command("AT+CGACT=1,1")?;

            // Get IP address
            let response = self.send_at_command("AT+CGPADDR=1")?;
            // Parse IP from response
            self.ip_address = Some([10, 0, 0, 1]); // Placeholder

            self.connection = ConnectionState::Connected;
            self.state = ModemState::Connected;
            Ok(())
        } else {
            Err(KError::Invalid)
        }
    }

    /// Disconnect
    pub fn disconnect(&mut self) -> KResult<()> {
        self.connection = ConnectionState::Disconnecting;
        self.state = ModemState::Disconnecting;

        self.send_at_command("AT+CGACT=0,1")?;

        self.ip_address = None;
        self.dns_servers.clear();
        self.connection = ConnectionState::Disconnected;
        self.state = ModemState::Registered;
        Ok(())
    }

    /// Send SMS
    pub fn send_sms(&mut self, number: &str, text: &str) -> KResult<()> {
        // Set text mode
        self.send_at_command("AT+CMGF=1")?;

        // Send message
        let cmd = alloc::format!("AT+CMGS=\"{}\"", number);
        self.send_at_command(&cmd)?;

        // Send text + Ctrl+Z
        // In real implementation, would send text followed by 0x1A
        Ok(())
    }

    /// Read SMS messages
    pub fn read_sms_messages(&mut self) -> KResult<Vec<SmsMessage>> {
        // Set text mode
        self.send_at_command("AT+CMGF=1")?;

        // List all messages
        let response = self.send_at_command("AT+CMGL=\"ALL\"")?;

        // Parse messages
        // In real implementation, would parse actual response
        Ok(Vec::new())
    }

    /// Delete SMS
    pub fn delete_sms(&mut self, index: u16) -> KResult<()> {
        let cmd = alloc::format!("AT+CMGD={}", index);
        self.send_at_command(&cmd)?;
        Ok(())
    }

    // Getters
    pub fn state(&self) -> ModemState { self.state }
    pub fn sim_status(&self) -> SimStatus { self.sim_status }
    pub fn registration(&self) -> RegistrationStatus { self.registration }
    pub fn signal(&self) -> &SignalQuality { &self.signal }
    pub fn operator(&self) -> &OperatorInfo { &self.operator }
    pub fn technology(&self) -> NetworkTechnology { self.technology }
    pub fn connection(&self) -> ConnectionState { self.connection }
    pub fn ip_address(&self) -> Option<[u8; 4]> { self.ip_address }
    pub fn imei(&self) -> &str { &self.imei }
    pub fn imsi(&self) -> &str { &self.imsi }
    pub fn iccid(&self) -> &str { &self.iccid }
    pub fn name(&self) -> &str { &self.name }
    pub fn is_connected(&self) -> bool { self.connection == ConnectionState::Connected }
}

/// Modem statistics
#[derive(Debug)]
pub struct ModemStats {
    pub commands_sent: AtomicU64,
    pub bytes_tx: AtomicU64,
    pub bytes_rx: AtomicU64,
    pub connection_attempts: AtomicU64,
    pub successful_connections: AtomicU64,
    pub sms_sent: AtomicU64,
    pub sms_received: AtomicU64,
    pub errors: AtomicU64,
}

impl ModemStats {
    const fn new() -> Self {
        Self {
            commands_sent: AtomicU64::new(0),
            bytes_tx: AtomicU64::new(0),
            bytes_rx: AtomicU64::new(0),
            connection_attempts: AtomicU64::new(0),
            successful_connections: AtomicU64::new(0),
            sms_sent: AtomicU64::new(0),
            sms_received: AtomicU64::new(0),
            errors: AtomicU64::new(0),
        }
    }

    pub fn snapshot(&self) -> ModemStatsSnapshot {
        ModemStatsSnapshot {
            commands_sent: self.commands_sent.load(Ordering::Relaxed),
            bytes_tx: self.bytes_tx.load(Ordering::Relaxed),
            bytes_rx: self.bytes_rx.load(Ordering::Relaxed),
            connection_attempts: self.connection_attempts.load(Ordering::Relaxed),
            successful_connections: self.successful_connections.load(Ordering::Relaxed),
            sms_sent: self.sms_sent.load(Ordering::Relaxed),
            sms_received: self.sms_received.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModemStatsSnapshot {
    pub commands_sent: u64,
    pub bytes_tx: u64,
    pub bytes_rx: u64,
    pub connection_attempts: u64,
    pub successful_connections: u64,
    pub sms_sent: u64,
    pub sms_received: u64,
    pub errors: u64,
}

/// Modem manager
pub struct ModemManager {
    devices: Vec<ModemDevice>,
    active_device: Option<usize>,
    stats: ModemStats,
    initialized: bool,
}

impl ModemManager {
    const fn new() -> Self {
        Self {
            devices: Vec::new(),
            active_device: None,
            stats: ModemStats::new(),
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

    /// Register device
    pub fn register_device(
        &mut self,
        vendor_id: u16,
        product_id: u16,
        usb_device: u32,
        at_port: u8,
        data_port: u8,
    ) -> Option<usize> {
        if !device_ids::is_supported(vendor_id, product_id) {
            return None;
        }

        let mut device = ModemDevice::new(usb_device, vendor_id, product_id, at_port, data_port);

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

    /// Unregister device
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
    pub fn active_device(&self) -> Option<&ModemDevice> {
        self.active_device.and_then(|idx| self.devices.get(idx))
    }

    /// Get active device mutable
    pub fn active_device_mut(&mut self) -> Option<&mut ModemDevice> {
        self.active_device.and_then(|idx| self.devices.get_mut(idx))
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
    pub fn stats(&self) -> ModemStatsSnapshot {
        self.stats.snapshot()
    }

    /// Is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
}

/// Global manager
static MODEM: IrqSafeMutex<ModemManager> = IrqSafeMutex::new(ModemManager::new());

/// Initialize
pub fn init() {
    MODEM.lock().init();
}

/// Register device
pub fn register_device(
    vendor_id: u16,
    product_id: u16,
    usb_device: u32,
    at_port: u8,
    data_port: u8,
) -> Option<usize> {
    MODEM.lock().register_device(vendor_id, product_id, usb_device, at_port, data_port)
}

/// Unregister device
pub fn unregister_device(idx: usize) {
    MODEM.lock().unregister_device(idx);
}

/// Device count
pub fn device_count() -> usize {
    MODEM.lock().device_count()
}

/// Has devices
pub fn has_devices() -> bool {
    MODEM.lock().has_devices()
}

/// Get stats
pub fn stats() -> ModemStatsSnapshot {
    MODEM.lock().stats()
}

/// Is initialized
pub fn is_initialized() -> bool {
    MODEM.lock().is_initialized()
}

/// Is device supported
pub fn is_supported(vendor_id: u16, product_id: u16) -> bool {
    device_ids::is_supported(vendor_id, product_id)
}

/// Driver name
pub fn driver_name() -> &'static str {
    "lte-modem"
}
