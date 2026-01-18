//! Low Latency Audio Subsystem
//!
//! Provides real-time, low-latency audio support for professional audio applications.
//! Features:
//! - JACK-compatible API
//! - Lock-free ring buffers
//! - Real-time thread scheduling
//! - Configurable buffer sizes (32-2048 samples)
//! - Xrun detection and recovery
//! - Multi-client synchronization

#![allow(dead_code)]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::boxed::Box;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use crate::sync::TicketSpinlock;

/// Global low-latency audio state
static LOWLATENCY_STATE: TicketSpinlock<Option<LowLatencyAudio>> = TicketSpinlock::new(None);

/// Buffer size presets (in samples)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum BufferSize {
    /// 32 samples (~0.7ms at 48kHz) - Ultra low latency
    Samples32 = 32,
    /// 64 samples (~1.3ms at 48kHz) - Very low latency
    Samples64 = 64,
    /// 128 samples (~2.7ms at 48kHz) - Low latency
    Samples128 = 128,
    /// 256 samples (~5.3ms at 48kHz) - Standard low latency
    Samples256 = 256,
    /// 512 samples (~10.7ms at 48kHz) - Conservative
    Samples512 = 512,
    /// 1024 samples (~21.3ms at 48kHz) - Safe
    Samples1024 = 1024,
    /// 2048 samples (~42.7ms at 48kHz) - Maximum buffer
    Samples2048 = 2048,
}

impl BufferSize {
    /// Calculate latency in microseconds at given sample rate
    pub fn latency_us(&self, sample_rate: u32) -> u32 {
        (*self as u32 * 1_000_000) / sample_rate
    }

    /// Calculate latency in milliseconds at given sample rate
    pub fn latency_ms(&self, sample_rate: u32) -> f32 {
        (*self as u32 as f32 * 1000.0) / sample_rate as f32
    }

    pub fn as_samples(&self) -> u32 {
        *self as u32
    }
}

/// Sample rates commonly used in professional audio
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SampleRate {
    /// 44100 Hz - CD quality
    Rate44100 = 44100,
    /// 48000 Hz - Professional standard
    Rate48000 = 48000,
    /// 88200 Hz - High resolution
    Rate88200 = 88200,
    /// 96000 Hz - High resolution
    Rate96000 = 96000,
    /// 176400 Hz - Very high resolution
    Rate176400 = 176400,
    /// 192000 Hz - Maximum
    Rate192000 = 192000,
}

impl SampleRate {
    pub fn as_hz(&self) -> u32 {
        *self as u32
    }
}

/// Audio data format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleFormat {
    /// 16-bit signed integer
    S16,
    /// 24-bit signed integer (in 32-bit container)
    S24,
    /// 32-bit signed integer
    S32,
    /// 32-bit floating point
    F32,
}

impl SampleFormat {
    pub fn bytes_per_sample(&self) -> usize {
        match self {
            SampleFormat::S16 => 2,
            SampleFormat::S24 => 4,
            SampleFormat::S32 => 4,
            SampleFormat::F32 => 4,
        }
    }
}

/// Real-time thread priority
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RtPriority {
    /// Lowest RT priority (51)
    Low,
    /// Normal RT priority (70)
    Normal,
    /// High RT priority (85)
    High,
    /// Maximum RT priority (99)
    Max,
    /// Custom priority value
    Custom(u8),
}

impl RtPriority {
    pub fn as_priority(&self) -> u8 {
        match self {
            RtPriority::Low => 51,
            RtPriority::Normal => 70,
            RtPriority::High => 85,
            RtPriority::Max => 99,
            RtPriority::Custom(p) => *p,
        }
    }
}

/// Xrun type (underrun or overrun)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XrunType {
    /// Buffer underrun (playback)
    Underrun,
    /// Buffer overrun (capture)
    Overrun,
}

/// Xrun event information
#[derive(Debug, Clone)]
pub struct XrunEvent {
    pub xrun_type: XrunType,
    pub client_id: ClientId,
    pub port_id: PortId,
    pub timestamp: u64,
    pub delayed_usecs: u64,
}

