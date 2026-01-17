//! MAC Layer Management Entity (MLME)
//!
//! Handles 802.11 state machine and management frame processing.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use crate::util::{KResult, KError};
use super::mac::{MacAddress, MacHeader, FrameControl, FrameType, ManagementSubtype, SequenceGenerator};
use super::frame::{
    Frame, FrameBody, BeaconBody, ProbeRequestBody, ProbeResponseBody,
    AuthenticationBody, DeauthenticationBody, AssociationRequestBody,
    AssociationResponseBody, DisassociationBody, InformationElement,
    ElementId, CapabilityInfo, AuthAlgorithm, StatusCode, ReasonCode,
};
use super::{WifiState, WifiNetwork, WifiChannel, SecurityType, RsnInfo};

/// MLME configuration
#[derive(Debug, Clone)]
pub struct MlmeConfig {
    /// Authentication timeout (milliseconds)
    pub auth_timeout_ms: u64,
    /// Association timeout (milliseconds)
    pub assoc_timeout_ms: u64,
    /// Maximum retries for authentication
    pub max_auth_retries: u8,
    /// Maximum retries for association
    pub max_assoc_retries: u8,
    /// Beacon miss threshold
    pub beacon_miss_threshold: u8,
    /// Scan dwell time per channel (milliseconds)
    pub scan_dwell_time_ms: u64,
}

impl Default for MlmeConfig {
    fn default() -> Self {
        Self {
            auth_timeout_ms: 500,
            assoc_timeout_ms: 500,
            max_auth_retries: 3,
            max_assoc_retries: 3,
            beacon_miss_threshold: 10,
            scan_dwell_time_ms: 100,
        }
    }
}

/// MLME events
#[derive(Debug, Clone)]
pub enum MlmeEvent {
    /// Scan started
    ScanStarted,
    /// Network found during scan
    NetworkFound(WifiNetwork),
    /// Scan completed
    ScanCompleted,
    /// Authentication started
    AuthStarted,
    /// Authentication completed
    AuthCompleted(StatusCode),
    /// Association started
    AssocStarted,
    /// Association completed
    AssocCompleted {
        status: StatusCode,
        association_id: u16,
    },
    /// Deauthenticated
    Deauthenticated(ReasonCode),
    /// Disassociated
    Disassociated(ReasonCode),
    /// Connection lost (beacon miss)
    ConnectionLost,
    /// Data frame received
    DataReceived(Vec<u8>),
}

/// MLME request types
#[derive(Debug, Clone)]
pub enum MlmeRequest {
    /// Start scanning
    Scan {
        ssid: Option<String>,
        channels: Vec<WifiChannel>,
    },
    /// Stop scanning
    ScanStop,
    /// Authenticate with AP
    Authenticate {
        bssid: MacAddress,
        algorithm: AuthAlgorithm,
    },
    /// Associate with AP
    Associate {
        bssid: MacAddress,
        ssid: String,
        capabilities: CapabilityInfo,
    },
    /// Deauthenticate
    Deauthenticate {
        bssid: MacAddress,
        reason: ReasonCode,
    },
    /// Disassociate
    Disassociate {
        bssid: MacAddress,
        reason: ReasonCode,
    },
}

/// MLME state machine
pub struct Mlme {
    /// Current state
    state: MlmeState,
    /// Configuration
    config: MlmeConfig,
    /// Our MAC address
    mac_address: MacAddress,
    /// Current BSSID (if connected)
    bssid: Option<MacAddress>,
    /// Current SSID
    ssid: Option<String>,
    /// Association ID
    association_id: Option<u16>,
    /// Sequence number generator
    sequence: SequenceGenerator,
    /// Authentication state
    auth_state: AuthState,
    /// Retry counters
    auth_retries: u8,
    assoc_retries: u8,
    /// Beacon miss counter
    beacon_miss_count: u8,
    /// Scan results
    scan_results: Vec<WifiNetwork>,
    /// Scan channels remaining
    scan_channels: Vec<WifiChannel>,
    /// Pending events
    events: Vec<MlmeEvent>,
}

