//! Tree View Widget
//!
//! A hierarchical tree view widget for displaying nested data structures.

use alloc::string::String;
use alloc::vec::Vec;
use super::{Widget, WidgetId, WidgetState, Bounds, WidgetEvent, MouseButton, theme};
use crate::gui::surface::Surface;
use crate::drivers::framebuffer::Color;

/// Unique identifier for tree nodes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TreeNodeId(pub u64);

static NEXT_NODE_ID: spin::Mutex<u64> = spin::Mutex::new(1);

impl TreeNodeId {
    pub fn new() -> Self {
        let mut next = NEXT_NODE_ID.lock();
        let id = *next;
        *next += 1;
        TreeNodeId(id)
    }
}

impl Default for TreeNodeId {
    fn default() -> Self {
        Self::new()
    }
}

/// Icon type for tree nodes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeNodeIcon {
    None,
    Folder,
    FolderOpen,
    File,
    Document,
    Image,
    Audio,
    Video,
    Archive,
    Code,
    Database,
    Settings,
    User,
    Computer,
    Network,
    Custom(u32),
}

/// A single node in the tree
#[derive(Debug, Clone)]
pub struct TreeNode {
    pub id: TreeNodeId,
    pub text: String,
    pub icon: TreeNodeIcon,
    pub children: Vec<TreeNode>,
    pub expanded: bool,
    pub selectable: bool,
    pub user_data: u64,
}

impl TreeNode {
    pub fn new(text: &str) -> Self {
        Self {
            id: TreeNodeId::new(),
            text: String::from(text),
            icon: TreeNodeIcon::None,
            children: Vec::new(),
            expanded: false,
            selectable: true,
            user_data: 0,
        }
    }

    pub fn with_icon(text: &str, icon: TreeNodeIcon) -> Self {
        Self {
            id: TreeNodeId::new(),
            text: String::from(text),
            icon,
            children: Vec::new(),
            expanded: false,
            selectable: true,
            user_data: 0,
        }
    }

    pub fn add_child(&mut self, child: TreeNode) {
        self.children.push(child);
    }

    pub fn set_expanded(&mut self, expanded: bool) {
        self.expanded = expanded;
    }

    pub fn has_children(&self) -> bool {
        !self.children.is_empty()
    }

    pub fn set_user_data(&mut self, data: u64) {
        self.user_data = data;
    }

    pub fn find_by_id(&self, id: TreeNodeId) -> Option<&TreeNode> {
        if self.id == id {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.find_by_id(id) {
                return Some(found);
            }
        }
        None
    }

    pub fn find_by_id_mut(&mut self, id: TreeNodeId) -> Option<&mut TreeNode> {
        if self.id == id {
            return Some(self);
        }
        for child in &mut self.children {
            if let Some(found) = child.find_by_id_mut(id) {
                return Some(found);
            }
        }
        None
    }
}

/// Tree view style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeViewStyle {
    Classic,
    Modern,
    Compact,
}

/// Selection mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMode {
    None,
    Single,
    Multiple,
}

/// Flat node representation for rendering
#[derive(Debug, Clone)]
struct FlatNode {
    id: TreeNodeId,
    depth: usize,
    text: String,
    icon: TreeNodeIcon,
    has_children: bool,
    expanded: bool,
    is_last_child: bool,
    parent_last_flags: Vec<bool>,
}

/// Tree View widget
pub struct TreeView {
    id: WidgetId,
    bounds: Bounds,
    roots: Vec<TreeNode>,
    selected: Vec<TreeNodeId>,
    focused_index: Option<usize>,
    scroll_offset: usize,
    style: TreeViewStyle,
    selection_mode: SelectionMode,
    item_height: usize,
    indent_width: usize,
    enabled: bool,
    visible: bool,
    state: WidgetState,
    show_root_lines: bool,
    show_icons: bool,
    on_select: Option<fn(TreeNodeId)>,
    on_expand: Option<fn(TreeNodeId, bool)>,
    on_double_click: Option<fn(TreeNodeId)>,
    flat_nodes: Vec<FlatNode>,
    flat_dirty: bool,
}

