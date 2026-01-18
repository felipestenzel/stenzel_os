//! NVIDIA GPU Firmware Driver
//!
//! Handles loading and management of NVIDIA GPU firmware blobs:
//! - Display firmware (DP/HDMI controller)
//! - PMU (Power Management Unit) firmware
//! - GR (Graphics) context switch firmware
//! - SEC2 (Security Engine) firmware
//! - GSP (GPU System Processor) firmware for newer GPUs

use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use spin::Mutex;

/// Firmware type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirmwareType {
    /// Display controller firmware
    Disp,
    /// Power Management Unit
    Pmu,
    /// Graphics context switch
    GrCtxsw,
    /// Graphics firmware image
    GrFecs,
    /// Graphics global context
    GrGpccs,
    /// Security Engine 2
    Sec2,
    /// GPU System Processor (Turing+)
    Gsp,
    /// NVDEC video decoder
    Nvdec,
    /// NVENC video encoder
    Nvenc,
    /// Copy Engine
    Ce,
}

impl FirmwareType {
    /// Get firmware file suffix
    pub fn file_suffix(&self) -> &'static str {
        match self {
            Self::Disp => "disp",
            Self::Pmu => "pmu",
            Self::GrCtxsw => "gr_ctxsw",
            Self::GrFecs => "gr_fecs",
            Self::GrGpccs => "gr_gpccs",
            Self::Sec2 => "sec2",
            Self::Gsp => "gsp",
            Self::Nvdec => "nvdec",
            Self::Nvenc => "nvenc",
            Self::Ce => "ce",
        }
    }
}

/// GPU generation for firmware selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NvidiaGen {
    Kepler,     // GK104, GK107, etc.
    Maxwell,    // GM107, GM204, etc.
    Pascal,     // GP102, GP104, etc.
    Volta,      // GV100
    Turing,     // TU102, TU104, etc.
    Ampere,     // GA102, GA104, etc.
    Ada,        // AD102, AD103, etc.
    Hopper,     // GH100
}

impl NvidiaGen {
    /// Get generation code name for firmware paths
    pub fn code_name(&self) -> &'static str {
        match self {
            Self::Kepler => "gk",
            Self::Maxwell => "gm",
            Self::Pascal => "gp",
            Self::Volta => "gv",
            Self::Turing => "tu",
            Self::Ampere => "ga",
            Self::Ada => "ad",
            Self::Hopper => "gh",
        }
    }

    /// Get chip ID prefix
    pub fn chip_prefix(&self) -> u8 {
        match self {
            Self::Kepler => 0xE0,
            Self::Maxwell => 0x12,
            Self::Pascal => 0x13,
            Self::Volta => 0x14,
            Self::Turing => 0x16,
            Self::Ampere => 0x17,
            Self::Ada => 0x19,
            Self::Hopper => 0x18,
        }
    }

    /// Whether this generation requires GSP firmware
    pub fn requires_gsp(&self) -> bool {
        matches!(self, Self::Turing | Self::Ampere | Self::Ada | Self::Hopper)
    }

    /// From device ID
    pub fn from_device_id(device_id: u16) -> Option<Self> {
        match device_id >> 8 {
            0x0F | 0x10 | 0x11 => Some(Self::Kepler),
            0x12 | 0x13 => Some(Self::Maxwell),
            0x15 | 0x1B | 0x1C | 0x1D => Some(Self::Pascal),
            0x1E => Some(Self::Volta),
            0x1F | 0x21 => Some(Self::Turing),
            0x22 | 0x24 | 0x25 => Some(Self::Ampere),
            0x26 | 0x27 | 0x28 => Some(Self::Ada),
            0x23 => Some(Self::Hopper),
            _ => None,
        }
    }
}

