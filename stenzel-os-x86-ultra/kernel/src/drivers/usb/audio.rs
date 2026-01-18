//! USB Audio Class (UAC) driver.
//!
//! Implements USB Audio Class 1.0 and 2.0 specification for audio devices.
//!
//! Features:
//! - Audio Control (AC) interface parsing
//! - Audio Streaming (AS) interface configuration
//! - Isochronous endpoint management
//! - Sample rate negotiation
//! - Volume/mute control

#![allow(dead_code)]

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::sync::TicketSpinlock;

/// USB Audio Class codes
pub mod class_codes {
    pub const AUDIO: u8 = 0x01;

    // Audio Interface Subclass Codes
    pub const AUDIOCONTROL: u8 = 0x01;
    pub const AUDIOSTREAMING: u8 = 0x02;
    pub const MIDISTREAMING: u8 = 0x03;

    // Audio Class-Specific Descriptor Types
    pub const CS_UNDEFINED: u8 = 0x20;
    pub const CS_DEVICE: u8 = 0x21;
    pub const CS_CONFIGURATION: u8 = 0x22;
    pub const CS_STRING: u8 = 0x23;
    pub const CS_INTERFACE: u8 = 0x24;
    pub const CS_ENDPOINT: u8 = 0x25;

    // Audio Control Interface Descriptor Subtypes
    pub const AC_HEADER: u8 = 0x01;
    pub const AC_INPUT_TERMINAL: u8 = 0x02;
    pub const AC_OUTPUT_TERMINAL: u8 = 0x03;
    pub const AC_MIXER_UNIT: u8 = 0x04;
    pub const AC_SELECTOR_UNIT: u8 = 0x05;
    pub const AC_FEATURE_UNIT: u8 = 0x06;
    pub const AC_PROCESSING_UNIT: u8 = 0x07;
    pub const AC_EXTENSION_UNIT: u8 = 0x08;
    // UAC2 additions
    pub const AC_CLOCK_SOURCE: u8 = 0x0A;
    pub const AC_CLOCK_SELECTOR: u8 = 0x0B;
    pub const AC_CLOCK_MULTIPLIER: u8 = 0x0C;
    pub const AC_SAMPLE_RATE_CONVERTER: u8 = 0x0D;

    // Audio Streaming Interface Descriptor Subtypes
    pub const AS_GENERAL: u8 = 0x01;
    pub const AS_FORMAT_TYPE: u8 = 0x02;
    pub const AS_FORMAT_SPECIFIC: u8 = 0x03;

    // Audio Endpoint Descriptor Subtypes
    pub const EP_GENERAL: u8 = 0x01;
}

/// Terminal types
pub mod terminal_types {
    // USB Terminal Types
    pub const USB_UNDEFINED: u16 = 0x0100;
    pub const USB_STREAMING: u16 = 0x0101;
    pub const USB_VENDOR_SPECIFIC: u16 = 0x01FF;

    // Input Terminal Types
    pub const INPUT_UNDEFINED: u16 = 0x0200;
    pub const MICROPHONE: u16 = 0x0201;
    pub const DESKTOP_MICROPHONE: u16 = 0x0202;
    pub const PERSONAL_MICROPHONE: u16 = 0x0203;
    pub const OMNI_MICROPHONE: u16 = 0x0204;
    pub const MICROPHONE_ARRAY: u16 = 0x0205;
    pub const PROC_MICROPHONE_ARRAY: u16 = 0x0206;

    // Output Terminal Types
    pub const OUTPUT_UNDEFINED: u16 = 0x0300;
    pub const SPEAKER: u16 = 0x0301;
    pub const HEADPHONES: u16 = 0x0302;
    pub const HMD_AUDIO: u16 = 0x0303;
    pub const DESKTOP_SPEAKER: u16 = 0x0304;
    pub const ROOM_SPEAKER: u16 = 0x0305;
    pub const COMM_SPEAKER: u16 = 0x0306;
    pub const LFE_SPEAKER: u16 = 0x0307;

