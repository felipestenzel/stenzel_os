//! Direct3D 9 Compatibility Layer
//!
//! Basic implementation of DirectX 9 graphics API for Windows compatibility.
//! This provides essential D3D9 structures, interfaces, and functions.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, Ordering};

/// D3D9 result type
pub type D3DResult = i32;

/// Success codes
pub mod d3d_ok {
    pub const D3D_OK: i32 = 0;
    pub const S_OK: i32 = 0;
    pub const S_FALSE: i32 = 1;
}

/// Error codes
pub mod d3derr {
    pub const D3DERR_INVALIDCALL: i32 = -2005530516; // 0x8876086C
    pub const D3DERR_NOTAVAILABLE: i32 = -2005530518; // 0x8876086A
    pub const D3DERR_OUTOFVIDEOMEMORY: i32 = -2005532292; // 0x8876017C
    pub const D3DERR_INVALIDDEVICE: i32 = -2005530519; // 0x88760869
    pub const D3DERR_DEVICELOST: i32 = -2005530520; // 0x88760868
    pub const D3DERR_DEVICENOTRESET: i32 = -2005530519; // 0x88760869
    pub const D3DERR_NOTFOUND: i32 = -2005530522; // 0x88760866
    pub const D3DERR_MOREDATA: i32 = -2005530521; // 0x88760867
    pub const D3DERR_DEVICEREMOVED: i32 = -2005530512; // 0x88760870
    pub const D3DERR_DRIVERINTERNALERROR: i32 = -2005530585; // 0x88760827
    pub const D3DERR_WASSTILLDRAWING: i32 = -2005532132; // 0x8876021C
    pub const E_OUTOFMEMORY: i32 = -2147024882; // 0x8007000E
    pub const E_FAIL: i32 = -2147467259; // 0x80004005
}

/// D3D format enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum D3DFormat {
    Unknown = 0,
    R8G8B8 = 20,
    A8R8G8B8 = 21,
    X8R8G8B8 = 22,
    R5G6B5 = 23,
    X1R5G5B5 = 24,
    A1R5G5B5 = 25,
    A4R4G4B4 = 26,
    R3G3B2 = 27,
    A8 = 28,
    A8R3G3B2 = 29,
    X4R4G4B4 = 30,
    A2B10G10R10 = 31,
    A8B8G8R8 = 32,
    X8B8G8R8 = 33,
    G16R16 = 34,
    A2R10G10B10 = 35,
    A16B16G16R16 = 36,
    A8P8 = 40,
    P8 = 41,
    L8 = 50,
    A8L8 = 51,
    A4L4 = 52,
    V8U8 = 60,
    L6V5U5 = 61,
    X8L8V8U8 = 62,
    Q8W8V8U8 = 63,
    V16U16 = 64,
    A2W10V10U10 = 67,
    D16Lockable = 70,
    D32 = 71,
    D15S1 = 73,
    D24S8 = 75,
    D24X8 = 77,
    D24X4S4 = 79,
    D16 = 80,
    D32FLockable = 82,
    D24FS8 = 83,
    D32Lockable = 84,
    S8Lockable = 85,
    L16 = 81,
    VertexData = 100,
    Index16 = 101,
    Index32 = 102,
    Q16W16V16U16 = 110,
    R16F = 111,
    G16R16F = 112,
    A16B16G16R16F = 113,
    R32F = 114,
    G32R32F = 115,
    A32B32G32R32F = 116,
    CxV8U8 = 117,
    A1 = 118,
    A2B10G10R10Xr = 119,
    BinaryBuffer = 199,
    // DXT compressed formats
    Dxt1 = 0x31545844, // 'DXT1'
    Dxt2 = 0x32545844, // 'DXT2'
    Dxt3 = 0x33545844, // 'DXT3'
    Dxt4 = 0x34545844, // 'DXT4'
    Dxt5 = 0x35545844, // 'DXT5'
}

impl D3DFormat {
    pub fn bits_per_pixel(&self) -> u32 {
        match self {
            D3DFormat::A8R8G8B8 | D3DFormat::X8R8G8B8 | D3DFormat::A8B8G8R8 |
            D3DFormat::X8B8G8R8 | D3DFormat::D32 | D3DFormat::D24S8 |
            D3DFormat::D24X8 => 32,
            D3DFormat::R8G8B8 => 24,
            D3DFormat::R5G6B5 | D3DFormat::X1R5G5B5 | D3DFormat::A1R5G5B5 |
            D3DFormat::A4R4G4B4 | D3DFormat::D16 | D3DFormat::D15S1 => 16,
            D3DFormat::A8 | D3DFormat::L8 | D3DFormat::P8 | D3DFormat::R3G3B2 => 8,
            _ => 32,
        }
    }