impl TreeView {
    pub fn new(x: isize, y: isize, width: usize, height: usize) -> Self {
        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(x, y, width, height),
            roots: Vec::new(),
            selected: Vec::new(),
            focused_index: None,
            scroll_offset: 0,
            style: TreeViewStyle::Modern,
            selection_mode: SelectionMode::Single,
            item_height: 22,
            indent_width: 20,
            enabled: true,
            visible: true,
            state: WidgetState::Normal,
            show_root_lines: true,
            show_icons: true,
            on_select: None,
            on_expand: None,
            on_double_click: None,
            flat_nodes: Vec::new(),
            flat_dirty: true,
        }
    }

    pub fn set_style(&mut self, style: TreeViewStyle) {
        self.style = style;
    }

    pub fn set_selection_mode(&mut self, mode: SelectionMode) {
        self.selection_mode = mode;
        if mode == SelectionMode::None {
            self.selected.clear();
        } else if mode == SelectionMode::Single && self.selected.len() > 1 {
            self.selected.truncate(1);
        }
    }

    pub fn set_item_height(&mut self, height: usize) {
        self.item_height = height;
    }

    pub fn set_indent_width(&mut self, width: usize) {
        self.indent_width = width;
    }

    pub fn set_show_root_lines(&mut self, show: bool) {
        self.show_root_lines = show;
    }

    pub fn set_show_icons(&mut self, show: bool) {
        self.show_icons = show;
    }

    pub fn add_root(&mut self, node: TreeNode) {
        self.roots.push(node);
        self.flat_dirty = true;
    }

    pub fn clear(&mut self) {
        self.roots.clear();
        self.selected.clear();
        self.focused_index = None;
        self.scroll_offset = 0;
        self.flat_dirty = true;
    }

    pub fn selected(&self) -> &[TreeNodeId] {
        &self.selected
    }

    pub fn select(&mut self, id: TreeNodeId) {
        match self.selection_mode {
            SelectionMode::None => {}
            SelectionMode::Single => {
                self.selected.clear();
                self.selected.push(id);
            }
            SelectionMode::Multiple => {
                if !self.selected.contains(&id) {
                    self.selected.push(id);
                }
            }
        }
        if let Some(callback) = self.on_select {
            callback(id);
        }
    }

    pub fn deselect(&mut self, id: TreeNodeId) {
        self.selected.retain(|&x| x != id);
    }

    pub fn clear_selection(&mut self) {
        self.selected.clear();
    }

    pub fn toggle_select(&mut self, id: TreeNodeId) {
        if self.selected.contains(&id) {
            self.deselect(id);
        } else {
            self.select(id);
        }
    }

    pub fn toggle_expand(&mut self, id: TreeNodeId) {
        let currently_expanded = self.is_expanded(id);
        for root in &mut self.roots {
            if let Some(found) = root.find_by_id_mut(id) {
                found.expanded = !currently_expanded;
                break;
            }
        }
        self.flat_dirty = true;
        if let Some(callback) = self.on_expand {
            callback(id, !currently_expanded);
        }
    }

    fn is_expanded(&self, id: TreeNodeId) -> bool {
        for root in &self.roots {
            if let Some(node) = root.find_by_id(id) {
                return node.expanded;
            }
        }
        false
    }

    pub fn expand_all(&mut self) {
        Self::expand_all_recursive(&mut self.roots);
        self.flat_dirty = true;
    }

    fn expand_all_recursive(nodes: &mut [TreeNode]) {
        for node in nodes {
            node.expanded = true;
            Self::expand_all_recursive(&mut node.children);
        }
    }

    pub fn collapse_all(&mut self) {
        Self::collapse_all_recursive(&mut self.roots);
        self.flat_dirty = true;
    }

    fn collapse_all_recursive(nodes: &mut [TreeNode]) {
        for node in nodes {
            node.expanded = false;
            Self::collapse_all_recursive(&mut node.children);
        }
    }

    pub fn find_node(&self, id: TreeNodeId) -> Option<&TreeNode> {
        for root in &self.roots {
            if let Some(found) = root.find_by_id(id) {
                return Some(found);
            }
        }
        None
    }

    pub fn find_node_mut(&mut self, id: TreeNodeId) -> Option<&mut TreeNode> {
        for root in &mut self.roots {
            if let Some(found) = root.find_by_id_mut(id) {
                return Some(found);
            }
        }
        None
    }

    pub fn set_on_select(&mut self, callback: fn(TreeNodeId)) {
        self.on_select = Some(callback);
    }

    pub fn set_on_expand(&mut self, callback: fn(TreeNodeId, bool)) {
        self.on_expand = Some(callback);
    }

    pub fn set_on_double_click(&mut self, callback: fn(TreeNodeId)) {
        self.on_double_click = Some(callback);
    }

    fn build_flat_nodes(&mut self) {
        self.flat_nodes.clear();
        let parent_flags: Vec<bool> = Vec::new();
        let roots_len = self.roots.len();
        // Build flat list by collecting from roots (avoids borrow conflict)
        let mut flat_list = Vec::new();
        for (i, root) in self.roots.iter().enumerate() {
            let is_last = i == roots_len - 1;
            Self::flatten_node_into(&mut flat_list, root, 0, is_last, parent_flags.clone());
        }
        self.flat_nodes = flat_list;
        self.flat_dirty = false;
    }

    fn flatten_node_into(flat_list: &mut Vec<FlatNode>, node: &TreeNode, depth: usize, is_last: bool, parent_flags: Vec<bool>) {
        let mut flags = parent_flags.clone();
        if depth > 0 {
            flags.push(is_last);
        }

        flat_list.push(FlatNode {
            id: node.id,
            depth,
            text: node.text.clone(),
            icon: node.icon,
            has_children: node.has_children(),
            expanded: node.expanded,
            is_last_child: is_last,
            parent_last_flags: flags.clone(),
        });

        if node.expanded {
            for (i, child) in node.children.iter().enumerate() {
                let child_is_last = i == node.children.len() - 1;
                Self::flatten_node_into(flat_list, child, depth + 1, child_is_last, flags.clone());
            }
        }
    }

    fn visible_count(&self) -> usize {
        self.flat_nodes.len()
    }

    fn items_in_view(&self) -> usize {
        if self.item_height == 0 {
            return 0;
        }
        self.bounds.height / self.item_height
    }

    fn ensure_visible(&mut self) {
        if let Some(idx) = self.focused_index {
            let items_in_view = self.items_in_view();
            if idx < self.scroll_offset {
                self.scroll_offset = idx;
            } else if idx >= self.scroll_offset + items_in_view {
                self.scroll_offset = idx.saturating_sub(items_in_view - 1);
            }
        }
    }

    fn node_at_y(&self, y: isize) -> Option<usize> {
        let rel_y = y - self.bounds.y;
        if rel_y < 0 {
            return None;
        }
        let idx = self.scroll_offset + (rel_y as usize) / self.item_height;
        if idx < self.flat_nodes.len() {
            Some(idx)
        } else {
            None
        }
    }

    fn is_in_expand_area(&self, x: isize, depth: usize) -> bool {
        let rel_x = x - self.bounds.x;
        if rel_x < 0 {
            return false;
        }
        let expand_start = (depth * self.indent_width) as isize;
        let expand_end = expand_start + 16;
        rel_x >= expand_start && rel_x < expand_end
    }
}