    // Bi-directional Terminal Types
    pub const BIDIRECTIONAL_UNDEFINED: u16 = 0x0400;
    pub const HANDSET: u16 = 0x0401;
    pub const HEADSET: u16 = 0x0402;
    pub const SPEAKERPHONE: u16 = 0x0403;
    pub const ECHO_SUPPRESSING_SPEAKERPHONE: u16 = 0x0404;
    pub const ECHO_CANCELING_SPEAKERPHONE: u16 = 0x0405;

    // External Terminal Types
    pub const EXTERNAL_UNDEFINED: u16 = 0x0600;
    pub const ANALOG_CONNECTOR: u16 = 0x0601;
    pub const DIGITAL_AUDIO_INTERFACE: u16 = 0x0602;
    pub const LINE_CONNECTOR: u16 = 0x0603;
    pub const LEGACY_AUDIO_CONNECTOR: u16 = 0x0604;
    pub const SPDIF_INTERFACE: u16 = 0x0605;
    pub const HDMI: u16 = 0x0607;
}

/// Audio format types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    Pcm,
    Pcm8,
    IeeeFloat,
    Alaw,
    Mulaw,
    Unknown(u16),
}

impl AudioFormat {
    pub fn from_format_tag(tag: u16) -> Self {
        match tag {
            0x0001 => AudioFormat::Pcm,
            0x0002 => AudioFormat::Pcm8,
            0x0003 => AudioFormat::IeeeFloat,
            0x0004 => AudioFormat::Alaw,
            0x0005 => AudioFormat::Mulaw,
            other => AudioFormat::Unknown(other),
        }
    }

    pub fn bits_per_sample(&self, sub_frame_size: u8) -> u8 {
        match self {
            AudioFormat::Pcm8 => 8,
            AudioFormat::Alaw | AudioFormat::Mulaw => 8,
            AudioFormat::Pcm | AudioFormat::IeeeFloat => sub_frame_size * 8,
            AudioFormat::Unknown(_) => sub_frame_size * 8,
        }
    }
}

/// Audio sample rate
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SampleRate(pub u32);

impl SampleRate {
    pub const RATE_8000: Self = SampleRate(8000);
    pub const RATE_11025: Self = SampleRate(11025);
    pub const RATE_16000: Self = SampleRate(16000);
    pub const RATE_22050: Self = SampleRate(22050);
    pub const RATE_32000: Self = SampleRate(32000);
    pub const RATE_44100: Self = SampleRate(44100);
    pub const RATE_48000: Self = SampleRate(48000);
    pub const RATE_88200: Self = SampleRate(88200);
    pub const RATE_96000: Self = SampleRate(96000);
    pub const RATE_176400: Self = SampleRate(176400);
    pub const RATE_192000: Self = SampleRate(192000);
}

/// Channel configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelConfig(pub u16);

impl ChannelConfig {
    pub const MONO: Self = ChannelConfig(0x0001);
    pub const STEREO: Self = ChannelConfig(0x0003);
    pub const SURROUND_51: Self = ChannelConfig(0x003F);
    pub const SURROUND_71: Self = ChannelConfig(0x00FF);

    pub fn channel_count(&self) -> u8 {
        self.0.count_ones() as u8
    }

    pub fn has_left(&self) -> bool { self.0 & 0x0001 != 0 }
    pub fn has_right(&self) -> bool { self.0 & 0x0002 != 0 }
    pub fn has_center(&self) -> bool { self.0 & 0x0004 != 0 }
    pub fn has_lfe(&self) -> bool { self.0 & 0x0008 != 0 }
    pub fn has_left_surround(&self) -> bool { self.0 & 0x0010 != 0 }
    pub fn has_right_surround(&self) -> bool { self.0 & 0x0020 != 0 }
    pub fn has_left_back(&self) -> bool { self.0 & 0x0040 != 0 }
    pub fn has_right_back(&self) -> bool { self.0 & 0x0080 != 0 }
}

