//! Audio Drivers
//!
//! Provides audio playback and recording capabilities.
//! Supports Intel HDA (High Definition Audio) controllers.
//! Includes:
//! - ALSA-compatible API for Linux applications
//! - Software audio mixer with per-channel volume, pan, and EQ

extern crate alloc;

pub mod ac97;
pub mod alsa;
pub mod hda;
pub mod mixer;
pub mod pcspkr;

use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;

/// Audio sample format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleFormat {
    U8,
    S16LE,
    S16BE,
    S24LE,
    S24BE,
    S32LE,
    S32BE,
    F32LE,
    F32BE,
}

impl SampleFormat {
    pub fn bytes_per_sample(&self) -> usize {
        match self {
            SampleFormat::U8 => 1,
            SampleFormat::S16LE | SampleFormat::S16BE => 2,
            SampleFormat::S24LE | SampleFormat::S24BE => 3,
            SampleFormat::S32LE | SampleFormat::S32BE |
            SampleFormat::F32LE | SampleFormat::F32BE => 4,
        }
    }
}

/// Audio stream configuration
#[derive(Debug, Clone)]
pub struct AudioConfig {
    pub sample_rate: u32,
    pub format: SampleFormat,
    pub channels: u8,
    pub buffer_size: usize,
    pub period_size: usize,
}

impl Default for AudioConfig {
    fn default() -> Self {
        AudioConfig {
            sample_rate: 44100,
            format: SampleFormat::S16LE,
            channels: 2,
            buffer_size: 4096,
            period_size: 1024,
        }
    }
}

/// Audio stream direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamDirection {
    Playback,
    Capture,
}

/// Audio stream state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamState {
    Stopped,
    Running,
    Paused,
    Draining,
}

/// Audio stream handle
pub type StreamHandle = u32;

/// Audio device capabilities
#[derive(Debug, Clone)]
pub struct AudioCapabilities {
    pub name: String,
    pub supported_rates: Vec<u32>,
    pub supported_formats: Vec<SampleFormat>,
    pub max_channels: u8,
    pub can_playback: bool,
    pub can_capture: bool,
}

/// Audio device trait
pub trait AudioDevice: Send + Sync {
    /// Get device name
    fn name(&self) -> &str;

    /// Get device capabilities
    fn capabilities(&self) -> AudioCapabilities;

    /// Open a playback stream
    fn open_playback(&mut self, config: &AudioConfig) -> Result<StreamHandle, &'static str>;

    /// Open a capture stream
    fn open_capture(&mut self, config: &AudioConfig) -> Result<StreamHandle, &'static str>;

    /// Close a stream
    fn close_stream(&mut self, handle: StreamHandle) -> Result<(), &'static str>;

    /// Start a stream
    fn start_stream(&mut self, handle: StreamHandle) -> Result<(), &'static str>;

    /// Stop a stream
    fn stop_stream(&mut self, handle: StreamHandle) -> Result<(), &'static str>;

    /// Write samples to playback stream
    fn write(&mut self, handle: StreamHandle, data: &[u8]) -> Result<usize, &'static str>;

    /// Read samples from capture stream
    fn read(&mut self, handle: StreamHandle, data: &mut [u8]) -> Result<usize, &'static str>;

    /// Get stream state
    fn stream_state(&self, handle: StreamHandle) -> Option<StreamState>;

    /// Get available bytes in buffer (for writing or reading)
    fn available(&self, handle: StreamHandle) -> usize;

    /// Set stream volume (0-100)
    fn set_volume(&mut self, handle: StreamHandle, volume: u8) -> Result<(), &'static str>;

    /// Get stream volume
    fn get_volume(&self, handle: StreamHandle) -> u8;

    /// Mute/unmute stream
    fn set_mute(&mut self, handle: StreamHandle, muted: bool) -> Result<(), &'static str>;

    /// Check if muted
    fn is_muted(&self, handle: StreamHandle) -> bool;
}

/// Global audio system
pub static AUDIO_SYSTEM: Mutex<AudioSystem> = Mutex::new(AudioSystem::new());

/// Audio system manager
pub struct AudioSystem {
    devices: Vec<hda::HdaController>,
    default_playback: Option<usize>,
    default_capture: Option<usize>,
    master_volume: u8,
    master_muted: bool,
}

impl AudioSystem {
    pub const fn new() -> Self {
        AudioSystem {
            devices: Vec::new(),
            default_playback: None,
            default_capture: None,
            master_volume: 100,
            master_muted: false,
        }
    }

    /// Register an audio device
    pub fn register_device(&mut self, device: hda::HdaController) {
        let idx = self.devices.len();
        self.devices.push(device);

        if self.default_playback.is_none() {
            self.default_playback = Some(idx);
        }
        if self.default_capture.is_none() {
            self.default_capture = Some(idx);
        }
    }

    /// Get number of devices
    pub fn device_count(&self) -> usize {
        self.devices.len()
    }

    /// Get device by index
    pub fn get_device(&mut self, index: usize) -> Option<&mut hda::HdaController> {
        self.devices.get_mut(index)
    }

    /// Get default playback device
    pub fn default_playback_device(&mut self) -> Option<&mut hda::HdaController> {
        self.default_playback.and_then(|idx| self.devices.get_mut(idx))
    }

    /// Get default capture device
    pub fn default_capture_device(&mut self) -> Option<&mut hda::HdaController> {
        self.default_capture.and_then(|idx| self.devices.get_mut(idx))
    }

    /// Set master volume
    pub fn set_master_volume(&mut self, volume: u8) {
        self.master_volume = volume.min(100);
    }

    /// Get master volume
    pub fn master_volume(&self) -> u8 {
        self.master_volume
    }

    /// Set master mute
    pub fn set_master_mute(&mut self, muted: bool) {
        self.master_muted = muted;
    }

    /// Check if master is muted
    pub fn is_master_muted(&self) -> bool {
        self.master_muted
    }
}

/// Initialize audio subsystem
pub fn init() {
    crate::kprintln!("audio: initializing audio subsystem");
    hda::init();
    ac97::init();
    mixer::init();
    crate::kprintln!("audio: audio subsystem ready");
}

/// Play a simple beep (for system sounds)
pub fn beep(frequency: u32, duration_ms: u32) {
    crate::kprintln!("audio: beep {}Hz for {}ms", frequency, duration_ms);
    // In a full implementation, this would use a hardware timer or
    // generate a sine wave through the audio driver
}
