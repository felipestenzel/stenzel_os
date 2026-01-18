//! AMD System Management Unit (SMU) Driver
//!
//! Provides interface to AMD's System Management Unit for:
//! - Power management
//! - Frequency/voltage scaling
//! - Thermal management
//! - Fan control
//! - Power limits (PPT, TDC, EDC)
//!
//! Supports SMU versions from Ryzen 1000 series through Ryzen 7000/9000

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use spin::Mutex;

/// SMU Message IDs for various operations
pub mod smu_msg {
    // Common messages (most SMU versions)
    pub const TEST_MESSAGE: u32 = 0x01;
    pub const GET_SMU_VERSION: u32 = 0x02;
    pub const GET_TABLE_VERSION: u32 = 0x03;
    pub const TRANSFER_TABLE_SMU2DRAM: u32 = 0x04;
    pub const TRANSFER_TABLE_DRAM2SMU: u32 = 0x05;

    // Power management
    pub const SET_PPT_LIMIT: u32 = 0x0A;
    pub const SET_TDC_LIMIT: u32 = 0x0B;
    pub const SET_EDC_LIMIT: u32 = 0x0C;
    pub const SET_SCALAR: u32 = 0x0D;
    pub const SET_POWER_PROFILE: u32 = 0x0E;

    // Frequency control
    pub const ENABLE_OC: u32 = 0x20;
    pub const DISABLE_OC: u32 = 0x21;
    pub const SET_ALL_CORE_FREQ_OFFSET: u32 = 0x22;
    pub const SET_PER_CORE_FREQ_OFFSET: u32 = 0x23;
    pub const SET_FCLK_FREQ: u32 = 0x24;
    pub const SET_MCLK_FREQ: u32 = 0x25;
    pub const SET_UCLK_FREQ: u32 = 0x26;

    // Thermal
    pub const SET_THERMAL_LIMIT: u32 = 0x30;
    pub const GET_THERMAL_LIMIT: u32 = 0x31;
    pub const ENABLE_THERMAL_THROTTLE: u32 = 0x32;
    pub const DISABLE_THERMAL_THROTTLE: u32 = 0x33;

    // Fan control
    pub const SET_FAN_CURVE: u32 = 0x40;
    pub const SET_FAN_SPEED_PERCENT: u32 = 0x41;
    pub const SET_FAN_SPEED_RPM: u32 = 0x42;
    pub const GET_FAN_SPEED_RPM: u32 = 0x43;
    pub const ENABLE_FAN_ZC: u32 = 0x44;  // Zero-crossing
    pub const DISABLE_FAN_ZC: u32 = 0x45;

    // Voltage
    pub const SET_CORE_VID: u32 = 0x50;
    pub const SET_SOC_VID: u32 = 0x51;
    pub const SET_GFX_VID: u32 = 0x52;
    pub const SET_VDDCR_SOC_OFFSET: u32 = 0x53;

    // STAPM (APU only)
    pub const SET_STAPM_LIMIT: u32 = 0x60;
    pub const SET_SLOW_PPT_LIMIT: u32 = 0x61;
    pub const SET_FAST_PPT_LIMIT: u32 = 0x62;
    pub const SET_SLOW_PPT_TIME: u32 = 0x63;
    pub const SET_STAPM_TIME: u32 = 0x64;

    // cTDP (configurable TDP)
    pub const SET_CTDP_LEVEL: u32 = 0x70;
    pub const GET_CTDP_LEVEL: u32 = 0x71;

    // PCIe
    pub const SET_PCIE_SPEED: u32 = 0x80;
    pub const SET_PCIE_WIDTH: u32 = 0x81;

    // Memory (for APU iGPU)
    pub const SET_VRAM_SIZE: u32 = 0x90;

    // Zen 4+ specific
    pub const GET_CURVE_OPTIMIZER: u32 = 0xA0;
    pub const SET_CURVE_OPTIMIZER: u32 = 0xA1;
}

