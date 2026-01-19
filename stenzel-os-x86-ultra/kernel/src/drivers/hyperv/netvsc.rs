//! Hyper-V NetVSC (Network Virtual Service Client)
//!
//! Synthetic network adapter for Hyper-V guests.

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use super::vmbus::{VmbusChannel, NETVSC_GUID};

/// NetVSC protocol versions
pub const NETVSC_PROTOCOL_VERSION_1: u32 = 0x00000001;
pub const NETVSC_PROTOCOL_VERSION_4: u32 = 0x00000004;
pub const NETVSC_PROTOCOL_VERSION_5: u32 = 0x00000005;
pub const NETVSC_PROTOCOL_VERSION_6: u32 = 0x00000006;

/// NetVSC message types
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetvscMsgType {
    None = 0,
    Init = 1,
    InitComplete = 2,
    ProtocolVersion = 100,
    ProtocolVersionResponse = 101,
    SendNdisConfig = 102,
    SendRndisPacket = 107,
    SendRndisPacketComplete = 108,
}

/// RNDIS message types
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RndisMessageType {
    PacketMsg = 0x00000001,
    InitMsg = 0x00000002,
    HaltMsg = 0x00000003,
    QueryMsg = 0x00000004,
    SetMsg = 0x00000005,
    ResetMsg = 0x00000006,
    IndicateStatusMsg = 0x00000007,
    KeepaliveMsg = 0x00000008,
    InitComplete = 0x80000002,
    QueryComplete = 0x80000004,
    SetComplete = 0x80000005,
    ResetComplete = 0x80000006,
    KeepaliveComplete = 0x80000008,
}

/// RNDIS OIDs
pub mod oid {
    pub const GEN_SUPPORTED_LIST: u32 = 0x00010101;
    pub const GEN_HARDWARE_STATUS: u32 = 0x00010102;
    pub const GEN_MEDIA_SUPPORTED: u32 = 0x00010103;
    pub const GEN_MEDIA_IN_USE: u32 = 0x00010104;
    pub const GEN_MAXIMUM_FRAME_SIZE: u32 = 0x00010106;
    pub const GEN_LINK_SPEED: u32 = 0x00010107;
    pub const GEN_TRANSMIT_BUFFER_SPACE: u32 = 0x00010108;
    pub const GEN_RECEIVE_BUFFER_SPACE: u32 = 0x00010109;
    pub const GEN_TRANSMIT_BLOCK_SIZE: u32 = 0x0001010A;
    pub const GEN_RECEIVE_BLOCK_SIZE: u32 = 0x0001010B;
    pub const GEN_VENDOR_ID: u32 = 0x0001010C;
    pub const GEN_VENDOR_DESCRIPTION: u32 = 0x0001010D;
    pub const GEN_CURRENT_PACKET_FILTER: u32 = 0x0001010E;
    pub const GEN_CURRENT_LOOKAHEAD: u32 = 0x0001010F;
    pub const GEN_DRIVER_VERSION: u32 = 0x00010110;
    pub const GEN_MAXIMUM_TOTAL_SIZE: u32 = 0x00010111;
    pub const GEN_PROTOCOL_OPTIONS: u32 = 0x00010112;
    pub const GEN_MAC_OPTIONS: u32 = 0x00010113;
    pub const GEN_MEDIA_CONNECT_STATUS: u32 = 0x00010114;
    pub const GEN_MAXIMUM_SEND_PACKETS: u32 = 0x00010115;
    pub const _802_3_PERMANENT_ADDRESS: u32 = 0x01010101;
    pub const _802_3_CURRENT_ADDRESS: u32 = 0x01010102;
    pub const _802_3_MAXIMUM_LIST_SIZE: u32 = 0x01010104;
}

/// NetVSC init message
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NetvscInitMsg {
    pub msg_type: u32,
    pub padding: u32,
    pub min_version: u32,
    pub max_version: u32,
}

/// NetVSC init complete
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NetvscInitComplete {
    pub msg_type: u32,
    pub padding: u32,
    pub negotiated_version: u32,
    pub status: u32,
}

/// RNDIS init message
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct RndisInitMsg {
    pub msg_type: u32,
    pub msg_len: u32,
    pub request_id: u32,
    pub major_version: u32,
    pub minor_version: u32,
    pub max_transfer_size: u32,
}

/// RNDIS packet header
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct RndisPacketHeader {
    pub msg_type: u32,
    pub msg_len: u32,
    pub data_offset: u32,
    pub data_len: u32,
    pub oob_data_offset: u32,
    pub oob_data_len: u32,
    pub num_oob_elements: u32,
    pub per_packet_info_offset: u32,
    pub per_packet_info_len: u32,
    pub reserved: [u32; 2],
}

/// Link state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkState {
    Down,
    Up,
    Unknown,
}

