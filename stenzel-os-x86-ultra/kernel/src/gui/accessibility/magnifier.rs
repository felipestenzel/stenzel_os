//! Screen Magnifier Accessibility Feature
//!
//! Provides screen magnification for users with low vision.
//! Supports multiple magnification modes and zoom levels.

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;

use crate::sync::IrqSafeMutex;

/// Magnification mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MagnificationMode {
    /// Full screen magnification
    FullScreen,
    /// Lens mode (magnified area follows cursor)
    Lens,
    /// Docked mode (magnified view in a portion of screen)
    Docked,
    /// Split screen (half normal, half magnified)
    Split,
    /// Picture-in-picture (small magnified window)
    PictureInPicture,
}

impl MagnificationMode {
    /// Get mode name
    pub fn name(&self) -> &'static str {
        match self {
            MagnificationMode::FullScreen => "Full Screen",
            MagnificationMode::Lens => "Lens",
            MagnificationMode::Docked => "Docked",
            MagnificationMode::Split => "Split Screen",
            MagnificationMode::PictureInPicture => "Picture-in-Picture",
        }
    }

    /// Get mode description
    pub fn description(&self) -> &'static str {
        match self {
            MagnificationMode::FullScreen => "Magnifies the entire screen",
            MagnificationMode::Lens => "Magnified area follows the cursor",
            MagnificationMode::Docked => "Magnified view in a fixed screen area",
            MagnificationMode::Split => "Screen split between normal and magnified",
            MagnificationMode::PictureInPicture => "Small magnified window overlay",
        }
    }

    /// All modes
    pub fn all() -> Vec<MagnificationMode> {
        alloc::vec![
            MagnificationMode::FullScreen,
            MagnificationMode::Lens,
            MagnificationMode::Docked,
            MagnificationMode::Split,
            MagnificationMode::PictureInPicture,
        ]
    }
}

/// Dock position for docked mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockPosition {
    /// Top of screen
    Top,
    /// Bottom of screen
    Bottom,
    /// Left side of screen
    Left,
    /// Right side of screen
    Right,
}

impl DockPosition {
    /// Get position name
    pub fn name(&self) -> &'static str {
        match self {
            DockPosition::Top => "Top",
            DockPosition::Bottom => "Bottom",
            DockPosition::Left => "Left",
            DockPosition::Right => "Right",
        }
    }
}

/// Lens shape for lens mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LensShape {
    /// Rectangular lens
    Rectangle,
    /// Square lens
    Square,
    /// Circular lens
    Circle,
}

impl LensShape {
    /// Get shape name
    pub fn name(&self) -> &'static str {
        match self {
            LensShape::Rectangle => "Rectangle",
            LensShape::Square => "Square",
            LensShape::Circle => "Circle",
        }
    }
}

/// Tracking mode for cursor following
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackingMode {
    /// Magnified view centered on cursor
    Centered,
    /// View moves when cursor reaches edge
    Proportional,
    /// View moves only when cursor pushes edge
    Push,
    /// View keeps cursor in a comfortable zone
    Comfortable,
}

impl TrackingMode {
    /// Get tracking mode name
    pub fn name(&self) -> &'static str {
        match self {
            TrackingMode::Centered => "Centered",
            TrackingMode::Proportional => "Proportional",
            TrackingMode::Push => "Push",
            TrackingMode::Comfortable => "Comfortable Zone",
        }
    }

    /// Get description
    pub fn description(&self) -> &'static str {
        match self {
            TrackingMode::Centered => "View always centered on cursor",
            TrackingMode::Proportional => "View position matches cursor position ratio",
            TrackingMode::Push => "View moves when cursor reaches edge",
            TrackingMode::Comfortable => "Keeps cursor in middle area of view",
        }
    }
}

/// Zoom level preset
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomLevel {
    /// 1.5x zoom
    X1_5,
    /// 2x zoom
    X2,
    /// 3x zoom
    X3,
    /// 4x zoom
    X4,
    /// 6x zoom
    X6,
    /// 8x zoom
    X8,
    /// 10x zoom
    X10,
    /// 16x zoom
    X16,
    /// Custom zoom level
    Custom(u16), // Zoom percentage (150-1600)
}

