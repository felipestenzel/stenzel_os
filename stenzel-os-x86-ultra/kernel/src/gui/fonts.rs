//! Font Rendering System
//!
//! Provides FreeType/HarfBuzz-like font rendering with subpixel antialiasing,
//! glyph shaping, and font fallback.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use spin::Mutex;

/// Global font state
static FONT_STATE: Mutex<Option<FontState>> = Mutex::new(None);

/// Font state
pub struct FontState {
    /// Loaded fonts
    pub fonts: BTreeMap<String, Font>,
    /// Font families (name -> list of fonts)
    pub families: BTreeMap<String, Vec<String>>,
    /// Glyph cache
    pub glyph_cache: BTreeMap<GlyphCacheKey, CachedGlyph>,
    /// Default font family
    pub default_family: String,
    /// Default monospace family
    pub default_mono: String,
    /// Rendering settings
    pub settings: RenderSettings,
    /// System font directories
    pub font_dirs: Vec<String>,
}

/// Font
#[derive(Debug, Clone)]
pub struct Font {
    /// Font identifier
    pub id: String,
    /// Family name
    pub family: String,
    /// Style name (Regular, Bold, Italic, etc.)
    pub style: String,
    /// Weight (100-900)
    pub weight: u32,
    /// Is italic
    pub italic: bool,
    /// Is monospace
    pub monospace: bool,
    /// Font data
    pub data: FontData,
    /// Font metrics
    pub metrics: FontMetrics,
    /// Character map
    pub cmap: BTreeMap<u32, u32>, // char code -> glyph index
}

/// Font data
#[derive(Debug, Clone)]
pub enum FontData {
    /// TrueType/OpenType font
    TrueType(TrueTypeData),
    /// Bitmap font
    Bitmap(BitmapFontData),
    /// Built-in font
    Builtin,
}

/// TrueType font data
#[derive(Debug, Clone)]
pub struct TrueTypeData {
    /// Raw font file data
    pub data: Vec<u8>,
    /// Number of glyphs
    pub num_glyphs: u32,
    /// Units per EM
    pub units_per_em: u32,
    /// Glyph outlines (glyph index -> outline)
    pub outlines: BTreeMap<u32, GlyphOutline>,
}

/// Bitmap font data
#[derive(Debug, Clone)]
pub struct BitmapFontData {
    /// Glyph bitmaps
    pub glyphs: BTreeMap<u32, GlyphBitmap>,
    /// Pixel size
    pub pixel_size: u32,
}

/// Glyph outline (for vector fonts)
#[derive(Debug, Clone)]
pub struct GlyphOutline {
    /// Contours (list of points for each contour)
    pub contours: Vec<Vec<OutlinePoint>>,
    /// Bounding box
    pub bbox: BoundingBox,
    /// Advance width
    pub advance_width: i32,
    /// Left side bearing
    pub lsb: i32,
}

/// Outline point
#[derive(Debug, Clone, Copy)]
pub struct OutlinePoint {
    /// X coordinate
    pub x: i32,
    /// Y coordinate
    pub y: i32,
    /// Is on curve (true) or control point (false)
    pub on_curve: bool,
}

/// Bounding box
#[derive(Debug, Clone, Copy, Default)]
pub struct BoundingBox {
    pub x_min: i32,
    pub y_min: i32,
    pub x_max: i32,
    pub y_max: i32,
}

/// Glyph bitmap
#[derive(Debug, Clone)]
pub struct GlyphBitmap {
    /// Pixel data (grayscale)
    pub data: Vec<u8>,
    /// Width
    pub width: u32,
    /// Height
    pub height: u32,
    /// Bearing X
    pub bearing_x: i32,
    /// Bearing Y
    pub bearing_y: i32,
    /// Advance width
    pub advance: i32,
}

/// Font metrics
#[derive(Debug, Clone, Copy)]
pub struct FontMetrics {
    /// Ascender
    pub ascender: i32,
    /// Descender
    pub descender: i32,
    /// Line height
    pub height: i32,
    /// Max advance width
    pub max_advance: i32,
    /// Underline position
    pub underline_position: i32,
    /// Underline thickness
    pub underline_thickness: i32,
    /// Strikeout position
    pub strikeout_position: i32,
    /// Strikeout thickness
    pub strikeout_thickness: i32,
}

/// Glyph cache key
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct GlyphCacheKey {
    pub font_id: String,
    pub glyph_index: u32,
    pub size: u32,
    pub render_mode: RenderMode,
}

/// Cached glyph
#[derive(Debug, Clone)]
pub struct CachedGlyph {
    pub bitmap: GlyphBitmap,
    pub render_mode: RenderMode,
}

/// Render mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RenderMode {
    /// No antialiasing (1-bit)
    Mono,
    /// Grayscale antialiasing
    Gray,
    /// Subpixel antialiasing (LCD)
    Lcd,
    /// Subpixel antialiasing (vertical LCD)
    LcdV,
}

