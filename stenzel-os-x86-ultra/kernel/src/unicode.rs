//! Unicode support module
//!
//! Provides Unicode character classification, UTF-8 encoding/decoding,
//! and basic text utilities for proper internationalization support.

use alloc::string::String;
use alloc::vec::Vec;

// ============================================================================
// UTF-8 Encoding/Decoding
// ============================================================================

/// Decode a UTF-8 byte sequence into a Unicode codepoint
/// Returns (codepoint, bytes_consumed), or (REPLACEMENT_CHAR, 1) on error
pub fn decode_utf8(bytes: &[u8]) -> (char, usize) {
    if bytes.is_empty() {
        return ('\u{FFFD}', 0);
    }

    let b0 = bytes[0];

    // ASCII (single byte)
    if b0 < 0x80 {
        return (b0 as char, 1);
    }

    // Two-byte sequence (110xxxxx 10xxxxxx)
    if (b0 & 0xE0) == 0xC0 {
        if bytes.len() < 2 || (bytes[1] & 0xC0) != 0x80 {
            return ('\u{FFFD}', 1);
        }
        let cp = ((b0 as u32 & 0x1F) << 6) | (bytes[1] as u32 & 0x3F);
        // Check for overlong encoding
        if cp < 0x80 {
            return ('\u{FFFD}', 2);
        }
        return (char::from_u32(cp).unwrap_or('\u{FFFD}'), 2);
    }

    // Three-byte sequence (1110xxxx 10xxxxxx 10xxxxxx)
    if (b0 & 0xF0) == 0xE0 {
        if bytes.len() < 3 || (bytes[1] & 0xC0) != 0x80 || (bytes[2] & 0xC0) != 0x80 {
            return ('\u{FFFD}', 1);
        }
        let cp = ((b0 as u32 & 0x0F) << 12)
            | ((bytes[1] as u32 & 0x3F) << 6)
            | (bytes[2] as u32 & 0x3F);
        // Check for overlong encoding and surrogates
        if cp < 0x800 || (0xD800..=0xDFFF).contains(&cp) {
            return ('\u{FFFD}', 3);
        }
        return (char::from_u32(cp).unwrap_or('\u{FFFD}'), 3);
    }

    // Four-byte sequence (11110xxx 10xxxxxx 10xxxxxx 10xxxxxx)
    if (b0 & 0xF8) == 0xF0 {
        if bytes.len() < 4
            || (bytes[1] & 0xC0) != 0x80
            || (bytes[2] & 0xC0) != 0x80
            || (bytes[3] & 0xC0) != 0x80
        {
            return ('\u{FFFD}', 1);
        }
        let cp = ((b0 as u32 & 0x07) << 18)
            | ((bytes[1] as u32 & 0x3F) << 12)
            | ((bytes[2] as u32 & 0x3F) << 6)
            | (bytes[3] as u32 & 0x3F);
        // Check for overlong encoding and out-of-range
        if cp < 0x10000 || cp > 0x10FFFF {
            return ('\u{FFFD}', 4);
        }
        return (char::from_u32(cp).unwrap_or('\u{FFFD}'), 4);
    }

    // Invalid start byte
    ('\u{FFFD}', 1)
}

/// Encode a Unicode codepoint as UTF-8
/// Returns the number of bytes written to the buffer
pub fn encode_utf8(c: char, buf: &mut [u8]) -> usize {
    let cp = c as u32;

    if cp < 0x80 {
        if buf.is_empty() {
            return 0;
        }
        buf[0] = cp as u8;
        return 1;
    }

    if cp < 0x800 {
        if buf.len() < 2 {
            return 0;
        }
        buf[0] = 0xC0 | ((cp >> 6) as u8);
        buf[1] = 0x80 | ((cp & 0x3F) as u8);
        return 2;
    }

    if cp < 0x10000 {
        if buf.len() < 3 {
            return 0;
        }
        buf[0] = 0xE0 | ((cp >> 12) as u8);
        buf[1] = 0x80 | (((cp >> 6) & 0x3F) as u8);
        buf[2] = 0x80 | ((cp & 0x3F) as u8);
        return 3;
    }

    if buf.len() < 4 {
        return 0;
    }
    buf[0] = 0xF0 | ((cp >> 18) as u8);
    buf[1] = 0x80 | (((cp >> 12) & 0x3F) as u8);
    buf[2] = 0x80 | (((cp >> 6) & 0x3F) as u8);
    buf[3] = 0x80 | ((cp & 0x3F) as u8);
    4
}

