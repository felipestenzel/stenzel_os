//! HDMI/DisplayPort Audio
//!
//! Provides audio output over HDMI and DisplayPort connections.
//! Integrates with GPU drivers (Intel, AMD) for audio routing.

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

/// Display connection type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayConnectionType {
    /// HDMI connection
    Hdmi,
    /// DisplayPort connection
    DisplayPort,
    /// Mini DisplayPort
    MiniDisplayPort,
    /// USB-C with DisplayPort Alt Mode
    UsbCDisplayPort,
    /// Embedded DisplayPort (laptop internal)
    EmbeddedDisplayPort,
}

impl DisplayConnectionType {
    pub fn name(&self) -> &'static str {
        match self {
            DisplayConnectionType::Hdmi => "HDMI",
            DisplayConnectionType::DisplayPort => "DisplayPort",
            DisplayConnectionType::MiniDisplayPort => "Mini DisplayPort",
            DisplayConnectionType::UsbCDisplayPort => "USB-C",
            DisplayConnectionType::EmbeddedDisplayPort => "eDP",
        }
    }

    /// Whether this connection type typically has audio
    pub fn supports_audio(&self) -> bool {
        // eDP typically doesn't carry audio
        !matches!(self, DisplayConnectionType::EmbeddedDisplayPort)
    }
}

/// HDMI audio format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HdmiAudioFormat {
    /// Linear PCM (mandatory)
    Lpcm,
    /// AC-3 (Dolby Digital)
    Ac3,
    /// MPEG-1/2 Audio
    Mpeg,
    /// MP3
    Mp3,
    /// MPEG-2 AAC
    Mpeg2Aac,
    /// DTS
    Dts,
    /// ATRAC
    Atrac,
    /// One Bit Audio (SACD)
    OneBitAudio,
    /// Dolby Digital Plus (E-AC-3)
    DolbyDigitalPlus,
    /// DTS-HD
    DtsHd,
    /// Dolby TrueHD / MLP
    TrueHd,
    /// DST (DSD)
    Dst,
    /// Microsoft WMA Pro
    WmaPro,
}

impl HdmiAudioFormat {
    /// Is this a compressed format?
    pub fn is_compressed(&self) -> bool {
        !matches!(self, HdmiAudioFormat::Lpcm)
    }

    /// Get format code (CEA-861)
    pub fn format_code(&self) -> u8 {
        match self {
            HdmiAudioFormat::Lpcm => 1,
            HdmiAudioFormat::Ac3 => 2,
            HdmiAudioFormat::Mpeg => 3,
            HdmiAudioFormat::Mp3 => 4,
            HdmiAudioFormat::Mpeg2Aac => 5,
            HdmiAudioFormat::Dts => 6,
            HdmiAudioFormat::Atrac => 7,
            HdmiAudioFormat::OneBitAudio => 8,
            HdmiAudioFormat::DolbyDigitalPlus => 9,
            HdmiAudioFormat::DtsHd => 10,
            HdmiAudioFormat::TrueHd => 11,
            HdmiAudioFormat::Dst => 12,
            HdmiAudioFormat::WmaPro => 13,
        }
    }
}

/// Short Audio Descriptor (SAD) from EDID
#[derive(Debug, Clone)]
pub struct ShortAudioDescriptor {
    /// Audio format
    pub format: HdmiAudioFormat,
    /// Maximum channel count (1-8)
    pub max_channels: u8,
    /// Supported sample rates (bitmap: 32, 44.1, 48, 88.2, 96, 176.4, 192 kHz)
    pub sample_rates: u8,
    /// For LPCM: supported bit depths (bitmap: 16, 20, 24 bits)
    /// For compressed: max bitrate
    pub format_specific: u8,
}

impl ShortAudioDescriptor {
    /// Get supported sample rates as a vector
    pub fn sample_rate_list(&self) -> Vec<u32> {
        let rates = [32000, 44100, 48000, 88200, 96000, 176400, 192000];
        rates.iter()
            .enumerate()
            .filter(|(i, _)| (self.sample_rates >> i) & 1 != 0)
            .map(|(_, &r)| r)
            .collect()
    }

