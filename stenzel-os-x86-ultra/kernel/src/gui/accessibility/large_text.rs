//! Large Text Accessibility Feature
//!
//! Provides text scaling for users with low vision.
//! Supports multiple scale factors and per-category scaling.

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;

use crate::sync::IrqSafeMutex;

/// Text scale factor preset
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextScale {
    /// Normal size (100%)
    Normal,
    /// Large (125%)
    Large,
    /// Larger (150%)
    Larger,
    /// Largest (175%)
    Largest,
    /// Extra large (200%)
    ExtraLarge,
    /// Custom scale factor
    Custom(u8), // Percentage as integer (100-400)
}

impl TextScale {
    /// Get scale factor as percentage
    pub fn percentage(&self) -> u16 {
        match self {
            TextScale::Normal => 100,
            TextScale::Large => 125,
            TextScale::Larger => 150,
            TextScale::Largest => 175,
            TextScale::ExtraLarge => 200,
            TextScale::Custom(p) => *p as u16,
        }
    }

    /// Get scale factor as multiplier
    pub fn multiplier(&self) -> f32 {
        self.percentage() as f32 / 100.0
    }

    /// Get name
    pub fn name(&self) -> &'static str {
        match self {
            TextScale::Normal => "Normal",
            TextScale::Large => "Large",
            TextScale::Larger => "Larger",
            TextScale::Largest => "Largest",
            TextScale::ExtraLarge => "Extra Large",
            TextScale::Custom(_) => "Custom",
        }
    }

    /// Get description
    pub fn description(&self) -> String {
        format!("{} ({}%)", self.name(), self.percentage())
    }

    /// Create from percentage
    pub fn from_percentage(p: u16) -> Self {
        match p {
            100 => TextScale::Normal,
            125 => TextScale::Large,
            150 => TextScale::Larger,
            175 => TextScale::Largest,
            200 => TextScale::ExtraLarge,
            p if p <= 400 => TextScale::Custom(p as u8),
            _ => TextScale::Custom(200), // Cap at 200%
        }
    }

    /// Get available presets
    pub fn presets() -> Vec<TextScale> {
        alloc::vec![
            TextScale::Normal,
            TextScale::Large,
            TextScale::Larger,
            TextScale::Largest,
            TextScale::ExtraLarge,
        ]
    }
}

/// Text category for differentiated scaling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextCategory {
    /// Body text / paragraphs
    Body,
    /// UI labels and controls
    Label,
    /// Headings (H1-H6)
    Heading,
    /// Menu items
    Menu,
    /// Button text
    Button,
    /// Input field text
    Input,
    /// Window titles
    WindowTitle,
    /// Tooltip text
    Tooltip,
    /// Status bar text
    StatusBar,
    /// Tab labels
    Tab,
    /// List items
    ListItem,
    /// Table content
    Table,
    /// Code/monospace text
    Code,
    /// Caption text
    Caption,
    /// Small/fine print
    SmallPrint,
}

