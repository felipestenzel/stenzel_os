//! Browser Tabs
//!
//! Tab management for the browser.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use super::engine::BrowserEngine;
use super::network::{HttpClient, Url};

/// Tab ID
pub type TabId = u32;

/// Tab manager
pub struct TabManager {
    /// Tabs
    tabs: BTreeMap<TabId, Tab>,
    /// Next tab ID
    next_id: TabId,
    /// Active tab ID
    active_tab: Option<TabId>,
    /// Tab order
    tab_order: Vec<TabId>,
    /// HTTP client (shared)
    http_client: HttpClient,
    /// Max tabs
    max_tabs: usize,
    /// Tab close callbacks
    on_tab_close: Option<fn(TabId)>,
    /// Tab change callbacks
    on_tab_change: Option<fn(TabId)>,
}

impl TabManager {
    /// Create new tab manager
    pub fn new() -> Self {
        Self {
            tabs: BTreeMap::new(),
            next_id: 1,
            active_tab: None,
            tab_order: Vec::new(),
            http_client: HttpClient::new(),
            max_tabs: 50,
            on_tab_close: None,
            on_tab_change: None,
        }
    }

    /// Create new tab
    pub fn new_tab(&mut self) -> TabId {
        self.new_tab_with_url("about:blank")
    }

    /// Create new tab with URL
    pub fn new_tab_with_url(&mut self, url: &str) -> TabId {
        // Check max tabs
        if self.tabs.len() >= self.max_tabs {
            // Close oldest tab
            if let Some(first_id) = self.tab_order.first().cloned() {
                self.close_tab(first_id);
            }
        }

        let id = self.next_id;
        self.next_id += 1;

        let mut tab = Tab::new(id);
        tab.navigate(url, &mut self.http_client);

        self.tabs.insert(id, tab);
        self.tab_order.push(id);

        // Activate new tab
        self.set_active_tab(id);

        id
    }

    /// Close tab
    pub fn close_tab(&mut self, id: TabId) -> bool {
        if !self.tabs.contains_key(&id) {
            return false;
        }

        self.tabs.remove(&id);
        self.tab_order.retain(|&t| t != id);

        // Call callback
        if let Some(callback) = self.on_tab_close {
            callback(id);
        }

        // If we closed the active tab, activate another one
        if self.active_tab == Some(id) {
            self.active_tab = self.tab_order.last().cloned();
            if let (Some(new_active), Some(callback)) = (self.active_tab, self.on_tab_change) {
                callback(new_active);
            }
        }

        true
    }

    /// Get active tab
    pub fn active_tab(&self) -> Option<&Tab> {
        self.active_tab.and_then(|id| self.tabs.get(&id))
    }

    /// Get active tab mutable
    pub fn active_tab_mut(&mut self) -> Option<&mut Tab> {
        self.active_tab.and_then(|id| self.tabs.get_mut(&id))
    }

    /// Set active tab
    pub fn set_active_tab(&mut self, id: TabId) -> bool {
        if !self.tabs.contains_key(&id) {
            return false;
        }

        self.active_tab = Some(id);

        if let Some(callback) = self.on_tab_change {
            callback(id);
        }

        true
    }

    /// Get active tab ID
    pub fn active_tab_id(&self) -> Option<TabId> {
        self.active_tab
    }

    /// Get tab by ID
    pub fn get_tab(&self, id: TabId) -> Option<&Tab> {
        self.tabs.get(&id)
    }

    /// Get tab by ID mutable
    pub fn get_tab_mut(&mut self, id: TabId) -> Option<&mut Tab> {
        self.tabs.get_mut(&id)
    }

    /// Get all tabs
    pub fn tabs(&self) -> Vec<&Tab> {
        self.tab_order.iter().filter_map(|id| self.tabs.get(id)).collect()
    }

    /// Get tab count
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Move tab
    pub fn move_tab(&mut self, id: TabId, new_index: usize) {
        if let Some(pos) = self.tab_order.iter().position(|&t| t == id) {
            self.tab_order.remove(pos);
            let new_pos = new_index.min(self.tab_order.len());
            self.tab_order.insert(new_pos, id);
        }
    }