/// Firmware header structure (common nouveau format)
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct FirmwareHeader {
    /// Magic "NVFW"
    pub magic: [u8; 4],
    /// Version
    pub version: u32,
    /// Header size
    pub header_size: u32,
    /// Data offset
    pub data_offset: u32,
    /// Data size
    pub data_size: u32,
    /// Code offset
    pub code_offset: u32,
    /// Code size
    pub code_size: u32,
    /// Signature offset
    pub sig_offset: u32,
    /// Signature size
    pub sig_size: u32,
}

/// GSP firmware header (Turing+)
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct GspFirmwareHeader {
    /// Magic "GSP-"
    pub magic: [u8; 4],
    /// Version (major << 16 | minor)
    pub version: u32,
    /// Bootloader offset
    pub bootloader_offset: u32,
    /// Bootloader size
    pub bootloader_size: u32,
    /// GSP image offset
    pub gsp_image_offset: u32,
    /// GSP image size
    pub gsp_image_size: u32,
    /// Signature production offset
    pub sig_prod_offset: u32,
    /// Signature production size
    pub sig_prod_size: u32,
    /// Signature debug offset
    pub sig_dbg_offset: u32,
    /// Signature debug size
    pub sig_dbg_size: u32,
}

/// Loaded firmware state
#[derive(Debug, Clone)]
pub struct LoadedFirmware {
    pub fw_type: FirmwareType,
    pub version: u32,
    pub code: Vec<u8>,
    pub data: Vec<u8>,
    pub signature: Vec<u8>,
    pub load_address: u64,
    pub entry_point: u64,
}

/// PMU firmware specific data
#[derive(Debug, Clone)]
pub struct PmuFirmwareInfo {
    pub version: u32,
    pub code_size: u32,
    pub data_size: u32,
    pub boot_base: u64,
    pub boot_size: u32,
    pub image_base: u64,
}

/// GSP firmware info
#[derive(Debug, Clone)]
pub struct GspFirmwareInfo {
    pub version_major: u16,
    pub version_minor: u16,
    pub bootloader_size: u32,
    pub gsp_image_size: u32,
    pub booter_load_offset: u64,
    pub gsp_wpr_base: u64,
}

/// Firmware loading error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirmwareError {
    NotFound,
    InvalidFormat,
    VersionMismatch,
    SignatureInvalid,
    LoadFailed,
    Timeout,
    DeviceError,
}

/// NVIDIA Firmware Manager
pub struct NvidiaFirmwareManager {
    /// GPU generation
    generation: Option<NvidiaGen>,
    /// Device ID
    device_id: u16,
    /// MMIO base address
    mmio_base: u64,
    /// Loaded firmware
    firmware: Vec<LoadedFirmware>,
    /// PMU info
    pmu_info: Option<PmuFirmwareInfo>,
    /// GSP info
    gsp_info: Option<GspFirmwareInfo>,
    /// Initialized
    initialized: bool,
}

/// Falcon MMIO registers
pub mod falcon_regs {
    // Falcon control registers
    pub const FALCON_IRQSSET: u32 = 0x000;
    pub const FALCON_IRQSCLR: u32 = 0x004;
    pub const FALCON_IRQSTAT: u32 = 0x008;
    pub const FALCON_IRQMODE: u32 = 0x00C;
    pub const FALCON_IRQMSET: u32 = 0x010;
    pub const FALCON_IRQMCLR: u32 = 0x014;
    pub const FALCON_IRQMASK: u32 = 0x018;
    pub const FALCON_IRQDEST: u32 = 0x01C;

    // Falcon misc
    pub const FALCON_SCRATCH0: u32 = 0x040;
    pub const FALCON_SCRATCH1: u32 = 0x044;
    pub const FALCON_ITFEN: u32 = 0x048;
    pub const FALCON_IDLESTATE: u32 = 0x04C;
    pub const FALCON_TRACEIDX: u32 = 0x148;
    pub const FALCON_TRACEPC: u32 = 0x14C;

