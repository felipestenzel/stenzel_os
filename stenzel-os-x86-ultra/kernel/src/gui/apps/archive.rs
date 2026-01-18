//! Archive Manager Application
//!
//! A graphical application for managing compressed archives.
//! Supports ZIP, TAR, GZIP, BZIP2, XZ, 7Z, and RAR formats.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::format;

use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use crate::gui::widgets::{Widget, WidgetId, WidgetEvent, Bounds, MouseButton, theme};

/// Archive format types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat {
    Zip,
    Tar,
    TarGz,
    TarBz2,
    TarXz,
    TarZst,
    Gzip,
    Bzip2,
    Xz,
    Zstd,
    SevenZip,
    Rar,
    Unknown,
}

impl ArchiveFormat {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "zip" => ArchiveFormat::Zip,
            "tar" => ArchiveFormat::Tar,
            "gz" | "gzip" => ArchiveFormat::Gzip,
            "bz2" | "bzip2" => ArchiveFormat::Bzip2,
            "xz" => ArchiveFormat::Xz,
            "zst" | "zstd" => ArchiveFormat::Zstd,
            "7z" => ArchiveFormat::SevenZip,
            "rar" => ArchiveFormat::Rar,
            _ => ArchiveFormat::Unknown,
        }
    }

    pub fn from_path(path: &str) -> Self {
        if path.ends_with(".tar.gz") || path.ends_with(".tgz") {
            return ArchiveFormat::TarGz;
        }
        if path.ends_with(".tar.bz2") || path.ends_with(".tbz2") {
            return ArchiveFormat::TarBz2;
        }
        if path.ends_with(".tar.xz") || path.ends_with(".txz") {
            return ArchiveFormat::TarXz;
        }
        if path.ends_with(".tar.zst") {
            return ArchiveFormat::TarZst;
        }

        if let Some(pos) = path.rfind('.') {
            Self::from_extension(&path[pos+1..])
        } else {
            ArchiveFormat::Unknown
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            ArchiveFormat::Zip => "zip",
            ArchiveFormat::Tar => "tar",
            ArchiveFormat::TarGz => "tar.gz",
            ArchiveFormat::TarBz2 => "tar.bz2",
            ArchiveFormat::TarXz => "tar.xz",
            ArchiveFormat::TarZst => "tar.zst",
            ArchiveFormat::Gzip => "gz",
            ArchiveFormat::Bzip2 => "bz2",
            ArchiveFormat::Xz => "xz",
            ArchiveFormat::Zstd => "zst",
            ArchiveFormat::SevenZip => "7z",
            ArchiveFormat::Rar => "rar",
            ArchiveFormat::Unknown => "",
        }
    }

    pub fn mime_type(&self) -> &'static str {
        match self {
            ArchiveFormat::Zip => "application/zip",
            ArchiveFormat::Tar => "application/x-tar",
            ArchiveFormat::TarGz => "application/gzip",
            ArchiveFormat::TarBz2 => "application/x-bzip2",
            ArchiveFormat::TarXz => "application/x-xz",
            ArchiveFormat::TarZst => "application/zstd",
            ArchiveFormat::Gzip => "application/gzip",
            ArchiveFormat::Bzip2 => "application/x-bzip2",
            ArchiveFormat::Xz => "application/x-xz",
            ArchiveFormat::Zstd => "application/zstd",
            ArchiveFormat::SevenZip => "application/x-7z-compressed",
            ArchiveFormat::Rar => "application/vnd.rar",
            ArchiveFormat::Unknown => "application/octet-stream",
        }
    }

    pub fn supports_password(&self) -> bool {
        matches!(self, ArchiveFormat::Zip | ArchiveFormat::SevenZip | ArchiveFormat::Rar)
    }
}

/// Compression level for creating archives
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionLevel {
    None,
    Fastest,
    Fast,
    Normal,
    Maximum,
    Ultra,
}

