//! GUI Applications
//!
//! Built-in graphical applications for Stenzel OS.

pub mod browser;
pub mod calculator;
pub mod filemanager;
pub mod imageviewer;
pub mod pdfviewer;
pub mod screenshot;
pub mod search;
pub mod settings;
pub mod softwarecenter;
pub mod taskmanager;
pub mod terminal;
pub mod texteditor;
pub mod trash;
pub mod videoplayer;

pub use browser::{
    BrowserEngine, RenderTree, RenderNode,
    HtmlParser, HtmlDocument, HtmlElement, HtmlNode,
    CssParser, StyleSheet, CssRule, CssSelector, CssDeclaration,
    LayoutEngine, LayoutBox, LayoutMode, BoxDimensions,
    Renderer, PaintCommand,
    Dom, DomNode, DomNodeType, DomElement, DomText,
    JsEngine, JsValue, JsContext, JsError,
    TabManager, Tab, TabId, TabState,
    DownloadManager, Download, DownloadState, DownloadError,
};
pub use calculator::Calculator;
pub use filemanager::FileManager;
pub use imageviewer::{ImageViewer, Image, ImageFormat, ZoomMode, create_test_image};
pub use pdfviewer::{PdfViewer, PdfDocument, PdfPage};
pub use screenshot::{ScreenshotWidget, CaptureMode, ImageFormat as ScreenshotFormat};
pub use search::{SearchWidget, SearchFilter, SearchResult};
pub use settings::Settings;
pub use taskmanager::TaskManager;
pub use terminal::Terminal;
pub use texteditor::TextEditor;
pub use trash::{TrashViewer, TrashedItem, TrashFileType};
pub use videoplayer::{VideoPlayer, MediaInfo, PlaybackState};
pub use softwarecenter::{SoftwareCenter, SoftwareCenterView, AppCategory, AppEntry, UpdateNotificationService};
