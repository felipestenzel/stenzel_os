//! High Contrast Mode for Accessibility
//!
//! Provides high contrast visual themes for users with low vision.
//! Supports multiple contrast schemes and customization options.

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;

use crate::sync::IrqSafeMutex;

/// High contrast color scheme preset
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContrastScheme {
    /// Black text on white background (default high contrast)
    BlackOnWhite,
    /// White text on black background
    WhiteOnBlack,
    /// Yellow text on black background
    YellowOnBlack,
    /// Green text on black background
    GreenOnBlack,
    /// Custom scheme with user-defined colors
    Custom,
}

impl ContrastScheme {
    /// Get scheme name
    pub fn name(&self) -> &'static str {
        match self {
            ContrastScheme::BlackOnWhite => "High Contrast White",
            ContrastScheme::WhiteOnBlack => "High Contrast Black",
            ContrastScheme::YellowOnBlack => "High Contrast Yellow",
            ContrastScheme::GreenOnBlack => "High Contrast Green",
            ContrastScheme::Custom => "Custom High Contrast",
        }
    }

    /// Get scheme description
    pub fn description(&self) -> &'static str {
        match self {
            ContrastScheme::BlackOnWhite => "Black text on white background",
            ContrastScheme::WhiteOnBlack => "White text on black background",
            ContrastScheme::YellowOnBlack => "Yellow text on black background",
            ContrastScheme::GreenOnBlack => "Green text on black background",
            ContrastScheme::Custom => "User-defined color scheme",
        }
    }
}

/// RGBA color representation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    /// Create a new color
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Create an opaque color
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Black color
    pub const fn black() -> Self {
        Self::rgb(0, 0, 0)
    }

    /// White color
    pub const fn white() -> Self {
        Self::rgb(255, 255, 255)
    }

    /// Yellow color (for high contrast)
    pub const fn yellow() -> Self {
        Self::rgb(255, 255, 0)
    }

    /// Green color (for high contrast)
    pub const fn green() -> Self {
        Self::rgb(0, 255, 0)
    }

    /// Blue color (for links)
    pub const fn blue() -> Self {
        Self::rgb(0, 128, 255)
    }

    /// Red color (for errors/alerts)
    pub const fn red() -> Self {
        Self::rgb(255, 0, 0)
    }

    /// Cyan color (for links on dark background)
    pub const fn cyan() -> Self {
        Self::rgb(0, 255, 255)
    }

    /// Magenta color (for visited links)
    pub const fn magenta() -> Self {
        Self::rgb(255, 0, 255)
    }

    /// Orange color (for warnings)
    pub const fn orange() -> Self {
        Self::rgb(255, 165, 0)
    }

    /// Create from hex value
    pub const fn from_hex(hex: u32) -> Self {
        Self {
            r: ((hex >> 16) & 0xFF) as u8,
            g: ((hex >> 8) & 0xFF) as u8,
            b: (hex & 0xFF) as u8,
            a: 255,
        }
    }

    /// Convert to hex value
    pub fn to_hex(&self) -> u32 {
        ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }

    /// Calculate contrast ratio with another color (WCAG formula)
    pub fn contrast_ratio(&self, other: &Color) -> f32 {
        let l1 = self.relative_luminance();
        let l2 = other.relative_luminance();

        let lighter = if l1 > l2 { l1 } else { l2 };
        let darker = if l1 > l2 { l2 } else { l1 };

        (lighter + 0.05) / (darker + 0.05)
    }

    /// Calculate relative luminance (WCAG formula)
    pub fn relative_luminance(&self) -> f32 {
        let r = self.linearize(self.r as f32 / 255.0);
        let g = self.linearize(self.g as f32 / 255.0);
        let b = self.linearize(self.b as f32 / 255.0);

        0.2126 * r + 0.7152 * g + 0.0722 * b
    }

    fn linearize(&self, value: f32) -> f32 {
        if value <= 0.03928 {
            value / 12.92
        } else {
            pow_f32((value + 0.055) / 1.055, 2.4)
        }
    }

    /// Check if this color meets WCAG AA contrast with another color (4.5:1)
    pub fn meets_wcag_aa(&self, other: &Color) -> bool {
        self.contrast_ratio(other) >= 4.5
    }

    /// Check if this color meets WCAG AAA contrast with another color (7:1)
    pub fn meets_wcag_aaa(&self, other: &Color) -> bool {
        self.contrast_ratio(other) >= 7.0
    }

    /// Invert color
    pub fn invert(&self) -> Self {
        Self::new(255 - self.r, 255 - self.g, 255 - self.b, self.a)
    }

    /// Mix with another color
    pub fn mix(&self, other: &Color, ratio: f32) -> Self {
        let r = (self.r as f32 * (1.0 - ratio) + other.r as f32 * ratio) as u8;
        let g = (self.g as f32 * (1.0 - ratio) + other.g as f32 * ratio) as u8;
        let b = (self.b as f32 * (1.0 - ratio) + other.b as f32 * ratio) as u8;
        let a = (self.a as f32 * (1.0 - ratio) + other.a as f32 * ratio) as u8;
        Self::new(r, g, b, a)
    }
}

