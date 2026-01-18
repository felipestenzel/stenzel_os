// SPDX-License-Identifier: MIT
// HDR (High Dynamic Range) driver for Stenzel OS
// Supports HDR10, HDR10+, Dolby Vision metadata, and tone mapping

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::string::ToString;
use alloc::collections::BTreeMap;
use crate::sync::TicketSpinlock;

// Simple math helpers for no_std
fn pow2_approx(x: f32) -> f32 {
    // Approximation of 2^x using lookup + interpolation for small x
    // For HDR EDID values, x is typically 0-8 range
    if x <= 0.0 { return 1.0; }
    let int_part = x as u32;
    let frac_part = x - int_part as f32;
    let base = 1u32 << int_part.min(10);
    base as f32 * (1.0 + frac_part * 0.693)  // Linear approx for 2^frac
}

fn square(x: f32) -> f32 {
    x * x
}

/// HDR standards
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HdrStandard {
    Sdr,            // Standard Dynamic Range
    Hdr10,          // Static HDR10 (ST.2084 PQ, BT.2020, static metadata)
    Hdr10Plus,      // HDR10+ (dynamic metadata)
    DolbyVision,    // Dolby Vision
    Hlg,            // Hybrid Log-Gamma (broadcast)
    PqHdr,          // Generic PQ HDR
}

impl HdrStandard {
    pub fn name(self) -> &'static str {
        match self {
            HdrStandard::Sdr => "SDR",
            HdrStandard::Hdr10 => "HDR10",
            HdrStandard::Hdr10Plus => "HDR10+",
            HdrStandard::DolbyVision => "Dolby Vision",
            HdrStandard::Hlg => "HLG",
            HdrStandard::PqHdr => "PQ HDR",
        }
    }

    pub fn transfer_function(self) -> TransferFunction {
        match self {
            HdrStandard::Sdr => TransferFunction::Srgb,
            HdrStandard::Hdr10 | HdrStandard::Hdr10Plus |
            HdrStandard::DolbyVision | HdrStandard::PqHdr => TransferFunction::Pq,
            HdrStandard::Hlg => TransferFunction::Hlg,
        }
    }
}

/// Transfer functions (EOTF/OETF)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferFunction {
    Linear,
    Srgb,       // IEC 61966-2-1 sRGB
    Bt1886,     // BT.1886 for SDR displays
    Pq,         // SMPTE ST.2084 Perceptual Quantizer
    Hlg,        // ARIB STD-B67 Hybrid Log-Gamma
    Gamma22,    // Simple gamma 2.2
    Gamma24,    // Simple gamma 2.4
}

impl TransferFunction {
    pub fn is_hdr(self) -> bool {
        matches!(self, TransferFunction::Pq | TransferFunction::Hlg)
    }
}

/// Color spaces / Color primaries
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorPrimaries {
    Bt709,      // sRGB / Rec.709 (SDR)
    Bt2020,     // Rec.2020 (Wide Color Gamut for HDR)
    DciP3,      // DCI-P3 (cinema)
    DisplayP3,  // Display P3 (Apple)
    AdobeRgb,   // Adobe RGB
    Bt601,      // NTSC/PAL legacy
}

impl ColorPrimaries {
    pub fn is_wide_gamut(self) -> bool {
        matches!(self, ColorPrimaries::Bt2020 | ColorPrimaries::DciP3 |
                 ColorPrimaries::DisplayP3 | ColorPrimaries::AdobeRgb)
    }

    /// Get CIE 1931 xy chromaticity coordinates
    pub fn primaries_xy(self) -> [[f32; 2]; 3] {
        match self {
            ColorPrimaries::Bt709 => [
                [0.640, 0.330],  // Red
                [0.300, 0.600],  // Green
                [0.150, 0.060],  // Blue
            ],
            ColorPrimaries::Bt2020 => [
                [0.708, 0.292],
                [0.170, 0.797],
                [0.131, 0.046],
            ],
            ColorPrimaries::DciP3 | ColorPrimaries::DisplayP3 => [
                [0.680, 0.320],
                [0.265, 0.690],
                [0.150, 0.060],
            ],
            ColorPrimaries::AdobeRgb => [
                [0.640, 0.330],
                [0.210, 0.710],
                [0.150, 0.060],
            ],
            ColorPrimaries::Bt601 => [
                [0.630, 0.340],
                [0.310, 0.595],
                [0.155, 0.070],
            ],
        }
    }

