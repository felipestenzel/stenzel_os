// SPDX-License-Identifier: MIT
// VDPAU (Video Decode and Presentation API for Unix) driver for Stenzel OS
// Originally created by NVIDIA, now used for GPU video acceleration

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::string::ToString;
use alloc::collections::BTreeMap;
use crate::sync::TicketSpinlock;

/// VDPAU version
pub const VDPAU_VERSION: u32 = 1;

/// Status codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum VdpStatus {
    Ok = 0,
    NoImplementation = 1,
    DisplayPreempted = 2,
    InvalidHandle = 3,
    InvalidPointer = 4,
    InvalidChromaType = 5,
    InvalidYCbCrFormat = 6,
    InvalidRgbaFormat = 7,
    InvalidIndexedFormat = 8,
    InvalidColorStandard = 9,
    InvalidColorTableFormat = 10,
    InvalidBlendFactor = 11,
    InvalidBlendEquation = 12,
    InvalidFlag = 13,
    InvalidDecoderProfile = 14,
    InvalidVideoMixerFeature = 15,
    InvalidVideoMixerParameter = 16,
    InvalidVideoMixerAttribute = 17,
    InvalidVideoMixerPictureStructure = 18,
    InvalidFuncId = 19,
    InvalidSize = 20,
    InvalidValue = 21,
    InvalidStruct = 22,
    ResourcesBusy = 23,
    Resources = 24,
    InvalidHandle2 = 25,
    InvalidDecoderTarget = 26,
    Error = -1,
}

impl VdpStatus {
    pub fn is_ok(self) -> bool {
        self == VdpStatus::Ok
    }

    pub fn name(self) -> &'static str {
        match self {
            VdpStatus::Ok => "OK",
            VdpStatus::NoImplementation => "No Implementation",
            VdpStatus::DisplayPreempted => "Display Preempted",
            VdpStatus::InvalidHandle => "Invalid Handle",
            VdpStatus::InvalidPointer => "Invalid Pointer",
            VdpStatus::InvalidChromaType => "Invalid Chroma Type",
            VdpStatus::InvalidYCbCrFormat => "Invalid YCbCr Format",
            VdpStatus::InvalidRgbaFormat => "Invalid RGBA Format",
            VdpStatus::InvalidIndexedFormat => "Invalid Indexed Format",
            VdpStatus::InvalidColorStandard => "Invalid Color Standard",
            VdpStatus::InvalidColorTableFormat => "Invalid Color Table Format",
            VdpStatus::InvalidBlendFactor => "Invalid Blend Factor",
            VdpStatus::InvalidBlendEquation => "Invalid Blend Equation",
            VdpStatus::InvalidFlag => "Invalid Flag",
            VdpStatus::InvalidDecoderProfile => "Invalid Decoder Profile",
            VdpStatus::InvalidVideoMixerFeature => "Invalid Video Mixer Feature",
            VdpStatus::InvalidVideoMixerParameter => "Invalid Video Mixer Parameter",
            VdpStatus::InvalidVideoMixerAttribute => "Invalid Video Mixer Attribute",
            VdpStatus::InvalidVideoMixerPictureStructure => "Invalid Picture Structure",
            VdpStatus::InvalidFuncId => "Invalid Function ID",
            VdpStatus::InvalidSize => "Invalid Size",
            VdpStatus::InvalidValue => "Invalid Value",
            VdpStatus::InvalidStruct => "Invalid Struct",
            VdpStatus::ResourcesBusy => "Resources Busy",
            VdpStatus::Resources => "Insufficient Resources",
            VdpStatus::InvalidHandle2 => "Invalid Handle",
            VdpStatus::InvalidDecoderTarget => "Invalid Decoder Target",
            VdpStatus::Error => "General Error",
        }
    }
}

/// Chroma types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum VdpChromaType {
    Type420 = 0,
    Type422 = 1,
    Type444 = 2,
    Type420_16 = 3,
    Type422_16 = 4,
    Type444_16 = 5,
}

impl VdpChromaType {
    pub fn bits_per_component(self) -> u32 {
        match self {
            VdpChromaType::Type420_16 |
            VdpChromaType::Type422_16 |
            VdpChromaType::Type444_16 => 16,
            _ => 8,
        }
    }

    pub fn subsampling(self) -> &'static str {
        match self {
            VdpChromaType::Type420 | VdpChromaType::Type420_16 => "4:2:0",
            VdpChromaType::Type422 | VdpChromaType::Type422_16 => "4:2:2",
            VdpChromaType::Type444 | VdpChromaType::Type444_16 => "4:4:4",
        }
    }
}

/// YCbCr formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum VdpYCbCrFormat {
    Nv12 = 0,          // NV12: Y plane + interleaved UV
    Yv12 = 1,          // YV12: Y plane + V plane + U plane
    Nv12_16 = 2,       // 16-bit NV12
    P010 = 3,          // 10-bit packed
    P016 = 4,          // 16-bit packed
    Y8u8v8a8 = 5,      // Packed YUVA
    V8u8y8a8 = 6,      // Packed VUYA
}

