//! Application Launcher
//!
//! Spotlight/GNOME-style application launcher with:
//! - Full-screen or popup mode
//! - Search with fuzzy matching
//! - Application categories
//! - Recent apps
//! - Quick actions (calculator, unit conversion)
//! - File search integration

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use spin::Mutex;

use crate::drivers::framebuffer::Color;
use super::surface::{Surface, PixelFormat};

static LAUNCHER_STATE: Mutex<Option<LauncherState>> = Mutex::new(None);
static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Launcher configuration
#[derive(Debug, Clone)]
pub struct LauncherConfig {
    /// Full-screen mode (GNOME-style) or popup (Spotlight-style)
    pub mode: LauncherMode,
    /// Maximum search results
    pub max_results: usize,
    /// Show recent apps
    pub show_recent: bool,
    /// Number of recent apps to show
    pub recent_count: usize,
    /// Show categories
    pub show_categories: bool,
    /// Enable fuzzy search
    pub fuzzy_search: bool,
    /// Enable quick actions (calculator, etc.)
    pub quick_actions: bool,
    /// Search files
    pub search_files: bool,
    /// Theme
    pub theme: LauncherTheme,
}

impl Default for LauncherConfig {
    fn default() -> Self {
        Self {
            mode: LauncherMode::Popup,
            max_results: 10,
            show_recent: true,
            recent_count: 5,
            show_categories: true,
            fuzzy_search: true,
            quick_actions: true,
            search_files: true,
            theme: LauncherTheme::default(),
        }
    }
}

/// Launcher mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LauncherMode {
    /// Full-screen overlay
    Fullscreen,
    /// Centered popup
    Popup,
    /// Dropdown from panel
    Dropdown,
}

/// Launcher theme
#[derive(Debug, Clone)]
pub struct LauncherTheme {
    pub background: Color,
    pub search_background: Color,
    pub search_text: Color,
    pub result_background: Color,
    pub result_hover: Color,
    pub result_text: Color,
    pub category_text: Color,
    pub hint_text: Color,
    pub border_radius: u32,
    pub shadow: bool,
}

impl Default for LauncherTheme {
    fn default() -> Self {
        Self {
            background: Color::rgba(30, 30, 30, 240),
            search_background: Color::rgba(50, 50, 50, 255),
            search_text: Color::rgba(255, 255, 255, 255),
            result_background: Color::rgba(40, 40, 40, 255),
            result_hover: Color::rgba(66, 133, 244, 255),
            result_text: Color::rgba(255, 255, 255, 255),
            category_text: Color::rgba(150, 150, 150, 255),
            hint_text: Color::rgba(100, 100, 100, 255),
            border_radius: 12,
            shadow: true,
        }
    }
}

/// Launcher state
#[derive(Debug)]
pub struct LauncherState {
    /// Configuration
    pub config: LauncherConfig,
    /// Visible
    pub visible: bool,
    /// Current search query
    pub search_query: String,
    /// Search results
    pub results: Vec<SearchResult>,
    /// Selected result index
    pub selected_index: usize,
    /// All registered applications
    pub applications: Vec<Application>,
    /// Recent applications (by app_id)
    pub recent_apps: Vec<String>,
    /// Categories
    pub categories: Vec<AppCategory>,
    /// Quick action result (if any)
    pub quick_action_result: Option<String>,
    /// Position and size
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Application entry
#[derive(Debug, Clone)]
pub struct Application {
    pub id: String,
    pub name: String,
    pub generic_name: Option<String>,
    pub comment: Option<String>,
    pub exec: String,
    pub icon: Option<Vec<u8>>,
    pub icon_name: Option<String>,
    pub categories: Vec<String>,
    pub keywords: Vec<String>,
    pub terminal: bool,
    pub no_display: bool,
    pub launch_count: u32,
}

/// Application category
#[derive(Debug, Clone)]
pub struct AppCategory {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
}

/// Search result types
#[derive(Debug, Clone)]
pub enum SearchResult {
    /// Application
    Application(AppResult),
    /// File
    File(FileResult),
    /// Quick action (calculator, etc.)
    QuickAction(QuickActionResult),
    /// Web search suggestion
    WebSearch(String),
}

/// Application search result
#[derive(Debug, Clone)]
pub struct AppResult {
    pub app_id: String,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<Vec<u8>>,
    pub exec: String,
    pub score: u32,
}

/// File search result
#[derive(Debug, Clone)]
pub struct FileResult {
    pub path: String,
    pub name: String,
    pub file_type: FileType,
    pub size: u64,
    pub modified: u64,
    pub score: u32,
}

/// File type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    File,
    Directory,
    Symlink,
    Image,
    Video,
    Audio,
    Document,
    Archive,
    Executable,
}