/// High contrast color palette
#[derive(Debug, Clone)]
pub struct ContrastPalette {
    /// Background color
    pub background: Color,
    /// Primary text color
    pub text: Color,
    /// Secondary/dimmed text color
    pub text_secondary: Color,
    /// Link color
    pub link: Color,
    /// Visited link color
    pub link_visited: Color,
    /// Active/hover link color
    pub link_active: Color,
    /// Button background
    pub button_bg: Color,
    /// Button text
    pub button_text: Color,
    /// Button border
    pub button_border: Color,
    /// Input field background
    pub input_bg: Color,
    /// Input field text
    pub input_text: Color,
    /// Input field border
    pub input_border: Color,
    /// Focus indicator color
    pub focus: Color,
    /// Selection background
    pub selection_bg: Color,
    /// Selection text
    pub selection_text: Color,
    /// Error/alert color
    pub error: Color,
    /// Warning color
    pub warning: Color,
    /// Success color
    pub success: Color,
    /// Disabled element color
    pub disabled: Color,
    /// Border color
    pub border: Color,
    /// Scrollbar background
    pub scrollbar_bg: Color,
    /// Scrollbar thumb
    pub scrollbar_thumb: Color,
}

impl ContrastPalette {
    /// Create palette for Black on White scheme
    pub fn black_on_white() -> Self {
        Self {
            background: Color::white(),
            text: Color::black(),
            text_secondary: Color::rgb(64, 64, 64),
            link: Color::rgb(0, 0, 128),
            link_visited: Color::rgb(128, 0, 128),
            link_active: Color::rgb(0, 0, 255),
            button_bg: Color::white(),
            button_text: Color::black(),
            button_border: Color::black(),
            input_bg: Color::white(),
            input_text: Color::black(),
            input_border: Color::black(),
            focus: Color::rgb(0, 0, 255),
            selection_bg: Color::rgb(0, 0, 128),
            selection_text: Color::white(),
            error: Color::rgb(128, 0, 0),
            warning: Color::rgb(128, 64, 0),
            success: Color::rgb(0, 128, 0),
            disabled: Color::rgb(128, 128, 128),
            border: Color::black(),
            scrollbar_bg: Color::rgb(200, 200, 200),
            scrollbar_thumb: Color::black(),
        }
    }

