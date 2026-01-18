//! DisplayPort Output Driver
//!
//! Provides DisplayPort-specific functionality:
//! - DP link training
//! - Multi-Stream Transport (MST)
//! - DisplayPort audio
//! - DPCD access
//! - eDP Panel Self-Refresh (PSR)

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use spin::Mutex;

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static DP_STATE: Mutex<Option<DisplayPortState>> = Mutex::new(None);

/// DisplayPort state
#[derive(Debug)]
pub struct DisplayPortState {
    /// DP ports
    pub ports: Vec<DpPort>,
    /// MST enabled globally
    pub mst_enabled: bool,
}

/// DisplayPort port
#[derive(Debug, Clone)]
pub struct DpPort {
    /// Port ID
    pub id: u32,
    /// Connector ID (from DRM)
    pub connector_id: u32,
    /// AUX channel base address
    pub aux_base: u64,
    /// Connected
    pub connected: bool,
    /// Link configuration
    pub link_config: DpLinkConfig,
    /// DPCD (DisplayPort Configuration Data)
    pub dpcd: DpcdInfo,
    /// Audio enabled
    pub audio_enabled: bool,
    /// MST hub connected
    pub mst_hub: Option<MstHub>,
    /// eDP port
    pub is_edp: bool,
    /// PSR enabled (eDP only)
    pub psr_enabled: bool,
    /// EDID
    pub edid: Option<Vec<u8>>,
}

/// DP link configuration
#[derive(Debug, Clone, Copy, Default)]
pub struct DpLinkConfig {
    /// Link rate (in 10kHz units, e.g., 162000 = 1.62 GHz)
    pub link_rate: u32,
    /// Lane count (1, 2, 4)
    pub lane_count: u8,
    /// Enhanced framing
    pub enhanced_framing: bool,
    /// Spread spectrum clocking
    pub ssc_enabled: bool,
    /// Voltage swing level (0-3)
    pub voltage_swing: [u8; 4],
    /// Pre-emphasis level (0-3)
    pub pre_emphasis: [u8; 4],
}

/// DPCD information
#[derive(Debug, Clone, Default)]
pub struct DpcdInfo {
    /// DPCD revision (0x10 = 1.0, 0x11 = 1.1, etc.)
    pub revision: u8,
    /// Max link rate
    pub max_link_rate: u8,
    /// Max lane count
    pub max_lane_count: u8,
    /// TPS3 supported
    pub tps3_supported: bool,
    /// TPS4 supported
    pub tps4_supported: bool,
    /// eDP capable
    pub edp_capable: bool,
    /// MST capable
    pub mst_capable: bool,
    /// DSC capable
    pub dsc_capable: bool,
    /// PSR capable
    pub psr_capable: bool,
    /// PSR version
    pub psr_version: u8,
    /// Audio supported
    pub audio_supported: bool,
    /// HDCP 1.4 capable
    pub hdcp14_capable: bool,
    /// HDCP 2.2/2.3 capable
    pub hdcp22_capable: bool,
}

/// MST Hub
#[derive(Debug, Clone)]
pub struct MstHub {
    /// GUID
    pub guid: [u8; 16],
    /// Port count
    pub port_count: u8,
    /// Connected ports
    pub ports: Vec<MstPort>,
}

/// MST Port
#[derive(Debug, Clone)]
pub struct MstPort {
    /// Port number
    pub port_num: u8,
    /// Peer device type
    pub peer_type: MstPeerType,
    /// Is connected
    pub connected: bool,
    /// Available PBN (Payload Bandwidth Number)
    pub available_pbn: u16,
    /// EDID
    pub edid: Option<Vec<u8>>,
}

/// MST peer device type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MstPeerType {
    None,
    SourceOrSstBranch,
    MstBranchDevice,
    SstSinkDevice,
    DpToLegacyConverter,
}

/// Link training result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkTrainingResult {
    Success,
    ClockRecoveryFailed,
    ChannelEqualizationFailed,
    TimedOut,
    AuxFailed,
}

/// DP AUX transaction type
#[derive(Debug, Clone, Copy)]
pub enum AuxTransaction {
    NativeWrite,
    NativeRead,
    I2cWrite,
    I2cRead,
    I2cWriteStatusRequest,
}

