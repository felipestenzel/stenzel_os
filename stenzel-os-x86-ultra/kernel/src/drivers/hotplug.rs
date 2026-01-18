//! Display Hotplug Driver
//!
//! Handles dynamic display connect/disconnect events:
//! - Hotplug detection (HPD)
//! - EDID re-reading on connect
//! - Automatic mode configuration
//! - Event notification system

#![allow(dead_code)]

use alloc::collections::VecDeque;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use spin::Mutex;

use super::display_pipe::ConnectionStatus;
use super::multimon::{ConnectionType, MonitorInfo};

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static HOTPLUG_STATE: Mutex<Option<HotplugState>> = Mutex::new(None);
static EVENT_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Hotplug state
#[derive(Debug)]
pub struct HotplugState {
    /// Connector states
    pub connectors: Vec<ConnectorState>,
    /// Pending events
    pub event_queue: VecDeque<HotplugEvent>,
    /// Registered callbacks
    pub callbacks: Vec<HotplugCallback>,
    /// HPD enabled
    pub hpd_enabled: bool,
    /// Polling interval (ms)
    pub poll_interval_ms: u32,
    /// Last poll timestamp
    pub last_poll: u64,
}

/// Connector state for hotplug tracking
#[derive(Debug, Clone)]
pub struct ConnectorState {
    /// Connector ID
    pub id: u32,
    /// Connection type
    pub connector_type: ConnectionType,
    /// Current connection status
    pub status: ConnectionStatus,
    /// Previous status (for change detection)
    pub prev_status: ConnectionStatus,
    /// HPD capable
    pub hpd_capable: bool,
    /// Last status change timestamp
    pub last_change: u64,
    /// EDID valid
    pub edid_valid: bool,
}

/// Hotplug event
#[derive(Debug, Clone)]
pub struct HotplugEvent {
    /// Event sequence number
    pub sequence: u64,
    /// Connector ID
    pub connector_id: u32,
    /// Event type
    pub event_type: HotplugEventType,
    /// Timestamp
    pub timestamp: u64,
    /// Connection type
    pub connection_type: ConnectionType,
}

/// Hotplug event types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotplugEventType {
    /// Display connected
    Connected,
    /// Display disconnected
    Disconnected,
    /// EDID changed (same display, different mode)
    EdidChanged,
    /// Link status changed (DP link training)
    LinkStatusChanged,
}

/// Hotplug callback
#[derive(Debug)]
pub struct HotplugCallback {
    /// Callback ID
    pub id: u32,
    /// Event mask
    pub event_mask: u32,
    /// Connector filter (None = all connectors)
    pub connector_filter: Option<u32>,
}

/// HPD interrupt source
#[derive(Debug, Clone, Copy)]
pub enum HpdSource {
    /// GPIO-based HPD
    Gpio(u32),
    /// PCH HPD
    Pch,
    /// Display controller HPD
    DisplayController,
    /// USB-C PD
    UsbcPd,
}

/// Error type
#[derive(Debug, Clone, Copy)]
pub enum HotplugError {
    NotInitialized,
    ConnectorNotFound,
    QueueEmpty,
    CallbackNotFound,
}

/// Event mask constants
pub mod event_mask {
    pub const CONNECT: u32 = 1 << 0;
    pub const DISCONNECT: u32 = 1 << 1;
    pub const EDID_CHANGE: u32 = 1 << 2;
    pub const LINK_STATUS: u32 = 1 << 3;
    pub const ALL: u32 = 0xF;
}

/// Initialize hotplug subsystem
pub fn init() -> Result<(), HotplugError> {
    if INITIALIZED.load(Ordering::Acquire) {
        return Ok(());
    }

    let state = HotplugState {
        connectors: Vec::new(),
        event_queue: VecDeque::new(),
        callbacks: Vec::new(),
        hpd_enabled: true,
        poll_interval_ms: 1000,
        last_poll: 0,
    };

    *HOTPLUG_STATE.lock() = Some(state);
    INITIALIZED.store(true, Ordering::Release);

    crate::kprintln!("hotplug: Display hotplug subsystem initialized");
    Ok(())
}

