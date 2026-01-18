//! GUI subsystem
//!
//! Provides windowing, compositing, and graphical user interface components.

pub mod animations;
pub mod apps;
pub mod compositor;
pub mod desktop;
pub mod lockscreen;
pub mod loginscreen;
pub mod multimon;
pub mod notifications;
pub mod shaping;
pub mod surface;
pub mod systray;
pub mod taskbar;
pub mod transparency;
pub mod widgets;
pub mod window;

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

/// Initialize the GUI subsystem
pub fn init() {
    crate::kprintln!("gui: initializing...");
    compositor::init();
    transparency::init();
    animations::init();

    // Initialize desktop if compositor is available
    if let Some((width, height)) = compositor::screen_size() {
        desktop::init(width, height);
        taskbar::init(width);
    }

    crate::kprintln!("gui: initialized");
}
