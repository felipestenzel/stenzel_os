//! Display Pipeline Driver
//!
//! Manages display output pipeline configuration for Intel/AMD graphics:
//! - CRTC (Cathode Ray Tube Controller) - display timing
//! - Plane (framebuffer sources)
//! - Encoder (signal conversion)
//! - Connector (physical outputs)

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use spin::Mutex;

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static DISPLAY_STATE: Mutex<Option<DisplayState>> = Mutex::new(None);

/// Display state manager
#[derive(Debug)]
pub struct DisplayState {
    /// MMIO base address
    pub mmio_base: u64,
    /// Available CRTCs
    pub crtcs: Vec<Crtc>,
    /// Available planes
    pub planes: Vec<Plane>,
    /// Available encoders
    pub encoders: Vec<Encoder>,
    /// Available connectors
    pub connectors: Vec<Connector>,
    /// Active display configurations
    pub active_configs: Vec<DisplayConfig>,
}

/// CRTC (display timing controller)
#[derive(Debug, Clone)]
pub struct Crtc {
    pub id: u32,
    pub pipe: Pipe,
    pub enabled: bool,
    pub mode: Option<DisplayMode>,
    pub gamma_size: u32,
    /// Primary plane bound to this CRTC
    pub primary_plane: Option<u32>,
    /// Cursor plane
    pub cursor_plane: Option<u32>,
}

/// Display pipe identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pipe {
    A,
    B,
    C,
    D,
}

impl Pipe {
    pub fn index(&self) -> usize {
        match self {
            Pipe::A => 0,
            Pipe::B => 1,
            Pipe::C => 2,
            Pipe::D => 3,
        }
    }
}

/// Display plane (framebuffer source)
#[derive(Debug, Clone)]
pub struct Plane {
    pub id: u32,
    pub plane_type: PlaneType,
    pub possible_crtcs: u32, // Bitmask
    pub formats: Vec<PixelFormat>,
    pub enabled: bool,
    pub fb_id: Option<u32>,
    pub crtc_id: Option<u32>,
    pub src_x: u32,
    pub src_y: u32,
    pub src_w: u32,
    pub src_h: u32,
    pub crtc_x: i32,
    pub crtc_y: i32,
    pub crtc_w: u32,
    pub crtc_h: u32,
    pub rotation: Rotation,
}

/// Plane type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaneType {
    /// Primary plane (main display)
    Primary,
    /// Overlay plane (hardware composition)
    Overlay,
    /// Cursor plane
    Cursor,
}

/// Plane rotation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rotation {
    Rotate0,
    Rotate90,
    Rotate180,
    Rotate270,
    ReflectX,
    ReflectY,
}

/// Pixel format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Xrgb8888,
    Argb8888,
    Xbgr8888,
    Abgr8888,
    Rgb565,
    Xrgb2101010,
    Argb2101010,
    Yuyv,
    Uyvy,
    Nv12,
    P010,
}

impl PixelFormat {
    pub fn bpp(&self) -> u32 {
        match self {
            PixelFormat::Rgb565 => 16,
            PixelFormat::Xrgb8888 | PixelFormat::Argb8888 |
            PixelFormat::Xbgr8888 | PixelFormat::Abgr8888 |
            PixelFormat::Xrgb2101010 | PixelFormat::Argb2101010 => 32,
            PixelFormat::Yuyv | PixelFormat::Uyvy => 16,
            PixelFormat::Nv12 => 12,
            PixelFormat::P010 => 15,
        }
    }

    pub fn fourcc(&self) -> u32 {
        match self {
            PixelFormat::Xrgb8888 => fourcc(b"XR24"),
            PixelFormat::Argb8888 => fourcc(b"AR24"),
            PixelFormat::Xbgr8888 => fourcc(b"XB24"),
            PixelFormat::Abgr8888 => fourcc(b"AB24"),
            PixelFormat::Rgb565 => fourcc(b"RG16"),
            PixelFormat::Xrgb2101010 => fourcc(b"XR30"),
            PixelFormat::Argb2101010 => fourcc(b"AR30"),
            PixelFormat::Yuyv => fourcc(b"YUYV"),
            PixelFormat::Uyvy => fourcc(b"UYVY"),
            PixelFormat::Nv12 => fourcc(b"NV12"),
            PixelFormat::P010 => fourcc(b"P010"),
        }
    }
}

