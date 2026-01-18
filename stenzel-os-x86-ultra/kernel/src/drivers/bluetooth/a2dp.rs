//! Bluetooth A2DP (Advanced Audio Distribution Profile)
//!
//! Implements Bluetooth audio streaming using:
//! - AVDTP (Audio/Video Distribution Transport Protocol)
//! - SBC codec (mandatory for A2DP)
//! - AAC codec (optional)
//!
//! Uses L2CAP channel PSM 0x0019 (AVDTP)
//!
//! References:
//! - Bluetooth A2DP Specification 1.3
//! - Bluetooth AVDTP Specification 1.3
//! - Bluetooth Assigned Numbers (codecs)

extern crate alloc;

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use crate::sync::TicketSpinlock;
use super::BdAddr;

/// L2CAP PSM for AVDTP
pub const AVDTP_PSM: u16 = 0x0019;

/// AVDTP signaling channel packet types
pub mod packet_type {
    pub const SINGLE: u8 = 0x00;
    pub const START: u8 = 0x01;
    pub const CONTINUE: u8 = 0x02;
    pub const END: u8 = 0x03;
}

/// AVDTP message types
pub mod message_type {
    pub const COMMAND: u8 = 0x00;
    pub const GENERAL_REJECT: u8 = 0x01;
    pub const RESPONSE_ACCEPT: u8 = 0x02;
    pub const RESPONSE_REJECT: u8 = 0x03;
}

/// AVDTP signal identifiers
pub mod signal {
    pub const DISCOVER: u8 = 0x01;
    pub const GET_CAPABILITIES: u8 = 0x02;
    pub const SET_CONFIGURATION: u8 = 0x03;
    pub const GET_CONFIGURATION: u8 = 0x04;
    pub const RECONFIGURE: u8 = 0x05;
    pub const OPEN: u8 = 0x06;
    pub const START: u8 = 0x07;
    pub const CLOSE: u8 = 0x08;
    pub const SUSPEND: u8 = 0x09;
    pub const ABORT: u8 = 0x0A;
    pub const SECURITY_CONTROL: u8 = 0x0B;
    pub const GET_ALL_CAPABILITIES: u8 = 0x0C;
    pub const DELAY_REPORT: u8 = 0x0D;
}

/// AVDTP error codes
pub mod error {
    pub const BAD_HEADER_FORMAT: u8 = 0x01;
    pub const BAD_LENGTH: u8 = 0x11;
    pub const BAD_ACP_SEID: u8 = 0x12;
    pub const SEP_IN_USE: u8 = 0x13;
    pub const SEP_NOT_IN_USE: u8 = 0x14;
    pub const BAD_SERV_CATEGORY: u8 = 0x17;
    pub const BAD_PAYLOAD_FORMAT: u8 = 0x18;
    pub const NOT_SUPPORTED_COMMAND: u8 = 0x19;
    pub const INVALID_CAPABILITIES: u8 = 0x1A;
    pub const BAD_RECOVERY_TYPE: u8 = 0x22;
    pub const BAD_MEDIA_TRANSPORT_FORMAT: u8 = 0x23;
    pub const BAD_RECOVERY_FORMAT: u8 = 0x25;
    pub const BAD_ROHC_FORMAT: u8 = 0x26;
    pub const BAD_CP_FORMAT: u8 = 0x27;
    pub const BAD_MULTIPLEXING_FORMAT: u8 = 0x28;
    pub const UNSUPPORTED_CONFIGURATION: u8 = 0x29;
    pub const BAD_STATE: u8 = 0x31;
}

/// Service categories for capabilities
pub mod category {
    pub const MEDIA_TRANSPORT: u8 = 0x01;
    pub const REPORTING: u8 = 0x02;
    pub const RECOVERY: u8 = 0x03;
    pub const CONTENT_PROTECTION: u8 = 0x04;
    pub const HEADER_COMPRESSION: u8 = 0x05;
    pub const MULTIPLEXING: u8 = 0x06;
    pub const MEDIA_CODEC: u8 = 0x07;
    pub const DELAY_REPORTING: u8 = 0x08;
}

/// Media types
pub mod media_type {
    pub const AUDIO: u8 = 0x00;
    pub const VIDEO: u8 = 0x01;
    pub const MULTIMEDIA: u8 = 0x02;
}

/// Audio codec types (Bluetooth assigned numbers)
pub mod codec {
    pub const SBC: u8 = 0x00;
    pub const MPEG_1_2_AUDIO: u8 = 0x01;
    pub const MPEG_2_4_AAC: u8 = 0x02;
    pub const ATRAC_FAMILY: u8 = 0x04;
    pub const VENDOR_SPECIFIC: u8 = 0xFF;
}

