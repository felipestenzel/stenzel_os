//! AMD PowerPlay - GPU Power Management Driver
//!
//! Handles Dynamic Power Management (DPM) for AMD GPUs including:
//! - DPM state management (performance levels)
//! - Fan curve control and thermal management
//! - Voltage/frequency scaling
//! - Power limits for GPU subsystems

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use spin::Mutex;

/// PowerPlay MMIO register offsets
pub mod regs {
    // SMC registers
    pub const SMC_IND_INDEX: u32 = 0x200;
    pub const SMC_IND_DATA: u32 = 0x204;

    // CG (Clock Gating) registers
    pub const CG_SPLL_FUNC_CNTL: u32 = 0xC0500140;
    pub const CG_SPLL_FUNC_CNTL_2: u32 = 0xC0500144;
    pub const CG_SPLL_FUNC_CNTL_3: u32 = 0xC0500148;
    pub const CG_SPLL_FUNC_CNTL_4: u32 = 0xC050014C;

    // DPM registers
    pub const DPM_TABLE_475: u32 = 0x3F000;
    pub const SOFT_REGISTERS_TABLE_28: u32 = 0x3F4C0;

    // SMU message interface
    pub const MP1_SMN_C2PMSG_90: u32 = 0x3B10A58;
    pub const MP1_SMN_C2PMSG_82: u32 = 0x3B10A48;
    pub const MP1_SMN_C2PMSG_66: u32 = 0x3B10908;

    // Fan control
    pub const CG_FDO_CTRL0: u32 = 0xC0300064;
    pub const CG_FDO_CTRL1: u32 = 0xC0300068;
    pub const CG_FDO_CTRL2: u32 = 0xC030006C;
    pub const CG_THERMAL_INT_CTRL: u32 = 0xC0300090;
    pub const CG_THERMAL_STATUS: u32 = 0xC0300008;

    // THM (Thermal) registers
    pub const THM_TCON_CUR_TMP: u32 = 0x59800;
    pub const THM_BACO_CNTL: u32 = 0x59870;

    // GPU metrics
    pub const GPU_METRICS_BASE: u32 = 0x50000;
}

/// DPM Performance Levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DpmLevel {
    /// Lowest power state
    Dpm0 = 0,
    Dpm1 = 1,
    Dpm2 = 2,
    Dpm3 = 3,
    Dpm4 = 4,
    Dpm5 = 5,
    Dpm6 = 6,
    /// Highest performance state
    Dpm7 = 7,
}

/// Power profile modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerProfile {
    Bootup,
    ThreeDFullScreen,
    PowerSaving,
    Video,
    VR,
    Compute,
    Custom,
}

/// Fan control mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FanControlMode {
    /// No fan control
    None,
    /// Automatic thermal-based control
    Auto,
    /// Manual PWM control
    Manual,
}

/// Clock type for DPM
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockType {
    Gfxclk,     // Graphics core clock
    Socclk,     // System-on-chip clock
    Uclk,       // Unified memory clock
    Fclk,       // Fabric/Infinity Fabric clock
    Dclk,       // Video decode clock
    Vclk,       // Video encode clock
    Dcefclk,    // Display controller clock
    Dispclk,    // Display clock
    Pixclk,     // Pixel clock
    Phyclk,     // PHY clock
}

/// DPM state entry
#[derive(Debug, Clone, Copy)]
pub struct DpmState {
    pub level: DpmLevel,
    pub enabled: bool,
    pub sclk_mhz: u32,      // Shader clock
    pub mclk_mhz: u32,      // Memory clock
    pub vddc_mv: u32,       // Core voltage
    pub vddci_mv: u32,      // I/O voltage
    pub power_mw: u32,      // Power draw estimate
}

/// Fan speed table entry
#[derive(Debug, Clone, Copy)]
pub struct FanTableEntry {
    pub temp_c: i32,
    pub pwm_percent: u8,
}

/// GPU thermal zone
#[derive(Debug, Clone, Copy)]
pub struct ThermalZone {
    pub edge_temp: i32,
    pub junction_temp: i32,
    pub memory_temp: i32,
    pub hotspot_temp: i32,
}

/// Power limits structure
#[derive(Debug, Clone, Copy)]
pub struct GpuPowerLimits {
    pub tdp_w: u32,
    pub max_tdp_w: u32,
    pub min_tdp_w: u32,
    pub default_tdp_w: u32,
    pub gfx_power_w: u32,
    pub soc_power_w: u32,
}

/// GPU generation for PowerPlay
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuGeneration {
    Polaris,    // GCN 4
    Vega,       // GCN 5
    Navi1x,     // RDNA 1
    Navi2x,     // RDNA 2
    Navi3x,     // RDNA 3
}