    /// Create palette for White on Black scheme
    pub fn white_on_black() -> Self {
        Self {
            background: Color::black(),
            text: Color::white(),
            text_secondary: Color::rgb(192, 192, 192),
            link: Color::cyan(),
            link_visited: Color::magenta(),
            link_active: Color::rgb(128, 255, 255),
            button_bg: Color::black(),
            button_text: Color::white(),
            button_border: Color::white(),
            input_bg: Color::black(),
            input_text: Color::white(),
            input_border: Color::white(),
            focus: Color::cyan(),
            selection_bg: Color::white(),
            selection_text: Color::black(),
            error: Color::rgb(255, 128, 128),
            warning: Color::orange(),
            success: Color::rgb(128, 255, 128),
            disabled: Color::rgb(128, 128, 128),
            border: Color::white(),
            scrollbar_bg: Color::rgb(32, 32, 32),
            scrollbar_thumb: Color::white(),
        }
    }

    /// Create palette for Yellow on Black scheme
    pub fn yellow_on_black() -> Self {
        Self {
            background: Color::black(),
            text: Color::yellow(),
            text_secondary: Color::rgb(192, 192, 0),
            link: Color::cyan(),
            link_visited: Color::rgb(255, 128, 255),
            link_active: Color::rgb(128, 255, 255),
            button_bg: Color::black(),
            button_text: Color::yellow(),
            button_border: Color::yellow(),
            input_bg: Color::black(),
            input_text: Color::yellow(),
            input_border: Color::yellow(),
            focus: Color::cyan(),
            selection_bg: Color::yellow(),
            selection_text: Color::black(),
            error: Color::rgb(255, 128, 128),
            warning: Color::orange(),
            success: Color::green(),
            disabled: Color::rgb(128, 128, 0),
            border: Color::yellow(),
            scrollbar_bg: Color::rgb(32, 32, 0),
            scrollbar_thumb: Color::yellow(),
        }
    }

    /// Create palette for Green on Black scheme
    pub fn green_on_black() -> Self {
        Self {
            background: Color::black(),
            text: Color::green(),
            text_secondary: Color::rgb(0, 192, 0),
            link: Color::cyan(),
            link_visited: Color::magenta(),
            link_active: Color::rgb(128, 255, 255),
            button_bg: Color::black(),
            button_text: Color::green(),
            button_border: Color::green(),
            input_bg: Color::black(),
            input_text: Color::green(),
            input_border: Color::green(),
            focus: Color::cyan(),
            selection_bg: Color::green(),
            selection_text: Color::black(),
            error: Color::rgb(255, 128, 128),
            warning: Color::yellow(),
            success: Color::rgb(128, 255, 128),
            disabled: Color::rgb(0, 128, 0),
            border: Color::green(),
            scrollbar_bg: Color::rgb(0, 32, 0),
            scrollbar_thumb: Color::green(),
        }
    }

    /// Get palette for a scheme
    pub fn for_scheme(scheme: ContrastScheme) -> Self {
        match scheme {
            ContrastScheme::BlackOnWhite => Self::black_on_white(),
            ContrastScheme::WhiteOnBlack => Self::white_on_black(),
            ContrastScheme::YellowOnBlack => Self::yellow_on_black(),
            ContrastScheme::GreenOnBlack => Self::green_on_black(),
            ContrastScheme::Custom => Self::white_on_black(), // Default to white on black
        }
    }
}

/// UI element type for applying contrast styles
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementType {
    /// Window background
    Window,
    /// Dialog background
    Dialog,
    /// Panel/container
    Panel,
    /// Button
    Button,
    /// Text input field
    TextInput,
    /// Checkbox
    Checkbox,
    /// Radio button
    RadioButton,
    /// Dropdown/combobox
    Dropdown,
    /// List/listbox
    List,
    /// List item
    ListItem,
    /// Menu
    Menu,
    /// Menu item
    MenuItem,
    /// Tab
    Tab,
    /// Scrollbar
    Scrollbar,
    /// Progress bar
    ProgressBar,
    /// Slider
    Slider,
    /// Tooltip
    Tooltip,
    /// Status bar
    StatusBar,
    /// Toolbar
    Toolbar,
    /// Link
    Link,
    /// Heading
    Heading,
    /// Paragraph text
    Paragraph,
    /// Label
    Label,
    /// Image (border only)
    Image,
    /// Table
    Table,
    /// Table header
    TableHeader,
    /// Table cell
    TableCell,
    /// Tree view
    Tree,
    /// Tree item
    TreeItem,
}

