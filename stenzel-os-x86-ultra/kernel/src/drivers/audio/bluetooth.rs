//! Bluetooth Audio Support
//!
//! Provides Bluetooth audio functionality:
//! - A2DP (Advanced Audio Distribution Profile) for stereo audio streaming
//! - HFP (Hands-Free Profile) for voice calls
//! - AVRCP (Audio/Video Remote Control Profile) for media controls
//! - Support for high-quality codecs (SBC, AAC, aptX, aptX HD, LDAC)

#![allow(dead_code)]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::sync::IrqSafeMutex;

use super::daemon::{self, NodeId, NodeType, ClientProtocol};
use super::{SampleFormat, StreamDirection};

/// Bluetooth audio codec type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BluetoothCodec {
    /// Sub-band Coding (mandatory A2DP codec)
    Sbc,
    /// Advanced Audio Coding
    Aac,
    /// Qualcomm aptX
    AptX,
    /// Qualcomm aptX HD
    AptXHd,
    /// Qualcomm aptX Low Latency
    AptXLl,
    /// Qualcomm aptX Adaptive
    AptXAdaptive,
    /// Sony LDAC
    Ldac,
    /// LC3 (LE Audio)
    Lc3,
    /// CVSD (voice, 64 kbps)
    Cvsd,
    /// mSBC (voice, wideband)
    MSbc,
}

impl BluetoothCodec {
    /// Get codec name
    pub fn name(&self) -> &'static str {
        match self {
            BluetoothCodec::Sbc => "SBC",
            BluetoothCodec::Aac => "AAC",
            BluetoothCodec::AptX => "aptX",
            BluetoothCodec::AptXHd => "aptX HD",
            BluetoothCodec::AptXLl => "aptX LL",
            BluetoothCodec::AptXAdaptive => "aptX Adaptive",
            BluetoothCodec::Ldac => "LDAC",
            BluetoothCodec::Lc3 => "LC3",
            BluetoothCodec::Cvsd => "CVSD",
            BluetoothCodec::MSbc => "mSBC",
        }
    }

    /// Get typical bitrate range (kbps)
    pub fn bitrate_range(&self) -> (u32, u32) {
        match self {
            BluetoothCodec::Sbc => (128, 345),
            BluetoothCodec::Aac => (128, 256),
            BluetoothCodec::AptX => (352, 352),
            BluetoothCodec::AptXHd => (576, 576),
            BluetoothCodec::AptXLl => (352, 352),
            BluetoothCodec::AptXAdaptive => (280, 420),
            BluetoothCodec::Ldac => (330, 990),
            BluetoothCodec::Lc3 => (32, 320),
            BluetoothCodec::Cvsd => (64, 64),
            BluetoothCodec::MSbc => (64, 64),
        }
    }

    /// Get sample rate support
    pub fn supported_sample_rates(&self) -> Vec<u32> {
        match self {
            BluetoothCodec::Sbc => vec![16000, 32000, 44100, 48000],
            BluetoothCodec::Aac => vec![44100, 48000],
            BluetoothCodec::AptX | BluetoothCodec::AptXLl => vec![44100, 48000],
            BluetoothCodec::AptXHd | BluetoothCodec::AptXAdaptive => vec![44100, 48000, 96000],
            BluetoothCodec::Ldac => vec![44100, 48000, 88200, 96000],
            BluetoothCodec::Lc3 => vec![8000, 16000, 24000, 32000, 48000],
            BluetoothCodec::Cvsd => vec![8000],
            BluetoothCodec::MSbc => vec![16000],
        }
    }

    /// Is this a voice codec?
    pub fn is_voice_codec(&self) -> bool {
        matches!(self, BluetoothCodec::Cvsd | BluetoothCodec::MSbc)
    }
}

/// Bluetooth audio profile
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BluetoothProfile {
    /// Advanced Audio Distribution Profile (stereo audio)
    A2dp,
    /// Hands-Free Profile (voice calls)
    Hfp,
    /// Headset Profile (legacy voice)
    Hsp,
    /// Audio/Video Remote Control Profile
    Avrcp,
    /// LE Audio
    LeAudio,
}