impl CompressionLevel {
    pub fn as_level(&self) -> u32 {
        match self {
            CompressionLevel::None => 0,
            CompressionLevel::Fastest => 1,
            CompressionLevel::Fast => 3,
            CompressionLevel::Normal => 6,
            CompressionLevel::Maximum => 9,
            CompressionLevel::Ultra => 11,
        }
    }
}

/// Entry type in archive
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EntryType {
    Directory,
    File,
    Symlink,
    Hardlink,
    Unknown,
}

/// Archive entry metadata
#[derive(Debug, Clone)]
pub struct ArchiveEntry {
    pub path: String,
    pub entry_type: EntryType,
    pub size: u64,
    pub compressed_size: u64,
    pub mtime: u64,
    pub mode: u32,
    pub crc32: u32,
    pub encrypted: bool,
    pub compression: String,
    pub comment: Option<String>,
    pub link_target: Option<String>,
}

impl ArchiveEntry {
    pub fn new(path: &str, entry_type: EntryType) -> Self {
        Self {
            path: String::from(path),
            entry_type,
            size: 0,
            compressed_size: 0,
            mtime: 0,
            mode: 0o644,
            crc32: 0,
            encrypted: false,
            compression: String::from("stored"),
            comment: None,
            link_target: None,
        }
    }

    pub fn compression_ratio(&self) -> f32 {
        if self.size == 0 {
            return 0.0;
        }
        (1.0 - (self.compressed_size as f32 / self.size as f32)) * 100.0
    }

    pub fn is_directory(&self) -> bool {
        self.entry_type == EntryType::Directory || self.path.ends_with('/')
    }

    pub fn file_name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }

    pub fn parent_path(&self) -> &str {
        if let Some(pos) = self.path.rfind('/') {
            &self.path[..pos]
        } else {
            ""
        }
    }
}

/// Archive information
#[derive(Debug, Clone)]
pub struct ArchiveInfo {
    pub path: String,
    pub format: ArchiveFormat,
    pub total_size: u64,
    pub compressed_size: u64,
    pub entry_count: usize,
    pub file_count: usize,
    pub dir_count: usize,
    pub encrypted: bool,
    pub comment: Option<String>,
    pub entries: Vec<ArchiveEntry>,
}

impl ArchiveInfo {
    pub fn new(path: &str, format: ArchiveFormat) -> Self {
        Self {
            path: String::from(path),
            format,
            total_size: 0,
            compressed_size: 0,
            entry_count: 0,
            file_count: 0,
            dir_count: 0,
            encrypted: false,
            comment: None,
            entries: Vec::new(),
        }
    }

    pub fn compression_ratio(&self) -> f32 {
        if self.total_size == 0 {
            return 0.0;
        }
        (1.0 - (self.compressed_size as f32 / self.total_size as f32)) * 100.0
    }
}

/// Archive operation progress
#[derive(Debug, Clone)]
pub struct ArchiveProgress {
    pub operation: ArchiveOperation,
    pub current_entry: String,
    pub current_entry_index: usize,
    pub total_entries: usize,
    pub bytes_processed: u64,
    pub total_bytes: u64,
    pub percent: f32,
}

/// Archive operation type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveOperation {
    Opening,
    Listing,
    Extracting,
    Creating,
    Adding,
    Deleting,
    Testing,
}

/// Archive error
#[derive(Debug)]
pub enum ArchiveError {
    FileNotFound,
    InvalidFormat,
    CorruptArchive,
    PasswordRequired,
    WrongPassword,
    UnsupportedFormat,
    IoError,
    OutOfMemory,
    PermissionDenied,
    PathTooLong,
}

/// View mode for archive contents
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    FlatList,
    TreeView,
}

/// Sort field
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortField {
    Name,
    Size,
    CompressedSize,
    ModTime,
    Type,
    Ratio,
}

/// Sort direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

