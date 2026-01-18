//! Trash / Recycle Bin
//!
//! System trash management for deleted files with restore capability.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use spin::Mutex;

use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use crate::gui::widgets::{Widget, WidgetId, WidgetEvent, Bounds, MouseButton, theme};

/// Global trash state
static TRASH: Mutex<Option<TrashState>> = Mutex::new(None);

/// Trash state
struct TrashState {
    /// Trashed items
    items: Vec<TrashedItem>,
    /// Trash directory path
    trash_path: String,
    /// Total size of trash (bytes)
    total_size: u64,
    /// Max trash size (bytes)
    max_size: u64,
}

/// A trashed item
#[derive(Debug, Clone)]
pub struct TrashedItem {
    /// Original file path
    pub original_path: String,
    /// Trash file name (unique ID)
    pub trash_name: String,
    /// Display name (original file name)
    pub display_name: String,
    /// File type
    pub file_type: TrashFileType,
    /// Size in bytes
    pub size: u64,
    /// Deletion time (unix timestamp)
    pub deleted_at: u64,
    /// Is directory
    pub is_directory: bool,
}

/// Trash file type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrashFileType {
    File,
    Directory,
    Symlink,
    Unknown,
}

impl TrashFileType {
    pub fn icon(&self) -> &'static str {
        match self {
            TrashFileType::File => "   ",
            TrashFileType::Directory => "[D]",
            TrashFileType::Symlink => "[L]",
            TrashFileType::Unknown => "[?]",
        }
    }
}

/// Initialize trash system
pub fn init() {
    let mut trash = TRASH.lock();
    if trash.is_some() {
        return;
    }

    *trash = Some(TrashState {
        items: Vec::new(),
        trash_path: "/.trash".to_string(),
        total_size: 0,
        max_size: 1024 * 1024 * 1024, // 1GB default
    });

    crate::kprintln!("trash: initialized");
}

/// Move file to trash
pub fn trash_file(path: &str) -> Result<(), TrashError> {
    let mut trash = TRASH.lock();
    let trash = trash.as_mut().ok_or(TrashError::NotInitialized)?;

    // Extract file name
    let display_name = path.rsplit('/').next().unwrap_or(path).to_string();

    // Generate unique trash name
    let trash_name = format!("{}-{}", trash.items.len(), display_name);

    // Would need actual file system operations here
    // For now, create the metadata entry
    let item = TrashedItem {
        original_path: path.to_string(),
        trash_name,
        display_name,
        file_type: TrashFileType::File,
        size: 0,
        deleted_at: get_timestamp(),
        is_directory: false,
    };

    trash.items.push(item);

    // TODO: Actually move file to trash directory
    // fs::rename(path, &format!("{}/{}", trash.trash_path, trash_name))?;

    Ok(())
}

/// Move directory to trash
pub fn trash_directory(path: &str) -> Result<(), TrashError> {
    let mut trash = TRASH.lock();
    let trash = trash.as_mut().ok_or(TrashError::NotInitialized)?;

    let display_name = path.rsplit('/').next().unwrap_or(path).to_string();
    let trash_name = format!("{}-{}", trash.items.len(), display_name);

    let item = TrashedItem {
        original_path: path.to_string(),
        trash_name,
        display_name,
        file_type: TrashFileType::Directory,
        size: 0,
        deleted_at: get_timestamp(),
        is_directory: true,
    };

    trash.items.push(item);

    Ok(())
}

/// Restore item from trash
pub fn restore(trash_name: &str) -> Result<String, TrashError> {
    let mut trash = TRASH.lock();
    let trash = trash.as_mut().ok_or(TrashError::NotInitialized)?;

    let idx = trash.items.iter()
        .position(|item| item.trash_name == trash_name)
        .ok_or(TrashError::ItemNotFound)?;

    let item = trash.items.remove(idx);
    let original_path = item.original_path.clone();

    // TODO: Actually move file back from trash directory
    // fs::rename(&format!("{}/{}", trash.trash_path, item.trash_name), &item.original_path)?;

    trash.total_size -= item.size;

    Ok(original_path)
}

/// Delete item permanently
pub fn delete_permanently(trash_name: &str) -> Result<(), TrashError> {
    let mut trash = TRASH.lock();
    let trash = trash.as_mut().ok_or(TrashError::NotInitialized)?;

    let idx = trash.items.iter()
        .position(|item| item.trash_name == trash_name)
        .ok_or(TrashError::ItemNotFound)?;

    let item = trash.items.remove(idx);

    // TODO: Actually delete file from trash directory
    // fs::remove(&format!("{}/{}", trash.trash_path, item.trash_name))?;

    trash.total_size -= item.size;

    Ok(())
}

/// Empty trash (delete all items permanently)
pub fn empty_trash() -> Result<usize, TrashError> {
    let mut trash = TRASH.lock();
    let trash = trash.as_mut().ok_or(TrashError::NotInitialized)?;

    let count = trash.items.len();

    // TODO: Delete all files from trash directory

    trash.items.clear();
    trash.total_size = 0;

    Ok(count)
}

