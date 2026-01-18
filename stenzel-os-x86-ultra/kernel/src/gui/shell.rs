//! Desktop Shell
//!
//! The main desktop shell combining:
//! - Panel/taskbar with app buttons
//! - System tray with status icons
//! - Dock for pinned applications
//! - Overview mode for window management
//! - Hot corners and gestures
//! - Workspace management

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use spin::Mutex;

use crate::drivers::framebuffer::Color;
use super::surface::{Surface, PixelFormat};
use super::window::WindowId;

static SHELL_STATE: Mutex<Option<ShellState>> = Mutex::new(None);
static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Shell configuration
#[derive(Debug, Clone)]
pub struct ShellConfig {
    /// Panel position
    pub panel_position: PanelPosition,
    /// Panel height (or width if vertical)
    pub panel_size: u32,
    /// Panel auto-hide
    pub panel_autohide: bool,
    /// Enable dock
    pub dock_enabled: bool,
    /// Dock position (only if separate from panel)
    pub dock_position: DockPosition,
    /// Dock icon size
    pub dock_icon_size: u32,
    /// Enable hot corners
    pub hot_corners_enabled: bool,
    /// Hot corner actions
    pub hot_corners: HotCorners,
    /// Number of workspaces
    pub num_workspaces: u32,
    /// Show workspace indicator
    pub show_workspace_indicator: bool,
    /// Theme
    pub theme: ShellTheme,
}

impl Default for ShellConfig {
    fn default() -> Self {
        Self {
            panel_position: PanelPosition::Top,
            panel_size: 32,
            panel_autohide: false,
            dock_enabled: true,
            dock_position: DockPosition::Bottom,
            dock_icon_size: 48,
            hot_corners_enabled: true,
            hot_corners: HotCorners::default(),
            num_workspaces: 4,
            show_workspace_indicator: true,
            theme: ShellTheme::default(),
        }
    }
}

/// Panel position
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelPosition {
    Top,
    Bottom,
    Left,
    Right,
}

/// Dock position
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockPosition {
    Bottom,
    Left,
    Right,
}

/// Hot corner actions
#[derive(Debug, Clone, Default)]
pub struct HotCorners {
    pub top_left: Option<HotCornerAction>,
    pub top_right: Option<HotCornerAction>,
    pub bottom_left: Option<HotCornerAction>,
    pub bottom_right: Option<HotCornerAction>,
}

/// Hot corner action
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotCornerAction {
    ShowOverview,
    ShowDesktop,
    ShowLauncher,
    ShowNotifications,
    LockScreen,
    Sleep,
    Custom(u32),
}

/// Shell theme
#[derive(Debug, Clone)]
pub struct ShellTheme {
    pub panel_background: Color,
    pub panel_text: Color,
    pub dock_background: Color,
    pub dock_highlight: Color,
    pub workspace_background: Color,
    pub workspace_active: Color,
    pub accent_color: Color,
    pub transparency: f32,
}

impl Default for ShellTheme {
    fn default() -> Self {
        Self {
            panel_background: Color::rgba(30, 30, 30, 230),
            panel_text: Color::rgba(255, 255, 255, 255),
            dock_background: Color::rgba(40, 40, 40, 200),
            dock_highlight: Color::rgba(100, 100, 100, 150),
            workspace_background: Color::rgba(50, 50, 50, 200),
            workspace_active: Color::rgba(66, 133, 244, 255),
            accent_color: Color::rgba(66, 133, 244, 255),
            transparency: 0.9,
        }
    }
}

/// Shell state
#[derive(Debug)]
pub struct ShellState {
    /// Configuration
    pub config: ShellConfig,
    /// Screen dimensions
    pub screen_width: u32,
    pub screen_height: u32,
    /// Panel state
    pub panel: PanelState,
    /// Dock state
    pub dock: DockState,
    /// Workspace state
    pub workspaces: WorkspaceState,
    /// Overview mode active
    pub overview_active: bool,
    /// Launcher visible
    pub launcher_visible: bool,
    /// Notifications visible
    pub notifications_visible: bool,
    /// Active window
    pub active_window: Option<WindowId>,
}

