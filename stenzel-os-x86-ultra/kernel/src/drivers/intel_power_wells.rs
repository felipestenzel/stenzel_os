//! Intel GPU Power Wells Driver
//!
//! Manages power domains (power wells) for Intel integrated and discrete GPUs.
//! Power wells allow selective power gating of GPU subsystems to save power.
//!
//! Supported generations:
//! - Gen9 (Skylake+): Basic power wells
//! - Gen11 (Ice Lake): Enhanced power management
//! - Gen12/Xe (Tiger Lake+): Advanced power domains
//! - Xe-HPG (Arc): Discrete GPU power management

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use crate::sync::TicketSpinlock;

/// Power well identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum PowerWell {
    /// Always-on power domain
    Misc,
    /// DDI A-D power domains
    DdiA,
    DdiB,
    DdiC,
    DdiD,
    DdiE,
    /// DDI IO power domains
    DdiAIo,
    DdiBIo,
    DdiCIo,
    DdiDIo,
    /// AUX channels
    AuxA,
    AuxB,
    AuxC,
    AuxD,
    AuxE,
    AuxF,
    AuxUsbc1,
    AuxUsbc2,
    AuxUsbc3,
    AuxUsbc4,
    AuxUsbc5,
    AuxUsbc6,
    /// Display core
    DisplayCore,
    /// Power well 1 (primary display)
    Pw1,
    /// Power well 2 (secondary display)
    Pw2,
    /// Power well 3
    Pw3,
    /// Power well 4
    Pw4,
    /// Power well 5
    Pw5,
    /// DC off (display C-state)
    DcOff,
    /// GT power domain (graphics engine)
    Gt,
    /// Media power domain
    Media,
    /// VD box (video decode)
    Vdbox0,
    Vdbox1,
    Vdbox2,
    Vdbox3,
    /// VE box (video encode)
    Vebox0,
    Vebox1,
    /// Render/3D engine
    Render,
    /// Compute engines (Xe)
    Compute0,
    Compute1,
    Compute2,
    Compute3,
    /// Copy engine
    Copy0,
    Copy1,
    /// Memory fabric
    MemoryFabric,
}

impl PowerWell {
    /// Get power well name
    pub fn name(&self) -> &'static str {
        match self {
            Self::Misc => "MISC",
            Self::DdiA => "DDI_A",
            Self::DdiB => "DDI_B",
            Self::DdiC => "DDI_C",
            Self::DdiD => "DDI_D",
            Self::DdiE => "DDI_E",
            Self::DdiAIo => "DDI_A_IO",
            Self::DdiBIo => "DDI_B_IO",
            Self::DdiCIo => "DDI_C_IO",
            Self::DdiDIo => "DDI_D_IO",
            Self::AuxA => "AUX_A",
            Self::AuxB => "AUX_B",
            Self::AuxC => "AUX_C",
            Self::AuxD => "AUX_D",
            Self::AuxE => "AUX_E",
            Self::AuxF => "AUX_F",
            Self::AuxUsbc1 => "AUX_USBC1",
            Self::AuxUsbc2 => "AUX_USBC2",
            Self::AuxUsbc3 => "AUX_USBC3",
            Self::AuxUsbc4 => "AUX_USBC4",
            Self::AuxUsbc5 => "AUX_USBC5",
            Self::AuxUsbc6 => "AUX_USBC6",
            Self::DisplayCore => "DISPLAY_CORE",
            Self::Pw1 => "PW1",
            Self::Pw2 => "PW2",
            Self::Pw3 => "PW3",
            Self::Pw4 => "PW4",
            Self::Pw5 => "PW5",
            Self::DcOff => "DC_OFF",
            Self::Gt => "GT",
            Self::Media => "MEDIA",
            Self::Vdbox0 => "VDBOX0",
            Self::Vdbox1 => "VDBOX1",
            Self::Vdbox2 => "VDBOX2",
            Self::Vdbox3 => "VDBOX3",
            Self::Vebox0 => "VEBOX0",
            Self::Vebox1 => "VEBOX1",
            Self::Render => "RENDER",
            Self::Compute0 => "COMPUTE0",
            Self::Compute1 => "COMPUTE1",
            Self::Compute2 => "COMPUTE2",
            Self::Compute3 => "COMPUTE3",
            Self::Copy0 => "COPY0",
            Self::Copy1 => "COPY1",
            Self::MemoryFabric => "MEMORY_FABRIC",
        }
    }

    /// Check if this is a display power well
    pub fn is_display(&self) -> bool {
        matches!(self,
            Self::DisplayCore | Self::Pw1 | Self::Pw2 | Self::Pw3 | Self::Pw4 | Self::Pw5
            | Self::DdiA | Self::DdiB | Self::DdiC | Self::DdiD | Self::DdiE
            | Self::DdiAIo | Self::DdiBIo | Self::DdiCIo | Self::DdiDIo
            | Self::AuxA | Self::AuxB | Self::AuxC | Self::AuxD | Self::AuxE | Self::AuxF
            | Self::AuxUsbc1 | Self::AuxUsbc2 | Self::AuxUsbc3 | Self::AuxUsbc4 | Self::AuxUsbc5 | Self::AuxUsbc6
        )
    }

    /// Check if this is a GT (graphics) power well
    pub fn is_gt(&self) -> bool {
        matches!(self,
            Self::Gt | Self::Render
            | Self::Compute0 | Self::Compute1 | Self::Compute2 | Self::Compute3
            | Self::Copy0 | Self::Copy1
        )
    }

    /// Check if this is a media power well
    pub fn is_media(&self) -> bool {
        matches!(self,
            Self::Media | Self::Vdbox0 | Self::Vdbox1 | Self::Vdbox2 | Self::Vdbox3
            | Self::Vebox0 | Self::Vebox1
        )
    }
}

