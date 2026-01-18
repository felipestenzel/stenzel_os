// SPDX-License-Identifier: MIT
// VA-API (Video Acceleration API) driver for Stenzel OS
// Hardware-accelerated video decode/encode support

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::string::ToString;
use alloc::collections::BTreeMap;
use crate::sync::TicketSpinlock;

/// VA-API version
pub const VA_VERSION_MAJOR: u32 = 1;
pub const VA_VERSION_MINOR: u32 = 18;

/// VA status codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum VaStatus {
    Success = 0,
    ErrorOperationFailed = 1,
    ErrorAllocationFailed = 2,
    ErrorInvalidDisplay = 3,
    ErrorInvalidConfig = 4,
    ErrorInvalidContext = 5,
    ErrorInvalidSurface = 6,
    ErrorInvalidBuffer = 7,
    ErrorInvalidImage = 8,
    ErrorInvalidSubpicture = 9,
    ErrorAttrNotSupported = 10,
    ErrorMaxNumExceeded = 11,
    ErrorUnsupportedProfile = 12,
    ErrorUnsupportedEntrypoint = 13,
    ErrorUnsupportedRtFormat = 14,
    ErrorUnsupportedBuffertype = 15,
    ErrorSurfaceBusy = 16,
    ErrorFlagNotSupported = 17,
    ErrorInvalidParameter = 18,
    ErrorResolutionNotSupported = 19,
    ErrorUnimplemented = 20,
    ErrorSurfaceInDisplaying = 21,
    ErrorInvalidImageFormat = 22,
    ErrorDecodingError = 23,
    ErrorEncodingError = 24,
    ErrorInvalidValue = 25,
    ErrorTimedOut = 26,
    ErrorUnknown = -1,
}

impl VaStatus {
    pub fn is_success(self) -> bool {
        self == VaStatus::Success
    }
}

/// Video codec profiles
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum VaProfile {
    None = 0,
    // MPEG-2
    Mpeg2Simple = 1,
    Mpeg2Main = 2,
    // MPEG-4
    Mpeg4Simple = 3,
    Mpeg4AdvancedSimple = 4,
    Mpeg4Main = 5,
    // H.264/AVC
    H264Baseline = 6,
    H264Main = 7,
    H264High = 8,
    H264ConstrainedBaseline = 13,
    H264MultiviewHigh = 14,
    H264StereoHigh = 15,
    H264High10 = 16,
    H264High422 = 17,
    H264High444 = 18,
    // VC-1
    Vc1Simple = 9,
    Vc1Main = 10,
    Vc1Advanced = 11,
    // JPEG
    JpegBaseline = 12,
    // VP8
    Vp8Version0_3 = 19,
    // VP9
    Vp9Profile0 = 20,
    Vp9Profile1 = 21,
    Vp9Profile2 = 22,
    Vp9Profile3 = 23,
    // HEVC/H.265
    HevcMain = 24,
    HevcMain10 = 25,
    HevcMain12 = 26,
    HevcMain422_10 = 27,
    HevcMain422_12 = 28,
    HevcMain444 = 29,
    HevcMain444_10 = 30,
    HevcMain444_12 = 31,
    HevcSccMain = 32,
    HevcSccMain10 = 33,
    HevcSccMain444 = 34,
    // AV1
    Av1Profile0 = 35,
    Av1Profile1 = 36,
    // Protected content
    Protected = 37,
}

impl VaProfile {
    pub fn name(self) -> &'static str {
        match self {
            VaProfile::None => "None",
            VaProfile::Mpeg2Simple => "MPEG2 Simple",
            VaProfile::Mpeg2Main => "MPEG2 Main",
            VaProfile::Mpeg4Simple => "MPEG4 Simple",
            VaProfile::Mpeg4AdvancedSimple => "MPEG4 Advanced Simple",
            VaProfile::Mpeg4Main => "MPEG4 Main",
            VaProfile::H264Baseline => "H.264 Baseline",
            VaProfile::H264Main => "H.264 Main",
            VaProfile::H264High => "H.264 High",
            VaProfile::H264ConstrainedBaseline => "H.264 Constrained Baseline",
            VaProfile::H264MultiviewHigh => "H.264 Multiview High",
            VaProfile::H264StereoHigh => "H.264 Stereo High",
            VaProfile::H264High10 => "H.264 High 10",
            VaProfile::H264High422 => "H.264 High 4:2:2",
            VaProfile::H264High444 => "H.264 High 4:4:4",
            VaProfile::Vc1Simple => "VC-1 Simple",
            VaProfile::Vc1Main => "VC-1 Main",
            VaProfile::Vc1Advanced => "VC-1 Advanced",
            VaProfile::JpegBaseline => "JPEG Baseline",
            VaProfile::Vp8Version0_3 => "VP8",
            VaProfile::Vp9Profile0 => "VP9 Profile 0",
            VaProfile::Vp9Profile1 => "VP9 Profile 1",
            VaProfile::Vp9Profile2 => "VP9 Profile 2",
            VaProfile::Vp9Profile3 => "VP9 Profile 3",
            VaProfile::HevcMain => "HEVC Main",
            VaProfile::HevcMain10 => "HEVC Main 10",
            VaProfile::HevcMain12 => "HEVC Main 12",
            VaProfile::HevcMain422_10 => "HEVC Main 4:2:2 10",
            VaProfile::HevcMain422_12 => "HEVC Main 4:2:2 12",
            VaProfile::HevcMain444 => "HEVC Main 4:4:4",
            VaProfile::HevcMain444_10 => "HEVC Main 4:4:4 10",
            VaProfile::HevcMain444_12 => "HEVC Main 4:4:4 12",
            VaProfile::HevcSccMain => "HEVC SCC Main",
            VaProfile::HevcSccMain10 => "HEVC SCC Main 10",
            VaProfile::HevcSccMain444 => "HEVC SCC Main 4:4:4",
            VaProfile::Av1Profile0 => "AV1 Profile 0",
            VaProfile::Av1Profile1 => "AV1 Profile 1",
            VaProfile::Protected => "Protected",
        }
    }

    pub fn is_encode_capable(self) -> bool {
        matches!(self,
            VaProfile::H264Baseline | VaProfile::H264Main | VaProfile::H264High |
            VaProfile::H264ConstrainedBaseline | VaProfile::H264High10 |
            VaProfile::HevcMain | VaProfile::HevcMain10 | VaProfile::HevcMain444 |
            VaProfile::Vp9Profile0 | VaProfile::Av1Profile0 |
            VaProfile::JpegBaseline
        )
    }
}

