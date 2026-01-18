//! PulseAudio Compatibility Layer
//!
//! Provides PulseAudio-compatible API for Linux applications.
//! This allows PulseAudio-based applications to work with Stenzel OS audio.

#![allow(dead_code)]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use crate::sync::IrqSafeMutex;

use super::daemon::{self, ClientId, ClientProtocol, NodeId};
use super::{AudioConfig, SampleFormat, StreamDirection};

/// PulseAudio sample format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum PaSampleFormat {
    U8 = 0,
    ALaw = 1,
    ULaw = 2,
    S16Le = 3,
    S16Be = 4,
    Float32Le = 5,
    Float32Be = 6,
    S32Le = 7,
    S32Be = 8,
    S24Le = 9,
    S24Be = 10,
    S24_32Le = 11,
    S24_32Be = 12,
    Max = 13,
    Invalid = -1,
}

impl PaSampleFormat {
    pub fn to_sample_format(self) -> Option<SampleFormat> {
        match self {
            PaSampleFormat::U8 => Some(SampleFormat::U8),
            PaSampleFormat::S16Le => Some(SampleFormat::S16LE),
            PaSampleFormat::S16Be => Some(SampleFormat::S16BE),
            PaSampleFormat::Float32Le => Some(SampleFormat::F32LE),
            PaSampleFormat::Float32Be => Some(SampleFormat::F32BE),
            PaSampleFormat::S32Le => Some(SampleFormat::S32LE),
            PaSampleFormat::S32Be => Some(SampleFormat::S32BE),
            PaSampleFormat::S24Le => Some(SampleFormat::S24LE),
            PaSampleFormat::S24Be => Some(SampleFormat::S24BE),
            _ => None,
        }
    }

    pub fn from_sample_format(format: SampleFormat) -> Self {
        match format {
            SampleFormat::U8 => PaSampleFormat::U8,
            SampleFormat::S16LE => PaSampleFormat::S16Le,
            SampleFormat::S16BE => PaSampleFormat::S16Be,
            SampleFormat::S24LE => PaSampleFormat::S24Le,
            SampleFormat::S24BE => PaSampleFormat::S24Be,
            SampleFormat::S32LE => PaSampleFormat::S32Le,
            SampleFormat::S32BE => PaSampleFormat::S32Be,
            SampleFormat::F32LE => PaSampleFormat::Float32Le,
            SampleFormat::F32BE => PaSampleFormat::Float32Be,
        }
    }

    pub fn frame_size(&self, channels: u8) -> usize {
        let sample_size = match self {
            PaSampleFormat::U8 | PaSampleFormat::ALaw | PaSampleFormat::ULaw => 1,
            PaSampleFormat::S16Le | PaSampleFormat::S16Be => 2,
            PaSampleFormat::S24Le | PaSampleFormat::S24Be => 3,
            PaSampleFormat::Float32Le | PaSampleFormat::Float32Be |
            PaSampleFormat::S32Le | PaSampleFormat::S32Be |
            PaSampleFormat::S24_32Le | PaSampleFormat::S24_32Be => 4,
            _ => 0,
        };
        sample_size * channels as usize
    }
}

/// PulseAudio sample spec
#[derive(Debug, Clone)]
pub struct PaSampleSpec {
    pub format: PaSampleFormat,
    pub rate: u32,
    pub channels: u8,
}

impl Default for PaSampleSpec {
    fn default() -> Self {
        Self {
            format: PaSampleFormat::S16Le,
            rate: 44100,
            channels: 2,
        }
    }
}

impl PaSampleSpec {
    pub fn bytes_per_second(&self) -> usize {
        self.format.frame_size(self.channels) * self.rate as usize
    }

    pub fn frame_size(&self) -> usize {
        self.format.frame_size(self.channels)
    }

    pub fn valid(&self) -> bool {
        self.format != PaSampleFormat::Invalid &&
        self.rate >= 1 && self.rate <= 192000 &&
        self.channels >= 1 && self.channels <= 32
    }
}

