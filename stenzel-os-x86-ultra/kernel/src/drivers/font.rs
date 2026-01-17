//! Font rendering module
//!
//! Provides bitmap fonts and text rendering capabilities.
//! Supports:
//! - Built-in 8x16 VGA-style bitmap font
//! - PSF1 fonts (Linux PC Screen Font version 1)
//! - PSF2 fonts (Linux PC Screen Font version 2)
//! - TrueType fonts (TTF/OTF)

use alloc::vec;
use alloc::vec::Vec;
use alloc::collections::BTreeMap;

// Helper functions for no_std float operations
#[inline]
fn floor_f32(x: f32) -> f32 {
    let xi = x as i32;
    if x < 0.0 && x != xi as f32 {
        (xi - 1) as f32
    } else {
        xi as f32
    }
}

#[inline]
fn ceil_f32(x: f32) -> f32 {
    let xi = x as i32;
    if x > 0.0 && x != xi as f32 {
        (xi + 1) as f32
    } else {
        xi as f32
    }
}

#[inline]
fn abs_f32(x: f32) -> f32 {
    if x < 0.0 { -x } else { x }
}

#[inline]
fn max_f32(a: f32, b: f32) -> f32 {
    if a > b { a } else { b }
}

/// A bitmap font
pub struct BitmapFont {
    /// Glyph width in pixels
    pub width: usize,
    /// Glyph height in pixels
    pub height: usize,
    /// First character code in the font
    pub first_char: u8,
    /// Number of characters in the font
    pub num_chars: usize,
    /// Raw glyph data (height bytes per glyph, MSB is leftmost pixel)
    pub data: &'static [u8],
}

impl BitmapFont {
    /// Get glyph data for a character
    pub fn get_glyph(&self, c: char) -> Option<&[u8]> {
        let code = c as u32;
        if code < self.first_char as u32 || code >= (self.first_char as u32 + self.num_chars as u32) {
            return None;
        }
        let index = (code - self.first_char as u32) as usize;
        let offset = index * self.height;
        let end = offset + self.height;
        if end <= self.data.len() {
            Some(&self.data[offset..end])
        } else {
            None
        }
    }

    /// Check if a pixel is set in a glyph
    pub fn get_pixel(&self, c: char, x: usize, y: usize) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        if let Some(glyph) = self.get_glyph(c) {
            // MSB is leftmost pixel
            (glyph[y] >> (self.width - 1 - x)) & 1 != 0
        } else {
            false
        }
    }

    /// Measure the width of a string in pixels
    pub fn measure_string(&self, s: &str) -> usize {
        s.chars().count() * self.width
    }

    /// Measure the height of text (just the font height for single line)
    pub fn line_height(&self) -> usize {
        self.height
    }
}

/// The default 8x16 VGA font
/// This is a subset covering ASCII printable characters (32-126)
pub static DEFAULT_FONT: BitmapFont = BitmapFont {
    width: 8,
    height: 16,
    first_char: 32,
    num_chars: 95,
    data: &FONT_8X16_DATA,
};