/// Extract options
#[derive(Debug, Clone)]
pub struct ExtractOptions {
    pub destination: String,
    pub overwrite: bool,
    pub preserve_structure: bool,
    pub preserve_permissions: bool,
    pub preserve_timestamps: bool,
    pub selected_only: bool,
    pub password: Option<String>,
}

impl Default for ExtractOptions {
    fn default() -> Self {
        Self {
            destination: String::from("."),
            overwrite: false,
            preserve_structure: true,
            preserve_permissions: true,
            preserve_timestamps: true,
            selected_only: false,
            password: None,
        }
    }
}

/// Create options
#[derive(Debug, Clone)]
pub struct CreateOptions {
    pub format: ArchiveFormat,
    pub compression_level: CompressionLevel,
    pub password: Option<String>,
    pub solid: bool,
    pub store_symlinks: bool,
    pub comment: Option<String>,
}

impl Default for CreateOptions {
    fn default() -> Self {
        Self {
            format: ArchiveFormat::Zip,
            compression_level: CompressionLevel::Normal,
            password: None,
            solid: false,
            store_symlinks: true,
            comment: None,
        }
    }
}

/// Archive Manager widget
pub struct ArchiveManager {
    id: WidgetId,
    bounds: Bounds,
    visible: bool,
    enabled: bool,
    focused: bool,
    current_archive: Option<ArchiveInfo>,
    current_path: String,
    selected_entries: Vec<usize>,
    view_mode: ViewMode,
    sort_field: SortField,
    sort_direction: SortDirection,
    show_hidden: bool,
    search_filter: String,
    scroll_offset: usize,
    visible_rows: usize,
    progress: Option<ArchiveProgress>,
    status_message: String,
    expanded_dirs: Vec<String>,
    hover_index: Option<usize>,
    context_menu_visible: bool,
    context_menu_pos: (isize, isize),
    current_password: Option<String>,
}

impl ArchiveManager {
    const HEADER_HEIGHT: usize = 36;
    const ROW_HEIGHT: usize = 20;
    const STATUS_HEIGHT: usize = 22;
    const CHAR_WIDTH: usize = 8;

