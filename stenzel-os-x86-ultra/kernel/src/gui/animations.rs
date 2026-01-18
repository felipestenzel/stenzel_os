//! UI Animation System
//!
//! Provides a comprehensive animation framework for the GUI subsystem including:
//! - Easing functions (linear, cubic, bounce, elastic, etc.)
//! - Property animations (position, size, opacity, color, scale, rotation)
//! - Animation timeline and sequencing
//! - Common preset animations (fade, slide, bounce, shake)

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::drivers::framebuffer::Color;
use super::window::WindowId;

/// Animation ID for tracking
pub type AnimationId = u64;

/// Global animation ID counter
static NEXT_ANIMATION_ID: AtomicU64 = AtomicU64::new(1);

fn next_animation_id() -> AnimationId {
    NEXT_ANIMATION_ID.fetch_add(1, Ordering::Relaxed)
}

/// Easing function type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EasingFunction {
    /// Linear interpolation (no easing)
    Linear,
    /// Quadratic ease-in (accelerate)
    EaseInQuad,
    /// Quadratic ease-out (decelerate)
    EaseOutQuad,
    /// Quadratic ease-in-out
    EaseInOutQuad,
    /// Cubic ease-in
    EaseInCubic,
    /// Cubic ease-out
    EaseOutCubic,
    /// Cubic ease-in-out
    EaseInOutCubic,
    /// Quartic ease-in
    EaseInQuart,
    /// Quartic ease-out
    EaseOutQuart,
    /// Quartic ease-in-out
    EaseInOutQuart,
    /// Quintic ease-in
    EaseInQuint,
    /// Quintic ease-out
    EaseOutQuint,
    /// Quintic ease-in-out
    EaseInOutQuint,
    /// Sinusoidal ease-in
    EaseInSine,
    /// Sinusoidal ease-out
    EaseOutSine,
    /// Sinusoidal ease-in-out
    EaseInOutSine,
    /// Exponential ease-in
    EaseInExpo,
    /// Exponential ease-out
    EaseOutExpo,
    /// Exponential ease-in-out
    EaseInOutExpo,
    /// Circular ease-in
    EaseInCirc,
    /// Circular ease-out
    EaseOutCirc,
    /// Circular ease-in-out
    EaseInOutCirc,
    /// Back ease-in (overshoot then accelerate)
    EaseInBack,
    /// Back ease-out (decelerate then overshoot)
    EaseOutBack,
    /// Back ease-in-out
    EaseInOutBack,
    /// Elastic ease-in (spring-like)
    EaseInElastic,
    /// Elastic ease-out
    EaseOutElastic,
    /// Elastic ease-in-out
    EaseInOutElastic,
    /// Bounce ease-in
    EaseInBounce,
    /// Bounce ease-out
    EaseOutBounce,
    /// Bounce ease-in-out
    EaseInOutBounce,
}

impl EasingFunction {
    /// Apply the easing function to a progress value (0.0 to 1.0)
    pub fn apply(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);

