//! Reduce Motion Accessibility Feature
//!
//! Provides motion reduction settings for users who:
//! - Have vestibular disorders
//! - Experience motion sickness
//! - Prefer less visual distraction
//! - Have photosensitive epilepsy
//!
//! Features:
//! - Disable or reduce animations
//! - Replace animations with crossfades
//! - Control parallax scrolling
//! - Reduce auto-playing content

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;

use crate::sync::IrqSafeMutex;

/// Animation type that can be reduced or disabled
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationType {
    /// Window open/close animations
    WindowTransitions,
    /// Menu slide/fade animations
    MenuAnimations,
    /// Scroll momentum/smoothing
    ScrollAnimations,
    /// Button hover/press effects
    ButtonEffects,
    /// Progress bar animations
    ProgressBars,
    /// Loading spinners
    Spinners,
    /// Parallax scrolling effects
    ParallaxScrolling,
    /// Hover zoom effects
    ZoomOnHover,
    /// Background animations
    BackgroundMotion,
    /// Page transitions
    PageTransitions,
    /// Notification slide-in
    NotificationAnimations,
    /// Cursor trail effects
    CursorEffects,
    /// Window minimize/maximize
    WindowMinMax,
    /// Workspace switching
    WorkspaceTransitions,
    /// Tooltip fade
    TooltipAnimations,
    /// Dropdown expand
    DropdownAnimations,
    /// Tab switching
    TabTransitions,
    /// Icon bounce/wiggle
    IconAnimations,
    /// Auto-playing videos
    AutoPlayVideo,
    /// Auto-playing GIFs
    AutoPlayGifs,
    /// Carousel/slideshow
    Carousels,
    /// Text reveal animations
    TextAnimations,
    /// All animations
    All,
}