/// Quick action result
#[derive(Debug, Clone)]
pub struct QuickActionResult {
    pub action_type: QuickActionType,
    pub query: String,
    pub result: String,
    pub copyable: bool,
}

/// Quick action types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickActionType {
    Calculator,
    UnitConversion,
    CurrencyConversion,
    ColorPreview,
    Definition,
    Translation,
}

/// Error type
#[derive(Debug, Clone, Copy)]
pub enum LauncherError {
    NotInitialized,
    AppNotFound,
    SearchFailed,
}

/// Initialize launcher
pub fn init(config: LauncherConfig) -> Result<(), LauncherError> {
    if INITIALIZED.load(Ordering::Acquire) {
        return Ok(());
    }

    let categories = vec![
        AppCategory { id: String::from("utilities"), name: String::from("Utilities"), icon: Some(String::from("applications-utilities")) },
        AppCategory { id: String::from("development"), name: String::from("Development"), icon: Some(String::from("applications-development")) },
        AppCategory { id: String::from("graphics"), name: String::from("Graphics"), icon: Some(String::from("applications-graphics")) },
        AppCategory { id: String::from("internet"), name: String::from("Internet"), icon: Some(String::from("applications-internet")) },
        AppCategory { id: String::from("multimedia"), name: String::from("Multimedia"), icon: Some(String::from("applications-multimedia")) },
        AppCategory { id: String::from("office"), name: String::from("Office"), icon: Some(String::from("applications-office")) },
        AppCategory { id: String::from("games"), name: String::from("Games"), icon: Some(String::from("applications-games")) },
        AppCategory { id: String::from("settings"), name: String::from("Settings"), icon: Some(String::from("preferences-system")) },
        AppCategory { id: String::from("system"), name: String::from("System"), icon: Some(String::from("applications-system")) },
    ];

    let state = LauncherState {
        config,
        visible: false,
        search_query: String::new(),
        results: Vec::new(),
        selected_index: 0,
        applications: Vec::new(),
        recent_apps: Vec::new(),
        categories,
        quick_action_result: None,
        x: 0,
        y: 0,
        width: 600,
        height: 400,
    };

    *LAUNCHER_STATE.lock() = Some(state);
    INITIALIZED.store(true, Ordering::Release);

    crate::kprintln!("launcher: Application launcher initialized");
    Ok(())
}

/// Register an application
pub fn register_app(app: Application) -> Result<(), LauncherError> {
    let mut state = LAUNCHER_STATE.lock();
    let state = state.as_mut().ok_or(LauncherError::NotInitialized)?;

    // Check if already registered
    if !state.applications.iter().any(|a| a.id == app.id) {
        state.applications.push(app);
    }

    Ok(())
}

/// Unregister an application
pub fn unregister_app(app_id: &str) -> Result<(), LauncherError> {
    let mut state = LAUNCHER_STATE.lock();
    let state = state.as_mut().ok_or(LauncherError::NotInitialized)?;

    state.applications.retain(|a| a.id != app_id);
    state.recent_apps.retain(|id| id != app_id);

    Ok(())
}

