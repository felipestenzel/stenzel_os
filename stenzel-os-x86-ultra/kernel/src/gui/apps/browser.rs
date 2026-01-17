//! Web Browser Application
//!
//! A basic web browser with HTML rendering capabilities.

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use core::cmp::min;

use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use crate::gui::widgets::{Widget, WidgetId, Bounds, WidgetEvent, MouseButton, theme};

/// Maximum history entries
const MAX_HISTORY: usize = 100;

/// Default home page
const HOME_PAGE: &str = "about:blank";

/// Browser navigation action
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NavAction {
    Back,
    Forward,
    Refresh,
    Home,
}

/// Toolbar button
struct ToolbarButton {
    bounds: Bounds,
    action: NavAction,
    icon: char,
    hovered: bool,
    enabled: bool,
}

impl ToolbarButton {
    fn new(x: isize, y: isize, action: NavAction, icon: char) -> Self {
        Self {
            bounds: Bounds::new(x, y, 32, 28),
            action,
            icon,
            hovered: false,
            enabled: true,
        }
    }
}

/// Parsed HTML element
#[derive(Debug, Clone)]
enum HtmlElement {
    Text(String),
    Break,
    Paragraph,
    Heading(u8, String),
    Link(String, String),
    Bold(String),
    Italic(String),
    ListItem(String),
    HorizontalRule,
    Image(String, String),
    Pre(String),
    Code(String),
}

/// Rendered line
#[derive(Debug, Clone)]
struct RenderLine {
    y: isize,
    elements: Vec<RenderElement>,
    height: usize,
}

/// Rendered element
#[derive(Debug, Clone)]
struct RenderElement {
    x: isize,
    width: usize,
    text: String,
    color: Color,
    link: Option<String>,
}

/// Loading state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadingState {
    Idle,
    Connecting,
    Loading,
    Done,
    Error,
}

/// Web Browser
pub struct Browser {
    id: WidgetId,
    bounds: Bounds,
    url: String,
    url_input: String,
    url_focused: bool,
    url_cursor: usize,
    title: String,
    html_content: String,
    elements: Vec<HtmlElement>,
    rendered: Vec<RenderLine>,
    scroll_y: isize,
    content_height: usize,
    history: Vec<String>,
    history_index: usize,
    toolbar_buttons: Vec<ToolbarButton>,
    loading_state: LoadingState,
    status: String,
    hovered_link: Option<String>,
    visible: bool,
    enabled: bool,
}

// Helper drawing functions
fn draw_string(surface: &mut Surface, x: isize, y: isize, text: &str, color: Color) {
    use crate::drivers::font::DEFAULT_FONT;

    if x < 0 || y < 0 {
        return;
    }

    let mut cx = x as usize;
    let cy = y as usize;

    for c in text.chars() {
        if let Some(glyph) = DEFAULT_FONT.get_glyph(c) {
            for row in 0..DEFAULT_FONT.height {
                let byte = glyph[row];
                for col in 0..DEFAULT_FONT.width {
                    if (byte >> (DEFAULT_FONT.width - 1 - col)) & 1 != 0 {
                        surface.set_pixel(cx + col, cy + row, color);
                    }
                }
            }
        }
        cx += DEFAULT_FONT.width;
    }
}

fn fill_rect_safe(surface: &mut Surface, x: isize, y: isize, width: usize, height: usize, color: Color) {
    if x < 0 || y < 0 || width == 0 || height == 0 {
        return;
    }
    surface.fill_rect(x as usize, y as usize, width, height, color);
}

fn draw_rect_safe(surface: &mut Surface, x: isize, y: isize, width: usize, height: usize, color: Color) {
    if x < 0 || y < 0 || width == 0 || height == 0 {
        return;
    }
    surface.draw_rect(x as usize, y as usize, width, height, color);
}

