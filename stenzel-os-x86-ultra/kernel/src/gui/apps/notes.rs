//! Notes Application
//!
//! A full-featured note-taking application with rich text support,
//! notebooks, tags, and search functionality.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;

use crate::gui::widgets::{Widget, WidgetId, WidgetEvent, Bounds, MouseButton};
use crate::gui::surface::Surface;
use crate::drivers::framebuffer::Color;

/// Text formatting style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextStyle {
    Normal,
    Bold,
    Italic,
    BoldItalic,
    Strikethrough,
    Code,
}

impl TextStyle {
    pub fn name(&self) -> &'static str {
        match self {
            TextStyle::Normal => "Normal",
            TextStyle::Bold => "Bold",
            TextStyle::Italic => "Italic",
            TextStyle::BoldItalic => "Bold Italic",
            TextStyle::Strikethrough => "Strikethrough",
            TextStyle::Code => "Code",
        }
    }

    pub fn toggle_bold(&self) -> TextStyle {
        match self {
            TextStyle::Normal => TextStyle::Bold,
            TextStyle::Bold => TextStyle::Normal,
            TextStyle::Italic => TextStyle::BoldItalic,
            TextStyle::BoldItalic => TextStyle::Italic,
            _ => *self,
        }
    }

    pub fn toggle_italic(&self) -> TextStyle {
        match self {
            TextStyle::Normal => TextStyle::Italic,
            TextStyle::Italic => TextStyle::Normal,
            TextStyle::Bold => TextStyle::BoldItalic,
            TextStyle::BoldItalic => TextStyle::Bold,
            _ => *self,
        }
    }
}

/// List type for formatting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListType {
    None,
    Bullet,
    Numbered,
    Checkbox,
    CheckboxChecked,
}

impl ListType {
    pub fn prefix(&self, number: usize) -> String {
        match self {
            ListType::None => String::new(),
            ListType::Bullet => "• ".to_string(),
            ListType::Numbered => {
                let mut s = String::new();
                s.push_str(&number.to_string());
                s.push_str(". ");
                s
            }
            ListType::Checkbox => "[ ] ".to_string(),
            ListType::CheckboxChecked => "[x] ".to_string(),
        }
    }

    pub fn cycle(&self) -> ListType {
        match self {
            ListType::None => ListType::Bullet,
            ListType::Bullet => ListType::Numbered,
            ListType::Numbered => ListType::Checkbox,
            ListType::Checkbox => ListType::None,
            ListType::CheckboxChecked => ListType::Checkbox,
        }
    }
}

/// Heading level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadingLevel {
    None,
    H1,
    H2,
    H3,
}

impl HeadingLevel {
    pub fn font_size(&self) -> usize {
        match self {
            HeadingLevel::None => 14,
            HeadingLevel::H1 => 24,
            HeadingLevel::H2 => 20,
            HeadingLevel::H3 => 16,
        }
    }

    pub fn prefix(&self) -> &'static str {
        match self {
            HeadingLevel::None => "",
            HeadingLevel::H1 => "# ",
            HeadingLevel::H2 => "## ",
            HeadingLevel::H3 => "### ",
        }
    }

    pub fn cycle(&self) -> HeadingLevel {
        match self {
            HeadingLevel::None => HeadingLevel::H1,
            HeadingLevel::H1 => HeadingLevel::H2,
            HeadingLevel::H2 => HeadingLevel::H3,
            HeadingLevel::H3 => HeadingLevel::None,
        }
    }
}

/// Text block in a note
#[derive(Debug, Clone)]
pub struct TextBlock {
    pub content: String,
    pub style: TextStyle,
    pub list_type: ListType,
    pub heading: HeadingLevel,
    pub indent_level: usize,
}

impl TextBlock {
    pub fn new(content: &str) -> Self {
        Self {
            content: content.to_string(),
            style: TextStyle::Normal,
            list_type: ListType::None,
            heading: HeadingLevel::None,
            indent_level: 0,
        }
    }

    pub fn with_heading(content: &str, heading: HeadingLevel) -> Self {
        Self {
            content: content.to_string(),
            style: TextStyle::Normal,
            list_type: ListType::None,
            heading,
            indent_level: 0,
        }
    }

    pub fn with_list(content: &str, list_type: ListType) -> Self {
        Self {
            content: content.to_string(),
            style: TextStyle::Normal,
            list_type,
            heading: HeadingLevel::None,
            indent_level: 0,
        }
    }

    pub fn to_markdown(&self) -> String {
        let mut result = String::new();

        // Add indent
        for _ in 0..self.indent_level {
            result.push_str("    ");
        }

        // Add heading prefix
        result.push_str(self.heading.prefix());

        // Add list prefix
        result.push_str(&self.list_type.prefix(1));

        // Add styled content
        let styled = match self.style {
            TextStyle::Normal => self.content.clone(),
            TextStyle::Bold => {
                let mut s = String::from("**");
                s.push_str(&self.content);
                s.push_str("**");
                s
            }
            TextStyle::Italic => {
                let mut s = String::from("*");
                s.push_str(&self.content);
                s.push('*');
                s
            }
            TextStyle::BoldItalic => {
                let mut s = String::from("***");
                s.push_str(&self.content);
                s.push_str("***");
                s
            }
            TextStyle::Strikethrough => {
                let mut s = String::from("~~");
                s.push_str(&self.content);
                s.push_str("~~");
                s
            }
            TextStyle::Code => {
                let mut s = String::from("`");
                s.push_str(&self.content);
                s.push('`');
                s
            }
        };

        result.push_str(&styled);
        result
    }

    pub fn to_plain_text(&self) -> String {
        let mut result = String::new();

        // Add indent
        for _ in 0..self.indent_level {
            result.push_str("    ");
        }

        // Add list prefix
        result.push_str(&self.list_type.prefix(1));

        result.push_str(&self.content);
        result
    }
}

/// Attachment in a note
#[derive(Debug, Clone)]
pub struct NoteAttachment {
    pub id: u64,
    pub filename: String,
    pub mime_type: String,
    pub size: u64,
    pub data: Vec<u8>,
}