/// Get the length in bytes of a UTF-8 character from its first byte
pub fn utf8_char_len(first_byte: u8) -> usize {
    if first_byte < 0x80 {
        1
    } else if (first_byte & 0xE0) == 0xC0 {
        2
    } else if (first_byte & 0xF0) == 0xE0 {
        3
    } else if (first_byte & 0xF8) == 0xF0 {
        4
    } else {
        1 // Invalid, treat as 1
    }
}

/// Count the number of Unicode characters in a UTF-8 string
pub fn char_count(s: &str) -> usize {
    s.chars().count()
}

/// Get the byte length needed to encode a character
pub fn char_utf8_len(c: char) -> usize {
    c.len_utf8()
}

// ============================================================================
// Character Classification (Unicode General Categories)
// ============================================================================

/// Unicode General Category
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    // Letters
    Lu, // Uppercase Letter
    Ll, // Lowercase Letter
    Lt, // Titlecase Letter
    Lm, // Modifier Letter
    Lo, // Other Letter

    // Marks
    Mn, // Nonspacing Mark
    Mc, // Spacing Combining Mark
    Me, // Enclosing Mark

    // Numbers
    Nd, // Decimal Number
    Nl, // Letter Number
    No, // Other Number

    // Punctuation
    Pc, // Connector Punctuation
    Pd, // Dash Punctuation
    Ps, // Open Punctuation
    Pe, // Close Punctuation
    Pi, // Initial Punctuation
    Pf, // Final Punctuation
    Po, // Other Punctuation

    // Symbols
    Sm, // Math Symbol
    Sc, // Currency Symbol
    Sk, // Modifier Symbol
    So, // Other Symbol

    // Separators
    Zs, // Space Separator
    Zl, // Line Separator
    Zp, // Paragraph Separator

    // Other
    Cc, // Control
    Cf, // Format
    Cs, // Surrogate
    Co, // Private Use
    Cn, // Not Assigned
}

/// Get the general category of a character
pub fn category(c: char) -> Category {
    let cp = c as u32;

    // ASCII fast path
    if cp < 0x80 {
        return ascii_category(cp as u8);
    }

    // Latin-1 Supplement (0x80-0xFF)
    if cp < 0x100 {
        return latin1_category(cp);
    }

    // Basic classification for common ranges
    match cp {
        // Latin Extended-A, Extended-B (mostly letters)
        0x0100..=0x024F => Category::Ll,
        // Greek and Coptic
        0x0370..=0x03FF => {
            if c.is_uppercase() {
                Category::Lu
            } else {
                Category::Ll
            }
        }
        // Cyrillic
        0x0400..=0x04FF => {
            if c.is_uppercase() {
                Category::Lu
            } else {
                Category::Ll
            }
        }
        // Arabic
        0x0600..=0x06FF => Category::Lo,
        // CJK Unified Ideographs
        0x4E00..=0x9FFF => Category::Lo,
        // Hiragana
        0x3040..=0x309F => Category::Lo,
        // Katakana
        0x30A0..=0x30FF => Category::Lo,
        // Hangul Syllables
        0xAC00..=0xD7AF => Category::Lo,
        // Private Use Area
        0xE000..=0xF8FF => Category::Co,
        // Specials
        0xFFF0..=0xFFFF => Category::Cn,
        // Surrogates (should not appear in valid UTF-8)
        0xD800..=0xDFFF => Category::Cs,
        // Emoji (common range)
        0x1F300..=0x1F9FF => Category::So,
        // Default to "Other Letter" for unclassified
        _ => {
            if c.is_alphabetic() {
                Category::Lo
            } else if c.is_numeric() {
                Category::No
            } else if c.is_whitespace() {
                Category::Zs
            } else if c.is_control() {
                Category::Cc
            } else {
                Category::Cn
            }
        }
    }
}

/// Classify ASCII characters
fn ascii_category(c: u8) -> Category {
    match c {
        0x00..=0x1F | 0x7F => Category::Cc, // Control characters
        b' ' => Category::Zs,                // Space
        b'!' | b'"' | b'#' | b'%' | b'&' | b'\'' | b'*' | b',' | b'.' | b'/' | b':' | b';'
        | b'?' | b'@' | b'\\' => Category::Po,
        b'$' => Category::Sc,                // Currency
        b'(' | b'[' | b'{' => Category::Ps, // Open punctuation
        b')' | b']' | b'}' => Category::Pe, // Close punctuation
        b'+' | b'<' | b'=' | b'>' | b'|' | b'~' => Category::Sm, // Math symbols
        b'-' => Category::Pd,                // Dash
        b'0'..=b'9' => Category::Nd,        // Digits
        b'A'..=b'Z' => Category::Lu,        // Uppercase
        b'^' | b'`' => Category::Sk,        // Modifier symbol
        b'_' => Category::Pc,               // Connector punctuation
        b'a'..=b'z' => Category::Ll,        // Lowercase
        _ => Category::Cn,
    }
}

