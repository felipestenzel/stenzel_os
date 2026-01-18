//! USB Audio Class Driver
//!
//! Implements USB Audio Class 1.0 and 2.0 for:
//! - USB DACs and sound cards
//! - USB microphones
//! - USB headsets
//! - USB audio interfaces for professional audio

#![allow(dead_code)]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::sync::IrqSafeMutex;

use super::daemon::{self, NodeId, NodeType, ClientProtocol};
use super::{AudioConfig, SampleFormat, StreamDirection};

/// USB Audio Class version
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbAudioClass {
    /// UAC 1.0 (USB 1.1/2.0)
    Uac1,
    /// UAC 2.0 (USB 2.0)
    Uac2,
    /// UAC 3.0 (USB 3.0+)
    Uac3,
}

/// USB Audio subclass
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbAudioSubclass {
    Undefined,
    AudioControl,
    AudioStreaming,
    MidiStreaming,
}

/// Audio terminal type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalType {
    // Input terminals
    UsbStreaming,
    Microphone,
    MicrophoneArray,
    ProcessingMic,

    // Output terminals
    Speaker,
    Headphones,
    HeadMounted,
    Desktop,
    RoomSpeaker,
    CommSpeaker,
    LowFreqEffects,

    // Bidirectional terminals
    Handset,
    Headset,
    Speakerphone,

    // Embedded function
    LineConnector,
    SpdifInterface,
    Ieee1394Av,
    VendorSpecific,
}

/// Sample frequency support
#[derive(Debug, Clone)]
pub enum SampleFrequencySupport {
    /// Discrete list of frequencies
    Discrete(Vec<u32>),
    /// Continuous range (min, max)
    Continuous { min: u32, max: u32 },
}

/// USB Audio endpoint descriptor
#[derive(Debug, Clone)]
pub struct UsbAudioEndpoint {
    pub address: u8,
    pub direction: StreamDirection,
    pub max_packet_size: u16,
    pub interval: u8,
    pub refresh: u8,
    pub synch_address: u8,
    pub attributes: u8,
    pub lock_delay_units: u8,
    pub lock_delay: u16,
}

impl UsbAudioEndpoint {
    /// Is this an isochronous endpoint?
    pub fn is_isochronous(&self) -> bool {
        (self.attributes & 0x03) == 0x01
    }

    /// Get synchronization type
    pub fn sync_type(&self) -> UsbAudioSyncType {
        match (self.attributes >> 2) & 0x03 {
            0 => UsbAudioSyncType::None,
            1 => UsbAudioSyncType::Asynchronous,
            2 => UsbAudioSyncType::Adaptive,
            3 => UsbAudioSyncType::Synchronous,
            _ => UsbAudioSyncType::None,
        }
    }
}

/// USB Audio synchronization type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbAudioSyncType {
    None,
    Asynchronous,
    Adaptive,
    Synchronous,
}

/// USB Audio format type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbAudioFormatType {
    TypeI,      // PCM
    TypeII,     // Compressed (AC-3, MPEG)
    TypeIII,    // High-precision PCM
    TypeIV,     // UAC3 extended
    Extended,
}

/// USB Audio format descriptor
#[derive(Debug, Clone)]
pub struct UsbAudioFormat {
    pub format_type: UsbAudioFormatType,
    pub channels: u8,
    pub subframe_size: u8,  // bytes per sample
    pub bit_resolution: u8, // actual bits used
    pub sample_frequencies: SampleFrequencySupport,
}

impl UsbAudioFormat {
    /// Get sample format
    pub fn sample_format(&self) -> Option<SampleFormat> {
        match (self.subframe_size, self.bit_resolution) {
            (1, 8) => Some(SampleFormat::U8),
            (2, 16) => Some(SampleFormat::S16LE),
            (3, 24) => Some(SampleFormat::S24LE),
            (4, 24) => Some(SampleFormat::S24LE), // 24-bit in 32-bit container
            (4, 32) => Some(SampleFormat::S32LE),
            _ => None,
        }
    }

    /// Get supported sample rates
    pub fn sample_rates(&self) -> Vec<u32> {
        match &self.sample_frequencies {
            SampleFrequencySupport::Discrete(rates) => rates.clone(),
            SampleFrequencySupport::Continuous { min, max } => {
                // Return common rates within range
                let common = [8000, 11025, 16000, 22050, 32000, 44100, 48000,
                             88200, 96000, 176400, 192000, 352800, 384000];
                common.iter()
                    .filter(|&&r| r >= *min && r <= *max)
                    .copied()
                    .collect()
            }
        }
    }
}