impl NoteAttachment {
    pub fn format_size(&self) -> String {
        if self.size < 1024 {
            let mut s = self.size.to_string();
            s.push_str(" B");
            s
        } else if self.size < 1024 * 1024 {
            let mut s = (self.size / 1024).to_string();
            s.push_str(" KB");
            s
        } else {
            let mut s = (self.size / (1024 * 1024)).to_string();
            s.push_str(" MB");
            s
        }
    }
}

/// Note color/label
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteColor {
    None,
    Red,
    Orange,
    Yellow,
    Green,
    Blue,
    Purple,
    Pink,
    Gray,
}

impl NoteColor {
    pub fn to_color(&self) -> Option<Color> {
        match self {
            NoteColor::None => None,
            NoteColor::Red => Some(Color::new(255, 99, 71)),
            NoteColor::Orange => Some(Color::new(255, 165, 0)),
            NoteColor::Yellow => Some(Color::new(255, 215, 0)),
            NoteColor::Green => Some(Color::new(50, 205, 50)),
            NoteColor::Blue => Some(Color::new(100, 149, 237)),
            NoteColor::Purple => Some(Color::new(138, 43, 226)),
            NoteColor::Pink => Some(Color::new(255, 105, 180)),
            NoteColor::Gray => Some(Color::new(128, 128, 128)),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            NoteColor::None => "None",
            NoteColor::Red => "Red",
            NoteColor::Orange => "Orange",
            NoteColor::Yellow => "Yellow",
            NoteColor::Green => "Green",
            NoteColor::Blue => "Blue",
            NoteColor::Purple => "Purple",
            NoteColor::Pink => "Pink",
            NoteColor::Gray => "Gray",
        }
    }

    pub fn all() -> &'static [NoteColor] {
        &[
            NoteColor::None,
            NoteColor::Red,
            NoteColor::Orange,
            NoteColor::Yellow,
            NoteColor::Green,
            NoteColor::Blue,
            NoteColor::Purple,
            NoteColor::Pink,
            NoteColor::Gray,
        ]
    }
}

/// A single note
#[derive(Debug, Clone)]
pub struct Note {
    pub id: u64,
    pub notebook_id: u64,
    pub title: String,
    pub blocks: Vec<TextBlock>,
    pub tags: Vec<String>,
    pub color: NoteColor,
    pub is_pinned: bool,
    pub is_locked: bool,
    pub is_archived: bool,
    pub is_trashed: bool,
    pub attachments: Vec<NoteAttachment>,
    pub created: u64,
    pub modified: u64,
    pub word_count: usize,
    pub character_count: usize,
}

impl Note {
    pub fn new(id: u64, notebook_id: u64, title: &str) -> Self {
        Self {
            id,
            notebook_id,
            title: title.to_string(),
            blocks: Vec::new(),
            tags: Vec::new(),
            color: NoteColor::None,
            is_pinned: false,
            is_locked: false,
            is_archived: false,
            is_trashed: false,
            attachments: Vec::new(),
            created: 0,
            modified: 0,
            word_count: 0,
            character_count: 0,
        }
    }

    pub fn plain_text(&self) -> String {
        let mut result = self.title.clone();
        result.push_str("\n\n");

        for block in &self.blocks {
            result.push_str(&block.to_plain_text());
            result.push('\n');
        }

        result
    }

    pub fn markdown(&self) -> String {
        let mut result = String::from("# ");
        result.push_str(&self.title);
        result.push_str("\n\n");

        for block in &self.blocks {
            result.push_str(&block.to_markdown());
            result.push('\n');
        }

        result
    }

    pub fn html(&self) -> String {
        let mut result = String::from("<!DOCTYPE html>\n<html>\n<head><title>");
        result.push_str(&self.title);
        result.push_str("</title></head>\n<body>\n<h1>");
        result.push_str(&self.title);
        result.push_str("</h1>\n");

        for block in &self.blocks {
            let tag = match block.heading {
                HeadingLevel::None => "p",
                HeadingLevel::H1 => "h1",
                HeadingLevel::H2 => "h2",
                HeadingLevel::H3 => "h3",
            };

            result.push('<');
            result.push_str(tag);
            result.push('>');

            match block.list_type {
                ListType::Checkbox => result.push_str("<input type=\"checkbox\">"),
                ListType::CheckboxChecked => result.push_str("<input type=\"checkbox\" checked>"),
                _ => {}
            }

            let styled = match block.style {
                TextStyle::Normal => block.content.clone(),
                TextStyle::Bold => {
                    let mut s = String::from("<strong>");
                    s.push_str(&block.content);
                    s.push_str("</strong>");
                    s
                }
                TextStyle::Italic => {
                    let mut s = String::from("<em>");
                    s.push_str(&block.content);
                    s.push_str("</em>");
                    s
                }
                TextStyle::BoldItalic => {
                    let mut s = String::from("<strong><em>");
                    s.push_str(&block.content);
                    s.push_str("</em></strong>");
                    s
                }
                TextStyle::Strikethrough => {
                    let mut s = String::from("<del>");
                    s.push_str(&block.content);
                    s.push_str("</del>");
                    s
                }
                TextStyle::Code => {
                    let mut s = String::from("<code>");
                    s.push_str(&block.content);
                    s.push_str("</code>");
                    s
                }
            };

            result.push_str(&styled);
            result.push_str("</");
            result.push_str(tag);
            result.push_str(">\n");
        }

        result.push_str("</body>\n</html>");
        result
    }

    pub fn preview(&self, max_chars: usize) -> String {
        let text = self.plain_text();
        if text.len() <= max_chars {
            text
        } else {
            let mut preview: String = text.chars().take(max_chars).collect();
            preview.push_str("...");
            preview
        }
    }

    pub fn update_stats(&mut self) {
        let text = self.plain_text();
        self.character_count = text.len();
        self.word_count = text.split_whitespace().count();
    }

    pub fn add_tag(&mut self, tag: &str) {
        let tag_str = tag.to_string();
        if !self.tags.contains(&tag_str) {
            self.tags.push(tag_str);
        }
    }

