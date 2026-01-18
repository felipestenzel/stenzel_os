//! Layout Engine
//!
//! CSS box model layout engine.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use super::dom::{DomNode, DomNodeType};
use super::css_parser::{StyleSheet, CssDeclaration, CssLength, LengthUnit, CssColor};

/// Layout engine
pub struct LayoutEngine {
    /// Viewport width
    pub viewport_width: f32,
    /// Viewport height
    pub viewport_height: f32,
    /// Default font size
    pub default_font_size: f32,
    /// Stylesheet
    pub stylesheet: Option<StyleSheet>,
}

impl LayoutEngine {
    /// Create new layout engine
    pub fn new(viewport_width: f32, viewport_height: f32) -> Self {
        Self {
            viewport_width,
            viewport_height,
            default_font_size: 16.0,
            stylesheet: None,
        }
    }

    /// Set stylesheet
    pub fn set_stylesheet(&mut self, stylesheet: StyleSheet) {
        self.stylesheet = Some(stylesheet);
    }

    /// Layout a DOM tree
    pub fn layout(&self, root: &DomNode) -> LayoutBox {
        let mut root_box = self.build_layout_tree(root);
        root_box.layout(self.viewport_width, self.viewport_height, self);
        root_box
    }

    /// Build layout tree from DOM
    fn build_layout_tree(&self, node: &DomNode) -> LayoutBox {
        let mut layout_box = LayoutBox::new(self.get_display_mode(node));

        // Copy node info
        layout_box.node_type = Some(node.node_type.clone());
        layout_box.tag_name = node.tag_name().map(String::from);
        layout_box.text_content = node.text.as_ref().map(|t| t.content.clone());

        // Apply styles
        self.apply_styles(node, &mut layout_box);

        // Build children
        for child in &node.children {
            if self.is_displayable(child) {
                let child_box = self.build_layout_tree(child);
                layout_box.children.push(child_box);
            }
        }

        layout_box
    }

    fn get_display_mode(&self, node: &DomNode) -> LayoutMode {
        match node.node_type {
            DomNodeType::Text => LayoutMode::Inline,
            DomNodeType::Element => {
                // Check for explicit display style
                if let Some(element) = &node.element {
                    if let Some(display) = element.style.get("display") {
                        return match display.as_str() {
                            "none" => LayoutMode::None,
                            "inline" => LayoutMode::Inline,
                            "inline-block" => LayoutMode::InlineBlock,
                            "flex" => LayoutMode::Flex,
                            "inline-flex" => LayoutMode::InlineFlex,
                            "grid" => LayoutMode::Grid,
                            _ => LayoutMode::Block,
                        };
                    }

                    // Default based on tag
                    if element.is_block() {
                        return LayoutMode::Block;
                    }
                }
                LayoutMode::Inline
            }
            _ => LayoutMode::Block,
        }
    }

    fn is_displayable(&self, node: &DomNode) -> bool {
        match node.node_type {
            DomNodeType::Text => {
                node.text.as_ref().map(|t| !t.is_whitespace).unwrap_or(false)
            }
            DomNodeType::Element => {
                if let Some(element) = &node.element {
                    // Skip non-visual elements
                    if matches!(element.tag_name.as_str(), "script" | "style" | "meta" | "link" | "head" | "title") {
                        return false;
                    }
                    // Check display: none
                    if element.style.get("display") == Some(&String::from("none")) {
                        return false;
                    }
                }
                true
            }
            DomNodeType::Comment => false,
            _ => true,
        }
    }

