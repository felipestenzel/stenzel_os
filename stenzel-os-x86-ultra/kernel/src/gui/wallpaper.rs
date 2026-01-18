//! Wallpaper System
//!
//! Provides wallpaper management with slideshow, dynamic wallpapers,
//! and multi-monitor support.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use spin::Mutex;

// ============================================================================
// Math Helpers (no_std compatible)
// ============================================================================

/// Approximate sin for f32 using Taylor series
fn sin_f32(x: f32) -> f32 {
    let pi = core::f32::consts::PI;
    let mut x = x % (2.0 * pi);
    if x > pi { x -= 2.0 * pi; }
    if x < -pi { x += 2.0 * pi; }
    let x2 = x * x;
    let x3 = x2 * x;
    let x5 = x3 * x2;
    let x7 = x5 * x2;
    x - x3 / 6.0 + x5 / 120.0 - x7 / 5040.0
}

/// Approximate cos for f32 using Taylor series
fn cos_f32(x: f32) -> f32 {
    sin_f32(x + core::f32::consts::PI / 2.0)
}

/// Approximate sqrt for f32 using Newton's method
fn sqrt_f32(x: f32) -> f32 {
    if x <= 0.0 { return 0.0; }
    let mut guess = x / 2.0;
    if guess == 0.0 { guess = 1.0; }
    for _ in 0..10 {
        let new_guess = (guess + x / guess) / 2.0;
        if (new_guess - guess).abs() < 1e-7 { break; }
        guess = new_guess;
    }
    guess
}

/// Approximate atan2 for f32
fn atan2_f32(y: f32, x: f32) -> f32 {
    let pi = core::f32::consts::PI;
    if x == 0.0 {
        if y > 0.0 { return pi / 2.0; }
        if y < 0.0 { return -pi / 2.0; }
        return 0.0;
    }
    let atan = atan_f32(y / x);
    if x > 0.0 { atan }
    else if y >= 0.0 { atan + pi }
    else { atan - pi }
}

/// Approximate atan for f32
fn atan_f32(x: f32) -> f32 {
    let pi = core::f32::consts::PI;
    if x > 1.0 { return pi / 2.0 - atan_f32(1.0 / x); }
    if x < -1.0 { return -pi / 2.0 - atan_f32(1.0 / x); }
    let x2 = x * x;
    let x3 = x * x2;
    let x5 = x3 * x2;
    let x7 = x5 * x2;
    x - x3 / 3.0 + x5 / 5.0 - x7 / 7.0
}

/// Global wallpaper state
static WALLPAPER_STATE: Mutex<Option<WallpaperState>> = Mutex::new(None);

/// Wallpaper state
pub struct WallpaperState {
    /// Current wallpaper
    pub current: Option<WallpaperInfo>,
    /// Wallpaper collection
    pub collection: BTreeMap<String, WallpaperInfo>,
    /// Slideshow settings
    pub slideshow: Option<SlideshowConfig>,
    /// Per-monitor wallpapers
    pub monitor_wallpapers: BTreeMap<u32, String>,
    /// Wallpaper directories
    pub directories: Vec<String>,
    /// Scaling mode
    pub scale_mode: ScaleMode,
    /// Background color (shown when no wallpaper or letterboxing)
    pub background_color: u32,
    /// Blur amount for lock screen
    pub lock_screen_blur: u32,
    /// Cached scaled wallpapers
    pub cache: BTreeMap<CacheKey, CachedWallpaper>,
}

/// Wallpaper info
#[derive(Debug, Clone)]
pub struct WallpaperInfo {
    /// Unique identifier
    pub id: String,
    /// Display name
    pub name: String,
    /// File path
    pub path: String,
    /// Wallpaper type
    pub wallpaper_type: WallpaperType,
    /// Original dimensions
    pub width: u32,
    pub height: u32,
    /// Image data (RGBA)
    pub data: Vec<u8>,
    /// Thumbnail (for picker)
    pub thumbnail: Option<Vec<u8>>,
    /// Dominant colors
    pub colors: DominantColors,
    /// Metadata
    pub metadata: WallpaperMetadata,
}

