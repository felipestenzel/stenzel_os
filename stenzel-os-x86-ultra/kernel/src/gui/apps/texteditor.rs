//! Text Editor
//!
//! A simple graphical text editor with basic editing features.

use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use crate::gui::widgets::{Widget, WidgetId, WidgetEvent, Bounds, MouseButton, theme};

/// Cursor position
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorPos {
    pub line: usize,
    pub col: usize,
}

impl CursorPos {
    pub fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }

    pub fn zero() -> Self {
        Self { line: 0, col: 0 }
    }
}

/// Text selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub start: CursorPos,
    pub end: CursorPos,
}

impl Selection {
    pub fn new(start: CursorPos, end: CursorPos) -> Self {
        Self { start, end }
    }

    /// Normalize so start <= end
    pub fn normalized(&self) -> Self {
        if self.start.line < self.end.line
            || (self.start.line == self.end.line && self.start.col <= self.end.col)
        {
            *self
        } else {
            Self {
                start: self.end,
                end: self.start,
            }
        }
    }

    /// Check if position is within selection
    pub fn contains(&self, pos: CursorPos) -> bool {
        let norm = self.normalized();
        if pos.line < norm.start.line || pos.line > norm.end.line {
            return false;
        }
        if pos.line == norm.start.line && pos.col < norm.start.col {
            return false;
        }
        if pos.line == norm.end.line && pos.col >= norm.end.col {
            return false;
        }
        true
    }

    /// Check if selection is empty
    pub fn is_empty(&self) -> bool {
        self.start.line == self.end.line && self.start.col == self.end.col
    }
}

/// Edit operation for undo/redo
#[derive(Debug, Clone)]
pub enum EditOp {
    Insert { pos: CursorPos, text: String },
    Delete { pos: CursorPos, text: String },
}

impl EditOp {
    pub fn inverse(&self) -> Self {
        match self {
            EditOp::Insert { pos, text } => EditOp::Delete {
                pos: *pos,
                text: text.clone(),
            },
            EditOp::Delete { pos, text } => EditOp::Insert {
                pos: *pos,
                text: text.clone(),
            },
        }
    }
}

/// File save callback
pub type SaveCallback = fn(&str, &str); // filename, content

/// File open callback
pub type OpenCallback = fn(&str) -> Option<String>; // filename -> content

/// Text editor widget
pub struct TextEditor {
    id: WidgetId,
    bounds: Bounds,

    // Document
    lines: Vec<String>,
    filename: Option<String>,
    modified: bool,

    // Cursor
    cursor: CursorPos,
    desired_col: usize, // For vertical navigation

    // Selection
    selection: Option<Selection>,
    selecting: bool,

    // Scroll position
    scroll_x: usize,
    scroll_y: usize,

    // View settings
    show_line_numbers: bool,
    line_number_width: usize,
    tab_size: usize,
    word_wrap: bool,

    // Undo/Redo
    undo_stack: VecDeque<EditOp>,
    redo_stack: VecDeque<EditOp>,
    undo_limit: usize,

    // Find/Replace
    find_text: String,
    replace_text: String,
    find_active: bool,
    find_results: Vec<CursorPos>,
    find_current: usize,

    // Clipboard (internal)
    clipboard: String,

    // UI state
    visible: bool,
    focused: bool,
    cursor_blink: bool,

    // Callbacks
    on_save: Option<SaveCallback>,
    on_open: Option<OpenCallback>,
}

impl TextEditor {
    const CHAR_WIDTH: usize = 8;
    const CHAR_HEIGHT: usize = 16;
    const LINE_HEIGHT: usize = 18;
    const PADDING: usize = 4;
    const HEADER_HEIGHT: usize = 24;
    const STATUS_HEIGHT: usize = 20;
    const FIND_BAR_HEIGHT: usize = 28;

