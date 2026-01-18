//! Direct Rendering Manager (DRM) / Kernel Mode Setting (KMS)
//!
//! Linux-compatible DRM/KMS subsystem for graphics management:
//! - Mode setting (KMS)
//! - Framebuffer management
//! - Display output configuration
//! - IOCTL interface for userspace

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use spin::Mutex;

use super::display_pipe::{
    Crtc, Connector, ConnectionStatus, DisplayMode, DpmsMode, Encoder, Pipe, Plane,
};
use super::gem::{GemHandle, GemTiling};

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static DRM_STATE: Mutex<Option<DrmDevice>> = Mutex::new(None);
static NEXT_FB_ID: AtomicU32 = AtomicU32::new(1);

/// DRM device
#[derive(Debug)]
pub struct DrmDevice {
    /// Device name
    pub name: String,
    /// Driver name
    pub driver: String,
    /// Driver version
    pub version: DrmVersion,
    /// Capabilities
    pub caps: DrmCaps,
    /// Framebuffers
    pub framebuffers: BTreeMap<u32, Framebuffer>,
    /// Open file handles
    pub open_files: Vec<DrmFile>,
    /// Master file descriptor
    pub master_fd: Option<i32>,
}

/// DRM version info
#[derive(Debug, Clone)]
pub struct DrmVersion {
    pub major: i32,
    pub minor: i32,
    pub patch: i32,
    pub name: String,
    pub date: String,
    pub desc: String,
}

/// DRM capabilities
#[derive(Debug, Clone)]
pub struct DrmCaps {
    /// Supports dumb buffers
    pub dumb_buffer: bool,
    /// Supports VBLANK high-CRTC
    pub vblank_high_crtc: bool,
    /// Supports dumb preferred depth
    pub dumb_preferred_depth: u32,
    /// Supports dumb prefer shadow
    pub dumb_prefer_shadow: bool,
    /// Supports PRIME import/export
    pub prime: bool,
    /// Supports timestamps (monotonic)
    pub timestamp_monotonic: bool,
    /// Supports async page flip
    pub async_page_flip: bool,
    /// Supports cursor width
    pub cursor_width: u32,
    /// Supports cursor height
    pub cursor_height: u32,
    /// Supports atomic modesetting
    pub atomic: bool,
    /// Supports modifiers
    pub modifiers: bool,
}

/// DRM framebuffer
#[derive(Debug, Clone)]
pub struct Framebuffer {
    pub id: u32,
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
    pub depth: u32,
    pub bpp: u32,
    pub handle: GemHandle,
    pub offset: u64,
    pub modifier: u64,
    pub flags: u32,
}

/// DRM file (open instance)
#[derive(Debug)]
pub struct DrmFile {
    pub fd: i32,
    pub is_master: bool,
    pub auth_magic: u32,
    pub framebuffers: Vec<u32>,
}

/// Mode resources (for ioctl)
#[derive(Debug, Clone)]
pub struct DrmModeResources {
    pub fb_id_count: u32,
    pub crtc_id_count: u32,
    pub connector_id_count: u32,
    pub encoder_id_count: u32,
    pub min_width: u32,
    pub max_width: u32,
    pub min_height: u32,
    pub max_height: u32,
    pub fb_ids: Vec<u32>,
    pub crtc_ids: Vec<u32>,
    pub connector_ids: Vec<u32>,
    pub encoder_ids: Vec<u32>,
}

/// DRM ioctl commands
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrmIoctl {
    Version = 0x00,
    GetUnique = 0x01,
    GetMagic = 0x02,
    SetVersion = 0x07,
    GetCap = 0x0C,
    SetMaster = 0x1E,
    DropMaster = 0x1F,
    ModeGetResources = 0xA0,
    ModeGetCrtc = 0xA1,
    ModeSetCrtc = 0xA2,
    ModeCursor = 0xA3,
    ModeGetGamma = 0xA4,
    ModeSetGamma = 0xA5,
    ModeGetEncoder = 0xA6,
    ModeGetConnector = 0xA7,
    ModeGetProperty = 0xAA,
    ModeSetProperty = 0xAB,
    ModeGetPropBlob = 0xAC,
    ModeGetFb = 0xAD,
    ModeAddFb = 0xAE,
    ModeRmFb = 0xAF,
    ModePageFlip = 0xB0,
    ModeDirtyFb = 0xB1,
    ModeCreateDumb = 0xB2,
    ModeMapDumb = 0xB3,
    ModeDestroyDumb = 0xB4,
    ModeGetPlaneResources = 0xB5,
    ModeGetPlane = 0xB6,
    ModeSetPlane = 0xB7,
    ModeAddFb2 = 0xB8,
    ModeObjGetProperties = 0xB9,
    ModeObjSetProperty = 0xBA,
    ModeCursor2 = 0xBB,
    ModeAtomic = 0xBC,
    ModeCreatePropertyBlob = 0xBD,
    ModeDestroyPropertyBlob = 0xBE,
}

