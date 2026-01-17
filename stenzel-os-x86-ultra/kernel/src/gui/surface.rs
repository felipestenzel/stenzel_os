//! Surface module
//!
//! A Surface is a rectangular buffer that can be drawn to and composited onto
//! the screen. Surfaces are the basic building blocks of the windowing system.

use alloc::vec::Vec;
use crate::drivers::framebuffer::Color;

/// A unique surface identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SurfaceId(pub u64);

static NEXT_SURFACE_ID: spin::Mutex<u64> = spin::Mutex::new(1);

impl SurfaceId {
    /// Generate a new unique surface ID
    pub fn new() -> Self {
        let mut next = NEXT_SURFACE_ID.lock();
        let id = *next;
        *next += 1;
        SurfaceId(id)
    }
}

impl Default for SurfaceId {
    fn default() -> Self {
        Self::new()
    }
}

/// Pixel format for surfaces
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    /// 32-bit RGBA (8 bits per channel)
    Rgba8888,
    /// 32-bit BGRA (8 bits per channel)
    Bgra8888,
    /// 32-bit RGB (8 bits per channel, alpha ignored)
    Rgbx8888,
    /// 24-bit RGB (8 bits per channel, no alpha)
    Rgb888,
}

impl PixelFormat {
    /// Get bytes per pixel for this format
    pub fn bytes_per_pixel(&self) -> usize {
        match self {
            PixelFormat::Rgba8888 => 4,
            PixelFormat::Bgra8888 => 4,
            PixelFormat::Rgbx8888 => 4,
            PixelFormat::Rgb888 => 3,
        }
    }

    /// Check if format has alpha channel
    pub fn has_alpha(&self) -> bool {
        matches!(self, PixelFormat::Rgba8888 | PixelFormat::Bgra8888)
    }
}

/// A drawable surface (buffer)
pub struct Surface {
    /// Unique identifier
    id: SurfaceId,
    /// Width in pixels
    width: usize,
    /// Height in pixels
    height: usize,
    /// Pixel format
    format: PixelFormat,
    /// Pixel data (row-major, top-to-bottom)
    data: Vec<u8>,
    /// Whether the surface needs to be redrawn
    dirty: bool,
    /// Dirty rectangle (optional, for partial updates)
    dirty_rect: Option<DirtyRect>,
}

/// A dirty rectangle marking the region that needs update
#[derive(Debug, Clone, Copy)]
pub struct DirtyRect {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

impl DirtyRect {
    /// Create a new dirty rectangle
    pub fn new(x: usize, y: usize, width: usize, height: usize) -> Self {
        Self { x, y, width, height }
    }

    /// Merge with another dirty rectangle (union)
    pub fn union(&self, other: &DirtyRect) -> DirtyRect {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let x2 = (self.x + self.width).max(other.x + other.width);
        let y2 = (self.y + self.height).max(other.y + other.height);
        DirtyRect {
            x,
            y,
            width: x2 - x,
            height: y2 - y,
        }
    }
}

impl Surface {
    /// Create a new surface with the given dimensions
    pub fn new(width: usize, height: usize, format: PixelFormat) -> Self {
        let size = width * height * format.bytes_per_pixel();
        let mut data = Vec::with_capacity(size);
        data.resize(size, 0);

        Self {
            id: SurfaceId::new(),
            width,
            height,
            format,
            data,
            dirty: true,
            dirty_rect: None,
        }
    }

    /// Create a surface and fill with a color
    pub fn new_with_color(width: usize, height: usize, format: PixelFormat, color: Color) -> Self {
        let mut surface = Self::new(width, height, format);
        surface.clear(color);
        surface
    }

    /// Get the surface ID
    pub fn id(&self) -> SurfaceId {
        self.id
    }

    /// Get width
    pub fn width(&self) -> usize {
        self.width
    }

    /// Get height
    pub fn height(&self) -> usize {
        self.height
    }

    /// Get pixel format
    pub fn format(&self) -> PixelFormat {
        self.format
    }

