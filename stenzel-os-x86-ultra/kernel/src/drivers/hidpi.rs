// SPDX-License-Identifier: MIT
// HiDPI Scaling driver for Stenzel OS
// Supports 4K, 5K, Retina displays with automatic and manual scaling

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::string::ToString;
use alloc::collections::BTreeMap;
use crate::sync::TicketSpinlock;

// Simple math helpers for no_std
fn floor_f32(x: f32) -> f32 {
    x as i32 as f32
}

fn sqrt_approx(x: f32) -> f32 {
    // Newton-Raphson approximation for sqrt
    if x <= 0.0 { return 0.0; }
    let mut guess = x / 2.0;
    for _ in 0..8 {  // 8 iterations is enough for good precision
        guess = (guess + x / guess) / 2.0;
    }
    guess
}

/// Scale factor presets
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScaleFactor {
    Scale100,   // 1.0x - No scaling
    Scale125,   // 1.25x
    Scale150,   // 1.5x
    Scale175,   // 1.75x
    Scale200,   // 2.0x - Retina/HiDPI
    Scale225,   // 2.25x
    Scale250,   // 2.5x
    Scale300,   // 3.0x
    Scale350,   // 3.5x
    Scale400,   // 4.0x
    Custom(u32), // Custom scale in hundredths (100 = 1.0x)
}

impl ScaleFactor {
    /// Get scale as float
    pub fn as_f32(self) -> f32 {
        match self {
            ScaleFactor::Scale100 => 1.0,
            ScaleFactor::Scale125 => 1.25,
            ScaleFactor::Scale150 => 1.5,
            ScaleFactor::Scale175 => 1.75,
            ScaleFactor::Scale200 => 2.0,
            ScaleFactor::Scale225 => 2.25,
            ScaleFactor::Scale250 => 2.5,
            ScaleFactor::Scale300 => 3.0,
            ScaleFactor::Scale350 => 3.5,
            ScaleFactor::Scale400 => 4.0,
            ScaleFactor::Custom(v) => v as f32 / 100.0,
        }
    }

    /// Get scale as integer percentage
    pub fn as_percent(self) -> u32 {
        match self {
            ScaleFactor::Scale100 => 100,
            ScaleFactor::Scale125 => 125,
            ScaleFactor::Scale150 => 150,
            ScaleFactor::Scale175 => 175,
            ScaleFactor::Scale200 => 200,
            ScaleFactor::Scale225 => 225,
            ScaleFactor::Scale250 => 250,
            ScaleFactor::Scale300 => 300,
            ScaleFactor::Scale350 => 350,
            ScaleFactor::Scale400 => 400,
            ScaleFactor::Custom(v) => v,
        }
    }

    /// Create from percentage
    pub fn from_percent(percent: u32) -> Self {
        match percent {
            100 => ScaleFactor::Scale100,
            125 => ScaleFactor::Scale125,
            150 => ScaleFactor::Scale150,
            175 => ScaleFactor::Scale175,
            200 => ScaleFactor::Scale200,
            225 => ScaleFactor::Scale225,
            250 => ScaleFactor::Scale250,
            300 => ScaleFactor::Scale300,
            350 => ScaleFactor::Scale350,
            400 => ScaleFactor::Scale400,
            _ => ScaleFactor::Custom(percent),
        }
    }

    /// Check if this is a HiDPI scale (>= 2x)
    pub fn is_hidpi(self) -> bool {
        self.as_f32() >= 2.0
    }

    /// Check if fractional scaling is needed
    pub fn is_fractional(self) -> bool {
        let scale = self.as_f32();
        let diff = scale - floor_f32(scale);
        (if diff < 0.0 { -diff } else { diff }) > 0.001
    }
}

/// Scaling method
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalingMethod {
    None,           // No scaling
    Integer,        // Integer scaling (2x, 3x, etc.)
    Fractional,     // Fractional scaling with filtering
    XRender,        // X11 XRender-based scaling
    Wayland,        // Native Wayland scaling
    Viewport,       // Viewport scaling (GPU)
}

