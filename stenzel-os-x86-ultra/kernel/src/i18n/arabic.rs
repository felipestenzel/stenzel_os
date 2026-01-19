//! Arabic Input Method Engine
//!
//! Provides Arabic keyboard input with RTL (Right-to-Left) support
//! and contextual character shaping.

#![allow(dead_code)]

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::collections::BTreeMap;

use super::ibus::{
    InputMethodEngine, InputMethodType, InputMethodState,
    Candidate, InputEvent, InputResult,
};

/// Arabic keyboard layout type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArabicLayout {
    /// Standard Arabic keyboard
    Standard,
    /// IBM Arabic layout
    Ibm,
    /// Arabic-French (AZERTY based)
    ArabicFrench,
}

impl ArabicLayout {
    pub fn name(&self) -> &'static str {
        match self {
            ArabicLayout::Standard => "Arabic Standard",
            ArabicLayout::Ibm => "Arabic IBM",
            ArabicLayout::ArabicFrench => "Arabic-French",
        }
    }
}

/// Arabic engine configuration
#[derive(Debug, Clone)]
pub struct ArabicConfig {
    /// Keyboard layout
    pub layout: ArabicLayout,
    /// Enable RTL direction
    pub rtl_enabled: bool,
    /// Auto-insert tashkeel (diacritics)
    pub auto_tashkeel: bool,
}

impl Default for ArabicConfig {
    fn default() -> Self {
        Self {
            layout: ArabicLayout::Standard,
            rtl_enabled: true,
            auto_tashkeel: false,
        }
    }
}

/// Arabic input engine
pub struct ArabicEngine {
    /// Configuration
    config: ArabicConfig,
    /// Current state
    state: InputMethodState,
    /// Keyboard mapping (key -> Arabic string, may be ligature)
    key_map: BTreeMap<char, String>,
    /// Shift key mapping
    shift_key_map: BTreeMap<char, String>,
}

impl ArabicEngine {
    /// Create a new Arabic engine
    pub fn new() -> Self {
        let mut engine = Self {
            config: ArabicConfig::default(),
            state: InputMethodState::Idle,
            key_map: BTreeMap::new(),
            shift_key_map: BTreeMap::new(),
        };
        engine.load_standard_layout();
        engine
    }

    /// Add key mapping
    fn add_key(&mut self, key: char, value: &str) {
        self.key_map.insert(key, value.to_string());
    }

    /// Add shift key mapping
    fn add_shift_key(&mut self, key: char, value: &str) {
        self.shift_key_map.insert(key, value.to_string());
    }

