//! MAC Address and MAC Layer
//!
//! IEEE 802.11 MAC address handling and utilities.

use alloc::string::String;
use core::fmt;

/// IEEE 802.11 MAC Address (6 bytes)
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(C)]
pub struct MacAddress(pub [u8; 6]);

impl MacAddress {
    /// Broadcast address (FF:FF:FF:FF:FF:FF)
    pub const BROADCAST: MacAddress = MacAddress([0xFF; 6]);

    /// Zero/null address (00:00:00:00:00:00)
    pub const ZERO: MacAddress = MacAddress([0; 6]);

    /// Create from bytes
    pub const fn new(bytes: [u8; 6]) -> Self {
        MacAddress(bytes)
    }

    /// Create from slice
    pub fn from_slice(bytes: &[u8]) -> Option<Self> {
        if bytes.len() >= 6 {
            let mut addr = [0u8; 6];
            addr.copy_from_slice(&bytes[..6]);
            Some(MacAddress(addr))
        } else {
            None
        }
    }

    /// Get raw bytes
    pub fn as_bytes(&self) -> &[u8; 6] {
        &self.0
    }

    /// Check if broadcast address
    pub fn is_broadcast(&self) -> bool {
        *self == Self::BROADCAST
    }

    /// Check if multicast address (bit 0 of first byte set)
    pub fn is_multicast(&self) -> bool {
        self.0[0] & 0x01 != 0
    }

    /// Check if unicast address
    pub fn is_unicast(&self) -> bool {
        !self.is_multicast()
    }

    /// Check if locally administered (bit 1 of first byte set)
    pub fn is_local(&self) -> bool {
        self.0[0] & 0x02 != 0
    }

    /// Check if universally administered
    pub fn is_universal(&self) -> bool {
        !self.is_local()
    }

    /// Check if zero/null address
    pub fn is_zero(&self) -> bool {
        *self == Self::ZERO
    }

    /// Get OUI (Organizationally Unique Identifier) - first 3 bytes
    pub fn oui(&self) -> [u8; 3] {
        [self.0[0], self.0[1], self.0[2]]
    }

    /// Get NIC-specific part - last 3 bytes
    pub fn nic(&self) -> [u8; 3] {
        [self.0[3], self.0[4], self.0[5]]
    }

    /// Parse from string (format: "XX:XX:XX:XX:XX:XX" or "XX-XX-XX-XX-XX-XX")
    pub fn parse(s: &str) -> Option<Self> {
        let parts: alloc::vec::Vec<&str> = if s.contains(':') {
            s.split(':').collect()
        } else if s.contains('-') {
            s.split('-').collect()
        } else {
            return None;
        };

        if parts.len() != 6 {
            return None;
        }

        let mut bytes = [0u8; 6];
        for (i, part) in parts.iter().enumerate() {
            bytes[i] = u8::from_str_radix(part, 16).ok()?;
        }

        Some(MacAddress(bytes))
    }

    /// Format as string
    pub fn to_string(&self) -> String {
        alloc::format!(
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            self.0[0], self.0[1], self.0[2],
            self.0[3], self.0[4], self.0[5]
        )
    }
}

impl fmt::Debug for MacAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            self.0[0], self.0[1], self.0[2],
            self.0[3], self.0[4], self.0[5]
        )
    }
}

impl fmt::Display for MacAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            self.0[0], self.0[1], self.0[2],
            self.0[3], self.0[4], self.0[5]
        )
    }
}

impl From<[u8; 6]> for MacAddress {
    fn from(bytes: [u8; 6]) -> Self {
        MacAddress(bytes)
    }
}

impl From<MacAddress> for [u8; 6] {
    fn from(mac: MacAddress) -> Self {
        mac.0
    }
}

/// MAC header fields
#[derive(Debug, Clone, Copy)]
pub struct MacHeader {
    /// Frame control field
    pub frame_control: FrameControl,
    /// Duration/ID field
    pub duration_id: u16,
    /// Address 1 (receiver/destination)
    pub addr1: MacAddress,
    /// Address 2 (transmitter/source)
    pub addr2: MacAddress,
    /// Address 3 (BSSID or destination/source depending on frame type)
    pub addr3: MacAddress,
    /// Sequence control
    pub sequence_control: u16,
    /// Address 4 (used in WDS frames)
    pub addr4: Option<MacAddress>,
}

impl MacHeader {
    /// Parse from bytes
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 24 {
            return None;
        }

        let frame_control = FrameControl::from_raw(u16::from_le_bytes([data[0], data[1]]));
        let duration_id = u16::from_le_bytes([data[2], data[3]]);
        let addr1 = MacAddress::from_slice(&data[4..10])?;
        let addr2 = MacAddress::from_slice(&data[10..16])?;
        let addr3 = MacAddress::from_slice(&data[16..22])?;
        let sequence_control = u16::from_le_bytes([data[22], data[23]]);

        // Address 4 present in WDS frames (To DS and From DS both set)
        let addr4 = if frame_control.to_ds() && frame_control.from_ds() {
            if data.len() >= 30 {
                MacAddress::from_slice(&data[24..30])
            } else {
                None
            }
        } else {
            None
        };

