//! Audio Mixer
//!
//! Software audio mixer that:
//! - Combines multiple audio streams into a single output
//! - Supports per-channel and master volume control
//! - Provides channel routing and panning
//! - Handles sample rate conversion (basic)
//! - Supports audio effects (basic equalizer)

#![allow(dead_code)]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use crate::sync::IrqSafeMutex;

use super::SampleFormat;

/// Unique mixer channel ID
pub type ChannelId = u32;

/// Audio channel type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelType {
    /// Master output channel
    Master,
    /// PCM playback (music, videos, etc.)
    Pcm,
    /// System sounds (notifications, beeps)
    System,
    /// Voice/VoIP applications
    Voice,
    /// Recording input
    Capture,
    /// Microphone monitor (loopback)
    Monitor,
    /// Custom application channel
    Application,
}

impl ChannelType {
    /// Get default volume for channel type
    pub fn default_volume(&self) -> u8 {
        match self {
            ChannelType::Master => 100,
            ChannelType::Pcm => 100,
            ChannelType::System => 80,
            ChannelType::Voice => 100,
            ChannelType::Capture => 100,
            ChannelType::Monitor => 0, // Disabled by default
            ChannelType::Application => 100,
        }
    }

    /// Get default priority (for mixing order)
    pub fn priority(&self) -> u8 {
        match self {
            ChannelType::Master => 255,
            ChannelType::System => 200,
            ChannelType::Voice => 150,
            ChannelType::Pcm => 100,
            ChannelType::Application => 80,
            ChannelType::Capture => 50,
            ChannelType::Monitor => 10,
        }
    }
}

/// Pan position (-100 = full left, 0 = center, +100 = full right)
pub type PanPosition = i8;

/// Mixer channel
pub struct MixerChannel {
    /// Channel ID
    id: ChannelId,
    /// Channel name
    name: String,
    /// Channel type
    channel_type: ChannelType,
    /// Volume (0-100)
    volume: AtomicU32,
    /// Muted
    muted: AtomicBool,
    /// Pan position
    pan: AtomicU32, // Stored as i8 + 128
    /// Sample rate (Hz)
    sample_rate: u32,
    /// Sample format
    format: SampleFormat,
    /// Number of channels (1=mono, 2=stereo, etc.)
    num_channels: u8,
    /// Audio buffer (interleaved samples)
    buffer: IrqSafeMutex<Vec<i16>>,
    /// Buffer read position
    read_pos: AtomicU64,
    /// Buffer write position
    write_pos: AtomicU64,
    /// Active (receiving data)
    active: AtomicBool,
    /// Solo mode (only this channel plays)
    solo: AtomicBool,
    /// Peak level (for metering)
    peak_left: AtomicU32,
    /// Peak level right
    peak_right: AtomicU32,
    /// Equalizer bands (10-band: 31Hz, 62Hz, 125Hz, 250Hz, 500Hz, 1kHz, 2kHz, 4kHz, 8kHz, 16kHz)
    eq_bands: [AtomicU32; 10],
    /// Equalizer enabled
    eq_enabled: AtomicBool,
}

impl MixerChannel {
    /// Create a new mixer channel
    pub fn new(
        id: ChannelId,
        name: String,
        channel_type: ChannelType,
        sample_rate: u32,
        format: SampleFormat,
        num_channels: u8,
    ) -> Self {
        let buffer_size = (sample_rate as usize) * 2; // 2 seconds buffer

        Self {
            id,
            name,
            channel_type,
            volume: AtomicU32::new(channel_type.default_volume() as u32),
            muted: AtomicBool::new(false),
            pan: AtomicU32::new(128), // Center (0 + 128)
            sample_rate,
            format,
            num_channels,
            buffer: IrqSafeMutex::new(vec![0i16; buffer_size * num_channels as usize]),
            read_pos: AtomicU64::new(0),
            write_pos: AtomicU64::new(0),
            active: AtomicBool::new(false),
            solo: AtomicBool::new(false),
            peak_left: AtomicU32::new(0),
            peak_right: AtomicU32::new(0),
            eq_bands: default_eq_bands(),
            eq_enabled: AtomicBool::new(false),
        }
    }

    /// Get channel ID
    pub fn id(&self) -> ChannelId {
        self.id
    }

    /// Get channel name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get channel type
    pub fn channel_type(&self) -> ChannelType {
        self.channel_type
    }

