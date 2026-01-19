//! Hyper-V VMBus
//!
//! Virtual Machine Bus for guest-host communication.

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

/// VMBus version
pub const VMBUS_VERSION_WS2008: u32 = 0x000D0000;
pub const VMBUS_VERSION_WIN7: u32 = 0x10000000;
pub const VMBUS_VERSION_WIN8: u32 = 0x20000000;
pub const VMBUS_VERSION_WIN8_1: u32 = 0x30000000;
pub const VMBUS_VERSION_WIN10: u32 = 0x40000000;

/// VMBus message types
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmbusMessageType {
    Invalid = 0,
    ChannelMessage = 1,
    TimerExpired = 2,
    None = 0xFFFFFFFF,
}

/// VMBus channel message types
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmbusChannelMessage {
    Invalid = 0,
    OfferChannel = 1,
    RescindChannelOffer = 2,
    RequestOffers = 3,
    AllOffersDelivered = 4,
    OpenChannel = 5,
    OpenChannelResult = 6,
    CloseChannel = 7,
    GpadlHeader = 8,
    GpadlBody = 9,
    GpadlCreated = 10,
    GpadlTeardown = 11,
    GpadlTorndown = 12,
    RelIdReleased = 13,
    InitiateContact = 14,
    VersionResponse = 15,
    UnloadRequest = 16,
    UnloadResponse = 17,
    TlConnectRequest = 19,
    ModifyChannel = 20,
    TlConnectResult = 21,
    ModifyChannelResponse = 22,
}

/// Channel GUID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(C)]
pub struct VmbusGuid {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

impl VmbusGuid {
    pub const fn new(data1: u32, data2: u16, data3: u16, data4: [u8; 8]) -> Self {
        Self { data1, data2, data3, data4 }
    }

    /// Check if this is the network device GUID
    pub fn is_netvsc(&self) -> bool {
        *self == NETVSC_GUID
    }

    /// Check if this is the storage device GUID
    pub fn is_storvsc(&self) -> bool {
        *self == STORVSC_GUID
    }

    /// Check if this is the time sync GUID
    pub fn is_timesync(&self) -> bool {
        *self == TIMESYNC_GUID
    }

    /// Check if this is the shutdown GUID
    pub fn is_shutdown(&self) -> bool {
        *self == SHUTDOWN_GUID
    }
}

/// Known device GUIDs
pub const NETVSC_GUID: VmbusGuid = VmbusGuid::new(
    0xf8615163, 0xdf3e, 0x46c5,
    [0x91, 0x3f, 0xf2, 0xd2, 0xf9, 0x65, 0xed, 0x0e]
);

pub const STORVSC_GUID: VmbusGuid = VmbusGuid::new(
    0xba6163d9, 0x04a1, 0x4d29,
    [0xb6, 0x05, 0x72, 0xe2, 0xff, 0xb1, 0xdc, 0x7f]
);

pub const TIMESYNC_GUID: VmbusGuid = VmbusGuid::new(
    0x9527e630, 0xd0ae, 0x497b,
    [0xad, 0xce, 0xe8, 0x0a, 0xb0, 0x17, 0x5c, 0xaf]
);

pub const SHUTDOWN_GUID: VmbusGuid = VmbusGuid::new(
    0x0e0b6031, 0x5213, 0x4934,
    [0x81, 0x8b, 0x38, 0xd9, 0x0c, 0xed, 0x39, 0xdb]
);

pub const HEARTBEAT_GUID: VmbusGuid = VmbusGuid::new(
    0x57164f39, 0x9115, 0x4e78,
    [0xab, 0x55, 0x38, 0x2f, 0x3b, 0xd5, 0x42, 0x2d]
);

pub const KVP_GUID: VmbusGuid = VmbusGuid::new(
    0xa9a0f4e7, 0x5a45, 0x4d96,
    [0xb8, 0x27, 0x8a, 0x84, 0x1e, 0x8c, 0x03, 0xe6]
);

/// Channel state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelState {
    Closed,
    OfferReceived,
    Opening,
    Open,
    Closing,
}

/// Ring buffer descriptor
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VmbusRingBuffer {
    pub write_index: u32,
    pub read_index: u32,
    pub interrupt_mask: u32,
    pub pending_send_sz: u32,
    pub reserved: [u32; 12],
    // Data follows...
}

/// Channel offer
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VmbusChannelOffer {
    pub interface_type: VmbusGuid,
    pub interface_instance: VmbusGuid,
    pub reserved1: u64,
    pub reserved2: u64,
    pub interrupt_flags: u16,
    pub mmio_megabytes: u16,
    pub user_defined: [u8; 120],
    pub sub_channel_index: u16,
    pub reserved3: u16,
}

/// VMBus channel
pub struct VmbusChannel {
    /// Channel ID
    id: u32,
    /// Offer info
    offer: VmbusChannelOffer,
    /// State
    state: ChannelState,
    /// Ring buffer (TX)
    tx_ring: Option<u64>,
    /// Ring buffer (RX)
    rx_ring: Option<u64>,
    /// Ring buffer size
    ring_size: u32,
    /// GPADL handle
    gpadl_handle: u32,
    /// Opened flag
    opened: AtomicBool,
    /// Pending packets
    pending_packets: AtomicU32,
}