/// 8x16 bitmap font data (VGA-style)
/// 95 characters from space (32) to tilde (126)
/// Each character is 16 bytes (one byte per row, MSB = leftmost pixel)
static FONT_8X16_DATA: [u8; 95 * 16] = [
    // 32: Space
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // 33: !
    0x00, 0x00, 0x18, 0x3C, 0x3C, 0x3C, 0x18, 0x18,
    0x18, 0x00, 0x18, 0x18, 0x00, 0x00, 0x00, 0x00,
    // 34: "
    0x00, 0x66, 0x66, 0x66, 0x24, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // 35: #
    0x00, 0x00, 0x00, 0x6C, 0x6C, 0xFE, 0x6C, 0x6C,
    0x6C, 0xFE, 0x6C, 0x6C, 0x00, 0x00, 0x00, 0x00,
    // 36: $
    0x18, 0x18, 0x7C, 0xC6, 0xC2, 0xC0, 0x7C, 0x06,
    0x06, 0x86, 0xC6, 0x7C, 0x18, 0x18, 0x00, 0x00,
    // 37: %
    0x00, 0x00, 0x00, 0x00, 0xC2, 0xC6, 0x0C, 0x18,
    0x30, 0x60, 0xC6, 0x86, 0x00, 0x00, 0x00, 0x00,
    // 38: &
    0x00, 0x00, 0x38, 0x6C, 0x6C, 0x38, 0x76, 0xDC,
    0xCC, 0xCC, 0xCC, 0x76, 0x00, 0x00, 0x00, 0x00,
    // 39: '
    0x00, 0x30, 0x30, 0x30, 0x60, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // 40: (
    0x00, 0x00, 0x0C, 0x18, 0x30, 0x30, 0x30, 0x30,
    0x30, 0x30, 0x18, 0x0C, 0x00, 0x00, 0x00, 0x00,
    // 41: )
    0x00, 0x00, 0x30, 0x18, 0x0C, 0x0C, 0x0C, 0x0C,
    0x0C, 0x0C, 0x18, 0x30, 0x00, 0x00, 0x00, 0x00,
    // 42: *
    0x00, 0x00, 0x00, 0x00, 0x00, 0x66, 0x3C, 0xFF,
    0x3C, 0x66, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // 43: +
    0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x7E,
    0x18, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // 44: ,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x18, 0x18, 0x18, 0x30, 0x00, 0x00, 0x00,
    // 45: -
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFE,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // 46: .
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x18, 0x18, 0x00, 0x00, 0x00, 0x00,
    // 47: /
    0x00, 0x00, 0x00, 0x00, 0x02, 0x06, 0x0C, 0x18,
    0x30, 0x60, 0xC0, 0x80, 0x00, 0x00, 0x00, 0x00,
    // 48: 0
    0x00, 0x00, 0x3C, 0x66, 0xC3, 0xC3, 0xDB, 0xDB,
    0xC3, 0xC3, 0x66, 0x3C, 0x00, 0x00, 0x00, 0x00,
    // 49: 1
    0x00, 0x00, 0x18, 0x38, 0x78, 0x18, 0x18, 0x18,
    0x18, 0x18, 0x18, 0x7E, 0x00, 0x00, 0x00, 0x00,
    // 50: 2
    0x00, 0x00, 0x7C, 0xC6, 0x06, 0x0C, 0x18, 0x30,
    0x60, 0xC0, 0xC6, 0xFE, 0x00, 0x00, 0x00, 0x00,
    // 51: 3
    0x00, 0x00, 0x7C, 0xC6, 0x06, 0x06, 0x3C, 0x06,
    0x06, 0x06, 0xC6, 0x7C, 0x00, 0x00, 0x00, 0x00,
    // 52: 4
    0x00, 0x00, 0x0C, 0x1C, 0x3C, 0x6C, 0xCC, 0xFE,
    0x0C, 0x0C, 0x0C, 0x1E, 0x00, 0x00, 0x00, 0x00,
    // 53: 5
    0x00, 0x00, 0xFE, 0xC0, 0xC0, 0xC0, 0xFC, 0x06,
    0x06, 0x06, 0xC6, 0x7C, 0x00, 0x00, 0x00, 0x00,
    // 54: 6
    0x00, 0x00, 0x38, 0x60, 0xC0, 0xC0, 0xFC, 0xC6,
    0xC6, 0xC6, 0xC6, 0x7C, 0x00, 0x00, 0x00, 0x00,
    // 55: 7
    0x00, 0x00, 0xFE, 0xC6, 0x06, 0x06, 0x0C, 0x18,
    0x30, 0x30, 0x30, 0x30, 0x00, 0x00, 0x00, 0x00,
    // 56: 8
    0x00, 0x00, 0x7C, 0xC6, 0xC6, 0xC6, 0x7C, 0xC6,
    0xC6, 0xC6, 0xC6, 0x7C, 0x00, 0x00, 0x00, 0x00,
    // 57: 9
    0x00, 0x00, 0x7C, 0xC6, 0xC6, 0xC6, 0x7E, 0x06,
    0x06, 0x06, 0x0C, 0x78, 0x00, 0x00, 0x00, 0x00,
    // 58: :
    0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x00, 0x00,
    0x00, 0x18, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00,
    // 59: ;
    0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x00, 0x00,
    0x00, 0x18, 0x18, 0x30, 0x00, 0x00, 0x00, 0x00,
    // 60: <
    0x00, 0x00, 0x00, 0x06, 0x0C, 0x18, 0x30, 0x60,
    0x30, 0x18, 0x0C, 0x06, 0x00, 0x00, 0x00, 0x00,
    // 61: =
    0x00, 0x00, 0x00, 0x00, 0x00, 0x7E, 0x00, 0x00,
    0x7E, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // 62: >
    0x00, 0x00, 0x00, 0x60, 0x30, 0x18, 0x0C, 0x06,
    0x0C, 0x18, 0x30, 0x60, 0x00, 0x00, 0x00, 0x00,
    // 63: ?
    0x00, 0x00, 0x7C, 0xC6, 0xC6, 0x0C, 0x18, 0x18,
    0x18, 0x00, 0x18, 0x18, 0x00, 0x00, 0x00, 0x00,
    // 64: @
    0x00, 0x00, 0x00, 0x7C, 0xC6, 0xC6, 0xDE, 0xDE,
    0xDE, 0xDC, 0xC0, 0x7C, 0x00, 0x00, 0x00, 0x00,
    // 65: A
    0x00, 0x00, 0x10, 0x38, 0x6C, 0xC6, 0xC6, 0xFE,
    0xC6, 0xC6, 0xC6, 0xC6, 0x00, 0x00, 0x00, 0x00,
    // 66: B
    0x00, 0x00, 0xFC, 0x66, 0x66, 0x66, 0x7C, 0x66,
    0x66, 0x66, 0x66, 0xFC, 0x00, 0x00, 0x00, 0x00,
    // 67: C
    0x00, 0x00, 0x3C, 0x66, 0xC2, 0xC0, 0xC0, 0xC0,
    0xC0, 0xC2, 0x66, 0x3C, 0x00, 0x00, 0x00, 0x00,
    // 68: D
    0x00, 0x00, 0xF8, 0x6C, 0x66, 0x66, 0x66, 0x66,
    0x66, 0x66, 0x6C, 0xF8, 0x00, 0x00, 0x00, 0x00,
    // 69: E
    0x00, 0x00, 0xFE, 0x66, 0x62, 0x68, 0x78, 0x68,
    0x60, 0x62, 0x66, 0xFE, 0x00, 0x00, 0x00, 0x00,
    // 70: F
    0x00, 0x00, 0xFE, 0x66, 0x62, 0x68, 0x78, 0x68,
    0x60, 0x60, 0x60, 0xF0, 0x00, 0x00, 0x00, 0x00,
    // 71: G
    0x00, 0x00, 0x3C, 0x66, 0xC2, 0xC0, 0xC0, 0xDE,
    0xC6, 0xC6, 0x66, 0x3A, 0x00, 0x00, 0x00, 0x00,
    // 72: H
    0x00, 0x00, 0xC6, 0xC6, 0xC6, 0xC6, 0xFE, 0xC6,
    0xC6, 0xC6, 0xC6, 0xC6, 0x00, 0x00, 0x00, 0x00,
    // 73: I
    0x00, 0x00, 0x3C, 0x18, 0x18, 0x18, 0x18, 0x18,
    0x18, 0x18, 0x18, 0x3C, 0x00, 0x00, 0x00, 0x00,
    // 74: J
    0x00, 0x00, 0x1E, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C,
    0xCC, 0xCC, 0xCC, 0x78, 0x00, 0x00, 0x00, 0x00,
    // 75: K
    0x00, 0x00, 0xE6, 0x66, 0x66, 0x6C, 0x78, 0x78,
    0x6C, 0x66, 0x66, 0xE6, 0x00, 0x00, 0x00, 0x00,
    // 76: L
    0x00, 0x00, 0xF0, 0x60, 0x60, 0x60, 0x60, 0x60,
    0x60, 0x62, 0x66, 0xFE, 0x00, 0x00, 0x00, 0x00,
    // 77: M
    0x00, 0x00, 0xC3, 0xE7, 0xFF, 0xFF, 0xDB, 0xC3,
    0xC3, 0xC3, 0xC3, 0xC3, 0x00, 0x00, 0x00, 0x00,
    // 78: N
    0x00, 0x00, 0xC6, 0xE6, 0xF6, 0xFE, 0xDE, 0xCE,
    0xC6, 0xC6, 0xC6, 0xC6, 0x00, 0x00, 0x00, 0x00,
    // 79: O
    0x00, 0x00, 0x7C, 0xC6, 0xC6, 0xC6, 0xC6, 0xC6,
    0xC6, 0xC6, 0xC6, 0x7C, 0x00, 0x00, 0x00, 0x00,
    // 80: P
    0x00, 0x00, 0xFC, 0x66, 0x66, 0x66, 0x7C, 0x60,
    0x60, 0x60, 0x60, 0xF0, 0x00, 0x00, 0x00, 0x00,
    // 81: Q
    0x00, 0x00, 0x7C, 0xC6, 0xC6, 0xC6, 0xC6, 0xC6,
    0xC6, 0xD6, 0xDE, 0x7C, 0x0C, 0x0E, 0x00, 0x00,
    // 82: R
    0x00, 0x00, 0xFC, 0x66, 0x66, 0x66, 0x7C, 0x6C,
    0x66, 0x66, 0x66, 0xE6, 0x00, 0x00, 0x00, 0x00,
    // 83: S
    0x00, 0x00, 0x7C, 0xC6, 0xC6, 0x60, 0x38, 0x0C,
    0x06, 0xC6, 0xC6, 0x7C, 0x00, 0x00, 0x00, 0x00,
    // 84: T
    0x00, 0x00, 0xFF, 0xDB, 0x99, 0x18, 0x18, 0x18,
    0x18, 0x18, 0x18, 0x3C, 0x00, 0x00, 0x00, 0x00,
    // 85: U
    0x00, 0x00, 0xC6, 0xC6, 0xC6, 0xC6, 0xC6, 0xC6,
    0xC6, 0xC6, 0xC6, 0x7C, 0x00, 0x00, 0x00, 0x00,
    // 86: V
    0x00, 0x00, 0xC3, 0xC3, 0xC3, 0xC3, 0xC3, 0xC3,
    0xC3, 0x66, 0x3C, 0x18, 0x00, 0x00, 0x00, 0x00,
    // 87: W
    0x00, 0x00, 0xC3, 0xC3, 0xC3, 0xC3, 0xC3, 0xDB,
    0xDB, 0xFF, 0x66, 0x66, 0x00, 0x00, 0x00, 0x00,
    // 88: X
    0x00, 0x00, 0xC3, 0xC3, 0x66, 0x3C, 0x18, 0x18,
    0x3C, 0x66, 0xC3, 0xC3, 0x00, 0x00, 0x00, 0x00,
    // 89: Y
    0x00, 0x00, 0xC3, 0xC3, 0xC3, 0x66, 0x3C, 0x18,
    0x18, 0x18, 0x18, 0x3C, 0x00, 0x00, 0x00, 0x00,
    // 90: Z
    0x00, 0x00, 0xFE, 0xC6, 0x86, 0x0C, 0x18, 0x30,
    0x60, 0xC2, 0xC6, 0xFE, 0x00, 0x00, 0x00, 0x00,
    // 91: [
    0x00, 0x00, 0x3C, 0x30, 0x30, 0x30, 0x30, 0x30,
    0x30, 0x30, 0x30, 0x3C, 0x00, 0x00, 0x00, 0x00,
    // 92: backslash
    0x00, 0x00, 0x00, 0x80, 0xC0, 0xE0, 0x70, 0x38,
    0x1C, 0x0E, 0x06, 0x02, 0x00, 0x00, 0x00, 0x00,
    // 93: ]
    0x00, 0x00, 0x3C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C,
    0x0C, 0x0C, 0x0C, 0x3C, 0x00, 0x00, 0x00, 0x00,
    // 94: ^
    0x10, 0x38, 0x6C, 0xC6, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // 95: _
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0x00, 0x00,
    // 96: `
    0x30, 0x30, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // 97: a
    0x00, 0x00, 0x00, 0x00, 0x00, 0x78, 0x0C, 0x7C,
    0xCC, 0xCC, 0xCC, 0x76, 0x00, 0x00, 0x00, 0x00,
    // 98: b
    0x00, 0x00, 0xE0, 0x60, 0x60, 0x78, 0x6C, 0x66,
    0x66, 0x66, 0x66, 0x7C, 0x00, 0x00, 0x00, 0x00,
    // 99: c
    0x00, 0x00, 0x00, 0x00, 0x00, 0x7C, 0xC6, 0xC0,
    0xC0, 0xC0, 0xC6, 0x7C, 0x00, 0x00, 0x00, 0x00,
    // 100: d
    0x00, 0x00, 0x1C, 0x0C, 0x0C, 0x3C, 0x6C, 0xCC,
    0xCC, 0xCC, 0xCC, 0x76, 0x00, 0x00, 0x00, 0x00,
    // 101: e
    0x00, 0x00, 0x00, 0x00, 0x00, 0x7C, 0xC6, 0xFE,
    0xC0, 0xC0, 0xC6, 0x7C, 0x00, 0x00, 0x00, 0x00,
    // 102: f
    0x00, 0x00, 0x38, 0x6C, 0x64, 0x60, 0xF0, 0x60,
    0x60, 0x60, 0x60, 0xF0, 0x00, 0x00, 0x00, 0x00,
    // 103: g
    0x00, 0x00, 0x00, 0x00, 0x00, 0x76, 0xCC, 0xCC,
    0xCC, 0xCC, 0xCC, 0x7C, 0x0C, 0xCC, 0x78, 0x00,
    // 104: h
    0x00, 0x00, 0xE0, 0x60, 0x60, 0x6C, 0x76, 0x66,
    0x66, 0x66, 0x66, 0xE6, 0x00, 0x00, 0x00, 0x00,
    // 105: i
    0x00, 0x00, 0x18, 0x18, 0x00, 0x38, 0x18, 0x18,
    0x18, 0x18, 0x18, 0x3C, 0x00, 0x00, 0x00, 0x00,
    // 106: j
    0x00, 0x00, 0x06, 0x06, 0x00, 0x0E, 0x06, 0x06,
    0x06, 0x06, 0x06, 0x06, 0x66, 0x66, 0x3C, 0x00,
    // 107: k
    0x00, 0x00, 0xE0, 0x60, 0x60, 0x66, 0x6C, 0x78,
    0x78, 0x6C, 0x66, 0xE6, 0x00, 0x00, 0x00, 0x00,
    // 108: l
    0x00, 0x00, 0x38, 0x18, 0x18, 0x18, 0x18, 0x18,
    0x18, 0x18, 0x18, 0x3C, 0x00, 0x00, 0x00, 0x00,
    // 109: m
    0x00, 0x00, 0x00, 0x00, 0x00, 0xE6, 0xFF, 0xDB,
    0xDB, 0xDB, 0xDB, 0xDB, 0x00, 0x00, 0x00, 0x00,
    // 110: n
    0x00, 0x00, 0x00, 0x00, 0x00, 0xDC, 0x66, 0x66,
    0x66, 0x66, 0x66, 0x66, 0x00, 0x00, 0x00, 0x00,
    // 111: o
    0x00, 0x00, 0x00, 0x00, 0x00, 0x7C, 0xC6, 0xC6,
    0xC6, 0xC6, 0xC6, 0x7C, 0x00, 0x00, 0x00, 0x00,
    // 112: p
    0x00, 0x00, 0x00, 0x00, 0x00, 0xDC, 0x66, 0x66,
    0x66, 0x66, 0x66, 0x7C, 0x60, 0x60, 0xF0, 0x00,
    // 113: q
    0x00, 0x00, 0x00, 0x00, 0x00, 0x76, 0xCC, 0xCC,
    0xCC, 0xCC, 0xCC, 0x7C, 0x0C, 0x0C, 0x1E, 0x00,
    // 114: r
    0x00, 0x00, 0x00, 0x00, 0x00, 0xDC, 0x76, 0x66,
    0x60, 0x60, 0x60, 0xF0, 0x00, 0x00, 0x00, 0x00,
    // 115: s
    0x00, 0x00, 0x00, 0x00, 0x00, 0x7C, 0xC6, 0x60,
    0x38, 0x0C, 0xC6, 0x7C, 0x00, 0x00, 0x00, 0x00,
    // 116: t
    0x00, 0x00, 0x10, 0x30, 0x30, 0xFC, 0x30, 0x30,
    0x30, 0x30, 0x36, 0x1C, 0x00, 0x00, 0x00, 0x00,
    // 117: u
    0x00, 0x00, 0x00, 0x00, 0x00, 0xCC, 0xCC, 0xCC,
    0xCC, 0xCC, 0xCC, 0x76, 0x00, 0x00, 0x00, 0x00,
    // 118: v
    0x00, 0x00, 0x00, 0x00, 0x00, 0xC3, 0xC3, 0xC3,
    0xC3, 0x66, 0x3C, 0x18, 0x00, 0x00, 0x00, 0x00,
    // 119: w
    0x00, 0x00, 0x00, 0x00, 0x00, 0xC3, 0xC3, 0xC3,
    0xDB, 0xDB, 0xFF, 0x66, 0x00, 0x00, 0x00, 0x00,
    // 120: x
    0x00, 0x00, 0x00, 0x00, 0x00, 0xC3, 0x66, 0x3C,
    0x18, 0x3C, 0x66, 0xC3, 0x00, 0x00, 0x00, 0x00,
    // 121: y
    0x00, 0x00, 0x00, 0x00, 0x00, 0xC6, 0xC6, 0xC6,
    0xC6, 0xC6, 0xC6, 0x7E, 0x06, 0x0C, 0xF8, 0x00,
    // 122: z
    0x00, 0x00, 0x00, 0x00, 0x00, 0xFE, 0xCC, 0x18,
    0x30, 0x60, 0xC6, 0xFE, 0x00, 0x00, 0x00, 0x00,
    // 123: {
    0x00, 0x00, 0x0E, 0x18, 0x18, 0x18, 0x70, 0x18,
    0x18, 0x18, 0x18, 0x0E, 0x00, 0x00, 0x00, 0x00,
    // 124: |
    0x00, 0x00, 0x18, 0x18, 0x18, 0x18, 0x00, 0x18,
    0x18, 0x18, 0x18, 0x18, 0x00, 0x00, 0x00, 0x00,
    // 125: }
    0x00, 0x00, 0x70, 0x18, 0x18, 0x18, 0x0E, 0x18,
    0x18, 0x18, 0x18, 0x70, 0x00, 0x00, 0x00, 0x00,
    // 126: ~
    0x00, 0x00, 0x76, 0xDC, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

/// Text alignment options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

/// Text renderer that uses a bitmap font
pub struct TextRenderer<'a> {
    pub font: &'a BitmapFont,
}

