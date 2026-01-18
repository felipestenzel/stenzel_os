//! Renderer
//!
//! Renders layout tree to paint commands.

use alloc::string::String;
use alloc::vec::Vec;
use super::layout::{LayoutBox, LayoutMode, BoxDimensions, Rect, ComputedStyle, TextDecoration};
use super::css_parser::CssColor;

/// Renderer for layout tree
pub struct Renderer {
    /// Paint commands
    commands: Vec<PaintCommand>,
    /// Scroll offset X
    pub scroll_x: f32,
    /// Scroll offset Y
    pub scroll_y: f32,
}

impl Renderer {
    /// Create new renderer
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            scroll_x: 0.0,
            scroll_y: 0.0,
        }
    }

    /// Render layout tree to paint commands
    pub fn render(&mut self, root: &LayoutBox) -> Vec<PaintCommand> {
        self.commands.clear();
        self.render_layout_box(root, 0.0, 0.0);
        self.commands.clone()
    }

    /// Render a single layout box
    fn render_layout_box(&mut self, layout_box: &LayoutBox, offset_x: f32, offset_y: f32) {
        if layout_box.layout_mode == LayoutMode::None || !layout_box.style.visible {
            return;
        }

        // Calculate absolute position
        let x = layout_box.dimensions.content.x + offset_x - self.scroll_x;
        let y = layout_box.dimensions.content.y + offset_y - self.scroll_y;

        let border_box = layout_box.dimensions.border_box();
        let padding_box = layout_box.dimensions.padding_box();

        // Render background
        if let Some(bg_color) = &layout_box.style.background_color {
            self.commands.push(PaintCommand::FillRect {
                x: border_box.x + offset_x - self.scroll_x,
                y: border_box.y + offset_y - self.scroll_y,
                width: border_box.width,
                height: border_box.height,
                color: *bg_color,
                border_radius: layout_box.style.border_radius,
            });
        }

        // Render border
        if layout_box.style.border_top_width > 0.0 ||
           layout_box.style.border_right_width > 0.0 ||
           layout_box.style.border_bottom_width > 0.0 ||
           layout_box.style.border_left_width > 0.0 {
            if let Some(border_color) = &layout_box.style.border_color {
                // Top border
                if layout_box.style.border_top_width > 0.0 {
                    self.commands.push(PaintCommand::FillRect {
                        x: border_box.x + offset_x - self.scroll_x,
                        y: border_box.y + offset_y - self.scroll_y,
                        width: border_box.width,
                        height: layout_box.style.border_top_width,
                        color: *border_color,
                        border_radius: 0.0,
                    });
                }
                // Right border
                if layout_box.style.border_right_width > 0.0 {
                    self.commands.push(PaintCommand::FillRect {
                        x: border_box.x + border_box.width - layout_box.style.border_right_width + offset_x - self.scroll_x,
                        y: border_box.y + offset_y - self.scroll_y,
                        width: layout_box.style.border_right_width,
                        height: border_box.height,
                        color: *border_color,
                        border_radius: 0.0,
                    });
                }
                // Bottom border
                if layout_box.style.border_bottom_width > 0.0 {
                    self.commands.push(PaintCommand::FillRect {
                        x: border_box.x + offset_x - self.scroll_x,
                        y: border_box.y + border_box.height - layout_box.style.border_bottom_width + offset_y - self.scroll_y,
                        width: border_box.width,
                        height: layout_box.style.border_bottom_width,
                        color: *border_color,
                        border_radius: 0.0,
                    });
                }
                // Left border
                if layout_box.style.border_left_width > 0.0 {
                    self.commands.push(PaintCommand::FillRect {
                        x: border_box.x + offset_x - self.scroll_x,
                        y: border_box.y + offset_y - self.scroll_y,
                        width: layout_box.style.border_left_width,
                        height: border_box.height,
                        color: *border_color,
                        border_radius: 0.0,
                    });
                }
            }
        }

        // Render text content
        if let Some(text) = &layout_box.text_content {
            let text_color = layout_box.style.color.unwrap_or(CssColor { r: 0, g: 0, b: 0, a: 255 });

            self.commands.push(PaintCommand::Text {
                x,
                y,
                text: text.clone(),
                color: text_color,
                font_size: layout_box.style.font_size,
                bold: layout_box.style.font_weight == super::layout::FontWeight::Bold,
                italic: layout_box.style.font_style == super::layout::FontStyle::Italic,
            });

            // Text decoration
            match layout_box.style.text_decoration {
                TextDecoration::Underline => {
                    self.commands.push(PaintCommand::Line {
                        x1: x,
                        y1: y + layout_box.style.font_size,
                        x2: x + layout_box.dimensions.content.width,
                        y2: y + layout_box.style.font_size,
                        color: text_color,
                        width: 1.0,
                    });
                }
                TextDecoration::LineThrough => {
                    self.commands.push(PaintCommand::Line {
                        x1: x,
                        y1: y + layout_box.style.font_size / 2.0,
                        x2: x + layout_box.dimensions.content.width,
                        y2: y + layout_box.style.font_size / 2.0,
                        color: text_color,
                        width: 1.0,
                    });
                }
                TextDecoration::Overline => {
                    self.commands.push(PaintCommand::Line {
                        x1: x,
                        y1: y,
                        x2: x + layout_box.dimensions.content.width,
                        y2: y,
                        color: text_color,
                        width: 1.0,
                    });
                }
                _ => {}
            }
        }

        // Render children
        for child in &layout_box.children {
            self.render_layout_box(child, x, y);
        }
    }

    /// Set scroll position
    pub fn set_scroll(&mut self, x: f32, y: f32) {
        self.scroll_x = x;
        self.scroll_y = y;
    }

    /// Get scroll position
    pub fn scroll(&self) -> (f32, f32) {
        (self.scroll_x, self.scroll_y)
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

/// Paint command for rendering
#[derive(Debug, Clone)]
pub enum PaintCommand {
    /// Fill rectangle
    FillRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: CssColor,
        border_radius: f32,
    },
    /// Stroke rectangle
    StrokeRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: CssColor,
        line_width: f32,
    },
    /// Draw text
    Text {
        x: f32,
        y: f32,
        text: String,
        color: CssColor,
        font_size: f32,
        bold: bool,
        italic: bool,
    },
    /// Draw line
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        color: CssColor,
        width: f32,
    },
    /// Draw image
    Image {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        src: String,
    },
    /// Clip region (for overflow)
    ClipRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },
    /// Restore clip
    RestoreClip,
    /// Set opacity
    SetOpacity(f32),
    /// Restore opacity
    RestoreOpacity,
}

