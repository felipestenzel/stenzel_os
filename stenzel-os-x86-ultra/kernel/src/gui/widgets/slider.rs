//! Slider widget
//!
//! An interactive slider for selecting values within a range.

use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use super::{Widget, WidgetId, WidgetEvent, Bounds, MouseButton, theme};

// Helper function for no_std
#[inline]
fn round_f32(x: f32) -> f32 {
    let xi = x as i32;
    let frac = x - xi as f32;
    if frac >= 0.5 {
        (xi + 1) as f32
    } else if frac <= -0.5 {
        (xi - 1) as f32
    } else {
        xi as f32
    }
}

/// Slider orientation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliderOrientation {
    Horizontal,
    Vertical,
}

/// Slider style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliderStyle {
    /// Classic slider with a rectangular thumb
    Classic,
    /// Modern slider with a circular thumb
    Modern,
    /// Flat style with a thin track
    Flat,
}

/// A slider widget for selecting values
pub struct Slider {
    id: WidgetId,
    bounds: Bounds,
    value: f32,
    min: f32,
    max: f32,
    step: f32,
    orientation: SliderOrientation,
    style: SliderStyle,
    enabled: bool,
    visible: bool,
    dragging: bool,
    hovered: bool,
    thumb_hovered: bool,
    show_value: bool,
    track_color: Option<Color>,
    thumb_color: Option<Color>,
    fill_color: Option<Color>,
    on_change: Option<fn(f32)>,
}

impl Slider {
    /// Create a new slider
    pub fn new(x: isize, y: isize, width: usize, height: usize) -> Self {
        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(x, y, width, height),
            value: 0.0,
            min: 0.0,
            max: 100.0,
            step: 1.0,
            orientation: SliderOrientation::Horizontal,
            style: SliderStyle::Modern,
            enabled: true,
            visible: true,
            dragging: false,
            hovered: false,
            thumb_hovered: false,
            show_value: false,
            track_color: None,
            thumb_color: None,
            fill_color: None,
            on_change: None,
        }
    }

    /// Create a horizontal slider
    pub fn horizontal(x: isize, y: isize, width: usize) -> Self {
        Self::new(x, y, width, 24)
    }

    /// Create a vertical slider
    pub fn vertical(x: isize, y: isize, height: usize) -> Self {
        let mut slider = Self::new(x, y, 24, height);
        slider.orientation = SliderOrientation::Vertical;
        slider
    }

    /// Set the value (clamped to range and snapped to step)
    pub fn set_value(&mut self, value: f32) {
        let clamped = value.clamp(self.min, self.max);
        // Snap to step
        let steps = round_f32((clamped - self.min) / self.step);
        self.value = self.min + steps * self.step;
        self.value = self.value.clamp(self.min, self.max);
    }

    /// Get current value
    pub fn value(&self) -> f32 {
        self.value
    }

    /// Set range
    pub fn set_range(&mut self, min: f32, max: f32) {
        self.min = min;
        self.max = max;
        self.set_value(self.value); // Re-clamp
    }

    /// Get minimum value
    pub fn min(&self) -> f32 {
        self.min
    }

    /// Get maximum value
    pub fn max(&self) -> f32 {
        self.max
    }

    /// Set step size
    pub fn set_step(&mut self, step: f32) {
        self.step = step.max(0.001);
    }

    /// Get step size
    pub fn step(&self) -> f32 {
        self.step
    }

    /// Set orientation
    pub fn set_orientation(&mut self, orientation: SliderOrientation) {
        self.orientation = orientation;
    }

    /// Set style
    pub fn set_style(&mut self, style: SliderStyle) {
        self.style = style;
    }

    /// Enable/disable value display
    pub fn set_show_value(&mut self, show: bool) {
        self.show_value = show;
    }

    /// Set track color
    pub fn set_track_color(&mut self, color: Color) {
        self.track_color = Some(color);
    }

    /// Set thumb color
    pub fn set_thumb_color(&mut self, color: Color) {
        self.thumb_color = Some(color);
    }

    /// Set fill color (the part of the track that's "filled")
    pub fn set_fill_color(&mut self, color: Color) {
        self.fill_color = Some(color);
    }

    /// Set change callback
    pub fn set_on_change(&mut self, callback: fn(f32)) {
        self.on_change = Some(callback);
    }

    /// Get normalized value (0.0 to 1.0)
    fn normalized(&self) -> f32 {
        if self.max <= self.min {
            return 0.0;
        }
        (self.value - self.min) / (self.max - self.min)
    }

    /// Get thumb bounds
    fn thumb_bounds(&self) -> Bounds {
        let thumb_size = self.thumb_size();
        let (thumb_x, thumb_y) = self.thumb_position();
        Bounds::new(thumb_x, thumb_y, thumb_size, thumb_size)
    }

    /// Get thumb size based on style and bounds
    fn thumb_size(&self) -> usize {
        match self.orientation {
            SliderOrientation::Horizontal => {
                match self.style {
                    SliderStyle::Classic => self.bounds.height.saturating_sub(4),
                    SliderStyle::Modern => self.bounds.height,
                    SliderStyle::Flat => (self.bounds.height as f32 * 1.5) as usize,
                }
            }
            SliderOrientation::Vertical => {
                match self.style {
                    SliderStyle::Classic => self.bounds.width.saturating_sub(4),
                    SliderStyle::Modern => self.bounds.width,
                    SliderStyle::Flat => (self.bounds.width as f32 * 1.5) as usize,
                }
            }
        }
    }

    /// Get thumb position
    fn thumb_position(&self) -> (isize, isize) {
        let thumb_size = self.thumb_size();
        let norm = self.normalized();

        match self.orientation {
            SliderOrientation::Horizontal => {
                let track_width = self.bounds.width.saturating_sub(thumb_size);
                let thumb_x = self.bounds.x + (track_width as f32 * norm) as isize;
                let thumb_y = self.bounds.y + (self.bounds.height as isize - thumb_size as isize) / 2;
                (thumb_x, thumb_y)
            }
            SliderOrientation::Vertical => {
                let track_height = self.bounds.height.saturating_sub(thumb_size);
                let thumb_x = self.bounds.x + (self.bounds.width as isize - thumb_size as isize) / 2;
                // Inverted for vertical (0 at bottom)
                let thumb_y = self.bounds.y + (track_height as f32 * (1.0 - norm)) as isize;
                (thumb_x, thumb_y)
            }
        }
    }

    /// Update value from mouse position
    fn update_from_position(&mut self, x: isize, y: isize) {
        let thumb_size = self.thumb_size();

        let norm = match self.orientation {
            SliderOrientation::Horizontal => {
                let track_width = self.bounds.width.saturating_sub(thumb_size) as f32;
                let rel_x = (x - self.bounds.x - thumb_size as isize / 2) as f32;
                (rel_x / track_width).clamp(0.0, 1.0)
            }
            SliderOrientation::Vertical => {
                let track_height = self.bounds.height.saturating_sub(thumb_size) as f32;
                let rel_y = (y - self.bounds.y - thumb_size as isize / 2) as f32;
                // Inverted for vertical
                1.0 - (rel_y / track_height).clamp(0.0, 1.0)
            }
        };

        let old_value = self.value;
        let new_value = self.min + norm * (self.max - self.min);
        self.set_value(new_value);

        // Trigger callback if value changed
        if (self.value - old_value).abs() > 0.0001 {
            if let Some(callback) = self.on_change {
                callback(self.value);
            }
        }
    }
}

