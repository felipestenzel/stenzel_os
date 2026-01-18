//! DRM Framebuffer Driver
//!
//! Provides DRM-based framebuffer management for display output:
//! - Framebuffer allocation and management
//! - Scanout buffer handling
//! - Double/triple buffering
//! - Page flipping
//! - Damage tracking

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use spin::Mutex;

use super::display_pipe::PixelFormat;
use super::gem::GemHandle;

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static FB_STATE: Mutex<Option<FramebufferState>> = Mutex::new(None);
static NEXT_FB_ID: AtomicU32 = AtomicU32::new(1);

/// Framebuffer state manager
#[derive(Debug)]
pub struct FramebufferState {
    /// Active framebuffers
    pub framebuffers: BTreeMap<u32, Framebuffer>,
    /// Scanout buffers per CRTC
    pub scanout_buffers: BTreeMap<u32, ScanoutBuffer>,
    /// Double buffering enabled
    pub double_buffering: bool,
    /// VSync enabled
    pub vsync_enabled: bool,
}

/// Framebuffer object
#[derive(Debug, Clone)]
pub struct Framebuffer {
    /// Framebuffer ID
    pub id: u32,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// Pitch (bytes per row)
    pub pitch: u32,
    /// Pixel format
    pub format: PixelFormat,
    /// Depth (bits per pixel)
    pub depth: u8,
    /// BPP
    pub bpp: u8,
    /// GEM handle for backing buffer
    pub gem_handle: GemHandle,
    /// Offset into GEM buffer
    pub offset: u64,
    /// Modifier (tiling, compression)
    pub modifier: u64,
    /// Number of planes (for multi-planar formats)
    pub num_planes: u8,
    /// Plane offsets
    pub plane_offsets: [u64; 4],
    /// Plane pitches
    pub plane_pitches: [u32; 4],
}

/// Scanout buffer for display
#[derive(Debug)]
pub struct ScanoutBuffer {
    /// CRTC ID
    pub crtc_id: u32,
    /// Front buffer ID
    pub front_buffer: u32,
    /// Back buffer ID (for double buffering)
    pub back_buffer: Option<u32>,
    /// Third buffer (for triple buffering)
    pub third_buffer: Option<u32>,
    /// Current buffer index
    pub current_index: u8,
    /// Pending flip
    pub pending_flip: bool,
    /// Flip sequence number
    pub flip_sequence: u64,
}

/// Page flip completion event
#[derive(Debug, Clone)]
pub struct PageFlipEvent {
    /// CRTC ID
    pub crtc_id: u32,
    /// Frame sequence number
    pub sequence: u64,
    /// Timestamp (nanoseconds)
    pub timestamp_ns: u64,
    /// User data
    pub user_data: u64,
}

/// Damage rectangle
#[derive(Debug, Clone, Copy)]
pub struct DamageRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Framebuffer creation flags
#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub enum FbFlags {
    None = 0,
    Interlaced = 1,
    ModifiersSet = 2,
}

/// DRM format modifiers
pub mod modifiers {
    pub const LINEAR: u64 = 0;
    pub const INTEL_X_TILED: u64 = (1 << 56) | 1;
    pub const INTEL_Y_TILED: u64 = (1 << 56) | 2;
    pub const INTEL_YF_TILED: u64 = (1 << 56) | 3;
    pub const INTEL_Y_TILED_CCS: u64 = (1 << 56) | 4;
    pub const AMD_GFX9_64K_S: u64 = (2 << 56) | 1;
    pub const AMD_GFX9_64K_D: u64 = (2 << 56) | 2;
    pub const AMD_GFX10_RBPLUS_64K_S: u64 = (2 << 56) | 3;
}

/// Error type
#[derive(Debug, Clone, Copy)]
pub enum FbError {
    NotInitialized,
    InvalidFormat,
    InvalidSize,
    AllocationFailed,
    NotFound,
    InvalidHandle,
    FlipPending,
    InvalidCrtc,
}