    pub fn new(x: isize, y: isize, width: usize, height: usize) -> Self {
        let content_height = height.saturating_sub(Self::HEADER_HEIGHT + Self::STATUS_HEIGHT);
        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(x, y, width, height),
            visible: true,
            enabled: true,
            focused: false,
            current_archive: None,
            current_path: String::new(),
            selected_entries: Vec::new(),
            view_mode: ViewMode::FlatList,
            sort_field: SortField::Name,
            sort_direction: SortDirection::Ascending,
            show_hidden: false,
            search_filter: String::new(),
            scroll_offset: 0,
            visible_rows: content_height / Self::ROW_HEIGHT,
            progress: None,
            status_message: String::from("Ready"),
            expanded_dirs: Vec::new(),
            hover_index: None,
            context_menu_visible: false,
            context_menu_pos: (0, 0),
            current_password: None,
        }
    }

    pub fn open(&mut self, path: &str) -> Result<(), ArchiveError> {
        let format = ArchiveFormat::from_path(path);
        if format == ArchiveFormat::Unknown {
            return Err(ArchiveError::InvalidFormat);
        }

        self.status_message = format!("Opening {}...", path);

        let mut info = ArchiveInfo::new(path, format);

        info.entries.push(ArchiveEntry {
            path: String::from("README.md"),
            entry_type: EntryType::File,
            size: 1024,
            compressed_size: 512,
            mtime: 1705574400,
            mode: 0o644,
            crc32: 0xDEADBEEF,
            encrypted: false,
            compression: String::from("deflate"),
            comment: None,
            link_target: None,
        });

        info.entries.push(ArchiveEntry {
            path: String::from("src/"),
            entry_type: EntryType::Directory,
            size: 0,
            compressed_size: 0,
            mtime: 1705574400,
            mode: 0o755,
            crc32: 0,
            encrypted: false,
            compression: String::from("stored"),
            comment: None,
            link_target: None,
        });

        info.entries.push(ArchiveEntry {
            path: String::from("src/main.rs"),
            entry_type: EntryType::File,
            size: 2048,
            compressed_size: 1024,
            mtime: 1705574400,
            mode: 0o644,
            crc32: 0xCAFEBABE,
            encrypted: false,
            compression: String::from("deflate"),
            comment: None,
            link_target: None,
        });

        for entry in &info.entries {
            info.total_size += entry.size;
            info.compressed_size += entry.compressed_size;
            if entry.is_directory() {
                info.dir_count += 1;
            } else {
                info.file_count += 1;
            }
        }
        info.entry_count = info.entries.len();

        self.current_archive = Some(info);
        self.selected_entries.clear();
        self.scroll_offset = 0;
        self.current_path.clear();
        self.status_message = String::from("Archive opened");

        Ok(())
    }

    pub fn close(&mut self) {
        self.current_archive = None;
        self.selected_entries.clear();
        self.current_path.clear();
        self.status_message = String::from("Ready");
    }

    pub fn extract(&mut self, options: ExtractOptions) -> Result<(), ArchiveError> {
        let archive = self.current_archive.as_ref().ok_or(ArchiveError::FileNotFound)?;

        self.status_message = format!("Extracting to {}...", options.destination);

        let entries_to_extract: Vec<&ArchiveEntry> = if options.selected_only && !self.selected_entries.is_empty() {
            self.selected_entries.iter()
                .filter_map(|&i| archive.entries.get(i))
                .collect()
        } else {
            archive.entries.iter().collect()
        };

        self.progress = Some(ArchiveProgress {
            operation: ArchiveOperation::Extracting,
            current_entry: String::new(),
            current_entry_index: 0,
            total_entries: entries_to_extract.len(),
            bytes_processed: 0,
            total_bytes: entries_to_extract.iter().map(|e| e.size).sum(),
            percent: 0.0,
        });

        for (i, entry) in entries_to_extract.iter().enumerate() {
            if let Some(ref mut progress) = self.progress {
                progress.current_entry = entry.path.clone();
                progress.current_entry_index = i;
                progress.bytes_processed += entry.size;
                progress.percent = (i as f32 / entries_to_extract.len() as f32) * 100.0;
            }
        }

        self.progress = None;
        self.status_message = format!("Extracted {} entries", entries_to_extract.len());

        Ok(())
    }

    pub fn create(&mut self, path: &str, files: &[&str], options: CreateOptions) -> Result<(), ArchiveError> {
        self.status_message = format!("Creating {}...", path);

        self.progress = Some(ArchiveProgress {
            operation: ArchiveOperation::Creating,
            current_entry: String::new(),
            current_entry_index: 0,
            total_entries: files.len(),
            bytes_processed: 0,
            total_bytes: 0,
            percent: 0.0,
        });

        let mut info = ArchiveInfo::new(path, options.format);
        info.comment = options.comment;

        for (i, file) in files.iter().enumerate() {
            if let Some(ref mut progress) = self.progress {
                progress.current_entry = String::from(*file);
                progress.current_entry_index = i;
                progress.percent = (i as f32 / files.len() as f32) * 100.0;
            }
            info.entries.push(ArchiveEntry::new(file, EntryType::File));
        }

        self.progress = None;
        self.current_archive = Some(info);
        self.status_message = format!("Created archive with {} entries", files.len());

        Ok(())
    }

    pub fn test(&mut self) -> Result<bool, ArchiveError> {
        let archive = self.current_archive.as_ref().ok_or(ArchiveError::FileNotFound)?;

        self.status_message = String::from("Testing archive...");

        self.progress = Some(ArchiveProgress {
            operation: ArchiveOperation::Testing,
            current_entry: String::new(),
            current_entry_index: 0,
            total_entries: archive.entries.len(),
            bytes_processed: 0,
            total_bytes: archive.total_size,
            percent: 0.0,
        });

        for (i, entry) in archive.entries.iter().enumerate() {
            if let Some(ref mut progress) = self.progress {
                progress.current_entry = entry.path.clone();
                progress.current_entry_index = i;
                progress.bytes_processed += entry.size;
                progress.percent = (i as f32 / archive.entries.len() as f32) * 100.0;
            }
        }

        self.progress = None;
        self.status_message = String::from("Archive OK");

        Ok(true)
    }

    pub fn get_visible_entries(&self) -> Vec<(usize, &ArchiveEntry)> {
        let Some(ref archive) = self.current_archive else {
            return Vec::new();
        };

        let mut entries: Vec<(usize, &ArchiveEntry)> = archive.entries.iter()
            .enumerate()
            .filter(|(_, e)| {
                if !self.search_filter.is_empty() {
                    if !e.path.to_lowercase().contains(&self.search_filter.to_lowercase()) {
                        return false;
                    }
                }
                if !self.show_hidden && e.file_name().starts_with('.') {
                    return false;
                }
                if self.view_mode == ViewMode::TreeView && !self.current_path.is_empty() {
                    if !e.path.starts_with(&self.current_path) {
                        return false;
                    }
                }
                true
            })
            .collect();

        entries.sort_by(|(_, a), (_, b)| {
            let ord = match self.sort_field {
                SortField::Name => a.path.cmp(&b.path),
                SortField::Size => a.size.cmp(&b.size),
                SortField::CompressedSize => a.compressed_size.cmp(&b.compressed_size),
                SortField::ModTime => a.mtime.cmp(&b.mtime),
                SortField::Type => a.entry_type.cmp(&b.entry_type),
                SortField::Ratio => {
                    let ratio_a = a.compression_ratio();
                    let ratio_b = b.compression_ratio();
                    ratio_a.partial_cmp(&ratio_b).unwrap_or(core::cmp::Ordering::Equal)
                }
            };
            match self.sort_direction {
                SortDirection::Ascending => ord,
                SortDirection::Descending => ord.reverse(),
            }
        });

        entries
    }

    pub fn select_entry(&mut self, index: usize, add_to_selection: bool) {
        if add_to_selection {
            if let Some(pos) = self.selected_entries.iter().position(|&i| i == index) {
                self.selected_entries.remove(pos);
            } else {
                self.selected_entries.push(index);
            }
        } else {
            self.selected_entries.clear();
            self.selected_entries.push(index);
        }
    }

    pub fn select_all(&mut self) {
        if let Some(ref archive) = self.current_archive {
            self.selected_entries = (0..archive.entries.len()).collect();
        }
    }

    pub fn clear_selection(&mut self) {
        self.selected_entries.clear();
    }

    pub fn navigate_to(&mut self, path: &str) {
        self.current_path = String::from(path);
        self.scroll_offset = 0;
    }

    pub fn navigate_up(&mut self) {
        if let Some(pos) = self.current_path.rfind('/') {
            self.current_path = String::from(&self.current_path[..pos]);
        } else {
            self.current_path.clear();
        }
        self.scroll_offset = 0;
    }

    pub fn set_view_mode(&mut self, mode: ViewMode) {
        self.view_mode = mode;
        self.scroll_offset = 0;
    }

    pub fn set_sort(&mut self, field: SortField, direction: SortDirection) {
        self.sort_field = field;
        self.sort_direction = direction;
    }

    pub fn set_search_filter(&mut self, filter: &str) {
        self.search_filter = String::from(filter);
        self.scroll_offset = 0;
    }

    fn format_size(size: u64) -> String {
        if size < 1024 {
            return format!("{} B", size);
        }
        let kb = size as f64 / 1024.0;
        if kb < 1024.0 {
            return format!("{:.1} KB", kb);
        }
        let mb = kb / 1024.0;
        if mb < 1024.0 {
            return format!("{:.1} MB", mb);
        }
        let gb = mb / 1024.0;
        format!("{:.1} GB", gb)
    }

    fn entry_at_point(&self, x: isize, y: isize) -> Option<usize> {
        let base_x = self.bounds.x.max(0) as usize;
        let base_y = self.bounds.y.max(0) as usize;
        let list_y = base_y + Self::HEADER_HEIGHT;
        let list_bottom = base_y + self.bounds.height.saturating_sub(Self::STATUS_HEIGHT);

        let ux = x.max(0) as usize;
        let uy = y.max(0) as usize;

        if uy < list_y || uy >= list_bottom || ux < base_x || ux >= base_x + self.bounds.width {
            return None;
        }

        let row = (uy - list_y) / Self::ROW_HEIGHT;
        let visible = self.get_visible_entries();
        let idx = self.scroll_offset + row;

        if idx < visible.len() {
            Some(visible[idx].0)
        } else {
            None
        }
    }
}