/// Unique client ID
pub type ClientId = u32;

/// Unique port ID
pub type PortId = u32;

/// Unique connection ID
pub type ConnectionId = u32;

/// Port direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortDirection {
    Input,
    Output,
}

/// Port flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortFlags(u32);

impl PortFlags {
    pub const NONE: Self = Self(0);
    pub const IS_PHYSICAL: Self = Self(1 << 0);
    pub const CAN_MONITOR: Self = Self(1 << 1);
    pub const IS_TERMINAL: Self = Self(1 << 2);
    pub const IS_CONTROL: Self = Self(1 << 3);

    pub fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

/// Client state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientState {
    /// Client created but not activated
    Inactive,
    /// Client is active and processing
    Active,
    /// Client processing is suspended
    Suspended,
    /// Client being shut down
    Closing,
}

/// Transport state for synchronization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportState {
    Stopped,
    Rolling,
    Starting,
    Stopping,
}

/// Transport position information
#[derive(Debug, Clone, Copy, Default)]
pub struct TransportPosition {
    /// Frame position
    pub frame: u64,
    /// Time in microseconds
    pub usecs: u64,
    /// Current sample rate
    pub sample_rate: u32,
    /// Bar position (for MIDI sync)
    pub bar: u32,
    /// Beat within bar
    pub beat: u32,
    /// Tick within beat
    pub tick: u32,
    /// Beats per minute
    pub bpm: f32,
    /// Time signature numerator
    pub time_sig_num: u8,
    /// Time signature denominator
    pub time_sig_denom: u8,
}

/// Lock-free ring buffer for audio data
pub struct AudioRingBuffer {
    buffer: Vec<f32>,
    capacity: usize,
    read_pos: AtomicU32,
    write_pos: AtomicU32,
    overflows: AtomicU64,
    underflows: AtomicU64,
}

impl AudioRingBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: vec![0.0; capacity],
            capacity,
            read_pos: AtomicU32::new(0),
            write_pos: AtomicU32::new(0),
            overflows: AtomicU64::new(0),
            underflows: AtomicU64::new(0),
        }
    }

    /// Get number of samples available to read
    pub fn available(&self) -> usize {
        let read = self.read_pos.load(Ordering::Acquire) as usize;
        let write = self.write_pos.load(Ordering::Acquire) as usize;
        if write >= read {
            write - read
        } else {
            self.capacity - read + write
        }
    }

    /// Get free space for writing
    pub fn space(&self) -> usize {
        self.capacity - self.available() - 1
    }

    /// Write samples to buffer (non-blocking)
    pub fn write(&mut self, samples: &[f32]) -> usize {
        let space = self.space();
        let to_write = core::cmp::min(samples.len(), space);

        if to_write < samples.len() {
            self.overflows.fetch_add(1, Ordering::Relaxed);
        }

        let write = self.write_pos.load(Ordering::Acquire) as usize;
        for (i, &sample) in samples[..to_write].iter().enumerate() {
            let pos = (write + i) % self.capacity;
            self.buffer[pos] = sample;
        }

        let new_write = (write + to_write) % self.capacity;
        self.write_pos.store(new_write as u32, Ordering::Release);

        to_write
    }

    /// Read samples from buffer (non-blocking)
    pub fn read(&mut self, samples: &mut [f32]) -> usize {
        let available = self.available();
        let to_read = core::cmp::min(samples.len(), available);

        if to_read < samples.len() {
            self.underflows.fetch_add(1, Ordering::Relaxed);
            // Fill remaining with silence
            for s in &mut samples[to_read..] {
                *s = 0.0;
            }
        }

        let read = self.read_pos.load(Ordering::Acquire) as usize;
        for (i, sample) in samples[..to_read].iter_mut().enumerate() {
            let pos = (read + i) % self.capacity;
            *sample = self.buffer[pos];
        }

        let new_read = (read + to_read) % self.capacity;
        self.read_pos.store(new_read as u32, Ordering::Release);

        to_read
    }

    /// Clear the buffer
    pub fn clear(&mut self) {
        self.read_pos.store(0, Ordering::Release);
        self.write_pos.store(0, Ordering::Release);
    }

    /// Get overflow count
    pub fn overflow_count(&self) -> u64 {
        self.overflows.load(Ordering::Relaxed)
    }

    /// Get underflow count
    pub fn underflow_count(&self) -> u64 {
        self.underflows.load(Ordering::Relaxed)
    }
}