/// Classify Latin-1 Supplement characters
fn latin1_category(cp: u32) -> Category {
    match cp {
        0x80..=0x9F => Category::Cc, // C1 control characters
        0xA0 => Category::Zs,        // No-break space
        0xA1 | 0xA7 | 0xB6 | 0xB7 | 0xBF => Category::Po, // Punctuation
        0xA2..=0xA5 => Category::Sc, // Currency symbols
        0xA6 | 0xA8 | 0xAF | 0xB4 | 0xB8 => Category::Sk, // Modifier symbols
        0xA9 | 0xAE | 0xB0 => Category::So, // Other symbols
        0xAA | 0xBA => Category::Lo, // Ordinal indicators
        0xAB => Category::Pi,        // Left guillemet
        0xAC | 0xB1 | 0xD7 | 0xF7 => Category::Sm, // Math symbols
        0xAD => Category::Cf,        // Soft hyphen
        0xB2 | 0xB3 | 0xB9 => Category::No, // Superscript numbers
        0xBB => Category::Pf,        // Right guillemet
        0xBC..=0xBE => Category::No, // Vulgar fractions
        0xC0..=0xD6 | 0xD8..=0xDE => Category::Lu, // Uppercase letters
        0xDF..=0xF6 | 0xF8..=0xFF => Category::Ll, // Lowercase letters
        _ => Category::Cn,
    }
}

// ============================================================================
// Character Properties
// ============================================================================

/// Check if character is a letter (L category)
pub fn is_letter(c: char) -> bool {
    matches!(
        category(c),
        Category::Lu | Category::Ll | Category::Lt | Category::Lm | Category::Lo
    )
}

/// Check if character is uppercase
pub fn is_uppercase(c: char) -> bool {
    c.is_uppercase() || category(c) == Category::Lu
}

/// Check if character is lowercase
pub fn is_lowercase(c: char) -> bool {
    c.is_lowercase() || category(c) == Category::Ll
}

/// Check if character is a digit (Nd category)
pub fn is_digit(c: char) -> bool {
    category(c) == Category::Nd
}

/// Check if character is a number (N category)
pub fn is_number(c: char) -> bool {
    matches!(category(c), Category::Nd | Category::Nl | Category::No)
}

/// Check if character is punctuation (P category)
pub fn is_punctuation(c: char) -> bool {
    matches!(
        category(c),
        Category::Pc
            | Category::Pd
            | Category::Ps
            | Category::Pe
            | Category::Pi
            | Category::Pf
            | Category::Po
    )
}

/// Check if character is a symbol (S category)
pub fn is_symbol(c: char) -> bool {
    matches!(
        category(c),
        Category::Sm | Category::Sc | Category::Sk | Category::So
    )
}

/// Check if character is whitespace (Z category + control whitespace)
pub fn is_whitespace(c: char) -> bool {
    c.is_whitespace()
        || matches!(category(c), Category::Zs | Category::Zl | Category::Zp)
}

/// Check if character is a control character
pub fn is_control(c: char) -> bool {
    category(c) == Category::Cc
}

/// Check if character is alphanumeric (letter or digit)
pub fn is_alphanumeric(c: char) -> bool {
    is_letter(c) || is_digit(c)
}

/// Check if character is printable (not control, not unassigned)
pub fn is_printable(c: char) -> bool {
    !matches!(category(c), Category::Cc | Category::Cf | Category::Cs | Category::Cn)
}

// ============================================================================
// Case Conversion
// ============================================================================

/// Convert character to uppercase
pub fn to_uppercase(c: char) -> char {
    // Use Rust's built-in for ASCII and common cases
    let mut iter = c.to_uppercase();
    iter.next().unwrap_or(c)
}

/// Convert character to lowercase
pub fn to_lowercase(c: char) -> char {
    let mut iter = c.to_lowercase();
    iter.next().unwrap_or(c)
}

/// Convert string to uppercase
pub fn string_to_uppercase(s: &str) -> String {
    s.to_uppercase()
}

/// Convert string to lowercase
pub fn string_to_lowercase(s: &str) -> String {
    s.to_lowercase()
}