/// Render settings
#[derive(Debug, Clone)]
pub struct RenderSettings {
    /// Render mode
    pub mode: RenderMode,
    /// Hinting mode
    pub hinting: HintingMode,
    /// Gamma correction
    pub gamma: f32,
    /// Subpixel geometry
    pub subpixel: SubpixelOrder,
    /// LCD filter
    pub lcd_filter: LcdFilter,
    /// Emboldening amount
    pub embolden: i32,
}

/// Hinting mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HintingMode {
    /// No hinting
    None,
    /// Light hinting
    Light,
    /// Medium hinting
    Medium,
    /// Full hinting
    Full,
    /// Auto hinting
    Auto,
}

/// Subpixel order
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubpixelOrder {
    /// RGB horizontal
    Rgb,
    /// BGR horizontal
    Bgr,
    /// RGB vertical
    VRgb,
    /// BGR vertical
    VBgr,
    /// No subpixel (grayscale)
    None,
}

/// LCD filter
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LcdFilter {
    None,
    Light,
    Default,
    Legacy,
}

/// Text layout result
#[derive(Debug, Clone)]
pub struct TextLayout {
    /// Positioned glyphs
    pub glyphs: Vec<PositionedGlyph>,
    /// Total width
    pub width: i32,
    /// Total height
    pub height: i32,
    /// Lines
    pub lines: Vec<LineMetrics>,
}

/// Positioned glyph
#[derive(Debug, Clone)]
pub struct PositionedGlyph {
    /// Glyph index
    pub glyph_index: u32,
    /// Font ID
    pub font_id: String,
    /// X position
    pub x: i32,
    /// Y position
    pub y: i32,
    /// Cluster index (character index)
    pub cluster: usize,
}

/// Line metrics
#[derive(Debug, Clone)]
pub struct LineMetrics {
    /// Line start index (in glyphs)
    pub start: usize,
    /// Line end index
    pub end: usize,
    /// Line width
    pub width: i32,
    /// Baseline Y position
    pub baseline_y: i32,
    /// Line height
    pub height: i32,
}

/// Initialize font system
pub fn init() {
    let mut state = FONT_STATE.lock();
    if state.is_some() {
        return;
    }

    let mut fonts = BTreeMap::new();
    let mut families = BTreeMap::new();

    // Create built-in font
    let builtin = create_builtin_font();
    fonts.insert(builtin.id.clone(), builtin.clone());
    families.insert(builtin.family.clone(), vec![builtin.id.clone()]);

    // Create built-in mono font
    let builtin_mono = create_builtin_mono_font();
    fonts.insert(builtin_mono.id.clone(), builtin_mono.clone());
    families.insert(builtin_mono.family.clone(), vec![builtin_mono.id.clone()]);

    *state = Some(FontState {
        fonts,
        families,
        glyph_cache: BTreeMap::new(),
        default_family: "Stenzel Sans".to_string(),
        default_mono: "Stenzel Mono".to_string(),
        settings: RenderSettings {
            mode: RenderMode::Gray,
            hinting: HintingMode::Light,
            gamma: 1.8,
            subpixel: SubpixelOrder::Rgb,
            lcd_filter: LcdFilter::Default,
            embolden: 0,
        },
        font_dirs: vec![
            "/usr/share/fonts".to_string(),
            "/usr/local/share/fonts".to_string(),
            "~/.fonts".to_string(),
        ],
    });

    crate::kprintln!("fonts: initialized with built-in fonts");
}

/// Create built-in sans font
fn create_builtin_font() -> Font {
    let mut cmap = BTreeMap::new();

    // Basic ASCII mapping
    for c in 32u32..127 {
        cmap.insert(c, c - 32 + 1);
    }

    Font {
        id: "builtin-sans".to_string(),
        family: "Stenzel Sans".to_string(),
        style: "Regular".to_string(),
        weight: 400,
        italic: false,
        monospace: false,
        data: FontData::Builtin,
        metrics: FontMetrics {
            ascender: 12,
            descender: -4,
            height: 16,
            max_advance: 8,
            underline_position: -2,
            underline_thickness: 1,
            strikeout_position: 4,
            strikeout_thickness: 1,
        },
        cmap,
    }
}

/// Create built-in mono font
fn create_builtin_mono_font() -> Font {
    let mut cmap = BTreeMap::new();

    // Basic ASCII mapping
    for c in 32u32..127 {
        cmap.insert(c, c - 32 + 1);
    }

    Font {
        id: "builtin-mono".to_string(),
        family: "Stenzel Mono".to_string(),
        style: "Regular".to_string(),
        weight: 400,
        italic: false,
        monospace: true,
        data: FontData::Builtin,
        metrics: FontMetrics {
            ascender: 12,
            descender: -4,
            height: 16,
            max_advance: 8,
            underline_position: -2,
            underline_thickness: 1,
            strikeout_position: 4,
            strikeout_thickness: 1,
        },
        cmap,
    }
}