        Some(MacHeader {
            frame_control,
            duration_id,
            addr1,
            addr2,
            addr3,
            sequence_control,
            addr4,
        })
    }

    /// Get header size in bytes
    pub fn size(&self) -> usize {
        if self.addr4.is_some() {
            30
        } else {
            24
        }
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> alloc::vec::Vec<u8> {
        let mut bytes = alloc::vec::Vec::with_capacity(self.size());

        bytes.extend_from_slice(&self.frame_control.to_raw().to_le_bytes());
        bytes.extend_from_slice(&self.duration_id.to_le_bytes());
        bytes.extend_from_slice(self.addr1.as_bytes());
        bytes.extend_from_slice(self.addr2.as_bytes());
        bytes.extend_from_slice(self.addr3.as_bytes());
        bytes.extend_from_slice(&self.sequence_control.to_le_bytes());

        if let Some(addr4) = self.addr4 {
            bytes.extend_from_slice(addr4.as_bytes());
        }

        bytes
    }

    /// Get sequence number from sequence control field
    pub fn sequence_number(&self) -> u16 {
        self.sequence_control >> 4
    }

    /// Get fragment number from sequence control field
    pub fn fragment_number(&self) -> u8 {
        (self.sequence_control & 0x0F) as u8
    }
}

/// Frame control field (2 bytes)
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameControl(u16);

impl FrameControl {
    /// Create from raw value
    pub fn from_raw(raw: u16) -> Self {
        FrameControl(raw)
    }

    /// Get raw value
    pub fn to_raw(self) -> u16 {
        self.0
    }

    /// Protocol version (bits 0-1)
    pub fn protocol_version(self) -> u8 {
        (self.0 & 0x03) as u8
    }

    /// Frame type (bits 2-3)
    pub fn frame_type(self) -> FrameType {
        match (self.0 >> 2) & 0x03 {
            0 => FrameType::Management,
            1 => FrameType::Control,
            2 => FrameType::Data,
            _ => FrameType::Reserved,
        }
    }

    /// Frame subtype (bits 4-7)
    pub fn subtype(self) -> u8 {
        ((self.0 >> 4) & 0x0F) as u8
    }

    /// To DS flag (bit 8)
    pub fn to_ds(self) -> bool {
        self.0 & (1 << 8) != 0
    }

    /// From DS flag (bit 9)
    pub fn from_ds(self) -> bool {
        self.0 & (1 << 9) != 0
    }

    /// More fragments flag (bit 10)
    pub fn more_fragments(self) -> bool {
        self.0 & (1 << 10) != 0
    }

    /// Retry flag (bit 11)
    pub fn retry(self) -> bool {
        self.0 & (1 << 11) != 0
    }

    /// Power management flag (bit 12)
    pub fn power_management(self) -> bool {
        self.0 & (1 << 12) != 0
    }

    /// More data flag (bit 13)
    pub fn more_data(self) -> bool {
        self.0 & (1 << 13) != 0
    }

    /// Protected frame flag (bit 14) - WEP/WPA encryption
    pub fn protected(self) -> bool {
        self.0 & (1 << 14) != 0
    }

    /// Order flag (bit 15)
    pub fn order(self) -> bool {
        self.0 & (1 << 15) != 0
    }

    /// Create new frame control
    pub fn new(
        frame_type: FrameType,
        subtype: u8,
        to_ds: bool,
        from_ds: bool,
    ) -> Self {
        let mut fc: u16 = 0;

        // Protocol version 0
        fc |= (frame_type as u16) << 2;
        fc |= ((subtype & 0x0F) as u16) << 4;

        if to_ds {
            fc |= 1 << 8;
        }
        if from_ds {
            fc |= 1 << 9;
        }

        FrameControl(fc)
    }

    /// Set protected flag
    pub fn set_protected(&mut self, protected: bool) {
        if protected {
            self.0 |= 1 << 14;
        } else {
            self.0 &= !(1 << 14);
        }
    }

    /// Set retry flag
    pub fn set_retry(&mut self, retry: bool) {
        if retry {
            self.0 |= 1 << 11;
        } else {
            self.0 &= !(1 << 11);
        }
    }
}

/// Frame type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FrameType {
    Management = 0,
    Control = 1,
    Data = 2,
    Reserved = 3,
}

/// Management frame subtypes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ManagementSubtype {
    AssociationRequest = 0,
    AssociationResponse = 1,
    ReassociationRequest = 2,
    ReassociationResponse = 3,
    ProbeRequest = 4,
    ProbeResponse = 5,
    TimingAdvertisement = 6,
    Beacon = 8,
    Atim = 9,
    Disassociation = 10,
    Authentication = 11,
    Deauthentication = 12,
    Action = 13,
    ActionNoAck = 14,
}

/// Control frame subtypes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ControlSubtype {
    BlockAckRequest = 8,
    BlockAck = 9,
    PsPoll = 10,
    Rts = 11,
    Cts = 12,
    Ack = 13,
    CfEnd = 14,
    CfEndCfAck = 15,
}

/// Data frame subtypes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DataSubtype {
    Data = 0,
    DataCfAck = 1,
    DataCfPoll = 2,
    DataCfAckCfPoll = 3,
    Null = 4,
    CfAck = 5,
    CfPoll = 6,
    CfAckCfPoll = 7,
    QosData = 8,
    QosDataCfAck = 9,
    QosDataCfPoll = 10,
    QosDataCfAckCfPoll = 11,
    QosNull = 12,
    QosCfPoll = 14,
    QosCfAckCfPoll = 15,
}

/// Sequence number generator
pub struct SequenceGenerator {
    current: u16,
}

impl SequenceGenerator {
    pub const fn new() -> Self {
        Self { current: 0 }
    }

    /// Get next sequence number (0-4095)
    pub fn next(&mut self) -> u16 {
        let seq = self.current;
        self.current = (self.current + 1) & 0x0FFF;
        seq
    }

    /// Create sequence control field
    pub fn sequence_control(&mut self, fragment: u8) -> u16 {
        (self.next() << 4) | ((fragment & 0x0F) as u16)
    }
}

impl Default for SequenceGenerator {
    fn default() -> Self {
        Self::new()
    }
}
