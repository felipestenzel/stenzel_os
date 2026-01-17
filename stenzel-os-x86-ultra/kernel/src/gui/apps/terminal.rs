//! Terminal Emulator
//!
//! A graphical terminal emulator with VT100/ANSI escape code support.

use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use crate::gui::widgets::{Widget, WidgetId, WidgetEvent, Bounds, MouseButton};

/// Terminal cell attributes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellAttrs {
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub blink: bool,
    pub reverse: bool,
    pub hidden: bool,
}

impl Default for CellAttrs {
    fn default() -> Self {
        Self {
            fg: Color::new(204, 204, 204), // Light gray
            bg: Color::new(0, 0, 0),       // Black
            bold: false,
            italic: false,
            underline: false,
            blink: false,
            reverse: false,
            hidden: false,
        }
    }
}

/// A single cell in the terminal
#[derive(Debug, Clone, Copy)]
pub struct Cell {
    pub c: char,
    pub attrs: CellAttrs,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            c: ' ',
            attrs: CellAttrs::default(),
        }
    }
}

/// Terminal color palette (16 standard colors)
pub struct ColorPalette {
    colors: [Color; 16],
}

impl Default for ColorPalette {
    fn default() -> Self {
        Self {
            colors: [
                Color::new(0, 0, 0),       // 0: Black
                Color::new(170, 0, 0),     // 1: Red
                Color::new(0, 170, 0),     // 2: Green
                Color::new(170, 85, 0),    // 3: Yellow/Brown
                Color::new(0, 0, 170),     // 4: Blue
                Color::new(170, 0, 170),   // 5: Magenta
                Color::new(0, 170, 170),   // 6: Cyan
                Color::new(170, 170, 170), // 7: White
                Color::new(85, 85, 85),    // 8: Bright Black (Gray)
                Color::new(255, 85, 85),   // 9: Bright Red
                Color::new(85, 255, 85),   // 10: Bright Green
                Color::new(255, 255, 85),  // 11: Bright Yellow
                Color::new(85, 85, 255),   // 12: Bright Blue
                Color::new(255, 85, 255),  // 13: Bright Magenta
                Color::new(85, 255, 255),  // 14: Bright Cyan
                Color::new(255, 255, 255), // 15: Bright White
            ],
        }
    }
}

impl ColorPalette {
    pub fn get(&self, index: u8) -> Color {
        if index < 16 {
            self.colors[index as usize]
        } else if index < 232 {
            // 216 color cube (6x6x6)
            let index = index - 16;
            let r = (index / 36) % 6;
            let g = (index / 6) % 6;
            let b = index % 6;
            Color::new(
                if r > 0 { r * 40 + 55 } else { 0 },
                if g > 0 { g * 40 + 55 } else { 0 },
                if b > 0 { b * 40 + 55 } else { 0 },
            )
        } else {
            // 24 grayscale
            let gray = (index - 232) * 10 + 8;
            Color::new(gray, gray, gray)
        }
    }
}

/// Parser state for escape sequences
#[derive(Debug, Clone, PartialEq, Eq)]
enum ParserState {
    Normal,
    Escape,
    Csi,
    CsiParam,
    Osc,
}

/// Terminal output callback
pub type TerminalOutputCallback = fn(&[u8]);

/// Terminal emulator widget
pub struct Terminal {
    id: WidgetId,
    bounds: Bounds,

    // Grid
    cols: usize,
    rows: usize,
    cells: Vec<Cell>,

    // Scrollback
    scrollback: VecDeque<Vec<Cell>>,
    scrollback_limit: usize,
    scroll_offset: usize,

    // Cursor
    cursor_x: usize,
    cursor_y: usize,
    cursor_visible: bool,
    cursor_blink: bool,
    saved_cursor: (usize, usize),

    // Attributes
    current_attrs: CellAttrs,
    palette: ColorPalette,

    // Parser
    parser_state: ParserState,
    csi_params: Vec<u32>,
    csi_intermediate: Vec<u8>,
    osc_string: String,

    // Modes
    insert_mode: bool,
    auto_wrap: bool,
    origin_mode: bool,

    // Scroll region
    scroll_top: usize,
    scroll_bottom: usize,

    // Callbacks
    on_output: Option<TerminalOutputCallback>,

    // Input buffer
    input_buffer: VecDeque<u8>,

    // State
    visible: bool,
    focused: bool,

    // Selection
    selection_start: Option<(usize, usize)>,
    selection_end: Option<(usize, usize)>,

    // Title
    title: String,
}

