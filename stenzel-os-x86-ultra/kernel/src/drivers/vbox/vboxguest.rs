//! VBoxGuest Driver
//!
//! VMMDev PCI device communication for VirtualBox.

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use super::{VmmDevRequestHeader, VmmDevRequestType};

/// Event types from host
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VboxEvent {
    MousePositionChanged = 1 << 0,
    MouseCapabilityChanged = 1 << 1,
    DisplayChange = 1 << 2,
    JudgeCredentials = 1 << 3,
    CredentialsResult = 1 << 4,
    Restore = 1 << 5,
    SeamlessChange = 1 << 6,
    MemoryBalloon = 1 << 7,
    StatisticsInterval = 1 << 8,
    VRdp = 1 << 9,
    ClipboardData = 1 << 10,
}

/// Event filter mask request
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VmmDevCtlGuestFilterMask {
    pub header: VmmDevRequestHeader,
    pub or_mask: u32,
    pub not_mask: u32,
}

/// Guest session request
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct VmmDevGetSessionId {
    pub header: VmmDevRequestHeader,
    pub session_id: u64,
}

/// IRQ acknowledge request
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct VmmDevAcknowledgeEvents {
    pub header: VmmDevRequestHeader,
    pub events: u32,
}

/// VBoxGuest session
pub struct VboxSession {
    /// Session ID
    id: u64,
    /// Event mask
    event_mask: u32,
    /// Pending events
    pending_events: AtomicU32,
    /// Active flag
    active: AtomicBool,
}

impl VboxSession {
    pub fn new(id: u64) -> Self {
        Self {
            id,
            event_mask: 0,
            pending_events: AtomicU32::new(0),
            active: AtomicBool::new(true),
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn set_event_mask(&mut self, mask: u32) {
        self.event_mask = mask;
    }

    pub fn pending_events(&self) -> u32 {
        self.pending_events.load(Ordering::Acquire)
    }

    pub fn add_event(&self, event: u32) {
        self.pending_events.fetch_or(event, Ordering::Release);
    }

    pub fn clear_event(&self, event: u32) {
        self.pending_events.fetch_and(!event, Ordering::Release);
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    pub fn close(&self) {
        self.active.store(false, Ordering::Release);
    }
}

/// VBoxGuest statistics
#[derive(Debug, Default)]
pub struct VboxGuestStats {
    pub irq_count: AtomicU64,
    pub events_received: AtomicU64,
    pub events_dispatched: AtomicU64,
    pub sessions_opened: AtomicU64,
    pub sessions_closed: AtomicU64,
}

/// VBoxGuest device driver
pub struct VboxGuestDriver {
    /// MMIO base address
    mmio_base: u64,
    /// I/O port base
    io_port: u16,
    /// IRQ
    irq: u8,
    /// Sessions
    sessions: Vec<VboxSession>,
    /// Next session ID
    next_session_id: u64,
    /// Global event mask
    global_event_mask: u32,
    /// Initialized flag
    initialized: AtomicBool,
    /// Statistics
    stats: VboxGuestStats,
}

impl VboxGuestDriver {
    /// Create new driver
    pub fn new(mmio_base: u64, io_port: u16, irq: u8) -> Self {
        Self {
            mmio_base,
            io_port,
            irq,
            sessions: Vec::new(),
            next_session_id: 1,
            global_event_mask: 0,
            initialized: AtomicBool::new(false),
            stats: VboxGuestStats::default(),
        }
    }

    /// Initialize driver
    pub fn init(&mut self) -> Result<(), &'static str> {
        // Enable all events by default
        self.set_event_filter(
            VboxEvent::MousePositionChanged as u32 |
            VboxEvent::DisplayChange as u32 |
            VboxEvent::SeamlessChange as u32 |
            VboxEvent::ClipboardData as u32,
            0
        )?;

        self.initialized.store(true, Ordering::Release);
        crate::kprintln!("vboxguest: Driver initialized");
        Ok(())
    }

    /// Set event filter
    fn set_event_filter(&mut self, or_mask: u32, not_mask: u32) -> Result<(), &'static str> {
        let request = VmmDevCtlGuestFilterMask {
            header: VmmDevRequestHeader::new(
                VmmDevRequestType::CtlGuestFilterMask,
                core::mem::size_of::<VmmDevCtlGuestFilterMask>() as u32
            ),
            or_mask,
            not_mask,
        };

        self.send_request(&request)?;
        self.global_event_mask = (self.global_event_mask | or_mask) & !not_mask;
        Ok(())
    }

    /// Send request to VMMDev
    fn send_request<T: Copy>(&self, request: &T) -> Result<(), &'static str> {
        let size = core::mem::size_of::<T>();
        let src = request as *const T as *const u8;

        unsafe {
            let dst = self.mmio_base as *mut u8;
            core::ptr::copy_nonoverlapping(src, dst, size);
        }

        if self.io_port != 0 {
            unsafe {
                core::arch::asm!(
                    "out dx, eax",
                    in("dx") self.io_port,
                    in("eax") self.mmio_base as u32,
                    options(nostack, nomem)
                );
            }
        }

        Ok(())
    }

