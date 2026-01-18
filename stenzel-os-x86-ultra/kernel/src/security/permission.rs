//! Application Permission System
//!
//! Manages application permissions for accessing sensitive resources like:
//! - Camera, microphone, screen recording
//! - Location, contacts, calendar
//! - File system, network, USB devices
//! - Notifications, background activity

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use spin::Once;
use bitflags::bitflags;

use crate::sync::IrqSafeMutex;
use crate::util::{KError, KResult};

// ============================================================================
// Permission Types
// ============================================================================

/// Permission categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u32)]
pub enum Permission {
    // Media devices
    Camera = 0,
    Microphone = 1,
    ScreenRecording = 2,
    ScreenSharing = 3,

    // Location & sensors
    Location = 10,
    LocationBackground = 11,
    MotionSensors = 12,
    Bluetooth = 13,
    BluetoothScan = 14,

    // User data
    Contacts = 20,
    Calendar = 21,
    Photos = 22,
    Files = 23,
    Downloads = 24,

    // System access
    Notifications = 30,
    BackgroundActivity = 31,
    Autostart = 32,
    SystemSettings = 33,

    // Hardware
    Usb = 40,
    Serial = 41,
    Gpu = 42,

    // Network
    NetworkClient = 50,
    NetworkServer = 51,
    NetworkLocal = 52,

    // IPC
    Dbus = 60,
    Clipboard = 61,
    GlobalHotkeys = 62,
}

impl Permission {
    /// Get permission from numeric value
    pub fn from_u32(v: u32) -> Option<Self> {
        match v {
            0 => Some(Permission::Camera),
            1 => Some(Permission::Microphone),
            2 => Some(Permission::ScreenRecording),
            3 => Some(Permission::ScreenSharing),
            10 => Some(Permission::Location),
            11 => Some(Permission::LocationBackground),
            12 => Some(Permission::MotionSensors),
            13 => Some(Permission::Bluetooth),
            14 => Some(Permission::BluetoothScan),
            20 => Some(Permission::Contacts),
            21 => Some(Permission::Calendar),
            22 => Some(Permission::Photos),
            23 => Some(Permission::Files),
            24 => Some(Permission::Downloads),
            30 => Some(Permission::Notifications),
            31 => Some(Permission::BackgroundActivity),
            32 => Some(Permission::Autostart),
            33 => Some(Permission::SystemSettings),
            40 => Some(Permission::Usb),
            41 => Some(Permission::Serial),
            42 => Some(Permission::Gpu),
            50 => Some(Permission::NetworkClient),
            51 => Some(Permission::NetworkServer),
            52 => Some(Permission::NetworkLocal),
            60 => Some(Permission::Dbus),
            61 => Some(Permission::Clipboard),
            62 => Some(Permission::GlobalHotkeys),
            _ => None,
        }
    }

    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            Permission::Camera => "Camera",
            Permission::Microphone => "Microphone",
            Permission::ScreenRecording => "Screen Recording",
            Permission::ScreenSharing => "Screen Sharing",
            Permission::Location => "Location",
            Permission::LocationBackground => "Background Location",
            Permission::MotionSensors => "Motion Sensors",
            Permission::Bluetooth => "Bluetooth",
            Permission::BluetoothScan => "Bluetooth Scanning",
            Permission::Contacts => "Contacts",
            Permission::Calendar => "Calendar",
            Permission::Photos => "Photos",
            Permission::Files => "Files",
            Permission::Downloads => "Downloads",
            Permission::Notifications => "Notifications",
            Permission::BackgroundActivity => "Background Activity",
            Permission::Autostart => "Autostart",
            Permission::SystemSettings => "System Settings",
            Permission::Usb => "USB Devices",
            Permission::Serial => "Serial Ports",
            Permission::Gpu => "GPU Access",
            Permission::NetworkClient => "Network (Client)",
            Permission::NetworkServer => "Network (Server)",
            Permission::NetworkLocal => "Local Network",
            Permission::Dbus => "D-Bus",
            Permission::Clipboard => "Clipboard",
            Permission::GlobalHotkeys => "Global Hotkeys",
        }
    }

    /// Get description
    pub fn description(&self) -> &'static str {
        match self {
            Permission::Camera => "Access the camera to take photos and videos",
            Permission::Microphone => "Access the microphone to record audio",
            Permission::ScreenRecording => "Record the contents of the screen",
            Permission::ScreenSharing => "Share the screen with other applications",
            Permission::Location => "Access your current location",
            Permission::LocationBackground => "Access location while in background",
            Permission::MotionSensors => "Access accelerometer and gyroscope",
            Permission::Bluetooth => "Connect to Bluetooth devices",
            Permission::BluetoothScan => "Scan for nearby Bluetooth devices",
            Permission::Contacts => "Access your contacts",
            Permission::Calendar => "Access your calendar events",
            Permission::Photos => "Access your photo library",
            Permission::Files => "Access files on your system",
            Permission::Downloads => "Access your Downloads folder",
            Permission::Notifications => "Send notifications",
            Permission::BackgroundActivity => "Run in the background",
            Permission::Autostart => "Start automatically at login",
            Permission::SystemSettings => "Modify system settings",
            Permission::Usb => "Access USB devices",
            Permission::Serial => "Access serial ports",
            Permission::Gpu => "Direct GPU access",
            Permission::NetworkClient => "Make network connections",
            Permission::NetworkServer => "Accept network connections",
            Permission::NetworkLocal => "Access local network services",
            Permission::Dbus => "Communicate via D-Bus",
            Permission::Clipboard => "Access the clipboard",
            Permission::GlobalHotkeys => "Register global keyboard shortcuts",
        }
    }

    /// Check if permission is considered dangerous
    pub fn is_dangerous(&self) -> bool {
        matches!(self,
            Permission::Camera |
            Permission::Microphone |
            Permission::ScreenRecording |
            Permission::Location |
            Permission::LocationBackground |
            Permission::Contacts |
            Permission::Files |
            Permission::SystemSettings |
            Permission::NetworkServer
        )
    }
}

