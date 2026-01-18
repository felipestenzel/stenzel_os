//! Network Shares Application
//!
//! SMB/NFS browser for accessing network file shares.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;

use crate::gui::widgets::{Widget, WidgetId, WidgetEvent, Bounds, MouseButton};
use crate::gui::surface::Surface;
use crate::drivers::framebuffer::Color;

/// Network share protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShareProtocol {
    Smb,
    Smb2,
    Smb3,
    Nfs3,
    Nfs4,
    Afp,
    WebDav,
    Ftp,
    Sftp,
}

impl ShareProtocol {
    pub fn name(&self) -> &'static str {
        match self {
            ShareProtocol::Smb => "SMB",
            ShareProtocol::Smb2 => "SMB 2.0",
            ShareProtocol::Smb3 => "SMB 3.0",
            ShareProtocol::Nfs3 => "NFS v3",
            ShareProtocol::Nfs4 => "NFS v4",
            ShareProtocol::Afp => "AFP",
            ShareProtocol::WebDav => "WebDAV",
            ShareProtocol::Ftp => "FTP",
            ShareProtocol::Sftp => "SFTP",
        }
    }

    pub fn default_port(&self) -> u16 {
        match self {
            ShareProtocol::Smb | ShareProtocol::Smb2 | ShareProtocol::Smb3 => 445,
            ShareProtocol::Nfs3 | ShareProtocol::Nfs4 => 2049,
            ShareProtocol::Afp => 548,
            ShareProtocol::WebDav => 80,
            ShareProtocol::Ftp => 21,
            ShareProtocol::Sftp => 22,
        }
    }

    pub fn uri_scheme(&self) -> &'static str {
        match self {
            ShareProtocol::Smb | ShareProtocol::Smb2 | ShareProtocol::Smb3 => "smb",
            ShareProtocol::Nfs3 | ShareProtocol::Nfs4 => "nfs",
            ShareProtocol::Afp => "afp",
            ShareProtocol::WebDav => "webdav",
            ShareProtocol::Ftp => "ftp",
            ShareProtocol::Sftp => "sftp",
        }
    }
}

/// Authentication method
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMethod {
    Anonymous,
    UserPassword,
    Kerberos,
    NtlmV2,
    PublicKey,
}

impl AuthMethod {
    pub fn name(&self) -> &'static str {
        match self {
            AuthMethod::Anonymous => "Anonymous",
            AuthMethod::UserPassword => "Username/Password",
            AuthMethod::Kerberos => "Kerberos",
            AuthMethod::NtlmV2 => "NTLMv2",
            AuthMethod::PublicKey => "Public Key",
        }
    }
}

/// Network share credentials
#[derive(Debug, Clone)]
pub struct ShareCredentials {
    pub auth_method: AuthMethod,
    pub username: Option<String>,
    pub password: Option<String>,
    pub domain: Option<String>,
    pub key_path: Option<String>,
    pub save_password: bool,
}

impl Default for ShareCredentials {
    fn default() -> Self {
        Self {
            auth_method: AuthMethod::Anonymous,
            username: None,
            password: None,
            domain: None,
            key_path: None,
            save_password: false,
        }
    }
}

impl ShareCredentials {
    pub fn with_user_password(username: &str, password: &str) -> Self {
        Self {
            auth_method: AuthMethod::UserPassword,
            username: Some(username.to_string()),
            password: Some(password.to_string()),
            domain: None,
            key_path: None,
            save_password: false,
        }
    }
}

/// Connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Authenticating,
    Error,
    Timeout,
}

impl ConnectionState {
    pub fn name(&self) -> &'static str {
        match self {
            ConnectionState::Disconnected => "Disconnected",
            ConnectionState::Connecting => "Connecting...",
            ConnectionState::Connected => "Connected",
            ConnectionState::Authenticating => "Authenticating...",
            ConnectionState::Error => "Error",
            ConnectionState::Timeout => "Timeout",
        }
    }

    pub fn is_connected(&self) -> bool {
        matches!(self, ConnectionState::Connected)
    }
}

/// Network server
#[derive(Debug, Clone)]
pub struct NetworkServer {
    pub id: u64,
    pub hostname: String,
    pub ip_address: Option<String>,
    pub port: u16,
    pub protocol: ShareProtocol,
    pub workgroup: Option<String>,
    pub shares: Vec<NetworkShare>,
    pub connection_state: ConnectionState,
    pub last_seen: u64,
    pub is_favorite: bool,
}

impl NetworkServer {
    pub fn new(id: u64, hostname: &str, protocol: ShareProtocol) -> Self {
        Self {
            id,
            hostname: hostname.to_string(),
            ip_address: None,
            port: protocol.default_port(),
            protocol,
            workgroup: None,
            shares: Vec::new(),
            connection_state: ConnectionState::Disconnected,
            last_seen: 0,
            is_favorite: false,
        }
    }