/// SBC codec information element
pub mod sbc {
    // Sampling frequency
    pub const FREQ_16000: u8 = 1 << 7;
    pub const FREQ_32000: u8 = 1 << 6;
    pub const FREQ_44100: u8 = 1 << 5;
    pub const FREQ_48000: u8 = 1 << 4;

    // Channel mode
    pub const MONO: u8 = 1 << 3;
    pub const DUAL_CHANNEL: u8 = 1 << 2;
    pub const STEREO: u8 = 1 << 1;
    pub const JOINT_STEREO: u8 = 1 << 0;

    // Block length
    pub const BLOCKS_4: u8 = 1 << 7;
    pub const BLOCKS_8: u8 = 1 << 6;
    pub const BLOCKS_12: u8 = 1 << 5;
    pub const BLOCKS_16: u8 = 1 << 4;

    // Subbands
    pub const SUBBANDS_4: u8 = 1 << 3;
    pub const SUBBANDS_8: u8 = 1 << 2;

    // Allocation method
    pub const ALLOCATION_SNR: u8 = 1 << 1;
    pub const ALLOCATION_LOUDNESS: u8 = 1 << 0;

    // Minimum/maximum bitpool values
    pub const MIN_BITPOOL: u8 = 2;
    pub const MAX_BITPOOL: u8 = 250;
}

/// SBC codec configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SbcConfiguration {
    /// Sampling frequency in Hz
    pub sample_rate: u32,
    /// Number of channels (1 or 2)
    pub channels: u8,
    /// Block length (4, 8, 12, or 16)
    pub block_length: u8,
    /// Number of subbands (4 or 8)
    pub subbands: u8,
    /// Allocation method
    pub allocation: SbcAllocationMethod,
    /// Bitpool value
    pub bitpool: u8,
}

/// SBC allocation method
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SbcAllocationMethod {
    Snr,
    Loudness,
}

impl Default for SbcConfiguration {
    fn default() -> Self {
        Self {
            sample_rate: 44100,
            channels: 2,
            block_length: 16,
            subbands: 8,
            allocation: SbcAllocationMethod::Loudness,
            bitpool: 53, // Default bitpool for high quality
        }
    }
}

impl SbcConfiguration {
    /// Calculate frame size in bytes
    pub fn frame_size(&self) -> usize {
        let nrof_subbands = self.subbands as usize;
        let nrof_blocks = self.block_length as usize;
        let nrof_channels = self.channels as usize;

        // Header (4 bytes) + join bits + samples
        let join_bits = if self.channels == 2 { nrof_subbands } else { 0 };

        4 + (4 * nrof_subbands * nrof_channels) / 8
            + (nrof_blocks * nrof_channels * self.bitpool as usize + join_bits + 7) / 8
    }

    /// Calculate bitrate in bits per second
    pub fn bitrate(&self) -> u32 {
        let frame_size = self.frame_size() as u32;
        let samples_per_frame = (self.subbands as u32) * (self.block_length as u32);

        (8 * frame_size * self.sample_rate) / samples_per_frame
    }

    /// Encode as capability bytes
    pub fn to_capability_bytes(&self) -> [u8; 4] {
        let mut bytes = [0u8; 4];

        // Byte 0: Sample rate and channel mode
        bytes[0] = match self.sample_rate {
            16000 => sbc::FREQ_16000,
            32000 => sbc::FREQ_32000,
            44100 => sbc::FREQ_44100,
            48000 => sbc::FREQ_48000,
            _ => sbc::FREQ_44100,
        };
        bytes[0] |= if self.channels == 1 { sbc::MONO } else { sbc::JOINT_STEREO };

        // Byte 1: Block length, subbands, allocation method
        bytes[1] = match self.block_length {
            4 => sbc::BLOCKS_4,
            8 => sbc::BLOCKS_8,
            12 => sbc::BLOCKS_12,
            16 => sbc::BLOCKS_16,
            _ => sbc::BLOCKS_16,
        };
        bytes[1] |= if self.subbands == 4 { sbc::SUBBANDS_4 } else { sbc::SUBBANDS_8 };
        bytes[1] |= match self.allocation {
            SbcAllocationMethod::Snr => sbc::ALLOCATION_SNR,
            SbcAllocationMethod::Loudness => sbc::ALLOCATION_LOUDNESS,
        };

        // Byte 2-3: Bitpool range
        bytes[2] = sbc::MIN_BITPOOL;
        bytes[3] = self.bitpool;

        bytes
    }

