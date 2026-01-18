//! L2CAP (Logical Link Control and Adaptation Protocol)
//!
//! Provides connection-oriented and connectionless data channels.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

/// L2CAP Channel IDs
pub mod cid {
    pub const NULL: u16 = 0x0000;
    pub const SIGNALING: u16 = 0x0001;
    pub const CONNECTIONLESS: u16 = 0x0002;
    pub const AMP_MANAGER: u16 = 0x0003;
    pub const ATT: u16 = 0x0004; // LE Attribute Protocol
    pub const LE_SIGNALING: u16 = 0x0005;
    pub const SMP: u16 = 0x0006; // LE Security Manager Protocol
    pub const BR_SMP: u16 = 0x0007; // BR/EDR Security Manager

    // Dynamic channels start at 0x0040
    pub const DYNAMIC_START: u16 = 0x0040;
    pub const DYNAMIC_END: u16 = 0xFFFF;
}

/// L2CAP PSM (Protocol/Service Multiplexer)
pub mod psm {
    pub const SDP: u16 = 0x0001;
    pub const RFCOMM: u16 = 0x0003;
    pub const BNEP: u16 = 0x000F;
    pub const HID_CONTROL: u16 = 0x0011;
    pub const HID_INTERRUPT: u16 = 0x0013;
    pub const AVCTP: u16 = 0x0017;
    pub const AVDTP: u16 = 0x0019;
    pub const AVCTP_BROWSING: u16 = 0x001B;
    pub const ATT: u16 = 0x001F;
    pub const EATT: u16 = 0x0027;
}

/// L2CAP Signaling Commands
pub mod signal {
    pub const COMMAND_REJECT: u8 = 0x01;
    pub const CONNECTION_REQUEST: u8 = 0x02;
    pub const CONNECTION_RESPONSE: u8 = 0x03;
    pub const CONFIGURATION_REQUEST: u8 = 0x04;
    pub const CONFIGURATION_RESPONSE: u8 = 0x05;
    pub const DISCONNECTION_REQUEST: u8 = 0x06;
    pub const DISCONNECTION_RESPONSE: u8 = 0x07;
    pub const ECHO_REQUEST: u8 = 0x08;
    pub const ECHO_RESPONSE: u8 = 0x09;
    pub const INFORMATION_REQUEST: u8 = 0x0A;
    pub const INFORMATION_RESPONSE: u8 = 0x0B;
    pub const CREATE_CHANNEL_REQUEST: u8 = 0x0C;
    pub const CREATE_CHANNEL_RESPONSE: u8 = 0x0D;
    pub const MOVE_CHANNEL_REQUEST: u8 = 0x0E;
    pub const MOVE_CHANNEL_RESPONSE: u8 = 0x0F;
    pub const MOVE_CHANNEL_CONFIRMATION: u8 = 0x10;
    pub const MOVE_CHANNEL_CONFIRMATION_RESPONSE: u8 = 0x11;
    pub const CONNECTION_PARAMETER_UPDATE_REQUEST: u8 = 0x12;
    pub const CONNECTION_PARAMETER_UPDATE_RESPONSE: u8 = 0x13;
    pub const LE_CREDIT_BASED_CONNECTION_REQUEST: u8 = 0x14;
    pub const LE_CREDIT_BASED_CONNECTION_RESPONSE: u8 = 0x15;
    pub const FLOW_CONTROL_CREDIT: u8 = 0x16;
    pub const CREDIT_BASED_CONNECTION_REQUEST: u8 = 0x17;
    pub const CREDIT_BASED_CONNECTION_RESPONSE: u8 = 0x18;
    pub const CREDIT_BASED_RECONFIGURE_REQUEST: u8 = 0x19;
    pub const CREDIT_BASED_RECONFIGURE_RESPONSE: u8 = 0x1A;
}

/// L2CAP Connection Response Results
pub mod conn_result {
    pub const SUCCESS: u16 = 0x0000;
    pub const PENDING: u16 = 0x0001;
    pub const PSM_NOT_SUPPORTED: u16 = 0x0002;
    pub const SECURITY_BLOCK: u16 = 0x0003;
    pub const NO_RESOURCES: u16 = 0x0004;
    pub const INVALID_SOURCE_CID: u16 = 0x0006;
    pub const SOURCE_CID_ALREADY_ALLOCATED: u16 = 0x0007;
}

