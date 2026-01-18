//! XWayland Compatibility Layer
//!
//! Provides X11 compatibility for legacy applications:
//! - X11 window management
//! - X11 to Wayland surface mapping
//! - Input event translation
//! - Clipboard/Selection handling
//! - DRI3/Present for hardware acceleration

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use spin::Mutex;

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static XWAYLAND_STATE: Mutex<Option<XWaylandState>> = Mutex::new(None);
static NEXT_XID: AtomicU32 = AtomicU32::new(1);
static NEXT_ATOM: AtomicU32 = AtomicU32::new(1);

/// XWayland state
#[derive(Debug)]
pub struct XWaylandState {
    /// Display number
    pub display_number: u32,
    /// Connected X clients
    pub clients: BTreeMap<u32, XClient>,
    /// X windows
    pub windows: BTreeMap<u32, XWindow>,
    /// Pixmaps
    pub pixmaps: BTreeMap<u32, XPixmap>,
    /// Graphics contexts
    pub gcs: BTreeMap<u32, XGraphicsContext>,
    /// Atoms
    pub atoms: BTreeMap<u32, String>,
    /// Atom name to ID mapping
    pub atom_names: BTreeMap<String, u32>,
    /// Selections
    pub selections: BTreeMap<u32, XSelection>,
    /// Root window
    pub root_window: u32,
    /// Default colormap
    pub default_colormap: u32,
    /// Screen info
    pub screen: XScreen,
}

/// X client
#[derive(Debug)]
pub struct XClient {
    /// Client ID
    pub id: u32,
    /// Client resources (windows, pixmaps, etc.)
    pub resources: Vec<u32>,
    /// Event mask
    pub event_mask: u32,
    /// Sequence number
    pub sequence: u16,
    /// Connected Wayland surface (if any)
    pub wayland_surface: Option<u64>,
}

/// X Window
#[derive(Debug, Clone)]
pub struct XWindow {
    /// Window ID (XID)
    pub id: u32,
    /// Parent window
    pub parent: u32,
    /// Position
    pub x: i16,
    pub y: i16,
    /// Size
    pub width: u16,
    pub height: u16,
    /// Border width
    pub border_width: u16,
    /// Depth
    pub depth: u8,
    /// Window class
    pub class: WindowClass,
    /// Visual
    pub visual: u32,
    /// Event mask
    pub event_mask: u32,
    /// Attributes
    pub attributes: WindowAttributes,
    /// Mapped state
    pub mapped: bool,
    /// Override redirect
    pub override_redirect: bool,
    /// Children
    pub children: Vec<u32>,
    /// Properties
    pub properties: BTreeMap<u32, XProperty>,
    /// Wayland surface mapping
    pub wayland_surface: Option<u64>,
}

/// Window class
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowClass {
    CopyFromParent,
    InputOutput,
    InputOnly,
}

/// Window attributes
#[derive(Debug, Clone, Default)]
pub struct WindowAttributes {
    pub background_pixmap: Option<u32>,
    pub background_pixel: u32,
    pub border_pixmap: Option<u32>,
    pub border_pixel: u32,
    pub bit_gravity: u8,
    pub win_gravity: u8,
    pub backing_store: u8,
    pub backing_planes: u32,
    pub backing_pixel: u32,
    pub override_redirect: bool,
    pub save_under: bool,
    pub colormap: u32,
    pub cursor: u32,
}

/// X Pixmap
#[derive(Debug, Clone)]
pub struct XPixmap {
    /// Pixmap ID
    pub id: u32,
    /// Drawable (window)
    pub drawable: u32,
    /// Size
    pub width: u16,
    pub height: u16,
    /// Depth
    pub depth: u8,
    /// Buffer
    pub buffer: Option<u64>,
}