    /// Set volume (0-100)
    pub fn set_volume(&self, volume: u8) {
        self.volume.store(volume.min(100) as u32, Ordering::Relaxed);
    }

    /// Get volume
    pub fn volume(&self) -> u8 {
        self.volume.load(Ordering::Relaxed) as u8
    }

    /// Set muted
    pub fn set_muted(&self, muted: bool) {
        self.muted.store(muted, Ordering::Relaxed);
    }

    /// Is muted
    pub fn is_muted(&self) -> bool {
        self.muted.load(Ordering::Relaxed)
    }

    /// Set pan position
    pub fn set_pan(&self, pan: PanPosition) {
        self.pan.store((pan as i32 + 128) as u32, Ordering::Relaxed);
    }

    /// Get pan position
    pub fn pan(&self) -> PanPosition {
        (self.pan.load(Ordering::Relaxed) as i32 - 128) as PanPosition
    }

    /// Set solo mode
    pub fn set_solo(&self, solo: bool) {
        self.solo.store(solo, Ordering::Relaxed);
    }

    /// Is in solo mode
    pub fn is_solo(&self) -> bool {
        self.solo.load(Ordering::Relaxed)
    }

    /// Is active
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    /// Get peak levels (left, right)
    pub fn peak_levels(&self) -> (u32, u32) {
        (
            self.peak_left.load(Ordering::Relaxed),
            self.peak_right.load(Ordering::Relaxed),
        )
    }

    /// Reset peak meters
    pub fn reset_peaks(&self) {
        self.peak_left.store(0, Ordering::Relaxed);
        self.peak_right.store(0, Ordering::Relaxed);
    }

    /// Set equalizer band (index 0-9, gain -12 to +12 dB, stored as 0-24)
    pub fn set_eq_band(&self, band: usize, gain_db: i8) {
        if band < 10 {
            let stored = (gain_db.clamp(-12, 12) + 12) as u32;
            self.eq_bands[band].store(stored, Ordering::Relaxed);
        }
    }

    /// Get equalizer band
    pub fn eq_band(&self, band: usize) -> i8 {
        if band < 10 {
            (self.eq_bands[band].load(Ordering::Relaxed) as i8) - 12
        } else {
            0
        }
    }

    /// Enable/disable equalizer
    pub fn set_eq_enabled(&self, enabled: bool) {
        self.eq_enabled.store(enabled, Ordering::Relaxed);
    }

    /// Is equalizer enabled
    pub fn eq_enabled(&self) -> bool {
        self.eq_enabled.load(Ordering::Relaxed)
    }

    /// Write samples to buffer (returns number of samples written)
    pub fn write_samples(&self, samples: &[i16]) -> usize {
        let mut buffer = self.buffer.lock();
        let buffer_len = buffer.len();

        let write_pos = self.write_pos.load(Ordering::Acquire) as usize;
        let read_pos = self.read_pos.load(Ordering::Acquire) as usize;

        // Calculate available space
        let available = if write_pos >= read_pos {
            buffer_len - (write_pos - read_pos) - 1
        } else {
            read_pos - write_pos - 1
        };

        let to_write = samples.len().min(available);
        if to_write == 0 {
            return 0;
        }

        // Write samples with wrap-around
        let mut pos = write_pos;
        for &sample in &samples[..to_write] {
            buffer[pos] = sample;
            pos = (pos + 1) % buffer_len;
        }

        self.write_pos.store(pos as u64, Ordering::Release);
        self.active.store(true, Ordering::Relaxed);

        to_write
    }

