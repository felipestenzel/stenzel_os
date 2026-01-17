//! Dialog widgets
//!
//! Modal dialogs, message boxes, and input dialogs.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use super::{Widget, WidgetId, WidgetState, WidgetEvent, Bounds, MouseButton, theme};
use super::button::Button;
use super::textbox::TextBox;
use super::label::Label;

/// Dialog result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogResult {
    None,
    Ok,
    Cancel,
    Yes,
    No,
    Retry,
    Abort,
    Ignore,
}

/// Dialog buttons preset
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogButtons {
    Ok,
    OkCancel,
    YesNo,
    YesNoCancel,
    RetryCancel,
    AbortRetryIgnore,
}

/// Message box icon type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageIcon {
    None,
    Info,
    Warning,
    Error,
    Question,
}

/// Dialog callback
pub type DialogCallback = fn(WidgetId, DialogResult);

/// Base dialog structure
pub struct Dialog {
    id: WidgetId,
    bounds: Bounds,
    title: String,
    visible: bool,
    dragging: bool,
    drag_offset: (isize, isize),
    result: DialogResult,
    on_close: Option<DialogCallback>,
}

impl Dialog {
    const TITLE_HEIGHT: usize = 28;
    const BORDER_WIDTH: usize = 2;

    /// Create a new dialog
    pub fn new(title: &str, width: usize, height: usize) -> Self {
        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(0, 0, width, height),
            title: String::from(title),
            visible: false,
            dragging: false,
            drag_offset: (0, 0),
            result: DialogResult::None,
            on_close: None,
        }
    }

    /// Get dialog ID
    pub fn id(&self) -> WidgetId {
        self.id
    }

    /// Center dialog on screen
    pub fn center(&mut self, screen_width: usize, screen_height: usize) {
        self.bounds.x = ((screen_width - self.bounds.width) / 2) as isize;
        self.bounds.y = ((screen_height - self.bounds.height) / 2) as isize;
    }

    /// Set position
    pub fn set_position(&mut self, x: isize, y: isize) {
        self.bounds.x = x;
        self.bounds.y = y;
    }

    /// Get bounds
    pub fn bounds(&self) -> Bounds {
        self.bounds
    }

    /// Get content bounds (excluding title bar)
    pub fn content_bounds(&self) -> Bounds {
        Bounds::new(
            self.bounds.x + Self::BORDER_WIDTH as isize,
            self.bounds.y + Self::TITLE_HEIGHT as isize,
            self.bounds.width - Self::BORDER_WIDTH * 2,
            self.bounds.height - Self::TITLE_HEIGHT - Self::BORDER_WIDTH,
        )
    }

    /// Show dialog
    pub fn show(&mut self) {
        self.visible = true;
        self.result = DialogResult::None;
    }

    /// Hide dialog
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Is visible?
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Get result
    pub fn result(&self) -> DialogResult {
        self.result
    }

    /// Close with result
    pub fn close(&mut self, result: DialogResult) {
        self.result = result;
        self.visible = false;
        if let Some(callback) = self.on_close {
            callback(self.id, result);
        }
    }

    /// Set close callback
    pub fn set_on_close(&mut self, callback: DialogCallback) {
        self.on_close = Some(callback);
    }

    /// Handle mouse event
    pub fn handle_event(&mut self, event: &WidgetEvent) -> bool {
        if !self.visible {
            return false;
        }

        match event {
            WidgetEvent::MouseDown { button: MouseButton::Left, x, y } => {
                // Check title bar for dragging
                let title_bounds = Bounds::new(
                    self.bounds.x,
                    self.bounds.y,
                    self.bounds.width,
                    Self::TITLE_HEIGHT,
                );

                if title_bounds.contains(*x, *y) {
                    // Check close button
                    let close_x = self.bounds.x + self.bounds.width as isize - 24;
                    if *x >= close_x && *x < close_x + 20 {
                        self.close(DialogResult::Cancel);
                        return true;
                    }

                    // Start dragging
                    self.dragging = true;
                    self.drag_offset = (*x - self.bounds.x, *y - self.bounds.y);
                    return true;
                }

                self.bounds.contains(*x, *y)
            }
            WidgetEvent::MouseUp { button: MouseButton::Left, .. } => {
                self.dragging = false;
                true
            }
            WidgetEvent::MouseMove { x, y } => {
                if self.dragging {
                    self.bounds.x = *x - self.drag_offset.0;
                    self.bounds.y = *y - self.drag_offset.1;
                    return true;
                }
                self.bounds.contains(*x, *y)
            }
            WidgetEvent::KeyDown { key: 0x01, .. } => { // Escape
                self.close(DialogResult::Cancel);
                true
            }
            _ => false,
        }
    }

    /// Render dialog frame
    pub fn render(&self, surface: &mut Surface) {
        if !self.visible {
            return;
        }

        let theme = theme();
        let x = self.bounds.x.max(0) as usize;
        let y = self.bounds.y.max(0) as usize;
        let w = self.bounds.width;
        let h = self.bounds.height;

        let title_bg = theme.accent;
        let bg_color = Color::new(45, 45, 53);
        let border_color = Color::new(70, 70, 78);

        // Draw shadow
        for py in 4..h + 4 {
            for px in 4..w + 4 {
                surface.set_pixel(x + px, y + py, Color::new(0, 0, 0));
            }
        }

        // Draw background
        for py in 0..h {
            for px in 0..w {
                surface.set_pixel(x + px, y + py, bg_color);
            }
        }

        // Draw border
        for px in 0..w {
            surface.set_pixel(x + px, y, border_color);
            surface.set_pixel(x + px, y + h - 1, border_color);
        }
        for py in 0..h {
            surface.set_pixel(x, y + py, border_color);
            surface.set_pixel(x + w - 1, y + py, border_color);
        }

        // Draw title bar
        for py in 0..Self::TITLE_HEIGHT {
            for px in 1..w - 1 {
                surface.set_pixel(x + px, y + py, title_bg);
            }
        }

        // Draw title bar bottom border
        for px in 1..w - 1 {
            surface.set_pixel(x + px, y + Self::TITLE_HEIGHT - 1, border_color);
        }

        // Draw title text
        let title_x = x + 10;
        let title_y = y + (Self::TITLE_HEIGHT - 16) / 2;
        for (i, c) in self.title.chars().enumerate() {
            draw_char_simple(surface, title_x + i * 8, title_y, c, Color::WHITE);
        }

        // Draw close button (X)
        let close_x = x + w - 24;
        let close_y = y + (Self::TITLE_HEIGHT - 12) / 2;

        // X mark
        for i in 0..10 {
            surface.set_pixel(close_x + i + 2, close_y + i, Color::WHITE);
            surface.set_pixel(close_x + i + 2, close_y + 9 - i, Color::WHITE);
        }
    }
}

