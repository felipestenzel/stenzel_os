//! Recent Files Widget
//!
//! Tracks and displays recently accessed files across the system.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use alloc::collections::BTreeMap;

use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use crate::gui::widgets::{Widget, WidgetId, WidgetEvent, Bounds, MouseButton, theme};

/// Maximum number of recent files to track
pub const MAX_RECENT_FILES: usize = 100;

/// Time threshold for "today" grouping (in seconds)
pub const TODAY_THRESHOLD: u64 = 86400;

/// Time threshold for "this week" grouping (in seconds)
pub const WEEK_THRESHOLD: u64 = 604800;

/// Time threshold for "this month" grouping (in seconds)
pub const MONTH_THRESHOLD: u64 = 2592000;

/// File type category for filtering
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileCategory {
    All,
    Documents,
    Images,
    Videos,
    Audio,
    Archives,
    Code,
    Other,
}

impl FileCategory {
    pub fn name(&self) -> &'static str {
        match self {
            FileCategory::All => "All Files",
            FileCategory::Documents => "Documents",
            FileCategory::Images => "Images",
            FileCategory::Videos => "Videos",
            FileCategory::Audio => "Audio",
            FileCategory::Archives => "Archives",
            FileCategory::Code => "Code",
            FileCategory::Other => "Other",
        }
    }

    pub fn icon(&self) -> char {
        match self {
            FileCategory::All => '*',
            FileCategory::Documents => 'D',
            FileCategory::Images => 'I',
            FileCategory::Videos => 'V',
            FileCategory::Audio => 'A',
            FileCategory::Archives => 'Z',
            FileCategory::Code => 'C',
            FileCategory::Other => '?',
        }
    }

    pub fn from_extension(ext: &str) -> FileCategory {
        match ext.to_lowercase().as_str() {
            // Documents
            "txt" | "doc" | "docx" | "pdf" | "odt" | "rtf" | "md" | "tex" => FileCategory::Documents,
            // Images
            "png" | "jpg" | "jpeg" | "gif" | "bmp" | "svg" | "ico" | "webp" | "tiff" => FileCategory::Images,
            // Videos
            "mp4" | "avi" | "mkv" | "mov" | "wmv" | "flv" | "webm" => FileCategory::Videos,
            // Audio
            "mp3" | "wav" | "flac" | "ogg" | "aac" | "wma" | "m4a" => FileCategory::Audio,
            // Archives
            "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" => FileCategory::Archives,
            // Code
            "rs" | "c" | "cpp" | "h" | "py" | "js" | "ts" | "java" | "go" | "rb" | "sh" | "html" | "css" => FileCategory::Code,
            // Other
            _ => FileCategory::Other,
        }
    }
}

/// Time grouping for recent files
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TimeGroup {
    Today,
    Yesterday,
    ThisWeek,
    ThisMonth,
    Older,
}

impl TimeGroup {
    pub fn name(&self) -> &'static str {
        match self {
            TimeGroup::Today => "Today",
            TimeGroup::Yesterday => "Yesterday",
            TimeGroup::ThisWeek => "This Week",
            TimeGroup::ThisMonth => "This Month",
            TimeGroup::Older => "Older",
        }
    }

    pub fn from_timestamp(accessed: u64, current_time: u64) -> TimeGroup {
        if current_time < accessed {
            return TimeGroup::Today;
        }

        let age = current_time - accessed;

        if age < TODAY_THRESHOLD {
            TimeGroup::Today
        } else if age < TODAY_THRESHOLD * 2 {
            TimeGroup::Yesterday
        } else if age < WEEK_THRESHOLD {
            TimeGroup::ThisWeek
        } else if age < MONTH_THRESHOLD {
            TimeGroup::ThisMonth
        } else {
            TimeGroup::Older
        }
    }
}

/// A recently accessed file entry
#[derive(Debug, Clone)]
pub struct RecentFile {
    /// Full path to the file
    pub path: String,
    /// File name (without path)
    pub name: String,
    /// File extension
    pub extension: String,
    /// File size in bytes
    pub size: u64,
    /// Last access timestamp (Unix epoch)
    pub last_accessed: u64,
    /// File category
    pub category: FileCategory,
    /// Application that opened the file
    pub opened_with: Option<String>,
    /// Whether the file still exists
    pub exists: bool,
    /// Thumbnail path (if available)
    pub thumbnail: Option<String>,
    /// Number of times accessed
    pub access_count: u32,
}