/// RGBA formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum VdpRgbaFormat {
    B8g8r8a8 = 0,
    R8g8b8a8 = 1,
    R10g10b10a2 = 2,
    B10g10r10a2 = 3,
    A8 = 4,
}

/// Indexed formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum VdpIndexedFormat {
    A4i4 = 0,
    I4a4 = 1,
    A8i8 = 2,
    I8a8 = 3,
}

/// Color standards
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum VdpColorStandard {
    Itur_bt_601 = 0,    // SD content
    Itur_bt_709 = 1,    // HD content
    Smpte_240m = 2,     // Legacy HD
    Itur_bt_2020 = 3,   // UHD/4K content
}

/// Decoder profiles
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum VdpDecoderProfile {
    // MPEG-1
    Mpeg1 = 0,

    // MPEG-2
    Mpeg2Simple = 1,
    Mpeg2Main = 2,

    // MPEG-4 Part 2
    Mpeg4PartSimple = 3,
    Mpeg4PartMain = 4,
    Mpeg4PartAdvancedSimple = 5,

    // H.264/AVC
    H264Baseline = 6,
    H264Main = 7,
    H264High = 8,
    H264ConstrainedBaseline = 20,
    H264Extended = 21,
    H264ProgressiveHigh = 22,
    H264ConstrainedHigh = 23,
    H264High444Predictive = 24,

    // VC-1
    Vc1Simple = 9,
    Vc1Main = 10,
    Vc1Advanced = 11,

    // Divx (MPEG-4)
    Divx4Qmobile = 12,
    Divx4Mobile = 13,
    Divx4HomeTheatre = 14,
    Divx4HdTv = 15,
    Divx5Qmobile = 16,
    Divx5Mobile = 17,
    Divx5HomeTheatre = 18,
    Divx5HdTv = 19,

    // HEVC/H.265
    HevcMain = 100,
    HevcMain10 = 101,
    HevcMain12 = 102,
    HevcMainStill = 103,
    HevcMain444 = 104,
    HevcMain444_10 = 105,
    HevcMain444_12 = 106,

    // VP9
    Vp9Profile0 = 110,
    Vp9Profile1 = 111,
    Vp9Profile2 = 112,
    Vp9Profile3 = 113,

    // AV1
    Av1Main = 120,
    Av1High = 121,
    Av1Professional = 122,
}

impl VdpDecoderProfile {
    pub fn name(self) -> &'static str {
        match self {
            VdpDecoderProfile::Mpeg1 => "MPEG-1",
            VdpDecoderProfile::Mpeg2Simple => "MPEG-2 Simple",
            VdpDecoderProfile::Mpeg2Main => "MPEG-2 Main",
            VdpDecoderProfile::Mpeg4PartSimple => "MPEG-4 Part 2 Simple",
            VdpDecoderProfile::Mpeg4PartMain => "MPEG-4 Part 2 Main",
            VdpDecoderProfile::Mpeg4PartAdvancedSimple => "MPEG-4 Part 2 ASP",
            VdpDecoderProfile::H264Baseline => "H.264 Baseline",
            VdpDecoderProfile::H264Main => "H.264 Main",
            VdpDecoderProfile::H264High => "H.264 High",
            VdpDecoderProfile::H264ConstrainedBaseline => "H.264 Constrained Baseline",
            VdpDecoderProfile::H264Extended => "H.264 Extended",
            VdpDecoderProfile::H264ProgressiveHigh => "H.264 Progressive High",
            VdpDecoderProfile::H264ConstrainedHigh => "H.264 Constrained High",
            VdpDecoderProfile::H264High444Predictive => "H.264 High 4:4:4",
            VdpDecoderProfile::Vc1Simple => "VC-1 Simple",
            VdpDecoderProfile::Vc1Main => "VC-1 Main",
            VdpDecoderProfile::Vc1Advanced => "VC-1 Advanced",
            VdpDecoderProfile::Divx4Qmobile => "DivX4 QMobile",
            VdpDecoderProfile::Divx4Mobile => "DivX4 Mobile",
            VdpDecoderProfile::Divx4HomeTheatre => "DivX4 Home Theatre",
            VdpDecoderProfile::Divx4HdTv => "DivX4 HD-TV",
            VdpDecoderProfile::Divx5Qmobile => "DivX5 QMobile",
            VdpDecoderProfile::Divx5Mobile => "DivX5 Mobile",
            VdpDecoderProfile::Divx5HomeTheatre => "DivX5 Home Theatre",
            VdpDecoderProfile::Divx5HdTv => "DivX5 HD-TV",
            VdpDecoderProfile::HevcMain => "HEVC Main",
            VdpDecoderProfile::HevcMain10 => "HEVC Main 10",
            VdpDecoderProfile::HevcMain12 => "HEVC Main 12",
            VdpDecoderProfile::HevcMainStill => "HEVC Main Still",
            VdpDecoderProfile::HevcMain444 => "HEVC Main 4:4:4",
            VdpDecoderProfile::HevcMain444_10 => "HEVC Main 4:4:4 10",
            VdpDecoderProfile::HevcMain444_12 => "HEVC Main 4:4:4 12",
            VdpDecoderProfile::Vp9Profile0 => "VP9 Profile 0",
            VdpDecoderProfile::Vp9Profile1 => "VP9 Profile 1",
            VdpDecoderProfile::Vp9Profile2 => "VP9 Profile 2",
            VdpDecoderProfile::Vp9Profile3 => "VP9 Profile 3",
            VdpDecoderProfile::Av1Main => "AV1 Main",
            VdpDecoderProfile::Av1High => "AV1 High",
            VdpDecoderProfile::Av1Professional => "AV1 Professional",
        }
    }
}

