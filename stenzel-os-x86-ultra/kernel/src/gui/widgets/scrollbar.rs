//! Scrollbar widget
//!
//! Vertical and horizontal scrollbars for scrollable content.

use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use super::{Widget, WidgetId, WidgetState, WidgetEvent, Bounds, MouseButton, theme};

/// Scrollbar orientation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollOrientation {
    Vertical,
    Horizontal,
}

/// Scrollbar change callback
pub type ScrollCallback = fn(WidgetId, usize);

/// A scrollbar widget
pub struct Scrollbar {
    id: WidgetId,
    bounds: Bounds,
    orientation: ScrollOrientation,
    value: usize,         // Current scroll position
    max_value: usize,     // Maximum scroll position
    visible_amount: usize, // Visible portion (determines thumb size)
    state: WidgetState,
    thumb_state: WidgetState,
    enabled: bool,
    visible: bool,
    dragging: bool,
    drag_offset: isize,
    on_scroll: Option<ScrollCallback>,
}

impl Scrollbar {
    const MIN_THUMB_SIZE: usize = 20;
    const ARROW_SIZE: usize = 16;

    /// Create a new scrollbar
    pub fn new(x: isize, y: isize, length: usize, orientation: ScrollOrientation) -> Self {
        let (width, height) = match orientation {
            ScrollOrientation::Vertical => (16, length),
            ScrollOrientation::Horizontal => (length, 16),
        };

        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(x, y, width, height),
            orientation,
            value: 0,
            max_value: 100,
            visible_amount: 20,
            state: WidgetState::Normal,
            thumb_state: WidgetState::Normal,
            enabled: true,
            visible: true,
            dragging: false,
            drag_offset: 0,
            on_scroll: None,
        }
    }

    /// Create vertical scrollbar
    pub fn vertical(x: isize, y: isize, height: usize) -> Self {
        Self::new(x, y, height, ScrollOrientation::Vertical)
    }

    /// Create horizontal scrollbar
    pub fn horizontal(x: isize, y: isize, width: usize) -> Self {
        Self::new(x, y, width, ScrollOrientation::Horizontal)
    }

    /// Set scroll value
    pub fn set_value(&mut self, value: usize) {
        self.value = value.min(self.max_value);
        self.notify_scroll();
    }

    /// Get current value
    pub fn value(&self) -> usize {
        self.value
    }

    /// Set maximum value
    pub fn set_max(&mut self, max: usize) {
        self.max_value = max;
        if self.value > max {
            self.value = max;
        }
    }

    /// Get maximum value
    pub fn max(&self) -> usize {
        self.max_value
    }

    /// Set visible amount (affects thumb size)
    pub fn set_visible_amount(&mut self, amount: usize) {
        self.visible_amount = amount.max(1);
    }

    /// Set scroll callback
    pub fn set_on_scroll(&mut self, callback: ScrollCallback) {
        self.on_scroll = Some(callback);
    }

    /// Scroll by delta
    pub fn scroll_by(&mut self, delta: isize) {
        let new_value = if delta < 0 {
            self.value.saturating_sub((-delta) as usize)
        } else {
            (self.value + delta as usize).min(self.max_value)
        };
        if new_value != self.value {
            self.value = new_value;
            self.notify_scroll();
        }
    }

    /// Calculate thumb bounds
    fn thumb_bounds(&self) -> Bounds {
        let track = self.track_bounds();
        let total = self.max_value + self.visible_amount;

        if total == 0 {
            return track;
        }

        match self.orientation {
            ScrollOrientation::Vertical => {
                let thumb_height = ((self.visible_amount as f32 / total as f32) * track.height as f32) as usize;
                let thumb_height = thumb_height.max(Self::MIN_THUMB_SIZE).min(track.height);

                let available = track.height.saturating_sub(thumb_height);
                let thumb_y = if self.max_value > 0 {
                    (self.value as f32 / self.max_value as f32 * available as f32) as isize
                } else {
                    0
                };

                Bounds::new(track.x, track.y + thumb_y, track.width, thumb_height)
            }
            ScrollOrientation::Horizontal => {
                let thumb_width = ((self.visible_amount as f32 / total as f32) * track.width as f32) as usize;
                let thumb_width = thumb_width.max(Self::MIN_THUMB_SIZE).min(track.width);

                let available = track.width.saturating_sub(thumb_width);
                let thumb_x = if self.max_value > 0 {
                    (self.value as f32 / self.max_value as f32 * available as f32) as isize
                } else {
                    0
                };

                Bounds::new(track.x + thumb_x, track.y, thumb_width, track.height)
            }
        }
    }

    /// Calculate track bounds (area where thumb moves)
    fn track_bounds(&self) -> Bounds {
        match self.orientation {
            ScrollOrientation::Vertical => Bounds::new(
                self.bounds.x,
                self.bounds.y + Self::ARROW_SIZE as isize,
                self.bounds.width,
                self.bounds.height.saturating_sub(Self::ARROW_SIZE * 2),
            ),
            ScrollOrientation::Horizontal => Bounds::new(
                self.bounds.x + Self::ARROW_SIZE as isize,
                self.bounds.y,
                self.bounds.width.saturating_sub(Self::ARROW_SIZE * 2),
                self.bounds.height,
            ),
        }
    }

    fn notify_scroll(&self) {
        if let Some(callback) = self.on_scroll {
            callback(self.id, self.value);
        }
    }

    fn value_from_position(&self, pos: isize) -> usize {
        let track = self.track_bounds();
        let thumb = self.thumb_bounds();

        match self.orientation {
            ScrollOrientation::Vertical => {
                let available = track.height.saturating_sub(thumb.height);
                if available == 0 {
                    return 0;
                }
                let rel_pos = (pos - track.y - self.drag_offset).max(0) as usize;
                let ratio = rel_pos as f32 / available as f32;
                (ratio * self.max_value as f32) as usize
            }
            ScrollOrientation::Horizontal => {
                let available = track.width.saturating_sub(thumb.width);
                if available == 0 {
                    return 0;
                }
                let rel_pos = (pos - track.x - self.drag_offset).max(0) as usize;
                let ratio = rel_pos as f32 / available as f32;
                (ratio * self.max_value as f32) as usize
            }
        }
    }
}

