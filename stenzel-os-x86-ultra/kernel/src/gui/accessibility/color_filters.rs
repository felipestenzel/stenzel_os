//! Color Filters for Accessibility
//!
//! Provides color filters for users with color vision deficiencies:
//! - Protanopia (red-blind)
//! - Deuteranopia (green-blind)
//! - Tritanopia (blue-blind)
//! - Grayscale
//! - Inverted colors
//! - Custom color adjustments

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;

use crate::sync::IrqSafeMutex;

/// Color vision deficiency type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorBlindnessType {
    /// Normal color vision
    Normal,
    /// Red-blind (Protanopia) - ~1% of males
    Protanopia,
    /// Red-weak (Protanomaly) - ~1% of males
    Protanomaly,
    /// Green-blind (Deuteranopia) - ~1% of males
    Deuteranopia,
    /// Green-weak (Deuteranomaly) - ~5% of males
    Deuteranomaly,
    /// Blue-blind (Tritanopia) - rare
    Tritanopia,
    /// Blue-weak (Tritanomaly) - rare
    Tritanomaly,
    /// Complete color blindness (Achromatopsia)
    Achromatopsia,
    /// Blue cone monochromacy
    BlueConeMonochromacy,
}

impl ColorBlindnessType {
    /// Get type name
    pub fn name(&self) -> &'static str {
        match self {
            ColorBlindnessType::Normal => "Normal Vision",
            ColorBlindnessType::Protanopia => "Protanopia (Red-Blind)",
            ColorBlindnessType::Protanomaly => "Protanomaly (Red-Weak)",
            ColorBlindnessType::Deuteranopia => "Deuteranopia (Green-Blind)",
            ColorBlindnessType::Deuteranomaly => "Deuteranomaly (Green-Weak)",
            ColorBlindnessType::Tritanopia => "Tritanopia (Blue-Blind)",
            ColorBlindnessType::Tritanomaly => "Tritanomaly (Blue-Weak)",
            ColorBlindnessType::Achromatopsia => "Achromatopsia (Total Color Blindness)",
            ColorBlindnessType::BlueConeMonochromacy => "Blue Cone Monochromacy",
        }
    }

    /// Get description
    pub fn description(&self) -> &'static str {
        match self {
            ColorBlindnessType::Normal => "No color vision correction needed",
            ColorBlindnessType::Protanopia => "Cannot perceive red light, confuses red/green",
            ColorBlindnessType::Protanomaly => "Reduced sensitivity to red light",
            ColorBlindnessType::Deuteranopia => "Cannot perceive green light, confuses red/green",
            ColorBlindnessType::Deuteranomaly => "Reduced sensitivity to green light (most common)",
            ColorBlindnessType::Tritanopia => "Cannot perceive blue light, confuses blue/yellow",
            ColorBlindnessType::Tritanomaly => "Reduced sensitivity to blue light",
            ColorBlindnessType::Achromatopsia => "Cannot perceive any colors, sees only grayscale",
            ColorBlindnessType::BlueConeMonochromacy => "Only blue cones functional",
        }
    }

    /// Get all types
    pub fn all() -> Vec<ColorBlindnessType> {
        alloc::vec![
            ColorBlindnessType::Normal,
            ColorBlindnessType::Protanopia,
            ColorBlindnessType::Protanomaly,
            ColorBlindnessType::Deuteranopia,
            ColorBlindnessType::Deuteranomaly,
            ColorBlindnessType::Tritanopia,
            ColorBlindnessType::Tritanomaly,
            ColorBlindnessType::Achromatopsia,
            ColorBlindnessType::BlueConeMonochromacy,
        ]
    }

    /// Check if this is a red-green deficiency
    pub fn is_red_green(&self) -> bool {
        matches!(
            self,
            ColorBlindnessType::Protanopia |
            ColorBlindnessType::Protanomaly |
            ColorBlindnessType::Deuteranopia |
            ColorBlindnessType::Deuteranomaly
        )
    }

    /// Check if this is a blue-yellow deficiency
    pub fn is_blue_yellow(&self) -> bool {
        matches!(
            self,
            ColorBlindnessType::Tritanopia |
            ColorBlindnessType::Tritanomaly
        )
    }
}

