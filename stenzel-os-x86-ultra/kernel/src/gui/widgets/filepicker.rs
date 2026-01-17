//! File Picker widget
//!
//! File open/save dialogs for selecting files and directories.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use super::{Widget, WidgetId, WidgetState, WidgetEvent, Bounds, MouseButton, theme};
use super::button::Button;
use super::textbox::TextBox;
use super::listview::{ListView, ListItem};
use super::dialog::{Dialog, DialogResult, DialogCallback};

/// File picker mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilePickerMode {
    /// Open file(s)
    Open,
    /// Save file
    Save,
    /// Select directory
    SelectDirectory,
}

/// File entry for display
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name: String,
    pub is_directory: bool,
    pub size: u64,
    pub modified: u64, // Timestamp
}

impl FileEntry {
    pub fn new(name: &str, is_directory: bool) -> Self {
        Self {
            name: String::from(name),
            is_directory,
            size: 0,
            modified: 0,
        }
    }

    pub fn directory(name: &str) -> Self {
        Self::new(name, true)
    }

    pub fn file(name: &str, size: u64) -> Self {
        Self {
            name: String::from(name),
            is_directory: false,
            size,
            modified: 0,
        }
    }

    /// Format size for display
    pub fn format_size(&self) -> String {
        if self.is_directory {
            return String::from("<DIR>");
        }

        if self.size < 1024 {
            format_num(self.size, " B")
        } else if self.size < 1024 * 1024 {
            format_num(self.size / 1024, " KB")
        } else if self.size < 1024 * 1024 * 1024 {
            format_num(self.size / (1024 * 1024), " MB")
        } else {
            format_num(self.size / (1024 * 1024 * 1024), " GB")
        }
    }
}

fn format_num(n: u64, suffix: &str) -> String {
    use alloc::string::ToString;
    let mut s = n.to_string();
    s.push_str(suffix);
    s
}

/// File picker callback
pub type FilePickerCallback = fn(WidgetId, DialogResult, &str);

/// File picker dialog
pub struct FilePicker {
    dialog: Dialog,
    mode: FilePickerMode,
    current_path: String,
    entries: Vec<FileEntry>,
    file_list: ListView,
    path_textbox: TextBox,
    filename_textbox: TextBox,
    ok_button: Button,
    cancel_button: Button,
    up_button: Button,
    selected_path: String,
    filter: Option<String>, // File extension filter
    on_complete: Option<FilePickerCallback>,
}

impl FilePicker {
    /// Create a new file picker
    pub fn new(mode: FilePickerMode) -> Self {
        let title = match mode {
            FilePickerMode::Open => "Open File",
            FilePickerMode::Save => "Save File",
            FilePickerMode::SelectDirectory => "Select Directory",
        };

        let mut dialog = Dialog::new(title, 500, 400);
        let content = dialog.content_bounds();

        // Path bar
        let path_textbox = TextBox::new(
            content.x + 35,
            content.y + 10,
            content.width - 50,
        );

        // Up button
        let up_button = Button::new(content.x + 5, content.y + 8, 25, 24, "^");

        // File list
        let file_list = ListView::new(
            content.x + 5,
            content.y + 45,
            content.width - 10,
            content.height - 120,
        );

        // Filename input (for save mode)
        let filename_textbox = TextBox::new(
            content.x + 80,
            content.y + content.height as isize - 65,
            content.width - 95,
        );

        // Buttons
        let button_y = content.y + content.height as isize - 35;
        let ok_label = match mode {
            FilePickerMode::Open => "Open",
            FilePickerMode::Save => "Save",
            FilePickerMode::SelectDirectory => "Select",
        };
        let ok_button = Button::new(content.x + content.width as isize - 180, button_y, 80, 28, ok_label);
        let cancel_button = Button::new(content.x + content.width as isize - 90, button_y, 80, 28, "Cancel");

        Self {
            dialog,
            mode,
            current_path: String::from("/"),
            entries: Vec::new(),
            file_list,
            path_textbox,
            filename_textbox,
            ok_button,
            cancel_button,
            up_button,
            selected_path: String::new(),
            filter: None,
            on_complete: None,
        }
    }

    /// Create open file picker
    pub fn open() -> Self {
        Self::new(FilePickerMode::Open)
    }

    /// Create save file picker
    pub fn save() -> Self {
        Self::new(FilePickerMode::Save)
    }

    /// Create directory picker
    pub fn select_directory() -> Self {
        Self::new(FilePickerMode::SelectDirectory)
    }

    /// Set file filter (e.g., "*.txt")
    pub fn set_filter(&mut self, filter: &str) {
        self.filter = Some(String::from(filter));
    }

