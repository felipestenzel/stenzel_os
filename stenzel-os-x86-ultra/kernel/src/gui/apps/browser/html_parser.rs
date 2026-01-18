//! HTML Parser
//!
//! HTML5 compliant parser for web pages.

use alloc::string::String;
use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use super::dom::{Dom, DomNode, DomElement, DomText, DomNodeType};

/// HTML parser
pub struct HtmlParser {
    /// Input HTML
    input: String,
    /// Current position
    pos: usize,
    /// Current line number
    line: usize,
    /// Current column
    column: usize,
}

impl HtmlParser {
    /// Create new parser
    pub fn new(html: &str) -> Self {
        Self {
            input: String::from(html),
            pos: 0,
            line: 1,
            column: 1,
        }
    }

    /// Parse HTML into DOM
    pub fn parse(&mut self) -> HtmlDocument {
        let mut doc = HtmlDocument::new();
        let mut dom = Dom::new();

        // Parse into tree
        let root = self.parse_nodes();

        // Find title
        if let Some(title_node) = find_element(&root, "title") {
            doc.title = title_node.text_content();
            dom.title = doc.title.clone();
        }

        // Find html element
        for node in &root {
            if node.tag_name() == Some("html") {
                dom.set_root(node.clone());
                break;
            }
        }

        // If no html element, create one
        if dom.root.is_none() {
            let mut html = DomNode::element("html");
            let mut body = DomNode::element("body");
            for node in root {
                body.add_child(node);
            }
            html.add_child(body);
            dom.set_root(html);
        }

        doc.dom = dom;
        doc
    }

    fn parse_nodes(&mut self) -> Vec<DomNode> {
        let mut nodes = Vec::new();

        loop {
            self.skip_whitespace();

            if self.eof() {
                break;
            }

            // Check for comment
            if self.starts_with("<!--") {
                if let Some(comment) = self.parse_comment() {
                    nodes.push(comment);
                }
                continue;
            }

            // Check for doctype
            if self.starts_with("<!DOCTYPE") || self.starts_with("<!doctype") {
                self.parse_doctype();
                continue;
            }

            // Check for closing tag
            if self.starts_with("</") {
                break;
            }

            // Check for opening tag
            if self.starts_with("<") {
                if let Some(element) = self.parse_element() {
                    nodes.push(element);
                }
                continue;
            }

            // Must be text
            if let Some(text) = self.parse_text() {
                if !text.text.as_ref().map(|t| t.is_whitespace).unwrap_or(true) {
                    nodes.push(text);
                }
            }
        }

        nodes
    }

    fn parse_element(&mut self) -> Option<DomNode> {
        // Consume '<'
        self.consume_char();

        // Parse tag name
        let tag_name = self.parse_tag_name();
        if tag_name.is_empty() {
            return None;
        }

        let mut node = DomNode::element(&tag_name);

        // Parse attributes
        loop {
            self.skip_whitespace();

            if self.eof() || self.starts_with(">") || self.starts_with("/>") {
                break;
            }

            if let Some((name, value)) = self.parse_attribute() {
                node.set_attribute(&name, &value);

                // Parse inline style
                if name == "style" {
                    if let Some(element) = &mut node.element {
                        parse_inline_style(&value, &mut element.style);
                    }
                }
            }
        }

        // Check for self-closing or void element
        let is_void = node.element.as_ref().map(|e| e.is_void()).unwrap_or(false);
        let self_closing = self.starts_with("/>");

        if self_closing {
            self.consume_char(); // '/'
            self.consume_char(); // '>'
            return Some(node);
        }

        // Consume '>'
        if self.starts_with(">") {
            self.consume_char();
        }

        // Void elements have no children
        if is_void {
            return Some(node);
        }

        // Special handling for script and style
        if tag_name == "script" || tag_name == "style" {
            let content = self.parse_raw_text(&tag_name);
            if !content.is_empty() {
                node.add_child(DomNode::text(&content));
            }
            return Some(node);
        }

        // Parse children
        let children = self.parse_nodes();
        for child in children {
            node.add_child(child);
        }

        // Consume closing tag
        self.parse_closing_tag(&tag_name);

        Some(node)
    }

    fn parse_tag_name(&mut self) -> String {
        let mut name = String::new();
        while !self.eof() {
            let c = self.current_char();
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ':' {
                name.push(self.consume_char());
            } else {
                break;
            }
        }
        name.to_lowercase()
    }

