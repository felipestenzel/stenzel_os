//! Screen Reader for Accessibility
//!
//! Provides text-to-speech functionality for visually impaired users.
//! Announces UI elements, reads text content, and provides navigation feedback.

#![allow(dead_code)]

use alloc::collections::VecDeque;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;

use crate::sync::IrqSafeMutex;

/// UI element role for accessibility
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessibleRole {
    /// Window/frame
    Window,
    /// Dialog/modal
    Dialog,
    /// Alert message
    Alert,
    /// Menu bar
    MenuBar,
    /// Menu
    Menu,
    /// Menu item
    MenuItem,
    /// Button
    Button,
    /// Link/hyperlink
    Link,
    /// Text input field
    TextField,
    /// Password field
    PasswordField,
    /// Text area (multiline)
    TextArea,
    /// Checkbox
    CheckBox,
    /// Radio button
    RadioButton,
    /// Combo box/dropdown
    ComboBox,
    /// List box
    ListBox,
    /// List item
    ListItem,
    /// Tree view
    TreeView,
    /// Tree item
    TreeItem,
    /// Table
    Table,
    /// Table row
    TableRow,
    /// Table cell
    TableCell,
    /// Tab list
    TabList,
    /// Tab
    Tab,
    /// Tab panel
    TabPanel,
    /// Scroll bar
    ScrollBar,
    /// Slider
    Slider,
    /// Progress bar
    ProgressBar,
    /// Tooltip
    Tooltip,
    /// Image
    Image,
    /// Static text/label
    StaticText,
    /// Heading (H1-H6)
    Heading,
    /// Paragraph
    Paragraph,
    /// Separator
    Separator,
    /// Group/container
    Group,
    /// Toolbar
    Toolbar,
    /// Status bar
    StatusBar,
    /// Notification
    Notification,
    /// Unknown
    Unknown,
}

impl AccessibleRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Window => "window",
            Self::Dialog => "dialog",
            Self::Alert => "alert",
            Self::MenuBar => "menu bar",
            Self::Menu => "menu",
            Self::MenuItem => "menu item",
            Self::Button => "button",
            Self::Link => "link",
            Self::TextField => "text field",
            Self::PasswordField => "password field",
            Self::TextArea => "text area",
            Self::CheckBox => "checkbox",
            Self::RadioButton => "radio button",
            Self::ComboBox => "combo box",
            Self::ListBox => "list box",
            Self::ListItem => "list item",
            Self::TreeView => "tree view",
            Self::TreeItem => "tree item",
            Self::Table => "table",
            Self::TableRow => "row",
            Self::TableCell => "cell",
            Self::TabList => "tab list",
            Self::Tab => "tab",
            Self::TabPanel => "tab panel",
            Self::ScrollBar => "scroll bar",
            Self::Slider => "slider",
            Self::ProgressBar => "progress bar",
            Self::Tooltip => "tooltip",
            Self::Image => "image",
            Self::StaticText => "text",
            Self::Heading => "heading",
            Self::Paragraph => "paragraph",
            Self::Separator => "separator",
            Self::Group => "group",
            Self::Toolbar => "toolbar",
            Self::StatusBar => "status bar",
            Self::Notification => "notification",
            Self::Unknown => "unknown",
        }
    }

    /// Check if this role is interactive
    pub fn is_interactive(&self) -> bool {
        matches!(
            self,
            Self::Button
                | Self::Link
                | Self::TextField
                | Self::PasswordField
                | Self::TextArea
                | Self::CheckBox
                | Self::RadioButton
                | Self::ComboBox
                | Self::ListItem
                | Self::TreeItem
                | Self::Tab
                | Self::Slider
                | Self::MenuItem
        )
    }
}

