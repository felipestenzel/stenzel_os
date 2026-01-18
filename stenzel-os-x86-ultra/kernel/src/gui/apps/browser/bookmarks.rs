//! Browser Bookmarks
//!
//! Bookmark management for the web browser including folders, tags, and sync.

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use alloc::collections::BTreeMap;

/// Unique bookmark identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BookmarkId(u64);

impl BookmarkId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

/// Unique folder identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FolderId(u64);

impl FolderId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

/// Unique tag identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TagId(u64);

impl TagId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

/// Bookmark type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookmarkType {
    /// Regular bookmark
    Bookmark,
    /// Separator line
    Separator,
    /// Folder reference
    FolderRef,
}

/// Bookmark entry
#[derive(Debug, Clone)]
pub struct Bookmark {
    pub id: BookmarkId,
    pub title: String,
    pub url: String,
    pub folder_id: Option<FolderId>,
    pub favicon_url: Option<String>,
    pub description: Option<String>,
    pub keywords: Vec<String>,
    pub tags: Vec<TagId>,
    pub created: u64,
    pub modified: u64,
    pub last_visited: Option<u64>,
    pub visit_count: u32,
    pub position: u32,
    pub bookmark_type: BookmarkType,
}

impl Bookmark {
    pub fn new(id: BookmarkId, title: &str, url: &str) -> Self {
        Self {
            id,
            title: String::from(title),
            url: String::from(url),
            folder_id: None,
            favicon_url: None,
            description: None,
            keywords: Vec::new(),
            tags: Vec::new(),
            created: 0,
            modified: 0,
            last_visited: None,
            visit_count: 0,
            position: 0,
            bookmark_type: BookmarkType::Bookmark,
        }
    }

    pub fn separator(id: BookmarkId) -> Self {
        Self {
            id,
            title: String::new(),
            url: String::new(),
            folder_id: None,
            favicon_url: None,
            description: None,
            keywords: Vec::new(),
            tags: Vec::new(),
            created: 0,
            modified: 0,
            last_visited: None,
            visit_count: 0,
            position: 0,
            bookmark_type: BookmarkType::Separator,
        }
    }

    pub fn domain(&self) -> &str {
        // Extract domain from URL
        if let Some(start) = self.url.find("://") {
            let after_proto = &self.url[start + 3..];
            if let Some(end) = after_proto.find('/') {
                return &after_proto[..end];
            }
            return after_proto;
        }
        &self.url
    }

    pub fn display_title(&self) -> &str {
        if self.title.is_empty() {
            self.domain()
        } else {
            &self.title
        }
    }

    pub fn is_separator(&self) -> bool {
        self.bookmark_type == BookmarkType::Separator
    }

    pub fn record_visit(&mut self, timestamp: u64) {
        self.visit_count += 1;
        self.last_visited = Some(timestamp);
    }
}

/// Bookmark folder
#[derive(Debug, Clone)]
pub struct BookmarkFolder {
    pub id: FolderId,
    pub name: String,
    pub parent_id: Option<FolderId>,
    pub position: u32,
    pub created: u64,
    pub modified: u64,
    pub is_expanded: bool,
    pub is_toolbar: bool,
    pub icon: Option<String>,
}

impl BookmarkFolder {
    pub fn new(id: FolderId, name: &str) -> Self {
        Self {
            id,
            name: String::from(name),
            parent_id: None,
            position: 0,
            created: 0,
            modified: 0,
            is_expanded: true,
            is_toolbar: false,
            icon: None,
        }
    }

    pub fn toolbar(id: FolderId) -> Self {
        let mut folder = Self::new(id, "Bookmarks Toolbar");
        folder.is_toolbar = true;
        folder
    }
}

/// Bookmark tag
#[derive(Debug, Clone)]
pub struct BookmarkTag {
    pub id: TagId,
    pub name: String,
    pub color: u32,
    pub bookmark_count: u32,
}

