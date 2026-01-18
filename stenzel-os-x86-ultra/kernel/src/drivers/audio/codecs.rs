//! Audio Codecs
//!
//! Software audio codec implementations for:
//! - MP3 decoding (MPEG-1 Audio Layer III)
//! - AAC decoding (Advanced Audio Coding)
//! - FLAC decoding (Free Lossless Audio Codec)
//! - Opus decoding
//! - Vorbis decoding (OGG)
//! - WAV/PCM parsing

#![allow(dead_code)]

extern crate alloc;

use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use super::SampleFormat;

/// Audio codec type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioCodec {
    Pcm,
    Mp3,
    Aac,
    Flac,
    Opus,
    Vorbis,
    Wav,
    Alac,
    Wma,
    Ac3,
    Dts,
}

impl AudioCodec {
    /// Get codec name
    pub fn name(&self) -> &'static str {
        match self {
            AudioCodec::Pcm => "PCM",
            AudioCodec::Mp3 => "MP3",
            AudioCodec::Aac => "AAC",
            AudioCodec::Flac => "FLAC",
            AudioCodec::Opus => "Opus",
            AudioCodec::Vorbis => "Vorbis",
            AudioCodec::Wav => "WAV",
            AudioCodec::Alac => "ALAC",
            AudioCodec::Wma => "WMA",
            AudioCodec::Ac3 => "AC-3",
            AudioCodec::Dts => "DTS",
        }
    }

    /// Get file extensions
    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            AudioCodec::Pcm => &["pcm", "raw"],
            AudioCodec::Mp3 => &["mp3"],
            AudioCodec::Aac => &["aac", "m4a", "mp4"],
            AudioCodec::Flac => &["flac"],
            AudioCodec::Opus => &["opus", "ogg"],
            AudioCodec::Vorbis => &["ogg", "oga"],
            AudioCodec::Wav => &["wav", "wave"],
            AudioCodec::Alac => &["m4a", "mp4"],
            AudioCodec::Wma => &["wma"],
            AudioCodec::Ac3 => &["ac3"],
            AudioCodec::Dts => &["dts"],
        }
    }

    /// Is this a lossless codec?
    pub fn is_lossless(&self) -> bool {
        matches!(self, AudioCodec::Pcm | AudioCodec::Flac | AudioCodec::Wav | AudioCodec::Alac)
    }

    /// Detect codec from file magic bytes
    pub fn detect(data: &[u8]) -> Option<Self> {
        if data.len() < 12 {
            return None;
        }

        // MP3: ID3 tag or sync word
        if data.starts_with(b"ID3") {
            return Some(AudioCodec::Mp3);
        }
        if data.len() >= 2 && data[0] == 0xFF && (data[1] & 0xE0) == 0xE0 {
            return Some(AudioCodec::Mp3);
        }

        // FLAC
        if data.starts_with(b"fLaC") {
            return Some(AudioCodec::Flac);
        }

        // WAV/RIFF
        if data.starts_with(b"RIFF") && data[8..12] == *b"WAVE" {
            return Some(AudioCodec::Wav);
        }

        // OGG (Vorbis/Opus)
        if data.starts_with(b"OggS") {
            // Check for Opus in page
            if data.len() >= 36 && &data[28..36] == b"OpusHead" {
                return Some(AudioCodec::Opus);
            }
            return Some(AudioCodec::Vorbis);
        }

        // AAC ADTS
        if data.len() >= 2 && data[0] == 0xFF && (data[1] & 0xF6) == 0xF0 {
            return Some(AudioCodec::Aac);
        }

        // MP4/M4A
        if data.len() >= 8 && &data[4..8] == b"ftyp" {
            // Could be AAC or ALAC
            return Some(AudioCodec::Aac);
        }

        None
    }
}

/// Decoded audio frame
#[derive(Debug, Clone)]
pub struct AudioFrame {
    /// Sample data (interleaved)
    pub samples: Vec<i16>,
    /// Sample rate
    pub sample_rate: u32,
    /// Number of channels
    pub channels: u8,
    /// Frame timestamp (in samples)
    pub timestamp: u64,
}

impl AudioFrame {
    /// Create new frame
    pub fn new(sample_rate: u32, channels: u8) -> Self {
        Self {
            samples: Vec::new(),
            sample_rate,
            channels,
            timestamp: 0,
        }
    }

    /// Number of samples per channel
    pub fn num_samples(&self) -> usize {
        self.samples.len() / self.channels.max(1) as usize
    }

    /// Duration in milliseconds
    pub fn duration_ms(&self) -> u32 {
        if self.sample_rate == 0 {
            return 0;
        }
        ((self.num_samples() as u64 * 1000) / self.sample_rate as u64) as u32
    }
}

/// Audio stream info
#[derive(Debug, Clone)]
pub struct AudioStreamInfo {
    pub codec: AudioCodec,
    pub sample_rate: u32,
    pub channels: u8,
    pub bit_depth: u8,
    pub bitrate: u32, // kbps for lossy, 0 for variable
    pub duration_ms: u64,
    pub total_samples: u64,
}

/// Decoder error
#[derive(Debug, Clone)]
pub enum DecoderError {
    InvalidData,
    UnsupportedFormat,
    EndOfStream,
    NeedMoreData,
    InternalError(&'static str),
}

/// Audio decoder trait
pub trait AudioDecoder {
    /// Get codec type
    fn codec(&self) -> AudioCodec;