/// Panel state
#[derive(Debug)]
pub struct PanelState {
    /// Panel bounds
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    /// Panel items
    pub items: Vec<PanelItem>,
    /// System tray icons
    pub tray_icons: Vec<TrayIcon>,
    /// Visible (for autohide)
    pub visible: bool,
    /// Hover state
    pub hovered: bool,
}

/// Panel item type
#[derive(Debug, Clone)]
pub enum PanelItem {
    /// Application menu button
    AppMenu,
    /// Window list
    WindowList(Vec<WindowButton>),
    /// Spacer
    Spacer,
    /// Workspace switcher
    WorkspaceSwitcher,
    /// System tray
    SystemTray,
    /// Clock
    Clock { format: String },
    /// User menu
    UserMenu,
    /// Custom widget
    Custom { id: String, width: u32 },
}

/// Window button in panel
#[derive(Debug, Clone)]
pub struct WindowButton {
    pub window_id: WindowId,
    pub title: String,
    pub icon: Option<Vec<u8>>,
    pub active: bool,
    pub attention: bool,
    pub x: i32,
    pub width: u32,
}

/// System tray icon
#[derive(Debug, Clone)]
pub struct TrayIcon {
    pub id: u64,
    pub name: String,
    pub icon: Vec<u8>,
    pub tooltip: String,
    pub x: i32,
}

/// Dock state
#[derive(Debug)]
pub struct DockState {
    /// Dock bounds
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    /// Pinned apps
    pub pinned_apps: Vec<DockItem>,
    /// Running apps (not pinned)
    pub running_apps: Vec<DockItem>,
    /// Trash/show desktop button
    pub show_desktop_button: bool,
    /// Visible (for autohide)
    pub visible: bool,
    /// Magnification on hover
    pub magnify: bool,
}

/// Dock item
#[derive(Debug, Clone)]
pub struct DockItem {
    pub id: u64,
    pub app_id: String,
    pub name: String,
    pub icon: Vec<u8>,
    pub command: String,
    pub pinned: bool,
    pub windows: Vec<WindowId>,
    pub x: i32,
    pub hovered: bool,
    pub bouncing: bool,
}

/// Workspace state
#[derive(Debug)]
pub struct WorkspaceState {
    /// All workspaces
    pub workspaces: Vec<Workspace>,
    /// Current workspace index
    pub current: usize,
    /// Workspace switcher visible
    pub switcher_visible: bool,
}

/// Single workspace
#[derive(Debug, Clone)]
pub struct Workspace {
    pub id: u32,
    pub name: String,
    pub windows: Vec<WindowId>,
    pub thumbnail: Option<Vec<u8>>,
}

/// Initialize shell
pub fn init(screen_width: u32, screen_height: u32, config: ShellConfig) -> Result<(), ShellError> {
    if INITIALIZED.load(Ordering::Acquire) {
        return Ok(());
    }

    let panel_height = config.panel_size;
    let panel_y = match config.panel_position {
        PanelPosition::Top => 0,
        PanelPosition::Bottom => screen_height as i32 - panel_height as i32,
        _ => 0,
    };

    let panel = PanelState {
        x: 0,
        y: panel_y,
        width: screen_width,
        height: panel_height,
        items: vec![
            PanelItem::AppMenu,
            PanelItem::WindowList(Vec::new()),
            PanelItem::Spacer,
            PanelItem::WorkspaceSwitcher,
            PanelItem::SystemTray,
            PanelItem::Clock { format: String::from("%H:%M") },
            PanelItem::UserMenu,
        ],
        tray_icons: Vec::new(),
        visible: true,
        hovered: false,
    };

    let dock_height = config.dock_icon_size + 16;
    let dock_y = screen_height as i32 - dock_height as i32 - panel_height as i32;

    let dock = DockState {
        x: 0,
        y: dock_y,
        width: screen_width,
        height: dock_height,
        pinned_apps: Vec::new(),
        running_apps: Vec::new(),
        show_desktop_button: true,
        visible: config.dock_enabled,
        magnify: true,
    };

    let mut workspaces = Vec::new();
    for i in 0..config.num_workspaces {
        workspaces.push(Workspace {
            id: i,
            name: format!("Workspace {}", i + 1),
            windows: Vec::new(),
            thumbnail: None,
        });
    }

    let workspace_state = WorkspaceState {
        workspaces,
        current: 0,
        switcher_visible: false,
    };

    let state = ShellState {
        config,
        screen_width,
        screen_height,
        panel,
        dock,
        workspaces: workspace_state,
        overview_active: false,
        launcher_visible: false,
        notifications_visible: false,
        active_window: None,
    };

    *SHELL_STATE.lock() = Some(state);
    INITIALIZED.store(true, Ordering::Release);

    crate::kprintln!("shell: Desktop shell initialized ({}x{})", screen_width, screen_height);
    Ok(())
}