impl<'a> TextRenderer<'a> {
    pub fn new(font: &'a BitmapFont) -> Self {
        Self { font }
    }

    /// Draw a character at the given position
    /// Returns the width of the drawn character
    pub fn draw_char<F>(&self, x: usize, y: usize, c: char, mut set_pixel: F) -> usize
    where
        F: FnMut(usize, usize),
    {
        if let Some(glyph) = self.font.get_glyph(c) {
            for row in 0..self.font.height {
                let byte = glyph[row];
                for col in 0..self.font.width {
                    if (byte >> (self.font.width - 1 - col)) & 1 != 0 {
                        set_pixel(x + col, y + row);
                    }
                }
            }
            self.font.width
        } else {
            // Draw a replacement character (filled box)
            for row in 0..self.font.height {
                for col in 0..self.font.width {
                    if row == 0 || row == self.font.height - 1 || col == 0 || col == self.font.width - 1 {
                        set_pixel(x + col, y + row);
                    }
                }
            }
            self.font.width
        }
    }

    /// Draw a string at the given position
    pub fn draw_string<F>(&self, x: usize, y: usize, s: &str, mut set_pixel: F)
    where
        F: FnMut(usize, usize),
    {
        let mut current_x = x;
        for c in s.chars() {
            if c == '\n' {
                // Handle newlines - would need to track y position
                continue;
            }
            current_x += self.draw_char(current_x, y, c, &mut set_pixel);
        }
    }
}

/// Initialize the font subsystem
pub fn init() {
    crate::kprintln!("font: initialized 8x16 bitmap font ({} characters)", DEFAULT_FONT.num_chars);
}

// ============================================================================
// PSF Font Support (PC Screen Font)
// ============================================================================

/// PSF1 magic bytes
const PSF1_MAGIC: [u8; 2] = [0x36, 0x04];

/// PSF2 magic bytes
const PSF2_MAGIC: [u8; 4] = [0x72, 0xb5, 0x4a, 0x86];

/// PSF1 mode flags
pub mod psf1_mode {
    /// Font has 512 glyphs instead of 256
    pub const MODE512: u8 = 0x01;
    /// Font has a Unicode table
    pub const MODEHASTAB: u8 = 0x02;
    /// Font has Unicode sequence table
    pub const MODEHASSEQ: u8 = 0x04;
}

/// PSF2 flags
pub mod psf2_flags {
    /// Font has a Unicode table
    pub const HAS_UNICODE_TABLE: u32 = 0x01;
}

/// PSF1 header (4 bytes)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Psf1Header {
    pub magic: [u8; 2],
    pub mode: u8,
    pub charsize: u8, // bytes per glyph (height, width is always 8)
}

/// PSF2 header (32 bytes)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Psf2Header {
    pub magic: [u8; 4],
    pub version: u32,
    pub headersize: u32,
    pub flags: u32,
    pub length: u32,      // number of glyphs
    pub charsize: u32,    // bytes per glyph
    pub height: u32,
    pub width: u32,
}

