//! IEEE 802.11 Frame Structures
//!
//! Frame parsing and generation for WiFi management and data frames.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use super::mac::{MacAddress, MacHeader, FrameControl, FrameType, ManagementSubtype};

/// Minimum frame size (header + FCS)
pub const MIN_FRAME_SIZE: usize = 24 + 4;

/// Maximum frame size (802.11n/ac)
pub const MAX_FRAME_SIZE: usize = 7991;

/// FCS (Frame Check Sequence) size - CRC32
pub const FCS_SIZE: usize = 4;

/// Parsed 802.11 frame
#[derive(Debug, Clone)]
pub struct Frame {
    /// MAC header
    pub header: MacHeader,
    /// Frame body
    pub body: FrameBody,
    /// FCS valid (if verified)
    pub fcs_valid: Option<bool>,
}

impl Frame {
    /// Parse frame from bytes
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < MIN_FRAME_SIZE {
            return None;
        }

        let header = MacHeader::parse(data)?;
        let body_start = header.size();

        // FCS is last 4 bytes
        let body_end = data.len() - FCS_SIZE;
        if body_end <= body_start {
            return None;
        }

        let body_data = &data[body_start..body_end];
        let body = FrameBody::parse(&header.frame_control, body_data)?;

        Some(Frame {
            header,
            body,
            fcs_valid: None,
        })
    }

    /// Serialize to bytes (without FCS)
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = self.header.to_bytes();
        bytes.extend(self.body.to_bytes());
        bytes
    }

    /// Calculate and append FCS
    pub fn to_bytes_with_fcs(&self) -> Vec<u8> {
        let mut bytes = self.to_bytes();
        let fcs = calculate_fcs(&bytes);
        bytes.extend_from_slice(&fcs.to_le_bytes());
        bytes
    }

    /// Verify FCS
    pub fn verify_fcs(&mut self, data: &[u8]) -> bool {
        if data.len() < FCS_SIZE {
            self.fcs_valid = Some(false);
            return false;
        }

        let frame_data = &data[..data.len() - FCS_SIZE];
        let expected_fcs = calculate_fcs(frame_data);
        let actual_fcs = u32::from_le_bytes([
            data[data.len() - 4],
            data[data.len() - 3],
            data[data.len() - 2],
            data[data.len() - 1],
        ]);

        let valid = expected_fcs == actual_fcs;
        self.fcs_valid = Some(valid);
        valid
    }
}

/// Frame body types
#[derive(Debug, Clone)]
pub enum FrameBody {
    /// Beacon frame
    Beacon(BeaconBody),
    /// Probe request
    ProbeRequest(ProbeRequestBody),
    /// Probe response
    ProbeResponse(ProbeResponseBody),
    /// Authentication frame
    Authentication(AuthenticationBody),
    /// Deauthentication frame
    Deauthentication(DeauthenticationBody),
    /// Association request
    AssociationRequest(AssociationRequestBody),
    /// Association response
    AssociationResponse(AssociationResponseBody),
    /// Disassociation frame
    Disassociation(DisassociationBody),
    /// Data frame
    Data(DataBody),
    /// QoS data frame
    QosData(QosDataBody),
    /// Action frame
    Action(ActionBody),
    /// Unknown/unsupported frame
    Unknown(Vec<u8>),
}

impl FrameBody {
    /// Parse body based on frame control
    pub fn parse(fc: &FrameControl, data: &[u8]) -> Option<Self> {
        match fc.frame_type() {
            FrameType::Management => {
                Self::parse_management(fc.subtype(), data)
            }
            FrameType::Data => {
                Self::parse_data(fc.subtype(), data)
            }
            FrameType::Control => {
                Some(FrameBody::Unknown(data.to_vec()))
            }
            FrameType::Reserved => {
                Some(FrameBody::Unknown(data.to_vec()))
            }
        }
    }

