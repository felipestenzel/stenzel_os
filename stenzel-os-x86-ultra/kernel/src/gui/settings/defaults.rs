//! Default Applications Settings
//!
//! Configure default applications for file types, URLs, and actions.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use spin::Mutex;

/// Global defaults settings state
static DEFAULTS_SETTINGS: Mutex<Option<DefaultsSettings>> = Mutex::new(None);

/// Defaults settings state
pub struct DefaultsSettings {
    /// Default applications for categories
    pub category_defaults: Vec<CategoryDefault>,
    /// MIME type associations
    pub mime_associations: Vec<MimeAssociation>,
    /// URL scheme handlers
    pub url_handlers: Vec<UrlHandler>,
    /// Available applications
    pub applications: Vec<ApplicationInfo>,
}

/// Default application for a category
#[derive(Debug, Clone)]
pub struct CategoryDefault {
    /// Category
    pub category: AppCategory,
    /// Default app ID
    pub app_id: Option<String>,
}

/// Application category
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppCategory {
    /// Web browser
    WebBrowser,
    /// Email client
    Email,
    /// Music player
    Music,
    /// Video player
    Video,
    /// Image viewer
    Photos,
    /// Text editor
    TextEditor,
    /// Terminal emulator
    Terminal,
    /// File manager
    FileManager,
    /// Calendar
    Calendar,
    /// PDF viewer
    PdfViewer,
    /// Archive manager
    ArchiveManager,
    /// Code editor
    CodeEditor,
}

impl AppCategory {
    pub fn name(&self) -> &'static str {
        match self {
            AppCategory::WebBrowser => "Web Browser",
            AppCategory::Email => "Email",
            AppCategory::Music => "Music",
            AppCategory::Video => "Video",
            AppCategory::Photos => "Photos",
            AppCategory::TextEditor => "Text Editor",
            AppCategory::Terminal => "Terminal",
            AppCategory::FileManager => "Files",
            AppCategory::Calendar => "Calendar",
            AppCategory::PdfViewer => "PDF Viewer",
            AppCategory::ArchiveManager => "Archives",
            AppCategory::CodeEditor => "Code Editor",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            AppCategory::WebBrowser => "Open web links and HTML files",
            AppCategory::Email => "Send and receive emails",
            AppCategory::Music => "Play audio files",
            AppCategory::Video => "Play video files",
            AppCategory::Photos => "View images and photos",
            AppCategory::TextEditor => "Edit plain text files",
            AppCategory::Terminal => "Command line interface",
            AppCategory::FileManager => "Browse and manage files",
            AppCategory::Calendar => "Manage events and schedules",
            AppCategory::PdfViewer => "View PDF documents",
            AppCategory::ArchiveManager => "Extract and create archives",
            AppCategory::CodeEditor => "Edit source code files",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            AppCategory::WebBrowser => "web-browser",
            AppCategory::Email => "mail-client",
            AppCategory::Music => "audio-player",
            AppCategory::Video => "video-player",
            AppCategory::Photos => "image-viewer",
            AppCategory::TextEditor => "text-editor",
            AppCategory::Terminal => "utilities-terminal",
            AppCategory::FileManager => "file-manager",
            AppCategory::Calendar => "calendar",
            AppCategory::PdfViewer => "application-pdf",
            AppCategory::ArchiveManager => "package-x-generic",
            AppCategory::CodeEditor => "text-x-generic",
        }
    }

    pub fn all() -> &'static [AppCategory] {
        &[
            AppCategory::WebBrowser,
            AppCategory::Email,
            AppCategory::Music,
            AppCategory::Video,
            AppCategory::Photos,
            AppCategory::TextEditor,
            AppCategory::Terminal,
            AppCategory::FileManager,
            AppCategory::Calendar,
            AppCategory::PdfViewer,
            AppCategory::ArchiveManager,
            AppCategory::CodeEditor,
        ]
    }

    /// Get common MIME types for this category
    pub fn mime_types(&self) -> &'static [&'static str] {
        match self {
            AppCategory::WebBrowser => &["text/html", "application/xhtml+xml", "x-scheme-handler/http", "x-scheme-handler/https"],
            AppCategory::Email => &["x-scheme-handler/mailto", "message/rfc822"],
            AppCategory::Music => &["audio/mpeg", "audio/flac", "audio/ogg", "audio/wav", "audio/aac", "audio/mp4"],
            AppCategory::Video => &["video/mp4", "video/x-matroska", "video/webm", "video/avi", "video/quicktime"],
            AppCategory::Photos => &["image/png", "image/jpeg", "image/gif", "image/webp", "image/svg+xml", "image/bmp"],
            AppCategory::TextEditor => &["text/plain", "text/markdown"],
            AppCategory::Terminal => &[],
            AppCategory::FileManager => &["inode/directory"],
            AppCategory::Calendar => &["text/calendar", "application/ics"],
            AppCategory::PdfViewer => &["application/pdf"],
            AppCategory::ArchiveManager => &["application/zip", "application/x-tar", "application/gzip", "application/x-7z-compressed", "application/x-rar-compressed"],
            AppCategory::CodeEditor => &["text/x-c", "text/x-c++", "text/x-python", "text/x-rust", "text/javascript", "application/json", "text/x-java"],
        }
    }
}

