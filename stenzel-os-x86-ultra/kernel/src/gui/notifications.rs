//! Notification System
//!
//! Toast-style notifications that appear in the corner of the screen.
//! Features:
//! - Multiple notification types (info, success, warning, error)
//! - Auto-dismiss with configurable timeout
//! - Click actions
//! - Notification queue and history
//! - Animation for show/hide

use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;

use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use crate::gui::widgets::{Widget, WidgetId, WidgetEvent, Bounds, MouseButton};

/// Notification ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NotificationId(u64);

impl NotificationId {
    /// Create a new unique notification ID
    pub fn new() -> Self {
        static NEXT_ID: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(1);
        NotificationId(NEXT_ID.fetch_add(1, core::sync::atomic::Ordering::Relaxed))
    }

    /// Get the raw ID
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

/// Notification priority/type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationType {
    /// Information message (blue)
    Info,
    /// Success message (green)
    Success,
    /// Warning message (yellow/orange)
    Warning,
    /// Error message (red)
    Error,
}

impl NotificationType {
    /// Get the accent color for this notification type
    pub fn color(&self) -> Color {
        match self {
            NotificationType::Info => Color::new(60, 140, 220),
            NotificationType::Success => Color::new(60, 180, 80),
            NotificationType::Warning => Color::new(230, 160, 30),
            NotificationType::Error => Color::new(220, 60, 60),
        }
    }

    /// Get the icon character for this type
    pub fn icon_char(&self) -> char {
        match self {
            NotificationType::Info => 'i',
            NotificationType::Success => '!',
            NotificationType::Warning => '!',
            NotificationType::Error => 'X',
        }
    }
}

/// Action callback type
pub type ActionCallback = fn(NotificationId, usize); // notification id, action index

/// Notification action (button)
#[derive(Clone)]
pub struct NotificationAction {
    /// Action label
    pub label: String,
    /// Whether this is a primary action
    pub primary: bool,
}

impl NotificationAction {
    pub fn new(label: &str, primary: bool) -> Self {
        Self {
            label: String::from(label),
            primary,
        }
    }
}

/// A single notification
pub struct Notification {
    /// Unique ID
    pub id: NotificationId,
    /// Notification type
    pub notification_type: NotificationType,
    /// Title (short, bold)
    pub title: String,
    /// Body text (can be multi-line)
    pub body: String,
    /// App name (shown small)
    pub app_name: Option<String>,
    /// Actions (buttons)
    pub actions: Vec<NotificationAction>,
    /// Action callback
    pub on_action: Option<ActionCallback>,
    /// Click callback (whole notification)
    pub on_click: Option<fn(NotificationId)>,
    /// Time to live in ticks (None = persistent)
    pub ttl: Option<u64>,
    /// Creation time (tick count)
    pub created_at: u64,
    /// Whether to show close button
    pub closable: bool,
}

impl Notification {
    /// Create a new notification
    pub fn new(notification_type: NotificationType, title: &str, body: &str) -> Self {
        Self {
            id: NotificationId::new(),
            notification_type,
            title: String::from(title),
            body: String::from(body),
            app_name: None,
            actions: Vec::new(),
            on_action: None,
            on_click: None,
            ttl: Some(300), // ~5 seconds at 60fps
            created_at: 0,
            closable: true,
        }
    }

    /// Create an info notification
    pub fn info(title: &str, body: &str) -> Self {
        Self::new(NotificationType::Info, title, body)
    }

    /// Create a success notification
    pub fn success(title: &str, body: &str) -> Self {
        Self::new(NotificationType::Success, title, body)
    }

    /// Create a warning notification
    pub fn warning(title: &str, body: &str) -> Self {
        Self::new(NotificationType::Warning, title, body)
    }

    /// Create an error notification
    pub fn error(title: &str, body: &str) -> Self {
        Self::new(NotificationType::Error, title, body)
    }

    /// Set app name
    pub fn with_app_name(mut self, name: &str) -> Self {
        self.app_name = Some(String::from(name));
        self
    }