/// Terminal descriptor
#[derive(Debug, Clone)]
pub struct Terminal {
    pub id: u8,
    pub terminal_type: u16,
    pub assoc_terminal: u8,
    pub channels: u8,
    pub channel_config: ChannelConfig,
    pub string_index: u8,
    pub is_input: bool,
}

impl Terminal {
    pub fn is_microphone(&self) -> bool {
        self.terminal_type >= terminal_types::INPUT_UNDEFINED &&
        self.terminal_type <= terminal_types::PROC_MICROPHONE_ARRAY
    }

    pub fn is_speaker(&self) -> bool {
        self.terminal_type >= terminal_types::OUTPUT_UNDEFINED &&
        self.terminal_type <= terminal_types::LFE_SPEAKER
    }

    pub fn is_usb_streaming(&self) -> bool {
        self.terminal_type == terminal_types::USB_STREAMING
    }
}

/// Feature unit descriptor - controls volume, mute, etc.
#[derive(Debug, Clone)]
pub struct FeatureUnit {
    pub id: u8,
    pub source_id: u8,
    pub controls: Vec<FeatureControls>,
    pub string_index: u8,
}

/// Feature controls bitmap
#[derive(Debug, Clone, Copy)]
pub struct FeatureControls(pub u16);

impl FeatureControls {
    pub fn has_mute(&self) -> bool { self.0 & 0x0001 != 0 }
    pub fn has_volume(&self) -> bool { self.0 & 0x0002 != 0 }
    pub fn has_bass(&self) -> bool { self.0 & 0x0004 != 0 }
    pub fn has_mid(&self) -> bool { self.0 & 0x0008 != 0 }
    pub fn has_treble(&self) -> bool { self.0 & 0x0010 != 0 }
    pub fn has_graphic_equalizer(&self) -> bool { self.0 & 0x0020 != 0 }
    pub fn has_automatic_gain(&self) -> bool { self.0 & 0x0040 != 0 }
    pub fn has_delay(&self) -> bool { self.0 & 0x0080 != 0 }
    pub fn has_bass_boost(&self) -> bool { self.0 & 0x0100 != 0 }
    pub fn has_loudness(&self) -> bool { self.0 & 0x0200 != 0 }
}

/// Audio streaming interface format
#[derive(Debug, Clone)]
pub struct StreamingFormat {
    pub format: AudioFormat,
    pub channels: u8,
    pub sub_frame_size: u8,
    pub bit_resolution: u8,
    pub sample_rates: Vec<SampleRate>,
    pub continuous_rates: Option<(SampleRate, SampleRate)>,
}

impl StreamingFormat {
    pub fn supports_rate(&self, rate: SampleRate) -> bool {
        if let Some((min, max)) = self.continuous_rates {
            return rate.0 >= min.0 && rate.0 <= max.0;
        }
        self.sample_rates.contains(&rate)
    }

    pub fn bytes_per_sample(&self) -> u8 {
        self.sub_frame_size * self.channels
    }

    pub fn bytes_per_second(&self, rate: SampleRate) -> u32 {
        rate.0 * self.bytes_per_sample() as u32
    }
}

/// Audio streaming endpoint
#[derive(Debug, Clone)]
pub struct StreamingEndpoint {
    pub address: u8,
    pub max_packet_size: u16,
    pub interval: u8,
    pub direction_in: bool,
    pub attributes: u8,
    pub lock_delay_units: u8,
    pub lock_delay: u16,
}

impl StreamingEndpoint {
    pub fn is_async(&self) -> bool {
        (self.attributes & 0x0C) == 0x04
    }

    pub fn is_adaptive(&self) -> bool {
        (self.attributes & 0x0C) == 0x08
    }

    pub fn is_sync(&self) -> bool {
        (self.attributes & 0x0C) == 0x0C
    }
}