    /// Read and mix samples (returns number of frames mixed)
    pub fn read_mix(&self, output: &mut [i32], master_volume: u8) -> usize {
        if self.is_muted() || !self.is_active() {
            return 0;
        }

        let volume = self.volume() as i32;
        let master = master_volume as i32;
        let pan = self.pan();

        // Calculate left/right gains based on pan
        let (left_gain, right_gain) = calculate_pan_gains(pan);

        let buffer = self.buffer.lock();
        let buffer_len = buffer.len();

        let write_pos = self.write_pos.load(Ordering::Acquire) as usize;
        let read_pos = self.read_pos.load(Ordering::Acquire) as usize;

        // Calculate available samples
        let available = if write_pos >= read_pos {
            write_pos - read_pos
        } else {
            buffer_len - read_pos + write_pos
        };

        // Process stereo frames
        let frames_to_read = (available / 2).min(output.len() / 2);
        if frames_to_read == 0 {
            self.active.store(false, Ordering::Relaxed);
            return 0;
        }

        let mut pos = read_pos;
        let mut max_left: i32 = 0;
        let mut max_right: i32 = 0;

        for frame in 0..frames_to_read {
            let left_sample = buffer[pos] as i32;
            pos = (pos + 1) % buffer_len;
            let right_sample = if self.num_channels >= 2 {
                let s = buffer[pos] as i32;
                pos = (pos + 1) % buffer_len;
                s
            } else {
                left_sample
            };

            // Apply volume and pan
            let left = (left_sample * volume * master * left_gain / 1000000) as i32;
            let right = (right_sample * volume * master * right_gain / 1000000) as i32;

            // Mix into output
            output[frame * 2] += left;
            output[frame * 2 + 1] += right;

            // Update peak meters
            max_left = max_left.max(left.abs());
            max_right = max_right.max(right.abs());
        }

        // Update read position
        drop(buffer);
        self.read_pos.store(pos as u64, Ordering::Release);

        // Update peak meters
        self.peak_left.fetch_max(max_left as u32, Ordering::Relaxed);
        self.peak_right.fetch_max(max_right as u32, Ordering::Relaxed);

        frames_to_read
    }

    /// Available buffer space (in samples)
    pub fn available_space(&self) -> usize {
        let buffer = self.buffer.lock();
        let buffer_len = buffer.len();

        let write_pos = self.write_pos.load(Ordering::Acquire) as usize;
        let read_pos = self.read_pos.load(Ordering::Acquire) as usize;

        if write_pos >= read_pos {
            buffer_len - (write_pos - read_pos) - 1
        } else {
            read_pos - write_pos - 1
        }
    }

    /// Available samples to read
    pub fn available_samples(&self) -> usize {
        let buffer = self.buffer.lock();
        let buffer_len = buffer.len();

        let write_pos = self.write_pos.load(Ordering::Acquire) as usize;
        let read_pos = self.read_pos.load(Ordering::Acquire) as usize;

        if write_pos >= read_pos {
            write_pos - read_pos
        } else {
            buffer_len - read_pos + write_pos
        }
    }

    /// Clear buffer
    pub fn clear(&self) {
        let mut buffer = self.buffer.lock();
        for sample in buffer.iter_mut() {
            *sample = 0;
        }
        self.read_pos.store(0, Ordering::Release);
        self.write_pos.store(0, Ordering::Release);
        self.active.store(false, Ordering::Relaxed);
    }
}

/// Calculate left and right gain from pan position
/// Returns (left_gain, right_gain) as values from 0-100
fn calculate_pan_gains(pan: PanPosition) -> (i32, i32) {
    let pan = pan.clamp(-100, 100) as i32;

    if pan == 0 {
        (100, 100)
    } else if pan < 0 {
        // Pan left: reduce right channel
        let right = ((100 + pan) * 100 / 100) as i32;
        (100, right.max(0))
    } else {
        // Pan right: reduce left channel
        let left = ((100 - pan) * 100 / 100) as i32;
        (left.max(0), 100)
    }
}

/// Audio Mixer
pub struct AudioMixer {
    /// Mixer name
    name: String,
    /// Channels
    channels: BTreeMap<ChannelId, MixerChannel>,
    /// Next channel ID
    next_channel_id: AtomicU32,
    /// Master output channel
    master_channel: Option<ChannelId>,
    /// Output sample rate
    output_sample_rate: u32,
    /// Output format
    output_format: SampleFormat,
    /// Output channels (1=mono, 2=stereo)
    output_num_channels: u8,
    /// Mixer active
    active: AtomicBool,
    /// Any channel in solo mode
    has_solo: AtomicBool,
}

impl AudioMixer {
    /// Create a new audio mixer
    pub fn new(name: &str, sample_rate: u32, num_channels: u8) -> Self {
        let mut mixer = Self {
            name: String::from(name),
            channels: BTreeMap::new(),
            next_channel_id: AtomicU32::new(1),
            master_channel: None,
            output_sample_rate: sample_rate,
            output_format: SampleFormat::S16LE,
            output_num_channels: num_channels,
            active: AtomicBool::new(false),
            has_solo: AtomicBool::new(false),
        };

        // Create master channel
        let master_id = mixer.add_channel("Master", ChannelType::Master);
        mixer.master_channel = Some(master_id);

        mixer
    }