/// SMU response codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SmuResponse {
    Ok = 0x01,
    Failed = 0xFF,
    UnknownCommand = 0xFE,
    CommandRejected = 0xFD,
    InvalidArgument = 0xFC,
    CommandBusy = 0xFB,
}

impl SmuResponse {
    pub fn from_u32(value: u32) -> Self {
        match value {
            0x01 => Self::Ok,
            0xFE => Self::UnknownCommand,
            0xFD => Self::CommandRejected,
            0xFC => Self::InvalidArgument,
            0xFB => Self::CommandBusy,
            _ => Self::Failed,
        }
    }

    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok)
    }
}

/// SMU versions by CPU generation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmuVersion {
    /// Zen 1 (Ryzen 1000, Threadripper 1000)
    Smu9,
    /// Zen+ (Ryzen 2000, Threadripper 2000)
    Smu10,
    /// Zen 2 (Ryzen 3000, Threadripper 3000)
    Smu11,
    /// Zen 3 (Ryzen 5000)
    Smu13_0_0,
    /// Zen 3+ / Rembrandt APU
    Smu13_0_4,
    /// Zen 4 (Ryzen 7000)
    Smu13_0_7,
    /// Zen 4c / Phoenix
    Smu13_0_8,
    /// Zen 5 (Ryzen 9000)
    Smu14,
    /// Unknown
    Unknown,
}

impl SmuVersion {
    pub fn from_version(major: u8, minor: u8) -> Self {
        match (major, minor) {
            (9, _) => Self::Smu9,
            (10, _) => Self::Smu10,
            (11, _) => Self::Smu11,
            (13, 0) => Self::Smu13_0_0,
            (13, 4) => Self::Smu13_0_4,
            (13, 7) => Self::Smu13_0_7,
            (13, 8) => Self::Smu13_0_8,
            (14, _) => Self::Smu14,
            _ => Self::Unknown,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Smu9 => "SMU 9.0 (Zen 1)",
            Self::Smu10 => "SMU 10.0 (Zen+)",
            Self::Smu11 => "SMU 11.0 (Zen 2)",
            Self::Smu13_0_0 => "SMU 13.0.0 (Zen 3)",
            Self::Smu13_0_4 => "SMU 13.0.4 (Zen 3+)",
            Self::Smu13_0_7 => "SMU 13.0.7 (Zen 4)",
            Self::Smu13_0_8 => "SMU 13.0.8 (Zen 4c)",
            Self::Smu14 => "SMU 14.0 (Zen 5)",
            Self::Unknown => "Unknown SMU",
        }
    }

    /// Check if this SMU version supports Curve Optimizer
    pub fn supports_curve_optimizer(&self) -> bool {
        matches!(self, Self::Smu13_0_0 | Self::Smu13_0_4 | Self::Smu13_0_7 | Self::Smu13_0_8 | Self::Smu14)
    }

    /// Check if this SMU version supports STAPM (APU)
    pub fn supports_stapm(&self) -> bool {
        matches!(self, Self::Smu11 | Self::Smu13_0_4 | Self::Smu13_0_8 | Self::Smu14)
    }
}

/// Power profile modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum PowerProfile {
    /// Balanced - default
    Balanced = 0,
    /// Quiet - prioritize low noise
    Quiet = 1,
    /// Performance - prioritize speed
    Performance = 2,
    /// Extreme Performance
    ExtremePerformance = 3,
    /// Power Saving
    PowerSaving = 4,
    /// Custom
    Custom = 5,
}

/// Fan curve point
#[derive(Debug, Clone, Copy)]
pub struct FanCurvePoint {
    pub temp_c: u8,
    pub fan_percent: u8,
}

/// Power limits structure
#[derive(Debug, Clone, Copy)]
pub struct PowerLimits {
    /// Package Power Tracking limit (Watts)
    pub ppt_limit: u32,
    /// Thermal Design Current limit (Amps)
    pub tdc_limit: u32,
    /// Electrical Design Current limit (Amps)
    pub edc_limit: u32,
    /// STAPM limit (Watts, APU only)
    pub stapm_limit: u32,
    /// Slow PPT limit (Watts, APU only)
    pub slow_ppt_limit: u32,
    /// Fast PPT limit (Watts, APU only)
    pub fast_ppt_limit: u32,
    /// Thermal limit (Celsius)
    pub thermal_limit: u32,
}