/// Initialize DRM subsystem
pub fn init(name: &str, driver: &str) {
    if INITIALIZED.swap(true, Ordering::SeqCst) {
        return;
    }

    crate::kprintln!("drm: Initializing DRM/KMS subsystem...");

    let device = DrmDevice {
        name: String::from(name),
        driver: String::from(driver),
        version: DrmVersion {
            major: 1,
            minor: 0,
            patch: 0,
            name: String::from(driver),
            date: String::from("20260117"),
            desc: String::from("Stenzel OS DRM Driver"),
        },
        caps: DrmCaps {
            dumb_buffer: true,
            vblank_high_crtc: true,
            dumb_preferred_depth: 24,
            dumb_prefer_shadow: false,
            prime: true,
            timestamp_monotonic: true,
            async_page_flip: true,
            cursor_width: 64,
            cursor_height: 64,
            atomic: true,
            modifiers: true,
        },
        framebuffers: BTreeMap::new(),
        open_files: Vec::new(),
        master_fd: None,
    };

    *DRM_STATE.lock() = Some(device);

    crate::kprintln!("drm: DRM device '{}' initialized (driver: {})", name, driver);
}

/// Open DRM device
pub fn drm_open(fd: i32) -> bool {
    let mut state = DRM_STATE.lock();
    let device = match state.as_mut() {
        Some(d) => d,
        None => return false,
    };

    let is_master = device.master_fd.is_none();
    if is_master {
        device.master_fd = Some(fd);
    }

    device.open_files.push(DrmFile {
        fd,
        is_master,
        auth_magic: 0,
        framebuffers: Vec::new(),
    });

    crate::kprintln!("drm: Opened fd {} (master: {})", fd, is_master);
    true
}

/// Close DRM device
pub fn drm_close(fd: i32) {
    let mut state = DRM_STATE.lock();
    let device = match state.as_mut() {
        Some(d) => d,
        None => return,
    };

    // Remove framebuffers owned by this file
    if let Some(pos) = device.open_files.iter().position(|f| f.fd == fd) {
        let file = device.open_files.remove(pos);
        for fb_id in file.framebuffers {
            device.framebuffers.remove(&fb_id);
        }

        if device.master_fd == Some(fd) {
            device.master_fd = None;
        }
    }

    crate::kprintln!("drm: Closed fd {}", fd);
}

/// Get DRM version
pub fn drm_version() -> Option<DrmVersion> {
    let state = DRM_STATE.lock();
    state.as_ref().map(|d| d.version.clone())
}

/// Get DRM capability
pub fn drm_get_cap(cap: u64) -> Option<u64> {
    let state = DRM_STATE.lock();
    let device = state.as_ref()?;

    Some(match cap {
        0x01 => device.caps.dumb_buffer as u64,
        0x02 => device.caps.vblank_high_crtc as u64,
        0x03 => device.caps.dumb_preferred_depth as u64,
        0x04 => device.caps.dumb_prefer_shadow as u64,
        0x05 => device.caps.prime as u64,
        0x06 => device.caps.timestamp_monotonic as u64,
        0x07 => device.caps.async_page_flip as u64,
        0x08 => device.caps.cursor_width as u64,
        0x09 => device.caps.cursor_height as u64,
        0x10 => device.caps.atomic as u64,
        0x11 => device.caps.modifiers as u64,
        _ => 0,
    })
}