/// Video mixer features
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum VdpVideoMixerFeature {
    DeinterlaceTemporal = 0,
    DeinterlaceTemporalSpatial = 1,
    InverseTeveticine = 2,
    NoiseReduction = 3,
    Sharpness = 4,
    Luma = 5,
    HighQualityScaling = 6,
}

/// Video mixer parameters
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum VdpVideoMixerParameter {
    VideoSurfaceWidth = 0,
    VideoSurfaceHeight = 1,
    ChromaType = 2,
    Layers = 3,
}

/// Video mixer attributes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum VdpVideoMixerAttribute {
    BackgroundColor = 0,
    CscMatrix = 1,
    NoiseReductionLevel = 2,
    SharpnessLevel = 3,
    LumaKeyMinLuma = 4,
    LumaKeyMaxLuma = 5,
    SkipChromaDeinterlace = 6,
}

/// Picture structure
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum VdpVideoMixerPictureStructure {
    TopField = 1,
    BottomField = 2,
    Frame = 3,
}

/// Blend factors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum VdpOutputSurfaceRenderBlendFactor {
    Zero = 0,
    One = 1,
    SrcColor = 2,
    OneMinusSrcColor = 3,
    SrcAlpha = 4,
    OneMinusSrcAlpha = 5,
    DstAlpha = 6,
    OneMinusDstAlpha = 7,
    DstColor = 8,
    OneMinusDstColor = 9,
    SrcAlphaSaturate = 10,
    ConstantColor = 11,
    OneMinusConstantColor = 12,
    ConstantAlpha = 13,
    OneMinusConstantAlpha = 14,
}

/// Blend equation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum VdpOutputSurfaceRenderBlendEquation {
    Add = 0,
    Subtract = 1,
    ReverseSubtract = 2,
    Min = 3,
    Max = 4,
}

/// Output surface render rotate modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum VdpOutputSurfaceRenderRotate {
    Rotate0 = 0,
    Rotate90 = 1,
    Rotate180 = 2,
    Rotate270 = 3,
}

/// Decoder capability
#[derive(Debug, Clone)]
pub struct DecoderCapability {
    pub profile: VdpDecoderProfile,
    pub is_supported: bool,
    pub max_level: u32,
    pub max_macroblocks: u32,
    pub max_width: u32,
    pub max_height: u32,
}

/// Video surface
#[derive(Debug, Clone)]
pub struct VdpVideoSurface {
    pub id: u32,
    pub chroma_type: VdpChromaType,
    pub width: u32,
    pub height: u32,
    pub buffer: u64,  // GPU buffer address
}

/// Output surface
#[derive(Debug, Clone)]
pub struct VdpOutputSurface {
    pub id: u32,
    pub rgba_format: VdpRgbaFormat,
    pub width: u32,
    pub height: u32,
    pub buffer: u64,
}

/// Bitmap surface (for OSD/subtitles)
#[derive(Debug, Clone)]
pub struct VdpBitmapSurface {
    pub id: u32,
    pub rgba_format: VdpRgbaFormat,
    pub width: u32,
    pub height: u32,
    pub frequently_accessed: bool,
    pub buffer: u64,
}

/// Decoder
#[derive(Debug, Clone)]
pub struct VdpDecoder {
    pub id: u32,
    pub profile: VdpDecoderProfile,
    pub width: u32,
    pub height: u32,
    pub max_references: u32,
}

/// Video mixer
#[derive(Debug, Clone)]
pub struct VdpVideoMixer {
    pub id: u32,
    pub features: Vec<VdpVideoMixerFeature>,
    pub video_width: u32,
    pub video_height: u32,
    pub chroma_type: VdpChromaType,
    pub layers: u32,

    // CSC matrix for color conversion
    pub csc_matrix: [[f32; 4]; 3],