/// Audio streaming interface
#[derive(Debug, Clone)]
pub struct StreamingInterface {
    pub interface_number: u8,
    pub alt_setting: u8,
    pub terminal_link: u8,
    pub format: StreamingFormat,
    pub endpoint: StreamingEndpoint,
}

/// Clock source (UAC2)
#[derive(Debug, Clone)]
pub struct ClockSource {
    pub id: u8,
    pub attributes: u8,
    pub assoc_terminal: u8,
    pub string_index: u8,
}

impl ClockSource {
    pub fn is_internal(&self) -> bool {
        (self.attributes & 0x03) == 0x01
    }

    pub fn is_external(&self) -> bool {
        (self.attributes & 0x03) == 0x00
    }

    pub fn is_synced_to_sof(&self) -> bool {
        (self.attributes & 0x03) == 0x03
    }
}

/// USB Audio device
#[derive(Debug)]
pub struct UsbAudioDevice {
    pub slot_id: u8,
    pub address: u8,
    pub vendor_id: u16,
    pub product_id: u16,
    pub name: String,
    pub uac_version: UacVersion,
    pub input_terminals: Vec<Terminal>,
    pub output_terminals: Vec<Terminal>,
    pub feature_units: Vec<FeatureUnit>,
    pub clock_sources: Vec<ClockSource>,
    pub streaming_interfaces: Vec<StreamingInterface>,
    pub current_playback_interface: Option<usize>,
    pub current_capture_interface: Option<usize>,
    pub active: AtomicBool,
    pub playback_active: AtomicBool,
    pub capture_active: AtomicBool,
    pub master_volume: AtomicU32,
    pub muted: AtomicBool,
}

/// UAC version
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UacVersion {
    Uac1,
    Uac2,
    Uac3,
    Unknown(u16),
}

impl UacVersion {
    pub fn from_bcd(bcd: u16) -> Self {
        match bcd >> 8 {
            1 => UacVersion::Uac1,
            2 => UacVersion::Uac2,
            3 => UacVersion::Uac3,
            _ => UacVersion::Unknown(bcd),
        }
    }
}

impl UsbAudioDevice {
    pub fn new(slot_id: u8, address: u8, vendor_id: u16, product_id: u16) -> Self {
        Self {
            slot_id,
            address,
            vendor_id,
            product_id,
            name: String::new(),
            uac_version: UacVersion::Uac1,
            input_terminals: Vec::new(),
            output_terminals: Vec::new(),
            feature_units: Vec::new(),
            clock_sources: Vec::new(),
            streaming_interfaces: Vec::new(),
            current_playback_interface: None,
            current_capture_interface: None,
            active: AtomicBool::new(false),
            playback_active: AtomicBool::new(false),
            capture_active: AtomicBool::new(false),
            master_volume: AtomicU32::new(100),
            muted: AtomicBool::new(false),
        }
    }

    pub fn has_playback(&self) -> bool {
        self.streaming_interfaces.iter().any(|si| !si.endpoint.direction_in)
    }

    pub fn has_capture(&self) -> bool {
        self.streaming_interfaces.iter().any(|si| si.endpoint.direction_in)
    }

    pub fn find_playback_interface(&self, rate: SampleRate, channels: u8) -> Option<&StreamingInterface> {
        self.streaming_interfaces.iter().find(|si| {
            !si.endpoint.direction_in &&
            si.format.channels == channels &&
            si.format.supports_rate(rate)
        })
    }

    pub fn find_capture_interface(&self, rate: SampleRate, channels: u8) -> Option<&StreamingInterface> {
        self.streaming_interfaces.iter().find(|si| {
            si.endpoint.direction_in &&
            si.format.channels == channels &&
            si.format.supports_rate(rate)
        })
    }

    pub fn supported_playback_rates(&self) -> Vec<SampleRate> {
        let mut rates = Vec::new();
        for si in &self.streaming_interfaces {
            if !si.endpoint.direction_in {
                for rate in &si.format.sample_rates {
                    if !rates.contains(rate) {
                        rates.push(*rate);
                    }
                }
            }
        }
        rates.sort_by_key(|r| r.0);
        rates
    }