/// Message box dialog
pub struct MessageBox {
    dialog: Dialog,
    message: String,
    icon: MessageIcon,
    buttons_type: DialogButtons,
    buttons: Vec<Button>,
    focused_button: usize,
}

impl MessageBox {
    /// Create a new message box
    pub fn new(title: &str, message: &str, icon: MessageIcon, buttons: DialogButtons) -> Self {
        let mut msg_box = Self {
            dialog: Dialog::new(title, 350, 150),
            message: String::from(message),
            icon,
            buttons_type: buttons,
            buttons: Vec::new(),
            focused_button: 0,
        };
        msg_box.create_buttons();
        msg_box
    }

    /// Create info message box
    pub fn info(title: &str, message: &str) -> Self {
        Self::new(title, message, MessageIcon::Info, DialogButtons::Ok)
    }

    /// Create warning message box
    pub fn warning(title: &str, message: &str) -> Self {
        Self::new(title, message, MessageIcon::Warning, DialogButtons::Ok)
    }

    /// Create error message box
    pub fn error(title: &str, message: &str) -> Self {
        Self::new(title, message, MessageIcon::Error, DialogButtons::Ok)
    }

    /// Create question message box
    pub fn question(title: &str, message: &str) -> Self {
        Self::new(title, message, MessageIcon::Question, DialogButtons::YesNo)
    }

    /// Create confirm message box
    pub fn confirm(title: &str, message: &str) -> Self {
        Self::new(title, message, MessageIcon::Question, DialogButtons::OkCancel)
    }