    /// Parse from capability bytes
    pub fn from_capability_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 4 {
            return None;
        }

        let sample_rate = if bytes[0] & sbc::FREQ_48000 != 0 {
            48000
        } else if bytes[0] & sbc::FREQ_44100 != 0 {
            44100
        } else if bytes[0] & sbc::FREQ_32000 != 0 {
            32000
        } else {
            16000
        };

        let channels = if bytes[0] & sbc::MONO != 0 { 1 } else { 2 };

        let block_length = if bytes[1] & sbc::BLOCKS_16 != 0 {
            16
        } else if bytes[1] & sbc::BLOCKS_12 != 0 {
            12
        } else if bytes[1] & sbc::BLOCKS_8 != 0 {
            8
        } else {
            4
        };

        let subbands = if bytes[1] & sbc::SUBBANDS_8 != 0 { 8 } else { 4 };

        let allocation = if bytes[1] & sbc::ALLOCATION_SNR != 0 {
            SbcAllocationMethod::Snr
        } else {
            SbcAllocationMethod::Loudness
        };

        Some(Self {
            sample_rate,
            channels,
            block_length,
            subbands,
            allocation,
            bitpool: bytes[3],
        })
    }
}

/// Stream Endpoint type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SepType {
    Source = 0,
    Sink = 1,
}

/// Stream Endpoint state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SepState {
    Idle,
    Configured,
    Open,
    Streaming,
    Closing,
    Aborting,
}

/// Stream Endpoint
#[derive(Debug)]
pub struct StreamEndpoint {
    /// Local SEID (Stream Endpoint ID, 1-63)
    pub seid: u8,
    /// Type (source or sink)
    pub sep_type: SepType,
    /// In use flag
    pub in_use: bool,
    /// Media type
    pub media_type: u8,
    /// Codec type
    pub codec_type: u8,
    /// State
    pub state: SepState,
    /// Remote SEID (when configured)
    pub remote_seid: Option<u8>,
    /// SBC configuration (if SBC codec)
    pub sbc_config: Option<SbcConfiguration>,
    /// Transport channel CID
    pub transport_cid: Option<u16>,
}

impl StreamEndpoint {
    pub fn new(seid: u8, sep_type: SepType, media_type: u8, codec_type: u8) -> Self {
        Self {
            seid,
            sep_type,
            in_use: false,
            media_type,
            codec_type,
            state: SepState::Idle,
            remote_seid: None,
            sbc_config: None,
            transport_cid: None,
        }
    }

    /// Build capability response for this endpoint
    pub fn build_capabilities(&self) -> Vec<u8> {
        let mut caps = Vec::new();

        // Media Transport
        caps.push(category::MEDIA_TRANSPORT);
        caps.push(0); // Length

        // Media Codec
        caps.push(category::MEDIA_CODEC);
        if self.codec_type == codec::SBC {
            caps.push(6); // Length
            caps.push(self.media_type << 4);
            caps.push(codec::SBC);

            // SBC capabilities
            let config = self.sbc_config.unwrap_or_default();
            let cap_bytes = config.to_capability_bytes();
            caps.extend_from_slice(&cap_bytes);
        } else {
            caps.push(2);
            caps.push(self.media_type << 4);
            caps.push(self.codec_type);
        }

        caps
    }
}

/// A2DP connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum A2dpState {
    Disconnected,
    Connecting,
    Connected,
    Configured,
    Open,
    Streaming,
    Suspended,
    Error,
}

/// A2DP role
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum A2dpRole {
    /// Audio source (e.g., phone)
    Source,
    /// Audio sink (e.g., headphones)
    Sink,
}

/// A2DP connection
pub struct A2dpConnection {
    /// Remote device address
    pub address: BdAddr,
    /// Device name
    pub name: Option<String>,
    /// Connection state
    pub state: A2dpState,
    /// Our role
    pub role: A2dpRole,
    /// ACL connection handle
    pub acl_handle: Option<u16>,
    /// Signaling channel CID
    pub signaling_cid: Option<u16>,
    /// Local stream endpoints
    pub local_seps: Vec<StreamEndpoint>,
    /// Transaction label counter
    pub transaction_label: u8,
    /// Pending command (for matching responses)
    pub pending_command: Option<u8>,
    /// Audio buffer
    pub audio_buffer: Vec<u8>,
    /// Sequence number for RTP
    pub sequence_number: u16,
    /// Timestamp for RTP
    pub timestamp: u32,
}