/// X Graphics Context
#[derive(Debug, Clone)]
pub struct XGraphicsContext {
    /// GC ID
    pub id: u32,
    /// Drawable
    pub drawable: u32,
    /// Function (copy, xor, etc.)
    pub function: GcFunction,
    /// Plane mask
    pub plane_mask: u32,
    /// Foreground
    pub foreground: u32,
    /// Background
    pub background: u32,
    /// Line width
    pub line_width: u16,
    /// Line style
    pub line_style: LineStyle,
    /// Cap style
    pub cap_style: CapStyle,
    /// Join style
    pub join_style: JoinStyle,
    /// Fill style
    pub fill_style: FillStyle,
    /// Fill rule
    pub fill_rule: FillRule,
    /// Arc mode
    pub arc_mode: ArcMode,
    /// Font
    pub font: u32,
    /// Subwindow mode
    pub subwindow_mode: SubwindowMode,
    /// Graphics exposures
    pub graphics_exposures: bool,
    /// Clip origin
    pub clip_x_origin: i16,
    pub clip_y_origin: i16,
    /// Clip mask
    pub clip_mask: Option<u32>,
    /// Dash offset
    pub dash_offset: u16,
    /// Dashes
    pub dashes: u8,
}

/// GC function
#[derive(Debug, Clone, Copy, Default)]
pub enum GcFunction {
    Clear,
    And,
    AndReverse,
    #[default]
    Copy,
    AndInverted,
    NoOp,
    Xor,
    Or,
    Nor,
    Equiv,
    Invert,
    OrReverse,
    CopyInverted,
    OrInverted,
    Nand,
    Set,
}

/// Line style
#[derive(Debug, Clone, Copy, Default)]
pub enum LineStyle {
    #[default]
    Solid,
    OnOffDash,
    DoubleDash,
}

/// Cap style
#[derive(Debug, Clone, Copy, Default)]
pub enum CapStyle {
    NotLast,
    #[default]
    Butt,
    Round,
    Projecting,
}

/// Join style
#[derive(Debug, Clone, Copy, Default)]
pub enum JoinStyle {
    #[default]
    Miter,
    Round,
    Bevel,
}

/// Fill style
#[derive(Debug, Clone, Copy, Default)]
pub enum FillStyle {
    #[default]
    Solid,
    Tiled,
    Stippled,
    OpaqueStippled,
}

/// Fill rule
#[derive(Debug, Clone, Copy, Default)]
pub enum FillRule {
    #[default]
    EvenOdd,
    Winding,
}

/// Arc mode
#[derive(Debug, Clone, Copy, Default)]
pub enum ArcMode {
    #[default]
    Chord,
    PieSlice,
}

/// Subwindow mode
#[derive(Debug, Clone, Copy, Default)]
pub enum SubwindowMode {
    #[default]
    ClipByChildren,
    IncludeInferiors,
}

/// X Property
#[derive(Debug, Clone)]
pub struct XProperty {
    /// Property atom
    pub atom: u32,
    /// Type atom
    pub property_type: u32,
    /// Format (8, 16, or 32)
    pub format: u8,
    /// Data
    pub data: Vec<u8>,
}

/// X Selection
#[derive(Debug, Clone)]
pub struct XSelection {
    /// Selection atom
    pub atom: u32,
    /// Owner window
    pub owner: u32,
    /// Last change time
    pub timestamp: u32,
}

/// X Screen
#[derive(Debug, Clone)]
pub struct XScreen {
    /// Root window
    pub root: u32,
    /// Default colormap
    pub default_colormap: u32,
    /// White pixel
    pub white_pixel: u32,
    /// Black pixel
    pub black_pixel: u32,
    /// Current input mask
    pub current_input_mask: u32,
    /// Width in pixels
    pub width: u16,
    /// Height in pixels
    pub height: u16,
    /// Width in mm
    pub width_mm: u16,
    /// Height in mm
    pub height_mm: u16,
    /// Min installed maps
    pub min_installed_maps: u16,
    /// Max installed maps
    pub max_installed_maps: u16,
    /// Root visual
    pub root_visual: u32,
    /// Backing stores
    pub backing_stores: u8,
    /// Save unders
    pub save_unders: bool,
    /// Root depth
    pub root_depth: u8,
    /// Allowed depths
    pub allowed_depths: Vec<XDepth>,
}

