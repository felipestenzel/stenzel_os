//! Multi-Monitor Support
//!
//! Manages multiple displays and provides a unified desktop experience across monitors.
//! Features:
//! - Monitor detection and hotplug
//! - Display arrangement (extended, clone, single)
//! - Per-monitor resolution and refresh rate
//! - Cross-monitor window management
//! - Primary monitor selection

#![allow(dead_code)]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, AtomicBool, Ordering};
use spin::{Mutex, RwLock};

// ============================================================================
// Constants
// ============================================================================

/// Maximum number of monitors supported
pub const MAX_MONITORS: usize = 8;

/// EDID block size
const EDID_BLOCK_SIZE: usize = 128;

// ============================================================================
// Monitor Information
// ============================================================================

/// Unique monitor identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MonitorId(pub u32);

/// Display connector type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectorType {
    /// VGA (analog)
    Vga,
    /// DVI
    Dvi,
    /// HDMI
    Hdmi,
    /// DisplayPort
    DisplayPort,
    /// Embedded DisplayPort (laptops)
    Edp,
    /// LVDS (internal LCD)
    Lvds,
    /// USB-C / Thunderbolt
    UsbC,
    /// Composite video
    Composite,
    /// Unknown
    Unknown,
}

impl ConnectorType {
    pub fn name(&self) -> &'static str {
        match self {
            ConnectorType::Vga => "VGA",
            ConnectorType::Dvi => "DVI",
            ConnectorType::Hdmi => "HDMI",
            ConnectorType::DisplayPort => "DP",
            ConnectorType::Edp => "eDP",
            ConnectorType::Lvds => "LVDS",
            ConnectorType::UsbC => "USB-C",
            ConnectorType::Composite => "Composite",
            ConnectorType::Unknown => "Unknown",
        }
    }
}

/// Monitor connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Monitor is connected and active
    Connected,
    /// Monitor is disconnected
    Disconnected,
    /// Connection state unknown
    Unknown,
}

/// Display mode (resolution + refresh)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayMode {
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// Refresh rate in Hz (times 1000 for precision, e.g., 60000 = 60Hz)
    pub refresh_millihertz: u32,
    /// Bits per pixel
    pub bpp: u8,
    /// Is this an interlaced mode?
    pub interlaced: bool,
}

impl DisplayMode {
    /// Create a new display mode
    pub fn new(width: u32, height: u32, refresh_hz: u32, bpp: u8) -> Self {
        Self {
            width,
            height,
            refresh_millihertz: refresh_hz * 1000,
            bpp,
            interlaced: false,
        }
    }

    /// Get refresh rate in Hz
    pub fn refresh_hz(&self) -> u32 {
        self.refresh_millihertz / 1000
    }

    /// Format as string (e.g., "1920x1080@60")
    pub fn format(&self) -> String {
        alloc::format!("{}x{}@{}", self.width, self.height, self.refresh_hz())
    }

    /// Check if this is a standard 16:9 resolution
    pub fn is_16_9(&self) -> bool {
        let ratio = self.width as f32 / self.height as f32;
        (ratio - 16.0 / 9.0).abs() < 0.01
    }

    /// Check if this is a standard 16:10 resolution
    pub fn is_16_10(&self) -> bool {
        let ratio = self.width as f32 / self.height as f32;
        (ratio - 16.0 / 10.0).abs() < 0.01
    }
}

/// Common display modes
pub mod modes {
    use super::DisplayMode;