/// DPCD register addresses
pub mod dpcd {
    pub const DPCD_REV: u32 = 0x00000;
    pub const MAX_LINK_RATE: u32 = 0x00001;
    pub const MAX_LANE_COUNT: u32 = 0x00002;
    pub const MAX_DOWNSPREAD: u32 = 0x00003;
    pub const NORP_DP_PWR_VOLTAGE_CAP: u32 = 0x00004;
    pub const DOWN_STREAM_PORT_COUNT: u32 = 0x00007;
    pub const RECEIVE_PORT0_CAP_0: u32 = 0x00008;
    pub const RECEIVE_PORT0_CAP_1: u32 = 0x00009;
    pub const MSTM_CAP: u32 = 0x00021;
    pub const EDP_CONFIGURATION_CAP: u32 = 0x0000D;
    pub const PSR_SUPPORT: u32 = 0x00070;
    pub const PSR_CAPS: u32 = 0x00071;
    pub const DSC_SUPPORT: u32 = 0x00060;

    pub const LINK_BW_SET: u32 = 0x00100;
    pub const LANE_COUNT_SET: u32 = 0x00101;
    pub const TRAINING_PATTERN_SET: u32 = 0x00102;
    pub const TRAINING_LANE0_SET: u32 = 0x00103;
    pub const DOWNSPREAD_CTRL: u32 = 0x00107;

    pub const SINK_COUNT: u32 = 0x00200;
    pub const DEVICE_SERVICE_IRQ_VECTOR: u32 = 0x00201;
    pub const LANE0_1_STATUS: u32 = 0x00202;
    pub const LANE2_3_STATUS: u32 = 0x00203;
    pub const LANE_ALIGN_STATUS_UPDATED: u32 = 0x00204;
    pub const SINK_STATUS: u32 = 0x00205;
    pub const ADJUST_REQUEST_LANE0_1: u32 = 0x00206;
    pub const ADJUST_REQUEST_LANE2_3: u32 = 0x00207;

    pub const PSR_EN_CFG: u32 = 0x00170;

    pub const MSTM_CTRL: u32 = 0x00111;
    pub const PAYLOAD_ALLOCATE_SET: u32 = 0x001C0;
    pub const PAYLOAD_ALLOCATE_START_TIME_SLOT: u32 = 0x001C1;
    pub const PAYLOAD_ALLOCATE_TIME_SLOT_COUNT: u32 = 0x001C2;
}

/// Link rates
pub mod link_rate {
    pub const RBR: u8 = 0x06;   // 1.62 Gbps
    pub const HBR: u8 = 0x0A;   // 2.7 Gbps
    pub const HBR2: u8 = 0x14;  // 5.4 Gbps
    pub const HBR3: u8 = 0x1E;  // 8.1 Gbps
}

/// Error type
#[derive(Debug, Clone, Copy)]
pub enum DpError {
    NotInitialized,
    PortNotFound,
    NotConnected,
    AuxFailed,
    LinkTrainingFailed,
    MstNotSupported,
    InvalidParameter,
    Timeout,
}

/// Initialize DisplayPort subsystem
pub fn init() -> Result<(), DpError> {
    if INITIALIZED.load(Ordering::Acquire) {
        return Ok(());
    }

    let state = DisplayPortState {
        ports: Vec::new(),
        mst_enabled: true,
    };

    *DP_STATE.lock() = Some(state);
    INITIALIZED.store(true, Ordering::Release);

    crate::kprintln!("displayport: DisplayPort subsystem initialized");
    Ok(())
}

/// Register DP port
pub fn register_port(port_id: u32, connector_id: u32, aux_base: u64, is_edp: bool) -> Result<(), DpError> {
    let mut state = DP_STATE.lock();
    let state = state.as_mut().ok_or(DpError::NotInitialized)?;

    let port = DpPort {
        id: port_id,
        connector_id,
        aux_base,
        connected: false,
        link_config: DpLinkConfig::default(),
        dpcd: DpcdInfo::default(),
        audio_enabled: false,
        mst_hub: None,
        is_edp,
        psr_enabled: false,
        edid: None,
    };

    state.ports.push(port);
    Ok(())
}