// ============================================================================
// Permission State
// ============================================================================

/// Permission grant state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionState {
    /// Not yet asked
    NotDetermined,
    /// Denied by user
    Denied,
    /// Granted by user
    Granted,
    /// Temporarily granted (for one session)
    GrantedOnce,
    /// Restricted by policy
    Restricted,
}

impl Default for PermissionState {
    fn default() -> Self {
        PermissionState::NotDetermined
    }
}

/// Permission request result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionResult {
    Granted,
    Denied,
    Pending,
    Error,
}

// ============================================================================
// Application Permissions
// ============================================================================

/// Application identifier
pub type AppId = String;

/// Permission entry for an application
#[derive(Debug, Clone)]
pub struct AppPermission {
    /// Permission type
    pub permission: Permission,
    /// Current state
    pub state: PermissionState,
    /// When it was granted/denied (Unix timestamp)
    pub timestamp: u64,
    /// Number of times accessed
    pub access_count: u64,
    /// Last access timestamp
    pub last_access: u64,
}

/// Permissions for a single application
#[derive(Debug, Clone)]
pub struct AppPermissions {
    /// Application ID
    pub app_id: AppId,
    /// Application name
    pub app_name: String,
    /// Path to application binary
    pub app_path: String,
    /// Permission entries
    permissions: BTreeMap<Permission, AppPermission>,
}

impl AppPermissions {
    pub fn new(app_id: &str, app_name: &str, app_path: &str) -> Self {
        Self {
            app_id: app_id.to_string(),
            app_name: app_name.to_string(),
            app_path: app_path.to_string(),
            permissions: BTreeMap::new(),
        }
    }

    /// Get permission state
    pub fn get_state(&self, permission: Permission) -> PermissionState {
        self.permissions.get(&permission)
            .map(|p| p.state)
            .unwrap_or(PermissionState::NotDetermined)
    }