    pub const VGA_640X480: DisplayMode = DisplayMode {
        width: 640, height: 480, refresh_millihertz: 60000, bpp: 32, interlaced: false,
    };
    pub const SVGA_800X600: DisplayMode = DisplayMode {
        width: 800, height: 600, refresh_millihertz: 60000, bpp: 32, interlaced: false,
    };
    pub const XGA_1024X768: DisplayMode = DisplayMode {
        width: 1024, height: 768, refresh_millihertz: 60000, bpp: 32, interlaced: false,
    };
    pub const WXGA_1280X800: DisplayMode = DisplayMode {
        width: 1280, height: 800, refresh_millihertz: 60000, bpp: 32, interlaced: false,
    };
    pub const SXGA_1280X1024: DisplayMode = DisplayMode {
        width: 1280, height: 1024, refresh_millihertz: 60000, bpp: 32, interlaced: false,
    };
    pub const HD_1366X768: DisplayMode = DisplayMode {
        width: 1366, height: 768, refresh_millihertz: 60000, bpp: 32, interlaced: false,
    };
    pub const WXGA_PLUS_1440X900: DisplayMode = DisplayMode {
        width: 1440, height: 900, refresh_millihertz: 60000, bpp: 32, interlaced: false,
    };
    pub const HD_PLUS_1600X900: DisplayMode = DisplayMode {
        width: 1600, height: 900, refresh_millihertz: 60000, bpp: 32, interlaced: false,
    };
    pub const UXGA_1600X1200: DisplayMode = DisplayMode {
        width: 1600, height: 1200, refresh_millihertz: 60000, bpp: 32, interlaced: false,
    };
    pub const WSXGA_PLUS_1680X1050: DisplayMode = DisplayMode {
        width: 1680, height: 1050, refresh_millihertz: 60000, bpp: 32, interlaced: false,
    };
    pub const FHD_1920X1080: DisplayMode = DisplayMode {
        width: 1920, height: 1080, refresh_millihertz: 60000, bpp: 32, interlaced: false,
    };
    pub const WUXGA_1920X1200: DisplayMode = DisplayMode {
        width: 1920, height: 1200, refresh_millihertz: 60000, bpp: 32, interlaced: false,
    };
    pub const QHD_2560X1440: DisplayMode = DisplayMode {
        width: 2560, height: 1440, refresh_millihertz: 60000, bpp: 32, interlaced: false,
    };
    pub const WQXGA_2560X1600: DisplayMode = DisplayMode {
        width: 2560, height: 1600, refresh_millihertz: 60000, bpp: 32, interlaced: false,
    };
    pub const UHD_3840X2160: DisplayMode = DisplayMode {
        width: 3840, height: 2160, refresh_millihertz: 60000, bpp: 32, interlaced: false,
    };
}

/// EDID information (parsed from monitor)
#[derive(Debug, Clone)]
pub struct EdidInfo {
    /// Manufacturer ID (3 letters)
    pub manufacturer: [u8; 3],
    /// Product code
    pub product_code: u16,
    /// Serial number
    pub serial_number: u32,
    /// Week of manufacture
    pub manufacture_week: u8,
    /// Year of manufacture
    pub manufacture_year: u16,
    /// Monitor name (from descriptor)
    pub name: String,
    /// Supported modes
    pub supported_modes: Vec<DisplayMode>,
    /// Preferred mode (native resolution)
    pub preferred_mode: Option<DisplayMode>,
    /// Maximum image size (cm)
    pub max_size_cm: (u8, u8),
    /// Supports audio
    pub supports_audio: bool,
    /// Display type (digital/analog)
    pub is_digital: bool,
}

impl EdidInfo {
    /// Parse EDID data
    pub fn from_bytes(data: &[u8; 128]) -> Option<Self> {
        // Check EDID header
        if data[0..8] != [0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00] {
            return None;
        }

        // Manufacturer ID (PNP ID in big-endian compressed form)
        let mfg_bytes = u16::from_be_bytes([data[8], data[9]]);
        let manufacturer = [
            ((mfg_bytes >> 10) & 0x1F) as u8 + b'A' - 1,
            ((mfg_bytes >> 5) & 0x1F) as u8 + b'A' - 1,
            (mfg_bytes & 0x1F) as u8 + b'A' - 1,
        ];

        // Product code (little-endian)
        let product_code = u16::from_le_bytes([data[10], data[11]]);

        // Serial number (little-endian)
        let serial_number = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);