    fn apply_styles(&self, node: &DomNode, layout_box: &mut LayoutBox) {
        // Default styles
        layout_box.style.font_size = self.default_font_size;

        // Apply inline styles
        if let Some(element) = &node.element {
            for (property, value) in &element.style {
                self.apply_style_property(property, value, layout_box);
            }
        }

        // Apply stylesheet rules
        if let Some(stylesheet) = &self.stylesheet {
            if let Some(element) = &node.element {
                let tag = &element.tag_name;
                let id = element.attributes.get("id").map(|s| s.as_str());
                let classes: Vec<&str> = element.attributes
                    .get("class")
                    .map(|s| s.split_whitespace().collect())
                    .unwrap_or_default();

                let matching_rules = stylesheet.get_matching_rules(tag, id, &classes);

                for rule in matching_rules {
                    for decl in &rule.declarations {
                        self.apply_style_property(&decl.property, &decl.value, layout_box);
                    }
                }
            }
        }
    }

    fn apply_style_property(&self, property: &str, value: &str, layout_box: &mut LayoutBox) {
        match property {
            // Dimensions
            "width" => {
                if let Some(len) = CssLength::parse(value) {
                    layout_box.style.width = Some(len);
                }
            }
            "height" => {
                if let Some(len) = CssLength::parse(value) {
                    layout_box.style.height = Some(len);
                }
            }
            "min-width" => {
                if let Some(len) = CssLength::parse(value) {
                    layout_box.style.min_width = Some(len);
                }
            }
            "min-height" => {
                if let Some(len) = CssLength::parse(value) {
                    layout_box.style.min_height = Some(len);
                }
            }
            "max-width" => {
                if let Some(len) = CssLength::parse(value) {
                    layout_box.style.max_width = Some(len);
                }
            }
            "max-height" => {
                if let Some(len) = CssLength::parse(value) {
                    layout_box.style.max_height = Some(len);
                }
            }

            // Margin
            "margin" => {
                if let Some(len) = CssLength::parse(value) {
                    layout_box.style.margin_top = Some(len);
                    layout_box.style.margin_right = Some(len);
                    layout_box.style.margin_bottom = Some(len);
                    layout_box.style.margin_left = Some(len);
                }
            }
            "margin-top" => {
                if let Some(len) = CssLength::parse(value) {
                    layout_box.style.margin_top = Some(len);
                }
            }
            "margin-right" => {
                if let Some(len) = CssLength::parse(value) {
                    layout_box.style.margin_right = Some(len);
                }
            }
            "margin-bottom" => {
                if let Some(len) = CssLength::parse(value) {
                    layout_box.style.margin_bottom = Some(len);
                }
            }
            "margin-left" => {
                if let Some(len) = CssLength::parse(value) {
                    layout_box.style.margin_left = Some(len);
                }
            }

            // Padding
            "padding" => {
                if let Some(len) = CssLength::parse(value) {
                    layout_box.style.padding_top = Some(len);
                    layout_box.style.padding_right = Some(len);
                    layout_box.style.padding_bottom = Some(len);
                    layout_box.style.padding_left = Some(len);
                }
            }
            "padding-top" => {
                if let Some(len) = CssLength::parse(value) {
                    layout_box.style.padding_top = Some(len);
                }
            }
            "padding-right" => {
                if let Some(len) = CssLength::parse(value) {
                    layout_box.style.padding_right = Some(len);
                }
            }
            "padding-bottom" => {
                if let Some(len) = CssLength::parse(value) {
                    layout_box.style.padding_bottom = Some(len);
                }
            }
            "padding-left" => {
                if let Some(len) = CssLength::parse(value) {
                    layout_box.style.padding_left = Some(len);
                }
            }

            // Border
            "border-width" => {
                if let Some(len) = CssLength::parse(value) {
                    layout_box.style.border_top_width = len.value;
                    layout_box.style.border_right_width = len.value;
                    layout_box.style.border_bottom_width = len.value;
                    layout_box.style.border_left_width = len.value;
                }
            }
            "border-top-width" => {
                if let Some(len) = CssLength::parse(value) {
                    layout_box.style.border_top_width = len.value;
                }
            }
            "border-right-width" => {
                if let Some(len) = CssLength::parse(value) {
                    layout_box.style.border_right_width = len.value;
                }
            }
            "border-bottom-width" => {
                if let Some(len) = CssLength::parse(value) {
                    layout_box.style.border_bottom_width = len.value;
                }
            }
            "border-left-width" => {
                if let Some(len) = CssLength::parse(value) {
                    layout_box.style.border_left_width = len.value;
                }
            }
            "border-color" => {
                if let Some(color) = CssColor::parse(value) {
                    layout_box.style.border_color = Some(color);
                }
            }
            "border-radius" => {
                if let Some(len) = CssLength::parse(value) {
                    layout_box.style.border_radius = len.value;
                }
            }

            // Colors
            "color" => {
                if let Some(color) = CssColor::parse(value) {
                    layout_box.style.color = Some(color);
                }
            }
            "background-color" | "background" => {
                if let Some(color) = CssColor::parse(value) {
                    layout_box.style.background_color = Some(color);
                }
            }

            // Font
            "font-size" => {
                if let Some(len) = CssLength::parse(value) {
                    layout_box.style.font_size = len.to_px(
                        self.default_font_size,
                        self.viewport_width,
                        self.viewport_height,
                    );
                }
            }
            "font-weight" => {
                layout_box.style.font_weight = match value {
                    "bold" | "700" | "800" | "900" => FontWeight::Bold,
                    "normal" | "400" => FontWeight::Normal,
                    "lighter" | "100" | "200" | "300" => FontWeight::Light,
                    _ => FontWeight::Normal,
                };
            }
            "font-style" => {
                layout_box.style.font_style = match value {
                    "italic" | "oblique" => FontStyle::Italic,
                    _ => FontStyle::Normal,
                };
            }
            "text-decoration" => {
                layout_box.style.text_decoration = match value {
                    "underline" => TextDecoration::Underline,
                    "line-through" => TextDecoration::LineThrough,
                    "overline" => TextDecoration::Overline,
                    _ => TextDecoration::None,
                };
            }
            "text-align" => {
                layout_box.style.text_align = match value {
                    "left" => TextAlign::Left,
                    "right" => TextAlign::Right,
                    "center" => TextAlign::Center,
                    "justify" => TextAlign::Justify,
                    _ => TextAlign::Left,
                };
            }
            "line-height" => {
                if let Some(len) = CssLength::parse(value) {
                    layout_box.style.line_height = len.to_px(
                        layout_box.style.font_size,
                        self.viewport_width,
                        self.viewport_height,
                    );
                } else if let Ok(multiplier) = value.parse::<f32>() {
                    layout_box.style.line_height = layout_box.style.font_size * multiplier;
                }
            }

            // Positioning
            "position" => {
                layout_box.style.position = match value {
                    "relative" => Position::Relative,
                    "absolute" => Position::Absolute,
                    "fixed" => Position::Fixed,
                    "sticky" => Position::Sticky,
                    _ => Position::Static,
                };
            }
            "top" => {
                if let Some(len) = CssLength::parse(value) {
                    layout_box.style.top = Some(len);
                }
            }
            "right" => {
                if let Some(len) = CssLength::parse(value) {
                    layout_box.style.right = Some(len);
                }
            }
            "bottom" => {
                if let Some(len) = CssLength::parse(value) {
                    layout_box.style.bottom = Some(len);
                }
            }
            "left" => {
                if let Some(len) = CssLength::parse(value) {
                    layout_box.style.left = Some(len);
                }
            }
            "z-index" => {
                if let Ok(z) = value.parse::<i32>() {
                    layout_box.style.z_index = z;
                }
            }

            // Flexbox
            "display" if value == "flex" || value == "inline-flex" => {
                layout_box.layout_mode = if value == "flex" {
                    LayoutMode::Flex
                } else {
                    LayoutMode::InlineFlex
                };
            }
            "flex-direction" => {
                layout_box.style.flex_direction = match value {
                    "row" => FlexDirection::Row,
                    "row-reverse" => FlexDirection::RowReverse,
                    "column" => FlexDirection::Column,
                    "column-reverse" => FlexDirection::ColumnReverse,
                    _ => FlexDirection::Row,
                };
            }
            "flex-wrap" => {
                layout_box.style.flex_wrap = match value {
                    "wrap" => FlexWrap::Wrap,
                    "wrap-reverse" => FlexWrap::WrapReverse,
                    _ => FlexWrap::NoWrap,
                };
            }
            "justify-content" => {
                layout_box.style.justify_content = match value {
                    "flex-start" | "start" => JustifyContent::FlexStart,
                    "flex-end" | "end" => JustifyContent::FlexEnd,
                    "center" => JustifyContent::Center,
                    "space-between" => JustifyContent::SpaceBetween,
                    "space-around" => JustifyContent::SpaceAround,
                    "space-evenly" => JustifyContent::SpaceEvenly,
                    _ => JustifyContent::FlexStart,
                };
            }
            "align-items" => {
                layout_box.style.align_items = match value {
                    "flex-start" | "start" => AlignItems::FlexStart,
                    "flex-end" | "end" => AlignItems::FlexEnd,
                    "center" => AlignItems::Center,
                    "baseline" => AlignItems::Baseline,
                    "stretch" => AlignItems::Stretch,
                    _ => AlignItems::Stretch,
                };
            }
            "gap" => {
                if let Some(len) = CssLength::parse(value) {
                    layout_box.style.gap = len.to_px(
                        layout_box.style.font_size,
                        self.viewport_width,
                        self.viewport_height,
                    );
                }
            }
            "flex-grow" => {
                if let Ok(v) = value.parse::<f32>() {
                    layout_box.style.flex_grow = v;
                }
            }
            "flex-shrink" => {
                if let Ok(v) = value.parse::<f32>() {
                    layout_box.style.flex_shrink = v;
                }
            }

            // Overflow
            "overflow" => {
                let v = match value {
                    "hidden" => Overflow::Hidden,
                    "scroll" => Overflow::Scroll,
                    "auto" => Overflow::Auto,
                    _ => Overflow::Visible,
                };
                layout_box.style.overflow_x = v;
                layout_box.style.overflow_y = v;
            }
            "overflow-x" => {
                layout_box.style.overflow_x = match value {
                    "hidden" => Overflow::Hidden,
                    "scroll" => Overflow::Scroll,
                    "auto" => Overflow::Auto,
                    _ => Overflow::Visible,
                };
            }
            "overflow-y" => {
                layout_box.style.overflow_y = match value {
                    "hidden" => Overflow::Hidden,
                    "scroll" => Overflow::Scroll,
                    "auto" => Overflow::Auto,
                    _ => Overflow::Visible,
                };
            }

            // Visibility
            "visibility" => {
                layout_box.style.visible = value != "hidden";
            }
            "opacity" => {
                if let Ok(v) = value.parse::<f32>() {
                    layout_box.style.opacity = v.clamp(0.0, 1.0);
                }
            }

            _ => {}
        }
    }
}