    pub fn remove_tag(&mut self, tag: &str) {
        self.tags.retain(|t| t != tag);
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn search_text(&self) -> String {
        let mut text = self.title.to_lowercase();
        text.push(' ');
        for block in &self.blocks {
            text.push_str(&block.content.to_lowercase());
            text.push(' ');
        }
        for tag in &self.tags {
            text.push_str(&tag.to_lowercase());
            text.push(' ');
        }
        text
    }

    pub fn format_date(&self, timestamp: u64) -> String {
        // Simple date formatting (would use real time in production)
        let days = timestamp / (24 * 60 * 60);
        let mut s = String::from("Day ");
        s.push_str(&days.to_string());
        s
    }
}

/// Notebook/folder for organizing notes
#[derive(Debug, Clone)]
pub struct Notebook {
    pub id: u64,
    pub name: String,
    pub color: NoteColor,
    pub icon: char,
    pub is_default: bool,
    pub is_locked: bool,
    pub parent_id: Option<u64>,
    pub note_count: usize,
    pub created: u64,
    pub modified: u64,
}

impl Notebook {
    pub fn new(id: u64, name: &str) -> Self {
        Self {
            id,
            name: name.to_string(),
            color: NoteColor::None,
            icon: '📓',
            is_default: false,
            is_locked: false,
            parent_id: None,
            note_count: 0,
            created: 0,
            modified: 0,
        }
    }
}

/// Tag with usage count
#[derive(Debug, Clone)]
pub struct Tag {
    pub name: String,
    pub color: NoteColor,
    pub usage_count: usize,
}

impl Tag {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            color: NoteColor::None,
            usage_count: 0,
        }
    }
}

/// Sort order for notes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    ModifiedDesc,
    ModifiedAsc,
    CreatedDesc,
    CreatedAsc,
    TitleAsc,
    TitleDesc,
}

impl SortOrder {
    pub fn name(&self) -> &'static str {
        match self {
            SortOrder::ModifiedDesc => "Modified (Newest)",
            SortOrder::ModifiedAsc => "Modified (Oldest)",
            SortOrder::CreatedDesc => "Created (Newest)",
            SortOrder::CreatedAsc => "Created (Oldest)",
            SortOrder::TitleAsc => "Title A-Z",
            SortOrder::TitleDesc => "Title Z-A",
        }
    }
}

/// View mode for the notes app
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    NoteList,
    NoteView,
    NoteEdit,
    NotebookList,
    TagList,
    Search,
    Settings,
}

/// Filter type for notes
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterType {
    All,
    Notebook(u64),
    Tag(String),
    Pinned,
    Archived,
    Trash,
    Recent,
}

/// Export format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    PlainText,
    Markdown,
    Html,
}

impl ExportFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            ExportFormat::PlainText => "txt",
            ExportFormat::Markdown => "md",
            ExportFormat::Html => "html",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            ExportFormat::PlainText => "Plain Text",
            ExportFormat::Markdown => "Markdown",
            ExportFormat::Html => "HTML",
        }
    }
}

// Helper functions for rendering
fn draw_char_at(surface: &mut Surface, x: usize, y: usize, c: char, color: Color) {
    use crate::drivers::font::DEFAULT_FONT;
    if let Some(glyph) = DEFAULT_FONT.get_glyph(c) {
        for row in 0..DEFAULT_FONT.height {
            let byte = glyph[row];
            for col in 0..DEFAULT_FONT.width {
                if (byte >> (7 - col)) & 1 != 0 {
                    surface.set_pixel(x + col, y + row, color);
                }
            }
        }
    }
}

fn draw_char(surface: &mut Surface, x: isize, y: isize, c: char, color: Color) {
    if x >= 0 && y >= 0 {
        draw_char_at(surface, x as usize, y as usize, c, color);
    }
}

fn draw_string(surface: &mut Surface, x: isize, y: isize, s: &str, color: Color) {
    if x < 0 || y < 0 { return; }
    let mut px = x as usize;
    for c in s.chars() {
        draw_char_at(surface, px, y as usize, c, color);
        px += 8;
    }
}

/// Notes application widget
pub struct NotesApp {
    id: WidgetId,
    bounds: Bounds,
    enabled: bool,
    visible: bool,

    // Data
    notes: Vec<Note>,
    notebooks: Vec<Notebook>,
    tags: Vec<Tag>,
    next_note_id: u64,
    next_notebook_id: u64,
    next_attachment_id: u64,

    // View state
    view_mode: ViewMode,
    filter: FilterType,
    sort_order: SortOrder,
    search_query: String,
    selected_note_id: Option<u64>,
    selected_notebook_id: Option<u64>,

    // UI state
    sidebar_width: usize,
    scroll_offset: usize,
    hovered_index: Option<usize>,
    editing_block: usize,
    cursor_position: usize,
    current_style: TextStyle,
    current_list: ListType,
    show_formatting_bar: bool,
}

impl NotesApp {
    pub fn new(id: WidgetId) -> Self {
        let mut app = Self {
            id,
            bounds: Bounds { x: 0, y: 0, width: 900, height: 650 },
            enabled: true,
            visible: true,
            notes: Vec::new(),
            notebooks: Vec::new(),
            tags: Vec::new(),
            next_note_id: 1,
            next_notebook_id: 1,
            next_attachment_id: 1,
            view_mode: ViewMode::NoteList,
            filter: FilterType::All,
            sort_order: SortOrder::ModifiedDesc,
            search_query: String::new(),
            selected_note_id: None,
            selected_notebook_id: None,
            sidebar_width: 200,
            scroll_offset: 0,
            hovered_index: None,
            editing_block: 0,
            cursor_position: 0,
            current_style: TextStyle::Normal,
            current_list: ListType::None,
            show_formatting_bar: true,
        };

        app.add_sample_data();
        app
    }