    /// Add an action
    pub fn with_action(mut self, label: &str, primary: bool) -> Self {
        self.actions.push(NotificationAction::new(label, primary));
        self
    }

    /// Set action callback
    pub fn with_action_callback(mut self, callback: ActionCallback) -> Self {
        self.on_action = Some(callback);
        self
    }

    /// Set click callback
    pub fn with_click_callback(mut self, callback: fn(NotificationId)) -> Self {
        self.on_click = Some(callback);
        self
    }

    /// Set TTL (in frames/ticks)
    pub fn with_ttl(mut self, ttl: Option<u64>) -> Self {
        self.ttl = ttl;
        self
    }

    /// Make persistent (no auto-dismiss)
    pub fn persistent(mut self) -> Self {
        self.ttl = None;
        self
    }

    /// Set closable
    pub fn with_closable(mut self, closable: bool) -> Self {
        self.closable = closable;
        self
    }
}

/// Animation state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnimState {
    /// Sliding in from right
    SlideIn(u8),
    /// Fully visible
    Visible,
    /// Sliding out to right
    SlideOut(u8),
    /// Hidden (ready to remove)
    Hidden,
}

/// Active notification (currently displayed)
struct ActiveNotification {
    notification: Notification,
    /// Current Y position
    y: usize,
    /// Target Y position
    target_y: usize,
    /// Animation state
    anim_state: AnimState,
    /// Hovered action index
    hovered_action: Option<usize>,
    /// Is close button hovered
    close_hovered: bool,
}

/// Notification Toast Manager
pub struct NotificationManager {
    id: WidgetId,
    bounds: Bounds,

    /// Currently displayed notifications
    active: Vec<ActiveNotification>,

    /// Queue of pending notifications
    queue: VecDeque<Notification>,

    /// Maximum visible notifications
    max_visible: usize,

    /// Notification width
    notification_width: usize,

    /// Spacing between notifications
    spacing: usize,

    /// Current tick counter
    tick: u64,

    /// Background color
    bg_color: Color,

    /// Text color
    text_color: Color,

    /// Secondary text color
    secondary_color: Color,

    /// Whether manager is visible
    visible: bool,

    /// Whether manager needs redraw
    dirty: bool,
}

impl NotificationManager {
    /// Width of a notification
    pub const NOTIFICATION_WIDTH: usize = 320;
    /// Minimum height
    pub const MIN_HEIGHT: usize = 80;
    /// Maximum height
    pub const MAX_HEIGHT: usize = 160;
    /// Padding
    pub const PADDING: usize = 12;
    /// Action button height
    pub const ACTION_HEIGHT: usize = 28;
    /// Close button size
    pub const CLOSE_SIZE: usize = 16;
    /// Animation frames
    pub const ANIM_FRAMES: u8 = 10;

