//! Terminal Tabs
//!
//! Multi-tab terminal emulator with session management, split views,
//! keyboard shortcuts, and advanced tab features.

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use core::sync::atomic::{AtomicU64, Ordering};
use crate::drivers::framebuffer::Color;
use crate::drivers::font::DEFAULT_FONT;
use crate::gui::surface::Surface;
use crate::gui::widgets::{Widget, WidgetId, WidgetEvent, Bounds, MouseButton};
use super::terminal::{Terminal, CellAttrs, Cell};

/// Generate unique tab IDs
static TAB_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Unique identifier for a terminal tab
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TabId(u64);

impl TabId {
    /// Generate a new unique tab ID
    pub fn new() -> Self {
        Self(TAB_ID_COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    /// Get the raw ID value
    pub fn value(&self) -> u64 {
        self.0
    }
}

impl Default for TabId {
    fn default() -> Self {
        Self::new()
    }
}

/// State of a terminal tab
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabState {
    /// Tab is active and receiving input
    Active,
    /// Tab is running in background
    Background,
    /// Tab has activity (bell, output while not focused)
    HasActivity,
    /// Tab is closing
    Closing,
}

/// Session state for persistence
#[derive(Debug, Clone)]
pub struct SessionState {
    /// Current working directory
    pub cwd: String,
    /// Environment variables
    pub env: Vec<(String, String)>,
    /// Command history
    pub history: Vec<String>,
    /// Scrollback buffer saved content
    pub scrollback_saved: bool,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            cwd: String::from("/"),
            env: Vec::new(),
            history: Vec::new(),
            scrollback_saved: false,
        }
    }
}

/// Terminal color scheme/profile
#[derive(Debug, Clone)]
pub struct TerminalProfile {
    pub name: String,
    pub foreground: Color,
    pub background: Color,
    pub cursor_color: Color,
    pub selection_color: Color,
    pub colors: [Color; 16],
    pub font_size: usize,
    pub opacity: u8,
}

impl Default for TerminalProfile {
    fn default() -> Self {
        Self {
            name: String::from("Default"),
            foreground: Color::new(204, 204, 204),
            background: Color::new(0, 0, 0),
            cursor_color: Color::new(255, 255, 255),
            selection_color: Color::new(100, 100, 200),
            colors: [
                Color::new(0, 0, 0),       // Black
                Color::new(170, 0, 0),     // Red
                Color::new(0, 170, 0),     // Green
                Color::new(170, 85, 0),    // Yellow
                Color::new(0, 0, 170),     // Blue
                Color::new(170, 0, 170),   // Magenta
                Color::new(0, 170, 170),   // Cyan
                Color::new(170, 170, 170), // White
                Color::new(85, 85, 85),    // Bright Black
                Color::new(255, 85, 85),   // Bright Red
                Color::new(85, 255, 85),   // Bright Green
                Color::new(255, 255, 85),  // Bright Yellow
                Color::new(85, 85, 255),   // Bright Blue
                Color::new(255, 85, 255),  // Bright Magenta
                Color::new(85, 255, 255),  // Bright Cyan
                Color::new(255, 255, 255), // Bright White
            ],
            font_size: 12,
            opacity: 255,
        }
    }
}

impl TerminalProfile {
    /// Dark theme
    pub fn dark() -> Self {
        Self::default()
    }

    /// Light theme
    pub fn light() -> Self {
        Self {
            name: String::from("Light"),
            foreground: Color::new(40, 40, 40),
            background: Color::new(255, 255, 255),
            cursor_color: Color::new(0, 0, 0),
            selection_color: Color::new(180, 200, 240),
            colors: [
                Color::new(0, 0, 0),
                Color::new(170, 0, 0),
                Color::new(0, 130, 0),
                Color::new(170, 85, 0),
                Color::new(0, 0, 170),
                Color::new(170, 0, 170),
                Color::new(0, 130, 130),
                Color::new(170, 170, 170),
                Color::new(85, 85, 85),
                Color::new(255, 85, 85),
                Color::new(85, 200, 85),
                Color::new(255, 200, 85),
                Color::new(85, 85, 255),
                Color::new(255, 85, 255),
                Color::new(85, 200, 200),
                Color::new(255, 255, 255),
            ],
            font_size: 12,
            opacity: 255,
        }
    }

    /// Solarized Dark theme
    pub fn solarized_dark() -> Self {
        Self {
            name: String::from("Solarized Dark"),
            foreground: Color::new(131, 148, 150),
            background: Color::new(0, 43, 54),
            cursor_color: Color::new(131, 148, 150),
            selection_color: Color::new(7, 54, 66),
            colors: [
                Color::new(7, 54, 66),     // Black
                Color::new(220, 50, 47),   // Red
                Color::new(133, 153, 0),   // Green
                Color::new(181, 137, 0),   // Yellow
                Color::new(38, 139, 210),  // Blue
                Color::new(211, 54, 130),  // Magenta
                Color::new(42, 161, 152),  // Cyan
                Color::new(238, 232, 213), // White
                Color::new(0, 43, 54),     // Bright Black
                Color::new(203, 75, 22),   // Bright Red
                Color::new(88, 110, 117),  // Bright Green
                Color::new(101, 123, 131), // Bright Yellow
                Color::new(131, 148, 150), // Bright Blue
                Color::new(108, 113, 196), // Bright Magenta
                Color::new(147, 161, 161), // Bright Cyan
                Color::new(253, 246, 227), // Bright White
            ],
            font_size: 12,
            opacity: 255,
        }
    }

