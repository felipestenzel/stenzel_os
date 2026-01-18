//! On-Screen Keyboard (OSK)
//!
//! Provides a virtual keyboard for users who cannot use a physical keyboard.
//! Features:
//! - Multiple keyboard layouts (QWERTY, AZERTY, Dvorak)
//! - Different keyboard modes (standard, compact, split)
//! - Customizable appearance
//! - Dwell clicking support
//! - Word prediction

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use alloc::collections::BTreeMap;

use crate::sync::IrqSafeMutex;

/// Key type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyType {
    /// Regular character key
    Character,
    /// Space bar
    Space,
    /// Backspace
    Backspace,
    /// Enter/Return
    Enter,
    /// Shift key
    Shift,
    /// Caps Lock
    CapsLock,
    /// Tab key
    Tab,
    /// Control key
    Ctrl,
    /// Alt key
    Alt,
    /// Windows/Super/Meta key
    Super,
    /// Function key (F1-F12)
    Function,
    /// Arrow key
    Arrow,
    /// Delete key
    Delete,
    /// Escape key
    Escape,
    /// Number/Symbol toggle
    NumberToggle,
    /// Language switch
    LanguageSwitch,
    /// Close OSK
    Close,
    /// Minimize OSK
    Minimize,
    /// Settings
    Settings,
}

/// Key definition
#[derive(Debug, Clone)]
pub struct KeyDefinition {
    /// Key type
    pub key_type: KeyType,
    /// Normal character
    pub normal: char,
    /// Shifted character
    pub shifted: char,
    /// Key label (for display)
    pub label: String,
    /// Shifted label
    pub shifted_label: String,
    /// Width in units (1 = standard key width)
    pub width: f32,
    /// Key code (scancode)
    pub key_code: u8,
}

impl KeyDefinition {
    /// Create a character key
    pub fn char(normal: char, shifted: char, code: u8) -> Self {
        Self {
            key_type: KeyType::Character,
            normal,
            shifted,
            label: String::from(char_to_str(normal)),
            shifted_label: String::from(char_to_str(shifted)),
            width: 1.0,
            key_code: code,
        }
    }

    /// Create a special key
    pub fn special(key_type: KeyType, label: &str, width: f32, code: u8) -> Self {
        Self {
            key_type,
            normal: '\0',
            shifted: '\0',
            label: String::from(label),
            shifted_label: String::from(label),
            width,
            key_code: code,
        }
    }

    /// Get display label based on shift state
    pub fn display_label(&self, shifted: bool) -> &str {
        if shifted && !self.shifted_label.is_empty() {
            &self.shifted_label
        } else {
            &self.label
        }
    }
}

/// Helper function to convert char to static str (simplified)
fn char_to_str(c: char) -> &'static str {
    match c {
        'a' => "a", 'b' => "b", 'c' => "c", 'd' => "d", 'e' => "e",
        'f' => "f", 'g' => "g", 'h' => "h", 'i' => "i", 'j' => "j",
        'k' => "k", 'l' => "l", 'm' => "m", 'n' => "n", 'o' => "o",
        'p' => "p", 'q' => "q", 'r' => "r", 's' => "s", 't' => "t",
        'u' => "u", 'v' => "v", 'w' => "w", 'x' => "x", 'y' => "y",
        'z' => "z",
        'A' => "A", 'B' => "B", 'C' => "C", 'D' => "D", 'E' => "E",
        'F' => "F", 'G' => "G", 'H' => "H", 'I' => "I", 'J' => "J",
        'K' => "K", 'L' => "L", 'M' => "M", 'N' => "N", 'O' => "O",
        'P' => "P", 'Q' => "Q", 'R' => "R", 'S' => "S", 'T' => "T",
        'U' => "U", 'V' => "V", 'W' => "W", 'X' => "X", 'Y' => "Y",
        'Z' => "Z",
        '0' => "0", '1' => "1", '2' => "2", '3' => "3", '4' => "4",
        '5' => "5", '6' => "6", '7' => "7", '8' => "8", '9' => "9",
        '!' => "!", '@' => "@", '#' => "#", '$' => "$", '%' => "%",
        '^' => "^", '&' => "&", '*' => "*", '(' => "(", ')' => ")",
        '-' => "-", '_' => "_", '=' => "=", '+' => "+", '[' => "[",
        ']' => "]", '{' => "{", '}' => "}", '\\' => "\\", '|' => "|",
        ';' => ";", ':' => ":", '\'' => "'", '"' => "\"", ',' => ",",
        '.' => ".", '<' => "<", '>' => ">", '/' => "/", '?' => "?",
        '`' => "`", '~' => "~", ' ' => " ",
        _ => "?",
    }
}

/// Keyboard layout type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardLayout {
    /// US QWERTY
    QwertyUs,
    /// UK QWERTY
    QwertyUk,
    /// French AZERTY
    Azerty,
    /// German QWERTZ
    Qwertz,
    /// Dvorak
    Dvorak,
    /// Colemak
    Colemak,
    /// Portuguese Brazilian ABNT2
    Abnt2,
}

impl KeyboardLayout {
    /// Get layout name
    pub fn name(&self) -> &'static str {
        match self {
            KeyboardLayout::QwertyUs => "QWERTY (US)",
            KeyboardLayout::QwertyUk => "QWERTY (UK)",
            KeyboardLayout::Azerty => "AZERTY (French)",
            KeyboardLayout::Qwertz => "QWERTZ (German)",
            KeyboardLayout::Dvorak => "Dvorak",
            KeyboardLayout::Colemak => "Colemak",
            KeyboardLayout::Abnt2 => "ABNT2 (Brazilian)",
        }
    }

    /// Get layout code
    pub fn code(&self) -> &'static str {
        match self {
            KeyboardLayout::QwertyUs => "en-US",
            KeyboardLayout::QwertyUk => "en-GB",
            KeyboardLayout::Azerty => "fr-FR",
            KeyboardLayout::Qwertz => "de-DE",
            KeyboardLayout::Dvorak => "en-US-dvorak",
            KeyboardLayout::Colemak => "en-US-colemak",
            KeyboardLayout::Abnt2 => "pt-BR",
        }
    }
}

/// Keyboard mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardMode {
    /// Standard full keyboard
    Standard,
    /// Compact keyboard (fewer keys)
    Compact,
    /// Split keyboard (two halves)
    Split,
    /// Numeric keypad only
    Numeric,
    /// Phone-style T9 keyboard
    Phone,
}

impl KeyboardMode {
    /// Get mode name
    pub fn name(&self) -> &'static str {
        match self {
            KeyboardMode::Standard => "Standard",
            KeyboardMode::Compact => "Compact",
            KeyboardMode::Split => "Split",
            KeyboardMode::Numeric => "Numeric",
            KeyboardMode::Phone => "Phone",
        }
    }
}

/// Key state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
    /// Normal state
    Normal,
    /// Hovered
    Hovered,
    /// Pressed
    Pressed,
    /// Locked (e.g., Caps Lock on)
    Locked,
    /// Disabled
    Disabled,
}

/// Keyboard position
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardPosition {
    /// Docked at bottom
    Bottom,
    /// Docked at top
    Top,
    /// Floating (draggable)
    Floating,
    /// Docked left
    Left,
    /// Docked right
    Right,
}

impl KeyboardPosition {
    /// Get position name
    pub fn name(&self) -> &'static str {
        match self {
            KeyboardPosition::Bottom => "Bottom",
            KeyboardPosition::Top => "Top",
            KeyboardPosition::Floating => "Floating",
            KeyboardPosition::Left => "Left",
            KeyboardPosition::Right => "Right",
        }
    }
}