    // Processing settings
    pub noise_reduction_level: f32,
    pub sharpness_level: f32,
    pub luma_key_min: f32,
    pub luma_key_max: f32,
    pub skip_chroma_deinterlace: bool,
    pub background_color: [f32; 4],
}

/// Presentation queue target
#[derive(Debug, Clone)]
pub struct VdpPresentationQueueTarget {
    pub id: u32,
    pub drawable: u64,  // X11 Drawable or native window handle
}

/// Presentation queue
#[derive(Debug, Clone)]
pub struct VdpPresentationQueue {
    pub id: u32,
    pub target_id: u32,
    pub background_color: [f32; 4],
}

/// GPU vendor
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VdpVendor {
    Nvidia,
    Mesa,  // Mesa VA-API to VDPAU wrapper
    Unknown,
}

/// VDPAU device state
pub struct VdpDevice {
    pub vendor: VdpVendor,
    pub api_version: u32,
    pub info_string: String,

    // GPU information
    pub mmio_base: u64,
    pub device_id: u16,

    // Capabilities
    pub decoder_caps: Vec<DecoderCapability>,
    pub max_video_surface_width: u32,
    pub max_video_surface_height: u32,
    pub max_output_surface_width: u32,
    pub max_output_surface_height: u32,

    // Resources
    next_video_surface_id: u32,
    video_surfaces: BTreeMap<u32, VdpVideoSurface>,
    next_output_surface_id: u32,
    output_surfaces: BTreeMap<u32, VdpOutputSurface>,
    next_bitmap_surface_id: u32,
    bitmap_surfaces: BTreeMap<u32, VdpBitmapSurface>,
    next_decoder_id: u32,
    decoders: BTreeMap<u32, VdpDecoder>,
    next_video_mixer_id: u32,
    video_mixers: BTreeMap<u32, VdpVideoMixer>,
    next_pq_target_id: u32,
    pq_targets: BTreeMap<u32, VdpPresentationQueueTarget>,
    next_pq_id: u32,
    pqs: BTreeMap<u32, VdpPresentationQueue>,

    initialized: bool,
}

impl VdpDevice {
    pub const fn new() -> Self {
        Self {
            vendor: VdpVendor::Unknown,
            api_version: VDPAU_VERSION,
            info_string: String::new(),
            mmio_base: 0,
            device_id: 0,
            decoder_caps: Vec::new(),
            max_video_surface_width: 0,
            max_video_surface_height: 0,
            max_output_surface_width: 0,
            max_output_surface_height: 0,
            next_video_surface_id: 1,
            video_surfaces: BTreeMap::new(),
            next_output_surface_id: 1,
            output_surfaces: BTreeMap::new(),
            next_bitmap_surface_id: 1,
            bitmap_surfaces: BTreeMap::new(),
            next_decoder_id: 1,
            decoders: BTreeMap::new(),
            next_video_mixer_id: 1,
            video_mixers: BTreeMap::new(),
            next_pq_target_id: 1,
            pq_targets: BTreeMap::new(),
            next_pq_id: 1,
            pqs: BTreeMap::new(),
            initialized: false,
        }
    }

    /// Initialize VDPAU device
    pub fn init(&mut self, vendor_id: u16, device_id: u16, mmio_base: u64) -> VdpStatus {
        self.mmio_base = mmio_base;
        self.device_id = device_id;

        match vendor_id {
            0x10DE => {
                // NVIDIA - native VDPAU support
                self.vendor = VdpVendor::Nvidia;
                self.info_string = "NVIDIA VDPAU Driver".to_string();
                self.init_nvidia(device_id);
            }
            0x8086 | 0x1002 => {
                // Intel or AMD - use Mesa VA-API to VDPAU wrapper
                self.vendor = VdpVendor::Mesa;
                self.info_string = if vendor_id == 0x8086 {
                    "Mesa VDPAU (Intel VA-API backend)".to_string()
                } else {
                    "Mesa VDPAU (AMD VA-API backend)".to_string()
                };
                self.init_mesa(vendor_id, device_id);
            }
            _ => {
                return VdpStatus::NoImplementation;
            }
        }

        self.initialized = true;
        crate::kprintln!("VDPAU: Initialized {} v{}", self.info_string, self.api_version);

        VdpStatus::Ok
    }

