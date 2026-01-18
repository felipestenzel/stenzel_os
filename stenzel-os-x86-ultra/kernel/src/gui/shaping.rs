//! Text Shaping Module
//!
//! Provides complex text layout and shaping for internationalized text rendering.
//! Implements:
//! - Unicode Bidirectional Algorithm (UAX #9)
//! - Script detection and itemization
//! - Arabic/Hebrew text shaping
//! - Ligature handling
//! - Grapheme cluster boundaries
//! - Combining character handling

#![allow(dead_code)]

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;

// ============================================================================
// Script Detection
// ============================================================================

/// Unicode script categories
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Script {
    Latin,
    Arabic,
    Hebrew,
    Cyrillic,
    Greek,
    Han,        // Chinese
    Hiragana,
    Katakana,
    Hangul,     // Korean
    Thai,
    Devanagari, // Hindi
    Tamil,
    Bengali,
    Common,     // Punctuation, numbers
    Inherited,  // Inherits script from context
    Unknown,
}

impl Script {
    /// Check if script is right-to-left
    pub fn is_rtl(&self) -> bool {
        matches!(self, Script::Arabic | Script::Hebrew)
    }

    /// Check if script needs complex shaping
    pub fn needs_shaping(&self) -> bool {
        matches!(
            self,
            Script::Arabic | Script::Hebrew | Script::Devanagari |
            Script::Tamil | Script::Bengali | Script::Thai
        )
    }
}

/// Detect script of a character
pub fn detect_script(c: char) -> Script {
    let cp = c as u32;

    match cp {
        // ASCII Latin
        0x0041..=0x005A | 0x0061..=0x007A => Script::Latin,
        // Latin Extended
        0x00C0..=0x024F => Script::Latin,
        // Latin Extended Additional
        0x1E00..=0x1EFF => Script::Latin,

        // Arabic
        0x0600..=0x06FF => Script::Arabic,
        0x0750..=0x077F => Script::Arabic, // Arabic Supplement
        0x08A0..=0x08FF => Script::Arabic, // Arabic Extended-A
        0xFB50..=0xFDFF => Script::Arabic, // Arabic Presentation Forms-A
        0xFE70..=0xFEFF => Script::Arabic, // Arabic Presentation Forms-B

        // Hebrew
        0x0590..=0x05FF => Script::Hebrew,
        0xFB1D..=0xFB4F => Script::Hebrew, // Hebrew Presentation Forms

        // Cyrillic
        0x0400..=0x04FF => Script::Cyrillic,
        0x0500..=0x052F => Script::Cyrillic, // Cyrillic Supplement

        // Greek
        0x0370..=0x03FF => Script::Greek,
        0x1F00..=0x1FFF => Script::Greek, // Greek Extended

        // CJK (Han)
        0x4E00..=0x9FFF => Script::Han,   // CJK Unified Ideographs
        0x3400..=0x4DBF => Script::Han,   // CJK Extension A
        0x20000..=0x2A6DF => Script::Han, // CJK Extension B

        // Japanese Hiragana
        0x3040..=0x309F => Script::Hiragana,

        // Japanese Katakana
        0x30A0..=0x30FF => Script::Katakana,
        0x31F0..=0x31FF => Script::Katakana, // Katakana Extensions

        // Korean Hangul
        0xAC00..=0xD7AF => Script::Hangul, // Syllables
        0x1100..=0x11FF => Script::Hangul, // Jamo

        // Thai
        0x0E00..=0x0E7F => Script::Thai,

        // Devanagari
        0x0900..=0x097F => Script::Devanagari,

        // Tamil
        0x0B80..=0x0BFF => Script::Tamil,

        // Bengali
        0x0980..=0x09FF => Script::Bengali,

        // Common (numbers, punctuation, etc.)
        0x0020..=0x0040 | 0x005B..=0x0060 | 0x007B..=0x007F => Script::Common,
        0x00A0..=0x00BF => Script::Common,
        0x2000..=0x206F => Script::Common, // General Punctuation
        0x3000..=0x303F => Script::Common, // CJK Symbols

        // Combining marks inherit script
        0x0300..=0x036F => Script::Inherited, // Combining Diacritical
        0x0483..=0x0489 => Script::Inherited, // Cyrillic combining
        0x064B..=0x065F => Script::Inherited, // Arabic combining
        0x0670 => Script::Inherited,          // Arabic superscript alef

        _ => Script::Unknown,
    }
}

// ============================================================================
// Bidirectional Algorithm
// ============================================================================

