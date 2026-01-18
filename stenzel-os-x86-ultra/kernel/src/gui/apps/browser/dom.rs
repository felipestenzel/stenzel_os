//! Document Object Model (DOM)
//!
//! DOM tree representation for HTML documents.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

/// DOM tree
pub struct Dom {
    /// Root node
    pub root: Option<DomNode>,
    /// Document title
    pub title: String,
    /// Base URL
    pub base_url: String,
}

impl Dom {
    /// Create new empty DOM
    pub fn new() -> Self {
        Self {
            root: None,
            title: String::new(),
            base_url: String::new(),
        }
    }

    /// Set document root
    pub fn set_root(&mut self, node: DomNode) {
        self.root = Some(node);
    }

    /// Get element by ID
    pub fn get_element_by_id(&self, id: &str) -> Option<&DomNode> {
        self.root.as_ref().and_then(|root| find_by_id(root, id))
    }

    /// Get elements by tag name
    pub fn get_elements_by_tag_name(&self, tag: &str) -> Vec<&DomNode> {
        let mut results = Vec::new();
        if let Some(root) = &self.root {
            find_by_tag(root, tag, &mut results);
        }
        results
    }

    /// Get elements by class name
    pub fn get_elements_by_class_name(&self, class: &str) -> Vec<&DomNode> {
        let mut results = Vec::new();
        if let Some(root) = &self.root {
            find_by_class(root, class, &mut results);
        }
        results
    }

    /// Query selector (simple CSS selector)
    pub fn query_selector(&self, selector: &str) -> Option<&DomNode> {
        self.root.as_ref().and_then(|root| query_select(root, selector))
    }

    /// Query selector all
    pub fn query_selector_all(&self, selector: &str) -> Vec<&DomNode> {
        let mut results = Vec::new();
        if let Some(root) = &self.root {
            query_select_all(root, selector, &mut results);
        }
        results
    }
}

impl Default for Dom {
    fn default() -> Self {
        Self::new()
    }
}

/// DOM node type
#[derive(Debug, Clone, PartialEq)]
pub enum DomNodeType {
    /// Element node
    Element,
    /// Text node
    Text,
    /// Comment node
    Comment,
    /// Document node
    Document,
    /// Document type node
    DocumentType,
    /// CDATA section
    CDataSection,
    /// Processing instruction
    ProcessingInstruction,
}

/// DOM node
#[derive(Debug, Clone)]
pub struct DomNode {
    /// Node type
    pub node_type: DomNodeType,
    /// Element data (if element)
    pub element: Option<DomElement>,
    /// Text content (if text node)
    pub text: Option<DomText>,
    /// Child nodes
    pub children: Vec<DomNode>,
    /// Parent reference (by index in tree traversal)
    parent_index: Option<usize>,
}

impl DomNode {
    /// Create new element node
    pub fn element(tag: &str) -> Self {
        Self {
            node_type: DomNodeType::Element,
            element: Some(DomElement::new(tag)),
            text: None,
            children: Vec::new(),
            parent_index: None,
        }
    }

    /// Create new text node
    pub fn text(content: &str) -> Self {
        Self {
            node_type: DomNodeType::Text,
            element: None,
            text: Some(DomText::new(content)),
            children: Vec::new(),
            parent_index: None,
        }
    }

    /// Create new comment node
    pub fn comment(content: &str) -> Self {
        Self {
            node_type: DomNodeType::Comment,
            element: None,
            text: Some(DomText::new(content)),
            children: Vec::new(),
            parent_index: None,
        }
    }

    /// Create document node
    pub fn document() -> Self {
        Self {
            node_type: DomNodeType::Document,
            element: None,
            text: None,
            children: Vec::new(),
            parent_index: None,
        }
    }

    /// Add child node
    pub fn add_child(&mut self, child: DomNode) {
        self.children.push(child);
    }

    /// Get tag name
    pub fn tag_name(&self) -> Option<&str> {
        self.element.as_ref().map(|e| e.tag_name.as_str())
    }

    /// Get attribute
    pub fn get_attribute(&self, name: &str) -> Option<&str> {
        self.element
            .as_ref()
            .and_then(|e| e.attributes.get(name))
            .map(|s| s.as_str())
    }

