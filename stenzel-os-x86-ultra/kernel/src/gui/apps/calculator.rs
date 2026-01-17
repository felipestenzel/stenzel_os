//! Calculator Application
//!
//! A basic calculator with standard arithmetic operations.

use alloc::string::String;
use alloc::vec::Vec;
use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use crate::gui::widgets::{Widget, WidgetId, Bounds, WidgetEvent, MouseButton, theme};

/// Calculator operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Operation {
    None,
    Add,
    Subtract,
    Multiply,
    Divide,
    Percent,
}

/// Calculator button type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ButtonType {
    Digit(u8),
    Operation(Operation),
    Decimal,
    Equals,
    Clear,
    ClearEntry,
    Backspace,
    Negate,
    MemoryClear,
    MemoryRecall,
    MemoryAdd,
    MemorySubtract,
}

/// A calculator button
struct CalcButton {
    bounds: Bounds,
    button_type: ButtonType,
    label: &'static str,
    hovered: bool,
    pressed: bool,
}

impl CalcButton {
    fn new(x: isize, y: isize, w: usize, h: usize, button_type: ButtonType, label: &'static str) -> Self {
        Self {
            bounds: Bounds::new(x, y, w, h),
            button_type,
            label,
            hovered: false,
            pressed: false,
        }
    }
}

/// Calculator widget
pub struct Calculator {
    id: WidgetId,
    bounds: Bounds,
    display: String,
    accumulator: f64,
    current_input: f64,
    pending_op: Operation,
    has_decimal: bool,
    decimal_places: u32,
    new_number: bool,
    memory: f64,
    error: bool,
    buttons: Vec<CalcButton>,
    enabled: bool,
    visible: bool,
}

