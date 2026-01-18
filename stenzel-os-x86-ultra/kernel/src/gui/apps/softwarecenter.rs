//! Software Center
//!
//! Graphical application store for installing, updating, and managing packages.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use alloc::collections::BTreeMap;
use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use crate::gui::widgets::{Widget, WidgetId, Bounds, WidgetEvent, MouseButton};

/// Software Center application
pub struct SoftwareCenter {
    /// Widget ID
    id: WidgetId,
    /// Window bounds
    bounds: Bounds,
    /// Is enabled
    enabled: bool,
    /// Is visible
    visible: bool,
    /// Current view
    view: SoftwareCenterView,
    /// Search query
    search_query: String,
    /// Selected category
    selected_category: Option<AppCategory>,
    /// Featured apps
    featured: Vec<AppEntry>,
    /// All apps (cached from search/browse)
    apps: Vec<AppEntry>,
    /// Currently selected app
    selected_app: Option<AppEntry>,
    /// Installed apps
    installed: Vec<InstalledApp>,
    /// Pending updates
    updates: Vec<UpdateEntry>,
    /// Active downloads
    downloads: Vec<DownloadProgress>,
    /// Scroll offset for app list
    scroll_offset: i32,
    /// Scroll offset for details
    details_scroll: i32,
    /// Is sidebar collapsed
    sidebar_collapsed: bool,
    /// Notifications
    notifications: Vec<Notification>,
    /// Update check in progress
    checking_updates: bool,
    /// Last update check time
    last_update_check: u64,
    /// Auto-update enabled
    auto_update: bool,
}

/// View modes for the software center
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoftwareCenterView {
    /// Browse featured and categories
    Browse,
    /// Search results
    Search,
    /// App details
    Details,
    /// Installed apps
    Installed,
    /// Available updates
    Updates,
    /// Settings
    Settings,
}

/// Application category
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppCategory {
    /// All applications
    All,
    /// Audio & Video
    AudioVideo,
    /// Development
    Development,
    /// Education
    Education,
    /// Games
    Games,
    /// Graphics
    Graphics,
    /// Internet
    Internet,
    /// Office
    Office,
    /// Science
    Science,
    /// System
    System,
    /// Utilities
    Utilities,
}

impl AppCategory {
    /// Get display name
    pub fn name(&self) -> &'static str {
        match self {
            Self::All => "All",
            Self::AudioVideo => "Audio & Video",
            Self::Development => "Development",
            Self::Education => "Education",
            Self::Games => "Games",
            Self::Graphics => "Graphics",
            Self::Internet => "Internet",
            Self::Office => "Office",
            Self::Science => "Science",
            Self::System => "System",
            Self::Utilities => "Utilities",
        }
    }

    /// Get icon name
    pub fn icon(&self) -> &'static str {
        match self {
            Self::All => "view-grid",
            Self::AudioVideo => "multimedia",
            Self::Development => "code",
            Self::Education => "school",
            Self::Games => "gamepad",
            Self::Graphics => "palette",
            Self::Internet => "globe",
            Self::Office => "document",
            Self::Science => "flask",
            Self::System => "settings",
            Self::Utilities => "wrench",
        }
    }

    /// Get all categories
    pub fn all() -> &'static [AppCategory] {
        &[
            Self::All,
            Self::AudioVideo,
            Self::Development,
            Self::Education,
            Self::Games,
            Self::Graphics,
            Self::Internet,
            Self::Office,
            Self::Science,
            Self::System,
            Self::Utilities,
        ]
    }
}

/// Application entry from repository
#[derive(Debug, Clone)]
pub struct AppEntry {
    /// Package name
    pub name: String,
    /// Display name
    pub display_name: String,
    /// Summary/tagline
    pub summary: String,
    /// Full description
    pub description: String,
    /// Version
    pub version: String,
    /// Author/developer
    pub author: String,
    /// License
    pub license: String,
    /// Category
    pub category: AppCategory,
    /// Download size
    pub download_size: u64,
    /// Installed size
    pub installed_size: u64,
    /// Homepage URL
    pub homepage: String,
    /// Screenshots (URLs)
    pub screenshots: Vec<String>,
    /// Is installed
    pub installed: bool,
    /// Installed version (if installed)
    pub installed_version: Option<String>,
    /// Has update available
    pub has_update: bool,
    /// Star rating (0-5)
    pub rating: f32,
    /// Number of ratings
    pub rating_count: u32,
    /// Repository name
    pub repository: String,
}