    /// Create a new notification manager
    pub fn new(screen_width: usize, screen_height: usize) -> Self {
        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(
                (screen_width - Self::NOTIFICATION_WIDTH - 16) as isize,
                16,
                Self::NOTIFICATION_WIDTH + 16,
                screen_height - 80, // Leave room for taskbar
            ),
            active: Vec::new(),
            queue: VecDeque::new(),
            max_visible: 5,
            notification_width: Self::NOTIFICATION_WIDTH,
            spacing: 8,
            tick: 0,
            bg_color: Color::new(40, 40, 50),
            text_color: Color::WHITE,
            secondary_color: Color::new(160, 160, 170),
            visible: true,
            dirty: true,
        }
    }

    /// Push a notification
    pub fn notify(&mut self, notification: Notification) -> NotificationId {
        let id = notification.id;

        if self.active.len() < self.max_visible {
            self.show_notification(notification);
        } else {
            self.queue.push_back(notification);
        }

        self.dirty = true;
        id
    }

    /// Show a notification immediately
    fn show_notification(&mut self, mut notification: Notification) {
        notification.created_at = self.tick;

        // Calculate Y position
        let mut y = 0;
        for n in &self.active {
            y = y.max(n.target_y + self.calculate_height(&n.notification) + self.spacing);
        }

        self.active.push(ActiveNotification {
            notification,
            y,
            target_y: y,
            anim_state: AnimState::SlideIn(0),
            hovered_action: None,
            close_hovered: false,
        });
    }

    /// Calculate notification height
    fn calculate_height(&self, notification: &Notification) -> usize {
        let mut height = Self::PADDING * 2; // Top and bottom padding
        height += 20; // Title height
        if notification.app_name.is_some() {
            height += 16; // App name
        }

        // Body lines (estimate)
        let body_lines = (notification.body.len() / 40).max(1).min(4);
        height += body_lines * 18;

        // Actions
        if !notification.actions.is_empty() {
            height += Self::ACTION_HEIGHT + 8;
        }

        height.min(Self::MAX_HEIGHT).max(Self::MIN_HEIGHT)
    }

    /// Dismiss a notification
    pub fn dismiss(&mut self, id: NotificationId) {
        for n in &mut self.active {
            if n.notification.id == id {
                n.anim_state = AnimState::SlideOut(0);
                self.dirty = true;
                return;
            }
        }
    }

    /// Clear all notifications
    pub fn clear_all(&mut self) {
        for n in &mut self.active {
            n.anim_state = AnimState::SlideOut(0);
        }
        self.queue.clear();
        self.dirty = true;
    }

    /// Update (call every frame)
    pub fn update(&mut self) {
        self.tick += 1;

        let mut positions_changed = false;
        let mut removals = Vec::new();

        // Update animations and check timeouts
        for (i, n) in self.active.iter_mut().enumerate() {
            match n.anim_state {
                AnimState::SlideIn(frame) => {
                    if frame < Self::ANIM_FRAMES {
                        n.anim_state = AnimState::SlideIn(frame + 1);
                        self.dirty = true;
                    } else {
                        n.anim_state = AnimState::Visible;
                    }
                }
                AnimState::Visible => {
                    // Check TTL
                    if let Some(ttl) = n.notification.ttl {
                        if self.tick - n.notification.created_at > ttl {
                            n.anim_state = AnimState::SlideOut(0);
                            self.dirty = true;
                        }
                    }
                }
                AnimState::SlideOut(frame) => {
                    if frame < Self::ANIM_FRAMES {
                        n.anim_state = AnimState::SlideOut(frame + 1);
                        self.dirty = true;
                    } else {
                        n.anim_state = AnimState::Hidden;
                        removals.push(i);
                        positions_changed = true;
                    }
                }
                AnimState::Hidden => {}
            }

            // Animate Y position
            if n.y != n.target_y {
                if n.y < n.target_y {
                    n.y = (n.y + 4).min(n.target_y);
                } else {
                    n.y = n.y.saturating_sub(4).max(n.target_y);
                }
                self.dirty = true;
            }
        }

        // Remove hidden notifications
        for i in removals.into_iter().rev() {
            self.active.remove(i);
        }

        // Recalculate positions if needed
        if positions_changed {
            self.recalculate_positions();

            // Show queued notifications
            while self.active.len() < self.max_visible {
                if let Some(notification) = self.queue.pop_front() {
                    self.show_notification(notification);
                } else {
                    break;
                }
            }
        }
    }

    /// Recalculate notification positions
    fn recalculate_positions(&mut self) {
        // Pre-calculate heights to avoid borrow issues
        let heights_and_visible: Vec<(usize, bool)> = self.active.iter()
            .map(|n| (self.calculate_height(&n.notification), n.anim_state != AnimState::Hidden))
            .collect();

        let mut y = 0;
        for (i, n) in self.active.iter_mut().enumerate() {
            let (height, visible) = heights_and_visible[i];
            if visible {
                n.target_y = y;
                y += height + self.spacing;
            }
        }
    }

    /// Get notification at position
    fn notification_at(&self, px: usize, py: usize) -> Option<(usize, bool, Option<usize>)> {
        for (i, n) in self.active.iter().enumerate() {
            if n.anim_state == AnimState::Hidden {
                continue;
            }

            let height = self.calculate_height(&n.notification);
            let x_offset = self.get_x_offset(&n.anim_state);

            if px >= x_offset && py >= n.y && py < n.y + height {
                // Check if on close button
                if n.notification.closable {
                    let close_x = x_offset + self.notification_width - Self::CLOSE_SIZE - Self::PADDING;
                    let close_y = n.y + Self::PADDING;
                    if px >= close_x && px < close_x + Self::CLOSE_SIZE &&
                       py >= close_y && py < close_y + Self::CLOSE_SIZE {
                        return Some((i, true, None));
                    }
                }

                // Check actions
                if !n.notification.actions.is_empty() {
                    let actions_y = n.y + height - Self::PADDING - Self::ACTION_HEIGHT;
                    if py >= actions_y && py < actions_y + Self::ACTION_HEIGHT {
                        let mut ax = x_offset + Self::PADDING;
                        for (ai, action) in n.notification.actions.iter().enumerate() {
                            let action_width = action.label.len() * 8 + 24;
                            if px >= ax && px < ax + action_width {
                                return Some((i, false, Some(ai)));
                            }
                            ax += action_width + 8;
                        }
                    }
                }

                return Some((i, false, None));
            }
        }
        None
    }

    /// Get X offset based on animation state
    fn get_x_offset(&self, state: &AnimState) -> usize {
        match state {
            AnimState::SlideIn(frame) => {
                let progress = *frame as usize * self.notification_width / Self::ANIM_FRAMES as usize;
                self.notification_width.saturating_sub(progress)
            }
            AnimState::SlideOut(frame) => {
                *frame as usize * self.notification_width / Self::ANIM_FRAMES as usize
            }
            _ => 0,
        }
    }

    /// Check if dirty
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Clear dirty flag
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    /// Get number of notifications (active + queued)
    pub fn count(&self) -> usize {
        self.active.len() + self.queue.len()
    }

    /// Draw a single notification
    fn draw_notification(&self, surface: &mut Surface, n: &ActiveNotification) {
        let height = self.calculate_height(&n.notification);
        let x_offset = self.get_x_offset(&n.anim_state);

        let base_x = self.bounds.x as usize + x_offset;
        let base_y = self.bounds.y as usize + n.y;

        // Background with rounded corners (simplified)
        for py in 0..height {
            for px in 0..self.notification_width {
                // Simple corner rounding
                let corner_radius = 6;
                let in_corner = (py < corner_radius && px < corner_radius) ||
                               (py < corner_radius && px >= self.notification_width - corner_radius) ||
                               (py >= height - corner_radius && px < corner_radius) ||
                               (py >= height - corner_radius && px >= self.notification_width - corner_radius);

                if !in_corner || {
                    let dx = if px < corner_radius { corner_radius - px } else { px - (self.notification_width - corner_radius) };
                    let dy = if py < corner_radius { corner_radius - py } else { py - (height - corner_radius) };
                    dx * dx + dy * dy <= corner_radius * corner_radius
                } {
                    surface.set_pixel(base_x + px, base_y + py, self.bg_color);
                }
            }
        }

        // Left accent bar
        let accent_color = n.notification.notification_type.color();
        for py in 4..(height - 4) {
            for px in 0..4 {
                surface.set_pixel(base_x + px, base_y + py, accent_color);
            }
        }

        // Icon
        let icon_x = base_x + Self::PADDING;
        let icon_y = base_y + Self::PADDING;
        self.draw_type_icon(surface, icon_x, icon_y, n.notification.notification_type);

        // Close button
        if n.notification.closable {
            let close_x = base_x + self.notification_width - Self::CLOSE_SIZE - Self::PADDING;
            let close_y = base_y + Self::PADDING;
            let close_color = if n.close_hovered {
                Color::new(255, 100, 100)
            } else {
                self.secondary_color
            };

            // X mark
            for i in 0..Self::CLOSE_SIZE {
                surface.set_pixel(close_x + i, close_y + i, close_color);
                surface.set_pixel(close_x + Self::CLOSE_SIZE - 1 - i, close_y + i, close_color);
            }
        }

        // App name (if present)
        let mut text_y = icon_y;
        if let Some(ref app_name) = n.notification.app_name {
            self.draw_string(surface, icon_x + 24, text_y, app_name, self.secondary_color);
            text_y += 16;
        }

        // Title
        self.draw_string(surface, icon_x + 24, text_y, &n.notification.title, self.text_color);
        text_y += 20;

        // Body (simplified, single line with ellipsis if too long)
        let max_body_width = self.notification_width - Self::PADDING * 2 - 24;
        let max_chars = max_body_width / 8;
        let body_display = if n.notification.body.len() > max_chars {
            let mut s = String::from(&n.notification.body[..max_chars.saturating_sub(3)]);
            s.push_str("...");
            s
        } else {
            n.notification.body.clone()
        };
        self.draw_string(surface, icon_x, text_y, &body_display, self.secondary_color);

        // Actions
        if !n.notification.actions.is_empty() {
            let actions_y = base_y + height - Self::PADDING - Self::ACTION_HEIGHT;
            let mut ax = base_x + Self::PADDING;

            for (i, action) in n.notification.actions.iter().enumerate() {
                let action_width = action.label.len() * 8 + 24;
                let is_hovered = n.hovered_action == Some(i);

                // Button background
                let btn_bg = if is_hovered {
                    Color::new(80, 80, 90)
                } else if action.primary {
                    accent_color
                } else {
                    Color::new(60, 60, 70)
                };

                for py in 0..Self::ACTION_HEIGHT {
                    for px in 0..action_width {
                        surface.set_pixel(ax + px, actions_y + py, btn_bg);
                    }
                }

                // Button text
                let text_color = if action.primary && !is_hovered {
                    Color::new(30, 30, 30)
                } else {
                    Color::WHITE
                };
                self.draw_string(surface, ax + 12, actions_y + 6, &action.label, text_color);

                ax += action_width + 8;
            }
        }
    }

    /// Draw type icon
    fn draw_type_icon(&self, surface: &mut Surface, x: usize, y: usize, t: NotificationType) {
        let color = t.color();

        // Draw a circle
        for py in 0..16 {
            for px in 0..16 {
                let dx = px as isize - 8;
                let dy = py as isize - 8;
                if dx * dx + dy * dy <= 64 {
                    surface.set_pixel(x + px, y + py, color);
                }
            }
        }

        // Draw icon character (simplified)
        let icon_color = Color::WHITE;
        match t {
            NotificationType::Info => {
                // "i" - dot and line
                surface.set_pixel(x + 8, y + 4, icon_color);
                for py in 7..12 {
                    surface.set_pixel(x + 8, y + py, icon_color);
                }
            }
            NotificationType::Success => {
                // Checkmark
                for i in 0..3 {
                    surface.set_pixel(x + 5 + i, y + 8 + i, icon_color);
                }
                for i in 0..5 {
                    surface.set_pixel(x + 7 + i, y + 10 - i, icon_color);
                }
            }
            NotificationType::Warning => {
                // "!"
                for py in 4..9 {
                    surface.set_pixel(x + 8, y + py, icon_color);
                }
                surface.set_pixel(x + 8, y + 11, icon_color);
            }
            NotificationType::Error => {
                // "X"
                for i in 0..8 {
                    surface.set_pixel(x + 4 + i, y + 4 + i, icon_color);
                    surface.set_pixel(x + 11 - i, y + 4 + i, icon_color);
                }
            }
        }
    }

    /// Draw string (simplified)
    fn draw_string(&self, surface: &mut Surface, x: usize, y: usize, s: &str, color: Color) {
        for (i, c) in s.chars().enumerate() {
            self.draw_char(surface, x + i * 8, y, c, color);
        }
    }

    /// Draw a character (very simplified bitmap font)
    fn draw_char(&self, surface: &mut Surface, x: usize, y: usize, c: char, color: Color) {
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
}