    /// Initialize NVIDIA VDPAU
    fn init_nvidia(&mut self, device_id: u16) {
        // Determine GPU generation
        let is_kepler = (device_id & 0xFF00) >= 0x0E00 && (device_id & 0xFF00) < 0x1100;
        let is_maxwell = (device_id & 0xFF00) >= 0x1300 && (device_id & 0xFF00) < 0x1600;
        let is_pascal = (device_id & 0xFF00) >= 0x1500 && (device_id & 0xFF00) < 0x1C00;
        let is_turing = (device_id & 0xFF00) >= 0x1E00 && (device_id & 0xFF00) < 0x2200;
        let is_ampere = (device_id & 0xFF00) >= 0x2200 && (device_id & 0xFF00) < 0x2600;
        let is_ada = (device_id & 0xFF00) >= 0x2600;

        // Surface limits
        self.max_video_surface_width = 8192;
        self.max_video_surface_height = 8192;
        self.max_output_surface_width = 8192;
        self.max_output_surface_height = 8192;

        let mut caps = Vec::new();

        // H.264
        caps.push(DecoderCapability {
            profile: VdpDecoderProfile::H264Baseline,
            is_supported: true,
            max_level: 52,
            max_macroblocks: 36864,  // 4096x4096
            max_width: 4096,
            max_height: 4096,
        });
        caps.push(DecoderCapability {
            profile: VdpDecoderProfile::H264Main,
            is_supported: true,
            max_level: 52,
            max_macroblocks: 36864,
            max_width: 4096,
            max_height: 4096,
        });
        caps.push(DecoderCapability {
            profile: VdpDecoderProfile::H264High,
            is_supported: true,
            max_level: 52,
            max_macroblocks: 36864,
            max_width: 4096,
            max_height: 4096,
        });

        // MPEG-2
        caps.push(DecoderCapability {
            profile: VdpDecoderProfile::Mpeg2Main,
            is_supported: true,
            max_level: 3,
            max_macroblocks: 8160,
            max_width: 1920,
            max_height: 1088,
        });

        // VC-1
        caps.push(DecoderCapability {
            profile: VdpDecoderProfile::Vc1Advanced,
            is_supported: true,
            max_level: 4,
            max_macroblocks: 8160,
            max_width: 1920,
            max_height: 1088,
        });

        // HEVC (Maxwell+)
        if is_maxwell || is_pascal || is_turing || is_ampere || is_ada {
            caps.push(DecoderCapability {
                profile: VdpDecoderProfile::HevcMain,
                is_supported: true,
                max_level: 62,
                max_macroblocks: 262144,  // 8192x8192 / 256
                max_width: 8192,
                max_height: 8192,
            });
            caps.push(DecoderCapability {
                profile: VdpDecoderProfile::HevcMain10,
                is_supported: true,
                max_level: 62,
                max_macroblocks: 262144,
                max_width: 8192,
                max_height: 8192,
            });
        }

        // VP9 (Pascal+)
        if is_pascal || is_turing || is_ampere || is_ada {
            caps.push(DecoderCapability {
                profile: VdpDecoderProfile::Vp9Profile0,
                is_supported: true,
                max_level: 6,
                max_macroblocks: 262144,
                max_width: 8192,
                max_height: 8192,
            });
            caps.push(DecoderCapability {
                profile: VdpDecoderProfile::Vp9Profile2,
                is_supported: is_turing || is_ampere || is_ada,
                max_level: 6,
                max_macroblocks: 262144,
                max_width: 8192,
                max_height: 8192,
            });
        }

        // AV1 (Ampere+)
        if is_ampere || is_ada {
            caps.push(DecoderCapability {
                profile: VdpDecoderProfile::Av1Main,
                is_supported: true,
                max_level: 6,
                max_macroblocks: 262144,
                max_width: 8192,
                max_height: 8192,
            });
        }

        self.decoder_caps = caps;
    }

    /// Initialize Mesa VDPAU (VA-API backend)
    fn init_mesa(&mut self, vendor_id: u16, device_id: u16) {
        // Surface limits (conservative for VA-API backends)
        self.max_video_surface_width = 8192;
        self.max_video_surface_height = 8192;
        self.max_output_surface_width = 8192;
        self.max_output_surface_height = 8192;

        let is_intel = vendor_id == 0x8086;
        let is_amd = vendor_id == 0x1002;

        let mut caps = Vec::new();

        // Common profiles for both Intel and AMD
        caps.push(DecoderCapability {
            profile: VdpDecoderProfile::H264Baseline,
            is_supported: true,
            max_level: 52,
            max_macroblocks: 36864,
            max_width: 4096,
            max_height: 4096,
        });
        caps.push(DecoderCapability {
            profile: VdpDecoderProfile::H264Main,
            is_supported: true,
            max_level: 52,
            max_macroblocks: 36864,
            max_width: 4096,
            max_height: 4096,
        });
        caps.push(DecoderCapability {
            profile: VdpDecoderProfile::H264High,
            is_supported: true,
            max_level: 52,
            max_macroblocks: 36864,
            max_width: 4096,
            max_height: 4096,
        });

        // HEVC
        caps.push(DecoderCapability {
            profile: VdpDecoderProfile::HevcMain,
            is_supported: true,
            max_level: 62,
            max_macroblocks: 262144,
            max_width: 8192,
            max_height: 8192,
        });
        caps.push(DecoderCapability {
            profile: VdpDecoderProfile::HevcMain10,
            is_supported: true,
            max_level: 62,
            max_macroblocks: 262144,
            max_width: 8192,
            max_height: 8192,
        });

        // VP9
        caps.push(DecoderCapability {
            profile: VdpDecoderProfile::Vp9Profile0,
            is_supported: true,
            max_level: 6,
            max_macroblocks: 262144,
            max_width: 8192,
            max_height: 8192,
        });

        // AV1 (newer Intel and AMD)
        let intel_supports_av1 = is_intel && (device_id & 0xFF00) >= 0x4C00;  // Tiger Lake+
        let amd_supports_av1 = is_amd && device_id >= 0x73A0;  // RDNA2+

        if intel_supports_av1 || amd_supports_av1 {
            caps.push(DecoderCapability {
                profile: VdpDecoderProfile::Av1Main,
                is_supported: true,
                max_level: 6,
                max_macroblocks: 262144,
                max_width: 8192,
                max_height: 8192,
            });
        }

        // MPEG-2
        caps.push(DecoderCapability {
            profile: VdpDecoderProfile::Mpeg2Main,
            is_supported: true,
            max_level: 3,
            max_macroblocks: 8160,
            max_width: 1920,
            max_height: 1088,
        });

        // VC-1
        caps.push(DecoderCapability {
            profile: VdpDecoderProfile::Vc1Advanced,
            is_supported: is_intel,  // Intel has better VC-1 support
            max_level: 4,
            max_macroblocks: 8160,
            max_width: 1920,
            max_height: 1088,
        });

        self.decoder_caps = caps;
    }

