//! TPM Disk Unlock
//!
//! Automatic disk unlock using TPM sealed keys.
//! Similar to systemd-cryptenroll and clevis.

#![allow(dead_code)]

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::sync::IrqSafeMutex;
use crate::util::kprintln;

/// TPM disk unlock state
static TPM_DISK_UNLOCK: IrqSafeMutex<Option<TpmDiskUnlock>> = IrqSafeMutex::new(None);

/// Statistics
static STATS: TpmDiskUnlockStats = TpmDiskUnlockStats::new();

/// Unlock method
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnlockMethod {
    /// TPM2 alone (PCR state)
    Tpm2,
    /// TPM2 + PIN
    Tpm2Pin,
    /// TPM2 + FIDO2 key
    Tpm2Fido2,
    /// TPM2 + Recovery key
    Tpm2Recovery,
    /// Password only (no TPM)
    Password,
    /// Recovery key only
    RecoveryKey,
}

impl UnlockMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Tpm2 => "tpm2",
            Self::Tpm2Pin => "tpm2+pin",
            Self::Tpm2Fido2 => "tpm2+fido2",
            Self::Tpm2Recovery => "tpm2+recovery",
            Self::Password => "password",
            Self::RecoveryKey => "recovery",
        }
    }

    pub fn requires_tpm(&self) -> bool {
        matches!(self, Self::Tpm2 | Self::Tpm2Pin | Self::Tpm2Fido2 | Self::Tpm2Recovery)
    }

    pub fn requires_user_input(&self) -> bool {
        matches!(self, Self::Tpm2Pin | Self::Password | Self::RecoveryKey)
    }
}

/// PCR policy for unlock
#[derive(Debug, Clone)]
pub struct PcrPolicy {
    /// PCR bank (SHA256, SHA1)
    pub hash_algorithm: HashAlgorithm,
    /// PCRs to bind to
    pub pcr_mask: u32,
    /// Expected PCR values (optional, for policy signing)
    pub expected_values: Vec<(u8, [u8; 32])>,
}

impl Default for PcrPolicy {
    fn default() -> Self {
        Self {
            hash_algorithm: HashAlgorithm::Sha256,
            // Default: PCR 0 (firmware), 7 (secure boot state)
            pcr_mask: (1 << 0) | (1 << 7),
            expected_values: Vec::new(),
        }
    }
}

/// Hash algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlgorithm {
    Sha1,
    Sha256,
    Sha384,
    Sha512,
}

impl HashAlgorithm {
    pub fn tpm2_alg_id(&self) -> u16 {
        match self {
            Self::Sha1 => 0x0004,
            Self::Sha256 => 0x000B,
            Self::Sha384 => 0x000C,
            Self::Sha512 => 0x000D,
        }
    }

    pub fn digest_size(&self) -> usize {
        match self {
            Self::Sha1 => 20,
            Self::Sha256 => 32,
            Self::Sha384 => 48,
            Self::Sha512 => 64,
        }
    }
}

/// Enrolled key slot
#[derive(Debug, Clone)]
pub struct EnrolledSlot {
    /// LUKS key slot index
    pub luks_slot: u8,
    /// Unlock method
    pub method: UnlockMethod,
    /// PCR policy (if TPM)
    pub pcr_policy: Option<PcrPolicy>,
    /// NV index for sealed key (if TPM)
    pub nv_index: Option<u32>,
    /// Description
    pub description: String,
    /// Creation timestamp
    pub enrolled_at: u64,
    /// Last successful unlock
    pub last_unlock: Option<u64>,
    /// Unlock count
    pub unlock_count: u64,
}

/// Disk configuration
#[derive(Debug, Clone)]
pub struct DiskConfig {
    /// Device path (e.g., /dev/sda1)
    pub device: String,
    /// LUKS UUID
    pub uuid: String,
    /// Enrolled slots
    pub slots: Vec<EnrolledSlot>,
    /// Auto-unlock enabled
    pub auto_unlock: bool,
    /// Timeout for TPM unlock (ms)
    pub timeout_ms: u32,
    /// Fallback to password on TPM failure
    pub fallback_password: bool,
}

