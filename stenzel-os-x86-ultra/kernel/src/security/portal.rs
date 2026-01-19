//! Portal System for Sandboxed Applications
//!
//! Provides controlled access to system resources for sandboxed apps.
//! Similar to XDG Desktop Portals / Flatpak portals.

#![allow(dead_code)]

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use alloc::collections::BTreeMap;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::sync::IrqSafeMutex;
use crate::util::kprintln;

/// Portal system state
static PORTAL_SYSTEM: IrqSafeMutex<Option<PortalSystem>> = IrqSafeMutex::new(None);

/// Statistics
static STATS: PortalStats = PortalStats::new();

/// Portal type
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PortalType {
    /// File chooser (open/save dialogs)
    FileChooser,
    /// Directory access
    Documents,
    /// Screenshot/screen recording
    Screenshot,
    /// Screen sharing
    Screencast,
    /// Camera access
    Camera,
    /// Microphone access
    Microphone,
    /// Location services
    Location,
    /// Notifications
    Notification,
    /// Email sending
    Email,
    /// Print service
    Print,
    /// Network status
    NetworkMonitor,
    /// Power/inhibit
    Inhibit,
    /// Background apps
    Background,
    /// Game mode
    GameMode,
    /// Clipboard
    Clipboard,
    /// Secrets/keyring
    Secret,
    /// Settings
    Settings,
    /// Wallpaper
    Wallpaper,
    /// Account (OAuth)
    Account,
    /// Trash
    Trash,
    /// Memory mapping
    MemoryMap,
    /// Input capture
    InputCapture,
    /// Global shortcuts
    GlobalShortcuts,
    /// Remote desktop
    RemoteDesktop,
    /// Dynamic launcher
    DynamicLauncher,
    /// USB device access
    Usb,
}

impl PortalType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FileChooser => "org.freedesktop.portal.FileChooser",
            Self::Documents => "org.freedesktop.portal.Documents",
            Self::Screenshot => "org.freedesktop.portal.Screenshot",
            Self::Screencast => "org.freedesktop.portal.Screencast",
            Self::Camera => "org.freedesktop.portal.Camera",
            Self::Microphone => "org.freedesktop.portal.Microphone",
            Self::Location => "org.freedesktop.portal.Location",
            Self::Notification => "org.freedesktop.portal.Notification",
            Self::Email => "org.freedesktop.portal.Email",
            Self::Print => "org.freedesktop.portal.Print",
            Self::NetworkMonitor => "org.freedesktop.portal.NetworkMonitor",
            Self::Inhibit => "org.freedesktop.portal.Inhibit",
            Self::Background => "org.freedesktop.portal.Background",
            Self::GameMode => "org.freedesktop.portal.GameMode",
            Self::Clipboard => "org.freedesktop.portal.Clipboard",
            Self::Secret => "org.freedesktop.portal.Secret",
            Self::Settings => "org.freedesktop.portal.Settings",
            Self::Wallpaper => "org.freedesktop.portal.Wallpaper",
            Self::Account => "org.freedesktop.portal.Account",
            Self::Trash => "org.freedesktop.portal.Trash",
            Self::MemoryMap => "org.freedesktop.portal.MemoryMonitor",
            Self::InputCapture => "org.freedesktop.portal.InputCapture",
            Self::GlobalShortcuts => "org.freedesktop.portal.GlobalShortcuts",
            Self::RemoteDesktop => "org.freedesktop.portal.RemoteDesktop",
            Self::DynamicLauncher => "org.freedesktop.portal.DynamicLauncher",
            Self::Usb => "org.freedesktop.portal.Usb",
        }
    }

    pub fn is_sensitive(&self) -> bool {
        matches!(
            self,
            Self::Camera
                | Self::Microphone
                | Self::Location
                | Self::Screenshot
                | Self::Screencast
                | Self::Clipboard
                | Self::Secret
                | Self::InputCapture
                | Self::RemoteDesktop
                | Self::Usb
        )
    }

    pub fn requires_user_interaction(&self) -> bool {
        matches!(
            self,
            Self::FileChooser | Self::Print | Self::Email | Self::Account
        )
    }
}