/// Bidi character type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BidiClass {
    L,   // Left-to-Right
    R,   // Right-to-Left
    AL,  // Arabic Letter
    EN,  // European Number
    ES,  // European Separator
    ET,  // European Terminator
    AN,  // Arabic Number
    CS,  // Common Separator
    NSM, // Non-Spacing Mark
    BN,  // Boundary Neutral
    B,   // Paragraph Separator
    S,   // Segment Separator
    WS,  // Whitespace
    ON,  // Other Neutral
    LRE, // Left-to-Right Embedding
    LRO, // Left-to-Right Override
    RLE, // Right-to-Left Embedding
    RLO, // Right-to-Left Override
    PDF, // Pop Directional Format
    LRI, // Left-to-Right Isolate
    RLI, // Right-to-Left Isolate
    FSI, // First Strong Isolate
    PDI, // Pop Directional Isolate
}

/// Get bidi class for a character
pub fn get_bidi_class(c: char) -> BidiClass {
    let cp = c as u32;

    match cp {
        // Strong LTR
        0x0041..=0x005A | 0x0061..=0x007A => BidiClass::L, // ASCII letters
        0x00C0..=0x00D6 | 0x00D8..=0x00F6 | 0x00F8..=0x00FF => BidiClass::L,
        0x0100..=0x024F => BidiClass::L, // Latin Extended
        0x0370..=0x03FF => BidiClass::L, // Greek
        0x0400..=0x04FF => BidiClass::L, // Cyrillic
        0x4E00..=0x9FFF => BidiClass::L, // CJK
        0x3040..=0x309F => BidiClass::L, // Hiragana
        0x30A0..=0x30FF => BidiClass::L, // Katakana
        0xAC00..=0xD7AF => BidiClass::L, // Hangul

        // Strong RTL
        0x05D0..=0x05EA => BidiClass::R, // Hebrew letters
        0xFB1D..=0xFB4F => BidiClass::R, // Hebrew presentation

        // Arabic Letter (strong RTL with shaping)
        0x0621..=0x064A => BidiClass::AL,
        0x066E..=0x06D3 => BidiClass::AL,
        0x06FA..=0x06FF => BidiClass::AL,
        0xFB50..=0xFDFF => BidiClass::AL, // Arabic Presentation
        0xFE70..=0xFEFF => BidiClass::AL,

        // Numbers
        0x0030..=0x0039 => BidiClass::EN, // ASCII digits
        0x0660..=0x0669 => BidiClass::AN, // Arabic-Indic digits
        0x06F0..=0x06F9 => BidiClass::EN, // Eastern Arabic digits

        // Separators
        0x002B | 0x002D => BidiClass::ES, // + -
        0x0023..=0x0025 | 0x00A2..=0x00A5 | 0x00B0..=0x00B1 => BidiClass::ET,
        0x002C | 0x002E | 0x003A | 0x00A0 => BidiClass::CS, // , . :

        // Whitespace
        0x0009..=0x000D | 0x0020 => BidiClass::WS,
        0x2000..=0x200A => BidiClass::WS, // Various spaces

        // Non-spacing marks
        0x0300..=0x036F => BidiClass::NSM, // Combining marks
        0x0591..=0x05BD => BidiClass::NSM, // Hebrew marks
        0x064B..=0x065F => BidiClass::NSM, // Arabic marks

        // Directional formatting
        0x200E => BidiClass::L,   // LRM
        0x200F => BidiClass::R,   // RLM
        0x202A => BidiClass::LRE,
        0x202B => BidiClass::RLE,
        0x202C => BidiClass::PDF,
        0x202D => BidiClass::LRO,
        0x202E => BidiClass::RLO,
        0x2066 => BidiClass::LRI,
        0x2067 => BidiClass::RLI,
        0x2068 => BidiClass::FSI,
        0x2069 => BidiClass::PDI,

        // Paragraph separator
        0x000A | 0x000D | 0x001C..=0x001E | 0x0085 | 0x2029 => BidiClass::B,

        // Segment separator
        0x0009 | 0x001F => BidiClass::S,

        // Neutral
        _ => BidiClass::ON,
    }
}

/// Represents a run of text with the same direction
#[derive(Debug, Clone)]
pub struct BidiRun {
    /// Start index in the original text
    pub start: usize,
    /// End index (exclusive)
    pub end: usize,
    /// Embedding level (even = LTR, odd = RTL)
    pub level: u8,
    /// Visual order position
    pub visual_pos: usize,
}

