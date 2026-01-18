//! UEFI Secure Boot Support
//!
//! Implements Secure Boot verification and key management for UEFI systems.
//! Verifies boot components using the UEFI db/dbx signature databases.
//!
//! ## Features
//! - Secure Boot state detection
//! - Signature database parsing (db, dbx, KEK, PK)
//! - Certificate verification (X.509, Authenticode)
//! - MOK (Machine Owner Key) support
//! - Shim loader compatibility

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::sync::IrqSafeMutex;
use crate::util::{KError, KResult};

/// EFI GUID type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct EfiGuid {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

impl EfiGuid {
    pub const fn new(data1: u32, data2: u16, data3: u16, data4: [u8; 8]) -> Self {
        Self { data1, data2, data3, data4 }
    }
}

/// Well-known GUIDs
pub mod guids {
    use super::EfiGuid;

    /// Global Variable GUID
    pub const EFI_GLOBAL_VARIABLE: EfiGuid = EfiGuid::new(
        0x8BE4DF61, 0x93CA, 0x11D2,
        [0xAA, 0x0D, 0x00, 0xE0, 0x98, 0x03, 0x2B, 0x8C]
    );

    /// Image Security Database GUID
    pub const EFI_IMAGE_SECURITY_DATABASE: EfiGuid = EfiGuid::new(
        0xD719B2CB, 0x3D3A, 0x4596,
        [0xA3, 0xBC, 0xDA, 0xD0, 0x0E, 0x67, 0x65, 0x6F]
    );

    /// Certificate X.509 GUID
    pub const EFI_CERT_X509_GUID: EfiGuid = EfiGuid::new(
        0xA5C059A1, 0x94E4, 0x4AA7,
        [0x87, 0xB5, 0xAB, 0x15, 0x5C, 0x2B, 0xF0, 0x72]
    );

    /// Certificate SHA256 GUID
    pub const EFI_CERT_SHA256_GUID: EfiGuid = EfiGuid::new(
        0xC1C41626, 0x504C, 0x4092,
        [0xAC, 0xA9, 0x41, 0xF9, 0x36, 0x93, 0x43, 0x28]
    );

    /// Certificate RSA2048 GUID
    pub const EFI_CERT_RSA2048_GUID: EfiGuid = EfiGuid::new(
        0x3C5766E8, 0x269C, 0x4E34,
        [0xAA, 0x14, 0xED, 0x77, 0x6E, 0x85, 0xB3, 0xB6]
    );

    /// Shim MOK Database GUID
    pub const SHIM_LOCK_GUID: EfiGuid = EfiGuid::new(
        0x605DAB50, 0xE046, 0x4300,
        [0xAB, 0xB6, 0x3D, 0xD8, 0x10, 0xDD, 0x8B, 0x23]
    );
}

/// Secure Boot mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecureBootMode {
    /// Secure Boot is disabled
    Disabled,
    /// Secure Boot is in Setup Mode (can enroll keys)
    SetupMode,
    /// Secure Boot is enabled and enforcing
    Enabled,
    /// Secure Boot is in Audit Mode (log but don't enforce)
    AuditMode,
    /// Secure Boot is in Deployed Mode (locked)
    DeployedMode,
}

/// Signature database entry type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureType {
    /// SHA-256 hash
    Sha256,
    /// SHA-384 hash
    Sha384,
    /// SHA-512 hash
    Sha512,
    /// RSA-2048 public key
    Rsa2048,
    /// X.509 certificate
    X509,
    /// X.509 certificate chain
    X509Chain,
    /// Unknown type
    Unknown,
}

/// A signature database entry
#[derive(Debug, Clone)]
pub struct SignatureEntry {
    /// Entry type
    pub sig_type: SignatureType,
    /// Owner GUID
    pub owner: EfiGuid,
    /// Signature data (certificate or hash)
    pub data: Vec<u8>,
}

/// Signature database (db, dbx, KEK, PK)
#[derive(Debug, Clone)]
pub struct SignatureDatabase {
    /// Database name
    pub name: String,
    /// Entries
    pub entries: Vec<SignatureEntry>,
}

impl SignatureDatabase {
    pub fn new(name: &str) -> Self {
        Self {
            name: String::from(name),
            entries: Vec::new(),
        }
    }