// Helper drawing functions
fn draw_line(surface: &mut Surface, x1: isize, y1: isize, x2: isize, y2: isize, color: Color) {
    // Simple horizontal or vertical line drawing
    if y1 == y2 {
        // Horizontal line
        let (start_x, end_x) = if x1 < x2 { (x1, x2) } else { (x2, x1) };
        let y = y1;
        if y < 0 {
            return;
        }
        for x in start_x.max(0)..=end_x.max(0) {
            surface.set_pixel(x as usize, y as usize, color);
        }
    } else if x1 == x2 {
        // Vertical line
        let (start_y, end_y) = if y1 < y2 { (y1, y2) } else { (y2, y1) };
        let x = x1;
        if x < 0 {
            return;
        }
        for y in start_y.max(0)..=end_y.max(0) {
            surface.set_pixel(x as usize, y as usize, color);
        }
    }
    // Note: diagonal lines would need Bresenham's algorithm
}

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
    if x < 0 || y < 0 {
        return;
    }
    surface.fill_rect(x as usize, y as usize, width, height, color);
}

fn draw_rect_safe(surface: &mut Surface, x: isize, y: isize, width: usize, height: usize, color: Color) {
    if x < 0 || y < 0 {
        return;
    }
    surface.draw_rect(x as usize, y as usize, width, height, color);
}

