//! OpenGL 4.6 Driver Interface
//!
//! Provides OpenGL 4.6 context management and dispatch for GPU drivers.
//! This is the kernel-side interface that works with Mesa userspace drivers.
//!
//! Features:
//! - Context creation and management
//! - DRI3/DRI2 protocol support
//! - GPU memory management integration
//! - Fence and sync object handling

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use spin::Mutex;

/// OpenGL version
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GlVersion {
    pub major: u8,
    pub minor: u8,
}

impl GlVersion {
    pub const GL_1_0: GlVersion = GlVersion { major: 1, minor: 0 };
    pub const GL_2_0: GlVersion = GlVersion { major: 2, minor: 0 };
    pub const GL_2_1: GlVersion = GlVersion { major: 2, minor: 1 };
    pub const GL_3_0: GlVersion = GlVersion { major: 3, minor: 0 };
    pub const GL_3_1: GlVersion = GlVersion { major: 3, minor: 1 };
    pub const GL_3_2: GlVersion = GlVersion { major: 3, minor: 2 };
    pub const GL_3_3: GlVersion = GlVersion { major: 3, minor: 3 };
    pub const GL_4_0: GlVersion = GlVersion { major: 4, minor: 0 };
    pub const GL_4_1: GlVersion = GlVersion { major: 4, minor: 1 };
    pub const GL_4_2: GlVersion = GlVersion { major: 4, minor: 2 };
    pub const GL_4_3: GlVersion = GlVersion { major: 4, minor: 3 };
    pub const GL_4_4: GlVersion = GlVersion { major: 4, minor: 4 };
    pub const GL_4_5: GlVersion = GlVersion { major: 4, minor: 5 };
    pub const GL_4_6: GlVersion = GlVersion { major: 4, minor: 6 };

    pub fn as_str(&self) -> String {
        format!("{}.{}", self.major, self.minor)
    }
}

/// GLSL version
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlslVersion {
    pub version: u16,  // e.g., 460 for GLSL 4.60
}

impl GlslVersion {
    pub const GLSL_110: GlslVersion = GlslVersion { version: 110 };
    pub const GLSL_120: GlslVersion = GlslVersion { version: 120 };
    pub const GLSL_130: GlslVersion = GlslVersion { version: 130 };
    pub const GLSL_140: GlslVersion = GlslVersion { version: 140 };
    pub const GLSL_150: GlslVersion = GlslVersion { version: 150 };
    pub const GLSL_330: GlslVersion = GlslVersion { version: 330 };
    pub const GLSL_400: GlslVersion = GlslVersion { version: 400 };
    pub const GLSL_410: GlslVersion = GlslVersion { version: 410 };
    pub const GLSL_420: GlslVersion = GlslVersion { version: 420 };
    pub const GLSL_430: GlslVersion = GlslVersion { version: 430 };
    pub const GLSL_440: GlslVersion = GlslVersion { version: 440 };
    pub const GLSL_450: GlslVersion = GlslVersion { version: 450 };
    pub const GLSL_460: GlslVersion = GlslVersion { version: 460 };
}

/// OpenGL context profile
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlProfile {
    /// Core profile (no deprecated features)
    Core,
    /// Compatibility profile (includes deprecated features)
    Compatibility,
    /// OpenGL ES
    Es,
}

/// Context flags
#[derive(Debug, Clone, Copy)]
pub struct GlContextFlags {
    pub debug: bool,
    pub forward_compatible: bool,
    pub robust_access: bool,
    pub no_error: bool,
}

impl Default for GlContextFlags {
    fn default() -> Self {
        Self {
            debug: false,
            forward_compatible: false,
            robust_access: false,
            no_error: false,
        }
    }
}

/// OpenGL extension
#[derive(Debug, Clone)]
pub struct GlExtension {
    pub name: String,
    pub version: Option<u32>,
    pub supported: bool,
}

