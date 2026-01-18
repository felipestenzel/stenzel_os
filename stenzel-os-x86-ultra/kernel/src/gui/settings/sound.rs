//! Sound Settings
//!
//! Volume, audio devices, input/output, and sound effects settings.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use spin::Mutex;

/// Global sound settings state
static SOUND_SETTINGS: Mutex<Option<SoundSettings>> = Mutex::new(None);

/// Sound settings state
pub struct SoundSettings {
    /// Master volume (0-100)
    pub master_volume: u32,
    /// Muted
    pub muted: bool,
    /// Output devices
    pub output_devices: Vec<AudioDevice>,
    /// Selected output device
    pub selected_output: Option<u32>,
    /// Input devices
    pub input_devices: Vec<AudioDevice>,
    /// Selected input device
    pub selected_input: Option<u32>,
    /// Input volume (0-100)
    pub input_volume: u32,
    /// Input muted
    pub input_muted: bool,
    /// Sound effects volume
    pub effects_volume: u32,
    /// Alert sounds enabled
    pub alert_sounds: bool,
    /// Per-application volumes
    pub app_volumes: Vec<AppVolume>,
}

/// Audio device
#[derive(Debug, Clone)]
pub struct AudioDevice {
    /// Device ID
    pub id: u32,
    /// Device name
    pub name: String,
    /// Device description
    pub description: String,
    /// Device type
    pub device_type: AudioDeviceType,
    /// Is default
    pub is_default: bool,
    /// Is available
    pub is_available: bool,
    /// Volume (0-100)
    pub volume: u32,
    /// Muted
    pub muted: bool,
    /// Balance (-100 to 100, 0 is center)
    pub balance: i32,
}

/// Audio device type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioDeviceType {
    /// Built-in speakers
    BuiltinSpeaker,
    /// Built-in microphone
    BuiltinMicrophone,
    /// Headphones
    Headphones,
    /// Headset (with mic)
    Headset,
    /// HDMI output
    Hdmi,
    /// DisplayPort output
    DisplayPort,
    /// USB audio
    Usb,
    /// Bluetooth audio
    Bluetooth,
    /// Line in
    LineIn,
    /// Line out
    LineOut,
    /// Unknown
    Unknown,
}

impl AudioDeviceType {
    pub fn icon(&self) -> &'static str {
        match self {
            AudioDeviceType::BuiltinSpeaker => "audio-speakers",
            AudioDeviceType::BuiltinMicrophone => "audio-input-microphone",
            AudioDeviceType::Headphones => "audio-headphones",
            AudioDeviceType::Headset => "audio-headset",
            AudioDeviceType::Hdmi | AudioDeviceType::DisplayPort => "video-display",
            AudioDeviceType::Usb => "audio-card",
            AudioDeviceType::Bluetooth => "bluetooth",
            AudioDeviceType::LineIn | AudioDeviceType::LineOut => "audio-input-line",
            AudioDeviceType::Unknown => "audio-card",
        }
    }
}

/// Per-application volume
#[derive(Debug, Clone)]
pub struct AppVolume {
    /// Application name
    pub app_name: String,
    /// Application ID
    pub app_id: String,
    /// Volume (0-100)
    pub volume: u32,
    /// Muted
    pub muted: bool,
}

/// Initialize sound settings
pub fn init() {
    let mut state = SOUND_SETTINGS.lock();
    if state.is_some() {
        return;
    }

    // Initialize with default devices
    *state = Some(SoundSettings {
        master_volume: 75,
        muted: false,
        output_devices: vec![
            AudioDevice {
                id: 0,
                name: "Built-in Speakers".to_string(),
                description: "Internal audio output".to_string(),
                device_type: AudioDeviceType::BuiltinSpeaker,
                is_default: true,
                is_available: true,
                volume: 75,
                muted: false,
                balance: 0,
            },
        ],
        selected_output: Some(0),
        input_devices: vec![
            AudioDevice {
                id: 0,
                name: "Built-in Microphone".to_string(),
                description: "Internal microphone".to_string(),
                device_type: AudioDeviceType::BuiltinMicrophone,
                is_default: true,
                is_available: true,
                volume: 80,
                muted: false,
                balance: 0,
            },
        ],
        selected_input: Some(0),
        input_volume: 80,
        input_muted: false,
        effects_volume: 100,
        alert_sounds: true,
        app_volumes: Vec::new(),
    });

    crate::kprintln!("sound settings: initialized");
}