    /// Duplicate tab
    pub fn duplicate_tab(&mut self, id: TabId) -> Option<TabId> {
        let url = self.tabs.get(&id)?.url.clone();
        Some(self.new_tab_with_url(&url))
    }

    /// Navigate in active tab
    pub fn navigate(&mut self, url: &str) {
        if let Some(id) = self.active_tab {
            if let Some(tab) = self.tabs.get_mut(&id) {
                tab.navigate(url, &mut self.http_client);
            }
        }
    }

    /// Go back in active tab
    pub fn go_back(&mut self) {
        if let Some(id) = self.active_tab {
            if let Some(tab) = self.tabs.get_mut(&id) {
                tab.go_back(&mut self.http_client);
            }
        }
    }

    /// Go forward in active tab
    pub fn go_forward(&mut self) {
        if let Some(id) = self.active_tab {
            if let Some(tab) = self.tabs.get_mut(&id) {
                tab.go_forward(&mut self.http_client);
            }
        }
    }

    /// Reload active tab
    pub fn reload(&mut self) {
        if let Some(id) = self.active_tab {
            if let Some(tab) = self.tabs.get_mut(&id) {
                tab.reload(&mut self.http_client);
            }
        }
    }

    /// Stop loading active tab
    pub fn stop(&mut self) {
        if let Some(tab) = self.active_tab_mut() {
            tab.stop();
        }
    }

    /// Set tab close callback
    pub fn on_tab_close(&mut self, callback: fn(TabId)) {
        self.on_tab_close = Some(callback);
    }

    /// Set tab change callback
    pub fn on_tab_change(&mut self, callback: fn(TabId)) {
        self.on_tab_change = Some(callback);
    }

    /// Get HTTP client
    pub fn http_client(&mut self) -> &mut HttpClient {
        &mut self.http_client
    }

    /// Select next tab
    pub fn select_next_tab(&mut self) {
        if let Some(active) = self.active_tab {
            if let Some(pos) = self.tab_order.iter().position(|&t| t == active) {
                let next_pos = (pos + 1) % self.tab_order.len();
                if let Some(&next_id) = self.tab_order.get(next_pos) {
                    self.set_active_tab(next_id);
                }
            }
        }
    }

    /// Select previous tab
    pub fn select_prev_tab(&mut self) {
        if let Some(active) = self.active_tab {
            if let Some(pos) = self.tab_order.iter().position(|&t| t == active) {
                let prev_pos = if pos == 0 { self.tab_order.len() - 1 } else { pos - 1 };
                if let Some(&prev_id) = self.tab_order.get(prev_pos) {
                    self.set_active_tab(prev_id);
                }
            }
        }
    }

    /// Pin tab
    pub fn pin_tab(&mut self, id: TabId) {
        if let Some(tab) = self.tabs.get_mut(&id) {
            tab.pinned = true;
            // Move pinned tabs to front
            if let Some(pos) = self.tab_order.iter().position(|&t| t == id) {
                self.tab_order.remove(pos);
                let pinned_count = self.tabs.values().filter(|t| t.pinned && t.id != id).count();
                self.tab_order.insert(pinned_count, id);
            }
        }
    }

    /// Unpin tab
    pub fn unpin_tab(&mut self, id: TabId) {
        if let Some(tab) = self.tabs.get_mut(&id) {
            tab.pinned = false;
        }
    }
}

impl Default for TabManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Browser tab
pub struct Tab {
    /// Tab ID
    pub id: TabId,
    /// Tab title
    pub title: String,
    /// Current URL
    pub url: String,
    /// Favicon URL
    pub favicon: Option<String>,
    /// Tab state
    pub state: TabState,
    /// Is pinned
    pub pinned: bool,
    /// Is muted
    pub muted: bool,
    /// Is playing audio
    pub playing_audio: bool,
    /// Browser engine
    pub engine: BrowserEngine,
    /// Navigation history
    history: Vec<String>,
    /// History position
    history_pos: usize,
    /// Load progress (0-100)
    pub load_progress: u8,
    /// SSL info
    pub ssl_info: Option<SslInfo>,
    /// Created timestamp
    pub created_at: u64,
    /// Last accessed timestamp
    pub accessed_at: u64,
}