/// Color filter type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorFilterType {
    /// No filter
    None,
    /// Grayscale conversion
    Grayscale,
    /// Inverted colors
    Inverted,
    /// Inverted grayscale
    InvertedGrayscale,
    /// Color blindness simulation (for testing)
    Simulation(ColorBlindnessType),
    /// Color blindness correction
    Correction(ColorBlindnessType),
    /// Red-green filter (enhances contrast)
    RedGreenFilter,
    /// Blue-yellow filter
    BlueYellowFilter,
    /// Sepia tone
    Sepia,
    /// Night mode (reduces blue light)
    NightMode,
    /// High saturation
    HighSaturation,
    /// Low saturation
    LowSaturation,
    /// Custom matrix filter
    Custom,
}

impl ColorFilterType {
    /// Get filter name
    pub fn name(&self) -> &'static str {
        match self {
            ColorFilterType::None => "None",
            ColorFilterType::Grayscale => "Grayscale",
            ColorFilterType::Inverted => "Inverted",
            ColorFilterType::InvertedGrayscale => "Inverted Grayscale",
            ColorFilterType::Simulation(_) => "Color Blindness Simulation",
            ColorFilterType::Correction(_) => "Color Blindness Correction",
            ColorFilterType::RedGreenFilter => "Red-Green Filter",
            ColorFilterType::BlueYellowFilter => "Blue-Yellow Filter",
            ColorFilterType::Sepia => "Sepia",
            ColorFilterType::NightMode => "Night Mode",
            ColorFilterType::HighSaturation => "High Saturation",
            ColorFilterType::LowSaturation => "Low Saturation",
            ColorFilterType::Custom => "Custom",
        }
    }
}

/// RGB color
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    /// Create new RGB color
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Convert to floating point (0.0-1.0)
    pub fn to_float(&self) -> (f32, f32, f32) {
        (
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
        )
    }

    /// Create from floating point (0.0-1.0)
    pub fn from_float(r: f32, g: f32, b: f32) -> Self {
        Self {
            r: (r.max(0.0).min(1.0) * 255.0) as u8,
            g: (g.max(0.0).min(1.0) * 255.0) as u8,
            b: (b.max(0.0).min(1.0) * 255.0) as u8,
        }
    }

    /// Convert to grayscale using luminance formula
    pub fn to_grayscale(&self) -> u8 {
        // ITU-R BT.709 luminance coefficients
        let luma = 0.2126 * self.r as f32 + 0.7152 * self.g as f32 + 0.0722 * self.b as f32;
        luma.max(0.0).min(255.0) as u8
    }

    /// Invert color
    pub fn invert(&self) -> Self {
        Self {
            r: 255 - self.r,
            g: 255 - self.g,
            b: 255 - self.b,
        }
    }

    /// Apply sepia filter
    pub fn to_sepia(&self) -> Self {
        let (r, g, b) = self.to_float();
        let new_r = 0.393 * r + 0.769 * g + 0.189 * b;
        let new_g = 0.349 * r + 0.686 * g + 0.168 * b;
        let new_b = 0.272 * r + 0.534 * g + 0.131 * b;
        Self::from_float(new_r, new_g, new_b)
    }

    /// Adjust saturation
    pub fn adjust_saturation(&self, factor: f32) -> Self {
        let gray = self.to_grayscale() as f32 / 255.0;
        let (r, g, b) = self.to_float();

        let new_r = gray + factor * (r - gray);
        let new_g = gray + factor * (g - gray);
        let new_b = gray + factor * (b - gray);

        Self::from_float(new_r, new_g, new_b)
    }

    /// Apply night mode (reduce blue)
    pub fn apply_night_mode(&self, intensity: f32) -> Self {
        let (r, g, b) = self.to_float();
        let blue_reduction = 1.0 - intensity * 0.5;
        let warm_boost = 1.0 + intensity * 0.1;

        Self::from_float(
            r * warm_boost,
            g,
            b * blue_reduction,
        )
    }
}