impl ScalingMethod {
    pub fn name(self) -> &'static str {
        match self {
            ScalingMethod::None => "None",
            ScalingMethod::Integer => "Integer",
            ScalingMethod::Fractional => "Fractional",
            ScalingMethod::XRender => "XRender",
            ScalingMethod::Wayland => "Wayland Native",
            ScalingMethod::Viewport => "Viewport (GPU)",
        }
    }
}

/// Scaling filter for fractional scaling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalingFilter {
    Nearest,        // Nearest neighbor (sharp but pixelated)
    Bilinear,       // Bilinear interpolation
    Bicubic,        // Bicubic (smoother)
    Lanczos,        // Lanczos (high quality)
    Spline,         // Spline interpolation
}

impl ScalingFilter {
    pub fn name(self) -> &'static str {
        match self {
            ScalingFilter::Nearest => "Nearest Neighbor",
            ScalingFilter::Bilinear => "Bilinear",
            ScalingFilter::Bicubic => "Bicubic",
            ScalingFilter::Lanczos => "Lanczos",
            ScalingFilter::Spline => "Spline",
        }
    }
}

/// DPI (Dots Per Inch) detection mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DpiMode {
    Auto,           // Automatic detection from EDID
    Manual,         // User-specified
    Xft,            // From Xft.dpi setting
    Gnome,          // GNOME text-scaling-factor
    Kde,            // KDE global scale
}

/// Display physical information
#[derive(Debug, Clone, Copy)]
pub struct DisplayPhysical {
    pub width_mm: u32,
    pub height_mm: u32,
    pub diagonal_inch: f32,
}

impl DisplayPhysical {
    pub fn new(width_mm: u32, height_mm: u32) -> Self {
        let diagonal_mm = sqrt_approx((width_mm * width_mm + height_mm * height_mm) as f32);
        let diagonal_inch = diagonal_mm / 25.4;
        Self {
            width_mm,
            height_mm,
            diagonal_inch,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.width_mm > 0 && self.height_mm > 0
    }
}

/// Display resolution
#[derive(Debug, Clone, Copy)]
pub struct DisplayResolution {
    pub width: u32,
    pub height: u32,
}

impl DisplayResolution {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    pub fn pixels(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    pub fn is_4k(&self) -> bool {
        self.width >= 3840 && self.height >= 2160
    }

    pub fn is_5k(&self) -> bool {
        self.width >= 5120 && self.height >= 2880
    }

    pub fn is_8k(&self) -> bool {
        self.width >= 7680 && self.height >= 4320
    }
}

/// Calculate DPI from physical size and resolution
pub fn calculate_dpi(resolution: DisplayResolution, physical: DisplayPhysical) -> (f32, f32) {
    if !physical.is_valid() {
        return (96.0, 96.0);  // Default fallback
    }

    let dpi_x = (resolution.width as f32 * 25.4) / physical.width_mm as f32;
    let dpi_y = (resolution.height as f32 * 25.4) / physical.height_mm as f32;

    (dpi_x, dpi_y)
}

/// Calculate recommended scale factor based on DPI
pub fn recommend_scale(dpi: f32) -> ScaleFactor {
    // Standard reference is 96 DPI (Windows) or 72 DPI (Mac)
    // We use 96 DPI as 1.0x
    let scale = dpi / 96.0;

    if scale < 1.125 {
        ScaleFactor::Scale100
    } else if scale < 1.375 {
        ScaleFactor::Scale125
    } else if scale < 1.625 {
        ScaleFactor::Scale150
    } else if scale < 1.875 {
        ScaleFactor::Scale175
    } else if scale < 2.125 {
        ScaleFactor::Scale200
    } else if scale < 2.375 {
        ScaleFactor::Scale225
    } else if scale < 2.75 {
        ScaleFactor::Scale250
    } else if scale < 3.25 {
        ScaleFactor::Scale300
    } else if scale < 3.75 {
        ScaleFactor::Scale350
    } else {
        ScaleFactor::Scale400
    }
}

/// Per-monitor scaling configuration
#[derive(Debug, Clone)]
pub struct MonitorScaling {
    pub connector_id: u32,
    pub name: String,
    pub resolution: DisplayResolution,
    pub physical: DisplayPhysical,
    pub dpi_x: f32,
    pub dpi_y: f32,
    pub scale: ScaleFactor,
    pub method: ScalingMethod,
    pub filter: ScalingFilter,
    pub effective_resolution: DisplayResolution,  // After scaling
}

impl MonitorScaling {
    pub fn new(connector_id: u32, name: &str, resolution: DisplayResolution,
               physical: DisplayPhysical) -> Self
    {
        let (dpi_x, dpi_y) = calculate_dpi(resolution, physical);
        let scale = recommend_scale((dpi_x + dpi_y) / 2.0);

        let scale_factor = scale.as_f32();
        let effective_resolution = DisplayResolution::new(
            (resolution.width as f32 / scale_factor) as u32,
            (resolution.height as f32 / scale_factor) as u32,
        );

        let method = if scale.is_fractional() {
            ScalingMethod::Fractional
        } else if scale.is_hidpi() {
            ScalingMethod::Integer
        } else {
            ScalingMethod::None
        };

        Self {
            connector_id,
            name: name.to_string(),
            resolution,
            physical,
            dpi_x,
            dpi_y,
            scale,
            method,
            filter: ScalingFilter::Bilinear,
            effective_resolution,
        }
    }

