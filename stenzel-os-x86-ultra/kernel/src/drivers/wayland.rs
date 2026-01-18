//! Wayland Compositor Support
//!
//! Provides kernel-side support for Wayland compositor:
//! - Buffer management (wl_buffer, dmabuf)
//! - Surface composition
//! - Input event routing
//! - DRM/KMS integration
//! - Shared memory support

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use spin::Mutex;

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static WAYLAND_STATE: Mutex<Option<WaylandState>> = Mutex::new(None);
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Wayland state
#[derive(Debug)]
pub struct WaylandState {
    /// Compositor instances
    pub compositors: BTreeMap<u64, Compositor>,
    /// Surfaces
    pub surfaces: BTreeMap<u64, Surface>,
    /// Buffers
    pub buffers: BTreeMap<u64, Buffer>,
    /// Outputs (displays)
    pub outputs: BTreeMap<u64, Output>,
    /// Seats (input devices)
    pub seats: BTreeMap<u64, Seat>,
    /// Global objects
    pub globals: Vec<Global>,
}

/// Compositor instance
#[derive(Debug)]
pub struct Compositor {
    /// Compositor ID
    pub id: u64,
    /// Process ID
    pub pid: u32,
    /// Socket path
    pub socket_path: String,
    /// Connected clients
    pub clients: Vec<Client>,
    /// Active output
    pub active_output: Option<u64>,
    /// Render backend
    pub backend: RenderBackend,
}

/// Wayland client
#[derive(Debug, Clone)]
pub struct Client {
    /// Client ID
    pub id: u64,
    /// Process ID
    pub pid: u32,
    /// Client socket fd
    pub socket_fd: u32,
    /// Owned objects
    pub objects: Vec<u64>,
    /// Focus surface
    pub focus_surface: Option<u64>,
}

/// Surface
#[derive(Debug)]
pub struct Surface {
    /// Surface ID
    pub id: u64,
    /// Client ID
    pub client_id: u64,
    /// Current buffer
    pub buffer: Option<u64>,
    /// Pending buffer
    pub pending_buffer: Option<u64>,
    /// Surface state
    pub state: SurfaceState,
    /// Input region
    pub input_region: Option<Region>,
    /// Opaque region
    pub opaque_region: Option<Region>,
    /// Damage region
    pub damage: Vec<Rect>,
    /// Frame callbacks
    pub frame_callbacks: Vec<u32>,
    /// Subsurfaces
    pub subsurfaces: Vec<Subsurface>,
    /// Role
    pub role: SurfaceRole,
    /// Transform
    pub transform: Transform,
    /// Scale factor
    pub scale: i32,
}

/// Surface state
#[derive(Debug, Clone, Default)]
pub struct SurfaceState {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub visible: bool,
    pub activated: bool,
    pub maximized: bool,
    pub fullscreen: bool,
    pub resizing: bool,
}

/// Surface role
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceRole {
    None,
    XdgToplevel,
    XdgPopup,
    Subsurface,
    Cursor,
    DragIcon,
    LayerSurface,
}

/// Subsurface
#[derive(Debug, Clone)]
pub struct Subsurface {
    pub surface_id: u64,
    pub parent_id: u64,
    pub x: i32,
    pub y: i32,
    pub sync: bool,
}

/// Transform
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transform {
    Normal,
    Rotate90,
    Rotate180,
    Rotate270,
    Flipped,
    Flipped90,
    Flipped180,
    Flipped270,
}

/// Region
#[derive(Debug, Clone)]
pub struct Region {
    pub rects: Vec<Rect>,
}

/// Rectangle
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Buffer
#[derive(Debug, Clone)]
pub struct Buffer {
    /// Buffer ID
    pub id: u64,
    /// Buffer type
    pub buffer_type: BufferType,
    /// Width
    pub width: u32,
    /// Height
    pub height: u32,
    /// Stride (bytes per row)
    pub stride: u32,
    /// Format
    pub format: BufferFormat,
    /// Data pointer (for shm)
    pub data_ptr: Option<u64>,
    /// DMA-BUF fd (for dmabuf)
    pub dmabuf_fd: Option<i32>,
    /// GEM handle
    pub gem_handle: Option<u32>,
    /// Modifier
    pub modifier: u64,
    /// Busy (being rendered)
    pub busy: bool,
}