        match self {
            EasingFunction::Linear => t,

            // Quadratic
            EasingFunction::EaseInQuad => t * t,
            EasingFunction::EaseOutQuad => t * (2.0 - t),
            EasingFunction::EaseInOutQuad => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    -1.0 + (4.0 - 2.0 * t) * t
                }
            }

            // Cubic
            EasingFunction::EaseInCubic => t * t * t,
            EasingFunction::EaseOutCubic => {
                let t = t - 1.0;
                t * t * t + 1.0
            }
            EasingFunction::EaseInOutCubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    let t = 2.0 * t - 2.0;
                    (t * t * t + 2.0) / 2.0
                }
            }

            // Quartic
            EasingFunction::EaseInQuart => t * t * t * t,
            EasingFunction::EaseOutQuart => {
                let t = t - 1.0;
                1.0 - t * t * t * t
            }
            EasingFunction::EaseInOutQuart => {
                if t < 0.5 {
                    8.0 * t * t * t * t
                } else {
                    let t = t - 1.0;
                    1.0 - 8.0 * t * t * t * t
                }
            }

            // Quintic
            EasingFunction::EaseInQuint => t * t * t * t * t,
            EasingFunction::EaseOutQuint => {
                let t = t - 1.0;
                t * t * t * t * t + 1.0
            }
            EasingFunction::EaseInOutQuint => {
                if t < 0.5 {
                    16.0 * t * t * t * t * t
                } else {
                    let t = 2.0 * t - 2.0;
                    (t * t * t * t * t + 2.0) / 2.0
                }
            }

            // Sinusoidal
            EasingFunction::EaseInSine => 1.0 - cos_approx(t * PI / 2.0),
            EasingFunction::EaseOutSine => sin_approx(t * PI / 2.0),
            EasingFunction::EaseInOutSine => -(cos_approx(PI * t) - 1.0) / 2.0,

            // Exponential
            EasingFunction::EaseInExpo => {
                if t == 0.0 { 0.0 } else { pow_approx(2.0, 10.0 * t - 10.0) }
            }
            EasingFunction::EaseOutExpo => {
                if t == 1.0 { 1.0 } else { 1.0 - pow_approx(2.0, -10.0 * t) }
            }
            EasingFunction::EaseInOutExpo => {
                if t == 0.0 {
                    0.0
                } else if t == 1.0 {
                    1.0
                } else if t < 0.5 {
                    pow_approx(2.0, 20.0 * t - 10.0) / 2.0
                } else {
                    (2.0 - pow_approx(2.0, -20.0 * t + 10.0)) / 2.0
                }
            }

            // Circular
            EasingFunction::EaseInCirc => 1.0 - sqrt_approx(1.0 - t * t),
            EasingFunction::EaseOutCirc => sqrt_approx(1.0 - (t - 1.0) * (t - 1.0)),
            EasingFunction::EaseInOutCirc => {
                if t < 0.5 {
                    (1.0 - sqrt_approx(1.0 - 4.0 * t * t)) / 2.0
                } else {
                    let t = -2.0 * t + 2.0;
                    (sqrt_approx(1.0 - t * t) + 1.0) / 2.0
                }
            }

            // Back
            EasingFunction::EaseInBack => {
                let c1 = 1.70158;
                let c3 = c1 + 1.0;
                c3 * t * t * t - c1 * t * t
            }
            EasingFunction::EaseOutBack => {
                let c1 = 1.70158;
                let c3 = c1 + 1.0;
                let t = t - 1.0;
                1.0 + c3 * t * t * t + c1 * t * t
            }
            EasingFunction::EaseInOutBack => {
                let c1 = 1.70158;
                let c2 = c1 * 1.525;
                if t < 0.5 {
                    let t = 2.0 * t;
                    (t * t * ((c2 + 1.0) * t - c2)) / 2.0
                } else {
                    let t = 2.0 * t - 2.0;
                    (t * t * ((c2 + 1.0) * t + c2) + 2.0) / 2.0
                }
            }

            // Elastic
            EasingFunction::EaseInElastic => {
                if t == 0.0 {
                    0.0
                } else if t == 1.0 {
                    1.0
                } else {
                    let c4 = 2.0 * PI / 3.0;
                    -pow_approx(2.0, 10.0 * t - 10.0) * sin_approx((t * 10.0 - 10.75) * c4)
                }
            }
            EasingFunction::EaseOutElastic => {
                if t == 0.0 {
                    0.0
                } else if t == 1.0 {
                    1.0
                } else {
                    let c4 = 2.0 * PI / 3.0;
                    pow_approx(2.0, -10.0 * t) * sin_approx((t * 10.0 - 0.75) * c4) + 1.0
                }
            }
            EasingFunction::EaseInOutElastic => {
                if t == 0.0 {
                    0.0
                } else if t == 1.0 {
                    1.0
                } else {
                    let c5 = 2.0 * PI / 4.5;
                    if t < 0.5 {
                        -(pow_approx(2.0, 20.0 * t - 10.0) * sin_approx((20.0 * t - 11.125) * c5)) / 2.0
                    } else {
                        pow_approx(2.0, -20.0 * t + 10.0) * sin_approx((20.0 * t - 11.125) * c5) / 2.0 + 1.0
                    }
                }
            }

            // Bounce
            EasingFunction::EaseInBounce => 1.0 - EasingFunction::EaseOutBounce.apply(1.0 - t),
            EasingFunction::EaseOutBounce => {
                let n1 = 7.5625;
                let d1 = 2.75;
                if t < 1.0 / d1 {
                    n1 * t * t
                } else if t < 2.0 / d1 {
                    let t = t - 1.5 / d1;
                    n1 * t * t + 0.75
                } else if t < 2.5 / d1 {
                    let t = t - 2.25 / d1;
                    n1 * t * t + 0.9375
                } else {
                    let t = t - 2.625 / d1;
                    n1 * t * t + 0.984375
                }
            }
            EasingFunction::EaseInOutBounce => {
                if t < 0.5 {
                    (1.0 - EasingFunction::EaseOutBounce.apply(1.0 - 2.0 * t)) / 2.0
                } else {
                    (1.0 + EasingFunction::EaseOutBounce.apply(2.0 * t - 1.0)) / 2.0
                }
            }
        }
    }
}

// Math constants and helpers for no_std
const PI: f32 = 3.14159265358979323846;

/// Approximate sine using Taylor series
fn sin_approx(x: f32) -> f32 {
    // Normalize to [-PI, PI]
    let mut x = x % (2.0 * PI);
    if x > PI {
        x -= 2.0 * PI;
    } else if x < -PI {
        x += 2.0 * PI;
    }

    // Taylor series: sin(x) = x - x^3/3! + x^5/5! - x^7/7! + ...
    let x2 = x * x;
    let x3 = x2 * x;
    let x5 = x3 * x2;
    let x7 = x5 * x2;

    x - x3 / 6.0 + x5 / 120.0 - x7 / 5040.0
}

