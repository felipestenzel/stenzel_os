//! Theme Engine
//!
//! Provides GTK-like theming system with dark mode, accent colors, and custom themes.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use spin::Mutex;

/// Global theme state
static THEME_STATE: Mutex<Option<ThemeState>> = Mutex::new(None);

/// Theme state
pub struct ThemeState {
    /// Current theme
    pub current_theme: Theme,
    /// Available themes
    pub themes: BTreeMap<String, Theme>,
    /// Color scheme (light/dark)
    pub color_scheme: ColorScheme,
    /// Accent color
    pub accent_color: AccentColor,
    /// Custom colors override
    pub custom_colors: Option<ThemeColors>,
    /// Font settings
    pub fonts: FontSettings,
    /// Animation settings
    pub animations: AnimationSettings,
    /// Callbacks for theme changes
    pub change_callbacks: Vec<ThemeChangeCallback>,
}

/// Color scheme
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorScheme {
    /// Light mode
    Light,
    /// Dark mode
    Dark,
    /// Follow system preference
    Auto,
}

/// Accent color
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccentColor {
    Blue,
    Purple,
    Pink,
    Red,
    Orange,
    Yellow,
    Green,
    Teal,
    Graphite,
    Custom(u32), // RGB value
}

impl AccentColor {
    /// Get RGB value for this accent color
    pub fn to_rgb(&self) -> (u8, u8, u8) {
        match self {
            AccentColor::Blue => (0, 122, 255),
            AccentColor::Purple => (175, 82, 222),
            AccentColor::Pink => (255, 45, 85),
            AccentColor::Red => (255, 59, 48),
            AccentColor::Orange => (255, 149, 0),
            AccentColor::Yellow => (255, 204, 0),
            AccentColor::Green => (52, 199, 89),
            AccentColor::Teal => (90, 200, 250),
            AccentColor::Graphite => (142, 142, 147),
            AccentColor::Custom(rgb) => {
                ((*rgb >> 16) as u8, (*rgb >> 8) as u8, *rgb as u8)
            }
        }
    }

    /// Get hex value for this accent color
    pub fn to_hex(&self) -> u32 {
        let (r, g, b) = self.to_rgb();
        ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
    }
}

/// Theme definition
#[derive(Debug, Clone)]
pub struct Theme {
    /// Theme name
    pub name: String,
    /// Theme display name
    pub display_name: String,
    /// Theme author
    pub author: String,
    /// Theme version
    pub version: String,
    /// Light mode colors
    pub light_colors: ThemeColors,
    /// Dark mode colors
    pub dark_colors: ThemeColors,
    /// Widget styles
    pub widget_styles: WidgetStyles,
    /// Window decorations
    pub decorations: DecorationStyle,
    /// Is system theme
    pub is_system: bool,
}

/// Theme colors
#[derive(Debug, Clone)]
pub struct ThemeColors {
    /// Background colors
    pub background: BackgroundColors,
    /// Foreground/text colors
    pub foreground: ForegroundColors,
    /// Border colors
    pub border: BorderColors,
    /// State colors (hover, active, etc.)
    pub state: StateColors,
    /// Semantic colors (success, warning, error)
    pub semantic: SemanticColors,
}

/// Background colors
#[derive(Debug, Clone)]
pub struct BackgroundColors {
    /// Primary background
    pub primary: u32,
    /// Secondary background
    pub secondary: u32,
    /// Tertiary background
    pub tertiary: u32,
    /// Window background
    pub window: u32,
    /// Header/toolbar background
    pub header: u32,
    /// Sidebar background
    pub sidebar: u32,
    /// Card/elevated surface background
    pub card: u32,
    /// Popup/modal background
    pub popup: u32,
    /// Selected item background
    pub selected: u32,
}

/// Foreground/text colors
#[derive(Debug, Clone)]
pub struct ForegroundColors {
    /// Primary text
    pub primary: u32,
    /// Secondary text
    pub secondary: u32,
    /// Tertiary/disabled text
    pub tertiary: u32,
    /// Inverted text (for dark backgrounds)
    pub inverted: u32,
    /// Link text
    pub link: u32,
    /// Placeholder text
    pub placeholder: u32,
}

/// Border colors
#[derive(Debug, Clone)]
pub struct BorderColors {
    /// Default border
    pub default: u32,
    /// Strong border
    pub strong: u32,
    /// Subtle border
    pub subtle: u32,
    /// Focus border
    pub focus: u32,
    /// Separator
    pub separator: u32,
}