/// L2CAP Configuration Response Results
pub mod config_result {
    pub const SUCCESS: u16 = 0x0000;
    pub const UNACCEPTABLE: u16 = 0x0001;
    pub const REJECTED: u16 = 0x0002;
    pub const UNKNOWN_OPTIONS: u16 = 0x0003;
    pub const PENDING: u16 = 0x0004;
    pub const FLOW_SPEC_REJECTED: u16 = 0x0005;
}

/// L2CAP Configuration Option Types
pub mod config_option {
    pub const MTU: u8 = 0x01;
    pub const FLUSH_TIMEOUT: u8 = 0x02;
    pub const QOS: u8 = 0x03;
    pub const RETRANSMISSION_FLOW_CONTROL: u8 = 0x04;
    pub const FCS: u8 = 0x05;
    pub const EXTENDED_FLOW_SPECIFICATION: u8 = 0x06;
    pub const EXTENDED_WINDOW_SIZE: u8 = 0x07;
}

/// L2CAP channel mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelMode {
    Basic,
    RetransmissionMode,
    FlowControlMode,
    EnhancedRetransmissionMode,
    StreamingMode,
    LeCreditBasedFlowControl,
    EnhancedCreditBasedFlowControl,
}

/// L2CAP channel state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelState {
    Closed,
    WaitConnect,
    WaitConnectRsp,
    Config,
    Open,
    WaitDisconnect,
    WaitCreateRsp,
    WaitMoveConfirm,
    WaitConfirmRsp,
}

/// L2CAP channel configuration
#[derive(Debug, Clone)]
pub struct ChannelConfig {
    pub mtu: u16,
    pub flush_timeout: u16,
    pub mode: ChannelMode,
    pub fcs: bool,
    pub tx_window: u8,
    pub max_transmit: u8,
    pub retrans_timeout: u16,
    pub monitor_timeout: u16,
}

impl Default for ChannelConfig {
    fn default() -> Self {
        Self {
            mtu: 672,
            flush_timeout: 0xFFFF,
            mode: ChannelMode::Basic,
            fcs: true,
            tx_window: 1,
            max_transmit: 3,
            retrans_timeout: 2000,
            monitor_timeout: 12000,
        }
    }
}

/// L2CAP channel
#[derive(Debug)]
pub struct L2capChannel {
    pub local_cid: u16,
    pub remote_cid: u16,
    pub psm: u16,
    pub state: ChannelState,
    pub config: ChannelConfig,
    pub remote_config: ChannelConfig,
    pub handle: u16, // ACL handle
    pub credits: u16,
    pub remote_credits: u16,
}

impl L2capChannel {
    pub fn new(local_cid: u16, psm: u16, handle: u16) -> Self {
        Self {
            local_cid,
            remote_cid: 0,
            psm,
            state: ChannelState::Closed,
            config: ChannelConfig::default(),
            remote_config: ChannelConfig::default(),
            handle,
            credits: 0,
            remote_credits: 0,
        }
    }
}

/// L2CAP basic header (4 bytes)
#[derive(Debug, Clone, Copy)]
pub struct L2capHeader {
    pub length: u16,
    pub cid: u16,
}

impl L2capHeader {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 4 {
            return None;
        }

        Some(Self {
            length: u16::from_le_bytes([bytes[0], bytes[1]]),
            cid: u16::from_le_bytes([bytes[2], bytes[3]]),
        })
    }

    pub fn to_bytes(&self) -> [u8; 4] {
        [
            (self.length & 0xFF) as u8,
            (self.length >> 8) as u8,
            (self.cid & 0xFF) as u8,
            (self.cid >> 8) as u8,
        ]
    }
}

/// L2CAP signaling command header
#[derive(Debug, Clone, Copy)]
pub struct SignalingHeader {
    pub code: u8,
    pub identifier: u8,
    pub length: u16,
}

impl SignalingHeader {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 4 {
            return None;
        }

        Some(Self {
            code: bytes[0],
            identifier: bytes[1],
            length: u16::from_le_bytes([bytes[2], bytes[3]]),
        })
    }

    pub fn to_bytes(&self) -> [u8; 4] {
        [
            self.code,
            self.identifier,
            (self.length & 0xFF) as u8,
            (self.length >> 8) as u8,
        ]
    }
}

/// L2CAP connection manager
pub struct L2capManager {
    channels: BTreeMap<u16, L2capChannel>,
    next_local_cid: u16,
    next_identifier: u8,
}

impl L2capManager {
    pub const fn new() -> Self {
        Self {
            channels: BTreeMap::new(),
            next_local_cid: cid::DYNAMIC_START,
            next_identifier: 1,
        }
    }