impl VmbusChannel {
    /// Create new channel from offer
    pub fn new(id: u32, offer: VmbusChannelOffer) -> Self {
        Self {
            id,
            offer,
            state: ChannelState::OfferReceived,
            tx_ring: None,
            rx_ring: None,
            ring_size: 0,
            gpadl_handle: 0,
            opened: AtomicBool::new(false),
            pending_packets: AtomicU32::new(0),
        }
    }

    /// Get channel ID
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Get interface type GUID
    pub fn interface_type(&self) -> &VmbusGuid {
        &self.offer.interface_type
    }

    /// Get channel state
    pub fn state(&self) -> ChannelState {
        self.state
    }

    /// Is channel open?
    pub fn is_open(&self) -> bool {
        self.opened.load(Ordering::Acquire)
    }

    /// Set up ring buffers
    pub fn setup_ring_buffers(&mut self, ring_size: u32) -> Result<(), &'static str> {
        // Allocate ring buffer pages
        let total_size = ring_size * 2; // TX + RX
        let pages = (total_size as usize + 4095) / 4096;

        // Allocate contiguous memory
        let mut frames = Vec::new();
        for _ in 0..pages {
            if let Some(frame) = crate::mm::alloc_frame() {
                frames.push(frame);
            } else {
                // Free already allocated
                for f in frames {
                    crate::mm::free_frame(f);
                }
                return Err("Failed to allocate ring buffers");
            }
        }

        if let Some(first) = frames.first() {
            let base = first.start_address().as_u64();
            self.tx_ring = Some(base);
            self.rx_ring = Some(base + ring_size as u64);
            self.ring_size = ring_size;

            // Initialize ring buffer headers
            unsafe {
                let tx_header = base as *mut VmbusRingBuffer;
                let rx_header = (base + ring_size as u64) as *mut VmbusRingBuffer;

                (*tx_header).write_index = 0;
                (*tx_header).read_index = 0;
                (*tx_header).interrupt_mask = 0;
                (*tx_header).pending_send_sz = 0;

                (*rx_header).write_index = 0;
                (*rx_header).read_index = 0;
                (*rx_header).interrupt_mask = 0;
                (*rx_header).pending_send_sz = 0;
            }
        }

        Ok(())
    }

    /// Write to TX ring buffer
    pub fn write(&self, data: &[u8]) -> Result<(), &'static str> {
        if !self.is_open() {
            return Err("Channel not open");
        }

        let tx_ring = self.tx_ring.ok_or("No TX ring")?;

        unsafe {
            let header = tx_ring as *mut VmbusRingBuffer;
            let data_start = tx_ring + core::mem::size_of::<VmbusRingBuffer>() as u64;
            let data_size = self.ring_size - core::mem::size_of::<VmbusRingBuffer>() as u32;

            let write_idx = (*header).write_index;
            let read_idx = (*header).read_index;

            // Calculate available space
            let available = if write_idx >= read_idx {
                data_size - write_idx + read_idx
            } else {
                read_idx - write_idx
            };

            if data.len() > available as usize {
                return Err("Ring buffer full");
            }

            // Write data
            let mut dst_idx = write_idx as usize;
            for &byte in data {
                let ptr = (data_start + dst_idx as u64) as *mut u8;
                *ptr = byte;
                dst_idx = (dst_idx + 1) % data_size as usize;
            }

            // Update write index
            (*header).write_index = dst_idx as u32;
        }

        self.pending_packets.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Read from RX ring buffer
    pub fn read(&self, buffer: &mut [u8]) -> Result<usize, &'static str> {
        if !self.is_open() {
            return Err("Channel not open");
        }

        let rx_ring = self.rx_ring.ok_or("No RX ring")?;

        unsafe {
            let header = rx_ring as *mut VmbusRingBuffer;
            let data_start = rx_ring + core::mem::size_of::<VmbusRingBuffer>() as u64;
            let data_size = self.ring_size - core::mem::size_of::<VmbusRingBuffer>() as u32;

            let write_idx = (*header).write_index;
            let read_idx = (*header).read_index;

            // Calculate available data
            let available = if write_idx >= read_idx {
                write_idx - read_idx
            } else {
                data_size - read_idx + write_idx
            };

            if available == 0 {
                return Ok(0);
            }

            let to_read = buffer.len().min(available as usize);

            // Read data
            let mut src_idx = read_idx as usize;
            for i in 0..to_read {
                let ptr = (data_start + src_idx as u64) as *const u8;
                buffer[i] = *ptr;
                src_idx = (src_idx + 1) % data_size as usize;
            }

            // Update read index
            (*header).read_index = src_idx as u32;

            Ok(to_read)
        }
    }

    /// Open channel
    pub fn open(&mut self) -> Result<(), &'static str> {
        if self.state != ChannelState::OfferReceived {
            return Err("Invalid channel state");
        }

        // Set up ring buffers (64KB each)
        self.setup_ring_buffers(65536)?;

        self.state = ChannelState::Opening;

        // In real implementation, send open channel message to host
        // and wait for response

        self.state = ChannelState::Open;
        self.opened.store(true, Ordering::Release);

        Ok(())
    }

    /// Close channel
    pub fn close(&mut self) {
        self.state = ChannelState::Closing;
        self.opened.store(false, Ordering::Release);

        // Free ring buffers
        if let Some(tx) = self.tx_ring.take() {
            let frame = x86_64::structures::paging::PhysFrame::containing_address(
                x86_64::PhysAddr::new(tx)
            );
            crate::mm::free_frame(frame);
        }

        self.state = ChannelState::Closed;
    }
}