impl Browser {
    pub fn new(x: isize, y: isize, width: usize, height: usize) -> Self {
        let mut toolbar_buttons = Vec::new();
        let btn_y = y + 4;

        toolbar_buttons.push(ToolbarButton::new(x + 8, btn_y, NavAction::Back, '<'));
        toolbar_buttons.push(ToolbarButton::new(x + 44, btn_y, NavAction::Forward, '>'));
        toolbar_buttons.push(ToolbarButton::new(x + 80, btn_y, NavAction::Refresh, 'R'));
        toolbar_buttons.push(ToolbarButton::new(x + 116, btn_y, NavAction::Home, 'H'));

        let mut browser = Self {
            id: WidgetId::new(),
            bounds: Bounds::new(x, y, width, height),
            url: String::from(HOME_PAGE),
            url_input: String::from(HOME_PAGE),
            url_focused: false,
            url_cursor: HOME_PAGE.len(),
            title: String::from("New Tab"),
            html_content: String::new(),
            elements: Vec::new(),
            rendered: Vec::new(),
            scroll_y: 0,
            content_height: 0,
            history: Vec::new(),
            history_index: 0,
            toolbar_buttons,
            loading_state: LoadingState::Idle,
            status: String::new(),
            hovered_link: None,
            visible: true,
            enabled: true,
        };

        browser.load_about_blank();
        browser
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn navigate(&mut self, url: &str) {
        let url = Self::normalize_url(url);
        self.url = url.clone();
        self.url_input = url.clone();
        self.url_cursor = self.url_input.len();

        if self.history.is_empty() || self.history.last() != Some(&url) {
            if self.history_index < self.history.len() {
                self.history.truncate(self.history_index + 1);
            }
            self.history.push(url.clone());
            if self.history.len() > MAX_HISTORY {
                self.history.remove(0);
            }
            self.history_index = self.history.len() - 1;
        }

        self.load_url(&url);
    }

    fn normalize_url(url: &str) -> String {
        let url = url.trim();
        if url.starts_with("about:") {
            return String::from(url);
        }
        if url.starts_with("http://") || url.starts_with("https://") {
            return String::from(url);
        }
        format!("https://{}", url)
    }

    fn load_url(&mut self, url: &str) {
        self.loading_state = LoadingState::Connecting;
        self.status = format!("Connecting to {}...", url);
        self.html_content.clear();
        self.elements.clear();
        self.rendered.clear();
        self.scroll_y = 0;

        if url.starts_with("about:") {
            self.load_about_page(url);
            return;
        }

        self.loading_state = LoadingState::Loading;
        self.status = format!("Loading {}...", url);

        match self.fetch_page(url) {
            Ok(content) => {
                self.html_content = content;
                self.parse_html();
                self.do_render();
                self.loading_state = LoadingState::Done;
                self.status = String::from("Done");
            }
            Err(e) => {
                self.loading_state = LoadingState::Error;
                self.status = format!("Error: {}", e);
                self.show_error(&format!("Failed to load: {}", e));
            }
        }
    }

    fn fetch_page(&self, url: &str) -> Result<String, &'static str> {
        if url.starts_with("https://") {
            match crate::net::tls::https_get(url) {
                Ok(response) => {
                    if response.status_code >= 200 && response.status_code < 300 {
                        Ok(core::str::from_utf8(&response.body)
                            .map(String::from)
                            .unwrap_or_else(|_| String::from("Invalid UTF-8")))
                    } else {
                        Err("HTTP error")
                    }
                }
                Err(_) => Err("Connection failed")
            }
        } else if url.starts_with("http://") {
            match crate::net::http::get(url) {
                Ok(response) => {
                    if response.status_code >= 200 && response.status_code < 300 {
                        Ok(core::str::from_utf8(&response.body)
                            .map(String::from)
                            .unwrap_or_else(|_| String::from("Invalid UTF-8")))
                    } else {
                        Err("HTTP error")
                    }
                }
                Err(_) => Err("Connection failed")
            }
        } else {
            Err("Invalid URL scheme")
        }
    }

    fn load_about_page(&mut self, url: &str) {
        match url {
            "about:blank" => self.load_about_blank(),
            "about:version" => self.load_about_version(),
            _ => self.load_about_blank(),
        }
        self.loading_state = LoadingState::Done;
        self.status = String::from("Done");
    }