/// Unlock result
#[derive(Debug)]
pub enum UnlockResult {
    Success {
        method: UnlockMethod,
        slot: u8,
        duration_ms: u64,
    },
    FailedTpm {
        error: TpmUnlockError,
        can_fallback: bool,
    },
    FailedPassword,
    FailedRecovery,
    Timeout,
    NotEnrolled,
    DeviceNotFound,
}

/// TPM unlock error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TpmUnlockError {
    TpmNotPresent,
    TpmNotInitialized,
    PcrMismatch,
    SealedKeyNotFound,
    UnsealFailed,
    PolicyNotSatisfied,
    AuthFailed,
    NvReadFailed,
    LuksOpenFailed,
    Timeout,
    Unknown,
}

impl TpmUnlockError {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TpmNotPresent => "TPM not present",
            Self::TpmNotInitialized => "TPM not initialized",
            Self::PcrMismatch => "PCR values changed",
            Self::SealedKeyNotFound => "Sealed key not found",
            Self::UnsealFailed => "Failed to unseal key",
            Self::PolicyNotSatisfied => "Policy not satisfied",
            Self::AuthFailed => "Authentication failed",
            Self::NvReadFailed => "NV read failed",
            Self::LuksOpenFailed => "LUKS open failed",
            Self::Timeout => "Operation timed out",
            Self::Unknown => "Unknown error",
        }
    }

    pub fn can_retry(&self) -> bool {
        matches!(self, Self::Timeout | Self::AuthFailed)
    }
}

/// Enroll options
#[derive(Debug, Clone)]
pub struct EnrollOptions {
    /// Method to enroll
    pub method: UnlockMethod,
    /// PCR policy
    pub pcr_policy: PcrPolicy,
    /// PIN (for Tpm2Pin method)
    pub pin: Option<String>,
    /// LUKS passphrase to enroll with
    pub passphrase: String,
    /// Description
    pub description: String,
    /// Preferred LUKS slot (None = auto)
    pub preferred_slot: Option<u8>,
    /// NV index to use (None = auto)
    pub nv_index: Option<u32>,
}

/// TPM Disk Unlock manager
pub struct TpmDiskUnlock {
    /// Configured disks
    disks: Vec<DiskConfig>,
    /// TPM available
    tpm_available: bool,
    /// TPM version
    tpm_version: Option<TpmVersion>,
    /// Default PCR policy
    default_pcr_policy: PcrPolicy,
    /// Auto-enrollment enabled
    auto_enroll: bool,
    /// Next available NV index
    next_nv_index: u32,
}

/// TPM version info
#[derive(Debug, Clone, Copy)]
pub struct TpmVersion {
    pub major: u8,
    pub minor: u8,
    pub revision: u8,
}

/// Statistics
pub struct TpmDiskUnlockStats {
    unlock_attempts: AtomicU64,
    successful_unlocks: AtomicU64,
    failed_unlocks: AtomicU64,
    pcr_mismatches: AtomicU64,
    password_fallbacks: AtomicU64,
    enrollments: AtomicU64,
    unenrollments: AtomicU64,
    tpm_errors: AtomicU64,
}

impl TpmDiskUnlockStats {
    const fn new() -> Self {
        Self {
            unlock_attempts: AtomicU64::new(0),
            successful_unlocks: AtomicU64::new(0),
            failed_unlocks: AtomicU64::new(0),
            pcr_mismatches: AtomicU64::new(0),
            password_fallbacks: AtomicU64::new(0),
            enrollments: AtomicU64::new(0),
            unenrollments: AtomicU64::new(0),
            tpm_errors: AtomicU64::new(0),
        }
    }
}

impl TpmDiskUnlock {
    /// Create new TPM disk unlock manager
    pub fn new() -> Self {
        Self {
            disks: Vec::new(),
            tpm_available: false,
            tpm_version: None,
            default_pcr_policy: PcrPolicy::default(),
            auto_enroll: false,
            next_nv_index: 0x01800000, // TPM2 owner NV space
        }
    }