/// Layout mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayoutMode {
    /// Block layout
    Block,
    /// Inline layout
    Inline,
    /// Inline-block layout
    InlineBlock,
    /// Flex layout
    Flex,
    /// Inline-flex layout
    InlineFlex,
    /// Grid layout
    Grid,
    /// Not displayed
    None,
}

/// Layout box
#[derive(Debug, Clone)]
pub struct LayoutBox {
    /// Layout mode
    pub layout_mode: LayoutMode,
    /// Computed dimensions
    pub dimensions: BoxDimensions,
    /// Style
    pub style: ComputedStyle,
    /// Children
    pub children: Vec<LayoutBox>,
    /// Node type (from DOM)
    pub node_type: Option<DomNodeType>,
    /// Tag name
    pub tag_name: Option<String>,
    /// Text content
    pub text_content: Option<String>,
}

impl LayoutBox {
    /// Create new layout box
    pub fn new(layout_mode: LayoutMode) -> Self {
        Self {
            layout_mode,
            dimensions: BoxDimensions::default(),
            style: ComputedStyle::default(),
            children: Vec::new(),
            node_type: None,
            tag_name: None,
            text_content: None,
        }
    }

    /// Layout this box and its children
    pub fn layout(&mut self, containing_width: f32, containing_height: f32, engine: &LayoutEngine) {
        match self.layout_mode {
            LayoutMode::Block => self.layout_block(containing_width, containing_height, engine),
            LayoutMode::Inline => self.layout_inline(containing_width, engine),
            LayoutMode::InlineBlock => self.layout_inline_block(containing_width, containing_height, engine),
            LayoutMode::Flex => self.layout_flex(containing_width, containing_height, engine),
            LayoutMode::InlineFlex => self.layout_flex(containing_width, containing_height, engine),
            LayoutMode::Grid => self.layout_block(containing_width, containing_height, engine),
            LayoutMode::None => {}
        }
    }

