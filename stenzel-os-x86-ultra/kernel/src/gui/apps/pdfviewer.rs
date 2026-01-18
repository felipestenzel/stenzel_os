//! PDF Viewer Application
//!
//! View and navigate PDF documents.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use spin::Mutex;
use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use crate::gui::widgets::{Widget, WidgetId, WidgetEvent, Bounds, MouseButton};

/// Global PDF viewer state
static PDF_VIEWER_STATE: Mutex<Option<PdfViewerState>> = Mutex::new(None);

/// PDF viewer state
pub struct PdfViewerState {
    /// Currently open document
    pub current_document: Option<PdfDocument>,
    /// Recent files
    pub recent: Vec<String>,
    /// Default zoom level
    pub default_zoom: ZoomLevel,
    /// Show outline by default
    pub show_outline: bool,
    /// Single/continuous page mode
    pub page_mode: PageMode,
}

/// PDF document
#[derive(Debug, Clone)]
pub struct PdfDocument {
    /// File path
    pub path: String,
    /// Title (from metadata)
    pub title: Option<String>,
    /// Author (from metadata)
    pub author: Option<String>,
    /// Number of pages
    pub page_count: usize,
    /// Current page (1-indexed)
    pub current_page: usize,
    /// Pages data
    pub pages: Vec<PdfPage>,
    /// Document outline (bookmarks)
    pub outline: Vec<OutlineEntry>,
    /// Zoom level
    pub zoom: f32,
    /// Scroll offset
    pub scroll_y: usize,
    /// Page width (pixels at 100%)
    pub page_width: u32,
    /// Page height (pixels at 100%)
    pub page_height: u32,
    /// Creation date
    pub created: Option<String>,
    /// Modification date
    pub modified: Option<String>,
}

/// PDF page
#[derive(Debug, Clone)]
pub struct PdfPage {
    /// Page number (1-indexed)
    pub number: usize,
    /// Page width
    pub width: u32,
    /// Page height
    pub height: u32,
    /// Rendered content (would be bitmap data)
    pub rendered: Option<Vec<u8>>,
    /// Text content
    pub text: Option<String>,
    /// Links on page
    pub links: Vec<PdfLink>,
    /// Annotations
    pub annotations: Vec<PdfAnnotation>,
}

/// PDF link
#[derive(Debug, Clone)]
pub struct PdfLink {
    /// Link bounds
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    /// Link target
    pub target: LinkTarget,
}

/// Link target
#[derive(Debug, Clone)]
pub enum LinkTarget {
    /// Internal page link
    Page(usize),
    /// External URL
    Url(String),
    /// Named destination
    Named(String),
}

/// PDF annotation
#[derive(Debug, Clone)]
pub struct PdfAnnotation {
    /// Annotation type
    pub kind: AnnotationType,
    /// Position
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    /// Content text
    pub content: Option<String>,
}

/// Annotation type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotationType {
    Highlight,
    Underline,
    Strikeout,
    Note,
    FreeText,
    Ink,
}

/// Outline entry (bookmark)
#[derive(Debug, Clone)]
pub struct OutlineEntry {
    /// Title
    pub title: String,
    /// Target page
    pub page: usize,
    /// Nesting level
    pub level: usize,
    /// Children
    pub children: Vec<OutlineEntry>,
    /// Is expanded
    pub expanded: bool,
}

/// Zoom level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomLevel {
    FitPage,
    FitWidth,
    Actual,
    Custom,
}

/// Page display mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageMode {
    Single,
    Continuous,
    TwoPage,
    TwoPageContinuous,
}

/// PDF viewer error
#[derive(Debug, Clone)]
pub enum PdfError {
    NotInitialized,
    FileNotFound(String),
    InvalidPdf(String),
    PasswordRequired,
    RenderError(String),
    PageOutOfRange,
}