impl AnimationType {
    /// Get animation type name
    pub fn name(&self) -> &'static str {
        match self {
            AnimationType::WindowTransitions => "Window Transitions",
            AnimationType::MenuAnimations => "Menu Animations",
            AnimationType::ScrollAnimations => "Scroll Animations",
            AnimationType::ButtonEffects => "Button Effects",
            AnimationType::ProgressBars => "Progress Bars",
            AnimationType::Spinners => "Spinners",
            AnimationType::ParallaxScrolling => "Parallax Scrolling",
            AnimationType::ZoomOnHover => "Zoom on Hover",
            AnimationType::BackgroundMotion => "Background Motion",
            AnimationType::PageTransitions => "Page Transitions",
            AnimationType::NotificationAnimations => "Notification Animations",
            AnimationType::CursorEffects => "Cursor Effects",
            AnimationType::WindowMinMax => "Window Minimize/Maximize",
            AnimationType::WorkspaceTransitions => "Workspace Transitions",
            AnimationType::TooltipAnimations => "Tooltip Animations",
            AnimationType::DropdownAnimations => "Dropdown Animations",
            AnimationType::TabTransitions => "Tab Transitions",
            AnimationType::IconAnimations => "Icon Animations",
            AnimationType::AutoPlayVideo => "Auto-Play Video",
            AnimationType::AutoPlayGifs => "Auto-Play GIFs",
            AnimationType::Carousels => "Carousels",
            AnimationType::TextAnimations => "Text Animations",
            AnimationType::All => "All Animations",
        }
    }

    /// Get animation type description
    pub fn description(&self) -> &'static str {
        match self {
            AnimationType::WindowTransitions => "Animations when windows open or close",
            AnimationType::MenuAnimations => "Slide and fade effects on menus",
            AnimationType::ScrollAnimations => "Smooth scrolling and momentum",
            AnimationType::ButtonEffects => "Hover and press visual effects on buttons",
            AnimationType::ProgressBars => "Animated progress bar fill effects",
            AnimationType::Spinners => "Loading spinner animations",
            AnimationType::ParallaxScrolling => "Layered scrolling depth effects",
            AnimationType::ZoomOnHover => "Elements that zoom when hovered",
            AnimationType::BackgroundMotion => "Moving or animated backgrounds",
            AnimationType::PageTransitions => "Animations between pages",
            AnimationType::NotificationAnimations => "Notifications sliding in/out",
            AnimationType::CursorEffects => "Mouse cursor trails and effects",
            AnimationType::WindowMinMax => "Minimize and maximize animations",
            AnimationType::WorkspaceTransitions => "Virtual desktop switching effects",
            AnimationType::TooltipAnimations => "Tooltip fade in/out",
            AnimationType::DropdownAnimations => "Dropdown expand/collapse effects",
            AnimationType::TabTransitions => "Tab switching animations",
            AnimationType::IconAnimations => "Bouncing or wiggling icons",
            AnimationType::AutoPlayVideo => "Videos that play automatically",
            AnimationType::AutoPlayGifs => "Animated GIFs that loop",
            AnimationType::Carousels => "Auto-advancing slideshows",
            AnimationType::TextAnimations => "Text reveal and typing effects",
            AnimationType::All => "All types of animations",
        }
    }

    /// Check if this is a motion-based animation (vs just visual effect)
    pub fn is_motion(&self) -> bool {
        matches!(
            self,
            AnimationType::WindowTransitions |
            AnimationType::MenuAnimations |
            AnimationType::ScrollAnimations |
            AnimationType::ParallaxScrolling |
            AnimationType::PageTransitions |
            AnimationType::NotificationAnimations |
            AnimationType::WindowMinMax |
            AnimationType::WorkspaceTransitions |
            AnimationType::DropdownAnimations |
            AnimationType::TabTransitions |
            AnimationType::Carousels |
            AnimationType::All
        )
    }

    /// Get all animation types (excluding All)
    pub fn all_types() -> Vec<AnimationType> {
        alloc::vec![
            AnimationType::WindowTransitions,
            AnimationType::MenuAnimations,
            AnimationType::ScrollAnimations,
            AnimationType::ButtonEffects,
            AnimationType::ProgressBars,
            AnimationType::Spinners,
            AnimationType::ParallaxScrolling,
            AnimationType::ZoomOnHover,
            AnimationType::BackgroundMotion,
            AnimationType::PageTransitions,
            AnimationType::NotificationAnimations,
            AnimationType::CursorEffects,
            AnimationType::WindowMinMax,
            AnimationType::WorkspaceTransitions,
            AnimationType::TooltipAnimations,
            AnimationType::DropdownAnimations,
            AnimationType::TabTransitions,
            AnimationType::IconAnimations,
            AnimationType::AutoPlayVideo,
            AnimationType::AutoPlayGifs,
            AnimationType::Carousels,
            AnimationType::TextAnimations,
        ]
    }
}

/// How to handle reduced motion
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionReduction {
    /// No reduction - play normally
    None,
    /// Reduce animation duration
    Reduced,
    /// Replace with simple crossfade
    Crossfade,
    /// Remove completely (instant transition)
    Disabled,
}

impl MotionReduction {
    /// Get reduction mode name
    pub fn name(&self) -> &'static str {
        match self {
            MotionReduction::None => "None",
            MotionReduction::Reduced => "Reduced",
            MotionReduction::Crossfade => "Crossfade",
            MotionReduction::Disabled => "Disabled",
        }
    }

    /// Get reduction mode description
    pub fn description(&self) -> &'static str {
        match self {
            MotionReduction::None => "Play animations normally",
            MotionReduction::Reduced => "Shorten animation duration",
            MotionReduction::Crossfade => "Replace with simple fade effect",
            MotionReduction::Disabled => "Remove animation completely",
        }
    }

    /// Get duration multiplier for this reduction mode
    pub fn duration_multiplier(&self) -> f32 {
        match self {
            MotionReduction::None => 1.0,
            MotionReduction::Reduced => 0.3, // 30% of original duration
            MotionReduction::Crossfade => 0.5, // Half duration for crossfade
            MotionReduction::Disabled => 0.0, // Instant
        }
    }
}