impl Terminal {
    const CHAR_WIDTH: usize = 8;
    const CHAR_HEIGHT: usize = 16;
    const PADDING: usize = 4;

    /// Create a new terminal
    pub fn new(x: isize, y: isize, width: usize, height: usize) -> Self {
        let cols = (width - Self::PADDING * 2) / Self::CHAR_WIDTH;
        let rows = (height - Self::PADDING * 2) / Self::CHAR_HEIGHT;
        let cells = vec![Cell::default(); cols * rows];

        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(x, y, width, height),
            cols,
            rows,
            cells,
            scrollback: VecDeque::new(),
            scrollback_limit: 1000,
            scroll_offset: 0,
            cursor_x: 0,
            cursor_y: 0,
            cursor_visible: true,
            cursor_blink: true,
            saved_cursor: (0, 0),
            current_attrs: CellAttrs::default(),
            palette: ColorPalette::default(),
            parser_state: ParserState::Normal,
            csi_params: Vec::new(),
            csi_intermediate: Vec::new(),
            osc_string: String::new(),
            insert_mode: false,
            auto_wrap: true,
            origin_mode: false,
            scroll_top: 0,
            scroll_bottom: rows.saturating_sub(1),
            on_output: None,
            input_buffer: VecDeque::new(),
            visible: true,
            focused: false,
            selection_start: None,
            selection_end: None,
            title: String::from("Terminal"),
        }
    }

    /// Get terminal title
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Set output callback
    pub fn set_on_output(&mut self, callback: TerminalOutputCallback) {
        self.on_output = Some(callback);
    }

    /// Write bytes to terminal (from PTY/shell)
    pub fn write(&mut self, data: &[u8]) {
        for &byte in data {
            self.process_byte(byte);
        }
    }

    /// Write string to terminal
    pub fn write_str(&mut self, s: &str) {
        self.write(s.as_bytes());
    }

    /// Send input to shell (keyboard)
    pub fn input(&mut self, data: &[u8]) {
        if let Some(callback) = self.on_output {
            callback(data);
        }
        for &b in data {
            self.input_buffer.push_back(b);
        }
    }

    /// Read pending input
    pub fn read_input(&mut self) -> Option<u8> {
        self.input_buffer.pop_front()
    }

    /// Process a single byte
    fn process_byte(&mut self, byte: u8) {
        match self.parser_state {
            ParserState::Normal => self.process_normal(byte),
            ParserState::Escape => self.process_escape(byte),
            ParserState::Csi | ParserState::CsiParam => self.process_csi(byte),
            ParserState::Osc => self.process_osc(byte),
        }
    }

    fn process_normal(&mut self, byte: u8) {
        match byte {
            0x00 => {} // NUL - ignore
            0x07 => {} // BEL - bell (could trigger sound)
            0x08 => self.backspace(),
            0x09 => self.tab(),
            0x0A | 0x0B | 0x0C => self.linefeed(),
            0x0D => self.carriage_return(),
            0x1B => self.parser_state = ParserState::Escape,
            0x7F => self.backspace(),
            _ => self.put_char(byte as char),
        }
    }

    fn process_escape(&mut self, byte: u8) {
        match byte {
            b'[' => {
                self.parser_state = ParserState::Csi;
                self.csi_params.clear();
                self.csi_intermediate.clear();
            }
            b']' => {
                self.parser_state = ParserState::Osc;
                self.osc_string.clear();
            }
            b'D' => { self.linefeed(); self.parser_state = ParserState::Normal; }
            b'E' => { self.carriage_return(); self.linefeed(); self.parser_state = ParserState::Normal; }
            b'M' => { self.reverse_linefeed(); self.parser_state = ParserState::Normal; }
            b'7' => { self.save_cursor(); self.parser_state = ParserState::Normal; }
            b'8' => { self.restore_cursor(); self.parser_state = ParserState::Normal; }
            b'c' => { self.reset(); self.parser_state = ParserState::Normal; }
            _ => self.parser_state = ParserState::Normal,
        }
    }

    fn process_csi(&mut self, byte: u8) {
        match byte {
            b'0'..=b'9' => {
                self.parser_state = ParserState::CsiParam;
                let digit = (byte - b'0') as u32;
                if let Some(last) = self.csi_params.last_mut() {
                    *last = last.saturating_mul(10).saturating_add(digit);
                } else {
                    self.csi_params.push(digit);
                }
            }
            b';' => {
                self.parser_state = ParserState::CsiParam;
                if self.csi_params.is_empty() {
                    self.csi_params.push(0);
                }
                self.csi_params.push(0);
            }
            b' '..=b'/' => {
                self.csi_intermediate.push(byte);
            }
            b'@'..=b'~' => {
                self.execute_csi(byte);
                self.parser_state = ParserState::Normal;
            }
            _ => self.parser_state = ParserState::Normal,
        }
    }

    fn process_osc(&mut self, byte: u8) {
        match byte {
            0x07 | 0x9C => {
                self.execute_osc();
                self.parser_state = ParserState::Normal;
            }
            0x1B => {
                // Could be ST (ESC \)
                self.parser_state = ParserState::Normal;
            }
            _ => {
                if self.osc_string.len() < 256 {
                    self.osc_string.push(byte as char);
                }
            }
        }
    }

    fn execute_csi(&mut self, cmd: u8) {
        // Clone params to avoid borrow checker issues with mutable self calls
        let params: Vec<u32> = self.csi_params.clone();
        let p0 = params.get(0).copied().unwrap_or(0);
        let p1 = params.get(1).copied().unwrap_or(0);

        match cmd {
            b'A' => self.cursor_up(p0.max(1) as usize),
            b'B' => self.cursor_down(p0.max(1) as usize),
            b'C' => self.cursor_forward(p0.max(1) as usize),
            b'D' => self.cursor_backward(p0.max(1) as usize),
            b'E' => { self.cursor_down(p0.max(1) as usize); self.cursor_x = 0; }
            b'F' => { self.cursor_up(p0.max(1) as usize); self.cursor_x = 0; }
            b'G' => self.cursor_x = (p0.max(1) as usize).saturating_sub(1).min(self.cols - 1),
            b'H' | b'f' => self.set_cursor(
                (p1.max(1) as usize).saturating_sub(1),
                (p0.max(1) as usize).saturating_sub(1),
            ),
            b'J' => self.erase_display(p0 as usize),
            b'K' => self.erase_line(p0 as usize),
            b'L' => self.insert_lines(p0.max(1) as usize),
            b'M' => self.delete_lines(p0.max(1) as usize),
            b'P' => self.delete_chars(p0.max(1) as usize),
            b'S' => self.scroll_up(p0.max(1) as usize),
            b'T' => self.scroll_down(p0.max(1) as usize),
            b'X' => self.erase_chars(p0.max(1) as usize),
            b'@' => self.insert_chars(p0.max(1) as usize),
            b'd' => self.cursor_y = (p0.max(1) as usize).saturating_sub(1).min(self.rows - 1),
            b'm' => self.set_graphics_mode(&params),
            b'n' => self.device_status_report(p0),
            b'r' => self.set_scroll_region(p0 as usize, p1 as usize),
            b's' => self.save_cursor(),
            b'u' => self.restore_cursor(),
            b'h' => self.set_mode(&params, true),
            b'l' => self.set_mode(&params, false),
            _ => {}
        }
    }

    fn execute_osc(&mut self) {
        // Parse OSC command
        if let Some(pos) = self.osc_string.find(';') {
            let cmd: u32 = self.osc_string[..pos].parse().unwrap_or(0);
            let arg = &self.osc_string[pos + 1..];

            match cmd {
                0 | 2 => {
                    // Set window title
                    self.title = String::from(arg);
                }
                1 => {
                    // Set icon name (ignore)
                }
                _ => {}
            }
        }
    }

    fn put_char(&mut self, c: char) {
        if self.cursor_x >= self.cols {
            if self.auto_wrap {
                self.carriage_return();
                self.linefeed();
            } else {
                self.cursor_x = self.cols - 1;
            }
        }

        let idx = self.cursor_y * self.cols + self.cursor_x;
        if idx < self.cells.len() {
            if self.insert_mode {
                // Shift cells right
                for i in (self.cursor_x + 1..self.cols).rev() {
                    let src = self.cursor_y * self.cols + i - 1;
                    let dst = self.cursor_y * self.cols + i;
                    if dst < self.cells.len() && src < self.cells.len() {
                        self.cells[dst] = self.cells[src];
                    }
                }
            }

            self.cells[idx] = Cell {
                c,
                attrs: self.current_attrs,
            };
        }

        self.cursor_x += 1;
    }

    fn backspace(&mut self) {
        if self.cursor_x > 0 {
            self.cursor_x -= 1;
        }
    }

    fn tab(&mut self) {
        let next_tab = ((self.cursor_x / 8) + 1) * 8;
        self.cursor_x = next_tab.min(self.cols - 1);
    }

    fn carriage_return(&mut self) {
        self.cursor_x = 0;
    }

    fn linefeed(&mut self) {
        if self.cursor_y == self.scroll_bottom {
            self.scroll_up(1);
        } else if self.cursor_y < self.rows - 1 {
            self.cursor_y += 1;
        }
    }

    fn reverse_linefeed(&mut self) {
        if self.cursor_y == self.scroll_top {
            self.scroll_down(1);
        } else if self.cursor_y > 0 {
            self.cursor_y -= 1;
        }
    }

    fn cursor_up(&mut self, n: usize) {
        self.cursor_y = self.cursor_y.saturating_sub(n);
    }

    fn cursor_down(&mut self, n: usize) {
        self.cursor_y = (self.cursor_y + n).min(self.rows - 1);
    }

    fn cursor_forward(&mut self, n: usize) {
        self.cursor_x = (self.cursor_x + n).min(self.cols - 1);
    }

    fn cursor_backward(&mut self, n: usize) {
        self.cursor_x = self.cursor_x.saturating_sub(n);
    }

    fn set_cursor(&mut self, x: usize, y: usize) {
        self.cursor_x = x.min(self.cols - 1);
        self.cursor_y = y.min(self.rows - 1);
    }

    fn save_cursor(&mut self) {
        self.saved_cursor = (self.cursor_x, self.cursor_y);
    }

    fn restore_cursor(&mut self) {
        self.cursor_x = self.saved_cursor.0.min(self.cols - 1);
        self.cursor_y = self.saved_cursor.1.min(self.rows - 1);
    }

    fn erase_display(&mut self, mode: usize) {
        match mode {
            0 => {
                // Erase from cursor to end
                let start = self.cursor_y * self.cols + self.cursor_x;
                for i in start..self.cells.len() {
                    self.cells[i] = Cell::default();
                }
            }
            1 => {
                // Erase from start to cursor
                let end = self.cursor_y * self.cols + self.cursor_x + 1;
                for i in 0..end.min(self.cells.len()) {
                    self.cells[i] = Cell::default();
                }
            }
            2 | 3 => {
                // Erase entire display
                for cell in &mut self.cells {
                    *cell = Cell::default();
                }
            }
            _ => {}
        }
    }

    fn erase_line(&mut self, mode: usize) {
        let row_start = self.cursor_y * self.cols;
        match mode {
            0 => {
                // Erase from cursor to end of line
                for i in self.cursor_x..self.cols {
                    if row_start + i < self.cells.len() {
                        self.cells[row_start + i] = Cell::default();
                    }
                }
            }
            1 => {
                // Erase from start of line to cursor
                for i in 0..=self.cursor_x {
                    if row_start + i < self.cells.len() {
                        self.cells[row_start + i] = Cell::default();
                    }
                }
            }
            2 => {
                // Erase entire line
                for i in 0..self.cols {
                    if row_start + i < self.cells.len() {
                        self.cells[row_start + i] = Cell::default();
                    }
                }
            }
            _ => {}
        }
    }

    fn insert_lines(&mut self, n: usize) {
        for _ in 0..n {
            // Shift lines down
            for y in (self.cursor_y + 1..=self.scroll_bottom).rev() {
                for x in 0..self.cols {
                    let src = (y - 1) * self.cols + x;
                    let dst = y * self.cols + x;
                    if dst < self.cells.len() && src < self.cells.len() {
                        self.cells[dst] = self.cells[src];
                    }
                }
            }
            // Clear current line
            for x in 0..self.cols {
                let idx = self.cursor_y * self.cols + x;
                if idx < self.cells.len() {
                    self.cells[idx] = Cell::default();
                }
            }
        }
    }

    fn delete_lines(&mut self, n: usize) {
        for _ in 0..n {
            // Shift lines up
            for y in self.cursor_y..self.scroll_bottom {
                for x in 0..self.cols {
                    let src = (y + 1) * self.cols + x;
                    let dst = y * self.cols + x;
                    if dst < self.cells.len() && src < self.cells.len() {
                        self.cells[dst] = self.cells[src];
                    }
                }
            }
            // Clear bottom line
            for x in 0..self.cols {
                let idx = self.scroll_bottom * self.cols + x;
                if idx < self.cells.len() {
                    self.cells[idx] = Cell::default();
                }
            }
        }
    }

    fn insert_chars(&mut self, n: usize) {
        let row_start = self.cursor_y * self.cols;
        for _ in 0..n {
            for x in (self.cursor_x + 1..self.cols).rev() {
                let src = row_start + x - 1;
                let dst = row_start + x;
                if dst < self.cells.len() && src < self.cells.len() {
                    self.cells[dst] = self.cells[src];
                }
            }
            if row_start + self.cursor_x < self.cells.len() {
                self.cells[row_start + self.cursor_x] = Cell::default();
            }
        }
    }

    fn delete_chars(&mut self, n: usize) {
        let row_start = self.cursor_y * self.cols;
        for _ in 0..n {
            for x in self.cursor_x..self.cols - 1 {
                let src = row_start + x + 1;
                let dst = row_start + x;
                if dst < self.cells.len() && src < self.cells.len() {
                    self.cells[dst] = self.cells[src];
                }
            }
            if row_start + self.cols - 1 < self.cells.len() {
                self.cells[row_start + self.cols - 1] = Cell::default();
            }
        }
    }

    fn erase_chars(&mut self, n: usize) {
        let row_start = self.cursor_y * self.cols;
        for i in 0..n {
            let idx = row_start + self.cursor_x + i;
            if idx < self.cells.len() && self.cursor_x + i < self.cols {
                self.cells[idx] = Cell::default();
            }
        }
    }

    fn scroll_up(&mut self, n: usize) {
        for _ in 0..n {
            // Save top line to scrollback
            if self.scrollback.len() >= self.scrollback_limit {
                self.scrollback.pop_front();
            }
            let mut top_line = Vec::with_capacity(self.cols);
            for x in 0..self.cols {
                top_line.push(self.cells[self.scroll_top * self.cols + x]);
            }
            self.scrollback.push_back(top_line);

            // Shift lines up
            for y in self.scroll_top..self.scroll_bottom {
                for x in 0..self.cols {
                    let src = (y + 1) * self.cols + x;
                    let dst = y * self.cols + x;
                    if dst < self.cells.len() && src < self.cells.len() {
                        self.cells[dst] = self.cells[src];
                    }
                }
            }
            // Clear bottom line
            for x in 0..self.cols {
                let idx = self.scroll_bottom * self.cols + x;
                if idx < self.cells.len() {
                    self.cells[idx] = Cell::default();
                }
            }
        }
    }

    fn scroll_down(&mut self, n: usize) {
        for _ in 0..n {
            // Shift lines down
            for y in (self.scroll_top + 1..=self.scroll_bottom).rev() {
                for x in 0..self.cols {
                    let src = (y - 1) * self.cols + x;
                    let dst = y * self.cols + x;
                    if dst < self.cells.len() && src < self.cells.len() {
                        self.cells[dst] = self.cells[src];
                    }
                }
            }
            // Clear top line
            for x in 0..self.cols {
                let idx = self.scroll_top * self.cols + x;
                if idx < self.cells.len() {
                    self.cells[idx] = Cell::default();
                }
            }
        }
    }

    fn set_scroll_region(&mut self, top: usize, bottom: usize) {
        let top = if top == 0 { 1 } else { top };
        let bottom = if bottom == 0 { self.rows } else { bottom };

        if top < bottom && bottom <= self.rows {
            self.scroll_top = top - 1;
            self.scroll_bottom = bottom - 1;
            self.cursor_x = 0;
            self.cursor_y = if self.origin_mode { self.scroll_top } else { 0 };
        }
    }

    fn set_graphics_mode(&mut self, params: &[u32]) {
        if params.is_empty() {
            self.current_attrs = CellAttrs::default();
            return;
        }

        let mut i = 0;
        while i < params.len() {
            match params[i] {
                0 => self.current_attrs = CellAttrs::default(),
                1 => self.current_attrs.bold = true,
                2 => {} // Dim (not implemented)
                3 => self.current_attrs.italic = true,
                4 => self.current_attrs.underline = true,
                5 => self.current_attrs.blink = true,
                7 => self.current_attrs.reverse = true,
                8 => self.current_attrs.hidden = true,
                21 => self.current_attrs.bold = false,
                22 => self.current_attrs.bold = false,
                23 => self.current_attrs.italic = false,
                24 => self.current_attrs.underline = false,
                25 => self.current_attrs.blink = false,
                27 => self.current_attrs.reverse = false,
                28 => self.current_attrs.hidden = false,
                30..=37 => {
                    let color_idx = params[i] - 30;
                    self.current_attrs.fg = self.palette.get(
                        if self.current_attrs.bold { color_idx as u8 + 8 } else { color_idx as u8 }
                    );
                }
                38 => {
                    // Extended foreground color
                    if i + 2 < params.len() && params[i + 1] == 5 {
                        self.current_attrs.fg = self.palette.get(params[i + 2] as u8);
                        i += 2;
                    } else if i + 4 < params.len() && params[i + 1] == 2 {
                        self.current_attrs.fg = Color::new(
                            params[i + 2] as u8,
                            params[i + 3] as u8,
                            params[i + 4] as u8,
                        );
                        i += 4;
                    }
                }
                39 => self.current_attrs.fg = CellAttrs::default().fg,
                40..=47 => {
                    self.current_attrs.bg = self.palette.get((params[i] - 40) as u8);
                }
                48 => {
                    // Extended background color
                    if i + 2 < params.len() && params[i + 1] == 5 {
                        self.current_attrs.bg = self.palette.get(params[i + 2] as u8);
                        i += 2;
                    } else if i + 4 < params.len() && params[i + 1] == 2 {
                        self.current_attrs.bg = Color::new(
                            params[i + 2] as u8,
                            params[i + 3] as u8,
                            params[i + 4] as u8,
                        );
                        i += 4;
                    }
                }
                49 => self.current_attrs.bg = CellAttrs::default().bg,
                90..=97 => {
                    self.current_attrs.fg = self.palette.get((params[i] - 90 + 8) as u8);
                }
                100..=107 => {
                    self.current_attrs.bg = self.palette.get((params[i] - 100 + 8) as u8);
                }
                _ => {}
            }
            i += 1;
        }
    }

    fn device_status_report(&mut self, code: u32) {
        match code {
            5 => {
                // Status report - terminal OK
                self.input(b"\x1b[0n");
            }
            6 => {
                // Cursor position report
                let response = format_cpr(self.cursor_y + 1, self.cursor_x + 1);
                self.input(response.as_bytes());
            }
            _ => {}
        }
    }

    fn set_mode(&mut self, params: &[u32], enable: bool) {
        for &param in params {
            match param {
                4 => self.insert_mode = enable,
                20 => {} // Auto newline (not implemented)
                _ => {}
            }
        }
    }

    fn reset(&mut self) {
        self.current_attrs = CellAttrs::default();
        self.cursor_x = 0;
        self.cursor_y = 0;
        self.cursor_visible = true;
        self.scroll_top = 0;
        self.scroll_bottom = self.rows - 1;
        self.insert_mode = false;
        self.auto_wrap = true;
        self.origin_mode = false;
        for cell in &mut self.cells {
            *cell = Cell::default();
        }
    }

    /// Handle keyboard input
    pub fn handle_key(&mut self, scancode: u8, modifiers: u8, c: Option<char>) {
        let ctrl = (modifiers & 0x02) != 0;
        let alt = (modifiers & 0x04) != 0;

        // Handle special keys
        match scancode {
            0x48 => { // Up
                if alt {
                    self.input(b"\x1b\x1b[A");
                } else {
                    self.input(b"\x1b[A");
                }
                return;
            }
            0x50 => { // Down
                if alt {
                    self.input(b"\x1b\x1b[B");
                } else {
                    self.input(b"\x1b[B");
                }
                return;
            }
            0x4D => { // Right
                if alt {
                    self.input(b"\x1b\x1b[C");
                } else {
                    self.input(b"\x1b[C");
                }
                return;
            }
            0x4B => { // Left
                if alt {
                    self.input(b"\x1b\x1b[D");
                } else {
                    self.input(b"\x1b[D");
                }
                return;
            }
            0x47 => { self.input(b"\x1b[H"); return; } // Home
            0x4F => { self.input(b"\x1b[F"); return; } // End
            0x49 => { self.input(b"\x1b[5~"); return; } // Page Up
            0x51 => { self.input(b"\x1b[6~"); return; } // Page Down
            0x52 => { self.input(b"\x1b[2~"); return; } // Insert
            0x53 => { self.input(b"\x1b[3~"); return; } // Delete
            0x3B => { self.input(b"\x1bOP"); return; } // F1
            0x3C => { self.input(b"\x1bOQ"); return; } // F2
            0x3D => { self.input(b"\x1bOR"); return; } // F3
            0x3E => { self.input(b"\x1bOS"); return; } // F4
            0x3F => { self.input(b"\x1b[15~"); return; } // F5
            0x40 => { self.input(b"\x1b[17~"); return; } // F6
            0x41 => { self.input(b"\x1b[18~"); return; } // F7
            0x42 => { self.input(b"\x1b[19~"); return; } // F8
            0x43 => { self.input(b"\x1b[20~"); return; } // F9
            0x44 => { self.input(b"\x1b[21~"); return; } // F10
            _ => {}
        }

        // Handle character input
        if let Some(ch) = c {
            if ctrl {
                // Ctrl+letter produces control character
                if ch >= 'a' && ch <= 'z' {
                    let ctrl_char = (ch as u8) - b'a' + 1;
                    self.input(&[ctrl_char]);
                } else if ch >= 'A' && ch <= 'Z' {
                    let ctrl_char = (ch as u8) - b'A' + 1;
                    self.input(&[ctrl_char]);
                }
            } else if alt {
                // Alt+key sends ESC followed by key
                let mut buf = [0u8; 5];
                buf[0] = 0x1b;
                let len = ch.encode_utf8(&mut buf[1..]).len();
                self.input(&buf[..len + 1]);
            } else {
                let mut buf = [0u8; 4];
                let len = ch.encode_utf8(&mut buf).len();
                self.input(&buf[..len]);
            }
        }
    }

    /// Scroll view (for scrollback)
    pub fn scroll_view(&mut self, delta: isize) {
        if delta < 0 {
            self.scroll_offset = self.scroll_offset.saturating_add((-delta) as usize);
            self.scroll_offset = self.scroll_offset.min(self.scrollback.len());
        } else {
            self.scroll_offset = self.scroll_offset.saturating_sub(delta as usize);
        }
    }

    /// Get cell at position (considering scrollback)
    fn get_cell(&self, x: usize, y: usize) -> Cell {
        if self.scroll_offset > 0 {
            let scrollback_row = self.scrollback.len().saturating_sub(self.scroll_offset) + y;
            if scrollback_row < self.scrollback.len() {
                if let Some(row) = self.scrollback.get(scrollback_row) {
                    if x < row.len() {
                        return row[x];
                    }
                }
                return Cell::default();
            }
            let actual_y = y.saturating_sub(self.scroll_offset.saturating_sub(self.scrollback.len().saturating_sub(scrollback_row)));
            if actual_y < self.rows {
                let idx = actual_y * self.cols + x;
                if idx < self.cells.len() {
                    return self.cells[idx];
                }
            }
        }

        let idx = y * self.cols + x;
        if idx < self.cells.len() {
            self.cells[idx]
        } else {
            Cell::default()
        }
    }
}

