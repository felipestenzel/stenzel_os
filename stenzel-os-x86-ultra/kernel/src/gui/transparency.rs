//! Window Transparency Module
//!
//! Provides transparency and visual effects for windows:
//! - Per-window opacity control
//! - Alpha blending for compositing
//! - Blur effects (Gaussian blur)
//! - Drop shadows
//! - Glass/Aero-style effects

#![allow(dead_code)]

use alloc::vec::Vec;
use crate::drivers::framebuffer::Color;
use super::surface::Surface;

// ============================================================================
// Math Helpers (no_std compatible)
// ============================================================================

/// Round f32 to nearest integer
fn round_f32(x: f32) -> f32 {
    if x >= 0.0 {
        (x + 0.5) as i32 as f32
    } else {
        (x - 0.5) as i32 as f32
    }
}

/// Approximate exp for f32 using Taylor series
fn exp_f32(x: f32) -> f32 {
    // Clamp input to reasonable range
    let x = x.max(-20.0).min(20.0);

    // Use Taylor series: e^x = 1 + x + x^2/2! + x^3/3! + ...
    let mut result = 1.0;
    let mut term = 1.0;
    for i in 1..20 {
        term *= x / (i as f32);
        result += term;
        if term.abs() < 1e-10 {
            break;
        }
    }
    result
}

/// Approximate sqrt for f32 using Newton's method
fn sqrt_f32(x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }

    // Initial guess
    let mut guess = x / 2.0;
    if guess == 0.0 {
        guess = 1.0;
    }

    // Newton's method iterations
    for _ in 0..10 {
        let new_guess = (guess + x / guess) / 2.0;
        if (new_guess - guess).abs() < 1e-7 {
            break;
        }
        guess = new_guess;
    }
    guess
}

// ============================================================================
// Opacity and Blending
// ============================================================================

/// Window opacity value (0-255, where 255 is fully opaque)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Opacity(pub u8);

impl Opacity {
    /// Fully transparent
    pub const TRANSPARENT: Opacity = Opacity(0);
    /// 25% opaque
    pub const QUARTER: Opacity = Opacity(64);
    /// 50% opaque
    pub const HALF: Opacity = Opacity(128);
    /// 75% opaque
    pub const THREE_QUARTER: Opacity = Opacity(192);
    /// Fully opaque
    pub const OPAQUE: Opacity = Opacity(255);

    /// Create from percentage (0-100)
    pub fn from_percent(percent: u8) -> Self {
        let clamped = percent.min(100);
        Opacity((clamped as u32 * 255 / 100) as u8)
    }

    /// Get as percentage (0-100)
    pub fn as_percent(&self) -> u8 {
        ((self.0 as u32 * 100) / 255) as u8
    }

    /// Get as float (0.0-1.0)
    pub fn as_float(&self) -> f32 {
        self.0 as f32 / 255.0
    }

    /// Create from float (0.0-1.0)
    pub fn from_float(f: f32) -> Self {
        let clamped = f.max(0.0).min(1.0);
        Opacity((clamped * 255.0) as u8)
    }

    /// Check if fully transparent
    pub fn is_transparent(&self) -> bool {
        self.0 == 0
    }

    /// Check if fully opaque
    pub fn is_opaque(&self) -> bool {
        self.0 == 255
    }

    /// Multiply two opacity values
    pub fn multiply(&self, other: Opacity) -> Opacity {
        Opacity(((self.0 as u32 * other.0 as u32) / 255) as u8)
    }
}

impl Default for Opacity {
    fn default() -> Self {
        Self::OPAQUE
    }
}

impl From<u8> for Opacity {
    fn from(value: u8) -> Self {
        Opacity(value)
    }
}

/// Blend mode for compositing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlendMode {
    /// Normal alpha blending (Porter-Duff Source Over)
    #[default]
    Normal,
    /// Multiply blend (darken)
    Multiply,
    /// Screen blend (lighten)
    Screen,
    /// Overlay blend
    Overlay,
    /// Additive blend
    Add,
    /// Subtractive blend
    Subtract,
    /// Replace (no blending)
    Replace,
}

