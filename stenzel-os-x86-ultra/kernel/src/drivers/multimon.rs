//! Multi-Monitor Support
//!
//! Provides multi-monitor configuration and management:
//! - Monitor detection and enumeration
//! - Display arrangement (extended, mirror, single)
//! - Per-monitor resolution and refresh rate
//! - Primary monitor selection
//! - Output cloning and spanning

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use spin::Mutex;

use super::display_pipe::{Connector, ConnectionStatus, ConnectorType as DpConnectorType, DisplayMode, Encoder};

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static MULTIMON_STATE: Mutex<Option<MultiMonitorState>> = Mutex::new(None);

/// Multi-monitor state
#[derive(Debug)]
pub struct MultiMonitorState {
    /// Connected monitors
    pub monitors: BTreeMap<u32, MonitorInfo>,
    /// Display arrangement
    pub arrangement: DisplayArrangement,
    /// Primary monitor ID
    pub primary_monitor: Option<u32>,
    /// Total virtual desktop size
    pub virtual_width: u32,
    pub virtual_height: u32,
}

/// Monitor information
#[derive(Debug, Clone)]
pub struct MonitorInfo {
    /// Monitor ID (connector ID)
    pub id: u32,
    /// Monitor name (from EDID)
    pub name: String,
    /// Manufacturer (from EDID)
    pub manufacturer: String,
    /// Serial number (from EDID)
    pub serial: String,
    /// Connection type
    pub connection_type: ConnectionType,
    /// Physical size in mm
    pub width_mm: u32,
    pub height_mm: u32,
    /// Current mode
    pub current_mode: Option<DisplayMode>,
    /// Supported modes
    pub supported_modes: Vec<DisplayMode>,
    /// Position in virtual desktop
    pub x: i32,
    pub y: i32,
    /// Rotation
    pub rotation: Rotation,
    /// Scale factor
    pub scale: f32,
    /// Is primary
    pub is_primary: bool,
    /// Is enabled
    pub enabled: bool,
    /// EDID data
    pub edid: Option<Vec<u8>>,
}

/// Connection type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionType {
    Unknown,
    Vga,
    DviI,
    DviD,
    DviA,
    Composite,
    SVideo,
    Lvds,
    Component,
    NinePinDin,
    DisplayPort,
    Hdmi,
    Edp,
    Virtual,
    Dsi,
    Dpi,
    Writeback,
    UsbC,
}

/// Display rotation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rotation {
    Normal,
    Left,  // 90 degrees counter-clockwise
    Right, // 90 degrees clockwise
    Inverted, // 180 degrees
}

/// Display arrangement mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayArrangement {
    /// Single display only
    Single,
    /// Clone/mirror all displays
    Clone,
    /// Extended desktop
    Extended,
    /// Internal display only (for laptops)
    InternalOnly,
    /// External display only
    ExternalOnly,
}

/// Monitor position preset
#[derive(Debug, Clone, Copy)]
pub enum PositionPreset {
    LeftOf(u32),
    RightOf(u32),
    Above(u32),
    Below(u32),
    SameAs(u32),
}

/// Error type
#[derive(Debug, Clone, Copy)]
pub enum MultiMonError {
    NotInitialized,
    MonitorNotFound,
    InvalidMode,
    ConfigurationFailed,
    EdidParseFailed,
    NoConnectedMonitors,
}

/// Initialize multi-monitor subsystem
pub fn init() -> Result<(), MultiMonError> {
    if INITIALIZED.load(Ordering::Acquire) {
        return Ok(());
    }

    let state = MultiMonitorState {
        monitors: BTreeMap::new(),
        arrangement: DisplayArrangement::Extended,
        primary_monitor: None,
        virtual_width: 0,
        virtual_height: 0,
    };

    *MULTIMON_STATE.lock() = Some(state);
    INITIALIZED.store(true, Ordering::Release);

    crate::kprintln!("multimon: Multi-monitor subsystem initialized");
    Ok(())
}