    /// Get mixer name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Add a channel
    pub fn add_channel(&mut self, name: &str, channel_type: ChannelType) -> ChannelId {
        let id = self.next_channel_id.fetch_add(1, Ordering::Relaxed);

        let channel = MixerChannel::new(
            id,
            String::from(name),
            channel_type,
            self.output_sample_rate,
            self.output_format,
            self.output_num_channels,
        );

        self.channels.insert(id, channel);
        id
    }

    /// Remove a channel
    pub fn remove_channel(&mut self, id: ChannelId) -> bool {
        // Don't allow removing master
        if Some(id) == self.master_channel {
            return false;
        }
        self.channels.remove(&id).is_some()
    }

    /// Get channel by ID
    pub fn get_channel(&self, id: ChannelId) -> Option<&MixerChannel> {
        self.channels.get(&id)
    }

    /// Get channel by ID (mutable)
    pub fn get_channel_mut(&mut self, id: ChannelId) -> Option<&mut MixerChannel> {
        self.channels.get_mut(&id)
    }

    /// Get master channel
    pub fn master_channel(&self) -> Option<&MixerChannel> {
        self.master_channel.and_then(|id| self.channels.get(&id))
    }

    /// Get master channel (mutable)
    pub fn master_channel_mut(&mut self) -> Option<&mut MixerChannel> {
        let id = self.master_channel?;
        self.channels.get_mut(&id)
    }

    /// Get all channels
    pub fn channels(&self) -> impl Iterator<Item = &MixerChannel> {
        self.channels.values()
    }

    /// Find channel by name
    pub fn find_channel_by_name(&self, name: &str) -> Option<&MixerChannel> {
        self.channels.values().find(|c| c.name() == name)
    }

    /// Find channel by type
    pub fn find_channels_by_type(&self, channel_type: ChannelType) -> Vec<&MixerChannel> {
        self.channels
            .values()
            .filter(|c| c.channel_type() == channel_type)
            .collect()
    }

    /// Set master volume
    pub fn set_master_volume(&mut self, volume: u8) {
        if let Some(master) = self.master_channel_mut() {
            master.set_volume(volume);
        }
    }

    /// Get master volume
    pub fn master_volume(&self) -> u8 {
        self.master_channel().map(|c| c.volume()).unwrap_or(100)
    }

    /// Set master mute
    pub fn set_master_mute(&mut self, muted: bool) {
        if let Some(master) = self.master_channel_mut() {
            master.set_muted(muted);
        }
    }

    /// Is master muted
    pub fn is_master_muted(&self) -> bool {
        self.master_channel().map(|c| c.is_muted()).unwrap_or(false)
    }

    /// Update solo state
    fn update_solo_state(&self) {
        let has_solo = self.channels.values().any(|c| c.is_solo());
        self.has_solo.store(has_solo, Ordering::Relaxed);
    }

    /// Mix all channels into output buffer
    /// Returns number of frames mixed
    pub fn mix(&mut self, output: &mut [i16]) -> usize {
        if self.is_master_muted() {
            // Zero output when master muted
            for sample in output.iter_mut() {
                *sample = 0;
            }
            return output.len() / 2;
        }

        let master_volume = self.master_volume();
        let has_solo = self.has_solo.load(Ordering::Relaxed);

        // Temporary mixing buffer (32-bit to avoid overflow)
        let mut mix_buffer: Vec<i32> = vec![0i32; output.len()];

        // Collect channels sorted by priority
        let mut channel_ids: Vec<_> = self.channels.keys().copied().collect();
        channel_ids.sort_by_key(|id| {
            self.channels.get(id).map(|c| c.channel_type().priority()).unwrap_or(0)
        });
        channel_ids.reverse(); // Higher priority first

        // Mix each channel
        for id in channel_ids {
            let channel = match self.channels.get(&id) {
                Some(c) => c,
                None => continue,
            };

            // Skip if in solo mode and this channel isn't solo
            if has_solo && !channel.is_solo() && channel.channel_type() != ChannelType::Master {
                continue;
            }

            // Skip master channel (it's used for output volume only)
            if channel.channel_type() == ChannelType::Master {
                continue;
            }

            channel.read_mix(&mut mix_buffer, master_volume);
        }

        // Convert to output format with clipping
        let frames = output.len() / 2;
        for i in 0..frames * 2 {
            // Clip to i16 range
            let sample = mix_buffer[i].clamp(-32768, 32767) as i16;
            output[i] = sample;
        }

        frames
    }

