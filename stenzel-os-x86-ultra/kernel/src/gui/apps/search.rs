//! File Search
//!
//! System-wide file search with filtering and indexing.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use spin::Mutex;

use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use crate::gui::widgets::{Widget, WidgetId, WidgetEvent, Bounds, MouseButton, theme};

/// Global search state
static SEARCH_STATE: Mutex<Option<SearchState>> = Mutex::new(None);

/// Search state
struct SearchState {
    /// Search index
    index: Vec<IndexedFile>,
    /// Index is being built
    indexing: bool,
    /// Last index time
    last_indexed: u64,
    /// Indexed directories
    indexed_dirs: Vec<String>,
}

/// Indexed file entry
#[derive(Debug, Clone)]
pub struct IndexedFile {
    /// File path
    pub path: String,
    /// File name (for fast matching)
    pub name: String,
    /// File type
    pub file_type: SearchFileType,
    /// Size in bytes
    pub size: u64,
    /// Modified time
    pub modified: u64,
    /// Content preview (first few bytes for text files)
    pub preview: Option<String>,
}

/// File type for search
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchFileType {
    File,
    Directory,
    Image,
    Video,
    Audio,
    Document,
    Archive,
    Code,
    Unknown,
}

impl SearchFileType {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "svg" => SearchFileType::Image,
            "mp4" | "mkv" | "avi" | "webm" | "mov" | "wmv" => SearchFileType::Video,
            "mp3" | "flac" | "ogg" | "wav" | "aac" | "m4a" => SearchFileType::Audio,
            "pdf" | "doc" | "docx" | "odt" | "txt" | "md" | "rtf" => SearchFileType::Document,
            "zip" | "tar" | "gz" | "7z" | "rar" | "xz" => SearchFileType::Archive,
            "rs" | "c" | "cpp" | "h" | "py" | "js" | "ts" | "go" | "java" | "rb" => SearchFileType::Code,
            _ => SearchFileType::File,
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            SearchFileType::File => "   ",
            SearchFileType::Directory => "[D]",
            SearchFileType::Image => "[I]",
            SearchFileType::Video => "[V]",
            SearchFileType::Audio => "[A]",
            SearchFileType::Document => "[T]",
            SearchFileType::Archive => "[Z]",
            SearchFileType::Code => "[C]",
            SearchFileType::Unknown => "[?]",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            SearchFileType::File => "File",
            SearchFileType::Directory => "Folder",
            SearchFileType::Image => "Image",
            SearchFileType::Video => "Video",
            SearchFileType::Audio => "Audio",
            SearchFileType::Document => "Document",
            SearchFileType::Archive => "Archive",
            SearchFileType::Code => "Code",
            SearchFileType::Unknown => "Unknown",
        }
    }
}

/// Search filter
#[derive(Debug, Clone)]
pub struct SearchFilter {
    /// File type filter
    pub file_type: Option<SearchFileType>,
    /// Minimum size (bytes)
    pub min_size: Option<u64>,
    /// Maximum size (bytes)
    pub max_size: Option<u64>,
    /// Modified after (timestamp)
    pub modified_after: Option<u64>,
    /// Modified before (timestamp)
    pub modified_before: Option<u64>,
    /// Search in path
    pub search_path: Option<String>,
    /// Include hidden files
    pub include_hidden: bool,
    /// Case sensitive
    pub case_sensitive: bool,
}

impl Default for SearchFilter {
    fn default() -> Self {
        Self {
            file_type: None,
            min_size: None,
            max_size: None,
            modified_after: None,
            modified_before: None,
            search_path: None,
            include_hidden: false,
            case_sensitive: false,
        }
    }
}

/// Search result
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Matched file
    pub file: IndexedFile,
    /// Match score (higher is better)
    pub score: u32,
    /// Match highlights (start, end positions)
    pub highlights: Vec<(usize, usize)>,
}

/// Initialize search system
pub fn init() {
    let mut state = SEARCH_STATE.lock();
    if state.is_some() {
        return;
    }

    *state = Some(SearchState {
        index: Vec::new(),
        indexing: false,
        last_indexed: 0,
        indexed_dirs: vec![
            "/".to_string(),
            "/home".to_string(),
        ],
    });

    crate::kprintln!("search: initialized");
}

/// Add file to index
pub fn index_file(file: IndexedFile) {
    let mut state = SEARCH_STATE.lock();
    if let Some(ref mut s) = *state {
        s.index.push(file);
    }
}