    /// Allocate a new local CID
    fn alloc_cid(&mut self) -> u16 {
        let cid = self.next_local_cid;
        self.next_local_cid = if self.next_local_cid >= cid::DYNAMIC_END {
            cid::DYNAMIC_START
        } else {
            self.next_local_cid + 1
        };
        cid
    }

    /// Allocate a new command identifier
    fn alloc_identifier(&mut self) -> u8 {
        let id = self.next_identifier;
        self.next_identifier = if self.next_identifier == 0xFF { 1 } else { self.next_identifier + 1 };
        id
    }

    /// Create a new channel
    pub fn create_channel(&mut self, psm: u16, handle: u16) -> u16 {
        let local_cid = self.alloc_cid();
        let channel = L2capChannel::new(local_cid, psm, handle);
        self.channels.insert(local_cid, channel);
        local_cid
    }

    /// Get channel by local CID
    pub fn get_channel(&self, cid: u16) -> Option<&L2capChannel> {
        self.channels.get(&cid)
    }

    /// Get mutable channel by local CID
    pub fn get_channel_mut(&mut self, cid: u16) -> Option<&mut L2capChannel> {
        self.channels.get_mut(&cid)
    }

    /// Remove channel
    pub fn remove_channel(&mut self, cid: u16) {
        self.channels.remove(&cid);
    }

    /// Build connection request
    pub fn build_connection_request(&mut self, psm: u16, source_cid: u16) -> Vec<u8> {
        let id = self.alloc_identifier();
        let mut data = Vec::with_capacity(8);

        // L2CAP header
        data.extend_from_slice(&L2capHeader { length: 8, cid: cid::SIGNALING }.to_bytes());

        // Signaling header
        data.extend_from_slice(&SignalingHeader {
            code: signal::CONNECTION_REQUEST,
            identifier: id,
            length: 4,
        }.to_bytes());

        // PSM and Source CID
        data.push((psm & 0xFF) as u8);
        data.push((psm >> 8) as u8);
        data.push((source_cid & 0xFF) as u8);
        data.push((source_cid >> 8) as u8);

        data
    }

    /// Build connection response
    pub fn build_connection_response(
        &self,
        id: u8,
        dest_cid: u16,
        source_cid: u16,
        result: u16,
        status: u16,
    ) -> Vec<u8> {
        let mut data = Vec::with_capacity(16);

        // L2CAP header
        data.extend_from_slice(&L2capHeader { length: 12, cid: cid::SIGNALING }.to_bytes());

        // Signaling header
        data.extend_from_slice(&SignalingHeader {
            code: signal::CONNECTION_RESPONSE,
            identifier: id,
            length: 8,
        }.to_bytes());

        // Dest CID, Source CID, Result, Status
        data.push((dest_cid & 0xFF) as u8);
        data.push((dest_cid >> 8) as u8);
        data.push((source_cid & 0xFF) as u8);
        data.push((source_cid >> 8) as u8);
        data.push((result & 0xFF) as u8);
        data.push((result >> 8) as u8);
        data.push((status & 0xFF) as u8);
        data.push((status >> 8) as u8);

        data
    }

    /// Build configuration request
    pub fn build_configuration_request(&mut self, dest_cid: u16, mtu: u16) -> Vec<u8> {
        let id = self.alloc_identifier();
        let mut data = Vec::with_capacity(16);

        // L2CAP header
        data.extend_from_slice(&L2capHeader { length: 12, cid: cid::SIGNALING }.to_bytes());

        // Signaling header
        data.extend_from_slice(&SignalingHeader {
            code: signal::CONFIGURATION_REQUEST,
            identifier: id,
            length: 8,
        }.to_bytes());

        // Dest CID and Flags
        data.push((dest_cid & 0xFF) as u8);
        data.push((dest_cid >> 8) as u8);
        data.push(0x00); // Flags low
        data.push(0x00); // Flags high

        // MTU option
        data.push(config_option::MTU);
        data.push(2); // Length
        data.push((mtu & 0xFF) as u8);
        data.push((mtu >> 8) as u8);

        data
    }

    /// Build configuration response
    pub fn build_configuration_response(
        &self,
        id: u8,
        source_cid: u16,
        result: u16,
    ) -> Vec<u8> {
        let mut data = Vec::with_capacity(14);

        // L2CAP header
        data.extend_from_slice(&L2capHeader { length: 10, cid: cid::SIGNALING }.to_bytes());

        // Signaling header
        data.extend_from_slice(&SignalingHeader {
            code: signal::CONFIGURATION_RESPONSE,
            identifier: id,
            length: 6,
        }.to_bytes());

        // Source CID, Flags, Result
        data.push((source_cid & 0xFF) as u8);
        data.push((source_cid >> 8) as u8);
        data.push(0x00); // Flags low
        data.push(0x00); // Flags high
        data.push((result & 0xFF) as u8);
        data.push((result >> 8) as u8);

        data
    }

