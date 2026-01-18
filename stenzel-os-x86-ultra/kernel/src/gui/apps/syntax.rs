//! Syntax Highlighting
//!
//! Multi-language syntax highlighting with customizable color schemes.
//! Supports Rust, Python, JavaScript, TypeScript, C/C++, HTML, CSS, JSON, Markdown, and more.

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use crate::drivers::framebuffer::Color;

/// Programming language for syntax highlighting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    PlainText,
    Rust,
    Python,
    JavaScript,
    TypeScript,
    C,
    Cpp,
    CSharp,
    Java,
    Go,
    Html,
    Css,
    Scss,
    Json,
    Yaml,
    Toml,
    Xml,
    Markdown,
    Shell,
    Sql,
    Php,
    Ruby,
    Swift,
    Kotlin,
    Lua,
    Zig,
    Haskell,
    Ocaml,
    Elixir,
    Makefile,
    Dockerfile,
    Gitignore,
}

impl Language {
    /// Detect language from file extension
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "rs" => Language::Rust,
            "py" | "pyw" | "pyi" => Language::Python,
            "js" | "mjs" | "cjs" => Language::JavaScript,
            "ts" | "tsx" => Language::TypeScript,
            "c" | "h" => Language::C,
            "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "c++" | "h++" => Language::Cpp,
            "cs" => Language::CSharp,
            "java" => Language::Java,
            "go" => Language::Go,
            "html" | "htm" => Language::Html,
            "css" => Language::Css,
            "scss" | "sass" => Language::Scss,
            "json" | "jsonc" => Language::Json,
            "yaml" | "yml" => Language::Yaml,
            "toml" => Language::Toml,
            "xml" | "xsl" | "xslt" | "xsd" | "svg" => Language::Xml,
            "md" | "markdown" | "mkd" => Language::Markdown,
            "sh" | "bash" | "zsh" | "fish" => Language::Shell,
            "sql" => Language::Sql,
            "php" => Language::Php,
            "rb" | "ruby" => Language::Ruby,
            "swift" => Language::Swift,
            "kt" | "kts" => Language::Kotlin,
            "lua" => Language::Lua,
            "zig" => Language::Zig,
            "hs" | "lhs" => Language::Haskell,
            "ml" | "mli" => Language::Ocaml,
            "ex" | "exs" => Language::Elixir,
            "makefile" | "make" | "mk" => Language::Makefile,
            "dockerfile" => Language::Dockerfile,
            "gitignore" => Language::Gitignore,
            _ => Language::PlainText,
        }
    }

    /// Get language name
    pub fn name(&self) -> &'static str {
        match self {
            Language::PlainText => "Plain Text",
            Language::Rust => "Rust",
            Language::Python => "Python",
            Language::JavaScript => "JavaScript",
            Language::TypeScript => "TypeScript",
            Language::C => "C",
            Language::Cpp => "C++",
            Language::CSharp => "C#",
            Language::Java => "Java",
            Language::Go => "Go",
            Language::Html => "HTML",
            Language::Css => "CSS",
            Language::Scss => "SCSS",
            Language::Json => "JSON",
            Language::Yaml => "YAML",
            Language::Toml => "TOML",
            Language::Xml => "XML",
            Language::Markdown => "Markdown",
            Language::Shell => "Shell",
            Language::Sql => "SQL",
            Language::Php => "PHP",
            Language::Ruby => "Ruby",
            Language::Swift => "Swift",
            Language::Kotlin => "Kotlin",
            Language::Lua => "Lua",
            Language::Zig => "Zig",
            Language::Haskell => "Haskell",
            Language::Ocaml => "OCaml",
            Language::Elixir => "Elixir",
            Language::Makefile => "Makefile",
            Language::Dockerfile => "Dockerfile",
            Language::Gitignore => "gitignore",
        }
    }

    /// Check if language uses line comments
    pub fn line_comment(&self) -> Option<&'static str> {
        match self {
            Language::Rust | Language::Go | Language::Swift | Language::Kotlin |
            Language::C | Language::Cpp | Language::CSharp | Language::Java |
            Language::JavaScript | Language::TypeScript | Language::Zig |
            Language::Scss => Some("//"),
            Language::Python | Language::Shell | Language::Ruby |
            Language::Yaml | Language::Toml | Language::Makefile |
            Language::Dockerfile | Language::Gitignore | Language::Elixir => Some("#"),
            Language::Lua | Language::Haskell => Some("--"),
            Language::Ocaml => None, // Uses (* *)
            Language::Html | Language::Xml => None, // Uses <!-- -->
            Language::Css => None, // Uses /* */
            Language::Sql => Some("--"),
            Language::Php => Some("//"),
            _ => None,
        }
    }

    /// Check if language uses block comments
    pub fn block_comment(&self) -> Option<(&'static str, &'static str)> {
        match self {
            Language::Rust | Language::Go | Language::Swift | Language::Kotlin |
            Language::C | Language::Cpp | Language::CSharp | Language::Java |
            Language::JavaScript | Language::TypeScript | Language::Css |
            Language::Scss | Language::Php | Language::Zig => Some(("/*", "*/")),
            Language::Html | Language::Xml => Some(("<!--", "-->")),
            Language::Python => Some(("\"\"\"", "\"\"\"")),
            Language::Lua => Some(("--[[", "]]")),
            Language::Haskell => Some(("{-", "-}")),
            Language::Ocaml => Some(("(*", "*)")),
            _ => None,
        }
    }
}

/// Token type for highlighting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenType {
    /// Plain text, no highlighting
    Normal,
    /// Keywords (if, for, while, fn, etc.)
    Keyword,
    /// Control flow keywords (return, break, continue)
    ControlFlow,
    /// Type names (i32, String, Vec, etc.)
    Type,
    /// Built-in functions and constants
    Builtin,
    /// Function definitions and calls
    Function,
    /// Method calls
    Method,
    /// Macro invocations
    Macro,
    /// String literals
    String,
    /// Character literals
    Char,
    /// Number literals (integer, float)
    Number,
    /// Boolean literals (true, false)
    Boolean,
    /// Null/None/nil
    Null,
    /// Line comment
    Comment,
    /// Block comment
    BlockComment,
    /// Documentation comment
    DocComment,
    /// Operators (+, -, *, /, etc.)
    Operator,
    /// Delimiters (brackets, parentheses, braces)
    Delimiter,
    /// Punctuation (comma, semicolon, etc.)
    Punctuation,
    /// Attribute/decorator (@, #[], etc.)
    Attribute,
    /// Namespace/module path
    Namespace,
    /// Variable name
    Variable,
    /// Parameter name
    Parameter,
    /// Property/field access
    Property,
    /// Constant name (UPPER_CASE)
    Constant,
    /// Label/lifetimes
    Label,
    /// Escape sequence in strings
    Escape,
    /// Format placeholder in strings
    FormatPlaceholder,
    /// Regex literal
    Regex,
    /// HTML/XML tag name
    TagName,
    /// HTML/XML attribute name
    TagAttribute,
    /// Error/invalid token
    Error,
    /// Special token (language-specific)
    Special,
}

/// A single highlighted token
#[derive(Debug, Clone)]
pub struct Token {
    pub start: usize,
    pub end: usize,
    pub token_type: TokenType,
}

impl Token {
    pub fn new(start: usize, end: usize, token_type: TokenType) -> Self {
        Self { start, end, token_type }
    }

    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }
}

/// Color scheme for syntax highlighting
#[derive(Debug, Clone)]
pub struct ColorScheme {
    pub name: String,
    pub background: Color,
    pub foreground: Color,
    pub cursor: Color,
    pub selection: Color,
    pub line_highlight: Color,
    pub line_numbers: Color,
    pub line_numbers_active: Color,

    // Token colors
    pub keyword: Color,
    pub control_flow: Color,
    pub type_color: Color,
    pub builtin: Color,
    pub function: Color,
    pub method: Color,
    pub macro_color: Color,
    pub string: Color,
    pub char_color: Color,
    pub number: Color,
    pub boolean: Color,
    pub null: Color,
    pub comment: Color,
    pub block_comment: Color,
    pub doc_comment: Color,
    pub operator: Color,
    pub delimiter: Color,
    pub punctuation: Color,
    pub attribute: Color,
    pub namespace: Color,
    pub variable: Color,
    pub parameter: Color,
    pub property: Color,
    pub constant: Color,
    pub label: Color,
    pub escape: Color,
    pub format_placeholder: Color,
    pub regex: Color,
    pub tag_name: Color,
    pub tag_attribute: Color,
    pub error: Color,
    pub special: Color,
}

impl Default for ColorScheme {
    fn default() -> Self {
        Self::monokai()
    }
}

impl ColorScheme {
    /// Get color for token type
    pub fn color_for(&self, token_type: TokenType) -> Color {
        match token_type {
            TokenType::Normal => self.foreground,
            TokenType::Keyword => self.keyword,
            TokenType::ControlFlow => self.control_flow,
            TokenType::Type => self.type_color,
            TokenType::Builtin => self.builtin,
            TokenType::Function => self.function,
            TokenType::Method => self.method,
            TokenType::Macro => self.macro_color,
            TokenType::String => self.string,
            TokenType::Char => self.char_color,
            TokenType::Number => self.number,
            TokenType::Boolean => self.boolean,
            TokenType::Null => self.null,
            TokenType::Comment => self.comment,
            TokenType::BlockComment => self.block_comment,
            TokenType::DocComment => self.doc_comment,
            TokenType::Operator => self.operator,
            TokenType::Delimiter => self.delimiter,
            TokenType::Punctuation => self.punctuation,
            TokenType::Attribute => self.attribute,
            TokenType::Namespace => self.namespace,
            TokenType::Variable => self.variable,
            TokenType::Parameter => self.parameter,
            TokenType::Property => self.property,
            TokenType::Constant => self.constant,
            TokenType::Label => self.label,
            TokenType::Escape => self.escape,
            TokenType::FormatPlaceholder => self.format_placeholder,
            TokenType::Regex => self.regex,
            TokenType::TagName => self.tag_name,
            TokenType::TagAttribute => self.tag_attribute,
            TokenType::Error => self.error,
            TokenType::Special => self.special,
        }
    }

