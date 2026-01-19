//! TPM Key Storage
//!
//! Secure key storage using TPM 2.0 hierarchies, sealing, and NVRAM.
//!
//! ## Features
//! - Key generation (RSA, ECC)
//! - Key sealing to PCR state
//! - Key import/export
//! - NVRAM key storage
//! - Key hierarchies (storage, endorsement, platform)

#![allow(dead_code)]

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use alloc::collections::BTreeMap;
use core::sync::atomic::{AtomicU32, Ordering};

use crate::sync::IrqSafeMutex;

/// TPM 2.0 hierarchy handles
pub mod hierarchies {
    pub const TPM_RH_OWNER: u32 = 0x40000001;
    pub const TPM_RH_NULL: u32 = 0x40000007;
    pub const TPM_RH_ENDORSEMENT: u32 = 0x4000000B;
    pub const TPM_RH_PLATFORM: u32 = 0x4000000C;
    pub const TPM_RH_LOCKOUT: u32 = 0x4000000A;
}

/// TPM 2.0 persistent handle ranges
pub mod handle_ranges {
    pub const PERSISTENT_FIRST: u32 = 0x81000000;
    pub const PERSISTENT_LAST: u32 = 0x81FFFFFF;
    pub const NV_INDEX_FIRST: u32 = 0x01000000;
    pub const NV_INDEX_LAST: u32 = 0x01FFFFFF;
    pub const TRANSIENT_FIRST: u32 = 0x80000000;
    pub const TRANSIENT_LAST: u32 = 0x80FFFFFF;
}

/// Key types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyType {
    /// RSA key
    Rsa2048,
    Rsa3072,
    Rsa4096,
    /// ECC key
    EccP256,
    EccP384,
    EccP521,
    /// AES symmetric key
    Aes128,
    Aes256,
    /// HMAC key
    HmacSha256,
}

impl KeyType {
    pub fn algorithm_id(&self) -> u16 {
        match self {
            KeyType::Rsa2048 | KeyType::Rsa3072 | KeyType::Rsa4096 => 0x0001, // RSA
            KeyType::EccP256 | KeyType::EccP384 | KeyType::EccP521 => 0x0023, // ECC
            KeyType::Aes128 | KeyType::Aes256 => 0x0006, // AES
            KeyType::HmacSha256 => 0x000B, // SHA256
        }
    }

    pub fn key_bits(&self) -> u16 {
        match self {
            KeyType::Rsa2048 => 2048,
            KeyType::Rsa3072 => 3072,
            KeyType::Rsa4096 => 4096,
            KeyType::EccP256 => 256,
            KeyType::EccP384 => 384,
            KeyType::EccP521 => 521,
            KeyType::Aes128 => 128,
            KeyType::Aes256 => 256,
            KeyType::HmacSha256 => 256,
        }
    }

    pub fn ecc_curve(&self) -> Option<u16> {
        match self {
            KeyType::EccP256 => Some(0x0003), // NIST P-256
            KeyType::EccP384 => Some(0x0004), // NIST P-384
            KeyType::EccP521 => Some(0x0005), // NIST P-521
            _ => None,
        }
    }
}

/// Key usage flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyUsage {
    /// Key can be used for signing
    pub sign: bool,
    /// Key can be used for encryption/decryption
    pub decrypt: bool,
    /// Key can be used to derive other keys
    pub derive: bool,
    /// Key can be used for restricted operations only
    pub restricted: bool,
    /// Key is a storage key (can wrap other keys)
    pub storage: bool,
}

impl Default for KeyUsage {
    fn default() -> Self {
        Self {
            sign: false,
            decrypt: false,
            derive: false,
            restricted: false,
            storage: false,
        }
    }
}

impl KeyUsage {
    pub fn signing() -> Self {
        Self {
            sign: true,
            ..Default::default()
        }
    }

    pub fn encryption() -> Self {
        Self {
            decrypt: true,
            ..Default::default()
        }
    }

    pub fn storage() -> Self {
        Self {
            decrypt: true,
            restricted: true,
            storage: true,
            ..Default::default()
        }
    }