/// Internal MLME state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MlmeState {
    Idle,
    Scanning,
    Authenticating,
    Authenticated,
    Associating,
    Associated,
}

/// Authentication state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuthState {
    None,
    Open1Sent,
    Shared1Sent,
    Shared2Sent,
    Shared3Sent,
    Completed,
}

impl Mlme {
    /// Create new MLME instance
    pub fn new(mac_address: MacAddress) -> Self {
        Self {
            state: MlmeState::Idle,
            config: MlmeConfig::default(),
            mac_address,
            bssid: None,
            ssid: None,
            association_id: None,
            sequence: SequenceGenerator::new(),
            auth_state: AuthState::None,
            auth_retries: 0,
            assoc_retries: 0,
            beacon_miss_count: 0,
            scan_results: Vec::new(),
            scan_channels: Vec::new(),
            events: Vec::new(),
        }
    }

    /// Get current state as WifiState
    pub fn wifi_state(&self) -> WifiState {
        match self.state {
            MlmeState::Idle => WifiState::Disconnected,
            MlmeState::Scanning => WifiState::Scanning,
            MlmeState::Authenticating => WifiState::Authenticating,
            MlmeState::Authenticated => WifiState::Authenticating,
            MlmeState::Associating => WifiState::Associating,
            MlmeState::Associated => WifiState::Connected,
        }
    }

    /// Get scan results
    pub fn scan_results(&self) -> &[WifiNetwork] {
        &self.scan_results
    }

    /// Get pending events
    pub fn poll_events(&mut self) -> Vec<MlmeEvent> {
        core::mem::take(&mut self.events)
    }

    /// Process MLME request
    pub fn request(&mut self, req: MlmeRequest) -> KResult<Vec<u8>> {
        match req {
            MlmeRequest::Scan { ssid, channels } => {
                self.start_scan(ssid, channels)
            }
            MlmeRequest::ScanStop => {
                self.stop_scan()
            }
            MlmeRequest::Authenticate { bssid, algorithm } => {
                self.start_auth(bssid, algorithm)
            }
            MlmeRequest::Associate { bssid, ssid, capabilities } => {
                self.start_assoc(bssid, ssid, capabilities)
            }
            MlmeRequest::Deauthenticate { bssid, reason } => {
                self.send_deauth(bssid, reason)
            }
            MlmeRequest::Disassociate { bssid, reason } => {
                self.send_disassoc(bssid, reason)
            }
        }
    }

    /// Process received frame
    pub fn process_frame(&mut self, data: &[u8]) -> KResult<Option<Vec<u8>>> {
        let frame = Frame::parse(data).ok_or(KError::Invalid)?;

        match frame.body {
            FrameBody::Beacon(beacon) => {
                self.process_beacon(&frame.header, beacon)
            }
            FrameBody::ProbeResponse(probe) => {
                self.process_probe_response(&frame.header, probe)
            }
            FrameBody::Authentication(auth) => {
                self.process_auth(&frame.header, auth)
            }
            FrameBody::Deauthentication(deauth) => {
                self.process_deauth(&frame.header, deauth)
            }
            FrameBody::AssociationResponse(assoc) => {
                self.process_assoc_response(&frame.header, assoc)
            }
            FrameBody::Disassociation(disassoc) => {
                self.process_disassoc(&frame.header, disassoc)
            }
            FrameBody::Data(_) | FrameBody::QosData(_) => {
                self.process_data(&frame)
            }
            _ => Ok(None),
        }
    }

    /// Start scanning
    fn start_scan(&mut self, ssid: Option<String>, channels: Vec<WifiChannel>) -> KResult<Vec<u8>> {
        self.state = MlmeState::Scanning;
        self.scan_results.clear();
        self.scan_channels = channels;
        self.events.push(MlmeEvent::ScanStarted);

        // Create probe request
        let rates = [0x82, 0x84, 0x8b, 0x96, 0x0c, 0x12, 0x18, 0x24]; // Basic rates
        let probe = ProbeRequestBody::new(ssid.as_deref(), &rates);

        let frame = self.create_management_frame(
            MacAddress::BROADCAST,
            MacAddress::BROADCAST,
            ManagementSubtype::ProbeRequest as u8,
            probe.to_bytes(),
        );

        Ok(frame)
    }