    /// Get supported bit depths for LPCM
    pub fn bit_depths(&self) -> Vec<u8> {
        if self.format != HdmiAudioFormat::Lpcm {
            return Vec::new();
        }
        let depths = [16, 20, 24];
        depths.iter()
            .enumerate()
            .filter(|(i, _)| (self.format_specific >> i) & 1 != 0)
            .map(|(_, &d)| d)
            .collect()
    }
}

/// HDMI audio capabilities from EDID
#[derive(Debug, Clone)]
pub struct HdmiAudioCapabilities {
    /// Basic audio support (2ch LPCM)
    pub basic_audio: bool,
    /// Short audio descriptors
    pub audio_descriptors: Vec<ShortAudioDescriptor>,
    /// Speaker allocation (CEA-861)
    pub speaker_allocation: u8,
}

impl Default for HdmiAudioCapabilities {
    fn default() -> Self {
        Self {
            basic_audio: true,
            audio_descriptors: vec![
                ShortAudioDescriptor {
                    format: HdmiAudioFormat::Lpcm,
                    max_channels: 2,
                    sample_rates: 0b0000111, // 32, 44.1, 48 kHz
                    format_specific: 0b001,  // 16-bit
                },
            ],
            speaker_allocation: 0, // Front L+R only
        }
    }
}

impl HdmiAudioCapabilities {
    /// Get maximum channels supported
    pub fn max_channels(&self) -> u8 {
        self.audio_descriptors
            .iter()
            .map(|sad| sad.max_channels)
            .max()
            .unwrap_or(2)
    }

    /// Get max LPCM sample rate
    pub fn max_lpcm_sample_rate(&self) -> u32 {
        self.audio_descriptors
            .iter()
            .filter(|sad| sad.format == HdmiAudioFormat::Lpcm)
            .flat_map(|sad| sad.sample_rate_list())
            .max()
            .unwrap_or(48000)
    }

    /// Get max LPCM bit depth
    pub fn max_lpcm_bit_depth(&self) -> u8 {
        self.audio_descriptors
            .iter()
            .filter(|sad| sad.format == HdmiAudioFormat::Lpcm)
            .flat_map(|sad| sad.bit_depths())
            .max()
            .unwrap_or(16)
    }

    /// Check if compressed format is supported
    pub fn supports_format(&self, format: HdmiAudioFormat) -> bool {
        self.audio_descriptors.iter().any(|sad| sad.format == format)
    }

    /// Decode speaker allocation
    pub fn speaker_layout(&self) -> SpeakerLayout {
        SpeakerLayout::from_allocation(self.speaker_allocation)
    }
}

/// Speaker layout
#[derive(Debug, Clone, Copy)]
pub struct SpeakerLayout {
    pub front_left_right: bool,
    pub lfe: bool,
    pub front_center: bool,
    pub rear_left_right: bool,
    pub rear_center: bool,
    pub front_left_right_center: bool,
    pub rear_left_right_center: bool,
    pub front_left_right_wide: bool,
    pub front_left_right_high: bool,
    pub top_center: bool,
    pub front_center_high: bool,
}

impl SpeakerLayout {
    pub fn from_allocation(allocation: u8) -> Self {
        Self {
            front_left_right: (allocation & 0x01) != 0,
            lfe: (allocation & 0x02) != 0,
            front_center: (allocation & 0x04) != 0,
            rear_left_right: (allocation & 0x08) != 0,
            rear_center: (allocation & 0x10) != 0,
            front_left_right_center: (allocation & 0x20) != 0,
            rear_left_right_center: (allocation & 0x40) != 0,
            front_left_right_wide: false, // Extended
            front_left_right_high: false,
            top_center: false,
            front_center_high: false,
        }
    }

    pub fn channel_count(&self) -> u8 {
        let mut count = 0u8;
        if self.front_left_right { count += 2; }
        if self.lfe { count += 1; }
        if self.front_center { count += 1; }
        if self.rear_left_right { count += 2; }
        if self.rear_center { count += 1; }
        if self.front_left_right_center { count += 2; }
        if self.rear_left_right_center { count += 2; }
        count.max(2) // Minimum stereo
    }

