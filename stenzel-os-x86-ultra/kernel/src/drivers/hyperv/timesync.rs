//! Hyper-V Time Synchronization
//!
//! Guest time synchronization service for Hyper-V.

#![allow(dead_code)]

use alloc::string::String;
use core::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};

use super::vmbus::{VmbusChannel, TIMESYNC_GUID};

/// Time sync message versions
pub const TIMESYNC_VERSION_1: u32 = 1;
pub const TIMESYNC_VERSION_3: u32 = 3;
pub const TIMESYNC_VERSION_4: u32 = 4;

/// Time sync message types
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeSyncMsgType {
    Negotiate = 0,
    NegotiateComplete = 1,
    Sample = 2,
    Sync = 3,
}

/// Time sync message header
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct TimeSyncMsgHeader {
    pub msg_type: u32,
    pub size: u32,
}

/// Time sync negotiate message
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct TimeSyncNegotiate {
    pub header: TimeSyncMsgHeader,
    pub version_requested: u32,
    pub version_granted: u32,
}

/// Time sample (version 1)
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct TimeSyncSampleV1 {
    pub header: TimeSyncMsgHeader,
    pub parent_time: u64,
    pub local_time: u64,
    pub sequence: u64,
}

/// Time sample (version 4)
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct TimeSyncSampleV4 {
    pub header: TimeSyncMsgHeader,
    pub reference_time: u64,
    pub guest_time: u64,
    pub flags: u32,
    pub leap_second: u8,
    pub reserved: [u8; 3],
}

/// Time sync flags
pub mod flags {
    pub const SYNC: u32 = 1 << 0;
    pub const SAMPLE: u32 = 1 << 1;
    pub const LEAP_SECOND: u32 = 1 << 2;
}

/// Time sync statistics
#[derive(Debug, Default)]
pub struct TimeSyncStats {
    pub samples_received: AtomicU64,
    pub syncs_applied: AtomicU64,
    pub last_sync_time: AtomicU64,
    pub total_adjustment_ns: AtomicI64,
}

/// Time sync service
pub struct TimeSyncService {
    /// VMBus channel ID
    channel_id: u32,
    /// Protocol version
    version: u32,
    /// Host time reference
    host_time_ref: AtomicU64,
    /// Time offset (ns)
    time_offset_ns: AtomicI64,
    /// Last sample sequence
    last_sequence: AtomicU64,
    /// Initialized flag
    initialized: AtomicBool,
    /// Statistics
    stats: TimeSyncStats,
}

impl TimeSyncService {
    /// Create new time sync service
    pub fn new(channel_id: u32) -> Self {
        Self {
            channel_id,
            version: 0,
            host_time_ref: AtomicU64::new(0),
            time_offset_ns: AtomicI64::new(0),
            last_sequence: AtomicU64::new(0),
            initialized: AtomicBool::new(false),
            stats: TimeSyncStats::default(),
        }
    }

    /// Initialize service
    pub fn init(&mut self, channel: &mut VmbusChannel) -> Result<(), &'static str> {
        // Open channel if needed
        if !channel.is_open() {
            channel.open()?;
        }

        // Negotiate version
        self.negotiate_version(channel)?;

        self.initialized.store(true, Ordering::Release);
        crate::kprintln!("hyperv-timesync: Initialized, version {}", self.version);