/// Animation timing information
#[derive(Debug, Clone, Copy)]
pub struct AnimationTiming {
    /// Original duration in milliseconds
    pub original_duration_ms: u32,
    /// Effective duration after reduction
    pub effective_duration_ms: u32,
    /// Whether animation is disabled
    pub disabled: bool,
    /// Whether to use crossfade instead
    pub use_crossfade: bool,
}

impl AnimationTiming {
    /// Create new timing with reduction applied
    pub fn new(original_ms: u32, reduction: MotionReduction) -> Self {
        match reduction {
            MotionReduction::None => Self {
                original_duration_ms: original_ms,
                effective_duration_ms: original_ms,
                disabled: false,
                use_crossfade: false,
            },
            MotionReduction::Reduced => Self {
                original_duration_ms: original_ms,
                effective_duration_ms: ((original_ms as f32) * 0.3) as u32,
                disabled: false,
                use_crossfade: false,
            },
            MotionReduction::Crossfade => Self {
                original_duration_ms: original_ms,
                effective_duration_ms: ((original_ms as f32) * 0.5) as u32,
                disabled: false,
                use_crossfade: true,
            },
            MotionReduction::Disabled => Self {
                original_duration_ms: original_ms,
                effective_duration_ms: 0,
                disabled: true,
                use_crossfade: false,
            },
        }
    }

    /// Check if animation should be skipped
    pub fn should_skip(&self) -> bool {
        self.disabled || self.effective_duration_ms == 0
    }
}

/// Reduce motion configuration
#[derive(Debug, Clone)]
pub struct ReduceMotionConfig {
    /// Whether reduce motion is enabled
    pub enabled: bool,
    /// Global reduction mode
    pub global_mode: MotionReduction,
    /// Per-animation type overrides (None = use global)
    pub overrides: [(AnimationType, Option<MotionReduction>); 22],
    /// Minimum allowed animation duration (ms)
    pub min_duration_ms: u32,
    /// Maximum allowed animation duration (ms)
    pub max_duration_ms: u32,
    /// Disable parallax completely
    pub disable_parallax: bool,
    /// Pause auto-playing media
    pub pause_auto_play: bool,
    /// Disable blinking text/cursors
    pub disable_blinking: bool,
    /// Disable scrolling text (marquee)
    pub disable_scrolling_text: bool,
    /// Disable zoom effects
    pub disable_zoom_effects: bool,
    /// Reduce transparency/blur effects
    pub reduce_transparency: bool,
    /// Use instant show/hide instead of fade
    pub instant_visibility: bool,
    /// Prefer static backgrounds
    pub prefer_static_backgrounds: bool,
    /// Apply to system UI only (not apps)
    pub system_ui_only: bool,
}

impl Default for ReduceMotionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            global_mode: MotionReduction::Reduced,
            overrides: [
                (AnimationType::WindowTransitions, None),
                (AnimationType::MenuAnimations, None),
                (AnimationType::ScrollAnimations, None),
                (AnimationType::ButtonEffects, None),
                (AnimationType::ProgressBars, None),
                (AnimationType::Spinners, None),
                (AnimationType::ParallaxScrolling, Some(MotionReduction::Disabled)),
                (AnimationType::ZoomOnHover, None),
                (AnimationType::BackgroundMotion, Some(MotionReduction::Disabled)),
                (AnimationType::PageTransitions, None),
                (AnimationType::NotificationAnimations, None),
                (AnimationType::CursorEffects, None),
                (AnimationType::WindowMinMax, None),
                (AnimationType::WorkspaceTransitions, None),
                (AnimationType::TooltipAnimations, None),
                (AnimationType::DropdownAnimations, None),
                (AnimationType::TabTransitions, None),
                (AnimationType::IconAnimations, None),
                (AnimationType::AutoPlayVideo, Some(MotionReduction::Disabled)),
                (AnimationType::AutoPlayGifs, Some(MotionReduction::Disabled)),
                (AnimationType::Carousels, Some(MotionReduction::Disabled)),
                (AnimationType::TextAnimations, None),
            ],
            min_duration_ms: 0,
            max_duration_ms: 300,
            disable_parallax: true,
            pause_auto_play: true,
            disable_blinking: false,
            disable_scrolling_text: true,
            disable_zoom_effects: false,
            reduce_transparency: false,
            instant_visibility: false,
            prefer_static_backgrounds: true,
            system_ui_only: false,
        }
    }
}