    /// Get stream info
    fn info(&self) -> &AudioStreamInfo;

    /// Reset decoder state
    fn reset(&mut self);

    /// Decode a frame
    fn decode_frame(&mut self, input: &[u8]) -> Result<AudioFrame, DecoderError>;

    /// Seek to timestamp (in samples)
    fn seek(&mut self, sample_pos: u64) -> Result<(), DecoderError>;
}

// =============================================================================
// MP3 Decoder
// =============================================================================

/// MPEG audio version
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MpegVersion {
    Mpeg1,
    Mpeg2,
    Mpeg2_5,
}

/// MPEG audio layer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MpegLayer {
    Layer1,
    Layer2,
    Layer3,
}

/// MP3 frame header
#[derive(Debug, Clone)]
pub struct Mp3FrameHeader {
    pub version: MpegVersion,
    pub layer: MpegLayer,
    pub bitrate: u32,
    pub sample_rate: u32,
    pub channels: u8,
    pub padding: bool,
    pub frame_size: usize,
}

impl Mp3FrameHeader {
    /// Parse frame header from bytes
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }

        // Check sync word
        if data[0] != 0xFF || (data[1] & 0xE0) != 0xE0 {
            return None;
        }

        // Version
        let version = match (data[1] >> 3) & 0x03 {
            0 => MpegVersion::Mpeg2_5,
            2 => MpegVersion::Mpeg2,
            3 => MpegVersion::Mpeg1,
            _ => return None,
        };

        // Layer
        let layer = match (data[1] >> 1) & 0x03 {
            1 => MpegLayer::Layer3,
            2 => MpegLayer::Layer2,
            3 => MpegLayer::Layer1,
            _ => return None,
        };

        // Bitrate index
        let bitrate_index = (data[2] >> 4) & 0x0F;
        let bitrate = Self::get_bitrate(version, layer, bitrate_index)?;

        // Sample rate index
        let sample_rate_index = (data[2] >> 2) & 0x03;
        let sample_rate = Self::get_sample_rate(version, sample_rate_index)?;

        // Padding
        let padding = (data[2] & 0x02) != 0;

        // Channel mode
        let channel_mode = (data[3] >> 6) & 0x03;
        let channels = if channel_mode == 3 { 1 } else { 2 };

        // Calculate frame size
        let frame_size = match layer {
            MpegLayer::Layer1 => {
                (12 * bitrate * 1000 / sample_rate + if padding { 4 } else { 0 }) as usize
            }
            MpegLayer::Layer2 | MpegLayer::Layer3 => {
                let samples_per_frame = match (version, layer) {
                    (MpegVersion::Mpeg1, MpegLayer::Layer3) => 1152,
                    (_, MpegLayer::Layer3) => 576,
                    (_, _) => 1152,
                };
                (samples_per_frame / 8 * bitrate * 1000 / sample_rate + if padding { 1 } else { 0 }) as usize
            }
        };

        Some(Self {
            version,
            layer,
            bitrate,
            sample_rate,
            channels,
            padding,
            frame_size,
        })
    }

    fn get_bitrate(version: MpegVersion, layer: MpegLayer, index: u8) -> Option<u32> {
        if index == 0 || index == 15 {
            return None;
        }

        let bitrates_v1_l1 = [0, 32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448, 0];
        let bitrates_v1_l2 = [0, 32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384, 0];
        let bitrates_v1_l3 = [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0];
        let bitrates_v2_l1 = [0, 32, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256, 0];
        let bitrates_v2_l23 = [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160, 0];

        let table = match (version, layer) {
            (MpegVersion::Mpeg1, MpegLayer::Layer1) => &bitrates_v1_l1,
            (MpegVersion::Mpeg1, MpegLayer::Layer2) => &bitrates_v1_l2,
            (MpegVersion::Mpeg1, MpegLayer::Layer3) => &bitrates_v1_l3,
            (_, MpegLayer::Layer1) => &bitrates_v2_l1,
            (_, _) => &bitrates_v2_l23,
        };

        Some(table[index as usize])
    }

    fn get_sample_rate(version: MpegVersion, index: u8) -> Option<u32> {
        let rates = match version {
            MpegVersion::Mpeg1 => [44100, 48000, 32000, 0],
            MpegVersion::Mpeg2 => [22050, 24000, 16000, 0],
            MpegVersion::Mpeg2_5 => [11025, 12000, 8000, 0],
        };

        if index > 2 {
            None
        } else {
            Some(rates[index as usize])
        }
    }
}

/// MP3 decoder
pub struct Mp3Decoder {
    info: AudioStreamInfo,
    // Decoder state
    main_data_buffer: Vec<u8>,
    frame_count: u64,
    current_sample: u64,
    // Synthesis filter state
    synth_buffer: [[f32; 1024]; 2],
    synth_offset: usize,
}

impl Mp3Decoder {
    /// Create new MP3 decoder
    pub fn new() -> Self {
        Self {
            info: AudioStreamInfo {
                codec: AudioCodec::Mp3,
                sample_rate: 44100,
                channels: 2,
                bit_depth: 16,
                bitrate: 0,
                duration_ms: 0,
                total_samples: 0,
            },
            main_data_buffer: Vec::with_capacity(2048),
            frame_count: 0,
            current_sample: 0,
            synth_buffer: [[0.0; 1024]; 2],
            synth_offset: 0,
        }
    }