    fn parse_attribute(&mut self) -> Option<(String, String)> {
        let name = self.parse_attribute_name();
        if name.is_empty() {
            // Skip invalid character
            if !self.eof() {
                self.consume_char();
            }
            return None;
        }

        self.skip_whitespace();

        // Check for '='
        if !self.starts_with("=") {
            // Boolean attribute
            return Some((name, String::new()));
        }

        self.consume_char(); // '='
        self.skip_whitespace();

        let value = self.parse_attribute_value();
        Some((name, value))
    }

    fn parse_attribute_name(&mut self) -> String {
        let mut name = String::new();
        while !self.eof() {
            let c = self.current_char();
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ':' || c == '.' {
                name.push(self.consume_char());
            } else {
                break;
            }
        }
        name.to_lowercase()
    }

    fn parse_attribute_value(&mut self) -> String {
        // Check for quoted value
        if self.starts_with("\"") {
            self.consume_char();
            let value = self.consume_until('"');
            self.consume_char(); // closing quote
            return decode_entities(&value);
        }

        if self.starts_with("'") {
            self.consume_char();
            let value = self.consume_until('\'');
            self.consume_char(); // closing quote
            return decode_entities(&value);
        }

        // Unquoted value
        let mut value = String::new();
        while !self.eof() {
            let c = self.current_char();
            if c.is_whitespace() || c == '>' || c == '/' {
                break;
            }
            value.push(self.consume_char());
        }
        decode_entities(&value)
    }

    fn parse_closing_tag(&mut self, expected: &str) {
        if !self.starts_with("</") {
            return;
        }

        self.consume_char(); // '<'
        self.consume_char(); // '/'

        let tag_name = self.parse_tag_name();

        // Consume rest of tag
        while !self.eof() && !self.starts_with(">") {
            self.consume_char();
        }

        if self.starts_with(">") {
            self.consume_char();
        }
    }

    fn parse_text(&mut self) -> Option<DomNode> {
        let mut text = String::new();

        while !self.eof() && !self.starts_with("<") {
            text.push(self.consume_char());
        }

        if text.is_empty() {
            return None;
        }

        let decoded = decode_entities(&text);
        Some(DomNode::text(&decoded))
    }

    fn parse_comment(&mut self) -> Option<DomNode> {
        // Consume "<!--"
        for _ in 0..4 {
            self.consume_char();
        }

        let mut content = String::new();

        while !self.eof() {
            if self.starts_with("-->") {
                for _ in 0..3 {
                    self.consume_char();
                }
                break;
            }
            content.push(self.consume_char());
        }

        Some(DomNode::comment(&content))
    }

    fn parse_doctype(&mut self) {
        // Just consume until '>'
        while !self.eof() && !self.starts_with(">") {
            self.consume_char();
        }
        if self.starts_with(">") {
            self.consume_char();
        }
    }

    fn parse_raw_text(&mut self, tag: &str) -> String {
        let mut content = String::new();
        let end_tag = alloc::format!("</{}", tag);

        while !self.eof() {
            if self.starts_with_insensitive(&end_tag) {
                break;
            }
            content.push(self.consume_char());
        }

        // Consume closing tag
        self.parse_closing_tag(tag);

        content
    }

    fn current_char(&self) -> char {
        self.input[self.pos..].chars().next().unwrap_or('\0')
    }

    fn consume_char(&mut self) -> char {
        let c = self.current_char();
        self.pos += c.len_utf8();

        if c == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }

        c
    }

    fn consume_until(&mut self, end: char) -> String {
        let mut result = String::new();
        while !self.eof() && self.current_char() != end {
            result.push(self.consume_char());
        }
        result
    }

    fn skip_whitespace(&mut self) {
        while !self.eof() && self.current_char().is_whitespace() {
            self.consume_char();
        }
    }

    fn eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn starts_with(&self, s: &str) -> bool {
        self.input[self.pos..].starts_with(s)
    }

    fn starts_with_insensitive(&self, s: &str) -> bool {
        let remaining = &self.input[self.pos..];
        if remaining.len() < s.len() {
            return false;
        }
        remaining[..s.len()].eq_ignore_ascii_case(s)
    }
}

/// HTML document
pub struct HtmlDocument {
    /// DOM tree
    pub dom: Dom,
    /// Document title
    pub title: String,
    /// Document charset
    pub charset: String,
    /// Base URL
    pub base_url: String,
    /// Links
    pub links: Vec<HtmlLink>,
    /// Scripts
    pub scripts: Vec<HtmlScript>,
    /// Stylesheets
    pub stylesheets: Vec<HtmlStylesheet>,
}

