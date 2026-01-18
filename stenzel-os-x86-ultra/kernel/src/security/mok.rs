//! Machine Owner Key (MOK) Manager
//!
//! Provides MOK management for Secure Boot key enrollment.
//! Allows users to enroll their own signing keys for:
//! - Custom kernel modules
//! - Third-party drivers (NVIDIA, VirtualBox, etc.)
//! - Custom kernels
//!
//! Works with shim bootloader for MOK persistence.
//!
//! Features:
//! - Key enrollment and revocation
//! - Password-protected operations
//! - Pending enrollment management
//! - Certificate validation
//! - Export/import of keys

#![allow(dead_code)]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use alloc::collections::BTreeMap;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use spin::{Mutex, RwLock};

use crate::util::{KResult, KError};
use super::secureboot::{EfiGuid, SignatureEntry, SignatureType, SignatureDatabase};

/// MOK operation type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MokOperation {
    /// Enroll a new key
    Enroll,
    /// Delete an existing key
    Delete,
    /// Reset MOK list
    Reset,
    /// Import from file
    Import,
    /// Export to file
    Export,
}

/// MOK entry state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MokState {
    /// Key is enrolled and active
    Enrolled,
    /// Key is pending enrollment (needs reboot confirmation)
    PendingEnroll,
    /// Key is pending deletion (needs reboot confirmation)
    PendingDelete,
    /// Key enrollment was rejected
    Rejected,
}

/// MOK key type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MokKeyType {
    /// X.509 certificate (DER format)
    X509,
    /// X.509 certificate (PEM format)
    X509Pem,
    /// RSA-2048 public key
    Rsa2048,
    /// SHA-256 hash
    Sha256,
}

impl MokKeyType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::X509 => "X.509 DER",
            Self::X509Pem => "X.509 PEM",
            Self::Rsa2048 => "RSA-2048",
            Self::Sha256 => "SHA-256",
        }
    }

    pub fn to_signature_type(&self) -> SignatureType {
        match self {
            Self::X509 | Self::X509Pem => SignatureType::X509,
            Self::Rsa2048 => SignatureType::Rsa2048,
            Self::Sha256 => SignatureType::Sha256,
        }
    }
}

/// MOK entry information
#[derive(Debug, Clone)]
pub struct MokEntry {
    /// Unique ID
    pub id: u32,
    /// Key type
    pub key_type: MokKeyType,
    /// Key data
    pub data: Vec<u8>,
    /// Common Name (from certificate)
    pub common_name: String,
    /// Issuer (from certificate)
    pub issuer: String,
    /// Subject (from certificate)
    pub subject: String,
    /// Not Before date
    pub not_before: String,
    /// Not After date
    pub not_after: String,
    /// Fingerprint (SHA-256)
    pub fingerprint: [u8; 32],
    /// Entry state
    pub state: MokState,
    /// When enrolled
    pub enrolled_at: u64,
    /// Description
    pub description: String,
    /// Owner GUID
    pub owner: EfiGuid,
}

impl MokEntry {
    /// Create new MOK entry from certificate data
    pub fn from_certificate(id: u32, key_type: MokKeyType, data: &[u8]) -> Option<Self> {
        // Parse certificate to extract metadata
        let (common_name, issuer, subject, not_before, not_after) =
            parse_certificate_info(data)?;

        // Calculate fingerprint
        let fingerprint = calculate_sha256(data);

        Some(Self {
            id,
            key_type,
            data: data.to_vec(),
            common_name,
            issuer,
            subject,
            not_before,
            not_after,
            fingerprint,
            state: MokState::PendingEnroll,
            enrolled_at: 0,
            description: String::new(),
            owner: EfiGuid::new(0, 0, 0, [0; 8]),
        })
    }

    /// Get fingerprint as hex string
    pub fn fingerprint_hex(&self) -> String {
        let mut hex = String::with_capacity(64);
        for (i, byte) in self.fingerprint.iter().enumerate() {
            if i > 0 && i % 2 == 0 {
                hex.push(':');
            }
            hex.push_str(&format!("{:02X}", byte));
        }
        hex
    }

    /// Check if certificate is expired
    pub fn is_expired(&self) -> bool {
        // Would check not_after against current time
        // For now, return false
        false
    }

    /// Check if certificate is not yet valid
    pub fn is_not_yet_valid(&self) -> bool {
        // Would check not_before against current time
        false
    }
}