    /// Set permission state
    pub fn set_state(&mut self, permission: Permission, state: PermissionState) {
        let now = crate::time::realtime().tv_sec as u64;

        if let Some(entry) = self.permissions.get_mut(&permission) {
            entry.state = state;
            entry.timestamp = now;
        } else {
            self.permissions.insert(permission, AppPermission {
                permission,
                state,
                timestamp: now,
                access_count: 0,
                last_access: 0,
            });
        }
    }

    /// Record an access
    pub fn record_access(&mut self, permission: Permission) {
        if let Some(entry) = self.permissions.get_mut(&permission) {
            entry.access_count += 1;
            entry.last_access = crate::time::realtime().tv_sec as u64;
        }
    }

    /// Check if permission is granted
    pub fn is_granted(&self, permission: Permission) -> bool {
        matches!(
            self.get_state(permission),
            PermissionState::Granted | PermissionState::GrantedOnce
        )
    }

    /// List all permissions
    pub fn list(&self) -> Vec<&AppPermission> {
        self.permissions.values().collect()
    }

    /// Reset all permissions
    pub fn reset_all(&mut self) {
        self.permissions.clear();
    }
}

// ============================================================================
// Permission Manager
// ============================================================================

/// Global permission manager
pub struct PermissionManager {
    /// App permissions indexed by app_id
    apps: IrqSafeMutex<BTreeMap<AppId, AppPermissions>>,
    /// Pending permission requests
    pending_requests: IrqSafeMutex<Vec<PermissionRequest>>,
    /// Request ID counter
    next_request_id: AtomicU64,
    /// Permission request callback (called when UI needs to show prompt)
    request_callback: IrqSafeMutex<Option<fn(&PermissionRequest)>>,
}

/// Permission request
#[derive(Debug, Clone)]
pub struct PermissionRequest {
    /// Request ID
    pub id: u64,
    /// Application ID
    pub app_id: AppId,
    /// Application name
    pub app_name: String,
    /// Requested permission
    pub permission: Permission,
    /// Timestamp
    pub timestamp: u64,
    /// PID of requesting process
    pub pid: u64,
}

impl PermissionManager {
    pub fn new() -> Self {
        Self {
            apps: IrqSafeMutex::new(BTreeMap::new()),
            pending_requests: IrqSafeMutex::new(Vec::new()),
            next_request_id: AtomicU64::new(1),
            request_callback: IrqSafeMutex::new(None),
        }
    }

    /// Register an application
    pub fn register_app(&self, app_id: &str, app_name: &str, app_path: &str) {
        let mut apps = self.apps.lock();
        if !apps.contains_key(app_id) {
            apps.insert(
                app_id.to_string(),
                AppPermissions::new(app_id, app_name, app_path),
            );
        }
    }

    /// Unregister an application
    pub fn unregister_app(&self, app_id: &str) {
        self.apps.lock().remove(app_id);
    }

    /// Check if permission is granted for an app
    pub fn check_permission(&self, app_id: &str, permission: Permission) -> PermissionState {
        self.apps.lock()
            .get(app_id)
            .map(|app| app.get_state(permission))
            .unwrap_or(PermissionState::NotDetermined)
    }

    /// Request permission (returns immediately, result via callback)
    pub fn request_permission(
        &self,
        app_id: &str,
        app_name: &str,
        permission: Permission,
        pid: u64,
    ) -> PermissionResult {
        // Check current state
        let current_state = self.check_permission(app_id, permission);

        match current_state {
            PermissionState::Granted => PermissionResult::Granted,
            PermissionState::GrantedOnce => PermissionResult::Granted,
            PermissionState::Denied => PermissionResult::Denied,
            PermissionState::Restricted => PermissionResult::Denied,
            PermissionState::NotDetermined => {
                // Create pending request
                let request = PermissionRequest {
                    id: self.next_request_id.fetch_add(1, Ordering::SeqCst),
                    app_id: app_id.to_string(),
                    app_name: app_name.to_string(),
                    permission,
                    timestamp: crate::time::realtime().tv_sec as u64,
                    pid,
                };

                self.pending_requests.lock().push(request.clone());

                // Notify UI
                if let Some(callback) = *self.request_callback.lock() {
                    callback(&request);
                }

                PermissionResult::Pending
            }
        }
    }

