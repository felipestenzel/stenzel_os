//! Login Screen
//!
//! Full-screen login interface for user authentication.

use alloc::string::String;
use alloc::vec::Vec;
use crate::drivers::framebuffer::Color;
use crate::gui::surface::Surface;
use crate::gui::widgets::{Widget, WidgetId, WidgetEvent, Bounds, MouseButton};

/// User entry for user selection
#[derive(Debug, Clone)]
pub struct UserEntry {
    pub uid: u32,
    pub username: String,
    pub display_name: String,
    pub has_password: bool,
}

impl UserEntry {
    pub fn new(uid: u32, username: &str, display_name: &str) -> Self {
        Self {
            uid,
            username: String::from(username),
            display_name: String::from(display_name),
            has_password: true,
        }
    }
}

/// Login state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginState {
    UserSelect,
    PasswordEntry,
    Authenticating,
    LoggedIn,
}

/// Login callback - returns true if authentication succeeds
pub type LoginCallback = fn(&str, &str) -> bool; // username, password -> success

/// Shutdown callback
pub type PowerCallback = fn();

/// Login Screen widget
pub struct LoginScreen {
    id: WidgetId,
    bounds: Bounds,

    // State
    state: LoginState,
    visible: bool,
    focused: bool,

    // Users
    users: Vec<UserEntry>,
    selected_user: usize,

    // Time/date
    time_str: String,
    date_str: String,

    // Password input
    password: String,
    error_message: Option<String>,
    input_focused: bool,
    shake_animation: u8,

    // Callbacks
    on_login: Option<LoginCallback>,
    on_shutdown: Option<PowerCallback>,
    on_restart: Option<PowerCallback>,
}

impl LoginScreen {
    const CHAR_WIDTH: usize = 8;
    const CHAR_HEIGHT: usize = 16;
    const INPUT_WIDTH: usize = 280;
    const INPUT_HEIGHT: usize = 40;
    const USER_CARD_SIZE: usize = 120;
    const USER_SPACING: usize = 20;