/// USB Audio streaming interface
#[derive(Debug, Clone)]
pub struct UsbAudioStreamingInterface {
    pub interface_number: u8,
    pub alternate_setting: u8,
    pub terminal_link: u8,
    pub format: UsbAudioFormat,
    pub endpoint: Option<UsbAudioEndpoint>,
}

/// USB Audio feature unit (volume, mute, etc.)
#[derive(Debug, Clone)]
pub struct UsbAudioFeatureUnit {
    pub unit_id: u8,
    pub source_id: u8,
    pub channels: u8,
    pub mute: Vec<bool>,        // Per-channel mute
    pub volume: Vec<i16>,       // Per-channel volume (in 1/256 dB)
    pub bass: Option<i8>,
    pub mid: Option<i8>,
    pub treble: Option<i8>,
    pub graphic_eq: bool,
    pub automatic_gain: bool,
    pub delay: Option<u16>,
    pub bass_boost: bool,
    pub loudness: bool,
}

impl UsbAudioFeatureUnit {
    /// Create with default values
    pub fn new(unit_id: u8, source_id: u8, channels: u8) -> Self {
        Self {
            unit_id,
            source_id,
            channels,
            mute: vec![false; channels as usize],
            volume: vec![0; channels as usize],
            bass: None,
            mid: None,
            treble: None,
            graphic_eq: false,
            automatic_gain: false,
            delay: None,
            bass_boost: false,
            loudness: false,
        }
    }

    /// Set volume for channel (in percent 0-100)
    pub fn set_volume_percent(&mut self, channel: usize, percent: u8) {
        if channel < self.volume.len() {
            // Convert 0-100 to dB range (typically -127 to 0 dB)
            // Using 256ths of dB internally
            // Use simplified linear-to-dB approximation suitable for no_std
            let db = if percent == 0 {
                -32768 // Mute
            } else {
                // Simple approximation: 0% = -60dB, 100% = 0dB
                let db_value = ((percent as i32 - 100) * 60) / 100;
                (db_value * 256) as i16
            };
            self.volume[channel] = db;
        }
    }

    /// Get volume for channel (in percent 0-100)
    pub fn volume_percent(&self, channel: usize) -> u8 {
        if channel >= self.volume.len() {
            return 0;
        }
        let db_256 = self.volume[channel];
        if db_256 <= -32768 {
            return 0;
        }
        // Simple approximation: -60dB = 0%, 0dB = 100%
        let db = db_256 / 256;
        let percent = ((db as i32 + 60) * 100 / 60).clamp(0, 100) as u8;
        percent
    }
}

/// USB Audio mixer unit
#[derive(Debug, Clone)]
pub struct UsbAudioMixerUnit {
    pub unit_id: u8,
    pub source_ids: Vec<u8>,
    pub input_channels: u8,
    pub output_channels: u8,
    pub mix_controls: Vec<Vec<bool>>, // [input][output] matrix
}

/// USB Audio processing unit
#[derive(Debug, Clone)]
pub struct UsbAudioProcessingUnit {
    pub unit_id: u8,
    pub process_type: ProcessingType,
    pub source_ids: Vec<u8>,
    pub channels: u8,
}

/// Processing unit type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingType {
    UpDownMix,
    DolbyProLogic,
    StereoExtender,
    Reverberation,
    Chorus,
    DynamicRangeCompressor,
}

/// USB Audio device
pub struct UsbAudioDevice {
    /// Device ID
    id: u32,
    /// USB address
    usb_address: u8,
    /// Vendor ID
    vendor_id: u16,
    /// Product ID
    product_id: u16,
    /// Device name
    name: String,
    /// USB Audio Class version
    class_version: UsbAudioClass,
    /// Input terminals
    input_terminals: Vec<(u8, TerminalType)>,
    /// Output terminals
    output_terminals: Vec<(u8, TerminalType)>,
    /// Feature units
    feature_units: BTreeMap<u8, UsbAudioFeatureUnit>,
    /// Streaming interfaces
    streaming_interfaces: Vec<UsbAudioStreamingInterface>,
    /// Current sample rate
    sample_rate: u32,
    /// Active playback interface
    active_playback: Option<u8>,
    /// Active capture interface
    active_capture: Option<u8>,
    /// Connected
    connected: AtomicBool,
    /// Audio daemon node ID (sink)
    sink_node_id: Option<NodeId>,
    /// Audio daemon node ID (source)
    source_node_id: Option<NodeId>,
}