    /// Monokai theme
    pub fn monokai() -> Self {
        Self {
            name: String::from("Monokai"),
            foreground: Color::new(248, 248, 242),
            background: Color::new(39, 40, 34),
            cursor_color: Color::new(248, 248, 242),
            selection_color: Color::new(73, 72, 62),
            colors: [
                Color::new(39, 40, 34),    // Black
                Color::new(249, 38, 114),  // Red
                Color::new(166, 226, 46),  // Green
                Color::new(244, 191, 117), // Yellow
                Color::new(102, 217, 239), // Blue
                Color::new(174, 129, 255), // Magenta
                Color::new(161, 239, 228), // Cyan
                Color::new(248, 248, 242), // White
                Color::new(117, 113, 94),  // Bright Black
                Color::new(249, 38, 114),  // Bright Red
                Color::new(166, 226, 46),  // Bright Green
                Color::new(244, 191, 117), // Bright Yellow
                Color::new(102, 217, 239), // Bright Blue
                Color::new(174, 129, 255), // Bright Magenta
                Color::new(161, 239, 228), // Bright Cyan
                Color::new(248, 248, 242), // Bright White
            ],
            font_size: 12,
            opacity: 255,
        }
    }
}

/// Split direction for terminal panes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

/// A pane in a split terminal view
pub struct TerminalPane {
    pub id: TabId,
    pub terminal: Terminal,
    pub split: Option<(SplitDirection, f32)>, // direction and position (0.0-1.0)
    pub children: Vec<TerminalPane>,
}

impl TerminalPane {
    pub fn new(x: isize, y: isize, width: usize, height: usize) -> Self {
        Self {
            id: TabId::new(),
            terminal: Terminal::new(x, y, width, height),
            split: None,
            children: Vec::new(),
        }
    }
}

/// A single terminal tab
pub struct TerminalTab {
    /// Unique identifier
    pub id: TabId,
    /// Tab display name
    pub name: String,
    /// Custom name set by user
    pub custom_name: Option<String>,
    /// Tab state
    pub state: TabState,
    /// Terminal widget
    pub terminal: Terminal,
    /// Session state
    pub session: SessionState,
    /// Terminal profile
    pub profile: TerminalProfile,
    /// Has unsaved activity notification
    pub has_activity: bool,
    /// Process ID if connected to shell
    pub pid: Option<u64>,
    /// Icon (emoji or name)
    pub icon: Option<String>,
    /// Is this tab pinned?
    pub pinned: bool,
}

impl TerminalTab {
    /// Create a new terminal tab
    pub fn new(x: isize, y: isize, width: usize, height: usize) -> Self {
        static TAB_NUM: AtomicU64 = AtomicU64::new(1);
        let num = TAB_NUM.fetch_add(1, Ordering::Relaxed);

        Self {
            id: TabId::new(),
            name: format!("Terminal {}", num),
            custom_name: None,
            state: TabState::Background,
            terminal: Terminal::new(x, y, width, height),
            session: SessionState::default(),
            profile: TerminalProfile::default(),
            has_activity: false,
            pid: None,
            icon: None,
            pinned: false,
        }
    }

    /// Get display name
    pub fn display_name(&self) -> &str {
        self.custom_name.as_ref().unwrap_or(&self.name)
    }

    /// Set custom name
    pub fn set_name(&mut self, name: &str) {
        self.custom_name = Some(String::from(name));
    }

    /// Clear custom name, revert to auto-generated
    pub fn clear_custom_name(&mut self) {
        self.custom_name = None;
    }

    /// Update name from terminal title (OSC)
    pub fn update_from_terminal(&mut self) {
        if self.custom_name.is_none() {
            let title = self.terminal.title();
            if !title.is_empty() && title != "Terminal" {
                self.name = String::from(title);
            }
        }
    }
}

/// View mode for terminal tabs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabViewMode {
    /// Normal single terminal view
    Single,
    /// Grid view showing multiple terminals
    Grid,
    /// Split view with panes
    Split,
}

/// Tab drag state
#[derive(Debug, Clone)]
struct DragState {
    tab_id: TabId,
    start_x: isize,
    start_y: isize,
    current_x: isize,
    current_y: isize,
    original_index: usize,
}

/// Multi-tab terminal emulator widget
pub struct TerminalTabs {
    id: WidgetId,
    bounds: Bounds,