    fn add_sample_data(&mut self) {
        // Create default notebooks
        let mut personal = Notebook::new(self.next_notebook_id, "Personal");
        personal.is_default = true;
        personal.icon = '📓';
        self.notebooks.push(personal);
        self.next_notebook_id += 1;

        let mut work = Notebook::new(self.next_notebook_id, "Work");
        work.icon = '💼';
        self.notebooks.push(work);
        self.next_notebook_id += 1;

        let mut ideas = Notebook::new(self.next_notebook_id, "Ideas");
        ideas.icon = '💡';
        ideas.color = NoteColor::Yellow;
        self.notebooks.push(ideas);
        self.next_notebook_id += 1;

        // Create sample notes
        let mut note1 = Note::new(self.next_note_id, 1, "Welcome to Notes");
        note1.blocks.push(TextBlock::with_heading("Getting Started", HeadingLevel::H2));
        note1.blocks.push(TextBlock::new("Welcome to the Notes app! Here you can create and organize your notes."));
        note1.blocks.push(TextBlock::new(""));
        note1.blocks.push(TextBlock::with_heading("Features", HeadingLevel::H2));
        note1.blocks.push(TextBlock::with_list("Rich text formatting (bold, italic, etc.)", ListType::Bullet));
        note1.blocks.push(TextBlock::with_list("Notebooks for organization", ListType::Bullet));
        note1.blocks.push(TextBlock::with_list("Tags for quick filtering", ListType::Bullet));
        note1.blocks.push(TextBlock::with_list("Search across all notes", ListType::Bullet));
        note1.blocks.push(TextBlock::with_list("Pin important notes", ListType::Bullet));
        note1.blocks.push(TextBlock::with_list("Export to multiple formats", ListType::Bullet));
        note1.is_pinned = true;
        note1.tags.push("welcome".to_string());
        note1.tags.push("tutorial".to_string());
        note1.update_stats();
        self.notes.push(note1);
        self.next_note_id += 1;

        let mut note2 = Note::new(self.next_note_id, 1, "Meeting Notes");
        note2.blocks.push(TextBlock::new("Meeting Date: January 18, 2026"));
        note2.blocks.push(TextBlock::new("Attendees: Team members"));
        note2.blocks.push(TextBlock::new(""));
        note2.blocks.push(TextBlock::with_heading("Agenda", HeadingLevel::H2));
        note2.blocks.push(TextBlock::with_list("Review project status", ListType::Numbered));
        note2.blocks.push(TextBlock::with_list("Discuss blockers", ListType::Numbered));
        note2.blocks.push(TextBlock::with_list("Plan next sprint", ListType::Numbered));
        note2.blocks.push(TextBlock::new(""));
        note2.blocks.push(TextBlock::with_heading("Action Items", HeadingLevel::H2));
        note2.blocks.push(TextBlock::with_list("Complete documentation", ListType::CheckboxChecked));
        note2.blocks.push(TextBlock::with_list("Review PRs", ListType::Checkbox));
        note2.blocks.push(TextBlock::with_list("Update roadmap", ListType::Checkbox));
        note2.tags.push("meeting".to_string());
        note2.tags.push("work".to_string());
        note2.update_stats();
        self.notes.push(note2);
        self.next_note_id += 1;

        let mut note3 = Note::new(self.next_note_id, 2, "Project Ideas");
        note3.blocks.push(TextBlock::new("Collection of project ideas to explore:"));
        note3.blocks.push(TextBlock::new(""));
        note3.blocks.push(TextBlock::with_list("Build a custom OS (in progress!)", ListType::Bullet));
        note3.blocks.push(TextBlock::with_list("Create a programming language", ListType::Bullet));
        note3.blocks.push(TextBlock::with_list("Design a game engine", ListType::Bullet));
        note3.blocks.push(TextBlock::with_list("Develop an AI assistant", ListType::Bullet));
        note3.notebook_id = 3;
        note3.color = NoteColor::Yellow;
        note3.tags.push("ideas".to_string());
        note3.tags.push("projects".to_string());
        note3.update_stats();
        self.notes.push(note3);
        self.next_note_id += 1;

        let mut note4 = Note::new(self.next_note_id, 2, "Shopping List");
        note4.blocks.push(TextBlock::with_list("Milk", ListType::Checkbox));
        note4.blocks.push(TextBlock::with_list("Bread", ListType::CheckboxChecked));
        note4.blocks.push(TextBlock::with_list("Eggs", ListType::Checkbox));
        note4.blocks.push(TextBlock::with_list("Coffee", ListType::Checkbox));
        note4.blocks.push(TextBlock::with_list("Fruits", ListType::CheckboxChecked));
        note4.color = NoteColor::Green;
        note4.tags.push("shopping".to_string());
        note4.tags.push("personal".to_string());
        note4.update_stats();
        self.notes.push(note4);
        self.next_note_id += 1;

        // Create sample tags
        self.tags.push(Tag { name: "welcome".to_string(), color: NoteColor::Blue, usage_count: 1 });
        self.tags.push(Tag { name: "tutorial".to_string(), color: NoteColor::Blue, usage_count: 1 });
        self.tags.push(Tag { name: "meeting".to_string(), color: NoteColor::Orange, usage_count: 1 });
        self.tags.push(Tag { name: "work".to_string(), color: NoteColor::Red, usage_count: 1 });
        self.tags.push(Tag { name: "ideas".to_string(), color: NoteColor::Yellow, usage_count: 1 });
        self.tags.push(Tag { name: "projects".to_string(), color: NoteColor::Purple, usage_count: 1 });
        self.tags.push(Tag { name: "shopping".to_string(), color: NoteColor::Green, usage_count: 1 });
        self.tags.push(Tag { name: "personal".to_string(), color: NoteColor::Pink, usage_count: 1 });

        // Update notebook note counts
        self.update_notebook_counts();
    }

    fn update_notebook_counts(&mut self) {
        for notebook in &mut self.notebooks {
            notebook.note_count = self.notes.iter()
                .filter(|n| n.notebook_id == notebook.id && !n.is_trashed)
                .count();
        }
    }

    // Note management
    pub fn create_note(&mut self, notebook_id: u64, title: &str) -> u64 {
        let note = Note::new(self.next_note_id, notebook_id, title);
        let id = note.id;
        self.notes.push(note);
        self.next_note_id += 1;
        self.update_notebook_counts();
        self.selected_note_id = Some(id);
        self.view_mode = ViewMode::NoteEdit;
        id
    }

    pub fn delete_note(&mut self, note_id: u64) {
        if let Some(note) = self.notes.iter_mut().find(|n| n.id == note_id) {
            if note.is_trashed {
                // Permanently delete
                self.notes.retain(|n| n.id != note_id);
            } else {
                // Move to trash
                note.is_trashed = true;
            }
        }
        self.update_notebook_counts();

        if self.selected_note_id == Some(note_id) {
            self.selected_note_id = None;
            self.view_mode = ViewMode::NoteList;
        }
    }