    pub fn is_depth_format(&self) -> bool {
        matches!(self,
            D3DFormat::D16Lockable | D3DFormat::D32 | D3DFormat::D15S1 |
            D3DFormat::D24S8 | D3DFormat::D24X8 | D3DFormat::D24X4S4 |
            D3DFormat::D16 | D3DFormat::D32FLockable | D3DFormat::D24FS8
        )
    }
}

/// D3D device type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum D3DDevType {
    Hal = 1,
    Ref = 2,
    Sw = 3,
    NullRef = 4,
}

/// D3D resource type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum D3DResourceType {
    Surface = 1,
    Volume = 2,
    Texture = 3,
    VolumeTexture = 4,
    CubeTexture = 5,
    VertexBuffer = 6,
    IndexBuffer = 7,
}

/// D3D pool type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum D3DPool {
    Default = 0,
    Managed = 1,
    SystemMem = 2,
    Scratch = 3,
}

/// D3D multisample type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum D3DMultiSample {
    None = 0,
    NonMaskable = 1,
    Samples2 = 2,
    Samples3 = 3,
    Samples4 = 4,
    Samples5 = 5,
    Samples6 = 6,
    Samples7 = 7,
    Samples8 = 8,
    Samples9 = 9,
    Samples10 = 10,
    Samples11 = 11,
    Samples12 = 12,
    Samples13 = 13,
    Samples14 = 14,
    Samples15 = 15,
    Samples16 = 16,
}

/// D3D swap effect
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum D3DSwapEffect {
    Discard = 1,
    Flip = 2,
    Copy = 3,
    Overlay = 4,
    FlipEx = 5,
}

/// D3D primitive type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum D3DPrimitiveType {
    PointList = 1,
    LineList = 2,
    LineStrip = 3,
    TriangleList = 4,
    TriangleStrip = 5,
    TriangleFan = 6,
}

impl D3DPrimitiveType {
    pub fn vertex_count(&self, primitive_count: u32) -> u32 {
        match self {
            D3DPrimitiveType::PointList => primitive_count,
            D3DPrimitiveType::LineList => primitive_count * 2,
            D3DPrimitiveType::LineStrip => primitive_count + 1,
            D3DPrimitiveType::TriangleList => primitive_count * 3,
            D3DPrimitiveType::TriangleStrip | D3DPrimitiveType::TriangleFan => primitive_count + 2,
        }
    }
}

/// D3D transform state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum D3DTransformState {
    View = 2,
    Projection = 3,
    Texture0 = 16,
    Texture1 = 17,
    Texture2 = 18,
    Texture3 = 19,
    Texture4 = 20,
    Texture5 = 21,
    Texture6 = 22,
    Texture7 = 23,
    World = 256,
    World1 = 257,
    World2 = 258,
    World3 = 259,
}

/// D3D render state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum D3DRenderState {
    ZEnable = 7,
    FillMode = 8,
    ShadeMode = 9,
    ZWriteEnable = 14,
    AlphaTestEnable = 15,
    LastPixel = 16,
    SrcBlend = 19,
    DestBlend = 20,
    CullMode = 22,
    ZFunc = 23,
    AlphaRef = 24,
    AlphaFunc = 25,
    DitherEnable = 26,
    AlphaBlendEnable = 27,
    FogEnable = 28,
    SpecularEnable = 29,
    FogColor = 34,
    FogTableMode = 35,
    FogStart = 36,
    FogEnd = 37,
    FogDensity = 38,
    RangeFogEnable = 48,
    StencilEnable = 52,
    StencilFail = 53,
    StencilZFail = 54,
    StencilPass = 55,
    StencilFunc = 56,
    StencilRef = 57,
    StencilMask = 58,
    StencilWriteMask = 59,
    TextureFactor = 60,
    Wrap0 = 128,
    Wrap1 = 129,
    Wrap2 = 130,
    Wrap3 = 131,
    Clipping = 136,
    Lighting = 137,
    Ambient = 139,
    FogVertexMode = 140,
    ColorVertex = 141,
    LocalViewer = 142,
    NormalizeNormals = 143,
    DiffuseMaterialSource = 145,
    SpecularMaterialSource = 146,
    AmbientMaterialSource = 147,
    EmissiveMaterialSource = 148,
    VertexBlend = 151,
    ClipPlaneEnable = 152,
    PointSize = 154,
    PointSizeMin = 155,
    PointSpriteEnable = 156,
    PointScaleEnable = 157,
    PointScaleA = 158,
    PointScaleB = 159,
    PointScaleC = 160,
    MultisampleAntialias = 161,
    MultisampleMask = 162,
    PatchEdgeStyle = 163,
    DebugMonitorToken = 165,
    PointSizeMax = 166,
    IndexedVertexBlendEnable = 167,
    ColorWriteEnable = 168,
    TweenFactor = 170,
    BlendOp = 171,
    PositionDegree = 172,
    NormalDegree = 173,
    ScissorTestEnable = 174,
    SlopeScaleDepthBias = 175,
    AntialiasedLineEnable = 176,
    MinTessellationLevel = 178,
    MaxTessellationLevel = 179,
    AdaptiveTessX = 180,
    AdaptiveTessY = 181,
    AdaptiveTessZ = 182,
    AdaptiveTessW = 183,
    EnableAdaptiveTessellation = 184,
    TwoSidedStencilMode = 185,
    CcwStencilFail = 186,
    CcwStencilZFail = 187,
    CcwStencilPass = 188,
    CcwStencilFunc = 189,
    ColorWriteEnable1 = 190,
    ColorWriteEnable2 = 191,
    ColorWriteEnable3 = 192,
    BlendFactor = 193,
    SrgbWriteEnable = 194,
    DepthBias = 195,
    Wrap8 = 198,
    Wrap9 = 199,
    Wrap10 = 200,
    Wrap11 = 201,
    Wrap12 = 202,
    Wrap13 = 203,
    Wrap14 = 204,
    Wrap15 = 205,
    SeparateAlphaBlendEnable = 206,
    SrcBlendAlpha = 207,
    DestBlendAlpha = 208,
    BlendOpAlpha = 209,
}