impl Default for PowerLimits {
    fn default() -> Self {
        Self {
            ppt_limit: 0,
            tdc_limit: 0,
            edc_limit: 0,
            stapm_limit: 0,
            slow_ppt_limit: 0,
            fast_ppt_limit: 0,
            thermal_limit: 95,
        }
    }
}

/// SMU telemetry data
#[derive(Debug, Clone, Copy, Default)]
pub struct SmuTelemetry {
    /// CPU Package power (Watts)
    pub cpu_power: f32,
    /// Socket power (Watts)
    pub socket_power: f32,
    /// SoC power (Watts)
    pub soc_power: f32,
    /// GFX/iGPU power (Watts)
    pub gfx_power: f32,
    /// CPU temperature (Celsius)
    pub cpu_temp: f32,
    /// SoC temperature (Celsius)
    pub soc_temp: f32,
    /// GFX temperature (Celsius)
    pub gfx_temp: f32,
    /// VRM temperature (Celsius)
    pub vrm_temp: f32,
    /// Current CPU frequency (MHz)
    pub cpu_freq: u32,
    /// Current Fabric frequency (MHz)
    pub fclk_freq: u32,
    /// Current Memory frequency (MHz)
    pub mclk_freq: u32,
    /// Current iGPU frequency (MHz)
    pub gfx_freq: u32,
    /// Current CPU voltage (V)
    pub cpu_voltage: f32,
    /// Current SoC voltage (V)
    pub soc_voltage: f32,
    /// Fan speed (RPM)
    pub fan_rpm: u32,
    /// CPU utilization (%)
    pub cpu_util: u8,
    /// iGPU utilization (%)
    pub gfx_util: u8,
}

/// Register offsets for SMU communication
mod regs {
    // MP1 (Message Port 1) registers - for CPU SMU
    pub const MP1_SMN_C2PMSG_90: u32 = 0x03B10528;  // Message response
    pub const MP1_SMN_C2PMSG_82: u32 = 0x03B10508;  // Message argument
    pub const MP1_SMN_C2PMSG_66: u32 = 0x03B10498;  // Message ID

    // Alternative MP1 register set (some platforms)
    pub const MP1_SMN_C2PMSG_75: u32 = 0x03B104AC;
    pub const MP1_SMN_C2PMSG_76: u32 = 0x03B104B0;
    pub const MP1_SMN_C2PMSG_77: u32 = 0x03B104B4;

    // RSMU (Root System Management Unit) registers - for GPU SMU
    pub const RSMU_SMN_C2PMSG_0: u32 = 0x03B10900;
    pub const RSMU_SMN_C2PMSG_1: u32 = 0x03B10904;

    // SMN (System Management Network) index/data registers
    pub const SMN_INDEX: u32 = 0x60;
    pub const SMN_DATA: u32 = 0x64;
}

/// AMD SMU driver
pub struct AmdSmu {
    /// PCI device (bus, device, function)
    pci_device: (u8, u8, u8),
    /// SMU version
    version: SmuVersion,
    /// SMU firmware version
    fw_version: u32,
    /// Current power limits
    power_limits: PowerLimits,
    /// Current power profile
    power_profile: PowerProfile,
    /// Is APU (has integrated graphics)
    is_apu: bool,
    /// Curve optimizer values per core
    curve_optimizer: Vec<i8>,
    /// Number of CPU cores
    num_cores: u32,
    /// Fan curve points
    fan_curve: Vec<FanCurvePoint>,
    /// OC enabled
    oc_enabled: bool,
    /// Initialized
    initialized: bool,
}