    pub fn display_name(&self) -> String {
        if let Some(ref ip) = self.ip_address {
            format!("{} ({})", self.hostname, ip)
        } else {
            self.hostname.clone()
        }
    }

    pub fn uri(&self) -> String {
        format!("{}://{}", self.protocol.uri_scheme(), self.hostname)
    }
}

/// Network share
#[derive(Debug, Clone)]
pub struct NetworkShare {
    pub name: String,
    pub path: String,
    pub share_type: ShareType,
    pub is_hidden: bool,
    pub comment: Option<String>,
    pub permissions: SharePermissions,
    pub size_total: Option<u64>,
    pub size_free: Option<u64>,
}

impl NetworkShare {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            path: format!("/{}", name),
            share_type: ShareType::Disk,
            is_hidden: name.ends_with('$'),
            comment: None,
            permissions: SharePermissions::default(),
            size_total: None,
            size_free: None,
        }
    }

    pub fn format_size(&self) -> Option<String> {
        self.size_total.map(|total| {
            let free = self.size_free.unwrap_or(0);
            let used = total - free;
            format!("{} GB / {} GB", used / (1024 * 1024 * 1024), total / (1024 * 1024 * 1024))
        })
    }
}

/// Share type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShareType {
    Disk,
    Printer,
    Device,
    Ipc,
    Admin,
}

impl ShareType {
    pub fn name(&self) -> &'static str {
        match self {
            ShareType::Disk => "Disk",
            ShareType::Printer => "Printer",
            ShareType::Device => "Device",
            ShareType::Ipc => "IPC",
            ShareType::Admin => "Admin",
        }
    }

    pub fn icon(&self) -> char {
        match self {
            ShareType::Disk => '📁',
            ShareType::Printer => '🖨',
            ShareType::Device => '🔌',
            ShareType::Ipc => '🔗',
            ShareType::Admin => '⚙',
        }
    }
}

/// Share permissions
#[derive(Debug, Clone, Copy, Default)]
pub struct SharePermissions {
    pub can_read: bool,
    pub can_write: bool,
    pub can_execute: bool,
    pub can_delete: bool,
}

/// Remote file entry
#[derive(Debug, Clone)]
pub struct RemoteFile {
    pub name: String,
    pub path: String,
    pub is_directory: bool,
    pub size: u64,
    pub modified: u64,
    pub created: u64,
    pub permissions: u32,
    pub is_hidden: bool,
    pub is_symlink: bool,
}

impl RemoteFile {
    pub fn format_size(&self) -> String {
        if self.is_directory {
            String::from("--")
        } else if self.size < 1024 {
            format!("{} B", self.size)
        } else if self.size < 1024 * 1024 {
            format!("{} KB", self.size / 1024)
        } else if self.size < 1024 * 1024 * 1024 {
            format!("{:.1} MB", self.size as f32 / (1024.0 * 1024.0))
        } else {
            format!("{:.1} GB", self.size as f32 / (1024.0 * 1024.0 * 1024.0))
        }
    }

    pub fn icon(&self) -> char {
        if self.is_directory {
            '📁'
        } else if self.is_symlink {
            '🔗'
        } else {
            '📄'
        }
    }
}

/// Mount point
#[derive(Debug, Clone)]
pub struct MountPoint {
    pub id: u64,
    pub server_id: u64,
    pub share_name: String,
    pub local_path: String,
    pub is_mounted: bool,
    pub auto_mount: bool,
    pub credentials: ShareCredentials,
}

impl MountPoint {
    pub fn new(id: u64, server_id: u64, share_name: &str, local_path: &str) -> Self {
        Self {
            id,
            server_id,
            share_name: share_name.to_string(),
            local_path: local_path.to_string(),
            is_mounted: false,
            auto_mount: false,
            credentials: ShareCredentials::default(),
        }
    }
}

/// Saved connection/bookmark
#[derive(Debug, Clone)]
pub struct SavedConnection {
    pub id: u64,
    pub name: String,
    pub uri: String,
    pub protocol: ShareProtocol,
    pub hostname: String,
    pub port: u16,
    pub share_path: Option<String>,
    pub credentials: ShareCredentials,
    pub auto_connect: bool,
    pub last_used: u64,
}

impl SavedConnection {
    pub fn new(id: u64, name: &str, uri: &str) -> Self {
        Self {
            id,
            name: name.to_string(),
            uri: uri.to_string(),
            protocol: ShareProtocol::Smb,
            hostname: String::new(),
            port: 445,
            share_path: None,
            credentials: ShareCredentials::default(),
            auto_connect: false,
            last_used: 0,
        }
    }
}

/// View mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Browse,
    Servers,
    Saved,
    MountPoints,
}

/// Error type
#[derive(Debug, Clone)]
pub enum ShareError {
    ConnectionFailed(String),
    AuthenticationFailed,
    PermissionDenied,
    ShareNotFound,
    NetworkError(String),
    Timeout,
    MountFailed(String),
    ProtocolError(String),
}

