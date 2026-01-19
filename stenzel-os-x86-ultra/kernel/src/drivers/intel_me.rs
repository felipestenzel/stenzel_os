//! Intel Management Engine Interface
//!
//! Provides basic interface to Intel ME/CSME:
//! - MEI (Management Engine Interface) driver
//! - HECI (Host Embedded Controller Interface)
//! - Basic ME commands and status
//! - Firmware version query
//! - ME state monitoring

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use crate::sync::IrqSafeMutex;
use crate::util::{KResult, KError};

/// Intel ME state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeState {
    /// ME is not present
    NotPresent,
    /// ME is initializing
    Initializing,
    /// ME is ready/operational
    Ready,
    /// ME is in recovery mode
    Recovery,
    /// ME is disabled
    Disabled,
    /// ME error state
    Error,
    /// Unknown state
    Unknown,
}

/// ME operation mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeOperationMode {
    /// Normal operation
    Normal,
    /// Debug mode
    Debug,
    /// Soft temporary disable
    SoftTempDisable,
    /// Security override jumper
    SecurityOverride,
    /// Enhanced debug mode
    EnhancedDebug,
}

/// ME firmware type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeFirmwareType {
    /// Consumer firmware
    Consumer,
    /// Corporate/vPro firmware
    Corporate,
    /// Server firmware
    Server,
    /// Unknown type
    Unknown,
}

/// MEI client protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeiClient {
    /// AMT (Active Management Technology)
    Amt,
    /// MKHI (Management Kernel Host Interface)
    Mkhi,
    /// ICC (Integrated Clock Controller)
    Icc,
    /// Hotham (for boot device)
    Hotham,
    /// PAVP (Protected Audio Video Path)
    Pavp,
    /// WDT (Watchdog Timer)
    Wdt,
    /// FWUpdate
    FwUpdate,
    /// Unknown client
    Unknown,
}

/// ME firmware version
#[derive(Debug, Clone, Default)]
pub struct MeVersion {
    /// Major version
    pub major: u16,
    /// Minor version
    pub minor: u16,
    /// Hotfix version
    pub hotfix: u16,
    /// Build number
    pub build: u16,
}

impl MeVersion {
    pub fn to_string(&self) -> String {
        alloc::format!("{}.{}.{}.{}", self.major, self.minor, self.hotfix, self.build)
    }
}

/// ME capabilities
#[derive(Debug, Clone, Default)]
pub struct MeCapabilities {
    /// AMT support
    pub amt: bool,
    /// Remote KVM
    pub kvm: bool,
    /// IDE-R (IDE Redirection)
    pub ide_r: bool,
    /// SOL (Serial over LAN)
    pub sol: bool,
    /// Boot Guard support
    pub boot_guard: bool,
    /// PTT (Platform Trust Technology)
    pub ptt: bool,
    /// Intel TXT support
    pub txt: bool,
    /// vPro support
    pub vpro: bool,
    /// Standard Manageability
    pub standard_manageability: bool,
    /// Remote configuration
    pub remote_config: bool,
}

/// ME status information
#[derive(Debug, Clone)]
pub struct MeStatus {
    /// Current state
    pub state: MeState,
    /// Operation mode
    pub operation_mode: MeOperationMode,
    /// Firmware type
    pub firmware_type: MeFirmwareType,
    /// Firmware version
    pub version: MeVersion,
    /// Platform configuration
    pub platform_id: u32,
    /// ME manufacturing mode
    pub manufacturing_mode: bool,
    /// Flash partition table valid
    pub fpt_valid: bool,
    /// ME update in progress
    pub update_in_progress: bool,
    /// Error code if any
    pub error_code: u32,
    /// Capabilities
    pub capabilities: MeCapabilities,
}

impl Default for MeStatus {
    fn default() -> Self {
        MeStatus {
            state: MeState::Unknown,
            operation_mode: MeOperationMode::Normal,
            firmware_type: MeFirmwareType::Unknown,
            version: MeVersion::default(),
            platform_id: 0,
            manufacturing_mode: false,
            fpt_valid: false,
            update_in_progress: false,
            error_code: 0,
            capabilities: MeCapabilities::default(),
        }
    }
}

/// MEI message header
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct MeiMsgHeader {
    /// Client address (ME side)
    pub me_addr: u8,
    /// Host address
    pub host_addr: u8,
    /// Reserved
    pub reserved: u8,
    /// Message length
    pub length: u16,
    /// Message complete flag
    pub msg_complete: u8,
}