impl AmdSmu {
    /// Create a new SMU instance
    pub fn new(pci_device: (u8, u8, u8), is_apu: bool, num_cores: u32) -> Self {
        Self {
            pci_device,
            version: SmuVersion::Unknown,
            fw_version: 0,
            power_limits: PowerLimits::default(),
            power_profile: PowerProfile::Balanced,
            is_apu,
            curve_optimizer: vec![0i8; num_cores as usize],
            num_cores,
            fan_curve: Vec::new(),
            oc_enabled: false,
            initialized: false,
        }
    }

    /// Read from SMN (System Management Network)
    fn smn_read(&self, addr: u32) -> u32 {
        let (bus, dev, func) = self.pci_device;

        // Write address to SMN index
        crate::drivers::pci::write_u32(bus, dev, func, regs::SMN_INDEX as u8, addr);
        // Read data from SMN data
        crate::drivers::pci::read_u32(bus, dev, func, regs::SMN_DATA as u8)
    }

    /// Write to SMN
    fn smn_write(&self, addr: u32, value: u32) {
        let (bus, dev, func) = self.pci_device;

        // Write address to SMN index
        crate::drivers::pci::write_u32(bus, dev, func, regs::SMN_INDEX as u8, addr);
        // Write data to SMN data
        crate::drivers::pci::write_u32(bus, dev, func, regs::SMN_DATA as u8, value);
    }

    /// Send message to SMU
    fn send_message(&self, msg: u32, arg: u32) -> Result<u32, SmuResponse> {
        // Wait for SMU to be ready
        let mut timeout = 100_000;
        while timeout > 0 {
            let resp = self.smn_read(regs::MP1_SMN_C2PMSG_90);
            if resp != 0 {
                break;
            }
            timeout -= 1;
        }

        if timeout == 0 {
            return Err(SmuResponse::CommandBusy);
        }

        // Clear response
        self.smn_write(regs::MP1_SMN_C2PMSG_90, 0);

        // Write argument
        self.smn_write(regs::MP1_SMN_C2PMSG_82, arg);

        // Send message
        self.smn_write(regs::MP1_SMN_C2PMSG_66, msg);

        // Wait for response
        timeout = 100_000;
        let mut response = 0u32;
        while timeout > 0 {
            response = self.smn_read(regs::MP1_SMN_C2PMSG_90);
            if response != 0 {
                break;
            }
            timeout -= 1;
        }

        let resp = SmuResponse::from_u32(response);
        if resp.is_ok() {
            // Read result from argument register
            let result = self.smn_read(regs::MP1_SMN_C2PMSG_82);
            Ok(result)
        } else {
            Err(resp)
        }
    }

    /// Initialize SMU
    pub fn init(&mut self) -> bool {
        // Test SMU communication
        if let Ok(_) = self.send_message(smu_msg::TEST_MESSAGE, 0) {
            // Get SMU version
            if let Ok(version) = self.send_message(smu_msg::GET_SMU_VERSION, 0) {
                self.fw_version = version;
                let major = ((version >> 24) & 0xFF) as u8;
                let minor = ((version >> 16) & 0xFF) as u8;
                self.version = SmuVersion::from_version(major, minor);

                crate::kprintln!("amd_smu: {} detected (FW {}.{}.{}.{})",
                    self.version.name(),
                    major, minor,
                    (version >> 8) & 0xFF,
                    version & 0xFF
                );

                // Read current power limits
                self.read_power_limits();

                // Setup default fan curve
                self.setup_default_fan_curve();

                self.initialized = true;
                return true;
            }
        }

        crate::kprintln!("amd_smu: failed to initialize SMU");
        false
    }

    /// Read current power limits
    fn read_power_limits(&mut self) {
        // These would normally be read from the SMU metrics table
        // For now, set reasonable defaults based on platform
        if self.is_apu {
            self.power_limits = PowerLimits {
                ppt_limit: 65,
                tdc_limit: 60,
                edc_limit: 90,
                stapm_limit: 54,
                slow_ppt_limit: 54,
                fast_ppt_limit: 80,
                thermal_limit: 95,
            };
        } else {
            self.power_limits = PowerLimits {
                ppt_limit: 142,
                tdc_limit: 110,
                edc_limit: 170,
                stapm_limit: 0,
                slow_ppt_limit: 0,
                fast_ppt_limit: 0,
                thermal_limit: 95,
            };
        }
    }