/// Show launcher
pub fn show() -> Result<(), LauncherError> {
    let mut state = LAUNCHER_STATE.lock();
    let state = state.as_mut().ok_or(LauncherError::NotInitialized)?;

    state.visible = true;
    state.search_query.clear();
    state.results.clear();
    state.selected_index = 0;
    state.quick_action_result = None;

    // Show recent apps if enabled
    if state.config.show_recent && !state.recent_apps.is_empty() {
        let recent: Vec<_> = state.recent_apps.iter()
            .filter_map(|id| state.applications.iter().find(|a| &a.id == id))
            .take(state.config.recent_count)
            .map(|app| SearchResult::Application(AppResult {
                app_id: app.id.clone(),
                name: app.name.clone(),
                description: app.comment.clone(),
                icon: app.icon.clone(),
                exec: app.exec.clone(),
                score: 100,
            }))
            .collect();
        state.results = recent;
    }

    Ok(())
}

/// Hide launcher
pub fn hide() -> Result<(), LauncherError> {
    let mut state = LAUNCHER_STATE.lock();
    let state = state.as_mut().ok_or(LauncherError::NotInitialized)?;

    state.visible = false;

    Ok(())
}

/// Toggle launcher visibility
pub fn toggle() -> Result<bool, LauncherError> {
    let mut state = LAUNCHER_STATE.lock();
    let state = state.as_mut().ok_or(LauncherError::NotInitialized)?;

    state.visible = !state.visible;

    if state.visible {
        state.search_query.clear();
        state.results.clear();
        state.selected_index = 0;
    }

    Ok(state.visible)
}

/// Update search query and perform search
pub fn search(query: &str) -> Result<Vec<SearchResult>, LauncherError> {
    let mut state = LAUNCHER_STATE.lock();
    let state = state.as_mut().ok_or(LauncherError::NotInitialized)?;

    state.search_query = String::from(query);
    state.selected_index = 0;
    state.results.clear();
    state.quick_action_result = None;

    if query.is_empty() {
        // Show recent apps
        if state.config.show_recent {
            let recent: Vec<_> = state.recent_apps.iter()
                .filter_map(|id| state.applications.iter().find(|a| &a.id == id))
                .take(state.config.recent_count)
                .map(|app| SearchResult::Application(AppResult {
                    app_id: app.id.clone(),
                    name: app.name.clone(),
                    description: app.comment.clone(),
                    icon: app.icon.clone(),
                    exec: app.exec.clone(),
                    score: 100,
                }))
                .collect();
            state.results = recent;
        }
        return Ok(state.results.clone());
    }

    let query_lower = query.to_lowercase();

    // Check for quick actions
    if state.config.quick_actions {
        if let Some(result) = try_quick_action(&query_lower) {
            state.quick_action_result = Some(result.result.clone());
            state.results.push(SearchResult::QuickAction(result));
        }
    }

    // Search applications
    let mut app_results: Vec<_> = state.applications.iter()
        .filter(|app| !app.no_display)
        .filter_map(|app| {
            let score = calculate_match_score(app, &query_lower, state.config.fuzzy_search);
            if score > 0 {
                Some(AppResult {
                    app_id: app.id.clone(),
                    name: app.name.clone(),
                    description: app.comment.clone(),
                    icon: app.icon.clone(),
                    exec: app.exec.clone(),
                    score,
                })
            } else {
                None
            }
        })
        .collect();

    // Sort by score
    app_results.sort_by(|a, b| b.score.cmp(&a.score));

    // Take top results
    for result in app_results.into_iter().take(state.config.max_results) {
        state.results.push(SearchResult::Application(result));
    }

    Ok(state.results.clone())
}