impl ReduceMotionConfig {
    /// Create a minimal motion config (maximum reduction)
    pub fn minimal_motion() -> Self {
        Self {
            enabled: true,
            global_mode: MotionReduction::Disabled,
            overrides: [
                (AnimationType::WindowTransitions, Some(MotionReduction::Disabled)),
                (AnimationType::MenuAnimations, Some(MotionReduction::Disabled)),
                (AnimationType::ScrollAnimations, Some(MotionReduction::Disabled)),
                (AnimationType::ButtonEffects, Some(MotionReduction::Disabled)),
                (AnimationType::ProgressBars, Some(MotionReduction::Disabled)),
                (AnimationType::Spinners, Some(MotionReduction::Crossfade)), // Keep some feedback
                (AnimationType::ParallaxScrolling, Some(MotionReduction::Disabled)),
                (AnimationType::ZoomOnHover, Some(MotionReduction::Disabled)),
                (AnimationType::BackgroundMotion, Some(MotionReduction::Disabled)),
                (AnimationType::PageTransitions, Some(MotionReduction::Disabled)),
                (AnimationType::NotificationAnimations, Some(MotionReduction::Disabled)),
                (AnimationType::CursorEffects, Some(MotionReduction::Disabled)),
                (AnimationType::WindowMinMax, Some(MotionReduction::Disabled)),
                (AnimationType::WorkspaceTransitions, Some(MotionReduction::Disabled)),
                (AnimationType::TooltipAnimations, Some(MotionReduction::Disabled)),
                (AnimationType::DropdownAnimations, Some(MotionReduction::Disabled)),
                (AnimationType::TabTransitions, Some(MotionReduction::Disabled)),
                (AnimationType::IconAnimations, Some(MotionReduction::Disabled)),
                (AnimationType::AutoPlayVideo, Some(MotionReduction::Disabled)),
                (AnimationType::AutoPlayGifs, Some(MotionReduction::Disabled)),
                (AnimationType::Carousels, Some(MotionReduction::Disabled)),
                (AnimationType::TextAnimations, Some(MotionReduction::Disabled)),
            ],
            min_duration_ms: 0,
            max_duration_ms: 0,
            disable_parallax: true,
            pause_auto_play: true,
            disable_blinking: true,
            disable_scrolling_text: true,
            disable_zoom_effects: true,
            reduce_transparency: true,
            instant_visibility: true,
            prefer_static_backgrounds: true,
            system_ui_only: false,
        }
    }

    /// Create a reduced motion config (moderate reduction)
    pub fn reduced_motion() -> Self {
        Self {
            enabled: true,
            global_mode: MotionReduction::Reduced,
            ..Default::default()
        }
    }

    /// Create a crossfade-only config
    pub fn crossfade_only() -> Self {
        let mut config = Self::default();
        config.enabled = true;
        config.global_mode = MotionReduction::Crossfade;
        config
    }
}

/// Reduce motion statistics
#[derive(Debug, Clone, Default)]
pub struct ReduceMotionStats {
    /// Animations reduced
    pub animations_reduced: u64,
    /// Animations disabled
    pub animations_disabled: u64,
    /// Animations converted to crossfade
    pub animations_crossfaded: u64,
    /// Auto-play media paused
    pub auto_play_paused: u64,
    /// Time saved from reduced animations (ms)
    pub time_saved_ms: u64,
    /// Session start timestamp
    pub session_start_ms: u64,
}

/// Animation request from UI components
#[derive(Debug, Clone)]
pub struct AnimationRequest {
    /// Type of animation
    pub animation_type: AnimationType,
    /// Requested duration (ms)
    pub duration_ms: u32,
    /// Whether animation is essential (progress feedback, etc.)
    pub essential: bool,
    /// Source component identifier
    pub source: String,
}

