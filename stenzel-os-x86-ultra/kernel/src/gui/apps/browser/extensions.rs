//! Browser Extensions Support
//!
//! Provides WebExtensions-compatible browser extension support:
//! - Extension manifest parsing
//! - Content scripts injection
//! - Background scripts
//! - Extension storage
//! - Message passing between content and background
//! - Browser action (toolbar buttons)
//! - Context menus

extern crate alloc;

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use crate::sync::IrqSafeMutex;
use crate::util::{KResult, KError};

/// Extension ID type
pub type ExtensionId = u32;

/// Extension state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionState {
    /// Not installed
    NotInstalled,
    /// Installing
    Installing,
    /// Installed but disabled
    Disabled,
    /// Enabled and running
    Enabled,
    /// Updating
    Updating,
    /// Error state
    Error,
}

/// Extension type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionType {
    /// Standard extension
    Extension,
    /// Theme
    Theme,
    /// Language pack
    LanguagePack,
    /// Search provider
    SearchProvider,
}

/// Extension permission
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtensionPermission {
    /// Access to tabs API
    Tabs,
    /// Access active tab
    ActiveTab,
    /// Access all URLs
    AllUrls,
    /// Access specific URL pattern
    UrlPattern(String),
    /// Storage access
    Storage,
    /// Cookies access
    Cookies,
    /// History access
    History,
    /// Bookmarks access
    Bookmarks,
    /// Downloads access
    Downloads,
    /// Context menus
    ContextMenus,
    /// Notifications
    Notifications,
    /// Web request
    WebRequest,
    /// Web request blocking
    WebRequestBlocking,
    /// Background scripts
    Background,
    /// Clipboard read
    ClipboardRead,
    /// Clipboard write
    ClipboardWrite,
    /// Native messaging
    NativeMessaging,
    /// Geolocation
    Geolocation,
    /// Identity
    Identity,
    /// Management
    Management,
    /// Privacy
    Privacy,
    /// Proxy
    Proxy,
    /// Theme
    Theme,
    /// Unknown permission
    Unknown(String),
}

/// Extension manifest
#[derive(Debug, Clone)]
pub struct ExtensionManifest {
    /// Manifest version (2 or 3)
    pub manifest_version: u32,
    /// Extension name
    pub name: String,
    /// Extension version
    pub version: String,
    /// Description
    pub description: String,
    /// Author
    pub author: String,
    /// Homepage URL
    pub homepage_url: String,
    /// Icons (size -> path)
    pub icons: BTreeMap<u32, String>,
    /// Permissions
    pub permissions: Vec<ExtensionPermission>,
    /// Optional permissions
    pub optional_permissions: Vec<ExtensionPermission>,
    /// Host permissions
    pub host_permissions: Vec<String>,
    /// Background scripts
    pub background: Option<BackgroundConfig>,
    /// Content scripts
    pub content_scripts: Vec<ContentScript>,
    /// Browser action
    pub browser_action: Option<BrowserAction>,
    /// Page action
    pub page_action: Option<PageAction>,
    /// Options page
    pub options_page: Option<String>,
    /// Web accessible resources
    pub web_accessible_resources: Vec<String>,
}

impl Default for ExtensionManifest {
    fn default() -> Self {
        ExtensionManifest {
            manifest_version: 3,
            name: String::new(),
            version: String::from("1.0.0"),
            description: String::new(),
            author: String::new(),
            homepage_url: String::new(),
            icons: BTreeMap::new(),
            permissions: Vec::new(),
            optional_permissions: Vec::new(),
            host_permissions: Vec::new(),
            background: None,
            content_scripts: Vec::new(),
            browser_action: None,
            page_action: None,
            options_page: None,
            web_accessible_resources: Vec::new(),
        }
    }
}

/// Background script configuration
#[derive(Debug, Clone)]
pub struct BackgroundConfig {
    /// Service worker script (MV3)
    pub service_worker: Option<String>,
    /// Background scripts (MV2)
    pub scripts: Vec<String>,
    /// Persistent background page (MV2)
    pub persistent: bool,
}

