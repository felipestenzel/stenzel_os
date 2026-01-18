//! Measured Boot Implementation
//!
//! Implements TPM-based measured boot to ensure system integrity by measuring
//! each boot component and extending PCRs (Platform Configuration Registers).
//!
//! ## PCR Allocation
//! - PCR 0: BIOS/UEFI firmware
//! - PCR 1: Platform configuration
//! - PCR 2: Option ROMs
//! - PCR 3: Option ROM configuration
//! - PCR 4: MBR/Boot loader (measured by firmware)
//! - PCR 5: Boot loader configuration
//! - PCR 6: Resume from S3
//! - PCR 7: Secure Boot policy
//! - PCR 8: Kernel image
//! - PCR 9: Kernel command line
//! - PCR 10: Reserved for IMA (Integrity Measurement Architecture)
//! - PCR 11: BitLocker (Windows) / custom use
//! - PCR 12-23: User-defined measurements
//!
//! ## Features
//! - Boot component measurement and verification
//! - PCR extension with SHA-256 digests
//! - Event log maintenance (TCG format)
//! - Sealed secrets support
//! - Remote attestation preparation
//! - Boot policy enforcement

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::sync::IrqSafeMutex;
use crate::util::{KError, KResult};
use super::tpm::{self, algorithms, TpmError};

/// PCR indices for measured boot
pub mod pcr_index {
    pub const PCR_FIRMWARE: u32 = 0;
    pub const PCR_PLATFORM_CONFIG: u32 = 1;
    pub const PCR_OPTION_ROMS: u32 = 2;
    pub const PCR_OPTION_ROM_CONFIG: u32 = 3;
    pub const PCR_BOOTLOADER: u32 = 4;
    pub const PCR_BOOTLOADER_CONFIG: u32 = 5;
    pub const PCR_RESUME: u32 = 6;
    pub const PCR_SECUREBOOT_POLICY: u32 = 7;
    pub const PCR_KERNEL: u32 = 8;
    pub const PCR_KERNEL_CMDLINE: u32 = 9;
    pub const PCR_IMA: u32 = 10;
    pub const PCR_CUSTOM_START: u32 = 12;
    pub const PCR_CUSTOM_END: u32 = 23;
}

/// Boot component type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootComponent {
    /// UEFI firmware
    Firmware,
    /// UEFI platform configuration
    PlatformConfig,
    /// Option ROMs
    OptionRom,
    /// Bootloader (GRUB, systemd-boot, etc.)
    Bootloader,
    /// Bootloader configuration
    BootloaderConfig,
    /// Kernel image
    Kernel,
    /// Kernel command line
    KernelCmdline,
    /// Initial ramdisk
    Initrd,
    /// Kernel modules
    KernelModule,
    /// Secure Boot policy
    SecureBootPolicy,
    /// User-defined component
    Custom(u32),
}

impl BootComponent {
    /// Get the PCR index for this component
    pub fn pcr_index(&self) -> u32 {
        match self {
            BootComponent::Firmware => pcr_index::PCR_FIRMWARE,
            BootComponent::PlatformConfig => pcr_index::PCR_PLATFORM_CONFIG,
            BootComponent::OptionRom => pcr_index::PCR_OPTION_ROMS,
            BootComponent::Bootloader => pcr_index::PCR_BOOTLOADER,
            BootComponent::BootloaderConfig => pcr_index::PCR_BOOTLOADER_CONFIG,
            BootComponent::Kernel => pcr_index::PCR_KERNEL,
            BootComponent::KernelCmdline => pcr_index::PCR_KERNEL_CMDLINE,
            BootComponent::Initrd => pcr_index::PCR_KERNEL,
            BootComponent::KernelModule => pcr_index::PCR_IMA,
            BootComponent::SecureBootPolicy => pcr_index::PCR_SECUREBOOT_POLICY,
            BootComponent::Custom(idx) => *idx,
        }
    }