/// Wallpaper type
#[derive(Debug, Clone)]
pub enum WallpaperType {
    /// Static image
    Static,
    /// Dynamic wallpaper (changes with time)
    Dynamic(DynamicConfig),
    /// Live wallpaper (animated)
    Live(LiveConfig),
    /// Solid color
    SolidColor(u32),
    /// Gradient
    Gradient(GradientConfig),
}

/// Dynamic wallpaper config
#[derive(Debug, Clone)]
pub struct DynamicConfig {
    /// Time-based variants
    pub variants: Vec<TimeVariant>,
    /// Transition duration (seconds)
    pub transition_duration: f32,
}

/// Time variant for dynamic wallpaper
#[derive(Debug, Clone)]
pub struct TimeVariant {
    /// Hour of day (0-23)
    pub hour: u8,
    /// Minute (0-59)
    pub minute: u8,
    /// Image path
    pub path: String,
    /// Image data
    pub data: Vec<u8>,
}

/// Live wallpaper config
#[derive(Debug, Clone)]
pub struct LiveConfig {
    /// Animation frames
    pub frames: Vec<Vec<u8>>,
    /// Frame delay (ms)
    pub delay_ms: u32,
    /// Loop mode
    pub loop_mode: LoopMode,
}

/// Loop mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopMode {
    Forever,
    Once,
    PingPong,
}

/// Gradient config
#[derive(Debug, Clone)]
pub struct GradientConfig {
    /// Gradient type
    pub gradient_type: GradientType,
    /// Color stops
    pub stops: Vec<GradientStop>,
    /// Angle (for linear)
    pub angle: f32,
}

/// Gradient type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradientType {
    Linear,
    Radial,
    Conic,
}

/// Gradient stop
#[derive(Debug, Clone, Copy)]
pub struct GradientStop {
    /// Position (0.0 - 1.0)
    pub position: f32,
    /// Color (RGB)
    pub color: u32,
}

/// Dominant colors from image
#[derive(Debug, Clone, Default)]
pub struct DominantColors {
    /// Primary color
    pub primary: u32,
    /// Secondary color
    pub secondary: u32,
    /// Accent color
    pub accent: u32,
    /// Is the image overall dark or light
    pub is_dark: bool,
}

/// Wallpaper metadata
#[derive(Debug, Clone, Default)]
pub struct WallpaperMetadata {
    /// Author
    pub author: Option<String>,
    /// License
    pub license: Option<String>,
    /// Description
    pub description: Option<String>,
    /// Tags
    pub tags: Vec<String>,
    /// Location (for photos)
    pub location: Option<String>,
}

/// Slideshow config
#[derive(Debug, Clone)]
pub struct SlideshowConfig {
    /// Wallpaper IDs in slideshow
    pub wallpapers: Vec<String>,
    /// Current index
    pub current_index: usize,
    /// Interval (seconds)
    pub interval_secs: u32,
    /// Shuffle order
    pub shuffle: bool,
    /// Last change time
    pub last_change_ms: u64,
    /// Transition type
    pub transition: SlideshowTransition,
}

/// Slideshow transition
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlideshowTransition {
    None,
    Fade,
    Slide,
    Zoom,
}

/// Scaling mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleMode {
    /// Fill screen, crop if needed
    Fill,
    /// Fit within screen, letterbox if needed
    Fit,
    /// Stretch to fill
    Stretch,
    /// Center without scaling
    Center,
    /// Tile/repeat
    Tile,
    /// Span across monitors
    Span,
}

/// Cache key
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CacheKey {
    pub wallpaper_id: String,
    pub width: u32,
    pub height: u32,
    pub scale_mode: u8,
}

/// Cached wallpaper
#[derive(Debug, Clone)]
pub struct CachedWallpaper {
    /// Scaled image data
    pub data: Vec<u8>,
    /// Width
    pub width: u32,
    /// Height
    pub height: u32,
}

