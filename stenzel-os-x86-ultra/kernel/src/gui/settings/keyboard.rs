//! Keyboard Settings
//!
//! Keyboard layout, input sources, shortcuts, and typing settings.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use spin::Mutex;

/// Global keyboard settings state
static KEYBOARD_SETTINGS: Mutex<Option<KeyboardSettings>> = Mutex::new(None);

/// Keyboard settings state
pub struct KeyboardSettings {
    /// Active keyboard layouts
    pub layouts: Vec<KeyboardLayout>,
    /// Current layout index
    pub current_layout: usize,
    /// Key repeat delay (ms)
    pub repeat_delay: u32,
    /// Key repeat rate (chars/sec)
    pub repeat_rate: u32,
    /// Cursor blink rate (ms)
    pub cursor_blink_rate: u32,
    /// Shortcuts
    pub shortcuts: Vec<Shortcut>,
    /// Caps Lock behavior
    pub caps_lock_behavior: CapsLockBehavior,
    /// Num Lock on login
    pub num_lock_on_login: bool,
    /// Input method
    pub input_method: Option<String>,
    /// Compose key
    pub compose_key: Option<ComposeKey>,
    /// Accessibility: Sticky keys
    pub sticky_keys: bool,
    /// Accessibility: Slow keys
    pub slow_keys: bool,
    /// Accessibility: Slow keys delay (ms)
    pub slow_keys_delay: u32,
    /// Accessibility: Bounce keys
    pub bounce_keys: bool,
    /// Accessibility: Bounce keys delay (ms)
    pub bounce_keys_delay: u32,
}

/// Keyboard layout
#[derive(Debug, Clone)]
pub struct KeyboardLayout {
    /// Layout code (e.g., "us", "de", "fr")
    pub code: String,
    /// Variant (e.g., "intl", "dvorak")
    pub variant: Option<String>,
    /// Display name
    pub name: String,
    /// Short name (for indicator)
    pub short_name: String,
}

impl KeyboardLayout {
    /// Create common layouts
    pub fn us() -> Self {
        KeyboardLayout {
            code: "us".to_string(),
            variant: None,
            name: "English (US)".to_string(),
            short_name: "EN".to_string(),
        }
    }

    pub fn us_intl() -> Self {
        KeyboardLayout {
            code: "us".to_string(),
            variant: Some("intl".to_string()),
            name: "English (US, International)".to_string(),
            short_name: "EN".to_string(),
        }
    }

    pub fn de() -> Self {
        KeyboardLayout {
            code: "de".to_string(),
            variant: None,
            name: "German".to_string(),
            short_name: "DE".to_string(),
        }
    }

    pub fn fr() -> Self {
        KeyboardLayout {
            code: "fr".to_string(),
            variant: None,
            name: "French".to_string(),
            short_name: "FR".to_string(),
        }
    }

    pub fn pt_br() -> Self {
        KeyboardLayout {
            code: "br".to_string(),
            variant: None,
            name: "Portuguese (Brazil)".to_string(),
            short_name: "PT".to_string(),
        }
    }

    pub fn dvorak() -> Self {
        KeyboardLayout {
            code: "us".to_string(),
            variant: Some("dvorak".to_string()),
            name: "English (Dvorak)".to_string(),
            short_name: "DV".to_string(),
        }
    }
}

/// Keyboard shortcut
#[derive(Debug, Clone)]
pub struct Shortcut {
    /// Action ID
    pub action: ShortcutAction,
    /// Key binding
    pub binding: KeyBinding,
    /// Is user-customized
    pub customized: bool,
}