/// Detect and enumerate connected monitors
pub fn detect_monitors() -> Result<Vec<u32>, MultiMonError> {
    let mut state = MULTIMON_STATE.lock();
    let state = state.as_mut().ok_or(MultiMonError::NotInitialized)?;

    let mut detected = Vec::new();

    // Query connectors from display_pipe
    let connectors = super::display_pipe::get_connectors();

    for connector in connectors {
        if connector.connection == ConnectionStatus::Connected {
            let monitor = create_monitor_info(&connector)?;
            let id = monitor.id;
            state.monitors.insert(id, monitor);
            detected.push(id);
        }
    }

    // Set primary if not set
    if state.primary_monitor.is_none() && !detected.is_empty() {
        state.primary_monitor = Some(detected[0]);
        if let Some(mon) = state.monitors.get_mut(&detected[0]) {
            mon.is_primary = true;
        }
    }

    // Update virtual desktop size
    recalculate_virtual_size(state);

    crate::kprintln!("multimon: Detected {} monitor(s)", detected.len());
    Ok(detected)
}

/// Create monitor info from connector
fn create_monitor_info(connector: &Connector) -> Result<MonitorInfo, MultiMonError> {
    let (name, manufacturer, serial, width_mm, height_mm) =
        if let Some(ref edid) = connector.edid {
            parse_edid_info(edid)?
        } else {
            (
                format!("Display {}", connector.id),
                String::from("Unknown"),
                String::new(),
                0,
                0,
            )
        };

    let conn_type = match connector.connector_type {
        DpConnectorType::Vga => ConnectionType::Vga,
        DpConnectorType::DviI => ConnectionType::DviI,
        DpConnectorType::DviD => ConnectionType::DviD,
        DpConnectorType::DviA => ConnectionType::DviA,
        DpConnectorType::DisplayPort => ConnectionType::DisplayPort,
        DpConnectorType::HdmiA | DpConnectorType::HdmiB => ConnectionType::Hdmi,
        DpConnectorType::Edp => ConnectionType::Edp,
        DpConnectorType::Virtual => ConnectionType::Virtual,
        DpConnectorType::Lvds => ConnectionType::Lvds,
        DpConnectorType::Dsi => ConnectionType::Dsi,
        DpConnectorType::Dpi => ConnectionType::Dpi,
        DpConnectorType::Writeback => ConnectionType::Writeback,
        DpConnectorType::Usb => ConnectionType::UsbC,
        _ => ConnectionType::Unknown,
    };

    Ok(MonitorInfo {
        id: connector.id,
        name,
        manufacturer,
        serial,
        connection_type: conn_type,
        width_mm,
        height_mm,
        current_mode: connector.modes.first().cloned(),
        supported_modes: connector.modes.clone(),
        x: 0,
        y: 0,
        rotation: Rotation::Normal,
        scale: 1.0,
        is_primary: false,
        enabled: true,
        edid: connector.edid.clone(),
    })
}

/// Parse EDID for monitor information
fn parse_edid_info(edid: &[u8]) -> Result<(String, String, String, u32, u32), MultiMonError> {
    if edid.len() < 128 {
        return Err(MultiMonError::EdidParseFailed);
    }

    // Check EDID header
    let header = &edid[0..8];
    if header != [0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00] {
        return Err(MultiMonError::EdidParseFailed);
    }

    // Manufacturer ID (bytes 8-9)
    let mfg_id = ((edid[8] as u16) << 8) | edid[9] as u16;
    let c1 = ((mfg_id >> 10) & 0x1f) as u8 + b'A' - 1;
    let c2 = ((mfg_id >> 5) & 0x1f) as u8 + b'A' - 1;
    let c3 = (mfg_id & 0x1f) as u8 + b'A' - 1;
    let manufacturer = String::from_utf8_lossy(&[c1, c2, c3]).to_string();

    // Physical size (bytes 21-22, in cm)
    let width_cm = edid[21] as u32;
    let height_cm = edid[22] as u32;

    // Parse descriptor blocks for monitor name
    let mut name = String::new();
    let mut serial = String::new();

    for i in 0..4 {
        let base = 54 + i * 18;
        if base + 18 > edid.len() {
            break;
        }

        // Check if it's a descriptor block (not timing)
        if edid[base] == 0 && edid[base + 1] == 0 {
            let tag = edid[base + 3];
            match tag {
                0xfc => {
                    // Monitor name
                    name = parse_edid_string(&edid[base + 5..base + 18]);
                }
                0xff => {
                    // Serial number
                    serial = parse_edid_string(&edid[base + 5..base + 18]);
                }
                _ => {}
            }
        }
    }

    if name.is_empty() {
        name = format!("{} Monitor", manufacturer);
    }

    Ok((name, manufacturer, serial, width_cm * 10, height_cm * 10))
}