/// Color for OSK theming
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OskColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl OskColor {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
}

/// OSK theme
#[derive(Debug, Clone)]
pub struct OskTheme {
    /// Background color
    pub background: OskColor,
    /// Key background
    pub key_bg: OskColor,
    /// Key background (hovered)
    pub key_bg_hover: OskColor,
    /// Key background (pressed)
    pub key_bg_pressed: OskColor,
    /// Special key background
    pub special_key_bg: OskColor,
    /// Key text color
    pub key_text: OskColor,
    /// Special key text
    pub special_key_text: OskColor,
    /// Border color
    pub border: OskColor,
    /// Suggestion bar background
    pub suggestion_bg: OskColor,
    /// Suggestion text
    pub suggestion_text: OskColor,
}

impl Default for OskTheme {
    fn default() -> Self {
        Self::light()
    }
}

impl OskTheme {
    /// Light theme
    pub fn light() -> Self {
        Self {
            background: OskColor::rgba(245, 245, 245, 230),
            key_bg: OskColor::rgb(255, 255, 255),
            key_bg_hover: OskColor::rgb(230, 230, 230),
            key_bg_pressed: OskColor::rgb(200, 200, 200),
            special_key_bg: OskColor::rgb(220, 220, 220),
            key_text: OskColor::rgb(0, 0, 0),
            special_key_text: OskColor::rgb(50, 50, 50),
            border: OskColor::rgb(180, 180, 180),
            suggestion_bg: OskColor::rgb(255, 255, 255),
            suggestion_text: OskColor::rgb(0, 0, 0),
        }
    }

    /// Dark theme
    pub fn dark() -> Self {
        Self {
            background: OskColor::rgba(30, 30, 30, 230),
            key_bg: OskColor::rgb(50, 50, 50),
            key_bg_hover: OskColor::rgb(70, 70, 70),
            key_bg_pressed: OskColor::rgb(90, 90, 90),
            special_key_bg: OskColor::rgb(40, 40, 40),
            key_text: OskColor::rgb(255, 255, 255),
            special_key_text: OskColor::rgb(200, 200, 200),
            border: OskColor::rgb(80, 80, 80),
            suggestion_bg: OskColor::rgb(40, 40, 40),
            suggestion_text: OskColor::rgb(255, 255, 255),
        }
    }

    /// High contrast theme
    pub fn high_contrast() -> Self {
        Self {
            background: OskColor::rgb(0, 0, 0),
            key_bg: OskColor::rgb(0, 0, 0),
            key_bg_hover: OskColor::rgb(0, 0, 128),
            key_bg_pressed: OskColor::rgb(0, 0, 255),
            special_key_bg: OskColor::rgb(0, 0, 0),
            key_text: OskColor::rgb(255, 255, 255),
            special_key_text: OskColor::rgb(255, 255, 0),
            border: OskColor::rgb(255, 255, 255),
            suggestion_bg: OskColor::rgb(0, 0, 0),
            suggestion_text: OskColor::rgb(255, 255, 255),
        }
    }
}

/// On-Screen Keyboard configuration
#[derive(Debug, Clone)]
pub struct OskConfig {
    /// Whether OSK is enabled
    pub enabled: bool,
    /// Keyboard layout
    pub layout: KeyboardLayout,
    /// Keyboard mode
    pub mode: KeyboardMode,
    /// Position
    pub position: KeyboardPosition,
    /// Theme
    pub theme: OskTheme,
    /// Key height in pixels
    pub key_height: u32,
    /// Key spacing in pixels
    pub key_spacing: u32,
    /// Border radius
    pub border_radius: u32,
    /// Show suggestions bar
    pub show_suggestions: bool,
    /// Number of suggestions to show
    pub max_suggestions: usize,
    /// Enable sound on key press
    pub sound_on_press: bool,
    /// Enable haptic feedback
    pub haptic_feedback: bool,
    /// Dwell clicking (hover to click)
    pub dwell_enabled: bool,
    /// Dwell time in ms
    pub dwell_time_ms: u32,
    /// Auto-show when text field focused
    pub auto_show: bool,
    /// Auto-hide when text field loses focus
    pub auto_hide: bool,
    /// Opacity (0-255)
    pub opacity: u8,
    /// Float position (when floating)
    pub float_x: i32,
    pub float_y: i32,
    /// Float size (when floating)
    pub float_width: u32,
    pub float_height: u32,
}

impl Default for OskConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            layout: KeyboardLayout::QwertyUs,
            mode: KeyboardMode::Standard,
            position: KeyboardPosition::Bottom,
            theme: OskTheme::light(),
            key_height: 50,
            key_spacing: 4,
            border_radius: 6,
            show_suggestions: true,
            max_suggestions: 5,
            sound_on_press: true,
            haptic_feedback: false,
            dwell_enabled: false,
            dwell_time_ms: 800,
            auto_show: true,
            auto_hide: true,
            opacity: 230,
            float_x: 100,
            float_y: 300,
            float_width: 800,
            float_height: 280,
        }
    }
}

/// Key visual state
#[derive(Debug, Clone)]
pub struct KeyVisual {
    /// Key definition
    pub key: KeyDefinition,
    /// Current state
    pub state: KeyState,
    /// Position (x, y)
    pub x: i32,
    pub y: i32,
    /// Size
    pub width: u32,
    pub height: u32,
    /// Row index
    pub row: usize,
    /// Column index in row
    pub col: usize,
}

impl KeyVisual {
    /// Check if point is inside key
    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && px < self.x + self.width as i32 &&
        py >= self.y && py < self.y + self.height as i32
    }
}

/// Word prediction entry
#[derive(Debug, Clone)]
pub struct Prediction {
    /// The predicted word
    pub word: String,
    /// Confidence score (0-100)
    pub confidence: u8,
    /// Usage frequency
    pub frequency: u32,
}

/// On-Screen Keyboard statistics
#[derive(Debug, Clone, Default)]
pub struct OskStats {
    /// Total keys pressed
    pub keys_pressed: u64,
    /// Characters typed
    pub chars_typed: u64,
    /// Words completed
    pub words_completed: u64,
    /// Predictions accepted
    pub predictions_accepted: u64,
    /// Backspaces
    pub backspaces: u64,
    /// Dwell clicks
    pub dwell_clicks: u64,
    /// Session start timestamp
    pub session_start_ms: u64,
}

/// On-Screen Keyboard manager
pub struct OnScreenKeyboard {
    /// Configuration
    config: OskConfig,
    /// Whether currently visible
    visible: bool,
    /// Current modifier states
    shift_active: bool,
    shift_locked: bool,
    ctrl_active: bool,
    alt_active: bool,
    super_active: bool,
    /// Current keyboard rows
    keys: Vec<Vec<KeyVisual>>,
    /// Current input buffer
    input_buffer: String,
    /// Word predictions
    predictions: Vec<Prediction>,
    /// Common words dictionary
    dictionary: BTreeMap<String, u32>,
    /// Statistics
    stats: OskStats,
    /// Key press callback
    on_key_press: Option<fn(char, u8)>,
    /// Special key callback
    on_special_key: Option<fn(KeyType, bool)>,
    /// Dwell tracking
    dwell_key_index: Option<(usize, usize)>,
    dwell_start_ms: u64,
    /// Screen dimensions
    screen_width: u32,
    screen_height: u32,
}

impl OnScreenKeyboard {
    /// Create a new on-screen keyboard
    pub fn new() -> Self {
        Self {
            config: OskConfig::default(),
            visible: false,
            shift_active: false,
            shift_locked: false,
            ctrl_active: false,
            alt_active: false,
            super_active: false,
            keys: Vec::new(),
            input_buffer: String::new(),
            predictions: Vec::new(),
            dictionary: BTreeMap::new(),
            stats: OskStats::default(),
            on_key_press: None,
            on_special_key: None,
            dwell_key_index: None,
            dwell_start_ms: 0,
            screen_width: 1920,
            screen_height: 1080,
        }
    }