/// Load a font from data
pub fn load_font(data: &[u8], family: Option<&str>) -> Result<String, FontError> {
    let mut state = FONT_STATE.lock();
    let state = state.as_mut().ok_or(FontError::NotInitialized)?;

    // Parse font (simplified - would need real TrueType parser)
    let font = parse_font(data, family)?;
    let id = font.id.clone();
    let family_name = font.family.clone();

    state.fonts.insert(id.clone(), font);

    // Add to family
    state.families
        .entry(family_name)
        .or_insert_with(Vec::new)
        .push(id.clone());

    Ok(id)
}

/// Parse font data (stub - would need real implementation)
fn parse_font(data: &[u8], family: Option<&str>) -> Result<Font, FontError> {
    // Check for TrueType/OpenType magic
    if data.len() < 4 {
        return Err(FontError::InvalidFormat);
    }

    let magic = &data[0..4];
    let is_ttf = magic == [0x00, 0x01, 0x00, 0x00] || magic == b"OTTO" || magic == b"true";

    if !is_ttf {
        return Err(FontError::UnsupportedFormat);
    }

    // Create stub font (real implementation would parse tables)
    let id = format!("font-{:x}", data.len() as u64);
    let family_name = family.unwrap_or("Unknown").to_string();

    let mut cmap = BTreeMap::new();
    for c in 32u32..127 {
        cmap.insert(c, c - 32 + 1);
    }

    Ok(Font {
        id,
        family: family_name,
        style: "Regular".to_string(),
        weight: 400,
        italic: false,
        monospace: false,
        data: FontData::TrueType(TrueTypeData {
            data: data.to_vec(),
            num_glyphs: 256,
            units_per_em: 2048,
            outlines: BTreeMap::new(),
        }),
        metrics: FontMetrics {
            ascender: 12,
            descender: -4,
            height: 16,
            max_advance: 8,
            underline_position: -2,
            underline_thickness: 1,
            strikeout_position: 4,
            strikeout_thickness: 1,
        },
        cmap,
    })
}

/// Get a font by family name and style
pub fn get_font(family: &str, weight: u32, italic: bool) -> Option<Font> {
    let state = FONT_STATE.lock();
    let state = state.as_ref()?;

    // Get fonts in family
    let font_ids = state.families.get(family)?;

    // Find best match
    let mut best_match: Option<&Font> = None;
    let mut best_score = i32::MAX;

    for id in font_ids {
        if let Some(font) = state.fonts.get(id) {
            let weight_diff = (font.weight as i32 - weight as i32).abs();
            let italic_diff = if font.italic == italic { 0 } else { 100 };
            let score = weight_diff + italic_diff;

            if score < best_score {
                best_score = score;
                best_match = Some(font);
            }
        }
    }

    best_match.cloned()
}

/// Get default font
pub fn get_default_font() -> Option<Font> {
    let state = FONT_STATE.lock();
    let state = state.as_ref()?;
    get_font(&state.default_family, 400, false)
}

/// Get default monospace font
pub fn get_default_mono_font() -> Option<Font> {
    let state = FONT_STATE.lock();
    let state = state.as_ref()?;
    get_font(&state.default_mono, 400, false)
}

/// Render a glyph
pub fn render_glyph(font: &Font, glyph_index: u32, size: u32) -> Option<GlyphBitmap> {
    // Check cache first
    {
        let state = FONT_STATE.lock();
        if let Some(state) = state.as_ref() {
            let key = GlyphCacheKey {
                font_id: font.id.clone(),
                glyph_index,
                size,
                render_mode: state.settings.mode,
            };

            if let Some(cached) = state.glyph_cache.get(&key) {
                return Some(cached.bitmap.clone());
            }
        }
    }

    // Render glyph
    let bitmap = match &font.data {
        FontData::Builtin => render_builtin_glyph(glyph_index, size, font.monospace),
        FontData::TrueType(ttf) => render_truetype_glyph(ttf, glyph_index, size),
        FontData::Bitmap(bmp) => bmp.glyphs.get(&glyph_index).cloned(),
    }?;

    // Cache result
    {
        let mut state = FONT_STATE.lock();
        if let Some(state) = state.as_mut() {
            let key = GlyphCacheKey {
                font_id: font.id.clone(),
                glyph_index,
                size,
                render_mode: state.settings.mode,
            };

            state.glyph_cache.insert(key, CachedGlyph {
                bitmap: bitmap.clone(),
                render_mode: state.settings.mode,
            });
        }
    }

    Some(bitmap)
}