impl Widget for TreeView {
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
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.state = WidgetState::Disabled;
        } else {
            self.state = WidgetState::Normal;
        }
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

        if self.flat_dirty {
            self.build_flat_nodes();
        }

        match event {
            WidgetEvent::MouseDown { button: MouseButton::Left, x, y } => {
                if self.bounds.contains(*x, *y) {
                    self.state = WidgetState::Pressed;
                    if let Some(idx) = self.node_at_y(*y) {
                        let node = &self.flat_nodes[idx];
                        let depth = node.depth;
                        let node_id = node.id;
                        let has_children = node.has_children;

                        if has_children && self.is_in_expand_area(*x, depth) {
                            self.toggle_expand(node_id);
                            self.build_flat_nodes();
                        } else {
                            self.focused_index = Some(idx);
                            match self.selection_mode {
                                SelectionMode::None => {}
                                SelectionMode::Single => {
                                    self.selected.clear();
                                    self.selected.push(node_id);
                                    if let Some(callback) = self.on_select {
                                        callback(node_id);
                                    }
                                }
                                SelectionMode::Multiple => {
                                    if !self.selected.contains(&node_id) {
                                        self.selected.push(node_id);
                                    }
                                    if let Some(callback) = self.on_select {
                                        callback(node_id);
                                    }
                                }
                            }
                        }
                    }
                    return true;
                }
            }
            WidgetEvent::MouseUp { button: MouseButton::Left, .. } => {
                if self.state == WidgetState::Pressed {
                    self.state = WidgetState::Focused;
                    return true;
                }
            }
            WidgetEvent::DoubleClick { button: MouseButton::Left, x, y } => {
                if self.bounds.contains(*x, *y) {
                    if let Some(idx) = self.node_at_y(*y) {
                        let node_id = self.flat_nodes[idx].id;
                        let has_children = self.flat_nodes[idx].has_children;

                        if has_children {
                            self.toggle_expand(node_id);
                            self.build_flat_nodes();
                        }

                        if let Some(callback) = self.on_double_click {
                            callback(node_id);
                        }
                    }
                    return true;
                }
            }
            WidgetEvent::Scroll { delta_y, .. } => {
                let max_scroll = self.visible_count().saturating_sub(self.items_in_view());
                if *delta_y < 0 {
                    self.scroll_offset = self.scroll_offset.saturating_add(3).min(max_scroll);
                } else if *delta_y > 0 {
                    self.scroll_offset = self.scroll_offset.saturating_sub(3);
                }
                return true;
            }
            WidgetEvent::KeyDown { key, modifiers } => {
                if self.state == WidgetState::Focused || self.state == WidgetState::Pressed {
                    let ctrl = (*modifiers & super::modifiers::CTRL) != 0;
                    match *key {
                        0x48 => { // Up arrow
                            if let Some(idx) = self.focused_index {
                                if idx > 0 {
                                    self.focused_index = Some(idx - 1);
                                    if self.selection_mode == SelectionMode::Single && !ctrl {
                                        self.selected.clear();
                                        self.selected.push(self.flat_nodes[idx - 1].id);
                                        if let Some(callback) = self.on_select {
                                            callback(self.flat_nodes[idx - 1].id);
                                        }
                                    }
                                    self.ensure_visible();
                                }
                            } else if !self.flat_nodes.is_empty() {
                                self.focused_index = Some(0);
                            }
                            return true;
                        }
                        0x50 => { // Down arrow
                            if let Some(idx) = self.focused_index {
                                if idx < self.flat_nodes.len() - 1 {
                                    self.focused_index = Some(idx + 1);
                                    if self.selection_mode == SelectionMode::Single && !ctrl {
                                        self.selected.clear();
                                        self.selected.push(self.flat_nodes[idx + 1].id);
                                        if let Some(callback) = self.on_select {
                                            callback(self.flat_nodes[idx + 1].id);
                                        }
                                    }
                                    self.ensure_visible();
                                }
                            } else if !self.flat_nodes.is_empty() {
                                self.focused_index = Some(0);
                            }
                            return true;
                        }
                        0x4B => { // Left arrow - collapse
                            if let Some(idx) = self.focused_index {
                                let node_id = self.flat_nodes[idx].id;
                                let expanded = self.flat_nodes[idx].expanded;
                                let has_children = self.flat_nodes[idx].has_children;

                                if has_children && expanded {
                                    self.toggle_expand(node_id);
                                    self.build_flat_nodes();
                                }
                            }
                            return true;
                        }
                        0x4D => { // Right arrow - expand
                            if let Some(idx) = self.focused_index {
                                let node_id = self.flat_nodes[idx].id;
                                let expanded = self.flat_nodes[idx].expanded;
                                let has_children = self.flat_nodes[idx].has_children;

                                if has_children && !expanded {
                                    self.toggle_expand(node_id);
                                    self.build_flat_nodes();
                                }
                            }
                            return true;
                        }
                        0x1C => { // Enter
                            if let Some(idx) = self.focused_index {
                                let node_id = self.flat_nodes[idx].id;
                                let has_children = self.flat_nodes[idx].has_children;

                                if has_children {
                                    self.toggle_expand(node_id);
                                    self.build_flat_nodes();
                                }

                                if let Some(callback) = self.on_double_click {
                                    callback(node_id);
                                }
                            }
                            return true;
                        }
                        0x39 => { // Space
                            if let Some(idx) = self.focused_index {
                                let node_id = self.flat_nodes[idx].id;

                                match self.selection_mode {
                                    SelectionMode::None => {}
                                    SelectionMode::Single => {
                                        self.selected.clear();
                                        self.selected.push(node_id);
                                        if let Some(callback) = self.on_select {
                                            callback(node_id);
                                        }
                                    }
                                    SelectionMode::Multiple => {
                                        self.toggle_select(node_id);
                                    }
                                }
                            }
                            return true;
                        }
                        0x47 => { // Home
                            if !self.flat_nodes.is_empty() {
                                self.focused_index = Some(0);
                                self.scroll_offset = 0;
                                if self.selection_mode == SelectionMode::Single {
                                    self.selected.clear();
                                    self.selected.push(self.flat_nodes[0].id);
                                    if let Some(callback) = self.on_select {
                                        callback(self.flat_nodes[0].id);
                                    }
                                }
                            }
                            return true;
                        }
                        0x4F => { // End
                            if !self.flat_nodes.is_empty() {
                                let last = self.flat_nodes.len() - 1;
                                self.focused_index = Some(last);
                                self.ensure_visible();
                                if self.selection_mode == SelectionMode::Single {
                                    self.selected.clear();
                                    self.selected.push(self.flat_nodes[last].id);
                                    if let Some(callback) = self.on_select {
                                        callback(self.flat_nodes[last].id);
                                    }
                                }
                            }
                            return true;
                        }
                        _ => {}
                    }
                }
            }
            WidgetEvent::Focus => {
                self.state = WidgetState::Focused;
                return true;
            }
            WidgetEvent::Blur => {
                self.state = WidgetState::Normal;
                return true;
            }
            _ => {}
        }
        false
    }

    fn render(&self, surface: &mut Surface) {
        if !self.visible {
            return;
        }

        let theme = theme();
        let bg = if self.enabled { theme.bg } else { theme.bg_disabled };
        let fg = if self.enabled { theme.fg } else { theme.fg_disabled };
        let border = if self.state == WidgetState::Focused { theme.border_focused } else { theme.border };

        // Background
        fill_rect_safe(surface, self.bounds.x, self.bounds.y, self.bounds.width, self.bounds.height, bg);

        // Border
        draw_rect_safe(surface, self.bounds.x, self.bounds.y, self.bounds.width, self.bounds.height, border);

        let flat_nodes = &self.flat_nodes;
        let items_in_view = self.items_in_view();
        let line_color = Color::new(80, 80, 90);
        let select_color = theme.accent;
        let hover_color = theme.bg_hover;

        for (view_idx, idx) in (self.scroll_offset..).take(items_in_view).enumerate() {
            if idx >= flat_nodes.len() {
                break;
            }

            let node = &flat_nodes[idx];
            let y = self.bounds.y + (view_idx * self.item_height) as isize;
            let base_x = self.bounds.x + 4;
            let indent_x = base_x + (node.depth * self.indent_width) as isize;

            let is_selected = self.selected.contains(&node.id);
            let is_focused = self.focused_index == Some(idx);

            // Selection/focus background
            if is_selected {
                fill_rect_safe(surface, self.bounds.x + 1, y, self.bounds.width - 2, self.item_height, select_color);
            } else if is_focused && self.state == WidgetState::Focused {
                fill_rect_safe(surface, self.bounds.x + 1, y, self.bounds.width - 2, self.item_height, hover_color);
            }

            // Draw tree lines (Classic style only)
            if self.style == TreeViewStyle::Classic && node.depth > 0 {
                for d in 0..node.depth {
                    let line_x = base_x + (d * self.indent_width) as isize + 8;

                    let is_parent_last = if d < node.parent_last_flags.len() {
                        node.parent_last_flags[d]
                    } else {
                        false
                    };

                    if d == node.depth - 1 {
                        let mid_y = y + (self.item_height / 2) as isize;
                        draw_line(surface, line_x, mid_y, indent_x, mid_y, line_color);

                        if node.is_last_child {
                            draw_line(surface, line_x, y, line_x, mid_y, line_color);
                        } else {
                            draw_line(surface, line_x, y, line_x, y + self.item_height as isize, line_color);
                        }
                    } else if !is_parent_last {
                        draw_line(surface, line_x, y, line_x, y + self.item_height as isize, line_color);
                    }
                }
            }

            // Draw expand/collapse indicator
            if node.has_children {
                let expand_x = indent_x + 2;
                let expand_y = y + (self.item_height / 2) as isize - 4;
                let expand_color = if self.enabled { fg } else { theme.fg_disabled };

                match self.style {
                    TreeViewStyle::Classic => {
                        // Box with +/-
                        draw_rect_safe(surface, expand_x, expand_y, 9, 9, expand_color);
                        fill_rect_safe(surface, expand_x + 1, expand_y + 1, 7, 7, bg);

                        // Horizontal line
                        draw_line(surface, expand_x + 2, expand_y + 4, expand_x + 6, expand_y + 4, expand_color);

                        // Vertical line (only if collapsed)
                        if !node.expanded {
                            draw_line(surface, expand_x + 4, expand_y + 2, expand_x + 4, expand_y + 6, expand_color);
                        }
                    }
                    TreeViewStyle::Modern | TreeViewStyle::Compact => {
                        // Triangle
                        if node.expanded {
                            // Down arrow
                            for i in 0..4 {
                                draw_line(
                                    surface,
                                    expand_x + 1 + i,
                                    expand_y + 2 + i,
                                    expand_x + 7 - i,
                                    expand_y + 2 + i,
                                    expand_color
                                );
                            }
                        } else {
                            // Right arrow
                            for i in 0..4 {
                                draw_line(
                                    surface,
                                    expand_x + 2 + i,
                                    expand_y + 1 + i,
                                    expand_x + 2 + i,
                                    expand_y + 7 - i,
                                    expand_color
                                );
                            }
                        }
                    }
                }
            }

            // Calculate text position
            let text_x = indent_x + if node.has_children { 14 } else { 4 };
            let text_y = y + (self.item_height / 2) as isize - 6;

            // Draw icon
            let mut label_x = text_x;
            if self.show_icons && node.icon != TreeNodeIcon::None {
                let icon_color = if is_selected { Color::WHITE } else { fg };
                draw_icon(surface, node.icon, text_x, text_y - 1, icon_color);
                label_x += 18;
            }

            // Draw text
            let text_color = if is_selected { Color::WHITE } else { fg };
            draw_string(surface, label_x, text_y, &node.text, text_color);
        }

        // Draw scrollbar if needed
        let total = flat_nodes.len();
        if total > items_in_view {
            let scrollbar_height = self.bounds.height - 4;
            let thumb_height = (scrollbar_height * items_in_view / total).max(20);
            let thumb_pos = if total > items_in_view {
                (scrollbar_height - thumb_height) * self.scroll_offset / (total - items_in_view)
            } else {
                0
            };

            let sb_x = self.bounds.x + self.bounds.width as isize - 10;
            let sb_y = self.bounds.y + 2;

            // Track
            fill_rect_safe(surface, sb_x, sb_y, 8, scrollbar_height, Color::new(50, 50, 58));

            // Thumb
            fill_rect_safe(surface, sb_x, sb_y + thumb_pos as isize, 8, thumb_height, Color::new(100, 100, 110));
        }
    }
}