/// Pending MOK operation
#[derive(Debug, Clone)]
pub struct PendingOperation {
    /// Operation type
    pub operation: MokOperation,
    /// Target entry ID (for Delete)
    pub entry_id: Option<u32>,
    /// New certificate data (for Enroll)
    pub certificate: Option<Vec<u8>>,
    /// Password hash for confirmation
    pub password_hash: [u8; 32],
    /// Request timestamp
    pub requested_at: u64,
    /// Expiry timestamp
    pub expires_at: u64,
}

/// MOK manager configuration
#[derive(Debug, Clone)]
pub struct MokConfig {
    /// Maximum number of MOK entries
    pub max_entries: usize,
    /// Pending operation timeout (seconds)
    pub pending_timeout_secs: u64,
    /// Require password for all operations
    pub require_password: bool,
    /// Minimum password length
    pub min_password_length: usize,
    /// Allow self-signed certificates
    pub allow_self_signed: bool,
    /// Enable audit logging
    pub audit_logging: bool,
}

impl Default for MokConfig {
    fn default() -> Self {
        Self {
            max_entries: 128,
            pending_timeout_secs: 600, // 10 minutes
            require_password: true,
            min_password_length: 8,
            allow_self_signed: true,
            audit_logging: true,
        }
    }
}

/// MOK manager statistics
#[derive(Debug, Clone, Default)]
pub struct MokStats {
    /// Total enrolled keys
    pub enrolled_count: u32,
    /// Pending enrollments
    pub pending_enroll_count: u32,
    /// Pending deletions
    pub pending_delete_count: u32,
    /// Total operations performed
    pub operations_count: u32,
    /// Failed operations
    pub failed_operations: u32,
    /// Rejected enrollments
    pub rejected_count: u32,
}

/// MOK operation result
#[derive(Debug, Clone)]
pub enum MokResult {
    /// Operation successful
    Success,
    /// Operation pending (needs reboot confirmation)
    Pending,
    /// Operation failed
    Failed(String),
    /// Password required
    PasswordRequired,
    /// Invalid certificate
    InvalidCertificate,
    /// Certificate already enrolled
    AlreadyEnrolled,
    /// Certificate not found
    NotFound,
    /// Maximum entries reached
    MaxEntriesReached,
}

/// MOK Manager
pub struct MokManager {
    /// Enrolled MOK entries
    entries: RwLock<BTreeMap<u32, MokEntry>>,
    /// Pending operations
    pending: Mutex<Vec<PendingOperation>>,
    /// Configuration
    config: RwLock<MokConfig>,
    /// Statistics
    stats: Mutex<MokStats>,
    /// Next entry ID
    next_id: AtomicU32,
    /// Initialized flag
    initialized: AtomicBool,
    /// Shim present flag
    shim_present: AtomicBool,
}

impl MokManager {
    pub const fn new() -> Self {
        Self {
            entries: RwLock::new(BTreeMap::new()),
            pending: Mutex::new(Vec::new()),
            config: RwLock::new(MokConfig {
                max_entries: 128,
                pending_timeout_secs: 600,
                require_password: true,
                min_password_length: 8,
                allow_self_signed: true,
                audit_logging: true,
            }),
            stats: Mutex::new(MokStats {
                enrolled_count: 0,
                pending_enroll_count: 0,
                pending_delete_count: 0,
                operations_count: 0,
                failed_operations: 0,
                rejected_count: 0,
            }),
            next_id: AtomicU32::new(1),
            initialized: AtomicBool::new(false),
            shim_present: AtomicBool::new(false),
        }
    }

    /// Initialize the MOK manager
    pub fn init(&self) -> KResult<()> {
        crate::kprintln!("mok: initializing MOK manager...");

        // Check if shim is present
        let shim_present = super::secureboot::has_shim();
        self.shim_present.store(shim_present, Ordering::SeqCst);

        if shim_present {
            // Load existing MOK entries from EFI variables
            self.load_mok_list()?;
        }

        self.initialized.store(true, Ordering::SeqCst);

        let stats = self.stats.lock();
        crate::kprintln!("mok: initialized, {} entries enrolled, shim={}",
            stats.enrolled_count, shim_present);

        Ok(())
    }

    /// Load MOK list from EFI variables
    fn load_mok_list(&self) -> KResult<()> {
        // Read MokListRT from shim
        // In a real implementation, this would call read_efi_variable()

        // For now, start with empty list
        Ok(())
    }