impl ZoomLevel {
    /// Get zoom as percentage
    pub fn percentage(&self) -> u16 {
        match self {
            ZoomLevel::X1_5 => 150,
            ZoomLevel::X2 => 200,
            ZoomLevel::X3 => 300,
            ZoomLevel::X4 => 400,
            ZoomLevel::X6 => 600,
            ZoomLevel::X8 => 800,
            ZoomLevel::X10 => 1000,
            ZoomLevel::X16 => 1600,
            ZoomLevel::Custom(p) => *p,
        }
    }

    /// Get zoom as multiplier
    pub fn multiplier(&self) -> f32 {
        self.percentage() as f32 / 100.0
    }

    /// Get zoom level name
    pub fn name(&self) -> String {
        match self {
            ZoomLevel::X1_5 => String::from("1.5x"),
            ZoomLevel::X2 => String::from("2x"),
            ZoomLevel::X3 => String::from("3x"),
            ZoomLevel::X4 => String::from("4x"),
            ZoomLevel::X6 => String::from("6x"),
            ZoomLevel::X8 => String::from("8x"),
            ZoomLevel::X10 => String::from("10x"),
            ZoomLevel::X16 => String::from("16x"),
            ZoomLevel::Custom(p) => format!("{:.1}x", *p as f32 / 100.0),
        }
    }

    /// Create from percentage
    pub fn from_percentage(p: u16) -> Self {
        match p {
            150 => ZoomLevel::X1_5,
            200 => ZoomLevel::X2,
            300 => ZoomLevel::X3,
            400 => ZoomLevel::X4,
            600 => ZoomLevel::X6,
            800 => ZoomLevel::X8,
            1000 => ZoomLevel::X10,
            1600 => ZoomLevel::X16,
            _ => ZoomLevel::Custom(p.max(100).min(1600)),
        }
    }

    /// Get preset levels
    pub fn presets() -> Vec<ZoomLevel> {
        alloc::vec![
            ZoomLevel::X1_5,
            ZoomLevel::X2,
            ZoomLevel::X3,
            ZoomLevel::X4,
            ZoomLevel::X6,
            ZoomLevel::X8,
            ZoomLevel::X10,
            ZoomLevel::X16,
        ]
    }
}

/// Point in screen coordinates
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    /// Create a new point
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// Rectangle in screen coordinates
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    /// Create a new rectangle
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    /// Check if a point is inside the rectangle
    pub fn contains(&self, point: Point) -> bool {
        point.x >= self.x
            && point.x < self.x + self.width as i32
            && point.y >= self.y
            && point.y < self.y + self.height as i32
    }

    /// Get center point
    pub fn center(&self) -> Point {
        Point::new(
            self.x + self.width as i32 / 2,
            self.y + self.height as i32 / 2,
        )
    }
}

/// Magnification view state
#[derive(Debug, Clone)]
pub struct MagnificationView {
    /// Visible screen area (source)
    pub source_rect: Rect,
    /// Display area (destination)
    pub dest_rect: Rect,
    /// Cursor position in screen coordinates
    pub cursor_pos: Point,
    /// Current zoom level
    pub zoom: ZoomLevel,
}

impl MagnificationView {
    /// Create a new magnification view
    pub fn new(screen_width: u32, screen_height: u32, zoom: ZoomLevel) -> Self {
        let source_rect = Self::calculate_source_rect(
            screen_width,
            screen_height,
            Point::new(screen_width as i32 / 2, screen_height as i32 / 2),
            zoom,
        );

        Self {
            source_rect,
            dest_rect: Rect::new(0, 0, screen_width, screen_height),
            cursor_pos: Point::new(screen_width as i32 / 2, screen_height as i32 / 2),
            zoom,
        }
    }

