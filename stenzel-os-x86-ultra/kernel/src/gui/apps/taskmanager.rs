//! Task Manager
//!
//! A graphical task manager for viewing and managing processes.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use crate::gui::widgets::{Widget, WidgetId, WidgetEvent, Bounds, MouseButton, theme};

/// Process state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    Running,
    Sleeping,
    Waiting,
    Stopped,
    Zombie,
}

impl ProcessState {
    pub fn label(&self) -> &'static str {
        match self {
            ProcessState::Running => "Running",
            ProcessState::Sleeping => "Sleeping",
            ProcessState::Waiting => "Waiting",
            ProcessState::Stopped => "Stopped",
            ProcessState::Zombie => "Zombie",
        }
    }

    pub fn short(&self) -> &'static str {
        match self {
            ProcessState::Running => "R",
            ProcessState::Sleeping => "S",
            ProcessState::Waiting => "W",
            ProcessState::Stopped => "T",
            ProcessState::Zombie => "Z",
        }
    }
}

/// Process information
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub ppid: u32,
    pub name: String,
    pub state: ProcessState,
    pub cpu_percent: f32,
    pub memory_kb: u64,
    pub threads: usize,
    pub user: String,
}

impl ProcessInfo {
    pub fn new(pid: u32, name: &str) -> Self {
        Self {
            pid,
            ppid: 0,
            name: String::from(name),
            state: ProcessState::Running,
            cpu_percent: 0.0,
            memory_kb: 0,
            threads: 1,
            user: String::from("root"),
        }
    }
}

/// System statistics
#[derive(Debug, Clone, Default)]
pub struct SystemStats {
    pub cpu_percent: f32,
    pub cpu_cores: usize,
    pub per_core_cpu: Vec<f32>,
    pub memory_total_kb: u64,
    pub memory_used_kb: u64,
    pub memory_free_kb: u64,
    pub memory_cached_kb: u64,
    pub swap_total_kb: u64,
    pub swap_used_kb: u64,
    pub uptime_secs: u64,
    pub process_count: usize,
    pub thread_count: usize,
}

/// Tab selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskManagerTab {
    Processes,
    Performance,
    Details,
}

impl TaskManagerTab {
    pub fn label(&self) -> &'static str {
        match self {
            TaskManagerTab::Processes => "Processes",
            TaskManagerTab::Performance => "Performance",
            TaskManagerTab::Details => "Details",
        }
    }

    pub fn all() -> &'static [TaskManagerTab] {
        &[
            TaskManagerTab::Processes,
            TaskManagerTab::Performance,
            TaskManagerTab::Details,
        ]
    }
}

/// Sort column for processes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortColumn {
    Pid,
    Name,
    Cpu,
    Memory,
    State,
}

/// Sort order
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    Ascending,
    Descending,
}

/// Kill callback
pub type KillCallback = fn(u32) -> bool; // pid -> success

/// Task Manager widget
pub struct TaskManager {
    id: WidgetId,
    bounds: Bounds,

    // Current tab
    current_tab: TaskManagerTab,

    // Data
    processes: Vec<ProcessInfo>,
    stats: SystemStats,

    // Selection
    selected_pid: Option<u32>,
    hover_row: Option<usize>,

    // Sorting
    sort_column: SortColumn,
    sort_order: SortOrder,

    // Scrolling
    scroll_offset: usize,

    // UI state
    visible: bool,
    focused: bool,

    // History for graphs (performance tab)
    cpu_history: Vec<f32>,
    memory_history: Vec<f32>,
    history_max: usize,

    // Callbacks
    on_kill: Option<KillCallback>,
}

impl TaskManager {
    const CHAR_WIDTH: usize = 8;
    const CHAR_HEIGHT: usize = 16;
    const HEADER_HEIGHT: usize = 32;
    const TAB_HEIGHT: usize = 28;
    const ROW_HEIGHT: usize = 20;
    const COLUMN_HEADER_HEIGHT: usize = 24;
    const FOOTER_HEIGHT: usize = 40;
    const PADDING: usize = 8;