/// LDAC quality mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LdacQuality {
    /// Best quality (990 kbps)
    High,
    /// Standard quality (660 kbps)
    Standard,
    /// Connection priority (330 kbps)
    Mobile,
    /// Adaptive quality
    Adaptive,
}

/// A2DP capabilities
#[derive(Debug, Clone)]
pub struct A2dpCapabilities {
    pub codecs: Vec<BluetoothCodec>,
    pub sample_rates: Vec<u32>,
    pub channel_modes: Vec<A2dpChannelMode>,
    pub bit_depths: Vec<u8>,
}

impl Default for A2dpCapabilities {
    fn default() -> Self {
        Self {
            codecs: vec![BluetoothCodec::Sbc, BluetoothCodec::Aac],
            sample_rates: vec![44100, 48000],
            channel_modes: vec![A2dpChannelMode::Stereo, A2dpChannelMode::JointStereo],
            bit_depths: vec![16, 24],
        }
    }
}

/// A2DP channel mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum A2dpChannelMode {
    Mono,
    DualChannel,
    Stereo,
    JointStereo,
}

/// HFP features
#[derive(Debug, Clone, Copy)]
pub struct HfpFeatures {
    pub echo_cancellation: bool,
    pub noise_reduction: bool,
    pub voice_recognition: bool,
    pub wideband_speech: bool,
    pub super_wideband: bool,
    pub codec_negotiation: bool,
}

impl Default for HfpFeatures {
    fn default() -> Self {
        Self {
            echo_cancellation: true,
            noise_reduction: true,
            voice_recognition: true,
            wideband_speech: true,
            super_wideband: false,
            codec_negotiation: true,
        }
    }
}

/// AVRCP player status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvrcpPlayStatus {
    Stopped,
    Playing,
    Paused,
    FwdSeek,
    RevSeek,
    Error,
}

/// AVRCP commands
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvrcpCommand {
    Play,
    Pause,
    Stop,
    Next,
    Previous,
    FastForward,
    Rewind,
    VolumeUp,
    VolumeDown,
    Mute,
}

/// Bluetooth audio device
pub struct BluetoothAudioDevice {
    /// Device ID
    id: u32,
    /// Bluetooth device address
    address: [u8; 6],
    /// Device name
    name: String,
    /// Supported profiles
    profiles: Vec<BluetoothProfile>,
    /// A2DP capabilities
    a2dp_caps: Option<A2dpCapabilities>,
    /// HFP features
    hfp_features: Option<HfpFeatures>,
    /// Currently connected profiles
    connected_profiles: Vec<BluetoothProfile>,
    /// Active codec
    active_codec: Option<BluetoothCodec>,
    /// Current sample rate
    sample_rate: u32,
    /// Volume (0-100)
    volume: AtomicU32,
    /// Muted
    muted: AtomicBool,
    /// Audio daemon node ID (for A2DP sink)
    sink_node_id: Option<NodeId>,
    /// Audio daemon node ID (for A2DP source/capture)
    source_node_id: Option<NodeId>,
    /// Connected
    connected: AtomicBool,
    /// Battery level (0-100, if supported)
    battery_level: Option<u8>,
    /// LDAC quality mode
    ldac_quality: LdacQuality,
}

impl BluetoothAudioDevice {
    /// Create a new Bluetooth audio device
    pub fn new(id: u32, address: [u8; 6], name: &str) -> Self {
        Self {
            id,
            address,
            name: name.to_string(),
            profiles: Vec::new(),
            a2dp_caps: None,
            hfp_features: None,
            connected_profiles: Vec::new(),
            active_codec: None,
            sample_rate: 48000,
            volume: AtomicU32::new(100),
            muted: AtomicBool::new(false),
            sink_node_id: None,
            source_node_id: None,
            connected: AtomicBool::new(false),
            battery_level: None,
            ldac_quality: LdacQuality::Standard,
        }
    }

    /// Get device ID
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Get Bluetooth address as string
    pub fn address_string(&self) -> String {
        alloc::format!(
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            self.address[0], self.address[1], self.address[2],
            self.address[3], self.address[4], self.address[5]
        )
    }

    /// Get device name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Add supported profile
    pub fn add_profile(&mut self, profile: BluetoothProfile) {
        if !self.profiles.contains(&profile) {
            self.profiles.push(profile);
        }
    }