    /// Calculate source rectangle for given center and zoom
    fn calculate_source_rect(
        screen_width: u32,
        screen_height: u32,
        center: Point,
        zoom: ZoomLevel,
    ) -> Rect {
        let multiplier = zoom.multiplier();
        let source_width = (screen_width as f32 / multiplier) as u32;
        let source_height = (screen_height as f32 / multiplier) as u32;

        let x = (center.x - source_width as i32 / 2)
            .max(0)
            .min((screen_width - source_width) as i32);
        let y = (center.y - source_height as i32 / 2)
            .max(0)
            .min((screen_height - source_height) as i32);

        Rect::new(x, y, source_width, source_height)
    }

    /// Update view to center on cursor
    pub fn center_on_cursor(&mut self, screen_width: u32, screen_height: u32) {
        self.source_rect = Self::calculate_source_rect(
            screen_width,
            screen_height,
            self.cursor_pos,
            self.zoom,
        );
    }

    /// Update cursor position
    pub fn set_cursor(&mut self, pos: Point) {
        self.cursor_pos = pos;
    }

    /// Set zoom level
    pub fn set_zoom(&mut self, zoom: ZoomLevel, screen_width: u32, screen_height: u32) {
        self.zoom = zoom;
        self.center_on_cursor(screen_width, screen_height);
    }
}

/// Lens view state
#[derive(Debug, Clone)]
pub struct LensView {
    /// Lens width
    pub width: u32,
    /// Lens height
    pub height: u32,
    /// Lens shape
    pub shape: LensShape,
    /// Current position (follows cursor)
    pub position: Point,
    /// Zoom level
    pub zoom: ZoomLevel,
}

impl LensView {
    /// Create a new lens view
    pub fn new(width: u32, height: u32, shape: LensShape, zoom: ZoomLevel) -> Self {
        Self {
            width,
            height,
            shape,
            position: Point::new(0, 0),
            zoom,
        }
    }

    /// Get lens bounds
    pub fn bounds(&self) -> Rect {
        Rect::new(
            self.position.x - self.width as i32 / 2,
            self.position.y - self.height as i32 / 2,
            self.width,
            self.height,
        )
    }

    /// Get source area for magnification
    pub fn source_bounds(&self) -> Rect {
        let multiplier = self.zoom.multiplier();
        let source_width = (self.width as f32 / multiplier) as u32;
        let source_height = (self.height as f32 / multiplier) as u32;

        Rect::new(
            self.position.x - source_width as i32 / 2,
            self.position.y - source_height as i32 / 2,
            source_width,
            source_height,
        )
    }
}

/// Magnifier configuration
#[derive(Debug, Clone)]
pub struct MagnifierConfig {
    /// Whether magnifier is enabled
    pub enabled: bool,
    /// Magnification mode
    pub mode: MagnificationMode,
    /// Zoom level
    pub zoom: ZoomLevel,
    /// Zoom increment for zoom in/out
    pub zoom_increment: u16,
    /// Tracking mode
    pub tracking: TrackingMode,
    /// Dock position (for docked mode)
    pub dock_position: DockPosition,
    /// Dock size percentage (25-75%)
    pub dock_size_percent: u8,
    /// Lens shape (for lens mode)
    pub lens_shape: LensShape,
    /// Lens width
    pub lens_width: u32,
    /// Lens height
    pub lens_height: u32,
    /// Invert colors in magnified view
    pub invert_colors: bool,
    /// Smooth scrolling
    pub smooth_scrolling: bool,
    /// Scroll speed factor
    pub scroll_speed: f32,
    /// Follow text caret
    pub follow_caret: bool,
    /// Follow keyboard focus
    pub follow_focus: bool,
    /// Show cursor in magnified view
    pub show_cursor: bool,
    /// Cursor magnification factor
    pub cursor_magnification: f32,
}

impl Default for MagnifierConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: MagnificationMode::FullScreen,
            zoom: ZoomLevel::X2,
            zoom_increment: 50, // 50% increment
            tracking: TrackingMode::Comfortable,
            dock_position: DockPosition::Top,
            dock_size_percent: 50,
            lens_shape: LensShape::Rectangle,
            lens_width: 400,
            lens_height: 300,
            invert_colors: false,
            smooth_scrolling: true,
            scroll_speed: 1.0,
            follow_caret: true,
            follow_focus: true,
            show_cursor: true,
            cursor_magnification: 2.0,
        }
    }
}