    // Column widths for process list
    const COL_PID: usize = 60;
    const COL_NAME: usize = 180;
    const COL_CPU: usize = 60;
    const COL_MEM: usize = 80;
    const COL_STATE: usize = 70;

    /// Create a new task manager
    pub fn new(x: isize, y: isize, width: usize, height: usize) -> Self {
        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(x, y, width, height),
            current_tab: TaskManagerTab::Processes,
            processes: Vec::new(),
            stats: SystemStats::default(),
            selected_pid: None,
            hover_row: None,
            sort_column: SortColumn::Cpu,
            sort_order: SortOrder::Descending,
            scroll_offset: 0,
            visible: true,
            focused: false,
            cpu_history: Vec::new(),
            memory_history: Vec::new(),
            history_max: 60, // 60 data points
            on_kill: None,
        }
    }

    /// Set kill callback
    pub fn set_on_kill(&mut self, callback: KillCallback) {
        self.on_kill = Some(callback);
    }

    /// Update processes list
    pub fn set_processes(&mut self, processes: Vec<ProcessInfo>) {
        self.processes = processes;
        self.sort_processes();
    }

    /// Update system stats
    pub fn set_stats(&mut self, stats: SystemStats) {
        // Add to history
        if self.cpu_history.len() >= self.history_max {
            self.cpu_history.remove(0);
        }
        self.cpu_history.push(stats.cpu_percent);

        let mem_percent = if stats.memory_total_kb > 0 {
            (stats.memory_used_kb as f32 / stats.memory_total_kb as f32) * 100.0
        } else {
            0.0
        };
        if self.memory_history.len() >= self.history_max {
            self.memory_history.remove(0);
        }
        self.memory_history.push(mem_percent);

        self.stats = stats;
    }

    /// Sort processes
    fn sort_processes(&mut self) {
        let sort_order = self.sort_order;
        let sort_column = self.sort_column;

        self.processes.sort_by(|a, b| {
            let cmp = match sort_column {
                SortColumn::Pid => a.pid.cmp(&b.pid),
                SortColumn::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortColumn::Cpu => a.cpu_percent.partial_cmp(&b.cpu_percent).unwrap_or(core::cmp::Ordering::Equal),
                SortColumn::Memory => a.memory_kb.cmp(&b.memory_kb),
                SortColumn::State => (a.state as u8).cmp(&(b.state as u8)),
            };

            if sort_order == SortOrder::Descending {
                cmp.reverse()
            } else {
                cmp
            }
        });
    }

    /// Toggle sort column
    pub fn toggle_sort(&mut self, column: SortColumn) {
        if self.sort_column == column {
            self.sort_order = match self.sort_order {
                SortOrder::Ascending => SortOrder::Descending,
                SortOrder::Descending => SortOrder::Ascending,
            };
        } else {
            self.sort_column = column;
            self.sort_order = SortOrder::Descending;
        }
        self.sort_processes();
    }

    /// Get visible rows
    fn visible_rows(&self) -> usize {
        let content_height = self.bounds.height
            .saturating_sub(Self::HEADER_HEIGHT + Self::TAB_HEIGHT + Self::COLUMN_HEADER_HEIGHT + Self::FOOTER_HEIGHT);
        content_height / Self::ROW_HEIGHT
    }

    /// Get row at point
    fn row_at_point(&self, x: isize, y: isize) -> Option<usize> {
        let local_y = (y - self.bounds.y) as usize;

        let list_start = Self::HEADER_HEIGHT + Self::TAB_HEIGHT + Self::COLUMN_HEADER_HEIGHT;
        if local_y < list_start {
            return None;
        }

        let row = (local_y - list_start) / Self::ROW_HEIGHT;
        let index = self.scroll_offset + row;

        if index < self.processes.len() {
            Some(index)
        } else {
            None
        }
    }

    /// Get column at point
    fn column_at_point(&self, x: isize) -> Option<SortColumn> {
        let local_x = (x - self.bounds.x - Self::PADDING as isize) as usize;

        if local_x < Self::COL_PID {
            Some(SortColumn::Pid)
        } else if local_x < Self::COL_PID + Self::COL_NAME {
            Some(SortColumn::Name)
        } else if local_x < Self::COL_PID + Self::COL_NAME + Self::COL_CPU {
            Some(SortColumn::Cpu)
        } else if local_x < Self::COL_PID + Self::COL_NAME + Self::COL_CPU + Self::COL_MEM {
            Some(SortColumn::Memory)
        } else {
            Some(SortColumn::State)
        }
    }

    /// Get tab at point
    fn tab_at_point(&self, x: isize, y: isize) -> Option<TaskManagerTab> {
        let local_x = (x - self.bounds.x) as usize;
        let local_y = (y - self.bounds.y) as usize;

        if local_y < Self::HEADER_HEIGHT || local_y >= Self::HEADER_HEIGHT + Self::TAB_HEIGHT {
            return None;
        }

        let tab_width = 100;
        let tab_idx = local_x / tab_width;
        TaskManagerTab::all().get(tab_idx).copied()
    }

    /// Check if end task button was clicked
    fn is_end_task_click(&self, x: isize, y: isize) -> bool {
        let local_x = (x - self.bounds.x) as usize;
        let local_y = (y - self.bounds.y) as usize;

        let button_x = self.bounds.width - 120;
        let button_y = self.bounds.height - Self::FOOTER_HEIGHT + 8;
        let button_w = 100;
        let button_h = 24;

        local_x >= button_x && local_x < button_x + button_w
            && local_y >= button_y && local_y < button_y + button_h
    }

    /// End selected task
    pub fn end_selected_task(&mut self) {
        if let Some(pid) = self.selected_pid {
            if let Some(callback) = self.on_kill {
                callback(pid);
            }
        }
    }
}