    /// Update scale factor
    pub fn set_scale(&mut self, scale: ScaleFactor) {
        self.scale = scale;

        let scale_factor = scale.as_f32();
        self.effective_resolution = DisplayResolution::new(
            (self.resolution.width as f32 / scale_factor) as u32,
            (self.resolution.height as f32 / scale_factor) as u32,
        );

        self.method = if scale.is_fractional() {
            ScalingMethod::Fractional
        } else if scale.is_hidpi() {
            ScalingMethod::Integer
        } else {
            ScalingMethod::None
        };
    }
}

/// Global HiDPI scaling settings
#[derive(Debug, Clone)]
pub struct HiDpiSettings {
    pub global_scale: Option<ScaleFactor>,  // Override all monitors
    pub auto_detect: bool,
    pub prefer_integer: bool,   // Prefer integer scales when possible
    pub filter: ScalingFilter,
    pub dpi_mode: DpiMode,
    pub force_dpi: Option<f32>,
    pub text_scale: f32,        // Additional text scaling (1.0 = normal)
    pub cursor_scale: f32,      // Cursor scaling
}

impl Default for HiDpiSettings {
    fn default() -> Self {
        Self {
            global_scale: None,
            auto_detect: true,
            prefer_integer: true,
            filter: ScalingFilter::Bilinear,
            dpi_mode: DpiMode::Auto,
            force_dpi: None,
            text_scale: 1.0,
            cursor_scale: 1.0,
        }
    }
}

/// Standard display configurations
pub mod presets {
    use super::*;

    /// Common display configurations
    pub fn macbook_retina_13() -> (DisplayResolution, DisplayPhysical) {
        (DisplayResolution::new(2560, 1600), DisplayPhysical::new(286, 179))
    }

    pub fn macbook_retina_16() -> (DisplayResolution, DisplayPhysical) {
        (DisplayResolution::new(3456, 2234), DisplayPhysical::new(345, 223))
    }

    pub fn imac_5k() -> (DisplayResolution, DisplayPhysical) {
        (DisplayResolution::new(5120, 2880), DisplayPhysical::new(597, 336))
    }

    pub fn dell_4k_27() -> (DisplayResolution, DisplayPhysical) {
        (DisplayResolution::new(3840, 2160), DisplayPhysical::new(597, 336))
    }

    pub fn lg_ultrafine_5k() -> (DisplayResolution, DisplayPhysical) {
        (DisplayResolution::new(5120, 2880), DisplayPhysical::new(597, 336))
    }

    pub fn standard_1080p_24() -> (DisplayResolution, DisplayPhysical) {
        (DisplayResolution::new(1920, 1080), DisplayPhysical::new(531, 299))
    }

