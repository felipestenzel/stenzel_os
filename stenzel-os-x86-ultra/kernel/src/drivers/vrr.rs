// SPDX-License-Identifier: MIT
// VRR (Variable Refresh Rate) / FreeSync / G-SYNC driver for Stenzel OS
// Adaptive sync support for tearing-free display

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::string::ToString;
use alloc::collections::BTreeMap;
use crate::sync::TicketSpinlock;

/// VRR technology types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VrrTechnology {
    None,
    AmdFreeSync,
    AmdFreeSyncPremium,
    AmdFreeSyncPremiumPro,
    NvidiaGSync,
    NvidiaGSyncCompatible,
    NvidiaGSyncUltimate,
    VesaAdaptiveSync,  // DisplayPort Adaptive-Sync
    HdmiVrr,           // HDMI 2.1 VRR
}

impl VrrTechnology {
    pub fn name(self) -> &'static str {
        match self {
            VrrTechnology::None => "None",
            VrrTechnology::AmdFreeSync => "AMD FreeSync",
            VrrTechnology::AmdFreeSyncPremium => "AMD FreeSync Premium",
            VrrTechnology::AmdFreeSyncPremiumPro => "AMD FreeSync Premium Pro",
            VrrTechnology::NvidiaGSync => "NVIDIA G-SYNC",
            VrrTechnology::NvidiaGSyncCompatible => "NVIDIA G-SYNC Compatible",
            VrrTechnology::NvidiaGSyncUltimate => "NVIDIA G-SYNC Ultimate",
            VrrTechnology::VesaAdaptiveSync => "VESA Adaptive-Sync",
            VrrTechnology::HdmiVrr => "HDMI 2.1 VRR",
        }
    }

    pub fn supports_hdr(self) -> bool {
        matches!(self,
            VrrTechnology::AmdFreeSyncPremiumPro |
            VrrTechnology::NvidiaGSync |
            VrrTechnology::NvidiaGSyncUltimate
        )
    }

    pub fn supports_lfc(self) -> bool {
        // LFC = Low Framerate Compensation
        matches!(self,
            VrrTechnology::AmdFreeSyncPremium |
            VrrTechnology::AmdFreeSyncPremiumPro |
            VrrTechnology::NvidiaGSync |
            VrrTechnology::NvidiaGSyncUltimate
        )
    }
}

/// VRR state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VrrState {
    Disabled,
    Enabled,
    Active,      // VRR is actively adjusting refresh
    Inactive,    // VRR enabled but within fixed range
    Error,
}

/// Connector type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectorType {
    Unknown,
    DisplayPort,
    Hdmi,
    Dvi,
    Vga,
    Edp,
}

impl ConnectorType {
    pub fn supports_vrr(self) -> bool {
        matches!(self,
            ConnectorType::DisplayPort |
            ConnectorType::Hdmi |
            ConnectorType::Edp
        )
    }
}

/// VRR range (min/max refresh rates)
#[derive(Debug, Clone, Copy)]
pub struct VrrRange {
    pub min_hz: u32,
    pub max_hz: u32,
}

impl VrrRange {
    pub fn new(min: u32, max: u32) -> Self {
        Self { min_hz: min, max_hz: max }
    }

    pub fn contains(&self, rate: u32) -> bool {
        rate >= self.min_hz && rate <= self.max_hz
    }

    pub fn span(&self) -> u32 {
        self.max_hz.saturating_sub(self.min_hz)
    }

    /// Check if LFC (Low Framerate Compensation) can help
    pub fn lfc_effective(&self) -> bool {
        // LFC is effective when max >= 2 * min
        self.max_hz >= self.min_hz * 2
    }
}

/// Monitor EDID VRR information
#[derive(Debug, Clone)]
pub struct EdidVrrInfo {
    pub supported: bool,
    pub technology: VrrTechnology,
    pub range: VrrRange,
    pub version: u8,
}

impl Default for EdidVrrInfo {
    fn default() -> Self {
        Self {
            supported: false,
            technology: VrrTechnology::None,
            range: VrrRange::new(0, 0),
            version: 0,
        }
    }
}

/// Monitor information
#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub id: u32,
    pub name: String,
    pub connector: ConnectorType,
    pub native_refresh: u32,
    pub current_refresh: u32,
    pub vrr_info: EdidVrrInfo,
}

/// GPU VRR capabilities
#[derive(Debug, Clone)]
pub struct GpuVrrCapabilities {
    pub supported: bool,
    pub technologies: Vec<VrrTechnology>,
    pub min_refresh: u32,
    pub max_refresh: u32,
}