/// Buffer type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferType {
    Shm,
    DmaBuf,
}

/// Buffer format (wl_shm formats)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferFormat {
    Argb8888,
    Xrgb8888,
    Rgb888,
    Rgb565,
    Abgr8888,
    Xbgr8888,
    Bgr888,
    Nv12,
    Yuyv,
}

/// Output (display)
#[derive(Debug, Clone)]
pub struct Output {
    /// Output ID
    pub id: u64,
    /// Name
    pub name: String,
    /// Make
    pub make: String,
    /// Model
    pub model: String,
    /// Physical size (mm)
    pub physical_width: u32,
    pub physical_height: u32,
    /// Position
    pub x: i32,
    pub y: i32,
    /// Current mode
    pub current_mode: OutputMode,
    /// Available modes
    pub modes: Vec<OutputMode>,
    /// Transform
    pub transform: Transform,
    /// Scale
    pub scale: i32,
    /// CRTC ID
    pub crtc_id: u32,
    /// Connector ID
    pub connector_id: u32,
}

/// Output mode
#[derive(Debug, Clone, Copy)]
pub struct OutputMode {
    pub width: u32,
    pub height: u32,
    pub refresh: u32, // mHz
    pub preferred: bool,
    pub current: bool,
}

/// Seat (input devices)
#[derive(Debug)]
pub struct Seat {
    /// Seat ID
    pub id: u64,
    /// Name
    pub name: String,
    /// Capabilities
    pub capabilities: SeatCapabilities,
    /// Keyboard state
    pub keyboard: Option<KeyboardState>,
    /// Pointer state
    pub pointer: Option<PointerState>,
    /// Touch state
    pub touch: Option<TouchState>,
    /// Focus client
    pub focus_client: Option<u64>,
}

/// Seat capabilities
#[derive(Debug, Clone, Copy, Default)]
pub struct SeatCapabilities {
    pub keyboard: bool,
    pub pointer: bool,
    pub touch: bool,
}

/// Keyboard state
#[derive(Debug, Clone)]
pub struct KeyboardState {
    pub keymap_format: KeymapFormat,
    pub keymap_fd: i32,
    pub keymap_size: u32,
    pub pressed_keys: Vec<u32>,
    pub modifiers: Modifiers,
    pub repeat_rate: i32,
    pub repeat_delay: i32,
}

/// Keymap format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeymapFormat {
    NoKeymap,
    XkbV1,
}

/// Keyboard modifiers
#[derive(Debug, Clone, Copy, Default)]
pub struct Modifiers {
    pub mods_depressed: u32,
    pub mods_latched: u32,
    pub mods_locked: u32,
    pub group: u32,
}

/// Pointer state
#[derive(Debug, Clone)]
pub struct PointerState {
    pub x: f64,
    pub y: f64,
    pub focus_surface: Option<u64>,
    pub buttons_pressed: Vec<u32>,
    pub cursor_surface: Option<u64>,
    pub cursor_hotspot_x: i32,
    pub cursor_hotspot_y: i32,
}

/// Touch state
#[derive(Debug, Clone)]
pub struct TouchState {
    pub focus_surface: Option<u64>,
    pub touches: BTreeMap<i32, TouchPoint>,
}

/// Touch point
#[derive(Debug, Clone, Copy)]
pub struct TouchPoint {
    pub id: i32,
    pub x: f64,
    pub y: f64,
}

/// Global object
#[derive(Debug, Clone)]
pub struct Global {
    pub name: u32,
    pub interface: String,
    pub version: u32,
}

/// Render backend
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderBackend {
    DrmKms,
    Fbdev,
    Headless,
}

/// Error type
#[derive(Debug, Clone, Copy)]
pub enum WaylandError {
    NotInitialized,
    InvalidCompositor,
    InvalidSurface,
    InvalidBuffer,
    InvalidSeat,
    InvalidOutput,
    InvalidClient,
    BufferBusy,
    NoMemory,
    ProtocolError,
}

