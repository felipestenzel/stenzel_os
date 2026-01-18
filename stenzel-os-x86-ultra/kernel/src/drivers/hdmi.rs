//! HDMI Output Driver
//!
//! Provides HDMI-specific functionality:
//! - HDMI link training
//! - HDMI audio output
//! - HDCP (optional)
//! - CEC support
//! - InfoFrames (AVI, Audio, Vendor)

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use spin::Mutex;

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static HDMI_STATE: Mutex<Option<HdmiState>> = Mutex::new(None);

/// HDMI state
#[derive(Debug)]
pub struct HdmiState {
    /// HDMI ports
    pub ports: Vec<HdmiPort>,
    /// HDCP enabled globally
    pub hdcp_enabled: bool,
    /// CEC enabled globally
    pub cec_enabled: bool,
}

/// HDMI port
#[derive(Debug, Clone)]
pub struct HdmiPort {
    /// Port ID
    pub id: u32,
    /// Connector ID (from DRM)
    pub connector_id: u32,
    /// MMIO base
    pub mmio_base: u64,
    /// Connected
    pub connected: bool,
    /// Current TMDS clock (kHz)
    pub tmds_clock: u32,
    /// Color depth (8, 10, 12 bits)
    pub color_depth: u8,
    /// Color format
    pub color_format: HdmiColorFormat,
    /// Audio enabled
    pub audio_enabled: bool,
    /// Audio format
    pub audio_format: HdmiAudioFormat,
    /// HDCP state
    pub hdcp_state: HdcpState,
    /// EDID
    pub edid: Option<Vec<u8>>,
    /// Sink capabilities
    pub sink_caps: HdmiSinkCaps,
}

/// HDMI color format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HdmiColorFormat {
    Rgb,
    Ycbcr444,
    Ycbcr422,
    Ycbcr420,
}

/// HDMI audio format
#[derive(Debug, Clone, Copy)]
pub struct HdmiAudioFormat {
    /// Sample rate (Hz)
    pub sample_rate: u32,
    /// Bits per sample
    pub bits: u8,
    /// Channel count
    pub channels: u8,
    /// Codec type
    pub codec: AudioCodec,
}

/// Audio codec
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioCodec {
    Pcm,
    Ac3,
    Dts,
    DtsHd,
    TrueHd,
    Eac3,
}

/// HDCP state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HdcpState {
    Disabled,
    Authenticating,
    Authenticated,
    Failed,
}

/// HDMI sink capabilities
#[derive(Debug, Clone, Default)]
pub struct HdmiSinkCaps {
    /// Maximum TMDS clock (kHz)
    pub max_tmds_clock: u32,
    /// Supports YCbCr 4:4:4
    pub ycbcr444: bool,
    /// Supports YCbCr 4:2:2
    pub ycbcr422: bool,
    /// Supports YCbCr 4:2:0
    pub ycbcr420: bool,
    /// Supports 10-bit color
    pub deep_color_10: bool,
    /// Supports 12-bit color
    pub deep_color_12: bool,
    /// Supports audio
    pub audio: bool,
    /// Supported audio sample rates
    pub audio_sample_rates: Vec<u32>,
    /// HDCP 1.4 support
    pub hdcp14: bool,
    /// HDCP 2.2/2.3 support
    pub hdcp22: bool,
    /// CEC support
    pub cec: bool,
    /// VRR support
    pub vrr: bool,
    /// ALLM (Auto Low Latency Mode)
    pub allm: bool,
}

/// InfoFrame types
#[derive(Debug, Clone, Copy)]
pub enum InfoFrameType {
    Avi = 0x82,
    Audio = 0x84,
    Spd = 0x83,
    VendorSpecific = 0x81,
    Drm = 0x87,
}

/// AVI InfoFrame
#[derive(Debug, Clone)]
pub struct AviInfoFrame {
    /// Color format
    pub color_format: HdmiColorFormat,
    /// Active format info valid
    pub afi_valid: bool,
    /// Bar info
    pub bar_info: u8,
    /// Scan info
    pub scan_info: u8,
    /// Colorimetry
    pub colorimetry: u8,
    /// Picture aspect ratio
    pub aspect_ratio: AspectRatio,
    /// Active format aspect ratio
    pub afa_ratio: u8,
    /// VIC (Video Identification Code)
    pub vic: u8,
    /// Pixel repetition
    pub pixel_rep: u8,
    /// Content type
    pub content_type: u8,
    /// YCC quantization
    pub ycc_quant: u8,
    /// RGB quantization
    pub rgb_quant: u8,
}