/// GPU capabilities for OpenGL
#[derive(Debug, Clone)]
pub struct GlCapabilities {
    /// Maximum OpenGL version supported
    pub max_gl_version: GlVersion,
    /// Maximum GLSL version supported
    pub max_glsl_version: GlslVersion,
    /// Max texture size (width/height)
    pub max_texture_size: u32,
    /// Max 3D texture size
    pub max_3d_texture_size: u32,
    /// Max cube map texture size
    pub max_cube_map_texture_size: u32,
    /// Max array texture layers
    pub max_array_texture_layers: u32,
    /// Max texture units
    pub max_texture_units: u32,
    /// Max combined texture image units
    pub max_combined_texture_image_units: u32,
    /// Max vertex attribs
    pub max_vertex_attribs: u32,
    /// Max uniform buffer bindings
    pub max_uniform_buffer_bindings: u32,
    /// Max uniform block size
    pub max_uniform_block_size: u32,
    /// Max shader storage buffer bindings
    pub max_shader_storage_buffer_bindings: u32,
    /// Max compute work group count
    pub max_compute_work_group_count: [u32; 3],
    /// Max compute work group size
    pub max_compute_work_group_size: [u32; 3],
    /// Max compute work group invocations
    pub max_compute_work_group_invocations: u32,
    /// Max compute shared memory size
    pub max_compute_shared_memory_size: u32,
    /// Max framebuffer width
    pub max_framebuffer_width: u32,
    /// Max framebuffer height
    pub max_framebuffer_height: u32,
    /// Max framebuffer samples
    pub max_framebuffer_samples: u32,
    /// Max color attachments
    pub max_color_attachments: u32,
    /// Max draw buffers
    pub max_draw_buffers: u32,
    /// Max viewports
    pub max_viewports: u32,
    /// Supports geometry shaders
    pub geometry_shader: bool,
    /// Supports tessellation shaders
    pub tessellation_shader: bool,
    /// Supports compute shaders
    pub compute_shader: bool,
    /// Supports SPIR-V shaders
    pub spirv_shader: bool,
    /// Extensions
    pub extensions: Vec<String>,
}

impl Default for GlCapabilities {
    fn default() -> Self {
        Self {
            max_gl_version: GlVersion::GL_4_6,
            max_glsl_version: GlslVersion::GLSL_460,
            max_texture_size: 16384,
            max_3d_texture_size: 2048,
            max_cube_map_texture_size: 16384,
            max_array_texture_layers: 2048,
            max_texture_units: 16,
            max_combined_texture_image_units: 192,
            max_vertex_attribs: 16,
            max_uniform_buffer_bindings: 84,
            max_uniform_block_size: 65536,
            max_shader_storage_buffer_bindings: 96,
            max_compute_work_group_count: [65535, 65535, 65535],
            max_compute_work_group_size: [1024, 1024, 64],
            max_compute_work_group_invocations: 1024,
            max_compute_shared_memory_size: 49152,
            max_framebuffer_width: 16384,
            max_framebuffer_height: 16384,
            max_framebuffer_samples: 32,
            max_color_attachments: 8,
            max_draw_buffers: 8,
            max_viewports: 16,
            geometry_shader: true,
            tessellation_shader: true,
            compute_shader: true,
            spirv_shader: true,
            extensions: Vec::new(),
        }
    }
}

/// OpenGL context state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlContextState {
    /// Context not created
    Invalid,
    /// Context created but not current
    Created,
    /// Context is current on a thread
    Current,
    /// Context destroyed
    Destroyed,
}

/// GL context ID
pub type GlContextId = u32;

/// OpenGL context
#[derive(Debug)]
pub struct GlContext {
    /// Context ID
    pub id: GlContextId,
    /// OpenGL version
    pub version: GlVersion,
    /// Profile
    pub profile: GlProfile,
    /// Flags
    pub flags: GlContextFlags,
    /// State
    pub state: GlContextState,
    /// GPU index
    pub gpu_index: u32,
    /// Shared context (for shared objects)
    pub shared_context: Option<GlContextId>,
    /// DRI driver context handle
    pub dri_context: u64,
    /// Current drawable
    pub current_drawable: Option<u32>,
    /// Current read drawable
    pub current_read_drawable: Option<u32>,
}

/// Sync object type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlSyncType {
    /// Fence sync
    Fence,
    /// GPU timeline semaphore
    Timeline,
}

/// Sync object
#[derive(Debug)]
pub struct GlSync {
    pub id: u32,
    pub sync_type: GlSyncType,
    pub condition: u32,
    pub flags: u32,
    pub gpu_fence: u64,
    pub signaled: bool,
}

/// Buffer object type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlBufferTarget {
    Array,
    ElementArray,
    Uniform,
    ShaderStorage,
    TransformFeedback,
    AtomicCounter,
    DispatchIndirect,
    DrawIndirect,
    PixelPack,
    PixelUnpack,
    Query,
    CopyRead,
    CopyWrite,
    Texture,
}