    /// Set attribute
    pub fn set_attribute(&mut self, name: &str, value: &str) {
        if let Some(element) = &mut self.element {
            element.attributes.insert(String::from(name), String::from(value));
        }
    }

    /// Remove attribute
    pub fn remove_attribute(&mut self, name: &str) {
        if let Some(element) = &mut self.element {
            element.attributes.remove(name);
        }
    }

    /// Check if has attribute
    pub fn has_attribute(&self, name: &str) -> bool {
        self.element
            .as_ref()
            .map(|e| e.attributes.contains_key(name))
            .unwrap_or(false)
    }

    /// Get ID
    pub fn id(&self) -> Option<&str> {
        self.get_attribute("id")
    }

    /// Get class list
    pub fn class_list(&self) -> Vec<&str> {
        self.get_attribute("class")
            .map(|c| c.split_whitespace().collect())
            .unwrap_or_default()
    }

    /// Check if has class
    pub fn has_class(&self, class: &str) -> bool {
        self.class_list().contains(&class)
    }

    /// Get text content
    pub fn text_content(&self) -> String {
        let mut content = String::new();
        collect_text(self, &mut content);
        content
    }

    /// Get inner HTML
    pub fn inner_html(&self) -> String {
        let mut html = String::new();
        for child in &self.children {
            serialize_node(child, &mut html);
        }
        html
    }

    /// Get outer HTML
    pub fn outer_html(&self) -> String {
        let mut html = String::new();
        serialize_node(self, &mut html);
        html
    }

    /// Get first child
    pub fn first_child(&self) -> Option<&DomNode> {
        self.children.first()
    }

    /// Get last child
    pub fn last_child(&self) -> Option<&DomNode> {
        self.children.last()
    }

    /// Check if is element
    pub fn is_element(&self) -> bool {
        self.node_type == DomNodeType::Element
    }

    /// Check if is text
    pub fn is_text(&self) -> bool {
        self.node_type == DomNodeType::Text
    }
}

/// DOM element
#[derive(Debug, Clone)]
pub struct DomElement {
    /// Tag name (lowercase)
    pub tag_name: String,
    /// Attributes
    pub attributes: BTreeMap<String, String>,
    /// Namespace URI
    pub namespace_uri: Option<String>,
    /// Inline style
    pub style: BTreeMap<String, String>,
}

impl DomElement {
    /// Create new element
    pub fn new(tag: &str) -> Self {
        Self {
            tag_name: String::from(tag).to_lowercase(),
            attributes: BTreeMap::new(),
            namespace_uri: None,
            style: BTreeMap::new(),
        }
    }

    /// Set style property
    pub fn set_style(&mut self, property: &str, value: &str) {
        self.style.insert(String::from(property), String::from(value));
    }

    /// Get style property
    pub fn get_style(&self, property: &str) -> Option<&str> {
        self.style.get(property).map(|s| s.as_str())
    }

    /// Is void element (self-closing)
    pub fn is_void(&self) -> bool {
        matches!(
            self.tag_name.as_str(),
            "area" | "base" | "br" | "col" | "embed" | "hr" | "img" | "input" |
            "link" | "meta" | "param" | "source" | "track" | "wbr"
        )
    }

    /// Is block element
    pub fn is_block(&self) -> bool {
        matches!(
            self.tag_name.as_str(),
            "address" | "article" | "aside" | "blockquote" | "canvas" | "dd" |
            "div" | "dl" | "dt" | "fieldset" | "figcaption" | "figure" | "footer" |
            "form" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "header" | "hr" |
            "li" | "main" | "nav" | "noscript" | "ol" | "p" | "pre" | "section" |
            "table" | "tfoot" | "ul" | "video"
        )
    }

    /// Is inline element
    pub fn is_inline(&self) -> bool {
        !self.is_block() && !self.is_void()
    }
}

/// DOM text node
#[derive(Debug, Clone)]
pub struct DomText {
    /// Text content
    pub content: String,
    /// Is whitespace only
    pub is_whitespace: bool,
}

impl DomText {
    /// Create new text node
    pub fn new(content: &str) -> Self {
        Self {
            content: String::from(content),
            is_whitespace: content.chars().all(|c| c.is_whitespace()),
        }
    }