impl HtmlDocument {
    /// Create new document
    pub fn new() -> Self {
        Self {
            dom: Dom::new(),
            title: String::new(),
            charset: String::from("UTF-8"),
            base_url: String::new(),
            links: Vec::new(),
            scripts: Vec::new(),
            stylesheets: Vec::new(),
        }
    }

    /// Get body element
    pub fn body(&self) -> Option<&DomNode> {
        self.dom.root.as_ref().and_then(|root| {
            for child in &root.children {
                if child.tag_name() == Some("body") {
                    return Some(child);
                }
            }
            None
        })
    }

    /// Get head element
    pub fn head(&self) -> Option<&DomNode> {
        self.dom.root.as_ref().and_then(|root| {
            for child in &root.children {
                if child.tag_name() == Some("head") {
                    return Some(child);
                }
            }
            None
        })
    }
}

impl Default for HtmlDocument {
    fn default() -> Self {
        Self::new()
    }
}

/// HTML element (for API compatibility)
pub type HtmlElement = DomElement;

/// HTML node (for API compatibility)
pub type HtmlNode = DomNode;

/// HTML link
#[derive(Debug, Clone)]
pub struct HtmlLink {
    /// Link href
    pub href: String,
    /// Link rel
    pub rel: String,
    /// Link type
    pub link_type: String,
}

/// HTML script
#[derive(Debug, Clone)]
pub struct HtmlScript {
    /// Script src
    pub src: Option<String>,
    /// Script content
    pub content: String,
    /// Is async
    pub is_async: bool,
    /// Is defer
    pub is_defer: bool,
    /// Script type
    pub script_type: String,
}

/// HTML stylesheet
#[derive(Debug, Clone)]
pub struct HtmlStylesheet {
    /// Stylesheet href
    pub href: Option<String>,
    /// Stylesheet content
    pub content: String,
    /// Media query
    pub media: String,
}

// Helper functions

fn find_element<'a>(nodes: &'a [DomNode], tag: &str) -> Option<&'a DomNode> {
    for node in nodes {
        if node.tag_name() == Some(tag) {
            return Some(node);
        }
        if let Some(found) = find_element(&node.children, tag) {
            return Some(found);
        }
    }
    None
}

fn decode_entities(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if c != '&' {
            result.push(c);
            continue;
        }

        // Collect entity
        let mut entity = String::new();
        while let Some(&c) = chars.peek() {
            if c == ';' {
                chars.next();
                break;
            }
            if c.is_whitespace() || c == '&' {
                break;
            }
            entity.push(chars.next().unwrap());
            if entity.len() > 10 {
                break;
            }
        }

        // Decode entity
        let decoded = match entity.as_str() {
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "quot" => '"',
            "apos" => '\'',
            "nbsp" => '\u{00A0}',
            "copy" => '\u{00A9}',
            "reg" => '\u{00AE}',
            "trade" => '\u{2122}',
            "mdash" => '\u{2014}',
            "ndash" => '\u{2013}',
            "lsquo" => '\u{2018}',
            "rsquo" => '\u{2019}',
            "ldquo" => '\u{201C}',
            "rdquo" => '\u{201D}',
            "bull" => '\u{2022}',
            "hellip" => '\u{2026}',
            s if s.starts_with('#') => {
                let num = if s.starts_with("#x") || s.starts_with("#X") {
                    u32::from_str_radix(&s[2..], 16).ok()
                } else {
                    s[1..].parse().ok()
                };
                if let Some(n) = num {
                    char::from_u32(n).unwrap_or('\u{FFFD}')
                } else {
                    result.push('&');
                    result.push_str(&entity);
                    result.push(';');
                    continue;
                }
            }
            _ => {
                result.push('&');
                result.push_str(&entity);
                result.push(';');
                continue;
            }
        };

        result.push(decoded);
    }

    result
}

fn parse_inline_style(style: &str, map: &mut BTreeMap<String, String>) {
    for declaration in style.split(';') {
        let declaration = declaration.trim();
        if declaration.is_empty() {
            continue;
        }

        if let Some(colon_pos) = declaration.find(':') {
            let property = declaration[..colon_pos].trim().to_lowercase();
            let value = declaration[colon_pos + 1..].trim();
            map.insert(property, String::from(value));
        }
    }
}