impl Widget for Slider {
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
            self.dragging = false;
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
                self.hovered = true;
                true
            }
            WidgetEvent::MouseLeave => {
                self.hovered = false;
                self.thumb_hovered = false;
                true
            }
            WidgetEvent::MouseMove { x, y } => {
                // Check if thumb is hovered
                let thumb_bounds = self.thumb_bounds();
                self.thumb_hovered = thumb_bounds.contains(*x, *y);

                if self.dragging {
                    self.update_from_position(*x, *y);
                    return true;
                }
                self.hovered
            }
            WidgetEvent::MouseDown { button: MouseButton::Left, x, y } => {
                if self.bounds.contains(*x, *y) {
                    self.dragging = true;
                    // Jump to click position
                    self.update_from_position(*x, *y);
                    return true;
                }
                false
            }
            WidgetEvent::MouseUp { button: MouseButton::Left, .. } => {
                if self.dragging {
                    self.dragging = false;
                    return true;
                }
                false
            }
            WidgetEvent::KeyDown { key, .. } => {
                // Arrow keys change value
                match *key {
                    0x4B | 0x25 => { // Left arrow
                        if self.orientation == SliderOrientation::Horizontal {
                            self.set_value(self.value - self.step);
                            if let Some(cb) = self.on_change { cb(self.value); }
                            return true;
                        }
                    }
                    0x4D | 0x27 => { // Right arrow
                        if self.orientation == SliderOrientation::Horizontal {
                            self.set_value(self.value + self.step);
                            if let Some(cb) = self.on_change { cb(self.value); }
                            return true;
                        }
                    }
                    0x48 | 0x26 => { // Up arrow
                        if self.orientation == SliderOrientation::Vertical {
                            self.set_value(self.value + self.step);
                            if let Some(cb) = self.on_change { cb(self.value); }
                            return true;
                        }
                    }
                    0x50 | 0x28 => { // Down arrow
                        if self.orientation == SliderOrientation::Vertical {
                            self.set_value(self.value - self.step);
                            if let Some(cb) = self.on_change { cb(self.value); }
                            return true;
                        }
                    }
                    _ => {}
                }
                false
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

        let track_color = self.track_color.unwrap_or(Color::new(40, 40, 48));
        let fill_color = self.fill_color.unwrap_or(theme.accent);
        let thumb_color = if !self.enabled {
            theme.fg_disabled
        } else if self.dragging {
            theme.bg_pressed
        } else if self.thumb_hovered {
            theme.bg_hover
        } else {
            self.thumb_color.unwrap_or(Color::WHITE)
        };

        match self.style {
            SliderStyle::Classic => self.render_classic(surface, x, y, w, h, track_color, fill_color, thumb_color),
            SliderStyle::Modern => self.render_modern(surface, x, y, w, h, track_color, fill_color, thumb_color),
            SliderStyle::Flat => self.render_flat(surface, x, y, w, h, track_color, fill_color, thumb_color),
        }

        // Draw value text if enabled
        if self.show_value {
            let text = format_value(self.value);
            let char_width = 8;
            let text_width = text.len() * char_width;

            let (text_x, text_y) = match self.orientation {
                SliderOrientation::Horizontal => {
                    (x + w + 8, y + (h - 16) / 2)
                }
                SliderOrientation::Vertical => {
                    (x + (w - text_width) / 2, y + h + 4)
                }
            };

            for (i, c) in text.chars().enumerate() {
                draw_char_simple(surface, text_x + i * char_width, text_y, c, theme.fg);
            }
        }
    }
}

