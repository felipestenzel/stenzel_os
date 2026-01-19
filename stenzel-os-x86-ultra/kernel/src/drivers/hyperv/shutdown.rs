//! Hyper-V Shutdown Integration
//!
//! Guest shutdown/restart integration for Hyper-V.

#![allow(dead_code)]

use alloc::string::String;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use super::vmbus::VmbusChannel;

/// Shutdown message versions
pub const SHUTDOWN_VERSION_1: u32 = 1;

/// Shutdown message types
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownMsgType {
    Negotiate = 0,
    NegotiateComplete = 1,
    Shutdown = 2,
    ShutdownComplete = 3,
}

/// Shutdown flags
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownFlags {
    None = 0,
    HibernateFlag = 1,
    RestartFlag = 2,
}

/// Shutdown state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownState {
    Ready,
    ShutdownRequested,
    RestartRequested,
    HibernateRequested,
    Processing,
}

/// Shutdown message header
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ShutdownMsgHeader {
    pub msg_type: u32,
    pub size: u32,
}

/// Shutdown negotiate message
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ShutdownNegotiate {
    pub header: ShutdownMsgHeader,
    pub version_requested: u32,
    pub version_granted: u32,
}

/// Shutdown request message
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ShutdownRequest {
    pub header: ShutdownMsgHeader,
    pub reason_code: u32,
    pub timeout_seconds: u32,
    pub flags: u32,
    pub display_message_offset: u32,
    pub display_message_length: u32,
}

/// Shutdown complete message
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ShutdownComplete {
    pub header: ShutdownMsgHeader,
    pub result: u32,
}

/// Shutdown service
pub struct ShutdownService {
    /// VMBus channel ID
    channel_id: u32,
    /// Protocol version
    version: u32,
    /// Current state
    state: ShutdownState,
    /// Shutdown requested flag
    shutdown_requested: AtomicBool,
    /// Restart requested flag
    restart_requested: AtomicBool,
    /// Reason code
    reason_code: AtomicU32,
    /// Initialized flag
    initialized: AtomicBool,
}

impl ShutdownService {
    /// Create new shutdown service
    pub fn new(channel_id: u32) -> Self {
        Self {
            channel_id,
            version: 0,
            state: ShutdownState::Ready,
            shutdown_requested: AtomicBool::new(false),
            restart_requested: AtomicBool::new(false),
            reason_code: AtomicU32::new(0),
            initialized: AtomicBool::new(false),
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
        crate::kprintln!("hyperv-shutdown: Initialized");

        Ok(())
    }

    /// Negotiate protocol version
    fn negotiate_version(&mut self, channel: &mut VmbusChannel) -> Result<(), &'static str> {
        let msg = ShutdownNegotiate {
            header: ShutdownMsgHeader {
                msg_type: ShutdownMsgType::Negotiate as u32,
                size: core::mem::size_of::<ShutdownNegotiate>() as u32,
            },
            version_requested: SHUTDOWN_VERSION_1,
            version_granted: 0,
        };

        let bytes = unsafe {
            core::slice::from_raw_parts(
                &msg as *const _ as *const u8,
                core::mem::size_of::<ShutdownNegotiate>()
            )
        };

        channel.write(bytes)?;

        // Would wait for NegotiateComplete response
        self.version = SHUTDOWN_VERSION_1;

        Ok(())
    }

    /// Process incoming shutdown message
    pub fn process_message(&mut self, channel: &VmbusChannel) -> Result<(), &'static str> {
        if !self.initialized.load(Ordering::Acquire) {
            return Err("Service not initialized");
        }

        let mut buffer = [0u8; 256];
        let read = channel.read(&mut buffer)?;

        if read < core::mem::size_of::<ShutdownMsgHeader>() {
            return Ok(());
        }

        let header = unsafe {
            &*(buffer.as_ptr() as *const ShutdownMsgHeader)
        };

        if header.msg_type == ShutdownMsgType::Shutdown as u32 {
            self.handle_shutdown_request(&buffer[..read])?;
        }