    /// Initialize the OSK
    pub fn init(&mut self) {
        self.stats.session_start_ms = crate::time::uptime_ms();
        self.load_layout();
        self.load_dictionary();
        crate::kprintln!("[osk] On-screen keyboard initialized");
    }

    /// Set screen dimensions
    pub fn set_screen_size(&mut self, width: u32, height: u32) {
        self.screen_width = width;
        self.screen_height = height;
        self.recalculate_positions();
    }

    /// Show the keyboard
    pub fn show(&mut self) {
        self.visible = true;
        self.recalculate_positions();
        crate::kprintln!("[osk] On-screen keyboard shown");
    }

    /// Hide the keyboard
    pub fn hide(&mut self) {
        self.visible = false;
        crate::kprintln!("[osk] On-screen keyboard hidden");
    }

    /// Toggle visibility
    pub fn toggle(&mut self) {
        if self.visible {
            self.hide();
        } else {
            self.show();
        }
    }

    /// Check if visible
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Enable OSK
    pub fn enable(&mut self) {
        self.config.enabled = true;
        crate::kprintln!("[osk] On-screen keyboard enabled");
    }

    /// Disable OSK
    pub fn disable(&mut self) {
        self.config.enabled = false;
        self.hide();
        crate::kprintln!("[osk] On-screen keyboard disabled");
    }

    /// Check if enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Load keyboard layout
    fn load_layout(&mut self) {
        self.keys.clear();

        match self.config.layout {
            KeyboardLayout::QwertyUs | KeyboardLayout::QwertyUk => {
                self.load_qwerty_layout();
            }
            KeyboardLayout::Azerty => {
                self.load_azerty_layout();
            }
            KeyboardLayout::Qwertz => {
                self.load_qwertz_layout();
            }
            KeyboardLayout::Dvorak => {
                self.load_dvorak_layout();
            }
            KeyboardLayout::Colemak => {
                self.load_colemak_layout();
            }
            KeyboardLayout::Abnt2 => {
                self.load_abnt2_layout();
            }
        }

        self.recalculate_positions();
    }

    /// Load QWERTY layout
    fn load_qwerty_layout(&mut self) {
        // Row 1: Number row
        let row1 = vec![
            KeyDefinition::char('`', '~', 0x29),
            KeyDefinition::char('1', '!', 0x02),
            KeyDefinition::char('2', '@', 0x03),
            KeyDefinition::char('3', '#', 0x04),
            KeyDefinition::char('4', '$', 0x05),
            KeyDefinition::char('5', '%', 0x06),
            KeyDefinition::char('6', '^', 0x07),
            KeyDefinition::char('7', '&', 0x08),
            KeyDefinition::char('8', '*', 0x09),
            KeyDefinition::char('9', '(', 0x0A),
            KeyDefinition::char('0', ')', 0x0B),
            KeyDefinition::char('-', '_', 0x0C),
            KeyDefinition::char('=', '+', 0x0D),
            KeyDefinition::special(KeyType::Backspace, "⌫", 2.0, 0x0E),
        ];

        // Row 2: QWERTY row
        let row2 = vec![
            KeyDefinition::special(KeyType::Tab, "Tab", 1.5, 0x0F),
            KeyDefinition::char('q', 'Q', 0x10),
            KeyDefinition::char('w', 'W', 0x11),
            KeyDefinition::char('e', 'E', 0x12),
            KeyDefinition::char('r', 'R', 0x13),
            KeyDefinition::char('t', 'T', 0x14),
            KeyDefinition::char('y', 'Y', 0x15),
            KeyDefinition::char('u', 'U', 0x16),
            KeyDefinition::char('i', 'I', 0x17),
            KeyDefinition::char('o', 'O', 0x18),
            KeyDefinition::char('p', 'P', 0x19),
            KeyDefinition::char('[', '{', 0x1A),
            KeyDefinition::char(']', '}', 0x1B),
            KeyDefinition::char('\\', '|', 0x2B),
        ];

        // Row 3: ASDF row
        let row3 = vec![
            KeyDefinition::special(KeyType::CapsLock, "Caps", 1.75, 0x3A),
            KeyDefinition::char('a', 'A', 0x1E),
            KeyDefinition::char('s', 'S', 0x1F),
            KeyDefinition::char('d', 'D', 0x20),
            KeyDefinition::char('f', 'F', 0x21),
            KeyDefinition::char('g', 'G', 0x22),
            KeyDefinition::char('h', 'H', 0x23),
            KeyDefinition::char('j', 'J', 0x24),
            KeyDefinition::char('k', 'K', 0x25),
            KeyDefinition::char('l', 'L', 0x26),
            KeyDefinition::char(';', ':', 0x27),
            KeyDefinition::char('\'', '"', 0x28),
            KeyDefinition::special(KeyType::Enter, "Enter", 2.25, 0x1C),
        ];

        // Row 4: ZXCV row
        let row4 = vec![
            KeyDefinition::special(KeyType::Shift, "Shift", 2.25, 0x2A),
            KeyDefinition::char('z', 'Z', 0x2C),
            KeyDefinition::char('x', 'X', 0x2D),
            KeyDefinition::char('c', 'C', 0x2E),
            KeyDefinition::char('v', 'V', 0x2F),
            KeyDefinition::char('b', 'B', 0x30),
            KeyDefinition::char('n', 'N', 0x31),
            KeyDefinition::char('m', 'M', 0x32),
            KeyDefinition::char(',', '<', 0x33),
            KeyDefinition::char('.', '>', 0x34),
            KeyDefinition::char('/', '?', 0x35),
            KeyDefinition::special(KeyType::Shift, "Shift", 2.75, 0x36),
        ];

        // Row 5: Bottom row
        let row5 = vec![
            KeyDefinition::special(KeyType::Ctrl, "Ctrl", 1.25, 0x1D),
            KeyDefinition::special(KeyType::Super, "⊞", 1.25, 0x5B),
            KeyDefinition::special(KeyType::Alt, "Alt", 1.25, 0x38),
            KeyDefinition::special(KeyType::Space, " ", 6.25, 0x39),
            KeyDefinition::special(KeyType::Alt, "Alt", 1.25, 0x38),
            KeyDefinition::special(KeyType::Super, "⊞", 1.25, 0x5C),
            KeyDefinition::special(KeyType::Settings, "⚙", 1.0, 0x00),
            KeyDefinition::special(KeyType::Close, "✕", 1.0, 0x00),
        ];

        // Convert to visual keys
        self.add_key_row(row1, 0);
        self.add_key_row(row2, 1);
        self.add_key_row(row3, 2);
        self.add_key_row(row4, 3);
        self.add_key_row(row5, 4);
    }