/// Element visual state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementState {
    /// Normal state
    Normal,
    /// Hovered state
    Hovered,
    /// Pressed/active state
    Pressed,
    /// Focused state
    Focused,
    /// Disabled state
    Disabled,
    /// Selected state
    Selected,
    /// Checked state (checkbox/radio)
    Checked,
    /// Error state
    Error,
}

/// Style for a specific element
#[derive(Debug, Clone)]
pub struct ElementStyle {
    /// Background color
    pub background: Color,
    /// Text/foreground color
    pub foreground: Color,
    /// Border color
    pub border: Color,
    /// Border width in pixels
    pub border_width: u8,
    /// Outline color (for focus)
    pub outline: Option<Color>,
    /// Outline width
    pub outline_width: u8,
    /// Outline offset
    pub outline_offset: i8,
}

impl ElementStyle {
    /// Create a new element style
    pub fn new(background: Color, foreground: Color, border: Color) -> Self {
        Self {
            background,
            foreground,
            border,
            border_width: 2,
            outline: None,
            outline_width: 2,
            outline_offset: 2,
        }
    }

    /// Set border width
    pub fn with_border_width(mut self, width: u8) -> Self {
        self.border_width = width;
        self
    }

    /// Set focus outline
    pub fn with_outline(mut self, color: Color) -> Self {
        self.outline = Some(color);
        self
    }
}

/// High contrast configuration
#[derive(Debug, Clone)]
pub struct HighContrastConfig {
    /// Whether high contrast mode is enabled
    pub enabled: bool,
    /// Active color scheme
    pub scheme: ContrastScheme,
    /// Custom palette (used when scheme is Custom)
    pub custom_palette: Option<ContrastPalette>,
    /// Minimum border width for UI elements
    pub min_border_width: u8,
    /// Focus indicator width
    pub focus_width: u8,
    /// Focus indicator offset from element
    pub focus_offset: i8,
    /// Remove background images
    pub remove_backgrounds: bool,
    /// Remove transparency effects
    pub remove_transparency: bool,
    /// Disable animations
    pub disable_animations: bool,
    /// Increase text boldness
    pub bold_text: bool,
    /// Underline all links
    pub underline_links: bool,
    /// Show button borders
    pub show_button_borders: bool,
    /// Apply to system UI only (not apps)
    pub system_ui_only: bool,
}

impl Default for HighContrastConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            scheme: ContrastScheme::WhiteOnBlack,
            custom_palette: None,
            min_border_width: 2,
            focus_width: 3,
            focus_offset: 2,
            remove_backgrounds: true,
            remove_transparency: true,
            disable_animations: true,
            bold_text: false,
            underline_links: true,
            show_button_borders: true,
            system_ui_only: false,
        }
    }
}

/// Statistics for high contrast usage
#[derive(Debug, Clone, Default)]
pub struct HighContrastStats {
    /// Number of times enabled
    pub times_enabled: u64,
    /// Total time enabled (ms)
    pub total_time_enabled_ms: u64,
    /// Number of scheme changes
    pub scheme_changes: u64,
    /// Number of elements styled
    pub elements_styled: u64,
    /// Session start timestamp
    pub session_start_ms: u64,
}

/// High contrast mode manager
pub struct HighContrastManager {
    /// Configuration
    config: HighContrastConfig,
    /// Active palette
    palette: ContrastPalette,
    /// Statistics
    stats: HighContrastStats,
    /// Enable timestamp (for tracking duration)
    enabled_since_ms: Option<u64>,
    /// Callback when mode changes
    on_mode_change: Option<fn(bool)>,
    /// Callback when scheme changes
    on_scheme_change: Option<fn(ContrastScheme)>,
}