    /// All terminal tabs
    tabs: Vec<TerminalTab>,
    /// Index of active tab
    active_tab: usize,

    /// View mode
    view_mode: TabViewMode,

    /// Available profiles
    profiles: Vec<TerminalProfile>,

    /// Tab bar height
    tab_bar_height: usize,

    /// Tab width
    tab_width: usize,

    /// Max visible tabs before scrolling
    max_visible_tabs: usize,

    /// Scroll offset for tab bar
    tab_scroll_offset: usize,

    /// Hovered tab index
    hovered_tab: Option<usize>,

    /// Hovered close button
    hovered_close: Option<usize>,

    /// Tab being dragged
    drag_state: Option<DragState>,

    /// Is renaming a tab
    renaming_tab: Option<usize>,
    rename_buffer: String,

    /// Show new tab button
    show_new_tab: bool,

    /// Keyboard shortcuts enabled
    shortcuts_enabled: bool,

    /// Widget state
    visible: bool,
    focused: bool,

    /// Recent closed tabs (for undo)
    closed_tabs: Vec<(usize, TerminalTab)>,
    max_closed_tabs: usize,
}

impl TerminalTabs {
    const TAB_BAR_HEIGHT: usize = 32;
    const TAB_WIDTH: usize = 180;
    const TAB_MIN_WIDTH: usize = 80;
    const CLOSE_BTN_SIZE: usize = 16;
    const NEW_TAB_BTN_WIDTH: usize = 32;

    /// Create a new terminal tabs widget
    pub fn new(x: isize, y: isize, width: usize, height: usize) -> Self {
        let tab_bar_height = Self::TAB_BAR_HEIGHT;
        let terminal_height = height.saturating_sub(tab_bar_height);

        // Create first tab
        let first_tab = TerminalTab::new(x, y + tab_bar_height as isize, width, terminal_height);

        // Calculate max visible tabs
        let max_visible = (width.saturating_sub(Self::NEW_TAB_BTN_WIDTH)) / Self::TAB_WIDTH;

        let mut widget = Self {
            id: WidgetId::new(),
            bounds: Bounds::new(x, y, width, height),
            tabs: vec![first_tab],
            active_tab: 0,
            view_mode: TabViewMode::Single,
            profiles: vec![
                TerminalProfile::dark(),
                TerminalProfile::light(),
                TerminalProfile::solarized_dark(),
                TerminalProfile::monokai(),
            ],
            tab_bar_height,
            tab_width: Self::TAB_WIDTH,
            max_visible_tabs: max_visible,
            tab_scroll_offset: 0,
            hovered_tab: None,
            hovered_close: None,
            drag_state: None,
            renaming_tab: None,
            rename_buffer: String::new(),
            show_new_tab: true,
            shortcuts_enabled: true,
            visible: true,
            focused: true,
            closed_tabs: Vec::new(),
            max_closed_tabs: 10,
        };

        // Set first tab as active
        if let Some(tab) = widget.tabs.first_mut() {
            tab.state = TabState::Active;
        }

        widget
    }

    /// Get number of tabs
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Get active tab
    pub fn active_tab(&self) -> Option<&TerminalTab> {
        self.tabs.get(self.active_tab)
    }

    /// Get active tab mutable
    pub fn active_tab_mut(&mut self) -> Option<&mut TerminalTab> {
        self.tabs.get_mut(self.active_tab)
    }

    /// Get active terminal
    pub fn active_terminal(&self) -> Option<&Terminal> {
        self.tabs.get(self.active_tab).map(|t| &t.terminal)
    }

    /// Get active terminal mutable
    pub fn active_terminal_mut(&mut self) -> Option<&mut Terminal> {
        self.tabs.get_mut(self.active_tab).map(|t| &mut t.terminal)
    }

    /// Create a new tab
    pub fn new_tab(&mut self) -> TabId {
        let terminal_y = self.bounds.y + self.tab_bar_height as isize;
        let terminal_height = self.bounds.height.saturating_sub(self.tab_bar_height);

        let mut tab = TerminalTab::new(
            self.bounds.x,
            terminal_y,
            self.bounds.width,
            terminal_height,
        );

        let id = tab.id;
        tab.state = TabState::Active;

        // Deactivate current tab
        if let Some(current) = self.tabs.get_mut(self.active_tab) {
            current.state = TabState::Background;
        }

        self.tabs.push(tab);
        self.active_tab = self.tabs.len() - 1;

        // Scroll to show new tab
        self.ensure_tab_visible(self.active_tab);

        id
    }

    /// Close a tab by index
    pub fn close_tab(&mut self, index: usize) -> bool {
        if self.tabs.len() <= 1 {
            return false; // Keep at least one tab
        }

        if index >= self.tabs.len() {
            return false;
        }

        // Don't close pinned tabs without explicit action
        if self.tabs[index].pinned {
            return false;
        }

        // Save to closed tabs for undo
        let tab = self.tabs.remove(index);
        if self.closed_tabs.len() >= self.max_closed_tabs {
            self.closed_tabs.remove(0);
        }
        self.closed_tabs.push((index, tab));

        // Adjust active tab
        if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        } else if self.active_tab > index {
            self.active_tab -= 1;
        }