    /// Check if profile is supported
    pub fn supports_profile(&self, profile: BluetoothProfile) -> bool {
        self.profiles.contains(&profile)
    }

    /// Set A2DP capabilities
    pub fn set_a2dp_capabilities(&mut self, caps: A2dpCapabilities) {
        self.a2dp_caps = Some(caps);
    }

    /// Get A2DP capabilities
    pub fn a2dp_capabilities(&self) -> Option<&A2dpCapabilities> {
        self.a2dp_caps.as_ref()
    }

    /// Set HFP features
    pub fn set_hfp_features(&mut self, features: HfpFeatures) {
        self.hfp_features = Some(features);
    }

    /// Get HFP features
    pub fn hfp_features(&self) -> Option<&HfpFeatures> {
        self.hfp_features.as_ref()
    }

    /// Connect A2DP profile
    pub fn connect_a2dp(&mut self, codec: BluetoothCodec) -> bool {
        if !self.supports_profile(BluetoothProfile::A2dp) {
            return false;
        }

        // Check if codec is supported
        if let Some(caps) = &self.a2dp_caps {
            if !caps.codecs.contains(&codec) {
                return false;
            }
        }

        self.active_codec = Some(codec);
        self.connected_profiles.push(BluetoothProfile::A2dp);
        self.connected.store(true, Ordering::Relaxed);

        // Register with audio daemon
        if let Some(client_id) = daemon::connect_client(&self.name, ClientProtocol::Native) {
            let sink_id = daemon::with_daemon_mut(|d| {
                let id = d.create_node(&alloc::format!("{} (A2DP)", self.name), NodeType::Sink);
                if let Some(node) = d.get_node_mut(id) {
                    node.set_property("device.class", "bluetooth");
                    node.set_property("bluetooth.codec", codec.name());
                }
                id
            });
            self.sink_node_id = sink_id;
        }

        crate::kprintln!("bluetooth_audio: connected A2DP with {} codec", codec.name());
        true
    }

    /// Connect HFP profile
    pub fn connect_hfp(&mut self) -> bool {
        if !self.supports_profile(BluetoothProfile::Hfp) {
            return false;
        }

        self.connected_profiles.push(BluetoothProfile::Hfp);
        self.connected.store(true, Ordering::Relaxed);

        // Use mSBC for wideband speech if supported
        let codec = if self.hfp_features.as_ref().map(|f| f.wideband_speech).unwrap_or(false) {
            BluetoothCodec::MSbc
        } else {
            BluetoothCodec::Cvsd
        };

        // Register bidirectional audio with daemon
        if let Some(client_id) = daemon::connect_client(&self.name, ClientProtocol::Native) {
            // Create source for microphone input
            let source_id = daemon::with_daemon_mut(|d| {
                let id = d.create_node(&alloc::format!("{} (HFP Mic)", self.name), NodeType::Source);
                if let Some(node) = d.get_node_mut(id) {
                    node.set_property("device.class", "bluetooth");
                    node.set_property("bluetooth.profile", "hfp");
                }
                id
            });
            self.source_node_id = source_id;

            // Create sink for speaker output
            let sink_id = daemon::with_daemon_mut(|d| {
                let id = d.create_node(&alloc::format!("{} (HFP Speaker)", self.name), NodeType::Sink);
                if let Some(node) = d.get_node_mut(id) {
                    node.set_property("device.class", "bluetooth");
                    node.set_property("bluetooth.profile", "hfp");
                }
                id
            });
            self.sink_node_id = sink_id;
        }

        crate::kprintln!("bluetooth_audio: connected HFP with {} codec", codec.name());
        true
    }

    /// Disconnect
    pub fn disconnect(&mut self) {
        self.connected_profiles.clear();
        self.active_codec = None;
        self.connected.store(false, Ordering::Relaxed);

        // Remove from audio daemon
        if let Some(sink_id) = self.sink_node_id.take() {
            daemon::with_daemon_mut(|d| d.remove_node(sink_id));
        }
        if let Some(source_id) = self.source_node_id.take() {
            daemon::with_daemon_mut(|d| d.remove_node(source_id));
        }

        crate::kprintln!("bluetooth_audio: disconnected {}", self.name);
    }