/// D3D texture stage state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum D3DTextureStageState {
    ColorOp = 1,
    ColorArg1 = 2,
    ColorArg2 = 3,
    AlphaOp = 4,
    AlphaArg1 = 5,
    AlphaArg2 = 6,
    BumpEnvMat00 = 7,
    BumpEnvMat01 = 8,
    BumpEnvMat10 = 9,
    BumpEnvMat11 = 10,
    TexCoordIndex = 11,
    BumpEnvLScale = 22,
    BumpEnvLOffset = 23,
    TextureTransformFlags = 24,
    ColorArg0 = 26,
    AlphaArg0 = 27,
    ResultArg = 28,
    Constant = 32,
}

/// D3D sampler state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum D3DSamplerState {
    AddressU = 1,
    AddressV = 2,
    AddressW = 3,
    BorderColor = 4,
    MagFilter = 5,
    MinFilter = 6,
    MipFilter = 7,
    MipMapLodBias = 8,
    MaxMipLevel = 9,
    MaxAnisotropy = 10,
    SrgbTexture = 11,
    ElementIndex = 12,
    DMapOffset = 13,
}

/// D3D FVF (Flexible Vertex Format) flags
pub mod d3dfvf {
    pub const XYZ: u32 = 0x002;
    pub const XYZRHW: u32 = 0x004;
    pub const XYZB1: u32 = 0x006;
    pub const XYZB2: u32 = 0x008;
    pub const XYZB3: u32 = 0x00A;
    pub const XYZB4: u32 = 0x00C;
    pub const XYZB5: u32 = 0x00E;
    pub const XYZW: u32 = 0x4002;
    pub const NORMAL: u32 = 0x010;
    pub const PSIZE: u32 = 0x020;
    pub const DIFFUSE: u32 = 0x040;
    pub const SPECULAR: u32 = 0x080;
    pub const TEX0: u32 = 0x000;
    pub const TEX1: u32 = 0x100;
    pub const TEX2: u32 = 0x200;
    pub const TEX3: u32 = 0x300;
    pub const TEX4: u32 = 0x400;
    pub const TEX5: u32 = 0x500;
    pub const TEX6: u32 = 0x600;
    pub const TEX7: u32 = 0x700;
    pub const TEX8: u32 = 0x800;
    pub const LASTBETA_UBYTE4: u32 = 0x1000;
    pub const LASTBETA_D3DCOLOR: u32 = 0x8000;

    pub fn vertex_size(fvf: u32) -> u32 {
        let mut size = 0u32;

        // Position
        let pos_mask = fvf & 0x00E;
        if pos_mask == XYZ || pos_mask == XYZRHW {
            size += 12; // 3 floats
            if pos_mask == XYZRHW {
                size += 4; // RHW
            }
        }

        // Normal
        if fvf & NORMAL != 0 {
            size += 12;
        }

        // Point size
        if fvf & PSIZE != 0 {
            size += 4;
        }

        // Diffuse
        if fvf & DIFFUSE != 0 {
            size += 4;
        }

        // Specular
        if fvf & SPECULAR != 0 {
            size += 4;
        }

        // Texture coordinates
        let tex_count = (fvf >> 8) & 0xF;
        size += tex_count * 8; // 2 floats per texcoord

        size
    }
}