    /// Stop scanning
    fn stop_scan(&mut self) -> KResult<Vec<u8>> {
        if self.state == MlmeState::Scanning {
            self.state = MlmeState::Idle;
            self.events.push(MlmeEvent::ScanCompleted);
        }
        Ok(Vec::new())
    }

    /// Start authentication
    fn start_auth(&mut self, bssid: MacAddress, algorithm: AuthAlgorithm) -> KResult<Vec<u8>> {
        self.state = MlmeState::Authenticating;
        self.bssid = Some(bssid);
        self.auth_retries = 0;
        self.events.push(MlmeEvent::AuthStarted);

        let auth = match algorithm {
            AuthAlgorithm::OpenSystem => {
                self.auth_state = AuthState::Open1Sent;
                AuthenticationBody::open_auth_request()
            }
            _ => {
                return Err(KError::NotSupported);
            }
        };

        let frame = self.create_management_frame(
            bssid,
            bssid,
            ManagementSubtype::Authentication as u8,
            auth.to_bytes(),
        );

        Ok(frame)
    }

    /// Start association
    fn start_assoc(
        &mut self,
        bssid: MacAddress,
        ssid: String,
        capabilities: CapabilityInfo,
    ) -> KResult<Vec<u8>> {
        if self.state != MlmeState::Authenticated {
            return Err(KError::Invalid);
        }

        self.state = MlmeState::Associating;
        self.ssid = Some(ssid.clone());
        self.assoc_retries = 0;
        self.events.push(MlmeEvent::AssocStarted);

        let mut elements = Vec::new();

        // SSID
        elements.push(InformationElement {
            id: ElementId::Ssid as u8,
            data: ssid.into_bytes(),
        });

        // Supported rates
        elements.push(InformationElement {
            id: ElementId::SupportedRates as u8,
            data: vec![0x82, 0x84, 0x8b, 0x96, 0x0c, 0x12, 0x18, 0x24],
        });

        let assoc = AssociationRequestBody {
            capability: capabilities,
            listen_interval: 10,
            elements,
        };

        let frame = self.create_management_frame(
            bssid,
            bssid,
            ManagementSubtype::AssociationRequest as u8,
            assoc.to_bytes(),
        );

        Ok(frame)
    }

    /// Send deauthentication
    fn send_deauth(&mut self, bssid: MacAddress, reason: ReasonCode) -> KResult<Vec<u8>> {
        let deauth = DeauthenticationBody { reason };

        let frame = self.create_management_frame(
            bssid,
            bssid,
            ManagementSubtype::Deauthentication as u8,
            deauth.to_bytes(),
        );

        self.state = MlmeState::Idle;
        self.bssid = None;
        self.ssid = None;
        self.association_id = None;
        self.auth_state = AuthState::None;

        Ok(frame)
    }

    /// Send disassociation
    fn send_disassoc(&mut self, bssid: MacAddress, reason: ReasonCode) -> KResult<Vec<u8>> {
        let disassoc = DisassociationBody { reason };

        let frame = self.create_management_frame(
            bssid,
            bssid,
            ManagementSubtype::Disassociation as u8,
            disassoc.to_bytes(),
        );

        self.state = MlmeState::Authenticated;
        self.association_id = None;

        Ok(frame)
    }