/// Initialize Wayland support
pub fn init() -> Result<(), WaylandError> {
    if INITIALIZED.load(Ordering::Acquire) {
        return Ok(());
    }

    // Create default globals
    let globals = vec![
        Global { name: 1, interface: String::from("wl_compositor"), version: 5 },
        Global { name: 2, interface: String::from("wl_subcompositor"), version: 1 },
        Global { name: 3, interface: String::from("wl_shm"), version: 1 },
        Global { name: 4, interface: String::from("wl_output"), version: 4 },
        Global { name: 5, interface: String::from("wl_seat"), version: 8 },
        Global { name: 6, interface: String::from("wl_data_device_manager"), version: 3 },
        Global { name: 7, interface: String::from("xdg_wm_base"), version: 4 },
        Global { name: 8, interface: String::from("zwp_linux_dmabuf_v1"), version: 4 },
        Global { name: 9, interface: String::from("zwlr_layer_shell_v1"), version: 4 },
        Global { name: 10, interface: String::from("zwp_pointer_constraints_v1"), version: 1 },
        Global { name: 11, interface: String::from("zwp_relative_pointer_manager_v1"), version: 1 },
        Global { name: 12, interface: String::from("wp_viewporter"), version: 1 },
        Global { name: 13, interface: String::from("wp_presentation"), version: 1 },
    ];

    let state = WaylandState {
        compositors: BTreeMap::new(),
        surfaces: BTreeMap::new(),
        buffers: BTreeMap::new(),
        outputs: BTreeMap::new(),
        seats: BTreeMap::new(),
        globals,
    };

    *WAYLAND_STATE.lock() = Some(state);
    INITIALIZED.store(true, Ordering::Release);

    crate::kprintln!("wayland: Wayland compositor support initialized");
    Ok(())
}

/// Create compositor instance
pub fn create_compositor(pid: u32, socket_path: &str, backend: RenderBackend) -> Result<u64, WaylandError> {
    let mut state = WAYLAND_STATE.lock();
    let state = state.as_mut().ok_or(WaylandError::NotInitialized)?;

    let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);

    let compositor = Compositor {
        id,
        pid,
        socket_path: String::from(socket_path),
        clients: Vec::new(),
        active_output: None,
        backend,
    };

    state.compositors.insert(id, compositor);

    crate::kprintln!("wayland: Created compositor {} on {}", id, socket_path);
    Ok(id)
}

/// Register client connection
pub fn register_client(compositor_id: u64, pid: u32, socket_fd: u32) -> Result<u64, WaylandError> {
    let mut state = WAYLAND_STATE.lock();
    let state = state.as_mut().ok_or(WaylandError::NotInitialized)?;

    let compositor = state.compositors.get_mut(&compositor_id).ok_or(WaylandError::InvalidCompositor)?;

    let client_id = NEXT_ID.fetch_add(1, Ordering::SeqCst);

    let client = Client {
        id: client_id,
        pid,
        socket_fd,
        objects: Vec::new(),
        focus_surface: None,
    };

    compositor.clients.push(client);

    Ok(client_id)
}

/// Create surface
pub fn create_surface(client_id: u64) -> Result<u64, WaylandError> {
    let mut state = WAYLAND_STATE.lock();
    let state = state.as_mut().ok_or(WaylandError::NotInitialized)?;

    let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);

    let surface = Surface {
        id,
        client_id,
        buffer: None,
        pending_buffer: None,
        state: SurfaceState::default(),
        input_region: None,
        opaque_region: None,
        damage: Vec::new(),
        frame_callbacks: Vec::new(),
        subsurfaces: Vec::new(),
        role: SurfaceRole::None,
        transform: Transform::Normal,
        scale: 1,
    };

    state.surfaces.insert(id, surface);

    Ok(id)
}

/// Attach buffer to surface
pub fn attach_buffer(surface_id: u64, buffer_id: u64, x: i32, y: i32) -> Result<(), WaylandError> {
    let mut state = WAYLAND_STATE.lock();
    let state = state.as_mut().ok_or(WaylandError::NotInitialized)?;

    let surface = state.surfaces.get_mut(&surface_id).ok_or(WaylandError::InvalidSurface)?;

    // Verify buffer exists
    if !state.buffers.contains_key(&buffer_id) {
        return Err(WaylandError::InvalidBuffer);
    }

    surface.pending_buffer = Some(buffer_id);
    surface.state.x = x;
    surface.state.y = y;

    Ok(())
}