/// Magnifier statistics
#[derive(Debug, Clone, Default)]
pub struct MagnifierStats {
    /// Number of times enabled
    pub times_enabled: u64,
    /// Total time enabled (ms)
    pub total_time_enabled_ms: u64,
    /// Number of zoom changes
    pub zoom_changes: u64,
    /// Number of mode changes
    pub mode_changes: u64,
    /// Total pan distance (pixels)
    pub total_pan_distance: u64,
    /// Session start timestamp
    pub session_start_ms: u64,
}

/// Screen magnifier manager
pub struct Magnifier {
    /// Configuration
    config: MagnifierConfig,
    /// Statistics
    stats: MagnifierStats,
    /// Enable timestamp
    enabled_since_ms: Option<u64>,
    /// Screen dimensions
    screen_width: u32,
    screen_height: u32,
    /// Full screen view state
    fullscreen_view: Option<MagnificationView>,
    /// Lens view state
    lens_view: Option<LensView>,
    /// Last cursor position
    last_cursor: Point,
    /// Callback when mode changes
    on_mode_change: Option<fn(bool)>,
    /// Callback for redraw
    on_redraw: Option<fn()>,
}

impl Magnifier {
    /// Create a new magnifier
    pub fn new() -> Self {
        Self {
            config: MagnifierConfig::default(),
            stats: MagnifierStats::default(),
            enabled_since_ms: None,
            screen_width: 1920,
            screen_height: 1080,
            fullscreen_view: None,
            lens_view: None,
            last_cursor: Point::new(0, 0),
            on_mode_change: None,
            on_redraw: None,
        }
    }

    /// Initialize the magnifier
    pub fn init(&mut self) {
        self.stats.session_start_ms = crate::time::uptime_ms();
        crate::kprintln!("[magnifier] Screen magnifier initialized");
    }

    /// Set screen dimensions
    pub fn set_screen_size(&mut self, width: u32, height: u32) {
        self.screen_width = width;
        self.screen_height = height;

        // Update views if active
        if let Some(ref mut view) = self.fullscreen_view {
            *view = MagnificationView::new(width, height, self.config.zoom);
        }
    }

    /// Enable magnifier
    pub fn enable(&mut self) {
        if !self.config.enabled {
            self.config.enabled = true;
            self.enabled_since_ms = Some(crate::time::uptime_ms());
            self.stats.times_enabled += 1;

            // Initialize view based on mode
            self.initialize_view();

            if let Some(callback) = self.on_mode_change {
                callback(true);
            }

            crate::kprintln!(
                "[magnifier] Magnifier enabled ({}, {})",
                self.config.mode.name(),
                self.config.zoom.name()
            );
        }
    }

    /// Disable magnifier
    pub fn disable(&mut self) {
        if self.config.enabled {
            self.config.enabled = false;

            // Track duration
            if let Some(start) = self.enabled_since_ms.take() {
                let now = crate::time::uptime_ms();
                self.stats.total_time_enabled_ms += now - start;
            }

            // Clear views
            self.fullscreen_view = None;
            self.lens_view = None;

            if let Some(callback) = self.on_mode_change {
                callback(false);
            }

            crate::kprintln!("[magnifier] Magnifier disabled");
        }
    }

    /// Toggle magnifier
    pub fn toggle(&mut self) {
        if self.config.enabled {
            self.disable();
        } else {
            self.enable();
        }
    }

    /// Check if magnifier is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Initialize view for current mode
    fn initialize_view(&mut self) {
        match self.config.mode {
            MagnificationMode::FullScreen | MagnificationMode::Docked | MagnificationMode::Split => {
                self.fullscreen_view = Some(MagnificationView::new(
                    self.screen_width,
                    self.screen_height,
                    self.config.zoom,
                ));
            }
            MagnificationMode::Lens | MagnificationMode::PictureInPicture => {
                self.lens_view = Some(LensView::new(
                    self.config.lens_width,
                    self.config.lens_height,
                    self.config.lens_shape,
                    self.config.zoom,
                ));
            }
        }
    }