    /// Grant permission (called from UI/policy)
    pub fn grant_permission(&self, app_id: &str, permission: Permission, once: bool) {
        let mut apps = self.apps.lock();

        if let Some(app) = apps.get_mut(app_id) {
            app.set_state(
                permission,
                if once { PermissionState::GrantedOnce } else { PermissionState::Granted },
            );
        }

        // Remove from pending
        self.pending_requests.lock().retain(|r| {
            !(r.app_id == app_id && r.permission == permission)
        });

        crate::kprintln!("permission: Granted {:?} to {}{}",
            permission, app_id, if once { " (once)" } else { "" });
    }

    /// Deny permission
    pub fn deny_permission(&self, app_id: &str, permission: Permission) {
        let mut apps = self.apps.lock();

        if let Some(app) = apps.get_mut(app_id) {
            app.set_state(permission, PermissionState::Denied);
        }

        // Remove from pending
        self.pending_requests.lock().retain(|r| {
            !(r.app_id == app_id && r.permission == permission)
        });

        crate::kprintln!("permission: Denied {:?} to {}", permission, app_id);
    }

    /// Revoke permission
    pub fn revoke_permission(&self, app_id: &str, permission: Permission) {
        let mut apps = self.apps.lock();

        if let Some(app) = apps.get_mut(app_id) {
            app.set_state(permission, PermissionState::NotDetermined);
        }
    }

    /// Get pending requests
    pub fn get_pending_requests(&self) -> Vec<PermissionRequest> {
        self.pending_requests.lock().clone()
    }

    /// Set permission request callback
    pub fn set_request_callback(&self, callback: fn(&PermissionRequest)) {
        *self.request_callback.lock() = Some(callback);
    }

    /// Get all permissions for an app
    pub fn get_app_permissions(&self, app_id: &str) -> Option<AppPermissions> {
        self.apps.lock().get(app_id).cloned()
    }

    /// List all registered apps
    pub fn list_apps(&self) -> Vec<AppId> {
        self.apps.lock().keys().cloned().collect()
    }

    /// Record permission access
    pub fn record_access(&self, app_id: &str, permission: Permission) {
        let mut apps = self.apps.lock();
        if let Some(app) = apps.get_mut(app_id) {
            app.record_access(permission);
        }
    }

    /// Reset all permissions for an app
    pub fn reset_app_permissions(&self, app_id: &str) {
        let mut apps = self.apps.lock();
        if let Some(app) = apps.get_mut(app_id) {
            app.reset_all();
        }
    }

    /// Clear all "granted once" permissions (call on session end)
    pub fn clear_session_permissions(&self) {
        let mut apps = self.apps.lock();
        for app in apps.values_mut() {
            for perm in app.permissions.values_mut() {
                if perm.state == PermissionState::GrantedOnce {
                    perm.state = PermissionState::NotDetermined;
                }
            }
        }
    }

    /// Handle permission request by request ID
    pub fn handle_request(&self, request_id: u64, grant: bool, once: bool) {
        let request = {
            let requests = self.pending_requests.lock();
            requests.iter().find(|r| r.id == request_id).cloned()
        };

        if let Some(request) = request {
            if grant {
                self.grant_permission(&request.app_id, request.permission, once);
            } else {
                self.deny_permission(&request.app_id, request.permission);
            }
        }
    }
}

impl Default for PermissionManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Global Instance
// ============================================================================

static PERMISSION_MANAGER: Once<PermissionManager> = Once::new();

/// Initialize permission subsystem
pub fn init() {
    PERMISSION_MANAGER.call_once(PermissionManager::new);
    crate::kprintln!("permission: Application permission system initialized");
}

/// Get global permission manager
pub fn manager() -> &'static PermissionManager {
    PERMISSION_MANAGER.get().expect("Permission manager not initialized")
}

// ============================================================================
// Syscall Interface
// ============================================================================