/// SMU message types for GPU
pub mod smu_msg {
    pub const TEST_MESSAGE: u16 = 0x01;
    pub const GET_SMU_VERSION: u16 = 0x02;
    pub const GET_DRIVER_IF_VERSION: u16 = 0x03;
    pub const SET_ALLOWED_FEATURE_MASK: u16 = 0x04;
    pub const GET_ENABLED_FEATURE_MASK: u16 = 0x05;
    pub const SET_WORKLOAD_MASK: u16 = 0x06;
    pub const SET_POWER_PROFILE: u16 = 0x07;
    pub const SET_FAN_CONTROL_MODE: u16 = 0x08;
    pub const SET_FAN_SPEED_PWM: u16 = 0x09;
    pub const SET_FAN_SPEED_RPM: u16 = 0x0A;
    pub const SET_FAN_TEMP_INPUT: u16 = 0x0B;
    pub const GET_FAN_CONTROL_MODE: u16 = 0x0C;
    pub const GET_FAN_SPEED_PWM: u16 = 0x0D;
    pub const GET_FAN_SPEED_RPM: u16 = 0x0E;
    pub const SET_HARD_MIN_GFXCLK: u16 = 0x0F;
    pub const SET_SOFT_MAX_GFXCLK: u16 = 0x10;
    pub const SET_HARD_MIN_SOCCLK: u16 = 0x11;
    pub const SET_SOFT_MAX_SOCCLK: u16 = 0x12;
    pub const SET_HARD_MIN_UCLK: u16 = 0x13;
    pub const SET_SOFT_MAX_UCLK: u16 = 0x14;
    pub const SET_HARD_MIN_FCLK: u16 = 0x15;
    pub const SET_SOFT_MAX_FCLK: u16 = 0x16;
    pub const GET_DPM_CLOCK_FREQ: u16 = 0x17;
    pub const GET_DPM_CLOCK_TABLE: u16 = 0x18;
    pub const SET_DPM_LEVEL: u16 = 0x19;
    pub const GET_CURRENT_POWER: u16 = 0x1A;
    pub const GET_GPU_METRICS: u16 = 0x1B;
    pub const ENABLE_GFX_OFF: u16 = 0x1C;
    pub const DISABLE_GFX_OFF: u16 = 0x1D;
    pub const SET_POWER_LIMIT: u16 = 0x1E;
    pub const GET_POWER_LIMIT: u16 = 0x1F;
    pub const SET_OVERDRIVE_GFX: u16 = 0x20;
    pub const SET_OVERDRIVE_MEM: u16 = 0x21;
    pub const GET_OVERDRIVE_TABLE: u16 = 0x22;
    pub const SET_OVERDRIVE_TABLE: u16 = 0x23;
    pub const ENTER_BACO: u16 = 0x24;
    pub const EXIT_BACO: u16 = 0x25;
    pub const ALLOW_GFXOFF: u16 = 0x26;
    pub const DISALLOW_GFXOFF: u16 = 0x27;
    pub const GFXOFF_CONTROL: u16 = 0x28;
    pub const SET_PEAK_GFXCLK: u16 = 0x29;
    pub const SET_PEAK_UCLK: u16 = 0x2A;
}

/// AMD PowerPlay driver state
pub struct AmdPowerPlay {
    /// MMIO base address
    mmio_base: u64,
    /// PCI device location
    pci_device: (u8, u8, u8),
    /// GPU generation
    generation: GpuGeneration,
    /// DPM states
    dpm_states: Vec<DpmState>,
    /// Current DPM level
    current_level: DpmLevel,
    /// Power profile
    power_profile: PowerProfile,
    /// Fan control mode
    fan_mode: FanControlMode,
    /// Fan curve
    fan_curve: Vec<FanTableEntry>,
    /// Power limits
    power_limits: GpuPowerLimits,
    /// Features enabled mask
    features_enabled: u64,
    /// GfxOff enabled
    gfxoff_enabled: bool,
    /// Is initialized
    initialized: bool,
}

impl AmdPowerPlay {
    /// Create new PowerPlay instance
    pub fn new(mmio_base: u64, pci_device: (u8, u8, u8), generation: GpuGeneration) -> Self {
        Self {
            mmio_base,
            pci_device,
            generation,
            dpm_states: Vec::new(),
            current_level: DpmLevel::Dpm0,
            power_profile: PowerProfile::Bootup,
            fan_mode: FanControlMode::Auto,
            fan_curve: Vec::new(),
            power_limits: GpuPowerLimits {
                tdp_w: 0,
                max_tdp_w: 0,
                min_tdp_w: 0,
                default_tdp_w: 0,
                gfx_power_w: 0,
                soc_power_w: 0,
            },
            features_enabled: 0,
            gfxoff_enabled: false,
            initialized: false,
        }
    }