/// Rebuild index for directory
pub fn rebuild_index(path: &str) {
    let mut state = SEARCH_STATE.lock();
    if let Some(ref mut s) = *state {
        s.indexing = true;
        // Would trigger actual filesystem scan here
        // For now just mark as indexing
    }
}

/// Search files
pub fn search(query: &str, filter: &SearchFilter) -> Vec<SearchResult> {
    let state = SEARCH_STATE.lock();
    let state = match state.as_ref() {
        Some(s) => s,
        None => return Vec::new(),
    };

    let query_lower = query.to_lowercase();
    let mut results = Vec::new();

    for file in &state.index {
        // Apply filters
        if let Some(file_type) = filter.file_type {
            if file.file_type != file_type {
                continue;
            }
        }

        if let Some(min_size) = filter.min_size {
            if file.size < min_size {
                continue;
            }
        }

        if let Some(max_size) = filter.max_size {
            if file.size > max_size {
                continue;
            }
        }

        if let Some(ref search_path) = filter.search_path {
            if !file.path.starts_with(search_path) {
                continue;
            }
        }

        if !filter.include_hidden && file.name.starts_with('.') {
            continue;
        }

        // Match query
        let name_to_match = if filter.case_sensitive {
            file.name.clone()
        } else {
            file.name.to_lowercase()
        };

        let query_to_match = if filter.case_sensitive {
            query.to_string()
        } else {
            query_lower.clone()
        };

        if let Some(pos) = name_to_match.find(&query_to_match) {
            // Calculate score based on match quality
            let mut score = 100u32;

            // Exact match bonus
            if name_to_match == query_to_match {
                score += 100;
            }

            // Prefix match bonus
            if pos == 0 {
                score += 50;
            }

            // Word boundary bonus
            if pos > 0 {
                let prev_char = name_to_match.chars().nth(pos - 1);
                if prev_char == Some('_') || prev_char == Some('-') || prev_char == Some('.') {
                    score += 25;
                }
            }

            let highlights = vec![(pos, pos + query_to_match.len())];

            results.push(SearchResult {
                file: file.clone(),
                score,
                highlights,
            });
        }
    }

    // Sort by score (descending)
    results.sort_by(|a, b| b.score.cmp(&a.score));

    results
}

/// Quick search (file name only, no content)
pub fn quick_search(query: &str) -> Vec<SearchResult> {
    search(query, &SearchFilter::default())
}

/// Get index size
pub fn get_index_size() -> usize {
    let state = SEARCH_STATE.lock();
    state.as_ref().map(|s| s.index.len()).unwrap_or(0)
}

/// Is indexing in progress
pub fn is_indexing() -> bool {
    let state = SEARCH_STATE.lock();
    state.as_ref().map(|s| s.indexing).unwrap_or(false)
}

/// Search widget
pub struct SearchWidget {
    id: WidgetId,
    bounds: Bounds,
    query: String,
    results: Vec<SearchResult>,
    filter: SearchFilter,
    selected_index: Option<usize>,
    scroll_offset: usize,
    visible_rows: usize,
    visible: bool,
    focused: bool,
    search_bar_focused: bool,
    hover_index: Option<usize>,
    on_open: Option<fn(&str)>,
}

impl SearchWidget {
    const CHAR_WIDTH: usize = 8;
    const ROW_HEIGHT: usize = 24;
    const SEARCH_BAR_HEIGHT: usize = 40;
    const FILTER_BAR_HEIGHT: usize = 32;