    /// D65 white point
    pub fn white_point() -> [f32; 2] {
        [0.3127, 0.3290]
    }
}

/// HDR static metadata (SMPTE ST.2086)
#[derive(Debug, Clone, Copy)]
pub struct HdrStaticMetadata {
    pub primaries: ColorPrimaries,
    pub white_point: [f32; 2],         // CIE 1931 xy
    pub max_luminance: f32,            // cd/m² (nits)
    pub min_luminance: f32,            // cd/m² (nits)
    pub max_content_light: u16,        // MaxCLL (nits)
    pub max_frame_average_light: u16,  // MaxFALL (nits)
}

impl Default for HdrStaticMetadata {
    fn default() -> Self {
        Self {
            primaries: ColorPrimaries::Bt2020,
            white_point: ColorPrimaries::white_point(),
            max_luminance: 1000.0,
            min_luminance: 0.001,
            max_content_light: 1000,
            max_frame_average_light: 400,
        }
    }
}

/// HDR dynamic metadata (HDR10+ / Dolby Vision)
#[derive(Debug, Clone)]
pub struct HdrDynamicMetadata {
    pub scene_max_luminance: f32,
    pub scene_avg_luminance: f32,
    pub bezier_curve_anchors: Vec<f32>,  // Tone mapping curve
    pub knee_point: [f32; 2],
}

impl Default for HdrDynamicMetadata {
    fn default() -> Self {
        Self {
            scene_max_luminance: 1000.0,
            scene_avg_luminance: 200.0,
            bezier_curve_anchors: vec![0.0, 0.25, 0.5, 0.75, 1.0],
            knee_point: [0.5, 0.5],
        }
    }
}

/// Tone mapping operator
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToneMapper {
    None,           // No tone mapping
    Reinhard,       // Reinhard global
    ReinhardMod,    // Modified Reinhard
    Aces,           // ACES filmic
    AcesApprox,     // ACES approximation
    Uncharted2,     // Uncharted 2 filmic
    Hable,          // Hable/Uncharted
    AgX,            // AgX (modern)
    Bt2390,         // ITU-R BT.2390 reference
}

impl ToneMapper {
    pub fn name(self) -> &'static str {
        match self {
            ToneMapper::None => "None",
            ToneMapper::Reinhard => "Reinhard",
            ToneMapper::ReinhardMod => "Reinhard Modified",
            ToneMapper::Aces => "ACES",
            ToneMapper::AcesApprox => "ACES Approximation",
            ToneMapper::Uncharted2 => "Uncharted 2",
            ToneMapper::Hable => "Hable",
            ToneMapper::AgX => "AgX",
            ToneMapper::Bt2390 => "BT.2390",
        }
    }

    /// Apply tone mapping (simplified, single value)
    pub fn apply(self, x: f32, max_luminance: f32) -> f32 {
        match self {
            ToneMapper::None => x,
            ToneMapper::Reinhard => x / (1.0 + x),
            ToneMapper::ReinhardMod => {
                let white = max_luminance;
                x * (1.0 + x / (white * white)) / (1.0 + x)
            }
            ToneMapper::AcesApprox => {
                let a = 2.51;
                let b = 0.03;
                let c = 2.43;
                let d = 0.59;
                let e = 0.14;
                (x * (a * x + b)) / (x * (c * x + d) + e)
            }
            _ => x / (1.0 + x),  // Fallback to Reinhard
        }
    }
}