/// Content script configuration
#[derive(Debug, Clone)]
pub struct ContentScript {
    /// URL match patterns
    pub matches: Vec<String>,
    /// Excluded URL patterns
    pub exclude_matches: Vec<String>,
    /// JavaScript files to inject
    pub js: Vec<String>,
    /// CSS files to inject
    pub css: Vec<String>,
    /// Run at (document_start, document_end, document_idle)
    pub run_at: ContentScriptRunAt,
    /// All frames or just main frame
    pub all_frames: bool,
    /// Match about:blank
    pub match_about_blank: bool,
}

/// When to run content script
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentScriptRunAt {
    /// Before DOM is loaded
    DocumentStart,
    /// After DOM is loaded
    DocumentEnd,
    /// After page is idle
    DocumentIdle,
}

impl Default for ContentScriptRunAt {
    fn default() -> Self {
        ContentScriptRunAt::DocumentIdle
    }
}

/// Browser action configuration
#[derive(Debug, Clone)]
pub struct BrowserAction {
    /// Default icon path
    pub default_icon: String,
    /// Default title
    pub default_title: String,
    /// Default popup HTML path
    pub default_popup: Option<String>,
    /// Badge text
    pub badge_text: String,
    /// Badge color
    pub badge_color: u32,
}

/// Page action configuration
#[derive(Debug, Clone)]
pub struct PageAction {
    /// Default icon path
    pub default_icon: String,
    /// Default title
    pub default_title: String,
    /// Default popup HTML path
    pub default_popup: Option<String>,
}

/// Installed extension
#[derive(Debug)]
pub struct Extension {
    /// Extension ID
    pub id: ExtensionId,
    /// Manifest
    pub manifest: ExtensionManifest,
    /// Extension type
    pub extension_type: ExtensionType,
    /// Current state
    pub state: ExtensionState,
    /// Install time
    pub install_time: u64,
    /// Update time
    pub update_time: u64,
    /// Extension directory path
    pub path: String,
    /// Storage data
    storage: BTreeMap<String, String>,
    /// Enabled on tabs
    enabled_tabs: Vec<u32>,
}

/// Context menu item
#[derive(Debug, Clone)]
pub struct ContextMenuItem {
    /// Item ID
    pub id: String,
    /// Parent item ID
    pub parent_id: Option<String>,
    /// Title
    pub title: String,
    /// Contexts where to show
    pub contexts: Vec<ContextType>,
    /// URL patterns to match
    pub document_url_patterns: Vec<String>,
    /// Target URL patterns
    pub target_url_patterns: Vec<String>,
    /// Enabled
    pub enabled: bool,
    /// Checked (for checkbox/radio items)
    pub checked: bool,
    /// Item type
    pub item_type: ContextMenuItemType,
}

/// Context types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextType {
    /// All contexts
    All,
    /// Page context
    Page,
    /// Selection context
    Selection,
    /// Link context
    Link,
    /// Editable context
    Editable,
    /// Image context
    Image,
    /// Video context
    Video,
    /// Audio context
    Audio,
    /// Frame context
    Frame,
    /// Browser action context
    BrowserAction,
    /// Page action context
    PageAction,
    /// Tab context
    Tab,
}

/// Context menu item type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextMenuItemType {
    /// Normal item
    Normal,
    /// Checkbox
    Checkbox,
    /// Radio
    Radio,
    /// Separator
    Separator,
}

/// Message from extension
#[derive(Debug, Clone)]
pub struct ExtensionMessage {
    /// Source extension ID
    pub from_extension: ExtensionId,
    /// Source (background, content, popup)
    pub from: MessageSource,
    /// Target (background, content, popup, tab)
    pub to: MessageTarget,
    /// Message data (JSON string)
    pub data: String,
    /// Timestamp
    pub timestamp: u64,
}

/// Message source
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageSource {
    /// Background script
    Background,
    /// Content script
    ContentScript,
    /// Popup
    Popup,
    /// Options page
    Options,
}

/// Message target
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageTarget {
    /// Background script
    Background,
    /// All content scripts
    AllContentScripts,
    /// Content script in specific tab
    Tab(u32),
    /// Runtime (sendMessage)
    Runtime,
}

/// Extension statistics
#[derive(Debug, Default)]
pub struct ExtensionStats {
    /// Extensions installed
    pub extensions_installed: AtomicU64,
    /// Extensions enabled
    pub extensions_enabled: AtomicU64,
    /// Content scripts injected
    pub content_scripts_injected: AtomicU64,
    /// Messages sent
    pub messages_sent: AtomicU64,
    /// Storage operations
    pub storage_operations: AtomicU64,
    /// Web requests intercepted
    pub web_requests_intercepted: AtomicU64,
}

