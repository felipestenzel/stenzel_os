//! Voice Control Accessibility Feature
//!
//! Provides voice-based control for users who cannot use keyboard/mouse:
//! - Voice commands for system control
//! - Dictation for text input
//! - Voice navigation
//! - Custom command macros

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use alloc::collections::BTreeMap;

use crate::sync::IrqSafeMutex;

/// Voice command category
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandCategory {
    /// Navigation commands (scroll, go to, open)
    Navigation,
    /// Editing commands (select, copy, paste, delete)
    Editing,
    /// Window management (close, minimize, maximize)
    Window,
    /// System commands (sleep, shutdown, volume)
    System,
    /// Application control (start, stop, switch)
    Application,
    /// Dictation (text input)
    Dictation,
    /// Accessibility settings
    Accessibility,
    /// Custom user commands
    Custom,
}

impl CommandCategory {
    /// Get category name
    pub fn name(&self) -> &'static str {
        match self {
            CommandCategory::Navigation => "Navigation",
            CommandCategory::Editing => "Editing",
            CommandCategory::Window => "Window",
            CommandCategory::System => "System",
            CommandCategory::Application => "Application",
            CommandCategory::Dictation => "Dictation",
            CommandCategory::Accessibility => "Accessibility",
            CommandCategory::Custom => "Custom",
        }
    }
}

/// Built-in voice command
#[derive(Debug, Clone)]
pub struct VoiceCommand {
    /// Command ID
    pub id: u32,
    /// Trigger phrases (any of these will activate the command)
    pub phrases: Vec<String>,
    /// Category
    pub category: CommandCategory,
    /// Description
    pub description: String,
    /// Action to perform
    pub action: CommandAction,
    /// Whether command is enabled
    pub enabled: bool,
    /// Number of times used
    pub use_count: u64,
}

impl VoiceCommand {
    /// Create a new voice command
    pub fn new(id: u32, category: CommandCategory, phrases: &[&str], description: &str, action: CommandAction) -> Self {
        Self {
            id,
            phrases: phrases.iter().map(|s| String::from(*s)).collect(),
            category,
            description: String::from(description),
            action,
            enabled: true,
            use_count: 0,
        }
    }

    /// Check if phrase matches this command
    pub fn matches(&self, phrase: &str) -> bool {
        let phrase_lower = phrase.to_lowercase();
        self.phrases.iter().any(|p| {
            let p_lower = p.to_lowercase();
            phrase_lower.starts_with(&p_lower) || p_lower.starts_with(&phrase_lower)
        })
    }
}

/// Trait for lowercase conversion
trait ToLowercase {
    fn to_lowercase(&self) -> String;
}

impl ToLowercase for str {
    fn to_lowercase(&self) -> String {
        let mut result = String::with_capacity(self.len());
        for c in self.chars() {
            if c >= 'A' && c <= 'Z' {
                result.push((c as u8 + 32) as char);
            } else {
                result.push(c);
            }
        }
        result
    }
}

/// Command action type
#[derive(Debug, Clone)]
pub enum CommandAction {
    /// Type text
    TypeText(String),
    /// Press key(s)
    PressKey(Vec<u8>),
    /// Mouse click at position
    MouseClick { x: i32, y: i32, button: u8 },
    /// Scroll
    Scroll { dx: i32, dy: i32 },
    /// Execute system command
    SystemCommand(SystemCommand),
    /// Navigate UI element
    Navigate(NavigationTarget),
    /// Window action
    WindowAction(WindowAction),
    /// Custom callback ID
    CustomCallback(u32),
    /// Open application
    OpenApp(String),
    /// Set accessibility option
    AccessibilityOption(AccessibilityAction),
    /// Macro (sequence of actions)
    Macro(Vec<CommandAction>),
    /// No action (used for dictation mode toggle)
    None,
}