    pub fn to_attributes(&self) -> u32 {
        let mut attrs = 0u32;

        // Object attributes
        attrs |= 1 << 1;  // userWithAuth
        attrs |= 1 << 4;  // fixedTPM
        attrs |= 1 << 5;  // fixedParent
        attrs |= 1 << 6;  // sensitiveDataOrigin

        if self.sign {
            attrs |= 1 << 18; // sign
        }
        if self.decrypt {
            attrs |= 1 << 17; // decrypt
        }
        if self.restricted {
            attrs |= 1 << 16; // restricted
        }

        attrs
    }
}

/// PCR selection for sealing
#[derive(Debug, Clone)]
pub struct PcrSelection {
    pub hash_alg: u16,
    pub pcr_select: [u8; 3],
}

impl PcrSelection {
    pub fn new(hash_alg: u16) -> Self {
        Self {
            hash_alg,
            pcr_select: [0; 3],
        }
    }

    pub fn select_pcr(&mut self, pcr: u8) {
        if pcr < 24 {
            self.pcr_select[(pcr / 8) as usize] |= 1 << (pcr % 8);
        }
    }

    pub fn select_boot_pcrs(&mut self) {
        // PCR 0-7 are boot measurements
        self.select_pcr(0);  // BIOS
        self.select_pcr(1);  // BIOS Configuration
        self.select_pcr(2);  // Option ROMs
        self.select_pcr(3);  // Option ROM Configuration
        self.select_pcr(4);  // MBR/Boot Loader
        self.select_pcr(5);  // Boot Configuration
        self.select_pcr(6);  // Platform-specific
        self.select_pcr(7);  // Secure Boot state
    }

    pub fn select_linux_pcrs(&mut self) {
        // Linux-specific PCRs
        self.select_pcr(8);  // Kernel command line
        self.select_pcr(9);  // Initrd
        self.select_pcr(10); // Kernel
        self.select_pcr(11); // Future use
    }
}

/// Sealed data policy
#[derive(Debug, Clone)]
pub struct SealPolicy {
    /// PCR binding
    pub pcr_selection: Option<PcrSelection>,
    /// Authorization value (password)
    pub auth_value: Option<Vec<u8>>,
    /// Policy hash
    pub policy_digest: Option<Vec<u8>>,
}

impl Default for SealPolicy {
    fn default() -> Self {
        Self {
            pcr_selection: None,
            auth_value: None,
            policy_digest: None,
        }
    }
}

impl SealPolicy {
    /// Create policy bound to boot PCRs
    pub fn boot_sealed() -> Self {
        let mut pcr = PcrSelection::new(0x000B); // SHA-256
        pcr.select_boot_pcrs();
        Self {
            pcr_selection: Some(pcr),
            ..Default::default()
        }
    }

    /// Create password-protected policy
    pub fn password(password: &[u8]) -> Self {
        Self {
            auth_value: Some(password.to_vec()),
            ..Default::default()
        }
    }

    /// Create policy with both PCR and password
    pub fn combined(password: &[u8]) -> Self {
        let mut pcr = PcrSelection::new(0x000B);
        pcr.select_boot_pcrs();
        Self {
            pcr_selection: Some(pcr),
            auth_value: Some(password.to_vec()),
            policy_digest: None,
        }
    }
}

/// Key handle
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyHandle(u32);

impl KeyHandle {
    pub fn new(handle: u32) -> Self {
        Self(handle)
    }

    pub fn value(&self) -> u32 {
        self.0
    }

    pub fn is_persistent(&self) -> bool {
        self.0 >= handle_ranges::PERSISTENT_FIRST && self.0 <= handle_ranges::PERSISTENT_LAST
    }

    pub fn is_transient(&self) -> bool {
        self.0 >= handle_ranges::TRANSIENT_FIRST && self.0 <= handle_ranges::TRANSIENT_LAST
    }
}

/// Key information
#[derive(Debug, Clone)]
pub struct KeyInfo {
    pub handle: KeyHandle,
    pub key_type: KeyType,
    pub usage: KeyUsage,
    pub name: String,
    pub parent_handle: u32,
    pub is_persistent: bool,
    pub creation_time: u64,
}

/// NVRAM index attributes
#[derive(Debug, Clone, Copy)]
pub struct NvAttributes {
    pub owner_write: bool,
    pub owner_read: bool,
    pub auth_write: bool,
    pub auth_read: bool,
    pub policy_write: bool,
    pub policy_read: bool,
    pub policy_delete: bool,
    pub write_stclear: bool,
    pub read_stclear: bool,
    pub written: bool,
    pub platform_create: bool,
}