    /// Load AZERTY layout (French)
    fn load_azerty_layout(&mut self) {
        // Similar to QWERTY but with French layout
        // Row 1: Number row (different from QWERTY)
        let row1 = vec![
            KeyDefinition::char('²', '³', 0x29),
            KeyDefinition::char('&', '1', 0x02),
            KeyDefinition::char('é', '2', 0x03),
            KeyDefinition::char('"', '3', 0x04),
            KeyDefinition::char('\'', '4', 0x05),
            KeyDefinition::char('(', '5', 0x06),
            KeyDefinition::char('-', '6', 0x07),
            KeyDefinition::char('è', '7', 0x08),
            KeyDefinition::char('_', '8', 0x09),
            KeyDefinition::char('ç', '9', 0x0A),
            KeyDefinition::char('à', '0', 0x0B),
            KeyDefinition::char(')', '°', 0x0C),
            KeyDefinition::char('=', '+', 0x0D),
            KeyDefinition::special(KeyType::Backspace, "⌫", 2.0, 0x0E),
        ];

        // Row 2: AZERTY row
        let row2 = vec![
            KeyDefinition::special(KeyType::Tab, "Tab", 1.5, 0x0F),
            KeyDefinition::char('a', 'A', 0x10),
            KeyDefinition::char('z', 'Z', 0x11),
            KeyDefinition::char('e', 'E', 0x12),
            KeyDefinition::char('r', 'R', 0x13),
            KeyDefinition::char('t', 'T', 0x14),
            KeyDefinition::char('y', 'Y', 0x15),
            KeyDefinition::char('u', 'U', 0x16),
            KeyDefinition::char('i', 'I', 0x17),
            KeyDefinition::char('o', 'O', 0x18),
            KeyDefinition::char('p', 'P', 0x19),
            KeyDefinition::char('^', '¨', 0x1A),
            KeyDefinition::char('$', '£', 0x1B),
            KeyDefinition::char('*', 'µ', 0x2B),
        ];

        // Row 3
        let row3 = vec![
            KeyDefinition::special(KeyType::CapsLock, "Caps", 1.75, 0x3A),
            KeyDefinition::char('q', 'Q', 0x1E),
            KeyDefinition::char('s', 'S', 0x1F),
            KeyDefinition::char('d', 'D', 0x20),
            KeyDefinition::char('f', 'F', 0x21),
            KeyDefinition::char('g', 'G', 0x22),
            KeyDefinition::char('h', 'H', 0x23),
            KeyDefinition::char('j', 'J', 0x24),
            KeyDefinition::char('k', 'K', 0x25),
            KeyDefinition::char('l', 'L', 0x26),
            KeyDefinition::char('m', 'M', 0x27),
            KeyDefinition::char('ù', '%', 0x28),
            KeyDefinition::special(KeyType::Enter, "Enter", 2.25, 0x1C),
        ];

        // Row 4
        let row4 = vec![
            KeyDefinition::special(KeyType::Shift, "Shift", 2.25, 0x2A),
            KeyDefinition::char('w', 'W', 0x2C),
            KeyDefinition::char('x', 'X', 0x2D),
            KeyDefinition::char('c', 'C', 0x2E),
            KeyDefinition::char('v', 'V', 0x2F),
            KeyDefinition::char('b', 'B', 0x30),
            KeyDefinition::char('n', 'N', 0x31),
            KeyDefinition::char(',', '?', 0x32),
            KeyDefinition::char(';', '.', 0x33),
            KeyDefinition::char(':', '/', 0x34),
            KeyDefinition::char('!', '§', 0x35),
            KeyDefinition::special(KeyType::Shift, "Shift", 2.75, 0x36),
        ];

        // Row 5
        let row5 = vec![
            KeyDefinition::special(KeyType::Ctrl, "Ctrl", 1.25, 0x1D),
            KeyDefinition::special(KeyType::Super, "⊞", 1.25, 0x5B),
            KeyDefinition::special(KeyType::Alt, "Alt", 1.25, 0x38),
            KeyDefinition::special(KeyType::Space, " ", 6.25, 0x39),
            KeyDefinition::special(KeyType::Alt, "Alt", 1.25, 0x38),
            KeyDefinition::special(KeyType::Super, "⊞", 1.25, 0x5C),
            KeyDefinition::special(KeyType::Settings, "⚙", 1.0, 0x00),
            KeyDefinition::special(KeyType::Close, "✕", 1.0, 0x00),
        ];

        self.add_key_row(row1, 0);
        self.add_key_row(row2, 1);
        self.add_key_row(row3, 2);
        self.add_key_row(row4, 3);
        self.add_key_row(row5, 4);
    }

    /// Load QWERTZ layout (German)
    fn load_qwertz_layout(&mut self) {
        // QWERTZ has Y and Z swapped
        let row1 = vec![
            KeyDefinition::char('^', '°', 0x29),
            KeyDefinition::char('1', '!', 0x02),
            KeyDefinition::char('2', '"', 0x03),
            KeyDefinition::char('3', '§', 0x04),
            KeyDefinition::char('4', '$', 0x05),
            KeyDefinition::char('5', '%', 0x06),
            KeyDefinition::char('6', '&', 0x07),
            KeyDefinition::char('7', '/', 0x08),
            KeyDefinition::char('8', '(', 0x09),
            KeyDefinition::char('9', ')', 0x0A),
            KeyDefinition::char('0', '=', 0x0B),
            KeyDefinition::char('ß', '?', 0x0C),
            KeyDefinition::char('´', '`', 0x0D),
            KeyDefinition::special(KeyType::Backspace, "⌫", 2.0, 0x0E),
        ];

        let row2 = vec![
            KeyDefinition::special(KeyType::Tab, "Tab", 1.5, 0x0F),
            KeyDefinition::char('q', 'Q', 0x10),
            KeyDefinition::char('w', 'W', 0x11),
            KeyDefinition::char('e', 'E', 0x12),
            KeyDefinition::char('r', 'R', 0x13),
            KeyDefinition::char('t', 'T', 0x14),
            KeyDefinition::char('z', 'Z', 0x15), // Z instead of Y
            KeyDefinition::char('u', 'U', 0x16),
            KeyDefinition::char('i', 'I', 0x17),
            KeyDefinition::char('o', 'O', 0x18),
            KeyDefinition::char('p', 'P', 0x19),
            KeyDefinition::char('ü', 'Ü', 0x1A),
            KeyDefinition::char('+', '*', 0x1B),
            KeyDefinition::char('#', '\'', 0x2B),
        ];

        let row3 = vec![
            KeyDefinition::special(KeyType::CapsLock, "Caps", 1.75, 0x3A),
            KeyDefinition::char('a', 'A', 0x1E),
            KeyDefinition::char('s', 'S', 0x1F),
            KeyDefinition::char('d', 'D', 0x20),
            KeyDefinition::char('f', 'F', 0x21),
            KeyDefinition::char('g', 'G', 0x22),
            KeyDefinition::char('h', 'H', 0x23),
            KeyDefinition::char('j', 'J', 0x24),
            KeyDefinition::char('k', 'K', 0x25),
            KeyDefinition::char('l', 'L', 0x26),
            KeyDefinition::char('ö', 'Ö', 0x27),
            KeyDefinition::char('ä', 'Ä', 0x28),
            KeyDefinition::special(KeyType::Enter, "Enter", 2.25, 0x1C),
        ];

        let row4 = vec![
            KeyDefinition::special(KeyType::Shift, "Shift", 2.25, 0x2A),
            KeyDefinition::char('y', 'Y', 0x2C), // Y instead of Z
            KeyDefinition::char('x', 'X', 0x2D),
            KeyDefinition::char('c', 'C', 0x2E),
            KeyDefinition::char('v', 'V', 0x2F),
            KeyDefinition::char('b', 'B', 0x30),
            KeyDefinition::char('n', 'N', 0x31),
            KeyDefinition::char('m', 'M', 0x32),
            KeyDefinition::char(',', ';', 0x33),
            KeyDefinition::char('.', ':', 0x34),
            KeyDefinition::char('-', '_', 0x35),
            KeyDefinition::special(KeyType::Shift, "Shift", 2.75, 0x36),
        ];

        let row5 = vec![
            KeyDefinition::special(KeyType::Ctrl, "Ctrl", 1.25, 0x1D),
            KeyDefinition::special(KeyType::Super, "⊞", 1.25, 0x5B),
            KeyDefinition::special(KeyType::Alt, "Alt", 1.25, 0x38),
            KeyDefinition::special(KeyType::Space, " ", 6.25, 0x39),
            KeyDefinition::special(KeyType::Alt, "Alt", 1.25, 0x38),
            KeyDefinition::special(KeyType::Super, "⊞", 1.25, 0x5C),
            KeyDefinition::special(KeyType::Settings, "⚙", 1.0, 0x00),
            KeyDefinition::special(KeyType::Close, "✕", 1.0, 0x00),
        ];

        self.add_key_row(row1, 0);
        self.add_key_row(row2, 1);
        self.add_key_row(row3, 2);
        self.add_key_row(row4, 3);
        self.add_key_row(row5, 4);
    }