    /// Parse from EFI variable data
    pub fn from_efi_var(name: &str, data: &[u8]) -> Option<Self> {
        let mut db = Self::new(name);

        // EFI_SIGNATURE_LIST format:
        // [SignatureType: 16 bytes GUID]
        // [SignatureListSize: 4 bytes]
        // [SignatureHeaderSize: 4 bytes]
        // [SignatureSize: 4 bytes]
        // [SignatureHeader: SignatureHeaderSize bytes]
        // [Signatures: (SignatureListSize - 28 - SignatureHeaderSize) bytes]

        let mut offset = 0;
        while offset + 28 <= data.len() {
            // Parse signature type GUID
            let sig_type = parse_signature_type(&data[offset..offset + 16]);
            let list_size = u32::from_le_bytes([
                data[offset + 16], data[offset + 17],
                data[offset + 18], data[offset + 19]
            ]) as usize;
            let header_size = u32::from_le_bytes([
                data[offset + 20], data[offset + 21],
                data[offset + 22], data[offset + 23]
            ]) as usize;
            let sig_size = u32::from_le_bytes([
                data[offset + 24], data[offset + 25],
                data[offset + 26], data[offset + 27]
            ]) as usize;

            if list_size == 0 || sig_size == 0 {
                break;
            }

            // Skip header
            let sig_start = offset + 28 + header_size;
            let sig_end = offset + list_size;

            // Parse individual signatures
            let mut sig_offset = sig_start;
            while sig_offset + sig_size <= sig_end {
                // First 16 bytes is SignatureOwner GUID
                let owner = parse_guid(&data[sig_offset..sig_offset + 16]);
                let sig_data = data[sig_offset + 16..sig_offset + sig_size].to_vec();

                db.entries.push(SignatureEntry {
                    sig_type,
                    owner,
                    data: sig_data,
                });

                sig_offset += sig_size;
            }

            offset += list_size;
        }

        Some(db)
    }

    /// Check if a hash is in the database
    pub fn contains_hash(&self, hash: &[u8]) -> bool {
        for entry in &self.entries {
            if matches!(entry.sig_type, SignatureType::Sha256 | SignatureType::Sha384 | SignatureType::Sha512) {
                if entry.data == hash {
                    return true;
                }
            }
        }
        false
    }

    /// Get all X.509 certificates
    pub fn get_certificates(&self) -> Vec<&SignatureEntry> {
        self.entries.iter()
            .filter(|e| matches!(e.sig_type, SignatureType::X509 | SignatureType::X509Chain))
            .collect()
    }

    /// Number of entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Parse GUID from bytes
fn parse_guid(data: &[u8]) -> EfiGuid {
    if data.len() < 16 {
        return EfiGuid::new(0, 0, 0, [0; 8]);
    }

    EfiGuid {
        data1: u32::from_le_bytes([data[0], data[1], data[2], data[3]]),
        data2: u16::from_le_bytes([data[4], data[5]]),
        data3: u16::from_le_bytes([data[6], data[7]]),
        data4: [data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15]],
    }
}

/// Parse signature type from GUID
fn parse_signature_type(data: &[u8]) -> SignatureType {
    let guid = parse_guid(data);

    if guid == guids::EFI_CERT_SHA256_GUID {
        SignatureType::Sha256
    } else if guid == guids::EFI_CERT_X509_GUID {
        SignatureType::X509
    } else if guid == guids::EFI_CERT_RSA2048_GUID {
        SignatureType::Rsa2048
    } else {
        SignatureType::Unknown
    }
}

/// Secure Boot state
pub struct SecureBootState {
    /// Secure Boot mode
    mode: SecureBootMode,
    /// Platform Key (PK)
    pk: Option<SignatureDatabase>,
    /// Key Exchange Keys (KEK)
    kek: Option<SignatureDatabase>,
    /// Authorized signature database (db)
    db: Option<SignatureDatabase>,
    /// Forbidden signature database (dbx)
    dbx: Option<SignatureDatabase>,
    /// Machine Owner Keys (MOK)
    mok: Option<SignatureDatabase>,
    /// MOK blacklist (MOKX)
    mokx: Option<SignatureDatabase>,
    /// Shim present
    shim_present: bool,
    /// Initialized
    initialized: bool,
}