/// Read DPCD register
pub fn aux_read(port_id: u32, address: u32, data: &mut [u8]) -> Result<usize, DpError> {
    let state = DP_STATE.lock();
    let state = state.as_ref().ok_or(DpError::NotInitialized)?;

    let _port = state
        .ports
        .iter()
        .find(|p| p.id == port_id)
        .ok_or(DpError::PortNotFound)?;

    // In a real implementation, this would perform AUX channel transaction
    // For now, return mock data for common DPCD registers
    let bytes_read = data.len().min(16);

    match address {
        dpcd::DPCD_REV => {
            if bytes_read >= 1 {
                data[0] = 0x14; // DP 1.4
            }
        }
        dpcd::MAX_LINK_RATE => {
            if bytes_read >= 1 {
                data[0] = link_rate::HBR3; // 8.1 Gbps
            }
        }
        dpcd::MAX_LANE_COUNT => {
            if bytes_read >= 1 {
                data[0] = 0x84; // 4 lanes, enhanced framing
            }
        }
        _ => {
            for byte in data.iter_mut().take(bytes_read) {
                *byte = 0;
            }
        }
    }

    Ok(bytes_read)
}

/// Write DPCD register
pub fn aux_write(port_id: u32, address: u32, data: &[u8]) -> Result<usize, DpError> {
    let state = DP_STATE.lock();
    let state = state.as_ref().ok_or(DpError::NotInitialized)?;

    let _port = state
        .ports
        .iter()
        .find(|p| p.id == port_id)
        .ok_or(DpError::PortNotFound)?;

    // In a real implementation, this would perform AUX channel write
    let _ = address;
    Ok(data.len().min(16))
}

/// Read sink DPCD capabilities
pub fn read_dpcd_caps(port_id: u32) -> Result<DpcdInfo, DpError> {
    let mut data = [0u8; 16];

    // Read DPCD receiver capability (0x0000-0x000F)
    aux_read(port_id, dpcd::DPCD_REV, &mut data)?;

    let revision = data[0];
    let max_link_rate = data[1];
    let max_lane_count = data[2] & 0x1F;
    let enhanced_framing = (data[2] & 0x80) != 0;

    // Read MST capability
    let mut mst_data = [0u8; 1];
    aux_read(port_id, dpcd::MSTM_CAP, &mut mst_data)?;
    let mst_capable = (mst_data[0] & 0x01) != 0;

    // Read PSR capability
    let mut psr_data = [0u8; 2];
    aux_read(port_id, dpcd::PSR_SUPPORT, &mut psr_data)?;
    let psr_capable = (psr_data[0] & 0x01) != 0;
    let psr_version = psr_data[0];

    // Read DSC capability
    let mut dsc_data = [0u8; 1];
    aux_read(port_id, dpcd::DSC_SUPPORT, &mut dsc_data)?;
    let dsc_capable = (dsc_data[0] & 0x01) != 0;

    let dpcd = DpcdInfo {
        revision,
        max_link_rate,
        max_lane_count,
        tps3_supported: revision >= 0x12,
        tps4_supported: revision >= 0x14,
        edp_capable: revision >= 0x13,
        mst_capable,
        dsc_capable,
        psr_capable,
        psr_version,
        audio_supported: true,
        hdcp14_capable: true,
        hdcp22_capable: revision >= 0x14,
    };

    // Update port DPCD info
    let mut state = DP_STATE.lock();
    if let Some(state) = state.as_mut() {
        if let Some(port) = state.ports.iter_mut().find(|p| p.id == port_id) {
            port.dpcd = dpcd.clone();
        }
    }

    Ok(dpcd)
}