        // Activate new current tab
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.state = TabState::Active;
        }

        true
    }

    /// Close active tab
    pub fn close_active_tab(&mut self) -> bool {
        self.close_tab(self.active_tab)
    }

    /// Reopen last closed tab
    pub fn reopen_closed_tab(&mut self) -> bool {
        if let Some((index, mut tab)) = self.closed_tabs.pop() {
            // Deactivate current
            if let Some(current) = self.tabs.get_mut(self.active_tab) {
                current.state = TabState::Background;
            }

            // Insert at original position or end
            let insert_at = index.min(self.tabs.len());
            tab.state = TabState::Active;
            self.tabs.insert(insert_at, tab);
            self.active_tab = insert_at;

            true
        } else {
            false
        }
    }

    /// Switch to tab by index
    pub fn switch_to_tab(&mut self, index: usize) {
        if index < self.tabs.len() && index != self.active_tab {
            // Deactivate current
            if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                tab.state = TabState::Background;
            }

            // Activate new
            self.active_tab = index;
            if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                tab.state = TabState::Active;
                tab.has_activity = false;
            }

            self.ensure_tab_visible(index);
        }
    }

    /// Switch to next tab
    pub fn next_tab(&mut self) {
        let next = (self.active_tab + 1) % self.tabs.len();
        self.switch_to_tab(next);
    }

    /// Switch to previous tab
    pub fn prev_tab(&mut self) {
        let prev = if self.active_tab == 0 {
            self.tabs.len() - 1
        } else {
            self.active_tab - 1
        };
        self.switch_to_tab(prev);
    }

    /// Move tab to new position
    pub fn move_tab(&mut self, from: usize, to: usize) {
        if from < self.tabs.len() && to < self.tabs.len() && from != to {
            let tab = self.tabs.remove(from);
            self.tabs.insert(to, tab);

            // Adjust active tab index
            if self.active_tab == from {
                self.active_tab = to;
            } else if from < self.active_tab && to >= self.active_tab {
                self.active_tab -= 1;
            } else if from > self.active_tab && to <= self.active_tab {
                self.active_tab += 1;
            }
        }
    }

    /// Pin/unpin a tab
    pub fn toggle_pin(&mut self, index: usize) {
        if let Some(tab) = self.tabs.get_mut(index) {
            tab.pinned = !tab.pinned;

            // Move pinned tabs to front
            if tab.pinned {
                let pinned_count = self.tabs.iter().take(index).filter(|t| t.pinned).count();
                self.move_tab(index, pinned_count);
            }
        }
    }

    /// Start renaming a tab
    pub fn start_rename(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.renaming_tab = Some(index);
            self.rename_buffer = String::from(self.tabs[index].display_name());
        }
    }

    /// Finish renaming
    pub fn finish_rename(&mut self) {
        if let Some(index) = self.renaming_tab {
            if !self.rename_buffer.is_empty() {
                if let Some(tab) = self.tabs.get_mut(index) {
                    tab.set_name(&self.rename_buffer);
                }
            }
        }
        self.renaming_tab = None;
        self.rename_buffer.clear();
    }

    /// Cancel renaming
    pub fn cancel_rename(&mut self) {
        self.renaming_tab = None;
        self.rename_buffer.clear();
    }

    /// Ensure tab is visible in tab bar
    fn ensure_tab_visible(&mut self, index: usize) {
        if index < self.tab_scroll_offset {
            self.tab_scroll_offset = index;
        } else if index >= self.tab_scroll_offset + self.max_visible_tabs {
            self.tab_scroll_offset = index - self.max_visible_tabs + 1;
        }
    }

    /// Get tab index at position
    fn tab_at_position(&self, x: isize, y: isize) -> Option<usize> {
        if y < self.bounds.y || y >= self.bounds.y + self.tab_bar_height as isize {
            return None;
        }

        let rel_x = (x - self.bounds.x) as usize;

        // Calculate actual tab width (shrink if too many tabs)
        let visible_count = self.tabs.len().min(self.max_visible_tabs);
        let available_width = self.bounds.width.saturating_sub(Self::NEW_TAB_BTN_WIDTH);
        let actual_tab_width = (available_width / visible_count.max(1))
            .min(Self::TAB_WIDTH)
            .max(Self::TAB_MIN_WIDTH);

        if rel_x < visible_count * actual_tab_width {
            let index = self.tab_scroll_offset + rel_x / actual_tab_width;
            if index < self.tabs.len() {
                return Some(index);
            }
        }

        None
    }

    /// Check if position is over close button
    fn is_over_close_button(&self, tab_index: usize, x: isize, y: isize) -> bool {
        let visible_count = self.tabs.len().min(self.max_visible_tabs);
        let available_width = self.bounds.width.saturating_sub(Self::NEW_TAB_BTN_WIDTH);
        let actual_tab_width = (available_width / visible_count.max(1))
            .min(Self::TAB_WIDTH)
            .max(Self::TAB_MIN_WIDTH);

        let visible_index = tab_index.saturating_sub(self.tab_scroll_offset);
        let tab_x = self.bounds.x + (visible_index * actual_tab_width) as isize;

        // Close button is in top-right of tab
        let close_x = tab_x + actual_tab_width as isize - Self::CLOSE_BTN_SIZE as isize - 4;
        let close_y = self.bounds.y + (self.tab_bar_height - Self::CLOSE_BTN_SIZE) as isize / 2;

        x >= close_x && x < close_x + Self::CLOSE_BTN_SIZE as isize &&
        y >= close_y && y < close_y + Self::CLOSE_BTN_SIZE as isize
    }

    /// Check if position is over new tab button
    fn is_over_new_tab_button(&self, x: isize, y: isize) -> bool {
        if y < self.bounds.y || y >= self.bounds.y + self.tab_bar_height as isize {
            return false;
        }

        let btn_x = self.bounds.x + self.bounds.width as isize - Self::NEW_TAB_BTN_WIDTH as isize;
        x >= btn_x && x < self.bounds.x + self.bounds.width as isize
    }

    /// Handle keyboard shortcut
    fn handle_shortcut(&mut self, key: u8, modifiers: u8) -> bool {
        if !self.shortcuts_enabled {
            return false;
        }

        let ctrl = (modifiers & 0x02) != 0;
        let shift = (modifiers & 0x01) != 0;
        let alt = (modifiers & 0x04) != 0;

        // Ctrl+T: New tab
        if ctrl && key == 0x14 {
            self.new_tab();
            return true;
        }

        // Ctrl+W: Close tab
        if ctrl && key == 0x11 {
            self.close_active_tab();
            return true;
        }

        // Ctrl+Shift+T: Reopen closed tab
        if ctrl && shift && key == 0x14 {
            self.reopen_closed_tab();
            return true;
        }

        // Ctrl+Tab: Next tab
        if ctrl && key == 0x0F {
            if shift {
                self.prev_tab();
            } else {
                self.next_tab();
            }
            return true;
        }

        // Ctrl+1-9: Switch to tab
        if ctrl && key >= 0x02 && key <= 0x0A {
            let index = (key - 0x02) as usize;
            if index < self.tabs.len() {
                self.switch_to_tab(index);
            }
            return true;
        }

        // Alt+Left/Right: Move tab
        if alt {
            if key == 0x4B { // Left
                if self.active_tab > 0 {
                    self.move_tab(self.active_tab, self.active_tab - 1);
                }
                return true;
            } else if key == 0x4D { // Right
                if self.active_tab < self.tabs.len() - 1 {
                    self.move_tab(self.active_tab, self.active_tab + 1);
                }
                return true;
            }
        }

        false
    }

    /// Write to active terminal
    pub fn write(&mut self, data: &[u8]) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.terminal.write(data);
            tab.update_from_terminal();
        }
    }

    /// Write to terminal by index
    pub fn write_to_tab(&mut self, index: usize, data: &[u8]) {
        if let Some(tab) = self.tabs.get_mut(index) {
            tab.terminal.write(data);
            tab.update_from_terminal();

            // Mark activity if not active
            if tab.state == TabState::Background {
                tab.has_activity = true;
                tab.state = TabState::HasActivity;
            }
        }
    }

    /// Draw text helper
    fn draw_text(&self, surface: &mut Surface, x: usize, y: usize, text: &str, color: Color) {
        let mut cx = x;
        for c in text.chars() {
            if let Some(glyph) = DEFAULT_FONT.get_glyph(c) {
                for row in 0..DEFAULT_FONT.height {
                    let byte = glyph[row];
                    for col in 0..DEFAULT_FONT.width {
                        if (byte >> (DEFAULT_FONT.width - 1 - col)) & 1 != 0 {
                            surface.set_pixel(cx + col, y + row, color);
                        }
                    }
                }
            }
            cx += DEFAULT_FONT.width;
        }
    }

    /// Render tab bar
    fn render_tab_bar(&self, surface: &mut Surface) {
        let x = self.bounds.x.max(0) as usize;
        let y = self.bounds.y.max(0) as usize;
        let w = self.bounds.width;
        let h = self.tab_bar_height;

        // Tab bar background
        let bar_bg = Color::new(40, 42, 54);
        for py in 0..h {
            for px in 0..w {
                surface.set_pixel(x + px, y + py, bar_bg);
            }
        }

        // Calculate tab dimensions
        let visible_count = self.tabs.len().min(self.max_visible_tabs);
        let available_width = w.saturating_sub(Self::NEW_TAB_BTN_WIDTH);
        let actual_tab_width = (available_width / visible_count.max(1))
            .min(Self::TAB_WIDTH)
            .max(Self::TAB_MIN_WIDTH);

        // Draw tabs
        for i in 0..visible_count {
            let tab_index = self.tab_scroll_offset + i;
            if tab_index >= self.tabs.len() {
                break;
            }

            let tab = &self.tabs[tab_index];
            let tab_x = x + i * actual_tab_width;

            // Tab background
            let tab_bg = if tab_index == self.active_tab {
                Color::new(68, 71, 90)
            } else if self.hovered_tab == Some(tab_index) {
                Color::new(55, 57, 70)
            } else {
                bar_bg
            };

            for py in 2..h - 2 {
                for px in 1..actual_tab_width - 1 {
                    surface.set_pixel(tab_x + px, y + py, tab_bg);
                }
            }

            // Active tab indicator
            if tab_index == self.active_tab {
                let indicator_color = Color::new(139, 233, 253);
                for px in 1..actual_tab_width - 1 {
                    surface.set_pixel(tab_x + px, y + h - 2, indicator_color);
                    surface.set_pixel(tab_x + px, y + h - 1, indicator_color);
                }
            }

            // Activity indicator
            if tab.has_activity && tab_index != self.active_tab {
                let activity_color = Color::new(255, 121, 198);
                for px in 1..6 {
                    for py in 2..7 {
                        surface.set_pixel(tab_x + px, y + py, activity_color);
                    }
                }
            }

            // Pin indicator
            if tab.pinned {
                let pin_color = Color::new(241, 250, 140);
                // Simple pin icon
                for py in 4..10 {
                    surface.set_pixel(tab_x + 4, y + py, pin_color);
                }
                for px in 2..7 {
                    surface.set_pixel(tab_x + px, y + 4, pin_color);
                }
            }

            // Tab name
            let name = tab.display_name();
            let max_name_len = (actual_tab_width - Self::CLOSE_BTN_SIZE - 16) / 8;
            let display_name = if name.len() > max_name_len {
                let truncated: String = name.chars().take(max_name_len.saturating_sub(2)).collect();
                format!("{}..", truncated)
            } else {
                String::from(name)
            };

            let text_color = if tab_index == self.active_tab {
                Color::new(248, 248, 242)
            } else {
                Color::new(189, 147, 249)
            };

            let text_x = tab_x + if tab.pinned { 12 } else { 8 };
            let text_y = y + (h - 16) / 2;
            self.draw_text(surface, text_x, text_y, &display_name, text_color);

            // Close button (not for pinned tabs)
            if !tab.pinned {
                let close_x = tab_x + actual_tab_width - Self::CLOSE_BTN_SIZE - 4;
                let close_y = y + (h - Self::CLOSE_BTN_SIZE) / 2;

                let close_bg = if self.hovered_close == Some(tab_index) {
                    Color::new(255, 85, 85)
                } else {
                    tab_bg
                };

                // Close button background
                for py in 0..Self::CLOSE_BTN_SIZE {
                    for px in 0..Self::CLOSE_BTN_SIZE {
                        surface.set_pixel(close_x + px, close_y + py, close_bg);
                    }
                }

                // X icon
                let x_color = Color::new(248, 248, 242);
                for i in 2..Self::CLOSE_BTN_SIZE - 2 {
                    surface.set_pixel(close_x + i, close_y + i, x_color);
                    surface.set_pixel(close_x + i, close_y + Self::CLOSE_BTN_SIZE - 1 - i, x_color);
                }
            }
        }

        // New tab button
        if self.show_new_tab {
            let btn_x = x + w - Self::NEW_TAB_BTN_WIDTH;
            let btn_bg = Color::new(55, 57, 70);

            for py in 4..h - 4 {
                for px in 4..Self::NEW_TAB_BTN_WIDTH - 4 {
                    surface.set_pixel(btn_x + px, y + py, btn_bg);
                }
            }

            // Plus icon
            let plus_color = Color::new(80, 250, 123);
            let center_x = btn_x + Self::NEW_TAB_BTN_WIDTH / 2;
            let center_y = y + h / 2;

            // Horizontal line
            for i in 0..10 {
                surface.set_pixel(center_x - 5 + i, center_y, plus_color);
            }
            // Vertical line
            for i in 0..10 {
                surface.set_pixel(center_x, center_y - 5 + i, plus_color);
            }
        }

        // Scroll indicators
        if self.tab_scroll_offset > 0 {
            // Left scroll indicator
            let arrow_color = Color::new(139, 233, 253);
            for i in 0..5 {
                surface.set_pixel(x + 2 + i, y + h / 2 - i, arrow_color);
                surface.set_pixel(x + 2 + i, y + h / 2 + i, arrow_color);
            }
        }

        if self.tab_scroll_offset + self.max_visible_tabs < self.tabs.len() {
            // Right scroll indicator
            let arrow_x = x + w - Self::NEW_TAB_BTN_WIDTH - 10;
            let arrow_color = Color::new(139, 233, 253);
            for i in 0..5 {
                surface.set_pixel(arrow_x + 5 - i, y + h / 2 - i, arrow_color);
                surface.set_pixel(arrow_x + 5 - i, y + h / 2 + i, arrow_color);
            }
        }

        // Bottom border
        let border_color = Color::new(68, 71, 90);
        for px in 0..w {
            surface.set_pixel(x + px, y + h - 1, border_color);
        }
    }

    /// Render rename input
    fn render_rename_input(&self, surface: &mut Surface) {
        if let Some(index) = self.renaming_tab {
            let visible_count = self.tabs.len().min(self.max_visible_tabs);
            let available_width = self.bounds.width.saturating_sub(Self::NEW_TAB_BTN_WIDTH);
            let actual_tab_width = (available_width / visible_count.max(1))
                .min(Self::TAB_WIDTH)
                .max(Self::TAB_MIN_WIDTH);

            let visible_index = index.saturating_sub(self.tab_scroll_offset);
            if visible_index >= visible_count {
                return;
            }

            let x = self.bounds.x.max(0) as usize + visible_index * actual_tab_width + 4;
            let y = self.bounds.y.max(0) as usize + 6;
            let w = actual_tab_width - Self::CLOSE_BTN_SIZE - 12;
            let h = 20;

            // Input background
            let bg = Color::new(30, 30, 40);
            for py in 0..h {
                for px in 0..w {
                    surface.set_pixel(x + px, y + py, bg);
                }
            }

            // Input border
            let border = Color::new(139, 233, 253);
            for px in 0..w {
                surface.set_pixel(x + px, y, border);
                surface.set_pixel(x + px, y + h - 1, border);
            }
            for py in 0..h {
                surface.set_pixel(x, y + py, border);
                surface.set_pixel(x + w - 1, y + py, border);
            }

            // Text
            let text_color = Color::new(248, 248, 242);
            let max_chars = (w - 8) / 8;
            let display_text: String = self.rename_buffer.chars().take(max_chars).collect();
            self.draw_text(surface, x + 4, y + 4, &display_text, text_color);

            // Cursor
            let cursor_x = x + 4 + display_text.len() * 8;
            if cursor_x < x + w - 4 {
                for py in 2..h - 2 {
                    surface.set_pixel(cursor_x, y + py, text_color);
                }
            }
        }
    }
}