impl Default for NvAttributes {
    fn default() -> Self {
        Self {
            owner_write: true,
            owner_read: true,
            auth_write: false,
            auth_read: false,
            policy_write: false,
            policy_read: false,
            policy_delete: false,
            write_stclear: false,
            read_stclear: false,
            written: false,
            platform_create: false,
        }
    }
}

impl NvAttributes {
    pub fn to_u32(&self) -> u32 {
        let mut attrs = 0u32;
        if self.owner_write { attrs |= 1 << 0; }
        if self.owner_read { attrs |= 1 << 1; }
        if self.auth_write { attrs |= 1 << 2; }
        if self.auth_read { attrs |= 1 << 3; }
        if self.policy_write { attrs |= 1 << 5; }
        if self.policy_read { attrs |= 1 << 4; }
        if self.policy_delete { attrs |= 1 << 10; }
        if self.write_stclear { attrs |= 1 << 14; }
        if self.read_stclear { attrs |= 1 << 15; }
        if self.written { attrs |= 1 << 29; }
        if self.platform_create { attrs |= 1 << 30; }
        attrs
    }
}

/// NVRAM index info
#[derive(Debug, Clone)]
pub struct NvIndex {
    pub index: u32,
    pub size: u16,
    pub attributes: NvAttributes,
    pub auth_policy: Option<Vec<u8>>,
}

/// Key storage error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyStorageError {
    TpmNotPresent,
    TpmError(u32),
    KeyNotFound,
    HandleExhausted,
    InvalidParameter,
    AuthFailed,
    PolicyFailed,
    NvramFull,
    OperationFailed,
}

pub type KeyStorageResult<T> = Result<T, KeyStorageError>;

/// TPM Key Storage Manager
pub struct TpmKeyStorage {
    /// Storage root key handle
    srk_handle: Option<KeyHandle>,
    /// Endorsement key handle
    ek_handle: Option<KeyHandle>,
    /// Loaded keys
    loaded_keys: BTreeMap<u32, KeyInfo>,
    /// NVRAM indices in use
    nv_indices: BTreeMap<u32, NvIndex>,
    /// Next transient handle
    next_transient: AtomicU32,
    /// Initialized
    initialized: bool,
}

impl TpmKeyStorage {
    /// Create new key storage manager
    pub fn new() -> Self {
        Self {
            srk_handle: None,
            ek_handle: None,
            loaded_keys: BTreeMap::new(),
            nv_indices: BTreeMap::new(),
            next_transient: AtomicU32::new(handle_ranges::TRANSIENT_FIRST),
            initialized: false,
        }
    }

    /// Initialize with TPM
    pub fn init(&mut self) -> KeyStorageResult<()> {
        // Create or load Storage Root Key (SRK)
        self.create_or_load_srk()?;

        // Create or load Endorsement Key (EK)
        self.create_or_load_ek()?;

        self.initialized = true;
        crate::kprintln!("tpm-keys: Key storage initialized");
        Ok(())
    }

    /// Create or load Storage Root Key
    fn create_or_load_srk(&mut self) -> KeyStorageResult<()> {
        // SRK is typically at persistent handle 0x81000001
        let srk_handle = 0x81000001u32;

        // Try to load existing SRK
        // In real implementation, would use TPM2_ReadPublic

        // If not found, create new SRK
        // TPM2_CreatePrimary in owner hierarchy
        self.srk_handle = Some(KeyHandle::new(srk_handle));
        Ok(())
    }

    /// Create or load Endorsement Key
    fn create_or_load_ek(&mut self) -> KeyStorageResult<()> {
        // EK is typically at persistent handle 0x81010001
        let ek_handle = 0x81010001u32;
        self.ek_handle = Some(KeyHandle::new(ek_handle));
        Ok(())
    }