    /// Get component name
    pub fn name(&self) -> &'static str {
        match self {
            BootComponent::Firmware => "Firmware",
            BootComponent::PlatformConfig => "Platform Config",
            BootComponent::OptionRom => "Option ROM",
            BootComponent::Bootloader => "Bootloader",
            BootComponent::BootloaderConfig => "Bootloader Config",
            BootComponent::Kernel => "Kernel",
            BootComponent::KernelCmdline => "Kernel Cmdline",
            BootComponent::Initrd => "Initrd",
            BootComponent::KernelModule => "Kernel Module",
            BootComponent::SecureBootPolicy => "Secure Boot Policy",
            BootComponent::Custom(_) => "Custom Component",
        }
    }
}

/// Event type in TCG event log
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum EventType {
    /// Pre-boot environment event
    PrebootCert = 0x00000000,
    /// POST code event
    PostCode = 0x00000001,
    /// No action (informational)
    NoAction = 0x00000003,
    /// Separator event
    Separator = 0x00000004,
    /// Action (describes what is measured)
    Action = 0x00000005,
    /// Platform configuration event
    EventTag = 0x00000006,
    /// S-CRTM contents
    SCrtmContents = 0x00000007,
    /// S-CRTM version
    SCrtmVersion = 0x00000008,
    /// CPU microcode
    CpuMicrocode = 0x00000009,
    /// Platform configuration data
    PlatformConfigFlags = 0x0000000A,
    /// Table of devices
    TableOfDevices = 0x0000000B,
    /// Compact hash (SHA-256)
    CompactHash = 0x0000000C,
    /// IPL (Initial Program Load)
    Ipl = 0x0000000D,
    /// IPL partition data
    IplPartitionData = 0x0000000E,
    /// Non-host code
    NonhostCode = 0x0000000F,
    /// Non-host configuration
    NonhostConfig = 0x00000010,
    /// Non-host info
    NonhostInfo = 0x00000011,
    /// Omit boot device events
    OmitBootDeviceEvents = 0x00000012,
    /// EFI event base
    EfiEventBase = 0x80000000,
    /// EFI variable driver config
    EfiVariableDriverConfig = 0x80000001,
    /// EFI variable boot
    EfiVariableBoot = 0x80000002,
    /// EFI boot services application
    EfiBootServicesApplication = 0x80000003,
    /// EFI boot services driver
    EfiBootServicesDriver = 0x80000004,
    /// EFI runtime services driver
    EfiRuntimeServicesDriver = 0x80000005,
    /// EFI GPT event
    EfiGptEvent = 0x80000006,
    /// EFI action
    EfiAction = 0x80000007,
    /// EFI platform firmware blob
    EfiPlatformFirmwareBlob = 0x80000008,
    /// EFI handoff tables
    EfiHandoffTables = 0x80000009,
    /// EFI platform firmware blob2
    EfiPlatformFirmwareBlob2 = 0x8000000A,
    /// EFI handoff tables2
    EfiHandoffTables2 = 0x8000000B,
    /// EFI variable boot2
    EfiVariableBoot2 = 0x8000000C,
    /// EFI hcrtm event
    EfiHcrtmEvent = 0x80000010,
    /// EFI variable authority
    EfiVariableAuthority = 0x800000E0,
    /// EFI SPDM firmware blob
    EfiSpdmFirmwareBlob = 0x800000E1,
    /// EFI SPDM firmware config
    EfiSpdmFirmwareConfig = 0x800000E2,
}

/// Measurement entry in the event log
#[derive(Debug, Clone)]
pub struct MeasurementEntry {
    /// PCR index
    pub pcr_index: u32,
    /// Event type
    pub event_type: EventType,
    /// SHA-256 digest
    pub digest: [u8; 32],
    /// Event description
    pub description: String,
    /// Boot component type
    pub component: BootComponent,
    /// Data size (for reference)
    pub data_size: usize,
    /// Timestamp (uptime in milliseconds)
    pub timestamp_ms: u64,
    /// Measurement sequence number
    pub sequence: u64,
}

/// Measurement result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasurementResult {
    /// Measurement successful
    Success,
    /// TPM not available
    TpmNotAvailable,
    /// PCR extend failed
    ExtendFailed,
    /// Digest computation failed
    DigestFailed,
    /// Component not found
    ComponentNotFound,
    /// Already measured
    AlreadyMeasured,
}

/// Boot measurement policy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasurementPolicy {
    /// Measure all boot components
    MeasureAll,
    /// Measure only critical components (kernel, cmdline, initrd)
    MeasureCritical,
    /// No measurement (disabled)
    Disabled,
}

