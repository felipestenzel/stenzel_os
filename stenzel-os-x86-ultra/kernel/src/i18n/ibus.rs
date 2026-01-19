//! IBus-like Input Method Framework
//!
//! Provides an input method framework for typing in complex scripts:
//! - Chinese (Pinyin, Wubi)
//! - Japanese (Hiragana, Katakana, Kanji)
//! - Korean (Hangul)
//! - Arabic (with RTL support)
//! - Other complex input methods
//!
//! Based on IBus (Intelligent Input Bus) architecture.

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;

use crate::sync::IrqSafeMutex;

/// Input method type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum InputMethodType {
    /// Direct input (no conversion)
    Direct,
    /// Chinese Pinyin
    Pinyin,
    /// Chinese Wubi
    Wubi,
    /// Japanese Romaji to Hiragana/Katakana
    Japanese,
    /// Korean Hangul
    Hangul,
    /// Arabic
    Arabic,
    /// Custom input method
    Custom,
}

impl InputMethodType {
    /// Get display name
    pub fn name(&self) -> &'static str {
        match self {
            InputMethodType::Direct => "Direct Input",
            InputMethodType::Pinyin => "Chinese Pinyin",
            InputMethodType::Wubi => "Chinese Wubi",
            InputMethodType::Japanese => "Japanese",
            InputMethodType::Hangul => "Korean Hangul",
            InputMethodType::Arabic => "Arabic",
            InputMethodType::Custom => "Custom",
        }
    }

    /// Get language code
    pub fn language_code(&self) -> &'static str {
        match self {
            InputMethodType::Direct => "en",
            InputMethodType::Pinyin => "zh",
            InputMethodType::Wubi => "zh",
            InputMethodType::Japanese => "ja",
            InputMethodType::Hangul => "ko",
            InputMethodType::Arabic => "ar",
            InputMethodType::Custom => "",
        }
    }

    /// All input method types
    pub fn all() -> &'static [InputMethodType] {
        &[
            InputMethodType::Direct,
            InputMethodType::Pinyin,
            InputMethodType::Wubi,
            InputMethodType::Japanese,
            InputMethodType::Hangul,
            InputMethodType::Arabic,
        ]
    }
}

/// Input method state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMethodState {
    /// Idle - not composing
    Idle,
    /// Composing - building input
    Composing,
    /// Selecting - choosing from candidates
    Selecting,
    /// Converting - converting input (Japanese)
    Converting,
}

impl InputMethodState {
    pub fn name(&self) -> &'static str {
        match self {
            InputMethodState::Idle => "Idle",
            InputMethodState::Composing => "Composing",
            InputMethodState::Selecting => "Selecting",
            InputMethodState::Converting => "Converting",
        }
    }
}

/// Candidate for input selection
#[derive(Debug, Clone)]
pub struct Candidate {
    /// The text to insert
    pub text: String,
    /// Display label (if different)
    pub label: Option<String>,
    /// Annotation or pronunciation guide
    pub annotation: Option<String>,
    /// Frequency/priority score
    pub score: u32,
}

impl Candidate {
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            label: None,
            annotation: None,
            score: 0,
        }
    }

    pub fn with_annotation(text: &str, annotation: &str) -> Self {
        Self {
            text: text.to_string(),
            label: None,
            annotation: Some(annotation.to_string()),
            score: 0,
        }
    }

    pub fn display_text(&self) -> &str {
        self.label.as_ref().unwrap_or(&self.text)
    }
}

/// Input event from keyboard
#[derive(Debug, Clone, Copy)]
pub struct InputEvent {
    /// Key code
    pub keycode: u8,
    /// Character if printable
    pub character: Option<char>,
    /// Modifier keys
    pub modifiers: KeyModifiers,
    /// Is key press (vs release)
    pub is_press: bool,
}

/// Key modifiers
#[derive(Debug, Clone, Copy, Default)]
pub struct KeyModifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub super_key: bool,
}