    /// Create a new key
    pub fn create_key(
        &mut self,
        key_type: KeyType,
        usage: KeyUsage,
        name: &str,
        policy: Option<SealPolicy>,
    ) -> KeyStorageResult<KeyHandle> {
        if !self.initialized {
            return Err(KeyStorageError::TpmNotPresent);
        }

        let srk = self.srk_handle.ok_or(KeyStorageError::TpmNotPresent)?;

        // Allocate handle
        let handle = self.allocate_transient_handle();

        // Build TPM2_Create command
        // In real implementation, would send command to TPM

        crate::kprintln!("tpm-keys: Creating {} key '{}'",
            match key_type {
                KeyType::Rsa2048 => "RSA-2048",
                KeyType::Rsa3072 => "RSA-3072",
                KeyType::Rsa4096 => "RSA-4096",
                KeyType::EccP256 => "ECC-P256",
                KeyType::EccP384 => "ECC-P384",
                KeyType::EccP521 => "ECC-P521",
                KeyType::Aes128 => "AES-128",
                KeyType::Aes256 => "AES-256",
                KeyType::HmacSha256 => "HMAC-SHA256",
            },
            name
        );

        // Record key info
        let info = KeyInfo {
            handle,
            key_type,
            usage,
            name: name.to_string(),
            parent_handle: srk.value(),
            is_persistent: false,
            creation_time: crate::time::ticks(),
        };

        self.loaded_keys.insert(handle.value(), info);

        Ok(handle)
    }

    /// Seal data to PCR state
    pub fn seal_data(
        &mut self,
        data: &[u8],
        policy: &SealPolicy,
        name: &str,
    ) -> KeyStorageResult<KeyHandle> {
        if !self.initialized {
            return Err(KeyStorageError::TpmNotPresent);
        }

        let _srk = self.srk_handle.ok_or(KeyStorageError::TpmNotPresent)?;
        let handle = self.allocate_transient_handle();

        // Build TPM2_Create command for sealed data object
        // In real implementation, would:
        // 1. Create policy session if PCR binding
        // 2. Call TPM2_Create with sealed data blob
        // 3. Load the sealed object

        crate::kprintln!("tpm-keys: Sealing {} bytes as '{}'", data.len(), name);

        let info = KeyInfo {
            handle,
            key_type: KeyType::HmacSha256, // Use for sealed data
            usage: KeyUsage::default(),
            name: name.to_string(),
            parent_handle: self.srk_handle.map(|h| h.value()).unwrap_or(0),
            is_persistent: false,
            creation_time: crate::time::ticks(),
        };

        self.loaded_keys.insert(handle.value(), info);

        Ok(handle)
    }

    /// Unseal data
    pub fn unseal_data(
        &self,
        handle: KeyHandle,
        auth: Option<&[u8]>,
    ) -> KeyStorageResult<Vec<u8>> {
        if !self.initialized {
            return Err(KeyStorageError::TpmNotPresent);
        }

        if !self.loaded_keys.contains_key(&handle.value()) {
            return Err(KeyStorageError::KeyNotFound);
        }

        // In real implementation, would:
        // 1. Start policy session
        // 2. Satisfy policy (PCR read, auth)
        // 3. Call TPM2_Unseal
        // 4. Return plaintext

        crate::kprintln!("tpm-keys: Unsealing data from handle 0x{:08X}", handle.value());

        // Return mock data for now
        Ok(vec![0; 32])
    }

    /// Make key persistent
    pub fn make_persistent(
        &mut self,
        handle: KeyHandle,
        persistent_handle: u32,
    ) -> KeyStorageResult<KeyHandle> {
        if !self.initialized {
            return Err(KeyStorageError::TpmNotPresent);
        }

        // Validate handle range
        if persistent_handle < handle_ranges::PERSISTENT_FIRST ||
           persistent_handle > handle_ranges::PERSISTENT_LAST {
            return Err(KeyStorageError::InvalidParameter);
        }

        // In real implementation, would call TPM2_EvictControl

        crate::kprintln!("tpm-keys: Making handle 0x{:08X} persistent at 0x{:08X}",
            handle.value(), persistent_handle);

        // Update key info
        if let Some(info) = self.loaded_keys.get_mut(&handle.value()) {
            info.is_persistent = true;
        }

        Ok(KeyHandle::new(persistent_handle))
    }

    /// Load key from persistent storage
    pub fn load_key(&mut self, persistent_handle: u32) -> KeyStorageResult<KeyHandle> {
        if !self.initialized {
            return Err(KeyStorageError::TpmNotPresent);
        }

        // In real implementation, would call TPM2_ReadPublic first
        // Then TPM2_Load if needed

        crate::kprintln!("tpm-keys: Loading persistent handle 0x{:08X}", persistent_handle);

        Ok(KeyHandle::new(persistent_handle))
    }