impl RecentFile {
    pub fn new(path: &str, size: u64, last_accessed: u64) -> Self {
        let name = path.rsplit('/').next().unwrap_or(path).to_string();
        let extension = name.rsplit('.').next().unwrap_or("").to_string();
        let category = FileCategory::from_extension(&extension);

        RecentFile {
            path: path.to_string(),
            name,
            extension,
            size,
            last_accessed,
            category,
            opened_with: None,
            exists: true,
            thumbnail: None,
            access_count: 1,
        }
    }

    pub fn format_size(&self) -> String {
        if self.size < 1024 {
            format!("{} B", self.size)
        } else if self.size < 1024 * 1024 {
            format!("{} KB", self.size / 1024)
        } else if self.size < 1024 * 1024 * 1024 {
            format!("{:.1} MB", self.size as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.2} GB", self.size as f64 / (1024.0 * 1024.0 * 1024.0))
        }
    }

    pub fn format_time(&self, current_time: u64) -> String {
        if current_time < self.last_accessed {
            return "Just now".to_string();
        }

        let age = current_time - self.last_accessed;

        if age < 60 {
            "Just now".to_string()
        } else if age < 3600 {
            format!("{} min ago", age / 60)
        } else if age < TODAY_THRESHOLD {
            format!("{} hours ago", age / 3600)
        } else if age < TODAY_THRESHOLD * 2 {
            "Yesterday".to_string()
        } else if age < WEEK_THRESHOLD {
            format!("{} days ago", age / TODAY_THRESHOLD)
        } else if age < MONTH_THRESHOLD {
            format!("{} weeks ago", age / WEEK_THRESHOLD)
        } else {
            format!("{} months ago", age / MONTH_THRESHOLD)
        }
    }

    pub fn time_group(&self, current_time: u64) -> TimeGroup {
        TimeGroup::from_timestamp(self.last_accessed, current_time)
    }
}

/// Sort order for recent files
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    RecentFirst,
    OldestFirst,
    NameAZ,
    NameZA,
    SizeSmallest,
    SizeLargest,
    MostAccessed,
}

impl SortOrder {
    pub fn name(&self) -> &'static str {
        match self {
            SortOrder::RecentFirst => "Most Recent",
            SortOrder::OldestFirst => "Oldest First",
            SortOrder::NameAZ => "Name (A-Z)",
            SortOrder::NameZA => "Name (Z-A)",
            SortOrder::SizeSmallest => "Size (Smallest)",
            SortOrder::SizeLargest => "Size (Largest)",
            SortOrder::MostAccessed => "Most Accessed",
        }
    }
}

/// Recent files manager - tracks and manages file access history
pub struct RecentFilesManager {
    files: Vec<RecentFile>,
    max_files: usize,
}

impl RecentFilesManager {
    pub fn new() -> Self {
        RecentFilesManager {
            files: Vec::new(),
            max_files: MAX_RECENT_FILES,
        }
    }

    pub fn with_max_files(max: usize) -> Self {
        RecentFilesManager {
            files: Vec::new(),
            max_files: max,
        }
    }

    /// Add or update a file in the recent files list
    pub fn add_file(&mut self, path: &str, size: u64, timestamp: u64) {
        // Check if file already exists
        if let Some(existing) = self.files.iter_mut().find(|f| f.path == path) {
            existing.last_accessed = timestamp;
            existing.access_count += 1;
            existing.size = size;
            return;
        }

        // Add new file
        let file = RecentFile::new(path, size, timestamp);
        self.files.push(file);

        // Sort by most recent
        self.files.sort_by(|a, b| b.last_accessed.cmp(&a.last_accessed));

        // Trim to max size
        if self.files.len() > self.max_files {
            self.files.truncate(self.max_files);
        }
    }

    /// Remove a file from recent files
    pub fn remove_file(&mut self, path: &str) {
        self.files.retain(|f| f.path != path);
    }