/// MIME type association
#[derive(Debug, Clone)]
pub struct MimeAssociation {
    /// MIME type
    pub mime_type: String,
    /// Default app ID
    pub default_app: Option<String>,
    /// All apps that can handle this type
    pub handlers: Vec<String>,
}

/// URL scheme handler
#[derive(Debug, Clone)]
pub struct UrlHandler {
    /// URL scheme (e.g., "http", "mailto", "steam")
    pub scheme: String,
    /// Default app ID
    pub default_app: Option<String>,
    /// All apps that can handle this scheme
    pub handlers: Vec<String>,
}

/// Application info
#[derive(Debug, Clone)]
pub struct ApplicationInfo {
    /// App ID
    pub id: String,
    /// Display name
    pub name: String,
    /// Icon name
    pub icon: String,
    /// Executable path
    pub exec: String,
    /// Supported MIME types
    pub mime_types: Vec<String>,
    /// Supported URL schemes
    pub url_schemes: Vec<String>,
    /// Is system app
    pub system_app: bool,
}

/// Initialize defaults settings
pub fn init() {
    let mut state = DEFAULTS_SETTINGS.lock();
    if state.is_some() {
        return;
    }

    // Initialize category defaults
    let mut category_defaults = Vec::new();
    for category in AppCategory::all() {
        category_defaults.push(CategoryDefault {
            category: *category,
            app_id: None,
        });
    }

    // Initialize with some built-in apps
    let applications = vec![
        ApplicationInfo {
            id: "org.stenzel.files".to_string(),
            name: "Files".to_string(),
            icon: "file-manager".to_string(),
            exec: "/usr/bin/files".to_string(),
            mime_types: vec!["inode/directory".to_string()],
            url_schemes: vec!["file".to_string()],
            system_app: true,
        },
        ApplicationInfo {
            id: "org.stenzel.terminal".to_string(),
            name: "Terminal".to_string(),
            icon: "utilities-terminal".to_string(),
            exec: "/usr/bin/terminal".to_string(),
            mime_types: Vec::new(),
            url_schemes: Vec::new(),
            system_app: true,
        },
        ApplicationInfo {
            id: "org.stenzel.text-editor".to_string(),
            name: "Text Editor".to_string(),
            icon: "text-editor".to_string(),
            exec: "/usr/bin/text-editor".to_string(),
            mime_types: vec!["text/plain".to_string(), "text/markdown".to_string()],
            url_schemes: Vec::new(),
            system_app: true,
        },
        ApplicationInfo {
            id: "org.stenzel.image-viewer".to_string(),
            name: "Image Viewer".to_string(),
            icon: "image-viewer".to_string(),
            exec: "/usr/bin/image-viewer".to_string(),
            mime_types: vec!["image/png".to_string(), "image/jpeg".to_string(), "image/gif".to_string()],
            url_schemes: Vec::new(),
            system_app: true,
        },
        ApplicationInfo {
            id: "org.stenzel.settings".to_string(),
            name: "Settings".to_string(),
            icon: "preferences-system".to_string(),
            exec: "/usr/bin/settings".to_string(),
            mime_types: Vec::new(),
            url_schemes: vec!["settings".to_string()],
            system_app: true,
        },
    ];

    // Set built-in defaults
    let mut defaults = category_defaults;
    defaults.iter_mut().find(|d| d.category == AppCategory::FileManager).map(|d| d.app_id = Some("org.stenzel.files".to_string()));
    defaults.iter_mut().find(|d| d.category == AppCategory::Terminal).map(|d| d.app_id = Some("org.stenzel.terminal".to_string()));
    defaults.iter_mut().find(|d| d.category == AppCategory::TextEditor).map(|d| d.app_id = Some("org.stenzel.text-editor".to_string()));
    defaults.iter_mut().find(|d| d.category == AppCategory::Photos).map(|d| d.app_id = Some("org.stenzel.image-viewer".to_string()));

    // Build MIME associations
    let mut mime_associations = Vec::new();
    for app in &applications {
        for mime in &app.mime_types {
            if let Some(assoc) = mime_associations.iter_mut().find(|a: &&mut MimeAssociation| a.mime_type == *mime) {
                assoc.handlers.push(app.id.clone());
            } else {
                mime_associations.push(MimeAssociation {
                    mime_type: mime.clone(),
                    default_app: Some(app.id.clone()),
                    handlers: vec![app.id.clone()],
                });
            }
        }
    }

    // Build URL handlers
    let mut url_handlers = Vec::new();
    for app in &applications {
        for scheme in &app.url_schemes {
            if let Some(handler) = url_handlers.iter_mut().find(|h: &&mut UrlHandler| h.scheme == *scheme) {
                handler.handlers.push(app.id.clone());
            } else {
                url_handlers.push(UrlHandler {
                    scheme: scheme.clone(),
                    default_app: Some(app.id.clone()),
                    handlers: vec![app.id.clone()],
                });
            }
        }
    }

    *state = Some(DefaultsSettings {
        category_defaults: defaults,
        mime_associations,
        url_handlers,
        applications,
    });

    crate::kprintln!("defaults settings: initialized");
}

