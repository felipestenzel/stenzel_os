//! Korean Hangul Input Method Engine
//!
//! Provides Korean (Hangul) input using 2-Set keyboard layout.
//! Combines jamo (초성, 중성, 종성) into syllable blocks.

#![allow(dead_code)]

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use super::ibus::{
    InputMethodEngine, InputMethodType, InputMethodState,
    Candidate, InputEvent, InputResult,
};

/// Hangul syllable constants
const HANGUL_BASE: u32 = 0xAC00;
const CHOSEONG_COUNT: u32 = 19;
const JUNGSEONG_COUNT: u32 = 21;
const JONGSEONG_COUNT: u32 = 28;

/// Choseong (initial consonants)
const CHOSEONG: [char; 19] = [
    'ㄱ', 'ㄲ', 'ㄴ', 'ㄷ', 'ㄸ', 'ㄹ', 'ㅁ', 'ㅂ', 'ㅃ', 'ㅅ',
    'ㅆ', 'ㅇ', 'ㅈ', 'ㅉ', 'ㅊ', 'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ',
];

/// Jungseong (medial vowels)
const JUNGSEONG: [char; 21] = [
    'ㅏ', 'ㅐ', 'ㅑ', 'ㅒ', 'ㅓ', 'ㅔ', 'ㅕ', 'ㅖ', 'ㅗ', 'ㅘ',
    'ㅙ', 'ㅚ', 'ㅛ', 'ㅜ', 'ㅝ', 'ㅞ', 'ㅟ', 'ㅠ', 'ㅡ', 'ㅢ',
    'ㅣ',
];

/// Jongseong (final consonants, 0 = no final)
const JONGSEONG: [Option<char>; 28] = [
    None, Some('ㄱ'), Some('ㄲ'), Some('ㄳ'), Some('ㄴ'), Some('ㄵ'), Some('ㄶ'),
    Some('ㄷ'), Some('ㄹ'), Some('ㄺ'), Some('ㄻ'), Some('ㄼ'), Some('ㄽ'),
    Some('ㄾ'), Some('ㄿ'), Some('ㅀ'), Some('ㅁ'), Some('ㅂ'), Some('ㅄ'),
    Some('ㅅ'), Some('ㅆ'), Some('ㅇ'), Some('ㅈ'), Some('ㅊ'), Some('ㅋ'),
    Some('ㅌ'), Some('ㅍ'), Some('ㅎ'),
];

/// Hangul engine configuration
#[derive(Debug, Clone)]
pub struct HangulConfig {
    /// Keyboard layout (2-Set or 3-Set)
    pub use_2set: bool,
}

impl Default for HangulConfig {
    fn default() -> Self {
        Self { use_2set: true }
    }
}

/// Current composition state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ComposeState {
    /// Empty - waiting for input
    Empty,
    /// Has choseong (initial consonant)
    Choseong,
    /// Has choseong + jungseong (initial + medial)
    ChoseongJungseong,
    /// Has complete syllable (choseong + jungseong + jongseong)
    Complete,
}

/// Hangul input engine
pub struct HangulEngine {
    /// Configuration
    config: HangulConfig,
    /// Current state
    state: InputMethodState,
    /// Compose state
    compose_state: ComposeState,
    /// Current choseong index
    choseong: Option<usize>,
    /// Current jungseong index
    jungseong: Option<usize>,
    /// Current jongseong index
    jongseong: Option<usize>,
    /// Committed text buffer
    committed: String,
}

impl HangulEngine {
    /// Create a new Hangul engine
    pub fn new() -> Self {
        Self {
            config: HangulConfig::default(),
            state: InputMethodState::Idle,
            compose_state: ComposeState::Empty,
            choseong: None,
            jungseong: None,
            jongseong: None,
            committed: String::new(),
        }
    }