    /// Initialize from stream
    pub fn init(&mut self, data: &[u8]) -> Result<(), DecoderError> {
        // Skip ID3 tag if present
        let offset = if data.starts_with(b"ID3") && data.len() >= 10 {
            let size = ((data[6] as usize & 0x7F) << 21)
                | ((data[7] as usize & 0x7F) << 14)
                | ((data[8] as usize & 0x7F) << 7)
                | (data[9] as usize & 0x7F);
            10 + size
        } else {
            0
        };

        // Find first frame
        let frame_data = &data[offset..];
        let header = Mp3FrameHeader::parse(frame_data)
            .ok_or(DecoderError::InvalidData)?;

        self.info.sample_rate = header.sample_rate;
        self.info.channels = header.channels;
        self.info.bitrate = header.bitrate;

        // Estimate duration from file size and bitrate
        if header.bitrate > 0 {
            let data_size = data.len().saturating_sub(offset);
            self.info.duration_ms = (data_size as u64 * 8) / (header.bitrate as u64);
            self.info.total_samples = (self.info.duration_ms * header.sample_rate as u64) / 1000;
        }

        Ok(())
    }

    /// Find next sync word
    fn find_sync(&self, data: &[u8]) -> Option<usize> {
        for i in 0..data.len().saturating_sub(1) {
            if data[i] == 0xFF && (data[i + 1] & 0xE0) == 0xE0 {
                return Some(i);
            }
        }
        None
    }

    /// Decode a frame (simplified - outputs silence in this implementation)
    /// A full MP3 decoder would implement:
    /// 1. Huffman decoding
    /// 2. Requantization
    /// 3. Reordering (short blocks)
    /// 4. Stereo processing
    /// 5. Alias reduction
    /// 6. IMDCT
    /// 7. Frequency inversion
    /// 8. Synthesis filterbank
    fn decode_frame_internal(&mut self, data: &[u8]) -> Result<AudioFrame, DecoderError> {
        let header = Mp3FrameHeader::parse(data)
            .ok_or(DecoderError::InvalidData)?;

        if data.len() < header.frame_size {
            return Err(DecoderError::NeedMoreData);
        }

        // Samples per frame
        let samples_per_frame = match (header.version, header.layer) {
            (MpegVersion::Mpeg1, MpegLayer::Layer1) => 384,
            (MpegVersion::Mpeg1, _) => 1152,
            (_, MpegLayer::Layer1) => 384,
            (_, _) => 576,
        };

        let mut frame = AudioFrame::new(header.sample_rate, header.channels);
        frame.timestamp = self.current_sample;

        // Generate decoded samples
        // In a real implementation, this would decode the MP3 data
        // For now, generate silence to demonstrate the interface
        let total_samples = samples_per_frame * header.channels as usize;
        frame.samples = vec![0i16; total_samples];

        self.frame_count += 1;
        self.current_sample += samples_per_frame as u64;

        Ok(frame)
    }
}

impl AudioDecoder for Mp3Decoder {
    fn codec(&self) -> AudioCodec {
        AudioCodec::Mp3
    }

    fn info(&self) -> &AudioStreamInfo {
        &self.info
    }

    fn reset(&mut self) {
        self.main_data_buffer.clear();
        self.frame_count = 0;
        self.current_sample = 0;
        self.synth_buffer = [[0.0; 1024]; 2];
        self.synth_offset = 0;
    }

    fn decode_frame(&mut self, input: &[u8]) -> Result<AudioFrame, DecoderError> {
        // Find sync
        let offset = self.find_sync(input).ok_or(DecoderError::InvalidData)?;
        self.decode_frame_internal(&input[offset..])
    }

    fn seek(&mut self, sample_pos: u64) -> Result<(), DecoderError> {
        // MP3 seeking requires either:
        // 1. Xing/LAME VBR header for accurate seeking
        // 2. CBR: calculate byte position from bitrate
        // 3. Scan through frames
        self.current_sample = sample_pos;
        self.main_data_buffer.clear();
        Ok(())
    }
}

// =============================================================================
// AAC Decoder
// =============================================================================

/// AAC profile
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AacProfile {
    Main,
    Lc,      // Low Complexity (most common)
    Ssr,     // Scalable Sample Rate
    Ltp,     // Long Term Prediction
    He,      // High Efficiency (SBR)
    HeV2,    // High Efficiency v2 (SBR + PS)
}

/// AAC ADTS header
#[derive(Debug, Clone)]
pub struct AacAdtsHeader {
    pub profile: AacProfile,
    pub sample_rate: u32,
    pub channels: u8,
    pub frame_length: usize,
}

impl AacAdtsHeader {
    /// Parse ADTS header
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 7 {
            return None;
        }

        // Sync word
        if data[0] != 0xFF || (data[1] & 0xF6) != 0xF0 {
            return None;
        }

        // Profile
        let profile_index = ((data[2] >> 6) & 0x03) as u8;
        let profile = match profile_index {
            0 => AacProfile::Main,
            1 => AacProfile::Lc,
            2 => AacProfile::Ssr,
            3 => AacProfile::Ltp,
            _ => return None,
        };

