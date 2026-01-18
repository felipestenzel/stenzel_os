//! Window Manager
//!
//! Advanced window management features:
//! - Window snapping (edges, corners, quarters)
//! - Virtual desktops/workspaces
//! - Alt+Tab window switcher
//! - Drag and drop handling
//! - Window tiling
//! - Picture-in-Picture

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use spin::Mutex;

use super::window::WindowId;

static WM_STATE: Mutex<Option<WindowManagerState>> = Mutex::new(None);
static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Window Manager configuration
#[derive(Debug, Clone)]
pub struct WmConfig {
    /// Enable window snapping
    pub snapping_enabled: bool,
    /// Snap threshold in pixels
    pub snap_threshold: u32,
    /// Edge snap zones
    pub edge_snap: bool,
    /// Corner snap zones
    pub corner_snap: bool,
    /// Number of virtual desktops
    pub num_desktops: u32,
    /// Enable tiling mode
    pub tiling_enabled: bool,
    /// Tiling gaps
    pub tiling_gap: u32,
    /// Alt+Tab shows thumbnails
    pub switcher_thumbnails: bool,
    /// Alt+Tab preview delay (ms)
    pub switcher_preview_delay: u32,
    /// Enable PiP mode
    pub pip_enabled: bool,
    /// PiP window size (percentage of screen)
    pub pip_size: u32,
}

impl Default for WmConfig {
    fn default() -> Self {
        Self {
            snapping_enabled: true,
            snap_threshold: 20,
            edge_snap: true,
            corner_snap: true,
            num_desktops: 4,
            tiling_enabled: false,
            tiling_gap: 8,
            switcher_thumbnails: true,
            switcher_preview_delay: 200,
            pip_enabled: true,
            pip_size: 25,
        }
    }
}

/// Window Manager state
#[derive(Debug)]
pub struct WindowManagerState {
    /// Configuration
    pub config: WmConfig,
    /// Screen dimensions
    pub screen_width: u32,
    pub screen_height: u32,
    /// Work area (excluding panels)
    pub work_area: Rect,
    /// All windows
    pub windows: BTreeMap<WindowId, ManagedWindow>,
    /// Window Z-order (front to back)
    pub z_order: Vec<WindowId>,
    /// Virtual desktops
    pub desktops: Vec<VirtualDesktop>,
    /// Current desktop index
    pub current_desktop: usize,
    /// Alt+Tab switcher state
    pub switcher: Option<SwitcherState>,
    /// PiP window (if any)
    pub pip_window: Option<WindowId>,
    /// Dragging state
    pub drag_state: Option<DragState>,
    /// Resizing state
    pub resize_state: Option<ResizeState>,
}

/// Rectangle
#[derive(Debug, Clone, Copy, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && px < self.x + self.width as i32 &&
        py >= self.y && py < self.y + self.height as i32
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.x + other.width as i32 &&
        self.x + self.width as i32 > other.x &&
        self.y < other.y + other.height as i32 &&
        self.y + self.height as i32 > other.y
    }
}

/// Managed window state
#[derive(Debug, Clone)]
pub struct ManagedWindow {
    pub id: WindowId,
    pub desktop: usize,
    pub rect: Rect,
    pub saved_rect: Option<Rect>, // For restoring from maximized/snapped
    pub state: WindowState,
    pub snap_zone: Option<SnapZone>,
    pub tile_position: Option<TilePosition>,
    pub minimized: bool,
    pub always_on_top: bool,
    pub skip_taskbar: bool,
}

/// Window state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowState {
    Normal,
    Maximized,
    Minimized,
    Snapped(SnapZone),
    Tiled(TilePosition),
    Fullscreen,
    PictureInPicture,
}

/// Snap zone
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapZone {
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Tile position in tiling mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TilePosition {
    pub column: u32,
    pub row: u32,
    pub col_span: u32,
    pub row_span: u32,
}

/// Virtual desktop
#[derive(Debug, Clone)]
pub struct VirtualDesktop {
    pub id: u32,
    pub name: String,
    pub windows: Vec<WindowId>,
    pub layout: TilingLayout,
}

/// Tiling layout mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TilingLayout {
    None,
    Horizontal,
    Vertical,
    Grid,
    Master, // One large + stack
}