impl ShareError {
    pub fn message(&self) -> String {
        match self {
            ShareError::ConnectionFailed(msg) => format!("Connection failed: {}", msg),
            ShareError::AuthenticationFailed => String::from("Authentication failed"),
            ShareError::PermissionDenied => String::from("Permission denied"),
            ShareError::ShareNotFound => String::from("Share not found"),
            ShareError::NetworkError(msg) => format!("Network error: {}", msg),
            ShareError::Timeout => String::from("Connection timeout"),
            ShareError::MountFailed(msg) => format!("Mount failed: {}", msg),
            ShareError::ProtocolError(msg) => format!("Protocol error: {}", msg),
        }
    }
}

// Helper functions for rendering
fn draw_char_at(surface: &mut Surface, x: usize, y: usize, c: char, color: Color) {
    use crate::drivers::font::DEFAULT_FONT;
    if let Some(glyph) = DEFAULT_FONT.get_glyph(c) {
        for row in 0..DEFAULT_FONT.height {
            let byte = glyph[row];
            for col in 0..DEFAULT_FONT.width {
                if (byte >> (7 - col)) & 1 != 0 {
                    surface.set_pixel(x + col, y + row, color);
                }
            }
        }
    }
}

fn draw_char(surface: &mut Surface, x: isize, y: isize, c: char, color: Color) {
    if x >= 0 && y >= 0 {
        draw_char_at(surface, x as usize, y as usize, c, color);
    }
}

fn draw_string(surface: &mut Surface, x: isize, y: isize, s: &str, color: Color) {
    if x < 0 || y < 0 { return; }
    let mut px = x as usize;
    for c in s.chars() {
        draw_char_at(surface, px, y as usize, c, color);
        px += 8;
    }
}

/// Network Shares browser widget
pub struct NetworkSharesBrowser {
    id: WidgetId,
    bounds: Bounds,
    enabled: bool,
    visible: bool,

    // Data
    servers: Vec<NetworkServer>,
    saved_connections: Vec<SavedConnection>,
    mount_points: Vec<MountPoint>,
    next_server_id: u64,
    next_connection_id: u64,
    next_mount_id: u64,

    // Navigation state
    current_server_id: Option<u64>,
    current_share: Option<String>,
    current_path: String,
    files: Vec<RemoteFile>,

    // View state
    view_mode: ViewMode,
    selected_index: Option<usize>,
    scroll_offset: usize,
    hovered_index: Option<usize>,

    // UI state
    sidebar_width: usize,
    show_hidden: bool,
    show_connect_dialog: bool,
    connect_uri: String,
    error_message: Option<String>,
    is_loading: bool,
}

impl NetworkSharesBrowser {
    pub fn new(id: WidgetId) -> Self {
        let mut browser = Self {
            id,
            bounds: Bounds { x: 0, y: 0, width: 900, height: 600 },
            enabled: true,
            visible: true,
            servers: Vec::new(),
            saved_connections: Vec::new(),
            mount_points: Vec::new(),
            next_server_id: 1,
            next_connection_id: 1,
            next_mount_id: 1,
            current_server_id: None,
            current_share: None,
            current_path: String::from("/"),
            files: Vec::new(),
            view_mode: ViewMode::Servers,
            selected_index: None,
            scroll_offset: 0,
            hovered_index: None,
            sidebar_width: 220,
            show_hidden: false,
            show_connect_dialog: false,
            connect_uri: String::new(),
            error_message: None,
            is_loading: false,
        };

        browser.add_sample_data();
        browser
    }