/// Audio port for low-latency processing
pub struct LowLatencyPort {
    pub id: PortId,
    pub client_id: ClientId,
    pub name: String,
    pub direction: PortDirection,
    pub flags: PortFlags,
    pub buffer: AudioRingBuffer,
    pub connected_to: Vec<PortId>,
    pub latency_frames: u32,
}

impl LowLatencyPort {
    pub fn new(
        id: PortId,
        client_id: ClientId,
        name: &str,
        direction: PortDirection,
        buffer_size: usize,
    ) -> Self {
        Self {
            id,
            client_id,
            name: String::from(name),
            direction,
            flags: PortFlags::NONE,
            buffer: AudioRingBuffer::new(buffer_size * 4), // 4x buffer for safety
            connected_to: Vec::new(),
            latency_frames: 0,
        }
    }
}

/// Audio client for low-latency processing
pub struct LowLatencyClient {
    pub id: ClientId,
    pub name: String,
    pub state: ClientState,
    pub rt_priority: RtPriority,
    pub ports: Vec<PortId>,
    pub process_callback: Option<Box<dyn Fn(&mut ProcessContext) + Send>>,
    pub xrun_count: AtomicU64,
    pub cpu_load: AtomicU32, // Percentage * 100
}

impl LowLatencyClient {
    pub fn new(id: ClientId, name: &str, rt_priority: RtPriority) -> Self {
        Self {
            id,
            name: String::from(name),
            state: ClientState::Inactive,
            rt_priority,
            ports: Vec::new(),
            process_callback: None,
            xrun_count: AtomicU64::new(0),
            cpu_load: AtomicU32::new(0),
        }
    }
}

/// Processing context passed to client callbacks
pub struct ProcessContext {
    pub client_id: ClientId,
    pub nframes: u32,
    pub sample_rate: u32,
    pub frame_time: u64,
    pub input_buffers: BTreeMap<PortId, Vec<f32>>,
    pub output_buffers: BTreeMap<PortId, Vec<f32>>,
}

impl ProcessContext {
    /// Get input buffer for port
    pub fn get_input(&self, port_id: PortId) -> Option<&[f32]> {
        self.input_buffers.get(&port_id).map(|v| v.as_slice())
    }

    /// Get mutable output buffer for port
    pub fn get_output(&mut self, port_id: PortId) -> Option<&mut [f32]> {
        self.output_buffers.get_mut(&port_id).map(|v| v.as_mut_slice())
    }
}

/// Connection between ports
#[derive(Debug, Clone)]
pub struct Connection {
    pub id: ConnectionId,
    pub source_port: PortId,
    pub dest_port: PortId,
}

/// Engine configuration
#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub sample_rate: SampleRate,
    pub buffer_size: BufferSize,
    pub periods: u32,
    pub rt_priority: RtPriority,
    pub soft_mode: bool, // Allow xruns without stopping
    pub freewheel: bool, // Run without realtime constraints
    pub sync_mode: SyncMode,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            sample_rate: SampleRate::Rate48000,
            buffer_size: BufferSize::Samples256,
            periods: 2,
            rt_priority: RtPriority::High,
            soft_mode: true,
            freewheel: false,
            sync_mode: SyncMode::Internal,
        }
    }
}

/// Synchronization mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMode {
    /// Internal clock
    Internal,
    /// External word clock
    WordClock,
    /// ADAT sync
    Adat,
    /// SPDIF sync
    Spdif,
}