/// Power well state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerWellState {
    /// Power well is off
    Off,
    /// Power well is powering up
    PoweringUp,
    /// Power well is on and stable
    On,
    /// Power well is powering down
    PoweringDown,
    /// Unknown state
    Unknown,
}

impl PowerWellState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::PoweringUp => "powering-up",
            Self::On => "on",
            Self::PoweringDown => "powering-down",
            Self::Unknown => "unknown",
        }
    }

    pub fn is_on(&self) -> bool {
        matches!(self, Self::On)
    }
}

/// Power well information
#[derive(Debug, Clone)]
pub struct PowerWellInfo {
    /// Power well identifier
    pub well: PowerWell,
    /// Current state
    pub state: PowerWellState,
    /// Reference count (number of users)
    pub ref_count: u32,
    /// Is this power well always on?
    pub always_on: bool,
    /// Dependencies (wells that must be on for this to work)
    pub dependencies: Vec<PowerWell>,
    /// Dependent wells (wells that depend on this)
    pub dependents: Vec<PowerWell>,
}

/// MMIO register offsets for power wells
pub mod regs {
    /// Power well control (Gen9+)
    pub const PWR_WELL_CTL1: u32 = 0x45400;
    pub const PWR_WELL_CTL2: u32 = 0x45404;
    pub const PWR_WELL_CTL3: u32 = 0x45408;
    pub const PWR_WELL_CTL4: u32 = 0x4540C;

    /// Power well status
    pub const PWR_WELL_CTL_AUX: u32 = 0x45440;

    /// DC state control
    pub const DC_STATE_EN: u32 = 0x45504;
    pub const DC_STATE_DEBUG: u32 = 0x45520;

    /// DC state flags
    pub const DC_STATE_DC5_EN: u32 = 1 << 0;
    pub const DC_STATE_DC6_EN: u32 = 1 << 1;
    pub const DC_STATE_DC9_EN: u32 = 1 << 2;

    /// FUSE_STATUS for capability detection
    pub const FUSE_STATUS: u32 = 0x42000;

    /// Gen12+ power domains
    pub const PG_ENABLE: u32 = 0x45600;
    pub const PG_STATUS: u32 = 0x45604;

    /// GT power control
    pub const GT_POWER_GATE: u32 = 0xA090;
    pub const GT_FORCE_WAKE: u32 = 0xA188;
    pub const GT_CORE_STATUS: u32 = 0xA000;