impl BookmarkTag {
    pub fn new(id: TagId, name: &str) -> Self {
        Self {
            id,
            name: String::from(name),
            color: 0x3498db, // Default blue
            bookmark_count: 0,
        }
    }

    pub fn with_color(mut self, color: u32) -> Self {
        self.color = color;
        self
    }
}

/// Special bookmark folders
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialFolder {
    /// Root bookmark folder
    Root,
    /// Bookmarks toolbar (below address bar)
    Toolbar,
    /// Other bookmarks (not in toolbar)
    Other,
    /// Bookmarks from mobile devices
    Mobile,
    /// Recently bookmarked
    Recent,
    /// Frequently visited bookmarks
    Frequent,
}

impl SpecialFolder {
    pub fn name(&self) -> &'static str {
        match self {
            SpecialFolder::Root => "Bookmarks",
            SpecialFolder::Toolbar => "Bookmarks Toolbar",
            SpecialFolder::Other => "Other Bookmarks",
            SpecialFolder::Mobile => "Mobile Bookmarks",
            SpecialFolder::Recent => "Recent Bookmarks",
            SpecialFolder::Frequent => "Frequently Visited",
        }
    }
}

/// Bookmark sort order
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    /// Manual ordering (by position)
    Manual,
    /// By title A-Z
    TitleAsc,
    /// By title Z-A
    TitleDesc,
    /// By URL A-Z
    UrlAsc,
    /// By URL Z-A
    UrlDesc,
    /// By date added (newest first)
    DateAddedDesc,
    /// By date added (oldest first)
    DateAddedAsc,
    /// By last visited (most recent first)
    LastVisitedDesc,
    /// By visit count (most visited first)
    VisitCountDesc,
}

impl SortOrder {
    pub fn name(&self) -> &'static str {
        match self {
            SortOrder::Manual => "Manual",
            SortOrder::TitleAsc => "Title A-Z",
            SortOrder::TitleDesc => "Title Z-A",
            SortOrder::UrlAsc => "URL A-Z",
            SortOrder::UrlDesc => "URL Z-A",
            SortOrder::DateAddedDesc => "Date Added (Newest)",
            SortOrder::DateAddedAsc => "Date Added (Oldest)",
            SortOrder::LastVisitedDesc => "Last Visited",
            SortOrder::VisitCountDesc => "Most Visited",
        }
    }
}

/// Import/export format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookmarkFormat {
    /// Netscape HTML format (most compatible)
    Html,
    /// JSON format (preserves all data)
    Json,
    /// Chrome/Edge JSON
    ChromeJson,
    /// Firefox JSON
    FirefoxJson,
}

impl BookmarkFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            BookmarkFormat::Html => "html",
            BookmarkFormat::Json => "json",
            BookmarkFormat::ChromeJson => "json",
            BookmarkFormat::FirefoxJson => "json",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            BookmarkFormat::Html => "HTML (Netscape)",
            BookmarkFormat::Json => "JSON",
            BookmarkFormat::ChromeJson => "Chrome/Edge",
            BookmarkFormat::FirefoxJson => "Firefox",
        }
    }
}

/// Bookmark search result
#[derive(Debug, Clone)]
pub struct BookmarkSearchResult {
    pub bookmark: Bookmark,
    pub folder_path: String,
    pub match_score: u32,
    pub matched_in_title: bool,
    pub matched_in_url: bool,
    pub matched_in_tags: bool,
}

/// Bookmark error types
#[derive(Debug, Clone)]
pub enum BookmarkError {
    NotFound,
    DuplicateUrl,
    InvalidFolder,
    CircularReference,
    ImportFailed(String),
    ExportFailed(String),
    SyncFailed(String),
}

/// Bookmark operation result
pub type BookmarkResult<T> = Result<T, BookmarkError>;