    fn add_sample_data(&mut self) {
        // Add some sample servers discovered on the network
        let mut server1 = NetworkServer::new(self.next_server_id, "nas-server", ShareProtocol::Smb);
        server1.ip_address = Some(String::from("192.168.1.100"));
        server1.workgroup = Some(String::from("WORKGROUP"));
        server1.shares = vec![
            NetworkShare {
                name: String::from("public"),
                path: String::from("/public"),
                share_type: ShareType::Disk,
                is_hidden: false,
                comment: Some(String::from("Public shared folder")),
                permissions: SharePermissions { can_read: true, can_write: true, can_execute: true, can_delete: false },
                size_total: Some(2_000_000_000_000),
                size_free: Some(500_000_000_000),
            },
            NetworkShare {
                name: String::from("media"),
                path: String::from("/media"),
                share_type: ShareType::Disk,
                is_hidden: false,
                comment: Some(String::from("Media files")),
                permissions: SharePermissions { can_read: true, can_write: false, can_execute: false, can_delete: false },
                size_total: Some(4_000_000_000_000),
                size_free: Some(1_000_000_000_000),
            },
            NetworkShare {
                name: String::from("backup"),
                path: String::from("/backup"),
                share_type: ShareType::Disk,
                is_hidden: false,
                comment: Some(String::from("Backup storage")),
                permissions: SharePermissions { can_read: true, can_write: true, can_execute: false, can_delete: true },
                size_total: Some(8_000_000_000_000),
                size_free: Some(3_000_000_000_000),
            },
        ];
        server1.connection_state = ConnectionState::Disconnected;
        self.servers.push(server1);
        self.next_server_id += 1;

        let mut server2 = NetworkServer::new(self.next_server_id, "file-server", ShareProtocol::Smb2);
        server2.ip_address = Some(String::from("192.168.1.50"));
        server2.workgroup = Some(String::from("WORKGROUP"));
        server2.shares = vec![
            NetworkShare::new("documents"),
            NetworkShare::new("projects"),
        ];
        server2.is_favorite = true;
        self.servers.push(server2);
        self.next_server_id += 1;

        let mut server3 = NetworkServer::new(self.next_server_id, "linux-server", ShareProtocol::Nfs4);
        server3.ip_address = Some(String::from("192.168.1.200"));
        server3.shares = vec![
            NetworkShare::new("home"),
            NetworkShare::new("var"),
        ];
        self.servers.push(server3);
        self.next_server_id += 1;

        // Add saved connections
        let mut saved1 = SavedConnection::new(self.next_connection_id, "Work NAS", "smb://nas.work.com/shared");
        saved1.hostname = String::from("nas.work.com");
        saved1.share_path = Some(String::from("/shared"));
        saved1.credentials = ShareCredentials::with_user_password("user", "");
        self.saved_connections.push(saved1);
        self.next_connection_id += 1;

        let saved2 = SavedConnection::new(self.next_connection_id, "Home Server", "smb://192.168.1.100/media");
        self.saved_connections.push(saved2);
        self.next_connection_id += 1;

        // Add mount points
        let mount1 = MountPoint::new(self.next_mount_id, 1, "public", "/mnt/nas/public");
        self.mount_points.push(mount1);
        self.next_mount_id += 1;
    }

    // Server management
    pub fn discover_servers(&mut self) {
        self.is_loading = true;
        // In a real implementation, this would scan the network
        // For now, we just use the sample data
        self.is_loading = false;
    }

    pub fn connect_to_server(&mut self, server_id: u64) {
        if let Some(server) = self.servers.iter_mut().find(|s| s.id == server_id) {
            server.connection_state = ConnectionState::Connecting;
            // Simulate connection
            server.connection_state = ConnectionState::Connected;
            self.current_server_id = Some(server_id);
            self.view_mode = ViewMode::Browse;
        }
    }

    pub fn disconnect_from_server(&mut self, server_id: u64) {
        if let Some(server) = self.servers.iter_mut().find(|s| s.id == server_id) {
            server.connection_state = ConnectionState::Disconnected;
        }
        if self.current_server_id == Some(server_id) {
            self.current_server_id = None;
            self.current_share = None;
            self.files.clear();
        }
    }

    pub fn toggle_favorite(&mut self, server_id: u64) {
        if let Some(server) = self.servers.iter_mut().find(|s| s.id == server_id) {
            server.is_favorite = !server.is_favorite;
        }
    }

    // Share navigation
    pub fn open_share(&mut self, share_name: &str) {
        self.current_share = Some(share_name.to_string());
        self.current_path = String::from("/");
        self.load_directory();
    }

    pub fn navigate_to(&mut self, path: &str) {
        self.current_path = path.to_string();
        self.load_directory();
    }

    pub fn go_up(&mut self) {
        if self.current_path == "/" {
            self.current_share = None;
        } else {
            // Go to parent directory
            if let Some(pos) = self.current_path.rfind('/') {
                if pos == 0 {
                    self.current_path = String::from("/");
                } else {
                    self.current_path = self.current_path[..pos].to_string();
                }
            }
        }
        self.load_directory();
    }

    fn load_directory(&mut self) {
        self.is_loading = true;
        self.files.clear();
        self.selected_index = None;
        self.scroll_offset = 0;

        // Simulate loading directory contents
        if self.current_share.is_some() {
            self.files = vec![
                RemoteFile {
                    name: String::from("Documents"),
                    path: format!("{}/Documents", self.current_path),
                    is_directory: true,
                    size: 0,
                    modified: 0,
                    created: 0,
                    permissions: 0o755,
                    is_hidden: false,
                    is_symlink: false,
                },
                RemoteFile {
                    name: String::from("Photos"),
                    path: format!("{}/Photos", self.current_path),
                    is_directory: true,
                    size: 0,
                    modified: 0,
                    created: 0,
                    permissions: 0o755,
                    is_hidden: false,
                    is_symlink: false,
                },
                RemoteFile {
                    name: String::from("readme.txt"),
                    path: format!("{}/readme.txt", self.current_path),
                    is_directory: false,
                    size: 4096,
                    modified: 0,
                    created: 0,
                    permissions: 0o644,
                    is_hidden: false,
                    is_symlink: false,
                },
                RemoteFile {
                    name: String::from("report.pdf"),
                    path: format!("{}/report.pdf", self.current_path),
                    is_directory: false,
                    size: 2_500_000,
                    modified: 0,
                    created: 0,
                    permissions: 0o644,
                    is_hidden: false,
                    is_symlink: false,
                },
                RemoteFile {
                    name: String::from(".hidden"),
                    path: format!("{}/.hidden", self.current_path),
                    is_directory: false,
                    size: 256,
                    modified: 0,
                    created: 0,
                    permissions: 0o600,
                    is_hidden: true,
                    is_symlink: false,
                },
            ];
        }

        self.is_loading = false;
    }