    // Falcon CPU control
    pub const FALCON_CPUCTL: u32 = 0x100;
    pub const FALCON_BOOTVEC: u32 = 0x104;
    pub const FALCON_HWCFG: u32 = 0x108;
    pub const FALCON_DMACTL: u32 = 0x10C;
    pub const FALCON_DMATRFBASE: u32 = 0x110;
    pub const FALCON_DMATRFMOFFS: u32 = 0x114;
    pub const FALCON_DMATRFCMD: u32 = 0x118;
    pub const FALCON_DMATRFFBOFFS: u32 = 0x11C;

    // CPUCTL bits
    pub const FALCON_CPUCTL_STARTCPU: u32 = 0x02;
    pub const FALCON_CPUCTL_HALTED: u32 = 0x10;

    // IMEM access
    pub const FALCON_IMEMC: u32 = 0x180;
    pub const FALCON_IMEMD: u32 = 0x184;
    pub const FALCON_IMEMT: u32 = 0x188;

    // DMEM access
    pub const FALCON_DMEMC: u32 = 0x1C0;
    pub const FALCON_DMEMD: u32 = 0x1C4;
    pub const FALCON_DMEMT: u32 = 0x1C8;

    // PMU specific offsets
    pub const NV_PMU_BASE: u32 = 0x10A000;
    pub const NV_PMU_FALCON: u32 = 0x10A100;
    pub const NV_PMU_MUTEX: u32 = 0x10A580;
    pub const NV_PMU_MUTEX_ID: u32 = 0x10A588;

    // SEC2 specific offsets
    pub const NV_SEC2_BASE: u32 = 0x840000;
    pub const NV_SEC2_FALCON: u32 = 0x840100;

    // GSP specific offsets
    pub const NV_GSP_BASE: u32 = 0x110000;
    pub const NV_GSP_FALCON: u32 = 0x110100;
}

impl NvidiaFirmwareManager {
    /// Create new firmware manager
    pub fn new(device_id: u16, mmio_base: u64) -> Self {
        Self {
            generation: NvidiaGen::from_device_id(device_id),
            device_id,
            mmio_base,
            firmware: Vec::new(),
            pmu_info: None,
            gsp_info: None,
            initialized: false,
        }
    }

    /// Initialize firmware manager
    pub fn init(&mut self) -> Result<(), FirmwareError> {
        crate::kprintln!("[nvidia-fw] Initializing for device 0x{:04X}", self.device_id);

        let gen = self.generation.ok_or(FirmwareError::NotFound)?;
        crate::kprintln!("[nvidia-fw] Detected generation: {:?}", gen);

        // Load required firmware for this generation
        self.load_firmware_set(gen)?;

        self.initialized = true;
        crate::kprintln!("[nvidia-fw] Initialization complete");

        Ok(())
    }

    /// Load firmware set for generation
    fn load_firmware_set(&mut self, gen: NvidiaGen) -> Result<(), FirmwareError> {
        // Load PMU firmware (all generations)
        self.load_pmu_firmware(gen)?;

        // Load GR firmware
        self.load_gr_firmware(gen)?;

        // Load SEC2 firmware (Maxwell+)
        if matches!(gen, NvidiaGen::Maxwell | NvidiaGen::Pascal |
                   NvidiaGen::Volta | NvidiaGen::Turing |
                   NvidiaGen::Ampere | NvidiaGen::Ada | NvidiaGen::Hopper) {
            self.load_sec2_firmware(gen)?;
        }

        // Load GSP firmware (Turing+)
        if gen.requires_gsp() {
            self.load_gsp_firmware(gen)?;
        }

        Ok(())
    }