        // Manufacture week and year
        let manufacture_week = data[16];
        let manufacture_year = 1990 + data[17] as u16;

        // Input definition
        let is_digital = (data[20] & 0x80) != 0;

        // Maximum image size (cm)
        let max_size_cm = (data[21], data[22]);

        // Parse preferred timing (detailed timing descriptor #1)
        let preferred_mode = parse_detailed_timing(&data[54..72]);

        // Parse monitor name from descriptors
        let name = parse_monitor_name(data);

        // Build supported modes from standard timings and detailed timings
        let mut supported_modes = Vec::new();
        if let Some(ref mode) = preferred_mode {
            supported_modes.push(*mode);
        }
        // Add standard timings (bytes 38-53)
        for i in 0..8 {
            if let Some(mode) = parse_standard_timing(data[38 + i * 2], data[39 + i * 2]) {
                if !supported_modes.iter().any(|m| m.width == mode.width && m.height == mode.height) {
                    supported_modes.push(mode);
                }
            }
        }

        Some(Self {
            manufacturer,
            product_code,
            serial_number,
            manufacture_week,
            manufacture_year,
            name,
            supported_modes,
            preferred_mode,
            max_size_cm,
            supports_audio: false, // Would need CEA extension
            is_digital,
        })
    }

    /// Get manufacturer as string
    pub fn manufacturer_string(&self) -> String {
        String::from_utf8_lossy(&self.manufacturer).to_string()
    }
}

/// Parse a detailed timing descriptor
fn parse_detailed_timing(data: &[u8]) -> Option<DisplayMode> {
    if data.len() < 18 {
        return None;
    }

    // Pixel clock in 10kHz units
    let pixel_clock = u16::from_le_bytes([data[0], data[1]]) as u32;
    if pixel_clock == 0 {
        return None;
    }

    // Horizontal timing
    let h_active = ((data[4] as u32 & 0xF0) << 4) | data[2] as u32;
    let h_blank = ((data[4] as u32 & 0x0F) << 8) | data[3] as u32;

    // Vertical timing
    let v_active = ((data[7] as u32 & 0xF0) << 4) | data[5] as u32;
    let v_blank = ((data[7] as u32 & 0x0F) << 8) | data[6] as u32;

    // Calculate refresh rate
    let h_total = h_active + h_blank;
    let v_total = v_active + v_blank;
    let refresh = if h_total > 0 && v_total > 0 {
        (pixel_clock * 10000) / (h_total * v_total)
    } else {
        60
    };

    // Interlaced flag
    let interlaced = (data[17] & 0x80) != 0;

    Some(DisplayMode {
        width: h_active,
        height: v_active,
        refresh_millihertz: refresh * 1000,
        bpp: 32,
        interlaced,
    })
}

/// Parse a standard timing entry
fn parse_standard_timing(byte1: u8, byte2: u8) -> Option<DisplayMode> {
    if byte1 == 0x01 && byte2 == 0x01 {
        return None; // Unused entry
    }
    if byte1 == 0x00 {
        return None;
    }

    let width = (byte1 as u32 + 31) * 8;
    let aspect = (byte2 >> 6) & 0x03;
    let height = match aspect {
        0 => width * 10 / 16, // 16:10
        1 => width * 3 / 4,   // 4:3
        2 => width * 4 / 5,   // 5:4
        _ => width * 9 / 16,  // 16:9
    };
    let refresh = (byte2 & 0x3F) as u32 + 60;

    Some(DisplayMode {
        width,
        height,
        refresh_millihertz: refresh * 1000,
        bpp: 32,
        interlaced: false,
    })
}

/// Parse monitor name from descriptor blocks
fn parse_monitor_name(data: &[u8]) -> String {
    // Check descriptor blocks at offsets 54, 72, 90, 108
    for offset in [54, 72, 90, 108].iter() {
        let block = &data[*offset..*offset + 18];
        // Check if it's a monitor name descriptor (tag 0xFC)
        if block[0] == 0 && block[1] == 0 && block[3] == 0xFC {
            let name_bytes: Vec<u8> = block[5..18]
                .iter()
                .take_while(|&&b| b != 0x0A && b != 0x00)
                .copied()
                .collect();
            return String::from_utf8_lossy(&name_bytes).trim().to_string();
        }
    }
    String::from("Unknown Monitor")
}