/// Bidi paragraph information
#[derive(Debug, Clone)]
pub struct BidiParagraph {
    /// Base direction (true = RTL)
    pub rtl: bool,
    /// Runs in logical order
    pub runs: Vec<BidiRun>,
    /// Character levels
    pub levels: Vec<u8>,
    /// Reordering map (logical -> visual)
    pub reorder_map: Vec<usize>,
}

impl BidiParagraph {
    /// Create a new bidi paragraph analysis
    pub fn new(text: &str, base_rtl: Option<bool>) -> Self {
        let chars: Vec<char> = text.chars().collect();
        let len = chars.len();

        if len == 0 {
            return BidiParagraph {
                rtl: base_rtl.unwrap_or(false),
                runs: Vec::new(),
                levels: Vec::new(),
                reorder_map: Vec::new(),
            };
        }

        // Step P2-P3: Determine base level
        let base_level = match base_rtl {
            Some(true) => 1u8,
            Some(false) => 0u8,
            None => {
                // Find first strong character
                let mut level = 0u8;
                for &c in &chars {
                    match get_bidi_class(c) {
                        BidiClass::L => { level = 0; break; }
                        BidiClass::R | BidiClass::AL => { level = 1; break; }
                        _ => {}
                    }
                }
                level
            }
        };

        // Simplified bidi algorithm: assign levels based on character class
        let mut levels = vec![base_level; len];

        for (i, &c) in chars.iter().enumerate() {
            let class = get_bidi_class(c);
            levels[i] = match class {
                BidiClass::L => {
                    if base_level == 1 { 2 } else { 0 }
                }
                BidiClass::R | BidiClass::AL => {
                    if base_level == 0 { 1 } else { 1 }
                }
                BidiClass::EN | BidiClass::AN => {
                    if base_level == 1 { 2 } else { 0 }
                }
                _ => base_level,
            };
        }

        // Build runs
        let mut runs = Vec::new();
        if !levels.is_empty() {
            let mut run_start = 0;
            let mut run_level = levels[0];

            for i in 1..=len {
                let level = if i < len { levels[i] } else { !run_level };
                if level != run_level {
                    runs.push(BidiRun {
                        start: run_start,
                        end: i,
                        level: run_level,
                        visual_pos: 0,
                    });
                    run_start = i;
                    run_level = level;
                }
            }
        }

        // Build reorder map
        let mut reorder_map: Vec<usize> = (0..len).collect();

        // Find max level
        let max_level = levels.iter().copied().max().unwrap_or(0);

        // Reverse runs at each level
        for level in (1..=max_level).rev() {
            let mut i = 0;
            while i < len {
                // Find start of run at this level
                while i < len && levels[i] < level {
                    i += 1;
                }
                let start = i;

                // Find end of run
                while i < len && levels[i] >= level {
                    i += 1;
                }
                let end = i;

                // Reverse this run in the reorder map
                if start < end {
                    reorder_map[start..end].reverse();
                }
            }
        }

        // Assign visual positions to runs
        for (visual_pos, run) in runs.iter_mut().enumerate() {
            run.visual_pos = visual_pos;
        }

        BidiParagraph {
            rtl: base_level == 1,
            runs,
            levels,
            reorder_map,
        }
    }

    /// Get text in visual order
    pub fn get_visual_order(&self, text: &str) -> String {
        let chars: Vec<char> = text.chars().collect();
        self.reorder_map.iter()
            .filter_map(|&i| chars.get(i).copied())
            .collect()
    }

    /// Check if the paragraph is purely LTR
    pub fn is_pure_ltr(&self) -> bool {
        self.levels.iter().all(|&l| l == 0)
    }

    /// Check if the paragraph is purely RTL
    pub fn is_pure_rtl(&self) -> bool {
        self.levels.iter().all(|&l| l % 2 == 1)
    }
}

// ============================================================================
// Arabic Shaping
// ============================================================================

/// Arabic joining type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArabicJoiningType {
    Right,       // Only joins to the right (alef, dal, etc.)
    Dual,        // Joins to both sides
    Causing,     // Joins neighbors but doesn't connect visually (tatweel)
    NonJoining,  // Doesn't join (space, numbers)
    Transparent, // Marks that don't affect joining
}

/// Arabic character form
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArabicForm {
    Isolated,
    Initial,
    Medial,
    Final,
}