/// HECI registers
mod heci_regs {
    /// Host control status register
    pub const H_CSR: u32 = 0x04;
    /// ME control status register
    pub const ME_CSR: u32 = 0x0C;
    /// Circular buffer depth
    pub const H_CB_WW: u32 = 0x00;
    /// Host read window
    pub const ME_CB_RW: u32 = 0x08;
    /// Host interrupt status
    pub const H_IS: u32 = 0x14;
    /// ME interrupt enable
    pub const ME_IE: u32 = 0x18;
    /// Host general status
    pub const H_GS: u32 = 0x4C;
    /// ME general status (FWSTS)
    pub const ME_GS: u32 = 0x40;
}

/// HECI CSR bits
mod heci_csr {
    /// Interrupt enable
    pub const IE: u32 = 1 << 0;
    /// Interrupt status
    pub const IS: u32 = 1 << 1;
    /// Interrupt generate
    pub const IG: u32 = 1 << 2;
    /// Ready
    pub const READY: u32 = 1 << 3;
    /// Reset
    pub const RESET: u32 = 1 << 4;
    /// Circular buffer read pointer mask
    pub const CBD_MASK: u32 = 0xFF << 24;
}

/// ME firmware status registers
mod fwsts {
    /// Working state
    pub const WORKING_STATE_MASK: u32 = 0xF;
    /// Operation state
    pub const OP_STATE_MASK: u32 = 0x7 << 4;
    /// ME FW error
    pub const FW_ERROR: u32 = 1 << 8;
    /// Operation mode
    pub const OP_MODE_MASK: u32 = 0xF << 16;
    /// Manufacturing mode
    pub const MFG_MODE: u32 = 1 << 4;
    /// FPT bad
    pub const FPT_BAD: u32 = 1 << 5;
    /// Update in progress
    pub const UPDATE_IN_PROGRESS: u32 = 1 << 11;
}

/// MEI statistics
#[derive(Debug, Default)]
pub struct MeiStats {
    /// Total messages sent
    pub messages_sent: AtomicU64,
    /// Total messages received
    pub messages_received: AtomicU64,
    /// Total bytes sent
    pub bytes_sent: AtomicU64,
    /// Total bytes received
    pub bytes_received: AtomicU64,
    /// Errors
    pub errors: AtomicU64,
    /// Resets
    pub resets: AtomicU64,
}

/// Intel ME Interface manager
pub struct IntelMeManager {
    /// Base MMIO address
    mmio_base: u64,
    /// PCI device location
    pci_location: Option<(u8, u8, u8)>,
    /// Current status
    status: MeStatus,
    /// MEI initialized
    initialized: bool,
    /// MEI enabled
    enabled: AtomicBool,
    /// Host address counter
    host_addr_counter: AtomicU32,
    /// Statistics
    stats: MeiStats,
    /// Connected clients
    clients: Vec<MeiClientConnection>,
}

/// MEI client connection
#[derive(Debug, Clone)]
pub struct MeiClientConnection {
    /// Client type
    pub client: MeiClient,
    /// ME address
    pub me_addr: u8,
    /// Host address
    pub host_addr: u8,
    /// Max message length
    pub max_msg_len: u32,
    /// Connected
    pub connected: bool,
}

pub static ME_MANAGER: IrqSafeMutex<IntelMeManager> = IrqSafeMutex::new(IntelMeManager::new());

impl IntelMeManager {
    pub const fn new() -> Self {
        IntelMeManager {
            mmio_base: 0,
            pci_location: None,
            status: MeStatus {
                state: MeState::Unknown,
                operation_mode: MeOperationMode::Normal,
                firmware_type: MeFirmwareType::Unknown,
                version: MeVersion {
                    major: 0,
                    minor: 0,
                    hotfix: 0,
                    build: 0,
                },
                platform_id: 0,
                manufacturing_mode: false,
                fpt_valid: false,
                update_in_progress: false,
                error_code: 0,
                capabilities: MeCapabilities {
                    amt: false,
                    kvm: false,
                    ide_r: false,
                    sol: false,
                    boot_guard: false,
                    ptt: false,
                    txt: false,
                    vpro: false,
                    standard_manageability: false,
                    remote_config: false,
                },
            },
            initialized: false,
            enabled: AtomicBool::new(false),
            host_addr_counter: AtomicU32::new(1),
            stats: MeiStats {
                messages_sent: AtomicU64::new(0),
                messages_received: AtomicU64::new(0),
                bytes_sent: AtomicU64::new(0),
                bytes_received: AtomicU64::new(0),
                errors: AtomicU64::new(0),
                resets: AtomicU64::new(0),
            },
            clients: Vec::new(),
        }
    }