/// X Depth
#[derive(Debug, Clone)]
pub struct XDepth {
    /// Depth
    pub depth: u8,
    /// Visuals
    pub visuals: Vec<XVisual>,
}

/// X Visual
#[derive(Debug, Clone)]
pub struct XVisual {
    /// Visual ID
    pub visual_id: u32,
    /// Class
    pub class: VisualClass,
    /// Bits per RGB
    pub bits_per_rgb: u8,
    /// Colormap entries
    pub colormap_entries: u16,
    /// Red mask
    pub red_mask: u32,
    /// Green mask
    pub green_mask: u32,
    /// Blue mask
    pub blue_mask: u32,
}

/// Visual class
#[derive(Debug, Clone, Copy)]
pub enum VisualClass {
    StaticGray,
    GrayScale,
    StaticColor,
    PseudoColor,
    TrueColor,
    DirectColor,
}

/// X Event
#[derive(Debug, Clone)]
pub enum XEvent {
    KeyPress { window: u32, keycode: u8, state: u16, x: i16, y: i16 },
    KeyRelease { window: u32, keycode: u8, state: u16, x: i16, y: i16 },
    ButtonPress { window: u32, button: u8, state: u16, x: i16, y: i16 },
    ButtonRelease { window: u32, button: u8, state: u16, x: i16, y: i16 },
    MotionNotify { window: u32, state: u16, x: i16, y: i16 },
    EnterNotify { window: u32, mode: u8 },
    LeaveNotify { window: u32, mode: u8 },
    FocusIn { window: u32, mode: u8 },
    FocusOut { window: u32, mode: u8 },
    Expose { window: u32, x: u16, y: u16, width: u16, height: u16, count: u16 },
    MapNotify { window: u32, override_redirect: bool },
    UnmapNotify { window: u32 },
    DestroyNotify { window: u32 },
    ConfigureNotify { window: u32, x: i16, y: i16, width: u16, height: u16, border_width: u16, above_sibling: u32, override_redirect: bool },
    PropertyNotify { window: u32, atom: u32, state: u8 },
    SelectionClear { owner: u32, selection: u32 },
    SelectionRequest { owner: u32, requestor: u32, selection: u32, target: u32, property: u32 },
    SelectionNotify { requestor: u32, selection: u32, target: u32, property: u32 },
    ClientMessage { window: u32, message_type: u32, format: u8, data: [u8; 20] },
}

/// Event mask bits
pub mod event_mask {
    pub const KEY_PRESS: u32 = 1 << 0;
    pub const KEY_RELEASE: u32 = 1 << 1;
    pub const BUTTON_PRESS: u32 = 1 << 2;
    pub const BUTTON_RELEASE: u32 = 1 << 3;
    pub const ENTER_WINDOW: u32 = 1 << 4;
    pub const LEAVE_WINDOW: u32 = 1 << 5;
    pub const POINTER_MOTION: u32 = 1 << 6;
    pub const EXPOSURE: u32 = 1 << 15;
    pub const STRUCTURE_NOTIFY: u32 = 1 << 17;
    pub const SUBSTRUCTURE_NOTIFY: u32 = 1 << 19;
    pub const SUBSTRUCTURE_REDIRECT: u32 = 1 << 20;
    pub const FOCUS_CHANGE: u32 = 1 << 21;
    pub const PROPERTY_CHANGE: u32 = 1 << 22;
}