impl AppEntry {
    /// Create a new app entry
    pub fn new(name: &str, display_name: &str) -> Self {
        Self {
            name: String::from(name),
            display_name: String::from(display_name),
            summary: String::new(),
            description: String::new(),
            version: String::from("1.0.0"),
            author: String::new(),
            license: String::new(),
            category: AppCategory::All,
            download_size: 0,
            installed_size: 0,
            homepage: String::new(),
            screenshots: Vec::new(),
            installed: false,
            installed_version: None,
            has_update: false,
            rating: 0.0,
            rating_count: 0,
            repository: String::from("main"),
        }
    }

    /// Format download size
    pub fn format_download_size(&self) -> String {
        format_size(self.download_size)
    }

    /// Format installed size
    pub fn format_installed_size(&self) -> String {
        format_size(self.installed_size)
    }
}

/// Installed application info
#[derive(Debug, Clone)]
pub struct InstalledApp {
    /// Package name
    pub name: String,
    /// Display name
    pub display_name: String,
    /// Installed version
    pub version: String,
    /// Install date (timestamp)
    pub install_date: u64,
    /// Installed size
    pub installed_size: u64,
    /// Category
    pub category: AppCategory,
    /// Is explicitly installed (not dependency)
    pub explicit: bool,
}

/// Update entry
#[derive(Debug, Clone)]
pub struct UpdateEntry {
    /// Package name
    pub name: String,
    /// Display name
    pub display_name: String,
    /// Current version
    pub current_version: String,
    /// New version
    pub new_version: String,
    /// Download size
    pub download_size: u64,
    /// Is security update
    pub security: bool,
    /// Changelog
    pub changelog: String,
}

/// Download progress
#[derive(Debug, Clone)]
pub struct DownloadProgress {
    /// Package name
    pub name: String,
    /// Display name
    pub display_name: String,
    /// Download state
    pub state: DownloadState,
    /// Total bytes
    pub total: u64,
    /// Downloaded bytes
    pub downloaded: u64,
    /// Speed in bytes/sec
    pub speed: u64,
    /// Start time
    pub start_time: u64,
}

impl DownloadProgress {
    /// Get progress percentage (0-100)
    pub fn percentage(&self) -> u32 {
        if self.total == 0 {
            0
        } else {
            ((self.downloaded * 100) / self.total) as u32
        }
    }

    /// Get ETA in seconds
    pub fn eta(&self) -> Option<u64> {
        if self.speed == 0 {
            None
        } else {
            let remaining = self.total.saturating_sub(self.downloaded);
            Some(remaining / self.speed)
        }
    }

    /// Format speed
    pub fn format_speed(&self) -> String {
        alloc::format!("{}/s", format_size(self.speed))
    }
}

/// Download state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadState {
    /// Queued
    Queued,
    /// Downloading
    Downloading,
    /// Installing
    Installing,
    /// Completed
    Completed,
    /// Failed
    Failed,
    /// Paused
    Paused,
}

/// Notification
#[derive(Debug, Clone)]
pub struct Notification {
    /// Notification ID
    pub id: u64,
    /// Title
    pub title: String,
    /// Message
    pub message: String,
    /// Type
    pub notification_type: NotificationType,
    /// Timestamp
    pub timestamp: u64,
    /// Is read
    pub read: bool,
}

/// Notification type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationType {
    /// Updates available
    UpdatesAvailable,
    /// Download complete
    DownloadComplete,
    /// Install complete
    InstallComplete,
    /// Error
    Error,
    /// Info
    Info,
}