    // Mount operations
    pub fn mount_share(&mut self, server_id: u64, share_name: &str, local_path: &str) {
        let mount = MountPoint::new(self.next_mount_id, server_id, share_name, local_path);
        self.mount_points.push(mount);
        self.next_mount_id += 1;

        // Mark as mounted
        if let Some(mp) = self.mount_points.last_mut() {
            mp.is_mounted = true;
        }
    }

    pub fn unmount(&mut self, mount_id: u64) {
        if let Some(mp) = self.mount_points.iter_mut().find(|m| m.id == mount_id) {
            mp.is_mounted = false;
        }
    }

    // Saved connections
    pub fn save_connection(&mut self, name: &str, uri: &str) {
        let saved = SavedConnection::new(self.next_connection_id, name, uri);
        self.saved_connections.push(saved);
        self.next_connection_id += 1;
    }

    pub fn delete_saved_connection(&mut self, connection_id: u64) {
        self.saved_connections.retain(|c| c.id != connection_id);
    }

    // UI helpers
    fn get_visible_count(&self) -> usize {
        let content_height = self.bounds.height.saturating_sub(100);
        content_height / 30
    }

    fn item_at_point(&self, x: isize, y: isize) -> Option<usize> {
        let content_x = self.bounds.x + self.sidebar_width as isize + 10;
        let content_y = self.bounds.y + 70;
        let content_width = self.bounds.width - self.sidebar_width - 20;

        if x < content_x || x >= content_x + content_width as isize {
            return None;
        }

        if y < content_y || y >= self.bounds.y + self.bounds.height as isize - 30 {
            return None;
        }

        let rel_y = y - content_y;
        let index = (rel_y / 30) as usize + self.scroll_offset;

        let count = match self.view_mode {
            ViewMode::Servers => self.servers.len(),
            ViewMode::Saved => self.saved_connections.len(),
            ViewMode::MountPoints => self.mount_points.len(),
            ViewMode::Browse => {
                if self.current_share.is_some() {
                    self.files.iter().filter(|f| self.show_hidden || !f.is_hidden).count()
                } else if let Some(server_id) = self.current_server_id {
                    self.servers.iter()
                        .find(|s| s.id == server_id)
                        .map(|s| s.shares.len())
                        .unwrap_or(0)
                } else {
                    0
                }
            }
        };

        if index < count {
            Some(index)
        } else {
            None
        }
    }
}

impl Widget for NetworkSharesBrowser {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn bounds(&self) -> Bounds {
        self.bounds
    }

    fn set_position(&mut self, x: isize, y: isize) {
        self.bounds.x = x;
        self.bounds.y = y;
    }