/// PulseAudio channel map position
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum PaChannelPosition {
    Invalid = -1,
    Mono = 0,
    FrontLeft = 1,
    FrontRight = 2,
    FrontCenter = 3,
    RearCenter = 4,
    RearLeft = 5,
    RearRight = 6,
    Lfe = 7,
    FrontLeftOfCenter = 8,
    FrontRightOfCenter = 9,
    SideLeft = 10,
    SideRight = 11,
    Aux0 = 12,
    // ... more positions
    Max = 36,
}

/// PulseAudio channel map
#[derive(Debug, Clone)]
pub struct PaChannelMap {
    pub channels: u8,
    pub map: [PaChannelPosition; 32],
}

impl Default for PaChannelMap {
    fn default() -> Self {
        let mut map = [PaChannelPosition::Invalid; 32];
        map[0] = PaChannelPosition::FrontLeft;
        map[1] = PaChannelPosition::FrontRight;
        Self { channels: 2, map }
    }
}

impl PaChannelMap {
    pub fn init_stereo() -> Self {
        Self::default()
    }

    pub fn init_mono() -> Self {
        let mut map = [PaChannelPosition::Invalid; 32];
        map[0] = PaChannelPosition::Mono;
        Self { channels: 1, map }
    }

    pub fn valid(&self) -> bool {
        self.channels >= 1 && self.channels <= 32
    }
}

/// PulseAudio buffer attributes
#[derive(Debug, Clone)]
pub struct PaBufferAttr {
    pub maxlength: u32,    // Maximum buffer length in bytes
    pub tlength: u32,      // Target buffer length for playback
    pub prebuf: u32,       // Pre-buffering
    pub minreq: u32,       // Minimum request size
    pub fragsize: u32,     // Fragment size for recording
}

impl Default for PaBufferAttr {
    fn default() -> Self {
        Self {
            maxlength: u32::MAX,
            tlength: u32::MAX,
            prebuf: u32::MAX,
            minreq: u32::MAX,
            fragsize: u32::MAX,
        }
    }
}

/// PulseAudio context state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum PaContextState {
    Unconnected = 0,
    Connecting = 1,
    Authorizing = 2,
    SettingName = 3,
    Ready = 4,
    Failed = 5,
    Terminated = 6,
}

/// PulseAudio stream state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum PaStreamState {
    Unconnected = 0,
    Creating = 1,
    Ready = 2,
    Failed = 3,
    Terminated = 4,
}

/// PulseAudio stream direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum PaStreamDirection {
    NoDirection = 0,
    Playback = 1,
    Record = 2,
    Upload = 3,
}

/// PulseAudio stream flags
pub mod pa_stream_flags {
    pub const NOFLAGS: u32 = 0;
    pub const START_CORKED: u32 = 1 << 0;
    pub const INTERPOLATE_TIMING: u32 = 1 << 1;
    pub const NOT_MONOTONIC: u32 = 1 << 2;
    pub const AUTO_TIMING_UPDATE: u32 = 1 << 3;
    pub const NO_REMAP_CHANNELS: u32 = 1 << 4;
    pub const NO_REMIX_CHANNELS: u32 = 1 << 5;
    pub const FIX_FORMAT: u32 = 1 << 6;
    pub const FIX_RATE: u32 = 1 << 7;
    pub const FIX_CHANNELS: u32 = 1 << 8;
    pub const DONT_MOVE: u32 = 1 << 9;
    pub const VARIABLE_RATE: u32 = 1 << 10;
    pub const PEAK_DETECT: u32 = 1 << 11;
    pub const START_MUTED: u32 = 1 << 12;
    pub const ADJUST_LATENCY: u32 = 1 << 13;
    pub const EARLY_REQUESTS: u32 = 1 << 14;
    pub const DONT_INHIBIT_AUTO_SUSPEND: u32 = 1 << 15;
    pub const START_UNMUTED: u32 = 1 << 16;
    pub const FAIL_ON_SUSPEND: u32 = 1 << 17;
    pub const RELATIVE_VOLUME: u32 = 1 << 18;
    pub const PASSTHROUGH: u32 = 1 << 19;
}

