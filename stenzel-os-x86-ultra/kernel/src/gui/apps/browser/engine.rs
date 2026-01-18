//! Browser Engine
//!
//! Core browser engine that coordinates HTML parsing, CSS parsing,
//! layout, and rendering.

use alloc::string::String;
use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use super::dom::{Dom, DomNode};
use super::html_parser::{HtmlParser, HtmlDocument};
use super::css_parser::{CssParser, StyleSheet};
use super::layout::{LayoutEngine, LayoutBox};
use super::render::{Renderer, PaintCommand, RenderTree, RenderNode};

/// Browser engine
pub struct BrowserEngine {
    /// Current document
    document: Option<HtmlDocument>,
    /// Combined stylesheet
    stylesheet: StyleSheet,
    /// Layout engine
    layout_engine: LayoutEngine,
    /// Renderer
    renderer: Renderer,
    /// Render tree
    render_tree: Option<RenderTree>,
    /// Viewport width
    viewport_width: f32,
    /// Viewport height
    viewport_height: f32,
    /// Scroll position X
    scroll_x: f32,
    /// Scroll position Y
    scroll_y: f32,
    /// Content height (for scrolling)
    content_height: f32,
    /// Content width
    content_width: f32,
    /// User agent stylesheet
    ua_stylesheet: StyleSheet,
    /// Resource cache
    resource_cache: BTreeMap<String, CachedResource>,
}

impl BrowserEngine {
    /// Create new browser engine
    pub fn new(viewport_width: f32, viewport_height: f32) -> Self {
        let mut engine = Self {
            document: None,
            stylesheet: StyleSheet::new(),
            layout_engine: LayoutEngine::new(viewport_width, viewport_height),
            renderer: Renderer::new(),
            render_tree: None,
            viewport_width,
            viewport_height,
            scroll_x: 0.0,
            scroll_y: 0.0,
            content_height: 0.0,
            content_width: viewport_width,
            ua_stylesheet: StyleSheet::new(),
            resource_cache: BTreeMap::new(),
        };

        // Parse default user agent stylesheet
        engine.ua_stylesheet = Self::default_ua_stylesheet();

        engine
    }

    /// Load HTML content
    pub fn load_html(&mut self, html: &str) {
        // Parse HTML
        let mut parser = HtmlParser::new(html);
        self.document = Some(parser.parse());

        // Extract and parse stylesheets
        self.extract_stylesheets();

        // Perform layout and render
        self.relayout();
    }

    /// Load CSS
    pub fn load_css(&mut self, css: &str) {
        let mut parser = CssParser::new(css);
        let stylesheet = parser.parse();

        // Merge with existing stylesheet
        self.stylesheet.rules.extend(stylesheet.rules);
        self.stylesheet.keyframes.extend(stylesheet.keyframes);
        self.stylesheet.font_faces.extend(stylesheet.font_faces);

        // Re-render
        self.relayout();
    }

    /// Get document title
    pub fn title(&self) -> &str {
        self.document.as_ref().map(|d| d.title.as_str()).unwrap_or("")
    }

    /// Get current scroll position
    pub fn scroll_position(&self) -> (f32, f32) {
        (self.scroll_x, self.scroll_y)
    }

    /// Set scroll position
    pub fn set_scroll(&mut self, x: f32, y: f32) {
        self.scroll_x = x.max(0.0).min((self.content_width - self.viewport_width).max(0.0));
        self.scroll_y = y.max(0.0).min((self.content_height - self.viewport_height).max(0.0));
        self.renderer.set_scroll(self.scroll_x, self.scroll_y);
    }

    /// Scroll by delta
    pub fn scroll_by(&mut self, dx: f32, dy: f32) {
        self.set_scroll(self.scroll_x + dx, self.scroll_y + dy);
    }

    /// Get content dimensions
    pub fn content_size(&self) -> (f32, f32) {
        (self.content_width, self.content_height)
    }

    /// Set viewport size
    pub fn set_viewport(&mut self, width: f32, height: f32) {
        self.viewport_width = width;
        self.viewport_height = height;
        self.layout_engine = LayoutEngine::new(width, height);
        self.relayout();
    }

    /// Get viewport size
    pub fn viewport(&self) -> (f32, f32) {
        (self.viewport_width, self.viewport_height)
    }

    /// Render to paint commands
    pub fn render(&mut self) -> Vec<PaintCommand> {
        if let Some(tree) = &self.render_tree {
            if let Some(root) = &tree.root {
                let layout = self.rebuild_layout_from_render(root);
                return self.renderer.render(&layout);
            }
        }

        // Fallback: render document directly
        if let Some(doc) = &self.document {
            if let Some(root) = &doc.dom.root {
                let layout = self.layout_engine.layout(root);
                return self.renderer.render(&layout);
            }
        }

        Vec::new()
    }

    /// Get render tree
    pub fn render_tree(&self) -> Option<&RenderTree> {
        self.render_tree.as_ref()
    }

    /// Hit test at position
    pub fn hit_test(&self, x: f32, y: f32) -> Option<HitTestResult> {
        let actual_x = x + self.scroll_x;
        let actual_y = y + self.scroll_y;

        if let Some(tree) = &self.render_tree {
            if let Some(root) = &tree.root {
                return self.hit_test_node(root, actual_x, actual_y, 0.0, 0.0);
            }
        }

        None
    }