impl UsbAudioDevice {
    /// Create new USB audio device
    pub fn new(id: u32, usb_address: u8, vendor_id: u16, product_id: u16, name: &str) -> Self {
        Self {
            id,
            usb_address,
            vendor_id,
            product_id,
            name: name.to_string(),
            class_version: UsbAudioClass::Uac1,
            input_terminals: Vec::new(),
            output_terminals: Vec::new(),
            feature_units: BTreeMap::new(),
            streaming_interfaces: Vec::new(),
            sample_rate: 48000,
            active_playback: None,
            active_capture: None,
            connected: AtomicBool::new(false),
            sink_node_id: None,
            source_node_id: None,
        }
    }

    /// Get device ID
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Get device name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get vendor and product ID
    pub fn ids(&self) -> (u16, u16) {
        (self.vendor_id, self.product_id)
    }

    /// Set USB Audio class version
    pub fn set_class_version(&mut self, version: UsbAudioClass) {
        self.class_version = version;
    }

    /// Add input terminal
    pub fn add_input_terminal(&mut self, id: u8, terminal_type: TerminalType) {
        self.input_terminals.push((id, terminal_type));
    }

    /// Add output terminal
    pub fn add_output_terminal(&mut self, id: u8, terminal_type: TerminalType) {
        self.output_terminals.push((id, terminal_type));
    }

    /// Add feature unit
    pub fn add_feature_unit(&mut self, unit: UsbAudioFeatureUnit) {
        self.feature_units.insert(unit.unit_id, unit);
    }

    /// Add streaming interface
    pub fn add_streaming_interface(&mut self, interface: UsbAudioStreamingInterface) {
        self.streaming_interfaces.push(interface);
    }

    /// Get supported sample rates for playback
    pub fn playback_sample_rates(&self) -> Vec<u32> {
        self.streaming_interfaces
            .iter()
            .filter(|i| i.endpoint.as_ref().map(|e| matches!(e.direction, StreamDirection::Playback)).unwrap_or(false))
            .flat_map(|i| i.format.sample_rates())
            .collect()
    }

    /// Get supported sample rates for capture
    pub fn capture_sample_rates(&self) -> Vec<u32> {
        self.streaming_interfaces
            .iter()
            .filter(|i| i.endpoint.as_ref().map(|e| matches!(e.direction, StreamDirection::Capture)).unwrap_or(false))
            .flat_map(|i| i.format.sample_rates())
            .collect()
    }

    /// Get max bit depth for playback
    pub fn max_playback_bit_depth(&self) -> u8 {
        self.streaming_interfaces
            .iter()
            .filter(|i| i.endpoint.as_ref().map(|e| matches!(e.direction, StreamDirection::Playback)).unwrap_or(false))
            .map(|i| i.format.bit_resolution)
            .max()
            .unwrap_or(16)
    }

    /// Get max channels for playback
    pub fn max_playback_channels(&self) -> u8 {
        self.streaming_interfaces
            .iter()
            .filter(|i| i.endpoint.as_ref().map(|e| matches!(e.direction, StreamDirection::Playback)).unwrap_or(false))
            .map(|i| i.format.channels)
            .max()
            .unwrap_or(2)
    }

    /// Connect device
    pub fn connect(&mut self) -> bool {
        if self.connected.load(Ordering::Relaxed) {
            return true;
        }

        // Register with audio daemon
        if let Some(client_id) = daemon::connect_client(&self.name, ClientProtocol::Native) {
            // Check if device has playback capability
            let has_playback = self.streaming_interfaces.iter().any(|i| {
                i.endpoint.as_ref().map(|e| matches!(e.direction, StreamDirection::Playback)).unwrap_or(false)
            });

            if has_playback {
                let sink_id = daemon::with_daemon_mut(|d| {
                    let id = d.create_node(&self.name, NodeType::Sink);
                    if let Some(node) = d.get_node_mut(id) {
                        node.set_property("device.class", "usb");
                        node.set_property("device.vendor_id", &alloc::format!("{:04x}", self.vendor_id));
                        node.set_property("device.product_id", &alloc::format!("{:04x}", self.product_id));
                    }
                    id
                });
                self.sink_node_id = sink_id;
            }

            // Check if device has capture capability
            let has_capture = self.streaming_interfaces.iter().any(|i| {
                i.endpoint.as_ref().map(|e| matches!(e.direction, StreamDirection::Capture)).unwrap_or(false)
            });

            if has_capture {
                let source_id = daemon::with_daemon_mut(|d| {
                    let id = d.create_node(&alloc::format!("{} (Mic)", self.name), NodeType::Source);
                    if let Some(node) = d.get_node_mut(id) {
                        node.set_property("device.class", "usb");
                    }
                    id
                });
                self.source_node_id = source_id;
            }
        }

        self.connected.store(true, Ordering::Relaxed);
        crate::kprintln!("usb_audio: connected device {}", self.name);
        true
    }