    /// Setup default fan curve
    fn setup_default_fan_curve(&mut self) {
        self.fan_curve = vec![
            FanCurvePoint { temp_c: 30, fan_percent: 0 },
            FanCurvePoint { temp_c: 40, fan_percent: 20 },
            FanCurvePoint { temp_c: 50, fan_percent: 30 },
            FanCurvePoint { temp_c: 60, fan_percent: 45 },
            FanCurvePoint { temp_c: 70, fan_percent: 60 },
            FanCurvePoint { temp_c: 80, fan_percent: 80 },
            FanCurvePoint { temp_c: 90, fan_percent: 100 },
        ];
    }

    /// Set PPT limit
    pub fn set_ppt_limit(&mut self, watts: u32) -> bool {
        if let Ok(_) = self.send_message(smu_msg::SET_PPT_LIMIT, watts * 1000) {
            self.power_limits.ppt_limit = watts;
            true
        } else {
            false
        }
    }

    /// Set TDC limit
    pub fn set_tdc_limit(&mut self, amps: u32) -> bool {
        if let Ok(_) = self.send_message(smu_msg::SET_TDC_LIMIT, amps * 1000) {
            self.power_limits.tdc_limit = amps;
            true
        } else {
            false
        }
    }

    /// Set EDC limit
    pub fn set_edc_limit(&mut self, amps: u32) -> bool {
        if let Ok(_) = self.send_message(smu_msg::SET_EDC_LIMIT, amps * 1000) {
            self.power_limits.edc_limit = amps;
            true
        } else {
            false
        }
    }

    /// Set thermal limit
    pub fn set_thermal_limit(&mut self, temp_c: u32) -> bool {
        if let Ok(_) = self.send_message(smu_msg::SET_THERMAL_LIMIT, temp_c) {
            self.power_limits.thermal_limit = temp_c;
            true
        } else {
            false
        }
    }

    /// Set STAPM limit (APU only)
    pub fn set_stapm_limit(&mut self, watts: u32) -> bool {
        if !self.is_apu || !self.version.supports_stapm() {
            return false;
        }

        if let Ok(_) = self.send_message(smu_msg::SET_STAPM_LIMIT, watts * 1000) {
            self.power_limits.stapm_limit = watts;
            true
        } else {
            false
        }
    }

    /// Set slow PPT limit (APU only)
    pub fn set_slow_ppt_limit(&mut self, watts: u32) -> bool {
        if !self.is_apu || !self.version.supports_stapm() {
            return false;
        }

        if let Ok(_) = self.send_message(smu_msg::SET_SLOW_PPT_LIMIT, watts * 1000) {
            self.power_limits.slow_ppt_limit = watts;
            true
        } else {
            false
        }
    }

    /// Set fast PPT limit (APU only)
    pub fn set_fast_ppt_limit(&mut self, watts: u32) -> bool {
        if !self.is_apu || !self.version.supports_stapm() {
            return false;
        }

        if let Ok(_) = self.send_message(smu_msg::SET_FAST_PPT_LIMIT, watts * 1000) {
            self.power_limits.fast_ppt_limit = watts;
            true
        } else {
            false
        }
    }

    /// Enable overclocking
    pub fn enable_oc(&mut self) -> bool {
        if let Ok(_) = self.send_message(smu_msg::ENABLE_OC, 0) {
            self.oc_enabled = true;
            true
        } else {
            false
        }
    }

    /// Disable overclocking
    pub fn disable_oc(&mut self) -> bool {
        if let Ok(_) = self.send_message(smu_msg::DISABLE_OC, 0) {
            self.oc_enabled = false;
            true
        } else {
            false
        }
    }