/// Get mode resources
pub fn drm_mode_get_resources() -> Option<DrmModeResources> {
    let state = DRM_STATE.lock();
    let device = state.as_ref()?;

    let crtcs = super::display_pipe::get_crtcs();
    let connectors = super::display_pipe::get_connectors();
    let planes = super::display_pipe::get_planes();

    Some(DrmModeResources {
        fb_id_count: device.framebuffers.len() as u32,
        crtc_id_count: crtcs.len() as u32,
        connector_id_count: connectors.len() as u32,
        encoder_id_count: 4, // From detect_encoders
        min_width: 0,
        max_width: 8192,
        min_height: 0,
        max_height: 8192,
        fb_ids: device.framebuffers.keys().copied().collect(),
        crtc_ids: crtcs.iter().map(|c| c.id).collect(),
        connector_ids: connectors.iter().map(|c| c.id).collect(),
        encoder_ids: vec![0, 1, 2, 3],
    })
}

/// Create framebuffer
pub fn drm_mode_add_fb(
    fd: i32,
    width: u32,
    height: u32,
    pitch: u32,
    depth: u32,
    bpp: u32,
    handle: GemHandle,
) -> Option<u32> {
    let mut state = DRM_STATE.lock();
    let device = state.as_mut()?;

    let fb_id = NEXT_FB_ID.fetch_add(1, Ordering::SeqCst);

    let fb = Framebuffer {
        id: fb_id,
        width,
        height,
        pitch,
        depth,
        bpp,
        handle,
        offset: 0,
        modifier: 0,
        flags: 0,
    };

    device.framebuffers.insert(fb_id, fb);

    // Track ownership
    if let Some(file) = device.open_files.iter_mut().find(|f| f.fd == fd) {
        file.framebuffers.push(fb_id);
    }

    crate::kprintln!("drm: Created framebuffer {} ({}x{}, pitch={}, bpp={})",
        fb_id, width, height, pitch, bpp);

    Some(fb_id)
}

/// Create framebuffer with modifiers (add_fb2)
pub fn drm_mode_add_fb2(
    fd: i32,
    width: u32,
    height: u32,
    pixel_format: u32,
    handles: &[GemHandle],
    pitches: &[u32],
    offsets: &[u64],
    modifier: u64,
    flags: u32,
) -> Option<u32> {
    let mut state = DRM_STATE.lock();
    let device = state.as_mut()?;

    let fb_id = NEXT_FB_ID.fetch_add(1, Ordering::SeqCst);

    let fb = Framebuffer {
        id: fb_id,
        width,
        height,
        pitch: pitches.first().copied().unwrap_or(0),
        depth: 24,
        bpp: 32,
        handle: handles.first().copied().unwrap_or(0),
        offset: offsets.first().copied().unwrap_or(0),
        modifier,
        flags,
    };

    device.framebuffers.insert(fb_id, fb);

    if let Some(file) = device.open_files.iter_mut().find(|f| f.fd == fd) {
        file.framebuffers.push(fb_id);
    }

    Some(fb_id)
}

/// Remove framebuffer
pub fn drm_mode_rm_fb(fd: i32, fb_id: u32) -> bool {
    let mut state = DRM_STATE.lock();
    let device = match state.as_mut() {
        Some(d) => d,
        None => return false,
    };

    if device.framebuffers.remove(&fb_id).is_some() {
        if let Some(file) = device.open_files.iter_mut().find(|f| f.fd == fd) {
            file.framebuffers.retain(|&id| id != fb_id);
        }
        crate::kprintln!("drm: Removed framebuffer {}", fb_id);
        true
    } else {
        false
    }
}

/// Get framebuffer info
pub fn drm_mode_get_fb(fb_id: u32) -> Option<Framebuffer> {
    let state = DRM_STATE.lock();
    let device = state.as_ref()?;
    device.framebuffers.get(&fb_id).cloned()
}

/// Set CRTC configuration
pub fn drm_mode_set_crtc(
    crtc_id: u32,
    fb_id: u32,
    x: u32,
    y: u32,
    connectors: &[u32],
    mode: Option<&DisplayMode>,
) -> bool {
    if let Some(mode) = mode {
        super::display_pipe::crtc_enable(crtc_id, mode, fb_id)
    } else {
        super::display_pipe::crtc_disable(crtc_id)
    }
}

/// Page flip
pub fn drm_mode_page_flip(crtc_id: u32, fb_id: u32, flags: u32, user_data: u64) -> bool {
    // Update the framebuffer on the specified CRTC
    // In a real implementation, this would:
    // 1. Wait for vblank
    // 2. Atomically swap the framebuffer
    // 3. Generate a page flip completion event

    crate::kprintln!("drm: Page flip on CRTC {} to FB {}", crtc_id, fb_id);
    true
}