    /// Enroll a new MOK key
    pub fn enroll(
        &self,
        key_type: MokKeyType,
        certificate: &[u8],
        password: &str,
        description: &str,
    ) -> MokResult {
        let config = self.config.read();

        // Validate password
        if config.require_password && password.len() < config.min_password_length {
            return MokResult::Failed(format!(
                "Password must be at least {} characters",
                config.min_password_length
            ));
        }

        // Check max entries
        let entries = self.entries.read();
        if entries.len() >= config.max_entries {
            return MokResult::MaxEntriesReached;
        }

        // Validate certificate
        if !self.validate_certificate(key_type, certificate) {
            return MokResult::InvalidCertificate;
        }

        // Check if already enrolled
        let fingerprint = calculate_sha256(certificate);
        for entry in entries.values() {
            if entry.fingerprint == fingerprint {
                return MokResult::AlreadyEnrolled;
            }
        }
        drop(entries);

        // Create new entry
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let mut entry = match MokEntry::from_certificate(id, key_type, certificate) {
            Some(e) => e,
            None => return MokResult::InvalidCertificate,
        };
        entry.description = String::from(description);
        entry.state = MokState::PendingEnroll;

        // Calculate password hash
        let password_hash = calculate_sha256(password.as_bytes());

        // Add to entries
        {
            let mut entries = self.entries.write();
            entries.insert(id, entry);
        }

        // Create pending operation
        {
            let mut pending = self.pending.lock();
            pending.push(PendingOperation {
                operation: MokOperation::Enroll,
                entry_id: Some(id),
                certificate: Some(certificate.to_vec()),
                password_hash,
                requested_at: get_timestamp(),
                expires_at: get_timestamp() + config.pending_timeout_secs,
            });
        }

        // Update stats
        {
            let mut stats = self.stats.lock();
            stats.pending_enroll_count += 1;
            stats.operations_count += 1;
        }

        if config.audit_logging {
            crate::kprintln!("mok: enrollment request for key {} (pending)", id);
        }

        MokResult::Pending
    }

    /// Delete a MOK key
    pub fn delete(&self, entry_id: u32, password: &str) -> MokResult {
        let config = self.config.read();

        // Validate password
        if config.require_password && password.len() < config.min_password_length {
            return MokResult::Failed("Invalid password".to_string());
        }

        // Check if entry exists
        let entries = self.entries.read();
        if !entries.contains_key(&entry_id) {
            return MokResult::NotFound;
        }
        drop(entries);

        // Mark for deletion
        {
            let mut entries = self.entries.write();
            if let Some(entry) = entries.get_mut(&entry_id) {
                entry.state = MokState::PendingDelete;
            }
        }

        // Calculate password hash
        let password_hash = calculate_sha256(password.as_bytes());

        // Create pending operation
        {
            let mut pending = self.pending.lock();
            pending.push(PendingOperation {
                operation: MokOperation::Delete,
                entry_id: Some(entry_id),
                certificate: None,
                password_hash,
                requested_at: get_timestamp(),
                expires_at: get_timestamp() + config.pending_timeout_secs,
            });
        }

        // Update stats
        {
            let mut stats = self.stats.lock();
            stats.pending_delete_count += 1;
            stats.operations_count += 1;
        }

        if config.audit_logging {
            crate::kprintln!("mok: deletion request for key {} (pending)", entry_id);
        }

        MokResult::Pending
    }