impl Calculator {
    pub fn new(x: isize, y: isize) -> Self {
        let width = 240;
        let height = 320;
        let button_w = 54;
        let button_h = 42;
        let gap = 4;
        let start_x = x + 8;
        let start_y = y + 60;

        let mut buttons = Vec::new();

        // Row 1: MC MR M+ M-
        let row_y = start_y;
        buttons.push(CalcButton::new(start_x, row_y, button_w, button_h, ButtonType::MemoryClear, "MC"));
        buttons.push(CalcButton::new(start_x + (button_w + gap) as isize, row_y, button_w, button_h, ButtonType::MemoryRecall, "MR"));
        buttons.push(CalcButton::new(start_x + 2 * (button_w + gap) as isize, row_y, button_w, button_h, ButtonType::MemoryAdd, "M+"));
        buttons.push(CalcButton::new(start_x + 3 * (button_w + gap) as isize, row_y, button_w, button_h, ButtonType::MemorySubtract, "M-"));

        // Row 2: C CE BS /
        let row_y = start_y + (button_h + gap) as isize;
        buttons.push(CalcButton::new(start_x, row_y, button_w, button_h, ButtonType::Clear, "C"));
        buttons.push(CalcButton::new(start_x + (button_w + gap) as isize, row_y, button_w, button_h, ButtonType::ClearEntry, "CE"));
        buttons.push(CalcButton::new(start_x + 2 * (button_w + gap) as isize, row_y, button_w, button_h, ButtonType::Backspace, "<"));
        buttons.push(CalcButton::new(start_x + 3 * (button_w + gap) as isize, row_y, button_w, button_h, ButtonType::Operation(Operation::Divide), "/"));

        // Row 3: 7 8 9 *
        let row_y = start_y + 2 * (button_h + gap) as isize;
        buttons.push(CalcButton::new(start_x, row_y, button_w, button_h, ButtonType::Digit(7), "7"));
        buttons.push(CalcButton::new(start_x + (button_w + gap) as isize, row_y, button_w, button_h, ButtonType::Digit(8), "8"));
        buttons.push(CalcButton::new(start_x + 2 * (button_w + gap) as isize, row_y, button_w, button_h, ButtonType::Digit(9), "9"));
        buttons.push(CalcButton::new(start_x + 3 * (button_w + gap) as isize, row_y, button_w, button_h, ButtonType::Operation(Operation::Multiply), "*"));

        // Row 4: 4 5 6 -
        let row_y = start_y + 3 * (button_h + gap) as isize;
        buttons.push(CalcButton::new(start_x, row_y, button_w, button_h, ButtonType::Digit(4), "4"));
        buttons.push(CalcButton::new(start_x + (button_w + gap) as isize, row_y, button_w, button_h, ButtonType::Digit(5), "5"));
        buttons.push(CalcButton::new(start_x + 2 * (button_w + gap) as isize, row_y, button_w, button_h, ButtonType::Digit(6), "6"));
        buttons.push(CalcButton::new(start_x + 3 * (button_w + gap) as isize, row_y, button_w, button_h, ButtonType::Operation(Operation::Subtract), "-"));

        // Row 5: 1 2 3 +
        let row_y = start_y + 4 * (button_h + gap) as isize;
        buttons.push(CalcButton::new(start_x, row_y, button_w, button_h, ButtonType::Digit(1), "1"));
        buttons.push(CalcButton::new(start_x + (button_w + gap) as isize, row_y, button_w, button_h, ButtonType::Digit(2), "2"));
        buttons.push(CalcButton::new(start_x + 2 * (button_w + gap) as isize, row_y, button_w, button_h, ButtonType::Digit(3), "3"));
        buttons.push(CalcButton::new(start_x + 3 * (button_w + gap) as isize, row_y, button_w, button_h, ButtonType::Operation(Operation::Add), "+"));

        // Row 6: +/- 0 . =
        let row_y = start_y + 5 * (button_h + gap) as isize;
        buttons.push(CalcButton::new(start_x, row_y, button_w, button_h, ButtonType::Negate, "+/-"));
        buttons.push(CalcButton::new(start_x + (button_w + gap) as isize, row_y, button_w, button_h, ButtonType::Digit(0), "0"));
        buttons.push(CalcButton::new(start_x + 2 * (button_w + gap) as isize, row_y, button_w, button_h, ButtonType::Decimal, "."));
        buttons.push(CalcButton::new(start_x + 3 * (button_w + gap) as isize, row_y, button_w, button_h, ButtonType::Equals, "="));

        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(x, y, width, height),
            display: String::from("0"),
            accumulator: 0.0,
            current_input: 0.0,
            pending_op: Operation::None,
            has_decimal: false,
            decimal_places: 0,
            new_number: true,
            memory: 0.0,
            error: false,
            buttons,
            enabled: true,
            visible: true,
        }
    }

    /// Clear all state
    fn clear(&mut self) {
        self.accumulator = 0.0;
        self.current_input = 0.0;
        self.pending_op = Operation::None;
        self.has_decimal = false;
        self.decimal_places = 0;
        self.new_number = true;
        self.error = false;
        self.display = String::from("0");
    }

    /// Clear entry (current number only)
    fn clear_entry(&mut self) {
        self.current_input = 0.0;
        self.has_decimal = false;
        self.decimal_places = 0;
        self.new_number = true;
        self.error = false;
        self.display = String::from("0");
    }

    /// Backspace
    fn backspace(&mut self) {
        if self.error || self.new_number {
            return;
        }
        if self.display.len() > 1 {
            let removed = self.display.pop();
            if removed == Some('.') {
                self.has_decimal = false;
            } else if self.has_decimal && self.decimal_places > 0 {
                self.decimal_places -= 1;
            }
            self.current_input = self.display.parse().unwrap_or(0.0);
        } else {
            self.display = String::from("0");
            self.current_input = 0.0;
        }
    }

    /// Input a digit
    fn input_digit(&mut self, digit: u8) {
        if self.error {
            self.clear();
        }

        if self.new_number {
            self.display.clear();
            self.has_decimal = false;
            self.decimal_places = 0;
            self.new_number = false;
        }

        // Limit display length
        if self.display.len() >= 15 {
            return;
        }

        // Don't add leading zeros
        if self.display == "0" && digit == 0 && !self.has_decimal {
            return;
        }

        if self.display == "0" && digit != 0 {
            self.display.clear();
        }

        self.display.push((b'0' + digit) as char);

        if self.has_decimal {
            self.decimal_places += 1;
        }

        self.current_input = self.display.parse().unwrap_or(0.0);
    }

    /// Input decimal point
    fn input_decimal(&mut self) {
        if self.error {
            self.clear();
        }

        if self.new_number {
            self.display = String::from("0");
            self.new_number = false;
        }

        if !self.has_decimal {
            self.display.push('.');
            self.has_decimal = true;
        }
    }

    /// Negate current number
    fn negate(&mut self) {
        if self.error {
            return;
        }
        self.current_input = -self.current_input;
        self.update_display();
    }

    /// Set pending operation
    fn set_operation(&mut self, op: Operation) {
        if self.error {
            return;
        }

        // If there's a pending operation, execute it first
        if self.pending_op != Operation::None && !self.new_number {
            self.execute();
        }

        self.accumulator = self.current_input;
        self.pending_op = op;
        self.new_number = true;
    }

    /// Execute pending operation
    fn execute(&mut self) {
        if self.error {
            return;
        }

        let result = match self.pending_op {
            Operation::None => self.current_input,
            Operation::Add => self.accumulator + self.current_input,
            Operation::Subtract => self.accumulator - self.current_input,
            Operation::Multiply => self.accumulator * self.current_input,
            Operation::Divide => {
                if self.current_input == 0.0 {
                    self.error = true;
                    self.display = String::from("Error");
                    return;
                }
                self.accumulator / self.current_input
            }
            Operation::Percent => self.accumulator * (self.current_input / 100.0),
        };

        // Check for overflow/infinity
        if result.is_infinite() || result.is_nan() {
            self.error = true;
            self.display = String::from("Error");
            return;
        }

        self.current_input = result;
        self.accumulator = result;
        self.pending_op = Operation::None;
        self.new_number = true;
        self.update_display();
    }

    /// Update display string from current_input
    fn update_display(&mut self) {
        // Format number nicely
        let abs_val = if self.current_input < 0.0 { -self.current_input } else { self.current_input };

        if abs_val == 0.0 {
            self.display = String::from("0");
        } else if abs_val >= 1e10 || abs_val < 1e-10 {
            // Scientific notation for very large/small numbers
            self.display = format_scientific(self.current_input);
        } else {
            // Regular format
            self.display = format_number(self.current_input);
        }
    }

    /// Memory operations
    fn memory_clear(&mut self) {
        self.memory = 0.0;
    }

    fn memory_recall(&mut self) {
        self.current_input = self.memory;
        self.update_display();
        self.new_number = true;
    }

    fn memory_add(&mut self) {
        self.memory += self.current_input;
    }

    fn memory_subtract(&mut self) {
        self.memory -= self.current_input;
    }

    /// Handle button press
    fn handle_button(&mut self, button_type: ButtonType) {
        match button_type {
            ButtonType::Digit(d) => self.input_digit(d),
            ButtonType::Decimal => self.input_decimal(),
            ButtonType::Operation(op) => self.set_operation(op),
            ButtonType::Equals => self.execute(),
            ButtonType::Clear => self.clear(),
            ButtonType::ClearEntry => self.clear_entry(),
            ButtonType::Backspace => self.backspace(),
            ButtonType::Negate => self.negate(),
            ButtonType::MemoryClear => self.memory_clear(),
            ButtonType::MemoryRecall => self.memory_recall(),
            ButtonType::MemoryAdd => self.memory_add(),
            ButtonType::MemorySubtract => self.memory_subtract(),
        }
    }

    /// Get button at position
    fn button_at(&self, x: isize, y: isize) -> Option<usize> {
        for (i, button) in self.buttons.iter().enumerate() {
            if button.bounds.contains(x, y) {
                return Some(i);
            }
        }
        None
    }
}