    fn create_buttons(&mut self) {
        let content = self.dialog.content_bounds();
        let button_y = content.y + content.height as isize - 35;

        let button_labels: Vec<(&str, DialogResult)> = match self.buttons_type {
            DialogButtons::Ok => vec![("OK", DialogResult::Ok)],
            DialogButtons::OkCancel => vec![("OK", DialogResult::Ok), ("Cancel", DialogResult::Cancel)],
            DialogButtons::YesNo => vec![("Yes", DialogResult::Yes), ("No", DialogResult::No)],
            DialogButtons::YesNoCancel => vec![
                ("Yes", DialogResult::Yes),
                ("No", DialogResult::No),
                ("Cancel", DialogResult::Cancel),
            ],
            DialogButtons::RetryCancel => vec![("Retry", DialogResult::Retry), ("Cancel", DialogResult::Cancel)],
            DialogButtons::AbortRetryIgnore => vec![
                ("Abort", DialogResult::Abort),
                ("Retry", DialogResult::Retry),
                ("Ignore", DialogResult::Ignore),
            ],
        };

        let button_width = 80;
        let button_spacing = 10;
        let total_width = button_labels.len() * button_width + (button_labels.len() - 1) * button_spacing;
        let start_x = content.x + (content.width - total_width) as isize / 2;

        self.buttons.clear();
        for (i, (label, _)) in button_labels.iter().enumerate() {
            let button_x = start_x + (i * (button_width + button_spacing)) as isize;
            self.buttons.push(Button::new(button_x, button_y, button_width, 28, label));
        }
    }

    /// Center on screen
    pub fn center(&mut self, screen_width: usize, screen_height: usize) {
        self.dialog.center(screen_width, screen_height);
        self.create_buttons();
    }

    /// Show
    pub fn show(&mut self) {
        self.dialog.show();
    }

    /// Is visible?
    pub fn is_visible(&self) -> bool {
        self.dialog.is_visible()
    }

    /// Get result
    pub fn result(&self) -> DialogResult {
        self.dialog.result()
    }

    /// Set close callback
    pub fn set_on_close(&mut self, callback: DialogCallback) {
        self.dialog.set_on_close(callback);
    }

    fn get_result_for_button(&self, index: usize) -> DialogResult {
        match self.buttons_type {
            DialogButtons::Ok => DialogResult::Ok,
            DialogButtons::OkCancel => {
                if index == 0 { DialogResult::Ok } else { DialogResult::Cancel }
            }
            DialogButtons::YesNo => {
                if index == 0 { DialogResult::Yes } else { DialogResult::No }
            }
            DialogButtons::YesNoCancel => {
                match index {
                    0 => DialogResult::Yes,
                    1 => DialogResult::No,
                    _ => DialogResult::Cancel,
                }
            }
            DialogButtons::RetryCancel => {
                if index == 0 { DialogResult::Retry } else { DialogResult::Cancel }
            }
            DialogButtons::AbortRetryIgnore => {
                match index {
                    0 => DialogResult::Abort,
                    1 => DialogResult::Retry,
                    _ => DialogResult::Ignore,
                }
            }
        }
    }

    /// Handle event
    pub fn handle_event(&mut self, event: &WidgetEvent) -> bool {
        if !self.dialog.is_visible() {
            return false;
        }

        // Check button clicks
        if let WidgetEvent::MouseDown { button: MouseButton::Left, x, y } = event {
            for (i, button) in self.buttons.iter().enumerate() {
                if button.bounds().contains(*x, *y) {
                    let result = self.get_result_for_button(i);
                    self.dialog.close(result);
                    return true;
                }
            }
        }

        // Handle keyboard
        if let WidgetEvent::KeyDown { key, .. } = event {
            match *key {
                0x1C => { // Enter
                    let result = self.get_result_for_button(self.focused_button);
                    self.dialog.close(result);
                    return true;
                }
                0x0F => { // Tab
                    self.focused_button = (self.focused_button + 1) % self.buttons.len();
                    return true;
                }
                _ => {}
            }
        }

        self.dialog.handle_event(event)
    }