/// 3x3 Color transformation matrix
#[derive(Debug, Clone, Copy)]
pub struct ColorMatrix {
    /// Matrix values [row][col]
    pub m: [[f32; 3]; 3],
}

impl ColorMatrix {
    /// Identity matrix (no transformation)
    pub const fn identity() -> Self {
        Self {
            m: [
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
            ],
        }
    }

    /// Grayscale matrix
    pub const fn grayscale() -> Self {
        Self {
            m: [
                [0.2126, 0.7152, 0.0722],
                [0.2126, 0.7152, 0.0722],
                [0.2126, 0.7152, 0.0722],
            ],
        }
    }

    /// Sepia matrix
    pub const fn sepia() -> Self {
        Self {
            m: [
                [0.393, 0.769, 0.189],
                [0.349, 0.686, 0.168],
                [0.272, 0.534, 0.131],
            ],
        }
    }

    /// Protanopia simulation matrix
    pub const fn protanopia_simulation() -> Self {
        Self {
            m: [
                [0.567, 0.433, 0.0],
                [0.558, 0.442, 0.0],
                [0.0, 0.242, 0.758],
            ],
        }
    }

    /// Deuteranopia simulation matrix
    pub const fn deuteranopia_simulation() -> Self {
        Self {
            m: [
                [0.625, 0.375, 0.0],
                [0.7, 0.3, 0.0],
                [0.0, 0.3, 0.7],
            ],
        }
    }

    /// Tritanopia simulation matrix
    pub const fn tritanopia_simulation() -> Self {
        Self {
            m: [
                [0.95, 0.05, 0.0],
                [0.0, 0.433, 0.567],
                [0.0, 0.475, 0.525],
            ],
        }
    }

    /// Protanopia correction matrix (Daltonization)
    pub fn protanopia_correction() -> Self {
        Self {
            m: [
                [1.0, 0.0, 0.0],
                [0.7, 1.0, 0.0],
                [0.7, 0.0, 1.0],
            ],
        }
    }

    /// Deuteranopia correction matrix (Daltonization)
    pub fn deuteranopia_correction() -> Self {
        Self {
            m: [
                [1.0, 0.7, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.7, 1.0],
            ],
        }
    }

    /// Tritanopia correction matrix (Daltonization)
    pub fn tritanopia_correction() -> Self {
        Self {
            m: [
                [1.0, 0.0, 0.7],
                [0.0, 1.0, 0.7],
                [0.0, 0.0, 1.0],
            ],
        }
    }

    /// Apply matrix to color
    pub fn apply(&self, color: Rgb) -> Rgb {
        let (r, g, b) = color.to_float();

        let new_r = self.m[0][0] * r + self.m[0][1] * g + self.m[0][2] * b;
        let new_g = self.m[1][0] * r + self.m[1][1] * g + self.m[1][2] * b;
        let new_b = self.m[2][0] * r + self.m[2][1] * g + self.m[2][2] * b;

        Rgb::from_float(new_r, new_g, new_b)
    }

    /// Multiply two matrices
    pub fn multiply(&self, other: &ColorMatrix) -> ColorMatrix {
        let mut result = [[0.0f32; 3]; 3];

        for i in 0..3 {
            for j in 0..3 {
                for k in 0..3 {
                    result[i][j] += self.m[i][k] * other.m[k][j];
                }
            }
        }

        ColorMatrix { m: result }
    }

    /// Interpolate between two matrices
    pub fn lerp(&self, other: &ColorMatrix, t: f32) -> ColorMatrix {
        let mut result = [[0.0f32; 3]; 3];
        let t = t.max(0.0).min(1.0);

        for i in 0..3 {
            for j in 0..3 {
                result[i][j] = self.m[i][j] * (1.0 - t) + other.m[i][j] * t;
            }
        }

        ColorMatrix { m: result }
    }
}

impl Default for ColorMatrix {
    fn default() -> Self {
        Self::identity()
    }
}