impl AnimationRequest {
    /// Create a new animation request
    pub fn new(animation_type: AnimationType, duration_ms: u32) -> Self {
        Self {
            animation_type,
            duration_ms,
            essential: false,
            source: String::new(),
        }
    }

    /// Mark as essential animation
    pub fn essential(mut self) -> Self {
        self.essential = true;
        self
    }

    /// Set source component
    pub fn from_source(mut self, source: &str) -> Self {
        self.source = String::from(source);
        self
    }
}

/// Reduce Motion Manager
pub struct ReduceMotionManager {
    /// Configuration
    config: ReduceMotionConfig,
    /// Statistics
    stats: ReduceMotionStats,
    /// Callback for motion preference queries
    on_motion_query: Option<fn(AnimationType) -> bool>,
    /// Callback for animation timing calculation
    on_timing_calc: Option<fn(&AnimationRequest, &AnimationTiming)>,
}

impl ReduceMotionManager {
    /// Create a new reduce motion manager
    pub fn new() -> Self {
        Self {
            config: ReduceMotionConfig::default(),
            stats: ReduceMotionStats::default(),
            on_motion_query: None,
            on_timing_calc: None,
        }
    }

    /// Initialize
    pub fn init(&mut self) {
        self.stats.session_start_ms = crate::time::uptime_ms();
        crate::kprintln!("[reduce_motion] Reduce motion manager initialized");
    }

    /// Enable reduce motion
    pub fn enable(&mut self) {
        self.config.enabled = true;
        crate::kprintln!("[reduce_motion] Reduce motion enabled (mode: {})",
            self.config.global_mode.name());
    }

    /// Disable reduce motion
    pub fn disable(&mut self) {
        self.config.enabled = false;
        crate::kprintln!("[reduce_motion] Reduce motion disabled");
    }

    /// Toggle reduce motion
    pub fn toggle(&mut self) {
        if self.config.enabled {
            self.disable();
        } else {
            self.enable();
        }
    }

    /// Check if reduce motion is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Set global reduction mode
    pub fn set_mode(&mut self, mode: MotionReduction) {
        self.config.global_mode = mode;
        crate::kprintln!("[reduce_motion] Mode set to {}", mode.name());
    }

    /// Get global reduction mode
    pub fn mode(&self) -> MotionReduction {
        self.config.global_mode
    }

    /// Set override for specific animation type
    pub fn set_override(&mut self, animation_type: AnimationType, mode: Option<MotionReduction>) {
        for (atype, override_mode) in &mut self.config.overrides {
            if *atype == animation_type {
                *override_mode = mode;
                return;
            }
        }
    }

    /// Get override for specific animation type
    pub fn get_override(&self, animation_type: AnimationType) -> Option<MotionReduction> {
        for (atype, override_mode) in &self.config.overrides {
            if *atype == animation_type {
                return *override_mode;
            }
        }
        None
    }

    /// Get effective reduction mode for an animation type
    pub fn get_reduction(&self, animation_type: AnimationType) -> MotionReduction {
        if !self.config.enabled {
            return MotionReduction::None;
        }

        // Check for specific override
        if let Some(override_mode) = self.get_override(animation_type) {
            return override_mode;
        }

        // Use global mode
        self.config.global_mode
    }

    /// Calculate animation timing for a request
    pub fn calculate_timing(&mut self, request: &AnimationRequest) -> AnimationTiming {
        let reduction = self.get_reduction(request.animation_type);
        let mut timing = AnimationTiming::new(request.duration_ms, reduction);

        // Apply duration limits
        if self.config.enabled {
            if timing.effective_duration_ms > self.config.max_duration_ms &&
               self.config.max_duration_ms > 0 {
                timing.effective_duration_ms = self.config.max_duration_ms;
            }
            if timing.effective_duration_ms < self.config.min_duration_ms &&
               !timing.disabled {
                timing.effective_duration_ms = self.config.min_duration_ms;
            }
        }

        // Update statistics
        match reduction {
            MotionReduction::None => {}
            MotionReduction::Reduced => {
                self.stats.animations_reduced += 1;
                self.stats.time_saved_ms += (request.duration_ms - timing.effective_duration_ms) as u64;
            }
            MotionReduction::Crossfade => {
                self.stats.animations_crossfaded += 1;
                self.stats.time_saved_ms += (request.duration_ms - timing.effective_duration_ms) as u64;
            }
            MotionReduction::Disabled => {
                self.stats.animations_disabled += 1;
                self.stats.time_saved_ms += request.duration_ms as u64;
            }
        }

        // Callback
        if let Some(callback) = self.on_timing_calc {
            callback(request, &timing);
        }

        timing
    }

