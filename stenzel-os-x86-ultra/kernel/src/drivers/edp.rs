//! Embedded DisplayPort (eDP) Driver
//!
//! Provides eDP-specific functionality for laptop panels:
//! - Panel power sequencing
//! - Backlight control integration
//! - Panel Self-Refresh (PSR)
//! - DPCD T3/T12 timing
//! - VBT (Video BIOS Table) parsing

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};
use spin::Mutex;

use super::displayport::{self, DpPort};

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static EDP_STATE: Mutex<Option<EdpState>> = Mutex::new(None);

/// eDP state
#[derive(Debug)]
pub struct EdpState {
    /// eDP panels
    pub panels: Vec<EdpPanel>,
    /// VBT data
    pub vbt: Option<VbtData>,
}

/// eDP panel
#[derive(Debug, Clone)]
pub struct EdpPanel {
    /// Panel ID (from connector)
    pub id: u32,
    /// DP port ID
    pub dp_port_id: u32,
    /// Panel info
    pub info: PanelInfo,
    /// Power state
    pub power_state: PanelPowerState,
    /// Power sequencing timings
    pub power_timing: PowerSequenceTiming,
    /// PSR state
    pub psr_state: PsrState,
    /// Backlight info
    pub backlight: BacklightInfo,
}

/// Panel information
#[derive(Debug, Clone, Default)]
pub struct PanelInfo {
    /// Panel width (mm)
    pub width_mm: u32,
    /// Panel height (mm)
    pub height_mm: u32,
    /// Native horizontal resolution
    pub native_width: u32,
    /// Native vertical resolution
    pub native_height: u32,
    /// Native refresh rate
    pub native_refresh: u32,
    /// Bits per color
    pub bpc: u8,
    /// Panel type
    pub panel_type: PanelType,
    /// Manufacturer
    pub manufacturer: String,
    /// Model
    pub model: String,
}

/// Panel type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PanelType {
    #[default]
    Unknown,
    Lcd,
    Oled,
    MiniLed,
}

/// Panel power state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelPowerState {
    Off,
    PoweringOn,
    On,
    PoweringOff,
    Standby,
}

/// Power sequence timing (in microseconds)
#[derive(Debug, Clone, Copy, Default)]
pub struct PowerSequenceTiming {
    /// T1: Panel VDD to HPD valid (T1_off to T2_on)
    pub t1_vdd_to_hpd: u32,
    /// T2: HPD to backlight on
    pub t2_hpd_to_bl: u32,
    /// T3: Backlight off to VDD off
    pub t3_bl_to_vdd: u32,
    /// T4: VDD off to VDD on (power cycle delay)
    pub t4_vdd_cycle: u32,
    /// T5: VDD on to aux valid
    pub t5_vdd_to_aux: u32,
    /// T6: Aux valid to HPD
    pub t6_aux_to_hpd: u32,
    /// T7: VDD valid (minimum)
    pub t7_vdd_valid: u32,
    /// T8: HPD low (minimum)
    pub t8_hpd_low: u32,
    /// T10: Backlight enable to video
    pub t10_bl_to_video: u32,
    /// T11: Video to backlight enable
    pub t11_video_to_bl: u32,
    /// T12: VDD to link training
    pub t12_vdd_to_link: u32,
}

/// PSR (Panel Self-Refresh) state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PsrState {
    Disabled,
    Enabled,
    Active,     // Panel is in self-refresh mode
    Exiting,    // Transitioning out of self-refresh
}

/// Backlight info
#[derive(Debug, Clone)]
pub struct BacklightInfo {
    /// Backlight type
    pub bl_type: BacklightType,
    /// Current brightness (0-100)
    pub brightness: u8,
    /// PWM frequency (Hz)
    pub pwm_freq: u32,
    /// Minimum brightness
    pub min_brightness: u8,
    /// DPCD backlight control supported
    pub dpcd_backlight: bool,
}

/// Backlight type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BacklightType {
    Pwm,
    DpAux,
    Hybrid,
}

/// VBT (Video BIOS Table) data
#[derive(Debug, Clone)]
pub struct VbtData {
    /// VBT version
    pub version: u16,
    /// Panel type index
    pub panel_type: u8,
    /// Power sequence timings
    pub power_timing: PowerSequenceTiming,
    /// Backlight info
    pub backlight_info: VbtBacklight,
    /// eDP link parameters
    pub link_params: VbtLinkParams,
}