/// Color filter configuration
#[derive(Debug, Clone)]
pub struct ColorFilterConfig {
    /// Whether color filters are enabled
    pub enabled: bool,
    /// Active filter type
    pub filter_type: ColorFilterType,
    /// Filter intensity (0.0-1.0)
    pub intensity: f32,
    /// Night mode settings
    pub night_mode_intensity: f32,
    /// Custom matrix (when filter_type is Custom)
    pub custom_matrix: ColorMatrix,
    /// Apply to system UI only
    pub system_ui_only: bool,
    /// Exclude specific apps
    pub excluded_apps: Vec<String>,
    /// Auto night mode based on time
    pub auto_night_mode: bool,
    /// Night mode start hour (24h)
    pub night_start_hour: u8,
    /// Night mode end hour (24h)
    pub night_end_hour: u8,
    /// Transition duration for filter changes (ms)
    pub transition_duration_ms: u32,
}

impl Default for ColorFilterConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            filter_type: ColorFilterType::None,
            intensity: 1.0,
            night_mode_intensity: 0.5,
            custom_matrix: ColorMatrix::identity(),
            system_ui_only: false,
            excluded_apps: Vec::new(),
            auto_night_mode: false,
            night_start_hour: 22,
            night_end_hour: 6,
            transition_duration_ms: 500,
        }
    }
}

/// Color filter statistics
#[derive(Debug, Clone, Default)]
pub struct ColorFilterStats {
    /// Pixels processed
    pub pixels_processed: u64,
    /// Frames filtered
    pub frames_filtered: u64,
    /// Filter changes
    pub filter_changes: u64,
    /// Night mode activations
    pub night_mode_activations: u64,
    /// Session start timestamp
    pub session_start_ms: u64,
}

/// Color Filter Manager
pub struct ColorFilterManager {
    /// Configuration
    config: ColorFilterConfig,
    /// Current active matrix
    active_matrix: ColorMatrix,
    /// Transition state
    transition_start_matrix: ColorMatrix,
    transition_end_matrix: ColorMatrix,
    transition_start_ms: u64,
    transitioning: bool,
    /// Statistics
    stats: ColorFilterStats,
    /// Color correction lookup table (for performance)
    lut_enabled: bool,
    lut: Option<Vec<Rgb>>,
}

impl ColorFilterManager {
    /// Create a new color filter manager
    pub fn new() -> Self {
        Self {
            config: ColorFilterConfig::default(),
            active_matrix: ColorMatrix::identity(),
            transition_start_matrix: ColorMatrix::identity(),
            transition_end_matrix: ColorMatrix::identity(),
            transition_start_ms: 0,
            transitioning: false,
            stats: ColorFilterStats::default(),
            lut_enabled: false,
            lut: None,
        }
    }

    /// Initialize
    pub fn init(&mut self) {
        self.stats.session_start_ms = crate::time::uptime_ms();
        crate::kprintln!("[color_filters] Color filter manager initialized");
    }

    /// Enable color filters
    pub fn enable(&mut self) {
        self.config.enabled = true;
        self.update_active_matrix();
        crate::kprintln!("[color_filters] Color filters enabled ({})",
            self.config.filter_type.name());
    }

    /// Disable color filters
    pub fn disable(&mut self) {
        self.config.enabled = false;
        self.start_transition(ColorMatrix::identity());
        crate::kprintln!("[color_filters] Color filters disabled");
    }

    /// Toggle color filters
    pub fn toggle(&mut self) {
        if self.config.enabled {
            self.disable();
        } else {
            self.enable();
        }
    }

    /// Check if enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Set filter type
    pub fn set_filter(&mut self, filter_type: ColorFilterType) {
        let old_type = self.config.filter_type;
        self.config.filter_type = filter_type;

        if self.config.enabled {
            self.update_active_matrix();
            self.stats.filter_changes += 1;
        }

        crate::kprintln!("[color_filters] Filter changed from {} to {}",
            old_type.name(), filter_type.name());
    }

    /// Get current filter type
    pub fn filter_type(&self) -> ColorFilterType {
        self.config.filter_type
    }

    /// Set filter intensity
    pub fn set_intensity(&mut self, intensity: f32) {
        self.config.intensity = intensity.max(0.0).min(1.0);
        if self.config.enabled {
            self.update_active_matrix();
        }
    }