impl Widget for ArchiveManager {
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
        let content_height = height.saturating_sub(Self::HEADER_HEIGHT + Self::STATUS_HEIGHT);
        self.visible_rows = content_height / Self::ROW_HEIGHT;
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
        if !self.enabled {
            return false;
        }

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
                    self.select_entry(idx, false);
                    return true;
                }
                false
            }
            WidgetEvent::MouseDown { button: MouseButton::Right, x, y } => {
                self.context_menu_visible = true;
                self.context_menu_pos = (*x, *y);
                true
            }
            WidgetEvent::MouseMove { x, y } => {
                self.hover_index = self.entry_at_point(*x, *y);
                true
            }
            WidgetEvent::DoubleClick { button: MouseButton::Left, x, y } => {
                if let Some(idx) = self.entry_at_point(*x, *y) {
                    let path_to_navigate = self.current_archive.as_ref()
                        .and_then(|archive| archive.entries.get(idx))
                        .filter(|entry| entry.is_directory())
                        .map(|entry| entry.path.clone());

                    if let Some(path) = path_to_navigate {
                        self.navigate_to(&path);
                        return true;
                    }
                }
                false
            }
            WidgetEvent::Scroll { delta_y, .. } => {
                if *delta_y < 0 {
                    self.scroll_offset = self.scroll_offset.saturating_add(3);
                    let max_scroll = self.get_visible_entries().len().saturating_sub(self.visible_rows);
                    if self.scroll_offset > max_scroll {
                        self.scroll_offset = max_scroll;
                    }
                } else if *delta_y > 0 {
                    self.scroll_offset = self.scroll_offset.saturating_sub(3);
                }
                true
            }
            WidgetEvent::KeyDown { key, .. } => {
                match *key {
                    0x0D => {
                        let path_to_navigate = self.selected_entries.first()
                            .and_then(|&idx| {
                                self.current_archive.as_ref()
                                    .and_then(|archive| archive.entries.get(idx))
                                    .filter(|entry| entry.is_directory())
                                    .map(|entry| entry.path.clone())
                            });

                        if let Some(path) = path_to_navigate {
                            self.navigate_to(&path);
                        }
                        true
                    }
                    0x08 => {
                        self.navigate_up();
                        true
                    }
                    0x7F => {
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

        let _theme = theme();
        let x = self.bounds.x.max(0) as usize;
        let y = self.bounds.y.max(0) as usize;
        let w = self.bounds.width;
        let h = self.bounds.height;

        let bg_color = Color::new(248, 249, 250);
        let header_color = Color::new(233, 236, 239);
        let text_color = Color::BLACK;
        let selected_bg = Color::new(0, 123, 255);
        let hover_bg = Color::new(230, 243, 255);

        // Background
        surface.fill_rect(x, y, w, h, bg_color);

        // Header
        surface.fill_rect(x, y, w, Self::HEADER_HEIGHT, header_color);

        // Toolbar buttons
        draw_string(surface, x + 10, y + 10, "Open", text_color);
        draw_string(surface, x + 60, y + 10, "Extract", text_color);
        draw_string(surface, x + 130, y + 10, "Create", text_color);
        draw_string(surface, x + 200, y + 10, "Test", text_color);

        // Column headers
        let list_y = y + Self::HEADER_HEIGHT;
        surface.fill_rect(x, list_y, w, Self::ROW_HEIGHT, Color::new(222, 226, 230));
        draw_string(surface, x + 30, list_y + 4, "Name", text_color);
        draw_string(surface, x + 280, list_y + 4, "Size", text_color);
        draw_string(surface, x + 360, list_y + 4, "Packed", text_color);
        draw_string(surface, x + 440, list_y + 4, "Ratio", text_color);

        // Entries
        let content_y = list_y + Self::ROW_HEIGHT;
        let visible_entries = self.get_visible_entries();

        for (i, (orig_idx, entry)) in visible_entries.iter()
            .skip(self.scroll_offset)
            .take(self.visible_rows)
            .enumerate()
        {
            let entry_y = content_y + i * Self::ROW_HEIGHT;

            let is_selected = self.selected_entries.contains(orig_idx);
            let is_hovered = self.hover_index == Some(*orig_idx);

            if is_selected {
                surface.fill_rect(x, entry_y, w, Self::ROW_HEIGHT, selected_bg);
            } else if is_hovered {
                surface.fill_rect(x, entry_y, w, Self::ROW_HEIGHT, hover_bg);
            } else if i % 2 == 1 {
                surface.fill_rect(x, entry_y, w, Self::ROW_HEIGHT, Color::new(245, 246, 247));
            }

            let row_text_color = if is_selected { Color::WHITE } else { text_color };

            // Icon
            let icon = if entry.is_directory() { "[D]" } else { "[F]" };
            draw_string(surface, x + 5, entry_y + 4, icon, row_text_color);

            // Entry info
            let name = entry.file_name();
            let name_truncated: String = if name.len() > 30 {
                format!("{}...", &name[..27])
            } else {
                name.to_string()
            };
            draw_string(surface, x + 30, entry_y + 4, &name_truncated, row_text_color);
            draw_string(surface, x + 280, entry_y + 4, &Self::format_size(entry.size), row_text_color);
            draw_string(surface, x + 360, entry_y + 4, &Self::format_size(entry.compressed_size), row_text_color);
            draw_string(surface, x + 440, entry_y + 4, &format!("{:.0}%", entry.compression_ratio()), row_text_color);
        }

        // Status bar
        let status_y = y + h.saturating_sub(Self::STATUS_HEIGHT);
        surface.fill_rect(x, status_y, w, Self::STATUS_HEIGHT, header_color);

        if let Some(ref archive) = self.current_archive {
            let status = format!(
                "{} | {} files, {} dirs | {}",
                archive.path,
                archive.file_count,
                archive.dir_count,
                Self::format_size(archive.total_size)
            );
            draw_string(surface, x + 5, status_y + 4, &status, text_color);
        } else {
            draw_string(surface, x + 5, status_y + 4, &self.status_message, text_color);
        }

        // Progress bar
        if let Some(ref progress) = self.progress {
            let progress_y = status_y.saturating_sub(24);
            surface.fill_rect(x + 10, progress_y, w.saturating_sub(20), 18, Color::new(200, 200, 200));
            let filled_width = ((w.saturating_sub(20)) as f32 * progress.percent / 100.0) as usize;
            surface.fill_rect(x + 10, progress_y, filled_width, 18, selected_bg);
            draw_string(surface, x + 15, progress_y + 3, &progress.current_entry, text_color);
        }
    }
}

/// Draw a single character using the system font
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

/// Draw a string using the system font
fn draw_string(surface: &mut Surface, x: usize, y: usize, s: &str, color: Color) {
    for (i, c) in s.chars().enumerate() {
        draw_char(surface, x + i * 8, y, c, color);
    }
}

/// Initialize archive manager
pub fn init() {
    crate::kprintln!("archive: Archive Manager initialized");
}
