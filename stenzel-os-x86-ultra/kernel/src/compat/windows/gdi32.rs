//! GDI32 Emulation
//!
//! Emulates Windows gdi32.dll - provides graphics device interface
//! for drawing operations, fonts, bitmaps, and device contexts.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;

/// Device context handle
pub type HDC = u64;
/// Bitmap handle
pub type HBITMAP = u64;
/// Brush handle
pub type HBRUSH = u64;
/// Pen handle
pub type HPEN = u64;
/// Font handle
pub type HFONT = u64;
/// Region handle
pub type HRGN = u64;
/// Palette handle
pub type HPALETTE = u64;

/// GDI object types
pub mod obj {
    pub const PEN: i32 = 1;
    pub const BRUSH: i32 = 2;
    pub const DC: i32 = 3;
    pub const METADC: i32 = 4;
    pub const PAL: i32 = 5;
    pub const FONT: i32 = 6;
    pub const BITMAP: i32 = 7;
    pub const REGION: i32 = 8;
    pub const METAFILE: i32 = 9;
    pub const MEMDC: i32 = 10;
    pub const EXTPEN: i32 = 11;
    pub const ENHMETADC: i32 = 12;
    pub const ENHMETAFILE: i32 = 13;
    pub const COLORSPACE: i32 = 14;
}

/// Stock objects
pub mod stock {
    pub const WHITE_BRUSH: i32 = 0;
    pub const LTGRAY_BRUSH: i32 = 1;
    pub const GRAY_BRUSH: i32 = 2;
    pub const DKGRAY_BRUSH: i32 = 3;
    pub const BLACK_BRUSH: i32 = 4;
    pub const NULL_BRUSH: i32 = 5;
    pub const HOLLOW_BRUSH: i32 = 5;
    pub const WHITE_PEN: i32 = 6;
    pub const BLACK_PEN: i32 = 7;
    pub const NULL_PEN: i32 = 8;
    pub const OEM_FIXED_FONT: i32 = 10;
    pub const ANSI_FIXED_FONT: i32 = 11;
    pub const ANSI_VAR_FONT: i32 = 12;
    pub const SYSTEM_FONT: i32 = 13;
    pub const DEVICE_DEFAULT_FONT: i32 = 14;
    pub const DEFAULT_PALETTE: i32 = 15;
    pub const SYSTEM_FIXED_FONT: i32 = 16;
    pub const DEFAULT_GUI_FONT: i32 = 17;
    pub const DC_BRUSH: i32 = 18;
    pub const DC_PEN: i32 = 19;
}

/// Pen styles
pub mod ps {
    pub const SOLID: i32 = 0;
    pub const DASH: i32 = 1;
    pub const DOT: i32 = 2;
    pub const DASHDOT: i32 = 3;
    pub const DASHDOTDOT: i32 = 4;
    pub const NULL: i32 = 5;
    pub const INSIDEFRAME: i32 = 6;
}

/// Brush styles
pub mod bs {
    pub const SOLID: i32 = 0;
    pub const NULL: i32 = 1;
    pub const HOLLOW: i32 = 1;
    pub const HATCHED: i32 = 2;
    pub const PATTERN: i32 = 3;
    pub const INDEXED: i32 = 4;
    pub const DIBPATTERN: i32 = 5;
    pub const DIBPATTERNPT: i32 = 6;
    pub const PATTERN8X8: i32 = 7;
    pub const DIBPATTERN8X8: i32 = 8;
}

/// Hatch styles
pub mod hs {
    pub const HORIZONTAL: i32 = 0;
    pub const VERTICAL: i32 = 1;
    pub const FDIAGONAL: i32 = 2;
    pub const BDIAGONAL: i32 = 3;
    pub const CROSS: i32 = 4;
    pub const DIAGCROSS: i32 = 5;
}

