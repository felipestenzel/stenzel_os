//! Keyboard Layouts
//!
//! Support for different keyboard layouts (US, ABNT2, etc.)
//! Converts PS/2 scancode Set 1 to characters based on active layout.

#![allow(dead_code)]

use alloc::string::String;
use spin::Mutex;

/// Available keyboard layouts
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// US English (QWERTY)
    US,
    /// Brazilian Portuguese (ABNT2)
    ABNT2,
}

impl Layout {
    /// Get layout name
    pub fn name(&self) -> &'static str {
        match self {
            Layout::US => "US",
            Layout::ABNT2 => "ABNT2",
        }
    }
}

/// Current active layout
static CURRENT_LAYOUT: Mutex<Layout> = Mutex::new(Layout::US);

/// Get current keyboard layout
pub fn current_layout() -> Layout {
    *CURRENT_LAYOUT.lock()
}

/// Set current keyboard layout
pub fn set_layout(layout: Layout) {
    *CURRENT_LAYOUT.lock() = layout;
    crate::kprintln!("keyboard: layout changed to {}", layout.name());
}

/// Dead key state for accent composition
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeadKey {
    None,
    Acute,      // ´ (produces á, é, í, ó, ú)
    Grave,      // ` (produces à, è, ì, ò, ù)
    Circumflex, // ^ (produces â, ê, î, ô, û)
    Tilde,      // ~ (produces ã, õ, ñ)
    Diaeresis,  // ¨ (produces ä, ë, ï, ö, ü)
}

static DEAD_KEY: Mutex<DeadKey> = Mutex::new(DeadKey::None);

/// Get current dead key state
pub fn dead_key() -> DeadKey {
    *DEAD_KEY.lock()
}

/// Set dead key state
pub fn set_dead_key(key: DeadKey) {
    *DEAD_KEY.lock() = key;
}

/// Clear dead key state
pub fn clear_dead_key() {
    *DEAD_KEY.lock() = DeadKey::None;
}

/// Convert scancode to character using current layout
pub fn scancode_to_char(scancode: u8, shift: bool, alt_gr: bool) -> KeyResult {
    let layout = current_layout();
    match layout {
        Layout::US => us_layout(scancode, shift, alt_gr),
        Layout::ABNT2 => abnt2_layout(scancode, shift, alt_gr),
    }
}

/// Result of scancode translation
#[derive(Debug, Clone)]
pub enum KeyResult {
    /// Regular character
    Char(u8),
    /// Extended character (UTF-8)
    ExtChar(String),
    /// Dead key (waiting for next keypress)
    Dead(DeadKey),
    /// No character (modifier key or unknown)
    None,
}

// ============================================================================
// US Layout
// ============================================================================

fn us_layout(scancode: u8, shift: bool, _alt_gr: bool) -> KeyResult {
    let ch = if shift {
        us_scancode_shift(scancode)
    } else {
        us_scancode_normal(scancode)
    };

    if ch != 0 {
        KeyResult::Char(ch)
    } else {
        KeyResult::None
    }
}

fn us_scancode_normal(sc: u8) -> u8 {
    match sc {
        0x01 => 27,   // ESC
        0x02 => b'1', 0x03 => b'2', 0x04 => b'3', 0x05 => b'4', 0x06 => b'5',
        0x07 => b'6', 0x08 => b'7', 0x09 => b'8', 0x0A => b'9', 0x0B => b'0',
        0x0C => b'-', 0x0D => b'=', 0x0E => 8,    // Backspace
        0x0F => b'\t',
        0x10 => b'q', 0x11 => b'w', 0x12 => b'e', 0x13 => b'r', 0x14 => b't',
        0x15 => b'y', 0x16 => b'u', 0x17 => b'i', 0x18 => b'o', 0x19 => b'p',
        0x1A => b'[', 0x1B => b']', 0x1C => b'\n', // Enter
        0x1E => b'a', 0x1F => b's', 0x20 => b'd', 0x21 => b'f', 0x22 => b'g',
        0x23 => b'h', 0x24 => b'j', 0x25 => b'k', 0x26 => b'l',
        0x27 => b';', 0x28 => b'\'', 0x29 => b'`',
        0x2B => b'\\',
        0x2C => b'z', 0x2D => b'x', 0x2E => b'c', 0x2F => b'v', 0x30 => b'b',
        0x31 => b'n', 0x32 => b'm',
        0x33 => b',', 0x34 => b'.', 0x35 => b'/',
        0x37 => b'*', // Keypad *
        0x39 => b' ', // Space
        // Keypad
        0x47 => b'7', 0x48 => b'8', 0x49 => b'9', 0x4A => b'-',
        0x4B => b'4', 0x4C => b'5', 0x4D => b'6', 0x4E => b'+',
        0x4F => b'1', 0x50 => b'2', 0x51 => b'3',
        0x52 => b'0', 0x53 => b'.',
        _ => 0,
    }
}