    /// Initialize Intel ME interface
    pub fn init(&mut self) -> KResult<()> {
        if self.initialized {
            return Ok(());
        }

        // Find MEI/HECI device on PCI
        if !self.probe_mei_device()? {
            self.status.state = MeState::NotPresent;
            return Ok(());
        }

        // Read firmware status
        self.read_firmware_status()?;

        // Initialize HECI interface
        self.init_heci()?;

        // Query firmware version
        if self.status.state == MeState::Ready {
            self.query_firmware_version()?;
            self.query_capabilities()?;
        }

        self.initialized = true;
        self.enabled.store(true, Ordering::SeqCst);

        crate::kprintln!("intel_me: initialized, FW version {}", self.status.version.to_string());
        Ok(())
    }

    /// Probe for MEI/HECI PCI device
    fn probe_mei_device(&mut self) -> KResult<bool> {
        // MEI device IDs (Intel vendor 0x8086)
        // Various PCH generations have different device IDs
        let mei_device_ids: [u16; 17] = [
            0x1E3A, // Panther Point
            0x8C3A, // Lynx Point
            0x9C3A, // Lynx Point LP
            0x9CBA, // Wildcat Point LP
            0xA13A, // Sunrise Point
            0x9D3A, // Sunrise Point LP
            0xA2BA, // Union Point
            0xA360, // Cannon Point
            0x02E0, // Comet Lake
            0x43E0, // Tiger Lake
            0xA0E0, // Tiger Lake LP
            0x7AE8, // Alder Lake
            0x7E70, // Meteor Lake
            0xA328, // Cannon Lake
            0x9DE0, // Cannon Lake LP
            0x34E0, // Ice Lake LP
            0x4DE0, // Jasper Lake
        ];

        let devices = crate::drivers::pci::scan();

        for device in devices {
            if device.id.vendor_id != 0x8086 {
                continue;
            }

            // Check if it's a MEI device
            let is_mei = mei_device_ids.contains(&device.id.device_id)
                || (device.class.class_code == 0x07
                    && device.class.subclass == 0x80);

            if is_mei {
                self.pci_location = Some((device.addr.bus, device.addr.device, device.addr.function));

                // Get MMIO base from BAR0
                let (bar_base, is_io) = crate::drivers::pci::read_bar(&device, 0);
                if !is_io && bar_base != 0 {
                    self.mmio_base = bar_base;
                    crate::kprintln!("intel_me: found MEI device at {:02x}:{:02x}.{} MMIO {:x}",
                        device.addr.bus, device.addr.device, device.addr.function, self.mmio_base);
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Read firmware status from FWSTS registers
    fn read_firmware_status(&mut self) -> KResult<()> {
        if self.mmio_base == 0 {
            return Err(KError::NotFound);
        }

        // Read FWSTS1 (ME_GS)
        let fwsts1 = self.read_reg(heci_regs::ME_GS);

        // Parse working state
        let working_state = fwsts1 & fwsts::WORKING_STATE_MASK;
        self.status.state = match working_state {
            0 => MeState::Initializing,
            5 => MeState::Ready,
            6 => MeState::Recovery,
            7 => MeState::Error,
            _ => MeState::Unknown,
        };

        // Parse operation mode
        let op_mode = (fwsts1 & fwsts::OP_MODE_MASK) >> 16;
        self.status.operation_mode = match op_mode {
            0 => MeOperationMode::Normal,
            1 => MeOperationMode::Debug,
            3 => MeOperationMode::SoftTempDisable,
            4 => MeOperationMode::SecurityOverride,
            5 => MeOperationMode::EnhancedDebug,
            _ => MeOperationMode::Normal,
        };

        // Check flags
        self.status.manufacturing_mode = (fwsts1 & fwsts::MFG_MODE) != 0;
        self.status.fpt_valid = (fwsts1 & fwsts::FPT_BAD) == 0;
        self.status.update_in_progress = (fwsts1 & fwsts::UPDATE_IN_PROGRESS) != 0;

        if (fwsts1 & fwsts::FW_ERROR) != 0 {
            self.status.error_code = (fwsts1 >> 12) & 0xF;
        }

        Ok(())
    }

    /// Initialize HECI interface
    fn init_heci(&mut self) -> KResult<()> {
        if self.mmio_base == 0 {
            return Err(KError::NotFound);
        }

        // Read host CSR
        let h_csr = self.read_reg(heci_regs::H_CSR);

        // Check if ME is ready
        let me_csr = self.read_reg(heci_regs::ME_CSR);
        if (me_csr & heci_csr::READY) == 0 {
            // ME not ready, try reset
            self.reset_heci()?;
        }

        // Enable host interrupts
        let h_csr = h_csr | heci_csr::IE | heci_csr::READY;
        self.write_reg(heci_regs::H_CSR, h_csr);

        // Clear any pending interrupt
        self.write_reg(heci_regs::H_CSR, h_csr | heci_csr::IS);

        Ok(())
    }

    /// Reset HECI interface
    fn reset_heci(&mut self) -> KResult<()> {
        // Set reset bit
        let h_csr = self.read_reg(heci_regs::H_CSR);
        self.write_reg(heci_regs::H_CSR, h_csr | heci_csr::RESET);

        // Wait for reset complete
        for _ in 0..1000 {
            let me_csr = self.read_reg(heci_regs::ME_CSR);
            if (me_csr & heci_csr::READY) != 0 {
                break;
            }
            // Small delay
            for _ in 0..1000 { core::hint::spin_loop(); }
        }

        // Clear reset bit, set ready
        let h_csr = self.read_reg(heci_regs::H_CSR);
        self.write_reg(heci_regs::H_CSR, (h_csr & !heci_csr::RESET) | heci_csr::READY | heci_csr::IE);

        self.stats.resets.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Query firmware version via MKHI
    fn query_firmware_version(&mut self) -> KResult<()> {
        // MKHI GET_FW_VERSION command
        // Group ID: 0xFF (GEN)
        // Command: 0x02 (GET_FW_VERSION)
        let cmd: [u8; 4] = [0xFF, 0x02, 0x00, 0x00];

        let response = self.send_mkhi_command(&cmd)?;

        if response.len() >= 20 {
            // Parse version response
            self.status.version.major = u16::from_le_bytes([response[4], response[5]]);
            self.status.version.minor = u16::from_le_bytes([response[6], response[7]]);
            self.status.version.hotfix = u16::from_le_bytes([response[8], response[9]]);
            self.status.version.build = u16::from_le_bytes([response[10], response[11]]);
        }

        Ok(())
    }

    /// Query ME capabilities
    fn query_capabilities(&mut self) -> KResult<()> {
        // MKHI GET_FW_FEATURE_STATE command
        // Group ID: 0xFF (GEN)
        // Command: 0x20 (GET_FW_FEATURE_STATE)
        let cmd: [u8; 4] = [0xFF, 0x20, 0x00, 0x00];

        let response = self.send_mkhi_command(&cmd)?;

        if response.len() >= 8 {
            let features = u32::from_le_bytes([response[4], response[5], response[6], response[7]]);

            self.status.capabilities.amt = (features & (1 << 0)) != 0;
            self.status.capabilities.kvm = (features & (1 << 1)) != 0;
            self.status.capabilities.ide_r = (features & (1 << 2)) != 0;
            self.status.capabilities.sol = (features & (1 << 3)) != 0;
            self.status.capabilities.boot_guard = (features & (1 << 8)) != 0;
            self.status.capabilities.ptt = (features & (1 << 16)) != 0;
        }

        Ok(())
    }

    /// Send MKHI command and get response
    fn send_mkhi_command(&mut self, cmd: &[u8]) -> KResult<Vec<u8>> {
        // MKHI client address is typically 0x07
        let header = MeiMsgHeader {
            me_addr: 0x07,
            host_addr: 0x01,
            reserved: 0,
            length: cmd.len() as u16,
            msg_complete: 1,
        };

        self.send_message(&header, cmd)?;
        self.receive_message()
    }

    /// Send MEI message
    fn send_message(&mut self, header: &MeiMsgHeader, data: &[u8]) -> KResult<()> {
        if self.mmio_base == 0 {
            return Err(KError::NotFound);
        }

        // Check ME ready
        let me_csr = self.read_reg(heci_regs::ME_CSR);
        if (me_csr & heci_csr::READY) == 0 {
            return Err(KError::IO);
        }

        // Write header as first dword
        let header_bytes = unsafe {
            core::slice::from_raw_parts(
                header as *const MeiMsgHeader as *const u8,
                core::mem::size_of::<MeiMsgHeader>()
            )
        };

        // Write header and data to circular buffer
        let mut offset = 0;
        for chunk in header_bytes.chunks(4) {
            let mut dword = [0u8; 4];
            dword[..chunk.len()].copy_from_slice(chunk);
            self.write_reg(heci_regs::H_CB_WW, u32::from_le_bytes(dword));
            offset += 4;
        }

        for chunk in data.chunks(4) {
            let mut dword = [0u8; 4];
            dword[..chunk.len()].copy_from_slice(chunk);
            self.write_reg(heci_regs::H_CB_WW, u32::from_le_bytes(dword));
        }

        // Generate interrupt
        let h_csr = self.read_reg(heci_regs::H_CSR);
        self.write_reg(heci_regs::H_CSR, h_csr | heci_csr::IG);

        self.stats.messages_sent.fetch_add(1, Ordering::Relaxed);
        self.stats.bytes_sent.fetch_add((header_bytes.len() + data.len()) as u64, Ordering::Relaxed);

        Ok(())
    }

    /// Receive MEI message
    fn receive_message(&mut self) -> KResult<Vec<u8>> {
        if self.mmio_base == 0 {
            return Err(KError::NotFound);
        }

        // Wait for message
        for _ in 0..10000 {
            let me_csr = self.read_reg(heci_regs::ME_CSR);
            let cbd = (me_csr & heci_csr::CBD_MASK) >> 24;
            if cbd > 0 {
                break;
            }
            for _ in 0..100 { core::hint::spin_loop(); }
        }

        // Read header
        let header_dword = self.read_reg(heci_regs::ME_CB_RW);
        let length = ((header_dword >> 16) & 0x1FF) as usize;

        // Read data
        let mut data = Vec::with_capacity(length);
        let dwords = (length + 3) / 4;

        for _ in 0..dwords {
            let dword = self.read_reg(heci_regs::ME_CB_RW);
            let bytes = dword.to_le_bytes();
            for &b in &bytes {
                if data.len() < length {
                    data.push(b);
                }
            }
        }

        // Clear interrupt
        let h_csr = self.read_reg(heci_regs::H_CSR);
        self.write_reg(heci_regs::H_CSR, h_csr | heci_csr::IS);

        self.stats.messages_received.fetch_add(1, Ordering::Relaxed);
        self.stats.bytes_received.fetch_add(data.len() as u64, Ordering::Relaxed);

        Ok(data)
    }

    /// Read HECI register
    fn read_reg(&self, offset: u32) -> u32 {
        if self.mmio_base == 0 {
            return 0;
        }
        unsafe {
            let ptr = (self.mmio_base + offset as u64) as *const u32;
            core::ptr::read_volatile(ptr)
        }
    }

    /// Write HECI register
    fn write_reg(&self, offset: u32, value: u32) {
        if self.mmio_base == 0 {
            return;
        }
        unsafe {
            let ptr = (self.mmio_base + offset as u64) as *mut u32;
            core::ptr::write_volatile(ptr, value);
        }
    }

    /// Get current ME status
    pub fn status(&self) -> &MeStatus {
        &self.status
    }

    /// Get ME state
    pub fn state(&self) -> MeState {
        self.status.state
    }

    /// Get firmware version
    pub fn version(&self) -> &MeVersion {
        &self.status.version
    }

    /// Get capabilities
    pub fn capabilities(&self) -> &MeCapabilities {
        &self.status.capabilities
    }

    /// Check if ME is present
    pub fn is_present(&self) -> bool {
        self.status.state != MeState::NotPresent
    }

    /// Check if ME is ready
    pub fn is_ready(&self) -> bool {
        self.status.state == MeState::Ready
    }

    /// Check if AMT is enabled
    pub fn is_amt_enabled(&self) -> bool {
        self.status.capabilities.amt
    }

    /// Get statistics
    pub fn stats(&self) -> &MeiStats {
        &self.stats
    }

    /// Refresh status
    pub fn refresh_status(&mut self) -> KResult<()> {
        self.read_firmware_status()
    }

    /// Disable ME temporarily (if supported)
    pub fn disable_temp(&mut self) -> KResult<()> {
        // MKHI HMRFPO_ENABLE command for temp disable
        // This is a sensitive operation
        if !self.is_ready() {
            return Err(KError::NotSupported);
        }

        crate::kprintln!("intel_me: temporary disable requested");
        // Actual implementation would send HMRFPO_ENABLE
        Ok(())
    }
}

/// Initialize Intel ME interface
pub fn init() -> KResult<()> {
    ME_MANAGER.lock().init()
}

/// Check if ME is present
pub fn is_present() -> bool {
    ME_MANAGER.lock().is_present()
}

/// Check if ME is ready
pub fn is_ready() -> bool {
    ME_MANAGER.lock().is_ready()
}

/// Get ME state
pub fn state() -> MeState {
    ME_MANAGER.lock().state()
}

/// Get firmware version string
pub fn version_string() -> String {
    ME_MANAGER.lock().version().to_string()
}

/// Check if AMT is enabled
pub fn is_amt_enabled() -> bool {
    ME_MANAGER.lock().is_amt_enabled()
}