    fn layout_block(&mut self, containing_width: f32, containing_height: f32, engine: &LayoutEngine) {
        // Calculate width
        self.calculate_block_width(containing_width, engine);

        // Calculate position based on margin
        let margin_top = self.style.margin_top
            .map(|l| l.to_px(self.style.font_size, engine.viewport_width, engine.viewport_height))
            .unwrap_or(0.0);
        let margin_left = self.style.margin_left
            .map(|l| l.to_px(self.style.font_size, engine.viewport_width, engine.viewport_height))
            .unwrap_or(0.0);

        self.dimensions.content.x = margin_left + self.style.border_left_width +
            self.style.padding_left.map(|l| l.to_px(self.style.font_size, engine.viewport_width, engine.viewport_height)).unwrap_or(0.0);
        self.dimensions.content.y = margin_top + self.style.border_top_width +
            self.style.padding_top.map(|l| l.to_px(self.style.font_size, engine.viewport_width, engine.viewport_height)).unwrap_or(0.0);

        // Layout children
        self.layout_block_children(engine);

        // Calculate height
        self.calculate_block_height(containing_height, engine);
    }

    fn calculate_block_width(&mut self, containing_width: f32, engine: &LayoutEngine) {
        let font_size = self.style.font_size;
        let vw = engine.viewport_width;
        let vh = engine.viewport_height;

        // Get explicit width or default to auto
        let width = self.style.width
            .map(|l| l.to_px(font_size, vw, vh))
            .unwrap_or(0.0);

        let margin_left = self.style.margin_left
            .map(|l| l.to_px(font_size, vw, vh))
            .unwrap_or(0.0);
        let margin_right = self.style.margin_right
            .map(|l| l.to_px(font_size, vw, vh))
            .unwrap_or(0.0);

        let border_left = self.style.border_left_width;
        let border_right = self.style.border_right_width;

        let padding_left = self.style.padding_left
            .map(|l| l.to_px(font_size, vw, vh))
            .unwrap_or(0.0);
        let padding_right = self.style.padding_right
            .map(|l| l.to_px(font_size, vw, vh))
            .unwrap_or(0.0);

        let total = margin_left + border_left + padding_left + width + padding_right + border_right + margin_right;

        // If width is auto, use remaining space
        if self.style.width.is_none() || self.style.width.map(|l| l.unit == LengthUnit::Auto).unwrap_or(false) {
            self.dimensions.content.width = containing_width - margin_left - margin_right - border_left - border_right - padding_left - padding_right;
        } else {
            self.dimensions.content.width = width;
        }

        // Apply min/max
        if let Some(min) = self.style.min_width {
            let min_px = min.to_px(font_size, vw, vh);
            if self.dimensions.content.width < min_px {
                self.dimensions.content.width = min_px;
            }
        }
        if let Some(max) = self.style.max_width {
            let max_px = max.to_px(font_size, vw, vh);
            if self.dimensions.content.width > max_px {
                self.dimensions.content.width = max_px;
            }
        }

        self.dimensions.padding.left = padding_left;
        self.dimensions.padding.right = padding_right;
        self.dimensions.border.left = border_left;
        self.dimensions.border.right = border_right;
        self.dimensions.margin.left = margin_left;
        self.dimensions.margin.right = margin_right;
    }