/// System command types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemCommand {
    /// Sleep the system
    Sleep,
    /// Shutdown
    Shutdown,
    /// Restart
    Restart,
    /// Lock screen
    Lock,
    /// Increase volume
    VolumeUp,
    /// Decrease volume
    VolumeDown,
    /// Mute/unmute
    VolumeMute,
    /// Increase brightness
    BrightnessUp,
    /// Decrease brightness
    BrightnessDown,
    /// Take screenshot
    Screenshot,
    /// Open file manager
    OpenFileManager,
    /// Open settings
    OpenSettings,
    /// Open terminal
    OpenTerminal,
    /// Open browser
    OpenBrowser,
}

/// Navigation target
#[derive(Debug, Clone)]
pub enum NavigationTarget {
    /// Next element
    Next,
    /// Previous element
    Previous,
    /// Go to specific element by name
    Element(String),
    /// Go to heading
    Heading,
    /// Go to link
    Link,
    /// Go to button
    Button,
    /// Go to text field
    TextField,
    /// Page up
    PageUp,
    /// Page down
    PageDown,
    /// Top of page
    Top,
    /// Bottom of page
    Bottom,
    /// Go back
    Back,
    /// Go forward
    Forward,
}

/// Window action
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowAction {
    /// Close window
    Close,
    /// Minimize window
    Minimize,
    /// Maximize window
    Maximize,
    /// Restore window
    Restore,
    /// Move window
    Move,
    /// Resize window
    Resize,
    /// Switch to next window
    NextWindow,
    /// Switch to previous window
    PrevWindow,
    /// Show all windows
    ShowAll,
}

/// Accessibility action
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessibilityAction {
    /// Toggle screen reader
    ToggleScreenReader,
    /// Toggle high contrast
    ToggleHighContrast,
    /// Toggle magnifier
    ToggleMagnifier,
    /// Zoom in
    ZoomIn,
    /// Zoom out
    ZoomOut,
    /// Toggle voice control
    ToggleVoiceControl,
    /// Toggle reduce motion
    ToggleReduceMotion,
}

/// Recognition result
#[derive(Debug, Clone)]
pub struct RecognitionResult {
    /// Recognized text
    pub text: String,
    /// Confidence score (0.0-1.0)
    pub confidence: f32,
    /// Alternative interpretations
    pub alternatives: Vec<(String, f32)>,
    /// Timestamp
    pub timestamp_ms: u64,
    /// Whether this is a final result
    pub is_final: bool,
}

/// Voice control state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceControlState {
    /// Not listening
    Idle,
    /// Listening for wake word
    WaitingForWakeWord,
    /// Listening for command
    Listening,
    /// Processing command
    Processing,
    /// In dictation mode
    Dictating,
    /// Error state
    Error,
}

impl VoiceControlState {
    /// Get state name
    pub fn name(&self) -> &'static str {
        match self {
            VoiceControlState::Idle => "Idle",
            VoiceControlState::WaitingForWakeWord => "Waiting for Wake Word",
            VoiceControlState::Listening => "Listening",
            VoiceControlState::Processing => "Processing",
            VoiceControlState::Dictating => "Dictating",
            VoiceControlState::Error => "Error",
        }
    }
}

/// Voice control configuration
#[derive(Debug, Clone)]
pub struct VoiceControlConfig {
    /// Whether voice control is enabled
    pub enabled: bool,
    /// Wake word (e.g., "Hey Stenzel")
    pub wake_word: String,
    /// Use wake word (vs always listening)
    pub use_wake_word: bool,
    /// Timeout for listening (ms)
    pub listen_timeout_ms: u32,
    /// Language/locale
    pub language: String,
    /// Play sound on recognition
    pub sound_on_recognition: bool,
    /// Play sound on error
    pub sound_on_error: bool,
    /// Show visual feedback
    pub visual_feedback: bool,
    /// Continuous dictation mode
    pub continuous_dictation: bool,
    /// Auto-punctuation in dictation
    pub auto_punctuation: bool,
    /// Profanity filter
    pub profanity_filter: bool,
    /// Minimum confidence threshold
    pub min_confidence: f32,
    /// Commands enabled by category
    pub enabled_categories: Vec<CommandCategory>,
}