        Ok(())
    }

    /// Handle shutdown request
    fn handle_shutdown_request(&mut self, data: &[u8]) -> Result<(), &'static str> {
        if data.len() < core::mem::size_of::<ShutdownRequest>() {
            return Err("Request too short");
        }

        let request = unsafe {
            &*(data.as_ptr() as *const ShutdownRequest)
        };

        self.reason_code.store(request.reason_code, Ordering::Relaxed);

        if request.flags & ShutdownFlags::RestartFlag as u32 != 0 {
            self.state = ShutdownState::RestartRequested;
            self.restart_requested.store(true, Ordering::Release);
            crate::kprintln!("hyperv-shutdown: Restart requested by host");
        } else if request.flags & ShutdownFlags::HibernateFlag as u32 != 0 {
            self.state = ShutdownState::HibernateRequested;
            crate::kprintln!("hyperv-shutdown: Hibernate requested by host");
        } else {
            self.state = ShutdownState::ShutdownRequested;
            self.shutdown_requested.store(true, Ordering::Release);
            crate::kprintln!("hyperv-shutdown: Shutdown requested by host");
        }

        Ok(())
    }

    /// Send shutdown complete response
    pub fn send_complete(&self, channel: &mut VmbusChannel, success: bool) -> Result<(), &'static str> {
        let msg = ShutdownComplete {
            header: ShutdownMsgHeader {
                msg_type: ShutdownMsgType::ShutdownComplete as u32,
                size: core::mem::size_of::<ShutdownComplete>() as u32,
            },
            result: if success { 0 } else { 1 },
        };

        let bytes = unsafe {
            core::slice::from_raw_parts(
                &msg as *const _ as *const u8,
                core::mem::size_of::<ShutdownComplete>()
            )
        };

        channel.write(bytes)
    }

    /// Check if shutdown requested
    pub fn is_shutdown_requested(&self) -> bool {
        self.shutdown_requested.load(Ordering::Acquire)
    }

    /// Check if restart requested
    pub fn is_restart_requested(&self) -> bool {
        self.restart_requested.load(Ordering::Acquire)
    }

    /// Get current state
    pub fn state(&self) -> ShutdownState {
        self.state
    }

    /// Get reason code
    pub fn reason_code(&self) -> u32 {
        self.reason_code.load(Ordering::Relaxed)
    }

    /// Clear shutdown request (after handling)
    pub fn clear_request(&mut self) {
        self.shutdown_requested.store(false, Ordering::Release);
        self.restart_requested.store(false, Ordering::Release);
        self.state = ShutdownState::Ready;
    }

    /// Format status
    pub fn format_status(&self) -> String {
        alloc::format!(
            "Hyper-V Shutdown: state={:?} shutdown={} restart={}",
            self.state,
            self.is_shutdown_requested(),
            self.is_restart_requested()
        )
    }
}

impl Default for ShutdownService {
    fn default() -> Self {
        Self::new(0)
    }
}

// Global shutdown service
static SHUTDOWN: crate::sync::IrqSafeMutex<Option<ShutdownService>> =
    crate::sync::IrqSafeMutex::new(None);

/// Initialize shutdown service
pub fn init(channel_id: u32, channel: &mut VmbusChannel) -> Result<(), &'static str> {
    let mut service = ShutdownService::new(channel_id);
    service.init(channel)?;
    *SHUTDOWN.lock() = Some(service);
    Ok(())
}

/// Check if shutdown requested
pub fn is_shutdown_requested() -> bool {
    SHUTDOWN.lock()
        .as_ref()
        .map(|s| s.is_shutdown_requested())
        .unwrap_or(false)
}

/// Check if restart requested
pub fn is_restart_requested() -> bool {
    SHUTDOWN.lock()
        .as_ref()
        .map(|s| s.is_restart_requested())
        .unwrap_or(false)
}

/// Get status
pub fn status() -> String {
    SHUTDOWN.lock()
        .as_ref()
        .map(|s| s.format_status())
        .unwrap_or_else(|| "Shutdown service not initialized".into())
}

/// Handle pending shutdown/restart
pub fn handle_pending() {
    if is_shutdown_requested() {
        crate::kprintln!("hyperv-shutdown: Executing host-requested shutdown");
        // Would trigger system shutdown
    } else if is_restart_requested() {
        crate::kprintln!("hyperv-shutdown: Executing host-requested restart");
        // Would trigger system restart
    }
}