    fn parse_management(subtype: u8, data: &[u8]) -> Option<Self> {
        match subtype {
            0 => AssociationRequestBody::parse(data).map(FrameBody::AssociationRequest),
            1 => AssociationResponseBody::parse(data).map(FrameBody::AssociationResponse),
            4 => ProbeRequestBody::parse(data).map(FrameBody::ProbeRequest),
            5 => ProbeResponseBody::parse(data).map(FrameBody::ProbeResponse),
            8 => BeaconBody::parse(data).map(FrameBody::Beacon),
            10 => DisassociationBody::parse(data).map(FrameBody::Disassociation),
            11 => AuthenticationBody::parse(data).map(FrameBody::Authentication),
            12 => DeauthenticationBody::parse(data).map(FrameBody::Deauthentication),
            13 => ActionBody::parse(data).map(FrameBody::Action),
            _ => Some(FrameBody::Unknown(data.to_vec())),
        }
    }

    fn parse_data(subtype: u8, data: &[u8]) -> Option<Self> {
        if subtype & 0x08 != 0 {
            // QoS data frame
            QosDataBody::parse(data).map(FrameBody::QosData)
        } else {
            DataBody::parse(data).map(FrameBody::Data)
        }
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            FrameBody::Beacon(b) => b.to_bytes(),
            FrameBody::ProbeRequest(b) => b.to_bytes(),
            FrameBody::ProbeResponse(b) => b.to_bytes(),
            FrameBody::Authentication(b) => b.to_bytes(),
            FrameBody::Deauthentication(b) => b.to_bytes(),
            FrameBody::AssociationRequest(b) => b.to_bytes(),
            FrameBody::AssociationResponse(b) => b.to_bytes(),
            FrameBody::Disassociation(b) => b.to_bytes(),
            FrameBody::Data(b) => b.to_bytes(),
            FrameBody::QosData(b) => b.to_bytes(),
            FrameBody::Action(b) => b.to_bytes(),
            FrameBody::Unknown(data) => data.clone(),
        }
    }
}

/// Beacon frame body
#[derive(Debug, Clone)]
pub struct BeaconBody {
    /// Timestamp (microseconds)
    pub timestamp: u64,
    /// Beacon interval (TUs)
    pub beacon_interval: u16,
    /// Capability information
    pub capability: CapabilityInfo,
    /// Information elements
    pub elements: Vec<InformationElement>,
}

impl BeaconBody {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 12 {
            return None;
        }

        let timestamp = u64::from_le_bytes([
            data[0], data[1], data[2], data[3],
            data[4], data[5], data[6], data[7],
        ]);
        let beacon_interval = u16::from_le_bytes([data[8], data[9]]);
        let capability = CapabilityInfo::from_raw(u16::from_le_bytes([data[10], data[11]]));
        let elements = parse_information_elements(&data[12..]);

        Some(BeaconBody {
            timestamp,
            beacon_interval,
            capability,
            elements,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.timestamp.to_le_bytes());
        bytes.extend_from_slice(&self.beacon_interval.to_le_bytes());
        bytes.extend_from_slice(&self.capability.to_raw().to_le_bytes());
        for elem in &self.elements {
            bytes.extend(elem.to_bytes());
        }
        bytes
    }

    /// Get SSID from elements
    pub fn ssid(&self) -> Option<String> {
        for elem in &self.elements {
            if elem.id == ElementId::Ssid as u8 {
                if elem.data.is_empty() {
                    return Some(String::new()); // Hidden SSID
                }
                return String::from_utf8(elem.data.clone()).ok();
            }
        }
        None
    }

    /// Get supported rates
    pub fn supported_rates(&self) -> Vec<u8> {
        let mut rates = Vec::new();
        for elem in &self.elements {
            if elem.id == ElementId::SupportedRates as u8 ||
               elem.id == ElementId::ExtendedSupportedRates as u8 {
                rates.extend_from_slice(&elem.data);
            }
        }
        rates
    }

    /// Get channel from DS Parameter Set
    pub fn channel(&self) -> Option<u8> {
        for elem in &self.elements {
            if elem.id == ElementId::DsParameterSet as u8 && elem.data.len() >= 1 {
                return Some(elem.data[0]);
            }
        }
        None
    }
}

/// Probe request body
#[derive(Debug, Clone)]
pub struct ProbeRequestBody {
    pub elements: Vec<InformationElement>,
}