    /// Is connected
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    /// Get active codec
    pub fn active_codec(&self) -> Option<BluetoothCodec> {
        self.active_codec
    }

    /// Set volume
    pub fn set_volume(&self, volume: u8) {
        self.volume.store(volume.min(100) as u32, Ordering::Relaxed);
        if let Some(sink_id) = self.sink_node_id {
            daemon::set_stream_volume(sink_id, volume);
        }
    }

    /// Get volume
    pub fn volume(&self) -> u8 {
        self.volume.load(Ordering::Relaxed) as u8
    }

    /// Set mute
    pub fn set_mute(&self, muted: bool) {
        self.muted.store(muted, Ordering::Relaxed);
        if let Some(sink_id) = self.sink_node_id {
            daemon::set_stream_mute(sink_id, muted);
        }
    }

    /// Is muted
    pub fn is_muted(&self) -> bool {
        self.muted.load(Ordering::Relaxed)
    }

    /// Set LDAC quality
    pub fn set_ldac_quality(&mut self, quality: LdacQuality) {
        self.ldac_quality = quality;
    }

    /// Get battery level
    pub fn battery_level(&self) -> Option<u8> {
        self.battery_level
    }

    /// Update battery level
    pub fn set_battery_level(&mut self, level: u8) {
        self.battery_level = Some(level.min(100));
    }
}

/// AVRCP controller for media control
pub struct AvrcpController {
    device_id: u32,
    play_status: AvrcpPlayStatus,
    position_ms: u64,
    track_length_ms: u64,
    track_title: String,
    track_artist: String,
    track_album: String,
    volume: u8,
}

impl AvrcpController {
    /// Create new AVRCP controller
    pub fn new(device_id: u32) -> Self {
        Self {
            device_id,
            play_status: AvrcpPlayStatus::Stopped,
            position_ms: 0,
            track_length_ms: 0,
            track_title: String::new(),
            track_artist: String::new(),
            track_album: String::new(),
            volume: 100,
        }
    }

    /// Send command
    pub fn send_command(&mut self, command: AvrcpCommand) -> bool {
        match command {
            AvrcpCommand::Play => {
                self.play_status = AvrcpPlayStatus::Playing;
                crate::kprintln!("avrcp: play");
            }
            AvrcpCommand::Pause => {
                self.play_status = AvrcpPlayStatus::Paused;
                crate::kprintln!("avrcp: pause");
            }
            AvrcpCommand::Stop => {
                self.play_status = AvrcpPlayStatus::Stopped;
                self.position_ms = 0;
                crate::kprintln!("avrcp: stop");
            }
            AvrcpCommand::Next => {
                self.position_ms = 0;
                crate::kprintln!("avrcp: next track");
            }
            AvrcpCommand::Previous => {
                self.position_ms = 0;
                crate::kprintln!("avrcp: previous track");
            }
            AvrcpCommand::FastForward => {
                self.play_status = AvrcpPlayStatus::FwdSeek;
                crate::kprintln!("avrcp: fast forward");
            }
            AvrcpCommand::Rewind => {
                self.play_status = AvrcpPlayStatus::RevSeek;
                crate::kprintln!("avrcp: rewind");
            }
            AvrcpCommand::VolumeUp => {
                self.volume = (self.volume + 5).min(100);
                crate::kprintln!("avrcp: volume up to {}", self.volume);
            }
            AvrcpCommand::VolumeDown => {
                self.volume = self.volume.saturating_sub(5);
                crate::kprintln!("avrcp: volume down to {}", self.volume);
            }
            AvrcpCommand::Mute => {
                self.volume = 0;
                crate::kprintln!("avrcp: mute");
            }
        }
        true
    }

    /// Get play status
    pub fn play_status(&self) -> AvrcpPlayStatus {
        self.play_status
    }

    /// Get position in milliseconds
    pub fn position_ms(&self) -> u64 {
        self.position_ms
    }

    /// Get track info
    pub fn track_info(&self) -> (&str, &str, &str) {
        (&self.track_title, &self.track_artist, &self.track_album)
    }

