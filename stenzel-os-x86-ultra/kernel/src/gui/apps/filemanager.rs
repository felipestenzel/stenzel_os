//! File Manager
//!
//! A graphical file manager application for browsing and managing files.

use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use crate::gui::widgets::{Widget, WidgetId, WidgetEvent, Bounds, MouseButton, theme};

/// File entry type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    File,
    Directory,
    Symlink,
    Device,
    Unknown,
}

/// A file entry for display
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name: String,
    pub file_type: FileType,
    pub size: u64,
    pub modified: u64, // Unix timestamp
    pub permissions: u16,
    pub selected: bool,
}

impl FileEntry {
    pub fn new(name: &str, file_type: FileType) -> Self {
        Self {
            name: String::from(name),
            file_type,
            size: 0,
            modified: 0,
            permissions: 0o755,
            selected: false,
        }
    }

    pub fn file(name: &str, size: u64) -> Self {
        Self {
            name: String::from(name),
            file_type: FileType::File,
            size,
            modified: 0,
            permissions: 0o644,
            selected: false,
        }
    }

    pub fn directory(name: &str) -> Self {
        Self {
            name: String::from(name),
            file_type: FileType::Directory,
            size: 0,
            modified: 0,
            permissions: 0o755,
            selected: false,
        }
    }

    /// Format size for display
    pub fn format_size(&self) -> String {
        if self.file_type == FileType::Directory {
            return String::from("-");
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

    /// Get icon character for file type
    pub fn icon(&self) -> &'static str {
        match self.file_type {
            FileType::Directory => "[D]",
            FileType::File => "   ",
            FileType::Symlink => "[L]",
            FileType::Device => "[*]",
            FileType::Unknown => "[?]",
        }
    }

    /// Format permissions as string
    pub fn format_permissions(&self) -> String {
        let mut s = String::with_capacity(9);
        let p = self.permissions;

        // Owner
        s.push(if p & 0o400 != 0 { 'r' } else { '-' });
        s.push(if p & 0o200 != 0 { 'w' } else { '-' });
        s.push(if p & 0o100 != 0 { 'x' } else { '-' });

        // Group
        s.push(if p & 0o040 != 0 { 'r' } else { '-' });
        s.push(if p & 0o020 != 0 { 'w' } else { '-' });
        s.push(if p & 0o010 != 0 { 'x' } else { '-' });

        // Others
        s.push(if p & 0o004 != 0 { 'r' } else { '-' });
        s.push(if p & 0o002 != 0 { 'w' } else { '-' });
        s.push(if p & 0o001 != 0 { 'x' } else { '-' });

        s
    }
}

fn format_num(n: u64, suffix: &str) -> String {
    use alloc::string::ToString;
    let mut s = n.to_string();
    s.push_str(suffix);
    s
}

/// Sidebar item type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarItemType {
    Location,
    Separator,
    Device,
}

/// A sidebar item
#[derive(Debug, Clone)]
pub struct SidebarItem {
    pub label: String,
    pub path: String,
    pub item_type: SidebarItemType,
}

impl SidebarItem {
    pub fn location(label: &str, path: &str) -> Self {
        Self {
            label: String::from(label),
            path: String::from(path),
            item_type: SidebarItemType::Location,
        }
    }

    pub fn separator() -> Self {
        Self {
            label: String::new(),
            path: String::new(),
            item_type: SidebarItemType::Separator,
        }
    }

    pub fn device(label: &str, path: &str) -> Self {
        Self {
            label: String::from(label),
            path: String::from(path),
            item_type: SidebarItemType::Device,
        }
    }
}

/// View mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    List,
    Icons,
    Details,
}

/// Sort mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    Name,
    Size,
    Date,
    Type,
}

/// Sort order
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    Ascending,
    Descending,
}

/// Clipboard operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardOp {
    Copy,
    Cut,
}

/// Clipboard entry
#[derive(Debug, Clone)]
pub struct ClipboardEntry {
    pub path: String,
    pub operation: ClipboardOp,
}

/// File operation callback
pub type FileOpCallback = fn(&str, &str, bool); // source, dest, is_cut

/// File manager navigation callback
pub type NavCallback = fn(&str);

/// File manager widget
pub struct FileManager {
    id: WidgetId,
    bounds: Bounds,