/// State colors
#[derive(Debug, Clone)]
pub struct StateColors {
    /// Hover state
    pub hover: u32,
    /// Active/pressed state
    pub active: u32,
    /// Focused state
    pub focused: u32,
    /// Disabled state
    pub disabled: u32,
    /// Selected state
    pub selected: u32,
}

/// Semantic colors
#[derive(Debug, Clone)]
pub struct SemanticColors {
    /// Success color
    pub success: u32,
    /// Warning color
    pub warning: u32,
    /// Error color
    pub error: u32,
    /// Info color
    pub info: u32,
}

/// Widget styles
#[derive(Debug, Clone)]
pub struct WidgetStyles {
    /// Button style
    pub button: ButtonStyle,
    /// Input field style
    pub input: InputStyle,
    /// Switch/toggle style
    pub switch: SwitchStyle,
    /// Checkbox style
    pub checkbox: CheckboxStyle,
    /// Radio button style
    pub radio: RadioStyle,
    /// Slider style
    pub slider: SliderStyle,
    /// Progress bar style
    pub progress: ProgressStyle,
    /// Tab style
    pub tab: TabStyle,
    /// Menu style
    pub menu: MenuStyle,
    /// Tooltip style
    pub tooltip: TooltipStyle,
    /// Scrollbar style
    pub scrollbar: ScrollbarStyle,
}

/// Button style
#[derive(Debug, Clone)]
pub struct ButtonStyle {
    /// Border radius
    pub border_radius: u32,
    /// Padding (top, right, bottom, left)
    pub padding: (u32, u32, u32, u32),
    /// Font weight
    pub font_weight: FontWeight,
    /// Shadow
    pub shadow: Option<Shadow>,
}

/// Input field style
#[derive(Debug, Clone)]
pub struct InputStyle {
    /// Border radius
    pub border_radius: u32,
    /// Border width
    pub border_width: u32,
    /// Padding
    pub padding: (u32, u32, u32, u32),
    /// Focus ring width
    pub focus_ring_width: u32,
}

/// Switch/toggle style
#[derive(Debug, Clone)]
pub struct SwitchStyle {
    /// Track width
    pub track_width: u32,
    /// Track height
    pub track_height: u32,
    /// Thumb diameter
    pub thumb_diameter: u32,
    /// Border radius (0 = rectangular)
    pub border_radius: u32,
}

/// Checkbox style
#[derive(Debug, Clone)]
pub struct CheckboxStyle {
    /// Size
    pub size: u32,
    /// Border radius
    pub border_radius: u32,
    /// Border width
    pub border_width: u32,
    /// Checkmark style
    pub checkmark: CheckmarkStyle,
}

/// Checkmark style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckmarkStyle {
    Check,
    Cross,
    Fill,
}

/// Radio button style
#[derive(Debug, Clone)]
pub struct RadioStyle {
    /// Outer diameter
    pub outer_diameter: u32,
    /// Inner dot diameter
    pub inner_diameter: u32,
    /// Border width
    pub border_width: u32,
}

/// Slider style
#[derive(Debug, Clone)]
pub struct SliderStyle {
    /// Track height
    pub track_height: u32,
    /// Thumb diameter
    pub thumb_diameter: u32,
    /// Track border radius
    pub track_radius: u32,
}

/// Progress bar style
#[derive(Debug, Clone)]
pub struct ProgressStyle {
    /// Height
    pub height: u32,
    /// Border radius
    pub border_radius: u32,
    /// Striped animation
    pub striped: bool,
}

/// Tab style
#[derive(Debug, Clone)]
pub struct TabStyle {
    /// Indicator style
    pub indicator: TabIndicatorStyle,
    /// Padding
    pub padding: (u32, u32, u32, u32),
    /// Gap between tabs
    pub gap: u32,
}

/// Tab indicator style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabIndicatorStyle {
    Underline,
    Background,
    Pill,
}

/// Menu style
#[derive(Debug, Clone)]
pub struct MenuStyle {
    /// Border radius
    pub border_radius: u32,
    /// Padding
    pub padding: (u32, u32, u32, u32),
    /// Item height
    pub item_height: u32,
    /// Separator height
    pub separator_height: u32,
    /// Shadow
    pub shadow: Option<Shadow>,
}