/// Register a connector for hotplug monitoring
pub fn register_connector(
    connector_id: u32,
    connector_type: ConnectionType,
    hpd_capable: bool,
) -> Result<(), HotplugError> {
    let mut state = HOTPLUG_STATE.lock();
    let state = state.as_mut().ok_or(HotplugError::NotInitialized)?;

    let connector = ConnectorState {
        id: connector_id,
        connector_type,
        status: ConnectionStatus::Unknown,
        prev_status: ConnectionStatus::Unknown,
        hpd_capable,
        last_change: 0,
        edid_valid: false,
    };

    state.connectors.push(connector);
    Ok(())
}

/// Unregister a connector
pub fn unregister_connector(connector_id: u32) -> Result<(), HotplugError> {
    let mut state = HOTPLUG_STATE.lock();
    let state = state.as_mut().ok_or(HotplugError::NotInitialized)?;

    state.connectors.retain(|c| c.id != connector_id);
    Ok(())
}

/// Handle HPD interrupt
pub fn handle_hpd_interrupt(connector_id: u32, connected: bool) {
    // Find connector and update status
    let mut state_guard = HOTPLUG_STATE.lock();
    if let Some(state) = state_guard.as_mut() {
        if let Some(connector) = state.connectors.iter_mut().find(|c| c.id == connector_id) {
            let new_status = if connected {
                ConnectionStatus::Connected
            } else {
                ConnectionStatus::Disconnected
            };

            if connector.status != new_status {
                connector.prev_status = connector.status;
                connector.status = new_status;
                connector.last_change = crate::time::uptime_ns() as u64;

                let event_type = if connected {
                    HotplugEventType::Connected
                } else {
                    HotplugEventType::Disconnected
                };

                let event = HotplugEvent {
                    sequence: EVENT_SEQUENCE.fetch_add(1, Ordering::SeqCst),
                    connector_id,
                    event_type,
                    timestamp: connector.last_change,
                    connection_type: connector.connector_type,
                };

                state.event_queue.push_back(event);
            }
        }
    }
}

/// Poll for connection status changes
pub fn poll_connectors() -> Vec<HotplugEvent> {
    let mut events = Vec::new();
    let mut state_guard = HOTPLUG_STATE.lock();

    if let Some(state) = state_guard.as_mut() {
        let now = crate::time::uptime_ns() as u64;
        state.last_poll = now;

        // Query display_pipe for current connector status
        let connectors = super::display_pipe::get_connectors();

        for dp_connector in &connectors {
            if let Some(our_connector) = state.connectors.iter_mut().find(|c| c.id == dp_connector.id) {
                let new_status = dp_connector.connection;

                if our_connector.status != new_status {
                    our_connector.prev_status = our_connector.status;
                    our_connector.status = new_status;
                    our_connector.last_change = now;

                    let event_type = match new_status {
                        ConnectionStatus::Connected => HotplugEventType::Connected,
                        ConnectionStatus::Disconnected => HotplugEventType::Disconnected,
                        _ => continue,
                    };

                    let event = HotplugEvent {
                        sequence: EVENT_SEQUENCE.fetch_add(1, Ordering::SeqCst),
                        connector_id: our_connector.id,
                        event_type,
                        timestamp: now,
                        connection_type: our_connector.connector_type,
                    };

                    events.push(event.clone());
                    state.event_queue.push_back(event);
                }
            }
        }
    }

    events
}

/// Get next pending event
pub fn get_event() -> Option<HotplugEvent> {
    HOTPLUG_STATE
        .lock()
        .as_mut()
        .and_then(|s| s.event_queue.pop_front())
}

/// Peek at next event without removing
pub fn peek_event() -> Option<HotplugEvent> {
    HOTPLUG_STATE
        .lock()
        .as_ref()
        .and_then(|s| s.event_queue.front().cloned())
}

/// Get all pending events
pub fn drain_events() -> Vec<HotplugEvent> {
    HOTPLUG_STATE
        .lock()
        .as_mut()
        .map(|s| s.event_queue.drain(..).collect())
        .unwrap_or_default()
}