/// Create dumb buffer
pub fn drm_mode_create_dumb(
    width: u32,
    height: u32,
    bpp: u32,
) -> Option<(GemHandle, u32, u64)> {
    let pitch = ((width * bpp + 31) / 32) * 4; // Align to 4 bytes
    let size = (pitch * height) as usize;

    let handle = super::gem::gem_create(size)?;

    Some((handle, pitch, size as u64))
}

/// Map dumb buffer
pub fn drm_mode_map_dumb(handle: GemHandle) -> Option<u64> {
    // Bind to GTT and return offset
    let offset = super::gem::gem_bind(handle)?;
    Some(offset)
}

/// Destroy dumb buffer
pub fn drm_mode_destroy_dumb(handle: GemHandle) -> bool {
    super::gem::gem_close(handle)
}

/// Get connector info
pub fn drm_mode_get_connector(connector_id: u32) -> Option<Connector> {
    let connectors = super::display_pipe::get_connectors();
    connectors.into_iter().find(|c| c.id == connector_id)
}

/// Get encoder info
pub fn drm_mode_get_encoder(encoder_id: u32) -> Option<Encoder> {
    // Would return encoder info from display_pipe
    None // Simplified
}

/// Get CRTC info
pub fn drm_mode_get_crtc(crtc_id: u32) -> Option<Crtc> {
    let crtcs = super::display_pipe::get_crtcs();
    crtcs.into_iter().find(|c| c.id == crtc_id)
}

/// Set plane
pub fn drm_mode_set_plane(
    plane_id: u32,
    crtc_id: u32,
    fb_id: u32,
    flags: u32,
    crtc_x: i32,
    crtc_y: i32,
    crtc_w: u32,
    crtc_h: u32,
    src_x: u32,
    src_y: u32,
    src_w: u32,
    src_h: u32,
) -> bool {
    super::display_pipe::plane_update(
        plane_id, fb_id, crtc_id,
        src_x, src_y, src_w, src_h,
        crtc_x, crtc_y, crtc_w, crtc_h,
    )
}

/// Atomic mode set
pub fn drm_mode_atomic(
    fd: i32,
    flags: u32,
    objs: &[(u32, u32, u64)], // (obj_id, prop_id, value)
) -> bool {
    // Atomic mode setting allows multiple properties to be set atomically
    // This is a simplified implementation

    for &(obj_id, prop_id, value) in objs {
        // Apply property changes
        // In a real implementation, these would be queued and applied atomically
    }

    true
}

/// Set cursor
pub fn drm_mode_cursor(crtc_id: u32, handle: GemHandle, width: u32, height: u32) -> bool {
    // Set cursor buffer on CRTC
    crate::kprintln!("drm: Set cursor on CRTC {} ({}x{})", crtc_id, width, height);
    true
}

/// Move cursor
pub fn drm_mode_cursor_move(crtc_id: u32, x: i32, y: i32) -> bool {
    // Move cursor position
    true
}

/// Wait for vblank
pub fn drm_wait_vblank(crtc_id: u32) {
    // In a real implementation, this would wait for the vertical blank period
    // For now, just a small delay
    for _ in 0..1000 {
        core::hint::spin_loop();
    }
}

/// Get CRTC gamma
pub fn drm_mode_get_gamma(crtc_id: u32) -> Option<(Vec<u16>, Vec<u16>, Vec<u16>)> {
    // Return current gamma LUT
    let size = 256;
    let red: Vec<u16> = (0..size).map(|i| (i * 256) as u16).collect();
    let green = red.clone();
    let blue = red.clone();
    Some((red, green, blue))
}

/// Set CRTC gamma
pub fn drm_mode_set_gamma(crtc_id: u32, red: &[u16], green: &[u16], blue: &[u16]) -> bool {
    // Set gamma LUT
    crate::kprintln!("drm: Set gamma LUT on CRTC {} ({} entries)",
        crtc_id, red.len());
    true
}

/// Check if DRM is initialized
pub fn is_initialized() -> bool {
    INITIALIZED.load(Ordering::SeqCst)
}

/// Get device name
pub fn get_device_name() -> Option<String> {
    let state = DRM_STATE.lock();
    state.as_ref().map(|d| d.name.clone())
}
