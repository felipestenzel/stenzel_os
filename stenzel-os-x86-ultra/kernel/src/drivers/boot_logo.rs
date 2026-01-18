//! Boot Logo Display
//!
//! Displays the Stenzel OS logo during boot with optional progress indicator.
//! Supports multiple logo styles and animations.

use super::framebuffer::{Color, with_framebuffer};
use alloc::string::String;
use spin::Mutex;

/// Boot logo configuration
static BOOT_LOGO_STATE: Mutex<BootLogoState> = Mutex::new(BootLogoState::new());

/// Boot logo state
struct BootLogoState {
    /// Whether the logo is currently displayed
    displayed: bool,
    /// Current progress (0-100)
    progress: u8,
    /// Whether to show progress bar
    show_progress: bool,
    /// Logo style
    style: LogoStyle,
    /// Boot messages
    boot_stage: BootStage,
}

impl BootLogoState {
    const fn new() -> Self {
        Self {
            displayed: false,
            progress: 0,
            show_progress: true,
            style: LogoStyle::Default,
            boot_stage: BootStage::Starting,
        }
    }
}

/// Boot logo style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogoStyle {
    /// Default style: text with gradient background
    Default,
    /// Minimal: simple text on black
    Minimal,
    /// Modern: centered logo with animation
    Modern,
    /// Retro: classic BIOS-style text
    Retro,
}

/// Boot stage for progress display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootStage {
    Starting,
    Hardware,
    Memory,
    Interrupts,
    Drivers,
    Filesystem,
    Network,
    Services,
    Desktop,
    Ready,
}

impl BootStage {
    /// Get the display name for this stage
    pub fn name(&self) -> &'static str {
        match self {
            BootStage::Starting => "Starting Stenzel OS...",
            BootStage::Hardware => "Detecting hardware...",
            BootStage::Memory => "Initializing memory...",
            BootStage::Interrupts => "Setting up interrupts...",
            BootStage::Drivers => "Loading drivers...",
            BootStage::Filesystem => "Mounting filesystems...",
            BootStage::Network => "Configuring network...",
            BootStage::Services => "Starting services...",
            BootStage::Desktop => "Preparing desktop...",
            BootStage::Ready => "Welcome to Stenzel OS!",
        }
    }

    /// Get the progress percentage for this stage
    pub fn progress(&self) -> u8 {
        match self {
            BootStage::Starting => 0,
            BootStage::Hardware => 10,
            BootStage::Memory => 20,
            BootStage::Interrupts => 30,
            BootStage::Drivers => 45,
            BootStage::Filesystem => 60,
            BootStage::Network => 75,
            BootStage::Services => 85,
            BootStage::Desktop => 95,
            BootStage::Ready => 100,
        }
    }
}

/// Primary colors for the Stenzel OS branding
pub mod colors {
    use super::Color;

    /// Primary brand color (deep blue)
    pub const PRIMARY: Color = Color::new(32, 64, 128);
    /// Secondary brand color (light blue)
    pub const SECONDARY: Color = Color::new(64, 128, 192);
    /// Accent color (orange)
    pub const ACCENT: Color = Color::new(255, 128, 0);
    /// Background dark
    pub const BG_DARK: Color = Color::new(16, 24, 32);
    /// Background gradient start
    pub const BG_GRADIENT_START: Color = Color::new(24, 36, 48);
    /// Background gradient end
    pub const BG_GRADIENT_END: Color = Color::new(8, 16, 24);
    /// Text primary (white)
    pub const TEXT_PRIMARY: Color = Color::WHITE;
    /// Text secondary (gray)
    pub const TEXT_SECONDARY: Color = Color::new(160, 160, 176);
    /// Progress bar background
    pub const PROGRESS_BG: Color = Color::new(48, 48, 64);
    /// Progress bar fill
    pub const PROGRESS_FILL: Color = Color::new(64, 192, 128);
}

/// ASCII art logo (compact version)
const LOGO_ASCII: &[&str] = &[
    "  ____  _                       _    ___  ____  ",
    " / ___|| |_ ___ _ __  _______  | |  / _ \\/ ___| ",
    " \\___ \\| __/ _ \\ '_ \\|_  / _ \\ | | | | | \\___ \\ ",
    "  ___) | ||  __/ | | |/ /  __/ | | | |_| |___) |",
    " |____/ \\__\\___|_| |_/___\\___| |_|  \\___/|____/ ",
];