/// Font weights
pub mod fw {
    pub const DONTCARE: i32 = 0;
    pub const THIN: i32 = 100;
    pub const EXTRALIGHT: i32 = 200;
    pub const ULTRALIGHT: i32 = 200;
    pub const LIGHT: i32 = 300;
    pub const NORMAL: i32 = 400;
    pub const REGULAR: i32 = 400;
    pub const MEDIUM: i32 = 500;
    pub const SEMIBOLD: i32 = 600;
    pub const DEMIBOLD: i32 = 600;
    pub const BOLD: i32 = 700;
    pub const EXTRABOLD: i32 = 800;
    pub const ULTRABOLD: i32 = 800;
    pub const HEAVY: i32 = 900;
    pub const BLACK: i32 = 900;
}

/// Character sets
pub mod charset {
    pub const ANSI_CHARSET: u32 = 0;
    pub const DEFAULT_CHARSET: u32 = 1;
    pub const SYMBOL_CHARSET: u32 = 2;
    pub const MAC_CHARSET: u32 = 77;
    pub const OEM_CHARSET: u32 = 255;
}

/// Background modes
pub mod bg {
    pub const TRANSPARENT: i32 = 1;
    pub const OPAQUE: i32 = 2;
}

/// ROP2 modes
pub mod rop2 {
    pub const BLACK: i32 = 1;
    pub const NOTMERGEPEN: i32 = 2;
    pub const MASKNOTPEN: i32 = 3;
    pub const NOTCOPYPEN: i32 = 4;
    pub const MASKPENNOT: i32 = 5;
    pub const NOT: i32 = 6;
    pub const XORPEN: i32 = 7;
    pub const NOTMASKPEN: i32 = 8;
    pub const MASKPEN: i32 = 9;
    pub const NOTXORPEN: i32 = 10;
    pub const NOP: i32 = 11;
    pub const MERGENOTPEN: i32 = 12;
    pub const COPYPEN: i32 = 13;
    pub const MERGEPENNOT: i32 = 14;
    pub const MERGEPEN: i32 = 15;
    pub const WHITE: i32 = 16;
}

/// Ternary raster operations (BitBlt)
pub mod rop {
    pub const SRCCOPY: u32 = 0x00CC0020;
    pub const SRCPAINT: u32 = 0x00EE0086;
    pub const SRCAND: u32 = 0x008800C6;
    pub const SRCINVERT: u32 = 0x00660046;
    pub const SRCERASE: u32 = 0x00440328;
    pub const NOTSRCCOPY: u32 = 0x00330008;
    pub const NOTSRCERASE: u32 = 0x001100A6;
    pub const MERGECOPY: u32 = 0x00C000CA;
    pub const MERGEPAINT: u32 = 0x00BB0226;
    pub const PATCOPY: u32 = 0x00F00021;
    pub const PATPAINT: u32 = 0x00FB0A09;
    pub const PATINVERT: u32 = 0x005A0049;
    pub const DSTINVERT: u32 = 0x00550009;
    pub const BLACKNESS: u32 = 0x00000042;
    pub const WHITENESS: u32 = 0x00FF0062;
}

/// Text alignment
pub mod ta {
    pub const NOUPDATECP: u32 = 0;
    pub const UPDATECP: u32 = 1;
    pub const LEFT: u32 = 0;
    pub const RIGHT: u32 = 2;
    pub const CENTER: u32 = 6;
    pub const TOP: u32 = 0;
    pub const BOTTOM: u32 = 8;
    pub const BASELINE: u32 = 24;
}

/// DIB color modes
pub mod dib {
    pub const RGB_COLORS: i32 = 0;
    pub const PAL_COLORS: i32 = 1;
}