    /// Load PMU firmware
    fn load_pmu_firmware(&mut self, gen: NvidiaGen) -> Result<(), FirmwareError> {
        crate::kprintln!("[nvidia-fw] Loading PMU firmware");

        // Get firmware path
        let fw_path = self.get_firmware_path(gen, FirmwareType::Pmu);
        crate::kprintln!("[nvidia-fw] Firmware path: {}", fw_path);

        // In a real implementation, load from filesystem
        // For now, create stub firmware info
        self.pmu_info = Some(PmuFirmwareInfo {
            version: 0x001,
            code_size: 0,
            data_size: 0,
            boot_base: 0,
            boot_size: 0,
            image_base: 0,
        });

        let fw = LoadedFirmware {
            fw_type: FirmwareType::Pmu,
            version: 0x001,
            code: Vec::new(),
            data: Vec::new(),
            signature: Vec::new(),
            load_address: 0,
            entry_point: 0,
        };

        self.firmware.push(fw);

        Ok(())
    }

    /// Load GR (Graphics) firmware
    fn load_gr_firmware(&mut self, gen: NvidiaGen) -> Result<(), FirmwareError> {
        crate::kprintln!("[nvidia-fw] Loading GR firmware");

        // FECS (Front End Context Switch) firmware
        let fecs_path = self.get_firmware_path(gen, FirmwareType::GrFecs);
        crate::kprintln!("[nvidia-fw] FECS firmware path: {}", fecs_path);

        let fecs = LoadedFirmware {
            fw_type: FirmwareType::GrFecs,
            version: 0x001,
            code: Vec::new(),
            data: Vec::new(),
            signature: Vec::new(),
            load_address: 0,
            entry_point: 0,
        };
        self.firmware.push(fecs);

        // GPCCS (GPC Context Switch) firmware
        let gpccs_path = self.get_firmware_path(gen, FirmwareType::GrGpccs);
        crate::kprintln!("[nvidia-fw] GPCCS firmware path: {}", gpccs_path);

        let gpccs = LoadedFirmware {
            fw_type: FirmwareType::GrGpccs,
            version: 0x001,
            code: Vec::new(),
            data: Vec::new(),
            signature: Vec::new(),
            load_address: 0,
            entry_point: 0,
        };
        self.firmware.push(gpccs);

        Ok(())
    }

    /// Load SEC2 firmware
    fn load_sec2_firmware(&mut self, gen: NvidiaGen) -> Result<(), FirmwareError> {
        crate::kprintln!("[nvidia-fw] Loading SEC2 firmware");

        let fw_path = self.get_firmware_path(gen, FirmwareType::Sec2);
        crate::kprintln!("[nvidia-fw] SEC2 firmware path: {}", fw_path);

        let fw = LoadedFirmware {
            fw_type: FirmwareType::Sec2,
            version: 0x001,
            code: Vec::new(),
            data: Vec::new(),
            signature: Vec::new(),
            load_address: 0,
            entry_point: 0,
        };
        self.firmware.push(fw);

        Ok(())
    }

    /// Load GSP firmware (Turing+)
    fn load_gsp_firmware(&mut self, gen: NvidiaGen) -> Result<(), FirmwareError> {
        crate::kprintln!("[nvidia-fw] Loading GSP firmware");

        let fw_path = self.get_firmware_path(gen, FirmwareType::Gsp);
        crate::kprintln!("[nvidia-fw] GSP firmware path: {}", fw_path);

        self.gsp_info = Some(GspFirmwareInfo {
            version_major: 1,
            version_minor: 0,
            bootloader_size: 0,
            gsp_image_size: 0,
            booter_load_offset: 0,
            gsp_wpr_base: 0,
        });

        let fw = LoadedFirmware {
            fw_type: FirmwareType::Gsp,
            version: 0x001,
            code: Vec::new(),
            data: Vec::new(),
            signature: Vec::new(),
            load_address: 0,
            entry_point: 0,
        };
        self.firmware.push(fw);

        Ok(())
    }