impl Slider {
    fn render_classic(&self, surface: &mut Surface, x: usize, y: usize, w: usize, h: usize,
                      track_color: Color, fill_color: Color, thumb_color: Color) {
        let theme = theme();

        match self.orientation {
            SliderOrientation::Horizontal => {
                let track_height = 6;
                let track_y = y + (h - track_height) / 2;

                // Draw track background
                for py in 0..track_height {
                    for px in 0..w {
                        surface.set_pixel(x + px, track_y + py, track_color);
                    }
                }

                // Draw filled portion
                let fill_width = (w as f32 * self.normalized()) as usize;
                for py in 0..track_height {
                    for px in 0..fill_width {
                        surface.set_pixel(x + px, track_y + py, fill_color);
                    }
                }

                // Draw thumb
                let (thumb_x, thumb_y) = self.thumb_position();
                let thumb_size = self.thumb_size();
                let tx = thumb_x.max(0) as usize;
                let ty = thumb_y.max(0) as usize;

                // Thumb border
                for py in 0..thumb_size {
                    for px in 0..thumb_size {
                        let on_border = py == 0 || py == thumb_size - 1 || px == 0 || px == thumb_size - 1;
                        let color = if on_border { theme.border } else { thumb_color };
                        surface.set_pixel(tx + px, ty + py, color);
                    }
                }
            }
            SliderOrientation::Vertical => {
                let track_width = 6;
                let track_x = x + (w - track_width) / 2;

                // Draw track background
                for py in 0..h {
                    for px in 0..track_width {
                        surface.set_pixel(track_x + px, y + py, track_color);
                    }
                }

                // Draw filled portion (from bottom)
                let fill_height = (h as f32 * self.normalized()) as usize;
                let fill_start = h.saturating_sub(fill_height);
                for py in fill_start..h {
                    for px in 0..track_width {
                        surface.set_pixel(track_x + px, y + py, fill_color);
                    }
                }

                // Draw thumb
                let (thumb_x, thumb_y) = self.thumb_position();
                let thumb_size = self.thumb_size();
                let tx = thumb_x.max(0) as usize;
                let ty = thumb_y.max(0) as usize;

                for py in 0..thumb_size {
                    for px in 0..thumb_size {
                        let on_border = py == 0 || py == thumb_size - 1 || px == 0 || px == thumb_size - 1;
                        let color = if on_border { theme.border } else { thumb_color };
                        surface.set_pixel(tx + px, ty + py, color);
                    }
                }
            }
        }
    }