fn draw_icon(surface: &mut Surface, icon: TreeNodeIcon, x: isize, y: isize, color: Color) {
    if x < 0 || y < 0 {
        return;
    }
    let x = x as usize;
    let y = y as usize;
    let size = 14;

    match icon {
        TreeNodeIcon::None => {}
        TreeNodeIcon::Folder => {
            surface.fill_rect(x, y + 4, size, size - 4, color);
            surface.fill_rect(x, y + 2, 6, 2, color);
        }
        TreeNodeIcon::FolderOpen => {
            surface.fill_rect(x, y + 4, size, size - 4, color);
            surface.fill_rect(x, y + 2, 6, 2, color);
            surface.fill_rect(x + 2, y + 6, size - 2, 2, Color::new(255, 255, 200));
        }
        TreeNodeIcon::File => {
            surface.fill_rect(x + 2, y, size - 4, size, color);
            surface.fill_rect(x + size - 4, y, 2, 4, Color::new(128, 128, 128));
        }
        TreeNodeIcon::Document => {
            surface.fill_rect(x + 2, y, size - 4, size, color);
            for i in 0..3 {
                let line_y = y + 3 + i * 3;
                for px in (x + 4)..(x + size - 4) {
                    surface.set_pixel(px, line_y, Color::new(100, 100, 100));
                }
            }
        }
        TreeNodeIcon::Image => {
            surface.fill_rect(x + 1, y + 1, size - 2, size - 2, Color::new(100, 150, 200));
            surface.fill_rect(x + 3, y + 5, 4, 4, Color::new(255, 200, 100));
            surface.fill_rect(x + 2, y + 9, size - 4, 3, Color::new(100, 180, 100));
        }
        TreeNodeIcon::Audio => {
            surface.fill_rect(x + 4, y + 2, 2, 8, color);
            surface.fill_rect(x + 2, y + 8, 4, 4, color);
            surface.fill_rect(x + 9, y + 2, 2, 6, color);
            surface.fill_rect(x + 7, y + 6, 4, 4, color);
        }
        TreeNodeIcon::Video => {
            surface.fill_rect(x + 2, y + 2, size - 4, size - 4, color);
            for i in 0..3 {
                surface.fill_rect(x, y + 2 + i * 4, 2, 2, color);
                surface.fill_rect(x + size - 2, y + 2 + i * 4, 2, 2, color);
            }
        }
        TreeNodeIcon::Archive => {
            surface.fill_rect(x + 2, y, size - 4, size, Color::new(180, 140, 80));
            for i in 0..4 {
                let stripe_y = y + 2 + i * 3;
                surface.fill_rect(x + 5, stripe_y, 4, 2, Color::new(120, 80, 40));
            }
        }
        TreeNodeIcon::Code => {
            // Simplified brackets
            for i in 0..3 {
                surface.set_pixel(x + 3 + i, y + 2 + i, color);
                surface.set_pixel(x + 3 + i, y + size - 2 - i, color);
                surface.set_pixel(x + size - 3 - i, y + 2 + i, color);
                surface.set_pixel(x + size - 3 - i, y + size - 2 - i, color);
            }
        }
        TreeNodeIcon::Database => {
            surface.fill_rect(x + 3, y + 2, size - 6, size - 4, color);
            surface.fill_rect(x + 2, y + 1, size - 4, 3, color);
            surface.fill_rect(x + 2, y + size - 4, size - 4, 3, color);
        }
        TreeNodeIcon::Settings => {
            surface.fill_rect(x + 5, y + 2, 4, size - 4, color);
            surface.fill_rect(x + 2, y + 5, size - 4, 4, color);
            surface.fill_rect(x + 5, y + 5, 4, 4, Color::new(40, 40, 50));
        }
        TreeNodeIcon::User => {
            surface.fill_rect(x + 5, y + 2, 4, 4, color);
            surface.fill_rect(x + 3, y + 7, 8, 6, color);
        }
        TreeNodeIcon::Computer => {
            surface.fill_rect(x + 1, y + 2, size - 2, size - 6, color);
            surface.fill_rect(x + 5, y + size - 4, 4, 2, color);
            surface.fill_rect(x + 3, y + size - 2, 8, 2, color);
        }
        TreeNodeIcon::Network => {
            surface.fill_rect(x + 5, y + 2, 4, 4, color);
            surface.fill_rect(x + 1, y + 9, 4, 4, color);
            surface.fill_rect(x + 9, y + 9, 4, 4, color);
        }
        TreeNodeIcon::Custom(_) => {
            surface.draw_rect(x, y, size, size, color);
        }
    }
}
