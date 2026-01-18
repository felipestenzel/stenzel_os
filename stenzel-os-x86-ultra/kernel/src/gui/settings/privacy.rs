//! Privacy Settings
//!
//! Location, camera, microphone, screen sharing, and app permissions.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use spin::Mutex;

/// Global privacy settings state
static PRIVACY_SETTINGS: Mutex<Option<PrivacySettings>> = Mutex::new(None);

/// Privacy settings state
pub struct PrivacySettings {
    /// Location services
    pub location: LocationSettings,
    /// Camera access
    pub camera: DeviceAccessSettings,
    /// Microphone access
    pub microphone: DeviceAccessSettings,
    /// Screen sharing/recording
    pub screen_sharing: DeviceAccessSettings,
    /// File access per app
    pub file_access: Vec<AppFileAccess>,
    /// Notification access
    pub notification_access: Vec<AppPermission>,
    /// Background app refresh
    pub background_refresh: BackgroundRefreshSettings,
    /// Diagnostics & analytics
    pub diagnostics: DiagnosticsSettings,
    /// Recent files tracking
    pub track_recent_files: bool,
    /// Usage data collection
    pub usage_data: bool,
}

/// Location settings
#[derive(Debug, Clone)]
pub struct LocationSettings {
    /// Location services enabled
    pub enabled: bool,
    /// GPS enabled
    pub gps_enabled: bool,
    /// WiFi-based location
    pub wifi_location: bool,
    /// App permissions
    pub app_permissions: Vec<LocationPermission>,
    /// Location history
    pub save_history: bool,
    /// History retention (days)
    pub history_days: u32,
}

/// Location permission for an app
#[derive(Debug, Clone)]
pub struct LocationPermission {
    /// App ID
    pub app_id: String,
    /// App name
    pub app_name: String,
    /// Permission level
    pub permission: LocationAccessLevel,
    /// Last accessed
    pub last_accessed: Option<u64>,
}

/// Location access level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocationAccessLevel {
    /// No access
    Denied,
    /// Ask each time
    AskEachTime,
    /// While using the app
    WhileUsing,
    /// Always (including background)
    Always,
}

impl LocationAccessLevel {
    pub fn name(&self) -> &'static str {
        match self {
            LocationAccessLevel::Denied => "Denied",
            LocationAccessLevel::AskEachTime => "Ask Each Time",
            LocationAccessLevel::WhileUsing => "While Using",
            LocationAccessLevel::Always => "Always",
        }
    }
}

/// Device access settings (camera, mic, screen)
#[derive(Debug, Clone)]
pub struct DeviceAccessSettings {
    /// Device access globally enabled
    pub enabled: bool,
    /// App permissions
    pub app_permissions: Vec<AppPermission>,
}

/// App permission
#[derive(Debug, Clone)]
pub struct AppPermission {
    /// App ID
    pub app_id: String,
    /// App name
    pub app_name: String,
    /// Allowed
    pub allowed: bool,
    /// Last accessed
    pub last_accessed: Option<u64>,
}

/// App file access
#[derive(Debug, Clone)]
pub struct AppFileAccess {
    /// App ID
    pub app_id: String,
    /// App name
    pub app_name: String,
    /// Full disk access
    pub full_disk_access: bool,
    /// Allowed folders
    pub allowed_folders: Vec<String>,
    /// Downloads folder access
    pub downloads_access: bool,
    /// Documents folder access
    pub documents_access: bool,
    /// Desktop folder access
    pub desktop_access: bool,
}

/// Background refresh settings
#[derive(Debug, Clone)]
pub struct BackgroundRefreshSettings {
    /// Globally enabled
    pub enabled: bool,
    /// Per-app settings
    pub app_settings: Vec<AppBackgroundRefresh>,
}

/// App background refresh setting
#[derive(Debug, Clone)]
pub struct AppBackgroundRefresh {
    /// App ID
    pub app_id: String,
    /// App name
    pub app_name: String,
    /// Allowed
    pub allowed: bool,
}