/// Error type for PSF parsing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PsfError {
    InvalidMagic,
    DataTooShort,
    InvalidHeader,
    UnsupportedVersion,
}

/// A parsed PSF font (either PSF1 or PSF2)
pub struct PsfFont {
    /// Glyph width in pixels
    pub width: usize,
    /// Glyph height in pixels
    pub height: usize,
    /// Number of glyphs in the font
    pub num_glyphs: usize,
    /// Bytes per glyph
    pub bytes_per_glyph: usize,
    /// Raw glyph data
    glyph_data: Vec<u8>,
    /// Unicode to glyph index mapping (if available)
    unicode_table: Option<BTreeMap<u32, usize>>,
}

impl PsfFont {
    /// Parse a PSF font from raw bytes
    pub fn parse(data: &[u8]) -> Result<Self, PsfError> {
        if data.len() < 4 {
            return Err(PsfError::DataTooShort);
        }

        // Check for PSF2 first (has 4-byte magic)
        if data[0..4] == PSF2_MAGIC {
            return Self::parse_psf2(data);
        }

        // Check for PSF1 (has 2-byte magic)
        if data[0..2] == PSF1_MAGIC {
            return Self::parse_psf1(data);
        }

        Err(PsfError::InvalidMagic)
    }

    /// Parse PSF1 format
    fn parse_psf1(data: &[u8]) -> Result<Self, PsfError> {
        if data.len() < core::mem::size_of::<Psf1Header>() {
            return Err(PsfError::DataTooShort);
        }

        let header = unsafe {
            &*(data.as_ptr() as *const Psf1Header)
        };

        let has_512 = (header.mode & psf1_mode::MODE512) != 0;
        let has_unicode = (header.mode & psf1_mode::MODEHASTAB) != 0;

        let num_glyphs = if has_512 { 512 } else { 256 };
        let height = header.charsize as usize;
        let width = 8; // PSF1 is always 8 pixels wide
        let bytes_per_glyph = height;

        let glyph_data_start = core::mem::size_of::<Psf1Header>();
        let glyph_data_end = glyph_data_start + num_glyphs * bytes_per_glyph;

        if data.len() < glyph_data_end {
            return Err(PsfError::DataTooShort);
        }

        let glyph_data = data[glyph_data_start..glyph_data_end].to_vec();

        // Parse Unicode table if present
        let unicode_table = if has_unicode && data.len() > glyph_data_end {
            Some(Self::parse_psf1_unicode(&data[glyph_data_end..], num_glyphs))
        } else {
            None
        };

        Ok(Self {
            width,
            height,
            num_glyphs,
            bytes_per_glyph,
            glyph_data,
            unicode_table,
        })
    }

    /// Parse PSF1 Unicode table
    fn parse_psf1_unicode(data: &[u8], num_glyphs: usize) -> BTreeMap<u32, usize> {
        let mut table = BTreeMap::new();
        let mut pos = 0;
        let mut glyph_index = 0;

        while pos < data.len() && glyph_index < num_glyphs {
            // Read Unicode values until we hit 0xFFFF (terminator)
            while pos + 1 < data.len() {
                let codepoint = u16::from_le_bytes([data[pos], data[pos + 1]]);
                pos += 2;

                if codepoint == 0xFFFF {
                    break;
                }

                // PSF1 only supports BMP (Basic Multilingual Plane)
                table.insert(codepoint as u32, glyph_index);
            }
            glyph_index += 1;
        }

        table
    }

    /// Parse PSF2 format
    fn parse_psf2(data: &[u8]) -> Result<Self, PsfError> {
        if data.len() < core::mem::size_of::<Psf2Header>() {
            return Err(PsfError::DataTooShort);
        }

        let header = unsafe {
            &*(data.as_ptr() as *const Psf2Header)
        };

        // Safety: read header fields
        let version = header.version;
        let headersize = header.headersize as usize;
        let flags = header.flags;
        let length = header.length as usize;
        let charsize = header.charsize as usize;
        let height = header.height as usize;
        let width = header.width as usize;

        if version != 0 {
            return Err(PsfError::UnsupportedVersion);
        }

        let glyph_data_start = headersize;
        let glyph_data_end = glyph_data_start + length * charsize;

        if data.len() < glyph_data_end {
            return Err(PsfError::DataTooShort);
        }

        let glyph_data = data[glyph_data_start..glyph_data_end].to_vec();

        // Parse Unicode table if present
        let has_unicode = (flags & psf2_flags::HAS_UNICODE_TABLE) != 0;
        let unicode_table = if has_unicode && data.len() > glyph_data_end {
            Some(Self::parse_psf2_unicode(&data[glyph_data_end..], length))
        } else {
            None
        };

        Ok(Self {
            width,
            height,
            num_glyphs: length,
            bytes_per_glyph: charsize,
            glyph_data,
            unicode_table,
        })
    }

    /// Parse PSF2 Unicode table (UTF-8 encoded)
    fn parse_psf2_unicode(data: &[u8], num_glyphs: usize) -> BTreeMap<u32, usize> {
        let mut table = BTreeMap::new();
        let mut pos = 0;
        let mut glyph_index = 0;

        while pos < data.len() && glyph_index < num_glyphs {
            while pos < data.len() {
                let byte = data[pos];

                // 0xFF marks end of entry for this glyph
                if byte == 0xFF {
                    pos += 1;
                    break;
                }

                // 0xFE marks start of a sequence (combining characters)
                // We skip sequences for now
                if byte == 0xFE {
                    pos += 1;
                    // Skip until 0xFF
                    while pos < data.len() && data[pos] != 0xFF {
                        pos += 1;
                    }
                    continue;
                }

                // Decode UTF-8
                let (codepoint, len) = Self::decode_utf8(&data[pos..]);
                if len > 0 {
                    table.insert(codepoint, glyph_index);
                    pos += len;
                } else {
                    pos += 1; // Skip invalid byte
                }
            }
            glyph_index += 1;
        }

        table
    }

    /// Decode a single UTF-8 character, returns (codepoint, bytes_consumed)
    fn decode_utf8(data: &[u8]) -> (u32, usize) {
        if data.is_empty() {
            return (0, 0);
        }

        let b0 = data[0];

        // Single byte (ASCII)
        if b0 < 0x80 {
            return (b0 as u32, 1);
        }

        // Two bytes
        if (b0 & 0xE0) == 0xC0 && data.len() >= 2 {
            let b1 = data[1];
            if (b1 & 0xC0) == 0x80 {
                let cp = ((b0 as u32 & 0x1F) << 6) | (b1 as u32 & 0x3F);
                return (cp, 2);
            }
        }

        // Three bytes
        if (b0 & 0xF0) == 0xE0 && data.len() >= 3 {
            let b1 = data[1];
            let b2 = data[2];
            if (b1 & 0xC0) == 0x80 && (b2 & 0xC0) == 0x80 {
                let cp = ((b0 as u32 & 0x0F) << 12)
                    | ((b1 as u32 & 0x3F) << 6)
                    | (b2 as u32 & 0x3F);
                return (cp, 3);
            }
        }

        // Four bytes
        if (b0 & 0xF8) == 0xF0 && data.len() >= 4 {
            let b1 = data[1];
            let b2 = data[2];
            let b3 = data[3];
            if (b1 & 0xC0) == 0x80 && (b2 & 0xC0) == 0x80 && (b3 & 0xC0) == 0x80 {
                let cp = ((b0 as u32 & 0x07) << 18)
                    | ((b1 as u32 & 0x3F) << 12)
                    | ((b2 as u32 & 0x3F) << 6)
                    | (b3 as u32 & 0x3F);
                return (cp, 4);
            }
        }

        // Invalid sequence
        (0xFFFD, 1) // Replacement character
    }

    /// Get glyph index for a character
    pub fn get_glyph_index(&self, c: char) -> Option<usize> {
        let codepoint = c as u32;

        // Try Unicode table first
        if let Some(ref table) = self.unicode_table {
            return table.get(&codepoint).copied();
        }

        // Fall back to direct mapping for ASCII range
        if codepoint < self.num_glyphs as u32 {
            Some(codepoint as usize)
        } else {
            None
        }
    }

    /// Get raw glyph data for a character
    pub fn get_glyph(&self, c: char) -> Option<&[u8]> {
        let index = self.get_glyph_index(c)?;
        let offset = index * self.bytes_per_glyph;
        let end = offset + self.bytes_per_glyph;
        if end <= self.glyph_data.len() {
            Some(&self.glyph_data[offset..end])
        } else {
            None
        }
    }