/// Entry points (decode, encode, processing)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum VaEntrypoint {
    Vld = 1,           // Variable Length Decode (decode)
    Idct = 2,          // Inverse DCT (decode)
    MoComp = 3,        // Motion Compensation (decode)
    Deblocking = 4,    // Deblocking (post-processing)
    EncSlice = 5,      // Slice-based encode
    EncPicture = 6,    // Picture-based encode
    EncSliceLp = 7,    // Low-power slice-based encode
    VideoProc = 8,     // Video processing
    Fei = 9,           // Flexible Encoding Infrastructure
    Stats = 10,        // Statistics collection
    ProtectedTeeComm = 11,  // Protected content TEE communication
    ProtectedContent = 12,  // Protected content decode
}

impl VaEntrypoint {
    pub fn name(self) -> &'static str {
        match self {
            VaEntrypoint::Vld => "VLD (Decode)",
            VaEntrypoint::Idct => "IDCT",
            VaEntrypoint::MoComp => "Motion Compensation",
            VaEntrypoint::Deblocking => "Deblocking",
            VaEntrypoint::EncSlice => "Slice Encode",
            VaEntrypoint::EncPicture => "Picture Encode",
            VaEntrypoint::EncSliceLp => "Low-Power Slice Encode",
            VaEntrypoint::VideoProc => "Video Processing",
            VaEntrypoint::Fei => "FEI",
            VaEntrypoint::Stats => "Statistics",
            VaEntrypoint::ProtectedTeeComm => "Protected TEE",
            VaEntrypoint::ProtectedContent => "Protected Content",
        }
    }

    pub fn is_decode(self) -> bool {
        matches!(self, VaEntrypoint::Vld | VaEntrypoint::Idct | VaEntrypoint::MoComp)
    }

    pub fn is_encode(self) -> bool {
        matches!(self, VaEntrypoint::EncSlice | VaEntrypoint::EncPicture | VaEntrypoint::EncSliceLp)
    }
}

/// Render target formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum VaRtFormat {
    Yuv420 = 0x00000001,
    Yuv422 = 0x00000002,
    Yuv444 = 0x00000004,
    Yuv411 = 0x00000008,
    Yuv400 = 0x00000010,
    Yuv420_10 = 0x00000100,
    Yuv422_10 = 0x00000200,
    Yuv444_10 = 0x00000400,
    Yuv420_12 = 0x00001000,
    Yuv422_12 = 0x00002000,
    Yuv444_12 = 0x00004000,
    Rgb16 = 0x00010000,
    Rgb32 = 0x00020000,
    RgbP = 0x00100000,
    Rgb32_10 = 0x00200000,
    Protected = 0x80000000,
}

impl VaRtFormat {
    pub fn bits_per_component(self) -> u32 {
        match self {
            VaRtFormat::Yuv420_10 | VaRtFormat::Yuv422_10 |
            VaRtFormat::Yuv444_10 | VaRtFormat::Rgb32_10 => 10,
            VaRtFormat::Yuv420_12 | VaRtFormat::Yuv422_12 |
            VaRtFormat::Yuv444_12 => 12,
            _ => 8,
        }
    }
}

/// Buffer types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum VaBufferType {
    PicParam = 0,
    IqMatrix = 1,
    BitPlane = 2,
    SliceGroupMap = 3,
    SliceParam = 4,
    SliceData = 5,
    MacroblockParam = 6,
    ResidualData = 7,
    DeblockingParam = 8,
    Image = 9,
    ProtectedSliceData = 10,
    QMatrix = 11,
    HuffmanTable = 12,
    Probability = 13,
    // Encode buffers
    EncCodedBuf = 21,
    EncSeqParam = 22,
    EncPicParam = 23,
    EncSliceParam = 24,
    EncPackedHeaderParam = 25,
    EncPackedHeaderData = 26,
    EncMiscParam = 27,
    EncMacroblockParam = 28,
    EncMacroblockMap = 29,
    EncQp = 30,
    // Video processing
    ProcPipelineParam = 41,
    ProcFilterParam = 42,
    // FEI
    FeiMvPredictor = 43,
    FeiMbCode = 44,
    FeiDistortion = 45,
    FeiMbControl = 46,
    FeiMvOut = 47,
    FeiQp = 48,
    // Stats
    StatsStatistics = 49,
    StatsMvPredictor = 50,
    StatsMv = 51,
    // Protected
    SubsampleEncrypt = 52,
    ProtectedSession = 53,
}