/// UI element state
#[derive(Debug, Clone, Copy, Default)]
pub struct AccessibleState {
    /// Is focused
    pub focused: bool,
    /// Is selected
    pub selected: bool,
    /// Is checked (checkboxes, radio buttons)
    pub checked: bool,
    /// Is expanded (trees, menus)
    pub expanded: bool,
    /// Is collapsed
    pub collapsed: bool,
    /// Is disabled
    pub disabled: bool,
    /// Is read-only
    pub readonly: bool,
    /// Is required (form fields)
    pub required: bool,
    /// Is invalid (form validation)
    pub invalid: bool,
    /// Is busy/loading
    pub busy: bool,
    /// Is pressed (buttons)
    pub pressed: bool,
    /// Is editable
    pub editable: bool,
    /// Is multiselectable
    pub multiselectable: bool,
}

impl AccessibleState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build state description
    pub fn describe(&self) -> String {
        let mut parts = Vec::new();

        if self.disabled {
            parts.push("disabled");
        }
        if self.checked {
            parts.push("checked");
        }
        if self.expanded {
            parts.push("expanded");
        }
        if self.collapsed {
            parts.push("collapsed");
        }
        if self.selected {
            parts.push("selected");
        }
        if self.pressed {
            parts.push("pressed");
        }
        if self.readonly {
            parts.push("read only");
        }
        if self.required {
            parts.push("required");
        }
        if self.invalid {
            parts.push("invalid");
        }
        if self.busy {
            parts.push("busy");
        }

        parts.join(", ")
    }
}

/// Accessible element information
#[derive(Debug, Clone)]
pub struct AccessibleElement {
    /// Element ID
    pub id: u64,
    /// Parent element ID (0 = root)
    pub parent_id: u64,
    /// Role
    pub role: AccessibleRole,
    /// Name/label
    pub name: String,
    /// Description
    pub description: String,
    /// Value (for inputs, sliders, etc.)
    pub value: String,
    /// State
    pub state: AccessibleState,
    /// Position in parent (for lists, etc.)
    pub position: u32,
    /// Total items in container
    pub set_size: u32,
    /// Heading level (1-6, 0 = not a heading)
    pub heading_level: u8,
    /// Shortcut key
    pub shortcut: String,
    /// Bounds (x, y, width, height)
    pub bounds: (i32, i32, u32, u32),
    /// Children IDs
    pub children: Vec<u64>,
}

impl AccessibleElement {
    pub fn new(id: u64, role: AccessibleRole, name: &str) -> Self {
        Self {
            id,
            parent_id: 0,
            role,
            name: String::from(name),
            description: String::new(),
            value: String::new(),
            state: AccessibleState::new(),
            position: 0,
            set_size: 0,
            heading_level: 0,
            shortcut: String::new(),
            bounds: (0, 0, 0, 0),
            children: Vec::new(),
        }
    }

    /// Build full announcement text
    pub fn announce(&self) -> String {
        let mut parts = Vec::new();

        // Name
        if !self.name.is_empty() {
            parts.push(self.name.clone());
        }

        // Value
        if !self.value.is_empty() {
            parts.push(self.value.clone());
        }

        // Role
        if self.role != AccessibleRole::StaticText || self.name.is_empty() {
            parts.push(String::from(self.role.as_str()));
        }

        // Heading level
        if self.heading_level > 0 {
            parts.push(format!("level {}", self.heading_level));
        }

        // Position
        if self.set_size > 1 {
            parts.push(format!("{} of {}", self.position + 1, self.set_size));
        }

        // State
        let state_desc = self.state.describe();
        if !state_desc.is_empty() {
            parts.push(state_desc);
        }

        // Shortcut
        if !self.shortcut.is_empty() {
            parts.push(format!("shortcut {}", self.shortcut));
        }

        // Description (at the end)
        if !self.description.is_empty() {
            parts.push(self.description.clone());
        }

        parts.join(", ")
    }
}

/// Speech priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SpeechPriority {
    /// Low priority (background info)
    Low = 0,
    /// Normal priority (navigation)
    Normal = 1,
    /// High priority (important info)
    High = 2,
    /// Alert (interrupts immediately)
    Alert = 3,
}