    /// Disconnect device
    pub fn disconnect(&mut self) {
        if let Some(sink_id) = self.sink_node_id.take() {
            daemon::with_daemon_mut(|d| d.remove_node(sink_id));
        }
        if let Some(source_id) = self.source_node_id.take() {
            daemon::with_daemon_mut(|d| d.remove_node(source_id));
        }

        self.connected.store(false, Ordering::Relaxed);
        self.active_playback = None;
        self.active_capture = None;
        crate::kprintln!("usb_audio: disconnected device {}", self.name);
    }

    /// Is connected
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    /// Set sample rate
    pub fn set_sample_rate(&mut self, rate: u32) -> bool {
        // Check if rate is supported
        let playback_rates = self.playback_sample_rates();
        let capture_rates = self.capture_sample_rates();

        if !playback_rates.is_empty() && !playback_rates.contains(&rate) {
            return false;
        }
        if !capture_rates.is_empty() && !capture_rates.contains(&rate) {
            return false;
        }

        self.sample_rate = rate;
        crate::kprintln!("usb_audio: set sample rate to {} Hz", rate);
        true
    }

    /// Get current sample rate
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Set volume (master)
    pub fn set_volume(&mut self, percent: u8) {
        // Apply to first feature unit found
        if let Some((_, unit)) = self.feature_units.iter_mut().next() {
            for ch in 0..unit.channels as usize {
                unit.set_volume_percent(ch, percent);
            }
        }

        // Update daemon node
        if let Some(sink_id) = self.sink_node_id {
            daemon::set_stream_volume(sink_id, percent);
        }
    }

    /// Set mute
    pub fn set_mute(&mut self, muted: bool) {
        if let Some((_, unit)) = self.feature_units.iter_mut().next() {
            for ch in 0..unit.mute.len() {
                unit.mute[ch] = muted;
            }
        }

        if let Some(sink_id) = self.sink_node_id {
            daemon::set_stream_mute(sink_id, muted);
        }
    }
}

/// USB Audio device info
#[derive(Debug, Clone)]
pub struct UsbAudioDeviceInfo {
    pub id: u32,
    pub name: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub has_playback: bool,
    pub has_capture: bool,
    pub max_sample_rate: u32,
    pub max_bit_depth: u8,
    pub max_channels: u8,
    pub is_connected: bool,
}

/// USB Audio manager
pub struct UsbAudioManager {
    devices: BTreeMap<u32, UsbAudioDevice>,
    next_device_id: AtomicU32,
    default_playback: Option<u32>,
    default_capture: Option<u32>,
}

impl UsbAudioManager {
    /// Create new manager
    pub const fn new() -> Self {
        Self {
            devices: BTreeMap::new(),
            next_device_id: AtomicU32::new(1),
            default_playback: None,
            default_capture: None,
        }
    }

    /// Register a USB audio device
    pub fn register_device(
        &mut self,
        usb_address: u8,
        vendor_id: u16,
        product_id: u16,
        name: &str,
    ) -> u32 {
        let id = self.next_device_id.fetch_add(1, Ordering::Relaxed);
        let device = UsbAudioDevice::new(id, usb_address, vendor_id, product_id, name);
        self.devices.insert(id, device);
        crate::kprintln!("usb_audio: registered device {} ({:04x}:{:04x})", name, vendor_id, product_id);
        id
    }

    /// Unregister device
    pub fn unregister_device(&mut self, id: u32) {
        if let Some(mut device) = self.devices.remove(&id) {
            device.disconnect();
        }

        if self.default_playback == Some(id) {
            self.default_playback = None;
        }
        if self.default_capture == Some(id) {
            self.default_capture = None;
        }
    }

    /// Get device by ID
    pub fn get_device(&self, id: u32) -> Option<&UsbAudioDevice> {
        self.devices.get(&id)
    }

    /// Get device by ID (mutable)
    pub fn get_device_mut(&mut self, id: u32) -> Option<&mut UsbAudioDevice> {
        self.devices.get_mut(&id)
    }