impl SoftwareCenter {
    /// Create new software center
    pub fn new() -> Self {
        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(0, 0, 1024, 768),
            enabled: true,
            visible: true,
            view: SoftwareCenterView::Browse,
            search_query: String::new(),
            selected_category: Some(AppCategory::All),
            featured: Vec::new(),
            apps: Vec::new(),
            selected_app: None,
            installed: Vec::new(),
            updates: Vec::new(),
            downloads: Vec::new(),
            scroll_offset: 0,
            details_scroll: 0,
            sidebar_collapsed: false,
            notifications: Vec::new(),
            checking_updates: false,
            last_update_check: 0,
            auto_update: true,
        }
    }

    /// Set search query
    pub fn search(&mut self, query: &str) {
        self.search_query = String::from(query);
        if query.is_empty() {
            self.view = SoftwareCenterView::Browse;
        } else {
            self.view = SoftwareCenterView::Search;
            self.perform_search();
        }
        self.scroll_offset = 0;
    }

    /// Perform search
    fn perform_search(&mut self) {
        let query = self.search_query.to_lowercase();
        self.apps = self.featured.iter()
            .filter(|app| {
                app.name.to_lowercase().contains(&query) ||
                app.display_name.to_lowercase().contains(&query) ||
                app.summary.to_lowercase().contains(&query)
            })
            .cloned()
            .collect();
    }

    /// Select category
    pub fn select_category(&mut self, category: AppCategory) {
        self.selected_category = Some(category);
        self.view = SoftwareCenterView::Browse;
        self.load_category(category);
        self.scroll_offset = 0;
    }

    /// Load apps for category
    fn load_category(&mut self, category: AppCategory) {
        if category == AppCategory::All {
            self.apps = self.featured.clone();
        } else {
            self.apps = self.featured.iter()
                .filter(|app| app.category == category)
                .cloned()
                .collect();
        }
    }

    /// Select an app to view details
    pub fn select_app(&mut self, app: AppEntry) {
        self.selected_app = Some(app);
        self.view = SoftwareCenterView::Details;
        self.details_scroll = 0;
    }

    /// Go back from details
    pub fn go_back(&mut self) {
        self.selected_app = None;
        self.view = if self.search_query.is_empty() {
            SoftwareCenterView::Browse
        } else {
            SoftwareCenterView::Search
        };
    }

    /// Install an app
    pub fn install(&mut self, name: &str) -> Result<(), String> {
        if self.downloads.iter().any(|d| d.name == name) {
            return Err(String::from("Already downloading"));
        }

        let app = self.apps.iter()
            .find(|a| a.name == name)
            .or_else(|| self.selected_app.as_ref().filter(|a| a.name == name))
            .cloned();

        if let Some(app) = app {
            self.downloads.push(DownloadProgress {
                name: app.name.clone(),
                display_name: app.display_name.clone(),
                state: DownloadState::Queued,
                total: app.download_size,
                downloaded: 0,
                speed: 0,
                start_time: crate::time::realtime().tv_sec as u64,
            });
            Ok(())
        } else {
            Err(String::from("App not found"))
        }
    }

    /// Remove an app
    pub fn remove(&mut self, name: &str) -> Result<(), String> {
        self.installed.retain(|app| app.name != name);

        for app in &mut self.apps {
            if app.name == name {
                app.installed = false;
                app.installed_version = None;
            }
        }
        if let Some(ref mut app) = self.selected_app {
            if app.name == name {
                app.installed = false;
                app.installed_version = None;
            }
        }

        Ok(())
    }

    /// Check for updates
    pub fn check_updates(&mut self) {
        self.checking_updates = true;
        self.last_update_check = crate::time::realtime().tv_sec as u64;
        self.checking_updates = false;
    }

    /// Apply all updates
    pub fn apply_all_updates(&mut self) {
        for update in self.updates.clone() {
            let _ = self.install(&update.name);
        }
    }

    /// Apply a single update
    pub fn apply_update(&mut self, name: &str) {
        let update_name = self.updates.iter()
            .find(|u| u.name == name)
            .map(|u| u.name.clone());
        if let Some(update_name) = update_name {
            let _ = self.install(&update_name);
        }
    }

    /// Cancel a download
    pub fn cancel_download(&mut self, name: &str) {
        self.downloads.retain(|d| d.name != name);
    }

    /// Pause a download
    pub fn pause_download(&mut self, name: &str) {
        if let Some(download) = self.downloads.iter_mut().find(|d| d.name == name) {
            download.state = DownloadState::Paused;
        }
    }

    /// Resume a download
    pub fn resume_download(&mut self, name: &str) {
        if let Some(download) = self.downloads.iter_mut().find(|d| d.name == name) {
            if download.state == DownloadState::Paused {
                download.state = DownloadState::Downloading;
            }
        }
    }

    /// Load featured apps
    pub fn load_featured(&mut self) {
        self.featured = vec![
            create_sample_app("firefox", "Firefox", AppCategory::Internet,
                "Fast and private web browser", 80_000_000),
            create_sample_app("vscode", "Visual Studio Code", AppCategory::Development,
                "Code editor for developers", 90_000_000),
            create_sample_app("gimp", "GIMP", AppCategory::Graphics,
                "Image manipulation program", 100_000_000),
            create_sample_app("vlc", "VLC Media Player", AppCategory::AudioVideo,
                "Plays everything", 50_000_000),
            create_sample_app("libreoffice", "LibreOffice", AppCategory::Office,
                "Free office suite", 500_000_000),
            create_sample_app("steam", "Steam", AppCategory::Games,
                "Gaming platform", 200_000_000),
            create_sample_app("blender", "Blender", AppCategory::Graphics,
                "3D creation suite", 300_000_000),
            create_sample_app("audacity", "Audacity", AppCategory::AudioVideo,
                "Audio editor", 40_000_000),
        ];

        self.apps = self.featured.clone();
    }

    /// Show updates view
    pub fn show_updates(&mut self) {
        self.view = SoftwareCenterView::Updates;
        self.scroll_offset = 0;
    }

    /// Show installed view
    pub fn show_installed(&mut self) {
        self.view = SoftwareCenterView::Installed;
        self.scroll_offset = 0;
    }

    /// Show settings
    pub fn show_settings(&mut self) {
        self.view = SoftwareCenterView::Settings;
    }

    /// Toggle auto-update
    pub fn set_auto_update(&mut self, enabled: bool) {
        self.auto_update = enabled;
    }

    /// Add notification
    pub fn add_notification(&mut self, title: &str, message: &str, notification_type: NotificationType) {
        let id = self.notifications.len() as u64;
        self.notifications.push(Notification {
            id,
            title: String::from(title),
            message: String::from(message),
            notification_type,
            timestamp: crate::time::realtime().tv_sec as u64,
            read: false,
        });
    }

    /// Mark notification as read
    pub fn mark_read(&mut self, id: u64) {
        if let Some(notification) = self.notifications.iter_mut().find(|n| n.id == id) {
            notification.read = true;
        }
    }

    /// Clear all notifications
    pub fn clear_notifications(&mut self) {
        self.notifications.clear();
    }

    /// Get unread notification count
    pub fn unread_count(&self) -> usize {
        self.notifications.iter().filter(|n| !n.read).count()
    }

    /// Get update count
    pub fn update_count(&self) -> usize {
        self.updates.len()
    }
}