/// Get all trashed items
pub fn get_items() -> Vec<TrashedItem> {
    let trash = TRASH.lock();
    trash.as_ref().map(|t| t.items.clone()).unwrap_or_default()
}

/// Get trash size
pub fn get_size() -> u64 {
    let trash = TRASH.lock();
    trash.as_ref().map(|t| t.total_size).unwrap_or(0)
}

/// Get item count
pub fn get_count() -> usize {
    let trash = TRASH.lock();
    trash.as_ref().map(|t| t.items.len()).unwrap_or(0)
}

/// Get timestamp (placeholder)
fn get_timestamp() -> u64 {
    // Would get actual time from time subsystem
    0
}

/// Trash error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrashError {
    NotInitialized,
    ItemNotFound,
    RestoreFailed,
    DeleteFailed,
    TrashFull,
}

/// Trash viewer widget
pub struct TrashViewer {
    id: WidgetId,
    bounds: Bounds,
    items: Vec<TrashedItem>,
    selected_indices: Vec<usize>,
    scroll_offset: usize,
    visible_rows: usize,
    visible: bool,
    focused: bool,
    hover_index: Option<usize>,
}

impl TrashViewer {
    const CHAR_WIDTH: usize = 8;
    const ROW_HEIGHT: usize = 20;
    const HEADER_HEIGHT: usize = 40;
    const TOOLBAR_HEIGHT: usize = 32;

    pub fn new(x: isize, y: isize, width: usize, height: usize) -> Self {
        let content_height = height.saturating_sub(Self::HEADER_HEIGHT + Self::TOOLBAR_HEIGHT);
        let visible_rows = content_height / Self::ROW_HEIGHT;

        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(x, y, width, height),
            items: Vec::new(),
            selected_indices: Vec::new(),
            scroll_offset: 0,
            visible_rows,
            visible: true,
            focused: false,
            hover_index: None,
        }
    }

    /// Refresh items from trash
    pub fn refresh(&mut self) {
        self.items = get_items();
        self.selected_indices.clear();
        self.scroll_offset = 0;
    }

    /// Restore selected items
    pub fn restore_selected(&mut self) -> Vec<String> {
        let mut restored = Vec::new();
        let names: Vec<_> = self.selected_indices.iter()
            .filter_map(|&i| self.items.get(i).map(|item| item.trash_name.clone()))
            .collect();

        for name in names {
            if let Ok(path) = restore(&name) {
                restored.push(path);
            }
        }

        self.refresh();
        restored
    }

    /// Delete selected items permanently
    pub fn delete_selected(&mut self) -> usize {
        let names: Vec<_> = self.selected_indices.iter()
            .filter_map(|&i| self.items.get(i).map(|item| item.trash_name.clone()))
            .collect();

        let mut deleted = 0;
        for name in names {
            if delete_permanently(&name).is_ok() {
                deleted += 1;
            }
        }

        self.refresh();
        deleted
    }

    /// Empty trash
    pub fn empty(&mut self) -> usize {
        match empty_trash() {
            Ok(count) => {
                self.refresh();
                count
            }
            Err(_) => 0,
        }
    }

    /// Select item at index
    pub fn select(&mut self, index: usize, add_to_selection: bool) {
        if index >= self.items.len() {
            return;
        }

        if add_to_selection {
            if let Some(pos) = self.selected_indices.iter().position(|&i| i == index) {
                self.selected_indices.remove(pos);
            } else {
                self.selected_indices.push(index);
            }
        } else {
            self.selected_indices.clear();
            self.selected_indices.push(index);
        }
    }

    /// Clear selection
    pub fn clear_selection(&mut self) {
        self.selected_indices.clear();
    }

    /// Get entry at point
    fn entry_at_point(&self, x: isize, y: isize) -> Option<usize> {
        let local_y = (y - self.bounds.y) as usize;

        let list_y = Self::HEADER_HEIGHT + Self::TOOLBAR_HEIGHT;
        if local_y < list_y {
            return None;
        }

        let row = (local_y - list_y) / Self::ROW_HEIGHT;
        let index = self.scroll_offset + row;

        if index < self.items.len() {
            Some(index)
        } else {
            None
        }
    }

    fn format_size(size: u64) -> String {
        if size < 1024 {
            format!("{} B", size)
        } else if size < 1024 * 1024 {
            format!("{} KB", size / 1024)
        } else if size < 1024 * 1024 * 1024 {
            format!("{} MB", size / (1024 * 1024))
        } else {
            format!("{} GB", size / (1024 * 1024 * 1024))
        }
    }
}

impl Widget for TrashViewer {
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