/// Approximate cosine using Taylor series
fn cos_approx(x: f32) -> f32 {
    sin_approx(x + PI / 2.0)
}

/// Approximate square root using Newton's method
fn sqrt_approx(x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }

    let mut guess = x / 2.0;
    for _ in 0..10 {
        guess = (guess + x / guess) / 2.0;
    }
    guess
}

/// Approximate power function for small integer-like exponents
fn pow_approx(base: f32, exp: f32) -> f32 {
    if exp == 0.0 {
        return 1.0;
    }
    if base == 0.0 {
        return 0.0;
    }

    // Use exp(exp * ln(base)) approximation
    let ln_base = ln_approx(base);
    exp_approx(exp * ln_base)
}

/// Approximate natural logarithm
fn ln_approx(x: f32) -> f32 {
    if x <= 0.0 {
        return f32::NEG_INFINITY;
    }

    // Normalize x to [1, 2) range
    let mut exp_adj = 0i32;
    let mut x = x;

    while x >= 2.0 {
        x /= 2.0;
        exp_adj += 1;
    }
    while x < 1.0 {
        x *= 2.0;
        exp_adj -= 1;
    }

    // For x in [1, 2), use Taylor series around 1
    let y = x - 1.0;
    let y2 = y * y;
    let y3 = y2 * y;
    let y4 = y3 * y;
    let y5 = y4 * y;

    let ln_x = y - y2/2.0 + y3/3.0 - y4/4.0 + y5/5.0;

    // Add back the exponent adjustment: ln(2) * exp_adj
    ln_x + (exp_adj as f32) * 0.693147
}

/// Approximate exponential function
fn exp_approx(x: f32) -> f32 {
    // Handle extreme values
    if x < -20.0 {
        return 0.0;
    }
    if x > 20.0 {
        return f32::MAX;
    }

    // Taylor series: e^x = 1 + x + x^2/2! + x^3/3! + ...
    let mut result = 1.0;
    let mut term = 1.0;

    for i in 1..20 {
        term *= x / (i as f32);
        result += term;
        if term.abs() < 1e-7 {
            break;
        }
    }

    result
}

/// Animation state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationState {
    /// Animation has not started
    Pending,
    /// Animation is running
    Running,
    /// Animation is paused
    Paused,
    /// Animation completed normally
    Completed,
    /// Animation was cancelled
    Cancelled,
}

/// Animation direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationDirection {
    /// Play forward only
    Normal,
    /// Play backward only
    Reverse,
    /// Alternate between forward and backward
    Alternate,
    /// Alternate starting with backward
    AlternateReverse,
}

/// Animation fill mode (what happens before/after animation)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationFillMode {
    /// No effect before or after
    None,
    /// Apply start values before animation
    Backwards,
    /// Retain end values after animation
    Forwards,
    /// Both backwards and forwards
    Both,
}

/// Property being animated
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnimatedProperty {
    /// X position
    X(f32, f32),
    /// Y position
    Y(f32, f32),
    /// Width
    Width(f32, f32),
    /// Height
    Height(f32, f32),
    /// Opacity (0.0 to 1.0)
    Opacity(f32, f32),
    /// Scale X factor
    ScaleX(f32, f32),
    /// Scale Y factor
    ScaleY(f32, f32),
    /// Rotation in degrees
    Rotation(f32, f32),
    /// Color (RGBA)
    Color(Color, Color),
    /// Background color
    BackgroundColor(Color, Color),
    /// Border radius
    BorderRadius(f32, f32),
    /// Custom property with name
    Custom { from: f32, to: f32 },
}

impl AnimatedProperty {
    /// Get the interpolated value at progress t
    pub fn interpolate(&self, t: f32) -> PropertyValue {
        match self {
            AnimatedProperty::X(from, to) => PropertyValue::Float(lerp(*from, *to, t)),
            AnimatedProperty::Y(from, to) => PropertyValue::Float(lerp(*from, *to, t)),
            AnimatedProperty::Width(from, to) => PropertyValue::Float(lerp(*from, *to, t)),
            AnimatedProperty::Height(from, to) => PropertyValue::Float(lerp(*from, *to, t)),
            AnimatedProperty::Opacity(from, to) => PropertyValue::Float(lerp(*from, *to, t)),
            AnimatedProperty::ScaleX(from, to) => PropertyValue::Float(lerp(*from, *to, t)),
            AnimatedProperty::ScaleY(from, to) => PropertyValue::Float(lerp(*from, *to, t)),
            AnimatedProperty::Rotation(from, to) => PropertyValue::Float(lerp(*from, *to, t)),
            AnimatedProperty::Color(from, to) => PropertyValue::Color(lerp_color(*from, *to, t)),
            AnimatedProperty::BackgroundColor(from, to) => PropertyValue::Color(lerp_color(*from, *to, t)),
            AnimatedProperty::BorderRadius(from, to) => PropertyValue::Float(lerp(*from, *to, t)),
            AnimatedProperty::Custom { from, to } => PropertyValue::Float(lerp(*from, *to, t)),
        }
    }