    /// Connect a device
    pub fn connect_device(&mut self, id: u32) -> bool {
        if let Some(device) = self.devices.get_mut(&id) {
            if device.connect() {
                // Set as default if none set
                let has_playback = device.streaming_interfaces.iter().any(|i| {
                    i.endpoint.as_ref().map(|e| matches!(e.direction, StreamDirection::Playback)).unwrap_or(false)
                });
                let has_capture = device.streaming_interfaces.iter().any(|i| {
                    i.endpoint.as_ref().map(|e| matches!(e.direction, StreamDirection::Capture)).unwrap_or(false)
                });

                if has_playback && self.default_playback.is_none() {
                    self.default_playback = Some(id);
                }
                if has_capture && self.default_capture.is_none() {
                    self.default_capture = Some(id);
                }
                return true;
            }
        }
        false
    }

    /// Disconnect device
    pub fn disconnect_device(&mut self, id: u32) {
        if let Some(device) = self.devices.get_mut(&id) {
            device.disconnect();
        }

        if self.default_playback == Some(id) {
            self.default_playback = self.devices.iter()
                .find(|(_, d)| d.is_connected())
                .map(|(id, _)| *id);
        }
        if self.default_capture == Some(id) {
            self.default_capture = self.devices.iter()
                .find(|(_, d)| d.is_connected())
                .map(|(id, _)| *id);
        }
    }

    /// List all devices
    pub fn list_devices(&self) -> Vec<UsbAudioDeviceInfo> {
        self.devices.values().map(|d| {
            let has_playback = d.streaming_interfaces.iter().any(|i| {
                i.endpoint.as_ref().map(|e| matches!(e.direction, StreamDirection::Playback)).unwrap_or(false)
            });
            let has_capture = d.streaming_interfaces.iter().any(|i| {
                i.endpoint.as_ref().map(|e| matches!(e.direction, StreamDirection::Capture)).unwrap_or(false)
            });

            UsbAudioDeviceInfo {
                id: d.id(),
                name: d.name().to_string(),
                vendor_id: d.vendor_id,
                product_id: d.product_id,
                has_playback,
                has_capture,
                max_sample_rate: d.playback_sample_rates().into_iter().max().unwrap_or(48000),
                max_bit_depth: d.max_playback_bit_depth(),
                max_channels: d.max_playback_channels(),
                is_connected: d.is_connected(),
            }
        }).collect()
    }

    /// Get default playback device
    pub fn default_playback_device(&self) -> Option<&UsbAudioDevice> {
        self.default_playback.and_then(|id| self.devices.get(&id))
    }

    /// Set default playback device
    pub fn set_default_playback(&mut self, id: u32) -> bool {
        if self.devices.get(&id).map(|d| d.is_connected()).unwrap_or(false) {
            self.default_playback = Some(id);
            true
        } else {
            false
        }
    }
}

// =============================================================================
// Global instance
// =============================================================================

static USB_AUDIO: IrqSafeMutex<UsbAudioManager> = IrqSafeMutex::new(UsbAudioManager::new());

/// Initialize USB audio
pub fn init() {
    crate::kprintln!("usb_audio: initialized");
}

/// Register USB audio device
pub fn register_device(usb_address: u8, vendor_id: u16, product_id: u16, name: &str) -> u32 {
    USB_AUDIO.lock().register_device(usb_address, vendor_id, product_id, name)
}

/// Configure device streaming interface
pub fn configure_interface(id: u32, interface: UsbAudioStreamingInterface) {
    if let Some(device) = USB_AUDIO.lock().get_device_mut(id) {
        device.add_streaming_interface(interface);
    }
}

/// Connect device
pub fn connect_device(id: u32) -> bool {
    USB_AUDIO.lock().connect_device(id)
}

/// Disconnect device
pub fn disconnect_device(id: u32) {
    USB_AUDIO.lock().disconnect_device(id);
}

/// List devices
pub fn list_devices() -> Vec<UsbAudioDeviceInfo> {
    USB_AUDIO.lock().list_devices()
}

/// Set device volume
pub fn set_volume(id: u32, volume: u8) {
    if let Some(device) = USB_AUDIO.lock().get_device_mut(id) {
        device.set_volume(volume);
    }
}

/// Set device mute
pub fn set_mute(id: u32, muted: bool) {
    if let Some(device) = USB_AUDIO.lock().get_device_mut(id) {
        device.set_mute(muted);
    }
}

/// Set sample rate
pub fn set_sample_rate(id: u32, rate: u32) -> bool {
    USB_AUDIO.lock()
        .get_device_mut(id)
        .map(|d| d.set_sample_rate(rate))
        .unwrap_or(false)
}