/// VBT backlight configuration
#[derive(Debug, Clone, Copy, Default)]
pub struct VbtBacklight {
    /// PWM frequency (Hz)
    pub pwm_freq: u32,
    /// Minimum brightness (0-255)
    pub min_brightness: u8,
    /// Active low polarity
    pub active_low: bool,
    /// Controller type
    pub controller: BacklightController,
}

/// Backlight controller type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BacklightController {
    #[default]
    None,
    PchPwm,
    LptPwm,
    Dpcd,
}

/// VBT link parameters
#[derive(Debug, Clone, Copy, Default)]
pub struct VbtLinkParams {
    /// Max link rate
    pub max_link_rate: u8,
    /// Max lane count
    pub max_lane_count: u8,
    /// Preemphasis
    pub preemphasis: u8,
    /// Voltage swing
    pub vswing: u8,
}

/// Error type
#[derive(Debug, Clone, Copy)]
pub enum EdpError {
    NotInitialized,
    PanelNotFound,
    PowerSequenceFailed,
    LinkTrainingFailed,
    PsrNotSupported,
    BacklightFailed,
    VbtParseFailed,
}

/// Initialize eDP subsystem
pub fn init() -> Result<(), EdpError> {
    if INITIALIZED.load(Ordering::Acquire) {
        return Ok(());
    }

    let state = EdpState {
        panels: Vec::new(),
        vbt: None,
    };

    *EDP_STATE.lock() = Some(state);
    INITIALIZED.store(true, Ordering::Release);

    crate::kprintln!("edp: eDP subsystem initialized");
    Ok(())
}

/// Register eDP panel
pub fn register_panel(panel_id: u32, dp_port_id: u32) -> Result<(), EdpError> {
    let mut state = EDP_STATE.lock();
    let state = state.as_mut().ok_or(EdpError::NotInitialized)?;

    let panel = EdpPanel {
        id: panel_id,
        dp_port_id,
        info: PanelInfo::default(),
        power_state: PanelPowerState::Off,
        power_timing: get_default_power_timing(),
        psr_state: PsrState::Disabled,
        backlight: BacklightInfo {
            bl_type: BacklightType::Pwm,
            brightness: 100,
            pwm_freq: 200,
            min_brightness: 1,
            dpcd_backlight: false,
        },
    };

    state.panels.push(panel);
    Ok(())
}

/// Get default power sequence timing
fn get_default_power_timing() -> PowerSequenceTiming {
    PowerSequenceTiming {
        t1_vdd_to_hpd: 210_000,    // 210ms
        t2_hpd_to_bl: 50_000,      // 50ms
        t3_bl_to_vdd: 200_000,     // 200ms
        t4_vdd_cycle: 500_000,     // 500ms
        t5_vdd_to_aux: 100_000,    // 100ms
        t6_aux_to_hpd: 100_000,    // 100ms
        t7_vdd_valid: 200_000,     // 200ms
        t8_hpd_low: 2_000,         // 2ms
        t10_bl_to_video: 0,
        t11_video_to_bl: 0,
        t12_vdd_to_link: 500_000,  // 500ms
    }
}

/// Power on panel
pub fn power_on(panel_id: u32) -> Result<(), EdpError> {
    let mut state = EDP_STATE.lock();
    let state = state.as_mut().ok_or(EdpError::NotInitialized)?;

    let panel = state
        .panels
        .iter_mut()
        .find(|p| p.id == panel_id)
        .ok_or(EdpError::PanelNotFound)?;

    if panel.power_state == PanelPowerState::On {
        return Ok(());
    }

    panel.power_state = PanelPowerState::PoweringOn;

    // Power sequence:
    // 1. Enable VDD
    // 2. Wait T1 for HPD
    // 3. Link training
    // 4. Enable video
    // 5. Wait T2
    // 6. Enable backlight

    crate::kprintln!("edp: Panel {} powering on", panel_id);

    // Wait T1: VDD to HPD
    delay_us(panel.power_timing.t1_vdd_to_hpd);

    // Perform link training
    let dp_port_id = panel.dp_port_id;
    drop(state); // Release lock before calling displayport functions

    if let Ok(dpcd) = displayport::read_dpcd_caps(dp_port_id) {
        let result = displayport::link_train(
            dp_port_id,
            dpcd.max_link_rate,
            dpcd.max_lane_count.min(4),
        );

        if result.is_err() {
            return Err(EdpError::LinkTrainingFailed);
        }
    }

    // Re-acquire lock
    let mut state = EDP_STATE.lock();
    let state = state.as_mut().ok_or(EdpError::NotInitialized)?;
    let panel = state
        .panels
        .iter_mut()
        .find(|p| p.id == panel_id)
        .ok_or(EdpError::PanelNotFound)?;

    // Wait T2: HPD to backlight
    delay_us(panel.power_timing.t2_hpd_to_bl);

    // Enable backlight
    enable_backlight_internal(panel)?;

    panel.power_state = PanelPowerState::On;
    crate::kprintln!("edp: Panel {} powered on", panel_id);

    Ok(())
}