    /// Process beacon frame
    fn process_beacon(
        &mut self,
        header: &MacHeader,
        beacon: BeaconBody,
    ) -> KResult<Option<Vec<u8>>> {
        // Reset beacon miss counter if from our AP
        if let Some(bssid) = self.bssid {
            if header.addr2 == bssid {
                self.beacon_miss_count = 0;
            }
        }

        // If scanning, add to results
        if self.state == MlmeState::Scanning {
            if let Some(network) = self.beacon_to_network(header, &beacon) {
                // Check if already in results
                let exists = self.scan_results.iter()
                    .any(|n| n.bssid == network.bssid);
                if !exists {
                    self.events.push(MlmeEvent::NetworkFound(network.clone()));
                    self.scan_results.push(network);
                }
            }
        }

        Ok(None)
    }

    /// Process probe response
    fn process_probe_response(
        &mut self,
        header: &MacHeader,
        probe: ProbeResponseBody,
    ) -> KResult<Option<Vec<u8>>> {
        if self.state == MlmeState::Scanning {
            // Convert to beacon body for processing
            let beacon = BeaconBody {
                timestamp: probe.timestamp,
                beacon_interval: probe.beacon_interval,
                capability: probe.capability,
                elements: probe.elements,
            };

            if let Some(network) = self.beacon_to_network(header, &beacon) {
                let exists = self.scan_results.iter()
                    .any(|n| n.bssid == network.bssid);
                if !exists {
                    self.events.push(MlmeEvent::NetworkFound(network.clone()));
                    self.scan_results.push(network);
                }
            }
        }

        Ok(None)
    }

    /// Process authentication frame
    fn process_auth(
        &mut self,
        header: &MacHeader,
        auth: AuthenticationBody,
    ) -> KResult<Option<Vec<u8>>> {
        if self.state != MlmeState::Authenticating {
            return Ok(None);
        }

        // Verify from our target AP
        if let Some(bssid) = self.bssid {
            if header.addr2 != bssid {
                return Ok(None);
            }
        }

        match auth.algorithm {
            AuthAlgorithm::OpenSystem => {
                if auth.sequence == 2 {
                    if auth.status.is_success() {
                        self.state = MlmeState::Authenticated;
                        self.auth_state = AuthState::Completed;
                        self.events.push(MlmeEvent::AuthCompleted(auth.status));
                    } else {
                        self.state = MlmeState::Idle;
                        self.events.push(MlmeEvent::AuthCompleted(auth.status));
                    }
                }
            }
            _ => {
                // Shared key not implemented
            }
        }

        Ok(None)
    }

    /// Process deauthentication frame
    fn process_deauth(
        &mut self,
        header: &MacHeader,
        deauth: DeauthenticationBody,
    ) -> KResult<Option<Vec<u8>>> {
        if let Some(bssid) = self.bssid {
            if header.addr2 == bssid {
                self.state = MlmeState::Idle;
                self.bssid = None;
                self.ssid = None;
                self.association_id = None;
                self.auth_state = AuthState::None;
                self.events.push(MlmeEvent::Deauthenticated(deauth.reason));
            }
        }

        Ok(None)
    }

    /// Process association response
    fn process_assoc_response(
        &mut self,
        header: &MacHeader,
        assoc: AssociationResponseBody,
    ) -> KResult<Option<Vec<u8>>> {
        if self.state != MlmeState::Associating {
            return Ok(None);
        }

        if let Some(bssid) = self.bssid {
            if header.addr2 != bssid {
                return Ok(None);
            }
        }

        if assoc.status.is_success() {
            self.state = MlmeState::Associated;
            self.association_id = Some(assoc.association_id);
            self.events.push(MlmeEvent::AssocCompleted {
                status: assoc.status,
                association_id: assoc.association_id,
            });
        } else {
            self.state = MlmeState::Authenticated;
            self.events.push(MlmeEvent::AssocCompleted {
                status: assoc.status,
                association_id: 0,
            });
        }

        Ok(None)
    }

    /// Process disassociation frame
    fn process_disassoc(
        &mut self,
        header: &MacHeader,
        disassoc: DisassociationBody,
    ) -> KResult<Option<Vec<u8>>> {
        if let Some(bssid) = self.bssid {
            if header.addr2 == bssid {
                if self.state == MlmeState::Associated {
                    self.state = MlmeState::Authenticated;
                    self.association_id = None;
                    self.events.push(MlmeEvent::Disassociated(disassoc.reason));
                }
            }
        }

        Ok(None)
    }