/// Standard atoms
pub mod atoms {
    pub const PRIMARY: u32 = 1;
    pub const SECONDARY: u32 = 2;
    pub const ARC: u32 = 3;
    pub const ATOM: u32 = 4;
    pub const BITMAP: u32 = 5;
    pub const CARDINAL: u32 = 6;
    pub const COLORMAP: u32 = 7;
    pub const CURSOR: u32 = 8;
    pub const INTEGER: u32 = 33;
    pub const STRING: u32 = 31;
    pub const UTF8_STRING: u32 = 315;
    pub const WM_NAME: u32 = 39;
    pub const WM_CLASS: u32 = 67;
    pub const WM_PROTOCOLS: u32 = 301;
    pub const WM_DELETE_WINDOW: u32 = 302;
    pub const WM_STATE: u32 = 303;
    pub const _NET_WM_NAME: u32 = 304;
    pub const _NET_WM_STATE: u32 = 305;
    pub const _NET_WM_WINDOW_TYPE: u32 = 306;
    pub const _NET_SUPPORTED: u32 = 307;
}

/// Error type
#[derive(Debug, Clone, Copy)]
pub enum XWaylandError {
    NotInitialized,
    InvalidWindow,
    InvalidPixmap,
    InvalidGc,
    InvalidAtom,
    BadValue,
    BadMatch,
    BadAlloc,
    BadAccess,
}

/// Initialize XWayland
pub fn init(display_number: u32, width: u16, height: u16) -> Result<(), XWaylandError> {
    if INITIALIZED.load(Ordering::Acquire) {
        return Ok(());
    }

    let root_window = NEXT_XID.fetch_add(1, Ordering::SeqCst);
    let default_colormap = NEXT_XID.fetch_add(1, Ordering::SeqCst);
    let root_visual = NEXT_XID.fetch_add(1, Ordering::SeqCst);

    let screen = XScreen {
        root: root_window,
        default_colormap,
        white_pixel: 0xFFFFFF,
        black_pixel: 0x000000,
        current_input_mask: 0,
        width,
        height,
        width_mm: (width as u32 * 254 / 96 / 10) as u16, // Approximate
        height_mm: (height as u32 * 254 / 96 / 10) as u16,
        min_installed_maps: 1,
        max_installed_maps: 1,
        root_visual,
        backing_stores: 0,
        save_unders: false,
        root_depth: 24,
        allowed_depths: vec![
            XDepth {
                depth: 24,
                visuals: vec![
                    XVisual {
                        visual_id: root_visual,
                        class: VisualClass::TrueColor,
                        bits_per_rgb: 8,
                        colormap_entries: 256,
                        red_mask: 0xFF0000,
                        green_mask: 0x00FF00,
                        blue_mask: 0x0000FF,
                    },
                ],
            },
            XDepth {
                depth: 32,
                visuals: vec![
                    XVisual {
                        visual_id: NEXT_XID.fetch_add(1, Ordering::SeqCst),
                        class: VisualClass::TrueColor,
                        bits_per_rgb: 8,
                        colormap_entries: 256,
                        red_mask: 0xFF0000,
                        green_mask: 0x00FF00,
                        blue_mask: 0x0000FF,
                    },
                ],
            },
        ],
    };

    // Create root window
    let root = XWindow {
        id: root_window,
        parent: 0,
        x: 0,
        y: 0,
        width,
        height,
        border_width: 0,
        depth: 24,
        class: WindowClass::InputOutput,
        visual: root_visual,
        event_mask: 0,
        attributes: WindowAttributes {
            background_pixel: 0x000000,
            colormap: default_colormap,
            ..Default::default()
        },
        mapped: true,
        override_redirect: false,
        children: Vec::new(),
        properties: BTreeMap::new(),
        wayland_surface: None,
    };

    let mut windows = BTreeMap::new();
    windows.insert(root_window, root);

    // Initialize standard atoms
    let mut atoms = BTreeMap::new();
    let mut atom_names = BTreeMap::new();

    let standard_atoms = [
        (atoms::PRIMARY, "PRIMARY"),
        (atoms::SECONDARY, "SECONDARY"),
        (atoms::ATOM, "ATOM"),
        (atoms::CARDINAL, "CARDINAL"),
        (atoms::INTEGER, "INTEGER"),
        (atoms::STRING, "STRING"),
        (atoms::WM_NAME, "WM_NAME"),
        (atoms::WM_CLASS, "WM_CLASS"),
        (atoms::WM_PROTOCOLS, "WM_PROTOCOLS"),
        (atoms::WM_DELETE_WINDOW, "WM_DELETE_WINDOW"),
        (atoms::WM_STATE, "WM_STATE"),
        (atoms::_NET_WM_NAME, "_NET_WM_NAME"),
        (atoms::_NET_WM_STATE, "_NET_WM_STATE"),
        (atoms::_NET_WM_WINDOW_TYPE, "_NET_WM_WINDOW_TYPE"),
        (atoms::_NET_SUPPORTED, "_NET_SUPPORTED"),
    ];

    for (id, name) in standard_atoms {
        atoms.insert(id, String::from(name));
        atom_names.insert(String::from(name), id);
    }

    let state = XWaylandState {
        display_number,
        clients: BTreeMap::new(),
        windows,
        pixmaps: BTreeMap::new(),
        gcs: BTreeMap::new(),
        atoms,
        atom_names,
        selections: BTreeMap::new(),
        root_window,
        default_colormap,
        screen,
    };

    *XWAYLAND_STATE.lock() = Some(state);
    INITIALIZED.store(true, Ordering::Release);

    crate::kprintln!("xwayland: XWayland on display :{} initialized ({}x{})", display_number, width, height);
    Ok(())
}