/// Shortcut action
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutAction {
    // Window management
    CloseWindow,
    MinimizeWindow,
    MaximizeWindow,
    FullscreenWindow,
    MoveWindowLeft,
    MoveWindowRight,
    MoveWindowUp,
    MoveWindowDown,
    SwitchWindow,
    SwitchWindowReverse,

    // Workspaces
    SwitchWorkspace1,
    SwitchWorkspace2,
    SwitchWorkspace3,
    SwitchWorkspace4,
    MoveToWorkspace1,
    MoveToWorkspace2,
    MoveToWorkspace3,
    MoveToWorkspace4,
    NextWorkspace,
    PrevWorkspace,

    // System
    LockScreen,
    Logout,
    Settings,
    ScreenshotFull,
    ScreenshotWindow,
    ScreenshotArea,

    // Applications
    LaunchTerminal,
    LaunchBrowser,
    LaunchFileManager,
    LaunchAppMenu,

    // Audio
    VolumeUp,
    VolumeDown,
    VolumeMute,
    MicMute,

    // Media
    MediaPlay,
    MediaPause,
    MediaNext,
    MediaPrev,

    // Brightness
    BrightnessUp,
    BrightnessDown,

    // Custom
    Custom(u32),
}

impl ShortcutAction {
    pub fn name(&self) -> &'static str {
        match self {
            ShortcutAction::CloseWindow => "Close Window",
            ShortcutAction::MinimizeWindow => "Minimize Window",
            ShortcutAction::MaximizeWindow => "Maximize Window",
            ShortcutAction::FullscreenWindow => "Fullscreen Window",
            ShortcutAction::MoveWindowLeft => "Move Window Left",
            ShortcutAction::MoveWindowRight => "Move Window Right",
            ShortcutAction::MoveWindowUp => "Move Window Up",
            ShortcutAction::MoveWindowDown => "Move Window Down",
            ShortcutAction::SwitchWindow => "Switch Window",
            ShortcutAction::SwitchWindowReverse => "Switch Window (Reverse)",
            ShortcutAction::SwitchWorkspace1 => "Switch to Workspace 1",
            ShortcutAction::SwitchWorkspace2 => "Switch to Workspace 2",
            ShortcutAction::SwitchWorkspace3 => "Switch to Workspace 3",
            ShortcutAction::SwitchWorkspace4 => "Switch to Workspace 4",
            ShortcutAction::MoveToWorkspace1 => "Move to Workspace 1",
            ShortcutAction::MoveToWorkspace2 => "Move to Workspace 2",
            ShortcutAction::MoveToWorkspace3 => "Move to Workspace 3",
            ShortcutAction::MoveToWorkspace4 => "Move to Workspace 4",
            ShortcutAction::NextWorkspace => "Next Workspace",
            ShortcutAction::PrevWorkspace => "Previous Workspace",
            ShortcutAction::LockScreen => "Lock Screen",
            ShortcutAction::Logout => "Log Out",
            ShortcutAction::Settings => "Open Settings",
            ShortcutAction::ScreenshotFull => "Screenshot (Full)",
            ShortcutAction::ScreenshotWindow => "Screenshot (Window)",
            ShortcutAction::ScreenshotArea => "Screenshot (Area)",
            ShortcutAction::LaunchTerminal => "Launch Terminal",
            ShortcutAction::LaunchBrowser => "Launch Browser",
            ShortcutAction::LaunchFileManager => "Launch File Manager",
            ShortcutAction::LaunchAppMenu => "Launch App Menu",
            ShortcutAction::VolumeUp => "Volume Up",
            ShortcutAction::VolumeDown => "Volume Down",
            ShortcutAction::VolumeMute => "Volume Mute",
            ShortcutAction::MicMute => "Microphone Mute",
            ShortcutAction::MediaPlay => "Media Play",
            ShortcutAction::MediaPause => "Media Pause",
            ShortcutAction::MediaNext => "Media Next",
            ShortcutAction::MediaPrev => "Media Previous",
            ShortcutAction::BrightnessUp => "Brightness Up",
            ShortcutAction::BrightnessDown => "Brightness Down",
            ShortcutAction::Custom(_) => "Custom Action",
        }
    }
}

/// Key binding
#[derive(Debug, Clone)]
pub struct KeyBinding {
    /// Modifiers
    pub modifiers: Modifiers,
    /// Key code
    pub key: u32,
    /// Key name (for display)
    pub key_name: String,
}

/// Key modifiers
#[derive(Debug, Clone, Copy, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub super_key: bool,
}