    /// Set magnification mode
    pub fn set_mode(&mut self, mode: MagnificationMode) {
        if self.config.mode != mode {
            self.config.mode = mode;
            self.stats.mode_changes += 1;

            // Re-initialize view for new mode
            if self.config.enabled {
                self.fullscreen_view = None;
                self.lens_view = None;
                self.initialize_view();
            }

            crate::kprintln!("[magnifier] Mode changed to: {}", mode.name());
        }
    }

    /// Get current mode
    pub fn mode(&self) -> MagnificationMode {
        self.config.mode
    }

    /// Set zoom level
    pub fn set_zoom(&mut self, zoom: ZoomLevel) {
        if self.config.zoom.percentage() != zoom.percentage() {
            self.config.zoom = zoom;
            self.stats.zoom_changes += 1;

            // Update views
            if let Some(ref mut view) = self.fullscreen_view {
                view.set_zoom(zoom, self.screen_width, self.screen_height);
            }
            if let Some(ref mut view) = self.lens_view {
                view.zoom = zoom;
            }

            self.request_redraw();
            crate::kprintln!("[magnifier] Zoom changed to: {}", zoom.name());
        }
    }

    /// Get current zoom level
    pub fn zoom(&self) -> ZoomLevel {
        self.config.zoom
    }

    /// Zoom in
    pub fn zoom_in(&mut self) {
        let current = self.config.zoom.percentage();
        let new_zoom = (current + self.config.zoom_increment).min(1600);
        self.set_zoom(ZoomLevel::from_percentage(new_zoom));
    }

    /// Zoom out
    pub fn zoom_out(&mut self) {
        let current = self.config.zoom.percentage();
        if current > self.config.zoom_increment {
            let new_zoom = current - self.config.zoom_increment;
            if new_zoom >= 100 {
                self.set_zoom(ZoomLevel::from_percentage(new_zoom));
            }
        }
    }

    /// Update cursor position
    pub fn update_cursor(&mut self, x: i32, y: i32) {
        let new_pos = Point::new(x, y);

        // Track pan distance
        let dx = (new_pos.x - self.last_cursor.x).abs() as u64;
        let dy = (new_pos.y - self.last_cursor.y).abs() as u64;
        self.stats.total_pan_distance += dx + dy;

        self.last_cursor = new_pos;

        if !self.config.enabled {
            return;
        }

        // Extract values needed for tracking
        let tracking = self.config.tracking;
        let screen_width = self.screen_width;
        let screen_height = self.screen_height;

        // Update views
        match self.config.mode {
            MagnificationMode::FullScreen => {
                if let Some(ref mut view) = self.fullscreen_view {
                    view.set_cursor(new_pos);
                    Self::apply_tracking_to_view(view, tracking, screen_width, screen_height);
                }
                self.request_redraw();
            }
            MagnificationMode::Lens | MagnificationMode::PictureInPicture => {
                if let Some(ref mut view) = self.lens_view {
                    view.position = new_pos;
                }
                self.request_redraw();
            }
            _ => {
                if let Some(ref mut view) = self.fullscreen_view {
                    view.set_cursor(new_pos);
                    Self::apply_tracking_to_view(view, tracking, screen_width, screen_height);
                }
                self.request_redraw();
            }
        }
    }