    /// Check if a pixel is set in a glyph
    pub fn get_pixel(&self, c: char, x: usize, y: usize) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }

        if let Some(glyph) = self.get_glyph(c) {
            // Calculate byte and bit position
            let bytes_per_row = (self.width + 7) / 8;
            let byte_idx = y * bytes_per_row + x / 8;
            let bit_idx = 7 - (x % 8); // MSB is leftmost

            if byte_idx < glyph.len() {
                (glyph[byte_idx] >> bit_idx) & 1 != 0
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Measure string width
    pub fn measure_string(&self, s: &str) -> usize {
        s.chars().count() * self.width
    }

    /// Get line height
    pub fn line_height(&self) -> usize {
        self.height
    }

    /// Check if font has Unicode support
    pub fn has_unicode(&self) -> bool {
        self.unicode_table.is_some()
    }
}

/// PSF font renderer
pub struct PsfTextRenderer<'a> {
    pub font: &'a PsfFont,
}

impl<'a> PsfTextRenderer<'a> {
    pub fn new(font: &'a PsfFont) -> Self {
        Self { font }
    }

    /// Draw a character at the given position
    pub fn draw_char<F>(&self, x: usize, y: usize, c: char, mut set_pixel: F) -> usize
    where
        F: FnMut(usize, usize),
    {
        if let Some(glyph) = self.font.get_glyph(c) {
            let bytes_per_row = (self.font.width + 7) / 8;

            for row in 0..self.font.height {
                for col in 0..self.font.width {
                    let byte_idx = row * bytes_per_row + col / 8;
                    let bit_idx = 7 - (col % 8);

                    if byte_idx < glyph.len() && (glyph[byte_idx] >> bit_idx) & 1 != 0 {
                        set_pixel(x + col, y + row);
                    }
                }
            }
            self.font.width
        } else {
            // Draw replacement box
            for row in 0..self.font.height {
                for col in 0..self.font.width {
                    if row == 0 || row == self.font.height - 1 || col == 0 || col == self.font.width - 1 {
                        set_pixel(x + col, y + row);
                    }
                }
            }
            self.font.width
        }
    }

    /// Draw a string at the given position
    pub fn draw_string<F>(&self, x: usize, y: usize, s: &str, mut set_pixel: F)
    where
        F: FnMut(usize, usize),
    {
        let mut current_x = x;
        for c in s.chars() {
            if c == '\n' {
                continue;
            }
            current_x += self.draw_char(current_x, y, c, &mut set_pixel);
        }
    }
}

// ============================================================================
// TrueType Font Support (TTF/OTF)
// ============================================================================

/// TrueType font errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TtfError {
    InvalidMagic,
    DataTooShort,
    TableNotFound,
    InvalidTable,
    InvalidGlyph,
    UnsupportedFormat,
}

/// TTF table tag (4 ASCII characters as u32)
fn table_tag(s: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*s)
}

/// Table directory entry
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct TableRecord {
    tag: u32,
    checksum: u32,
    offset: u32,
    length: u32,
}

/// head table structure
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct HeadTable {
    major_version: u16,
    minor_version: u16,
    font_revision: i32,
    checksum_adjustment: u32,
    magic_number: u32,
    flags: u16,
    units_per_em: u16,
    created: i64,
    modified: i64,
    x_min: i16,
    y_min: i16,
    x_max: i16,
    y_max: i16,
    mac_style: u16,
    lowest_rec_ppem: u16,
    font_direction_hint: i16,
    index_to_loc_format: i16,
    glyph_data_format: i16,
}

/// hhea table structure (horizontal header)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct HheaTable {
    major_version: u16,
    minor_version: u16,
    ascender: i16,
    descender: i16,
    line_gap: i16,
    advance_width_max: u16,
    min_left_side_bearing: i16,
    min_right_side_bearing: i16,
    x_max_extent: i16,
    caret_slope_rise: i16,
    caret_slope_run: i16,
    caret_offset: i16,
    _reserved: [i16; 4],
    metric_data_format: i16,
    number_of_h_metrics: u16,
}

/// maxp table structure
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct MaxpTable {
    version: i32,
    num_glyphs: u16,
}

/// Horizontal metrics entry
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct HmtxEntry {
    advance_width: u16,
    left_side_bearing: i16,
}

/// A point in a glyph outline
#[derive(Debug, Clone, Copy)]
pub struct GlyphPoint {
    pub x: i16,
    pub y: i16,
    pub on_curve: bool,
}

/// A contour (closed path) in a glyph
#[derive(Debug, Clone)]
pub struct GlyphContour {
    pub points: Vec<GlyphPoint>,
}

/// A parsed glyph
#[derive(Debug, Clone)]
pub struct TtfGlyph {
    /// Glyph bounding box
    pub x_min: i16,
    pub y_min: i16,
    pub x_max: i16,
    pub y_max: i16,
    /// Advance width (how much to advance X after drawing)
    pub advance_width: u16,
    /// Left side bearing
    pub lsb: i16,
    /// Contours (for simple glyphs)
    pub contours: Vec<GlyphContour>,
    /// Is this a compound glyph?
    pub is_compound: bool,
}

/// Parsed TrueType font
pub struct TtfFont {
    /// Raw font data
    data: Vec<u8>,
    /// Units per em (design units)
    pub units_per_em: u16,
    /// Ascender (above baseline)
    pub ascender: i16,
    /// Descender (below baseline, usually negative)
    pub descender: i16,
    /// Line gap
    pub line_gap: i16,
    /// Number of glyphs
    pub num_glyphs: u16,
    /// Number of horizontal metrics
    num_h_metrics: u16,
    /// Index to loc format (0 = short, 1 = long)
    index_to_loc_format: i16,
    /// Table offsets
    cmap_offset: usize,
    glyf_offset: usize,
    loca_offset: usize,
    hmtx_offset: usize,
    /// Character to glyph mapping (format 4 or 12)
    cmap_format: u16,
    cmap_data_offset: usize,
}

impl TtfFont {
    /// Parse a TrueType font from raw data
    pub fn parse(data: &[u8]) -> Result<Self, TtfError> {
        if data.len() < 12 {
            return Err(TtfError::DataTooShort);
        }

        // Check magic number (0x00010000 for TrueType, 'OTTO' for OpenType/CFF)
        let sfnt_version = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        if sfnt_version != 0x00010000 && sfnt_version != 0x4F54544F {
            return Err(TtfError::InvalidMagic);
        }

        let num_tables = u16::from_be_bytes([data[4], data[5]]) as usize;

        if data.len() < 12 + num_tables * 16 {
            return Err(TtfError::DataTooShort);
        }

        // Find required tables
        let mut head_offset = None;
        let mut hhea_offset = None;
        let mut maxp_offset = None;
        let mut cmap_offset = None;
        let mut glyf_offset = None;
        let mut loca_offset = None;
        let mut hmtx_offset = None;

        for i in 0..num_tables {
            let record_offset = 12 + i * 16;
            let tag = u32::from_be_bytes([
                data[record_offset],
                data[record_offset + 1],
                data[record_offset + 2],
                data[record_offset + 3],
            ]);
            let offset = u32::from_be_bytes([
                data[record_offset + 8],
                data[record_offset + 9],
                data[record_offset + 10],
                data[record_offset + 11],
            ]) as usize;

            match &tag.to_be_bytes() {
                b"head" => head_offset = Some(offset),
                b"hhea" => hhea_offset = Some(offset),
                b"maxp" => maxp_offset = Some(offset),
                b"cmap" => cmap_offset = Some(offset),
                b"glyf" => glyf_offset = Some(offset),
                b"loca" => loca_offset = Some(offset),
                b"hmtx" => hmtx_offset = Some(offset),
                _ => {}
            }
        }

        // All tables required for TrueType outlines
        let head_offset = head_offset.ok_or(TtfError::TableNotFound)?;
        let hhea_offset = hhea_offset.ok_or(TtfError::TableNotFound)?;
        let maxp_offset = maxp_offset.ok_or(TtfError::TableNotFound)?;
        let cmap_offset = cmap_offset.ok_or(TtfError::TableNotFound)?;
        let glyf_offset = glyf_offset.ok_or(TtfError::TableNotFound)?;
        let loca_offset = loca_offset.ok_or(TtfError::TableNotFound)?;
        let hmtx_offset = hmtx_offset.ok_or(TtfError::TableNotFound)?;

        // Parse head table
        if data.len() < head_offset + core::mem::size_of::<HeadTable>() {
            return Err(TtfError::DataTooShort);
        }
        let head = unsafe { &*(data.as_ptr().add(head_offset) as *const HeadTable) };
        let units_per_em = u16::from_be(head.units_per_em);
        let index_to_loc_format = i16::from_be(head.index_to_loc_format);

        // Parse hhea table
        if data.len() < hhea_offset + core::mem::size_of::<HheaTable>() {
            return Err(TtfError::DataTooShort);
        }
        let hhea = unsafe { &*(data.as_ptr().add(hhea_offset) as *const HheaTable) };
        let ascender = i16::from_be(hhea.ascender);
        let descender = i16::from_be(hhea.descender);
        let line_gap = i16::from_be(hhea.line_gap);
        let num_h_metrics = u16::from_be(hhea.number_of_h_metrics);

        // Parse maxp table
        if data.len() < maxp_offset + core::mem::size_of::<MaxpTable>() {
            return Err(TtfError::DataTooShort);
        }
        let maxp = unsafe { &*(data.as_ptr().add(maxp_offset) as *const MaxpTable) };
        let num_glyphs = u16::from_be(maxp.num_glyphs);

        // Parse cmap table to find a usable subtable
        let (cmap_format, cmap_data_offset) = Self::find_cmap_subtable(data, cmap_offset)?;

        Ok(Self {
            data: data.to_vec(),
            units_per_em,
            ascender,
            descender,
            line_gap,
            num_glyphs,
            num_h_metrics,
            index_to_loc_format,
            cmap_offset,
            glyf_offset,
            loca_offset,
            hmtx_offset,
            cmap_format,
            cmap_data_offset,
        })
    }