/// Render tree (styled layout tree ready for painting)
#[derive(Debug, Clone)]
pub struct RenderTree {
    /// Root of render tree
    pub root: Option<RenderNode>,
    /// Total content height
    pub content_height: f32,
    /// Total content width
    pub content_width: f32,
}

impl RenderTree {
    /// Create new empty render tree
    pub fn new() -> Self {
        Self {
            root: None,
            content_height: 0.0,
            content_width: 0.0,
        }
    }

    /// Build render tree from layout
    pub fn from_layout(layout: LayoutBox) -> Self {
        let content_height = layout.dimensions.margin_box().height;
        let content_width = layout.dimensions.margin_box().width;

        Self {
            root: Some(RenderNode::from_layout(layout)),
            content_height,
            content_width,
        }
    }
}

impl Default for RenderTree {
    fn default() -> Self {
        Self::new()
    }
}

/// Render node
#[derive(Debug, Clone)]
pub struct RenderNode {
    /// Layout info
    pub bounds: Rect,
    /// Background color
    pub background: Option<CssColor>,
    /// Border color
    pub border_color: Option<CssColor>,
    /// Border widths
    pub border_widths: [f32; 4],
    /// Border radius
    pub border_radius: f32,
    /// Text content
    pub text: Option<TextRender>,
    /// Opacity
    pub opacity: f32,
    /// Visibility
    pub visible: bool,
    /// Children
    pub children: Vec<RenderNode>,
}

impl RenderNode {
    /// Create from layout box
    pub fn from_layout(layout: LayoutBox) -> Self {
        let text = layout.text_content.as_ref().map(|t| TextRender {
            content: t.clone(),
            color: layout.style.color.unwrap_or(CssColor { r: 0, g: 0, b: 0, a: 255 }),
            font_size: layout.style.font_size,
            bold: layout.style.font_weight == super::layout::FontWeight::Bold,
            italic: layout.style.font_style == super::layout::FontStyle::Italic,
            decoration: layout.style.text_decoration,
        });

        Self {
            bounds: layout.dimensions.border_box(),
            background: layout.style.background_color,
            border_color: layout.style.border_color,
            border_widths: [
                layout.style.border_top_width,
                layout.style.border_right_width,
                layout.style.border_bottom_width,
                layout.style.border_left_width,
            ],
            border_radius: layout.style.border_radius,
            text,
            opacity: layout.style.opacity,
            visible: layout.style.visible,
            children: layout.children.into_iter().map(RenderNode::from_layout).collect(),
        }
    }
}