    /// RC6 (render C-state) control
    pub const RC_CONTROL: u32 = 0xA090;
    pub const RC_STATE: u32 = 0xA094;
    pub const RC6_RESIDENCY: u32 = 0xA0A0;
    pub const RC6p_RESIDENCY: u32 = 0xA0A4;
    pub const RC6pp_RESIDENCY: u32 = 0xA0A8;

    /// Power well enable/request bits
    pub const PW_CTL_REQ: u32 = 1 << 31;
    pub const PW_CTL_STATE: u32 = 1 << 30;
    pub const PW_CTL_RESET: u32 = 1 << 29;

    /// AUX power well bits per channel
    pub const AUX_IO_POWER_REQ: u32 = 1 << 27;
    pub const AUX_IO_POWER_STATE: u32 = 1 << 26;
}

/// Platform generation for power well support
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// Gen9 (Skylake, Kaby Lake)
    Gen9,
    /// Gen9LP (Broxton, Gemini Lake)
    Gen9Lp,
    /// Gen10 (Cannon Lake - limited release)
    Gen10,
    /// Gen11 (Ice Lake)
    Gen11,
    /// Gen12 (Tiger Lake)
    Gen12,
    /// Gen12.5 (DG1)
    Gen12_5,
    /// Alder Lake
    AdlerLake,
    /// Raptor Lake
    RaptorLake,
    /// Xe-HPG (Arc)
    XeHpg,
    /// Unknown
    Unknown,
}

impl Platform {
    /// Detect platform from device ID
    pub fn from_device_id(device_id: u16) -> Self {
        match device_id >> 8 {
            0x19 => Self::Gen9,      // Skylake
            0x59 => Self::Gen9,      // Kaby Lake
            0x3E => Self::Gen9,      // Coffee Lake
            0x5A => Self::Gen9Lp,    // Broxton
            0x31 => Self::Gen9Lp,    // Gemini Lake
            0x8A => Self::Gen11,     // Ice Lake
            0x9A => Self::Gen12,     // Tiger Lake
            0x49 => Self::Gen12_5,   // DG1
            0x46 => Self::AdlerLake, // Alder Lake-S
            0xA7 => Self::RaptorLake, // Raptor Lake
            0x56 | 0x57 => Self::XeHpg, // Arc
            _ => Self::Unknown,
        }
    }

    /// Get supported power wells for this platform
    pub fn supported_wells(&self) -> Vec<PowerWell> {
        match self {
            Self::Gen9 | Self::Gen9Lp => vec![
                PowerWell::Misc,
                PowerWell::Pw1,
                PowerWell::Pw2,
                PowerWell::DdiA,
                PowerWell::DdiB,
                PowerWell::DdiC,
                PowerWell::DdiD,
                PowerWell::AuxA,
                PowerWell::AuxB,
                PowerWell::AuxC,
                PowerWell::AuxD,
            ],
            Self::Gen11 => vec![
                PowerWell::Misc,
                PowerWell::Pw1,
                PowerWell::Pw2,
                PowerWell::Pw3,
                PowerWell::Pw4,
                PowerWell::DdiA,
                PowerWell::DdiB,
                PowerWell::DdiC,
                PowerWell::DdiD,
                PowerWell::DdiE,
                PowerWell::AuxA,
                PowerWell::AuxB,
                PowerWell::AuxC,
                PowerWell::AuxD,
                PowerWell::AuxE,
                PowerWell::AuxF,
                PowerWell::Gt,
                PowerWell::Media,
            ],
            Self::Gen12 | Self::AdlerLake | Self::RaptorLake => vec![
                PowerWell::Misc,
                PowerWell::DisplayCore,
                PowerWell::Pw1,
                PowerWell::Pw2,
                PowerWell::Pw3,
                PowerWell::Pw4,
                PowerWell::Pw5,
                PowerWell::DdiA,
                PowerWell::DdiB,
                PowerWell::DdiC,
                PowerWell::DdiD,
                PowerWell::DdiAIo,
                PowerWell::DdiBIo,
                PowerWell::DdiCIo,
                PowerWell::DdiDIo,
                PowerWell::AuxA,
                PowerWell::AuxB,
                PowerWell::AuxC,
                PowerWell::AuxD,
                PowerWell::AuxUsbc1,
                PowerWell::AuxUsbc2,
                PowerWell::AuxUsbc3,
                PowerWell::AuxUsbc4,
                PowerWell::DcOff,
                PowerWell::Gt,
                PowerWell::Media,
                PowerWell::Vdbox0,
                PowerWell::Vdbox1,
                PowerWell::Vebox0,
                PowerWell::Vebox1,
                PowerWell::Render,
            ],
            Self::Gen12_5 | Self::XeHpg => vec![
                PowerWell::Misc,
                PowerWell::DisplayCore,
                PowerWell::Pw1,
                PowerWell::Pw2,
                PowerWell::DdiA,
                PowerWell::DdiB,
                PowerWell::AuxA,
                PowerWell::AuxB,
                PowerWell::Gt,
                PowerWell::Media,
                PowerWell::Vdbox0,
                PowerWell::Vdbox1,
                PowerWell::Vebox0,
                PowerWell::Vebox1,
                PowerWell::Render,
                PowerWell::Compute0,
                PowerWell::Compute1,
                PowerWell::Copy0,
                PowerWell::MemoryFabric,
            ],
            _ => vec![PowerWell::Misc],
        }
    }