    /// Get firmware path for a given type
    fn get_firmware_path(&self, gen: NvidiaGen, fw_type: FirmwareType) -> String {
        // Standard nouveau firmware paths
        let chip_id = self.get_chip_id(gen);

        match fw_type {
            FirmwareType::Gsp => {
                format!("/lib/firmware/nvidia/{}/gsp/gsp.bin", chip_id)
            }
            _ => {
                format!("/lib/firmware/nvidia/{}/{}.bin", chip_id, fw_type.file_suffix())
            }
        }
    }

    /// Get chip ID string (e.g., "gm204", "tu102")
    fn get_chip_id(&self, gen: NvidiaGen) -> String {
        let prefix = gen.code_name();
        let suffix = match self.device_id >> 4 & 0xF {
            0 => "100",
            2 => "102",
            4 => "104",
            6 => "106",
            7 => "107",
            8 => "108",
            _ => "xxx",
        };
        format!("{}{}", prefix, suffix)
    }

    /// Read MMIO register
    fn mmio_read(&self, offset: u32) -> u32 {
        unsafe {
            let ptr = (self.mmio_base + offset as u64) as *const u32;
            core::ptr::read_volatile(ptr)
        }
    }

    /// Write MMIO register
    fn mmio_write(&self, offset: u32, value: u32) {
        unsafe {
            let ptr = (self.mmio_base + offset as u64) as *mut u32;
            core::ptr::write_volatile(ptr, value);
        }
    }

    /// Upload firmware to Falcon IMEM
    pub fn upload_to_imem(&self, falcon_base: u32, code: &[u8]) -> Result<(), FirmwareError> {
        crate::kprintln!("[nvidia-fw] Uploading to IMEM at base 0x{:08X}", falcon_base);

        // Halt Falcon first
        self.mmio_write(falcon_base + falcon_regs::FALCON_CPUCTL, 0);

        // Wait for halt
        let mut timeout = 10000;
        while timeout > 0 {
            let status = self.mmio_read(falcon_base + falcon_regs::FALCON_CPUCTL);
            if status & falcon_regs::FALCON_CPUCTL_HALTED != 0 {
                break;
            }
            timeout -= 1;
            for _ in 0..100 {
                core::hint::spin_loop();
            }
        }

        if timeout == 0 {
            return Err(FirmwareError::Timeout);
        }

        // Set IMEM transfer base address
        self.mmio_write(falcon_base + falcon_regs::FALCON_IMEMC,
                       0x00000000 | (1 << 24)); // Address 0, auto-increment

        // Upload code in 4-byte chunks
        for chunk in code.chunks(4) {
            let mut word: u32 = 0;
            for (i, &byte) in chunk.iter().enumerate() {
                word |= (byte as u32) << (i * 8);
            }
            self.mmio_write(falcon_base + falcon_regs::FALCON_IMEMD, word);
        }

        Ok(())
    }

    /// Upload firmware to Falcon DMEM
    pub fn upload_to_dmem(&self, falcon_base: u32, data: &[u8]) -> Result<(), FirmwareError> {
        crate::kprintln!("[nvidia-fw] Uploading to DMEM at base 0x{:08X}", falcon_base);

        // Set DMEM transfer base address
        self.mmio_write(falcon_base + falcon_regs::FALCON_DMEMC,
                       0x00000000 | (1 << 24)); // Address 0, auto-increment

        // Upload data in 4-byte chunks
        for chunk in data.chunks(4) {
            let mut word: u32 = 0;
            for (i, &byte) in chunk.iter().enumerate() {
                word |= (byte as u32) << (i * 8);
            }
            self.mmio_write(falcon_base + falcon_regs::FALCON_DMEMD, word);
        }

        Ok(())
    }