/// Expected PCR value for verification
#[derive(Debug, Clone)]
pub struct ExpectedPcrValue {
    /// PCR index
    pub pcr_index: u32,
    /// Expected digest
    pub expected_digest: [u8; 32],
    /// Description
    pub description: String,
    /// Required for boot to continue
    pub required: bool,
}

/// Boot measurement configuration
#[derive(Debug, Clone)]
pub struct MeasuredBootConfig {
    /// Measurement policy
    pub policy: MeasurementPolicy,
    /// Hash algorithm (default SHA-256)
    pub hash_algorithm: u16,
    /// Enable event logging
    pub enable_logging: bool,
    /// Maximum event log entries
    pub max_log_entries: usize,
    /// Enable sealed secrets
    pub enable_sealed_secrets: bool,
    /// Expected PCR values for verification
    pub expected_values: Vec<ExpectedPcrValue>,
    /// Enforce expected values (fail if mismatch)
    pub enforce_expected: bool,
    /// Log measurements to serial console
    pub log_to_console: bool,
}

impl Default for MeasuredBootConfig {
    fn default() -> Self {
        Self {
            policy: MeasurementPolicy::MeasureAll,
            hash_algorithm: algorithms::TPM2_ALG_SHA256,
            enable_logging: true,
            max_log_entries: 1024,
            enable_sealed_secrets: true,
            expected_values: Vec::new(),
            enforce_expected: false,
            log_to_console: true,
        }
    }
}

/// Measured boot statistics
#[derive(Debug, Clone, Default)]
pub struct MeasuredBootStats {
    /// Total measurements performed
    pub total_measurements: u64,
    /// Successful measurements
    pub successful_measurements: u64,
    /// Failed measurements
    pub failed_measurements: u64,
    /// PCR extends performed
    pub pcr_extends: u64,
    /// Verification failures
    pub verification_failures: u64,
    /// Last measurement timestamp
    pub last_measurement_ms: u64,
}

/// Sealed secret
#[derive(Debug, Clone)]
pub struct SealedSecret {
    /// Secret identifier
    pub id: String,
    /// Sealed data (encrypted by TPM)
    pub sealed_data: Vec<u8>,
    /// PCR policy (PCR values that must match to unseal)
    pub pcr_policy: BTreeMap<u32, [u8; 32]>,
    /// Creation timestamp
    pub created_ms: u64,
    /// Description
    pub description: String,
}

/// Measured Boot Manager
pub struct MeasuredBootManager {
    /// Configuration
    config: MeasuredBootConfig,
    /// Event log
    event_log: Vec<MeasurementEntry>,
    /// PCR shadow values (in-memory copy)
    pcr_shadow: BTreeMap<u32, [u8; 32]>,
    /// Measured components (to avoid duplicate measurements)
    measured_components: Vec<(BootComponent, String)>,
    /// Statistics
    stats: MeasuredBootStats,
    /// Sealed secrets
    sealed_secrets: BTreeMap<String, SealedSecret>,
    /// Measurement sequence counter
    sequence_counter: AtomicU64,
    /// Initialized flag
    initialized: bool,
    /// Boot verified (all expected PCRs match)
    boot_verified: bool,
}

impl MeasuredBootManager {
    /// Create a new measured boot manager
    pub const fn new() -> Self {
        Self {
            config: MeasuredBootConfig {
                policy: MeasurementPolicy::MeasureAll,
                hash_algorithm: algorithms::TPM2_ALG_SHA256,
                enable_logging: true,
                max_log_entries: 1024,
                enable_sealed_secrets: true,
                expected_values: Vec::new(),
                enforce_expected: false,
                log_to_console: true,
            },
            event_log: Vec::new(),
            pcr_shadow: BTreeMap::new(),
            measured_components: Vec::new(),
            stats: MeasuredBootStats {
                total_measurements: 0,
                successful_measurements: 0,
                failed_measurements: 0,
                pcr_extends: 0,
                verification_failures: 0,
                last_measurement_ms: 0,
            },
            sealed_secrets: BTreeMap::new(),
            sequence_counter: AtomicU64::new(0),
            initialized: false,
            boot_verified: false,
        }
    }