/// Initialize PDF viewer
pub fn init() {
    let mut state = PDF_VIEWER_STATE.lock();
    if state.is_some() {
        return;
    }

    *state = Some(PdfViewerState {
        current_document: None,
        recent: Vec::new(),
        default_zoom: ZoomLevel::FitWidth,
        show_outline: true,
        page_mode: PageMode::Continuous,
    });

    crate::kprintln!("pdfviewer: initialized");
}

/// Open a PDF file
pub fn open(path: &str) -> Result<PdfDocument, PdfError> {
    let mut state = PDF_VIEWER_STATE.lock();
    let s = state.as_mut().ok_or(PdfError::NotInitialized)?;

    // Read and parse PDF (simplified - would use actual PDF parser)
    // For now, create a placeholder document
    let doc = PdfDocument {
        path: path.to_string(),
        title: Some(path.rsplit('/').next().unwrap_or(path).to_string()),
        author: None,
        page_count: 1,
        current_page: 1,
        pages: vec![PdfPage {
            number: 1,
            width: 612, // US Letter width in points
            height: 792, // US Letter height in points
            rendered: None,
            text: Some("PDF content would be rendered here.".to_string()),
            links: Vec::new(),
            annotations: Vec::new(),
        }],
        outline: Vec::new(),
        zoom: 1.0,
        scroll_y: 0,
        page_width: 612,
        page_height: 792,
        created: None,
        modified: None,
    };

    // Add to recent
    if !s.recent.contains(&path.to_string()) {
        s.recent.insert(0, path.to_string());
        if s.recent.len() > 20 {
            s.recent.pop();
        }
    }

    s.current_document = Some(doc.clone());

    Ok(doc)
}

/// Close current document
pub fn close() {
    let mut state = PDF_VIEWER_STATE.lock();
    if let Some(ref mut s) = *state {
        s.current_document = None;
    }
}

/// Go to specific page
pub fn go_to_page(page: usize) -> Result<(), PdfError> {
    let mut state = PDF_VIEWER_STATE.lock();
    let s = state.as_mut().ok_or(PdfError::NotInitialized)?;
    let doc = s.current_document.as_mut().ok_or(PdfError::NotInitialized)?;

    if page < 1 || page > doc.page_count {
        return Err(PdfError::PageOutOfRange);
    }

    doc.current_page = page;
    Ok(())
}

/// Next page
pub fn next_page() {
    let mut state = PDF_VIEWER_STATE.lock();
    if let Some(ref mut s) = *state {
        if let Some(ref mut doc) = s.current_document {
            if doc.current_page < doc.page_count {
                doc.current_page += 1;
            }
        }
    }
}

/// Previous page
pub fn previous_page() {
    let mut state = PDF_VIEWER_STATE.lock();
    if let Some(ref mut s) = *state {
        if let Some(ref mut doc) = s.current_document {
            if doc.current_page > 1 {
                doc.current_page -= 1;
            }
        }
    }
}

/// First page
pub fn first_page() {
    let mut state = PDF_VIEWER_STATE.lock();
    if let Some(ref mut s) = *state {
        if let Some(ref mut doc) = s.current_document {
            doc.current_page = 1;
        }
    }
}

/// Last page
pub fn last_page() {
    let mut state = PDF_VIEWER_STATE.lock();
    if let Some(ref mut s) = *state {
        if let Some(ref mut doc) = s.current_document {
            doc.current_page = doc.page_count;
        }
    }
}

/// Set zoom level
pub fn set_zoom(zoom: f32) {
    let mut state = PDF_VIEWER_STATE.lock();
    if let Some(ref mut s) = *state {
        if let Some(ref mut doc) = s.current_document {
            doc.zoom = zoom.max(0.1).min(10.0);
        }
    }
}

/// Zoom in
pub fn zoom_in() {
    let mut state = PDF_VIEWER_STATE.lock();
    if let Some(ref mut s) = *state {
        if let Some(ref mut doc) = s.current_document {
            doc.zoom = (doc.zoom * 1.25).min(10.0);
        }
    }
}