    // Current state
    current_path: String,
    entries: Vec<FileEntry>,
    selected_indices: Vec<usize>,

    // History
    history_back: VecDeque<String>,
    history_forward: VecDeque<String>,
    history_limit: usize,

    // View settings
    view_mode: ViewMode,
    sort_mode: SortMode,
    sort_order: SortOrder,
    show_hidden: bool,

    // Sidebar
    sidebar_items: Vec<SidebarItem>,
    sidebar_width: usize,
    sidebar_selected: Option<usize>,

    // Scrolling
    scroll_offset: usize,
    visible_rows: usize,

    // Clipboard
    clipboard: Vec<ClipboardEntry>,

    // UI state
    visible: bool,
    focused: bool,
    hover_index: Option<usize>,
    renaming_index: Option<usize>,
    rename_text: String,

    // Address bar
    address_bar_focused: bool,
    address_bar_text: String,

    // Callbacks
    on_navigate: Option<NavCallback>,
    on_file_open: Option<NavCallback>,
}

impl FileManager {
    const CHAR_WIDTH: usize = 8;
    const CHAR_HEIGHT: usize = 16;
    const ROW_HEIGHT: usize = 20;
    const HEADER_HEIGHT: usize = 32;
    const TOOLBAR_HEIGHT: usize = 28;
    const PADDING: usize = 4;
    const ICON_SIZE: usize = 16;
    const MIN_SIDEBAR_WIDTH: usize = 120;