/// Tooltip style
#[derive(Debug, Clone)]
pub struct TooltipStyle {
    /// Border radius
    pub border_radius: u32,
    /// Padding
    pub padding: (u32, u32, u32, u32),
    /// Arrow size
    pub arrow_size: u32,
    /// Delay before showing (ms)
    pub delay_ms: u32,
}

/// Scrollbar style
#[derive(Debug, Clone)]
pub struct ScrollbarStyle {
    /// Width/thickness
    pub width: u32,
    /// Border radius
    pub border_radius: u32,
    /// Show on hover only
    pub overlay: bool,
    /// Minimum thumb length
    pub min_thumb_length: u32,
}

/// Shadow definition
#[derive(Debug, Clone)]
pub struct Shadow {
    /// X offset
    pub x: i32,
    /// Y offset
    pub y: i32,
    /// Blur radius
    pub blur: u32,
    /// Spread radius
    pub spread: i32,
    /// Color (RGBA)
    pub color: u32,
}

/// Window decoration style
#[derive(Debug, Clone)]
pub struct DecorationStyle {
    /// Title bar height
    pub titlebar_height: u32,
    /// Border radius
    pub border_radius: u32,
    /// Border width
    pub border_width: u32,
    /// Button style
    pub button_style: WindowButtonStyle,
    /// Button size
    pub button_size: u32,
    /// Button spacing
    pub button_spacing: u32,
    /// Title alignment
    pub title_alignment: TitleAlignment,
    /// Shadow
    pub shadow: Option<Shadow>,
}

/// Window button style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowButtonStyle {
    /// macOS style (colored circles)
    MacOS,
    /// Windows style (icons)
    Windows,
    /// Linux/GNOME style (symbolic icons)
    Gnome,
    /// Custom
    Custom,
}

/// Title alignment
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleAlignment {
    Left,
    Center,
    Right,
}

/// Font weight
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWeight {
    Thin,
    Light,
    Regular,
    Medium,
    SemiBold,
    Bold,
    ExtraBold,
    Black,
}

impl FontWeight {
    /// Get numeric weight value
    pub fn to_weight(&self) -> u32 {
        match self {
            FontWeight::Thin => 100,
            FontWeight::Light => 300,
            FontWeight::Regular => 400,
            FontWeight::Medium => 500,
            FontWeight::SemiBold => 600,
            FontWeight::Bold => 700,
            FontWeight::ExtraBold => 800,
            FontWeight::Black => 900,
        }
    }
}

/// Font settings
#[derive(Debug, Clone)]
pub struct FontSettings {
    /// System font family
    pub system_font: String,
    /// Monospace font family
    pub monospace_font: String,
    /// Default font size
    pub default_size: u32,
    /// Small font size
    pub small_size: u32,
    /// Large font size
    pub large_size: u32,
    /// Title font size
    pub title_size: u32,
    /// Line height multiplier (e.g., 1.5)
    pub line_height: f32,
    /// Letter spacing
    pub letter_spacing: f32,
    /// Enable font smoothing
    pub smoothing: FontSmoothing,
    /// Hinting mode
    pub hinting: FontHinting,
}

/// Font smoothing mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontSmoothing {
    None,
    Grayscale,
    Subpixel,
    Auto,
}

/// Font hinting mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontHinting {
    None,
    Slight,
    Medium,
    Full,
    Auto,
}

/// Animation settings
#[derive(Debug, Clone)]
pub struct AnimationSettings {
    /// Enable animations
    pub enabled: bool,
    /// Reduce motion (accessibility)
    pub reduce_motion: bool,
    /// Default duration (ms)
    pub default_duration_ms: u32,
    /// Default easing
    pub default_easing: EasingFunction,
    /// Transition duration (ms)
    pub transition_duration_ms: u32,
}

/// Easing function
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EasingFunction {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    Spring,
}

/// Theme change callback
pub type ThemeChangeCallback = fn(&Theme, ColorScheme, AccentColor);

