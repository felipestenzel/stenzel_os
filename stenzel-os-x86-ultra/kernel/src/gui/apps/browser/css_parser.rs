//! CSS Parser
//!
//! CSS3 parser for stylesheets.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use core::str::FromStr;

/// CSS parser
pub struct CssParser {
    /// Input CSS
    input: String,
    /// Current position
    pos: usize,
}

impl CssParser {
    /// Create new parser
    pub fn new(css: &str) -> Self {
        Self {
            input: String::from(css),
            pos: 0,
        }
    }

    /// Parse CSS into stylesheet
    pub fn parse(&mut self) -> StyleSheet {
        let mut stylesheet = StyleSheet::new();

        self.skip_whitespace_and_comments();

        while !self.eof() {
            // Check for at-rule
            if self.starts_with("@") {
                if let Some(at_rule) = self.parse_at_rule() {
                    match at_rule.name.as_str() {
                        "import" => {
                            stylesheet.imports.push(at_rule.prelude.clone());
                        }
                        "media" => {
                            // Parse media query rules
                            let media_query = at_rule.prelude.clone();
                            if let Some(content) = at_rule.content {
                                let mut inner_parser = CssParser::new(&content);
                                let inner_rules = inner_parser.parse_rules();
                                for mut rule in inner_rules {
                                    rule.media_query = Some(media_query.clone());
                                    stylesheet.rules.push(rule);
                                }
                            }
                        }
                        "keyframes" | "-webkit-keyframes" => {
                            if let Some(content) = at_rule.content {
                                let keyframes = self.parse_keyframes(&at_rule.prelude, &content);
                                stylesheet.keyframes.push(keyframes);
                            }
                        }
                        "font-face" => {
                            if let Some(content) = at_rule.content {
                                let declarations = self.parse_declarations(&content);
                                stylesheet.font_faces.push(FontFace { declarations });
                            }
                        }
                        _ => {}
                    }
                }
            } else {
                // Parse rule
                if let Some(rule) = self.parse_rule() {
                    stylesheet.rules.push(rule);
                }
            }

            self.skip_whitespace_and_comments();
        }

        stylesheet
    }

    fn parse_rules(&mut self) -> Vec<CssRule> {
        let mut rules = Vec::new();

        self.skip_whitespace_and_comments();

        while !self.eof() {
            if let Some(rule) = self.parse_rule() {
                rules.push(rule);
            }
            self.skip_whitespace_and_comments();
        }

        rules
    }

    fn parse_rule(&mut self) -> Option<CssRule> {
        self.skip_whitespace_and_comments();

        // Parse selectors
        let selectors_str = self.consume_until('{');
        if selectors_str.is_empty() {
            return None;
        }

        // Consume '{'
        if !self.consume_if('{') {
            return None;
        }

        // Parse declarations
        let declarations_str = self.consume_until('}');
        let declarations = self.parse_declarations(&declarations_str);

        // Consume '}'
        self.consume_if('}');

        // Parse selectors
        let selectors = parse_selectors(&selectors_str);

        Some(CssRule {
            selectors,
            declarations,
            media_query: None,
        })
    }

    fn parse_at_rule(&mut self) -> Option<AtRule> {
        // Consume '@'
        self.consume_char();

        // Parse name
        let name = self.parse_identifier();
        self.skip_whitespace();

        // Parse prelude (everything before '{' or ';')
        let mut prelude = String::new();
        while !self.eof() && !self.starts_with("{") && !self.starts_with(";") {
            prelude.push(self.consume_char());
        }
        let prelude = prelude.trim().to_string();

        // Check for content block
        let content = if self.starts_with("{") {
            self.consume_char(); // '{'
            let content = self.consume_balanced_braces();
            self.consume_if('}');
            Some(content)
        } else {
            self.consume_if(';');
            None
        };

        Some(AtRule {
            name,
            prelude,
            content,
        })
    }

    fn parse_declarations(&self, input: &str) -> Vec<CssDeclaration> {
        let mut declarations = Vec::new();

        for part in input.split(';') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }

            if let Some(colon_pos) = part.find(':') {
                let property = part[..colon_pos].trim().to_lowercase();
                let mut value = part[colon_pos + 1..].trim().to_string();

                // Check for !important
                let important = if value.to_lowercase().ends_with("!important") {
                    value = value[..value.len() - 10].trim().to_string();
                    true
                } else {
                    false
                };

                declarations.push(CssDeclaration {
                    property,
                    value,
                    important,
                });
            }
        }

        declarations
    }

    fn parse_keyframes(&self, name: &str, content: &str) -> Keyframes {
        let mut keyframes = Keyframes {
            name: name.to_string(),
            frames: Vec::new(),
        };

        let mut pos = 0;
        let chars: Vec<char> = content.chars().collect();

        while pos < chars.len() {
            // Skip whitespace
            while pos < chars.len() && chars[pos].is_whitespace() {
                pos += 1;
            }

            if pos >= chars.len() {
                break;
            }

            // Parse selector (0%, 50%, 100%, from, to)
            let mut selector = String::new();
            while pos < chars.len() && chars[pos] != '{' {
                selector.push(chars[pos]);
                pos += 1;
            }
            let selector = selector.trim().to_string();

            if pos >= chars.len() {
                break;
            }

            pos += 1; // Skip '{'

            // Parse declarations
            let mut decl_str = String::new();
            let mut depth = 1;
            while pos < chars.len() && depth > 0 {
                if chars[pos] == '{' {
                    depth += 1;
                } else if chars[pos] == '}' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                decl_str.push(chars[pos]);
                pos += 1;
            }

            pos += 1; // Skip '}'

            let declarations = self.parse_declarations(&decl_str);

            // Parse percentage
            let percentage = if selector == "from" {
                0.0
            } else if selector == "to" {
                100.0
            } else if let Some(pct) = selector.strip_suffix('%') {
                pct.trim().parse().unwrap_or(0.0)
            } else {
                0.0
            };

            keyframes.frames.push(KeyframeFrame {
                percentage,
                declarations,
            });
        }

        keyframes
    }

    fn parse_identifier(&mut self) -> String {
        let mut name = String::new();
        while !self.eof() {
            let c = self.current_char();
            if c.is_alphanumeric() || c == '-' || c == '_' {
                name.push(self.consume_char());
            } else {
                break;
            }
        }
        name.to_lowercase()
    }

    fn consume_balanced_braces(&mut self) -> String {
        let mut result = String::new();
        let mut depth = 1;

        while !self.eof() && depth > 0 {
            let c = self.current_char();
            if c == '{' {
                depth += 1;
            } else if c == '}' {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            result.push(self.consume_char());
        }

        result
    }

    fn current_char(&self) -> char {
        self.input[self.pos..].chars().next().unwrap_or('\0')
    }

    fn consume_char(&mut self) -> char {
        let c = self.current_char();
        self.pos += c.len_utf8();
        c
    }

    fn consume_until(&mut self, end: char) -> String {
        let mut result = String::new();
        while !self.eof() && self.current_char() != end {
            result.push(self.consume_char());
        }
        result
    }

    fn consume_if(&mut self, c: char) -> bool {
        if !self.eof() && self.current_char() == c {
            self.consume_char();
            true
        } else {
            false
        }
    }

    fn skip_whitespace(&mut self) {
        while !self.eof() && self.current_char().is_whitespace() {
            self.consume_char();
        }
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            self.skip_whitespace();

            if self.starts_with("/*") {
                self.consume_char();
                self.consume_char();
                while !self.eof() && !self.starts_with("*/") {
                    self.consume_char();
                }
                if self.starts_with("*/") {
                    self.consume_char();
                    self.consume_char();
                }
                continue;
            }

            break;
        }
    }

    fn eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn starts_with(&self, s: &str) -> bool {
        self.input[self.pos..].starts_with(s)
    }
}