/// Create window
pub fn create_window(
    parent: u32,
    x: i16,
    y: i16,
    width: u16,
    height: u16,
    border_width: u16,
    depth: u8,
    class: WindowClass,
    visual: u32,
) -> Result<u32, XWaylandError> {
    let mut state = XWAYLAND_STATE.lock();
    let state = state.as_mut().ok_or(XWaylandError::NotInitialized)?;

    // Verify parent exists
    if !state.windows.contains_key(&parent) {
        return Err(XWaylandError::InvalidWindow);
    }

    let id = NEXT_XID.fetch_add(1, Ordering::SeqCst);

    let window = XWindow {
        id,
        parent,
        x,
        y,
        width,
        height,
        border_width,
        depth,
        class,
        visual,
        event_mask: 0,
        attributes: WindowAttributes::default(),
        mapped: false,
        override_redirect: false,
        children: Vec::new(),
        properties: BTreeMap::new(),
        wayland_surface: None,
    };

    state.windows.insert(id, window);

    // Add to parent's children
    if let Some(parent_win) = state.windows.get_mut(&parent) {
        parent_win.children.push(id);
    }

    Ok(id)
}

/// Destroy window
pub fn destroy_window(window: u32) -> Result<(), XWaylandError> {
    let mut state = XWAYLAND_STATE.lock();
    let state = state.as_mut().ok_or(XWaylandError::NotInitialized)?;

    // Remove from parent's children
    if let Some(win) = state.windows.get(&window) {
        let parent = win.parent;
        if let Some(parent_win) = state.windows.get_mut(&parent) {
            parent_win.children.retain(|&c| c != window);
        }
    }

    // Recursively destroy children
    if let Some(win) = state.windows.get(&window).cloned() {
        for child in win.children {
            let _ = destroy_window_internal(state, child);
        }
    }

    state.windows.remove(&window);
    Ok(())
}

fn destroy_window_internal(state: &mut XWaylandState, window: u32) -> Result<(), XWaylandError> {
    if let Some(win) = state.windows.get(&window).cloned() {
        for child in win.children {
            let _ = destroy_window_internal(state, child);
        }
    }
    state.windows.remove(&window);
    Ok(())
}