/// D3D clear flags
pub mod d3dclear {
    pub const TARGET: u32 = 1;
    pub const ZBUFFER: u32 = 2;
    pub const STENCIL: u32 = 4;
}

/// D3D usage flags
pub mod d3dusage {
    pub const RENDERTARGET: u32 = 0x00000001;
    pub const DEPTHSTENCIL: u32 = 0x00000002;
    pub const DYNAMIC: u32 = 0x00000200;
    pub const AUTOGENMIPMAP: u32 = 0x00000400;
    pub const DMAP: u32 = 0x00004000;
    pub const QUERY_LEGACYBUMPMAP: u32 = 0x00008000;
    pub const WRITEONLY: u32 = 0x00000008;
    pub const SOFTWAREPROCESSING: u32 = 0x00000010;
    pub const DONOTCLIP: u32 = 0x00000020;
    pub const POINTS: u32 = 0x00000040;
    pub const RTPATCHES: u32 = 0x00000080;
    pub const NPATCHES: u32 = 0x00000100;
}

/// D3D lock flags
pub mod d3dlock {
    pub const READONLY: u32 = 0x00000010;
    pub const DISCARD: u32 = 0x00002000;
    pub const NOOVERWRITE: u32 = 0x00001000;
    pub const NOSYSLOCK: u32 = 0x00000800;
    pub const DONOTWAIT: u32 = 0x00004000;
    pub const NO_DIRTY_UPDATE: u32 = 0x00008000;
}

/// Present parameters
#[derive(Debug, Clone)]
#[repr(C)]
pub struct D3DPresentParameters {
    pub back_buffer_width: u32,
    pub back_buffer_height: u32,
    pub back_buffer_format: D3DFormat,
    pub back_buffer_count: u32,
    pub multi_sample_type: D3DMultiSample,
    pub multi_sample_quality: u32,
    pub swap_effect: D3DSwapEffect,
    pub device_window: u64, // HWND
    pub windowed: bool,
    pub enable_auto_depth_stencil: bool,
    pub auto_depth_stencil_format: D3DFormat,
    pub flags: u32,
    pub fullscreen_refresh_rate_hz: u32,
    pub presentation_interval: u32,
}

impl Default for D3DPresentParameters {
    fn default() -> Self {
        Self {
            back_buffer_width: 800,
            back_buffer_height: 600,
            back_buffer_format: D3DFormat::X8R8G8B8,
            back_buffer_count: 1,
            multi_sample_type: D3DMultiSample::None,
            multi_sample_quality: 0,
            swap_effect: D3DSwapEffect::Discard,
            device_window: 0,
            windowed: true,
            enable_auto_depth_stencil: true,
            auto_depth_stencil_format: D3DFormat::D24S8,
            flags: 0,
            fullscreen_refresh_rate_hz: 0,
            presentation_interval: 1,
        }
    }
}

/// 4x4 Matrix
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct D3DMatrix {
    pub m: [[f32; 4]; 4],
}

impl Default for D3DMatrix {
    fn default() -> Self {
        Self::identity()
    }
}

impl D3DMatrix {
    pub fn identity() -> Self {
        Self {
            m: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    pub fn zero() -> Self {
        Self {
            m: [[0.0; 4]; 4],
        }
    }
}

/// Viewport
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct D3DViewport9 {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub min_z: f32,
    pub max_z: f32,
}

impl Default for D3DViewport9 {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            width: 800,
            height: 600,
            min_z: 0.0,
            max_z: 1.0,
        }
    }
}

/// Material
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct D3DMaterial9 {
    pub diffuse: D3DColorValue,
    pub ambient: D3DColorValue,
    pub specular: D3DColorValue,
    pub emissive: D3DColorValue,
    pub power: f32,
}

/// Color value (RGBA float)
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct D3DColorValue {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl D3DColorValue {
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub fn from_argb(argb: u32) -> Self {
        Self {
            a: ((argb >> 24) & 0xFF) as f32 / 255.0,
            r: ((argb >> 16) & 0xFF) as f32 / 255.0,
            g: ((argb >> 8) & 0xFF) as f32 / 255.0,
            b: (argb & 0xFF) as f32 / 255.0,
        }
    }

    pub fn to_argb(&self) -> u32 {
        let a = (self.a * 255.0) as u32;
        let r = (self.r * 255.0) as u32;
        let g = (self.g * 255.0) as u32;
        let b = (self.b * 255.0) as u32;
        (a << 24) | (r << 16) | (g << 8) | b
    }
}

/// Light
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct D3DLight9 {
    pub light_type: D3DLightType,
    pub diffuse: D3DColorValue,
    pub specular: D3DColorValue,
    pub ambient: D3DColorValue,
    pub position: D3DVector,
    pub direction: D3DVector,
    pub range: f32,
    pub falloff: f32,
    pub attenuation0: f32,
    pub attenuation1: f32,
    pub attenuation2: f32,
    pub theta: f32,
    pub phi: f32,
}

/// Light type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum D3DLightType {
    Point = 1,
    Spot = 2,
    Directional = 3,
}