impl A2dpConnection {
    pub fn new(address: BdAddr, role: A2dpRole) -> Self {
        let mut conn = Self {
            address,
            name: None,
            state: A2dpState::Disconnected,
            role,
            acl_handle: None,
            signaling_cid: None,
            local_seps: Vec::new(),
            transaction_label: 0,
            pending_command: None,
            audio_buffer: Vec::new(),
            sequence_number: 0,
            timestamp: 0,
        };

        // Create default endpoint based on role
        let sep_type = match role {
            A2dpRole::Source => SepType::Source,
            A2dpRole::Sink => SepType::Sink,
        };

        let mut sep = StreamEndpoint::new(1, sep_type, media_type::AUDIO, codec::SBC);
        sep.sbc_config = Some(SbcConfiguration::default());
        conn.local_seps.push(sep);

        conn
    }

    /// Get next transaction label
    pub fn next_transaction_label(&mut self) -> u8 {
        let label = self.transaction_label;
        self.transaction_label = (self.transaction_label + 1) & 0x0F;
        label
    }

    /// Get configured endpoint
    pub fn configured_endpoint(&self) -> Option<&StreamEndpoint> {
        self.local_seps.iter().find(|sep| sep.state != SepState::Idle)
    }

    /// Get configured endpoint (mutable)
    pub fn configured_endpoint_mut(&mut self) -> Option<&mut StreamEndpoint> {
        self.local_seps.iter_mut().find(|sep| sep.state != SepState::Idle)
    }
}

/// AVDTP signaling packet builder
pub struct AvdtpBuilder;

impl AvdtpBuilder {
    /// Build single packet header
    pub fn header(transaction_label: u8, message_type: u8, signal_id: u8) -> Vec<u8> {
        vec![
            (transaction_label << 4) | (packet_type::SINGLE << 2) | message_type,
            signal_id,
        ]
    }

    /// Build DISCOVER command
    pub fn discover(transaction_label: u8) -> Vec<u8> {
        Self::header(transaction_label, message_type::COMMAND, signal::DISCOVER)
    }

    /// Build GET_CAPABILITIES command
    pub fn get_capabilities(transaction_label: u8, acp_seid: u8) -> Vec<u8> {
        let mut pdu = Self::header(transaction_label, message_type::COMMAND, signal::GET_CAPABILITIES);
        pdu.push(acp_seid << 2);
        pdu
    }

    /// Build GET_ALL_CAPABILITIES command
    pub fn get_all_capabilities(transaction_label: u8, acp_seid: u8) -> Vec<u8> {
        let mut pdu = Self::header(transaction_label, message_type::COMMAND, signal::GET_ALL_CAPABILITIES);
        pdu.push(acp_seid << 2);
        pdu
    }

    /// Build SET_CONFIGURATION command
    pub fn set_configuration(
        transaction_label: u8,
        acp_seid: u8,
        int_seid: u8,
        capabilities: &[u8],
    ) -> Vec<u8> {
        let mut pdu = Self::header(transaction_label, message_type::COMMAND, signal::SET_CONFIGURATION);
        pdu.push(acp_seid << 2);
        pdu.push(int_seid << 2);
        pdu.extend_from_slice(capabilities);
        pdu
    }

    /// Build OPEN command
    pub fn open(transaction_label: u8, acp_seid: u8) -> Vec<u8> {
        let mut pdu = Self::header(transaction_label, message_type::COMMAND, signal::OPEN);
        pdu.push(acp_seid << 2);
        pdu
    }

    /// Build START command
    pub fn start(transaction_label: u8, seid_list: &[u8]) -> Vec<u8> {
        let mut pdu = Self::header(transaction_label, message_type::COMMAND, signal::START);
        for &seid in seid_list {
            pdu.push(seid << 2);
        }
        pdu
    }

    /// Build SUSPEND command
    pub fn suspend(transaction_label: u8, seid_list: &[u8]) -> Vec<u8> {
        let mut pdu = Self::header(transaction_label, message_type::COMMAND, signal::SUSPEND);
        for &seid in seid_list {
            pdu.push(seid << 2);
        }
        pdu
    }

    /// Build CLOSE command
    pub fn close(transaction_label: u8, acp_seid: u8) -> Vec<u8> {
        let mut pdu = Self::header(transaction_label, message_type::COMMAND, signal::CLOSE);
        pdu.push(acp_seid << 2);
        pdu
    }