/// Calculate match score for an application
fn calculate_match_score(app: &Application, query: &str, fuzzy: bool) -> u32 {
    let name_lower = app.name.to_lowercase();

    // Exact match
    if name_lower == query {
        return 1000;
    }

    // Starts with query
    if name_lower.starts_with(query) {
        return 900;
    }

    // Contains query
    if name_lower.contains(query) {
        return 700;
    }

    // Check generic name
    if let Some(ref generic) = app.generic_name {
        let generic_lower = generic.to_lowercase();
        if generic_lower.contains(query) {
            return 600;
        }
    }

    // Check keywords
    for keyword in &app.keywords {
        if keyword.to_lowercase().contains(query) {
            return 500;
        }
    }

    // Check comment/description
    if let Some(ref comment) = app.comment {
        if comment.to_lowercase().contains(query) {
            return 300;
        }
    }

    // Fuzzy matching
    if fuzzy {
        let score = fuzzy_match(&name_lower, query);
        if score > 0 {
            return score;
        }
    }

    0
}

/// Simple fuzzy matching
fn fuzzy_match(text: &str, query: &str) -> u32 {
    let mut query_chars = query.chars().peekable();
    let mut matched = 0;
    let mut consecutive = 0;
    let mut max_consecutive = 0;

    for c in text.chars() {
        if let Some(&qc) = query_chars.peek() {
            if c == qc {
                query_chars.next();
                matched += 1;
                consecutive += 1;
                if consecutive > max_consecutive {
                    max_consecutive = consecutive;
                }
            } else {
                consecutive = 0;
            }
        }
    }

    if query_chars.peek().is_none() {
        // All query characters matched
        let base_score = 100;
        let length_bonus = (text.len() as i32 - query.len() as i32).abs() as u32;
        let consecutive_bonus = max_consecutive * 10;

        base_score + consecutive_bonus - length_bonus.min(50)
    } else {
        0
    }
}

/// Try to execute a quick action
fn try_quick_action(query: &str) -> Option<QuickActionResult> {
    // Calculator: detect math expressions
    if query.chars().all(|c| c.is_ascii_digit() || "+-*/().^ ".contains(c)) && query.len() > 1 {
        if let Some(result) = evaluate_expression(query) {
            return Some(QuickActionResult {
                action_type: QuickActionType::Calculator,
                query: query.to_string(),
                result: format!("= {}", result),
                copyable: true,
            });
        }
    }

    // Color preview: detect hex colors
    if (query.starts_with('#') || query.starts_with("0x")) && query.len() >= 4 {
        let hex = query.trim_start_matches('#').trim_start_matches("0x");
        if hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Some(QuickActionResult {
                action_type: QuickActionType::ColorPreview,
                query: query.to_string(),
                result: format!("Color: #{}", hex.to_uppercase()),
                copyable: true,
            });
        }
    }

    None
}

/// Simple expression evaluator
fn evaluate_expression(expr: &str) -> Option<f64> {
    let expr = expr.replace(' ', "");

    // Very basic: just handle simple operations
    // In a real implementation, would use a proper parser

    // Try to parse as a simple number
    if let Ok(n) = expr.parse::<f64>() {
        return Some(n);
    }

    // Try simple addition
    if let Some(pos) = expr.rfind('+') {
        if pos > 0 {
            let left = evaluate_expression(&expr[..pos])?;
            let right = evaluate_expression(&expr[pos + 1..])?;
            return Some(left + right);
        }
    }

    // Try simple subtraction
    if let Some(pos) = expr.rfind('-') {
        if pos > 0 {
            let left = evaluate_expression(&expr[..pos])?;
            let right = evaluate_expression(&expr[pos + 1..])?;
            return Some(left - right);
        }
    }

    // Try simple multiplication
    if let Some(pos) = expr.rfind('*') {
        let left = evaluate_expression(&expr[..pos])?;
        let right = evaluate_expression(&expr[pos + 1..])?;
        return Some(left * right);
    }

    // Try simple division
    if let Some(pos) = expr.rfind('/') {
        let left = evaluate_expression(&expr[..pos])?;
        let right = evaluate_expression(&expr[pos + 1..])?;
        if right != 0.0 {
            return Some(left / right);
        }
    }

    None
}

