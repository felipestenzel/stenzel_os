//! Flatpak Compatibility Layer
//!
//! Provides support for running Flatpak applications.
//! Implements OSTree/OCI image handling, sandbox environment, portals,
//! and runtime management.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::collections::BTreeMap;
use alloc::boxed::Box;
use crate::sync::TicketSpinlock;

/// Global Flatpak runtime state
static FLATPAK_STATE: TicketSpinlock<Option<FlatpakRuntime>> = TicketSpinlock::new(None);

/// Flatpak application ID
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AppId(pub String);

impl AppId {
    pub fn new(id: &str) -> Self {
        Self(String::from(id))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Flatpak branch/version
#[derive(Debug, Clone)]
pub struct Branch(pub String);

impl Branch {
    pub fn stable() -> Self {
        Self(String::from("stable"))
    }

    pub fn master() -> Self {
        Self(String::from("master"))
    }
}

/// Architecture type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    X86_64,
    Aarch64,
    I686,
    Armv7,
}

impl Arch {
    pub fn current() -> Self {
        Arch::X86_64 // Stenzel OS is x86_64 only for now
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Arch::X86_64 => "x86_64",
            Arch::Aarch64 => "aarch64",
            Arch::I686 => "i686",
            Arch::Armv7 => "arm",
        }
    }
}

/// Flatpak remote repository
#[derive(Debug, Clone)]
pub struct Remote {
    pub name: String,
    pub url: String,
    pub title: Option<String>,
    pub collection_id: Option<String>,
    pub gpg_key: Option<Vec<u8>>,
    pub enabled: bool,
    pub priority: i32,
}

impl Remote {
    pub fn flathub() -> Self {
        Self {
            name: String::from("flathub"),
            url: String::from("https://flathub.org/repo/"),
            title: Some(String::from("Flathub")),
            collection_id: Some(String::from("org.flathub.Stable")),
            gpg_key: None,
            enabled: true,
            priority: 1,
        }
    }

    pub fn new(name: &str, url: &str) -> Self {
        Self {
            name: String::from(name),
            url: String::from(url),
            title: None,
            collection_id: None,
            gpg_key: None,
            enabled: true,
            priority: 10,
        }
    }
}

/// Flatpak installation type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallationType {
    System,
    User,
}

/// Flatpak reference (app or runtime)
#[derive(Debug, Clone)]
pub struct Ref {
    pub kind: RefKind,
    pub name: String,
    pub arch: Arch,
    pub branch: Branch,
}

impl Ref {
    pub fn app(name: &str, arch: Arch, branch: Branch) -> Self {
        Self {
            kind: RefKind::App,
            name: String::from(name),
            arch,
            branch,
        }
    }

    pub fn runtime(name: &str, arch: Arch, branch: Branch) -> Self {
        Self {
            kind: RefKind::Runtime,
            name: String::from(name),
            arch,
            branch,
        }
    }

    pub fn format_ref(&self) -> String {
        let kind_str = match self.kind {
            RefKind::App => "app",
            RefKind::Runtime => "runtime",
        };
        let mut result = String::new();
        result.push_str(kind_str);
        result.push('/');
        result.push_str(&self.name);
        result.push('/');
        result.push_str(self.arch.as_str());
        result.push('/');
        result.push_str(&self.branch.0);
        result
    }
}

/// Reference kind
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefKind {
    App,
    Runtime,
}

/// Installed application metadata
#[derive(Debug, Clone)]
pub struct InstalledApp {
    pub app_ref: Ref,
    pub remote: String,
    pub commit: String,
    pub metadata: AppMetadata,
    pub size: u64,
    pub install_time: u64,
    pub is_current: bool,
    pub deploy_path: String,
}

/// Application metadata (from org.freedesktop.Flatpak metadata file)
#[derive(Debug, Clone)]
pub struct AppMetadata {
    pub name: String,
    pub runtime: String,
    pub sdk: Option<String>,
    pub command: String,
    pub permissions: Permissions,
    pub finish_args: Vec<String>,
    pub extensions: Vec<Extension>,
}