impl ProbeRequestBody {
    pub fn parse(data: &[u8]) -> Option<Self> {
        Some(ProbeRequestBody {
            elements: parse_information_elements(data),
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        for elem in &self.elements {
            bytes.extend(elem.to_bytes());
        }
        bytes
    }

    /// Create probe request for specific SSID
    pub fn new(ssid: Option<&str>, rates: &[u8]) -> Self {
        let mut elements = Vec::new();

        // SSID element
        let ssid_data = ssid.map(|s| s.as_bytes().to_vec()).unwrap_or_default();
        elements.push(InformationElement {
            id: ElementId::Ssid as u8,
            data: ssid_data,
        });

        // Supported rates
        elements.push(InformationElement {
            id: ElementId::SupportedRates as u8,
            data: rates.to_vec(),
        });

        ProbeRequestBody { elements }
    }
}

/// Probe response body (same structure as beacon)
#[derive(Debug, Clone)]
pub struct ProbeResponseBody {
    pub timestamp: u64,
    pub beacon_interval: u16,
    pub capability: CapabilityInfo,
    pub elements: Vec<InformationElement>,
}

impl ProbeResponseBody {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 12 {
            return None;
        }

        let timestamp = u64::from_le_bytes([
            data[0], data[1], data[2], data[3],
            data[4], data[5], data[6], data[7],
        ]);
        let beacon_interval = u16::from_le_bytes([data[8], data[9]]);
        let capability = CapabilityInfo::from_raw(u16::from_le_bytes([data[10], data[11]]));
        let elements = parse_information_elements(&data[12..]);

        Some(ProbeResponseBody {
            timestamp,
            beacon_interval,
            capability,
            elements,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.timestamp.to_le_bytes());
        bytes.extend_from_slice(&self.beacon_interval.to_le_bytes());
        bytes.extend_from_slice(&self.capability.to_raw().to_le_bytes());
        for elem in &self.elements {
            bytes.extend(elem.to_bytes());
        }
        bytes
    }
}

/// Authentication frame body
#[derive(Debug, Clone)]
pub struct AuthenticationBody {
    /// Authentication algorithm
    pub algorithm: AuthAlgorithm,
    /// Authentication sequence number
    pub sequence: u16,
    /// Status code
    pub status: StatusCode,
    /// Challenge text (for shared key auth)
    pub challenge: Option<Vec<u8>>,
}

impl AuthenticationBody {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 6 {
            return None;
        }

        let algorithm = AuthAlgorithm::from_raw(u16::from_le_bytes([data[0], data[1]]));
        let sequence = u16::from_le_bytes([data[2], data[3]]);
        let status = StatusCode::from_raw(u16::from_le_bytes([data[4], data[5]]));

        let challenge = if data.len() > 6 {
            let elements = parse_information_elements(&data[6..]);
            elements.into_iter()
                .find(|e| e.id == ElementId::ChallengeText as u8)
                .map(|e| e.data)
        } else {
            None
        };

        Some(AuthenticationBody {
            algorithm,
            sequence,
            status,
            challenge,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.algorithm.to_raw().to_le_bytes());
        bytes.extend_from_slice(&self.sequence.to_le_bytes());
        bytes.extend_from_slice(&self.status.to_raw().to_le_bytes());
        if let Some(ref challenge) = self.challenge {
            bytes.push(ElementId::ChallengeText as u8);
            bytes.push(challenge.len() as u8);
            bytes.extend_from_slice(challenge);
        }
        bytes
    }

    /// Create open system authentication request
    pub fn open_auth_request() -> Self {
        AuthenticationBody {
            algorithm: AuthAlgorithm::OpenSystem,
            sequence: 1,
            status: StatusCode::Success,
            challenge: None,
        }
    }
}

/// Deauthentication frame body
#[derive(Debug, Clone)]
pub struct DeauthenticationBody {
    pub reason: ReasonCode,
}

impl DeauthenticationBody {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 2 {
            return None;
        }
        let reason = ReasonCode::from_raw(u16::from_le_bytes([data[0], data[1]]));
        Some(DeauthenticationBody { reason })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.reason.to_raw().to_le_bytes().to_vec()
    }
}

/// Association request body
#[derive(Debug, Clone)]
pub struct AssociationRequestBody {
    pub capability: CapabilityInfo,
    pub listen_interval: u16,
    pub elements: Vec<InformationElement>,
}

impl AssociationRequestBody {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }

        let capability = CapabilityInfo::from_raw(u16::from_le_bytes([data[0], data[1]]));
        let listen_interval = u16::from_le_bytes([data[2], data[3]]);
        let elements = parse_information_elements(&data[4..]);

        Some(AssociationRequestBody {
            capability,
            listen_interval,
            elements,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.capability.to_raw().to_le_bytes());
        bytes.extend_from_slice(&self.listen_interval.to_le_bytes());
        for elem in &self.elements {
            bytes.extend(elem.to_bytes());
        }
        bytes
    }
}

/// Association response body
#[derive(Debug, Clone)]
pub struct AssociationResponseBody {
    pub capability: CapabilityInfo,
    pub status: StatusCode,
    pub association_id: u16,
    pub elements: Vec<InformationElement>,
}

impl AssociationResponseBody {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 6 {
            return None;
        }

        let capability = CapabilityInfo::from_raw(u16::from_le_bytes([data[0], data[1]]));
        let status = StatusCode::from_raw(u16::from_le_bytes([data[2], data[3]]));
        let association_id = u16::from_le_bytes([data[4], data[5]]) & 0x3FFF;
        let elements = parse_information_elements(&data[6..]);

        Some(AssociationResponseBody {
            capability,
            status,
            association_id,
            elements,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.capability.to_raw().to_le_bytes());
        bytes.extend_from_slice(&self.status.to_raw().to_le_bytes());
        bytes.extend_from_slice(&(self.association_id | 0xC000).to_le_bytes());
        for elem in &self.elements {
            bytes.extend(elem.to_bytes());
        }
        bytes
    }
}

/// Disassociation frame body
#[derive(Debug, Clone)]
pub struct DisassociationBody {
    pub reason: ReasonCode,
}

impl DisassociationBody {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 2 {
            return None;
        }
        let reason = ReasonCode::from_raw(u16::from_le_bytes([data[0], data[1]]));
        Some(DisassociationBody { reason })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.reason.to_raw().to_le_bytes().to_vec()
    }
}

/// Data frame body
#[derive(Debug, Clone)]
pub struct DataBody {
    pub llc_header: Option<LlcHeader>,
    pub payload: Vec<u8>,
}

impl DataBody {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 8 {
            return Some(DataBody {
                llc_header: None,
                payload: data.to_vec(),
            });
        }

        let llc_header = LlcHeader::parse(data);
        let payload = if llc_header.is_some() {
            data[8..].to_vec()
        } else {
            data.to_vec()
        };

        Some(DataBody { llc_header, payload })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        if let Some(ref llc) = self.llc_header {
            bytes.extend(llc.to_bytes());
        }
        bytes.extend_from_slice(&self.payload);
        bytes
    }
}

/// QoS data frame body
#[derive(Debug, Clone)]
pub struct QosDataBody {
    pub qos_control: u16,
    pub llc_header: Option<LlcHeader>,
    pub payload: Vec<u8>,
}

impl QosDataBody {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 2 {
            return None;
        }

        let qos_control = u16::from_le_bytes([data[0], data[1]]);
        let remaining = &data[2..];

        if remaining.len() < 8 {
            return Some(QosDataBody {
                qos_control,
                llc_header: None,
                payload: remaining.to_vec(),
            });
        }

        let llc_header = LlcHeader::parse(remaining);
        let payload = if llc_header.is_some() {
            remaining[8..].to_vec()
        } else {
            remaining.to_vec()
        };

        Some(QosDataBody {
            qos_control,
            llc_header,
            payload,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.qos_control.to_le_bytes());
        if let Some(ref llc) = self.llc_header {
            bytes.extend(llc.to_bytes());
        }
        bytes.extend_from_slice(&self.payload);
        bytes
    }

    /// Get TID (Traffic Identifier) from QoS control
    pub fn tid(&self) -> u8 {
        (self.qos_control & 0x0F) as u8
    }
}

/// Action frame body
#[derive(Debug, Clone)]
pub struct ActionBody {
    pub category: u8,
    pub action: u8,
    pub data: Vec<u8>,
}

impl ActionBody {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 2 {
            return None;
        }