    /// Check if platform supports DC5/DC6 states
    pub fn supports_dc_states(&self) -> bool {
        !matches!(self, Self::Unknown)
    }

    /// Check if platform supports RC6
    pub fn supports_rc6(&self) -> bool {
        !matches!(self, Self::Unknown)
    }
}

/// Power wells manager
pub struct PowerWellsManager {
    /// MMIO base address
    mmio_base: u64,
    /// Platform type
    platform: Platform,
    /// Power well states
    wells: Vec<PowerWellInfo>,
    /// DC state enabled
    dc_state: u32,
    /// RC6 enabled
    rc6_enabled: bool,
    /// Initialization complete
    initialized: bool,
}

impl PowerWellsManager {
    /// Create a new power wells manager
    pub fn new(mmio_base: u64, device_id: u16) -> Self {
        let platform = Platform::from_device_id(device_id);
        let supported = platform.supported_wells();

        let wells = supported.iter().map(|&well| {
            PowerWellInfo {
                well,
                state: PowerWellState::Unknown,
                ref_count: 0,
                always_on: matches!(well, PowerWell::Misc),
                dependencies: Vec::new(),
                dependents: Vec::new(),
            }
        }).collect();

        Self {
            mmio_base,
            platform,
            wells,
            dc_state: 0,
            rc6_enabled: false,
            initialized: false,
        }
    }

    /// Initialize power wells
    pub fn init(&mut self) {
        // Read current state of all power wells
        // Use index-based iteration to avoid borrow checker issues
        for i in 0..self.wells.len() {
            let well = self.wells[i].well;
            let state = self.read_power_well_state(well);
            self.wells[i].state = state;
        }

        // Setup dependencies
        self.setup_dependencies();

        // Enable always-on wells
        for i in 0..self.wells.len() {
            if self.wells[i].always_on && !self.wells[i].state.is_on() {
                let well = self.wells[i].well;
                self.enable_power_well(well);
            }
        }

        self.initialized = true;
    }

    /// Setup power well dependencies
    fn setup_dependencies(&mut self) {
        // PW2 depends on PW1, etc.
        // This varies by platform
        match self.platform {
            Platform::Gen12 | Platform::AdlerLake | Platform::RaptorLake => {
                // Gen12 dependency chain: DisplayCore -> PW1 -> PW2 -> PW3 -> etc.
                self.add_dependency(PowerWell::Pw1, PowerWell::DisplayCore);
                self.add_dependency(PowerWell::Pw2, PowerWell::Pw1);
                self.add_dependency(PowerWell::Pw3, PowerWell::Pw2);
                self.add_dependency(PowerWell::Pw4, PowerWell::Pw3);
                self.add_dependency(PowerWell::Pw5, PowerWell::Pw4);

                // DDI IO depends on display core
                self.add_dependency(PowerWell::DdiAIo, PowerWell::DisplayCore);
                self.add_dependency(PowerWell::DdiBIo, PowerWell::DisplayCore);
                self.add_dependency(PowerWell::DdiCIo, PowerWell::DisplayCore);
                self.add_dependency(PowerWell::DdiDIo, PowerWell::DisplayCore);
            }
            _ => {}
        }
    }