/// Get Arabic joining type for a character
pub fn get_arabic_joining_type(c: char) -> ArabicJoiningType {
    let cp = c as u32;

    match cp {
        // Right-joining (alef, dal, dhal, ra, zain, waw)
        0x0622..=0x0623 => ArabicJoiningType::Right, // Alef variants
        0x0627 => ArabicJoiningType::Right,          // Alef
        0x062F => ArabicJoiningType::Right,          // Dal
        0x0630 => ArabicJoiningType::Right,          // Dhal
        0x0631 => ArabicJoiningType::Right,          // Ra
        0x0632 => ArabicJoiningType::Right,          // Zain
        0x0648 => ArabicJoiningType::Right,          // Waw
        0x0671..=0x0673 => ArabicJoiningType::Right, // Alef variants

        // Dual-joining (most Arabic letters)
        0x0628 => ArabicJoiningType::Dual, // Ba
        0x062A..=0x062E => ArabicJoiningType::Dual, // Ta through Kha
        0x0633..=0x063A => ArabicJoiningType::Dual, // Sin through Ghain
        0x0641..=0x0647 => ArabicJoiningType::Dual, // Fa through Ha
        0x0649..=0x064A => ArabicJoiningType::Dual, // Alef Maksura, Ya

        // Join-causing
        0x0640 => ArabicJoiningType::Causing, // Tatweel

        // Transparent (combining marks)
        0x064B..=0x065F => ArabicJoiningType::Transparent,
        0x0670 => ArabicJoiningType::Transparent,

        // Non-joining by default
        _ => ArabicJoiningType::NonJoining,
    }
}

/// Arabic presentation form mapping
/// Returns (isolated, final, initial, medial) forms
fn get_arabic_forms(c: char) -> Option<(char, char, char, char)> {
    match c {
        // Alef
        '\u{0627}' => Some(('\u{FE8D}', '\u{FE8E}', '\u{FE8D}', '\u{FE8E}')),
        // Ba
        '\u{0628}' => Some(('\u{FE8F}', '\u{FE90}', '\u{FE91}', '\u{FE92}')),
        // Ta Marbuta
        '\u{0629}' => Some(('\u{FE93}', '\u{FE94}', '\u{FE93}', '\u{FE94}')),
        // Ta
        '\u{062A}' => Some(('\u{FE95}', '\u{FE96}', '\u{FE97}', '\u{FE98}')),
        // Tha
        '\u{062B}' => Some(('\u{FE99}', '\u{FE9A}', '\u{FE9B}', '\u{FE9C}')),
        // Jim
        '\u{062C}' => Some(('\u{FE9D}', '\u{FE9E}', '\u{FE9F}', '\u{FEA0}')),
        // Ha
        '\u{062D}' => Some(('\u{FEA1}', '\u{FEA2}', '\u{FEA3}', '\u{FEA4}')),
        // Kha
        '\u{062E}' => Some(('\u{FEA5}', '\u{FEA6}', '\u{FEA7}', '\u{FEA8}')),
        // Dal
        '\u{062F}' => Some(('\u{FEA9}', '\u{FEAA}', '\u{FEA9}', '\u{FEAA}')),
        // Dhal
        '\u{0630}' => Some(('\u{FEAB}', '\u{FEAC}', '\u{FEAB}', '\u{FEAC}')),
        // Ra
        '\u{0631}' => Some(('\u{FEAD}', '\u{FEAE}', '\u{FEAD}', '\u{FEAE}')),
        // Zain
        '\u{0632}' => Some(('\u{FEAF}', '\u{FEB0}', '\u{FEAF}', '\u{FEB0}')),
        // Sin
        '\u{0633}' => Some(('\u{FEB1}', '\u{FEB2}', '\u{FEB3}', '\u{FEB4}')),
        // Shin
        '\u{0634}' => Some(('\u{FEB5}', '\u{FEB6}', '\u{FEB7}', '\u{FEB8}')),
        // Sad
        '\u{0635}' => Some(('\u{FEB9}', '\u{FEBA}', '\u{FEBB}', '\u{FEBC}')),
        // Dad
        '\u{0636}' => Some(('\u{FEBD}', '\u{FEBE}', '\u{FEBF}', '\u{FEC0}')),
        // Tah
        '\u{0637}' => Some(('\u{FEC1}', '\u{FEC2}', '\u{FEC3}', '\u{FEC4}')),
        // Zah
        '\u{0638}' => Some(('\u{FEC5}', '\u{FEC6}', '\u{FEC7}', '\u{FEC8}')),
        // Ain
        '\u{0639}' => Some(('\u{FEC9}', '\u{FECA}', '\u{FECB}', '\u{FECC}')),
        // Ghain
        '\u{063A}' => Some(('\u{FECD}', '\u{FECE}', '\u{FECF}', '\u{FED0}')),
        // Fa
        '\u{0641}' => Some(('\u{FED1}', '\u{FED2}', '\u{FED3}', '\u{FED4}')),
        // Qaf
        '\u{0642}' => Some(('\u{FED5}', '\u{FED6}', '\u{FED7}', '\u{FED8}')),
        // Kaf
        '\u{0643}' => Some(('\u{FED9}', '\u{FEDA}', '\u{FEDB}', '\u{FEDC}')),
        // Lam
        '\u{0644}' => Some(('\u{FEDD}', '\u{FEDE}', '\u{FEDF}', '\u{FEE0}')),
        // Mim
        '\u{0645}' => Some(('\u{FEE1}', '\u{FEE2}', '\u{FEE3}', '\u{FEE4}')),
        // Nun
        '\u{0646}' => Some(('\u{FEE5}', '\u{FEE6}', '\u{FEE7}', '\u{FEE8}')),
        // Ha
        '\u{0647}' => Some(('\u{FEE9}', '\u{FEEA}', '\u{FEEB}', '\u{FEEC}')),
        // Waw
        '\u{0648}' => Some(('\u{FEED}', '\u{FEEE}', '\u{FEED}', '\u{FEEE}')),
        // Alef Maksura
        '\u{0649}' => Some(('\u{FEEF}', '\u{FEF0}', '\u{FEEF}', '\u{FEF0}')),
        // Ya
        '\u{064A}' => Some(('\u{FEF1}', '\u{FEF2}', '\u{FEF3}', '\u{FEF4}')),

        _ => None,
    }
}