/// VRR timing parameters
#[derive(Debug, Clone, Copy)]
pub struct VrrTiming {
    pub vfp_base: u32,      // Base vertical front porch
    pub vfp_extend: u32,    // Extended VFP for lower refresh
    pub vsync: u32,         // VSync duration
    pub vbp: u32,           // Vertical back porch
    pub vactive: u32,       // Active lines
}

impl VrrTiming {
    /// Calculate total vertical timing for a target refresh rate
    pub fn total_lines_for_refresh(&self, target_hz: u32, pixel_clock_khz: u32, hactive: u32) -> u32 {
        if target_hz == 0 {
            return 0;
        }
        // Total lines = pixel_clock / (h_total * target_refresh)
        let htotal = hactive;  // Simplified
        pixel_clock_khz * 1000 / (htotal * target_hz)
    }
}

/// LFC (Low Framerate Compensation) settings
#[derive(Debug, Clone, Copy)]
pub struct LfcSettings {
    pub enabled: bool,
    pub multiplier: u32,     // Frame repeat count
    pub threshold_hz: u32,   // Below this rate, activate LFC
}

impl Default for LfcSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            multiplier: 2,
            threshold_hz: 48,  // Common min for 48-144Hz monitors
        }
    }
}

/// Per-connector VRR state
#[derive(Debug, Clone)]
pub struct ConnectorVrrState {
    pub connector_id: u32,
    pub enabled: bool,
    pub state: VrrState,
    pub technology: VrrTechnology,
    pub range: VrrRange,
    pub current_refresh: u32,
    pub target_refresh: u32,
    pub lfc: LfcSettings,
    pub timing: Option<VrrTiming>,
}

impl ConnectorVrrState {
    pub fn new(connector_id: u32) -> Self {
        Self {
            connector_id,
            enabled: false,
            state: VrrState::Disabled,
            technology: VrrTechnology::None,
            range: VrrRange::new(0, 0),
            current_refresh: 60,
            target_refresh: 60,
            lfc: LfcSettings::default(),
            timing: None,
        }
    }
}

/// DisplayPort Adaptive-Sync DPCD registers
pub mod dp_dpcd {
    pub const DOWNSPREAD_CTRL: u32 = 0x107;
    pub const EDP_CONFIGURATION_SET: u32 = 0x10A;
    pub const ADAPTIVE_SYNC_CAPS: u32 = 0x1A0;
    pub const ADAPTIVE_SYNC_CTRL: u32 = 0x1A1;
    pub const ADAPTIVE_SYNC_STATUS: u32 = 0x1A2;
    pub const VRR_MIN_REFRESH: u32 = 0x1A3;
    pub const VRR_MAX_REFRESH: u32 = 0x1A4;

    // Bits in ADAPTIVE_SYNC_CAPS
    pub const ADAPTIVE_SYNC_SDP_SUPPORTED: u8 = 0x01;
    pub const ADAPTIVE_SYNC_AS_SUPPORTED: u8 = 0x02;

    // Bits in ADAPTIVE_SYNC_CTRL
    pub const ADAPTIVE_SYNC_ENABLE: u8 = 0x01;
    pub const ADAPTIVE_SYNC_IGNORE_MSA: u8 = 0x02;
}

/// HDMI 2.1 VRR data structures
pub mod hdmi_vrr {
    // HF-VSDB (HDMI Forum Vendor-Specific Data Block)
    pub const VRR_MIN_INDEX: usize = 8;
    pub const VRR_MAX_INDEX: usize = 9;

    // FreeSync VSDB
    pub const AMD_VSDB_OUI: [u8; 3] = [0x00, 0x00, 0x1A];  // AMD OUI

    // Capabilities
    pub const QFT_SUPPORT: u8 = 0x01;  // Quick Frame Transport
    pub const QMS_SUPPORT: u8 = 0x02;  // Quick Media Switching
    pub const VRR_SUPPORT: u8 = 0x40;
    pub const M_DELTA: u8 = 0x80;
}

/// AMD FreeSync specific
pub mod freesync {
    // VESA VSDB extended block
    pub const FREESYNC_V1: u8 = 0x01;
    pub const FREESYNC_V2: u8 = 0x02;
    pub const FREESYNC_PREMIUM: u8 = 0x03;
    pub const FREESYNC_PREMIUM_PRO: u8 = 0x04;

    // Status bits
    pub const FS_ACTIVE: u8 = 0x01;
    pub const FS_LFC_ACTIVE: u8 = 0x02;
    pub const FS_HDR_ACTIVE: u8 = 0x04;
}