/// PulseAudio seek mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum PaSeekMode {
    Relative = 0,
    Absolute = 1,
    RelativeOnRead = 2,
    RelativeEnd = 3,
}

/// PulseAudio volume
pub type PaVolume = u32;

pub const PA_VOLUME_MUTED: PaVolume = 0;
pub const PA_VOLUME_NORM: PaVolume = 0x10000;
pub const PA_VOLUME_MAX: PaVolume = u32::MAX / 2;
pub const PA_VOLUME_INVALID: PaVolume = u32::MAX;

/// PulseAudio channel volumes
#[derive(Debug, Clone)]
pub struct PaCvolume {
    pub channels: u8,
    pub values: [PaVolume; 32],
}

impl Default for PaCvolume {
    fn default() -> Self {
        let mut values = [PA_VOLUME_NORM; 32];
        Self { channels: 2, values }
    }
}

impl PaCvolume {
    pub fn set(&mut self, channels: u8, volume: PaVolume) {
        self.channels = channels;
        for i in 0..channels as usize {
            self.values[i] = volume;
        }
    }

    pub fn avg(&self) -> PaVolume {
        if self.channels == 0 {
            return PA_VOLUME_MUTED;
        }
        let sum: u64 = self.values[..self.channels as usize]
            .iter()
            .map(|&v| v as u64)
            .sum();
        (sum / self.channels as u64) as PaVolume
    }

    pub fn to_linear(&self) -> f64 {
        let avg = self.avg();
        if avg == PA_VOLUME_MUTED {
            0.0
        } else {
            // PulseAudio uses cubic volume scale
            let f = avg as f64 / PA_VOLUME_NORM as f64;
            f * f * f
        }
    }

    pub fn to_percent(&self) -> u8 {
        (self.to_linear() * 100.0).min(100.0) as u8
    }
}

/// Simple cube root approximation using Newton's method
fn cbrt_approx(x: f64) -> f64 {
    if x == 0.0 {
        return 0.0;
    }
    let negative = x < 0.0;
    let x = if negative { -x } else { x };

    // Initial guess
    let mut guess = x / 3.0;
    if guess == 0.0 {
        guess = 1.0;
    }

    // Newton's method iterations
    for _ in 0..10 {
        guess = (2.0 * guess + x / (guess * guess)) / 3.0;
    }

    if negative { -guess } else { guess }
}

/// Convert percent (0-100) to PA volume
pub fn pa_sw_volume_from_linear(linear: f64) -> PaVolume {
    if linear <= 0.0 {
        PA_VOLUME_MUTED
    } else {
        // Cube root for linear to PA conversion
        let f = cbrt_approx(linear);
        (f * PA_VOLUME_NORM as f64) as PaVolume
    }
}

/// PulseAudio sink info
#[derive(Debug, Clone)]
pub struct PaSinkInfo {
    pub name: String,
    pub index: u32,
    pub description: String,
    pub sample_spec: PaSampleSpec,
    pub channel_map: PaChannelMap,
    pub owner_module: u32,
    pub volume: PaCvolume,
    pub mute: bool,
    pub monitor_source: u32,
    pub monitor_source_name: String,
    pub latency: u64,
    pub driver: String,
    pub flags: u32,
    pub state: i32,
}

/// PulseAudio source info
#[derive(Debug, Clone)]
pub struct PaSourceInfo {
    pub name: String,
    pub index: u32,
    pub description: String,
    pub sample_spec: PaSampleSpec,
    pub channel_map: PaChannelMap,
    pub owner_module: u32,
    pub volume: PaCvolume,
    pub mute: bool,
    pub monitor_of_sink: u32,
    pub monitor_of_sink_name: String,
    pub latency: u64,
    pub driver: String,
    pub flags: u32,
    pub state: i32,
}