        Some(ActionBody {
            category: data[0],
            action: data[1],
            data: data[2..].to_vec(),
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(2 + self.data.len());
        bytes.push(self.category);
        bytes.push(self.action);
        bytes.extend_from_slice(&self.data);
        bytes
    }
}

/// LLC/SNAP header
#[derive(Debug, Clone)]
pub struct LlcHeader {
    pub dsap: u8,
    pub ssap: u8,
    pub control: u8,
    pub oui: [u8; 3],
    pub ether_type: u16,
}

impl LlcHeader {
    /// Standard SNAP header
    pub const SNAP: LlcHeader = LlcHeader {
        dsap: 0xAA,
        ssap: 0xAA,
        control: 0x03,
        oui: [0, 0, 0],
        ether_type: 0,
    };

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 8 {
            return None;
        }

        let dsap = data[0];
        let ssap = data[1];
        let control = data[2];

        // Check for SNAP
        if dsap != 0xAA || ssap != 0xAA || control != 0x03 {
            return None;
        }

        let oui = [data[3], data[4], data[5]];
        let ether_type = u16::from_be_bytes([data[6], data[7]]);

        Some(LlcHeader {
            dsap,
            ssap,
            control,
            oui,
            ether_type,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        vec![
            self.dsap,
            self.ssap,
            self.control,
            self.oui[0],
            self.oui[1],
            self.oui[2],
            (self.ether_type >> 8) as u8,
            (self.ether_type & 0xFF) as u8,
        ]
    }

    /// Create SNAP header for Ethernet type
    pub fn new_snap(ether_type: u16) -> Self {
        LlcHeader {
            dsap: 0xAA,
            ssap: 0xAA,
            control: 0x03,
            oui: [0, 0, 0],
            ether_type,
        }
    }
}

/// Information Element
#[derive(Debug, Clone)]
pub struct InformationElement {
    pub id: u8,
    pub data: Vec<u8>,
}

impl InformationElement {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(2 + self.data.len());
        bytes.push(self.id);
        bytes.push(self.data.len() as u8);
        bytes.extend_from_slice(&self.data);
        bytes
    }
}

/// Element IDs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ElementId {
    Ssid = 0,
    SupportedRates = 1,
    FhParameterSet = 2,
    DsParameterSet = 3,
    CfParameterSet = 4,
    Tim = 5,
    IbssParameterSet = 6,
    Country = 7,
    ChallengeText = 16,
    PowerConstraint = 32,
    PowerCapability = 33,
    TpcRequest = 34,
    TpcReport = 35,
    SupportedChannels = 36,
    ChannelSwitchAnnouncement = 37,
    QuietElement = 40,
    IbssDfs = 41,
    ErpInformation = 42,
    HtCapabilities = 45,
    Rsn = 48,
    ExtendedSupportedRates = 50,
    HtOperation = 61,
    VhtCapabilities = 191,
    VhtOperation = 192,
    VendorSpecific = 221,
}

/// Parse information elements from data
pub fn parse_information_elements(data: &[u8]) -> Vec<InformationElement> {
    let mut elements = Vec::new();
    let mut pos = 0;

    while pos + 2 <= data.len() {
        let id = data[pos];
        let len = data[pos + 1] as usize;
        pos += 2;

        if pos + len > data.len() {
            break;
        }

        elements.push(InformationElement {
            id,
            data: data[pos..pos + len].to_vec(),
        });

        pos += len;
    }

    elements
}

/// Capability Information field
#[derive(Debug, Clone, Copy, Default)]
pub struct CapabilityInfo(u16);

impl CapabilityInfo {
    pub fn from_raw(raw: u16) -> Self {
        CapabilityInfo(raw)
    }

    pub fn to_raw(self) -> u16 {
        self.0
    }

    pub fn ess(self) -> bool { self.0 & (1 << 0) != 0 }
    pub fn ibss(self) -> bool { self.0 & (1 << 1) != 0 }
    pub fn cf_pollable(self) -> bool { self.0 & (1 << 2) != 0 }
    pub fn cf_poll_request(self) -> bool { self.0 & (1 << 3) != 0 }
    pub fn privacy(self) -> bool { self.0 & (1 << 4) != 0 }
    pub fn short_preamble(self) -> bool { self.0 & (1 << 5) != 0 }
    pub fn spectrum_management(self) -> bool { self.0 & (1 << 8) != 0 }
    pub fn qos(self) -> bool { self.0 & (1 << 9) != 0 }
    pub fn short_slot_time(self) -> bool { self.0 & (1 << 10) != 0 }
    pub fn apsd(self) -> bool { self.0 & (1 << 11) != 0 }
    pub fn radio_measurement(self) -> bool { self.0 & (1 << 12) != 0 }
    pub fn delayed_block_ack(self) -> bool { self.0 & (1 << 14) != 0 }
    pub fn immediate_block_ack(self) -> bool { self.0 & (1 << 15) != 0 }