    /// Build ABORT command
    pub fn abort(transaction_label: u8, acp_seid: u8) -> Vec<u8> {
        let mut pdu = Self::header(transaction_label, message_type::COMMAND, signal::ABORT);
        pdu.push(acp_seid << 2);
        pdu
    }

    /// Build DELAY_REPORT command
    pub fn delay_report(transaction_label: u8, acp_seid: u8, delay: u16) -> Vec<u8> {
        let mut pdu = Self::header(transaction_label, message_type::COMMAND, signal::DELAY_REPORT);
        pdu.push(acp_seid << 2);
        pdu.extend_from_slice(&delay.to_be_bytes());
        pdu
    }

    /// Build accept response
    pub fn accept(transaction_label: u8, signal_id: u8) -> Vec<u8> {
        Self::header(transaction_label, message_type::RESPONSE_ACCEPT, signal_id)
    }

    /// Build reject response
    pub fn reject(transaction_label: u8, signal_id: u8, error_code: u8) -> Vec<u8> {
        let mut pdu = Self::header(transaction_label, message_type::RESPONSE_REJECT, signal_id);
        pdu.push(error_code);
        pdu
    }

    /// Build DISCOVER response
    pub fn discover_response(transaction_label: u8, endpoints: &[&StreamEndpoint]) -> Vec<u8> {
        let mut pdu = Self::header(transaction_label, message_type::RESPONSE_ACCEPT, signal::DISCOVER);
        for sep in endpoints {
            let info = (sep.seid << 2) | if sep.in_use { 2 } else { 0 };
            let type_info = ((sep.media_type & 0x0F) << 4) | ((sep.sep_type as u8) << 3);
            pdu.push(info);
            pdu.push(type_info);
        }
        pdu
    }

    /// Build capabilities response
    pub fn capabilities_response(transaction_label: u8, capabilities: &[u8]) -> Vec<u8> {
        let mut pdu = Self::header(transaction_label, message_type::RESPONSE_ACCEPT, signal::GET_CAPABILITIES);
        pdu.extend_from_slice(capabilities);
        pdu
    }
}

/// RTP header for A2DP media packets
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct RtpHeader {
    /// Version (2), Padding (1), Extension (1), CSRC count (4)
    pub flags1: u8,
    /// Marker (1), Payload type (7)
    pub flags2: u8,
    /// Sequence number
    pub sequence: u16,
    /// Timestamp
    pub timestamp: u32,
    /// SSRC
    pub ssrc: u32,
}

impl RtpHeader {
    pub const SIZE: usize = 12;

    pub fn new(sequence: u16, timestamp: u32, ssrc: u32) -> Self {
        Self {
            flags1: 0x80, // Version 2
            flags2: 0x60, // Payload type 96 (dynamic)
            sequence: sequence.to_be(),
            timestamp: timestamp.to_be(),
            ssrc: ssrc.to_be(),
        }
    }

    pub fn to_bytes(&self) -> [u8; 12] {
        unsafe { core::mem::transmute(*self) }
    }
}

/// A2DP media packet header (after RTP)
#[derive(Debug, Clone, Copy)]
pub struct A2dpMediaHeader {
    /// Fragment indicator
    pub fragmented: bool,
    /// Starting packet
    pub starting: bool,
    /// Last packet
    pub last: bool,
    /// Number of frames
    pub num_frames: u8,
}

impl A2dpMediaHeader {
    pub fn new(num_frames: u8) -> Self {
        Self {
            fragmented: false,
            starting: false,
            last: false,
            num_frames,
        }
    }

    pub fn to_byte(&self) -> u8 {
        let mut b = 0u8;
        if self.fragmented { b |= 0x80; }
        if self.starting { b |= 0x40; }
        if self.last { b |= 0x20; }
        b | (self.num_frames & 0x0F)
    }
}

/// Bluetooth A2DP manager
pub struct A2dpManager {
    /// Active connections
    connections: Vec<A2dpConnection>,
    /// Default role
    default_role: A2dpRole,
    /// SSRC for RTP
    ssrc: u32,
    /// Audio callback (for sink mode)
    audio_callback: Option<fn(&[u8], &SbcConfiguration)>,
}

impl A2dpManager {
    pub const fn new() -> Self {
        Self {
            connections: Vec::new(),
            default_role: A2dpRole::Sink,
            ssrc: 0x12345678,
            audio_callback: None,
        }
    }

    /// Set default role
    pub fn set_role(&mut self, role: A2dpRole) {
        self.default_role = role;
    }