    /// Initialize with TPM
    pub fn init(&mut self) -> Result<(), TpmUnlockError> {
        kprintln!("tpm-disk-unlock: Initializing...");

        // Check TPM availability
        self.tpm_available = self.detect_tpm();

        if self.tpm_available {
            kprintln!("tpm-disk-unlock: TPM detected");
            self.tpm_version = Some(TpmVersion {
                major: 2,
                minor: 0,
                revision: 0,
            });
        } else {
            kprintln!("tpm-disk-unlock: No TPM detected");
        }

        // Load saved configurations
        self.load_config();

        Ok(())
    }

    /// Detect TPM presence
    fn detect_tpm(&self) -> bool {
        // Check TPM2 at MMIO address
        let tpm_base = 0xFED4_0000u64;

        // In real implementation, read TPM registers
        // For now, assume present if ACPI table found

        // Placeholder - would check TPM_ACCESS register
        true
    }

    /// Load disk configurations
    fn load_config(&mut self) {
        // Would load from /etc/crypttab or similar
        kprintln!("tpm-disk-unlock: Loading configuration...");
    }

    /// Add disk for auto-unlock
    pub fn add_disk(&mut self, config: DiskConfig) {
        kprintln!("tpm-disk-unlock: Adding disk {}", config.device);
        self.disks.push(config);
    }

    /// Remove disk
    pub fn remove_disk(&mut self, device: &str) -> bool {
        let before = self.disks.len();
        self.disks.retain(|d| d.device != device);
        before != self.disks.len()
    }

    /// Enroll a key for disk unlock
    pub fn enroll(
        &mut self,
        device: &str,
        options: EnrollOptions,
    ) -> Result<EnrolledSlot, TpmUnlockError> {
        kprintln!("tpm-disk-unlock: Enrolling {} for {}", options.method.as_str(), device);

        // Find or create disk config
        let disk = self.disks.iter_mut().find(|d| d.device == device);

        if options.method.requires_tpm() && !self.tpm_available {
            return Err(TpmUnlockError::TpmNotPresent);
        }

        // Allocate NV index for TPM methods
        let nv_index = if options.method.requires_tpm() {
            let idx = options.nv_index.unwrap_or(self.next_nv_index);
            self.next_nv_index = idx + 1;
            Some(idx)
        } else {
            None
        };

        // Find available LUKS slot
        let luks_slot = options.preferred_slot.unwrap_or_else(|| {
            // Find first unused slot
            let used_slots: Vec<u8> = disk.map(|d| d.slots.iter().map(|s| s.luks_slot).collect())
                .unwrap_or_default();
            (0..8).find(|s| !used_slots.contains(s)).unwrap_or(7)
        });

        // Create enrolled slot
        let slot = EnrolledSlot {
            luks_slot,
            method: options.method,
            pcr_policy: if options.method.requires_tpm() {
                Some(options.pcr_policy)
            } else {
                None
            },
            nv_index,
            description: options.description,
            enrolled_at: crate::time::uptime_ms(),
            last_unlock: None,
            unlock_count: 0,
        };

        // Seal key to TPM if needed
        if let Some(nv_idx) = nv_index {
            self.seal_key_to_tpm(
                &options.passphrase,
                nv_idx,
                slot.pcr_policy.as_ref().unwrap(),
                options.pin.as_deref(),
            )?;
        }

        // Add LUKS key slot
        self.add_luks_key_slot(device, &options.passphrase, luks_slot)?;

        // Update disk config
        if let Some(disk) = self.disks.iter_mut().find(|d| d.device == device) {
            disk.slots.push(slot.clone());
        } else {
            self.disks.push(DiskConfig {
                device: device.to_string(),
                uuid: self.get_luks_uuid(device).unwrap_or_default(),
                slots: vec![slot.clone()],
                auto_unlock: true,
                timeout_ms: 30000,
                fallback_password: true,
            });
        }

        STATS.enrollments.fetch_add(1, Ordering::Relaxed);
        kprintln!("tpm-disk-unlock: Enrolled slot {} for {}", luks_slot, device);

        Ok(slot)
    }

