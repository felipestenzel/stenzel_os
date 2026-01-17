//! Volume Control Widget
//!
//! A popup widget for controlling system volume with a slider,
//! mute button, and visual feedback.

use alloc::string::String;
use alloc::vec::Vec;
use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use super::{Widget, WidgetId, WidgetState, WidgetEvent, Bounds, MouseButton, theme};

/// Volume change callback
pub type VolumeChangeCallback = fn(u8, bool);

/// Audio output device
#[derive(Debug, Clone)]
pub struct AudioOutput {
    /// Device ID
    pub id: u32,
    /// Device name
    pub name: String,
    /// Whether this is the default device
    pub is_default: bool,
    /// Current volume (0-100)
    pub volume: u8,
    /// Whether muted
    pub muted: bool,
}

impl AudioOutput {
    /// Create a new audio output
    pub fn new(id: u32, name: &str) -> Self {
        Self {
            id,
            name: String::from(name),
            is_default: false,
            volume: 50,
            muted: false,
        }
    }
}

/// Volume control popup widget
pub struct VolumeControl {
    id: WidgetId,
    bounds: Bounds,

    /// Current volume (0-100)
    volume: u8,
    /// Whether muted
    muted: bool,
    /// Available audio outputs
    outputs: Vec<AudioOutput>,
    /// Selected output index
    selected_output: usize,

    /// Whether the popup is visible
    visible: bool,
    /// Widget state
    state: WidgetState,
    /// Slider being dragged
    dragging_slider: bool,
    /// Hover state
    hovered_area: HoveredArea,

    /// Callback when volume changes
    on_volume_change: Option<VolumeChangeCallback>,

    /// Colors
    bg_color: Color,
    slider_bg_color: Color,
    slider_fill_color: Color,
    slider_thumb_color: Color,
    text_color: Color,
    mute_color: Color,
    icon_color: Color,
}

/// Area being hovered
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HoveredArea {
    None,
    Slider,
    MuteButton,
    OutputItem(usize),
    CloseButton,
}

impl VolumeControl {
    /// Popup width
    const WIDTH: usize = 280;
    /// Popup height (varies with output count)
    const BASE_HEIGHT: usize = 160;
    /// Output item height
    const OUTPUT_HEIGHT: usize = 32;
    /// Slider height
    const SLIDER_HEIGHT: usize = 8;
    /// Thumb size
    const THUMB_SIZE: usize = 16;
    /// Padding
    const PADDING: usize = 16;