/// Alt+Tab switcher state
#[derive(Debug, Clone)]
pub struct SwitcherState {
    pub visible: bool,
    pub windows: Vec<WindowId>,
    pub selected_index: usize,
    pub started_at: u64,
}

/// Drag state
#[derive(Debug, Clone, Copy)]
pub struct DragState {
    pub window: WindowId,
    pub start_x: i32,
    pub start_y: i32,
    pub window_start_x: i32,
    pub window_start_y: i32,
    pub in_snap_preview: bool,
    pub preview_zone: Option<SnapZone>,
}

/// Resize state
#[derive(Debug, Clone, Copy)]
pub struct ResizeState {
    pub window: WindowId,
    pub start_x: i32,
    pub start_y: i32,
    pub start_rect: Rect,
    pub edge: ResizeEdge,
}

/// Resize edge
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeEdge {
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Error type
#[derive(Debug, Clone, Copy)]
pub enum WmError {
    NotInitialized,
    WindowNotFound,
    DesktopNotFound,
    InvalidOperation,
}

/// Initialize window manager
pub fn init(screen_width: u32, screen_height: u32, work_area: Rect, config: WmConfig) -> Result<(), WmError> {
    if INITIALIZED.load(Ordering::Acquire) {
        return Ok(());
    }

    let mut desktops = Vec::new();
    for i in 0..config.num_desktops {
        desktops.push(VirtualDesktop {
            id: i,
            name: format!("Desktop {}", i + 1),
            windows: Vec::new(),
            layout: TilingLayout::None,
        });
    }

    let state = WindowManagerState {
        config,
        screen_width,
        screen_height,
        work_area,
        windows: BTreeMap::new(),
        z_order: Vec::new(),
        desktops,
        current_desktop: 0,
        switcher: None,
        pip_window: None,
        drag_state: None,
        resize_state: None,
    };

    *WM_STATE.lock() = Some(state);
    INITIALIZED.store(true, Ordering::Release);

    crate::kprintln!("window_manager: Window manager initialized");
    Ok(())
}

use alloc::format;

/// Add a window
pub fn add_window(window: WindowId, rect: Rect, desktop: Option<usize>) -> Result<(), WmError> {
    let mut state = WM_STATE.lock();
    let state = state.as_mut().ok_or(WmError::NotInitialized)?;

    let desktop_idx = desktop.unwrap_or(state.current_desktop);

    let managed = ManagedWindow {
        id: window,
        desktop: desktop_idx,
        rect,
        saved_rect: None,
        state: WindowState::Normal,
        snap_zone: None,
        tile_position: None,
        minimized: false,
        always_on_top: false,
        skip_taskbar: false,
    };

    state.windows.insert(window, managed);
    state.z_order.push(window);

    if desktop_idx < state.desktops.len() {
        state.desktops[desktop_idx].windows.push(window);
    }

    Ok(())
}

/// Remove a window
pub fn remove_window(window: WindowId) -> Result<(), WmError> {
    let mut state = WM_STATE.lock();
    let state = state.as_mut().ok_or(WmError::NotInitialized)?;

    state.windows.remove(&window);
    state.z_order.retain(|&w| w != window);

    for desktop in &mut state.desktops {
        desktop.windows.retain(|&w| w != window);
    }

    if state.pip_window == Some(window) {
        state.pip_window = None;
    }

    Ok(())
}

/// Focus a window (bring to front)
pub fn focus_window(window: WindowId) -> Result<(), WmError> {
    let mut state = WM_STATE.lock();
    let state = state.as_mut().ok_or(WmError::NotInitialized)?;

    if !state.windows.contains_key(&window) {
        return Err(WmError::WindowNotFound);
    }

    // Move to front of z-order (but behind always-on-top windows)
    state.z_order.retain(|&w| w != window);

    let insert_pos = state.z_order.iter()
        .position(|&w| {
            state.windows.get(&w).map(|win| win.always_on_top).unwrap_or(false)
        })
        .unwrap_or(state.z_order.len());

    state.z_order.insert(insert_pos, window);

    Ok(())
}

/// Maximize a window
pub fn maximize_window(window: WindowId) -> Result<(), WmError> {
    let mut state = WM_STATE.lock();
    let state = state.as_mut().ok_or(WmError::NotInitialized)?;

    let win = state.windows.get_mut(&window).ok_or(WmError::WindowNotFound)?;

    if win.state != WindowState::Maximized {
        win.saved_rect = Some(win.rect);
        win.rect = state.work_area;
        win.state = WindowState::Maximized;
    }

    Ok(())
}

/// Restore window from maximized/minimized
pub fn restore_window(window: WindowId) -> Result<(), WmError> {
    let mut state = WM_STATE.lock();
    let state = state.as_mut().ok_or(WmError::NotInitialized)?;

    let win = state.windows.get_mut(&window).ok_or(WmError::WindowNotFound)?;

    if let Some(saved) = win.saved_rect.take() {
        win.rect = saved;
    }
    win.state = WindowState::Normal;
    win.minimized = false;
    win.snap_zone = None;

    Ok(())
}

/// Minimize a window
pub fn minimize_window(window: WindowId) -> Result<(), WmError> {
    let mut state = WM_STATE.lock();
    let state = state.as_mut().ok_or(WmError::NotInitialized)?;

    let win = state.windows.get_mut(&window).ok_or(WmError::WindowNotFound)?;
    win.minimized = true;
    win.state = WindowState::Minimized;

    Ok(())
}

/// Snap window to zone
pub fn snap_window(window: WindowId, zone: SnapZone) -> Result<(), WmError> {
    let mut state = WM_STATE.lock();
    let state = state.as_mut().ok_or(WmError::NotInitialized)?;

    let work = state.work_area;
    let win = state.windows.get_mut(&window).ok_or(WmError::WindowNotFound)?;

    // Save current rect if not already snapped
    if win.snap_zone.is_none() && win.state == WindowState::Normal {
        win.saved_rect = Some(win.rect);
    }

    let half_width = work.width / 2;
    let half_height = work.height / 2;

    win.rect = match zone {
        SnapZone::Left => Rect::new(work.x, work.y, half_width, work.height),
        SnapZone::Right => Rect::new(work.x + half_width as i32, work.y, half_width, work.height),
        SnapZone::Top => Rect::new(work.x, work.y, work.width, half_height),
        SnapZone::Bottom => Rect::new(work.x, work.y + half_height as i32, work.width, half_height),
        SnapZone::TopLeft => Rect::new(work.x, work.y, half_width, half_height),
        SnapZone::TopRight => Rect::new(work.x + half_width as i32, work.y, half_width, half_height),
        SnapZone::BottomLeft => Rect::new(work.x, work.y + half_height as i32, half_width, half_height),
        SnapZone::BottomRight => Rect::new(work.x + half_width as i32, work.y + half_height as i32, half_width, half_height),
    };

    win.state = WindowState::Snapped(zone);
    win.snap_zone = Some(zone);

    Ok(())
}

/// Detect snap zone from cursor position
pub fn detect_snap_zone(x: i32, y: i32) -> Option<SnapZone> {
    let state = WM_STATE.lock();
    let state = state.as_ref()?;

    if !state.config.snapping_enabled {
        return None;
    }

    let threshold = state.config.snap_threshold as i32;
    let work = state.work_area;

    let at_left = x <= work.x + threshold;
    let at_right = x >= work.x + work.width as i32 - threshold;
    let at_top = y <= work.y + threshold;
    let at_bottom = y >= work.y + work.height as i32 - threshold;

    if state.config.corner_snap {
        if at_left && at_top { return Some(SnapZone::TopLeft); }
        if at_right && at_top { return Some(SnapZone::TopRight); }
        if at_left && at_bottom { return Some(SnapZone::BottomLeft); }
        if at_right && at_bottom { return Some(SnapZone::BottomRight); }
    }

    if state.config.edge_snap {
        if at_left { return Some(SnapZone::Left); }
        if at_right { return Some(SnapZone::Right); }
        if at_top { return Some(SnapZone::Top); }
        if at_bottom { return Some(SnapZone::Bottom); }
    }

    None
}

/// Start dragging a window
pub fn start_drag(window: WindowId, mouse_x: i32, mouse_y: i32) -> Result<(), WmError> {
    let mut state = WM_STATE.lock();
    let state = state.as_mut().ok_or(WmError::NotInitialized)?;

    let win = state.windows.get(&window).ok_or(WmError::WindowNotFound)?;

    state.drag_state = Some(DragState {
        window,
        start_x: mouse_x,
        start_y: mouse_y,
        window_start_x: win.rect.x,
        window_start_y: win.rect.y,
        in_snap_preview: false,
        preview_zone: None,
    });

    Ok(())
}

/// Update drag position
pub fn update_drag(mouse_x: i32, mouse_y: i32) -> Result<Option<SnapZone>, WmError> {
    let mut state = WM_STATE.lock();
    let state = state.as_mut().ok_or(WmError::NotInitialized)?;

    let drag = state.drag_state.as_mut().ok_or(WmError::InvalidOperation)?;

    // Detect snap zone
    let zone = detect_snap_zone_internal(&state.config, &state.work_area, mouse_x, mouse_y);
    drag.preview_zone = zone;
    drag.in_snap_preview = zone.is_some();

    // Move window if not snapping
    if zone.is_none() {
        let dx = mouse_x - drag.start_x;
        let dy = mouse_y - drag.start_y;
        let window = drag.window;

        if let Some(win) = state.windows.get_mut(&window) {
            win.rect.x = drag.window_start_x + dx;
            win.rect.y = drag.window_start_y + dy;
        }
    }

    Ok(zone)
}

fn detect_snap_zone_internal(config: &WmConfig, work: &Rect, x: i32, y: i32) -> Option<SnapZone> {
    if !config.snapping_enabled {
        return None;
    }

    let threshold = config.snap_threshold as i32;

    let at_left = x <= work.x + threshold;
    let at_right = x >= work.x + work.width as i32 - threshold;
    let at_top = y <= work.y + threshold;
    let at_bottom = y >= work.y + work.height as i32 - threshold;

    if config.corner_snap {
        if at_left && at_top { return Some(SnapZone::TopLeft); }
        if at_right && at_top { return Some(SnapZone::TopRight); }
        if at_left && at_bottom { return Some(SnapZone::BottomLeft); }
        if at_right && at_bottom { return Some(SnapZone::BottomRight); }
    }

    if config.edge_snap {
        if at_left { return Some(SnapZone::Left); }
        if at_right { return Some(SnapZone::Right); }
        if at_top { return Some(SnapZone::Top); }
        if at_bottom { return Some(SnapZone::Bottom); }
    }

    None
}

/// End drag
pub fn end_drag() -> Result<Option<SnapZone>, WmError> {
    let mut state = WM_STATE.lock();
    let state = state.as_mut().ok_or(WmError::NotInitialized)?;

    let drag = state.drag_state.take().ok_or(WmError::InvalidOperation)?;

    if let Some(zone) = drag.preview_zone {
        let _ = snap_window_internal(state, drag.window, zone);
        return Ok(Some(zone));
    }

    Ok(None)
}

fn snap_window_internal(state: &mut WindowManagerState, window: WindowId, zone: SnapZone) -> Result<(), WmError> {
    let work = state.work_area;
    let win = state.windows.get_mut(&window).ok_or(WmError::WindowNotFound)?;

    if win.snap_zone.is_none() && win.state == WindowState::Normal {
        win.saved_rect = Some(win.rect);
    }

    let half_width = work.width / 2;
    let half_height = work.height / 2;

    win.rect = match zone {
        SnapZone::Left => Rect::new(work.x, work.y, half_width, work.height),
        SnapZone::Right => Rect::new(work.x + half_width as i32, work.y, half_width, work.height),
        SnapZone::Top => Rect::new(work.x, work.y, work.width, half_height),
        SnapZone::Bottom => Rect::new(work.x, work.y + half_height as i32, work.width, half_height),
        SnapZone::TopLeft => Rect::new(work.x, work.y, half_width, half_height),
        SnapZone::TopRight => Rect::new(work.x + half_width as i32, work.y, half_width, half_height),
        SnapZone::BottomLeft => Rect::new(work.x, work.y + half_height as i32, half_width, half_height),
        SnapZone::BottomRight => Rect::new(work.x + half_width as i32, work.y + half_height as i32, half_width, half_height),
    };

    win.state = WindowState::Snapped(zone);
    win.snap_zone = Some(zone);

    Ok(())
}

/// Switch to a virtual desktop
pub fn switch_desktop(index: usize) -> Result<(), WmError> {
    let mut state = WM_STATE.lock();
    let state = state.as_mut().ok_or(WmError::NotInitialized)?;

    if index >= state.desktops.len() {
        return Err(WmError::DesktopNotFound);
    }

    state.current_desktop = index;
    crate::kprintln!("window_manager: Switched to desktop {}", index + 1);

    Ok(())
}

/// Move window to a desktop
pub fn move_to_desktop(window: WindowId, desktop: usize) -> Result<(), WmError> {
    let mut state = WM_STATE.lock();
    let state = state.as_mut().ok_or(WmError::NotInitialized)?;

    if desktop >= state.desktops.len() {
        return Err(WmError::DesktopNotFound);
    }

    let win = state.windows.get_mut(&window).ok_or(WmError::WindowNotFound)?;
    let old_desktop = win.desktop;
    win.desktop = desktop;

    // Update desktop window lists
    if old_desktop < state.desktops.len() {
        state.desktops[old_desktop].windows.retain(|&w| w != window);
    }
    state.desktops[desktop].windows.push(window);

    Ok(())
}

/// Start Alt+Tab switcher
pub fn start_switcher() -> Result<(), WmError> {
    let mut state = WM_STATE.lock();
    let state = state.as_mut().ok_or(WmError::NotInitialized)?;

    let current = state.current_desktop;
    let windows: Vec<WindowId> = state.windows.iter()
        .filter(|(_, w)| w.desktop == current && !w.minimized && !w.skip_taskbar)
        .map(|(&id, _)| id)
        .collect();

    if windows.is_empty() {
        return Ok(());
    }

    state.switcher = Some(SwitcherState {
        visible: true,
        windows,
        selected_index: 0,
        started_at: crate::time::uptime_ns(),
    });

    Ok(())
}

/// Move to next window in switcher
pub fn switcher_next() -> Result<Option<WindowId>, WmError> {
    let mut state = WM_STATE.lock();
    let state = state.as_mut().ok_or(WmError::NotInitialized)?;

    let switcher = state.switcher.as_mut().ok_or(WmError::InvalidOperation)?;

    if !switcher.windows.is_empty() {
        switcher.selected_index = (switcher.selected_index + 1) % switcher.windows.len();
        return Ok(Some(switcher.windows[switcher.selected_index]));
    }

    Ok(None)
}

/// Move to previous window in switcher
pub fn switcher_previous() -> Result<Option<WindowId>, WmError> {
    let mut state = WM_STATE.lock();
    let state = state.as_mut().ok_or(WmError::NotInitialized)?;

    let switcher = state.switcher.as_mut().ok_or(WmError::InvalidOperation)?;

    if !switcher.windows.is_empty() {
        if switcher.selected_index == 0 {
            switcher.selected_index = switcher.windows.len() - 1;
        } else {
            switcher.selected_index -= 1;
        }
        return Ok(Some(switcher.windows[switcher.selected_index]));
    }

    Ok(None)
}

/// End switcher and focus selected window
pub fn end_switcher() -> Result<Option<WindowId>, WmError> {
    let mut state = WM_STATE.lock();
    let state = state.as_mut().ok_or(WmError::NotInitialized)?;

    let switcher = state.switcher.take().ok_or(WmError::InvalidOperation)?;

    if !switcher.windows.is_empty() {
        let selected = switcher.windows[switcher.selected_index];

        // Move to front of z-order
        state.z_order.retain(|&w| w != selected);
        let insert_pos = state.z_order.iter()
            .position(|&w| {
                state.windows.get(&w).map(|win| win.always_on_top).unwrap_or(false)
            })
            .unwrap_or(state.z_order.len());
        state.z_order.insert(insert_pos, selected);

        return Ok(Some(selected));
    }

    Ok(None)
}

/// Cancel switcher without changing focus
pub fn cancel_switcher() -> Result<(), WmError> {
    let mut state = WM_STATE.lock();
    let state = state.as_mut().ok_or(WmError::NotInitialized)?;

    state.switcher = None;

    Ok(())
}

/// Enable PiP mode for a window
pub fn enable_pip(window: WindowId) -> Result<(), WmError> {
    let mut state = WM_STATE.lock();
    let state = state.as_mut().ok_or(WmError::NotInitialized)?;

    if !state.config.pip_enabled {
        return Err(WmError::InvalidOperation);
    }

    let win = state.windows.get_mut(&window).ok_or(WmError::WindowNotFound)?;

    // Save current rect
    win.saved_rect = Some(win.rect);

    // Calculate PiP size
    let pip_width = state.screen_width * state.config.pip_size / 100;
    let pip_height = pip_width * 9 / 16; // 16:9 aspect ratio

    // Position in bottom-right corner
    let x = state.screen_width as i32 - pip_width as i32 - 20;
    let y = state.screen_height as i32 - pip_height as i32 - 20;

    win.rect = Rect::new(x, y, pip_width, pip_height);
    win.state = WindowState::PictureInPicture;
    win.always_on_top = true;

    state.pip_window = Some(window);

    Ok(())
}

/// Disable PiP mode
pub fn disable_pip(window: WindowId) -> Result<(), WmError> {
    let mut state = WM_STATE.lock();
    let state = state.as_mut().ok_or(WmError::NotInitialized)?;

    let win = state.windows.get_mut(&window).ok_or(WmError::WindowNotFound)?;

    if win.state != WindowState::PictureInPicture {
        return Err(WmError::InvalidOperation);
    }

    // Restore saved rect
    if let Some(saved) = win.saved_rect.take() {
        win.rect = saved;
    }

    win.state = WindowState::Normal;
    win.always_on_top = false;

    state.pip_window = None;

    Ok(())
}

/// Get windows for current desktop (in z-order)
pub fn get_visible_windows() -> Vec<WindowId> {
    WM_STATE
        .lock()
        .as_ref()
        .map(|state| {
            let current = state.current_desktop;
            state.z_order.iter()
                .filter(|&&w| {
                    state.windows.get(&w)
                        .map(|win| win.desktop == current && !win.minimized)
                        .unwrap_or(false)
                })
                .copied()
                .collect()
        })
        .unwrap_or_default()
}

/// Get window rect
pub fn get_window_rect(window: WindowId) -> Option<Rect> {
    WM_STATE
        .lock()
        .as_ref()
        .and_then(|state| state.windows.get(&window).map(|w| w.rect))
}

/// Get switcher state
pub fn get_switcher_state() -> Option<(Vec<WindowId>, usize)> {
    WM_STATE
        .lock()
        .as_ref()
        .and_then(|state| {
            state.switcher.as_ref().map(|s| (s.windows.clone(), s.selected_index))
        })
}

/// Get current desktop index
pub fn get_current_desktop() -> usize {
    WM_STATE
        .lock()
        .as_ref()
        .map(|state| state.current_desktop)
        .unwrap_or(0)
}

/// Get total desktop count
pub fn get_desktop_count() -> usize {
    WM_STATE
        .lock()
        .as_ref()
        .map(|state| state.desktops.len())
        .unwrap_or(0)
}