/// Set master volume
pub fn set_master_volume(volume: u32) {
    let mut state = SOUND_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.master_volume = volume.min(100);
        // TODO: Apply to audio system
    }
}

/// Get master volume
pub fn get_master_volume() -> u32 {
    let state = SOUND_SETTINGS.lock();
    state.as_ref().map(|s| s.master_volume).unwrap_or(50)
}

/// Set muted
pub fn set_muted(muted: bool) {
    let mut state = SOUND_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.muted = muted;
    }
}

/// Get muted
pub fn is_muted() -> bool {
    let state = SOUND_SETTINGS.lock();
    state.as_ref().map(|s| s.muted).unwrap_or(false)
}

/// Get output devices
pub fn get_output_devices() -> Vec<AudioDevice> {
    let state = SOUND_SETTINGS.lock();
    state.as_ref().map(|s| s.output_devices.clone()).unwrap_or_default()
}

/// Get input devices
pub fn get_input_devices() -> Vec<AudioDevice> {
    let state = SOUND_SETTINGS.lock();
    state.as_ref().map(|s| s.input_devices.clone()).unwrap_or_default()
}

/// Set output device
pub fn set_output_device(device_id: u32) -> Result<(), SoundError> {
    let mut state = SOUND_SETTINGS.lock();
    let state = state.as_mut().ok_or(SoundError::NotInitialized)?;

    if !state.output_devices.iter().any(|d| d.id == device_id) {
        return Err(SoundError::DeviceNotFound);
    }

    state.selected_output = Some(device_id);

    Ok(())
}

/// Set input device
pub fn set_input_device(device_id: u32) -> Result<(), SoundError> {
    let mut state = SOUND_SETTINGS.lock();
    let state = state.as_mut().ok_or(SoundError::NotInitialized)?;

    if !state.input_devices.iter().any(|d| d.id == device_id) {
        return Err(SoundError::DeviceNotFound);
    }

    state.selected_input = Some(device_id);

    Ok(())
}

/// Set device volume
pub fn set_device_volume(device_id: u32, volume: u32, is_output: bool) -> Result<(), SoundError> {
    let mut state = SOUND_SETTINGS.lock();
    let state = state.as_mut().ok_or(SoundError::NotInitialized)?;

    let devices = if is_output { &mut state.output_devices } else { &mut state.input_devices };
    let device = devices.iter_mut()
        .find(|d| d.id == device_id)
        .ok_or(SoundError::DeviceNotFound)?;

    device.volume = volume.min(100);

    Ok(())
}

/// Set device balance
pub fn set_device_balance(device_id: u32, balance: i32, is_output: bool) -> Result<(), SoundError> {
    let mut state = SOUND_SETTINGS.lock();
    let state = state.as_mut().ok_or(SoundError::NotInitialized)?;

    let devices = if is_output { &mut state.output_devices } else { &mut state.input_devices };
    let device = devices.iter_mut()
        .find(|d| d.id == device_id)
        .ok_or(SoundError::DeviceNotFound)?;

    device.balance = balance.clamp(-100, 100);

    Ok(())
}

/// Set input volume
pub fn set_input_volume(volume: u32) {
    let mut state = SOUND_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.input_volume = volume.min(100);
    }
}

/// Set input muted
pub fn set_input_muted(muted: bool) {
    let mut state = SOUND_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.input_muted = muted;
    }
}

/// Set application volume
pub fn set_app_volume(app_id: &str, volume: u32) {
    let mut state = SOUND_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        if let Some(app) = s.app_volumes.iter_mut().find(|a| a.app_id == app_id) {
            app.volume = volume.min(100);
        } else {
            s.app_volumes.push(AppVolume {
                app_name: app_id.to_string(),
                app_id: app_id.to_string(),
                volume: volume.min(100),
                muted: false,
            });
        }
    }
}

/// Get application volumes
pub fn get_app_volumes() -> Vec<AppVolume> {
    let state = SOUND_SETTINGS.lock();
    state.as_ref().map(|s| s.app_volumes.clone()).unwrap_or_default()
}

/// Set alert sounds enabled
pub fn set_alert_sounds(enabled: bool) {
    let mut state = SOUND_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.alert_sounds = enabled;
    }
}

/// Sound error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundError {
    NotInitialized,
    DeviceNotFound,
    ConfigFailed,
}