    /// Load standard Arabic keyboard layout
    fn load_standard_layout(&mut self) {
        // Row 1 (number row) - base
        self.add_key('`', "\u{0630}"); // ذ
        self.add_key('1', "\u{0661}"); // ١
        self.add_key('2', "\u{0662}"); // ٢
        self.add_key('3', "\u{0663}"); // ٣
        self.add_key('4', "\u{0664}"); // ٤
        self.add_key('5', "\u{0665}"); // ٥
        self.add_key('6', "\u{0666}"); // ٦
        self.add_key('7', "\u{0667}"); // ٧
        self.add_key('8', "\u{0668}"); // ٨
        self.add_key('9', "\u{0669}"); // ٩
        self.add_key('0', "\u{0660}"); // ٠
        self.add_key('-', "-");
        self.add_key('=', "=");

        // Row 1 - shifted
        self.add_shift_key('~', "\u{0651}"); // Shadda ّ
        self.add_shift_key('!', "!");
        self.add_shift_key('@', "@");
        self.add_shift_key('#', "#");
        self.add_shift_key('$', "$");
        self.add_shift_key('%', "\u{066A}"); // ٪
        self.add_shift_key('^', "^");
        self.add_shift_key('&', "&");
        self.add_shift_key('*', "*");
        self.add_shift_key('(', ")");
        self.add_shift_key(')', "(");
        self.add_shift_key('_', "_");
        self.add_shift_key('+', "+");

        // Row 2 (QWERTY) - base
        self.add_key('q', "\u{0636}"); // ض
        self.add_key('w', "\u{0635}"); // ص
        self.add_key('e', "\u{062B}"); // ث
        self.add_key('r', "\u{0642}"); // ق
        self.add_key('t', "\u{0641}"); // ف
        self.add_key('y', "\u{063A}"); // غ
        self.add_key('u', "\u{0639}"); // ع
        self.add_key('i', "\u{0647}"); // ه
        self.add_key('o', "\u{062E}"); // خ
        self.add_key('p', "\u{062D}"); // ح
        self.add_key('[', "\u{062C}"); // ج
        self.add_key(']', "\u{062F}"); // د
        self.add_key('\\', "\\");

        // Row 2 - shifted
        self.add_shift_key('Q', "\u{064E}"); // Fatha َ
        self.add_shift_key('W', "\u{064B}"); // Tanween Fatha ً
        self.add_shift_key('E', "\u{064F}"); // Damma ُ
        self.add_shift_key('R', "\u{064C}"); // Tanween Damma ٌ
        self.add_shift_key('T', "\u{0644}\u{0625}"); // لإ (Lam-Alef with Hamza below)
        self.add_shift_key('Y', "\u{0625}"); // إ (Alef with Hamza below)
        self.add_shift_key('U', "\u{2018}"); // ' (left single quote)
        self.add_shift_key('I', "\u{00F7}"); // ÷
        self.add_shift_key('O', "\u{00D7}"); // ×
        self.add_shift_key('P', "\u{061B}"); // ؛ (Arabic semicolon)
        self.add_shift_key('{', "<");
        self.add_shift_key('}', ">");
        self.add_shift_key('|', "|");

        // Row 3 (ASDF) - base
        self.add_key('a', "\u{0634}"); // ش
        self.add_key('s', "\u{0633}"); // س
        self.add_key('d', "\u{064A}"); // ي
        self.add_key('f', "\u{0628}"); // ب
        self.add_key('g', "\u{0644}"); // ل
        self.add_key('h', "\u{0627}"); // ا
        self.add_key('j', "\u{062A}"); // ت
        self.add_key('k', "\u{0646}"); // ن
        self.add_key('l', "\u{0645}"); // م
        self.add_key(';', "\u{0643}"); // ك
        self.add_key('\'', "\u{0637}"); // ط

        // Row 3 - shifted
        self.add_shift_key('A', "\u{0650}"); // Kasra ِ
        self.add_shift_key('S', "\u{064D}"); // Tanween Kasra ٍ
        self.add_shift_key('D', "]");
        self.add_shift_key('F', "[");
        self.add_shift_key('G', "\u{0644}\u{0623}"); // لأ (Lam-Alef with Hamza above)
        self.add_shift_key('H', "\u{0623}"); // أ (Alef with Hamza above)
        self.add_shift_key('J', "\u{0640}"); // ـ (Tatweel)
        self.add_shift_key('K', "\u{060C}"); // ، (Arabic comma)
        self.add_shift_key('L', "/");
        self.add_shift_key(':', ":");
        self.add_shift_key('"', "\"");

        // Row 4 (ZXCV) - base
        self.add_key('z', "\u{0626}"); // ئ
        self.add_key('x', "\u{0621}"); // ء
        self.add_key('c', "\u{0624}"); // ؤ
        self.add_key('v', "\u{0631}"); // ر
        self.add_key('b', "\u{0644}\u{0627}"); // لا (Lam-Alef ligature)
        self.add_key('n', "\u{0649}"); // ى
        self.add_key('m', "\u{0629}"); // ة
        self.add_key(',', "\u{0648}"); // و
        self.add_key('.', "\u{0632}"); // ز
        self.add_key('/', "\u{0638}"); // ظ

        // Row 4 - shifted
        self.add_shift_key('Z', "~");
        self.add_shift_key('X', "\u{0652}"); // Sukun ْ
        self.add_shift_key('C', "}");
        self.add_shift_key('V', "{");
        self.add_shift_key('B', "\u{0644}\u{0622}"); // لآ (Lam-Alef with Madda)
        self.add_shift_key('N', "\u{0622}"); // آ (Alef with Madda)
        self.add_shift_key('M', "\u{2019}"); // ' (right single quote)
        self.add_shift_key('<', ",");
        self.add_shift_key('>', ".");
        self.add_shift_key('?', "\u{061F}"); // ؟ (Arabic question mark)

        // Space stays space
        self.add_key(' ', " ");
    }