/// VMBus statistics
#[derive(Debug, Default)]
pub struct VmbusStats {
    pub messages_sent: AtomicU64,
    pub messages_received: AtomicU64,
    pub channels_opened: AtomicU64,
    pub channels_closed: AtomicU64,
    pub errors: AtomicU64,
}

/// VMBus manager
pub struct Vmbus {
    /// Negotiated version
    version: u32,
    /// Channels by ID
    channels: BTreeMap<u32, VmbusChannel>,
    /// Next channel ID
    next_channel_id: u32,
    /// Message port physical address
    msg_port: u64,
    /// Event port physical address
    event_port: u64,
    /// Initialized flag
    initialized: AtomicBool,
    /// Statistics
    stats: VmbusStats,
}

impl Vmbus {
    /// Create new VMBus
    pub fn new() -> Self {
        Self {
            version: 0,
            channels: BTreeMap::new(),
            next_channel_id: 0,
            msg_port: 0,
            event_port: 0,
            initialized: AtomicBool::new(false),
            stats: VmbusStats::default(),
        }
    }

    /// Initialize VMBus
    pub fn init(&mut self) -> Result<(), &'static str> {
        // Negotiate version
        self.version = VMBUS_VERSION_WIN10;

        // Allocate message and event pages
        if let Some(frame) = crate::mm::alloc_frame() {
            self.msg_port = frame.start_address().as_u64();
        } else {
            return Err("Failed to allocate message port");
        }

        if let Some(frame) = crate::mm::alloc_frame() {
            self.event_port = frame.start_address().as_u64();
        } else {
            // Free msg_port
            let frame = x86_64::structures::paging::PhysFrame::containing_address(
                x86_64::PhysAddr::new(self.msg_port)
            );
            crate::mm::free_frame(frame);
            return Err("Failed to allocate event port");
        }

        self.initialized.store(true, Ordering::Release);
        crate::kprintln!("vmbus: Initialized, version 0x{:08X}", self.version);
        Ok(())
    }

    /// Request channel offers from host
    pub fn request_offers(&mut self) {
        // In real implementation, send RequestOffers message
        self.stats.messages_sent.fetch_add(1, Ordering::Relaxed);
    }

    /// Handle channel offer
    pub fn handle_offer(&mut self, offer: VmbusChannelOffer) {
        let id = self.next_channel_id;
        self.next_channel_id += 1;

        let channel = VmbusChannel::new(id, offer);
        self.channels.insert(id, channel);

        self.stats.messages_received.fetch_add(1, Ordering::Relaxed);
    }

    /// Get channel by ID
    pub fn get_channel(&mut self, id: u32) -> Option<&mut VmbusChannel> {
        self.channels.get_mut(&id)
    }

    /// Find channel by interface type
    pub fn find_channel_by_type(&mut self, guid: &VmbusGuid) -> Option<&mut VmbusChannel> {
        for channel in self.channels.values_mut() {
            if channel.interface_type() == guid {
                return Some(channel);
            }
        }
        None
    }

    /// Get all channels
    pub fn channels(&self) -> impl Iterator<Item = &VmbusChannel> {
        self.channels.values()
    }

    /// Get channel count
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }

    /// Get negotiated version
    pub fn version(&self) -> u32 {
        self.version
    }

    /// Get statistics
    pub fn stats(&self) -> &VmbusStats {
        &self.stats
    }

    /// Format status
    pub fn format_status(&self) -> String {
        alloc::format!(
            "VMBus: version=0x{:08X} channels={} msgs_sent={} msgs_recv={}",
            self.version, self.channels.len(),
            self.stats.messages_sent.load(Ordering::Relaxed),
            self.stats.messages_received.load(Ordering::Relaxed)
        )
    }
}

impl Default for Vmbus {
    fn default() -> Self {
        Self::new()
    }
}

/// Global VMBus instance
static VMBUS: crate::sync::IrqSafeMutex<Option<Vmbus>> = crate::sync::IrqSafeMutex::new(None);

/// Initialize VMBus
pub fn init() -> Result<(), &'static str> {
    let mut vmbus = Vmbus::new();
    vmbus.init()?;
    *VMBUS.lock() = Some(vmbus);
    Ok(())
}

/// Get VMBus status
pub fn status() -> String {
    VMBUS.lock()
        .as_ref()
        .map(|v| v.format_status())
        .unwrap_or_else(|| "VMBus not initialized".into())
}