    /// Find a usable cmap subtable (prefer format 12, fall back to format 4)
    fn find_cmap_subtable(data: &[u8], cmap_offset: usize) -> Result<(u16, usize), TtfError> {
        if data.len() < cmap_offset + 4 {
            return Err(TtfError::DataTooShort);
        }

        let num_tables = u16::from_be_bytes([data[cmap_offset + 2], data[cmap_offset + 3]]) as usize;

        let mut format4_offset = None;
        let mut format12_offset = None;

        for i in 0..num_tables {
            let record_offset = cmap_offset + 4 + i * 8;
            if data.len() < record_offset + 8 {
                break;
            }

            let platform_id = u16::from_be_bytes([data[record_offset], data[record_offset + 1]]);
            let encoding_id = u16::from_be_bytes([data[record_offset + 2], data[record_offset + 3]]);
            let subtable_offset = u32::from_be_bytes([
                data[record_offset + 4],
                data[record_offset + 5],
                data[record_offset + 6],
                data[record_offset + 7],
            ]) as usize;

            let subtable_abs = cmap_offset + subtable_offset;
            if data.len() < subtable_abs + 2 {
                continue;
            }

            let format = u16::from_be_bytes([data[subtable_abs], data[subtable_abs + 1]]);

            // Prefer Unicode platform (0) or Windows Unicode (3, 1) or (3, 10)
            if platform_id == 0 || (platform_id == 3 && (encoding_id == 1 || encoding_id == 10)) {
                if format == 12 {
                    format12_offset = Some(subtable_abs);
                } else if format == 4 && format12_offset.is_none() {
                    format4_offset = Some(subtable_abs);
                }
            }
        }

        // Prefer format 12 (supports full Unicode)
        if let Some(offset) = format12_offset {
            return Ok((12, offset));
        }
        if let Some(offset) = format4_offset {
            return Ok((4, offset));
        }

        Err(TtfError::UnsupportedFormat)
    }

    /// Map a Unicode codepoint to a glyph index
    pub fn get_glyph_index(&self, codepoint: u32) -> Option<u16> {
        match self.cmap_format {
            4 => self.cmap_format4_lookup(codepoint),
            12 => self.cmap_format12_lookup(codepoint),
            _ => None,
        }
    }

    /// Format 4 cmap lookup (BMP only)
    fn cmap_format4_lookup(&self, codepoint: u32) -> Option<u16> {
        if codepoint > 0xFFFF {
            return None;
        }
        let codepoint = codepoint as u16;
        let data = &self.data;
        let offset = self.cmap_data_offset;

        if data.len() < offset + 14 {
            return None;
        }

        let seg_count = u16::from_be_bytes([data[offset + 6], data[offset + 7]]) / 2;
        let end_codes_offset = offset + 14;
        let start_codes_offset = end_codes_offset + (seg_count as usize) * 2 + 2;
        let id_deltas_offset = start_codes_offset + (seg_count as usize) * 2;
        let id_range_offsets_offset = id_deltas_offset + (seg_count as usize) * 2;

        for i in 0..(seg_count as usize) {
            let end_code = u16::from_be_bytes([
                data[end_codes_offset + i * 2],
                data[end_codes_offset + i * 2 + 1],
            ]);
            let start_code = u16::from_be_bytes([
                data[start_codes_offset + i * 2],
                data[start_codes_offset + i * 2 + 1],
            ]);

            if codepoint >= start_code && codepoint <= end_code {
                let id_delta = i16::from_be_bytes([
                    data[id_deltas_offset + i * 2],
                    data[id_deltas_offset + i * 2 + 1],
                ]);
                let id_range_offset = u16::from_be_bytes([
                    data[id_range_offsets_offset + i * 2],
                    data[id_range_offsets_offset + i * 2 + 1],
                ]);

                if id_range_offset == 0 {
                    return Some((codepoint as i16 + id_delta) as u16);
                } else {
                    let glyph_offset = id_range_offsets_offset
                        + i * 2
                        + (id_range_offset as usize)
                        + (codepoint - start_code) as usize * 2;
                    if data.len() < glyph_offset + 2 {
                        return None;
                    }
                    let glyph = u16::from_be_bytes([data[glyph_offset], data[glyph_offset + 1]]);
                    if glyph != 0 {
                        return Some((glyph as i16 + id_delta) as u16);
                    }
                }
            }
        }

        None
    }

    /// Format 12 cmap lookup (full Unicode)
    fn cmap_format12_lookup(&self, codepoint: u32) -> Option<u16> {
        let data = &self.data;
        let offset = self.cmap_data_offset;

        if data.len() < offset + 16 {
            return None;
        }

        let num_groups = u32::from_be_bytes([
            data[offset + 12],
            data[offset + 13],
            data[offset + 14],
            data[offset + 15],
        ]) as usize;

        for i in 0..num_groups {
            let group_offset = offset + 16 + i * 12;
            if data.len() < group_offset + 12 {
                break;
            }

            let start_char = u32::from_be_bytes([
                data[group_offset],
                data[group_offset + 1],
                data[group_offset + 2],
                data[group_offset + 3],
            ]);
            let end_char = u32::from_be_bytes([
                data[group_offset + 4],
                data[group_offset + 5],
                data[group_offset + 6],
                data[group_offset + 7],
            ]);
            let start_glyph = u32::from_be_bytes([
                data[group_offset + 8],
                data[group_offset + 9],
                data[group_offset + 10],
                data[group_offset + 11],
            ]);

            if codepoint >= start_char && codepoint <= end_char {
                return Some((start_glyph + codepoint - start_char) as u16);
            }
        }

        None
    }

    /// Get glyph offset in glyf table
    fn get_glyph_offset(&self, glyph_index: u16) -> Option<usize> {
        if glyph_index >= self.num_glyphs {
            return None;
        }

        let data = &self.data;
        let loca = self.loca_offset;

        let (offset, next_offset) = if self.index_to_loc_format == 0 {
            // Short format (2 bytes per entry, multiply by 2)
            let idx = (glyph_index as usize) * 2;
            if data.len() < loca + idx + 4 {
                return None;
            }
            let o = u16::from_be_bytes([data[loca + idx], data[loca + idx + 1]]) as usize * 2;
            let n = u16::from_be_bytes([data[loca + idx + 2], data[loca + idx + 3]]) as usize * 2;
            (o, n)
        } else {
            // Long format (4 bytes per entry)
            let idx = (glyph_index as usize) * 4;
            if data.len() < loca + idx + 8 {
                return None;
            }
            let o = u32::from_be_bytes([
                data[loca + idx],
                data[loca + idx + 1],
                data[loca + idx + 2],
                data[loca + idx + 3],
            ]) as usize;
            let n = u32::from_be_bytes([
                data[loca + idx + 4],
                data[loca + idx + 5],
                data[loca + idx + 6],
                data[loca + idx + 7],
            ]) as usize;
            (o, n)
        };

        // Empty glyph (like space)
        if offset == next_offset {
            return None;
        }

        Some(self.glyf_offset + offset)
    }

