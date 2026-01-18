//! Settings Application
//!
//! Provides system settings management with a unified interface for all settings panels.

pub mod display;
pub mod sound;
pub mod network;
pub mod bluetooth;
pub mod power;
pub mod keyboard;
pub mod mouse;
pub mod users;
pub mod datetime;
pub mod privacy;
pub mod defaults;
pub mod about;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use spin::Mutex;

/// Global settings state
static SETTINGS_STATE: Mutex<Option<SettingsState>> = Mutex::new(None);

/// Settings state
pub struct SettingsState {
    /// Current panel
    pub current_panel: SettingsPanel,
    /// Search query
    pub search_query: String,
    /// Search results
    pub search_results: Vec<SettingsItem>,
    /// Navigation history
    pub history: Vec<SettingsPanel>,
    /// Sidebar collapsed
    pub sidebar_collapsed: bool,
}

/// Settings panel
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsPanel {
    /// Display settings
    Display,
    /// Sound settings
    Sound,
    /// Network settings
    Network,
    /// Bluetooth settings
    Bluetooth,
    /// Power settings
    Power,
    /// Keyboard settings
    Keyboard,
    /// Mouse/Touchpad settings
    Mouse,
    /// Users & Accounts
    Users,
    /// Date & Time
    DateTime,
    /// Privacy settings
    Privacy,
    /// Default applications
    Defaults,
    /// About this computer
    About,
}

impl SettingsPanel {
    /// Get panel name
    pub fn name(&self) -> &'static str {
        match self {
            SettingsPanel::Display => "Display",
            SettingsPanel::Sound => "Sound",
            SettingsPanel::Network => "Network",
            SettingsPanel::Bluetooth => "Bluetooth",
            SettingsPanel::Power => "Power",
            SettingsPanel::Keyboard => "Keyboard",
            SettingsPanel::Mouse => "Mouse & Touchpad",
            SettingsPanel::Users => "Users & Accounts",
            SettingsPanel::DateTime => "Date & Time",
            SettingsPanel::Privacy => "Privacy",
            SettingsPanel::Defaults => "Default Apps",
            SettingsPanel::About => "About",
        }
    }

    /// Get panel icon
    pub fn icon(&self) -> &'static str {
        match self {
            SettingsPanel::Display => "video-display",
            SettingsPanel::Sound => "audio-speakers",
            SettingsPanel::Network => "network-wireless",
            SettingsPanel::Bluetooth => "bluetooth",
            SettingsPanel::Power => "battery",
            SettingsPanel::Keyboard => "input-keyboard",
            SettingsPanel::Mouse => "input-mouse",
            SettingsPanel::Users => "system-users",
            SettingsPanel::DateTime => "preferences-system-time",
            SettingsPanel::Privacy => "preferences-system-privacy",
            SettingsPanel::Defaults => "preferences-desktop-default-applications",
            SettingsPanel::About => "help-about",
        }
    }

    /// Get all panels in order
    pub fn all() -> &'static [SettingsPanel] {
        &[
            SettingsPanel::Network,
            SettingsPanel::Bluetooth,
            SettingsPanel::Display,
            SettingsPanel::Sound,
            SettingsPanel::Power,
            SettingsPanel::Keyboard,
            SettingsPanel::Mouse,
            SettingsPanel::Users,
            SettingsPanel::DateTime,
            SettingsPanel::Privacy,
            SettingsPanel::Defaults,
            SettingsPanel::About,
        ]
    }
}

/// Settings item (for search)
#[derive(Debug, Clone)]
pub struct SettingsItem {
    /// Item name
    pub name: String,
    /// Item description
    pub description: String,
    /// Parent panel
    pub panel: SettingsPanel,
    /// Keywords for search
    pub keywords: Vec<String>,
}

/// Initialize settings
pub fn init() {
    let mut state = SETTINGS_STATE.lock();
    if state.is_some() {
        return;
    }

    *state = Some(SettingsState {
        current_panel: SettingsPanel::Network,
        search_query: String::new(),
        search_results: Vec::new(),
        history: Vec::new(),
        sidebar_collapsed: false,
    });

    // Initialize all settings panels
    drop(state);
    display::init();
    sound::init();
    network::init();
    bluetooth::init();
    power::init();
    keyboard::init();
    mouse::init();
    users::init();
    datetime::init();
    privacy::init();
    defaults::init();
    about::init();

    crate::kprintln!("settings: initialized");
}

/// Navigate to panel
pub fn navigate_to(panel: SettingsPanel) {
    let mut state = SETTINGS_STATE.lock();
    if let Some(ref mut s) = *state {
        // Save to history
        if s.current_panel != panel {
            s.history.push(s.current_panel);
        }
        s.current_panel = panel;
    }
}