impl Widget for TerminalTabs {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn bounds(&self) -> Bounds {
        self.bounds
    }

    fn set_position(&mut self, x: isize, y: isize) {
        self.bounds.x = x;
        self.bounds.y = y;

        // Update terminal positions
        let terminal_y = y + self.tab_bar_height as isize;
        for tab in &mut self.tabs {
            tab.terminal.set_position(x, terminal_y);
        }
    }

    fn set_size(&mut self, width: usize, height: usize) {
        self.bounds.width = width;
        self.bounds.height = height;

        // Recalculate max visible tabs
        self.max_visible_tabs = (width.saturating_sub(Self::NEW_TAB_BTN_WIDTH)) / Self::TAB_WIDTH;

        // Update terminal sizes
        let terminal_height = height.saturating_sub(self.tab_bar_height);
        for tab in &mut self.tabs {
            tab.terminal.set_size(width, terminal_height);
        }
    }

    fn is_enabled(&self) -> bool {
        true
    }

    fn set_enabled(&mut self, _enabled: bool) {}

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn handle_event(&mut self, event: &WidgetEvent) -> bool {
        match event {
            WidgetEvent::Focus => {
                self.focused = true;
                true
            }
            WidgetEvent::Blur => {
                self.focused = false;
                true
            }
            WidgetEvent::KeyDown { key, modifiers } => {
                // Handle rename mode
                if self.renaming_tab.is_some() {
                    match *key {
                        0x1C => { // Enter
                            self.finish_rename();
                            return true;
                        }
                        0x01 => { // Escape
                            self.cancel_rename();
                            return true;
                        }
                        0x0E | 0x7F => { // Backspace
                            self.rename_buffer.pop();
                            return true;
                        }
                        _ => {}
                    }
                    return true;
                }

                // Check for shortcuts
                if self.handle_shortcut(*key, *modifiers) {
                    return true;
                }

                // Forward to active terminal
                if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                    return tab.terminal.handle_event(event);
                }
                false
            }
            WidgetEvent::Character { c } => {
                // Handle rename mode
                if self.renaming_tab.is_some() {
                    if self.rename_buffer.len() < 32 {
                        self.rename_buffer.push(*c);
                    }
                    return true;
                }

                // Forward to active terminal
                if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                    return tab.terminal.handle_event(event);
                }
                false
            }
            WidgetEvent::MouseDown { button: MouseButton::Left, x, y } => {
                // Check tab bar
                if *y >= self.bounds.y && *y < self.bounds.y + self.tab_bar_height as isize {
                    // New tab button
                    if self.is_over_new_tab_button(*x, *y) {
                        self.new_tab();
                        return true;
                    }

                    // Tab click
                    if let Some(index) = self.tab_at_position(*x, *y) {
                        // Close button
                        if self.is_over_close_button(index, *x, *y) {
                            self.close_tab(index);
                            return true;
                        }

                        // Start drag
                        self.drag_state = Some(DragState {
                            tab_id: self.tabs[index].id,
                            start_x: *x,
                            start_y: *y,
                            current_x: *x,
                            current_y: *y,
                            original_index: index,
                        });

                        // Switch to tab
                        self.switch_to_tab(index);
                        return true;
                    }

                    return true;
                }

                // Forward to terminal
                if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                    return tab.terminal.handle_event(event);
                }
                false
            }
            WidgetEvent::MouseDown { button: MouseButton::Middle, x, y } => {
                // Middle click on tab closes it
                if *y >= self.bounds.y && *y < self.bounds.y + self.tab_bar_height as isize {
                    if let Some(index) = self.tab_at_position(*x, *y) {
                        self.close_tab(index);
                        return true;
                    }
                }
                false
            }
            WidgetEvent::MouseUp { button: MouseButton::Left, x, y } => {
                if let Some(drag) = self.drag_state.take() {
                    // Check if dropped on different position
                    if let Some(drop_index) = self.tab_at_position(*x, *y) {
                        if drop_index != drag.original_index {
                            self.move_tab(self.active_tab, drop_index);
                        }
                    }
                    return true;
                }

                if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                    return tab.terminal.handle_event(event);
                }
                false
            }
            WidgetEvent::MouseMove { x, y } => {
                // Update drag state
                if let Some(ref mut drag) = self.drag_state {
                    drag.current_x = *x;
                    drag.current_y = *y;
                }

                // Update hover state
                if *y >= self.bounds.y && *y < self.bounds.y + self.tab_bar_height as isize {
                    self.hovered_tab = self.tab_at_position(*x, *y);

                    // Check close button hover
                    if let Some(index) = self.hovered_tab {
                        if self.is_over_close_button(index, *x, *y) {
                            self.hovered_close = Some(index);
                        } else {
                            self.hovered_close = None;
                        }
                    } else {
                        self.hovered_close = None;
                    }

                    return true;
                } else {
                    self.hovered_tab = None;
                    self.hovered_close = None;
                }

                // Forward to terminal
                if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                    return tab.terminal.handle_event(event);
                }
                false
            }
            WidgetEvent::Scroll { delta_y, .. } => {
                // Scroll tab bar
                if self.hovered_tab.is_some() {
                    if *delta_y > 0 && self.tab_scroll_offset > 0 {
                        self.tab_scroll_offset -= 1;
                    } else if *delta_y < 0 && self.tab_scroll_offset + self.max_visible_tabs < self.tabs.len() {
                        self.tab_scroll_offset += 1;
                    }
                    return true;
                }

                // Forward to terminal
                if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                    return tab.terminal.handle_event(event);
                }
                false
            }
            WidgetEvent::MouseDown { button: MouseButton::Right, x, y } => {
                // Right click on tab for context menu (rename)
                if *y >= self.bounds.y && *y < self.bounds.y + self.tab_bar_height as isize {
                    if let Some(index) = self.tab_at_position(*x, *y) {
                        self.start_rename(index);
                        return true;
                    }
                }
                false
            }
            _ => {
                // Forward other events to terminal
                if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                    return tab.terminal.handle_event(event);
                }
                false
            }
        }
    }

    fn render(&self, surface: &mut Surface) {
        if !self.visible {
            return;
        }

        // Render tab bar
        self.render_tab_bar(surface);

        // Render active terminal
        if let Some(tab) = self.tabs.get(self.active_tab) {
            tab.terminal.render(surface);
        }

        // Render rename input
        self.render_rename_input(surface);

        // Render drag indicator
        if let Some(ref drag) = self.drag_state {
            if let Some(drop_index) = self.tab_at_position(drag.current_x, drag.current_y) {
                if drop_index != self.active_tab {
                    // Draw drop indicator
                    let visible_count = self.tabs.len().min(self.max_visible_tabs);
                    let available_width = self.bounds.width.saturating_sub(Self::NEW_TAB_BTN_WIDTH);
                    let actual_tab_width = (available_width / visible_count.max(1))
                        .min(Self::TAB_WIDTH)
                        .max(Self::TAB_MIN_WIDTH);

                    let visible_index = drop_index.saturating_sub(self.tab_scroll_offset);
                    let indicator_x = self.bounds.x.max(0) as usize + visible_index * actual_tab_width;
                    let indicator_y = self.bounds.y.max(0) as usize;

                    let indicator_color = Color::new(80, 250, 123);
                    for py in 0..self.tab_bar_height {
                        surface.set_pixel(indicator_x, indicator_y + py, indicator_color);
                        surface.set_pixel(indicator_x + 1, indicator_y + py, indicator_color);
                    }
                }
            }
        }
    }
}