/// Shape Arabic text by selecting appropriate presentation forms
pub fn shape_arabic(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();

    if len == 0 {
        return String::new();
    }

    let mut result = String::with_capacity(text.len());

    for i in 0..len {
        let c = chars[i];
        let jt = get_arabic_joining_type(c);

        // Skip transparent characters, add them as-is
        if jt == ArabicJoiningType::Transparent {
            result.push(c);
            continue;
        }

        // Find previous joining character
        let prev_joins = if i > 0 {
            let mut j = i - 1;
            loop {
                let pjt = get_arabic_joining_type(chars[j]);
                if pjt != ArabicJoiningType::Transparent {
                    break matches!(pjt, ArabicJoiningType::Dual | ArabicJoiningType::Causing);
                }
                if j == 0 { break false; }
                j -= 1;
            }
        } else {
            false
        };

        // Find next joining character
        let next_joins = if i < len - 1 {
            let mut j = i + 1;
            loop {
                if j >= len { break false; }
                let njt = get_arabic_joining_type(chars[j]);
                if njt != ArabicJoiningType::Transparent {
                    break matches!(njt, ArabicJoiningType::Dual | ArabicJoiningType::Right | ArabicJoiningType::Causing);
                }
                j += 1;
            }
        } else {
            false
        };

        // Determine form
        let form = match (jt, prev_joins, next_joins) {
            (ArabicJoiningType::Dual, true, true) => ArabicForm::Medial,
            (ArabicJoiningType::Dual, true, false) => ArabicForm::Final,
            (ArabicJoiningType::Dual, false, true) => ArabicForm::Initial,
            (ArabicJoiningType::Dual, false, false) => ArabicForm::Isolated,
            (ArabicJoiningType::Right, true, _) => ArabicForm::Final,
            (ArabicJoiningType::Right, false, _) => ArabicForm::Isolated,
            _ => ArabicForm::Isolated,
        };

        // Get presentation form
        if let Some((iso, fin, ini, med)) = get_arabic_forms(c) {
            let shaped = match form {
                ArabicForm::Isolated => iso,
                ArabicForm::Final => fin,
                ArabicForm::Initial => ini,
                ArabicForm::Medial => med,
            };
            result.push(shaped);
        } else {
            result.push(c);
        }
    }

    result
}

// ============================================================================
// Grapheme Clusters
// ============================================================================

/// Grapheme break property
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphemeBreak {
    CR,
    LF,
    Control,
    Extend,
    ZWJ,
    RegionalIndicator,
    Prepend,
    SpacingMark,
    L,      // Hangul Leading
    V,      // Hangul Vowel
    T,      // Hangul Trailing
    LV,     // Hangul LV syllable
    LVT,    // Hangul LVT syllable
    Other,
}