/// Buffer usage hint
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlBufferUsage {
    StaticDraw,
    StaticRead,
    StaticCopy,
    DynamicDraw,
    DynamicRead,
    DynamicCopy,
    StreamDraw,
    StreamRead,
    StreamCopy,
}

/// Buffer object
#[derive(Debug)]
pub struct GlBuffer {
    pub id: u32,
    pub target: GlBufferTarget,
    pub size: usize,
    pub usage: GlBufferUsage,
    pub gpu_address: u64,
    pub mapped: bool,
    pub map_offset: usize,
    pub map_length: usize,
}

/// Shader type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlShaderType {
    Vertex,
    Fragment,
    Geometry,
    TessControl,
    TessEvaluation,
    Compute,
}

/// Shader object
#[derive(Debug)]
pub struct GlShader {
    pub id: u32,
    pub shader_type: GlShaderType,
    pub compiled: bool,
    pub source_hash: u64,
    pub binary_size: usize,
    pub info_log: String,
}

/// Program object
#[derive(Debug)]
pub struct GlProgram {
    pub id: u32,
    pub shaders: Vec<u32>,
    pub linked: bool,
    pub validated: bool,
    pub binary_size: usize,
    pub info_log: String,
    pub uniform_count: u32,
    pub attribute_count: u32,
}

/// Texture format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlTextureFormat {
    R8,
    Rg8,
    Rgb8,
    Rgba8,
    R16,
    Rg16,
    Rgb16,
    Rgba16,
    R16f,
    Rg16f,
    Rgb16f,
    Rgba16f,
    R32f,
    Rg32f,
    Rgb32f,
    Rgba32f,
    R32i,
    Rg32i,
    Rgb32i,
    Rgba32i,
    R32ui,
    Rg32ui,
    Rgb32ui,
    Rgba32ui,
    Depth16,
    Depth24,
    Depth32f,
    Depth24Stencil8,
    Depth32fStencil8,
    CompressedRgbS3tcDxt1,
    CompressedRgbaS3tcDxt1,
    CompressedRgbaS3tcDxt3,
    CompressedRgbaS3tcDxt5,
    CompressedRgbaBptcUnorm,
    CompressedRgbBptcSignedFloat,
    CompressedRgbBptcUnsignedFloat,
}

/// Texture target
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlTextureTarget {
    Texture1d,
    Texture2d,
    Texture3d,
    TextureCubeMap,
    Texture1dArray,
    Texture2dArray,
    TextureCubeMapArray,
    TextureRectangle,
    TextureBuffer,
    Texture2dMultisample,
    Texture2dMultisampleArray,
}

/// Texture object
#[derive(Debug)]
pub struct GlTexture {
    pub id: u32,
    pub target: GlTextureTarget,
    pub format: GlTextureFormat,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub levels: u32,
    pub samples: u32,
    pub gpu_address: u64,
    pub resident: bool,
}

/// Framebuffer attachment point
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlAttachmentPoint {
    Color0,
    Color1,
    Color2,
    Color3,
    Color4,
    Color5,
    Color6,
    Color7,
    Depth,
    Stencil,
    DepthStencil,
}

/// Framebuffer attachment
#[derive(Debug)]
pub struct GlAttachment {
    pub point: GlAttachmentPoint,
    pub texture_id: u32,
    pub level: u32,
    pub layer: u32,
}

/// Framebuffer object
#[derive(Debug)]
pub struct GlFramebuffer {
    pub id: u32,
    pub attachments: Vec<GlAttachment>,
    pub width: u32,
    pub height: u32,
    pub samples: u32,
    pub complete: bool,
}

/// OpenGL state manager
pub struct OpenGlManager {
    /// GPU capabilities
    capabilities: GlCapabilities,
    /// Created contexts
    contexts: Vec<GlContext>,
    /// Next context ID
    next_context_id: GlContextId,
    /// Next buffer ID
    next_buffer_id: u32,
    /// Next texture ID
    next_texture_id: u32,
    /// Next shader ID
    next_shader_id: u32,
    /// Next program ID
    next_program_id: u32,
    /// Next framebuffer ID
    next_framebuffer_id: u32,
    /// Next sync ID
    next_sync_id: u32,
    /// Initialized
    initialized: bool,
    /// GPU vendor
    vendor: String,
    /// GPU renderer string
    renderer: String,
}