impl Widget for Scrollbar {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn bounds(&self) -> Bounds {
        self.bounds
    }

    fn set_position(&mut self, x: isize, y: isize) {
        self.bounds.x = x;
        self.bounds.y = y;
    }

    fn set_size(&mut self, width: usize, height: usize) {
        self.bounds.width = width;
        self.bounds.height = height;
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.state = WidgetState::Disabled;
            self.thumb_state = WidgetState::Disabled;
        } else if self.state == WidgetState::Disabled {
            self.state = WidgetState::Normal;
            self.thumb_state = WidgetState::Normal;
        }
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn handle_event(&mut self, event: &WidgetEvent) -> bool {
        if !self.enabled || !self.visible {
            return false;
        }

        match event {
            WidgetEvent::MouseEnter => {
                self.state = WidgetState::Hovered;
                true
            }
            WidgetEvent::MouseLeave => {
                if !self.dragging {
                    self.state = WidgetState::Normal;
                    self.thumb_state = WidgetState::Normal;
                }
                true
            }
            WidgetEvent::MouseMove { x, y } => {
                if self.dragging {
                    let new_value = match self.orientation {
                        ScrollOrientation::Vertical => self.value_from_position(*y),
                        ScrollOrientation::Horizontal => self.value_from_position(*x),
                    };
                    if new_value != self.value {
                        self.value = new_value.min(self.max_value);
                        self.notify_scroll();
                    }
                    return true;
                }

                // Check if over thumb
                let thumb = self.thumb_bounds();
                if thumb.contains(*x, *y) {
                    self.thumb_state = WidgetState::Hovered;
                } else {
                    self.thumb_state = WidgetState::Normal;
                }
                true
            }
            WidgetEvent::MouseDown { button: MouseButton::Left, x, y } => {
                let thumb = self.thumb_bounds();
                let track = self.track_bounds();

                if thumb.contains(*x, *y) {
                    // Start dragging thumb
                    self.dragging = true;
                    self.thumb_state = WidgetState::Pressed;
                    self.drag_offset = match self.orientation {
                        ScrollOrientation::Vertical => *y - thumb.y,
                        ScrollOrientation::Horizontal => *x - thumb.x,
                    };
                } else if track.contains(*x, *y) {
                    // Page up/down
                    match self.orientation {
                        ScrollOrientation::Vertical => {
                            if *y < thumb.y {
                                self.scroll_by(-(self.visible_amount as isize));
                            } else {
                                self.scroll_by(self.visible_amount as isize);
                            }
                        }
                        ScrollOrientation::Horizontal => {
                            if *x < thumb.x {
                                self.scroll_by(-(self.visible_amount as isize));
                            } else {
                                self.scroll_by(self.visible_amount as isize);
                            }
                        }
                    }
                } else {
                    // Check arrow buttons
                    let up_arrow = Bounds::new(
                        self.bounds.x,
                        self.bounds.y,
                        Self::ARROW_SIZE,
                        Self::ARROW_SIZE,
                    );
                    let down_arrow = match self.orientation {
                        ScrollOrientation::Vertical => Bounds::new(
                            self.bounds.x,
                            self.bounds.y + self.bounds.height as isize - Self::ARROW_SIZE as isize,
                            Self::ARROW_SIZE,
                            Self::ARROW_SIZE,
                        ),
                        ScrollOrientation::Horizontal => Bounds::new(
                            self.bounds.x + self.bounds.width as isize - Self::ARROW_SIZE as isize,
                            self.bounds.y,
                            Self::ARROW_SIZE,
                            Self::ARROW_SIZE,
                        ),
                    };

                    if up_arrow.contains(*x, *y) {
                        self.scroll_by(-1);
                    } else if down_arrow.contains(*x, *y) {
                        self.scroll_by(1);
                    }
                }
                true
            }
            WidgetEvent::MouseUp { button: MouseButton::Left, .. } => {
                self.dragging = false;
                self.thumb_state = WidgetState::Normal;
                true
            }
            WidgetEvent::Scroll { delta_y, .. } => {
                self.scroll_by(*delta_y as isize * 3);
                true
            }
            _ => false,
        }
    }