/// Initialize wallpaper system
pub fn init() {
    let mut state = WALLPAPER_STATE.lock();
    if state.is_some() {
        return;
    }

    let mut collection = BTreeMap::new();

    // Create default gradient wallpaper
    let default = create_default_wallpaper();
    collection.insert(default.id.clone(), default.clone());

    // Create some built-in wallpapers
    let blue = create_solid_wallpaper("solid-blue", "Blue", 0x007AFF);
    collection.insert(blue.id.clone(), blue);

    let dark = create_solid_wallpaper("solid-dark", "Dark", 0x1E1E1E);
    collection.insert(dark.id.clone(), dark);

    let gradient1 = create_gradient_wallpaper("gradient-sunset", "Sunset", &[
        (0.0, 0xFF6B6B),
        (0.5, 0xFECA57),
        (1.0, 0xFF9FF3),
    ]);
    collection.insert(gradient1.id.clone(), gradient1);

    let gradient2 = create_gradient_wallpaper("gradient-ocean", "Ocean", &[
        (0.0, 0x667eea),
        (1.0, 0x764ba2),
    ]);
    collection.insert(gradient2.id.clone(), gradient2);

    *state = Some(WallpaperState {
        current: Some(default),
        collection,
        slideshow: None,
        monitor_wallpapers: BTreeMap::new(),
        directories: vec![
            "/usr/share/wallpapers".to_string(),
            "~/.local/share/wallpapers".to_string(),
        ],
        scale_mode: ScaleMode::Fill,
        background_color: 0x1E1E1E,
        lock_screen_blur: 20,
        cache: BTreeMap::new(),
    });

    crate::kprintln!("wallpaper: initialized with default wallpaper");
}

/// Create default gradient wallpaper
fn create_default_wallpaper() -> WallpaperInfo {
    let width = 1920;
    let height = 1080;
    let data = render_gradient(
        width,
        height,
        &GradientConfig {
            gradient_type: GradientType::Linear,
            stops: vec![
                GradientStop { position: 0.0, color: 0x2C3E50 },
                GradientStop { position: 0.5, color: 0x3498DB },
                GradientStop { position: 1.0, color: 0x2980B9 },
            ],
            angle: 135.0,
        },
    );

    WallpaperInfo {
        id: "default".to_string(),
        name: "Stenzel Default".to_string(),
        path: String::new(),
        wallpaper_type: WallpaperType::Gradient(GradientConfig {
            gradient_type: GradientType::Linear,
            stops: vec![
                GradientStop { position: 0.0, color: 0x2C3E50 },
                GradientStop { position: 0.5, color: 0x3498DB },
                GradientStop { position: 1.0, color: 0x2980B9 },
            ],
            angle: 135.0,
        }),
        width,
        height,
        data,
        thumbnail: None,
        colors: DominantColors {
            primary: 0x3498DB,
            secondary: 0x2C3E50,
            accent: 0x2980B9,
            is_dark: true,
        },
        metadata: WallpaperMetadata::default(),
    }
}

/// Create solid color wallpaper
fn create_solid_wallpaper(id: &str, name: &str, color: u32) -> WallpaperInfo {
    let width = 64;
    let height = 64;
    let r = ((color >> 16) & 0xFF) as u8;
    let g = ((color >> 8) & 0xFF) as u8;
    let b = (color & 0xFF) as u8;

    let mut data = vec![0u8; (width * height * 4) as usize];
    for i in 0..(width * height) as usize {
        data[i * 4] = r;
        data[i * 4 + 1] = g;
        data[i * 4 + 2] = b;
        data[i * 4 + 3] = 255;
    }

    let luminance = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;

    WallpaperInfo {
        id: id.to_string(),
        name: name.to_string(),
        path: String::new(),
        wallpaper_type: WallpaperType::SolidColor(color),
        width,
        height,
        data,
        thumbnail: None,
        colors: DominantColors {
            primary: color,
            secondary: color,
            accent: color,
            is_dark: luminance < 128.0,
        },
        metadata: WallpaperMetadata::default(),
    }
}