/// Map window
pub fn map_window(window: u32) -> Result<(), XWaylandError> {
    let mut state = XWAYLAND_STATE.lock();
    let state = state.as_mut().ok_or(XWaylandError::NotInitialized)?;

    let win = state.windows.get_mut(&window).ok_or(XWaylandError::InvalidWindow)?;

    if !win.mapped {
        win.mapped = true;

        // Create Wayland surface for top-level windows
        if win.parent == state.root_window && !win.override_redirect {
            // In real implementation, would create Wayland xdg_surface
        }
    }

    Ok(())
}

/// Unmap window
pub fn unmap_window(window: u32) -> Result<(), XWaylandError> {
    let mut state = XWAYLAND_STATE.lock();
    let state = state.as_mut().ok_or(XWaylandError::NotInitialized)?;

    let win = state.windows.get_mut(&window).ok_or(XWaylandError::InvalidWindow)?;
    win.mapped = false;

    Ok(())
}

/// Configure window
pub fn configure_window(
    window: u32,
    x: Option<i16>,
    y: Option<i16>,
    width: Option<u16>,
    height: Option<u16>,
    border_width: Option<u16>,
) -> Result<(), XWaylandError> {
    let mut state = XWAYLAND_STATE.lock();
    let state = state.as_mut().ok_or(XWaylandError::NotInitialized)?;

    let win = state.windows.get_mut(&window).ok_or(XWaylandError::InvalidWindow)?;

    if let Some(new_x) = x {
        win.x = new_x;
    }
    if let Some(new_y) = y {
        win.y = new_y;
    }
    if let Some(new_width) = width {
        win.width = new_width;
    }
    if let Some(new_height) = height {
        win.height = new_height;
    }
    if let Some(new_border) = border_width {
        win.border_width = new_border;
    }

    Ok(())
}

/// Change window attributes
pub fn change_window_attributes(window: u32, attributes: WindowAttributes) -> Result<(), XWaylandError> {
    let mut state = XWAYLAND_STATE.lock();
    let state = state.as_mut().ok_or(XWaylandError::NotInitialized)?;

    let win = state.windows.get_mut(&window).ok_or(XWaylandError::InvalidWindow)?;
    win.attributes = attributes;

    Ok(())
}

/// Get window attributes
pub fn get_window_attributes(window: u32) -> Result<WindowAttributes, XWaylandError> {
    let state = XWAYLAND_STATE.lock();
    let state = state.as_ref().ok_or(XWaylandError::NotInitialized)?;

    state.windows
        .get(&window)
        .map(|w| w.attributes.clone())
        .ok_or(XWaylandError::InvalidWindow)
}

/// Change property
pub fn change_property(
    window: u32,
    property: u32,
    property_type: u32,
    format: u8,
    data: &[u8],
) -> Result<(), XWaylandError> {
    let mut state = XWAYLAND_STATE.lock();
    let state = state.as_mut().ok_or(XWaylandError::NotInitialized)?;

    let win = state.windows.get_mut(&window).ok_or(XWaylandError::InvalidWindow)?;

    let prop = XProperty {
        atom: property,
        property_type,
        format,
        data: data.to_vec(),
    };

    win.properties.insert(property, prop);

    Ok(())
}

/// Get property
pub fn get_property(window: u32, property: u32) -> Result<Option<XProperty>, XWaylandError> {
    let state = XWAYLAND_STATE.lock();
    let state = state.as_ref().ok_or(XWaylandError::NotInitialized)?;

    let win = state.windows.get(&window).ok_or(XWaylandError::InvalidWindow)?;

    Ok(win.properties.get(&property).cloned())
}

/// Delete property
pub fn delete_property(window: u32, property: u32) -> Result<(), XWaylandError> {
    let mut state = XWAYLAND_STATE.lock();
    let state = state.as_mut().ok_or(XWaylandError::NotInitialized)?;

    let win = state.windows.get_mut(&window).ok_or(XWaylandError::InvalidWindow)?;
    win.properties.remove(&property);

    Ok(())
}