    fn load_about_blank(&mut self) {
        self.title = String::from("New Tab");
        self.html_content = String::from(
            "<h1>Welcome to Stenzel Browser</h1>\
             <p>Enter a URL in the address bar to get started.</p>\
             <p>Try visiting:</p>\
             <li><a href=\"about:version\">about:version</a></li>\
             <li><a href=\"http://example.com\">example.com</a></li>"
        );
        self.parse_html();
        self.do_render();
    }

    fn load_about_version(&mut self) {
        self.title = String::from("About Stenzel Browser");
        self.html_content = String::from(
            "<h1>Stenzel Browser</h1>\
             <p><b>Version:</b> 1.0.0</p>\
             <p><b>Engine:</b> Stenzel HTML</p>\
             <p><b>OS:</b> Stenzel OS</p>\
             <hr>\
             <h2>Features</h2>\
             <li>HTTP/HTTPS support</li>\
             <li>Basic HTML rendering</li>\
             <li>Link navigation</li>\
             <li>History back/forward</li>"
        );
        self.parse_html();
        self.do_render();
    }

    fn show_error(&mut self, message: &str) {
        self.title = String::from("Error");
        self.html_content = format!(
            "<h1>Error Loading Page</h1>\
             <p>{}</p>\
             <p><a href=\"about:blank\">Go to home page</a></p>",
            message
        );
        self.parse_html();
        self.do_render();
    }

    fn parse_html(&mut self) {
        self.elements.clear();
        let html = self.html_content.clone();

        if let Some(start) = html.find("<title>") {
            if let Some(end) = html[start..].find("</title>") {
                self.title = String::from(html[start + 7..start + end].trim());
            }
        }

        let body_start = html.find("<body>").map(|i| i + 6).unwrap_or(0);
        let body_end = html.find("</body>").unwrap_or(html.len());
        let body = &html[body_start..body_end];

        let chars: Vec<char> = body.chars().collect();
        let mut i = 0;
        let mut current_text = String::new();

        while i < chars.len() {
            if chars[i] == '<' {
                if !current_text.is_empty() {
                    let text = current_text.trim();
                    if !text.is_empty() {
                        self.elements.push(HtmlElement::Text(String::from(text)));
                    }
                    current_text.clear();
                }

                let tag_start = i;
                while i < chars.len() && chars[i] != '>' {
                    i += 1;
                }
                if i < chars.len() {
                    i += 1;
                }

                let tag: String = chars[tag_start..i].iter().collect();
                self.handle_tag(&tag, &chars, &mut i);
            } else {
                current_text.push(chars[i]);
                i += 1;
            }
        }

        let text = current_text.trim();
        if !text.is_empty() {
            self.elements.push(HtmlElement::Text(String::from(text)));
        }
    }