fn fourcc(code: &[u8; 4]) -> u32 {
    u32::from_le_bytes(*code)
}

/// Encoder (signal conversion)
#[derive(Debug, Clone)]
pub struct Encoder {
    pub id: u32,
    pub encoder_type: EncoderType,
    pub possible_crtcs: u32,
    pub possible_clones: u32,
    pub crtc_id: Option<u32>,
}

/// Encoder type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncoderType {
    None,
    Dac,     // VGA
    Tmds,    // HDMI/DVI
    Lvds,    // Laptop panel
    Tvdac,   // TV output
    Virtual,
    Dsi,     // DSI display
    Dpmst,   // DP MST
    Dpi,     // DPI
}

/// Physical connector
#[derive(Debug, Clone)]
pub struct Connector {
    pub id: u32,
    pub connector_type: ConnectorType,
    pub connector_type_id: u32,
    pub connection: ConnectionStatus,
    pub encoder_id: Option<u32>,
    pub possible_encoders: u32,
    pub modes: Vec<DisplayMode>,
    pub edid: Option<Vec<u8>>,
    pub dpms: DpmsMode,
    pub properties: BTreeMap<String, u64>,
}

/// Connector type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectorType {
    Unknown,
    Vga,
    DviI,
    DviD,
    DviA,
    Composite,
    Svideo,
    Lvds,
    Component,
    NinePinDin,
    DisplayPort,
    HdmiA,
    HdmiB,
    Tv,
    Edp,
    Virtual,
    Dsi,
    Dpi,
    Writeback,
    Spi,
    Usb,
}

/// Connection status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Connected,
    Disconnected,
    Unknown,
}

/// DPMS (Display Power Management Signaling) mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DpmsMode {
    On,
    Standby,
    Suspend,
    Off,
}

/// Display mode (resolution + timing)
#[derive(Debug, Clone)]
pub struct DisplayMode {
    pub name: String,
    pub clock: u32,        // Pixel clock in kHz
    pub hdisplay: u16,
    pub hsync_start: u16,
    pub hsync_end: u16,
    pub htotal: u16,
    pub vdisplay: u16,
    pub vsync_start: u16,
    pub vsync_end: u16,
    pub vtotal: u16,
    pub hskew: u16,
    pub vscan: u16,
    pub vrefresh: u32,
    pub flags: ModeFlags,
    pub mode_type: ModeType,
}