    pub fn new(x: isize, y: isize, width: usize, height: usize) -> Self {
        let content_height = height.saturating_sub(Self::SEARCH_BAR_HEIGHT + Self::FILTER_BAR_HEIGHT);
        let visible_rows = content_height / Self::ROW_HEIGHT;

        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(x, y, width, height),
            query: String::new(),
            results: Vec::new(),
            filter: SearchFilter::default(),
            selected_index: None,
            scroll_offset: 0,
            visible_rows,
            visible: true,
            focused: false,
            search_bar_focused: true,
            hover_index: None,
            on_open: None,
        }
    }

    /// Set open callback
    pub fn set_on_open(&mut self, callback: fn(&str)) {
        self.on_open = Some(callback);
    }

    /// Execute search
    pub fn do_search(&mut self) {
        self.results = search(&self.query, &self.filter);
        self.selected_index = if self.results.is_empty() { None } else { Some(0) };
        self.scroll_offset = 0;
    }

    /// Set file type filter
    pub fn set_type_filter(&mut self, file_type: Option<SearchFileType>) {
        self.filter.file_type = file_type;
        self.do_search();
    }

    /// Set search path
    pub fn set_search_path(&mut self, path: Option<String>) {
        self.filter.search_path = path;
        self.do_search();
    }

    /// Toggle hidden files
    pub fn toggle_hidden(&mut self) {
        self.filter.include_hidden = !self.filter.include_hidden;
        self.do_search();
    }

    /// Open selected result
    fn open_selected(&mut self) {
        if let Some(idx) = self.selected_index {
            if let Some(result) = self.results.get(idx) {
                if let Some(callback) = self.on_open {
                    callback(&result.file.path);
                }
            }
        }
    }

    /// Get entry at point
    fn entry_at_point(&self, x: isize, y: isize) -> Option<usize> {
        let local_y = (y - self.bounds.y) as usize;
        let list_y = Self::SEARCH_BAR_HEIGHT + Self::FILTER_BAR_HEIGHT;

        if local_y < list_y {
            return None;
        }

        let row = (local_y - list_y) / Self::ROW_HEIGHT;
        let index = self.scroll_offset + row;

        if index < self.results.len() {
            Some(index)
        } else {
            None
        }
    }
}

impl Widget for SearchWidget {
    fn id(&self) -> WidgetId { self.id }
    fn bounds(&self) -> Bounds { self.bounds }

    fn set_position(&mut self, x: isize, y: isize) {
        self.bounds.x = x;
        self.bounds.y = y;
    }

    fn set_size(&mut self, width: usize, height: usize) {
        self.bounds.width = width;
        self.bounds.height = height;
        let content_height = height.saturating_sub(Self::SEARCH_BAR_HEIGHT + Self::FILTER_BAR_HEIGHT);
        self.visible_rows = content_height / Self::ROW_HEIGHT;
    }

    fn is_enabled(&self) -> bool { true }
    fn set_enabled(&mut self, _enabled: bool) {}
    fn is_visible(&self) -> bool { self.visible }
    fn set_visible(&mut self, visible: bool) { self.visible = visible; }