/// Perform link training
pub fn link_train(port_id: u32, target_rate: u8, target_lanes: u8) -> Result<LinkTrainingResult, DpError> {
    let mut state = DP_STATE.lock();
    let state = state.as_mut().ok_or(DpError::NotInitialized)?;

    let port = state
        .ports
        .iter_mut()
        .find(|p| p.id == port_id)
        .ok_or(DpError::PortNotFound)?;

    // Validate parameters against DPCD caps
    if target_rate > port.dpcd.max_link_rate {
        return Err(DpError::InvalidParameter);
    }
    if target_lanes > port.dpcd.max_lane_count {
        return Err(DpError::InvalidParameter);
    }

    crate::kprintln!(
        "displayport: Link training port {} at rate 0x{:02X}, {} lanes",
        port_id,
        target_rate,
        target_lanes
    );

    // Phase 1: Clock Recovery
    // Set link rate and lane count
    aux_write(port_id, dpcd::LINK_BW_SET, &[target_rate])?;
    aux_write(port_id, dpcd::LANE_COUNT_SET, &[target_lanes | 0x80])?; // Enhanced framing

    // Set training pattern 1
    aux_write(port_id, dpcd::TRAINING_PATTERN_SET, &[0x21])?; // TPS1 + scrambling disabled

    // Adjust voltage swing and pre-emphasis
    let mut vs_pe = [0u8; 4];
    for i in 0..target_lanes as usize {
        vs_pe[i] = port.link_config.voltage_swing[i] | (port.link_config.pre_emphasis[i] << 3);
    }
    aux_write(port_id, dpcd::TRAINING_LANE0_SET, &vs_pe[..target_lanes as usize])?;

    // Read lane status
    let mut status = [0u8; 2];
    aux_read(port_id, dpcd::LANE0_1_STATUS, &mut status)?;

    // Check clock recovery
    let cr_done = check_clock_recovery(&status, target_lanes);
    if !cr_done {
        aux_write(port_id, dpcd::TRAINING_PATTERN_SET, &[0x00])?;
        return Ok(LinkTrainingResult::ClockRecoveryFailed);
    }

    // Phase 2: Channel Equalization
    // Set training pattern 2, 3, or 4
    let tps = if port.dpcd.tps4_supported {
        0x27 // TPS4
    } else if port.dpcd.tps3_supported {
        0x23 // TPS3
    } else {
        0x22 // TPS2
    };
    aux_write(port_id, dpcd::TRAINING_PATTERN_SET, &[tps])?;

    // Read lane status again
    aux_read(port_id, dpcd::LANE0_1_STATUS, &mut status)?;

    // Check channel equalization
    let eq_done = check_channel_eq(&status, target_lanes);
    if !eq_done {
        aux_write(port_id, dpcd::TRAINING_PATTERN_SET, &[0x00])?;
        return Ok(LinkTrainingResult::ChannelEqualizationFailed);
    }

    // End training
    aux_write(port_id, dpcd::TRAINING_PATTERN_SET, &[0x00])?;

    // Update port config
    port.link_config.link_rate = link_rate_to_khz(target_rate);
    port.link_config.lane_count = target_lanes;
    port.link_config.enhanced_framing = true;
    port.connected = true;

    crate::kprintln!(
        "displayport: Link training successful: {} Gbps x {} lanes",
        port.link_config.link_rate / 100000,
        target_lanes
    );

    Ok(LinkTrainingResult::Success)
}

/// Check clock recovery status
fn check_clock_recovery(status: &[u8], lanes: u8) -> bool {
    for i in 0..lanes {
        let byte_idx = (i / 2) as usize;
        let nibble_shift = (i % 2) * 4;
        let lane_status = (status[byte_idx] >> nibble_shift) & 0x0F;
        if (lane_status & 0x01) == 0 {
            return false;
        }
    }
    true
}

/// Check channel equalization status
fn check_channel_eq(status: &[u8], lanes: u8) -> bool {
    for i in 0..lanes {
        let byte_idx = (i / 2) as usize;
        let nibble_shift = (i % 2) * 4;
        let lane_status = (status[byte_idx] >> nibble_shift) & 0x0F;
        if (lane_status & 0x07) != 0x07 {
            return false;
        }
    }
    true
}

/// Convert link rate code to kHz
fn link_rate_to_khz(rate: u8) -> u32 {
    match rate {
        link_rate::RBR => 162000,
        link_rate::HBR => 270000,
        link_rate::HBR2 => 540000,
        link_rate::HBR3 => 810000,
        _ => rate as u32 * 27000,
    }
}