/// 3D Vector
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct D3DVector {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// Locked rectangle (for texture locking)
#[derive(Debug, Clone)]
#[repr(C)]
pub struct D3DLockedRect {
    pub pitch: i32,
    pub bits: u64, // Pointer to data
}

/// Surface description
#[derive(Debug, Clone)]
#[repr(C)]
pub struct D3DSurfaceDesc {
    pub format: D3DFormat,
    pub resource_type: D3DResourceType,
    pub usage: u32,
    pub pool: D3DPool,
    pub multi_sample_type: D3DMultiSample,
    pub multi_sample_quality: u32,
    pub width: u32,
    pub height: u32,
}

/// Resource handle counter
static NEXT_RESOURCE_HANDLE: AtomicU32 = AtomicU32::new(1);

fn next_resource_handle() -> u32 {
    NEXT_RESOURCE_HANDLE.fetch_add(1, Ordering::Relaxed)
}

/// D3D9 Texture resource
#[derive(Clone)]
pub struct D3D9Texture {
    pub handle: u32,
    pub width: u32,
    pub height: u32,
    pub levels: u32,
    pub format: D3DFormat,
    pub pool: D3DPool,
    pub usage: u32,
    pub data: Vec<Vec<u8>>, // Mip levels
}

impl D3D9Texture {
    pub fn new(width: u32, height: u32, levels: u32, format: D3DFormat, pool: D3DPool) -> Self {
        let actual_levels = if levels == 0 {
            // Calculate mipmap levels
            let max_dim = width.max(height);
            (32 - max_dim.leading_zeros()) as u32
        } else {
            levels
        };

        let mut data = Vec::with_capacity(actual_levels as usize);
        let bpp = format.bits_per_pixel();
        let mut w = width;
        let mut h = height;

        for _ in 0..actual_levels {
            let size = (w * h * bpp / 8) as usize;
            data.push(alloc::vec![0u8; size]);
            w = (w / 2).max(1);
            h = (h / 2).max(1);
        }

        Self {
            handle: next_resource_handle(),
            width,
            height,
            levels: actual_levels,
            format,
            pool,
            usage: 0,
            data,
        }
    }
}

/// D3D9 Vertex Buffer
#[derive(Clone)]
pub struct D3D9VertexBuffer {
    pub handle: u32,
    pub size: u32,
    pub fvf: u32,
    pub pool: D3DPool,
    pub usage: u32,
    pub data: Vec<u8>,
    pub locked: bool,
}

impl D3D9VertexBuffer {
    pub fn new(size: u32, fvf: u32, pool: D3DPool, usage: u32) -> Self {
        Self {
            handle: next_resource_handle(),
            size,
            fvf,
            pool,
            usage,
            data: alloc::vec![0u8; size as usize],
            locked: false,
        }
    }
}

/// D3D9 Index Buffer
#[derive(Clone)]
pub struct D3D9IndexBuffer {
    pub handle: u32,
    pub size: u32,
    pub format: D3DFormat,
    pub pool: D3DPool,
    pub usage: u32,
    pub data: Vec<u8>,
    pub locked: bool,
}

impl D3D9IndexBuffer {
    pub fn new(size: u32, format: D3DFormat, pool: D3DPool, usage: u32) -> Self {
        Self {
            handle: next_resource_handle(),
            size,
            format,
            pool,
            usage,
            data: alloc::vec![0u8; size as usize],
            locked: false,
        }
    }
}

/// D3D9 Device state
pub struct D3D9DeviceState {
    /// Render states
    pub render_states: BTreeMap<u32, u32>,
    /// Texture stage states
    pub texture_stage_states: [[u32; 33]; 8],
    /// Sampler states
    pub sampler_states: [[u32; 14]; 16],
    /// Transforms
    pub transforms: BTreeMap<u32, D3DMatrix>,
    /// Current viewport
    pub viewport: D3DViewport9,
    /// Current material
    pub material: D3DMaterial9,
    /// Lights
    pub lights: BTreeMap<u32, D3DLight9>,
    /// Light enabled flags
    pub lights_enabled: BTreeMap<u32, bool>,
    /// Current FVF
    pub fvf: u32,
    /// Vertex declaration (for shaders)
    pub vertex_decl: Option<u32>,
    /// Current vertex shader
    pub vertex_shader: Option<u32>,
    /// Current pixel shader
    pub pixel_shader: Option<u32>,
    /// Stream sources
    pub stream_sources: [(Option<u32>, u32, u32); 16], // (vb handle, offset, stride)
    /// Indices
    pub indices: Option<u32>,
    /// Textures
    pub textures: [Option<u32>; 8],
    /// Scissor rect
    pub scissor_rect: (i32, i32, i32, i32),
    /// Clip planes
    pub clip_planes: [D3DVector; 6],
}

impl Default for D3D9DeviceState {
    fn default() -> Self {
        Self {
            render_states: BTreeMap::new(),
            texture_stage_states: [[0; 33]; 8],
            sampler_states: [[0; 14]; 16],
            transforms: BTreeMap::new(),
            viewport: D3DViewport9::default(),
            material: D3DMaterial9::default(),
            lights: BTreeMap::new(),
            lights_enabled: BTreeMap::new(),
            fvf: 0,
            vertex_decl: None,
            vertex_shader: None,
            pixel_shader: None,
            stream_sources: [(None, 0, 0); 16],
            indices: None,
            textures: [None; 8],
            scissor_rect: (0, 0, 0, 0),
            clip_planes: [D3DVector::default(); 6],
        }
    }
}

/// D3D9 Device
pub struct D3D9Device {
    /// Device handle
    pub handle: u32,
    /// Present parameters
    pub present_params: D3DPresentParameters,
    /// Device state
    pub state: D3D9DeviceState,
    /// Textures
    pub textures: BTreeMap<u32, D3D9Texture>,
    /// Vertex buffers
    pub vertex_buffers: BTreeMap<u32, D3D9VertexBuffer>,
    /// Index buffers
    pub index_buffers: BTreeMap<u32, D3D9IndexBuffer>,
    /// In scene
    pub in_scene: bool,
    /// Device lost
    pub device_lost: bool,
    /// Frame count
    pub frame_count: u64,
}

impl D3D9Device {
    pub fn new(present_params: D3DPresentParameters) -> Self {
        let mut state = D3D9DeviceState::default();

        // Set default render states
        state.render_states.insert(D3DRenderState::ZEnable as u32, 1);
        state.render_states.insert(D3DRenderState::FillMode as u32, 3); // Solid
        state.render_states.insert(D3DRenderState::ShadeMode as u32, 2); // Gouraud
        state.render_states.insert(D3DRenderState::ZWriteEnable as u32, 1);
        state.render_states.insert(D3DRenderState::CullMode as u32, 3); // CCW
        state.render_states.insert(D3DRenderState::Lighting as u32, 1);

        // Set identity transforms
        state.transforms.insert(D3DTransformState::World as u32, D3DMatrix::identity());
        state.transforms.insert(D3DTransformState::View as u32, D3DMatrix::identity());
        state.transforms.insert(D3DTransformState::Projection as u32, D3DMatrix::identity());

        Self {
            handle: next_resource_handle(),
            present_params,
            state,
            textures: BTreeMap::new(),
            vertex_buffers: BTreeMap::new(),
            index_buffers: BTreeMap::new(),
            in_scene: false,
            device_lost: false,
            frame_count: 0,
        }
    }

    /// Begin scene
    pub fn begin_scene(&mut self) -> D3DResult {
        if self.in_scene {
            return d3derr::D3DERR_INVALIDCALL;
        }
        self.in_scene = true;
        d3d_ok::D3D_OK
    }

    /// End scene
    pub fn end_scene(&mut self) -> D3DResult {
        if !self.in_scene {
            return d3derr::D3DERR_INVALIDCALL;
        }
        self.in_scene = false;
        d3d_ok::D3D_OK
    }

    /// Present (flip buffers)
    pub fn present(&mut self) -> D3DResult {
        self.frame_count += 1;
        d3d_ok::D3D_OK
    }

    /// Clear render target
    pub fn clear(&mut self, flags: u32, color: u32, z: f32, stencil: u32) -> D3DResult {
        // In real implementation, this would clear framebuffer
        let _ = (flags, color, z, stencil);
        d3d_ok::D3D_OK
    }

    /// Set render state
    pub fn set_render_state(&mut self, state: D3DRenderState, value: u32) -> D3DResult {
        self.state.render_states.insert(state as u32, value);
        d3d_ok::D3D_OK
    }

    /// Get render state
    pub fn get_render_state(&self, state: D3DRenderState) -> u32 {
        *self.state.render_states.get(&(state as u32)).unwrap_or(&0)
    }

    /// Set transform
    pub fn set_transform(&mut self, state: D3DTransformState, matrix: &D3DMatrix) -> D3DResult {
        self.state.transforms.insert(state as u32, *matrix);
        d3d_ok::D3D_OK
    }

    /// Get transform
    pub fn get_transform(&self, state: D3DTransformState) -> D3DMatrix {
        self.state.transforms.get(&(state as u32)).cloned().unwrap_or_default()
    }

    /// Set viewport
    pub fn set_viewport(&mut self, viewport: &D3DViewport9) -> D3DResult {
        self.state.viewport = *viewport;
        d3d_ok::D3D_OK
    }

    /// Get viewport
    pub fn get_viewport(&self) -> D3DViewport9 {
        self.state.viewport
    }

    /// Set FVF
    pub fn set_fvf(&mut self, fvf: u32) -> D3DResult {
        self.state.fvf = fvf;
        d3d_ok::D3D_OK
    }

    /// Create texture
    pub fn create_texture(
        &mut self,
        width: u32,
        height: u32,
        levels: u32,
        usage: u32,
        format: D3DFormat,
        pool: D3DPool,
    ) -> Result<u32, D3DResult> {
        let mut texture = D3D9Texture::new(width, height, levels, format, pool);
        texture.usage = usage;
        let handle = texture.handle;
        self.textures.insert(handle, texture);
        Ok(handle)
    }

    /// Set texture
    pub fn set_texture(&mut self, stage: u32, handle: Option<u32>) -> D3DResult {
        if stage >= 8 {
            return d3derr::D3DERR_INVALIDCALL;
        }
        self.state.textures[stage as usize] = handle;
        d3d_ok::D3D_OK
    }

    /// Create vertex buffer
    pub fn create_vertex_buffer(
        &mut self,
        length: u32,
        usage: u32,
        fvf: u32,
        pool: D3DPool,
    ) -> Result<u32, D3DResult> {
        let vb = D3D9VertexBuffer::new(length, fvf, pool, usage);
        let handle = vb.handle;
        self.vertex_buffers.insert(handle, vb);
        Ok(handle)
    }

    /// Create index buffer
    pub fn create_index_buffer(
        &mut self,
        length: u32,
        usage: u32,
        format: D3DFormat,
        pool: D3DPool,
    ) -> Result<u32, D3DResult> {
        let ib = D3D9IndexBuffer::new(length, format, pool, usage);
        let handle = ib.handle;
        self.index_buffers.insert(handle, ib);
        Ok(handle)
    }

    /// Set stream source
    pub fn set_stream_source(
        &mut self,
        stream: u32,
        vb_handle: Option<u32>,
        offset: u32,
        stride: u32,
    ) -> D3DResult {
        if stream >= 16 {
            return d3derr::D3DERR_INVALIDCALL;
        }
        self.state.stream_sources[stream as usize] = (vb_handle, offset, stride);
        d3d_ok::D3D_OK
    }

    /// Set indices
    pub fn set_indices(&mut self, ib_handle: Option<u32>) -> D3DResult {
        self.state.indices = ib_handle;
        d3d_ok::D3D_OK
    }

    /// Draw primitive
    pub fn draw_primitive(
        &mut self,
        primitive_type: D3DPrimitiveType,
        start_vertex: u32,
        primitive_count: u32,
    ) -> D3DResult {
        if !self.in_scene {
            return d3derr::D3DERR_INVALIDCALL;
        }

        // In real implementation, this would render primitives
        let _vertex_count = primitive_type.vertex_count(primitive_count);
        let _ = start_vertex;

        d3d_ok::D3D_OK
    }

    /// Draw indexed primitive
    pub fn draw_indexed_primitive(
        &mut self,
        primitive_type: D3DPrimitiveType,
        base_vertex_index: i32,
        min_vertex_index: u32,
        num_vertices: u32,
        start_index: u32,
        primitive_count: u32,
    ) -> D3DResult {
        if !self.in_scene {
            return d3derr::D3DERR_INVALIDCALL;
        }

        // In real implementation, this would render indexed primitives
        let _ = (base_vertex_index, min_vertex_index, num_vertices, start_index);
        let _ = primitive_type.vertex_count(primitive_count);

        d3d_ok::D3D_OK
    }

    /// Draw primitive UP (user pointer)
    pub fn draw_primitive_up(
        &mut self,
        primitive_type: D3DPrimitiveType,
        primitive_count: u32,
        _vertex_data: &[u8],
        _vertex_stride: u32,
    ) -> D3DResult {
        if !self.in_scene {
            return d3derr::D3DERR_INVALIDCALL;
        }

        let _ = primitive_type.vertex_count(primitive_count);

        d3d_ok::D3D_OK
    }

    /// Set material
    pub fn set_material(&mut self, material: &D3DMaterial9) -> D3DResult {
        self.state.material = *material;
        d3d_ok::D3D_OK
    }

    /// Set light
    pub fn set_light(&mut self, index: u32, light: &D3DLight9) -> D3DResult {
        self.state.lights.insert(index, *light);
        d3d_ok::D3D_OK
    }

    /// Light enable
    pub fn light_enable(&mut self, index: u32, enable: bool) -> D3DResult {
        self.state.lights_enabled.insert(index, enable);
        d3d_ok::D3D_OK
    }

    /// Reset device
    pub fn reset(&mut self, present_params: &D3DPresentParameters) -> D3DResult {
        self.present_params = present_params.clone();
        self.device_lost = false;
        d3d_ok::D3D_OK
    }

    /// Test cooperative level
    pub fn test_cooperative_level(&self) -> D3DResult {
        if self.device_lost {
            d3derr::D3DERR_DEVICELOST
        } else {
            d3d_ok::D3D_OK
        }
    }
}

/// D3D9 Interface (IDirect3D9)
pub struct Direct3D9 {
    /// Adapter count
    pub adapter_count: u32,
    /// Devices created
    pub devices: Vec<D3D9Device>,
}

impl Direct3D9 {
    pub fn new() -> Self {
        Self {
            adapter_count: 1,
            devices: Vec::new(),
        }
    }

    /// Get adapter count
    pub fn get_adapter_count(&self) -> u32 {
        self.adapter_count
    }

    /// Get adapter identifier
    pub fn get_adapter_identifier(&self, _adapter: u32) -> D3DAdapterIdentifier {
        D3DAdapterIdentifier {
            driver: String::from("Stenzel OS D3D9"),
            description: String::from("Stenzel OS Direct3D 9 Compatibility Layer"),
            device_name: String::from("\\\\.\\DISPLAY1"),
            driver_version: 0x0009_0000_0000_0001,
            vendor_id: 0x1002, // AMD-like
            device_id: 0x67B1, // Generic
            subsys_id: 0,
            revision: 1,
            device_identifier: [0; 16],
            whql_level: 1,
        }
    }

    /// Check device type
    pub fn check_device_type(
        &self,
        _adapter: u32,
        dev_type: D3DDevType,
        _adapter_format: D3DFormat,
        _back_buffer_format: D3DFormat,
        _windowed: bool,
    ) -> D3DResult {
        match dev_type {
            D3DDevType::Hal => d3d_ok::D3D_OK,
            D3DDevType::Ref => d3d_ok::D3D_OK,
            _ => d3derr::D3DERR_NOTAVAILABLE,
        }
    }

    /// Create device
    pub fn create_device(
        &mut self,
        _adapter: u32,
        _dev_type: D3DDevType,
        _focus_window: u64,
        _behavior_flags: u32,
        present_params: D3DPresentParameters,
    ) -> Result<usize, D3DResult> {
        let device = D3D9Device::new(present_params);
        let index = self.devices.len();
        self.devices.push(device);
        Ok(index)
    }

    /// Get device
    pub fn get_device(&mut self, index: usize) -> Option<&mut D3D9Device> {
        self.devices.get_mut(index)
    }
}

impl Default for Direct3D9 {
    fn default() -> Self {
        Self::new()
    }
}

/// Adapter identifier
#[derive(Debug, Clone)]
pub struct D3DAdapterIdentifier {
    pub driver: String,
    pub description: String,
    pub device_name: String,
    pub driver_version: u64,
    pub vendor_id: u32,
    pub device_id: u32,
    pub subsys_id: u32,
    pub revision: u32,
    pub device_identifier: [u8; 16],
    pub whql_level: u32,
}

/// Global D3D9 instance
static mut D3D9_INSTANCE: Option<Direct3D9> = None;

/// Create Direct3D9 instance (Direct3DCreate9 equivalent)
pub fn direct3d_create9(sdk_version: u32) -> Option<&'static mut Direct3D9> {
    // SDK version 32 = D3D_SDK_VERSION for D3D9
    if sdk_version != 32 {
        crate::kprintln!("d3d9: Warning: SDK version {} (expected 32)", sdk_version);
    }

    unsafe {
        if D3D9_INSTANCE.is_none() {
            D3D9_INSTANCE = Some(Direct3D9::new());
        }
        D3D9_INSTANCE.as_mut()
    }
}

/// Initialize D3D9 subsystem
pub fn init() {
    crate::kprintln!("d3d9: Direct3D 9 compatibility layer initialized");
}

/// Format status
pub fn format_status() -> String {
    unsafe {
        if let Some(ref d3d9) = D3D9_INSTANCE {
            alloc::format!(
                "Direct3D 9:\n  Adapters: {}\n  Devices: {}",
                d3d9.adapter_count,
                d3d9.devices.len()
            )
        } else {
            String::from("Direct3D 9: Not initialized")
        }
    }
}