/// Error type
#[derive(Debug, Clone, Copy)]
pub enum ShellError {
    NotInitialized,
    InvalidWorkspace,
    InvalidWindow,
    DockFull,
}

/// Show overview mode (expose all windows)
pub fn show_overview() -> Result<(), ShellError> {
    let mut state = SHELL_STATE.lock();
    let state = state.as_mut().ok_or(ShellError::NotInitialized)?;

    state.overview_active = true;
    crate::kprintln!("shell: Overview mode activated");

    Ok(())
}

/// Hide overview mode
pub fn hide_overview() -> Result<(), ShellError> {
    let mut state = SHELL_STATE.lock();
    let state = state.as_mut().ok_or(ShellError::NotInitialized)?;

    state.overview_active = false;

    Ok(())
}

/// Toggle overview mode
pub fn toggle_overview() -> Result<bool, ShellError> {
    let mut state = SHELL_STATE.lock();
    let state = state.as_mut().ok_or(ShellError::NotInitialized)?;

    state.overview_active = !state.overview_active;

    Ok(state.overview_active)
}

/// Show launcher (app menu)
pub fn show_launcher() -> Result<(), ShellError> {
    let mut state = SHELL_STATE.lock();
    let state = state.as_mut().ok_or(ShellError::NotInitialized)?;

    state.launcher_visible = true;

    Ok(())
}

/// Hide launcher
pub fn hide_launcher() -> Result<(), ShellError> {
    let mut state = SHELL_STATE.lock();
    let state = state.as_mut().ok_or(ShellError::NotInitialized)?;

    state.launcher_visible = false;

    Ok(())
}

/// Toggle launcher
pub fn toggle_launcher() -> Result<bool, ShellError> {
    let mut state = SHELL_STATE.lock();
    let state = state.as_mut().ok_or(ShellError::NotInitialized)?;

    state.launcher_visible = !state.launcher_visible;

    Ok(state.launcher_visible)
}

/// Show notification panel
pub fn show_notifications() -> Result<(), ShellError> {
    let mut state = SHELL_STATE.lock();
    let state = state.as_mut().ok_or(ShellError::NotInitialized)?;

    state.notifications_visible = true;

    Ok(())
}

/// Hide notification panel
pub fn hide_notifications() -> Result<(), ShellError> {
    let mut state = SHELL_STATE.lock();
    let state = state.as_mut().ok_or(ShellError::NotInitialized)?;

    state.notifications_visible = false;

    Ok(())
}

/// Switch to workspace
pub fn switch_workspace(index: usize) -> Result<(), ShellError> {
    let mut state = SHELL_STATE.lock();
    let state = state.as_mut().ok_or(ShellError::NotInitialized)?;

    if index >= state.workspaces.workspaces.len() {
        return Err(ShellError::InvalidWorkspace);
    }

    state.workspaces.current = index;
    crate::kprintln!("shell: Switched to workspace {}", index + 1);

    Ok(())
}

/// Get current workspace index
pub fn get_current_workspace() -> Result<usize, ShellError> {
    let state = SHELL_STATE.lock();
    let state = state.as_ref().ok_or(ShellError::NotInitialized)?;

    Ok(state.workspaces.current)
}