impl Widget for Terminal {
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

        // Recalculate grid size
        let new_cols = (width - Self::PADDING * 2) / Self::CHAR_WIDTH;
        let new_rows = (height - Self::PADDING * 2) / Self::CHAR_HEIGHT;

        if new_cols != self.cols || new_rows != self.rows {
            let mut new_cells = vec![Cell::default(); new_cols * new_rows];

            // Copy existing content
            for y in 0..new_rows.min(self.rows) {
                for x in 0..new_cols.min(self.cols) {
                    let old_idx = y * self.cols + x;
                    let new_idx = y * new_cols + x;
                    if old_idx < self.cells.len() {
                        new_cells[new_idx] = self.cells[old_idx];
                    }
                }
            }

            self.cols = new_cols;
            self.rows = new_rows;
            self.cells = new_cells;
            self.scroll_bottom = new_rows.saturating_sub(1);
            self.cursor_x = self.cursor_x.min(new_cols.saturating_sub(1));
            self.cursor_y = self.cursor_y.min(new_rows.saturating_sub(1));
        }
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
        match event {
            WidgetEvent::Focus => {
                self.focused = true;
                true
            }
            WidgetEvent::Blur => {
                self.focused = false;
                true
            }
            WidgetEvent::KeyDown { key, modifiers } => {
                if self.focused {
                    self.handle_key(*key, *modifiers, None);
                    true
                } else {
                    false
                }
            }
            WidgetEvent::Character { c } => {
                if self.focused {
                    self.handle_key(0, 0, Some(*c));
                    true
                } else {
                    false
                }
            }
            WidgetEvent::Scroll { delta_y, .. } => {
                self.scroll_view(*delta_y as isize);
                true
            }
            WidgetEvent::MouseDown { button: MouseButton::Left, x, y } => {
                // Start selection
                let col = ((*x - self.bounds.x) as usize).saturating_sub(Self::PADDING) / Self::CHAR_WIDTH;
                let row = ((*y - self.bounds.y) as usize).saturating_sub(Self::PADDING) / Self::CHAR_HEIGHT;
                self.selection_start = Some((col.min(self.cols - 1), row.min(self.rows - 1)));
                self.selection_end = self.selection_start;
                true
            }
            WidgetEvent::MouseMove { x, y } => {
                if self.selection_start.is_some() {
                    let col = ((*x - self.bounds.x) as usize).saturating_sub(Self::PADDING) / Self::CHAR_WIDTH;
                    let row = ((*y - self.bounds.y) as usize).saturating_sub(Self::PADDING) / Self::CHAR_HEIGHT;
                    self.selection_end = Some((col.min(self.cols - 1), row.min(self.rows - 1)));
                }
                true
            }
            WidgetEvent::MouseUp { button: MouseButton::Left, .. } => {
                // End selection
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

        // Draw background
        let default_bg = CellAttrs::default().bg;
        for py in 0..h {
            for px in 0..w {
                surface.set_pixel(x + px, y + py, default_bg);
            }
        }

        // Draw cells
        for row in 0..self.rows {
            for col in 0..self.cols {
                let cell = self.get_cell(col, row);
                let char_x = x + Self::PADDING + col * Self::CHAR_WIDTH;
                let char_y = y + Self::PADDING + row * Self::CHAR_HEIGHT;

                let (fg, bg) = if cell.attrs.reverse {
                    (cell.attrs.bg, cell.attrs.fg)
                } else {
                    (cell.attrs.fg, cell.attrs.bg)
                };

                // Draw cell background if not default
                if bg != default_bg {
                    for py in 0..Self::CHAR_HEIGHT {
                        for px in 0..Self::CHAR_WIDTH {
                            surface.set_pixel(char_x + px, char_y + py, bg);
                        }
                    }
                }

                // Draw character
                if cell.c != ' ' && !cell.attrs.hidden {
                    draw_char(surface, char_x, char_y, cell.c, fg);
                }

                // Draw underline
                if cell.attrs.underline {
                    for px in 0..Self::CHAR_WIDTH {
                        surface.set_pixel(char_x + px, char_y + Self::CHAR_HEIGHT - 1, fg);
                    }
                }
            }
        }

        // Draw cursor
        if self.focused && self.cursor_visible && self.scroll_offset == 0 {
            let cursor_x = x + Self::PADDING + self.cursor_x * Self::CHAR_WIDTH;
            let cursor_y = y + Self::PADDING + self.cursor_y * Self::CHAR_HEIGHT;

            let cursor_color = Color::new(204, 204, 204);
            for py in 0..Self::CHAR_HEIGHT {
                for px in 0..Self::CHAR_WIDTH {
                    surface.set_pixel(cursor_x + px, cursor_y + py, cursor_color);
                }
            }

            // Redraw character under cursor with inverted colors
            let cell = self.get_cell(self.cursor_x, self.cursor_y);
            if cell.c != ' ' {
                draw_char(surface, cursor_x, cursor_y, cell.c, Color::new(0, 0, 0));
            }
        }

        // Draw selection highlight
        if let (Some(start), Some(end)) = (self.selection_start, self.selection_end) {
            let (start_col, start_row) = start;
            let (end_col, end_row) = end;

            let (min_row, max_row) = if start_row <= end_row {
                (start_row, end_row)
            } else {
                (end_row, start_row)
            };

            for row in min_row..=max_row {
                let (col_start, col_end) = if row == min_row && row == max_row {
                    if start_col <= end_col { (start_col, end_col) } else { (end_col, start_col) }
                } else if row == min_row {
                    if start_row <= end_row { (start_col, self.cols - 1) } else { (0, start_col) }
                } else if row == max_row {
                    if start_row <= end_row { (0, end_col) } else { (end_col, self.cols - 1) }
                } else {
                    (0, self.cols - 1)
                };

                for col in col_start..=col_end {
                    let sel_x = x + Self::PADDING + col * Self::CHAR_WIDTH;
                    let sel_y = y + Self::PADDING + row * Self::CHAR_HEIGHT;

                    // Semi-transparent selection
                    for py in 0..Self::CHAR_HEIGHT {
                        for px in 0..Self::CHAR_WIDTH {
                            if let Some(old) = surface.get_pixel(sel_x + px, sel_y + py) {
                                let blended = Color::new(
                                    ((old.r as u16 + 100) / 2) as u8,
                                    ((old.g as u16 + 100) / 2) as u8,
                                    ((old.b as u16 + 200) / 2) as u8,
                                );
                                surface.set_pixel(sel_x + px, sel_y + py, blended);
                            }
                        }
                    }
                }
            }
        }
    }
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

fn format_cpr(row: usize, col: usize) -> String {
    use alloc::string::ToString;
    let mut s = String::from("\x1b[");
    s.push_str(&row.to_string());
    s.push(';');
    s.push_str(&col.to_string());
    s.push('R');
    s
}