    fn layout_block_children(&mut self, engine: &LayoutEngine) {
        let mut y = 0.0f32;

        for child in &mut self.children {
            child.layout(self.dimensions.content.width, engine.viewport_height, engine);
            child.dimensions.content.y += y;
            y = child.dimensions.margin_box().height + child.dimensions.content.y;
        }

        // Content height is the total height of children
        self.dimensions.content.height = y;
    }

    fn calculate_block_height(&mut self, containing_height: f32, engine: &LayoutEngine) {
        let font_size = self.style.font_size;
        let vw = engine.viewport_width;
        let vh = engine.viewport_height;

        // Explicit height overrides
        if let Some(height) = self.style.height {
            if height.unit != LengthUnit::Auto {
                self.dimensions.content.height = height.to_px(font_size, vw, vh);
            }
        }

        // Apply min/max
        if let Some(min) = self.style.min_height {
            let min_px = min.to_px(font_size, vw, vh);
            if self.dimensions.content.height < min_px {
                self.dimensions.content.height = min_px;
            }
        }
        if let Some(max) = self.style.max_height {
            let max_px = max.to_px(font_size, vw, vh);
            if self.dimensions.content.height > max_px {
                self.dimensions.content.height = max_px;
            }
        }

        // Set padding/border/margin for height
        self.dimensions.padding.top = self.style.padding_top
            .map(|l| l.to_px(font_size, vw, vh))
            .unwrap_or(0.0);
        self.dimensions.padding.bottom = self.style.padding_bottom
            .map(|l| l.to_px(font_size, vw, vh))
            .unwrap_or(0.0);
        self.dimensions.border.top = self.style.border_top_width;
        self.dimensions.border.bottom = self.style.border_bottom_width;
        self.dimensions.margin.top = self.style.margin_top
            .map(|l| l.to_px(font_size, vw, vh))
            .unwrap_or(0.0);
        self.dimensions.margin.bottom = self.style.margin_bottom
            .map(|l| l.to_px(font_size, vw, vh))
            .unwrap_or(0.0);
    }