    /// Seal key to TPM
    fn seal_key_to_tpm(
        &self,
        passphrase: &str,
        nv_index: u32,
        policy: &PcrPolicy,
        pin: Option<&str>,
    ) -> Result<(), TpmUnlockError> {
        kprintln!("tpm-disk-unlock: Sealing key to TPM NV 0x{:08x}", nv_index);

        // In real implementation:
        // 1. Read current PCR values
        // 2. Create policy session with PCRs
        // 3. Add PIN authorization if specified
        // 4. Seal passphrase to TPM
        // 5. Write sealed blob to NV index

        // For now, placeholder
        let _sealed_data = self.create_sealed_blob(passphrase.as_bytes(), policy, pin)?;

        Ok(())
    }

    /// Create sealed blob
    fn create_sealed_blob(
        &self,
        data: &[u8],
        policy: &PcrPolicy,
        pin: Option<&str>,
    ) -> Result<Vec<u8>, TpmUnlockError> {
        // Placeholder sealed blob format:
        // [4 bytes: magic]
        // [4 bytes: version]
        // [4 bytes: pcr_mask]
        // [2 bytes: hash_alg]
        // [2 bytes: pin_length]
        // [N bytes: pin_hash if present]
        // [4 bytes: data_length]
        // [N bytes: encrypted_data]
        // [32 bytes: auth_tag]

        let mut blob = Vec::with_capacity(256);

        // Magic
        blob.extend_from_slice(b"TSBL");
        // Version
        blob.extend_from_slice(&1u32.to_le_bytes());
        // PCR mask
        blob.extend_from_slice(&policy.pcr_mask.to_le_bytes());
        // Hash algorithm
        blob.extend_from_slice(&policy.hash_algorithm.tpm2_alg_id().to_le_bytes());

        // PIN
        if let Some(pin) = pin {
            let pin_hash = self.hash_pin(pin);
            blob.extend_from_slice(&(pin_hash.len() as u16).to_le_bytes());
            blob.extend_from_slice(&pin_hash);
        } else {
            blob.extend_from_slice(&0u16.to_le_bytes());
        }

        // Data (would be encrypted in real implementation)
        blob.extend_from_slice(&(data.len() as u32).to_le_bytes());
        blob.extend_from_slice(data);

        // Auth tag (placeholder)
        blob.extend_from_slice(&[0u8; 32]);

        Ok(blob)
    }

    /// Hash PIN for storage
    fn hash_pin(&self, pin: &str) -> [u8; 32] {
        // Simple hash for demonstration
        let mut hash = [0u8; 32];
        for (i, byte) in pin.bytes().enumerate() {
            hash[i % 32] ^= byte;
        }
        hash
    }

    /// Add LUKS key slot
    fn add_luks_key_slot(
        &self,
        _device: &str,
        _passphrase: &str,
        _slot: u8,
    ) -> Result<(), TpmUnlockError> {
        // Would call cryptsetup luksAddKey
        Ok(())
    }

    /// Get LUKS UUID
    fn get_luks_uuid(&self, _device: &str) -> Option<String> {
        // Would read LUKS header
        Some("00000000-0000-0000-0000-000000000000".to_string())
    }