impl Widget for NotificationManager {
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
        self.dirty = true;
    }

    fn handle_event(&mut self, event: &WidgetEvent) -> bool {
        if !self.visible || self.active.is_empty() {
            return false;
        }

        match event {
            WidgetEvent::MouseMove { x, y, .. } => {
                let px = (*x as isize - self.bounds.x) as usize;
                let py = (*y as isize - self.bounds.y) as usize;

                // Clear all hover states
                for n in &mut self.active {
                    n.hovered_action = None;
                    n.close_hovered = false;
                }

                if let Some((idx, is_close, action_idx)) = self.notification_at(px, py) {
                    if is_close {
                        self.active[idx].close_hovered = true;
                    } else {
                        self.active[idx].hovered_action = action_idx;
                    }
                    self.dirty = true;
                    return true;
                }
                false
            }
            WidgetEvent::MouseDown { button: MouseButton::Left, x, y } => {
                let px = (*x as isize - self.bounds.x) as usize;
                let py = (*y as isize - self.bounds.y) as usize;

                if let Some((idx, is_close, action_idx)) = self.notification_at(px, py) {
                    let n = &self.active[idx];

                    if is_close {
                        // Close the notification
                        let id = n.notification.id;
                        self.dismiss(id);
                        return true;
                    }

                    if let Some(ai) = action_idx {
                        // Action clicked
                        if let Some(callback) = n.notification.on_action {
                            callback(n.notification.id, ai);
                        }
                        // Dismiss after action
                        let id = n.notification.id;
                        self.dismiss(id);
                        return true;
                    }

                    // General click
                    if let Some(callback) = n.notification.on_click {
                        callback(n.notification.id);
                    }
                    return true;
                }
                false
            }
            _ => false,
        }
    }

    fn render(&self, surface: &mut Surface) {
        if !self.visible {
            return;
        }

        for n in &self.active {
            if n.anim_state != AnimState::Hidden {
                self.draw_notification(surface, n);
            }
        }
    }
}