    /// Load Dvorak layout
    fn load_dvorak_layout(&mut self) {
        let row1 = vec![
            KeyDefinition::char('`', '~', 0x29),
            KeyDefinition::char('1', '!', 0x02),
            KeyDefinition::char('2', '@', 0x03),
            KeyDefinition::char('3', '#', 0x04),
            KeyDefinition::char('4', '$', 0x05),
            KeyDefinition::char('5', '%', 0x06),
            KeyDefinition::char('6', '^', 0x07),
            KeyDefinition::char('7', '&', 0x08),
            KeyDefinition::char('8', '*', 0x09),
            KeyDefinition::char('9', '(', 0x0A),
            KeyDefinition::char('0', ')', 0x0B),
            KeyDefinition::char('[', '{', 0x0C),
            KeyDefinition::char(']', '}', 0x0D),
            KeyDefinition::special(KeyType::Backspace, "⌫", 2.0, 0x0E),
        ];

        let row2 = vec![
            KeyDefinition::special(KeyType::Tab, "Tab", 1.5, 0x0F),
            KeyDefinition::char('\'', '"', 0x10),
            KeyDefinition::char(',', '<', 0x11),
            KeyDefinition::char('.', '>', 0x12),
            KeyDefinition::char('p', 'P', 0x13),
            KeyDefinition::char('y', 'Y', 0x14),
            KeyDefinition::char('f', 'F', 0x15),
            KeyDefinition::char('g', 'G', 0x16),
            KeyDefinition::char('c', 'C', 0x17),
            KeyDefinition::char('r', 'R', 0x18),
            KeyDefinition::char('l', 'L', 0x19),
            KeyDefinition::char('/', '?', 0x1A),
            KeyDefinition::char('=', '+', 0x1B),
            KeyDefinition::char('\\', '|', 0x2B),
        ];

        let row3 = vec![
            KeyDefinition::special(KeyType::CapsLock, "Caps", 1.75, 0x3A),
            KeyDefinition::char('a', 'A', 0x1E),
            KeyDefinition::char('o', 'O', 0x1F),
            KeyDefinition::char('e', 'E', 0x20),
            KeyDefinition::char('u', 'U', 0x21),
            KeyDefinition::char('i', 'I', 0x22),
            KeyDefinition::char('d', 'D', 0x23),
            KeyDefinition::char('h', 'H', 0x24),
            KeyDefinition::char('t', 'T', 0x25),
            KeyDefinition::char('n', 'N', 0x26),
            KeyDefinition::char('s', 'S', 0x27),
            KeyDefinition::char('-', '_', 0x28),
            KeyDefinition::special(KeyType::Enter, "Enter", 2.25, 0x1C),
        ];

        let row4 = vec![
            KeyDefinition::special(KeyType::Shift, "Shift", 2.25, 0x2A),
            KeyDefinition::char(';', ':', 0x2C),
            KeyDefinition::char('q', 'Q', 0x2D),
            KeyDefinition::char('j', 'J', 0x2E),
            KeyDefinition::char('k', 'K', 0x2F),
            KeyDefinition::char('x', 'X', 0x30),
            KeyDefinition::char('b', 'B', 0x31),
            KeyDefinition::char('m', 'M', 0x32),
            KeyDefinition::char('w', 'W', 0x33),
            KeyDefinition::char('v', 'V', 0x34),
            KeyDefinition::char('z', 'Z', 0x35),
            KeyDefinition::special(KeyType::Shift, "Shift", 2.75, 0x36),
        ];

        let row5 = vec![
            KeyDefinition::special(KeyType::Ctrl, "Ctrl", 1.25, 0x1D),
            KeyDefinition::special(KeyType::Super, "⊞", 1.25, 0x5B),
            KeyDefinition::special(KeyType::Alt, "Alt", 1.25, 0x38),
            KeyDefinition::special(KeyType::Space, " ", 6.25, 0x39),
            KeyDefinition::special(KeyType::Alt, "Alt", 1.25, 0x38),
            KeyDefinition::special(KeyType::Super, "⊞", 1.25, 0x5C),
            KeyDefinition::special(KeyType::Settings, "⚙", 1.0, 0x00),
            KeyDefinition::special(KeyType::Close, "✕", 1.0, 0x00),
        ];

        self.add_key_row(row1, 0);
        self.add_key_row(row2, 1);
        self.add_key_row(row3, 2);
        self.add_key_row(row4, 3);
        self.add_key_row(row5, 4);
    }

    /// Load Colemak layout
    fn load_colemak_layout(&mut self) {
        // Colemak is similar to QWERTY but with different letter positions
        let row1 = vec![
            KeyDefinition::char('`', '~', 0x29),
            KeyDefinition::char('1', '!', 0x02),
            KeyDefinition::char('2', '@', 0x03),
            KeyDefinition::char('3', '#', 0x04),
            KeyDefinition::char('4', '$', 0x05),
            KeyDefinition::char('5', '%', 0x06),
            KeyDefinition::char('6', '^', 0x07),
            KeyDefinition::char('7', '&', 0x08),
            KeyDefinition::char('8', '*', 0x09),
            KeyDefinition::char('9', '(', 0x0A),
            KeyDefinition::char('0', ')', 0x0B),
            KeyDefinition::char('-', '_', 0x0C),
            KeyDefinition::char('=', '+', 0x0D),
            KeyDefinition::special(KeyType::Backspace, "⌫", 2.0, 0x0E),
        ];

        let row2 = vec![
            KeyDefinition::special(KeyType::Tab, "Tab", 1.5, 0x0F),
            KeyDefinition::char('q', 'Q', 0x10),
            KeyDefinition::char('w', 'W', 0x11),
            KeyDefinition::char('f', 'F', 0x12),
            KeyDefinition::char('p', 'P', 0x13),
            KeyDefinition::char('g', 'G', 0x14),
            KeyDefinition::char('j', 'J', 0x15),
            KeyDefinition::char('l', 'L', 0x16),
            KeyDefinition::char('u', 'U', 0x17),
            KeyDefinition::char('y', 'Y', 0x18),
            KeyDefinition::char(';', ':', 0x19),
            KeyDefinition::char('[', '{', 0x1A),
            KeyDefinition::char(']', '}', 0x1B),
            KeyDefinition::char('\\', '|', 0x2B),
        ];

        let row3 = vec![
            KeyDefinition::special(KeyType::CapsLock, "Caps", 1.75, 0x3A),
            KeyDefinition::char('a', 'A', 0x1E),
            KeyDefinition::char('r', 'R', 0x1F),
            KeyDefinition::char('s', 'S', 0x20),
            KeyDefinition::char('t', 'T', 0x21),
            KeyDefinition::char('d', 'D', 0x22),
            KeyDefinition::char('h', 'H', 0x23),
            KeyDefinition::char('n', 'N', 0x24),
            KeyDefinition::char('e', 'E', 0x25),
            KeyDefinition::char('i', 'I', 0x26),
            KeyDefinition::char('o', 'O', 0x27),
            KeyDefinition::char('\'', '"', 0x28),
            KeyDefinition::special(KeyType::Enter, "Enter", 2.25, 0x1C),
        ];

        let row4 = vec![
            KeyDefinition::special(KeyType::Shift, "Shift", 2.25, 0x2A),
            KeyDefinition::char('z', 'Z', 0x2C),
            KeyDefinition::char('x', 'X', 0x2D),
            KeyDefinition::char('c', 'C', 0x2E),
            KeyDefinition::char('v', 'V', 0x2F),
            KeyDefinition::char('b', 'B', 0x30),
            KeyDefinition::char('k', 'K', 0x31),
            KeyDefinition::char('m', 'M', 0x32),
            KeyDefinition::char(',', '<', 0x33),
            KeyDefinition::char('.', '>', 0x34),
            KeyDefinition::char('/', '?', 0x35),
            KeyDefinition::special(KeyType::Shift, "Shift", 2.75, 0x36),
        ];

        let row5 = vec![
            KeyDefinition::special(KeyType::Ctrl, "Ctrl", 1.25, 0x1D),
            KeyDefinition::special(KeyType::Super, "⊞", 1.25, 0x5B),
            KeyDefinition::special(KeyType::Alt, "Alt", 1.25, 0x38),
            KeyDefinition::special(KeyType::Space, " ", 6.25, 0x39),
            KeyDefinition::special(KeyType::Alt, "Alt", 1.25, 0x38),
            KeyDefinition::special(KeyType::Super, "⊞", 1.25, 0x5C),
            KeyDefinition::special(KeyType::Settings, "⚙", 1.0, 0x00),
            KeyDefinition::special(KeyType::Close, "✕", 1.0, 0x00),
        ];

        self.add_key_row(row1, 0);
        self.add_key_row(row2, 1);
        self.add_key_row(row3, 2);
        self.add_key_row(row4, 3);
        self.add_key_row(row5, 4);
    }