    /// Initialize measured boot
    pub fn init(&mut self) -> KResult<()> {
        if self.initialized {
            return Ok(());
        }

        // Check if TPM is available
        if !tpm::is_available() {
            crate::kprintln!("measured_boot: TPM not available, measurements disabled");
            self.config.policy = MeasurementPolicy::Disabled;
            self.initialized = true;
            return Ok(());
        }

        // Initialize PCR shadow with initial values
        for pcr in 0..24 {
            // Initial PCR value is all zeros (before any extends)
            self.pcr_shadow.insert(pcr, [0u8; 32]);
        }

        // Read actual PCR values from TPM to sync shadow
        self.sync_pcr_shadow()?;

        self.initialized = true;

        crate::kprintln!("measured_boot: initialized with {:?} policy", self.config.policy);

        Ok(())
    }

    /// Sync PCR shadow with actual TPM values
    fn sync_pcr_shadow(&mut self) -> KResult<()> {
        for pcr in 0..24 {
            match tpm::pcr_read(pcr, self.config.hash_algorithm) {
                Ok(digest) => {
                    if digest.len() == 32 {
                        let mut arr = [0u8; 32];
                        arr.copy_from_slice(&digest);
                        self.pcr_shadow.insert(pcr, arr);
                    }
                }
                Err(_) => {
                    // PCR read may fail for some PCRs, that's OK
                }
            }
        }
        Ok(())
    }

    /// Measure a boot component
    pub fn measure(&mut self, component: BootComponent, data: &[u8], description: &str) -> MeasurementResult {
        if self.config.policy == MeasurementPolicy::Disabled {
            return MeasurementResult::TpmNotAvailable;
        }

        // Check if already measured
        let key = (component, description.to_string());
        if self.measured_components.contains(&key) {
            return MeasurementResult::AlreadyMeasured;
        }

        self.stats.total_measurements += 1;

        // Compute SHA-256 digest
        let digest = match self.compute_sha256(data) {
            Some(d) => d,
            None => {
                self.stats.failed_measurements += 1;
                return MeasurementResult::DigestFailed;
            }
        };

        let pcr_index = component.pcr_index();

        // Extend PCR in TPM
        match tpm::pcr_extend(pcr_index, self.config.hash_algorithm, &digest) {
            Ok(()) => {
                self.stats.pcr_extends += 1;
            }
            Err(_) => {
                self.stats.failed_measurements += 1;
                return MeasurementResult::ExtendFailed;
            }
        }

        // Update shadow PCR (PCR_new = SHA256(PCR_old || measurement))
        self.extend_shadow_pcr(pcr_index, &digest);

        // Log the measurement
        if self.config.enable_logging {
            let seq = self.sequence_counter.fetch_add(1, Ordering::SeqCst);
            let timestamp = crate::time::uptime_ms();

            let entry = MeasurementEntry {
                pcr_index,
                event_type: self.component_to_event_type(component),
                digest,
                description: description.to_string(),
                component,
                data_size: data.len(),
                timestamp_ms: timestamp,
                sequence: seq,
            };

            if self.event_log.len() < self.config.max_log_entries {
                self.event_log.push(entry);
            }

            self.stats.last_measurement_ms = timestamp;
        }

        // Mark as measured
        self.measured_components.push(key);

        if self.config.log_to_console {
            crate::kprintln!("measured_boot: PCR{} extended with {} ({} bytes)",
                pcr_index, description, data.len());
        }

        self.stats.successful_measurements += 1;
        MeasurementResult::Success
    }

    /// Measure kernel image
    pub fn measure_kernel(&mut self, kernel_data: &[u8]) -> MeasurementResult {
        self.measure(BootComponent::Kernel, kernel_data, "Stenzel OS Kernel")
    }

    /// Measure kernel command line
    pub fn measure_cmdline(&mut self, cmdline: &str) -> MeasurementResult {
        self.measure(BootComponent::KernelCmdline, cmdline.as_bytes(), cmdline)
    }

    /// Measure initrd
    pub fn measure_initrd(&mut self, initrd_data: &[u8]) -> MeasurementResult {
        self.measure(BootComponent::Initrd, initrd_data, "Initial Ramdisk")
    }

