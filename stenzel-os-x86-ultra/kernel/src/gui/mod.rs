//! GUI subsystem
//!
//! Provides windowing, compositing, and graphical user interface components.

pub mod animations;
pub mod apps;
pub mod compositor;
pub mod cursors;
pub mod desktop;
pub mod fonts;
pub mod icons;
pub mod launcher;
pub mod lockscreen;
pub mod loginscreen;
pub mod multimon;
pub mod notification_center;
pub mod notifications;
pub mod shaping;
pub mod shell;
pub mod surface;
pub mod systray;
pub mod taskbar;
pub mod theme;
pub mod transparency;
pub mod wallpaper;
pub mod settings;
pub mod widgets;
pub mod window;
pub mod window_manager;

pub use animations::{
    Animation, AnimationConfig, AnimationDirection, AnimationFillMode, AnimationGroup,
    AnimationId, AnimationManager, AnimationSequence, AnimationState, AnimatedProperty,
    EasingFunction, PropertyValue, presets as animation_presets,
};
pub use compositor::Compositor;
pub use desktop::{Desktop, DesktopIcon, Wallpaper};
pub use lockscreen::LockScreen;
pub use loginscreen::LoginScreen;
pub use notifications::{NotificationManager, Notification, NotificationId, NotificationType};
pub use surface::Surface;
pub use systray::{SystemTray, TrayItem, TrayItemId, TrayIconType, NetworkStatus, VolumeLevel, BatteryStatus};
pub use taskbar::Taskbar;
pub use window::{Window, WindowId};
pub use transparency::{Opacity, BlendMode, WindowTransparency, BlurConfig, ShadowConfig, GlassConfig};
pub use theme::{ColorScheme, AccentColor, ThemeColors, ThemeError};
pub use icons::{IconData, BuiltinIcon, IconError};
pub use cursors::{CursorType, CursorImage, CursorError};
pub use fonts::{Font, FontMetrics, TextLayout, RenderedText, FontError};
pub use wallpaper::{WallpaperInfo, ScaleMode, WallpaperError};

/// Initialize the GUI subsystem
pub fn init() {
    crate::kprintln!("gui: initializing...");
    compositor::init();
    transparency::init();
    animations::init();
    theme::init();
    icons::init();
    cursors::init();
    fonts::init();
    wallpaper::init();
    settings::init();

    // Initialize desktop if compositor is available
    if let Some((width, height)) = compositor::screen_size() {
        desktop::init(width, height);
        taskbar::init(width);
    }

    crate::kprintln!("gui: initialized");
}
