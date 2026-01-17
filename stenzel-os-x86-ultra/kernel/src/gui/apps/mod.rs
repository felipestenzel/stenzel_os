//! GUI Applications
//!
//! Built-in graphical applications for Stenzel OS.

pub mod browser;
pub mod filemanager;
pub mod settings;
pub mod taskmanager;
pub mod terminal;
pub mod texteditor;
pub mod imageviewer;
pub mod calculator;

pub use browser::Browser;
pub use filemanager::FileManager;
pub use settings::Settings;
pub use taskmanager::TaskManager;
pub use terminal::Terminal;
pub use texteditor::TextEditor;
pub use imageviewer::{ImageViewer, Image, ImageFormat, ZoomMode, create_test_image};
pub use calculator::Calculator;