    /// Apply tracking mode to view (static to avoid borrowing issues)
    fn apply_tracking_to_view(view: &mut MagnificationView, tracking: TrackingMode, screen_width: u32, screen_height: u32) {
        match tracking {
            TrackingMode::Centered => {
                view.center_on_cursor(screen_width, screen_height);
            }
            TrackingMode::Proportional => {
                // View position follows cursor position proportionally
                let x_ratio = view.cursor_pos.x as f32 / screen_width as f32;
                let y_ratio = view.cursor_pos.y as f32 / screen_height as f32;

                let max_x = screen_width - view.source_rect.width;
                let max_y = screen_height - view.source_rect.height;

                view.source_rect.x = (x_ratio * max_x as f32) as i32;
                view.source_rect.y = (y_ratio * max_y as f32) as i32;
            }
            TrackingMode::Push => {
                // Only move view when cursor pushes edge
                let margin = 50; // pixels
                if view.cursor_pos.x < view.source_rect.x + margin {
                    view.source_rect.x = (view.cursor_pos.x - margin).max(0);
                }
                if view.cursor_pos.x > view.source_rect.x + view.source_rect.width as i32 - margin {
                    view.source_rect.x = (view.cursor_pos.x - view.source_rect.width as i32 + margin)
                        .min((screen_width - view.source_rect.width) as i32);
                }
                if view.cursor_pos.y < view.source_rect.y + margin {
                    view.source_rect.y = (view.cursor_pos.y - margin).max(0);
                }
                if view.cursor_pos.y > view.source_rect.y + view.source_rect.height as i32 - margin {
                    view.source_rect.y = (view.cursor_pos.y - view.source_rect.height as i32 + margin)
                        .min((screen_height - view.source_rect.height) as i32);
                }
            }
            TrackingMode::Comfortable => {
                // Keep cursor in comfortable zone (middle 60%)
                let margin_x = view.source_rect.width * 20 / 100;
                let margin_y = view.source_rect.height * 20 / 100;

                let inner_left = view.source_rect.x + margin_x as i32;
                let inner_right = view.source_rect.x + view.source_rect.width as i32 - margin_x as i32;
                let inner_top = view.source_rect.y + margin_y as i32;
                let inner_bottom = view.source_rect.y + view.source_rect.height as i32 - margin_y as i32;

                if view.cursor_pos.x < inner_left {
                    view.source_rect.x -= inner_left - view.cursor_pos.x;
                } else if view.cursor_pos.x > inner_right {
                    view.source_rect.x += view.cursor_pos.x - inner_right;
                }

                if view.cursor_pos.y < inner_top {
                    view.source_rect.y -= inner_top - view.cursor_pos.y;
                } else if view.cursor_pos.y > inner_bottom {
                    view.source_rect.y += view.cursor_pos.y - inner_bottom;
                }

                // Clamp to screen bounds
                view.source_rect.x = view.source_rect.x.max(0).min((screen_width - view.source_rect.width) as i32);
                view.source_rect.y = view.source_rect.y.max(0).min((screen_height - view.source_rect.height) as i32);
            }
        }
    }

    /// Request redraw
    fn request_redraw(&self) {
        if let Some(callback) = self.on_redraw {
            callback();
        }
    }

    /// Get full screen view
    pub fn fullscreen_view(&self) -> Option<&MagnificationView> {
        self.fullscreen_view.as_ref()
    }

    /// Get lens view
    pub fn lens_view(&self) -> Option<&LensView> {
        self.lens_view.as_ref()
    }

    /// Set tracking mode
    pub fn set_tracking(&mut self, tracking: TrackingMode) {
        self.config.tracking = tracking;
    }

    /// Set lens size
    pub fn set_lens_size(&mut self, width: u32, height: u32) {
        self.config.lens_width = width;
        self.config.lens_height = height;

        if let Some(ref mut view) = self.lens_view {
            view.width = width;
            view.height = height;
        }
    }

    /// Set lens shape
    pub fn set_lens_shape(&mut self, shape: LensShape) {
        self.config.lens_shape = shape;

        if let Some(ref mut view) = self.lens_view {
            view.shape = shape;
        }
    }

    /// Set color inversion
    pub fn set_invert_colors(&mut self, invert: bool) {
        self.config.invert_colors = invert;
        self.request_redraw();
    }

    /// Get configuration
    pub fn config(&self) -> &MagnifierConfig {
        &self.config
    }

    /// Set configuration
    pub fn set_config(&mut self, config: MagnifierConfig) {
        let was_enabled = self.config.enabled;
        self.config = config;

        if self.config.enabled && !was_enabled {
            self.enable();
        } else if !self.config.enabled && was_enabled {
            self.disable();
        }
    }