impl Widget for TaskManager {
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
            WidgetEvent::MouseDown { button: MouseButton::Left, x, y } => {
                // Check tabs
                if let Some(tab) = self.tab_at_point(*x, *y) {
                    self.current_tab = tab;
                    return true;
                }

                // Check column headers for sorting
                let local_y = (*y - self.bounds.y) as usize;
                let header_y = Self::HEADER_HEIGHT + Self::TAB_HEIGHT;
                if local_y >= header_y && local_y < header_y + Self::COLUMN_HEADER_HEIGHT {
                    if let Some(col) = self.column_at_point(*x) {
                        self.toggle_sort(col);
                        return true;
                    }
                }

                // Check end task button
                if self.is_end_task_click(*x, *y) {
                    self.end_selected_task();
                    return true;
                }

                // Check process row
                if let Some(row) = self.row_at_point(*x, *y) {
                    if let Some(proc) = self.processes.get(row) {
                        self.selected_pid = Some(proc.pid);
                    }
                    return true;
                }

                false
            }
            WidgetEvent::MouseMove { x, y } => {
                self.hover_row = self.row_at_point(*x, *y);
                true
            }
            WidgetEvent::Scroll { delta_y, .. } => {
                if self.current_tab == TaskManagerTab::Processes {
                    if *delta_y < 0 {
                        self.scroll_offset = self.scroll_offset.saturating_add(3);
                        let max = self.processes.len().saturating_sub(self.visible_rows());
                        self.scroll_offset = self.scroll_offset.min(max);
                    } else {
                        self.scroll_offset = self.scroll_offset.saturating_sub(3);
                    }
                }
                true
            }
            WidgetEvent::KeyDown { key, .. } => {
                if self.focused {
                    match key {
                        0x53 => { // Delete key
                            self.end_selected_task();
                            true
                        }
                        0x48 => { // Up
                            if let Some(pid) = self.selected_pid {
                                let idx = self.processes.iter().position(|p| p.pid == pid);
                                if let Some(i) = idx {
                                    if i > 0 {
                                        self.selected_pid = Some(self.processes[i - 1].pid);
                                    }
                                }
                            }
                            true
                        }
                        0x50 => { // Down
                            if let Some(pid) = self.selected_pid {
                                let idx = self.processes.iter().position(|p| p.pid == pid);
                                if let Some(i) = idx {
                                    if i + 1 < self.processes.len() {
                                        self.selected_pid = Some(self.processes[i + 1].pid);
                                    }
                                }
                            }
                            true
                        }
                        _ => false,
                    }
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn render(&self, surface: &mut Surface) {
        if !self.visible {
            return;
        }

        let theme = theme();
        let x = self.bounds.x.max(0) as usize;
        let y = self.bounds.y.max(0) as usize;
        let w = self.bounds.width;
        let h = self.bounds.height;

        // Background
        let bg = Color::new(30, 30, 30);
        for py in 0..h {
            for px in 0..w {
                surface.set_pixel(x + px, y + py, bg);
            }
        }

        // Header
        let header_bg = Color::new(45, 45, 48);
        for py in 0..Self::HEADER_HEIGHT {
            for px in 0..w {
                surface.set_pixel(x + px, y + py, header_bg);
            }
        }

        draw_string(surface, x + Self::PADDING, y + 8, "Task Manager", theme.fg);

        // Tabs
        let tab_y = y + Self::HEADER_HEIGHT;
        for (i, tab) in TaskManagerTab::all().iter().enumerate() {
            let tab_x = x + i * 100;
            let is_active = *tab == self.current_tab;

            let tab_bg = if is_active {
                Color::new(50, 50, 53)
            } else {
                Color::new(37, 37, 38)
            };

            for py in 0..Self::TAB_HEIGHT {
                for px in 0..98 {
                    surface.set_pixel(tab_x + px, tab_y + py, tab_bg);
                }
            }

            // Tab label
            let label_color = if is_active { theme.fg } else { Color::new(150, 150, 150) };
            draw_string(surface, tab_x + 12, tab_y + 6, tab.label(), label_color);

            // Active indicator
            if is_active {
                let accent = Color::new(0, 122, 204);
                for px in 0..98 {
                    surface.set_pixel(tab_x + px, tab_y + Self::TAB_HEIGHT - 2, accent);
                }
            }
        }

        // Content
        match self.current_tab {
            TaskManagerTab::Processes => self.render_processes(surface, x, y, w, h),
            TaskManagerTab::Performance => self.render_performance(surface, x, y, w, h),
            TaskManagerTab::Details => self.render_details(surface, x, y, w, h),
        }

        // Footer
        let footer_y = y + h - Self::FOOTER_HEIGHT;
        let footer_bg = Color::new(37, 37, 38);
        for py in 0..Self::FOOTER_HEIGHT {
            for px in 0..w {
                surface.set_pixel(x + px, footer_y + py, footer_bg);
            }
        }

        // Process count
        let count_str = format_count(self.stats.process_count, self.stats.thread_count);
        draw_string(surface, x + Self::PADDING, footer_y + 12, &count_str, theme.fg);

        // End task button (processes tab only)
        if self.current_tab == TaskManagerTab::Processes && self.selected_pid.is_some() {
            let btn_x = x + w - 120;
            let btn_y = footer_y + 8;
            let btn_w = 100;
            let btn_h = 24;

            let btn_bg = Color::new(196, 43, 28);
            for py in 0..btn_h {
                for px in 0..btn_w {
                    surface.set_pixel(btn_x + px, btn_y + py, btn_bg);
                }
            }
            draw_string(surface, btn_x + 16, btn_y + 4, "End Task", Color::new(255, 255, 255));
        }
    }
}

impl TaskManager {
    fn render_processes(&self, surface: &mut Surface, x: usize, y: usize, w: usize, h: usize) {
        let theme = theme();
        let content_y = y + Self::HEADER_HEIGHT + Self::TAB_HEIGHT;

        // Column headers
        let header_bg = Color::new(37, 37, 38);
        for py in 0..Self::COLUMN_HEADER_HEIGHT {
            for px in 0..w {
                surface.set_pixel(x + px, content_y + py, header_bg);
            }
        }

        let header_fg = Color::new(180, 180, 180);
        let mut col_x = x + Self::PADDING;

        // Sort indicator
        let sort_ind = match self.sort_order {
            SortOrder::Ascending => "^",
            SortOrder::Descending => "v",
        };

        draw_string(surface, col_x, content_y + 4, "PID", header_fg);
        if self.sort_column == SortColumn::Pid {
            draw_string(surface, col_x + 32, content_y + 4, sort_ind, header_fg);
        }
        col_x += Self::COL_PID;

        draw_string(surface, col_x, content_y + 4, "Name", header_fg);
        if self.sort_column == SortColumn::Name {
            draw_string(surface, col_x + 40, content_y + 4, sort_ind, header_fg);
        }
        col_x += Self::COL_NAME;

        draw_string(surface, col_x, content_y + 4, "CPU %", header_fg);
        if self.sort_column == SortColumn::Cpu {
            draw_string(surface, col_x + 48, content_y + 4, sort_ind, header_fg);
        }
        col_x += Self::COL_CPU;

        draw_string(surface, col_x, content_y + 4, "Memory", header_fg);
        if self.sort_column == SortColumn::Memory {
            draw_string(surface, col_x + 56, content_y + 4, sort_ind, header_fg);
        }
        col_x += Self::COL_MEM;

        draw_string(surface, col_x, content_y + 4, "Status", header_fg);

        // Process list
        let list_y = content_y + Self::COLUMN_HEADER_HEIGHT;
        let visible = self.visible_rows();

        for (row_idx, proc) in self.processes.iter()
            .skip(self.scroll_offset)
            .take(visible)
            .enumerate()
        {
            let actual_idx = self.scroll_offset + row_idx;
            let row_y = list_y + row_idx * Self::ROW_HEIGHT;

            // Row background
            let is_selected = self.selected_pid == Some(proc.pid);
            let is_hovered = self.hover_row == Some(actual_idx);

            let row_bg = if is_selected {
                Color::new(0, 88, 156)
            } else if is_hovered {
                Color::new(50, 50, 53)
            } else if row_idx % 2 == 0 {
                Color::new(35, 35, 38)
            } else {
                Color::new(30, 30, 30)
            };

            for py in 0..Self::ROW_HEIGHT {
                for px in 0..w {
                    surface.set_pixel(x + px, row_y + py, row_bg);
                }
            }

            let fg = if is_selected {
                Color::new(255, 255, 255)
            } else {
                theme.fg
            };

            let mut col_x = x + Self::PADDING;

            // PID
            let pid_str = format_num(proc.pid as u64, "");
            draw_string(surface, col_x, row_y + 2, &pid_str, fg);
            col_x += Self::COL_PID;

            // Name
            let name_display: String = proc.name.chars().take(20).collect();
            draw_string(surface, col_x, row_y + 2, &name_display, fg);
            col_x += Self::COL_NAME;

            // CPU %
            let cpu_str = format_percent(proc.cpu_percent);
            let cpu_color = if proc.cpu_percent > 80.0 {
                Color::new(255, 100, 100)
            } else if proc.cpu_percent > 50.0 {
                Color::new(255, 200, 100)
            } else {
                fg
            };
            draw_string(surface, col_x, row_y + 2, &cpu_str, cpu_color);
            col_x += Self::COL_CPU;

            // Memory
            let mem_str = format_memory(proc.memory_kb);
            draw_string(surface, col_x, row_y + 2, &mem_str, fg);
            col_x += Self::COL_MEM;

            // Status
            let state_color = match proc.state {
                ProcessState::Running => Color::new(100, 200, 100),
                ProcessState::Sleeping => fg,
                ProcessState::Zombie => Color::new(255, 100, 100),
                _ => Color::new(150, 150, 150),
            };
            draw_string(surface, col_x, row_y + 2, proc.state.label(), state_color);
        }

        // Scrollbar
        if self.processes.len() > visible {
            let sb_x = x + w - 12;
            let sb_y = list_y;
            let sb_h = h - Self::HEADER_HEIGHT - Self::TAB_HEIGHT - Self::COLUMN_HEADER_HEIGHT - Self::FOOTER_HEIGHT;

            let track_color = Color::new(50, 50, 53);
            for py in 0..sb_h {
                for px in 0..8 {
                    surface.set_pixel(sb_x + px, sb_y + py, track_color);
                }
            }

            let total = self.processes.len() as f32;
            let vis = visible as f32;
            let thumb_h = ((vis / total) * sb_h as f32).max(20.0) as usize;
            let thumb_pos = ((self.scroll_offset as f32 / total) * sb_h as f32) as usize;

            let thumb_color = Color::new(100, 100, 100);
            for py in 0..thumb_h {
                for px in 0..8 {
                    let ty = sb_y + thumb_pos + py;
                    if ty < sb_y + sb_h {
                        surface.set_pixel(sb_x + px, ty, thumb_color);
                    }
                }
            }
        }
    }

    fn render_performance(&self, surface: &mut Surface, x: usize, y: usize, w: usize, h: usize) {
        let theme = theme();
        let content_y = y + Self::HEADER_HEIGHT + Self::TAB_HEIGHT + Self::PADDING;
        let content_h = h - Self::HEADER_HEIGHT - Self::TAB_HEIGHT - Self::FOOTER_HEIGHT - Self::PADDING * 2;

        // CPU section
        draw_string(surface, x + Self::PADDING, content_y, "CPU", theme.fg);

        let cpu_str = format_percent(self.stats.cpu_percent);
        draw_string(surface, x + Self::PADDING + 50, content_y, &cpu_str, theme.fg);

        // CPU graph
        let graph_x = x + Self::PADDING;
        let graph_y = content_y + 24;
        let graph_w = w / 2 - Self::PADDING * 2;
        let graph_h = (content_h - 80) / 2;

        self.render_graph(surface, graph_x, graph_y, graph_w, graph_h, &self.cpu_history, Color::new(17, 125, 187));

        // Memory section
        let mem_y = graph_y + graph_h + 30;
        draw_string(surface, x + Self::PADDING, mem_y, "Memory", theme.fg);

        let mem_pct = if self.stats.memory_total_kb > 0 {
            (self.stats.memory_used_kb as f32 / self.stats.memory_total_kb as f32) * 100.0
        } else {
            0.0
        };
        let mem_str = format_percent(mem_pct);
        draw_string(surface, x + Self::PADDING + 70, mem_y, &mem_str, theme.fg);

        // Memory graph
        let mem_graph_y = mem_y + 24;
        self.render_graph(surface, graph_x, mem_graph_y, graph_w, graph_h, &self.memory_history, Color::new(139, 18, 174));

        // Stats panel (right side)
        let stats_x = x + w / 2 + Self::PADDING;
        let label_color = Color::new(150, 150, 150);

        // CPU stats
        draw_string(surface, stats_x, content_y, "CPU Usage:", label_color);
        draw_string(surface, stats_x + 120, content_y, &cpu_str, theme.fg);

        draw_string(surface, stats_x, content_y + 24, "Cores:", label_color);
        let cores_str = format_num(self.stats.cpu_cores as u64, "");
        draw_string(surface, stats_x + 120, content_y + 24, &cores_str, theme.fg);

        // Memory stats
        draw_string(surface, stats_x, mem_y, "Total:", label_color);
        let total_str = format_memory(self.stats.memory_total_kb);
        draw_string(surface, stats_x + 120, mem_y, &total_str, theme.fg);

        draw_string(surface, stats_x, mem_y + 24, "Used:", label_color);
        let used_str = format_memory(self.stats.memory_used_kb);
        draw_string(surface, stats_x + 120, mem_y + 24, &used_str, theme.fg);

        draw_string(surface, stats_x, mem_y + 48, "Free:", label_color);
        let free_str = format_memory(self.stats.memory_free_kb);
        draw_string(surface, stats_x + 120, mem_y + 48, &free_str, theme.fg);

        draw_string(surface, stats_x, mem_y + 72, "Cached:", label_color);
        let cached_str = format_memory(self.stats.memory_cached_kb);
        draw_string(surface, stats_x + 120, mem_y + 72, &cached_str, theme.fg);

        // Uptime
        let uptime_y = mem_y + 120;
        draw_string(surface, stats_x, uptime_y, "Uptime:", label_color);
        let uptime_str = format_uptime(self.stats.uptime_secs);
        draw_string(surface, stats_x + 120, uptime_y, &uptime_str, theme.fg);
    }

    fn render_graph(&self, surface: &mut Surface, x: usize, y: usize, w: usize, h: usize, data: &[f32], color: Color) {
        // Graph background
        let bg = Color::new(20, 20, 24);
        for py in 0..h {
            for px in 0..w {
                surface.set_pixel(x + px, y + py, bg);
            }
        }

        // Border
        let border = Color::new(60, 60, 63);
        for px in 0..w {
            surface.set_pixel(x + px, y, border);
            surface.set_pixel(x + px, y + h - 1, border);
        }
        for py in 0..h {
            surface.set_pixel(x, y + py, border);
            surface.set_pixel(x + w - 1, y + py, border);
        }

        // Grid lines
        let grid = Color::new(40, 40, 44);
        for i in 1..4 {
            let gy = y + (h * i) / 4;
            for px in 0..w {
                surface.set_pixel(x + px, gy, grid);
            }
        }

        // Data line
        if data.len() >= 2 {
            let step = w as f32 / self.history_max as f32;

            for (i, window) in data.windows(2).enumerate() {
                let x1 = x + (i as f32 * step) as usize;
                let x2 = x + ((i + 1) as f32 * step) as usize;

                let y1 = y + h - 2 - ((window[0] / 100.0) * (h - 4) as f32) as usize;
                let y2 = y + h - 2 - ((window[1] / 100.0) * (h - 4) as f32) as usize;

                // Simple line drawing
                let dx = (x2 as isize - x1 as isize).abs();
                let dy = (y2 as isize - y1 as isize).abs();
                let steps = dx.max(dy).max(1) as usize;

                for s in 0..=steps {
                    let t = s as f32 / steps as f32;
                    let px = (x1 as f32 + t * (x2 as f32 - x1 as f32)) as usize;
                    let py = (y1 as f32 + t * (y2 as f32 - y1 as f32)) as usize;
                    if py >= y && py < y + h {
                        surface.set_pixel(px, py, color);
                        if py + 1 < y + h {
                            surface.set_pixel(px, py + 1, color);
                        }
                    }
                }
            }
        }
    }

    fn render_details(&self, surface: &mut Surface, x: usize, y: usize, _w: usize, _h: usize) {
        let theme = theme();
        let content_y = y + Self::HEADER_HEIGHT + Self::TAB_HEIGHT + Self::PADDING;

        draw_string(surface, x + Self::PADDING, content_y, "System Details", theme.fg);

        let label_color = Color::new(150, 150, 150);
        let value_color = theme.fg;

        let mut row = 1;
        let row_h = 24;

        draw_string(surface, x + Self::PADDING, content_y + row * row_h, "Processes:", label_color);
        draw_string(surface, x + 150, content_y + row * row_h, &format_num(self.stats.process_count as u64, ""), value_color);
        row += 1;

        draw_string(surface, x + Self::PADDING, content_y + row * row_h, "Threads:", label_color);
        draw_string(surface, x + 150, content_y + row * row_h, &format_num(self.stats.thread_count as u64, ""), value_color);
        row += 1;

        draw_string(surface, x + Self::PADDING, content_y + row * row_h, "CPU Cores:", label_color);
        draw_string(surface, x + 150, content_y + row * row_h, &format_num(self.stats.cpu_cores as u64, ""), value_color);
        row += 1;

        draw_string(surface, x + Self::PADDING, content_y + row * row_h, "Uptime:", label_color);
        draw_string(surface, x + 150, content_y + row * row_h, &format_uptime(self.stats.uptime_secs), value_color);
        row += 1;

        if self.stats.swap_total_kb > 0 {
            draw_string(surface, x + Self::PADDING, content_y + row * row_h, "Swap:", label_color);
            let swap_str = format_memory_pair(self.stats.swap_used_kb, self.stats.swap_total_kb);
            draw_string(surface, x + 150, content_y + row * row_h, &swap_str, value_color);
        }
    }
}

fn draw_string(surface: &mut Surface, x: usize, y: usize, s: &str, color: Color) {
    for (i, c) in s.chars().enumerate() {
        draw_char_simple(surface, x + i * 8, y, c, color);
    }
}

fn draw_char_simple(surface: &mut Surface, x: usize, y: usize, c: char, color: Color) {
    use crate::drivers::font::DEFAULT_FONT;

    if let Some(glyph) = DEFAULT_FONT.get_glyph(c) {
        for row in 0..DEFAULT_FONT.height {
            let byte = glyph[row];
            for col in 0..DEFAULT_FONT.width {
                if (byte >> (DEFAULT_FONT.width - 1 - col)) & 1 != 0 {
                    surface.set_pixel(x + col, y + row, color);
                }
            }
        }
    }
}

fn format_num(n: u64, suffix: &str) -> String {
    use alloc::string::ToString;
    let mut s = n.to_string();
    s.push_str(suffix);
    s
}

fn format_percent(p: f32) -> String {
    use alloc::string::ToString;
    let whole = p as u64;
    let frac = ((p - whole as f32) * 10.0) as u64;
    let mut s = whole.to_string();
    s.push('.');
    s.push_str(&frac.to_string());
    s.push('%');
    s
}

fn format_memory(kb: u64) -> String {
    use alloc::string::ToString;
    if kb < 1024 {
        let mut s = kb.to_string();
        s.push_str(" KB");
        s
    } else if kb < 1024 * 1024 {
        let mb = kb / 1024;
        let mut s = mb.to_string();
        s.push_str(" MB");
        s
    } else {
        let gb = kb / (1024 * 1024);
        let frac = (kb % (1024 * 1024)) / (1024 * 100);
        let mut s = gb.to_string();
        s.push('.');
        s.push_str(&frac.to_string());
        s.push_str(" GB");
        s
    }
}

fn format_memory_pair(used: u64, total: u64) -> String {
    let mut s = format_memory(used);
    s.push_str(" / ");
    s.push_str(&format_memory(total));
    s
}

fn format_uptime(secs: u64) -> String {
    use alloc::string::ToString;
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;

    let mut s = String::new();
    if days > 0 {
        s.push_str(&days.to_string());
        s.push_str("d ");
    }
    s.push_str(&hours.to_string());
    s.push_str("h ");
    s.push_str(&mins.to_string());
    s.push('m');
    s
}

fn format_count(procs: usize, threads: usize) -> String {
    use alloc::string::ToString;
    let mut s = procs.to_string();
    s.push_str(" processes, ");
    s.push_str(&threads.to_string());
    s.push_str(" threads");
    s
}