    /// Query decoder capabilities
    pub fn get_decoder_capabilities(&self, profile: VdpDecoderProfile) -> Option<&DecoderCapability> {
        self.decoder_caps.iter().find(|c| c.profile == profile && c.is_supported)
    }

    /// Create video surface
    pub fn video_surface_create(&mut self, chroma_type: VdpChromaType,
                                 width: u32, height: u32) -> Result<u32, VdpStatus>
    {
        if width > self.max_video_surface_width || height > self.max_video_surface_height {
            return Err(VdpStatus::InvalidSize);
        }

        let id = self.next_video_surface_id;
        self.next_video_surface_id += 1;

        let surface = VdpVideoSurface {
            id,
            chroma_type,
            width,
            height,
            buffer: 0,  // Will be allocated
        };

        self.video_surfaces.insert(id, surface);
        Ok(id)
    }

    /// Destroy video surface
    pub fn video_surface_destroy(&mut self, surface_id: u32) -> VdpStatus {
        if self.video_surfaces.remove(&surface_id).is_some() {
            VdpStatus::Ok
        } else {
            VdpStatus::InvalidHandle
        }
    }

    /// Get video surface parameters
    pub fn video_surface_get_parameters(&self, surface_id: u32)
        -> Result<(VdpChromaType, u32, u32), VdpStatus>
    {
        if let Some(surface) = self.video_surfaces.get(&surface_id) {
            Ok((surface.chroma_type, surface.width, surface.height))
        } else {
            Err(VdpStatus::InvalidHandle)
        }
    }

    /// Create output surface
    pub fn output_surface_create(&mut self, rgba_format: VdpRgbaFormat,
                                  width: u32, height: u32) -> Result<u32, VdpStatus>
    {
        if width > self.max_output_surface_width || height > self.max_output_surface_height {
            return Err(VdpStatus::InvalidSize);
        }

        let id = self.next_output_surface_id;
        self.next_output_surface_id += 1;

        let surface = VdpOutputSurface {
            id,
            rgba_format,
            width,
            height,
            buffer: 0,
        };

        self.output_surfaces.insert(id, surface);
        Ok(id)
    }

    /// Destroy output surface
    pub fn output_surface_destroy(&mut self, surface_id: u32) -> VdpStatus {
        if self.output_surfaces.remove(&surface_id).is_some() {
            VdpStatus::Ok
        } else {
            VdpStatus::InvalidHandle
        }
    }

    /// Create bitmap surface
    pub fn bitmap_surface_create(&mut self, rgba_format: VdpRgbaFormat,
                                  width: u32, height: u32,
                                  frequently_accessed: bool) -> Result<u32, VdpStatus>
    {
        let id = self.next_bitmap_surface_id;
        self.next_bitmap_surface_id += 1;

        let surface = VdpBitmapSurface {
            id,
            rgba_format,
            width,
            height,
            frequently_accessed,
            buffer: 0,
        };

        self.bitmap_surfaces.insert(id, surface);
        Ok(id)
    }

    /// Destroy bitmap surface
    pub fn bitmap_surface_destroy(&mut self, surface_id: u32) -> VdpStatus {
        if self.bitmap_surfaces.remove(&surface_id).is_some() {
            VdpStatus::Ok
        } else {
            VdpStatus::InvalidHandle
        }
    }