/// Configuration attributes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum VaConfigAttribType {
    RtFormat = 0,
    SpatialResidu = 1,
    SpatialClipping = 2,
    IntraResidual = 3,
    Encryption = 4,
    RateControl = 5,
    DecSliceMode = 6,
    DecJpeg = 7,
    DecProcessing = 8,
    EncPackedHeaders = 10,
    EncInterlaced = 11,
    EncMaxRefFrames = 13,
    EncMaxSlices = 14,
    EncSliceStructure = 15,
    EncMacroblockInfo = 16,
    MaxPictureWidth = 18,
    MaxPictureHeight = 19,
    EncQualityRange = 21,
    EncQuantization = 22,
    EncIntraRefresh = 23,
    EncSkipFrame = 24,
    EncRoi = 25,
    EncRateControlExt = 26,
    ProcessingRate = 27,
    EncDirtyRect = 28,
    EncParallelRateControl = 29,
    EncDynamicScaling = 30,
    FrameSizeToleranceSupport = 31,
    FeiFunction = 32,
    EncTileSupport = 33,
    Custom = 34,
    DecAv1Features = 35,
    TeeType = 36,
    TeeTypeLive = 37,
    ProtectedContentCipherAlgorithm = 38,
    ProtectedContentCipherBlockSize = 39,
    ProtectedContentCipherMode = 40,
    ProtectedContentCipherSampleType = 41,
    ProtectedContentUsage = 42,
    EncAv1 = 43,
    EncAv1Ext1 = 44,
    EncAv1Ext2 = 45,
    EncPerBlockControl = 46,
    ContextPriority = 47,
    MaxFrameSize = 48,
    PredictionDirection = 49,
}

/// Surface attribute types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum VaSurfaceAttribType {
    None = 0,
    PixelFormat = 1,
    MinWidth = 2,
    MaxWidth = 3,
    MinHeight = 4,
    MaxHeight = 5,
    MemoryType = 6,
    ExternalBufferDescriptor = 7,
    UsageHint = 8,
    DrmFormatModifiers = 9,
}

/// Pixel formats (FourCC)
pub mod fourcc {
    pub const NV12: u32 = 0x3231564E; // 'NV12'
    pub const NV21: u32 = 0x3132564E; // 'NV21'
    pub const YV12: u32 = 0x32315659; // 'YV12'
    pub const IYUV: u32 = 0x56555949; // 'IYUV'
    pub const I420: u32 = 0x30323449; // 'I420'
    pub const YUY2: u32 = 0x32595559; // 'YUY2'
    pub const UYVY: u32 = 0x59565955; // 'UYVY'
    pub const Y800: u32 = 0x30303859; // 'Y800'
    pub const P010: u32 = 0x30313050; // 'P010'
    pub const P012: u32 = 0x32313050; // 'P012'
    pub const P016: u32 = 0x36313050; // 'P016'
    pub const Y210: u32 = 0x30313259; // 'Y210'
    pub const Y212: u32 = 0x32313259; // 'Y212'
    pub const Y216: u32 = 0x36313259; // 'Y216'
    pub const Y410: u32 = 0x30313459; // 'Y410'
    pub const Y412: u32 = 0x32313459; // 'Y412'
    pub const Y416: u32 = 0x36313459; // 'Y416'
    pub const RGBX: u32 = 0x58424752; // 'RGBX'
    pub const BGRX: u32 = 0x58524742; // 'BGRX'
    pub const ARGB: u32 = 0x42475241; // 'ARGB'
    pub const ABGR: u32 = 0x52474241; // 'ABGR'
    pub const RGBA: u32 = 0x41424752; // 'RGBA'
    pub const BGRA: u32 = 0x41524742; // 'BGRA'
    pub const A2R10G10B10: u32 = 0x30335241; // 'AR30'
    pub const A2B10G10R10: u32 = 0x30334241; // 'AB30'
}

/// Hardware vendor types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaVendor {
    Intel,
    Amd,
    Nvidia,
    Unknown,
}

/// Configuration
#[derive(Debug, Clone)]
pub struct VaConfig {
    pub id: u32,
    pub profile: VaProfile,
    pub entrypoint: VaEntrypoint,
    pub rt_format: u32,
    pub attributes: Vec<(VaConfigAttribType, u32)>,
}

/// Surface
#[derive(Debug, Clone)]
pub struct VaSurface {
    pub id: u32,
    pub width: u32,
    pub height: u32,
    pub format: u32,
    pub rt_format: VaRtFormat,
    pub buffer_handle: u64,
    pub pitch: u32,
    pub offset: u32,
    pub in_use: bool,
}

/// Context
#[derive(Debug, Clone)]
pub struct VaContext {
    pub id: u32,
    pub config_id: u32,
    pub picture_width: u32,
    pub picture_height: u32,
    pub surfaces: Vec<u32>,
    pub flags: u32,
}

/// Buffer
#[derive(Debug, Clone)]
pub struct VaBuffer {
    pub id: u32,
    pub buffer_type: VaBufferType,
    pub size: usize,
    pub num_elements: u32,
    pub data: Vec<u8>,
    pub mapped: bool,
}

/// Image
#[derive(Debug, Clone)]
pub struct VaImage {
    pub id: u32,
    pub format: u32,
    pub width: u32,
    pub height: u32,
    pub data_size: u32,
    pub num_planes: u32,
    pub pitches: [u32; 4],
    pub offsets: [u32; 4],
    pub buf_id: u32,
}

/// Profile capabilities
#[derive(Debug, Clone)]
pub struct ProfileCapability {
    pub profile: VaProfile,
    pub entrypoints: Vec<VaEntrypoint>,
    pub max_width: u32,
    pub max_height: u32,
    pub rt_formats: u32,
}

/// Encode rate control modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum VaRcMode {
    None = 0,
    Cbr = 1,        // Constant bitrate
    Vbr = 2,        // Variable bitrate
    Vcm = 4,        // Video conferencing mode
    Cqp = 8,        // Constant QP
    Vbr_Constrained = 16,
    Icq = 32,       // Intelligent constant quality
    Mb = 64,        // Macroblock-level rate control
    Cfs = 128,      // Constant frame size
    Parallel = 256,
    Qvbr = 512,     // Quality variable bitrate
    Avbr = 1024,    // Adaptive variable bitrate
}

/// Video processing pipeline
#[derive(Debug, Clone)]
pub struct VppCapabilities {
    pub deinterlacing: bool,
    pub noise_reduction: bool,
    pub sharpening: bool,
    pub color_balance: bool,
    pub skin_tone_enhancement: bool,
    pub proc_amp: bool,
    pub scaling: bool,
    pub blending: bool,
    pub color_standard_conversion: bool,
    pub rotation: bool,
    pub mirroring: bool,
    pub hdr_tone_mapping: bool,
    pub high_dynamic_range: bool,
    pub three_dlut: bool,
}