    /// Set content
    pub fn set_content(&mut self, content: &str) {
        self.content = String::from(content);
        self.is_whitespace = content.chars().all(|c| c.is_whitespace());
    }

    /// Get length
    pub fn len(&self) -> usize {
        self.content.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }
}

// Helper functions

fn find_by_id<'a>(node: &'a DomNode, id: &str) -> Option<&'a DomNode> {
    if node.id() == Some(id) {
        return Some(node);
    }
    for child in &node.children {
        if let Some(found) = find_by_id(child, id) {
            return Some(found);
        }
    }
    None
}

fn find_by_tag<'a>(node: &'a DomNode, tag: &str, results: &mut Vec<&'a DomNode>) {
    if node.tag_name() == Some(tag) {
        results.push(node);
    }
    for child in &node.children {
        find_by_tag(child, tag, results);
    }
}

fn find_by_class<'a>(node: &'a DomNode, class: &str, results: &mut Vec<&'a DomNode>) {
    if node.has_class(class) {
        results.push(node);
    }
    for child in &node.children {
        find_by_class(child, class, results);
    }
}

fn query_select<'a>(node: &'a DomNode, selector: &str) -> Option<&'a DomNode> {
    if matches_selector(node, selector) {
        return Some(node);
    }
    for child in &node.children {
        if let Some(found) = query_select(child, selector) {
            return Some(found);
        }
    }
    None
}

fn query_select_all<'a>(node: &'a DomNode, selector: &str, results: &mut Vec<&'a DomNode>) {
    if matches_selector(node, selector) {
        results.push(node);
    }
    for child in &node.children {
        query_select_all(child, selector, results);
    }
}

fn matches_selector(node: &DomNode, selector: &str) -> bool {
    if !node.is_element() {
        return false;
    }

    // ID selector
    if selector.starts_with('#') {
        return node.id() == Some(&selector[1..]);
    }

    // Class selector
    if selector.starts_with('.') {
        return node.has_class(&selector[1..]);
    }

    // Tag selector
    node.tag_name() == Some(selector)
}

fn collect_text(node: &DomNode, content: &mut String) {
    if let Some(text) = &node.text {
        content.push_str(&text.content);
    }
    for child in &node.children {
        collect_text(child, content);
    }
}

fn serialize_node(node: &DomNode, html: &mut String) {
    match node.node_type {
        DomNodeType::Element => {
            if let Some(element) = &node.element {
                html.push('<');
                html.push_str(&element.tag_name);

                for (name, value) in &element.attributes {
                    html.push(' ');
                    html.push_str(name);
                    html.push_str("=\"");
                    html.push_str(&escape_html(value));
                    html.push('"');
                }

                if element.is_void() {
                    html.push_str(" />");
                } else {
                    html.push('>');
                    for child in &node.children {
                        serialize_node(child, html);
                    }
                    html.push_str("</");
                    html.push_str(&element.tag_name);
                    html.push('>');
                }
            }
        }
        DomNodeType::Text => {
            if let Some(text) = &node.text {
                html.push_str(&escape_html(&text.content));
            }
        }
        DomNodeType::Comment => {
            if let Some(text) = &node.text {
                html.push_str("<!--");
                html.push_str(&text.content);
                html.push_str("-->");
            }
        }
        _ => {
            for child in &node.children {
                serialize_node(child, html);
            }
        }
    }
}

fn escape_html(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '&' => result.push_str("&amp;"),
            '"' => result.push_str("&quot;"),
            '\'' => result.push_str("&#39;"),
            _ => result.push(c),
        }
    }
    result
}

/// Node iterator for depth-first traversal
pub struct NodeIterator<'a> {
    stack: Vec<&'a DomNode>,
}

impl<'a> NodeIterator<'a> {
    /// Create new iterator starting from node
    pub fn new(root: &'a DomNode) -> Self {
        Self { stack: vec![root] }
    }
}

impl<'a> Iterator for NodeIterator<'a> {
    type Item = &'a DomNode;

    fn next(&mut self) -> Option<Self::Item> {
        self.stack.pop().map(|node| {
            // Add children in reverse order so first child is processed first
            for child in node.children.iter().rev() {
                self.stack.push(child);
            }
            node
        })
    }
}