/// Large ASCII art logo
const LOGO_LARGE: &[&str] = &[
    "   _____ _______ ______ _   _ ____________ _       ",
    "  / ____|__   __|  ____| \\ | |___  /  ____| |      ",
    " | (___    | |  | |__  |  \\| |  / /| |__  | |      ",
    "  \\___ \\   | |  |  __| | . ` | / / |  __| | |      ",
    "  ____) |  | |  | |____| |\\  |/ /__| |____| |____  ",
    " |_____/   |_|  |______|_| \\_/_____|______|______| ",
    "                                                   ",
    "        ____   _____ ",
    "       / __ \\ / ____|",
    "      | |  | | (___  ",
    "      | |  | |\\___ \\ ",
    "      | |__| |____) |",
    "       \\____/|_____/ ",
];

/// Simple logo using box-drawing characters
const LOGO_SIMPLE: &[&str] = &[
    "╔═══════════════════════════════════╗",
    "║                                   ║",
    "║     STENZEL OS                    ║",
    "║     A Modern Operating System     ║",
    "║                                   ║",
    "╚═══════════════════════════════════╝",
];

/// Initialize the boot logo subsystem
pub fn init() {
    let mut state = BOOT_LOGO_STATE.lock();
    state.displayed = false;
    state.progress = 0;
    state.boot_stage = BootStage::Starting;
}

/// Display the boot logo
pub fn show() {
    let style = {
        let state = BOOT_LOGO_STATE.lock();
        state.style
    };

    match style {
        LogoStyle::Default => show_default_logo(),
        LogoStyle::Minimal => show_minimal_logo(),
        LogoStyle::Modern => show_modern_logo(),
        LogoStyle::Retro => show_retro_logo(),
    }

    BOOT_LOGO_STATE.lock().displayed = true;
}

/// Show the default logo style
fn show_default_logo() {
    with_framebuffer(|fb| {
        let width = fb.width();
        let height = fb.height();

        // Draw gradient background
        draw_gradient_background(fb, width, height);

        // Calculate logo position (centered)
        let logo_height = LOGO_ASCII.len() * 16; // Assuming 16px font height
        let logo_width = LOGO_ASCII[0].len() * 8; // Assuming 8px font width
        let logo_x = (width.saturating_sub(logo_width)) / 2;
        let logo_y = (height.saturating_sub(logo_height)) / 3;

        // Draw the ASCII logo
        for (i, line) in LOGO_ASCII.iter().enumerate() {
            let y = logo_y + i * 16;
            fb.draw_string(logo_x, y, line, colors::SECONDARY, None);
        }

        // Draw version text
        let version_text = "Version 1.0";
        let version_x = (width - version_text.len() * 8) / 2;
        let version_y = logo_y + logo_height + 20;
        fb.draw_string(version_x, version_y, version_text, colors::TEXT_SECONDARY, None);

        // Draw copyright
        let copyright = "(c) 2026 Stenzel OS Project";
        let copyright_x = (width - copyright.len() * 8) / 2;
        let copyright_y = height - 40;
        fb.draw_string(copyright_x, copyright_y, copyright, colors::TEXT_SECONDARY, None);
    });
}

/// Show minimal logo style
fn show_minimal_logo() {
    with_framebuffer(|fb| {
        let width = fb.width();
        let height = fb.height();

        // Black background
        fb.clear(Color::BLACK);

        // Draw simple centered text
        let text = "STENZEL OS";
        let text_x = (width - text.len() * 8) / 2;
        let text_y = height / 2 - 8;
        fb.draw_string(text_x, text_y, text, Color::WHITE, None);

        // Subtle underline
        let line_width = text.len() * 8 + 20;
        let line_x = (width - line_width) / 2;
        let line_y = text_y + 20;
        fb.fill_rect(line_x, line_y, line_width, 2, colors::SECONDARY);
    });
}