/// Parse EDID string (13 bytes, space-padded or newline-terminated)
fn parse_edid_string(data: &[u8]) -> String {
    let mut result = String::new();
    for &b in data {
        if b == 0x0a || b == 0x00 {
            break;
        }
        if b >= 0x20 && b <= 0x7e {
            result.push(b as char);
        }
    }
    result.trim().to_string()
}

/// Recalculate virtual desktop size
fn recalculate_virtual_size(state: &mut MultiMonitorState) {
    let mut max_x = 0i32;
    let mut max_y = 0i32;

    for mon in state.monitors.values() {
        if !mon.enabled {
            continue;
        }
        if let Some(ref mode) = mon.current_mode {
            let (w, h) = match mon.rotation {
                Rotation::Normal | Rotation::Inverted => (mode.hdisplay as i32, mode.vdisplay as i32),
                Rotation::Left | Rotation::Right => (mode.vdisplay as i32, mode.hdisplay as i32),
            };
            let scaled_w = (w as f32 * mon.scale) as i32;
            let scaled_h = (h as f32 * mon.scale) as i32;
            max_x = max_x.max(mon.x + scaled_w);
            max_y = max_y.max(mon.y + scaled_h);
        }
    }

    state.virtual_width = max_x.max(0) as u32;
    state.virtual_height = max_y.max(0) as u32;
}

/// Get monitor info
pub fn get_monitor(id: u32) -> Result<MonitorInfo, MultiMonError> {
    let state = MULTIMON_STATE.lock();
    let state = state.as_ref().ok_or(MultiMonError::NotInitialized)?;

    state
        .monitors
        .get(&id)
        .cloned()
        .ok_or(MultiMonError::MonitorNotFound)
}

/// Get all connected monitors
pub fn get_monitors() -> Vec<MonitorInfo> {
    MULTIMON_STATE
        .lock()
        .as_ref()
        .map(|s| s.monitors.values().cloned().collect())
        .unwrap_or_default()
}

/// Set display mode for monitor
pub fn set_mode(monitor_id: u32, mode: DisplayMode) -> Result<(), MultiMonError> {
    let mut state = MULTIMON_STATE.lock();
    let state = state.as_mut().ok_or(MultiMonError::NotInitialized)?;

    let monitor = state
        .monitors
        .get_mut(&monitor_id)
        .ok_or(MultiMonError::MonitorNotFound)?;

    // Verify mode is supported
    if !monitor.supported_modes.iter().any(|m|
        m.hdisplay == mode.hdisplay &&
        m.vdisplay == mode.vdisplay &&
        m.vrefresh == mode.vrefresh
    ) {
        return Err(MultiMonError::InvalidMode);
    }

    monitor.current_mode = Some(mode);
    recalculate_virtual_size(state);

    Ok(())
}

/// Set monitor position
pub fn set_position(monitor_id: u32, x: i32, y: i32) -> Result<(), MultiMonError> {
    let mut state = MULTIMON_STATE.lock();
    let state = state.as_mut().ok_or(MultiMonError::NotInitialized)?;

    let monitor = state
        .monitors
        .get_mut(&monitor_id)
        .ok_or(MultiMonError::MonitorNotFound)?;

    monitor.x = x;
    monitor.y = y;
    recalculate_virtual_size(state);

    Ok(())
}

