//! Browser Module
//!
//! Complete web browser implementation with HTML/CSS rendering engine.

pub mod engine;
pub mod html_parser;
pub mod css_parser;
pub mod layout;
pub mod render;
pub mod dom;
pub mod javascript;
pub mod tabs;
pub mod downloads;
pub mod network;

pub use engine::BrowserEngine;
pub use render::{RenderTree, RenderNode};
pub use html_parser::{HtmlParser, HtmlDocument, HtmlElement, HtmlNode};
pub use css_parser::{CssParser, StyleSheet, CssRule, CssSelector, CssDeclaration};
pub use layout::{LayoutEngine, LayoutBox, LayoutMode, BoxDimensions};
pub use render::{Renderer, PaintCommand};
pub use dom::{Dom, DomNode, DomNodeType, DomElement, DomText};
pub use javascript::{JsEngine, JsValue, JsContext, JsError};
pub use tabs::{TabManager, Tab, TabId, TabState};
pub use downloads::{DownloadManager, Download, DownloadState, DownloadError};