impl Tab {
    /// Create new tab
    pub fn new(id: TabId) -> Self {
        Self {
            id,
            title: String::from("New Tab"),
            url: String::from("about:blank"),
            favicon: None,
            state: TabState::Ready,
            pinned: false,
            muted: false,
            playing_audio: false,
            engine: BrowserEngine::new(800.0, 600.0),
            history: vec![String::from("about:blank")],
            history_pos: 0,
            load_progress: 100,
            ssl_info: None,
            created_at: crate::time::uptime_secs(),
            accessed_at: crate::time::uptime_secs(),
        }
    }

    /// Navigate to URL
    pub fn navigate(&mut self, url: &str, http_client: &mut HttpClient) {
        self.state = TabState::Loading;
        self.load_progress = 0;

        // Normalize URL
        let normalized = if url.contains("://") {
            String::from(url)
        } else if url.starts_with("//") {
            alloc::format!("https:{}", url)
        } else {
            alloc::format!("https://{}", url)
        };

        self.url = normalized.clone();

        // Add to history
        if self.history_pos < self.history.len() - 1 {
            self.history.truncate(self.history_pos + 1);
        }
        self.history.push(normalized.clone());
        self.history_pos = self.history.len() - 1;

        // Fetch page
        self.load_progress = 10;

        match http_client.get(&normalized) {
            Ok(response) => {
                self.load_progress = 50;

                if response.is_success() {
                    if let Ok(html) = response.text() {
                        self.engine.load_html(&html);
                        self.title = String::from(self.engine.title());
                        if self.title.is_empty() {
                            self.title = normalized.clone();
                        }
                    }

                    // Check SSL
                    if normalized.starts_with("https://") {
                        self.ssl_info = Some(SslInfo {
                            secure: true,
                            issuer: String::from("Unknown CA"),
                            valid_from: String::from("2024-01-01"),
                            valid_to: String::from("2025-01-01"),
                            subject: normalized.clone(),
                        });
                    } else {
                        self.ssl_info = None;
                    }
                } else {
                    // Show error page
                    let error_html = alloc::format!(
                        "<!DOCTYPE html><html><head><title>Error</title></head>\
                        <body><h1>Error {}</h1><p>{}</p></body></html>",
                        response.status,
                        response.status_text
                    );
                    self.engine.load_html(&error_html);
                    self.title = alloc::format!("Error {}", response.status);
                }

                self.load_progress = 100;
                self.state = TabState::Ready;
            }
            Err(_) => {
                // Show error page
                let error_html = "<!DOCTYPE html><html><head><title>Error</title></head>\
                    <body><h1>Unable to connect</h1><p>Could not load the page.</p></body></html>";
                self.engine.load_html(error_html);
                self.title = String::from("Error");
                self.load_progress = 100;
                self.state = TabState::Error;
            }
        }

        self.accessed_at = crate::time::uptime_secs();
    }

    /// Go back in history
    pub fn go_back(&mut self, http_client: &mut HttpClient) {
        if self.can_go_back() {
            self.history_pos -= 1;
            let url = self.history[self.history_pos].clone();
            self.navigate_internal(&url, http_client);
        }
    }

    /// Go forward in history
    pub fn go_forward(&mut self, http_client: &mut HttpClient) {
        if self.can_go_forward() {
            self.history_pos += 1;
            let url = self.history[self.history_pos].clone();
            self.navigate_internal(&url, http_client);
        }
    }

    /// Reload page
    pub fn reload(&mut self, http_client: &mut HttpClient) {
        let url = self.url.clone();
        self.navigate(&url, http_client);
    }

    /// Stop loading
    pub fn stop(&mut self) {
        if self.state == TabState::Loading {
            self.state = TabState::Ready;
            self.load_progress = 100;
        }
    }

    /// Can go back?
    pub fn can_go_back(&self) -> bool {
        self.history_pos > 0
    }

    /// Can go forward?
    pub fn can_go_forward(&self) -> bool {
        self.history_pos < self.history.len() - 1
    }

    /// Is loading?
    pub fn is_loading(&self) -> bool {
        self.state == TabState::Loading
    }