// ============================================================================
// Monitor Structure
// ============================================================================

/// Represents a single monitor
#[derive(Debug)]
pub struct Monitor {
    /// Unique identifier
    pub id: MonitorId,
    /// Connector type
    pub connector: ConnectorType,
    /// Connector index (e.g., HDMI-1, HDMI-2)
    pub connector_index: u8,
    /// Connection state
    pub state: ConnectionState,
    /// EDID information
    pub edid: Option<EdidInfo>,
    /// Current display mode
    pub current_mode: Option<DisplayMode>,
    /// Available modes
    pub available_modes: Vec<DisplayMode>,
    /// Position in virtual desktop (x, y)
    pub position: (i32, i32),
    /// Is this the primary monitor?
    pub is_primary: bool,
    /// Rotation in degrees (0, 90, 180, 270)
    pub rotation: u16,
    /// Scale factor (1.0 = 100%, 2.0 = 200%)
    pub scale: f32,
    /// Brightness (0-100)
    pub brightness: u8,
    /// Framebuffer address (if directly mapped)
    pub framebuffer_addr: Option<u64>,
    /// Framebuffer stride (bytes per row)
    pub framebuffer_stride: u32,
}

impl Monitor {
    /// Create a new monitor
    pub fn new(id: MonitorId, connector: ConnectorType, connector_index: u8) -> Self {
        Self {
            id,
            connector,
            connector_index,
            state: ConnectionState::Unknown,
            edid: None,
            current_mode: None,
            available_modes: Vec::new(),
            position: (0, 0),
            is_primary: false,
            rotation: 0,
            scale: 1.0,
            brightness: 100,
            framebuffer_addr: None,
            framebuffer_stride: 0,
        }
    }

    /// Get the monitor name
    pub fn name(&self) -> String {
        if let Some(ref edid) = self.edid {
            if !edid.name.is_empty() {
                return edid.name.clone();
            }
        }
        alloc::format!("{}-{}", self.connector.name(), self.connector_index)
    }

    /// Get effective resolution (accounting for rotation)
    pub fn effective_resolution(&self) -> Option<(u32, u32)> {
        self.current_mode.map(|mode| {
            if self.rotation == 90 || self.rotation == 270 {
                (mode.height, mode.width)
            } else {
                (mode.width, mode.height)
            }
        })
    }

    /// Get bounding rectangle in virtual desktop
    pub fn bounds(&self) -> Option<(i32, i32, u32, u32)> {
        self.effective_resolution().map(|(w, h)| {
            (self.position.0, self.position.1, w, h)
        })
    }

    /// Check if a point is on this monitor
    pub fn contains_point(&self, x: i32, y: i32) -> bool {
        if let Some((bx, by, bw, bh)) = self.bounds() {
            x >= bx && x < bx + bw as i32 && y >= by && y < by + bh as i32
        } else {
            false
        }
    }

    /// Set display mode
    pub fn set_mode(&mut self, mode: DisplayMode) -> bool {
        // Check if mode is supported
        if self.available_modes.iter().any(|m|
            m.width == mode.width && m.height == mode.height && m.refresh_hz() == mode.refresh_hz()
        ) || self.available_modes.is_empty() {
            self.current_mode = Some(mode);
            self.framebuffer_stride = mode.width * (mode.bpp as u32 / 8);
            true
        } else {
            false
        }
    }
}

// ============================================================================
// Display Arrangement
// ============================================================================

/// Display arrangement mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrangementMode {
    /// Extend desktop across all monitors
    Extended,
    /// Clone primary monitor to all others
    Clone,
    /// Only use primary monitor
    PrimaryOnly,
    /// Only use external monitors
    ExternalOnly,
}