    /// Check if animation should be shown
    pub fn should_animate(&self, animation_type: AnimationType) -> bool {
        if !self.config.enabled {
            return true;
        }

        let reduction = self.get_reduction(animation_type);
        reduction != MotionReduction::Disabled
    }

    /// Check if parallax should be disabled
    pub fn should_disable_parallax(&self) -> bool {
        self.config.enabled && self.config.disable_parallax
    }

    /// Check if auto-play should be paused
    pub fn should_pause_auto_play(&self) -> bool {
        self.config.enabled && self.config.pause_auto_play
    }

    /// Check if blinking should be disabled
    pub fn should_disable_blinking(&self) -> bool {
        self.config.enabled && self.config.disable_blinking
    }

    /// Check if scrolling text should be disabled
    pub fn should_disable_scrolling_text(&self) -> bool {
        self.config.enabled && self.config.disable_scrolling_text
    }

    /// Check if zoom effects should be disabled
    pub fn should_disable_zoom(&self) -> bool {
        self.config.enabled && self.config.disable_zoom_effects
    }

    /// Check if transparency should be reduced
    pub fn should_reduce_transparency(&self) -> bool {
        self.config.enabled && self.config.reduce_transparency
    }

    /// Check if instant visibility should be used
    pub fn should_instant_visibility(&self) -> bool {
        self.config.enabled && self.config.instant_visibility
    }

    /// Check if static backgrounds are preferred
    pub fn prefers_static_backgrounds(&self) -> bool {
        self.config.enabled && self.config.prefer_static_backgrounds
    }

    /// Record auto-play media pause
    pub fn record_auto_play_paused(&mut self) {
        self.stats.auto_play_paused += 1;
    }

    /// Apply preset configuration
    pub fn apply_preset(&mut self, preset: &str) {
        match preset {
            "minimal" => {
                self.config = ReduceMotionConfig::minimal_motion();
            }
            "reduced" => {
                self.config = ReduceMotionConfig::reduced_motion();
            }
            "crossfade" => {
                self.config = ReduceMotionConfig::crossfade_only();
            }
            "default" => {
                self.config = ReduceMotionConfig::default();
                self.config.enabled = true;
            }
            _ => {
                crate::kprintln!("[reduce_motion] Unknown preset: {}", preset);
            }
        }
        crate::kprintln!("[reduce_motion] Applied preset: {}", preset);
    }

    /// Set motion query callback
    pub fn set_motion_query_callback(&mut self, callback: fn(AnimationType) -> bool) {
        self.on_motion_query = Some(callback);
    }

    /// Set timing calculation callback
    pub fn set_timing_callback(&mut self, callback: fn(&AnimationRequest, &AnimationTiming)) {
        self.on_timing_calc = Some(callback);
    }

    /// Get configuration
    pub fn config(&self) -> &ReduceMotionConfig {
        &self.config
    }

    /// Set configuration
    pub fn set_config(&mut self, config: ReduceMotionConfig) {
        self.config = config;
    }

    /// Get statistics
    pub fn stats(&self) -> &ReduceMotionStats {
        &self.stats
    }

