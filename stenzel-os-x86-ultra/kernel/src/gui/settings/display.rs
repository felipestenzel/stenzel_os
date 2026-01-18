//! Display Settings
//!
//! Resolution, scaling, multi-monitor, night light, and refresh rate settings.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use spin::Mutex;

/// Global display settings state
static DISPLAY_SETTINGS: Mutex<Option<DisplaySettings>> = Mutex::new(None);

/// Display settings state
pub struct DisplaySettings {
    /// Connected monitors
    pub monitors: Vec<MonitorInfo>,
    /// Primary monitor index
    pub primary_monitor: usize,
    /// Night light settings
    pub night_light: NightLightSettings,
    /// Global scale factor
    pub global_scale: ScaleFactor,
}

/// Monitor info
#[derive(Debug, Clone)]
pub struct MonitorInfo {
    /// Monitor ID
    pub id: u32,
    /// Monitor name
    pub name: String,
    /// Current resolution
    pub resolution: Resolution,
    /// Available resolutions
    pub available_resolutions: Vec<Resolution>,
    /// Current refresh rate
    pub refresh_rate: u32,
    /// Available refresh rates
    pub available_refresh_rates: Vec<u32>,
    /// Position (x, y)
    pub position: (i32, i32),
    /// Rotation
    pub rotation: Rotation,
    /// Scale factor
    pub scale: ScaleFactor,
    /// Is enabled
    pub enabled: bool,
    /// Is built-in (laptop panel)
    pub is_builtin: bool,
    /// Physical size in mm
    pub physical_size_mm: (u32, u32),
    /// Connection type
    pub connection: ConnectionType,
}

/// Resolution
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

impl Resolution {
    /// Common resolutions
    pub const HD: Resolution = Resolution { width: 1280, height: 720 };
    pub const FHD: Resolution = Resolution { width: 1920, height: 1080 };
    pub const QHD: Resolution = Resolution { width: 2560, height: 1440 };
    pub const UHD: Resolution = Resolution { width: 3840, height: 2160 };

    /// Get aspect ratio string
    pub fn aspect_ratio(&self) -> &'static str {
        let ratio = self.width as f32 / self.height as f32;
        if (ratio - 16.0/9.0).abs() < 0.1 { "16:9" }
        else if (ratio - 16.0/10.0).abs() < 0.1 { "16:10" }
        else if (ratio - 4.0/3.0).abs() < 0.1 { "4:3" }
        else if (ratio - 21.0/9.0).abs() < 0.1 { "21:9" }
        else { "Other" }
    }
}

/// Rotation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rotation {
    Normal,
    Left,
    Right,
    Inverted,
}

impl Rotation {
    pub fn degrees(&self) -> u32 {
        match self {
            Rotation::Normal => 0,
            Rotation::Right => 90,
            Rotation::Inverted => 180,
            Rotation::Left => 270,
        }
    }
}

/// Scale factor
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleFactor {
    Scale100,  // 1x
    Scale125,  // 1.25x
    Scale150,  // 1.5x
    Scale175,  // 1.75x
    Scale200,  // 2x
    Scale250,  // 2.5x
    Scale300,  // 3x
    Custom(u32), // Custom percentage
}

impl ScaleFactor {
    pub fn as_percent(&self) -> u32 {
        match self {
            ScaleFactor::Scale100 => 100,
            ScaleFactor::Scale125 => 125,
            ScaleFactor::Scale150 => 150,
            ScaleFactor::Scale175 => 175,
            ScaleFactor::Scale200 => 200,
            ScaleFactor::Scale250 => 250,
            ScaleFactor::Scale300 => 300,
            ScaleFactor::Custom(p) => *p,
        }
    }

    pub fn as_multiplier(&self) -> f32 {
        self.as_percent() as f32 / 100.0
    }
}

/// Connection type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionType {
    Internal,
    Hdmi,
    DisplayPort,
    Usbc,
    Vga,
    Dvi,
    Unknown,
}

/// Night light settings
#[derive(Debug, Clone)]
pub struct NightLightSettings {
    /// Enabled
    pub enabled: bool,
    /// Color temperature (Kelvin)
    pub temperature: u32,
    /// Schedule mode
    pub schedule: NightLightSchedule,
    /// Current strength (0-100)
    pub strength: u32,
}

/// Night light schedule
#[derive(Debug, Clone)]
pub enum NightLightSchedule {
    /// Manual on/off
    Manual,
    /// Sunset to sunrise
    SunsetToSunrise,
    /// Custom schedule
    Custom { start_hour: u8, start_minute: u8, end_hour: u8, end_minute: u8 },
}

/// Initialize display settings
pub fn init() {
    let mut state = DISPLAY_SETTINGS.lock();
    if state.is_some() {
        return;
    }

    // Detect monitors (placeholder - would query DRM/display drivers)
    let monitors = vec![
        MonitorInfo {
            id: 0,
            name: "Built-in Display".to_string(),
            resolution: Resolution::FHD,
            available_resolutions: vec![
                Resolution::HD,
                Resolution::FHD,
                Resolution::QHD,
            ],
            refresh_rate: 60,
            available_refresh_rates: vec![60],
            position: (0, 0),
            rotation: Rotation::Normal,
            scale: ScaleFactor::Scale100,
            enabled: true,
            is_builtin: true,
            physical_size_mm: (294, 165),
            connection: ConnectionType::Internal,
        },
    ];

    *state = Some(DisplaySettings {
        monitors,
        primary_monitor: 0,
        night_light: NightLightSettings {
            enabled: false,
            temperature: 4000,
            schedule: NightLightSchedule::Manual,
            strength: 50,
        },
        global_scale: ScaleFactor::Scale100,
    });

    crate::kprintln!("display settings: initialized");
}