    /// Build disconnection request
    pub fn build_disconnection_request(&mut self, dest_cid: u16, source_cid: u16) -> Vec<u8> {
        let id = self.alloc_identifier();
        let mut data = Vec::with_capacity(12);

        // L2CAP header
        data.extend_from_slice(&L2capHeader { length: 8, cid: cid::SIGNALING }.to_bytes());

        // Signaling header
        data.extend_from_slice(&SignalingHeader {
            code: signal::DISCONNECTION_REQUEST,
            identifier: id,
            length: 4,
        }.to_bytes());

        // Dest CID, Source CID
        data.push((dest_cid & 0xFF) as u8);
        data.push((dest_cid >> 8) as u8);
        data.push((source_cid & 0xFF) as u8);
        data.push((source_cid >> 8) as u8);

        data
    }

    /// Build disconnection response
    pub fn build_disconnection_response(
        &self,
        id: u8,
        dest_cid: u16,
        source_cid: u16,
    ) -> Vec<u8> {
        let mut data = Vec::with_capacity(12);

        // L2CAP header
        data.extend_from_slice(&L2capHeader { length: 8, cid: cid::SIGNALING }.to_bytes());

        // Signaling header
        data.extend_from_slice(&SignalingHeader {
            code: signal::DISCONNECTION_RESPONSE,
            identifier: id,
            length: 4,
        }.to_bytes());

        // Dest CID, Source CID
        data.push((dest_cid & 0xFF) as u8);
        data.push((dest_cid >> 8) as u8);
        data.push((source_cid & 0xFF) as u8);
        data.push((source_cid >> 8) as u8);

        data
    }

    /// Build L2CAP data packet
    pub fn build_data_packet(&self, cid: u16, data: &[u8]) -> Vec<u8> {
        let mut packet = Vec::with_capacity(4 + data.len());

        // L2CAP header
        packet.extend_from_slice(&L2capHeader {
            length: data.len() as u16,
            cid,
        }.to_bytes());

        packet.extend_from_slice(data);
        packet
    }

    /// Process incoming L2CAP packet
    pub fn process_packet(&mut self, handle: u16, data: &[u8]) -> Option<L2capEvent> {
        let header = L2capHeader::from_bytes(data)?;
        let payload = &data[4..];

        if payload.len() < header.length as usize {
            return None;
        }

        match header.cid {
            cid::SIGNALING => self.process_signaling(handle, payload),
            cid::CONNECTIONLESS => None, // Not implemented
            cid::LE_SIGNALING => self.process_le_signaling(handle, payload),
            cid if cid >= cid::DYNAMIC_START => {
                // Data for dynamic channel
                Some(L2capEvent::Data {
                    cid,
                    data: payload.to_vec(),
                })
            }
            _ => None,
        }
    }

    fn process_signaling(&mut self, handle: u16, data: &[u8]) -> Option<L2capEvent> {
        let header = SignalingHeader::from_bytes(data)?;
        let params = &data[4..];

        match header.code {
            signal::CONNECTION_REQUEST => {
                if params.len() >= 4 {
                    let psm = u16::from_le_bytes([params[0], params[1]]);
                    let source_cid = u16::from_le_bytes([params[2], params[3]]);
                    return Some(L2capEvent::ConnectionRequest {
                        id: header.identifier,
                        handle,
                        psm,
                        source_cid,
                    });
                }
            }
            signal::CONNECTION_RESPONSE => {
                if params.len() >= 8 {
                    let dest_cid = u16::from_le_bytes([params[0], params[1]]);
                    let source_cid = u16::from_le_bytes([params[2], params[3]]);
                    let result = u16::from_le_bytes([params[4], params[5]]);
                    return Some(L2capEvent::ConnectionResponse {
                        id: header.identifier,
                        dest_cid,
                        source_cid,
                        result,
                    });
                }
            }
            signal::CONFIGURATION_REQUEST => {
                if params.len() >= 4 {
                    let dest_cid = u16::from_le_bytes([params[0], params[1]]);
                    return Some(L2capEvent::ConfigurationRequest {
                        id: header.identifier,
                        dest_cid,
                        options: params[4..].to_vec(),
                    });
                }
            }
            signal::CONFIGURATION_RESPONSE => {
                if params.len() >= 6 {
                    let source_cid = u16::from_le_bytes([params[0], params[1]]);
                    let result = u16::from_le_bytes([params[4], params[5]]);
                    return Some(L2capEvent::ConfigurationResponse {
                        id: header.identifier,
                        source_cid,
                        result,
                    });
                }
            }
            signal::DISCONNECTION_REQUEST => {
                if params.len() >= 4 {
                    let dest_cid = u16::from_le_bytes([params[0], params[1]]);
                    let source_cid = u16::from_le_bytes([params[2], params[3]]);
                    return Some(L2capEvent::DisconnectionRequest {
                        id: header.identifier,
                        dest_cid,
                        source_cid,
                    });
                }
            }
            signal::DISCONNECTION_RESPONSE => {
                if params.len() >= 4 {
                    let dest_cid = u16::from_le_bytes([params[0], params[1]]);
                    let source_cid = u16::from_le_bytes([params[2], params[3]]);
                    return Some(L2capEvent::DisconnectionResponse {
                        id: header.identifier,
                        dest_cid,
                        source_cid,
                    });
                }
            }
            _ => {}
        }

        None
    }