// ============================================================================
// String Utilities
// ============================================================================

/// Reverse a string (by grapheme clusters would be ideal, but we do by chars)
pub fn reverse_string(s: &str) -> String {
    s.chars().rev().collect()
}

/// Trim whitespace from both ends
pub fn trim(s: &str) -> &str {
    s.trim()
}

/// Trim whitespace from start
pub fn trim_start(s: &str) -> &str {
    s.trim_start()
}

/// Trim whitespace from end
pub fn trim_end(s: &str) -> &str {
    s.trim_end()
}

/// Check if string contains only ASCII
pub fn is_ascii(s: &str) -> bool {
    s.is_ascii()
}

/// Check if string is valid UTF-8 (always true for &str, but useful for bytes)
pub fn is_valid_utf8(bytes: &[u8]) -> bool {
    core::str::from_utf8(bytes).is_ok()
}

/// Convert bytes to string, replacing invalid UTF-8 with replacement character
pub fn from_utf8_lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

// ============================================================================
// Unicode Ranges (for font fallback, etc.)
// ============================================================================

/// Unicode block/script identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnicodeBlock {
    BasicLatin,
    Latin1Supplement,
    LatinExtendedA,
    LatinExtendedB,
    GreekAndCoptic,
    Cyrillic,
    Arabic,
    Hebrew,
    Thai,
    CjkUnifiedIdeographs,
    Hiragana,
    Katakana,
    HangulSyllables,
    Emoji,
    PrivateUse,
    Unknown,
}

/// Get the Unicode block for a character
pub fn unicode_block(c: char) -> UnicodeBlock {
    let cp = c as u32;
    match cp {
        0x0000..=0x007F => UnicodeBlock::BasicLatin,
        0x0080..=0x00FF => UnicodeBlock::Latin1Supplement,
        0x0100..=0x017F => UnicodeBlock::LatinExtendedA,
        0x0180..=0x024F => UnicodeBlock::LatinExtendedB,
        0x0370..=0x03FF => UnicodeBlock::GreekAndCoptic,
        0x0400..=0x04FF => UnicodeBlock::Cyrillic,
        0x0590..=0x05FF => UnicodeBlock::Hebrew,
        0x0600..=0x06FF => UnicodeBlock::Arabic,
        0x0E00..=0x0E7F => UnicodeBlock::Thai,
        0x3040..=0x309F => UnicodeBlock::Hiragana,
        0x30A0..=0x30FF => UnicodeBlock::Katakana,
        0x4E00..=0x9FFF => UnicodeBlock::CjkUnifiedIdeographs,
        0xAC00..=0xD7AF => UnicodeBlock::HangulSyllables,
        0xE000..=0xF8FF => UnicodeBlock::PrivateUse,
        0x1F300..=0x1F9FF => UnicodeBlock::Emoji,
        _ => UnicodeBlock::Unknown,
    }
}

/// Check if a character requires a CJK-capable font
pub fn needs_cjk_font(c: char) -> bool {
    matches!(
        unicode_block(c),
        UnicodeBlock::CjkUnifiedIdeographs
            | UnicodeBlock::Hiragana
            | UnicodeBlock::Katakana
            | UnicodeBlock::HangulSyllables
    )
}

/// Check if a character requires an RTL (right-to-left) layout
pub fn is_rtl(c: char) -> bool {
    matches!(unicode_block(c), UnicodeBlock::Arabic | UnicodeBlock::Hebrew)
}

// ============================================================================
// Display Width (for terminal/console)
// ============================================================================

/// Get the display width of a character (0, 1, or 2 cells)
/// This is important for terminal emulation with CJK characters
pub fn char_width(c: char) -> usize {
    let cp = c as u32;

    // Control characters and combining marks have zero width
    if c.is_control() || matches!(category(c), Category::Mn | Category::Me | Category::Cf) {
        return 0;
    }

    // Most CJK characters are double-width
    if needs_cjk_font(c) {
        return 2;
    }

    // Emoji are typically double-width
    if unicode_block(c) == UnicodeBlock::Emoji {
        return 2;
    }

    // Fullwidth forms
    if (0xFF00..=0xFF60).contains(&cp) || (0xFFE0..=0xFFE6).contains(&cp) {
        return 2;
    }

    // Default to single width
    1
}

/// Get the display width of a string
pub fn string_width(s: &str) -> usize {
    s.chars().map(char_width).sum()
}

/// Initialize the unicode module
pub fn init() {
    crate::kprintln!("unicode: initialized (UTF-8, character classification, display width)");
}