    /// Render
    pub fn render(&self, surface: &mut Surface) {
        if !self.dialog.is_visible() {
            return;
        }

        self.dialog.render(surface);

        let theme = theme();
        let content = self.dialog.content_bounds();
        let x = content.x.max(0) as usize;
        let y = content.y.max(0) as usize;

        // Draw icon
        let icon_x = x + 15;
        let icon_y = y + 15;
        let icon_size = 32;

        match self.icon {
            MessageIcon::Info => {
                // Blue circle with 'i'
                let center = icon_size / 2;
                for py in 0..icon_size {
                    for px in 0..icon_size {
                        let dx = px as isize - center as isize;
                        let dy = py as isize - center as isize;
                        if dx * dx + dy * dy <= (center as isize * center as isize) {
                            surface.set_pixel(icon_x + px, icon_y + py, Color::new(60, 130, 200));
                        }
                    }
                }
                // Draw 'i'
                for py in 8..12 {
                    surface.set_pixel(icon_x + center, icon_y + py, Color::WHITE);
                }
                for py in 14..24 {
                    surface.set_pixel(icon_x + center, icon_y + py, Color::WHITE);
                }
            }
            MessageIcon::Warning => {
                // Yellow triangle with '!'
                let center = icon_size / 2;
                for py in 0..icon_size {
                    let half_width = py / 2;
                    let start_x = center.saturating_sub(half_width);
                    let end_x = center + half_width;
                    for px in start_x..=end_x {
                        surface.set_pixel(icon_x + px, icon_y + py, Color::new(255, 200, 0));
                    }
                }
                // Draw '!'
                for py in 8..20 {
                    surface.set_pixel(icon_x + center, icon_y + py, Color::new(0, 0, 0));
                }
                for py in 23..27 {
                    surface.set_pixel(icon_x + center, icon_y + py, Color::new(0, 0, 0));
                }
            }
            MessageIcon::Error => {
                // Red circle with 'X'
                let center = icon_size / 2;
                for py in 0..icon_size {
                    for px in 0..icon_size {
                        let dx = px as isize - center as isize;
                        let dy = py as isize - center as isize;
                        if dx * dx + dy * dy <= (center as isize * center as isize) {
                            surface.set_pixel(icon_x + px, icon_y + py, Color::new(200, 50, 50));
                        }
                    }
                }
                // Draw 'X'
                for i in 8..24 {
                    surface.set_pixel(icon_x + i, icon_y + i, Color::WHITE);
                    surface.set_pixel(icon_x + i, icon_y + 31 - i, Color::WHITE);
                }
            }
            MessageIcon::Question => {
                // Blue circle with '?'
                let center = icon_size / 2;
                for py in 0..icon_size {
                    for px in 0..icon_size {
                        let dx = px as isize - center as isize;
                        let dy = py as isize - center as isize;
                        if dx * dx + dy * dy <= (center as isize * center as isize) {
                            surface.set_pixel(icon_x + px, icon_y + py, Color::new(60, 130, 200));
                        }
                    }
                }
                // Simple '?' shape
                for px in 12..20 {
                    surface.set_pixel(icon_x + px, icon_y + 8, Color::WHITE);
                }
                surface.set_pixel(icon_x + 20, icon_y + 9, Color::WHITE);
                surface.set_pixel(icon_x + 20, icon_y + 10, Color::WHITE);
                for py in 11..16 {
                    surface.set_pixel(icon_x + center, icon_y + py, Color::WHITE);
                }
                for py in 19..23 {
                    surface.set_pixel(icon_x + center, icon_y + py, Color::WHITE);
                }
            }
            MessageIcon::None => {}
        }

        // Draw message
        let msg_x = if self.icon == MessageIcon::None { x + 15 } else { x + 60 };
        let msg_y = y + 25;

        for (i, c) in self.message.chars().enumerate() {
            let cx = msg_x + (i % 30) * 8;
            let cy = msg_y + (i / 30) * 18;
            draw_char_simple(surface, cx, cy, c, theme.fg);
        }

        // Draw buttons
        for (i, button) in self.buttons.iter().enumerate() {
            // Highlight focused button
            if i == self.focused_button {
                let b = button.bounds();
                let bx = b.x.max(0) as usize;
                let by = b.y.max(0) as usize;
                // Draw focus indicator
                for px in 0..b.width {
                    surface.set_pixel(bx + px, by - 2, theme.accent);
                }
            }
            button.render(surface);
        }
    }
}