/// PulseAudio sink input info (playback stream)
#[derive(Debug, Clone)]
pub struct PaSinkInputInfo {
    pub index: u32,
    pub name: String,
    pub owner_module: u32,
    pub client: u32,
    pub sink: u32,
    pub sample_spec: PaSampleSpec,
    pub channel_map: PaChannelMap,
    pub volume: PaCvolume,
    pub mute: bool,
    pub buffer_usec: u64,
    pub sink_usec: u64,
    pub driver: String,
}

/// PulseAudio source output info (capture stream)
#[derive(Debug, Clone)]
pub struct PaSourceOutputInfo {
    pub index: u32,
    pub name: String,
    pub owner_module: u32,
    pub client: u32,
    pub source: u32,
    pub sample_spec: PaSampleSpec,
    pub channel_map: PaChannelMap,
    pub volume: PaCvolume,
    pub mute: bool,
    pub buffer_usec: u64,
    pub source_usec: u64,
    pub driver: String,
}

/// PulseAudio server info
#[derive(Debug, Clone)]
pub struct PaServerInfo {
    pub user_name: String,
    pub host_name: String,
    pub server_version: String,
    pub server_name: String,
    pub sample_spec: PaSampleSpec,
    pub default_sink_name: String,
    pub default_source_name: String,
    pub cookie: u32,
    pub channel_map: PaChannelMap,
}

/// PulseAudio context
pub struct PaContext {
    name: String,
    state: PaContextState,
    daemon_client_id: Option<ClientId>,
    server_name: String,
}