impl AppMetadata {
    pub fn new(name: &str, runtime: &str, command: &str) -> Self {
        Self {
            name: String::from(name),
            runtime: String::from(runtime),
            sdk: None,
            command: String::from(command),
            permissions: Permissions::default(),
            finish_args: Vec::new(),
            extensions: Vec::new(),
        }
    }
}

/// Flatpak permissions
#[derive(Debug, Clone, Default)]
pub struct Permissions {
    /// Share host network
    pub share_network: bool,
    /// Share host IPC
    pub share_ipc: bool,
    /// Socket access
    pub sockets: Vec<SocketPermission>,
    /// Device access
    pub devices: Vec<DevicePermission>,
    /// Filesystem access
    pub filesystem: Vec<FilesystemPermission>,
    /// D-Bus access
    pub dbus_access: Vec<DbusPermission>,
    /// Environment variables
    pub environment: BTreeMap<String, String>,
    /// Allow multiarch
    pub allow_multiarch: bool,
    /// Allow Bluetooth
    pub allow_bluetooth: bool,
    /// Allow Canbus
    pub allow_canbus: bool,
    /// Persistent directories
    pub persistent_dirs: Vec<String>,
}

/// Socket permission types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SocketPermission {
    X11,
    Wayland,
    Fallback,
    PulseAudio,
    System,
    Session,
    Ssh,
    Pcsc,
    Cups,
    GpgAgent,
}

impl SocketPermission {
    pub fn as_str(&self) -> &'static str {
        match self {
            SocketPermission::X11 => "x11",
            SocketPermission::Wayland => "wayland",
            SocketPermission::Fallback => "fallback-x11",
            SocketPermission::PulseAudio => "pulseaudio",
            SocketPermission::System => "system-bus",
            SocketPermission::Session => "session-bus",
            SocketPermission::Ssh => "ssh-auth",
            SocketPermission::Pcsc => "pcsc",
            SocketPermission::Cups => "cups",
            SocketPermission::GpgAgent => "gpg-agent",
        }
    }
}

/// Device permission types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DevicePermission {
    Dri,
    Kvm,
    Shm,
    All,
}

/// Filesystem permission
#[derive(Debug, Clone)]
pub struct FilesystemPermission {
    pub path: String,
    pub access: FilesystemAccess,
}

/// Filesystem access level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemAccess {
    ReadOnly,
    ReadWrite,
    Create,
}

/// D-Bus permission
#[derive(Debug, Clone)]
pub struct DbusPermission {
    pub bus_type: DbusType,
    pub name: String,
    pub access: DbusAccess,
}

/// D-Bus bus type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbusType {
    Session,
    System,
}

/// D-Bus access level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbusAccess {
    Talk,
    Own,
    See,
}

/// Extension point
#[derive(Debug, Clone)]
pub struct Extension {
    pub name: String,
    pub directory: String,
    pub version: Option<String>,
    pub subdirectories: bool,
    pub no_autodownload: bool,
    pub autodelete: bool,
}

/// Installed runtime metadata
#[derive(Debug, Clone)]
pub struct InstalledRuntime {
    pub runtime_ref: Ref,
    pub remote: String,
    pub commit: String,
    pub sdk_ref: Option<Ref>,
    pub size: u64,
    pub install_time: u64,
    pub deploy_path: String,
}

/// Flatpak runtime manager
#[derive(Debug)]
pub struct FlatpakRuntime {
    /// Installed applications
    apps: BTreeMap<String, InstalledApp>,
    /// Installed runtimes
    runtimes: BTreeMap<String, InstalledRuntime>,
    /// Configured remotes
    remotes: Vec<Remote>,
    /// System installation path
    system_path: String,
    /// User installation path
    user_path: String,
    /// Running instances
    instances: BTreeMap<u32, RunningInstance>,
    /// Portal manager
    portals: PortalManager,
    /// Next instance ID
    next_instance_id: u32,
}