impl TextCategory {
    /// Get category name
    pub fn name(&self) -> &'static str {
        match self {
            TextCategory::Body => "Body Text",
            TextCategory::Label => "Labels",
            TextCategory::Heading => "Headings",
            TextCategory::Menu => "Menus",
            TextCategory::Button => "Buttons",
            TextCategory::Input => "Input Fields",
            TextCategory::WindowTitle => "Window Titles",
            TextCategory::Tooltip => "Tooltips",
            TextCategory::StatusBar => "Status Bar",
            TextCategory::Tab => "Tabs",
            TextCategory::ListItem => "List Items",
            TextCategory::Table => "Tables",
            TextCategory::Code => "Code",
            TextCategory::Caption => "Captions",
            TextCategory::SmallPrint => "Small Print",
        }
    }

    /// Get default base size in pixels
    pub fn base_size(&self) -> u16 {
        match self {
            TextCategory::Body => 14,
            TextCategory::Label => 12,
            TextCategory::Heading => 24,
            TextCategory::Menu => 13,
            TextCategory::Button => 13,
            TextCategory::Input => 14,
            TextCategory::WindowTitle => 14,
            TextCategory::Tooltip => 11,
            TextCategory::StatusBar => 11,
            TextCategory::Tab => 12,
            TextCategory::ListItem => 13,
            TextCategory::Table => 13,
            TextCategory::Code => 13,
            TextCategory::Caption => 11,
            TextCategory::SmallPrint => 10,
        }
    }

    /// Get minimum size in pixels (accessibility minimum)
    pub fn min_size(&self) -> u16 {
        match self {
            TextCategory::Tooltip | TextCategory::StatusBar | TextCategory::Caption | TextCategory::SmallPrint => 10,
            _ => 12,
        }
    }

    /// All categories
    pub fn all() -> Vec<TextCategory> {
        alloc::vec![
            TextCategory::Body,
            TextCategory::Label,
            TextCategory::Heading,
            TextCategory::Menu,
            TextCategory::Button,
            TextCategory::Input,
            TextCategory::WindowTitle,
            TextCategory::Tooltip,
            TextCategory::StatusBar,
            TextCategory::Tab,
            TextCategory::ListItem,
            TextCategory::Table,
            TextCategory::Code,
            TextCategory::Caption,
            TextCategory::SmallPrint,
        ]
    }
}

/// Font weight for accessibility
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWeight {
    /// Normal weight
    Normal,
    /// Medium weight
    Medium,
    /// Semi-bold weight
    SemiBold,
    /// Bold weight
    Bold,
}

impl FontWeight {
    /// Get weight value (100-900 scale)
    pub fn value(&self) -> u16 {
        match self {
            FontWeight::Normal => 400,
            FontWeight::Medium => 500,
            FontWeight::SemiBold => 600,
            FontWeight::Bold => 700,
        }
    }

    /// Get name
    pub fn name(&self) -> &'static str {
        match self {
            FontWeight::Normal => "Normal",
            FontWeight::Medium => "Medium",
            FontWeight::SemiBold => "Semi-Bold",
            FontWeight::Bold => "Bold",
        }
    }
}

/// Calculated text properties
#[derive(Debug, Clone)]
pub struct TextProperties {
    /// Font size in pixels
    pub size: u16,
    /// Line height multiplier
    pub line_height: f32,
    /// Letter spacing adjustment
    pub letter_spacing: f32,
    /// Word spacing adjustment
    pub word_spacing: f32,
    /// Font weight
    pub weight: FontWeight,
}

impl TextProperties {
    /// Create default properties for a size
    pub fn default_for_size(size: u16) -> Self {
        Self {
            size,
            line_height: 1.5,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            weight: FontWeight::Normal,
        }
    }

    /// Get line height in pixels
    pub fn line_height_px(&self) -> u16 {
        (self.size as f32 * self.line_height) as u16
    }
}

/// Per-category scale overrides
#[derive(Debug, Clone)]
pub struct CategoryScales {
    /// Body text scale
    pub body: Option<TextScale>,
    /// Label scale
    pub label: Option<TextScale>,
    /// Heading scale
    pub heading: Option<TextScale>,
    /// Menu scale
    pub menu: Option<TextScale>,
    /// Button scale
    pub button: Option<TextScale>,
    /// Input scale
    pub input: Option<TextScale>,
}

impl Default for CategoryScales {
    fn default() -> Self {
        Self {
            body: None,
            label: None,
            heading: None,
            menu: None,
            button: None,
            input: None,
        }
    }
}

impl CategoryScales {
    /// Get scale for a category (or None if using global)
    pub fn get(&self, category: TextCategory) -> Option<TextScale> {
        match category {
            TextCategory::Body => self.body,
            TextCategory::Label => self.label,
            TextCategory::Heading => self.heading,
            TextCategory::Menu => self.menu,
            TextCategory::Button => self.button,
            TextCategory::Input => self.input,
            _ => None,
        }
    }

    /// Set scale for a category
    pub fn set(&mut self, category: TextCategory, scale: Option<TextScale>) {
        match category {
            TextCategory::Body => self.body = scale,
            TextCategory::Label => self.label = scale,
            TextCategory::Heading => self.heading = scale,
            TextCategory::Menu => self.menu = scale,
            TextCategory::Button => self.button = scale,
            TextCategory::Input => self.input = scale,
            _ => {}
        }
    }
}