/// NVIDIA G-SYNC specific
pub mod gsync {
    // G-SYNC module types
    pub const MODULE_V1: u32 = 1;
    pub const MODULE_V2: u32 = 2;
    pub const COMPATIBLE: u32 = 3;
    pub const ULTIMATE: u32 = 4;

    // G-SYNC DPCD
    pub const GSYNC_CAP: u32 = 0x2200;
    pub const GSYNC_CTRL: u32 = 0x2201;
}

/// VRR Controller
pub struct VrrController {
    // GPU information
    pub gpu_vendor: u16,
    pub gpu_device: u16,
    pub gpu_capabilities: GpuVrrCapabilities,

    // Connector states
    pub connectors: BTreeMap<u32, ConnectorVrrState>,

    // Monitor information
    pub monitors: Vec<MonitorInfo>,

    // Global settings
    pub global_enabled: bool,
    pub force_enable: bool,  // Enable even without proper EDID
    pub allow_tearing: bool,

    // MMIO base for GPU register access
    mmio_base: u64,

    initialized: bool,
}

impl VrrController {
    pub const fn new() -> Self {
        Self {
            gpu_vendor: 0,
            gpu_device: 0,
            gpu_capabilities: GpuVrrCapabilities {
                supported: false,
                technologies: Vec::new(),
                min_refresh: 0,
                max_refresh: 0,
            },
            connectors: BTreeMap::new(),
            monitors: Vec::new(),
            global_enabled: false,
            force_enable: false,
            allow_tearing: false,
            mmio_base: 0,
            initialized: false,
        }
    }

    /// Initialize VRR controller
    pub fn init(&mut self, gpu_vendor: u16, gpu_device: u16, mmio_base: u64) -> Result<(), &'static str> {
        self.gpu_vendor = gpu_vendor;
        self.gpu_device = gpu_device;
        self.mmio_base = mmio_base;

        // Initialize GPU-specific capabilities
        match gpu_vendor {
            0x8086 => self.init_intel()?,
            0x1002 => self.init_amd()?,
            0x10DE => self.init_nvidia()?,
            _ => return Err("Unsupported GPU vendor for VRR"),
        }

        self.initialized = true;
        crate::kprintln!("VRR: Initialized, {} technologies supported",
            self.gpu_capabilities.technologies.len());