// Number formatting helpers
fn format_number(n: f64) -> String {
    let abs_n = if n < 0.0 { -n } else { n };
    let int_part = abs_n as i64;
    let frac_part = abs_n - (int_part as f64);

    if frac_part < 1e-10 {
        // Integer
        if n < 0.0 {
            alloc::format!("-{}", int_part)
        } else {
            alloc::format!("{}", int_part)
        }
    } else {
        // Has decimal
        let s = alloc::format!("{:.10}", n);
        // Trim trailing zeros
        let trimmed = s.trim_end_matches('0').trim_end_matches('.');
        String::from(trimmed)
    }
}

fn format_scientific(n: f64) -> String {
    if n == 0.0 {
        return String::from("0");
    }

    let abs_n = if n < 0.0 { -n } else { n };
    let mut exp = 0i32;
    let mut mantissa = abs_n;

    if mantissa >= 10.0 {
        while mantissa >= 10.0 {
            mantissa /= 10.0;
            exp += 1;
        }
    } else if mantissa < 1.0 {
        while mantissa < 1.0 {
            mantissa *= 10.0;
            exp -= 1;
        }
    }

    if n < 0.0 {
        alloc::format!("-{:.4}e{}", mantissa, exp)
    } else {
        alloc::format!("{:.4}e{}", mantissa, exp)
    }
}