/// VA-API display state
pub struct VaDisplay {
    pub vendor: VaVendor,
    pub vendor_string: String,
    pub driver_version: String,
    pub major_version: u32,
    pub minor_version: u32,

    // GPU MMIO base
    pub mmio_base: u64,

    // Video engine registers (Intel-style)
    pub vdbox_base: u64,
    pub vebox_base: u64,

    // Capabilities
    pub profiles: Vec<ProfileCapability>,
    pub vpp: VppCapabilities,

    // Resources
    next_config_id: u32,
    configs: BTreeMap<u32, VaConfig>,
    next_surface_id: u32,
    surfaces: BTreeMap<u32, VaSurface>,
    next_context_id: u32,
    contexts: BTreeMap<u32, VaContext>,
    next_buffer_id: u32,
    buffers: BTreeMap<u32, VaBuffer>,
    next_image_id: u32,
    images: BTreeMap<u32, VaImage>,

    initialized: bool,
}

impl VaDisplay {
    pub const fn new() -> Self {
        Self {
            vendor: VaVendor::Unknown,
            vendor_string: String::new(),
            driver_version: String::new(),
            major_version: VA_VERSION_MAJOR,
            minor_version: VA_VERSION_MINOR,
            mmio_base: 0,
            vdbox_base: 0,
            vebox_base: 0,
            profiles: Vec::new(),
            vpp: VppCapabilities {
                deinterlacing: false,
                noise_reduction: false,
                sharpening: false,
                color_balance: false,
                skin_tone_enhancement: false,
                proc_amp: false,
                scaling: false,
                blending: false,
                color_standard_conversion: false,
                rotation: false,
                mirroring: false,
                hdr_tone_mapping: false,
                high_dynamic_range: false,
                three_dlut: false,
            },
            next_config_id: 1,
            configs: BTreeMap::new(),
            next_surface_id: 1,
            surfaces: BTreeMap::new(),
            next_context_id: 1,
            contexts: BTreeMap::new(),
            next_buffer_id: 1,
            buffers: BTreeMap::new(),
            next_image_id: 1,
            images: BTreeMap::new(),
            initialized: false,
        }
    }

    /// Initialize VA-API with detected GPU
    pub fn init(&mut self, vendor_id: u16, device_id: u16, mmio_base: u64) -> VaStatus {
        self.mmio_base = mmio_base;

        // Detect vendor
        match vendor_id {
            0x8086 => {
                self.vendor = VaVendor::Intel;
                self.vendor_string = "Intel".to_string();
                self.init_intel(device_id);
            }
            0x1002 => {
                self.vendor = VaVendor::Amd;
                self.vendor_string = "AMD".to_string();
                self.init_amd(device_id);
            }
            0x10DE => {
                self.vendor = VaVendor::Nvidia;
                self.vendor_string = "NVIDIA".to_string();
                self.init_nvidia(device_id);
            }
            _ => {
                return VaStatus::ErrorInvalidDisplay;
            }
        }

        self.initialized = true;
        crate::kprintln!("VA-API: Initialized {} driver v{}.{}",
            self.vendor_string, self.major_version, self.minor_version);

        VaStatus::Success
    }

    /// Initialize Intel Quick Sync Video
    fn init_intel(&mut self, device_id: u16) {
        self.driver_version = "Intel iHD driver 23.4.0".to_string();

        // Video box base (varies by generation)
        self.vdbox_base = self.mmio_base + 0x1C0000;
        self.vebox_base = self.mmio_base + 0x1C8000;

        // Intel supports extensive profiles
        let mut profiles = Vec::new();

        // Check generation from device ID
        let is_gen12_plus = (device_id & 0xFF00) >= 0x4C00;  // Tiger Lake+
        let is_gen11_plus = (device_id & 0xFF00) >= 0x8A00;  // Ice Lake+

        // H.264
        profiles.push(ProfileCapability {
            profile: VaProfile::H264ConstrainedBaseline,
            entrypoints: vec![VaEntrypoint::Vld, VaEntrypoint::EncSlice, VaEntrypoint::EncSliceLp],
            max_width: 4096,
            max_height: 4096,
            rt_formats: VaRtFormat::Yuv420 as u32,
        });
        profiles.push(ProfileCapability {
            profile: VaProfile::H264Main,
            entrypoints: vec![VaEntrypoint::Vld, VaEntrypoint::EncSlice, VaEntrypoint::EncSliceLp],
            max_width: 4096,
            max_height: 4096,
            rt_formats: VaRtFormat::Yuv420 as u32,
        });
        profiles.push(ProfileCapability {
            profile: VaProfile::H264High,
            entrypoints: vec![VaEntrypoint::Vld, VaEntrypoint::EncSlice, VaEntrypoint::EncSliceLp],
            max_width: 4096,
            max_height: 4096,
            rt_formats: VaRtFormat::Yuv420 as u32,
        });

        // HEVC
        profiles.push(ProfileCapability {
            profile: VaProfile::HevcMain,
            entrypoints: vec![VaEntrypoint::Vld, VaEntrypoint::EncSlice, VaEntrypoint::EncSliceLp],
            max_width: 8192,
            max_height: 8192,
            rt_formats: VaRtFormat::Yuv420 as u32,
        });
        profiles.push(ProfileCapability {
            profile: VaProfile::HevcMain10,
            entrypoints: vec![VaEntrypoint::Vld, VaEntrypoint::EncSlice, VaEntrypoint::EncSliceLp],
            max_width: 8192,
            max_height: 8192,
            rt_formats: VaRtFormat::Yuv420_10 as u32,
        });

        // VP9
        profiles.push(ProfileCapability {
            profile: VaProfile::Vp9Profile0,
            entrypoints: vec![VaEntrypoint::Vld, VaEntrypoint::EncSlice],
            max_width: 8192,
            max_height: 8192,
            rt_formats: VaRtFormat::Yuv420 as u32,
        });
        profiles.push(ProfileCapability {
            profile: VaProfile::Vp9Profile2,
            entrypoints: vec![VaEntrypoint::Vld],
            max_width: 8192,
            max_height: 8192,
            rt_formats: VaRtFormat::Yuv420_10 as u32,
        });

        // AV1 (Gen12+)
        if is_gen12_plus {
            profiles.push(ProfileCapability {
                profile: VaProfile::Av1Profile0,
                entrypoints: vec![VaEntrypoint::Vld, VaEntrypoint::EncSlice],
                max_width: 8192,
                max_height: 8192,
                rt_formats: VaRtFormat::Yuv420 as u32 | VaRtFormat::Yuv420_10 as u32,
            });
        }

        // JPEG
        profiles.push(ProfileCapability {
            profile: VaProfile::JpegBaseline,
            entrypoints: vec![VaEntrypoint::Vld, VaEntrypoint::EncPicture],
            max_width: 16384,
            max_height: 16384,
            rt_formats: VaRtFormat::Yuv420 as u32 | VaRtFormat::Yuv422 as u32 | VaRtFormat::Yuv444 as u32,
        });

        // MPEG-2
        profiles.push(ProfileCapability {
            profile: VaProfile::Mpeg2Main,
            entrypoints: vec![VaEntrypoint::Vld],
            max_width: 1920,
            max_height: 1088,
            rt_formats: VaRtFormat::Yuv420 as u32,
        });

        // VC-1
        profiles.push(ProfileCapability {
            profile: VaProfile::Vc1Advanced,
            entrypoints: vec![VaEntrypoint::Vld],
            max_width: 1920,
            max_height: 1088,
            rt_formats: VaRtFormat::Yuv420 as u32,
        });

        self.profiles = profiles;

        // VPP capabilities
        self.vpp = VppCapabilities {
            deinterlacing: true,
            noise_reduction: true,
            sharpening: true,
            color_balance: true,
            skin_tone_enhancement: true,
            proc_amp: true,
            scaling: true,
            blending: true,
            color_standard_conversion: true,
            rotation: true,
            mirroring: true,
            hdr_tone_mapping: is_gen11_plus,
            high_dynamic_range: is_gen11_plus,
            three_dlut: is_gen12_plus,
        };
    }