impl FlatpakRuntime {
    pub fn new() -> Self {
        Self {
            apps: BTreeMap::new(),
            runtimes: BTreeMap::new(),
            remotes: vec![Remote::flathub()],
            system_path: String::from("/var/lib/flatpak"),
            user_path: String::from("~/.local/share/flatpak"),
            instances: BTreeMap::new(),
            portals: PortalManager::new(),
            next_instance_id: 1,
        }
    }
}

/// Running Flatpak instance
#[derive(Debug)]
pub struct RunningInstance {
    pub instance_id: u32,
    pub app_ref: Ref,
    pub pid: u32,
    pub sandbox: SandboxConfig,
    pub start_time: u64,
}

/// Sandbox configuration (Bubblewrap equivalent)
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    /// User namespace
    pub user_ns: bool,
    /// PID namespace
    pub pid_ns: bool,
    /// Network namespace (if share_network is false)
    pub net_ns: bool,
    /// IPC namespace (if share_ipc is false)
    pub ipc_ns: bool,
    /// Mount namespace
    pub mount_ns: bool,
    /// UTS namespace
    pub uts_ns: bool,
    /// Seccomp filter
    pub seccomp: SeccompConfig,
    /// Root filesystem path
    pub rootfs: String,
    /// Bind mounts
    pub bind_mounts: Vec<BindMount>,
    /// Environment variables
    pub env: BTreeMap<String, String>,
    /// Working directory
    pub cwd: String,
}

impl SandboxConfig {
    pub fn new(rootfs: &str) -> Self {
        Self {
            user_ns: true,
            pid_ns: true,
            net_ns: false,
            ipc_ns: true,
            mount_ns: true,
            uts_ns: true,
            seccomp: SeccompConfig::default_flatpak(),
            rootfs: String::from(rootfs),
            bind_mounts: Vec::new(),
            env: BTreeMap::new(),
            cwd: String::from("/app"),
        }
    }

    pub fn add_bind_mount(&mut self, source: &str, dest: &str, readonly: bool) {
        self.bind_mounts.push(BindMount {
            source: String::from(source),
            dest: String::from(dest),
            readonly,
        });
    }
}

/// Bind mount configuration
#[derive(Debug, Clone)]
pub struct BindMount {
    pub source: String,
    pub dest: String,
    pub readonly: bool,
}

/// Seccomp configuration
#[derive(Debug, Clone)]
pub struct SeccompConfig {
    /// Default action for unlisted syscalls
    pub default_action: SeccompAction,
    /// Allowed syscalls
    pub allowed_syscalls: Vec<u32>,
    /// Blocked syscalls
    pub blocked_syscalls: Vec<u32>,
}

impl SeccompConfig {
    pub fn default_flatpak() -> Self {
        Self {
            default_action: SeccompAction::Errno(1), // EPERM
            allowed_syscalls: Self::flatpak_allowed_syscalls(),
            blocked_syscalls: Vec::new(),
        }
    }

    fn flatpak_allowed_syscalls() -> Vec<u32> {
        // Common syscalls allowed in Flatpak sandbox
        vec![
            0,   // read
            1,   // write
            2,   // open
            3,   // close
            4,   // stat
            5,   // fstat
            6,   // lstat
            7,   // poll
            8,   // lseek
            9,   // mmap
            10,  // mprotect
            11,  // munmap
            12,  // brk
            // ... many more syscalls
            60,  // exit
            231, // exit_group
        ]
    }
}

/// Seccomp action
#[derive(Debug, Clone, Copy)]
pub enum SeccompAction {
    Allow,
    Kill,
    Errno(i32),
    Trap,
    Log,
}

/// Portal manager for XDG Desktop Portals
#[derive(Debug)]
pub struct PortalManager {
    /// Registered portals
    portals: BTreeMap<String, Box<dyn Portal + Send>>,
}

