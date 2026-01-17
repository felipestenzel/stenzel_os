//! GUI Widgets module
//!
//! Provides reusable UI components for building graphical applications.

pub mod button;
pub mod label;
pub mod textbox;
pub mod checkbox;
pub mod radio;
pub mod dropdown;
pub mod progress;
pub mod slider;
pub mod scrollbar;
pub mod treeview;
pub mod tabcontrol;
pub mod listview;
pub mod menu;
pub mod dialog;
pub mod filepicker;
pub mod startmenu;
pub mod clock;
pub mod volume;
pub mod network;
pub mod battery;

pub use button::Button;
pub use label::Label;
pub use textbox::TextBox;
pub use checkbox::Checkbox;
pub use radio::{RadioButton, RadioGroup};
pub use dropdown::Dropdown;
pub use progress::ProgressBar;
pub use slider::{Slider, SliderOrientation, SliderStyle};
pub use scrollbar::Scrollbar;
pub use treeview::{TreeView, TreeNode, TreeNodeId, TreeNodeIcon, TreeViewStyle, SelectionMode};
pub use tabcontrol::{TabControl, Tab, TabId, TabStyle, TabPosition};
pub use listview::ListView;
pub use menu::{MenuBar, MenuItem, ContextMenu};
pub use dialog::{Dialog, DialogResult, MessageBox};
pub use filepicker::FilePicker;
pub use startmenu::{StartMenu, StartMenuItem};
pub use clock::Clock;
pub use volume::{VolumeControl, AudioOutput};
pub use network::{NetworkIndicator, NetworkInterface, WiFiNetwork, ConnectionType, ConnectionStatus, SignalStrength};
pub use battery::{BatteryIndicator, BatteryInfo, BatteryState, PowerProfile};

use crate::drivers::framebuffer::Color;
use super::surface::Surface;

/// Unique widget identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WidgetId(pub u64);

static NEXT_WIDGET_ID: spin::Mutex<u64> = spin::Mutex::new(1);

impl WidgetId {
    pub fn new() -> Self {
        let mut next = NEXT_WIDGET_ID.lock();
        let id = *next;
        *next += 1;
        WidgetId(id)
    }
}

impl Default for WidgetId {
    fn default() -> Self {
        Self::new()
    }
}

/// Widget state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetState {
    Normal,
    Hovered,
    Pressed,
    Focused,
    Disabled,
}

/// Common widget bounds
#[derive(Debug, Clone, Copy)]
pub struct Bounds {
    pub x: isize,
    pub y: isize,
    pub width: usize,
    pub height: usize,
}

impl Bounds {
    pub fn new(x: isize, y: isize, width: usize, height: usize) -> Self {
        Self { x, y, width, height }
    }

    pub fn contains(&self, px: isize, py: isize) -> bool {
        px >= self.x
            && px < self.x + self.width as isize
            && py >= self.y
            && py < self.y + self.height as isize
    }
}

/// Mouse button for events
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Widget event
#[derive(Debug, Clone)]
pub enum WidgetEvent {
    MouseEnter,
    MouseLeave,
    MouseMove { x: isize, y: isize },
    MouseDown { button: MouseButton, x: isize, y: isize },
    MouseUp { button: MouseButton, x: isize, y: isize },
    Click { button: MouseButton },
    DoubleClick { button: MouseButton, x: isize, y: isize },
    KeyDown { key: u8, modifiers: u8 },
    KeyUp { key: u8, modifiers: u8 },
    Character { c: char },
    Focus,
    Blur,
    Scroll { delta_x: i32, delta_y: i32 },
}

/// Key modifiers
pub mod modifiers {
    pub const SHIFT: u8 = 1 << 0;
    pub const CTRL: u8 = 1 << 1;
    pub const ALT: u8 = 1 << 2;
    pub const META: u8 = 1 << 3;
}

/// Widget theme colors
#[derive(Debug, Clone, Copy)]
pub struct WidgetTheme {
    /// Background color
    pub bg: Color,
    /// Background when hovered
    pub bg_hover: Color,
    /// Background when pressed
    pub bg_pressed: Color,
    /// Background when disabled
    pub bg_disabled: Color,
    /// Foreground/text color
    pub fg: Color,
    /// Foreground when disabled
    pub fg_disabled: Color,
    /// Border color
    pub border: Color,
    /// Border when focused
    pub border_focused: Color,
    /// Accent color
    pub accent: Color,
}

impl Default for WidgetTheme {
    fn default() -> Self {
        Self {
            bg: Color::new(60, 60, 68),
            bg_hover: Color::new(75, 75, 85),
            bg_pressed: Color::new(50, 50, 58),
            bg_disabled: Color::new(45, 45, 50),
            fg: Color::WHITE,
            fg_disabled: Color::new(128, 128, 128),
            border: Color::new(80, 80, 90),
            border_focused: Color::new(100, 150, 255),
            accent: Color::new(0, 120, 215),
        }
    }
}

/// Global widget theme
static THEME: spin::Mutex<WidgetTheme> = spin::Mutex::new(WidgetTheme {
    bg: Color { r: 60, g: 60, b: 68, a: 255 },
    bg_hover: Color { r: 75, g: 75, b: 85, a: 255 },
    bg_pressed: Color { r: 50, g: 50, b: 58, a: 255 },
    bg_disabled: Color { r: 45, g: 45, b: 50, a: 255 },
    fg: Color { r: 255, g: 255, b: 255, a: 255 },
    fg_disabled: Color { r: 128, g: 128, b: 128, a: 255 },
    border: Color { r: 80, g: 80, b: 90, a: 255 },
    border_focused: Color { r: 100, g: 150, b: 255, a: 255 },
    accent: Color { r: 0, g: 120, b: 215, a: 255 },
});

/// Get current theme
pub fn theme() -> WidgetTheme {
    *THEME.lock()
}

/// Set theme
pub fn set_theme(theme: WidgetTheme) {
    *THEME.lock() = theme;
}

/// Common trait for all widgets
pub trait Widget {
    /// Get widget ID
    fn id(&self) -> WidgetId;

    /// Get bounds
    fn bounds(&self) -> Bounds;

    /// Set position
    fn set_position(&mut self, x: isize, y: isize);

    /// Set size
    fn set_size(&mut self, width: usize, height: usize);

    /// Check if enabled
    fn is_enabled(&self) -> bool;

    /// Enable/disable
    fn set_enabled(&mut self, enabled: bool);

    /// Check if visible
    fn is_visible(&self) -> bool;

    /// Show/hide
    fn set_visible(&mut self, visible: bool);

    /// Handle event, returns true if event was consumed
    fn handle_event(&mut self, event: &WidgetEvent) -> bool;

    /// Render to surface
    fn render(&self, surface: &mut Surface);

    /// Check if widget contains point
    fn contains(&self, x: isize, y: isize) -> bool {
        self.bounds().contains(x, y)
    }
}