fn us_scancode_shift(sc: u8) -> u8 {
    match sc {
        0x01 => 27,   // ESC
        0x02 => b'!', 0x03 => b'@', 0x04 => b'#', 0x05 => b'$', 0x06 => b'%',
        0x07 => b'^', 0x08 => b'&', 0x09 => b'*', 0x0A => b'(', 0x0B => b')',
        0x0C => b'_', 0x0D => b'+', 0x0E => 8,    // Backspace
        0x0F => b'\t',
        0x10 => b'Q', 0x11 => b'W', 0x12 => b'E', 0x13 => b'R', 0x14 => b'T',
        0x15 => b'Y', 0x16 => b'U', 0x17 => b'I', 0x18 => b'O', 0x19 => b'P',
        0x1A => b'{', 0x1B => b'}', 0x1C => b'\n', // Enter
        0x1E => b'A', 0x1F => b'S', 0x20 => b'D', 0x21 => b'F', 0x22 => b'G',
        0x23 => b'H', 0x24 => b'J', 0x25 => b'K', 0x26 => b'L',
        0x27 => b':', 0x28 => b'"', 0x29 => b'~',
        0x2B => b'|',
        0x2C => b'Z', 0x2D => b'X', 0x2E => b'C', 0x2F => b'V', 0x30 => b'B',
        0x31 => b'N', 0x32 => b'M',
        0x33 => b'<', 0x34 => b'>', 0x35 => b'?',
        0x37 => b'*', // Keypad *
        0x39 => b' ', // Space
        // Keypad (same as without shift)
        0x47 => b'7', 0x48 => b'8', 0x49 => b'9', 0x4A => b'-',
        0x4B => b'4', 0x4C => b'5', 0x4D => b'6', 0x4E => b'+',
        0x4F => b'1', 0x50 => b'2', 0x51 => b'3',
        0x52 => b'0', 0x53 => b'.',
        _ => 0,
    }
}

// ============================================================================
// ABNT2 Layout (Brazilian Portuguese)
// ============================================================================

/// ABNT2 layout scancode mapping
/// Key differences from US:
/// - Scancode 0x28 is ' (apostrophe), not ` (accent is dead key)
/// - Scancode 0x29 is ' (acute accent dead key) / " (shift)
/// - Scancode 0x2B is ] / }
/// - Extra key 0x56 is \ / |
/// - Scancode 0x1A is ´ (dead key) / ` (dead key shift)
/// - Scancode 0x1B is [ / {
/// - Scancode 0x27 is ç (cedilla)
/// - Keypad . is , for ABNT2
fn abnt2_layout(scancode: u8, shift: bool, alt_gr: bool) -> KeyResult {
    // Handle dead keys from previous keystroke
    let dead = dead_key();
    if dead != DeadKey::None {
        let result = compose_dead_key(dead, scancode, shift);
        clear_dead_key();
        return result;
    }

    // Check for dead key scancodes
    match scancode {
        // Acute accent key (key between = and Backspace on ABNT2)
        // Actually, on ABNT2 the acute/grave dead key is usually scancode 0x28
        0x28 if !shift && !alt_gr => {
            set_dead_key(DeadKey::Acute);
            return KeyResult::Dead(DeadKey::Acute);
        }
        0x28 if shift && !alt_gr => {
            set_dead_key(DeadKey::Grave);
            return KeyResult::Dead(DeadKey::Grave);
        }
        // Tilde/circumflex key (scancode 0x29 on ABNT2)
        0x29 if !shift && !alt_gr => {
            set_dead_key(DeadKey::Tilde);
            return KeyResult::Dead(DeadKey::Tilde);
        }
        0x29 if shift && !alt_gr => {
            set_dead_key(DeadKey::Circumflex);
            return KeyResult::Dead(DeadKey::Circumflex);
        }
        _ => {}
    }

    // AltGr combinations for ABNT2
    if alt_gr {
        return abnt2_altgr(scancode);
    }

    let ch = if shift {
        abnt2_scancode_shift(scancode)
    } else {
        abnt2_scancode_normal(scancode)
    };

    // Check for special extended characters
    match (scancode, shift) {
        // ç key (scancode 0x27 on ABNT2)
        (0x27, false) => KeyResult::ExtChar("ç".into()),
        (0x27, true) => KeyResult::ExtChar("Ç".into()),
        _ if ch != 0 => KeyResult::Char(ch),
        _ => KeyResult::None,
    }
}