    pub fn restore_note(&mut self, note_id: u64) {
        if let Some(note) = self.notes.iter_mut().find(|n| n.id == note_id) {
            note.is_trashed = false;
        }
        self.update_notebook_counts();
    }

    pub fn archive_note(&mut self, note_id: u64) {
        if let Some(note) = self.notes.iter_mut().find(|n| n.id == note_id) {
            note.is_archived = !note.is_archived;
        }
        self.update_notebook_counts();
    }

    pub fn pin_note(&mut self, note_id: u64) {
        if let Some(note) = self.notes.iter_mut().find(|n| n.id == note_id) {
            note.is_pinned = !note.is_pinned;
        }
    }

    pub fn move_note(&mut self, note_id: u64, notebook_id: u64) {
        if let Some(note) = self.notes.iter_mut().find(|n| n.id == note_id) {
            note.notebook_id = notebook_id;
        }
        self.update_notebook_counts();
    }

    pub fn get_note(&self, note_id: u64) -> Option<&Note> {
        self.notes.iter().find(|n| n.id == note_id)
    }

    pub fn get_note_mut(&mut self, note_id: u64) -> Option<&mut Note> {
        self.notes.iter_mut().find(|n| n.id == note_id)
    }

    // Notebook management
    pub fn create_notebook(&mut self, name: &str) -> u64 {
        let notebook = Notebook::new(self.next_notebook_id, name);
        let id = notebook.id;
        self.notebooks.push(notebook);
        self.next_notebook_id += 1;
        id
    }

    pub fn delete_notebook(&mut self, notebook_id: u64) {
        // Don't delete the default notebook
        if self.notebooks.iter().any(|n| n.id == notebook_id && n.is_default) {
            return;
        }

        // Move notes to default notebook
        let default_id = self.notebooks.iter()
            .find(|n| n.is_default)
            .map(|n| n.id)
            .unwrap_or(1);

        for note in &mut self.notes {
            if note.notebook_id == notebook_id {
                note.notebook_id = default_id;
            }
        }

        self.notebooks.retain(|n| n.id != notebook_id);
        self.update_notebook_counts();
    }

    // Tag management
    pub fn add_tag(&mut self, name: &str) {
        if !self.tags.iter().any(|t| t.name == name) {
            self.tags.push(Tag::new(name));
        }
    }

    pub fn remove_tag(&mut self, name: &str) {
        self.tags.retain(|t| t.name != name);
        for note in &mut self.notes {
            note.remove_tag(name);
        }
    }

    // Filtering and sorting
    pub fn filtered_notes(&self) -> Vec<&Note> {
        let mut filtered: Vec<&Note> = self.notes.iter()
            .filter(|note| {
                // Apply filter
                let passes_filter = match &self.filter {
                    FilterType::All => !note.is_trashed && !note.is_archived,
                    FilterType::Notebook(id) => note.notebook_id == *id && !note.is_trashed && !note.is_archived,
                    FilterType::Tag(tag) => note.has_tag(tag) && !note.is_trashed && !note.is_archived,
                    FilterType::Pinned => note.is_pinned && !note.is_trashed && !note.is_archived,
                    FilterType::Archived => note.is_archived && !note.is_trashed,
                    FilterType::Trash => note.is_trashed,
                    FilterType::Recent => !note.is_trashed && !note.is_archived,
                };

                // Apply search
                let passes_search = self.search_query.is_empty() ||
                    note.search_text().contains(&self.search_query.to_lowercase());

                passes_filter && passes_search
            })
            .collect();

        // Sort notes (pinned first for non-trash views)
        filtered.sort_by(|a, b| {
            // Pinned notes first (except in trash view)
            if !matches!(self.filter, FilterType::Trash) {
                if a.is_pinned != b.is_pinned {
                    return b.is_pinned.cmp(&a.is_pinned);
                }
            }

            // Then by selected sort order
            match self.sort_order {
                SortOrder::ModifiedDesc => b.modified.cmp(&a.modified),
                SortOrder::ModifiedAsc => a.modified.cmp(&b.modified),
                SortOrder::CreatedDesc => b.created.cmp(&a.created),
                SortOrder::CreatedAsc => a.created.cmp(&b.created),
                SortOrder::TitleAsc => a.title.cmp(&b.title),
                SortOrder::TitleDesc => b.title.cmp(&a.title),
            }
        });

        filtered
    }

    pub fn set_filter(&mut self, filter: FilterType) {
        self.filter = filter;
        self.scroll_offset = 0;
        self.selected_note_id = None;
    }

    pub fn set_sort_order(&mut self, order: SortOrder) {
        self.sort_order = order;
    }

    pub fn set_search_query(&mut self, query: &str) {
        self.search_query = query.to_string();
        self.scroll_offset = 0;
    }

    // Export
    pub fn export_note(&self, note_id: u64, format: ExportFormat) -> Option<String> {
        self.get_note(note_id).map(|note| {
            match format {
                ExportFormat::PlainText => note.plain_text(),
                ExportFormat::Markdown => note.markdown(),
                ExportFormat::Html => note.html(),
            }
        })
    }

    // UI helpers
    fn note_at_point(&self, x: isize, y: isize) -> Option<usize> {
        let list_x = self.bounds.x + self.sidebar_width as isize;
        let list_y = self.bounds.y + 50;
        let list_width = 280isize;
        let item_height = 80isize;

        if x < list_x || x >= list_x + list_width {
            return None;
        }

        if y < list_y || y >= self.bounds.y + self.bounds.height as isize {
            return None;
        }

        let rel_y = y - list_y;
        let index = (rel_y / item_height) as usize + self.scroll_offset;

        let notes = self.filtered_notes();
        if index < notes.len() {
            Some(index)
        } else {
            None
        }
    }