/// Large text configuration
#[derive(Debug, Clone)]
pub struct LargeTextConfig {
    /// Whether large text mode is enabled
    pub enabled: bool,
    /// Global text scale
    pub scale: TextScale,
    /// Per-category scale overrides
    pub category_scales: CategoryScales,
    /// Increase line height for readability
    pub increased_line_height: bool,
    /// Line height multiplier (1.0 = normal)
    pub line_height_factor: f32,
    /// Increase letter spacing
    pub increased_letter_spacing: bool,
    /// Letter spacing factor
    pub letter_spacing_factor: f32,
    /// Increase word spacing
    pub increased_word_spacing: bool,
    /// Word spacing factor
    pub word_spacing_factor: f32,
    /// Use bolder fonts
    pub bold_text: bool,
    /// Minimum font size (pixels)
    pub min_font_size: u16,
    /// Maximum font size (pixels)
    pub max_font_size: u16,
    /// Apply to UI only (not content)
    pub ui_only: bool,
}

impl Default for LargeTextConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            scale: TextScale::Normal,
            category_scales: CategoryScales::default(),
            increased_line_height: true,
            line_height_factor: 1.5,
            increased_letter_spacing: false,
            letter_spacing_factor: 1.0,
            increased_word_spacing: false,
            word_spacing_factor: 1.0,
            bold_text: false,
            min_font_size: 12,
            max_font_size: 72,
            ui_only: false,
        }
    }
}

/// Statistics for large text usage
#[derive(Debug, Clone, Default)]
pub struct LargeTextStats {
    /// Number of times enabled
    pub times_enabled: u64,
    /// Total time enabled (ms)
    pub total_time_enabled_ms: u64,
    /// Number of scale changes
    pub scale_changes: u64,
    /// Number of text calculations
    pub calculations: u64,
    /// Session start timestamp
    pub session_start_ms: u64,
}

/// Large text manager
pub struct LargeTextManager {
    /// Configuration
    config: LargeTextConfig,
    /// Statistics
    stats: LargeTextStats,
    /// Enable timestamp
    enabled_since_ms: Option<u64>,
    /// Callback when mode changes
    on_mode_change: Option<fn(bool)>,
    /// Callback when scale changes
    on_scale_change: Option<fn(TextScale)>,
}

impl LargeTextManager {
    /// Create a new large text manager
    pub fn new() -> Self {
        Self {
            config: LargeTextConfig::default(),
            stats: LargeTextStats::default(),
            enabled_since_ms: None,
            on_mode_change: None,
            on_scale_change: None,
        }
    }

    /// Initialize the manager
    pub fn init(&mut self) {
        self.stats.session_start_ms = crate::time::uptime_ms();
        crate::kprintln!("[large_text] Large text manager initialized");
    }

    /// Enable large text mode
    pub fn enable(&mut self) {
        if !self.config.enabled {
            self.config.enabled = true;
            self.enabled_since_ms = Some(crate::time::uptime_ms());
            self.stats.times_enabled += 1;

            if let Some(callback) = self.on_mode_change {
                callback(true);
            }

            crate::kprintln!("[large_text] Large text mode enabled ({})", self.config.scale.description());
        }
    }

    /// Disable large text mode
    pub fn disable(&mut self) {
        if self.config.enabled {
            self.config.enabled = false;

            // Track duration
            if let Some(start) = self.enabled_since_ms.take() {
                let now = crate::time::uptime_ms();
                self.stats.total_time_enabled_ms += now - start;
            }

            if let Some(callback) = self.on_mode_change {
                callback(false);
            }

            crate::kprintln!("[large_text] Large text mode disabled");
        }
    }

    /// Toggle large text mode
    pub fn toggle(&mut self) {
        if self.config.enabled {
            self.disable();
        } else {
            self.enable();
        }
    }