/// CSS stylesheet
#[derive(Debug, Clone)]
pub struct StyleSheet {
    /// CSS rules
    pub rules: Vec<CssRule>,
    /// Import URLs
    pub imports: Vec<String>,
    /// Keyframe animations
    pub keyframes: Vec<Keyframes>,
    /// Font faces
    pub font_faces: Vec<FontFace>,
}

impl StyleSheet {
    /// Create new empty stylesheet
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            imports: Vec::new(),
            keyframes: Vec::new(),
            font_faces: Vec::new(),
        }
    }

    /// Get rules matching a selector
    pub fn get_matching_rules(&self, element_tag: &str, element_id: Option<&str>, element_classes: &[&str]) -> Vec<&CssRule> {
        let mut matching = Vec::new();

        for rule in &self.rules {
            for selector in &rule.selectors {
                if selector.matches(element_tag, element_id, element_classes) {
                    matching.push(rule);
                    break;
                }
            }
        }

        // Sort by specificity
        matching.sort_by(|a, b| {
            let spec_a = a.selectors.first().map(|s| s.specificity()).unwrap_or(0);
            let spec_b = b.selectors.first().map(|s| s.specificity()).unwrap_or(0);
            spec_a.cmp(&spec_b)
        });

        matching
    }
}

impl Default for StyleSheet {
    fn default() -> Self {
        Self::new()
    }
}

/// CSS rule
#[derive(Debug, Clone)]
pub struct CssRule {
    /// Selectors
    pub selectors: Vec<CssSelector>,
    /// Declarations
    pub declarations: Vec<CssDeclaration>,
    /// Media query (if any)
    pub media_query: Option<String>,
}

/// CSS selector
#[derive(Debug, Clone)]
pub struct CssSelector {
    /// Selector parts
    pub parts: Vec<SelectorPart>,
    /// Raw selector string
    pub raw: String,
}

impl CssSelector {
    /// Create new selector from string
    pub fn new(selector: &str) -> Self {
        let parts = parse_selector_parts(selector);
        Self {
            parts,
            raw: selector.to_string(),
        }
    }

    /// Calculate specificity (a, b, c format as single number)
    pub fn specificity(&self) -> u32 {
        let mut ids = 0u32;
        let mut classes = 0u32;
        let mut elements = 0u32;

        for part in &self.parts {
            match part {
                SelectorPart::Id(_) => ids += 1,
                SelectorPart::Class(_) => classes += 1,
                SelectorPart::Tag(_) => elements += 1,
                SelectorPart::Attribute { .. } => classes += 1,
                SelectorPart::PseudoClass(_) => classes += 1,
                SelectorPart::PseudoElement(_) => elements += 1,
                SelectorPart::Universal => {}
                SelectorPart::Combinator(_) => {}
            }
        }

        (ids * 10000) + (classes * 100) + elements
    }

    /// Check if selector matches element
    pub fn matches(&self, tag: &str, id: Option<&str>, classes: &[&str]) -> bool {
        for part in &self.parts {
            match part {
                SelectorPart::Tag(t) => {
                    if t != tag && t != "*" {
                        return false;
                    }
                }
                SelectorPart::Id(i) => {
                    if id != Some(i.as_str()) {
                        return false;
                    }
                }
                SelectorPart::Class(c) => {
                    if !classes.contains(&c.as_str()) {
                        return false;
                    }
                }
                SelectorPart::Universal => {}
                SelectorPart::Combinator(_) => {
                    // Combinators are handled during tree matching
                    // For simple matching, we just skip them
                }
                _ => {}
            }
        }
        true
    }
}

/// Selector part
#[derive(Debug, Clone)]
pub enum SelectorPart {
    /// Tag name
    Tag(String),
    /// ID selector
    Id(String),
    /// Class selector
    Class(String),
    /// Attribute selector
    Attribute {
        name: String,
        operator: Option<String>,
        value: Option<String>,
    },
    /// Pseudo-class
    PseudoClass(String),
    /// Pseudo-element
    PseudoElement(String),
    /// Universal selector
    Universal,
    /// Combinator
    Combinator(Combinator),
}