/// Show modern logo style with decorative elements
fn show_modern_logo() {
    with_framebuffer(|fb| {
        let width = fb.width();
        let height = fb.height();

        // Draw gradient background
        draw_gradient_background(fb, width, height);

        // Draw decorative circles
        draw_decorative_circles(fb, width, height);

        // Draw the logo
        let center_x = width / 2;
        let center_y = height / 3;

        // Draw "S" emblem (large circle with S)
        let radius = 60isize;
        fb.fill_circle(center_x as isize, center_y as isize, radius, colors::PRIMARY);
        fb.fill_circle(center_x as isize, center_y as isize, radius - 5, colors::SECONDARY);
        fb.fill_circle(center_x as isize, center_y as isize, radius - 10, colors::PRIMARY);

        // Draw "S" letter in the circle
        let s_x = center_x - 16;
        let s_y = center_y as usize - 24;
        fb.draw_string(s_x, s_y, "S", Color::WHITE, None);
        // Make the S bigger by drawing it multiple times with offset
        fb.draw_string(s_x + 1, s_y, "S", Color::WHITE, None);
        fb.draw_string(s_x, s_y + 1, "S", Color::WHITE, None);
        fb.draw_string(s_x + 1, s_y + 1, "S", Color::WHITE, None);

        // Draw OS name below
        let name = "STENZEL OS";
        let name_x = (width - name.len() * 8) / 2;
        let name_y = center_y as usize + radius as usize + 30;
        fb.draw_text_shadowed(name_x, name_y, name, Color::WHITE, Color::BLACK, 2);

        // Draw tagline
        let tagline = "A Modern Operating System";
        let tagline_x = (width - tagline.len() * 8) / 2;
        let tagline_y = name_y + 30;
        fb.draw_string(tagline_x, tagline_y, tagline, colors::TEXT_SECONDARY, None);
    });
}

/// Show retro BIOS-style logo
fn show_retro_logo() {
    with_framebuffer(|fb| {
        let width = fb.width();
        let height = fb.height();

        // Classic blue background
        fb.clear(Color::new(0, 0, 170));

        // White border
        fb.draw_rect(2, 2, width - 4, height - 4, Color::WHITE);

        // Title bar
        fb.fill_rect(4, 4, width - 8, 20, Color::new(0, 170, 170));
        let title = " Stenzel OS Setup ";
        fb.draw_string(8, 6, title, Color::BLACK, None);

        // Logo using simple box
        let box_y = 50;
        for (i, line) in LOGO_SIMPLE.iter().enumerate() {
            let y = box_y + i * 16;
            let x = (width - line.chars().count() * 8) / 2;
            fb.draw_string(x, y, line, Color::WHITE, None);
        }

        // System info
        let info_y = height / 2 + 50;
        let info_items = [
            "CPU: x86_64 Compatible Processor",
            "Memory: Detecting...",
            "Storage: Detecting...",
            "",
            "Press any key to continue...",
        ];

        for (i, item) in info_items.iter().enumerate() {
            let y = info_y + i * 18;
            fb.draw_string(20, y, item, Color::LIGHT_GRAY, None);
        }

        // Footer
        let footer = "Copyright (c) 2026 Stenzel OS Project. All rights reserved.";
        let footer_x = (width - footer.len() * 8) / 2;
        fb.draw_string(footer_x, height - 30, footer, Color::GRAY, None);
    });
}

/// Draw a vertical gradient background
fn draw_gradient_background(fb: &mut super::framebuffer::FrameBufferState, width: usize, height: usize) {
    for y in 0..height {
        // Interpolate between gradient start and end colors
        let t = y as u32 * 255 / height as u32;
        let r = colors::BG_GRADIENT_START.r as u32 +
                (colors::BG_GRADIENT_END.r as i32 - colors::BG_GRADIENT_START.r as i32) as u32 * t / 255;
        let g = colors::BG_GRADIENT_START.g as u32 +
                (colors::BG_GRADIENT_END.g as i32 - colors::BG_GRADIENT_START.g as i32) as u32 * t / 255;
        let b = colors::BG_GRADIENT_START.b as u32 +
                (colors::BG_GRADIENT_END.b as i32 - colors::BG_GRADIENT_START.b as i32) as u32 * t / 255;
        let color = Color::new(r as u8, g as u8, b as u8);
        fb.fill_rect(0, y, width, 1, color);
    }
}