    /// Unenroll a key slot
    pub fn unenroll(&mut self, device: &str, slot: u8) -> Result<(), TpmUnlockError> {
        kprintln!("tpm-disk-unlock: Unenrolling slot {} for {}", slot, device);

        // First, find the NV index to clear (before mutable borrow)
        let nv_index_to_clear = {
            let disk = self.disks.iter().find(|d| d.device == device)
                .ok_or(TpmUnlockError::SealedKeyNotFound)?;
            let enrolled = disk.slots.iter().find(|s| s.luks_slot == slot)
                .ok_or(TpmUnlockError::SealedKeyNotFound)?;
            enrolled.nv_index
        };

        // Remove from TPM NV if present
        if let Some(nv_index) = nv_index_to_clear {
            self.clear_nv_index(nv_index)?;
        }

        // Remove LUKS key slot
        self.remove_luks_key_slot(device, slot)?;

        // Remove from config
        if let Some(disk) = self.disks.iter_mut().find(|d| d.device == device) {
            disk.slots.retain(|s| s.luks_slot != slot);
        }

        STATS.unenrollments.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Clear NV index
    fn clear_nv_index(&self, _nv_index: u32) -> Result<(), TpmUnlockError> {
        // Would use TPM2_NV_UndefineSpace
        Ok(())
    }

    /// Remove LUKS key slot
    fn remove_luks_key_slot(&self, _device: &str, _slot: u8) -> Result<(), TpmUnlockError> {
        // Would call cryptsetup luksKillSlot
        Ok(())
    }

    /// Attempt to unlock disk
    pub fn unlock(&mut self, device: &str, pin: Option<&str>) -> UnlockResult {
        kprintln!("tpm-disk-unlock: Attempting to unlock {}", device);
        STATS.unlock_attempts.fetch_add(1, Ordering::Relaxed);

        // First, gather info we need without mutable borrow
        let (slots_to_try, fallback_password) = {
            let disk = match self.disks.iter().find(|d| d.device == device) {
                Some(d) => d,
                None => return UnlockResult::NotEnrolled,
            };

            let slots: Vec<(usize, UnlockMethod, u8, Option<u32>, Option<PcrPolicy>)> =
                disk.slots.iter().enumerate()
                    .filter(|(_, s)| s.method.requires_tpm())
                    .map(|(i, s)| (i, s.method, s.luks_slot, s.nv_index, s.pcr_policy.clone()))
                    .collect();
            (slots, disk.fallback_password)
        };

        let start = crate::time::uptime_ms();

        // Try TPM slots
        for (slot_idx, method, luks_slot, nv_index, pcr_policy) in slots_to_try {
            let result = self.try_tpm_unlock_inner(device, method, nv_index, pcr_policy.as_ref(), pin);
            match result {
                Ok(()) => {
                    let duration = crate::time::uptime_ms() - start;
                    // Update slot stats
                    if let Some(disk) = self.disks.iter_mut().find(|d| d.device == device) {
                        if let Some(slot) = disk.slots.get_mut(slot_idx) {
                            slot.last_unlock = Some(crate::time::uptime_ms());
                            slot.unlock_count += 1;
                        }
                    }
                    STATS.successful_unlocks.fetch_add(1, Ordering::Relaxed);

                    return UnlockResult::Success {
                        method,
                        slot: luks_slot,
                        duration_ms: duration,
                    };
                }
                Err(e) => {
                    kprintln!("tpm-disk-unlock: TPM unlock failed: {}", e.as_str());
                    if e == TpmUnlockError::PcrMismatch {
                        STATS.pcr_mismatches.fetch_add(1, Ordering::Relaxed);
                    }
                    if !fallback_password {
                        STATS.failed_unlocks.fetch_add(1, Ordering::Relaxed);
                        return UnlockResult::FailedTpm {
                            error: e,
                            can_fallback: false,
                        };
                    }
                    // Continue to next slot or fallback
                }
            }
        }

        // Fallback to password if enabled
        if fallback_password {
            STATS.password_fallbacks.fetch_add(1, Ordering::Relaxed);
            return UnlockResult::FailedTpm {
                error: TpmUnlockError::PcrMismatch,
                can_fallback: true,
            };
        }

        STATS.failed_unlocks.fetch_add(1, Ordering::Relaxed);
        UnlockResult::NotEnrolled
    }

    /// Inner TPM unlock without slot reference
    fn try_tpm_unlock_inner(
        &self,
        device: &str,
        method: UnlockMethod,
        nv_index: Option<u32>,
        pcr_policy: Option<&PcrPolicy>,
        pin: Option<&str>,
    ) -> Result<(), TpmUnlockError> {
        if !self.tpm_available {
            return Err(TpmUnlockError::TpmNotPresent);
        }

        let nv_index = nv_index.ok_or(TpmUnlockError::SealedKeyNotFound)?;
        let policy = pcr_policy.ok_or(TpmUnlockError::PolicyNotSatisfied)?;

        // 1. Check if PIN is required
        if method == UnlockMethod::Tpm2Pin && pin.is_none() {
            return Err(TpmUnlockError::AuthFailed);
        }

        // 2. Verify current PCR values match policy
        if !self.verify_pcr_policy(policy)? {
            return Err(TpmUnlockError::PcrMismatch);
        }

        // 3. Read sealed blob from NV
        let sealed = self.read_nv_index(nv_index)?;

        // 4. Unseal with policy session
        let passphrase = self.unseal_blob(&sealed, policy, pin)?;

        // 5. Open LUKS with passphrase
        self.open_luks(device, &passphrase, 0)?;

        Ok(())
    }

    /// Verify PCR policy
    fn verify_pcr_policy(&self, policy: &PcrPolicy) -> Result<bool, TpmUnlockError> {
        // Read current PCR values
        let current = self.read_pcrs(policy.pcr_mask, policy.hash_algorithm)?;

        // If expected values are set, compare
        if !policy.expected_values.is_empty() {
            for (pcr, expected) in &policy.expected_values {
                if let Some(actual) = current.iter().find(|(p, _)| p == pcr) {
                    if &actual.1 != expected {
                        kprintln!("tpm-disk-unlock: PCR {} mismatch", pcr);
                        return Ok(false);
                    }
                }
            }
        }

        Ok(true)
    }

    /// Read PCR values
    fn read_pcrs(
        &self,
        mask: u32,
        _algorithm: HashAlgorithm,
    ) -> Result<Vec<(u8, [u8; 32])>, TpmUnlockError> {
        let mut values = Vec::new();

        for pcr in 0..24 {
            if mask & (1 << pcr) != 0 {
                // Would use TPM2_PCR_Read
                let value = [0u8; 32]; // Placeholder
                values.push((pcr, value));
            }
        }

        Ok(values)
    }

    /// Read NV index
    fn read_nv_index(&self, _nv_index: u32) -> Result<Vec<u8>, TpmUnlockError> {
        // Would use TPM2_NV_Read
        // Placeholder - return empty sealed blob
        Ok(vec![0u8; 128])
    }

    /// Unseal blob
    fn unseal_blob(
        &self,
        _sealed: &[u8],
        _policy: &PcrPolicy,
        _pin: Option<&str>,
    ) -> Result<String, TpmUnlockError> {
        // Would use TPM2_Unseal with policy session
        // Placeholder
        Ok("unsealed_passphrase".to_string())
    }

    /// Open LUKS device
    fn open_luks(
        &self,
        _device: &str,
        _passphrase: &str,
        _slot: u8,
    ) -> Result<(), TpmUnlockError> {
        // Would call cryptsetup open
        kprintln!("tpm-disk-unlock: Opening LUKS device");
        Ok(())
    }

    /// Unlock with password fallback
    pub fn unlock_with_password(&mut self, device: &str, password: &str) -> UnlockResult {
        kprintln!("tpm-disk-unlock: Password unlock for {}", device);

        let disk = match self.disks.iter().find(|d| d.device == device) {
            Some(d) => d,
            None => return UnlockResult::NotEnrolled,
        };

        // Find password slot
        let slot = disk.slots.iter()
            .find(|s| s.method == UnlockMethod::Password)
            .map(|s| s.luks_slot)
            .unwrap_or(0);

        let start = crate::time::uptime_ms();

        if self.open_luks(device, password, slot).is_ok() {
            STATS.successful_unlocks.fetch_add(1, Ordering::Relaxed);
            UnlockResult::Success {
                method: UnlockMethod::Password,
                slot,
                duration_ms: crate::time::uptime_ms() - start,
            }
        } else {
            STATS.failed_unlocks.fetch_add(1, Ordering::Relaxed);
            UnlockResult::FailedPassword
        }
    }

    /// Unlock with recovery key
    pub fn unlock_with_recovery(&mut self, device: &str, recovery_key: &str) -> UnlockResult {
        kprintln!("tpm-disk-unlock: Recovery key unlock for {}", device);

        let disk = match self.disks.iter().find(|d| d.device == device) {
            Some(d) => d,
            None => return UnlockResult::NotEnrolled,
        };

        // Find recovery slot
        let slot = disk.slots.iter()
            .find(|s| s.method == UnlockMethod::RecoveryKey)
            .map(|s| s.luks_slot)
            .unwrap_or(7);

        let start = crate::time::uptime_ms();

        // Convert recovery key to passphrase
        let passphrase = recovery_key.replace('-', "").to_lowercase();

        if self.open_luks(device, &passphrase, slot).is_ok() {
            STATS.successful_unlocks.fetch_add(1, Ordering::Relaxed);
            UnlockResult::Success {
                method: UnlockMethod::RecoveryKey,
                slot,
                duration_ms: crate::time::uptime_ms() - start,
            }
        } else {
            STATS.failed_unlocks.fetch_add(1, Ordering::Relaxed);
            UnlockResult::FailedRecovery
        }
    }

    /// Auto-unlock all enrolled disks
    pub fn auto_unlock_all(&mut self) -> Vec<(String, UnlockResult)> {
        let devices: Vec<String> = self.disks.iter()
            .filter(|d| d.auto_unlock)
            .map(|d| d.device.clone())
            .collect();

        let mut results = Vec::new();

        for device in devices {
            let result = self.unlock(&device, None);
            results.push((device, result));
        }

        results
    }

    /// Get enrolled disks
    pub fn get_disks(&self) -> &[DiskConfig] {
        &self.disks
    }

    /// Get disk by device
    pub fn get_disk(&self, device: &str) -> Option<&DiskConfig> {
        self.disks.iter().find(|d| d.device == device)
    }

    /// Check if TPM is available
    pub fn is_tpm_available(&self) -> bool {
        self.tpm_available
    }

    /// Get TPM version
    pub fn tpm_version(&self) -> Option<TpmVersion> {
        self.tpm_version
    }

    /// Re-seal key after PCR change (e.g., kernel update)
    pub fn reseal(
        &mut self,
        device: &str,
        slot: u8,
        passphrase: &str,
    ) -> Result<(), TpmUnlockError> {
        kprintln!("tpm-disk-unlock: Resealing slot {} for {}", slot, device);

        // Extract nv_index and policy info first
        let (nv_index, policy) = {
            let disk = self.disks.iter().find(|d| d.device == device)
                .ok_or(TpmUnlockError::SealedKeyNotFound)?;
            let enrolled = disk.slots.iter().find(|s| s.luks_slot == slot)
                .ok_or(TpmUnlockError::SealedKeyNotFound)?;

            match (enrolled.nv_index, enrolled.pcr_policy.clone()) {
                (Some(nv), Some(p)) => (nv, p),
                _ => return Err(TpmUnlockError::PolicyNotSatisfied),
            }
        };

        // Clear old sealed data
        self.clear_nv_index(nv_index)?;

        // Seal with current PCR values
        self.seal_key_to_tpm(passphrase, nv_index, &policy, None)?;

        kprintln!("tpm-disk-unlock: Resealed successfully");
        Ok(())
    }

    /// Format status
    pub fn format_status(&self) -> String {
        use alloc::fmt::Write;
        let mut s = String::new();

        let _ = writeln!(s, "TPM Disk Unlock Status:");
        let _ = writeln!(s, "  TPM Available: {}", self.tpm_available);
        if let Some(v) = self.tpm_version {
            let _ = writeln!(s, "  TPM Version: {}.{}.{}", v.major, v.minor, v.revision);
        }
        let _ = writeln!(s, "  Enrolled Disks: {}", self.disks.len());

        for disk in &self.disks {
            let _ = writeln!(s, "\n  Device: {}", disk.device);
            let _ = writeln!(s, "    UUID: {}", disk.uuid);
            let _ = writeln!(s, "    Auto-unlock: {}", disk.auto_unlock);
            let _ = writeln!(s, "    Slots: {}", disk.slots.len());

            for slot in &disk.slots {
                let _ = writeln!(s, "      Slot {}: {} ({})",
                    slot.luks_slot, slot.method.as_str(), slot.description);
            }
        }

        s
    }
}

impl Default for TpmDiskUnlock {
    fn default() -> Self {
        Self::new()
    }
}

// === Public API ===

/// Initialize TPM disk unlock
pub fn init() {
    let mut guard = TPM_DISK_UNLOCK.lock();
    if guard.is_none() {
        let mut unlock = TpmDiskUnlock::new();
        if let Err(e) = unlock.init() {
            kprintln!("tpm-disk-unlock: Init failed: {:?}", e);
        }
        *guard = Some(unlock);
    }
}

/// Execute a function with access to the manager
pub fn with_manager<F, R>(f: F) -> R
where
    F: FnOnce(&TpmDiskUnlock) -> R,
{
    let guard = TPM_DISK_UNLOCK.lock();
    f(guard.as_ref().expect("TPM disk unlock not initialized"))
}

/// Execute a function with mutable access to the manager
pub fn with_manager_mut<F, R>(f: F) -> R
where
    F: FnOnce(&mut TpmDiskUnlock) -> R,
{
    let mut guard = TPM_DISK_UNLOCK.lock();
    f(guard.as_mut().expect("TPM disk unlock not initialized"))
}

/// Check if TPM is available
pub fn is_tpm_available() -> bool {
    TPM_DISK_UNLOCK.lock().as_ref()
        .map(|m| m.is_tpm_available())
        .unwrap_or(false)
}

/// Enroll disk for TPM unlock
pub fn enroll(device: &str, options: EnrollOptions) -> Result<EnrolledSlot, TpmUnlockError> {
    TPM_DISK_UNLOCK.lock().as_mut()
        .expect("Not initialized")
        .enroll(device, options)
}

/// Unenroll disk slot
pub fn unenroll(device: &str, slot: u8) -> Result<(), TpmUnlockError> {
    TPM_DISK_UNLOCK.lock().as_mut()
        .expect("Not initialized")
        .unenroll(device, slot)
}

/// Unlock disk
pub fn unlock(device: &str, pin: Option<&str>) -> UnlockResult {
    TPM_DISK_UNLOCK.lock().as_mut()
        .expect("Not initialized")
        .unlock(device, pin)
}

/// Unlock with password
pub fn unlock_with_password(device: &str, password: &str) -> UnlockResult {
    TPM_DISK_UNLOCK.lock().as_mut()
        .expect("Not initialized")
        .unlock_with_password(device, password)
}

/// Unlock with recovery key
pub fn unlock_with_recovery(device: &str, recovery_key: &str) -> UnlockResult {
    TPM_DISK_UNLOCK.lock().as_mut()
        .expect("Not initialized")
        .unlock_with_recovery(device, recovery_key)
}

/// Auto-unlock all disks
pub fn auto_unlock_all() -> Vec<(String, UnlockResult)> {
    TPM_DISK_UNLOCK.lock().as_mut()
        .expect("Not initialized")
        .auto_unlock_all()
}

/// Reseal after PCR change
pub fn reseal(device: &str, slot: u8, passphrase: &str) -> Result<(), TpmUnlockError> {
    TPM_DISK_UNLOCK.lock().as_mut()
        .expect("Not initialized")
        .reseal(device, slot, passphrase)
}

/// Get statistics
pub fn stats() -> (u64, u64, u64, u64) {
    (
        STATS.unlock_attempts.load(Ordering::Relaxed),
        STATS.successful_unlocks.load(Ordering::Relaxed),
        STATS.failed_unlocks.load(Ordering::Relaxed),
        STATS.pcr_mismatches.load(Ordering::Relaxed),
    )
}

/// Format status
pub fn format_status() -> String {
    TPM_DISK_UNLOCK.lock().as_ref()
        .map(|m| m.format_status())
        .unwrap_or_else(|| "TPM Disk Unlock: Not initialized".to_string())
}