/// Move window to workspace
pub fn move_window_to_workspace(window: WindowId, workspace_index: usize) -> Result<(), ShellError> {
    let mut state = SHELL_STATE.lock();
    let state = state.as_mut().ok_or(ShellError::NotInitialized)?;

    if workspace_index >= state.workspaces.workspaces.len() {
        return Err(ShellError::InvalidWorkspace);
    }

    // Remove from current workspace
    for ws in &mut state.workspaces.workspaces {
        ws.windows.retain(|&w| w != window);
    }

    // Add to new workspace
    state.workspaces.workspaces[workspace_index].windows.push(window);

    Ok(())
}

/// Add window to shell tracking
pub fn add_window(window: WindowId, title: &str, workspace_index: Option<usize>) -> Result<(), ShellError> {
    let mut state = SHELL_STATE.lock();
    let state = state.as_mut().ok_or(ShellError::NotInitialized)?;

    let ws_index = workspace_index.unwrap_or(state.workspaces.current);

    if ws_index >= state.workspaces.workspaces.len() {
        return Err(ShellError::InvalidWorkspace);
    }

    // Add to workspace
    state.workspaces.workspaces[ws_index].windows.push(window);

    // Add to panel window list
    for item in &mut state.panel.items {
        if let PanelItem::WindowList(windows) = item {
            windows.push(WindowButton {
                window_id: window,
                title: String::from(title),
                icon: None,
                active: false,
                attention: false,
                x: 0,
                width: 150,
            });
            break;
        }
    }

    Ok(())
}

/// Remove window from shell tracking
pub fn remove_window(window: WindowId) -> Result<(), ShellError> {
    let mut state = SHELL_STATE.lock();
    let state = state.as_mut().ok_or(ShellError::NotInitialized)?;

    // Remove from all workspaces
    for ws in &mut state.workspaces.workspaces {
        ws.windows.retain(|&w| w != window);
    }

    // Remove from panel window list
    for item in &mut state.panel.items {
        if let PanelItem::WindowList(windows) = item {
            windows.retain(|w| w.window_id != window);
            break;
        }
    }

    // Remove from dock running apps
    for app in &mut state.dock.running_apps {
        app.windows.retain(|&w| w != window);
    }
    state.dock.running_apps.retain(|app| !app.windows.is_empty() || app.pinned);

    if state.active_window == Some(window) {
        state.active_window = None;
    }

    Ok(())
}

/// Set active window
pub fn set_active_window(window: Option<WindowId>) -> Result<(), ShellError> {
    let mut state = SHELL_STATE.lock();
    let state = state.as_mut().ok_or(ShellError::NotInitialized)?;

    state.active_window = window;

    // Update panel window buttons
    for item in &mut state.panel.items {
        if let PanelItem::WindowList(windows) = item {
            for btn in windows {
                btn.active = Some(btn.window_id) == window;
            }
            break;
        }
    }

    Ok(())
}

/// Pin app to dock
pub fn pin_to_dock(app_id: &str, name: &str, command: &str, icon: Vec<u8>) -> Result<u64, ShellError> {
    let mut state = SHELL_STATE.lock();
    let state = state.as_mut().ok_or(ShellError::NotInitialized)?;

    static NEXT_DOCK_ID: AtomicU64 = AtomicU64::new(1);
    let id = NEXT_DOCK_ID.fetch_add(1, Ordering::SeqCst);

    let item = DockItem {
        id,
        app_id: String::from(app_id),
        name: String::from(name),
        icon,
        command: String::from(command),
        pinned: true,
        windows: Vec::new(),
        x: 0,
        hovered: false,
        bouncing: false,
    };

    state.dock.pinned_apps.push(item);

    Ok(id)
}

/// Unpin app from dock
pub fn unpin_from_dock(id: u64) -> Result<(), ShellError> {
    let mut state = SHELL_STATE.lock();
    let state = state.as_mut().ok_or(ShellError::NotInitialized)?;

    state.dock.pinned_apps.retain(|app| app.id != id);

    Ok(())
}