/// Register a callback for hotplug events
pub fn register_callback(event_mask: u32, connector_filter: Option<u32>) -> Result<u32, HotplugError> {
    static NEXT_CALLBACK_ID: AtomicU32 = AtomicU32::new(1);

    let mut state = HOTPLUG_STATE.lock();
    let state = state.as_mut().ok_or(HotplugError::NotInitialized)?;

    let id = NEXT_CALLBACK_ID.fetch_add(1, Ordering::SeqCst);
    let callback = HotplugCallback {
        id,
        event_mask,
        connector_filter,
    };

    state.callbacks.push(callback);
    Ok(id)
}

/// Unregister a callback
pub fn unregister_callback(callback_id: u32) -> Result<(), HotplugError> {
    let mut state = HOTPLUG_STATE.lock();
    let state = state.as_mut().ok_or(HotplugError::NotInitialized)?;

    let initial_len = state.callbacks.len();
    state.callbacks.retain(|c| c.id != callback_id);

    if state.callbacks.len() == initial_len {
        return Err(HotplugError::CallbackNotFound);
    }

    Ok(())
}

/// Enable/disable HPD
pub fn set_hpd_enabled(enabled: bool) {
    if let Some(state) = HOTPLUG_STATE.lock().as_mut() {
        state.hpd_enabled = enabled;
    }
}

/// Check if HPD is enabled
pub fn is_hpd_enabled() -> bool {
    HOTPLUG_STATE
        .lock()
        .as_ref()
        .map(|s| s.hpd_enabled)
        .unwrap_or(false)
}

/// Set poll interval
pub fn set_poll_interval(ms: u32) {
    if let Some(state) = HOTPLUG_STATE.lock().as_mut() {
        state.poll_interval_ms = ms;
    }
}

/// Get connector status
pub fn get_connector_status(connector_id: u32) -> Result<ConnectionStatus, HotplugError> {
    let state = HOTPLUG_STATE.lock();
    let state = state.as_ref().ok_or(HotplugError::NotInitialized)?;

    state
        .connectors
        .iter()
        .find(|c| c.id == connector_id)
        .map(|c| c.status)
        .ok_or(HotplugError::ConnectorNotFound)
}

/// Get all connector states
pub fn get_all_connector_states() -> Vec<ConnectorState> {
    HOTPLUG_STATE
        .lock()
        .as_ref()
        .map(|s| s.connectors.clone())
        .unwrap_or_default()
}

/// Check if any connector has pending status change
pub fn has_pending_changes() -> bool {
    HOTPLUG_STATE
        .lock()
        .as_ref()
        .map(|s| !s.event_queue.is_empty())
        .unwrap_or(false)
}

/// Get event count in queue
pub fn event_queue_length() -> usize {
    HOTPLUG_STATE
        .lock()
        .as_ref()
        .map(|s| s.event_queue.len())
        .unwrap_or(0)
}

/// Process hotplug event and update multi-monitor config
pub fn process_event(event: &HotplugEvent) -> Result<(), HotplugError> {
    match event.event_type {
        HotplugEventType::Connected => {
            // Re-detect monitors
            let _ = super::multimon::detect_monitors();
            crate::kprintln!(
                "hotplug: Display connected on connector {} ({:?})",
                event.connector_id,
                event.connection_type
            );
        }
        HotplugEventType::Disconnected => {
            // Update monitor list
            let _ = super::multimon::detect_monitors();
            crate::kprintln!(
                "hotplug: Display disconnected from connector {}",
                event.connector_id
            );
        }
        HotplugEventType::EdidChanged => {
            // Re-read EDID and update modes
            let _ = super::multimon::detect_monitors();
            crate::kprintln!(
                "hotplug: EDID changed on connector {}",
                event.connector_id
            );
        }
        HotplugEventType::LinkStatusChanged => {
            crate::kprintln!(
                "hotplug: Link status changed on connector {}",
                event.connector_id
            );
        }
    }
    Ok(())
}

/// Process all pending events
pub fn process_all_events() -> usize {
    let events = drain_events();
    let count = events.len();
    for event in events {
        let _ = process_event(&event);
    }
    count
}

/// Simulate HPD for testing
#[cfg(feature = "test")]
pub fn simulate_hpd(connector_id: u32, connected: bool) {
    handle_hpd_interrupt(connector_id, connected);
}