/// Get grapheme break property
pub fn get_grapheme_break(c: char) -> GraphemeBreak {
    let cp = c as u32;

    match cp {
        0x000D => GraphemeBreak::CR,
        0x000A => GraphemeBreak::LF,

        // Control characters
        0x0000..=0x001F | 0x007F..=0x009F => GraphemeBreak::Control,
        0x200B | 0x200C => GraphemeBreak::Control, // ZWSP, ZWNJ
        0x2028 | 0x2029 => GraphemeBreak::Control, // Line/paragraph separator

        // Extend (combining marks, etc.)
        0x0300..=0x036F => GraphemeBreak::Extend, // Combining diacriticals
        0x0483..=0x0489 => GraphemeBreak::Extend, // Cyrillic combining
        0x0591..=0x05BD => GraphemeBreak::Extend, // Hebrew marks
        0x064B..=0x065F => GraphemeBreak::Extend, // Arabic marks
        0x0670 => GraphemeBreak::Extend,
        0x0E31 | 0x0E34..=0x0E3A | 0x0E47..=0x0E4E => GraphemeBreak::Extend, // Thai
        0xFE00..=0xFE0F => GraphemeBreak::Extend, // Variation selectors
        0x1F3FB..=0x1F3FF => GraphemeBreak::Extend, // Emoji skin tones

        // ZWJ
        0x200D => GraphemeBreak::ZWJ,

        // Regional indicators
        0x1F1E6..=0x1F1FF => GraphemeBreak::RegionalIndicator,

        // Hangul jamo
        0x1100..=0x115F | 0xA960..=0xA97C => GraphemeBreak::L,
        0x1160..=0x11A7 | 0xD7B0..=0xD7C6 => GraphemeBreak::V,
        0x11A8..=0x11FF | 0xD7CB..=0xD7FB => GraphemeBreak::T,

        // Hangul syllables
        0xAC00..=0xD7A3 => {
            // LV if (cp - 0xAC00) % 28 == 0, else LVT
            if (cp - 0xAC00) % 28 == 0 {
                GraphemeBreak::LV
            } else {
                GraphemeBreak::LVT
            }
        }

        // Spacing marks
        0x0903 | 0x093B | 0x093E..=0x0940 => GraphemeBreak::SpacingMark, // Devanagari
        0x0949..=0x094C | 0x094E..=0x094F => GraphemeBreak::SpacingMark,

        _ => GraphemeBreak::Other,
    }
}

/// A grapheme cluster (user-perceived character)
#[derive(Debug, Clone)]
pub struct GraphemeCluster {
    /// Start byte offset
    pub start: usize,
    /// End byte offset (exclusive)
    pub end: usize,
    /// Characters in this cluster
    pub chars: Vec<char>,
}

/// Find grapheme cluster boundaries in text
pub fn find_grapheme_clusters(text: &str) -> Vec<GraphemeCluster> {
    let mut clusters = Vec::new();
    let chars: Vec<(usize, char)> = text.char_indices().collect();

    if chars.is_empty() {
        return clusters;
    }

    let mut cluster_start = 0;
    let mut cluster_chars = vec![chars[0].1];
    let mut prev_break = get_grapheme_break(chars[0].1);
    let mut ri_count = 0;

    if prev_break == GraphemeBreak::RegionalIndicator {
        ri_count = 1;
    }

    for i in 1..chars.len() {
        let (byte_idx, c) = chars[i];
        let curr_break = get_grapheme_break(c);

        // Check if we should break before this character
        let should_break = should_break_grapheme(prev_break, curr_break, ri_count);

        if should_break {
            // End current cluster
            let end_byte = byte_idx;
            clusters.push(GraphemeCluster {
                start: chars[cluster_start].0,
                end: end_byte,
                chars: cluster_chars.clone(),
            });

            // Start new cluster
            cluster_start = i;
            cluster_chars = vec![c];
            ri_count = 0;
        } else {
            cluster_chars.push(c);
        }

        // Update RI count
        if curr_break == GraphemeBreak::RegionalIndicator {
            ri_count += 1;
        } else if curr_break != GraphemeBreak::Extend && curr_break != GraphemeBreak::ZWJ {
            ri_count = 0;
        }

        prev_break = curr_break;
    }

    // Don't forget the last cluster
    clusters.push(GraphemeCluster {
        start: chars[cluster_start].0,
        end: text.len(),
        chars: cluster_chars,
    });

    clusters
}