    /// Clear all recent files
    pub fn clear(&mut self) {
        self.files.clear();
    }

    /// Get all recent files
    pub fn files(&self) -> &[RecentFile] {
        &self.files
    }

    /// Get files filtered by category
    pub fn files_by_category(&self, category: FileCategory) -> Vec<&RecentFile> {
        if category == FileCategory::All {
            self.files.iter().collect()
        } else {
            self.files.iter().filter(|f| f.category == category).collect()
        }
    }

    /// Get files grouped by time
    pub fn files_by_time_group(&self, current_time: u64) -> BTreeMap<TimeGroup, Vec<&RecentFile>> {
        let mut groups: BTreeMap<TimeGroup, Vec<&RecentFile>> = BTreeMap::new();

        for file in &self.files {
            let group = file.time_group(current_time);
            groups.entry(group).or_insert_with(Vec::new).push(file);
        }

        groups
    }

    /// Mark files as non-existent if they've been deleted
    pub fn validate_files(&mut self, file_exists_fn: impl Fn(&str) -> bool) {
        for file in &mut self.files {
            file.exists = file_exists_fn(&file.path);
        }
    }

    /// Remove non-existent files
    pub fn prune_missing(&mut self) {
        self.files.retain(|f| f.exists);
    }

    /// Get statistics
    pub fn stats(&self) -> RecentFilesStats {
        let mut stats = RecentFilesStats::default();
        stats.total_files = self.files.len();

        for file in &self.files {
            stats.total_size += file.size;
            stats.total_accesses += file.access_count as u64;

            match file.category {
                FileCategory::Documents => stats.documents += 1,
                FileCategory::Images => stats.images += 1,
                FileCategory::Videos => stats.videos += 1,
                FileCategory::Audio => stats.audio += 1,
                FileCategory::Archives => stats.archives += 1,
                FileCategory::Code => stats.code += 1,
                _ => stats.other += 1,
            }
        }

        stats
    }
}

impl Default for RecentFilesManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about recent files
#[derive(Debug, Default, Clone)]
pub struct RecentFilesStats {
    pub total_files: usize,
    pub total_size: u64,
    pub total_accesses: u64,
    pub documents: usize,
    pub images: usize,
    pub videos: usize,
    pub audio: usize,
    pub archives: usize,
    pub code: usize,
    pub other: usize,
}

/// Recent Files Widget - GUI for browsing recent files
pub struct RecentFilesWidget {
    id: WidgetId,
    bounds: Bounds,
    enabled: bool,
    visible: bool,

    /// File manager
    manager: RecentFilesManager,

    /// Current filter category
    filter_category: FileCategory,

    /// Current sort order
    sort_order: SortOrder,

    /// Search query
    search_query: String,

    /// Selected file index
    selected_index: Option<usize>,

    /// Scroll offset
    scroll_offset: usize,

    /// Show grouped by time
    group_by_time: bool,

    /// Current simulated time for demo
    current_time: u64,

    /// Category filter buttons
    category_buttons: Vec<(FileCategory, Bounds)>,

    /// Filtered and sorted files (indices into manager.files)
    filtered_indices: Vec<usize>,

    /// Hovered item
    hovered_index: Option<usize>,

    /// Show file details panel
    show_details: bool,
}

impl RecentFilesWidget {
    pub fn new(id: WidgetId, x: isize, y: isize, width: usize, height: usize) -> Self {
        let mut widget = RecentFilesWidget {
            id,
            bounds: Bounds { x, y, width, height },
            enabled: true,
            visible: true,
            manager: RecentFilesManager::new(),
            filter_category: FileCategory::All,
            sort_order: SortOrder::RecentFirst,
            search_query: String::new(),
            selected_index: None,
            scroll_offset: 0,
            group_by_time: true,
            current_time: 1737216000, // 2025-01-18 simulated
            category_buttons: Vec::new(),
            filtered_indices: Vec::new(),
            hovered_index: None,
            show_details: true,
        };

        // Add sample recent files for demo
        widget.add_sample_files();
        widget.update_filtered_list();
        widget.update_category_buttons();

        widget
    }