    /// Create a new file manager
    pub fn new(x: isize, y: isize, width: usize, height: usize) -> Self {
        let sidebar_width = Self::MIN_SIDEBAR_WIDTH + 40;
        let content_height = height.saturating_sub(Self::HEADER_HEIGHT + Self::TOOLBAR_HEIGHT);
        let visible_rows = content_height / Self::ROW_HEIGHT;

        let mut sidebar_items = Vec::new();
        sidebar_items.push(SidebarItem::location("Home", "/home"));
        sidebar_items.push(SidebarItem::location("Root", "/"));
        sidebar_items.push(SidebarItem::location("Documents", "/home/documents"));
        sidebar_items.push(SidebarItem::location("Downloads", "/home/downloads"));
        sidebar_items.push(SidebarItem::separator());
        sidebar_items.push(SidebarItem::device("Disk", "/"));

        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(x, y, width, height),
            current_path: String::from("/"),
            entries: Vec::new(),
            selected_indices: Vec::new(),
            history_back: VecDeque::new(),
            history_forward: VecDeque::new(),
            history_limit: 50,
            view_mode: ViewMode::Details,
            sort_mode: SortMode::Name,
            sort_order: SortOrder::Ascending,
            show_hidden: false,
            sidebar_items,
            sidebar_width,
            sidebar_selected: None,
            scroll_offset: 0,
            visible_rows,
            clipboard: Vec::new(),
            visible: true,
            focused: false,
            hover_index: None,
            renaming_index: None,
            rename_text: String::new(),
            address_bar_focused: false,
            address_bar_text: String::from("/"),
            on_navigate: None,
            on_file_open: None,
        }
    }

    /// Get current path
    pub fn current_path(&self) -> &str {
        &self.current_path
    }

    /// Set navigation callback
    pub fn set_on_navigate(&mut self, callback: NavCallback) {
        self.on_navigate = Some(callback);
    }

    /// Set file open callback
    pub fn set_on_file_open(&mut self, callback: NavCallback) {
        self.on_file_open = Some(callback);
    }

    /// Set entries (called by system to populate file list)
    pub fn set_entries(&mut self, entries: Vec<FileEntry>) {
        self.entries = entries;
        self.selected_indices.clear();
        self.scroll_offset = 0;
        self.sort_entries();
    }

    /// Navigate to a path
    pub fn navigate(&mut self, path: &str) {
        // Add current to history
        if self.history_back.len() >= self.history_limit {
            self.history_back.pop_front();
        }
        self.history_back.push_back(self.current_path.clone());
        self.history_forward.clear();

        self.current_path = String::from(path);
        self.address_bar_text = String::from(path);
        self.selected_indices.clear();
        self.scroll_offset = 0;

        if let Some(callback) = self.on_navigate {
            callback(path);
        }
    }

    /// Go back in history
    pub fn go_back(&mut self) {
        if let Some(path) = self.history_back.pop_back() {
            self.history_forward.push_front(self.current_path.clone());
            self.current_path = path.clone();
            self.address_bar_text = path.clone();
            self.selected_indices.clear();
            self.scroll_offset = 0;

            if let Some(callback) = self.on_navigate {
                callback(&path);
            }
        }
    }

    /// Go forward in history
    pub fn go_forward(&mut self) {
        if let Some(path) = self.history_forward.pop_front() {
            self.history_back.push_back(self.current_path.clone());
            self.current_path = path.clone();
            self.address_bar_text = path.clone();
            self.selected_indices.clear();
            self.scroll_offset = 0;

            if let Some(callback) = self.on_navigate {
                callback(&path);
            }
        }
    }

    /// Go up one directory
    pub fn go_up(&mut self) {
        if self.current_path == "/" {
            return;
        }

        if let Some(pos) = self.current_path.rfind('/') {
            let parent = if pos == 0 {
                String::from("/")
            } else {
                String::from(&self.current_path[..pos])
            };
            self.navigate(&parent);
        }
    }

    /// Refresh current directory
    pub fn refresh(&mut self) {
        if let Some(callback) = self.on_navigate {
            callback(&self.current_path);
        }
    }

    /// Sort entries
    fn sort_entries(&mut self) {
        let sort_order = self.sort_order;
        let sort_mode = self.sort_mode;

        self.entries.sort_by(|a, b| {
            // Directories always first
            let dir_cmp = match (a.file_type == FileType::Directory, b.file_type == FileType::Directory) {
                (true, false) => return core::cmp::Ordering::Less,
                (false, true) => return core::cmp::Ordering::Greater,
                _ => core::cmp::Ordering::Equal,
            };

            let cmp = match sort_mode {
                SortMode::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortMode::Size => a.size.cmp(&b.size),
                SortMode::Date => a.modified.cmp(&b.modified),
                SortMode::Type => {
                    let ext_a = a.name.rsplit('.').next().unwrap_or("");
                    let ext_b = b.name.rsplit('.').next().unwrap_or("");
                    ext_a.to_lowercase().cmp(&ext_b.to_lowercase())
                }
            };

            if sort_order == SortOrder::Descending {
                cmp.reverse()
            } else {
                cmp
            }
        });
    }

    /// Toggle sort mode
    pub fn toggle_sort(&mut self, mode: SortMode) {
        if self.sort_mode == mode {
            self.sort_order = match self.sort_order {
                SortOrder::Ascending => SortOrder::Descending,
                SortOrder::Descending => SortOrder::Ascending,
            };
        } else {
            self.sort_mode = mode;
            self.sort_order = SortOrder::Ascending;
        }
        self.sort_entries();
    }

    /// Toggle hidden files
    pub fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        // Refresh would filter entries
    }

    /// Set view mode
    pub fn set_view_mode(&mut self, mode: ViewMode) {
        self.view_mode = mode;
    }

    /// Select entry at index
    pub fn select(&mut self, index: usize, add_to_selection: bool) {
        if index >= self.entries.len() {
            return;
        }

        if add_to_selection {
            if let Some(pos) = self.selected_indices.iter().position(|&i| i == index) {
                self.selected_indices.remove(pos);
                self.entries[index].selected = false;
            } else {
                self.selected_indices.push(index);
                self.entries[index].selected = true;
            }
        } else {
            // Clear previous selection
            for &idx in &self.selected_indices {
                if idx < self.entries.len() {
                    self.entries[idx].selected = false;
                }
            }
            self.selected_indices.clear();
            self.selected_indices.push(index);
            self.entries[index].selected = true;
        }
    }

    /// Select all
    pub fn select_all(&mut self) {
        self.selected_indices.clear();
        for (i, entry) in self.entries.iter_mut().enumerate() {
            entry.selected = true;
            self.selected_indices.push(i);
        }
    }

    /// Clear selection
    pub fn clear_selection(&mut self) {
        for entry in &mut self.entries {
            entry.selected = false;
        }
        self.selected_indices.clear();
    }

    /// Get selected entries
    pub fn selected_entries(&self) -> Vec<&FileEntry> {
        self.selected_indices
            .iter()
            .filter_map(|&i| self.entries.get(i))
            .collect()
    }

    /// Copy selected to clipboard
    pub fn copy_selected(&mut self) {
        self.clipboard.clear();
        for &idx in &self.selected_indices {
            if let Some(entry) = self.entries.get(idx) {
                let path = build_path(&self.current_path, &entry.name);
                self.clipboard.push(ClipboardEntry {
                    path,
                    operation: ClipboardOp::Copy,
                });
            }
        }
    }

    /// Cut selected to clipboard
    pub fn cut_selected(&mut self) {
        self.clipboard.clear();
        for &idx in &self.selected_indices {
            if let Some(entry) = self.entries.get(idx) {
                let path = build_path(&self.current_path, &entry.name);
                self.clipboard.push(ClipboardEntry {
                    path,
                    operation: ClipboardOp::Cut,
                });
            }
        }
    }

    /// Get clipboard contents
    pub fn clipboard(&self) -> &[ClipboardEntry] {
        &self.clipboard
    }

    /// Clear clipboard
    pub fn clear_clipboard(&mut self) {
        self.clipboard.clear();
    }

    /// Open selected entry
    fn open_selected(&mut self) {
        if self.selected_indices.len() != 1 {
            return;
        }

        let idx = self.selected_indices[0];
        if let Some(entry) = self.entries.get(idx) {
            if entry.file_type == FileType::Directory {
                let new_path = build_path(&self.current_path, &entry.name);
                self.navigate(&new_path);
            } else if let Some(callback) = self.on_file_open {
                let path = build_path(&self.current_path, &entry.name);
                callback(&path);
            }
        }
    }

    /// Start renaming
    pub fn start_rename(&mut self, index: usize) {
        if index < self.entries.len() {
            self.renaming_index = Some(index);
            self.rename_text = self.entries[index].name.clone();
        }
    }

    /// Finish renaming
    pub fn finish_rename(&mut self) -> Option<(String, String)> {
        if let Some(idx) = self.renaming_index {
            let old_name = self.entries[idx].name.clone();
            let new_name = self.rename_text.clone();
            self.renaming_index = None;
            self.rename_text.clear();

            if !new_name.is_empty() && new_name != old_name {
                return Some((
                    build_path(&self.current_path, &old_name),
                    build_path(&self.current_path, &new_name),
                ));
            }
        }
        self.renaming_index = None;
        self.rename_text.clear();
        None
    }

    /// Cancel renaming
    pub fn cancel_rename(&mut self) {
        self.renaming_index = None;
        self.rename_text.clear();
    }

    /// Get entry at point
    fn entry_at_point(&self, x: isize, y: isize) -> Option<usize> {
        let local_x = (x - self.bounds.x) as usize;
        let local_y = (y - self.bounds.y) as usize;

        // Check if in file list area
        let list_x = self.sidebar_width + Self::PADDING;
        let list_y = Self::HEADER_HEIGHT + Self::TOOLBAR_HEIGHT;
        let list_width = self.bounds.width.saturating_sub(self.sidebar_width + Self::PADDING * 2);
        let list_height = self.bounds.height.saturating_sub(list_y);

        if local_x < list_x || local_x >= list_x + list_width {
            return None;
        }
        if local_y < list_y || local_y >= list_y + list_height {
            return None;
        }

        let row = (local_y - list_y) / Self::ROW_HEIGHT;
        let index = self.scroll_offset + row;

        if index < self.entries.len() {
            Some(index)
        } else {
            None
        }
    }

    /// Get sidebar item at point
    fn sidebar_item_at_point(&self, x: isize, y: isize) -> Option<usize> {
        let local_x = (x - self.bounds.x) as usize;
        let local_y = (y - self.bounds.y) as usize;

        if local_x >= self.sidebar_width {
            return None;
        }

        let list_y = Self::HEADER_HEIGHT + Self::TOOLBAR_HEIGHT;
        if local_y < list_y {
            return None;
        }

        let row = (local_y - list_y) / Self::ROW_HEIGHT;
        if row < self.sidebar_items.len() {
            Some(row)
        } else {
            None
        }
    }

    /// Handle keyboard input
    fn handle_key(&mut self, scancode: u8, modifiers: u8) -> bool {
        let ctrl = (modifiers & 0x02) != 0;

        // Handle renaming
        if self.renaming_index.is_some() {
            match scancode {
                0x1C => { // Enter
                    self.finish_rename();
                    return true;
                }
                0x01 => { // Escape
                    self.cancel_rename();
                    return true;
                }
                0x0E => { // Backspace
                    self.rename_text.pop();
                    return true;
                }
                _ => return false,
            }
        }

        // Handle address bar
        if self.address_bar_focused {
            match scancode {
                0x1C => { // Enter
                    let path = self.address_bar_text.clone();
                    self.navigate(&path);
                    self.address_bar_focused = false;
                    return true;
                }
                0x01 => { // Escape
                    self.address_bar_text = self.current_path.clone();
                    self.address_bar_focused = false;
                    return true;
                }
                0x0E => { // Backspace
                    self.address_bar_text.pop();
                    return true;
                }
                _ => return false,
            }
        }

        // File list navigation and commands
        match scancode {
            0x48 => { // Up arrow
                if !self.selected_indices.is_empty() {
                    let min_idx = *self.selected_indices.iter().min().unwrap_or(&0);
                    if min_idx > 0 {
                        self.select(min_idx - 1, ctrl);
                        if min_idx - 1 < self.scroll_offset {
                            self.scroll_offset = min_idx - 1;
                        }
                    }
                } else if !self.entries.is_empty() {
                    self.select(0, false);
                }
                true
            }
            0x50 => { // Down arrow
                if !self.selected_indices.is_empty() {
                    let max_idx = *self.selected_indices.iter().max().unwrap_or(&0);
                    if max_idx + 1 < self.entries.len() {
                        self.select(max_idx + 1, ctrl);
                        if max_idx + 1 >= self.scroll_offset + self.visible_rows {
                            self.scroll_offset = max_idx + 2 - self.visible_rows;
                        }
                    }
                } else if !self.entries.is_empty() {
                    self.select(0, false);
                }
                true
            }
            0x1C => { // Enter
                self.open_selected();
                true
            }
            0x0E | 0x53 => { // Backspace or Delete
                // Delete selected - would trigger callback
                true
            }
            _ if ctrl => {
                match scancode {
                    0x1E => { // Ctrl+A - Select all
                        self.select_all();
                        true
                    }
                    0x2E => { // Ctrl+C - Copy
                        self.copy_selected();
                        true
                    }
                    0x2D => { // Ctrl+X - Cut
                        self.cut_selected();
                        true
                    }
                    0x2F => { // Ctrl+V - Paste
                        // Paste would trigger callback
                        true
                    }
                    _ => false,
                }
            }
            0x3C => { // F2 - Rename
                if self.selected_indices.len() == 1 {
                    self.start_rename(self.selected_indices[0]);
                }
                true
            }
            0x3F => { // F5 - Refresh
                self.refresh();
                true
            }
            _ => false,
        }
    }
}

