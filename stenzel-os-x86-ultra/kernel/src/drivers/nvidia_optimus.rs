//! NVIDIA Optimus / Hybrid Graphics Driver
//!
//! Handles switching between integrated (Intel/AMD) and discrete (NVIDIA) GPUs:
//! - Automatic GPU switching based on workload
//! - Manual GPU selection
//! - Power management for discrete GPU
//! - Display output routing (muxless/muxed configurations)

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use spin::Mutex;

/// GPU type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuType {
    /// Intel integrated GPU
    IntelIntegrated,
    /// AMD integrated GPU (APU)
    AmdIntegrated,
    /// NVIDIA discrete GPU
    NvidiaDiscrete,
    /// AMD discrete GPU
    AmdDiscrete,
    /// Unknown
    Unknown,
}

impl GpuType {
    /// Check if this is an integrated GPU
    pub fn is_integrated(&self) -> bool {
        matches!(self, Self::IntelIntegrated | Self::AmdIntegrated)
    }

    /// Check if this is a discrete GPU
    pub fn is_discrete(&self) -> bool {
        matches!(self, Self::NvidiaDiscrete | Self::AmdDiscrete)
    }

    /// Get vendor string
    pub fn vendor(&self) -> &'static str {
        match self {
            Self::IntelIntegrated => "Intel",
            Self::AmdIntegrated | Self::AmdDiscrete => "AMD",
            Self::NvidiaDiscrete => "NVIDIA",
            Self::Unknown => "Unknown",
        }
    }
}

/// Hybrid graphics configuration type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HybridType {
    /// No hybrid graphics (single GPU)
    None,
    /// Muxless - iGPU always drives display, dGPU renders to iGPU framebuffer
    Muxless,
    /// Muxed - Hardware MUX switches display between GPUs
    Muxed,
    /// Dynamic - Software-controlled rendering with PRIME/DRI offload
    Dynamic,
}

/// GPU power state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuPowerState {
    /// GPU is fully active
    Active,
    /// GPU is in low power state
    LowPower,
    /// GPU is suspended (D3cold)
    Suspended,
    /// GPU is powered off
    Off,
}

/// Optimus switching mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchingMode {
    /// Always use integrated GPU
    IntegratedOnly,
    /// Always use discrete GPU
    DiscreteOnly,
    /// Automatic switching based on workload
    Automatic,
    /// Render on dGPU, display on iGPU (PRIME render offload)
    RenderOffload,
    /// User-controlled per-application
    OnDemand,
}

/// GPU selection policy for automatic mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionPolicy {
    /// Prefer power efficiency
    PowerSaving,
    /// Prefer performance
    Performance,
    /// Balance power and performance
    Balanced,
    /// Select based on application profile
    ProfileBased,
}

/// GPU information
#[derive(Debug, Clone)]
pub struct GpuInfo {
    /// GPU type
    pub gpu_type: GpuType,
    /// PCI bus/device/function
    pub pci_bdf: (u8, u8, u8),
    /// Device ID
    pub device_id: u16,
    /// Vendor ID
    pub vendor_id: u16,
    /// MMIO base address
    pub mmio_base: u64,
    /// Current power state
    pub power_state: GpuPowerState,
    /// Whether this GPU is currently active for rendering
    pub active: bool,
    /// Whether this GPU drives a display
    pub drives_display: bool,
    /// GPU name
    pub name: String,
}

/// ACPI _DSM method GUIDs
pub mod dsm_guids {
    /// NVIDIA Optimus DSM GUID
    pub const OPTIMUS_DSM: [u8; 16] = [
        0xa0, 0xa0, 0x95, 0x9d,
        0x60, 0x00, 0x48, 0x4d,
        0xb3, 0x4d, 0x7e, 0x5f,
        0xea, 0x12, 0x9f, 0xd4,
    ];

