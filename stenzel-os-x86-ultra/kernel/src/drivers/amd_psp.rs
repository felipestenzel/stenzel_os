//! AMD Platform Security Processor Interface
//!
//! Provides interface to AMD PSP (Secure Processor):
//! - PSP mailbox communication
//! - Firmware version query
//! - fTPM (firmware TPM) interface
//! - Secure boot state query
//! - PSP capabilities detection

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::sync::IrqSafeMutex;
use crate::util::{KResult, KError};

/// PSP state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PspState {
    /// PSP not present
    NotPresent,
    /// PSP initializing
    Initializing,
    /// PSP ready
    Ready,
    /// PSP in secure boot mode
    SecureBoot,
    /// PSP error
    Error,
    /// Unknown state
    Unknown,
}

/// PSP capability flags
#[derive(Debug, Clone, Copy, Default)]
pub struct PspCapabilities {
    /// fTPM support
    pub ftpm: bool,
    /// Secure boot support
    pub secure_boot: bool,
    /// SEV (Secure Encrypted Virtualization)
    pub sev: bool,
    /// SEV-ES (Encrypted State)
    pub sev_es: bool,
    /// SEV-SNP (Secure Nested Paging)
    pub sev_snp: bool,
    /// SME (Secure Memory Encryption)
    pub sme: bool,
    /// TSME (Transparent SME)
    pub tsme: bool,
    /// Debug unlock capability
    pub debug_unlock: bool,
    /// PSP recovery mode
    pub recovery: bool,
}

/// PSP firmware version
#[derive(Debug, Clone, Default)]
pub struct PspVersion {
    /// Major version
    pub major: u8,
    /// Minor version
    pub minor: u8,
    /// Revision
    pub revision: u8,
    /// Build number
    pub build: u16,
}

impl PspVersion {
    pub fn to_string(&self) -> String {
        alloc::format!("{}.{}.{}.{}", self.major, self.minor, self.revision, self.build)
    }
}

/// PSP security state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PspSecurityState {
    /// Debug mode (not secure)
    Debug,
    /// Secure mode
    Secure,
    /// Secure mode with customer key
    SecureCustomerKey,
    /// Unknown
    Unknown,
}

/// fTPM state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FtpmState {
    /// Not present
    NotPresent,
    /// Disabled in BIOS
    Disabled,
    /// Initializing
    Initializing,
    /// Ready for commands
    Ready,
    /// Error state
    Error,
}

/// SEV state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SevState {
    /// Not supported
    NotSupported,
    /// Supported but not initialized
    Uninitialized,
    /// Initialized and ready
    Initialized,
    /// In use by guest
    Active,
    /// Error
    Error,
}

/// PSP command result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum PspCmdResult {
    /// Success
    Success = 0,
    /// Invalid command
    InvalidCommand = 1,
    /// Invalid param
    InvalidParam = 2,
    /// Invalid address
    InvalidAddress = 3,
    /// Invalid length
    InvalidLength = 4,
    /// Resource busy
    Busy = 5,
    /// Hardware error
    HardwareError = 6,
    /// Security violation
    SecurityViolation = 7,
    /// Unknown error
    Unknown = 0xFF,
}

/// PSP mailbox commands
#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub enum PspMailboxCmd {
    /// Get PSP version
    GetVersion = 0x01,
    /// Get fTPM info
    GetFtpmInfo = 0x10,
    /// fTPM send command
    FtpmSendCmd = 0x11,
    /// fTPM receive response
    FtpmRecvResp = 0x12,
    /// Get SEV info
    GetSevInfo = 0x20,
    /// SEV init
    SevInit = 0x21,
    /// SEV shutdown
    SevShutdown = 0x22,
    /// Get platform status
    GetPlatformStatus = 0x30,
    /// Get capabilities
    GetCapabilities = 0x31,
}

/// PSP mailbox register offsets
mod psp_regs {
    /// Mailbox command register
    pub const MBOX_CMD: u32 = 0x10570;
    /// Mailbox status register
    pub const MBOX_STATUS: u32 = 0x10574;
    /// Mailbox command parameter 0
    pub const MBOX_PARAM0: u32 = 0x10578;
    /// Mailbox command parameter 1
    pub const MBOX_PARAM1: u32 = 0x1057C;
    /// Mailbox command parameter 2
    pub const MBOX_PARAM2: u32 = 0x10580;
    /// Mailbox response
    pub const MBOX_RESP: u32 = 0x10584;
    /// PSP feature register
    pub const PSP_FEATURE: u32 = 0x10590;
    /// PSP capability register
    pub const PSP_CAP: u32 = 0x10594;
}