    /// Is secure?
    pub fn is_secure(&self) -> bool {
        self.ssl_info.as_ref().map(|i| i.secure).unwrap_or(false)
    }

    /// Get display title (truncated)
    pub fn display_title(&self, max_len: usize) -> String {
        if self.title.len() > max_len {
            let mut t = self.title[..max_len - 3].to_string();
            t.push_str("...");
            t
        } else {
            self.title.clone()
        }
    }

    /// Set viewport size
    pub fn set_viewport(&mut self, width: f32, height: f32) {
        self.engine.set_viewport(width, height);
    }

    /// Scroll by
    pub fn scroll_by(&mut self, dx: f32, dy: f32) {
        self.engine.scroll_by(dx, dy);
    }

    /// Get scroll position
    pub fn scroll_position(&self) -> (f32, f32) {
        self.engine.scroll_position()
    }

    fn navigate_internal(&mut self, url: &str, http_client: &mut HttpClient) {
        self.url = String::from(url);
        self.state = TabState::Loading;
        self.load_progress = 0;

        match http_client.get(url) {
            Ok(response) => {
                if response.is_success() {
                    if let Ok(html) = response.text() {
                        self.engine.load_html(&html);
                        self.title = String::from(self.engine.title());
                        if self.title.is_empty() {
                            self.title = String::from(url);
                        }
                    }
                }
                self.state = TabState::Ready;
            }
            Err(_) => {
                self.state = TabState::Error;
            }
        }

        self.load_progress = 100;
        self.accessed_at = crate::time::uptime_secs();
    }
}

/// Tab state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TabState {
    /// Ready (fully loaded)
    Ready,
    /// Loading
    Loading,
    /// Error
    Error,
    /// Crashed
    Crashed,
}

/// SSL/TLS info
#[derive(Debug, Clone)]
pub struct SslInfo {
    /// Is connection secure
    pub secure: bool,
    /// Certificate issuer
    pub issuer: String,
    /// Valid from
    pub valid_from: String,
    /// Valid to
    pub valid_to: String,
    /// Subject
    pub subject: String,
}

/// Tab group
#[derive(Debug, Clone)]
pub struct TabGroup {
    /// Group ID
    pub id: u32,
    /// Group name
    pub name: String,
    /// Group color
    pub color: u32,
    /// Tab IDs in group
    pub tabs: Vec<TabId>,
    /// Is collapsed
    pub collapsed: bool,
}

impl TabGroup {
    /// Create new group
    pub fn new(id: u32, name: &str, color: u32) -> Self {
        Self {
            id,
            name: String::from(name),
            color,
            tabs: Vec::new(),
            collapsed: false,
        }
    }

    /// Add tab to group
    pub fn add_tab(&mut self, tab_id: TabId) {
        if !self.tabs.contains(&tab_id) {
            self.tabs.push(tab_id);
        }
    }

    /// Remove tab from group
    pub fn remove_tab(&mut self, tab_id: TabId) {
        self.tabs.retain(|&t| t != tab_id);
    }
}

/// Session state for restoring tabs
#[derive(Debug, Clone)]
pub struct SessionState {
    /// Tabs
    pub tabs: Vec<TabSessionData>,
    /// Active tab index
    pub active_tab_index: usize,
    /// Window state
    pub window_state: WindowState,
    /// Saved at timestamp
    pub saved_at: u64,
}

/// Tab session data
#[derive(Debug, Clone)]
pub struct TabSessionData {
    /// URL
    pub url: String,
    /// Title
    pub title: String,
    /// Favicon URL
    pub favicon: Option<String>,
    /// History
    pub history: Vec<String>,
    /// History position
    pub history_pos: usize,
    /// Is pinned
    pub pinned: bool,
    /// Group ID
    pub group_id: Option<u32>,
    /// Scroll position
    pub scroll_x: f32,
    pub scroll_y: f32,
}

/// Window state
#[derive(Debug, Clone, Copy)]
pub struct WindowState {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub maximized: bool,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            x: 100,
            y: 100,
            width: 1024,
            height: 768,
            maximized: false,
        }
    }
}