    fn layout_inline(&mut self, containing_width: f32, engine: &LayoutEngine) {
        // For inline boxes, width and height are determined by content
        if let Some(text) = &self.text_content {
            // Simple text measurement (approximate)
            let char_width = self.style.font_size * 0.6;
            let line_height = self.style.line_height.max(self.style.font_size * 1.2);

            self.dimensions.content.width = (text.len() as f32 * char_width).min(containing_width);
            self.dimensions.content.height = line_height;
        }
    }

    fn layout_inline_block(&mut self, containing_width: f32, containing_height: f32, engine: &LayoutEngine) {
        self.layout_block(containing_width, containing_height, engine);
    }

    fn layout_flex(&mut self, containing_width: f32, containing_height: f32, engine: &LayoutEngine) {
        self.calculate_block_width(containing_width, engine);

        let font_size = self.style.font_size;
        let vw = engine.viewport_width;
        let vh = engine.viewport_height;

        // Calculate available space
        let padding_left = self.style.padding_left.map(|l| l.to_px(font_size, vw, vh)).unwrap_or(0.0);
        let padding_right = self.style.padding_right.map(|l| l.to_px(font_size, vw, vh)).unwrap_or(0.0);
        let padding_top = self.style.padding_top.map(|l| l.to_px(font_size, vw, vh)).unwrap_or(0.0);
        let padding_bottom = self.style.padding_bottom.map(|l| l.to_px(font_size, vw, vh)).unwrap_or(0.0);

        let available_width = self.dimensions.content.width - padding_left - padding_right;
        let gap = self.style.gap;

        // Layout children first to get their sizes
        for child in &mut self.children {
            child.layout(available_width, containing_height, engine);
        }

        // Calculate flex
        let is_row = matches!(self.style.flex_direction, FlexDirection::Row | FlexDirection::RowReverse);

        if is_row {
            self.layout_flex_row(available_width, gap);
        } else {
            self.layout_flex_column(containing_height, gap);
        }

        // Calculate height based on children
        let mut max_height = 0.0f32;
        for child in &self.children {
            let child_bottom = child.dimensions.content.y + child.dimensions.margin_box().height;
            max_height = max_height.max(child_bottom);
        }

        self.dimensions.content.height = max_height + padding_top + padding_bottom;
        self.calculate_block_height(containing_height, engine);
    }