    /// Initialize AMD VCN (Video Core Next)
    fn init_amd(&mut self, device_id: u16) {
        self.driver_version = "Mesa Gallium radeonsi VA-API 23.3.0".to_string();

        // AMD VCN base varies
        self.vdbox_base = self.mmio_base + 0x1FA00;

        // Check for VCN generation
        let is_vcn3 = device_id >= 0x73A0;  // RDNA 2+
        let is_vcn4 = device_id >= 0x7440;  // RDNA 3+

        let mut profiles = Vec::new();

        // H.264
        profiles.push(ProfileCapability {
            profile: VaProfile::H264ConstrainedBaseline,
            entrypoints: vec![VaEntrypoint::Vld, VaEntrypoint::EncSlice],
            max_width: 4096,
            max_height: 4096,
            rt_formats: VaRtFormat::Yuv420 as u32,
        });
        profiles.push(ProfileCapability {
            profile: VaProfile::H264Main,
            entrypoints: vec![VaEntrypoint::Vld, VaEntrypoint::EncSlice],
            max_width: 4096,
            max_height: 4096,
            rt_formats: VaRtFormat::Yuv420 as u32,
        });
        profiles.push(ProfileCapability {
            profile: VaProfile::H264High,
            entrypoints: vec![VaEntrypoint::Vld, VaEntrypoint::EncSlice],
            max_width: 4096,
            max_height: 4096,
            rt_formats: VaRtFormat::Yuv420 as u32,
        });

        // HEVC
        profiles.push(ProfileCapability {
            profile: VaProfile::HevcMain,
            entrypoints: vec![VaEntrypoint::Vld, VaEntrypoint::EncSlice],
            max_width: 8192,
            max_height: 8192,
            rt_formats: VaRtFormat::Yuv420 as u32,
        });
        profiles.push(ProfileCapability {
            profile: VaProfile::HevcMain10,
            entrypoints: vec![VaEntrypoint::Vld, VaEntrypoint::EncSlice],
            max_width: 8192,
            max_height: 8192,
            rt_formats: VaRtFormat::Yuv420_10 as u32,
        });

        // VP9
        profiles.push(ProfileCapability {
            profile: VaProfile::Vp9Profile0,
            entrypoints: vec![VaEntrypoint::Vld],
            max_width: 8192,
            max_height: 8192,
            rt_formats: VaRtFormat::Yuv420 as u32,
        });
        profiles.push(ProfileCapability {
            profile: VaProfile::Vp9Profile2,
            entrypoints: vec![VaEntrypoint::Vld],
            max_width: 8192,
            max_height: 8192,
            rt_formats: VaRtFormat::Yuv420_10 as u32,
        });

        // AV1 (VCN 3+)
        if is_vcn3 {
            profiles.push(ProfileCapability {
                profile: VaProfile::Av1Profile0,
                entrypoints: vec![VaEntrypoint::Vld],
                max_width: 8192,
                max_height: 8192,
                rt_formats: VaRtFormat::Yuv420 as u32 | VaRtFormat::Yuv420_10 as u32,
            });
        }

        // AV1 encode (VCN 4+)
        if is_vcn4 {
            if let Some(av1_cap) = profiles.iter_mut().find(|p| p.profile == VaProfile::Av1Profile0) {
                av1_cap.entrypoints.push(VaEntrypoint::EncSlice);
            }
        }

        // JPEG
        profiles.push(ProfileCapability {
            profile: VaProfile::JpegBaseline,
            entrypoints: vec![VaEntrypoint::Vld],
            max_width: 16384,
            max_height: 16384,
            rt_formats: VaRtFormat::Yuv420 as u32,
        });

        self.profiles = profiles;

        // VPP capabilities
        self.vpp = VppCapabilities {
            deinterlacing: true,
            noise_reduction: false,
            sharpening: true,
            color_balance: true,
            skin_tone_enhancement: false,
            proc_amp: true,
            scaling: true,
            blending: true,
            color_standard_conversion: true,
            rotation: true,
            mirroring: true,
            hdr_tone_mapping: is_vcn3,
            high_dynamic_range: is_vcn3,
            three_dlut: false,
        };
    }