    /// Get horizontal metrics for a glyph
    fn get_h_metrics(&self, glyph_index: u16) -> (u16, i16) {
        let data = &self.data;
        let hmtx = self.hmtx_offset;

        if glyph_index < self.num_h_metrics {
            let idx = (glyph_index as usize) * 4;
            if data.len() >= hmtx + idx + 4 {
                let aw = u16::from_be_bytes([data[hmtx + idx], data[hmtx + idx + 1]]);
                let lsb = i16::from_be_bytes([data[hmtx + idx + 2], data[hmtx + idx + 3]]);
                return (aw, lsb);
            }
        } else {
            // Use last advance width for monospace-like glyphs
            let last_idx = (self.num_h_metrics as usize - 1) * 4;
            let aw = if data.len() >= hmtx + last_idx + 2 {
                u16::from_be_bytes([data[hmtx + last_idx], data[hmtx + last_idx + 1]])
            } else {
                0
            };
            // LSB is in separate array
            let lsb_idx = (self.num_h_metrics as usize) * 4
                + (glyph_index - self.num_h_metrics) as usize * 2;
            let lsb = if data.len() >= hmtx + lsb_idx + 2 {
                i16::from_be_bytes([data[hmtx + lsb_idx], data[hmtx + lsb_idx + 1]])
            } else {
                0
            };
            return (aw, lsb);
        }

        (0, 0)
    }

    /// Parse a simple glyph (not compound)
    fn parse_simple_glyph(&self, offset: usize, num_contours: i16) -> Option<Vec<GlyphContour>> {
        if num_contours <= 0 {
            return Some(Vec::new());
        }

        let data = &self.data;
        let num_contours = num_contours as usize;

        // Read end points of each contour
        let mut end_pts = Vec::with_capacity(num_contours);
        for i in 0..num_contours {
            if data.len() < offset + 10 + (i + 1) * 2 {
                return None;
            }
            let idx = offset + 10 + i * 2;
            let end_pt = u16::from_be_bytes([data[idx], data[idx + 1]]);
            end_pts.push(end_pt as usize);
        }

        let num_points = *end_pts.last()? + 1;

        // Skip instruction length and instructions
        let inst_len_offset = offset + 10 + num_contours * 2;
        if data.len() < inst_len_offset + 2 {
            return None;
        }
        let inst_len =
            u16::from_be_bytes([data[inst_len_offset], data[inst_len_offset + 1]]) as usize;

        let flags_offset = inst_len_offset + 2 + inst_len;

        // Read flags
        let mut flags = Vec::with_capacity(num_points);
        let mut pos = flags_offset;
        while flags.len() < num_points {
            if pos >= data.len() {
                return None;
            }
            let flag = data[pos];
            pos += 1;
            flags.push(flag);

            // Repeat flag
            if (flag & 0x08) != 0 {
                if pos >= data.len() {
                    return None;
                }
                let repeat = data[pos] as usize;
                pos += 1;
                for _ in 0..repeat {
                    flags.push(flag);
                }
            }
        }

        // Read X coordinates
        let mut x_coords = Vec::with_capacity(num_points);
        let mut x: i16 = 0;
        for &flag in &flags {
            let x_short = (flag & 0x02) != 0;
            let x_same_or_positive = (flag & 0x10) != 0;

            if x_short {
                if pos >= data.len() {
                    return None;
                }
                let dx = data[pos] as i16;
                pos += 1;
                if x_same_or_positive {
                    x += dx;
                } else {
                    x -= dx;
                }
            } else if !x_same_or_positive {
                if pos + 1 >= data.len() {
                    return None;
                }
                let dx = i16::from_be_bytes([data[pos], data[pos + 1]]);
                pos += 2;
                x += dx;
            }
            // else: x_same_or_positive && !x_short means x is same as previous
            x_coords.push(x);
        }

        // Read Y coordinates
        let mut y_coords = Vec::with_capacity(num_points);
        let mut y: i16 = 0;
        for &flag in &flags {
            let y_short = (flag & 0x04) != 0;
            let y_same_or_positive = (flag & 0x20) != 0;

            if y_short {
                if pos >= data.len() {
                    return None;
                }
                let dy = data[pos] as i16;
                pos += 1;
                if y_same_or_positive {
                    y += dy;
                } else {
                    y -= dy;
                }
            } else if !y_same_or_positive {
                if pos + 1 >= data.len() {
                    return None;
                }
                let dy = i16::from_be_bytes([data[pos], data[pos + 1]]);
                pos += 2;
                y += dy;
            }
            y_coords.push(y);
        }

        // Build contours
        let mut contours = Vec::with_capacity(num_contours);
        let mut start = 0;
        for &end in &end_pts {
            let mut points = Vec::with_capacity(end - start + 1);
            for i in start..=end {
                points.push(GlyphPoint {
                    x: x_coords[i],
                    y: y_coords[i],
                    on_curve: (flags[i] & 0x01) != 0,
                });
            }
            contours.push(GlyphContour { points });
            start = end + 1;
        }

        Some(contours)
    }

    /// Get a parsed glyph
    pub fn get_glyph(&self, glyph_index: u16) -> Option<TtfGlyph> {
        let (advance_width, lsb) = self.get_h_metrics(glyph_index);

        let offset = match self.get_glyph_offset(glyph_index) {
            Some(o) => o,
            None => {
                // Empty glyph (like space)
                return Some(TtfGlyph {
                    x_min: 0,
                    y_min: 0,
                    x_max: 0,
                    y_max: 0,
                    advance_width,
                    lsb,
                    contours: Vec::new(),
                    is_compound: false,
                });
            }
        };

        let data = &self.data;
        if data.len() < offset + 10 {
            return None;
        }

        let num_contours = i16::from_be_bytes([data[offset], data[offset + 1]]);
        let x_min = i16::from_be_bytes([data[offset + 2], data[offset + 3]]);
        let y_min = i16::from_be_bytes([data[offset + 4], data[offset + 5]]);
        let x_max = i16::from_be_bytes([data[offset + 6], data[offset + 7]]);
        let y_max = i16::from_be_bytes([data[offset + 8], data[offset + 9]]);

        if num_contours >= 0 {
            // Simple glyph
            let contours = self.parse_simple_glyph(offset, num_contours)?;
            Some(TtfGlyph {
                x_min,
                y_min,
                x_max,
                y_max,
                advance_width,
                lsb,
                contours,
                is_compound: false,
            })
        } else {
            // Compound glyph - for now return empty
            // TODO: implement compound glyph parsing
            Some(TtfGlyph {
                x_min,
                y_min,
                x_max,
                y_max,
                advance_width,
                lsb,
                contours: Vec::new(),
                is_compound: true,
            })
        }
    }

    /// Get glyph for a character
    pub fn get_glyph_for_char(&self, c: char) -> Option<TtfGlyph> {
        let glyph_index = self.get_glyph_index(c as u32)?;
        self.get_glyph(glyph_index)
    }

    /// Calculate pixel size for a given point size and DPI
    pub fn pixel_size(&self, point_size: f32, dpi: f32) -> f32 {
        point_size * dpi / 72.0
    }

    /// Calculate scale factor from design units to pixels
    pub fn scale(&self, pixel_height: f32) -> f32 {
        pixel_height / self.units_per_em as f32
    }
}

/// Rasterized glyph bitmap
#[derive(Debug, Clone)]
pub struct TtfBitmap {
    /// Width in pixels
    pub width: usize,
    /// Height in pixels
    pub height: usize,
    /// Grayscale pixel data (0-255)
    pub data: Vec<u8>,
    /// Horizontal offset from origin
    pub x_offset: i32,
    /// Vertical offset from baseline (positive is up)
    pub y_offset: i32,
    /// Advance width in pixels
    pub advance: f32,
}

/// Simple TrueType rasterizer
pub struct TtfRasterizer<'a> {
    pub font: &'a TtfFont,
    pub pixel_height: f32,
    scale: f32,
}

impl<'a> TtfRasterizer<'a> {
    /// Create a rasterizer for a given pixel height
    pub fn new(font: &'a TtfFont, pixel_height: f32) -> Self {
        let scale = font.scale(pixel_height);
        Self {
            font,
            pixel_height,
            scale,
        }
    }