    /// NVIDIA GPS (GPU Power Steering) DSM GUID
    pub const GPS_DSM: [u8; 16] = [
        0x95, 0x1f, 0xf8, 0xa6,
        0xd3, 0x6e, 0x4d, 0xa7,
        0x9c, 0xf9, 0xbb, 0xcf,
        0x4e, 0x8c, 0x5d, 0x0e,
    ];

    /// Nouveau Runtime Power Management DSM GUID
    pub const NOUVEAU_RPM_DSM: [u8; 16] = [
        0x4d, 0x45, 0x53, 0x41,
        0x2d, 0x4e, 0x4f, 0x55,
        0x56, 0x2d, 0x44, 0x53,
        0x4d, 0x30, 0x00, 0x00,
    ];
}

/// Optimus DSM functions
pub mod dsm_funcs {
    /// Query supported functions
    pub const QUERY_FUNCTIONS: u32 = 0;
    /// GPU power control
    pub const GPU_POWER_CTRL: u32 = 1;
    /// Display MUX control
    pub const MUX_CONTROL: u32 = 2;
    /// Get GPU state
    pub const GET_GPU_STATE: u32 = 3;
    /// Set display mode
    pub const SET_DISPLAY_MODE: u32 = 4;
    /// ROM sharing control
    pub const ROM_SHARING: u32 = 5;
    /// Optimus flags
    pub const OPTIMUS_FLAGS: u32 = 6;
}

/// Runtime power management states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimePmState {
    /// PM disabled
    Disabled,
    /// PM enabled but GPU is active
    Active,
    /// GPU can auto-suspend
    AutoSuspend,
    /// GPU is runtime suspended
    Suspended,
}

/// Application profile for GPU selection
#[derive(Debug, Clone)]
pub struct AppProfile {
    /// Application name or path pattern
    pub name: String,
    /// Preferred GPU
    pub preferred_gpu: GpuType,
    /// Force discrete GPU
    pub force_discrete: bool,
    /// Environment variables to set
    pub env_vars: Vec<(String, String)>,
}

/// NVIDIA Optimus Manager
pub struct OptimusManager {
    /// Integrated GPU info
    igpu: Option<GpuInfo>,
    /// Discrete GPU info
    dgpu: Option<GpuInfo>,
    /// Hybrid type
    hybrid_type: HybridType,
    /// Current switching mode
    switching_mode: SwitchingMode,
    /// Selection policy
    selection_policy: SelectionPolicy,
    /// Runtime PM state
    runtime_pm: RuntimePmState,
    /// Auto-suspend delay (ms)
    auto_suspend_delay_ms: u32,
    /// Application profiles
    app_profiles: Vec<AppProfile>,
    /// Whether Optimus is supported
    supported: bool,
    /// Whether MUX is present
    has_mux: bool,
    /// Current active GPU for rendering
    active_render_gpu: GpuType,
    /// Current display GPU
    display_gpu: GpuType,
}

/// PCI configuration space offsets
pub mod pci_regs {
    pub const PCI_VENDOR_ID: u8 = 0x00;
    pub const PCI_DEVICE_ID: u8 = 0x02;
    pub const PCI_COMMAND: u8 = 0x04;
    pub const PCI_STATUS: u8 = 0x06;
    pub const PCI_CLASS_CODE: u8 = 0x09;
    pub const PCI_BAR0: u8 = 0x10;
}

impl OptimusManager {
    /// Create new Optimus manager
    pub fn new() -> Self {
        Self {
            igpu: None,
            dgpu: None,
            hybrid_type: HybridType::None,
            switching_mode: SwitchingMode::Automatic,
            selection_policy: SelectionPolicy::Balanced,
            runtime_pm: RuntimePmState::Disabled,
            auto_suspend_delay_ms: 5000,
            app_profiles: Vec::new(),
            supported: false,
            has_mux: false,
            active_render_gpu: GpuType::Unknown,
            display_gpu: GpuType::Unknown,
        }
    }

    /// Initialize Optimus
    pub fn init(&mut self) -> Result<(), &'static str> {
        crate::kprintln!("[optimus] Initializing hybrid graphics");