    pub fn supported_capture_rates(&self) -> Vec<SampleRate> {
        let mut rates = Vec::new();
        for si in &self.streaming_interfaces {
            if si.endpoint.direction_in {
                for rate in &si.format.sample_rates {
                    if !rates.contains(rate) {
                        rates.push(*rate);
                    }
                }
            }
        }
        rates.sort_by_key(|r| r.0);
        rates
    }

    pub fn set_volume(&self, volume: u8) {
        self.master_volume.store(volume as u32, Ordering::SeqCst);
    }

    pub fn get_volume(&self) -> u8 {
        (self.master_volume.load(Ordering::SeqCst) & 0xFF) as u8
    }

    pub fn set_mute(&self, muted: bool) {
        self.muted.store(muted, Ordering::SeqCst);
    }

    pub fn is_muted(&self) -> bool {
        self.muted.load(Ordering::SeqCst)
    }
}

/// Audio stream state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamState {
    Stopped,
    Starting,
    Running,
    Paused,
    Draining,
    Error,
}

/// Audio buffer descriptor
#[derive(Debug)]
pub struct AudioBuffer {
    pub data: Box<[u8]>,
    pub sample_count: usize,
    pub channels: u8,
    pub sample_rate: SampleRate,
    pub format: AudioFormat,
    pub timestamp: u64,
}

/// USB Audio driver state
pub struct UsbAudioDriver {
    devices: Vec<UsbAudioDevice>,
    default_playback_device: Option<usize>,
    default_capture_device: Option<usize>,
    initialized: bool,
}

impl UsbAudioDriver {
    pub const fn new() -> Self {
        Self {
            devices: Vec::new(),
            default_playback_device: None,
            default_capture_device: None,
            initialized: false,
        }
    }

    pub fn init(&mut self) {
        crate::kprintln!("usb_audio: initializing USB Audio driver");
        self.initialized = true;
    }

    pub fn register_device(&mut self, device: UsbAudioDevice) -> usize {
        let index = self.devices.len();

        crate::kprintln!("usb_audio: registered device {} ({}:{:04X}:{:04X})",
            device.name,
            device.slot_id,
            device.vendor_id,
            device.product_id
        );

        // Set as default if first device with capability
        if device.has_playback() && self.default_playback_device.is_none() {
            self.default_playback_device = Some(index);
            crate::kprintln!("usb_audio: set as default playback device");
        }

        if device.has_capture() && self.default_capture_device.is_none() {
            self.default_capture_device = Some(index);
            crate::kprintln!("usb_audio: set as default capture device");
        }

        self.devices.push(device);
        index
    }

    pub fn unregister_device(&mut self, index: usize) {
        if index < self.devices.len() {
            let device = &self.devices[index];
            crate::kprintln!("usb_audio: unregistering device {}", device.name);

            if self.default_playback_device == Some(index) {
                self.default_playback_device = None;
            }
            if self.default_capture_device == Some(index) {
                self.default_capture_device = None;
            }

            self.devices.remove(index);
        }
    }

    pub fn get_device(&self, index: usize) -> Option<&UsbAudioDevice> {
        self.devices.get(index)
    }

    pub fn get_device_mut(&mut self, index: usize) -> Option<&mut UsbAudioDevice> {
        self.devices.get_mut(index)
    }

    pub fn device_count(&self) -> usize {
        self.devices.len()
    }

    pub fn list_devices(&self) -> &[UsbAudioDevice] {
        &self.devices
    }

    pub fn default_playback(&self) -> Option<&UsbAudioDevice> {
        self.default_playback_device.and_then(|i| self.devices.get(i))
    }

    pub fn default_capture(&self) -> Option<&UsbAudioDevice> {
        self.default_capture_device.and_then(|i| self.devices.get(i))
    }