/// Calculate link bandwidth
pub fn calculate_bandwidth(port_id: u32) -> Result<u64, DpError> {
    let state = DP_STATE.lock();
    let state = state.as_ref().ok_or(DpError::NotInitialized)?;

    let port = state
        .ports
        .iter()
        .find(|p| p.id == port_id)
        .ok_or(DpError::PortNotFound)?;

    // Bandwidth = link_rate * lane_count * 8/10 (8b/10b encoding)
    // For HBR3 with 4 lanes: 8.1 * 4 * 0.8 = 25.92 Gbps
    let bw = port.link_config.link_rate as u64
        * port.link_config.lane_count as u64
        * 8
        / 10;

    Ok(bw)
}

/// Enable MST mode
pub fn enable_mst(port_id: u32) -> Result<(), DpError> {
    let mut state = DP_STATE.lock();
    let state = state.as_mut().ok_or(DpError::NotInitialized)?;

    let port = state
        .ports
        .iter_mut()
        .find(|p| p.id == port_id)
        .ok_or(DpError::PortNotFound)?;

    if !port.dpcd.mst_capable {
        return Err(DpError::MstNotSupported);
    }

    // Enable MST mode in DPCD
    aux_write(port_id, dpcd::MSTM_CTRL, &[0x01])?;

    crate::kprintln!("displayport: MST enabled on port {}", port_id);
    Ok(())
}

/// Enable Panel Self-Refresh (eDP only)
pub fn enable_psr(port_id: u32) -> Result<(), DpError> {
    let mut state = DP_STATE.lock();
    let state = state.as_mut().ok_or(DpError::NotInitialized)?;

    let port = state
        .ports
        .iter_mut()
        .find(|p| p.id == port_id)
        .ok_or(DpError::PortNotFound)?;

    if !port.is_edp || !port.dpcd.psr_capable {
        return Err(DpError::InvalidParameter);
    }

    // Enable PSR in DPCD
    aux_write(port_id, dpcd::PSR_EN_CFG, &[0x01])?;
    port.psr_enabled = true;

    crate::kprintln!("displayport: PSR enabled on eDP port {}", port_id);
    Ok(())
}

/// Disable PSR
pub fn disable_psr(port_id: u32) -> Result<(), DpError> {
    let mut state = DP_STATE.lock();
    let state = state.as_mut().ok_or(DpError::NotInitialized)?;

    let port = state
        .ports
        .iter_mut()
        .find(|p| p.id == port_id)
        .ok_or(DpError::PortNotFound)?;

    aux_write(port_id, dpcd::PSR_EN_CFG, &[0x00])?;
    port.psr_enabled = false;

    Ok(())
}

/// Enable DP audio
pub fn enable_audio(port_id: u32) -> Result<(), DpError> {
    let mut state = DP_STATE.lock();
    let state = state.as_mut().ok_or(DpError::NotInitialized)?;

    let port = state
        .ports
        .iter_mut()
        .find(|p| p.id == port_id)
        .ok_or(DpError::PortNotFound)?;

    if !port.dpcd.audio_supported {
        return Err(DpError::InvalidParameter);
    }

    port.audio_enabled = true;
    crate::kprintln!("displayport: Audio enabled on port {}", port_id);

    Ok(())
}

/// Disable DP audio
pub fn disable_audio(port_id: u32) -> Result<(), DpError> {
    let mut state = DP_STATE.lock();
    let state = state.as_mut().ok_or(DpError::NotInitialized)?;

    let port = state
        .ports
        .iter_mut()
        .find(|p| p.id == port_id)
        .ok_or(DpError::PortNotFound)?;

    port.audio_enabled = false;
    Ok(())
}

/// Get port info
pub fn get_port(port_id: u32) -> Option<DpPort> {
    DP_STATE
        .lock()
        .as_ref()
        .and_then(|s| s.ports.iter().find(|p| p.id == port_id).cloned())
}

/// Get all DP ports
pub fn get_ports() -> Vec<DpPort> {
    DP_STATE
        .lock()
        .as_ref()
        .map(|s| s.ports.clone())
        .unwrap_or_default()
}

/// Get connected eDP ports
pub fn get_edp_ports() -> Vec<DpPort> {
    DP_STATE
        .lock()
        .as_ref()
        .map(|s| s.ports.iter().filter(|p| p.is_edp).cloned().collect())
        .unwrap_or_default()
}