/// Create gradient wallpaper
fn create_gradient_wallpaper(id: &str, name: &str, stops: &[(f32, u32)]) -> WallpaperInfo {
    let width = 256;
    let height = 256;

    let gradient_stops: Vec<GradientStop> = stops
        .iter()
        .map(|(pos, color)| GradientStop {
            position: *pos,
            color: *color,
        })
        .collect();

    let config = GradientConfig {
        gradient_type: GradientType::Linear,
        stops: gradient_stops.clone(),
        angle: 135.0,
    };

    let data = render_gradient(width, height, &config);

    let primary = stops.first().map(|(_, c)| *c).unwrap_or(0);
    let secondary = stops.last().map(|(_, c)| *c).unwrap_or(0);

    WallpaperInfo {
        id: id.to_string(),
        name: name.to_string(),
        path: String::new(),
        wallpaper_type: WallpaperType::Gradient(config),
        width,
        height,
        data,
        thumbnail: None,
        colors: DominantColors {
            primary,
            secondary,
            accent: primary,
            is_dark: true,
        },
        metadata: WallpaperMetadata::default(),
    }
}

/// Render gradient to RGBA
fn render_gradient(width: u32, height: u32, config: &GradientConfig) -> Vec<u8> {
    let mut data = vec![0u8; (width * height * 4) as usize];

    match config.gradient_type {
        GradientType::Linear => {
            let angle_rad = config.angle * core::f32::consts::PI / 180.0;
            let cos_a = cos_f32(angle_rad);
            let sin_a = sin_f32(angle_rad);

            for y in 0..height {
                for x in 0..width {
                    // Calculate position along gradient
                    let nx = x as f32 / width as f32 - 0.5;
                    let ny = y as f32 / height as f32 - 0.5;
                    let t = (nx * cos_a + ny * sin_a + 0.5).clamp(0.0, 1.0);

                    let color = interpolate_gradient(&config.stops, t);
                    let idx = ((y * width + x) * 4) as usize;
                    data[idx] = ((color >> 16) & 0xFF) as u8;
                    data[idx + 1] = ((color >> 8) & 0xFF) as u8;
                    data[idx + 2] = (color & 0xFF) as u8;
                    data[idx + 3] = 255;
                }
            }
        }
        GradientType::Radial => {
            let cx = width as f32 / 2.0;
            let cy = height as f32 / 2.0;
            let max_dist = sqrt_f32(cx * cx + cy * cy);

            for y in 0..height {
                for x in 0..width {
                    let dx = x as f32 - cx;
                    let dy = y as f32 - cy;
                    let dist = sqrt_f32(dx * dx + dy * dy);
                    let t = (dist / max_dist).clamp(0.0, 1.0);

                    let color = interpolate_gradient(&config.stops, t);
                    let idx = ((y * width + x) * 4) as usize;
                    data[idx] = ((color >> 16) & 0xFF) as u8;
                    data[idx + 1] = ((color >> 8) & 0xFF) as u8;
                    data[idx + 2] = (color & 0xFF) as u8;
                    data[idx + 3] = 255;
                }
            }
        }
        GradientType::Conic => {
            let cx = width as f32 / 2.0;
            let cy = height as f32 / 2.0;

            for y in 0..height {
                for x in 0..width {
                    let dx = x as f32 - cx;
                    let dy = y as f32 - cy;
                    let angle = atan2_f32(dy, dx);
                    let t = ((angle / core::f32::consts::PI + 1.0) / 2.0).clamp(0.0, 1.0);

                    let color = interpolate_gradient(&config.stops, t);
                    let idx = ((y * width + x) * 4) as usize;
                    data[idx] = ((color >> 16) & 0xFF) as u8;
                    data[idx + 1] = ((color >> 8) & 0xFF) as u8;
                    data[idx + 2] = (color & 0xFF) as u8;
                    data[idx + 3] = 255;
                }
            }
        }
    }

    data
}