    /// Monokai color scheme (default)
    pub fn monokai() -> Self {
        Self {
            name: String::from("Monokai"),
            background: Color::new(39, 40, 34),
            foreground: Color::new(248, 248, 242),
            cursor: Color::new(248, 248, 240),
            selection: Color::new(73, 72, 62),
            line_highlight: Color::new(62, 61, 50),
            line_numbers: Color::new(144, 144, 138),
            line_numbers_active: Color::new(248, 248, 242),

            keyword: Color::new(249, 38, 114),
            control_flow: Color::new(249, 38, 114),
            type_color: Color::new(102, 217, 239),
            builtin: Color::new(102, 217, 239),
            function: Color::new(166, 226, 46),
            method: Color::new(166, 226, 46),
            macro_color: Color::new(166, 226, 46),
            string: Color::new(230, 219, 116),
            char_color: Color::new(230, 219, 116),
            number: Color::new(174, 129, 255),
            boolean: Color::new(174, 129, 255),
            null: Color::new(174, 129, 255),
            comment: Color::new(117, 113, 94),
            block_comment: Color::new(117, 113, 94),
            doc_comment: Color::new(117, 113, 94),
            operator: Color::new(249, 38, 114),
            delimiter: Color::new(248, 248, 242),
            punctuation: Color::new(248, 248, 242),
            attribute: Color::new(166, 226, 46),
            namespace: Color::new(102, 217, 239),
            variable: Color::new(248, 248, 242),
            parameter: Color::new(253, 151, 31),
            property: Color::new(248, 248, 242),
            constant: Color::new(174, 129, 255),
            label: Color::new(174, 129, 255),
            escape: Color::new(174, 129, 255),
            format_placeholder: Color::new(102, 217, 239),
            regex: Color::new(230, 219, 116),
            tag_name: Color::new(249, 38, 114),
            tag_attribute: Color::new(166, 226, 46),
            error: Color::new(249, 38, 114),
            special: Color::new(253, 151, 31),
        }
    }

    /// Dracula color scheme
    pub fn dracula() -> Self {
        Self {
            name: String::from("Dracula"),
            background: Color::new(40, 42, 54),
            foreground: Color::new(248, 248, 242),
            cursor: Color::new(248, 248, 242),
            selection: Color::new(68, 71, 90),
            line_highlight: Color::new(68, 71, 90),
            line_numbers: Color::new(108, 117, 125),
            line_numbers_active: Color::new(248, 248, 242),

            keyword: Color::new(255, 121, 198),
            control_flow: Color::new(255, 121, 198),
            type_color: Color::new(139, 233, 253),
            builtin: Color::new(139, 233, 253),
            function: Color::new(80, 250, 123),
            method: Color::new(80, 250, 123),
            macro_color: Color::new(80, 250, 123),
            string: Color::new(241, 250, 140),
            char_color: Color::new(241, 250, 140),
            number: Color::new(189, 147, 249),
            boolean: Color::new(189, 147, 249),
            null: Color::new(189, 147, 249),
            comment: Color::new(98, 114, 164),
            block_comment: Color::new(98, 114, 164),
            doc_comment: Color::new(98, 114, 164),
            operator: Color::new(255, 121, 198),
            delimiter: Color::new(248, 248, 242),
            punctuation: Color::new(248, 248, 242),
            attribute: Color::new(80, 250, 123),
            namespace: Color::new(139, 233, 253),
            variable: Color::new(248, 248, 242),
            parameter: Color::new(255, 184, 108),
            property: Color::new(248, 248, 242),
            constant: Color::new(189, 147, 249),
            label: Color::new(255, 121, 198),
            escape: Color::new(255, 121, 198),
            format_placeholder: Color::new(139, 233, 253),
            regex: Color::new(241, 250, 140),
            tag_name: Color::new(255, 121, 198),
            tag_attribute: Color::new(80, 250, 123),
            error: Color::new(255, 85, 85),
            special: Color::new(255, 184, 108),
        }
    }

    /// One Dark color scheme
    pub fn one_dark() -> Self {
        Self {
            name: String::from("One Dark"),
            background: Color::new(40, 44, 52),
            foreground: Color::new(171, 178, 191),
            cursor: Color::new(82, 139, 255),
            selection: Color::new(62, 68, 81),
            line_highlight: Color::new(44, 49, 58),
            line_numbers: Color::new(76, 82, 99),
            line_numbers_active: Color::new(171, 178, 191),

            keyword: Color::new(198, 120, 221),
            control_flow: Color::new(198, 120, 221),
            type_color: Color::new(229, 192, 123),
            builtin: Color::new(229, 192, 123),
            function: Color::new(97, 175, 239),
            method: Color::new(97, 175, 239),
            macro_color: Color::new(97, 175, 239),
            string: Color::new(152, 195, 121),
            char_color: Color::new(152, 195, 121),
            number: Color::new(209, 154, 102),
            boolean: Color::new(209, 154, 102),
            null: Color::new(209, 154, 102),
            comment: Color::new(92, 99, 112),
            block_comment: Color::new(92, 99, 112),
            doc_comment: Color::new(92, 99, 112),
            operator: Color::new(171, 178, 191),
            delimiter: Color::new(171, 178, 191),
            punctuation: Color::new(171, 178, 191),
            attribute: Color::new(229, 192, 123),
            namespace: Color::new(224, 108, 117),
            variable: Color::new(224, 108, 117),
            parameter: Color::new(224, 108, 117),
            property: Color::new(171, 178, 191),
            constant: Color::new(209, 154, 102),
            label: Color::new(198, 120, 221),
            escape: Color::new(86, 182, 194),
            format_placeholder: Color::new(86, 182, 194),
            regex: Color::new(152, 195, 121),
            tag_name: Color::new(224, 108, 117),
            tag_attribute: Color::new(209, 154, 102),
            error: Color::new(224, 108, 117),
            special: Color::new(229, 192, 123),
        }
    }

    /// Solarized Dark color scheme
    pub fn solarized_dark() -> Self {
        Self {
            name: String::from("Solarized Dark"),
            background: Color::new(0, 43, 54),
            foreground: Color::new(131, 148, 150),
            cursor: Color::new(131, 148, 150),
            selection: Color::new(7, 54, 66),
            line_highlight: Color::new(7, 54, 66),
            line_numbers: Color::new(88, 110, 117),
            line_numbers_active: Color::new(147, 161, 161),

            keyword: Color::new(133, 153, 0),
            control_flow: Color::new(133, 153, 0),
            type_color: Color::new(181, 137, 0),
            builtin: Color::new(181, 137, 0),
            function: Color::new(38, 139, 210),
            method: Color::new(38, 139, 210),
            macro_color: Color::new(203, 75, 22),
            string: Color::new(42, 161, 152),
            char_color: Color::new(42, 161, 152),
            number: Color::new(211, 54, 130),
            boolean: Color::new(211, 54, 130),
            null: Color::new(211, 54, 130),
            comment: Color::new(88, 110, 117),
            block_comment: Color::new(88, 110, 117),
            doc_comment: Color::new(88, 110, 117),
            operator: Color::new(133, 153, 0),
            delimiter: Color::new(131, 148, 150),
            punctuation: Color::new(131, 148, 150),
            attribute: Color::new(108, 113, 196),
            namespace: Color::new(181, 137, 0),
            variable: Color::new(38, 139, 210),
            parameter: Color::new(203, 75, 22),
            property: Color::new(131, 148, 150),
            constant: Color::new(211, 54, 130),
            label: Color::new(211, 54, 130),
            escape: Color::new(220, 50, 47),
            format_placeholder: Color::new(38, 139, 210),
            regex: Color::new(42, 161, 152),
            tag_name: Color::new(38, 139, 210),
            tag_attribute: Color::new(181, 137, 0),
            error: Color::new(220, 50, 47),
            special: Color::new(108, 113, 196),
        }
    }

    /// Nord color scheme
    pub fn nord() -> Self {
        Self {
            name: String::from("Nord"),
            background: Color::new(46, 52, 64),
            foreground: Color::new(216, 222, 233),
            cursor: Color::new(216, 222, 233),
            selection: Color::new(67, 76, 94),
            line_highlight: Color::new(59, 66, 82),
            line_numbers: Color::new(76, 86, 106),
            line_numbers_active: Color::new(216, 222, 233),

            keyword: Color::new(129, 161, 193),
            control_flow: Color::new(129, 161, 193),
            type_color: Color::new(143, 188, 187),
            builtin: Color::new(143, 188, 187),
            function: Color::new(136, 192, 208),
            method: Color::new(136, 192, 208),
            macro_color: Color::new(136, 192, 208),
            string: Color::new(163, 190, 140),
            char_color: Color::new(163, 190, 140),
            number: Color::new(180, 142, 173),
            boolean: Color::new(180, 142, 173),
            null: Color::new(180, 142, 173),
            comment: Color::new(97, 110, 136),
            block_comment: Color::new(97, 110, 136),
            doc_comment: Color::new(97, 110, 136),
            operator: Color::new(129, 161, 193),
            delimiter: Color::new(216, 222, 233),
            punctuation: Color::new(216, 222, 233),
            attribute: Color::new(208, 135, 112),
            namespace: Color::new(143, 188, 187),
            variable: Color::new(216, 222, 233),
            parameter: Color::new(208, 135, 112),
            property: Color::new(216, 222, 233),
            constant: Color::new(180, 142, 173),
            label: Color::new(180, 142, 173),
            escape: Color::new(235, 203, 139),
            format_placeholder: Color::new(136, 192, 208),
            regex: Color::new(163, 190, 140),
            tag_name: Color::new(129, 161, 193),
            tag_attribute: Color::new(143, 188, 187),
            error: Color::new(191, 97, 106),
            special: Color::new(235, 203, 139),
        }
    }