impl KeyModifiers {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn has_any(&self) -> bool {
        self.shift || self.ctrl || self.alt || self.super_key
    }
}

/// Result of processing an input event
#[derive(Debug, Clone)]
pub enum InputResult {
    /// Event was not handled, pass through
    NotHandled,
    /// Event was consumed, no output
    Consumed,
    /// Commit this text
    Commit(String),
    /// Update preedit text (composing)
    Preedit {
        text: String,
        cursor: usize,
    },
    /// Show candidates
    ShowCandidates(Vec<Candidate>),
    /// Hide candidates
    HideCandidates,
}

/// Input method engine trait
pub trait InputMethodEngine: Send + Sync {
    /// Get the input method type
    fn im_type(&self) -> InputMethodType;

    /// Process an input event
    fn process_key(&mut self, event: InputEvent) -> InputResult;

    /// Get current preedit text
    fn preedit(&self) -> &str;

    /// Get current candidates
    fn candidates(&self) -> &[Candidate];

    /// Get current state
    fn state(&self) -> InputMethodState;

    /// Reset the engine
    fn reset(&mut self);

    /// Get selected candidate index
    fn selected_index(&self) -> usize;

    /// Select candidate by index
    fn select_candidate(&mut self, index: usize) -> Option<String>;

    /// Move selection up
    fn move_up(&mut self);

    /// Move selection down
    fn move_down(&mut self);

    /// Page up in candidates
    fn page_up(&mut self);

    /// Page down in candidates
    fn page_down(&mut self);

    /// Commit current selection/preedit
    fn commit(&mut self) -> Option<String>;

    /// Cancel composition
    fn cancel(&mut self);
}

/// Direct input engine (no conversion)
pub struct DirectInputEngine;

impl InputMethodEngine for DirectInputEngine {
    fn im_type(&self) -> InputMethodType {
        InputMethodType::Direct
    }

    fn process_key(&mut self, event: InputEvent) -> InputResult {
        if event.is_press {
            if let Some(c) = event.character {
                if !event.modifiers.ctrl && !event.modifiers.alt {
                    return InputResult::Commit(c.to_string());
                }
            }
        }
        InputResult::NotHandled
    }

    fn preedit(&self) -> &str { "" }
    fn candidates(&self) -> &[Candidate] { &[] }
    fn state(&self) -> InputMethodState { InputMethodState::Idle }
    fn reset(&mut self) {}
    fn selected_index(&self) -> usize { 0 }
    fn select_candidate(&mut self, _index: usize) -> Option<String> { None }
    fn move_up(&mut self) {}
    fn move_down(&mut self) {}
    fn page_up(&mut self) {}
    fn page_down(&mut self) {}
    fn commit(&mut self) -> Option<String> { None }
    fn cancel(&mut self) {}
}

/// Input method bus configuration
#[derive(Debug, Clone)]
pub struct IBusConfig {
    /// Enabled input methods
    pub enabled_methods: Vec<InputMethodType>,
    /// Current input method
    pub current_method: InputMethodType,
    /// Toggle key (default: Ctrl+Space)
    pub toggle_keycode: u8,
    pub toggle_modifiers: KeyModifiers,
    /// Switch key (default: Ctrl+Shift)
    pub switch_keycode: u8,
    pub switch_modifiers: KeyModifiers,
    /// Max candidates to show
    pub max_candidates: usize,
    /// Candidate window orientation (horizontal/vertical)
    pub horizontal_candidates: bool,
    /// Show candidate numbers
    pub show_candidate_numbers: bool,
    /// Auto-commit on single candidate
    pub auto_commit_single: bool,
    /// Candidate page size
    pub page_size: usize,
}