    fn add_sample_files(&mut self) {
        let base_time = self.current_time;

        // Today's files
        self.manager.add_file("/home/user/Documents/report.pdf", 1_500_000, base_time - 3600);
        self.manager.add_file("/home/user/Pictures/photo.jpg", 3_200_000, base_time - 7200);
        self.manager.add_file("/home/user/Code/main.rs", 15_000, base_time - 1800);
        self.manager.add_file("/home/user/Downloads/archive.zip", 50_000_000, base_time - 5400);

        // Yesterday's files
        self.manager.add_file("/home/user/Documents/notes.txt", 5_000, base_time - 100_000);
        self.manager.add_file("/home/user/Music/song.mp3", 8_000_000, base_time - 120_000);

        // This week
        self.manager.add_file("/home/user/Videos/movie.mp4", 2_000_000_000, base_time - 300_000);
        self.manager.add_file("/home/user/Documents/presentation.pdf", 25_000_000, base_time - 400_000);
        self.manager.add_file("/home/user/Code/lib.rs", 8_000, base_time - 350_000);

        // This month
        self.manager.add_file("/home/user/Pictures/vacation.png", 5_500_000, base_time - 1_500_000);
        self.manager.add_file("/home/user/Documents/contract.docx", 120_000, base_time - 2_000_000);

        // Older
        self.manager.add_file("/home/user/Archives/backup.tar.gz", 500_000_000, base_time - 5_000_000);
        self.manager.add_file("/home/user/Code/old_project.c", 25_000, base_time - 8_000_000);
    }

    fn update_filtered_list(&mut self) {
        self.filtered_indices.clear();

        let files = self.manager.files();

        for (idx, file) in files.iter().enumerate() {
            // Category filter
            if self.filter_category != FileCategory::All && file.category != self.filter_category {
                continue;
            }

            // Search filter
            if !self.search_query.is_empty() {
                let query_lower = self.search_query.to_lowercase();
                if !file.name.to_lowercase().contains(&query_lower) &&
                   !file.path.to_lowercase().contains(&query_lower) {
                    continue;
                }
            }

            self.filtered_indices.push(idx);
        }

        // Sort
        let files = self.manager.files();
        match self.sort_order {
            SortOrder::RecentFirst => {
                self.filtered_indices.sort_by(|a, b| {
                    files[*b].last_accessed.cmp(&files[*a].last_accessed)
                });
            }
            SortOrder::OldestFirst => {
                self.filtered_indices.sort_by(|a, b| {
                    files[*a].last_accessed.cmp(&files[*b].last_accessed)
                });
            }
            SortOrder::NameAZ => {
                self.filtered_indices.sort_by(|a, b| {
                    files[*a].name.cmp(&files[*b].name)
                });
            }
            SortOrder::NameZA => {
                self.filtered_indices.sort_by(|a, b| {
                    files[*b].name.cmp(&files[*a].name)
                });
            }
            SortOrder::SizeSmallest => {
                self.filtered_indices.sort_by(|a, b| {
                    files[*a].size.cmp(&files[*b].size)
                });
            }
            SortOrder::SizeLargest => {
                self.filtered_indices.sort_by(|a, b| {
                    files[*b].size.cmp(&files[*a].size)
                });
            }
            SortOrder::MostAccessed => {
                self.filtered_indices.sort_by(|a, b| {
                    files[*b].access_count.cmp(&files[*a].access_count)
                });
            }
        }
    }

    fn update_category_buttons(&mut self) {
        self.category_buttons.clear();

        let categories = [
            FileCategory::All,
            FileCategory::Documents,
            FileCategory::Images,
            FileCategory::Videos,
            FileCategory::Audio,
            FileCategory::Archives,
            FileCategory::Code,
        ];

        let button_width = 90;
        let button_height = 24;
        let padding = 5;
        let start_x = self.bounds.x + 10;
        let start_y = self.bounds.y + 40;

        for (i, cat) in categories.iter().enumerate() {
            let x = start_x + (i as isize) * (button_width + padding) as isize;
            self.category_buttons.push((*cat, Bounds {
                x,
                y: start_y,
                width: button_width,
                height: button_height,
            }));
        }
    }