/// Set monitor position relative to another
pub fn set_position_relative(monitor_id: u32, preset: PositionPreset) -> Result<(), MultiMonError> {
    let state = MULTIMON_STATE.lock();
    let state = state.as_ref().ok_or(MultiMonError::NotInitialized)?;

    let (x, y) = match preset {
        PositionPreset::LeftOf(ref_id) => {
            let ref_mon = state.monitors.get(&ref_id).ok_or(MultiMonError::MonitorNotFound)?;
            let this_mon = state.monitors.get(&monitor_id).ok_or(MultiMonError::MonitorNotFound)?;
            let width = this_mon.current_mode.as_ref().map(|m| m.hdisplay as i32).unwrap_or(0);
            (ref_mon.x - width, ref_mon.y)
        }
        PositionPreset::RightOf(ref_id) => {
            let ref_mon = state.monitors.get(&ref_id).ok_or(MultiMonError::MonitorNotFound)?;
            let width = ref_mon.current_mode.as_ref().map(|m| m.hdisplay as i32).unwrap_or(0);
            (ref_mon.x + width, ref_mon.y)
        }
        PositionPreset::Above(ref_id) => {
            let ref_mon = state.monitors.get(&ref_id).ok_or(MultiMonError::MonitorNotFound)?;
            let this_mon = state.monitors.get(&monitor_id).ok_or(MultiMonError::MonitorNotFound)?;
            let height = this_mon.current_mode.as_ref().map(|m| m.vdisplay as i32).unwrap_or(0);
            (ref_mon.x, ref_mon.y - height)
        }
        PositionPreset::Below(ref_id) => {
            let ref_mon = state.monitors.get(&ref_id).ok_or(MultiMonError::MonitorNotFound)?;
            let height = ref_mon.current_mode.as_ref().map(|m| m.vdisplay as i32).unwrap_or(0);
            (ref_mon.x, ref_mon.y + height)
        }
        PositionPreset::SameAs(ref_id) => {
            let ref_mon = state.monitors.get(&ref_id).ok_or(MultiMonError::MonitorNotFound)?;
            (ref_mon.x, ref_mon.y)
        }
    };
    drop(state);

    set_position(monitor_id, x, y)
}

/// Set primary monitor
pub fn set_primary(monitor_id: u32) -> Result<(), MultiMonError> {
    let mut state = MULTIMON_STATE.lock();
    let state = state.as_mut().ok_or(MultiMonError::NotInitialized)?;

    // Check monitor exists
    if !state.monitors.contains_key(&monitor_id) {
        return Err(MultiMonError::MonitorNotFound);
    }

    // Clear old primary
    if let Some(old_id) = state.primary_monitor {
        if let Some(mon) = state.monitors.get_mut(&old_id) {
            mon.is_primary = false;
        }
    }

    // Set new primary
    state.primary_monitor = Some(monitor_id);
    if let Some(mon) = state.monitors.get_mut(&monitor_id) {
        mon.is_primary = true;
    }

    Ok(())
}

/// Get primary monitor
pub fn get_primary() -> Option<u32> {
    MULTIMON_STATE
        .lock()
        .as_ref()
        .and_then(|s| s.primary_monitor)
}