        // Scan for GPUs
        self.scan_gpus()?;

        // Detect hybrid configuration
        self.detect_hybrid_config()?;

        // Initialize ACPI DSM if available
        self.init_acpi_dsm()?;

        // Set up runtime PM
        self.init_runtime_pm()?;

        // Load default application profiles
        self.load_default_profiles();

        self.supported = self.igpu.is_some() && self.dgpu.is_some();

        if self.supported {
            crate::kprintln!("[optimus] Hybrid graphics: {:?}", self.hybrid_type);
            crate::kprintln!("[optimus] iGPU: {}", self.igpu.as_ref().map(|g| g.name.as_str()).unwrap_or("none"));
            crate::kprintln!("[optimus] dGPU: {}", self.dgpu.as_ref().map(|g| g.name.as_str()).unwrap_or("none"));
        } else {
            crate::kprintln!("[optimus] No hybrid graphics detected");
        }

        Ok(())
    }

    /// Scan for GPUs on PCI bus
    fn scan_gpus(&mut self) -> Result<(), &'static str> {
        crate::kprintln!("[optimus] Scanning for GPUs");

        // Scan PCI for display class devices (03:00:00 = VGA)
        for bus in 0..=255u8 {
            for device in 0..32u8 {
                for func in 0..8u8 {
                    let vendor_id = crate::drivers::pci::read_u16(bus, device, func, pci_regs::PCI_VENDOR_ID);
                    if vendor_id == 0xFFFF {
                        continue;
                    }

                    let device_id = crate::drivers::pci::read_u16(bus, device, func, pci_regs::PCI_DEVICE_ID);
                    let class_code = crate::drivers::pci::read_u8(bus, device, func, pci_regs::PCI_CLASS_CODE);

                    // VGA compatible controller (class 03)
                    if class_code != 0x03 {
                        continue;
                    }

                    let gpu_type = match vendor_id {
                        0x8086 => GpuType::IntelIntegrated,
                        0x1002 => {
                            // Check if AMD APU or discrete
                            if self.is_amd_apu(device_id) {
                                GpuType::AmdIntegrated
                            } else {
                                GpuType::AmdDiscrete
                            }
                        }
                        0x10DE => GpuType::NvidiaDiscrete,
                        _ => GpuType::Unknown,
                    };

                    let bar0 = crate::drivers::pci::read_u32(bus, device, func, pci_regs::PCI_BAR0);
                    let mmio_base = (bar0 & 0xFFFFFFF0) as u64;

                    let gpu_info = GpuInfo {
                        gpu_type,
                        pci_bdf: (bus, device, func),
                        device_id,
                        vendor_id,
                        mmio_base,
                        power_state: GpuPowerState::Active,
                        active: false,
                        drives_display: false,
                        name: self.get_gpu_name(vendor_id, device_id),
                    };

                    crate::kprintln!("[optimus] Found GPU: {:?} at {:02X}:{:02X}.{} (0x{:04X}:0x{:04X})",
                                   gpu_type, bus, device, func, vendor_id, device_id);

                    if gpu_type.is_integrated() {
                        self.igpu = Some(gpu_info);
                    } else if gpu_type.is_discrete() {
                        self.dgpu = Some(gpu_info);
                    }
                }
            }
        }

        Ok(())
    }

    /// Check if AMD device is an APU
    fn is_amd_apu(&self, device_id: u16) -> bool {
        // Common AMD APU device IDs
        matches!(device_id,
            0x15DD | 0x15D8 | // Raven/Picasso
            0x1636 | 0x1638 | // Renoir
            0x1681 | 0x164C | // Cezanne
            0x15E7 | 0x1900 | // Barcelo/Rembrandt
            0x15BF | 0x15C8 | // Phoenix
            0x1506 | 0x150E   // Strix Point
        )
    }

    /// Get GPU name from IDs
    fn get_gpu_name(&self, vendor_id: u16, device_id: u16) -> String {
        match vendor_id {
            0x8086 => {
                // Intel
                match device_id >> 4 {
                    0x591 => format!("Intel HD Graphics 630"),
                    0x3E9 => format!("Intel UHD Graphics 630"),
                    0x9A4 => format!("Intel Iris Xe Graphics"),
                    0xA78 => format!("Intel UHD Graphics 770"),
                    _ => format!("Intel GPU 0x{:04X}", device_id),
                }
            }
            0x1002 => {
                // AMD
                format!("AMD GPU 0x{:04X}", device_id)
            }
            0x10DE => {
                // NVIDIA
                match device_id >> 8 {
                    0x12 | 0x13 => format!("NVIDIA GeForce GTX (Maxwell)"),
                    0x15 | 0x1B | 0x1C | 0x1D => format!("NVIDIA GeForce GTX 10 Series"),
                    0x1E | 0x1F => format!("NVIDIA GeForce RTX 20 Series"),
                    0x22 | 0x24 | 0x25 => format!("NVIDIA GeForce RTX 30 Series"),
                    0x26 | 0x27 | 0x28 => format!("NVIDIA GeForce RTX 40 Series"),
                    _ => format!("NVIDIA GPU 0x{:04X}", device_id),
                }
            }
            _ => format!("Unknown GPU 0x{:04X}:0x{:04X}", vendor_id, device_id),
        }
    }

    /// Detect hybrid graphics configuration
    fn detect_hybrid_config(&mut self) -> Result<(), &'static str> {
        if self.igpu.is_none() || self.dgpu.is_none() {
            self.hybrid_type = HybridType::None;
            return Ok(());
        }

        // Check for MUX presence via ACPI or EC
        self.has_mux = self.detect_mux();

        if self.has_mux {
            self.hybrid_type = HybridType::Muxed;
        } else {
            // Most modern Optimus laptops are muxless
            self.hybrid_type = HybridType::Muxless;
        }

        // In muxless, iGPU always drives display
        if self.hybrid_type == HybridType::Muxless {
            if let Some(igpu) = &mut self.igpu {
                igpu.drives_display = true;
                igpu.active = true;
            }
            self.display_gpu = self.igpu.as_ref().map(|g| g.gpu_type).unwrap_or(GpuType::Unknown);
            self.active_render_gpu = self.display_gpu;
        }

        Ok(())
    }

    /// Detect if system has a display MUX
    fn detect_mux(&self) -> bool {
        // Check ACPI for MUX control methods
        // For now, assume muxless (most common)
        false
    }

    /// Initialize ACPI DSM methods
    fn init_acpi_dsm(&mut self) -> Result<(), &'static str> {
        crate::kprintln!("[optimus] Checking ACPI DSM support");

        // Would query ACPI for Optimus DSM support
        // For now, assume basic support

        Ok(())
    }

    /// Initialize runtime power management
    fn init_runtime_pm(&mut self) -> Result<(), &'static str> {
        if self.dgpu.is_some() {
            self.runtime_pm = RuntimePmState::AutoSuspend;
            crate::kprintln!("[optimus] Runtime PM enabled, auto-suspend delay: {}ms",
                           self.auto_suspend_delay_ms);
        }
        Ok(())
    }

    /// Load default application profiles
    fn load_default_profiles(&mut self) {
        // Games - use discrete GPU
        self.app_profiles.push(AppProfile {
            name: "steam".into(),
            preferred_gpu: GpuType::NvidiaDiscrete,
            force_discrete: true,
            env_vars: vec![
                ("__NV_PRIME_RENDER_OFFLOAD".into(), "1".into()),
                ("__GLX_VENDOR_LIBRARY_NAME".into(), "nvidia".into()),
            ],
        });

        // Video playback - can use integrated
        self.app_profiles.push(AppProfile {
            name: "vlc".into(),
            preferred_gpu: GpuType::IntelIntegrated,
            force_discrete: false,
            env_vars: Vec::new(),
        });

        // 3D applications
        self.app_profiles.push(AppProfile {
            name: "blender".into(),
            preferred_gpu: GpuType::NvidiaDiscrete,
            force_discrete: true,
            env_vars: vec![
                ("__NV_PRIME_RENDER_OFFLOAD".into(), "1".into()),
            ],
        });
    }

    /// Set switching mode
    pub fn set_switching_mode(&mut self, mode: SwitchingMode) -> Result<(), &'static str> {
        crate::kprintln!("[optimus] Setting switching mode: {:?}", mode);

        self.switching_mode = mode;

        match mode {
            SwitchingMode::IntegratedOnly => {
                self.power_off_dgpu()?;
            }
            SwitchingMode::DiscreteOnly => {
                self.power_on_dgpu()?;
                self.active_render_gpu = GpuType::NvidiaDiscrete;
            }
            SwitchingMode::Automatic | SwitchingMode::RenderOffload | SwitchingMode::OnDemand => {
                // Keep dGPU available but suspended
                self.enable_runtime_pm()?;
            }
        }

        Ok(())
    }

    /// Set selection policy
    pub fn set_selection_policy(&mut self, policy: SelectionPolicy) {
        crate::kprintln!("[optimus] Setting selection policy: {:?}", policy);
        self.selection_policy = policy;
    }

    /// Power on discrete GPU
    pub fn power_on_dgpu(&mut self) -> Result<(), &'static str> {
        // Check current state first
        let needs_power_on = self.dgpu.as_ref()
            .map(|d| d.power_state == GpuPowerState::Off || d.power_state == GpuPowerState::Suspended)
            .unwrap_or(false);

        if needs_power_on {
            crate::kprintln!("[optimus] Powering on dGPU");

            // Call ACPI DSM to power on
            self.acpi_dgpu_power(true)?;

            if let Some(dgpu) = &mut self.dgpu {
                dgpu.power_state = GpuPowerState::Active;
            }
            self.runtime_pm = RuntimePmState::Active;
        }
        Ok(())
    }

    /// Power off discrete GPU
    pub fn power_off_dgpu(&mut self) -> Result<(), &'static str> {
        // Check current state first
        let needs_power_off = self.dgpu.as_ref()
            .map(|d| d.power_state == GpuPowerState::Active || d.power_state == GpuPowerState::LowPower)
            .unwrap_or(false);

        if needs_power_off {
            crate::kprintln!("[optimus] Powering off dGPU");

            // Call ACPI DSM to power off
            self.acpi_dgpu_power(false)?;

            let display_gpu = self.display_gpu;
            if let Some(dgpu) = &mut self.dgpu {
                dgpu.power_state = GpuPowerState::Off;
            }
            self.active_render_gpu = display_gpu;
        }
        Ok(())
    }

    /// Suspend discrete GPU (D3cold)
    pub fn suspend_dgpu(&mut self) -> Result<(), &'static str> {
        // Get info first to avoid borrow issues
        let (needs_suspend, pci_bdf) = self.dgpu.as_ref()
            .map(|d| {
                let needs = d.power_state == GpuPowerState::Active || d.power_state == GpuPowerState::LowPower;
                (needs, d.pci_bdf)
            })
            .unwrap_or((false, (0, 0, 0)));

        if needs_suspend {
            crate::kprintln!("[optimus] Suspending dGPU to D3cold");

            // Put device into D3cold via PCI PM
            let (bus, dev, func) = pci_bdf;
            let pm_cap = self.find_pci_capability(bus, dev, func, 0x01); // PCI_CAP_ID_PM
            if let Some(pm_offset) = pm_cap {
                // Set D3cold state
                let pm_ctrl = crate::drivers::pci::read_u16(bus, dev, func, pm_offset + 4);
                crate::drivers::pci::write_u16(bus, dev, func, pm_offset + 4, (pm_ctrl & !0x03) | 0x03);
            }

            if let Some(dgpu) = &mut self.dgpu {
                dgpu.power_state = GpuPowerState::Suspended;
            }
            self.runtime_pm = RuntimePmState::Suspended;
        }
        Ok(())
    }

    /// Resume discrete GPU from D3cold
    pub fn resume_dgpu(&mut self) -> Result<(), &'static str> {
        // Get info first to avoid borrow issues
        let (needs_resume, pci_bdf) = self.dgpu.as_ref()
            .map(|d| (d.power_state == GpuPowerState::Suspended, d.pci_bdf))
            .unwrap_or((false, (0, 0, 0)));

        if needs_resume {
            crate::kprintln!("[optimus] Resuming dGPU from D3cold");

            // Restore D0 state via PCI PM
            let (bus, dev, func) = pci_bdf;
            let pm_cap = self.find_pci_capability(bus, dev, func, 0x01);
            if let Some(pm_offset) = pm_cap {
                let pm_ctrl = crate::drivers::pci::read_u16(bus, dev, func, pm_offset + 4);
                crate::drivers::pci::write_u16(bus, dev, func, pm_offset + 4, pm_ctrl & !0x03);
            }

            if let Some(dgpu) = &mut self.dgpu {
                dgpu.power_state = GpuPowerState::Active;
            }
            self.runtime_pm = RuntimePmState::Active;
        }
        Ok(())
    }

    /// Find PCI capability
    fn find_pci_capability(&self, bus: u8, dev: u8, func: u8, cap_id: u8) -> Option<u8> {
        let status = crate::drivers::pci::read_u16(bus, dev, func, pci_regs::PCI_STATUS);
        if status & 0x10 == 0 {
            return None; // No capabilities list
        }

        let mut cap_ptr = crate::drivers::pci::read_u8(bus, dev, func, 0x34) & 0xFC;

        while cap_ptr != 0 {
            let id = crate::drivers::pci::read_u8(bus, dev, func, cap_ptr);
            if id == cap_id {
                return Some(cap_ptr);
            }
            cap_ptr = crate::drivers::pci::read_u8(bus, dev, func, cap_ptr + 1) & 0xFC;
        }

        None
    }

    /// Call ACPI DSM for dGPU power control
    fn acpi_dgpu_power(&self, power_on: bool) -> Result<(), &'static str> {
        // In real implementation, would invoke ACPI _DSM method
        // with NVIDIA Optimus GUID and GPU_POWER_CTRL function
        crate::kprintln!("[optimus] ACPI DSM: power {}", if power_on { "on" } else { "off" });
        Ok(())
    }

    /// Enable runtime PM
    fn enable_runtime_pm(&mut self) -> Result<(), &'static str> {
        self.runtime_pm = RuntimePmState::AutoSuspend;
        crate::kprintln!("[optimus] Runtime PM enabled");
        Ok(())
    }

    /// Select GPU for application
    pub fn select_gpu_for_app(&mut self, app_name: &str) -> GpuType {
        // Check profiles - collect results first to avoid borrow issues
        let mut matched_profile: Option<(GpuType, bool)> = None;
        for profile in &self.app_profiles {
            if app_name.contains(&profile.name) {
                matched_profile = Some((profile.preferred_gpu, profile.force_discrete));
                break;
            }
        }

        if let Some((gpu_type, force_discrete)) = matched_profile {
            if force_discrete {
                let _ = self.power_on_dgpu();
            }
            return gpu_type;
        }

        // Use policy
        match self.selection_policy {
            SelectionPolicy::PowerSaving => {
                self.igpu.as_ref().map(|g| g.gpu_type).unwrap_or(GpuType::Unknown)
            }
            SelectionPolicy::Performance => {
                let _ = self.power_on_dgpu();
                self.dgpu.as_ref().map(|g| g.gpu_type).unwrap_or(GpuType::Unknown)
            }
            SelectionPolicy::Balanced | SelectionPolicy::ProfileBased => {
                // Default to integrated
                self.igpu.as_ref().map(|g| g.gpu_type).unwrap_or(GpuType::Unknown)
            }
        }
    }

    /// Add application profile
    pub fn add_profile(&mut self, profile: AppProfile) {
        self.app_profiles.push(profile);
    }

    /// Get environment variables for PRIME render offload
    pub fn get_prime_env(&self) -> Vec<(String, String)> {
        vec![
            ("__NV_PRIME_RENDER_OFFLOAD".into(), "1".into()),
            ("__VK_LAYER_NV_optimus".into(), "NVIDIA_only".into()),
            ("__GLX_VENDOR_LIBRARY_NAME".into(), "nvidia".into()),
        ]
    }

    /// Get status
    pub fn get_status(&self) -> String {
        let igpu_status = self.igpu.as_ref().map(|g| {
            format!("{} ({:?})", g.name, g.power_state)
        }).unwrap_or_else(|| "None".into());

        let dgpu_status = self.dgpu.as_ref().map(|g| {
            format!("{} ({:?})", g.name, g.power_state)
        }).unwrap_or_else(|| "None".into());

        format!(
            "NVIDIA Optimus Status:\n\
             Supported: {}\n\
             Hybrid Type: {:?}\n\
             Switching Mode: {:?}\n\
             Selection Policy: {:?}\n\
             Runtime PM: {:?}\n\
             iGPU: {}\n\
             dGPU: {}\n\
             Active Render GPU: {:?}\n\
             Display GPU: {:?}\n\
             Has MUX: {}",
            self.supported,
            self.hybrid_type,
            self.switching_mode,
            self.selection_policy,
            self.runtime_pm,
            igpu_status,
            dgpu_status,
            self.active_render_gpu,
            self.display_gpu,
            self.has_mux
        )
    }

    /// Check if Optimus is supported
    pub fn is_supported(&self) -> bool {
        self.supported
    }

    /// Get hybrid type
    pub fn get_hybrid_type(&self) -> HybridType {
        self.hybrid_type
    }

    /// Get current render GPU
    pub fn get_render_gpu(&self) -> GpuType {
        self.active_render_gpu
    }

    /// Get display GPU
    pub fn get_display_gpu(&self) -> GpuType {
        self.display_gpu
    }
}