    /// Measure a kernel module
    pub fn measure_module(&mut self, module_name: &str, module_data: &[u8]) -> MeasurementResult {
        self.measure(BootComponent::KernelModule, module_data, module_name)
    }

    /// Measure Secure Boot policy
    pub fn measure_secureboot_policy(&mut self, policy_data: &[u8]) -> MeasurementResult {
        self.measure(BootComponent::SecureBootPolicy, policy_data, "Secure Boot Policy")
    }

    /// Compute SHA-256 digest
    fn compute_sha256(&self, data: &[u8]) -> Option<[u8; 32]> {
        // Use software SHA-256 implementation
        Some(sha256_hash(data))
    }

    /// Extend shadow PCR: PCR_new = SHA256(PCR_old || measurement)
    fn extend_shadow_pcr(&mut self, pcr_index: u32, measurement: &[u8; 32]) {
        if let Some(current) = self.pcr_shadow.get(&pcr_index) {
            // Concatenate current PCR value with new measurement
            let mut combined = [0u8; 64];
            combined[0..32].copy_from_slice(current);
            combined[32..64].copy_from_slice(measurement);

            // Hash the combined value
            let new_value = sha256_hash(&combined);
            self.pcr_shadow.insert(pcr_index, new_value);
        }
    }

    /// Get event type for a component
    fn component_to_event_type(&self, component: BootComponent) -> EventType {
        match component {
            BootComponent::Firmware => EventType::EfiPlatformFirmwareBlob,
            BootComponent::PlatformConfig => EventType::PlatformConfigFlags,
            BootComponent::OptionRom => EventType::EfiBootServicesDriver,
            BootComponent::Bootloader => EventType::EfiBootServicesApplication,
            BootComponent::BootloaderConfig => EventType::EfiVariableBoot,
            BootComponent::Kernel => EventType::EfiBootServicesApplication,
            BootComponent::KernelCmdline => EventType::EfiAction,
            BootComponent::Initrd => EventType::EfiBootServicesApplication,
            BootComponent::KernelModule => EventType::Ipl,
            BootComponent::SecureBootPolicy => EventType::EfiVariableAuthority,
            BootComponent::Custom(_) => EventType::Action,
        }
    }

    /// Verify boot against expected PCR values
    pub fn verify_boot(&mut self) -> KResult<bool> {
        if self.config.expected_values.is_empty() {
            self.boot_verified = true;
            return Ok(true);
        }

        let mut all_match = true;

        for expected in &self.config.expected_values {
            if let Some(actual) = self.pcr_shadow.get(&expected.pcr_index) {
                if actual != &expected.expected_digest {
                    crate::kprintln!("measured_boot: PCR{} mismatch! Expected {:02x?}, got {:02x?}",
                        expected.pcr_index,
                        &expected.expected_digest[0..8],
                        &actual[0..8]
                    );

                    self.stats.verification_failures += 1;

                    if expected.required {
                        all_match = false;
                    }
                }
            } else {
                if expected.required {
                    all_match = false;
                }
            }
        }

        self.boot_verified = all_match;

        if self.config.enforce_expected && !all_match {
            return Err(KError::PermissionDenied);
        }

        Ok(all_match)
    }

    /// Get PCR value (shadow)
    pub fn get_pcr(&self, pcr_index: u32) -> Option<[u8; 32]> {
        self.pcr_shadow.get(&pcr_index).copied()
    }

    /// Get PCR value from TPM
    pub fn read_pcr_from_tpm(&self, pcr_index: u32) -> KResult<[u8; 32]> {
        let digest = tpm::pcr_read(pcr_index, self.config.hash_algorithm)
            .map_err(|_| KError::IO)?;

        if digest.len() != 32 {
            return Err(KError::Invalid);
        }

        let mut arr = [0u8; 32];
        arr.copy_from_slice(&digest);
        Ok(arr)
    }

    /// Get event log
    pub fn event_log(&self) -> &[MeasurementEntry] {
        &self.event_log
    }

    /// Get statistics
    pub fn stats(&self) -> &MeasuredBootStats {
        &self.stats
    }