fn abnt2_scancode_normal(sc: u8) -> u8 {
    match sc {
        0x01 => 27,   // ESC
        0x02 => b'1', 0x03 => b'2', 0x04 => b'3', 0x05 => b'4', 0x06 => b'5',
        0x07 => b'6', 0x08 => b'7', 0x09 => b'8', 0x0A => b'9', 0x0B => b'0',
        0x0C => b'-', 0x0D => b'=', 0x0E => 8,    // Backspace
        0x0F => b'\t',
        0x10 => b'q', 0x11 => b'w', 0x12 => b'e', 0x13 => b'r', 0x14 => b't',
        0x15 => b'y', 0x16 => b'u', 0x17 => b'i', 0x18 => b'o', 0x19 => b'p',
        0x1A => b'[', // Acute dead key handled separately
        0x1B => b']',
        0x1C => b'\n', // Enter
        0x1E => b'a', 0x1F => b's', 0x20 => b'd', 0x21 => b'f', 0x22 => b'g',
        0x23 => b'h', 0x24 => b'j', 0x25 => b'k', 0x26 => b'l',
        // 0x27 is ç (handled separately)
        // 0x28 is ´ dead key (handled separately)
        // 0x29 is ~ dead key (handled separately)
        0x2B => b'\\',
        0x2C => b'z', 0x2D => b'x', 0x2E => b'c', 0x2F => b'v', 0x30 => b'b',
        0x31 => b'n', 0x32 => b'm',
        0x33 => b',', 0x34 => b'.', 0x35 => b';', // Note: ABNT2 has ; here, not /
        0x37 => b'*', // Keypad *
        0x39 => b' ', // Space
        0x56 => b'/', // Extra key on ABNT2 (between left shift and Z)
        // Keypad - ABNT2 uses comma instead of period
        0x47 => b'7', 0x48 => b'8', 0x49 => b'9', 0x4A => b'-',
        0x4B => b'4', 0x4C => b'5', 0x4D => b'6', 0x4E => b'+',
        0x4F => b'1', 0x50 => b'2', 0x51 => b'3',
        0x52 => b'0', 0x53 => b',', // Comma on ABNT2 keypad
        _ => 0,
    }
}

fn abnt2_scancode_shift(sc: u8) -> u8 {
    match sc {
        0x01 => 27,   // ESC
        0x02 => b'!', 0x03 => b'@', 0x04 => b'#', 0x05 => b'$', 0x06 => b'%',
        0x07 => 0,    // Dead key ¨ (diaeresis) - would need special handling
        0x08 => b'&', 0x09 => b'*', 0x0A => b'(', 0x0B => b')',
        0x0C => b'_', 0x0D => b'+', 0x0E => 8,    // Backspace
        0x0F => b'\t',
        0x10 => b'Q', 0x11 => b'W', 0x12 => b'E', 0x13 => b'R', 0x14 => b'T',
        0x15 => b'Y', 0x16 => b'U', 0x17 => b'I', 0x18 => b'O', 0x19 => b'P',
        0x1A => b'{',
        0x1B => b'}',
        0x1C => b'\n', // Enter
        0x1E => b'A', 0x1F => b'S', 0x20 => b'D', 0x21 => b'F', 0x22 => b'G',
        0x23 => b'H', 0x24 => b'J', 0x25 => b'K', 0x26 => b'L',
        // 0x27 is Ç (handled separately)
        // 0x28 is ` dead key (handled separately)
        // 0x29 is ^ dead key (handled separately)
        0x2B => b'|',
        0x2C => b'Z', 0x2D => b'X', 0x2E => b'C', 0x2F => b'V', 0x30 => b'B',
        0x31 => b'N', 0x32 => b'M',
        0x33 => b'<', 0x34 => b'>', 0x35 => b':', // ABNT2 has : here
        0x37 => b'*', // Keypad *
        0x39 => b' ', // Space
        0x56 => b'?', // Extra key on ABNT2
        // Keypad (same as without shift)
        0x47 => b'7', 0x48 => b'8', 0x49 => b'9', 0x4A => b'-',
        0x4B => b'4', 0x4C => b'5', 0x4D => b'6', 0x4E => b'+',
        0x4F => b'1', 0x50 => b'2', 0x51 => b'3',
        0x52 => b'0', 0x53 => b',',
        _ => 0,
    }
}

/// AltGr combinations for ABNT2
fn abnt2_altgr(sc: u8) -> KeyResult {
    match sc {
        0x10 => KeyResult::Char(b'/'),  // AltGr+Q = /
        0x11 => KeyResult::Char(b'?'),  // AltGr+W = ?
        0x12 => KeyResult::ExtChar("°".into()), // AltGr+E = degree symbol
        0x1A => KeyResult::ExtChar("ª".into()), // AltGr+[ = ª
        0x1B => KeyResult::ExtChar("º".into()), // AltGr+] = º
        _ => KeyResult::None,
    }
}