    /// Get surround format name
    pub fn format_name(&self) -> &'static str {
        match self.channel_count() {
            1 | 2 => "Stereo",
            3 => "2.1",
            4 => "Quadraphonic",
            5 => "4.1",
            6 => "5.1",
            7 => "6.1",
            8 => "7.1",
            _ => "Multi-channel",
        }
    }
}

/// HDMI audio output device
pub struct HdmiAudioDevice {
    /// Device ID
    id: u32,
    /// Associated display connector ID
    connector_id: u32,
    /// Connection type
    connection_type: DisplayConnectionType,
    /// Device name (usually monitor name)
    name: String,
    /// Audio capabilities
    capabilities: HdmiAudioCapabilities,
    /// Current sample rate
    sample_rate: u32,
    /// Current bit depth
    bit_depth: u8,
    /// Current channel count
    channels: u8,
    /// Volume (0-100)
    volume: AtomicU32,
    /// Muted
    muted: AtomicBool,
    /// Connected
    connected: AtomicBool,
    /// Audio daemon node ID
    daemon_node_id: Option<NodeId>,
    /// GPU driver type
    gpu_driver: GpuDriver,
}

/// GPU driver type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuDriver {
    Intel,
    Amd,
    Nvidia,
    Unknown,
}

impl HdmiAudioDevice {
    /// Create new HDMI audio device
    pub fn new(
        id: u32,
        connector_id: u32,
        connection_type: DisplayConnectionType,
        name: &str,
        gpu_driver: GpuDriver,
    ) -> Self {
        Self {
            id,
            connector_id,
            connection_type,
            name: name.to_string(),
            capabilities: HdmiAudioCapabilities::default(),
            sample_rate: 48000,
            bit_depth: 16,
            channels: 2,
            volume: AtomicU32::new(100),
            muted: AtomicBool::new(false),
            connected: AtomicBool::new(false),
            daemon_node_id: None,
            gpu_driver,
        }
    }

    /// Get device ID
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Get display connector ID
    pub fn connector_id(&self) -> u32 {
        self.connector_id
    }

    /// Get device name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get connection type
    pub fn connection_type(&self) -> DisplayConnectionType {
        self.connection_type
    }

    /// Set audio capabilities (from EDID)
    pub fn set_capabilities(&mut self, caps: HdmiAudioCapabilities) {
        self.capabilities = caps;
    }

    /// Get audio capabilities
    pub fn capabilities(&self) -> &HdmiAudioCapabilities {
        &self.capabilities
    }

    /// Connect device
    pub fn connect(&mut self) -> bool {
        if !self.connection_type.supports_audio() {
            return false;
        }

        // Verify basic audio is supported
        if !self.capabilities.basic_audio {
            return false;
        }

        // Register with audio daemon
        if let Some(client_id) = daemon::connect_client(&self.name, ClientProtocol::Native) {
            let device_name = alloc::format!("{} ({})", self.name, self.connection_type.name());
            let node_id = daemon::with_daemon_mut(|d| {
                let id = d.create_node(&device_name, NodeType::Sink);
                if let Some(node) = d.get_node_mut(id) {
                    node.set_property("device.class", "hdmi");
                    node.set_property("device.connection", self.connection_type.name());
                    node.set_property("audio.channels", &alloc::format!("{}", self.capabilities.max_channels()));
                }
                id
            });
            self.daemon_node_id = node_id;
        }

        self.connected.store(true, Ordering::Relaxed);
        crate::kprintln!(
            "hdmi_audio: connected {} ({}) - {} up to {}kHz/{}-bit",
            self.name,
            self.connection_type.name(),
            self.capabilities.speaker_layout().format_name(),
            self.capabilities.max_lpcm_sample_rate() / 1000,
            self.capabilities.max_lpcm_bit_depth()
        );
        true
    }