    /// Check if boot is verified
    pub fn is_boot_verified(&self) -> bool {
        self.boot_verified
    }

    /// Set expected PCR value
    pub fn set_expected_pcr(&mut self, pcr_index: u32, digest: [u8; 32], description: &str, required: bool) {
        self.config.expected_values.push(ExpectedPcrValue {
            pcr_index,
            expected_digest: digest,
            description: description.to_string(),
            required,
        });
    }

    /// Seal a secret to current PCR state
    pub fn seal_secret(&mut self, id: &str, secret: &[u8], description: &str) -> KResult<()> {
        if !self.config.enable_sealed_secrets {
            return Err(KError::NotSupported);
        }

        if !tpm::is_available() {
            return Err(KError::NotSupported);
        }

        // Create PCR policy from current shadow values
        let mut pcr_policy = BTreeMap::new();
        for pcr in [pcr_index::PCR_KERNEL, pcr_index::PCR_KERNEL_CMDLINE] {
            if let Some(value) = self.pcr_shadow.get(&pcr) {
                pcr_policy.insert(pcr, *value);
            }
        }

        // For now, we just store the secret (real TPM sealing would use TPM2_Create)
        // This is a placeholder for the full TPM sealing implementation
        let sealed = SealedSecret {
            id: id.to_string(),
            sealed_data: secret.to_vec(), // Would be encrypted by TPM in real implementation
            pcr_policy,
            created_ms: crate::time::uptime_ms(),
            description: description.to_string(),
        };

        self.sealed_secrets.insert(id.to_string(), sealed);

        crate::kprintln!("measured_boot: sealed secret '{}' to PCR policy", id);

        Ok(())
    }

    /// Unseal a secret (only if PCR state matches)
    pub fn unseal_secret(&self, id: &str) -> KResult<Vec<u8>> {
        let sealed = self.sealed_secrets.get(id)
            .ok_or(KError::NotFound)?;

        // Verify PCR policy
        for (pcr_idx, expected) in &sealed.pcr_policy {
            if let Some(current) = self.pcr_shadow.get(pcr_idx) {
                if current != expected {
                    crate::kprintln!("measured_boot: unseal failed - PCR{} mismatch", pcr_idx);
                    return Err(KError::PermissionDenied);
                }
            } else {
                return Err(KError::PermissionDenied);
            }
        }

        // In real implementation, this would call TPM2_Unseal
        Ok(sealed.sealed_data.clone())
    }

    /// Export event log in binary TCG format
    pub fn export_event_log(&self) -> Vec<u8> {
        let mut log = Vec::new();

        for entry in &self.event_log {
            // TCG_PCR_EVENT2 format (simplified)
            // PCR index (4 bytes)
            log.extend_from_slice(&entry.pcr_index.to_le_bytes());
            // Event type (4 bytes)
            log.extend_from_slice(&(entry.event_type as u32).to_le_bytes());
            // Digest count (4 bytes) - always 1 for SHA-256
            log.extend_from_slice(&1u32.to_le_bytes());
            // Algorithm ID (2 bytes)
            log.extend_from_slice(&self.config.hash_algorithm.to_le_bytes());
            // Digest (32 bytes)
            log.extend_from_slice(&entry.digest);
            // Event size (4 bytes)
            let event_data = entry.description.as_bytes();
            log.extend_from_slice(&(event_data.len() as u32).to_le_bytes());
            // Event data
            log.extend_from_slice(event_data);
        }

        log
    }

    /// Get configuration
    pub fn config(&self) -> &MeasuredBootConfig {
        &self.config
    }

    /// Set configuration
    pub fn set_config(&mut self, config: MeasuredBootConfig) {
        self.config = config;
    }