    /// Set all-core frequency offset (MHz)
    pub fn set_all_core_freq_offset(&mut self, offset_mhz: i32) -> bool {
        if !self.oc_enabled {
            return false;
        }

        let value = if offset_mhz < 0 {
            (0x80000000u32 | (-offset_mhz as u32))
        } else {
            offset_mhz as u32
        };

        self.send_message(smu_msg::SET_ALL_CORE_FREQ_OFFSET, value).is_ok()
    }

    /// Set per-core frequency offset (Curve Optimizer style)
    pub fn set_curve_optimizer(&mut self, core: u32, offset: i8) -> bool {
        if !self.version.supports_curve_optimizer() {
            return false;
        }

        if core >= self.num_cores {
            return false;
        }

        // Value format: core in upper 8 bits, offset in lower 8 bits (signed)
        let value = ((core & 0xFF) << 24) | ((offset as u8) as u32);

        if self.send_message(smu_msg::SET_CURVE_OPTIMIZER, value).is_ok() {
            self.curve_optimizer[core as usize] = offset;
            true
        } else {
            false
        }
    }

    /// Set power profile
    pub fn set_power_profile(&mut self, profile: PowerProfile) -> bool {
        if let Ok(_) = self.send_message(smu_msg::SET_POWER_PROFILE, profile as u32) {
            self.power_profile = profile;
            true
        } else {
            false
        }
    }

    /// Set fan speed percentage
    pub fn set_fan_speed_percent(&mut self, percent: u8) -> bool {
        let value = percent.min(100) as u32;
        self.send_message(smu_msg::SET_FAN_SPEED_PERCENT, value).is_ok()
    }

    /// Get fan speed RPM
    pub fn get_fan_speed_rpm(&self) -> Option<u32> {
        self.send_message(smu_msg::GET_FAN_SPEED_RPM, 0).ok()
    }

    /// Get current power limits
    pub fn get_power_limits(&self) -> &PowerLimits {
        &self.power_limits
    }

    /// Get current power profile
    pub fn get_power_profile(&self) -> PowerProfile {
        self.power_profile
    }

    /// Get SMU version
    pub fn get_version(&self) -> SmuVersion {
        self.version
    }

    /// Get firmware version
    pub fn get_fw_version(&self) -> u32 {
        self.fw_version
    }

    /// Get curve optimizer values
    pub fn get_curve_optimizer(&self) -> &[i8] {
        &self.curve_optimizer
    }

    /// Is OC enabled
    pub fn is_oc_enabled(&self) -> bool {
        self.oc_enabled
    }

    /// Is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Format status for display
    pub fn format_status(&self) -> String {
        let limits = &self.power_limits;
        let mut s = format!("AMD SMU {}\n", self.version.name());
        s += &format!("  Firmware: {:08X}\n", self.fw_version);
        s += &format!("  OC: {}\n", if self.oc_enabled { "enabled" } else { "disabled" });
        s += &format!("  Profile: {:?}\n", self.power_profile);
        s += &format!("  Power Limits:\n");
        s += &format!("    PPT: {}W, TDC: {}A, EDC: {}A\n",
            limits.ppt_limit, limits.tdc_limit, limits.edc_limit);
        if self.is_apu {
            s += &format!("    STAPM: {}W, Slow PPT: {}W, Fast PPT: {}W\n",
                limits.stapm_limit, limits.slow_ppt_limit, limits.fast_ppt_limit);
        }
        s += &format!("    Thermal: {}°C\n", limits.thermal_limit);
        s
    }
}

// Global SMU instance
static AMD_SMU: Mutex<Option<AmdSmu>> = Mutex::new(None);