/// Diagnostics settings
#[derive(Debug, Clone)]
pub struct DiagnosticsSettings {
    /// Share diagnostics with system
    pub share_diagnostics: bool,
    /// Share crash reports
    pub share_crash_reports: bool,
    /// Share usage statistics
    pub share_usage_stats: bool,
    /// Improve by sharing analytics
    pub improve_analytics: bool,
}

/// Initialize privacy settings
pub fn init() {
    let mut state = PRIVACY_SETTINGS.lock();
    if state.is_some() {
        return;
    }

    *state = Some(PrivacySettings {
        location: LocationSettings {
            enabled: false,
            gps_enabled: true,
            wifi_location: true,
            app_permissions: Vec::new(),
            save_history: false,
            history_days: 7,
        },
        camera: DeviceAccessSettings {
            enabled: true,
            app_permissions: Vec::new(),
        },
        microphone: DeviceAccessSettings {
            enabled: true,
            app_permissions: Vec::new(),
        },
        screen_sharing: DeviceAccessSettings {
            enabled: true,
            app_permissions: Vec::new(),
        },
        file_access: Vec::new(),
        notification_access: Vec::new(),
        background_refresh: BackgroundRefreshSettings {
            enabled: true,
            app_settings: Vec::new(),
        },
        diagnostics: DiagnosticsSettings {
            share_diagnostics: false,
            share_crash_reports: true,
            share_usage_stats: false,
            improve_analytics: false,
        },
        track_recent_files: true,
        usage_data: false,
    });

    crate::kprintln!("privacy settings: initialized");
}

/// Set location services enabled
pub fn set_location_enabled(enabled: bool) {
    let mut state = PRIVACY_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.location.enabled = enabled;
    }
}

/// Is location enabled
pub fn is_location_enabled() -> bool {
    let state = PRIVACY_SETTINGS.lock();
    state.as_ref().map(|s| s.location.enabled).unwrap_or(false)
}

/// Set app location permission
pub fn set_app_location_permission(app_id: &str, app_name: &str, permission: LocationAccessLevel) {
    let mut state = PRIVACY_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        if let Some(perm) = s.location.app_permissions.iter_mut().find(|p| p.app_id == app_id) {
            perm.permission = permission;
        } else {
            s.location.app_permissions.push(LocationPermission {
                app_id: app_id.to_string(),
                app_name: app_name.to_string(),
                permission,
                last_accessed: None,
            });
        }
    }
}

/// Get location permissions
pub fn get_location_permissions() -> Vec<LocationPermission> {
    let state = PRIVACY_SETTINGS.lock();
    state.as_ref().map(|s| s.location.app_permissions.clone()).unwrap_or_default()
}

/// Set camera enabled
pub fn set_camera_enabled(enabled: bool) {
    let mut state = PRIVACY_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.camera.enabled = enabled;
    }
}

/// Is camera enabled
pub fn is_camera_enabled() -> bool {
    let state = PRIVACY_SETTINGS.lock();
    state.as_ref().map(|s| s.camera.enabled).unwrap_or(true)
}

/// Set app camera permission
pub fn set_app_camera_permission(app_id: &str, app_name: &str, allowed: bool) {
    let mut state = PRIVACY_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        if let Some(perm) = s.camera.app_permissions.iter_mut().find(|p| p.app_id == app_id) {
            perm.allowed = allowed;
        } else {
            s.camera.app_permissions.push(AppPermission {
                app_id: app_id.to_string(),
                app_name: app_name.to_string(),
                allowed,
                last_accessed: None,
            });
        }
    }
}

/// Get camera permissions
pub fn get_camera_permissions() -> Vec<AppPermission> {
    let state = PRIVACY_SETTINGS.lock();
    state.as_ref().map(|s| s.camera.app_permissions.clone()).unwrap_or_default()
}

/// Set microphone enabled
pub fn set_microphone_enabled(enabled: bool) {
    let mut state = PRIVACY_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.microphone.enabled = enabled;
    }
}

/// Is microphone enabled
pub fn is_microphone_enabled() -> bool {
    let state = PRIVACY_SETTINGS.lock();
    state.as_ref().map(|s| s.microphone.enabled).unwrap_or(true)
}