    /// GitHub Light color scheme
    pub fn github_light() -> Self {
        Self {
            name: String::from("GitHub Light"),
            background: Color::new(255, 255, 255),
            foreground: Color::new(36, 41, 46),
            cursor: Color::new(36, 41, 46),
            selection: Color::new(200, 225, 255),
            line_highlight: Color::new(248, 248, 248),
            line_numbers: Color::new(149, 157, 165),
            line_numbers_active: Color::new(36, 41, 46),

            keyword: Color::new(215, 58, 73),
            control_flow: Color::new(215, 58, 73),
            type_color: Color::new(111, 66, 193),
            builtin: Color::new(111, 66, 193),
            function: Color::new(111, 66, 193),
            method: Color::new(111, 66, 193),
            macro_color: Color::new(111, 66, 193),
            string: Color::new(3, 47, 98),
            char_color: Color::new(3, 47, 98),
            number: Color::new(0, 92, 197),
            boolean: Color::new(0, 92, 197),
            null: Color::new(0, 92, 197),
            comment: Color::new(106, 115, 125),
            block_comment: Color::new(106, 115, 125),
            doc_comment: Color::new(106, 115, 125),
            operator: Color::new(215, 58, 73),
            delimiter: Color::new(36, 41, 46),
            punctuation: Color::new(36, 41, 46),
            attribute: Color::new(111, 66, 193),
            namespace: Color::new(111, 66, 193),
            variable: Color::new(227, 98, 9),
            parameter: Color::new(227, 98, 9),
            property: Color::new(0, 92, 197),
            constant: Color::new(0, 92, 197),
            label: Color::new(111, 66, 193),
            escape: Color::new(0, 92, 197),
            format_placeholder: Color::new(0, 92, 197),
            regex: Color::new(3, 47, 98),
            tag_name: Color::new(34, 134, 58),
            tag_attribute: Color::new(111, 66, 193),
            error: Color::new(215, 58, 73),
            special: Color::new(227, 98, 9),
        }
    }

    /// Gruvbox Dark color scheme
    pub fn gruvbox_dark() -> Self {
        Self {
            name: String::from("Gruvbox Dark"),
            background: Color::new(40, 40, 40),
            foreground: Color::new(235, 219, 178),
            cursor: Color::new(235, 219, 178),
            selection: Color::new(80, 73, 69),
            line_highlight: Color::new(60, 56, 54),
            line_numbers: Color::new(124, 111, 100),
            line_numbers_active: Color::new(235, 219, 178),

            keyword: Color::new(251, 73, 52),
            control_flow: Color::new(251, 73, 52),
            type_color: Color::new(250, 189, 47),
            builtin: Color::new(250, 189, 47),
            function: Color::new(184, 187, 38),
            method: Color::new(184, 187, 38),
            macro_color: Color::new(184, 187, 38),
            string: Color::new(184, 187, 38),
            char_color: Color::new(184, 187, 38),
            number: Color::new(211, 134, 155),
            boolean: Color::new(211, 134, 155),
            null: Color::new(211, 134, 155),
            comment: Color::new(146, 131, 116),
            block_comment: Color::new(146, 131, 116),
            doc_comment: Color::new(146, 131, 116),
            operator: Color::new(235, 219, 178),
            delimiter: Color::new(235, 219, 178),
            punctuation: Color::new(235, 219, 178),
            attribute: Color::new(254, 128, 25),
            namespace: Color::new(250, 189, 47),
            variable: Color::new(131, 165, 152),
            parameter: Color::new(254, 128, 25),
            property: Color::new(235, 219, 178),
            constant: Color::new(211, 134, 155),
            label: Color::new(211, 134, 155),
            escape: Color::new(254, 128, 25),
            format_placeholder: Color::new(131, 165, 152),
            regex: Color::new(184, 187, 38),
            tag_name: Color::new(251, 73, 52),
            tag_attribute: Color::new(250, 189, 47),
            error: Color::new(251, 73, 52),
            special: Color::new(254, 128, 25),
        }
    }

    /// VS Code Dark+ color scheme
    pub fn vscode_dark() -> Self {
        Self {
            name: String::from("VS Code Dark+"),
            background: Color::new(30, 30, 30),
            foreground: Color::new(212, 212, 212),
            cursor: Color::new(255, 255, 255),
            selection: Color::new(38, 79, 120),
            line_highlight: Color::new(40, 40, 44),
            line_numbers: Color::new(133, 133, 133),
            line_numbers_active: Color::new(200, 200, 200),

            keyword: Color::new(86, 156, 214),
            control_flow: Color::new(197, 134, 192),
            type_color: Color::new(78, 201, 176),
            builtin: Color::new(78, 201, 176),
            function: Color::new(220, 220, 170),
            method: Color::new(220, 220, 170),
            macro_color: Color::new(220, 220, 170),
            string: Color::new(206, 145, 120),
            char_color: Color::new(206, 145, 120),
            number: Color::new(181, 206, 168),
            boolean: Color::new(86, 156, 214),
            null: Color::new(86, 156, 214),
            comment: Color::new(106, 153, 85),
            block_comment: Color::new(106, 153, 85),
            doc_comment: Color::new(106, 153, 85),
            operator: Color::new(212, 212, 212),
            delimiter: Color::new(212, 212, 212),
            punctuation: Color::new(212, 212, 212),
            attribute: Color::new(78, 201, 176),
            namespace: Color::new(78, 201, 176),
            variable: Color::new(156, 220, 254),
            parameter: Color::new(156, 220, 254),
            property: Color::new(156, 220, 254),
            constant: Color::new(100, 150, 200),
            label: Color::new(197, 134, 192),
            escape: Color::new(215, 186, 125),
            format_placeholder: Color::new(78, 201, 176),
            regex: Color::new(215, 186, 125),
            tag_name: Color::new(86, 156, 214),
            tag_attribute: Color::new(156, 220, 254),
            error: Color::new(244, 71, 71),
            special: Color::new(215, 186, 125),
        }
    }
}

/// Highlighter state (for multi-line tokens)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighlighterState {
    /// Normal state
    Normal,
    /// Inside multi-line string
    MultiLineString,
    /// Inside block comment
    BlockComment,
    /// Inside doc comment
    DocComment,
    /// Inside raw string (Rust r#""#)
    RawString(u8),
}

impl Default for HighlighterState {
    fn default() -> Self {
        Self::Normal
    }
}

/// Highlighted line result
#[derive(Debug, Clone)]
pub struct HighlightedLine {
    pub tokens: Vec<Token>,
    pub end_state: HighlighterState,
}

impl HighlightedLine {
    pub fn new() -> Self {
        Self {
            tokens: Vec::new(),
            end_state: HighlighterState::Normal,
        }
    }
}

impl Default for HighlightedLine {
    fn default() -> Self {
        Self::new()
    }
}

/// Syntax highlighter
pub struct SyntaxHighlighter {
    language: Language,
    scheme: ColorScheme,
}

impl SyntaxHighlighter {
    /// Create a new syntax highlighter
    pub fn new(language: Language) -> Self {
        Self {
            language,
            scheme: ColorScheme::default(),
        }
    }

    /// Create highlighter with specific color scheme
    pub fn with_scheme(language: Language, scheme: ColorScheme) -> Self {
        Self { language, scheme }
    }

    /// Get current language
    pub fn language(&self) -> Language {
        self.language
    }

    /// Set language
    pub fn set_language(&mut self, language: Language) {
        self.language = language;
    }

    /// Get color scheme
    pub fn scheme(&self) -> &ColorScheme {
        &self.scheme
    }

    /// Set color scheme
    pub fn set_scheme(&mut self, scheme: ColorScheme) {
        self.scheme = scheme;
    }

    /// Get color for token type
    pub fn color_for(&self, token_type: TokenType) -> Color {
        self.scheme.color_for(token_type)
    }

    /// Highlight a single line
    pub fn highlight_line(&self, line: &str, state: HighlighterState) -> HighlightedLine {
        match self.language {
            Language::Rust => self.highlight_rust(line, state),
            Language::Python => self.highlight_python(line, state),
            Language::JavaScript | Language::TypeScript => self.highlight_javascript(line, state),
            Language::C | Language::Cpp | Language::CSharp | Language::Java => self.highlight_c_like(line, state),
            Language::Go => self.highlight_go(line, state),
            Language::Html | Language::Xml => self.highlight_html(line, state),
            Language::Css | Language::Scss => self.highlight_css(line, state),
            Language::Json => self.highlight_json(line, state),
            Language::Shell => self.highlight_shell(line, state),
            Language::Markdown => self.highlight_markdown(line, state),
            _ => self.highlight_generic(line, state),
        }
    }