/// Intern atom
pub fn intern_atom(name: &str, only_if_exists: bool) -> Result<u32, XWaylandError> {
    let mut state = XWAYLAND_STATE.lock();
    let state = state.as_mut().ok_or(XWaylandError::NotInitialized)?;

    // Check if atom already exists
    if let Some(&atom) = state.atom_names.get(name) {
        return Ok(atom);
    }

    if only_if_exists {
        return Err(XWaylandError::InvalidAtom);
    }

    // Create new atom
    let atom = NEXT_ATOM.fetch_add(1, Ordering::SeqCst) + 1000; // Offset to avoid conflicts
    state.atoms.insert(atom, String::from(name));
    state.atom_names.insert(String::from(name), atom);

    Ok(atom)
}

/// Get atom name
pub fn get_atom_name(atom: u32) -> Result<String, XWaylandError> {
    let state = XWAYLAND_STATE.lock();
    let state = state.as_ref().ok_or(XWaylandError::NotInitialized)?;

    state.atoms
        .get(&atom)
        .cloned()
        .ok_or(XWaylandError::InvalidAtom)
}

/// Create pixmap
pub fn create_pixmap(drawable: u32, width: u16, height: u16, depth: u8) -> Result<u32, XWaylandError> {
    let mut state = XWAYLAND_STATE.lock();
    let state = state.as_mut().ok_or(XWaylandError::NotInitialized)?;

    let id = NEXT_XID.fetch_add(1, Ordering::SeqCst);

    let pixmap = XPixmap {
        id,
        drawable,
        width,
        height,
        depth,
        buffer: None,
    };

    state.pixmaps.insert(id, pixmap);

    Ok(id)
}

/// Free pixmap
pub fn free_pixmap(pixmap: u32) -> Result<(), XWaylandError> {
    let mut state = XWAYLAND_STATE.lock();
    let state = state.as_mut().ok_or(XWaylandError::NotInitialized)?;

    state.pixmaps.remove(&pixmap).ok_or(XWaylandError::InvalidPixmap)?;

    Ok(())
}

/// Create graphics context
pub fn create_gc(drawable: u32) -> Result<u32, XWaylandError> {
    let mut state = XWAYLAND_STATE.lock();
    let state = state.as_mut().ok_or(XWaylandError::NotInitialized)?;

    let id = NEXT_XID.fetch_add(1, Ordering::SeqCst);

    let gc = XGraphicsContext {
        id,
        drawable,
        function: GcFunction::Copy,
        plane_mask: !0,
        foreground: 0,
        background: 0xFFFFFF,
        line_width: 0,
        line_style: LineStyle::Solid,
        cap_style: CapStyle::Butt,
        join_style: JoinStyle::Miter,
        fill_style: FillStyle::Solid,
        fill_rule: FillRule::EvenOdd,
        arc_mode: ArcMode::PieSlice,
        font: 0,
        subwindow_mode: SubwindowMode::ClipByChildren,
        graphics_exposures: true,
        clip_x_origin: 0,
        clip_y_origin: 0,
        clip_mask: None,
        dash_offset: 0,
        dashes: 4,
    };

    state.gcs.insert(id, gc);

    Ok(id)
}

/// Free graphics context
pub fn free_gc(gc: u32) -> Result<(), XWaylandError> {
    let mut state = XWAYLAND_STATE.lock();
    let state = state.as_mut().ok_or(XWaylandError::NotInitialized)?;

    state.gcs.remove(&gc).ok_or(XWaylandError::InvalidGc)?;

    Ok(())
}

/// Set selection owner
pub fn set_selection_owner(selection: u32, owner: u32, timestamp: u32) -> Result<(), XWaylandError> {
    let mut state = XWAYLAND_STATE.lock();
    let state = state.as_mut().ok_or(XWaylandError::NotInitialized)?;

    if owner != 0 && !state.windows.contains_key(&owner) {
        return Err(XWaylandError::InvalidWindow);
    }

    if owner == 0 {
        state.selections.remove(&selection);
    } else {
        state.selections.insert(selection, XSelection {
            atom: selection,
            owner,
            timestamp,
        });
    }

    Ok(())
}