    /// Confirm a pending operation (called from MOK manager UI at boot)
    pub fn confirm_pending(&self, password: &str) -> MokResult {
        let password_hash = calculate_sha256(password.as_bytes());
        let current_time = get_timestamp();

        let mut pending = self.pending.lock();
        let mut confirmed = Vec::new();

        // Find matching pending operations
        for (idx, op) in pending.iter().enumerate() {
            if op.password_hash == password_hash && op.expires_at > current_time {
                confirmed.push(idx);
            }
        }

        if confirmed.is_empty() {
            return MokResult::Failed("Invalid password or no pending operations".to_string());
        }

        // Process confirmed operations (in reverse to maintain indices)
        for idx in confirmed.into_iter().rev() {
            let op = pending.remove(idx);

            match op.operation {
                MokOperation::Enroll => {
                    if let Some(entry_id) = op.entry_id {
                        let mut entries = self.entries.write();
                        if let Some(entry) = entries.get_mut(&entry_id) {
                            entry.state = MokState::Enrolled;
                            entry.enrolled_at = current_time;
                        }
                        drop(entries);

                        let mut stats = self.stats.lock();
                        stats.enrolled_count += 1;
                        stats.pending_enroll_count = stats.pending_enroll_count.saturating_sub(1);

                        crate::kprintln!("mok: key {} enrolled successfully", entry_id);
                    }
                }
                MokOperation::Delete => {
                    if let Some(entry_id) = op.entry_id {
                        let mut entries = self.entries.write();
                        entries.remove(&entry_id);
                        drop(entries);

                        let mut stats = self.stats.lock();
                        stats.enrolled_count = stats.enrolled_count.saturating_sub(1);
                        stats.pending_delete_count = stats.pending_delete_count.saturating_sub(1);

                        crate::kprintln!("mok: key {} deleted successfully", entry_id);
                    }
                }
                MokOperation::Reset => {
                    let mut entries = self.entries.write();
                    entries.clear();
                    drop(entries);

                    let mut stats = self.stats.lock();
                    stats.enrolled_count = 0;
                    stats.pending_enroll_count = 0;
                    stats.pending_delete_count = 0;

                    crate::kprintln!("mok: MOK list reset");
                }
                _ => {}
            }
        }

        // Save to EFI variables
        if let Err(e) = self.save_mok_list() {
            return MokResult::Failed(format!("Failed to save: {:?}", e));
        }

        MokResult::Success
    }

    /// Cancel a pending operation
    pub fn cancel_pending(&self, entry_id: Option<u32>) -> MokResult {
        let mut pending = self.pending.lock();

        // Remove matching pending operations
        pending.retain(|op| {
            if let Some(id) = entry_id {
                op.entry_id != Some(id)
            } else {
                false // Remove all if no ID specified
            }
        });

        // Reset entry states
        if let Some(id) = entry_id {
            let mut entries = self.entries.write();
            if let Some(entry) = entries.get_mut(&id) {
                if entry.state == MokState::PendingEnroll {
                    // Remove entry if it was pending enrollment
                    entries.remove(&id);
                } else if entry.state == MokState::PendingDelete {
                    // Restore to enrolled state
                    entry.state = MokState::Enrolled;
                }
            }
        }

        let config = self.config.read();
        if config.audit_logging {
            crate::kprintln!("mok: pending operation cancelled");
        }

        MokResult::Success
    }

    /// Reset MOK list (requires password confirmation at next boot)
    pub fn reset(&self, password: &str) -> MokResult {
        let config = self.config.read();

        if config.require_password && password.len() < config.min_password_length {
            return MokResult::Failed("Invalid password".to_string());
        }

        let password_hash = calculate_sha256(password.as_bytes());

        let mut pending = self.pending.lock();
        pending.push(PendingOperation {
            operation: MokOperation::Reset,
            entry_id: None,
            certificate: None,
            password_hash,
            requested_at: get_timestamp(),
            expires_at: get_timestamp() + config.pending_timeout_secs,
        });

        if config.audit_logging {
            crate::kprintln!("mok: reset request (pending)");
        }

        MokResult::Pending
    }

    /// Validate a certificate
    fn validate_certificate(&self, key_type: MokKeyType, data: &[u8]) -> bool {
        match key_type {
            MokKeyType::X509 => validate_x509_der(data),
            MokKeyType::X509Pem => validate_x509_pem(data),
            MokKeyType::Rsa2048 => data.len() == 256, // 2048 bits
            MokKeyType::Sha256 => data.len() == 32,
        }
    }

    /// Save MOK list to EFI variables
    fn save_mok_list(&self) -> KResult<()> {
        // Build EFI_SIGNATURE_LIST
        let entries = self.entries.read();

        if entries.is_empty() {
            // Write empty MokList
            return Ok(());
        }

        // In a real implementation, this would call write_efi_variable()
        // to update MokNew/MokAuth for shim to process on next boot

        Ok(())
    }

    /// Get a specific entry
    pub fn get_entry(&self, id: u32) -> Option<MokEntry> {
        self.entries.read().get(&id).cloned()
    }

    /// Get all enrolled entries
    pub fn get_enrolled(&self) -> Vec<MokEntry> {
        self.entries.read()
            .values()
            .filter(|e| e.state == MokState::Enrolled)
            .cloned()
            .collect()
    }

    /// Get all pending entries
    pub fn get_pending(&self) -> Vec<MokEntry> {
        self.entries.read()
            .values()
            .filter(|e| matches!(e.state, MokState::PendingEnroll | MokState::PendingDelete))
            .cloned()
            .collect()
    }