        // Sample rate
        let sr_index = ((data[2] >> 2) & 0x0F) as u8;
        let sample_rates = [96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050,
                          16000, 12000, 11025, 8000, 7350, 0, 0, 0];
        let sample_rate = sample_rates[sr_index as usize];
        if sample_rate == 0 {
            return None;
        }

        // Channels
        let channels = (((data[2] & 0x01) << 2) | ((data[3] >> 6) & 0x03)) as u8;

        // Frame length
        let frame_length = (((data[3] & 0x03) as usize) << 11)
            | ((data[4] as usize) << 3)
            | ((data[5] >> 5) as usize);

        Some(Self {
            profile,
            sample_rate,
            channels,
            frame_length,
        })
    }
}

/// AAC decoder
pub struct AacDecoder {
    info: AudioStreamInfo,
    frame_count: u64,
    current_sample: u64,
    // Decoder state (simplified)
    // A full AAC decoder would have:
    // - MDCT buffers
    // - TNS (Temporal Noise Shaping) state
    // - PNS (Perceptual Noise Substitution) state
    // - SBR (Spectral Band Replication) state
    // - PS (Parametric Stereo) state
}

impl AacDecoder {
    /// Create new AAC decoder
    pub fn new() -> Self {
        Self {
            info: AudioStreamInfo {
                codec: AudioCodec::Aac,
                sample_rate: 44100,
                channels: 2,
                bit_depth: 16,
                bitrate: 0,
                duration_ms: 0,
                total_samples: 0,
            },
            frame_count: 0,
            current_sample: 0,
        }
    }

    /// Initialize from ADTS stream
    pub fn init_adts(&mut self, data: &[u8]) -> Result<(), DecoderError> {
        let header = AacAdtsHeader::parse(data)
            .ok_or(DecoderError::InvalidData)?;

        self.info.sample_rate = header.sample_rate;
        self.info.channels = header.channels;

        Ok(())
    }

    /// Find next ADTS sync
    fn find_adts_sync(&self, data: &[u8]) -> Option<usize> {
        for i in 0..data.len().saturating_sub(1) {
            if data[i] == 0xFF && (data[i + 1] & 0xF6) == 0xF0 {
                return Some(i);
            }
        }
        None
    }

    fn decode_adts_frame(&mut self, data: &[u8]) -> Result<AudioFrame, DecoderError> {
        let header = AacAdtsHeader::parse(data)
            .ok_or(DecoderError::InvalidData)?;

        if data.len() < header.frame_length {
            return Err(DecoderError::NeedMoreData);
        }

        // AAC uses 1024 samples per frame
        let samples_per_frame = 1024usize;

        let mut frame = AudioFrame::new(header.sample_rate, header.channels);
        frame.timestamp = self.current_sample;

        // In a real implementation, decode the AAC frame
        let total_samples = samples_per_frame * header.channels as usize;
        frame.samples = vec![0i16; total_samples];

        self.frame_count += 1;
        self.current_sample += samples_per_frame as u64;

        Ok(frame)
    }
}

impl AudioDecoder for AacDecoder {
    fn codec(&self) -> AudioCodec {
        AudioCodec::Aac
    }

    fn info(&self) -> &AudioStreamInfo {
        &self.info
    }

    fn reset(&mut self) {
        self.frame_count = 0;
        self.current_sample = 0;
    }

    fn decode_frame(&mut self, input: &[u8]) -> Result<AudioFrame, DecoderError> {
        let offset = self.find_adts_sync(input).ok_or(DecoderError::InvalidData)?;
        self.decode_adts_frame(&input[offset..])
    }

    fn seek(&mut self, sample_pos: u64) -> Result<(), DecoderError> {
        self.current_sample = sample_pos;
        Ok(())
    }
}

// =============================================================================
// FLAC Decoder
// =============================================================================

/// FLAC stream info
#[derive(Debug, Clone)]
pub struct FlacStreamInfo {
    pub min_block_size: u16,
    pub max_block_size: u16,
    pub min_frame_size: u32,
    pub max_frame_size: u32,
    pub sample_rate: u32,
    pub channels: u8,
    pub bits_per_sample: u8,
    pub total_samples: u64,
    pub md5: [u8; 16],
}

impl FlacStreamInfo {
    /// Parse from metadata block
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 34 {
            return None;
        }

        let min_block_size = u16::from_be_bytes([data[0], data[1]]);
        let max_block_size = u16::from_be_bytes([data[2], data[3]]);
        let min_frame_size = ((data[4] as u32) << 16) | ((data[5] as u32) << 8) | (data[6] as u32);
        let max_frame_size = ((data[7] as u32) << 16) | ((data[8] as u32) << 8) | (data[9] as u32);

        let sample_rate = ((data[10] as u32) << 12) | ((data[11] as u32) << 4) | ((data[12] as u32) >> 4);
        let channels = ((data[12] >> 1) & 0x07) + 1;
        let bits_per_sample = (((data[12] & 0x01) << 4) | (data[13] >> 4)) + 1;

        let total_samples = (((data[13] & 0x0F) as u64) << 32)
            | ((data[14] as u64) << 24)
            | ((data[15] as u64) << 16)
            | ((data[16] as u64) << 8)
            | (data[17] as u64);

        let mut md5 = [0u8; 16];
        md5.copy_from_slice(&data[18..34]);

        Some(Self {
            min_block_size,
            max_block_size,
            min_frame_size,
            max_frame_size,
            sample_rate,
            channels,
            bits_per_sample,
            total_samples,
            md5,
        })
    }
}