/// NetVSC statistics
#[derive(Debug, Default)]
pub struct NetvscStats {
    pub tx_packets: AtomicU64,
    pub rx_packets: AtomicU64,
    pub tx_bytes: AtomicU64,
    pub rx_bytes: AtomicU64,
    pub tx_errors: AtomicU64,
    pub rx_errors: AtomicU64,
}

/// NetVSC device
pub struct NetvscDevice {
    /// VMBus channel (not owned)
    channel_id: u32,
    /// MAC address
    mac: [u8; 6],
    /// Link state
    link_state: LinkState,
    /// MTU
    mtu: u16,
    /// Max frame size
    max_frame_size: u32,
    /// Link speed (bps)
    link_speed: u64,
    /// Protocol version
    protocol_version: u32,
    /// RNDIS initialized
    rndis_initialized: bool,
    /// Request ID counter
    request_id: u32,
    /// Initialized flag
    initialized: AtomicBool,
    /// Statistics
    stats: NetvscStats,
    /// Receive buffer
    recv_buffer: Vec<u8>,
    /// Send buffer
    send_buffer: Vec<u8>,
}

impl NetvscDevice {
    /// Default MTU
    pub const DEFAULT_MTU: u16 = 1500;
    /// Max frame size
    pub const MAX_FRAME_SIZE: u32 = 65536;

    /// Create new device
    pub fn new(channel_id: u32) -> Self {
        Self {
            channel_id,
            mac: [0; 6],
            link_state: LinkState::Unknown,
            mtu: Self::DEFAULT_MTU,
            max_frame_size: Self::MAX_FRAME_SIZE,
            link_speed: 10_000_000_000, // 10 Gbps default
            protocol_version: 0,
            rndis_initialized: false,
            request_id: 0,
            initialized: AtomicBool::new(false),
            stats: NetvscStats::default(),
            recv_buffer: vec![0u8; Self::MAX_FRAME_SIZE as usize],
            send_buffer: vec![0u8; Self::MAX_FRAME_SIZE as usize],
        }
    }

    /// Get next request ID
    fn next_request_id(&mut self) -> u32 {
        self.request_id += 1;
        self.request_id
    }

    /// Initialize device with channel
    pub fn init(&mut self, channel: &mut VmbusChannel) -> Result<(), &'static str> {
        // Open channel if not already open
        if !channel.is_open() {
            channel.open()?;
        }

        // Negotiate protocol version
        let init_msg = NetvscInitMsg {
            msg_type: NetvscMsgType::Init as u32,
            padding: 0,
            min_version: NETVSC_PROTOCOL_VERSION_1,
            max_version: NETVSC_PROTOCOL_VERSION_6,
        };

        // Send init message
        let msg_bytes = unsafe {
            core::slice::from_raw_parts(
                &init_msg as *const _ as *const u8,
                core::mem::size_of::<NetvscInitMsg>()
            )
        };
        channel.write(msg_bytes)?;

        // Wait for response (simulated)
        self.protocol_version = NETVSC_PROTOCOL_VERSION_6;

        // Initialize RNDIS
        self.init_rndis(channel)?;

        // Query MAC address
        self.query_mac_address(channel)?;

        // Query link speed
        self.query_link_speed(channel)?;

        self.link_state = LinkState::Up;
        self.initialized.store(true, Ordering::Release);

        crate::kprintln!("netvsc: Initialized, MAC={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.mac[0], self.mac[1], self.mac[2],
            self.mac[3], self.mac[4], self.mac[5]);