impl HighContrastManager {
    /// Create a new high contrast manager
    pub fn new() -> Self {
        let config = HighContrastConfig::default();
        let palette = ContrastPalette::for_scheme(config.scheme);

        Self {
            config,
            palette,
            stats: HighContrastStats::default(),
            enabled_since_ms: None,
            on_mode_change: None,
            on_scheme_change: None,
        }
    }

    /// Initialize the manager
    pub fn init(&mut self) {
        self.stats.session_start_ms = crate::time::uptime_ms();
        crate::kprintln!("[high_contrast] High contrast manager initialized");
    }

    /// Enable high contrast mode
    pub fn enable(&mut self) {
        if !self.config.enabled {
            self.config.enabled = true;
            self.enabled_since_ms = Some(crate::time::uptime_ms());
            self.stats.times_enabled += 1;

            if let Some(callback) = self.on_mode_change {
                callback(true);
            }

            crate::kprintln!("[high_contrast] High contrast mode enabled ({})", self.config.scheme.name());
        }
    }

    /// Disable high contrast mode
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

            crate::kprintln!("[high_contrast] High contrast mode disabled");
        }
    }

    /// Toggle high contrast mode
    pub fn toggle(&mut self) {
        if self.config.enabled {
            self.disable();
        } else {
            self.enable();
        }
    }

    /// Check if high contrast mode is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Set the color scheme
    pub fn set_scheme(&mut self, scheme: ContrastScheme) {
        if self.config.scheme != scheme {
            self.config.scheme = scheme;

            // Update palette
            self.palette = if scheme == ContrastScheme::Custom {
                self.config.custom_palette.clone().unwrap_or_else(ContrastPalette::white_on_black)
            } else {
                ContrastPalette::for_scheme(scheme)
            };

            self.stats.scheme_changes += 1;

            if let Some(callback) = self.on_scheme_change {
                callback(scheme);
            }

            crate::kprintln!("[high_contrast] Scheme changed to: {}", scheme.name());
        }
    }

    /// Get current scheme
    pub fn scheme(&self) -> ContrastScheme {
        self.config.scheme
    }

    /// Set custom palette
    pub fn set_custom_palette(&mut self, palette: ContrastPalette) {
        self.config.custom_palette = Some(palette.clone());

        if self.config.scheme == ContrastScheme::Custom {
            self.palette = palette;
        }
    }

    /// Get current palette
    pub fn palette(&self) -> &ContrastPalette {
        &self.palette
    }

    /// Get style for an element
    pub fn get_style(&mut self, element_type: ElementType, state: ElementState) -> ElementStyle {
        self.stats.elements_styled += 1;

        let palette = &self.palette;

        match element_type {
            ElementType::Window | ElementType::Dialog | ElementType::Panel => {
                ElementStyle::new(
                    palette.background,
                    palette.text,
                    palette.border,
                )
            }

            ElementType::Button => {
                let (bg, fg, border) = match state {
                    ElementState::Normal => (palette.button_bg, palette.button_text, palette.button_border),
                    ElementState::Hovered => (palette.button_border, palette.button_bg, palette.button_border),
                    ElementState::Pressed => (palette.button_border, palette.button_bg, palette.button_border),
                    ElementState::Focused => (palette.button_bg, palette.button_text, palette.focus),
                    ElementState::Disabled => (palette.background, palette.disabled, palette.disabled),
                    _ => (palette.button_bg, palette.button_text, palette.button_border),
                };
                ElementStyle::new(bg, fg, border)
                    .with_border_width(self.config.min_border_width)
                    .with_outline(palette.focus)
            }

            ElementType::TextInput | ElementType::Dropdown => {
                let (bg, fg, border) = match state {
                    ElementState::Normal => (palette.input_bg, palette.input_text, palette.input_border),
                    ElementState::Focused => (palette.input_bg, palette.input_text, palette.focus),
                    ElementState::Error => (palette.input_bg, palette.input_text, palette.error),
                    ElementState::Disabled => (palette.background, palette.disabled, palette.disabled),
                    _ => (palette.input_bg, palette.input_text, palette.input_border),
                };
                ElementStyle::new(bg, fg, border)
                    .with_border_width(self.config.min_border_width)
            }

            ElementType::Checkbox | ElementType::RadioButton => {
                let (bg, fg, border) = match state {
                    ElementState::Checked => (palette.text, palette.background, palette.text),
                    ElementState::Disabled => (palette.background, palette.disabled, palette.disabled),
                    ElementState::Focused => (palette.background, palette.text, palette.focus),
                    _ => (palette.background, palette.text, palette.text),
                };
                ElementStyle::new(bg, fg, border)
                    .with_border_width(self.config.min_border_width)
            }

            ElementType::Link => {
                let fg = match state {
                    ElementState::Normal => palette.link,
                    ElementState::Hovered => palette.link_active,
                    ElementState::Pressed => palette.link_active,
                    ElementState::Selected => palette.link_visited,
                    ElementState::Disabled => palette.disabled,
                    _ => palette.link,
                };
                ElementStyle::new(palette.background, fg, Color::new(0, 0, 0, 0))
            }

            ElementType::List | ElementType::Tree | ElementType::Table => {
                ElementStyle::new(
                    palette.background,
                    palette.text,
                    palette.border,
                ).with_border_width(self.config.min_border_width)
            }

            ElementType::ListItem | ElementType::TreeItem | ElementType::TableCell => {
                let (bg, fg) = match state {
                    ElementState::Selected => (palette.selection_bg, palette.selection_text),
                    ElementState::Hovered => (palette.text.mix(&palette.background, 0.8), palette.text),
                    ElementState::Disabled => (palette.background, palette.disabled),
                    _ => (palette.background, palette.text),
                };
                ElementStyle::new(bg, fg, Color::new(0, 0, 0, 0))
            }

            ElementType::Menu | ElementType::Toolbar => {
                ElementStyle::new(
                    palette.background,
                    palette.text,
                    palette.border,
                ).with_border_width(self.config.min_border_width)
            }

            ElementType::MenuItem => {
                let (bg, fg) = match state {
                    ElementState::Hovered | ElementState::Selected => {
                        (palette.selection_bg, palette.selection_text)
                    }
                    ElementState::Disabled => (palette.background, palette.disabled),
                    _ => (palette.background, palette.text),
                };
                ElementStyle::new(bg, fg, Color::new(0, 0, 0, 0))
            }

            ElementType::Tab => {
                let (bg, fg, border) = match state {
                    ElementState::Selected => (palette.background, palette.text, palette.text),
                    ElementState::Hovered => (palette.text.mix(&palette.background, 0.9), palette.text, palette.border),
                    _ => (palette.text.mix(&palette.background, 0.95), palette.text_secondary, palette.border),
                };
                ElementStyle::new(bg, fg, border)
            }

            ElementType::Scrollbar => {
                ElementStyle::new(
                    palette.scrollbar_bg,
                    palette.scrollbar_thumb,
                    palette.border,
                )
            }

            ElementType::ProgressBar | ElementType::Slider => {
                ElementStyle::new(
                    palette.background,
                    palette.text,
                    palette.border,
                ).with_border_width(self.config.min_border_width)
            }

            ElementType::Tooltip => {
                ElementStyle::new(
                    palette.text,
                    palette.background,
                    palette.text,
                ).with_border_width(1)
            }

            ElementType::StatusBar => {
                ElementStyle::new(
                    palette.background,
                    palette.text,
                    palette.border,
                ).with_border_width(1)
            }

            ElementType::Heading | ElementType::Paragraph | ElementType::Label => {
                ElementStyle::new(
                    Color::new(0, 0, 0, 0), // Transparent background
                    palette.text,
                    Color::new(0, 0, 0, 0),
                )
            }

            ElementType::Image => {
                // Images get a border in high contrast mode
                ElementStyle::new(
                    Color::new(0, 0, 0, 0),
                    palette.text,
                    palette.border,
                ).with_border_width(1)
            }

            ElementType::TableHeader => {
                ElementStyle::new(
                    palette.text.mix(&palette.background, 0.9),
                    palette.text,
                    palette.border,
                ).with_border_width(1)
            }
        }
    }

    /// Get focus indicator style
    pub fn get_focus_style(&self) -> ElementStyle {
        ElementStyle {
            background: Color::new(0, 0, 0, 0),
            foreground: self.palette.focus,
            border: self.palette.focus,
            border_width: self.config.focus_width,
            outline: Some(self.palette.focus),
            outline_width: self.config.focus_width,
            outline_offset: self.config.focus_offset,
        }
    }

    /// Check if backgrounds should be removed
    pub fn should_remove_backgrounds(&self) -> bool {
        self.config.enabled && self.config.remove_backgrounds
    }

    /// Check if transparency should be removed
    pub fn should_remove_transparency(&self) -> bool {
        self.config.enabled && self.config.remove_transparency
    }

    /// Check if animations should be disabled
    pub fn should_disable_animations(&self) -> bool {
        self.config.enabled && self.config.disable_animations
    }

    /// Check if text should be bold
    pub fn should_use_bold_text(&self) -> bool {
        self.config.enabled && self.config.bold_text
    }

    /// Check if links should be underlined
    pub fn should_underline_links(&self) -> bool {
        self.config.enabled && self.config.underline_links
    }

    /// Get configuration
    pub fn config(&self) -> &HighContrastConfig {
        &self.config
    }

    /// Set configuration
    pub fn set_config(&mut self, config: HighContrastConfig) {
        let was_enabled = self.config.enabled;
        let old_scheme = self.config.scheme;

        self.config = config;

        // Update palette if scheme changed
        if self.config.scheme != old_scheme {
            self.set_scheme(self.config.scheme);
        }

        // Track enable/disable
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

    /// Set scheme change callback
    pub fn set_scheme_change_callback(&mut self, callback: fn(ContrastScheme)) {
        self.on_scheme_change = Some(callback);
    }

    /// Get statistics
    pub fn stats(&self) -> HighContrastStats {
        let mut stats = self.stats.clone();

        // Add current session duration if enabled
        if let Some(start) = self.enabled_since_ms {
            let now = crate::time::uptime_ms();
            stats.total_time_enabled_ms += now - start;
        }

        stats
    }

    /// Get list of available schemes
    pub fn available_schemes() -> Vec<ContrastScheme> {
        alloc::vec![
            ContrastScheme::WhiteOnBlack,
            ContrastScheme::BlackOnWhite,
            ContrastScheme::YellowOnBlack,
            ContrastScheme::GreenOnBlack,
            ContrastScheme::Custom,
        ]
    }

    /// Format status string
    pub fn format_status(&self) -> String {
        let stats = self.stats();

        format!(
            "High Contrast: {}\n\
             Scheme: {}\n\
             Times enabled: {}\n\
             Total time enabled: {} ms\n\
             Elements styled: {}",
            if self.config.enabled { "Enabled" } else { "Disabled" },
            self.config.scheme.name(),
            stats.times_enabled,
            stats.total_time_enabled_ms,
            stats.elements_styled
        )
    }
}

