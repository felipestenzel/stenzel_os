//! ALSA-like Audio API
//!
//! Provides a Linux ALSA-compatible API for audio applications.
//! This allows Linux audio software to work with minimal modifications.

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use spin::Mutex;

use super::{AudioConfig, AudioDevice, SampleFormat, StreamDirection, StreamHandle, StreamState, AUDIO_SYSTEM};

/// PCM stream type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum SndPcmStream {
    Playback = 0,
    Capture = 1,
}

/// PCM access type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum SndPcmAccess {
    MmapInterleaved = 0,
    MmapNoninterleaved = 1,
    MmapComplex = 2,
    RwInterleaved = 3,
    RwNoninterleaved = 4,
}

/// PCM format (ALSA-compatible)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum SndPcmFormat {
    Unknown = -1,
    S8 = 0,
    U8 = 1,
    S16Le = 2,
    S16Be = 3,
    U16Le = 4,
    U16Be = 5,
    S24Le = 6,
    S24Be = 7,
    U24Le = 8,
    U24Be = 9,
    S32Le = 10,
    S32Be = 11,
    U32Le = 12,
    U32Be = 13,
    FloatLe = 14,
    FloatBe = 15,
    Float64Le = 16,
    Float64Be = 17,
    // ... more formats
    S24_3Le = 32,
    S24_3Be = 33,
    U24_3Le = 34,
    U24_3Be = 35,
}

impl SndPcmFormat {
    pub fn to_sample_format(self) -> Option<SampleFormat> {
        match self {
            SndPcmFormat::U8 => Some(SampleFormat::U8),
            SndPcmFormat::S16Le => Some(SampleFormat::S16LE),
            SndPcmFormat::S16Be => Some(SampleFormat::S16BE),
            SndPcmFormat::S24Le | SndPcmFormat::S24_3Le => Some(SampleFormat::S24LE),
            SndPcmFormat::S24Be | SndPcmFormat::S24_3Be => Some(SampleFormat::S24BE),
            SndPcmFormat::S32Le => Some(SampleFormat::S32LE),
            SndPcmFormat::S32Be => Some(SampleFormat::S32BE),
            SndPcmFormat::FloatLe => Some(SampleFormat::F32LE),
            SndPcmFormat::FloatBe => Some(SampleFormat::F32BE),
            _ => None,
        }
    }

    pub fn from_sample_format(format: SampleFormat) -> Self {
        match format {
            SampleFormat::U8 => SndPcmFormat::U8,
            SampleFormat::S16LE => SndPcmFormat::S16Le,
            SampleFormat::S16BE => SndPcmFormat::S16Be,
            SampleFormat::S24LE => SndPcmFormat::S24Le,
            SampleFormat::S24BE => SndPcmFormat::S24Be,
            SampleFormat::S32LE => SndPcmFormat::S32Le,
            SampleFormat::S32BE => SndPcmFormat::S32Be,
            SampleFormat::F32LE => SndPcmFormat::FloatLe,
            SampleFormat::F32BE => SndPcmFormat::FloatBe,
        }
    }

    pub fn physical_width(&self) -> i32 {
        match self {
            SndPcmFormat::S8 | SndPcmFormat::U8 => 8,
            SndPcmFormat::S16Le | SndPcmFormat::S16Be |
            SndPcmFormat::U16Le | SndPcmFormat::U16Be => 16,
            SndPcmFormat::S24_3Le | SndPcmFormat::S24_3Be |
            SndPcmFormat::U24_3Le | SndPcmFormat::U24_3Be => 24,
            SndPcmFormat::S24Le | SndPcmFormat::S24Be |
            SndPcmFormat::U24Le | SndPcmFormat::U24Be |
            SndPcmFormat::S32Le | SndPcmFormat::S32Be |
            SndPcmFormat::U32Le | SndPcmFormat::U32Be |
            SndPcmFormat::FloatLe | SndPcmFormat::FloatBe => 32,
            SndPcmFormat::Float64Le | SndPcmFormat::Float64Be => 64,
            _ => 0,
        }
    }
}

/// PCM state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum SndPcmState {
    Open = 0,
    Setup = 1,
    Prepared = 2,
    Running = 3,
    Xrun = 4,
    Draining = 5,
    Paused = 6,
    Suspended = 7,
    Disconnected = 8,
}