impl Default for VoiceControlConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            wake_word: String::from("Hey Stenzel"),
            use_wake_word: true,
            listen_timeout_ms: 5000,
            language: String::from("en-US"),
            sound_on_recognition: true,
            sound_on_error: true,
            visual_feedback: true,
            continuous_dictation: false,
            auto_punctuation: true,
            profanity_filter: false,
            min_confidence: 0.6,
            enabled_categories: vec![
                CommandCategory::Navigation,
                CommandCategory::Editing,
                CommandCategory::Window,
                CommandCategory::System,
                CommandCategory::Application,
                CommandCategory::Dictation,
                CommandCategory::Accessibility,
            ],
        }
    }
}

/// Voice control statistics
#[derive(Debug, Clone, Default)]
pub struct VoiceControlStats {
    /// Commands recognized
    pub commands_recognized: u64,
    /// Commands executed
    pub commands_executed: u64,
    /// Recognition errors
    pub recognition_errors: u64,
    /// Dictation words
    pub dictation_words: u64,
    /// Wake word detections
    pub wake_word_detections: u64,
    /// Total listening time (ms)
    pub total_listen_time_ms: u64,
    /// Session start timestamp
    pub session_start_ms: u64,
}

/// Voice Control Manager
pub struct VoiceControl {
    /// Configuration
    config: VoiceControlConfig,
    /// Current state
    state: VoiceControlState,
    /// Built-in commands
    commands: Vec<VoiceCommand>,
    /// Custom commands
    custom_commands: BTreeMap<u32, VoiceCommand>,
    /// Next custom command ID
    next_custom_id: u32,
    /// Current dictation buffer
    dictation_buffer: String,
    /// Statistics
    stats: VoiceControlStats,
    /// Last recognition result
    last_result: Option<RecognitionResult>,
    /// Recognition callback
    on_recognition: Option<fn(&RecognitionResult)>,
    /// Command execution callback
    on_command: Option<fn(&VoiceCommand)>,
    /// Audio input callback (for integration)
    on_audio_input: Option<fn(&[i16]) -> Option<RecognitionResult>>,
    /// Listen start time
    listen_start_ms: u64,
}

impl VoiceControl {
    /// Create a new voice control manager
    pub fn new() -> Self {
        Self {
            config: VoiceControlConfig::default(),
            state: VoiceControlState::Idle,
            commands: Vec::new(),
            custom_commands: BTreeMap::new(),
            next_custom_id: 1000,
            dictation_buffer: String::new(),
            stats: VoiceControlStats::default(),
            last_result: None,
            on_recognition: None,
            on_command: None,
            on_audio_input: None,
            listen_start_ms: 0,
        }
    }

    /// Initialize voice control
    pub fn init(&mut self) {
        self.stats.session_start_ms = crate::time::uptime_ms();
        self.load_default_commands();
        crate::kprintln!("[voice_control] Voice control initialized with {} commands",
            self.commands.len());
    }