/// Commit surface state
pub fn commit_surface(surface_id: u64) -> Result<(), WaylandError> {
    let mut state = WAYLAND_STATE.lock();
    let state = state.as_mut().ok_or(WaylandError::NotInitialized)?;

    let surface = state.surfaces.get_mut(&surface_id).ok_or(WaylandError::InvalidSurface)?;

    // Release old buffer if any
    if let Some(old_buffer) = surface.buffer {
        if let Some(buf) = state.buffers.get_mut(&old_buffer) {
            buf.busy = false;
        }
    }

    // Apply pending state
    surface.buffer = surface.pending_buffer.take();

    // Mark new buffer as busy
    if let Some(new_buffer) = surface.buffer {
        if let Some(buf) = state.buffers.get_mut(&new_buffer) {
            buf.busy = true;
        }
    }

    // Apply buffer dimensions to surface state
    if let Some(buf_id) = surface.buffer {
        if let Some(buf) = state.buffers.get(&buf_id) {
            surface.state.width = buf.width;
            surface.state.height = buf.height;
            surface.state.visible = true;
        }
    }

    Ok(())
}

/// Create shared memory buffer
pub fn create_shm_buffer(
    width: u32,
    height: u32,
    stride: u32,
    format: BufferFormat,
    data_ptr: u64,
) -> Result<u64, WaylandError> {
    let mut state = WAYLAND_STATE.lock();
    let state = state.as_mut().ok_or(WaylandError::NotInitialized)?;

    let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);

    let buffer = Buffer {
        id,
        buffer_type: BufferType::Shm,
        width,
        height,
        stride,
        format,
        data_ptr: Some(data_ptr),
        dmabuf_fd: None,
        gem_handle: None,
        modifier: 0,
        busy: false,
    };

    state.buffers.insert(id, buffer);

    Ok(id)
}

/// Create DMA-BUF buffer
pub fn create_dmabuf(
    width: u32,
    height: u32,
    format: BufferFormat,
    modifier: u64,
    gem_handle: u32,
    stride: u32,
) -> Result<u64, WaylandError> {
    let mut state = WAYLAND_STATE.lock();
    let state = state.as_mut().ok_or(WaylandError::NotInitialized)?;

    let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);

    let buffer = Buffer {
        id,
        buffer_type: BufferType::DmaBuf,
        width,
        height,
        stride,
        format,
        data_ptr: None,
        dmabuf_fd: None,
        gem_handle: Some(gem_handle),
        modifier,
        busy: false,
    };

    state.buffers.insert(id, buffer);

    Ok(id)
}

/// Register output
pub fn register_output(
    name: &str,
    make: &str,
    model: &str,
    physical_width: u32,
    physical_height: u32,
    crtc_id: u32,
    connector_id: u32,
    modes: Vec<OutputMode>,
) -> Result<u64, WaylandError> {
    let mut state = WAYLAND_STATE.lock();
    let state = state.as_mut().ok_or(WaylandError::NotInitialized)?;

    let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);

    let current_mode = modes.iter().find(|m| m.current).copied().unwrap_or(OutputMode {
        width: 1920,
        height: 1080,
        refresh: 60000,
        preferred: true,
        current: true,
    });

    let output = Output {
        id,
        name: String::from(name),
        make: String::from(make),
        model: String::from(model),
        physical_width,
        physical_height,
        x: 0,
        y: 0,
        current_mode,
        modes,
        transform: Transform::Normal,
        scale: 1,
        crtc_id,
        connector_id,
    };

    state.outputs.insert(id, output);

    Ok(id)
}