impl PaContext {
    /// Create a new context
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            state: PaContextState::Unconnected,
            daemon_client_id: None,
            server_name: String::new(),
        }
    }

    /// Connect to the server
    pub fn connect(&mut self, server: Option<&str>) -> i32 {
        self.state = PaContextState::Connecting;
        self.server_name = server.unwrap_or("stenzel-audio").to_string();

        // Connect to daemon
        match daemon::connect_client(&self.name, ClientProtocol::PulseAudio) {
            Some(client_id) => {
                self.daemon_client_id = Some(client_id);
                self.state = PaContextState::Ready;
                0
            }
            None => {
                self.state = PaContextState::Failed;
                -1
            }
        }
    }

    /// Disconnect from the server
    pub fn disconnect(&mut self) {
        if let Some(client_id) = self.daemon_client_id.take() {
            daemon::disconnect_client(client_id);
        }
        self.state = PaContextState::Terminated;
    }

    /// Get context state
    pub fn get_state(&self) -> PaContextState {
        self.state
    }

    /// Check if context is ready
    pub fn is_ready(&self) -> bool {
        self.state == PaContextState::Ready
    }

    /// Get server info
    pub fn get_server_info(&self) -> Option<PaServerInfo> {
        if !self.is_ready() {
            return None;
        }

        Some(PaServerInfo {
            user_name: "root".to_string(),
            host_name: "stenzel".to_string(),
            server_version: "1.0.0".to_string(),
            server_name: self.server_name.clone(),
            sample_spec: PaSampleSpec::default(),
            default_sink_name: "default_sink".to_string(),
            default_source_name: "default_source".to_string(),
            cookie: 0x12345678,
            channel_map: PaChannelMap::default(),
        })
    }

    /// Get list of sinks
    pub fn get_sink_info_list(&self) -> Vec<PaSinkInfo> {
        if !self.is_ready() {
            return Vec::new();
        }

        let devices = daemon::list_devices();
        devices
            .iter()
            .filter(|d| matches!(d.direction, StreamDirection::Playback))
            .enumerate()
            .map(|(i, d)| PaSinkInfo {
                name: d.name.clone(),
                index: i as u32,
                description: d.description.clone(),
                sample_spec: PaSampleSpec {
                    format: PaSampleFormat::S16Le,
                    rate: d.sample_rate,
                    channels: d.channels,
                },
                channel_map: PaChannelMap::default(),
                owner_module: 0,
                volume: PaCvolume::default(),
                mute: false,
                monitor_source: i as u32,
                monitor_source_name: alloc::format!("{}.monitor", d.name),
                latency: 0,
                driver: "stenzel-audio".to_string(),
                flags: 0,
                state: if d.is_available { 0 } else { 2 },
            })
            .collect()
    }

    /// Get list of sources
    pub fn get_source_info_list(&self) -> Vec<PaSourceInfo> {
        if !self.is_ready() {
            return Vec::new();
        }

        let devices = daemon::list_devices();
        devices
            .iter()
            .filter(|d| matches!(d.direction, StreamDirection::Capture))
            .enumerate()
            .map(|(i, d)| PaSourceInfo {
                name: d.name.clone(),
                index: i as u32,
                description: d.description.clone(),
                sample_spec: PaSampleSpec {
                    format: PaSampleFormat::S16Le,
                    rate: d.sample_rate,
                    channels: d.channels,
                },
                channel_map: PaChannelMap::default(),
                owner_module: 0,
                volume: PaCvolume::default(),
                mute: false,
                monitor_of_sink: u32::MAX,
                monitor_of_sink_name: String::new(),
                latency: 0,
                driver: "stenzel-audio".to_string(),
                flags: 0,
                state: if d.is_available { 0 } else { 2 },
            })
            .collect()
    }

    /// Get list of sink inputs (playback streams)
    pub fn get_sink_input_info_list(&self) -> Vec<PaSinkInputInfo> {
        if !self.is_ready() {
            return Vec::new();
        }

        let streams = daemon::list_streams();
        streams
            .iter()
            .filter(|s| matches!(s.direction, StreamDirection::Playback))
            .enumerate()
            .map(|(i, s)| {
                let mut volume = PaCvolume::default();
                volume.set(2, pa_sw_volume_from_linear(s.volume as f64 / 100.0));

                PaSinkInputInfo {
                    index: i as u32,
                    name: s.stream_name.clone(),
                    owner_module: 0,
                    client: 0,
                    sink: s.device_id.unwrap_or(0),
                    sample_spec: PaSampleSpec::default(),
                    channel_map: PaChannelMap::default(),
                    volume,
                    mute: s.muted,
                    buffer_usec: 0,
                    sink_usec: 0,
                    driver: "stenzel-audio".to_string(),
                }
            })
            .collect()
    }

    /// Set sink volume
    pub fn set_sink_volume_by_index(&self, index: u32, volume: &PaCvolume) -> bool {
        let devices = daemon::list_devices();
        let device = devices
            .iter()
            .filter(|d| matches!(d.direction, StreamDirection::Playback))
            .nth(index as usize);

        if let Some(d) = device {
            daemon::set_stream_volume(d.id, volume.to_percent())
        } else {
            false
        }
    }

    /// Set sink mute
    pub fn set_sink_mute_by_index(&self, index: u32, mute: bool) -> bool {
        let devices = daemon::list_devices();
        let device = devices
            .iter()
            .filter(|d| matches!(d.direction, StreamDirection::Playback))
            .nth(index as usize);

        if let Some(d) = device {
            daemon::set_stream_mute(d.id, mute)
        } else {
            false
        }
    }

    /// Set sink input volume
    pub fn set_sink_input_volume(&self, index: u32, volume: &PaCvolume) -> bool {
        let streams = daemon::list_streams();
        let stream = streams
            .iter()
            .filter(|s| matches!(s.direction, StreamDirection::Playback))
            .nth(index as usize);

        if let Some(s) = stream {
            daemon::set_stream_volume(s.id, volume.to_percent())
        } else {
            false
        }
    }

    /// Set sink input mute
    pub fn set_sink_input_mute(&self, index: u32, mute: bool) -> bool {
        let streams = daemon::list_streams();
        let stream = streams
            .iter()
            .filter(|s| matches!(s.direction, StreamDirection::Playback))
            .nth(index as usize);

        if let Some(s) = stream {
            daemon::set_stream_mute(s.id, mute)
        } else {
            false
        }
    }

    /// Set default sink
    pub fn set_default_sink(&self, name: &str) -> bool {
        let devices = daemon::list_devices();
        let device = devices
            .iter()
            .find(|d| d.name == name && matches!(d.direction, StreamDirection::Playback));

        if let Some(d) = device {
            daemon::set_default_sink(d.id)
        } else {
            false
        }
    }

    /// Set default source
    pub fn set_default_source(&self, name: &str) -> bool {
        let devices = daemon::list_devices();
        let device = devices
            .iter()
            .find(|d| d.name == name && matches!(d.direction, StreamDirection::Capture));

        if let Some(d) = device {
            daemon::set_default_source(d.id)
        } else {
            false
        }
    }
}