/// Text rendering info
#[derive(Debug, Clone)]
pub struct TextRender {
    /// Text content
    pub content: String,
    /// Text color
    pub color: CssColor,
    /// Font size
    pub font_size: f32,
    /// Is bold
    pub bold: bool,
    /// Is italic
    pub italic: bool,
    /// Text decoration
    pub decoration: TextDecoration,
}

/// Display list for painting
#[derive(Debug, Clone)]
pub struct DisplayList {
    /// Items to paint
    pub items: Vec<DisplayItem>,
}

impl DisplayList {
    /// Create empty display list
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Build from render tree
    pub fn from_render_tree(tree: &RenderTree, scroll_x: f32, scroll_y: f32) -> Self {
        let mut list = Self::new();
        if let Some(root) = &tree.root {
            list.build_from_node(root, 0.0, 0.0, scroll_x, scroll_y);
        }
        list
    }

    fn build_from_node(&mut self, node: &RenderNode, offset_x: f32, offset_y: f32, scroll_x: f32, scroll_y: f32) {
        if !node.visible {
            return;
        }

        let x = node.bounds.x + offset_x - scroll_x;
        let y = node.bounds.y + offset_y - scroll_y;

        // Background
        if let Some(bg) = &node.background {
            self.items.push(DisplayItem::SolidColor {
                rect: DisplayRect {
                    x,
                    y,
                    width: node.bounds.width,
                    height: node.bounds.height,
                },
                color: *bg,
            });
        }

        // Border
        if let Some(bc) = &node.border_color {
            // Top
            if node.border_widths[0] > 0.0 {
                self.items.push(DisplayItem::SolidColor {
                    rect: DisplayRect {
                        x,
                        y,
                        width: node.bounds.width,
                        height: node.border_widths[0],
                    },
                    color: *bc,
                });
            }
            // Right
            if node.border_widths[1] > 0.0 {
                self.items.push(DisplayItem::SolidColor {
                    rect: DisplayRect {
                        x: x + node.bounds.width - node.border_widths[1],
                        y,
                        width: node.border_widths[1],
                        height: node.bounds.height,
                    },
                    color: *bc,
                });
            }
            // Bottom
            if node.border_widths[2] > 0.0 {
                self.items.push(DisplayItem::SolidColor {
                    rect: DisplayRect {
                        x,
                        y: y + node.bounds.height - node.border_widths[2],
                        width: node.bounds.width,
                        height: node.border_widths[2],
                    },
                    color: *bc,
                });
            }
            // Left
            if node.border_widths[3] > 0.0 {
                self.items.push(DisplayItem::SolidColor {
                    rect: DisplayRect {
                        x,
                        y,
                        width: node.border_widths[3],
                        height: node.bounds.height,
                    },
                    color: *bc,
                });
            }
        }

        // Text
        if let Some(text) = &node.text {
            self.items.push(DisplayItem::Text {
                rect: DisplayRect {
                    x: x + node.border_widths[3],
                    y: y + node.border_widths[0],
                    width: node.bounds.width - node.border_widths[1] - node.border_widths[3],
                    height: node.bounds.height - node.border_widths[0] - node.border_widths[2],
                },
                text: text.content.clone(),
                color: text.color,
                font_size: text.font_size,
            });
        }

        // Children
        for child in &node.children {
            self.build_from_node(child, x, y, 0.0, 0.0);
        }
    }
}

impl Default for DisplayList {
    fn default() -> Self {
        Self::new()
    }
}

/// Display item
#[derive(Debug, Clone)]
pub enum DisplayItem {
    /// Solid color rectangle
    SolidColor {
        rect: DisplayRect,
        color: CssColor,
    },
    /// Text
    Text {
        rect: DisplayRect,
        text: String,
        color: CssColor,
        font_size: f32,
    },
    /// Image
    Image {
        rect: DisplayRect,
        src: String,
    },
}

/// Display rectangle
#[derive(Debug, Clone, Copy)]
pub struct DisplayRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