/// Power off panel
pub fn power_off(panel_id: u32) -> Result<(), EdpError> {
    let mut state = EDP_STATE.lock();
    let state = state.as_mut().ok_or(EdpError::NotInitialized)?;

    let panel = state
        .panels
        .iter_mut()
        .find(|p| p.id == panel_id)
        .ok_or(EdpError::PanelNotFound)?;

    if panel.power_state == PanelPowerState::Off {
        return Ok(());
    }

    panel.power_state = PanelPowerState::PoweringOff;

    // Power off sequence:
    // 1. Disable backlight
    // 2. Wait T3
    // 3. Disable video
    // 4. Disable VDD
    // 5. Wait T4 before next power on

    crate::kprintln!("edp: Panel {} powering off", panel_id);

    // Disable backlight
    disable_backlight_internal(panel)?;

    // Wait T3: backlight to VDD off
    delay_us(panel.power_timing.t3_bl_to_vdd);

    panel.power_state = PanelPowerState::Off;
    crate::kprintln!("edp: Panel {} powered off", panel_id);

    Ok(())
}

/// Enable backlight
fn enable_backlight_internal(panel: &mut EdpPanel) -> Result<(), EdpError> {
    // Set brightness to saved value
    match panel.backlight.bl_type {
        BacklightType::Pwm => {
            // Use PCH PWM controller
            // In a real implementation, this would write to PWM registers
        }
        BacklightType::DpAux => {
            // Use DPCD backlight control
            // Write to DPCD 0x720 (EDP_BACKLIGHT_MODE_SET)
        }
        BacklightType::Hybrid => {
            // Use both for different ranges
        }
    }
    Ok(())
}

/// Disable backlight
fn disable_backlight_internal(panel: &mut EdpPanel) -> Result<(), EdpError> {
    panel.backlight.brightness = 0;
    // Turn off backlight hardware
    Ok(())
}

/// Set backlight brightness
pub fn set_brightness(panel_id: u32, brightness: u8) -> Result<(), EdpError> {
    let mut state = EDP_STATE.lock();
    let state = state.as_mut().ok_or(EdpError::NotInitialized)?;

    let panel = state
        .panels
        .iter_mut()
        .find(|p| p.id == panel_id)
        .ok_or(EdpError::PanelNotFound)?;

    let brightness = brightness.max(panel.backlight.min_brightness).min(100);
    panel.backlight.brightness = brightness;

    // Apply brightness
    match panel.backlight.bl_type {
        BacklightType::Pwm => {
            // Calculate PWM duty cycle
            let duty = (brightness as u32 * 0xFFFF) / 100;
            let _ = duty; // Would write to PWM register
        }
        BacklightType::DpAux => {
            // Write to DPCD backlight brightness
            let dp_port_id = panel.dp_port_id;
            let level = (brightness as u16 * 0xFFFF) / 100;
            let data = [(level >> 8) as u8, level as u8];
            let _ = displayport::aux_write(dp_port_id, 0x722, &data); // EDP_BACKLIGHT_BRIGHTNESS
        }
        BacklightType::Hybrid => {
            // Combination approach
        }
    }

    Ok(())
}

/// Get backlight brightness
pub fn get_brightness(panel_id: u32) -> Result<u8, EdpError> {
    let state = EDP_STATE.lock();
    let state = state.as_ref().ok_or(EdpError::NotInitialized)?;

    let panel = state
        .panels
        .iter()
        .find(|p| p.id == panel_id)
        .ok_or(EdpError::PanelNotFound)?;

    Ok(panel.backlight.brightness)
}