/// Speech utterance
#[derive(Debug, Clone)]
pub struct SpeechUtterance {
    /// Text to speak
    pub text: String,
    /// Priority
    pub priority: SpeechPriority,
    /// Pitch (0.5 to 2.0, 1.0 = normal)
    pub pitch: f32,
    /// Rate (0.5 to 4.0, 1.0 = normal)
    pub rate: f32,
    /// Volume (0.0 to 1.0)
    pub volume: f32,
    /// Interrupt current speech
    pub interrupt: bool,
    /// Language/locale
    pub lang: String,
    /// Timestamp
    pub timestamp: u64,
}

impl SpeechUtterance {
    pub fn new(text: &str) -> Self {
        Self {
            text: String::from(text),
            priority: SpeechPriority::Normal,
            pitch: 1.0,
            rate: 1.0,
            volume: 1.0,
            interrupt: false,
            lang: String::from("en-US"),
            timestamp: crate::time::uptime_ms(),
        }
    }

    pub fn with_priority(mut self, priority: SpeechPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn alert(text: &str) -> Self {
        Self::new(text)
            .with_priority(SpeechPriority::Alert)
    }
}

/// Verbosity level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerbosityLevel {
    /// Minimal (name only)
    Minimal,
    /// Brief (name + role)
    Brief,
    /// Normal (name + role + state)
    Normal,
    /// Verbose (full description)
    Verbose,
}

/// Navigation mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationMode {
    /// Navigate all elements
    All,
    /// Navigate interactive elements only
    Interactive,
    /// Navigate headings only
    Headings,
    /// Navigate links only
    Links,
    /// Navigate form controls only
    FormControls,
    /// Navigate landmarks (main, nav, etc.)
    Landmarks,
    /// Navigate tables only
    Tables,
    /// Navigate by words
    Words,
    /// Navigate by characters
    Characters,
    /// Navigate by lines
    Lines,
}

/// Screen reader configuration
#[derive(Debug, Clone)]
pub struct ScreenReaderConfig {
    /// Is enabled
    pub enabled: bool,
    /// Verbosity level
    pub verbosity: VerbosityLevel,
    /// Speech rate (0.5 to 4.0)
    pub speech_rate: f32,
    /// Speech pitch (0.5 to 2.0)
    pub speech_pitch: f32,
    /// Speech volume (0.0 to 1.0)
    pub speech_volume: f32,
    /// Announce on focus change
    pub announce_focus: bool,
    /// Announce on hover
    pub announce_hover: bool,
    /// Read typed characters
    pub echo_characters: bool,
    /// Read typed words
    pub echo_words: bool,
    /// Read typed lines
    pub echo_lines: bool,
    /// Use phonetic spelling
    pub phonetic_spelling: bool,
    /// Read punctuation
    pub read_punctuation: bool,
    /// Use sounds for navigation
    pub navigation_sounds: bool,
    /// Auto-read page on load
    pub auto_read_page: bool,
    /// Language
    pub language: String,
}

impl ScreenReaderConfig {
    pub fn default() -> Self {
        Self {
            enabled: false,
            verbosity: VerbosityLevel::Normal,
            speech_rate: 1.0,
            speech_pitch: 1.0,
            speech_volume: 1.0,
            announce_focus: true,
            announce_hover: false,
            echo_characters: false,
            echo_words: true,
            echo_lines: false,
            phonetic_spelling: false,
            read_punctuation: false,
            navigation_sounds: true,
            auto_read_page: false,
            language: String::from("en-US"),
        }
    }
}

/// Screen reader statistics
#[derive(Debug, Clone, Copy, Default)]
pub struct ScreenReaderStats {
    /// Total utterances spoken
    pub total_utterances: u64,
    /// Total characters spoken
    pub total_characters: u64,
    /// Focus changes announced
    pub focus_announcements: u64,
    /// Navigation actions
    pub navigation_count: u64,
    /// Time enabled (ms)
    pub enabled_time_ms: u64,
    /// Interruptions
    pub interruptions: u64,
    /// Last announcement time
    pub last_announcement_time: u64,
}

