//! Notification Center
//!
//! Centralized notification management with:
//! - Notification history
//! - Grouping by application
//! - Do Not Disturb mode
//! - Quick settings panel
//! - Calendar widget integration
//! - Action buttons

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use spin::Mutex;

use crate::drivers::framebuffer::Color;
use super::surface::{Surface, PixelFormat};

static NC_STATE: Mutex<Option<NotificationCenterState>> = Mutex::new(None);
static INITIALIZED: AtomicBool = AtomicBool::new(false);
static NEXT_NOTIFICATION_ID: AtomicU64 = AtomicU64::new(1);

/// Notification Center configuration
#[derive(Debug, Clone)]
pub struct NotificationCenterConfig {
    /// Position on screen
    pub position: Position,
    /// Width
    pub width: u32,
    /// Max height (before scrolling)
    pub max_height: u32,
    /// Show quick settings
    pub show_quick_settings: bool,
    /// Show calendar
    pub show_calendar: bool,
    /// Group notifications by app
    pub group_by_app: bool,
    /// Maximum notifications in history
    pub max_history: usize,
    /// Auto-collapse old notifications
    pub auto_collapse: bool,
    /// Theme
    pub theme: NotificationCenterTheme,
}

impl Default for NotificationCenterConfig {
    fn default() -> Self {
        Self {
            position: Position::TopRight,
            width: 380,
            max_height: 600,
            show_quick_settings: true,
            show_calendar: true,
            group_by_app: true,
            max_history: 100,
            auto_collapse: true,
            theme: NotificationCenterTheme::default(),
        }
    }
}

/// Position
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    TopRight,
    TopLeft,
    BottomRight,
    BottomLeft,
    Center,
}

/// Theme
#[derive(Debug, Clone)]
pub struct NotificationCenterTheme {
    pub background: Color,
    pub card_background: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub accent: Color,
    pub urgent_color: Color,
    pub success_color: Color,
    pub warning_color: Color,
    pub separator: Color,
    pub border_radius: u32,
}

impl Default for NotificationCenterTheme {
    fn default() -> Self {
        Self {
            background: Color::rgba(25, 25, 25, 245),
            card_background: Color::rgba(45, 45, 45, 255),
            text_primary: Color::rgba(255, 255, 255, 255),
            text_secondary: Color::rgba(170, 170, 170, 255),
            accent: Color::rgba(66, 133, 244, 255),
            urgent_color: Color::rgba(234, 67, 53, 255),
            success_color: Color::rgba(52, 168, 83, 255),
            warning_color: Color::rgba(251, 188, 4, 255),
            separator: Color::rgba(80, 80, 80, 255),
            border_radius: 12,
        }
    }
}

/// Notification Center state
#[derive(Debug)]
pub struct NotificationCenterState {
    /// Configuration
    pub config: NotificationCenterConfig,
    /// Visible
    pub visible: bool,
    /// Notifications (grouped by app if enabled)
    pub notifications: BTreeMap<String, Vec<Notification>>,
    /// Notification history
    pub history: Vec<Notification>,
    /// Do Not Disturb mode
    pub dnd_enabled: bool,
    /// DND until timestamp (0 = indefinite)
    pub dnd_until: u64,
    /// Quick settings
    pub quick_settings: Vec<QuickSetting>,
    /// Current date for calendar
    pub current_date: (u16, u8, u8), // year, month, day
    /// Scroll offset
    pub scroll_offset: i32,
    /// Screen dimensions
    pub screen_width: u32,
    pub screen_height: u32,
}

/// Single notification
#[derive(Debug, Clone)]
pub struct Notification {
    pub id: u64,
    pub app_id: String,
    pub app_name: String,
    pub app_icon: Option<Vec<u8>>,
    pub summary: String,
    pub body: Option<String>,
    pub icon: Option<NotificationIcon>,
    pub urgency: Urgency,
    pub timestamp: u64,
    pub actions: Vec<NotificationAction>,
    pub hints: NotificationHints,
    pub read: bool,
    pub expired: bool,
    pub collapsed: bool,
}

/// Notification icon
#[derive(Debug, Clone)]
pub enum NotificationIcon {
    /// Named icon (from theme)
    Named(String),
    /// Inline icon data (RGBA)
    Data { data: Vec<u8>, width: u32, height: u32 },
    /// File path
    Path(String),
}

/// Urgency level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Urgency {
    Low,
    Normal,
    Critical,
}

/// Notification action
#[derive(Debug, Clone)]
pub struct NotificationAction {
    pub id: String,
    pub label: String,
    pub is_default: bool,
}