/// Interpolate gradient color at position t
fn interpolate_gradient(stops: &[GradientStop], t: f32) -> u32 {
    if stops.is_empty() {
        return 0;
    }

    if stops.len() == 1 {
        return stops[0].color;
    }

    // Find surrounding stops
    let mut prev_stop = &stops[0];
    let mut next_stop = &stops[stops.len() - 1];

    for i in 0..stops.len() - 1 {
        if t >= stops[i].position && t <= stops[i + 1].position {
            prev_stop = &stops[i];
            next_stop = &stops[i + 1];
            break;
        }
    }

    // Interpolate between stops
    let range = next_stop.position - prev_stop.position;
    let local_t = if range > 0.0001 {
        (t - prev_stop.position) / range
    } else {
        0.0
    };

    let r1 = ((prev_stop.color >> 16) & 0xFF) as f32;
    let g1 = ((prev_stop.color >> 8) & 0xFF) as f32;
    let b1 = (prev_stop.color & 0xFF) as f32;

    let r2 = ((next_stop.color >> 16) & 0xFF) as f32;
    let g2 = ((next_stop.color >> 8) & 0xFF) as f32;
    let b2 = (next_stop.color & 0xFF) as f32;

    let r = (r1 + (r2 - r1) * local_t) as u32;
    let g = (g1 + (g2 - g1) * local_t) as u32;
    let b = (b1 + (b2 - b1) * local_t) as u32;

    (r << 16) | (g << 8) | b
}

/// Set wallpaper by ID
pub fn set_wallpaper(id: &str) -> Result<(), WallpaperError> {
    let mut state = WALLPAPER_STATE.lock();
    let state = state.as_mut().ok_or(WallpaperError::NotInitialized)?;

    if let Some(wallpaper) = state.collection.get(id) {
        state.current = Some(wallpaper.clone());
        state.slideshow = None; // Cancel slideshow
        crate::kprintln!("wallpaper: set to '{}'", wallpaper.name);
        Ok(())
    } else {
        Err(WallpaperError::NotFound)
    }
}

/// Set wallpaper for specific monitor
pub fn set_monitor_wallpaper(monitor_id: u32, wallpaper_id: &str) -> Result<(), WallpaperError> {
    let mut state = WALLPAPER_STATE.lock();
    let state = state.as_mut().ok_or(WallpaperError::NotInitialized)?;

    if state.collection.contains_key(wallpaper_id) {
        state.monitor_wallpapers.insert(monitor_id, wallpaper_id.to_string());
        Ok(())
    } else {
        Err(WallpaperError::NotFound)
    }
}

/// Get current wallpaper
pub fn get_current() -> Option<WallpaperInfo> {
    let state = WALLPAPER_STATE.lock();
    state.as_ref().and_then(|s| s.current.clone())
}

/// Get wallpaper for monitor
pub fn get_monitor_wallpaper(monitor_id: u32) -> Option<WallpaperInfo> {
    let state = WALLPAPER_STATE.lock();
    let state = state.as_ref()?;

    // Check monitor-specific wallpaper first
    if let Some(id) = state.monitor_wallpapers.get(&monitor_id) {
        if let Some(wp) = state.collection.get(id) {
            return Some(wp.clone());
        }
    }

    // Fall back to current wallpaper
    state.current.clone()
}

/// Get scaled wallpaper for display
pub fn get_scaled_wallpaper(width: u32, height: u32) -> Option<CachedWallpaper> {
    let mut state = WALLPAPER_STATE.lock();
    let state = state.as_mut()?;

    let current = state.current.as_ref()?;

    // Check cache
    let cache_key = CacheKey {
        wallpaper_id: current.id.clone(),
        width,
        height,
        scale_mode: state.scale_mode as u8,
    };

    if let Some(cached) = state.cache.get(&cache_key) {
        return Some(cached.clone());
    }

    // Scale wallpaper
    let scaled = scale_wallpaper(current, width, height, state.scale_mode, state.background_color);

    // Cache result
    state.cache.insert(cache_key, scaled.clone());

    Some(scaled)
}