impl SecureBootState {
    pub const fn new() -> Self {
        Self {
            mode: SecureBootMode::Disabled,
            pk: None,
            kek: None,
            db: None,
            dbx: None,
            mok: None,
            mokx: None,
            shim_present: false,
            initialized: false,
        }
    }

    /// Initialize Secure Boot state from EFI variables
    pub fn init(&mut self) {
        // Read Secure Boot variable
        if let Some(sb_var) = read_efi_variable("SecureBoot", &guids::EFI_GLOBAL_VARIABLE) {
            if !sb_var.is_empty() && sb_var[0] == 1 {
                self.mode = SecureBootMode::Enabled;
            }
        }

        // Read Setup Mode variable
        if let Some(setup_var) = read_efi_variable("SetupMode", &guids::EFI_GLOBAL_VARIABLE) {
            if !setup_var.is_empty() && setup_var[0] == 1 {
                self.mode = SecureBootMode::SetupMode;
            }
        }

        // Load signature databases
        if let Some(pk_data) = read_efi_variable("PK", &guids::EFI_GLOBAL_VARIABLE) {
            self.pk = SignatureDatabase::from_efi_var("PK", &pk_data);
        }

        if let Some(kek_data) = read_efi_variable("KEK", &guids::EFI_GLOBAL_VARIABLE) {
            self.kek = SignatureDatabase::from_efi_var("KEK", &kek_data);
        }

        if let Some(db_data) = read_efi_variable("db", &guids::EFI_IMAGE_SECURITY_DATABASE) {
            self.db = SignatureDatabase::from_efi_var("db", &db_data);
        }

        if let Some(dbx_data) = read_efi_variable("dbx", &guids::EFI_IMAGE_SECURITY_DATABASE) {
            self.dbx = SignatureDatabase::from_efi_var("dbx", &dbx_data);
        }

        // Check for shim MOK
        if let Some(mok_data) = read_efi_variable("MokListRT", &guids::SHIM_LOCK_GUID) {
            self.mok = SignatureDatabase::from_efi_var("MokList", &mok_data);
            self.shim_present = true;
        }

        if let Some(mokx_data) = read_efi_variable("MokListXRT", &guids::SHIM_LOCK_GUID) {
            self.mokx = SignatureDatabase::from_efi_var("MokListX", &mokx_data);
        }

        self.initialized = true;

        crate::kprintln!("secureboot: mode={:?}, db={} certs, dbx={} entries",
            self.mode,
            self.db.as_ref().map(|d| d.len()).unwrap_or(0),
            self.dbx.as_ref().map(|d| d.len()).unwrap_or(0)
        );
    }

    /// Get Secure Boot mode
    pub fn mode(&self) -> SecureBootMode {
        self.mode
    }

    /// Check if Secure Boot is enabled
    pub fn is_enabled(&self) -> bool {
        matches!(self.mode, SecureBootMode::Enabled | SecureBootMode::DeployedMode)
    }

    /// Check if in Setup Mode
    pub fn is_setup_mode(&self) -> bool {
        self.mode == SecureBootMode::SetupMode
    }

    /// Check if shim is present
    pub fn has_shim(&self) -> bool {
        self.shim_present
    }

    /// Verify a PE image hash against db/dbx
    pub fn verify_image_hash(&self, hash: &[u8]) -> VerifyResult {
        // First check dbx (blacklist)
        if let Some(ref dbx) = self.dbx {
            if dbx.contains_hash(hash) {
                return VerifyResult::Blacklisted;
            }
        }

        // Check MOK blacklist
        if let Some(ref mokx) = self.mokx {
            if mokx.contains_hash(hash) {
                return VerifyResult::Blacklisted;
            }
        }

        // Check db (whitelist)
        if let Some(ref db) = self.db {
            if db.contains_hash(hash) {
                return VerifyResult::Verified;
            }
        }

        // Check MOK
        if let Some(ref mok) = self.mok {
            if mok.contains_hash(hash) {
                return VerifyResult::VerifiedMok;
            }
        }

        // Not found - if Secure Boot enabled, this is a failure
        if self.is_enabled() {
            VerifyResult::NotTrusted
        } else {
            VerifyResult::NotVerified
        }
    }