    pub fn set_default_playback(&mut self, index: usize) {
        if index < self.devices.len() && self.devices[index].has_playback() {
            self.default_playback_device = Some(index);
        }
    }

    pub fn set_default_capture(&mut self, index: usize) {
        if index < self.devices.len() && self.devices[index].has_capture() {
            self.default_capture_device = Some(index);
        }
    }
}

/// Global USB audio driver instance
static USB_AUDIO_DRIVER: TicketSpinlock<UsbAudioDriver> = TicketSpinlock::new(UsbAudioDriver::new());

/// Initialize USB audio driver
pub fn init() {
    USB_AUDIO_DRIVER.lock().init();
}

/// Parse audio control interface descriptors
pub fn parse_ac_interface(data: &[u8]) -> Option<(Vec<Terminal>, Vec<Terminal>, Vec<FeatureUnit>)> {
    if data.len() < 8 {
        return None;
    }

    let mut input_terminals = Vec::new();
    let mut output_terminals = Vec::new();
    let mut feature_units = Vec::new();

    let mut pos = 0;
    while pos < data.len() {
        let len = data[pos] as usize;
        if len < 3 || pos + len > data.len() {
            break;
        }

        let desc_type = data[pos + 1];
        let desc_subtype = data[pos + 2];

        if desc_type == class_codes::CS_INTERFACE {
            match desc_subtype {
                class_codes::AC_INPUT_TERMINAL if len >= 12 => {
                    let terminal = Terminal {
                        id: data[pos + 3],
                        terminal_type: u16::from_le_bytes([data[pos + 4], data[pos + 5]]),
                        assoc_terminal: data[pos + 6],
                        channels: data[pos + 7],
                        channel_config: ChannelConfig(u16::from_le_bytes([data[pos + 8], data[pos + 9]])),
                        string_index: data[pos + 11],
                        is_input: true,
                    };
                    input_terminals.push(terminal);
                }
                class_codes::AC_OUTPUT_TERMINAL if len >= 9 => {
                    let terminal = Terminal {
                        id: data[pos + 3],
                        terminal_type: u16::from_le_bytes([data[pos + 4], data[pos + 5]]),
                        assoc_terminal: data[pos + 6],
                        channels: 0, // Output terminals don't specify channels
                        channel_config: ChannelConfig(0),
                        string_index: data[pos + 8],
                        is_input: false,
                    };
                    output_terminals.push(terminal);
                }
                class_codes::AC_FEATURE_UNIT if len >= 7 => {
                    let control_size = data[pos + 5] as usize;
                    let num_channels = if control_size > 0 {
                        (len - 7) / control_size
                    } else {
                        0
                    };

                    let mut controls = Vec::new();
                    for i in 0..=num_channels {
                        let offset = pos + 6 + i * control_size;
                        if offset + control_size <= pos + len {
                            let ctrl = if control_size == 1 {
                                data[offset] as u16
                            } else if control_size >= 2 {
                                u16::from_le_bytes([data[offset], data[offset + 1]])
                            } else {
                                0
                            };
                            controls.push(FeatureControls(ctrl));
                        }
                    }

                    let feature = FeatureUnit {
                        id: data[pos + 3],
                        source_id: data[pos + 4],
                        controls,
                        string_index: data[pos + len - 1],
                    };
                    feature_units.push(feature);
                }
                _ => {}
            }
        }

        pos += len;
    }

    Some((input_terminals, output_terminals, feature_units))
}