    /// Create a new text editor
    pub fn new(x: isize, y: isize, width: usize, height: usize) -> Self {
        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(x, y, width, height),
            lines: vec![String::new()],
            filename: None,
            modified: false,
            cursor: CursorPos::zero(),
            desired_col: 0,
            selection: None,
            selecting: false,
            scroll_x: 0,
            scroll_y: 0,
            show_line_numbers: true,
            line_number_width: 50,
            tab_size: 4,
            word_wrap: false,
            undo_stack: VecDeque::new(),
            redo_stack: VecDeque::new(),
            undo_limit: 1000,
            find_text: String::new(),
            replace_text: String::new(),
            find_active: false,
            find_results: Vec::new(),
            find_current: 0,
            clipboard: String::new(),
            visible: true,
            focused: false,
            cursor_blink: true,
            on_save: None,
            on_open: None,
        }
    }

    /// Get filename
    pub fn filename(&self) -> Option<&str> {
        self.filename.as_deref()
    }

    /// Set filename
    pub fn set_filename(&mut self, filename: &str) {
        self.filename = Some(String::from(filename));
    }

    /// Check if modified
    pub fn is_modified(&self) -> bool {
        self.modified
    }

    /// Get content as string
    pub fn content(&self) -> String {
        self.lines.join("\n")
    }

    /// Set content from string
    pub fn set_content(&mut self, content: &str) {
        self.lines.clear();
        for line in content.split('\n') {
            self.lines.push(String::from(line));
        }
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.cursor = CursorPos::zero();
        self.selection = None;
        self.scroll_x = 0;
        self.scroll_y = 0;
        self.modified = false;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.update_line_number_width();
    }

    /// Set callbacks
    pub fn set_on_save(&mut self, callback: SaveCallback) {
        self.on_save = Some(callback);
    }

    pub fn set_on_open(&mut self, callback: OpenCallback) {
        self.on_open = Some(callback);
    }

    /// Update line number width based on line count
    fn update_line_number_width(&mut self) {
        let digits = format_usize(self.lines.len()).len();
        self.line_number_width = (digits + 2) * Self::CHAR_WIDTH;
    }

    /// Get visible rows
    fn visible_rows(&self) -> usize {
        let content_height = self.bounds.height
            .saturating_sub(Self::HEADER_HEIGHT + Self::STATUS_HEIGHT);
        if self.find_active {
            content_height.saturating_sub(Self::FIND_BAR_HEIGHT) / Self::LINE_HEIGHT
        } else {
            content_height / Self::LINE_HEIGHT
        }
    }

    /// Get visible columns
    fn visible_cols(&self) -> usize {
        let line_area = if self.show_line_numbers {
            self.bounds.width.saturating_sub(self.line_number_width + Self::PADDING * 2)
        } else {
            self.bounds.width.saturating_sub(Self::PADDING * 2)
        };
        line_area / Self::CHAR_WIDTH
    }

    /// Ensure cursor is visible
    fn ensure_cursor_visible(&mut self) {
        let visible_rows = self.visible_rows();
        let visible_cols = self.visible_cols();

        // Vertical scrolling
        if self.cursor.line < self.scroll_y {
            self.scroll_y = self.cursor.line;
        } else if self.cursor.line >= self.scroll_y + visible_rows {
            self.scroll_y = self.cursor.line.saturating_sub(visible_rows) + 1;
        }

        // Horizontal scrolling
        if self.cursor.col < self.scroll_x {
            self.scroll_x = self.cursor.col;
        } else if self.cursor.col >= self.scroll_x + visible_cols {
            self.scroll_x = self.cursor.col.saturating_sub(visible_cols) + 1;
        }
    }

    /// Get line at cursor, safely
    fn current_line(&self) -> &str {
        self.lines.get(self.cursor.line).map_or("", |s| s.as_str())
    }

    /// Get line length at cursor
    fn current_line_len(&self) -> usize {
        self.current_line().chars().count()
    }

    /// Record edit operation for undo
    fn record_op(&mut self, op: EditOp) {
        if self.undo_stack.len() >= self.undo_limit {
            self.undo_stack.pop_front();
        }
        self.undo_stack.push_back(op);
        self.redo_stack.clear();
        self.modified = true;
    }

    /// Move cursor
    fn move_cursor(&mut self, new_pos: CursorPos, extend_selection: bool) {
        if extend_selection {
            if self.selection.is_none() {
                self.selection = Some(Selection::new(self.cursor, self.cursor));
            }
            if let Some(ref mut sel) = self.selection {
                sel.end = new_pos;
            }
        } else {
            self.selection = None;
        }
        self.cursor = new_pos;
        self.ensure_cursor_visible();
    }

    /// Cursor left
    pub fn cursor_left(&mut self, extend_selection: bool) {
        let new_pos = if self.cursor.col > 0 {
            CursorPos::new(self.cursor.line, self.cursor.col - 1)
        } else if self.cursor.line > 0 {
            let prev_len = self.lines.get(self.cursor.line - 1)
                .map(|l| l.chars().count())
                .unwrap_or(0);
            CursorPos::new(self.cursor.line - 1, prev_len)
        } else {
            self.cursor
        };
        self.move_cursor(new_pos, extend_selection);
        self.desired_col = self.cursor.col;
    }

    /// Cursor right
    pub fn cursor_right(&mut self, extend_selection: bool) {
        let line_len = self.current_line_len();
        let new_pos = if self.cursor.col < line_len {
            CursorPos::new(self.cursor.line, self.cursor.col + 1)
        } else if self.cursor.line + 1 < self.lines.len() {
            CursorPos::new(self.cursor.line + 1, 0)
        } else {
            self.cursor
        };
        self.move_cursor(new_pos, extend_selection);
        self.desired_col = self.cursor.col;
    }

    /// Cursor up
    pub fn cursor_up(&mut self, extend_selection: bool) {
        if self.cursor.line > 0 {
            let new_line = self.cursor.line - 1;
            let new_col = self.desired_col.min(
                self.lines.get(new_line)
                    .map(|l| l.chars().count())
                    .unwrap_or(0)
            );
            self.move_cursor(CursorPos::new(new_line, new_col), extend_selection);
        }
    }

    /// Cursor down
    pub fn cursor_down(&mut self, extend_selection: bool) {
        if self.cursor.line + 1 < self.lines.len() {
            let new_line = self.cursor.line + 1;
            let new_col = self.desired_col.min(
                self.lines.get(new_line)
                    .map(|l| l.chars().count())
                    .unwrap_or(0)
            );
            self.move_cursor(CursorPos::new(new_line, new_col), extend_selection);
        }
    }

    /// Cursor home (start of line)
    pub fn cursor_home(&mut self, extend_selection: bool) {
        self.move_cursor(CursorPos::new(self.cursor.line, 0), extend_selection);
        self.desired_col = 0;
    }

    /// Cursor end (end of line)
    pub fn cursor_end(&mut self, extend_selection: bool) {
        let col = self.current_line_len();
        self.move_cursor(CursorPos::new(self.cursor.line, col), extend_selection);
        self.desired_col = col;
    }

    /// Page up
    pub fn page_up(&mut self, extend_selection: bool) {
        let rows = self.visible_rows();
        let new_line = self.cursor.line.saturating_sub(rows);
        let new_col = self.desired_col.min(
            self.lines.get(new_line)
                .map(|l| l.chars().count())
                .unwrap_or(0)
        );
        self.move_cursor(CursorPos::new(new_line, new_col), extend_selection);
        self.scroll_y = self.scroll_y.saturating_sub(rows);
    }

    /// Page down
    pub fn page_down(&mut self, extend_selection: bool) {
        let rows = self.visible_rows();
        let new_line = (self.cursor.line + rows).min(self.lines.len().saturating_sub(1));
        let new_col = self.desired_col.min(
            self.lines.get(new_line)
                .map(|l| l.chars().count())
                .unwrap_or(0)
        );
        self.move_cursor(CursorPos::new(new_line, new_col), extend_selection);
        self.scroll_y = (self.scroll_y + rows).min(self.lines.len().saturating_sub(rows));
    }

    /// Insert character at cursor
    pub fn insert_char(&mut self, c: char) {
        // Delete selection first if any
        if let Some(sel) = self.selection.take() {
            self.delete_selection(sel);
        }

        if c == '\n' {
            // Split line
            let line = &self.lines[self.cursor.line];
            let (before, after): (String, String) = {
                let chars: Vec<char> = line.chars().collect();
                let before: String = chars[..self.cursor.col].iter().collect();
                let after: String = chars[self.cursor.col..].iter().collect();
                (before, after)
            };

            self.lines[self.cursor.line] = before;
            self.lines.insert(self.cursor.line + 1, after);

            self.record_op(EditOp::Insert {
                pos: self.cursor,
                text: String::from("\n"),
            });

            self.cursor.line += 1;
            self.cursor.col = 0;
            self.desired_col = 0;
            self.update_line_number_width();
        } else {
            // Insert character
            let line = &mut self.lines[self.cursor.line];
            let mut chars: Vec<char> = line.chars().collect();
            if self.cursor.col > chars.len() {
                self.cursor.col = chars.len();
            }
            chars.insert(self.cursor.col, c);
            *line = chars.into_iter().collect();

            self.record_op(EditOp::Insert {
                pos: self.cursor,
                text: String::from(c),
            });

            self.cursor.col += 1;
            self.desired_col = self.cursor.col;
        }

        self.ensure_cursor_visible();
    }

    /// Insert string at cursor
    pub fn insert_str(&mut self, s: &str) {
        for c in s.chars() {
            self.insert_char(c);
        }
    }

    /// Delete character before cursor (backspace)
    pub fn backspace(&mut self) {
        // Delete selection first if any
        if let Some(sel) = self.selection.take() {
            self.delete_selection(sel);
            return;
        }

        if self.cursor.col > 0 {
            // Delete char in line
            let line = &mut self.lines[self.cursor.line];
            let mut chars: Vec<char> = line.chars().collect();
            let deleted = chars.remove(self.cursor.col - 1);
            *line = chars.into_iter().collect();

            self.record_op(EditOp::Delete {
                pos: CursorPos::new(self.cursor.line, self.cursor.col - 1),
                text: String::from(deleted),
            });

            self.cursor.col -= 1;
        } else if self.cursor.line > 0 {
            // Join with previous line
            let current_line = self.lines.remove(self.cursor.line);
            let prev_line = &mut self.lines[self.cursor.line - 1];
            let new_col = prev_line.chars().count();
            prev_line.push_str(&current_line);

            self.record_op(EditOp::Delete {
                pos: CursorPos::new(self.cursor.line - 1, new_col),
                text: String::from("\n"),
            });

            self.cursor.line -= 1;
            self.cursor.col = new_col;
            self.update_line_number_width();
        }

        self.desired_col = self.cursor.col;
        self.ensure_cursor_visible();
    }

    /// Delete character at cursor (delete key)
    pub fn delete(&mut self) {
        // Delete selection first if any
        if let Some(sel) = self.selection.take() {
            self.delete_selection(sel);
            return;
        }

        let line_len = self.current_line_len();
        if self.cursor.col < line_len {
            // Delete char in line
            let line = &mut self.lines[self.cursor.line];
            let mut chars: Vec<char> = line.chars().collect();
            let deleted = chars.remove(self.cursor.col);
            *line = chars.into_iter().collect();

            self.record_op(EditOp::Delete {
                pos: self.cursor,
                text: String::from(deleted),
            });
        } else if self.cursor.line + 1 < self.lines.len() {
            // Join with next line
            let next_line = self.lines.remove(self.cursor.line + 1);
            self.lines[self.cursor.line].push_str(&next_line);

            self.record_op(EditOp::Delete {
                pos: self.cursor,
                text: String::from("\n"),
            });

            self.update_line_number_width();
        }
    }

    /// Delete selection
    fn delete_selection(&mut self, sel: Selection) {
        let sel = sel.normalized();

        // Get selected text first (for undo)
        let selected_text = self.get_selection_text(&sel);

        if sel.start.line == sel.end.line {
            // Single line deletion
            let line = &mut self.lines[sel.start.line];
            let chars: Vec<char> = line.chars().collect();
            let new_line: String = chars[..sel.start.col]
                .iter()
                .chain(chars[sel.end.col..].iter())
                .collect();
            *line = new_line;
        } else {
            // Multi-line deletion
            let start_line = &self.lines[sel.start.line];
            let end_line = &self.lines[sel.end.line];

            let start_chars: Vec<char> = start_line.chars().collect();
            let end_chars: Vec<char> = end_line.chars().collect();

            let new_line: String = start_chars[..sel.start.col]
                .iter()
                .chain(end_chars[sel.end.col..].iter())
                .collect();

            // Remove lines between
            for _ in sel.start.line..=sel.end.line {
                if sel.start.line < self.lines.len() {
                    self.lines.remove(sel.start.line);
                }
            }
            self.lines.insert(sel.start.line, new_line);
        }

        self.record_op(EditOp::Delete {
            pos: sel.start,
            text: selected_text,
        });

        self.cursor = sel.start;
        self.desired_col = self.cursor.col;
        self.update_line_number_width();
        self.ensure_cursor_visible();
    }

    /// Get text in selection
    fn get_selection_text(&self, sel: &Selection) -> String {
        let sel = sel.normalized();

        if sel.start.line == sel.end.line {
            let line = &self.lines[sel.start.line];
            let chars: Vec<char> = line.chars().collect();
            chars[sel.start.col..sel.end.col].iter().collect()
        } else {
            let mut result = String::new();

            // First line
            let first = &self.lines[sel.start.line];
            let chars: Vec<char> = first.chars().collect();
            result.push_str(&chars[sel.start.col..].iter().collect::<String>());
            result.push('\n');

            // Middle lines
            for line_idx in (sel.start.line + 1)..sel.end.line {
                result.push_str(&self.lines[line_idx]);
                result.push('\n');
            }

            // Last line
            let last = &self.lines[sel.end.line];
            let chars: Vec<char> = last.chars().collect();
            result.push_str(&chars[..sel.end.col].iter().collect::<String>());

            result
        }
    }

    /// Select all
    pub fn select_all(&mut self) {
        let last_line = self.lines.len().saturating_sub(1);
        let last_col = self.lines.get(last_line)
            .map(|l| l.chars().count())
            .unwrap_or(0);

        self.selection = Some(Selection::new(
            CursorPos::zero(),
            CursorPos::new(last_line, last_col),
        ));
        self.cursor = CursorPos::new(last_line, last_col);
    }

    /// Copy selection to clipboard
    pub fn copy(&mut self) {
        if let Some(sel) = &self.selection {
            self.clipboard = self.get_selection_text(sel);
        }
    }

    /// Cut selection to clipboard
    pub fn cut(&mut self) {
        self.copy();
        if let Some(sel) = self.selection.take() {
            self.delete_selection(sel);
        }
    }

    /// Paste from clipboard
    pub fn paste(&mut self) {
        let text = self.clipboard.clone();
        self.insert_str(&text);
    }

    /// Undo last operation
    pub fn undo(&mut self) {
        if let Some(op) = self.undo_stack.pop_back() {
            self.apply_op(&op.inverse());
            self.redo_stack.push_back(op);
        }
    }

    /// Redo last undone operation
    pub fn redo(&mut self) {
        if let Some(op) = self.redo_stack.pop_back() {
            self.apply_op(&op);
            self.undo_stack.push_back(op);
        }
    }

    /// Apply edit operation (without recording)
    fn apply_op(&mut self, op: &EditOp) {
        match op {
            EditOp::Insert { pos, text } => {
                self.cursor = *pos;
                self.selection = None;
                for c in text.chars() {
                    if c == '\n' {
                        let line = &self.lines[self.cursor.line];
                        let chars: Vec<char> = line.chars().collect();
                        let before: String = chars[..self.cursor.col].iter().collect();
                        let after: String = chars[self.cursor.col..].iter().collect();
                        self.lines[self.cursor.line] = before;
                        self.lines.insert(self.cursor.line + 1, after);
                        self.cursor.line += 1;
                        self.cursor.col = 0;
                    } else {
                        let line = &mut self.lines[self.cursor.line];
                        let mut chars: Vec<char> = line.chars().collect();
                        chars.insert(self.cursor.col, c);
                        *line = chars.into_iter().collect();
                        self.cursor.col += 1;
                    }
                }
            }
            EditOp::Delete { pos, text } => {
                // Calculate end position
                let mut end = *pos;
                for c in text.chars() {
                    if c == '\n' {
                        end.line += 1;
                        end.col = 0;
                    } else {
                        end.col += 1;
                    }
                }

                let sel = Selection::new(*pos, end);
                self.cursor = *pos;
                self.selection = None;

                // Delete without recording
                let sel_norm = sel.normalized();
                if sel_norm.start.line == sel_norm.end.line {
                    let line = &mut self.lines[sel_norm.start.line];
                    let chars: Vec<char> = line.chars().collect();
                    let new_line: String = chars[..sel_norm.start.col]
                        .iter()
                        .chain(chars[sel_norm.end.col..].iter())
                        .collect();
                    *line = new_line;
                } else {
                    let start_line = &self.lines[sel_norm.start.line];
                    let end_line = &self.lines[sel_norm.end.line];
                    let start_chars: Vec<char> = start_line.chars().collect();
                    let end_chars: Vec<char> = end_line.chars().collect();
                    let new_line: String = start_chars[..sel_norm.start.col]
                        .iter()
                        .chain(end_chars[sel_norm.end.col..].iter())
                        .collect();
                    for _ in sel_norm.start.line..=sel_norm.end.line {
                        if sel_norm.start.line < self.lines.len() {
                            self.lines.remove(sel_norm.start.line);
                        }
                    }
                    self.lines.insert(sel_norm.start.line, new_line);
                }

                self.cursor = *pos;
            }
        }

        self.update_line_number_width();
        self.ensure_cursor_visible();
    }

    /// Toggle find bar
    pub fn toggle_find(&mut self) {
        self.find_active = !self.find_active;
        if !self.find_active {
            self.find_results.clear();
        }
    }

    /// Find next occurrence
    pub fn find_next(&mut self) {
        if self.find_text.is_empty() {
            return;
        }

        // Simple search - could be optimized
        self.find_results.clear();

        for (line_idx, line) in self.lines.iter().enumerate() {
            let mut start = 0;
            while let Some(pos) = line[start..].find(&self.find_text) {
                let col = line[..start + pos].chars().count();
                self.find_results.push(CursorPos::new(line_idx, col));
                start += pos + 1;
            }
        }

        if !self.find_results.is_empty() {
            // Find next from cursor
            let current = self.find_results.iter()
                .position(|p| p.line > self.cursor.line
                    || (p.line == self.cursor.line && p.col > self.cursor.col))
                .unwrap_or(0);

            self.find_current = current;
            self.cursor = self.find_results[current];
            self.selection = Some(Selection::new(
                self.cursor,
                CursorPos::new(self.cursor.line, self.cursor.col + self.find_text.chars().count()),
            ));
            self.ensure_cursor_visible();
        }
    }

    /// Find previous occurrence
    pub fn find_prev(&mut self) {
        if self.find_results.is_empty() {
            return;
        }

        if self.find_current > 0 {
            self.find_current -= 1;
        } else {
            self.find_current = self.find_results.len() - 1;
        }

        self.cursor = self.find_results[self.find_current];
        self.selection = Some(Selection::new(
            self.cursor,
            CursorPos::new(self.cursor.line, self.cursor.col + self.find_text.chars().count()),
        ));
        self.ensure_cursor_visible();
    }

    /// Replace current and find next
    pub fn replace(&mut self) {
        if self.find_results.is_empty() {
            return;
        }

        // Delete found text
        if let Some(sel) = self.selection.take() {
            self.delete_selection(sel);
        }

        // Insert replacement
        self.insert_str(&self.replace_text.clone());

        // Find next
        self.find_next();
    }

    /// Replace all occurrences
    pub fn replace_all(&mut self) {
        while !self.find_results.is_empty() {
            self.replace();
        }
    }

    /// Get cursor position at screen coordinates
    fn pos_at_point(&self, x: isize, y: isize) -> Option<CursorPos> {
        let local_x = (x - self.bounds.x) as usize;
        let local_y = (y - self.bounds.y) as usize;

        // Check bounds
        let content_y = Self::HEADER_HEIGHT;
        let content_x = if self.show_line_numbers {
            self.line_number_width
        } else {
            0
        } + Self::PADDING;

        if local_y < content_y || local_x < content_x {
            return None;
        }

        let row = (local_y - content_y) / Self::LINE_HEIGHT + self.scroll_y;
        let col = (local_x - content_x) / Self::CHAR_WIDTH + self.scroll_x;

        if row >= self.lines.len() {
            return None;
        }

        let line_len = self.lines[row].chars().count();
        Some(CursorPos::new(row, col.min(line_len)))
    }

    /// Handle keyboard input
    fn handle_key(&mut self, scancode: u8, modifiers: u8) -> bool {
        let shift = (modifiers & 0x01) != 0;
        let ctrl = (modifiers & 0x02) != 0;

        match scancode {
            // Arrow keys
            0x48 => { self.cursor_up(shift); true }
            0x50 => { self.cursor_down(shift); true }
            0x4B => { self.cursor_left(shift); true }
            0x4D => { self.cursor_right(shift); true }

            // Home/End
            0x47 => { self.cursor_home(shift); true }
            0x4F => { self.cursor_end(shift); true }

            // Page Up/Down
            0x49 => { self.page_up(shift); true }
            0x51 => { self.page_down(shift); true }

            // Backspace
            0x0E => { self.backspace(); true }

            // Delete
            0x53 => { self.delete(); true }

            // Enter
            0x1C => { self.insert_char('\n'); true }

            // Tab
            0x0F => {
                for _ in 0..self.tab_size {
                    self.insert_char(' ');
                }
                true
            }

            // Ctrl+key combinations
            _ if ctrl => {
                match scancode {
                    0x1E => { self.select_all(); true }          // Ctrl+A
                    0x2E => { self.copy(); true }                // Ctrl+C
                    0x2D => { self.cut(); true }                 // Ctrl+X
                    0x2F => { self.paste(); true }               // Ctrl+V
                    0x2C => { self.undo(); true }                // Ctrl+Z
                    0x15 => { self.redo(); true }                // Ctrl+Y
                    0x21 => { self.toggle_find(); true }         // Ctrl+F
                    0x22 => { self.find_next(); true }           // Ctrl+G
                    0x1F => {                                    // Ctrl+S - Save
                        if let (Some(callback), Some(filename)) = (self.on_save, &self.filename) {
                            callback(filename, &self.content());
                            self.modified = false;
                        }
                        true
                    }
                    _ => false,
                }
            }

            _ => false,
        }
    }
}