impl Default for IBusConfig {
    fn default() -> Self {
        Self {
            enabled_methods: vec![InputMethodType::Direct],
            current_method: InputMethodType::Direct,
            toggle_keycode: 0x39, // Space
            toggle_modifiers: KeyModifiers { ctrl: true, ..Default::default() },
            switch_keycode: 0x2A, // Left Shift
            switch_modifiers: KeyModifiers { ctrl: true, shift: true, ..Default::default() },
            max_candidates: 10,
            horizontal_candidates: false,
            show_candidate_numbers: true,
            auto_commit_single: true,
            page_size: 5,
        }
    }
}

/// Input method statistics
#[derive(Debug, Clone, Default)]
pub struct IBusStats {
    /// Total keys processed
    pub keys_processed: u64,
    /// Characters committed
    pub characters_committed: u64,
    /// Candidates shown
    pub candidates_shown: u64,
    /// Input method switches
    pub im_switches: u64,
    /// Composition cancellations
    pub cancellations: u64,
}

/// Callback types
pub type OnCommitCallback = fn(&str);
pub type OnPreeditCallback = fn(&str, usize);
pub type OnCandidatesCallback = fn(&[Candidate], usize);
pub type OnIMChangeCallback = fn(InputMethodType);

/// Main IBus manager
pub struct IBusManager {
    /// Configuration
    config: IBusConfig,
    /// Current engine
    current_engine: InputMethodType,
    /// Registered engines
    engines: BTreeMap<InputMethodType, EngineHolder>,
    /// Statistics
    stats: IBusStats,
    /// Commit callback
    on_commit: Option<OnCommitCallback>,
    /// Preedit callback
    on_preedit: Option<OnPreeditCallback>,
    /// Candidates callback
    on_candidates: Option<OnCandidatesCallback>,
    /// IM change callback
    on_im_change: Option<OnIMChangeCallback>,
}

/// Holder for engine (since we can't have dyn Trait directly)
enum EngineHolder {
    Direct(DirectInputEngine),
    // Add other engines as they're implemented
}

impl EngineHolder {
    fn as_engine(&self) -> &dyn InputMethodEngine {
        match self {
            EngineHolder::Direct(e) => e,
        }
    }

    fn as_engine_mut(&mut self) -> &mut dyn InputMethodEngine {
        match self {
            EngineHolder::Direct(e) => e,
        }
    }
}

impl IBusManager {
    /// Create a new IBus manager
    pub fn new() -> Self {
        let mut engines = BTreeMap::new();
        engines.insert(InputMethodType::Direct, EngineHolder::Direct(DirectInputEngine));

        Self {
            config: IBusConfig::default(),
            current_engine: InputMethodType::Direct,
            engines,
            stats: IBusStats::default(),
            on_commit: None,
            on_preedit: None,
            on_candidates: None,
            on_im_change: None,
        }
    }

    /// Initialize the IBus manager
    pub fn init(&mut self) {
        // Register default engines
        self.config.enabled_methods = vec![
            InputMethodType::Direct,
        ];

        crate::kprintln!("[ibus] Input method framework initialized");
    }

    /// Enable an input method
    pub fn enable_method(&mut self, im_type: InputMethodType) {
        if !self.config.enabled_methods.contains(&im_type) {
            self.config.enabled_methods.push(im_type);
        }
    }

    /// Disable an input method
    pub fn disable_method(&mut self, im_type: InputMethodType) {
        // Can't disable Direct
        if im_type != InputMethodType::Direct {
            self.config.enabled_methods.retain(|&m| m != im_type);
        }
    }

    /// Set current input method
    pub fn set_current_method(&mut self, im_type: InputMethodType) -> bool {
        if self.engines.contains_key(&im_type) {
            // Reset current engine
            if let Some(engine) = self.engines.get_mut(&self.current_engine) {
                engine.as_engine_mut().reset();
            }

            self.current_engine = im_type;
            self.config.current_method = im_type;
            self.stats.im_switches += 1;

            if let Some(callback) = self.on_im_change {
                callback(im_type);
            }

            crate::kprintln!("[ibus] Switched to {}", im_type.name());
            true
        } else {
            false
        }
    }