    /// Get property name
    pub fn name(&self) -> &'static str {
        match self {
            AnimatedProperty::X(_, _) => "x",
            AnimatedProperty::Y(_, _) => "y",
            AnimatedProperty::Width(_, _) => "width",
            AnimatedProperty::Height(_, _) => "height",
            AnimatedProperty::Opacity(_, _) => "opacity",
            AnimatedProperty::ScaleX(_, _) => "scaleX",
            AnimatedProperty::ScaleY(_, _) => "scaleY",
            AnimatedProperty::Rotation(_, _) => "rotation",
            AnimatedProperty::Color(_, _) => "color",
            AnimatedProperty::BackgroundColor(_, _) => "backgroundColor",
            AnimatedProperty::BorderRadius(_, _) => "borderRadius",
            AnimatedProperty::Custom { .. } => "custom",
        }
    }
}

/// Interpolated property value
#[derive(Debug, Clone, Copy)]
pub enum PropertyValue {
    Float(f32),
    Color(Color),
}

impl PropertyValue {
    pub fn as_float(&self) -> Option<f32> {
        match self {
            PropertyValue::Float(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_color(&self) -> Option<Color> {
        match self {
            PropertyValue::Color(c) => Some(*c),
            _ => None,
        }
    }
}

/// Linear interpolation
fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

/// Interpolate between two colors
fn lerp_color(from: Color, to: Color, t: f32) -> Color {
    Color {
        r: (lerp(from.r as f32, to.r as f32, t) as u8),
        g: (lerp(from.g as f32, to.g as f32, t) as u8),
        b: (lerp(from.b as f32, to.b as f32, t) as u8),
        a: (lerp(from.a as f32, to.a as f32, t) as u8),
    }
}

/// Animation configuration
#[derive(Debug, Clone)]
pub struct AnimationConfig {
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Delay before starting in milliseconds
    pub delay_ms: u64,
    /// Easing function
    pub easing: EasingFunction,
    /// Animation direction
    pub direction: AnimationDirection,
    /// Fill mode
    pub fill_mode: AnimationFillMode,
    /// Number of iterations (0 = infinite)
    pub iterations: u32,
}

impl Default for AnimationConfig {
    fn default() -> Self {
        Self {
            duration_ms: 300,
            delay_ms: 0,
            easing: EasingFunction::EaseOutCubic,
            direction: AnimationDirection::Normal,
            fill_mode: AnimationFillMode::Forwards,
            iterations: 1,
        }
    }
}

impl AnimationConfig {
    pub fn new(duration_ms: u64) -> Self {
        Self {
            duration_ms,
            ..Default::default()
        }
    }

    pub fn with_easing(mut self, easing: EasingFunction) -> Self {
        self.easing = easing;
        self
    }

    pub fn with_delay(mut self, delay_ms: u64) -> Self {
        self.delay_ms = delay_ms;
        self
    }

    pub fn with_direction(mut self, direction: AnimationDirection) -> Self {
        self.direction = direction;
        self
    }

    pub fn with_iterations(mut self, iterations: u32) -> Self {
        self.iterations = iterations;
        self
    }

    pub fn infinite(mut self) -> Self {
        self.iterations = 0;
        self
    }
}

/// A single animation
pub struct Animation {
    /// Unique ID
    pub id: AnimationId,
    /// Animation name (optional)
    pub name: Option<String>,
    /// Target window ID (if applicable)
    pub target: Option<WindowId>,
    /// Properties being animated
    pub properties: Vec<AnimatedProperty>,
    /// Configuration
    pub config: AnimationConfig,
    /// Current state
    pub state: AnimationState,
    /// Start time (ms since system start)
    pub start_time: u64,
    /// Current iteration
    pub current_iteration: u32,
    /// Callback when animation updates
    pub on_update: Option<Box<dyn Fn(AnimationId, &[PropertyValue]) + Send>>,
    /// Callback when animation completes
    pub on_complete: Option<Box<dyn Fn(AnimationId) + Send>>,
}

impl Animation {
    /// Create a new animation
    pub fn new(properties: Vec<AnimatedProperty>, config: AnimationConfig) -> Self {
        Self {
            id: next_animation_id(),
            name: None,
            target: None,
            properties,
            config,
            state: AnimationState::Pending,
            start_time: 0,
            current_iteration: 0,
            on_update: None,
            on_complete: None,
        }
    }

    /// Set animation name
    pub fn with_name(mut self, name: &str) -> Self {
        self.name = Some(String::from(name));
        self
    }

    /// Set target window
    pub fn with_target(mut self, target: WindowId) -> Self {
        self.target = Some(target);
        self
    }

    /// Set update callback
    pub fn on_update<F>(mut self, callback: F) -> Self
    where
        F: Fn(AnimationId, &[PropertyValue]) + Send + 'static,
    {
        self.on_update = Some(Box::new(callback));
        self
    }

    /// Set completion callback
    pub fn on_complete<F>(mut self, callback: F) -> Self
    where
        F: Fn(AnimationId) + Send + 'static,
    {
        self.on_complete = Some(Box::new(callback));
        self
    }

    /// Start the animation
    pub fn start(&mut self, current_time: u64) {
        self.start_time = current_time;
        self.state = AnimationState::Running;
        self.current_iteration = 0;
    }

    /// Pause the animation
    pub fn pause(&mut self) {
        if self.state == AnimationState::Running {
            self.state = AnimationState::Paused;
        }
    }

    /// Resume the animation
    pub fn resume(&mut self) {
        if self.state == AnimationState::Paused {
            self.state = AnimationState::Running;
        }
    }

    /// Cancel the animation
    pub fn cancel(&mut self) {
        self.state = AnimationState::Cancelled;
    }

    /// Update the animation and return current property values
    pub fn update(&mut self, current_time: u64) -> Option<Vec<PropertyValue>> {
        if self.state != AnimationState::Running {
            return None;
        }

        let elapsed = current_time.saturating_sub(self.start_time);

        // Handle delay
        if elapsed < self.config.delay_ms {
            return None;
        }

        let elapsed_after_delay = elapsed - self.config.delay_ms;
        let duration = self.config.duration_ms;

        // Calculate iteration and progress within iteration
        let total_elapsed_iterations = if duration > 0 {
            elapsed_after_delay / duration
        } else {
            0
        };

        // Check if we've completed all iterations
        if self.config.iterations > 0 && total_elapsed_iterations >= self.config.iterations as u64 {
            self.state = AnimationState::Completed;

            // Return final values
            let values: Vec<PropertyValue> = self.properties
                .iter()
                .map(|p| p.interpolate(1.0))
                .collect();

            if let Some(ref callback) = self.on_complete {
                callback(self.id);
            }

            return Some(values);
        }

        // Calculate progress within current iteration
        let iteration_progress = if duration > 0 {
            ((elapsed_after_delay % duration) as f32) / (duration as f32)
        } else {
            1.0
        };

        // Apply direction
        let directed_progress = match self.config.direction {
            AnimationDirection::Normal => iteration_progress,
            AnimationDirection::Reverse => 1.0 - iteration_progress,
            AnimationDirection::Alternate => {
                if total_elapsed_iterations % 2 == 0 {
                    iteration_progress
                } else {
                    1.0 - iteration_progress
                }
            }
            AnimationDirection::AlternateReverse => {
                if total_elapsed_iterations % 2 == 0 {
                    1.0 - iteration_progress
                } else {
                    iteration_progress
                }
            }
        };

        // Apply easing
        let eased_progress = self.config.easing.apply(directed_progress);

        // Interpolate properties
        let values: Vec<PropertyValue> = self.properties
            .iter()
            .map(|p| p.interpolate(eased_progress))
            .collect();

        // Call update callback
        if let Some(ref callback) = self.on_update {
            callback(self.id, &values);
        }

        Some(values)
    }

    /// Check if animation is active (running or paused)
    pub fn is_active(&self) -> bool {
        matches!(self.state, AnimationState::Running | AnimationState::Paused)
    }

    /// Check if animation is complete
    pub fn is_complete(&self) -> bool {
        matches!(self.state, AnimationState::Completed | AnimationState::Cancelled)
    }
}

/// Animation sequence (play animations one after another)
pub struct AnimationSequence {
    pub id: AnimationId,
    pub name: Option<String>,
    pub animations: Vec<Animation>,
    pub current_index: usize,
    pub state: AnimationState,
    pub loop_count: u32,
    pub current_loop: u32,
}

impl AnimationSequence {
    pub fn new(animations: Vec<Animation>) -> Self {
        Self {
            id: next_animation_id(),
            name: None,
            animations,
            current_index: 0,
            state: AnimationState::Pending,
            loop_count: 1,
            current_loop: 0,
        }
    }

    pub fn with_name(mut self, name: &str) -> Self {
        self.name = Some(String::from(name));
        self
    }

    pub fn with_loop(mut self, count: u32) -> Self {
        self.loop_count = count;
        self
    }

    pub fn start(&mut self, current_time: u64) {
        self.state = AnimationState::Running;
        self.current_index = 0;
        self.current_loop = 0;

        if let Some(anim) = self.animations.get_mut(0) {
            anim.start(current_time);
        }
    }

    pub fn update(&mut self, current_time: u64) -> Option<Vec<PropertyValue>> {
        if self.state != AnimationState::Running {
            return None;
        }

        if let Some(anim) = self.animations.get_mut(self.current_index) {
            let result = anim.update(current_time);

            if anim.is_complete() {
                self.current_index += 1;

                if self.current_index >= self.animations.len() {
                    self.current_loop += 1;

                    if self.loop_count > 0 && self.current_loop >= self.loop_count {
                        self.state = AnimationState::Completed;
                    } else {
                        self.current_index = 0;
                        // Reset all animations
                        for anim in &mut self.animations {
                            anim.state = AnimationState::Pending;
                        }
                        if let Some(anim) = self.animations.get_mut(0) {
                            anim.start(current_time);
                        }
                    }
                } else if let Some(next_anim) = self.animations.get_mut(self.current_index) {
                    next_anim.start(current_time);
                }
            }

            result
        } else {
            self.state = AnimationState::Completed;
            None
        }
    }
}

/// Animation group (play animations in parallel)
pub struct AnimationGroup {
    pub id: AnimationId,
    pub name: Option<String>,
    pub animations: Vec<Animation>,
    pub state: AnimationState,
}

impl AnimationGroup {
    pub fn new(animations: Vec<Animation>) -> Self {
        Self {
            id: next_animation_id(),
            name: None,
            animations,
            state: AnimationState::Pending,
        }
    }

    pub fn with_name(mut self, name: &str) -> Self {
        self.name = Some(String::from(name));
        self
    }

    pub fn start(&mut self, current_time: u64) {
        self.state = AnimationState::Running;
        for anim in &mut self.animations {
            anim.start(current_time);
        }
    }

    pub fn update(&mut self, current_time: u64) -> Vec<Option<Vec<PropertyValue>>> {
        if self.state != AnimationState::Running {
            return vec![];
        }

        let mut all_complete = true;
        let results: Vec<Option<Vec<PropertyValue>>> = self.animations
            .iter_mut()
            .map(|anim| {
                let result = anim.update(current_time);
                if !anim.is_complete() {
                    all_complete = false;
                }
                result
            })
            .collect();

        if all_complete {
            self.state = AnimationState::Completed;
        }

        results
    }
}

/// Common preset animations
pub mod presets {
    use super::*;

    /// Fade in animation
    pub fn fade_in(duration_ms: u64) -> Animation {
        Animation::new(
            vec![AnimatedProperty::Opacity(0.0, 1.0)],
            AnimationConfig::new(duration_ms).with_easing(EasingFunction::EaseOutCubic),
        )
    }

    /// Fade out animation
    pub fn fade_out(duration_ms: u64) -> Animation {
        Animation::new(
            vec![AnimatedProperty::Opacity(1.0, 0.0)],
            AnimationConfig::new(duration_ms).with_easing(EasingFunction::EaseInCubic),
        )
    }

    /// Slide in from left
    pub fn slide_in_left(duration_ms: u64, distance: f32) -> Animation {
        Animation::new(
            vec![
                AnimatedProperty::X(-distance, 0.0),
                AnimatedProperty::Opacity(0.0, 1.0),
            ],
            AnimationConfig::new(duration_ms).with_easing(EasingFunction::EaseOutCubic),
        )
    }

    /// Slide in from right
    pub fn slide_in_right(duration_ms: u64, distance: f32) -> Animation {
        Animation::new(
            vec![
                AnimatedProperty::X(distance, 0.0),
                AnimatedProperty::Opacity(0.0, 1.0),
            ],
            AnimationConfig::new(duration_ms).with_easing(EasingFunction::EaseOutCubic),
        )
    }

    /// Slide in from top
    pub fn slide_in_top(duration_ms: u64, distance: f32) -> Animation {
        Animation::new(
            vec![
                AnimatedProperty::Y(-distance, 0.0),
                AnimatedProperty::Opacity(0.0, 1.0),
            ],
            AnimationConfig::new(duration_ms).with_easing(EasingFunction::EaseOutCubic),
        )
    }

    /// Slide in from bottom
    pub fn slide_in_bottom(duration_ms: u64, distance: f32) -> Animation {
        Animation::new(
            vec![
                AnimatedProperty::Y(distance, 0.0),
                AnimatedProperty::Opacity(0.0, 1.0),
            ],
            AnimationConfig::new(duration_ms).with_easing(EasingFunction::EaseOutCubic),
        )
    }

    /// Scale in (zoom in)
    pub fn scale_in(duration_ms: u64) -> Animation {
        Animation::new(
            vec![
                AnimatedProperty::ScaleX(0.0, 1.0),
                AnimatedProperty::ScaleY(0.0, 1.0),
                AnimatedProperty::Opacity(0.0, 1.0),
            ],
            AnimationConfig::new(duration_ms).with_easing(EasingFunction::EaseOutBack),
        )
    }

    /// Scale out (zoom out)
    pub fn scale_out(duration_ms: u64) -> Animation {
        Animation::new(
            vec![
                AnimatedProperty::ScaleX(1.0, 0.0),
                AnimatedProperty::ScaleY(1.0, 0.0),
                AnimatedProperty::Opacity(1.0, 0.0),
            ],
            AnimationConfig::new(duration_ms).with_easing(EasingFunction::EaseInBack),
        )
    }

    /// Bounce animation
    pub fn bounce(duration_ms: u64, bounce_height: f32) -> Animation {
        Animation::new(
            vec![AnimatedProperty::Y(0.0, -bounce_height)],
            AnimationConfig::new(duration_ms)
                .with_easing(EasingFunction::EaseOutBounce)
                .with_direction(AnimationDirection::Alternate)
                .with_iterations(2),
        )
    }

    /// Shake animation (horizontal)
    pub fn shake(duration_ms: u64, intensity: f32) -> Animation {
        Animation::new(
            vec![AnimatedProperty::X(-intensity, intensity)],
            AnimationConfig::new(duration_ms)
                .with_easing(EasingFunction::EaseInOutQuad)
                .with_direction(AnimationDirection::Alternate)
                .with_iterations(6),
        )
    }

    /// Pulse animation (scale up and down)
    pub fn pulse(duration_ms: u64) -> Animation {
        Animation::new(
            vec![
                AnimatedProperty::ScaleX(1.0, 1.1),
                AnimatedProperty::ScaleY(1.0, 1.1),
            ],
            AnimationConfig::new(duration_ms)
                .with_easing(EasingFunction::EaseInOutQuad)
                .with_direction(AnimationDirection::Alternate)
                .with_iterations(0), // Infinite
        )
    }

    /// Rotate animation
    pub fn rotate(duration_ms: u64, degrees: f32) -> Animation {
        Animation::new(
            vec![AnimatedProperty::Rotation(0.0, degrees)],
            AnimationConfig::new(duration_ms).with_easing(EasingFunction::Linear),
        )
    }

    /// Spin animation (continuous rotation)
    pub fn spin(duration_ms: u64) -> Animation {
        Animation::new(
            vec![AnimatedProperty::Rotation(0.0, 360.0)],
            AnimationConfig::new(duration_ms)
                .with_easing(EasingFunction::Linear)
                .infinite(),
        )
    }

    /// Color transition
    pub fn color_transition(duration_ms: u64, from: Color, to: Color) -> Animation {
        Animation::new(
            vec![AnimatedProperty::Color(from, to)],
            AnimationConfig::new(duration_ms).with_easing(EasingFunction::EaseInOutQuad),
        )
    }

    /// Flash animation (opacity flash)
    pub fn flash(duration_ms: u64) -> Animation {
        Animation::new(
            vec![AnimatedProperty::Opacity(1.0, 0.0)],
            AnimationConfig::new(duration_ms)
                .with_easing(EasingFunction::Linear)
                .with_direction(AnimationDirection::Alternate)
                .with_iterations(4),
        )
    }

    /// Attention-grabbing wobble
    pub fn wobble(duration_ms: u64) -> Animation {
        Animation::new(
            vec![
                AnimatedProperty::Rotation(-5.0, 5.0),
                AnimatedProperty::X(-10.0, 10.0),
            ],
            AnimationConfig::new(duration_ms)
                .with_easing(EasingFunction::EaseInOutQuad)
                .with_direction(AnimationDirection::Alternate)
                .with_iterations(6),
        )
    }

    /// Elastic entrance
    pub fn elastic_in(duration_ms: u64) -> Animation {
        Animation::new(
            vec![
                AnimatedProperty::ScaleX(0.0, 1.0),
                AnimatedProperty::ScaleY(0.0, 1.0),
            ],
            AnimationConfig::new(duration_ms).with_easing(EasingFunction::EaseOutElastic),
        )
    }

    /// Window minimize animation
    pub fn minimize(duration_ms: u64, target_y: f32, target_height: f32) -> Animation {
        Animation::new(
            vec![
                AnimatedProperty::Y(0.0, target_y),
                AnimatedProperty::Height(1.0, target_height),
                AnimatedProperty::Opacity(1.0, 0.5),
            ],
            AnimationConfig::new(duration_ms).with_easing(EasingFunction::EaseInCubic),
        )
    }

    /// Window maximize animation
    pub fn maximize(duration_ms: u64, from_y: f32, from_height: f32) -> Animation {
        Animation::new(
            vec![
                AnimatedProperty::Y(from_y, 0.0),
                AnimatedProperty::Height(from_height, 1.0),
                AnimatedProperty::Opacity(0.5, 1.0),
            ],
            AnimationConfig::new(duration_ms).with_easing(EasingFunction::EaseOutCubic),
        )
    }
}

/// Animation manager for tracking and updating animations
pub struct AnimationManager {
    animations: BTreeMap<AnimationId, Animation>,
    sequences: BTreeMap<AnimationId, AnimationSequence>,
    groups: BTreeMap<AnimationId, AnimationGroup>,
}

impl AnimationManager {
    pub fn new() -> Self {
        Self {
            animations: BTreeMap::new(),
            sequences: BTreeMap::new(),
            groups: BTreeMap::new(),
        }
    }

    /// Add and start an animation
    pub fn play(&mut self, animation: Animation, current_time: u64) -> AnimationId {
        let id = animation.id;
        let mut anim = animation;
        anim.start(current_time);
        self.animations.insert(id, anim);
        id
    }

    /// Add and start a sequence
    pub fn play_sequence(&mut self, sequence: AnimationSequence, current_time: u64) -> AnimationId {
        let id = sequence.id;
        let mut seq = sequence;
        seq.start(current_time);
        self.sequences.insert(id, seq);
        id
    }

    /// Add and start a group
    pub fn play_group(&mut self, group: AnimationGroup, current_time: u64) -> AnimationId {
        let id = group.id;
        let mut grp = group;
        grp.start(current_time);
        self.groups.insert(id, grp);
        id
    }

    /// Cancel an animation
    pub fn cancel(&mut self, id: AnimationId) {
        if let Some(anim) = self.animations.get_mut(&id) {
            anim.cancel();
        }
    }

    /// Pause an animation
    pub fn pause(&mut self, id: AnimationId) {
        if let Some(anim) = self.animations.get_mut(&id) {
            anim.pause();
        }
    }

    /// Resume an animation
    pub fn resume(&mut self, id: AnimationId) {
        if let Some(anim) = self.animations.get_mut(&id) {
            anim.resume();
        }
    }

    /// Update all animations
    pub fn update(&mut self, current_time: u64) {
        // Update individual animations
        let mut completed_ids = Vec::new();

        for (id, anim) in &mut self.animations {
            anim.update(current_time);
            if anim.is_complete() {
                completed_ids.push(*id);
            }
        }

        // Remove completed animations
        for id in completed_ids {
            self.animations.remove(&id);
        }

        // Update sequences
        let mut completed_seqs = Vec::new();

        for (id, seq) in &mut self.sequences {
            seq.update(current_time);
            if seq.state == AnimationState::Completed {
                completed_seqs.push(*id);
            }
        }

        for id in completed_seqs {
            self.sequences.remove(&id);
        }

        // Update groups
        let mut completed_groups = Vec::new();

        for (id, group) in &mut self.groups {
            group.update(current_time);
            if group.state == AnimationState::Completed {
                completed_groups.push(*id);
            }
        }

        for id in completed_groups {
            self.groups.remove(&id);
        }
    }

    /// Get animation by ID
    pub fn get(&self, id: AnimationId) -> Option<&Animation> {
        self.animations.get(&id)
    }

    /// Get animation state
    pub fn get_state(&self, id: AnimationId) -> Option<AnimationState> {
        self.animations.get(&id).map(|a| a.state)
    }

    /// Check if animation is active
    pub fn is_active(&self, id: AnimationId) -> bool {
        self.animations.get(&id).map(|a| a.is_active()).unwrap_or(false)
    }

    /// Get count of active animations
    pub fn active_count(&self) -> usize {
        self.animations.values().filter(|a| a.is_active()).count()
            + self.sequences.values().filter(|s| s.state == AnimationState::Running).count()
            + self.groups.values().filter(|g| g.state == AnimationState::Running).count()
    }

    /// Cancel all animations
    pub fn cancel_all(&mut self) {
        for anim in self.animations.values_mut() {
            anim.cancel();
        }
        self.animations.clear();
        self.sequences.clear();
        self.groups.clear();
    }

    /// Cancel animations for a specific window
    pub fn cancel_for_window(&mut self, window_id: WindowId) {
        let ids_to_remove: Vec<AnimationId> = self.animations
            .iter()
            .filter(|(_, a)| a.target == Some(window_id))
            .map(|(id, _)| *id)
            .collect();

        for id in ids_to_remove {
            self.animations.remove(&id);
        }
    }
}

impl Default for AnimationManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global animation manager
static mut ANIMATION_MANAGER: Option<AnimationManager> = None;

/// Initialize the animation system
pub fn init() {
    unsafe {
        ANIMATION_MANAGER = Some(AnimationManager::new());
    }
    crate::kprintln!("animations: initialized");
}

/// Get the global animation manager
pub fn manager() -> &'static mut AnimationManager {
    unsafe {
        ANIMATION_MANAGER.as_mut().expect("Animation system not initialized")
    }
}

/// Play an animation
pub fn play(animation: Animation, current_time: u64) -> AnimationId {
    manager().play(animation, current_time)
}

/// Play a sequence
pub fn play_sequence(sequence: AnimationSequence, current_time: u64) -> AnimationId {
    manager().play_sequence(sequence, current_time)
}

/// Play a group
pub fn play_group(group: AnimationGroup, current_time: u64) -> AnimationId {
    manager().play_group(group, current_time)
}

/// Update all animations
pub fn update(current_time: u64) {
    manager().update(current_time);
}

/// Cancel an animation
pub fn cancel(id: AnimationId) {
    manager().cancel(id);
}

/// Get active animation count
pub fn active_count() -> usize {
    manager().active_count()
}

/// Format animation system status
pub fn format_status() -> String {
    let mgr = manager();
    alloc::format!(
        "Animation System:\n  Active animations: {}\n  Active sequences: {}\n  Active groups: {}",
        mgr.animations.values().filter(|a| a.is_active()).count(),
        mgr.sequences.values().filter(|s| s.state == AnimationState::Running).count(),
        mgr.groups.values().filter(|g| g.state == AnimationState::Running).count(),
    )
}