/// Initialize framebuffer subsystem
pub fn init() -> Result<(), FbError> {
    if INITIALIZED.load(Ordering::Acquire) {
        return Ok(());
    }

    let state = FramebufferState {
        framebuffers: BTreeMap::new(),
        scanout_buffers: BTreeMap::new(),
        double_buffering: true,
        vsync_enabled: true,
    };

    *FB_STATE.lock() = Some(state);
    INITIALIZED.store(true, Ordering::Release);

    crate::kprintln!("drm_fb: Framebuffer subsystem initialized");
    Ok(())
}

/// Create a new framebuffer
pub fn create_framebuffer(
    width: u32,
    height: u32,
    format: PixelFormat,
    gem_handle: GemHandle,
    pitch: u32,
    offset: u64,
    modifier: u64,
) -> Result<u32, FbError> {
    if !INITIALIZED.load(Ordering::Acquire) {
        return Err(FbError::NotInitialized);
    }

    // Validate parameters
    if width == 0 || height == 0 || width > 16384 || height > 16384 {
        return Err(FbError::InvalidSize);
    }

    let (depth, bpp) = format_info(format)?;

    let fb_id = NEXT_FB_ID.fetch_add(1, Ordering::SeqCst);

    let fb = Framebuffer {
        id: fb_id,
        width,
        height,
        pitch,
        format,
        depth,
        bpp,
        gem_handle,
        offset,
        modifier,
        num_planes: 1,
        plane_offsets: [offset, 0, 0, 0],
        plane_pitches: [pitch, 0, 0, 0],
    };

    let mut state = FB_STATE.lock();
    let state = state.as_mut().ok_or(FbError::NotInitialized)?;
    state.framebuffers.insert(fb_id, fb);

    Ok(fb_id)
}

/// Create framebuffer with multiple planes (for YUV formats)
pub fn create_framebuffer_multi(
    width: u32,
    height: u32,
    format: PixelFormat,
    gem_handles: &[GemHandle],
    pitches: &[u32],
    offsets: &[u64],
    modifier: u64,
) -> Result<u32, FbError> {
    if !INITIALIZED.load(Ordering::Acquire) {
        return Err(FbError::NotInitialized);
    }

    let num_planes = gem_handles.len().min(4) as u8;
    if num_planes == 0 {
        return Err(FbError::InvalidHandle);
    }

    let (depth, bpp) = format_info(format)?;
    let fb_id = NEXT_FB_ID.fetch_add(1, Ordering::SeqCst);

    let mut plane_offsets = [0u64; 4];
    let mut plane_pitches = [0u32; 4];

    for i in 0..num_planes as usize {
        plane_offsets[i] = offsets.get(i).copied().unwrap_or(0);
        plane_pitches[i] = pitches.get(i).copied().unwrap_or(0);
    }

    let fb = Framebuffer {
        id: fb_id,
        width,
        height,
        pitch: pitches.first().copied().unwrap_or(0),
        format,
        depth,
        bpp,
        gem_handle: gem_handles.first().copied().unwrap_or(0),
        offset: offsets.first().copied().unwrap_or(0),
        modifier,
        num_planes,
        plane_offsets,
        plane_pitches,
    };

    let mut state = FB_STATE.lock();
    let state = state.as_mut().ok_or(FbError::NotInitialized)?;
    state.framebuffers.insert(fb_id, fb);

    Ok(fb_id)
}

/// Destroy a framebuffer
pub fn destroy_framebuffer(fb_id: u32) -> Result<(), FbError> {
    let mut state = FB_STATE.lock();
    let state = state.as_mut().ok_or(FbError::NotInitialized)?;

    if state.framebuffers.remove(&fb_id).is_none() {
        return Err(FbError::NotFound);
    }

    Ok(())
}