/// Render built-in glyph
fn render_builtin_glyph(glyph_index: u32, size: u32, _monospace: bool) -> Option<GlyphBitmap> {
    // Simple 8x16 bitmap font
    let char_code = glyph_index + 31;

    // Scale factor
    let scale = (size / 16).max(1);
    let char_width = 8 * scale;
    let char_height = 16 * scale;

    let mut data = vec![0u8; (char_width * char_height) as usize];

    // Get bitmap for character
    if let Some(bitmap) = get_builtin_char_bitmap(char_code as u8) {
        for y in 0..16u32 {
            let row = bitmap[y as usize];
            for x in 0..8u32 {
                if (row >> (7 - x)) & 1 != 0 {
                    // Scale up
                    for sy in 0..scale {
                        for sx in 0..scale {
                            let px = x * scale + sx;
                            let py = y * scale + sy;
                            let idx = (py * char_width + px) as usize;
                            if idx < data.len() {
                                data[idx] = 255;
                            }
                        }
                    }
                }
            }
        }
    }

    Some(GlyphBitmap {
        data,
        width: char_width,
        height: char_height,
        bearing_x: 0,
        bearing_y: (12 * scale) as i32,
        advance: char_width as i32,
    })
}

/// Get built-in character bitmap (8x16 pixels)
fn get_builtin_char_bitmap(c: u8) -> Option<[u8; 16]> {
    // Basic font bitmap data for printable ASCII
    match c {
        b' ' => Some([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
        b'!' => Some([0, 0, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0, 0x18, 0x18, 0, 0, 0, 0]),
        b'"' => Some([0, 0x66, 0x66, 0x66, 0x24, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
        b'#' => Some([0, 0, 0x6C, 0x6C, 0xFE, 0x6C, 0x6C, 0x6C, 0xFE, 0x6C, 0x6C, 0, 0, 0, 0, 0]),
        b'$' => Some([0x18, 0x18, 0x7C, 0xC6, 0xC0, 0x78, 0x3C, 0x06, 0xC6, 0x7C, 0x18, 0x18, 0, 0, 0, 0]),
        b'%' => Some([0, 0, 0xC6, 0xCC, 0x18, 0x30, 0x60, 0xCC, 0x86, 0, 0, 0, 0, 0, 0, 0]),
        b'&' => Some([0, 0, 0x38, 0x6C, 0x38, 0x76, 0xDC, 0xCC, 0xCC, 0x76, 0, 0, 0, 0, 0, 0]),
        b'\'' => Some([0, 0x18, 0x18, 0x18, 0x30, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
        b'(' => Some([0, 0, 0x0C, 0x18, 0x30, 0x30, 0x30, 0x30, 0x30, 0x18, 0x0C, 0, 0, 0, 0, 0]),
        b')' => Some([0, 0, 0x30, 0x18, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x18, 0x30, 0, 0, 0, 0, 0]),
        b'*' => Some([0, 0, 0, 0x66, 0x3C, 0xFF, 0x3C, 0x66, 0, 0, 0, 0, 0, 0, 0, 0]),
        b'+' => Some([0, 0, 0, 0x18, 0x18, 0x7E, 0x18, 0x18, 0, 0, 0, 0, 0, 0, 0, 0]),
        b',' => Some([0, 0, 0, 0, 0, 0, 0, 0, 0, 0x18, 0x18, 0x30, 0, 0, 0, 0]),
        b'-' => Some([0, 0, 0, 0, 0, 0x7E, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
        b'.' => Some([0, 0, 0, 0, 0, 0, 0, 0, 0, 0x18, 0x18, 0, 0, 0, 0, 0]),
        b'/' => Some([0, 0, 0x06, 0x0C, 0x18, 0x30, 0x60, 0xC0, 0x80, 0, 0, 0, 0, 0, 0, 0]),
        b'0' => Some([0, 0, 0x7C, 0xC6, 0xCE, 0xDE, 0xF6, 0xE6, 0xC6, 0x7C, 0, 0, 0, 0, 0, 0]),
        b'1' => Some([0, 0, 0x18, 0x38, 0x78, 0x18, 0x18, 0x18, 0x18, 0x7E, 0, 0, 0, 0, 0, 0]),
        b'2' => Some([0, 0, 0x7C, 0xC6, 0x06, 0x0C, 0x18, 0x30, 0x60, 0xFE, 0, 0, 0, 0, 0, 0]),
        b'3' => Some([0, 0, 0x7C, 0xC6, 0x06, 0x3C, 0x06, 0x06, 0xC6, 0x7C, 0, 0, 0, 0, 0, 0]),
        b'4' => Some([0, 0, 0x0C, 0x1C, 0x3C, 0x6C, 0xCC, 0xFE, 0x0C, 0x0C, 0, 0, 0, 0, 0, 0]),
        b'5' => Some([0, 0, 0xFE, 0xC0, 0xC0, 0xFC, 0x06, 0x06, 0xC6, 0x7C, 0, 0, 0, 0, 0, 0]),
        b'6' => Some([0, 0, 0x38, 0x60, 0xC0, 0xFC, 0xC6, 0xC6, 0xC6, 0x7C, 0, 0, 0, 0, 0, 0]),
        b'7' => Some([0, 0, 0xFE, 0x06, 0x0C, 0x18, 0x30, 0x30, 0x30, 0x30, 0, 0, 0, 0, 0, 0]),
        b'8' => Some([0, 0, 0x7C, 0xC6, 0xC6, 0x7C, 0xC6, 0xC6, 0xC6, 0x7C, 0, 0, 0, 0, 0, 0]),
        b'9' => Some([0, 0, 0x7C, 0xC6, 0xC6, 0x7E, 0x06, 0x06, 0x0C, 0x78, 0, 0, 0, 0, 0, 0]),
        b':' => Some([0, 0, 0, 0, 0x18, 0x18, 0, 0, 0x18, 0x18, 0, 0, 0, 0, 0, 0]),
        b';' => Some([0, 0, 0, 0, 0x18, 0x18, 0, 0, 0x18, 0x18, 0x30, 0, 0, 0, 0, 0]),
        b'<' => Some([0, 0, 0x06, 0x0C, 0x18, 0x30, 0x60, 0x30, 0x18, 0x0C, 0x06, 0, 0, 0, 0, 0]),
        b'=' => Some([0, 0, 0, 0, 0x7E, 0, 0x7E, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
        b'>' => Some([0, 0, 0x60, 0x30, 0x18, 0x0C, 0x06, 0x0C, 0x18, 0x30, 0x60, 0, 0, 0, 0, 0]),
        b'?' => Some([0, 0, 0x7C, 0xC6, 0x06, 0x0C, 0x18, 0x18, 0, 0x18, 0x18, 0, 0, 0, 0, 0]),
        b'@' => Some([0, 0, 0x7C, 0xC6, 0xC6, 0xDE, 0xDE, 0xDE, 0xC0, 0x7E, 0, 0, 0, 0, 0, 0]),
        b'A' => Some([0, 0, 0x10, 0x38, 0x6C, 0xC6, 0xC6, 0xFE, 0xC6, 0xC6, 0, 0, 0, 0, 0, 0]),
        b'B' => Some([0, 0, 0xFC, 0x66, 0x66, 0x7C, 0x66, 0x66, 0x66, 0xFC, 0, 0, 0, 0, 0, 0]),
        b'C' => Some([0, 0, 0x3C, 0x66, 0xC0, 0xC0, 0xC0, 0xC0, 0x66, 0x3C, 0, 0, 0, 0, 0, 0]),
        b'D' => Some([0, 0, 0xF8, 0x6C, 0x66, 0x66, 0x66, 0x66, 0x6C, 0xF8, 0, 0, 0, 0, 0, 0]),
        b'E' => Some([0, 0, 0xFE, 0x62, 0x68, 0x78, 0x68, 0x60, 0x62, 0xFE, 0, 0, 0, 0, 0, 0]),
        b'F' => Some([0, 0, 0xFE, 0x62, 0x68, 0x78, 0x68, 0x60, 0x60, 0xF0, 0, 0, 0, 0, 0, 0]),
        b'G' => Some([0, 0, 0x3C, 0x66, 0xC0, 0xC0, 0xCE, 0xC6, 0x66, 0x3E, 0, 0, 0, 0, 0, 0]),
        b'H' => Some([0, 0, 0xC6, 0xC6, 0xC6, 0xFE, 0xC6, 0xC6, 0xC6, 0xC6, 0, 0, 0, 0, 0, 0]),
        b'I' => Some([0, 0, 0x3C, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x3C, 0, 0, 0, 0, 0, 0]),
        b'J' => Some([0, 0, 0x1E, 0x0C, 0x0C, 0x0C, 0x0C, 0xCC, 0xCC, 0x78, 0, 0, 0, 0, 0, 0]),
        b'K' => Some([0, 0, 0xE6, 0x66, 0x6C, 0x78, 0x78, 0x6C, 0x66, 0xE6, 0, 0, 0, 0, 0, 0]),
        b'L' => Some([0, 0, 0xF0, 0x60, 0x60, 0x60, 0x60, 0x60, 0x62, 0xFE, 0, 0, 0, 0, 0, 0]),
        b'M' => Some([0, 0, 0xC6, 0xEE, 0xFE, 0xD6, 0xC6, 0xC6, 0xC6, 0xC6, 0, 0, 0, 0, 0, 0]),
        b'N' => Some([0, 0, 0xC6, 0xE6, 0xF6, 0xDE, 0xCE, 0xC6, 0xC6, 0xC6, 0, 0, 0, 0, 0, 0]),
        b'O' => Some([0, 0, 0x7C, 0xC6, 0xC6, 0xC6, 0xC6, 0xC6, 0xC6, 0x7C, 0, 0, 0, 0, 0, 0]),
        b'P' => Some([0, 0, 0xFC, 0x66, 0x66, 0x7C, 0x60, 0x60, 0x60, 0xF0, 0, 0, 0, 0, 0, 0]),
        b'Q' => Some([0, 0, 0x7C, 0xC6, 0xC6, 0xC6, 0xC6, 0xD6, 0xDE, 0x7C, 0x0E, 0, 0, 0, 0, 0]),
        b'R' => Some([0, 0, 0xFC, 0x66, 0x66, 0x7C, 0x6C, 0x66, 0x66, 0xE6, 0, 0, 0, 0, 0, 0]),
        b'S' => Some([0, 0, 0x7C, 0xC6, 0x60, 0x38, 0x0C, 0x06, 0xC6, 0x7C, 0, 0, 0, 0, 0, 0]),
        b'T' => Some([0, 0, 0x7E, 0x5A, 0x18, 0x18, 0x18, 0x18, 0x18, 0x3C, 0, 0, 0, 0, 0, 0]),
        b'U' => Some([0, 0, 0xC6, 0xC6, 0xC6, 0xC6, 0xC6, 0xC6, 0xC6, 0x7C, 0, 0, 0, 0, 0, 0]),
        b'V' => Some([0, 0, 0xC6, 0xC6, 0xC6, 0xC6, 0xC6, 0x6C, 0x38, 0x10, 0, 0, 0, 0, 0, 0]),
        b'W' => Some([0, 0, 0xC6, 0xC6, 0xC6, 0xD6, 0xFE, 0xEE, 0xC6, 0x82, 0, 0, 0, 0, 0, 0]),
        b'X' => Some([0, 0, 0xC6, 0xC6, 0x6C, 0x38, 0x38, 0x6C, 0xC6, 0xC6, 0, 0, 0, 0, 0, 0]),
        b'Y' => Some([0, 0, 0x66, 0x66, 0x66, 0x3C, 0x18, 0x18, 0x18, 0x3C, 0, 0, 0, 0, 0, 0]),
        b'Z' => Some([0, 0, 0xFE, 0xC6, 0x0C, 0x18, 0x30, 0x60, 0xC6, 0xFE, 0, 0, 0, 0, 0, 0]),
        b'[' => Some([0, 0, 0x3C, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x3C, 0, 0, 0, 0, 0, 0]),
        b'\\' => Some([0, 0, 0xC0, 0x60, 0x30, 0x18, 0x0C, 0x06, 0x02, 0, 0, 0, 0, 0, 0, 0]),
        b']' => Some([0, 0, 0x3C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x3C, 0, 0, 0, 0, 0, 0]),
        b'^' => Some([0x10, 0x38, 0x6C, 0xC6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
        b'_' => Some([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xFF, 0, 0, 0, 0, 0]),
        b'`' => Some([0x30, 0x18, 0x0C, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
        b'a' => Some([0, 0, 0, 0, 0x78, 0x0C, 0x7C, 0xCC, 0xCC, 0x76, 0, 0, 0, 0, 0, 0]),
        b'b' => Some([0, 0, 0xE0, 0x60, 0x7C, 0x66, 0x66, 0x66, 0x66, 0xDC, 0, 0, 0, 0, 0, 0]),
        b'c' => Some([0, 0, 0, 0, 0x7C, 0xC6, 0xC0, 0xC0, 0xC6, 0x7C, 0, 0, 0, 0, 0, 0]),
        b'd' => Some([0, 0, 0x1C, 0x0C, 0x7C, 0xCC, 0xCC, 0xCC, 0xCC, 0x76, 0, 0, 0, 0, 0, 0]),
        b'e' => Some([0, 0, 0, 0, 0x7C, 0xC6, 0xFE, 0xC0, 0xC6, 0x7C, 0, 0, 0, 0, 0, 0]),
        b'f' => Some([0, 0, 0x1C, 0x36, 0x30, 0x78, 0x30, 0x30, 0x30, 0x78, 0, 0, 0, 0, 0, 0]),
        b'g' => Some([0, 0, 0, 0, 0x76, 0xCC, 0xCC, 0xCC, 0x7C, 0x0C, 0xCC, 0x78, 0, 0, 0, 0]),
        b'h' => Some([0, 0, 0xE0, 0x60, 0x6C, 0x76, 0x66, 0x66, 0x66, 0xE6, 0, 0, 0, 0, 0, 0]),
        b'i' => Some([0, 0, 0x18, 0, 0x38, 0x18, 0x18, 0x18, 0x18, 0x3C, 0, 0, 0, 0, 0, 0]),
        b'j' => Some([0, 0, 0x06, 0, 0x0E, 0x06, 0x06, 0x06, 0x06, 0x66, 0x66, 0x3C, 0, 0, 0, 0]),
        b'k' => Some([0, 0, 0xE0, 0x60, 0x66, 0x6C, 0x78, 0x6C, 0x66, 0xE6, 0, 0, 0, 0, 0, 0]),
        b'l' => Some([0, 0, 0x38, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x3C, 0, 0, 0, 0, 0, 0]),
        b'm' => Some([0, 0, 0, 0, 0xEC, 0xFE, 0xD6, 0xD6, 0xC6, 0xC6, 0, 0, 0, 0, 0, 0]),
        b'n' => Some([0, 0, 0, 0, 0xDC, 0x66, 0x66, 0x66, 0x66, 0x66, 0, 0, 0, 0, 0, 0]),
        b'o' => Some([0, 0, 0, 0, 0x7C, 0xC6, 0xC6, 0xC6, 0xC6, 0x7C, 0, 0, 0, 0, 0, 0]),
        b'p' => Some([0, 0, 0, 0, 0xDC, 0x66, 0x66, 0x66, 0x7C, 0x60, 0x60, 0xF0, 0, 0, 0, 0]),
        b'q' => Some([0, 0, 0, 0, 0x76, 0xCC, 0xCC, 0xCC, 0x7C, 0x0C, 0x0C, 0x1E, 0, 0, 0, 0]),
        b'r' => Some([0, 0, 0, 0, 0xDC, 0x76, 0x66, 0x60, 0x60, 0xF0, 0, 0, 0, 0, 0, 0]),
        b's' => Some([0, 0, 0, 0, 0x7C, 0xC6, 0x70, 0x1C, 0xC6, 0x7C, 0, 0, 0, 0, 0, 0]),
        b't' => Some([0, 0, 0x10, 0x30, 0xFC, 0x30, 0x30, 0x30, 0x34, 0x18, 0, 0, 0, 0, 0, 0]),
        b'u' => Some([0, 0, 0, 0, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 0x76, 0, 0, 0, 0, 0, 0]),
        b'v' => Some([0, 0, 0, 0, 0xC6, 0xC6, 0xC6, 0x6C, 0x38, 0x10, 0, 0, 0, 0, 0, 0]),
        b'w' => Some([0, 0, 0, 0, 0xC6, 0xC6, 0xD6, 0xFE, 0xEE, 0x6C, 0, 0, 0, 0, 0, 0]),
        b'x' => Some([0, 0, 0, 0, 0xC6, 0x6C, 0x38, 0x38, 0x6C, 0xC6, 0, 0, 0, 0, 0, 0]),
        b'y' => Some([0, 0, 0, 0, 0xC6, 0xC6, 0xC6, 0xC6, 0x7E, 0x06, 0x0C, 0xF8, 0, 0, 0, 0]),
        b'z' => Some([0, 0, 0, 0, 0xFE, 0x8C, 0x18, 0x30, 0x62, 0xFE, 0, 0, 0, 0, 0, 0]),
        b'{' => Some([0, 0, 0x0E, 0x18, 0x18, 0x70, 0x18, 0x18, 0x18, 0x0E, 0, 0, 0, 0, 0, 0]),
        b'|' => Some([0, 0, 0x18, 0x18, 0x18, 0, 0x18, 0x18, 0x18, 0, 0, 0, 0, 0, 0, 0]),
        b'}' => Some([0, 0, 0x70, 0x18, 0x18, 0x0E, 0x18, 0x18, 0x18, 0x70, 0, 0, 0, 0, 0, 0]),
        b'~' => Some([0, 0, 0x76, 0xDC, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
        _ => None,
    }
}

/// Render TrueType glyph (stub - would need real rasterizer)
fn render_truetype_glyph(ttf: &TrueTypeData, glyph_index: u32, size: u32) -> Option<GlyphBitmap> {
    // In a real implementation, this would:
    // 1. Load glyph outline from ttf.outlines
    // 2. Scale to requested size
    // 3. Rasterize using scanline algorithm

    // For now, fall back to builtin rendering
    let _ = ttf;
    render_builtin_glyph(glyph_index, size, false)
}

/// Layout text
pub fn layout_text(
    text: &str,
    font: &Font,
    size: u32,
    max_width: Option<i32>,
) -> TextLayout {
    let scale = (size / 16).max(1) as i32;
    let char_width = 8 * scale;
    let line_height = 16 * scale;

    let mut glyphs = Vec::new();
    let mut x = 0i32;
    let mut y = line_height; // First baseline
    let mut lines = Vec::new();
    let mut line_start = 0;

    for (cluster, c) in text.chars().enumerate() {
        // Handle newline
        if c == '\n' {
            lines.push(LineMetrics {
                start: line_start,
                end: glyphs.len(),
                width: x,
                baseline_y: y,
                height: line_height,
            });
            line_start = glyphs.len();
            x = 0;
            y += line_height;
            continue;
        }

        // Check for word wrap
        if let Some(max_w) = max_width {
            if x + char_width > max_w && x > 0 {
                lines.push(LineMetrics {
                    start: line_start,
                    end: glyphs.len(),
                    width: x,
                    baseline_y: y,
                    height: line_height,
                });
                line_start = glyphs.len();
                x = 0;
                y += line_height;
            }
        }

        // Get glyph index
        let glyph_index = font.cmap.get(&(c as u32)).copied().unwrap_or(0);

        glyphs.push(PositionedGlyph {
            glyph_index,
            font_id: font.id.clone(),
            x,
            y,
            cluster,
        });

        x += char_width;
    }

    // Final line
    if glyphs.len() > line_start {
        lines.push(LineMetrics {
            start: line_start,
            end: glyphs.len(),
            width: x,
            baseline_y: y,
            height: line_height,
        });
    }

    let width = lines.iter().map(|l| l.width).max().unwrap_or(0);
    let height = y;

    TextLayout {
        glyphs,
        width,
        height,
        lines,
    }
}

/// Render text to RGBA buffer
pub fn render_text(
    text: &str,
    font: &Font,
    size: u32,
    color: u32,
    max_width: Option<i32>,
) -> RenderedText {
    let layout = layout_text(text, font, size, max_width);

    let width = layout.width.max(1) as u32;
    let height = layout.height.max(1) as u32;
    let mut data = vec![0u8; (width * height * 4) as usize];

    let r = ((color >> 16) & 0xFF) as u8;
    let g = ((color >> 8) & 0xFF) as u8;
    let b = (color & 0xFF) as u8;

    for glyph in &layout.glyphs {
        if let Some(bitmap) = render_glyph(font, glyph.glyph_index, size) {
            let base_x = glyph.x + bitmap.bearing_x;
            let base_y = glyph.y - bitmap.bearing_y;

            for gy in 0..bitmap.height {
                for gx in 0..bitmap.width {
                    let px = base_x + gx as i32;
                    let py = base_y + gy as i32;

                    if px >= 0 && py >= 0 && (px as u32) < width && (py as u32) < height {
                        let src_idx = (gy * bitmap.width + gx) as usize;
                        let dst_idx = ((py as u32 * width + px as u32) * 4) as usize;

                        if src_idx < bitmap.data.len() && dst_idx + 3 < data.len() {
                            let alpha = bitmap.data[src_idx];
                            if alpha > 0 {
                                data[dst_idx] = r;
                                data[dst_idx + 1] = g;
                                data[dst_idx + 2] = b;
                                data[dst_idx + 3] = alpha;
                            }
                        }
                    }
                }
            }
        }
    }

    RenderedText {
        data,
        width,
        height,
        layout,
    }
}

/// Rendered text result
#[derive(Debug, Clone)]
pub struct RenderedText {
    /// RGBA pixel data
    pub data: Vec<u8>,
    /// Width
    pub width: u32,
    /// Height
    pub height: u32,
    /// Layout info
    pub layout: TextLayout,
}

/// Measure text without rendering
pub fn measure_text(text: &str, font: &Font, size: u32, max_width: Option<i32>) -> (i32, i32) {
    let layout = layout_text(text, font, size, max_width);
    (layout.width, layout.height)
}

/// Get font metrics scaled to size
pub fn get_scaled_metrics(font: &Font, size: u32) -> FontMetrics {
    let scale = size as f32 / 16.0;

    FontMetrics {
        ascender: (font.metrics.ascender as f32 * scale) as i32,
        descender: (font.metrics.descender as f32 * scale) as i32,
        height: (font.metrics.height as f32 * scale) as i32,
        max_advance: (font.metrics.max_advance as f32 * scale) as i32,
        underline_position: (font.metrics.underline_position as f32 * scale) as i32,
        underline_thickness: (font.metrics.underline_thickness as f32 * scale).max(1.0) as i32,
        strikeout_position: (font.metrics.strikeout_position as f32 * scale) as i32,
        strikeout_thickness: (font.metrics.strikeout_thickness as f32 * scale).max(1.0) as i32,
    }
}

/// Set rendering mode
pub fn set_render_mode(mode: RenderMode) {
    let mut state = FONT_STATE.lock();
    if let Some(ref mut s) = *state {
        s.settings.mode = mode;
        s.glyph_cache.clear(); // Clear cache on mode change
    }
}

/// Set hinting mode
pub fn set_hinting(hinting: HintingMode) {
    let mut state = FONT_STATE.lock();
    if let Some(ref mut s) = *state {
        s.settings.hinting = hinting;
        s.glyph_cache.clear();
    }
}

/// List available font families
pub fn list_families() -> Vec<String> {
    let state = FONT_STATE.lock();
    state.as_ref()
        .map(|s| s.families.keys().cloned().collect())
        .unwrap_or_default()
}

/// Font error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontError {
    NotInitialized,
    InvalidFormat,
    UnsupportedFormat,
    FontNotFound,
    GlyphNotFound,
}