/// Determine if we should break between two grapheme break classes
fn should_break_grapheme(prev: GraphemeBreak, curr: GraphemeBreak, ri_count: usize) -> bool {
    use GraphemeBreak::*;

    // GB3: Don't break between CR and LF
    if prev == CR && curr == LF {
        return false;
    }

    // GB4: Break after controls
    if matches!(prev, CR | LF | Control) {
        return true;
    }

    // GB5: Break before controls
    if matches!(curr, CR | LF | Control) {
        return true;
    }

    // GB6-8: Hangul rules
    match (prev, curr) {
        (L, L | V | LV | LVT) => return false,
        (LV | V, V | T) => return false,
        (LVT | T, T) => return false,
        _ => {}
    }

    // GB9: Don't break before Extend or ZWJ
    if matches!(curr, Extend | ZWJ) {
        return false;
    }

    // GB9a: Don't break before SpacingMark
    if curr == SpacingMark {
        return false;
    }

    // GB9b: Don't break after Prepend
    if prev == Prepend {
        return false;
    }

    // GB12-13: Regional Indicator pairs
    if prev == RegionalIndicator && curr == RegionalIndicator {
        // Only pair up (break after even count)
        return ri_count % 2 == 0;
    }

    // GB999: Otherwise break
    true
}

/// Count the number of grapheme clusters in text
pub fn grapheme_count(text: &str) -> usize {
    find_grapheme_clusters(text).len()
}

// ============================================================================
// Text Shaping API
// ============================================================================

/// A shaped glyph for rendering
#[derive(Debug, Clone)]
pub struct ShapedGlyph {
    /// The codepoint to render
    pub codepoint: char,
    /// Index in original text (in grapheme clusters)
    pub cluster: usize,
    /// X offset from pen position
    pub x_offset: i32,
    /// Y offset from pen position
    pub y_offset: i32,
    /// X advance (how much to move pen after this glyph)
    pub x_advance: i32,
    /// Y advance
    pub y_advance: i32,
}

/// A shaped text run
#[derive(Debug, Clone)]
pub struct ShapedRun {
    /// Start index in original text
    pub start: usize,
    /// End index
    pub end: usize,
    /// Script of this run
    pub script: Script,
    /// Direction (true = RTL)
    pub rtl: bool,
    /// Shaped glyphs
    pub glyphs: Vec<ShapedGlyph>,
}

/// Shaper configuration
#[derive(Debug, Clone)]
pub struct ShaperConfig {
    /// Enable ligatures
    pub ligatures: bool,
    /// Enable kerning
    pub kerning: bool,
    /// Font size in pixels
    pub size: u32,
    /// Horizontal DPI
    pub dpi_x: u32,
    /// Vertical DPI
    pub dpi_y: u32,
}

impl Default for ShaperConfig {
    fn default() -> Self {
        ShaperConfig {
            ligatures: true,
            kerning: true,
            size: 16,
            dpi_x: 96,
            dpi_y: 96,
        }
    }
}

/// Shape text for rendering
pub fn shape_text(text: &str, config: &ShaperConfig) -> Vec<ShapedRun> {
    let mut runs = Vec::new();

    if text.is_empty() {
        return runs;
    }

    // Do bidi analysis
    let bidi = BidiParagraph::new(text, None);

    // Itemize by script
    let items = itemize_by_script(text);

    // Shape each item
    for item in items {
        let item_text = &text[item.start..item.end];
        let rtl = item.script.is_rtl();

        // Apply Arabic shaping if needed
        let shaped_text = if item.script == Script::Arabic {
            shape_arabic(item_text)
        } else {
            item_text.to_string()
        };

        // Create glyphs
        let mut glyphs = Vec::new();
        let mut x = 0i32;
        let glyph_width = config.size as i32;

        for (cluster, c) in shaped_text.chars().enumerate() {
            let glyph = ShapedGlyph {
                codepoint: c,
                cluster: item.start + cluster,
                x_offset: 0,
                y_offset: 0,
                x_advance: glyph_width,
                y_advance: 0,
            };
            glyphs.push(glyph);
            x += glyph_width;
        }

        // Reverse glyph order for RTL
        if rtl {
            glyphs.reverse();
            // Recalculate offsets for RTL
            let mut x = 0;
            for glyph in &mut glyphs {
                glyph.x_offset = x;
                x += glyph.x_advance;
            }
        }

        runs.push(ShapedRun {
            start: item.start,
            end: item.end,
            script: item.script,
            rtl,
            glyphs,
        });
    }

    // Reorder runs according to bidi levels
    if !bidi.is_pure_ltr() {
        // Simple reordering: reverse RTL runs
        let mut visual_runs = Vec::new();
        for run in runs {
            if run.rtl {
                visual_runs.insert(0, run);
            } else {
                visual_runs.push(run);
            }
        }
        return visual_runs;
    }

    runs
}