    fn get_visible_item_count(&self) -> usize {
        let list_height = self.bounds.height.saturating_sub(120); // Header area
        list_height / 32 // Item height
    }

    fn set_filter_category(&mut self, category: FileCategory) {
        self.filter_category = category;
        self.selected_index = None;
        self.scroll_offset = 0;
        self.update_filtered_list();
    }

    pub fn add_recent_file(&mut self, path: &str, size: u64, timestamp: u64) {
        self.manager.add_file(path, size, timestamp);
        self.update_filtered_list();
    }

    pub fn remove_recent_file(&mut self, path: &str) {
        self.manager.remove_file(path);
        self.update_filtered_list();
    }

    pub fn clear_recent_files(&mut self) {
        self.manager.clear();
        self.filtered_indices.clear();
        self.selected_index = None;
    }

    pub fn get_selected_file(&self) -> Option<&RecentFile> {
        self.selected_index.and_then(|idx| {
            self.filtered_indices.get(idx).and_then(|file_idx| {
                self.manager.files().get(*file_idx)
            })
        })
    }

    pub fn set_sort_order(&mut self, order: SortOrder) {
        self.sort_order = order;
        self.update_filtered_list();
    }

    pub fn set_search_query(&mut self, query: &str) {
        self.search_query = query.to_string();
        self.selected_index = None;
        self.scroll_offset = 0;
        self.update_filtered_list();
    }

    pub fn stats(&self) -> RecentFilesStats {
        self.manager.stats()
    }
}

impl Widget for RecentFilesWidget {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn bounds(&self) -> Bounds {
        self.bounds
    }

    fn set_position(&mut self, x: isize, y: isize) {
        self.bounds.x = x;
        self.bounds.y = y;
        self.update_category_buttons();
    }