/// Engine statistics
#[derive(Debug, Clone, Default)]
pub struct EngineStats {
    pub total_frames: u64,
    pub xrun_count: u64,
    pub max_process_time_us: u64,
    pub avg_process_time_us: u64,
    pub cpu_load_percent: f32,
    pub clients_count: u32,
    pub ports_count: u32,
    pub connections_count: u32,
}

/// Low latency audio engine
pub struct LowLatencyAudio {
    /// Engine configuration
    config: EngineConfig,
    /// Registered clients
    clients: BTreeMap<ClientId, LowLatencyClient>,
    /// Registered ports
    ports: BTreeMap<PortId, LowLatencyPort>,
    /// Port connections
    connections: BTreeMap<ConnectionId, Connection>,
    /// Transport state
    transport_state: TransportState,
    /// Transport position
    transport_position: TransportPosition,
    /// Engine running
    running: AtomicBool,
    /// Current frame position
    frame_position: AtomicU64,
    /// Xrun events
    xrun_events: Vec<XrunEvent>,
    /// Next client ID
    next_client_id: ClientId,
    /// Next port ID
    next_port_id: PortId,
    /// Next connection ID
    next_connection_id: ConnectionId,
}

impl LowLatencyAudio {
    pub fn new(config: EngineConfig) -> Self {
        Self {
            config,
            clients: BTreeMap::new(),
            ports: BTreeMap::new(),
            connections: BTreeMap::new(),
            transport_state: TransportState::Stopped,
            transport_position: TransportPosition::default(),
            running: AtomicBool::new(false),
            frame_position: AtomicU64::new(0),
            xrun_events: Vec::new(),
            next_client_id: 1,
            next_port_id: 1,
            next_connection_id: 1,
        }
    }
}

/// Error type for low latency audio
#[derive(Debug)]
pub enum LowLatencyError {
    NotInitialized,
    AlreadyRunning,
    NotRunning,
    ClientNotFound,
    PortNotFound,
    ConnectionNotFound,
    BufferTooSmall,
    InvalidConfig,
    XrunOccurred,
    ResourceBusy,
    PermissionDenied,
}

// ============================================================================
// Public API
// ============================================================================

/// Initialize the low latency audio engine
pub fn init() {
    init_with_config(EngineConfig::default())
}

/// Initialize with custom configuration
pub fn init_with_config(config: EngineConfig) {
    let mut state = LOWLATENCY_STATE.lock();
    if state.is_none() {
        *state = Some(LowLatencyAudio::new(config));
    }
    crate::kprintln!("[lowlatency] Low latency audio engine initialized");
}

/// Get engine configuration
pub fn get_config() -> Result<EngineConfig, LowLatencyError> {
    let state = LOWLATENCY_STATE.lock();
    let engine = state.as_ref().ok_or(LowLatencyError::NotInitialized)?;
    Ok(engine.config.clone())
}

/// Update engine configuration (only when stopped)
pub fn set_config(config: EngineConfig) -> Result<(), LowLatencyError> {
    let mut state = LOWLATENCY_STATE.lock();
    let engine = state.as_mut().ok_or(LowLatencyError::NotInitialized)?;

    if engine.running.load(Ordering::Acquire) {
        return Err(LowLatencyError::AlreadyRunning);
    }

    engine.config = config;
    Ok(())
}

/// Start the audio engine
pub fn start() -> Result<(), LowLatencyError> {
    let mut state = LOWLATENCY_STATE.lock();
    let engine = state.as_mut().ok_or(LowLatencyError::NotInitialized)?;

    if engine.running.load(Ordering::Acquire) {
        return Err(LowLatencyError::AlreadyRunning);
    }

    engine.running.store(true, Ordering::Release);
    engine.transport_state = TransportState::Rolling;

    // TODO: Start actual audio processing thread with RT priority

    Ok(())
}