/// Initialize theme engine
pub fn init() {
    let mut state = THEME_STATE.lock();
    if state.is_some() {
        return;
    }

    let default_theme = create_default_theme();
    let mut themes = BTreeMap::new();
    themes.insert("default".to_string(), default_theme.clone());
    themes.insert("high-contrast".to_string(), create_high_contrast_theme());

    *state = Some(ThemeState {
        current_theme: default_theme,
        themes,
        color_scheme: ColorScheme::Auto,
        accent_color: AccentColor::Blue,
        custom_colors: None,
        fonts: FontSettings {
            system_font: "Inter".to_string(),
            monospace_font: "JetBrains Mono".to_string(),
            default_size: 14,
            small_size: 12,
            large_size: 16,
            title_size: 24,
            line_height: 1.5,
            letter_spacing: 0.0,
            smoothing: FontSmoothing::Subpixel,
            hinting: FontHinting::Slight,
        },
        animations: AnimationSettings {
            enabled: true,
            reduce_motion: false,
            default_duration_ms: 200,
            default_easing: EasingFunction::EaseOut,
            transition_duration_ms: 150,
        },
        change_callbacks: Vec::new(),
    });

    crate::kprintln!("theme: initialized with default theme");
}

/// Create default Stenzel theme
fn create_default_theme() -> Theme {
    Theme {
        name: "default".to_string(),
        display_name: "Stenzel".to_string(),
        author: "Stenzel OS".to_string(),
        version: "1.0".to_string(),
        light_colors: ThemeColors {
            background: BackgroundColors {
                primary: 0xFFFFFF,
                secondary: 0xF5F5F5,
                tertiary: 0xE8E8E8,
                window: 0xFFFFFF,
                header: 0xF0F0F0,
                sidebar: 0xF8F8F8,
                card: 0xFFFFFF,
                popup: 0xFFFFFF,
                selected: 0xE0E8FF,
            },
            foreground: ForegroundColors {
                primary: 0x1A1A1A,
                secondary: 0x666666,
                tertiary: 0x999999,
                inverted: 0xFFFFFF,
                link: 0x007AFF,
                placeholder: 0xAAAAAA,
            },
            border: BorderColors {
                default: 0xD0D0D0,
                strong: 0xB0B0B0,
                subtle: 0xE0E0E0,
                focus: 0x007AFF,
                separator: 0xE8E8E8,
            },
            state: StateColors {
                hover: 0xF0F0F0,
                active: 0xE0E0E0,
                focused: 0xE0E8FF,
                disabled: 0xF5F5F5,
                selected: 0xE0E8FF,
            },
            semantic: SemanticColors {
                success: 0x34C759,
                warning: 0xFF9500,
                error: 0xFF3B30,
                info: 0x007AFF,
            },
        },
        dark_colors: ThemeColors {
            background: BackgroundColors {
                primary: 0x1E1E1E,
                secondary: 0x2D2D2D,
                tertiary: 0x3D3D3D,
                window: 0x1E1E1E,
                header: 0x2A2A2A,
                sidebar: 0x252525,
                card: 0x2D2D2D,
                popup: 0x3D3D3D,
                selected: 0x3D4F6F,
            },
            foreground: ForegroundColors {
                primary: 0xF0F0F0,
                secondary: 0xA0A0A0,
                tertiary: 0x707070,
                inverted: 0x1A1A1A,
                link: 0x5AC8FA,
                placeholder: 0x606060,
            },
            border: BorderColors {
                default: 0x404040,
                strong: 0x505050,
                subtle: 0x353535,
                focus: 0x5AC8FA,
                separator: 0x3D3D3D,
            },
            state: StateColors {
                hover: 0x3D3D3D,
                active: 0x4D4D4D,
                focused: 0x3D4F6F,
                disabled: 0x2D2D2D,
                selected: 0x3D4F6F,
            },
            semantic: SemanticColors {
                success: 0x32D74B,
                warning: 0xFF9F0A,
                error: 0xFF453A,
                info: 0x5AC8FA,
            },
        },
        widget_styles: create_default_widget_styles(),
        decorations: DecorationStyle {
            titlebar_height: 32,
            border_radius: 10,
            border_width: 1,
            button_style: WindowButtonStyle::MacOS,
            button_size: 12,
            button_spacing: 8,
            title_alignment: TitleAlignment::Center,
            shadow: Some(Shadow {
                x: 0,
                y: 4,
                blur: 20,
                spread: 0,
                color: 0x40000000,
            }),
        },
        is_system: true,
    }
}