    pub fn standard_1440p_27() -> (DisplayResolution, DisplayPhysical) {
        (DisplayResolution::new(2560, 1440), DisplayPhysical::new(597, 336))
    }
}

/// HiDPI Controller
pub struct HiDpiController {
    pub monitors: BTreeMap<u32, MonitorScaling>,
    pub settings: HiDpiSettings,
    initialized: bool,
}

impl HiDpiController {
    pub const fn new() -> Self {
        Self {
            monitors: BTreeMap::new(),
            settings: HiDpiSettings {
                global_scale: None,
                auto_detect: true,
                prefer_integer: true,
                filter: ScalingFilter::Bilinear,
                dpi_mode: DpiMode::Auto,
                force_dpi: None,
                text_scale: 1.0,
                cursor_scale: 1.0,
            },
            initialized: false,
        }
    }

    /// Initialize HiDPI controller
    pub fn init(&mut self) -> Result<(), &'static str> {
        self.initialized = true;
        crate::kprintln!("HiDPI: Initialized scaling controller");
        Ok(())
    }

    /// Register monitor from EDID
    pub fn register_monitor(&mut self, connector_id: u32, name: &str,
                           resolution: DisplayResolution, physical: DisplayPhysical)
    {
        let scaling = MonitorScaling::new(connector_id, name, resolution, physical);

        crate::kprintln!("HiDPI: Registered {} ({}x{} @ {:.0}x{:.0} DPI, {}x scale)",
            name, resolution.width, resolution.height,
            scaling.dpi_x, scaling.dpi_y, scaling.scale.as_percent());

        self.monitors.insert(connector_id, scaling);
    }

    /// Parse EDID for physical size
    pub fn parse_edid_physical(&self, edid: &[u8]) -> DisplayPhysical {
        if edid.len() < 128 {
            return DisplayPhysical::new(0, 0);
        }

        // Physical size in cm (bytes 21-22)
        let width_cm = edid[21] as u32;
        let height_cm = edid[22] as u32;

        if width_cm == 0 || height_cm == 0 {
            // Try detailed timing descriptor
            return self.parse_dtd_physical(edid);
        }

        DisplayPhysical::new(width_cm * 10, height_cm * 10)
    }

    /// Parse detailed timing descriptor for physical size
    fn parse_dtd_physical(&self, edid: &[u8]) -> DisplayPhysical {
        // First DTD at offset 54
        if edid.len() < 71 {
            return DisplayPhysical::new(0, 0);
        }

        let width_mm_low = edid[66] as u32;
        let height_mm_low = edid[67] as u32;
        let size_high = edid[68] as u32;

        let width_mm = width_mm_low | ((size_high >> 4) << 8);
        let height_mm = height_mm_low | ((size_high & 0x0F) << 8);

        DisplayPhysical::new(width_mm, height_mm)
    }

    /// Set scale factor for a monitor
    pub fn set_scale(&mut self, connector_id: u32, scale: ScaleFactor) -> Result<(), &'static str> {
        if let Some(monitor) = self.monitors.get_mut(&connector_id) {
            monitor.set_scale(scale);
            crate::kprintln!("HiDPI: Set connector {} to {}x scale",
                connector_id, scale.as_percent());
            Ok(())
        } else {
            Err("Monitor not found")
        }
    }

    /// Set global scale (applies to all monitors)
    pub fn set_global_scale(&mut self, scale: Option<ScaleFactor>) {
        self.settings.global_scale = scale;

        if let Some(s) = scale {
            // Apply to all monitors
            for (_id, monitor) in &mut self.monitors {
                monitor.set_scale(s);
            }
            crate::kprintln!("HiDPI: Set global scale to {}x", s.as_percent());
        } else {
            crate::kprintln!("HiDPI: Disabled global scale (using per-monitor)");
        }
    }

    /// Set scaling filter
    pub fn set_filter(&mut self, filter: ScalingFilter) {
        self.settings.filter = filter;

        for (_id, monitor) in &mut self.monitors {
            monitor.filter = filter;
        }

        crate::kprintln!("HiDPI: Set filter to {}", filter.name());
    }

    /// Set text scaling (on top of display scaling)
    pub fn set_text_scale(&mut self, scale: f32) {
        self.settings.text_scale = scale.clamp(0.5, 3.0);
        crate::kprintln!("HiDPI: Set text scale to {}x", self.settings.text_scale);
    }