    /// Get all entries
    pub fn get_all(&self) -> Vec<MokEntry> {
        self.entries.read().values().cloned().collect()
    }

    /// Check if a hash is in the MOK list
    pub fn contains_hash(&self, hash: &[u8]) -> bool {
        self.entries.read()
            .values()
            .filter(|e| e.state == MokState::Enrolled)
            .any(|e| e.fingerprint.as_slice() == hash || e.data == hash)
    }

    /// Export entry to DER format
    pub fn export_der(&self, id: u32) -> Option<Vec<u8>> {
        self.entries.read()
            .get(&id)
            .filter(|e| matches!(e.key_type, MokKeyType::X509 | MokKeyType::X509Pem))
            .map(|e| e.data.clone())
    }

    /// Export entry to PEM format
    pub fn export_pem(&self, id: u32) -> Option<String> {
        self.entries.read()
            .get(&id)
            .filter(|e| matches!(e.key_type, MokKeyType::X509 | MokKeyType::X509Pem))
            .map(|e| {
                let base64 = base64_encode(&e.data);
                format!(
                    "-----BEGIN CERTIFICATE-----\n{}\n-----END CERTIFICATE-----\n",
                    base64
                )
            })
    }

    /// Get configuration
    pub fn get_config(&self) -> MokConfig {
        self.config.read().clone()
    }

    /// Set configuration
    pub fn set_config(&self, config: MokConfig) {
        let mut current = self.config.write();
        *current = config;
    }

    /// Get statistics
    pub fn get_stats(&self) -> MokStats {
        self.stats.lock().clone()
    }

    /// Check if shim is present
    pub fn has_shim(&self) -> bool {
        self.shim_present.load(Ordering::SeqCst)
    }

    /// Check if MOK manager is available
    pub fn is_available(&self) -> bool {
        self.initialized.load(Ordering::SeqCst) && self.shim_present.load(Ordering::SeqCst)
    }

    /// Format status for display
    pub fn format_status(&self) -> String {
        let stats = self.stats.lock();
        let config = self.config.read();

        format!(
            "MOK Manager Status:\n\
             - Initialized: {}\n\
             - Shim present: {}\n\
             - Enrolled keys: {}\n\
             - Pending enrollments: {}\n\
             - Pending deletions: {}\n\
             - Max entries: {}\n\
             - Require password: {}\n",
            self.initialized.load(Ordering::SeqCst),
            self.shim_present.load(Ordering::SeqCst),
            stats.enrolled_count,
            stats.pending_enroll_count,
            stats.pending_delete_count,
            config.max_entries,
            config.require_password
        )
    }

    /// Cleanup expired pending operations
    pub fn cleanup_expired(&self) {
        let current_time = get_timestamp();

        let mut pending = self.pending.lock();
        let before_len = pending.len();

        pending.retain(|op| op.expires_at > current_time);

        let expired = before_len - pending.len();
        if expired > 0 {
            let mut stats = self.stats.lock();
            stats.rejected_count += expired as u32;

            crate::kprintln!("mok: {} expired operations removed", expired);
        }
    }
}

// Helper functions

/// Calculate SHA-256 hash
fn calculate_sha256(data: &[u8]) -> [u8; 32] {
    // Use the kernel's SHA-256 implementation
    crate::crypto::sha256(data)
}

/// Parse X.509 certificate information
fn parse_certificate_info(data: &[u8]) -> Option<(String, String, String, String, String)> {
    // Very simplified X.509 DER parsing
    // Real implementation would use a proper ASN.1 parser

    if data.len() < 10 {
        return None;
    }

    // Check for SEQUENCE tag
    if data[0] != 0x30 {
        return None;
    }

    // For now, return placeholder values
    // A real implementation would parse the certificate structure
    Some((
        String::from("Unknown CN"),
        String::from("Unknown Issuer"),
        String::from("Unknown Subject"),
        String::from("Unknown"),
        String::from("Unknown"),
    ))
}

/// Validate X.509 DER certificate
fn validate_x509_der(data: &[u8]) -> bool {
    // Basic DER format check
    if data.len() < 10 {
        return false;
    }

    // Must start with SEQUENCE tag
    if data[0] != 0x30 {
        return false;
    }

    // Check length encoding
    if data[1] & 0x80 != 0 {
        let len_bytes = (data[1] & 0x7F) as usize;
        if len_bytes > 4 || data.len() < 2 + len_bytes {
            return false;
        }
    }

    true
}