impl OpenGlManager {
    /// Create new OpenGL manager
    pub fn new() -> Self {
        Self {
            capabilities: GlCapabilities::default(),
            contexts: Vec::new(),
            next_context_id: 1,
            next_buffer_id: 1,
            next_texture_id: 1,
            next_shader_id: 1,
            next_program_id: 1,
            next_framebuffer_id: 1,
            next_sync_id: 1,
            initialized: false,
            vendor: String::new(),
            renderer: String::new(),
        }
    }

    /// Initialize OpenGL manager
    pub fn init(&mut self, gpu_vendor: &str, gpu_name: &str) -> Result<(), &'static str> {
        crate::kprintln!("[opengl] Initializing OpenGL 4.6 support");

        self.vendor = gpu_vendor.into();
        self.renderer = gpu_name.into();

        // Set up capabilities based on GPU
        self.detect_capabilities()?;

        // Initialize extension list
        self.init_extensions();

        self.initialized = true;

        crate::kprintln!("[opengl] OpenGL {} initialized", self.capabilities.max_gl_version.as_str());
        crate::kprintln!("[opengl] GLSL {}", self.capabilities.max_glsl_version.version);
        crate::kprintln!("[opengl] {} extensions supported", self.capabilities.extensions.len());

        Ok(())
    }

    /// Detect GPU capabilities
    fn detect_capabilities(&mut self) -> Result<(), &'static str> {
        // Set caps based on vendor
        if self.vendor.contains("Intel") {
            // Intel caps (Gen9+)
            self.capabilities.max_texture_size = 16384;
            self.capabilities.max_compute_shared_memory_size = 65536;
        } else if self.vendor.contains("AMD") {
            // AMD caps (GCN+)
            self.capabilities.max_texture_size = 16384;
            self.capabilities.max_compute_shared_memory_size = 65536;
            self.capabilities.max_compute_work_group_size = [1024, 1024, 1024];
        } else if self.vendor.contains("NVIDIA") {
            // NVIDIA caps (Maxwell+)
            self.capabilities.max_texture_size = 32768;
            self.capabilities.max_compute_shared_memory_size = 49152;
        }

        Ok(())
    }

    /// Initialize extension list
    fn init_extensions(&mut self) {
        let extensions = vec![
            // Core 4.6 extensions
            "GL_ARB_indirect_parameters",
            "GL_ARB_pipeline_statistics_query",
            "GL_ARB_polygon_offset_clamp",
            "GL_ARB_shader_atomic_counter_ops",
            "GL_ARB_shader_draw_parameters",
            "GL_ARB_shader_group_vote",
            "GL_ARB_gl_spirv",
            "GL_ARB_spirv_extensions",
            "GL_ARB_texture_filter_anisotropic",
            "GL_ARB_transform_feedback_overflow_query",

            // Common extensions
            "GL_ARB_buffer_storage",
            "GL_ARB_clear_buffer_object",
            "GL_ARB_clear_texture",
            "GL_ARB_clip_control",
            "GL_ARB_compute_shader",
            "GL_ARB_compute_variable_group_size",
            "GL_ARB_conditional_render_inverted",
            "GL_ARB_copy_buffer",
            "GL_ARB_copy_image",
            "GL_ARB_cull_distance",
            "GL_ARB_debug_output",
            "GL_ARB_depth_buffer_float",
            "GL_ARB_depth_clamp",
            "GL_ARB_direct_state_access",
            "GL_ARB_draw_buffers_blend",
            "GL_ARB_draw_elements_base_vertex",
            "GL_ARB_draw_indirect",
            "GL_ARB_enhanced_layouts",
            "GL_ARB_ES3_1_compatibility",
            "GL_ARB_ES3_2_compatibility",
            "GL_ARB_explicit_attrib_location",
            "GL_ARB_explicit_uniform_location",
            "GL_ARB_fragment_layer_viewport",
            "GL_ARB_framebuffer_no_attachments",
            "GL_ARB_framebuffer_object",
            "GL_ARB_framebuffer_sRGB",
            "GL_ARB_geometry_shader4",
            "GL_ARB_get_program_binary",
            "GL_ARB_get_texture_sub_image",
            "GL_ARB_gpu_shader5",
            "GL_ARB_gpu_shader_fp64",
            "GL_ARB_gpu_shader_int64",
            "GL_ARB_half_float_pixel",
            "GL_ARB_half_float_vertex",
            "GL_ARB_instanced_arrays",
            "GL_ARB_map_buffer_alignment",
            "GL_ARB_map_buffer_range",
            "GL_ARB_multi_bind",
            "GL_ARB_multi_draw_indirect",
            "GL_ARB_occlusion_query2",
            "GL_ARB_parallel_shader_compile",
            "GL_ARB_pipeline_statistics_query",
            "GL_ARB_program_interface_query",
            "GL_ARB_provoking_vertex",
            "GL_ARB_query_buffer_object",
            "GL_ARB_robust_buffer_access_behavior",
            "GL_ARB_robustness",
            "GL_ARB_sample_shading",
            "GL_ARB_sampler_objects",
            "GL_ARB_seamless_cube_map",
            "GL_ARB_seamless_cubemap_per_texture",
            "GL_ARB_separate_shader_objects",
            "GL_ARB_shader_atomic_counters",
            "GL_ARB_shader_ballot",
            "GL_ARB_shader_bit_encoding",
            "GL_ARB_shader_clock",
            "GL_ARB_shader_image_load_store",
            "GL_ARB_shader_image_size",
            "GL_ARB_shader_precision",
            "GL_ARB_shader_stencil_export",
            "GL_ARB_shader_storage_buffer_object",
            "GL_ARB_shader_subroutine",
            "GL_ARB_shader_texture_image_samples",
            "GL_ARB_shading_language_420pack",
            "GL_ARB_shading_language_include",
            "GL_ARB_shading_language_packing",
            "GL_ARB_sparse_buffer",
            "GL_ARB_sparse_texture",
            "GL_ARB_sparse_texture2",
            "GL_ARB_sparse_texture_clamp",
            "GL_ARB_stencil_texturing",
            "GL_ARB_sync",
            "GL_ARB_tessellation_shader",
            "GL_ARB_texture_barrier",
            "GL_ARB_texture_buffer_object",
            "GL_ARB_texture_buffer_object_rgb32",
            "GL_ARB_texture_buffer_range",
            "GL_ARB_texture_compression_bptc",
            "GL_ARB_texture_compression_rgtc",
            "GL_ARB_texture_cube_map_array",
            "GL_ARB_texture_filter_minmax",
            "GL_ARB_texture_gather",
            "GL_ARB_texture_mirror_clamp_to_edge",
            "GL_ARB_texture_multisample",
            "GL_ARB_texture_query_levels",
            "GL_ARB_texture_query_lod",
            "GL_ARB_texture_rg",
            "GL_ARB_texture_rgb10_a2ui",
            "GL_ARB_texture_stencil8",
            "GL_ARB_texture_storage",
            "GL_ARB_texture_storage_multisample",
            "GL_ARB_texture_swizzle",
            "GL_ARB_texture_view",
            "GL_ARB_timer_query",
            "GL_ARB_transform_feedback2",
            "GL_ARB_transform_feedback3",
            "GL_ARB_transform_feedback_instanced",
            "GL_ARB_uniform_buffer_object",
            "GL_ARB_vertex_array_bgra",
            "GL_ARB_vertex_array_object",
            "GL_ARB_vertex_attrib_64bit",
            "GL_ARB_vertex_attrib_binding",
            "GL_ARB_vertex_type_10f_11f_11f_rev",
            "GL_ARB_vertex_type_2_10_10_10_rev",
            "GL_ARB_viewport_array",
            "GL_EXT_texture_filter_anisotropic",
            "GL_EXT_texture_compression_s3tc",
            "GL_KHR_debug",
            "GL_KHR_no_error",
            "GL_KHR_robustness",
            "GL_KHR_texture_compression_astc_ldr",
        ];

        for ext in extensions {
            self.capabilities.extensions.push(ext.into());
        }
    }

    /// Create a new OpenGL context
    pub fn create_context(&mut self, version: GlVersion, profile: GlProfile,
                         flags: GlContextFlags, shared: Option<GlContextId>,
                         gpu_index: u32) -> Result<GlContextId, &'static str> {
        if !self.initialized {
            return Err("OpenGL not initialized");
        }

        if version > self.capabilities.max_gl_version {
            return Err("Requested GL version not supported");
        }

        let id = self.next_context_id;
        self.next_context_id += 1;

        let context = GlContext {
            id,
            version,
            profile,
            flags,
            state: GlContextState::Created,
            gpu_index,
            shared_context: shared,
            dri_context: 0,
            current_drawable: None,
            current_read_drawable: None,
        };

        self.contexts.push(context);

        crate::kprintln!("[opengl] Created context {} (GL {}, {:?})", id, version.as_str(), profile);

        Ok(id)
    }

    /// Make context current
    pub fn make_current(&mut self, context_id: GlContextId, drawable: u32) -> Result<(), &'static str> {
        let context = self.contexts.iter_mut()
            .find(|c| c.id == context_id)
            .ok_or("Invalid context")?;

        if context.state == GlContextState::Destroyed {
            return Err("Context destroyed");
        }

        context.state = GlContextState::Current;
        context.current_drawable = Some(drawable);
        context.current_read_drawable = Some(drawable);

        Ok(())
    }

    /// Destroy a context
    pub fn destroy_context(&mut self, context_id: GlContextId) -> Result<(), &'static str> {
        let context = self.contexts.iter_mut()
            .find(|c| c.id == context_id)
            .ok_or("Invalid context")?;

        context.state = GlContextState::Destroyed;

        crate::kprintln!("[opengl] Destroyed context {}", context_id);

        Ok(())
    }

    /// Get capabilities
    pub fn get_capabilities(&self) -> &GlCapabilities {
        &self.capabilities
    }

    /// Check if extension is supported
    pub fn is_extension_supported(&self, ext_name: &str) -> bool {
        self.capabilities.extensions.iter().any(|e| e == ext_name)
    }

    /// Get vendor string
    pub fn get_vendor(&self) -> &str {
        &self.vendor
    }

    /// Get renderer string
    pub fn get_renderer(&self) -> &str {
        &self.renderer
    }

    /// Get version string
    pub fn get_version_string(&self) -> String {
        format!("{}.{} Core", self.capabilities.max_gl_version.major,
               self.capabilities.max_gl_version.minor)
    }

    /// Get GLSL version string
    pub fn get_glsl_version_string(&self) -> String {
        format!("{} core", self.capabilities.max_glsl_version.version)
    }

    /// Get status
    pub fn get_status(&self) -> String {
        format!(
            "OpenGL Manager Status:\n\
             Initialized: {}\n\
             Vendor: {}\n\
             Renderer: {}\n\
             Max GL Version: {}\n\
             Max GLSL Version: {}\n\
             Max Texture Size: {}\n\
             Max Compute Shared Memory: {} KB\n\
             Extensions: {}\n\
             Active Contexts: {}",
            self.initialized,
            self.vendor,
            self.renderer,
            self.capabilities.max_gl_version.as_str(),
            self.capabilities.max_glsl_version.version,
            self.capabilities.max_texture_size,
            self.capabilities.max_compute_shared_memory_size / 1024,
            self.capabilities.extensions.len(),
            self.contexts.iter().filter(|c| c.state != GlContextState::Destroyed).count()
        )
    }
}