/// Bookmark manager
pub struct BookmarkManager {
    bookmarks: BTreeMap<BookmarkId, Bookmark>,
    folders: BTreeMap<FolderId, BookmarkFolder>,
    tags: BTreeMap<TagId, BookmarkTag>,

    // Special folder IDs
    root_folder_id: FolderId,
    toolbar_folder_id: FolderId,
    other_folder_id: FolderId,
    mobile_folder_id: FolderId,

    // ID counters
    next_bookmark_id: u64,
    next_folder_id: u64,
    next_tag_id: u64,

    // Settings
    default_sort_order: SortOrder,
    show_favicons: bool,
    show_toolbar: bool,
    confirm_delete: bool,
}

impl BookmarkManager {
    pub fn new() -> Self {
        let mut manager = Self {
            bookmarks: BTreeMap::new(),
            folders: BTreeMap::new(),
            tags: BTreeMap::new(),
            root_folder_id: FolderId::new(0),
            toolbar_folder_id: FolderId::new(1),
            other_folder_id: FolderId::new(2),
            mobile_folder_id: FolderId::new(3),
            next_bookmark_id: 1,
            next_folder_id: 4,
            next_tag_id: 1,
            default_sort_order: SortOrder::Manual,
            show_favicons: true,
            show_toolbar: true,
            confirm_delete: true,
        };

        // Create special folders
        manager.create_special_folders();
        manager
    }

    fn create_special_folders(&mut self) {
        // Root folder
        let root = BookmarkFolder::new(self.root_folder_id, "Bookmarks");
        self.folders.insert(self.root_folder_id, root);

        // Toolbar folder
        let mut toolbar = BookmarkFolder::toolbar(self.toolbar_folder_id);
        toolbar.parent_id = Some(self.root_folder_id);
        toolbar.position = 0;
        self.folders.insert(self.toolbar_folder_id, toolbar);

        // Other bookmarks folder
        let mut other = BookmarkFolder::new(self.other_folder_id, "Other Bookmarks");
        other.parent_id = Some(self.root_folder_id);
        other.position = 1;
        self.folders.insert(self.other_folder_id, other);

        // Mobile bookmarks folder
        let mut mobile = BookmarkFolder::new(self.mobile_folder_id, "Mobile Bookmarks");
        mobile.parent_id = Some(self.root_folder_id);
        mobile.position = 2;
        self.folders.insert(self.mobile_folder_id, mobile);
    }

    /// Add a new bookmark
    pub fn add_bookmark(&mut self, title: &str, url: &str) -> BookmarkId {
        let id = BookmarkId::new(self.next_bookmark_id);
        self.next_bookmark_id += 1;

        let mut bookmark = Bookmark::new(id, title, url);
        bookmark.folder_id = Some(self.other_folder_id);
        bookmark.position = self.count_bookmarks_in_folder(self.other_folder_id) as u32;

        self.bookmarks.insert(id, bookmark);
        id
    }

    /// Add bookmark to toolbar
    pub fn add_to_toolbar(&mut self, title: &str, url: &str) -> BookmarkId {
        let id = BookmarkId::new(self.next_bookmark_id);
        self.next_bookmark_id += 1;

        let mut bookmark = Bookmark::new(id, title, url);
        bookmark.folder_id = Some(self.toolbar_folder_id);
        bookmark.position = self.count_bookmarks_in_folder(self.toolbar_folder_id) as u32;

        self.bookmarks.insert(id, bookmark);
        id
    }

    /// Add bookmark to specific folder
    pub fn add_to_folder(&mut self, title: &str, url: &str, folder_id: FolderId) -> BookmarkResult<BookmarkId> {
        if !self.folders.contains_key(&folder_id) {
            return Err(BookmarkError::InvalidFolder);
        }

        let id = BookmarkId::new(self.next_bookmark_id);
        self.next_bookmark_id += 1;

        let mut bookmark = Bookmark::new(id, title, url);
        bookmark.folder_id = Some(folder_id);
        bookmark.position = self.count_bookmarks_in_folder(folder_id) as u32;

        self.bookmarks.insert(id, bookmark);
        Ok(id)
    }