/// Scale wallpaper to target size
fn scale_wallpaper(
    wallpaper: &WallpaperInfo,
    target_width: u32,
    target_height: u32,
    mode: ScaleMode,
    bg_color: u32,
) -> CachedWallpaper {
    let src_width = wallpaper.width;
    let src_height = wallpaper.height;

    let mut data = vec![0u8; (target_width * target_height * 4) as usize];

    // Fill background
    let bg_r = ((bg_color >> 16) & 0xFF) as u8;
    let bg_g = ((bg_color >> 8) & 0xFF) as u8;
    let bg_b = (bg_color & 0xFF) as u8;

    for i in 0..(target_width * target_height) as usize {
        data[i * 4] = bg_r;
        data[i * 4 + 1] = bg_g;
        data[i * 4 + 2] = bg_b;
        data[i * 4 + 3] = 255;
    }

    match mode {
        ScaleMode::Fill => {
            // Scale to fill, cropping if needed
            let scale = (target_width as f32 / src_width as f32)
                .max(target_height as f32 / src_height as f32);
            let scaled_w = (src_width as f32 * scale) as u32;
            let scaled_h = (src_height as f32 * scale) as u32;
            let offset_x = (target_width as i32 - scaled_w as i32) / 2;
            let offset_y = (target_height as i32 - scaled_h as i32) / 2;

            scale_and_blit(
                &wallpaper.data,
                src_width,
                src_height,
                &mut data,
                target_width,
                target_height,
                offset_x,
                offset_y,
                scaled_w,
                scaled_h,
            );
        }
        ScaleMode::Fit => {
            // Scale to fit, letterboxing if needed
            let scale = (target_width as f32 / src_width as f32)
                .min(target_height as f32 / src_height as f32);
            let scaled_w = (src_width as f32 * scale) as u32;
            let scaled_h = (src_height as f32 * scale) as u32;
            let offset_x = (target_width as i32 - scaled_w as i32) / 2;
            let offset_y = (target_height as i32 - scaled_h as i32) / 2;

            scale_and_blit(
                &wallpaper.data,
                src_width,
                src_height,
                &mut data,
                target_width,
                target_height,
                offset_x,
                offset_y,
                scaled_w,
                scaled_h,
            );
        }
        ScaleMode::Stretch => {
            // Stretch to fill exactly
            scale_and_blit(
                &wallpaper.data,
                src_width,
                src_height,
                &mut data,
                target_width,
                target_height,
                0,
                0,
                target_width,
                target_height,
            );
        }
        ScaleMode::Center => {
            // Center without scaling
            let offset_x = (target_width as i32 - src_width as i32) / 2;
            let offset_y = (target_height as i32 - src_height as i32) / 2;

            scale_and_blit(
                &wallpaper.data,
                src_width,
                src_height,
                &mut data,
                target_width,
                target_height,
                offset_x,
                offset_y,
                src_width,
                src_height,
            );
        }
        ScaleMode::Tile => {
            // Tile/repeat
            for tile_y in 0..((target_height + src_height - 1) / src_height) {
                for tile_x in 0..((target_width + src_width - 1) / src_width) {
                    scale_and_blit(
                        &wallpaper.data,
                        src_width,
                        src_height,
                        &mut data,
                        target_width,
                        target_height,
                        (tile_x * src_width) as i32,
                        (tile_y * src_height) as i32,
                        src_width,
                        src_height,
                    );
                }
            }
        }
        ScaleMode::Span => {
            // Same as fill for single monitor
            let scale = (target_width as f32 / src_width as f32)
                .max(target_height as f32 / src_height as f32);
            let scaled_w = (src_width as f32 * scale) as u32;
            let scaled_h = (src_height as f32 * scale) as u32;
            let offset_x = (target_width as i32 - scaled_w as i32) / 2;
            let offset_y = (target_height as i32 - scaled_h as i32) / 2;

            scale_and_blit(
                &wallpaper.data,
                src_width,
                src_height,
                &mut data,
                target_width,
                target_height,
                offset_x,
                offset_y,
                scaled_w,
                scaled_h,
            );
        }
    }

    CachedWallpaper {
        data,
        width: target_width,
        height: target_height,
    }
}