impl Modifiers {
    pub fn to_string(&self) -> String {
        let mut parts = Vec::new();
        if self.super_key { parts.push("Super"); }
        if self.ctrl { parts.push("Ctrl"); }
        if self.alt { parts.push("Alt"); }
        if self.shift { parts.push("Shift"); }

        let mut result = String::new();
        for (i, part) in parts.iter().enumerate() {
            if i > 0 {
                result.push_str(" + ");
            }
            result.push_str(part);
        }
        result
    }
}

/// Caps Lock behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapsLockBehavior {
    /// Default Caps Lock
    Default,
    /// Act as Escape
    Escape,
    /// Act as Ctrl
    Ctrl,
    /// Act as Backspace
    Backspace,
    /// Disabled
    Disabled,
}

impl CapsLockBehavior {
    pub fn name(&self) -> &'static str {
        match self {
            CapsLockBehavior::Default => "Default",
            CapsLockBehavior::Escape => "Escape",
            CapsLockBehavior::Ctrl => "Ctrl",
            CapsLockBehavior::Backspace => "Backspace",
            CapsLockBehavior::Disabled => "Disabled",
        }
    }
}

/// Compose key
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposeKey {
    RightAlt,
    LeftCtrl,
    RightCtrl,
    CapsLock,
    Menu,
    Disabled,
}

impl ComposeKey {
    pub fn name(&self) -> &'static str {
        match self {
            ComposeKey::RightAlt => "Right Alt",
            ComposeKey::LeftCtrl => "Left Ctrl",
            ComposeKey::RightCtrl => "Right Ctrl",
            ComposeKey::CapsLock => "Caps Lock",
            ComposeKey::Menu => "Menu Key",
            ComposeKey::Disabled => "Disabled",
        }
    }
}

/// Initialize keyboard settings
pub fn init() {
    let mut state = KEYBOARD_SETTINGS.lock();
    if state.is_some() {
        return;
    }

    *state = Some(KeyboardSettings {
        layouts: vec![KeyboardLayout::us()],
        current_layout: 0,
        repeat_delay: 500,
        repeat_rate: 30,
        cursor_blink_rate: 530,
        shortcuts: get_default_shortcuts(),
        caps_lock_behavior: CapsLockBehavior::Default,
        num_lock_on_login: true,
        input_method: None,
        compose_key: Some(ComposeKey::RightAlt),
        sticky_keys: false,
        slow_keys: false,
        slow_keys_delay: 300,
        bounce_keys: false,
        bounce_keys_delay: 300,
    });

    crate::kprintln!("keyboard settings: initialized");
}

/// Get default shortcuts
fn get_default_shortcuts() -> Vec<Shortcut> {
    vec![
        Shortcut {
            action: ShortcutAction::CloseWindow,
            binding: KeyBinding {
                modifiers: Modifiers { alt: true, ..Default::default() },
                key: 0x46, // F4
                key_name: "F4".to_string(),
            },
            customized: false,
        },
        Shortcut {
            action: ShortcutAction::SwitchWindow,
            binding: KeyBinding {
                modifiers: Modifiers { alt: true, ..Default::default() },
                key: 0x09, // Tab
                key_name: "Tab".to_string(),
            },
            customized: false,
        },
        Shortcut {
            action: ShortcutAction::LaunchTerminal,
            binding: KeyBinding {
                modifiers: Modifiers { ctrl: true, alt: true, ..Default::default() },
                key: 0x14, // T
                key_name: "T".to_string(),
            },
            customized: false,
        },
        Shortcut {
            action: ShortcutAction::LockScreen,
            binding: KeyBinding {
                modifiers: Modifiers { super_key: true, ..Default::default() },
                key: 0x26, // L
                key_name: "L".to_string(),
            },
            customized: false,
        },
        Shortcut {
            action: ShortcutAction::ScreenshotFull,
            binding: KeyBinding {
                modifiers: Modifiers::default(),
                key: 0x6F, // Print Screen
                key_name: "Print".to_string(),
            },
            customized: false,
        },
    ]
}

/// Get current layout
pub fn get_current_layout() -> Option<KeyboardLayout> {
    let state = KEYBOARD_SETTINGS.lock();
    state.as_ref().and_then(|s| s.layouts.get(s.current_layout).cloned())
}