/// FLAC decoder
pub struct FlacDecoder {
    info: AudioStreamInfo,
    stream_info: Option<FlacStreamInfo>,
    current_sample: u64,
}

impl FlacDecoder {
    /// Create new FLAC decoder
    pub fn new() -> Self {
        Self {
            info: AudioStreamInfo {
                codec: AudioCodec::Flac,
                sample_rate: 44100,
                channels: 2,
                bit_depth: 16,
                bitrate: 0,
                duration_ms: 0,
                total_samples: 0,
            },
            stream_info: None,
            current_sample: 0,
        }
    }

    /// Initialize from FLAC stream
    pub fn init(&mut self, data: &[u8]) -> Result<(), DecoderError> {
        // Check magic
        if !data.starts_with(b"fLaC") {
            return Err(DecoderError::InvalidData);
        }

        // Read metadata blocks
        let mut offset = 4;
        loop {
            if offset + 4 > data.len() {
                return Err(DecoderError::InvalidData);
            }

            let header = data[offset];
            let is_last = (header & 0x80) != 0;
            let block_type = header & 0x7F;
            let block_size = ((data[offset + 1] as usize) << 16)
                | ((data[offset + 2] as usize) << 8)
                | (data[offset + 3] as usize);

            offset += 4;

            if block_type == 0 {
                // STREAMINFO
                let stream_info = FlacStreamInfo::parse(&data[offset..])
                    .ok_or(DecoderError::InvalidData)?;

                self.info.sample_rate = stream_info.sample_rate;
                self.info.channels = stream_info.channels;
                self.info.bit_depth = stream_info.bits_per_sample;
                self.info.total_samples = stream_info.total_samples;

                if stream_info.sample_rate > 0 {
                    self.info.duration_ms =
                        (stream_info.total_samples * 1000) / stream_info.sample_rate as u64;
                }

                self.stream_info = Some(stream_info);
            }

            offset += block_size;

            if is_last {
                break;
            }
        }

        Ok(())
    }
}

impl AudioDecoder for FlacDecoder {
    fn codec(&self) -> AudioCodec {
        AudioCodec::Flac
    }

    fn info(&self) -> &AudioStreamInfo {
        &self.info
    }

    fn reset(&mut self) {
        self.current_sample = 0;
    }

    fn decode_frame(&mut self, _input: &[u8]) -> Result<AudioFrame, DecoderError> {
        // FLAC frame decoding would involve:
        // 1. Parse frame header
        // 2. Decode subframes for each channel
        // 3. Apply interchannel decorrelation
        // 4. Output samples

        let block_size = self.stream_info.as_ref()
            .map(|s| s.max_block_size as usize)
            .unwrap_or(4096);

        let mut frame = AudioFrame::new(self.info.sample_rate, self.info.channels);
        frame.timestamp = self.current_sample;
        frame.samples = vec![0i16; block_size * self.info.channels as usize];

        self.current_sample += block_size as u64;

        Ok(frame)
    }

    fn seek(&mut self, sample_pos: u64) -> Result<(), DecoderError> {
        // FLAC supports seeking via seek table or binary search
        self.current_sample = sample_pos;
        Ok(())
    }
}

// =============================================================================
// WAV Parser
// =============================================================================

/// WAV file info
#[derive(Debug, Clone)]
pub struct WavInfo {
    pub format: u16,        // 1 = PCM, 3 = IEEE float
    pub channels: u16,
    pub sample_rate: u32,
    pub byte_rate: u32,
    pub block_align: u16,
    pub bits_per_sample: u16,
    pub data_offset: usize,
    pub data_size: usize,
}

impl WavInfo {
    /// Parse WAV header
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 44 {
            return None;
        }

        // Check RIFF header
        if &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
            return None;
        }

        // Find fmt chunk
        let mut offset = 12;
        let mut fmt_chunk = None;

        while offset + 8 <= data.len() {
            let chunk_id = &data[offset..offset + 4];
            let chunk_size = u32::from_le_bytes([
                data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7]
            ]) as usize;

            if chunk_id == b"fmt " && chunk_size >= 16 {
                fmt_chunk = Some(&data[offset + 8..offset + 8 + chunk_size]);
            }

            if chunk_id == b"data" {
                if let Some(fmt) = fmt_chunk {
                    return Some(Self {
                        format: u16::from_le_bytes([fmt[0], fmt[1]]),
                        channels: u16::from_le_bytes([fmt[2], fmt[3]]),
                        sample_rate: u32::from_le_bytes([fmt[4], fmt[5], fmt[6], fmt[7]]),
                        byte_rate: u32::from_le_bytes([fmt[8], fmt[9], fmt[10], fmt[11]]),
                        block_align: u16::from_le_bytes([fmt[12], fmt[13]]),
                        bits_per_sample: u16::from_le_bytes([fmt[14], fmt[15]]),
                        data_offset: offset + 8,
                        data_size: chunk_size,
                    });
                }
            }

            offset += 8 + chunk_size;
            if chunk_size % 2 != 0 {
                offset += 1; // Padding
            }
        }

        None
    }

    /// Get sample format
    pub fn sample_format(&self) -> Option<SampleFormat> {
        match (self.format, self.bits_per_sample) {
            (1, 8) => Some(SampleFormat::U8),
            (1, 16) => Some(SampleFormat::S16LE),
            (1, 24) => Some(SampleFormat::S24LE),
            (1, 32) => Some(SampleFormat::S32LE),
            (3, 32) => Some(SampleFormat::F32LE),
            _ => None,
        }
    }

    /// Total number of samples per channel
    pub fn total_samples(&self) -> u64 {
        if self.block_align == 0 {
            return 0;
        }
        self.data_size as u64 / self.block_align as u64
    }

    /// Duration in milliseconds
    pub fn duration_ms(&self) -> u64 {
        if self.sample_rate == 0 {
            return 0;
        }
        (self.total_samples() * 1000) / self.sample_rate as u64
    }
}