/// Color (COLORREF is DWORD: 0x00BBGGRR)
pub fn rgb(r: u8, g: u8, b: u8) -> u32 {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

pub fn get_r(color: u32) -> u8 {
    (color & 0xFF) as u8
}

pub fn get_g(color: u32) -> u8 {
    ((color >> 8) & 0xFF) as u8
}

pub fn get_b(color: u32) -> u8 {
    ((color >> 16) & 0xFF) as u8
}

/// Point structure
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct POINT {
    pub x: i32,
    pub y: i32,
}

/// Size structure
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct SIZE {
    pub cx: i32,
    pub cy: i32,
}

/// Rectangle structure
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// Bitmap info header
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct BITMAPINFOHEADER {
    pub bi_size: u32,
    pub bi_width: i32,
    pub bi_height: i32,
    pub bi_planes: u16,
    pub bi_bit_count: u16,
    pub bi_compression: u32,
    pub bi_size_image: u32,
    pub bi_x_pels_per_meter: i32,
    pub bi_y_pels_per_meter: i32,
    pub bi_clr_used: u32,
    pub bi_clr_important: u32,
}

/// Log font structure
#[repr(C)]
#[derive(Debug, Clone)]
pub struct LOGFONT {
    pub lf_height: i32,
    pub lf_width: i32,
    pub lf_escapement: i32,
    pub lf_orientation: i32,
    pub lf_weight: i32,
    pub lf_italic: u8,
    pub lf_underline: u8,
    pub lf_strike_out: u8,
    pub lf_char_set: u8,
    pub lf_out_precision: u8,
    pub lf_clip_precision: u8,
    pub lf_quality: u8,
    pub lf_pitch_and_family: u8,
    pub lf_face_name: String,
}

impl Default for LOGFONT {
    fn default() -> Self {
        Self {
            lf_height: 0,
            lf_width: 0,
            lf_escapement: 0,
            lf_orientation: 0,
            lf_weight: fw::NORMAL,
            lf_italic: 0,
            lf_underline: 0,
            lf_strike_out: 0,
            lf_char_set: charset::DEFAULT_CHARSET as u8,
            lf_out_precision: 0,
            lf_clip_precision: 0,
            lf_quality: 0,
            lf_pitch_and_family: 0,
            lf_face_name: String::new(),
        }
    }
}

/// Text metrics
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct TEXTMETRIC {
    pub tm_height: i32,
    pub tm_ascent: i32,
    pub tm_descent: i32,
    pub tm_internal_leading: i32,
    pub tm_external_leading: i32,
    pub tm_ave_char_width: i32,
    pub tm_max_char_width: i32,
    pub tm_weight: i32,
    pub tm_overhang: i32,
    pub tm_digitized_aspect_x: i32,
    pub tm_digitized_aspect_y: i32,
    pub tm_first_char: u8,
    pub tm_last_char: u8,
    pub tm_default_char: u8,
    pub tm_break_char: u8,
    pub tm_italic: u8,
    pub tm_underlined: u8,
    pub tm_struck_out: u8,
    pub tm_pitch_and_family: u8,
    pub tm_char_set: u8,
}

/// Device context state
#[derive(Debug, Clone)]
pub struct DeviceContextState {
    pub text_color: u32,
    pub bg_color: u32,
    pub bg_mode: i32,
    pub rop2: i32,
    pub text_align: u32,
    pub brush_color: u32,
    pub pen_color: u32,
    pub pen_style: i32,
    pub pen_width: i32,
    pub current_x: i32,
    pub current_y: i32,
    pub selected_brush: HBRUSH,
    pub selected_pen: HPEN,
    pub selected_font: HFONT,
    pub selected_bitmap: HBITMAP,
}

impl Default for DeviceContextState {
    fn default() -> Self {
        Self {
            text_color: rgb(0, 0, 0),          // Black
            bg_color: rgb(255, 255, 255),     // White
            bg_mode: bg::OPAQUE,
            rop2: rop2::COPYPEN,
            text_align: ta::LEFT | ta::TOP,
            brush_color: rgb(255, 255, 255),  // White
            pen_color: rgb(0, 0, 0),          // Black
            pen_style: ps::SOLID,
            pen_width: 1,
            current_x: 0,
            current_y: 0,
            selected_brush: 0,
            selected_pen: 0,
            selected_font: 0,
            selected_bitmap: 0,
        }
    }
}

/// GDI object info
#[derive(Debug, Clone)]
pub enum GdiObject {
    Pen { color: u32, style: i32, width: i32 },
    Brush { color: u32, style: i32 },
    Font { logfont: LOGFONT },
    Bitmap { width: i32, height: i32, bpp: u16, data: Vec<u8> },
    Region { rect: RECT },
    Palette { entries: Vec<u32> },
}

/// GDI32 emulator state
pub struct Gdi32Emulator {
    /// Device contexts
    device_contexts: BTreeMap<HDC, DeviceContextState>,
    /// GDI objects
    gdi_objects: BTreeMap<u64, GdiObject>,
    /// Next handle
    next_handle: u64,
    /// Stock objects
    stock_objects: BTreeMap<i32, u64>,
}

impl Gdi32Emulator {
    pub fn new() -> Self {
        let mut emulator = Self {
            device_contexts: BTreeMap::new(),
            gdi_objects: BTreeMap::new(),
            next_handle: 0x1000,
            stock_objects: BTreeMap::new(),
        };

        // Create stock objects
        emulator.init_stock_objects();
        emulator
    }

    fn init_stock_objects(&mut self) {
        // Create stock brushes first
        let white_brush = self.create_object(GdiObject::Brush { color: rgb(255, 255, 255), style: bs::SOLID });
        let ltgray_brush = self.create_object(GdiObject::Brush { color: rgb(192, 192, 192), style: bs::SOLID });
        let gray_brush = self.create_object(GdiObject::Brush { color: rgb(128, 128, 128), style: bs::SOLID });
        let dkgray_brush = self.create_object(GdiObject::Brush { color: rgb(64, 64, 64), style: bs::SOLID });
        let black_brush = self.create_object(GdiObject::Brush { color: rgb(0, 0, 0), style: bs::SOLID });
        let null_brush = self.create_object(GdiObject::Brush { color: 0, style: bs::NULL });

        // Create stock pens
        let white_pen = self.create_object(GdiObject::Pen { color: rgb(255, 255, 255), style: ps::SOLID, width: 1 });
        let black_pen = self.create_object(GdiObject::Pen { color: rgb(0, 0, 0), style: ps::SOLID, width: 1 });
        let null_pen = self.create_object(GdiObject::Pen { color: 0, style: ps::NULL, width: 0 });

        // Create stock fonts
        let system_font = self.create_object(GdiObject::Font {
            logfont: LOGFONT {
                lf_height: 16,
                lf_face_name: String::from("System"),
                ..Default::default()
            }
        });
        let default_gui_font = self.create_object(GdiObject::Font {
            logfont: LOGFONT {
                lf_height: 13,
                lf_face_name: String::from("MS Shell Dlg"),
                ..Default::default()
            }
        });

        // Now insert into stock objects
        self.stock_objects.insert(stock::WHITE_BRUSH, white_brush);
        self.stock_objects.insert(stock::LTGRAY_BRUSH, ltgray_brush);
        self.stock_objects.insert(stock::GRAY_BRUSH, gray_brush);
        self.stock_objects.insert(stock::DKGRAY_BRUSH, dkgray_brush);
        self.stock_objects.insert(stock::BLACK_BRUSH, black_brush);
        self.stock_objects.insert(stock::NULL_BRUSH, null_brush);
        self.stock_objects.insert(stock::WHITE_PEN, white_pen);
        self.stock_objects.insert(stock::BLACK_PEN, black_pen);
        self.stock_objects.insert(stock::NULL_PEN, null_pen);
        self.stock_objects.insert(stock::SYSTEM_FONT, system_font);
        self.stock_objects.insert(stock::DEFAULT_GUI_FONT, default_gui_font);
    }

    fn create_object(&mut self, obj: GdiObject) -> u64 {
        let handle = self.next_handle;
        self.next_handle += 1;
        self.gdi_objects.insert(handle, obj);
        handle
    }

    // ========== Device Context Functions ==========

    /// CreateDC
    pub fn create_dc(&mut self, _driver: Option<&str>, _device: Option<&str>) -> HDC {
        let hdc = self.next_handle;
        self.next_handle += 1;
        self.device_contexts.insert(hdc, DeviceContextState::default());
        hdc
    }

    /// CreateCompatibleDC
    pub fn create_compatible_dc(&mut self, _hdc: HDC) -> HDC {
        self.create_dc(None, None)
    }

    /// DeleteDC
    pub fn delete_dc(&mut self, hdc: HDC) -> bool {
        self.device_contexts.remove(&hdc).is_some()
    }

    /// SaveDC
    pub fn save_dc(&mut self, _hdc: HDC) -> i32 {
        // Would save DC state to stack
        1
    }

    /// RestoreDC
    pub fn restore_dc(&mut self, _hdc: HDC, _saved: i32) -> bool {
        // Would restore DC state
        true
    }

    // ========== GDI Object Functions ==========

    /// GetStockObject
    pub fn get_stock_object(&self, obj_type: i32) -> u64 {
        self.stock_objects.get(&obj_type).copied().unwrap_or(0)
    }

    /// SelectObject
    pub fn select_object(&mut self, hdc: HDC, obj: u64) -> u64 {
        // First, extract object properties
        #[derive(Clone, Copy)]
        enum ObjInfo {
            Pen { color: u32, style: i32, width: i32 },
            Brush { color: u32 },
            Font,
            Bitmap,
            None,
        }

        let obj_info = if let Some(gdi_obj) = self.gdi_objects.get(&obj) {
            match gdi_obj {
                GdiObject::Pen { color, style, width } => ObjInfo::Pen { color: *color, style: *style, width: *width },
                GdiObject::Brush { color, .. } => ObjInfo::Brush { color: *color },
                GdiObject::Font { .. } => ObjInfo::Font,
                GdiObject::Bitmap { .. } => ObjInfo::Bitmap,
                _ => ObjInfo::None,
            }
        } else {
            ObjInfo::None
        };

        // Now update the device context
        if let Some(dc) = self.device_contexts.get_mut(&hdc) {
            match obj_info {
                ObjInfo::Pen { color, style, width } => {
                    let old = dc.selected_pen;
                    dc.selected_pen = obj;
                    dc.pen_color = color;
                    dc.pen_style = style;
                    dc.pen_width = width;
                    return old;
                }
                ObjInfo::Brush { color } => {
                    let old = dc.selected_brush;
                    dc.selected_brush = obj;
                    dc.brush_color = color;
                    return old;
                }
                ObjInfo::Font => {
                    let old = dc.selected_font;
                    dc.selected_font = obj;
                    return old;
                }
                ObjInfo::Bitmap => {
                    let old = dc.selected_bitmap;
                    dc.selected_bitmap = obj;
                    return old;
                }
                ObjInfo::None => {}
            }
        }
        0
    }

    /// DeleteObject
    pub fn delete_object(&mut self, obj: u64) -> bool {
        // Don't delete stock objects
        if self.stock_objects.values().any(|&h| h == obj) {
            return false;
        }
        self.gdi_objects.remove(&obj).is_some()
    }

    /// GetObject
    pub fn get_object(&self, obj: u64) -> Option<&GdiObject> {
        self.gdi_objects.get(&obj)
    }

    // ========== Pen Functions ==========

    /// CreatePen
    pub fn create_pen(&mut self, style: i32, width: i32, color: u32) -> HPEN {
        self.create_object(GdiObject::Pen { color, style, width })
    }

    // ========== Brush Functions ==========

    /// CreateSolidBrush
    pub fn create_solid_brush(&mut self, color: u32) -> HBRUSH {
        self.create_object(GdiObject::Brush { color, style: bs::SOLID })
    }

    /// CreateHatchBrush
    pub fn create_hatch_brush(&mut self, hatch: i32, color: u32) -> HBRUSH {
        self.create_object(GdiObject::Brush { color, style: hatch })
    }

    // ========== Font Functions ==========

    /// CreateFontIndirect
    pub fn create_font_indirect(&mut self, logfont: &LOGFONT) -> HFONT {
        self.create_object(GdiObject::Font { logfont: logfont.clone() })
    }

    /// CreateFont
    pub fn create_font(
        &mut self,
        height: i32,
        width: i32,
        escapement: i32,
        orientation: i32,
        weight: i32,
        italic: u8,
        underline: u8,
        strike_out: u8,
        char_set: u8,
        out_precision: u8,
        clip_precision: u8,
        quality: u8,
        pitch_and_family: u8,
        face_name: &str,
    ) -> HFONT {
        let logfont = LOGFONT {
            lf_height: height,
            lf_width: width,
            lf_escapement: escapement,
            lf_orientation: orientation,
            lf_weight: weight,
            lf_italic: italic,
            lf_underline: underline,
            lf_strike_out: strike_out,
            lf_char_set: char_set,
            lf_out_precision: out_precision,
            lf_clip_precision: clip_precision,
            lf_quality: quality,
            lf_pitch_and_family: pitch_and_family,
            lf_face_name: String::from(face_name),
        };
        self.create_font_indirect(&logfont)
    }

    /// GetTextMetrics
    pub fn get_text_metrics(&self, _hdc: HDC) -> TEXTMETRIC {
        // Return default metrics
        TEXTMETRIC {
            tm_height: 16,
            tm_ascent: 13,
            tm_descent: 3,
            tm_internal_leading: 0,
            tm_external_leading: 0,
            tm_ave_char_width: 8,
            tm_max_char_width: 16,
            tm_weight: fw::NORMAL,
            tm_first_char: 32,
            tm_last_char: 126,
            tm_default_char: b'?',
            tm_break_char: b' ',
            ..Default::default()
        }
    }

    // ========== Bitmap Functions ==========

    /// CreateBitmap
    pub fn create_bitmap(&mut self, width: i32, height: i32, planes: u32, bpp: u32) -> HBITMAP {
        let size = ((width * height * bpp as i32 / 8) as usize).max(1);
        self.create_object(GdiObject::Bitmap {
            width,
            height,
            bpp: bpp as u16,
            data: vec![0u8; size],
        })
    }

    /// CreateCompatibleBitmap
    pub fn create_compatible_bitmap(&mut self, _hdc: HDC, width: i32, height: i32) -> HBITMAP {
        self.create_bitmap(width, height, 1, 32)
    }

    // ========== Color Functions ==========

    /// SetTextColor
    pub fn set_text_color(&mut self, hdc: HDC, color: u32) -> u32 {
        if let Some(dc) = self.device_contexts.get_mut(&hdc) {
            let old = dc.text_color;
            dc.text_color = color;
            old
        } else {
            0xFFFFFFFF
        }
    }

    /// GetTextColor
    pub fn get_text_color(&self, hdc: HDC) -> u32 {
        self.device_contexts.get(&hdc).map(|dc| dc.text_color).unwrap_or(0)
    }

    /// SetBkColor
    pub fn set_bk_color(&mut self, hdc: HDC, color: u32) -> u32 {
        if let Some(dc) = self.device_contexts.get_mut(&hdc) {
            let old = dc.bg_color;
            dc.bg_color = color;
            old
        } else {
            0xFFFFFFFF
        }
    }

    /// GetBkColor
    pub fn get_bk_color(&self, hdc: HDC) -> u32 {
        self.device_contexts.get(&hdc).map(|dc| dc.bg_color).unwrap_or(0)
    }

    /// SetBkMode
    pub fn set_bk_mode(&mut self, hdc: HDC, mode: i32) -> i32 {
        if let Some(dc) = self.device_contexts.get_mut(&hdc) {
            let old = dc.bg_mode;
            dc.bg_mode = mode;
            old
        } else {
            0
        }
    }

    /// GetBkMode
    pub fn get_bk_mode(&self, hdc: HDC) -> i32 {
        self.device_contexts.get(&hdc).map(|dc| dc.bg_mode).unwrap_or(0)
    }

    // ========== Drawing Functions ==========

    /// MoveTo
    pub fn move_to(&mut self, hdc: HDC, x: i32, y: i32) -> Option<POINT> {
        if let Some(dc) = self.device_contexts.get_mut(&hdc) {
            let old = POINT { x: dc.current_x, y: dc.current_y };
            dc.current_x = x;
            dc.current_y = y;
            Some(old)
        } else {
            None
        }
    }

    /// LineTo
    pub fn line_to(&mut self, hdc: HDC, x: i32, y: i32) -> bool {
        crate::kprintln!("gdi32: LineTo({}, {})", x, y);
        if let Some(dc) = self.device_contexts.get_mut(&hdc) {
            dc.current_x = x;
            dc.current_y = y;
            true
        } else {
            false
        }
    }

    /// Rectangle
    pub fn rectangle(&mut self, hdc: HDC, left: i32, top: i32, right: i32, bottom: i32) -> bool {
        crate::kprintln!("gdi32: Rectangle({}, {}, {}, {})", left, top, right, bottom);
        self.device_contexts.contains_key(&hdc)
    }

    /// Ellipse
    pub fn ellipse(&mut self, hdc: HDC, left: i32, top: i32, right: i32, bottom: i32) -> bool {
        crate::kprintln!("gdi32: Ellipse({}, {}, {}, {})", left, top, right, bottom);
        self.device_contexts.contains_key(&hdc)
    }

    /// FillRect
    pub fn fill_rect(&mut self, hdc: HDC, rect: &RECT, brush: HBRUSH) -> bool {
        crate::kprintln!("gdi32: FillRect({:?})", rect);
        self.device_contexts.contains_key(&hdc)
    }

    /// FrameRect
    pub fn frame_rect(&mut self, hdc: HDC, rect: &RECT, brush: HBRUSH) -> bool {
        self.device_contexts.contains_key(&hdc)
    }

    /// SetPixel
    pub fn set_pixel(&mut self, hdc: HDC, x: i32, y: i32, color: u32) -> u32 {
        if self.device_contexts.contains_key(&hdc) {
            color
        } else {
            0xFFFFFFFF
        }
    }

    /// GetPixel
    pub fn get_pixel(&self, hdc: HDC, _x: i32, _y: i32) -> u32 {
        if self.device_contexts.contains_key(&hdc) {
            0 // Would return actual pixel color
        } else {
            0xFFFFFFFF
        }
    }

    /// BitBlt
    pub fn bit_blt(
        &mut self,
        hdc_dest: HDC,
        x_dest: i32,
        y_dest: i32,
        width: i32,
        height: i32,
        hdc_src: HDC,
        x_src: i32,
        y_src: i32,
        rop: u32,
    ) -> bool {
        crate::kprintln!("gdi32: BitBlt(dest=({},{}), size=({},{}), src=({},{}), rop={:#x})",
            x_dest, y_dest, width, height, x_src, y_src, rop);
        self.device_contexts.contains_key(&hdc_dest) && self.device_contexts.contains_key(&hdc_src)
    }

    /// StretchBlt
    pub fn stretch_blt(
        &mut self,
        hdc_dest: HDC,
        x_dest: i32,
        y_dest: i32,
        width_dest: i32,
        height_dest: i32,
        hdc_src: HDC,
        x_src: i32,
        y_src: i32,
        width_src: i32,
        height_src: i32,
        rop: u32,
    ) -> bool {
        self.device_contexts.contains_key(&hdc_dest) && self.device_contexts.contains_key(&hdc_src)
    }

    // ========== Text Functions ==========

    /// TextOut
    pub fn text_out(&mut self, hdc: HDC, x: i32, y: i32, text: &str) -> bool {
        crate::kprintln!("gdi32: TextOut({}, {}, \"{}\")", x, y, text);
        self.device_contexts.contains_key(&hdc)
    }

    /// ExtTextOut
    pub fn ext_text_out(&mut self, hdc: HDC, x: i32, y: i32, options: u32, rect: Option<&RECT>, text: &str) -> bool {
        self.device_contexts.contains_key(&hdc)
    }

    /// GetTextExtentPoint32
    pub fn get_text_extent_point32(&self, _hdc: HDC, text: &str) -> SIZE {
        // Simplified: 8 pixels per character, 16 height
        SIZE {
            cx: (text.len() * 8) as i32,
            cy: 16,
        }
    }

    /// SetTextAlign
    pub fn set_text_align(&mut self, hdc: HDC, align: u32) -> u32 {
        if let Some(dc) = self.device_contexts.get_mut(&hdc) {
            let old = dc.text_align;
            dc.text_align = align;
            old
        } else {
            0xFFFFFFFF
        }
    }
}

impl Default for Gdi32Emulator {
    fn default() -> Self {
        Self::new()
    }
}

/// Get GDI32 exports for the loader
pub fn get_exports() -> BTreeMap<String, u64> {
    let mut exports = BTreeMap::new();

    let funcs = [
        // Device context
        "CreateDCA", "CreateDCW", "CreateCompatibleDC",
        "DeleteDC", "SaveDC", "RestoreDC",
        "GetDeviceCaps",

        // Objects
        "GetStockObject", "SelectObject", "DeleteObject", "GetObjectA", "GetObjectW",
        "GetObjectType", "GetCurrentObject",

        // Pens
        "CreatePen", "CreatePenIndirect", "ExtCreatePen",

        // Brushes
        "CreateSolidBrush", "CreateHatchBrush", "CreatePatternBrush",
        "CreateBrushIndirect",

        // Fonts
        "CreateFontA", "CreateFontW", "CreateFontIndirectA", "CreateFontIndirectW",
        "GetTextMetricsA", "GetTextMetricsW",
        "GetTextFaceA", "GetTextFaceW",
        "EnumFontFamiliesA", "EnumFontFamiliesW",

        // Bitmaps
        "CreateBitmap", "CreateBitmapIndirect", "CreateCompatibleBitmap",
        "CreateDIBitmap", "CreateDIBSection",
        "GetBitmapBits", "SetBitmapBits",
        "GetDIBits", "SetDIBits",

        // Regions
        "CreateRectRgn", "CreateRectRgnIndirect", "CreateEllipticRgn",
        "CreatePolygonRgn", "CombineRgn",
        "SetWindowRgn", "GetWindowRgn",

        // Colors
        "SetTextColor", "GetTextColor",
        "SetBkColor", "GetBkColor",
        "SetBkMode", "GetBkMode",
        "SetROP2", "GetROP2",

        // Drawing
        "MoveToEx", "LineTo", "Polyline", "PolylineTo",
        "Rectangle", "Ellipse", "RoundRect", "Polygon",
        "Arc", "ArcTo", "Pie", "Chord",
        "FillRect", "FrameRect", "InvertRect",
        "FillRgn", "FrameRgn", "PaintRgn", "InvertRgn",
        "SetPixel", "GetPixel", "SetPixelV",
        "FloodFill", "ExtFloodFill",

        // Blitting
        "BitBlt", "StretchBlt", "PatBlt", "MaskBlt", "PlgBlt",
        "TransparentBlt", "AlphaBlend", "GradientFill",
        "StretchDIBits", "SetDIBitsToDevice",

        // Text
        "TextOutA", "TextOutW", "ExtTextOutA", "ExtTextOutW",
        "DrawTextA", "DrawTextW", "DrawTextExA", "DrawTextExW",
        "GetTextExtentPoint32A", "GetTextExtentPoint32W",
        "GetTextExtentPointA", "GetTextExtentPointW",
        "SetTextAlign", "GetTextAlign",
        "SetTextCharacterExtra", "GetTextCharacterExtra",
        "TabbedTextOutA", "TabbedTextOutW",

        // Paths
        "BeginPath", "EndPath", "AbortPath", "CloseFigure",
        "FillPath", "StrokePath", "StrokeAndFillPath",

        // Clipping
        "GetClipBox", "GetClipRgn", "SelectClipRgn",
        "IntersectClipRect", "ExcludeClipRect",

        // Mapping
        "SetMapMode", "GetMapMode",
        "SetViewportOrgEx", "GetViewportOrgEx",
        "SetWindowOrgEx", "GetWindowOrgEx",
        "SetViewportExtEx", "GetViewportExtEx",
        "SetWindowExtEx", "GetWindowExtEx",
        "DPtoLP", "LPtoDP",

        // Palettes
        "CreatePalette", "SelectPalette", "RealizePalette",
        "GetNearestColor", "GetNearestPaletteIndex",
    ];

    let mut addr = 0x7FC0_0000u64;
    for func in funcs {
        exports.insert(String::from(func), addr);
        addr += 16;
    }

    exports
}