    /// Start Falcon execution
    pub fn start_falcon(&self, falcon_base: u32, boot_vector: u32) -> Result<(), FirmwareError> {
        crate::kprintln!("[nvidia-fw] Starting Falcon at 0x{:08X}, boot vector 0x{:08X}",
                        falcon_base, boot_vector);

        // Set boot vector
        self.mmio_write(falcon_base + falcon_regs::FALCON_BOOTVEC, boot_vector);

        // Start CPU
        self.mmio_write(falcon_base + falcon_regs::FALCON_CPUCTL,
                       falcon_regs::FALCON_CPUCTL_STARTCPU);

        // Wait for startup
        let mut timeout = 10000;
        while timeout > 0 {
            let status = self.mmio_read(falcon_base + falcon_regs::FALCON_CPUCTL);
            if status & falcon_regs::FALCON_CPUCTL_HALTED == 0 {
                break;
            }
            timeout -= 1;
            for _ in 0..100 {
                core::hint::spin_loop();
            }
        }

        if timeout == 0 {
            return Err(FirmwareError::Timeout);
        }

        Ok(())
    }

    /// Initialize PMU
    pub fn init_pmu(&mut self) -> Result<(), FirmwareError> {
        if !self.initialized {
            return Err(FirmwareError::DeviceError);
        }

        crate::kprintln!("[nvidia-fw] Initializing PMU");

        // Find PMU firmware
        let pmu_fw = self.firmware.iter()
            .find(|fw| fw.fw_type == FirmwareType::Pmu)
            .ok_or(FirmwareError::NotFound)?;

        // Upload to PMU Falcon
        if !pmu_fw.code.is_empty() {
            self.upload_to_imem(falcon_regs::NV_PMU_FALCON, &pmu_fw.code)?;
        }

        if !pmu_fw.data.is_empty() {
            self.upload_to_dmem(falcon_regs::NV_PMU_FALCON, &pmu_fw.data)?;
        }

        // Start PMU
        if pmu_fw.entry_point != 0 {
            self.start_falcon(falcon_regs::NV_PMU_FALCON, pmu_fw.entry_point as u32)?;
        }

        Ok(())
    }

    /// Initialize GSP (Turing+)
    pub fn init_gsp(&mut self) -> Result<(), FirmwareError> {
        if !self.initialized {
            return Err(FirmwareError::DeviceError);
        }

        let gen = self.generation.ok_or(FirmwareError::NotFound)?;
        if !gen.requires_gsp() {
            return Ok(()); // Not needed for this generation
        }

        crate::kprintln!("[nvidia-fw] Initializing GSP");

        // Find GSP firmware
        let gsp_fw = self.firmware.iter()
            .find(|fw| fw.fw_type == FirmwareType::Gsp)
            .ok_or(FirmwareError::NotFound)?;

        // GSP initialization is complex and involves:
        // 1. Setting up Write-Protect Region (WPR)
        // 2. Loading GSP bootloader
        // 3. Loading GSP RM (Resource Manager) firmware
        // 4. Starting GSP and waiting for handshake

        crate::kprintln!("[nvidia-fw] GSP firmware version: {}.{}",
                        self.gsp_info.as_ref().map(|i| i.version_major).unwrap_or(0),
                        self.gsp_info.as_ref().map(|i| i.version_minor).unwrap_or(0));

        Ok(())
    }

    /// Get loaded firmware
    pub fn get_firmware(&self, fw_type: FirmwareType) -> Option<&LoadedFirmware> {
        self.firmware.iter().find(|fw| fw.fw_type == fw_type)
    }

    /// Check if firmware is loaded
    pub fn is_firmware_loaded(&self, fw_type: FirmwareType) -> bool {
        self.firmware.iter().any(|fw| fw.fw_type == fw_type)
    }

    /// Get generation
    pub fn get_generation(&self) -> Option<NvidiaGen> {
        self.generation
    }