    /// Load ABNT2 layout (Brazilian Portuguese)
    fn load_abnt2_layout(&mut self) {
        let row1 = vec![
            KeyDefinition::char('\'', '"', 0x29),
            KeyDefinition::char('1', '!', 0x02),
            KeyDefinition::char('2', '@', 0x03),
            KeyDefinition::char('3', '#', 0x04),
            KeyDefinition::char('4', '$', 0x05),
            KeyDefinition::char('5', '%', 0x06),
            KeyDefinition::char('6', '¨', 0x07),
            KeyDefinition::char('7', '&', 0x08),
            KeyDefinition::char('8', '*', 0x09),
            KeyDefinition::char('9', '(', 0x0A),
            KeyDefinition::char('0', ')', 0x0B),
            KeyDefinition::char('-', '_', 0x0C),
            KeyDefinition::char('=', '+', 0x0D),
            KeyDefinition::special(KeyType::Backspace, "⌫", 2.0, 0x0E),
        ];

        let row2 = vec![
            KeyDefinition::special(KeyType::Tab, "Tab", 1.5, 0x0F),
            KeyDefinition::char('q', 'Q', 0x10),
            KeyDefinition::char('w', 'W', 0x11),
            KeyDefinition::char('e', 'E', 0x12),
            KeyDefinition::char('r', 'R', 0x13),
            KeyDefinition::char('t', 'T', 0x14),
            KeyDefinition::char('y', 'Y', 0x15),
            KeyDefinition::char('u', 'U', 0x16),
            KeyDefinition::char('i', 'I', 0x17),
            KeyDefinition::char('o', 'O', 0x18),
            KeyDefinition::char('p', 'P', 0x19),
            KeyDefinition::char('´', '`', 0x1A),
            KeyDefinition::char('[', '{', 0x1B),
            KeyDefinition::char(']', '}', 0x2B),
        ];

        let row3 = vec![
            KeyDefinition::special(KeyType::CapsLock, "Caps", 1.75, 0x3A),
            KeyDefinition::char('a', 'A', 0x1E),
            KeyDefinition::char('s', 'S', 0x1F),
            KeyDefinition::char('d', 'D', 0x20),
            KeyDefinition::char('f', 'F', 0x21),
            KeyDefinition::char('g', 'G', 0x22),
            KeyDefinition::char('h', 'H', 0x23),
            KeyDefinition::char('j', 'J', 0x24),
            KeyDefinition::char('k', 'K', 0x25),
            KeyDefinition::char('l', 'L', 0x26),
            KeyDefinition::char('ç', 'Ç', 0x27),
            KeyDefinition::char('~', '^', 0x28),
            KeyDefinition::special(KeyType::Enter, "Enter", 2.25, 0x1C),
        ];

        let row4 = vec![
            KeyDefinition::special(KeyType::Shift, "Shift", 2.25, 0x2A),
            KeyDefinition::char('\\', '|', 0x56),
            KeyDefinition::char('z', 'Z', 0x2C),
            KeyDefinition::char('x', 'X', 0x2D),
            KeyDefinition::char('c', 'C', 0x2E),
            KeyDefinition::char('v', 'V', 0x2F),
            KeyDefinition::char('b', 'B', 0x30),
            KeyDefinition::char('n', 'N', 0x31),
            KeyDefinition::char('m', 'M', 0x32),
            KeyDefinition::char(',', '<', 0x33),
            KeyDefinition::char('.', '>', 0x34),
            KeyDefinition::char(';', ':', 0x35),
            KeyDefinition::special(KeyType::Shift, "Shift", 1.75, 0x36),
        ];

        let row5 = vec![
            KeyDefinition::special(KeyType::Ctrl, "Ctrl", 1.25, 0x1D),
            KeyDefinition::special(KeyType::Super, "⊞", 1.25, 0x5B),
            KeyDefinition::special(KeyType::Alt, "Alt", 1.25, 0x38),
            KeyDefinition::special(KeyType::Space, " ", 6.25, 0x39),
            KeyDefinition::special(KeyType::Alt, "Alt", 1.25, 0x38),
            KeyDefinition::special(KeyType::Super, "⊞", 1.25, 0x5C),
            KeyDefinition::special(KeyType::Settings, "⚙", 1.0, 0x00),
            KeyDefinition::special(KeyType::Close, "✕", 1.0, 0x00),
        ];

        self.add_key_row(row1, 0);
        self.add_key_row(row2, 1);
        self.add_key_row(row3, 2);
        self.add_key_row(row4, 3);
        self.add_key_row(row5, 4);
    }

    /// Add a row of keys
    fn add_key_row(&mut self, definitions: Vec<KeyDefinition>, row: usize) {
        let visuals: Vec<KeyVisual> = definitions.into_iter().enumerate().map(|(col, key)| {
            KeyVisual {
                key,
                state: KeyState::Normal,
                x: 0,
                y: 0,
                width: 0,
                height: 0,
                row,
                col,
            }
        }).collect();
        self.keys.push(visuals);
    }

    /// Recalculate key positions
    fn recalculate_positions(&mut self) {
        let (kb_x, kb_y, kb_width) = self.calculate_keyboard_bounds();

        // Calculate base key width from keyboard width
        // A standard row is about 15 units wide
        let base_key_width = (kb_width - (14 * self.config.key_spacing)) / 15;
        let key_height = self.config.key_height;
        let spacing = self.config.key_spacing;

        let mut y = kb_y;

        // Add space for suggestions bar
        if self.config.show_suggestions {
            y += 40 + spacing;
        }

        for row in &mut self.keys {
            let mut x = kb_x;

            for key_visual in row.iter_mut() {
                let width = (base_key_width as f32 * key_visual.key.width) as u32;

                key_visual.x = x as i32;
                key_visual.y = y as i32;
                key_visual.width = width;
                key_visual.height = key_height;

                x += width + spacing;
            }

            y += key_height + spacing;
        }
    }