/// Scale and blit image
fn scale_and_blit(
    src: &[u8],
    src_width: u32,
    src_height: u32,
    dst: &mut [u8],
    dst_width: u32,
    dst_height: u32,
    offset_x: i32,
    offset_y: i32,
    scaled_width: u32,
    scaled_height: u32,
) {
    for dy in 0..scaled_height {
        let dst_y = offset_y + dy as i32;
        if dst_y < 0 || dst_y >= dst_height as i32 {
            continue;
        }

        for dx in 0..scaled_width {
            let dst_x = offset_x + dx as i32;
            if dst_x < 0 || dst_x >= dst_width as i32 {
                continue;
            }

            // Sample from source (nearest neighbor)
            let src_x = (dx as f32 / scaled_width as f32 * src_width as f32) as u32;
            let src_y = (dy as f32 / scaled_height as f32 * src_height as f32) as u32;

            if src_x < src_width && src_y < src_height {
                let src_idx = ((src_y * src_width + src_x) * 4) as usize;
                let dst_idx = ((dst_y as u32 * dst_width + dst_x as u32) * 4) as usize;

                if src_idx + 3 < src.len() && dst_idx + 3 < dst.len() {
                    dst[dst_idx] = src[src_idx];
                    dst[dst_idx + 1] = src[src_idx + 1];
                    dst[dst_idx + 2] = src[src_idx + 2];
                    dst[dst_idx + 3] = src[src_idx + 3];
                }
            }
        }
    }
}

/// Set scale mode
pub fn set_scale_mode(mode: ScaleMode) {
    let mut state = WALLPAPER_STATE.lock();
    if let Some(ref mut s) = *state {
        s.scale_mode = mode;
        s.cache.clear(); // Clear cache on mode change
    }
}

/// Get scale mode
pub fn get_scale_mode() -> ScaleMode {
    let state = WALLPAPER_STATE.lock();
    state.as_ref().map(|s| s.scale_mode).unwrap_or(ScaleMode::Fill)
}

/// Start slideshow
pub fn start_slideshow(wallpaper_ids: &[&str], interval_secs: u32, shuffle: bool) -> Result<(), WallpaperError> {
    let mut state = WALLPAPER_STATE.lock();
    let state = state.as_mut().ok_or(WallpaperError::NotInitialized)?;

    // Validate wallpaper IDs
    for id in wallpaper_ids {
        if !state.collection.contains_key(*id) {
            return Err(WallpaperError::NotFound);
        }
    }

    let wallpapers: Vec<String> = wallpaper_ids.iter().map(|s| s.to_string()).collect();

    state.slideshow = Some(SlideshowConfig {
        wallpapers,
        current_index: 0,
        interval_secs,
        shuffle,
        last_change_ms: 0,
        transition: SlideshowTransition::Fade,
    });

    // Set first wallpaper
    if let Some(id) = wallpaper_ids.first() {
        if let Some(wp) = state.collection.get(*id) {
            state.current = Some(wp.clone());
        }
    }

    Ok(())
}

/// Stop slideshow
pub fn stop_slideshow() {
    let mut state = WALLPAPER_STATE.lock();
    if let Some(ref mut s) = *state {
        s.slideshow = None;
    }
}