    /// Load default commands
    fn load_default_commands(&mut self) {
        let mut id = 0u32;

        // Navigation commands
        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::Navigation,
            &["scroll down", "page down"],
            "Scroll down the page",
            CommandAction::Scroll { dx: 0, dy: 100 },
        ));

        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::Navigation,
            &["scroll up", "page up"],
            "Scroll up the page",
            CommandAction::Scroll { dx: 0, dy: -100 },
        ));

        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::Navigation,
            &["go to top", "top of page"],
            "Go to top of page",
            CommandAction::Navigate(NavigationTarget::Top),
        ));

        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::Navigation,
            &["go to bottom", "bottom of page"],
            "Go to bottom of page",
            CommandAction::Navigate(NavigationTarget::Bottom),
        ));

        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::Navigation,
            &["next", "next item"],
            "Go to next item",
            CommandAction::Navigate(NavigationTarget::Next),
        ));

        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::Navigation,
            &["previous", "previous item"],
            "Go to previous item",
            CommandAction::Navigate(NavigationTarget::Previous),
        ));

        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::Navigation,
            &["go back", "back"],
            "Navigate back",
            CommandAction::Navigate(NavigationTarget::Back),
        ));

        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::Navigation,
            &["go forward", "forward"],
            "Navigate forward",
            CommandAction::Navigate(NavigationTarget::Forward),
        ));

        // Editing commands
        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::Editing,
            &["select all"],
            "Select all text",
            CommandAction::PressKey(vec![0x1D, 0x1E]), // Ctrl+A
        ));

        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::Editing,
            &["copy", "copy that"],
            "Copy selection",
            CommandAction::PressKey(vec![0x1D, 0x2E]), // Ctrl+C
        ));

        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::Editing,
            &["paste"],
            "Paste from clipboard",
            CommandAction::PressKey(vec![0x1D, 0x2F]), // Ctrl+V
        ));

        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::Editing,
            &["cut"],
            "Cut selection",
            CommandAction::PressKey(vec![0x1D, 0x2D]), // Ctrl+X
        ));

        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::Editing,
            &["undo"],
            "Undo last action",
            CommandAction::PressKey(vec![0x1D, 0x2C]), // Ctrl+Z
        ));

        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::Editing,
            &["redo"],
            "Redo last action",
            CommandAction::PressKey(vec![0x1D, 0x15]), // Ctrl+Y
        ));

        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::Editing,
            &["delete", "delete that"],
            "Delete selection",
            CommandAction::PressKey(vec![0x53]), // Delete key
        ));

        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::Editing,
            &["new line", "enter"],
            "Insert new line",
            CommandAction::PressKey(vec![0x1C]), // Enter
        ));

        // Window commands
        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::Window,
            &["close window", "close"],
            "Close current window",
            CommandAction::WindowAction(WindowAction::Close),
        ));

        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::Window,
            &["minimize window", "minimize"],
            "Minimize current window",
            CommandAction::WindowAction(WindowAction::Minimize),
        ));

        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::Window,
            &["maximize window", "maximize"],
            "Maximize current window",
            CommandAction::WindowAction(WindowAction::Maximize),
        ));

        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::Window,
            &["next window", "switch window"],
            "Switch to next window",
            CommandAction::WindowAction(WindowAction::NextWindow),
        ));

        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::Window,
            &["show all windows", "show windows"],
            "Show all windows",
            CommandAction::WindowAction(WindowAction::ShowAll),
        ));

        // System commands
        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::System,
            &["lock screen", "lock computer"],
            "Lock the screen",
            CommandAction::SystemCommand(SystemCommand::Lock),
        ));

        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::System,
            &["volume up", "louder"],
            "Increase volume",
            CommandAction::SystemCommand(SystemCommand::VolumeUp),
        ));

        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::System,
            &["volume down", "quieter"],
            "Decrease volume",
            CommandAction::SystemCommand(SystemCommand::VolumeDown),
        ));

        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::System,
            &["mute", "unmute"],
            "Toggle mute",
            CommandAction::SystemCommand(SystemCommand::VolumeMute),
        ));

        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::System,
            &["brightness up", "brighter"],
            "Increase brightness",
            CommandAction::SystemCommand(SystemCommand::BrightnessUp),
        ));

        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::System,
            &["brightness down", "dimmer"],
            "Decrease brightness",
            CommandAction::SystemCommand(SystemCommand::BrightnessDown),
        ));

        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::System,
            &["take screenshot", "screenshot"],
            "Take a screenshot",
            CommandAction::SystemCommand(SystemCommand::Screenshot),
        ));

        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::System,
            &["open settings", "settings"],
            "Open system settings",
            CommandAction::SystemCommand(SystemCommand::OpenSettings),
        ));

        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::System,
            &["open terminal", "terminal"],
            "Open terminal",
            CommandAction::SystemCommand(SystemCommand::OpenTerminal),
        ));

        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::System,
            &["open browser", "browser"],
            "Open web browser",
            CommandAction::SystemCommand(SystemCommand::OpenBrowser),
        ));

        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::System,
            &["open files", "file manager"],
            "Open file manager",
            CommandAction::SystemCommand(SystemCommand::OpenFileManager),
        ));

        // Dictation commands
        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::Dictation,
            &["start dictation", "dictate"],
            "Start dictation mode",
            CommandAction::None,
        ));

        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::Dictation,
            &["stop dictation", "stop listening"],
            "Stop dictation mode",
            CommandAction::None,
        ));

        // Accessibility commands
        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::Accessibility,
            &["toggle screen reader", "screen reader"],
            "Toggle screen reader",
            CommandAction::AccessibilityOption(AccessibilityAction::ToggleScreenReader),
        ));

        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::Accessibility,
            &["toggle high contrast", "high contrast"],
            "Toggle high contrast mode",
            CommandAction::AccessibilityOption(AccessibilityAction::ToggleHighContrast),
        ));

        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::Accessibility,
            &["toggle magnifier", "magnifier"],
            "Toggle screen magnifier",
            CommandAction::AccessibilityOption(AccessibilityAction::ToggleMagnifier),
        ));

        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::Accessibility,
            &["zoom in"],
            "Zoom in",
            CommandAction::AccessibilityOption(AccessibilityAction::ZoomIn),
        ));

        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::Accessibility,
            &["zoom out"],
            "Zoom out",
            CommandAction::AccessibilityOption(AccessibilityAction::ZoomOut),
        ));

        self.commands.push(VoiceCommand::new(
            { id += 1; id },
            CommandCategory::Accessibility,
            &["reduce motion"],
            "Toggle reduce motion",
            CommandAction::AccessibilityOption(AccessibilityAction::ToggleReduceMotion),
        ));
    }

    /// Enable voice control
    pub fn enable(&mut self) {
        self.config.enabled = true;
        self.state = if self.config.use_wake_word {
            VoiceControlState::WaitingForWakeWord
        } else {
            VoiceControlState::Listening
        };
        crate::kprintln!("[voice_control] Voice control enabled");
    }

    /// Disable voice control
    pub fn disable(&mut self) {
        self.config.enabled = false;
        self.state = VoiceControlState::Idle;
        crate::kprintln!("[voice_control] Voice control disabled");
    }

    /// Toggle voice control
    pub fn toggle(&mut self) {
        if self.config.enabled {
            self.disable();
        } else {
            self.enable();
        }
    }

    /// Check if enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get current state
    pub fn state(&self) -> VoiceControlState {
        self.state
    }

    /// Start listening for commands
    pub fn start_listening(&mut self) {
        if !self.config.enabled {
            return;
        }

        self.state = VoiceControlState::Listening;
        self.listen_start_ms = crate::time::uptime_ms();
        crate::kprintln!("[voice_control] Started listening");
    }

    /// Stop listening
    pub fn stop_listening(&mut self) {
        if self.state == VoiceControlState::Listening {
            let duration = crate::time::uptime_ms() - self.listen_start_ms;
            self.stats.total_listen_time_ms += duration;
        }

        self.state = if self.config.use_wake_word {
            VoiceControlState::WaitingForWakeWord
        } else {
            VoiceControlState::Idle
        };
    }

    /// Start dictation mode
    pub fn start_dictation(&mut self) {
        if !self.config.enabled {
            return;
        }

        self.state = VoiceControlState::Dictating;
        self.dictation_buffer.clear();
        crate::kprintln!("[voice_control] Dictation mode started");
    }

    /// Stop dictation mode
    pub fn stop_dictation(&mut self) -> String {
        let text = self.dictation_buffer.clone();
        self.dictation_buffer.clear();

        self.state = if self.config.use_wake_word {
            VoiceControlState::WaitingForWakeWord
        } else {
            VoiceControlState::Listening
        };

        crate::kprintln!("[voice_control] Dictation mode stopped");
        text
    }

    /// Process recognition result
    pub fn process_recognition(&mut self, result: RecognitionResult) -> Option<&VoiceCommand> {
        if !self.config.enabled {
            return None;
        }

        // Check confidence threshold
        if result.confidence < self.config.min_confidence {
            self.stats.recognition_errors += 1;
            return None;
        }

        self.last_result = Some(result.clone());
        self.stats.commands_recognized += 1;

        // Call recognition callback
        if let Some(callback) = self.on_recognition {
            callback(&result);
        }

        // Check for wake word
        if self.state == VoiceControlState::WaitingForWakeWord {
            if result.text.to_lowercase().contains(&self.config.wake_word.to_lowercase()) {
                self.stats.wake_word_detections += 1;
                self.start_listening();
                return None;
            }
            return None;
        }

        // Dictation mode
        if self.state == VoiceControlState::Dictating {
            if result.text.to_lowercase() == "stop dictation" {
                self.stop_dictation();
                return None;
            }

            // Add to dictation buffer
            if !self.dictation_buffer.is_empty() {
                self.dictation_buffer.push(' ');
            }
            self.dictation_buffer.push_str(&result.text);

            // Count words
            self.stats.dictation_words += result.text.split_whitespace().count() as u64;

            return None;
        }

        // Command mode - find matching command
        let text = result.text.clone();

        // Check built-in commands
        for cmd in &mut self.commands {
            if cmd.enabled && cmd.matches(&text) {
                if self.config.enabled_categories.contains(&cmd.category) {
                    cmd.use_count += 1;
                    self.stats.commands_executed += 1;
                    return Some(cmd);
                }
            }
        }

        // Check custom commands
        for (_, cmd) in &mut self.custom_commands {
            if cmd.enabled && cmd.matches(&text) {
                cmd.use_count += 1;
                self.stats.commands_executed += 1;
                return Some(cmd);
            }
        }

        None
    }

    /// Add custom command
    pub fn add_custom_command(&mut self, phrases: &[&str], description: &str, action: CommandAction) -> u32 {
        let id = self.next_custom_id;
        self.next_custom_id += 1;

        let cmd = VoiceCommand::new(id, CommandCategory::Custom, phrases, description, action);
        self.custom_commands.insert(id, cmd);

        crate::kprintln!("[voice_control] Added custom command: {} (ID: {})", description, id);
        id
    }

    /// Remove custom command
    pub fn remove_custom_command(&mut self, id: u32) -> bool {
        self.custom_commands.remove(&id).is_some()
    }

    /// Get command by ID
    pub fn get_command(&self, id: u32) -> Option<&VoiceCommand> {
        // Check built-in
        for cmd in &self.commands {
            if cmd.id == id {
                return Some(cmd);
            }
        }
        // Check custom
        self.custom_commands.get(&id)
    }

    /// Enable/disable command
    pub fn set_command_enabled(&mut self, id: u32, enabled: bool) {
        for cmd in &mut self.commands {
            if cmd.id == id {
                cmd.enabled = enabled;
                return;
            }
        }
        if let Some(cmd) = self.custom_commands.get_mut(&id) {
            cmd.enabled = enabled;
        }
    }

    /// Set wake word
    pub fn set_wake_word(&mut self, wake_word: &str) {
        self.config.wake_word = String::from(wake_word);
    }

    /// Get wake word
    pub fn wake_word(&self) -> &str {
        &self.config.wake_word
    }

    /// Enable/disable wake word
    pub fn set_use_wake_word(&mut self, use_it: bool) {
        self.config.use_wake_word = use_it;
        if self.config.enabled {
            self.state = if use_it {
                VoiceControlState::WaitingForWakeWord
            } else {
                VoiceControlState::Listening
            };
        }
    }

    /// Set language
    pub fn set_language(&mut self, language: &str) {
        self.config.language = String::from(language);
    }

    /// Get language
    pub fn language(&self) -> &str {
        &self.config.language
    }

    /// Set minimum confidence
    pub fn set_min_confidence(&mut self, confidence: f32) {
        self.config.min_confidence = confidence.max(0.0).min(1.0);
    }

    /// Set recognition callback
    pub fn set_recognition_callback(&mut self, callback: fn(&RecognitionResult)) {
        self.on_recognition = Some(callback);
    }

    /// Set command callback
    pub fn set_command_callback(&mut self, callback: fn(&VoiceCommand)) {
        self.on_command = Some(callback);
    }

    /// Set audio input callback
    pub fn set_audio_input_callback(&mut self, callback: fn(&[i16]) -> Option<RecognitionResult>) {
        self.on_audio_input = Some(callback);
    }

    /// Get all commands
    pub fn commands(&self) -> &[VoiceCommand] {
        &self.commands
    }

    /// Get custom commands
    pub fn custom_commands(&self) -> &BTreeMap<u32, VoiceCommand> {
        &self.custom_commands
    }

    /// Get commands by category
    pub fn commands_by_category(&self, category: CommandCategory) -> Vec<&VoiceCommand> {
        let mut result: Vec<&VoiceCommand> = self.commands
            .iter()
            .filter(|c| c.category == category)
            .collect();

        result.extend(
            self.custom_commands
                .values()
                .filter(|c| c.category == category)
        );

        result
    }

    /// Get dictation buffer
    pub fn dictation_buffer(&self) -> &str {
        &self.dictation_buffer
    }

    /// Get last recognition result
    pub fn last_result(&self) -> Option<&RecognitionResult> {
        self.last_result.as_ref()
    }

    /// Get configuration
    pub fn config(&self) -> &VoiceControlConfig {
        &self.config
    }

    /// Set configuration
    pub fn set_config(&mut self, config: VoiceControlConfig) {
        self.config = config;
    }

    /// Get statistics
    pub fn stats(&self) -> &VoiceControlStats {
        &self.stats
    }

    /// Format status string
    pub fn format_status(&self) -> String {
        format!(
            "Voice Control:\n\
             Enabled: {}\n\
             State: {}\n\
             Wake word: {} ({})\n\
             Language: {}\n\
             Min confidence: {:.0}%\n\
             Commands: {} built-in, {} custom\n\
             Commands executed: {}\n\
             Dictation words: {}\n\
             Wake word detections: {}",
            if self.config.enabled { "Yes" } else { "No" },
            self.state.name(),
            self.config.wake_word,
            if self.config.use_wake_word { "enabled" } else { "disabled" },
            self.config.language,
            self.config.min_confidence * 100.0,
            self.commands.len(),
            self.custom_commands.len(),
            self.stats.commands_executed,
            self.stats.dictation_words,
            self.stats.wake_word_detections
        )
    }
}