    /// Get filter intensity
    pub fn intensity(&self) -> f32 {
        self.config.intensity
    }

    /// Set color blindness correction
    pub fn set_color_blindness_correction(&mut self, cb_type: ColorBlindnessType) {
        if cb_type == ColorBlindnessType::Normal {
            self.config.filter_type = ColorFilterType::None;
        } else {
            self.config.filter_type = ColorFilterType::Correction(cb_type);
        }

        if self.config.enabled {
            self.update_active_matrix();
        }

        crate::kprintln!("[color_filters] Color blindness correction set to {}",
            cb_type.name());
    }

    /// Set color blindness simulation (for testing)
    pub fn set_color_blindness_simulation(&mut self, cb_type: ColorBlindnessType) {
        if cb_type == ColorBlindnessType::Normal {
            self.config.filter_type = ColorFilterType::None;
        } else {
            self.config.filter_type = ColorFilterType::Simulation(cb_type);
        }

        if self.config.enabled {
            self.update_active_matrix();
        }
    }

    /// Enable night mode
    pub fn enable_night_mode(&mut self, intensity: f32) {
        self.config.filter_type = ColorFilterType::NightMode;
        self.config.night_mode_intensity = intensity.max(0.0).min(1.0);
        self.config.enabled = true;
        self.update_active_matrix();
        self.stats.night_mode_activations += 1;
        crate::kprintln!("[color_filters] Night mode enabled (intensity: {:.0}%)",
            intensity * 100.0);
    }

    /// Disable night mode
    pub fn disable_night_mode(&mut self) {
        if self.config.filter_type == ColorFilterType::NightMode {
            self.config.filter_type = ColorFilterType::None;
            self.config.enabled = false;
            self.start_transition(ColorMatrix::identity());
            crate::kprintln!("[color_filters] Night mode disabled");
        }
    }

    /// Set custom matrix
    pub fn set_custom_matrix(&mut self, matrix: ColorMatrix) {
        self.config.custom_matrix = matrix;
        self.config.filter_type = ColorFilterType::Custom;

        if self.config.enabled {
            self.update_active_matrix();
        }
    }

    /// Update active matrix based on current settings
    fn update_active_matrix(&mut self) {
        let target_matrix = self.get_matrix_for_filter(self.config.filter_type);

        // Apply intensity interpolation
        let intensity_matrix = if self.config.intensity < 1.0 {
            ColorMatrix::identity().lerp(&target_matrix, self.config.intensity)
        } else {
            target_matrix
        };

        self.start_transition(intensity_matrix);
    }

    /// Get matrix for a filter type
    fn get_matrix_for_filter(&self, filter_type: ColorFilterType) -> ColorMatrix {
        match filter_type {
            ColorFilterType::None => ColorMatrix::identity(),
            ColorFilterType::Grayscale => ColorMatrix::grayscale(),
            ColorFilterType::Inverted => {
                // Invert is not a simple matrix, handle specially
                ColorMatrix {
                    m: [
                        [-1.0, 0.0, 0.0],
                        [0.0, -1.0, 0.0],
                        [0.0, 0.0, -1.0],
                    ],
                }
            }
            ColorFilterType::InvertedGrayscale => {
                let gray = ColorMatrix::grayscale();
                // Apply grayscale then invert
                ColorMatrix {
                    m: [
                        [-gray.m[0][0], -gray.m[0][1], -gray.m[0][2]],
                        [-gray.m[1][0], -gray.m[1][1], -gray.m[1][2]],
                        [-gray.m[2][0], -gray.m[2][1], -gray.m[2][2]],
                    ],
                }
            }
            ColorFilterType::Simulation(cb_type) => {
                self.get_simulation_matrix(cb_type)
            }
            ColorFilterType::Correction(cb_type) => {
                self.get_correction_matrix(cb_type)
            }
            ColorFilterType::RedGreenFilter => {
                // Enhance red-green contrast
                ColorMatrix {
                    m: [
                        [1.5, -0.5, 0.0],
                        [-0.5, 1.5, 0.0],
                        [0.0, 0.0, 1.0],
                    ],
                }
            }
            ColorFilterType::BlueYellowFilter => {
                // Enhance blue-yellow contrast
                ColorMatrix {
                    m: [
                        [1.0, 0.0, 0.0],
                        [0.0, 1.5, -0.5],
                        [0.0, -0.5, 1.5],
                    ],
                }
            }
            ColorFilterType::Sepia => ColorMatrix::sepia(),
            ColorFilterType::NightMode => {
                // Reduce blue, warm up
                let i = self.config.night_mode_intensity;
                ColorMatrix {
                    m: [
                        [1.0 + i * 0.1, 0.0, 0.0],
                        [0.0, 1.0, 0.0],
                        [0.0, 0.0, 1.0 - i * 0.5],
                    ],
                }
            }
            ColorFilterType::HighSaturation => {
                ColorMatrix {
                    m: [
                        [1.5, -0.25, -0.25],
                        [-0.25, 1.5, -0.25],
                        [-0.25, -0.25, 1.5],
                    ],
                }
            }
            ColorFilterType::LowSaturation => {
                let gray = ColorMatrix::grayscale();
                ColorMatrix::identity().lerp(&gray, 0.5)
            }
            ColorFilterType::Custom => self.config.custom_matrix,
        }
    }