    /// Map key to jamo (2-Set keyboard layout)
    fn key_to_jamo(&self, ch: char) -> Option<Jamo> {
        // 2-Set Korean keyboard layout mapping
        match ch {
            // Consonants (Shift for double consonants)
            'r' => Some(Jamo::Consonant(0)),  // ㄱ
            'R' => Some(Jamo::Consonant(1)),  // ㄲ
            's' => Some(Jamo::Consonant(2)),  // ㄴ
            'e' => Some(Jamo::Consonant(3)),  // ㄷ
            'E' => Some(Jamo::Consonant(4)),  // ㄸ
            'f' => Some(Jamo::Consonant(5)),  // ㄹ
            'a' => Some(Jamo::Consonant(6)),  // ㅁ
            'q' => Some(Jamo::Consonant(7)),  // ㅂ
            'Q' => Some(Jamo::Consonant(8)),  // ㅃ
            't' => Some(Jamo::Consonant(9)),  // ㅅ
            'T' => Some(Jamo::Consonant(10)), // ㅆ
            'd' => Some(Jamo::Consonant(11)), // ㅇ
            'w' => Some(Jamo::Consonant(12)), // ㅈ
            'W' => Some(Jamo::Consonant(13)), // ㅉ
            'c' => Some(Jamo::Consonant(14)), // ㅊ
            'z' => Some(Jamo::Consonant(15)), // ㅋ
            'x' => Some(Jamo::Consonant(16)), // ㅌ
            'v' => Some(Jamo::Consonant(17)), // ㅍ
            'g' => Some(Jamo::Consonant(18)), // ㅎ

            // Vowels
            'k' => Some(Jamo::Vowel(0)),  // ㅏ
            'o' => Some(Jamo::Vowel(1)),  // ㅐ
            'i' => Some(Jamo::Vowel(2)),  // ㅑ
            'O' => Some(Jamo::Vowel(3)),  // ㅒ
            'j' => Some(Jamo::Vowel(4)),  // ㅓ
            'p' => Some(Jamo::Vowel(5)),  // ㅔ
            'u' => Some(Jamo::Vowel(6)),  // ㅕ
            'P' => Some(Jamo::Vowel(7)),  // ㅖ
            'h' => Some(Jamo::Vowel(8)),  // ㅗ
            'y' => Some(Jamo::Vowel(12)), // ㅛ
            'n' => Some(Jamo::Vowel(13)), // ㅜ
            'b' => Some(Jamo::Vowel(17)), // ㅠ
            'm' => Some(Jamo::Vowel(18)), // ㅡ
            'l' => Some(Jamo::Vowel(20)), // ㅣ

            _ => None,
        }
    }

    /// Get choseong index from consonant
    fn consonant_to_choseong(&self, idx: usize) -> Option<usize> {
        // Direct mapping for most cases
        Some(idx)
    }

    /// Get jongseong index from consonant
    fn consonant_to_jongseong(&self, idx: usize) -> Option<usize> {
        // Map choseong index to jongseong index
        match idx {
            0 => Some(1),   // ㄱ
            1 => Some(2),   // ㄲ
            2 => Some(4),   // ㄴ
            3 => Some(7),   // ㄷ
            5 => Some(8),   // ㄹ
            6 => Some(16),  // ㅁ
            7 => Some(17),  // ㅂ
            9 => Some(19),  // ㅅ
            10 => Some(20), // ㅆ
            11 => Some(21), // ㅇ
            12 => Some(22), // ㅈ
            14 => Some(23), // ㅊ
            15 => Some(24), // ㅋ
            16 => Some(25), // ㅌ
            17 => Some(26), // ㅍ
            18 => Some(27), // ㅎ
            _ => None,
        }
    }

    /// Compose a complete Hangul syllable
    fn compose_syllable(&self) -> Option<char> {
        let cho = self.choseong?;
        let jung = self.jungseong?;
        let jong = self.jongseong.unwrap_or(0);

        let code = HANGUL_BASE
            + (cho as u32) * JUNGSEONG_COUNT * JONGSEONG_COUNT
            + (jung as u32) * JONGSEONG_COUNT
            + (jong as u32);

        char::from_u32(code)
    }

    /// Get current preedit text
    fn get_preedit(&self) -> String {
        match self.compose_state {
            ComposeState::Empty => String::new(),
            ComposeState::Choseong => {
                if let Some(cho) = self.choseong {
                    CHOSEONG.get(cho).map(|c| c.to_string()).unwrap_or_default()
                } else {
                    String::new()
                }
            }
            ComposeState::ChoseongJungseong | ComposeState::Complete => {
                self.compose_syllable().map(|c| c.to_string()).unwrap_or_default()
            }
        }
    }

    /// Process a jamo input
    fn process_jamo(&mut self, jamo: Jamo) -> InputResult {
        match jamo {
            Jamo::Consonant(idx) => self.process_consonant(idx),
            Jamo::Vowel(idx) => self.process_vowel(idx),
        }
    }