/// Zoom out
pub fn zoom_out() {
    let mut state = PDF_VIEWER_STATE.lock();
    if let Some(ref mut s) = *state {
        if let Some(ref mut doc) = s.current_document {
            doc.zoom = (doc.zoom / 1.25).max(0.1);
        }
    }
}

/// Fit to page
pub fn fit_page() {
    let mut state = PDF_VIEWER_STATE.lock();
    if let Some(ref mut s) = *state {
        s.default_zoom = ZoomLevel::FitPage;
    }
}

/// Fit to width
pub fn fit_width() {
    let mut state = PDF_VIEWER_STATE.lock();
    if let Some(ref mut s) = *state {
        s.default_zoom = ZoomLevel::FitWidth;
    }
}

/// Set page mode
pub fn set_page_mode(mode: PageMode) {
    let mut state = PDF_VIEWER_STATE.lock();
    if let Some(ref mut s) = *state {
        s.page_mode = mode;
    }
}

/// Scroll document
pub fn scroll(delta: isize) {
    let mut state = PDF_VIEWER_STATE.lock();
    if let Some(ref mut s) = *state {
        if let Some(ref mut doc) = s.current_document {
            if delta > 0 {
                doc.scroll_y = doc.scroll_y.saturating_add(delta as usize);
            } else {
                doc.scroll_y = doc.scroll_y.saturating_sub((-delta) as usize);
            }
        }
    }
}

/// Search text in document
pub fn search(_query: &str) -> Vec<SearchResult> {
    // Would search through document text
    Vec::new()
}

/// Search result
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Page number
    pub page: usize,
    /// Position on page
    pub x: u32,
    pub y: u32,
    /// Match bounds
    pub width: u32,
    pub height: u32,
    /// Context text
    pub context: String,
}

/// Get current document
pub fn get_current_document() -> Option<PdfDocument> {
    let state = PDF_VIEWER_STATE.lock();
    state.as_ref().and_then(|s| s.current_document.clone())
}

/// Get recent files
pub fn get_recent() -> Vec<String> {
    let state = PDF_VIEWER_STATE.lock();
    state.as_ref().map(|s| s.recent.clone()).unwrap_or_default()
}

// Theme colors
fn window_background() -> Color { Color::new(60, 60, 60) }
fn page_color() -> Color { Color::new(255, 255, 255) }
fn toolbar_color() -> Color { Color::new(45, 45, 45) }
fn text_color() -> Color { Color::new(240, 240, 240) }
fn accent_color() -> Color { Color::new(0, 120, 215) }
fn button_color() -> Color { Color::new(70, 70, 70) }

/// PDF viewer widget
pub struct PdfViewer {
    id: WidgetId,
    bounds: Bounds,
    enabled: bool,
    visible: bool,
    show_toolbar: bool,
    show_outline: bool,
    outline_width: usize,
    dragging: bool,
    drag_start_y: isize,
    drag_start_scroll: usize,
}

impl PdfViewer {
    pub fn new(id: WidgetId, bounds: Bounds) -> Self {
        Self {
            id,
            bounds,
            enabled: true,
            visible: true,
            show_toolbar: true,
            show_outline: false,
            outline_width: 200,
            dragging: false,
            drag_start_y: 0,
            drag_start_scroll: 0,
        }
    }

    pub fn toggle_outline(&mut self) {
        self.show_outline = !self.show_outline;
    }