impl From<StreamState> for SndPcmState {
    fn from(state: StreamState) -> Self {
        match state {
            StreamState::Stopped => SndPcmState::Setup,
            StreamState::Running => SndPcmState::Running,
            StreamState::Paused => SndPcmState::Paused,
            StreamState::Draining => SndPcmState::Draining,
        }
    }
}

/// Error codes (ALSA-compatible negated errno)
pub mod err {
    pub const OK: i32 = 0;
    pub const EPERM: i32 = -1;
    pub const ENOENT: i32 = -2;
    pub const EINTR: i32 = -4;
    pub const EIO: i32 = -5;
    pub const ENXIO: i32 = -6;
    pub const ENODEV: i32 = -19;
    pub const EINVAL: i32 = -22;
    pub const EPIPE: i32 = -32;  // Underrun/overrun
    pub const EAGAIN: i32 = -11; // Try again
    pub const ENOBUFS: i32 = -105;
    pub const ENOSYS: i32 = -38;
    pub const EBADFD: i32 = -77;
    pub const ESTRPIPE: i32 = -86; // Suspended
}

/// Hardware parameters
#[derive(Debug, Clone)]
pub struct SndPcmHwParams {
    pub access: SndPcmAccess,
    pub format: SndPcmFormat,
    pub rate: u32,
    pub channels: u32,
    pub period_size: u64,
    pub periods: u32,
    pub buffer_size: u64,
}

impl Default for SndPcmHwParams {
    fn default() -> Self {
        SndPcmHwParams {
            access: SndPcmAccess::RwInterleaved,
            format: SndPcmFormat::S16Le,
            rate: 44100,
            channels: 2,
            period_size: 1024,
            periods: 4,
            buffer_size: 4096,
        }
    }
}

/// Software parameters
#[derive(Debug, Clone)]
pub struct SndPcmSwParams {
    pub start_threshold: u64,
    pub stop_threshold: u64,
    pub silence_threshold: u64,
    pub silence_size: u64,
    pub avail_min: u64,
    pub xfer_align: u64,
}

impl Default for SndPcmSwParams {
    fn default() -> Self {
        SndPcmSwParams {
            start_threshold: 1,
            stop_threshold: u64::MAX,
            silence_threshold: 0,
            silence_size: 0,
            avail_min: 1,
            xfer_align: 1,
        }
    }
}

/// PCM status
#[derive(Debug, Clone, Default)]
pub struct SndPcmStatus {
    pub state: i32,
    pub trigger_tstamp: (i64, i64), // sec, nsec
    pub tstamp: (i64, i64),
    pub delay: i64,
    pub avail: u64,
    pub avail_max: u64,
    pub overrange: u64,
}

/// PCM handle
pub struct SndPcm {
    name: String,
    stream: SndPcmStream,
    device_index: usize,
    stream_handle: Option<StreamHandle>,
    hw_params: SndPcmHwParams,
    sw_params: SndPcmSwParams,
    state: SndPcmState,
    nonblock: bool,
}

impl SndPcm {
    /// Open a PCM device
    pub fn open(name: &str, stream: SndPcmStream, mode: i32) -> Result<Self, i32> {
        crate::kprintln!("alsa: snd_pcm_open({}, {:?}, {})", name, stream, mode);

        // Parse device name (e.g., "default", "hw:0,0", "plughw:0,0")
        let device_index = parse_device_name(name).unwrap_or(0);

        // Check if device exists
        let audio = AUDIO_SYSTEM.lock();
        if device_index >= audio.device_count() {
            return Err(err::ENODEV);
        }
        drop(audio);

        Ok(SndPcm {
            name: name.to_string(),
            stream,
            device_index,
            stream_handle: None,
            hw_params: SndPcmHwParams::default(),
            sw_params: SndPcmSwParams::default(),
            state: SndPcmState::Open,
            nonblock: (mode & 0x0004) != 0, // O_NONBLOCK
        })
    }

    /// Close the PCM device
    pub fn close(mut self) -> i32 {
        crate::kprintln!("alsa: snd_pcm_close({})", self.name);

        if let Some(handle) = self.stream_handle.take() {
            let mut audio = AUDIO_SYSTEM.lock();
            if let Some(device) = audio.get_device(self.device_index) {
                let _ = device.close_stream(handle);
            }
        }

        err::OK
    }