    fn render(&self, surface: &mut Surface) {
        if !self.visible {
            return;
        }

        let theme = theme();
        let x = self.bounds.x.max(0) as usize;
        let y = self.bounds.y.max(0) as usize;
        let w = self.bounds.width;
        let h = self.bounds.height;

        let bg_color = Color::new(45, 45, 53);
        let track_color = Color::new(35, 35, 43);
        let thumb_color = match self.thumb_state {
            WidgetState::Pressed => theme.accent,
            WidgetState::Hovered => Color::new(100, 100, 110),
            WidgetState::Disabled => Color::new(60, 60, 68),
            _ => Color::new(80, 80, 88),
        };
        let arrow_color = theme.fg;

        // Draw background
        for py in 0..h {
            for px in 0..w {
                surface.set_pixel(x + px, y + py, bg_color);
            }
        }

        // Draw track
        let track = self.track_bounds();
        let tx = track.x.max(0) as usize;
        let ty = track.y.max(0) as usize;
        for py in 0..track.height {
            for px in 0..track.width {
                surface.set_pixel(tx + px, ty + py, track_color);
            }
        }

        // Draw thumb
        let thumb = self.thumb_bounds();
        let thx = thumb.x.max(0) as usize;
        let thy = thumb.y.max(0) as usize;
        for py in 0..thumb.height {
            for px in 0..thumb.width {
                surface.set_pixel(thx + px, thy + py, thumb_color);
            }
        }

        // Draw arrows
        match self.orientation {
            ScrollOrientation::Vertical => {
                // Up arrow
                let arrow_cx = x + w / 2;
                let arrow_cy = y + Self::ARROW_SIZE / 2;
                for i in 0..5 {
                    for j in 0..=i {
                        surface.set_pixel(arrow_cx - j, arrow_cy + i, arrow_color);
                        surface.set_pixel(arrow_cx + j, arrow_cy + i, arrow_color);
                    }
                }

                // Down arrow
                let arrow_cy = y + h - Self::ARROW_SIZE / 2;
                for i in 0..5 {
                    for j in 0..=i {
                        surface.set_pixel(arrow_cx - j, arrow_cy - i, arrow_color);
                        surface.set_pixel(arrow_cx + j, arrow_cy - i, arrow_color);
                    }
                }
            }
            ScrollOrientation::Horizontal => {
                // Left arrow
                let arrow_cx = x + Self::ARROW_SIZE / 2;
                let arrow_cy = y + h / 2;
                for i in 0..5 {
                    for j in 0..=i {
                        surface.set_pixel(arrow_cx + i, arrow_cy - j, arrow_color);
                        surface.set_pixel(arrow_cx + i, arrow_cy + j, arrow_color);
                    }
                }

                // Right arrow
                let arrow_cx = x + w - Self::ARROW_SIZE / 2;
                for i in 0..5 {
                    for j in 0..=i {
                        surface.set_pixel(arrow_cx - i, arrow_cy - j, arrow_color);
                        surface.set_pixel(arrow_cx - i, arrow_cy + j, arrow_color);
                    }
                }
            }
        }
    }
}