    /// Get authorized certificates (db + MOK)
    pub fn get_trusted_certs(&self) -> Vec<&SignatureEntry> {
        let mut certs = Vec::new();

        if let Some(ref db) = self.db {
            certs.extend(db.get_certificates());
        }

        if let Some(ref mok) = self.mok {
            certs.extend(mok.get_certificates());
        }

        certs
    }

    /// Get blacklisted hashes/certs (dbx + MOKX)
    pub fn get_blacklist(&self) -> Vec<&SignatureEntry> {
        let mut entries = Vec::new();

        if let Some(ref dbx) = self.dbx {
            entries.extend(dbx.entries.iter());
        }

        if let Some(ref mokx) = self.mokx {
            entries.extend(mokx.entries.iter());
        }

        entries
    }

    /// Get statistics
    pub fn stats(&self) -> SecureBootStats {
        SecureBootStats {
            mode: self.mode,
            pk_present: self.pk.is_some(),
            kek_count: self.kek.as_ref().map(|k| k.len()).unwrap_or(0),
            db_count: self.db.as_ref().map(|d| d.len()).unwrap_or(0),
            dbx_count: self.dbx.as_ref().map(|d| d.len()).unwrap_or(0),
            mok_count: self.mok.as_ref().map(|m| m.len()).unwrap_or(0),
            mokx_count: self.mokx.as_ref().map(|m| m.len()).unwrap_or(0),
            shim_present: self.shim_present,
        }
    }
}

/// Verification result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyResult {
    /// Image verified via db
    Verified,
    /// Image verified via MOK
    VerifiedMok,
    /// Image hash found in blacklist (dbx/MOKX)
    Blacklisted,
    /// Image not in any trust database
    NotTrusted,
    /// Secure Boot disabled, not verified
    NotVerified,
    /// Verification error
    Error,
}

/// Secure Boot statistics
#[derive(Debug, Clone)]
pub struct SecureBootStats {
    pub mode: SecureBootMode,
    pub pk_present: bool,
    pub kek_count: usize,
    pub db_count: usize,
    pub dbx_count: usize,
    pub mok_count: usize,
    pub mokx_count: usize,
    pub shim_present: bool,
}

/// Read an EFI variable
fn read_efi_variable(_name: &str, _guid: &EfiGuid) -> Option<Vec<u8>> {
    // In a real implementation, this would call UEFI Runtime Services
    // GetVariable() or read from /sys/firmware/efi/efivars/

    // For now, return None (no UEFI runtime support yet)
    None
}

/// Write an EFI variable
#[allow(dead_code)]
fn write_efi_variable(_name: &str, _guid: &EfiGuid, _data: &[u8]) -> KResult<()> {
    // Would call UEFI Runtime Services SetVariable()
    Err(KError::NotSupported)
}

// =============================================================================
// Global State
// =============================================================================

static SECURE_BOOT: IrqSafeMutex<SecureBootState> = IrqSafeMutex::new(SecureBootState::new());
static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Initialize Secure Boot subsystem
pub fn init() {
    if INITIALIZED.load(Ordering::Acquire) {
        return;
    }

    SECURE_BOOT.lock().init();
    INITIALIZED.store(true, Ordering::Release);

    let stats = SECURE_BOOT.lock().stats();
    if stats.mode == SecureBootMode::Enabled {
        crate::kprintln!("secureboot: UEFI Secure Boot is ENABLED");
    } else {
        crate::kprintln!("secureboot: Secure Boot disabled or not available");
    }
}

/// Get current Secure Boot mode
pub fn mode() -> SecureBootMode {
    SECURE_BOOT.lock().mode()
}

/// Check if Secure Boot is enabled
pub fn is_enabled() -> bool {
    SECURE_BOOT.lock().is_enabled()
}

/// Check if in Setup Mode
pub fn is_setup_mode() -> bool {
    SECURE_BOOT.lock().is_setup_mode()
}

/// Verify a PE image hash
pub fn verify_hash(hash: &[u8]) -> VerifyResult {
    SECURE_BOOT.lock().verify_image_hash(hash)
}

/// Get Secure Boot statistics
pub fn stats() -> SecureBootStats {
    SECURE_BOOT.lock().stats()
}

/// Check if shim is present
pub fn has_shim() -> bool {
    SECURE_BOOT.lock().has_shim()
}