/// Aspect ratio
#[derive(Debug, Clone, Copy)]
pub enum AspectRatio {
    NoData,
    Ratio4x3,
    Ratio16x9,
    Ratio64x27,
    Ratio256x135,
}

/// Audio InfoFrame
#[derive(Debug, Clone)]
pub struct AudioInfoFrame {
    /// Channel count (1-8)
    pub channel_count: u8,
    /// Coding type
    pub coding_type: u8,
    /// Sample size
    pub sample_size: u8,
    /// Sample frequency
    pub sample_freq: u8,
    /// Channel allocation
    pub channel_allocation: u8,
    /// Level shift
    pub level_shift: u8,
    /// Down-mix inhibit
    pub down_mix_inhibit: bool,
}

/// CEC message
#[derive(Debug, Clone)]
pub struct CecMessage {
    /// Source address
    pub src: u8,
    /// Destination address
    pub dst: u8,
    /// Opcode
    pub opcode: CecOpcode,
    /// Parameters
    pub params: Vec<u8>,
}

/// CEC opcodes
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum CecOpcode {
    FeatureAbort = 0x00,
    ImageViewOn = 0x04,
    Standby = 0x36,
    SetStreamPath = 0x86,
    GivePhysicalAddress = 0x83,
    ReportPhysicalAddress = 0x84,
    GivePowerStatus = 0x8F,
    ReportPowerStatus = 0x90,
    ActiveSource = 0x82,
    RequestActiveSource = 0x85,
    RoutingChange = 0x80,
    RoutingInformation = 0x81,
    SetOsdName = 0x47,
    GiveOsdName = 0x46,
    MenuRequest = 0x8D,
    MenuStatus = 0x8E,
    UserControlPressed = 0x44,
    UserControlReleased = 0x45,
    GiveDeviceVendorId = 0x8C,
    DeviceVendorId = 0x87,
    Abort = 0xFF,
}

/// Error type
#[derive(Debug, Clone, Copy)]
pub enum HdmiError {
    NotInitialized,
    PortNotFound,
    NotConnected,
    LinkTrainingFailed,
    HdcpFailed,
    AudioConfigFailed,
    CecFailed,
    InvalidParameter,
}

/// Initialize HDMI subsystem
pub fn init() -> Result<(), HdmiError> {
    if INITIALIZED.load(Ordering::Acquire) {
        return Ok(());
    }

    let state = HdmiState {
        ports: Vec::new(),
        hdcp_enabled: false,
        cec_enabled: true,
    };

    *HDMI_STATE.lock() = Some(state);
    INITIALIZED.store(true, Ordering::Release);

    crate::kprintln!("hdmi: HDMI subsystem initialized");
    Ok(())
}

/// Register HDMI port
pub fn register_port(port_id: u32, connector_id: u32, mmio_base: u64) -> Result<(), HdmiError> {
    let mut state = HDMI_STATE.lock();
    let state = state.as_mut().ok_or(HdmiError::NotInitialized)?;

    let port = HdmiPort {
        id: port_id,
        connector_id,
        mmio_base,
        connected: false,
        tmds_clock: 0,
        color_depth: 8,
        color_format: HdmiColorFormat::Rgb,
        audio_enabled: false,
        audio_format: HdmiAudioFormat {
            sample_rate: 48000,
            bits: 16,
            channels: 2,
            codec: AudioCodec::Pcm,
        },
        hdcp_state: HdcpState::Disabled,
        edid: None,
        sink_caps: HdmiSinkCaps::default(),
    };

    state.ports.push(port);
    Ok(())
}

/// Enable HDMI output
pub fn enable_output(port_id: u32, mode: &super::display_pipe::DisplayMode) -> Result<(), HdmiError> {
    let mut state = HDMI_STATE.lock();
    let state = state.as_mut().ok_or(HdmiError::NotInitialized)?;

    let port = state
        .ports
        .iter_mut()
        .find(|p| p.id == port_id)
        .ok_or(HdmiError::PortNotFound)?;

    // Calculate TMDS clock
    // TMDS clock = pixel clock * (color depth / 8)
    let tmds_clock = mode.clock * port.color_depth as u32 / 8;

    // Verify sink supports this clock
    if port.sink_caps.max_tmds_clock > 0 && tmds_clock > port.sink_caps.max_tmds_clock {
        return Err(HdmiError::InvalidParameter);
    }

    port.tmds_clock = tmds_clock;
    port.connected = true;

    // Configure AVI InfoFrame
    let avi = create_avi_infoframe(mode, port.color_format);
    send_infoframe(port_id, InfoFrameType::Avi, &encode_avi_infoframe(&avi))?;

    crate::kprintln!(
        "hdmi: Port {} enabled at {}x{}@{}Hz, TMDS={}kHz",
        port_id,
        mode.hdisplay,
        mode.vdisplay,
        mode.vrefresh,
        tmds_clock
    );

    Ok(())
}