/// Mailbox status bits
mod mbox_status {
    /// Command ready to execute
    pub const CMD_READY: u32 = 1 << 0;
    /// Command in progress
    pub const CMD_RUNNING: u32 = 1 << 1;
    /// Command complete
    pub const CMD_COMPLETE: u32 = 1 << 2;
    /// Error occurred
    pub const ERROR: u32 = 1 << 31;
    /// Result code mask
    pub const RESULT_MASK: u32 = 0x00FF0000;
    /// Result code shift
    pub const RESULT_SHIFT: u32 = 16;
}

/// PSP statistics
#[derive(Debug, Default)]
pub struct PspStats {
    /// Commands sent
    pub commands_sent: AtomicU64,
    /// Commands completed
    pub commands_completed: AtomicU64,
    /// Command errors
    pub command_errors: AtomicU64,
    /// fTPM commands
    pub ftpm_commands: AtomicU64,
    /// SEV operations
    pub sev_operations: AtomicU64,
}

/// PSP status information
#[derive(Debug, Clone)]
pub struct PspStatus {
    /// Current state
    pub state: PspState,
    /// Firmware version
    pub version: PspVersion,
    /// Security state
    pub security_state: PspSecurityState,
    /// Capabilities
    pub capabilities: PspCapabilities,
    /// fTPM state
    pub ftpm_state: FtpmState,
    /// SEV state
    pub sev_state: SevState,
    /// Platform ID
    pub platform_id: u32,
    /// Error code (if any)
    pub error_code: u32,
}

impl Default for PspStatus {
    fn default() -> Self {
        PspStatus {
            state: PspState::Unknown,
            version: PspVersion::default(),
            security_state: PspSecurityState::Unknown,
            capabilities: PspCapabilities::default(),
            ftpm_state: FtpmState::NotPresent,
            sev_state: SevState::NotSupported,
            platform_id: 0,
            error_code: 0,
        }
    }
}

/// AMD PSP Manager
pub struct AmdPspManager {
    /// SMN (System Management Network) base address
    smn_base: u64,
    /// PSP mailbox MMIO base
    mbox_base: u64,
    /// Current status
    status: PspStatus,
    /// PSP present
    present: bool,
    /// Initialized
    initialized: bool,
    /// Statistics
    stats: PspStats,
}

pub static PSP_MANAGER: IrqSafeMutex<AmdPspManager> = IrqSafeMutex::new(AmdPspManager::new());

impl AmdPspManager {
    pub const fn new() -> Self {
        AmdPspManager {
            smn_base: 0,
            mbox_base: 0,
            status: PspStatus {
                state: PspState::Unknown,
                version: PspVersion {
                    major: 0,
                    minor: 0,
                    revision: 0,
                    build: 0,
                },
                security_state: PspSecurityState::Unknown,
                capabilities: PspCapabilities {
                    ftpm: false,
                    secure_boot: false,
                    sev: false,
                    sev_es: false,
                    sev_snp: false,
                    sme: false,
                    tsme: false,
                    debug_unlock: false,
                    recovery: false,
                },
                ftpm_state: FtpmState::NotPresent,
                sev_state: SevState::NotSupported,
                platform_id: 0,
                error_code: 0,
            },
            present: false,
            initialized: false,
            stats: PspStats {
                commands_sent: AtomicU64::new(0),
                commands_completed: AtomicU64::new(0),
                command_errors: AtomicU64::new(0),
                ftpm_commands: AtomicU64::new(0),
                sev_operations: AtomicU64::new(0),
            },
        }
    }

    /// Initialize PSP interface
    pub fn init(&mut self) -> KResult<()> {
        if self.initialized {
            return Ok(());
        }

        // Check if running on AMD CPU
        if !self.detect_amd_cpu() {
            self.status.state = PspState::NotPresent;
            return Ok(());
        }

        // Find PSP device via SMN
        if !self.probe_psp_device()? {
            self.status.state = PspState::NotPresent;
            return Ok(());
        }

        self.present = true;

        // Query PSP version
        self.query_version()?;

        // Query capabilities
        self.query_capabilities()?;

        // Query platform status
        self.query_platform_status()?;

        // Initialize fTPM if available
        if self.status.capabilities.ftpm {
            self.init_ftpm()?;
        }

        // Check SEV support
        if self.status.capabilities.sev {
            self.check_sev_status()?;
        }

        self.status.state = PspState::Ready;
        self.initialized = true;

        crate::kprintln!("amd_psp: initialized, FW version {}", self.status.version.to_string());
        Ok(())
    }