/// Input dialog for getting text input
pub struct InputDialog {
    dialog: Dialog,
    prompt: String,
    textbox: TextBox,
    ok_button: Button,
    cancel_button: Button,
    input_value: String,
}

impl InputDialog {
    /// Create a new input dialog
    pub fn new(title: &str, prompt: &str, default_value: &str) -> Self {
        let mut dialog = Dialog::new(title, 350, 140);
        let content = dialog.content_bounds();

        let mut textbox = TextBox::new(
            content.x + 15,
            content.y + 40,
            content.width - 30,
        );
        textbox.set_text(default_value);

        let button_y = content.y + content.height as isize - 40;
        let ok_button = Button::new(content.x + content.width as isize / 2 - 90, button_y, 80, 28, "OK");
        let cancel_button = Button::new(content.x + content.width as isize / 2 + 10, button_y, 80, 28, "Cancel");

        Self {
            dialog,
            prompt: String::from(prompt),
            textbox,
            ok_button,
            cancel_button,
            input_value: String::from(default_value),
        }
    }

    /// Center on screen
    pub fn center(&mut self, screen_width: usize, screen_height: usize) {
        self.dialog.center(screen_width, screen_height);
        // Recalculate widget positions
        let content = self.dialog.content_bounds();
        self.textbox.set_position(content.x + 15, content.y + 40);
        let button_y = content.y + content.height as isize - 40;
        self.ok_button.set_position(content.x + content.width as isize / 2 - 90, button_y);
        self.cancel_button.set_position(content.x + content.width as isize / 2 + 10, button_y);
    }

    /// Show
    pub fn show(&mut self) {
        self.dialog.show();
    }

    /// Is visible?
    pub fn is_visible(&self) -> bool {
        self.dialog.is_visible()
    }

    /// Get result
    pub fn result(&self) -> DialogResult {
        self.dialog.result()
    }

    /// Get input value
    pub fn value(&self) -> &str {
        &self.input_value
    }

    /// Set close callback
    pub fn set_on_close(&mut self, callback: DialogCallback) {
        self.dialog.set_on_close(callback);
    }

    /// Handle event
    pub fn handle_event(&mut self, event: &WidgetEvent) -> bool {
        if !self.dialog.is_visible() {
            return false;
        }

        // Handle textbox
        if self.textbox.handle_event(event) {
            self.input_value = String::from(self.textbox.text());
            return true;
        }

        // Handle button clicks
        if let WidgetEvent::MouseDown { button: MouseButton::Left, x, y } = event {
            if self.ok_button.bounds().contains(*x, *y) {
                self.input_value = String::from(self.textbox.text());
                self.dialog.close(DialogResult::Ok);
                return true;
            }
            if self.cancel_button.bounds().contains(*x, *y) {
                self.dialog.close(DialogResult::Cancel);
                return true;
            }
        }

        // Handle keyboard
        if let WidgetEvent::KeyDown { key, .. } = event {
            match *key {
                0x1C => { // Enter
                    self.input_value = String::from(self.textbox.text());
                    self.dialog.close(DialogResult::Ok);
                    return true;
                }
                _ => {}
            }
        }

        self.dialog.handle_event(event)
    }

    /// Render
    pub fn render(&self, surface: &mut Surface) {
        if !self.dialog.is_visible() {
            return;
        }

        self.dialog.render(surface);

        let theme = theme();
        let content = self.dialog.content_bounds();
        let x = content.x.max(0) as usize;
        let y = content.y.max(0) as usize;

        // Draw prompt
        for (i, c) in self.prompt.chars().enumerate() {
            draw_char_simple(surface, x + 15 + i * 8, y + 15, c, theme.fg);
        }

        // Draw textbox
        self.textbox.render(surface);

        // Draw buttons
        self.ok_button.render(surface);
        self.cancel_button.render(surface);
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