/// Global notification manager
static NOTIFICATION_MANAGER: Mutex<Option<NotificationManager>> = Mutex::new(None);

/// Initialize the notification system
pub fn init(screen_width: usize, screen_height: usize) {
    let manager = NotificationManager::new(screen_width, screen_height);
    *NOTIFICATION_MANAGER.lock() = Some(manager);
    crate::kprintln!("notifications: initialized");
}

/// Show a notification
pub fn notify(notification: Notification) -> NotificationId {
    let mut manager = NOTIFICATION_MANAGER.lock();
    if let Some(ref mut m) = *manager {
        m.notify(notification)
    } else {
        NotificationId::new() // Return dummy ID if not initialized
    }
}

/// Show a simple info notification
pub fn info(title: &str, body: &str) -> NotificationId {
    notify(Notification::info(title, body))
}

/// Show a simple success notification
pub fn success(title: &str, body: &str) -> NotificationId {
    notify(Notification::success(title, body))
}

/// Show a simple warning notification
pub fn warning(title: &str, body: &str) -> NotificationId {
    notify(Notification::warning(title, body))
}

/// Show a simple error notification
pub fn error(title: &str, body: &str) -> NotificationId {
    notify(Notification::error(title, body))
}

/// Dismiss a notification
pub fn dismiss(id: NotificationId) {
    let mut manager = NOTIFICATION_MANAGER.lock();
    if let Some(ref mut m) = *manager {
        m.dismiss(id);
    }
}

/// Clear all notifications
pub fn clear_all() {
    let mut manager = NOTIFICATION_MANAGER.lock();
    if let Some(ref mut m) = *manager {
        m.clear_all();
    }
}

/// Update the notification manager (call every frame)
pub fn update() {
    let mut manager = NOTIFICATION_MANAGER.lock();
    if let Some(ref mut m) = *manager {
        m.update();
    }
}

/// Execute with notification manager access
pub fn with_manager<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut NotificationManager) -> R,
{
    let mut manager = NOTIFICATION_MANAGER.lock();
    manager.as_mut().map(f)
}

/// Check if notification manager is available
pub fn is_available() -> bool {
    NOTIFICATION_MANAGER.lock().is_some()
}

/// Get notification count
pub fn count() -> usize {
    let manager = NOTIFICATION_MANAGER.lock();
    manager.as_ref().map(|m| m.count()).unwrap_or(0)
}