/// Initialize AMD SMU
pub fn init() {
    // Find AMD root complex (host bridge)
    // PCI 00:00.0 is typically the host bridge
    let vendor = crate::drivers::pci::read_u16(0, 0, 0, 0);
    let device = crate::drivers::pci::read_u16(0, 0, 0, 2);

    // Check for AMD vendor ID
    if vendor != 0x1022 {
        crate::kprintln!("amd_smu: no AMD platform detected");
        return;
    }

    // Determine if APU based on device ID
    let is_apu = matches!(device,
        0x1630 | // Renoir
        0x1480 | // Matisse
        0x1440 | // Raven
        0x15D0 | // Raven 2
        0x1450 | // Raven Ridge
        0x166A | // Cezanne
        0x14B5 | // Phoenix
        0x14E8   // Strix
    );

    // Get core count from CPUID leaf 0x80000008
    // ECX bits 7:0 = number of cores - 1
    let num_cores = unsafe {
        let ecx: u32;
        core::arch::asm!(
            "push rbx",
            "mov eax, 0x80000008",
            "cpuid",
            "pop rbx",
            out("ecx") ecx,
            out("eax") _,
            out("edx") _,
            options(nostack, preserves_flags)
        );
        ((ecx & 0xFF) + 1)
    };

    let mut smu = AmdSmu::new((0, 0, 0), is_apu, num_cores);

    if smu.init() {
        *AMD_SMU.lock() = Some(smu);
    }
}

/// Get current SMU status
pub fn format_status() -> Option<String> {
    AMD_SMU.lock().as_ref().map(|smu| smu.format_status())
}

/// Set PPT limit
pub fn set_ppt_limit(watts: u32) -> bool {
    AMD_SMU.lock().as_mut().map(|smu| smu.set_ppt_limit(watts)).unwrap_or(false)
}

/// Set TDC limit
pub fn set_tdc_limit(amps: u32) -> bool {
    AMD_SMU.lock().as_mut().map(|smu| smu.set_tdc_limit(amps)).unwrap_or(false)
}

/// Set EDC limit
pub fn set_edc_limit(amps: u32) -> bool {
    AMD_SMU.lock().as_mut().map(|smu| smu.set_edc_limit(amps)).unwrap_or(false)
}

/// Set thermal limit
pub fn set_thermal_limit(temp_c: u32) -> bool {
    AMD_SMU.lock().as_mut().map(|smu| smu.set_thermal_limit(temp_c)).unwrap_or(false)
}

/// Set STAPM limit (APU only)
pub fn set_stapm_limit(watts: u32) -> bool {
    AMD_SMU.lock().as_mut().map(|smu| smu.set_stapm_limit(watts)).unwrap_or(false)
}

/// Set power profile
pub fn set_power_profile(profile: PowerProfile) -> bool {
    AMD_SMU.lock().as_mut().map(|smu| smu.set_power_profile(profile)).unwrap_or(false)
}

/// Enable overclocking
pub fn enable_oc() -> bool {
    AMD_SMU.lock().as_mut().map(|smu| smu.enable_oc()).unwrap_or(false)
}

/// Disable overclocking
pub fn disable_oc() -> bool {
    AMD_SMU.lock().as_mut().map(|smu| smu.disable_oc()).unwrap_or(false)
}

/// Set all-core frequency offset
pub fn set_freq_offset(offset_mhz: i32) -> bool {
    AMD_SMU.lock().as_mut().map(|smu| smu.set_all_core_freq_offset(offset_mhz)).unwrap_or(false)
}

/// Set curve optimizer for a specific core
pub fn set_curve_optimizer(core: u32, offset: i8) -> bool {
    AMD_SMU.lock().as_mut().map(|smu| smu.set_curve_optimizer(core, offset)).unwrap_or(false)
}

/// Get fan speed RPM
pub fn get_fan_speed() -> Option<u32> {
    AMD_SMU.lock().as_ref().and_then(|smu| smu.get_fan_speed_rpm())
}

/// Set fan speed percentage
pub fn set_fan_speed(percent: u8) -> bool {
    AMD_SMU.lock().as_mut().map(|smu| smu.set_fan_speed_percent(percent)).unwrap_or(false)
}

/// Check if SMU is initialized
pub fn is_initialized() -> bool {
    AMD_SMU.lock().as_ref().map(|smu| smu.is_initialized()).unwrap_or(false)
}