    /// Set hardware parameters
    pub fn hw_params(&mut self, params: &SndPcmHwParams) -> i32 {
        crate::kprintln!("alsa: snd_pcm_hw_params({:?})", params);

        // Build config
        let format = match params.format.to_sample_format() {
            Some(f) => f,
            None => return err::EINVAL,
        };

        let config = AudioConfig {
            sample_rate: params.rate,
            format,
            channels: params.channels as u8,
            buffer_size: params.buffer_size as usize,
            period_size: params.period_size as usize,
        };

        // Open stream on device
        let mut audio = AUDIO_SYSTEM.lock();
        let device = match audio.get_device(self.device_index) {
            Some(d) => d,
            None => return err::ENODEV,
        };

        let handle = match self.stream {
            SndPcmStream::Playback => device.open_playback(&config),
            SndPcmStream::Capture => device.open_capture(&config),
        };

        match handle {
            Ok(h) => {
                self.stream_handle = Some(h);
                self.hw_params = params.clone();
                self.state = SndPcmState::Setup;
                err::OK
            }
            Err(_) => err::EIO,
        }
    }

    /// Set software parameters
    pub fn sw_params(&mut self, params: &SndPcmSwParams) -> i32 {
        self.sw_params = params.clone();
        err::OK
    }

    /// Prepare for playback/capture
    pub fn prepare(&mut self) -> i32 {
        crate::kprintln!("alsa: snd_pcm_prepare({})", self.name);

        if self.stream_handle.is_none() {
            return err::EBADFD;
        }

        self.state = SndPcmState::Prepared;
        err::OK
    }

    /// Start playback/capture
    pub fn start(&mut self) -> i32 {
        crate::kprintln!("alsa: snd_pcm_start({})", self.name);

        let handle = match self.stream_handle {
            Some(h) => h,
            None => return err::EBADFD,
        };

        let mut audio = AUDIO_SYSTEM.lock();
        let device = match audio.get_device(self.device_index) {
            Some(d) => d,
            None => return err::ENODEV,
        };

        match device.start_stream(handle) {
            Ok(_) => {
                self.state = SndPcmState::Running;
                err::OK
            }
            Err(_) => err::EIO,
        }
    }

    /// Stop (drop) playback/capture
    pub fn drop_pcm(&mut self) -> i32 {
        crate::kprintln!("alsa: snd_pcm_drop({})", self.name);

        let handle = match self.stream_handle {
            Some(h) => h,
            None => return err::EBADFD,
        };

        let mut audio = AUDIO_SYSTEM.lock();
        let device = match audio.get_device(self.device_index) {
            Some(d) => d,
            None => return err::ENODEV,
        };

        match device.stop_stream(handle) {
            Ok(_) => {
                self.state = SndPcmState::Setup;
                err::OK
            }
            Err(_) => err::EIO,
        }
    }

    /// Drain (finish playing and stop)
    pub fn drain(&mut self) -> i32 {
        crate::kprintln!("alsa: snd_pcm_drain({})", self.name);
        self.state = SndPcmState::Draining;
        // In a real implementation, wait for buffer to drain
        self.drop_pcm()
    }

    /// Pause playback
    pub fn pause(&mut self, enable: bool) -> i32 {
        crate::kprintln!("alsa: snd_pcm_pause({}, {})", self.name, enable);

        if enable {
            self.state = SndPcmState::Paused;
        } else if self.state == SndPcmState::Paused {
            self.state = SndPcmState::Running;
        }

        err::OK
    }

    /// Get current state
    pub fn state(&self) -> SndPcmState {
        self.state
    }

    /// Write interleaved samples
    pub fn writei(&mut self, buffer: &[u8], frames: u64) -> i64 {
        let handle = match self.stream_handle {
            Some(h) => h,
            None => return err::EBADFD as i64,
        };

        if self.stream != SndPcmStream::Playback {
            return err::EINVAL as i64;
        }

        let frame_size = (self.hw_params.channels as usize) *
                         (self.hw_params.format.physical_width() as usize / 8);
        let bytes = (frames as usize) * frame_size;

        let data = &buffer[..bytes.min(buffer.len())];

        let mut audio = AUDIO_SYSTEM.lock();
        let device = match audio.get_device(self.device_index) {
            Some(d) => d,
            None => return err::ENODEV as i64,
        };

        // Auto-start if prepared
        if self.state == SndPcmState::Prepared {
            let _ = device.start_stream(handle);
            self.state = SndPcmState::Running;
        }

        match device.write(handle, data) {
            Ok(written) => (written / frame_size) as i64,
            Err(_) => err::EIO as i64,
        }
    }