/// Get default app for category
pub fn get_default_for_category(category: AppCategory) -> Option<ApplicationInfo> {
    let state = DEFAULTS_SETTINGS.lock();
    state.as_ref().and_then(|s| {
        let default = s.category_defaults.iter().find(|d| d.category == category)?;
        let app_id = default.app_id.as_ref()?;
        s.applications.iter().find(|a| &a.id == app_id).cloned()
    })
}

/// Set default app for category
pub fn set_default_for_category(category: AppCategory, app_id: Option<&str>) {
    let mut state = DEFAULTS_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        if let Some(default) = s.category_defaults.iter_mut().find(|d| d.category == category) {
            default.app_id = app_id.map(|s| s.to_string());
        }
    }
}

/// Get category defaults
pub fn get_category_defaults() -> Vec<CategoryDefault> {
    let state = DEFAULTS_SETTINGS.lock();
    state.as_ref().map(|s| s.category_defaults.clone()).unwrap_or_default()
}

/// Get default app for MIME type
pub fn get_default_for_mime(mime_type: &str) -> Option<ApplicationInfo> {
    let state = DEFAULTS_SETTINGS.lock();
    state.as_ref().and_then(|s| {
        let assoc = s.mime_associations.iter().find(|a| a.mime_type == mime_type)?;
        let app_id = assoc.default_app.as_ref()?;
        s.applications.iter().find(|a| &a.id == app_id).cloned()
    })
}

/// Set default app for MIME type
pub fn set_default_for_mime(mime_type: &str, app_id: Option<&str>) {
    let mut state = DEFAULTS_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        if let Some(assoc) = s.mime_associations.iter_mut().find(|a| a.mime_type == mime_type) {
            assoc.default_app = app_id.map(|s| s.to_string());
        } else {
            s.mime_associations.push(MimeAssociation {
                mime_type: mime_type.to_string(),
                default_app: app_id.map(|s| s.to_string()),
                handlers: app_id.map(|s| vec![s.to_string()]).unwrap_or_default(),
            });
        }
    }
}