    /// Get current input method
    pub fn current_method(&self) -> InputMethodType {
        self.current_engine
    }

    /// Toggle to next input method
    pub fn toggle_method(&mut self) {
        if self.config.enabled_methods.len() <= 1 {
            return;
        }

        let current_idx = self.config.enabled_methods
            .iter()
            .position(|&m| m == self.current_engine)
            .unwrap_or(0);

        let next_idx = (current_idx + 1) % self.config.enabled_methods.len();
        let next_method = self.config.enabled_methods[next_idx];
        self.set_current_method(next_method);
    }

    /// Process a key event
    pub fn process_key(&mut self, event: InputEvent) -> InputResult {
        self.stats.keys_processed += 1;

        // Check for toggle/switch shortcuts
        if event.is_press {
            // Toggle IM
            if event.keycode == self.config.toggle_keycode
                && event.modifiers.ctrl == self.config.toggle_modifiers.ctrl
                && event.modifiers.alt == self.config.toggle_modifiers.alt
                && event.modifiers.shift == self.config.toggle_modifiers.shift
            {
                self.toggle_method();
                return InputResult::Consumed;
            }
        }

        // Get current engine
        let engine = match self.engines.get_mut(&self.current_engine) {
            Some(e) => e,
            None => return InputResult::NotHandled,
        };

        // Process with engine
        let result = engine.as_engine_mut().process_key(event);

        // Handle result
        match &result {
            InputResult::Commit(text) => {
                self.stats.characters_committed += text.chars().count() as u64;
                if let Some(callback) = self.on_commit {
                    callback(text);
                }
            }
            InputResult::Preedit { text, cursor } => {
                if let Some(callback) = self.on_preedit {
                    callback(text, *cursor);
                }
            }
            InputResult::ShowCandidates(candidates) => {
                self.stats.candidates_shown += 1;
                if let Some(callback) = self.on_candidates {
                    let idx = engine.as_engine().selected_index();
                    callback(candidates, idx);
                }
            }
            _ => {}
        }

        result
    }

    /// Commit current composition
    pub fn commit(&mut self) -> Option<String> {
        let engine = self.engines.get_mut(&self.current_engine)?;
        let text = engine.as_engine_mut().commit();

        if let Some(ref t) = text {
            self.stats.characters_committed += t.chars().count() as u64;
            if let Some(callback) = self.on_commit {
                callback(t);
            }
        }

        text
    }

    /// Cancel current composition
    pub fn cancel(&mut self) {
        if let Some(engine) = self.engines.get_mut(&self.current_engine) {
            engine.as_engine_mut().cancel();
            self.stats.cancellations += 1;

            if let Some(callback) = self.on_preedit {
                callback("", 0);
            }
        }
    }

    /// Reset current engine
    pub fn reset(&mut self) {
        if let Some(engine) = self.engines.get_mut(&self.current_engine) {
            engine.as_engine_mut().reset();
        }
    }

    /// Get current preedit text
    pub fn preedit(&self) -> &str {
        self.engines.get(&self.current_engine)
            .map(|e| e.as_engine().preedit())
            .unwrap_or("")
    }

    /// Get current candidates
    pub fn candidates(&self) -> Vec<Candidate> {
        self.engines.get(&self.current_engine)
            .map(|e| e.as_engine().candidates().to_vec())
            .unwrap_or_default()
    }

    /// Get current state
    pub fn state(&self) -> InputMethodState {
        self.engines.get(&self.current_engine)
            .map(|e| e.as_engine().state())
            .unwrap_or(InputMethodState::Idle)
    }

    /// Select candidate by index
    pub fn select_candidate(&mut self, index: usize) -> Option<String> {
        let engine = self.engines.get_mut(&self.current_engine)?;
        let text = engine.as_engine_mut().select_candidate(index);

        if let Some(ref t) = text {
            self.stats.characters_committed += t.chars().count() as u64;
            if let Some(callback) = self.on_commit {
                callback(t);
            }
        }

        text
    }