    /// Create a new login screen
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            id: WidgetId::new(),
            bounds: Bounds::new(0, 0, width, height),
            state: LoginState::UserSelect,
            visible: true,
            focused: true,
            users: Vec::new(),
            selected_user: 0,
            time_str: String::from("00:00"),
            date_str: String::from("Monday, January 1"),
            password: String::new(),
            error_message: None,
            input_focused: true,
            shake_animation: 0,
            on_login: None,
            on_shutdown: None,
            on_restart: None,
        }
    }

    /// Set login callback
    pub fn set_on_login(&mut self, callback: LoginCallback) {
        self.on_login = Some(callback);
    }

    /// Set shutdown callback
    pub fn set_on_shutdown(&mut self, callback: PowerCallback) {
        self.on_shutdown = Some(callback);
    }

    /// Set restart callback
    pub fn set_on_restart(&mut self, callback: PowerCallback) {
        self.on_restart = Some(callback);
    }

    /// Set available users
    pub fn set_users(&mut self, users: Vec<UserEntry>) {
        self.users = users;
        self.selected_user = 0;
    }

    /// Add a user
    pub fn add_user(&mut self, user: UserEntry) {
        self.users.push(user);
    }

    /// Update time
    pub fn set_time(&mut self, hours: u8, minutes: u8) {
        self.time_str = format_time(hours, minutes);
    }

    /// Update date
    pub fn set_date(&mut self, weekday: &str, month: &str, day: u8) {
        self.date_str = format_date(weekday, month, day);
    }

    /// Is logged in
    pub fn is_logged_in(&self) -> bool {
        self.state == LoginState::LoggedIn
    }

    /// Get selected user
    pub fn selected_user(&self) -> Option<&UserEntry> {
        self.users.get(self.selected_user)
    }

    /// Select a user
    pub fn select_user(&mut self, index: usize) {
        if index < self.users.len() {
            self.selected_user = index;
            self.password.clear();
            self.error_message = None;

            // If user has no password, go directly to password entry
            // (which will succeed on empty password)
            if !self.users[index].has_password {
                self.state = LoginState::PasswordEntry;
            }
        }
    }

    /// Confirm user selection
    fn confirm_user(&mut self) {
        if self.users.is_empty() {
            return;
        }
        self.state = LoginState::PasswordEntry;
        self.password.clear();
        self.error_message = None;
        self.input_focused = true;
    }

    /// Go back to user selection
    fn back_to_user_select(&mut self) {
        self.state = LoginState::UserSelect;
        self.password.clear();
        self.error_message = None;
    }

    /// Attempt login
    fn attempt_login(&mut self) {
        if self.users.is_empty() {
            return;
        }

        let user = &self.users[self.selected_user];
        let username = user.username.clone();

        self.state = LoginState::Authenticating;

        let success = if let Some(callback) = self.on_login {
            callback(&username, &self.password)
        } else {
            // No callback - allow login (for testing)
            true
        };

        if success {
            self.state = LoginState::LoggedIn;
            self.visible = false;
            self.password.clear();
        } else {
            self.state = LoginState::PasswordEntry;
            self.password.clear();
            self.error_message = Some(String::from("Incorrect password"));
            self.shake_animation = 10;
        }
    }

    /// Handle key input
    fn handle_key(&mut self, scancode: u8) -> bool {
        match self.state {
            LoginState::UserSelect => {
                match scancode {
                    0x4B => { // Left
                        if self.selected_user > 0 {
                            self.selected_user -= 1;
                        }
                        true
                    }
                    0x4D => { // Right
                        if self.selected_user + 1 < self.users.len() {
                            self.selected_user += 1;
                        }
                        true
                    }
                    0x1C => { // Enter
                        self.confirm_user();
                        true
                    }
                    _ => false,
                }
            }
            LoginState::PasswordEntry => {
                match scancode {
                    0x1C => { // Enter
                        self.attempt_login();
                        true
                    }
                    0x0E => { // Backspace
                        self.password.pop();
                        self.error_message = None;
                        true
                    }
                    0x01 => { // Escape
                        self.back_to_user_select();
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    /// Get user card position
    fn user_card_position(&self, index: usize) -> (usize, usize) {
        let total_width = self.users.len() * (Self::USER_CARD_SIZE + Self::USER_SPACING) - Self::USER_SPACING;
        let start_x = (self.bounds.width / 2).saturating_sub(total_width / 2);
        let card_y = self.bounds.height / 2 - Self::USER_CARD_SIZE;

        let card_x = start_x + index * (Self::USER_CARD_SIZE + Self::USER_SPACING);
        (card_x, card_y)
    }

    /// Get user at point
    fn user_at_point(&self, x: isize, y: isize) -> Option<usize> {
        let lx = x as usize;
        let ly = y as usize;

        for i in 0..self.users.len() {
            let (cx, cy) = self.user_card_position(i);
            if lx >= cx && lx < cx + Self::USER_CARD_SIZE
                && ly >= cy && ly < cy + Self::USER_CARD_SIZE
            {
                return Some(i);
            }
        }
        None
    }
}

impl Widget for LoginScreen {
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
        if !self.visible {
            return false;
        }

        match event {
            WidgetEvent::Focus => {
                self.focused = true;
                true
            }
            WidgetEvent::Blur => {
                self.focused = false;
                true
            }
            WidgetEvent::KeyDown { key, .. } => {
                self.handle_key(*key)
            }
            WidgetEvent::Character { c } => {
                if self.state == LoginState::PasswordEntry && self.input_focused {
                    if *c >= ' ' && *c != '\x7f' {
                        if self.password.len() < 64 {
                            self.password.push(*c);
                            self.error_message = None;
                        }
                        return true;
                    }
                }
                false
            }
            WidgetEvent::MouseDown { button: MouseButton::Left, x, y } => {
                match self.state {
                    LoginState::UserSelect => {
                        if let Some(idx) = self.user_at_point(*x, *y) {
                            self.selected_user = idx;
                            self.confirm_user();
                            return true;
                        }
                    }
                    LoginState::PasswordEntry => {
                        self.input_focused = true;
                        return true;
                    }
                    _ => {}
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

        let w = self.bounds.width;
        let h = self.bounds.height;

        // Background gradient
        for py in 0..h {
            let t = py as f32 / h as f32;
            let r = (20.0 + t * 10.0) as u8;
            let g = (25.0 + t * 15.0) as u8;
            let b = (40.0 + t * 20.0) as u8;
            for px in 0..w {
                surface.set_pixel(px, py, Color::new(r, g, b));
            }
        }

        let center_x = w / 2;

        // Time at top
        let time_y = 50;
        draw_string_scaled(surface, center_x.saturating_sub(self.time_str.len() * 8 * 3 / 2), time_y, &self.time_str, Color::new(255, 255, 255), 3);

        // Date
        let date_y = time_y + 50;
        draw_string_centered(surface, center_x, date_y, &self.date_str, Color::new(180, 180, 180));

        match self.state {
            LoginState::UserSelect => {
                // User cards
                for (i, user) in self.users.iter().enumerate() {
                    let (card_x, card_y) = self.user_card_position(i);
                    let is_selected = i == self.selected_user;

                    // Card background
                    let card_bg = if is_selected {
                        Color::new(60, 60, 80)
                    } else {
                        Color::new(40, 40, 55)
                    };

                    for py in 0..Self::USER_CARD_SIZE {
                        for px in 0..Self::USER_CARD_SIZE {
                            surface.set_pixel(card_x + px, card_y + py, card_bg);
                        }
                    }

                    // Selection border
                    if is_selected {
                        let border = Color::new(0, 122, 204);
                        for px in 0..Self::USER_CARD_SIZE {
                            surface.set_pixel(card_x + px, card_y, border);
                            surface.set_pixel(card_x + px, card_y + Self::USER_CARD_SIZE - 1, border);
                        }
                        for py in 0..Self::USER_CARD_SIZE {
                            surface.set_pixel(card_x, card_y + py, border);
                            surface.set_pixel(card_x + Self::USER_CARD_SIZE - 1, card_y + py, border);
                        }
                    }

                    // Avatar circle
                    let avatar_center_x = card_x + Self::USER_CARD_SIZE / 2;
                    let avatar_center_y = card_y + 40;
                    let avatar_radius = 24;

                    let avatar_bg = Color::new(80, 80, 100);
                    for py in 0..avatar_radius * 2 {
                        for px in 0..avatar_radius * 2 {
                            let dx = px as isize - avatar_radius as isize;
                            let dy = py as isize - avatar_radius as isize;
                            if dx * dx + dy * dy <= (avatar_radius as isize * avatar_radius as isize) {
                                surface.set_pixel(avatar_center_x - avatar_radius + px, avatar_center_y - avatar_radius + py, avatar_bg);
                            }
                        }
                    }

                    // User icon
                    let icon_color = Color::new(150, 150, 170);
                    let head_r = 8;
                    for py in 0..head_r * 2 {
                        for px in 0..head_r * 2 {
                            let dx = px as isize - head_r as isize;
                            let dy = py as isize - head_r as isize;
                            if dx * dx + dy * dy <= (head_r as isize * head_r as isize) {
                                surface.set_pixel(avatar_center_x - head_r + px, avatar_center_y - 10 + py, icon_color);
                            }
                        }
                    }
                    for py in 0..14 {
                        let width = 6 + py / 2;
                        for px in 0..width {
                            surface.set_pixel(avatar_center_x - width / 2 + px, avatar_center_y + 6 + py, icon_color);
                        }
                    }

                    // Display name
                    let name = &user.display_name;
                    let name_y = card_y + Self::USER_CARD_SIZE - 30;
                    let name_x = card_x + (Self::USER_CARD_SIZE.saturating_sub(name.len() * 8)) / 2;
                    draw_string(surface, name_x, name_y, name, Color::new(255, 255, 255));
                }

                // Hint
                let hint_y = h - 60;
                draw_string_centered(surface, center_x, hint_y, "Select a user and press Enter", Color::new(100, 100, 120));
            }
            LoginState::PasswordEntry | LoginState::Authenticating => {
                // Show selected user
                if let Some(user) = self.users.get(self.selected_user) {
                    let user_y = self.bounds.height / 2 - 80;

                    // Avatar
                    let avatar_center_x = center_x;
                    let avatar_center_y = user_y + 40;
                    let avatar_radius = 40;

                    let avatar_bg = Color::new(60, 60, 80);
                    for py in 0..avatar_radius * 2 {
                        for px in 0..avatar_radius * 2 {
                            let dx = px as isize - avatar_radius as isize;
                            let dy = py as isize - avatar_radius as isize;
                            if dx * dx + dy * dy <= (avatar_radius as isize * avatar_radius as isize) {
                                surface.set_pixel(avatar_center_x - avatar_radius + px, avatar_center_y - avatar_radius + py, avatar_bg);
                            }
                        }
                    }

                    // User icon
                    let icon_color = Color::new(150, 150, 170);
                    let head_r = 12;
                    for py in 0..head_r * 2 {
                        for px in 0..head_r * 2 {
                            let dx = px as isize - head_r as isize;
                            let dy = py as isize - head_r as isize;
                            if dx * dx + dy * dy <= (head_r as isize * head_r as isize) {
                                surface.set_pixel(avatar_center_x - head_r + px, avatar_center_y - 16 + py, icon_color);
                            }
                        }
                    }
                    for py in 0..24 {
                        let width = 10 + py / 2;
                        for px in 0..width {
                            surface.set_pixel(avatar_center_x - width / 2 + px, avatar_center_y + 4 + py, icon_color);
                        }
                    }

                    // Display name
                    let name_y = user_y + 100;
                    draw_string_centered(surface, center_x, name_y, &user.display_name, Color::new(255, 255, 255));

                    // Password input
                    let input_y = name_y + 40;
                    let input_x = center_x.saturating_sub(Self::INPUT_WIDTH / 2);

                    let shake_offset = if self.shake_animation > 0 {
                        ((self.shake_animation as isize % 4) - 2) * 3
                    } else {
                        0
                    };
                    let input_x_shake = (input_x as isize + shake_offset).max(0) as usize;

                    // Input background
                    let input_bg = Color::new(50, 50, 60);
                    for py in 0..Self::INPUT_HEIGHT {
                        for px in 0..Self::INPUT_WIDTH {
                            surface.set_pixel(input_x_shake + px, input_y + py, input_bg);
                        }
                    }

                    // Border
                    let border = Color::new(0, 122, 204);
                    for px in 0..Self::INPUT_WIDTH {
                        surface.set_pixel(input_x_shake + px, input_y, border);
                        surface.set_pixel(input_x_shake + px, input_y + Self::INPUT_HEIGHT - 1, border);
                    }
                    for py in 0..Self::INPUT_HEIGHT {
                        surface.set_pixel(input_x_shake, input_y + py, border);
                        surface.set_pixel(input_x_shake + Self::INPUT_WIDTH - 1, input_y + py, border);
                    }

                    // Password
                    if self.password.is_empty() {
                        draw_string(surface, input_x_shake + 16, input_y + 12, "Password", Color::new(100, 100, 120));
                    } else {
                        let dots: String = (0..self.password.len()).map(|_| '*').collect();
                        draw_string(surface, input_x_shake + 16, input_y + 12, &dots, Color::new(255, 255, 255));
                    }

                    // Cursor
                    if self.input_focused {
                        let cursor_x = input_x_shake + 16 + self.password.len() * 8;
                        for py in 0..20 {
                            surface.set_pixel(cursor_x, input_y + 10 + py, Color::new(255, 255, 255));
                        }
                    }

                    // Error
                    if let Some(ref error) = self.error_message {
                        let error_y = input_y + Self::INPUT_HEIGHT + 16;
                        draw_string_centered(surface, center_x, error_y, error, Color::new(255, 100, 100));
                    }

                    // Hint
                    let hint_y = input_y + Self::INPUT_HEIGHT + 50;
                    draw_string_centered(surface, center_x, hint_y, "Press Esc to go back", Color::new(100, 100, 120));
                }
            }
            LoginState::LoggedIn => {
                // Should not render when logged in
            }
        }

        // Bottom branding
        let brand_y = h - 30;
        draw_string_centered(surface, center_x, brand_y, "Stenzel OS", Color::new(80, 80, 100));

        // Power buttons (bottom right)
        let btn_y = h - 50;
        let btn_x = w - 100;

        // Shutdown button
        draw_string(surface, btn_x, btn_y, "[Power]", Color::new(150, 150, 170));
    }
}

fn draw_string(surface: &mut Surface, x: usize, y: usize, s: &str, color: Color) {
    for (i, c) in s.chars().enumerate() {
        draw_char_simple(surface, x + i * 8, y, c, color);
    }
}

fn draw_string_centered(surface: &mut Surface, center_x: usize, y: usize, s: &str, color: Color) {
    let width = s.len() * 8;
    let x = center_x.saturating_sub(width / 2);
    draw_string(surface, x, y, s, color);
}

fn draw_string_scaled(surface: &mut Surface, x: usize, y: usize, s: &str, color: Color, scale: usize) {
    for (char_idx, c) in s.chars().enumerate() {
        draw_char_scaled(surface, x + char_idx * 8 * scale, y, c, color, scale);
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

fn draw_char_scaled(surface: &mut Surface, x: usize, y: usize, c: char, color: Color, scale: usize) {
    use crate::drivers::font::DEFAULT_FONT;

    if let Some(glyph) = DEFAULT_FONT.get_glyph(c) {
        for row in 0..DEFAULT_FONT.height {
            let byte = glyph[row];
            for col in 0..DEFAULT_FONT.width {
                if (byte >> (DEFAULT_FONT.width - 1 - col)) & 1 != 0 {
                    for sy in 0..scale {
                        for sx in 0..scale {
                            surface.set_pixel(x + col * scale + sx, y + row * scale + sy, color);
                        }
                    }
                }
            }
        }
    }
}

fn format_time(hours: u8, minutes: u8) -> String {
    use alloc::string::ToString;
    let mut s = String::new();
    if hours < 10 { s.push('0'); }
    s.push_str(&hours.to_string());
    s.push(':');
    if minutes < 10 { s.push('0'); }
    s.push_str(&minutes.to_string());
    s
}

fn format_date(weekday: &str, month: &str, day: u8) -> String {
    use alloc::string::ToString;
    let mut s = String::from(weekday);
    s.push_str(", ");
    s.push_str(month);
    s.push(' ');
    s.push_str(&day.to_string());
    s
}