    /// Create decoder
    pub fn decoder_create(&mut self, profile: VdpDecoderProfile,
                          width: u32, height: u32, max_references: u32)
        -> Result<u32, VdpStatus>
    {
        // Check if profile is supported
        let cap = self.decoder_caps.iter()
            .find(|c| c.profile == profile && c.is_supported);

        if cap.is_none() {
            return Err(VdpStatus::InvalidDecoderProfile);
        }

        let cap = cap.unwrap();
        if width > cap.max_width || height > cap.max_height {
            return Err(VdpStatus::InvalidSize);
        }

        let id = self.next_decoder_id;
        self.next_decoder_id += 1;

        let decoder = VdpDecoder {
            id,
            profile,
            width,
            height,
            max_references,
        };

        self.decoders.insert(id, decoder);
        Ok(id)
    }

    /// Destroy decoder
    pub fn decoder_destroy(&mut self, decoder_id: u32) -> VdpStatus {
        if self.decoders.remove(&decoder_id).is_some() {
            VdpStatus::Ok
        } else {
            VdpStatus::InvalidHandle
        }
    }

    /// Decode (submit bitstream)
    pub fn decoder_render(&self, decoder_id: u32, _target_surface: u32,
                          _bitstream: &[u8]) -> VdpStatus
    {
        if !self.decoders.contains_key(&decoder_id) {
            return VdpStatus::InvalidHandle;
        }

        // In a real implementation, this would submit the bitstream
        // to the hardware decoder

        VdpStatus::Ok
    }

    /// Create video mixer
    pub fn video_mixer_create(&mut self, features: &[VdpVideoMixerFeature],
                              video_width: u32, video_height: u32,
                              chroma_type: VdpChromaType, layers: u32)
        -> Result<u32, VdpStatus>
    {
        let id = self.next_video_mixer_id;
        self.next_video_mixer_id += 1;

        // Default BT.709 CSC matrix
        let csc_matrix = [
            [1.164, 0.0, 1.793, 0.0],
            [1.164, -0.213, -0.533, 0.0],
            [1.164, 2.112, 0.0, 0.0],
        ];

        let mixer = VdpVideoMixer {
            id,
            features: features.to_vec(),
            video_width,
            video_height,
            chroma_type,
            layers,
            csc_matrix,
            noise_reduction_level: 0.0,
            sharpness_level: 0.0,
            luma_key_min: 0.0,
            luma_key_max: 1.0,
            skip_chroma_deinterlace: false,
            background_color: [0.0, 0.0, 0.0, 1.0],
        };

        self.video_mixers.insert(id, mixer);
        Ok(id)
    }

    /// Set video mixer attribute
    pub fn video_mixer_set_attribute(&mut self, mixer_id: u32,
                                      attribute: VdpVideoMixerAttribute,
                                      value: f32) -> VdpStatus
    {
        if let Some(mixer) = self.video_mixers.get_mut(&mixer_id) {
            match attribute {
                VdpVideoMixerAttribute::NoiseReductionLevel => {
                    mixer.noise_reduction_level = value.clamp(0.0, 1.0);
                }
                VdpVideoMixerAttribute::SharpnessLevel => {
                    mixer.sharpness_level = value.clamp(-1.0, 1.0);
                }
                VdpVideoMixerAttribute::LumaKeyMinLuma => {
                    mixer.luma_key_min = value.clamp(0.0, 1.0);
                }
                VdpVideoMixerAttribute::LumaKeyMaxLuma => {
                    mixer.luma_key_max = value.clamp(0.0, 1.0);
                }
                VdpVideoMixerAttribute::SkipChromaDeinterlace => {
                    mixer.skip_chroma_deinterlace = value != 0.0;
                }
                _ => return VdpStatus::InvalidVideoMixerAttribute,
            }
            VdpStatus::Ok
        } else {
            VdpStatus::InvalidHandle
        }
    }

    /// Destroy video mixer
    pub fn video_mixer_destroy(&mut self, mixer_id: u32) -> VdpStatus {
        if self.video_mixers.remove(&mixer_id).is_some() {
            VdpStatus::Ok
        } else {
            VdpStatus::InvalidHandle
        }
    }

    /// Video mixer render
    pub fn video_mixer_render(&self, mixer_id: u32,
                              _background_surface: Option<u32>,
                              _video_surface: u32,
                              _picture_structure: VdpVideoMixerPictureStructure,
                              _destination_surface: u32) -> VdpStatus
    {
        if !self.video_mixers.contains_key(&mixer_id) {
            return VdpStatus::InvalidHandle;
        }

        // In a real implementation, this would:
        // 1. Apply color space conversion
        // 2. Apply deinterlacing if needed
        // 3. Apply noise reduction/sharpening
        // 4. Scale and composite to output surface

        VdpStatus::Ok
    }