impl Widget for FileManager {
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

        let content_height = height.saturating_sub(Self::HEADER_HEIGHT + Self::TOOLBAR_HEIGHT);
        self.visible_rows = content_height / Self::ROW_HEIGHT;
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
                if self.renaming_index.is_some() {
                    if *c >= ' ' {
                        self.rename_text.push(*c);
                    }
                    return true;
                }
                if self.address_bar_focused {
                    if *c >= ' ' {
                        self.address_bar_text.push(*c);
                    }
                    return true;
                }
                false
            }
            WidgetEvent::MouseDown { button: MouseButton::Left, x, y } => {
                // Check toolbar buttons
                let local_y = (*y - self.bounds.y) as usize;
                if local_y < Self::HEADER_HEIGHT + Self::TOOLBAR_HEIGHT {
                    // Check navigation buttons area
                    let local_x = (*x - self.bounds.x) as usize;
                    let btn_y = Self::HEADER_HEIGHT;

                    if local_y >= btn_y && local_y < btn_y + Self::TOOLBAR_HEIGHT {
                        if local_x >= 4 && local_x < 28 {
                            self.go_back();
                            return true;
                        } else if local_x >= 32 && local_x < 56 {
                            self.go_forward();
                            return true;
                        } else if local_x >= 60 && local_x < 84 {
                            self.go_up();
                            return true;
                        } else if local_x >= 88 && local_x < 112 {
                            self.refresh();
                            return true;
                        }
                    }

                    // Check address bar
                    if local_y >= 4 && local_y < Self::HEADER_HEIGHT - 4 {
                        if local_x >= 120 && local_x < self.bounds.width - 8 {
                            self.address_bar_focused = true;
                            return true;
                        }
                    }
                    return false;
                }

                // Check sidebar
                if let Some(idx) = self.sidebar_item_at_point(*x, *y) {
                    let item = &self.sidebar_items[idx];
                    if item.item_type != SidebarItemType::Separator {
                        self.sidebar_selected = Some(idx);
                        let path = item.path.clone();
                        self.navigate(&path);
                    }
                    return true;
                }

                // Check file list
                if let Some(idx) = self.entry_at_point(*x, *y) {
                    let ctrl = false; // Would get from modifiers
                    self.select(idx, ctrl);
                    return true;
                }

                false
            }
            WidgetEvent::DoubleClick { x, y, .. } => {
                if let Some(idx) = self.entry_at_point(*x, *y) {
                    self.select(idx, false);
                    self.open_selected();
                    return true;
                }
                false
            }
            WidgetEvent::MouseMove { x, y } => {
                self.hover_index = self.entry_at_point(*x, *y);
                true
            }
            WidgetEvent::Scroll { delta_y, .. } => {
                if *delta_y < 0 {
                    self.scroll_offset = self.scroll_offset.saturating_add(3);
                    let max_scroll = self.entries.len().saturating_sub(self.visible_rows);
                    self.scroll_offset = self.scroll_offset.min(max_scroll);
                } else {
                    self.scroll_offset = self.scroll_offset.saturating_sub(3);
                }
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

        // Background
        for py in 0..h {
            for px in 0..w {
                surface.set_pixel(x + px, y + py, theme.bg);
            }
        }

        // Header (address bar area)
        let header_color = Color::new(45, 45, 48);
        for py in 0..Self::HEADER_HEIGHT {
            for px in 0..w {
                surface.set_pixel(x + px, y + py, header_color);
            }
        }

        // Address bar background
        let addr_x = x + 120;
        let addr_y = y + 6;
        let addr_w = w.saturating_sub(128);
        let addr_h = Self::HEADER_HEIGHT - 12;
        let addr_bg = if self.address_bar_focused {
            Color::new(255, 255, 255)
        } else {
            Color::new(60, 60, 63)
        };
        let addr_fg = if self.address_bar_focused {
            Color::new(0, 0, 0)
        } else {
            theme.fg
        };

        for py in 0..addr_h {
            for px in 0..addr_w {
                surface.set_pixel(addr_x + px, addr_y + py, addr_bg);
            }
        }

        // Address bar text
        let display_text = if self.address_bar_focused {
            &self.address_bar_text
        } else {
            &self.current_path
        };
        for (i, c) in display_text.chars().take(50).enumerate() {
            draw_char_simple(surface, addr_x + 4 + i * Self::CHAR_WIDTH, addr_y + 2, c, addr_fg);
        }

        // Toolbar
        let toolbar_y = y + Self::HEADER_HEIGHT;
        let toolbar_color = Color::new(50, 50, 53);
        for py in 0..Self::TOOLBAR_HEIGHT {
            for px in 0..w {
                surface.set_pixel(x + px, toolbar_y + py, toolbar_color);
            }
        }

        // Toolbar buttons: Back, Forward, Up, Refresh
        draw_toolbar_button(surface, x + 4, toolbar_y + 4, "<", !self.history_back.is_empty());
        draw_toolbar_button(surface, x + 32, toolbar_y + 4, ">", !self.history_forward.is_empty());
        draw_toolbar_button(surface, x + 60, toolbar_y + 4, "^", self.current_path != "/");
        draw_toolbar_button(surface, x + 88, toolbar_y + 4, "R", true);

        // Sidebar
        let list_y = y + Self::HEADER_HEIGHT + Self::TOOLBAR_HEIGHT;
        let sidebar_bg = Color::new(37, 37, 38);
        for py in 0..(h - Self::HEADER_HEIGHT - Self::TOOLBAR_HEIGHT) {
            for px in 0..self.sidebar_width {
                surface.set_pixel(x + px, list_y + py, sidebar_bg);
            }
        }

        // Sidebar items
        for (i, item) in self.sidebar_items.iter().enumerate() {
            let item_y = list_y + i * Self::ROW_HEIGHT;
            if item_y + Self::ROW_HEIGHT > y + h {
                break;
            }

            match item.item_type {
                SidebarItemType::Separator => {
                    // Draw separator line
                    let sep_y = item_y + Self::ROW_HEIGHT / 2;
                    for px in 4..(self.sidebar_width - 4) {
                        surface.set_pixel(x + px, sep_y, Color::new(60, 60, 63));
                    }
                }
                _ => {
                    // Highlight selected/hovered
                    let is_selected = self.sidebar_selected == Some(i);
                    if is_selected {
                        for py in 0..Self::ROW_HEIGHT {
                            for px in 0..self.sidebar_width {
                                surface.set_pixel(x + px, item_y + py, theme.accent);
                            }
                        }
                    }

                    // Icon
                    let icon = if item.item_type == SidebarItemType::Device { "[D]" } else { "[ ]" };
                    for (j, c) in icon.chars().enumerate() {
                        draw_char_simple(surface, x + 4 + j * Self::CHAR_WIDTH, item_y + 2, c, theme.fg);
                    }

                    // Label
                    for (j, c) in item.label.chars().take(15).enumerate() {
                        draw_char_simple(surface, x + 32 + j * Self::CHAR_WIDTH, item_y + 2, c, theme.fg);
                    }
                }
            }
        }

        // File list background
        let list_x = x + self.sidebar_width;
        let list_w = w.saturating_sub(self.sidebar_width);
        let list_h = h.saturating_sub(Self::HEADER_HEIGHT + Self::TOOLBAR_HEIGHT);

        // Column headers for details view
        if self.view_mode == ViewMode::Details {
            let header_y = list_y;
            let header_bg = Color::new(45, 45, 48);
            for py in 0..Self::ROW_HEIGHT {
                for px in 0..list_w {
                    surface.set_pixel(list_x + px, header_y + py, header_bg);
                }
            }

            // Name, Size, Modified headers
            draw_string(surface, list_x + 4, header_y + 2, "Name", theme.fg);
            draw_string(surface, list_x + list_w - 160, header_y + 2, "Size", theme.fg);
            draw_string(surface, list_x + list_w - 80, header_y + 2, "Type", theme.fg);
        }

        // File entries
        let entries_y = if self.view_mode == ViewMode::Details {
            list_y + Self::ROW_HEIGHT
        } else {
            list_y
        };

        let visible_entries: Vec<_> = self.entries.iter()
            .skip(self.scroll_offset)
            .take(self.visible_rows)
            .enumerate()
            .collect();

        for (row_idx, entry) in visible_entries {
            let entry_y = entries_y + row_idx * Self::ROW_HEIGHT;
            if entry_y + Self::ROW_HEIGHT > y + h {
                break;
            }

            let actual_idx = self.scroll_offset + row_idx;

            // Background for selected/hovered
            let bg = if entry.selected {
                theme.accent
            } else if self.hover_index == Some(actual_idx) {
                Color::new(50, 50, 53)
            } else {
                theme.bg
            };

            for py in 0..Self::ROW_HEIGHT {
                for px in 0..list_w {
                    surface.set_pixel(list_x + px, entry_y + py, bg);
                }
            }

            // Icon
            let icon = entry.icon();
            for (i, c) in icon.chars().enumerate() {
                draw_char_simple(surface, list_x + 4 + i * Self::CHAR_WIDTH, entry_y + 2, c, theme.fg);
            }

            // Name
            let name_x = list_x + 32;
            if self.renaming_index == Some(actual_idx) {
                // Editing
                let edit_bg = Color::new(255, 255, 255);
                let edit_w = 200;
                for py in 0..Self::ROW_HEIGHT - 4 {
                    for px in 0..edit_w {
                        surface.set_pixel(name_x + px, entry_y + 2 + py, edit_bg);
                    }
                }
                for (i, c) in self.rename_text.chars().take(24).enumerate() {
                    draw_char_simple(surface, name_x + 2 + i * Self::CHAR_WIDTH, entry_y + 2, c, Color::new(0, 0, 0));
                }
            } else {
                for (i, c) in entry.name.chars().take(30).enumerate() {
                    draw_char_simple(surface, name_x + i * Self::CHAR_WIDTH, entry_y + 2, c, theme.fg);
                }
            }

            // Size (details view)
            if self.view_mode == ViewMode::Details {
                let size_str = entry.format_size();
                let size_x = list_x + list_w - 160;
                for (i, c) in size_str.chars().enumerate() {
                    draw_char_simple(surface, size_x + i * Self::CHAR_WIDTH, entry_y + 2, c, theme.fg);
                }

                // Type
                let type_str = match entry.file_type {
                    FileType::Directory => "Folder",
                    FileType::File => "File",
                    FileType::Symlink => "Link",
                    FileType::Device => "Device",
                    FileType::Unknown => "Unknown",
                };
                let type_x = list_x + list_w - 80;
                for (i, c) in type_str.chars().enumerate() {
                    draw_char_simple(surface, type_x + i * Self::CHAR_WIDTH, entry_y + 2, c, theme.fg);
                }
            }
        }

        // Scrollbar (if needed)
        if self.entries.len() > self.visible_rows {
            let scrollbar_x = x + w - 12;
            let scrollbar_y = entries_y;
            let scrollbar_h = list_h;
            let scrollbar_track = Color::new(50, 50, 53);

            for py in 0..scrollbar_h {
                for px in 0..8 {
                    surface.set_pixel(scrollbar_x + px, scrollbar_y + py, scrollbar_track);
                }
            }

            // Thumb
            let total = self.entries.len() as f32;
            let visible = self.visible_rows as f32;
            let thumb_h = ((visible / total) * scrollbar_h as f32).max(20.0) as usize;
            let thumb_pos = ((self.scroll_offset as f32 / total) * scrollbar_h as f32) as usize;
            let thumb_color = Color::new(100, 100, 100);

            for py in 0..thumb_h {
                for px in 0..8 {
                    let ty = scrollbar_y + thumb_pos + py;
                    if ty < scrollbar_y + scrollbar_h {
                        surface.set_pixel(scrollbar_x + px, ty, thumb_color);
                    }
                }
            }
        }

        // Status bar
        let status_y = y + h - 20;
        let status_bg = Color::new(0, 122, 204);
        for py in 0..20 {
            for px in 0..w {
                surface.set_pixel(x + px, status_y + py, status_bg);
            }
        }

        // Status text
        let status_text = format_status(self.entries.len(), self.selected_indices.len());
        for (i, c) in status_text.chars().enumerate() {
            draw_char_simple(surface, x + 8 + i * Self::CHAR_WIDTH, status_y + 2, c, Color::new(255, 255, 255));
        }
    }
}