    fn process_le_signaling(&mut self, handle: u16, data: &[u8]) -> Option<L2capEvent> {
        let header = SignalingHeader::from_bytes(data)?;
        let params = &data[4..];

        match header.code {
            signal::CONNECTION_PARAMETER_UPDATE_REQUEST => {
                if params.len() >= 8 {
                    let interval_min = u16::from_le_bytes([params[0], params[1]]);
                    let interval_max = u16::from_le_bytes([params[2], params[3]]);
                    let latency = u16::from_le_bytes([params[4], params[5]]);
                    let timeout = u16::from_le_bytes([params[6], params[7]]);
                    return Some(L2capEvent::LeConnectionParameterUpdate {
                        id: header.identifier,
                        handle,
                        interval_min,
                        interval_max,
                        latency,
                        timeout,
                    });
                }
            }
            signal::LE_CREDIT_BASED_CONNECTION_REQUEST => {
                if params.len() >= 10 {
                    let le_psm = u16::from_le_bytes([params[0], params[1]]);
                    let source_cid = u16::from_le_bytes([params[2], params[3]]);
                    let mtu = u16::from_le_bytes([params[4], params[5]]);
                    let mps = u16::from_le_bytes([params[6], params[7]]);
                    let initial_credits = u16::from_le_bytes([params[8], params[9]]);
                    return Some(L2capEvent::LeCreditConnectionRequest {
                        id: header.identifier,
                        handle,
                        le_psm,
                        source_cid,
                        mtu,
                        mps,
                        initial_credits,
                    });
                }
            }
            signal::FLOW_CONTROL_CREDIT => {
                if params.len() >= 4 {
                    let cid = u16::from_le_bytes([params[0], params[1]]);
                    let credits = u16::from_le_bytes([params[2], params[3]]);
                    return Some(L2capEvent::FlowControlCredit { cid, credits });
                }
            }
            _ => {}
        }

        None
    }
}

/// L2CAP events
#[derive(Debug)]
pub enum L2capEvent {
    ConnectionRequest {
        id: u8,
        handle: u16,
        psm: u16,
        source_cid: u16,
    },
    ConnectionResponse {
        id: u8,
        dest_cid: u16,
        source_cid: u16,
        result: u16,
    },
    ConfigurationRequest {
        id: u8,
        dest_cid: u16,
        options: Vec<u8>,
    },
    ConfigurationResponse {
        id: u8,
        source_cid: u16,
        result: u16,
    },
    DisconnectionRequest {
        id: u8,
        dest_cid: u16,
        source_cid: u16,
    },
    DisconnectionResponse {
        id: u8,
        dest_cid: u16,
        source_cid: u16,
    },
    LeConnectionParameterUpdate {
        id: u8,
        handle: u16,
        interval_min: u16,
        interval_max: u16,
        latency: u16,
        timeout: u16,
    },
    LeCreditConnectionRequest {
        id: u8,
        handle: u16,
        le_psm: u16,
        source_cid: u16,
        mtu: u16,
        mps: u16,
        initial_credits: u16,
    },
    FlowControlCredit {
        cid: u16,
        credits: u16,
    },
    Data {
        cid: u16,
        data: Vec<u8>,
    },
}