    fn render_toolbar(&self, surface: &mut Surface, x: usize, y: usize, w: usize) {
        let toolbar_height = 40;
        surface.fill_rect(x, y, w, toolbar_height, toolbar_color());

        // Get document info
        let state = PDF_VIEWER_STATE.lock();
        if let Some(ref s) = *state {
            if let Some(ref doc) = s.current_document {
                // Page navigation
                let nav_x = x + 10;

                // Previous page button
                surface.fill_rect(nav_x, y + 8, 24, 24, button_color());

                // Page indicator
                let page_text = alloc::format!("{} / {}", doc.current_page, doc.page_count);
                let _text_x = nav_x + 35;
                // Would render text here

                // Next page button
                surface.fill_rect(nav_x + 100, y + 8, 24, 24, button_color());

                // Zoom controls
                let zoom_x = x + w / 2 - 60;

                // Zoom out button
                surface.fill_rect(zoom_x, y + 8, 24, 24, button_color());

                // Zoom level
                let zoom_text = alloc::format!("{}%", (doc.zoom * 100.0) as u32);
                let _zoom_text_x = zoom_x + 35;
                // Would render text here
                let _ = zoom_text;
                let _ = page_text;

                // Zoom in button
                surface.fill_rect(zoom_x + 90, y + 8, 24, 24, button_color());

                // Toggle outline button
                let outline_btn_x = x + w - 40;
                let btn_color = if self.show_outline { accent_color() } else { button_color() };
                surface.fill_rect(outline_btn_x, y + 8, 24, 24, btn_color);
            }
        }
    }

    fn render_outline(&self, surface: &mut Surface, x: usize, y: usize, h: usize) {
        surface.fill_rect(x, y, self.outline_width, h, Color::new(50, 50, 50));

        // Would render outline entries here
        let state = PDF_VIEWER_STATE.lock();
        if let Some(ref s) = *state {
            if let Some(ref doc) = s.current_document {
                let mut entry_y = y + 10;
                for entry in &doc.outline {
                    // Render outline entry
                    let indent = entry.level * 20;
                    surface.fill_rect(x + 10 + indent, entry_y, self.outline_width - 20 - indent, 24, Color::new(60, 60, 60));
                    // Would render entry.title here
                    let _ = entry;
                    entry_y += 28;
                    if entry_y > y + h - 30 {
                        break;
                    }
                }
            }
        }
    }

    fn render_page(&self, surface: &mut Surface, x: usize, y: usize, w: usize, h: usize) {
        // Page area background
        surface.fill_rect(x, y, w, h, window_background());

        let state = PDF_VIEWER_STATE.lock();
        if let Some(ref s) = *state {
            if let Some(ref doc) = s.current_document {
                // Calculate page dimensions
                let page_w = (doc.page_width as f32 * doc.zoom) as usize;
                let page_h = (doc.page_height as f32 * doc.zoom) as usize;

                // Center page horizontally
                let page_x = if page_w < w {
                    x + (w - page_w) / 2
                } else {
                    x
                };

                let page_y = y + 20; // Some padding

                // Draw page shadow
                surface.fill_rect(page_x + 3, page_y + 3, page_w, page_h.min(h - 40), Color::new(30, 30, 30));

                // Draw page
                surface.fill_rect(page_x, page_y, page_w, page_h.min(h - 40), page_color());

                // Would render actual PDF content here
                // For now, just show a placeholder
                if page_w > 100 && page_h > 100 {
                    // Simulate some text lines
                    let text_color = Color::new(50, 50, 50);
                    let line_spacing = 20;
                    let margin = 40;
                    let mut line_y = page_y + margin;

                    while line_y < page_y + page_h.min(h - 40) - margin {
                        let line_w = (page_w - margin * 2).min(w - margin * 2);
                        surface.fill_rect(page_x + margin, line_y, line_w, 8, text_color);
                        line_y += line_spacing;
                    }
                }
            }
        } else {
            // No document - show placeholder
            let center_x = x + w / 2;
            let center_y = y + h / 2;

            // Document icon placeholder
            surface.fill_rect(center_x - 30, center_y - 40, 60, 80, Color::new(80, 80, 80));
            surface.fill_rect(center_x - 20, center_y - 20, 40, 5, Color::new(60, 60, 60));
            surface.fill_rect(center_x - 20, center_y - 10, 40, 5, Color::new(60, 60, 60));
            surface.fill_rect(center_x - 20, center_y, 30, 5, Color::new(60, 60, 60));
        }
    }
}