/// Notification hints
#[derive(Debug, Clone, Default)]
pub struct NotificationHints {
    pub category: Option<String>,
    pub resident: bool,
    pub transient: bool,
    pub action_icons: bool,
    pub sound_file: Option<String>,
    pub sound_name: Option<String>,
    pub suppress_sound: bool,
    pub image_data: Option<Vec<u8>>,
    pub desktop_entry: Option<String>,
}

/// Quick setting
#[derive(Debug, Clone)]
pub struct QuickSetting {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub setting_type: QuickSettingType,
    pub enabled: bool,
    pub value: u32, // For sliders
}

/// Quick setting type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickSettingType {
    Toggle,
    Slider,
    Menu,
}

/// Error type
#[derive(Debug, Clone, Copy)]
pub enum NotificationCenterError {
    NotInitialized,
    NotificationNotFound,
    InvalidAction,
}

/// Initialize notification center
pub fn init(config: NotificationCenterConfig, screen_width: u32, screen_height: u32) -> Result<(), NotificationCenterError> {
    if INITIALIZED.load(Ordering::Acquire) {
        return Ok(());
    }

    let quick_settings = vec![
        QuickSetting {
            id: String::from("wifi"),
            name: String::from("Wi-Fi"),
            icon: String::from("network-wireless"),
            setting_type: QuickSettingType::Toggle,
            enabled: true,
            value: 0,
        },
        QuickSetting {
            id: String::from("bluetooth"),
            name: String::from("Bluetooth"),
            icon: String::from("bluetooth"),
            setting_type: QuickSettingType::Toggle,
            enabled: false,
            value: 0,
        },
        QuickSetting {
            id: String::from("dnd"),
            name: String::from("Do Not Disturb"),
            icon: String::from("notifications-disabled"),
            setting_type: QuickSettingType::Toggle,
            enabled: false,
            value: 0,
        },
        QuickSetting {
            id: String::from("nightlight"),
            name: String::from("Night Light"),
            icon: String::from("night-light"),
            setting_type: QuickSettingType::Toggle,
            enabled: false,
            value: 0,
        },
        QuickSetting {
            id: String::from("airplane"),
            name: String::from("Airplane Mode"),
            icon: String::from("airplane-mode"),
            setting_type: QuickSettingType::Toggle,
            enabled: false,
            value: 0,
        },
        QuickSetting {
            id: String::from("brightness"),
            name: String::from("Brightness"),
            icon: String::from("display-brightness"),
            setting_type: QuickSettingType::Slider,
            enabled: true,
            value: 80,
        },
        QuickSetting {
            id: String::from("volume"),
            name: String::from("Volume"),
            icon: String::from("audio-volume-high"),
            setting_type: QuickSettingType::Slider,
            enabled: true,
            value: 70,
        },
    ];

    let state = NotificationCenterState {
        config,
        visible: false,
        notifications: BTreeMap::new(),
        history: Vec::new(),
        dnd_enabled: false,
        dnd_until: 0,
        quick_settings,
        current_date: (2026, 1, 17),
        scroll_offset: 0,
        screen_width,
        screen_height,
    };

    *NC_STATE.lock() = Some(state);
    INITIALIZED.store(true, Ordering::Release);

    crate::kprintln!("notification_center: Notification center initialized");
    Ok(())
}

/// Show notification center
pub fn show() -> Result<(), NotificationCenterError> {
    let mut state = NC_STATE.lock();
    let state = state.as_mut().ok_or(NotificationCenterError::NotInitialized)?;

    state.visible = true;
    state.scroll_offset = 0;

    Ok(())
}

/// Hide notification center
pub fn hide() -> Result<(), NotificationCenterError> {
    let mut state = NC_STATE.lock();
    let state = state.as_mut().ok_or(NotificationCenterError::NotInitialized)?;

    state.visible = false;

    Ok(())
}

/// Toggle visibility
pub fn toggle() -> Result<bool, NotificationCenterError> {
    let mut state = NC_STATE.lock();
    let state = state.as_mut().ok_or(NotificationCenterError::NotInitialized)?;

    state.visible = !state.visible;

    Ok(state.visible)
}