    /// Process consonant input
    fn process_consonant(&mut self, idx: usize) -> InputResult {
        match self.compose_state {
            ComposeState::Empty => {
                // Start new syllable with choseong
                self.choseong = Some(idx);
                self.compose_state = ComposeState::Choseong;
                self.state = InputMethodState::Composing;

                InputResult::Preedit {
                    text: self.get_preedit(),
                    cursor: 1,
                }
            }

            ComposeState::Choseong => {
                // Replace choseong
                self.choseong = Some(idx);

                InputResult::Preedit {
                    text: self.get_preedit(),
                    cursor: 1,
                }
            }

            ComposeState::ChoseongJungseong => {
                // Try to add as jongseong
                if let Some(jong_idx) = self.consonant_to_jongseong(idx) {
                    self.jongseong = Some(jong_idx);
                    self.compose_state = ComposeState::Complete;

                    InputResult::Preedit {
                        text: self.get_preedit(),
                        cursor: 1,
                    }
                } else {
                    // Commit current and start new
                    let committed = self.get_preedit();
                    self.reset_compose();
                    self.choseong = Some(idx);
                    self.compose_state = ComposeState::Choseong;

                    InputResult::Commit(committed)
                }
            }

            ComposeState::Complete => {
                // Commit current syllable, start new one with this consonant
                let committed = self.get_preedit();
                self.reset_compose();
                self.choseong = Some(idx);
                self.compose_state = ComposeState::Choseong;

                InputResult::Commit(committed)
            }
        }
    }

    /// Process vowel input
    fn process_vowel(&mut self, idx: usize) -> InputResult {
        match self.compose_state {
            ComposeState::Empty => {
                // Start with just vowel (ㅇ + vowel)
                self.choseong = Some(11); // ㅇ
                self.jungseong = Some(idx);
                self.compose_state = ComposeState::ChoseongJungseong;
                self.state = InputMethodState::Composing;

                InputResult::Preedit {
                    text: self.get_preedit(),
                    cursor: 1,
                }
            }

            ComposeState::Choseong => {
                // Add jungseong to existing choseong
                self.jungseong = Some(idx);
                self.compose_state = ComposeState::ChoseongJungseong;

                InputResult::Preedit {
                    text: self.get_preedit(),
                    cursor: 1,
                }
            }

            ComposeState::ChoseongJungseong => {
                // Try compound vowel (ㅗ+ㅏ=ㅘ, etc.)
                if let Some(compound) = self.try_compound_vowel(self.jungseong.unwrap(), idx) {
                    self.jungseong = Some(compound);
                    InputResult::Preedit {
                        text: self.get_preedit(),
                        cursor: 1,
                    }
                } else {
                    // Commit current, start new
                    let committed = self.get_preedit();
                    self.reset_compose();
                    self.choseong = Some(11); // ㅇ
                    self.jungseong = Some(idx);
                    self.compose_state = ComposeState::ChoseongJungseong;

                    InputResult::Commit(committed)
                }
            }

            ComposeState::Complete => {
                // Move jongseong to next syllable's choseong
                if let Some(jong) = self.jongseong {
                    // Find corresponding choseong for this jongseong
                    let new_cho = self.jongseong_to_choseong(jong);
                    let committed = {
                        self.jongseong = None;
                        self.compose_state = ComposeState::ChoseongJungseong;
                        self.get_preedit()
                    };

                    self.reset_compose();
                    self.choseong = new_cho;
                    self.jungseong = Some(idx);
                    self.compose_state = ComposeState::ChoseongJungseong;

                    InputResult::Commit(committed)
                } else {
                    // Just commit and start new
                    let committed = self.get_preedit();
                    self.reset_compose();
                    self.choseong = Some(11);
                    self.jungseong = Some(idx);
                    self.compose_state = ComposeState::ChoseongJungseong;

                    InputResult::Commit(committed)
                }
            }
        }
    }

    /// Try to create compound vowel
    fn try_compound_vowel(&self, first: usize, second: usize) -> Option<usize> {
        match (first, second) {
            (8, 0) => Some(9),   // ㅗ + ㅏ = ㅘ
            (8, 1) => Some(10),  // ㅗ + ㅐ = ㅙ
            (8, 20) => Some(11), // ㅗ + ㅣ = ㅚ
            (13, 4) => Some(14), // ㅜ + ㅓ = ㅝ
            (13, 5) => Some(15), // ㅜ + ㅔ = ㅞ
            (13, 20) => Some(16),// ㅜ + ㅣ = ㅟ
            (18, 20) => Some(19),// ㅡ + ㅣ = ㅢ
            _ => None,
        }
    }