/// Navigate back
pub fn navigate_back() -> bool {
    let mut state = SETTINGS_STATE.lock();
    if let Some(ref mut s) = *state {
        if let Some(panel) = s.history.pop() {
            s.current_panel = panel;
            return true;
        }
    }
    false
}

/// Get current panel
pub fn get_current_panel() -> Option<SettingsPanel> {
    let state = SETTINGS_STATE.lock();
    state.as_ref().map(|s| s.current_panel)
}

/// Search settings
pub fn search(query: &str) -> Vec<SettingsItem> {
    let query_lower = query.to_lowercase();
    let mut results = Vec::new();

    // Build search index
    let items = get_all_settings_items();

    for item in items {
        let name_match = item.name.to_lowercase().contains(&query_lower);
        let desc_match = item.description.to_lowercase().contains(&query_lower);
        let keyword_match = item.keywords.iter().any(|k| k.to_lowercase().contains(&query_lower));

        if name_match || desc_match || keyword_match {
            results.push(item);
        }
    }

    results
}

/// Get all settings items for search
fn get_all_settings_items() -> Vec<SettingsItem> {
    let mut items = Vec::new();

    // Display items
    items.push(SettingsItem {
        name: "Resolution".to_string(),
        description: "Change screen resolution".to_string(),
        panel: SettingsPanel::Display,
        keywords: vec!["screen".to_string(), "display".to_string(), "monitor".to_string()],
    });
    items.push(SettingsItem {
        name: "Night Light".to_string(),
        description: "Reduce blue light at night".to_string(),
        panel: SettingsPanel::Display,
        keywords: vec!["blue".to_string(), "light".to_string(), "eye".to_string(), "night".to_string()],
    });
    items.push(SettingsItem {
        name: "Scale".to_string(),
        description: "Adjust text and UI scaling".to_string(),
        panel: SettingsPanel::Display,
        keywords: vec!["hidpi".to_string(), "retina".to_string(), "4k".to_string(), "scaling".to_string()],
    });

    // Sound items
    items.push(SettingsItem {
        name: "Volume".to_string(),
        description: "Adjust system volume".to_string(),
        panel: SettingsPanel::Sound,
        keywords: vec!["audio".to_string(), "speaker".to_string(), "loud".to_string()],
    });
    items.push(SettingsItem {
        name: "Output Device".to_string(),
        description: "Select audio output device".to_string(),
        panel: SettingsPanel::Sound,
        keywords: vec!["speaker".to_string(), "headphone".to_string(), "hdmi".to_string()],
    });
    items.push(SettingsItem {
        name: "Input Device".to_string(),
        description: "Select microphone".to_string(),
        panel: SettingsPanel::Sound,
        keywords: vec!["microphone".to_string(), "mic".to_string(), "recording".to_string()],
    });

    // Network items
    items.push(SettingsItem {
        name: "Wi-Fi".to_string(),
        description: "Connect to wireless networks".to_string(),
        panel: SettingsPanel::Network,
        keywords: vec!["wireless".to_string(), "wifi".to_string(), "internet".to_string()],
    });
    items.push(SettingsItem {
        name: "Ethernet".to_string(),
        description: "Wired network connection".to_string(),
        panel: SettingsPanel::Network,
        keywords: vec!["wired".to_string(), "lan".to_string(), "cable".to_string()],
    });
    items.push(SettingsItem {
        name: "VPN".to_string(),
        description: "Virtual private network".to_string(),
        panel: SettingsPanel::Network,
        keywords: vec!["vpn".to_string(), "tunnel".to_string(), "security".to_string()],
    });

    // Power items
    items.push(SettingsItem {
        name: "Battery".to_string(),
        description: "View battery status".to_string(),
        panel: SettingsPanel::Power,
        keywords: vec!["battery".to_string(), "charge".to_string(), "power".to_string()],
    });
    items.push(SettingsItem {
        name: "Power Saving".to_string(),
        description: "Configure power saving options".to_string(),
        panel: SettingsPanel::Power,
        keywords: vec!["battery".to_string(), "save".to_string(), "eco".to_string()],
    });

    // Keyboard items
    items.push(SettingsItem {
        name: "Keyboard Layout".to_string(),
        description: "Select keyboard layout".to_string(),
        panel: SettingsPanel::Keyboard,
        keywords: vec!["layout".to_string(), "language".to_string(), "input".to_string()],
    });
    items.push(SettingsItem {
        name: "Shortcuts".to_string(),
        description: "Configure keyboard shortcuts".to_string(),
        panel: SettingsPanel::Keyboard,
        keywords: vec!["hotkey".to_string(), "shortcut".to_string(), "binding".to_string()],
    });

    items
}

/// Toggle sidebar
pub fn toggle_sidebar() {
    let mut state = SETTINGS_STATE.lock();
    if let Some(ref mut s) = *state {
        s.sidebar_collapsed = !s.sidebar_collapsed;
    }
}