    fn is_enabled(&self) -> bool { true }
    fn set_enabled(&mut self, _enabled: bool) {}
    fn is_visible(&self) -> bool { self.visible }
    fn set_visible(&mut self, visible: bool) { self.visible = visible; }

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
            WidgetEvent::MouseDown { button: MouseButton::Left, x, y } => {
                if let Some(idx) = self.entry_at_point(*x, *y) {
                    self.select(idx, false);
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
                    let max_scroll = self.items.len().saturating_sub(self.visible_rows);
                    self.scroll_offset = self.scroll_offset.min(max_scroll);
                } else {
                    self.scroll_offset = self.scroll_offset.saturating_sub(3);
                }
                true
            }
            WidgetEvent::KeyDown { key, .. } => {
                match *key {
                    0x53 => { // Delete - permanent delete
                        self.delete_selected();
                        true
                    }
                    0x13 => { // R - restore
                        self.restore_selected();
                        true
                    }
                    _ => false,
                }
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

        // Header
        let header_bg = Color::new(45, 45, 48);
        for py in 0..Self::HEADER_HEIGHT {
            for px in 0..w {
                surface.set_pixel(x + px, y + py, header_bg);
            }
        }

        // Title
        let title = "Trash";
        for (i, c) in title.chars().enumerate() {
            draw_char(surface, x + 8 + i * Self::CHAR_WIDTH, y + 12, c, theme.fg);
        }

        // Item count and size
        let info = format!("{} items, {} total", self.items.len(), Self::format_size(get_size()));
        for (i, c) in info.chars().enumerate() {
            draw_char(surface, x + w - 200 + i * Self::CHAR_WIDTH, y + 12, c, Color::new(150, 150, 150));
        }

        // Toolbar
        let toolbar_y = y + Self::HEADER_HEIGHT;
        let toolbar_bg = Color::new(50, 50, 53);
        for py in 0..Self::TOOLBAR_HEIGHT {
            for px in 0..w {
                surface.set_pixel(x + px, toolbar_y + py, toolbar_bg);
            }
        }

        // Toolbar buttons
        draw_button(surface, x + 8, toolbar_y + 4, "Restore", !self.selected_indices.is_empty());
        draw_button(surface, x + 80, toolbar_y + 4, "Delete", !self.selected_indices.is_empty());
        draw_button(surface, x + 152, toolbar_y + 4, "Empty Trash", !self.items.is_empty());

        // Column headers
        let list_y = y + Self::HEADER_HEIGHT + Self::TOOLBAR_HEIGHT;
        let header_row_bg = Color::new(40, 40, 43);
        for py in 0..Self::ROW_HEIGHT {
            for px in 0..w {
                surface.set_pixel(x + px, list_y + py, header_row_bg);
            }
        }

        draw_string(surface, x + 8, list_y + 2, "Name", theme.fg);
        draw_string(surface, x + w / 2, list_y + 2, "Original Location", theme.fg);
        draw_string(surface, x + w - 100, list_y + 2, "Size", theme.fg);

        // Items
        let items_y = list_y + Self::ROW_HEIGHT;
        for (row_idx, item) in self.items.iter().skip(self.scroll_offset).take(self.visible_rows).enumerate() {
            let item_y = items_y + row_idx * Self::ROW_HEIGHT;
            let actual_idx = self.scroll_offset + row_idx;

            // Background
            let bg = if self.selected_indices.contains(&actual_idx) {
                theme.accent
            } else if self.hover_index == Some(actual_idx) {
                Color::new(50, 50, 53)
            } else {
                theme.bg
            };

            for py in 0..Self::ROW_HEIGHT {
                for px in 0..w {
                    surface.set_pixel(x + px, item_y + py, bg);
                }
            }

            // Icon and name
            let icon = item.file_type.icon();
            for (i, c) in icon.chars().enumerate() {
                draw_char(surface, x + 8 + i * Self::CHAR_WIDTH, item_y + 2, c, theme.fg);
            }
            for (i, c) in item.display_name.chars().take(30).enumerate() {
                draw_char(surface, x + 40 + i * Self::CHAR_WIDTH, item_y + 2, c, theme.fg);
            }

            // Original location
            for (i, c) in item.original_path.chars().take(40).enumerate() {
                draw_char(surface, x + w / 2 + i * Self::CHAR_WIDTH, item_y + 2, c, Color::new(150, 150, 150));
            }

            // Size
            let size_str = Self::format_size(item.size);
            for (i, c) in size_str.chars().enumerate() {
                draw_char(surface, x + w - 100 + i * Self::CHAR_WIDTH, item_y + 2, c, theme.fg);
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

fn draw_string(surface: &mut Surface, x: usize, y: usize, s: &str, color: Color) {
    for (i, c) in s.chars().enumerate() {
        draw_char(surface, x + i * 8, y, c, color);
    }
}

fn draw_button(surface: &mut Surface, x: usize, y: usize, label: &str, enabled: bool) {
    let bg = if enabled { Color::new(70, 70, 73) } else { Color::new(50, 50, 53) };
    let fg = if enabled { Color::new(255, 255, 255) } else { Color::new(100, 100, 100) };
    let width = label.len() * 8 + 16;

    for py in 0..24 {
        for px in 0..width {
            surface.set_pixel(x + px, y + py, bg);
        }
    }

    for (i, c) in label.chars().enumerate() {
        draw_char(surface, x + 8 + i * 8, y + 4, c, fg);
    }
}