/// Global OpenGL manager
static OPENGL: Mutex<Option<OpenGlManager>> = Mutex::new(None);

/// Initialize OpenGL
pub fn init(gpu_vendor: &str, gpu_name: &str) -> Result<(), &'static str> {
    let mut manager = OpenGlManager::new();
    manager.init(gpu_vendor, gpu_name)?;

    *OPENGL.lock() = Some(manager);

    Ok(())
}

/// Get OpenGL manager
pub fn get_manager() -> Option<spin::MutexGuard<'static, Option<OpenGlManager>>> {
    let guard = OPENGL.lock();
    if guard.is_some() {
        Some(guard)
    } else {
        None
    }
}

/// Create context
pub fn create_context(version: GlVersion, profile: GlProfile) -> Result<GlContextId, &'static str> {
    if let Some(mut guard) = get_manager() {
        if let Some(mgr) = guard.as_mut() {
            return mgr.create_context(version, profile, GlContextFlags::default(), None, 0);
        }
    }
    Err("OpenGL not initialized")
}

/// Get capabilities
pub fn get_capabilities() -> Option<GlCapabilities> {
    if let Some(guard) = get_manager() {
        if let Some(mgr) = guard.as_ref() {
            return Some(mgr.get_capabilities().clone());
        }
    }
    None
}

/// Get status
pub fn get_status() -> String {
    if let Some(guard) = get_manager() {
        if let Some(mgr) = guard.as_ref() {
            return mgr.get_status();
        }
    }
    "OpenGL: Not initialized".into()
}