impl PortalManager {
    pub fn new() -> Self {
        let mut portals: BTreeMap<String, Box<dyn Portal + Send>> = BTreeMap::new();

        // Register standard portals
        portals.insert(String::from("org.freedesktop.portal.FileChooser"),
            Box::new(FileChooserPortal::new()));
        portals.insert(String::from("org.freedesktop.portal.OpenURI"),
            Box::new(OpenUriPortal::new()));
        portals.insert(String::from("org.freedesktop.portal.Notification"),
            Box::new(NotificationPortal::new()));
        portals.insert(String::from("org.freedesktop.portal.Screenshot"),
            Box::new(ScreenshotPortal::new()));
        portals.insert(String::from("org.freedesktop.portal.Camera"),
            Box::new(CameraPortal::new()));

        Self { portals }
    }

    pub fn handle_request(&self, portal_name: &str, method: &str, args: &[PortalArg])
        -> Result<PortalResponse, PortalError>
    {
        if let Some(portal) = self.portals.get(portal_name) {
            portal.handle_method(method, args)
        } else {
            Err(PortalError::UnknownPortal)
        }
    }
}

/// Portal trait for XDG Desktop Portals
pub trait Portal: core::fmt::Debug + Send {
    fn name(&self) -> &str;
    fn version(&self) -> u32;
    fn handle_method(&self, method: &str, args: &[PortalArg]) -> Result<PortalResponse, PortalError>;
}

/// Portal argument
#[derive(Debug, Clone)]
pub enum PortalArg {
    String(String),
    Uint32(u32),
    Int32(i32),
    Bool(bool),
    ByteArray(Vec<u8>),
    StringArray(Vec<String>),
    Dict(BTreeMap<String, PortalArg>),
    Handle(u32),
}

/// Portal response
#[derive(Debug)]
pub enum PortalResponse {
    Success(BTreeMap<String, PortalArg>),
    Cancelled,
    Failed,
}

/// Portal error
#[derive(Debug)]
pub enum PortalError {
    UnknownPortal,
    UnknownMethod,
    InvalidArgs,
    PermissionDenied,
    Cancelled,
    Failed,
}

/// FileChooser portal
#[derive(Debug)]
pub struct FileChooserPortal {
    version: u32,
}

impl FileChooserPortal {
    pub fn new() -> Self {
        Self { version: 3 }
    }
}

impl Portal for FileChooserPortal {
    fn name(&self) -> &str { "org.freedesktop.portal.FileChooser" }
    fn version(&self) -> u32 { self.version }

    fn handle_method(&self, method: &str, _args: &[PortalArg]) -> Result<PortalResponse, PortalError> {
        match method {
            "OpenFile" => {
                // TODO: Show native file chooser dialog
                let mut result = BTreeMap::new();
                result.insert(String::from("uris"),
                    PortalArg::StringArray(vec![String::from("file:///home/user/document.txt")]));
                Ok(PortalResponse::Success(result))
            }
            "SaveFile" => {
                let mut result = BTreeMap::new();
                result.insert(String::from("uris"),
                    PortalArg::StringArray(vec![String::from("file:///home/user/saved.txt")]));
                Ok(PortalResponse::Success(result))
            }
            "SaveFiles" => {
                let mut result = BTreeMap::new();
                result.insert(String::from("uris"), PortalArg::StringArray(Vec::new()));
                Ok(PortalResponse::Success(result))
            }
            _ => Err(PortalError::UnknownMethod),
        }
    }
}

/// OpenURI portal
#[derive(Debug)]
pub struct OpenUriPortal {
    version: u32,
}

impl OpenUriPortal {
    pub fn new() -> Self {
        Self { version: 4 }
    }
}

impl Portal for OpenUriPortal {
    fn name(&self) -> &str { "org.freedesktop.portal.OpenURI" }
    fn version(&self) -> u32 { self.version }

    fn handle_method(&self, method: &str, _args: &[PortalArg]) -> Result<PortalResponse, PortalError> {
        match method {
            "OpenURI" => {
                // TODO: Open URI with appropriate handler
                Ok(PortalResponse::Success(BTreeMap::new()))
            }
            "OpenFile" => {
                Ok(PortalResponse::Success(BTreeMap::new()))
            }
            "OpenDirectory" => {
                Ok(PortalResponse::Success(BTreeMap::new()))
            }
            _ => Err(PortalError::UnknownMethod),
        }
    }
}