/// Parse audio streaming interface format
pub fn parse_as_format(data: &[u8]) -> Option<StreamingFormat> {
    if data.len() < 8 {
        return None;
    }

    let format_tag = u16::from_le_bytes([data[0], data[1]]);
    let channels = data[2];
    let sub_frame_size = data[3];
    let bit_resolution = data[4];
    let freq_type = data[5];

    let mut sample_rates = Vec::new();
    let mut continuous = None;

    if freq_type == 0 {
        // Continuous frequencies
        if data.len() >= 12 {
            let min_rate = u32::from_le_bytes([data[6], data[7], data[8], 0]);
            let max_rate = u32::from_le_bytes([data[9], data[10], data[11], 0]);
            continuous = Some((SampleRate(min_rate), SampleRate(max_rate)));
        }
    } else {
        // Discrete frequencies
        for i in 0..freq_type as usize {
            let offset = 6 + i * 3;
            if offset + 3 <= data.len() {
                let rate = u32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], 0]);
                sample_rates.push(SampleRate(rate));
            }
        }
    }

    Some(StreamingFormat {
        format: AudioFormat::from_format_tag(format_tag),
        channels,
        sub_frame_size,
        bit_resolution,
        sample_rates,
        continuous_rates: continuous,
    })
}

/// Check if interface is USB Audio Class
pub fn is_audio_interface(class: u8, subclass: u8) -> bool {
    class == class_codes::AUDIO &&
    (subclass == class_codes::AUDIOCONTROL || subclass == class_codes::AUDIOSTREAMING)
}

/// Register a USB audio device
pub fn register_device(device: UsbAudioDevice) -> usize {
    USB_AUDIO_DRIVER.lock().register_device(device)
}

/// Unregister a USB audio device
pub fn unregister_device(index: usize) {
    USB_AUDIO_DRIVER.lock().unregister_device(index)
}

/// Get device count
pub fn device_count() -> usize {
    USB_AUDIO_DRIVER.lock().device_count()
}

/// Get device info string
pub fn format_devices() -> String {
    use core::fmt::Write;
    let mut output = String::new();
    let driver = USB_AUDIO_DRIVER.lock();

    writeln!(output, "USB Audio Devices: {}", driver.device_count()).ok();

    for (i, device) in driver.list_devices().iter().enumerate() {
        let default_pb = driver.default_playback_device == Some(i);
        let default_cap = driver.default_capture_device == Some(i);

        let mut flags = String::new();
        if default_pb { flags.push_str(" [default playback]"); }
        if default_cap { flags.push_str(" [default capture]"); }

        writeln!(output, "  [{}] {} ({:04X}:{:04X}){}",
            i,
            device.name,
            device.vendor_id,
            device.product_id,
            flags
        ).ok();

        if device.has_playback() {
            let rates = device.supported_playback_rates();
            let rates_str: Vec<String> = rates.iter().map(|r| format!("{}Hz", r.0)).collect();
            writeln!(output, "      Playback: {}", rates_str.join(", ")).ok();
        }

        if device.has_capture() {
            let rates = device.supported_capture_rates();
            let rates_str: Vec<String> = rates.iter().map(|r| format!("{}Hz", r.0)).collect();
            writeln!(output, "      Capture: {}", rates_str.join(", ")).ok();
        }

        writeln!(output, "      Volume: {}%{}",
            device.get_volume(),
            if device.is_muted() { " (muted)" } else { "" }
        ).ok();
    }

    output
}

/// Audio class request types
pub mod requests {
    pub const SET_CUR: u8 = 0x01;
    pub const SET_MIN: u8 = 0x02;
    pub const SET_MAX: u8 = 0x03;
    pub const SET_RES: u8 = 0x04;
    pub const GET_CUR: u8 = 0x81;
    pub const GET_MIN: u8 = 0x82;
    pub const GET_MAX: u8 = 0x83;
    pub const GET_RES: u8 = 0x84;

    // UAC2 requests
    pub const CUR: u8 = 0x01;
    pub const RANGE: u8 = 0x02;

    // Control selectors
    pub const FU_MUTE_CONTROL: u8 = 0x01;
    pub const FU_VOLUME_CONTROL: u8 = 0x02;
    pub const FU_BASS_CONTROL: u8 = 0x03;
    pub const FU_MID_CONTROL: u8 = 0x04;
    pub const FU_TREBLE_CONTROL: u8 = 0x05;
    pub const FU_GRAPHIC_EQUALIZER_CONTROL: u8 = 0x06;
    pub const FU_AUTOMATIC_GAIN_CONTROL: u8 = 0x07;
    pub const FU_DELAY_CONTROL: u8 = 0x08;
    pub const FU_BASS_BOOST_CONTROL: u8 = 0x09;
    pub const FU_LOUDNESS_CONTROL: u8 = 0x0A;