    /// Sign data with key
    pub fn sign(
        &self,
        handle: KeyHandle,
        data: &[u8],
        auth: Option<&[u8]>,
    ) -> KeyStorageResult<Vec<u8>> {
        if !self.initialized {
            return Err(KeyStorageError::TpmNotPresent);
        }

        let info = self.loaded_keys.get(&handle.value())
            .ok_or(KeyStorageError::KeyNotFound)?;

        if !info.usage.sign {
            return Err(KeyStorageError::InvalidParameter);
        }

        // In real implementation, would:
        // 1. Hash data if needed
        // 2. Call TPM2_Sign with appropriate scheme
        // 3. Return signature

        crate::kprintln!("tpm-keys: Signing {} bytes with handle 0x{:08X}",
            data.len(), handle.value());

        // Return mock signature
        let sig_len = match info.key_type {
            KeyType::Rsa2048 => 256,
            KeyType::Rsa3072 => 384,
            KeyType::Rsa4096 => 512,
            KeyType::EccP256 => 64,
            KeyType::EccP384 => 96,
            KeyType::EccP521 => 132,
            _ => 64,
        };

        Ok(vec![0; sig_len])
    }

    /// Decrypt data with key
    pub fn decrypt(
        &self,
        handle: KeyHandle,
        ciphertext: &[u8],
        auth: Option<&[u8]>,
    ) -> KeyStorageResult<Vec<u8>> {
        if !self.initialized {
            return Err(KeyStorageError::TpmNotPresent);
        }

        let info = self.loaded_keys.get(&handle.value())
            .ok_or(KeyStorageError::KeyNotFound)?;

        if !info.usage.decrypt {
            return Err(KeyStorageError::InvalidParameter);
        }

        // In real implementation, would call TPM2_RSA_Decrypt or TPM2_ECDH_ZGen

        crate::kprintln!("tpm-keys: Decrypting {} bytes with handle 0x{:08X}",
            ciphertext.len(), handle.value());

        // Return mock plaintext
        Ok(vec![0; ciphertext.len()])
    }

    /// Create NVRAM index
    pub fn create_nv_index(
        &mut self,
        index: u32,
        size: u16,
        attributes: NvAttributes,
    ) -> KeyStorageResult<()> {
        if !self.initialized {
            return Err(KeyStorageError::TpmNotPresent);
        }

        // Validate index range
        if index < handle_ranges::NV_INDEX_FIRST || index > handle_ranges::NV_INDEX_LAST {
            return Err(KeyStorageError::InvalidParameter);
        }

        // In real implementation, would call TPM2_NV_DefineSpace

        crate::kprintln!("tpm-keys: Creating NV index 0x{:08X} ({} bytes)", index, size);

        let nv = NvIndex {
            index,
            size,
            attributes,
            auth_policy: None,
        };

        self.nv_indices.insert(index, nv);
        Ok(())
    }

    /// Write to NVRAM
    pub fn nv_write(
        &self,
        index: u32,
        data: &[u8],
        offset: u16,
        auth: Option<&[u8]>,
    ) -> KeyStorageResult<()> {
        if !self.initialized {
            return Err(KeyStorageError::TpmNotPresent);
        }

        let nv = self.nv_indices.get(&index)
            .ok_or(KeyStorageError::KeyNotFound)?;

        if offset as usize + data.len() > nv.size as usize {
            return Err(KeyStorageError::InvalidParameter);
        }

        // In real implementation, would call TPM2_NV_Write

        crate::kprintln!("tpm-keys: Writing {} bytes to NV index 0x{:08X} at offset {}",
            data.len(), index, offset);

        Ok(())
    }

    /// Read from NVRAM
    pub fn nv_read(
        &self,
        index: u32,
        size: u16,
        offset: u16,
        auth: Option<&[u8]>,
    ) -> KeyStorageResult<Vec<u8>> {
        if !self.initialized {
            return Err(KeyStorageError::TpmNotPresent);
        }

        let nv = self.nv_indices.get(&index)
            .ok_or(KeyStorageError::KeyNotFound)?;

        if offset as usize + size as usize > nv.size as usize {
            return Err(KeyStorageError::InvalidParameter);
        }

        // In real implementation, would call TPM2_NV_Read

        crate::kprintln!("tpm-keys: Reading {} bytes from NV index 0x{:08X} at offset {}",
            size, index, offset);

        Ok(vec![0; size as usize])
    }