impl Drop for PaContext {
    fn drop(&mut self) {
        self.disconnect();
    }
}

/// PulseAudio stream
pub struct PaStream {
    name: String,
    direction: PaStreamDirection,
    sample_spec: PaSampleSpec,
    channel_map: PaChannelMap,
    state: PaStreamState,
    context_client_id: Option<ClientId>,
    daemon_node_id: Option<NodeId>,
    buffer: IrqSafeMutex<Vec<u8>>,
    buffer_attr: PaBufferAttr,
    volume: PaCvolume,
    muted: AtomicBool,
    corked: AtomicBool,
    write_index: AtomicU64,
    read_index: AtomicU64,
}

impl PaStream {
    /// Create a new stream
    pub fn new(
        ctx: &PaContext,
        name: &str,
        sample_spec: &PaSampleSpec,
        channel_map: Option<&PaChannelMap>,
    ) -> Option<Self> {
        if !ctx.is_ready() || !sample_spec.valid() {
            return None;
        }

        let map = channel_map.cloned().unwrap_or_default();

        Some(Self {
            name: name.to_string(),
            direction: PaStreamDirection::NoDirection,
            sample_spec: sample_spec.clone(),
            channel_map: map,
            state: PaStreamState::Unconnected,
            context_client_id: ctx.daemon_client_id,
            daemon_node_id: None,
            buffer: IrqSafeMutex::new(vec![0u8; 65536]),
            buffer_attr: PaBufferAttr::default(),
            volume: PaCvolume::default(),
            muted: AtomicBool::new(false),
            corked: AtomicBool::new(false),
            write_index: AtomicU64::new(0),
            read_index: AtomicU64::new(0),
        })
    }

    /// Connect stream for playback
    pub fn connect_playback(
        &mut self,
        dev: Option<&str>,
        attr: Option<&PaBufferAttr>,
        flags: u32,
    ) -> i32 {
        let client_id = match self.context_client_id {
            Some(id) => id,
            None => return -1,
        };

        self.direction = PaStreamDirection::Playback;
        self.state = PaStreamState::Creating;

        if let Some(a) = attr {
            self.buffer_attr = a.clone();
        }

        // Create playback stream in daemon
        match daemon::create_playback_stream(client_id, &self.name) {
            Some(node_id) => {
                self.daemon_node_id = Some(node_id);
                self.state = PaStreamState::Ready;

                // Handle flags
                if flags & pa_stream_flags::START_CORKED != 0 {
                    self.corked.store(true, Ordering::Relaxed);
                }
                if flags & pa_stream_flags::START_MUTED != 0 {
                    self.muted.store(true, Ordering::Relaxed);
                }

                0
            }
            None => {
                self.state = PaStreamState::Failed;
                -1
            }
        }
    }