/// Add a notification
pub fn notify(
    app_id: &str,
    app_name: &str,
    summary: &str,
    body: Option<&str>,
    urgency: Urgency,
    actions: Vec<NotificationAction>,
    hints: NotificationHints,
) -> Result<u64, NotificationCenterError> {
    let mut state = NC_STATE.lock();
    let state = state.as_mut().ok_or(NotificationCenterError::NotInitialized)?;

    // Check DND mode
    if state.dnd_enabled && urgency != Urgency::Critical {
        // Still add to history but don't show
        let id = NEXT_NOTIFICATION_ID.fetch_add(1, Ordering::SeqCst);
        let notification = Notification {
            id,
            app_id: String::from(app_id),
            app_name: String::from(app_name),
            app_icon: None,
            summary: String::from(summary),
            body: body.map(String::from),
            icon: None,
            urgency,
            timestamp: crate::time::uptime_ns(),
            actions,
            hints,
            read: false,
            expired: false,
            collapsed: false,
        };

        state.history.push(notification);
        trim_history(&mut state.history, state.config.max_history);

        return Ok(id);
    }

    let id = NEXT_NOTIFICATION_ID.fetch_add(1, Ordering::SeqCst);

    let notification = Notification {
        id,
        app_id: String::from(app_id),
        app_name: String::from(app_name),
        app_icon: None,
        summary: String::from(summary),
        body: body.map(String::from),
        icon: None,
        urgency,
        timestamp: crate::time::uptime_ns(),
        actions,
        hints,
        read: false,
        expired: false,
        collapsed: state.config.auto_collapse,
    };

    // Add to grouped notifications
    if state.config.group_by_app {
        state.notifications
            .entry(String::from(app_id))
            .or_insert_with(Vec::new)
            .push(notification.clone());
    } else {
        state.notifications
            .entry(String::from("all"))
            .or_insert_with(Vec::new)
            .push(notification.clone());
    }

    // Add to history
    state.history.push(notification);
    trim_history(&mut state.history, state.config.max_history);

    crate::kprintln!("notification_center: New notification from {} - {}", app_name, summary);

    Ok(id)
}

fn trim_history(history: &mut Vec<Notification>, max: usize) {
    if history.len() > max {
        history.drain(0..(history.len() - max));
    }
}

/// Close/dismiss a notification
pub fn close_notification(id: u64) -> Result<(), NotificationCenterError> {
    let mut state = NC_STATE.lock();
    let state = state.as_mut().ok_or(NotificationCenterError::NotInitialized)?;

    for notifications in state.notifications.values_mut() {
        notifications.retain(|n| n.id != id);
    }

    // Clean up empty groups
    state.notifications.retain(|_, v| !v.is_empty());

    // Mark as expired in history
    if let Some(n) = state.history.iter_mut().find(|n| n.id == id) {
        n.expired = true;
    }

    Ok(())
}

/// Close all notifications from an app
pub fn close_app_notifications(app_id: &str) -> Result<usize, NotificationCenterError> {
    let mut state = NC_STATE.lock();
    let state = state.as_mut().ok_or(NotificationCenterError::NotInitialized)?;

    let count = state.notifications.get(app_id).map(|v| v.len()).unwrap_or(0);
    state.notifications.remove(app_id);

    // Mark as expired in history
    for n in state.history.iter_mut().filter(|n| n.app_id == app_id) {
        n.expired = true;
    }

    Ok(count)
}

/// Close all notifications
pub fn clear_all() -> Result<usize, NotificationCenterError> {
    let mut state = NC_STATE.lock();
    let state = state.as_mut().ok_or(NotificationCenterError::NotInitialized)?;

    let count: usize = state.notifications.values().map(|v| v.len()).sum();
    state.notifications.clear();

    // Mark all as expired in history
    for n in &mut state.history {
        n.expired = true;
    }

    Ok(count)
}

/// Mark notification as read
pub fn mark_read(id: u64) -> Result<(), NotificationCenterError> {
    let mut state = NC_STATE.lock();
    let state = state.as_mut().ok_or(NotificationCenterError::NotInitialized)?;

    for notifications in state.notifications.values_mut() {
        if let Some(n) = notifications.iter_mut().find(|n| n.id == id) {
            n.read = true;
            break;
        }
    }

    if let Some(n) = state.history.iter_mut().find(|n| n.id == id) {
        n.read = true;
    }

    Ok(())
}

/// Mark all notifications as read
pub fn mark_all_read() -> Result<(), NotificationCenterError> {
    let mut state = NC_STATE.lock();
    let state = state.as_mut().ok_or(NotificationCenterError::NotInitialized)?;

    for notifications in state.notifications.values_mut() {
        for n in notifications {
            n.read = true;
        }
    }

    for n in &mut state.history {
        n.read = true;
    }

    Ok(())
}

/// Invoke a notification action
pub fn invoke_action(notification_id: u64, action_id: &str) -> Result<(), NotificationCenterError> {
    let state = NC_STATE.lock();
    let state = state.as_ref().ok_or(NotificationCenterError::NotInitialized)?;

    // Find the notification and action
    for notifications in state.notifications.values() {
        if let Some(n) = notifications.iter().find(|n| n.id == notification_id) {
            if n.actions.iter().any(|a| a.id == action_id) {
                crate::kprintln!(
                    "notification_center: Action '{}' invoked on notification {}",
                    action_id,
                    notification_id
                );
                return Ok(());
            }
        }
    }

    Err(NotificationCenterError::InvalidAction)
}