        Ok(())
    }

    /// Initialize RNDIS
    fn init_rndis(&mut self, channel: &mut VmbusChannel) -> Result<(), &'static str> {
        let request_id = self.next_request_id();

        let rndis_init = RndisInitMsg {
            msg_type: RndisMessageType::InitMsg as u32,
            msg_len: core::mem::size_of::<RndisInitMsg>() as u32,
            request_id,
            major_version: 1,
            minor_version: 0,
            max_transfer_size: Self::MAX_FRAME_SIZE,
        };

        // Wrap in RNDIS packet
        let msg_bytes = unsafe {
            core::slice::from_raw_parts(
                &rndis_init as *const _ as *const u8,
                core::mem::size_of::<RndisInitMsg>()
            )
        };

        channel.write(msg_bytes)?;

        // Would wait for InitComplete response
        self.rndis_initialized = true;

        Ok(())
    }

    /// Query MAC address via RNDIS
    fn query_mac_address(&mut self, _channel: &mut VmbusChannel) -> Result<(), &'static str> {
        // In real implementation, send RNDIS query for OID_802_3_CURRENT_ADDRESS
        // For now, generate a MAC address
        self.mac = [0x00, 0x15, 0x5D, 0x12, 0x34, 0x56]; // Hyper-V MAC prefix
        Ok(())
    }

    /// Query link speed
    fn query_link_speed(&mut self, _channel: &mut VmbusChannel) -> Result<(), &'static str> {
        // In real implementation, send RNDIS query for OID_GEN_LINK_SPEED
        self.link_speed = 10_000_000_000; // 10 Gbps
        Ok(())
    }

    /// Get MAC address
    pub fn mac(&self) -> &[u8; 6] {
        &self.mac
    }

    /// Get link state
    pub fn link_state(&self) -> LinkState {
        self.link_state
    }

    /// Get MTU
    pub fn mtu(&self) -> u16 {
        self.mtu
    }

    /// Get link speed (bps)
    pub fn link_speed(&self) -> u64 {
        self.link_speed
    }

    /// Transmit packet
    pub fn transmit(&mut self, channel: &mut VmbusChannel, data: &[u8]) -> Result<(), &'static str> {
        if !self.initialized.load(Ordering::Acquire) {
            return Err("Device not initialized");
        }

        if self.link_state != LinkState::Up {
            return Err("Link is down");
        }

        if data.len() > self.mtu as usize + 14 {
            return Err("Packet too large");
        }

        // Build RNDIS packet
        let header = RndisPacketHeader {
            msg_type: RndisMessageType::PacketMsg as u32,
            msg_len: (core::mem::size_of::<RndisPacketHeader>() + data.len()) as u32,
            data_offset: core::mem::size_of::<RndisPacketHeader>() as u32 - 8,
            data_len: data.len() as u32,
            oob_data_offset: 0,
            oob_data_len: 0,
            num_oob_elements: 0,
            per_packet_info_offset: 0,
            per_packet_info_len: 0,
            reserved: [0; 2],
        };

        // Copy header to send buffer
        let header_bytes = unsafe {
            core::slice::from_raw_parts(
                &header as *const _ as *const u8,
                core::mem::size_of::<RndisPacketHeader>()
            )
        };

        self.send_buffer[..header_bytes.len()].copy_from_slice(header_bytes);
        self.send_buffer[header_bytes.len()..header_bytes.len() + data.len()].copy_from_slice(data);

        let total_len = header_bytes.len() + data.len();
        channel.write(&self.send_buffer[..total_len])?;

        self.stats.tx_packets.fetch_add(1, Ordering::Relaxed);
        self.stats.tx_bytes.fetch_add(data.len() as u64, Ordering::Relaxed);

        Ok(())
    }

    /// Receive packet
    pub fn receive(&mut self, channel: &VmbusChannel) -> Option<Vec<u8>> {
        if !self.initialized.load(Ordering::Acquire) {
            return None;
        }

        let mut buffer = vec![0u8; Self::MAX_FRAME_SIZE as usize];
        let read = channel.read(&mut buffer).ok()?;

        if read < core::mem::size_of::<RndisPacketHeader>() {
            return None;
        }

        // Parse RNDIS header
        let header = unsafe {
            &*(buffer.as_ptr() as *const RndisPacketHeader)
        };

        if header.msg_type != RndisMessageType::PacketMsg as u32 {
            return None;
        }

        let data_start = header.data_offset as usize + 8;
        let data_len = header.data_len as usize;

        if data_start + data_len > read {
            self.stats.rx_errors.fetch_add(1, Ordering::Relaxed);
            return None;
        }

        let data = buffer[data_start..data_start + data_len].to_vec();

        self.stats.rx_packets.fetch_add(1, Ordering::Relaxed);
        self.stats.rx_bytes.fetch_add(data_len as u64, Ordering::Relaxed);

        Some(data)
    }

    /// Get statistics
    pub fn stats(&self) -> &NetvscStats {
        &self.stats
    }

    /// Format status
    pub fn format_status(&self) -> String {
        alloc::format!(
            "NetVSC: MAC={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} Link={:?} Speed={}Mbps",
            self.mac[0], self.mac[1], self.mac[2],
            self.mac[3], self.mac[4], self.mac[5],
            self.link_state, self.link_speed / 1_000_000
        )
    }
}

impl Default for NetvscDevice {
    fn default() -> Self {
        Self::new(0)
    }
}

/// NetVSC device manager
pub struct NetvscManager {
    devices: Vec<NetvscDevice>,
}

impl NetvscManager {
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
        }
    }

    pub fn add_device(&mut self, device: NetvscDevice) -> usize {
        let idx = self.devices.len();
        self.devices.push(device);
        idx
    }

    pub fn get_device(&mut self, idx: usize) -> Option<&mut NetvscDevice> {
        self.devices.get_mut(idx)
    }

    pub fn device_count(&self) -> usize {
        self.devices.len()
    }
}

impl Default for NetvscManager {
    fn default() -> Self {
        Self::new()
    }
}