    /// Check if large text mode is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Set the text scale
    pub fn set_scale(&mut self, scale: TextScale) {
        if self.config.scale != scale {
            self.config.scale = scale;
            self.stats.scale_changes += 1;

            if let Some(callback) = self.on_scale_change {
                callback(scale);
            }

            crate::kprintln!("[large_text] Scale changed to: {}", scale.description());
        }
    }

    /// Get current scale
    pub fn scale(&self) -> TextScale {
        self.config.scale
    }

    /// Set scale for a specific category
    pub fn set_category_scale(&mut self, category: TextCategory, scale: Option<TextScale>) {
        self.config.category_scales.set(category, scale);
    }

    /// Get scale for a specific category
    pub fn get_category_scale(&self, category: TextCategory) -> TextScale {
        self.config.category_scales.get(category).unwrap_or(self.config.scale)
    }

    /// Calculate scaled font size
    pub fn calculate_size(&mut self, base_size: u16, category: TextCategory) -> u16 {
        self.stats.calculations += 1;

        if !self.config.enabled {
            return base_size;
        }

        let scale = self.get_category_scale(category);
        let scaled = (base_size as f32 * scale.multiplier()) as u16;

        // Apply min/max constraints
        let size = scaled.max(self.config.min_font_size).min(self.config.max_font_size);

        // Ensure minimum accessibility size
        size.max(category.min_size())
    }

    /// Calculate text properties for a category
    pub fn calculate_properties(&mut self, category: TextCategory) -> TextProperties {
        let base_size = category.base_size();
        let size = self.calculate_size(base_size, category);

        let line_height = if self.config.enabled && self.config.increased_line_height {
            self.config.line_height_factor
        } else {
            1.4
        };

        let letter_spacing = if self.config.enabled && self.config.increased_letter_spacing {
            (self.config.letter_spacing_factor - 1.0) * size as f32 * 0.1
        } else {
            0.0
        };

        let word_spacing = if self.config.enabled && self.config.increased_word_spacing {
            (self.config.word_spacing_factor - 1.0) * size as f32 * 0.25
        } else {
            0.0
        };

        let weight = if self.config.enabled && self.config.bold_text {
            FontWeight::Medium
        } else {
            FontWeight::Normal
        };

        TextProperties {
            size,
            line_height,
            letter_spacing,
            word_spacing,
            weight,
        }
    }

    /// Get scaled size for a specific base size
    pub fn scaled_size(&mut self, base_size: u16) -> u16 {
        self.calculate_size(base_size, TextCategory::Body)
    }

    /// Set bold text option
    pub fn set_bold_text(&mut self, enabled: bool) {
        self.config.bold_text = enabled;
    }

    /// Set line height factor
    pub fn set_line_height(&mut self, factor: f32) {
        self.config.line_height_factor = factor.max(1.0).min(3.0);
        self.config.increased_line_height = factor > 1.0;
    }

    /// Set letter spacing factor
    pub fn set_letter_spacing(&mut self, factor: f32) {
        self.config.letter_spacing_factor = factor.max(0.5).min(2.0);
        self.config.increased_letter_spacing = factor != 1.0;
    }

    /// Set word spacing factor
    pub fn set_word_spacing(&mut self, factor: f32) {
        self.config.word_spacing_factor = factor.max(0.5).min(2.0);
        self.config.increased_word_spacing = factor != 1.0;
    }

    /// Set minimum font size
    pub fn set_min_size(&mut self, size: u16) {
        self.config.min_font_size = size.max(8).min(32);
    }

    /// Set maximum font size
    pub fn set_max_size(&mut self, size: u16) {
        self.config.max_font_size = size.max(16).min(128);
    }

    /// Get configuration
    pub fn config(&self) -> &LargeTextConfig {
        &self.config
    }

    /// Set configuration
    pub fn set_config(&mut self, config: LargeTextConfig) {
        let was_enabled = self.config.enabled;
        let old_scale = self.config.scale;

        self.config = config;

        // Track changes
        if self.config.scale != old_scale {
            self.stats.scale_changes += 1;
        }

        // Handle enable/disable
        if self.config.enabled != was_enabled {
            if self.config.enabled {
                self.enable();
            } else {
                self.disable();
            }
        }
    }