    /// Connect stream for recording
    pub fn connect_record(
        &mut self,
        dev: Option<&str>,
        attr: Option<&PaBufferAttr>,
        flags: u32,
    ) -> i32 {
        let client_id = match self.context_client_id {
            Some(id) => id,
            None => return -1,
        };

        self.direction = PaStreamDirection::Record;
        self.state = PaStreamState::Creating;

        if let Some(a) = attr {
            self.buffer_attr = a.clone();
        }

        // Create capture stream in daemon
        match daemon::create_capture_stream(client_id, &self.name) {
            Some(node_id) => {
                self.daemon_node_id = Some(node_id);
                self.state = PaStreamState::Ready;

                if flags & pa_stream_flags::START_CORKED != 0 {
                    self.corked.store(true, Ordering::Relaxed);
                }

                0
            }
            None => {
                self.state = PaStreamState::Failed;
                -1
            }
        }
    }

    /// Disconnect the stream
    pub fn disconnect(&mut self) -> i32 {
        self.state = PaStreamState::Terminated;
        self.daemon_node_id = None;
        0
    }

    /// Get stream state
    pub fn get_state(&self) -> PaStreamState {
        self.state
    }

    /// Get sample spec
    pub fn get_sample_spec(&self) -> &PaSampleSpec {
        &self.sample_spec
    }

    /// Get channel map
    pub fn get_channel_map(&self) -> &PaChannelMap {
        &self.channel_map
    }

    /// Get buffer attributes
    pub fn get_buffer_attr(&self) -> &PaBufferAttr {
        &self.buffer_attr
    }

    /// Write data to playback stream
    pub fn write(&self, data: &[u8], _seek: PaSeekMode) -> i64 {
        if self.direction != PaStreamDirection::Playback {
            return -1;
        }
        if self.corked.load(Ordering::Relaxed) {
            return 0;
        }

        let mut buffer = self.buffer.lock();
        let write_idx = self.write_index.load(Ordering::Acquire) as usize;
        let buffer_len = buffer.len();

        let to_write = data.len().min(buffer_len - (write_idx % buffer_len));
        let start = write_idx % buffer_len;

        buffer[start..start + to_write].copy_from_slice(&data[..to_write]);
        self.write_index.fetch_add(to_write as u64, Ordering::Release);

        to_write as i64
    }

    /// Read data from capture stream
    pub fn read(&self, data: &mut [u8]) -> i64 {
        if self.direction != PaStreamDirection::Record {
            return -1;
        }
        if self.corked.load(Ordering::Relaxed) {
            return 0;
        }

        let buffer = self.buffer.lock();
        let read_idx = self.read_index.load(Ordering::Acquire) as usize;
        let write_idx = self.write_index.load(Ordering::Acquire) as usize;
        let buffer_len = buffer.len();

        let available = write_idx.saturating_sub(read_idx);
        let to_read = data.len().min(available).min(buffer_len);
        let start = read_idx % buffer_len;

        data[..to_read].copy_from_slice(&buffer[start..start + to_read]);
        drop(buffer);
        self.read_index.fetch_add(to_read as u64, Ordering::Release);

        to_read as i64
    }

    /// Get writable size
    pub fn writable_size(&self) -> usize {
        let buffer = self.buffer.lock();
        let write_idx = self.write_index.load(Ordering::Acquire) as usize;
        let read_idx = self.read_index.load(Ordering::Acquire) as usize;

        let used = write_idx.saturating_sub(read_idx);
        buffer.len().saturating_sub(used)
    }

    /// Get readable size
    pub fn readable_size(&self) -> usize {
        let write_idx = self.write_index.load(Ordering::Acquire) as usize;
        let read_idx = self.read_index.load(Ordering::Acquire) as usize;
        write_idx.saturating_sub(read_idx)
    }

    /// Cork (pause) the stream
    pub fn cork(&self, cork: bool) -> i32 {
        self.corked.store(cork, Ordering::Relaxed);
        0
    }