impl BlendMode {
    /// Blend a source color with a destination color
    pub fn blend(&self, src: Color, dst: Color, opacity: Opacity) -> Color {
        match self {
            BlendMode::Normal => blend_normal(src, dst, opacity),
            BlendMode::Multiply => blend_multiply(src, dst, opacity),
            BlendMode::Screen => blend_screen(src, dst, opacity),
            BlendMode::Overlay => blend_overlay(src, dst, opacity),
            BlendMode::Add => blend_add(src, dst, opacity),
            BlendMode::Subtract => blend_subtract(src, dst, opacity),
            BlendMode::Replace => {
                if opacity.is_opaque() {
                    src
                } else {
                    blend_normal(src, dst, opacity)
                }
            }
        }
    }
}

/// Normal alpha blending (Porter-Duff Source Over)
fn blend_normal(src: Color, dst: Color, opacity: Opacity) -> Color {
    if opacity.is_opaque() && src.a == 255 {
        return src;
    }
    if opacity.is_transparent() || src.a == 0 {
        return dst;
    }

    // Premultiply source alpha with opacity
    let src_alpha = ((src.a as u32 * opacity.0 as u32) / 255) as u8;
    let inv_alpha = 255 - src_alpha;

    Color {
        r: ((src.r as u32 * src_alpha as u32 + dst.r as u32 * inv_alpha as u32) / 255) as u8,
        g: ((src.g as u32 * src_alpha as u32 + dst.g as u32 * inv_alpha as u32) / 255) as u8,
        b: ((src.b as u32 * src_alpha as u32 + dst.b as u32 * inv_alpha as u32) / 255) as u8,
        a: (src_alpha as u32 + (dst.a as u32 * inv_alpha as u32) / 255) as u8,
    }
}

/// Multiply blend (darkening)
fn blend_multiply(src: Color, dst: Color, opacity: Opacity) -> Color {
    let result = Color {
        r: ((src.r as u32 * dst.r as u32) / 255) as u8,
        g: ((src.g as u32 * dst.g as u32) / 255) as u8,
        b: ((src.b as u32 * dst.b as u32) / 255) as u8,
        a: dst.a,
    };
    blend_normal(result, dst, opacity)
}

/// Screen blend (lightening)
fn blend_screen(src: Color, dst: Color, opacity: Opacity) -> Color {
    let result = Color {
        r: 255 - (((255 - src.r as u32) * (255 - dst.r as u32)) / 255) as u8,
        g: 255 - (((255 - src.g as u32) * (255 - dst.g as u32)) / 255) as u8,
        b: 255 - (((255 - src.b as u32) * (255 - dst.b as u32)) / 255) as u8,
        a: dst.a,
    };
    blend_normal(result, dst, opacity)
}

/// Overlay blend
fn blend_overlay(src: Color, dst: Color, opacity: Opacity) -> Color {
    fn overlay_channel(a: u8, b: u8) -> u8 {
        if b < 128 {
            ((2 * a as u32 * b as u32) / 255) as u8
        } else {
            (255 - 2 * ((255 - a as u32) * (255 - b as u32)) / 255) as u8
        }
    }
    let result = Color {
        r: overlay_channel(src.r, dst.r),
        g: overlay_channel(src.g, dst.g),
        b: overlay_channel(src.b, dst.b),
        a: dst.a,
    };
    blend_normal(result, dst, opacity)
}

/// Additive blend
fn blend_add(src: Color, dst: Color, opacity: Opacity) -> Color {
    let add_r = (src.r as u32 + dst.r as u32).min(255) as u8;
    let add_g = (src.g as u32 + dst.g as u32).min(255) as u8;
    let add_b = (src.b as u32 + dst.b as u32).min(255) as u8;
    let result = Color { r: add_r, g: add_g, b: add_b, a: dst.a };
    blend_normal(result, dst, opacity)
}

/// Subtractive blend
fn blend_subtract(src: Color, dst: Color, opacity: Opacity) -> Color {
    let sub_r = (dst.r as i32 - src.r as i32).max(0) as u8;
    let sub_g = (dst.g as i32 - src.g as i32).max(0) as u8;
    let sub_b = (dst.b as i32 - src.b as i32).max(0) as u8;
    let result = Color { r: sub_r, g: sub_g, b: sub_b, a: dst.a };
    blend_normal(result, dst, opacity)
}

// ============================================================================
// Blur Effects
// ============================================================================

/// Blur type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlurType {
    /// Box blur (simple, fast)
    Box,
    /// Gaussian blur (better quality)
    Gaussian,
    /// Kawase blur (good for real-time)
    Kawase,
}