    /// Set mode change callback
    pub fn set_mode_change_callback(&mut self, callback: fn(bool)) {
        self.on_mode_change = Some(callback);
    }

    /// Set scale change callback
    pub fn set_scale_change_callback(&mut self, callback: fn(TextScale)) {
        self.on_scale_change = Some(callback);
    }

    /// Get statistics
    pub fn stats(&self) -> LargeTextStats {
        let mut stats = self.stats.clone();

        // Add current session duration if enabled
        if let Some(start) = self.enabled_since_ms {
            let now = crate::time::uptime_ms();
            stats.total_time_enabled_ms += now - start;
        }

        stats
    }

    /// Get effective scale multiplier
    pub fn effective_multiplier(&self) -> f32 {
        if self.config.enabled {
            self.config.scale.multiplier()
        } else {
            1.0
        }
    }

    /// Format status string
    pub fn format_status(&self) -> String {
        let stats = self.stats();

        format!(
            "Large Text: {}\n\
             Scale: {}\n\
             Bold text: {}\n\
             Line height: {:.1}x\n\
             Min size: {}px, Max size: {}px\n\
             Times enabled: {}\n\
             Total time enabled: {} ms\n\
             Text calculations: {}",
            if self.config.enabled { "Enabled" } else { "Disabled" },
            self.config.scale.description(),
            if self.config.bold_text { "Yes" } else { "No" },
            self.config.line_height_factor,
            self.config.min_font_size,
            self.config.max_font_size,
            stats.times_enabled,
            stats.total_time_enabled_ms,
            stats.calculations
        )
    }
}

/// Global large text manager instance
static LARGE_TEXT_MANAGER: IrqSafeMutex<Option<LargeTextManager>> = IrqSafeMutex::new(None);

/// Initialize large text mode
pub fn init() {
    let mut manager = LargeTextManager::new();
    manager.init();
    *LARGE_TEXT_MANAGER.lock() = Some(manager);
}

/// Enable large text mode
pub fn enable() {
    if let Some(ref mut manager) = *LARGE_TEXT_MANAGER.lock() {
        manager.enable();
    }
}

/// Disable large text mode
pub fn disable() {
    if let Some(ref mut manager) = *LARGE_TEXT_MANAGER.lock() {
        manager.disable();
    }
}

/// Toggle large text mode
pub fn toggle() {
    if let Some(ref mut manager) = *LARGE_TEXT_MANAGER.lock() {
        manager.toggle();
    }
}

/// Check if large text mode is enabled
pub fn is_enabled() -> bool {
    LARGE_TEXT_MANAGER.lock().as_ref().map(|m| m.is_enabled()).unwrap_or(false)
}

/// Set text scale
pub fn set_scale(scale: TextScale) {
    if let Some(ref mut manager) = *LARGE_TEXT_MANAGER.lock() {
        manager.set_scale(scale);
    }
}

/// Get current scale
pub fn get_scale() -> TextScale {
    LARGE_TEXT_MANAGER.lock().as_ref().map(|m| m.scale()).unwrap_or(TextScale::Normal)
}

/// Calculate scaled size for a base size
pub fn scaled_size(base_size: u16) -> u16 {
    LARGE_TEXT_MANAGER.lock().as_mut().map(|m| m.scaled_size(base_size)).unwrap_or(base_size)
}

/// Calculate text properties for a category
pub fn get_properties(category: TextCategory) -> Option<TextProperties> {
    LARGE_TEXT_MANAGER.lock().as_mut().map(|m| m.calculate_properties(category))
}

/// Get effective multiplier
pub fn multiplier() -> f32 {
    LARGE_TEXT_MANAGER.lock().as_ref().map(|m| m.effective_multiplier()).unwrap_or(1.0)
}

/// Get statistics
pub fn stats() -> Option<LargeTextStats> {
    LARGE_TEXT_MANAGER.lock().as_ref().map(|m| m.stats())
}

/// Get status string
pub fn status() -> String {
    LARGE_TEXT_MANAGER.lock().as_ref()
        .map(|m| m.format_status())
        .unwrap_or_else(|| String::from("Large Text: Not initialized"))
}