/// CSS combinator
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Combinator {
    /// Descendant (space)
    Descendant,
    /// Child (>)
    Child,
    /// Adjacent sibling (+)
    AdjacentSibling,
    /// General sibling (~)
    GeneralSibling,
}

/// CSS declaration
#[derive(Debug, Clone)]
pub struct CssDeclaration {
    /// Property name
    pub property: String,
    /// Property value
    pub value: String,
    /// Is !important
    pub important: bool,
}

impl CssDeclaration {
    /// Create new declaration
    pub fn new(property: &str, value: &str) -> Self {
        Self {
            property: property.to_string(),
            value: value.to_string(),
            important: false,
        }
    }

    /// Parse color value
    pub fn parse_color(&self) -> Option<CssColor> {
        CssColor::parse(&self.value)
    }

    /// Parse length value
    pub fn parse_length(&self) -> Option<CssLength> {
        CssLength::parse(&self.value)
    }
}

/// At-rule (internal)
struct AtRule {
    name: String,
    prelude: String,
    content: Option<String>,
}

/// Keyframes animation
#[derive(Debug, Clone)]
pub struct Keyframes {
    /// Animation name
    pub name: String,
    /// Keyframe frames
    pub frames: Vec<KeyframeFrame>,
}

/// Keyframe frame
#[derive(Debug, Clone)]
pub struct KeyframeFrame {
    /// Percentage (0-100)
    pub percentage: f32,
    /// Declarations
    pub declarations: Vec<CssDeclaration>,
}

/// Font face
#[derive(Debug, Clone)]
pub struct FontFace {
    /// Declarations
    pub declarations: Vec<CssDeclaration>,
}

/// CSS color value
#[derive(Debug, Clone, Copy)]
pub struct CssColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl CssColor {
    /// Parse color from string
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim().to_lowercase();

        // Named colors
        if let Some(color) = named_color(&value) {
            return Some(color);
        }

        // Hex color
        if value.starts_with('#') {
            return Self::parse_hex(&value[1..]);
        }

        // rgb() / rgba()
        if value.starts_with("rgb") {
            return Self::parse_rgb(&value);
        }

        // hsl() / hsla()
        if value.starts_with("hsl") {
            return Self::parse_hsl(&value);
        }

        None
    }

    fn parse_hex(hex: &str) -> Option<Self> {
        let hex = hex.trim();
        match hex.len() {
            3 => {
                let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
                let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
                let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
                Some(Self { r, g, b, a: 255 })
            }
            4 => {
                let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
                let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
                let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
                let a = u8::from_str_radix(&hex[3..4], 16).ok()? * 17;
                Some(Self { r, g, b, a })
            }
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some(Self { r, g, b, a: 255 })
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                Some(Self { r, g, b, a })
            }
            _ => None,
        }
    }

    fn parse_rgb(value: &str) -> Option<Self> {
        let start = value.find('(')?;
        let end = value.find(')')?;
        let content = &value[start + 1..end];
        let parts: Vec<&str> = content.split(|c| c == ',' || c == '/').collect();

        if parts.len() >= 3 {
            let r = parse_color_component(parts[0].trim())?;
            let g = parse_color_component(parts[1].trim())?;
            let b = parse_color_component(parts[2].trim())?;
            let a = if parts.len() >= 4 {
                parse_alpha_component(parts[3].trim())?
            } else {
                255
            };
            Some(Self { r, g, b, a })
        } else {
            None
        }
    }

    fn parse_hsl(value: &str) -> Option<Self> {
        let start = value.find('(')?;
        let end = value.find(')')?;
        let content = &value[start + 1..end];
        let parts: Vec<&str> = content.split(|c| c == ',' || c == '/').collect();

        if parts.len() >= 3 {
            let h: f32 = parse_f32(parts[0].trim().trim_end_matches("deg"))?;
            let s: f32 = parse_f32(parts[1].trim().trim_end_matches('%'))? / 100.0;
            let l: f32 = parse_f32(parts[2].trim().trim_end_matches('%'))? / 100.0;

            let (r, g, b) = hsl_to_rgb(h, s, l);
            let a = if parts.len() >= 4 {
                parse_alpha_component(parts[3].trim())?
            } else {
                255
            };

            Some(Self { r, g, b, a })
        } else {
            None
        }
    }
}