/// Draw decorative circles in the background
fn draw_decorative_circles(fb: &mut super::framebuffer::FrameBufferState, width: usize, height: usize) {
    // Draw several translucent circles for decoration
    let circles = [
        (width / 4, height / 4, 100, 10),
        (width * 3 / 4, height / 3, 80, 8),
        (width / 5, height * 2 / 3, 60, 15),
        (width * 4 / 5, height * 3 / 4, 90, 12),
    ];

    for (cx, cy, radius, alpha) in circles.iter() {
        // Draw with a very subtle color
        let color = Color::with_alpha(64, 128, 192, *alpha);
        fb.draw_circle(*cx as isize, *cy as isize, *radius as isize, color);
        fb.draw_circle(*cx as isize, *cy as isize, (*radius - 2) as isize, color);
    }
}

/// Update the boot progress
pub fn set_progress(progress: u8) {
    let show = {
        let mut state = BOOT_LOGO_STATE.lock();
        state.progress = progress.min(100);
        state.displayed && state.show_progress
    };

    if show {
        draw_progress_bar();
    }
}

/// Set the current boot stage
pub fn set_stage(stage: BootStage) {
    let (show, should_draw) = {
        let mut state = BOOT_LOGO_STATE.lock();
        state.boot_stage = stage;
        state.progress = stage.progress();
        (state.displayed && state.show_progress, state.displayed)
    };

    if should_draw {
        draw_stage_text();
        if show {
            draw_progress_bar();
        }
    }
}

/// Draw the progress bar
fn draw_progress_bar() {
    let progress = BOOT_LOGO_STATE.lock().progress;

    with_framebuffer(|fb| {
        let width = fb.width();
        let height = fb.height();

        // Progress bar dimensions
        let bar_width = width / 2;
        let bar_height = 8;
        let bar_x = (width - bar_width) / 2;
        let bar_y = height * 2 / 3;

        // Draw background
        fb.fill_rect(bar_x, bar_y, bar_width, bar_height, colors::PROGRESS_BG);

        // Draw border
        fb.draw_rect(bar_x, bar_y, bar_width, bar_height, colors::TEXT_SECONDARY);

        // Draw fill
        let fill_width = (bar_width - 4) * progress as usize / 100;
        if fill_width > 0 {
            fb.fill_rect(bar_x + 2, bar_y + 2, fill_width, bar_height - 4, colors::PROGRESS_FILL);
        }

        // Draw percentage text
        let percent_text = format_progress(progress);
        let text_x = bar_x + bar_width + 10;
        let text_y = bar_y - 2;
        // Clear previous text area
        fb.fill_rect(text_x, text_y, 40, 16, colors::BG_DARK);
        fb.draw_string(text_x, text_y, &percent_text, colors::TEXT_PRIMARY, None);
    });
}

/// Format progress as percentage string
fn format_progress(progress: u8) -> String {
    let mut s = String::new();
    if progress >= 100 {
        s.push_str("100%");
    } else if progress >= 10 {
        s.push(char::from_digit((progress / 10) as u32, 10).unwrap_or('0'));
        s.push(char::from_digit((progress % 10) as u32, 10).unwrap_or('0'));
        s.push('%');
    } else {
        s.push(char::from_digit(progress as u32, 10).unwrap_or('0'));
        s.push('%');
    }
    s
}

/// Draw the current stage text
fn draw_stage_text() {
    let stage = BOOT_LOGO_STATE.lock().boot_stage;

    with_framebuffer(|fb| {
        let width = fb.width();
        let height = fb.height();

        let text = stage.name();
        let text_x = (width - text.len() * 8) / 2;
        let text_y = height * 2 / 3 + 30;

        // Clear previous text area
        fb.fill_rect(0, text_y, width, 20, colors::BG_DARK);

        // Draw new text
        fb.draw_string(text_x, text_y, text, colors::TEXT_SECONDARY, None);
    });
}

/// Set the logo style
pub fn set_style(style: LogoStyle) {
    BOOT_LOGO_STATE.lock().style = style;
}

/// Enable or disable progress bar
pub fn set_show_progress(show: bool) {
    BOOT_LOGO_STATE.lock().show_progress = show;
}

/// Hide the boot logo (typically called when starting the GUI)
pub fn hide() {
    BOOT_LOGO_STATE.lock().displayed = false;

    with_framebuffer(|fb| {
        fb.clear(Color::BLACK);
    });
}

/// Check if the boot logo is currently displayed
pub fn is_displayed() -> bool {
    BOOT_LOGO_STATE.lock().displayed
}