    /// Set initial path
    pub fn set_path(&mut self, path: &str) {
        self.current_path = String::from(path);
        self.path_textbox.set_text(path);
        // Would trigger refresh of file list in real implementation
    }

    /// Set initial filename (for save mode)
    pub fn set_filename(&mut self, filename: &str) {
        self.filename_textbox.set_text(filename);
    }

    /// Set completion callback
    pub fn set_on_complete(&mut self, callback: FilePickerCallback) {
        self.on_complete = Some(callback);
    }

    /// Set entries (called by system to populate file list)
    pub fn set_entries(&mut self, entries: Vec<FileEntry>) {
        self.entries = entries;
        self.refresh_list();
    }

    /// Refresh the file list display
    fn refresh_list(&mut self) {
        self.file_list.clear();

        // Sort: directories first, then files
        let mut sorted: Vec<&FileEntry> = self.entries.iter().collect();
        sorted.sort_by(|a, b| {
            match (a.is_directory, b.is_directory) {
                (true, false) => core::cmp::Ordering::Less,
                (false, true) => core::cmp::Ordering::Greater,
                _ => a.name.cmp(&b.name),
            }
        });

        for entry in sorted {
            // Apply filter for files
            if !entry.is_directory {
                if let Some(ref filter) = self.filter {
                    if !Self::matches_filter(&entry.name, filter) {
                        continue;
                    }
                }
            }

            // Skip files in directory mode
            if self.mode == FilePickerMode::SelectDirectory && !entry.is_directory {
                continue;
            }

            let icon = if entry.is_directory { "[D] " } else { "    " };
            let display = format_entry(icon, &entry.name, &entry.format_size());
            self.file_list.add(&display);
        }
    }

    /// Check if filename matches filter
    fn matches_filter(name: &str, filter: &str) -> bool {
        if filter == "*" || filter == "*.*" {
            return true;
        }

        // Simple extension matching
        if let Some(ext) = filter.strip_prefix("*.") {
            return name.to_lowercase().ends_with(&format!(".{}", ext.to_lowercase()));
        }

        true
    }

    /// Navigate to directory
    pub fn navigate(&mut self, path: &str) {
        self.current_path = String::from(path);
        self.path_textbox.set_text(path);
        // Would trigger filesystem read in real implementation
    }

    /// Go up one directory
    pub fn go_up(&mut self) {
        if self.current_path == "/" {
            return;
        }

        // Find last slash
        if let Some(pos) = self.current_path.rfind('/') {
            if pos == 0 {
                self.navigate("/");
            } else {
                let parent: String = self.current_path.chars().take(pos).collect();
                self.navigate(&parent);
            }
        }
    }

    /// Center on screen
    pub fn center(&mut self, screen_width: usize, screen_height: usize) {
        self.dialog.center(screen_width, screen_height);
        self.update_layout();
    }