    /// Update track info
    pub fn set_track_info(&mut self, title: &str, artist: &str, album: &str, length_ms: u64) {
        self.track_title = title.to_string();
        self.track_artist = artist.to_string();
        self.track_album = album.to_string();
        self.track_length_ms = length_ms;
        self.position_ms = 0;
    }

    /// Get volume
    pub fn volume(&self) -> u8 {
        self.volume
    }

    /// Set volume (from remote)
    pub fn set_volume(&mut self, volume: u8) {
        self.volume = volume.min(127);
    }
}

/// Bluetooth audio manager
pub struct BluetoothAudioManager {
    devices: BTreeMap<u32, BluetoothAudioDevice>,
    next_device_id: AtomicU32,
    default_a2dp_device: Option<u32>,
    default_hfp_device: Option<u32>,
    avrcp_controllers: BTreeMap<u32, AvrcpController>,
}

impl BluetoothAudioManager {
    /// Create new manager
    pub const fn new() -> Self {
        Self {
            devices: BTreeMap::new(),
            next_device_id: AtomicU32::new(1),
            default_a2dp_device: None,
            default_hfp_device: None,
            avrcp_controllers: BTreeMap::new(),
        }
    }

    /// Register a discovered device
    pub fn register_device(&mut self, address: [u8; 6], name: &str) -> u32 {
        let id = self.next_device_id.fetch_add(1, Ordering::Relaxed);
        let device = BluetoothAudioDevice::new(id, address, name);
        self.devices.insert(id, device);
        crate::kprintln!("bluetooth_audio: registered device {} ({})", name, id);
        id
    }

    /// Unregister a device
    pub fn unregister_device(&mut self, id: u32) {
        if let Some(mut device) = self.devices.remove(&id) {
            device.disconnect();
        }
        self.avrcp_controllers.remove(&id);

        if self.default_a2dp_device == Some(id) {
            self.default_a2dp_device = None;
        }
        if self.default_hfp_device == Some(id) {
            self.default_hfp_device = None;
        }
    }

    /// Get device by ID
    pub fn get_device(&self, id: u32) -> Option<&BluetoothAudioDevice> {
        self.devices.get(&id)
    }

    /// Get device by ID (mutable)
    pub fn get_device_mut(&mut self, id: u32) -> Option<&mut BluetoothAudioDevice> {
        self.devices.get_mut(&id)
    }

    /// List all devices
    pub fn list_devices(&self) -> Vec<&BluetoothAudioDevice> {
        self.devices.values().collect()
    }

    /// List connected devices
    pub fn list_connected_devices(&self) -> Vec<&BluetoothAudioDevice> {
        self.devices.values().filter(|d| d.is_connected()).collect()
    }

    /// Connect device with A2DP
    pub fn connect_a2dp(&mut self, id: u32, codec: BluetoothCodec) -> bool {
        if let Some(device) = self.devices.get_mut(&id) {
            if device.connect_a2dp(codec) {
                if self.default_a2dp_device.is_none() {
                    self.default_a2dp_device = Some(id);
                }

                // Create AVRCP controller
                self.avrcp_controllers.insert(id, AvrcpController::new(id));
                return true;
            }
        }
        false
    }

    /// Connect device with HFP
    pub fn connect_hfp(&mut self, id: u32) -> bool {
        if let Some(device) = self.devices.get_mut(&id) {
            if device.connect_hfp() {
                if self.default_hfp_device.is_none() {
                    self.default_hfp_device = Some(id);
                }
                return true;
            }
        }
        false
    }

    /// Disconnect device
    pub fn disconnect(&mut self, id: u32) {
        if let Some(device) = self.devices.get_mut(&id) {
            device.disconnect();
        }
        self.avrcp_controllers.remove(&id);

        if self.default_a2dp_device == Some(id) {
            // Find another connected A2DP device
            self.default_a2dp_device = self.devices
                .iter()
                .find(|(_, d)| d.is_connected() && d.active_codec().is_some())
                .map(|(id, _)| *id);
        }
    }

    /// Get default A2DP device
    pub fn default_a2dp_device(&self) -> Option<&BluetoothAudioDevice> {
        self.default_a2dp_device.and_then(|id| self.devices.get(&id))
    }