/// Check if app has permission
pub fn sys_check_permission(app_id: &str, permission: u32) -> KResult<bool> {
    let perm = Permission::from_u32(permission)
        .ok_or(KError::Invalid)?;

    let state = manager().check_permission(app_id, perm);
    Ok(matches!(state, PermissionState::Granted | PermissionState::GrantedOnce))
}

/// Request permission from app
pub fn sys_request_permission(
    app_id: &str,
    app_name: &str,
    permission: u32,
    pid: u64,
) -> KResult<PermissionResult> {
    let perm = Permission::from_u32(permission)
        .ok_or(KError::Invalid)?;

    Ok(manager().request_permission(app_id, app_name, perm, pid))
}

/// Grant permission (admin only)
pub fn sys_grant_permission(app_id: &str, permission: u32, once: bool) -> KResult<()> {
    let perm = Permission::from_u32(permission)
        .ok_or(KError::Invalid)?;

    manager().grant_permission(app_id, perm, once);
    Ok(())
}

/// Deny permission (admin only)
pub fn sys_deny_permission(app_id: &str, permission: u32) -> KResult<()> {
    let perm = Permission::from_u32(permission)
        .ok_or(KError::Invalid)?;

    manager().deny_permission(app_id, perm);
    Ok(())
}

/// Revoke permission
pub fn sys_revoke_permission(app_id: &str, permission: u32) -> KResult<()> {
    let perm = Permission::from_u32(permission)
        .ok_or(KError::Invalid)?;

    manager().revoke_permission(app_id, perm);
    Ok(())
}

// ============================================================================
// Permission Enforcement Helpers
// ============================================================================

/// Check permission and record access if granted
pub fn check_and_record(app_id: &str, permission: Permission) -> bool {
    let mgr = manager();
    let state = mgr.check_permission(app_id, permission);

    if matches!(state, PermissionState::Granted | PermissionState::GrantedOnce) {
        mgr.record_access(app_id, permission);
        true
    } else {
        false
    }
}

/// Require permission or return error
pub fn require_permission(app_id: &str, permission: Permission) -> KResult<()> {
    if check_and_record(app_id, permission) {
        Ok(())
    } else {
        Err(KError::PermissionDenied)
    }
}

// ============================================================================
// Built-in Permission Presets
// ============================================================================

bitflags! {
    /// Permission set flags for quick checking
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PermissionSet: u64 {
        const CAMERA = 1 << 0;
        const MICROPHONE = 1 << 1;
        const SCREEN_RECORDING = 1 << 2;
        const LOCATION = 1 << 3;
        const CONTACTS = 1 << 4;
        const FILES = 1 << 5;
        const NETWORK = 1 << 6;
        const NOTIFICATIONS = 1 << 7;
        const BACKGROUND = 1 << 8;

        // Presets
        const NONE = 0;
        const MINIMAL = Self::NOTIFICATIONS.bits();
        const MEDIA = Self::CAMERA.bits() | Self::MICROPHONE.bits();
        const FULL_NETWORK = Self::NETWORK.bits();
    }
}

impl PermissionSet {
    /// Convert to list of Permission enum values
    pub fn to_permissions(&self) -> Vec<Permission> {
        let mut perms = Vec::new();
        if self.contains(PermissionSet::CAMERA) {
            perms.push(Permission::Camera);
        }
        if self.contains(PermissionSet::MICROPHONE) {
            perms.push(Permission::Microphone);
        }
        if self.contains(PermissionSet::SCREEN_RECORDING) {
            perms.push(Permission::ScreenRecording);
        }
        if self.contains(PermissionSet::LOCATION) {
            perms.push(Permission::Location);
        }
        if self.contains(PermissionSet::CONTACTS) {
            perms.push(Permission::Contacts);
        }
        if self.contains(PermissionSet::FILES) {
            perms.push(Permission::Files);
        }
        if self.contains(PermissionSet::NETWORK) {
            perms.push(Permission::NetworkClient);
        }
        if self.contains(PermissionSet::NOTIFICATIONS) {
            perms.push(Permission::Notifications);
        }
        if self.contains(PermissionSet::BACKGROUND) {
            perms.push(Permission::BackgroundActivity);
        }
        perms
    }
}