    fn sidebar_item_at_point(&self, x: isize, y: isize) -> Option<FilterType> {
        if x < self.bounds.x || x >= self.bounds.x + self.sidebar_width as isize {
            return None;
        }

        let item_height = 32isize;
        let rel_y = y - self.bounds.y - 50;

        if rel_y < 0 {
            return None;
        }

        let index = (rel_y / item_height) as usize;

        // Items: All Notes, Pinned, then notebooks, then tags section, then Archive, Trash
        let mut current = 0;

        if index == current { return Some(FilterType::All); }
        current += 1;

        if index == current { return Some(FilterType::Pinned); }
        current += 1;

        // Notebooks
        for notebook in &self.notebooks {
            if index == current {
                return Some(FilterType::Notebook(notebook.id));
            }
            current += 1;
        }

        // Skip tag header
        current += 1;

        // Tags
        for tag in &self.tags {
            if index == current {
                return Some(FilterType::Tag(tag.name.clone()));
            }
            current += 1;
        }

        // Archive and Trash
        if index == current { return Some(FilterType::Archived); }
        current += 1;

        if index == current { return Some(FilterType::Trash); }

        None
    }

    fn get_visible_count(&self) -> usize {
        let available_height = self.bounds.height.saturating_sub(50);
        available_height / 80
    }

    fn select_note(&mut self, note_id: u64) {
        self.selected_note_id = Some(note_id);
        self.view_mode = ViewMode::NoteView;
        self.editing_block = 0;
        self.cursor_position = 0;
    }

    fn edit_selected_note(&mut self) {
        if self.selected_note_id.is_some() {
            self.view_mode = ViewMode::NoteEdit;
        }
    }
}