/// Get framebuffer info
pub fn get_framebuffer(fb_id: u32) -> Result<Framebuffer, FbError> {
    let state = FB_STATE.lock();
    let state = state.as_ref().ok_or(FbError::NotInitialized)?;

    state
        .framebuffers
        .get(&fb_id)
        .cloned()
        .ok_or(FbError::NotFound)
}

/// Setup scanout buffer for a CRTC
pub fn setup_scanout(crtc_id: u32, front_fb: u32, back_fb: Option<u32>) -> Result<(), FbError> {
    let mut state = FB_STATE.lock();
    let state = state.as_mut().ok_or(FbError::NotInitialized)?;

    // Verify framebuffers exist
    if !state.framebuffers.contains_key(&front_fb) {
        return Err(FbError::NotFound);
    }
    if let Some(back) = back_fb {
        if !state.framebuffers.contains_key(&back) {
            return Err(FbError::NotFound);
        }
    }

    let scanout = ScanoutBuffer {
        crtc_id,
        front_buffer: front_fb,
        back_buffer: back_fb,
        third_buffer: None,
        current_index: 0,
        pending_flip: false,
        flip_sequence: 0,
    };

    state.scanout_buffers.insert(crtc_id, scanout);
    Ok(())
}

/// Setup triple buffering
pub fn setup_triple_buffer(crtc_id: u32, fb1: u32, fb2: u32, fb3: u32) -> Result<(), FbError> {
    let mut state = FB_STATE.lock();
    let state = state.as_mut().ok_or(FbError::NotInitialized)?;

    // Verify all framebuffers exist
    for fb in [fb1, fb2, fb3] {
        if !state.framebuffers.contains_key(&fb) {
            return Err(FbError::NotFound);
        }
    }

    let scanout = ScanoutBuffer {
        crtc_id,
        front_buffer: fb1,
        back_buffer: Some(fb2),
        third_buffer: Some(fb3),
        current_index: 0,
        pending_flip: false,
        flip_sequence: 0,
    };

    state.scanout_buffers.insert(crtc_id, scanout);
    Ok(())
}

/// Request page flip
pub fn page_flip(crtc_id: u32, fb_id: u32, user_data: u64) -> Result<u64, FbError> {
    let mut state = FB_STATE.lock();
    let state = state.as_mut().ok_or(FbError::NotInitialized)?;

    let scanout = state
        .scanout_buffers
        .get_mut(&crtc_id)
        .ok_or(FbError::InvalidCrtc)?;

    if scanout.pending_flip {
        return Err(FbError::FlipPending);
    }

    // Verify framebuffer exists
    if !state.framebuffers.contains_key(&fb_id) {
        return Err(FbError::NotFound);
    }

    scanout.pending_flip = true;
    scanout.flip_sequence += 1;
    let sequence = scanout.flip_sequence;

    // Update front buffer
    scanout.front_buffer = fb_id;
    scanout.current_index = (scanout.current_index + 1) % 3;

    // In a real implementation, this would program the display controller
    // to flip to the new buffer at the next vblank

    Ok(sequence)
}

/// Complete page flip (called from vblank handler)
pub fn complete_page_flip(crtc_id: u32) -> Option<PageFlipEvent> {
    let mut state = FB_STATE.lock();
    let state = state.as_mut()?;

    let scanout = state.scanout_buffers.get_mut(&crtc_id)?;

    if !scanout.pending_flip {
        return None;
    }

    scanout.pending_flip = false;

    Some(PageFlipEvent {
        crtc_id,
        sequence: scanout.flip_sequence,
        timestamp_ns: crate::time::uptime_ns(),
        user_data: 0,
    })
}