    /// Create a new folder
    pub fn create_folder(&mut self, name: &str, parent_id: Option<FolderId>) -> BookmarkResult<FolderId> {
        let parent = parent_id.unwrap_or(self.other_folder_id);

        if !self.folders.contains_key(&parent) {
            return Err(BookmarkError::InvalidFolder);
        }

        let id = FolderId::new(self.next_folder_id);
        self.next_folder_id += 1;

        let mut folder = BookmarkFolder::new(id, name);
        folder.parent_id = Some(parent);
        folder.position = self.count_subfolders(parent) as u32;

        self.folders.insert(id, folder);
        Ok(id)
    }

    /// Create a new tag
    pub fn create_tag(&mut self, name: &str) -> TagId {
        let id = TagId::new(self.next_tag_id);
        self.next_tag_id += 1;

        let tag = BookmarkTag::new(id, name);
        self.tags.insert(id, tag);
        id
    }

    /// Add tag to bookmark
    pub fn add_tag_to_bookmark(&mut self, bookmark_id: BookmarkId, tag_id: TagId) -> BookmarkResult<()> {
        let bookmark = self.bookmarks.get_mut(&bookmark_id)
            .ok_or(BookmarkError::NotFound)?;

        if !bookmark.tags.contains(&tag_id) {
            bookmark.tags.push(tag_id);

            if let Some(tag) = self.tags.get_mut(&tag_id) {
                tag.bookmark_count += 1;
            }
        }

        Ok(())
    }

    /// Remove bookmark
    pub fn remove_bookmark(&mut self, id: BookmarkId) -> BookmarkResult<Bookmark> {
        self.bookmarks.remove(&id).ok_or(BookmarkError::NotFound)
    }

    /// Remove folder and all contents
    pub fn remove_folder(&mut self, id: FolderId) -> BookmarkResult<()> {
        // Don't allow deleting special folders
        if id == self.root_folder_id ||
           id == self.toolbar_folder_id ||
           id == self.other_folder_id ||
           id == self.mobile_folder_id {
            return Err(BookmarkError::InvalidFolder);
        }

        // Remove all bookmarks in folder
        let bookmark_ids: Vec<_> = self.bookmarks.iter()
            .filter(|(_, b)| b.folder_id == Some(id))
            .map(|(id, _)| *id)
            .collect();

        for bookmark_id in bookmark_ids {
            self.bookmarks.remove(&bookmark_id);
        }

        // Remove subfolders recursively
        let subfolder_ids: Vec<_> = self.folders.iter()
            .filter(|(_, f)| f.parent_id == Some(id))
            .map(|(id, _)| *id)
            .collect();

        for folder_id in subfolder_ids {
            let _ = self.remove_folder(folder_id);
        }

        self.folders.remove(&id);
        Ok(())
    }

    /// Move bookmark to folder
    pub fn move_bookmark(&mut self, bookmark_id: BookmarkId, folder_id: FolderId) -> BookmarkResult<()> {
        if !self.folders.contains_key(&folder_id) {
            return Err(BookmarkError::InvalidFolder);
        }

        let new_position = self.count_bookmarks_in_folder(folder_id) as u32;

        let bookmark = self.bookmarks.get_mut(&bookmark_id)
            .ok_or(BookmarkError::NotFound)?;

        bookmark.folder_id = Some(folder_id);
        bookmark.position = new_position;

        Ok(())
    }