impl Widget for NotesApp {
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
        match event {
            WidgetEvent::MouseDown { x, y, button } => {
                if *button == MouseButton::Left {
                    // Check sidebar
                    if let Some(filter) = self.sidebar_item_at_point(*x, *y) {
                        self.set_filter(filter);
                        self.view_mode = ViewMode::NoteList;
                        return true;
                    }

                    // Check note list
                    if let Some(idx) = self.note_at_point(*x, *y) {
                        let notes = self.filtered_notes();
                        if idx < notes.len() {
                            let note_id = notes[idx].id;
                            self.select_note(note_id);
                            return true;
                        }
                    }

                    // Check toolbar buttons
                    let toolbar_y = self.bounds.y;
                    let content_x = self.bounds.x + self.sidebar_width as isize + 290;

                    if *y >= toolbar_y && *y < toolbar_y + 40 {
                        // New note button
                        if *x >= self.bounds.x + self.sidebar_width as isize && *x < self.bounds.x + self.sidebar_width as isize + 80 {
                            let notebook_id = match &self.filter {
                                FilterType::Notebook(id) => *id,
                                _ => self.notebooks.iter().find(|n| n.is_default).map(|n| n.id).unwrap_or(1),
                            };
                            self.create_note(notebook_id, "Untitled Note");
                            return true;
                        }

                        // Edit button (if viewing a note)
                        if self.view_mode == ViewMode::NoteView && self.selected_note_id.is_some() {
                            if *x >= content_x && *x < content_x + 60 {
                                self.edit_selected_note();
                                return true;
                            }
                        }
                    }
                }
                false
            }

            WidgetEvent::MouseMove { x, y } => {
                self.hovered_index = self.note_at_point(*x, *y);
                true
            }

            WidgetEvent::Scroll { delta_y, .. } => {
                let notes = self.filtered_notes();
                let visible = self.get_visible_count();
                let max_scroll = notes.len().saturating_sub(visible);

                if *delta_y < 0 && self.scroll_offset > 0 {
                    self.scroll_offset = self.scroll_offset.saturating_sub(1);
                } else if *delta_y > 0 && self.scroll_offset < max_scroll {
                    self.scroll_offset += 1;
                }
                true
            }

            WidgetEvent::KeyDown { key, .. } => {
                match *key {
                    0x48 => { // Up
                        let notes = self.filtered_notes();
                        if let Some(id) = self.selected_note_id {
                            if let Some(pos) = notes.iter().position(|n| n.id == id) {
                                if pos > 0 {
                                    self.select_note(notes[pos - 1].id);
                                    if pos.saturating_sub(1) < self.scroll_offset {
                                        self.scroll_offset = pos.saturating_sub(1);
                                    }
                                }
                            }
                        } else if !notes.is_empty() {
                            self.select_note(notes[0].id);
                        }
                        true
                    }
                    0x50 => { // Down
                        let notes = self.filtered_notes();
                        if let Some(id) = self.selected_note_id {
                            if let Some(pos) = notes.iter().position(|n| n.id == id) {
                                if pos + 1 < notes.len() {
                                    self.select_note(notes[pos + 1].id);
                                    let visible = self.get_visible_count();
                                    if pos + 1 >= self.scroll_offset + visible {
                                        self.scroll_offset = (pos + 2).saturating_sub(visible);
                                    }
                                }
                            }
                        } else if !notes.is_empty() {
                            self.select_note(notes[0].id);
                        }
                        true
                    }
                    0x1C => { // Enter - edit note
                        if self.view_mode == ViewMode::NoteView {
                            self.edit_selected_note();
                        }
                        true
                    }
                    0x1B => { // Escape - back to list
                        if self.view_mode == ViewMode::NoteEdit {
                            self.view_mode = ViewMode::NoteView;
                        } else if self.view_mode == ViewMode::NoteView {
                            self.view_mode = ViewMode::NoteList;
                            self.selected_note_id = None;
                        }
                        true
                    }
                    0x53 | 0x7F => { // Delete
                        if let Some(id) = self.selected_note_id {
                            self.delete_note(id);
                        }
                        true
                    }
                    _ => false,
                }
            }

            _ => false,
        }
    }

    fn render(&self, surface: &mut Surface) {
        let bg = Color::new(30, 30, 35);
        let sidebar_bg = Color::new(25, 25, 30);
        let list_bg = Color::new(35, 35, 40);
        let content_bg = Color::new(40, 40, 45);
        let border_color = Color::new(60, 60, 65);
        let text_color = Color::new(230, 230, 230);
        let dim_text = Color::new(150, 150, 155);
        let accent_color = Color::new(255, 200, 100);
        let selected_bg = Color::new(60, 60, 70);
        let hover_bg = Color::new(50, 50, 55);

        // Background
        for y in 0..self.bounds.height {
            for x in 0..self.bounds.width {
                surface.set_pixel(
                    (self.bounds.x as usize) + x,
                    (self.bounds.y as usize) + y,
                    bg
                );
            }
        }

        // Sidebar
        for y in 0..self.bounds.height {
            for x in 0..self.sidebar_width {
                surface.set_pixel(
                    (self.bounds.x as usize) + x,
                    (self.bounds.y as usize) + y,
                    sidebar_bg
                );
            }
        }

        // Sidebar title
        draw_string(surface, self.bounds.x + 15, self.bounds.y + 15, "Notes", accent_color);

        // Sidebar items
        let mut sidebar_y = self.bounds.y + 50;
        let item_height = 32isize;

        // All Notes
        let is_selected = matches!(self.filter, FilterType::All);
        if is_selected {
            for y in 0..item_height as usize {
                for x in 0..self.sidebar_width {
                    surface.set_pixel(
                        (self.bounds.x as usize) + x,
                        (sidebar_y as usize) + y,
                        selected_bg
                    );
                }
            }
        }
        draw_string(surface, self.bounds.x + 15, sidebar_y + 8, "All Notes", if is_selected { accent_color } else { text_color });
        sidebar_y += item_height;

        // Pinned
        let is_selected = matches!(self.filter, FilterType::Pinned);
        if is_selected {
            for y in 0..item_height as usize {
                for x in 0..self.sidebar_width {
                    surface.set_pixel(
                        (self.bounds.x as usize) + x,
                        (sidebar_y as usize) + y,
                        selected_bg
                    );
                }
            }
        }
        draw_string(surface, self.bounds.x + 15, sidebar_y + 8, "Pinned", if is_selected { accent_color } else { text_color });
        sidebar_y += item_height;

        // Notebooks section
        draw_string(surface, self.bounds.x + 15, sidebar_y + 8, "NOTEBOOKS", dim_text);
        sidebar_y += item_height;

        for notebook in &self.notebooks {
            let is_selected = matches!(&self.filter, FilterType::Notebook(id) if *id == notebook.id);
            if is_selected {
                for y in 0..item_height as usize {
                    for x in 0..self.sidebar_width {
                        surface.set_pixel(
                            (self.bounds.x as usize) + x,
                            (sidebar_y as usize) + y,
                            selected_bg
                        );
                    }
                }
            }

            let icon_str: String = notebook.icon.to_string();
            draw_string(surface, self.bounds.x + 15, sidebar_y + 8, &icon_str, text_color);
            draw_string(surface, self.bounds.x + 30, sidebar_y + 8, &notebook.name, if is_selected { accent_color } else { text_color });

            let count_str = notebook.note_count.to_string();
            draw_string(surface, self.bounds.x + self.sidebar_width as isize - 30, sidebar_y + 8, &count_str, dim_text);

            sidebar_y += item_height;
        }

        // Tags section
        draw_string(surface, self.bounds.x + 15, sidebar_y + 8, "TAGS", dim_text);
        sidebar_y += item_height;

        for tag in self.tags.iter().take(5) {
            let is_selected = matches!(&self.filter, FilterType::Tag(t) if t == &tag.name);
            if is_selected {
                for y in 0..item_height as usize {
                    for x in 0..self.sidebar_width {
                        surface.set_pixel(
                            (self.bounds.x as usize) + x,
                            (sidebar_y as usize) + y,
                            selected_bg
                        );
                    }
                }
            }

            let tag_name = String::from("#") + &tag.name;
            draw_string(surface, self.bounds.x + 15, sidebar_y + 8, &tag_name,
                if is_selected { accent_color } else if let Some(c) = tag.color.to_color() { c } else { text_color });
            sidebar_y += item_height;
        }

        sidebar_y += item_height; // Spacer

        // Archive
        let is_selected = matches!(self.filter, FilterType::Archived);
        if is_selected {
            for y in 0..item_height as usize {
                for x in 0..self.sidebar_width {
                    surface.set_pixel(
                        (self.bounds.x as usize) + x,
                        (sidebar_y as usize) + y,
                        selected_bg
                    );
                }
            }
        }
        draw_string(surface, self.bounds.x + 15, sidebar_y + 8, "Archive", if is_selected { accent_color } else { dim_text });
        sidebar_y += item_height;

        // Trash
        let is_selected = matches!(self.filter, FilterType::Trash);
        if is_selected {
            for y in 0..item_height as usize {
                for x in 0..self.sidebar_width {
                    surface.set_pixel(
                        (self.bounds.x as usize) + x,
                        (sidebar_y as usize) + y,
                        selected_bg
                    );
                }
            }
        }
        draw_string(surface, self.bounds.x + 15, sidebar_y + 8, "Trash", if is_selected { accent_color } else { dim_text });

        // Sidebar border
        for y in 0..self.bounds.height {
            surface.set_pixel(
                (self.bounds.x as usize) + self.sidebar_width,
                (self.bounds.y as usize) + y,
                border_color
            );
        }

        // Note list
        let list_x = self.bounds.x + self.sidebar_width as isize + 1;
        let list_width = 280usize;

        // List background
        for y in 0..self.bounds.height {
            for x in 0..list_width {
                surface.set_pixel(
                    (list_x as usize) + x,
                    (self.bounds.y as usize) + y,
                    list_bg
                );
            }
        }

        // Toolbar
        draw_string(surface, list_x + 10, self.bounds.y + 15, "+ New", accent_color);

        // Note list header
        let filter_name = match &self.filter {
            FilterType::All => "All Notes",
            FilterType::Notebook(id) => {
                self.notebooks.iter()
                    .find(|n| n.id == *id)
                    .map(|n| n.name.as_str())
                    .unwrap_or("Notebook")
            }
            FilterType::Tag(t) => t.as_str(),
            FilterType::Pinned => "Pinned",
            FilterType::Archived => "Archive",
            FilterType::Trash => "Trash",
            FilterType::Recent => "Recent",
        };
        draw_string(surface, list_x + 10, self.bounds.y + 55, filter_name, text_color);

        // Note items
        let notes = self.filtered_notes();
        let item_height = 80isize;
        let visible_count = self.get_visible_count();
        let mut list_y = self.bounds.y + 80;

        for (i, note) in notes.iter().skip(self.scroll_offset).take(visible_count).enumerate() {
            let is_selected = self.selected_note_id == Some(note.id);
            let is_hovered = self.hovered_index == Some(i + self.scroll_offset);

            // Item background
            let item_bg = if is_selected { selected_bg } else if is_hovered { hover_bg } else { list_bg };
            for y in 0..item_height as usize - 1 {
                for x in 0..list_width {
                    surface.set_pixel(
                        (list_x as usize) + x,
                        (list_y as usize) + y,
                        item_bg
                    );
                }
            }

            // Pin indicator
            if note.is_pinned {
                draw_char(surface, list_x + 5, list_y + 10, '*', accent_color);
            }

            // Note title
            let title_color = if is_selected { accent_color } else { text_color };
            let title = if note.title.len() > 30 {
                let mut t: String = note.title.chars().take(27).collect();
                t.push_str("...");
                t
            } else {
                note.title.clone()
            };
            draw_string(surface, list_x + 15, list_y + 10, &title, title_color);

            // Preview
            let preview = note.preview(35);
            let preview_lines: Vec<&str> = preview.lines().take(2).collect();
            let preview_text = if !preview_lines.is_empty() {
                let line = preview_lines[0];
                if line.len() > 35 {
                    let mut p: String = line.chars().take(32).collect();
                    p.push_str("...");
                    p
                } else {
                    line.to_string()
                }
            } else {
                String::from("No content")
            };
            draw_string(surface, list_x + 15, list_y + 30, &preview_text, dim_text);

            // Tags
            if !note.tags.is_empty() {
                let tag_preview = format!("#{}", note.tags[0]);
                draw_string(surface, list_x + 15, list_y + 50, &tag_preview,
                    Color::new(100, 149, 237));
            }

            // Color indicator
            if let Some(color) = note.color.to_color() {
                for cy in 0..item_height as usize - 2 {
                    surface.set_pixel(
                        (list_x as usize) + list_width - 4,
                        (list_y as usize) + cy,
                        color
                    );
                }
            }

            // Item separator
            for x in 0..list_width {
                surface.set_pixel(
                    (list_x as usize) + x,
                    (list_y + item_height - 1) as usize,
                    border_color
                );
            }

            list_y += item_height;
        }

        // List border
        for y in 0..self.bounds.height {
            surface.set_pixel(
                (list_x as usize) + list_width,
                (self.bounds.y as usize) + y,
                border_color
            );
        }

        // Content area
        let content_x = list_x + list_width as isize + 1;
        let content_width = self.bounds.width - self.sidebar_width - list_width - 2;

        // Content background
        for y in 0..self.bounds.height {
            for x in 0..content_width {
                surface.set_pixel(
                    (content_x as usize) + x,
                    (self.bounds.y as usize) + y,
                    content_bg
                );
            }
        }

        // Render note content or empty state
        if let Some(note_id) = self.selected_note_id {
            if let Some(note) = self.notes.iter().find(|n| n.id == note_id) {
                // Note header
                draw_string(surface, content_x + 20, self.bounds.y + 15, &note.title, accent_color);

                // Edit button
                if self.view_mode == ViewMode::NoteView {
                    draw_string(surface, content_x + content_width as isize - 100, self.bounds.y + 15, "[Edit]", dim_text);
                }

                // Tags
                let mut tag_x = content_x + 20;
                for tag in &note.tags {
                    let tag_str = String::from("#") + tag;
                    draw_string(surface, tag_x, self.bounds.y + 35, &tag_str, Color::new(100, 149, 237));
                    tag_x += (tag.len() + 2) as isize * 8;
                }

                // Word count
                let stats = format!("{} words", note.word_count);
                draw_string(surface, content_x + content_width as isize - 100, self.bounds.y + 35, &stats, dim_text);

                // Content
                let mut content_y = self.bounds.y + 70;
                let line_height = 20isize;

                for block in &note.blocks {
                    // Heading style
                    let text_col = match block.heading {
                        HeadingLevel::H1 => accent_color,
                        HeadingLevel::H2 => text_color,
                        _ => text_color,
                    };

                    // Indent
                    let indent = (block.indent_level * 20) as isize;
                    let block_x = content_x + 20 + indent;

                    // List prefix
                    match block.list_type {
                        ListType::Bullet => {
                            draw_char(surface, block_x, content_y, '*', dim_text);
                        }
                        ListType::Numbered => {
                            draw_string(surface, block_x, content_y, "1.", dim_text);
                        }
                        ListType::Checkbox => {
                            draw_string(surface, block_x, content_y, "[ ]", dim_text);
                        }
                        ListType::CheckboxChecked => {
                            draw_string(surface, block_x, content_y, "[x]", Color::new(50, 205, 50));
                        }
                        ListType::None => {}
                    }

                    let text_x = if block.list_type != ListType::None {
                        block_x + 30
                    } else {
                        block_x
                    };

                    // Content text
                    let content_text = &block.content;
                    let display_text = if content_text.len() > 60 {
                        let mut t: String = content_text.chars().take(57).collect();
                        t.push_str("...");
                        t
                    } else {
                        content_text.clone()
                    };

                    draw_string(surface, text_x, content_y, &display_text, text_col);

                    content_y += line_height;

                    if content_y > self.bounds.y + self.bounds.height as isize - 20 {
                        break;
                    }
                }

                // Mode indicator
                let mode_str = match self.view_mode {
                    ViewMode::NoteEdit => "[Editing]",
                    ViewMode::NoteView => "[Viewing]",
                    _ => "",
                };
                draw_string(surface, content_x + 20, self.bounds.y + self.bounds.height as isize - 25, mode_str, dim_text);
            }
        } else {
            // Empty state
            let center_x = content_x + (content_width as isize / 2) - 60;
            let center_y = self.bounds.y + (self.bounds.height as isize / 2) - 20;

            draw_string(surface, center_x, center_y, "No note selected", dim_text);
            draw_string(surface, center_x - 20, center_y + 25, "Select a note or create new", dim_text);
        }

        // Bottom stats bar
        let notes_count = self.filtered_notes().len();
        let total_count = self.notes.iter().filter(|n| !n.is_trashed).count();
        let stats_str = format!("{} notes shown, {} total", notes_count, total_count);
        draw_string(surface, self.bounds.x + self.sidebar_width as isize + 10,
            self.bounds.y + self.bounds.height as isize - 20, &stats_str, dim_text);
    }
}

/// Initialize the notes module
pub fn init() {
    crate::kprintln!("[Notes] Notes application initialized");
}