/// TTS callback type
pub type TtsCallback = fn(&str, f32, f32, f32);

/// Screen reader state
struct ScreenReaderState {
    /// Configuration
    config: ScreenReaderConfig,
    /// Statistics
    stats: ScreenReaderStats,
    /// Speech queue
    queue: VecDeque<SpeechUtterance>,
    /// Currently speaking
    speaking: bool,
    /// Current utterance
    current_utterance: Option<SpeechUtterance>,
    /// Focus element ID
    focus_id: u64,
    /// Navigation mode
    nav_mode: NavigationMode,
    /// TTS callback
    tts_callback: Option<TtsCallback>,
    /// Enable timestamp
    enabled_since: u64,
    /// Last word (for word echo)
    last_word: String,
    /// Initialized
    initialized: bool,
}

impl ScreenReaderState {
    const fn new() -> Self {
        Self {
            config: ScreenReaderConfig {
                enabled: false,
                verbosity: VerbosityLevel::Normal,
                speech_rate: 1.0,
                speech_pitch: 1.0,
                speech_volume: 1.0,
                announce_focus: true,
                announce_hover: false,
                echo_characters: false,
                echo_words: true,
                echo_lines: false,
                phonetic_spelling: false,
                read_punctuation: false,
                navigation_sounds: true,
                auto_read_page: false,
                language: String::new(), // Will be set on init
            },
            stats: ScreenReaderStats {
                total_utterances: 0,
                total_characters: 0,
                focus_announcements: 0,
                navigation_count: 0,
                enabled_time_ms: 0,
                interruptions: 0,
                last_announcement_time: 0,
            },
            queue: VecDeque::new(),
            speaking: false,
            current_utterance: None,
            focus_id: 0,
            nav_mode: NavigationMode::All,
            tts_callback: None,
            enabled_since: 0,
            last_word: String::new(),
            initialized: false,
        }
    }
}

/// Global screen reader
static SCREEN_READER: IrqSafeMutex<ScreenReaderState> = IrqSafeMutex::new(ScreenReaderState::new());

/// Screen reader manager
pub struct ScreenReader;

impl ScreenReader {
    /// Initialize screen reader
    pub fn init() {
        let mut state = SCREEN_READER.lock();
        if state.initialized {
            return;
        }

        state.config.language = String::from("en-US");
        state.initialized = true;
        crate::kprintln!("[screen_reader] Screen reader initialized");
    }

    /// Enable/disable screen reader
    pub fn set_enabled(enabled: bool) {
        let mut state = SCREEN_READER.lock();
        let was_enabled = state.config.enabled;
        state.config.enabled = enabled;

        if enabled && !was_enabled {
            state.enabled_since = crate::time::uptime_ms();
            crate::kprintln!("[screen_reader] Screen reader enabled");

            // Queue welcome message
            let utterance = SpeechUtterance::new("Screen reader enabled")
                .with_priority(SpeechPriority::Alert);
            state.queue.push_back(utterance);
        } else if !enabled && was_enabled {
            let now = crate::time::uptime_ms();
            state.stats.enabled_time_ms += now - state.enabled_since;
            state.queue.clear();
            state.speaking = false;
            crate::kprintln!("[screen_reader] Screen reader disabled");
        }
    }

    /// Check if enabled
    pub fn is_enabled() -> bool {
        SCREEN_READER.lock().config.enabled
    }

    /// Set TTS callback
    pub fn set_tts_callback(callback: TtsCallback) {
        let mut state = SCREEN_READER.lock();
        state.tts_callback = Some(callback);
    }

    /// Speak text
    pub fn speak(text: &str) {
        Self::speak_with_priority(text, SpeechPriority::Normal);
    }