    /// Format status string
    pub fn format_status(&self) -> String {
        let mut s = String::new();
        use core::fmt::Write;

        let _ = writeln!(s, "Measured Boot Status:");
        let _ = writeln!(s, "  Policy: {:?}", self.config.policy);
        let _ = writeln!(s, "  TPM Available: {}", tpm::is_available());
        let _ = writeln!(s, "  Boot Verified: {}", self.boot_verified);
        let _ = writeln!(s, "  Total Measurements: {}", self.stats.total_measurements);
        let _ = writeln!(s, "  Successful: {}", self.stats.successful_measurements);
        let _ = writeln!(s, "  Failed: {}", self.stats.failed_measurements);
        let _ = writeln!(s, "  Event Log Entries: {}", self.event_log.len());

        if !self.pcr_shadow.is_empty() {
            let _ = writeln!(s, "  PCR Values:");
            for pcr in [
                pcr_index::PCR_KERNEL,
                pcr_index::PCR_KERNEL_CMDLINE,
                pcr_index::PCR_SECUREBOOT_POLICY,
            ] {
                if let Some(value) = self.pcr_shadow.get(&pcr) {
                    let _ = writeln!(s, "    PCR{}: {:02x}{:02x}{:02x}{:02x}...",
                        pcr, value[0], value[1], value[2], value[3]);
                }
            }
        }

        s
    }
}

// =============================================================================
// SHA-256 Implementation (minimal for measured boot)
// =============================================================================

/// SHA-256 constants
const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5,
    0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
    0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc,
    0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
    0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
    0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
    0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5,
    0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
    0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// Initial hash values for SHA-256
const SHA256_H: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
    0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// Compute SHA-256 hash
fn sha256_hash(data: &[u8]) -> [u8; 32] {
    let mut h = SHA256_H;

    // Pre-processing: padding
    let ml = (data.len() as u64) * 8;
    let mut padded = data.to_vec();
    padded.push(0x80);

    while (padded.len() % 64) != 56 {
        padded.push(0x00);
    }

    padded.extend_from_slice(&ml.to_be_bytes());

    // Process each 512-bit chunk
    for chunk in padded.chunks(64) {
        let mut w = [0u32; 64];

        // Copy chunk into first 16 words
        for (i, word) in chunk.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }

        // Extend the first 16 words into the remaining 48 words
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }

        // Initialize working variables
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);

        // Compression function main loop
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(SHA256_K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        // Add the compressed chunk to the current hash value
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    // Produce the final hash value (big-endian)
    let mut result = [0u8; 32];
    for (i, &word) in h.iter().enumerate() {
        result[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }

    result
}

// =============================================================================
// Global Instance
// =============================================================================

pub static MEASURED_BOOT: IrqSafeMutex<MeasuredBootManager> = IrqSafeMutex::new(MeasuredBootManager::new());

/// Initialize measured boot subsystem
pub fn init() {
    if let Err(e) = MEASURED_BOOT.lock().init() {
        crate::kprintln!("measured_boot: initialization failed: {:?}", e);
    }
}

/// Measure kernel
pub fn measure_kernel(data: &[u8]) -> MeasurementResult {
    MEASURED_BOOT.lock().measure_kernel(data)
}

/// Measure kernel command line
pub fn measure_cmdline(cmdline: &str) -> MeasurementResult {
    MEASURED_BOOT.lock().measure_cmdline(cmdline)
}

/// Measure initrd
pub fn measure_initrd(data: &[u8]) -> MeasurementResult {
    MEASURED_BOOT.lock().measure_initrd(data)
}

/// Measure kernel module
pub fn measure_module(name: &str, data: &[u8]) -> MeasurementResult {
    MEASURED_BOOT.lock().measure_module(name, data)
}

/// Verify boot integrity
pub fn verify_boot() -> KResult<bool> {
    MEASURED_BOOT.lock().verify_boot()
}

/// Check if boot is verified
pub fn is_verified() -> bool {
    MEASURED_BOOT.lock().is_boot_verified()
}

/// Get PCR value
pub fn get_pcr(index: u32) -> Option<[u8; 32]> {
    MEASURED_BOOT.lock().get_pcr(index)
}

/// Seal a secret
pub fn seal_secret(id: &str, secret: &[u8], description: &str) -> KResult<()> {
    MEASURED_BOOT.lock().seal_secret(id, secret, description)
}

/// Unseal a secret
pub fn unseal_secret(id: &str) -> KResult<Vec<u8>> {
    MEASURED_BOOT.lock().unseal_secret(id)
}

/// Get statistics
pub fn stats() -> MeasuredBootStats {
    MEASURED_BOOT.lock().stats().clone()
}

/// Format status
pub fn status() -> String {
    MEASURED_BOOT.lock().format_status()
}