    /// Initialize PowerPlay
    pub fn init(&mut self) -> Result<(), &'static str> {
        crate::kprintln!("[PowerPlay] Initializing for {:?}", self.generation);

        // Check SMU communication
        if !self.check_smu_ready() {
            return Err("SMU not ready");
        }

        // Get SMU version
        let smu_version = self.get_smu_version();
        crate::kprintln!("[PowerPlay] SMU version: 0x{:08X}", smu_version);

        // Initialize DPM states based on generation
        self.init_dpm_states()?;

        // Initialize fan control
        self.init_fan_control()?;

        // Get power limits
        self.get_power_limits_from_smu()?;

        // Enable required features
        self.enable_features()?;

        // Set initial power profile
        self.set_power_profile(PowerProfile::Bootup)?;

        self.initialized = true;
        crate::kprintln!("[PowerPlay] Initialization complete");

        Ok(())
    }

    /// Check if SMU is ready for commands
    fn check_smu_ready(&self) -> bool {
        // Read SMU response register
        let response = self.smc_read(regs::MP1_SMN_C2PMSG_90);
        response == 1 || response == 0
    }

    /// Get SMU version
    fn get_smu_version(&self) -> u32 {
        self.send_smu_msg(smu_msg::GET_SMU_VERSION, 0).unwrap_or(0)
    }

    /// Send message to SMU
    fn send_smu_msg(&self, msg: u16, param: u32) -> Result<u32, &'static str> {
        // Wait for SMU ready
        let mut timeout = 10000;
        while timeout > 0 {
            let resp = self.smc_read(regs::MP1_SMN_C2PMSG_90);
            if resp == 1 {
                break;
            }
            timeout -= 1;
            for _ in 0..100 {
                core::hint::spin_loop();
            }
        }

        if timeout == 0 {
            return Err("SMU timeout waiting for ready");
        }

        // Clear response register
        self.smc_write(regs::MP1_SMN_C2PMSG_90, 0);

        // Write parameter
        self.smc_write(regs::MP1_SMN_C2PMSG_82, param);

        // Send message
        self.smc_write(regs::MP1_SMN_C2PMSG_66, msg as u32);

        // Wait for response
        timeout = 10000;
        while timeout > 0 {
            let resp = self.smc_read(regs::MP1_SMN_C2PMSG_90);
            if resp != 0 {
                if resp == 1 {
                    // Read result
                    return Ok(self.smc_read(regs::MP1_SMN_C2PMSG_82));
                } else {
                    return Err("SMU message failed");
                }
            }
            timeout -= 1;
            for _ in 0..100 {
                core::hint::spin_loop();
            }
        }

        Err("SMU timeout waiting for response")
    }

    /// Read SMC register via MMIO
    fn smc_read(&self, reg: u32) -> u32 {
        unsafe {
            let ptr = (self.mmio_base + regs::SMC_IND_INDEX as u64) as *mut u32;
            core::ptr::write_volatile(ptr, reg);
            let data_ptr = (self.mmio_base + regs::SMC_IND_DATA as u64) as *const u32;
            core::ptr::read_volatile(data_ptr)
        }
    }

    /// Write SMC register via MMIO
    fn smc_write(&self, reg: u32, value: u32) {
        unsafe {
            let ptr = (self.mmio_base + regs::SMC_IND_INDEX as u64) as *mut u32;
            core::ptr::write_volatile(ptr, reg);
            let data_ptr = (self.mmio_base + regs::SMC_IND_DATA as u64) as *mut u32;
            core::ptr::write_volatile(data_ptr, value);
        }
    }

    /// Read MMIO register directly
    fn mmio_read(&self, offset: u32) -> u32 {
        unsafe {
            let ptr = (self.mmio_base + offset as u64) as *const u32;
            core::ptr::read_volatile(ptr)
        }
    }

    /// Write MMIO register directly
    fn mmio_write(&self, offset: u32, value: u32) {
        unsafe {
            let ptr = (self.mmio_base + offset as u64) as *mut u32;
            core::ptr::write_volatile(ptr, value);
        }
    }

    /// Initialize DPM states
    fn init_dpm_states(&mut self) -> Result<(), &'static str> {
        self.dpm_states.clear();

        // Get DPM table from SMU or use defaults based on generation
        match self.generation {
            GpuGeneration::Polaris => self.init_polaris_dpm(),
            GpuGeneration::Vega => self.init_vega_dpm(),
            GpuGeneration::Navi1x => self.init_navi1x_dpm(),
            GpuGeneration::Navi2x => self.init_navi2x_dpm(),
            GpuGeneration::Navi3x => self.init_navi3x_dpm(),
        }

        Ok(())
    }

    /// Initialize Polaris DPM states
    fn init_polaris_dpm(&mut self) {
        // Polaris typical DPM states
        self.dpm_states = vec![
            DpmState { level: DpmLevel::Dpm0, enabled: true, sclk_mhz: 300, mclk_mhz: 300, vddc_mv: 750, vddci_mv: 850, power_mw: 15000 },
            DpmState { level: DpmLevel::Dpm1, enabled: true, sclk_mhz: 600, mclk_mhz: 600, vddc_mv: 800, vddci_mv: 900, power_mw: 35000 },
            DpmState { level: DpmLevel::Dpm2, enabled: true, sclk_mhz: 900, mclk_mhz: 900, vddc_mv: 900, vddci_mv: 950, power_mw: 65000 },
            DpmState { level: DpmLevel::Dpm3, enabled: true, sclk_mhz: 1000, mclk_mhz: 1500, vddc_mv: 950, vddci_mv: 1000, power_mw: 85000 },
            DpmState { level: DpmLevel::Dpm4, enabled: true, sclk_mhz: 1100, mclk_mhz: 1750, vddc_mv: 1000, vddci_mv: 1050, power_mw: 110000 },
            DpmState { level: DpmLevel::Dpm5, enabled: true, sclk_mhz: 1200, mclk_mhz: 2000, vddc_mv: 1050, vddci_mv: 1100, power_mw: 130000 },
            DpmState { level: DpmLevel::Dpm6, enabled: true, sclk_mhz: 1300, mclk_mhz: 2000, vddc_mv: 1100, vddci_mv: 1100, power_mw: 145000 },
            DpmState { level: DpmLevel::Dpm7, enabled: true, sclk_mhz: 1400, mclk_mhz: 2000, vddc_mv: 1150, vddci_mv: 1100, power_mw: 160000 },
        ];
    }

    /// Initialize Vega DPM states
    fn init_vega_dpm(&mut self) {
        // Vega typical DPM states
        self.dpm_states = vec![
            DpmState { level: DpmLevel::Dpm0, enabled: true, sclk_mhz: 400, mclk_mhz: 167, vddc_mv: 800, vddci_mv: 850, power_mw: 20000 },
            DpmState { level: DpmLevel::Dpm1, enabled: true, sclk_mhz: 700, mclk_mhz: 500, vddc_mv: 850, vddci_mv: 900, power_mw: 45000 },
            DpmState { level: DpmLevel::Dpm2, enabled: true, sclk_mhz: 1000, mclk_mhz: 700, vddc_mv: 900, vddci_mv: 950, power_mw: 80000 },
            DpmState { level: DpmLevel::Dpm3, enabled: true, sclk_mhz: 1200, mclk_mhz: 800, vddc_mv: 950, vddci_mv: 1000, power_mw: 120000 },
            DpmState { level: DpmLevel::Dpm4, enabled: true, sclk_mhz: 1350, mclk_mhz: 900, vddc_mv: 1000, vddci_mv: 1050, power_mw: 160000 },
            DpmState { level: DpmLevel::Dpm5, enabled: true, sclk_mhz: 1450, mclk_mhz: 945, vddc_mv: 1050, vddci_mv: 1100, power_mw: 200000 },
            DpmState { level: DpmLevel::Dpm6, enabled: true, sclk_mhz: 1550, mclk_mhz: 945, vddc_mv: 1100, vddci_mv: 1100, power_mw: 240000 },
            DpmState { level: DpmLevel::Dpm7, enabled: true, sclk_mhz: 1630, mclk_mhz: 945, vddc_mv: 1150, vddci_mv: 1100, power_mw: 295000 },
        ];
    }

    /// Initialize Navi 1x DPM states
    fn init_navi1x_dpm(&mut self) {
        // Navi 10/14 typical DPM states (RDNA 1)
        self.dpm_states = vec![
            DpmState { level: DpmLevel::Dpm0, enabled: true, sclk_mhz: 500, mclk_mhz: 100, vddc_mv: 750, vddci_mv: 800, power_mw: 10000 },
            DpmState { level: DpmLevel::Dpm1, enabled: true, sclk_mhz: 800, mclk_mhz: 625, vddc_mv: 800, vddci_mv: 850, power_mw: 35000 },
            DpmState { level: DpmLevel::Dpm2, enabled: true, sclk_mhz: 1100, mclk_mhz: 875, vddc_mv: 850, vddci_mv: 900, power_mw: 70000 },
            DpmState { level: DpmLevel::Dpm3, enabled: true, sclk_mhz: 1400, mclk_mhz: 875, vddc_mv: 900, vddci_mv: 950, power_mw: 110000 },
            DpmState { level: DpmLevel::Dpm4, enabled: true, sclk_mhz: 1600, mclk_mhz: 875, vddc_mv: 950, vddci_mv: 1000, power_mw: 145000 },
            DpmState { level: DpmLevel::Dpm5, enabled: true, sclk_mhz: 1750, mclk_mhz: 875, vddc_mv: 1000, vddci_mv: 1050, power_mw: 175000 },
            DpmState { level: DpmLevel::Dpm6, enabled: true, sclk_mhz: 1850, mclk_mhz: 875, vddc_mv: 1050, vddci_mv: 1100, power_mw: 200000 },
            DpmState { level: DpmLevel::Dpm7, enabled: true, sclk_mhz: 1905, mclk_mhz: 875, vddc_mv: 1100, vddci_mv: 1100, power_mw: 225000 },
        ];
    }

    /// Initialize Navi 2x DPM states
    fn init_navi2x_dpm(&mut self) {
        // Navi 21/22/23/24 typical DPM states (RDNA 2)
        self.dpm_states = vec![
            DpmState { level: DpmLevel::Dpm0, enabled: true, sclk_mhz: 500, mclk_mhz: 96, vddc_mv: 700, vddci_mv: 750, power_mw: 8000 },
            DpmState { level: DpmLevel::Dpm1, enabled: true, sclk_mhz: 900, mclk_mhz: 675, vddc_mv: 750, vddci_mv: 800, power_mw: 40000 },
            DpmState { level: DpmLevel::Dpm2, enabled: true, sclk_mhz: 1300, mclk_mhz: 1000, vddc_mv: 800, vddci_mv: 850, power_mw: 90000 },
            DpmState { level: DpmLevel::Dpm3, enabled: true, sclk_mhz: 1700, mclk_mhz: 1000, vddc_mv: 850, vddci_mv: 900, power_mw: 140000 },
            DpmState { level: DpmLevel::Dpm4, enabled: true, sclk_mhz: 2000, mclk_mhz: 1000, vddc_mv: 900, vddci_mv: 950, power_mw: 190000 },
            DpmState { level: DpmLevel::Dpm5, enabled: true, sclk_mhz: 2200, mclk_mhz: 1000, vddc_mv: 950, vddci_mv: 1000, power_mw: 240000 },
            DpmState { level: DpmLevel::Dpm6, enabled: true, sclk_mhz: 2350, mclk_mhz: 1000, vddc_mv: 1000, vddci_mv: 1050, power_mw: 280000 },
            DpmState { level: DpmLevel::Dpm7, enabled: true, sclk_mhz: 2500, mclk_mhz: 1000, vddc_mv: 1050, vddci_mv: 1100, power_mw: 320000 },
        ];
    }

    /// Initialize Navi 3x DPM states
    fn init_navi3x_dpm(&mut self) {
        // Navi 31/32/33 typical DPM states (RDNA 3)
        self.dpm_states = vec![
            DpmState { level: DpmLevel::Dpm0, enabled: true, sclk_mhz: 500, mclk_mhz: 96, vddc_mv: 650, vddci_mv: 700, power_mw: 6000 },
            DpmState { level: DpmLevel::Dpm1, enabled: true, sclk_mhz: 1000, mclk_mhz: 900, vddc_mv: 700, vddci_mv: 750, power_mw: 50000 },
            DpmState { level: DpmLevel::Dpm2, enabled: true, sclk_mhz: 1500, mclk_mhz: 1200, vddc_mv: 750, vddci_mv: 800, power_mw: 120000 },
            DpmState { level: DpmLevel::Dpm3, enabled: true, sclk_mhz: 2000, mclk_mhz: 1500, vddc_mv: 800, vddci_mv: 850, power_mw: 180000 },
            DpmState { level: DpmLevel::Dpm4, enabled: true, sclk_mhz: 2300, mclk_mhz: 1800, vddc_mv: 850, vddci_mv: 900, power_mw: 240000 },
            DpmState { level: DpmLevel::Dpm5, enabled: true, sclk_mhz: 2500, mclk_mhz: 2000, vddc_mv: 900, vddci_mv: 950, power_mw: 300000 },
            DpmState { level: DpmLevel::Dpm6, enabled: true, sclk_mhz: 2700, mclk_mhz: 2250, vddc_mv: 950, vddci_mv: 1000, power_mw: 360000 },
            DpmState { level: DpmLevel::Dpm7, enabled: true, sclk_mhz: 2900, mclk_mhz: 2400, vddc_mv: 1000, vddci_mv: 1050, power_mw: 420000 },
        ];
    }

    /// Initialize fan control
    fn init_fan_control(&mut self) -> Result<(), &'static str> {
        // Set up default fan curve
        self.fan_curve = vec![
            FanTableEntry { temp_c: 30, pwm_percent: 0 },
            FanTableEntry { temp_c: 40, pwm_percent: 20 },
            FanTableEntry { temp_c: 50, pwm_percent: 35 },
            FanTableEntry { temp_c: 60, pwm_percent: 50 },
            FanTableEntry { temp_c: 70, pwm_percent: 70 },
            FanTableEntry { temp_c: 80, pwm_percent: 90 },
            FanTableEntry { temp_c: 90, pwm_percent: 100 },
        ];

        // Enable automatic fan control by default
        self.set_fan_mode(FanControlMode::Auto)?;

        Ok(())
    }

    /// Get power limits from SMU
    fn get_power_limits_from_smu(&mut self) -> Result<(), &'static str> {
        // Try to get power limit from SMU
        if let Ok(tdp) = self.send_smu_msg(smu_msg::GET_POWER_LIMIT, 0) {
            self.power_limits.tdp_w = tdp / 1000; // mW to W
            self.power_limits.default_tdp_w = tdp / 1000;
        } else {
            // Use defaults based on generation
            match self.generation {
                GpuGeneration::Polaris => {
                    self.power_limits.tdp_w = 150;
                    self.power_limits.max_tdp_w = 200;
                    self.power_limits.min_tdp_w = 75;
                }
                GpuGeneration::Vega => {
                    self.power_limits.tdp_w = 295;
                    self.power_limits.max_tdp_w = 350;
                    self.power_limits.min_tdp_w = 150;
                }
                GpuGeneration::Navi1x => {
                    self.power_limits.tdp_w = 225;
                    self.power_limits.max_tdp_w = 280;
                    self.power_limits.min_tdp_w = 130;
                }
                GpuGeneration::Navi2x => {
                    self.power_limits.tdp_w = 320;
                    self.power_limits.max_tdp_w = 400;
                    self.power_limits.min_tdp_w = 160;
                }
                GpuGeneration::Navi3x => {
                    self.power_limits.tdp_w = 355;
                    self.power_limits.max_tdp_w = 450;
                    self.power_limits.min_tdp_w = 200;
                }
            }
            self.power_limits.default_tdp_w = self.power_limits.tdp_w;
        }

        Ok(())
    }

    /// Enable PowerPlay features
    fn enable_features(&mut self) -> Result<(), &'static str> {
        // Enable DPM
        let feature_mask: u64 =
            (1 << 0) |  // DPM_PREFETCHER
            (1 << 1) |  // DPM_GFXCLK
            (1 << 2) |  // DPM_GFX_PACE
            (1 << 3) |  // DPM_UCLK
            (1 << 4) |  // DPM_SOCCLK
            (1 << 5) |  // DPM_MP0CLK
            (1 << 6) |  // DPM_LINK
            (1 << 7) |  // DPM_DCEFCLK
            (1 << 8) |  // DS_GFXCLK
            (1 << 9) |  // DS_SOCCLK
            (1 << 10) | // DS_LCLK
            (1 << 11) | // DS_DCEFCLK
            (1 << 12) | // DS_UCLK
            (1 << 13) | // GFX_ULV
            (1 << 14) | // FW_DSTATE
            (1 << 15) | // GFXOFF
            (1 << 16) | // BACO
            (1 << 17) | // VCN_PG
            (1 << 18) | // JPEG_PG
            (1 << 19) | // FAN_CONTROL
            (1 << 20) | // THERMAL
            (1 << 21);  // GFX_DCS

        // Send feature mask to SMU
        let _ = self.send_smu_msg(smu_msg::SET_ALLOWED_FEATURE_MASK, feature_mask as u32);
        let _ = self.send_smu_msg(smu_msg::SET_ALLOWED_FEATURE_MASK, (feature_mask >> 32) as u32);

        self.features_enabled = feature_mask;

        Ok(())
    }

    /// Set power profile
    pub fn set_power_profile(&mut self, profile: PowerProfile) -> Result<(), &'static str> {
        let profile_id = match profile {
            PowerProfile::Bootup => 0,
            PowerProfile::ThreeDFullScreen => 1,
            PowerProfile::PowerSaving => 2,
            PowerProfile::Video => 3,
            PowerProfile::VR => 4,
            PowerProfile::Compute => 5,
            PowerProfile::Custom => 6,
        };

        self.send_smu_msg(smu_msg::SET_POWER_PROFILE, profile_id)?;
        self.power_profile = profile;

        crate::kprintln!("[PowerPlay] Set power profile: {:?}", profile);

        Ok(())
    }

    /// Set DPM level
    pub fn set_dpm_level(&mut self, level: DpmLevel) -> Result<(), &'static str> {
        let level_id = level as u32;

        // Check if level is valid
        if level_id as usize >= self.dpm_states.len() {
            return Err("Invalid DPM level");
        }

        // Check if level is enabled
        if !self.dpm_states[level_id as usize].enabled {
            return Err("DPM level is disabled");
        }

        self.send_smu_msg(smu_msg::SET_DPM_LEVEL, level_id)?;
        self.current_level = level;

        crate::kprintln!("[PowerPlay] Set DPM level: {:?}", level);

        Ok(())
    }

    /// Set fan control mode
    pub fn set_fan_mode(&mut self, mode: FanControlMode) -> Result<(), &'static str> {
        let mode_id = match mode {
            FanControlMode::None => 0,
            FanControlMode::Auto => 1,
            FanControlMode::Manual => 2,
        };

        self.send_smu_msg(smu_msg::SET_FAN_CONTROL_MODE, mode_id)?;
        self.fan_mode = mode;

        Ok(())
    }

    /// Set fan speed (manual mode only)
    pub fn set_fan_speed_pwm(&mut self, pwm_percent: u8) -> Result<(), &'static str> {
        if self.fan_mode != FanControlMode::Manual {
            return Err("Fan not in manual mode");
        }

        let pwm = (pwm_percent as u32).min(100);
        self.send_smu_msg(smu_msg::SET_FAN_SPEED_PWM, pwm)?;

        Ok(())
    }

    /// Get current fan speed
    pub fn get_fan_speed(&self) -> u32 {
        self.send_smu_msg(smu_msg::GET_FAN_SPEED_RPM, 0).unwrap_or(0)
    }

    /// Get GPU temperature
    pub fn get_temperature(&self) -> ThermalZone {
        // Read temperature from thermal register
        let temp_raw = self.smc_read(regs::THM_TCON_CUR_TMP);

        // Convert to Celsius (format varies by generation)
        let edge_temp = ((temp_raw >> 8) & 0x1FF) as i32;

        // For RDNA, try to get junction/memory temps
        let junction_temp = if self.generation as u8 >= GpuGeneration::Navi1x as u8 {
            edge_temp + 10 // Estimate
        } else {
            edge_temp
        };

        ThermalZone {
            edge_temp,
            junction_temp,
            memory_temp: edge_temp + 5,
            hotspot_temp: junction_temp + 5,
        }
    }

    /// Get current power draw
    pub fn get_current_power(&self) -> u32 {
        self.send_smu_msg(smu_msg::GET_CURRENT_POWER, 0).unwrap_or(0)
    }

    /// Set power limit
    pub fn set_power_limit(&mut self, tdp_w: u32) -> Result<(), &'static str> {
        if tdp_w < self.power_limits.min_tdp_w || tdp_w > self.power_limits.max_tdp_w {
            return Err("Power limit out of range");
        }

        let tdp_mw = tdp_w * 1000;
        self.send_smu_msg(smu_msg::SET_POWER_LIMIT, tdp_mw)?;
        self.power_limits.tdp_w = tdp_w;

        crate::kprintln!("[PowerPlay] Set power limit: {}W", tdp_w);

        Ok(())
    }

    /// Set GPU clock limits
    pub fn set_gfx_clock_range(&mut self, min_mhz: u32, max_mhz: u32) -> Result<(), &'static str> {
        self.send_smu_msg(smu_msg::SET_HARD_MIN_GFXCLK, min_mhz)?;
        self.send_smu_msg(smu_msg::SET_SOFT_MAX_GFXCLK, max_mhz)?;

        crate::kprintln!("[PowerPlay] Set GFX clock range: {}-{} MHz", min_mhz, max_mhz);

        Ok(())
    }

    /// Set memory clock limits
    pub fn set_mem_clock_range(&mut self, min_mhz: u32, max_mhz: u32) -> Result<(), &'static str> {
        self.send_smu_msg(smu_msg::SET_HARD_MIN_UCLK, min_mhz)?;
        self.send_smu_msg(smu_msg::SET_SOFT_MAX_UCLK, max_mhz)?;

        crate::kprintln!("[PowerPlay] Set MEM clock range: {}-{} MHz", min_mhz, max_mhz);

        Ok(())
    }

    /// Enable/disable GfxOff (deep idle state)
    pub fn set_gfxoff(&mut self, enable: bool) -> Result<(), &'static str> {
        let msg = if enable {
            smu_msg::ENABLE_GFX_OFF
        } else {
            smu_msg::DISABLE_GFX_OFF
        };

        self.send_smu_msg(msg, 0)?;
        self.gfxoff_enabled = enable;

        crate::kprintln!("[PowerPlay] GfxOff: {}", if enable { "enabled" } else { "disabled" });

        Ok(())
    }

    /// Enter BACO (Bus Active, Chip Off) state
    pub fn enter_baco(&self) -> Result<(), &'static str> {
        crate::kprintln!("[PowerPlay] Entering BACO state");
        self.send_smu_msg(smu_msg::ENTER_BACO, 0)?;
        Ok(())
    }

    /// Exit BACO state
    pub fn exit_baco(&self) -> Result<(), &'static str> {
        crate::kprintln!("[PowerPlay] Exiting BACO state");
        self.send_smu_msg(smu_msg::EXIT_BACO, 0)?;
        Ok(())
    }

    /// Get current DPM state info
    pub fn get_current_dpm_state(&self) -> Option<&DpmState> {
        self.dpm_states.get(self.current_level as usize)
    }

    /// Get all DPM states
    pub fn get_dpm_states(&self) -> &[DpmState] {
        &self.dpm_states
    }

    /// Get power limits
    pub fn get_power_limits(&self) -> &GpuPowerLimits {
        &self.power_limits
    }

    /// Get features enabled mask
    pub fn get_features_enabled(&self) -> u64 {
        self.features_enabled
    }

    /// Check if initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Get GPU generation
    pub fn get_generation(&self) -> GpuGeneration {
        self.generation
    }

    /// Update fan curve
    pub fn set_fan_curve(&mut self, curve: Vec<FanTableEntry>) -> Result<(), &'static str> {
        if curve.len() < 2 {
            return Err("Fan curve must have at least 2 points");
        }

        // Verify curve is monotonic
        for i in 1..curve.len() {
            if curve[i].temp_c <= curve[i-1].temp_c {
                return Err("Fan curve temperatures must be increasing");
            }
        }

        self.fan_curve = curve;

        // Apply curve to hardware (implementation depends on generation)
        self.apply_fan_curve()?;

        Ok(())
    }

    /// Apply fan curve to hardware
    fn apply_fan_curve(&self) -> Result<(), &'static str> {
        // Write fan table to SMU
        // This is generation-specific - for now just log
        crate::kprintln!("[PowerPlay] Applied fan curve with {} points", self.fan_curve.len());
        Ok(())
    }

    /// Get status summary
    pub fn get_status(&self) -> String {
        let temp = self.get_temperature();
        let power = self.get_current_power();
        let fan = self.get_fan_speed();

        format!(
            "PowerPlay Status:\n\
             Generation: {:?}\n\
             DPM Level: {:?}\n\
             Profile: {:?}\n\
             Temperature: {}°C (edge) / {}°C (junction)\n\
             Power: {} mW\n\
             Fan: {} RPM ({:?})\n\
             TDP: {}W / {}W max\n\
             GfxOff: {}",
            self.generation,
            self.current_level,
            self.power_profile,
            temp.edge_temp,
            temp.junction_temp,
            power,
            fan,
            self.fan_mode,
            self.power_limits.tdp_w,
            self.power_limits.max_tdp_w,
            if self.gfxoff_enabled { "enabled" } else { "disabled" }
        )
    }
}