/// Set display arrangement
pub fn set_arrangement(arrangement: DisplayArrangement) -> Result<(), MultiMonError> {
    let mut state = MULTIMON_STATE.lock();
    let state = state.as_mut().ok_or(MultiMonError::NotInitialized)?;

    state.arrangement = arrangement;

    // Apply arrangement
    match arrangement {
        DisplayArrangement::Clone => {
            // Position all monitors at (0, 0)
            for mon in state.monitors.values_mut() {
                mon.x = 0;
                mon.y = 0;
            }
        }
        DisplayArrangement::Extended => {
            // Arrange monitors horizontally
            let mut x = 0i32;
            for mon in state.monitors.values_mut() {
                if mon.enabled {
                    mon.x = x;
                    mon.y = 0;
                    if let Some(ref mode) = mon.current_mode {
                        x += mode.hdisplay as i32;
                    }
                }
            }
        }
        DisplayArrangement::Single => {
            // Only primary is enabled
            for mon in state.monitors.values_mut() {
                mon.enabled = mon.is_primary;
            }
        }
        DisplayArrangement::InternalOnly => {
            for mon in state.monitors.values_mut() {
                mon.enabled = matches!(mon.connection_type, ConnectionType::Lvds | ConnectionType::Edp | ConnectionType::Dsi);
            }
        }
        DisplayArrangement::ExternalOnly => {
            for mon in state.monitors.values_mut() {
                mon.enabled = !matches!(mon.connection_type, ConnectionType::Lvds | ConnectionType::Edp | ConnectionType::Dsi);
            }
        }
    }

    recalculate_virtual_size(state);
    Ok(())
}

/// Set monitor rotation
pub fn set_rotation(monitor_id: u32, rotation: Rotation) -> Result<(), MultiMonError> {
    let mut state = MULTIMON_STATE.lock();
    let state = state.as_mut().ok_or(MultiMonError::NotInitialized)?;

    let monitor = state
        .monitors
        .get_mut(&monitor_id)
        .ok_or(MultiMonError::MonitorNotFound)?;

    monitor.rotation = rotation;
    recalculate_virtual_size(state);

    Ok(())
}

/// Set monitor scale
pub fn set_scale(monitor_id: u32, scale: f32) -> Result<(), MultiMonError> {
    if scale < 0.5 || scale > 4.0 {
        return Err(MultiMonError::InvalidMode);
    }

    let mut state = MULTIMON_STATE.lock();
    let state = state.as_mut().ok_or(MultiMonError::NotInitialized)?;

    let monitor = state
        .monitors
        .get_mut(&monitor_id)
        .ok_or(MultiMonError::MonitorNotFound)?;

    monitor.scale = scale;
    recalculate_virtual_size(state);

    Ok(())
}

/// Enable/disable monitor
pub fn set_enabled(monitor_id: u32, enabled: bool) -> Result<(), MultiMonError> {
    let mut state = MULTIMON_STATE.lock();
    let state = state.as_mut().ok_or(MultiMonError::NotInitialized)?;

    let monitor = state
        .monitors
        .get_mut(&monitor_id)
        .ok_or(MultiMonError::MonitorNotFound)?;

    monitor.enabled = enabled;
    recalculate_virtual_size(state);

    Ok(())
}

/// Get virtual desktop size
pub fn get_virtual_size() -> (u32, u32) {
    MULTIMON_STATE
        .lock()
        .as_ref()
        .map(|s| (s.virtual_width, s.virtual_height))
        .unwrap_or((0, 0))
}

/// Map point to monitor
pub fn point_to_monitor(x: i32, y: i32) -> Option<u32> {
    let state = MULTIMON_STATE.lock();
    let state = state.as_ref()?;

    for mon in state.monitors.values() {
        if !mon.enabled {
            continue;
        }
        if let Some(ref mode) = mon.current_mode {
            let (w, h) = match mon.rotation {
                Rotation::Normal | Rotation::Inverted => (mode.hdisplay as i32, mode.vdisplay as i32),
                Rotation::Left | Rotation::Right => (mode.vdisplay as i32, mode.hdisplay as i32),
            };
            if x >= mon.x && x < mon.x + w && y >= mon.y && y < mon.y + h {
                return Some(mon.id);
            }
        }
    }

    None
}

/// Get monitor count
pub fn monitor_count() -> usize {
    MULTIMON_STATE
        .lock()
        .as_ref()
        .map(|s| s.monitors.len())
        .unwrap_or(0)
}