/// Select next result
pub fn select_next() -> Result<usize, LauncherError> {
    let mut state = LAUNCHER_STATE.lock();
    let state = state.as_mut().ok_or(LauncherError::NotInitialized)?;

    if !state.results.is_empty() {
        state.selected_index = (state.selected_index + 1) % state.results.len();
    }

    Ok(state.selected_index)
}

/// Select previous result
pub fn select_previous() -> Result<usize, LauncherError> {
    let mut state = LAUNCHER_STATE.lock();
    let state = state.as_mut().ok_or(LauncherError::NotInitialized)?;

    if !state.results.is_empty() {
        if state.selected_index == 0 {
            state.selected_index = state.results.len() - 1;
        } else {
            state.selected_index -= 1;
        }
    }

    Ok(state.selected_index)
}

/// Activate selected result
pub fn activate_selected() -> Result<Option<String>, LauncherError> {
    let mut state = LAUNCHER_STATE.lock();
    let state = state.as_mut().ok_or(LauncherError::NotInitialized)?;

    if state.results.is_empty() {
        return Ok(None);
    }

    let result = &state.results[state.selected_index];

    match result {
        SearchResult::Application(app) => {
            // Add to recent
            state.recent_apps.retain(|id| id != &app.app_id);
            state.recent_apps.insert(0, app.app_id.clone());
            if state.recent_apps.len() > 20 {
                state.recent_apps.pop();
            }

            // Increment launch count
            if let Some(a) = state.applications.iter_mut().find(|a| a.id == app.app_id) {
                a.launch_count += 1;
            }

            state.visible = false;
            Ok(Some(app.exec.clone()))
        }
        SearchResult::File(file) => {
            state.visible = false;
            Ok(Some(file.path.clone()))
        }
        SearchResult::QuickAction(action) => {
            // Copy result to clipboard would happen here
            Ok(Some(action.result.clone()))
        }
        SearchResult::WebSearch(query) => {
            state.visible = false;
            Ok(Some(format!("https://search.example.com?q={}", query)))
        }
    }
}

/// Get current state info
pub fn get_state() -> Result<LauncherStateInfo, LauncherError> {
    let state = LAUNCHER_STATE.lock();
    let state = state.as_ref().ok_or(LauncherError::NotInitialized)?;

    Ok(LauncherStateInfo {
        visible: state.visible,
        query: state.search_query.clone(),
        result_count: state.results.len(),
        selected_index: state.selected_index,
        quick_action_result: state.quick_action_result.clone(),
    })
}

/// State info for external use
#[derive(Debug, Clone)]
pub struct LauncherStateInfo {
    pub visible: bool,
    pub query: String,
    pub result_count: usize,
    pub selected_index: usize,
    pub quick_action_result: Option<String>,
}

/// Get all registered applications
pub fn get_applications() -> Result<Vec<Application>, LauncherError> {
    let state = LAUNCHER_STATE.lock();
    let state = state.as_ref().ok_or(LauncherError::NotInitialized)?;

    Ok(state.applications.clone())
}

/// Get applications by category
pub fn get_applications_by_category(category: &str) -> Result<Vec<Application>, LauncherError> {
    let state = LAUNCHER_STATE.lock();
    let state = state.as_ref().ok_or(LauncherError::NotInitialized)?;

    let apps = state.applications.iter()
        .filter(|app| !app.no_display && app.categories.iter().any(|c| c.to_lowercase() == category.to_lowercase()))
        .cloned()
        .collect();

    Ok(apps)
}

/// Get all categories
pub fn get_categories() -> Result<Vec<AppCategory>, LauncherError> {
    let state = LAUNCHER_STATE.lock();
    let state = state.as_ref().ok_or(LauncherError::NotInitialized)?;

    Ok(state.categories.clone())
}

/// Check if launcher is visible
pub fn is_visible() -> bool {
    LAUNCHER_STATE
        .lock()
        .as_ref()
        .map(|s| s.visible)
        .unwrap_or(false)
}