    /// Write samples to a channel
    pub fn write_to_channel(&self, channel_id: ChannelId, samples: &[i16]) -> usize {
        match self.channels.get(&channel_id) {
            Some(channel) => channel.write_samples(samples),
            None => 0,
        }
    }

    /// Get number of channels
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }

    /// Is mixer active
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    /// Set mixer active
    pub fn set_active(&self, active: bool) {
        self.active.store(active, Ordering::Relaxed);
    }
}

/// Mixer preset for common configurations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MixerPreset {
    /// Music playback (full frequency response)
    Music,
    /// Movie playback (enhanced bass, dialog boost)
    Movie,
    /// Voice chat (reduce bass, boost mid-range)
    Voice,
    /// Gaming (enhanced surround effect)
    Gaming,
    /// Flat response (no EQ)
    Flat,
}

impl MixerPreset {
    /// Get equalizer settings for preset
    pub fn eq_settings(&self) -> [i8; 10] {
        match self {
            MixerPreset::Music => [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            MixerPreset::Movie => [3, 4, 2, -1, 0, 1, 2, 3, 2, 1],
            MixerPreset::Voice => [-3, -2, 0, 2, 3, 4, 3, 1, 0, -1],
            MixerPreset::Gaming => [2, 3, 1, 0, -1, 0, 1, 3, 4, 3],
            MixerPreset::Flat => [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        }
    }

    /// Apply preset to a channel
    pub fn apply_to_channel(&self, channel: &MixerChannel) {
        let settings = self.eq_settings();
        for (i, &gain) in settings.iter().enumerate() {
            channel.set_eq_band(i, gain);
        }
        channel.set_eq_enabled(*self != MixerPreset::Flat);
    }
}

/// Create default equalizer bands (all 0 dB)
fn default_eq_bands() -> [AtomicU32; 10] {
    [
        AtomicU32::new(12), // 0 dB (12 = 0 + 12)
        AtomicU32::new(12),
        AtomicU32::new(12),
        AtomicU32::new(12),
        AtomicU32::new(12),
        AtomicU32::new(12),
        AtomicU32::new(12),
        AtomicU32::new(12),
        AtomicU32::new(12),
        AtomicU32::new(12),
    ]
}

// =============================================================================
// Global mixer instance
// =============================================================================

static GLOBAL_MIXER: IrqSafeMutex<Option<AudioMixer>> = IrqSafeMutex::new(None);

/// Initialize global mixer
pub fn init() {
    let mixer = AudioMixer::new("System Mixer", 44100, 2);
    *GLOBAL_MIXER.lock() = Some(mixer);
    crate::kprintln!("audio_mixer: initialized system mixer");
}

/// Get global mixer lock guard
pub fn with_mixer<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&AudioMixer) -> R,
{
    GLOBAL_MIXER.lock().as_ref().map(f)
}

/// Get global mixer lock guard (mutable)
pub fn with_mixer_mut<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut AudioMixer) -> R,
{
    GLOBAL_MIXER.lock().as_mut().map(f)
}

/// Add a channel to global mixer
pub fn add_channel(name: &str, channel_type: ChannelType) -> Option<ChannelId> {
    GLOBAL_MIXER.lock().as_mut().map(|m| m.add_channel(name, channel_type))
}

/// Set master volume
pub fn set_master_volume(volume: u8) {
    if let Some(mixer) = GLOBAL_MIXER.lock().as_mut() {
        mixer.set_master_volume(volume);
    }
}

/// Get master volume
pub fn get_master_volume() -> u8 {
    GLOBAL_MIXER.lock().as_ref().map(|m| m.master_volume()).unwrap_or(100)
}

/// Set master mute
pub fn set_master_mute(muted: bool) {
    if let Some(mixer) = GLOBAL_MIXER.lock().as_mut() {
        mixer.set_master_mute(muted);
    }
}

/// Is master muted
pub fn is_master_muted() -> bool {
    GLOBAL_MIXER.lock().as_ref().map(|m| m.is_master_muted()).unwrap_or(false)
}

/// Mix and get output
pub fn mix_output(output: &mut [i16]) -> usize {
    GLOBAL_MIXER.lock().as_mut().map(|m| m.mix(output)).unwrap_or(0)
}