        Ok(())
    }

    /// Initialize Intel VRR (Panel Self Refresh / VRR)
    fn init_intel(&mut self) -> Result<(), &'static str> {
        self.gpu_capabilities = GpuVrrCapabilities {
            supported: true,
            technologies: vec![VrrTechnology::VesaAdaptiveSync, VrrTechnology::HdmiVrr],
            min_refresh: 40,
            max_refresh: 360,
        };
        Ok(())
    }

    /// Initialize AMD FreeSync
    fn init_amd(&mut self) -> Result<(), &'static str> {
        self.gpu_capabilities = GpuVrrCapabilities {
            supported: true,
            technologies: vec![
                VrrTechnology::AmdFreeSync,
                VrrTechnology::AmdFreeSyncPremium,
                VrrTechnology::AmdFreeSyncPremiumPro,
                VrrTechnology::VesaAdaptiveSync,
                VrrTechnology::HdmiVrr,
            ],
            min_refresh: 30,
            max_refresh: 500,  // RDNA3 supports up to 500Hz
        };
        Ok(())
    }

    /// Initialize NVIDIA G-SYNC
    fn init_nvidia(&mut self) -> Result<(), &'static str> {
        self.gpu_capabilities = GpuVrrCapabilities {
            supported: true,
            technologies: vec![
                VrrTechnology::NvidiaGSync,
                VrrTechnology::NvidiaGSyncCompatible,
                VrrTechnology::NvidiaGSyncUltimate,
                VrrTechnology::VesaAdaptiveSync,
                VrrTechnology::HdmiVrr,
            ],
            min_refresh: 30,
            max_refresh: 480,
        };
        Ok(())
    }

    /// Parse EDID for VRR capabilities
    pub fn parse_edid_vrr(&self, edid: &[u8]) -> EdidVrrInfo {
        if edid.len() < 128 {
            return EdidVrrInfo::default();
        }

        let mut info = EdidVrrInfo::default();

        // Check extension blocks for VRR info
        let num_extensions = edid[126] as usize;
        if num_extensions == 0 || edid.len() < 128 + 128 * num_extensions {
            return info;
        }

        // Scan extension blocks
        for ext_idx in 0..num_extensions {
            let ext_start = 128 + ext_idx * 128;
            let ext_block = &edid[ext_start..ext_start + 128];

            if ext_block[0] == 0x02 {  // CEA extension
                // Look for VSDB and VRR data blocks
                if let Some(vrr_info) = self.parse_cea_vrr(ext_block) {
                    info = vrr_info;
                    break;
                }
            } else if ext_block[0] == 0x70 {  // DisplayID extension
                // Look for Adaptive-Sync in DisplayID
                if let Some(vrr_info) = self.parse_displayid_vrr(ext_block) {
                    info = vrr_info;
                    break;
                }
            }
        }

        info
    }

    /// Parse CEA extension for VRR info
    fn parse_cea_vrr(&self, cea_block: &[u8]) -> Option<EdidVrrInfo> {
        let dtd_start = cea_block[2] as usize;
        if dtd_start < 4 || dtd_start > 127 {
            return None;
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

                // HDMI Forum VSDB (VRR support)
                if ext_tag == 0x01 && length >= 10 {
                    let vrr_min = cea_block[offset + hdmi_vrr::VRR_MIN_INDEX + 1] as u32;
                    let vrr_max_low = cea_block[offset + hdmi_vrr::VRR_MAX_INDEX + 1] as u32;
                    let vrr_max = vrr_max_low | ((cea_block[offset + 10] as u32 & 0x03) << 8);

                    if vrr_min > 0 && vrr_max > vrr_min {
                        return Some(EdidVrrInfo {
                            supported: true,
                            technology: VrrTechnology::HdmiVrr,
                            range: VrrRange::new(vrr_min, vrr_max),
                            version: 1,
                        });
                    }
                }

                // AMD FreeSync VSDB
                if ext_tag == 0x13 && length >= 6 {
                    let version = cea_block[offset + 3];
                    let vrr_min = cea_block[offset + 4] as u32;
                    let vrr_max = cea_block[offset + 5] as u32;

                    let technology = match version {
                        1 => VrrTechnology::AmdFreeSync,
                        2 => VrrTechnology::AmdFreeSyncPremium,
                        3..=4 => VrrTechnology::AmdFreeSyncPremiumPro,
                        _ => VrrTechnology::AmdFreeSync,
                    };

                    return Some(EdidVrrInfo {
                        supported: true,
                        technology,
                        range: VrrRange::new(vrr_min, vrr_max),
                        version,
                    });
                }
            }

            offset += 1 + length;
        }

        None
    }

    /// Parse DisplayID for Adaptive-Sync
    fn parse_displayid_vrr(&self, _displayid_block: &[u8]) -> Option<EdidVrrInfo> {
        // DisplayID 2.0 Adaptive Sync Data Block parsing
        // Simplified - would need full DisplayID parsing
        None
    }

    /// Register a connector
    pub fn register_connector(&mut self, connector_id: u32, connector_type: ConnectorType) {
        let state = ConnectorVrrState::new(connector_id);
        self.connectors.insert(connector_id, state);

        // Check if connector supports VRR
        if connector_type.supports_vrr() {
            crate::kprintln!("VRR: Registered connector {} ({:?})", connector_id, connector_type);
        }
    }

    /// Set monitor VRR capabilities from EDID
    pub fn set_monitor_vrr(&mut self, connector_id: u32, edid: &[u8]) {
        let vrr_info = self.parse_edid_vrr(edid);

        if let Some(state) = self.connectors.get_mut(&connector_id) {
            state.technology = vrr_info.technology;
            state.range = vrr_info.range;

            // Configure LFC based on range
            if vrr_info.range.lfc_effective() && state.technology.supports_lfc() {
                state.lfc.enabled = true;
                state.lfc.threshold_hz = vrr_info.range.min_hz;
            }

            crate::kprintln!("VRR: Connector {} supports {} ({}-{}Hz)",
                connector_id, vrr_info.technology.name(),
                vrr_info.range.min_hz, vrr_info.range.max_hz);
        }
    }

    /// Enable VRR on a connector
    pub fn enable(&mut self, connector_id: u32) -> Result<(), &'static str> {
        if let Some(state) = self.connectors.get_mut(&connector_id) {
            if state.technology == VrrTechnology::None && !self.force_enable {
                return Err("VRR not supported on this connector");
            }

            state.enabled = true;
            state.state = VrrState::Enabled;

            // Enable at GPU level based on vendor
            match self.gpu_vendor {
                0x1002 => self.enable_freesync(connector_id)?,
                0x10DE => self.enable_gsync(connector_id)?,
                0x8086 => self.enable_adaptive_sync(connector_id)?,
                _ => {}
            }

            crate::kprintln!("VRR: Enabled on connector {}", connector_id);
            Ok(())
        } else {
            Err("Connector not found")
        }
    }

    /// Disable VRR on a connector
    pub fn disable(&mut self, connector_id: u32) -> Result<(), &'static str> {
        if let Some(state) = self.connectors.get_mut(&connector_id) {
            state.enabled = false;
            state.state = VrrState::Disabled;

            crate::kprintln!("VRR: Disabled on connector {}", connector_id);
            Ok(())
        } else {
            Err("Connector not found")
        }
    }

    /// Enable AMD FreeSync
    fn enable_freesync(&self, _connector_id: u32) -> Result<(), &'static str> {
        // Enable FreeSync through CRTC registers
        // This would write to DC_FreeSync_CTL registers

        Ok(())
    }

    /// Enable NVIDIA G-SYNC
    fn enable_gsync(&self, _connector_id: u32) -> Result<(), &'static str> {
        // Enable G-SYNC through NVIDIA-specific registers

        Ok(())
    }

    /// Enable VESA Adaptive-Sync (Intel/generic)
    fn enable_adaptive_sync(&self, _connector_id: u32) -> Result<(), &'static str> {
        // Enable Adaptive-Sync through DPCD or TRANS_VRR registers

        Ok(())
    }

    /// Set target refresh rate
    pub fn set_target_refresh(&mut self, connector_id: u32, target_hz: u32) -> Result<(), &'static str> {
        if let Some(state) = self.connectors.get_mut(&connector_id) {
            if !state.enabled {
                return Err("VRR not enabled");
            }

            let mut effective_hz = target_hz;

            // Apply LFC if needed
            if state.lfc.enabled && target_hz < state.range.min_hz {
                // Multiply frame rate to stay within VRR range
                let multiplier = (state.range.min_hz + target_hz - 1) / target_hz;
                effective_hz = target_hz * multiplier;
                state.lfc.multiplier = multiplier;
            }

            // Clamp to range
            effective_hz = effective_hz.clamp(state.range.min_hz, state.range.max_hz);

            state.target_refresh = effective_hz;

            // Update state
            if state.current_refresh != effective_hz {
                state.state = VrrState::Active;
            } else {
                state.state = VrrState::Inactive;
            }

            Ok(())
        } else {
            Err("Connector not found")
        }
    }

    /// Get current VRR state
    pub fn get_state(&self, connector_id: u32) -> Option<&ConnectorVrrState> {
        self.connectors.get(&connector_id)
    }

    /// Global enable
    pub fn set_global_enabled(&mut self, enabled: bool) {
        self.global_enabled = enabled;
        crate::kprintln!("VRR: Global {} ", if enabled { "enabled" } else { "disabled" });
    }

    /// Get status string
    pub fn get_status(&self) -> String {
        let mut status = String::new();

        status.push_str("VRR Status:\n");
        status.push_str(&alloc::format!("  GPU: {:04X}:{:04X}\n", self.gpu_vendor, self.gpu_device));
        status.push_str(&alloc::format!("  Supported: {}\n", self.gpu_capabilities.supported));
        status.push_str(&alloc::format!("  Global Enabled: {}\n", self.global_enabled));

        status.push_str("  Technologies:\n");
        for tech in &self.gpu_capabilities.technologies {
            status.push_str(&alloc::format!("    {}\n", tech.name()));
        }

        status.push_str(&alloc::format!("  Connectors: {}\n", self.connectors.len()));
        for (id, state) in &self.connectors {
            status.push_str(&alloc::format!("    Connector {}: {} ({:?})\n",
                id, state.technology.name(), state.state));
            if state.enabled {
                status.push_str(&alloc::format!("      Range: {}-{}Hz\n",
                    state.range.min_hz, state.range.max_hz));
                status.push_str(&alloc::format!("      Current: {}Hz, Target: {}Hz\n",
                    state.current_refresh, state.target_refresh));
                status.push_str(&alloc::format!("      LFC: {}\n",
                    if state.lfc.enabled { "Enabled" } else { "Disabled" }));
            }
        }

        status
    }
}

/// Global VRR controller
static VRR_CONTROLLER: TicketSpinlock<Option<VrrController>> = TicketSpinlock::new(None);

/// Initialize VRR
pub fn init(gpu_vendor: u16, gpu_device: u16, mmio_base: u64) -> Result<(), &'static str> {
    let mut guard = VRR_CONTROLLER.lock();
    let mut controller = VrrController::new();
    controller.init(gpu_vendor, gpu_device, mmio_base)?;
    *guard = Some(controller);
    Ok(())
}

/// Get VRR controller
pub fn get_controller() -> Option<&'static TicketSpinlock<Option<VrrController>>> {
    Some(&VRR_CONTROLLER)
}