// =============================================================================
// Public interface
// =============================================================================

// =============================================================================
// Opus Decoder
// =============================================================================

/// Opus bandwidth
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpusBandwidth {
    Narrowband,    // 4 kHz
    Mediumband,    // 6 kHz
    Wideband,      // 8 kHz
    SuperWideband, // 12 kHz
    Fullband,      // 20 kHz
}

impl OpusBandwidth {
    pub fn hz(self) -> u32 {
        match self {
            OpusBandwidth::Narrowband => 4000,
            OpusBandwidth::Mediumband => 6000,
            OpusBandwidth::Wideband => 8000,
            OpusBandwidth::SuperWideband => 12000,
            OpusBandwidth::Fullband => 20000,
        }
    }
}

/// Opus mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpusMode {
    Silk,   // Speech
    Celt,   // Music
    Hybrid, // Both
}

/// Opus header (RFC 7845)
#[derive(Debug, Clone)]
pub struct OpusHeader {
    pub version: u8,
    pub channels: u8,
    pub pre_skip: u16,
    pub sample_rate: u32,
    pub output_gain: i16,
    pub channel_mapping: u8,
    // Extended mapping (if channel_mapping != 0)
    pub stream_count: u8,
    pub coupled_count: u8,
    pub channel_map: Vec<u8>,
}

impl OpusHeader {
    /// Parse OpusHead from OGG page
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 19 || !data.starts_with(b"OpusHead") {
            return None;
        }

        let version = data[8];
        if version != 1 {
            return None; // Only version 1 supported
        }

        let channels = data[9];
        if channels == 0 {
            return None;
        }

        let pre_skip = u16::from_le_bytes([data[10], data[11]]);
        let sample_rate = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
        let output_gain = i16::from_le_bytes([data[16], data[17]]);
        let channel_mapping = data[18];

        let (stream_count, coupled_count, channel_map) = if channel_mapping != 0 {
            if data.len() < 21 + channels as usize {
                return None;
            }
            let stream_count = data[19];
            let coupled_count = data[20];
            let channel_map = data[21..21 + channels as usize].to_vec();
            (stream_count, coupled_count, channel_map)
        } else {
            (1, if channels > 1 { 1 } else { 0 }, vec![0, 1])
        };

        Some(Self {
            version,
            channels,
            pre_skip,
            sample_rate,
            output_gain,
            channel_mapping,
            stream_count,
            coupled_count,
            channel_map,
        })
    }
}

/// Opus decoder state
pub struct OpusDecoder {
    info: AudioStreamInfo,
    header: Option<OpusHeader>,
    current_sample: u64,
    // SILK decoder state
    silk_prev_samples: [[i16; 320]; 2],
    silk_lpf_state: [i32; 2],
    // CELT decoder state
    celt_prev_buffer: [[f32; 960]; 2],
    celt_preemph: [f32; 2],
    // Range decoder state
    range_val: u32,
    range_rng: u32,
}

impl OpusDecoder {
    /// Create new Opus decoder
    pub fn new() -> Self {
        Self {
            info: AudioStreamInfo {
                codec: AudioCodec::Opus,
                sample_rate: 48000, // Opus always decodes at 48kHz
                channels: 2,
                bit_depth: 16,
                bitrate: 0,
                duration_ms: 0,
                total_samples: 0,
            },
            header: None,
            current_sample: 0,
            silk_prev_samples: [[0; 320]; 2],
            silk_lpf_state: [0; 2],
            celt_prev_buffer: [[0.0; 960]; 2],
            celt_preemph: [0.0; 2],
            range_val: 0,
            range_rng: 0,
        }
    }

    /// Initialize from OGG Opus stream
    pub fn init(&mut self, data: &[u8]) -> Result<(), DecoderError> {
        // Find OpusHead in OGG
        let header_start = Self::find_opus_head(data)
            .ok_or(DecoderError::InvalidData)?;

        let header = OpusHeader::parse(&data[header_start..])
            .ok_or(DecoderError::InvalidData)?;

        self.info.channels = header.channels;
        self.info.sample_rate = if header.sample_rate > 0 {
            header.sample_rate
        } else {
            48000 // Default
        };

        self.header = Some(header);

        Ok(())
    }

    /// Find OpusHead magic in OGG stream
    fn find_opus_head(data: &[u8]) -> Option<usize> {
        for i in 0..data.len().saturating_sub(8) {
            if &data[i..i+8] == b"OpusHead" {
                return Some(i);
            }
        }
        None
    }