    /// Calculate keyboard bounds based on position
    fn calculate_keyboard_bounds(&self) -> (u32, u32, u32) {
        match self.config.position {
            KeyboardPosition::Bottom => {
                let height = self.calculate_total_height();
                let y = self.screen_height - height;
                (0, y, self.screen_width)
            }
            KeyboardPosition::Top => {
                (0, 0, self.screen_width)
            }
            KeyboardPosition::Floating => {
                (self.config.float_x as u32, self.config.float_y as u32, self.config.float_width)
            }
            KeyboardPosition::Left => {
                let width = self.screen_width / 2;
                (0, self.screen_height / 2, width)
            }
            KeyboardPosition::Right => {
                let width = self.screen_width / 2;
                (self.screen_width / 2, self.screen_height / 2, width)
            }
        }
    }

    /// Calculate total keyboard height
    fn calculate_total_height(&self) -> u32 {
        let rows = self.keys.len() as u32;
        let mut height = rows * self.config.key_height + (rows - 1) * self.config.key_spacing;

        if self.config.show_suggestions {
            height += 40 + self.config.key_spacing;
        }

        height + 10 // padding
    }

    /// Load basic dictionary for predictions
    fn load_dictionary(&mut self) {
        // Add common English words
        let words = [
            ("the", 1000), ("be", 900), ("to", 850), ("of", 800), ("and", 750),
            ("a", 700), ("in", 650), ("that", 600), ("have", 550), ("I", 500),
            ("it", 490), ("for", 480), ("not", 470), ("on", 460), ("with", 450),
            ("he", 440), ("as", 430), ("you", 420), ("do", 410), ("at", 400),
            ("this", 390), ("but", 380), ("his", 370), ("by", 360), ("from", 350),
            ("they", 340), ("we", 330), ("say", 320), ("her", 310), ("she", 300),
            ("hello", 200), ("world", 190), ("computer", 180), ("keyboard", 170),
            ("password", 160), ("email", 150), ("message", 140), ("search", 130),
        ];

        for (word, freq) in words.iter() {
            self.dictionary.insert(String::from(*word), *freq);
        }
    }

    /// Update predictions based on current input
    fn update_predictions(&mut self) {
        self.predictions.clear();

        if self.input_buffer.is_empty() {
            return;
        }

        let prefix = self.input_buffer.to_lowercase();
        let mut matches: Vec<_> = self.dictionary
            .iter()
            .filter(|(word, _)| word.starts_with(&prefix))
            .map(|(word, freq)| {
                Prediction {
                    word: word.clone(),
                    confidence: (*freq / 10).min(100) as u8,
                    frequency: *freq,
                }
            })
            .collect();

        // Sort by frequency
        matches.sort_by(|a, b| b.frequency.cmp(&a.frequency));

        // Take top N
        self.predictions = matches.into_iter().take(self.config.max_suggestions).collect();
    }

    /// Process mouse/touch at position
    pub fn process_input(&mut self, x: i32, y: i32, pressed: bool) -> Option<KeyEventOutput> {
        if !self.visible {
            return None;
        }

        // Find key at position
        let key_index = self.find_key_at(x, y);

        if pressed {
            if let Some((row, col)) = key_index {
                // Update visual state
                self.keys[row][col].state = KeyState::Pressed;

                // Process the key press
                return self.handle_key_press(row, col);
            }
        } else {
            // Reset all key states to normal
            for row in &mut self.keys {
                for key in row.iter_mut() {
                    if key.state == KeyState::Pressed {
                        key.state = KeyState::Normal;
                    }
                }
            }
        }

        None
    }

    /// Process hover (for dwell clicking)
    pub fn process_hover(&mut self, x: i32, y: i32) {
        if !self.visible || !self.config.dwell_enabled {
            return;
        }

        let key_index = self.find_key_at(x, y);
        let now = crate::time::uptime_ms();

        if key_index != self.dwell_key_index {
            // Moved to different key, reset dwell timer
            self.dwell_key_index = key_index;
            self.dwell_start_ms = now;

            // Update hover states
            for row in &mut self.keys {
                for key in row.iter_mut() {
                    key.state = KeyState::Normal;
                }
            }

            if let Some((row, col)) = key_index {
                self.keys[row][col].state = KeyState::Hovered;
            }
        } else if let Some((row, col)) = key_index {
            // Same key, check dwell time
            if now - self.dwell_start_ms >= self.config.dwell_time_ms as u64 {
                // Dwell click!
                self.stats.dwell_clicks += 1;
                self.dwell_start_ms = now; // Reset for next dwell

                // Trigger key press
                if let Some(output) = self.handle_key_press(row, col) {
                    // Emit the output via callback
                    self.emit_key_event(&output);
                }
            }
        }
    }

    /// Find key at screen position
    fn find_key_at(&self, x: i32, y: i32) -> Option<(usize, usize)> {
        for (row_idx, row) in self.keys.iter().enumerate() {
            for (col_idx, key) in row.iter().enumerate() {
                if key.contains(x, y) {
                    return Some((row_idx, col_idx));
                }
            }
        }
        None
    }

    /// Handle key press
    fn handle_key_press(&mut self, row: usize, col: usize) -> Option<KeyEventOutput> {
        // Extract key info before any mutable borrows
        let key_type = self.keys[row][col].key.key_type;
        let key_code = self.keys[row][col].key.key_code;
        let normal_char = self.keys[row][col].key.normal;
        let shifted_char = self.keys[row][col].key.shifted;

        self.stats.keys_pressed += 1;

        match key_type {
            KeyType::Character | KeyType::Space => {
                let c = if self.shift_active || self.shift_locked {
                    shifted_char
                } else {
                    normal_char
                };

                // Add to buffer for predictions
                if c != ' ' {
                    self.input_buffer.push(c);
                } else {
                    // Space completes word
                    if !self.input_buffer.is_empty() {
                        self.stats.words_completed += 1;
                    }
                    self.input_buffer.clear();
                }

                self.update_predictions();
                self.stats.chars_typed += 1;

                // Reset shift if not locked
                if self.shift_active && !self.shift_locked {
                    self.shift_active = false;
                }

                Some(KeyEventOutput::Character(c, key_code))
            }
            KeyType::Backspace => {
                self.input_buffer.pop();
                self.update_predictions();
                self.stats.backspaces += 1;
                Some(KeyEventOutput::Special(KeyType::Backspace, true))
            }
            KeyType::Enter => {
                self.input_buffer.clear();
                self.predictions.clear();
                self.stats.words_completed += 1;
                Some(KeyEventOutput::Special(KeyType::Enter, true))
            }
            KeyType::Tab => {
                Some(KeyEventOutput::Special(KeyType::Tab, true))
            }
            KeyType::Shift => {
                if self.shift_active {
                    // Second press: lock
                    self.shift_locked = !self.shift_locked;
                    self.shift_active = self.shift_locked;
                } else {
                    // First press: activate
                    self.shift_active = true;
                    self.shift_locked = false;
                }

                // Update shift key visual state
                for row in &mut self.keys {
                    for key in row.iter_mut() {
                        if key.key.key_type == KeyType::Shift {
                            key.state = if self.shift_locked {
                                KeyState::Locked
                            } else if self.shift_active {
                                KeyState::Pressed
                            } else {
                                KeyState::Normal
                            };
                        }
                    }
                }

                None
            }
            KeyType::CapsLock => {
                self.shift_locked = !self.shift_locked;
                self.shift_active = self.shift_locked;

                // Update caps lock visual state
                for row in &mut self.keys {
                    for key in row.iter_mut() {
                        if key.key.key_type == KeyType::CapsLock {
                            key.state = if self.shift_locked {
                                KeyState::Locked
                            } else {
                                KeyState::Normal
                            };
                        }
                    }
                }

                Some(KeyEventOutput::Special(KeyType::CapsLock, self.shift_locked))
            }
            KeyType::Ctrl => {
                self.ctrl_active = !self.ctrl_active;
                None
            }
            KeyType::Alt => {
                self.alt_active = !self.alt_active;
                None
            }
            KeyType::Super => {
                self.super_active = !self.super_active;
                None
            }
            KeyType::Close => {
                self.hide();
                None
            }
            KeyType::Minimize => {
                // TODO: Implement minimize
                None
            }
            KeyType::Settings => {
                // TODO: Show settings
                None
            }
            _ => None,
        }
    }