    /// Check if corked
    pub fn is_corked(&self) -> bool {
        self.corked.load(Ordering::Relaxed)
    }

    /// Flush the buffer
    pub fn flush(&self) -> i32 {
        let mut buffer = self.buffer.lock();
        buffer.fill(0);
        self.write_index.store(0, Ordering::Release);
        self.read_index.store(0, Ordering::Release);
        0
    }

    /// Drain the buffer
    pub fn drain(&self) -> i32 {
        // In a real implementation, would wait for buffer to empty
        0
    }

    /// Trigger a stream update
    pub fn trigger(&self) -> i32 {
        0
    }

    /// Set stream volume
    pub fn set_volume(&mut self, volume: &PaCvolume) {
        self.volume = volume.clone();
        if let Some(node_id) = self.daemon_node_id {
            daemon::set_stream_volume(node_id, volume.to_percent());
        }
    }

    /// Get stream volume
    pub fn get_volume(&self) -> &PaCvolume {
        &self.volume
    }

    /// Set stream mute
    pub fn set_mute(&self, mute: bool) {
        self.muted.store(mute, Ordering::Relaxed);
        if let Some(node_id) = self.daemon_node_id {
            daemon::set_stream_mute(node_id, mute);
        }
    }

    /// Get stream mute
    pub fn get_mute(&self) -> bool {
        self.muted.load(Ordering::Relaxed)
    }

    /// Get latency
    pub fn get_latency(&self) -> u64 {
        // Estimate latency based on buffer
        let samples = self.writable_size() / self.sample_spec.frame_size();
        (samples as u64 * 1_000_000) / self.sample_spec.rate as u64
    }
}

/// Simple API for single-stream playback
pub struct PaSimple {
    stream: PaStream,
}

impl PaSimple {
    /// Create simple playback stream
    pub fn new_playback(
        server: Option<&str>,
        name: &str,
        stream_name: &str,
        sample_spec: &PaSampleSpec,
        channel_map: Option<&PaChannelMap>,
        attr: Option<&PaBufferAttr>,
    ) -> Option<Self> {
        let mut ctx = PaContext::new(name);
        if ctx.connect(server) < 0 {
            return None;
        }

        let mut stream = PaStream::new(&ctx, stream_name, sample_spec, channel_map)?;
        if stream.connect_playback(None, attr, 0) < 0 {
            return None;
        }

        Some(Self { stream })
    }

    /// Create simple recording stream
    pub fn new_record(
        server: Option<&str>,
        name: &str,
        stream_name: &str,
        sample_spec: &PaSampleSpec,
        channel_map: Option<&PaChannelMap>,
        attr: Option<&PaBufferAttr>,
    ) -> Option<Self> {
        let mut ctx = PaContext::new(name);
        if ctx.connect(server) < 0 {
            return None;
        }

        let mut stream = PaStream::new(&ctx, stream_name, sample_spec, channel_map)?;
        if stream.connect_record(None, attr, 0) < 0 {
            return None;
        }

        Some(Self { stream })
    }

    /// Write data
    pub fn write(&self, data: &[u8]) -> i64 {
        self.stream.write(data, PaSeekMode::Relative)
    }

    /// Read data
    pub fn read(&self, data: &mut [u8]) -> i64 {
        self.stream.read(data)
    }

    /// Drain
    pub fn drain(&self) -> i32 {
        self.stream.drain()
    }

    /// Flush
    pub fn flush(&self) -> i32 {
        self.stream.flush()
    }

    /// Get latency
    pub fn get_latency(&self) -> u64 {
        self.stream.get_latency()
    }
}

// =============================================================================
// Global initialization
// =============================================================================

/// Initialize PulseAudio compatibility layer
pub fn init() {
    crate::kprintln!("pulse: PulseAudio compatibility layer initialized");
}

/// Create a new PulseAudio context
pub fn context_new(name: &str) -> PaContext {
    PaContext::new(name)
}