/// Software pow implementation for no_std
fn pow_f32(base: f32, exp: f32) -> f32 {
    // Use exp(exp * ln(base)) approximation
    if base <= 0.0 {
        return 0.0;
    }
    exp_f32(exp * ln_f32(base))
}

/// Software ln implementation for no_std
fn ln_f32(x: f32) -> f32 {
    if x <= 0.0 {
        return f32::NEG_INFINITY;
    }

    // Use integer log2 plus polynomial approximation
    let bits = x.to_bits();
    let exp = ((bits >> 23) & 0xFF) as i32 - 127;
    let mantissa_bits = (bits & 0x7FFFFF) | 0x3F800000;
    let m = f32::from_bits(mantissa_bits);

    // ln(x) = ln(2^exp * m) = exp * ln(2) + ln(m)
    let ln2 = 0.693147;
    let t = m - 1.0;

    // Taylor series for ln(1+t)
    let ln_m = t - 0.5 * t * t + t * t * t / 3.0 - t * t * t * t / 4.0;

    exp as f32 * ln2 + ln_m
}

/// Software floor implementation for no_std
fn floor_f32(x: f32) -> f32 {
    let xi = x as i32;
    if x < 0.0 && x != xi as f32 {
        (xi - 1) as f32
    } else {
        xi as f32
    }
}