    /// Check if initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Get status string
    pub fn get_status(&self) -> String {
        let gen_str = self.generation
            .map(|g| format!("{:?}", g))
            .unwrap_or_else(|| "Unknown".to_string());

        let fw_list: Vec<String> = self.firmware.iter()
            .map(|fw| format!("{:?} v{}", fw.fw_type, fw.version))
            .collect();

        format!(
            "NVIDIA Firmware Manager:\n\
             Device ID: 0x{:04X}\n\
             Generation: {}\n\
             Initialized: {}\n\
             Loaded firmware: {:?}\n\
             PMU: {}\n\
             GSP: {}",
            self.device_id,
            gen_str,
            self.initialized,
            fw_list,
            if self.pmu_info.is_some() { "loaded" } else { "not loaded" },
            if self.gsp_info.is_some() { "loaded" } else { "not required/loaded" }
        )
    }
}

/// Global firmware manager
static NVIDIA_FW: Mutex<Option<NvidiaFirmwareManager>> = Mutex::new(None);

/// Initialize NVIDIA firmware for a device
pub fn init(device_id: u16, mmio_base: u64) -> Result<(), FirmwareError> {
    let mut fw = NvidiaFirmwareManager::new(device_id, mmio_base);
    fw.init()?;

    *NVIDIA_FW.lock() = Some(fw);

    Ok(())
}

/// Get firmware manager
pub fn get_manager() -> Option<spin::MutexGuard<'static, Option<NvidiaFirmwareManager>>> {
    let guard = NVIDIA_FW.lock();
    if guard.is_some() {
        Some(guard)
    } else {
        None
    }
}

/// Check if a firmware type is loaded
pub fn is_loaded(fw_type: FirmwareType) -> bool {
    if let Some(guard) = get_manager() {
        if let Some(mgr) = guard.as_ref() {
            return mgr.is_firmware_loaded(fw_type);
        }
    }
    false
}

/// Get firmware status
pub fn get_status() -> String {
    if let Some(guard) = get_manager() {
        if let Some(mgr) = guard.as_ref() {
            return mgr.get_status();
        }
    }
    "NVIDIA Firmware: Not initialized".to_string()
}

/// Firmware paths for common chips
pub mod firmware_paths {
    /// Kepler
    pub const GK104_PMU: &str = "/lib/firmware/nouveau/nvac_pmu.bin";
    pub const GK104_FECS: &str = "/lib/firmware/nouveau/nvac_fecs.bin";
    pub const GK104_GPCCS: &str = "/lib/firmware/nouveau/nvac_gpccs.bin";

    /// Maxwell
    pub const GM204_PMU: &str = "/lib/firmware/nvidia/gm204/pmu.bin";
    pub const GM204_FECS: &str = "/lib/firmware/nvidia/gm204/gr/fecs.bin";
    pub const GM204_GPCCS: &str = "/lib/firmware/nvidia/gm204/gr/gpccs.bin";
    pub const GM204_SEC2: &str = "/lib/firmware/nvidia/gm204/sec2.bin";

    /// Pascal
    pub const GP102_PMU: &str = "/lib/firmware/nvidia/gp102/pmu.bin";
    pub const GP102_SEC2: &str = "/lib/firmware/nvidia/gp102/sec2.bin";
    pub const GP102_ACRSEC: &str = "/lib/firmware/nvidia/gp102/acr/bl.bin";
    pub const GP102_GRFECS: &str = "/lib/firmware/nvidia/gp102/gr/fecs_bl.bin";
    pub const GP102_GRGPCCS: &str = "/lib/firmware/nvidia/gp102/gr/gpccs_bl.bin";

    /// Turing
    pub const TU102_GSP: &str = "/lib/firmware/nvidia/tu102/gsp/gsp.bin";
    pub const TU102_SEC2: &str = "/lib/firmware/nvidia/tu102/sec2/hs_bl_sig.bin";
    pub const TU102_PMU: &str = "/lib/firmware/nvidia/tu102/pmu/hs_bl_sig.bin";

    /// Ampere
    pub const GA102_GSP: &str = "/lib/firmware/nvidia/ga102/gsp/gsp.bin";

    /// Ada
    pub const AD102_GSP: &str = "/lib/firmware/nvidia/ad102/gsp/gsp.bin";
}