/// Register seat
pub fn register_seat(name: &str, capabilities: SeatCapabilities) -> Result<u64, WaylandError> {
    let mut state = WAYLAND_STATE.lock();
    let state = state.as_mut().ok_or(WaylandError::NotInitialized)?;

    let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);

    let seat = Seat {
        id,
        name: String::from(name),
        capabilities,
        keyboard: if capabilities.keyboard {
            Some(KeyboardState {
                keymap_format: KeymapFormat::XkbV1,
                keymap_fd: -1,
                keymap_size: 0,
                pressed_keys: Vec::new(),
                modifiers: Modifiers::default(),
                repeat_rate: 25,
                repeat_delay: 600,
            })
        } else {
            None
        },
        pointer: if capabilities.pointer {
            Some(PointerState {
                x: 0.0,
                y: 0.0,
                focus_surface: None,
                buttons_pressed: Vec::new(),
                cursor_surface: None,
                cursor_hotspot_x: 0,
                cursor_hotspot_y: 0,
            })
        } else {
            None
        },
        touch: if capabilities.touch {
            Some(TouchState {
                focus_surface: None,
                touches: BTreeMap::new(),
            })
        } else {
            None
        },
        focus_client: None,
    };

    state.seats.insert(id, seat);

    Ok(id)
}

/// Send keyboard key event
pub fn send_key_event(seat_id: u64, surface_id: u64, key: u32, state_val: u32, serial: u32) -> Result<(), WaylandError> {
    let mut state = WAYLAND_STATE.lock();
    let state_ref = state.as_mut().ok_or(WaylandError::NotInitialized)?;

    let seat = state_ref.seats.get_mut(&seat_id).ok_or(WaylandError::InvalidSeat)?;
    let keyboard = seat.keyboard.as_mut().ok_or(WaylandError::InvalidSeat)?;

    if state_val == 1 {
        // Key pressed
        if !keyboard.pressed_keys.contains(&key) {
            keyboard.pressed_keys.push(key);
        }
    } else {
        // Key released
        keyboard.pressed_keys.retain(|&k| k != key);
    }

    // In real implementation, this would send wl_keyboard.key event to client
    Ok(())
}

/// Send pointer motion event
pub fn send_pointer_motion(seat_id: u64, surface_id: u64, x: f64, y: f64) -> Result<(), WaylandError> {
    let mut state = WAYLAND_STATE.lock();
    let state_ref = state.as_mut().ok_or(WaylandError::NotInitialized)?;

    let seat = state_ref.seats.get_mut(&seat_id).ok_or(WaylandError::InvalidSeat)?;
    let pointer = seat.pointer.as_mut().ok_or(WaylandError::InvalidSeat)?;

    pointer.x = x;
    pointer.y = y;
    pointer.focus_surface = Some(surface_id);

    // In real implementation, this would send wl_pointer.motion event to client
    Ok(())
}

/// Send pointer button event
pub fn send_pointer_button(seat_id: u64, button: u32, state_val: u32, serial: u32) -> Result<(), WaylandError> {
    let mut state = WAYLAND_STATE.lock();
    let state_ref = state.as_mut().ok_or(WaylandError::NotInitialized)?;

    let seat = state_ref.seats.get_mut(&seat_id).ok_or(WaylandError::InvalidSeat)?;
    let pointer = seat.pointer.as_mut().ok_or(WaylandError::InvalidSeat)?;

    if state_val == 1 {
        // Button pressed
        if !pointer.buttons_pressed.contains(&button) {
            pointer.buttons_pressed.push(button);
        }
    } else {
        // Button released
        pointer.buttons_pressed.retain(|&b| b != button);
    }

    // In real implementation, this would send wl_pointer.button event to client
    Ok(())
}

/// Get surface list for rendering
pub fn get_visible_surfaces() -> Vec<(u64, SurfaceState)> {
    WAYLAND_STATE
        .lock()
        .as_ref()
        .map(|s| {
            s.surfaces
                .iter()
                .filter(|(_, surf)| surf.state.visible && surf.buffer.is_some())
                .map(|(id, surf)| (*id, surf.state.clone()))
                .collect()
        })
        .unwrap_or_default()
}

/// Get compositor count
pub fn compositor_count() -> usize {
    WAYLAND_STATE
        .lock()
        .as_ref()
        .map(|s| s.compositors.len())
        .unwrap_or(0)
}

/// Get surface count
pub fn surface_count() -> usize {
    WAYLAND_STATE
        .lock()
        .as_ref()
        .map(|s| s.surfaces.len())
        .unwrap_or(0)
}