/// Software exp implementation for no_std
fn exp_f32(x: f32) -> f32 {
    // Clamp to avoid overflow
    let x = if x > 88.0 { 88.0 } else if x < -88.0 { -88.0 } else { x };

    // Use exp(x) = 2^(x/ln2)
    let ln2 = 0.693147;
    let y = x / ln2;
    let k = floor_f32(y) as i32;
    let f = y - k as f32;

    // 2^f using polynomial approximation
    let two_f = 1.0 + f * (0.693147 + f * (0.240226 + f * (0.055504 + f * 0.009618)));

    // 2^k * 2^f
    if k >= 0 && k < 128 {
        two_f * (1u32 << k as u32) as f32
    } else if k < 0 && k > -128 {
        two_f / (1u32 << (-k) as u32) as f32
    } else {
        0.0
    }
}

/// Global high contrast manager instance
static HIGH_CONTRAST_MANAGER: IrqSafeMutex<Option<HighContrastManager>> = IrqSafeMutex::new(None);

/// Initialize high contrast mode
pub fn init() {
    let mut manager = HighContrastManager::new();
    manager.init();
    *HIGH_CONTRAST_MANAGER.lock() = Some(manager);
}

/// Enable high contrast mode
pub fn enable() {
    if let Some(ref mut manager) = *HIGH_CONTRAST_MANAGER.lock() {
        manager.enable();
    }
}