/// Notification portal
#[derive(Debug)]
pub struct NotificationPortal {
    version: u32,
}

impl NotificationPortal {
    pub fn new() -> Self {
        Self { version: 1 }
    }
}

impl Portal for NotificationPortal {
    fn name(&self) -> &str { "org.freedesktop.portal.Notification" }
    fn version(&self) -> u32 { self.version }

    fn handle_method(&self, method: &str, _args: &[PortalArg]) -> Result<PortalResponse, PortalError> {
        match method {
            "AddNotification" => {
                // TODO: Show notification
                Ok(PortalResponse::Success(BTreeMap::new()))
            }
            "RemoveNotification" => {
                Ok(PortalResponse::Success(BTreeMap::new()))
            }
            _ => Err(PortalError::UnknownMethod),
        }
    }
}

/// Screenshot portal
#[derive(Debug)]
pub struct ScreenshotPortal {
    version: u32,
}

impl ScreenshotPortal {
    pub fn new() -> Self {
        Self { version: 2 }
    }
}

impl Portal for ScreenshotPortal {
    fn name(&self) -> &str { "org.freedesktop.portal.Screenshot" }
    fn version(&self) -> u32 { self.version }

    fn handle_method(&self, method: &str, _args: &[PortalArg]) -> Result<PortalResponse, PortalError> {
        match method {
            "Screenshot" => {
                let mut result = BTreeMap::new();
                result.insert(String::from("uri"),
                    PortalArg::String(String::from("file:///tmp/screenshot.png")));
                Ok(PortalResponse::Success(result))
            }
            "PickColor" => {
                let mut result = BTreeMap::new();
                result.insert(String::from("color"),
                    PortalArg::StringArray(vec![
                        String::from("1.0"), // r
                        String::from("1.0"), // g
                        String::from("1.0"), // b
                    ]));
                Ok(PortalResponse::Success(result))
            }
            _ => Err(PortalError::UnknownMethod),
        }
    }
}

/// Camera portal
#[derive(Debug)]
pub struct CameraPortal {
    version: u32,
}

impl CameraPortal {
    pub fn new() -> Self {
        Self { version: 1 }
    }
}

impl Portal for CameraPortal {
    fn name(&self) -> &str { "org.freedesktop.portal.Camera" }
    fn version(&self) -> u32 { self.version }

    fn handle_method(&self, method: &str, _args: &[PortalArg]) -> Result<PortalResponse, PortalError> {
        match method {
            "AccessCamera" => {
                // TODO: Grant camera access
                Ok(PortalResponse::Success(BTreeMap::new()))
            }
            "OpenPipeWireRemote" => {
                let mut result = BTreeMap::new();
                result.insert(String::from("fd"), PortalArg::Handle(0));
                Ok(PortalResponse::Success(result))
            }
            _ => Err(PortalError::UnknownMethod),
        }
    }
}

/// OSTree repository interface
#[derive(Debug)]
pub struct OsTreeRepo {
    pub path: String,
    pub mode: OsTreeMode,
}

/// OSTree repository mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsTreeMode {
    Archive,
    Bare,
    BareUser,
    BareUserOnly,
}

impl OsTreeRepo {
    pub fn open(path: &str) -> Result<Self, OsTreeError> {
        Ok(Self {
            path: String::from(path),
            mode: OsTreeMode::Archive,
        })
    }

    pub fn pull(&self, _remote: &str, _refs: &[&str]) -> Result<(), OsTreeError> {
        // TODO: Implement OSTree pull
        Ok(())
    }

    pub fn checkout(&self, _commit: &str, _dest: &str) -> Result<(), OsTreeError> {
        // TODO: Implement OSTree checkout with hardlinks
        Ok(())
    }