/// Global PowerPlay instance
static POWERPLAY: Mutex<Option<AmdPowerPlay>> = Mutex::new(None);

/// Initialize PowerPlay for a GPU
pub fn init_powerplay(mmio_base: u64, pci_device: (u8, u8, u8), device_id: u16) -> Result<(), &'static str> {
    // Determine GPU generation from device ID
    let generation = match device_id {
        // Polaris (GCN 4)
        0x67C0..=0x67FF | 0x6980..=0x699F => GpuGeneration::Polaris,
        // Vega (GCN 5)
        0x6860..=0x687F | 0x6920..=0x695F => GpuGeneration::Vega,
        // Navi 1x (RDNA 1)
        0x7310..=0x731F | 0x7340..=0x734F | 0x7360..=0x736F => GpuGeneration::Navi1x,
        // Navi 2x (RDNA 2)
        0x73A0..=0x73BF | 0x73C0..=0x73DF | 0x73E0..=0x73FF => GpuGeneration::Navi2x,
        // Navi 3x (RDNA 3)
        0x7440..=0x744F | 0x7480..=0x749F => GpuGeneration::Navi3x,
        _ => return Err("Unknown GPU generation"),
    };

    let mut pp = AmdPowerPlay::new(mmio_base, pci_device, generation);
    pp.init()?;

    *POWERPLAY.lock() = Some(pp);

    Ok(())
}

/// Get PowerPlay instance
pub fn get_powerplay() -> Option<spin::MutexGuard<'static, Option<AmdPowerPlay>>> {
    let guard = POWERPLAY.lock();
    if guard.is_some() {
        Some(guard)
    } else {
        None
    }
}

/// Set power profile (convenience function)
pub fn set_profile(profile: PowerProfile) -> Result<(), &'static str> {
    if let Some(mut guard) = get_powerplay() {
        if let Some(pp) = guard.as_mut() {
            return pp.set_power_profile(profile);
        }
    }
    Err("PowerPlay not initialized")
}

/// Get GPU temperature (convenience function)
pub fn get_gpu_temp() -> Option<i32> {
    if let Some(guard) = get_powerplay() {
        if let Some(pp) = guard.as_ref() {
            return Some(pp.get_temperature().edge_temp);
        }
    }
    None
}
