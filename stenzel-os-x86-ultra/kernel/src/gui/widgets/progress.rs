//! Progress Bar widget
//!
//! A progress bar for showing completion status.

use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use super::{Widget, WidgetId, WidgetEvent, Bounds, theme};

/// Progress bar style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressStyle {
    /// Horizontal bar (default)
    Horizontal,
    /// Vertical bar
    Vertical,
    /// Indeterminate (animated, no specific progress)
    Indeterminate,
}

/// A progress bar widget
pub struct ProgressBar {
    id: WidgetId,
    bounds: Bounds,
    value: f32,      // 0.0 to 1.0
    min: f32,
    max: f32,
    style: ProgressStyle,
    show_text: bool,
    color: Option<Color>,
    visible: bool,
    indeterminate_pos: usize, // For animation
}

impl ProgressBar {
    /// Create a new progress bar
    pub fn new(x: isize, y: isize, width: usize, height: usize) -> Self {
        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(x, y, width, height),
            value: 0.0,
            min: 0.0,
            max: 1.0,
            style: ProgressStyle::Horizontal,
            show_text: false,
            color: None,
            visible: true,
            indeterminate_pos: 0,
        }
    }

    /// Create a horizontal progress bar
    pub fn horizontal(x: isize, y: isize, width: usize) -> Self {
        Self::new(x, y, width, 20)
    }

    /// Create a vertical progress bar
    pub fn vertical(x: isize, y: isize, height: usize) -> Self {
        let mut bar = Self::new(x, y, 20, height);
        bar.style = ProgressStyle::Vertical;
        bar
    }

    /// Set value (clamped to min/max)
    pub fn set_value(&mut self, value: f32) {
        self.value = value.clamp(self.min, self.max);
    }

    /// Get current value
    pub fn value(&self) -> f32 {
        self.value
    }

    /// Set range
    pub fn set_range(&mut self, min: f32, max: f32) {
        self.min = min;
        self.max = max;
        self.value = self.value.clamp(min, max);
    }

    /// Get progress as percentage (0.0 to 1.0)
    pub fn progress(&self) -> f32 {
        if self.max <= self.min {
            return 0.0;
        }
        (self.value - self.min) / (self.max - self.min)
    }

    /// Set progress as percentage
    pub fn set_progress(&mut self, progress: f32) {
        let progress = progress.clamp(0.0, 1.0);
        self.value = self.min + progress * (self.max - self.min);
    }

    /// Increment value
    pub fn increment(&mut self, amount: f32) {
        self.set_value(self.value + amount);
    }

    /// Set style
    pub fn set_style(&mut self, style: ProgressStyle) {
        self.style = style;
    }

    /// Enable/disable text display
    pub fn set_show_text(&mut self, show: bool) {
        self.show_text = show;
    }

    /// Set custom progress color
    pub fn set_color(&mut self, color: Color) {
        self.color = Some(color);
    }

    /// Advance indeterminate animation
    pub fn tick(&mut self) {
        if self.style == ProgressStyle::Indeterminate {
            self.indeterminate_pos = (self.indeterminate_pos + 2) % (self.bounds.width * 2);
        }
    }
}

impl Widget for ProgressBar {
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
        true
    }

    fn set_enabled(&mut self, _enabled: bool) {
        // Progress bars don't have enabled state
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn handle_event(&mut self, _event: &WidgetEvent) -> bool {
        false // Progress bars don't handle events
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

        let bg_color = Color::new(40, 40, 48);
        let progress_color = self.color.unwrap_or(theme.accent);
        let border_color = theme.border;

        // Draw background
        for py in 1..h.saturating_sub(1) {
            for px in 1..w.saturating_sub(1) {
                surface.set_pixel(x + px, y + py, bg_color);
            }
        }

        // Draw border
        for px in 0..w {
            surface.set_pixel(x + px, y, border_color);
            surface.set_pixel(x + px, y + h.saturating_sub(1), border_color);
        }
        for py in 0..h {
            surface.set_pixel(x, y + py, border_color);
            surface.set_pixel(x + w.saturating_sub(1), y + py, border_color);
        }

        // Draw progress
        match self.style {
            ProgressStyle::Horizontal => {
                let progress_width = ((w - 4) as f32 * self.progress()) as usize;
                for py in 2..h.saturating_sub(2) {
                    for px in 2..2 + progress_width {
                        surface.set_pixel(x + px, y + py, progress_color);
                    }
                }
            }
            ProgressStyle::Vertical => {
                let progress_height = ((h - 4) as f32 * self.progress()) as usize;
                let start_y = h - 2 - progress_height;
                for py in start_y..h.saturating_sub(2) {
                    for px in 2..w.saturating_sub(2) {
                        surface.set_pixel(x + px, y + py, progress_color);
                    }
                }
            }
            ProgressStyle::Indeterminate => {
                // Animated sliding block
                let block_width = w / 4;
                let pos = self.indeterminate_pos;

                // Calculate position with bounce effect
                let total_travel = w - 4 - block_width;
                let cycle_pos = pos % (total_travel * 2);
                let block_x = if cycle_pos < total_travel {
                    cycle_pos
                } else {
                    total_travel * 2 - cycle_pos
                };

                for py in 2..h.saturating_sub(2) {
                    for px in 0..block_width {
                        let draw_x = x + 2 + block_x + px;
                        if draw_x < x + w - 2 {
                            surface.set_pixel(draw_x, y + py, progress_color);
                        }
                    }
                }
            }
        }

        // Draw percentage text if enabled
        if self.show_text && self.style != ProgressStyle::Indeterminate {
            let percent = (self.progress() * 100.0) as u32;
            let text = format_percent(percent);

            let char_width = 8;
            let text_width = text.len() * char_width;
            let text_x = x + (w - text_width) / 2;
            let text_y = y + (h - 16) / 2;

            // Draw text with contrasting color
            for (i, c) in text.chars().enumerate() {
                draw_char_simple(surface, text_x + i * char_width, text_y, c, Color::WHITE);
            }
        }
    }
}

fn format_percent(percent: u32) -> alloc::string::String {
    use alloc::string::ToString;
    let mut s = percent.to_string();
    s.push('%');
    s
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