    /// Move folder
    pub fn move_folder(&mut self, folder_id: FolderId, new_parent_id: FolderId) -> BookmarkResult<()> {
        // Can't move special folders
        if folder_id == self.root_folder_id ||
           folder_id == self.toolbar_folder_id ||
           folder_id == self.other_folder_id ||
           folder_id == self.mobile_folder_id {
            return Err(BookmarkError::InvalidFolder);
        }

        // Check for circular reference
        if self.is_descendant_of(new_parent_id, folder_id) {
            return Err(BookmarkError::CircularReference);
        }

        let new_position = self.count_subfolders(new_parent_id) as u32;

        let folder = self.folders.get_mut(&folder_id)
            .ok_or(BookmarkError::NotFound)?;

        folder.parent_id = Some(new_parent_id);
        folder.position = new_position;

        Ok(())
    }

    /// Check if folder is descendant of another
    fn is_descendant_of(&self, potential_descendant: FolderId, potential_ancestor: FolderId) -> bool {
        let mut current = potential_descendant;

        while let Some(folder) = self.folders.get(&current) {
            if let Some(parent) = folder.parent_id {
                if parent == potential_ancestor {
                    return true;
                }
                current = parent;
            } else {
                break;
            }
        }

        false
    }

    /// Get bookmark by ID
    pub fn get_bookmark(&self, id: BookmarkId) -> Option<&Bookmark> {
        self.bookmarks.get(&id)
    }

    /// Get mutable bookmark
    pub fn get_bookmark_mut(&mut self, id: BookmarkId) -> Option<&mut Bookmark> {
        self.bookmarks.get_mut(&id)
    }

    /// Get folder by ID
    pub fn get_folder(&self, id: FolderId) -> Option<&BookmarkFolder> {
        self.folders.get(&id)
    }

    /// Get bookmarks in folder
    pub fn get_bookmarks_in_folder(&self, folder_id: FolderId) -> Vec<&Bookmark> {
        let mut bookmarks: Vec<_> = self.bookmarks.values()
            .filter(|b| b.folder_id == Some(folder_id))
            .collect();

        bookmarks.sort_by_key(|b| b.position);
        bookmarks
    }

    /// Get subfolders
    pub fn get_subfolders(&self, parent_id: FolderId) -> Vec<&BookmarkFolder> {
        let mut folders: Vec<_> = self.folders.values()
            .filter(|f| f.parent_id == Some(parent_id))
            .collect();

        folders.sort_by_key(|f| f.position);
        folders
    }

    /// Get toolbar bookmarks
    pub fn get_toolbar_bookmarks(&self) -> Vec<&Bookmark> {
        self.get_bookmarks_in_folder(self.toolbar_folder_id)
    }

    /// Count bookmarks in folder
    fn count_bookmarks_in_folder(&self, folder_id: FolderId) -> usize {
        self.bookmarks.values()
            .filter(|b| b.folder_id == Some(folder_id))
            .count()
    }

    /// Count subfolders
    fn count_subfolders(&self, parent_id: FolderId) -> usize {
        self.folders.values()
            .filter(|f| f.parent_id == Some(parent_id))
            .count()
    }

    /// Search bookmarks
    pub fn search(&self, query: &str) -> Vec<BookmarkSearchResult> {
        let query_lower = query.to_ascii_lowercase();
        let mut results = Vec::new();

        for bookmark in self.bookmarks.values() {
            if bookmark.is_separator() {
                continue;
            }

            let mut score = 0u32;
            let mut matched_in_title = false;
            let mut matched_in_url = false;
            let mut matched_in_tags = false;

            // Check title
            let title_lower = bookmark.title.to_ascii_lowercase();
            if title_lower.contains(&query_lower) {
                score += 100;
                matched_in_title = true;

                if title_lower.starts_with(&query_lower) {
                    score += 50;
                }
            }

            // Check URL
            let url_lower = bookmark.url.to_ascii_lowercase();
            if url_lower.contains(&query_lower) {
                score += 50;
                matched_in_url = true;
            }

            // Check keywords
            for keyword in &bookmark.keywords {
                if keyword.to_ascii_lowercase().contains(&query_lower) {
                    score += 30;
                }
            }

            // Check tags
            for tag_id in &bookmark.tags {
                if let Some(tag) = self.tags.get(tag_id) {
                    if tag.name.to_ascii_lowercase().contains(&query_lower) {
                        score += 20;
                        matched_in_tags = true;
                    }
                }
            }

            // Add visit count bonus
            score += (bookmark.visit_count / 10).min(50);

            if score > 0 {
                let folder_path = self.get_folder_path(bookmark.folder_id);
                results.push(BookmarkSearchResult {
                    bookmark: bookmark.clone(),
                    folder_path,
                    match_score: score,
                    matched_in_title,
                    matched_in_url,
                    matched_in_tags,
                });
            }
        }

        // Sort by score (highest first)
        results.sort_by(|a, b| b.match_score.cmp(&a.match_score));
        results
    }