    fn layout_flex_row(&mut self, available_width: f32, gap: f32) {
        let mut total_child_width = 0.0f32;
        let mut total_flex_grow = 0.0f32;

        for child in &self.children {
            total_child_width += child.dimensions.margin_box().width;
            total_flex_grow += child.style.flex_grow;
        }

        let total_gaps = if self.children.len() > 1 {
            gap * (self.children.len() - 1) as f32
        } else {
            0.0
        };

        let free_space = available_width - total_child_width - total_gaps;

        // Position children
        let mut x = 0.0f32;

        // Apply justify-content
        match self.style.justify_content {
            JustifyContent::Center => {
                x = free_space / 2.0;
            }
            JustifyContent::FlexEnd => {
                x = free_space;
            }
            JustifyContent::SpaceBetween if self.children.len() > 1 => {
                // Handled in loop
            }
            JustifyContent::SpaceAround if !self.children.is_empty() => {
                x = free_space / (self.children.len() as f32 * 2.0);
            }
            JustifyContent::SpaceEvenly if !self.children.is_empty() => {
                x = free_space / (self.children.len() as f32 + 1.0);
            }
            _ => {}
        }

        let space_between = if matches!(self.style.justify_content, JustifyContent::SpaceBetween) && self.children.len() > 1 {
            free_space / (self.children.len() - 1) as f32
        } else {
            0.0
        };

        for (i, child) in self.children.iter_mut().enumerate() {
            // Distribute extra space based on flex-grow
            let extra = if total_flex_grow > 0.0 && free_space > 0.0 {
                (child.style.flex_grow / total_flex_grow) * free_space
            } else {
                0.0
            };

            if extra > 0.0 {
                child.dimensions.content.width += extra;
            }

            child.dimensions.content.x = x;

            // Align items
            match self.style.align_items {
                AlignItems::Center => {
                    // Would need container height
                }
                AlignItems::FlexEnd => {
                    // Would need container height
                }
                _ => {}
            }

            x += child.dimensions.margin_box().width + gap;

            if matches!(self.style.justify_content, JustifyContent::SpaceBetween) {
                x += space_between;
            }
        }
    }

    fn layout_flex_column(&mut self, available_height: f32, gap: f32) {
        let mut y = 0.0f32;

        for child in &mut self.children {
            child.dimensions.content.y = y;
            y += child.dimensions.margin_box().height + gap;
        }
    }
}

/// Box dimensions
#[derive(Debug, Clone, Copy, Default)]
pub struct BoxDimensions {
    /// Content area
    pub content: Rect,
    /// Padding
    pub padding: EdgeSizes,
    /// Border
    pub border: EdgeSizes,
    /// Margin
    pub margin: EdgeSizes,
}

impl BoxDimensions {
    /// Get padding box
    pub fn padding_box(&self) -> Rect {
        self.content.expanded_by(self.padding)
    }

    /// Get border box
    pub fn border_box(&self) -> Rect {
        self.padding_box().expanded_by(self.border)
    }

    /// Get margin box
    pub fn margin_box(&self) -> Rect {
        self.border_box().expanded_by(self.margin)
    }
}

/// Rectangle
#[derive(Debug, Clone, Copy, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    /// Expand by edge sizes
    pub fn expanded_by(&self, edge: EdgeSizes) -> Rect {
        Rect {
            x: self.x - edge.left,
            y: self.y - edge.top,
            width: self.width + edge.left + edge.right,
            height: self.height + edge.top + edge.bottom,
        }
    }
}