    fn set_size(&mut self, width: usize, height: usize) {
        self.bounds.width = width;
        self.bounds.height = height;
        self.update_category_buttons();
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
            WidgetEvent::MouseDown { x, y, button } => {
                if *button != MouseButton::Left {
                    return false;
                }

                let px = *x as isize;
                let py = *y as isize;

                // Check if inside widget
                if px < self.bounds.x || px >= self.bounds.x + self.bounds.width as isize ||
                   py < self.bounds.y || py >= self.bounds.y + self.bounds.height as isize {
                    return false;
                }

                // Check category buttons
                for (cat, bounds) in &self.category_buttons {
                    if px >= bounds.x && px < bounds.x + bounds.width as isize &&
                       py >= bounds.y && py < bounds.y + bounds.height as isize {
                        let cat_copy = *cat;
                        self.set_filter_category(cat_copy);
                        return true;
                    }
                }

                // Check file list clicks
                let list_start_y = self.bounds.y + 80;
                let item_height = 32isize;

                if py >= list_start_y {
                    let relative_y = py - list_start_y;
                    let clicked_idx = (relative_y / item_height) as usize + self.scroll_offset;

                    if clicked_idx < self.filtered_indices.len() {
                        self.selected_index = Some(clicked_idx);
                        return true;
                    }
                }

                false
            }

            WidgetEvent::MouseMove { x, y } => {
                let px = *x as isize;
                let py = *y as isize;

                // Check if inside widget
                if px < self.bounds.x || px >= self.bounds.x + self.bounds.width as isize ||
                   py < self.bounds.y || py >= self.bounds.y + self.bounds.height as isize {
                    self.hovered_index = None;
                    return false;
                }

                let list_start_y = self.bounds.y + 80;
                let item_height = 32isize;

                if py >= list_start_y {
                    let relative_y = py - list_start_y;
                    let hovered_idx = (relative_y / item_height) as usize + self.scroll_offset;

                    if hovered_idx < self.filtered_indices.len() {
                        self.hovered_index = Some(hovered_idx);
                    } else {
                        self.hovered_index = None;
                    }
                } else {
                    self.hovered_index = None;
                }

                true
            }

            WidgetEvent::Scroll { delta_y, .. } => {
                let max_scroll = self.filtered_indices.len().saturating_sub(self.get_visible_item_count());

                if *delta_y < 0 && self.scroll_offset < max_scroll {
                    self.scroll_offset = (self.scroll_offset + 3).min(max_scroll);
                    return true;
                } else if *delta_y > 0 && self.scroll_offset > 0 {
                    self.scroll_offset = self.scroll_offset.saturating_sub(3);
                    return true;
                }

                false
            }

            WidgetEvent::KeyDown { key, .. } => {
                match *key {
                    0x48 => { // Up arrow
                        if let Some(idx) = self.selected_index {
                            if idx > 0 {
                                self.selected_index = Some(idx - 1);
                                if idx - 1 < self.scroll_offset {
                                    self.scroll_offset = idx - 1;
                                }
                                return true;
                            }
                        } else if !self.filtered_indices.is_empty() {
                            self.selected_index = Some(0);
                            return true;
                        }
                        false
                    }
                    0x50 => { // Down arrow
                        if let Some(idx) = self.selected_index {
                            if idx + 1 < self.filtered_indices.len() {
                                self.selected_index = Some(idx + 1);
                                let visible = self.get_visible_item_count();
                                if idx + 1 >= self.scroll_offset + visible {
                                    self.scroll_offset = idx + 2 - visible;
                                }
                                return true;
                            }
                        } else if !self.filtered_indices.is_empty() {
                            self.selected_index = Some(0);
                            return true;
                        }
                        false
                    }
                    0x53 | 0x7F => { // Delete
                        if let Some(idx) = self.selected_index {
                            if let Some(file_idx) = self.filtered_indices.get(idx) {
                                if let Some(file) = self.manager.files().get(*file_idx) {
                                    let path = file.path.clone();
                                    self.manager.remove_file(&path);
                                    self.update_filtered_list();
                                    if idx >= self.filtered_indices.len() && idx > 0 {
                                        self.selected_index = Some(idx - 1);
                                    } else if self.filtered_indices.is_empty() {
                                        self.selected_index = None;
                                    }
                                    return true;
                                }
                            }
                        }
                        false
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

        let _theme = theme();
        let bg_color = Color::new(30, 30, 35);
        let text_color = Color::new(220, 220, 220);
        let accent_color = Color::new(0, 122, 255);
        let selected_bg = Color::new(60, 100, 180);
        let hover_bg = Color::new(70, 70, 80);
        let header_bg = Color::new(40, 40, 50);
        let border_color = Color::new(80, 80, 90);

        let x = self.bounds.x as usize;
        let y = self.bounds.y as usize;
        let w = self.bounds.width;
        let h = self.bounds.height;

        // Draw background
        surface.fill_rect(x, y, w, h, bg_color);

        // Draw border
        surface.draw_rect(x, y, w, h, border_color);

        // Draw header
        surface.fill_rect(x, y, w, 35, header_bg);
        draw_string(surface, x + 10, y + 10, "Recent Files", text_color);

        // Draw stats
        let stats = self.manager.stats();
        let stats_text = format!("{} files", stats.total_files);
        draw_string(surface, x + w - 100, y + 10, &stats_text, Color::new(150, 150, 150));

        // Draw category buttons
        for (cat, bounds) in &self.category_buttons {
            let is_selected = *cat == self.filter_category;
            let btn_bg = if is_selected { accent_color } else { Color::new(50, 50, 60) };
            let btn_text = if is_selected { Color::new(255, 255, 255) } else { text_color };

            surface.fill_rect(bounds.x as usize, bounds.y as usize, bounds.width, bounds.height, btn_bg);
            draw_string(surface, bounds.x as usize + 5, bounds.y as usize + 6, cat.name(), btn_text);
        }

        // Draw file list header
        let list_header_y = y + 70;
        surface.fill_rect(x, list_header_y, w, 20, Color::new(35, 35, 45));
        draw_string(surface, x + 10, list_header_y + 4, "Name", Color::new(180, 180, 180));
        draw_string(surface, x + 300, list_header_y + 4, "Size", Color::new(180, 180, 180));
        draw_string(surface, x + 400, list_header_y + 4, "Accessed", Color::new(180, 180, 180));
        draw_string(surface, x + 550, list_header_y + 4, "Type", Color::new(180, 180, 180));

        // Draw file list
        let list_start_y = y + 90;
        let item_height = 32;
        let visible_items = self.get_visible_item_count();
        let files = self.manager.files();

        for (display_idx, file_idx) in self.filtered_indices.iter()
            .skip(self.scroll_offset)
            .take(visible_items)
            .enumerate()
        {
            let actual_idx = display_idx + self.scroll_offset;
            let item_y = list_start_y + display_idx * item_height;

            if let Some(file) = files.get(*file_idx) {
                // Background for selection/hover
                let item_bg = if Some(actual_idx) == self.selected_index {
                    selected_bg
                } else if Some(actual_idx) == self.hovered_index {
                    hover_bg
                } else if display_idx % 2 == 0 {
                    Color::new(35, 35, 40)
                } else {
                    bg_color
                };

                surface.fill_rect(x + 1, item_y, w - 2, item_height - 2, item_bg);

                // Category icon
                let icon = file.category.icon();
                draw_char(surface, x + 10, item_y + 8, icon, accent_color);

                // File name (truncate if too long)
                let name = if file.name.len() > 32 {
                    format!("{}...", &file.name[..29])
                } else {
                    file.name.clone()
                };
                let name_color = if file.exists { text_color } else { Color::new(150, 100, 100) };
                draw_string(surface, x + 30, item_y + 8, &name, name_color);

                // Size
                let size_str = file.format_size();
                draw_string(surface, x + 300, item_y + 8, &size_str, Color::new(150, 150, 150));

                // Access time
                let time_str = file.format_time(self.current_time);
                draw_string(surface, x + 400, item_y + 8, &time_str, Color::new(150, 150, 150));

                // Category name
                draw_string(surface, x + 550, item_y + 8, file.category.name(), Color::new(120, 120, 130));
            }
        }

        // Draw scrollbar if needed
        if self.filtered_indices.len() > visible_items {
            let scrollbar_x = x + w - 12;
            let scrollbar_height = h - 100;
            let scrollbar_y = list_start_y;

            // Track
            surface.fill_rect(scrollbar_x, scrollbar_y, 8, scrollbar_height, Color::new(40, 40, 50));

            // Thumb
            let total_items = self.filtered_indices.len();
            let thumb_height = (visible_items * scrollbar_height / total_items).max(20);
            let thumb_pos = self.scroll_offset * (scrollbar_height - thumb_height) /
                           (total_items - visible_items).max(1);

            surface.fill_rect(scrollbar_x, scrollbar_y + thumb_pos, 8, thumb_height, accent_color);
        }

        // Draw details panel if a file is selected and details are enabled
        if self.show_details {
            if let Some(file) = self.get_selected_file() {
                let panel_y = y + h - 80;
                surface.fill_rect(x, panel_y, w, 78, header_bg);
                surface.draw_rect(x, panel_y, w, 78, border_color);

                draw_string(surface, x + 10, panel_y + 8, "Selected:", Color::new(150, 150, 150));
                draw_string(surface, x + 80, panel_y + 8, &file.name, text_color);

                draw_string(surface, x + 10, panel_y + 28, "Path:", Color::new(150, 150, 150));
                let path_display = if file.path.len() > 70 {
                    format!("...{}", &file.path[file.path.len()-67..])
                } else {
                    file.path.clone()
                };
                draw_string(surface, x + 50, panel_y + 28, &path_display, Color::new(130, 130, 140));

                draw_string(surface, x + 10, panel_y + 48, "Size:", Color::new(150, 150, 150));
                draw_string(surface, x + 50, panel_y + 48, &file.format_size(), text_color);

                let access_str = format!("Accessed {} times", file.access_count);
                draw_string(surface, x + 200, panel_y + 48, &access_str, Color::new(130, 130, 140));
            }
        }

        // Empty state
        if self.filtered_indices.is_empty() {
            let empty_msg = if self.search_query.is_empty() {
                "No recent files"
            } else {
                "No files match your search"
            };
            let msg_x = x + (w - empty_msg.len() * 8) / 2;
            let msg_y = y + h / 2;
            draw_string(surface, msg_x, msg_y, empty_msg, Color::new(120, 120, 130));
        }
    }
}

// Helper functions for text rendering
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