/// Stop the audio engine
pub fn stop() -> Result<(), LowLatencyError> {
    let mut state = LOWLATENCY_STATE.lock();
    let engine = state.as_mut().ok_or(LowLatencyError::NotInitialized)?;

    if !engine.running.load(Ordering::Acquire) {
        return Err(LowLatencyError::NotRunning);
    }

    engine.running.store(false, Ordering::Release);
    engine.transport_state = TransportState::Stopped;

    Ok(())
}

/// Check if engine is running
pub fn is_running() -> bool {
    let state = LOWLATENCY_STATE.lock();
    if let Some(ref engine) = *state {
        engine.running.load(Ordering::Acquire)
    } else {
        false
    }
}

/// Create a new client
pub fn create_client(name: &str, rt_priority: RtPriority) -> Result<ClientId, LowLatencyError> {
    let mut state = LOWLATENCY_STATE.lock();
    let engine = state.as_mut().ok_or(LowLatencyError::NotInitialized)?;

    let client_id = engine.next_client_id;
    engine.next_client_id += 1;

    let client = LowLatencyClient::new(client_id, name, rt_priority);
    engine.clients.insert(client_id, client);

    Ok(client_id)
}

/// Destroy a client
pub fn destroy_client(client_id: ClientId) -> Result<(), LowLatencyError> {
    let mut state = LOWLATENCY_STATE.lock();
    let engine = state.as_mut().ok_or(LowLatencyError::NotInitialized)?;

    // Remove all ports owned by this client
    let ports_to_remove: Vec<PortId> = engine.ports
        .iter()
        .filter(|(_, p)| p.client_id == client_id)
        .map(|(id, _)| *id)
        .collect();

    for port_id in ports_to_remove {
        engine.ports.remove(&port_id);
        // Remove connections to/from this port
        engine.connections.retain(|_, c| {
            c.source_port != port_id && c.dest_port != port_id
        });
    }

    engine.clients.remove(&client_id).ok_or(LowLatencyError::ClientNotFound)?;
    Ok(())
}

/// Activate a client
pub fn activate_client(client_id: ClientId) -> Result<(), LowLatencyError> {
    let mut state = LOWLATENCY_STATE.lock();
    let engine = state.as_mut().ok_or(LowLatencyError::NotInitialized)?;

    let client = engine.clients.get_mut(&client_id)
        .ok_or(LowLatencyError::ClientNotFound)?;

    client.state = ClientState::Active;
    Ok(())
}

/// Deactivate a client
pub fn deactivate_client(client_id: ClientId) -> Result<(), LowLatencyError> {
    let mut state = LOWLATENCY_STATE.lock();
    let engine = state.as_mut().ok_or(LowLatencyError::NotInitialized)?;

    let client = engine.clients.get_mut(&client_id)
        .ok_or(LowLatencyError::ClientNotFound)?;

    client.state = ClientState::Inactive;
    Ok(())
}

/// Register an audio port
pub fn register_port(
    client_id: ClientId,
    name: &str,
    direction: PortDirection,
    flags: PortFlags,
) -> Result<PortId, LowLatencyError> {
    let mut state = LOWLATENCY_STATE.lock();
    let engine = state.as_mut().ok_or(LowLatencyError::NotInitialized)?;

    // Verify client exists
    if !engine.clients.contains_key(&client_id) {
        return Err(LowLatencyError::ClientNotFound);
    }

    let port_id = engine.next_port_id;
    engine.next_port_id += 1;

    let buffer_size = engine.config.buffer_size.as_samples() as usize;
    let mut port = LowLatencyPort::new(port_id, client_id, name, direction, buffer_size);
    port.flags = flags;

    engine.ports.insert(port_id, port);

    // Add port to client's port list
    if let Some(client) = engine.clients.get_mut(&client_id) {
        client.ports.push(port_id);
    }

    Ok(port_id)
}