    /// Get folder path as string
    fn get_folder_path(&self, folder_id: Option<FolderId>) -> String {
        let mut path = Vec::new();
        let mut current = folder_id;

        while let Some(id) = current {
            if let Some(folder) = self.folders.get(&id) {
                if id != self.root_folder_id {
                    path.push(folder.name.clone());
                }
                current = folder.parent_id;
            } else {
                break;
            }
        }

        path.reverse();
        path.join(" > ")
    }

    /// Find bookmark by URL
    pub fn find_by_url(&self, url: &str) -> Option<&Bookmark> {
        self.bookmarks.values().find(|b| b.url == url)
    }

    /// Check if URL is bookmarked
    pub fn is_bookmarked(&self, url: &str) -> bool {
        self.find_by_url(url).is_some()
    }

    /// Get recent bookmarks
    pub fn get_recent(&self, limit: usize) -> Vec<&Bookmark> {
        let mut bookmarks: Vec<_> = self.bookmarks.values()
            .filter(|b| !b.is_separator())
            .collect();

        bookmarks.sort_by(|a, b| b.created.cmp(&a.created));
        bookmarks.truncate(limit);
        bookmarks
    }

    /// Get frequently visited bookmarks
    pub fn get_frequent(&self, limit: usize) -> Vec<&Bookmark> {
        let mut bookmarks: Vec<_> = self.bookmarks.values()
            .filter(|b| !b.is_separator() && b.visit_count > 0)
            .collect();

        bookmarks.sort_by(|a, b| b.visit_count.cmp(&a.visit_count));
        bookmarks.truncate(limit);
        bookmarks
    }

    /// Get bookmarks by tag
    pub fn get_by_tag(&self, tag_id: TagId) -> Vec<&Bookmark> {
        self.bookmarks.values()
            .filter(|b| b.tags.contains(&tag_id))
            .collect()
    }

    /// Get all tags
    pub fn get_tags(&self) -> Vec<&BookmarkTag> {
        self.tags.values().collect()
    }

    /// Total bookmark count
    pub fn total_bookmarks(&self) -> usize {
        self.bookmarks.values().filter(|b| !b.is_separator()).count()
    }

    /// Total folder count
    pub fn total_folders(&self) -> usize {
        self.folders.len()
    }

    /// Export to HTML (Netscape bookmark format)
    pub fn export_html(&self) -> String {
        let mut html = String::from("<!DOCTYPE NETSCAPE-Bookmark-file-1>\n");
        html.push_str("<!-- This is an automatically generated file.\n");
        html.push_str("     It will be read and overwritten.\n");
        html.push_str("     DO NOT EDIT! -->\n");
        html.push_str("<META HTTP-EQUIV=\"Content-Type\" CONTENT=\"text/html; charset=UTF-8\">\n");
        html.push_str("<TITLE>Bookmarks</TITLE>\n");
        html.push_str("<H1>Bookmarks</H1>\n");
        html.push_str("<DL><p>\n");

        self.export_folder_html(&mut html, self.toolbar_folder_id, 1);
        self.export_folder_html(&mut html, self.other_folder_id, 1);

        html.push_str("</DL><p>\n");
        html
    }