    /// Detect if running on AMD CPU
    fn detect_amd_cpu(&self) -> bool {
        // Check CPUID for AMD
        let cpuid = unsafe { core::arch::x86_64::__cpuid(0) };
        let vendor = [
            cpuid.ebx.to_le_bytes(),
            cpuid.edx.to_le_bytes(),
            cpuid.ecx.to_le_bytes(),
        ];

        let vendor_str = [
            vendor[0][0], vendor[0][1], vendor[0][2], vendor[0][3],
            vendor[1][0], vendor[1][1], vendor[1][2], vendor[1][3],
            vendor[2][0], vendor[2][1], vendor[2][2], vendor[2][3],
        ];

        // "AuthenticAMD"
        &vendor_str == b"AuthenticAMD"
    }

    /// Probe for PSP device
    fn probe_psp_device(&mut self) -> KResult<bool> {
        // PSP is typically accessible via Root Complex
        // Look for AMD FCH or Northbridge

        let devices = crate::drivers::pci::scan();

        for device in devices {
            // AMD vendor ID
            if device.id.vendor_id != 0x1022 {
                continue;
            }

            // PSP-related device IDs
            let psp_devices: [u16; 12] = [
                0x1456, // Zen PSP (crypto coprocessor)
                0x1468, // Zen PSP
                0x1486, // Starship PSP
                0x1498, // Zen 2 PSP
                0x149A, // Zen 2 PSP
                0x14CA, // Zen 3 PSP
                0x14CE, // Zen 3 PSP
                0x15DF, // Raven Ridge PSP
                0x1649, // Renoir PSP
                0x1578, // Carrizo PSP
                0x1537, // Kabini PSP
                0x156B, // Stoney Ridge PSP
            ];

            if psp_devices.contains(&device.id.device_id) {
                // Get BAR0 for PSP MMIO
                let (bar_base, is_io) = crate::drivers::pci::read_bar(&device, 0);
                if !is_io && bar_base != 0 {
                    self.mbox_base = bar_base;
                    crate::kprintln!("amd_psp: found PSP device at {:02x}:{:02x}.{} MMIO {:x}",
                        device.addr.bus, device.addr.device, device.addr.function, bar_base);
                    return Ok(true);
                }
            }

            // Also check for crypto coprocessor class (0x10 subclass 0x80)
            if device.class.class_code == 0x10 && device.class.subclass == 0x80 {
                let (bar_base, is_io) = crate::drivers::pci::read_bar(&device, 0);
                if !is_io && bar_base != 0 {
                    self.mbox_base = bar_base;
                    return Ok(true);
                }
            }
        }

        // Try direct SMN access (for newer platforms)
        self.probe_via_smn()
    }

    /// Probe PSP via SMN (System Management Network)
    fn probe_via_smn(&mut self) -> KResult<bool> {
        // SMN is accessed via Root Complex registers
        // This is platform-specific and requires FCH BAR
        // For now, return false if not found via PCI
        Ok(false)
    }

    /// Query PSP firmware version
    fn query_version(&mut self) -> KResult<()> {
        let result = self.send_mailbox_cmd(PspMailboxCmd::GetVersion, 0, 0, 0)?;

        // Parse version from response
        self.status.version.major = ((result >> 24) & 0xFF) as u8;
        self.status.version.minor = ((result >> 16) & 0xFF) as u8;
        self.status.version.revision = ((result >> 8) & 0xFF) as u8;
        self.status.version.build = (result & 0xFF) as u16;

        Ok(())
    }

    /// Query PSP capabilities
    fn query_capabilities(&mut self) -> KResult<()> {
        let result = self.send_mailbox_cmd(PspMailboxCmd::GetCapabilities, 0, 0, 0)?;

        // Parse capability bits
        self.status.capabilities.ftpm = (result & (1 << 0)) != 0;
        self.status.capabilities.secure_boot = (result & (1 << 1)) != 0;
        self.status.capabilities.sev = (result & (1 << 2)) != 0;
        self.status.capabilities.sev_es = (result & (1 << 3)) != 0;
        self.status.capabilities.sev_snp = (result & (1 << 4)) != 0;
        self.status.capabilities.sme = (result & (1 << 5)) != 0;
        self.status.capabilities.tsme = (result & (1 << 6)) != 0;
        self.status.capabilities.debug_unlock = (result & (1 << 8)) != 0;
        self.status.capabilities.recovery = (result & (1 << 9)) != 0;

        // Also check CPUID for SME/SEV support
        self.detect_sme_sev_cpuid();

        Ok(())
    }