    /// Get bytes per row (stride)
    pub fn stride(&self) -> usize {
        self.width * self.format.bytes_per_pixel()
    }

    /// Check if surface is dirty (needs redraw)
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Mark surface as dirty
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
        self.dirty_rect = None; // Full redraw
    }

    /// Mark a region as dirty
    pub fn mark_dirty_rect(&mut self, x: usize, y: usize, width: usize, height: usize) {
        self.dirty = true;
        let rect = DirtyRect::new(x, y, width, height);
        self.dirty_rect = Some(match self.dirty_rect {
            Some(existing) => existing.union(&rect),
            None => rect,
        });
    }

    /// Clear the dirty flag
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
        self.dirty_rect = None;
    }

    /// Get the dirty rectangle
    pub fn dirty_rect(&self) -> Option<DirtyRect> {
        self.dirty_rect
    }

    /// Get raw pixel data
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Get mutable raw pixel data
    pub fn data_mut(&mut self) -> &mut [u8] {
        self.dirty = true;
        &mut self.data
    }

    /// Get a row of pixel data
    pub fn row(&self, y: usize) -> Option<&[u8]> {
        if y >= self.height {
            return None;
        }
        let start = y * self.stride();
        let end = start + self.stride();
        Some(&self.data[start..end])
    }

    /// Get a mutable row of pixel data
    pub fn row_mut(&mut self, y: usize) -> Option<&mut [u8]> {
        if y >= self.height {
            return None;
        }
        self.dirty = true;
        let stride = self.stride();
        let start = y * stride;
        let end = start + stride;
        Some(&mut self.data[start..end])
    }

    /// Clear the surface with a color
    pub fn clear(&mut self, color: Color) {
        let bpp = self.format.bytes_per_pixel();
        let pixel = self.color_to_bytes(color);

        for y in 0..self.height {
            for x in 0..self.width {
                let offset = (y * self.width + x) * bpp;
                for i in 0..bpp {
                    self.data[offset + i] = pixel[i];
                }
            }
        }
        self.dirty = true;
    }

    /// Set a pixel
    pub fn set_pixel(&mut self, x: usize, y: usize, color: Color) {
        if x >= self.width || y >= self.height {
            return;
        }

        let bpp = self.format.bytes_per_pixel();
        let offset = (y * self.width + x) * bpp;
        let pixel = self.color_to_bytes(color);

        for i in 0..bpp {
            self.data[offset + i] = pixel[i];
        }
        self.dirty = true;
    }

    /// Get a pixel
    pub fn get_pixel(&self, x: usize, y: usize) -> Option<Color> {
        if x >= self.width || y >= self.height {
            return None;
        }

        let bpp = self.format.bytes_per_pixel();
        let offset = (y * self.width + x) * bpp;
        Some(self.bytes_to_color(&self.data[offset..offset + bpp]))
    }

    /// Set a pixel with alpha blending
    pub fn set_pixel_blended(&mut self, x: usize, y: usize, color: Color) {
        if x >= self.width || y >= self.height {
            return;
        }

        if color.a == 255 {
            self.set_pixel(x, y, color);
            return;
        }

        if color.a == 0 {
            return;
        }

        if let Some(bg) = self.get_pixel(x, y) {
            let blended = color.blend_over(bg);
            self.set_pixel(x, y, blended);
        }
    }

    /// Fill a rectangle
    pub fn fill_rect(&mut self, x: usize, y: usize, width: usize, height: usize, color: Color) {
        let x_end = (x + width).min(self.width);
        let y_end = (y + height).min(self.height);

        for py in y..y_end {
            for px in x..x_end {
                self.set_pixel(px, py, color);
            }
        }
    }

    /// Draw a rectangle outline
    pub fn draw_rect(&mut self, x: usize, y: usize, width: usize, height: usize, color: Color) {
        if width == 0 || height == 0 {
            return;
        }

        // Top and bottom edges
        for px in x..x + width {
            self.set_pixel(px, y, color);
            if height > 1 {
                self.set_pixel(px, y + height - 1, color);
            }
        }

        // Left and right edges
        for py in y + 1..y + height - 1 {
            self.set_pixel(x, py, color);
            if width > 1 {
                self.set_pixel(x + width - 1, py, color);
            }
        }
    }

    /// Draw a line using Bresenham's algorithm
    pub fn draw_line(&mut self, x0: isize, y0: isize, x1: isize, y1: isize, color: Color) {
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        let mut x = x0;
        let mut y = y0;

        loop {
            if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
                self.set_pixel(x as usize, y as usize, color);
            }

            if x == x1 && y == y1 {
                break;
            }

            let e2 = 2 * err;
            if e2 >= dy {
                if x == x1 { break; }
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                if y == y1 { break; }
                err += dx;
                y += sy;
            }
        }
    }

    /// Draw a polygon outline from a list of points
    /// Points are (x, y) coordinates, connected in order, with last connecting to first
    pub fn draw_polygon(&mut self, points: &[(isize, isize)], color: Color) {
        if points.len() < 2 {
            return;
        }

        // Draw lines between consecutive points
        for i in 0..points.len() {
            let next = (i + 1) % points.len();
            self.draw_line(points[i].0, points[i].1, points[next].0, points[next].1, color);
        }
    }

    /// Fill a polygon using scanline algorithm
    /// Points are (x, y) coordinates forming a closed polygon
    pub fn fill_polygon(&mut self, points: &[(isize, isize)], color: Color) {
        if points.len() < 3 {
            return;
        }

        // Find bounding box
        let min_y = points.iter().map(|p| p.1).min().unwrap_or(0);
        let max_y = points.iter().map(|p| p.1).max().unwrap_or(0);
        let min_x = points.iter().map(|p| p.0).min().unwrap_or(0);
        let max_x = points.iter().map(|p| p.0).max().unwrap_or(0);

        // Clip to surface bounds
        let start_y = min_y.max(0) as usize;
        let end_y = (max_y as usize).min(self.height);
        let _ = (min_x, max_x); // Used for context

        // Scanline fill
        let mut intersections = Vec::new();

        for y in start_y..end_y {
            intersections.clear();
            let y_f = y as isize;

            // Find all intersections with polygon edges
            for i in 0..points.len() {
                let j = (i + 1) % points.len();
                let (x0, y0) = points[i];
                let (x1, y1) = points[j];

                // Skip horizontal edges
                if y0 == y1 {
                    continue;
                }

                // Check if scanline intersects this edge
                let (y_min, y_max) = if y0 < y1 { (y0, y1) } else { (y1, y0) };
                if y_f < y_min || y_f >= y_max {
                    continue;
                }

                // Calculate x intersection using linear interpolation
                let x_intersect = x0 + (y_f - y0) * (x1 - x0) / (y1 - y0);
                intersections.push(x_intersect);
            }

            // Sort intersections
            intersections.sort();

            // Fill between pairs of intersections
            let mut i = 0;
            while i + 1 < intersections.len() {
                let x_start = intersections[i].max(0) as usize;
                let x_end = (intersections[i + 1] as usize).min(self.width);
                for x in x_start..x_end {
                    self.set_pixel(x, y, color);
                }
                i += 2;
            }
        }
    }

    /// Draw a triangle (convenience function)
    pub fn draw_triangle(&mut self, x0: isize, y0: isize, x1: isize, y1: isize, x2: isize, y2: isize, color: Color) {
        self.draw_polygon(&[(x0, y0), (x1, y1), (x2, y2)], color);
    }

    /// Fill a triangle (convenience function)
    pub fn fill_triangle(&mut self, x0: isize, y0: isize, x1: isize, y1: isize, x2: isize, y2: isize, color: Color) {
        self.fill_polygon(&[(x0, y0), (x1, y1), (x2, y2)], color);
    }

    // ======================== Anti-aliasing ========================

    // Helper functions for floating point math (no_std compatible)
    #[inline]
    fn floor_f32(x: f32) -> f32 {
        let xi = x as i32;
        if x < 0.0 && x != xi as f32 { (xi - 1) as f32 } else { xi as f32 }
    }

    #[inline]
    fn ceil_f32(x: f32) -> f32 {
        let xi = x as i32;
        if x > 0.0 && x != xi as f32 { (xi + 1) as f32 } else { xi as f32 }
    }

    #[inline]
    fn round_f32(x: f32) -> f32 {
        Self::floor_f32(x + 0.5)
    }

    #[inline]
    fn fract_f32(x: f32) -> f32 {
        x - Self::floor_f32(x)
    }

    #[inline]
    fn abs_f32(x: f32) -> f32 {
        if x < 0.0 { -x } else { x }
    }

    #[inline]
    fn sqrt_f32(x: f32) -> f32 {
        // Newton-Raphson approximation for square root
        if x <= 0.0 { return 0.0; }
        let mut guess = x / 2.0;
        for _ in 0..10 {
            guess = (guess + x / guess) / 2.0;
        }
        guess
    }

    #[inline]
    fn clamp_f32(x: f32, min: f32, max: f32) -> f32 {
        if x < min { min } else if x > max { max } else { x }
    }

    /// Blend a color with the existing pixel using alpha
    fn blend_pixel(&mut self, x: usize, y: usize, color: Color, alpha: f32) {
        if x >= self.width || y >= self.height {
            return;
        }

        let existing = self.get_pixel(x, y).unwrap_or(Color { r: 0, g: 0, b: 0, a: 255 });
        let alpha = Self::clamp_f32(alpha, 0.0, 1.0);
        let inv_alpha = 1.0 - alpha;

        let blended = Color {
            r: (color.r as f32 * alpha + existing.r as f32 * inv_alpha) as u8,
            g: (color.g as f32 * alpha + existing.g as f32 * inv_alpha) as u8,
            b: (color.b as f32 * alpha + existing.b as f32 * inv_alpha) as u8,
            a: 255,
        };
        self.set_pixel(x, y, blended);
    }

    /// Draw an anti-aliased line using Xiaolin Wu's algorithm
    pub fn draw_line_aa(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, color: Color) {
        let steep = Self::abs_f32(y1 - y0) > Self::abs_f32(x1 - x0);

        let (x0, y0, x1, y1) = if steep {
            (y0, x0, y1, x1)
        } else {
            (x0, y0, x1, y1)
        };

        let (x0, y0, x1, y1) = if x0 > x1 {
            (x1, y1, x0, y0)
        } else {
            (x0, y0, x1, y1)
        };

        let dx = x1 - x0;
        let dy = y1 - y0;
        let gradient = if dx == 0.0 { 1.0 } else { dy / dx };

        // Handle first endpoint
        let xend = Self::round_f32(x0);
        let yend = y0 + gradient * (xend - x0);
        let xgap = 1.0 - Self::fract_f32(x0 + 0.5);
        let xpxl1 = xend as isize;
        let ypxl1 = Self::floor_f32(yend) as isize;

        if steep {
            self.blend_pixel(ypxl1 as usize, xpxl1 as usize, color, (1.0 - Self::fract_f32(yend)) * xgap);
            self.blend_pixel((ypxl1 + 1) as usize, xpxl1 as usize, color, Self::fract_f32(yend) * xgap);
        } else {
            self.blend_pixel(xpxl1 as usize, ypxl1 as usize, color, (1.0 - Self::fract_f32(yend)) * xgap);
            self.blend_pixel(xpxl1 as usize, (ypxl1 + 1) as usize, color, Self::fract_f32(yend) * xgap);
        }

        let mut intery = yend + gradient;

        // Handle second endpoint
        let xend = Self::round_f32(x1);
        let yend = y1 + gradient * (xend - x1);
        let xgap = Self::fract_f32(x1 + 0.5);
        let xpxl2 = xend as isize;
        let ypxl2 = Self::floor_f32(yend) as isize;

        if steep {
            self.blend_pixel(ypxl2 as usize, xpxl2 as usize, color, (1.0 - Self::fract_f32(yend)) * xgap);
            self.blend_pixel((ypxl2 + 1) as usize, xpxl2 as usize, color, Self::fract_f32(yend) * xgap);
        } else {
            self.blend_pixel(xpxl2 as usize, ypxl2 as usize, color, (1.0 - Self::fract_f32(yend)) * xgap);
            self.blend_pixel(xpxl2 as usize, (ypxl2 + 1) as usize, color, Self::fract_f32(yend) * xgap);
        }

        // Main loop
        for x in (xpxl1 + 1)..xpxl2 {
            let y_floor = Self::floor_f32(intery) as usize;
            let frac = Self::fract_f32(intery);
            if steep {
                self.blend_pixel(y_floor, x as usize, color, 1.0 - frac);
                self.blend_pixel(y_floor + 1, x as usize, color, frac);
            } else {
                self.blend_pixel(x as usize, y_floor, color, 1.0 - frac);
                self.blend_pixel(x as usize, y_floor + 1, color, frac);
            }
            intery += gradient;
        }
    }

    /// Draw an anti-aliased polygon outline
    pub fn draw_polygon_aa(&mut self, points: &[(f32, f32)], color: Color) {
        if points.len() < 2 {
            return;
        }

        for i in 0..points.len() {
            let next = (i + 1) % points.len();
            self.draw_line_aa(points[i].0, points[i].1, points[next].0, points[next].1, color);
        }
    }

    /// Draw an anti-aliased circle
    pub fn draw_circle_aa(&mut self, cx: f32, cy: f32, radius: f32, color: Color) {
        if radius <= 0.0 {
            return;
        }

        // Draw circle using 8-way symmetry with anti-aliasing
        let r2 = radius * radius;
        let r_ceil = Self::ceil_f32(radius) as isize;

        for xi in 0..=r_ceil {
            let x = xi as f32;
            // Calculate y from circle equation
            let y_exact = Self::sqrt_f32(r2 - x * x);
            let y_floor = Self::floor_f32(y_exact);
            let coverage = y_exact - y_floor;

            // Draw 8 octants with anti-aliasing
            let cxi = cx as isize;
            let cyi = cy as isize;
            let yfi = y_floor as isize;

            // Use helper to safely compute pixel coordinates
            let mut plot = |sx: isize, sy: isize, alpha: f32| {
                if sx >= 0 && sy >= 0 {
                    self.blend_pixel(sx as usize, sy as usize, color, alpha);
                }
            };

            // Top and bottom pairs
            plot(cxi + xi, cyi - yfi - 1, coverage);
            plot(cxi + xi, cyi - yfi, 1.0 - coverage);
            plot(cxi + xi, cyi + yfi, 1.0 - coverage);
            plot(cxi + xi, cyi + yfi + 1, coverage);

            plot(cxi - xi, cyi - yfi - 1, coverage);
            plot(cxi - xi, cyi - yfi, 1.0 - coverage);
            plot(cxi - xi, cyi + yfi, 1.0 - coverage);
            plot(cxi - xi, cyi + yfi + 1, coverage);

            // Left and right pairs (swap x and y)
            plot(cxi - yfi - 1, cyi + xi, coverage);
            plot(cxi - yfi, cyi + xi, 1.0 - coverage);
            plot(cxi + yfi, cyi + xi, 1.0 - coverage);
            plot(cxi + yfi + 1, cyi + xi, coverage);

            plot(cxi - yfi - 1, cyi - xi, coverage);
            plot(cxi - yfi, cyi - xi, 1.0 - coverage);
            plot(cxi + yfi, cyi - xi, 1.0 - coverage);
            plot(cxi + yfi + 1, cyi - xi, coverage);
        }
    }

    /// Blit (copy) another surface onto this one
    pub fn blit(&mut self, src: &Surface, dst_x: isize, dst_y: isize) {
        self.blit_region(src, 0, 0, src.width, src.height, dst_x, dst_y);
    }

    /// Blit a region of another surface onto this one
    pub fn blit_region(
        &mut self,
        src: &Surface,
        src_x: usize,
        src_y: usize,
        src_w: usize,
        src_h: usize,
        dst_x: isize,
        dst_y: isize,
    ) {
        // Calculate actual visible region
        let (sx_start, dx_start) = if dst_x < 0 {
            (src_x + (-dst_x) as usize, 0usize)
        } else {
            (src_x, dst_x as usize)
        };

        let (sy_start, dy_start) = if dst_y < 0 {
            (src_y + (-dst_y) as usize, 0usize)
        } else {
            (src_y, dst_y as usize)
        };

        let copy_w = src_w
            .saturating_sub(if dst_x < 0 { (-dst_x) as usize } else { 0 })
            .min(self.width.saturating_sub(dx_start))
            .min(src.width.saturating_sub(sx_start));

        let copy_h = src_h
            .saturating_sub(if dst_y < 0 { (-dst_y) as usize } else { 0 })
            .min(self.height.saturating_sub(dy_start))
            .min(src.height.saturating_sub(sy_start));

        if copy_w == 0 || copy_h == 0 {
            return;
        }

        // Copy pixels
        for y in 0..copy_h {
            for x in 0..copy_w {
                if let Some(color) = src.get_pixel(sx_start + x, sy_start + y) {
                    if src.format.has_alpha() && color.a < 255 {
                        self.set_pixel_blended(dx_start + x, dy_start + y, color);
                    } else {
                        self.set_pixel(dx_start + x, dy_start + y, color);
                    }
                }
            }
        }
    }

    /// Convert a Color to bytes in this surface's format
    fn color_to_bytes(&self, color: Color) -> [u8; 4] {
        match self.format {
            PixelFormat::Rgba8888 => [color.r, color.g, color.b, color.a],
            PixelFormat::Bgra8888 => [color.b, color.g, color.r, color.a],
            PixelFormat::Rgbx8888 => [color.r, color.g, color.b, 0xFF],
            PixelFormat::Rgb888 => [color.r, color.g, color.b, 0],
        }
    }

    /// Convert bytes from this surface's format to Color
    fn bytes_to_color(&self, bytes: &[u8]) -> Color {
        match self.format {
            PixelFormat::Rgba8888 => Color::rgba(bytes[0], bytes[1], bytes[2], bytes[3]),
            PixelFormat::Bgra8888 => Color::rgba(bytes[2], bytes[1], bytes[0], bytes[3]),
            PixelFormat::Rgbx8888 => Color::new(bytes[0], bytes[1], bytes[2]),
            PixelFormat::Rgb888 => Color::new(bytes[0], bytes[1], bytes[2]),
        }
    }

    /// Resize the surface (creates a new buffer, doesn't scale content)
    pub fn resize(&mut self, new_width: usize, new_height: usize) {
        if new_width == self.width && new_height == self.height {
            return;
        }

        let new_size = new_width * new_height * self.format.bytes_per_pixel();
        let mut new_data = Vec::with_capacity(new_size);
        new_data.resize(new_size, 0);

        // Copy existing data that fits
        let copy_w = self.width.min(new_width);
        let copy_h = self.height.min(new_height);
        let bpp = self.format.bytes_per_pixel();
        let old_stride = self.width * bpp;
        let new_stride = new_width * bpp;

        for y in 0..copy_h {
            let src_start = y * old_stride;
            let dst_start = y * new_stride;
            let copy_bytes = copy_w * bpp;
            new_data[dst_start..dst_start + copy_bytes]
                .copy_from_slice(&self.data[src_start..src_start + copy_bytes]);
        }

        self.width = new_width;
        self.height = new_height;
        self.data = new_data;
        self.dirty = true;
    }
}