    /// Speak text with priority
    pub fn speak_with_priority(text: &str, priority: SpeechPriority) {
        let mut state = SCREEN_READER.lock();
        if !state.config.enabled {
            return;
        }

        let utterance = SpeechUtterance {
            text: String::from(text),
            priority,
            pitch: state.config.speech_pitch,
            rate: state.config.speech_rate,
            volume: state.config.speech_volume,
            interrupt: priority == SpeechPriority::Alert,
            lang: state.config.language.clone(),
            timestamp: crate::time::uptime_ms(),
        };

        if utterance.interrupt {
            state.stats.interruptions += 1;
            state.queue.clear();
            state.speaking = false;
        }

        state.queue.push_back(utterance);
    }

    /// Speak alert (high priority, interrupts)
    pub fn alert(text: &str) {
        Self::speak_with_priority(text, SpeechPriority::Alert);
    }

    /// Announce focus change
    pub fn announce_focus(element: &AccessibleElement) {
        let mut state = SCREEN_READER.lock();
        if !state.config.enabled || !state.config.announce_focus {
            return;
        }

        state.focus_id = element.id;
        state.stats.focus_announcements += 1;

        let announcement = Self::build_announcement(element, &state.config);
        let utterance = SpeechUtterance {
            text: announcement,
            priority: SpeechPriority::Normal,
            pitch: state.config.speech_pitch,
            rate: state.config.speech_rate,
            volume: state.config.speech_volume,
            interrupt: true, // Focus always interrupts
            lang: state.config.language.clone(),
            timestamp: crate::time::uptime_ms(),
        };

        // Clear lower priority items
        state.queue.retain(|u| u.priority >= SpeechPriority::High);
        state.queue.push_back(utterance);
    }

    /// Build announcement for element based on verbosity
    fn build_announcement(element: &AccessibleElement, config: &ScreenReaderConfig) -> String {
        match config.verbosity {
            VerbosityLevel::Minimal => {
                if element.name.is_empty() {
                    String::from(element.role.as_str())
                } else {
                    element.name.clone()
                }
            }
            VerbosityLevel::Brief => {
                if element.name.is_empty() {
                    String::from(element.role.as_str())
                } else {
                    format!("{}, {}", element.name, element.role.as_str())
                }
            }
            VerbosityLevel::Normal | VerbosityLevel::Verbose => {
                element.announce()
            }
        }
    }

    /// Echo typed character
    pub fn echo_character(ch: char) {
        let state = SCREEN_READER.lock();
        if !state.config.enabled || !state.config.echo_characters {
            return;
        }
        drop(state);

        let text = if ch.is_alphabetic() {
            ch.to_string()
        } else {
            Self::describe_character(ch)
        };

        Self::speak_with_priority(&text, SpeechPriority::High);
    }

    /// Echo typed word
    pub fn echo_word(word: &str) {
        let mut state = SCREEN_READER.lock();
        if !state.config.enabled || !state.config.echo_words {
            return;
        }

        // Don't repeat same word
        if state.last_word == word {
            return;
        }
        state.last_word = String::from(word);
        drop(state);

        Self::speak_with_priority(word, SpeechPriority::High);
    }