impl Default for SoftwareCenter {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for SoftwareCenter {
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
        if !self.enabled {
            return false;
        }

        match event {
            WidgetEvent::MouseDown { button: MouseButton::Left, x, y } => {
                let sidebar_width = if self.sidebar_collapsed { 60 } else { 200 };

                // Handle sidebar clicks
                if *x < self.bounds.x + sidebar_width as isize {
                    // Navigation item clicks
                    let nav_y = self.bounds.y + 60;
                    if *y >= nav_y && *y < nav_y + 40 {
                        self.view = SoftwareCenterView::Browse;
                    } else if *y >= nav_y + 40 && *y < nav_y + 80 {
                        self.view = SoftwareCenterView::Installed;
                    } else if *y >= nav_y + 80 && *y < nav_y + 120 {
                        self.view = SoftwareCenterView::Updates;
                    } else if *y >= nav_y + 120 && *y < nav_y + 160 {
                        self.view = SoftwareCenterView::Settings;
                    }
                    return true;
                }

                true
            }
            WidgetEvent::Scroll { delta_y, .. } => {
                match self.view {
                    SoftwareCenterView::Details => {
                        self.details_scroll = (self.details_scroll + delta_y * 30).max(0);
                    }
                    _ => {
                        self.scroll_offset = (self.scroll_offset + delta_y * 30).max(0);
                    }
                }
                true
            }
            WidgetEvent::Character { c } => {
                // Add to search
                self.search_query.push(*c);
                self.perform_search();
                true
            }
            WidgetEvent::KeyDown { key, .. } => {
                if *key == 8 && !self.search_query.is_empty() {
                    // Backspace
                    self.search_query.pop();
                    self.perform_search();
                }
                true
            }
            _ => false,
        }
    }