/// Disable HDMI output
pub fn disable_output(port_id: u32) -> Result<(), HdmiError> {
    let mut state = HDMI_STATE.lock();
    let state = state.as_mut().ok_or(HdmiError::NotInitialized)?;

    let port = state
        .ports
        .iter_mut()
        .find(|p| p.id == port_id)
        .ok_or(HdmiError::PortNotFound)?;

    port.connected = false;
    port.tmds_clock = 0;
    port.audio_enabled = false;

    Ok(())
}

/// Enable HDMI audio
pub fn enable_audio(port_id: u32, format: HdmiAudioFormat) -> Result<(), HdmiError> {
    let mut state = HDMI_STATE.lock();
    let state = state.as_mut().ok_or(HdmiError::NotInitialized)?;

    let port = state
        .ports
        .iter_mut()
        .find(|p| p.id == port_id)
        .ok_or(HdmiError::PortNotFound)?;

    if !port.connected {
        return Err(HdmiError::NotConnected);
    }

    if !port.sink_caps.audio {
        return Err(HdmiError::AudioConfigFailed);
    }

    port.audio_format = format;
    port.audio_enabled = true;

    // Send Audio InfoFrame
    let audio_if = AudioInfoFrame {
        channel_count: format.channels,
        coding_type: match format.codec {
            AudioCodec::Pcm => 1,
            AudioCodec::Ac3 => 2,
            AudioCodec::Dts => 7,
            _ => 0,
        },
        sample_size: match format.bits {
            16 => 1,
            20 => 2,
            24 => 3,
            _ => 0,
        },
        sample_freq: match format.sample_rate {
            32000 => 1,
            44100 => 2,
            48000 => 3,
            88200 => 4,
            96000 => 5,
            176400 => 6,
            192000 => 7,
            _ => 0,
        },
        channel_allocation: 0,
        level_shift: 0,
        down_mix_inhibit: false,
    };

    send_infoframe(port_id, InfoFrameType::Audio, &encode_audio_infoframe(&audio_if))?;

    crate::kprintln!(
        "hdmi: Audio enabled on port {}: {}Hz {}ch",
        port_id,
        format.sample_rate,
        format.channels
    );

    Ok(())
}

/// Disable HDMI audio
pub fn disable_audio(port_id: u32) -> Result<(), HdmiError> {
    let mut state = HDMI_STATE.lock();
    let state = state.as_mut().ok_or(HdmiError::NotInitialized)?;

    let port = state
        .ports
        .iter_mut()
        .find(|p| p.id == port_id)
        .ok_or(HdmiError::PortNotFound)?;

    port.audio_enabled = false;
    Ok(())
}

/// Set color format
pub fn set_color_format(port_id: u32, format: HdmiColorFormat, depth: u8) -> Result<(), HdmiError> {
    let mut state = HDMI_STATE.lock();
    let state = state.as_mut().ok_or(HdmiError::NotInitialized)?;

    let port = state
        .ports
        .iter_mut()
        .find(|p| p.id == port_id)
        .ok_or(HdmiError::PortNotFound)?;

    // Validate format support
    match format {
        HdmiColorFormat::Ycbcr444 if !port.sink_caps.ycbcr444 => {
            return Err(HdmiError::InvalidParameter);
        }
        HdmiColorFormat::Ycbcr422 if !port.sink_caps.ycbcr422 => {
            return Err(HdmiError::InvalidParameter);
        }
        HdmiColorFormat::Ycbcr420 if !port.sink_caps.ycbcr420 => {
            return Err(HdmiError::InvalidParameter);
        }
        _ => {}
    }

    // Validate depth support
    match depth {
        10 if !port.sink_caps.deep_color_10 => {
            return Err(HdmiError::InvalidParameter);
        }
        12 if !port.sink_caps.deep_color_12 => {
            return Err(HdmiError::InvalidParameter);
        }
        8 | 10 | 12 => {}
        _ => return Err(HdmiError::InvalidParameter),
    }

    port.color_format = format;
    port.color_depth = depth;

    Ok(())
}