/// Extension manager
pub struct ExtensionManager {
    /// Installed extensions
    extensions: Vec<Extension>,
    /// Context menu items (extension_id -> items)
    context_menus: BTreeMap<ExtensionId, Vec<ContextMenuItem>>,
    /// Message handlers
    message_handlers: BTreeMap<ExtensionId, Vec<Box<dyn Fn(&ExtensionMessage) + Send + Sync>>>,
    /// Next extension ID
    next_id: AtomicU32,
    /// Initialized
    initialized: bool,
    /// Statistics
    stats: ExtensionStats,
}

pub static EXTENSION_MANAGER: IrqSafeMutex<ExtensionManager> = IrqSafeMutex::new(ExtensionManager::new());

impl ExtensionManager {
    pub const fn new() -> Self {
        ExtensionManager {
            extensions: Vec::new(),
            context_menus: BTreeMap::new(),
            message_handlers: BTreeMap::new(),
            next_id: AtomicU32::new(1),
            initialized: false,
            stats: ExtensionStats {
                extensions_installed: AtomicU64::new(0),
                extensions_enabled: AtomicU64::new(0),
                content_scripts_injected: AtomicU64::new(0),
                messages_sent: AtomicU64::new(0),
                storage_operations: AtomicU64::new(0),
                web_requests_intercepted: AtomicU64::new(0),
            },
        }
    }

    /// Initialize extension manager
    pub fn init(&mut self) -> KResult<()> {
        if self.initialized {
            return Ok(());
        }

        // Load installed extensions from storage
        self.load_extensions()?;

        self.initialized = true;
        crate::kprintln!("browser: extension manager initialized with {} extension(s)",
            self.extensions.len());
        Ok(())
    }

    /// Load installed extensions
    fn load_extensions(&mut self) -> KResult<()> {
        // Would load from extension directory
        // For now, just initialize empty
        Ok(())
    }

    /// Install extension from path
    pub fn install(&mut self, path: &str) -> KResult<ExtensionId> {
        // Parse manifest
        let manifest = self.parse_manifest(path)?;

        let id = self.next_id.fetch_add(1, Ordering::SeqCst);

        let extension = Extension {
            id,
            manifest,
            extension_type: ExtensionType::Extension,
            state: ExtensionState::Disabled,
            install_time: crate::time::uptime_ms(),
            update_time: crate::time::uptime_ms(),
            path: String::from(path),
            storage: BTreeMap::new(),
            enabled_tabs: Vec::new(),
        };

        self.extensions.push(extension);
        self.stats.extensions_installed.fetch_add(1, Ordering::Relaxed);

        crate::kprintln!("browser: installed extension {} from {}", id, path);
        Ok(id)
    }

    /// Parse extension manifest
    fn parse_manifest(&self, _path: &str) -> KResult<ExtensionManifest> {
        // Would read and parse manifest.json
        // For now, return default
        Ok(ExtensionManifest::default())
    }

    /// Uninstall extension
    pub fn uninstall(&mut self, id: ExtensionId) -> KResult<()> {
        let pos = self.extensions.iter().position(|e| e.id == id)
            .ok_or(KError::NotFound)?;

        // Disable first
        if self.extensions[pos].state == ExtensionState::Enabled {
            self.disable(id)?;
        }

        // Remove context menus
        self.context_menus.remove(&id);

        // Remove message handlers
        self.message_handlers.remove(&id);

        // Remove extension
        self.extensions.remove(pos);

        crate::kprintln!("browser: uninstalled extension {}", id);
        Ok(())
    }

    /// Enable extension
    pub fn enable(&mut self, id: ExtensionId) -> KResult<()> {
        // First check state and get background config
        let (already_enabled, has_background) = {
            let extension = self.extensions.iter().find(|e| e.id == id)
                .ok_or(KError::NotFound)?;
            (extension.state == ExtensionState::Enabled, extension.manifest.background.is_some())
        };

        if already_enabled {
            return Ok(());
        }

        // Start background script if needed
        if has_background {
            Self::start_background_script_static(id)?;
        }

        // Now update state
        let extension = self.extensions.iter_mut().find(|e| e.id == id)
            .ok_or(KError::NotFound)?;
        extension.state = ExtensionState::Enabled;
        self.stats.extensions_enabled.fetch_add(1, Ordering::Relaxed);

        crate::kprintln!("browser: enabled extension {}", id);
        Ok(())
    }