    /// Emit key event via callbacks
    fn emit_key_event(&self, output: &KeyEventOutput) {
        match output {
            KeyEventOutput::Character(c, code) => {
                if let Some(callback) = self.on_key_press {
                    callback(*c, *code);
                }
            }
            KeyEventOutput::Special(key_type, active) => {
                if let Some(callback) = self.on_special_key {
                    callback(*key_type, *active);
                }
            }
        }
    }

    /// Accept a prediction
    pub fn accept_prediction(&mut self, index: usize) -> Option<String> {
        if index >= self.predictions.len() {
            return None;
        }

        let prediction = &self.predictions[index];
        let word = prediction.word.clone();

        // Calculate what characters to emit (word minus what's already typed)
        let remaining = if word.len() > self.input_buffer.len() {
            &word[self.input_buffer.len()..]
        } else {
            ""
        };

        self.input_buffer.clear();
        self.predictions.clear();
        self.stats.predictions_accepted += 1;
        self.stats.words_completed += 1;

        Some(String::from(remaining))
    }

    /// Set layout
    pub fn set_layout(&mut self, layout: KeyboardLayout) {
        self.config.layout = layout;
        self.load_layout();
        crate::kprintln!("[osk] Layout changed to {}", layout.name());
    }

    /// Get current layout
    pub fn layout(&self) -> KeyboardLayout {
        self.config.layout
    }

    /// Set mode
    pub fn set_mode(&mut self, mode: KeyboardMode) {
        self.config.mode = mode;
        // Reload layout for mode-specific changes
        self.load_layout();
    }

    /// Get current mode
    pub fn mode(&self) -> KeyboardMode {
        self.config.mode
    }

    /// Set position
    pub fn set_position(&mut self, position: KeyboardPosition) {
        self.config.position = position;
        self.recalculate_positions();
    }

    /// Get current position
    pub fn position(&self) -> KeyboardPosition {
        self.config.position
    }

    /// Set theme
    pub fn set_theme(&mut self, theme: OskTheme) {
        self.config.theme = theme;
    }

    /// Get current theme
    pub fn theme(&self) -> &OskTheme {
        &self.config.theme
    }

    /// Enable/disable dwell clicking
    pub fn set_dwell_enabled(&mut self, enabled: bool) {
        self.config.dwell_enabled = enabled;
    }

    /// Set dwell time
    pub fn set_dwell_time(&mut self, ms: u32) {
        self.config.dwell_time_ms = ms.max(200).min(3000);
    }

    /// Set key press callback
    pub fn set_key_press_callback(&mut self, callback: fn(char, u8)) {
        self.on_key_press = Some(callback);
    }

    /// Set special key callback
    pub fn set_special_key_callback(&mut self, callback: fn(KeyType, bool)) {
        self.on_special_key = Some(callback);
    }

    /// Get configuration
    pub fn config(&self) -> &OskConfig {
        &self.config
    }

    /// Set configuration
    pub fn set_config(&mut self, config: OskConfig) {
        self.config = config;
        self.load_layout();
    }

    /// Get statistics
    pub fn stats(&self) -> &OskStats {
        &self.stats
    }

    /// Get keys for rendering
    pub fn get_keys(&self) -> &Vec<Vec<KeyVisual>> {
        &self.keys
    }

    /// Get predictions
    pub fn get_predictions(&self) -> &[Prediction] {
        &self.predictions
    }

    /// Get keyboard bounds
    pub fn get_bounds(&self) -> (i32, i32, u32, u32) {
        let (x, y, width) = self.calculate_keyboard_bounds();
        let height = self.calculate_total_height();
        (x as i32, y as i32, width, height)
    }

    /// Check if shift is active
    pub fn is_shift_active(&self) -> bool {
        self.shift_active || self.shift_locked
    }

    /// Format status string
    pub fn format_status(&self) -> String {
        format!(
            "On-Screen Keyboard:\n\
             Enabled: {}\n\
             Visible: {}\n\
             Layout: {}\n\
             Mode: {}\n\
             Position: {}\n\
             Dwell: {} ({}ms)\n\
             Keys pressed: {}\n\
             Predictions accepted: {}",
            if self.config.enabled { "Yes" } else { "No" },
            if self.visible { "Yes" } else { "No" },
            self.config.layout.name(),
            self.config.mode.name(),
            self.config.position.name(),
            if self.config.dwell_enabled { "Enabled" } else { "Disabled" },
            self.config.dwell_time_ms,
            self.stats.keys_pressed,
            self.stats.predictions_accepted
        )
    }
}

/// Key event output
#[derive(Debug, Clone)]
pub enum KeyEventOutput {
    /// Character key pressed
    Character(char, u8),
    /// Special key pressed
    Special(KeyType, bool),
}

/// Global OSK instance
static OSK: IrqSafeMutex<Option<OnScreenKeyboard>> = IrqSafeMutex::new(None);

/// Initialize on-screen keyboard
pub fn init() {
    let mut osk = OnScreenKeyboard::new();
    osk.init();
    *OSK.lock() = Some(osk);
}

/// Enable OSK
pub fn enable() {
    if let Some(ref mut osk) = *OSK.lock() {
        osk.enable();
    }
}

/// Disable OSK
pub fn disable() {
    if let Some(ref mut osk) = *OSK.lock() {
        osk.disable();
    }
}

/// Check if enabled
pub fn is_enabled() -> bool {
    OSK.lock().as_ref().map(|osk| osk.is_enabled()).unwrap_or(false)
}

/// Show OSK
pub fn show() {
    if let Some(ref mut osk) = *OSK.lock() {
        osk.show();
    }
}

/// Hide OSK
pub fn hide() {
    if let Some(ref mut osk) = *OSK.lock() {
        osk.hide();
    }
}

/// Toggle OSK visibility
pub fn toggle() {
    if let Some(ref mut osk) = *OSK.lock() {
        osk.toggle();
    }
}

/// Check if visible
pub fn is_visible() -> bool {
    OSK.lock().as_ref().map(|osk| osk.is_visible()).unwrap_or(false)
}

/// Set layout
pub fn set_layout(layout: KeyboardLayout) {
    if let Some(ref mut osk) = *OSK.lock() {
        osk.set_layout(layout);
    }
}

/// Get layout
pub fn get_layout() -> KeyboardLayout {
    OSK.lock().as_ref().map(|osk| osk.layout()).unwrap_or(KeyboardLayout::QwertyUs)
}

/// Get status string
pub fn status() -> String {
    OSK.lock().as_ref()
        .map(|osk| osk.format_status())
        .unwrap_or_else(|| String::from("On-Screen Keyboard: Not initialized"))
}

/// Get statistics
pub fn stats() -> Option<OskStats> {
    OSK.lock().as_ref().map(|osk| osk.stats().clone())
}