    /// Set default A2DP device
    pub fn set_default_a2dp_device(&mut self, id: u32) -> bool {
        if self.devices.get(&id).map(|d| d.is_connected()).unwrap_or(false) {
            self.default_a2dp_device = Some(id);
            true
        } else {
            false
        }
    }

    /// Get AVRCP controller for device
    pub fn get_avrcp(&mut self, id: u32) -> Option<&mut AvrcpController> {
        self.avrcp_controllers.get_mut(&id)
    }

    /// Select best codec for device
    pub fn select_best_codec(&self, id: u32) -> Option<BluetoothCodec> {
        let device = self.devices.get(&id)?;
        let caps = device.a2dp_capabilities()?;

        // Priority: LDAC > aptX HD > aptX > AAC > SBC
        let priority = [
            BluetoothCodec::Ldac,
            BluetoothCodec::AptXHd,
            BluetoothCodec::AptXAdaptive,
            BluetoothCodec::AptX,
            BluetoothCodec::Aac,
            BluetoothCodec::Sbc,
        ];

        priority.iter().find(|c| caps.codecs.contains(c)).copied()
    }
}

// =============================================================================
// Global instance
// =============================================================================

static BLUETOOTH_AUDIO: IrqSafeMutex<BluetoothAudioManager> =
    IrqSafeMutex::new(BluetoothAudioManager::new());

/// Initialize Bluetooth audio
pub fn init() {
    crate::kprintln!("bluetooth_audio: initialized");
}

/// Register a Bluetooth audio device
pub fn register_device(address: [u8; 6], name: &str) -> u32 {
    BLUETOOTH_AUDIO.lock().register_device(address, name)
}

/// Configure device profiles
pub fn configure_device(id: u32, profiles: &[BluetoothProfile], caps: Option<A2dpCapabilities>) {
    let mut manager = BLUETOOTH_AUDIO.lock();
    if let Some(device) = manager.get_device_mut(id) {
        for profile in profiles {
            device.add_profile(*profile);
        }
        if let Some(c) = caps {
            device.set_a2dp_capabilities(c);
        }
    }
}

/// Connect A2DP
pub fn connect_a2dp(id: u32) -> bool {
    let mut manager = BLUETOOTH_AUDIO.lock();
    let codec = manager.select_best_codec(id).unwrap_or(BluetoothCodec::Sbc);
    manager.connect_a2dp(id, codec)
}

/// Connect HFP
pub fn connect_hfp(id: u32) -> bool {
    BLUETOOTH_AUDIO.lock().connect_hfp(id)
}

/// Disconnect device
pub fn disconnect(id: u32) {
    BLUETOOTH_AUDIO.lock().disconnect(id);
}

/// List devices
pub fn list_devices() -> Vec<(u32, String, bool)> {
    BLUETOOTH_AUDIO.lock()
        .list_devices()
        .iter()
        .map(|d| (d.id(), d.name().to_string(), d.is_connected()))
        .collect()
}

/// Set device volume
pub fn set_volume(id: u32, volume: u8) {
    if let Some(device) = BLUETOOTH_AUDIO.lock().get_device(id) {
        device.set_volume(volume);
    }
}

/// AVRCP play
pub fn avrcp_play(id: u32) -> bool {
    BLUETOOTH_AUDIO.lock()
        .get_avrcp(id)
        .map(|c| c.send_command(AvrcpCommand::Play))
        .unwrap_or(false)
}

/// AVRCP pause
pub fn avrcp_pause(id: u32) -> bool {
    BLUETOOTH_AUDIO.lock()
        .get_avrcp(id)
        .map(|c| c.send_command(AvrcpCommand::Pause))
        .unwrap_or(false)
}

/// AVRCP next
pub fn avrcp_next(id: u32) -> bool {
    BLUETOOTH_AUDIO.lock()
        .get_avrcp(id)
        .map(|c| c.send_command(AvrcpCommand::Next))
        .unwrap_or(false)
}

/// AVRCP previous
pub fn avrcp_previous(id: u32) -> bool {
    BLUETOOTH_AUDIO.lock()
        .get_avrcp(id)
        .map(|c| c.send_command(AvrcpCommand::Previous))
        .unwrap_or(false)
}