/// Get selection owner
pub fn get_selection_owner(selection: u32) -> Result<u32, XWaylandError> {
    let state = XWAYLAND_STATE.lock();
    let state = state.as_ref().ok_or(XWaylandError::NotInitialized)?;

    Ok(state.selections.get(&selection).map(|s| s.owner).unwrap_or(0))
}

/// Convert input event from Wayland to X11
pub fn convert_input_event(wayland_event: WaylandInputEvent) -> Option<XEvent> {
    match wayland_event {
        WaylandInputEvent::Key { window, keycode, pressed } => {
            if pressed {
                Some(XEvent::KeyPress { window, keycode, state: 0, x: 0, y: 0 })
            } else {
                Some(XEvent::KeyRelease { window, keycode, state: 0, x: 0, y: 0 })
            }
        }
        WaylandInputEvent::Button { window, button, pressed, x, y } => {
            if pressed {
                Some(XEvent::ButtonPress { window, button, state: 0, x, y })
            } else {
                Some(XEvent::ButtonRelease { window, button, state: 0, x, y })
            }
        }
        WaylandInputEvent::Motion { window, x, y } => {
            Some(XEvent::MotionNotify { window, state: 0, x, y })
        }
        WaylandInputEvent::Enter { window } => {
            Some(XEvent::EnterNotify { window, mode: 0 })
        }
        WaylandInputEvent::Leave { window } => {
            Some(XEvent::LeaveNotify { window, mode: 0 })
        }
    }
}

/// Wayland input event (for conversion)
#[derive(Debug, Clone)]
pub enum WaylandInputEvent {
    Key { window: u32, keycode: u8, pressed: bool },
    Button { window: u32, button: u8, pressed: bool, x: i16, y: i16 },
    Motion { window: u32, x: i16, y: i16 },
    Enter { window: u32 },
    Leave { window: u32 },
}

/// Map X window to Wayland surface
pub fn map_to_wayland_surface(window: u32, wayland_surface: u64) -> Result<(), XWaylandError> {
    let mut state = XWAYLAND_STATE.lock();
    let state = state.as_mut().ok_or(XWaylandError::NotInitialized)?;

    let win = state.windows.get_mut(&window).ok_or(XWaylandError::InvalidWindow)?;
    win.wayland_surface = Some(wayland_surface);

    Ok(())
}

/// Get Wayland surface for X window
pub fn get_wayland_surface(window: u32) -> Result<Option<u64>, XWaylandError> {
    let state = XWAYLAND_STATE.lock();
    let state = state.as_ref().ok_or(XWaylandError::NotInitialized)?;

    state.windows
        .get(&window)
        .map(|w| w.wayland_surface)
        .ok_or(XWaylandError::InvalidWindow)
}

/// Get root window
pub fn get_root_window() -> Result<u32, XWaylandError> {
    let state = XWAYLAND_STATE.lock();
    let state = state.as_ref().ok_or(XWaylandError::NotInitialized)?;

    Ok(state.root_window)
}

/// Get screen info
pub fn get_screen() -> Result<XScreen, XWaylandError> {
    let state = XWAYLAND_STATE.lock();
    let state = state.as_ref().ok_or(XWaylandError::NotInitialized)?;

    Ok(state.screen.clone())
}

/// Get window count
pub fn get_window_count() -> usize {
    XWAYLAND_STATE
        .lock()
        .as_ref()
        .map(|s| s.windows.len())
        .unwrap_or(0)
}

/// List children of window
pub fn query_tree(window: u32) -> Result<(u32, u32, Vec<u32>), XWaylandError> {
    let state = XWAYLAND_STATE.lock();
    let state = state.as_ref().ok_or(XWaylandError::NotInitialized)?;

    let win = state.windows.get(&window).ok_or(XWaylandError::InvalidWindow)?;

    Ok((state.root_window, win.parent, win.children.clone()))
}