    /// Describe a character
    fn describe_character(ch: char) -> String {
        match ch {
            ' ' => String::from("space"),
            '\n' => String::from("new line"),
            '\t' => String::from("tab"),
            '!' => String::from("exclamation mark"),
            '?' => String::from("question mark"),
            '.' => String::from("period"),
            ',' => String::from("comma"),
            ':' => String::from("colon"),
            ';' => String::from("semicolon"),
            '"' => String::from("quote"),
            '\'' => String::from("apostrophe"),
            '(' => String::from("left parenthesis"),
            ')' => String::from("right parenthesis"),
            '[' => String::from("left bracket"),
            ']' => String::from("right bracket"),
            '{' => String::from("left brace"),
            '}' => String::from("right brace"),
            '<' => String::from("less than"),
            '>' => String::from("greater than"),
            '/' => String::from("slash"),
            '\\' => String::from("backslash"),
            '|' => String::from("pipe"),
            '@' => String::from("at sign"),
            '#' => String::from("hash"),
            '$' => String::from("dollar"),
            '%' => String::from("percent"),
            '^' => String::from("caret"),
            '&' => String::from("ampersand"),
            '*' => String::from("asterisk"),
            '-' => String::from("hyphen"),
            '_' => String::from("underscore"),
            '+' => String::from("plus"),
            '=' => String::from("equals"),
            '~' => String::from("tilde"),
            '`' => String::from("backtick"),
            _ if ch.is_digit(10) => ch.to_string(),
            _ => format!("{}", ch),
        }
    }

    /// Set navigation mode
    pub fn set_navigation_mode(mode: NavigationMode) {
        let mut state = SCREEN_READER.lock();
        state.nav_mode = mode;
        state.stats.navigation_count += 1;

        let mode_name = match mode {
            NavigationMode::All => "all elements",
            NavigationMode::Interactive => "interactive elements",
            NavigationMode::Headings => "headings",
            NavigationMode::Links => "links",
            NavigationMode::FormControls => "form controls",
            NavigationMode::Landmarks => "landmarks",
            NavigationMode::Tables => "tables",
            NavigationMode::Words => "words",
            NavigationMode::Characters => "characters",
            NavigationMode::Lines => "lines",
        };

        let announcement = format!("Navigation mode: {}", mode_name);
        let utterance = SpeechUtterance::new(&announcement)
            .with_priority(SpeechPriority::High);
        state.queue.push_back(utterance);
    }

    /// Get navigation mode
    pub fn navigation_mode() -> NavigationMode {
        SCREEN_READER.lock().nav_mode
    }

    /// Process speech queue (call periodically)
    pub fn process_queue() {
        let mut state = SCREEN_READER.lock();

        if !state.config.enabled || state.speaking {
            return;
        }

        if let Some(utterance) = state.queue.pop_front() {
            state.speaking = true;
            state.current_utterance = Some(utterance.clone());
            state.stats.total_utterances += 1;
            state.stats.total_characters += utterance.text.len() as u64;
            state.stats.last_announcement_time = crate::time::uptime_ms();

            // Call TTS
            if let Some(callback) = state.tts_callback {
                callback(&utterance.text, utterance.rate, utterance.pitch, utterance.volume);
            }
        }
    }

    /// Mark current speech as finished
    pub fn speech_finished() {
        let mut state = SCREEN_READER.lock();
        state.speaking = false;
        state.current_utterance = None;
    }

    /// Stop speaking
    pub fn stop() {
        let mut state = SCREEN_READER.lock();
        state.queue.clear();
        state.speaking = false;
        state.current_utterance = None;
    }

    /// Get configuration
    pub fn config() -> ScreenReaderConfig {
        SCREEN_READER.lock().config.clone()
    }

    /// Set configuration
    pub fn set_config(config: ScreenReaderConfig) {
        let mut state = SCREEN_READER.lock();
        let was_enabled = state.config.enabled;
        state.config = config;

        // Handle enable/disable
        if state.config.enabled && !was_enabled {
            state.enabled_since = crate::time::uptime_ms();
        } else if !state.config.enabled && was_enabled {
            let now = crate::time::uptime_ms();
            state.stats.enabled_time_ms += now - state.enabled_since;
        }
    }

    /// Set verbosity
    pub fn set_verbosity(level: VerbosityLevel) {
        let mut state = SCREEN_READER.lock();
        state.config.verbosity = level;

        let name = match level {
            VerbosityLevel::Minimal => "minimal",
            VerbosityLevel::Brief => "brief",
            VerbosityLevel::Normal => "normal",
            VerbosityLevel::Verbose => "verbose",
        };

        let utterance = SpeechUtterance::new(&format!("Verbosity: {}", name))
            .with_priority(SpeechPriority::High);
        state.queue.push_back(utterance);
    }