    /// Detect SME/SEV via CPUID
    fn detect_sme_sev_cpuid(&mut self) {
        // CPUID leaf 0x8000001F for AMD memory encryption
        let cpuid = unsafe { core::arch::x86_64::__cpuid(0x80000000) };
        if cpuid.eax >= 0x8000001F {
            let enc_cpuid = unsafe { core::arch::x86_64::__cpuid(0x8000001F) };

            // EAX bit 0: SME, bit 1: SEV, bit 3: SEV-ES, bit 4: SEV-SNP
            self.status.capabilities.sme = self.status.capabilities.sme || (enc_cpuid.eax & 1) != 0;
            self.status.capabilities.sev = self.status.capabilities.sev || (enc_cpuid.eax & 2) != 0;
            self.status.capabilities.sev_es = self.status.capabilities.sev_es || (enc_cpuid.eax & 8) != 0;
            self.status.capabilities.sev_snp = self.status.capabilities.sev_snp || (enc_cpuid.eax & 16) != 0;
        }
    }

    /// Query platform status
    fn query_platform_status(&mut self) -> KResult<()> {
        let result = self.send_mailbox_cmd(PspMailboxCmd::GetPlatformStatus, 0, 0, 0)?;

        // Parse platform status
        let state_bits = (result >> 24) & 0xFF;
        self.status.security_state = match state_bits {
            0 => PspSecurityState::Debug,
            1 => PspSecurityState::Secure,
            2 => PspSecurityState::SecureCustomerKey,
            _ => PspSecurityState::Unknown,
        };

        self.status.platform_id = result & 0xFFFF;
        Ok(())
    }

    /// Initialize fTPM
    fn init_ftpm(&mut self) -> KResult<()> {
        self.status.ftpm_state = FtpmState::Initializing;

        let result = self.send_mailbox_cmd(PspMailboxCmd::GetFtpmInfo, 0, 0, 0)?;

        if result != 0 {
            self.status.ftpm_state = FtpmState::Ready;
            crate::kprintln!("amd_psp: fTPM initialized");
        } else {
            self.status.ftpm_state = FtpmState::Disabled;
        }

        Ok(())
    }

    /// Check SEV status
    fn check_sev_status(&mut self) -> KResult<()> {
        let result = self.send_mailbox_cmd(PspMailboxCmd::GetSevInfo, 0, 0, 0)?;

        let state_bits = (result >> 24) & 0xFF;
        self.status.sev_state = match state_bits {
            0 => SevState::Uninitialized,
            1 => SevState::Initialized,
            2 => SevState::Active,
            _ => SevState::NotSupported,
        };

        Ok(())
    }

    /// Send mailbox command to PSP
    fn send_mailbox_cmd(&mut self, cmd: PspMailboxCmd, param0: u32, param1: u32, param2: u32) -> KResult<u32> {
        if self.mbox_base == 0 {
            return Err(KError::NotFound);
        }

        // Wait for mailbox ready
        for _ in 0..10000 {
            let status = self.read_mbox_reg(psp_regs::MBOX_STATUS);
            if (status & mbox_status::CMD_RUNNING) == 0 {
                break;
            }
            for _ in 0..100 { core::hint::spin_loop(); }
        }

        // Write parameters
        self.write_mbox_reg(psp_regs::MBOX_PARAM0, param0);
        self.write_mbox_reg(psp_regs::MBOX_PARAM1, param1);
        self.write_mbox_reg(psp_regs::MBOX_PARAM2, param2);

        // Write command to start execution
        self.write_mbox_reg(psp_regs::MBOX_CMD, cmd as u32 | mbox_status::CMD_READY);

        self.stats.commands_sent.fetch_add(1, Ordering::Relaxed);

        // Wait for completion
        for _ in 0..100000 {
            let status = self.read_mbox_reg(psp_regs::MBOX_STATUS);

            if (status & mbox_status::CMD_COMPLETE) != 0 {
                // Check for error
                if (status & mbox_status::ERROR) != 0 {
                    self.stats.command_errors.fetch_add(1, Ordering::Relaxed);
                    let result_code = (status & mbox_status::RESULT_MASK) >> mbox_status::RESULT_SHIFT;
                    self.status.error_code = result_code;
                    return Err(KError::IO);
                }

                self.stats.commands_completed.fetch_add(1, Ordering::Relaxed);

                // Read response
                let response = self.read_mbox_reg(psp_regs::MBOX_RESP);
                return Ok(response);
            }

            for _ in 0..100 { core::hint::spin_loop(); }
        }

        // Timeout
        self.stats.command_errors.fetch_add(1, Ordering::Relaxed);
        Err(KError::Timeout)
    }