    /// Initialize NVIDIA NVDEC/NVENC
    fn init_nvidia(&mut self, device_id: u16) {
        self.driver_version = "NVIDIA VA-API driver 535.0".to_string();

        // NVIDIA doesn't have native VA-API but we can emulate through NVDEC/NVENC
        self.vdbox_base = self.mmio_base + 0x84000;

        // Check generation
        let is_turing_plus = device_id >= 0x1E00;  // RTX 20 series+
        let is_ampere_plus = device_id >= 0x2200;  // RTX 30 series+
        let is_ada_plus = device_id >= 0x2600;     // RTX 40 series+

        let mut profiles = Vec::new();

        // H.264
        profiles.push(ProfileCapability {
            profile: VaProfile::H264ConstrainedBaseline,
            entrypoints: vec![VaEntrypoint::Vld, VaEntrypoint::EncSlice],
            max_width: 4096,
            max_height: 4096,
            rt_formats: VaRtFormat::Yuv420 as u32,
        });
        profiles.push(ProfileCapability {
            profile: VaProfile::H264Main,
            entrypoints: vec![VaEntrypoint::Vld, VaEntrypoint::EncSlice],
            max_width: 4096,
            max_height: 4096,
            rt_formats: VaRtFormat::Yuv420 as u32,
        });
        profiles.push(ProfileCapability {
            profile: VaProfile::H264High,
            entrypoints: vec![VaEntrypoint::Vld, VaEntrypoint::EncSlice],
            max_width: 4096,
            max_height: 4096,
            rt_formats: VaRtFormat::Yuv420 as u32,
        });

        // HEVC
        profiles.push(ProfileCapability {
            profile: VaProfile::HevcMain,
            entrypoints: vec![VaEntrypoint::Vld, VaEntrypoint::EncSlice],
            max_width: 8192,
            max_height: 8192,
            rt_formats: VaRtFormat::Yuv420 as u32,
        });
        profiles.push(ProfileCapability {
            profile: VaProfile::HevcMain10,
            entrypoints: vec![VaEntrypoint::Vld, VaEntrypoint::EncSlice],
            max_width: 8192,
            max_height: 8192,
            rt_formats: VaRtFormat::Yuv420_10 as u32,
        });

        // VP9
        profiles.push(ProfileCapability {
            profile: VaProfile::Vp9Profile0,
            entrypoints: vec![VaEntrypoint::Vld],
            max_width: 8192,
            max_height: 8192,
            rt_formats: VaRtFormat::Yuv420 as u32,
        });
        profiles.push(ProfileCapability {
            profile: VaProfile::Vp9Profile2,
            entrypoints: vec![VaEntrypoint::Vld],
            max_width: 8192,
            max_height: 8192,
            rt_formats: VaRtFormat::Yuv420_10 as u32,
        });

        // AV1 (Ampere+)
        if is_ampere_plus {
            profiles.push(ProfileCapability {
                profile: VaProfile::Av1Profile0,
                entrypoints: vec![VaEntrypoint::Vld],
                max_width: 8192,
                max_height: 8192,
                rt_formats: VaRtFormat::Yuv420 as u32 | VaRtFormat::Yuv420_10 as u32,
            });
        }

        // AV1 encode (Ada Lovelace+)
        if is_ada_plus {
            if let Some(av1_cap) = profiles.iter_mut().find(|p| p.profile == VaProfile::Av1Profile0) {
                av1_cap.entrypoints.push(VaEntrypoint::EncSlice);
            }
        }

        self.profiles = profiles;

        // VPP capabilities (via CUDA/OptiX, limited in VA-API wrapper)
        self.vpp = VppCapabilities {
            deinterlacing: true,
            noise_reduction: is_turing_plus,
            sharpening: true,
            color_balance: true,
            skin_tone_enhancement: false,
            proc_amp: true,
            scaling: true,
            blending: true,
            color_standard_conversion: true,
            rotation: true,
            mirroring: true,
            hdr_tone_mapping: is_ampere_plus,
            high_dynamic_range: is_turing_plus,
            three_dlut: false,
        };
    }

    /// Query profiles
    pub fn query_profiles(&self) -> Vec<VaProfile> {
        self.profiles.iter().map(|p| p.profile).collect()
    }

    /// Query entrypoints for a profile
    pub fn query_entrypoints(&self, profile: VaProfile) -> Vec<VaEntrypoint> {
        self.profiles.iter()
            .find(|p| p.profile == profile)
            .map(|p| p.entrypoints.clone())
            .unwrap_or_default()
    }

    /// Get configuration attributes
    pub fn get_config_attribs(&self, profile: VaProfile, entrypoint: VaEntrypoint)
        -> Vec<(VaConfigAttribType, u32)>
    {
        let mut attribs = Vec::new();

        if let Some(cap) = self.profiles.iter().find(|p| p.profile == profile) {
            // RT format
            attribs.push((VaConfigAttribType::RtFormat, cap.rt_formats));

            // Max resolution
            attribs.push((VaConfigAttribType::MaxPictureWidth, cap.max_width));
            attribs.push((VaConfigAttribType::MaxPictureHeight, cap.max_height));

            // Encode-specific
            if entrypoint.is_encode() {
                attribs.push((VaConfigAttribType::RateControl,
                    VaRcMode::Cbr as u32 | VaRcMode::Vbr as u32 | VaRcMode::Cqp as u32));
                attribs.push((VaConfigAttribType::EncMaxSlices, 32));
                attribs.push((VaConfigAttribType::EncMaxRefFrames, 4));
            }
        }

        attribs
    }