/// Blur configuration
#[derive(Debug, Clone, Copy)]
pub struct BlurConfig {
    /// Type of blur
    pub blur_type: BlurType,
    /// Blur radius in pixels
    pub radius: u32,
    /// Number of blur passes (for Kawase blur)
    pub passes: u32,
}

impl Default for BlurConfig {
    fn default() -> Self {
        Self {
            blur_type: BlurType::Box,
            radius: 5,
            passes: 1,
        }
    }
}

/// Apply box blur to a surface
pub fn box_blur(surface: &mut Surface, radius: u32) {
    if radius == 0 {
        return;
    }

    let width = surface.width();
    let height = surface.height();
    let radius = radius as usize;

    // Create temp buffer
    let mut temp: Vec<Color> = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            temp.push(surface.get_pixel(x, y).unwrap_or(Color::BLACK));
        }
    }

    // Horizontal pass
    for y in 0..height {
        for x in 0..width {
            let mut r_sum: u32 = 0;
            let mut g_sum: u32 = 0;
            let mut b_sum: u32 = 0;
            let mut a_sum: u32 = 0;
            let mut count: u32 = 0;

            let x_start = x.saturating_sub(radius);
            let x_end = (x + radius + 1).min(width);

            for sx in x_start..x_end {
                let idx = y * width + sx;
                let c = temp[idx];
                r_sum += c.r as u32;
                g_sum += c.g as u32;
                b_sum += c.b as u32;
                a_sum += c.a as u32;
                count += 1;
            }

            let avg = Color {
                r: (r_sum / count) as u8,
                g: (g_sum / count) as u8,
                b: (b_sum / count) as u8,
                a: (a_sum / count) as u8,
            };
            surface.set_pixel(x, y, avg);
        }
    }

    // Update temp buffer with horizontal result
    for y in 0..height {
        for x in 0..width {
            temp[y * width + x] = surface.get_pixel(x, y).unwrap_or(Color::BLACK);
        }
    }

    // Vertical pass
    for y in 0..height {
        for x in 0..width {
            let mut r_sum: u32 = 0;
            let mut g_sum: u32 = 0;
            let mut b_sum: u32 = 0;
            let mut a_sum: u32 = 0;
            let mut count: u32 = 0;

            let y_start = y.saturating_sub(radius);
            let y_end = (y + radius + 1).min(height);

            for sy in y_start..y_end {
                let idx = sy * width + x;
                let c = temp[idx];
                r_sum += c.r as u32;
                g_sum += c.g as u32;
                b_sum += c.b as u32;
                a_sum += c.a as u32;
                count += 1;
            }

            let avg = Color {
                r: (r_sum / count) as u8,
                g: (g_sum / count) as u8,
                b: (b_sum / count) as u8,
                a: (a_sum / count) as u8,
            };
            surface.set_pixel(x, y, avg);
        }
    }
}