    /// Disconnect device
    pub fn disconnect(&mut self) {
        if let Some(node_id) = self.daemon_node_id.take() {
            daemon::with_daemon_mut(|d| d.remove_node(node_id));
        }

        self.connected.store(false, Ordering::Relaxed);
        crate::kprintln!("hdmi_audio: disconnected {}", self.name);
    }

    /// Is connected
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    /// Set sample rate
    pub fn set_sample_rate(&mut self, rate: u32) -> bool {
        let supported = self.capabilities.audio_descriptors
            .iter()
            .filter(|sad| sad.format == HdmiAudioFormat::Lpcm)
            .flat_map(|sad| sad.sample_rate_list())
            .any(|r| r == rate);

        if supported {
            self.sample_rate = rate;
            true
        } else {
            false
        }
    }

    /// Get current sample rate
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Set bit depth
    pub fn set_bit_depth(&mut self, depth: u8) -> bool {
        let supported = self.capabilities.audio_descriptors
            .iter()
            .filter(|sad| sad.format == HdmiAudioFormat::Lpcm)
            .flat_map(|sad| sad.bit_depths())
            .any(|d| d == depth);

        if supported {
            self.bit_depth = depth;
            true
        } else {
            false
        }
    }

    /// Set channel count
    pub fn set_channels(&mut self, channels: u8) -> bool {
        if channels <= self.capabilities.max_channels() {
            self.channels = channels;
            true
        } else {
            false
        }
    }

    /// Set volume
    pub fn set_volume(&self, volume: u8) {
        self.volume.store(volume.min(100) as u32, Ordering::Relaxed);
        if let Some(node_id) = self.daemon_node_id {
            daemon::set_stream_volume(node_id, volume);
        }
    }

    /// Get volume
    pub fn volume(&self) -> u8 {
        self.volume.load(Ordering::Relaxed) as u8
    }

    /// Set mute
    pub fn set_mute(&self, muted: bool) {
        self.muted.store(muted, Ordering::Relaxed);
        if let Some(node_id) = self.daemon_node_id {
            daemon::set_stream_mute(node_id, muted);
        }
    }

    /// Is muted
    pub fn is_muted(&self) -> bool {
        self.muted.load(Ordering::Relaxed)
    }
}

/// HDMI audio device info
#[derive(Debug, Clone)]
pub struct HdmiAudioDeviceInfo {
    pub id: u32,
    pub connector_id: u32,
    pub name: String,
    pub connection_type: DisplayConnectionType,
    pub max_channels: u8,
    pub max_sample_rate: u32,
    pub max_bit_depth: u8,
    pub supports_surround: bool,
    pub is_connected: bool,
}

/// HDMI audio manager
pub struct HdmiAudioManager {
    devices: BTreeMap<u32, HdmiAudioDevice>,
    next_device_id: AtomicU32,
    default_device: Option<u32>,
}

impl HdmiAudioManager {
    /// Create new manager
    pub const fn new() -> Self {
        Self {
            devices: BTreeMap::new(),
            next_device_id: AtomicU32::new(1),
            default_device: None,
        }
    }

    /// Register HDMI audio device
    pub fn register_device(
        &mut self,
        connector_id: u32,
        connection_type: DisplayConnectionType,
        name: &str,
        gpu_driver: GpuDriver,
    ) -> u32 {
        let id = self.next_device_id.fetch_add(1, Ordering::Relaxed);
        let device = HdmiAudioDevice::new(id, connector_id, connection_type, name, gpu_driver);
        self.devices.insert(id, device);
        crate::kprintln!(
            "hdmi_audio: registered {} ({}) on connector {}",
            name,
            connection_type.name(),
            connector_id
        );
        id
    }

    /// Unregister device
    pub fn unregister_device(&mut self, id: u32) {
        if let Some(mut device) = self.devices.remove(&id) {
            device.disconnect();
        }

        if self.default_device == Some(id) {
            self.default_device = None;
        }
    }

    /// Get device by ID
    pub fn get_device(&self, id: u32) -> Option<&HdmiAudioDevice> {
        self.devices.get(&id)
    }

    /// Get device by ID (mutable)
    pub fn get_device_mut(&mut self, id: u32) -> Option<&mut HdmiAudioDevice> {
        self.devices.get_mut(&id)
    }