    fn render_modern(&self, surface: &mut Surface, x: usize, y: usize, w: usize, h: usize,
                     track_color: Color, fill_color: Color, thumb_color: Color) {
        match self.orientation {
            SliderOrientation::Horizontal => {
                let track_height = 4;
                let track_y = y + (h - track_height) / 2;
                let thumb_size = self.thumb_size();
                let thumb_radius = thumb_size / 2;

                // Draw track with rounded ends (simplified)
                for py in 0..track_height {
                    for px in 0..w {
                        surface.set_pixel(x + px, track_y + py, track_color);
                    }
                }

                // Draw filled portion
                let fill_width = (w as f32 * self.normalized()) as usize;
                for py in 0..track_height {
                    for px in 0..fill_width {
                        surface.set_pixel(x + px, track_y + py, fill_color);
                    }
                }

                // Draw circular thumb
                let (thumb_x, thumb_y) = self.thumb_position();
                let cx = thumb_x + thumb_radius as isize;
                let cy = thumb_y + thumb_radius as isize;

                self.draw_circle(surface, cx, cy, thumb_radius, thumb_color);
            }
            SliderOrientation::Vertical => {
                let track_width = 4;
                let track_x = x + (w - track_width) / 2;
                let thumb_size = self.thumb_size();
                let thumb_radius = thumb_size / 2;

                // Draw track
                for py in 0..h {
                    for px in 0..track_width {
                        surface.set_pixel(track_x + px, y + py, track_color);
                    }
                }

                // Draw filled portion
                let fill_height = (h as f32 * self.normalized()) as usize;
                let fill_start = h.saturating_sub(fill_height);
                for py in fill_start..h {
                    for px in 0..track_width {
                        surface.set_pixel(track_x + px, y + py, fill_color);
                    }
                }

                // Draw circular thumb
                let (thumb_x, thumb_y) = self.thumb_position();
                let cx = thumb_x + thumb_radius as isize;
                let cy = thumb_y + thumb_radius as isize;

                self.draw_circle(surface, cx, cy, thumb_radius, thumb_color);
            }
        }
    }

    fn render_flat(&self, surface: &mut Surface, x: usize, y: usize, w: usize, h: usize,
                   track_color: Color, fill_color: Color, thumb_color: Color) {
        match self.orientation {
            SliderOrientation::Horizontal => {
                let track_height = 2;
                let track_y = y + (h - track_height) / 2;

                // Draw track
                for py in 0..track_height {
                    for px in 0..w {
                        surface.set_pixel(x + px, track_y + py, track_color);
                    }
                }

                // Draw filled portion
                let fill_width = (w as f32 * self.normalized()) as usize;
                for py in 0..track_height {
                    for px in 0..fill_width {
                        surface.set_pixel(x + px, track_y + py, fill_color);
                    }
                }

                // Draw small circular thumb
                let (thumb_x, thumb_y) = self.thumb_position();
                let thumb_size = self.thumb_size();
                let cx = thumb_x + (thumb_size / 2) as isize;
                let cy = thumb_y + (thumb_size / 2) as isize;
                let radius = thumb_size / 3;

                self.draw_circle(surface, cx, cy, radius, thumb_color);
            }
            SliderOrientation::Vertical => {
                let track_width = 2;
                let track_x = x + (w - track_width) / 2;

                // Draw track
                for py in 0..h {
                    for px in 0..track_width {
                        surface.set_pixel(track_x + px, y + py, track_color);
                    }
                }

                // Draw filled portion
                let fill_height = (h as f32 * self.normalized()) as usize;
                let fill_start = h.saturating_sub(fill_height);
                for py in fill_start..h {
                    for px in 0..track_width {
                        surface.set_pixel(track_x + px, y + py, fill_color);
                    }
                }

                // Draw small circular thumb
                let (thumb_x, thumb_y) = self.thumb_position();
                let thumb_size = self.thumb_size();
                let cx = thumb_x + (thumb_size / 2) as isize;
                let cy = thumb_y + (thumb_size / 2) as isize;
                let radius = thumb_size / 3;

                self.draw_circle(surface, cx, cy, radius, thumb_color);
            }
        }
    }

    /// Draw a filled circle (simplified midpoint circle algorithm)
    fn draw_circle(&self, surface: &mut Surface, cx: isize, cy: isize, radius: usize, color: Color) {
        let r = radius as isize;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy <= r * r {
                    let px = cx + dx;
                    let py = cy + dy;
                    if px >= 0 && py >= 0 {
                        surface.set_pixel(px as usize, py as usize, color);
                    }
                }
            }
        }
    }
}

fn format_value(value: f32) -> alloc::string::String {
    use alloc::string::ToString;
    use alloc::string::String;

    // Simple integer formatting
    let int_val = value as i32;
    let frac = ((value - int_val as f32).abs() * 10.0) as i32;

    if frac == 0 {
        int_val.to_string()
    } else {
        let mut s = int_val.to_string();
        s.push('.');
        s.push_str(&frac.to_string());
        s
    }
}

fn draw_char_simple(surface: &mut Surface, x: usize, y: usize, c: char, color: Color) {
    use crate::drivers::font::DEFAULT_FONT;

    if let Some(glyph) = DEFAULT_FONT.get_glyph(c) {
        for row in 0..DEFAULT_FONT.height {
            let byte = glyph[row];
            for col in 0..DEFAULT_FONT.width {
                if (byte >> (DEFAULT_FONT.width - 1 - col)) & 1 != 0 {
                    surface.set_pixel(x + col, y + row, color);
                }
            }
        }
    }
}