/// Enable Do Not Disturb mode
pub fn enable_dnd(until: Option<u64>) -> Result<(), NotificationCenterError> {
    let mut state = NC_STATE.lock();
    let state = state.as_mut().ok_or(NotificationCenterError::NotInitialized)?;

    state.dnd_enabled = true;
    state.dnd_until = until.unwrap_or(0);

    // Update quick settings
    if let Some(dnd) = state.quick_settings.iter_mut().find(|s| s.id == "dnd") {
        dnd.enabled = true;
    }

    crate::kprintln!("notification_center: Do Not Disturb enabled");

    Ok(())
}

/// Disable Do Not Disturb mode
pub fn disable_dnd() -> Result<(), NotificationCenterError> {
    let mut state = NC_STATE.lock();
    let state = state.as_mut().ok_or(NotificationCenterError::NotInitialized)?;

    state.dnd_enabled = false;
    state.dnd_until = 0;

    // Update quick settings
    if let Some(dnd) = state.quick_settings.iter_mut().find(|s| s.id == "dnd") {
        dnd.enabled = false;
    }

    Ok(())
}

/// Toggle quick setting
pub fn toggle_quick_setting(setting_id: &str) -> Result<bool, NotificationCenterError> {
    let mut state = NC_STATE.lock();
    let state = state.as_mut().ok_or(NotificationCenterError::NotInitialized)?;

    if let Some(setting) = state.quick_settings.iter_mut().find(|s| s.id == setting_id) {
        if setting.setting_type == QuickSettingType::Toggle {
            setting.enabled = !setting.enabled;

            // Handle special cases
            if setting_id == "dnd" {
                state.dnd_enabled = setting.enabled;
            }

            return Ok(setting.enabled);
        }
    }

    Err(NotificationCenterError::InvalidAction)
}

/// Set quick setting slider value
pub fn set_quick_setting_value(setting_id: &str, value: u32) -> Result<(), NotificationCenterError> {
    let mut state = NC_STATE.lock();
    let state = state.as_mut().ok_or(NotificationCenterError::NotInitialized)?;

    if let Some(setting) = state.quick_settings.iter_mut().find(|s| s.id == setting_id) {
        if setting.setting_type == QuickSettingType::Slider {
            setting.value = value.min(100);
            return Ok(());
        }
    }

    Err(NotificationCenterError::InvalidAction)
}

/// Get notification count
pub fn get_notification_count() -> usize {
    NC_STATE
        .lock()
        .as_ref()
        .map(|s| s.notifications.values().map(|v| v.len()).sum())
        .unwrap_or(0)
}

/// Get unread notification count
pub fn get_unread_count() -> usize {
    NC_STATE
        .lock()
        .as_ref()
        .map(|s| {
            s.notifications.values()
                .flat_map(|v| v.iter())
                .filter(|n| !n.read)
                .count()
        })
        .unwrap_or(0)
}

/// Get all notifications grouped by app
pub fn get_grouped_notifications() -> Result<BTreeMap<String, Vec<Notification>>, NotificationCenterError> {
    let state = NC_STATE.lock();
    let state = state.as_ref().ok_or(NotificationCenterError::NotInitialized)?;

    Ok(state.notifications.clone())
}

/// Get notification history
pub fn get_history(limit: Option<usize>) -> Result<Vec<Notification>, NotificationCenterError> {
    let state = NC_STATE.lock();
    let state = state.as_ref().ok_or(NotificationCenterError::NotInitialized)?;

    let history: Vec<_> = state.history.iter()
        .rev()
        .take(limit.unwrap_or(50))
        .cloned()
        .collect();

    Ok(history)
}

/// Get quick settings
pub fn get_quick_settings() -> Result<Vec<QuickSetting>, NotificationCenterError> {
    let state = NC_STATE.lock();
    let state = state.as_ref().ok_or(NotificationCenterError::NotInitialized)?;

    Ok(state.quick_settings.clone())
}

/// Check if notification center is visible
pub fn is_visible() -> bool {
    NC_STATE
        .lock()
        .as_ref()
        .map(|s| s.visible)
        .unwrap_or(false)
}

/// Check if DND is enabled
pub fn is_dnd_enabled() -> bool {
    NC_STATE
        .lock()
        .as_ref()
        .map(|s| s.dnd_enabled)
        .unwrap_or(false)
}

/// Scroll notification center
pub fn scroll(delta: i32) -> Result<(), NotificationCenterError> {
    let mut state = NC_STATE.lock();
    let state = state.as_mut().ok_or(NotificationCenterError::NotInitialized)?;

    state.scroll_offset = (state.scroll_offset + delta).max(0);

    Ok(())
}