    pub fn commit(&self, _tree: &str, _subject: &str) -> Result<String, OsTreeError> {
        // TODO: Implement OSTree commit
        Ok(String::from("deadbeef"))
    }
}

/// OSTree error
#[derive(Debug)]
pub enum OsTreeError {
    NotFound,
    InvalidRepo,
    NetworkError,
    ChecksumMismatch,
    SignatureError,
}

/// Flatpak error
#[derive(Debug)]
pub enum FlatpakError {
    NotInitialized,
    AppNotFound,
    RuntimeNotFound,
    RemoteNotFound,
    AlreadyInstalled,
    DependencyError,
    PermissionDenied,
    SandboxError,
    OsTreeError(OsTreeError),
    IoError,
}

// ============================================================================
// Public API
// ============================================================================

/// Initialize Flatpak runtime
pub fn init() {
    let mut state = FLATPAK_STATE.lock();
    if state.is_none() {
        *state = Some(FlatpakRuntime::new());
    }
    crate::kprintln!("[flatpak] Flatpak compatibility layer initialized");
}

/// Add a remote repository
pub fn add_remote(remote: Remote) -> Result<(), FlatpakError> {
    let mut state = FLATPAK_STATE.lock();
    let runtime = state.as_mut().ok_or(FlatpakError::NotInitialized)?;

    // Check if already exists
    if runtime.remotes.iter().any(|r| r.name == remote.name) {
        return Ok(());
    }

    runtime.remotes.push(remote);
    Ok(())
}

/// Remove a remote repository
pub fn remove_remote(name: &str) -> Result<(), FlatpakError> {
    let mut state = FLATPAK_STATE.lock();
    let runtime = state.as_mut().ok_or(FlatpakError::NotInitialized)?;

    runtime.remotes.retain(|r| r.name != name);
    Ok(())
}

/// List configured remotes
pub fn list_remotes() -> Result<Vec<Remote>, FlatpakError> {
    let state = FLATPAK_STATE.lock();
    let runtime = state.as_ref().ok_or(FlatpakError::NotInitialized)?;

    Ok(runtime.remotes.clone())
}

/// Install an application
pub fn install(remote: &str, app_ref: Ref, _installation: InstallationType) -> Result<InstalledApp, FlatpakError> {
    let mut state = FLATPAK_STATE.lock();
    let runtime = state.as_mut().ok_or(FlatpakError::NotInitialized)?;

    // Check if remote exists
    if !runtime.remotes.iter().any(|r| r.name == remote) {
        return Err(FlatpakError::RemoteNotFound);
    }

    // Check if already installed
    let ref_str = app_ref.format_ref();
    if runtime.apps.contains_key(&ref_str) {
        return Err(FlatpakError::AlreadyInstalled);
    }

    // TODO: Actually pull from OSTree repository
    // For now, create a stub installed app
    let deploy_path = {
        let mut path = String::new();
        path.push_str("/var/lib/flatpak/app/");
        path.push_str(&app_ref.name);
        path.push_str("/");
        path.push_str(app_ref.arch.as_str());
        path.push_str("/");
        path.push_str(&app_ref.branch.0);
        path
    };

    let installed = InstalledApp {
        app_ref: app_ref.clone(),
        remote: String::from(remote),
        commit: String::from("0000000000"),
        metadata: AppMetadata::new(&app_ref.name, "org.freedesktop.Platform/x86_64/23.08", "/app/bin/app"),
        size: 0,
        install_time: 0, // TODO: Get current time
        is_current: true,
        deploy_path,
    };

    runtime.apps.insert(ref_str, installed.clone());
    Ok(installed)
}

/// Uninstall an application
pub fn uninstall(app_ref: &Ref) -> Result<(), FlatpakError> {
    let mut state = FLATPAK_STATE.lock();
    let runtime = state.as_mut().ok_or(FlatpakError::NotInitialized)?;

    let ref_str = app_ref.format_ref();
    if runtime.apps.remove(&ref_str).is_none() {
        return Err(FlatpakError::AppNotFound);
    }

    Ok(())
}