    /// Create configuration
    pub fn create_config(&mut self, profile: VaProfile, entrypoint: VaEntrypoint,
                         attribs: &[(VaConfigAttribType, u32)]) -> Result<u32, VaStatus>
    {
        // Verify profile/entrypoint
        if let Some(cap) = self.profiles.iter().find(|p| p.profile == profile) {
            if !cap.entrypoints.contains(&entrypoint) {
                return Err(VaStatus::ErrorUnsupportedEntrypoint);
            }
        } else {
            return Err(VaStatus::ErrorUnsupportedProfile);
        }

        let id = self.next_config_id;
        self.next_config_id += 1;

        let mut rt_format = VaRtFormat::Yuv420 as u32;
        for (attr_type, value) in attribs {
            if *attr_type == VaConfigAttribType::RtFormat {
                rt_format = *value;
            }
        }

        let config = VaConfig {
            id,
            profile,
            entrypoint,
            rt_format,
            attributes: attribs.to_vec(),
        };

        self.configs.insert(id, config);
        Ok(id)
    }

    /// Destroy configuration
    pub fn destroy_config(&mut self, config_id: u32) -> VaStatus {
        if self.configs.remove(&config_id).is_some() {
            VaStatus::Success
        } else {
            VaStatus::ErrorInvalidConfig
        }
    }

    /// Create surfaces
    pub fn create_surfaces(&mut self, width: u32, height: u32, format: VaRtFormat,
                           count: u32) -> Result<Vec<u32>, VaStatus>
    {
        let mut surface_ids = Vec::new();

        for _ in 0..count {
            let id = self.next_surface_id;
            self.next_surface_id += 1;

            let surface = VaSurface {
                id,
                width,
                height,
                format: fourcc::NV12,
                rt_format: format,
                buffer_handle: 0,  // Will be allocated by GPU driver
                pitch: width,
                offset: 0,
                in_use: false,
            };

            self.surfaces.insert(id, surface);
            surface_ids.push(id);
        }

        Ok(surface_ids)
    }

    /// Destroy surfaces
    pub fn destroy_surfaces(&mut self, surface_ids: &[u32]) -> VaStatus {
        for id in surface_ids {
            self.surfaces.remove(id);
        }
        VaStatus::Success
    }

    /// Create context
    pub fn create_context(&mut self, config_id: u32, width: u32, height: u32,
                          flags: u32, surfaces: &[u32]) -> Result<u32, VaStatus>
    {
        if !self.configs.contains_key(&config_id) {
            return Err(VaStatus::ErrorInvalidConfig);
        }

        let id = self.next_context_id;
        self.next_context_id += 1;

        let context = VaContext {
            id,
            config_id,
            picture_width: width,
            picture_height: height,
            surfaces: surfaces.to_vec(),
            flags,
        };

        self.contexts.insert(id, context);
        Ok(id)
    }

    /// Destroy context
    pub fn destroy_context(&mut self, context_id: u32) -> VaStatus {
        if self.contexts.remove(&context_id).is_some() {
            VaStatus::Success
        } else {
            VaStatus::ErrorInvalidContext
        }
    }

    /// Create buffer
    pub fn create_buffer(&mut self, context_id: u32, buffer_type: VaBufferType,
                         size: usize, num_elements: u32, data: Option<&[u8]>)
        -> Result<u32, VaStatus>
    {
        if !self.contexts.contains_key(&context_id) {
            return Err(VaStatus::ErrorInvalidContext);
        }

        let id = self.next_buffer_id;
        self.next_buffer_id += 1;

        let buffer_data = if let Some(d) = data {
            d.to_vec()
        } else {
            vec![0u8; size]
        };

        let buffer = VaBuffer {
            id,
            buffer_type,
            size,
            num_elements,
            data: buffer_data,
            mapped: false,
        };

        self.buffers.insert(id, buffer);
        Ok(id)
    }

    /// Map buffer
    pub fn map_buffer(&mut self, buffer_id: u32) -> Result<*mut u8, VaStatus> {
        if let Some(buffer) = self.buffers.get_mut(&buffer_id) {
            buffer.mapped = true;
            Ok(buffer.data.as_mut_ptr())
        } else {
            Err(VaStatus::ErrorInvalidBuffer)
        }
    }

    /// Unmap buffer
    pub fn unmap_buffer(&mut self, buffer_id: u32) -> VaStatus {
        if let Some(buffer) = self.buffers.get_mut(&buffer_id) {
            buffer.mapped = false;
            VaStatus::Success
        } else {
            VaStatus::ErrorInvalidBuffer
        }
    }

    /// Destroy buffer
    pub fn destroy_buffer(&mut self, buffer_id: u32) -> VaStatus {
        if self.buffers.remove(&buffer_id).is_some() {
            VaStatus::Success
        } else {
            VaStatus::ErrorInvalidBuffer
        }
    }

    /// Begin picture
    pub fn begin_picture(&mut self, context_id: u32, target_surface: u32) -> VaStatus {
        if !self.contexts.contains_key(&context_id) {
            return VaStatus::ErrorInvalidContext;
        }

        if let Some(surface) = self.surfaces.get_mut(&target_surface) {
            if surface.in_use {
                return VaStatus::ErrorSurfaceBusy;
            }
            surface.in_use = true;
            VaStatus::Success
        } else {
            VaStatus::ErrorInvalidSurface
        }
    }

    /// Render picture
    pub fn render_picture(&mut self, context_id: u32, buffers: &[u32]) -> VaStatus {
        if !self.contexts.contains_key(&context_id) {
            return VaStatus::ErrorInvalidContext;
        }

        for buf_id in buffers {
            if !self.buffers.contains_key(buf_id) {
                return VaStatus::ErrorInvalidBuffer;
            }
        }

        // In a real implementation, this would submit decode/encode commands
        // to the video engine

        VaStatus::Success
    }