/// Mark damage region on framebuffer
pub fn dirty_fb(fb_id: u32, clips: &[DamageRect]) -> Result<(), FbError> {
    let state = FB_STATE.lock();
    let state = state.as_ref().ok_or(FbError::NotInitialized)?;

    // Verify framebuffer exists
    if !state.framebuffers.contains_key(&fb_id) {
        return Err(FbError::NotFound);
    }

    // In a real implementation, this would:
    // 1. Track dirty regions
    // 2. Trigger partial updates for displays that support it
    // 3. Optimize scanout for only changed regions

    let _ = clips; // Use damage info for partial updates
    Ok(())
}

/// Get current scanout buffer for CRTC
pub fn get_current_scanout(crtc_id: u32) -> Result<u32, FbError> {
    let state = FB_STATE.lock();
    let state = state.as_ref().ok_or(FbError::NotInitialized)?;

    let scanout = state
        .scanout_buffers
        .get(&crtc_id)
        .ok_or(FbError::InvalidCrtc)?;

    Ok(scanout.front_buffer)
}

/// Get back buffer for CRTC (for rendering)
pub fn get_back_buffer(crtc_id: u32) -> Result<Option<u32>, FbError> {
    let state = FB_STATE.lock();
    let state = state.as_ref().ok_or(FbError::NotInitialized)?;

    let scanout = state
        .scanout_buffers
        .get(&crtc_id)
        .ok_or(FbError::InvalidCrtc)?;

    Ok(scanout.back_buffer)
}

/// Enable/disable VSync
pub fn set_vsync(enabled: bool) {
    if let Some(state) = FB_STATE.lock().as_mut() {
        state.vsync_enabled = enabled;
    }
}

/// Check if VSync is enabled
pub fn is_vsync_enabled() -> bool {
    FB_STATE
        .lock()
        .as_ref()
        .map(|s| s.vsync_enabled)
        .unwrap_or(true)
}

/// Wait for vblank
pub fn wait_vblank(crtc_id: u32) -> Result<u64, FbError> {
    // In a real implementation, this would wait for the vblank interrupt
    // For now, simulate by returning current timestamp
    let _ = crtc_id;
    Ok(crate::time::uptime_ns())
}

/// Get format information (returns depth, bpp)
fn format_info(format: PixelFormat) -> Result<(u8, u8), FbError> {
    match format {
        PixelFormat::Argb8888 | PixelFormat::Abgr8888 => Ok((32, 32)),
        PixelFormat::Xrgb8888 | PixelFormat::Xbgr8888 => Ok((24, 32)),
        PixelFormat::Rgb565 => Ok((16, 16)),
        PixelFormat::Xrgb2101010 | PixelFormat::Argb2101010 => Ok((30, 32)),
        PixelFormat::Nv12 => Ok((12, 12)), // YUV 4:2:0
        PixelFormat::P010 => Ok((15, 15)), // 10-bit YUV 4:2:0
        PixelFormat::Yuyv | PixelFormat::Uyvy => Ok((16, 16)), // YUV 4:2:2
    }
}

/// Calculate minimum pitch for format and width
pub fn calculate_pitch(format: PixelFormat, width: u32) -> Result<u32, FbError> {
    let (_, bpp) = format_info(format)?;
    // Align to 64 bytes for optimal DMA
    let bytes_per_pixel = bpp as u32 / 8;
    let min_pitch = width * bytes_per_pixel;
    Ok((min_pitch + 63) & !63)
}

/// Calculate required buffer size
pub fn calculate_buffer_size(format: PixelFormat, width: u32, height: u32) -> Result<u64, FbError> {
    let pitch = calculate_pitch(format, width)?;
    Ok(pitch as u64 * height as u64)
}

/// List all framebuffers
pub fn list_framebuffers() -> Vec<u32> {
    FB_STATE
        .lock()
        .as_ref()
        .map(|s| s.framebuffers.keys().copied().collect())
        .unwrap_or_default()
}

/// Get framebuffer count
pub fn framebuffer_count() -> usize {
    FB_STATE
        .lock()
        .as_ref()
        .map(|s| s.framebuffers.len())
        .unwrap_or(0)
}