    /// Create a new volume control popup
    pub fn new(x: isize, y: isize) -> Self {
        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(x, y, Self::WIDTH, Self::BASE_HEIGHT),
            volume: 50,
            muted: false,
            outputs: Vec::new(),
            selected_output: 0,
            visible: false,
            state: WidgetState::Normal,
            dragging_slider: false,
            hovered_area: HoveredArea::None,
            on_volume_change: None,
            bg_color: Color::new(40, 40, 48),
            slider_bg_color: Color::new(60, 60, 68),
            slider_fill_color: Color::new(0, 120, 215),
            slider_thumb_color: Color::WHITE,
            text_color: Color::WHITE,
            mute_color: Color::new(255, 100, 100),
            icon_color: Color::WHITE,
        }
    }

    /// Show the popup
    pub fn show(&mut self) {
        self.visible = true;
        self.update_height();
    }

    /// Hide the popup
    pub fn hide(&mut self) {
        self.visible = false;
        self.dragging_slider = false;
    }

    /// Toggle visibility
    pub fn toggle(&mut self) {
        if self.visible {
            self.hide();
        } else {
            self.show();
        }
    }

    /// Set volume (0-100)
    pub fn set_volume(&mut self, volume: u8) {
        self.volume = volume.min(100);
        if let Some(output) = self.outputs.get_mut(self.selected_output) {
            output.volume = self.volume;
        }
    }

    /// Get current volume
    pub fn volume(&self) -> u8 {
        self.volume
    }

    /// Set muted state
    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
        if let Some(output) = self.outputs.get_mut(self.selected_output) {
            output.muted = muted;
        }
    }

    /// Toggle mute
    pub fn toggle_mute(&mut self) {
        self.set_muted(!self.muted);
        if let Some(callback) = self.on_volume_change {
            callback(self.volume, self.muted);
        }
    }

    /// Check if muted
    pub fn is_muted(&self) -> bool {
        self.muted
    }

    /// Add an audio output device
    pub fn add_output(&mut self, output: AudioOutput) {
        if output.is_default {
            self.selected_output = self.outputs.len();
            self.volume = output.volume;
            self.muted = output.muted;
        }
        self.outputs.push(output);
        self.update_height();
    }

    /// Remove an audio output
    pub fn remove_output(&mut self, id: u32) {
        if let Some(idx) = self.outputs.iter().position(|o| o.id == id) {
            self.outputs.remove(idx);
            if self.selected_output >= self.outputs.len() && !self.outputs.is_empty() {
                self.selected_output = self.outputs.len() - 1;
            }
            self.update_height();
        }
    }

    /// Select an output device
    pub fn select_output(&mut self, index: usize) {
        if index < self.outputs.len() {
            self.selected_output = index;
            if let Some(output) = self.outputs.get(index) {
                self.volume = output.volume;
                self.muted = output.muted;
            }
        }
    }

    /// Set volume change callback
    pub fn set_on_volume_change(&mut self, callback: VolumeChangeCallback) {
        self.on_volume_change = Some(callback);
    }

    /// Update height based on output count
    fn update_height(&mut self) {
        let outputs_height = if self.outputs.len() > 1 {
            (self.outputs.len() * Self::OUTPUT_HEIGHT) + Self::PADDING
        } else {
            0
        };
        self.bounds.height = Self::BASE_HEIGHT + outputs_height;
    }

    /// Get slider bounds
    fn slider_bounds(&self) -> Bounds {
        let x = self.bounds.x + Self::PADDING as isize + 40; // After mute button
        let y = self.bounds.y + 60;
        let width = self.bounds.width - Self::PADDING * 2 - 50;
        Bounds::new(x, y, width, Self::SLIDER_HEIGHT)
    }

    /// Get mute button bounds
    fn mute_button_bounds(&self) -> Bounds {
        let x = self.bounds.x + Self::PADDING as isize;
        let y = self.bounds.y + 52;
        Bounds::new(x, y, 32, 24)
    }

    /// Get output item bounds
    fn output_bounds(&self, index: usize) -> Bounds {
        let x = self.bounds.x + Self::PADDING as isize;
        let y = self.bounds.y + 100 + (index * Self::OUTPUT_HEIGHT) as isize;
        let width = self.bounds.width - Self::PADDING * 2;
        Bounds::new(x, y, width, Self::OUTPUT_HEIGHT - 4)
    }

    /// Get thumb position from volume
    fn thumb_position(&self) -> isize {
        let slider = self.slider_bounds();
        let track_width = slider.width.saturating_sub(Self::THUMB_SIZE);
        let position = (self.volume as usize * track_width) / 100;
        slider.x + position as isize
    }

    /// Update volume from mouse x position
    fn update_volume_from_position(&mut self, x: isize) {
        let slider = self.slider_bounds();
        let rel_x = (x - slider.x).max(0) as usize;
        let track_width = slider.width.saturating_sub(Self::THUMB_SIZE);
        let new_volume = ((rel_x * 100) / track_width.max(1)).min(100) as u8;

        if new_volume != self.volume {
            self.set_volume(new_volume);
            if let Some(callback) = self.on_volume_change {
                callback(self.volume, self.muted);
            }
        }
    }

    /// Draw the volume icon
    fn draw_volume_icon(&self, surface: &mut Surface, x: usize, y: usize, size: usize) {
        let color = if self.muted { self.mute_color } else { self.icon_color };

        // Speaker shape
        let sx = x + 2;
        let sy = y + size / 4;
        let speaker_h = size / 2;

        // Speaker body
        for py in 0..speaker_h {
            surface.set_pixel(sx, sy + py, color);
            surface.set_pixel(sx + 1, sy + py, color);
        }

        // Cone
        for py in 0..size {
            let offset = ((py as isize - size as isize / 2).abs() as usize).min(size / 3);
            for px in 0..offset {
                surface.set_pixel(sx + 2 + px, y + py, color);
            }
        }

        if self.muted {
            // Draw X
            for i in 0..8 {
                let mx = x + size / 2 + i;
                let my1 = y + 2 + i;
                let my2 = y + size - 3 - i;
                surface.set_pixel(mx, my1, self.mute_color);
                surface.set_pixel(mx, my2, self.mute_color);
            }
        } else {
            // Sound waves based on volume
            let waves = if self.volume == 0 {
                0
            } else if self.volume <= 33 {
                1
            } else if self.volume <= 66 {
                2
            } else {
                3
            };

            let wave_x = x + size / 2 + 2;
            for w in 0..waves {
                let wave_h = (w + 1) * 4;
                let start_y = y + (size - wave_h) / 2;
                for py in 0..wave_h {
                    surface.set_pixel(wave_x + w * 3, start_y + py, color);
                }
            }
        }
    }

    /// Draw percentage text
    fn draw_percentage(&self, surface: &mut Surface, x: usize, y: usize) {
        let text = format_percentage(self.volume);
        for (i, c) in text.chars().enumerate() {
            draw_char(surface, x + i * 8, y, c, self.text_color);
        }
    }
}