    /// Process data frame
    fn process_data(&mut self, frame: &Frame) -> KResult<Option<Vec<u8>>> {
        if self.state != MlmeState::Associated {
            return Ok(None);
        }

        // Extract payload
        let payload = match &frame.body {
            FrameBody::Data(data) => data.payload.clone(),
            FrameBody::QosData(data) => data.payload.clone(),
            _ => return Ok(None),
        };

        self.events.push(MlmeEvent::DataReceived(payload));
        Ok(None)
    }

    /// Check for beacon miss (call periodically)
    pub fn check_beacon_miss(&mut self) {
        if self.state == MlmeState::Associated {
            self.beacon_miss_count += 1;
            if self.beacon_miss_count >= self.config.beacon_miss_threshold {
                self.state = MlmeState::Idle;
                self.bssid = None;
                self.ssid = None;
                self.association_id = None;
                self.auth_state = AuthState::None;
                self.events.push(MlmeEvent::ConnectionLost);
            }
        }
    }

    /// Convert beacon to WifiNetwork
    fn beacon_to_network(&self, header: &MacHeader, beacon: &BeaconBody) -> Option<WifiNetwork> {
        let ssid = beacon.ssid()?;
        let bssid = header.addr2;
        let channel = beacon.channel().map(|ch| WifiChannel {
            number: ch,
            frequency: channel_to_frequency(ch),
            width: super::ChannelWidth::Mhz20,
        });

        // Determine security type
        let security = self.determine_security(beacon);

        // Parse RSN info if present
        let rsn_info = self.parse_rsn(beacon);

        Some(WifiNetwork {
            ssid,
            bssid,
            channel,
            signal_strength: 0, // Would need RSSI from driver
            security,
            rsn_info,
            wpa_info: None, // WPA1 not commonly used
        })
    }

    /// Determine security type from beacon
    fn determine_security(&self, beacon: &BeaconBody) -> SecurityType {
        // Check for RSN (WPA2/WPA3)
        for elem in &beacon.elements {
            if elem.id == ElementId::Rsn as u8 {
                // Parse RSN IE to determine WPA2 vs WPA3
                if let Some(rsn) = parse_rsn_ie(&elem.data) {
                    for akm in &rsn.akm_suites {
                        if *akm == super::AkmSuite::Sae {
                            return SecurityType::Wpa3Sae;
                        }
                    }
                    for akm in &rsn.akm_suites {
                        if *akm == super::AkmSuite::Psk {
                            return SecurityType::Wpa2Psk;
                        }
                    }
                }
            }
        }

        // Check for WPA1 (vendor specific)
        for elem in &beacon.elements {
            if elem.id == ElementId::VendorSpecific as u8 && elem.data.len() >= 4 {
                // Microsoft OUI for WPA
                if elem.data[0] == 0x00 && elem.data[1] == 0x50 &&
                   elem.data[2] == 0xf2 && elem.data[3] == 0x01 {
                    return SecurityType::WpaPsk;
                }
            }
        }

        // Check privacy bit for WEP
        if beacon.capability.privacy() {
            return SecurityType::Wep;
        }

        SecurityType::Open
    }

    /// Parse RSN information element
    fn parse_rsn(&self, beacon: &BeaconBody) -> Option<RsnInfo> {
        for elem in &beacon.elements {
            if elem.id == ElementId::Rsn as u8 {
                return parse_rsn_ie(&elem.data);
            }
        }
        None
    }

    /// Create management frame
    fn create_management_frame(
        &mut self,
        addr1: MacAddress,
        addr3: MacAddress,
        subtype: u8,
        body: Vec<u8>,
    ) -> Vec<u8> {
        let fc = FrameControl::new(FrameType::Management, subtype, false, false);

        let header = MacHeader {
            frame_control: fc,
            duration_id: 0,
            addr1,
            addr2: self.mac_address,
            addr3,
            sequence_control: self.sequence.sequence_control(0),
            addr4: None,
        };

        let mut frame = header.to_bytes();
        frame.extend(body);
        frame
    }
}