    /// Read interleaved samples
    pub fn readi(&mut self, buffer: &mut [u8], frames: u64) -> i64 {
        let handle = match self.stream_handle {
            Some(h) => h,
            None => return err::EBADFD as i64,
        };

        if self.stream != SndPcmStream::Capture {
            return err::EINVAL as i64;
        }

        let frame_size = (self.hw_params.channels as usize) *
                         (self.hw_params.format.physical_width() as usize / 8);
        let bytes = (frames as usize) * frame_size;
        let buf_len = buffer.len();

        let data = &mut buffer[..bytes.min(buf_len)];

        let mut audio = AUDIO_SYSTEM.lock();
        let device = match audio.get_device(self.device_index) {
            Some(d) => d,
            None => return err::ENODEV as i64,
        };

        // Auto-start if prepared
        if self.state == SndPcmState::Prepared {
            let _ = device.start_stream(handle);
            self.state = SndPcmState::Running;
        }

        match device.read(handle, data) {
            Ok(read) => (read / frame_size) as i64,
            Err(_) => err::EIO as i64,
        }
    }

    /// Get available frames
    pub fn avail(&self) -> i64 {
        let handle = match self.stream_handle {
            Some(h) => h,
            None => return err::EBADFD as i64,
        };

        let audio = AUDIO_SYSTEM.lock();
        let frame_size = (self.hw_params.channels as usize) *
                         (self.hw_params.format.physical_width() as usize / 8);

        // Would call device.available(handle)
        // For now, return buffer size
        (self.hw_params.buffer_size / frame_size as u64) as i64
    }

    /// Get available frames and update state
    pub fn avail_update(&mut self) -> i64 {
        self.avail()
    }

    /// Wait for space/data to become available
    pub fn wait(&self, timeout: i32) -> i32 {
        crate::kprintln!("alsa: snd_pcm_wait({}, {})", self.name, timeout);
        // In a real implementation, would block until space/data available
        err::OK
    }

    /// Recover from error state
    pub fn recover(&mut self, err_code: i32, silent: bool) -> i32 {
        if !silent {
            crate::kprintln!("alsa: snd_pcm_recover({}, {})", self.name, err_code);
        }

        match err_code {
            err::EPIPE => {
                // Underrun/overrun - prepare and restart
                self.prepare();
                err::OK
            }
            err::ESTRPIPE => {
                // Suspended - try to resume
                self.prepare();
                err::OK
            }
            _ => err_code,
        }
    }

    /// Get PCM status
    pub fn status(&self, status: &mut SndPcmStatus) -> i32 {
        status.state = self.state as i32;
        status.avail = self.avail() as u64;
        err::OK
    }

    /// Get delay in frames
    pub fn delay(&self) -> i64 {
        // Return estimate based on buffer
        self.hw_params.buffer_size as i64 - self.avail()
    }

    /// Link two PCM streams
    pub fn link(&mut self, _other: &mut SndPcm) -> i32 {
        // Not implemented
        err::ENOSYS
    }

    /// Unlink PCM stream
    pub fn unlink(&mut self) -> i32 {
        err::OK
    }
}

/// Parse device name like "hw:0,0" or "default"
fn parse_device_name(name: &str) -> Option<usize> {
    if name == "default" || name == "null" {
        return Some(0);
    }

    // Parse "hw:X,Y" or "plughw:X,Y"
    let parts: Vec<_> = name.split(':').collect();
    if parts.len() >= 2 {
        let nums: Vec<_> = parts[1].split(',').collect();
        if let Some(card) = nums.get(0) {
            return card.parse().ok();
        }
    }

    Some(0)
}

/// Mixer element
pub struct SndMixerElem {
    name: String,
    index: u32,
    has_playback_volume: bool,
    has_capture_volume: bool,
    has_playback_switch: bool,
    has_capture_switch: bool,
    playback_volume: i64,
    capture_volume: i64,
    playback_switch: bool,
    capture_switch: bool,
    volume_min: i64,
    volume_max: i64,
}