impl Widget for PdfViewer {
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

    fn render(&self, surface: &mut Surface) {
        let x = self.bounds.x as usize;
        let y = self.bounds.y as usize;
        let w = self.bounds.width;
        let h = self.bounds.height;

        // Toolbar
        let toolbar_height = if self.show_toolbar { 40 } else { 0 };
        if self.show_toolbar {
            self.render_toolbar(surface, x, y, w);
        }

        // Content area
        let content_y = y + toolbar_height;
        let content_h = h - toolbar_height;

        // Outline sidebar
        let outline_w = if self.show_outline { self.outline_width } else { 0 };
        if self.show_outline {
            self.render_outline(surface, x, content_y, content_h);
        }

        // Page area
        let page_x = x + outline_w;
        let page_w = w - outline_w;
        self.render_page(surface, page_x, content_y, page_w, content_h);
    }

    fn handle_event(&mut self, event: &WidgetEvent) -> bool {
        match event {
            WidgetEvent::MouseDown { x, y, button: MouseButton::Left } => {
                let rel_x = (*x - self.bounds.x) as usize;
                let rel_y = (*y - self.bounds.y) as usize;
                let w = self.bounds.width;

                // Toolbar clicks
                if self.show_toolbar && rel_y < 40 {
                    // Previous page
                    if rel_x >= 10 && rel_x < 34 {
                        previous_page();
                        return true;
                    }
                    // Next page
                    if rel_x >= 110 && rel_x < 134 {
                        next_page();
                        return true;
                    }

                    // Zoom controls
                    let zoom_x = w / 2 - 60;
                    if rel_x >= zoom_x && rel_x < zoom_x + 24 {
                        zoom_out();
                        return true;
                    }
                    if rel_x >= zoom_x + 90 && rel_x < zoom_x + 114 {
                        zoom_in();
                        return true;
                    }

                    // Outline toggle
                    if rel_x >= w - 40 && rel_x < w - 16 {
                        self.toggle_outline();
                        return true;
                    }

                    return true;
                }

                // Start drag to scroll
                self.dragging = true;
                self.drag_start_y = *y;
                let state = PDF_VIEWER_STATE.lock();
                if let Some(ref s) = *state {
                    if let Some(ref doc) = s.current_document {
                        self.drag_start_scroll = doc.scroll_y;
                    }
                }

                false
            }
            WidgetEvent::MouseUp { .. } => {
                self.dragging = false;
                false
            }
            WidgetEvent::MouseMove { y, .. } => {
                if self.dragging {
                    let delta = self.drag_start_y - *y;
                    let mut state = PDF_VIEWER_STATE.lock();
                    if let Some(ref mut s) = *state {
                        if let Some(ref mut doc) = s.current_document {
                            if delta > 0 {
                                doc.scroll_y = self.drag_start_scroll.saturating_add(delta as usize);
                            } else {
                                doc.scroll_y = self.drag_start_scroll.saturating_sub((-delta) as usize);
                            }
                        }
                    }
                    return true;
                }
                false
            }
            WidgetEvent::Scroll { delta_y, .. } => {
                scroll(*delta_y as isize * 30);
                true
            }
            WidgetEvent::KeyDown { key, .. } => {
                match *key {
                    0x49 => { // Page Up
                        previous_page();
                        return true;
                    }
                    0x51 => { // Page Down
                        next_page();
                        return true;
                    }
                    0x47 => { // Home
                        first_page();
                        return true;
                    }
                    0x4F => { // End
                        last_page();
                        return true;
                    }
                    0x4E => { // + (numpad)
                        zoom_in();
                        return true;
                    }
                    0x4A => { // - (numpad)
                        zoom_out();
                        return true;
                    }
                    _ => {}
                }
                false
            }
            _ => false,
        }
    }
}