    /// Set audio callback for sink mode
    pub fn on_audio(&mut self, callback: fn(&[u8], &SbcConfiguration)) {
        self.audio_callback = Some(callback);
    }

    /// Add connection
    pub fn add_connection(&mut self, address: BdAddr) {
        if self.get_connection(&address).is_some() {
            return;
        }

        let conn = A2dpConnection::new(address, self.default_role);
        self.connections.push(conn);

        crate::kprintln!("a2dp: added connection {}", address.to_string());
    }

    /// Get connection by address
    pub fn get_connection(&self, address: &BdAddr) -> Option<&A2dpConnection> {
        self.connections.iter().find(|c| c.address == *address)
    }

    /// Get connection by address (mutable)
    pub fn get_connection_mut(&mut self, address: &BdAddr) -> Option<&mut A2dpConnection> {
        self.connections.iter_mut().find(|c| c.address == *address)
    }

    /// Remove connection
    pub fn remove_connection(&mut self, address: &BdAddr) {
        self.connections.retain(|c| c.address != *address);
    }

    /// Handle L2CAP connection
    pub fn handle_l2cap_connect(&mut self, address: &BdAddr, cid: u16) {
        if let Some(conn) = self.get_connection_mut(address) {
            conn.signaling_cid = Some(cid);
            conn.state = A2dpState::Connected;
            crate::kprintln!("a2dp: signaling channel connected (CID {})", cid);
        }
    }

    /// Handle L2CAP disconnection
    pub fn handle_l2cap_disconnect(&mut self, address: &BdAddr, cid: u16) {
        if let Some(conn) = self.get_connection_mut(address) {
            if conn.signaling_cid == Some(cid) {
                conn.signaling_cid = None;
                conn.state = A2dpState::Disconnected;
            }
            // Check transport channel
            if let Some(sep) = conn.configured_endpoint_mut() {
                if sep.transport_cid == Some(cid) {
                    sep.transport_cid = None;
                    sep.state = SepState::Configured;
                }
            }
        }
    }

    /// Process AVDTP signaling data
    pub fn process_signaling(&mut self, address: &BdAddr, data: &[u8]) -> Option<Vec<u8>> {
        if data.len() < 2 {
            return None;
        }

        let header = data[0];
        let transaction_label = (header >> 4) & 0x0F;
        let _packet_type = (header >> 2) & 0x03;
        let message_type = header & 0x03;
        let signal_id = data[1];

        // Handle commands
        if message_type == message_type::COMMAND {
            return self.handle_command(address, transaction_label, signal_id, &data[2..]);
        }

        // Handle responses
        if message_type == message_type::RESPONSE_ACCEPT ||
           message_type == message_type::RESPONSE_REJECT {
            self.handle_response(address, signal_id, message_type == message_type::RESPONSE_ACCEPT, &data[2..]);
        }

        None
    }