// Helper drawing functions
fn draw_string(surface: &mut Surface, x: isize, y: isize, text: &str, color: Color) {
    use crate::drivers::font::DEFAULT_FONT;

    if x < 0 || y < 0 {
        return;
    }

    let mut cx = x as usize;
    let cy = y as usize;

    for c in text.chars() {
        if let Some(glyph) = DEFAULT_FONT.get_glyph(c) {
            for row in 0..DEFAULT_FONT.height {
                let byte = glyph[row];
                for col in 0..DEFAULT_FONT.width {
                    if (byte >> (DEFAULT_FONT.width - 1 - col)) & 1 != 0 {
                        surface.set_pixel(cx + col, cy + row, color);
                    }
                }
            }
        }
        cx += DEFAULT_FONT.width;
    }
}

fn fill_rect_safe(surface: &mut Surface, x: isize, y: isize, width: usize, height: usize, color: Color) {
    if x < 0 || y < 0 || width == 0 || height == 0 {
        return;
    }
    surface.fill_rect(x as usize, y as usize, width, height, color);
}

fn draw_rect_safe(surface: &mut Surface, x: isize, y: isize, width: usize, height: usize, color: Color) {
    if x < 0 || y < 0 || width == 0 || height == 0 {
        return;
    }
    surface.draw_rect(x as usize, y as usize, width, height, color);
}

impl Widget for Calculator {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn bounds(&self) -> Bounds {
        self.bounds
    }