/// Validate X.509 PEM certificate
fn validate_x509_pem(data: &[u8]) -> bool {
    let text = match core::str::from_utf8(data) {
        Ok(s) => s,
        Err(_) => return false,
    };

    text.contains("-----BEGIN CERTIFICATE-----")
        && text.contains("-----END CERTIFICATE-----")
}

/// Base64 encode
fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);
    let mut i = 0;

    while i + 3 <= data.len() {
        let n = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8) | (data[i + 2] as u32);
        result.push(ALPHABET[((n >> 18) & 63) as usize] as char);
        result.push(ALPHABET[((n >> 12) & 63) as usize] as char);
        result.push(ALPHABET[((n >> 6) & 63) as usize] as char);
        result.push(ALPHABET[(n & 63) as usize] as char);
        i += 3;
    }

    match data.len() - i {
        1 => {
            let n = (data[i] as u32) << 16;
            result.push(ALPHABET[((n >> 18) & 63) as usize] as char);
            result.push(ALPHABET[((n >> 12) & 63) as usize] as char);
            result.push('=');
            result.push('=');
        }
        2 => {
            let n = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8);
            result.push(ALPHABET[((n >> 18) & 63) as usize] as char);
            result.push(ALPHABET[((n >> 12) & 63) as usize] as char);
            result.push(ALPHABET[((n >> 6) & 63) as usize] as char);
            result.push('=');
        }
        _ => {}
    }

    result
}

/// Get current timestamp (seconds)
fn get_timestamp() -> u64 {
    // Use kernel uptime
    crate::time::uptime_secs()
}

// ============================================================================
// Global instance
// ============================================================================

static MOK_MANAGER: MokManager = MokManager::new();

/// Initialize MOK manager
pub fn init() -> KResult<()> {
    MOK_MANAGER.init()
}

/// Enroll a new MOK key
pub fn enroll(key_type: MokKeyType, certificate: &[u8], password: &str, description: &str) -> MokResult {
    MOK_MANAGER.enroll(key_type, certificate, password, description)
}

/// Delete a MOK key
pub fn delete(entry_id: u32, password: &str) -> MokResult {
    MOK_MANAGER.delete(entry_id, password)
}

/// Confirm pending operations
pub fn confirm_pending(password: &str) -> MokResult {
    MOK_MANAGER.confirm_pending(password)
}

/// Cancel pending operation
pub fn cancel_pending(entry_id: Option<u32>) -> MokResult {
    MOK_MANAGER.cancel_pending(entry_id)
}

/// Reset MOK list
pub fn reset(password: &str) -> MokResult {
    MOK_MANAGER.reset(password)
}

/// Get all enrolled entries
pub fn get_enrolled() -> Vec<MokEntry> {
    MOK_MANAGER.get_enrolled()
}

/// Get all pending entries
pub fn get_pending() -> Vec<MokEntry> {
    MOK_MANAGER.get_pending()
}

/// Get all entries
pub fn get_all() -> Vec<MokEntry> {
    MOK_MANAGER.get_all()
}

/// Get specific entry
pub fn get_entry(id: u32) -> Option<MokEntry> {
    MOK_MANAGER.get_entry(id)
}

/// Check if hash is in MOK list
pub fn contains_hash(hash: &[u8]) -> bool {
    MOK_MANAGER.contains_hash(hash)
}

/// Export entry as DER
pub fn export_der(id: u32) -> Option<Vec<u8>> {
    MOK_MANAGER.export_der(id)
}

/// Export entry as PEM
pub fn export_pem(id: u32) -> Option<String> {
    MOK_MANAGER.export_pem(id)
}

/// Get configuration
pub fn get_config() -> MokConfig {
    MOK_MANAGER.get_config()
}

/// Set configuration
pub fn set_config(config: MokConfig) {
    MOK_MANAGER.set_config(config)
}

/// Get statistics
pub fn get_stats() -> MokStats {
    MOK_MANAGER.get_stats()
}

/// Check if MOK manager is available
pub fn is_available() -> bool {
    MOK_MANAGER.is_available()
}

/// Check if shim is present
pub fn has_shim() -> bool {
    MOK_MANAGER.has_shim()
}

/// Format status
pub fn format_status() -> String {
    MOK_MANAGER.format_status()
}

/// Cleanup expired operations
pub fn cleanup_expired() {
    MOK_MANAGER.cleanup_expired()
}