/// Display arrangement configuration
#[derive(Debug, Clone)]
pub struct DisplayArrangement {
    /// Arrangement mode
    pub mode: ArrangementMode,
    /// Primary monitor ID
    pub primary: Option<MonitorId>,
    /// Total virtual desktop size
    pub virtual_width: u32,
    pub virtual_height: u32,
}

impl Default for DisplayArrangement {
    fn default() -> Self {
        Self {
            mode: ArrangementMode::Extended,
            primary: None,
            virtual_width: 0,
            virtual_height: 0,
        }
    }
}

// ============================================================================
// Multi-Monitor Manager
// ============================================================================

/// Manages all connected monitors
pub struct MultiMonitorManager {
    /// All monitors (connected and disconnected)
    monitors: RwLock<Vec<Monitor>>,
    /// Current arrangement
    arrangement: RwLock<DisplayArrangement>,
    /// Next monitor ID
    next_id: AtomicU32,
    /// Hotplug detection enabled
    hotplug_enabled: AtomicBool,
    /// Callback for monitor changes
    change_callbacks: Mutex<Vec<fn(MonitorEvent)>>,
}

/// Monitor event for callbacks
#[derive(Debug, Clone)]
pub enum MonitorEvent {
    /// Monitor connected
    Connected(MonitorId),
    /// Monitor disconnected
    Disconnected(MonitorId),
    /// Monitor mode changed
    ModeChanged(MonitorId, DisplayMode),
    /// Arrangement changed
    ArrangementChanged,
}

impl MultiMonitorManager {
    /// Create a new manager
    pub const fn new() -> Self {
        Self {
            monitors: RwLock::new(Vec::new()),
            arrangement: RwLock::new(DisplayArrangement {
                mode: ArrangementMode::Extended,
                primary: None,
                virtual_width: 0,
                virtual_height: 0,
            }),
            next_id: AtomicU32::new(1),
            hotplug_enabled: AtomicBool::new(true),
            change_callbacks: Mutex::new(Vec::new()),
        }
    }

    /// Add a monitor
    pub fn add_monitor(&self, connector: ConnectorType, connector_index: u8) -> MonitorId {
        let id = MonitorId(self.next_id.fetch_add(1, Ordering::Relaxed));
        let monitor = Monitor::new(id, connector, connector_index);

        let mut monitors = self.monitors.write();
        monitors.push(monitor);

        self.notify(MonitorEvent::Connected(id));
        id
    }

    /// Remove a monitor
    pub fn remove_monitor(&self, id: MonitorId) -> bool {
        let mut monitors = self.monitors.write();
        if let Some(pos) = monitors.iter().position(|m| m.id == id) {
            monitors.remove(pos);
            self.notify(MonitorEvent::Disconnected(id));
            true
        } else {
            false
        }
    }

    /// Get a monitor by ID
    pub fn get_monitor(&self, id: MonitorId) -> Option<Monitor> {
        let monitors = self.monitors.read();
        monitors.iter().find(|m| m.id == id).cloned()
    }

    /// Get all connected monitors
    pub fn connected_monitors(&self) -> Vec<MonitorId> {
        self.monitors.read()
            .iter()
            .filter(|m| m.state == ConnectionState::Connected)
            .map(|m| m.id)
            .collect()
    }

    /// Get all monitors
    pub fn all_monitors(&self) -> Vec<Monitor> {
        self.monitors.read().clone()
    }

    /// Get the primary monitor
    pub fn primary_monitor(&self) -> Option<MonitorId> {
        self.arrangement.read().primary
    }

    /// Set the primary monitor
    pub fn set_primary(&self, id: MonitorId) -> bool {
        let monitors = self.monitors.read();
        if monitors.iter().any(|m| m.id == id) {
            drop(monitors);

            let mut monitors = self.monitors.write();
            for m in monitors.iter_mut() {
                m.is_primary = m.id == id;
            }
            drop(monitors);

            self.arrangement.write().primary = Some(id);
            self.notify(MonitorEvent::ArrangementChanged);
            true
        } else {
            false
        }
    }