    fn render(&self, surface: &mut Surface) {
        let x = self.bounds.x.max(0) as usize;
        let y = self.bounds.y.max(0) as usize;
        let w = self.bounds.width;
        let h = self.bounds.height;

        // Background
        surface.fill_rect(x, y, w, h, Color::new(30, 30, 30));

        // Sidebar
        let sidebar_width = if self.sidebar_collapsed { 60 } else { 200 };
        surface.fill_rect(x, y, sidebar_width, h, Color::new(45, 45, 45));

        // Header/search bar
        let header_height = 56;
        if w > sidebar_width {
            surface.fill_rect(x + sidebar_width, y, w - sidebar_width, header_height, Color::new(45, 45, 45));
        }

        // Search box
        let search_x = x + sidebar_width + 16;
        let search_y = y + 12;
        let search_w = 400.min(w.saturating_sub(sidebar_width + 32));
        surface.fill_rect(search_x, search_y, search_w, 32, Color::new(60, 60, 60));

        // Content area
        let content_x = x + sidebar_width;
        let content_y = y + header_height;
        let content_w = w.saturating_sub(sidebar_width);
        let content_h = h.saturating_sub(header_height);

        match self.view {
            SoftwareCenterView::Browse | SoftwareCenterView::Search => {
                // App grid
                let card_width = 180;
                let card_height = 200;
                let padding = 16;
                let cols = (content_w.saturating_sub(padding) / (card_width + padding)).max(1);

                for (i, app) in self.apps.iter().enumerate() {
                    let col = i % cols;
                    let row = i / cols;
                    let scroll = self.scroll_offset.max(0) as usize;
                    let card_x = content_x + padding + col * (card_width + padding);
                    let base_y = content_y + padding + row * (card_height + padding);

                    if base_y >= scroll && base_y < scroll + content_h + card_height {
                        let card_y = base_y.saturating_sub(scroll);

                        // Card background
                        surface.fill_rect(card_x, card_y, card_width, card_height, Color::new(60, 60, 60));

                        // Icon placeholder
                        let icon_size = 64;
                        let icon_x = card_x + (card_width - icon_size) / 2;
                        let icon_y = card_y + 16;
                        surface.fill_rect(icon_x, icon_y, icon_size, icon_size, Color::new(90, 90, 90));

                        // Install button
                        if !app.installed {
                            let btn_w = 80;
                            let btn_h = 28;
                            let btn_x = card_x + (card_width - btn_w) / 2;
                            let btn_y = card_y + card_height - btn_h - 16;
                            surface.fill_rect(btn_x, btn_y, btn_w, btn_h, Color::new(76, 175, 80));
                        }
                    }
                }
            }
            SoftwareCenterView::Details => {
                if let Some(ref _app) = self.selected_app {
                    // Icon
                    surface.fill_rect(content_x + 32, content_y + 32, 128, 128, Color::new(90, 90, 90));

                    // Install/Remove button
                    let btn_x = content_x + content_w.saturating_sub(120);
                    let btn_color = Color::new(76, 175, 80);
                    surface.fill_rect(btn_x, content_y + 32, 100, 36, btn_color);

                    // Screenshots area
                    for i in 0..3 {
                        let ss_x = content_x + 32 + i * 336;
                        surface.fill_rect(ss_x, content_y + 200, 320, 180, Color::new(70, 70, 70));
                    }
                }
            }
            SoftwareCenterView::Updates => {
                let mut item_y = content_y + 16;
                for update in &self.updates {
                    surface.fill_rect(content_x + 16, item_y, content_w.saturating_sub(32), 80, Color::new(60, 60, 60));

                    if update.security {
                        let badge_x = content_x + content_w.saturating_sub(100);
                        surface.fill_rect(badge_x, item_y + 28, 60, 24, Color::new(255, 152, 0));
                    }

                    item_y += 88;
                }

                if !self.updates.is_empty() {
                    let btn_x = content_x + content_w.saturating_sub(140);
                    surface.fill_rect(btn_x, content_y + 16, 120, 36, Color::new(76, 175, 80));
                }
            }
            SoftwareCenterView::Installed => {
                let mut item_y = content_y + 16;
                for _app in &self.installed {
                    surface.fill_rect(content_x + 16, item_y, content_w.saturating_sub(32), 64, Color::new(60, 60, 60));
                    item_y += 72;
                }
            }
            SoftwareCenterView::Settings => {
                // Auto-update toggle
                let toggle_color = if self.auto_update { Color::new(76, 175, 80) } else { Color::new(117, 117, 117) };
                let toggle_x = content_x + content_w.saturating_sub(82);
                surface.fill_rect(toggle_x, content_y + 32, 50, 26, toggle_color);
            }
        }

        // Downloads panel (bottom)
        if !self.downloads.is_empty() {
            let panel_h = 80;
            let panel_y = y + h.saturating_sub(panel_h);
            surface.fill_rect(x + sidebar_width, panel_y, w.saturating_sub(sidebar_width), panel_h, Color::new(45, 45, 45));

            if let Some(download) = self.downloads.iter().find(|d| d.state == DownloadState::Downloading) {
                let progress_x = x + sidebar_width + 120;
                let progress_y = panel_y + 40;
                let progress_w = 300usize;
                let progress_h = 8;
                let filled = ((progress_w as u64 * download.downloaded) / download.total.max(1)) as usize;

                surface.fill_rect(progress_x, progress_y, progress_w, progress_h, Color::new(70, 70, 70));
                surface.fill_rect(progress_x, progress_y, filled, progress_h, Color::new(76, 175, 80));
            }
        }
    }
}