/// Portal permission level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PermissionLevel {
    /// Always deny
    Deny = 0,
    /// Ask user every time
    Ask = 1,
    /// Ask once, then remember
    AskOnce = 2,
    /// Allow for this session
    Session = 3,
    /// Always allow
    Allow = 4,
}

impl PermissionLevel {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Session | Self::Allow)
    }

    pub fn needs_prompt(&self) -> bool {
        matches!(self, Self::Ask | Self::AskOnce)
    }
}

/// Portal request
#[derive(Debug, Clone)]
pub struct PortalRequest {
    /// Request ID
    pub id: u64,
    /// Portal type
    pub portal: PortalType,
    /// Requesting app ID
    pub app_id: String,
    /// Request parameters
    pub params: PortalParams,
    /// Timestamp
    pub timestamp: u64,
    /// User responded
    pub responded: bool,
    /// Result
    pub result: Option<PortalResult>,
}

/// Portal parameters
#[derive(Debug, Clone)]
pub enum PortalParams {
    FileChooser(FileChooserParams),
    Screenshot(ScreenshotParams),
    Screencast(ScreencastParams),
    Camera(CameraParams),
    Location(LocationParams),
    Notification(NotificationParams),
    Secret(SecretParams),
    Clipboard(ClipboardParams),
    Usb(UsbParams),
    Generic(BTreeMap<String, String>),
}