/// Display HDR capabilities
#[derive(Debug, Clone)]
pub struct DisplayHdrCapabilities {
    pub hdr_supported: bool,
    pub standards: Vec<HdrStandard>,
    pub eotfs: Vec<TransferFunction>,
    pub color_primaries: ColorPrimaries,
    pub max_luminance: f32,
    pub min_luminance: f32,
    pub max_full_frame_luminance: f32,
    pub color_depth: u8,  // 8, 10, 12 bits
}

impl Default for DisplayHdrCapabilities {
    fn default() -> Self {
        Self {
            hdr_supported: false,
            standards: vec![HdrStandard::Sdr],
            eotfs: vec![TransferFunction::Srgb],
            color_primaries: ColorPrimaries::Bt709,
            max_luminance: 300.0,
            min_luminance: 0.5,
            max_full_frame_luminance: 300.0,
            color_depth: 8,
        }
    }
}

/// HDR state for a connector
#[derive(Debug, Clone)]
pub struct ConnectorHdrState {
    pub connector_id: u32,
    pub enabled: bool,
    pub current_standard: HdrStandard,
    pub static_metadata: HdrStaticMetadata,
    pub dynamic_metadata: Option<HdrDynamicMetadata>,
    pub tone_mapper: ToneMapper,
    pub display_caps: DisplayHdrCapabilities,
    pub sdr_boost: f32,       // SDR content brightness boost (1.0 = no boost)
    pub paper_white: f32,     // Reference white level (nits)
}

impl ConnectorHdrState {
    pub fn new(connector_id: u32) -> Self {
        Self {
            connector_id,
            enabled: false,
            current_standard: HdrStandard::Sdr,
            static_metadata: HdrStaticMetadata::default(),
            dynamic_metadata: None,
            tone_mapper: ToneMapper::Bt2390,
            display_caps: DisplayHdrCapabilities::default(),
            sdr_boost: 1.0,
            paper_white: 203.0,  // SDR reference white in HDR
        }
    }
}

/// EDID HDR parsing
pub mod edid_hdr {
    // CEA-861 HDR Static Metadata Data Block
    pub const HDR_STATIC_METADATA_BLOCK: u8 = 0x06;

    // EOTF flags
    pub const EOTF_TRADITIONAL_SDR: u8 = 0x01;
    pub const EOTF_TRADITIONAL_HDR: u8 = 0x02;
    pub const EOTF_SMPTE_ST2084: u8 = 0x04;
    pub const EOTF_HLG: u8 = 0x08;

    // Static metadata type flags
    pub const SM_TYPE1: u8 = 0x01;  // ST.2086 supported
}

/// InfoFrame types for HDR
pub mod infoframe {
    // HDR Dynamic Range and Mastering InfoFrame
    pub const HDR_DRM_TYPE: u8 = 0x87;
    pub const HDR_DRM_VERSION: u8 = 0x01;

    // EOTF codes
    pub const EOTF_SDR_LUMINANCE: u8 = 0;
    pub const EOTF_HDR_LUMINANCE: u8 = 1;
    pub const EOTF_SMPTE_ST2084: u8 = 2;
    pub const EOTF_HLG: u8 = 3;

    // Static metadata descriptor
    pub const SM_TYPE1: u8 = 0;
}

/// HDR Controller
pub struct HdrController {
    pub gpu_vendor: u16,
    pub gpu_device: u16,
    pub connectors: BTreeMap<u32, ConnectorHdrState>,
    pub global_enabled: bool,
    pub auto_hdr: bool,  // Auto-enable HDR for HDR content
    mmio_base: u64,
    initialized: bool,
}

impl HdrController {
    pub const fn new() -> Self {
        Self {
            gpu_vendor: 0,
            gpu_device: 0,
            connectors: BTreeMap::new(),
            global_enabled: false,
            auto_hdr: true,
            mmio_base: 0,
            initialized: false,
        }
    }

    /// Initialize HDR controller
    pub fn init(&mut self, gpu_vendor: u16, gpu_device: u16, mmio_base: u64) -> Result<(), &'static str> {
        self.gpu_vendor = gpu_vendor;
        self.gpu_device = gpu_device;
        self.mmio_base = mmio_base;
        self.initialized = true;