    /// Map key to Arabic string
    fn map_key(&self, ch: char, shift: bool) -> Option<&String> {
        if shift {
            self.shift_key_map.get(&ch)
        } else {
            self.key_map.get(&ch)
        }
    }

    /// Check if character is Arabic
    pub fn is_arabic(ch: char) -> bool {
        let code = ch as u32;
        // Arabic block: U+0600 to U+06FF
        // Arabic Supplement: U+0750 to U+077F
        // Arabic Extended-A: U+08A0 to U+08FF
        (code >= 0x0600 && code <= 0x06FF)
            || (code >= 0x0750 && code <= 0x077F)
            || (code >= 0x08A0 && code <= 0x08FF)
    }
}

impl Default for ArabicEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl InputMethodEngine for ArabicEngine {
    fn im_type(&self) -> InputMethodType {
        InputMethodType::Arabic
    }

    fn process_key(&mut self, event: InputEvent) -> InputResult {
        if !event.is_press {
            return InputResult::NotHandled;
        }

        // Pass through control/alt combinations
        if event.modifiers.ctrl || event.modifiers.alt {
            return InputResult::NotHandled;
        }

        let ch = match event.character {
            Some(c) => c,
            None => return InputResult::NotHandled,
        };

        // Handle special keys
        match ch {
            '\r' | '\n' | '\t' | '\x08' | '\x7f' | '\x1b' => {
                return InputResult::NotHandled;
            }
            _ => {}
        }

        // Try to map the key to Arabic
        let shift = event.modifiers.shift;

        // For uppercase letters or shift symbols
        let lookup_char = if ch.is_ascii_uppercase() || shift {
            ch
        } else {
            ch.to_ascii_lowercase()
        };

        let use_shift = ch.is_ascii_uppercase() || (shift && !ch.is_ascii_alphabetic());

        if let Some(arabic) = self.map_key(lookup_char, use_shift) {
            return InputResult::Commit(arabic.clone());
        }

        // If no mapping found, pass through
        InputResult::NotHandled
    }

    fn preedit(&self) -> &str {
        ""
    }

    fn candidates(&self) -> &[Candidate] {
        &[]
    }

    fn state(&self) -> InputMethodState {
        self.state
    }

    fn reset(&mut self) {
        self.state = InputMethodState::Idle;
    }

    fn selected_index(&self) -> usize {
        0
    }

    fn select_candidate(&mut self, _index: usize) -> Option<String> {
        None
    }

    fn move_up(&mut self) {}
    fn move_down(&mut self) {}
    fn page_up(&mut self) {}
    fn page_down(&mut self) {}

    fn commit(&mut self) -> Option<String> {
        None
    }

    fn cancel(&mut self) {
        self.reset();
    }
}

/// RTL text direction marker
pub const RTL_MARKER: char = '\u{200F}';
/// LTR text direction marker
pub const LTR_MARKER: char = '\u{200E}';

/// Check if text should be rendered RTL
pub fn should_render_rtl(text: &str) -> bool {
    text.chars().any(ArabicEngine::is_arabic)
}

/// Wrap text with RTL markers if needed
pub fn wrap_rtl(text: &str) -> String {
    if should_render_rtl(text) {
        alloc::format!("{}{}{}", RTL_MARKER, text, LTR_MARKER)
    } else {
        text.to_string()
    }
}