/// Get apps that can handle MIME type
pub fn get_handlers_for_mime(mime_type: &str) -> Vec<ApplicationInfo> {
    let state = DEFAULTS_SETTINGS.lock();
    state.as_ref().map(|s| {
        let assoc = s.mime_associations.iter().find(|a| a.mime_type == mime_type);
        if let Some(assoc) = assoc {
            assoc.handlers.iter()
                .filter_map(|id| s.applications.iter().find(|a| &a.id == id).cloned())
                .collect()
        } else {
            Vec::new()
        }
    }).unwrap_or_default()
}

/// Get default app for URL scheme
pub fn get_default_for_scheme(scheme: &str) -> Option<ApplicationInfo> {
    let state = DEFAULTS_SETTINGS.lock();
    state.as_ref().and_then(|s| {
        let handler = s.url_handlers.iter().find(|h| h.scheme == scheme)?;
        let app_id = handler.default_app.as_ref()?;
        s.applications.iter().find(|a| &a.id == app_id).cloned()
    })
}

/// Set default app for URL scheme
pub fn set_default_for_scheme(scheme: &str, app_id: Option<&str>) {
    let mut state = DEFAULTS_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        if let Some(handler) = s.url_handlers.iter_mut().find(|h| h.scheme == scheme) {
            handler.default_app = app_id.map(|s| s.to_string());
        } else {
            s.url_handlers.push(UrlHandler {
                scheme: scheme.to_string(),
                default_app: app_id.map(|s| s.to_string()),
                handlers: app_id.map(|s| vec![s.to_string()]).unwrap_or_default(),
            });
        }
    }
}

/// Get all MIME associations
pub fn get_mime_associations() -> Vec<MimeAssociation> {
    let state = DEFAULTS_SETTINGS.lock();
    state.as_ref().map(|s| s.mime_associations.clone()).unwrap_or_default()
}

/// Get all URL handlers
pub fn get_url_handlers() -> Vec<UrlHandler> {
    let state = DEFAULTS_SETTINGS.lock();
    state.as_ref().map(|s| s.url_handlers.clone()).unwrap_or_default()
}

/// Get all applications
pub fn get_applications() -> Vec<ApplicationInfo> {
    let state = DEFAULTS_SETTINGS.lock();
    state.as_ref().map(|s| s.applications.clone()).unwrap_or_default()
}

/// Get application by ID
pub fn get_application(app_id: &str) -> Option<ApplicationInfo> {
    let state = DEFAULTS_SETTINGS.lock();
    state.as_ref().and_then(|s| {
        s.applications.iter().find(|a| a.id == app_id).cloned()
    })
}

/// Register application
pub fn register_application(app: ApplicationInfo) {
    let mut state = DEFAULTS_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        // Remove existing if present
        s.applications.retain(|a| a.id != app.id);

        // Add MIME handlers
        for mime in &app.mime_types {
            if let Some(assoc) = s.mime_associations.iter_mut().find(|a| a.mime_type == *mime) {
                if !assoc.handlers.contains(&app.id) {
                    assoc.handlers.push(app.id.clone());
                }
            } else {
                s.mime_associations.push(MimeAssociation {
                    mime_type: mime.clone(),
                    default_app: None,
                    handlers: vec![app.id.clone()],
                });
            }
        }

        // Add URL handlers
        for scheme in &app.url_schemes {
            if let Some(handler) = s.url_handlers.iter_mut().find(|h| h.scheme == *scheme) {
                if !handler.handlers.contains(&app.id) {
                    handler.handlers.push(app.id.clone());
                }
            } else {
                s.url_handlers.push(UrlHandler {
                    scheme: scheme.clone(),
                    default_app: None,
                    handlers: vec![app.id.clone()],
                });
            }
        }

        // Add application
        s.applications.push(app);
    }
}

/// Defaults error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultsError {
    NotInitialized,
    AppNotFound,
    MimeTypeNotSupported,
}
