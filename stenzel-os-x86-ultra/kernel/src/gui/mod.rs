//! GUI subsystem
//!
//! Provides windowing, compositing, and graphical user interface components.

pub mod apps;
pub mod compositor;
pub mod desktop;
pub mod lockscreen;
pub mod loginscreen;
pub mod notifications;
pub mod surface;
pub mod systray;
pub mod taskbar;
pub mod widgets;
pub mod window;

pub use compositor::Compositor;
pub use desktop::{Desktop, DesktopIcon, Wallpaper};
pub use lockscreen::LockScreen;
pub use loginscreen::LoginScreen;
pub use notifications::{NotificationManager, Notification, NotificationId, NotificationType};
pub use surface::Surface;
pub use systray::{SystemTray, TrayItem, TrayItemId, TrayIconType, NetworkStatus, VolumeLevel, BatteryStatus};
pub use taskbar::Taskbar;
pub use window::{Window, WindowId};

/// Initialize the GUI subsystem
pub fn init() {
    crate::kprintln!("gui: initializing...");
    compositor::init();

    // Initialize desktop if compositor is available
    if let Some((width, height)) = compositor::screen_size() {
        desktop::init(width, height);
        taskbar::init(width);
    }

    crate::kprintln!("gui: initialized");
}