        crate::kprintln!("HDR: Initialized for GPU {:04X}:{:04X}", gpu_vendor, gpu_device);
        Ok(())
    }

    /// Parse EDID for HDR capabilities
    pub fn parse_edid_hdr(&self, edid: &[u8]) -> DisplayHdrCapabilities {
        let mut caps = DisplayHdrCapabilities::default();

        if edid.len() < 128 {
            return caps;
        }

        let num_extensions = edid[126] as usize;
        if num_extensions == 0 || edid.len() < 128 + 128 * num_extensions {
            return caps;
        }

        // Scan CEA extensions for HDR data block
        for ext_idx in 0..num_extensions {
            let ext_start = 128 + ext_idx * 128;
            let ext_block = &edid[ext_start..ext_start + 128];

            if ext_block[0] == 0x02 {  // CEA extension
                self.parse_cea_hdr(ext_block, &mut caps);
            }
        }

        caps
    }

    /// Parse CEA extension for HDR metadata
    fn parse_cea_hdr(&self, cea_block: &[u8], caps: &mut DisplayHdrCapabilities) {
        let dtd_start = cea_block[2] as usize;
        if dtd_start < 4 || dtd_start > 127 {
            return;
        }

        let mut offset = 4;
        while offset < dtd_start {
            let header = cea_block[offset];
            let tag = (header >> 5) & 0x07;
            let length = (header & 0x1F) as usize;

            if offset + 1 + length > dtd_start {
                break;
            }

            if tag == 7 {  // Extended tag
                let ext_tag = cea_block[offset + 1];

                // HDR Static Metadata Data Block
                if ext_tag == edid_hdr::HDR_STATIC_METADATA_BLOCK && length >= 3 {
                    let eotf_byte = cea_block[offset + 2];
                    let sm_byte = cea_block[offset + 3];

                    if eotf_byte & edid_hdr::EOTF_SMPTE_ST2084 != 0 {
                        caps.hdr_supported = true;
                        caps.standards.push(HdrStandard::Hdr10);
                        caps.eotfs.push(TransferFunction::Pq);
                    }
                    if eotf_byte & edid_hdr::EOTF_HLG != 0 {
                        caps.hdr_supported = true;
                        caps.standards.push(HdrStandard::Hlg);
                        caps.eotfs.push(TransferFunction::Hlg);
                    }

                    if sm_byte & edid_hdr::SM_TYPE1 != 0 && length >= 6 {
                        // Max luminance (stored as 50*2^(x/32), in cd/m²)
                        let max_lum_raw = cea_block[offset + 4];
                        if max_lum_raw > 0 {
                            caps.max_luminance = 50.0 * pow2_approx(max_lum_raw as f32 / 32.0);
                        }

                        // Max frame-average luminance
                        let max_fall_raw = cea_block[offset + 5];
                        if max_fall_raw > 0 {
                            caps.max_full_frame_luminance = 50.0 * pow2_approx(max_fall_raw as f32 / 32.0);
                        }

                        // Min luminance (stored as max_lum * (x/255)^2 / 100)
                        if length >= 7 {
                            let min_lum_raw = cea_block[offset + 6];
                            caps.min_luminance = caps.max_luminance *
                                square(min_lum_raw as f32 / 255.0) / 100.0;
                        }
                    }

                    caps.color_primaries = ColorPrimaries::Bt2020;
                    caps.color_depth = 10;
                }

                // Colorimetry Data Block
                if ext_tag == 0x05 && length >= 2 {
                    let colorimetry = cea_block[offset + 2];
                    if colorimetry & 0x80 != 0 {  // BT.2020 RGB
                        caps.color_primaries = ColorPrimaries::Bt2020;
                    }
                }
            }

            offset += 1 + length;
        }
    }

    /// Register connector
    pub fn register_connector(&mut self, connector_id: u32) {
        self.connectors.insert(connector_id, ConnectorHdrState::new(connector_id));
    }

    /// Set display HDR capabilities from EDID
    pub fn set_display_caps(&mut self, connector_id: u32, edid: &[u8]) {
        let caps = self.parse_edid_hdr(edid);

        if let Some(state) = self.connectors.get_mut(&connector_id) {
            state.display_caps = caps;

            if state.display_caps.hdr_supported {
                crate::kprintln!("HDR: Connector {} supports HDR (max {}nits)",
                    connector_id, state.display_caps.max_luminance);
            }
        }
    }

    /// Enable HDR
    pub fn enable(&mut self, connector_id: u32, standard: HdrStandard) -> Result<(), &'static str> {
        if let Some(state) = self.connectors.get_mut(&connector_id) {
            if !state.display_caps.hdr_supported && standard != HdrStandard::Sdr {
                return Err("Display does not support HDR");
            }

            if !state.display_caps.standards.contains(&standard) && standard != HdrStandard::Sdr {
                return Err("Display does not support this HDR standard");
            }

            state.enabled = true;
            state.current_standard = standard;

            // Set appropriate static metadata
            state.static_metadata = HdrStaticMetadata {
                primaries: state.display_caps.color_primaries,
                white_point: ColorPrimaries::white_point(),
                max_luminance: state.display_caps.max_luminance,
                min_luminance: state.display_caps.min_luminance,
                max_content_light: state.display_caps.max_luminance as u16,
                max_frame_average_light: state.display_caps.max_full_frame_luminance as u16,
            };

            // Send HDR infoframe
            self.send_hdr_infoframe(connector_id)?;

            crate::kprintln!("HDR: Enabled {} on connector {}", standard.name(), connector_id);
            Ok(())
        } else {
            Err("Connector not found")
        }
    }

    /// Disable HDR
    pub fn disable(&mut self, connector_id: u32) -> Result<(), &'static str> {
        if let Some(state) = self.connectors.get_mut(&connector_id) {
            state.enabled = false;
            state.current_standard = HdrStandard::Sdr;

            crate::kprintln!("HDR: Disabled on connector {}", connector_id);
            Ok(())
        } else {
            Err("Connector not found")
        }
    }

    /// Set static metadata
    pub fn set_static_metadata(&mut self, connector_id: u32, metadata: HdrStaticMetadata)
        -> Result<(), &'static str>
    {
        if let Some(state) = self.connectors.get_mut(&connector_id) {
            state.static_metadata = metadata;

            if state.enabled {
                self.send_hdr_infoframe(connector_id)?;
            }
            Ok(())
        } else {
            Err("Connector not found")
        }
    }

    /// Set dynamic metadata (HDR10+)
    pub fn set_dynamic_metadata(&mut self, connector_id: u32, metadata: HdrDynamicMetadata)
        -> Result<(), &'static str>
    {
        if let Some(state) = self.connectors.get_mut(&connector_id) {
            state.dynamic_metadata = Some(metadata);
            Ok(())
        } else {
            Err("Connector not found")
        }
    }

    /// Set tone mapper
    pub fn set_tone_mapper(&mut self, connector_id: u32, mapper: ToneMapper)
        -> Result<(), &'static str>
    {
        if let Some(state) = self.connectors.get_mut(&connector_id) {
            state.tone_mapper = mapper;
            crate::kprintln!("HDR: Set tone mapper to {} on connector {}",
                mapper.name(), connector_id);
            Ok(())
        } else {
            Err("Connector not found")
        }
    }

    /// Set SDR brightness boost
    pub fn set_sdr_boost(&mut self, connector_id: u32, boost: f32) -> Result<(), &'static str> {
        if let Some(state) = self.connectors.get_mut(&connector_id) {
            state.sdr_boost = boost.clamp(1.0, 4.0);
            Ok(())
        } else {
            Err("Connector not found")
        }
    }

    /// Set paper white level (SDR reference white in HDR mode)
    pub fn set_paper_white(&mut self, connector_id: u32, nits: f32) -> Result<(), &'static str> {
        if let Some(state) = self.connectors.get_mut(&connector_id) {
            state.paper_white = nits.clamp(80.0, 500.0);
            Ok(())
        } else {
            Err("Connector not found")
        }
    }

    /// Send HDR infoframe to display
    fn send_hdr_infoframe(&self, connector_id: u32) -> Result<(), &'static str> {
        if let Some(state) = self.connectors.get(&connector_id) {
            // Build InfoFrame packet
            let mut _infoframe = [0u8; 32];

            // Header
            _infoframe[0] = infoframe::HDR_DRM_TYPE;
            _infoframe[1] = infoframe::HDR_DRM_VERSION;
            _infoframe[2] = 26;  // Length

            // EOTF
            _infoframe[3] = match state.current_standard {
                HdrStandard::Sdr => infoframe::EOTF_SDR_LUMINANCE,
                HdrStandard::Hdr10 | HdrStandard::Hdr10Plus |
                HdrStandard::DolbyVision => infoframe::EOTF_SMPTE_ST2084,
                HdrStandard::Hlg => infoframe::EOTF_HLG,
                HdrStandard::PqHdr => infoframe::EOTF_SMPTE_ST2084,
            };

            // Static metadata descriptor ID
            _infoframe[4] = infoframe::SM_TYPE1;

            // In a real implementation, this would write to HDMI/DP infoframe registers

            Ok(())
        } else {
            Err("Connector not found")
        }
    }

    /// Get HDR state
    pub fn get_state(&self, connector_id: u32) -> Option<&ConnectorHdrState> {
        self.connectors.get(&connector_id)
    }

    /// Get status string
    pub fn get_status(&self) -> String {
        let mut status = String::new();

        status.push_str("HDR Status:\n");
        status.push_str(&alloc::format!("  GPU: {:04X}:{:04X}\n", self.gpu_vendor, self.gpu_device));
        status.push_str(&alloc::format!("  Global Enabled: {}\n", self.global_enabled));
        status.push_str(&alloc::format!("  Auto HDR: {}\n", self.auto_hdr));

        for (id, state) in &self.connectors {
            status.push_str(&alloc::format!("  Connector {}:\n", id));
            status.push_str(&alloc::format!("    HDR Support: {}\n", state.display_caps.hdr_supported));
            status.push_str(&alloc::format!("    Current: {}\n", state.current_standard.name()));
            status.push_str(&alloc::format!("    Enabled: {}\n", state.enabled));

            if state.display_caps.hdr_supported {
                status.push_str(&alloc::format!("    Max Luminance: {} nits\n",
                    state.display_caps.max_luminance));
                status.push_str(&alloc::format!("    Min Luminance: {} nits\n",
                    state.display_caps.min_luminance));
                status.push_str(&alloc::format!("    Color Depth: {} bits\n",
                    state.display_caps.color_depth));
            }

            if state.enabled {
                status.push_str(&alloc::format!("    Tone Mapper: {}\n", state.tone_mapper.name()));
                status.push_str(&alloc::format!("    Paper White: {} nits\n", state.paper_white));
                status.push_str(&alloc::format!("    SDR Boost: {}x\n", state.sdr_boost));
            }
        }

        status
    }
}

/// Global HDR controller
static HDR_CONTROLLER: TicketSpinlock<Option<HdrController>> = TicketSpinlock::new(None);

/// Initialize HDR
pub fn init(gpu_vendor: u16, gpu_device: u16, mmio_base: u64) -> Result<(), &'static str> {
    let mut guard = HDR_CONTROLLER.lock();
    let mut controller = HdrController::new();
    controller.init(gpu_vendor, gpu_device, mmio_base)?;
    *guard = Some(controller);
    Ok(())
}

/// Get HDR controller
pub fn get_controller() -> Option<&'static TicketSpinlock<Option<HdrController>>> {
    Some(&HDR_CONTROLLER)
}