impl DisplayMode {
    pub fn new(width: u16, height: u16, refresh: u32) -> Self {
        let htotal = width + 160;  // Simplified
        let vtotal = height + 40;
        let clock = (htotal as u32) * (vtotal as u32) * refresh / 1000;

        Self {
            name: alloc::format!("{}x{}@{}", width, height, refresh),
            clock,
            hdisplay: width,
            hsync_start: width + 48,
            hsync_end: width + 48 + 32,
            htotal,
            vdisplay: height,
            vsync_start: height + 3,
            vsync_end: height + 3 + 6,
            vtotal,
            hskew: 0,
            vscan: 0,
            vrefresh: refresh,
            flags: ModeFlags::empty(),
            mode_type: ModeType::PREFERRED,
        }
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct ModeFlags: u32 {
        const PHSYNC = 1 << 0;
        const NHSYNC = 1 << 1;
        const PVSYNC = 1 << 2;
        const NVSYNC = 1 << 3;
        const INTERLACE = 1 << 4;
        const DBLSCAN = 1 << 5;
        const CSYNC = 1 << 6;
        const PCSYNC = 1 << 7;
        const NCSYNC = 1 << 8;
        const HSKEW = 1 << 9;
        const DBLCLK = 1 << 12;
        const CLKDIV2 = 1 << 13;
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct ModeType: u32 {
        const PREFERRED = 1 << 3;
        const USERDEF = 1 << 5;
        const DRIVER = 1 << 6;
    }
}

/// Display configuration
#[derive(Debug, Clone)]
pub struct DisplayConfig {
    pub crtc_id: u32,
    pub connector_id: u32,
    pub encoder_id: u32,
    pub mode: DisplayMode,
    pub fb_id: u32,
}

/// Initialize display pipeline
pub fn init(mmio_base: u64) {
    if INITIALIZED.swap(true, Ordering::SeqCst) {
        return;
    }

    crate::kprintln!("display_pipe: Initializing display pipeline...");

    // Detect available display resources
    let crtcs = detect_crtcs(mmio_base);
    let planes = detect_planes(mmio_base, crtcs.len());
    let encoders = detect_encoders(mmio_base);
    let connectors = detect_connectors(mmio_base);

    crate::kprintln!("display_pipe: Found {} CRTCs, {} planes, {} encoders, {} connectors",
        crtcs.len(), planes.len(), encoders.len(), connectors.len());

    let state = DisplayState {
        mmio_base,
        crtcs,
        planes,
        encoders,
        connectors,
        active_configs: Vec::new(),
    };

    *DISPLAY_STATE.lock() = Some(state);

    crate::kprintln!("display_pipe: Display pipeline initialized");
}

/// Detect available CRTCs
fn detect_crtcs(mmio_base: u64) -> Vec<Crtc> {
    let mut crtcs = Vec::new();

    // Most Intel GPUs have 3-4 CRTCs (pipes)
    for (i, pipe) in [Pipe::A, Pipe::B, Pipe::C].iter().enumerate() {
        crtcs.push(Crtc {
            id: i as u32,
            pipe: *pipe,
            enabled: false,
            mode: None,
            gamma_size: 256,
            primary_plane: Some(i as u32),
            cursor_plane: Some(i as u32 + 10),
        });
    }

    crtcs
}

/// Detect available planes
fn detect_planes(mmio_base: u64, num_crtcs: usize) -> Vec<Plane> {
    let mut planes = Vec::new();
    let formats = vec![
        PixelFormat::Xrgb8888,
        PixelFormat::Argb8888,
        PixelFormat::Rgb565,
    ];

    // Primary planes (one per CRTC)
    for i in 0..num_crtcs {
        planes.push(Plane {
            id: i as u32,
            plane_type: PlaneType::Primary,
            possible_crtcs: 1 << i,
            formats: formats.clone(),
            enabled: false,
            fb_id: None,
            crtc_id: None,
            src_x: 0, src_y: 0, src_w: 0, src_h: 0,
            crtc_x: 0, crtc_y: 0, crtc_w: 0, crtc_h: 0,
            rotation: Rotation::Rotate0,
        });
    }

    // Overlay planes
    for i in 0..num_crtcs {
        planes.push(Plane {
            id: (num_crtcs + i) as u32,
            plane_type: PlaneType::Overlay,
            possible_crtcs: 1 << i,
            formats: formats.clone(),
            enabled: false,
            fb_id: None,
            crtc_id: None,
            src_x: 0, src_y: 0, src_w: 0, src_h: 0,
            crtc_x: 0, crtc_y: 0, crtc_w: 0, crtc_h: 0,
            rotation: Rotation::Rotate0,
        });
    }

    // Cursor planes
    for i in 0..num_crtcs {
        planes.push(Plane {
            id: (num_crtcs * 2 + i) as u32 + 10,
            plane_type: PlaneType::Cursor,
            possible_crtcs: 1 << i,
            formats: vec![PixelFormat::Argb8888],
            enabled: false,
            fb_id: None,
            crtc_id: None,
            src_x: 0, src_y: 0, src_w: 64, src_h: 64,
            crtc_x: 0, crtc_y: 0, crtc_w: 64, crtc_h: 64,
            rotation: Rotation::Rotate0,
        });
    }

    planes
}

/// Detect available encoders
fn detect_encoders(mmio_base: u64) -> Vec<Encoder> {
    vec![
        Encoder {
            id: 0,
            encoder_type: EncoderType::Dac,
            possible_crtcs: 0b111,
            possible_clones: 0,
            crtc_id: None,
        },
        Encoder {
            id: 1,
            encoder_type: EncoderType::Tmds,
            possible_crtcs: 0b111,
            possible_clones: 0,
            crtc_id: None,
        },
        Encoder {
            id: 2,
            encoder_type: EncoderType::Dpmst,
            possible_crtcs: 0b111,
            possible_clones: 0,
            crtc_id: None,
        },
        Encoder {
            id: 3,
            encoder_type: EncoderType::Lvds,
            possible_crtcs: 0b001, // Usually only pipe A
            possible_clones: 0,
            crtc_id: None,
        },
    ]
}

/// Detect available connectors
fn detect_connectors(mmio_base: u64) -> Vec<Connector> {
    vec![
        Connector {
            id: 0,
            connector_type: ConnectorType::Edp,
            connector_type_id: 1,
            connection: ConnectionStatus::Connected, // Laptop panel usually connected
            encoder_id: Some(3),
            possible_encoders: 1 << 3,
            modes: vec![
                DisplayMode::new(1920, 1080, 60),
                DisplayMode::new(1920, 1080, 48),
            ],
            edid: None,
            dpms: DpmsMode::On,
            properties: BTreeMap::new(),
        },
        Connector {
            id: 1,
            connector_type: ConnectorType::HdmiA,
            connector_type_id: 1,
            connection: ConnectionStatus::Disconnected,
            encoder_id: None,
            possible_encoders: 1 << 1,
            modes: Vec::new(),
            edid: None,
            dpms: DpmsMode::Off,
            properties: BTreeMap::new(),
        },
        Connector {
            id: 2,
            connector_type: ConnectorType::DisplayPort,
            connector_type_id: 1,
            connection: ConnectionStatus::Disconnected,
            encoder_id: None,
            possible_encoders: 1 << 2,
            modes: Vec::new(),
            edid: None,
            dpms: DpmsMode::Off,
            properties: BTreeMap::new(),
        },
    ]
}

/// Enable CRTC with specified mode
pub fn crtc_enable(crtc_id: u32, mode: &DisplayMode, fb_id: u32) -> bool {
    let mut state = DISPLAY_STATE.lock();
    let state = match state.as_mut() {
        Some(s) => s,
        None => return false,
    };

    let crtc = match state.crtcs.iter_mut().find(|c| c.id == crtc_id) {
        Some(c) => c,
        None => return false,
    };

    // Program display timings
    program_pipe_timings(state.mmio_base, crtc.pipe, mode);

    crtc.enabled = true;
    crtc.mode = Some(mode.clone());

    crate::kprintln!("display_pipe: Enabled CRTC {} with mode {}x{}@{}",
        crtc_id, mode.hdisplay, mode.vdisplay, mode.vrefresh);

    true
}

/// Disable CRTC
pub fn crtc_disable(crtc_id: u32) -> bool {
    let mut state = DISPLAY_STATE.lock();
    let state = match state.as_mut() {
        Some(s) => s,
        None => return false,
    };

    let crtc = match state.crtcs.iter_mut().find(|c| c.id == crtc_id) {
        Some(c) => c,
        None => return false,
    };

    crtc.enabled = false;
    crtc.mode = None;

    crate::kprintln!("display_pipe: Disabled CRTC {}", crtc_id);
    true
}

/// Program pipe display timings
fn program_pipe_timings(mmio_base: u64, pipe: Pipe, mode: &DisplayMode) {
    let pipe_offset = match pipe {
        Pipe::A => 0x60000,
        Pipe::B => 0x61000,
        Pipe::C => 0x62000,
        Pipe::D => 0x63000,
    };

    unsafe {
        let base = mmio_base + pipe_offset;

        // HTOTAL
        let htotal = ((mode.htotal as u32 - 1) << 16) | (mode.hdisplay as u32 - 1);
        core::ptr::write_volatile((base + 0x00) as *mut u32, htotal);

        // HBLANK
        let hblank = ((mode.htotal as u32 - 1) << 16) | (mode.hdisplay as u32 - 1);
        core::ptr::write_volatile((base + 0x04) as *mut u32, hblank);

        // HSYNC
        let hsync = ((mode.hsync_end as u32 - 1) << 16) | (mode.hsync_start as u32 - 1);
        core::ptr::write_volatile((base + 0x08) as *mut u32, hsync);

        // VTOTAL
        let vtotal = ((mode.vtotal as u32 - 1) << 16) | (mode.vdisplay as u32 - 1);
        core::ptr::write_volatile((base + 0x0C) as *mut u32, vtotal);

        // VBLANK
        let vblank = ((mode.vtotal as u32 - 1) << 16) | (mode.vdisplay as u32 - 1);
        core::ptr::write_volatile((base + 0x10) as *mut u32, vblank);

        // VSYNC
        let vsync = ((mode.vsync_end as u32 - 1) << 16) | (mode.vsync_start as u32 - 1);
        core::ptr::write_volatile((base + 0x14) as *mut u32, vsync);
    }
}

/// Update plane
pub fn plane_update(plane_id: u32, fb_id: u32, crtc_id: u32,
                    src_x: u32, src_y: u32, src_w: u32, src_h: u32,
                    crtc_x: i32, crtc_y: i32, crtc_w: u32, crtc_h: u32) -> bool {
    let mut state = DISPLAY_STATE.lock();
    let state = match state.as_mut() {
        Some(s) => s,
        None => return false,
    };

    let plane = match state.planes.iter_mut().find(|p| p.id == plane_id) {
        Some(p) => p,
        None => return false,
    };

    plane.fb_id = Some(fb_id);
    plane.crtc_id = Some(crtc_id);
    plane.src_x = src_x;
    plane.src_y = src_y;
    plane.src_w = src_w;
    plane.src_h = src_h;
    plane.crtc_x = crtc_x;
    plane.crtc_y = crtc_y;
    plane.crtc_w = crtc_w;
    plane.crtc_h = crtc_h;
    plane.enabled = true;

    true
}

/// Set connector DPMS mode
pub fn connector_set_dpms(connector_id: u32, mode: DpmsMode) -> bool {
    let mut state = DISPLAY_STATE.lock();
    let state = match state.as_mut() {
        Some(s) => s,
        None => return false,
    };

    let connector = match state.connectors.iter_mut().find(|c| c.id == connector_id) {
        Some(c) => c,
        None => return false,
    };

    connector.dpms = mode;
    true
}

/// Get connector status
pub fn connector_detect(connector_id: u32) -> Option<ConnectionStatus> {
    let state = DISPLAY_STATE.lock();
    let state = state.as_ref()?;

    state.connectors.iter()
        .find(|c| c.id == connector_id)
        .map(|c| c.connection)
}

/// Get connector modes
pub fn connector_get_modes(connector_id: u32) -> Vec<DisplayMode> {
    let state = DISPLAY_STATE.lock();
    match state.as_ref() {
        Some(s) => {
            s.connectors.iter()
                .find(|c| c.id == connector_id)
                .map(|c| c.modes.clone())
                .unwrap_or_default()
        }
        None => Vec::new(),
    }
}

/// Get all CRTCs
pub fn get_crtcs() -> Vec<Crtc> {
    let state = DISPLAY_STATE.lock();
    match state.as_ref() {
        Some(s) => s.crtcs.clone(),
        None => Vec::new(),
    }
}

/// Get all connectors
pub fn get_connectors() -> Vec<Connector> {
    let state = DISPLAY_STATE.lock();
    match state.as_ref() {
        Some(s) => s.connectors.clone(),
        None => Vec::new(),
    }
}

/// Get all planes
pub fn get_planes() -> Vec<Plane> {
    let state = DISPLAY_STATE.lock();
    match state.as_ref() {
        Some(s) => s.planes.clone(),
        None => Vec::new(),
    }
}