    /// Set cursor scale
    pub fn set_cursor_scale(&mut self, scale: f32) {
        self.settings.cursor_scale = scale.clamp(0.5, 4.0);
        crate::kprintln!("HiDPI: Set cursor scale to {}x", self.settings.cursor_scale);
    }

    /// Get effective DPI for a monitor
    pub fn get_effective_dpi(&self, connector_id: u32) -> Option<f32> {
        if let Some(dpi) = self.settings.force_dpi {
            return Some(dpi);
        }

        self.monitors.get(&connector_id).map(|m| (m.dpi_x + m.dpi_y) / 2.0)
    }

    /// Get effective scale for a monitor
    pub fn get_effective_scale(&self, connector_id: u32) -> Option<f32> {
        if let Some(global) = self.settings.global_scale {
            return Some(global.as_f32());
        }

        self.monitors.get(&connector_id).map(|m| m.scale.as_f32())
    }

    /// Convert logical coordinates to physical
    pub fn logical_to_physical(&self, connector_id: u32, x: i32, y: i32) -> (i32, i32) {
        let scale = self.get_effective_scale(connector_id).unwrap_or(1.0);
        ((x as f32 * scale) as i32, (y as f32 * scale) as i32)
    }

    /// Convert physical coordinates to logical
    pub fn physical_to_logical(&self, connector_id: u32, x: i32, y: i32) -> (i32, i32) {
        let scale = self.get_effective_scale(connector_id).unwrap_or(1.0);
        ((x as f32 / scale) as i32, (y as f32 / scale) as i32)
    }

    /// Get monitor scaling info
    pub fn get_monitor(&self, connector_id: u32) -> Option<&MonitorScaling> {
        self.monitors.get(&connector_id)
    }

    /// Get status string
    pub fn get_status(&self) -> String {
        let mut status = String::new();

        status.push_str("HiDPI Status:\n");
        status.push_str(&alloc::format!("  Auto Detect: {}\n", self.settings.auto_detect));
        status.push_str(&alloc::format!("  Prefer Integer: {}\n", self.settings.prefer_integer));
        status.push_str(&alloc::format!("  Filter: {}\n", self.settings.filter.name()));
        status.push_str(&alloc::format!("  Text Scale: {}x\n", self.settings.text_scale));
        status.push_str(&alloc::format!("  Cursor Scale: {}x\n", self.settings.cursor_scale));

        if let Some(global) = self.settings.global_scale {
            status.push_str(&alloc::format!("  Global Scale: {}x\n", global.as_percent()));
        }

        status.push_str(&alloc::format!("  Monitors: {}\n", self.monitors.len()));
        for (id, monitor) in &self.monitors {
            status.push_str(&alloc::format!("    Connector {}:\n", id));
            status.push_str(&alloc::format!("      Name: {}\n", monitor.name));
            status.push_str(&alloc::format!("      Resolution: {}x{}\n",
                monitor.resolution.width, monitor.resolution.height));
            status.push_str(&alloc::format!("      Physical: {}x{} mm ({:.1}\")\n",
                monitor.physical.width_mm, monitor.physical.height_mm,
                monitor.physical.diagonal_inch));
            status.push_str(&alloc::format!("      DPI: {:.0}x{:.0}\n",
                monitor.dpi_x, monitor.dpi_y));
            status.push_str(&alloc::format!("      Scale: {}% ({})\n",
                monitor.scale.as_percent(), monitor.method.name()));
            status.push_str(&alloc::format!("      Effective: {}x{}\n",
                monitor.effective_resolution.width, monitor.effective_resolution.height));
        }

        status
    }
}

/// Global HiDPI controller
static HIDPI_CONTROLLER: TicketSpinlock<Option<HiDpiController>> = TicketSpinlock::new(None);

/// Initialize HiDPI
pub fn init() -> Result<(), &'static str> {
    let mut guard = HIDPI_CONTROLLER.lock();
    let mut controller = HiDpiController::new();
    controller.init()?;
    *guard = Some(controller);
    Ok(())
}

/// Get HiDPI controller
pub fn get_controller() -> Option<&'static TicketSpinlock<Option<HiDpiController>>> {
    Some(&HIDPI_CONTROLLER)
}