/// Create high contrast theme
fn create_high_contrast_theme() -> Theme {
    let mut theme = create_default_theme();
    theme.name = "high-contrast".to_string();
    theme.display_name = "High Contrast".to_string();

    // High contrast light
    theme.light_colors.background.primary = 0xFFFFFF;
    theme.light_colors.foreground.primary = 0x000000;
    theme.light_colors.border.default = 0x000000;
    theme.light_colors.border.strong = 0x000000;

    // High contrast dark
    theme.dark_colors.background.primary = 0x000000;
    theme.dark_colors.foreground.primary = 0xFFFFFF;
    theme.dark_colors.border.default = 0xFFFFFF;
    theme.dark_colors.border.strong = 0xFFFFFF;

    theme
}

/// Create default widget styles
fn create_default_widget_styles() -> WidgetStyles {
    WidgetStyles {
        button: ButtonStyle {
            border_radius: 8,
            padding: (8, 16, 8, 16),
            font_weight: FontWeight::Medium,
            shadow: None,
        },
        input: InputStyle {
            border_radius: 6,
            border_width: 1,
            padding: (8, 12, 8, 12),
            focus_ring_width: 2,
        },
        switch: SwitchStyle {
            track_width: 44,
            track_height: 24,
            thumb_diameter: 20,
            border_radius: 12,
        },
        checkbox: CheckboxStyle {
            size: 18,
            border_radius: 4,
            border_width: 2,
            checkmark: CheckmarkStyle::Check,
        },
        radio: RadioStyle {
            outer_diameter: 18,
            inner_diameter: 10,
            border_width: 2,
        },
        slider: SliderStyle {
            track_height: 4,
            thumb_diameter: 16,
            track_radius: 2,
        },
        progress: ProgressStyle {
            height: 4,
            border_radius: 2,
            striped: false,
        },
        tab: TabStyle {
            indicator: TabIndicatorStyle::Underline,
            padding: (8, 16, 8, 16),
            gap: 0,
        },
        menu: MenuStyle {
            border_radius: 8,
            padding: (4, 0, 4, 0),
            item_height: 32,
            separator_height: 1,
            shadow: Some(Shadow {
                x: 0,
                y: 2,
                blur: 8,
                spread: 0,
                color: 0x30000000,
            }),
        },
        tooltip: TooltipStyle {
            border_radius: 4,
            padding: (4, 8, 4, 8),
            arrow_size: 6,
            delay_ms: 500,
        },
        scrollbar: ScrollbarStyle {
            width: 8,
            border_radius: 4,
            overlay: true,
            min_thumb_length: 40,
        },
    }
}

/// Set color scheme
pub fn set_color_scheme(scheme: ColorScheme) {
    let mut state = THEME_STATE.lock();
    if let Some(ref mut s) = *state {
        let old_scheme = s.color_scheme;
        s.color_scheme = scheme;

        if old_scheme != scheme {
            notify_theme_change(s);
        }

        crate::kprintln!("theme: color scheme set to {:?}", scheme);
    }
}

/// Get current color scheme
pub fn get_color_scheme() -> ColorScheme {
    let state = THEME_STATE.lock();
    state.as_ref().map(|s| s.color_scheme).unwrap_or(ColorScheme::Auto)
}

/// Check if dark mode is active
pub fn is_dark_mode() -> bool {
    let state = THEME_STATE.lock();
    if let Some(ref s) = *state {
        match s.color_scheme {
            ColorScheme::Dark => true,
            ColorScheme::Light => false,
            ColorScheme::Auto => {
                // In auto mode, check time of day (dark from 6pm to 6am)
                // For now, default to light
                false
            }
        }
    } else {
        false
    }
}

/// Set accent color
pub fn set_accent_color(color: AccentColor) {
    let mut state = THEME_STATE.lock();
    if let Some(ref mut s) = *state {
        s.accent_color = color;
        notify_theme_change(s);
        crate::kprintln!("theme: accent color set to {:?}", color);
    }
}

/// Get current accent color
pub fn get_accent_color() -> AccentColor {
    let state = THEME_STATE.lock();
    state.as_ref().map(|s| s.accent_color).unwrap_or(AccentColor::Blue)
}

/// Get current colors based on color scheme
pub fn get_current_colors() -> Option<ThemeColors> {
    let state = THEME_STATE.lock();
    if let Some(ref s) = *state {
        // Check for custom colors override first
        if let Some(ref custom) = s.custom_colors {
            return Some(custom.clone());
        }

        // Return appropriate colors based on scheme
        if is_dark_mode_internal(s) {
            Some(s.current_theme.dark_colors.clone())
        } else {
            Some(s.current_theme.light_colors.clone())
        }
    } else {
        None
    }
}