    fn handle_tag(&mut self, tag: &str, chars: &[char], pos: &mut usize) {
        let tag = tag.trim_start_matches('<').trim_end_matches('>').trim();
        let tag_lower: String = tag.chars().map(|c| c.to_ascii_lowercase()).collect();

        if tag_lower == "br" || tag_lower == "br/" {
            self.elements.push(HtmlElement::Break);
            return;
        }
        if tag_lower == "hr" || tag_lower == "hr/" {
            self.elements.push(HtmlElement::HorizontalRule);
            return;
        }

        if tag.starts_with('/') {
            return;
        }

        let tag_name: String = tag_lower.split_whitespace().next().unwrap_or("").chars().collect();

        match tag_name.as_str() {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                let level = tag_name.chars().nth(1).unwrap_or('1').to_digit(10).unwrap_or(1) as u8;
                let end_tag = format!("</h{}>", level);
                if let Some(content) = Self::extract_until_static(chars, pos, &end_tag) {
                    self.elements.push(HtmlElement::Heading(level, String::from(content.trim())));
                }
            }
            "p" => {
                self.elements.push(HtmlElement::Paragraph);
            }
            "a" => {
                let href = Self::extract_attribute_static(tag, "href").unwrap_or_else(String::new);
                if let Some(content) = Self::extract_until_static(chars, pos, "</a>") {
                    let text = Self::strip_tags_static(&content);
                    self.elements.push(HtmlElement::Link(href, String::from(text.trim())));
                }
            }
            "b" | "strong" => {
                if let Some(content) = Self::extract_until_any_static(chars, pos, &["</b>", "</strong>"]) {
                    let text = Self::strip_tags_static(&content);
                    self.elements.push(HtmlElement::Bold(String::from(text.trim())));
                }
            }
            "i" | "em" => {
                if let Some(content) = Self::extract_until_any_static(chars, pos, &["</i>", "</em>"]) {
                    let text = Self::strip_tags_static(&content);
                    self.elements.push(HtmlElement::Italic(String::from(text.trim())));
                }
            }
            "li" => {
                if let Some(content) = Self::extract_until_static(chars, pos, "</li>") {
                    let text = Self::strip_tags_static(&content);
                    self.elements.push(HtmlElement::ListItem(String::from(text.trim())));
                }
            }
            "pre" => {
                if let Some(content) = Self::extract_until_static(chars, pos, "</pre>") {
                    self.elements.push(HtmlElement::Pre(content));
                }
            }
            "code" => {
                if let Some(content) = Self::extract_until_static(chars, pos, "</code>") {
                    self.elements.push(HtmlElement::Code(content));
                }
            }
            "img" => {
                let src = Self::extract_attribute_static(tag, "src").unwrap_or_else(String::new);
                let alt = Self::extract_attribute_static(tag, "alt").unwrap_or_else(|| String::from("[Image]"));
                self.elements.push(HtmlElement::Image(src, alt));
            }
            _ => {}
        }
    }

    fn extract_attribute_static(tag: &str, name: &str) -> Option<String> {
        let pattern = format!("{}=\"", name);
        let tag_lower: String = tag.chars().map(|c| c.to_ascii_lowercase()).collect();
        if let Some(start) = tag_lower.find(&pattern) {
            let value_start = start + pattern.len();
            if let Some(end) = tag[value_start..].find('"') {
                return Some(String::from(&tag[value_start..value_start + end]));
            }
        }
        None
    }

    fn extract_until_static(chars: &[char], pos: &mut usize, end_tag: &str) -> Option<String> {
        let start = *pos;
        let end_lower: String = end_tag.chars().map(|c| c.to_ascii_lowercase()).collect();

        while *pos < chars.len() {
            let remaining: String = chars[*pos..].iter().collect();
            let remaining_lower: String = remaining.chars().map(|c| c.to_ascii_lowercase()).collect();
            if remaining_lower.starts_with(&end_lower) {
                let content: String = chars[start..*pos].iter().collect();
                *pos += end_tag.len();
                return Some(content);
            }
            *pos += 1;
        }
        None
    }

    fn extract_until_any_static(chars: &[char], pos: &mut usize, end_tags: &[&str]) -> Option<String> {
        let start = *pos;

        while *pos < chars.len() {
            let remaining: String = chars[*pos..].iter().collect();
            let remaining_lower: String = remaining.chars().map(|c| c.to_ascii_lowercase()).collect();

            for end_tag in end_tags {
                let end_lower: String = end_tag.chars().map(|c| c.to_ascii_lowercase()).collect();
                if remaining_lower.starts_with(&end_lower) {
                    let content: String = chars[start..*pos].iter().collect();
                    *pos += end_tag.len();
                    return Some(content);
                }
            }
            *pos += 1;
        }
        None
    }

    fn strip_tags_static(text: &str) -> String {
        let mut result = String::new();
        let mut in_tag = false;

        for c in text.chars() {
            if c == '<' {
                in_tag = true;
            } else if c == '>' {
                in_tag = false;
            } else if !in_tag {
                result.push(c);
            }
        }

        result.replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&nbsp;", " ")
    }

    fn do_render(&mut self) {
        self.rendered.clear();

        let content_width = self.bounds.width.saturating_sub(20);
        let char_width = 8;
        let line_height = 18;
        let mut y: isize = 0;

        // Clone elements to avoid borrow issues
        let elements = self.elements.clone();

        for element in &elements {
            match element {
                HtmlElement::Text(text) => {
                    Self::render_text_to(&mut self.rendered, text, &mut y, content_width, char_width, line_height,
                                         Color::BLACK, None);
                }
                HtmlElement::Heading(level, text) => {
                    y += 10;
                    let size_mult = if *level == 1 { 2 } else { 1 };
                    let h_line_height = line_height * size_mult;
                    Self::render_text_to(&mut self.rendered, text, &mut y, content_width, char_width * size_mult,
                                         h_line_height, Color::BLACK, None);
                    y += 5;
                }
                HtmlElement::Paragraph => {
                    y += line_height as isize;
                }
                HtmlElement::Break => {
                    y += line_height as isize;
                }
                HtmlElement::Link(href, text) => {
                    Self::render_text_to(&mut self.rendered, text, &mut y, content_width, char_width, line_height,
                                         Color::new(0, 0, 200), Some(href.clone()));
                }
                HtmlElement::Bold(text) => {
                    Self::render_text_to(&mut self.rendered, text, &mut y, content_width, char_width, line_height,
                                         Color::BLACK, None);
                }
                HtmlElement::Italic(text) => {
                    Self::render_text_to(&mut self.rendered, text, &mut y, content_width, char_width, line_height,
                                         Color::new(80, 80, 80), None);
                }
                HtmlElement::ListItem(text) => {
                    let bullet_text = format!("  * {}", text);
                    Self::render_text_to(&mut self.rendered, &bullet_text, &mut y, content_width, char_width, line_height,
                                         Color::BLACK, None);
                }
                HtmlElement::HorizontalRule => {
                    y += 5;
                    let mut line_text = String::new();
                    for _ in 0..(content_width / char_width) {
                        line_text.push('-');
                    }
                    self.rendered.push(RenderLine {
                        y,
                        elements: vec![RenderElement {
                            x: 0,
                            width: content_width,
                            text: line_text,
                            color: Color::new(180, 180, 180),
                            link: None,
                        }],
                        height: 2,
                    });
                    y += 7;
                }
                HtmlElement::Pre(text) | HtmlElement::Code(text) => {
                    for line in text.lines() {
                        self.rendered.push(RenderLine {
                            y,
                            elements: vec![RenderElement {
                                x: 10,
                                width: line.len() * char_width,
                                text: String::from(line),
                                color: Color::new(60, 60, 60),
                                link: None,
                            }],
                            height: line_height,
                        });
                        y += line_height as isize;
                    }
                }
                HtmlElement::Image(_, alt) => {
                    Self::render_text_to(&mut self.rendered, &format!("[{}]", alt), &mut y, content_width, char_width,
                                         line_height, Color::new(100, 100, 100), None);
                }
            }
        }

        self.content_height = y as usize + line_height;
    }

    fn render_text_to(rendered: &mut Vec<RenderLine>, text: &str, y: &mut isize, max_width: usize, char_width: usize,
                      line_height: usize, color: Color, link: Option<String>) {
        let words: Vec<&str> = text.split_whitespace().collect();
        let mut line_elements: Vec<RenderElement> = Vec::new();
        let mut current_x: isize = 10;
        let space_width = char_width;

        for word in words {
            let word_width = word.len() * char_width;

            if current_x as usize + word_width > max_width && !line_elements.is_empty() {
                rendered.push(RenderLine {
                    y: *y,
                    elements: line_elements,
                    height: line_height,
                });
                *y += line_height as isize;
                line_elements = Vec::new();
                current_x = 10;
            }

            line_elements.push(RenderElement {
                x: current_x,
                width: word_width,
                text: String::from(word),
                color,
                link: link.clone(),
            });
            current_x += word_width as isize + space_width as isize;
        }

        if !line_elements.is_empty() {
            rendered.push(RenderLine {
                y: *y,
                elements: line_elements,
                height: line_height,
            });
            *y += line_height as isize;
        }
    }

    fn go_back(&mut self) {
        if self.history_index > 0 {
            self.history_index -= 1;
            let url = self.history[self.history_index].clone();
            self.url = url.clone();
            self.url_input = url.clone();
            self.url_cursor = self.url_input.len();
            self.load_url(&url);
        }
    }

    fn go_forward(&mut self) {
        if self.history_index + 1 < self.history.len() {
            self.history_index += 1;
            let url = self.history[self.history_index].clone();
            self.url = url.clone();
            self.url_input = url.clone();
            self.url_cursor = self.url_input.len();
            self.load_url(&url);
        }
    }

    fn refresh(&mut self) {
        let url = self.url.clone();
        self.load_url(&url);
    }

    fn go_home(&mut self) {
        self.navigate(HOME_PAGE);
    }

    fn handle_url_key(&mut self, key: char) {
        match key {
            '\x08' => {
                if self.url_cursor > 0 {
                    self.url_input.remove(self.url_cursor - 1);
                    self.url_cursor -= 1;
                }
            }
            '\n' | '\r' => {
                let url = self.url_input.clone();
                self.navigate(&url);
                self.url_focused = false;
            }
            '\x1b' => {
                self.url_input = self.url.clone();
                self.url_cursor = self.url_input.len();
                self.url_focused = false;
            }
            c if c.is_ascii() && !c.is_control() => {
                self.url_input.insert(self.url_cursor, c);
                self.url_cursor += 1;
            }
            _ => {}
        }
    }

    fn is_in_content(&self, x: isize, y: isize) -> bool {
        let content_top = self.bounds.y + 36;
        let content_bottom = self.bounds.y + self.bounds.height as isize - 20;
        y >= content_top && y < content_bottom
    }

    fn find_link_at(&self, x: isize, y: isize) -> Option<String> {
        let content_x = x - self.bounds.x;
        let content_y = y - (self.bounds.y + 36) + self.scroll_y;

        for line in &self.rendered {
            if content_y >= line.y && content_y < line.y + line.height as isize {
                for elem in &line.elements {
                    if content_x >= elem.x && content_x < elem.x + elem.width as isize {
                        if let Some(ref link) = elem.link {
                            return Some(link.clone());
                        }
                    }
                }
            }
        }
        None
    }

    fn resolve_url(&self, href: &str) -> String {
        if href.starts_with("http://") || href.starts_with("https://") || href.starts_with("about:") {
            return String::from(href);
        }

        if let Some(base_end) = self.url.rfind('/') {
            if self.url[..base_end].contains("://") {
                let after_scheme = self.url.find("://").unwrap_or(0) + 3;
                if base_end > after_scheme {
                    return format!("{}/{}", &self.url[..base_end], href.trim_start_matches('/'));
                }
            }
        }

        if self.url.ends_with('/') {
            format!("{}{}", self.url, href)
        } else {
            format!("{}/{}", self.url, href)
        }
    }
}