    /// Create new session
    pub fn create_session(&mut self) -> u64 {
        let id = self.next_session_id;
        self.next_session_id += 1;

        let session = VboxSession::new(id);
        self.sessions.push(session);

        self.stats.sessions_opened.fetch_add(1, Ordering::Relaxed);
        id
    }

    /// Close session
    pub fn close_session(&mut self, id: u64) {
        if let Some(idx) = self.sessions.iter().position(|s| s.id() == id) {
            self.sessions[idx].close();
            self.sessions.remove(idx);
            self.stats.sessions_closed.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Get session
    pub fn get_session(&self, id: u64) -> Option<&VboxSession> {
        self.sessions.iter().find(|s| s.id() == id)
    }

    /// Handle IRQ
    pub fn handle_irq(&mut self) {
        self.stats.irq_count.fetch_add(1, Ordering::Relaxed);

        // Read and acknowledge events
        let events = self.read_and_ack_events();

        if events != 0 {
            self.stats.events_received.fetch_add(1, Ordering::Relaxed);
            self.dispatch_events(events);
        }
    }

    /// Read and acknowledge events
    fn read_and_ack_events(&mut self) -> u32 {
        let mut request = VmmDevAcknowledgeEvents {
            header: VmmDevRequestHeader::new(
                VmmDevRequestType::Idle,
                core::mem::size_of::<VmmDevAcknowledgeEvents>() as u32
            ),
            events: 0,
        };

        if self.send_request(&request).is_ok() {
            let response: VmmDevAcknowledgeEvents = unsafe {
                core::ptr::read_volatile(self.mmio_base as *const VmmDevAcknowledgeEvents)
            };
            request.events = response.events;
        }

        request.events
    }

    /// Dispatch events to sessions
    fn dispatch_events(&mut self, events: u32) {
        for session in &self.sessions {
            if session.is_active() && (events & session.event_mask) != 0 {
                session.add_event(events & session.event_mask);
                self.stats.events_dispatched.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Wait for event (blocking)
    pub fn wait_for_event(&self, session_id: u64, timeout_ms: u32) -> u32 {
        let session = match self.get_session(session_id) {
            Some(s) => s,
            None => return 0,
        };

        // Simple polling implementation
        let start = crate::time::ticks();
        // Assume 1000 ticks per second (1ms per tick)
        let timeout_ticks = timeout_ms as u64;

        while crate::time::ticks() - start < timeout_ticks {
            let events = session.pending_events();
            if events != 0 {
                return events;
            }
            // Yield CPU
            core::hint::spin_loop();
        }

        0
    }

    /// Get statistics
    pub fn stats(&self) -> &VboxGuestStats {
        &self.stats
    }

    /// Format status
    pub fn format_status(&self) -> String {
        alloc::format!(
            "VBoxGuest: sessions={} irqs={} events={}",
            self.sessions.len(),
            self.stats.irq_count.load(Ordering::Relaxed),
            self.stats.events_received.load(Ordering::Relaxed)
        )
    }
}

impl Default for VboxGuestDriver {
    fn default() -> Self {
        Self::new(0, 0, 0)
    }
}