fn build_path(base: &str, name: &str) -> String {
    let mut s = String::from(base);
    if !base.ends_with('/') {
        s.push('/');
    }
    s.push_str(name);
    s
}

fn format_status(total: usize, selected: usize) -> String {
    use alloc::string::ToString;
    let mut s = total.to_string();
    s.push_str(" items");
    if selected > 0 {
        s.push_str(", ");
        s.push_str(&selected.to_string());
        s.push_str(" selected");
    }
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

fn draw_string(surface: &mut Surface, x: usize, y: usize, s: &str, color: Color) {
    for (i, c) in s.chars().enumerate() {
        draw_char_simple(surface, x + i * 8, y, c, color);
    }
}

fn draw_toolbar_button(surface: &mut Surface, x: usize, y: usize, label: &str, enabled: bool) {
    let bg = if enabled {
        Color::new(70, 70, 73)
    } else {
        Color::new(50, 50, 53)
    };
    let fg = if enabled {
        Color::new(255, 255, 255)
    } else {
        Color::new(100, 100, 100)
    };

    // Button background
    for py in 0..20 {
        for px in 0..24 {
            surface.set_pixel(x + px, y + py, bg);
        }
    }

    // Label centered
    let label_x = x + (24 - label.len() * 8) / 2;
    let label_y = y + 2;
    for (i, c) in label.chars().enumerate() {
        draw_char_simple(surface, label_x + i * 8, label_y, c, fg);
    }
}