/// File chooser parameters
#[derive(Debug, Clone)]
pub struct FileChooserParams {
    pub mode: FileChooserMode,
    pub title: Option<String>,
    pub accept_label: Option<String>,
    pub filters: Vec<FileFilter>,
    pub current_folder: Option<String>,
    pub current_name: Option<String>,
    pub multiple: bool,
    pub directory: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileChooserMode {
    Open,
    Save,
    OpenMultiple,
    SelectFolder,
}

#[derive(Debug, Clone)]
pub struct FileFilter {
    pub name: String,
    pub patterns: Vec<String>,
    pub mime_types: Vec<String>,
}

/// Screenshot parameters
#[derive(Debug, Clone)]
pub struct ScreenshotParams {
    pub modal: bool,
    pub interactive: bool,
}

/// Screencast parameters
#[derive(Debug, Clone)]
pub struct ScreencastParams {
    pub types: ScreencastTypes,
    pub multiple: bool,
    pub cursor_mode: CursorMode,
    pub persist_mode: PersistMode,
    pub restore_token: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreencastTypes {
    Monitor,
    Window,
    Virtual,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorMode {
    Hidden,
    Embedded,
    Metadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistMode {
    DoNotPersist,
    Application,
    ExplicitlyRevoked,
}

/// Camera parameters
#[derive(Debug, Clone)]
pub struct CameraParams {
    pub device_id: Option<String>,
}

/// Location parameters
#[derive(Debug, Clone)]
pub struct LocationParams {
    pub distance_threshold: u32,
    pub time_threshold: u32,
    pub accuracy: LocationAccuracy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocationAccuracy {
    None,
    Country,
    City,
    Neighborhood,
    Street,
    Exact,
}

/// Notification parameters
#[derive(Debug, Clone)]
pub struct NotificationParams {
    pub id: String,
    pub title: String,
    pub body: Option<String>,
    pub icon: Option<String>,
    pub priority: NotificationPriority,
    pub default_action: Option<String>,
    pub buttons: Vec<NotificationButton>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationPriority {
    Low,
    Normal,
    High,
    Urgent,
}

#[derive(Debug, Clone)]
pub struct NotificationButton {
    pub action: String,
    pub label: String,
}

/// Secret parameters
#[derive(Debug, Clone)]
pub struct SecretParams {
    pub fd: Option<u32>,
}

/// Clipboard parameters
#[derive(Debug, Clone)]
pub struct ClipboardParams {
    pub session_handle: Option<String>,
}

/// USB parameters
#[derive(Debug, Clone)]
pub struct UsbParams {
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    pub device_class: Option<u8>,
}

/// Portal result
#[derive(Debug, Clone)]
pub enum PortalResult {
    Success(PortalResponse),
    Cancelled,
    Denied,
    Error(String),
}

/// Portal response
#[derive(Debug, Clone)]
pub enum PortalResponse {
    FileChooser(FileChooserResponse),
    Screenshot(ScreenshotResponse),
    Screencast(ScreencastResponse),
    Camera(CameraResponse),
    Location(LocationResponse),
    Notification(NotificationResponse),
    Secret(SecretResponse),
    Clipboard(ClipboardResponse),
    Usb(UsbResponse),
    Generic(BTreeMap<String, String>),
}

#[derive(Debug, Clone)]
pub struct FileChooserResponse {
    pub uris: Vec<String>,
    pub writable: bool,
}

#[derive(Debug, Clone)]
pub struct ScreenshotResponse {
    pub uri: String,
}

#[derive(Debug, Clone)]
pub struct ScreencastResponse {
    pub session_handle: String,
    pub streams: Vec<ScreencastStream>,
    pub restore_token: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ScreencastStream {
    pub node_id: u32,
    pub source_type: ScreencastTypes,
    pub position: Option<(i32, i32)>,
    pub size: Option<(u32, u32)>,
}

#[derive(Debug, Clone)]
pub struct CameraResponse {
    pub pipewire_remote: u32,
}

#[derive(Debug, Clone)]
pub struct LocationResponse {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: Option<f64>,
    pub accuracy: f64,
    pub speed: Option<f64>,
    pub heading: Option<f64>,
    pub timestamp: u64,
}

#[derive(Debug, Clone)]
pub struct NotificationResponse {
    pub action: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SecretResponse {
    pub token: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct ClipboardResponse {
    pub formats: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct UsbResponse {
    pub device_handle: u64,
    pub device_path: String,
}

/// App portal permissions
#[derive(Debug, Clone)]
pub struct AppPermissions {
    pub app_id: String,
    pub permissions: BTreeMap<PortalType, PermissionLevel>,
    pub granted_files: Vec<String>,
    pub granted_devices: Vec<String>,
    pub session_tokens: Vec<String>,
}

impl AppPermissions {
    fn new(app_id: String) -> Self {
        Self {
            app_id,
            permissions: BTreeMap::new(),
            granted_files: Vec::new(),
            granted_devices: Vec::new(),
            session_tokens: Vec::new(),
        }
    }
}

/// Portal error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortalError {
    NotInitialized,
    PermissionDenied,
    NotFound,
    InvalidRequest,
    UserCancelled,
    Timeout,
    Busy,
    NotSupported,
    InternalError,
}

impl PortalError {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotInitialized => "Portal system not initialized",
            Self::PermissionDenied => "Permission denied",
            Self::NotFound => "Resource not found",
            Self::InvalidRequest => "Invalid request",
            Self::UserCancelled => "User cancelled",
            Self::Timeout => "Request timeout",
            Self::Busy => "Portal busy",
            Self::NotSupported => "Not supported",
            Self::InternalError => "Internal error",
        }
    }
}

pub type PortalSystemResult<T> = Result<T, PortalError>;

/// Statistics
pub struct PortalStats {
    requests: AtomicU64,
    granted: AtomicU64,
    denied: AtomicU64,
    cancelled: AtomicU64,
    timeouts: AtomicU64,
}

impl PortalStats {
    const fn new() -> Self {
        Self {
            requests: AtomicU64::new(0),
            granted: AtomicU64::new(0),
            denied: AtomicU64::new(0),
            cancelled: AtomicU64::new(0),
            timeouts: AtomicU64::new(0),
        }
    }
}

/// Portal System
pub struct PortalSystem {
    /// Enabled portals
    enabled_portals: Vec<PortalType>,
    /// App permissions
    app_permissions: BTreeMap<String, AppPermissions>,
    /// Pending requests
    pending_requests: Vec<PortalRequest>,
    /// Next request ID
    next_request_id: u64,
    /// Request timeout (ms)
    request_timeout: u64,
    /// Default permission for new apps
    default_permission: PermissionLevel,
    /// Require user interaction for sensitive portals
    require_interaction: bool,
    /// Log all requests
    audit_logging: bool,
}

impl PortalSystem {
    fn new() -> Self {
        Self {
            enabled_portals: vec![
                PortalType::FileChooser,
                PortalType::Notification,
                PortalType::NetworkMonitor,
                PortalType::Settings,
                PortalType::Trash,
            ],
            app_permissions: BTreeMap::new(),
            pending_requests: Vec::new(),
            next_request_id: 1,
            request_timeout: 30000, // 30 seconds
            default_permission: PermissionLevel::Ask,
            require_interaction: true,
            audit_logging: true,
        }
    }

    /// Enable a portal
    pub fn enable_portal(&mut self, portal: PortalType) {
        if !self.enabled_portals.contains(&portal) {
            self.enabled_portals.push(portal);
            kprintln!("portal: Enabled {}", portal.as_str());
        }
    }

    /// Disable a portal
    pub fn disable_portal(&mut self, portal: PortalType) {
        self.enabled_portals.retain(|p| *p != portal);
        kprintln!("portal: Disabled {}", portal.as_str());
    }

    /// Check if portal is enabled
    pub fn is_enabled(&self, portal: PortalType) -> bool {
        self.enabled_portals.contains(&portal)
    }

    /// Set app permission
    pub fn set_permission(&mut self, app_id: &str, portal: PortalType, level: PermissionLevel) {
        let perms = self.app_permissions
            .entry(app_id.to_string())
            .or_insert_with(|| AppPermissions::new(app_id.to_string()));

        perms.permissions.insert(portal, level);

        if self.audit_logging {
            kprintln!(
                "portal: Set permission for {} on {} to {:?}",
                app_id,
                portal.as_str(),
                level
            );
        }
    }

    /// Get app permission
    pub fn get_permission(&self, app_id: &str, portal: PortalType) -> PermissionLevel {
        self.app_permissions
            .get(app_id)
            .and_then(|p| p.permissions.get(&portal))
            .copied()
            .unwrap_or(self.default_permission)
    }

    /// Request portal access
    pub fn request(
        &mut self,
        app_id: &str,
        portal: PortalType,
        params: PortalParams,
    ) -> PortalSystemResult<u64> {
        STATS.requests.fetch_add(1, Ordering::Relaxed);

        // Check if portal is enabled
        if !self.is_enabled(portal) {
            return Err(PortalError::NotSupported);
        }

        // Check permission
        let permission = self.get_permission(app_id, portal);

        if permission == PermissionLevel::Deny {
            STATS.denied.fetch_add(1, Ordering::Relaxed);
            if self.audit_logging {
                kprintln!("portal: Denied {} access to {} for {}", app_id, portal.as_str(), app_id);
            }
            return Err(PortalError::PermissionDenied);
        }

        // Create request
        let request_id = self.next_request_id;
        self.next_request_id += 1;

        let request = PortalRequest {
            id: request_id,
            portal,
            app_id: app_id.to_string(),
            params,
            timestamp: crate::time::uptime_ms(),
            responded: false,
            result: None,
        };

        if self.audit_logging {
            kprintln!(
                "portal: Request {} from {} for {}",
                request_id,
                app_id,
                portal.as_str()
            );
        }

        // If permission is already granted, process immediately
        if permission.is_allowed() && !portal.requires_user_interaction() {
            return self.process_request_immediate(request);
        }

        // Queue for user interaction
        self.pending_requests.push(request);
        Ok(request_id)
    }

    /// Process request immediately (already permitted)
    fn process_request_immediate(&mut self, mut request: PortalRequest) -> PortalSystemResult<u64> {
        let response = self.generate_response(&request)?;
        request.responded = true;
        request.result = Some(PortalResult::Success(response));

        STATS.granted.fetch_add(1, Ordering::Relaxed);

        // Grant file access if needed
        if let PortalParams::FileChooser(ref params) = request.params {
            if let Some(folder) = &params.current_folder {
                self.grant_file_access(&request.app_id, folder);
            }
        }

        Ok(request.id)
    }

    /// Generate response for request
    fn generate_response(&self, request: &PortalRequest) -> PortalSystemResult<PortalResponse> {
        match &request.params {
            PortalParams::FileChooser(params) => {
                // Would show file chooser dialog
                Ok(PortalResponse::FileChooser(FileChooserResponse {
                    uris: vec![params.current_folder.clone().unwrap_or_else(|| "/home".to_string())],
                    writable: matches!(params.mode, FileChooserMode::Save),
                }))
            }
            PortalParams::Screenshot(_) => {
                // Would capture screenshot
                Ok(PortalResponse::Screenshot(ScreenshotResponse {
                    uri: "file:///tmp/screenshot.png".to_string(),
                }))
            }
            PortalParams::Screencast(params) => {
                Ok(PortalResponse::Screencast(ScreencastResponse {
                    session_handle: alloc::format!("/session/{}", request.id),
                    streams: vec![ScreencastStream {
                        node_id: 1,
                        source_type: params.types,
                        position: Some((0, 0)),
                        size: Some((1920, 1080)),
                    }],
                    restore_token: if params.persist_mode != PersistMode::DoNotPersist {
                        Some(alloc::format!("token_{}", request.id))
                    } else {
                        None
                    },
                }))
            }
            PortalParams::Camera(_) => {
                Ok(PortalResponse::Camera(CameraResponse {
                    pipewire_remote: 1,
                }))
            }
            PortalParams::Location(_) => {
                Ok(PortalResponse::Location(LocationResponse {
                    latitude: 0.0,
                    longitude: 0.0,
                    altitude: None,
                    accuracy: 1000.0,
                    speed: None,
                    heading: None,
                    timestamp: crate::time::uptime_ms(),
                }))
            }
            PortalParams::Notification(params) => {
                // Would show notification
                Ok(PortalResponse::Notification(NotificationResponse {
                    action: params.default_action.clone(),
                }))
            }
            PortalParams::Secret(_) => {
                Ok(PortalResponse::Secret(SecretResponse {
                    token: vec![0u8; 32],
                }))
            }
            PortalParams::Clipboard(_) => {
                Ok(PortalResponse::Clipboard(ClipboardResponse {
                    formats: vec!["text/plain".to_string()],
                }))
            }
            PortalParams::Usb(params) => {
                Ok(PortalResponse::Usb(UsbResponse {
                    device_handle: 1,
                    device_path: alloc::format!(
                        "/dev/usb/{}:{}",
                        params.vendor_id.unwrap_or(0),
                        params.product_id.unwrap_or(0)
                    ),
                }))
            }
            PortalParams::Generic(_) => {
                Ok(PortalResponse::Generic(BTreeMap::new()))
            }
        }
    }

    /// Respond to pending request (user decision)
    pub fn respond(&mut self, request_id: u64, allowed: bool) -> PortalSystemResult<()> {
        let idx = self.pending_requests
            .iter()
            .position(|r| r.id == request_id)
            .ok_or(PortalError::NotFound)?;

        let mut request = self.pending_requests.remove(idx);

        if allowed {
            let response = self.generate_response(&request)?;
            request.result = Some(PortalResult::Success(response));
            STATS.granted.fetch_add(1, Ordering::Relaxed);

            // Update permission if AskOnce
            let permission = self.get_permission(&request.app_id, request.portal);
            if permission == PermissionLevel::AskOnce {
                self.set_permission(&request.app_id, request.portal, PermissionLevel::Allow);
            }
        } else {
            request.result = Some(PortalResult::Denied);
            STATS.denied.fetch_add(1, Ordering::Relaxed);

            // Update permission if AskOnce
            let permission = self.get_permission(&request.app_id, request.portal);
            if permission == PermissionLevel::AskOnce {
                self.set_permission(&request.app_id, request.portal, PermissionLevel::Deny);
            }
        }

        request.responded = true;

        if self.audit_logging {
            kprintln!(
                "portal: Request {} {} for {}",
                request_id,
                if allowed { "granted" } else { "denied" },
                request.app_id
            );
        }

        Ok(())
    }

    /// Cancel request
    pub fn cancel(&mut self, request_id: u64) -> PortalSystemResult<()> {
        let idx = self.pending_requests
            .iter()
            .position(|r| r.id == request_id)
            .ok_or(PortalError::NotFound)?;

        self.pending_requests.remove(idx);
        STATS.cancelled.fetch_add(1, Ordering::Relaxed);

        if self.audit_logging {
            kprintln!("portal: Request {} cancelled", request_id);
        }

        Ok(())
    }

    /// Get pending requests for user
    pub fn get_pending(&self) -> Vec<&PortalRequest> {
        self.pending_requests.iter().collect()
    }

    /// Grant file access to app
    pub fn grant_file_access(&mut self, app_id: &str, path: &str) {
        let perms = self.app_permissions
            .entry(app_id.to_string())
            .or_insert_with(|| AppPermissions::new(app_id.to_string()));

        if !perms.granted_files.contains(&path.to_string()) {
            perms.granted_files.push(path.to_string());
        }

        if self.audit_logging {
            kprintln!("portal: Granted {} access to {}", app_id, path);
        }
    }

    /// Check if app has file access
    pub fn has_file_access(&self, app_id: &str, path: &str) -> bool {
        self.app_permissions
            .get(app_id)
            .map(|p| {
                p.granted_files.iter().any(|f| {
                    path.starts_with(f) || f == path
                })
            })
            .unwrap_or(false)
    }

    /// Revoke file access
    pub fn revoke_file_access(&mut self, app_id: &str, path: &str) {
        if let Some(perms) = self.app_permissions.get_mut(app_id) {
            perms.granted_files.retain(|f| f != path);
        }
    }

    /// Grant device access
    pub fn grant_device_access(&mut self, app_id: &str, device: &str) {
        let perms = self.app_permissions
            .entry(app_id.to_string())
            .or_insert_with(|| AppPermissions::new(app_id.to_string()));

        if !perms.granted_devices.contains(&device.to_string()) {
            perms.granted_devices.push(device.to_string());
        }
    }

    /// Check device access
    pub fn has_device_access(&self, app_id: &str, device: &str) -> bool {
        self.app_permissions
            .get(app_id)
            .map(|p| p.granted_devices.contains(&device.to_string()))
            .unwrap_or(false)
    }

    /// Revoke all permissions for app
    pub fn revoke_all(&mut self, app_id: &str) {
        self.app_permissions.remove(app_id);
        if self.audit_logging {
            kprintln!("portal: Revoked all permissions for {}", app_id);
        }
    }

    /// List app permissions
    pub fn list_permissions(&self, app_id: &str) -> Option<&AppPermissions> {
        self.app_permissions.get(app_id)
    }

    /// List all apps with permissions
    pub fn list_apps(&self) -> Vec<&str> {
        self.app_permissions.keys().map(|s| s.as_str()).collect()
    }

    /// Clean up timed out requests
    pub fn cleanup_timeouts(&mut self) {
        let now = crate::time::uptime_ms();
        let timeout = self.request_timeout;

        let timed_out: Vec<u64> = self.pending_requests
            .iter()
            .filter(|r| now - r.timestamp > timeout)
            .map(|r| r.id)
            .collect();

        for id in &timed_out {
            if let Some(idx) = self.pending_requests.iter().position(|r| r.id == *id) {
                self.pending_requests.remove(idx);
                STATS.timeouts.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Get statistics
    pub fn get_stats(&self) -> (u64, u64, u64, u64, u64) {
        (
            STATS.requests.load(Ordering::Relaxed),
            STATS.granted.load(Ordering::Relaxed),
            STATS.denied.load(Ordering::Relaxed),
            STATS.cancelled.load(Ordering::Relaxed),
            STATS.timeouts.load(Ordering::Relaxed),
        )
    }
}

// Public API

/// Initialize portal system
pub fn init() {
    let mut guard = PORTAL_SYSTEM.lock();
    if guard.is_none() {
        *guard = Some(PortalSystem::new());
        kprintln!("portal: Initialized");
    }
}

/// Enable portal
pub fn enable_portal(portal: PortalType) {
    let mut guard = PORTAL_SYSTEM.lock();
    if let Some(system) = guard.as_mut() {
        system.enable_portal(portal);
    }
}

/// Disable portal
pub fn disable_portal(portal: PortalType) {
    let mut guard = PORTAL_SYSTEM.lock();
    if let Some(system) = guard.as_mut() {
        system.disable_portal(portal);
    }
}

/// Check if portal is enabled
pub fn is_enabled(portal: PortalType) -> bool {
    let guard = PORTAL_SYSTEM.lock();
    guard.as_ref().map(|s| s.is_enabled(portal)).unwrap_or(false)
}

/// Set permission
pub fn set_permission(app_id: &str, portal: PortalType, level: PermissionLevel) {
    let mut guard = PORTAL_SYSTEM.lock();
    if let Some(system) = guard.as_mut() {
        system.set_permission(app_id, portal, level);
    }
}

/// Get permission
pub fn get_permission(app_id: &str, portal: PortalType) -> PermissionLevel {
    let guard = PORTAL_SYSTEM.lock();
    guard.as_ref()
        .map(|s| s.get_permission(app_id, portal))
        .unwrap_or(PermissionLevel::Deny)
}

/// Request portal access
pub fn request(app_id: &str, portal: PortalType, params: PortalParams) -> PortalSystemResult<u64> {
    let mut guard = PORTAL_SYSTEM.lock();
    let system = guard.as_mut().ok_or(PortalError::NotInitialized)?;
    system.request(app_id, portal, params)
}

/// Respond to request
pub fn respond(request_id: u64, allowed: bool) -> PortalSystemResult<()> {
    let mut guard = PORTAL_SYSTEM.lock();
    let system = guard.as_mut().ok_or(PortalError::NotInitialized)?;
    system.respond(request_id, allowed)
}

/// Cancel request
pub fn cancel(request_id: u64) -> PortalSystemResult<()> {
    let mut guard = PORTAL_SYSTEM.lock();
    let system = guard.as_mut().ok_or(PortalError::NotInitialized)?;
    system.cancel(request_id)
}

/// Grant file access
pub fn grant_file_access(app_id: &str, path: &str) {
    let mut guard = PORTAL_SYSTEM.lock();
    if let Some(system) = guard.as_mut() {
        system.grant_file_access(app_id, path);
    }
}

/// Check file access
pub fn has_file_access(app_id: &str, path: &str) -> bool {
    let guard = PORTAL_SYSTEM.lock();
    guard.as_ref()
        .map(|s| s.has_file_access(app_id, path))
        .unwrap_or(false)
}

/// Revoke all permissions
pub fn revoke_all(app_id: &str) {
    let mut guard = PORTAL_SYSTEM.lock();
    if let Some(system) = guard.as_mut() {
        system.revoke_all(app_id);
    }
}

/// List apps
pub fn list_apps() -> Vec<String> {
    let guard = PORTAL_SYSTEM.lock();
    guard.as_ref()
        .map(|s| s.list_apps().iter().map(|s| s.to_string()).collect())
        .unwrap_or_default()
}

/// Get statistics
pub fn get_stats() -> (u64, u64, u64, u64, u64) {
    let guard = PORTAL_SYSTEM.lock();
    guard.as_ref()
        .map(|s| s.get_stats())
        .unwrap_or((0, 0, 0, 0, 0))
}