impl Widget for TextEditor {
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
                    self.handle_key(*key, *modifiers)
                } else {
                    false
                }
            }
            WidgetEvent::Character { c } => {
                if self.focused && *c >= ' ' && *c != '\x7f' {
                    self.insert_char(*c);
                    true
                } else {
                    false
                }
            }
            WidgetEvent::MouseDown { button: MouseButton::Left, x, y } => {
                if let Some(pos) = self.pos_at_point(*x, *y) {
                    self.cursor = pos;
                    self.desired_col = pos.col;
                    self.selection = Some(Selection::new(pos, pos));
                    self.selecting = true;
                    return true;
                }
                false
            }
            WidgetEvent::MouseMove { x, y } => {
                if self.selecting {
                    if let Some(pos) = self.pos_at_point(*x, *y) {
                        self.cursor = pos;
                        if let Some(ref mut sel) = self.selection {
                            sel.end = pos;
                        }
                    }
                }
                true
            }
            WidgetEvent::MouseUp { button: MouseButton::Left, .. } => {
                self.selecting = false;
                // Clear selection if empty
                if let Some(sel) = &self.selection {
                    if sel.is_empty() {
                        self.selection = None;
                    }
                }
                true
            }
            WidgetEvent::Scroll { delta_y, .. } => {
                if *delta_y < 0 {
                    self.scroll_y = self.scroll_y.saturating_add(3);
                    let max = self.lines.len().saturating_sub(self.visible_rows());
                    self.scroll_y = self.scroll_y.min(max);
                } else {
                    self.scroll_y = self.scroll_y.saturating_sub(3);
                }
                true
            }
            WidgetEvent::DoubleClick { x, y, .. } => {
                // Select word at position
                if let Some(pos) = self.pos_at_point(*x, *y) {
                    let line = &self.lines[pos.line];
                    let chars: Vec<char> = line.chars().collect();

                    // Find word boundaries
                    let mut start = pos.col;
                    let mut end = pos.col;

                    while start > 0 && chars.get(start - 1).map_or(false, |c| c.is_alphanumeric() || *c == '_') {
                        start -= 1;
                    }
                    while end < chars.len() && chars.get(end).map_or(false, |c| c.is_alphanumeric() || *c == '_') {
                        end += 1;
                    }

                    if start < end {
                        self.selection = Some(Selection::new(
                            CursorPos::new(pos.line, start),
                            CursorPos::new(pos.line, end),
                        ));
                        self.cursor = CursorPos::new(pos.line, end);
                    }
                    return true;
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

        // Background
        let editor_bg = Color::new(30, 30, 30);
        for py in 0..h {
            for px in 0..w {
                surface.set_pixel(x + px, y + py, editor_bg);
            }
        }

        // Header bar
        let header_bg = Color::new(45, 45, 48);
        for py in 0..Self::HEADER_HEIGHT {
            for px in 0..w {
                surface.set_pixel(x + px, y + py, header_bg);
            }
        }

        // Title (filename)
        let title = self.filename.as_deref().unwrap_or("Untitled");
        let title_with_mod = if self.modified {
            let mut t = String::from("* ");
            t.push_str(title);
            t
        } else {
            String::from(title)
        };
        for (i, c) in title_with_mod.chars().take(50).enumerate() {
            draw_char_simple(surface, x + 8 + i * Self::CHAR_WIDTH, y + 4, c, theme.fg);
        }

        // Content area
        let content_x = x;
        let content_y = y + Self::HEADER_HEIGHT;
        let content_h = h.saturating_sub(Self::HEADER_HEIGHT + Self::STATUS_HEIGHT);

        // Line numbers background
        if self.show_line_numbers {
            let ln_bg = Color::new(37, 37, 38);
            for py in 0..content_h {
                for px in 0..self.line_number_width {
                    surface.set_pixel(content_x + px, content_y + py, ln_bg);
                }
            }
        }

        // Text area start
        let text_x = if self.show_line_numbers {
            content_x + self.line_number_width + Self::PADDING
        } else {
            content_x + Self::PADDING
        };

        // Render visible lines
        let visible_rows = self.visible_rows();
        for row_idx in 0..visible_rows {
            let line_idx = self.scroll_y + row_idx;
            if line_idx >= self.lines.len() {
                break;
            }

            let line_y = content_y + row_idx * Self::LINE_HEIGHT;

            // Line number
            if self.show_line_numbers {
                let ln_str = format_usize(line_idx + 1);
                let ln_x = content_x + self.line_number_width - (ln_str.len() + 1) * Self::CHAR_WIDTH;
                let ln_color = if line_idx == self.cursor.line {
                    Color::new(180, 180, 180)
                } else {
                    Color::new(100, 100, 100)
                };
                for (i, c) in ln_str.chars().enumerate() {
                    draw_char_simple(surface, ln_x + i * Self::CHAR_WIDTH, line_y + 1, c, ln_color);
                }
            }

            // Current line highlight
            if line_idx == self.cursor.line && self.focused {
                let highlight = Color::new(40, 40, 44);
                let text_w = w.saturating_sub(self.line_number_width + Self::PADDING);
                for py in 0..Self::LINE_HEIGHT {
                    for px in 0..text_w {
                        surface.set_pixel(text_x + px, line_y + py, highlight);
                    }
                }
            }

            // Line text
            let line = &self.lines[line_idx];
            let chars: Vec<char> = line.chars().collect();
            let visible_cols = self.visible_cols();

            for col_idx in 0..visible_cols {
                let char_idx = self.scroll_x + col_idx;
                if char_idx >= chars.len() {
                    break;
                }

                let char_x = text_x + col_idx * Self::CHAR_WIDTH;
                let char_y = line_y + 1;

                // Check if in selection
                let in_selection = if let Some(ref sel) = self.selection {
                    sel.contains(CursorPos::new(line_idx, char_idx))
                } else {
                    false
                };

                // Draw selection background
                if in_selection {
                    let sel_color = Color::new(38, 79, 120);
                    for py in 0..Self::LINE_HEIGHT {
                        for px in 0..Self::CHAR_WIDTH {
                            surface.set_pixel(char_x + px, line_y + py, sel_color);
                        }
                    }
                }

                // Draw character
                let c = chars[char_idx];
                let fg = if in_selection {
                    Color::new(255, 255, 255)
                } else {
                    Color::new(212, 212, 212)
                };
                draw_char_simple(surface, char_x, char_y, c, fg);
            }
        }

        // Cursor
        if self.focused && self.cursor_blink {
            let cursor_row = self.cursor.line.saturating_sub(self.scroll_y);
            let cursor_col = self.cursor.col.saturating_sub(self.scroll_x);

            if cursor_row < visible_rows && self.cursor.line >= self.scroll_y {
                let cursor_x = text_x + cursor_col * Self::CHAR_WIDTH;
                let cursor_y = content_y + cursor_row * Self::LINE_HEIGHT;

                let cursor_color = Color::new(255, 255, 255);
                for py in 0..Self::LINE_HEIGHT {
                    surface.set_pixel(cursor_x, cursor_y + py, cursor_color);
                    surface.set_pixel(cursor_x + 1, cursor_y + py, cursor_color);
                }
            }
        }

        // Scrollbar
        if self.lines.len() > visible_rows {
            let sb_x = x + w - 12;
            let sb_y = content_y;
            let sb_h = content_h;
            let sb_track = Color::new(50, 50, 53);

            for py in 0..sb_h {
                for px in 0..8 {
                    surface.set_pixel(sb_x + px, sb_y + py, sb_track);
                }
            }

            let total = self.lines.len() as f32;
            let visible = visible_rows as f32;
            let thumb_h = ((visible / total) * sb_h as f32).max(20.0) as usize;
            let thumb_pos = ((self.scroll_y as f32 / total) * sb_h as f32) as usize;
            let thumb_color = Color::new(100, 100, 100);

            for py in 0..thumb_h {
                for px in 0..8 {
                    let ty = sb_y + thumb_pos + py;
                    if ty < sb_y + sb_h {
                        surface.set_pixel(sb_x + px, ty, thumb_color);
                    }
                }
            }
        }

        // Status bar
        let status_y = y + h - Self::STATUS_HEIGHT;
        let status_bg = Color::new(0, 122, 204);
        for py in 0..Self::STATUS_HEIGHT {
            for px in 0..w {
                surface.set_pixel(x + px, status_y + py, status_bg);
            }
        }

        // Status text: line, column
        let status = format_status(self.cursor.line + 1, self.cursor.col + 1, self.lines.len());
        for (i, c) in status.chars().enumerate() {
            draw_char_simple(surface, x + 8 + i * Self::CHAR_WIDTH, status_y + 2, c, Color::new(255, 255, 255));
        }

        // Modified indicator in status
        if self.modified {
            let mod_x = x + w - 80;
            for (i, c) in "Modified".chars().enumerate() {
                draw_char_simple(surface, mod_x + i * Self::CHAR_WIDTH, status_y + 2, c, Color::new(255, 255, 255));
            }
        }

        // Find bar (if active)
        if self.find_active {
            let find_y = status_y - Self::FIND_BAR_HEIGHT;
            let find_bg = Color::new(60, 60, 63);
            for py in 0..Self::FIND_BAR_HEIGHT {
                for px in 0..w {
                    surface.set_pixel(x + px, find_y + py, find_bg);
                }
            }

            // "Find:" label
            for (i, c) in "Find:".chars().enumerate() {
                draw_char_simple(surface, x + 8 + i * Self::CHAR_WIDTH, find_y + 6, c, theme.fg);
            }

            // Find text box
            let find_input_x = x + 56;
            let find_input_w = 200;
            for py in 0..20 {
                for px in 0..find_input_w {
                    surface.set_pixel(find_input_x + px, find_y + 4 + py, Color::new(45, 45, 48));
                }
            }
            for (i, c) in self.find_text.chars().take(24).enumerate() {
                draw_char_simple(surface, find_input_x + 4 + i * Self::CHAR_WIDTH, find_y + 6, c, theme.fg);
            }

            // Results count
            let results_str = format_find_results(self.find_results.len(), self.find_current);
            let results_x = find_input_x + find_input_w + 16;
            for (i, c) in results_str.chars().enumerate() {
                draw_char_simple(surface, results_x + i * Self::CHAR_WIDTH, find_y + 6, c, theme.fg);
            }
        }
    }
}

fn format_usize(n: usize) -> String {
    use alloc::string::ToString;
    n.to_string()
}

fn format_status(line: usize, col: usize, total_lines: usize) -> String {
    use alloc::string::ToString;
    let mut s = String::from("Ln ");
    s.push_str(&line.to_string());
    s.push_str(", Col ");
    s.push_str(&col.to_string());
    s.push_str("  |  ");
    s.push_str(&total_lines.to_string());
    s.push_str(" lines");
    s
}

fn format_find_results(count: usize, current: usize) -> String {
    use alloc::string::ToString;
    if count == 0 {
        String::from("No results")
    } else {
        let mut s = (current + 1).to_string();
        s.push('/');
        s.push_str(&count.to_string());
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