/// Parse RSN Information Element
fn parse_rsn_ie(data: &[u8]) -> Option<RsnInfo> {
    if data.len() < 8 {
        return None;
    }

    let version = u16::from_le_bytes([data[0], data[1]]);
    if version != 1 {
        return None;
    }

    let group_cipher = parse_cipher_suite(&data[2..6])?;

    let mut pos = 6;

    // Pairwise cipher suites
    if pos + 2 > data.len() {
        return None;
    }
    let pairwise_count = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
    pos += 2;

    let mut pairwise_ciphers = Vec::new();
    for _ in 0..pairwise_count {
        if pos + 4 > data.len() {
            break;
        }
        if let Some(cipher) = parse_cipher_suite(&data[pos..pos + 4]) {
            pairwise_ciphers.push(cipher);
        }
        pos += 4;
    }

    // AKM suites
    if pos + 2 > data.len() {
        return Some(RsnInfo {
            version,
            group_cipher,
            pairwise_ciphers,
            akm_suites: Vec::new(),
            capabilities: 0,
        });
    }
    let akm_count = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
    pos += 2;

    let mut akm_suites = Vec::new();
    for _ in 0..akm_count {
        if pos + 4 > data.len() {
            break;
        }
        if let Some(akm) = parse_akm_suite(&data[pos..pos + 4]) {
            akm_suites.push(akm);
        }
        pos += 4;
    }

    // RSN capabilities
    let capabilities = if pos + 2 <= data.len() {
        u16::from_le_bytes([data[pos], data[pos + 1]])
    } else {
        0
    };

    Some(RsnInfo {
        version,
        group_cipher,
        pairwise_ciphers,
        akm_suites,
        capabilities,
    })
}

/// Parse cipher suite
fn parse_cipher_suite(data: &[u8]) -> Option<super::CipherSuite> {
    if data.len() < 4 {
        return None;
    }

    // IEEE OUI
    if data[0] == 0x00 && data[1] == 0x0f && data[2] == 0xac {
        match data[3] {
            0 => Some(super::CipherSuite::None),
            1 => Some(super::CipherSuite::Wep40),
            2 => Some(super::CipherSuite::Tkip),
            4 => Some(super::CipherSuite::Ccmp),
            5 => Some(super::CipherSuite::Wep104),
            8 => Some(super::CipherSuite::Gcmp),
            _ => None,
        }
    } else {
        None
    }
}

/// Parse AKM suite
fn parse_akm_suite(data: &[u8]) -> Option<super::AkmSuite> {
    if data.len() < 4 {
        return None;
    }

    // IEEE OUI
    if data[0] == 0x00 && data[1] == 0x0f && data[2] == 0xac {
        match data[3] {
            1 => Some(super::AkmSuite::Eap),
            2 => Some(super::AkmSuite::Psk),
            8 => Some(super::AkmSuite::Sae),
            18 => Some(super::AkmSuite::Owe),
            _ => None,
        }
    } else {
        None
    }
}

/// Convert channel number to frequency (MHz)
fn channel_to_frequency(channel: u8) -> u32 {
    if channel >= 1 && channel <= 13 {
        // 2.4 GHz
        2407 + (channel as u32) * 5
    } else if channel == 14 {
        // 2.4 GHz Japan
        2484
    } else if channel >= 36 && channel <= 177 {
        // 5 GHz
        5000 + (channel as u32) * 5
    } else {
        0
    }
}

/// Convert frequency to channel number
pub fn frequency_to_channel(freq: u32) -> Option<u8> {
    if freq >= 2412 && freq <= 2472 {
        Some(((freq - 2407) / 5) as u8)
    } else if freq == 2484 {
        Some(14)
    } else if freq >= 5180 && freq <= 5885 {
        Some(((freq - 5000) / 5) as u8)
    } else {
        None
    }
}