    fn export_folder_html(&self, html: &mut String, folder_id: FolderId, indent: usize) {
        let indent_str: String = "    ".repeat(indent);

        if let Some(folder) = self.folders.get(&folder_id) {
            html.push_str(&format!("{}<DT><H3>{}</H3>\n", indent_str, folder.name));
            html.push_str(&format!("{}<DL><p>\n", indent_str));

            // Subfolders
            for subfolder in self.get_subfolders(folder_id) {
                self.export_folder_html(html, subfolder.id, indent + 1);
            }

            // Bookmarks
            for bookmark in self.get_bookmarks_in_folder(folder_id) {
                if bookmark.is_separator() {
                    html.push_str(&format!("{}<HR>\n", "    ".repeat(indent + 1)));
                } else {
                    html.push_str(&format!(
                        "{}<DT><A HREF=\"{}\">{}</A>\n",
                        "    ".repeat(indent + 1),
                        bookmark.url,
                        bookmark.title
                    ));
                }
            }

            html.push_str(&format!("{}</DL><p>\n", indent_str));
        }
    }

    /// Special folder accessors
    pub fn root_folder(&self) -> FolderId {
        self.root_folder_id
    }

    pub fn toolbar_folder(&self) -> FolderId {
        self.toolbar_folder_id
    }

    pub fn other_folder(&self) -> FolderId {
        self.other_folder_id
    }

    pub fn mobile_folder(&self) -> FolderId {
        self.mobile_folder_id
    }

    /// Settings
    pub fn show_toolbar(&self) -> bool {
        self.show_toolbar
    }

    pub fn set_show_toolbar(&mut self, show: bool) {
        self.show_toolbar = show;
    }

    pub fn show_favicons(&self) -> bool {
        self.show_favicons
    }

    pub fn set_show_favicons(&mut self, show: bool) {
        self.show_favicons = show;
    }

    /// Add sample bookmarks for demo
    pub fn add_sample_data(&mut self) {
        // Add some common bookmarks to toolbar
        let _ = self.add_to_toolbar("Google", "https://www.google.com");
        let _ = self.add_to_toolbar("GitHub", "https://github.com");
        let _ = self.add_to_toolbar("Stack Overflow", "https://stackoverflow.com");
        let _ = self.add_to_toolbar("Wikipedia", "https://en.wikipedia.org");

        // Create a "Development" folder
        if let Ok(dev_folder) = self.create_folder("Development", Some(self.other_folder_id)) {
            let _ = self.add_to_folder("Rust Lang", "https://www.rust-lang.org", dev_folder);
            let _ = self.add_to_folder("Docs.rs", "https://docs.rs", dev_folder);
            let _ = self.add_to_folder("crates.io", "https://crates.io", dev_folder);
        }

        // Create a "News" folder
        if let Ok(news_folder) = self.create_folder("News", Some(self.other_folder_id)) {
            let _ = self.add_to_folder("Hacker News", "https://news.ycombinator.com", news_folder);
            let _ = self.add_to_folder("Reddit", "https://www.reddit.com", news_folder);
        }

        // Add some tags
        let work_tag = self.create_tag("Work");
        let rust_tag = self.create_tag("Rust");

        // Tag some bookmarks
        for (_, bookmark) in self.bookmarks.iter_mut() {
            if bookmark.url.contains("rust") || bookmark.url.contains("crates") || bookmark.url.contains("docs.rs") {
                bookmark.tags.push(rust_tag);
            }
            if bookmark.url.contains("github") || bookmark.url.contains("stackoverflow") {
                bookmark.tags.push(work_tag);
            }
        }
    }
}

impl Default for BookmarkManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize bookmarks module
pub fn init() -> BookmarkManager {
    let mut manager = BookmarkManager::new();
    manager.add_sample_data();
    manager
}