impl Widget for Browser {
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
        self.do_render();
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
            WidgetEvent::MouseMove { x, y } => {
                for btn in &mut self.toolbar_buttons {
                    btn.hovered = btn.bounds.contains(*x, *y);
                }

                if self.is_in_content(*x, *y) {
                    self.hovered_link = self.find_link_at(*x, *y);
                    if let Some(ref link) = self.hovered_link {
                        self.status = link.clone();
                    } else {
                        self.status.clear();
                    }
                }
                true
            }
            WidgetEvent::MouseDown { x, y, button: MouseButton::Left } => {
                for btn in &self.toolbar_buttons {
                    if btn.bounds.contains(*x, *y) && btn.enabled {
                        match btn.action {
                            NavAction::Back => self.go_back(),
                            NavAction::Forward => self.go_forward(),
                            NavAction::Refresh => self.refresh(),
                            NavAction::Home => self.go_home(),
                        }
                        return true;
                    }
                }

                let url_bar = Bounds::new(
                    self.bounds.x + 156,
                    self.bounds.y + 4,
                    self.bounds.width.saturating_sub(166),
                    28
                );
                if url_bar.contains(*x, *y) {
                    self.url_focused = true;
                    let click_x = x - url_bar.x - 4;
                    let char_pos = (click_x / 8) as usize;
                    self.url_cursor = min(char_pos, self.url_input.len());
                    return true;
                } else {
                    self.url_focused = false;
                }

                if self.is_in_content(*x, *y) {
                    if let Some(href) = self.find_link_at(*x, *y) {
                        let resolved = self.resolve_url(&href);
                        self.navigate(&resolved);
                        return true;
                    }
                }

                true
            }
            WidgetEvent::Scroll { delta_y, .. } => {
                let max_scroll = (self.content_height as isize)
                    .saturating_sub((self.bounds.height as isize) - 56);
                self.scroll_y = (self.scroll_y - *delta_y as isize * 20)
                    .max(0)
                    .min(max_scroll.max(0));
                true
            }
            WidgetEvent::Character { c } => {
                if self.url_focused {
                    self.handle_url_key(*c);
                    true
                } else {
                    false
                }
            }
            _ => false
        }
    }

    fn render(&self, surface: &mut Surface) {
        if !self.visible {
            return;
        }

        let x = self.bounds.x;
        let y = self.bounds.y;
        let w = self.bounds.width;
        let h = self.bounds.height;

        fill_rect_safe(surface, x, y, w, h, Color::WHITE);
        fill_rect_safe(surface, x, y, w, 36, Color::new(240, 240, 240));

        for btn in &self.toolbar_buttons {
            let bg_color = if btn.hovered && btn.enabled {
                Color::new(220, 220, 220)
            } else {
                Color::new(240, 240, 240)
            };
            let fg_color = if btn.enabled {
                Color::BLACK
            } else {
                Color::new(160, 160, 160)
            };

            fill_rect_safe(surface, btn.bounds.x, btn.bounds.y, btn.bounds.width, btn.bounds.height, bg_color);
            draw_rect_safe(surface, btn.bounds.x, btn.bounds.y, btn.bounds.width, btn.bounds.height,
                           Color::new(180, 180, 180));

            let icon_str: String = [btn.icon].iter().collect();
            draw_string(surface, btn.bounds.x + 10, btn.bounds.y + 6, &icon_str, fg_color);
        }

        let url_x = x + 156;
        let url_y = y + 4;
        let url_w = w.saturating_sub(166);
        let url_h = 28;

        let url_bg = if self.url_focused {
            Color::WHITE
        } else {
            Color::new(250, 250, 250)
        };
        fill_rect_safe(surface, url_x, url_y, url_w, url_h, url_bg);
        draw_rect_safe(surface, url_x, url_y, url_w, url_h,
                       if self.url_focused { Color::new(100, 150, 255) } else { Color::new(200, 200, 200) });

        let visible_url = if self.url_input.len() * 8 > url_w - 8 {
            let start = self.url_input.len().saturating_sub((url_w - 8) / 8);
            &self.url_input[start..]
        } else {
            &self.url_input
        };
        draw_string(surface, url_x + 4, url_y + 6, visible_url, Color::BLACK);

        if self.url_focused {
            let cursor_x = url_x + 4 + (self.url_cursor * 8) as isize;
            if cursor_x < url_x + url_w as isize - 4 {
                surface.draw_line(cursor_x, url_y + 4, cursor_x, url_y + url_h as isize - 4, Color::BLACK);
            }
        }

        let content_y = y + 36;
        let content_h = h.saturating_sub(56);

        for line in &self.rendered {
            let line_y = content_y + line.y - self.scroll_y;

            if (line_y + line.height as isize) < content_y || line_y > (content_y + content_h as isize) {
                continue;
            }

            for elem in &line.elements {
                let elem_x = x + elem.x;

                let color = if elem.link.is_some() && self.hovered_link == elem.link {
                    Color::new(100, 0, 200)
                } else {
                    elem.color
                };

                draw_string(surface, elem_x, line_y, &elem.text, color);

                if elem.link.is_some() {
                    let underline_y = line_y + 14;
                    surface.draw_line(elem_x, underline_y, elem_x + elem.width as isize, underline_y, color);
                }
            }
        }

        let status_y = y + h as isize - 20;
        fill_rect_safe(surface, x, status_y, w, 20, Color::new(245, 245, 245));
        surface.draw_line(x, status_y, x + w as isize, status_y, Color::new(220, 220, 220));

        let status_text = if self.loading_state == LoadingState::Loading {
            &self.status
        } else if let Some(ref link) = self.hovered_link {
            link
        } else {
            &self.status
        };
        draw_string(surface, x + 4, status_y + 3, status_text, Color::new(100, 100, 100));

        if self.loading_state == LoadingState::Loading {
            draw_string(surface, x + w as isize - 80, status_y + 3, "Loading...", Color::new(100, 100, 100));
        }
    }
}