/// Internal dark mode check
fn is_dark_mode_internal(s: &ThemeState) -> bool {
    match s.color_scheme {
        ColorScheme::Dark => true,
        ColorScheme::Light => false,
        ColorScheme::Auto => false, // Default to light in auto mode
    }
}

/// Set theme by name
pub fn set_theme(name: &str) -> Result<(), ThemeError> {
    let mut state = THEME_STATE.lock();
    if let Some(ref mut s) = *state {
        if let Some(theme) = s.themes.get(name) {
            s.current_theme = theme.clone();
            notify_theme_change(s);
            crate::kprintln!("theme: switched to '{}'", name);
            Ok(())
        } else {
            Err(ThemeError::ThemeNotFound)
        }
    } else {
        Err(ThemeError::NotInitialized)
    }
}

/// Get current theme
pub fn get_current_theme() -> Option<Theme> {
    let state = THEME_STATE.lock();
    state.as_ref().map(|s| s.current_theme.clone())
}

/// List available themes
pub fn list_themes() -> Vec<String> {
    let state = THEME_STATE.lock();
    state.as_ref().map(|s| s.themes.keys().cloned().collect()).unwrap_or_default()
}

/// Register a theme
pub fn register_theme(theme: Theme) -> Result<(), ThemeError> {
    let mut state = THEME_STATE.lock();
    if let Some(ref mut s) = *state {
        s.themes.insert(theme.name.clone(), theme);
        Ok(())
    } else {
        Err(ThemeError::NotInitialized)
    }
}

/// Unregister a theme
pub fn unregister_theme(name: &str) -> Result<(), ThemeError> {
    let mut state = THEME_STATE.lock();
    if let Some(ref mut s) = *state {
        if name == "default" {
            return Err(ThemeError::CannotRemoveDefault);
        }
        s.themes.remove(name);
        Ok(())
    } else {
        Err(ThemeError::NotInitialized)
    }
}

/// Register theme change callback
pub fn on_theme_change(callback: ThemeChangeCallback) {
    let mut state = THEME_STATE.lock();
    if let Some(ref mut s) = *state {
        s.change_callbacks.push(callback);
    }
}

/// Notify theme change
fn notify_theme_change(s: &ThemeState) {
    for callback in &s.change_callbacks {
        callback(&s.current_theme, s.color_scheme, s.accent_color);
    }
}

/// Set animation settings
pub fn set_animations_enabled(enabled: bool) {
    let mut state = THEME_STATE.lock();
    if let Some(ref mut s) = *state {
        s.animations.enabled = enabled;
    }
}

/// Set reduce motion (accessibility)
pub fn set_reduce_motion(reduce: bool) {
    let mut state = THEME_STATE.lock();
    if let Some(ref mut s) = *state {
        s.animations.reduce_motion = reduce;
        if reduce {
            s.animations.enabled = false;
        }
    }
}

/// Get animation settings
pub fn get_animation_settings() -> Option<AnimationSettings> {
    let state = THEME_STATE.lock();
    state.as_ref().map(|s| s.animations.clone())
}

/// Get font settings
pub fn get_font_settings() -> Option<FontSettings> {
    let state = THEME_STATE.lock();
    state.as_ref().map(|s| s.fonts.clone())
}

/// Set font size
pub fn set_font_size(size: u32) {
    let mut state = THEME_STATE.lock();
    if let Some(ref mut s) = *state {
        s.fonts.default_size = size;
        s.fonts.small_size = (size as f32 * 0.85) as u32;
        s.fonts.large_size = (size as f32 * 1.15) as u32;
        s.fonts.title_size = (size as f32 * 1.7) as u32;
    }
}

/// Set font smoothing
pub fn set_font_smoothing(smoothing: FontSmoothing) {
    let mut state = THEME_STATE.lock();
    if let Some(ref mut s) = *state {
        s.fonts.smoothing = smoothing;
    }
}

/// Get widget styles
pub fn get_widget_styles() -> Option<WidgetStyles> {
    let state = THEME_STATE.lock();
    state.as_ref().map(|s| s.current_theme.widget_styles.clone())
}