    /// Delete NVRAM index
    pub fn delete_nv_index(&mut self, index: u32) -> KeyStorageResult<()> {
        if !self.initialized {
            return Err(KeyStorageError::TpmNotPresent);
        }

        // In real implementation, would call TPM2_NV_UndefineSpace
        self.nv_indices.remove(&index);

        crate::kprintln!("tpm-keys: Deleted NV index 0x{:08X}", index);
        Ok(())
    }

    /// Flush transient key
    pub fn flush_key(&mut self, handle: KeyHandle) -> KeyStorageResult<()> {
        if handle.is_persistent() {
            return Err(KeyStorageError::InvalidParameter);
        }

        // In real implementation, would call TPM2_FlushContext
        self.loaded_keys.remove(&handle.value());

        crate::kprintln!("tpm-keys: Flushed handle 0x{:08X}", handle.value());
        Ok(())
    }

    /// Get key info
    pub fn key_info(&self, handle: KeyHandle) -> Option<&KeyInfo> {
        self.loaded_keys.get(&handle.value())
    }

    /// List loaded keys
    pub fn list_keys(&self) -> Vec<&KeyInfo> {
        self.loaded_keys.values().collect()
    }

    /// List NVRAM indices
    pub fn list_nv_indices(&self) -> Vec<&NvIndex> {
        self.nv_indices.values().collect()
    }

    /// Allocate transient handle
    fn allocate_transient_handle(&self) -> KeyHandle {
        let handle = self.next_transient.fetch_add(1, Ordering::Relaxed);
        if handle > handle_ranges::TRANSIENT_LAST {
            self.next_transient.store(handle_ranges::TRANSIENT_FIRST, Ordering::Relaxed);
        }
        KeyHandle::new(handle)
    }

    /// Get SRK handle
    pub fn srk_handle(&self) -> Option<KeyHandle> {
        self.srk_handle
    }

    /// Get EK handle
    pub fn ek_handle(&self) -> Option<KeyHandle> {
        self.ek_handle
    }

    /// Format status
    pub fn format_status(&self) -> String {
        alloc::format!(
            "TPM Key Storage: initialized={} srk={} ek={} keys={} nv_indices={}",
            self.initialized,
            self.srk_handle.map(|h| alloc::format!("0x{:08X}", h.value())).unwrap_or_else(|| "none".to_string()),
            self.ek_handle.map(|h| alloc::format!("0x{:08X}", h.value())).unwrap_or_else(|| "none".to_string()),
            self.loaded_keys.len(),
            self.nv_indices.len()
        )
    }
}

impl Default for TpmKeyStorage {
    fn default() -> Self {
        Self::new()
    }
}

// Global key storage
static TPM_KEY_STORAGE: IrqSafeMutex<Option<TpmKeyStorage>> = IrqSafeMutex::new(None);

/// Initialize TPM key storage
pub fn init() -> KeyStorageResult<()> {
    let mut storage = TpmKeyStorage::new();
    storage.init()?;
    *TPM_KEY_STORAGE.lock() = Some(storage);
    Ok(())
}

/// Create a new key
pub fn create_key(
    key_type: KeyType,
    usage: KeyUsage,
    name: &str,
) -> KeyStorageResult<KeyHandle> {
    TPM_KEY_STORAGE.lock()
        .as_mut()
        .ok_or(KeyStorageError::TpmNotPresent)?
        .create_key(key_type, usage, name, None)
}

/// Seal data
pub fn seal_data(data: &[u8], name: &str) -> KeyStorageResult<KeyHandle> {
    TPM_KEY_STORAGE.lock()
        .as_mut()
        .ok_or(KeyStorageError::TpmNotPresent)?
        .seal_data(data, &SealPolicy::boot_sealed(), name)
}

/// Unseal data
pub fn unseal_data(handle: KeyHandle) -> KeyStorageResult<Vec<u8>> {
    TPM_KEY_STORAGE.lock()
        .as_ref()
        .ok_or(KeyStorageError::TpmNotPresent)?
        .unseal_data(handle, None)
}

/// Get status
pub fn status() -> String {
    TPM_KEY_STORAGE.lock()
        .as_ref()
        .map(|s| s.format_status())
        .unwrap_or_else(|| "TPM Key Storage not initialized".to_string())
}