    /// Create presentation queue target
    pub fn presentation_queue_target_create(&mut self, drawable: u64)
        -> Result<u32, VdpStatus>
    {
        let id = self.next_pq_target_id;
        self.next_pq_target_id += 1;

        let target = VdpPresentationQueueTarget {
            id,
            drawable,
        };

        self.pq_targets.insert(id, target);
        Ok(id)
    }

    /// Destroy presentation queue target
    pub fn presentation_queue_target_destroy(&mut self, target_id: u32) -> VdpStatus {
        if self.pq_targets.remove(&target_id).is_some() {
            VdpStatus::Ok
        } else {
            VdpStatus::InvalidHandle
        }
    }

    /// Create presentation queue
    pub fn presentation_queue_create(&mut self, target_id: u32)
        -> Result<u32, VdpStatus>
    {
        if !self.pq_targets.contains_key(&target_id) {
            return Err(VdpStatus::InvalidHandle);
        }

        let id = self.next_pq_id;
        self.next_pq_id += 1;

        let pq = VdpPresentationQueue {
            id,
            target_id,
            background_color: [0.0, 0.0, 0.0, 1.0],
        };

        self.pqs.insert(id, pq);
        Ok(id)
    }

    /// Destroy presentation queue
    pub fn presentation_queue_destroy(&mut self, pq_id: u32) -> VdpStatus {
        if self.pqs.remove(&pq_id).is_some() {
            VdpStatus::Ok
        } else {
            VdpStatus::InvalidHandle
        }
    }

    /// Set presentation queue background color
    pub fn presentation_queue_set_background_color(&mut self, pq_id: u32,
                                                    color: [f32; 4]) -> VdpStatus
    {
        if let Some(pq) = self.pqs.get_mut(&pq_id) {
            pq.background_color = color;
            VdpStatus::Ok
        } else {
            VdpStatus::InvalidHandle
        }
    }

    /// Display (present) a surface
    pub fn presentation_queue_display(&self, pq_id: u32, _surface_id: u32,
                                       _earliest_presentation_time: u64) -> VdpStatus
    {
        if !self.pqs.contains_key(&pq_id) {
            return VdpStatus::InvalidHandle;
        }

        // In a real implementation, this would:
        // 1. Queue the surface for display
        // 2. Wait until earliest_presentation_time
        // 3. Flip the surface to the display

        VdpStatus::Ok
    }

    /// Block until surface is idle
    pub fn presentation_queue_block_until_surface_idle(&self, pq_id: u32,
                                                        _surface_id: u32) -> VdpStatus
    {
        if !self.pqs.contains_key(&pq_id) {
            return VdpStatus::InvalidHandle;
        }

        // In a real implementation, this would wait for GPU completion

        VdpStatus::Ok
    }

    /// Get status
    pub fn get_status(&self) -> String {
        let mut status = String::new();

        status.push_str("VDPAU Status:\n");
        status.push_str(&alloc::format!("  Backend: {:?}\n", self.vendor));
        status.push_str(&alloc::format!("  Info: {}\n", self.info_string));
        status.push_str(&alloc::format!("  API Version: {}\n", self.api_version));
        status.push_str(&alloc::format!("  Max Video Surface: {}x{}\n",
            self.max_video_surface_width, self.max_video_surface_height));
        status.push_str(&alloc::format!("  Max Output Surface: {}x{}\n",
            self.max_output_surface_width, self.max_output_surface_height));

        status.push_str("  Decoder Profiles:\n");
        for cap in &self.decoder_caps {
            if cap.is_supported {
                status.push_str(&alloc::format!("    {}: {}x{}\n",
                    cap.profile.name(), cap.max_width, cap.max_height));
            }
        }

        status.push_str(&alloc::format!("  Video Surfaces: {}\n", self.video_surfaces.len()));
        status.push_str(&alloc::format!("  Output Surfaces: {}\n", self.output_surfaces.len()));
        status.push_str(&alloc::format!("  Decoders: {}\n", self.decoders.len()));
        status.push_str(&alloc::format!("  Video Mixers: {}\n", self.video_mixers.len()));
        status.push_str(&alloc::format!("  Presentation Queues: {}\n", self.pqs.len()));

        status
    }
}

/// Global VDPAU device
static VDPAU_DEVICE: TicketSpinlock<Option<VdpDevice>> = TicketSpinlock::new(None);

/// Initialize VDPAU
pub fn init(vendor_id: u16, device_id: u16, mmio_base: u64) -> VdpStatus {
    let mut guard = VDPAU_DEVICE.lock();
    let mut device = VdpDevice::new();
    let status = device.init(vendor_id, device_id, mmio_base);
    if status.is_ok() {
        *guard = Some(device);
    }
    status
}

/// Get VDPAU device
pub fn get_device() -> Option<&'static TicketSpinlock<Option<VdpDevice>>> {
    Some(&VDPAU_DEVICE)
}

/// Get error string
pub fn get_error_string(status: VdpStatus) -> &'static str {
    status.name()
}