    /// Highlight Rust code
    fn highlight_rust(&self, line: &str, mut state: HighlighterState) -> HighlightedLine {
        let mut result = HighlightedLine::new();
        let chars: Vec<char> = line.chars().collect();
        let len = chars.len();
        let mut i = 0;

        // Continue from previous state
        if state == HighlighterState::BlockComment {
            while i < len {
                if i + 1 < len && chars[i] == '*' && chars[i + 1] == '/' {
                    result.tokens.push(Token::new(0, i + 2, TokenType::BlockComment));
                    i += 2;
                    state = HighlighterState::Normal;
                    break;
                }
                i += 1;
            }
            if state == HighlighterState::BlockComment {
                result.tokens.push(Token::new(0, len, TokenType::BlockComment));
                result.end_state = state;
                return result;
            }
        }

        while i < len {
            let start = i;

            // Skip whitespace
            if chars[i].is_whitespace() {
                i += 1;
                continue;
            }

            // Line comment
            if i + 1 < len && chars[i] == '/' && chars[i + 1] == '/' {
                // Doc comment
                let token_type = if i + 2 < len && (chars[i + 2] == '/' || chars[i + 2] == '!') {
                    TokenType::DocComment
                } else {
                    TokenType::Comment
                };
                result.tokens.push(Token::new(start, len, token_type));
                break;
            }

            // Block comment
            if i + 1 < len && chars[i] == '/' && chars[i + 1] == '*' {
                let doc = i + 2 < len && chars[i + 2] == '*';
                let token_type = if doc { TokenType::DocComment } else { TokenType::BlockComment };

                i += 2;
                while i + 1 < len {
                    if chars[i] == '*' && chars[i + 1] == '/' {
                        i += 2;
                        result.tokens.push(Token::new(start, i, token_type));
                        break;
                    }
                    i += 1;
                }
                if i >= len || (i + 1 >= len && !(chars[len - 2] == '*' && chars[len - 1] == '/')) {
                    result.tokens.push(Token::new(start, len, token_type));
                    result.end_state = HighlighterState::BlockComment;
                }
                continue;
            }

            // String literal
            if chars[i] == '"' {
                i += 1;
                while i < len && chars[i] != '"' {
                    if chars[i] == '\\' && i + 1 < len {
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                if i < len {
                    i += 1;
                }
                result.tokens.push(Token::new(start, i, TokenType::String));
                continue;
            }

            // Raw string
            if i + 1 < len && chars[i] == 'r' && (chars[i + 1] == '"' || chars[i + 1] == '#') {
                i += 1;
                let mut hashes = 0;
                while i < len && chars[i] == '#' {
                    hashes += 1;
                    i += 1;
                }
                if i < len && chars[i] == '"' {
                    i += 1;
                    loop {
                        if i >= len {
                            break;
                        }
                        if chars[i] == '"' {
                            let mut end_hashes = 0;
                            let mut j = i + 1;
                            while j < len && chars[j] == '#' && end_hashes < hashes {
                                end_hashes += 1;
                                j += 1;
                            }
                            if end_hashes == hashes {
                                i = j;
                                break;
                            }
                        }
                        i += 1;
                    }
                }
                result.tokens.push(Token::new(start, i, TokenType::String));
                continue;
            }

            // Character literal
            if chars[i] == '\'' {
                i += 1;
                // Check for lifetime or char
                if i < len {
                    if chars[i].is_alphabetic() || chars[i] == '_' {
                        // Could be lifetime
                        let ident_start = i;
                        while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                            i += 1;
                        }
                        if i < len && chars[i] == '\'' {
                            // It's a char
                            i += 1;
                            result.tokens.push(Token::new(start, i, TokenType::Char));
                        } else {
                            // It's a lifetime
                            result.tokens.push(Token::new(start, i, TokenType::Label));
                        }
                        continue;
                    } else if chars[i] == '\\' && i + 2 < len {
                        // Escape sequence
                        i += 2;
                        if i < len && chars[i] == '\'' {
                            i += 1;
                        }
                        result.tokens.push(Token::new(start, i, TokenType::Char));
                        continue;
                    } else if i + 1 < len && chars[i + 1] == '\'' {
                        // Single char
                        i += 2;
                        result.tokens.push(Token::new(start, i, TokenType::Char));
                        continue;
                    }
                }
                result.tokens.push(Token::new(start, i, TokenType::Normal));
                continue;
            }

            // Number
            if chars[i].is_numeric() || (chars[i] == '.' && i + 1 < len && chars[i + 1].is_numeric()) {
                // Hex, binary, octal
                if chars[i] == '0' && i + 1 < len {
                    if chars[i + 1] == 'x' || chars[i + 1] == 'X' {
                        i += 2;
                        while i < len && (chars[i].is_ascii_hexdigit() || chars[i] == '_') {
                            i += 1;
                        }
                    } else if chars[i + 1] == 'b' || chars[i + 1] == 'B' {
                        i += 2;
                        while i < len && (chars[i] == '0' || chars[i] == '1' || chars[i] == '_') {
                            i += 1;
                        }
                    } else if chars[i + 1] == 'o' || chars[i + 1] == 'O' {
                        i += 2;
                        while i < len && (chars[i] >= '0' && chars[i] <= '7' || chars[i] == '_') {
                            i += 1;
                        }
                    } else {
                        i += 1;
                    }
                } else {
                    while i < len && (chars[i].is_numeric() || chars[i] == '_' || chars[i] == '.') {
                        i += 1;
                    }
                }
                // Type suffix
                while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                result.tokens.push(Token::new(start, i, TokenType::Number));
                continue;
            }

            // Identifier or keyword
            if chars[i].is_alphabetic() || chars[i] == '_' {
                while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let ident: String = chars[start..i].iter().collect();

                // Check for macro
                if i < len && chars[i] == '!' {
                    i += 1;
                    result.tokens.push(Token::new(start, i, TokenType::Macro));
                    continue;
                }

                let token_type = match ident.as_str() {
                    // Keywords
                    "as" | "break" | "const" | "continue" | "crate" | "else" | "enum" |
                    "extern" | "fn" | "for" | "if" | "impl" | "in" | "let" | "loop" |
                    "match" | "mod" | "move" | "mut" | "pub" | "ref" | "return" |
                    "self" | "Self" | "static" | "struct" | "super" | "trait" | "type" |
                    "unsafe" | "use" | "where" | "while" | "async" | "await" | "dyn" => TokenType::Keyword,

                    // Control flow
                    "return" | "break" | "continue" => TokenType::ControlFlow,

                    // Boolean
                    "true" | "false" => TokenType::Boolean,

                    // Types
                    "bool" | "char" | "str" | "u8" | "u16" | "u32" | "u64" | "u128" |
                    "usize" | "i8" | "i16" | "i32" | "i64" | "i128" | "isize" |
                    "f32" | "f64" | "String" | "Vec" | "Option" | "Result" | "Box" |
                    "Rc" | "Arc" | "Cell" | "RefCell" | "Mutex" | "RwLock" |
                    "HashMap" | "BTreeMap" | "HashSet" | "BTreeSet" | "VecDeque" => TokenType::Type,

                    // Builtins
                    "Some" | "None" | "Ok" | "Err" => TokenType::Builtin,

                    // Check for constant (all caps)
                    _ if ident.chars().all(|c| c.is_uppercase() || c == '_' || c.is_numeric()) &&
                         ident.chars().any(|c| c.is_alphabetic()) => TokenType::Constant,

                    // Check for type (starts with uppercase)
                    _ if ident.chars().next().map_or(false, |c| c.is_uppercase()) => TokenType::Type,

                    _ => TokenType::Normal,
                };
                result.tokens.push(Token::new(start, i, token_type));
                continue;
            }

            // Attribute
            if chars[i] == '#' {
                if i + 1 < len && chars[i + 1] == '[' {
                    i += 2;
                    let mut depth = 1;
                    while i < len && depth > 0 {
                        if chars[i] == '[' {
                            depth += 1;
                        } else if chars[i] == ']' {
                            depth -= 1;
                        }
                        i += 1;
                    }
                    result.tokens.push(Token::new(start, i, TokenType::Attribute));
                    continue;
                }
            }

            // Operators
            if is_rust_operator(chars[i]) {
                while i < len && is_rust_operator(chars[i]) {
                    i += 1;
                }
                result.tokens.push(Token::new(start, i, TokenType::Operator));
                continue;
            }

            // Delimiters
            if chars[i] == '(' || chars[i] == ')' || chars[i] == '[' ||
               chars[i] == ']' || chars[i] == '{' || chars[i] == '}' {
                i += 1;
                result.tokens.push(Token::new(start, i, TokenType::Delimiter));
                continue;
            }

            // Punctuation
            if chars[i] == ',' || chars[i] == ';' || chars[i] == ':' || chars[i] == '.' {
                i += 1;
                result.tokens.push(Token::new(start, i, TokenType::Punctuation));
                continue;
            }

            // Unknown
            i += 1;
            result.tokens.push(Token::new(start, i, TokenType::Normal));
        }

        result.end_state = state;
        result
    }

    /// Highlight Python code
    fn highlight_python(&self, line: &str, state: HighlighterState) -> HighlightedLine {
        let mut result = HighlightedLine::new();
        let chars: Vec<char> = line.chars().collect();
        let len = chars.len();
        let mut i = 0;

        // Handle multi-line string continuation
        if state == HighlighterState::MultiLineString {
            while i + 2 < len {
                if chars[i] == '"' && chars[i + 1] == '"' && chars[i + 2] == '"' {
                    result.tokens.push(Token::new(0, i + 3, TokenType::String));
                    i += 3;
                    result.end_state = HighlighterState::Normal;
                    break;
                }
                i += 1;
            }
            if i + 2 >= len && state == HighlighterState::MultiLineString {
                result.tokens.push(Token::new(0, len, TokenType::String));
                result.end_state = HighlighterState::MultiLineString;
                return result;
            }
        }

        while i < len {
            let start = i;

            // Skip whitespace
            if chars[i].is_whitespace() {
                i += 1;
                continue;
            }

            // Comment
            if chars[i] == '#' {
                result.tokens.push(Token::new(start, len, TokenType::Comment));
                break;
            }

            // Triple-quoted string
            if i + 2 < len && chars[i] == '"' && chars[i + 1] == '"' && chars[i + 2] == '"' {
                i += 3;
                while i + 2 < len {
                    if chars[i] == '"' && chars[i + 1] == '"' && chars[i + 2] == '"' {
                        i += 3;
                        result.tokens.push(Token::new(start, i, TokenType::String));
                        break;
                    }
                    i += 1;
                }
                if i + 2 >= len {
                    result.tokens.push(Token::new(start, len, TokenType::String));
                    result.end_state = HighlighterState::MultiLineString;
                }
                continue;
            }

            // String
            if chars[i] == '"' || chars[i] == '\'' {
                let quote = chars[i];
                i += 1;
                while i < len && chars[i] != quote {
                    if chars[i] == '\\' && i + 1 < len {
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                if i < len {
                    i += 1;
                }
                result.tokens.push(Token::new(start, i, TokenType::String));
                continue;
            }

            // F-string prefix
            if (chars[i] == 'f' || chars[i] == 'r' || chars[i] == 'b' || chars[i] == 'F' || chars[i] == 'R' || chars[i] == 'B') &&
               i + 1 < len && (chars[i + 1] == '"' || chars[i + 1] == '\'') {
                let quote = chars[i + 1];
                i += 2;
                while i < len && chars[i] != quote {
                    if chars[i] == '\\' && i + 1 < len {
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                if i < len {
                    i += 1;
                }
                result.tokens.push(Token::new(start, i, TokenType::String));
                continue;
            }

            // Number
            if chars[i].is_numeric() || (chars[i] == '.' && i + 1 < len && chars[i + 1].is_numeric()) {
                if chars[i] == '0' && i + 1 < len {
                    if chars[i + 1] == 'x' || chars[i + 1] == 'X' {
                        i += 2;
                        while i < len && (chars[i].is_ascii_hexdigit() || chars[i] == '_') {
                            i += 1;
                        }
                    } else if chars[i + 1] == 'b' || chars[i + 1] == 'B' {
                        i += 2;
                        while i < len && (chars[i] == '0' || chars[i] == '1' || chars[i] == '_') {
                            i += 1;
                        }
                    } else if chars[i + 1] == 'o' || chars[i + 1] == 'O' {
                        i += 2;
                        while i < len && (chars[i] >= '0' && chars[i] <= '7' || chars[i] == '_') {
                            i += 1;
                        }
                    } else {
                        i += 1;
                    }
                } else {
                    while i < len && (chars[i].is_numeric() || chars[i] == '_' || chars[i] == '.' ||
                                      chars[i] == 'e' || chars[i] == 'E' || chars[i] == '+' || chars[i] == '-' ||
                                      chars[i] == 'j' || chars[i] == 'J') {
                        i += 1;
                    }
                }
                result.tokens.push(Token::new(start, i, TokenType::Number));
                continue;
            }

            // Identifier or keyword
            if chars[i].is_alphabetic() || chars[i] == '_' {
                while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let ident: String = chars[start..i].iter().collect();

                let token_type = match ident.as_str() {
                    // Keywords
                    "and" | "as" | "assert" | "async" | "await" | "break" | "class" |
                    "continue" | "def" | "del" | "elif" | "else" | "except" | "finally" |
                    "for" | "from" | "global" | "if" | "import" | "in" | "is" |
                    "lambda" | "nonlocal" | "not" | "or" | "pass" | "raise" | "return" |
                    "try" | "while" | "with" | "yield" => TokenType::Keyword,

                    // Boolean/None
                    "True" | "False" => TokenType::Boolean,
                    "None" => TokenType::Null,

                    // Builtins
                    "print" | "len" | "range" | "str" | "int" | "float" | "list" |
                    "dict" | "set" | "tuple" | "type" | "isinstance" | "hasattr" |
                    "getattr" | "setattr" | "open" | "input" | "map" | "filter" |
                    "zip" | "enumerate" | "sorted" | "reversed" | "sum" | "min" |
                    "max" | "abs" | "round" | "all" | "any" | "super" | "self" |
                    "cls" => TokenType::Builtin,

                    // Decorators are handled separately
                    _ if ident.starts_with('_') && ident.ends_with('_') && ident.len() > 2 => TokenType::Special,

                    _ if ident.chars().all(|c| c.is_uppercase() || c == '_') &&
                         ident.chars().any(|c| c.is_alphabetic()) => TokenType::Constant,

                    _ if ident.chars().next().map_or(false, |c| c.is_uppercase()) => TokenType::Type,

                    _ => TokenType::Normal,
                };
                result.tokens.push(Token::new(start, i, token_type));
                continue;
            }

            // Decorator
            if chars[i] == '@' {
                i += 1;
                while i < len && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '.') {
                    i += 1;
                }
                result.tokens.push(Token::new(start, i, TokenType::Attribute));
                continue;
            }

            // Operators
            if is_python_operator(chars[i]) {
                while i < len && is_python_operator(chars[i]) {
                    i += 1;
                }
                result.tokens.push(Token::new(start, i, TokenType::Operator));
                continue;
            }

            // Delimiters
            if chars[i] == '(' || chars[i] == ')' || chars[i] == '[' ||
               chars[i] == ']' || chars[i] == '{' || chars[i] == '}' {
                i += 1;
                result.tokens.push(Token::new(start, i, TokenType::Delimiter));
                continue;
            }

            // Punctuation
            if chars[i] == ',' || chars[i] == ';' || chars[i] == ':' || chars[i] == '.' {
                i += 1;
                result.tokens.push(Token::new(start, i, TokenType::Punctuation));
                continue;
            }

            // Unknown
            i += 1;
            result.tokens.push(Token::new(start, i, TokenType::Normal));
        }

        result
    }

    /// Highlight JavaScript/TypeScript code
    fn highlight_javascript(&self, line: &str, state: HighlighterState) -> HighlightedLine {
        let mut result = HighlightedLine::new();
        let chars: Vec<char> = line.chars().collect();
        let len = chars.len();
        let mut i = 0;

        // Handle block comment continuation
        if state == HighlighterState::BlockComment {
            while i < len {
                if i + 1 < len && chars[i] == '*' && chars[i + 1] == '/' {
                    result.tokens.push(Token::new(0, i + 2, TokenType::BlockComment));
                    i += 2;
                    result.end_state = HighlighterState::Normal;
                    break;
                }
                i += 1;
            }
            if i >= len && state == HighlighterState::BlockComment {
                result.tokens.push(Token::new(0, len, TokenType::BlockComment));
                result.end_state = HighlighterState::BlockComment;
                return result;
            }
        }

        while i < len {
            let start = i;

            // Skip whitespace
            if chars[i].is_whitespace() {
                i += 1;
                continue;
            }

            // Line comment
            if i + 1 < len && chars[i] == '/' && chars[i + 1] == '/' {
                result.tokens.push(Token::new(start, len, TokenType::Comment));
                break;
            }

            // Block comment
            if i + 1 < len && chars[i] == '/' && chars[i + 1] == '*' {
                let doc = i + 2 < len && chars[i + 2] == '*';
                let token_type = if doc { TokenType::DocComment } else { TokenType::BlockComment };

                i += 2;
                while i + 1 < len {
                    if chars[i] == '*' && chars[i + 1] == '/' {
                        i += 2;
                        result.tokens.push(Token::new(start, i, token_type));
                        break;
                    }
                    i += 1;
                }
                if i >= len {
                    result.tokens.push(Token::new(start, len, token_type));
                    result.end_state = HighlighterState::BlockComment;
                }
                continue;
            }

            // Template literal
            if chars[i] == '`' {
                i += 1;
                while i < len && chars[i] != '`' {
                    if chars[i] == '\\' && i + 1 < len {
                        i += 2;
                    } else if chars[i] == '$' && i + 1 < len && chars[i + 1] == '{' {
                        // Template expression - simple handling
                        i += 2;
                        let mut depth = 1;
                        while i < len && depth > 0 {
                            if chars[i] == '{' {
                                depth += 1;
                            } else if chars[i] == '}' {
                                depth -= 1;
                            }
                            i += 1;
                        }
                    } else {
                        i += 1;
                    }
                }
                if i < len {
                    i += 1;
                }
                result.tokens.push(Token::new(start, i, TokenType::String));
                continue;
            }

            // String
            if chars[i] == '"' || chars[i] == '\'' {
                let quote = chars[i];
                i += 1;
                while i < len && chars[i] != quote {
                    if chars[i] == '\\' && i + 1 < len {
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                if i < len {
                    i += 1;
                }
                result.tokens.push(Token::new(start, i, TokenType::String));
                continue;
            }

            // Number
            if chars[i].is_numeric() || (chars[i] == '.' && i + 1 < len && chars[i + 1].is_numeric()) {
                if chars[i] == '0' && i + 1 < len {
                    if chars[i + 1] == 'x' || chars[i + 1] == 'X' {
                        i += 2;
                        while i < len && (chars[i].is_ascii_hexdigit() || chars[i] == '_') {
                            i += 1;
                        }
                    } else if chars[i + 1] == 'b' || chars[i + 1] == 'B' {
                        i += 2;
                        while i < len && (chars[i] == '0' || chars[i] == '1' || chars[i] == '_') {
                            i += 1;
                        }
                    } else if chars[i + 1] == 'o' || chars[i + 1] == 'O' {
                        i += 2;
                        while i < len && (chars[i] >= '0' && chars[i] <= '7' || chars[i] == '_') {
                            i += 1;
                        }
                    } else {
                        i += 1;
                    }
                } else {
                    while i < len && (chars[i].is_numeric() || chars[i] == '_' || chars[i] == '.' ||
                                      chars[i] == 'e' || chars[i] == 'E' || chars[i] == '+' || chars[i] == '-') {
                        i += 1;
                    }
                }
                // BigInt suffix
                if i < len && chars[i] == 'n' {
                    i += 1;
                }
                result.tokens.push(Token::new(start, i, TokenType::Number));
                continue;
            }

            // Identifier or keyword
            if chars[i].is_alphabetic() || chars[i] == '_' || chars[i] == '$' {
                while i < len && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '$') {
                    i += 1;
                }
                let ident: String = chars[start..i].iter().collect();

                let token_type = match ident.as_str() {
                    // Keywords
                    "break" | "case" | "catch" | "class" | "const" | "continue" |
                    "debugger" | "default" | "delete" | "do" | "else" | "enum" |
                    "export" | "extends" | "finally" | "for" | "function" | "if" |
                    "implements" | "import" | "in" | "instanceof" | "interface" |
                    "let" | "new" | "package" | "private" | "protected" | "public" |
                    "return" | "static" | "super" | "switch" | "this" | "throw" |
                    "try" | "typeof" | "var" | "void" | "while" | "with" |
                    "yield" | "async" | "await" | "of" => TokenType::Keyword,

                    // Boolean/Null
                    "true" | "false" => TokenType::Boolean,
                    "null" | "undefined" => TokenType::Null,

                    // TypeScript specific
                    "type" | "namespace" | "module" | "declare" | "abstract" |
                    "readonly" | "as" | "is" | "keyof" | "infer" | "never" |
                    "unknown" | "any" if self.language == Language::TypeScript => TokenType::Keyword,

                    // Types
                    "Array" | "Object" | "String" | "Number" | "Boolean" | "Symbol" |
                    "BigInt" | "Function" | "Promise" | "Map" | "Set" | "WeakMap" |
                    "WeakSet" | "Date" | "RegExp" | "Error" | "TypeError" | "RangeError" |
                    "SyntaxError" | "ReferenceError" => TokenType::Type,

                    // Builtins
                    "console" | "window" | "document" | "Math" | "JSON" | "NaN" |
                    "Infinity" | "globalThis" => TokenType::Builtin,

                    _ if ident.chars().all(|c| c.is_uppercase() || c == '_') &&
                         ident.chars().any(|c| c.is_alphabetic()) => TokenType::Constant,

                    _ if ident.chars().next().map_or(false, |c| c.is_uppercase()) => TokenType::Type,

                    _ => TokenType::Normal,
                };
                result.tokens.push(Token::new(start, i, token_type));
                continue;
            }

            // Decorator (TypeScript)
            if chars[i] == '@' && self.language == Language::TypeScript {
                i += 1;
                while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                result.tokens.push(Token::new(start, i, TokenType::Attribute));
                continue;
            }

            // Arrow function
            if i + 1 < len && chars[i] == '=' && chars[i + 1] == '>' {
                i += 2;
                result.tokens.push(Token::new(start, i, TokenType::Operator));
                continue;
            }

            // Operators
            if is_js_operator(chars[i]) {
                while i < len && is_js_operator(chars[i]) {
                    i += 1;
                }
                result.tokens.push(Token::new(start, i, TokenType::Operator));
                continue;
            }

            // Delimiters
            if chars[i] == '(' || chars[i] == ')' || chars[i] == '[' ||
               chars[i] == ']' || chars[i] == '{' || chars[i] == '}' {
                i += 1;
                result.tokens.push(Token::new(start, i, TokenType::Delimiter));
                continue;
            }

            // Punctuation
            if chars[i] == ',' || chars[i] == ';' || chars[i] == ':' || chars[i] == '.' {
                i += 1;
                result.tokens.push(Token::new(start, i, TokenType::Punctuation));
                continue;
            }

            // Unknown
            i += 1;
            result.tokens.push(Token::new(start, i, TokenType::Normal));
        }

        result
    }

    /// Highlight C-like languages (C, C++, C#, Java)
    fn highlight_c_like(&self, line: &str, state: HighlighterState) -> HighlightedLine {
        // Similar structure to Rust/JS highlighting, with language-specific keywords
        let mut result = HighlightedLine::new();
        let chars: Vec<char> = line.chars().collect();
        let len = chars.len();
        let mut i = 0;

        // Handle block comment continuation
        if state == HighlighterState::BlockComment {
            while i < len {
                if i + 1 < len && chars[i] == '*' && chars[i + 1] == '/' {
                    result.tokens.push(Token::new(0, i + 2, TokenType::BlockComment));
                    i += 2;
                    result.end_state = HighlighterState::Normal;
                    break;
                }
                i += 1;
            }
            if i >= len && state == HighlighterState::BlockComment {
                result.tokens.push(Token::new(0, len, TokenType::BlockComment));
                result.end_state = HighlighterState::BlockComment;
                return result;
            }
        }

        while i < len {
            let start = i;

            if chars[i].is_whitespace() {
                i += 1;
                continue;
            }

            // Preprocessor (C/C++)
            if chars[i] == '#' && (self.language == Language::C || self.language == Language::Cpp) {
                result.tokens.push(Token::new(start, len, TokenType::Attribute));
                break;
            }

            // Line comment
            if i + 1 < len && chars[i] == '/' && chars[i + 1] == '/' {
                let doc = i + 2 < len && chars[i + 2] == '/';
                let token_type = if doc { TokenType::DocComment } else { TokenType::Comment };
                result.tokens.push(Token::new(start, len, token_type));
                break;
            }

            // Block comment
            if i + 1 < len && chars[i] == '/' && chars[i + 1] == '*' {
                let doc = i + 2 < len && chars[i + 2] == '*';
                let token_type = if doc { TokenType::DocComment } else { TokenType::BlockComment };

                i += 2;
                while i + 1 < len {
                    if chars[i] == '*' && chars[i + 1] == '/' {
                        i += 2;
                        result.tokens.push(Token::new(start, i, token_type));
                        break;
                    }
                    i += 1;
                }
                if i >= len {
                    result.tokens.push(Token::new(start, len, token_type));
                    result.end_state = HighlighterState::BlockComment;
                }
                continue;
            }

            // String
            if chars[i] == '"' {
                i += 1;
                while i < len && chars[i] != '"' {
                    if chars[i] == '\\' && i + 1 < len {
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                if i < len {
                    i += 1;
                }
                result.tokens.push(Token::new(start, i, TokenType::String));
                continue;
            }

            // Character
            if chars[i] == '\'' {
                i += 1;
                while i < len && chars[i] != '\'' {
                    if chars[i] == '\\' && i + 1 < len {
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                if i < len {
                    i += 1;
                }
                result.tokens.push(Token::new(start, i, TokenType::Char));
                continue;
            }

            // Number
            if chars[i].is_numeric() || (chars[i] == '.' && i + 1 < len && chars[i + 1].is_numeric()) {
                while i < len && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '.' ||
                                  chars[i] == 'x' || chars[i] == 'X' || chars[i] == '+' || chars[i] == '-') {
                    i += 1;
                }
                result.tokens.push(Token::new(start, i, TokenType::Number));
                continue;
            }

            // Identifier or keyword
            if chars[i].is_alphabetic() || chars[i] == '_' {
                while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let ident: String = chars[start..i].iter().collect();

                let token_type = self.classify_c_identifier(&ident);
                result.tokens.push(Token::new(start, i, token_type));
                continue;
            }

            // Annotation (Java/C#)
            if chars[i] == '@' && (self.language == Language::Java || self.language == Language::CSharp) {
                i += 1;
                while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                result.tokens.push(Token::new(start, i, TokenType::Attribute));
                continue;
            }

            // Operators
            if is_c_operator(chars[i]) {
                while i < len && is_c_operator(chars[i]) {
                    i += 1;
                }
                result.tokens.push(Token::new(start, i, TokenType::Operator));
                continue;
            }

            // Delimiters
            if chars[i] == '(' || chars[i] == ')' || chars[i] == '[' ||
               chars[i] == ']' || chars[i] == '{' || chars[i] == '}' {
                i += 1;
                result.tokens.push(Token::new(start, i, TokenType::Delimiter));
                continue;
            }

            // Punctuation
            if chars[i] == ',' || chars[i] == ';' || chars[i] == ':' || chars[i] == '.' {
                i += 1;
                result.tokens.push(Token::new(start, i, TokenType::Punctuation));
                continue;
            }

            // Unknown
            i += 1;
            result.tokens.push(Token::new(start, i, TokenType::Normal));
        }

        result
    }

    /// Classify C-like identifier
    fn classify_c_identifier(&self, ident: &str) -> TokenType {
        // C keywords
        let c_keywords = ["auto", "break", "case", "const", "continue", "default",
            "do", "else", "enum", "extern", "for", "goto", "if", "inline",
            "register", "restrict", "return", "sizeof", "static", "struct",
            "switch", "typedef", "union", "volatile", "while"];

        // C++ additional keywords
        let cpp_keywords = ["alignas", "alignof", "and", "and_eq", "asm", "bitand",
            "bitor", "catch", "class", "compl", "concept", "consteval", "constexpr",
            "constinit", "const_cast", "co_await", "co_return", "co_yield", "decltype",
            "delete", "dynamic_cast", "explicit", "export", "friend", "mutable",
            "namespace", "new", "noexcept", "not", "not_eq", "nullptr", "operator",
            "or", "or_eq", "override", "private", "protected", "public", "reflexpr",
            "reinterpret_cast", "requires", "static_assert", "static_cast", "template",
            "this", "thread_local", "throw", "try", "typeid", "typename", "using",
            "virtual", "xor", "xor_eq", "final"];

        // C# keywords
        let csharp_keywords = ["abstract", "as", "base", "bool", "break", "byte",
            "case", "catch", "char", "checked", "class", "const", "continue", "decimal",
            "default", "delegate", "do", "double", "else", "enum", "event", "explicit",
            "extern", "false", "finally", "fixed", "float", "for", "foreach", "goto",
            "if", "implicit", "in", "int", "interface", "internal", "is", "lock",
            "long", "namespace", "new", "null", "object", "operator", "out", "override",
            "params", "private", "protected", "public", "readonly", "ref", "return",
            "sbyte", "sealed", "short", "sizeof", "stackalloc", "static", "string",
            "struct", "switch", "this", "throw", "true", "try", "typeof", "uint",
            "ulong", "unchecked", "unsafe", "ushort", "using", "virtual", "void",
            "volatile", "while", "async", "await", "var", "dynamic", "yield"];

        // Java keywords
        let java_keywords = ["abstract", "assert", "boolean", "break", "byte", "case",
            "catch", "char", "class", "const", "continue", "default", "do", "double",
            "else", "enum", "extends", "final", "finally", "float", "for", "goto",
            "if", "implements", "import", "instanceof", "int", "interface", "long",
            "native", "new", "package", "private", "protected", "public", "return",
            "short", "static", "strictfp", "super", "switch", "synchronized", "this",
            "throw", "throws", "transient", "try", "void", "volatile", "while", "var",
            "record", "sealed", "permits", "yield"];

        // Types
        let types = ["void", "char", "short", "int", "long", "float", "double",
            "signed", "unsigned", "bool", "size_t", "int8_t", "int16_t", "int32_t",
            "int64_t", "uint8_t", "uint16_t", "uint32_t", "uint64_t", "string",
            "String", "Object", "Boolean", "Integer", "Long", "Double", "Float",
            "Char", "List", "ArrayList", "HashMap", "HashSet"];

        if matches!(self.language, Language::C | Language::Cpp) && c_keywords.contains(&ident) {
            return TokenType::Keyword;
        }
        if self.language == Language::Cpp && cpp_keywords.contains(&ident) {
            return TokenType::Keyword;
        }
        if self.language == Language::CSharp && csharp_keywords.contains(&ident) {
            return TokenType::Keyword;
        }
        if self.language == Language::Java && java_keywords.contains(&ident) {
            return TokenType::Keyword;
        }
        if types.contains(&ident) {
            return TokenType::Type;
        }
        if ident == "true" || ident == "false" {
            return TokenType::Boolean;
        }
        if ident == "null" || ident == "nullptr" || ident == "NULL" {
            return TokenType::Null;
        }
        if ident.chars().all(|c| c.is_uppercase() || c == '_') && ident.chars().any(|c| c.is_alphabetic()) {
            return TokenType::Constant;
        }
        if ident.chars().next().map_or(false, |c| c.is_uppercase()) {
            return TokenType::Type;
        }
        TokenType::Normal
    }

    // Stub implementations for other languages
    fn highlight_go(&self, line: &str, state: HighlighterState) -> HighlightedLine {
        self.highlight_c_like(line, state)
    }

    fn highlight_html(&self, line: &str, _state: HighlighterState) -> HighlightedLine {
        let mut result = HighlightedLine::new();
        let chars: Vec<char> = line.chars().collect();
        let len = chars.len();
        let mut i = 0;

        while i < len {
            let start = i;

            // Comment
            if i + 3 < len && chars[i] == '<' && chars[i + 1] == '!' &&
               chars[i + 2] == '-' && chars[i + 3] == '-' {
                i += 4;
                while i + 2 < len && !(chars[i] == '-' && chars[i + 1] == '-' && chars[i + 2] == '>') {
                    i += 1;
                }
                if i + 2 < len {
                    i += 3;
                }
                result.tokens.push(Token::new(start, i, TokenType::Comment));
                continue;
            }

            // Tag
            if chars[i] == '<' {
                i += 1;
                let closing = i < len && chars[i] == '/';
                if closing {
                    i += 1;
                }

                // Tag name
                let tag_start = i;
                while i < len && (chars[i].is_alphanumeric() || chars[i] == '-' || chars[i] == ':') {
                    i += 1;
                }
                if i > tag_start {
                    result.tokens.push(Token::new(start, tag_start, TokenType::Delimiter));
                    result.tokens.push(Token::new(tag_start, i, TokenType::TagName));
                }

                // Attributes
                while i < len && chars[i] != '>' {
                    if chars[i].is_whitespace() {
                        i += 1;
                        continue;
                    }

                    // Attribute name
                    if chars[i].is_alphabetic() || chars[i] == '-' || chars[i] == ':' {
                        let attr_start = i;
                        while i < len && (chars[i].is_alphanumeric() || chars[i] == '-' || chars[i] == ':') {
                            i += 1;
                        }
                        result.tokens.push(Token::new(attr_start, i, TokenType::TagAttribute));

                        // =
                        if i < len && chars[i] == '=' {
                            result.tokens.push(Token::new(i, i + 1, TokenType::Punctuation));
                            i += 1;
                        }

                        // Attribute value
                        if i < len && (chars[i] == '"' || chars[i] == '\'') {
                            let quote = chars[i];
                            let val_start = i;
                            i += 1;
                            while i < len && chars[i] != quote {
                                i += 1;
                            }
                            if i < len {
                                i += 1;
                            }
                            result.tokens.push(Token::new(val_start, i, TokenType::String));
                        }
                        continue;
                    }

                    if chars[i] == '/' {
                        i += 1;
                    } else {
                        i += 1;
                    }
                }

                if i < len && chars[i] == '>' {
                    result.tokens.push(Token::new(i, i + 1, TokenType::Delimiter));
                    i += 1;
                }
                continue;
            }

            // Text content
            while i < len && chars[i] != '<' {
                i += 1;
            }
            if i > start {
                result.tokens.push(Token::new(start, i, TokenType::Normal));
            }
        }

        result
    }

    fn highlight_css(&self, line: &str, state: HighlighterState) -> HighlightedLine {
        let mut result = HighlightedLine::new();
        let chars: Vec<char> = line.chars().collect();
        let len = chars.len();
        let mut i = 0;

        // Block comment continuation
        if state == HighlighterState::BlockComment {
            while i < len {
                if i + 1 < len && chars[i] == '*' && chars[i + 1] == '/' {
                    result.tokens.push(Token::new(0, i + 2, TokenType::BlockComment));
                    i += 2;
                    result.end_state = HighlighterState::Normal;
                    break;
                }
                i += 1;
            }
            if i >= len && state == HighlighterState::BlockComment {
                result.tokens.push(Token::new(0, len, TokenType::BlockComment));
                result.end_state = HighlighterState::BlockComment;
                return result;
            }
        }

        while i < len {
            let start = i;

            if chars[i].is_whitespace() {
                i += 1;
                continue;
            }

            // Comment
            if i + 1 < len && chars[i] == '/' && chars[i + 1] == '*' {
                i += 2;
                while i + 1 < len {
                    if chars[i] == '*' && chars[i + 1] == '/' {
                        i += 2;
                        result.tokens.push(Token::new(start, i, TokenType::BlockComment));
                        break;
                    }
                    i += 1;
                }
                if i >= len {
                    result.tokens.push(Token::new(start, len, TokenType::BlockComment));
                    result.end_state = HighlighterState::BlockComment;
                }
                continue;
            }

            // Selector or property
            if chars[i].is_alphabetic() || chars[i] == '_' || chars[i] == '-' ||
               chars[i] == '.' || chars[i] == '#' || chars[i] == '*' || chars[i] == ':' {
                while i < len && !chars[i].is_whitespace() && chars[i] != '{' &&
                      chars[i] != ':' && chars[i] != ';' && chars[i] != '}' {
                    i += 1;
                }
                let token = chars[start..i].iter().collect::<String>();
                let token_type = if token.starts_with('.') || token.starts_with('#') ||
                                    token.starts_with(':') || token.starts_with('*') {
                    TokenType::TagName
                } else {
                    TokenType::Property
                };
                result.tokens.push(Token::new(start, i, token_type));
                continue;
            }

            // String
            if chars[i] == '"' || chars[i] == '\'' {
                let quote = chars[i];
                i += 1;
                while i < len && chars[i] != quote {
                    if chars[i] == '\\' && i + 1 < len {
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                if i < len {
                    i += 1;
                }
                result.tokens.push(Token::new(start, i, TokenType::String));
                continue;
            }

            // Number
            if chars[i].is_numeric() || (chars[i] == '.' && i + 1 < len && chars[i + 1].is_numeric()) ||
               chars[i] == '#' {
                if chars[i] == '#' {
                    // Hex color
                    i += 1;
                    while i < len && chars[i].is_ascii_hexdigit() {
                        i += 1;
                    }
                } else {
                    while i < len && (chars[i].is_alphanumeric() || chars[i] == '.' || chars[i] == '%') {
                        i += 1;
                    }
                }
                result.tokens.push(Token::new(start, i, TokenType::Number));
                continue;
            }

            // Delimiters and punctuation
            if chars[i] == '{' || chars[i] == '}' || chars[i] == '(' || chars[i] == ')' ||
               chars[i] == '[' || chars[i] == ']' {
                i += 1;
                result.tokens.push(Token::new(start, i, TokenType::Delimiter));
                continue;
            }

            if chars[i] == ':' || chars[i] == ';' || chars[i] == ',' {
                i += 1;
                result.tokens.push(Token::new(start, i, TokenType::Punctuation));
                continue;
            }

            i += 1;
            result.tokens.push(Token::new(start, i, TokenType::Normal));
        }

        result
    }

    fn highlight_json(&self, line: &str, _state: HighlighterState) -> HighlightedLine {
        let mut result = HighlightedLine::new();
        let chars: Vec<char> = line.chars().collect();
        let len = chars.len();
        let mut i = 0;

        while i < len {
            let start = i;

            if chars[i].is_whitespace() {
                i += 1;
                continue;
            }

            // String (could be key or value)
            if chars[i] == '"' {
                i += 1;
                while i < len && chars[i] != '"' {
                    if chars[i] == '\\' && i + 1 < len {
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                if i < len {
                    i += 1;
                }
                // Check if followed by colon (it's a key)
                let mut j = i;
                while j < len && chars[j].is_whitespace() {
                    j += 1;
                }
                let token_type = if j < len && chars[j] == ':' {
                    TokenType::Property
                } else {
                    TokenType::String
                };
                result.tokens.push(Token::new(start, i, token_type));
                continue;
            }

            // Number
            if chars[i].is_numeric() || chars[i] == '-' {
                while i < len && (chars[i].is_numeric() || chars[i] == '.' ||
                                  chars[i] == 'e' || chars[i] == 'E' || chars[i] == '+' || chars[i] == '-') {
                    i += 1;
                }
                result.tokens.push(Token::new(start, i, TokenType::Number));
                continue;
            }

            // true/false/null
            if chars[i].is_alphabetic() {
                while i < len && chars[i].is_alphabetic() {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                let token_type = match word.as_str() {
                    "true" | "false" => TokenType::Boolean,
                    "null" => TokenType::Null,
                    _ => TokenType::Error,
                };
                result.tokens.push(Token::new(start, i, token_type));
                continue;
            }

            // Delimiters and punctuation
            if chars[i] == '{' || chars[i] == '}' || chars[i] == '[' || chars[i] == ']' {
                i += 1;
                result.tokens.push(Token::new(start, i, TokenType::Delimiter));
                continue;
            }

            if chars[i] == ':' || chars[i] == ',' {
                i += 1;
                result.tokens.push(Token::new(start, i, TokenType::Punctuation));
                continue;
            }

            i += 1;
            result.tokens.push(Token::new(start, i, TokenType::Normal));
        }

        result
    }

    fn highlight_shell(&self, line: &str, _state: HighlighterState) -> HighlightedLine {
        let mut result = HighlightedLine::new();
        let chars: Vec<char> = line.chars().collect();
        let len = chars.len();
        let mut i = 0;

        while i < len {
            let start = i;

            if chars[i].is_whitespace() {
                i += 1;
                continue;
            }

            // Comment
            if chars[i] == '#' {
                result.tokens.push(Token::new(start, len, TokenType::Comment));
                break;
            }

            // String
            if chars[i] == '"' || chars[i] == '\'' {
                let quote = chars[i];
                i += 1;
                while i < len && chars[i] != quote {
                    if chars[i] == '\\' && quote == '"' && i + 1 < len {
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                if i < len {
                    i += 1;
                }
                result.tokens.push(Token::new(start, i, TokenType::String));
                continue;
            }

            // Variable
            if chars[i] == '$' {
                i += 1;
                if i < len && chars[i] == '{' {
                    let var_start = start;
                    while i < len && chars[i] != '}' {
                        i += 1;
                    }
                    if i < len {
                        i += 1;
                    }
                    result.tokens.push(Token::new(var_start, i, TokenType::Variable));
                } else if i < len && chars[i] == '(' {
                    // Command substitution
                    let sub_start = start;
                    let mut depth = 1;
                    i += 1;
                    while i < len && depth > 0 {
                        if chars[i] == '(' {
                            depth += 1;
                        } else if chars[i] == ')' {
                            depth -= 1;
                        }
                        i += 1;
                    }
                    result.tokens.push(Token::new(sub_start, i, TokenType::Special));
                } else {
                    while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                        i += 1;
                    }
                    result.tokens.push(Token::new(start, i, TokenType::Variable));
                }
                continue;
            }

            // Number
            if chars[i].is_numeric() {
                while i < len && chars[i].is_numeric() {
                    i += 1;
                }
                result.tokens.push(Token::new(start, i, TokenType::Number));
                continue;
            }

            // Identifier or keyword
            if chars[i].is_alphabetic() || chars[i] == '_' {
                while i < len && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '-') {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                let token_type = match word.as_str() {
                    "if" | "then" | "else" | "elif" | "fi" | "case" | "esac" |
                    "for" | "while" | "until" | "do" | "done" | "in" |
                    "function" | "return" | "local" | "export" | "readonly" |
                    "declare" | "typeset" | "unset" | "source" | "alias" |
                    "break" | "continue" | "exit" | "trap" | "eval" | "exec" |
                    "shift" | "set" | "test" => TokenType::Keyword,
                    "true" | "false" => TokenType::Boolean,
                    "cd" | "pwd" | "echo" | "printf" | "read" | "cat" | "ls" |
                    "rm" | "cp" | "mv" | "mkdir" | "grep" | "sed" | "awk" |
                    "find" | "xargs" | "sort" | "uniq" | "wc" | "head" | "tail" |
                    "cut" | "tr" | "tee" | "chmod" | "chown" | "sudo" => TokenType::Builtin,
                    _ => TokenType::Normal,
                };
                result.tokens.push(Token::new(start, i, token_type));
                continue;
            }

            // Operators and special chars
            if chars[i] == '|' || chars[i] == '&' || chars[i] == '>' || chars[i] == '<' ||
               chars[i] == ';' || chars[i] == '!' {
                while i < len && (chars[i] == '|' || chars[i] == '&' || chars[i] == '>' ||
                                  chars[i] == '<' || chars[i] == '!' || chars[i] == '=') {
                    i += 1;
                }
                result.tokens.push(Token::new(start, i, TokenType::Operator));
                continue;
            }

            // Delimiters
            if chars[i] == '(' || chars[i] == ')' || chars[i] == '[' ||
               chars[i] == ']' || chars[i] == '{' || chars[i] == '}' {
                i += 1;
                result.tokens.push(Token::new(start, i, TokenType::Delimiter));
                continue;
            }

            i += 1;
            result.tokens.push(Token::new(start, i, TokenType::Normal));
        }

        result
    }

    fn highlight_markdown(&self, line: &str, _state: HighlighterState) -> HighlightedLine {
        let mut result = HighlightedLine::new();
        let chars: Vec<char> = line.chars().collect();
        let len = chars.len();
        let mut i = 0;

        // Check for heading
        if !chars.is_empty() && chars[0] == '#' {
            let mut level = 0;
            while i < len && chars[i] == '#' {
                level += 1;
                i += 1;
            }
            if level <= 6 && i < len && chars[i] == ' ' {
                result.tokens.push(Token::new(0, len, TokenType::Keyword));
                return result;
            }
        }

        // Code block marker
        if len >= 3 && chars[0] == '`' && chars[1] == '`' && chars[2] == '`' {
            result.tokens.push(Token::new(0, len, TokenType::Special));
            return result;
        }

        // List marker
        if !chars.is_empty() && (chars[0] == '-' || chars[0] == '*' || chars[0] == '+') &&
           len > 1 && chars[1] == ' ' {
            result.tokens.push(Token::new(0, 1, TokenType::Keyword));
            i = 2;
        }

        // Numbered list
        if !chars.is_empty() && chars[0].is_numeric() {
            let mut j = 0;
            while j < len && chars[j].is_numeric() {
                j += 1;
            }
            if j < len && chars[j] == '.' && j + 1 < len && chars[j + 1] == ' ' {
                result.tokens.push(Token::new(0, j + 1, TokenType::Keyword));
                i = j + 2;
            }
        }

        // Blockquote
        if !chars.is_empty() && chars[0] == '>' {
            result.tokens.push(Token::new(0, 1, TokenType::Keyword));
            i = 1;
        }

        while i < len {
            let start = i;

            // Inline code
            if chars[i] == '`' {
                i += 1;
                while i < len && chars[i] != '`' {
                    i += 1;
                }
                if i < len {
                    i += 1;
                }
                result.tokens.push(Token::new(start, i, TokenType::String));
                continue;
            }

            // Bold
            if i + 1 < len && chars[i] == '*' && chars[i + 1] == '*' {
                i += 2;
                let content_start = i;
                while i + 1 < len && !(chars[i] == '*' && chars[i + 1] == '*') {
                    i += 1;
                }
                if i + 1 < len {
                    result.tokens.push(Token::new(start, i + 2, TokenType::Keyword));
                    i += 2;
                } else {
                    result.tokens.push(Token::new(start, len, TokenType::Normal));
                    i = len;
                }
                continue;
            }

            // Italic
            if chars[i] == '*' || chars[i] == '_' {
                let marker = chars[i];
                i += 1;
                while i < len && chars[i] != marker {
                    i += 1;
                }
                if i < len {
                    i += 1;
                    result.tokens.push(Token::new(start, i, TokenType::Property));
                } else {
                    result.tokens.push(Token::new(start, len, TokenType::Normal));
                }
                continue;
            }

            // Link
            if chars[i] == '[' {
                i += 1;
                while i < len && chars[i] != ']' {
                    i += 1;
                }
                if i < len && i + 1 < len && chars[i] == ']' && chars[i + 1] == '(' {
                    i += 2;
                    while i < len && chars[i] != ')' {
                        i += 1;
                    }
                    if i < len {
                        i += 1;
                    }
                    result.tokens.push(Token::new(start, i, TokenType::TagName));
                    continue;
                }
            }

            // Normal text
            while i < len && chars[i] != '`' && chars[i] != '*' && chars[i] != '_' && chars[i] != '[' {
                i += 1;
            }
            if i > start {
                result.tokens.push(Token::new(start, i, TokenType::Normal));
            }
        }

        result
    }

    fn highlight_generic(&self, line: &str, _state: HighlighterState) -> HighlightedLine {
        let mut result = HighlightedLine::new();
        if !line.is_empty() {
            result.tokens.push(Token::new(0, line.len(), TokenType::Normal));
        }
        result
    }
}

// Helper functions for operator detection
fn is_rust_operator(c: char) -> bool {
    matches!(c, '+' | '-' | '*' | '/' | '%' | '=' | '<' | '>' | '!' | '&' | '|' | '^' | '~' | '?')
}

fn is_python_operator(c: char) -> bool {
    matches!(c, '+' | '-' | '*' | '/' | '%' | '=' | '<' | '>' | '!' | '&' | '|' | '^' | '~' | '@')
}

fn is_js_operator(c: char) -> bool {
    matches!(c, '+' | '-' | '*' | '/' | '%' | '=' | '<' | '>' | '!' | '&' | '|' | '^' | '~' | '?')
}

fn is_c_operator(c: char) -> bool {
    matches!(c, '+' | '-' | '*' | '/' | '%' | '=' | '<' | '>' | '!' | '&' | '|' | '^' | '~' | '?')
}

/// Initialize syntax highlighting module
pub fn init() {
    // Nothing to initialize
}