/// Unregister a port
pub fn unregister_port(port_id: PortId) -> Result<(), LowLatencyError> {
    let mut state = LOWLATENCY_STATE.lock();
    let engine = state.as_mut().ok_or(LowLatencyError::NotInitialized)?;

    let port = engine.ports.remove(&port_id)
        .ok_or(LowLatencyError::PortNotFound)?;

    // Remove from client's port list
    if let Some(client) = engine.clients.get_mut(&port.client_id) {
        client.ports.retain(|&id| id != port_id);
    }

    // Remove connections
    engine.connections.retain(|_, c| {
        c.source_port != port_id && c.dest_port != port_id
    });

    Ok(())
}

/// Connect two ports
pub fn connect_ports(source: PortId, dest: PortId) -> Result<ConnectionId, LowLatencyError> {
    let mut state = LOWLATENCY_STATE.lock();
    let engine = state.as_mut().ok_or(LowLatencyError::NotInitialized)?;

    // Verify ports exist
    if !engine.ports.contains_key(&source) || !engine.ports.contains_key(&dest) {
        return Err(LowLatencyError::PortNotFound);
    }

    let conn_id = engine.next_connection_id;
    engine.next_connection_id += 1;

    let connection = Connection {
        id: conn_id,
        source_port: source,
        dest_port: dest,
    };

    engine.connections.insert(conn_id, connection);

    // Update port connected_to lists
    if let Some(src_port) = engine.ports.get_mut(&source) {
        src_port.connected_to.push(dest);
    }
    if let Some(dst_port) = engine.ports.get_mut(&dest) {
        dst_port.connected_to.push(source);
    }

    Ok(conn_id)
}

/// Disconnect two ports
pub fn disconnect_ports(source: PortId, dest: PortId) -> Result<(), LowLatencyError> {
    let mut state = LOWLATENCY_STATE.lock();
    let engine = state.as_mut().ok_or(LowLatencyError::NotInitialized)?;

    // Find and remove the connection
    let mut found_id = None;
    for (id, conn) in engine.connections.iter() {
        if conn.source_port == source && conn.dest_port == dest {
            found_id = Some(*id);
            break;
        }
    }

    if let Some(id) = found_id {
        engine.connections.remove(&id);

        // Update port connected_to lists
        if let Some(src_port) = engine.ports.get_mut(&source) {
            src_port.connected_to.retain(|&p| p != dest);
        }
        if let Some(dst_port) = engine.ports.get_mut(&dest) {
            dst_port.connected_to.retain(|&p| p != source);
        }

        Ok(())
    } else {
        Err(LowLatencyError::ConnectionNotFound)
    }
}

/// Get list of all ports
pub fn list_ports() -> Result<Vec<PortId>, LowLatencyError> {
    let state = LOWLATENCY_STATE.lock();
    let engine = state.as_ref().ok_or(LowLatencyError::NotInitialized)?;

    Ok(engine.ports.keys().copied().collect())
}

/// Get list of all clients
pub fn list_clients() -> Result<Vec<ClientId>, LowLatencyError> {
    let state = LOWLATENCY_STATE.lock();
    let engine = state.as_ref().ok_or(LowLatencyError::NotInitialized)?;

    Ok(engine.clients.keys().copied().collect())
}

/// Get engine statistics
pub fn get_stats() -> Result<EngineStats, LowLatencyError> {
    let state = LOWLATENCY_STATE.lock();
    let engine = state.as_ref().ok_or(LowLatencyError::NotInitialized)?;

    Ok(EngineStats {
        total_frames: engine.frame_position.load(Ordering::Relaxed),
        xrun_count: engine.xrun_events.len() as u64,
        max_process_time_us: 0, // TODO: Track
        avg_process_time_us: 0, // TODO: Track
        cpu_load_percent: 0.0,  // TODO: Track
        clients_count: engine.clients.len() as u32,
        ports_count: engine.ports.len() as u32,
        connections_count: engine.connections.len() as u32,
    })
}