    pub fn new() -> Self {
        CapabilityInfo(0)
    }

    pub fn set_ess(&mut self, val: bool) {
        if val { self.0 |= 1 << 0; } else { self.0 &= !(1 << 0); }
    }

    pub fn set_privacy(&mut self, val: bool) {
        if val { self.0 |= 1 << 4; } else { self.0 &= !(1 << 4); }
    }

    pub fn set_short_preamble(&mut self, val: bool) {
        if val { self.0 |= 1 << 5; } else { self.0 &= !(1 << 5); }
    }

    pub fn set_short_slot_time(&mut self, val: bool) {
        if val { self.0 |= 1 << 10; } else { self.0 &= !(1 << 10); }
    }
}

/// Authentication algorithms
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthAlgorithm {
    OpenSystem,
    SharedKey,
    FastBssTransition,
    Sae,
    Unknown(u16),
}

impl AuthAlgorithm {
    pub fn from_raw(raw: u16) -> Self {
        match raw {
            0 => AuthAlgorithm::OpenSystem,
            1 => AuthAlgorithm::SharedKey,
            2 => AuthAlgorithm::FastBssTransition,
            3 => AuthAlgorithm::Sae,
            _ => AuthAlgorithm::Unknown(raw),
        }
    }

    pub fn to_raw(self) -> u16 {
        match self {
            AuthAlgorithm::OpenSystem => 0,
            AuthAlgorithm::SharedKey => 1,
            AuthAlgorithm::FastBssTransition => 2,
            AuthAlgorithm::Sae => 3,
            AuthAlgorithm::Unknown(v) => v,
        }
    }
}

/// Status codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusCode {
    Success,
    UnspecifiedFailure,
    CapabilitiesNotSupported,
    ReassociationDenied,
    AssociationDeniedOutsideStandard,
    AuthAlgorithmNotSupported,
    AuthSequenceOutOfOrder,
    ChallengeFailure,
    AuthTimeout,
    ApFull,
    RateNotSupported,
    Unknown(u16),
}

impl StatusCode {
    pub fn from_raw(raw: u16) -> Self {
        match raw {
            0 => StatusCode::Success,
            1 => StatusCode::UnspecifiedFailure,
            10 => StatusCode::CapabilitiesNotSupported,
            11 => StatusCode::ReassociationDenied,
            12 => StatusCode::AssociationDeniedOutsideStandard,
            13 => StatusCode::AuthAlgorithmNotSupported,
            14 => StatusCode::AuthSequenceOutOfOrder,
            15 => StatusCode::ChallengeFailure,
            16 => StatusCode::AuthTimeout,
            17 => StatusCode::ApFull,
            18 => StatusCode::RateNotSupported,
            _ => StatusCode::Unknown(raw),
        }
    }

    pub fn to_raw(self) -> u16 {
        match self {
            StatusCode::Success => 0,
            StatusCode::UnspecifiedFailure => 1,
            StatusCode::CapabilitiesNotSupported => 10,
            StatusCode::ReassociationDenied => 11,
            StatusCode::AssociationDeniedOutsideStandard => 12,
            StatusCode::AuthAlgorithmNotSupported => 13,
            StatusCode::AuthSequenceOutOfOrder => 14,
            StatusCode::ChallengeFailure => 15,
            StatusCode::AuthTimeout => 16,
            StatusCode::ApFull => 17,
            StatusCode::RateNotSupported => 18,
            StatusCode::Unknown(v) => v,
        }
    }

    pub fn is_success(self) -> bool {
        matches!(self, StatusCode::Success)
    }
}