impl SndMixerElem {
    pub fn new(name: &str) -> Self {
        SndMixerElem {
            name: name.to_string(),
            index: 0,
            has_playback_volume: true,
            has_capture_volume: true,
            has_playback_switch: true,
            has_capture_switch: true,
            playback_volume: 100,
            capture_volume: 100,
            playback_switch: true,
            capture_switch: true,
            volume_min: 0,
            volume_max: 100,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn has_playback_volume(&self) -> bool {
        self.has_playback_volume
    }

    pub fn has_capture_volume(&self) -> bool {
        self.has_capture_volume
    }

    pub fn has_playback_switch(&self) -> bool {
        self.has_playback_switch
    }

    pub fn has_capture_switch(&self) -> bool {
        self.has_capture_switch
    }

    pub fn get_playback_volume(&self) -> i64 {
        self.playback_volume
    }

    pub fn set_playback_volume(&mut self, volume: i64) {
        self.playback_volume = volume.clamp(self.volume_min, self.volume_max);
    }

    pub fn get_capture_volume(&self) -> i64 {
        self.capture_volume
    }

    pub fn set_capture_volume(&mut self, volume: i64) {
        self.capture_volume = volume.clamp(self.volume_min, self.volume_max);
    }

    pub fn get_playback_switch(&self) -> bool {
        self.playback_switch
    }

    pub fn set_playback_switch(&mut self, on: bool) {
        self.playback_switch = on;
    }

    pub fn get_capture_switch(&self) -> bool {
        self.capture_switch
    }

    pub fn set_capture_switch(&mut self, on: bool) {
        self.capture_switch = on;
    }

    pub fn get_volume_range(&self) -> (i64, i64) {
        (self.volume_min, self.volume_max)
    }
}

/// Mixer handle
pub struct SndMixer {
    name: String,
    elements: Vec<SndMixerElem>,
}

impl SndMixer {
    pub fn open(_mode: i32) -> Result<Self, i32> {
        Ok(SndMixer {
            name: String::from("default"),
            elements: Vec::new(),
        })
    }

    pub fn attach(&mut self, name: &str) -> i32 {
        self.name = name.to_string();
        err::OK
    }

    pub fn register(&mut self) -> i32 {
        // Create default mixer elements
        self.elements.push(SndMixerElem::new("Master"));
        self.elements.push(SndMixerElem::new("PCM"));
        self.elements.push(SndMixerElem::new("Speaker"));
        self.elements.push(SndMixerElem::new("Headphone"));
        self.elements.push(SndMixerElem::new("Mic"));
        self.elements.push(SndMixerElem::new("Capture"));
        err::OK
    }

    pub fn load(&mut self) -> i32 {
        err::OK
    }

    pub fn close(self) -> i32 {
        err::OK
    }

    pub fn first_elem(&mut self) -> Option<&mut SndMixerElem> {
        self.elements.first_mut()
    }

    pub fn find_elem(&mut self, name: &str) -> Option<&mut SndMixerElem> {
        self.elements.iter_mut().find(|e| e.name == name)
    }

    pub fn elem_count(&self) -> usize {
        self.elements.len()
    }
}

/// Convert ALSA dB value to linear (0-100)
pub fn db_to_linear(db: i64) -> u8 {
    // Simplified conversion
    // dB is typically -9999 to 0, where 0 is maximum
    let linear = if db >= 0 {
        100
    } else if db <= -6000 {
        0
    } else {
        ((db + 6000) * 100 / 6000) as u8
    };
    linear
}

/// Convert linear volume (0-100) to ALSA dB
pub fn linear_to_db(linear: u8) -> i64 {
    if linear >= 100 {
        0
    } else if linear == 0 {
        -9999
    } else {
        (linear as i64) * 6000 / 100 - 6000
    }
}

/// Get error string
pub fn strerror(errnum: i32) -> &'static str {
    match errnum {
        err::OK => "Success",
        err::EPERM => "Operation not permitted",
        err::ENOENT => "No such file or directory",
        err::EINTR => "Interrupted system call",
        err::EIO => "Input/output error",
        err::ENXIO => "No such device or address",
        err::ENODEV => "No such device",
        err::EINVAL => "Invalid argument",
        err::EPIPE => "Underrun/overrun occurred",
        err::EAGAIN => "Resource temporarily unavailable",
        err::ENOBUFS => "No buffer space available",
        err::ENOSYS => "Function not implemented",
        err::EBADFD => "File descriptor in bad state",
        err::ESTRPIPE => "Device suspended",
        _ => "Unknown error",
    }
}

/// Initialize ALSA subsystem
pub fn init() {
    crate::kprintln!("alsa: ALSA-like API initialized");
}