    /// End picture
    pub fn end_picture(&mut self, context_id: u32) -> VaStatus {
        if !self.contexts.contains_key(&context_id) {
            return VaStatus::ErrorInvalidContext;
        }

        // In a real implementation, this would execute queued commands

        VaStatus::Success
    }

    /// Sync surface (wait for completion)
    pub fn sync_surface(&mut self, surface_id: u32) -> VaStatus {
        if let Some(surface) = self.surfaces.get_mut(&surface_id) {
            // In a real implementation, this would wait for HW completion
            surface.in_use = false;
            VaStatus::Success
        } else {
            VaStatus::ErrorInvalidSurface
        }
    }

    /// Query surface status
    pub fn query_surface_status(&self, surface_id: u32) -> Result<bool, VaStatus> {
        if let Some(surface) = self.surfaces.get(&surface_id) {
            Ok(!surface.in_use)
        } else {
            Err(VaStatus::ErrorInvalidSurface)
        }
    }

    /// Create image
    pub fn create_image(&mut self, format: u32, width: u32, height: u32)
        -> Result<u32, VaStatus>
    {
        let id = self.next_image_id;
        self.next_image_id += 1;

        // Calculate size based on format
        let (data_size, num_planes, pitches, offsets) = match format {
            fourcc::NV12 => {
                let y_size = width * height;
                let uv_size = width * height / 2;
                (y_size + uv_size, 2, [width, width, 0, 0], [0, y_size, 0, 0])
            }
            fourcc::P010 => {
                let y_size = width * height * 2;
                let uv_size = width * height;
                (y_size + uv_size, 2, [width * 2, width * 2, 0, 0], [0, y_size, 0, 0])
            }
            fourcc::RGBA | fourcc::BGRA | fourcc::ARGB | fourcc::ABGR => {
                let size = width * height * 4;
                (size, 1, [width * 4, 0, 0, 0], [0, 0, 0, 0])
            }
            _ => return Err(VaStatus::ErrorInvalidImageFormat),
        };

        // Create backing buffer
        let buf_id = self.create_buffer(1, VaBufferType::Image, data_size as usize, 1, None)?;

        let image = VaImage {
            id,
            format,
            width,
            height,
            data_size,
            num_planes,
            pitches,
            offsets,
            buf_id,
        };

        self.images.insert(id, image);
        Ok(id)
    }

    /// Destroy image
    pub fn destroy_image(&mut self, image_id: u32) -> VaStatus {
        if let Some(image) = self.images.remove(&image_id) {
            self.destroy_buffer(image.buf_id);
            VaStatus::Success
        } else {
            VaStatus::ErrorInvalidImage
        }
    }

    /// Get image (copy surface to image)
    pub fn get_image(&self, surface_id: u32, _image_id: u32) -> VaStatus {
        if !self.surfaces.contains_key(&surface_id) {
            return VaStatus::ErrorInvalidSurface;
        }

        // In a real implementation, this would copy decoded data
        VaStatus::Success
    }

    /// Put image (copy image to surface)
    pub fn put_image(&self, surface_id: u32, _image_id: u32) -> VaStatus {
        if !self.surfaces.contains_key(&surface_id) {
            return VaStatus::ErrorInvalidSurface;
        }

        // In a real implementation, this would upload data to surface
        VaStatus::Success
    }

    /// Query video processing pipeline
    pub fn query_video_proc_pipeline_caps(&self) -> &VppCapabilities {
        &self.vpp
    }

    /// Get status string
    pub fn get_status(&self) -> String {
        let mut status = String::new();

        status.push_str("VA-API Status:\n");
        status.push_str(&alloc::format!("  Vendor: {}\n", self.vendor_string));
        status.push_str(&alloc::format!("  Driver: {}\n", self.driver_version));
        status.push_str(&alloc::format!("  Version: {}.{}\n", self.major_version, self.minor_version));
        status.push_str(&alloc::format!("  Profiles: {}\n", self.profiles.len()));

        for cap in &self.profiles {
            let entrypoints: Vec<&str> = cap.entrypoints.iter().map(|e| e.name()).collect();
            status.push_str(&alloc::format!("    {}: {:?} ({}x{})\n",
                cap.profile.name(), entrypoints, cap.max_width, cap.max_height));
        }

        status.push_str(&alloc::format!("  Configs: {}\n", self.configs.len()));
        status.push_str(&alloc::format!("  Surfaces: {}\n", self.surfaces.len()));
        status.push_str(&alloc::format!("  Contexts: {}\n", self.contexts.len()));

        status.push_str("  VPP Capabilities:\n");
        status.push_str(&alloc::format!("    Deinterlacing: {}\n", self.vpp.deinterlacing));
        status.push_str(&alloc::format!("    Noise Reduction: {}\n", self.vpp.noise_reduction));
        status.push_str(&alloc::format!("    Sharpening: {}\n", self.vpp.sharpening));
        status.push_str(&alloc::format!("    HDR Tone Mapping: {}\n", self.vpp.hdr_tone_mapping));

        status
    }
}

/// Global VA-API display
static VA_DISPLAY: TicketSpinlock<Option<VaDisplay>> = TicketSpinlock::new(None);

/// Initialize VA-API
pub fn init(vendor_id: u16, device_id: u16, mmio_base: u64) -> VaStatus {
    let mut guard = VA_DISPLAY.lock();
    let mut display = VaDisplay::new();
    let status = display.init(vendor_id, device_id, mmio_base);
    if status.is_success() {
        *guard = Some(display);
    }
    status
}

/// Get VA-API display
pub fn get_display() -> Option<&'static TicketSpinlock<Option<VaDisplay>>> {
    Some(&VA_DISPLAY)
}