    fn handle_event(&mut self, event: &WidgetEvent) -> bool {
        match event {
            WidgetEvent::Focus => { self.focused = true; true }
            WidgetEvent::Blur => { self.focused = false; true }
            WidgetEvent::Character { c } => {
                if self.search_bar_focused && *c >= ' ' {
                    self.query.push(*c);
                    self.do_search();
                    return true;
                }
                false
            }
            WidgetEvent::KeyDown { key, .. } => {
                match *key {
                    0x0E => { // Backspace
                        if self.search_bar_focused {
                            self.query.pop();
                            self.do_search();
                            return true;
                        }
                    }
                    0x1C => { // Enter
                        self.open_selected();
                        return true;
                    }
                    0x48 => { // Up
                        if let Some(idx) = self.selected_index {
                            if idx > 0 {
                                self.selected_index = Some(idx - 1);
                                if idx - 1 < self.scroll_offset {
                                    self.scroll_offset = idx - 1;
                                }
                            }
                        }
                        return true;
                    }
                    0x50 => { // Down
                        if let Some(idx) = self.selected_index {
                            if idx + 1 < self.results.len() {
                                self.selected_index = Some(idx + 1);
                                if idx + 1 >= self.scroll_offset + self.visible_rows {
                                    self.scroll_offset = idx + 2 - self.visible_rows;
                                }
                            }
                        } else if !self.results.is_empty() {
                            self.selected_index = Some(0);
                        }
                        return true;
                    }
                    0x01 => { // Escape
                        self.query.clear();
                        self.results.clear();
                        self.selected_index = None;
                        return true;
                    }
                    _ => {}
                }
                false
            }
            WidgetEvent::MouseDown { button: MouseButton::Left, x, y } => {
                let local_y = (*y - self.bounds.y) as usize;

                // Check search bar
                if local_y < Self::SEARCH_BAR_HEIGHT {
                    self.search_bar_focused = true;
                    return true;
                }

                // Check results
                if let Some(idx) = self.entry_at_point(*x, *y) {
                    self.selected_index = Some(idx);
                    self.search_bar_focused = false;
                    return true;
                }
                false
            }
            WidgetEvent::DoubleClick { x, y, .. } => {
                if let Some(idx) = self.entry_at_point(*x, *y) {
                    self.selected_index = Some(idx);
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
                    let max = self.results.len().saturating_sub(self.visible_rows);
                    self.scroll_offset = self.scroll_offset.min(max);
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

        // Search bar background
        let search_bg = Color::new(45, 45, 48);
        for py in 0..Self::SEARCH_BAR_HEIGHT {
            for px in 0..w {
                surface.set_pixel(x + px, y + py, search_bg);
            }
        }

        // Search input
        let input_x = x + 8;
        let input_y = y + 8;
        let input_w = w - 16;
        let input_h = Self::SEARCH_BAR_HEIGHT - 16;
        let input_bg = if self.search_bar_focused {
            Color::new(255, 255, 255)
        } else {
            Color::new(60, 60, 63)
        };
        let input_fg = if self.search_bar_focused {
            Color::new(0, 0, 0)
        } else {
            theme.fg
        };

        for py in 0..input_h {
            for px in 0..input_w {
                surface.set_pixel(input_x + px, input_y + py, input_bg);
            }
        }

        // Search icon / placeholder
        let display_text = if self.query.is_empty() && !self.search_bar_focused {
            "Search files..."
        } else {
            &self.query
        };
        let text_color = if self.query.is_empty() && !self.search_bar_focused {
            Color::new(100, 100, 100)
        } else {
            input_fg
        };

        for (i, c) in display_text.chars().take(60).enumerate() {
            draw_char(surface, input_x + 8 + i * Self::CHAR_WIDTH, input_y + 3, c, text_color);
        }

        // Filter bar
        let filter_y = y + Self::SEARCH_BAR_HEIGHT;
        let filter_bg = Color::new(50, 50, 53);
        for py in 0..Self::FILTER_BAR_HEIGHT {
            for px in 0..w {
                surface.set_pixel(x + px, filter_y + py, filter_bg);
            }
        }

        // Filter labels
        draw_string(surface, x + 8, filter_y + 8, "All", theme.fg);
        draw_string(surface, x + 50, filter_y + 8, "Images", Color::new(150, 150, 150));
        draw_string(surface, x + 120, filter_y + 8, "Documents", Color::new(150, 150, 150));
        draw_string(surface, x + 210, filter_y + 8, "Code", Color::new(150, 150, 150));

        // Results count
        let count_str = format!("{} results", self.results.len());
        let count_x = x + w - count_str.len() * Self::CHAR_WIDTH - 8;
        for (i, c) in count_str.chars().enumerate() {
            draw_char(surface, count_x + i * Self::CHAR_WIDTH, filter_y + 8, c, Color::new(100, 100, 100));
        }

        // Results list
        let list_y = y + Self::SEARCH_BAR_HEIGHT + Self::FILTER_BAR_HEIGHT;

        for (row_idx, result) in self.results.iter().skip(self.scroll_offset).take(self.visible_rows).enumerate() {
            let item_y = list_y + row_idx * Self::ROW_HEIGHT;
            let actual_idx = self.scroll_offset + row_idx;

            // Background
            let bg = if self.selected_index == Some(actual_idx) {
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

            // Icon
            let icon = result.file.file_type.icon();
            for (i, c) in icon.chars().enumerate() {
                draw_char(surface, x + 8 + i * Self::CHAR_WIDTH, item_y + 4, c, theme.fg);
            }

            // Name with highlights
            let name = &result.file.name;
            for (i, c) in name.chars().take(40).enumerate() {
                let in_highlight = result.highlights.iter().any(|(start, end)| i >= *start && i < *end);
                let color = if in_highlight { Color::new(255, 200, 0) } else { theme.fg };
                draw_char(surface, x + 40 + i * Self::CHAR_WIDTH, item_y + 4, c, color);
            }

            // Path
            let path_start = x + 40 + 41 * Self::CHAR_WIDTH;
            for (i, c) in result.file.path.chars().take(40).enumerate() {
                draw_char(surface, path_start + i * Self::CHAR_WIDTH, item_y + 4, c, Color::new(100, 100, 100));
            }
        }

        // Empty state
        if self.results.is_empty() && !self.query.is_empty() {
            let msg = "No results found";
            let msg_x = x + (w - msg.len() * Self::CHAR_WIDTH) / 2;
            let msg_y = list_y + 50;
            for (i, c) in msg.chars().enumerate() {
                draw_char(surface, msg_x + i * Self::CHAR_WIDTH, msg_y, c, Color::new(100, 100, 100));
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