    /// Add a dependency between power wells
    fn add_dependency(&mut self, well: PowerWell, depends_on: PowerWell) {
        if let Some(w) = self.wells.iter_mut().find(|w| w.well == well) {
            if !w.dependencies.contains(&depends_on) {
                w.dependencies.push(depends_on);
            }
        }
        if let Some(w) = self.wells.iter_mut().find(|w| w.well == depends_on) {
            if !w.dependents.contains(&well) {
                w.dependents.push(well);
            }
        }
    }

    /// Read MMIO register
    fn read_reg(&self, offset: u32) -> u32 {
        unsafe {
            core::ptr::read_volatile((self.mmio_base + offset as u64) as *const u32)
        }
    }

    /// Write MMIO register
    fn write_reg(&self, offset: u32, value: u32) {
        unsafe {
            core::ptr::write_volatile((self.mmio_base + offset as u64) as *mut u32, value);
        }
    }

    /// Read power well state from hardware
    fn read_power_well_state(&self, well: PowerWell) -> PowerWellState {
        // Determine which register and bit to check based on well and platform
        let (reg, req_bit, state_bit) = self.get_well_register(well);
        if reg == 0 {
            return PowerWellState::Unknown;
        }

        let value = self.read_reg(reg);
        let requested = value & req_bit != 0;
        let enabled = value & state_bit != 0;

        if !requested {
            PowerWellState::Off
        } else if enabled {
            PowerWellState::On
        } else {
            PowerWellState::PoweringUp
        }
    }

    /// Get register and bits for a power well
    fn get_well_register(&self, well: PowerWell) -> (u32, u32, u32) {
        match self.platform {
            Platform::Gen9 | Platform::Gen9Lp => {
                match well {
                    PowerWell::Pw1 => (regs::PWR_WELL_CTL1, 1 << 29, 1 << 28),
                    PowerWell::Pw2 => (regs::PWR_WELL_CTL1, 1 << 31, 1 << 30),
                    PowerWell::DdiA => (regs::PWR_WELL_CTL2, 1 << 1, 1 << 0),
                    PowerWell::DdiB => (regs::PWR_WELL_CTL2, 1 << 3, 1 << 2),
                    PowerWell::DdiC => (regs::PWR_WELL_CTL2, 1 << 5, 1 << 4),
                    PowerWell::DdiD => (regs::PWR_WELL_CTL2, 1 << 7, 1 << 6),
                    _ => (0, 0, 0),
                }
            }
            Platform::Gen12 | Platform::AdlerLake | Platform::RaptorLake => {
                match well {
                    PowerWell::DisplayCore => (regs::PWR_WELL_CTL1, 1 << 31, 1 << 30),
                    PowerWell::Pw1 => (regs::PWR_WELL_CTL1, 1 << 29, 1 << 28),
                    PowerWell::Pw2 => (regs::PWR_WELL_CTL1, 1 << 27, 1 << 26),
                    PowerWell::Pw3 => (regs::PWR_WELL_CTL1, 1 << 25, 1 << 24),
                    PowerWell::Pw4 => (regs::PWR_WELL_CTL1, 1 << 23, 1 << 22),
                    PowerWell::Pw5 => (regs::PWR_WELL_CTL1, 1 << 21, 1 << 20),
                    PowerWell::DdiA => (regs::PWR_WELL_CTL2, 1 << 1, 1 << 0),
                    PowerWell::DdiB => (regs::PWR_WELL_CTL2, 1 << 3, 1 << 2),
                    PowerWell::DdiC => (regs::PWR_WELL_CTL2, 1 << 5, 1 << 4),
                    PowerWell::DdiD => (regs::PWR_WELL_CTL2, 1 << 7, 1 << 6),
                    _ => (0, 0, 0),
                }
            }
            _ => (0, 0, 0),
        }
    }