/// Get transport state
pub fn get_transport_state() -> Result<TransportState, LowLatencyError> {
    let state = LOWLATENCY_STATE.lock();
    let engine = state.as_ref().ok_or(LowLatencyError::NotInitialized)?;

    Ok(engine.transport_state)
}

/// Set transport state
pub fn set_transport_state(transport_state: TransportState) -> Result<(), LowLatencyError> {
    let mut state = LOWLATENCY_STATE.lock();
    let engine = state.as_mut().ok_or(LowLatencyError::NotInitialized)?;

    engine.transport_state = transport_state;
    Ok(())
}

/// Get transport position
pub fn get_transport_position() -> Result<TransportPosition, LowLatencyError> {
    let state = LOWLATENCY_STATE.lock();
    let engine = state.as_ref().ok_or(LowLatencyError::NotInitialized)?;

    Ok(engine.transport_position)
}

/// Set transport position
pub fn set_transport_position(position: TransportPosition) -> Result<(), LowLatencyError> {
    let mut state = LOWLATENCY_STATE.lock();
    let engine = state.as_mut().ok_or(LowLatencyError::NotInitialized)?;

    engine.transport_position = position;
    engine.frame_position.store(position.frame, Ordering::Release);
    Ok(())
}

/// Get sample rate
pub fn get_sample_rate() -> Result<u32, LowLatencyError> {
    let state = LOWLATENCY_STATE.lock();
    let engine = state.as_ref().ok_or(LowLatencyError::NotInitialized)?;

    Ok(engine.config.sample_rate.as_hz())
}

/// Get buffer size
pub fn get_buffer_size() -> Result<u32, LowLatencyError> {
    let state = LOWLATENCY_STATE.lock();
    let engine = state.as_ref().ok_or(LowLatencyError::NotInitialized)?;

    Ok(engine.config.buffer_size.as_samples())
}

/// Get latency in microseconds
pub fn get_latency_us() -> Result<u32, LowLatencyError> {
    let state = LOWLATENCY_STATE.lock();
    let engine = state.as_ref().ok_or(LowLatencyError::NotInitialized)?;

    Ok(engine.config.buffer_size.latency_us(engine.config.sample_rate.as_hz()))
}

/// Report an xrun
pub fn report_xrun(client_id: ClientId, port_id: PortId, xrun_type: XrunType) {
    let mut state = LOWLATENCY_STATE.lock();
    if let Some(ref mut engine) = *state {
        let event = XrunEvent {
            xrun_type,
            client_id,
            port_id,
            timestamp: engine.frame_position.load(Ordering::Relaxed),
            delayed_usecs: 0, // TODO: Calculate actual delay
        };
        engine.xrun_events.push(event);

        // Update client xrun count
        if let Some(client) = engine.clients.get(&client_id) {
            client.xrun_count.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Get recent xrun events
pub fn get_xrun_events() -> Result<Vec<XrunEvent>, LowLatencyError> {
    let state = LOWLATENCY_STATE.lock();
    let engine = state.as_ref().ok_or(LowLatencyError::NotInitialized)?;

    Ok(engine.xrun_events.clone())
}

/// Clear xrun events
pub fn clear_xrun_events() -> Result<(), LowLatencyError> {
    let mut state = LOWLATENCY_STATE.lock();
    let engine = state.as_mut().ok_or(LowLatencyError::NotInitialized)?;

    engine.xrun_events.clear();
    Ok(())
}

/// Enable freewheel mode (non-realtime processing)
pub fn set_freewheel(enabled: bool) -> Result<(), LowLatencyError> {
    let mut state = LOWLATENCY_STATE.lock();
    let engine = state.as_mut().ok_or(LowLatencyError::NotInitialized)?;

    engine.config.freewheel = enabled;
    Ok(())
}

/// Check if freewheel mode is enabled
pub fn is_freewheel() -> Result<bool, LowLatencyError> {
    let state = LOWLATENCY_STATE.lock();
    let engine = state.as_ref().ok_or(LowLatencyError::NotInitialized)?;

    Ok(engine.config.freewheel)
}