/// Edge sizes (for padding, border, margin)
#[derive(Debug, Clone, Copy, Default)]
pub struct EdgeSizes {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

/// Computed style
#[derive(Debug, Clone)]
pub struct ComputedStyle {
    // Dimensions
    pub width: Option<CssLength>,
    pub height: Option<CssLength>,
    pub min_width: Option<CssLength>,
    pub min_height: Option<CssLength>,
    pub max_width: Option<CssLength>,
    pub max_height: Option<CssLength>,

    // Margin
    pub margin_top: Option<CssLength>,
    pub margin_right: Option<CssLength>,
    pub margin_bottom: Option<CssLength>,
    pub margin_left: Option<CssLength>,

    // Padding
    pub padding_top: Option<CssLength>,
    pub padding_right: Option<CssLength>,
    pub padding_bottom: Option<CssLength>,
    pub padding_left: Option<CssLength>,

    // Border
    pub border_top_width: f32,
    pub border_right_width: f32,
    pub border_bottom_width: f32,
    pub border_left_width: f32,
    pub border_color: Option<CssColor>,
    pub border_radius: f32,

    // Colors
    pub color: Option<CssColor>,
    pub background_color: Option<CssColor>,

    // Font
    pub font_size: f32,
    pub font_weight: FontWeight,
    pub font_style: FontStyle,
    pub text_decoration: TextDecoration,
    pub text_align: TextAlign,
    pub line_height: f32,

    // Position
    pub position: Position,
    pub top: Option<CssLength>,
    pub right: Option<CssLength>,
    pub bottom: Option<CssLength>,
    pub left: Option<CssLength>,
    pub z_index: i32,

    // Flexbox
    pub flex_direction: FlexDirection,
    pub flex_wrap: FlexWrap,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    pub gap: f32,
    pub flex_grow: f32,
    pub flex_shrink: f32,

    // Overflow
    pub overflow_x: Overflow,
    pub overflow_y: Overflow,

    // Visibility
    pub visible: bool,
    pub opacity: f32,
}

impl Default for ComputedStyle {
    fn default() -> Self {
        Self {
            width: None,
            height: None,
            min_width: None,
            min_height: None,
            max_width: None,
            max_height: None,
            margin_top: None,
            margin_right: None,
            margin_bottom: None,
            margin_left: None,
            padding_top: None,
            padding_right: None,
            padding_bottom: None,
            padding_left: None,
            border_top_width: 0.0,
            border_right_width: 0.0,
            border_bottom_width: 0.0,
            border_left_width: 0.0,
            border_color: None,
            border_radius: 0.0,
            color: Some(CssColor { r: 0, g: 0, b: 0, a: 255 }),
            background_color: None,
            font_size: 16.0,
            font_weight: FontWeight::Normal,
            font_style: FontStyle::Normal,
            text_decoration: TextDecoration::None,
            text_align: TextAlign::Left,
            line_height: 20.0,
            position: Position::Static,
            top: None,
            right: None,
            bottom: None,
            left: None,
            z_index: 0,
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::NoWrap,
            justify_content: JustifyContent::FlexStart,
            align_items: AlignItems::Stretch,
            gap: 0.0,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            overflow_x: Overflow::Visible,
            overflow_y: Overflow::Visible,
            visible: true,
            opacity: 1.0,
        }
    }
}

// Style enums

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FontWeight {
    Light,
    Normal,
    Bold,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FontStyle {
    Normal,
    Italic,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextDecoration {
    None,
    Underline,
    Overline,
    LineThrough,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextAlign {
    Left,
    Right,
    Center,
    Justify,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Position {
    Static,
    Relative,
    Absolute,
    Fixed,
    Sticky,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FlexDirection {
    Row,
    RowReverse,
    Column,
    ColumnReverse,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FlexWrap {
    NoWrap,
    Wrap,
    WrapReverse,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JustifyContent {
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AlignItems {
    FlexStart,
    FlexEnd,
    Center,
    Baseline,
    Stretch,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Overflow {
    Visible,
    Hidden,
    Scroll,
    Auto,
}