/// List installed applications
pub fn list_installed_apps() -> Result<Vec<InstalledApp>, FlatpakError> {
    let state = FLATPAK_STATE.lock();
    let runtime = state.as_ref().ok_or(FlatpakError::NotInitialized)?;

    Ok(runtime.apps.values().cloned().collect())
}

/// List installed runtimes
pub fn list_installed_runtimes() -> Result<Vec<InstalledRuntime>, FlatpakError> {
    let state = FLATPAK_STATE.lock();
    let runtime = state.as_ref().ok_or(FlatpakError::NotInitialized)?;

    Ok(runtime.runtimes.values().cloned().collect())
}

/// Run an application
pub fn run(app_ref: &Ref, args: &[&str]) -> Result<RunningInstance, FlatpakError> {
    let mut state = FLATPAK_STATE.lock();
    let runtime = state.as_mut().ok_or(FlatpakError::NotInitialized)?;

    let ref_str = app_ref.format_ref();
    let app = runtime.apps.get(&ref_str).ok_or(FlatpakError::AppNotFound)?;

    // Create sandbox configuration from permissions
    let mut sandbox = SandboxConfig::new(&app.deploy_path);

    // Apply permissions
    if app.metadata.permissions.share_network {
        sandbox.net_ns = false;
    }
    if app.metadata.permissions.share_ipc {
        sandbox.ipc_ns = false;
    }

    // Add filesystem permissions
    for fs_perm in &app.metadata.permissions.filesystem {
        sandbox.add_bind_mount(
            &fs_perm.path,
            &fs_perm.path,
            fs_perm.access == FilesystemAccess::ReadOnly
        );
    }

    // Set environment
    for (key, value) in &app.metadata.permissions.environment {
        sandbox.env.insert(key.clone(), value.clone());
    }

    // Prepare command
    let _command = {
        let mut cmd = Vec::new();
        cmd.push(app.metadata.command.clone());
        for arg in args {
            cmd.push(String::from(*arg));
        }
        cmd
    };

    // TODO: Actually spawn process in sandbox
    let instance_id = runtime.next_instance_id;
    runtime.next_instance_id += 1;

    let instance = RunningInstance {
        instance_id,
        app_ref: app_ref.clone(),
        pid: 0, // TODO: Get actual PID
        sandbox,
        start_time: 0, // TODO: Get current time
    };

    runtime.instances.insert(instance_id, instance.clone());

    Ok(instance)
}

/// Stop a running instance
pub fn stop(instance_id: u32) -> Result<(), FlatpakError> {
    let mut state = FLATPAK_STATE.lock();
    let runtime = state.as_mut().ok_or(FlatpakError::NotInitialized)?;

    if runtime.instances.remove(&instance_id).is_none() {
        return Err(FlatpakError::AppNotFound);
    }

    // TODO: Actually kill the process

    Ok(())
}

/// List running instances
pub fn list_instances() -> Result<Vec<u32>, FlatpakError> {
    let state = FLATPAK_STATE.lock();
    let runtime = state.as_ref().ok_or(FlatpakError::NotInitialized)?;

    Ok(runtime.instances.keys().copied().collect())
}

/// Handle portal request
pub fn handle_portal_request(portal: &str, method: &str, args: &[PortalArg]) -> Result<PortalResponse, PortalError> {
    let state = FLATPAK_STATE.lock();
    let runtime = state.as_ref().ok_or(PortalError::Failed)?;

    runtime.portals.handle_request(portal, method, args)
}

/// Update an application
pub fn update(app_ref: &Ref) -> Result<(), FlatpakError> {
    let state = FLATPAK_STATE.lock();
    let runtime = state.as_ref().ok_or(FlatpakError::NotInitialized)?;

    let ref_str = app_ref.format_ref();
    if !runtime.apps.contains_key(&ref_str) {
        return Err(FlatpakError::AppNotFound);
    }

    // TODO: Actually pull updates from OSTree

    Ok(())
}