    /// Set mode change callback
    pub fn set_mode_change_callback(&mut self, callback: fn(bool)) {
        self.on_mode_change = Some(callback);
    }

    /// Set redraw callback
    pub fn set_redraw_callback(&mut self, callback: fn()) {
        self.on_redraw = Some(callback);
    }

    /// Get statistics
    pub fn stats(&self) -> MagnifierStats {
        let mut stats = self.stats.clone();

        if let Some(start) = self.enabled_since_ms {
            let now = crate::time::uptime_ms();
            stats.total_time_enabled_ms += now - start;
        }

        stats
    }

    /// Format status string
    pub fn format_status(&self) -> String {
        let stats = self.stats();

        format!(
            "Magnifier: {}\n\
             Mode: {}\n\
             Zoom: {}\n\
             Tracking: {}\n\
             Invert colors: {}\n\
             Times enabled: {}\n\
             Total time enabled: {} ms\n\
             Zoom changes: {}\n\
             Mode changes: {}",
            if self.config.enabled { "Enabled" } else { "Disabled" },
            self.config.mode.name(),
            self.config.zoom.name(),
            self.config.tracking.name(),
            if self.config.invert_colors { "Yes" } else { "No" },
            stats.times_enabled,
            stats.total_time_enabled_ms,
            stats.zoom_changes,
            stats.mode_changes
        )
    }
}

/// Global magnifier instance
static MAGNIFIER: IrqSafeMutex<Option<Magnifier>> = IrqSafeMutex::new(None);

/// Initialize magnifier
pub fn init() {
    let mut magnifier = Magnifier::new();
    magnifier.init();
    *MAGNIFIER.lock() = Some(magnifier);
}

/// Enable magnifier
pub fn enable() {
    if let Some(ref mut magnifier) = *MAGNIFIER.lock() {
        magnifier.enable();
    }
}

/// Disable magnifier
pub fn disable() {
    if let Some(ref mut magnifier) = *MAGNIFIER.lock() {
        magnifier.disable();
    }
}

/// Toggle magnifier
pub fn toggle() {
    if let Some(ref mut magnifier) = *MAGNIFIER.lock() {
        magnifier.toggle();
    }
}

/// Check if magnifier is enabled
pub fn is_enabled() -> bool {
    MAGNIFIER.lock().as_ref().map(|m| m.is_enabled()).unwrap_or(false)
}

/// Set zoom level
pub fn set_zoom(zoom: ZoomLevel) {
    if let Some(ref mut magnifier) = *MAGNIFIER.lock() {
        magnifier.set_zoom(zoom);
    }
}

/// Get current zoom
pub fn get_zoom() -> ZoomLevel {
    MAGNIFIER.lock().as_ref().map(|m| m.zoom()).unwrap_or(ZoomLevel::X2)
}

/// Zoom in
pub fn zoom_in() {
    if let Some(ref mut magnifier) = *MAGNIFIER.lock() {
        magnifier.zoom_in();
    }
}

/// Zoom out
pub fn zoom_out() {
    if let Some(ref mut magnifier) = *MAGNIFIER.lock() {
        magnifier.zoom_out();
    }
}

/// Update cursor position
pub fn update_cursor(x: i32, y: i32) {
    if let Some(ref mut magnifier) = *MAGNIFIER.lock() {
        magnifier.update_cursor(x, y);
    }
}

/// Set screen size
pub fn set_screen_size(width: u32, height: u32) {
    if let Some(ref mut magnifier) = *MAGNIFIER.lock() {
        magnifier.set_screen_size(width, height);
    }
}

/// Get statistics
pub fn stats() -> Option<MagnifierStats> {
    MAGNIFIER.lock().as_ref().map(|m| m.stats())
}

/// Get status string
pub fn status() -> String {
    MAGNIFIER.lock().as_ref()
        .map(|m| m.format_status())
        .unwrap_or_else(|| String::from("Magnifier: Not initialized"))
}