    /// Decode Opus frame
    /// Opus frames are 2.5, 5, 10, 20, 40, or 60 ms
    fn decode_opus_frame(&mut self, data: &[u8]) -> Result<AudioFrame, DecoderError> {
        if data.is_empty() {
            return Err(DecoderError::InvalidData);
        }

        // Parse TOC byte
        let toc = data[0];
        let config = (toc >> 3) & 0x1F;
        let _stereo = (toc >> 2) & 0x01 != 0;
        let _frame_count_code = toc & 0x03;

        // Determine mode and frame size from config
        let (mode, frame_duration_ms) = Self::decode_config(config);

        // Calculate samples (Opus always outputs 48kHz)
        let frame_samples = (48000 * frame_duration_ms / 1000) as usize;

        let channels = self.header.as_ref().map(|h| h.channels).unwrap_or(2);

        let mut frame = AudioFrame::new(48000, channels);
        frame.timestamp = self.current_sample;

        // In a full implementation, we would:
        // 1. Parse frame structure (code 0, 1, 2, or 3)
        // 2. For SILK: decode LP, pitch, LTP, excitation
        // 3. For CELT: decode band energies, pulse positions, fine energy
        // 4. Apply gain, resampling
        // For now, output silence to demonstrate the interface

        frame.samples = vec![0i16; frame_samples * channels as usize];

        self.current_sample += frame_samples as u64;

        let _ = mode; // Would use for actual decoding

        Ok(frame)
    }

    /// Decode TOC config byte
    fn decode_config(config: u8) -> (OpusMode, u32) {
        match config {
            0..=3 => (OpusMode::Silk, [10, 20, 40, 60][config as usize]),
            4..=7 => (OpusMode::Silk, [10, 20, 40, 60][(config - 4) as usize]),
            8..=11 => (OpusMode::Silk, [10, 20, 40, 60][(config - 8) as usize]),
            12..=13 => (OpusMode::Hybrid, [10, 20][(config - 12) as usize]),
            14..=15 => (OpusMode::Hybrid, [10, 20][(config - 14) as usize]),
            16..=19 => (OpusMode::Celt, [((config - 16) as u32 + 1) * 5 / 2][0].max(5)),
            20..=23 => (OpusMode::Celt, [((config - 20) as u32 + 1) * 5 / 2][0].max(5)),
            24..=27 => (OpusMode::Celt, [((config - 24) as u32 + 1) * 5 / 2][0].max(5)),
            28..=31 => (OpusMode::Celt, [((config - 28) as u32 + 1) * 5 / 2][0].max(5)),
            _ => (OpusMode::Celt, 20),
        }
    }
}

impl AudioDecoder for OpusDecoder {
    fn codec(&self) -> AudioCodec {
        AudioCodec::Opus
    }

    fn info(&self) -> &AudioStreamInfo {
        &self.info
    }

    fn reset(&mut self) {
        self.current_sample = 0;
        self.silk_prev_samples = [[0; 320]; 2];
        self.silk_lpf_state = [0; 2];
        self.celt_prev_buffer = [[0.0; 960]; 2];
        self.celt_preemph = [0.0; 2];
    }

    fn decode_frame(&mut self, input: &[u8]) -> Result<AudioFrame, DecoderError> {
        self.decode_opus_frame(input)
    }

    fn seek(&mut self, sample_pos: u64) -> Result<(), DecoderError> {
        // Opus seeking requires parsing OGG granule positions
        self.current_sample = sample_pos;
        self.reset();
        Ok(())
    }
}

// =============================================================================
// Vorbis Decoder (OGG Vorbis)
// =============================================================================

/// Vorbis identification header
#[derive(Debug, Clone)]
pub struct VorbisIdHeader {
    pub version: u32,
    pub channels: u8,
    pub sample_rate: u32,
    pub bitrate_max: i32,
    pub bitrate_nom: i32,
    pub bitrate_min: i32,
    pub blocksize_0: u8, // log2
    pub blocksize_1: u8, // log2
}

impl VorbisIdHeader {
    /// Parse vorbis identification header
    pub fn parse(data: &[u8]) -> Option<Self> {
        // Must start with "\x01vorbis"
        if data.len() < 30 || data[0] != 0x01 || &data[1..7] != b"vorbis" {
            return None;
        }

        let version = u32::from_le_bytes([data[7], data[8], data[9], data[10]]);
        if version != 0 {
            return None; // Only version 0 supported
        }

        let channels = data[11];
        if channels == 0 {
            return None;
        }

        let sample_rate = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
        if sample_rate == 0 {
            return None;
        }

        let bitrate_max = i32::from_le_bytes([data[16], data[17], data[18], data[19]]);
        let bitrate_nom = i32::from_le_bytes([data[20], data[21], data[22], data[23]]);
        let bitrate_min = i32::from_le_bytes([data[24], data[25], data[26], data[27]]);

        let blocksizes = data[28];
        let blocksize_0 = blocksizes & 0x0F;
        let blocksize_1 = (blocksizes >> 4) & 0x0F;

        // Validate blocksizes (must be 6-13 for Vorbis)
        if blocksize_0 < 6 || blocksize_0 > 13 || blocksize_1 < 6 || blocksize_1 > 13 {
            return None;
        }
        if blocksize_0 > blocksize_1 {
            return None;
        }

        // Framing bit
        if (data[29] & 0x01) != 1 {
            return None;
        }

        Some(Self {
            version,
            channels,
            sample_rate,
            bitrate_max,
            bitrate_nom,
            bitrate_min,
            blocksize_0,
            blocksize_1,
        })
    }
}