/// Disable high contrast mode
pub fn disable() {
    if let Some(ref mut manager) = *HIGH_CONTRAST_MANAGER.lock() {
        manager.disable();
    }
}

/// Toggle high contrast mode
pub fn toggle() {
    if let Some(ref mut manager) = *HIGH_CONTRAST_MANAGER.lock() {
        manager.toggle();
    }
}

/// Check if high contrast mode is enabled
pub fn is_enabled() -> bool {
    HIGH_CONTRAST_MANAGER.lock().as_ref().map(|m| m.is_enabled()).unwrap_or(false)
}

/// Set color scheme
pub fn set_scheme(scheme: ContrastScheme) {
    if let Some(ref mut manager) = *HIGH_CONTRAST_MANAGER.lock() {
        manager.set_scheme(scheme);
    }
}

/// Get current scheme
pub fn get_scheme() -> ContrastScheme {
    HIGH_CONTRAST_MANAGER.lock().as_ref().map(|m| m.scheme()).unwrap_or(ContrastScheme::WhiteOnBlack)
}

/// Get style for an element
pub fn get_style(element_type: ElementType, state: ElementState) -> Option<ElementStyle> {
    HIGH_CONTRAST_MANAGER.lock().as_mut().map(|m| m.get_style(element_type, state))
}

/// Get current palette
pub fn get_palette() -> Option<ContrastPalette> {
    HIGH_CONTRAST_MANAGER.lock().as_ref().map(|m| m.palette().clone())
}

/// Get statistics
pub fn stats() -> Option<HighContrastStats> {
    HIGH_CONTRAST_MANAGER.lock().as_ref().map(|m| m.stats())
}

/// Get status string
pub fn status() -> String {
    HIGH_CONTRAST_MANAGER.lock().as_ref()
        .map(|m| m.format_status())
        .unwrap_or_else(|| String::from("High Contrast: Not initialized"))
}