    fn set_position(&mut self, x: isize, y: isize) {
        let dx = x - self.bounds.x;
        let dy = y - self.bounds.y;
        self.bounds.x = x;
        self.bounds.y = y;
        // Move all buttons
        for button in &mut self.buttons {
            button.bounds.x += dx;
            button.bounds.y += dy;
        }
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
            WidgetEvent::MouseMove { x, y } => {
                // Update hover state for all buttons
                for button in &mut self.buttons {
                    button.hovered = button.bounds.contains(*x, *y);
                }
                return true;
            }
            WidgetEvent::MouseLeave => {
                for button in &mut self.buttons {
                    button.hovered = false;
                    button.pressed = false;
                }
                return true;
            }
            WidgetEvent::MouseDown { button: MouseButton::Left, x, y } => {
                if let Some(idx) = self.button_at(*x, *y) {
                    self.buttons[idx].pressed = true;
                    return true;
                }
            }
            WidgetEvent::MouseUp { button: MouseButton::Left, x, y } => {
                for button in &mut self.buttons {
                    button.pressed = false;
                }
                if let Some(idx) = self.button_at(*x, *y) {
                    let button_type = self.buttons[idx].button_type;
                    self.handle_button(button_type);
                    return true;
                }
            }
            WidgetEvent::KeyDown { key, .. } => {
                // Keyboard shortcuts
                match *key {
                    0x02..=0x0A => { // 1-9
                        self.input_digit((*key - 1) as u8);
                        return true;
                    }
                    0x0B => { // 0
                        self.input_digit(0);
                        return true;
                    }
                    0x34 => { // . (period)
                        self.input_decimal();
                        return true;
                    }
                    0x0D => { // + (numpad)
                        self.set_operation(Operation::Add);
                        return true;
                    }
                    0x4A => { // - (numpad)
                        self.set_operation(Operation::Subtract);
                        return true;
                    }
                    0x37 => { // * (numpad)
                        self.set_operation(Operation::Multiply);
                        return true;
                    }
                    0x35 | 0xB5 => { // /
                        self.set_operation(Operation::Divide);
                        return true;
                    }
                    0x1C | 0x9C => { // Enter
                        self.execute();
                        return true;
                    }
                    0x0E => { // Backspace
                        self.backspace();
                        return true;
                    }
                    0x01 => { // Escape - clear
                        self.clear();
                        return true;
                    }
                    _ => {}
                }
            }
            WidgetEvent::Character { c } => {
                match *c {
                    '0'..='9' => {
                        self.input_digit((*c as u8) - b'0');
                        return true;
                    }
                    '.' => {
                        self.input_decimal();
                        return true;
                    }
                    '+' => {
                        self.set_operation(Operation::Add);
                        return true;
                    }
                    '-' => {
                        self.set_operation(Operation::Subtract);
                        return true;
                    }
                    '*' => {
                        self.set_operation(Operation::Multiply);
                        return true;
                    }
                    '/' => {
                        self.set_operation(Operation::Divide);
                        return true;
                    }
                    '=' => {
                        self.execute();
                        return true;
                    }
                    '%' => {
                        self.set_operation(Operation::Percent);
                        return true;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        false
    }

    fn render(&self, surface: &mut Surface) {
        if !self.visible {
            return;
        }

        let theme = theme();
        let bg = Color::new(35, 35, 40);
        let display_bg = Color::new(25, 25, 30);

        // Background
        fill_rect_safe(surface, self.bounds.x, self.bounds.y,
                       self.bounds.width, self.bounds.height, bg);

        // Display area
        let display_x = self.bounds.x + 8;
        let display_y = self.bounds.y + 8;
        let display_w = self.bounds.width - 16;
        let display_h = 44;

        fill_rect_safe(surface, display_x, display_y, display_w, display_h, display_bg);
        draw_rect_safe(surface, display_x, display_y, display_w, display_h, theme.border);

        // Display text (right-aligned)
        let text_color = if self.error { Color::new(255, 100, 100) } else { theme.fg };
        let text_width = self.display.len() * 8;
        let text_x = display_x + (display_w as isize) - (text_width as isize) - 8;
        let text_y = display_y + (display_h as isize) / 2 - 8;
        draw_string(surface, text_x, text_y, &self.display, text_color);

        // Memory indicator
        if self.memory != 0.0 {
            draw_string(surface, display_x + 4, display_y + 4, "M", Color::new(100, 150, 255));
        }

        // Draw buttons
        for button in &self.buttons {
            let btn_bg = if button.pressed {
                theme.bg_pressed
            } else if button.hovered {
                theme.bg_hover
            } else {
                // Different colors for different button types
                match button.button_type {
                    ButtonType::Digit(_) => Color::new(60, 60, 68),
                    ButtonType::Operation(_) | ButtonType::Equals => theme.accent,
                    ButtonType::Clear | ButtonType::ClearEntry => Color::new(180, 80, 80),
                    _ => Color::new(50, 55, 60),
                }
            };

            fill_rect_safe(surface, button.bounds.x, button.bounds.y,
                          button.bounds.width, button.bounds.height, btn_bg);

            // Button border
            let border_color = if button.hovered || button.pressed {
                theme.border_focused
            } else {
                theme.border
            };
            draw_rect_safe(surface, button.bounds.x, button.bounds.y,
                          button.bounds.width, button.bounds.height, border_color);

            // Button label (centered)
            let label_width = button.label.len() * 8;
            let label_x = button.bounds.x + (button.bounds.width as isize - label_width as isize) / 2;
            let label_y = button.bounds.y + (button.bounds.height as isize) / 2 - 8;

            let text_color = match button.button_type {
                ButtonType::Operation(_) | ButtonType::Equals => Color::WHITE,
                _ => theme.fg,
            };

            draw_string(surface, label_x, label_y, button.label, text_color);
        }
    }
}