/// Apply Gaussian blur to a surface
pub fn gaussian_blur(surface: &mut Surface, radius: u32) {
    if radius == 0 {
        return;
    }

    // Generate 1D Gaussian kernel
    let kernel = generate_gaussian_kernel(radius);
    let width = surface.width();
    let height = surface.height();

    // Create temp buffer
    let mut temp: Vec<Color> = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            temp.push(surface.get_pixel(x, y).unwrap_or(Color::BLACK));
        }
    }

    // Horizontal pass
    let half_radius = radius as isize;
    for y in 0..height {
        for x in 0..width {
            let mut r_sum: f32 = 0.0;
            let mut g_sum: f32 = 0.0;
            let mut b_sum: f32 = 0.0;
            let mut a_sum: f32 = 0.0;

            for (i, &weight) in kernel.iter().enumerate() {
                let sx = (x as isize + i as isize - half_radius)
                    .max(0)
                    .min(width as isize - 1) as usize;
                let idx = y * width + sx;
                let c = temp[idx];
                r_sum += c.r as f32 * weight;
                g_sum += c.g as f32 * weight;
                b_sum += c.b as f32 * weight;
                a_sum += c.a as f32 * weight;
            }

            let blurred = Color {
                r: round_f32(r_sum).max(0.0).min(255.0) as u8,
                g: round_f32(g_sum).max(0.0).min(255.0) as u8,
                b: round_f32(b_sum).max(0.0).min(255.0) as u8,
                a: round_f32(a_sum).max(0.0).min(255.0) as u8,
            };
            surface.set_pixel(x, y, blurred);
        }
    }

    // Update temp buffer
    for y in 0..height {
        for x in 0..width {
            temp[y * width + x] = surface.get_pixel(x, y).unwrap_or(Color::BLACK);
        }
    }

    // Vertical pass
    for y in 0..height {
        for x in 0..width {
            let mut r_sum: f32 = 0.0;
            let mut g_sum: f32 = 0.0;
            let mut b_sum: f32 = 0.0;
            let mut a_sum: f32 = 0.0;

            for (i, &weight) in kernel.iter().enumerate() {
                let sy = (y as isize + i as isize - half_radius)
                    .max(0)
                    .min(height as isize - 1) as usize;
                let idx = sy * width + x;
                let c = temp[idx];
                r_sum += c.r as f32 * weight;
                g_sum += c.g as f32 * weight;
                b_sum += c.b as f32 * weight;
                a_sum += c.a as f32 * weight;
            }

            let blurred = Color {
                r: round_f32(r_sum).max(0.0).min(255.0) as u8,
                g: round_f32(g_sum).max(0.0).min(255.0) as u8,
                b: round_f32(b_sum).max(0.0).min(255.0) as u8,
                a: round_f32(a_sum).max(0.0).min(255.0) as u8,
            };
            surface.set_pixel(x, y, blurred);
        }
    }
}

/// Generate a 1D Gaussian kernel
fn generate_gaussian_kernel(radius: u32) -> Vec<f32> {
    let size = (radius * 2 + 1) as usize;
    let sigma = radius as f32 / 3.0;
    let mut kernel = Vec::with_capacity(size);
    let mut sum = 0.0;

    for i in 0..size {
        let x = i as f32 - radius as f32;
        let value = gaussian(x, sigma);
        kernel.push(value);
        sum += value;
    }

    // Normalize
    for k in &mut kernel {
        *k /= sum;
    }

    kernel
}

/// Gaussian function
fn gaussian(x: f32, sigma: f32) -> f32 {
    let exp = -(x * x) / (2.0 * sigma * sigma);
    exp_f32(exp) / (sigma * sqrt_f32(2.0 * core::f32::consts::PI))
}

// ============================================================================
// Drop Shadow
// ============================================================================

/// Shadow configuration
#[derive(Debug, Clone, Copy)]
pub struct ShadowConfig {
    /// Shadow color
    pub color: Color,
    /// Horizontal offset
    pub offset_x: i32,
    /// Vertical offset
    pub offset_y: i32,
    /// Blur radius
    pub blur_radius: u32,
    /// Shadow spread (positive = larger, negative = smaller)
    pub spread: i32,
}

impl Default for ShadowConfig {
    fn default() -> Self {
        Self {
            color: Color::with_alpha(0, 0, 0, 128),
            offset_x: 4,
            offset_y: 4,
            blur_radius: 8,
            spread: 0,
        }
    }
}

/// Generate a drop shadow surface for a given rectangle
pub fn generate_shadow(width: usize, height: usize, config: &ShadowConfig) -> Surface {
    let shadow_width = width + (config.spread.abs() * 2) as usize + config.blur_radius as usize * 2;
    let shadow_height = height + (config.spread.abs() * 2) as usize + config.blur_radius as usize * 2;

    let mut shadow = Surface::new(shadow_width, shadow_height, super::surface::PixelFormat::Rgba8888);

    // Fill shadow rectangle
    let margin = config.blur_radius as usize + config.spread.abs() as usize;
    let inner_width = width.saturating_add_signed(config.spread as isize * 2);
    let inner_height = height.saturating_add_signed(config.spread as isize * 2);

    for y in 0..inner_height {
        for x in 0..inner_width {
            let sx = margin + x;
            let sy = margin + y;
            if sx < shadow_width && sy < shadow_height {
                shadow.set_pixel(sx, sy, config.color);
            }
        }
    }

    // Apply blur
    if config.blur_radius > 0 {
        box_blur(&mut shadow, config.blur_radius);
    }

    shadow
}

// ============================================================================
// Glass Effect (Aero-style)
// ============================================================================