/// CSS length value
#[derive(Debug, Clone, Copy)]
pub struct CssLength {
    pub value: f32,
    pub unit: LengthUnit,
}

/// Length unit
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LengthUnit {
    Px,
    Em,
    Rem,
    Percent,
    Vh,
    Vw,
    Vmin,
    Vmax,
    Pt,
    Cm,
    Mm,
    In,
    Auto,
}

impl CssLength {
    /// Parse length from string
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim().to_lowercase();

        if value == "auto" {
            return Some(Self { value: 0.0, unit: LengthUnit::Auto });
        }

        if value == "0" {
            return Some(Self { value: 0.0, unit: LengthUnit::Px });
        }

        let (num, unit) = if value.ends_with("px") {
            (value.trim_end_matches("px"), LengthUnit::Px)
        } else if value.ends_with("em") {
            (value.trim_end_matches("em"), LengthUnit::Em)
        } else if value.ends_with("rem") {
            (value.trim_end_matches("rem"), LengthUnit::Rem)
        } else if value.ends_with('%') {
            (value.trim_end_matches('%'), LengthUnit::Percent)
        } else if value.ends_with("vh") {
            (value.trim_end_matches("vh"), LengthUnit::Vh)
        } else if value.ends_with("vw") {
            (value.trim_end_matches("vw"), LengthUnit::Vw)
        } else if value.ends_with("vmin") {
            (value.trim_end_matches("vmin"), LengthUnit::Vmin)
        } else if value.ends_with("vmax") {
            (value.trim_end_matches("vmax"), LengthUnit::Vmax)
        } else if value.ends_with("pt") {
            (value.trim_end_matches("pt"), LengthUnit::Pt)
        } else if value.ends_with("cm") {
            (value.trim_end_matches("cm"), LengthUnit::Cm)
        } else if value.ends_with("mm") {
            (value.trim_end_matches("mm"), LengthUnit::Mm)
        } else if value.ends_with("in") {
            (value.trim_end_matches("in"), LengthUnit::In)
        } else {
            // Assume pixels for unitless numbers
            (value.as_str(), LengthUnit::Px)
        };

        let num: f32 = parse_f32(num)?;
        Some(Self { value: num, unit })
    }

    /// Convert to pixels
    pub fn to_px(&self, font_size: f32, viewport_width: f32, viewport_height: f32) -> f32 {
        match self.unit {
            LengthUnit::Px => self.value,
            LengthUnit::Em => self.value * font_size,
            LengthUnit::Rem => self.value * 16.0, // Root font size
            LengthUnit::Percent => self.value, // Context dependent
            LengthUnit::Vh => self.value * viewport_height / 100.0,
            LengthUnit::Vw => self.value * viewport_width / 100.0,
            LengthUnit::Vmin => self.value * viewport_width.min(viewport_height) / 100.0,
            LengthUnit::Vmax => self.value * viewport_width.max(viewport_height) / 100.0,
            LengthUnit::Pt => self.value * 1.333,
            LengthUnit::Cm => self.value * 37.795,
            LengthUnit::Mm => self.value * 3.7795,
            LengthUnit::In => self.value * 96.0,
            LengthUnit::Auto => 0.0,
        }
    }
}

// Helper functions

fn parse_selectors(input: &str) -> Vec<CssSelector> {
    input
        .split(',')
        .map(|s| CssSelector::new(s.trim()))
        .collect()
}