/// Set app microphone permission
pub fn set_app_microphone_permission(app_id: &str, app_name: &str, allowed: bool) {
    let mut state = PRIVACY_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        if let Some(perm) = s.microphone.app_permissions.iter_mut().find(|p| p.app_id == app_id) {
            perm.allowed = allowed;
        } else {
            s.microphone.app_permissions.push(AppPermission {
                app_id: app_id.to_string(),
                app_name: app_name.to_string(),
                allowed,
                last_accessed: None,
            });
        }
    }
}

/// Get microphone permissions
pub fn get_microphone_permissions() -> Vec<AppPermission> {
    let state = PRIVACY_SETTINGS.lock();
    state.as_ref().map(|s| s.microphone.app_permissions.clone()).unwrap_or_default()
}

/// Set screen sharing enabled
pub fn set_screen_sharing_enabled(enabled: bool) {
    let mut state = PRIVACY_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.screen_sharing.enabled = enabled;
    }
}

/// Set app screen sharing permission
pub fn set_app_screen_sharing_permission(app_id: &str, app_name: &str, allowed: bool) {
    let mut state = PRIVACY_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        if let Some(perm) = s.screen_sharing.app_permissions.iter_mut().find(|p| p.app_id == app_id) {
            perm.allowed = allowed;
        } else {
            s.screen_sharing.app_permissions.push(AppPermission {
                app_id: app_id.to_string(),
                app_name: app_name.to_string(),
                allowed,
                last_accessed: None,
            });
        }
    }
}

/// Grant full disk access to app
pub fn grant_full_disk_access(app_id: &str, app_name: &str) {
    let mut state = PRIVACY_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        if let Some(access) = s.file_access.iter_mut().find(|a| a.app_id == app_id) {
            access.full_disk_access = true;
        } else {
            s.file_access.push(AppFileAccess {
                app_id: app_id.to_string(),
                app_name: app_name.to_string(),
                full_disk_access: true,
                allowed_folders: Vec::new(),
                downloads_access: true,
                documents_access: true,
                desktop_access: true,
            });
        }
    }
}

/// Revoke full disk access from app
pub fn revoke_full_disk_access(app_id: &str) {
    let mut state = PRIVACY_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        if let Some(access) = s.file_access.iter_mut().find(|a| a.app_id == app_id) {
            access.full_disk_access = false;
        }
    }
}

/// Get file access permissions
pub fn get_file_access_permissions() -> Vec<AppFileAccess> {
    let state = PRIVACY_SETTINGS.lock();
    state.as_ref().map(|s| s.file_access.clone()).unwrap_or_default()
}

/// Set background refresh enabled
pub fn set_background_refresh_enabled(enabled: bool) {
    let mut state = PRIVACY_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.background_refresh.enabled = enabled;
    }
}

/// Set app background refresh
pub fn set_app_background_refresh(app_id: &str, app_name: &str, allowed: bool) {
    let mut state = PRIVACY_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        if let Some(setting) = s.background_refresh.app_settings.iter_mut().find(|a| a.app_id == app_id) {
            setting.allowed = allowed;
        } else {
            s.background_refresh.app_settings.push(AppBackgroundRefresh {
                app_id: app_id.to_string(),
                app_name: app_name.to_string(),
                allowed,
            });
        }
    }
}

/// Set diagnostics sharing
pub fn set_share_diagnostics(share: bool) {
    let mut state = PRIVACY_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.diagnostics.share_diagnostics = share;
    }
}

/// Set crash reports sharing
pub fn set_share_crash_reports(share: bool) {
    let mut state = PRIVACY_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.diagnostics.share_crash_reports = share;
    }
}

/// Get diagnostics settings
pub fn get_diagnostics() -> Option<DiagnosticsSettings> {
    let state = PRIVACY_SETTINGS.lock();
    state.as_ref().map(|s| s.diagnostics.clone())
}

/// Set track recent files
pub fn set_track_recent_files(enabled: bool) {
    let mut state = PRIVACY_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.track_recent_files = enabled;
    }
}

/// Clear recent files
pub fn clear_recent_files() {
    // TODO: Clear recent files from file manager
    crate::kprintln!("privacy: cleared recent files");
}

/// Privacy error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivacyError {
    NotInitialized,
    AppNotFound,
    PermissionDenied,
}