/// Draw a spinner animation at the given position
/// Call this repeatedly to animate the spinner
pub fn draw_spinner(frame: usize) {
    with_framebuffer(|fb| {
        let width = fb.width();
        let height = fb.height();

        let cx = width / 2;
        let cy = height * 2 / 3 - 40;
        let radius = 20isize;

        // Clear spinner area
        fb.fill_rect(cx - 25, cy - 25, 50, 50, colors::BG_DARK);

        // Draw spinner segments
        let segments = 8;
        for i in 0..segments {
            let angle = (frame + i) % segments;
            let brightness = 255 - (angle * 32).min(255);
            let color = Color::new(brightness as u8, brightness as u8, brightness as u8);

            // Calculate segment position
            let seg_angle = i as f64 * 3.14159 * 2.0 / segments as f64;
            // Using integer approximation for sin/cos
            let (sin_approx, cos_approx) = approx_sincos(i, segments);
            let x = cx as isize + (radius * cos_approx / 100);
            let y = cy as isize + (radius * sin_approx / 100);

            fb.fill_circle(x, y, 4, color);
        }
    });
}

/// Approximate sin and cos using a lookup table (returns values scaled by 100)
fn approx_sincos(i: usize, segments: usize) -> (isize, isize) {
    // Pre-calculated values for 8 segments (45 degree increments)
    // sin and cos values multiplied by 100
    let table: [(isize, isize); 8] = [
        (0, 100),    // 0 degrees
        (71, 71),    // 45 degrees
        (100, 0),    // 90 degrees
        (71, -71),   // 135 degrees
        (0, -100),   // 180 degrees
        (-71, -71),  // 225 degrees
        (-100, 0),   // 270 degrees
        (-71, 71),   // 315 degrees
    ];

    if segments == 8 && i < 8 {
        table[i]
    } else {
        (0, 0)
    }
}

/// Show a quick boot splash (for fast boot without progress)
pub fn show_splash() {
    with_framebuffer(|fb| {
        let width = fb.width();
        let height = fb.height();

        // Quick black to avoid flash
        fb.clear(colors::BG_DARK);

        // Just show the name centered
        let name = "STENZEL OS";
        let name_x = (width - name.len() * 8) / 2;
        let name_y = height / 2 - 8;
        fb.draw_text_shadowed(name_x, name_y, name, Color::WHITE, Color::BLACK, 1);
    });

    BOOT_LOGO_STATE.lock().displayed = true;
}

/// Display an error message on the boot screen
pub fn show_error(message: &str) {
    with_framebuffer(|fb| {
        let width = fb.width();
        let height = fb.height();

        // Error banner
        let banner_y = height / 2;
        let banner_height = 60;
        fb.fill_rect(0, banner_y, width, banner_height, Color::new(128, 0, 0));

        // Error icon (!)
        let icon_x = 20;
        let icon_y = banner_y + 20;
        fb.draw_string(icon_x, icon_y, "!", Color::YELLOW, None);

        // Error message
        let msg_x = 50;
        let msg_y = banner_y + 10;
        fb.draw_string(msg_x, msg_y, "Boot Error:", Color::WHITE, None);

        // Truncate message if too long
        let max_chars = (width - 60) / 8;
        let display_msg = if message.len() > max_chars {
            &message[..max_chars]
        } else {
            message
        };
        fb.draw_string(msg_x, msg_y + 20, display_msg, Color::WHITE, None);

        // Help text
        let help = "Press any key to continue or power button to shutdown";
        let help_x = (width - help.len() * 8) / 2;
        let help_y = banner_y + banner_height + 20;
        fb.draw_string(help_x, help_y, help, colors::TEXT_SECONDARY, None);
    });
}

/// Display boot completion animation
pub fn show_complete() {
    set_stage(BootStage::Ready);

    with_framebuffer(|fb| {
        let width = fb.width();
        let height = fb.height();

        // Checkmark position
        let cx = width / 2;
        let cy = height * 2 / 3 - 40;

        // Draw green checkmark circle
        fb.fill_circle(cx as isize, cy as isize, 25, Color::new(32, 160, 64));

        // Draw checkmark (using lines would be ideal, but use text for simplicity)
        // The checkmark is approximated with a simple character
        fb.draw_string(cx - 8, cy - 8, "OK", Color::WHITE, None);
    });
}