    /// Enable a power well
    pub fn enable_power_well(&mut self, well: PowerWell) -> bool {
        // First enable dependencies
        if let Some(deps) = self.wells.iter()
            .find(|w| w.well == well)
            .map(|w| w.dependencies.clone())
        {
            for dep in deps {
                self.enable_power_well(dep);
            }
        }

        let (reg, req_bit, state_bit) = self.get_well_register(well);
        if reg == 0 {
            return false;
        }

        // Request power well enable
        let value = self.read_reg(reg);
        self.write_reg(reg, value | req_bit);

        // Wait for power well to stabilize
        for _ in 0..1000 {
            let value = self.read_reg(reg);
            if value & state_bit != 0 {
                // Update state
                if let Some(w) = self.wells.iter_mut().find(|w| w.well == well) {
                    w.state = PowerWellState::On;
                    w.ref_count += 1;
                }
                return true;
            }
            // Small delay
            for _ in 0..1000 { core::hint::spin_loop(); }
        }

        false
    }

    /// Disable a power well
    pub fn disable_power_well(&mut self, well: PowerWell) -> bool {
        // Check if well can be disabled
        if let Some(w) = self.wells.iter().find(|w| w.well == well) {
            if w.always_on || w.ref_count > 1 {
                // Decrement ref count but don't disable
                if let Some(w) = self.wells.iter_mut().find(|w| w.well == well) {
                    if w.ref_count > 0 {
                        w.ref_count -= 1;
                    }
                }
                return true;
            }

            // Check if any dependents are still enabled
            for dep in &w.dependents {
                if let Some(dw) = self.wells.iter().find(|w| w.well == *dep) {
                    if dw.state.is_on() {
                        return false;
                    }
                }
            }
        }

        let (reg, req_bit, state_bit) = self.get_well_register(well);
        if reg == 0 {
            return false;
        }

        // Clear request bit
        let value = self.read_reg(reg);
        self.write_reg(reg, value & !req_bit);

        // Wait for power well to turn off
        for _ in 0..1000 {
            let value = self.read_reg(reg);
            if value & state_bit == 0 {
                // Update state
                if let Some(w) = self.wells.iter_mut().find(|w| w.well == well) {
                    w.state = PowerWellState::Off;
                    w.ref_count = 0;
                }
                return true;
            }
            for _ in 0..1000 { core::hint::spin_loop(); }
        }

        false
    }

    /// Get power well state
    pub fn get_state(&self, well: PowerWell) -> Option<PowerWellState> {
        self.wells.iter()
            .find(|w| w.well == well)
            .map(|w| w.state)
    }

    /// Enable DC5/DC6 states for power saving
    pub fn enable_dc_states(&mut self, dc5: bool, dc6: bool) {
        let mut value = 0u32;
        if dc5 {
            value |= regs::DC_STATE_DC5_EN;
        }
        if dc6 {
            value |= regs::DC_STATE_DC6_EN;
        }
        self.write_reg(regs::DC_STATE_EN, value);
        self.dc_state = value;
    }

    /// Disable DC states
    pub fn disable_dc_states(&mut self) {
        self.write_reg(regs::DC_STATE_EN, 0);
        self.dc_state = 0;
    }

    /// Enable RC6 (render C-state)
    pub fn enable_rc6(&mut self) {
        // Enable RC6 in control register
        let value = self.read_reg(regs::RC_CONTROL);
        self.write_reg(regs::RC_CONTROL, value | (1 << 18)); // RC6 enable bit
        self.rc6_enabled = true;
    }

    /// Disable RC6
    pub fn disable_rc6(&mut self) {
        let value = self.read_reg(regs::RC_CONTROL);
        self.write_reg(regs::RC_CONTROL, value & !(1 << 18));
        self.rc6_enabled = false;
    }

    /// Get RC6 residency (time spent in RC6)
    pub fn get_rc6_residency(&self) -> u64 {
        self.read_reg(regs::RC6_RESIDENCY) as u64
    }

    /// Force wake the GT
    pub fn gt_force_wake(&mut self) {
        self.write_reg(regs::GT_FORCE_WAKE, 0xFFFF0001);

        // Wait for wake
        for _ in 0..1000 {
            let status = self.read_reg(regs::GT_CORE_STATUS);
            if status & 1 != 0 {
                break;
            }
            for _ in 0..1000 { core::hint::spin_loop(); }
        }
    }