/// Get monitors
pub fn get_monitors() -> Vec<MonitorInfo> {
    let state = DISPLAY_SETTINGS.lock();
    state.as_ref().map(|s| s.monitors.clone()).unwrap_or_default()
}

/// Set monitor resolution
pub fn set_resolution(monitor_id: u32, resolution: Resolution) -> Result<(), DisplayError> {
    let mut state = DISPLAY_SETTINGS.lock();
    let state = state.as_mut().ok_or(DisplayError::NotInitialized)?;

    let monitor = state.monitors.iter_mut()
        .find(|m| m.id == monitor_id)
        .ok_or(DisplayError::MonitorNotFound)?;

    if !monitor.available_resolutions.contains(&resolution) {
        return Err(DisplayError::UnsupportedResolution);
    }

    monitor.resolution = resolution;

    // TODO: Apply to hardware via DRM

    Ok(())
}

/// Set monitor refresh rate
pub fn set_refresh_rate(monitor_id: u32, rate: u32) -> Result<(), DisplayError> {
    let mut state = DISPLAY_SETTINGS.lock();
    let state = state.as_mut().ok_or(DisplayError::NotInitialized)?;

    let monitor = state.monitors.iter_mut()
        .find(|m| m.id == monitor_id)
        .ok_or(DisplayError::MonitorNotFound)?;

    if !monitor.available_refresh_rates.contains(&rate) {
        return Err(DisplayError::UnsupportedRefreshRate);
    }

    monitor.refresh_rate = rate;

    Ok(())
}

/// Set monitor rotation
pub fn set_rotation(monitor_id: u32, rotation: Rotation) -> Result<(), DisplayError> {
    let mut state = DISPLAY_SETTINGS.lock();
    let state = state.as_mut().ok_or(DisplayError::NotInitialized)?;

    let monitor = state.monitors.iter_mut()
        .find(|m| m.id == monitor_id)
        .ok_or(DisplayError::MonitorNotFound)?;

    monitor.rotation = rotation;

    Ok(())
}

/// Set monitor scale
pub fn set_scale(monitor_id: u32, scale: ScaleFactor) -> Result<(), DisplayError> {
    let mut state = DISPLAY_SETTINGS.lock();
    let state = state.as_mut().ok_or(DisplayError::NotInitialized)?;

    let monitor = state.monitors.iter_mut()
        .find(|m| m.id == monitor_id)
        .ok_or(DisplayError::MonitorNotFound)?;

    monitor.scale = scale;

    Ok(())
}

/// Set monitor position
pub fn set_position(monitor_id: u32, x: i32, y: i32) -> Result<(), DisplayError> {
    let mut state = DISPLAY_SETTINGS.lock();
    let state = state.as_mut().ok_or(DisplayError::NotInitialized)?;

    let monitor = state.monitors.iter_mut()
        .find(|m| m.id == monitor_id)
        .ok_or(DisplayError::MonitorNotFound)?;

    monitor.position = (x, y);

    Ok(())
}

/// Set primary monitor
pub fn set_primary(monitor_id: u32) -> Result<(), DisplayError> {
    let mut state = DISPLAY_SETTINGS.lock();
    let state = state.as_mut().ok_or(DisplayError::NotInitialized)?;

    let idx = state.monitors.iter()
        .position(|m| m.id == monitor_id)
        .ok_or(DisplayError::MonitorNotFound)?;

    state.primary_monitor = idx;

    Ok(())
}

/// Enable/disable monitor
pub fn set_enabled(monitor_id: u32, enabled: bool) -> Result<(), DisplayError> {
    let mut state = DISPLAY_SETTINGS.lock();
    let state = state.as_mut().ok_or(DisplayError::NotInitialized)?;

    let monitor = state.monitors.iter_mut()
        .find(|m| m.id == monitor_id)
        .ok_or(DisplayError::MonitorNotFound)?;

    monitor.enabled = enabled;

    Ok(())
}

/// Set night light enabled
pub fn set_night_light_enabled(enabled: bool) {
    let mut state = DISPLAY_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.night_light.enabled = enabled;
    }
}

/// Set night light temperature
pub fn set_night_light_temperature(temperature: u32) {
    let mut state = DISPLAY_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.night_light.temperature = temperature.clamp(2500, 6500);
    }
}

/// Set night light schedule
pub fn set_night_light_schedule(schedule: NightLightSchedule) {
    let mut state = DISPLAY_SETTINGS.lock();
    if let Some(ref mut s) = *state {
        s.night_light.schedule = schedule;
    }
}

/// Get night light settings
pub fn get_night_light() -> Option<NightLightSettings> {
    let state = DISPLAY_SETTINGS.lock();
    state.as_ref().map(|s| s.night_light.clone())
}

/// Display error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayError {
    NotInitialized,
    MonitorNotFound,
    UnsupportedResolution,
    UnsupportedRefreshRate,
    ConfigFailed,
}