/// Reason codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasonCode {
    Unspecified,
    PreviousAuthNoLongerValid,
    DeauthLeavingBss,
    DisassocInactivity,
    DisassocApBusy,
    Class2FrameFromNonauth,
    Class3FrameFromNonassoc,
    DisassocLeavingBss,
    StaNotAuthenticated,
    PowerCapBad,
    SupportedChannelsBad,
    InvalidIe,
    MicFailure,
    FourWayHandshakeTimeout,
    GroupKeyHandshakeTimeout,
    IeInFourWayHandshakeDifferent,
    InvalidGroupCipher,
    InvalidPairwiseCipher,
    InvalidAkmp,
    UnsupportedRsnVersion,
    InvalidRsnCapabilities,
    Ieee8021XAuthFailed,
    CipherSuiteRejected,
    Unknown(u16),
}

impl ReasonCode {
    pub fn from_raw(raw: u16) -> Self {
        match raw {
            1 => ReasonCode::Unspecified,
            2 => ReasonCode::PreviousAuthNoLongerValid,
            3 => ReasonCode::DeauthLeavingBss,
            4 => ReasonCode::DisassocInactivity,
            5 => ReasonCode::DisassocApBusy,
            6 => ReasonCode::Class2FrameFromNonauth,
            7 => ReasonCode::Class3FrameFromNonassoc,
            8 => ReasonCode::DisassocLeavingBss,
            9 => ReasonCode::StaNotAuthenticated,
            10 => ReasonCode::PowerCapBad,
            11 => ReasonCode::SupportedChannelsBad,
            13 => ReasonCode::InvalidIe,
            14 => ReasonCode::MicFailure,
            15 => ReasonCode::FourWayHandshakeTimeout,
            16 => ReasonCode::GroupKeyHandshakeTimeout,
            17 => ReasonCode::IeInFourWayHandshakeDifferent,
            18 => ReasonCode::InvalidGroupCipher,
            19 => ReasonCode::InvalidPairwiseCipher,
            20 => ReasonCode::InvalidAkmp,
            21 => ReasonCode::UnsupportedRsnVersion,
            22 => ReasonCode::InvalidRsnCapabilities,
            23 => ReasonCode::Ieee8021XAuthFailed,
            24 => ReasonCode::CipherSuiteRejected,
            _ => ReasonCode::Unknown(raw),
        }
    }

    pub fn to_raw(self) -> u16 {
        match self {
            ReasonCode::Unspecified => 1,
            ReasonCode::PreviousAuthNoLongerValid => 2,
            ReasonCode::DeauthLeavingBss => 3,
            ReasonCode::DisassocInactivity => 4,
            ReasonCode::DisassocApBusy => 5,
            ReasonCode::Class2FrameFromNonauth => 6,
            ReasonCode::Class3FrameFromNonassoc => 7,
            ReasonCode::DisassocLeavingBss => 8,
            ReasonCode::StaNotAuthenticated => 9,
            ReasonCode::PowerCapBad => 10,
            ReasonCode::SupportedChannelsBad => 11,
            ReasonCode::InvalidIe => 13,
            ReasonCode::MicFailure => 14,
            ReasonCode::FourWayHandshakeTimeout => 15,
            ReasonCode::GroupKeyHandshakeTimeout => 16,
            ReasonCode::IeInFourWayHandshakeDifferent => 17,
            ReasonCode::InvalidGroupCipher => 18,
            ReasonCode::InvalidPairwiseCipher => 19,
            ReasonCode::InvalidAkmp => 20,
            ReasonCode::UnsupportedRsnVersion => 21,
            ReasonCode::InvalidRsnCapabilities => 22,
            ReasonCode::Ieee8021XAuthFailed => 23,
            ReasonCode::CipherSuiteRejected => 24,
            ReasonCode::Unknown(v) => v,
        }
    }
}

/// Calculate IEEE 802.11 FCS (CRC-32)
pub fn calculate_fcs(data: &[u8]) -> u32 {
    const CRC32_TABLE: [u32; 256] = generate_crc32_table();

    let mut crc = 0xFFFFFFFF_u32;
    for &byte in data {
        let index = ((crc ^ byte as u32) & 0xFF) as usize;
        crc = (crc >> 8) ^ CRC32_TABLE[index];
    }
    !crc
}

/// Generate CRC32 lookup table at compile time
const fn generate_crc32_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u32;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}