    /// Convert jongseong back to choseong
    fn jongseong_to_choseong(&self, jong: usize) -> Option<usize> {
        match jong {
            1 => Some(0),   // ㄱ
            2 => Some(1),   // ㄲ
            4 => Some(2),   // ㄴ
            7 => Some(3),   // ㄷ
            8 => Some(5),   // ㄹ
            16 => Some(6),  // ㅁ
            17 => Some(7),  // ㅂ
            19 => Some(9),  // ㅅ
            20 => Some(10), // ㅆ
            21 => Some(11), // ㅇ
            22 => Some(12), // ㅈ
            23 => Some(14), // ㅊ
            24 => Some(15), // ㅋ
            25 => Some(16), // ㅌ
            26 => Some(17), // ㅍ
            27 => Some(18), // ㅎ
            _ => None,
        }
    }

    /// Reset composition state
    fn reset_compose(&mut self) {
        self.choseong = None;
        self.jungseong = None;
        self.jongseong = None;
        self.compose_state = ComposeState::Empty;
    }
}

/// Jamo type
#[derive(Debug, Clone, Copy)]
enum Jamo {
    Consonant(usize),
    Vowel(usize),
}

impl Default for HangulEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl InputMethodEngine for HangulEngine {
    fn im_type(&self) -> InputMethodType {
        InputMethodType::Hangul
    }

    fn process_key(&mut self, event: InputEvent) -> InputResult {
        if !event.is_press {
            return InputResult::NotHandled;
        }

        if event.modifiers.ctrl || event.modifiers.alt {
            return InputResult::NotHandled;
        }

        let ch = match event.character {
            Some(c) => c,
            None => return InputResult::NotHandled,
        };

        // Handle special keys
        match ch {
            '\x08' | '\x7f' => {
                // Backspace
                match self.compose_state {
                    ComposeState::Empty => return InputResult::NotHandled,
                    ComposeState::Choseong => {
                        self.reset_compose();
                        self.state = InputMethodState::Idle;
                        return InputResult::Preedit {
                            text: String::new(),
                            cursor: 0,
                        };
                    }
                    ComposeState::ChoseongJungseong => {
                        self.jungseong = None;
                        self.compose_state = ComposeState::Choseong;
                        return InputResult::Preedit {
                            text: self.get_preedit(),
                            cursor: 1,
                        };
                    }
                    ComposeState::Complete => {
                        self.jongseong = None;
                        self.compose_state = ComposeState::ChoseongJungseong;
                        return InputResult::Preedit {
                            text: self.get_preedit(),
                            cursor: 1,
                        };
                    }
                }
            }

            '\x1b' => {
                // Escape
                if self.compose_state != ComposeState::Empty {
                    self.reset_compose();
                    self.state = InputMethodState::Idle;
                    return InputResult::Preedit {
                        text: String::new(),
                        cursor: 0,
                    };
                }
            }

            ' ' | '\r' | '\n' => {
                // Space/Enter commits
                if self.compose_state != ComposeState::Empty {
                    let text = self.get_preedit();
                    self.reset_compose();
                    self.state = InputMethodState::Idle;

                    if ch == ' ' {
                        return InputResult::Commit(alloc::format!("{} ", text));
                    } else {
                        return InputResult::Commit(text);
                    }
                }
            }

            _ => {}
        }

        // Try to map to jamo
        if let Some(jamo) = self.key_to_jamo(ch) {
            return self.process_jamo(jamo);
        }

        // For non-Korean characters, commit current composition first
        if self.compose_state != ComposeState::Empty {
            let committed = self.get_preedit();
            self.reset_compose();
            self.state = InputMethodState::Idle;
            return InputResult::Commit(alloc::format!("{}{}", committed, ch));
        }

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
        self.reset_compose();
        self.state = InputMethodState::Idle;
        self.committed.clear();
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
        if self.compose_state != ComposeState::Empty {
            let text = self.get_preedit();
            self.reset_compose();
            self.state = InputMethodState::Idle;
            Some(text)
        } else {
            None
        }
    }

    fn cancel(&mut self) {
        self.reset();
    }
}