/// Compose dead key with following character
fn compose_dead_key(dead: DeadKey, scancode: u8, shift: bool) -> KeyResult {
    // Get the base character for composition
    let base = if shift {
        abnt2_scancode_shift(scancode)
    } else {
        abnt2_scancode_normal(scancode)
    };

    let composed = match (dead, base.to_ascii_lowercase()) {
        // Acute accent (´)
        (DeadKey::Acute, b'a') => if base == b'A' { "Á" } else { "á" },
        (DeadKey::Acute, b'e') => if base == b'E' { "É" } else { "é" },
        (DeadKey::Acute, b'i') => if base == b'I' { "Í" } else { "í" },
        (DeadKey::Acute, b'o') => if base == b'O' { "Ó" } else { "ó" },
        (DeadKey::Acute, b'u') => if base == b'U' { "Ú" } else { "ú" },
        (DeadKey::Acute, b' ') => "´",

        // Grave accent (`)
        (DeadKey::Grave, b'a') => if base == b'A' { "À" } else { "à" },
        (DeadKey::Grave, b'e') => if base == b'E' { "È" } else { "è" },
        (DeadKey::Grave, b'i') => if base == b'I' { "Ì" } else { "ì" },
        (DeadKey::Grave, b'o') => if base == b'O' { "Ò" } else { "ò" },
        (DeadKey::Grave, b'u') => if base == b'U' { "Ù" } else { "ù" },
        (DeadKey::Grave, b' ') => "`",

        // Circumflex (^)
        (DeadKey::Circumflex, b'a') => if base == b'A' { "Â" } else { "â" },
        (DeadKey::Circumflex, b'e') => if base == b'E' { "Ê" } else { "ê" },
        (DeadKey::Circumflex, b'i') => if base == b'I' { "Î" } else { "î" },
        (DeadKey::Circumflex, b'o') => if base == b'O' { "Ô" } else { "ô" },
        (DeadKey::Circumflex, b'u') => if base == b'U' { "Û" } else { "û" },
        (DeadKey::Circumflex, b' ') => "^",

        // Tilde (~)
        (DeadKey::Tilde, b'a') => if base == b'A' { "Ã" } else { "ã" },
        (DeadKey::Tilde, b'o') => if base == b'O' { "Õ" } else { "õ" },
        (DeadKey::Tilde, b'n') => if base == b'N' { "Ñ" } else { "ñ" },
        (DeadKey::Tilde, b' ') => "~",

        // Diaeresis (¨)
        (DeadKey::Diaeresis, b'a') => if base == b'A' { "Ä" } else { "ä" },
        (DeadKey::Diaeresis, b'e') => if base == b'E' { "Ë" } else { "ë" },
        (DeadKey::Diaeresis, b'i') => if base == b'I' { "Ï" } else { "ï" },
        (DeadKey::Diaeresis, b'o') => if base == b'O' { "Ö" } else { "ö" },
        (DeadKey::Diaeresis, b'u') => if base == b'U' { "Ü" } else { "ü" },
        (DeadKey::Diaeresis, b'y') => if base == b'Y' { "Ÿ" } else { "ÿ" },
        (DeadKey::Diaeresis, b' ') => "¨",

        // No composition - return both dead key symbol and character
        (DeadKey::None, _) => return KeyResult::None,
        _ => {
            // Return the dead key character followed by the base character
            let dk_char = match dead {
                DeadKey::Acute => "´",
                DeadKey::Grave => "`",
                DeadKey::Circumflex => "^",
                DeadKey::Tilde => "~",
                DeadKey::Diaeresis => "¨",
                DeadKey::None => "",
            };
            if base != 0 {
                let mut s = String::from(dk_char);
                s.push(base as char);
                return KeyResult::ExtChar(s);
            } else {
                return KeyResult::ExtChar(dk_char.into());
            }
        }
    };

    KeyResult::ExtChar(composed.into())
}

/// List all available layouts
pub fn available_layouts() -> &'static [Layout] {
    &[Layout::US, Layout::ABNT2]
}

/// Parse layout name string
pub fn parse_layout(name: &str) -> Option<Layout> {
    match name.to_uppercase().as_str() {
        "US" | "US-EN" | "EN-US" | "QWERTY" => Some(Layout::US),
        "ABNT2" | "BR" | "PT-BR" | "BRAZILIAN" => Some(Layout::ABNT2),
        _ => None,
    }
}