    /// Find element at position
    fn hit_test_node(&self, node: &RenderNode, x: f32, y: f32, offset_x: f32, offset_y: f32) -> Option<HitTestResult> {
        let node_x = node.bounds.x + offset_x;
        let node_y = node.bounds.y + offset_y;

        // Check if point is in this node
        if x >= node_x && x < node_x + node.bounds.width &&
           y >= node_y && y < node_y + node.bounds.height {
            // Check children first (they're on top)
            for child in node.children.iter().rev() {
                if let Some(result) = self.hit_test_node(child, x, y, node_x, node_y) {
                    return Some(result);
                }
            }

            // Return this node
            return Some(HitTestResult {
                x: x - node_x,
                y: y - node_y,
                text: node.text.as_ref().map(|t| t.content.clone()),
            });
        }

        None
    }

    /// Get DOM reference
    pub fn dom(&self) -> Option<&Dom> {
        self.document.as_ref().map(|d| &d.dom)
    }

    /// Get document reference
    pub fn document(&self) -> Option<&HtmlDocument> {
        self.document.as_ref()
    }

    // Private methods

    fn extract_stylesheets(&mut self) {
        // Start with UA stylesheet
        self.stylesheet = self.ua_stylesheet.clone();

        if let Some(doc) = &self.document {
            // Find style elements
            let style_elements = doc.dom.get_elements_by_tag_name("style");
            for style in style_elements {
                let css = style.text_content();
                if !css.is_empty() {
                    let mut parser = CssParser::new(&css);
                    let sheet = parser.parse();
                    self.stylesheet.rules.extend(sheet.rules);
                }
            }

            // Find inline styles from head (link elements)
            // Note: We don't fetch external stylesheets here, just parse inline ones
        }
    }

    fn relayout(&mut self) {
        // Set stylesheet on layout engine
        self.layout_engine.set_stylesheet(self.stylesheet.clone());

        if let Some(doc) = &self.document {
            if let Some(root) = &doc.dom.root {
                // Perform layout
                let layout = self.layout_engine.layout(root);

                // Store content dimensions
                self.content_height = layout.dimensions.margin_box().height;
                self.content_width = layout.dimensions.margin_box().width;

                // Build render tree
                self.render_tree = Some(RenderTree::from_layout(layout));
            }
        }
    }

    fn rebuild_layout_from_render(&self, _node: &RenderNode) -> LayoutBox {
        // For now, just relayout from DOM
        if let Some(doc) = &self.document {
            if let Some(root) = &doc.dom.root {
                return self.layout_engine.layout(root);
            }
        }

        LayoutBox::new(super::layout::LayoutMode::Block)
    }

    fn default_ua_stylesheet() -> StyleSheet {
        let ua_css = r#"
            html, body {
                margin: 0;
                padding: 0;
            }
            body {
                font-family: system-ui, sans-serif;
                font-size: 16px;
                line-height: 1.5;
                color: #000;
                background: #fff;
            }
            h1 { font-size: 2em; font-weight: bold; margin: 0.67em 0; }
            h2 { font-size: 1.5em; font-weight: bold; margin: 0.83em 0; }
            h3 { font-size: 1.17em; font-weight: bold; margin: 1em 0; }
            h4 { font-size: 1em; font-weight: bold; margin: 1.33em 0; }
            h5 { font-size: 0.83em; font-weight: bold; margin: 1.67em 0; }
            h6 { font-size: 0.67em; font-weight: bold; margin: 2.33em 0; }
            p { margin: 1em 0; }
            a { color: #0066cc; text-decoration: underline; }
            a:visited { color: #551a8b; }
            a:hover { color: #0044aa; }
            strong, b { font-weight: bold; }
            em, i { font-style: italic; }
            code, pre { font-family: monospace; background: #f4f4f4; }
            pre { padding: 1em; overflow: auto; }
            ul, ol { padding-left: 40px; margin: 1em 0; }
            li { display: list-item; }
            table { border-collapse: collapse; }
            th, td { padding: 0.5em; border: 1px solid #ccc; }
            th { font-weight: bold; background: #f0f0f0; }
            img { max-width: 100%; }
            hr { border: none; border-top: 1px solid #ccc; margin: 1em 0; }
            blockquote { margin: 1em 40px; padding-left: 1em; border-left: 4px solid #ccc; }
            input, button, select, textarea {
                font-family: inherit;
                font-size: inherit;
            }
            button, input[type="button"], input[type="submit"] {
                padding: 0.5em 1em;
                border: 1px solid #ccc;
                background: #f0f0f0;
                cursor: pointer;
            }
            button:hover {
                background: #e0e0e0;
            }
        "#;

        let mut parser = CssParser::new(ua_css);
        parser.parse()
    }
}

impl Default for BrowserEngine {
    fn default() -> Self {
        Self::new(800.0, 600.0)
    }
}

/// Hit test result
#[derive(Debug, Clone)]
pub struct HitTestResult {
    /// X position within element
    pub x: f32,
    /// Y position within element
    pub y: f32,
    /// Text content if any
    pub text: Option<String>,
}

/// Cached resource
#[derive(Debug, Clone)]
pub struct CachedResource {
    /// Resource type
    pub resource_type: ResourceType,
    /// Resource data
    pub data: Vec<u8>,
    /// Content type
    pub content_type: String,
    /// Cache time
    pub cached_at: u64,
    /// Expiry time (if any)
    pub expires_at: Option<u64>,
}

/// Resource type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResourceType {
    /// HTML document
    Html,
    /// CSS stylesheet
    Css,
    /// JavaScript
    JavaScript,
    /// Image
    Image,
    /// Font
    Font,
    /// Other
    Other,
}