    /// Get simulation matrix for color blindness type
    fn get_simulation_matrix(&self, cb_type: ColorBlindnessType) -> ColorMatrix {
        match cb_type {
            ColorBlindnessType::Normal => ColorMatrix::identity(),
            ColorBlindnessType::Protanopia => ColorMatrix::protanopia_simulation(),
            ColorBlindnessType::Protanomaly => {
                ColorMatrix::identity().lerp(&ColorMatrix::protanopia_simulation(), 0.5)
            }
            ColorBlindnessType::Deuteranopia => ColorMatrix::deuteranopia_simulation(),
            ColorBlindnessType::Deuteranomaly => {
                ColorMatrix::identity().lerp(&ColorMatrix::deuteranopia_simulation(), 0.5)
            }
            ColorBlindnessType::Tritanopia => ColorMatrix::tritanopia_simulation(),
            ColorBlindnessType::Tritanomaly => {
                ColorMatrix::identity().lerp(&ColorMatrix::tritanopia_simulation(), 0.5)
            }
            ColorBlindnessType::Achromatopsia => ColorMatrix::grayscale(),
            ColorBlindnessType::BlueConeMonochromacy => {
                // Simulate with heavy grayscale + blue tint
                ColorMatrix {
                    m: [
                        [0.1, 0.1, 0.8],
                        [0.1, 0.1, 0.8],
                        [0.1, 0.1, 0.8],
                    ],
                }
            }
        }
    }

    /// Get correction matrix for color blindness type
    fn get_correction_matrix(&self, cb_type: ColorBlindnessType) -> ColorMatrix {
        match cb_type {
            ColorBlindnessType::Normal => ColorMatrix::identity(),
            ColorBlindnessType::Protanopia | ColorBlindnessType::Protanomaly => {
                ColorMatrix::protanopia_correction()
            }
            ColorBlindnessType::Deuteranopia | ColorBlindnessType::Deuteranomaly => {
                ColorMatrix::deuteranopia_correction()
            }
            ColorBlindnessType::Tritanopia | ColorBlindnessType::Tritanomaly => {
                ColorMatrix::tritanopia_correction()
            }
            ColorBlindnessType::Achromatopsia | ColorBlindnessType::BlueConeMonochromacy => {
                // No correction possible, enhance contrast instead
                ColorMatrix {
                    m: [
                        [1.2, 0.0, 0.0],
                        [0.0, 1.2, 0.0],
                        [0.0, 0.0, 1.2],
                    ],
                }
            }
        }
    }

    /// Start transition to new matrix
    fn start_transition(&mut self, target: ColorMatrix) {
        self.transition_start_matrix = self.active_matrix;
        self.transition_end_matrix = target;
        self.transition_start_ms = crate::time::uptime_ms();
        self.transitioning = true;

        // Invalidate LUT
        self.lut = None;
    }