/// Start HDCP authentication
pub fn start_hdcp(port_id: u32) -> Result<(), HdmiError> {
    let mut state = HDMI_STATE.lock();
    let state = state.as_mut().ok_or(HdmiError::NotInitialized)?;

    if !state.hdcp_enabled {
        return Err(HdmiError::HdcpFailed);
    }

    let port = state
        .ports
        .iter_mut()
        .find(|p| p.id == port_id)
        .ok_or(HdmiError::PortNotFound)?;

    if !port.sink_caps.hdcp14 && !port.sink_caps.hdcp22 {
        return Err(HdmiError::HdcpFailed);
    }

    port.hdcp_state = HdcpState::Authenticating;

    // In a real implementation, this would perform HDCP handshake
    // For now, simulate success
    port.hdcp_state = HdcpState::Authenticated;

    Ok(())
}

/// Send CEC message
pub fn cec_send(port_id: u32, msg: &CecMessage) -> Result<(), HdmiError> {
    let state = HDMI_STATE.lock();
    let state = state.as_ref().ok_or(HdmiError::NotInitialized)?;

    if !state.cec_enabled {
        return Err(HdmiError::CecFailed);
    }

    let _port = state
        .ports
        .iter()
        .find(|p| p.id == port_id)
        .ok_or(HdmiError::PortNotFound)?;

    // In a real implementation, this would send CEC message via hardware
    crate::kprintln!(
        "hdmi: CEC send on port {}: {:?} -> {}",
        port_id,
        msg.opcode,
        msg.dst
    );

    Ok(())
}

/// Turn on connected TV via CEC
pub fn cec_power_on(port_id: u32) -> Result<(), HdmiError> {
    let msg = CecMessage {
        src: 4, // Playback device
        dst: 0, // TV
        opcode: CecOpcode::ImageViewOn,
        params: Vec::new(),
    };
    cec_send(port_id, &msg)
}

/// Put connected TV in standby via CEC
pub fn cec_standby(port_id: u32) -> Result<(), HdmiError> {
    let msg = CecMessage {
        src: 4,
        dst: 0,
        opcode: CecOpcode::Standby,
        params: Vec::new(),
    };
    cec_send(port_id, &msg)
}

/// Create AVI InfoFrame from mode
fn create_avi_infoframe(mode: &super::display_pipe::DisplayMode, color_format: HdmiColorFormat) -> AviInfoFrame {
    AviInfoFrame {
        color_format,
        afi_valid: true,
        bar_info: 0,
        scan_info: 0,
        colorimetry: if mode.hdisplay >= 1920 { 2 } else { 1 }, // BT.709 or BT.601
        aspect_ratio: if mode.hdisplay * 9 == mode.vdisplay * 16 {
            AspectRatio::Ratio16x9
        } else {
            AspectRatio::Ratio4x3
        },
        afa_ratio: 8, // Same as picture
        vic: mode_to_vic(mode),
        pixel_rep: 0,
        content_type: 0,
        ycc_quant: 0,
        rgb_quant: 0,
    }
}

/// Convert mode to VIC (Video Identification Code)
fn mode_to_vic(mode: &super::display_pipe::DisplayMode) -> u8 {
    match (mode.hdisplay, mode.vdisplay, mode.vrefresh) {
        (640, 480, 60) => 1,
        (720, 480, 60) => 3,
        (1280, 720, 60) => 4,
        (1920, 1080, 60) => 16,
        (1920, 1080, 50) => 31,
        (3840, 2160, 30) => 95,
        (3840, 2160, 60) => 97,
        (4096, 2160, 60) => 102,
        _ => 0,
    }
}

/// Encode AVI InfoFrame
fn encode_avi_infoframe(avi: &AviInfoFrame) -> Vec<u8> {
    let mut data = vec![0u8; 13];

    // Byte 1: Y1Y0 A0 B1B0 S1S0
    data[0] = ((avi.color_format as u8) << 5)
        | ((avi.afi_valid as u8) << 4)
        | ((avi.bar_info & 0x3) << 2)
        | (avi.scan_info & 0x3);

    // Byte 2: C1C0 M1M0 R3R2R1R0
    data[1] = ((avi.colorimetry & 0x3) << 6)
        | ((avi.aspect_ratio as u8 & 0x3) << 4)
        | (avi.afa_ratio & 0xF);

    // Byte 3: ITC EC2EC1EC0 Q1Q0 SC1SC0
    data[2] = (avi.ycc_quant << 2) | (avi.rgb_quant & 0x3);

    // Byte 4: VIC
    data[3] = avi.vic;

    // Byte 5: YQ1YQ0 CN1CN0 PR3PR2PR1PR0
    data[4] = (avi.content_type << 4) | (avi.pixel_rep & 0xF);

    data
}