/// Get all layouts
pub fn get_layouts() -> Vec<KeyboardLayout> {
    let state = KEYBOARD_SETTINGS.lock();
    state.as_ref().map(|s| s.layouts.clone()).unwrap_or_default()
}

/// Add layout
pub fn add_layout(layout: KeyboardLayout) {
    let mut state = KEYBOARD_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.layouts.push(layout);
    }
}

/// Remove layout
pub fn remove_layout(index: usize) -> Result<(), KeyboardError> {
    let mut state = KEYBOARD_SETTINGS.lock();
    let state = state.as_mut().ok_or(KeyboardError::NotInitialized)?;

    if state.layouts.len() <= 1 {
        return Err(KeyboardError::CannotRemoveLastLayout);
    }

    if index >= state.layouts.len() {
        return Err(KeyboardError::LayoutNotFound);
    }

    state.layouts.remove(index);
    if state.current_layout >= state.layouts.len() {
        state.current_layout = state.layouts.len() - 1;
    }

    Ok(())
}

/// Switch to next layout
pub fn next_layout() {
    let mut state = KEYBOARD_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.current_layout = (s.current_layout + 1) % s.layouts.len();
    }
}

/// Set current layout
pub fn set_layout(index: usize) -> Result<(), KeyboardError> {
    let mut state = KEYBOARD_SETTINGS.lock();
    let state = state.as_mut().ok_or(KeyboardError::NotInitialized)?;

    if index >= state.layouts.len() {
        return Err(KeyboardError::LayoutNotFound);
    }

    state.current_layout = index;

    Ok(())
}

/// Set repeat delay
pub fn set_repeat_delay(ms: u32) {
    let mut state = KEYBOARD_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.repeat_delay = ms.clamp(100, 2000);
    }
}

/// Set repeat rate
pub fn set_repeat_rate(rate: u32) {
    let mut state = KEYBOARD_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.repeat_rate = rate.clamp(5, 100);
    }
}

/// Get shortcuts
pub fn get_shortcuts() -> Vec<Shortcut> {
    let state = KEYBOARD_SETTINGS.lock();
    state.as_ref().map(|s| s.shortcuts.clone()).unwrap_or_default()
}

/// Set shortcut binding
pub fn set_shortcut(action: ShortcutAction, binding: KeyBinding) {
    let mut state = KEYBOARD_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        if let Some(shortcut) = s.shortcuts.iter_mut().find(|sc| sc.action == action) {
            shortcut.binding = binding;
            shortcut.customized = true;
        } else {
            s.shortcuts.push(Shortcut {
                action,
                binding,
                customized: true,
            });
        }
    }
}

/// Reset shortcut to default
pub fn reset_shortcut(action: ShortcutAction) {
    let defaults = get_default_shortcuts();
    let mut state = KEYBOARD_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        if let Some(default) = defaults.iter().find(|sc| sc.action == action) {
            if let Some(shortcut) = s.shortcuts.iter_mut().find(|sc| sc.action == action) {
                *shortcut = default.clone();
            }
        }
    }
}

/// Set Caps Lock behavior
pub fn set_caps_lock_behavior(behavior: CapsLockBehavior) {
    let mut state = KEYBOARD_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.caps_lock_behavior = behavior;
    }
}

/// Set sticky keys
pub fn set_sticky_keys(enabled: bool) {
    let mut state = KEYBOARD_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.sticky_keys = enabled;
    }
}

/// Set slow keys
pub fn set_slow_keys(enabled: bool, delay: Option<u32>) {
    let mut state = KEYBOARD_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.slow_keys = enabled;
        if let Some(d) = delay {
            s.slow_keys_delay = d;
        }
    }
}

/// Set bounce keys
pub fn set_bounce_keys(enabled: bool, delay: Option<u32>) {
    let mut state = KEYBOARD_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.bounce_keys = enabled;
        if let Some(d) = delay {
            s.bounce_keys_delay = d;
        }
    }
}

/// Keyboard error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardError {
    NotInitialized,
    LayoutNotFound,
    CannotRemoveLastLayout,
    ShortcutConflict,
}