    fn update_layout(&mut self) {
        let content = self.dialog.content_bounds();

        self.path_textbox.set_position(content.x + 35, content.y + 10);
        self.up_button.set_position(content.x + 5, content.y + 8);
        self.file_list.set_position(content.x + 5, content.y + 45);
        self.filename_textbox.set_position(content.x + 80, content.y + content.height as isize - 65);

        let button_y = content.y + content.height as isize - 35;
        self.ok_button.set_position(content.x + content.width as isize - 180, button_y);
        self.cancel_button.set_position(content.x + content.width as isize - 90, button_y);
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

    /// Get selected path
    pub fn selected_path(&self) -> &str {
        &self.selected_path
    }

    /// Build full path from selection
    fn build_path(&self) -> String {
        if self.mode == FilePickerMode::Save {
            let filename = self.filename_textbox.text();
            if self.current_path == "/" {
                format_path("/", filename)
            } else {
                format_path(&self.current_path, filename)
            }
        } else if let Some(index) = self.file_list.selected_index() {
            // Get actual entry from sorted list
            let mut sorted: Vec<&FileEntry> = self.entries.iter()
                .filter(|e| {
                    if self.mode == FilePickerMode::SelectDirectory {
                        e.is_directory
                    } else if !e.is_directory {
                        if let Some(ref filter) = self.filter {
                            Self::matches_filter(&e.name, filter)
                        } else {
                            true
                        }
                    } else {
                        true
                    }
                })
                .collect();
            sorted.sort_by(|a, b| {
                match (a.is_directory, b.is_directory) {
                    (true, false) => core::cmp::Ordering::Less,
                    (false, true) => core::cmp::Ordering::Greater,
                    _ => a.name.cmp(&b.name),
                }
            });

            if let Some(entry) = sorted.get(index) {
                if self.current_path == "/" {
                    format_path("/", &entry.name)
                } else {
                    format_path(&self.current_path, &entry.name)
                }
            } else {
                self.current_path.clone()
            }
        } else {
            self.current_path.clone()
        }
    }

    /// Handle event
    pub fn handle_event(&mut self, event: &WidgetEvent) -> bool {
        if !self.dialog.is_visible() {
            return false;
        }

        // Handle path textbox
        if self.path_textbox.handle_event(event) {
            return true;
        }

        // Handle filename textbox (save mode)
        if self.mode == FilePickerMode::Save {
            if self.filename_textbox.handle_event(event) {
                return true;
            }
        }

        // Handle file list
        if self.file_list.handle_event(event) {
            return true;
        }

        // Handle buttons
        if let WidgetEvent::MouseDown { button: MouseButton::Left, x, y } = event {
            if self.up_button.bounds().contains(*x, *y) {
                self.go_up();
                return true;
            }

            if self.ok_button.bounds().contains(*x, *y) {
                self.selected_path = self.build_path();
                let path = self.selected_path.clone();
                self.dialog.close(DialogResult::Ok);
                if let Some(callback) = self.on_complete {
                    callback(self.dialog.id(), DialogResult::Ok, &path);
                }
                return true;
            }

            if self.cancel_button.bounds().contains(*x, *y) {
                self.dialog.close(DialogResult::Cancel);
                if let Some(callback) = self.on_complete {
                    callback(self.dialog.id(), DialogResult::Cancel, "");
                }
                return true;
            }
        }

        // Handle double-click on list (navigate into directory)
        if let WidgetEvent::DoubleClick { x, y, .. } = event {
            if self.file_list.bounds().contains(*x, *y) {
                if let Some(index) = self.file_list.selected_index() {
                    // Get entry and check if directory
                    let mut sorted: Vec<&FileEntry> = self.entries.iter()
                        .filter(|e| {
                            if self.mode == FilePickerMode::SelectDirectory {
                                e.is_directory
                            } else {
                                true
                            }
                        })
                        .collect();
                    sorted.sort_by(|a, b| {
                        match (a.is_directory, b.is_directory) {
                            (true, false) => core::cmp::Ordering::Less,
                            (false, true) => core::cmp::Ordering::Greater,
                            _ => a.name.cmp(&b.name),
                        }
                    });

                    if let Some(entry) = sorted.get(index) {
                        if entry.is_directory {
                            let new_path = if self.current_path == "/" {
                                format_path("/", &entry.name)
                            } else {
                                format_path(&self.current_path, &entry.name)
                            };
                            self.navigate(&new_path);
                        } else {
                            // Double-click file = select and confirm
                            self.selected_path = self.build_path();
                            let path = self.selected_path.clone();
                            self.dialog.close(DialogResult::Ok);
                            if let Some(callback) = self.on_complete {
                                callback(self.dialog.id(), DialogResult::Ok, &path);
                            }
                        }
                    }
                }
                return true;
            }
        }

        // Handle Enter key
        if let WidgetEvent::KeyDown { key: 0x1C, .. } = event {
            self.selected_path = self.build_path();
            let path = self.selected_path.clone();
            self.dialog.close(DialogResult::Ok);
            if let Some(callback) = self.on_complete {
                callback(self.dialog.id(), DialogResult::Ok, &path);
            }
            return true;
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

        // Draw up button
        self.up_button.render(surface);

        // Draw path textbox
        self.path_textbox.render(surface);

        // Draw file list
        self.file_list.render(surface);

        // Draw filename label and textbox (save mode)
        if self.mode == FilePickerMode::Save {
            let label_x = content.x.max(0) as usize + 10;
            let label_y = (content.y + content.height as isize - 61).max(0) as usize;

            for (i, c) in "Filename:".chars().enumerate() {
                draw_char_simple(surface, label_x + i * 8, label_y, c, theme.fg);
            }

            self.filename_textbox.render(surface);
        }

        // Draw buttons
        self.ok_button.render(surface);
        self.cancel_button.render(surface);
    }
}

fn format_entry(icon: &str, name: &str, size: &str) -> String {
    let mut s = String::from(icon);
    s.push_str(name);
    // Pad to align size
    let name_len = name.chars().count();
    let padding = 35usize.saturating_sub(name_len);
    for _ in 0..padding {
        s.push(' ');
    }
    s.push_str(size);
    s
}

fn format_path(base: &str, name: &str) -> String {
    let mut s = String::from(base);
    if !base.ends_with('/') {
        s.push('/');
    }
    s.push_str(name);
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