/// Update slideshow (call periodically)
pub fn update_slideshow(current_time_ms: u64) {
    let mut state = WALLPAPER_STATE.lock();
    if let Some(ref mut s) = *state {
        if let Some(ref mut slideshow) = s.slideshow {
            let elapsed = current_time_ms.saturating_sub(slideshow.last_change_ms);
            if elapsed >= (slideshow.interval_secs as u64 * 1000) {
                // Advance to next wallpaper
                slideshow.current_index = (slideshow.current_index + 1) % slideshow.wallpapers.len();
                slideshow.last_change_ms = current_time_ms;

                if let Some(id) = slideshow.wallpapers.get(slideshow.current_index) {
                    if let Some(wp) = s.collection.get(id) {
                        s.current = Some(wp.clone());
                        s.cache.clear();
                    }
                }
            }
        }
    }
}

/// List available wallpapers
pub fn list_wallpapers() -> Vec<(String, String)> {
    let state = WALLPAPER_STATE.lock();
    state.as_ref()
        .map(|s| {
            s.collection
                .values()
                .map(|w| (w.id.clone(), w.name.clone()))
                .collect()
        })
        .unwrap_or_default()
}

/// Add wallpaper from image data
pub fn add_wallpaper(id: &str, name: &str, width: u32, height: u32, data: Vec<u8>) -> Result<(), WallpaperError> {
    let mut state = WALLPAPER_STATE.lock();
    let state = state.as_mut().ok_or(WallpaperError::NotInitialized)?;

    if data.len() != (width * height * 4) as usize {
        return Err(WallpaperError::InvalidData);
    }

    let colors = extract_dominant_colors(&data, width, height);

    let wallpaper = WallpaperInfo {
        id: id.to_string(),
        name: name.to_string(),
        path: String::new(),
        wallpaper_type: WallpaperType::Static,
        width,
        height,
        data,
        thumbnail: None,
        colors,
        metadata: WallpaperMetadata::default(),
    };

    state.collection.insert(id.to_string(), wallpaper);
    Ok(())
}

/// Extract dominant colors from image
fn extract_dominant_colors(data: &[u8], width: u32, height: u32) -> DominantColors {
    // Simple algorithm: sample pixels and find most common colors
    let sample_count = ((width * height) as usize).min(1000);
    let step = (width * height) as usize / sample_count;

    let mut total_r: u64 = 0;
    let mut total_g: u64 = 0;
    let mut total_b: u64 = 0;
    let mut count: u64 = 0;

    for i in (0..(width * height) as usize).step_by(step) {
        let idx = i * 4;
        if idx + 2 < data.len() {
            total_r += data[idx] as u64;
            total_g += data[idx + 1] as u64;
            total_b += data[idx + 2] as u64;
            count += 1;
        }
    }

    let avg_r = if count > 0 { (total_r / count) as u32 } else { 128 };
    let avg_g = if count > 0 { (total_g / count) as u32 } else { 128 };
    let avg_b = if count > 0 { (total_b / count) as u32 } else { 128 };

    let primary = (avg_r << 16) | (avg_g << 8) | avg_b;
    let luminance = 0.299 * avg_r as f32 + 0.587 * avg_g as f32 + 0.114 * avg_b as f32;

    DominantColors {
        primary,
        secondary: primary,
        accent: primary,
        is_dark: luminance < 128.0,
    }
}

/// Remove wallpaper
pub fn remove_wallpaper(id: &str) -> Result<(), WallpaperError> {
    let mut state = WALLPAPER_STATE.lock();
    let state = state.as_mut().ok_or(WallpaperError::NotInitialized)?;

    if id == "default" {
        return Err(WallpaperError::CannotRemoveDefault);
    }

    state.collection.remove(id);

    // If removed current, switch to default
    if state.current.as_ref().map(|c| c.id.as_str()) == Some(id) {
        state.current = state.collection.get("default").cloned();
    }

    Ok(())
}

/// Clear wallpaper cache
pub fn clear_cache() {
    let mut state = WALLPAPER_STATE.lock();
    if let Some(ref mut s) = *state {
        s.cache.clear();
    }
}

/// Wallpaper error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallpaperError {
    NotInitialized,
    NotFound,
    InvalidData,
    CannotRemoveDefault,
    IoError,
}