    /// Set arrangement mode
    pub fn set_arrangement_mode(&self, mode: ArrangementMode) {
        self.arrangement.write().mode = mode;
        self.recalculate_arrangement();
        self.notify(MonitorEvent::ArrangementChanged);
    }

    /// Get current arrangement
    pub fn arrangement(&self) -> DisplayArrangement {
        self.arrangement.read().clone()
    }

    /// Set monitor position
    pub fn set_position(&self, id: MonitorId, x: i32, y: i32) {
        let mut monitors = self.monitors.write();
        if let Some(monitor) = monitors.iter_mut().find(|m| m.id == id) {
            monitor.position = (x, y);
        }
        drop(monitors);
        self.recalculate_arrangement();
    }

    /// Set monitor mode
    pub fn set_mode(&self, id: MonitorId, mode: DisplayMode) -> bool {
        let mut monitors = self.monitors.write();
        if let Some(monitor) = monitors.iter_mut().find(|m| m.id == id) {
            if monitor.set_mode(mode) {
                drop(monitors);
                self.recalculate_arrangement();
                self.notify(MonitorEvent::ModeChanged(id, mode));
                return true;
            }
        }
        false
    }

    /// Set EDID data for a monitor
    pub fn set_edid(&self, id: MonitorId, edid_data: &[u8; 128]) {
        let mut monitors = self.monitors.write();
        if let Some(monitor) = monitors.iter_mut().find(|m| m.id == id) {
            if let Some(edid) = EdidInfo::from_bytes(edid_data) {
                monitor.available_modes = edid.supported_modes.clone();
                if let Some(preferred) = edid.preferred_mode {
                    if monitor.current_mode.is_none() {
                        monitor.current_mode = Some(preferred);
                    }
                }
                monitor.edid = Some(edid);
            }
        }
    }

    /// Set monitor connection state
    pub fn set_connected(&self, id: MonitorId, connected: bool) {
        let mut monitors = self.monitors.write();
        if let Some(monitor) = monitors.iter_mut().find(|m| m.id == id) {
            let old_state = monitor.state;
            monitor.state = if connected {
                ConnectionState::Connected
            } else {
                ConnectionState::Disconnected
            };

            if old_state != monitor.state {
                let event = if connected {
                    MonitorEvent::Connected(id)
                } else {
                    MonitorEvent::Disconnected(id)
                };
                drop(monitors);
                self.notify(event);
            }
        }
    }

    /// Recalculate virtual desktop size
    fn recalculate_arrangement(&self) {
        let monitors = self.monitors.read();
        let mut arrangement = self.arrangement.write();

        let mut max_x = 0i32;
        let mut max_y = 0i32;

        for monitor in monitors.iter() {
            if let Some((x, y, w, h)) = monitor.bounds() {
                let right = x + w as i32;
                let bottom = y + h as i32;
                if right > max_x {
                    max_x = right;
                }
                if bottom > max_y {
                    max_y = bottom;
                }
            }
        }

        arrangement.virtual_width = max_x.max(0) as u32;
        arrangement.virtual_height = max_y.max(0) as u32;
    }

    /// Find monitor at point
    pub fn monitor_at_point(&self, x: i32, y: i32) -> Option<MonitorId> {
        let monitors = self.monitors.read();
        monitors.iter()
            .find(|m| m.contains_point(x, y))
            .map(|m| m.id)
    }

    /// Auto-arrange monitors horizontally
    pub fn auto_arrange_horizontal(&self) {
        let mut monitors = self.monitors.write();
        let mut x = 0i32;

        for monitor in monitors.iter_mut() {
            if monitor.state == ConnectionState::Connected {
                monitor.position = (x, 0);
                if let Some((w, _)) = monitor.effective_resolution() {
                    x += w as i32;
                }
            }
        }
        drop(monitors);
        self.recalculate_arrangement();
        self.notify(MonitorEvent::ArrangementChanged);
    }