    /// Set speech rate
    pub fn set_speech_rate(rate: f32) {
        let mut state = SCREEN_READER.lock();
        state.config.speech_rate = rate.clamp(0.5, 4.0);
    }

    /// Set speech pitch
    pub fn set_speech_pitch(pitch: f32) {
        let mut state = SCREEN_READER.lock();
        state.config.speech_pitch = pitch.clamp(0.5, 2.0);
    }

    /// Set speech volume
    pub fn set_speech_volume(volume: f32) {
        let mut state = SCREEN_READER.lock();
        state.config.speech_volume = volume.clamp(0.0, 1.0);
    }

    /// Get statistics
    pub fn stats() -> ScreenReaderStats {
        let mut state = SCREEN_READER.lock();

        // Update enabled time if currently enabled
        if state.config.enabled {
            let now = crate::time::uptime_ms();
            state.stats.enabled_time_ms += now - state.enabled_since;
            state.enabled_since = now;
        }

        state.stats
    }

    /// Check if initialized
    pub fn is_initialized() -> bool {
        SCREEN_READER.lock().initialized
    }

    /// Get queue size
    pub fn queue_size() -> usize {
        SCREEN_READER.lock().queue.len()
    }

    /// Is currently speaking
    pub fn is_speaking() -> bool {
        SCREEN_READER.lock().speaking
    }

    /// Format status for display
    pub fn format_status() -> String {
        let state = SCREEN_READER.lock();
        use alloc::fmt::Write;
        let mut s = String::new();

        let _ = writeln!(s, "Screen Reader Status:");
        let _ = writeln!(s, "  Initialized: {}", state.initialized);
        let _ = writeln!(s, "  Enabled: {}", state.config.enabled);
        let _ = writeln!(s, "  Speaking: {}", state.speaking);
        let _ = writeln!(s, "  Queue size: {}", state.queue.len());
        let _ = writeln!(s, "  Navigation mode: {:?}", state.nav_mode);
        let _ = writeln!(s, "  Configuration:");
        let _ = writeln!(s, "    Verbosity: {:?}", state.config.verbosity);
        let _ = writeln!(s, "    Speech rate: {:.1}", state.config.speech_rate);
        let _ = writeln!(s, "    Speech pitch: {:.1}", state.config.speech_pitch);
        let _ = writeln!(s, "    Speech volume: {:.1}", state.config.speech_volume);
        let _ = writeln!(s, "    Echo characters: {}", state.config.echo_characters);
        let _ = writeln!(s, "    Echo words: {}", state.config.echo_words);
        let _ = writeln!(s, "  Statistics:");
        let _ = writeln!(s, "    Total utterances: {}", state.stats.total_utterances);
        let _ = writeln!(s, "    Total characters: {}", state.stats.total_characters);
        let _ = writeln!(s, "    Focus announcements: {}", state.stats.focus_announcements);
        let _ = writeln!(s, "    Navigation count: {}", state.stats.navigation_count);
        let _ = writeln!(s, "    Enabled time: {} ms", state.stats.enabled_time_ms);

        s
    }
}

// ============================================================================
// Public API
// ============================================================================

/// Initialize screen reader
pub fn init() {
    ScreenReader::init();
}

/// Enable/disable screen reader
pub fn set_enabled(enabled: bool) {
    ScreenReader::set_enabled(enabled);
}

/// Check if enabled
pub fn is_enabled() -> bool {
    ScreenReader::is_enabled()
}

/// Speak text
pub fn speak(text: &str) {
    ScreenReader::speak(text);
}

/// Speak alert
pub fn alert(text: &str) {
    ScreenReader::alert(text);
}

/// Stop speaking
pub fn stop() {
    ScreenReader::stop();
}

/// Get status
pub fn status() -> String {
    ScreenReader::format_status()
}