    /// Read mailbox register
    fn read_mbox_reg(&self, offset: u32) -> u32 {
        if self.mbox_base == 0 {
            return 0;
        }
        unsafe {
            let ptr = (self.mbox_base + offset as u64) as *const u32;
            core::ptr::read_volatile(ptr)
        }
    }

    /// Write mailbox register
    fn write_mbox_reg(&self, offset: u32, value: u32) {
        if self.mbox_base == 0 {
            return;
        }
        unsafe {
            let ptr = (self.mbox_base + offset as u64) as *mut u32;
            core::ptr::write_volatile(ptr, value);
        }
    }

    /// Send fTPM command
    pub fn ftpm_send_command(&mut self, cmd: &[u8]) -> KResult<Vec<u8>> {
        if self.status.ftpm_state != FtpmState::Ready {
            return Err(KError::NotSupported);
        }

        // fTPM commands are sent via shared memory
        // For now, return placeholder
        self.stats.ftpm_commands.fetch_add(1, Ordering::Relaxed);

        Ok(Vec::new())
    }

    /// Initialize SEV
    pub fn sev_init(&mut self) -> KResult<()> {
        if !self.status.capabilities.sev {
            return Err(KError::NotSupported);
        }

        self.send_mailbox_cmd(PspMailboxCmd::SevInit, 0, 0, 0)?;
        self.status.sev_state = SevState::Initialized;
        self.stats.sev_operations.fetch_add(1, Ordering::Relaxed);

        crate::kprintln!("amd_psp: SEV initialized");
        Ok(())
    }

    /// Shutdown SEV
    pub fn sev_shutdown(&mut self) -> KResult<()> {
        if self.status.sev_state != SevState::Initialized {
            return Ok(());
        }

        self.send_mailbox_cmd(PspMailboxCmd::SevShutdown, 0, 0, 0)?;
        self.status.sev_state = SevState::Uninitialized;
        self.stats.sev_operations.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }

    /// Get current status
    pub fn status(&self) -> &PspStatus {
        &self.status
    }

    /// Get PSP state
    pub fn state(&self) -> PspState {
        self.status.state
    }

    /// Get firmware version
    pub fn version(&self) -> &PspVersion {
        &self.status.version
    }

    /// Get capabilities
    pub fn capabilities(&self) -> &PspCapabilities {
        &self.status.capabilities
    }

    /// Check if PSP is present
    pub fn is_present(&self) -> bool {
        self.present
    }

    /// Check if fTPM is available
    pub fn is_ftpm_available(&self) -> bool {
        self.status.ftpm_state == FtpmState::Ready
    }

    /// Check if SEV is supported
    pub fn is_sev_supported(&self) -> bool {
        self.status.capabilities.sev
    }

    /// Check if SME is supported
    pub fn is_sme_supported(&self) -> bool {
        self.status.capabilities.sme
    }

    /// Get statistics
    pub fn stats(&self) -> &PspStats {
        &self.stats
    }

    /// Refresh status
    pub fn refresh_status(&mut self) -> KResult<()> {
        if !self.present {
            return Ok(());
        }
        self.query_platform_status()
    }
}

/// Initialize AMD PSP interface
pub fn init() -> KResult<()> {
    PSP_MANAGER.lock().init()
}

/// Check if PSP is present
pub fn is_present() -> bool {
    PSP_MANAGER.lock().is_present()
}

/// Get PSP state
pub fn state() -> PspState {
    PSP_MANAGER.lock().state()
}

/// Get firmware version string
pub fn version_string() -> String {
    PSP_MANAGER.lock().version().to_string()
}

/// Check if fTPM is available
pub fn is_ftpm_available() -> bool {
    PSP_MANAGER.lock().is_ftpm_available()
}

/// Check if SEV is supported
pub fn is_sev_supported() -> bool {
    PSP_MANAGER.lock().is_sev_supported()
}

/// Check if SME is supported
pub fn is_sme_supported() -> bool {
    PSP_MANAGER.lock().is_sme_supported()
}