    /// Register a change callback
    pub fn on_change(&self, callback: fn(MonitorEvent)) {
        self.change_callbacks.lock().push(callback);
    }

    /// Notify all callbacks
    fn notify(&self, event: MonitorEvent) {
        let callbacks = self.change_callbacks.lock();
        for callback in callbacks.iter() {
            callback(event.clone());
        }
    }

    /// Print monitor info
    pub fn print_info(&self) {
        let monitors = self.monitors.read();
        let arrangement = self.arrangement.read();

        crate::kprintln!(
            "multimon: {} monitors, virtual desktop {}x{}",
            monitors.len(),
            arrangement.virtual_width,
            arrangement.virtual_height
        );

        for monitor in monitors.iter() {
            let mode_str = monitor.current_mode
                .map(|m| m.format())
                .unwrap_or_else(|| String::from("no mode"));
            crate::kprintln!(
                "  {}: {} at ({}, {}) - {} {}",
                monitor.name(),
                mode_str,
                monitor.position.0,
                monitor.position.1,
                if monitor.is_primary { "[PRIMARY]" } else { "" },
                match monitor.state {
                    ConnectionState::Connected => "connected",
                    ConnectionState::Disconnected => "disconnected",
                    ConnectionState::Unknown => "unknown",
                }
            );
        }
    }
}

impl Clone for Monitor {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            connector: self.connector,
            connector_index: self.connector_index,
            state: self.state,
            edid: self.edid.clone(),
            current_mode: self.current_mode,
            available_modes: self.available_modes.clone(),
            position: self.position,
            is_primary: self.is_primary,
            rotation: self.rotation,
            scale: self.scale,
            brightness: self.brightness,
            framebuffer_addr: self.framebuffer_addr,
            framebuffer_stride: self.framebuffer_stride,
        }
    }
}

// ============================================================================
// Global State
// ============================================================================

static MULTI_MONITOR: MultiMonitorManager = MultiMonitorManager::new();

/// Get the multi-monitor manager
pub fn manager() -> &'static MultiMonitorManager {
    &MULTI_MONITOR
}

/// Initialize multi-monitor support
pub fn init() {
    crate::kprintln!("multimon: multi-monitor support initialized");
}

/// Add a monitor from framebuffer info
pub fn add_from_framebuffer(
    connector: ConnectorType,
    width: u32,
    height: u32,
    framebuffer_addr: u64,
    stride: u32,
) -> MonitorId {
    let id = MULTI_MONITOR.add_monitor(connector, 0);

    let mut monitors = MULTI_MONITOR.monitors.write();
    if let Some(monitor) = monitors.iter_mut().find(|m| m.id == id) {
        monitor.current_mode = Some(DisplayMode::new(width, height, 60, 32));
        monitor.framebuffer_addr = Some(framebuffer_addr);
        monitor.framebuffer_stride = stride;
        monitor.state = ConnectionState::Connected;
        monitor.is_primary = true;
    }
    drop(monitors);

    MULTI_MONITOR.set_primary(id);
    MULTI_MONITOR.recalculate_arrangement();

    id
}

/// Get all connected monitors
pub fn connected_monitors() -> Vec<MonitorId> {
    MULTI_MONITOR.connected_monitors()
}

/// Get primary monitor
pub fn primary_monitor() -> Option<MonitorId> {
    MULTI_MONITOR.primary_monitor()
}

/// Get virtual desktop size
pub fn virtual_desktop_size() -> (u32, u32) {
    let arr = MULTI_MONITOR.arrangement();
    (arr.virtual_width, arr.virtual_height)
}

/// Find which monitor contains a point
pub fn monitor_at_point(x: i32, y: i32) -> Option<MonitorId> {
    MULTI_MONITOR.monitor_at_point(x, y)
}

/// Print monitor status
pub fn print_status() {
    MULTI_MONITOR.print_info();
}