/// Glass effect configuration
#[derive(Debug, Clone, Copy)]
pub struct GlassConfig {
    /// Background tint color
    pub tint_color: Color,
    /// Tint opacity (0-255)
    pub tint_opacity: u8,
    /// Background blur radius
    pub blur_radius: u32,
    /// Saturation adjustment (1.0 = normal, 0.0 = grayscale)
    pub saturation: f32,
    /// Brightness adjustment (1.0 = normal)
    pub brightness: f32,
}

impl Default for GlassConfig {
    fn default() -> Self {
        Self {
            tint_color: Color::WHITE,
            tint_opacity: 64,
            blur_radius: 10,
            saturation: 1.0,
            brightness: 1.1,
        }
    }
}

/// Apply glass effect to a surface
pub fn apply_glass_effect(surface: &mut Surface, config: &GlassConfig) {
    // Apply blur
    if config.blur_radius > 0 {
        box_blur(surface, config.blur_radius);
    }

    let width = surface.width();
    let height = surface.height();

    // Apply saturation, brightness, and tint
    for y in 0..height {
        for x in 0..width {
            if let Some(mut color) = surface.get_pixel(x, y) {
                // Adjust saturation
                if config.saturation != 1.0 {
                    let gray = (color.r as f32 * 0.299 + color.g as f32 * 0.587 + color.b as f32 * 0.114) as u8;
                    color.r = lerp_u8(gray, color.r, config.saturation);
                    color.g = lerp_u8(gray, color.g, config.saturation);
                    color.b = lerp_u8(gray, color.b, config.saturation);
                }

                // Adjust brightness
                if config.brightness != 1.0 {
                    color.r = ((color.r as f32 * config.brightness).min(255.0)) as u8;
                    color.g = ((color.g as f32 * config.brightness).min(255.0)) as u8;
                    color.b = ((color.b as f32 * config.brightness).min(255.0)) as u8;
                }

                // Apply tint
                if config.tint_opacity > 0 {
                    let tint_alpha = config.tint_opacity as f32 / 255.0;
                    let inv_alpha = 1.0 - tint_alpha;
                    color.r = (color.r as f32 * inv_alpha + config.tint_color.r as f32 * tint_alpha) as u8;
                    color.g = (color.g as f32 * inv_alpha + config.tint_color.g as f32 * tint_alpha) as u8;
                    color.b = (color.b as f32 * inv_alpha + config.tint_color.b as f32 * tint_alpha) as u8;
                }

                surface.set_pixel(x, y, color);
            }
        }
    }
}

/// Linear interpolation for u8
fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 * (1.0 - t) + b as f32 * t) as u8
}

// ============================================================================
// Window Transparency Settings
// ============================================================================

/// Window transparency settings
#[derive(Debug, Clone)]
pub struct WindowTransparency {
    /// Overall window opacity
    pub opacity: Opacity,
    /// Blend mode for compositing
    pub blend_mode: BlendMode,
    /// Enable drop shadow
    pub shadow_enabled: bool,
    /// Shadow configuration
    pub shadow_config: ShadowConfig,
    /// Enable glass effect
    pub glass_enabled: bool,
    /// Glass configuration
    pub glass_config: GlassConfig,
    /// Enable click-through (for fully transparent areas)
    pub click_through: bool,
    /// Alpha threshold for click-through (pixels with alpha below this are click-through)
    pub click_through_threshold: u8,
}

impl Default for WindowTransparency {
    fn default() -> Self {
        Self {
            opacity: Opacity::OPAQUE,
            blend_mode: BlendMode::Normal,
            shadow_enabled: true,
            shadow_config: ShadowConfig::default(),
            glass_enabled: false,
            glass_config: GlassConfig::default(),
            click_through: false,
            click_through_threshold: 10,
        }
    }
}

impl WindowTransparency {
    /// Create with specific opacity
    pub fn with_opacity(opacity: Opacity) -> Self {
        Self {
            opacity,
            ..Default::default()
        }
    }

    /// Create fully transparent (for overlay windows)
    pub fn transparent() -> Self {
        Self {
            opacity: Opacity::TRANSPARENT,
            shadow_enabled: false,
            ..Default::default()
        }
    }

    /// Create semi-transparent (50%)
    pub fn semi_transparent() -> Self {
        Self {
            opacity: Opacity::HALF,
            ..Default::default()
        }
    }

    /// Create glass effect window
    pub fn glass() -> Self {
        Self {
            glass_enabled: true,
            shadow_enabled: true,
            ..Default::default()
        }
    }