/// Encode Audio InfoFrame
fn encode_audio_infoframe(audio: &AudioInfoFrame) -> Vec<u8> {
    let mut data = vec![0u8; 10];

    // Byte 1: CT3CT2CT1CT0 CC2CC1CC0
    data[0] = ((audio.coding_type & 0xF) << 4) | ((audio.channel_count - 1) & 0x7);

    // Byte 2: SF2SF1SF0 SS1SS0
    data[1] = ((audio.sample_freq & 0x7) << 2) | (audio.sample_size & 0x3);

    // Byte 4: CA7-CA0
    data[3] = audio.channel_allocation;

    // Byte 5: DM_INH LSV3-LSV0
    data[4] = ((audio.down_mix_inhibit as u8) << 7) | ((audio.level_shift & 0xF) << 3);

    data
}

/// Send InfoFrame
fn send_infoframe(_port_id: u32, _frame_type: InfoFrameType, _data: &[u8]) -> Result<(), HdmiError> {
    // In a real implementation, this would write to HDMI controller registers
    Ok(())
}

/// Get HDMI port info
pub fn get_port(port_id: u32) -> Option<HdmiPort> {
    HDMI_STATE
        .lock()
        .as_ref()
        .and_then(|s| s.ports.iter().find(|p| p.id == port_id).cloned())
}

/// Get all HDMI ports
pub fn get_ports() -> Vec<HdmiPort> {
    HDMI_STATE
        .lock()
        .as_ref()
        .map(|s| s.ports.clone())
        .unwrap_or_default()
}

/// Parse EDID and populate sink capabilities
pub fn parse_sink_caps(port_id: u32, edid: &[u8]) -> Result<(), HdmiError> {
    let mut state = HDMI_STATE.lock();
    let state = state.as_mut().ok_or(HdmiError::NotInitialized)?;

    let port = state
        .ports
        .iter_mut()
        .find(|p| p.id == port_id)
        .ok_or(HdmiError::PortNotFound)?;

    port.edid = Some(edid.to_vec());

    // Parse basic EDID (128 bytes)
    if edid.len() >= 128 {
        // Parse extension blocks for HDMI capabilities
        let extensions = edid.get(126).copied().unwrap_or(0);
        if extensions > 0 && edid.len() >= 256 {
            parse_cea_extension(port, &edid[128..256]);
        }
    }

    Ok(())
}

/// Parse CEA-861 extension block
fn parse_cea_extension(port: &mut HdmiPort, cea: &[u8]) {
    if cea.len() < 4 || cea[0] != 0x02 {
        return;
    }

    let dtd_start = cea[2] as usize;
    if dtd_start < 4 {
        return;
    }

    // Parse data blocks
    let mut offset = 4;
    while offset < dtd_start && offset < cea.len() {
        let header = cea[offset];
        let tag = (header >> 5) & 0x7;
        let len = (header & 0x1F) as usize;

        if offset + 1 + len > cea.len() {
            break;
        }

        match tag {
            1 => {
                // Audio Data Block
                port.sink_caps.audio = true;
            }
            3 => {
                // Vendor Specific Data Block
                if len >= 3 {
                    let oui = (cea[offset + 3] as u32)
                        | ((cea[offset + 2] as u32) << 8)
                        | ((cea[offset + 1] as u32) << 16);
                    if oui == 0x000C03 {
                        // HDMI Licensing LLC
                        parse_hdmi_vsdb(port, &cea[offset + 1..offset + 1 + len]);
                    }
                }
            }
            7 => {
                // Extended tag
                if len >= 1 {
                    let ext_tag = cea[offset + 1];
                    match ext_tag {
                        0 => {
                            // Video Capability Data Block
                        }
                        5 => {
                            // Colorimetry Data Block
                        }
                        6 => {
                            // HDR Static Metadata Data Block
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }

        offset += 1 + len;
    }
}

/// Parse HDMI Vendor Specific Data Block
fn parse_hdmi_vsdb(port: &mut HdmiPort, data: &[u8]) {
    if data.len() < 5 {
        return;
    }

    // Physical address is at bytes 3-4
    // Deep color info at byte 5
    if data.len() >= 6 {
        let dc_flags = data[5];
        port.sink_caps.deep_color_10 = (dc_flags & 0x10) != 0;
        port.sink_caps.deep_color_12 = (dc_flags & 0x20) != 0;
        port.sink_caps.ycbcr444 = (dc_flags & 0x08) != 0;
    }

    // Max TMDS clock at byte 6 (if present)
    if data.len() >= 7 {
        port.sink_caps.max_tmds_clock = data[6] as u32 * 5000; // 5MHz units
    }
}
