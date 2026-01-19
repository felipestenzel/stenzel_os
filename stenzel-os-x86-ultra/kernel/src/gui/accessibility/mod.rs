//! Accessibility Features Module
//!
//! Provides accessibility features for users with disabilities:
//! - Screen reader for visually impaired users
//! - High contrast mode for low vision
//! - Large text scaling
//! - Screen magnifier
//! - Sticky Keys, Slow Keys, Bounce Keys, Mouse Keys
//! - On-screen keyboard
//! - Reduce motion
//! - Color filters for color blindness
//! - Voice control

pub mod color_filters;
pub mod high_contrast;
pub mod keyboard;
pub mod large_text;
pub mod magnifier;
pub mod osk;
pub mod reduce_motion;
pub mod screen_reader;
pub mod voice_control;

pub use screen_reader::{
    ScreenReader, AccessibleRole, AccessibleState, AccessibleElement,
    SpeechPriority, SpeechUtterance, VerbosityLevel, NavigationMode,
    ScreenReaderConfig, ScreenReaderStats, TtsCallback,
};

pub use high_contrast::{
    HighContrastManager, HighContrastConfig, HighContrastStats,
    ContrastScheme, ContrastPalette, Color, ElementType, ElementState, ElementStyle,
};

pub use large_text::{
    LargeTextManager, LargeTextConfig, LargeTextStats,
    TextScale, TextCategory, TextProperties, FontWeight, CategoryScales,
};

pub use magnifier::{
    Magnifier, MagnifierConfig, MagnifierStats,
    MagnificationMode, DockPosition, LensShape, TrackingMode, ZoomLevel,
    Point, Rect, MagnificationView, LensView,
};

pub use keyboard::{
    KeyboardAccessibility, KeyboardAccessibilityStats,
    ModifierKey, StickyState, StickyKeysConfig,
    SlowKeysConfig, BounceKeysConfig, MouseKeysConfig,
    MouseButton, KeyEventType, KeyEventResult,
};

pub use osk::{
    OnScreenKeyboard, OskConfig, OskStats, OskTheme, OskColor,
    KeyboardLayout, KeyboardMode, KeyboardPosition, KeyType,
    KeyDefinition, KeyVisual, KeyState, Prediction, KeyEventOutput,
};

pub use reduce_motion::{
    ReduceMotionManager, ReduceMotionConfig, ReduceMotionStats,
    AnimationType, MotionReduction, AnimationTiming, AnimationRequest,
};

pub use color_filters::{
    ColorFilterManager, ColorFilterConfig, ColorFilterStats,
    ColorBlindnessType, ColorFilterType, Rgb, ColorMatrix,
};

pub use voice_control::{
    VoiceControl, VoiceControlConfig, VoiceControlStats,
    VoiceCommand, CommandCategory, CommandAction, VoiceControlState,
    RecognitionResult, SystemCommand, NavigationTarget, WindowAction, AccessibilityAction,
};

/// Initialize all accessibility features
pub fn init() {
    screen_reader::init();
    high_contrast::init();
    large_text::init();
    magnifier::init();
    keyboard::init();
    osk::init();
    reduce_motion::init();
    color_filters::init();
    voice_control::init();
    crate::kprintln!("[accessibility] Accessibility module initialized");
}