    /// Set commit callback
    pub fn set_commit_callback(&mut self, callback: OnCommitCallback) {
        self.on_commit = Some(callback);
    }

    /// Set preedit callback
    pub fn set_preedit_callback(&mut self, callback: OnPreeditCallback) {
        self.on_preedit = Some(callback);
    }

    /// Set candidates callback
    pub fn set_candidates_callback(&mut self, callback: OnCandidatesCallback) {
        self.on_candidates = Some(callback);
    }

    /// Set IM change callback
    pub fn set_im_change_callback(&mut self, callback: OnIMChangeCallback) {
        self.on_im_change = Some(callback);
    }

    /// Get enabled methods
    pub fn enabled_methods(&self) -> &[InputMethodType] {
        &self.config.enabled_methods
    }

    /// Get configuration
    pub fn config(&self) -> &IBusConfig {
        &self.config
    }

    /// Set configuration
    pub fn set_config(&mut self, config: IBusConfig) {
        self.config = config;
    }

    /// Get statistics
    pub fn stats(&self) -> &IBusStats {
        &self.stats
    }

    /// Format status string
    pub fn format_status(&self) -> String {
        alloc::format!(
            "IBus Input Methods:\n\
             Current: {}\n\
             State: {}\n\
             Enabled: {:?}\n\
             Keys processed: {}\n\
             Characters committed: {}\n\
             IM switches: {}",
            self.current_engine.name(),
            self.state().name(),
            self.config.enabled_methods.iter().map(|m| m.name()).collect::<Vec<_>>(),
            self.stats.keys_processed,
            self.stats.characters_committed,
            self.stats.im_switches
        )
    }
}

impl Default for IBusManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Global Instance
// ============================================================================

/// Global IBus manager
static IBUS: IrqSafeMutex<Option<IBusManager>> = IrqSafeMutex::new(None);

/// Initialize IBus framework
pub fn init() {
    let mut mgr = IBusManager::new();
    mgr.init();
    *IBUS.lock() = Some(mgr);
}

/// Get current input method
pub fn current_method() -> InputMethodType {
    IBUS.lock().as_ref().map(|m| m.current_method()).unwrap_or(InputMethodType::Direct)
}

/// Set current input method
pub fn set_method(im_type: InputMethodType) -> bool {
    IBUS.lock().as_mut().map(|m| m.set_current_method(im_type)).unwrap_or(false)
}

/// Toggle input method
pub fn toggle() {
    if let Some(ref mut mgr) = *IBUS.lock() {
        mgr.toggle_method();
    }
}

/// Process key event
pub fn process_key(event: InputEvent) -> InputResult {
    IBUS.lock().as_mut()
        .map(|m| m.process_key(event))
        .unwrap_or(InputResult::NotHandled)
}

/// Commit current composition
pub fn commit() -> Option<String> {
    IBUS.lock().as_mut().and_then(|m| m.commit())
}

/// Cancel current composition
pub fn cancel() {
    if let Some(ref mut mgr) = *IBUS.lock() {
        mgr.cancel();
    }
}

/// Get preedit text
pub fn preedit() -> String {
    IBUS.lock().as_ref().map(|m| m.preedit().to_string()).unwrap_or_default()
}

/// Get candidates
pub fn candidates() -> Vec<Candidate> {
    IBUS.lock().as_ref().map(|m| m.candidates()).unwrap_or_default()
}

/// Get current state
pub fn state() -> InputMethodState {
    IBUS.lock().as_ref().map(|m| m.state()).unwrap_or(InputMethodState::Idle)
}

/// Get status string
pub fn status() -> String {
    IBUS.lock().as_ref().map(|m| m.format_status()).unwrap_or_else(|| "IBus not initialized".to_string())
}

/// Get stats
pub fn stats() -> IBusStats {
    IBUS.lock().as_ref().map(|m| m.stats().clone()).unwrap_or_default()
}