/// Format file size
fn format_size(bytes: u64) -> String {
    if bytes >= 1_000_000_000 {
        alloc::format!("{:.1} GB", bytes as f64 / 1_000_000_000.0)
    } else if bytes >= 1_000_000 {
        alloc::format!("{:.1} MB", bytes as f64 / 1_000_000.0)
    } else if bytes >= 1_000 {
        alloc::format!("{:.1} KB", bytes as f64 / 1_000.0)
    } else {
        alloc::format!("{} B", bytes)
    }
}

/// Create a sample app entry
fn create_sample_app(name: &str, display_name: &str, category: AppCategory, summary: &str, size: u64) -> AppEntry {
    let mut app = AppEntry::new(name, display_name);
    app.category = category;
    app.summary = String::from(summary);
    app.download_size = size;
    app.installed_size = size * 2;
    app.screenshots = vec![
        String::from("screenshot1.png"),
        String::from("screenshot2.png"),
    ];
    app.rating = 4.5;
    app.rating_count = 1000;
    app
}

/// Update notification service
pub struct UpdateNotificationService {
    /// Check interval in seconds
    check_interval: u64,
    /// Last check time
    last_check: u64,
    /// Pending notifications
    pending: Vec<UpdateNotification>,
    /// Is enabled
    enabled: bool,
}

/// Update notification
#[derive(Debug, Clone)]
pub struct UpdateNotification {
    /// Number of updates available
    pub update_count: u32,
    /// Has security updates
    pub has_security: bool,
    /// Total download size
    pub total_size: u64,
    /// Timestamp
    pub timestamp: u64,
}

impl UpdateNotificationService {
    /// Create new service
    pub fn new() -> Self {
        Self {
            check_interval: 3600,
            last_check: 0,
            pending: Vec::new(),
            enabled: true,
        }
    }

    /// Enable/disable service
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Set check interval
    pub fn set_interval(&mut self, seconds: u64) {
        self.check_interval = seconds;
    }

    /// Check if update check is due
    pub fn should_check(&self) -> bool {
        if !self.enabled {
            return false;
        }
        let now = crate::time::realtime().tv_sec as u64;
        now - self.last_check >= self.check_interval
    }

    /// Perform update check
    pub fn check_updates(&mut self) {
        self.last_check = crate::time::realtime().tv_sec as u64;
    }

    /// Get pending notifications
    pub fn get_notifications(&self) -> &[UpdateNotification] {
        &self.pending
    }

    /// Clear notifications
    pub fn clear(&mut self) {
        self.pending.clear();
    }
}

impl Default for UpdateNotificationService {
    fn default() -> Self {
        Self::new()
    }
}