fn parse_selector_parts(selector: &str) -> Vec<SelectorPart> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let selector = selector.trim();

    let mut chars = selector.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '#' => {
                if !current.is_empty() {
                    parts.push(SelectorPart::Tag(current.clone()));
                    current.clear();
                }
                let mut id = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_alphanumeric() || c == '-' || c == '_' {
                        id.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }
                parts.push(SelectorPart::Id(id));
            }
            '.' => {
                if !current.is_empty() {
                    parts.push(SelectorPart::Tag(current.clone()));
                    current.clear();
                }
                let mut class = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_alphanumeric() || c == '-' || c == '_' {
                        class.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }
                parts.push(SelectorPart::Class(class));
            }
            '[' => {
                if !current.is_empty() {
                    parts.push(SelectorPart::Tag(current.clone()));
                    current.clear();
                }
                let mut attr = String::new();
                while let Some(c) = chars.next() {
                    if c == ']' {
                        break;
                    }
                    attr.push(c);
                }
                // Parse attribute selector
                if let Some(eq_pos) = attr.find('=') {
                    let name = attr[..eq_pos].trim_matches(|c| c == '~' || c == '|' || c == '^' || c == '$' || c == '*').to_string();
                    let operator = Some(attr[..eq_pos + 1].chars().filter(|&c| c == '~' || c == '|' || c == '^' || c == '$' || c == '*' || c == '=').collect());
                    let value = Some(attr[eq_pos + 1..].trim_matches(|c| c == '"' || c == '\'').to_string());
                    parts.push(SelectorPart::Attribute { name, operator, value });
                } else {
                    parts.push(SelectorPart::Attribute { name: attr, operator: None, value: None });
                }
            }
            ':' => {
                if !current.is_empty() {
                    parts.push(SelectorPart::Tag(current.clone()));
                    current.clear();
                }
                let is_element = chars.peek() == Some(&':');
                if is_element {
                    chars.next();
                }
                let mut pseudo = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_alphanumeric() || c == '-' || c == '_' {
                        pseudo.push(chars.next().unwrap());
                    } else if c == '(' {
                        pseudo.push(chars.next().unwrap());
                        let mut depth = 1;
                        while let Some(c) = chars.next() {
                            pseudo.push(c);
                            if c == '(' { depth += 1; }
                            if c == ')' { depth -= 1; if depth == 0 { break; } }
                        }
                    } else {
                        break;
                    }
                }
                if is_element {
                    parts.push(SelectorPart::PseudoElement(pseudo));
                } else {
                    parts.push(SelectorPart::PseudoClass(pseudo));
                }
            }
            '*' => {
                parts.push(SelectorPart::Universal);
            }
            ' ' => {
                if !current.is_empty() {
                    parts.push(SelectorPart::Tag(current.clone()));
                    current.clear();
                }
                // Skip extra spaces
                while chars.peek() == Some(&' ') {
                    chars.next();
                }
                // Check for other combinators
                match chars.peek() {
                    Some(&'>') => {
                        chars.next();
                        while chars.peek() == Some(&' ') { chars.next(); }
                        parts.push(SelectorPart::Combinator(Combinator::Child));
                    }
                    Some(&'+') => {
                        chars.next();
                        while chars.peek() == Some(&' ') { chars.next(); }
                        parts.push(SelectorPart::Combinator(Combinator::AdjacentSibling));
                    }
                    Some(&'~') => {
                        chars.next();
                        while chars.peek() == Some(&' ') { chars.next(); }
                        parts.push(SelectorPart::Combinator(Combinator::GeneralSibling));
                    }
                    _ => {
                        parts.push(SelectorPart::Combinator(Combinator::Descendant));
                    }
                }
            }
            '>' => {
                if !current.is_empty() {
                    parts.push(SelectorPart::Tag(current.clone()));
                    current.clear();
                }
                while chars.peek() == Some(&' ') { chars.next(); }
                parts.push(SelectorPart::Combinator(Combinator::Child));
            }
            '+' => {
                if !current.is_empty() {
                    parts.push(SelectorPart::Tag(current.clone()));
                    current.clear();
                }
                while chars.peek() == Some(&' ') { chars.next(); }
                parts.push(SelectorPart::Combinator(Combinator::AdjacentSibling));
            }
            '~' => {
                if !current.is_empty() {
                    parts.push(SelectorPart::Tag(current.clone()));
                    current.clear();
                }
                while chars.peek() == Some(&' ') { chars.next(); }
                parts.push(SelectorPart::Combinator(Combinator::GeneralSibling));
            }
            _ => {
                current.push(c);
            }
        }
    }

    if !current.is_empty() {
        parts.push(SelectorPart::Tag(current));
    }

    parts
}