    /// Handle AVDTP command
    fn handle_command(&mut self, address: &BdAddr, label: u8, signal: u8, params: &[u8]) -> Option<Vec<u8>> {
        match signal {
            signal::DISCOVER => {
                // Return our stream endpoints
                let seps: Vec<&StreamEndpoint> = if let Some(conn) = self.get_connection(address) {
                    conn.local_seps.iter().collect()
                } else {
                    Vec::new()
                };

                Some(AvdtpBuilder::discover_response(label, &seps))
            }

            signal::GET_CAPABILITIES | signal::GET_ALL_CAPABILITIES => {
                if params.is_empty() {
                    return Some(AvdtpBuilder::reject(label, signal, error::BAD_LENGTH));
                }

                let seid = params[0] >> 2;

                if let Some(conn) = self.get_connection(address) {
                    if let Some(sep) = conn.local_seps.iter().find(|s| s.seid == seid) {
                        let caps = sep.build_capabilities();
                        return Some(AvdtpBuilder::capabilities_response(label, &caps));
                    }
                }

                Some(AvdtpBuilder::reject(label, signal, error::BAD_ACP_SEID))
            }

            signal::SET_CONFIGURATION => {
                if params.len() < 2 {
                    return Some(AvdtpBuilder::reject(label, signal, error::BAD_LENGTH));
                }

                let acp_seid = params[0] >> 2;
                let int_seid = params[1] >> 2;
                let _caps = &params[2..];

                if let Some(conn) = self.get_connection_mut(address) {
                    if let Some(sep) = conn.local_seps.iter_mut().find(|s| s.seid == acp_seid) {
                        sep.state = SepState::Configured;
                        sep.remote_seid = Some(int_seid);
                        sep.in_use = true;
                        conn.state = A2dpState::Configured;

                        return Some(AvdtpBuilder::accept(label, signal));
                    }
                }

                Some(AvdtpBuilder::reject(label, signal, error::BAD_ACP_SEID))
            }

            signal::OPEN => {
                if params.is_empty() {
                    return Some(AvdtpBuilder::reject(label, signal, error::BAD_LENGTH));
                }

                let acp_seid = params[0] >> 2;

                if let Some(conn) = self.get_connection_mut(address) {
                    if let Some(sep) = conn.local_seps.iter_mut().find(|s| s.seid == acp_seid) {
                        if sep.state == SepState::Configured {
                            sep.state = SepState::Open;
                            conn.state = A2dpState::Open;
                            return Some(AvdtpBuilder::accept(label, signal));
                        }
                    }
                }

                Some(AvdtpBuilder::reject(label, signal, error::BAD_STATE))
            }

            signal::START => {
                if params.is_empty() {
                    return Some(AvdtpBuilder::reject(label, signal, error::BAD_LENGTH));
                }

                if let Some(conn) = self.get_connection_mut(address) {
                    for i in 0..params.len() {
                        let seid = params[i] >> 2;
                        if let Some(sep) = conn.local_seps.iter_mut().find(|s| s.seid == seid) {
                            if sep.state == SepState::Open {
                                sep.state = SepState::Streaming;
                            }
                        }
                    }
                    conn.state = A2dpState::Streaming;
                    crate::kprintln!("a2dp: streaming started");
                    return Some(AvdtpBuilder::accept(label, signal));
                }

                Some(AvdtpBuilder::reject(label, signal, error::BAD_STATE))
            }

            signal::SUSPEND => {
                if params.is_empty() {
                    return Some(AvdtpBuilder::reject(label, signal, error::BAD_LENGTH));
                }

                if let Some(conn) = self.get_connection_mut(address) {
                    for i in 0..params.len() {
                        let seid = params[i] >> 2;
                        if let Some(sep) = conn.local_seps.iter_mut().find(|s| s.seid == seid) {
                            if sep.state == SepState::Streaming {
                                sep.state = SepState::Open;
                            }
                        }
                    }
                    conn.state = A2dpState::Suspended;
                    return Some(AvdtpBuilder::accept(label, signal));
                }

                Some(AvdtpBuilder::reject(label, signal, error::BAD_STATE))
            }

            signal::CLOSE => {
                if params.is_empty() {
                    return Some(AvdtpBuilder::reject(label, signal, error::BAD_LENGTH));
                }

                let acp_seid = params[0] >> 2;

                if let Some(conn) = self.get_connection_mut(address) {
                    if let Some(sep) = conn.local_seps.iter_mut().find(|s| s.seid == acp_seid) {
                        sep.state = SepState::Closing;
                        return Some(AvdtpBuilder::accept(label, signal));
                    }
                }

                Some(AvdtpBuilder::reject(label, signal, error::BAD_ACP_SEID))
            }

            signal::ABORT => {
                if params.is_empty() {
                    return Some(AvdtpBuilder::reject(label, signal, error::BAD_LENGTH));
                }

                let acp_seid = params[0] >> 2;

                if let Some(conn) = self.get_connection_mut(address) {
                    if let Some(sep) = conn.local_seps.iter_mut().find(|s| s.seid == acp_seid) {
                        sep.state = SepState::Idle;
                        sep.in_use = false;
                        sep.remote_seid = None;
                        return Some(AvdtpBuilder::accept(label, signal));
                    }
                }

                Some(AvdtpBuilder::accept(label, signal))
            }

            signal::DELAY_REPORT => {
                // Accept delay reports
                Some(AvdtpBuilder::accept(label, signal))
            }

            _ => {
                Some(AvdtpBuilder::reject(label, signal, error::NOT_SUPPORTED_COMMAND))
            }
        }
    }

    /// Handle AVDTP response
    fn handle_response(&mut self, address: &BdAddr, signal: u8, accepted: bool, params: &[u8]) {
        if accepted {
            crate::kprintln!("a2dp: signal {} accepted", signal);
        } else {
            let error = params.first().copied().unwrap_or(0);
            crate::kprintln!("a2dp: signal {} rejected (error {})", signal, error);
        }
    }