impl Widget for VolumeControl {
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

    fn set_enabled(&mut self, _enabled: bool) {}

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn handle_event(&mut self, event: &WidgetEvent) -> bool {
        if !self.visible {
            return false;
        }

        match event {
            WidgetEvent::MouseMove { x, y } => {
                // Update hovered area
                let mute_bounds = self.mute_button_bounds();
                let slider_bounds = self.slider_bounds();

                if mute_bounds.contains(*x, *y) {
                    self.hovered_area = HoveredArea::MuteButton;
                } else if slider_bounds.contains(*x, *y) ||
                    (slider_bounds.x <= *x && *x <= slider_bounds.x + slider_bounds.width as isize &&
                     slider_bounds.y - 8 <= *y && *y <= slider_bounds.y + slider_bounds.height as isize + 8) {
                    self.hovered_area = HoveredArea::Slider;
                } else {
                    // Check output items
                    let mut found = false;
                    for i in 0..self.outputs.len() {
                        if self.output_bounds(i).contains(*x, *y) {
                            self.hovered_area = HoveredArea::OutputItem(i);
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        self.hovered_area = HoveredArea::None;
                    }
                }

                // Handle slider dragging
                if self.dragging_slider {
                    self.update_volume_from_position(*x);
                    return true;
                }

                true
            }
            WidgetEvent::MouseDown { button: MouseButton::Left, x, y } => {
                let mute_bounds = self.mute_button_bounds();
                let slider_bounds = self.slider_bounds();

                if mute_bounds.contains(*x, *y) {
                    self.toggle_mute();
                    return true;
                }

                // Check if clicking on slider area
                let extended_slider = Bounds::new(
                    slider_bounds.x,
                    slider_bounds.y - 8,
                    slider_bounds.width,
                    slider_bounds.height + 16,
                );
                if extended_slider.contains(*x, *y) {
                    self.dragging_slider = true;
                    self.update_volume_from_position(*x);
                    return true;
                }

                // Check output items
                for i in 0..self.outputs.len() {
                    if self.output_bounds(i).contains(*x, *y) {
                        self.select_output(i);
                        return true;
                    }
                }

                // Click outside to close
                if !self.bounds.contains(*x, *y) {
                    self.hide();
                    return true;
                }

                true
            }
            WidgetEvent::MouseUp { button: MouseButton::Left, .. } => {
                if self.dragging_slider {
                    self.dragging_slider = false;
                    return true;
                }
                false
            }
            WidgetEvent::Scroll { delta_y, .. } => {
                // Scroll to change volume
                let change = (*delta_y as i32 * 5).clamp(-100, 100);
                let new_vol = (self.volume as i32 + change).clamp(0, 100) as u8;
                self.set_volume(new_vol);
                if let Some(callback) = self.on_volume_change {
                    callback(self.volume, self.muted);
                }
                true
            }
            WidgetEvent::Blur => {
                self.hide();
                true
            }
            _ => false,
        }
    }

    fn render(&self, surface: &mut Surface) {
        if !self.visible {
            return;
        }

        let x = self.bounds.x.max(0) as usize;
        let y = self.bounds.y.max(0) as usize;
        let w = self.bounds.width;
        let h = self.bounds.height;

        // Draw popup background with shadow
        for py in 0..h + 4 {
            for px in 0..w + 4 {
                if px >= 4 && py >= 4 {
                    surface.set_pixel(x + px - 4, y + py - 4, self.bg_color);
                } else {
                    // Shadow
                    surface.set_pixel(x + px, y + py, Color::new(0, 0, 0));
                }
            }
        }

        // Draw border
        let border_color = Color::new(80, 80, 90);
        for px in 0..w {
            surface.set_pixel(x + px, y, border_color);
            surface.set_pixel(x + px, y + h - 1, border_color);
        }
        for py in 0..h {
            surface.set_pixel(x, y + py, border_color);
            surface.set_pixel(x + w - 1, y + py, border_color);
        }

        // Title
        let title = "Volume";
        let title_x = x + Self::PADDING;
        let title_y = y + 12;
        for (i, c) in title.chars().enumerate() {
            draw_char(surface, title_x + i * 8, title_y, c, self.text_color);
        }

        // Volume icon
        self.draw_volume_icon(surface, x + Self::PADDING, y + 48, 24);

        // Mute button highlight
        if self.hovered_area == HoveredArea::MuteButton {
            let mb = self.mute_button_bounds();
            let mbx = mb.x.max(0) as usize;
            let mby = mb.y.max(0) as usize;
            for py in 0..mb.height {
                for px in 0..mb.width {
                    surface.set_pixel(mbx + px, mby + py, Color::new(60, 60, 70));
                }
            }
            self.draw_volume_icon(surface, mbx, mby, 24);
        }

        // Slider track
        let slider = self.slider_bounds();
        let sx = slider.x.max(0) as usize;
        let sy = slider.y.max(0) as usize;

        // Track background
        for py in 0..Self::SLIDER_HEIGHT {
            for px in 0..slider.width {
                surface.set_pixel(sx + px, sy + py, self.slider_bg_color);
            }
        }

        // Filled portion
        let fill_width = (self.volume as usize * slider.width) / 100;
        let fill_color = if self.muted {
            Color::new(80, 80, 90)
        } else {
            self.slider_fill_color
        };
        for py in 0..Self::SLIDER_HEIGHT {
            for px in 0..fill_width {
                surface.set_pixel(sx + px, sy + py, fill_color);
            }
        }

        // Thumb
        let thumb_x = self.thumb_position().max(0) as usize;
        let thumb_y = sy.saturating_sub(4);
        let thumb_color = if self.dragging_slider || self.hovered_area == HoveredArea::Slider {
            Color::new(200, 200, 220)
        } else {
            self.slider_thumb_color
        };

        // Draw circular thumb
        let thumb_cx = thumb_x + Self::THUMB_SIZE / 2;
        let thumb_cy = thumb_y + Self::THUMB_SIZE / 2;
        let radius = Self::THUMB_SIZE / 2;

        for dy in 0..Self::THUMB_SIZE {
            for dx in 0..Self::THUMB_SIZE {
                let dist_x = (dx as isize - radius as isize).abs();
                let dist_y = (dy as isize - radius as isize).abs();
                if dist_x * dist_x + dist_y * dist_y <= (radius * radius) as isize {
                    surface.set_pixel(thumb_x + dx, thumb_y + dy, thumb_color);
                }
            }
        }

        // Volume percentage
        let pct_x = sx + slider.width + 12;
        let pct_y = sy.saturating_sub(2);
        self.draw_percentage(surface, pct_x, pct_y);

        // Output devices (if more than one)
        if self.outputs.len() > 1 {
            let divider_y = y + 92;
            for px in Self::PADDING..(w - Self::PADDING) {
                surface.set_pixel(x + px, divider_y, border_color);
            }

            for (i, output) in self.outputs.iter().enumerate() {
                let ob = self.output_bounds(i);
                let obx = ob.x.max(0) as usize;
                let oby = ob.y.max(0) as usize;

                // Highlight selected or hovered
                let is_selected = i == self.selected_output;
                let is_hovered = self.hovered_area == HoveredArea::OutputItem(i);

                if is_selected || is_hovered {
                    let bg = if is_selected {
                        self.slider_fill_color
                    } else {
                        Color::new(60, 60, 70)
                    };
                    for py in 0..ob.height {
                        for px in 0..ob.width {
                            surface.set_pixel(obx + px, oby + py, bg);
                        }
                    }
                }

                // Output icon (speaker)
                let icon_size = 16;
                let icon_y = oby + (ob.height - icon_size) / 2;
                for py in 0..icon_size {
                    for px in 0..4 {
                        let offset = ((py as isize - 8).abs() as usize).min(3);
                        if px <= offset {
                            surface.set_pixel(obx + 4 + px, icon_y + py, self.text_color);
                        }
                    }
                }

                // Output name
                let name_x = obx + 28;
                let name_y = oby + (ob.height - 12) / 2;
                for (ci, c) in output.name.chars().take(28).enumerate() {
                    draw_char(surface, name_x + ci * 8, name_y, c, self.text_color);
                }

                // Default indicator
                if output.is_default {
                    let check_x = obx + ob.width - 20;
                    let check_y = oby + ob.height / 2 - 4;
                    // Simple checkmark
                    surface.set_pixel(check_x, check_y + 4, Color::new(100, 255, 100));
                    surface.set_pixel(check_x + 1, check_y + 5, Color::new(100, 255, 100));
                    surface.set_pixel(check_x + 2, check_y + 6, Color::new(100, 255, 100));
                    surface.set_pixel(check_x + 3, check_y + 5, Color::new(100, 255, 100));
                    surface.set_pixel(check_x + 4, check_y + 4, Color::new(100, 255, 100));
                    surface.set_pixel(check_x + 5, check_y + 3, Color::new(100, 255, 100));
                    surface.set_pixel(check_x + 6, check_y + 2, Color::new(100, 255, 100));
                }
            }
        }
    }
}

// Helper functions

fn format_percentage(value: u8) -> alloc::string::String {
    use alloc::string::ToString;
    let mut s = value.to_string();
    s.push('%');
    s
}

fn draw_char(surface: &mut Surface, x: usize, y: usize, c: char, color: Color) {
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

/// Global volume state
use spin::Mutex;

static VOLUME_STATE: Mutex<VolumeState> = Mutex::new(VolumeState {
    volume: 50,
    muted: false,
});

struct VolumeState {
    volume: u8,
    muted: bool,
}

/// Get current system volume
pub fn get_volume() -> u8 {
    VOLUME_STATE.lock().volume
}

/// Set system volume
pub fn set_volume(volume: u8) {
    let mut state = VOLUME_STATE.lock();
    state.volume = volume.min(100);
    // Here would notify audio driver
}

/// Check if system is muted
pub fn is_muted() -> bool {
    VOLUME_STATE.lock().muted
}

/// Set mute state
pub fn set_muted(muted: bool) {
    let mut state = VOLUME_STATE.lock();
    state.muted = muted;
    // Here would notify audio driver
}

/// Toggle mute
pub fn toggle_mute() {
    let mut state = VOLUME_STATE.lock();
    state.muted = !state.muted;
    // Here would notify audio driver
}