    /// Find device by connector ID
    pub fn find_by_connector(&self, connector_id: u32) -> Option<&HdmiAudioDevice> {
        self.devices.values().find(|d| d.connector_id() == connector_id)
    }

    /// Connect device
    pub fn connect_device(&mut self, id: u32) -> bool {
        if let Some(device) = self.devices.get_mut(&id) {
            if device.connect() {
                if self.default_device.is_none() {
                    self.default_device = Some(id);
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

        if self.default_device == Some(id) {
            self.default_device = self.devices.iter()
                .find(|(_, d)| d.is_connected())
                .map(|(id, _)| *id);
        }
    }

    /// Handle display hotplug
    pub fn handle_hotplug(&mut self, connector_id: u32, connected: bool) {
        // Find device by connector
        let device_id = self.devices.iter()
            .find(|(_, d)| d.connector_id() == connector_id)
            .map(|(id, _)| *id);

        if let Some(id) = device_id {
            if connected {
                self.connect_device(id);
            } else {
                self.disconnect_device(id);
            }
        }
    }

    /// List all devices
    pub fn list_devices(&self) -> Vec<HdmiAudioDeviceInfo> {
        self.devices.values().map(|d| {
            HdmiAudioDeviceInfo {
                id: d.id(),
                connector_id: d.connector_id(),
                name: d.name().to_string(),
                connection_type: d.connection_type(),
                max_channels: d.capabilities().max_channels(),
                max_sample_rate: d.capabilities().max_lpcm_sample_rate(),
                max_bit_depth: d.capabilities().max_lpcm_bit_depth(),
                supports_surround: d.capabilities().max_channels() > 2,
                is_connected: d.is_connected(),
            }
        }).collect()
    }

    /// Get default device
    pub fn default_device(&self) -> Option<&HdmiAudioDevice> {
        self.default_device.and_then(|id| self.devices.get(&id))
    }

    /// Set default device
    pub fn set_default_device(&mut self, id: u32) -> bool {
        if self.devices.get(&id).map(|d| d.is_connected()).unwrap_or(false) {
            self.default_device = Some(id);
            true
        } else {
            false
        }
    }
}

// =============================================================================
// Global instance
// =============================================================================

static HDMI_AUDIO: IrqSafeMutex<HdmiAudioManager> = IrqSafeMutex::new(HdmiAudioManager::new());

/// Initialize HDMI audio
pub fn init() {
    crate::kprintln!("hdmi_audio: initialized");
}

/// Register HDMI audio device
pub fn register_device(
    connector_id: u32,
    connection_type: DisplayConnectionType,
    name: &str,
    gpu_driver: GpuDriver,
) -> u32 {
    HDMI_AUDIO.lock().register_device(connector_id, connection_type, name, gpu_driver)
}

/// Set device capabilities from EDID
pub fn set_capabilities(id: u32, caps: HdmiAudioCapabilities) {
    if let Some(device) = HDMI_AUDIO.lock().get_device_mut(id) {
        device.set_capabilities(caps);
    }
}

/// Connect device
pub fn connect_device(id: u32) -> bool {
    HDMI_AUDIO.lock().connect_device(id)
}

/// Disconnect device
pub fn disconnect_device(id: u32) {
    HDMI_AUDIO.lock().disconnect_device(id);
}

/// Handle display hotplug
pub fn handle_hotplug(connector_id: u32, connected: bool) {
    HDMI_AUDIO.lock().handle_hotplug(connector_id, connected);
}

/// List devices
pub fn list_devices() -> Vec<HdmiAudioDeviceInfo> {
    HDMI_AUDIO.lock().list_devices()
}

/// Set volume
pub fn set_volume(id: u32, volume: u8) {
    if let Some(device) = HDMI_AUDIO.lock().get_device(id) {
        device.set_volume(volume);
    }
}

/// Set mute
pub fn set_mute(id: u32, muted: bool) {
    if let Some(device) = HDMI_AUDIO.lock().get_device(id) {
        device.set_mute(muted);
    }
}