/// Add system tray icon
pub fn add_tray_icon(name: &str, icon: Vec<u8>, tooltip: &str) -> Result<u64, ShellError> {
    let mut state = SHELL_STATE.lock();
    let state = state.as_mut().ok_or(ShellError::NotInitialized)?;

    static NEXT_TRAY_ID: AtomicU64 = AtomicU64::new(1);
    let id = NEXT_TRAY_ID.fetch_add(1, Ordering::SeqCst);

    let tray_icon = TrayIcon {
        id,
        name: String::from(name),
        icon,
        tooltip: String::from(tooltip),
        x: 0,
    };

    state.panel.tray_icons.push(tray_icon);

    Ok(id)
}

/// Remove system tray icon
pub fn remove_tray_icon(id: u64) -> Result<(), ShellError> {
    let mut state = SHELL_STATE.lock();
    let state = state.as_mut().ok_or(ShellError::NotInitialized)?;

    state.panel.tray_icons.retain(|icon| icon.id != id);

    Ok(())
}

/// Handle hot corner activation
pub fn handle_hot_corner(x: i32, y: i32) -> Result<Option<HotCornerAction>, ShellError> {
    let state = SHELL_STATE.lock();
    let state = state.as_ref().ok_or(ShellError::NotInitialized)?;

    if !state.config.hot_corners_enabled {
        return Ok(None);
    }

    let screen_w = state.screen_width as i32;
    let screen_h = state.screen_height as i32;
    let threshold = 5;

    let action = if x <= threshold && y <= threshold {
        state.config.hot_corners.top_left
    } else if x >= screen_w - threshold && y <= threshold {
        state.config.hot_corners.top_right
    } else if x <= threshold && y >= screen_h - threshold {
        state.config.hot_corners.bottom_left
    } else if x >= screen_w - threshold && y >= screen_h - threshold {
        state.config.hot_corners.bottom_right
    } else {
        None
    };

    Ok(action)
}

/// Get shell configuration
pub fn get_config() -> Result<ShellConfig, ShellError> {
    let state = SHELL_STATE.lock();
    let state = state.as_ref().ok_or(ShellError::NotInitialized)?;

    Ok(state.config.clone())
}

/// Update shell configuration
pub fn set_config(config: ShellConfig) -> Result<(), ShellError> {
    let mut state = SHELL_STATE.lock();
    let state = state.as_mut().ok_or(ShellError::NotInitialized)?;

    state.config = config;

    Ok(())
}

/// Get usable screen area (excluding panel and dock)
pub fn get_work_area() -> Result<(i32, i32, u32, u32), ShellError> {
    let state = SHELL_STATE.lock();
    let state = state.as_ref().ok_or(ShellError::NotInitialized)?;

    let mut x = 0i32;
    let mut y = 0i32;
    let mut w = state.screen_width;
    let mut h = state.screen_height;

    // Subtract panel
    match state.config.panel_position {
        PanelPosition::Top => {
            y += state.panel.height as i32;
            h -= state.panel.height;
        }
        PanelPosition::Bottom => {
            h -= state.panel.height;
        }
        PanelPosition::Left => {
            x += state.panel.width as i32;
            w -= state.panel.width;
        }
        PanelPosition::Right => {
            w -= state.panel.width;
        }
    }

    // Subtract dock if enabled and not same position as panel
    if state.config.dock_enabled && state.dock.visible {
        match state.config.dock_position {
            DockPosition::Bottom if state.config.panel_position != PanelPosition::Bottom => {
                h -= state.dock.height;
            }
            DockPosition::Left if state.config.panel_position != PanelPosition::Left => {
                x += state.dock.width as i32;
                w -= state.dock.width;
            }
            DockPosition::Right if state.config.panel_position != PanelPosition::Right => {
                w -= state.dock.width;
            }
            _ => {}
        }
    }

    Ok((x, y, w, h))
}

/// Check if shell is initialized
pub fn is_initialized() -> bool {
    INITIALIZED.load(Ordering::Acquire)
}