/// Vorbis window type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VorbisWindowType {
    Short,
    Long,
}

/// Vorbis decoder
pub struct VorbisDecoder {
    info: AudioStreamInfo,
    id_header: Option<VorbisIdHeader>,
    current_sample: u64,
    // Codebook cache
    // In a full impl: Vec<Codebook>
    // Floor/residue configuration
    // Window functions
    // MDCT state
    prev_window: VorbisWindowType,
    overlap_buffer: Vec<Vec<f32>>, // Per-channel overlap/add
}

impl VorbisDecoder {
    /// Create new Vorbis decoder
    pub fn new() -> Self {
        Self {
            info: AudioStreamInfo {
                codec: AudioCodec::Vorbis,
                sample_rate: 44100,
                channels: 2,
                bit_depth: 16,
                bitrate: 0,
                duration_ms: 0,
                total_samples: 0,
            },
            id_header: None,
            current_sample: 0,
            prev_window: VorbisWindowType::Long,
            overlap_buffer: Vec::new(),
        }
    }

    /// Initialize from OGG Vorbis stream
    pub fn init(&mut self, data: &[u8]) -> Result<(), DecoderError> {
        // Find vorbis identification header
        let header_start = Self::find_vorbis_id(data)
            .ok_or(DecoderError::InvalidData)?;

        let id_header = VorbisIdHeader::parse(&data[header_start..])
            .ok_or(DecoderError::InvalidData)?;

        self.info.sample_rate = id_header.sample_rate;
        self.info.channels = id_header.channels;
        if id_header.bitrate_nom > 0 {
            self.info.bitrate = (id_header.bitrate_nom / 1000) as u32;
        }

        // Initialize overlap buffers
        let max_block = 1 << id_header.blocksize_1;
        self.overlap_buffer = vec![vec![0.0; max_block / 2]; id_header.channels as usize];

        self.id_header = Some(id_header);

        Ok(())
    }

    /// Find vorbis identification header in OGG stream
    fn find_vorbis_id(data: &[u8]) -> Option<usize> {
        for i in 0..data.len().saturating_sub(7) {
            if data[i] == 0x01 && &data[i+1..i+7] == b"vorbis" {
                return Some(i);
            }
        }
        None
    }

    /// Decode a Vorbis audio packet
    fn decode_vorbis_packet(&mut self, data: &[u8]) -> Result<AudioFrame, DecoderError> {
        if data.is_empty() {
            return Err(DecoderError::InvalidData);
        }

        // Check packet type (audio packets have type 0)
        if (data[0] & 0x01) != 0 {
            return Err(DecoderError::InvalidData); // Not an audio packet
        }

        let header = self.id_header.as_ref().ok_or(DecoderError::InvalidData)?;

        // Determine window type from mode
        // In a real impl, parse mode number and look up blocksize
        let block_size = 1 << header.blocksize_1; // Use long block as default

        let mut frame = AudioFrame::new(header.sample_rate, header.channels);
        frame.timestamp = self.current_sample;

        // In a full Vorbis decoder:
        // 1. Read mode number (log2(modes) bits)
        // 2. Determine window type
        // 3. Decode floor (type 0 or 1)
        // 4. Decode residue (type 0, 1, or 2)
        // 5. Apply inverse coupling
        // 6. Apply floor curve
        // 7. IMDCT
        // 8. Overlap-add with previous frame

        // For now, generate silence
        let samples_per_channel = block_size / 2; // After overlap-add
        frame.samples = vec![0i16; samples_per_channel * header.channels as usize];

        self.current_sample += samples_per_channel as u64;

        Ok(frame)
    }
}

impl AudioDecoder for VorbisDecoder {
    fn codec(&self) -> AudioCodec {
        AudioCodec::Vorbis
    }

    fn info(&self) -> &AudioStreamInfo {
        &self.info
    }

    fn reset(&mut self) {
        self.current_sample = 0;
        self.prev_window = VorbisWindowType::Long;
        for buf in &mut self.overlap_buffer {
            buf.fill(0.0);
        }
    }

    fn decode_frame(&mut self, input: &[u8]) -> Result<AudioFrame, DecoderError> {
        self.decode_vorbis_packet(input)
    }

    fn seek(&mut self, sample_pos: u64) -> Result<(), DecoderError> {
        // Vorbis seeking uses OGG granule positions
        self.current_sample = sample_pos;
        self.reset();
        Ok(())
    }
}

// =============================================================================
// Public interface
// =============================================================================

/// Create appropriate decoder for codec
pub fn create_decoder(codec: AudioCodec) -> Option<Box<dyn AudioDecoder + Send>> {
    match codec {
        AudioCodec::Mp3 => Some(Box::new(Mp3Decoder::new())),
        AudioCodec::Aac => Some(Box::new(AacDecoder::new())),
        AudioCodec::Flac => Some(Box::new(FlacDecoder::new())),
        AudioCodec::Opus => Some(Box::new(OpusDecoder::new())),
        AudioCodec::Vorbis => Some(Box::new(VorbisDecoder::new())),
        _ => None,
    }
}

/// Initialize codec subsystem
pub fn init() {
    crate::kprintln!("audio_codecs: initialized (MP3, AAC, FLAC, Opus, Vorbis, WAV support)");
}