    /// Disable extension
    pub fn disable(&mut self, id: ExtensionId) -> KResult<()> {
        // First check state
        let is_enabled = {
            let extension = self.extensions.iter().find(|e| e.id == id)
                .ok_or(KError::NotFound)?;
            extension.state == ExtensionState::Enabled
        };

        if !is_enabled {
            return Ok(());
        }

        // Stop background script
        Self::stop_background_script_static(id)?;

        // Update state
        let extension = self.extensions.iter_mut().find(|e| e.id == id)
            .ok_or(KError::NotFound)?;
        extension.state = ExtensionState::Disabled;
        self.stats.extensions_enabled.fetch_sub(1, Ordering::Relaxed);

        crate::kprintln!("browser: disabled extension {}", id);
        Ok(())
    }

    /// Start background script (static)
    fn start_background_script_static(_id: ExtensionId) -> KResult<()> {
        // Would run the background script
        Ok(())
    }

    /// Stop background script (static)
    fn stop_background_script_static(_id: ExtensionId) -> KResult<()> {
        // Would stop the background script
        Ok(())
    }

    /// Inject content scripts for a URL
    pub fn inject_content_scripts(&mut self, url: &str, tab_id: u32) -> KResult<Vec<ExtensionId>> {
        let mut injected = Vec::new();

        for extension in &self.extensions {
            if extension.state != ExtensionState::Enabled {
                continue;
            }

            for script in &extension.manifest.content_scripts {
                if self.url_matches_patterns(url, &script.matches, &script.exclude_matches) {
                    // Would inject JS and CSS
                    injected.push(extension.id);
                    self.stats.content_scripts_injected.fetch_add(1, Ordering::Relaxed);
                }
            }
        }

        Ok(injected)
    }

    /// Check if URL matches patterns
    fn url_matches_patterns(&self, url: &str, matches: &[String], excludes: &[String]) -> bool {
        // Check excludes first
        for pattern in excludes {
            if self.url_matches_pattern(url, pattern) {
                return false;
            }
        }

        // Check matches
        for pattern in matches {
            if self.url_matches_pattern(url, pattern) {
                return true;
            }
        }

        false
    }

    /// Check if URL matches a single pattern
    fn url_matches_pattern(&self, url: &str, pattern: &str) -> bool {
        // Simple pattern matching
        if pattern == "<all_urls>" {
            return true;
        }

        // Check for exact match
        if url == pattern {
            return true;
        }

        // Check for wildcard patterns (simplified)
        if pattern.contains('*') {
            let parts: Vec<&str> = pattern.split('*').collect();
            if parts.len() == 2 {
                return url.starts_with(parts[0]) && url.ends_with(parts[1]);
            }
        }

        false
    }

    /// Send message between extension components
    pub fn send_message(&mut self, message: ExtensionMessage) -> KResult<()> {
        self.stats.messages_sent.fetch_add(1, Ordering::Relaxed);

        // Find handlers for target
        if let Some(handlers) = self.message_handlers.get(&message.from_extension) {
            for handler in handlers {
                handler(&message);
            }
        }

        Ok(())
    }

    /// Storage get
    pub fn storage_get(&self, id: ExtensionId, key: &str) -> Option<String> {
        let extension = self.extensions.iter().find(|e| e.id == id)?;
        extension.storage.get(key).cloned()
    }

    /// Storage set
    pub fn storage_set(&mut self, id: ExtensionId, key: &str, value: &str) -> KResult<()> {
        let extension = self.extensions.iter_mut().find(|e| e.id == id)
            .ok_or(KError::NotFound)?;

        extension.storage.insert(String::from(key), String::from(value));
        self.stats.storage_operations.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }

    /// Storage remove
    pub fn storage_remove(&mut self, id: ExtensionId, key: &str) -> KResult<()> {
        let extension = self.extensions.iter_mut().find(|e| e.id == id)
            .ok_or(KError::NotFound)?;

        extension.storage.remove(key);
        self.stats.storage_operations.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }

    /// Add context menu item
    pub fn create_context_menu(&mut self, id: ExtensionId, item: ContextMenuItem) -> KResult<()> {
        let items = self.context_menus.entry(id).or_insert_with(Vec::new);
        items.push(item);
        Ok(())
    }

    /// Remove context menu item
    pub fn remove_context_menu(&mut self, id: ExtensionId, item_id: &str) -> KResult<()> {
        if let Some(items) = self.context_menus.get_mut(&id) {
            items.retain(|i| i.id != item_id);
        }
        Ok(())
    }

    /// Get context menu items for context
    pub fn get_context_menu_items(&self, context: ContextType, url: &str) -> Vec<(&Extension, &ContextMenuItem)> {
        let mut result = Vec::new();

        for extension in &self.extensions {
            if extension.state != ExtensionState::Enabled {
                continue;
            }

            if let Some(items) = self.context_menus.get(&extension.id) {
                for item in items {
                    if item.contexts.contains(&context) || item.contexts.contains(&ContextType::All) {
                        if item.document_url_patterns.is_empty()
                            || item.document_url_patterns.iter().any(|p| self.url_matches_pattern(url, p)) {
                            result.push((extension, item));
                        }
                    }
                }
            }
        }

        result
    }

    /// Set badge text
    pub fn set_badge_text(&mut self, id: ExtensionId, text: &str) -> KResult<()> {
        let extension = self.extensions.iter_mut().find(|e| e.id == id)
            .ok_or(KError::NotFound)?;

        if let Some(ref mut action) = extension.manifest.browser_action {
            action.badge_text = String::from(text);
        }

        Ok(())
    }

    /// Set badge color
    pub fn set_badge_color(&mut self, id: ExtensionId, color: u32) -> KResult<()> {
        let extension = self.extensions.iter_mut().find(|e| e.id == id)
            .ok_or(KError::NotFound)?;

        if let Some(ref mut action) = extension.manifest.browser_action {
            action.badge_color = color;
        }

        Ok(())
    }

    /// Get extension by ID
    pub fn get_extension(&self, id: ExtensionId) -> Option<&Extension> {
        self.extensions.iter().find(|e| e.id == id)
    }

    /// List all extensions
    pub fn list_extensions(&self) -> &[Extension] {
        &self.extensions
    }

    /// List enabled extensions
    pub fn list_enabled(&self) -> Vec<&Extension> {
        self.extensions.iter()
            .filter(|e| e.state == ExtensionState::Enabled)
            .collect()
    }

    /// Get statistics
    pub fn stats(&self) -> &ExtensionStats {
        &self.stats
    }

    /// Check if extension has permission
    pub fn has_permission(&self, id: ExtensionId, permission: &ExtensionPermission) -> bool {
        if let Some(extension) = self.get_extension(id) {
            extension.manifest.permissions.contains(permission)
        } else {
            false
        }
    }

    /// Request optional permission
    pub fn request_permission(&mut self, id: ExtensionId, permission: ExtensionPermission) -> KResult<bool> {
        let extension = self.extensions.iter_mut().find(|e| e.id == id)
            .ok_or(KError::NotFound)?;

        if extension.manifest.optional_permissions.contains(&permission) {
            extension.manifest.permissions.push(permission);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get extension count
    pub fn extension_count(&self) -> usize {
        self.extensions.len()
    }

    /// Get enabled count
    pub fn enabled_count(&self) -> usize {
        self.extensions.iter().filter(|e| e.state == ExtensionState::Enabled).count()
    }
}

/// Initialize extension manager
pub fn init() -> KResult<()> {
    EXTENSION_MANAGER.lock().init()
}

/// Install extension
pub fn install(path: &str) -> KResult<ExtensionId> {
    EXTENSION_MANAGER.lock().install(path)
}

/// Enable extension
pub fn enable(id: ExtensionId) -> KResult<()> {
    EXTENSION_MANAGER.lock().enable(id)
}

/// Disable extension
pub fn disable(id: ExtensionId) -> KResult<()> {
    EXTENSION_MANAGER.lock().disable(id)
}

/// Get extension count
pub fn extension_count() -> usize {
    EXTENSION_MANAGER.lock().extension_count()
}

/// Get enabled count
pub fn enabled_count() -> usize {
    EXTENSION_MANAGER.lock().enabled_count()
}