/// Script run for itemization
#[derive(Debug, Clone)]
struct ScriptRun {
    start: usize,
    end: usize,
    script: Script,
}

/// Itemize text by script
fn itemize_by_script(text: &str) -> Vec<ScriptRun> {
    let mut items = Vec::new();

    let chars: Vec<(usize, char)> = text.char_indices().collect();
    if chars.is_empty() {
        return items;
    }

    let mut run_start = 0;
    let mut run_script = detect_script(chars[0].1);

    // Resolve inherited/common scripts
    if matches!(run_script, Script::Inherited | Script::Common) {
        run_script = Script::Latin; // Default
    }

    for i in 1..chars.len() {
        let (_, c) = chars[i];
        let mut script = detect_script(c);

        // Inherited takes the previous script
        if script == Script::Inherited {
            script = run_script;
        }

        // Common can extend any run
        if script == Script::Common {
            continue;
        }

        if script != run_script {
            // End current run
            items.push(ScriptRun {
                start: chars[run_start].0,
                end: chars[i].0,
                script: run_script,
            });
            run_start = i;
            run_script = script;
        }
    }

    // Last run
    items.push(ScriptRun {
        start: chars[run_start].0,
        end: text.len(),
        script: run_script,
    });

    items
}

// ============================================================================
// Ligature Handling
// ============================================================================

/// Common Latin ligatures
pub fn apply_latin_ligatures(text: &str) -> String {
    let mut result = text.to_string();

    // Common ligatures
    let ligatures = [
        ("ff", "\u{FB00}"),  // ff ligature
        ("fi", "\u{FB01}"),  // fi ligature
        ("fl", "\u{FB02}"),  // fl ligature
        ("ffi", "\u{FB03}"), // ffi ligature
        ("ffl", "\u{FB04}"), // ffl ligature
        ("st", "\u{FB06}"),  // st ligature (historical)
    ];

    // Apply in order of length (longest first)
    for &(from, to) in &ligatures {
        result = result.replace(from, to);
    }

    result
}

/// Arabic ligatures (Lam-Alef combinations)
pub fn apply_arabic_ligatures(text: &str) -> String {
    let mut result = text.to_string();

    // Lam-Alef ligatures
    let ligatures = [
        ("\u{0644}\u{0622}", "\u{FEF5}"), // Lam + Alef Madda isolated
        ("\u{0644}\u{0623}", "\u{FEF7}"), // Lam + Alef Hamza Above isolated
        ("\u{0644}\u{0625}", "\u{FEF9}"), // Lam + Alef Hamza Below isolated
        ("\u{0644}\u{0627}", "\u{FEFB}"), // Lam + Alef isolated
    ];

    for &(from, to) in &ligatures {
        result = result.replace(from, to);
    }

    result
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Measure the visual width of shaped text in glyph units
pub fn measure_shaped_width(runs: &[ShapedRun]) -> i32 {
    runs.iter()
        .flat_map(|r| &r.glyphs)
        .map(|g| g.x_advance)
        .sum()
}

/// Get the visual string (for debugging/display)
pub fn get_visual_string(runs: &[ShapedRun]) -> String {
    runs.iter()
        .flat_map(|r| r.glyphs.iter().map(|g| g.codepoint))
        .collect()
}

/// Check if text contains any RTL characters
pub fn has_rtl(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(get_bidi_class(c), BidiClass::R | BidiClass::AL)
    })
}

/// Check if text contains any complex scripts requiring shaping
pub fn needs_shaping(text: &str) -> bool {
    text.chars().any(|c| detect_script(c).needs_shaping())
}

/// Initialize the shaping module
pub fn init() {
    // No runtime initialization needed for now
    // Future: could load additional shaping data
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_script_detection() {
        assert_eq!(detect_script('A'), Script::Latin);
        assert_eq!(detect_script('א'), Script::Hebrew);
        assert_eq!(detect_script('ب'), Script::Arabic);
        assert_eq!(detect_script('你'), Script::Han);
        assert_eq!(detect_script('あ'), Script::Hiragana);
    }

    #[test]
    fn test_bidi_ltr() {
        let bidi = BidiParagraph::new("Hello World", None);
        assert!(!bidi.rtl);
        assert!(bidi.is_pure_ltr());
    }

    #[test]
    fn test_grapheme_clusters() {
        let clusters = find_grapheme_clusters("café");
        assert_eq!(clusters.len(), 4);
    }
}