/// Search for applications in remotes
pub fn search(_query: &str) -> Result<Vec<Ref>, FlatpakError> {
    let state = FLATPAK_STATE.lock();
    let _runtime = state.as_ref().ok_or(FlatpakError::NotInitialized)?;

    // TODO: Implement search via appstream data
    Ok(Vec::new())
}

/// Get application info
pub fn get_app_info(app_ref: &Ref) -> Result<InstalledApp, FlatpakError> {
    let state = FLATPAK_STATE.lock();
    let runtime = state.as_ref().ok_or(FlatpakError::NotInitialized)?;

    let ref_str = app_ref.format_ref();
    runtime.apps.get(&ref_str).cloned().ok_or(FlatpakError::AppNotFound)
}

/// Parse metadata file content
pub fn parse_metadata(content: &str) -> Result<AppMetadata, FlatpakError> {
    // Simple INI-style parser
    let mut metadata = AppMetadata::new("unknown", "org.freedesktop.Platform//23.08", "/app/bin/app");

    let mut current_section = String::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            current_section = String::from(&line[1..line.len()-1]);
            continue;
        }

        if let Some(eq_pos) = line.find('=') {
            let key = line[..eq_pos].trim();
            let value = line[eq_pos+1..].trim();

            match current_section.as_str() {
                "Application" => {
                    match key {
                        "name" => metadata.name = String::from(value),
                        "runtime" => metadata.runtime = String::from(value),
                        "sdk" => metadata.sdk = Some(String::from(value)),
                        "command" => metadata.command = String::from(value),
                        _ => {}
                    }
                }
                "Context" => {
                    match key {
                        "shared" => {
                            for item in value.split(';') {
                                match item.trim() {
                                    "network" => metadata.permissions.share_network = true,
                                    "ipc" => metadata.permissions.share_ipc = true,
                                    _ => {}
                                }
                            }
                        }
                        "sockets" => {
                            for item in value.split(';') {
                                match item.trim() {
                                    "x11" => metadata.permissions.sockets.push(SocketPermission::X11),
                                    "wayland" => metadata.permissions.sockets.push(SocketPermission::Wayland),
                                    "pulseaudio" => metadata.permissions.sockets.push(SocketPermission::PulseAudio),
                                    "system-bus" => metadata.permissions.sockets.push(SocketPermission::System),
                                    "session-bus" => metadata.permissions.sockets.push(SocketPermission::Session),
                                    _ => {}
                                }
                            }
                        }
                        "devices" => {
                            for item in value.split(';') {
                                match item.trim() {
                                    "dri" => metadata.permissions.devices.push(DevicePermission::Dri),
                                    "kvm" => metadata.permissions.devices.push(DevicePermission::Kvm),
                                    "shm" => metadata.permissions.devices.push(DevicePermission::Shm),
                                    "all" => metadata.permissions.devices.push(DevicePermission::All),
                                    _ => {}
                                }
                            }
                        }
                        "filesystems" => {
                            for item in value.split(';') {
                                let item = item.trim();
                                if item.is_empty() { continue; }

                                let (path, access) = if item.ends_with(":ro") {
                                    (&item[..item.len()-3], FilesystemAccess::ReadOnly)
                                } else if item.ends_with(":rw") {
                                    (&item[..item.len()-3], FilesystemAccess::ReadWrite)
                                } else if item.ends_with(":create") {
                                    (&item[..item.len()-7], FilesystemAccess::Create)
                                } else {
                                    (item, FilesystemAccess::ReadWrite)
                                };

                                metadata.permissions.filesystem.push(FilesystemPermission {
                                    path: String::from(path),
                                    access,
                                });
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    Ok(metadata)
}

impl Clone for RunningInstance {
    fn clone(&self) -> Self {
        Self {
            instance_id: self.instance_id,
            app_ref: self.app_ref.clone(),
            pid: self.pid,
            sandbox: self.sandbox.clone(),
            start_time: self.start_time,
        }
    }
}