    /// Set opacity from percentage (0-100)
    pub fn set_opacity_percent(&mut self, percent: u8) {
        self.opacity = Opacity::from_percent(percent);
    }

    /// Enable/disable shadow
    pub fn set_shadow(&mut self, enabled: bool) {
        self.shadow_enabled = enabled;
    }

    /// Enable/disable glass effect
    pub fn set_glass(&mut self, enabled: bool) {
        self.glass_enabled = enabled;
    }

    /// Check if window is fully opaque
    pub fn is_opaque(&self) -> bool {
        self.opacity.is_opaque() && !self.glass_enabled
    }

    /// Check if click should pass through at given alpha
    pub fn should_click_through(&self, alpha: u8) -> bool {
        self.click_through && alpha < self.click_through_threshold
    }
}

// ============================================================================
// Compositing Helpers
// ============================================================================

/// Blit a surface with transparency settings
pub fn blit_with_transparency(
    dst: &mut Surface,
    src: &Surface,
    dst_x: isize,
    dst_y: isize,
    transparency: &WindowTransparency,
) {
    let src_width = src.width();
    let src_height = src.height();
    let dst_width = dst.width();
    let dst_height = dst.height();

    // Calculate visible region
    let start_x = dst_x.max(0) as usize;
    let start_y = dst_y.max(0) as usize;
    let end_x = ((dst_x + src_width as isize) as usize).min(dst_width);
    let end_y = ((dst_y + src_height as isize) as usize).min(dst_height);

    let src_start_x = if dst_x < 0 { (-dst_x) as usize } else { 0 };
    let src_start_y = if dst_y < 0 { (-dst_y) as usize } else { 0 };

    // Blit with blending
    for y in start_y..end_y {
        let src_y = y - start_y + src_start_y;
        for x in start_x..end_x {
            let src_x = x - start_x + src_start_x;

            if let Some(src_color) = src.get_pixel(src_x, src_y) {
                if let Some(dst_color) = dst.get_pixel(x, y) {
                    let blended = transparency.blend_mode.blend(src_color, dst_color, transparency.opacity);
                    dst.set_pixel(x, y, blended);
                }
            }
        }
    }
}

/// Apply transparency effects to a region (for glass effect)
pub fn apply_transparency_effects(
    dst: &mut Surface,
    x: isize,
    y: isize,
    width: usize,
    height: usize,
    transparency: &WindowTransparency,
) {
    if !transparency.glass_enabled {
        return;
    }

    // Extract region, apply glass effect, and put back
    let start_x = x.max(0) as usize;
    let start_y = y.max(0) as usize;
    let end_x = ((x + width as isize) as usize).min(dst.width());
    let end_y = ((y + height as isize) as usize).min(dst.height());

    // Create temporary surface for the region
    let region_width = end_x.saturating_sub(start_x);
    let region_height = end_y.saturating_sub(start_y);

    if region_width == 0 || region_height == 0 {
        return;
    }

    let mut region = Surface::new(region_width, region_height, super::surface::PixelFormat::Rgba8888);

    // Copy region to temp surface
    for ry in 0..region_height {
        for rx in 0..region_width {
            if let Some(c) = dst.get_pixel(start_x + rx, start_y + ry) {
                region.set_pixel(rx, ry, c);
            }
        }
    }

    // Apply glass effect
    apply_glass_effect(&mut region, &transparency.glass_config);

    // Copy back
    for ry in 0..region_height {
        for rx in 0..region_width {
            if let Some(c) = region.get_pixel(rx, ry) {
                dst.set_pixel(start_x + rx, start_y + ry, c);
            }
        }
    }
}

// ============================================================================
// Initialization
// ============================================================================

/// Initialize the transparency module
pub fn init() {
    // No runtime initialization needed
    crate::kprintln!("gui: transparency module initialized");
}

/// Format transparency info
pub fn format_transparency_info(t: &WindowTransparency) -> alloc::string::String {
    use alloc::format;
    format!(
        "Opacity: {}%, Shadow: {}, Glass: {}, ClickThrough: {}",
        t.opacity.as_percent(),
        if t.shadow_enabled { "on" } else { "off" },
        if t.glass_enabled { "on" } else { "off" },
        if t.click_through { "on" } else { "off" }
    )
}