/// Global voice control manager
static VOICE_CONTROL: IrqSafeMutex<Option<VoiceControl>> = IrqSafeMutex::new(None);

/// Initialize voice control
pub fn init() {
    let mut vc = VoiceControl::new();
    vc.init();
    *VOICE_CONTROL.lock() = Some(vc);
}

/// Enable voice control
pub fn enable() {
    if let Some(ref mut vc) = *VOICE_CONTROL.lock() {
        vc.enable();
    }
}

/// Disable voice control
pub fn disable() {
    if let Some(ref mut vc) = *VOICE_CONTROL.lock() {
        vc.disable();
    }
}

/// Toggle voice control
pub fn toggle() {
    if let Some(ref mut vc) = *VOICE_CONTROL.lock() {
        vc.toggle();
    }
}

/// Check if enabled
pub fn is_enabled() -> bool {
    VOICE_CONTROL.lock().as_ref().map(|vc| vc.is_enabled()).unwrap_or(false)
}

/// Get current state
pub fn state() -> VoiceControlState {
    VOICE_CONTROL.lock().as_ref().map(|vc| vc.state()).unwrap_or(VoiceControlState::Idle)
}

/// Start listening
pub fn start_listening() {
    if let Some(ref mut vc) = *VOICE_CONTROL.lock() {
        vc.start_listening();
    }
}

/// Stop listening
pub fn stop_listening() {
    if let Some(ref mut vc) = *VOICE_CONTROL.lock() {
        vc.stop_listening();
    }
}

/// Start dictation
pub fn start_dictation() {
    if let Some(ref mut vc) = *VOICE_CONTROL.lock() {
        vc.start_dictation();
    }
}

/// Stop dictation
pub fn stop_dictation() -> String {
    VOICE_CONTROL.lock().as_mut()
        .map(|vc| vc.stop_dictation())
        .unwrap_or_default()
}

/// Get status string
pub fn status() -> String {
    VOICE_CONTROL.lock().as_ref()
        .map(|vc| vc.format_status())
        .unwrap_or_else(|| String::from("Voice Control: Not initialized"))
}

/// Get statistics
pub fn stats() -> Option<VoiceControlStats> {
    VOICE_CONTROL.lock().as_ref().map(|vc| vc.stats().clone())
}