/// Get window decoration style
pub fn get_decoration_style() -> Option<DecorationStyle> {
    let state = THEME_STATE.lock();
    state.as_ref().map(|s| s.current_theme.decorations.clone())
}

/// Theme error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeError {
    NotInitialized,
    ThemeNotFound,
    CannotRemoveDefault,
    InvalidTheme,
}

/// Color utilities
pub mod color_utils {
    /// Convert RGB to HSL
    pub fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
        let r = r as f32 / 255.0;
        let g = g as f32 / 255.0;
        let b = b as f32 / 255.0;

        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) / 2.0;

        if (max - min).abs() < 0.0001 {
            return (0.0, 0.0, l);
        }

        let d = max - min;
        let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };

        let h = if (max - r).abs() < 0.0001 {
            (g - b) / d + if g < b { 6.0 } else { 0.0 }
        } else if (max - g).abs() < 0.0001 {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };

        (h / 6.0, s, l)
    }

    /// Convert HSL to RGB
    pub fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
        if s.abs() < 0.0001 {
            let v = (l * 255.0) as u8;
            return (v, v, v);
        }

        let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
        let p = 2.0 * l - q;

        let r = hue_to_rgb(p, q, h + 1.0 / 3.0);
        let g = hue_to_rgb(p, q, h);
        let b = hue_to_rgb(p, q, h - 1.0 / 3.0);

        ((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
    }

    fn hue_to_rgb(p: f32, q: f32, mut t: f32) -> f32 {
        if t < 0.0 { t += 1.0; }
        if t > 1.0 { t -= 1.0; }

        if t < 1.0 / 6.0 { return p + (q - p) * 6.0 * t; }
        if t < 1.0 / 2.0 { return q; }
        if t < 2.0 / 3.0 { return p + (q - p) * (2.0 / 3.0 - t) * 6.0; }
        p
    }

    /// Lighten a color
    pub fn lighten(rgb: u32, amount: f32) -> u32 {
        let r = ((rgb >> 16) & 0xFF) as u8;
        let g = ((rgb >> 8) & 0xFF) as u8;
        let b = (rgb & 0xFF) as u8;

        let (h, s, l) = rgb_to_hsl(r, g, b);
        let new_l = (l + amount).min(1.0);
        let (r, g, b) = hsl_to_rgb(h, s, new_l);

        ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
    }

    /// Darken a color
    pub fn darken(rgb: u32, amount: f32) -> u32 {
        let r = ((rgb >> 16) & 0xFF) as u8;
        let g = ((rgb >> 8) & 0xFF) as u8;
        let b = (rgb & 0xFF) as u8;

        let (h, s, l) = rgb_to_hsl(r, g, b);
        let new_l = (l - amount).max(0.0);
        let (r, g, b) = hsl_to_rgb(h, s, new_l);

        ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
    }

    /// Mix two colors
    pub fn mix(color1: u32, color2: u32, ratio: f32) -> u32 {
        let r1 = ((color1 >> 16) & 0xFF) as f32;
        let g1 = ((color1 >> 8) & 0xFF) as f32;
        let b1 = (color1 & 0xFF) as f32;

        let r2 = ((color2 >> 16) & 0xFF) as f32;
        let g2 = ((color2 >> 8) & 0xFF) as f32;
        let b2 = (color2 & 0xFF) as f32;

        let r = (r1 * (1.0 - ratio) + r2 * ratio) as u32;
        let g = (g1 * (1.0 - ratio) + g2 * ratio) as u32;
        let b = (b1 * (1.0 - ratio) + b2 * ratio) as u32;

        (r << 16) | (g << 8) | b
    }

    /// Get contrasting text color (black or white)
    pub fn get_contrast_color(bg: u32) -> u32 {
        let r = ((bg >> 16) & 0xFF) as f32;
        let g = ((bg >> 8) & 0xFF) as f32;
        let b = (bg & 0xFF) as f32;

        // Calculate relative luminance
        let luminance = 0.299 * r + 0.587 * g + 0.114 * b;

        if luminance > 128.0 {
            0x000000 // Black text on light background
        } else {
            0xFFFFFF // White text on dark background
        }
    }

    /// Apply alpha to color
    pub fn with_alpha(rgb: u32, alpha: u8) -> u32 {
        ((alpha as u32) << 24) | rgb
    }
}