    /// Rasterize a glyph to a bitmap
    pub fn rasterize_glyph(&self, glyph: &TtfGlyph) -> TtfBitmap {
        if glyph.contours.is_empty() {
            // Empty glyph (like space)
            return TtfBitmap {
                width: 0,
                height: 0,
                data: Vec::new(),
                x_offset: (glyph.lsb as f32 * self.scale) as i32,
                y_offset: 0,
                advance: glyph.advance_width as f32 * self.scale,
            };
        }

        // Calculate bounding box in pixels
        let x_min = floor_f32(glyph.x_min as f32 * self.scale) as i32;
        let y_min = floor_f32(glyph.y_min as f32 * self.scale) as i32;
        let x_max = ceil_f32(glyph.x_max as f32 * self.scale) as i32;
        let y_max = ceil_f32(glyph.y_max as f32 * self.scale) as i32;

        let width = (x_max - x_min + 1).max(1) as usize;
        let height = (y_max - y_min + 1).max(1) as usize;

        let mut bitmap = vec![0u8; width * height];

        // Rasterize using scanline algorithm with anti-aliasing
        // This is a simplified version - uses edge crossing counting
        for contour in &glyph.contours {
            if contour.points.len() < 2 {
                continue;
            }

            // Process each edge
            let mut i = 0;
            while i < contour.points.len() {
                let p0 = &contour.points[i];
                let next_i = (i + 1) % contour.points.len();
                let p1 = &contour.points[next_i];

                if p0.on_curve && p1.on_curve {
                    // Line segment
                    self.rasterize_line(
                        &mut bitmap,
                        width,
                        height,
                        x_min,
                        y_min,
                        p0.x as f32 * self.scale,
                        p0.y as f32 * self.scale,
                        p1.x as f32 * self.scale,
                        p1.y as f32 * self.scale,
                    );
                    i += 1;
                } else if p0.on_curve && !p1.on_curve {
                    // Quadratic Bezier curve
                    let next_next_i = (next_i + 1) % contour.points.len();
                    let p2 = &contour.points[next_next_i];

                    let end_x;
                    let end_y;
                    if p2.on_curve {
                        end_x = p2.x as f32 * self.scale;
                        end_y = p2.y as f32 * self.scale;
                        i += 2;
                    } else {
                        // Implicit on-curve point midway
                        end_x = ((p1.x + p2.x) / 2) as f32 * self.scale;
                        end_y = ((p1.y + p2.y) / 2) as f32 * self.scale;
                        i += 1;
                    }

                    self.rasterize_quadratic(
                        &mut bitmap,
                        width,
                        height,
                        x_min,
                        y_min,
                        p0.x as f32 * self.scale,
                        p0.y as f32 * self.scale,
                        p1.x as f32 * self.scale,
                        p1.y as f32 * self.scale,
                        end_x,
                        end_y,
                    );
                } else {
                    // Off-curve point without preceding on-curve - create implicit start
                    i += 1;
                }
            }
        }

        // Fill using non-zero winding rule
        self.fill_contours(&mut bitmap, width, height, glyph, x_min, y_min);

        TtfBitmap {
            width,
            height,
            data: bitmap,
            x_offset: x_min,
            y_offset: y_max, // Y increases downward in bitmap
            advance: glyph.advance_width as f32 * self.scale,
        }
    }

    /// Rasterize a line segment
    fn rasterize_line(
        &self,
        bitmap: &mut [u8],
        width: usize,
        height: usize,
        x_off: i32,
        y_off: i32,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
    ) {
        // Simple Bresenham-style line with coverage
        let dx = abs_f32(x1 - x0);
        let dy = abs_f32(y1 - y0);
        let steps = ceil_f32(max_f32(dx, dy)) as usize;

        if steps == 0 {
            return;
        }

        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let x = x0 + t * (x1 - x0);
            let y = y0 + t * (y1 - y0);

            let px = (x - x_off as f32) as i32;
            let py = height as i32 - 1 - (y - y_off as f32) as i32;

            if px >= 0 && px < width as i32 && py >= 0 && py < height as i32 {
                let idx = py as usize * width + px as usize;
                bitmap[idx] = bitmap[idx].saturating_add(64);
            }
        }
    }

    /// Rasterize a quadratic Bezier curve
    fn rasterize_quadratic(
        &self,
        bitmap: &mut [u8],
        width: usize,
        height: usize,
        x_off: i32,
        y_off: i32,
        x0: f32,
        y0: f32,
        cx: f32,
        cy: f32,
        x1: f32,
        y1: f32,
    ) {
        // Subdivide curve into line segments
        let steps = 8;
        let mut prev_x = x0;
        let mut prev_y = y0;

        for i in 1..=steps {
            let t = i as f32 / steps as f32;
            let t2 = t * t;
            let mt = 1.0 - t;
            let mt2 = mt * mt;

            let x = mt2 * x0 + 2.0 * mt * t * cx + t2 * x1;
            let y = mt2 * y0 + 2.0 * mt * t * cy + t2 * y1;

            self.rasterize_line(bitmap, width, height, x_off, y_off, prev_x, prev_y, x, y);

            prev_x = x;
            prev_y = y;
        }
    }

    /// Fill contours using scanline algorithm
    fn fill_contours(
        &self,
        bitmap: &mut [u8],
        width: usize,
        height: usize,
        glyph: &TtfGlyph,
        x_off: i32,
        y_off: i32,
    ) {
        // Simple even-odd fill
        for row in 0..height {
            let y = (height - 1 - row) as f32 + y_off as f32 + 0.5;
            let y = y / self.scale;

            let mut crossings = Vec::new();

            for contour in &glyph.contours {
                if contour.points.len() < 2 {
                    continue;
                }

                for i in 0..contour.points.len() {
                    let p0 = &contour.points[i];
                    let p1 = &contour.points[(i + 1) % contour.points.len()];

                    let y0 = p0.y as f32;
                    let y1 = p1.y as f32;

                    // Check if edge crosses this scanline
                    if (y0 <= y && y1 > y) || (y1 <= y && y0 > y) {
                        let t = (y - y0) / (y1 - y0);
                        let x = p0.x as f32 + t * (p1.x - p0.x) as f32;
                        crossings.push(x);
                    }
                }
            }

            crossings.sort_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));

            // Fill between pairs of crossings
            for pair in crossings.chunks(2) {
                if pair.len() == 2 {
                    let x_start = ((pair[0] * self.scale - x_off as f32).max(0.0) as usize).min(width);
                    let x_end = ((pair[1] * self.scale - x_off as f32 + 1.0) as usize).min(width);

                    for x in x_start..x_end {
                        let idx = row * width + x;
                        if idx < bitmap.len() {
                            bitmap[idx] = 255;
                        }
                    }
                }
            }
        }
    }

    /// Rasterize a character
    pub fn rasterize_char(&self, c: char) -> Option<TtfBitmap> {
        let glyph = self.font.get_glyph_for_char(c)?;
        Some(self.rasterize_glyph(&glyph))
    }

    /// Get ascender in pixels
    pub fn ascender(&self) -> f32 {
        self.font.ascender as f32 * self.scale
    }

    /// Get descender in pixels (usually negative)
    pub fn descender(&self) -> f32 {
        self.font.descender as f32 * self.scale
    }

    /// Get line height in pixels
    pub fn line_height(&self) -> f32 {
        (self.font.ascender - self.font.descender + self.font.line_gap) as f32 * self.scale
    }
}

/// TrueType text renderer
pub struct TtfTextRenderer<'a> {
    rasterizer: TtfRasterizer<'a>,
}

impl<'a> TtfTextRenderer<'a> {
    pub fn new(font: &'a TtfFont, pixel_height: f32) -> Self {
        Self {
            rasterizer: TtfRasterizer::new(font, pixel_height),
        }
    }

    /// Draw a character at the given position with alpha blending
    /// set_pixel callback receives (x, y, alpha) where alpha is 0-255
    pub fn draw_char<F>(&self, x: i32, y: i32, c: char, mut set_pixel: F) -> f32
    where
        F: FnMut(i32, i32, u8),
    {
        if let Some(bitmap) = self.rasterizer.rasterize_char(c) {
            let base_y = y - bitmap.y_offset;

            for row in 0..bitmap.height {
                for col in 0..bitmap.width {
                    let alpha = bitmap.data[row * bitmap.width + col];
                    if alpha > 0 {
                        let px = x + bitmap.x_offset + col as i32;
                        let py = base_y + row as i32;
                        set_pixel(px, py, alpha);
                    }
                }
            }

            bitmap.advance
        } else {
            // Return some default advance for missing glyphs
            self.rasterizer.pixel_height * 0.5
        }
    }

    /// Draw a string at the given position
    pub fn draw_string<F>(&self, x: i32, y: i32, s: &str, mut set_pixel: F)
    where
        F: FnMut(i32, i32, u8),
    {
        let mut current_x = x as f32;
        for c in s.chars() {
            if c == '\n' {
                continue;
            }
            let advance = self.draw_char(current_x as i32, y, c, &mut set_pixel);
            current_x += advance;
        }
    }

    /// Measure string width in pixels
    pub fn measure_string(&self, s: &str) -> f32 {
        let mut width = 0.0;
        for c in s.chars() {
            if let Some(glyph) = self.rasterizer.font.get_glyph_for_char(c) {
                width += glyph.advance_width as f32 * self.rasterizer.scale;
            } else {
                width += self.rasterizer.pixel_height * 0.5;
            }
        }
        width
    }

    /// Get line height
    pub fn line_height(&self) -> f32 {
        self.rasterizer.line_height()
    }

    /// Get ascender
    pub fn ascender(&self) -> f32 {
        self.rasterizer.ascender()
    }
}