    /// Get CSS media query value (prefers-reduced-motion)
    pub fn prefers_reduced_motion(&self) -> &'static str {
        if self.config.enabled {
            "reduce"
        } else {
            "no-preference"
        }
    }

    /// Format status string
    pub fn format_status(&self) -> String {
        format!(
            "Reduce Motion:\n\
             Enabled: {}\n\
             Mode: {}\n\
             Disable parallax: {}\n\
             Pause auto-play: {}\n\
             Disable blinking: {}\n\
             Reduce transparency: {}\n\
             Animations reduced: {}\n\
             Animations disabled: {}\n\
             Animations crossfaded: {}\n\
             Auto-play paused: {}\n\
             Time saved: {}ms",
            if self.config.enabled { "Yes" } else { "No" },
            self.config.global_mode.name(),
            if self.config.disable_parallax { "Yes" } else { "No" },
            if self.config.pause_auto_play { "Yes" } else { "No" },
            if self.config.disable_blinking { "Yes" } else { "No" },
            if self.config.reduce_transparency { "Yes" } else { "No" },
            self.stats.animations_reduced,
            self.stats.animations_disabled,
            self.stats.animations_crossfaded,
            self.stats.auto_play_paused,
            self.stats.time_saved_ms
        )
    }
}

/// Global reduce motion manager
static REDUCE_MOTION: IrqSafeMutex<Option<ReduceMotionManager>> = IrqSafeMutex::new(None);

/// Initialize reduce motion
pub fn init() {
    let mut manager = ReduceMotionManager::new();
    manager.init();
    *REDUCE_MOTION.lock() = Some(manager);
}

/// Enable reduce motion
pub fn enable() {
    if let Some(ref mut manager) = *REDUCE_MOTION.lock() {
        manager.enable();
    }
}

/// Disable reduce motion
pub fn disable() {
    if let Some(ref mut manager) = *REDUCE_MOTION.lock() {
        manager.disable();
    }
}

/// Toggle reduce motion
pub fn toggle() {
    if let Some(ref mut manager) = *REDUCE_MOTION.lock() {
        manager.toggle();
    }
}

/// Check if enabled
pub fn is_enabled() -> bool {
    REDUCE_MOTION.lock().as_ref().map(|m| m.is_enabled()).unwrap_or(false)
}

/// Set reduction mode
pub fn set_mode(mode: MotionReduction) {
    if let Some(ref mut manager) = *REDUCE_MOTION.lock() {
        manager.set_mode(mode);
    }
}

/// Get reduction mode
pub fn get_mode() -> MotionReduction {
    REDUCE_MOTION.lock().as_ref().map(|m| m.mode()).unwrap_or(MotionReduction::None)
}

/// Check if animation should be shown
pub fn should_animate(animation_type: AnimationType) -> bool {
    REDUCE_MOTION.lock().as_ref().map(|m| m.should_animate(animation_type)).unwrap_or(true)
}

/// Calculate timing for animation request
pub fn calculate_timing(request: &AnimationRequest) -> AnimationTiming {
    if let Some(ref mut manager) = *REDUCE_MOTION.lock() {
        manager.calculate_timing(request)
    } else {
        AnimationTiming::new(request.duration_ms, MotionReduction::None)
    }
}

/// Check if parallax should be disabled
pub fn should_disable_parallax() -> bool {
    REDUCE_MOTION.lock().as_ref().map(|m| m.should_disable_parallax()).unwrap_or(false)
}

/// Check if auto-play should be paused
pub fn should_pause_auto_play() -> bool {
    REDUCE_MOTION.lock().as_ref().map(|m| m.should_pause_auto_play()).unwrap_or(false)
}

/// Get prefers-reduced-motion value
pub fn prefers_reduced_motion() -> &'static str {
    REDUCE_MOTION.lock().as_ref().map(|m| m.prefers_reduced_motion()).unwrap_or("no-preference")
}

/// Apply preset
pub fn apply_preset(preset: &str) {
    if let Some(ref mut manager) = *REDUCE_MOTION.lock() {
        manager.apply_preset(preset);
    }
}

/// Get status string
pub fn status() -> String {
    REDUCE_MOTION.lock().as_ref()
        .map(|m| m.format_status())
        .unwrap_or_else(|| String::from("Reduce Motion: Not initialized"))
}

/// Get statistics
pub fn stats() -> Option<ReduceMotionStats> {
    REDUCE_MOTION.lock().as_ref().map(|m| m.stats().clone())
}