    /// Process media data (audio)
    pub fn process_media(&mut self, address: &BdAddr, data: &[u8]) {
        if data.len() < RtpHeader::SIZE + 1 {
            return;
        }

        // Skip RTP header
        let media_header = data[RtpHeader::SIZE];
        let _num_frames = media_header & 0x0F;
        let audio_data = &data[RtpHeader::SIZE + 1..];

        // Get SBC config
        let config = if let Some(conn) = self.get_connection(address) {
            conn.configured_endpoint()
                .and_then(|sep| sep.sbc_config)
                .unwrap_or_default()
        } else {
            return;
        };

        // Call audio callback
        if let Some(callback) = self.audio_callback {
            callback(audio_data, &config);
        }
    }

    /// Build media packet for source mode
    pub fn build_media_packet(&mut self, address: &BdAddr, sbc_data: &[u8], num_frames: u8) -> Option<Vec<u8>> {
        let conn = self.get_connection_mut(address)?;

        let seq = conn.sequence_number;
        conn.sequence_number = conn.sequence_number.wrapping_add(1);

        let config = conn.configured_endpoint()?.sbc_config?;
        let samples_per_frame = (config.subbands as u32) * (config.block_length as u32);
        conn.timestamp = conn.timestamp.wrapping_add(samples_per_frame * num_frames as u32);

        let rtp = RtpHeader::new(seq, conn.timestamp, self.ssrc);
        let media_header = A2dpMediaHeader::new(num_frames);

        let mut packet = Vec::with_capacity(RtpHeader::SIZE + 1 + sbc_data.len());
        packet.extend_from_slice(&rtp.to_bytes());
        packet.push(media_header.to_byte());
        packet.extend_from_slice(sbc_data);

        Some(packet)
    }

    /// Get connection count
    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    /// Get streaming connection count
    pub fn streaming_count(&self) -> usize {
        self.connections.iter()
            .filter(|c| c.state == A2dpState::Streaming)
            .count()
    }
}

/// Global A2DP manager
pub static A2DP_MANAGER: TicketSpinlock<A2dpManager> =
    TicketSpinlock::new(A2dpManager::new());

// =============================================================================
// Public API
// =============================================================================

/// Initialize A2DP subsystem
pub fn init() {
    crate::kprintln!("bluetooth: A2DP profile initialized");
}

/// Add A2DP connection
pub fn add_connection(address: BdAddr) {
    A2DP_MANAGER.lock().add_connection(address);
}

/// Remove A2DP connection
pub fn remove_connection(address: &BdAddr) {
    A2DP_MANAGER.lock().remove_connection(address);
}

/// Handle L2CAP connection
pub fn handle_connect(address: &BdAddr, cid: u16) {
    A2DP_MANAGER.lock().handle_l2cap_connect(address, cid);
}

/// Handle L2CAP disconnection
pub fn handle_disconnect(address: &BdAddr, cid: u16) {
    A2DP_MANAGER.lock().handle_l2cap_disconnect(address, cid);
}

/// Process signaling data
pub fn process_signaling(address: &BdAddr, data: &[u8]) -> Option<Vec<u8>> {
    A2DP_MANAGER.lock().process_signaling(address, data)
}

/// Process media data
pub fn process_media(address: &BdAddr, data: &[u8]) {
    A2DP_MANAGER.lock().process_media(address, data);
}

/// Set audio callback
pub fn on_audio(callback: fn(&[u8], &SbcConfiguration)) {
    A2DP_MANAGER.lock().on_audio(callback);
}

/// Set A2DP role
pub fn set_role(role: A2dpRole) {
    A2DP_MANAGER.lock().set_role(role);
}

/// Get connection count
pub fn connection_count() -> usize {
    A2DP_MANAGER.lock().connection_count()
}

/// Get streaming count
pub fn streaming_count() -> usize {
    A2DP_MANAGER.lock().streaming_count()
}

/// Format status
pub fn format_status() -> String {
    use core::fmt::Write;
    let mut output = String::new();

    let manager = A2DP_MANAGER.lock();

    writeln!(output, "A2DP Connections: {}", manager.connection_count()).ok();
    writeln!(output, "Streaming: {}", manager.streaming_count()).ok();

    for conn in &manager.connections {
        writeln!(output, "  {} - {:?} ({:?})",
            conn.address.to_string(),
            conn.state,
            conn.role
        ).ok();

        for sep in &conn.local_seps {
            writeln!(output, "    SEP {} {:?} {:?}",
                sep.seid,
                sep.sep_type,
                sep.state
            ).ok();

            if let Some(config) = &sep.sbc_config {
                writeln!(output, "      SBC: {}Hz {}ch {} kbps",
                    config.sample_rate,
                    config.channels,
                    config.bitrate() / 1000
                ).ok();
            }
        }
    }

    output
}