/// Global Optimus manager
static OPTIMUS: Mutex<Option<OptimusManager>> = Mutex::new(None);

/// Initialize Optimus
pub fn init() -> Result<(), &'static str> {
    let mut manager = OptimusManager::new();
    manager.init()?;

    *OPTIMUS.lock() = Some(manager);

    Ok(())
}

/// Get Optimus manager
pub fn get_manager() -> Option<spin::MutexGuard<'static, Option<OptimusManager>>> {
    let guard = OPTIMUS.lock();
    if guard.is_some() {
        Some(guard)
    } else {
        None
    }
}

/// Check if Optimus is supported
pub fn is_supported() -> bool {
    if let Some(guard) = get_manager() {
        if let Some(mgr) = guard.as_ref() {
            return mgr.is_supported();
        }
    }
    false
}

/// Select GPU for application
pub fn select_gpu(app_name: &str) -> GpuType {
    if let Some(mut guard) = get_manager() {
        if let Some(mgr) = guard.as_mut() {
            return mgr.select_gpu_for_app(app_name);
        }
    }
    GpuType::Unknown
}

/// Power off discrete GPU
pub fn power_off_dgpu() -> Result<(), &'static str> {
    if let Some(mut guard) = get_manager() {
        if let Some(mgr) = guard.as_mut() {
            return mgr.power_off_dgpu();
        }
    }
    Err("Optimus not initialized")
}

/// Power on discrete GPU
pub fn power_on_dgpu() -> Result<(), &'static str> {
    if let Some(mut guard) = get_manager() {
        if let Some(mgr) = guard.as_mut() {
            return mgr.power_on_dgpu();
        }
    }
    Err("Optimus not initialized")
}

/// Get status
pub fn get_status() -> String {
    if let Some(guard) = get_manager() {
        if let Some(mgr) = guard.as_ref() {
            return mgr.get_status();
        }
    }
    "Optimus: Not initialized".into()
}