fn parse_color_component(s: &str) -> Option<u8> {
    if s.ends_with('%') {
        let pct: f32 = s.trim_end_matches('%').parse().ok()?;
        Some((pct * 2.55) as u8)
    } else {
        s.parse().ok()
    }
}

fn parse_alpha_component(s: &str) -> Option<u8> {
    if s.ends_with('%') {
        let pct: f32 = s.trim_end_matches('%').parse().ok()?;
        Some((pct * 2.55) as u8)
    } else {
        let alpha: f32 = s.parse().ok()?;
        Some((alpha * 255.0) as u8)
    }
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    if s == 0.0 {
        let v = (l * 255.0) as u8;
        return (v, v, v);
    }

    let h = h / 360.0;
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;

    let r = hue_to_rgb(p, q, h + 1.0 / 3.0);
    let g = hue_to_rgb(p, q, h);
    let b = hue_to_rgb(p, q, h - 1.0 / 3.0);

    ((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

fn hue_to_rgb(p: f32, q: f32, mut t: f32) -> f32 {
    if t < 0.0 { t += 1.0; }
    if t > 1.0 { t -= 1.0; }
    if t < 1.0 / 6.0 { return p + (q - p) * 6.0 * t; }
    if t < 1.0 / 2.0 { return q; }
    if t < 2.0 / 3.0 { return p + (q - p) * (2.0 / 3.0 - t) * 6.0; }
    p
}

fn named_color(name: &str) -> Option<CssColor> {
    let (r, g, b) = match name {
        "black" => (0, 0, 0),
        "white" => (255, 255, 255),
        "red" => (255, 0, 0),
        "green" => (0, 128, 0),
        "blue" => (0, 0, 255),
        "yellow" => (255, 255, 0),
        "cyan" | "aqua" => (0, 255, 255),
        "magenta" | "fuchsia" => (255, 0, 255),
        "gray" | "grey" => (128, 128, 128),
        "silver" => (192, 192, 192),
        "maroon" => (128, 0, 0),
        "olive" => (128, 128, 0),
        "lime" => (0, 255, 0),
        "navy" => (0, 0, 128),
        "purple" => (128, 0, 128),
        "teal" => (0, 128, 128),
        "orange" => (255, 165, 0),
        "pink" => (255, 192, 203),
        "brown" => (165, 42, 42),
        "coral" => (255, 127, 80),
        "gold" => (255, 215, 0),
        "indigo" => (75, 0, 130),
        "violet" => (238, 130, 238),
        "transparent" => return Some(CssColor { r: 0, g: 0, b: 0, a: 0 }),
        _ => return None,
    };
    Some(CssColor { r, g, b, a: 255 })
}

/// Parse f32 from string (no_std compatible)
fn parse_f32(s: &str) -> Option<f32> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    let (sign, s) = if s.starts_with('-') {
        (-1.0f32, &s[1..])
    } else if s.starts_with('+') {
        (1.0f32, &s[1..])
    } else {
        (1.0f32, s)
    };

    let mut result = 0.0f32;
    let mut decimal_place = 0i32;
    let mut seen_dot = false;

    for c in s.chars() {
        if c == '.' {
            if seen_dot {
                return None; // Multiple dots
            }
            seen_dot = true;
        } else if c.is_ascii_digit() {
            let digit = (c as u8 - b'0') as f32;
            if seen_dot {
                decimal_place += 1;
                let mut divisor = 1.0f32;
                for _ in 0..decimal_place {
                    divisor *= 10.0;
                }
                result += digit / divisor;
            } else {
                result = result * 10.0 + digit;
            }
        } else {
            return None; // Invalid character
        }
    }

    Some(sign * result)
}