        Ok(())
    }

    /// Negotiate protocol version
    fn negotiate_version(&mut self, channel: &mut VmbusChannel) -> Result<(), &'static str> {
        let msg = TimeSyncNegotiate {
            header: TimeSyncMsgHeader {
                msg_type: TimeSyncMsgType::Negotiate as u32,
                size: core::mem::size_of::<TimeSyncNegotiate>() as u32,
            },
            version_requested: TIMESYNC_VERSION_4,
            version_granted: 0,
        };

        let bytes = unsafe {
            core::slice::from_raw_parts(
                &msg as *const _ as *const u8,
                core::mem::size_of::<TimeSyncNegotiate>()
            )
        };

        channel.write(bytes)?;

        // Would wait for NegotiateComplete response
        self.version = TIMESYNC_VERSION_4;

        Ok(())
    }

    /// Process incoming time sync message
    pub fn process_message(&mut self, channel: &VmbusChannel) -> Result<(), &'static str> {
        if !self.initialized.load(Ordering::Acquire) {
            return Err("Service not initialized");
        }

        let mut buffer = [0u8; 128];
        let read = channel.read(&mut buffer)?;

        if read < core::mem::size_of::<TimeSyncMsgHeader>() {
            return Ok(());
        }

        let header = unsafe {
            &*(buffer.as_ptr() as *const TimeSyncMsgHeader)
        };

        match header.msg_type {
            t if t == TimeSyncMsgType::Sample as u32 => {
                self.handle_sample(&buffer[..read])?;
            }
            t if t == TimeSyncMsgType::Sync as u32 => {
                self.handle_sync(&buffer[..read])?;
            }
            _ => {}
        }

        Ok(())
    }

    /// Handle time sample
    fn handle_sample(&mut self, data: &[u8]) -> Result<(), &'static str> {
        if self.version >= TIMESYNC_VERSION_4 {
            if data.len() < core::mem::size_of::<TimeSyncSampleV4>() {
                return Err("Sample too short");
            }

            let sample = unsafe {
                &*(data.as_ptr() as *const TimeSyncSampleV4)
            };

            // Calculate offset
            let host_time = sample.reference_time;
            let guest_time = sample.guest_time;
            let offset = host_time as i64 - guest_time as i64;

            self.time_offset_ns.store(offset * 100, Ordering::Relaxed); // Convert 100ns to ns
            self.host_time_ref.store(host_time, Ordering::Relaxed);

            if sample.flags & flags::SYNC != 0 {
                self.apply_time_correction(offset * 100);
            }
        } else {
            if data.len() < core::mem::size_of::<TimeSyncSampleV1>() {
                return Err("Sample too short");
            }

            let sample = unsafe {
                &*(data.as_ptr() as *const TimeSyncSampleV1)
            };

            let offset = sample.parent_time as i64 - sample.local_time as i64;
            self.time_offset_ns.store(offset * 100, Ordering::Relaxed);
            self.last_sequence.store(sample.sequence, Ordering::Relaxed);
        }

        self.stats.samples_received.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }

    /// Handle time sync
    fn handle_sync(&mut self, _data: &[u8]) -> Result<(), &'static str> {
        // Sync request means immediate time correction needed
        let offset = self.time_offset_ns.load(Ordering::Relaxed);
        self.apply_time_correction(offset);
        Ok(())
    }

    /// Apply time correction
    fn apply_time_correction(&self, offset_ns: i64) {
        // In real implementation, adjust system time
        // For now, just record it
        self.stats.syncs_applied.fetch_add(1, Ordering::Relaxed);
        self.stats.total_adjustment_ns.fetch_add(offset_ns, Ordering::Relaxed);
        self.stats.last_sync_time.store(
            crate::time::ticks() as u64,
            Ordering::Relaxed
        );
    }

    /// Get current time offset (ns)
    pub fn get_time_offset(&self) -> i64 {
        self.time_offset_ns.load(Ordering::Relaxed)
    }

    /// Get host time reference
    pub fn get_host_time(&self) -> u64 {
        self.host_time_ref.load(Ordering::Relaxed)
    }

    /// Get adjusted time (100ns units)
    pub fn get_adjusted_time(&self) -> u64 {
        let base = super::get_time_ref_count();
        let offset = self.time_offset_ns.load(Ordering::Relaxed) / 100;
        (base as i64 + offset) as u64
    }

    /// Get statistics
    pub fn stats(&self) -> &TimeSyncStats {
        &self.stats
    }

    /// Format status
    pub fn format_status(&self) -> String {
        let offset_ms = self.time_offset_ns.load(Ordering::Relaxed) / 1_000_000;
        alloc::format!(
            "Hyper-V TimeSync: version={} offset={}ms syncs={}",
            self.version, offset_ms,
            self.stats.syncs_applied.load(Ordering::Relaxed)
        )
    }
}

impl Default for TimeSyncService {
    fn default() -> Self {
        Self::new(0)
    }
}

// Global time sync service
static TIMESYNC: crate::sync::IrqSafeMutex<Option<TimeSyncService>> =
    crate::sync::IrqSafeMutex::new(None);

/// Initialize time sync service
pub fn init(channel_id: u32, channel: &mut VmbusChannel) -> Result<(), &'static str> {
    let mut service = TimeSyncService::new(channel_id);
    service.init(channel)?;
    *TIMESYNC.lock() = Some(service);
    Ok(())
}

/// Get time offset
pub fn get_time_offset() -> i64 {
    TIMESYNC.lock()
        .as_ref()
        .map(|s| s.get_time_offset())
        .unwrap_or(0)
}

/// Get adjusted time
pub fn get_adjusted_time() -> u64 {
    TIMESYNC.lock()
        .as_ref()
        .map(|s| s.get_adjusted_time())
        .unwrap_or(0)
}

/// Get status
pub fn status() -> String {
    TIMESYNC.lock()
        .as_ref()
        .map(|s| s.format_status())
        .unwrap_or_else(|| "TimeSync not initialized".into())
}