    /// Update transition (call each frame)
    pub fn update(&mut self) {
        if !self.transitioning {
            return;
        }

        let now = crate::time::uptime_ms();
        let elapsed = now - self.transition_start_ms;
        let duration = self.config.transition_duration_ms as u64;

        if elapsed >= duration {
            // Transition complete
            self.active_matrix = self.transition_end_matrix;
            self.transitioning = false;

            // Rebuild LUT if enabled
            if self.lut_enabled {
                self.rebuild_lut();
            }
        } else {
            // Interpolate
            let t = elapsed as f32 / duration as f32;
            self.active_matrix = self.transition_start_matrix.lerp(&self.transition_end_matrix, t);
        }
    }

    /// Apply filter to a single color
    pub fn apply_filter(&mut self, color: Rgb) -> Rgb {
        if !self.config.enabled {
            return color;
        }

        self.stats.pixels_processed += 1;

        // Use LUT if available
        if self.lut_enabled {
            if let Some(ref lut) = self.lut {
                let index = ((color.r as usize) << 16) | ((color.g as usize) << 8) | (color.b as usize);
                if index < lut.len() {
                    return lut[index];
                }
            }
        }

        // Handle special case for inversion
        let result = match self.config.filter_type {
            ColorFilterType::Inverted if self.config.intensity >= 1.0 => {
                color.invert()
            }
            ColorFilterType::InvertedGrayscale if self.config.intensity >= 1.0 => {
                let gray = color.to_grayscale();
                Rgb::new(255 - gray, 255 - gray, 255 - gray)
            }
            _ => {
                self.active_matrix.apply(color)
            }
        };

        result
    }

    /// Apply filter to a buffer of pixels (RGBA format)
    pub fn apply_filter_buffer(&mut self, buffer: &mut [u8]) {
        if !self.config.enabled {
            return;
        }

        self.stats.frames_filtered += 1;

        // Process 4 bytes at a time (RGBA)
        for chunk in buffer.chunks_mut(4) {
            if chunk.len() >= 3 {
                let color = Rgb::new(chunk[0], chunk[1], chunk[2]);
                let filtered = self.apply_filter(color);
                chunk[0] = filtered.r;
                chunk[1] = filtered.g;
                chunk[2] = filtered.b;
                // Alpha unchanged
            }
        }
    }

    /// Enable LUT for faster processing (uses more memory)
    pub fn enable_lut(&mut self) {
        self.lut_enabled = true;
        self.rebuild_lut();
    }

    /// Disable LUT
    pub fn disable_lut(&mut self) {
        self.lut_enabled = false;
        self.lut = None;
    }

    /// Rebuild lookup table
    fn rebuild_lut(&mut self) {
        // Full LUT would be 16MB (256^3 * 3 bytes), use smaller approximation
        // We'll use a 32x32x32 LUT with interpolation
        let size = 32;
        let mut lut = Vec::with_capacity(size * size * size);

        for r in 0..size {
            for g in 0..size {
                for b in 0..size {
                    let color = Rgb::new(
                        (r * 255 / (size - 1)) as u8,
                        (g * 255 / (size - 1)) as u8,
                        (b * 255 / (size - 1)) as u8,
                    );
                    let filtered = self.active_matrix.apply(color);
                    lut.push(filtered);
                }
            }
        }

        self.lut = Some(lut);
    }

    /// Check if auto night mode should be active
    pub fn check_auto_night_mode(&mut self, current_hour: u8) {
        if !self.config.auto_night_mode {
            return;
        }

        let should_be_night = if self.config.night_start_hour > self.config.night_end_hour {
            // Crosses midnight (e.g., 22:00 - 06:00)
            current_hour >= self.config.night_start_hour || current_hour < self.config.night_end_hour
        } else {
            // Same day (e.g., 20:00 - 23:00)
            current_hour >= self.config.night_start_hour && current_hour < self.config.night_end_hour
        };

        let is_night = self.config.filter_type == ColorFilterType::NightMode && self.config.enabled;

        if should_be_night && !is_night {
            self.enable_night_mode(self.config.night_mode_intensity);
        } else if !should_be_night && is_night {
            self.disable_night_mode();
        }
    }