/// Enable PSR (Panel Self-Refresh)
pub fn enable_psr(panel_id: u32) -> Result<(), EdpError> {
    let mut state = EDP_STATE.lock();
    let state = state.as_mut().ok_or(EdpError::NotInitialized)?;

    let panel = state
        .panels
        .iter_mut()
        .find(|p| p.id == panel_id)
        .ok_or(EdpError::PanelNotFound)?;

    let dp_port_id = panel.dp_port_id;

    // Check if PSR is supported
    if let Some(dp_port) = displayport::get_port(dp_port_id) {
        if !dp_port.dpcd.psr_capable {
            return Err(EdpError::PsrNotSupported);
        }
    }

    // Enable PSR via DP AUX
    let _ = displayport::enable_psr(dp_port_id);

    panel.psr_state = PsrState::Enabled;
    crate::kprintln!("edp: PSR enabled on panel {}", panel_id);

    Ok(())
}

/// Disable PSR
pub fn disable_psr(panel_id: u32) -> Result<(), EdpError> {
    let mut state = EDP_STATE.lock();
    let state = state.as_mut().ok_or(EdpError::NotInitialized)?;

    let panel = state
        .panels
        .iter_mut()
        .find(|p| p.id == panel_id)
        .ok_or(EdpError::PanelNotFound)?;

    let dp_port_id = panel.dp_port_id;
    let _ = displayport::disable_psr(dp_port_id);

    panel.psr_state = PsrState::Disabled;

    Ok(())
}

/// Parse VBT (Video BIOS Table)
pub fn parse_vbt(vbt_data: &[u8]) -> Result<VbtData, EdpError> {
    if vbt_data.len() < 32 {
        return Err(EdpError::VbtParseFailed);
    }

    // Check VBT signature "$VBT"
    if &vbt_data[0..4] != b"$VBT" {
        return Err(EdpError::VbtParseFailed);
    }

    let version = u16::from_le_bytes([vbt_data[4], vbt_data[5]]);

    // Parse BDB (BIOS Data Block) header
    // This is a simplified parser; real VBT parsing is complex

    let vbt = VbtData {
        version,
        panel_type: 0,
        power_timing: get_default_power_timing(),
        backlight_info: VbtBacklight::default(),
        link_params: VbtLinkParams::default(),
    };

    // Store VBT data
    if let Some(state) = EDP_STATE.lock().as_mut() {
        state.vbt = Some(vbt.clone());
    }

    Ok(vbt)
}

/// Apply VBT settings to panel
pub fn apply_vbt_settings(panel_id: u32) -> Result<(), EdpError> {
    let mut state = EDP_STATE.lock();
    let state = state.as_mut().ok_or(EdpError::NotInitialized)?;

    let vbt = state.vbt.clone().ok_or(EdpError::VbtParseFailed)?;

    let panel = state
        .panels
        .iter_mut()
        .find(|p| p.id == panel_id)
        .ok_or(EdpError::PanelNotFound)?;

    // Apply power timing from VBT
    panel.power_timing = vbt.power_timing;

    // Apply backlight settings
    panel.backlight.pwm_freq = vbt.backlight_info.pwm_freq;
    panel.backlight.min_brightness = vbt.backlight_info.min_brightness;

    Ok(())
}

/// Get panel info
pub fn get_panel(panel_id: u32) -> Option<EdpPanel> {
    EDP_STATE
        .lock()
        .as_ref()
        .and_then(|s| s.panels.iter().find(|p| p.id == panel_id).cloned())
}

/// Get all eDP panels
pub fn get_panels() -> Vec<EdpPanel> {
    EDP_STATE
        .lock()
        .as_ref()
        .map(|s| s.panels.clone())
        .unwrap_or_default()
}

/// Get panel power state
pub fn get_power_state(panel_id: u32) -> Result<PanelPowerState, EdpError> {
    let state = EDP_STATE.lock();
    let state = state.as_ref().ok_or(EdpError::NotInitialized)?;

    let panel = state
        .panels
        .iter()
        .find(|p| p.id == panel_id)
        .ok_or(EdpError::PanelNotFound)?;

    Ok(panel.power_state)
}

/// Simple microsecond delay (busy wait)
fn delay_us(us: u32) {
    // In a real implementation, this would use proper timer facilities
    // For now, just busy-wait using TSC
    let start = crate::time::uptime_ns();
    let target = start + (us as u64 * 1000);
    while crate::time::uptime_ns() < target {
        core::hint::spin_loop();
    }
}