    /// Release GT force wake
    pub fn gt_force_wake_release(&mut self) {
        self.write_reg(regs::GT_FORCE_WAKE, 0xFFFF0000);
    }

    /// Get all power well info
    pub fn get_wells(&self) -> &[PowerWellInfo] {
        &self.wells
    }

    /// Format status for display
    pub fn format_status(&self) -> String {
        let mut s = String::from("Intel Power Wells:\n");
        s.push_str(&format!("  Platform: {:?}\n", self.platform));
        s.push_str(&format!("  Initialized: {}\n", self.initialized));
        s.push_str(&format!("  DC state: 0x{:X}\n", self.dc_state));
        s.push_str(&format!("  RC6 enabled: {}\n", self.rc6_enabled));

        s.push_str("  Power Wells:\n");
        for well in &self.wells {
            let refs = if well.ref_count > 0 {
                format!(" (refs: {})", well.ref_count)
            } else {
                String::new()
            };
            let always = if well.always_on { " [always-on]" } else { "" };
            s.push_str(&format!("    {}: {}{}{}\n",
                well.well.name(), well.state.as_str(), refs, always));
        }

        s
    }
}

/// Global power wells manager
static POWER_WELLS_INIT: AtomicBool = AtomicBool::new(false);
static POWER_WELLS: TicketSpinlock<Option<PowerWellsManager>> = TicketSpinlock::new(None);

/// Initialize power wells for an Intel GPU
pub fn init(mmio_base: u64, device_id: u16) {
    if POWER_WELLS_INIT.swap(true, Ordering::SeqCst) {
        return;
    }

    let mut manager = PowerWellsManager::new(mmio_base, device_id);
    manager.init();

    *POWER_WELLS.lock() = Some(manager);
}

/// Check if power wells are initialized
pub fn is_initialized() -> bool {
    POWER_WELLS_INIT.load(Ordering::SeqCst)
}

/// Enable a power well
pub fn enable(well: PowerWell) -> bool {
    let mut guard = POWER_WELLS.lock();
    if let Some(manager) = guard.as_mut() {
        manager.enable_power_well(well)
    } else {
        false
    }
}

/// Disable a power well
pub fn disable(well: PowerWell) -> bool {
    let mut guard = POWER_WELLS.lock();
    if let Some(manager) = guard.as_mut() {
        manager.disable_power_well(well)
    } else {
        false
    }
}

/// Get power well state
pub fn get_state(well: PowerWell) -> PowerWellState {
    let guard = POWER_WELLS.lock();
    if let Some(manager) = guard.as_ref() {
        manager.get_state(well).unwrap_or(PowerWellState::Unknown)
    } else {
        PowerWellState::Unknown
    }
}

/// Enable DC states
pub fn enable_dc_states(dc5: bool, dc6: bool) {
    let mut guard = POWER_WELLS.lock();
    if let Some(manager) = guard.as_mut() {
        manager.enable_dc_states(dc5, dc6);
    }
}

/// Enable RC6
pub fn enable_rc6() {
    let mut guard = POWER_WELLS.lock();
    if let Some(manager) = guard.as_mut() {
        manager.enable_rc6();
    }
}

/// Disable RC6
pub fn disable_rc6() {
    let mut guard = POWER_WELLS.lock();
    if let Some(manager) = guard.as_mut() {
        manager.disable_rc6();
    }
}

/// Force wake GT
pub fn gt_force_wake() {
    let mut guard = POWER_WELLS.lock();
    if let Some(manager) = guard.as_mut() {
        manager.gt_force_wake();
    }
}

/// Release GT force wake
pub fn gt_force_wake_release() {
    let mut guard = POWER_WELLS.lock();
    if let Some(manager) = guard.as_mut() {
        manager.gt_force_wake_release();
    }
}

/// Format status for display
pub fn format_status() -> String {
    let guard = POWER_WELLS.lock();
    if let Some(manager) = guard.as_ref() {
        manager.format_status()
    } else {
        String::from("Intel Power Wells: Not initialized\n")
    }
}