    /// Get configuration
    pub fn config(&self) -> &ColorFilterConfig {
        &self.config
    }

    /// Set configuration
    pub fn set_config(&mut self, config: ColorFilterConfig) {
        self.config = config;
        if self.config.enabled {
            self.update_active_matrix();
        }
    }

    /// Get statistics
    pub fn stats(&self) -> &ColorFilterStats {
        &self.stats
    }

    /// Get active matrix (for debugging/display)
    pub fn active_matrix(&self) -> &ColorMatrix {
        &self.active_matrix
    }

    /// Format status string
    pub fn format_status(&self) -> String {
        format!(
            "Color Filters:\n\
             Enabled: {}\n\
             Filter: {}\n\
             Intensity: {:.0}%\n\
             Night mode intensity: {:.0}%\n\
             Auto night mode: {}\n\
             LUT enabled: {}\n\
             Frames filtered: {}\n\
             Filter changes: {}",
            if self.config.enabled { "Yes" } else { "No" },
            self.config.filter_type.name(),
            self.config.intensity * 100.0,
            self.config.night_mode_intensity * 100.0,
            if self.config.auto_night_mode { "Yes" } else { "No" },
            if self.lut_enabled { "Yes" } else { "No" },
            self.stats.frames_filtered,
            self.stats.filter_changes
        )
    }
}

/// Global color filter manager
static COLOR_FILTERS: IrqSafeMutex<Option<ColorFilterManager>> = IrqSafeMutex::new(None);

/// Initialize color filters
pub fn init() {
    let mut manager = ColorFilterManager::new();
    manager.init();
    *COLOR_FILTERS.lock() = Some(manager);
}

/// Enable color filters
pub fn enable() {
    if let Some(ref mut manager) = *COLOR_FILTERS.lock() {
        manager.enable();
    }
}

/// Disable color filters
pub fn disable() {
    if let Some(ref mut manager) = *COLOR_FILTERS.lock() {
        manager.disable();
    }
}

/// Toggle color filters
pub fn toggle() {
    if let Some(ref mut manager) = *COLOR_FILTERS.lock() {
        manager.toggle();
    }
}

/// Check if enabled
pub fn is_enabled() -> bool {
    COLOR_FILTERS.lock().as_ref().map(|m| m.is_enabled()).unwrap_or(false)
}

/// Set filter type
pub fn set_filter(filter_type: ColorFilterType) {
    if let Some(ref mut manager) = *COLOR_FILTERS.lock() {
        manager.set_filter(filter_type);
    }
}

/// Get filter type
pub fn get_filter() -> ColorFilterType {
    COLOR_FILTERS.lock().as_ref().map(|m| m.filter_type()).unwrap_or(ColorFilterType::None)
}

/// Set color blindness correction
pub fn set_color_blindness_correction(cb_type: ColorBlindnessType) {
    if let Some(ref mut manager) = *COLOR_FILTERS.lock() {
        manager.set_color_blindness_correction(cb_type);
    }
}

/// Enable night mode
pub fn enable_night_mode(intensity: f32) {
    if let Some(ref mut manager) = *COLOR_FILTERS.lock() {
        manager.enable_night_mode(intensity);
    }
}

/// Disable night mode
pub fn disable_night_mode() {
    if let Some(ref mut manager) = *COLOR_FILTERS.lock() {
        manager.disable_night_mode();
    }
}

/// Apply filter to color
pub fn apply_filter(color: Rgb) -> Rgb {
    if let Some(ref mut manager) = *COLOR_FILTERS.lock() {
        manager.apply_filter(color)
    } else {
        color
    }
}

/// Get status string
pub fn status() -> String {
    COLOR_FILTERS.lock().as_ref()
        .map(|m| m.format_status())
        .unwrap_or_else(|| String::from("Color Filters: Not initialized"))
}

/// Get statistics
pub fn stats() -> Option<ColorFilterStats> {
    COLOR_FILTERS.lock().as_ref().map(|m| m.stats().clone())
}