    // Clock source control selectors (UAC2)
    pub const CS_SAM_FREQ_CONTROL: u8 = 0x01;
    pub const CS_CLOCK_VALID_CONTROL: u8 = 0x02;
}

/// Volume in dB (1/256 dB units)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VolumeDb(pub i16);

impl VolumeDb {
    pub const SILENCE: Self = VolumeDb(-0x8000);
    pub const MIN_AUDIBLE: Self = VolumeDb(-0x7FFF);
    pub const ZERO_DB: Self = VolumeDb(0);
    pub const MAX: Self = VolumeDb(0x7FFF);

    /// Convert from percentage (0-100) to dB value.
    /// Uses a lookup table approximation for no_std compatibility.
    pub fn from_percent(percent: u8) -> Self {
        if percent == 0 {
            Self::SILENCE
        } else if percent >= 100 {
            Self::ZERO_DB
        } else {
            // Approximate logarithmic curve using piecewise linear
            // 100% = 0dB, 50% = -6dB, 25% = -12dB, 10% = -20dB, 1% = -40dB
            let db_256 = match percent {
                90..=99 => -((100 - percent as i16) * 26),   // -0.26dB per percent
                70..=89 => -256 - ((90 - percent as i16) * 51), // ~-6dB at 70%
                50..=69 => -1280 - ((70 - percent as i16) * 64), // ~-10dB at 50%
                25..=49 => -2560 - ((50 - percent as i16) * 102), // ~-15dB at 25%
                10..=24 => -5120 - ((25 - percent as i16) * 170), // ~-25dB at 10%
                1..=9 => -7680 - ((10 - percent as i16) * 256),   // ~-40dB at 1%
                _ => -10240,
            };
            VolumeDb(db_256)
        }
    }

    /// Convert from dB to percentage (0-100).
    /// Uses inverse of the from_percent approximation.
    pub fn to_percent(&self) -> u8 {
        if self.0 == -0x8000 {
            0
        } else if self.0 >= 0 {
            100
        } else {
            // Inverse of the piecewise approximation
            let db_256 = self.0 as i32;
            if db_256 > -256 {
                (100 + db_256 / 26) as u8
            } else if db_256 > -1280 {
                (90 + (db_256 + 256) / 51) as u8
            } else if db_256 > -2560 {
                (70 + (db_256 + 1280) / 64) as u8
            } else if db_256 > -5120 {
                (50 + (db_256 + 2560) / 102) as u8
            } else if db_256 > -7680 {
                (25 + (db_256 + 5120) / 170) as u8
            } else if db_256 > -10240 {
                (10 + (db_256 + 7680) / 256) as u8
            } else {
                1
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_count() {
        assert_eq!(ChannelConfig::MONO.channel_count(), 1);
        assert_eq!(ChannelConfig::STEREO.channel_count(), 2);
        assert_eq!(ChannelConfig::SURROUND_51.channel_count(), 6);
        assert_eq!(ChannelConfig::SURROUND_71.channel_count(), 8);
    }

    #[test]
    fn test_sample_rate_support() {
        let format = StreamingFormat {
            format: AudioFormat::Pcm,
            channels: 2,
            sub_frame_size: 2,
            bit_resolution: 16,
            sample_rates: vec![SampleRate::RATE_44100, SampleRate::RATE_48000],
            continuous_rates: None,
        };

        assert!(format.supports_rate(SampleRate::RATE_44100));
        assert!(format.supports_rate(SampleRate::RATE_48000));
        assert!(!format.supports_rate(SampleRate::RATE_96000));
    }
}