    fn set_size(&mut self, width: usize, height: usize) {
        self.bounds.width = width;
        self.bounds.height = height;
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn handle_event(&mut self, event: &WidgetEvent) -> bool {
        match event {
            WidgetEvent::MouseDown { x, y, button } => {
                if *button != MouseButton::Left {
                    return false;
                }

                // Check sidebar items
                if *x >= self.bounds.x && *x < self.bounds.x + self.sidebar_width as isize {
                    let rel_y = *y - self.bounds.y - 60;
                    if rel_y >= 0 {
                        let index = (rel_y / 35) as usize;
                        match index {
                            0 => { self.view_mode = ViewMode::Servers; return true; }
                            1 => { self.view_mode = ViewMode::Saved; return true; }
                            2 => { self.view_mode = ViewMode::MountPoints; return true; }
                            _ => {}
                        }
                    }
                }

                // Check content area
                if let Some(idx) = self.item_at_point(*x, *y) {
                    match self.view_mode {
                        ViewMode::Servers => {
                            if idx < self.servers.len() {
                                let server_id = self.servers[idx].id;
                                if self.servers[idx].connection_state.is_connected() {
                                    self.current_server_id = Some(server_id);
                                    self.view_mode = ViewMode::Browse;
                                } else {
                                    self.connect_to_server(server_id);
                                }
                                return true;
                            }
                        }
                        ViewMode::Browse => {
                            if self.current_share.is_some() {
                                let visible_files: Vec<_> = self.files.iter()
                                    .filter(|f| self.show_hidden || !f.is_hidden)
                                    .collect();
                                if idx < visible_files.len() {
                                    if visible_files[idx].is_directory {
                                        let path = visible_files[idx].path.clone();
                                        self.navigate_to(&path);
                                    }
                                    self.selected_index = Some(idx);
                                    return true;
                                }
                            } else if let Some(server_id) = self.current_server_id {
                                if let Some(server) = self.servers.iter().find(|s| s.id == server_id) {
                                    if idx < server.shares.len() {
                                        let share_name = server.shares[idx].name.clone();
                                        self.open_share(&share_name);
                                        return true;
                                    }
                                }
                            }
                        }
                        ViewMode::Saved => {
                            if idx < self.saved_connections.len() {
                                self.selected_index = Some(idx);
                                return true;
                            }
                        }
                        ViewMode::MountPoints => {
                            if idx < self.mount_points.len() {
                                self.selected_index = Some(idx);
                                return true;
                            }
                        }
                    }
                }

                // Toolbar buttons
                if *y >= self.bounds.y + 15 && *y < self.bounds.y + 45 {
                    let btn_x = self.bounds.x + self.sidebar_width as isize + 10;
                    if *x >= btn_x && *x < btn_x + 80 {
                        // Refresh button
                        self.discover_servers();
                        return true;
                    }
                    if *x >= btn_x + 90 && *x < btn_x + 170 {
                        // Connect button
                        self.show_connect_dialog = true;
                        return true;
                    }
                }

                false
            }

            WidgetEvent::MouseMove { x, y } => {
                self.hovered_index = self.item_at_point(*x, *y);
                true
            }

            WidgetEvent::Scroll { delta_y, .. } => {
                let count = match self.view_mode {
                    ViewMode::Servers => self.servers.len(),
                    ViewMode::Saved => self.saved_connections.len(),
                    ViewMode::MountPoints => self.mount_points.len(),
                    ViewMode::Browse => {
                        if self.current_share.is_some() {
                            self.files.len()
                        } else {
                            self.servers.iter()
                                .find(|s| Some(s.id) == self.current_server_id)
                                .map(|s| s.shares.len())
                                .unwrap_or(0)
                        }
                    }
                };
                let visible = self.get_visible_count();
                let max_scroll = count.saturating_sub(visible);

                if *delta_y < 0 && self.scroll_offset > 0 {
                    self.scroll_offset = self.scroll_offset.saturating_sub(1);
                } else if *delta_y > 0 && self.scroll_offset < max_scroll {
                    self.scroll_offset += 1;
                }
                true
            }

            WidgetEvent::KeyDown { key, .. } => {
                match *key {
                    0x1B => { // Escape - go back
                        if self.current_share.is_some() {
                            self.go_up();
                        } else if self.current_server_id.is_some() {
                            self.current_server_id = None;
                            self.view_mode = ViewMode::Servers;
                        }
                        true
                    }
                    0x1C => { // Enter - open selected
                        if let Some(idx) = self.selected_index {
                            match self.view_mode {
                                ViewMode::Servers if idx < self.servers.len() => {
                                    let server_id = self.servers[idx].id;
                                    self.connect_to_server(server_id);
                                }
                                ViewMode::Browse => {
                                    if self.current_share.is_some() {
                                        let visible_files: Vec<_> = self.files.iter()
                                            .filter(|f| self.show_hidden || !f.is_hidden)
                                            .collect();
                                        if idx < visible_files.len() && visible_files[idx].is_directory {
                                            let path = visible_files[idx].path.clone();
                                            self.navigate_to(&path);
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                        true
                    }
                    0x2E => { // H - toggle hidden files
                        self.show_hidden = !self.show_hidden;
                        true
                    }
                    _ => false,
                }
            }

            _ => false,
        }
    }

    fn render(&self, surface: &mut Surface) {
        let bg = Color::new(30, 30, 35);
        let sidebar_bg = Color::new(25, 25, 30);
        let text_color = Color::new(230, 230, 230);
        let dim_text = Color::new(150, 150, 155);
        let accent_color = Color::new(100, 180, 255);
        let selected_bg = Color::new(60, 60, 70);
        let hover_bg = Color::new(45, 45, 50);
        let border_color = Color::new(60, 60, 65);
        let connected_color = Color::new(80, 200, 80);
        let disconnected_color = Color::new(150, 150, 150);

        // Background
        for y in 0..self.bounds.height {
            for x in 0..self.bounds.width {
                surface.set_pixel(
                    (self.bounds.x as usize) + x,
                    (self.bounds.y as usize) + y,
                    bg
                );
            }
        }

        // Sidebar
        for y in 0..self.bounds.height {
            for x in 0..self.sidebar_width {
                surface.set_pixel(
                    (self.bounds.x as usize) + x,
                    (self.bounds.y as usize) + y,
                    sidebar_bg
                );
            }
        }

        // Sidebar title
        draw_string(surface, self.bounds.x + 15, self.bounds.y + 20, "Network", accent_color);

        // Sidebar items
        let items = [
            ("Servers", ViewMode::Servers),
            ("Saved", ViewMode::Saved),
            ("Mounts", ViewMode::MountPoints),
        ];

        let mut sidebar_y = self.bounds.y + 60;
        for (name, mode) in items.iter() {
            let is_selected = self.view_mode == *mode;
            if is_selected {
                for y in 0..30 {
                    for x in 0..self.sidebar_width {
                        surface.set_pixel(
                            (self.bounds.x as usize) + x,
                            (sidebar_y as usize) + y,
                            selected_bg
                        );
                    }
                }
            }
            draw_string(surface, self.bounds.x + 15, sidebar_y + 8, name,
                if is_selected { accent_color } else { text_color });
            sidebar_y += 35;
        }

        // Favorites section
        sidebar_y += 20;
        draw_string(surface, self.bounds.x + 15, sidebar_y, "FAVORITES", dim_text);
        sidebar_y += 25;

        for server in self.servers.iter().filter(|s| s.is_favorite) {
            draw_string(surface, self.bounds.x + 20, sidebar_y, &server.hostname, text_color);
            sidebar_y += 25;
        }

        // Sidebar border
        for y in 0..self.bounds.height {
            surface.set_pixel(
                (self.bounds.x as usize) + self.sidebar_width,
                (self.bounds.y as usize) + y,
                border_color
            );
        }

        // Content area
        let content_x = self.bounds.x + self.sidebar_width as isize + 10;
        let content_y = self.bounds.y + 60;

        // Toolbar
        draw_string(surface, content_x, self.bounds.y + 20, "[Refresh]", dim_text);
        draw_string(surface, content_x + 90, self.bounds.y + 20, "[Connect]", dim_text);

        // Path/title bar
        let title = match self.view_mode {
            ViewMode::Servers => String::from("Network Servers"),
            ViewMode::Saved => String::from("Saved Connections"),
            ViewMode::MountPoints => String::from("Mount Points"),
            ViewMode::Browse => {
                if let Some(ref share) = self.current_share {
                    if let Some(server_id) = self.current_server_id {
                        if let Some(server) = self.servers.iter().find(|s| s.id == server_id) {
                            format!("smb://{}/{}{}", server.hostname, share, self.current_path)
                        } else {
                            share.clone()
                        }
                    } else {
                        share.clone()
                    }
                } else if let Some(server_id) = self.current_server_id {
                    self.servers.iter()
                        .find(|s| s.id == server_id)
                        .map(|s| format!("Shares on {}", s.hostname))
                        .unwrap_or_default()
                } else {
                    String::from("Browse")
                }
            }
        };
        draw_string(surface, content_x, self.bounds.y + 45, &title, accent_color);

        // Content
        let item_height = 30isize;
        let visible_count = self.get_visible_count();
        let mut item_y = content_y + 10;

        match self.view_mode {
            ViewMode::Servers => {
                for (i, server) in self.servers.iter().skip(self.scroll_offset).take(visible_count).enumerate() {
                    let is_hovered = self.hovered_index == Some(i + self.scroll_offset);
                    let is_selected = self.selected_index == Some(i + self.scroll_offset);

                    if is_selected || is_hovered {
                        for y in 0..item_height as usize - 2 {
                            for x in 0..(self.bounds.width - self.sidebar_width - 20) {
                                surface.set_pixel(
                                    (content_x as usize) + x,
                                    (item_y as usize) + y,
                                    if is_selected { selected_bg } else { hover_bg }
                                );
                            }
                        }
                    }

                    // Status indicator
                    let status_color = if server.connection_state.is_connected() {
                        connected_color
                    } else {
                        disconnected_color
                    };
                    draw_char(surface, content_x + 5, item_y + 5, '●', status_color);

                    // Favorite star
                    if server.is_favorite {
                        draw_char(surface, content_x + 20, item_y + 5, '★', Color::new(255, 200, 50));
                    }

                    // Server name
                    let display = server.display_name();
                    draw_string(surface, content_x + 35, item_y + 5, &display, text_color);

                    // Protocol
                    draw_string(surface, content_x + 300, item_y + 5, server.protocol.name(), dim_text);

                    // Share count
                    let share_count = format!("{} shares", server.shares.len());
                    draw_string(surface, content_x + 400, item_y + 5, &share_count, dim_text);

                    item_y += item_height;
                }
            }

            ViewMode::Browse => {
                if let Some(ref _share) = self.current_share {
                    // Show files
                    let visible_files: Vec<_> = self.files.iter()
                        .filter(|f| self.show_hidden || !f.is_hidden)
                        .collect();

                    for (i, file) in visible_files.iter().skip(self.scroll_offset).take(visible_count).enumerate() {
                        let is_hovered = self.hovered_index == Some(i + self.scroll_offset);
                        let is_selected = self.selected_index == Some(i + self.scroll_offset);

                        if is_selected || is_hovered {
                            for y in 0..item_height as usize - 2 {
                                for x in 0..(self.bounds.width - self.sidebar_width - 20) {
                                    surface.set_pixel(
                                        (content_x as usize) + x,
                                        (item_y as usize) + y,
                                        if is_selected { selected_bg } else { hover_bg }
                                    );
                                }
                            }
                        }

                        // Icon
                        let icon: String = file.icon().to_string();
                        draw_string(surface, content_x + 5, item_y + 5, &icon, text_color);

                        // Name
                        let name_color = if file.is_directory { accent_color } else { text_color };
                        draw_string(surface, content_x + 25, item_y + 5, &file.name, name_color);

                        // Size
                        draw_string(surface, content_x + 300, item_y + 5, &file.format_size(), dim_text);

                        item_y += item_height;
                    }
                } else if let Some(server_id) = self.current_server_id {
                    // Show shares
                    if let Some(server) = self.servers.iter().find(|s| s.id == server_id) {
                        for (i, share) in server.shares.iter().skip(self.scroll_offset).take(visible_count).enumerate() {
                            let is_hovered = self.hovered_index == Some(i + self.scroll_offset);
                            let is_selected = self.selected_index == Some(i + self.scroll_offset);

                            if is_selected || is_hovered {
                                for y in 0..item_height as usize - 2 {
                                    for x in 0..(self.bounds.width - self.sidebar_width - 20) {
                                        surface.set_pixel(
                                            (content_x as usize) + x,
                                            (item_y as usize) + y,
                                            if is_selected { selected_bg } else { hover_bg }
                                        );
                                    }
                                }
                            }

                            // Icon
                            let icon: String = share.share_type.icon().to_string();
                            draw_string(surface, content_x + 5, item_y + 5, &icon, text_color);

                            // Name
                            draw_string(surface, content_x + 25, item_y + 5, &share.name, accent_color);

                            // Type
                            draw_string(surface, content_x + 200, item_y + 5, share.share_type.name(), dim_text);

                            // Comment
                            if let Some(ref comment) = share.comment {
                                let short_comment = if comment.len() > 30 {
                                    let mut c: String = comment.chars().take(27).collect();
                                    c.push_str("...");
                                    c
                                } else {
                                    comment.clone()
                                };
                                draw_string(surface, content_x + 280, item_y + 5, &short_comment, dim_text);
                            }

                            item_y += item_height;
                        }
                    }
                }
            }

            ViewMode::Saved => {
                for (i, saved) in self.saved_connections.iter().skip(self.scroll_offset).take(visible_count).enumerate() {
                    let is_hovered = self.hovered_index == Some(i + self.scroll_offset);
                    let is_selected = self.selected_index == Some(i + self.scroll_offset);

                    if is_selected || is_hovered {
                        for y in 0..item_height as usize - 2 {
                            for x in 0..(self.bounds.width - self.sidebar_width - 20) {
                                surface.set_pixel(
                                    (content_x as usize) + x,
                                    (item_y as usize) + y,
                                    if is_selected { selected_bg } else { hover_bg }
                                );
                            }
                        }
                    }

                    draw_string(surface, content_x + 5, item_y + 5, &saved.name, text_color);
                    draw_string(surface, content_x + 200, item_y + 5, &saved.uri, dim_text);

                    item_y += item_height;
                }
            }

            ViewMode::MountPoints => {
                for (i, mount) in self.mount_points.iter().skip(self.scroll_offset).take(visible_count).enumerate() {
                    let is_hovered = self.hovered_index == Some(i + self.scroll_offset);
                    let is_selected = self.selected_index == Some(i + self.scroll_offset);

                    if is_selected || is_hovered {
                        for y in 0..item_height as usize - 2 {
                            for x in 0..(self.bounds.width - self.sidebar_width - 20) {
                                surface.set_pixel(
                                    (content_x as usize) + x,
                                    (item_y as usize) + y,
                                    if is_selected { selected_bg } else { hover_bg }
                                );
                            }
                        }
                    }

                    let status_color = if mount.is_mounted { connected_color } else { disconnected_color };
                    draw_char(surface, content_x + 5, item_y + 5, '●', status_color);

                    draw_string(surface, content_x + 25, item_y + 5, &mount.share_name, text_color);
                    draw_string(surface, content_x + 200, item_y + 5, &mount.local_path, dim_text);

                    item_y += item_height;
                }
            }
        }

        // Status bar
        let status_y = self.bounds.y + self.bounds.height as isize - 25;
        for x in 0..(self.bounds.width - self.sidebar_width) {
            surface.set_pixel(
                (content_x as usize) + x,
                status_y as usize,
                border_color
            );
        }

        // Status text
        let connected_count = self.servers.iter().filter(|s| s.connection_state.is_connected()).count();
        let status_str = format!("{} servers, {} connected", self.servers.len(), connected_count);
        draw_string(surface, content_x, status_y + 8, &status_str, dim_text);

        if self.is_loading {
            draw_string(surface, content_x + 200, status_y + 8, "Loading...", accent_color);
        }

        if let Some(ref err) = self.error_message {
            draw_string(surface, content_x + 300, status_y + 8, err, Color::new(255, 100, 100));
        }
    }
}

/// Initialize the network shares module
pub fn init() {
    crate::kprintln!("[NetworkShares] Network shares browser initialized");
}
